// SPDX-License-Identifier: Apache-2.0

//! The run loop's decisions (`crates/application/src/executor.rs`): the
//! dispatch guard, the `MANUAL_REVIEW` halt, step judgement and the run
//! outcome. Pure functions over plans the real planner builds, edited only to
//! change one thing, and reads synthesised in memory — nothing here runs a
//! process. The I/O shell's own tests, over recordings and the real read
//! sites, are in `crates/cli/tests/executor.rs`.

use macportsbrew_application::{
    build_plan, execution_refusals, next_action, outcome_of, Action, AnalysisOptions, CheckKind,
    Disposition, HaltReason, PlanRequest, RunOutcome, StepRecord, Unevaluable,
    PARTIAL_SUCCESS_EXIT_CODE,
};
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, Capability,
    CapabilityState, CommandLineTools, Elevation, EnvironmentSnapshot, ErrorId, FailureCause,
    InstalledPackage, Machine, MacosVersion, MigrationPlan, PackageIdentity, PackageKind,
    PackageMetadata, Provenance, ProviderCapabilities, ProviderDetection, ProviderId,
    ProviderInventory, ProviderPresence, ProviderSnapshot, RequestedSet, Snapshot, StepKind,
    Translation, VerificationCheck, Version, SNAPSHOT_SCHEMA_VERSION,
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

/// `tool` held by both sides, and the stateful `postgresql@17`.
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
            today: "2026-09-30".into(),
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

/// Every plan the planner builds over these snapshots.
fn planner_plans() -> Vec<MigrationPlan> {
    let mut plans = Vec::new();
    for source in [HB, MP] {
        let (snap, package, to) = install_needed(source);
        for retain in [false, true] {
            plans.push(plan_for(&snap, package, to, retain));
        }
    }
    let both = both_held();
    plans.push(plan_for(&both, "tool", MP, false));
    plans.push(plan_for(&both, "tool", HB, false));
    plans.push(plan_for(&both, "postgresql@17", MP, false));
    plans
}

/// The verification part of a planner plan, renumbered: what the gate passes
/// in this build. `VERIFY_DESTINATION` keeps only the checks this build can
/// evaluate; the bracketing steps stay, so a run of it skips them.
fn read_only(plan: &MigrationPlan) -> MigrationPlan {
    let mut out = plan.clone();
    out.steps.retain(|s| {
        matches!(
            s.kind,
            StepKind::SnapshotBefore
                | StepKind::VerifyDestination
                | StepKind::VerifyRemoval
                | StepKind::RecordHistory
        )
    });
    for (i, step) in out.steps.iter_mut().enumerate() {
        step.index = u32::try_from(i).expect("small") + 1;
        let kind = step.kind;
        step.verification
            .retain(|c| macportsbrew_application::evaluable_in_this_build(kind, c));
    }
    out.elevated_steps.clear();
    out
}

fn position(plan: &MigrationPlan, kind: StepKind) -> usize {
    plan.steps
        .iter()
        .position(|s| s.kind == kind)
        .unwrap_or_else(|| panic!("the plan carries a {kind:?} step"))
}

fn record(plan: &MigrationPlan, position: usize, disposition: Disposition) -> StepRecord {
    StepRecord {
        index: plan.steps[position].index,
        kind: plan.steps[position].kind,
        disposition,
    }
}

/// Drives `next_action` over a plan with every dispatched step judged as
/// verified, and returns the positions it would have dispatched.
fn dispatched_positions(plan: &MigrationPlan) -> Vec<usize> {
    let mut records = Vec::new();
    let mut dispatched = Vec::new();
    for position in 0..plan.steps.len() {
        let disposition = match next_action(plan, position, &records) {
            Action::Dispatch => {
                dispatched.push(position);
                Disposition::Verified {
                    evidence: Vec::new(),
                }
            }
            Action::PassOver(d) => d,
            Action::Halt(reason) => {
                records.push(record(
                    plan,
                    position,
                    Disposition::Halted {
                        reason,
                        evidence: Vec::new(),
                    },
                ));
                break;
            }
        };
        records.push(record(plan, position, disposition));
    }
    dispatched
}

/// The plan without its `MANUAL_REVIEW` steps, renumbered, so that nothing
/// but the dispatch guard stands between a mutating step and a dispatch.
fn without_review(plan: &MigrationPlan) -> MigrationPlan {
    let mut probe = plan.clone();
    probe.steps.retain(|s| s.kind != StepKind::ManualReview);
    for (i, s) in probe.steps.iter_mut().enumerate() {
        s.index = u32::try_from(i).expect("small") + 1;
    }
    probe
}

fn verified_up_to(plan: &MigrationPlan, position: usize) -> Vec<StepRecord> {
    (0..position)
        .map(|p| {
            record(
                plan,
                p,
                Disposition::Verified {
                    evidence: Vec::new(),
                },
            )
        })
        .collect()
}

/// Parametrised over every mutating `StepKind` (A9): with the earlier steps
/// recorded as verified, each mutating step of every plan the planner builds
/// is asked about with its commands and without them.
#[test]
fn invariant_01_the_loop_never_dispatches_a_mutating_step() {
    let plans = planner_plans();
    let a_read = plans
        .iter()
        .flat_map(|q| &q.steps)
        .find(|s| s.kind == StepKind::VerifyDestination)
        .expect("a read to borrow")
        .commands
        .clone();
    let mut seen = BTreeSet::new();
    for plan in &plans {
        let probe = without_review(plan);
        for p in 0..probe.steps.len() {
            let kind = probe.steps[p].kind;
            if !kind.is_mutating() {
                continue;
            }
            let records = verified_up_to(&probe, p);
            let mut with = probe.clone();
            if with.steps[p].commands.is_empty() {
                with.steps[p].commands.clone_from(&a_read);
            }
            assert_eq!(
                next_action(&with, p, &records),
                Action::Halt(HaltReason::UnexecutedMutation),
                "{kind:?} with a command"
            );
            let mut without = probe.clone();
            without.steps[p].commands.clear();
            assert_eq!(
                next_action(&without, p, &records),
                Action::PassOver(Disposition::NotDispatchedMutating),
                "{kind:?} without one"
            );
            seen.insert(format!("{kind:?}"));
        }
    }
    let every: BTreeSet<String> = [
        StepKind::AddDestination,
        StepKind::SetPrimary,
        StepKind::RemoveSource,
        StepKind::CleanupReview,
    ]
    .iter()
    .map(|k| format!("{k:?}"))
    .collect();
    assert_eq!(seen, every, "every mutating kind was asked about");
}

#[test]
fn no_step_of_any_plan_this_planner_builds_is_dispatched_unless_it_is_a_read() {
    for plan in planner_plans() {
        for p in dispatched_positions(&plan) {
            assert!(
                matches!(
                    plan.steps[p].kind,
                    StepKind::VerifyDestination | StepKind::VerifyRemoval
                ),
                "{:?}",
                plan.steps[p].kind
            );
        }
    }
}

#[test]
fn invariant_19_manual_review_halts_before_any_dispatch() {
    let plan = plan_for(&both_held(), "postgresql@17", MP, false);
    let review = plan.steps[position(&plan, StepKind::ManualReview)].index;
    assert!(dispatched_positions(&plan).is_empty());
    // Whichever step is asked about, including one placed before the review.
    let mut ahead = read_only(&plan_for(&both_held(), "tool", MP, false));
    let mut stop = plan.steps[position(&plan, StepKind::ManualReview)].clone();
    stop.index = u32::try_from(ahead.steps.len()).expect("small") + 1;
    ahead.steps.push(stop);
    for position in 0..ahead.steps.len() {
        let records: Vec<StepRecord> = (0..position)
            .map(|p| {
                record(
                    &ahead,
                    p,
                    Disposition::Verified {
                        evidence: Vec::new(),
                    },
                )
            })
            .collect();
        assert!(
            matches!(
                next_action(&ahead, position, &records),
                Action::Halt(HaltReason::ManualReview { .. })
            ),
            "position {position}"
        );
    }
    assert_eq!(
        next_action(&plan, 0, &[]),
        Action::Halt(HaltReason::ManualReview { step: review })
    );
}

#[test]
fn the_bracketing_steps_are_not_performed_and_verify_resolution_cannot_pass() {
    let plan = plan_for(&install_needed(HB).0, "gnu-sed", MP, false);
    assert_eq!(
        next_action(&plan, 0, &[]),
        Action::PassOver(Disposition::NotPerformedInThisBuild)
    );
    let resolution = position(&plan, StepKind::VerifyResolution);
    let records: Vec<StepRecord> = (0..resolution)
        .map(|p| {
            record(
                &plan,
                p,
                Disposition::Verified {
                    evidence: Vec::new(),
                },
            )
        })
        .collect();
    assert_eq!(
        next_action(&plan, resolution, &records),
        Action::Halt(HaltReason::Unevaluable {
            check: None,
            why: Unevaluable::NoEvaluationForStepKind
        })
    );
    let last = plan.steps.len() - 1;
    assert_eq!(plan.steps[last].kind, StepKind::RecordHistory);
}

#[test]
fn a_verification_step_with_a_check_it_cannot_evaluate_is_not_dispatched() {
    let plan = plan_for(&install_needed(HB).0, "gnu-sed", MP, false);
    let mut ro = read_only(&plan);
    let verify = position(&ro, StepKind::VerifyDestination);
    ro.steps[verify]
        .verification
        .push(VerificationCheck::NoCrossProviderLinkage);
    let records: Vec<StepRecord> = (0..verify)
        .map(|p| record(&ro, p, Disposition::NotPerformedInThisBuild))
        .collect();
    assert_eq!(
        next_action(&ro, verify, &records),
        Action::Halt(HaltReason::Unevaluable {
            check: Some(CheckKind::NoCrossProviderLinkage),
            why: Unevaluable::CheckHasNoEvaluation
        })
    );
    ro.steps[verify].verification.clear();
    assert_eq!(
        next_action(&ro, verify, &records),
        Action::Halt(HaltReason::Unevaluable {
            check: None,
            why: Unevaluable::NoCheck
        })
    );
    let mut ro = read_only(&plan);
    ro.steps[verify].commands.clear();
    assert_eq!(
        next_action(&ro, verify, &records),
        Action::Halt(HaltReason::Unevaluable {
            check: None,
            why: Unevaluable::NoCommand
        })
    );
}

#[test]
fn records_that_do_not_match_the_plan_halt_the_run() {
    let plan = read_only(&full());
    // Asked for step 1 with nothing recorded for step 0.
    assert_eq!(
        next_action(&plan, 1, &[]),
        Action::Halt(HaltReason::RecordsDoNotMatchPlan)
    );
    // After a halt, nothing further is dispatched.
    let halted = vec![record(
        &plan,
        0,
        Disposition::Halted {
            reason: HaltReason::UnexecutedMutation,
            evidence: Vec::new(),
        },
    )];
    assert_eq!(
        next_action(&plan, 1, &halted),
        Action::Halt(HaltReason::RecordsDoNotMatchPlan)
    );
    // A record naming another step.
    let mut wrong = vec![record(&plan, 0, Disposition::NotPerformedInThisBuild)];
    wrong[0].index = 9;
    assert_eq!(
        next_action(&plan, 1, &wrong),
        Action::Halt(HaltReason::RecordsDoNotMatchPlan)
    );
    assert!(matches!(
        outcome_of(&plan, &wrong),
        RunOutcome::Halted {
            reason: HaltReason::RecordsDoNotMatchPlan,
            ..
        }
    ));
    assert!(matches!(
        outcome_of(&plan, &[]),
        RunOutcome::Halted {
            reason: HaltReason::RecordsDoNotMatchPlan,
            ..
        }
    ));
}

fn full() -> MigrationPlan {
    plan_for(&install_needed(HB).0, "gnu-sed", MP, false)
}

#[test]
fn the_read_only_plan_used_here_is_one_the_gate_passes_in_both_directions() {
    for source in [HB, MP] {
        let (snap, package, to) = install_needed(source);
        let ro = read_only(&plan_for(&snap, package, to, false));
        let report = execution_refusals(&ro);
        assert!(report.may_execute(), "{source}: {:?}", report.refusals());
    }
}

#[test]
fn invariant_03_partial_success_is_its_own_outcome_and_exit_code() {
    let plan = read_only(&full());
    let verify = position(&plan, StepKind::VerifyDestination);
    let removal = position(&plan, StepKind::VerifyRemoval);
    let mut records: Vec<StepRecord> = Vec::new();
    for p in 0..plan.steps.len() {
        let disposition = if p == verify {
            Disposition::Verified {
                evidence: Vec::new(),
            }
        } else if p == removal {
            Disposition::Halted {
                reason: HaltReason::VerificationFailed {
                    command: 0,
                    cause: FailureCause::NamePresent,
                },
                evidence: Vec::new(),
            }
        } else if p > removal {
            Disposition::NotReached
        } else {
            Disposition::NotPerformedInThisBuild
        };
        records.push(record(&plan, p, disposition));
    }
    let outcome = outcome_of(&plan, &records);
    assert_eq!(
        outcome,
        RunOutcome::PartialSuccess {
            step: plan.steps[removal].index,
            reason: HaltReason::VerificationFailed {
                command: 0,
                cause: FailureCause::NamePresent,
            },
        }
    );
    assert_eq!(outcome.exit_code(), PARTIAL_SUCCESS_EXIT_CODE);
    assert_eq!(PARTIAL_SUCCESS_EXIT_CODE, 3);
    assert_ne!(outcome, RunOutcome::Completed);
}

#[test]
fn a_halt_is_never_reported_as_completed() {
    let plan = read_only(&full());
    let reasons = [
        HaltReason::UnexecutedMutation,
        HaltReason::SiteRefused { command: 0 },
        HaltReason::VerificationFailed {
            command: 0,
            cause: FailureCause::DidNotFinish,
        },
        HaltReason::Unevaluable {
            check: None,
            why: Unevaluable::NoPassingRead,
        },
    ];
    for at in 0..plan.steps.len() {
        for reason in &reasons {
            for before in [
                Disposition::Verified {
                    evidence: Vec::new(),
                },
                Disposition::NotPerformedInThisBuild,
            ] {
                let records: Vec<StepRecord> = (0..plan.steps.len())
                    .map(|p| {
                        let d = match p.cmp(&at) {
                            std::cmp::Ordering::Less => before.clone(),
                            std::cmp::Ordering::Equal => Disposition::Halted {
                                reason: reason.clone(),
                                evidence: Vec::new(),
                            },
                            std::cmp::Ordering::Greater => Disposition::NotReached,
                        };
                        record(&plan, p, d)
                    })
                    .collect();
                let outcome = outcome_of(&plan, &records);
                assert_ne!(outcome, RunOutcome::Completed, "{reason:?} at {at}");
                assert_ne!(outcome.exit_code(), 0, "{reason:?} at {at}");
                let carried_out = at > 0 && matches!(before, Disposition::Verified { .. });
                if carried_out {
                    assert_eq!(
                        outcome,
                        RunOutcome::PartialSuccess {
                            step: plan.steps[at].index,
                            reason: reason.clone()
                        }
                    );
                } else {
                    assert_eq!(
                        outcome,
                        RunOutcome::Halted {
                            step: plan.steps[at].index,
                            reason: reason.clone()
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn a_run_that_skipped_its_snapshot_is_never_completed() {
    let plan = read_only(&full());
    assert_eq!(plan.steps[0].kind, StepKind::SnapshotBefore);
    let all_verified_but_brackets: Vec<StepRecord> = (0..plan.steps.len())
        .map(|p| {
            let d = match plan.steps[p].kind {
                StepKind::SnapshotBefore | StepKind::RecordHistory => {
                    Disposition::NotPerformedInThisBuild
                }
                _ => Disposition::Verified {
                    evidence: Vec::new(),
                },
            };
            record(&plan, p, d)
        })
        .collect();
    assert_eq!(
        outcome_of(&plan, &all_verified_but_brackets),
        RunOutcome::PartialSuccess {
            step: 1,
            reason: HaltReason::NotPerformedInThisBuild
        }
    );
    // A commandless mutating step passed over is not carried out either.
    let mut passed_over = all_verified_but_brackets.clone();
    passed_over[0].disposition = Disposition::NotDispatchedMutating;
    assert_ne!(outcome_of(&plan, &passed_over), RunOutcome::Completed);
    // Nothing carried out at all: halted, not partial.
    let nothing: Vec<StepRecord> = (0..plan.steps.len())
        .map(|p| record(&plan, p, Disposition::NotPerformedInThisBuild))
        .collect();
    assert_eq!(
        outcome_of(&plan, &nothing),
        RunOutcome::Halted {
            step: 1,
            reason: HaltReason::NotPerformedInThisBuild
        }
    );
    assert_eq!(outcome_of(&plan, &nothing).exit_code(), 4);
    // And only a run that carried out every step completes.
    let mut only_reads = plan.clone();
    only_reads
        .steps
        .retain(|s| !matches!(s.kind, StepKind::SnapshotBefore | StepKind::RecordHistory));
    for (i, s) in only_reads.steps.iter_mut().enumerate() {
        s.index = u32::try_from(i).expect("small") + 1;
    }
    let every: Vec<StepRecord> = (0..only_reads.steps.len())
        .map(|p| {
            record(
                &only_reads,
                p,
                Disposition::Verified {
                    evidence: Vec::new(),
                },
            )
        })
        .collect();
    assert_eq!(outcome_of(&only_reads, &every), RunOutcome::Completed);
}

#[test]
fn invariant_06_a_verification_read_is_never_dispatched_elevated() {
    // A read the plan says needs elevation, or whose elevation nobody
    // established, halts before dispatch rather than running unelevated:
    // fail closed (the gate refuses the same plan first).
    let plan = read_only(&full());
    let verify = position(&plan, StepKind::VerifyDestination);
    let records: Vec<StepRecord> = (0..verify)
        .map(|p| record(&plan, p, Disposition::NotPerformedInThisBuild))
        .collect();
    assert_eq!(next_action(&plan, verify, &records), Action::Dispatch);
    for elevation in [
        Elevation::Required { reason: "x".into() },
        Elevation::Unknown,
    ] {
        let mut elevated = plan.clone();
        elevated.steps[verify].commands[0].elevation = elevation.clone();
        assert_eq!(
            next_action(&elevated, verify, &records),
            Action::Halt(HaltReason::ElevationNotNone { command: 0 }),
            "{elevation:?}"
        );
        assert_eq!(
            HaltReason::ElevationNotNone { command: 0 }.error_id(),
            ErrorId::SafetyBlocked
        );
    }
}

/// Every `ErrorId`, by an exhaustive match: a variant added to the domain
/// fails to compile here until it is listed.
fn every_error_id() -> Vec<ErrorId> {
    let all = [
        ErrorId::Usage,
        ErrorId::ProviderNotFound,
        ErrorId::ProviderUnhealthy,
        ErrorId::ProviderOutputChanged,
        ErrorId::ProviderTimeout,
        ErrorId::PackageNotFound,
        ErrorId::CapabilityMissing,
        ErrorId::SnapshotUnusable,
        ErrorId::RecordingUnusable,
        ErrorId::InputUnusable,
        ErrorId::PolicyDataInvalid,
        ErrorId::SafetyBlocked,
        ErrorId::SafetyUnverifiable,
        ErrorId::ProviderPresenceUnproven,
        ErrorId::NotImplementedInThisPass,
        ErrorId::Io,
        ErrorId::Internal,
    ];
    for id in all {
        match id {
            ErrorId::Usage
            | ErrorId::ProviderNotFound
            | ErrorId::ProviderUnhealthy
            | ErrorId::ProviderOutputChanged
            | ErrorId::ProviderTimeout
            | ErrorId::PackageNotFound
            | ErrorId::CapabilityMissing
            | ErrorId::SnapshotUnusable
            | ErrorId::RecordingUnusable
            | ErrorId::InputUnusable
            | ErrorId::PolicyDataInvalid
            | ErrorId::SafetyBlocked
            | ErrorId::SafetyUnverifiable
            | ErrorId::ProviderPresenceUnproven
            | ErrorId::NotImplementedInThisPass
            | ErrorId::Io
            | ErrorId::Internal => {}
        }
    }
    all.to_vec()
}

#[test]
fn no_error_maps_to_the_partial_success_exit_code() {
    // Exit 3 is an outcome, never an error: a halt can only reach it by being
    // a partial success, which `outcome_of` decides (INV-03).
    for id in every_error_id() {
        assert_ne!(id.exit_code(), PARTIAL_SUCCESS_EXIT_CODE, "{id:?}");
    }
}
