// SPDX-License-Identifier: Apache-2.0

//! Unit tests of step judgement over plans the real planner builds,
//! narrowed with `PlannedRun::map_plan`, which exists only in test builds
//! (and under the non-default `test-support` feature): an integration test
//! of this crate could not reach it without a self-dependency the layering
//! check forbids. Nothing here runs a process.

use super::*;
use crate::analysis::AnalysisOptions;
use crate::execution_gate::evaluable_in_this_build;
use crate::planner::{plan_run, PlanRequest};
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence,
    CommandLineTools, EnvironmentSnapshot, InstalledPackage, Machine, MacosVersion,
    PackageIdentity, PackageKind, PackageMetadata, ProcessEnd, Provenance, ProviderCapabilities,
    ProviderDetection, ProviderId, ProviderInventory, ProviderSnapshot, RequestedSet, Translation,
    Version, SNAPSHOT_SCHEMA_VERSION,
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
    providers.insert(ProviderId::new(HB), provider(HB, "/opt/first-provider", hb));
    providers.insert(
        ProviderId::new(MP),
        provider(MP, "/opt/second-provider", mp),
    );
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
            .retain(|c| evaluable_in_this_build(kind, c));
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

fn finished(stdout: &str) -> ReadOutput<'_> {
    ReadOutput {
        end: ProcessEnd::Exited { code: 0 },
        stdout,
        truncated: false,
    }
}

/// What the destination read of a plan prints when the package is there.
fn naming_destination(plan: &MigrationPlan) -> String {
    format!(
        "  {} 1.0\n",
        plan.destination_name().expect("a destination").as_str()
    )
}

/// The real-install plan from Homebrew, narrowed to its read steps, paired
/// with the snapshot it was planned from.
fn full_run() -> PlannedRun {
    planned(&install_needed(HB).0, "gnu-sed", MP).map_plan(|p| *p = read_only(p))
}

fn planned(snap: &Snapshot, package: &str, to: &str) -> PlannedRun {
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

#[test]
fn a_verification_failure_halts_and_names_the_check() {
    let run = full_run();
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let absent = finished("  unrelated 1.0\n");
    assert_eq!(
        judge_step(&run, verify, &[StepRead::Output(absent)]),
        Disposition::Halted {
            reason: HaltReason::VerificationFailed {
                command: 0,
                cause: FailureCause::NameAbsent
            },
            evidence: vec![macportsbrew_domain::ReadEvidence::of(
                &absent,
                macportsbrew_domain::Verdict::Failed(FailureCause::NameAbsent)
            )],
        }
    );
    let removal = position(run.plan(), StepKind::VerifyRemoval);
    let still_there = finished("gnu-sed 4.9\nother 1.0\n");
    assert!(matches!(
        judge_step(&run, removal, &[StepRead::Output(still_there)]),
        Disposition::Halted {
            reason: HaltReason::VerificationFailed {
                command: 0,
                cause: FailureCause::NamePresent
            },
            ..
        }
    ));
}

#[test]
fn a_truncated_or_killed_read_halts() {
    let run = full_run();
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let named = naming_destination(run.plan());
    let truncated = ReadOutput {
        end: ProcessEnd::Exited { code: 0 },
        stdout: &named,
        truncated: true,
    };
    let killed = ReadOutput {
        end: ProcessEnd::DidNotFinish,
        stdout: &named,
        truncated: false,
    };
    for (output, cause) in [
        (truncated, FailureCause::Truncated),
        (killed, FailureCause::DidNotFinish),
    ] {
        assert!(matches!(
            judge_step(&run, verify, &[StepRead::Output(output)]),
            Disposition::Halted { reason: HaltReason::VerificationFailed { cause: c, .. }, .. } if c == cause
        ));
    }
    // A read that could not be obtained halts the step without a judgement.
    for (read, reason) in [
        (
            StepRead::SiteRefused,
            HaltReason::SiteRefused { command: 0 },
        ),
        (
            StepRead::SiteUnavailable,
            HaltReason::SiteUnavailable { command: 0 },
        ),
        (
            StepRead::TransportFailed(ErrorId::ProviderTimeout),
            HaltReason::TransportFailed {
                command: 0,
                error: ErrorId::ProviderTimeout,
            },
        ),
    ] {
        assert_eq!(
            judge_step(&run, verify, &[read]),
            Disposition::Halted {
                reason,
                evidence: Vec::new()
            }
        );
    }
}

#[test]
fn every_check_must_be_backed_by_a_passing_read() {
    let run = full_run();
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let named = naming_destination(run.plan());
    let passed = judge_step(&run, verify, &[StepRead::Output(finished(&named))]);
    assert!(
        matches!(passed, Disposition::Verified { ref evidence } if evidence.len() == 1),
        "{passed:?}"
    );
    // Fewer reads than commands: never a pass.
    assert_eq!(
        judge_step(&run, verify, &[]),
        Disposition::Halted {
            reason: HaltReason::Unevaluable {
                check: None,
                why: Unevaluable::MissingRead
            },
            evidence: Vec::new()
        }
    );
    // A step whose checks no command answers: no command, nothing passed.
    let bare = run.clone().map_plan(|p| p.steps[verify].commands.clear());
    assert!(matches!(
        judge_step(&bare, verify, &[]),
        Disposition::Halted {
            reason: HaltReason::Unevaluable {
                why: Unevaluable::NoPassingRead,
                ..
            },
            ..
        }
    ));
    // A check with no evaluation on the step, even beside a passing read.
    let extra = run.clone().map_plan(|p| {
        p.steps[verify]
            .verification
            .push(VerificationCheck::ProviderIntegrityCheck);
    });
    assert!(matches!(
        judge_step(&extra, verify, &[StepRead::Output(finished(&named))]),
        Disposition::Halted {
            reason: HaltReason::Unevaluable {
                why: Unevaluable::CheckHasNoEvaluation,
                ..
            },
            ..
        }
    ));
    // A removal check on a destination step is not answered by its read.
    let misplaced = run.clone().map_plan(|p| {
        p.steps[verify].verification = vec![VerificationCheck::PackageAbsent];
    });
    assert!(matches!(
        judge_step(&misplaced, verify, &[StepRead::Output(finished(&named))]),
        Disposition::Halted {
            reason: HaltReason::Unevaluable { .. },
            ..
        }
    ));
}

/// The destination of `snap` edited by `edit`, planned from, and the plan's
/// destination check made to carry `PACKAGE_NEWLY_PRESENT`: the snapshot the
/// judgement reads is always the one the plan was built from.
fn newly_present_from(edit: impl FnOnce(&mut Snapshot)) -> Option<Unevaluable> {
    let (mut snap, package, to) = install_needed(HB);
    edit(&mut snap);
    let run = planned(&snap, package, to).map_plan(|p| {
        *p = read_only(p);
        let verify = position(p, StepKind::VerifyDestination);
        if !p.steps[verify]
            .verification
            .contains(&VerificationCheck::PackageNewlyPresent)
        {
            p.steps[verify]
                .verification
                .push(VerificationCheck::PackageNewlyPresent);
        }
    });
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let named = naming_destination(run.plan());
    match judge_step(&run, verify, &[StepRead::Output(finished(&named))]) {
        Disposition::Halted {
            reason:
                HaltReason::Unevaluable {
                    check: Some(CheckKind::PackageNewlyPresent),
                    why,
                },
            ..
        } => Some(why),
        Disposition::Verified { .. } => None,
        other => panic!("{other:?}"),
    }
}

#[test]
fn newly_present_needs_the_package_absent_from_the_snapshot_before_the_run() {
    let destination = ProviderId::new(MP);
    assert!(
        full_run().plan().steps[position(full_run().plan(), StepKind::VerifyDestination)]
            .verification
            .contains(&VerificationCheck::PackageNewlyPresent)
    );
    assert_eq!(
        newly_present_from(|_| {}),
        None,
        "absent before, present now"
    );
    // Present before the run: presence now proves nothing happened.
    assert_eq!(
        newly_present_from(|snap| {
            let mp = snap.providers.get_mut(&destination).expect("mp");
            mp.inventory.installed.push(InstalledPackage {
                identity: PackageIdentity::new(destination.clone(), "gsed"),
                kind: PackageKind::Package,
                version: Version::new("1.0"),
                revision: None,
                variants: BTreeSet::new(),
                active: true,
                requested: Some(true),
                installed_from_prebuilt: Some(true),
                install_path: None,
            });
        }),
        Some(Unevaluable::PresentBeforeTheRun)
    );
    // An inventory not recorded as readable is not a yes.
    assert_eq!(
        newly_present_from(|snap| {
            snap.providers
                .get_mut(&destination)
                .expect("mp")
                .capabilities = ProviderCapabilities::default();
        }),
        Some(Unevaluable::PresenceBeforeUnknown)
    );
}

#[test]
fn partial_success_exits_three_and_nothing_else_does() {
    assert_eq!(
        RunOutcome::PartialSuccess {
            step: 2,
            reason: HaltReason::NotPerformedInThisBuild
        }
        .exit_code(),
        3
    );
    assert_eq!(RunOutcome::Refused { refusals: 1 }.exit_code(), 4);
    assert_eq!(RunOutcome::Completed.exit_code(), 0);
    for reason in [
        HaltReason::UnexecutedMutation,
        HaltReason::NotPerformedInThisBuild,
        HaltReason::ManualReview { step: 1 },
        HaltReason::SiteRefused { command: 0 },
        HaltReason::VerificationFailed {
            command: 0,
            cause: FailureCause::Truncated,
        },
        HaltReason::Unevaluable {
            check: None,
            why: Unevaluable::NoCheck,
        },
    ] {
        assert_eq!(RunOutcome::Halted { step: 1, reason }.exit_code(), 4);
    }
    assert_eq!(
        RunOutcome::Halted {
            step: 1,
            reason: HaltReason::TransportFailed {
                command: 0,
                error: ErrorId::ProviderTimeout
            }
        }
        .exit_code(),
        ErrorId::ProviderTimeout.exit_code()
    );
}
