// SPDX-License-Identifier: Apache-2.0

//! The history record built from a run (`crates/application/src/history.rs`,
//! S8): from the plan the planner built, the per-step records the loop's
//! decisions give, and the outcome `outcome_of` derives. Nothing here runs a
//! process or reads a clock; the start time is supplied, as the one clock read
//! in the workspace supplies it (`invariant_14_the_wall_clock_is_read_in_exactly_one_place`,
//! `crates/adapters/tests/invariants.rs`). The record's own rules are tested in
//! `crates/domain/tests/history.rs`.

use macportsbrew_application::{
    judge_step, next_action, outcome_of, plan_run, run_record, Action, AnalysisOptions,
    Disposition, HaltReason, PlanRequest, PlannedRun, RunOutcome, StepRead, StepRecord,
    Unevaluable,
};
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, Capability,
    CapabilityState, CommandLineTools, EnvironmentSnapshot, FailureCause, InstalledPackage,
    Machine, MacosVersion, MigrationPlan, PackageIdentity, PackageKind, PackageMetadata,
    ProcessEnd, Provenance, ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory,
    ProviderPresence, ProviderSnapshot, ReadOutput, RequestedSet, RunEnding, RunRecord, RunStart,
    RunStatus, Snapshot, StepKind, Translation, Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const HB: &str = "homebrew";
const MP: &str = "macports";

struct Pkg {
    name: &'static str,
    homepage: Option<&'static str>,
    service: bool,
}

fn pkg(name: &'static str) -> Pkg {
    Pkg {
        name,
        homepage: None,
        service: false,
    }
}

fn provider(id: &str, prefix: &str, packages: &[Pkg]) -> ProviderSnapshot {
    let pid = ProviderId::new(id);
    let mut inventory = ProviderInventory::default();
    for p in packages {
        inventory.installed.push(InstalledPackage {
            identity: PackageIdentity::new(pid.clone(), p.name),
            kind: PackageKind::Package,
            version: Version::new("1.0"),
            revision: None,
            variants: BTreeSet::new(),
            active: true,
            requested: Some(true),
            installed_from_prebuilt: Some(true),
            install_path: None,
        });
        inventory.requested.names.insert(p.name.to_string());
        inventory.metadata.insert(
            p.name.to_string(),
            PackageMetadata {
                homepage: p.homepage.map(str::to_string),
                defines_service: p.service,
                service: Some(macportsbrew_domain::ServiceDefinition::both(
                    macportsbrew_domain::Answer::from(p.service),
                )),
                ..PackageMetadata::default()
            },
        );
        inventory.availability.insert(
            p.name.to_string(),
            AvailabilityAssessment {
                availability: ArtifactAvailability::BinaryAvailable,
                evidence: vec![AvailabilityEvidence {
                    source: "test".into(),
                    detail: "prebuilt".into(),
                    offline: true,
                }],
                cached: false,
            },
        );
    }
    inventory.requested = RequestedSet {
        names: inventory.requested.names.clone(),
        mechanism: "test".into(),
    };
    let mut capabilities = ProviderCapabilities::default();
    capabilities.capabilities.insert(
        Capability::MachineReadableInventory,
        CapabilityState::Available,
    );
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
        capabilities,
        inventory,
        environment: BTreeMap::new(),
        invocation_environment: BTreeMap::new(),
        resolutions: BTreeMap::new(),
        configuration: macportsbrew_domain::ProviderConfiguration::NotRecorded,
    }
}

fn snapshot(hb: &[Pkg], mp: &[Pkg]) -> Snapshot {
    let mut providers = BTreeMap::new();
    providers.insert(ProviderId::new(HB), provider(HB, "/opt/homebrew", hb));
    providers.insert(ProviderId::new(MP), provider(MP, "/opt/local", mp));
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-30T00:00:00Z".into(),
            captured_by: "executor.rs".into(),
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
        providers,
    }
}

/// A package held by `source` only, paired with a different name on the
/// other side by the curated table, so its plan carries a real add in either
/// direction (INV-17).
fn install_needed(source: &str) -> (Snapshot, &'static str, &'static str) {
    if source == HB {
        (snapshot(&[pkg("gnu-sed")], &[pkg("other")]), "gnu-sed", MP)
    } else {
        (snapshot(&[pkg("other")], &[pkg("gsed")]), "gsed", HB)
    }
}

fn planned(source: &str) -> PlannedRun {
    let (snap, package, to) = install_needed(source);
    planned_from(&snap, package, to)
}

fn planned_from(snap: &Snapshot, package: &str, to: &str) -> PlannedRun {
    plan_run(
        snap,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        &AnalysisOptions {
            today: "2026-09-30".into(),
            advanced: false,
            only: BTreeSet::new(),
        },
        &PlanRequest {
            package: package.into(),
            to: ProviderId::new(to),
            retain_source: false,
        },
        &snap.providers.keys().cloned().collect::<Vec<_>>(),
    )
    .expect("the planner builds this plan")
}

fn start() -> RunStart {
    RunStart {
        id: "run-1".into(),
        started_at: "2026-10-01T00:00:00Z".into(),
        tool_version: "0.1.0".into(),
    }
}

fn finished(stdout: &str) -> ReadOutput<'_> {
    ReadOutput {
        end: ProcessEnd::Exited { code: 0 },
        stdout,
        truncated: false,
    }
}

fn verified() -> Disposition {
    Disposition::Verified {
        evidence: Vec::new(),
    }
}

fn halted(reason: HaltReason) -> Disposition {
    Disposition::Halted {
        ends: Vec::new(),
        reason,
        evidence: Vec::new(),
    }
}

/// One record per step of `plan`, from `by_kind`; every step after the first
/// halt is `NOT_REACHED`, as the loop leaves it.
fn records(
    plan: &MigrationPlan,
    mut by_kind: impl FnMut(usize, StepKind) -> Disposition,
) -> Vec<StepRecord> {
    let mut out: Vec<StepRecord> = Vec::new();
    for (position, step) in plan.steps.iter().enumerate() {
        let stopped = out
            .iter()
            .any(|r| matches!(r.disposition, Disposition::Halted { .. }));
        let disposition = if stopped {
            Disposition::NotReached
        } else {
            by_kind(position, step.kind)
        };
        out.push(StepRecord {
            index: step.index,
            kind: step.kind,
            disposition,
        });
    }
    out
}

/// What this build's loop records for a step it does not dispatch.
fn guarded(kind: StepKind) -> Disposition {
    if kind.is_mutating() {
        Disposition::NotDispatchedMutating
    } else {
        Disposition::NotPerformedInThisBuild
    }
}

fn record_of(run: &PlannedRun, records: &[StepRecord]) -> (RunOutcome, RunRecord) {
    let outcome = outcome_of(run.plan(), records);
    let record = run_record(run, records, start(), None);
    (outcome, record)
}

/// Every string anywhere in a serialised value.
fn strings(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(items) => items.iter().for_each(|v| strings(v, out)),
        serde_json::Value::Object(map) => map.iter().for_each(|(k, v)| {
            out.push(k.clone());
            strings(v, out);
        }),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

#[test]
fn a_run_record_cannot_carry_an_inventory() {
    for source in [HB, MP] {
        let run = planned(source);
        // A complete inventory, as a removal read prints it: the package is gone,
        // and every other line names something on this machine.
        let inventory = "secret-internal-tool 9.9\nother 1.0\nanother-private-thing 2.0\n";
        let records = records(run.plan(), |position, kind| match kind {
            StepKind::VerifyRemoval => {
                judge_step(&run, position, &[StepRead::Output(finished(inventory))])
            }
            StepKind::VerifyDestination | StepKind::VerifyResolution => verified(),
            other => guarded(other),
        });
        let removal = records
            .iter()
            .find(|r| r.kind == StepKind::VerifyRemoval)
            .expect("a removal step");
        assert!(
            matches!(removal.disposition, Disposition::Verified { .. }),
            "the removal read is judged and kept: {removal:?}"
        );
        let (_, record) = record_of(&run, &records);
        let text = serde_json::to_string(&record).expect("serialises");
        for line in inventory.lines() {
            let name = line.split_whitespace().next().expect("a name");
            if name != "other" {
                assert!(!text.contains(name), "{name} reached the record: {text}");
            }
        }
        assert!(
            text.contains("\"lineCount\":3"),
            "the count is kept: {text}"
        );
        // Structurally: an inventory is lines, and no string in the record holds
        // a line break.
        let mut all = Vec::new();
        strings(&serde_json::to_value(&record).expect("a value"), &mut all);
        assert!(all.iter().all(|s| !s.contains('\n')), "{all:?}");
    }
}

#[test]
fn invariant_03_a_partial_run_is_recorded_as_partial_never_success() {
    for source in [HB, MP] {
        // What a mutating build would hand back: the destination verified, then
        // the source removal dispatched and its read failed.
        let run = planned(source);
        let records = records(run.plan(), |_, kind| match kind {
            StepKind::VerifyDestination | StepKind::VerifyResolution => verified(),
            StepKind::RemoveSource => halted(HaltReason::VerificationFailed {
                command: 0,
                cause: FailureCause::NamePresent,
            }),
            other => guarded(other),
        });
        let (outcome, record) = record_of(&run, &records);
        assert!(
            matches!(outcome, RunOutcome::PartialSuccess { .. }),
            "{outcome:?}"
        );
        assert!(record.mutation_dispatched());
        assert_eq!(record.status(), RunStatus::Partial);
        assert_ne!(record.status(), RunStatus::Success);
        assert!(matches!(record.ending(), RunEnding::PartialSuccess { .. }));
    }
}

#[test]
fn a_partial_success_that_dispatched_no_mutation_is_not_a_partial_migration() {
    for source in [HB, MP] {
        // This build's loop over a plan whose mutating steps carry no command:
        // the destination read passes, the resolution cannot be evaluated.
        let run = planned(source);
        let records = records(run.plan(), |_, kind| match kind {
            StepKind::VerifyDestination => verified(),
            StepKind::VerifyResolution => halted(HaltReason::Unevaluable {
                check: None,
                why: Unevaluable::NoEvaluationForStepKind,
            }),
            other => guarded(other),
        });
        let (outcome, record) = record_of(&run, &records);
        assert_eq!(outcome.exit_code(), 3, "{outcome:?}");
        assert!(!record.mutation_dispatched());
        assert_eq!(record.status(), RunStatus::NoChange);
        // The loop's ending is kept beside the status, not lost to it.
        assert!(matches!(record.ending(), RunEnding::PartialSuccess { .. }));
    }
}

#[test]
fn a_run_the_dispatch_guard_halted_changed_nothing() {
    // The real decisions over the planner's own plan: the first mutating step
    // carries a command, so the guard halts the run there.
    for source in [HB, MP] {
        let run = planned(source);
        let plan = run.plan();
        let mut records: Vec<StepRecord> = Vec::new();
        for position in 0..plan.steps.len() {
            let disposition = if records
                .iter()
                .any(|r| matches!(r.disposition, Disposition::Halted { .. }))
            {
                Disposition::NotReached
            } else {
                match next_action(plan, position, &records) {
                    Action::PassOver(d) => d,
                    Action::Halt(reason) => halted(reason),
                    Action::Dispatch
                    | Action::Certify
                    | Action::DispatchMutating { .. }
                    | Action::TakeSnapshot
                    | Action::RecordHistory => {
                        panic!("nothing is dispatched before the guard halts")
                    }
                }
            };
            records.push(StepRecord {
                index: plan.steps[position].index,
                kind: plan.steps[position].kind,
                disposition,
            });
        }
        assert!(records
            .iter()
            .any(|r| r.disposition == halted(HaltReason::UnexecutedMutation)));
        let (outcome, record) = record_of(&run, &records);
        assert!(matches!(outcome, RunOutcome::Halted { .. }), "{outcome:?}");
        assert!(!record.mutation_dispatched());
        assert_eq!(record.status(), RunStatus::NoChange);
    }
}

#[test]
fn a_halt_the_records_cannot_place_counts_as_a_dispatch() {
    // Fail toward "something changed": records that do not match the plan,
    // on a mutating step, may hide a dispatch.
    let run = planned(HB);
    let unplaced = records(run.plan(), |_, kind| match kind {
        StepKind::AddDestination => halted(HaltReason::RecordsDoNotMatchPlan),
        other => guarded(other),
    });
    assert!(record_of(&run, &unplaced).1.mutation_dispatched());
    // A halt only the guard or the review check gives is not a dispatch.
    for reason in [
        HaltReason::UnexecutedMutation,
        HaltReason::ManualReview { step: 2 },
    ] {
        let guarded_run = records(run.plan(), |_, kind| match kind {
            StepKind::AddDestination => halted(reason.clone()),
            other => guarded(other),
        });
        assert!(
            !record_of(&run, &guarded_run).1.mutation_dispatched(),
            "{reason:?}"
        );
    }
}

#[test]
fn the_record_is_built_from_the_plan_and_the_snapshot_before_it() {
    // An older, inactive row of the same package comes first: the version
    // recorded is the active one.
    let (mut snap, package, to) = install_needed(HB);
    let rows = &mut snap
        .providers
        .get_mut(&ProviderId::new(HB))
        .expect("the source")
        .inventory
        .installed;
    let mut older = rows[0].clone();
    older.version = Version::new("0.9");
    older.active = false;
    rows.insert(0, older);
    let run = planned_from(&snap, package, to);
    let records = records(run.plan(), |_, kind| guarded(kind));
    let (_, record) = record_of(&run, &records);
    let subject = record.subject();
    assert_eq!(subject.package, run.plan().package);
    assert_eq!(subject.direction.source, run.plan().source);
    assert_eq!(subject.direction.destination, run.plan().destination);
    assert_eq!(subject.versions.source, Some(Version::new("1.0")));
    assert_eq!(subject.versions.destination, None);
    assert_eq!(subject.retain_source, run.plan().retain_source);
    assert_eq!(subject.primary_after, run.plan().primary_after);
    assert_eq!(record.steps().len(), run.plan().steps.len());
    assert_eq!(record.started_at(), "2026-10-01T00:00:00Z");
}

#[test]
fn a_run_record_serialises_deterministically() {
    for source in [HB, MP] {
        let bytes = || {
            let run = planned(source);
            let records = records(run.plan(), |_, kind| match kind {
                StepKind::VerifyDestination => verified(),
                other => guarded(other),
            });
            serde_json::to_vec(&record_of(&run, &records).1).expect("serialises")
        };
        assert_eq!(bytes(), bytes());
    }
}

#[test]
fn a_read_back_matches_only_the_record_that_was_written() {
    use macportsbrew_application::read_back_matches;
    let written = "{\n  \"schemaVersion\": 2\n}\n";
    assert!(read_back_matches(written, written));
    // Another record, a truncated one, one with a changed byte, and nothing.
    assert!(!read_back_matches(
        written,
        "{\n  \"schemaVersion\": 3\n}\n"
    ));
    assert!(!read_back_matches(written, &written[..written.len() - 1]));
    assert!(!read_back_matches(written, ""));
    // Two empty texts are not a confirmation: an empty write is no record.
    assert!(!read_back_matches("", ""));
}
