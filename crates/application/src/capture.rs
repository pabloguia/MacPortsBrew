// SPDX-License-Identifier: Apache-2.0

//! Capturing a snapshot through the domain's read contracts. The only
//! place the application layer talks to providers; everything after it is
//! a function of the snapshot.

use crate::TOOL_VERSION;
use macportsbrew_domain::ProviderEnvironmentContract;
use macportsbrew_domain::ProviderPresence;
use macportsbrew_domain::{
    is_safe_captured_path, is_safe_provider_name, is_safe_provider_text, placement_holds,
    recorded_index_holds, redact_home, DomainError, EnvironmentSnapshot, ErrorId, ExecutableList,
    ExecutableName, ExecutableOwner, HomeRedaction, ListReason, MachineRead, PathSearch, Placement,
    Provenance, ProviderId, ProviderRead, ProviderSnapshot, Resolution, ResolutionState, Snapshot,
    MAX_EXECUTABLES, SNAPSHOT_SCHEMA_VERSION,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
    /// The packages whose executable lists to read, by name, as the command
    /// named them (`docs/passes/16-plans/F.md`, Design §3 (B)). A name only
    /// selects installed rows - in every provider that holds one - and the read
    /// is made for the row's own identity; a name no inventory holds selects
    /// nothing and sends nothing. Empty for every command that names none,
    /// which keeps those captures exactly as they were.
    pub executables_for: BTreeSet<String>,
}

/// One package's list and placement, before they are filed on its row.
type ReadList = (ProviderId, String, ExecutableList, Placement);

/// Calls [`ProviderRead::executables`] for every installed row whose name was
/// selected, once per provider and name. An `Err` is the adapter's own defect
/// or a provider that vanished, and aborts the capture (F.md §2, SR-8); a read
/// that answered badly is that package's `UNKNOWN`, inside the `Ok`.
fn read_executables(
    ctx: &CaptureContext<'_>,
    providers: &BTreeMap<ProviderId, ProviderSnapshot>,
) -> Result<Vec<ReadList>, DomainError> {
    let mut lists = Vec::new();
    let selected = executables_selection(providers, &ctx.executables_for);
    for provider in &ctx.providers {
        let Some(snap) = providers.get(provider.id()) else {
            continue;
        };
        let mut seen = BTreeSet::new();
        for row in &snap.inventory.installed {
            let name = &row.identity.name;
            if !selected.contains(&(provider.id().clone(), name.clone()))
                || !seen.insert(name.clone())
            {
                continue;
            }
            let (list, placement) = provider.executables(&row.identity)?;
            lists.push((provider.id().clone(), name.clone(), list, placement));
        }
    }
    Ok(lists)
}

/// The rows a capture reads the executable list of, for `names`: per
/// provider that is present, each installed row of a named package that has a
/// metadata entry to file the list on (a row without one could not keep it, so
/// no read is sent for it; every installed row any adapter writes has one).
///
/// Public because the `snapshot` command reports what it did with each name
/// from this same selection, so the report cannot drift from the reads
/// (security review of F-2b, L-2).
#[must_use]
pub fn executables_selection(
    providers: &BTreeMap<ProviderId, ProviderSnapshot>,
    names: &BTreeSet<String>,
) -> BTreeSet<(ProviderId, String)> {
    let mut selected = BTreeSet::new();
    if names.is_empty() {
        return selected;
    }
    for (id, snap) in providers {
        if !matches!(snap.detection.presence, ProviderPresence::Present) {
            continue;
        }
        for row in &snap.inventory.installed {
            let name = &row.identity.name;
            if names.contains(name) && snap.inventory.metadata.contains_key(name) {
                selected.insert((id.clone(), name.clone()));
            }
        }
    }
    selected
}

/// What `snapshot <package>...` tells the user about a name the capture did
/// not read the list of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKey {
    /// No provider this run could read has it installed.
    PackageNotInstalled,
    /// A provider lists it as installed, and no read was made for it: the
    /// provider is not present, or the row has no metadata to file a list on.
    PackageNotRead,
}

impl NoticeKey {
    /// The catalogue key the command renders.
    #[must_use]
    pub fn message_id(self) -> &'static str {
        match self {
            NoticeKey::PackageNotInstalled => "snapshot.package_not_installed",
            NoticeKey::PackageNotRead => "snapshot.package_not_read",
        }
    }
}

/// The notice for every name in `names` that [`executables_selection`] did
/// not select, in name order; a selected name gets none. Pure, beside the
/// selection it is decided from, so what the user is told cannot drift from
/// what was read (security review of F-2b, L-2).
#[must_use]
pub fn executables_notices(
    providers: &BTreeMap<ProviderId, ProviderSnapshot>,
    names: &BTreeSet<String>,
) -> Vec<(String, NoticeKey)> {
    let selected = executables_selection(providers, names);
    names
        .iter()
        .filter(|name| !selected.iter().any(|(_, read)| read == *name))
        .map(|name| {
            let listed = providers.values().any(|p| {
                p.inventory
                    .installed
                    .iter()
                    .any(|r| &r.identity.name == name)
            });
            let key = if listed {
                NoticeKey::PackageNotRead
            } else {
                NoticeKey::PackageNotInstalled
            };
            (name.clone(), key)
        })
        .collect()
}

/// Whether a hit at `path`, answered `r`, is one the snapshot's load rule
/// accepts once redacted: the path a captured path (absolute or `~/`-rooted,
/// bounded, no `..`, no control character), a canonical path printable, and an
/// owner package a provider name. A hit that fails is recorded in neither map
/// (security review of F-2b, M-1), so a machine's own `PATH` shape - an entry
/// such as `/opt/x/libexec/../bin` - can never make its capture refuse itself.
fn recordable(r: &Resolution, path: &Path, home: &HomeRedaction) -> bool {
    let shown = redact_home(&path.to_string_lossy(), home);
    let canonical = r
        .canonical_path
        .as_deref()
        .is_none_or(|c| is_safe_provider_text(&redact_home(&c.to_string_lossy(), home)));
    let owner = match &r.owner {
        ExecutableOwner::Provider {
            package: Some(package),
            ..
        } => is_safe_provider_name(package),
        _ => true,
    };
    is_safe_captured_path(&shown) && canonical && owner
}

/// The best answer any provider gives about `path`, by the rule capture has
/// always used: the first provider, in the registry's order, that names a
/// provider owner; else the first that answers at all.
fn owner_of(ctx: &CaptureContext<'_>, path: &Path) -> Option<(ProviderId, Resolution)> {
    let mut best = None;
    for provider in &ctx.providers {
        if let Ok(r) = provider.resolution(path) {
            let is_owner = matches!(r.owner, ExecutableOwner::Provider { .. });
            if is_owner || best.is_none() {
                best = Some((provider.id().clone(), r));
                if is_owner {
                    break;
                }
            }
        }
    }
    best
}

/// An adapter's [`Placement::OnPathDir`], checked against the capture's own
/// search of `PATH` (F.md SR L-1). The placement says each executable lands
/// in `dir`; where `dir` is itself a `PATH` entry, at index `k`, a shell
/// searching `PATH` reaches `dir/e` no later than `k`. So the search must have
/// found `e` at an index of at most `k`. Found nowhere, or found only further
/// along, contradicts the placement, which becomes
/// `UNKNOWN { CONTRADICTED_BY_RESOLUTION }` - an inference (an adapter's
/// "this package's linked names are linked") is never trusted over an
/// observation. A hit with no index, a name not observed, and a `dir` not on
/// `PATH` settle nothing and leave the placement as the adapter stated it.
///
/// **A contradiction check, not a confirmation** (security review of F-2b,
/// L-4): a hit at an earlier entry is consistent with the placement and says
/// nothing about whether `dir/e` exists. A surviving `ON_PATH_DIR` is the
/// adapter's statement, not observed fact, and F-3 must read it so.
///
/// `environment.path` is already redacted here and `dir` not yet, so `dir` is
/// redacted the same way before the two are compared; positions are the same
/// in both copies.
fn confirmed_by_resolution(
    placement: Placement,
    environment: &EnvironmentSnapshot,
    home: &HomeRedaction,
) -> Placement {
    let Placement::OnPathDir { dirs } = &placement else {
        return placement;
    };
    for (executable, dir) in dirs {
        let redacted = redact_home(dir, home);
        let Some(entry) = environment.path.iter().position(|e| {
            Path::new(e)
                .components()
                .eq(Path::new(&redacted).components())
        }) else {
            continue;
        };
        let contradicted = match environment.resolutions.get(executable) {
            Some(ResolutionState::NotOnPath) => true,
            Some(ResolutionState::Found {
                path_index: Some(index),
                ..
            }) => *index as usize > entry,
            Some(
                ResolutionState::Found {
                    path_index: None, ..
                }
                | ResolutionState::NotObserved,
            )
            | None => false,
        };
        if contradicted {
            return Placement::unknown(ListReason::ContradictedByResolution);
        }
    }
    placement
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
    // The executable lists, for the packages the command named only (F.md
    // §3 (B): a listing costs a provider call per package). The user's string
    // only selects a row; the read is made for the row's own identity, so the
    // argv carries the name the inventory recorded (SR-13).
    let lists = read_executables(ctx, &providers)?;
    // Resolutions: for every installed name and every name a known list
    // holds, ask PATH which file would run and every provider who owns it.
    let package_names: BTreeSet<String> = providers
        .values()
        .flat_map(|p| p.inventory.installed_names())
        .collect();
    let mut names = package_names.clone();
    // Bounded (security review of F-2b, L-3): each hit can ask a provider who
    // owns it, which for some providers is a subprocess. Listed names beyond
    // the budget are not searched and stay NOT_OBSERVED - unknown, never
    // NOT_ON_PATH - in list order, so the same capture keeps the same names.
    let mut budget = MAX_EXECUTABLES;
    for (_, _, list, _) in &lists {
        if let Some(known) = list.known() {
            for listed in known {
                if names.contains(listed.as_str()) {
                    continue;
                }
                if budget == 0 {
                    break;
                }
                budget -= 1;
                names.insert(listed.as_str().to_string());
            }
        }
    }
    for name in names {
        // The name class first (SR2-N5): a name outside it is never searched
        // and stays NOT_OBSERVED - never NOT_ON_PATH.
        let Ok(executable) = ExecutableName::parse(&name) else {
            continue;
        };
        let (path, path_index) = match ctx.machine.search_path(&executable) {
            PathSearch::NotSearched => continue,
            PathSearch::NotFound => {
                environment
                    .resolutions
                    .insert(executable, ResolutionState::NotOnPath);
                continue;
            }
            PathSearch::Found { path, path_index } => (path, path_index),
        };
        let Some((owner_id, mut r)) = owner_of(ctx, &path) else {
            // Found, and no provider would say anything about it: nothing
            // is recorded rather than an owner made up (NOT_OBSERVED), the
            // same as the per-provider map, which has never recorded one.
            continue;
        };
        // A live capture never aborts on its own machine's PATH shape
        // (security review of F-2b, M-1): a hit the load rule would refuse - a
        // path through a `..` entry, one over MAX_PATH_LEN, a canonical path
        // with a control character, an owner package that is not a name - is
        // recorded in neither map, so it reads as NOT_OBSERVED and the
        // agreement between the maps stays exact. Judged on what the snapshot
        // will hold: the redacted form.
        if !recordable(&r, &path, &ctx.home) {
            continue;
        }
        r.executable.clone_from(&name);
        // The per-provider map keeps the earlier rule exactly - installed
        // package names only, filed under the provider that answered best -
        // derived from the same search and never read to decide (SG2-Q3).
        if package_names.contains(&name) {
            if let Some(p) = providers.get_mut(&owner_id) {
                p.resolutions.insert(name, r.clone());
            }
        }
        environment.resolutions.insert(
            executable,
            ResolutionState::Found {
                resolution: r,
                path_index,
            },
        );
    }
    // Recorded last, once both the lists and the resolutions are known: an
    // adapter's placement is checked against what PATH actually did (SR L-1).
    for (id, name, list, placement) in lists {
        let placement = confirmed_by_resolution(placement, &environment, &ctx.home);
        if let Some(provider) = providers.get_mut(&id) {
            if let Some(meta) = provider.inventory.metadata.get_mut(&name) {
                meta.set_executables(list, placement);
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
        // A placement the load rule would refuse - built, say, from a prefix
        // with a `.` component - is the adapter's statement in a shape this
        // build cannot keep, so it is recorded as unknown rather than making
        // `snapshot <package>` refuse itself (security review of F-2b, I-4).
        // Checked after redaction, on exactly what the loader will read.
        let prefix = snap.detection.prefix.clone();
        for meta in snap.inventory.metadata.values_mut() {
            if placement_holds(
                &meta.placement,
                meta.executable_list.known(),
                prefix.as_deref(),
            )
            .is_err()
            {
                meta.placement = Placement::unknown(ErrorId::ProviderOutputChanged);
            }
        }
    }
    // The PATH map's paths by the same function as the per-provider map's,
    // so the two copies of one resolution stay byte-equal (SG2-Q3).
    for state in environment.resolutions.values_mut() {
        if let ResolutionState::Found {
            resolution,
            path_index,
        } = state
        {
            redact_option_path(&mut resolution.path, &ctx.home);
            redact_option_path(&mut resolution.canonical_path, &ctx.home);
            // The index was right on the unredacted PATH; two entries the
            // machine kept apart can read alike once redacted (two homes
            // redacted by shape, say), and the load rule checks the redacted
            // copy. Then no index is recorded - unpredictable, the safe
            // reading - rather than one the rule would refuse (security
            // review of F-2b, I-1).
            if let (Some(index), Some(path)) = (*path_index, resolution.path.as_deref()) {
                if !recorded_index_holds(&environment.path, index, path) {
                    *path_index = None;
                }
            }
        }
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
    // A placement names directories under the prefix, which can be under the
    // home (DP-021), and a not-linked reason is the provider's free text. The
    // load rule compares a directory with the prefix, and both are redacted
    // here by the same function, so the comparison still holds.
    for meta in snap.inventory.metadata.values_mut() {
        match &mut meta.placement {
            Placement::OnPathDir { dirs } => {
                for dir in dirs.values_mut() {
                    *dir = redact_home(dir, home);
                }
            }
            Placement::NotLinked { reason } => *reason = redact_home(reason, home),
            Placement::Unknown { .. } => {}
        }
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
