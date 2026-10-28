// SPDX-License-Identifier: Apache-2.0

//! The execution gate (`crates/application/src/execution_gate.rs`): one test
//! per refusal, the invariants it enforces, and the claim that it refuses
//! every plan this build's planner produces.
//!
//! Plans are built by the real planner wherever the planner can produce the
//! case, and otherwise by editing a plan it built — never written from
//! nothing, so that each test starts from a shape the product can emit and
//! changes exactly one thing. Every-fixture coverage over the captured
//! recordings is in `crates/cli/tests/execution_gate_fixtures.rs`: replaying a
//! recording needs the adapters, which this crate may not depend on, even for
//! tests (`scripts/check-layering.sh` reads dev-dependencies too).

use macportsbrew_application::{
    build_plan, evaluable_in_this_build, execution_refusals, AnalysisOptions, ExecutionRefusal,
    GateReport, PlanRequest,
};
use macportsbrew_domain::plan::PLAN_SCHEMA_VERSION;
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence,
    CommandLineTools, Elevation, EnvironmentSnapshot, ExpectationError, InstalledPackage, Machine,
    MacosVersion, MigrationPlan, PackageIdentity, PackageKind, PackageMetadata, Provenance,
    ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory, ProviderPresence,
    ProviderSnapshot, RequestedSet, Snapshot, StepKind, SuccessSemantic, Translation,
    VerificationCheck, Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const HB: &str = "homebrew";
const MP: &str = "macports";

struct Pkg {
    name: &'static str,
    homepage: Option<&'static str>,
    service: bool,
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
        capabilities: ProviderCapabilities::default(),
        inventory,
        environment: BTreeMap::new(),
        invocation_environment: [("PATH".to_string(), format!("{prefix}/bin:/usr/bin:/bin"))]
            .into_iter()
            .collect(),
        resolutions: BTreeMap::new(),
        configuration: macportsbrew_domain::ProviderConfiguration::Read { files: Vec::new() },
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
            captured_at: "2026-09-27T00:00:00Z".into(),
            captured_by: "execution_gate.rs".into(),
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

fn pkg(name: &'static str) -> Pkg {
    Pkg {
        name,
        homepage: None,
        service: false,
    }
}

/// `gnu-sed` under Homebrew and `gsed` under MacPorts are paired by the
/// curated table, so a package held by one side plans a real add on the other
/// — in either direction (INV-17). The other side holds an unrelated package
/// so that both providers are detected with a non-empty inventory.
fn install_needed(source: &str) -> (Snapshot, &'static str, &'static str) {
    if source == HB {
        (snapshot(&[pkg("gnu-sed")], &[pkg("other")]), "gnu-sed", MP)
    } else {
        (snapshot(&[pkg("other")], &[pkg("gsed")]), "gsed", HB)
    }
}

/// `tool` on both sides, paired by name and homepage: the destination already
/// holds it, so the add step carries no command. `postgresql@17` is a service
/// and stateful by curated data, which halts at `MANUAL_REVIEW`.
fn both_held() -> Snapshot {
    let tool = || Pkg {
        name: "tool",
        homepage: Some("https://example.org/tool"),
        service: false,
    };
    snapshot(
        &[
            tool(),
            Pkg {
                name: "postgresql@17",
                homepage: None,
                service: true,
            },
        ],
        &[tool()],
    )
}

fn plan_for(snap: &Snapshot, package: &str, to: &str, retain: bool) -> MigrationPlan {
    build_plan(
        snap,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        &AnalysisOptions {
            today: "2026-09-27".into(),
            advanced: false,
            only: BTreeSet::new(),
        },
        &PlanRequest {
            package: package.into(),
            to: ProviderId::new(to),
            retain_source: retain,
        },
        &snap.providers.keys().cloned().collect::<Vec<_>>(),
    )
    .expect("the planner builds this plan")
}

/// A real-install plan, source removed: every step kind but `MANUAL_REVIEW`.
fn full_plan() -> MigrationPlan {
    let (snap, package, to) = install_needed(HB);
    plan_for(&snap, package, to, false)
}

fn position(plan: &MigrationPlan, kind: StepKind) -> usize {
    plan.steps
        .iter()
        .position(|s| s.kind == kind)
        .unwrap_or_else(|| panic!("the plan carries a {kind:?} step"))
}

fn index_of(plan: &MigrationPlan, kind: StepKind) -> u32 {
    plan.steps[position(plan, kind)].index
}

fn refused(report: &GateReport, predicate: impl Fn(&ExecutionRefusal) -> bool) -> bool {
    report.refusals().iter().any(predicate)
}

/// The step a refusal names, 0 for one about the whole plan.
fn step_of(refusal: &ExecutionRefusal) -> u32 {
    match refusal {
        ExecutionRefusal::SchemaVersionNotThisBuilds { .. }
        | ExecutionRefusal::Ordering
        | ExecutionRefusal::StepIndicesNotSequential
        | ExecutionRefusal::EmptyPlan
        | ExecutionRefusal::NoRecordHistoryAtEnd
        | ExecutionRefusal::SameProviderOnBothSides
        | ExecutionRefusal::PlanNotDryRunOnly => 0,
        ExecutionRefusal::ManualReview { step }
        | ExecutionRefusal::NoSnapshotBeforeFirstMutation { step }
        | ExecutionRefusal::CheckNotEvaluableInThisBuild { step, .. }
        | ExecutionRefusal::CheckHasNoAnsweringRead { step, .. }
        | ExecutionRefusal::VerificationAssertsNothing { step }
        | ExecutionRefusal::Expectation { step, .. }
        | ExecutionRefusal::CommandOnStepThePlannerWritesNoneOn { step }
        | ExecutionRefusal::CommandIsNotThePlannersRead { step }
        | ExecutionRefusal::MutationNotInThisBuild { step }
        | ExecutionRefusal::ProviderConfiguration { step, .. }
        | ExecutionRefusal::MutationIsNotThePlannersCommand { step }
        | ExecutionRefusal::MutationNotVerifiedAfter { step } => *step,
        ExecutionRefusal::Placeholder { steps }
        | ExecutionRefusal::UnrecognisedCheck { steps }
        | ExecutionRefusal::UnknownSuccess { steps }
        | ExecutionRefusal::ElevationRequired { steps }
        | ExecutionRefusal::ElevationUnknown { steps }
        | ExecutionRefusal::MoreThanOneSnapshotBefore { steps }
        | ExecutionRefusal::MoreThanOneRecordHistory { steps } => {
            *steps.iter().min().expect("a step")
        }
    }
}

fn assert_refused(report: &GateReport) {
    assert!(!report.may_execute(), "{:?}", report.refusals());
    assert_eq!(report.exit_code(), 4, "any refusal is exit 4 (SE-009)");
}

// --- one per refusal -------------------------------------------------------

#[test]
fn an_ordering_violation_is_refused() {
    let mut plan = full_plan();
    assert!(!refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::Ordering
    )));
    let remove = position(&plan, StepKind::RemoveSource);
    let verify = position(&plan, StepKind::VerifyDestination);
    plan.steps.swap(remove, verify);
    assert!(plan.check_ordering().is_err());
    let report = execution_refusals(&plan);
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::Ordering
    )));
    assert_refused(&report);
}

#[test]
fn invariant_19_manual_review_refuses_execution_and_names_the_step() {
    let plan = plan_for(&both_held(), "postgresql@17", MP, false);
    let step = index_of(&plan, StepKind::ManualReview);
    let report = execution_refusals(&plan);
    assert!(
        refused(
            &report,
            |r| matches!(r, ExecutionRefusal::ManualReview { step: s } if *s == step)
        ),
        "{:?}",
        report.refusals()
    );
    assert_refused(&report);
    // Nothing outranks it: it is refused even with every other obligation met,
    // which this plan does, because a manual-review plan carries no commands.
    assert!(plan.steps.iter().all(|s| s.commands.is_empty()));
}

#[test]
fn a_placeholder_command_is_refused() {
    let mut plan = full_plan();
    let add = position(&plan, StepKind::AddDestination);
    plan.steps[add].commands[0].placeholder = true;
    let step = plan.steps[add].index;
    let report = execution_refusals(&plan);
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::Placeholder { steps } if steps == &vec![step]
    )));
    assert_refused(&report);
}

#[test]
fn an_unrecognised_check_is_refused() {
    let mut plan = full_plan();
    let removal = position(&plan, StepKind::VerifyRemoval);
    plan.steps[removal]
        .verification
        .push(VerificationCheck::Unknown);
    let step = plan.steps[removal].index;
    let report = execution_refusals(&plan);
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::UnrecognisedCheck { steps } if steps == &vec![step]
    )));
    // Refused as unrecognised, never also as "recognised but not evaluable".
    assert!(!refused(&report, |r| matches!(
        r,
        ExecutionRefusal::CheckNotEvaluableInThisBuild {
            check: VerificationCheck::Unknown,
            ..
        }
    )));
}

#[test]
fn a_recognised_check_is_not_thereby_evaluable() {
    let liveness = VerificationCheck::LivenessProbe {
        executable: "gsed".into(),
        flag: "--version".into(),
    };
    assert!(liveness.is_satisfiable(), "the domain recognises it");
    assert!(
        !evaluable_in_this_build(StepKind::VerifyDestination, &liveness),
        "this build cannot run it"
    );
    let dest = StepKind::VerifyDestination;
    let removal = StepKind::VerifyRemoval;
    for (kind, check, evaluable) in [
        // Pass 08 slice 4, D11: "installed" is not "active", so no read in
        // this build answers it, on any step.
        (dest, VerificationCheck::ProviderReportsActive, false),
        (dest, VerificationCheck::PackageNewlyPresent, true),
        (removal, VerificationCheck::PackageAbsent, true),
        // The evaluable checks, each on a step whose read does not answer it.
        (removal, VerificationCheck::ProviderReportsActive, false),
        (removal, VerificationCheck::PackageNewlyPresent, false),
        (dest, VerificationCheck::PackageAbsent, false),
        (
            StepKind::SetPrimary,
            VerificationCheck::ProviderReportsActive,
            false,
        ),
        (dest, VerificationCheck::ProviderIntegrityCheck, false),
        (
            dest,
            VerificationCheck::ExecutablesOwnedByDestination {
                executables: vec!["gsed".into()],
            },
            false,
        ),
        (dest, liveness.clone(), false),
        (dest, VerificationCheck::NoCrossProviderLinkage, false),
        (dest, VerificationCheck::Unknown, false),
    ] {
        assert_eq!(
            evaluable_in_this_build(kind, &check),
            evaluable,
            "{kind:?} {check:?}"
        );
    }

    let plan = full_plan();
    let step = index_of(&plan, StepKind::VerifyDestination);
    let report = execution_refusals(&plan);
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::CheckNotEvaluableInThisBuild { step: s, check }
            if *s == step && *check == liveness
    )));
    // The evaluable one on the same step is not refused.
    assert!(!refused(&report, |r| matches!(
        r,
        ExecutionRefusal::CheckNotEvaluableInThisBuild {
            step: s,
            check: VerificationCheck::PackageNewlyPresent,
        } if *s == step
    )));
}

#[test]
fn a_verification_step_that_asserts_nothing_is_refused() {
    let mut plan = full_plan();
    // As built: `VERIFY_RESOLUTION` carries neither a command nor a check.
    let resolution = index_of(&plan, StepKind::VerifyResolution);
    assert!(refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::VerificationAssertsNothing { step } if *step == resolution
    )));
    // A removal check with its command but without its check.
    let removal = position(&plan, StepKind::VerifyRemoval);
    let removal_index = plan.steps[removal].index;
    assert!(!refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::VerificationAssertsNothing { step } if *step == removal_index
    )));
    plan.steps[removal].verification.clear();
    assert!(refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::VerificationAssertsNothing { step } if *step == removal_index
    )));
    // And one with its check but without its command.
    let mut plan = full_plan();
    plan.steps[removal].commands.clear();
    let report = execution_refusals(&plan);
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::VerificationAssertsNothing { step } if *step == removal_index
    )));
    assert_refused(&report);
}

#[test]
fn a_command_of_unknown_success_is_refused() {
    let mut plan = full_plan();
    let add = position(&plan, StepKind::AddDestination);
    plan.steps[add].commands[0].success = SuccessSemantic::Unknown;
    let step = plan.steps[add].index;
    let report = execution_refusals(&plan);
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::UnknownSuccess { steps } if steps == &vec![step]
    )));
    assert_refused(&report);
}

#[test]
fn a_missing_destination_name_on_a_names_check_is_a_refusal() {
    let mut plan = full_plan();
    let verify = index_of(&plan, StepKind::VerifyDestination);
    assert!(!refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::Expectation { .. }
    )));
    plan.destination_package = None;
    let report = execution_refusals(&plan);
    assert!(
        refused(&report, |r| matches!(
            r,
            ExecutionRefusal::Expectation {
                step,
                error: ExpectationError::NoDestinationName
            } if *step == verify
        )),
        "{:?}",
        report.refusals()
    );
    assert_refused(&report);
}

#[test]
fn a_plan_on_one_provider_both_sides_is_refused() {
    let mut plan = full_plan();
    assert!(!refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::SameProviderOnBothSides
    )));
    plan.destination = plan.source.clone();
    let report = execution_refusals(&plan);
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::SameProviderOnBothSides
    )));
    assert_refused(&report);
}

#[test]
fn invariant_06_elevation_refuses_execution_and_never_rendering() {
    let mut plan = full_plan();
    let add = position(&plan, StepKind::AddDestination);
    let remove = position(&plan, StepKind::RemoveSource);
    let (add_index, remove_index) = (plan.steps[add].index, plan.steps[remove].index);
    plan.steps[add].commands[0].elevation = Elevation::Required {
        reason: "test".into(),
    };
    plan.steps[remove].commands[0].elevation = Elevation::Unknown;

    let report = execution_refusals(&plan);
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::ElevationRequired { steps } if steps == &vec![add_index]
    )));
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::ElevationUnknown { steps } if steps == &vec![remove_index]
    )));
    assert_refused(&report);

    // Rendering is not the gate's question: the plan is still well-formed data
    // that serialises and reads back unchanged, and the gate report is a
    // separate value that leaves it untouched.
    assert!(plan.check_ordering().is_ok());
    let json = serde_json::to_string(&plan).expect("an elevated plan serialises");
    let back: MigrationPlan = serde_json::from_str(&json).expect("and reads back");
    assert_eq!(back, plan);
}

#[test]
fn a_mutating_step_is_refused_in_this_build() {
    let plan = full_plan();
    let add = index_of(&plan, StepKind::AddDestination);
    let report = execution_refusals(&plan);
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::MutationNotInThisBuild { step } if *step == add
    )));
    // Where the destination already holds the package the add step carries no
    // command, and it is still the first mutating step: the refusal is keyed
    // on the kind, not on whether a command is attached.
    let plan = plan_for(&both_held(), "tool", MP, true);
    let add = position(&plan, StepKind::AddDestination);
    assert!(plan.steps[add].commands.is_empty());
    let add = plan.steps[add].index;
    assert!(refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::MutationNotInThisBuild { step } if *step == add
    )));
}

/// A mutating step with no command is still a mutation. Before the refusal was
/// keyed on the kind alone, `[SNAPSHOT_BEFORE, CLEANUP_REVIEW, RECORD_HISTORY]`
/// — bracketed, dry-run-only, commandless — passed the gate: nothing but this
/// refusal stands between it and execution.
#[test]
fn a_mutating_step_with_no_command_is_refused_in_this_build() {
    let mut plan = full_plan();
    plan.steps.retain(|s| {
        matches!(
            s.kind,
            StepKind::SnapshotBefore | StepKind::CleanupReview | StepKind::RecordHistory
        )
    });
    renumber(&mut plan);
    plan.elevated_steps = Vec::new();
    let kinds: Vec<StepKind> = plan.steps.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [
            StepKind::SnapshotBefore,
            StepKind::CleanupReview,
            StepKind::RecordHistory
        ]
    );
    assert!(plan.steps.iter().all(|s| s.commands.is_empty()));
    assert!(plan.dry_run_only);
    let report = execution_refusals(&plan);
    assert_refused(&report);
    assert_eq!(
        report.refusals(),
        [ExecutionRefusal::MutationNotInThisBuild { step: 2 }],
        "the only refusal of a bracketed, commandless mutating plan"
    );
}

#[test]
fn a_plan_that_claims_it_can_run_mutations_is_refused() {
    let mut plan = full_plan();
    assert!(
        plan.dry_run_only,
        "the planner writes what this build can do"
    );
    assert!(!refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::PlanNotDryRunOnly
    )));
    plan.dry_run_only = false;
    let report = execution_refusals(&plan);
    assert!(refused(&report, |r| matches!(
        r,
        ExecutionRefusal::PlanNotDryRunOnly
    )));
    assert_refused(&report);
}

/// Renumbers a plan's steps `1..=n` in list order, so that an edit which
/// removes or moves a step changes nothing but what it set out to change.
fn renumber(plan: &mut MigrationPlan) {
    for (step, index) in plan.steps.iter_mut().zip(1u32..) {
        step.index = index;
    }
}

#[test]
fn a_plan_of_another_schema_version_is_refused() {
    let base = full_plan();
    assert_eq!(base.schema_version, PLAN_SCHEMA_VERSION);
    assert!(!refused(&execution_refusals(&base), |r| matches!(
        r,
        ExecutionRefusal::SchemaVersionNotThisBuilds { .. }
    )));
    for found in [0, PLAN_SCHEMA_VERSION + 1] {
        let mut plan = base.clone();
        plan.schema_version = found;
        let report = execution_refusals(&plan);
        assert!(
            refused(&report, |r| r
                == &ExecutionRefusal::SchemaVersionNotThisBuilds {
                    found,
                    expected: PLAN_SCHEMA_VERSION,
                }),
            "{found}: {:?}",
            report.refusals()
        );
        assert_refused(&report);
        // A plan-level refusal, so it leads the report.
        assert!(matches!(
            report.refusals()[0],
            ExecutionRefusal::SchemaVersionNotThisBuilds { .. }
        ));
    }
    let mut plan = base;
    plan.schema_version = PLAN_SCHEMA_VERSION + 1;
    let json = serde_json::to_value(execution_refusals(&plan)).unwrap();
    assert!(json["refusals"].as_array().unwrap().iter().any(|r| {
        r["refusal"] == "SCHEMA_VERSION_NOT_THIS_BUILDS"
            && r["found"] == PLAN_SCHEMA_VERSION + 1
            && r["expected"] == PLAN_SCHEMA_VERSION
    }));
}

#[test]
fn a_mutation_with_no_snapshot_before_it_is_refused() {
    let base = full_plan();
    let is_refused = |plan: &MigrationPlan, step: u32| {
        let report = execution_refusals(plan);
        assert_refused(&report);
        refused(&report, |r| {
            matches!(
                r,
                ExecutionRefusal::NoSnapshotBeforeFirstMutation { step: s } if *s == step
            )
        })
    };
    assert_eq!(base.steps[0].kind, StepKind::SnapshotBefore);
    let first = base
        .steps
        .iter()
        .find(|s| s.kind.is_mutating())
        .expect("a mutating step")
        .index;
    assert!(!is_refused(&base, first));

    // The snapshot removed.
    let mut plan = base.clone();
    plan.steps.remove(0);
    renumber(&mut plan);
    let first = index_of(&plan, StepKind::AddDestination);
    assert!(is_refused(&plan, first), "{:?}", execution_refusals(&plan));

    // The snapshot moved after the first mutating step: present is not enough.
    let mut plan = base.clone();
    let add = position(&plan, StepKind::AddDestination);
    plan.steps.swap(0, add);
    renumber(&mut plan);
    assert_eq!(plan.steps[0].kind, StepKind::AddDestination);
    assert!(is_refused(&plan, 1));

    // A plan whose first mutating step is not `ADD_DESTINATION`: the one the
    // destination already holds, with the add step taken out as well.
    let mut plan = plan_for(&both_held(), "tool", MP, true);
    plan.steps
        .retain(|s| !matches!(s.kind, StepKind::SnapshotBefore | StepKind::AddDestination));
    renumber(&mut plan);
    let primary = index_of(&plan, StepKind::SetPrimary);
    assert!(is_refused(&plan, primary));
}

#[test]
fn a_mutating_plan_that_does_not_end_with_record_history_is_refused() {
    let base = full_plan();
    let is_refused = |plan: &MigrationPlan| {
        let report = execution_refusals(plan);
        assert_refused(&report);
        refused(&report, |r| {
            matches!(r, ExecutionRefusal::NoRecordHistoryAtEnd)
        })
    };
    assert_eq!(
        base.steps.last().map(|s| s.kind),
        Some(StepKind::RecordHistory)
    );
    assert!(!is_refused(&base));

    // The record removed.
    let mut plan = base.clone();
    plan.steps.pop();
    assert!(is_refused(&plan));

    // The record present, but a mutating step after it.
    let mut plan = base.clone();
    let last = plan.steps.len() - 1;
    plan.steps.swap(last - 1, last);
    renumber(&mut plan);
    assert_eq!(plan.steps[last].kind, StepKind::CleanupReview);
    assert!(is_refused(&plan));

    // A plan with no mutating step needs no bracket: the `MANUAL_REVIEW` stop
    // is refused as such and not for this.
    let stop = plan_for(&both_held(), "postgresql@17", MP, false);
    assert!(!stop.has_mutating_steps());
    let report = execution_refusals(&stop);
    assert!(!refused(&report, |r| matches!(
        r,
        ExecutionRefusal::NoRecordHistoryAtEnd
            | ExecutionRefusal::NoSnapshotBeforeFirstMutation { .. }
    )));

    // A mutating plan that ends at `MANUAL_REVIEW` is refused for both: the
    // stop, and a success path whose last step is not the record. The halt
    // does not stand in for the record (the executor writes one on a halt
    // anyway, obligation 12).
    let mut plan = base.clone();
    plan.steps.retain(|s| {
        matches!(
            s.kind,
            StepKind::SnapshotBefore | StepKind::AddDestination | StepKind::VerifyDestination
        )
    });
    let mut review = stop.steps[position(&stop, StepKind::ManualReview)].clone();
    review.index = 4;
    plan.steps.push(review);
    renumber(&mut plan);
    let kinds: Vec<StepKind> = plan.steps.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [
            StepKind::SnapshotBefore,
            StepKind::AddDestination,
            StepKind::VerifyDestination,
            StepKind::ManualReview
        ]
    );
    assert!(is_refused(&plan));
    assert!(refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::ManualReview { step: 4 }
    )));
}

#[test]
fn a_repeated_snapshot_or_history_record_is_refused() {
    let base = full_plan();
    let is_refused = |plan: &MigrationPlan, snapshot: bool, steps: &[u32]| {
        let report = execution_refusals(plan);
        assert_refused(&report);
        refused(&report, |r| match r {
            ExecutionRefusal::MoreThanOneSnapshotBefore { steps: s } => snapshot && s == steps,
            ExecutionRefusal::MoreThanOneRecordHistory { steps: s } => !snapshot && s == steps,
            _ => false,
        })
    };
    let report = execution_refusals(&base);
    assert!(!refused(&report, |r| matches!(
        r,
        ExecutionRefusal::MoreThanOneSnapshotBefore { .. }
            | ExecutionRefusal::MoreThanOneRecordHistory { .. }
    )));

    // A second snapshot right after the first. `check_ordering` reads only the
    // first, so it does not see this.
    let mut plan = base.clone();
    plan.steps.insert(1, plan.steps[0].clone());
    renumber(&mut plan);
    assert!(plan.check_ordering().is_ok());
    assert!(
        is_refused(&plan, true, &[1, 2]),
        "{:?}",
        execution_refusals(&plan)
    );

    // A second record, at the end: the plan still ends with one.
    let mut plan = base.clone();
    let last = plan.steps.last().expect("a step").clone();
    plan.steps.push(last);
    renumber(&mut plan);
    let n = u32::try_from(plan.steps.len()).expect("small");
    assert!(plan.check_ordering().is_ok());
    assert!(is_refused(&plan, false, &[n - 1, n]));
    assert!(!refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::NoRecordHistoryAtEnd
    )));

    // On a plan with no mutating step too: the smallest plan the gate
    // accepts, given two records.
    let mut plan = base.clone();
    plan.steps.retain(|s| {
        matches!(
            s.kind,
            StepKind::VerifyDestination | StepKind::RecordHistory
        )
    });
    plan.steps[0].verification = vec![VerificationCheck::PackageNewlyPresent];
    let record = plan.steps[1].clone();
    plan.steps.push(record);
    renumber(&mut plan);
    plan.risks = Vec::new();
    plan.elevated_steps = Vec::new();
    assert!(!plan.has_mutating_steps());
    let report = execution_refusals(&plan);
    assert_eq!(
        report.refusals(),
        [ExecutionRefusal::MoreThanOneRecordHistory { steps: vec![2, 3] }]
    );
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(
        json["refusals"][0]["refusal"],
        "MORE_THAN_ONE_RECORD_HISTORY"
    );
}

#[test]
fn a_plan_with_no_steps_is_refused() {
    let base = full_plan();
    let plan = MigrationPlan {
        steps: Vec::new(),
        risks: Vec::new(),
        elevated_steps: Vec::new(),
        ..base
    };
    let report = execution_refusals(&plan);
    assert_refused(&report);
    assert_eq!(
        report.refusals(),
        [ExecutionRefusal::EmptyPlan],
        "nothing else is wrong with it"
    );
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        serde_json::json!({ "refusals": [{ "refusal": "EMPTY_PLAN" }] })
    );
}

// --- the gate as a whole ---------------------------------------------------

#[test]
fn every_plan_this_planner_builds_is_refused_for_execution_in_this_build() {
    let mut plans = Vec::new();
    for source in [HB, MP] {
        let (snap, package, to) = install_needed(source);
        for retain in [false, true] {
            plans.push(plan_for(&snap, package, to, retain));
        }
    }
    for to in [HB, MP] {
        for retain in [false, true] {
            plans.push(plan_for(&both_held(), "tool", to, retain));
        }
    }
    plans.push(plan_for(&both_held(), "postgresql@17", MP, false));
    let directions: BTreeSet<_> = plans
        .iter()
        .map(|p| (p.source.clone(), p.destination.clone()))
        .collect();
    assert_eq!(directions.len(), 2, "both directions are covered (INV-17)");
    for plan in &plans {
        let report = execution_refusals(plan);
        assert_refused(&report);
        if plan.requires_manual_review() {
            // A plan stopping at `MANUAL_REVIEW` carries no mutating step; it is
            // refused as such, and not as a mutation.
            assert!(!plan.has_mutating_steps());
            assert!(refused(&report, |r| matches!(
                r,
                ExecutionRefusal::ManualReview { .. }
            )));
        } else {
            assert!(
                refused(&report, |r| matches!(
                    r,
                    ExecutionRefusal::MutationNotInThisBuild { .. }
                )),
                "a plan that would change the machine names the first step that would"
            );
        }
        assert_planners_commands_pass_the_command_rules(&report);
    }
}

/// The two command rules refuse forgeries, and the schema-version and
/// bracketing rules refuse plans of another build's shape — never what the
/// planner wrote.
fn assert_planners_commands_pass_the_command_rules(report: &GateReport) {
    assert!(
        !refused(report, |r| matches!(
            r,
            ExecutionRefusal::CommandOnStepThePlannerWritesNoneOn { .. }
                | ExecutionRefusal::CommandIsNotThePlannersRead { .. }
                | ExecutionRefusal::SchemaVersionNotThisBuilds { .. }
                | ExecutionRefusal::NoSnapshotBeforeFirstMutation { .. }
                | ExecutionRefusal::NoRecordHistoryAtEnd
                | ExecutionRefusal::EmptyPlan
                | ExecutionRefusal::MoreThanOneSnapshotBefore { .. }
                | ExecutionRefusal::MoreThanOneRecordHistory { .. }
        )),
        "{:?}",
        report.refusals()
    );
}

#[test]
fn a_command_on_a_step_the_planner_writes_none_on_is_refused() {
    let base = full_plan();
    assert_planners_commands_pass_the_command_rules(&execution_refusals(&base));
    // The read the plan is allowed to carry, moved onto each step that may
    // carry none: a harmless argv, so only the step kind can refuse it.
    let read = base.steps[position(&base, StepKind::VerifyRemoval)].commands[0].clone();
    for kind in [
        StepKind::SnapshotBefore,
        StepKind::VerifyResolution,
        StepKind::CleanupReview,
        StepKind::RecordHistory,
    ] {
        let mut plan = base.clone();
        let at = position(&plan, kind);
        plan.steps[at].commands.push(read.clone());
        let step = plan.steps[at].index;
        let report = execution_refusals(&plan);
        assert!(
            refused(&report, |r| matches!(
                r,
                ExecutionRefusal::CommandOnStepThePlannerWritesNoneOn { step: s } if *s == step
            )),
            "{kind:?}: {:?}",
            report.refusals()
        );
        assert_refused(&report);
    }
    let mut plan = plan_for(&both_held(), "postgresql@17", MP, false);
    let at = position(&plan, StepKind::ManualReview);
    plan.steps[at].commands.push(read);
    let step = plan.steps[at].index;
    assert!(refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::CommandOnStepThePlannerWritesNoneOn { step: s } if *s == step
    )));
}

#[test]
fn a_forged_verification_command_is_refused() {
    let base = full_plan();
    let add = position(&base, StepKind::AddDestination);
    let remove = position(&base, StepKind::RemoveSource);
    let verify = position(&base, StepKind::VerifyDestination);
    let removal = position(&base, StepKind::VerifyRemoval);
    let refused_as_forgery = |plan: &MigrationPlan, at: usize| {
        let step = plan.steps[at].index;
        let report = execution_refusals(plan);
        assert_refused(&report);
        refused(&report, |r| {
            matches!(
                r,
                ExecutionRefusal::CommandIsNotThePlannersRead { step: s } if *s == step
            )
        })
    };
    assert!(!refused_as_forgery(&base, verify));
    assert!(!refused_as_forgery(&base, removal));

    // A mutating verb carried by a verification step, each way round.
    let mut plan = base.clone();
    plan.steps[verify].commands[0].argv = base.steps[add].commands[0].argv.clone();
    assert!(
        refused_as_forgery(&plan, verify),
        "an add dressed as a read"
    );
    let mut plan = base.clone();
    plan.steps[removal].commands[0].argv = base.steps[remove].commands[0].argv.clone();
    assert!(
        refused_as_forgery(&plan, removal),
        "a removal dressed as a read"
    );

    // The read with one argument too many, and the inventory read naming a
    // package — both one element away from the planner's.
    let mut plan = base.clone();
    plan.steps[verify].commands[0].argv.push("extra".into());
    assert!(refused_as_forgery(&plan, verify));
    let mut plan = base.clone();
    let name = plan.package.name.clone();
    plan.steps[removal].commands[0].argv.push(name);
    assert!(refused_as_forgery(&plan, removal));

    // The planner's read with its semantic swapped: on a MacPorts read,
    // `EXIT_STATUS` is the fail-open one, because `port -q installed` exits 0
    // for an absent port. `expectation_for` alone accepts it.
    let mut plan = base.clone();
    assert_eq!(plan.destination, ProviderId::new(MP));
    plan.steps[verify].commands[0].success = SuccessSemantic::ExitStatus;
    assert!(macportsbrew_domain::expectation_for(&plan, verify, 0).is_ok());
    assert!(
        refused_as_forgery(&plan, verify),
        "a semantic the table does not state"
    );

    // A second command beside the genuine one.
    let mut plan = base.clone();
    let forged = base.steps[remove].commands[0].clone();
    plan.steps[removal].commands.push(forged);
    assert!(refused_as_forgery(&plan, removal));

    // A provider the table does not hold derives only an illustration, and an
    // illustration is never the planner's read, even written out exactly.
    let mut plan = base.clone();
    plan.destination = ProviderId::new("elsewhere");
    let name = plan
        .destination_package
        .clone()
        .expect("a destination name");
    plan.steps[verify].commands[0].argv = vec!["<verify>".into(), name];
    // The semantic too, set to what the absent entry derives, so that only
    // the illustration itself can make this a forgery.
    plan.steps[verify].commands[0].success = SuccessSemantic::Unknown;
    assert!(refused_as_forgery(&plan, verify));
    // And a plan with no destination name derives no read at all.
    let mut plan = base;
    plan.destination_package = None;
    assert!(refused_as_forgery(&plan, verify));
}

#[test]
fn refusals_are_ordered_deterministically() {
    let mut plan = full_plan();
    plan.dry_run_only = false;
    plan.destination_package = None;
    let removal = position(&plan, StepKind::VerifyRemoval);
    plan.steps[removal]
        .verification
        .push(VerificationCheck::Unknown);
    let add = position(&plan, StepKind::AddDestination);
    plan.steps[add].commands[0].elevation = Elevation::Unknown;

    let first = execution_refusals(&plan);
    assert_eq!(first, execution_refusals(&plan), "same plan, same report");
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&execution_refusals(&plan.clone())).unwrap(),
        "and the same bytes (INV-15)"
    );
    let steps: Vec<u32> = first.refusals().iter().map(step_of).collect();
    let mut sorted = steps.clone();
    sorted.sort_unstable();
    assert_eq!(steps, sorted, "ordered by the step each refusal names");
    // Plan-level refusals lead, in declaration order.
    assert!(matches!(
        first.refusals()[0],
        ExecutionRefusal::SameProviderOnBothSides | ExecutionRefusal::PlanNotDryRunOnly
    ));
    // Within one step, by kind in declaration order: on `VERIFY_DESTINATION`
    // every not-evaluable check precedes the expectation refusal.
    let verify = index_of(&plan, StepKind::VerifyDestination);
    let on_verify: Vec<&ExecutionRefusal> = first
        .refusals()
        .iter()
        .filter(|r| step_of(r) == verify)
        .collect();
    let last_check = on_verify
        .iter()
        .rposition(|r| matches!(r, ExecutionRefusal::CheckNotEvaluableInThisBuild { .. }))
        .expect("the destination carries checks this build cannot evaluate");
    let expectation = on_verify
        .iter()
        .position(|r| matches!(r, ExecutionRefusal::Expectation { .. }))
        .expect("and no destination name");
    assert!(last_check < expectation, "{on_verify:?}");
}

#[test]
fn a_refusal_serialises_under_a_stable_tag() {
    let mut plan = full_plan();
    plan.dry_run_only = false;
    let json = serde_json::to_value(execution_refusals(&plan)).unwrap();
    let refusals = json["refusals"].as_array().expect("a list of refusals");
    assert!(refusals
        .iter()
        .any(|r| r["refusal"] == "PLAN_NOT_DRY_RUN_ONLY"));
    assert!(refusals
        .iter()
        .any(|r| r["refusal"] == "VERIFICATION_ASSERTS_NOTHING" && r["step"].is_u64()));
}

/// `THIS_BUILD_DISPATCHES_MUTATIONS` is read by the planner, which writes it
/// into `dryRunOnly`, and by the gate — nowhere else (A11). A dispatch guard
/// that read it would let a flipped constant reach a provider.
#[test]
fn the_mutation_constant_is_read_only_by_the_planner_and_the_gate() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .to_path_buf();
    let allowed = [
        "domain/src/plan.rs",
        "application/src/planner.rs",
        "application/src/execution_gate.rs",
    ];
    let mut readers = Vec::new();
    let mut stack = vec![crates.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable") {
            let path = entry.expect("entry").path();
            let rel = path.strip_prefix(&crates).expect("under crates/");
            if path.is_dir() {
                if rel
                    .components()
                    .nth(1)
                    .is_none_or(|c| c.as_os_str() == "src")
                {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "rs")
                && std::fs::read_to_string(&path)
                    .expect("utf-8 source")
                    .contains("THIS_BUILD_DISPATCHES_MUTATIONS")
            {
                readers.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    readers.sort();
    for reader in &readers {
        assert!(
            allowed.contains(&reader.as_str()),
            "{reader} reads THIS_BUILD_DISPATCHES_MUTATIONS"
        );
    }
    assert_eq!(readers.len(), allowed.len(), "{readers:?}");
    // With the constant `false`, a planner writing a literal `true` builds the
    // same plans, so only the text can tell the two apart — and the day the
    // constant flips, only the derived field stays truthful.
    let planner = std::fs::read_to_string(crates.join("application/src/planner.rs"))
        .expect("the planner source");
    assert!(
        planner.contains("dry_run_only: !THIS_BUILD_DISPATCHES_MUTATIONS,"),
        "the planner derives dryRunOnly from the constant"
    );
}

/// **The switch, pinned** (pass 08 slice 4; work order §2). Flipping it is
/// the owner's decision on a proposal slice 6 writes, after every item the flip
/// tripwires name is delivered; the change that flips it edits this test in
/// the same commit, in plain sight. It also pins that this build grants no
/// mutation allowance to product code.
#[test]
fn this_build_does_not_dispatch_mutations() {
    let switch = macportsbrew_domain::plan::THIS_BUILD_DISPATCHES_MUTATIONS;
    assert!(!switch, "this build does not dispatch mutations");
    assert!(macportsbrew_application::mutation_allowance().is_none());
}

/// Step numbers are plan data, so a refusal naming several may list them in
/// any order; it sorts by the smallest it names, not by whichever came first.
#[test]
fn a_refusal_naming_several_steps_sorts_by_the_smallest() {
    let mut plan = full_plan();
    let add = position(&plan, StepKind::AddDestination);
    let remove = position(&plan, StepKind::RemoveSource);
    plan.steps[add].commands[0].elevation = Elevation::Unknown;
    plan.steps[remove].commands[0].elevation = Elevation::Unknown;
    let (a, r) = (plan.steps[add].index, plan.steps[remove].index);
    plan.steps[add].index = r;
    plan.steps[remove].index = a;
    let report = execution_refusals(&plan);
    assert!(refused(&report, |x| matches!(
        x,
        ExecutionRefusal::ElevationUnknown { steps } if steps == &vec![r, a]
    )));
    let steps: Vec<u32> = report.refusals().iter().map(step_of).collect();
    let mut sorted = steps.clone();
    sorted.sort_unstable();
    assert_eq!(steps, sorted, "{:?}", report.refusals());
}

/// A check is answered by its own step's read, so the same check on a step
/// that reads nothing cannot be evaluated there.
#[test]
fn a_check_on_a_step_with_no_provider_read_is_not_evaluable() {
    let plan = full_plan();
    let primary = position(&plan, StepKind::SetPrimary);
    assert_eq!(
        plan.steps[primary].verification,
        vec![VerificationCheck::ProviderReportsActive],
        "the planner puts this check on SET_PRIMARY"
    );
    let step = plan.steps[primary].index;
    assert!(refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::CheckNotEvaluableInThisBuild {
            step: s,
            check: VerificationCheck::ProviderReportsActive,
        } if *s == step
    )));
}

#[test]
fn step_indices_that_are_not_one_to_n_in_order_are_refused() {
    let base = full_plan();
    let is_refused = |plan: &MigrationPlan| {
        refused(&execution_refusals(plan), |r| {
            matches!(r, ExecutionRefusal::StepIndicesNotSequential)
        })
    };
    assert!(!is_refused(&base));
    let mut repeated = base.clone();
    repeated.steps[1].index = repeated.steps[0].index;
    assert!(is_refused(&repeated), "a repeated index");
    let mut skipped = base.clone();
    let last = skipped.steps.len() - 1;
    skipped.steps[last].index += 1;
    assert!(is_refused(&skipped), "a skipped index");
    let mut zero = base.clone();
    for step in &mut zero.steps {
        step.index -= 1;
    }
    assert!(is_refused(&zero), "counted from 0");
    let mut swapped = base;
    let (a, b) = (swapped.steps[1].index, swapped.steps[2].index);
    swapped.steps[1].index = b;
    swapped.steps[2].index = a;
    assert!(is_refused(&swapped), "out of order");
}

/// The smallest plan this gate lets through, pinned so that any widening of
/// what it accepts shows up in a diff: one `VERIFY_DESTINATION`, the
/// planner's own read toward a detected destination, checked only by
/// `PACKAGE_NEWLY_PRESENT` (`PROVIDER_REPORTS_ACTIVE` until pass 08 slice 4,
/// D11), nothing mutating, nothing elevating. With no
/// mutating step it needs neither bracket of obligation 11, so it carries no
/// `SNAPSHOT_BEFORE` and does not end with `RECORD_HISTORY`.
#[test]
fn the_minimal_plan_the_gate_accepts() {
    let base = full_plan();
    let mut step = base.steps[position(&base, StepKind::VerifyDestination)].clone();
    step.index = 1;
    step.verification = vec![VerificationCheck::PackageNewlyPresent];
    let plan = MigrationPlan {
        steps: vec![step],
        risks: Vec::new(),
        elevated_steps: Vec::new(),
        ..base
    };
    assert_eq!(plan.steps[0].commands.len(), 1);
    assert_eq!(plan.steps[0].commands[0].elevation, Elevation::None);
    let report = execution_refusals(&plan);
    assert!(report.may_execute(), "{:?}", report.refusals());
    assert_eq!(report.exit_code(), 0);
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        serde_json::json!({ "refusals": [] })
    );
}

// --- DP-027 (b): a provider's own configuration ------------------------------

/// The risk the planner writes when a provider's configuration overrides how
/// this tool runs it (`planner::PROVIDER_CONFIGURATION_RISK`).
fn configuration_risk(provider: &str) -> macportsbrew_domain::PlanRisk {
    macportsbrew_domain::PlanRisk {
        id: macportsbrew_application::planner::PROVIDER_CONFIGURATION_RISK.into(),
        reason: macportsbrew_domain::Reason::new("reason.provider_configuration_outranks")
            .with("provider", &provider)
            .with("file", &"~/.homebrew/brew.env")
            .with("keys", &"HOMEBREW_NO_ANALYTICS"),
        blocking: false,
    }
}

fn configuration_refusals(report: &GateReport) -> Vec<(u32, String)> {
    report
        .refusals()
        .iter()
        .filter_map(|r| match r {
            ExecutionRefusal::ProviderConfiguration { step, provider } => {
                Some((*step, provider.to_string()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_plan_with_no_configuration_risk_draws_no_configuration_refusal() {
    assert!(configuration_refusals(&execution_refusals(&full_plan())).is_empty());
}

#[test]
fn a_step_that_changes_a_provider_whose_configuration_overrides_this_tool_is_refused() {
    // The destination's configuration: every mutating step naming it is
    // refused, and no step naming only the source.
    let mut plan = full_plan();
    let to = plan.destination.to_string();
    plan.risks.push(configuration_risk(&to));
    let report = execution_refusals(&plan);
    assert_refused(&report);
    let refused = configuration_refusals(&report);
    let expected: Vec<(u32, String)> = plan
        .steps
        .iter()
        .filter(|s| s.kind.is_mutating())
        .filter(|s| s.provider.as_ref().is_none_or(|p| p.to_string() == to))
        .map(|s| (s.index, to.clone()))
        .collect();
    assert!(!expected.is_empty(), "the plan changes the destination");
    assert_eq!(refused, expected);
    assert!(refused
        .iter()
        .any(|(step, _)| *step == index_of(&plan, StepKind::AddDestination)));
}

#[test]
fn invariant_17_the_source_side_is_refused_the_same_way() {
    let mut plan = full_plan();
    let from = plan.source.to_string();
    plan.risks.push(configuration_risk(&from));
    let refused = configuration_refusals(&execution_refusals(&plan));
    assert!(
        refused
            .iter()
            .any(|(step, p)| *step == index_of(&plan, StepKind::RemoveSource) && *p == from),
        "{refused:?}"
    );
    assert!(
        !refused
            .iter()
            .any(|(step, _)| *step == index_of(&plan, StepKind::AddDestination)),
        "the destination's step is not the source's: {refused:?}"
    );
}

#[test]
fn a_configuration_risk_on_a_provider_the_plan_does_not_change_refuses_nothing_of_its_own() {
    let mut plan = full_plan();
    plan.risks.push(configuration_risk("pkgsrc"));
    let refused = configuration_refusals(&execution_refusals(&plan));
    // Only a mutating step with no provider named is refused for it.
    assert!(
        refused.iter().all(|(step, _)| plan
            .steps
            .iter()
            .any(|s| s.index == *step && s.provider.is_none())),
        "{refused:?}"
    );
}

#[test]
fn an_unknown_configuration_risk_is_refused_like_a_found_one() {
    let mut plan = full_plan();
    let to = plan.destination.to_string();
    plan.risks.push(macportsbrew_domain::PlanRisk {
        id: macportsbrew_application::planner::PROVIDER_CONFIGURATION_RISK.into(),
        reason: macportsbrew_domain::Reason::new("reason.provider_configuration_unknown")
            .with("provider", &to),
        blocking: false,
    });
    let refused = configuration_refusals(&execution_refusals(&plan));
    assert!(
        refused
            .iter()
            .any(|(step, p)| *step == index_of(&plan, StepKind::AddDestination) && *p == to),
        "{refused:?}"
    );
}
// --- work order 16 part S ------------------------------------------------------

/// The verification step's commands refused as not the planner's, or not.
fn refused_as_not_the_planners(plan: &MigrationPlan, at: usize) -> bool {
    let step = plan.steps[at].index;
    refused(&execution_refusals(plan), |r| {
        matches!(
            r,
            ExecutionRefusal::CommandIsNotThePlannersRead { step: s } if *s == step
        )
    })
}

#[test]
fn planner_and_gate_share_one_verify_destination_rule() {
    // V2: the planner's VERIFY_DESTINATION carries exactly what
    // `verify_destination_reads` derives, and the gate's pin compares by
    // position and in number. With the shipped table the list is `[verify]`.
    let table = macportsbrew_application::provider_commands::load().expect("the table loads");
    for source in [HB, MP] {
        let (snap, package, to) = install_needed(source);
        let plan = plan_for(&snap, package, to, false);
        let verify = position(&plan, StepKind::VerifyDestination);
        let name = plan
            .destination_name()
            .expect("a destination")
            .as_str()
            .to_string();
        let derived = macportsbrew_application::provider_commands::verify_destination_reads(
            &table,
            plan.destination.as_str(),
            &name,
        );
        assert_eq!(derived.len(), 1, "{source}: S-1 is inert");
        assert_eq!(derived[0].0, "verify", "{source}");
        let carried: Vec<(Vec<String>, SuccessSemantic)> = plan.steps[verify]
            .commands
            .iter()
            .map(|c| (c.argv.clone(), c.success))
            .collect();
        let expected: Vec<(Vec<String>, SuccessSemantic)> = derived
            .into_iter()
            .map(|(_, argv, success)| (argv, success))
            .collect();
        assert_eq!(carried, expected, "{source}");
        assert!(!refused_as_not_the_planners(&plan, verify), "{source}");

        // An extra command after it — the planner's own, repeated: the length
        // differs, so a zip that stops at the shorter list would let it by.
        let mut extra = plan.clone();
        let first = extra.steps[verify].commands[0].clone();
        extra.steps[verify].commands.push(first.clone());
        assert!(
            refused_as_not_the_planners(&extra, verify),
            "{source}: extra"
        );

        // A foreign read placed first, the planner's second: reordered.
        let mut reordered = plan.clone();
        let mut other = first.clone();
        other.argv.push("--another".into());
        reordered.steps[verify].commands.insert(0, other.clone());
        assert!(
            refused_as_not_the_planners(&reordered, verify),
            "{source}: reordered"
        );

        // The planner's read swapped for another one: same length, wrong cell.
        let mut swapped = plan.clone();
        swapped.steps[verify].commands[0] = other;
        assert!(
            refused_as_not_the_planners(&swapped, verify),
            "{source}: swapped"
        );

        // Its semantic swapped alone.
        let mut semantic = plan.clone();
        semantic.steps[verify].commands[0].success = SuccessSemantic::StdoutMarksPackageActive;
        assert!(
            refused_as_not_the_planners(&semantic, verify),
            "{source}: semantic"
        );
    }
}

#[test]
fn a_check_evaluable_but_unanswered_by_the_steps_reads_is_refused() {
    // M2b: evaluable means evaluable by this plan. A removal check whose read
    // cannot answer absence is refused at the gate, by name, not halted later.
    let mut plan = full_plan();
    let removal = position(&plan, StepKind::VerifyRemoval);
    let step = plan.steps[removal].index;
    assert!(!refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::CheckHasNoAnsweringRead { .. }
    )));
    for command in &mut plan.steps[removal].commands {
        command.success = SuccessSemantic::StdoutNamesPackage;
    }
    let report = execution_refusals(&plan);
    assert!(
        refused(&report, |r| matches!(
            r,
            ExecutionRefusal::CheckHasNoAnsweringRead { step: s, check: VerificationCheck::PackageAbsent }
                if *s == step
        )),
        "{:?}",
        report.refusals()
    );
    assert_refused(&report);

    // The same on a destination: presence answered by nothing.
    let mut plan = full_plan();
    let verify = position(&plan, StepKind::VerifyDestination);
    let step = plan.steps[verify].index;
    for command in &mut plan.steps[verify].commands {
        command.success = SuccessSemantic::StdoutOmitsPackage;
    }
    assert!(refused(&execution_refusals(&plan), |r| matches!(
        r,
        ExecutionRefusal::CheckHasNoAnsweringRead { step: s, check: VerificationCheck::PackageNewlyPresent }
            if *s == step
    )));
}

#[test]
fn the_planner_step_sequence_is_unchanged() {
    // INV-01's ordering: part S changes which commands a VERIFY_DESTINATION
    // carries, never where it sits.
    use StepKind::*;
    for source in [HB, MP] {
        let (snap, package, to) = install_needed(source);
        let plan = plan_for(&snap, package, to, false);
        let kinds: Vec<StepKind> = plan.steps.iter().map(|s| s.kind).collect();
        assert_eq!(
            kinds,
            [
                SnapshotBefore,
                AddDestination,
                VerifyDestination,
                SetPrimary,
                VerifyResolution,
                RemoveSource,
                VerifyRemoval,
                CleanupReview,
                RecordHistory,
            ],
            "{source}"
        );
        assert!(plan.check_ordering().is_ok(), "{source}");
    }
}
