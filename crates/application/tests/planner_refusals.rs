// SPDX-License-Identifier: Apache-2.0

//! What the planner refuses to plan, and why.
//!
//! `planner.rs` had **no unit tests at all** — every line it covered was
//! covered incidentally, by integration tests asking for plans that succeed.
//! The first coverage measurement this repository has ever run put the file at
//! 83.99%, and the uncovered lines were almost entirely its `return Err` arms
//! (DP-015).
//!
//! That is the wrong half to leave untested. `07-quality-and-testing.md` puts
//! planning at a 95% floor for a stated reason — "this is the code that can
//! destroy a machine" — and a planner's refusals *are* its safety behaviour: a
//! plan that should never have been produced is the failure mode, not a plan
//! that is slightly wrong. Each test here names the refusal rather than the
//! line, so it stays meaningful if the code moves.
//!
//! The fixtures are deliberately minimal — no metadata, no availability, no
//! dependency edges — because a refusal happens before any of that is read.
//! The richer two-provider fixture lives in `invariants.rs` and is built for a
//! different question.

use macportsbrew_application::{
    build_plan, explain_package, migration_start, plan_outcome, AnalysisOptions, MigrationStart,
    PlanOutcome, PlanRequest,
};
use macportsbrew_domain::{
    Architecture, Capability, CapabilityState, CommandLineTools, EnvironmentSnapshot, ErrorId,
    InstalledPackage, Machine, MacosVersion, PackageIdentity, PackageKind, PackageMetadata,
    Provenance, ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory,
    ProviderPresence, ProviderSnapshot, RequestedSet, Snapshot, Translation, Version,
    SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const HB: &str = "homebrew";
const MP: &str = "macports";

/// The provider ids a complete input carries, as the command line passes
/// them from the registry.
fn known() -> Vec<ProviderId> {
    vec![ProviderId::new(HB), ProviderId::new(MP)]
}

/// A provider holding exactly the named packages, all requested.
///
/// `(name, homepage)`: a shared homepage is how the mapping engine discovers a
/// counterpart, and therefore how one provider's single name comes to be the
/// counterpart of several of another's — the ambiguity the planner refuses.
fn holding(id: &str, prefix: &str, names: &[(&str, Option<&str>)]) -> ProviderSnapshot {
    let pid = ProviderId::new(id);
    let mut inventory = ProviderInventory::default();
    for (name, homepage) in names {
        inventory.installed.push(InstalledPackage {
            identity: PackageIdentity::new(pid.clone(), *name),
            kind: PackageKind::Package,
            version: Version::new("1.0"),
            revision: None,
            variants: BTreeSet::new(),
            active: true,
            requested: Some(true),
            installed_from_prebuilt: Some(true),
            install_path: None,
        });
        inventory.requested.names.insert((*name).to_string());
        inventory.metadata.insert(
            (*name).to_string(),
            PackageMetadata {
                homepage: homepage.map(str::to_string),
                ..PackageMetadata::default()
            },
        );
    }
    inventory.requested = RequestedSet {
        names: inventory.requested.names.clone(),
        mechanism: "test".into(),
    };
    ProviderSnapshot {
        detection: ProviderDetection {
            provider: pid,
            presence: ProviderPresence::Present,
            version: Some(Version::new("9.9")),
            prefix: Some(PathBuf::from(prefix)),
            executable: None,
            architecture: Some(Architecture::Arm64),
            prefix_redacted: false,
        },
        // The planner reads a missing inventory entry as unread, so a
        // provider these fixtures describe as read has to say so.
        capabilities: ProviderCapabilities {
            capabilities: [(
                Capability::MachineReadableInventory,
                CapabilityState::Available,
            )]
            .into_iter()
            .collect(),
        },
        inventory,
        environment: BTreeMap::new(),
        invocation_environment: [("PATH".to_string(), format!("{prefix}/bin:/usr/bin:/bin"))]
            .into_iter()
            .collect(),
        resolutions: BTreeMap::new(),
        configuration: macportsbrew_domain::ProviderConfiguration::Read { files: Vec::new() },
    }
}

fn snapshot_of(providers: Vec<(&str, ProviderSnapshot)>) -> Snapshot {
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-05T00:00:00Z".into(),
            captured_by: "planner_refusals.rs".into(),
            origin: "synthesised".into(),
            note: None,
        },
        machine: Machine {
            architecture: Architecture::Arm64,
            macos: MacosVersion::new(15, 6, 1),
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Installed {
                path: PathBuf::from("/Library/Developer/CommandLineTools"),
            },
        },
        environment: EnvironmentSnapshot::default(),
        providers: providers
            .into_iter()
            .map(|(id, p)| (ProviderId::new(id), p))
            .collect(),
    }
}

fn both(hb: &[(&str, Option<&str>)], mp: &[(&str, Option<&str>)]) -> Snapshot {
    snapshot_of(vec![
        (HB, holding(HB, "/opt/homebrew", hb)),
        (MP, holding(MP, "/opt/local", mp)),
    ])
}

fn plan_for(snapshot: &Snapshot, package: &str, to: &str) -> Result<(), (ErrorId, String)> {
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    let options = AnalysisOptions::default();
    build_plan(
        snapshot,
        &policy,
        &options,
        &PlanRequest {
            package: package.to_string(),
            to: ProviderId::new(to),
            retain_source: false,
        },
        &known(),
    )
    .map(|_| ())
    .map_err(|e| (e.id, e.detail))
}

#[test]
fn a_package_no_provider_holds_is_refused_by_name() {
    let snap = both(&[("wget", None)], &[("curl", None)]);
    let (id, detail) = plan_for(&snap, "nothing-like-this", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::PackageNotFound);
    assert!(
        detail.contains("nothing-like-this") && detail.contains("not installed"),
        "the refusal must name the package the user asked for: {detail}"
    );
}

#[test]
fn a_package_only_the_destination_holds_is_refused_as_nothing_to_move() {
    // Not an error about the package: the request is coherent and there is
    // simply nothing to do. Producing an empty plan here would be worse — it
    // would read as "this migration is safe" for a migration that is not
    // happening.
    let snap = both(&[], &[("curl", None)]);
    let (id, detail) = plan_for(&snap, "curl", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::Usage);
    assert!(
        detail.contains("already held by") && detail.contains(MP),
        "the refusal must say who already holds it: {detail}"
    );
}

fn outcome_for(snapshot: &Snapshot, package: &str, to: &str) -> Result<PlanOutcome, ErrorId> {
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    plan_outcome(
        snapshot,
        &policy,
        &AnalysisOptions::default(),
        &PlanRequest {
            package: package.to_string(),
            to: ProviderId::new(to),
            retain_source: false,
        },
        &known(),
    )
    .map_err(|e| e.id)
}

/// The same snapshot with `id`'s provider replaced by what `capture` records
/// for one it could not read: `Broken`, an empty inventory, every capability
/// `Unknown`.
fn with_broken(mut snap: Snapshot, id: &str) -> Snapshot {
    let p = snap
        .providers
        .get_mut(&ProviderId::new(id))
        .expect("the provider is in the snapshot");
    p.detection.presence = ProviderPresence::Broken {
        reason: "error.provider_unhealthy".into(),
    };
    p.detection.version = None;
    p.inventory = ProviderInventory::default();
    p.capabilities = ProviderCapabilities {
        capabilities: Capability::all()
            .iter()
            .map(|c| (*c, CapabilityState::Unknown))
            .collect(),
    };
    snap
}

fn already_at(package: &str, provider: &str) -> PlanOutcome {
    PlanOutcome::AlreadyAtDestination {
        package: package.into(),
        provider: ProviderId::new(provider),
    }
}

#[test]
fn invariant_16_a_package_only_its_destination_holds_is_an_outcome_not_an_error() {
    // INV-16: a re-run after a migration that removed its source finds the
    // package at its destination and nowhere else. That is "nothing
    // further", which a caller has to be able to tell from a refused request
    // without reading a message.
    let snap = both(&[], &[("curl", None)]);
    assert_eq!(outcome_for(&snap, "curl", MP), Ok(already_at("curl", MP)));
}

#[test]
fn invariant_17_the_mirror_direction_is_already_at_its_destination_too() {
    // The mirror of the INV-16 case: the direction is not special.
    let snap = both(&[("wget", None)], &[]);
    assert_eq!(outcome_for(&snap, "wget", HB), Ok(already_at("wget", HB)));
}

#[test]
fn an_absent_other_provider_does_not_stop_it_being_already_there() {
    // Genuinely absent is a fact about the other side, unlike broken.
    let mut snap = both(&[], &[("curl", None)]);
    let hb = snap
        .providers
        .get_mut(&ProviderId::new(HB))
        .expect("present");
    hb.detection.presence = ProviderPresence::Absent;
    // `capture` records an absent provider's capabilities as unknown; that
    // must not be read as an unreadable inventory.
    hb.capabilities = ProviderCapabilities {
        capabilities: Capability::all()
            .iter()
            .map(|c| (*c, CapabilityState::Unknown))
            .collect(),
    };
    assert_eq!(outcome_for(&snap, "curl", MP), Ok(already_at("curl", MP)));
}

#[test]
fn a_broken_other_provider_is_not_evidence_the_package_was_moved() {
    // A broken provider's inventory is empty because it could not be read.
    // Reading that as "holds nothing" would report a package it still holds
    // as already moved.
    let snap = with_broken(both(&[("curl", None)], &[("curl", None)]), HB);
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    let err = plan_outcome(
        &snap,
        &policy,
        &AnalysisOptions::default(),
        &PlanRequest {
            package: "curl".into(),
            to: ProviderId::new(MP),
            retain_source: false,
        },
        &known(),
    )
    .expect_err("must refuse");
    assert_eq!(err.id, ErrorId::ProviderUnhealthy);
    assert!(
        err.detail.contains(HB),
        "the refusal must name it: {}",
        err.detail
    );
    // `plan` gives the same refusal: before this, it said "already held".
    assert_eq!(
        plan_for(&snap, "curl", MP).map_err(|(id, _)| id),
        Err(ErrorId::ProviderUnhealthy)
    );
    assert_eq!(ErrorId::ProviderUnhealthy.exit_code(), 5);
    // The reason is a message identifier for the UI to render, not English.
    assert!(!err.detail.contains("error."), "{}", err.detail);
}

#[test]
fn an_other_provider_whose_inventory_cannot_be_read_is_not_evidence_either() {
    let mut snap = both(&[], &[("curl", None)]);
    snap.providers
        .get_mut(&ProviderId::new(HB))
        .expect("present")
        .capabilities = ProviderCapabilities {
        capabilities: [(
            Capability::MachineReadableInventory,
            CapabilityState::Unknown,
        )]
        .into_iter()
        .collect(),
    };
    assert_eq!(
        outcome_for(&snap, "curl", MP),
        Err(ErrorId::SafetyUnverifiable)
    );
    // `plan` gives the same refusal, exit 4: before this, "already held", exit 2.
    assert_eq!(
        plan_for(&snap, "curl", MP).map_err(|(id, _)| id.exit_code()),
        Err(4)
    );
}

#[test]
fn an_other_provider_with_no_inventory_entry_is_not_evidence_either() {
    // A capability table that does not mention the inventory has not said it
    // was read; an absent entry is never a yes.
    let mut snap = both(&[], &[("curl", None)]);
    snap.providers
        .get_mut(&ProviderId::new(HB))
        .expect("present")
        .capabilities = ProviderCapabilities::default();
    assert_eq!(
        outcome_for(&snap, "curl", MP),
        Err(ErrorId::SafetyUnverifiable)
    );
    // `plan` gives the same refusal, exit 4: before this, "already held", exit 2.
    assert_eq!(
        plan_for(&snap, "curl", MP).map_err(|(id, _)| id.exit_code()),
        Err(4)
    );
}

#[test]
fn a_known_provider_missing_from_the_input_is_not_evidence_either() {
    // A document with a provider's entry deleted - not recorded as absent,
    // simply not there - has not said that provider holds nothing.
    let mut snap = both(&[], &[("curl", None)]);
    snap.providers.remove(&ProviderId::new(HB));
    let err = plan_for(&snap, "curl", MP).expect_err("must refuse");
    assert_eq!(err.0, ErrorId::SafetyUnverifiable);
    assert_eq!(err.0.exit_code(), 4);
    assert!(err.1.contains(HB), "the refusal must name it: {}", err.1);
    assert_eq!(
        outcome_for(&snap, "curl", MP),
        Err(ErrorId::SafetyUnverifiable)
    );
}

#[test]
fn only_the_other_providers_inventories_are_questioned() {
    // The rule is about whether anything *else* still holds the package; the
    // destination's own row is what was read to find it there.
    let mut snap = both(&[], &[("curl", None)]);
    snap.providers
        .get_mut(&ProviderId::new(MP))
        .expect("present")
        .capabilities = ProviderCapabilities {
        capabilities: [(
            Capability::MachineReadableInventory,
            CapabilityState::Unknown,
        )]
        .into_iter()
        .collect(),
    };
    assert_eq!(outcome_for(&snap, "curl", MP), Ok(already_at("curl", MP)));
}

#[test]
fn an_inactive_copy_at_the_destination_is_not_already_there() {
    // An inactive copy is on disk but is not what runs.
    let mut snap = both(&[], &[("curl", None)]);
    for p in &mut snap
        .providers
        .get_mut(&ProviderId::new(MP))
        .expect("present")
        .inventory
        .installed
    {
        p.active = false;
    }
    assert_eq!(outcome_for(&snap, "curl", MP), Err(ErrorId::Usage));
    let (id, detail) = plan_for(&snap, "curl", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::Usage);
    assert_eq!(detail, format!("curl is already held by {MP}"));
}

#[test]
fn a_package_held_by_nobody_is_not_already_at_its_destination() {
    // Absent everywhere is not "already moved": reading it that way would
    // report a migration as done that never happened.
    let snap = both(&[("wget", None)], &[("curl", None)]);
    assert_eq!(
        outcome_for(&snap, "nothing-like-this", MP),
        Err(ErrorId::PackageNotFound)
    );
    // And a package held elsewhere is a plan, not an outcome.
    assert!(
        matches!(outcome_for(&snap, "wget", MP), Ok(PlanOutcome::Plan(_))),
        "a package the destination does not hold must be planned"
    );
}

#[test]
fn build_plan_still_reports_it_as_usage() {
    // `plan`'s exit 2 and its message are a contract; the typed outcome is an
    // addition beside it, not a change to it.
    let snap = both(&[], &[("curl", None)]);
    let (id, detail) = plan_for(&snap, "curl", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::Usage);
    assert_eq!(detail, format!("curl is already held by {MP}"));
}

#[test]
fn a_destination_the_snapshot_does_not_know_is_refused() {
    // A snapshot carrying one provider, asked to plan toward another. The
    // guard exists because a missing destination would otherwise be read as a
    // destination holding nothing, which is exactly the shape of a valid
    // migration.
    let snap = snapshot_of(vec![(HB, holding(HB, "/opt/homebrew", &[("wget", None)]))]);
    let (id, detail) = plan_for(&snap, "wget", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::ProviderNotFound);
    assert!(
        detail.contains(MP),
        "the refusal must name the destination it does not know: {detail}"
    );
}

#[test]
fn a_name_that_is_the_counterpart_of_several_packages_is_refused_with_the_choices() {
    // The regression this arm was written for: one provider's single name can
    // be another's several. Taking the first was silent and produced a plan
    // for a *different package* than the one named. Ambiguity never acts
    // automatically — it reports the choices, like every other ambiguous match
    // in this project.
    //
    // Built the way the analysis produces it: two Homebrew units whose
    // MacPorts counterpart row carries the same name.
    let snap = both(
        &[
            ("python@3.13", Some("https://www.python.org")),
            ("python@3.14", Some("https://www.python.org")),
        ],
        &[("python312", Some("https://www.python.org"))],
    );
    let outcome = plan_for(&snap, "python312", HB);
    let Err((id, detail)) = outcome else {
        // If the curated data stops mapping these two onto one name, this test
        // is no longer exercising ambiguity and must be rebuilt rather than
        // quietly passing.
        panic!(
            "expected an ambiguous match; the fixture no longer produces one, \
             so this test asserts nothing"
        );
    };
    assert_eq!(id, ErrorId::Usage);
    assert!(
        detail.contains("more than one package") && detail.contains("python312"),
        "the refusal must name the ambiguous spelling and the choices: {detail}"
    );
}

// -- explain ----------------------------------------------------------------

#[test]
fn explaining_a_package_no_provider_holds_is_refused_by_name() {
    // `explain` is the command a user reaches for when the answer surprises
    // them, so its own failure has to be legible: the name they typed, and
    // that nothing holds it — not an empty explanation, which would read as
    // "there is nothing to say about this package".
    let snap = both(&[("wget", None)], &[("curl", None)]);
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    let err = explain_package(
        &snap,
        &policy,
        &AnalysisOptions::default(),
        "nothing-like-this",
    )
    .expect_err("must refuse");
    assert_eq!(err.id, ErrorId::PackageNotFound);
    assert!(
        err.detail.contains("nothing-like-this"),
        "the refusal must name the package the user asked for: {}",
        err.detail
    );
}

#[test]
fn explaining_a_package_by_the_other_providers_name_finds_it() {
    // The `or` half of the same lookup: a unit is findable by its own name or
    // by what a provider calls its row. Without this the test above passes on
    // an `explain` that finds nothing at all.
    let snap = both(&[("wget", None)], &[]);
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    explain_package(&snap, &policy, &AnalysisOptions::default(), "wget")
        .expect("a held package must be explainable");
}

/// F-48: policy load must not accept a curated name the planner will refuse.
///
/// `check_curated_names` in `PolicyData::load_from` applies
/// `is_safe_provider_name`, which is the *last* of the three gates
/// `planner::command` runs on a package name. The first is the provider's
/// reserved-word list from `provider-commands.toml` — thirteen words for
/// MacPorts, `all`, `installed`, `active`, `leaves` among them. A curated row
/// whose destination is one of those loads, plans, and fails at the planner with
/// `ErrorId::Usage`: *"That command line is not valid … Run macportsbrew
/// --help"*. The user's command line was valid and `--help` cannot help them,
/// which is a worse misattribution than the one F-48 was filed about.
///
/// The two lists do not meet in either layer. `policy` *could* read
/// `provider-commands.toml` — it is in `data/policy/` beside the six files
/// `policy` already embeds — but the parse lives here, in
/// `provider_commands.rs`, together with the absoluteness check on `executable`,
/// so a copy in `policy` would be two spellings of one file. `build_plan` does
/// see both — it takes `&PolicyData` and calls `provider_commands::load()` — so
/// the containment could be checked at runtime; it is not, because a per-request
/// path should validate the request rather than rescan a compiled-in table on
/// every plan. The whole-table property belongs here. `security-reviewer` found
/// the gap while reviewing the fix for the other half.
///
/// This asserts containment, not the absence of a check: policy accepting a name
/// the planner rejects is the defect, and either list moving makes it fail.
///
/// **The reserved half has teeth on one side only, today.** Exactly one provider
/// declares `reserved_names` — MacPorts, thirteen words — and every shipped
/// curated row runs homebrew to macports, so the destination names are compared
/// against a real list and the source names against an empty one. That is the
/// half that matters (a destination name is what reaches `command()` as
/// `dest_name`), and SE-051 already records the asymmetry as open research
/// rather than a decision. It is said here because a reader would otherwise take
/// "asserted where both are visible" as covering both directions, and the
/// counted assertion below fails if the last real list is ever emptied.
#[test]
fn every_curated_name_is_one_the_planner_would_accept() {
    let data = macportsbrew_policy::PolicyData::load_embedded().expect("embedded policy data");
    let commands = macportsbrew_application::provider_commands::load().expect("provider commands");
    let rows = data.curated_mappings();
    assert!(!rows.is_empty(), "an empty table would assert nothing");
    let mut compared_against_a_real_list = 0usize;

    for row in rows {
        for (provider, name) in [
            (&row.source_provider, &row.source_name),
            (&row.destination_provider, &row.destination_name),
        ] {
            assert!(
                macportsbrew_domain::is_safe_provider_name(name),
                "curated {provider} name {name:?} is not one a provider could report"
            );
            assert!(
                !name.is_empty(),
                "curated {provider} name is empty, which the planner calls an internal error"
            );
            let entry = commands.get(provider.as_str()).unwrap_or_else(|| {
                panic!(
                    "curated row names provider {provider:?}, which provider-commands.toml \
                     does not describe - so the reserved-word half of this test would \
                     silently check nothing"
                )
            });
            {
                assert!(
                    !entry
                        .reserved_names
                        .iter()
                        .any(|reserved| reserved.eq_ignore_ascii_case(name)),
                    "curated {provider} name {name:?} is a word that provider reads as a set \
                     of ports; the planner refuses it as a usage error, blaming a command line \
                     that was correct (F-48)"
                );
                if !entry.reserved_names.is_empty() {
                    compared_against_a_real_list += 1;
                }
            }
        }
    }

    // Without this the reserved half can go quiet without failing: empty the one
    // provider list that has words in it, or add rows in a direction no provider
    // describes, and every assertion above still passes while checking nothing.
    assert!(
        compared_against_a_real_list > 0,
        "no curated name was compared against a non-empty reserved-word list, so \
         this test's reserved half asserted nothing"
    );
}

/// DP-016(c): a plan is refused when the source cannot say whether its packages
/// define services, because INV-19's protection would be silently absent.
///
/// `classify` reads `metadata.defines_service`, a bool the source provider
/// fills. Where the adapter could not look it reports
/// `Capability::ServiceMetadata: Unknown` and that bool is `false`, so
/// `PackageClass::Service` is never added and the rule that forces
/// `MANUAL_REVIEW` never fires — on a plan that looks entirely ordinary. Under
/// `--replay` that is every MacPorts package (F-25, F-29).
///
/// The owner ruled (c) over (a): refuse the plan rather than treat `Unknown` as
/// a service, because (a) would make every replayed MacPorts plan a
/// `MANUAL_REVIEW` and cost `--replay` the thing it exists to demonstrate.
///
/// Four cases, because the refusal has to be *narrow* to be worth having: the
/// two that decide it, the one that proves the question is asked of the source,
/// and the one that keeps "never asked" from meaning "could not look".
#[test]
fn invariant_19_a_source_that_cannot_answer_the_service_question_refuses_the_plan() {
    let with = |snap: &mut Snapshot, provider: &str, state: CapabilityState| {
        snap.providers
            .get_mut(&ProviderId::new(provider))
            .expect("provider")
            .capabilities
            .capabilities
            .insert(Capability::ServiceMetadata, state);
    };

    // Unknown on the source: refused, and the refusal names what is missing.
    let mut snap = both(&[("wget", None)], &[]);
    with(&mut snap, HB, CapabilityState::Unknown);
    let (id, detail) = plan_for(&snap, "wget", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::SafetyUnverifiable);
    assert!(
        detail.contains("service") && detail.contains("wget"),
        "the refusal must say which fact is missing and for what: {detail}"
    );

    // Available on the source: planning is untouched. Without this the test
    // would pass just as well if the refusal were unconditional.
    let mut snap = both(&[("wget", None)], &[]);
    with(&mut snap, HB, CapabilityState::Available);
    assert!(
        plan_for(&snap, "wget", MP).is_ok(),
        "a source that can answer still plans"
    );

    // Unknown on the *destination* only. The question is about the package being
    // moved, which the source classifies; a destination that cannot answer says
    // nothing about it.
    let mut snap = both(&[("wget", None)], &[]);
    with(&mut snap, HB, CapabilityState::Available);
    with(&mut snap, MP, CapabilityState::Unknown);
    assert!(
        plan_for(&snap, "wget", MP).is_ok(),
        "the destination's ignorance is not about the package being removed"
    );

    // Unavailable on the source: refused too, and this arm was missing until
    // `security-reviewer` reproduced the gap on 2026-09-18. The predicate
    // matched only `Unknown`, so a document saying `UNAVAILABLE` planned
    // normally - exit 0, a full plan with a `REMOVE_SOURCE` step - while the
    // two shapes this pass had just closed refused.
    //
    // `Unavailable` is not an affirmative answer to "can this provider tell
    // you whether a package defines a service". A provider that does not offer
    // service metadata cannot tell you, exactly as one whose scan failed
    // cannot. Only `Available` is.
    let mut snap = both(&[("wget", None)], &[]);
    with(
        &mut snap,
        HB,
        CapabilityState::Unavailable {
            reason: "provider_exposes_no_service_metadata".into(),
        },
    );
    let (id, _) = plan_for(&snap, "wget", MP)
        .expect_err("a source that reports the capability unavailable must refuse too");
    assert_eq!(id, ErrorId::SafetyUnverifiable);

    // Absent entirely, which is what every other test in this file carries.
    // "Never reported" is not "could not be detected", and reading it as the
    // latter would refuse every plan built from a hand-made snapshot.
    let snap = both(&[("wget", None)], &[]);
    assert!(
        plan_for(&snap, "wget", MP).is_ok(),
        "a capability nobody reported was never asked about, not undetectable"
    );
}

/// `migration_start` for `package` toward `to`, with every other argument as
/// `outcome_for` passes it.
fn start_for(
    snapshot: &Snapshot,
    package: &str,
    to: &str,
) -> Result<MigrationStart, (ErrorId, String)> {
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    migration_start(
        snapshot,
        &policy,
        &AnalysisOptions::default(),
        &PlanRequest {
            package: package.to_string(),
            to: ProviderId::new(to),
            retain_source: false,
        },
        &known(),
    )
    .map_err(|e| (e.id, e.detail))
}

#[test]
fn invariant_16_migrate_reports_nothing_to_move_when_it_can_be_shown() {
    // Both providers present and read, the destination the only holder, and
    // nothing elsewhere that could be the same package: nothing to move.
    let snap = both(&[("wget", None)], &[("curl", None)]);
    match start_for(&snap, "curl", MP) {
        Ok(MigrationStart::NothingToMove {
            package,
            provider,
            not_detected,
        }) => {
            assert_eq!(package, "curl");
            assert_eq!(provider, ProviderId::new(MP));
            assert!(not_detected.is_empty(), "{not_detected:?}");
        }
        other => panic!("expected nothing to move, got {other:?}"),
    }
}

#[test]
fn migrate_gets_a_plan_paired_with_the_snapshot_it_was_built_from() {
    let snap = both(&[("wget", None)], &[("curl", None)]);
    match start_for(&snap, "wget", MP) {
        Ok(MigrationStart::Run(run)) => {
            assert_eq!(run.plan().package.name, "wget");
            assert_eq!(run.plan().destination, ProviderId::new(MP));
        }
        other => panic!("expected a plan, got {other:?}"),
    }
}

#[test]
fn invariant_16_migrate_reports_nothing_to_move_naming_a_provider_that_was_not_detected() {
    // DP-026 (b): an absent provider is not proof it holds nothing, because
    // detection only looks where a provider is documented to live - so the
    // answer stands and names it, rather than refusing a re-run that changes
    // nothing (INV-16).
    let mut snap = both(&[], &[("curl", None)]);
    let hb = snap
        .providers
        .get_mut(&ProviderId::new(HB))
        .expect("present");
    hb.detection.presence = ProviderPresence::Absent;
    hb.capabilities = ProviderCapabilities {
        capabilities: Capability::all()
            .iter()
            .map(|c| (*c, CapabilityState::Unknown))
            .collect(),
    };
    assert_eq!(outcome_for(&snap, "curl", MP), Ok(already_at("curl", MP)));
    match start_for(&snap, "curl", MP) {
        Ok(MigrationStart::NothingToMove { not_detected, .. }) => {
            assert_eq!(not_detected, vec![ProviderId::new(HB)]);
        }
        other => panic!("expected nothing to move, got {other:?}"),
    }
}

#[test]
fn migrate_never_reports_nothing_to_move_while_another_name_may_be_the_package() {
    // One formula and two ports share a homepage: the formula's mapping
    // toward the ports is ambiguous, so it selects neither and sits in a unit
    // of its own, and `plan_outcome` sees the port alone at its destination.
    // The formula may be that port under another name.
    let home = Some("https://nodejs.org");
    let snap = both(&[("node", home)], &[("nodejs22", home), ("nodejs24", home)]);
    assert_eq!(
        outcome_for(&snap, "nodejs22", MP),
        Ok(already_at("nodejs22", MP)),
        "the control: the outcome alone calls it already there"
    );
    let (id, detail) = start_for(&snap, "nodejs22", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::SafetyBlocked, "{detail}");
    assert!(
        detail.contains("holds node") && detail.contains("under another name"),
        "the refusal names what may be the same package: {detail}"
    );
}

#[test]
fn migrate_passes_on_plan_outcomes_refusals_unchanged() {
    let snap = with_broken(both(&[], &[("curl", None)]), HB);
    let (id, _) = start_for(&snap, "curl", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::ProviderUnhealthy);
}

#[test]
fn invariant_17_the_mirror_direction_is_refused_while_another_name_may_be_the_package() {
    // The same three packages, the formula asked for at its own provider:
    // the ports that may be it under another name stand in the way exactly
    // as the formula did for the port.
    let home = Some("https://nodejs.org");
    let snap = both(&[("node", home)], &[("nodejs22", home), ("nodejs24", home)]);
    assert_eq!(outcome_for(&snap, "node", HB), Ok(already_at("node", HB)));
    let (id, detail) = start_for(&snap, "node", HB).expect_err("must refuse");
    assert_eq!(id, ErrorId::SafetyBlocked, "{detail}");
    assert!(
        detail.contains("macports holds nodejs2")
            && detail.contains("may be node under another name"),
        "the refusal names the holder and the package it may be: {detail}"
    );
}

#[test]
fn invariant_17_an_undetected_provider_is_named_in_the_mirror_direction_too() {
    let mut snap = both(&[("wget", None)], &[]);
    let mp = snap
        .providers
        .get_mut(&ProviderId::new(MP))
        .expect("present");
    mp.detection.presence = ProviderPresence::Absent;
    mp.capabilities = ProviderCapabilities {
        capabilities: Capability::all()
            .iter()
            .map(|c| (*c, CapabilityState::Unknown))
            .collect(),
    };
    assert_eq!(outcome_for(&snap, "wget", HB), Ok(already_at("wget", HB)));
    match start_for(&snap, "wget", HB) {
        Ok(MigrationStart::NothingToMove { not_detected, .. }) => {
            assert_eq!(not_detected, vec![ProviderId::new(MP)]);
        }
        other => panic!("expected nothing to move, got {other:?}"),
    }
}

/// The planner raises `reason.mapping_confidence_below_threshold` as well as
/// the policy rule, and its `confidence` and `threshold` parameters must be in
/// the vocabulary the rule uses - the one the renderer resolves through the
/// `confidence.*` labels. It used to pass `format!("{confidence:?}")`, so the
/// same reason id arrived as `None` from one emitter and `NONE` from the other,
/// and the sentence printed whichever it was given.
#[test]
fn the_planners_mapping_confidence_params_are_confidence_identifiers() {
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    let vocabulary = macportsbrew_policy::labels::confidence_vocabulary();
    let snapshot = both(&[("onlyhere", None)], &[("unrelated", None)]);
    let plan = build_plan(
        &snapshot,
        &policy,
        &AnalysisOptions::default(),
        &PlanRequest {
            package: "onlyhere".to_string(),
            to: ProviderId::new(MP),
            retain_source: false,
        },
        &known(),
    )
    .expect("a package with no counterpart still gets a plan that halts for review");
    let reason = plan
        .risks
        .iter()
        .map(|risk| &risk.reason)
        .find(|reason| reason.id.as_str() == "reason.mapping_confidence_below_threshold")
        .expect("no counterpart is below the mapping threshold");
    for (name, expected) in macportsbrew_policy::labels::mapping_confidence_params(
        macportsbrew_domain::Confidence::None,
    ) {
        let value = reason.params.get(name).map(String::as_str);
        assert_eq!(value, Some(expected), "parameter {name}");
        assert!(
            vocabulary.contains(&expected),
            "{expected} is a Confidence identifier"
        );
    }
}

// --- DP-027 (c): a provider's own configuration on the plan ------------------

fn with_configuration(mut snap: Snapshot, provider: &str, keys: &[&str]) -> Snapshot {
    snap.providers
        .get_mut(&ProviderId::new(provider))
        .expect("present")
        .configuration = macportsbrew_domain::ProviderConfiguration::Read {
        files: vec![macportsbrew_domain::ConfigurationFile {
            path: "~/.config-file".into(),
            scope: macportsbrew_domain::ConfigurationScope::User,
            readable: true,
            keys: keys.iter().map(|k| (*k).to_string()).collect(),
        }],
    };
    snap
}

fn configuration_risks(snap: &Snapshot, package: &str, to: &str) -> Vec<(String, bool)> {
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    let plan = build_plan(
        snap,
        &policy,
        &AnalysisOptions::default(),
        &PlanRequest {
            package: package.to_string(),
            to: ProviderId::new(to),
            retain_source: false,
        },
        &known(),
    )
    .expect("a plan");
    plan.risks
        .iter()
        .filter(|r| r.id == macportsbrew_application::planner::PROVIDER_CONFIGURATION_RISK)
        .map(|r| {
            (
                r.reason.params.get("provider").cloned().unwrap_or_default(),
                r.blocking,
            )
        })
        .collect()
}

#[test]
fn a_provider_whose_configuration_overrides_this_tool_is_a_non_blocking_plan_risk() {
    let snap = with_configuration(
        both(&[("wget", None)], &[("curl", None)]),
        HB,
        &["HOMEBREW_NO_ANALYTICS"],
    );
    assert_eq!(
        configuration_risks(&snap, "wget", MP),
        vec![(HB.to_string(), false)]
    );
}

#[test]
fn invariant_17_the_destination_side_raises_it_too() {
    let snap = with_configuration(
        both(&[("wget", None)], &[("curl", None)]),
        MP,
        &["portdbpath"],
    );
    assert_eq!(
        configuration_risks(&snap, "wget", MP),
        vec![(MP.to_string(), false)]
    );
}

#[test]
fn a_configuration_file_that_sets_no_key_raises_nothing() {
    let snap = with_configuration(both(&[("wget", None)], &[("curl", None)]), HB, &[]);
    assert!(configuration_risks(&snap, "wget", MP).is_empty());
}

#[test]
fn a_configuration_nobody_could_read_is_an_unknown_on_the_plan() {
    // A recording's answer. `migrate --replay --yes` reaches the gate, so the
    // unknown travels as a risk the gate refuses from (spec-guardian,
    // 2026-10-04); an unknown is never a yes.
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    for state in [
        macportsbrew_domain::ProviderConfiguration::Unknowable,
        macportsbrew_domain::ProviderConfiguration::NotRecorded,
    ] {
        let mut snap = both(&[("wget", None)], &[("curl", None)]);
        snap.providers
            .get_mut(&ProviderId::new(HB))
            .expect("present")
            .configuration = state.clone();
        let plan = build_plan(
            &snap,
            &policy,
            &AnalysisOptions::default(),
            &PlanRequest {
                package: "wget".to_string(),
                to: ProviderId::new(MP),
                retain_source: false,
            },
            &known(),
        )
        .expect("a plan");
        let unknown: Vec<&str> = plan
            .risks
            .iter()
            .filter(|r| r.id == macportsbrew_application::planner::PROVIDER_CONFIGURATION_RISK)
            .map(|r| r.reason.id.as_str())
            .collect();
        assert_eq!(
            unknown,
            ["reason.provider_configuration_unknown"],
            "{state:?}"
        );
    }
}

#[test]
fn invariant_17_the_mirror_direction_raises_it_on_both_sides() {
    // MacPorts to Homebrew: the source's configuration and the destination's
    // each raise their own risk, exactly as in the other direction.
    let snap = with_configuration(
        with_configuration(
            both(&[("wget", None)], &[("curl", None)]),
            MP,
            &["portdbpath"],
        ),
        HB,
        &["HOMEBREW_CACHE"],
    );
    let mut risks = configuration_risks(&snap, "curl", HB);
    risks.sort();
    assert_eq!(
        risks,
        vec![(HB.to_string(), false), (MP.to_string(), false)]
    );
}
