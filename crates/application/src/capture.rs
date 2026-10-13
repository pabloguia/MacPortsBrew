// SPDX-License-Identifier: Apache-2.0

//! Capturing a snapshot through the domain's read contracts. The only
//! place the application layer talks to providers; everything after it is
//! a function of the snapshot.

use crate::TOOL_VERSION;
use macportsbrew_domain::ProviderEnvironmentContract;
use macportsbrew_domain::ProviderPresence;
use macportsbrew_domain::{
    redact_home, DomainError, ErrorId, HomeRedaction, MachineRead, Provenance, ProviderId,
    ProviderRead, ProviderSnapshot, Snapshot, SNAPSHOT_SCHEMA_VERSION,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// What a capture needs.
pub struct CaptureContext<'a> {
    /// The machine reader.
    pub machine: &'a dyn MachineRead,
    /// Every provider the tool knows, present or not.
    pub providers: Vec<&'a dyn ProviderRead>,
    /// Policy version in effect.
    pub policy_version: String,
    /// Capture time, RFC 3339 UTC, passed in.
    pub captured_at: String,
    /// The command that captured it.
    pub captured_by: String,
    /// The home directory to redact out of everything the providers say.
    ///
    /// `None` under replay and when the home is unknown, which is also what
    /// makes redaction a no-op there: a recording was redacted when it was
    /// captured, and re-redacting it against *this* machine's home would be
    /// a host fact leaking into a replayed analysis (INV-15).
    pub home: HomeRedaction,
    /// What each provider's adapter could have put in an invocation
    /// environment, from the layer that knows providers.
    ///
    /// Required rather than optional, and consumed here rather than by the
    /// caller, because a capture that is not checked is the gap: `capture` is
    /// public, and a future subcommand or embedder that built a snapshot
    /// without running the rule would reopen F-21 silently. Making the facts
    /// part of the context means a `CaptureContext` cannot be constructed
    /// without them.
    pub environment_contracts: BTreeMap<String, ProviderEnvironmentContract>,
}

/// Captures a snapshot. A provider that fails to answer is recorded as
/// broken with the error identifier; the capture itself only fails when the
/// machine cannot be read at all.
pub fn capture(ctx: &CaptureContext<'_>) -> Result<Snapshot, DomainError> {
    let machine = ctx.machine.machine()?;
    let mut environment = ctx.machine.environment()?;
    let mut providers = BTreeMap::new();
    let mut installations = BTreeMap::new();
    for provider in &ctx.providers {
        let snap = match provider.snapshot() {
            Ok(s) => s,
            Err(e) => broken(provider.id(), &e),
        };
        // Only for a provider that is here: an absent or broken one has no
        // installation to be found twice, and says nothing rather than "none".
        if matches!(snap.detection.presence, ProviderPresence::Present) {
            let found = provider.installations();
            if !found.is_not_recorded() {
                installations.insert(provider.id().clone(), found);
            }
        }
        providers.insert(provider.id().clone(), snap);
    }
    redact_installations(&mut installations, &ctx.home);
    environment.installations = installations;
    // Resolutions: for every installed name, ask PATH which file would run
    // and every provider who owns it. Keyed by executable name.
    let mut names: Vec<String> = providers
        .values()
        .flat_map(|p| p.inventory.installed_names())
        .collect();
    names.sort();
    names.dedup();
    for name in names {
        let Some(path) = ctx.machine.which(&name) else {
            continue;
        };
        let mut best = None;
        for provider in &ctx.providers {
            if let Ok(r) = provider.resolution(&path) {
                let is_owner = matches!(
                    r.owner,
                    macportsbrew_domain::ExecutableOwner::Provider { .. }
                );
                if is_owner || best.is_none() {
                    let owner_id = provider.id().clone();
                    best = Some((owner_id, r));
                    if is_owner {
                        break;
                    }
                }
            }
        }
        if let Some((owner_id, mut r)) = best {
            r.executable.clone_from(&name);
            if let Some(p) = providers.get_mut(&owner_id) {
                p.resolutions.insert(name, r);
            }
        }
    }
    // Redaction happens here, at the boundary, and not inside each adapter:
    // this is the one place every provider-derived path passes through, and a
    // snapshot is exactly the file `06-cli-and-ux.md` ("Privacy and
    // diagnostics") says must carry no home path and no user name. Doing it
    // last means the real paths were still available above, where `resolution`
    // needed to canonicalise them and compare them with a prefix.
    for snap in providers.values_mut() {
        redact_provider(snap, &ctx.home);
    }
    let snapshot = Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: TOOL_VERSION.to_string(),
        policy_version: ctx.policy_version.clone(),
        provenance: Provenance {
            captured_at: ctx.captured_at.clone(),
            captured_by: ctx.captured_by.clone(),
            origin: "captured".into(),
            note: None,
        },
        machine,
        environment,
        providers,
    };
    // Last, and here rather than in the caller. "Constructed" does not mean
    // safe: the leading `PATH` is `<prefix>/bin` and the prefix is whatever the
    // provider - or a recording - answered, so a capture can carry the same
    // leak a file can state outright (F-21). Running it inside `capture` means
    // no route produces an unchecked snapshot.
    //
    // And `InputUnusable` rather than the identifier `check` builds, because
    // this is the *capture* route: a live machine under a live runner, a
    // recording under `--replay`. Neither is a snapshot file, and the snapshot
    // remediation told both of them to take a fresh snapshot and re-aim a flag
    // they had not used - F-41's defect, alive here as well as at
    // `planner::command`, and reproduced by answering `brew --prefix` with the
    // other provider's prefix in a recording. `load_snapshot` keeps
    // `SnapshotUnusable`: there the input really is that file.
    //
    // The route *is* knowable here - `ctx.home` is `Recorded` exactly under
    // replay - and is deliberately not branched on. A neutral identifier is
    // preferred to a branch: reading `HomeRedaction` as a route proxy couples a
    // remediation to redaction policy, so a later change to how homes are
    // redacted would silently re-aim an error message. If the route is wanted
    // here it should arrive as its own field.
    //
    // Only `SnapshotUnusable` is rewritten. `check` returns nothing else today,
    // and a blanket rewrite would restamp whatever it returns tomorrow - a
    // `SafetyBlocked` or a `PolicyDataInvalid` silently relabelled "input
    // unusable" is the invisible wrongness this repository calls its one
    // unrecoverable error.
    snapshot.check(&ctx.environment_contracts).map_err(|e| {
        if e.id == ErrorId::SnapshotUnusable {
            DomainError::new(ErrorId::InputUnusable, e.detail)
        } else {
            e
        }
    })?;
    Ok(snapshot)
}

/// Replaces the home directory with `~` in every path-valued field of one
/// provider's snapshot.
///
/// The fields are enumerated rather than filtered by name because a provider
/// decides what its own `environment` map is called: Homebrew's `--cache` is a
/// home path on every machine, and `HOME` itself is in the sanitised
/// invocation environment INV-10 builds. Anything holding a path is listed
/// here; a new path-valued field is a line in this function.
fn redact_provider(snap: &mut ProviderSnapshot, home: &HomeRedaction) {
    for value in snap.environment.values_mut() {
        *value = redact_home(value, home);
    }
    // A capability's `reason` is free text, and free text an adapter writes
    // about its own installation quotes paths: the offline-metadata reason
    // names the cache file it looked for, which is under the home on every
    // machine. Free text is exactly where a redaction rule keyed on field
    // names would have missed it.
    for state in snap.capabilities.capabilities.values_mut() {
        if let macportsbrew_domain::CapabilityState::Unavailable { reason } = state {
            *reason = redact_home(reason, home);
        }
    }
    for value in snap.invocation_environment.values_mut() {
        *value = redact_home(value, home);
    }
    // Recorded here because here is where it happens. A consumer that re-derives
    // this from the string catches `~` and misses `/opt/USER/brew`, which is the
    // shape that fails open (F-49). `redact_option_path` answers whether it
    // changed the value, which is the whole question.
    snap.detection.prefix_redacted = redact_option_path(&mut snap.detection.prefix, home);
    redact_option_path(&mut snap.detection.executable, home);
    for package in &mut snap.inventory.installed {
        redact_option_path(&mut package.install_path, home);
    }
    for resolution in snap.resolutions.values_mut() {
        redact_option_path(&mut resolution.path, home);
        redact_option_path(&mut resolution.canonical_path, home);
    }
    // A configuration file under a prefix that is itself under the home - a
    // Homebrew installed in the user's directory (DP-021) - is named by that
    // prefix. A `~/` file is already written without the home.
    if let macportsbrew_domain::ProviderConfiguration::Read { files } = &mut snap.configuration {
        for file in files {
            file.path = redact_home(&file.path, home);
        }
    }
}

/// Replaces the home directory with `~` in every prefix a provider was found
/// at. Today the candidates are system paths from policy data, so nothing is
/// changed; the rule is here because a candidate under the home is a policy
/// edit away, and a snapshot is the file that must carry no user name.
fn redact_installations(
    installations: &mut BTreeMap<ProviderId, macportsbrew_domain::Installations>,
    home: &HomeRedaction,
) {
    for found in installations.values_mut() {
        if let macportsbrew_domain::Installations::Read { prefixes } = found {
            for prefix in prefixes.iter_mut() {
                *prefix = redact_home(prefix, home);
            }
        }
    }
}

/// Redacts in place, and answers whether that changed anything.
///
/// `false` for an absent path as well as for one redaction left alone: the
/// question this answers is "is what is recorded different from what was
/// observed", and for `None` there is nothing recorded either way.
fn redact_option_path(path: &mut Option<PathBuf>, home: &HomeRedaction) -> bool {
    let Some(value) = path else { return false };
    let redacted = PathBuf::from(redact_home(&value.to_string_lossy(), home));
    let changed = redacted != *value;
    *value = redacted;
    changed
}

/// The snapshot recorded for a provider whose read failed.
///
/// Takes the id rather than the reader, because the id is all it uses and the
/// difference is a unit test rather than a `ProviderRead` stub. It had none
/// until 2026-09-18, which is how it stayed the last producer of an empty
/// capability table after both adapters had stopped writing one.
fn broken(provider: &ProviderId, error: &DomainError) -> ProviderSnapshot {
    ProviderSnapshot {
        detection: macportsbrew_domain::ProviderDetection {
            provider: provider.clone(),
            presence: macportsbrew_domain::ProviderPresence::Broken {
                reason: error.id.message_id().to_string(),
            },
            version: None,
            prefix: None,
            executable: None,
            architecture: None,
            prefix_redacted: false,
            // A broken provider has no prefix, so there was nothing to redact.
        },
        // Nine unknowns rather than an empty table, for the reason both
        // adapters now give: a provider this run could not read cannot be
        // asked what it can do. An empty table here was the third producer of
        // a shape the domain's load-door rule is documented as never seeing,
        // and it is reachable on an ordinary machine - a present Homebrew
        // whose JSON this build cannot parse takes this branch - so `analyze`
        // live and `analyze --from-snapshot` of the snapshot written from it
        // would have disagreed, which is INV-15 (security-reviewer,
        // 2026-09-18).
        capabilities: macportsbrew_domain::ProviderCapabilities {
            capabilities: macportsbrew_domain::Capability::all()
                .iter()
                .map(|c| (*c, macportsbrew_domain::CapabilityState::Unknown))
                .collect(),
        },
        inventory: macportsbrew_domain::ProviderInventory::default(),
        environment: BTreeMap::new(),
        invocation_environment: BTreeMap::new(),
        resolutions: BTreeMap::new(),
        configuration: macportsbrew_domain::ProviderConfiguration::NotRecorded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macportsbrew_domain::{
        Capability, CapabilityState, ProviderCapabilities, ProviderDetection, ProviderId,
        ProviderPresence,
    };

    #[test]
    fn a_prefix_under_the_home_is_redacted_in_the_installation_record() {
        let id = ProviderId::new("any-provider");
        let mut found = BTreeMap::from([(
            id.clone(),
            macportsbrew_domain::Installations::Read {
                prefixes: vec!["/Users/testuser/tool".into(), "/usr/local".into()],
            },
        )]);
        redact_installations(
            &mut found,
            &HomeRedaction::Known(std::path::PathBuf::from("/Users/testuser")),
        );
        assert_eq!(
            found[&id],
            macportsbrew_domain::Installations::Read {
                prefixes: vec!["~/tool".into(), "/usr/local".into()]
            }
        );
        // An unknown stays unknown: there is nothing to redact in it.
        let mut unknown =
            BTreeMap::from([(id.clone(), macportsbrew_domain::Installations::Unknowable)]);
        redact_installations(
            &mut unknown,
            &HomeRedaction::Known(std::path::PathBuf::from("/Users/testuser")),
        );
        assert_eq!(unknown[&id], macportsbrew_domain::Installations::Unknowable);
    }

    #[test]
    fn a_provider_whose_read_failed_records_every_capability_as_unknown() {
        // The fix that made this true shipped without a test, which `AGENTS.md`
        // calls incomplete - and this is the one producer the fixture matrix
        // cannot reach, because no recording makes `snapshot()` return `Err`
        // (security-reviewer, 2026-09-18). `macports-broken` does not exercise
        // it: that provider reaches `Broken` through `detect()` and never
        // through here.
        //
        // An empty table here would put the live route and the document route
        // out of step - `analyze` of the machine would show no capabilities
        // and `analyze --from-snapshot` of the snapshot written from it would
        // show nine unknowns, which is INV-15 and is exactly what the fixture
        // matrix caught the first time this rule was written.
        let snap = broken(
            &ProviderId::new("homebrew"),
            &DomainError::new(
                macportsbrew_domain::ErrorId::ProviderOutputChanged,
                "the JSON this build reads is not the JSON it got",
            ),
        );
        assert_eq!(
            snap.capabilities.capabilities.len(),
            Capability::all().len(),
            "every capability, not an empty table"
        );
        assert!(
            snap.capabilities
                .capabilities
                .values()
                .all(|s| *s == CapabilityState::Unknown),
            "and every one of them unknown: a provider this run could not read \
             cannot be asked what it can do"
        );
    }

    #[test]
    fn a_capability_reason_is_redacted_like_any_other_free_text() {
        // `redact_provider`'s own comment names this case: the offline-metadata
        // reason quotes the cache file it looked for, which is under the home on
        // every machine, and a redaction rule keyed on field names would miss
        // it. It was covered only through a live-filesystem read that INV-15
        // forbids under replay; guarding that read took the coverage with it
        // (F-19), so the rule is asserted here directly.
        let home = HomeRedaction::Known("/Users/someone".into());
        let mut snap = ProviderSnapshot {
            detection: ProviderDetection {
                provider: ProviderId::new("homebrew"),
                presence: ProviderPresence::Present,
                version: None,
                prefix: None,
                executable: None,
                architecture: None,
                prefix_redacted: false,
            },
            capabilities: ProviderCapabilities {
                capabilities: [(
                    Capability::OfflineMetadata,
                    CapabilityState::Unavailable {
                        reason: "no cached index at /Users/someone/Library/Caches/Homebrew/api"
                            .into(),
                    },
                )]
                .into_iter()
                .collect(),
            },
            inventory: macportsbrew_domain::ProviderInventory::default(),
            environment: BTreeMap::new(),
            invocation_environment: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            configuration: macportsbrew_domain::ProviderConfiguration::NotRecorded,
        };
        redact_provider(&mut snap, &home);
        let CapabilityState::Unavailable { reason } =
            &snap.capabilities.capabilities[&Capability::OfflineMetadata]
        else {
            panic!("the state is unchanged in kind");
        };
        assert!(
            !reason.contains("/Users/someone"),
            "the home path survived redaction: {reason}"
        );
        assert!(
            reason.contains("~/Library/Caches/Homebrew/api"),
            "and the rest of the path is kept, or the reason stops being useful: {reason}"
        );
    }
}
