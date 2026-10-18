// SPDX-License-Identifier: Apache-2.0

//! `restore_request`, and the plans the one planner builds from it (pass 08
//! slice 5, Part C; D9, D11). Both directions, because a restore is a migration
//! the other way and must be built by the same code (INV-17); the ordering,
//! reverse-dependency and stateful/service rules are the planner's, and are
//! shown here to hold on restore plans.

use macportsbrew_application::{
    check_same_package, plan_outcome, restore_notes, restore_request, AnalysisOptions, KeepReason,
    PlanOutcome, Removal, RestoreNote, RestoreRefusal,
};
use macportsbrew_domain::{
    Architecture, Capability, CapabilityState, CommandEnd, CommandLineTools, DependencyEdge,
    Direction, EnvironmentSnapshot, ErrorId, InstalledPackage, Machine, MacosVersion,
    MigrationPlan, Outcome, PackageIdentity, PackageKind, PackageMetadata, Provenance,
    ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory, ProviderPresence,
    ProviderSnapshot, RecordedStep, RecordedVersions, RequestedSet, RunEnding, RunRecord, RunStart,
    RunSubject, Snapshot, StepKind, StepResult, Translation, Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const HB: &str = "homebrew";
const MP: &str = "macports";

/// The package every test moves, as each provider spells it. The pair is a
/// curated equivalence in `curated-mappings.toml`, which is held in one
/// direction and indexed in both (INV-17): a restore toward the run's source
/// has no installed counterpart left to discover a mapping from, so only a
/// curated row (or a counterpart still installed) lets the planner plan it.
fn name_at(provider: &str) -> &'static str {
    if provider == HB {
        "pkgconf"
    } else {
        "pkgconfig"
    }
}

fn known() -> Vec<ProviderId> {
    vec![ProviderId::new(HB), ProviderId::new(MP)]
}

/// A provider holding exactly the named packages, all requested, each sharing a
/// homepage with its counterpart so the mapping is discovered by evidence.
fn holding(id: &str, prefix: &str, names: &[&str], service: &[&str]) -> ProviderSnapshot {
    let pid = ProviderId::new(id);
    let mut inventory = ProviderInventory::default();
    for name in names {
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
                homepage: Some(if name.starts_with("pkgconf") {
                    "https://example.org/pkgconf".to_string()
                } else {
                    format!("https://example.org/{name}")
                }),
                defines_service: service.contains(name),
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
        capabilities: ProviderCapabilities {
            capabilities: [
                (
                    Capability::MachineReadableInventory,
                    CapabilityState::Available,
                ),
                (Capability::ServiceMetadata, CapabilityState::Available),
            ]
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

fn snapshot_of(hb: ProviderSnapshot, mp: ProviderSnapshot) -> Snapshot {
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-10-03T12:00:00Z".into(),
            captured_by: "restore.rs".into(),
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
        providers: [(ProviderId::new(HB), hb), (ProviderId::new(MP), mp)]
            .into_iter()
            .collect(),
    }
}

fn machine(hb_holds: bool, mp_holds: bool) -> Snapshot {
    let names = |holds: bool, provider: &'static str| -> Vec<&'static str> {
        if holds {
            vec![name_at(provider)]
        } else {
            Vec::new()
        }
    };
    snapshot_of(
        holding(HB, "/opt/homebrew", &names(hb_holds, HB), &[]),
        holding(MP, "/opt/local", &names(mp_holds, MP), &[]),
    )
}

/// A run of `package` (as `source` spells it; the destination's spelling is
/// the other name of the pair when it is the pair) from `source` to
/// `destination` whose add ended as `add`.
fn run(source: &str, destination: &str, package: &str, add: StepResult) -> RunRecord {
    let step = |index, kind, result| RecordedStep::new(index, kind, None, result, false);
    RunRecord::new(
        RunStart {
            id: "20261003T120000Z-00000000000000aa".into(),
            started_at: "2026-10-03T12:00:00Z".into(),
            tool_version: "0.1.0".into(),
        },
        RunSubject {
            plan_schema_version: 1,
            policy_version: "test".into(),
            package: PackageIdentity::new(ProviderId::new(source), package),
            direction: Direction {
                source: ProviderId::new(source),
                destination: ProviderId::new(destination),
            },
            destination_package: (package == name_at(source))
                .then(|| name_at(destination).to_string()),
            versions: RecordedVersions {
                source: Some(Version::new("1.0")),
                destination: None,
            },
            action: Outcome::MigrateTo {
                provider: ProviderId::new(destination),
            },
            overrode: None,
            retain_source: false,
            primary_after: ProviderId::new(destination),
        },
        vec![
            step(0, StepKind::SnapshotBefore, StepResult::SnapshotTaken),
            step(1, StepKind::AddDestination, add),
        ],
        RunEnding::Completed,
        Some("snapshot-20261003T120000Z.json".into()),
    )
}

fn added() -> StepResult {
    StepResult::Performed {
        ends: vec![CommandEnd::Exited { code: 0 }],
    }
}

fn plan_for(
    live: &Snapshot,
    record: &RunRecord,
    before: &Snapshot,
) -> (macportsbrew_application::RestoreIntent, PlanOutcome) {
    let intent = restore_request(record, before).expect("a request");
    let policy = PolicyData::load_embedded().expect("policy");
    let outcome = plan_outcome(
        live,
        &policy,
        &AnalysisOptions::default(),
        &intent.request,
        &known(),
    )
    .expect("a planner outcome");
    (intent, outcome)
}

fn plan_of(outcome: PlanOutcome) -> MigrationPlan {
    match outcome {
        PlanOutcome::Plan(plan) => *plan,
        PlanOutcome::AlreadyAtDestination { .. } => panic!("expected a plan"),
    }
}

fn kinds(plan: &MigrationPlan) -> Vec<StepKind> {
    plan.steps.iter().map(|s| s.kind).collect()
}

/// The restore of a run that moved the package from `a` to `b`, the source
/// removed: `a` held it before, only `b` holds it now.
fn moved_and_removed(a: &str, b: &str) -> (RunRecord, Snapshot, Snapshot) {
    let record = run(a, b, name_at(a), added());
    let held_by = |provider: &str| machine(provider == HB, provider == MP);
    (record, held_by(a), held_by(b))
}

fn manual_review_reason(plan: &MigrationPlan) -> String {
    plan.steps
        .iter()
        .find(|s| s.kind == StepKind::ManualReview)
        .map(|s| s.reason.id.as_str().to_owned())
        .expect("a manual review step")
}

#[test]
fn invariant_17_restore_is_direction_neutral() {
    for (a, b) in [(HB, MP), (MP, HB)] {
        let (record, before, live) = moved_and_removed(a, b);
        let (intent, outcome) = plan_for(&live, &record, &before);
        assert_eq!(intent.removal, Removal::Removes, "{a} -> {b}");
        let plan = plan_of(outcome);
        assert!(
            !plan.requires_manual_review(),
            "{a} -> {b}: {:?}",
            plan.steps[0]
        );
        // Put back where it came from, taken from where the run put it.
        assert_eq!(plan.destination.as_str(), a);
        assert_eq!(plan.source.as_str(), b);
        assert_eq!(check_same_package(&intent, &plan), Ok(()));
        // As the source spells it, not as the run's destination does.
        assert_eq!(plan.destination_package.as_deref(), Some(name_at(a)));
    }
}

#[test]
fn invariant_01_a_restore_plan_adds_before_it_removes() {
    for (a, b) in [(HB, MP), (MP, HB)] {
        let (record, before, live) = moved_and_removed(a, b);
        let (_, outcome) = plan_for(&live, &record, &before);
        let plan = plan_of(outcome);
        let order = kinds(&plan);
        let position = |kind| {
            order
                .iter()
                .position(|k| *k == kind)
                .unwrap_or_else(|| panic!("{a} -> {b}: no {kind:?} in {order:?}"))
        };
        let (add, verify, remove) = (
            position(StepKind::AddDestination),
            position(StepKind::VerifyDestination),
            position(StepKind::RemoveSource),
        );
        assert!(add < verify && verify < remove, "{a} -> {b}: {order:?}");
        // The reinstall is a real command, not the empty step of a package the
        // destination already holds.
        assert!(!plan.steps[add].commands.is_empty(), "{a} -> {b}");
        assert_eq!(plan.check_ordering(), Ok(()));
        assert!(!plan.retain_source);
    }
}

#[test]
fn a_run_made_with_retain_source_is_the_cheap_case_nothing_is_reinstalled() {
    for (a, b) in [(HB, MP), (MP, HB)] {
        let record = run(a, b, name_at(a), added());
        // Before: only `a`. After: both, because the run kept its source.
        let before = machine(a == HB, a == MP);
        let live = machine(true, true);
        let (intent, outcome) = plan_for(&live, &record, &before);
        assert_eq!(intent.removal, Removal::Removes);
        let plan = plan_of(outcome);
        let order = kinds(&plan);
        let add = order
            .iter()
            .position(|k| *k == StepKind::AddDestination)
            .expect("the step stays, so every plan has one shape");
        assert!(
            plan.steps[add].commands.is_empty(),
            "nothing is reinstalled: {a} -> {b}"
        );
        let verify = order
            .iter()
            .position(|k| *k == StepKind::VerifyDestination)
            .expect("the retained package is verified first");
        let remove = order
            .iter()
            .position(|k| *k == StepKind::RemoveSource)
            .expect("then what the run added is removed");
        assert!(verify < remove, "{order:?}");
    }
}

#[test]
fn a_package_the_destination_held_before_the_run_is_never_removed() {
    for (a, b) in [(HB, MP), (MP, HB)] {
        let record = run(a, b, name_at(a), added());
        // Before the run both held it; the add was a no-op the record still
        // shows as performed.
        let before = machine(true, true);
        let live = machine(true, true);
        let (intent, outcome) = plan_for(&live, &record, &before);
        assert_eq!(
            intent.removal,
            Removal::Keeps(KeepReason::DestinationHeldItBefore)
        );
        assert!(intent.request.retain_source);
        let plan = plan_of(outcome);
        assert!(
            !kinds(&plan).contains(&StepKind::RemoveSource),
            "{a} -> {b}: {:?}",
            kinds(&plan)
        );
    }
}

#[test]
fn a_run_whose_add_did_not_exit_zero_removes_nothing() {
    for (a, b) in [(HB, MP), (MP, HB)] {
        let before = machine(a == HB, a == MP);
        let live = machine(true, true);
        let halted = StepResult::Halted {
            error: ErrorId::ProviderTimeout,
            evidence: Vec::new(),
            ends: vec![CommandEnd::DidNotFinish],
        };
        let record = run(a, b, name_at(a), halted);
        let (intent, outcome) = plan_for(&live, &record, &before);
        assert_eq!(intent.removal, Removal::Keeps(KeepReason::AddNotPerformed));
        assert!(intent.request.retain_source);
        assert!(!kinds(&plan_of(outcome)).contains(&StepKind::RemoveSource));
        // Nor does an add that exited non-zero.
        let failed = StepResult::Halted {
            error: ErrorId::Io,
            evidence: Vec::new(),
            ends: vec![CommandEnd::Exited { code: 1 }],
        };
        let record = run(a, b, name_at(a), failed);
        assert_eq!(
            restore_request(&record, &before).unwrap().removal,
            Removal::Keeps(KeepReason::AddNotPerformed)
        );
    }
}

#[test]
fn a_destination_the_snapshot_did_not_read_is_not_known_to_have_lacked_the_package() {
    let record = run(HB, MP, name_at(HB), added());
    let mut before = machine(true, false);
    before
        .providers
        .get_mut(&ProviderId::new(MP))
        .unwrap()
        .detection
        .presence = ProviderPresence::Absent;
    let intent = restore_request(&record, &before).expect("a request");
    assert_eq!(
        intent.removal,
        Removal::Keeps(KeepReason::BeforeStateNotEstablished)
    );
    assert!(intent.request.retain_source);
    let mut missing = machine(true, false);
    missing.providers.remove(&ProviderId::new(MP));
    assert_eq!(
        restore_request(&record, &missing).unwrap().removal,
        Removal::Keeps(KeepReason::BeforeStateNotEstablished)
    );
}

#[test]
fn a_run_that_dispatched_nothing_has_no_restore_request() {
    let step = |index, kind, result| RecordedStep::new(index, kind, None, result, false);
    let record = RunRecord::new(
        RunStart {
            id: "20261003T120000Z-00000000000000aa".into(),
            started_at: "2026-10-03T12:00:00Z".into(),
            tool_version: "0.1.0".into(),
        },
        run(HB, MP, name_at(HB), added()).subject().clone(),
        vec![step(
            1,
            StepKind::AddDestination,
            StepResult::NotDispatchedMutating,
        )],
        RunEnding::Refused { refusals: 1 },
        None,
    );
    assert_eq!(
        restore_request(&record, &machine(true, false)),
        Err(RestoreRefusal::NothingDispatched)
    );
}

#[test]
fn a_machine_already_back_in_its_earlier_state_has_nothing_to_restore() {
    for (a, b) in [(HB, MP), (MP, HB)] {
        let (record, before, _) = moved_and_removed(a, b);
        // The live machine is the earlier one again: only the source holds it.
        let (_, outcome) = plan_for(&before, &record, &before);
        assert!(
            matches!(outcome, PlanOutcome::AlreadyAtDestination { .. }),
            "{a} -> {b}: {outcome:?}"
        );
    }
}

#[test]
fn a_plan_for_another_package_than_the_run_moved_is_refused_by_name() {
    let (record, before, live) = moved_and_removed(HB, MP);
    let (mut intent, outcome) = plan_for(&live, &record, &before);
    let plan = plan_of(outcome);
    intent.expected_package = "wget".into();
    match check_same_package(&intent, &plan) {
        Err(RestoreRefusal::NotTheRunsPackage { planned, recorded }) => {
            assert!(planned.contains("pkgconf"), "{planned}");
            assert!(recorded.contains("wget"), "{recorded}");
        }
        other => panic!("{other:?}"),
    }
    // Nor one taken from somewhere else than where the run put it.
    let (mut intent, outcome) = plan_for(&live, &record, &before);
    intent.restore_from = ProviderId::new(HB);
    assert!(check_same_package(&intent, &plan_of(outcome)).is_err());
}

#[test]
fn the_request_names_the_package_as_the_runs_destination_spells_it() {
    // Mutation proof (slice 5): the planner resolves a package through its unit,
    // which answers to either spelling of a curated pair, so the outcome of a
    // plan does not show which spelling the request carried. The request does.
    let (record, before, _) = moved_and_removed(HB, MP);
    let intent = restore_request(&record, &before).unwrap();
    assert_eq!(intent.request.package, name_at(MP));
    assert_eq!(intent.request.to.as_str(), HB);
    // A run that kept one name on both sides asks for that name.
    let same = restore_request(&run(HB, MP, "htop", added()), &machine(true, false)).unwrap();
    assert_eq!(same.request.package, "htop");
}

#[test]
fn a_plan_that_puts_the_package_back_somewhere_else_is_refused() {
    // Mutation proof (slice 5): each of the three comparisons has its own case.
    let (record, before, live) = moved_and_removed(HB, MP);
    let (mut intent, outcome) = plan_for(&live, &record, &before);
    let plan = plan_of(outcome);
    assert_eq!(check_same_package(&intent, &plan), Ok(()));
    intent.restore_to = ProviderId::new(MP);
    assert!(matches!(
        check_same_package(&intent, &plan),
        Err(RestoreRefusal::NotTheRunsPackage { .. })
    ));
}

#[test]
fn invariant_04_a_restore_never_removes_what_another_package_of_that_provider_needs() {
    // The package is what the run added; `app`, installed in the same provider,
    // depends on it. The planner stops for a person rather than breaking it.
    for (a, b) in [(HB, MP), (MP, HB)] {
        let (record, before, mut live) = moved_and_removed(a, b);
        let holder = live.providers.get_mut(&ProviderId::new(b)).unwrap();
        holder.inventory.installed.push(InstalledPackage {
            identity: PackageIdentity::new(ProviderId::new(b), "app"),
            kind: PackageKind::Package,
            version: Version::new("1.0"),
            revision: None,
            variants: BTreeSet::new(),
            active: true,
            requested: Some(true),
            installed_from_prebuilt: Some(true),
            install_path: None,
        });
        holder.inventory.dependencies.push(DependencyEdge {
            from: "app".into(),
            to: name_at(b).into(),
            build_only: false,
        });
        let (_, outcome) = plan_for(&live, &record, &before);
        let plan = plan_of(outcome);
        assert_eq!(
            manual_review_reason(&plan),
            "reason.reverse_dependencies_present",
            "{a} -> {b}"
        );
    }
}

#[test]
fn invariant_19_a_restore_never_touches_a_service_package() {
    for (a, b) in [(HB, MP), (MP, HB)] {
        let (record, before, _) = moved_and_removed(a, b);
        // The run's destination now holds it, and it defines a service there.
        let service = |provider: &str| {
            holding(
                provider,
                "/opt/x",
                &[name_at(provider)],
                &[name_at(provider)],
            )
        };
        let none = |provider: &str| holding(provider, "/opt/y", &[], &[]);
        let live = if b == HB {
            snapshot_of(service(HB), none(MP))
        } else {
            snapshot_of(none(HB), service(MP))
        };
        let (_, outcome) = plan_for(&live, &record, &before);
        assert_eq!(
            manual_review_reason(&plan_of(outcome)),
            "reason.class_requires_review",
            "{a} -> {b}"
        );
    }
}

#[test]
fn the_notes_say_what_the_plan_does_not() {
    let (record, before, _) = moved_and_removed(HB, MP);
    let intent = restore_request(&record, &before).unwrap();
    assert_eq!(
        restore_notes(&intent, false, false),
        vec![RestoreNote::NotVersionExact {
            package: "pkgconf".into(),
            recorded_version: Some("1.0".into()),
        }]
    );
    let held = restore_request(&run(HB, MP, name_at(HB), added()), &machine(true, true)).unwrap();
    let notes = restore_notes(&held, true, true);
    assert!(notes.contains(&RestoreNote::KeepsDestination {
        reason: KeepReason::DestinationHeldItBefore
    }));
    assert!(notes.contains(&RestoreNote::RecordNotConfirmed));
    assert!(notes.contains(&RestoreNote::OutcomeUncertain));
}

#[test]
fn a_restore_with_no_evidence_for_the_reverse_mapping_stops_for_a_person() {
    // Declared limitation (SE-092, pass 08 slice 5): when the run removed its
    // source, nothing installed there is left to discover a mapping from, so a
    // package without a curated row cannot reach the planner's confidence
    // threshold in the reverse direction and the plan stops at manual review.
    // The same planner, the same rule as `migrate`; not special-cased.
    let record = run(HB, MP, "htop", added());
    let before = snapshot_of(
        holding(HB, "/opt/homebrew", &["htop"], &[]),
        holding(MP, "/opt/local", &[], &[]),
    );
    let live = snapshot_of(
        holding(HB, "/opt/homebrew", &[], &[]),
        holding(MP, "/opt/local", &["htop"], &[]),
    );
    let (_, outcome) = plan_for(&live, &record, &before);
    assert_eq!(
        manual_review_reason(&plan_of(outcome)),
        "reason.mapping_confidence_below_threshold"
    );
}
