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
use macportsbrew_domain::PrimaryReason;
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence,
    CommandLineTools, EnvironmentSnapshot, InstalledPackage, Machine, MacosVersion,
    PackageIdentity, PackageKind, PackageMetadata, ProcessEnd, Provenance, ProviderCapabilities,
    ProviderDetection, ProviderId, ProviderInventory, ProviderSnapshot, RequestedSet, Translation,
    Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_domain::{ExecutableOwner, ResolutionState};
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

/// What each command of a plan's `VERIFY_DESTINATION` prints when the package
/// is there and active: the presence read names it, and the activation read
/// marks it in its provider's own shape (R-20; `verification_json`'s shape for
/// Homebrew).
fn destination_outputs(plan: &MigrationPlan) -> Vec<String> {
    use macportsbrew_domain::SuccessSemantic;
    let verify = position(plan, StepKind::VerifyDestination);
    let name = plan.destination_name().expect("a destination").as_str();
    plan.steps[verify]
        .commands
        .iter()
        .map(|c| match c.success {
            SuccessSemantic::StdoutMarksPackageActive => format!("  {name} @1.0_0 (active)\n"),
            SuccessSemantic::JsonReportsPackageLinked => format!(
                r#"{{"formulae": [{{"name": "{name}", "linked_keg": "1.0", "installed": [{{"version": "1.0"}}]}}], "casks": []}}"#
            ),
            _ => naming_destination(plan),
        })
        .collect()
}

/// Each of `outputs` as a read that finished.
fn passing(outputs: &[String]) -> Vec<StepRead<'_>> {
    outputs
        .iter()
        .map(|o| StepRead::Output(finished(o)))
        .collect()
}

/// The real-install plan from Homebrew, narrowed to its read steps, paired
/// with the snapshot it was planned from. Its `VERIFY_DESTINATION` carries the
/// whole contract with a probe written directly for the one executable (the
/// fixture lists none, and no probe clears the gate in this build), so the
/// judge can reach `Verified`.
fn full_run() -> PlannedRun {
    planned(&install_needed(HB).0, "gnu-sed", MP).map_plan(|p| *p = with_probes(&read_only(p)))
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
        judge(&run, verify, &[StepRead::Output(absent)]),
        Disposition::Halted {
            ends: Vec::new(),
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
        judge(&run, removal, &[StepRead::Output(still_there)]),
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
            judge(&run, verify, &[StepRead::Output(output)]),
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
            judge(&run, verify, &[read]),
            Disposition::Halted {
                ends: Vec::new(),
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
    let outputs = destination_outputs(run.plan());
    assert_eq!(outputs.len(), 2, "presence and activation (S-2)");
    let passed = judge(&run, verify, &passing(&outputs));
    assert!(
        matches!(passed, Disposition::Verified { ref evidence } if evidence.len() == 2),
        "{passed:?}"
    );
    // One read for two commands: never a pass.
    assert!(matches!(
        judge(&run, verify, &[StepRead::Output(finished(&named))]),
        Disposition::Halted {
            reason: HaltReason::Unevaluable {
                why: Unevaluable::MissingRead,
                ..
            },
            ..
        }
    ));
    // Fewer reads than commands: never a pass.
    assert_eq!(
        judge(&run, verify, &[]),
        Disposition::Halted {
            ends: Vec::new(),
            reason: HaltReason::Unevaluable {
                check: None,
                why: Unevaluable::MissingRead
            },
            evidence: Vec::new()
        }
    );
    // A step whose checks no command answers: no command, nothing passed. Judged
    // directly: `certify_destination` refuses such a step before its reads,
    // since it names no program to re-check for (SR 9; tested below).
    let bare = run.clone().map_plan(|p| p.steps[verify].commands.clear());
    assert!(matches!(
        judge_reads(&bare, verify, &[]),
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
            .push(VerificationCheck::ResolvesTo {
                executable: exe("gnu-sed"),
                provider: ProviderId::new(HB),
                package: "gnu-sed".into(),
            });
    });
    assert!(matches!(
        judge(&extra, verify, &passing(&outputs)),
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
        judge(&misplaced, verify, &passing(&outputs)),
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
        *p = with_probes(&read_only(p));
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
    let outputs = destination_outputs(run.plan());
    match judge(&run, verify, &passing(&outputs)) {
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

/// The plan from `source`'s `install_needed` snapshot, edited by `edit`, its
/// destination check narrowed to `PROVIDER_REPORTS_ACTIVE` alone, judged with
/// every read passing. `edit` gets the snapshot, the destination and its
/// package name.
fn active_alone_from(
    source: &str,
    edit: impl FnOnce(&mut Snapshot, &ProviderId, &str),
) -> Option<Unevaluable> {
    let (mut snap, package, to) = install_needed(source);
    let probe = planned(&snap, package, to);
    let destination = probe.plan().destination.clone();
    let name = probe
        .plan()
        .destination_name()
        .expect("named")
        .as_str()
        .to_owned();
    edit(&mut snap, &destination, &name);
    let run = planned(&snap, package, to).map_plan(|p| {
        // The whole destination contract stays; only which read answers
        // presence is narrowed, to `PROVIDER_REPORTS_ACTIVE` alone.
        *p = with_probes(&read_only(p));
        let verify = position(p, StepKind::VerifyDestination);
        p.steps[verify].verification.retain(|c| {
            !matches!(
                c,
                VerificationCheck::PackageNewlyPresent | VerificationCheck::ProviderReportsActive
            )
        });
        p.steps[verify]
            .verification
            .push(VerificationCheck::ProviderReportsActive);
    });
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let outputs = destination_outputs(run.plan());
    match judge(&run, verify, &passing(&outputs)) {
        Disposition::Halted {
            reason:
                HaltReason::Unevaluable {
                    check: Some(CheckKind::ProviderReportsActive),
                    why,
                },
            ..
        } => Some(why),
        Disposition::Verified { .. } => None,
        other => panic!("{other:?}"),
    }
}

/// 04, check 1: "active" is read from a whole inventory, so on a step without
/// `PACKAGE_NEWLY_PRESENT` it certifies only a consolidation — the package the
/// destination already held before the run — in both directions (INV-01,
/// INV-17; spec-guardian F1 and security-reviewer M1 on PR #44).
#[test]
fn invariant_01_provider_reports_active_alone_never_verifies_an_install() {
    for source in [HB, MP] {
        assert_eq!(
            active_alone_from(source, |_, _, _| {}),
            Some(Unevaluable::ActiveAloneAfterAnInstall),
            "{source}: absent before the run"
        );
        assert_eq!(
            active_alone_from(source, |snap, destination, name| {
                let provider = snap.providers.get_mut(destination).expect("destination");
                provider.inventory.installed.push(InstalledPackage {
                    identity: PackageIdentity::new(destination.clone(), name),
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
            None,
            "{source}: a consolidation is guarded by active alone"
        );
        assert_eq!(
            active_alone_from(source, |snap, destination, _| {
                snap.providers
                    .get_mut(destination)
                    .expect("destination")
                    .capabilities = ProviderCapabilities::default();
            }),
            Some(Unevaluable::PresenceBeforeUnknown),
            "{source}: an inventory not recorded as readable is not a yes"
        );
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
            reason: HaltReason::NotPerformedInThisBuild,
            unchecked: Vec::new(),
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

// --- Under a mutation allowance (pass 08 slice 4) -------------------------

use crate::execution_gate::{clear, execution_refusals_for, ExecutionRefusal, MutationAllowance};
use crate::history::run_record;
use macportsbrew_domain::{RunStart, RunStatus};

/// The program the plan's first `VERIFY_DESTINATION` names, which the shell
/// re-reads the destination for (SR 9); empty when there is none.
fn verify_program(run: &PlannedRun) -> String {
    run.plan()
        .steps
        .iter()
        .find(|s| s.kind == StepKind::VerifyDestination)
        .and_then(|s| s.commands.first())
        .map(|c| c.program.clone())
        .unwrap_or_default()
}

/// A snapshot whose providers' configuration was read and found empty, so the
/// planner raises no configuration risk (DP-027 (b)) and the gate's
/// configuration refusal does not stand between a test and the mutating path.
/// A configuration re-check's token for `run`'s destination, as the shell
/// obtains one before certifying (SE-107 C1). Taken against a configured copy
/// of the snapshot, so a test about the reads is not a test about the
/// configuration.
fn token(run: &PlannedRun) -> ConfigurationRechecked {
    crate::configuration_recheck::recheck_provider_configuration(
        &run.plan().destination,
        &verify_program(run),
        &configured(run.before().clone()),
        &macportsbrew_domain::ProviderConfiguration::Read { files: Vec::new() },
    )
    .expect("an empty configuration outranks nothing")
}

/// How the shells judge the step at `position`: a `VERIFY_DESTINATION` by
/// `certify_destination` with a fresh token, every other step by `judge_step`.
fn judge(run: &PlannedRun, position: usize, reads: &[StepRead<'_>]) -> Disposition {
    if run.plan().steps[position].kind == StepKind::VerifyDestination {
        certify_destination(
            run,
            position,
            token(run),
            reads,
            &all_hold(run.plan(), position),
        )
    } else {
        judge_step(run, position, reads)
    }
}

/// The answers a destination site gives when every question it is asked
/// `Holds`, walked through [`next_destination_question`] so the answers are
/// exactly the questions the sequencing asks (work order 16 part H).
fn all_hold(plan: &MigrationPlan, position: usize) -> Vec<DestinationAnswer> {
    let mut answers = Vec::new();
    loop {
        let question = next_destination_question(plan, position, &answers);
        if question == Question::Done {
            return answers;
        }
        answers.push(DestinationAnswer {
            question,
            answer: CheckAnswer::Holds,
        });
    }
}

/// What the re-read before a removal is answered when every executable's
/// ownership `Holds`.
fn owned_hold(plan: &MigrationPlan, reverify: &Reverify) -> Vec<DestinationAnswer> {
    reverify_questions(plan, reverify)
        .into_iter()
        .map(|question| DestinationAnswer {
            question,
            answer: CheckAnswer::Holds,
        })
        .collect()
}

/// `plan` with its `VERIFY_DESTINATION` made to list one executable and probe it,
/// whatever the fixture recorded: every recorded skip is replaced by a probe
/// written directly, so the judge can reach `Verified`. This bypasses the gate
/// on purpose: through the gate, in this build, no probe clears (the table
/// lands in H-2), so this is only for tests that call the judge.
fn with_probes(plan: &MigrationPlan) -> MigrationPlan {
    let mut out = plan.clone();
    for step in out
        .steps
        .iter_mut()
        .filter(|s| s.kind == StepKind::VerifyDestination)
    {
        let listed: Vec<String> = step
            .verification
            .iter()
            .find_map(|c| match c {
                VerificationCheck::ExecutablesOwnedByDestination { executables } => {
                    Some(executables.clone())
                }
                _ => None,
            })
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| vec!["gnu-sed".into()]);
        step.verification.retain(|c| {
            !matches!(
                c,
                VerificationCheck::LivenessProbeSkipped { .. }
                    | VerificationCheck::LivenessProbe { .. }
                    | VerificationCheck::ExecutablesOwnedByDestination { .. }
            )
        });
        step.verification
            .push(VerificationCheck::ExecutablesOwnedByDestination {
                executables: listed.clone(),
            });
        for executable in listed {
            step.verification.push(VerificationCheck::LivenessProbe {
                executable,
                argv: vec!["--version".into()],
            });
        }
    }
    out
}

fn configured(mut snap: Snapshot) -> Snapshot {
    for provider in snap.providers.values_mut() {
        provider.configuration =
            macportsbrew_domain::ProviderConfiguration::Read { files: Vec::new() };
    }
    snap
}

/// A planner plan narrowed so the gate passes it under an allowance — each
/// edit a statement about this build, not about the plan:
/// `dryRunOnly` false; `VERIFY_RESOLUTION` removed (the gate refuses one that
/// asserts nothing, and it carries no command); every check this build cannot
/// evaluate removed, `SET_PRIMARY`'s included; `SET_PRIMARY`'s command
/// removed (layer 4 of SE-036 does not exist, so it is never dispatched);
/// renumbered.
fn mutable(plan: &MigrationPlan) -> MigrationPlan {
    let mut out = plan.clone();
    out.dry_run_only = false;
    out.steps.retain(|s| s.kind != StepKind::VerifyResolution);
    for (i, step) in out.steps.iter_mut().enumerate() {
        step.index = u32::try_from(i).expect("small") + 1;
        let kind = step.kind;
        step.verification
            .retain(|c| evaluable_in_this_build(kind, c));
        if kind == StepKind::SetPrimary {
            step.commands.clear();
            // The fixture records no executable list, so the planner refuses
            // the primary (`EXECUTABLES_UNKNOWN`); narrowed to `NO_EXECUTABLES`,
            // the one decision that completes with nothing resolved (F-3), and
            // `VERIFY_RESOLUTION` goes with it; both edits are cut once the
            // recapture gives the fixtures real lists (F.md, "As built (F-3)",
            // deviation 9).
            step.primary = Some(macportsbrew_domain::PrimaryDecision::NoExecutables);
        }
    }
    out.elevated_steps.clear();
    out
}

fn forced_run(from: &str) -> PlannedRun {
    let (snap, package, to) = install_needed(from);
    planned(&configured(snap), package, to).map_plan(|p| *p = mutable(p))
}

/// From Homebrew toward MacPorts with the add **narrowed away** — as if the
/// package had been installed by hand: MacPorts' own writes elevate and consent
/// is not wired, so this is the one way this build reaches a removal at all,
/// Homebrew's unelevated uninstall (data, not code: INV-17). A consolidation
/// cannot stand in: it is a different case from the one this tests.
fn consolidation_run() -> PlannedRun {
    planned(&consolidation_snapshot(), "gnu-sed", MP).map_plan(|p| *p = without_add(p))
}

fn without_add(plan: &MigrationPlan) -> MigrationPlan {
    let mut out = mutable(plan);
    for step in &mut out.steps {
        if step.kind == StepKind::AddDestination {
            step.commands.clear();
        }
    }
    out
}

fn verified() -> Disposition {
    Disposition::Verified {
        evidence: Vec::new(),
    }
}

/// Records for `plan` up to `position`, each as `next_action_with` would have
/// let it end with every dispatch succeeding.
fn records_until(run: &PlannedRun, position: usize) -> Vec<StepRecord> {
    let plan = run.plan();
    let allowance = MutationAllowance::forced_in_tests();
    let mut records = Vec::new();
    for at in 0..position {
        let disposition = match next_action_with(run, at, &records, Some(&allowance)) {
            Action::Dispatch | Action::Certify => verified(),
            Action::Resolve => Disposition::Resolved {
                executables: Vec::new(),
                path_differs: false,
            },
            Action::DispatchMutating { .. } => Disposition::Performed {
                ends: vec![CommandEnd::Exited { code: 0 }],
            },
            Action::TakeSnapshot => Disposition::SnapshotTaken,
            Action::RecordHistory => Disposition::RecordPendingReadBack,
            Action::PassOver(d) => d,
            Action::Halt(reason) => panic!("step {at} halted: {reason:?}"),
        };
        records.push(StepRecord {
            index: plan.steps[at].index,
            kind: plan.steps[at].kind,
            disposition,
        });
    }
    records
}

#[test]
fn without_an_allowance_next_action_with_is_next_action() {
    for from in [HB, MP] {
        let (snap, package, to) = install_needed(from);
        let run = planned(&configured(snap), package, to);
        let plan = run.plan();
        for position in 0..plan.steps.len() {
            assert_eq!(
                next_action_with(&run, position, &[], None),
                next_action(plan, position, &[]),
            );
        }
    }
}

#[test]
fn an_allowance_dispatches_an_unelevated_add_and_halts_an_elevated_one() {
    let allowance = MutationAllowance::forced_in_tests();
    // From MacPorts toward Homebrew: Homebrew's add needs no elevation.
    let run = forced_run(MP);
    let plan = run.plan();
    let add = position(plan, StepKind::AddDestination);
    assert_eq!(
        next_action_with(&run, add, &records_until(&run, add), Some(&allowance)),
        Action::DispatchMutating { reverify: None }
    );
    // From Homebrew toward MacPorts: MacPorts' add elevates (data, not code).
    let run = forced_run(HB);
    let plan = run.plan();
    let add = position(plan, StepKind::AddDestination);
    assert_eq!(
        next_action_with(&run, add, &records_until(&run, add), Some(&allowance)),
        Action::Halt(HaltReason::ElevationNotNone { command: 0 })
    );
}

#[test]
fn elevation_is_re_derived_from_the_table_never_read_from_the_plan() {
    let allowance = MutationAllowance::forced_in_tests();
    // A plan edited to say its elevating add needs nothing.
    let run = forced_run(HB).map_plan(|p| {
        let add = position(p, StepKind::AddDestination);
        for c in &mut p.steps[add].commands {
            c.elevation = Elevation::None;
        }
    });
    let plan = run.plan();
    let add = position(plan, StepKind::AddDestination);
    assert_eq!(
        next_action_with(&run, add, &records_until(&run, add), Some(&allowance)),
        Action::Halt(HaltReason::ElevationNotNone { command: 0 })
    );
    let report = execution_refusals_for(&run, Some(&allowance));
    let step = plan.steps[add].index;
    assert!(report
        .refusals()
        .contains(&ExecutionRefusal::MutationIsNotThePlannersCommand { step }));
    assert!(report.refusals().iter().any(
        |r| matches!(r, ExecutionRefusal::ElevationRequired { steps } if steps.contains(&step))
    ));
}

#[test]
fn invariant_06_the_guard_halts_an_elevated_command_whatever_the_tokens() {
    // Work order 16 part E-3: consent is carried to the site and never to the
    // guard. The elevated add halts under an allowance, and the guard's
    // signature names no consent, so no token can change its answer in this
    // slice (part J relaxes it, for a covering token only).
    let allowance = MutationAllowance::forced_in_tests();
    // Both directions (INV-17): toward MacPorts its add elevates, toward
    // Homebrew MacPorts' removal does.
    for (from, kind) in [(HB, StepKind::AddDestination), (MP, StepKind::RemoveSource)] {
        let run = forced_run(from);
        let at = position(run.plan(), kind);
        let asked = crate::consent::elevation_requests(run.plan(), run.before()).expect("requests");
        assert!(
            asked.iter().any(|r| r.step == run.plan().steps[at].index),
            "{from}: the step asks"
        );
        assert_eq!(
            next_action_with(&run, at, &records_until(&run, at), Some(&allowance)),
            Action::Halt(HaltReason::ElevationNotNone { command: 0 }),
            "{from}"
        );
    }
    let source = include_str!("../executor.rs");
    let signature = source
        .split("pub fn next_action_with(")
        .nth(1)
        .and_then(|rest| rest.split('{').next())
        .expect("the guard's signature");
    for consent in ["Consent", "consent"] {
        assert!(
            !signature.contains(consent),
            "the guard takes consent: {signature}"
        );
    }
}

#[test]
fn consent_for_never_asks_about_a_plan_that_disagrees_with_the_table() {
    // A plan whose elevating command is not the table's: nothing is put to a
    // person as something to approve (obligation 9).
    // Both directions (INV-17): the elevating add toward MacPorts, the
    // elevating removal toward Homebrew.
    for (from, kind) in [(HB, StepKind::AddDestination), (MP, StepKind::RemoveSource)] {
        for forge in [
            (|c: &mut PlanCommand| c.argv.push("--x".into())) as fn(&mut PlanCommand),
            |c: &mut PlanCommand| c.program = "/tmp/evil".into(),
        ] {
            let run = forced_run(from).map_plan(|p| {
                let at = position(p, kind);
                forge(&mut p.steps[at].commands[0]);
            });
            let got = crate::consent::consent_for(&run, |_| panic!("asked about a forged plan"));
            assert_eq!(
                got.expect_err("refused"),
                crate::consent::ConsentHalt::PlanDisagrees,
                "{from}"
            );
        }
    }
}

#[test]
fn a_mutating_command_that_is_not_the_planners_is_refused_under_an_allowance() {
    let allowance = MutationAllowance::forced_in_tests();
    let run = forced_run(MP).map_plan(|p| {
        let add = position(p, StepKind::AddDestination);
        p.steps[add].commands[0].argv.push("--force".into());
    });
    let plan = run.plan();
    let step = plan.steps[position(plan, StepKind::AddDestination)].index;
    assert!(execution_refusals_for(&run, Some(&allowance))
        .refusals()
        .contains(&ExecutionRefusal::MutationIsNotThePlannersCommand { step }));
    // Without the allowance the same plan is refused for the build anyway.
    assert!(!crate::execution_gate::execution_refusals(plan).may_execute());
}

#[test]
fn the_narrowed_plan_clears_the_gate_only_under_an_allowance() {
    // Toward Homebrew the removal is MacPorts' and elevates: refused even
    // under an allowance, because the gate admits no elevation until part J
    // (consent is wired and unreachable, work order 16 E-3).
    let elevated = forced_run(MP);
    let report = execution_refusals_for(&elevated, Some(&MutationAllowance::forced_in_tests()));
    assert!(report
        .refusals()
        .iter()
        .any(|r| matches!(r, ExecutionRefusal::ElevationRequired { .. })));
    let run = consolidation_run();
    assert!(execution_refusals_for(&run, None)
        .refusals()
        .iter()
        .any(|r| matches!(r, ExecutionRefusal::MutationNotInThisBuild { .. })));
    let cleared = clear(run, Some(MutationAllowance::forced_in_tests())).expect("cleared");
    assert!(cleared.allowance().is_some());
    // The planner's own plan is refused even under an allowance: the
    // allowance removes the build-state refusals and nothing else.
    let (snap, package, to) = install_needed(MP);
    let unnarrowed = planned(&configured(snap), package, to);
    assert!(clear(unnarrowed, Some(MutationAllowance::forced_in_tests())).is_err());
}

/// "Verified" here means the plan's own checks on that `VERIFY_DESTINATION`
/// passed. 04's full destination contract is not evaluable in this build
/// (DP-051's four checks are refused by the gate), so the narrowed plan
/// carries `PACKAGE_NEWLY_PRESENT` and `PROVIDER_REPORTS_ACTIVE`; a flip tripwire holds
/// the switch off until the contract is evaluable (slice 4 review, spec 2).
#[test]
fn invariant_01_a_removal_is_dispatched_only_after_its_destination_verified() {
    let allowance = MutationAllowance::forced_in_tests();
    let run = consolidation_run();
    let plan = run.plan();
    let remove = position(plan, StepKind::RemoveSource);
    let verify = position(plan, StepKind::VerifyDestination);
    let good = records_until(&run, remove);
    assert_eq!(
        next_action_with(&run, remove, &good, Some(&allowance)),
        Action::DispatchMutating {
            reverify: reverify_for(plan, remove)
        }
    );
    // Any other ending of the guarding verification: never a dispatch.
    for other in [
        Disposition::NotDispatchedMutating,
        Disposition::NotPerformedInThisBuild,
        Disposition::Performed { ends: Vec::new() },
    ] {
        let mut records = good.clone();
        records[verify].disposition = other.clone();
        assert!(
            !matches!(
                next_action_with(&run, remove, &records, Some(&allowance)),
                Action::DispatchMutating { .. }
            ),
            "{other:?}"
        );
    }
    // A plan with the verification removed altogether is refused by the gate
    // (ordering, INV-01), and the loop still does not dispatch the removal.
    let run = consolidation_run().map_plan(|p| {
        p.steps.retain(|s| s.kind != StepKind::VerifyDestination);
        for (i, step) in p.steps.iter_mut().enumerate() {
            step.index = u32::try_from(i).expect("small") + 1;
        }
    });
    let plan = run.plan();
    let remove = position(plan, StepKind::RemoveSource);
    let records = records_until(&run, remove);
    assert!(!matches!(
        next_action_with(&run, remove, &records, Some(&allowance)),
        Action::DispatchMutating { .. }
    ),);
    assert!(!execution_refusals_for(&run, Some(&allowance)).may_execute());
}

#[test]
fn a_mutating_step_with_checks_of_its_own_halts_rather_than_dropping_them() {
    let allowance = MutationAllowance::forced_in_tests();
    let run = forced_run(MP).map_plan(|p| {
        let add = position(p, StepKind::AddDestination);
        p.steps[add]
            .verification
            .push(VerificationCheck::PackageNewlyPresent);
    });
    let plan = run.plan();
    let add = position(plan, StepKind::AddDestination);
    assert_eq!(
        next_action_with(&run, add, &records_until(&run, add), Some(&allowance)),
        Action::Halt(HaltReason::MutatingStepCarriesVerification)
    );
}

#[test]
fn set_primary_and_cleanup_are_never_dispatched_under_an_allowance() {
    let allowance = MutationAllowance::forced_in_tests();
    let (snap, package, to) = install_needed(MP);
    let run = planned(&configured(snap), package, to);
    let plan = run.plan();
    for (at, step) in plan.steps.iter().enumerate() {
        if matches!(step.kind, StepKind::SetPrimary | StepKind::CleanupReview) {
            let records: Vec<StepRecord> = plan.steps[..at]
                .iter()
                .map(|s| StepRecord {
                    index: s.index,
                    kind: s.kind,
                    disposition: verified(),
                })
                .collect();
            assert!(!matches!(
                next_action_with(&run, at, &records, Some(&allowance)),
                Action::DispatchMutating { .. }
            ),);
        }
    }
}

#[test]
fn a_set_primary_step_carrying_a_provider_command_halts_under_an_allowance() {
    // The mutation proof found the test above only meets planner plans, whose
    // SET_PRIMARY targets the tool; a doctored plan pointing it at a provider
    // must halt rather than dispatch as an add (plan §9).
    let allowance = MutationAllowance::forced_in_tests();
    let run = forced_run(MP).map_plan(|p| {
        let add = position(p, StepKind::AddDestination);
        let command = p.steps[add].commands[0].clone();
        let set = position(p, StepKind::SetPrimary);
        p.steps[set].commands = vec![command];
    });
    let set = position(run.plan(), StepKind::SetPrimary);
    assert_eq!(
        next_action_with(&run, set, &records_until(&run, set), Some(&allowance)),
        Action::Halt(HaltReason::UnexecutedMutation)
    );
}

fn exited(code: i32) -> MutationResult {
    MutationResult::Ended(CommandEnd::Exited { code })
}

/// The narrowed plan with its add carrying a second, identical command: the
/// gate refuses this (one command per step is the planner's), so it is a
/// judgement-level test only (D4).
fn two_command_add() -> (PlannedRun, usize) {
    let run = forced_run(MP).map_plan(|p| {
        let add = position(p, StepKind::AddDestination);
        let c = p.steps[add].commands[0].clone();
        p.steps[add].commands.push(c);
    });
    let add = position(run.plan(), StepKind::AddDestination);
    (run, add)
}

#[test]
fn a_mutation_every_command_of_which_exited_zero_is_performed_not_verified() {
    let (run, add) = two_command_add();
    assert_eq!(
        judge_mutation(run.plan(), add, &[exited(0), exited(0)]),
        Disposition::Performed {
            ends: vec![CommandEnd::Exited { code: 0 }; 2]
        }
    );
    // Fewer ends than commands, with none failing: not performed.
    assert!(matches!(
        judge_mutation(run.plan(), add, &[exited(0)]),
        Disposition::Halted { .. }
    ));
}

#[test]
fn a_halt_on_the_second_command_keeps_the_first_and_is_recorded_as_dispatched() {
    let (run, add) = two_command_add();
    let cases = [
        (exited(1), HaltReason::MutationFailed { command: 1 }, 2),
        (
            MutationResult::Ended(CommandEnd::DidNotFinish),
            HaltReason::MutationFailed { command: 1 },
            2,
        ),
        (
            MutationResult::Refused,
            HaltReason::MutationRefused { command: 1 },
            1,
        ),
        (
            MutationResult::TransportFailed(ErrorId::Io),
            HaltReason::TransportFailed {
                command: 1,
                error: ErrorId::Io,
            },
            1,
        ),
        // The mutation proof's most serious survivor: a precondition that
        // fails before the second command must not lose the first's end, or a
        // performed mutation would go unrecorded (obligation 12).
        (
            MutationResult::PreconditionFailed(Precondition::LockLost),
            HaltReason::PreconditionFailed {
                command: 1,
                why: Precondition::LockLost,
            },
            1,
        ),
    ];
    for (second, reason, kept) in cases {
        let disposition = judge_mutation(run.plan(), add, &[exited(0), second]);
        let Disposition::Halted {
            reason: got, ends, ..
        } = &disposition
        else {
            panic!("halted: {disposition:?}");
        };
        assert_eq!(*got, reason);
        assert_eq!(ends.len(), kept);
        let record = record_with(&run, add, disposition.clone());
        assert!(record.mutation_dispatched(), "{reason:?}");
        assert_ne!(record.status(), RunStatus::Success);
    }
}

#[test]
fn a_refusal_of_the_first_command_still_counts_as_dispatched() {
    let (run, add) = two_command_add();
    let disposition = judge_mutation(run.plan(), add, &[MutationResult::Refused]);
    assert_eq!(
        disposition,
        Disposition::Halted {
            reason: HaltReason::MutationRefused { command: 0 },
            evidence: Vec::new(),
            ends: Vec::new(),
        }
    );
    assert!(record_with(&run, add, disposition).mutation_dispatched());
}

#[test]
fn a_performed_mutation_with_a_halted_verification_is_partial_never_success() {
    let run = forced_run(MP);
    let plan = run.plan();
    let add = position(plan, StepKind::AddDestination);
    let mut records = records_until(&run, add + 1);
    records.push(StepRecord {
        index: plan.steps[add + 1].index,
        kind: plan.steps[add + 1].kind,
        disposition: Disposition::Halted {
            reason: HaltReason::VerificationFailed {
                command: 0,
                cause: FailureCause::NameAbsent,
            },
            evidence: Vec::new(),
            ends: Vec::new(),
        },
    });
    for step in &plan.steps[records.len()..] {
        records.push(StepRecord {
            index: step.index,
            kind: step.kind,
            disposition: Disposition::NotReached,
        });
    }
    assert!(matches!(
        outcome_of(plan, &records),
        RunOutcome::PartialSuccess { .. }
    ));
    let record = run_record(&run, &records, start(), Some(SNAPSHOT));
    assert!(record.mutation_dispatched());
    assert_eq!(record.status(), RunStatus::Partial);
}

/// The snapshot file name the runs in these tests took first.
const SNAPSHOT: &str = "snapshot-20260930T000000Z.json";

fn start() -> RunStart {
    RunStart {
        id: "20260930T000000Z-0123456789abcdef".into(),
        started_at: "2026-09-30T00:00:00Z".into(),
        tool_version: "test".into(),
    }
}

/// The record of a run whose steps before `at` succeeded, whose step `at`
/// ended as `disposition`, and whose later steps were not reached.
fn record_with(
    run: &PlannedRun,
    at: usize,
    disposition: Disposition,
) -> macportsbrew_domain::RunRecord {
    let plan = run.plan();
    let mut records = records_until(run, at);
    records.push(StepRecord {
        index: plan.steps[at].index,
        kind: plan.steps[at].kind,
        disposition,
    });
    for step in &plan.steps[records.len()..] {
        records.push(StepRecord {
            index: step.index,
            kind: step.kind,
            disposition: Disposition::NotReached,
        });
    }
    run_record(run, &records, start(), Some(SNAPSHOT))
}

// --- D6: re-plan under the lock ------------------------------------------

/// Re-plans `snap` exactly as `consolidation_run` planned its snapshot.
fn replanned(snap: &Snapshot) -> MigrationPlan {
    without_add(planned(snap, "gnu-sed", MP).plan())
}

fn consolidation_snapshot() -> Snapshot {
    configured(snapshot(&[pkg("gnu-sed")], &[pkg("other")]))
}

#[test]
fn an_unchanged_machine_plans_to_the_same_plan() {
    let run = consolidation_run();
    let now = consolidation_snapshot();
    assert_eq!(
        plan_still_holds(run.plan(), &replanned(&now), run.before(), &now),
        Ok(())
    );
}

#[test]
fn invariant_04_a_dependent_that_appeared_under_the_lock_refuses_the_run() {
    let run = consolidation_run();
    let mut now = consolidation_snapshot();
    now.providers
        .get_mut(&ProviderId::new(HB))
        .expect("homebrew")
        .inventory
        .dependencies
        .push(macportsbrew_domain::DependencyEdge {
            from: "dependent".into(),
            to: "gnu-sed".into(),
            build_only: false,
        });
    // The dependent is a package of the source provider itself (INV-04).
    let dependent = provider(HB, "/opt/first-provider", &[pkg("dependent")])
        .inventory
        .installed
        .remove(0);
    now.providers
        .get_mut(&ProviderId::new(HB))
        .expect("homebrew")
        .inventory
        .installed
        .push(dependent);
    let fresh = replanned(&now);
    assert_ne!(*run.plan(), fresh, "the planner sees the dependent");
    assert_eq!(
        plan_still_holds(run.plan(), &fresh, run.before(), &now),
        Err(StateFact::ReverseDependency)
    );
}

#[test]
fn invariant_19_a_service_that_appeared_under_the_lock_refuses_the_run() {
    let run = consolidation_run();
    let mut now = consolidation_snapshot();
    set_service(&mut now, macportsbrew_domain::Answer::Yes);
    let fresh = replanned(&now);
    assert_ne!(*run.plan(), fresh, "the planner sees the service");
    assert_eq!(
        plan_still_holds(run.plan(), &fresh, run.before(), &now),
        Err(StateFact::Classification)
    );
}

/// Sets the source row's service answer in `snap`.
fn set_service(snap: &mut Snapshot, answer: macportsbrew_domain::Answer) {
    let meta = snap
        .providers
        .get_mut(&ProviderId::new(HB))
        .expect("homebrew")
        .inventory
        .metadata
        .get_mut("gnu-sed")
        .expect("metadata");
    *meta = std::mem::take(meta).with_service(macportsbrew_domain::ServiceDefinition::both(answer));
}

#[test]
fn invariant_19_a_service_answer_that_became_unknown_under_the_lock_refuses_the_run() {
    // The re-plan under the lock goes through the same planner (part D §2),
    // and the shell's recapture propagates its refusal: nothing is replanned,
    // so nothing runs (`crates/cli/src/app/migrate.rs`, `recapture`).
    let mut now = consolidation_snapshot();
    set_service(&mut now, macportsbrew_domain::Answer::Unknown);
    let refused = plan_run(
        &now,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        &AnalysisOptions {
            today: "2026-09-30".into(),
            advanced: false,
            only: BTreeSet::new(),
        },
        &PlanRequest {
            package: "gnu-sed".into(),
            to: ProviderId::new(MP),
            retain_source: false,
        },
        &now.providers.keys().cloned().collect::<Vec<_>>(),
    )
    .expect_err("an unknown answer refuses the re-plan");
    assert_eq!(refused.id, macportsbrew_domain::ErrorId::SafetyUnverifiable);
}

#[test]
fn plans_that_differ_for_no_listed_fact_are_named_plan_differs() {
    let run = consolidation_run();
    let now = consolidation_snapshot();
    let mut fresh = replanned(&now);
    fresh.policy_version.push_str("-other");
    assert_eq!(
        plan_still_holds(run.plan(), &fresh, run.before(), &now),
        Err(StateFact::PlanDiffers)
    );
}

#[test]
fn a_plan_whose_entry_moved_under_the_lock_is_refused() {
    // DP-037 (part C plan §9): the entry decides the confirmation level part
    // K will require, so a re-plan under the lock that enters the row
    // differently must refuse, even when no listed fact names why.
    let run = consolidation_run();
    let now = consolidation_snapshot();
    let mut fresh = replanned(&now);
    fresh.entered_by = Some(macportsbrew_domain::EntryMode::Named);
    assert_ne!(run.plan().entered_by, fresh.entered_by);
    assert_eq!(
        plan_still_holds(run.plan(), &fresh, run.before(), &now),
        Err(StateFact::PlanDiffers)
    );
}

#[test]
fn a_provider_that_moved_or_changed_what_it_can_do_under_the_lock_is_named() {
    // The mutation proof: the run is refused either way, but the diagnosis
    // named neither fact in any test, so swapping them survived.
    let run = consolidation_run();
    let differs = |now: &Snapshot| {
        let mut fresh = replanned(now);
        fresh.policy_version.push('x');
        plan_still_holds(run.plan(), &fresh, run.before(), now)
    };
    let mut moved = consolidation_snapshot();
    moved
        .providers
        .get_mut(&ProviderId::new(HB))
        .expect("homebrew")
        .detection
        .prefix = Some("/somewhere/else".into());
    assert_eq!(differs(&moved), Err(StateFact::ProviderLocation));

    let mut changed = consolidation_snapshot();
    let capabilities = &mut changed
        .providers
        .get_mut(&ProviderId::new(HB))
        .expect("homebrew")
        .capabilities
        .capabilities;
    let state = capabilities
        .values_mut()
        .next()
        .expect("the fixture states a capability");
    assert_ne!(*state, macportsbrew_domain::CapabilityState::Unknown);
    *state = macportsbrew_domain::CapabilityState::Unknown;
    assert_eq!(differs(&changed), Err(StateFact::ProviderCapability));
}

#[test]
fn the_source_package_changing_under_the_lock_is_named() {
    let run = consolidation_run();
    let mut now = consolidation_snapshot();
    now.providers
        .get_mut(&ProviderId::new(HB))
        .expect("homebrew")
        .inventory
        .installed[0]
        .version = Version::new("2.0");
    let fresh = replanned(&now);
    // Whatever the planner made of it, the comparison refuses on difference
    // and the diagnosis names the package.
    if *run.plan() != fresh {
        assert_eq!(
            plan_still_holds(run.plan(), &fresh, run.before(), &now),
            Err(StateFact::SourcePackage)
        );
    }
    assert_eq!(
        plan_still_holds(
            run.plan(),
            &{
                let mut f = fresh.clone();
                f.policy_version.push('x');
                f
            },
            run.before(),
            &now
        ),
        Err(StateFact::SourcePackage)
    );
}

// --- slice 4 end-of-slice review additions ---------------------------------

#[test]
fn a_gui_apps_mutation_is_re_derived_for_its_kind_and_never_dispatched_on_none() {
    // Security 2 / spec 1: the table's `*_gui_app` cells are `unknown`, so a
    // kind-blind re-derivation read a cask's UNKNOWN as NONE. With the plan's
    // field edited to NONE, nothing re-derived for the kind may agree with it.
    let allowance = MutationAllowance::forced_in_tests();
    let mut snap = consolidation_snapshot();
    for p in snap.providers.values_mut() {
        for row in &mut p.inventory.installed {
            row.kind = PackageKind::GuiApp;
        }
    }
    // The planner stops a GUI app at MANUAL_REVIEW, so the run is the
    // narrowed consolidation paired with a snapshot that says GUI app.
    let narrowed = consolidation_run();
    let run = PlannedRun::new(narrowed.plan().clone(), snap);
    let plan = run.plan();
    let remove = position(plan, StepKind::RemoveSource);
    assert_eq!(plan.steps[remove].commands[0].elevation, Elevation::None);
    assert!(!matches!(
        next_action_with(&run, remove, &records_until(&run, remove), Some(&allowance)),
        Action::DispatchMutating { .. }
    ),);
    let step = plan.steps[remove].index;
    assert!(execution_refusals_for(&run, Some(&allowance))
        .refusals()
        .contains(&ExecutionRefusal::MutationIsNotThePlannersCommand { step }));
}

#[test]
fn invariant_19_manual_review_halts_and_is_refused_under_an_allowance() {
    let allowance = MutationAllowance::forced_in_tests();
    let run = consolidation_run().map_plan(|p| {
        let last = p.steps.len();
        let mut review = p.steps[last - 1].clone();
        review.kind = StepKind::ManualReview;
        review.index = u32::try_from(last).expect("small") + 1;
        p.steps.push(review);
    });
    let plan = run.plan();
    let review = plan.steps.last().expect("a step").index;
    // Checked over the whole plan before anything about the step, allowance
    // or not: the first step already halts at the review.
    assert_eq!(
        next_action_with(&run, 0, &[], Some(&allowance)),
        Action::Halt(HaltReason::ManualReview { step: review })
    );
    assert!(execution_refusals_for(&run, Some(&allowance))
        .refusals()
        .contains(&ExecutionRefusal::ManualReview { step: review }));
}

#[test]
fn invariant_03_a_mutation_with_no_verification_after_it_is_refused() {
    let allowance = MutationAllowance::forced_in_tests();
    let run = consolidation_run().map_plan(|p| {
        p.steps.retain(|s| s.kind != StepKind::VerifyRemoval);
        for (i, step) in p.steps.iter_mut().enumerate() {
            step.index = u32::try_from(i).expect("small") + 1;
        }
    });
    let plan = run.plan();
    let step = plan.steps[position(plan, StepKind::RemoveSource)].index;
    assert!(execution_refusals_for(&run, Some(&allowance))
        .refusals()
        .contains(&ExecutionRefusal::MutationNotVerifiedAfter { step }));
    // And with nothing dropped, nothing of this kind is refused.
    let run = consolidation_run();
    assert!(!execution_refusals_for(&run, Some(&allowance))
        .refusals()
        .iter()
        .any(|r| matches!(r, ExecutionRefusal::MutationNotVerifiedAfter { .. })));
}

// --- Pass 08 slice 5: the two brackets performed (D1, D2, D3) ---------------

/// A run toward Homebrew that **can** end `Completed`: the retain-source shape,
/// narrowed further. The layers this build does not have are removed whole —
/// `SET_PRIMARY` (layer 4 of SE-036 does not exist, so the step would be passed
/// over, and a passed-over step is not a carried-out one) and `VERIFY_RESOLUTION`
/// (no evaluation) — and so is everything that removes the source. A real
/// planned run cannot complete until those exist; that is carried, not hidden
/// here (slice 5 report).
fn completable_run() -> PlannedRun {
    forced_run(MP).map_plan(|p| {
        // `SET_PRIMARY` stays since F-3: `NO_EXECUTABLES` is carried out as
        // `NOTHING_TO_CHANGE` (F.md, "cut the narrowing").
        p.steps.retain(|s| {
            !matches!(
                s.kind,
                StepKind::RemoveSource | StepKind::VerifyRemoval | StepKind::CleanupReview
            )
        });
        for (i, step) in p.steps.iter_mut().enumerate() {
            step.index = u32::try_from(i).expect("small") + 1;
        }
        p.retain_source = true;
    })
}

#[test]
fn only_an_allowance_performs_the_two_brackets() {
    let allowance = MutationAllowance::forced_in_tests();
    let run = completable_run();
    let plan = run.plan();
    let (snapshot, record) = (
        position(plan, StepKind::SnapshotBefore),
        position(plan, StepKind::RecordHistory),
    );
    assert_eq!(
        next_action_with(&run, snapshot, &[], Some(&allowance)),
        Action::TakeSnapshot
    );
    assert_eq!(
        next_action_with(&run, record, &records_until(&run, record), Some(&allowance)),
        Action::RecordHistory
    );
    // The read-only loop still passes both over: the shipped binary is unchanged.
    for at in [snapshot, record] {
        assert_eq!(
            next_action(plan, at, &records_until(&run, at)),
            Action::PassOver(Disposition::NotPerformedInThisBuild),
            "step {at}"
        );
    }
}

#[test]
fn a_mutating_command_is_never_dispatched_before_the_snapshot_was_taken() {
    // 04: a snapshot is taken before the first mutating step. Every record the
    // snapshot step could have ended as other than taken keeps the add back.
    let allowance = MutationAllowance::forced_in_tests();
    let run = forced_run(MP);
    let plan = run.plan();
    let add = position(plan, StepKind::AddDestination);
    let snapshot = position(plan, StepKind::SnapshotBefore);
    for instead in [
        Disposition::NotPerformedInThisBuild,
        Disposition::NotReached,
        verified(),
        Disposition::NotDispatchedMutating,
    ] {
        let mut records = records_until(&run, add);
        records[snapshot].disposition = instead.clone();
        let answer = next_action_with(&run, add, &records, Some(&allowance));
        assert!(
            matches!(answer, Action::Halt(_)),
            "{instead:?} let the add through: {answer:?}"
        );
    }
    // And the control: with the snapshot taken, the same add is dispatched.
    assert_eq!(
        next_action_with(&run, add, &records_until(&run, add), Some(&allowance)),
        Action::DispatchMutating { reverify: None }
    );
}

#[test]
fn a_run_with_both_brackets_performed_completes_and_a_skipped_one_never_does() {
    let run = completable_run();
    let plan = run.plan();
    let all = records_until(&run, plan.steps.len());
    assert_eq!(outcome_of(plan, &all), RunOutcome::Completed);
    for kind in [StepKind::SnapshotBefore, StepKind::RecordHistory] {
        let mut skipped = all.clone();
        let at = position(plan, kind);
        skipped[at].disposition = Disposition::NotPerformedInThisBuild;
        assert!(
            !matches!(outcome_of(plan, &skipped), RunOutcome::Completed),
            "{kind:?} skipped still completed"
        );
    }
}

#[test]
fn the_record_of_a_completed_run_names_its_snapshot_and_stores_itself_pending() {
    use macportsbrew_domain::StepResult;
    let run = completable_run();
    let plan = run.plan();
    let all = records_until(&run, plan.steps.len());
    let record = run_record(&run, &all, start(), Some(SNAPSHOT));
    assert_eq!(record.snapshot_path(), Some(SNAPSHOT));
    assert_eq!(record.status(), RunStatus::Success);
    let results: Vec<(StepKind, &StepResult)> = record
        .steps()
        .iter()
        .map(|s| (s.kind(), s.result()))
        .collect();
    assert!(results
        .iter()
        .any(|(k, r)| *k == StepKind::SnapshotBefore && matches!(r, StepResult::SnapshotTaken)));
    assert!(results
        .iter()
        .any(|(k, r)| *k == StepKind::RecordHistory
            && matches!(r, StepResult::RecordPendingReadBack)));
    // The record never certifies its own write: nothing marks it verified.
    assert!(!results
        .iter()
        .any(|(k, r)| *k == StepKind::RecordHistory && matches!(r, StepResult::Verified { .. })));
}

#[test]
fn a_snapshot_that_could_not_be_taken_halts_the_run_and_records_nothing_was_dispatched() {
    let run = forced_run(MP);
    let plan = run.plan();
    let at = position(plan, StepKind::SnapshotBefore);
    let halt = Disposition::Halted {
        reason: HaltReason::SnapshotNotTaken,
        evidence: Vec::new(),
        ends: Vec::new(),
    };
    let record = record_with(&run, at, halt);
    assert!(!record.mutation_dispatched());
    assert_eq!(record.status(), RunStatus::NoChange);
    assert_eq!(
        HaltReason::SnapshotNotTaken.error_id(),
        ErrorId::SafetyBlocked
    );
}
// --- work order 16 part S: certification and the re-read before removal -----

/// A re-check's token for `provider`, against a configured copy of `run`'s
/// snapshot.
fn token_for(run: &PlannedRun, provider: &ProviderId) -> ConfigurationRechecked {
    crate::configuration_recheck::recheck_provider_configuration(
        provider,
        &verify_program(run),
        &configured(run.before().clone()),
        &macportsbrew_domain::ProviderConfiguration::Read { files: Vec::new() },
    )
    .expect("an empty configuration outranks nothing")
}

#[test]
fn judge_step_never_verifies_a_verify_destination_even_with_passing_reads() {
    // D1: the reads would pass through `certify_destination`, and still
    // `judge_step` answers `NOT_CERTIFIED_HERE` before looking at them.
    let run = full_run();
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let outputs = destination_outputs(run.plan());
    let reads = passing(&outputs);
    let probed = run.clone();
    assert!(matches!(
        certify_destination(
            &probed,
            verify,
            token(&probed),
            &reads,
            &all_hold(probed.plan(), verify)
        ),
        Disposition::Verified { .. }
    ));
    assert_eq!(
        judge_step(&run, verify, &reads),
        Disposition::Halted {
            reason: HaltReason::Unevaluable {
                check: None,
                why: Unevaluable::NotCertifiedHere
            },
            evidence: Vec::new(),
            ends: Vec::new(),
        }
    );
    // And `next_action` never sends it to `judge_step`: it answers `Certify`.
    let records: Vec<StepRecord> = (0..verify)
        .map(|p| StepRecord {
            index: run.plan().steps[p].index,
            kind: run.plan().steps[p].kind,
            disposition: Disposition::NotPerformedInThisBuild,
        })
        .collect();
    assert_eq!(next_action(run.plan(), verify, &records), Action::Certify);
}

/// A re-check's token for `provider`, re-read for `program`.
fn token_program(run: &PlannedRun, provider: &ProviderId, program: &str) -> ConfigurationRechecked {
    crate::configuration_recheck::recheck_provider_configuration(
        provider,
        program,
        &configured(run.before().clone()),
        &macportsbrew_domain::ProviderConfiguration::Read { files: Vec::new() },
    )
    .expect("an empty configuration outranks nothing")
}

fn wrong_program(command: usize) -> Disposition {
    Disposition::Halted {
        reason: HaltReason::PreconditionFailed {
            command,
            why: Precondition::WrongProgramToken,
        },
        evidence: Vec::new(),
        ends: Vec::new(),
    }
}

#[test]
fn invariant_01_certify_destination_refuses_a_token_for_another_program() {
    // SR 9: a re-check made for one program certifies nothing a command of
    // another program reads.
    let run = full_run();
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let named = naming_destination(run.plan());
    let destination = run.plan().destination.clone();
    assert_eq!(
        certify_destination(
            &run,
            verify,
            token_program(&run, &destination, "another-program"),
            &[StepRead::Output(finished(&named))],
            &[]
        ),
        wrong_program(0)
    );
    // Every command, not only the first: a later command naming another
    // program is refused at its own index.
    let last = run.plan().steps[verify].commands.len();
    assert!(last >= 1);
    let more = run.clone().map_plan(|p| {
        let mut other = p.steps[verify].commands[0].clone();
        other.program = "another-program".into();
        p.steps[verify].commands.push(other);
    });
    assert_eq!(
        certify_destination(&more, verify, token(&more), &[], &[]),
        wrong_program(last)
    );
    // A step with no command names no program: nothing is certified.
    let bare = run.clone().map_plan(|p| p.steps[verify].commands.clear());
    assert_eq!(
        certify_destination(&bare, verify, token(&run), &[], &[]),
        wrong_program(0)
    );
}

#[test]
fn invariant_01_a_destination_token_for_another_program_is_refused() {
    let run = forced_run(MP);
    let plan = run.plan();
    let reverify =
        reverify_for(plan, position(plan, StepKind::RemoveSource)).expect("a removal is re-read");
    let outputs = destination_outputs(plan);
    let destination = plan.destination.clone();
    assert_eq!(
        destination_still_verified(
            &run,
            &reverify,
            token_program(&run, &destination, "another-program"),
            &passing(&outputs),
            &owned_hold(run.plan(), &reverify),
        ),
        Err(Precondition::WrongProgramToken)
    );
    // A verify step with no command names no program: refused as such here,
    // before the operation count would refuse it otherwise (mutant 3).
    let verify = reverify.verify_position();
    let bare = run.clone().map_plan(|p| p.steps[verify].commands.clear());
    assert_eq!(
        destination_still_verified(&bare, &reverify, token(&run), &[], &[]),
        Err(Precondition::WrongProgramToken)
    );
}

#[test]
fn certify_destination_refuses_a_token_for_another_provider() {
    // L1: a re-check of the source certifies nothing about the destination.
    let run = full_run();
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let named = naming_destination(run.plan());
    let source = run.plan().source.clone();
    assert_eq!(
        certify_destination(
            &run,
            verify,
            token_for(&run, &source),
            &[StepRead::Output(finished(&named))],
            &[]
        ),
        Disposition::Halted {
            reason: HaltReason::PreconditionFailed {
                command: 0,
                why: Precondition::WrongProviderToken
            },
            evidence: Vec::new(),
            ends: Vec::new(),
        }
    );
    // A position that is not a VERIFY_DESTINATION is not certified either.
    let removal = position(run.plan(), StepKind::VerifyRemoval);
    assert_eq!(
        certify_destination(&run, removal, token(&run), &[], &[]),
        Disposition::Halted {
            reason: HaltReason::RecordsDoNotMatchPlan,
            evidence: Vec::new(),
            ends: Vec::new(),
        }
    );
}

#[test]
fn reverify_for_names_the_verify_step_for_remove_source_only() {
    for from in [HB, MP] {
        let run = forced_run(from);
        let plan = run.plan();
        let verify = position(plan, StepKind::VerifyDestination);
        for (at, step) in plan.steps.iter().enumerate() {
            let answer = reverify_for(plan, at);
            if step.kind == StepKind::RemoveSource {
                let r = answer.unwrap_or_else(|| panic!("{from}: a removal is re-read"));
                assert_eq!(r.verify_position(), verify, "{from}");
                assert_eq!(r.provider(), &plan.destination, "{from}");
            } else {
                assert_eq!(answer, None, "{from}: {:?}", step.kind);
            }
        }
        // Only a removal is re-read, wherever another step sits: an add placed
        // after the VERIFY_DESTINATION (a doctored plan) is still `None`.
        let mut late_add = plan.clone();
        let add = position(&late_add, StepKind::AddDestination);
        let copy = late_add.steps[add].clone();
        late_add.steps.push(copy);
        let last = late_add.steps.len() - 1;
        assert_eq!(reverify_for(&late_add, last), None, "{from}: an add");
        // A removal with no VERIFY_DESTINATION before it has nothing to re-read.
        let mut bare = plan.clone();
        bare.steps.retain(|s| s.kind != StepKind::VerifyDestination);
        let remove = position(&bare, StepKind::RemoveSource);
        assert_eq!(reverify_for(&bare, remove), None, "{from}");
    }
}

#[test]
fn invariant_01_removal_refused_when_the_destination_vanished_after_verify() {
    // Both directions: the destination verified, then a re-read that no longer
    // names it, and the removal does not go ahead.
    for from in [HB, MP] {
        let run = forced_run(from).map_plan(|p| *p = with_probes(p));
        let plan = run.plan();
        let remove = position(plan, StepKind::RemoveSource);
        let reverify = reverify_for(plan, remove).expect("a removal is re-read");
        let outputs = destination_outputs(plan);
        assert_eq!(
            destination_still_verified(
                &run,
                &reverify,
                token(&run),
                &passing(&outputs),
                &owned_hold(run.plan(), &reverify)
            ),
            Ok(()),
            "{from}: the control"
        );
        // Gone: the read fails as a provider's read of a missing package does.
        let gone = ReadOutput {
            end: ProcessEnd::Exited { code: 1 },
            stdout: "",
            truncated: false,
        };
        // Each beside a passing activation read, so the refusal is the presence
        // read's and not a missing read's.
        let active = StepRead::Output(finished(&outputs[1]));
        assert_eq!(
            destination_still_verified(
                &run,
                &reverify,
                token(&run),
                &[StepRead::Output(gone), active],
                &owned_hold(run.plan(), &reverify),
            ),
            Err(Precondition::DestinationNotReverified),
            "{from}"
        );
        // And where the read is judged by what it names, a listing without it.
        let judged_by_name = plan.steps[reverify.verify_position()].commands[0].success
            == macportsbrew_domain::SuccessSemantic::StdoutNamesPackage;
        if judged_by_name {
            assert_eq!(
                destination_still_verified(
                    &run,
                    &reverify,
                    token(&run),
                    &[StepRead::Output(finished("  unrelated 1.0\n")), active],
                    &owned_hold(run.plan(), &reverify),
                ),
                Err(Precondition::DestinationNotReverified),
                "{from}"
            );
        }
        // Still there, no longer active: presence passes and activation does
        // not (S-2, `PROVIDER_REPORTS_ACTIVE`), in each provider's own shape.
        let name = plan.destination_name().expect("a destination").as_str();
        let inactive = match plan.steps[reverify.verify_position()].commands[1].success {
            macportsbrew_domain::SuccessSemantic::StdoutMarksPackageActive => {
                format!("  {name} @1.0_0\n")
            }
            macportsbrew_domain::SuccessSemantic::JsonReportsPackageLinked => format!(
                r#"{{"formulae": [{{"name": "{name}", "linked_keg": null, "installed": [{{"version": "1.0"}}]}}], "casks": []}}"#
            ),
            other => panic!("{from}: {other:?} is not an activation read"),
        };
        assert_eq!(
            destination_still_verified(
                &run,
                &reverify,
                token(&run),
                &[
                    StepRead::Output(finished(&outputs[0])),
                    StepRead::Output(finished(&inactive))
                ],
                &owned_hold(run.plan(), &reverify),
            ),
            Err(Precondition::DestinationNotReverified),
            "{from}: present but inactive"
        );
    }
}

#[test]
fn invariant_01_removal_refused_when_the_reread_is_truncated_failed_or_unjudged() {
    let run = forced_run(MP);
    let plan = run.plan();
    let reverify =
        reverify_for(plan, position(plan, StepKind::RemoveSource)).expect("a removal is re-read");
    let named = naming_destination(plan);
    let outputs = destination_outputs(plan);
    let active = || StepRead::Output(finished(&outputs[1]));
    // Each bad presence read beside a passing activation read; the last two
    // are a read missing (the second) and none at all.
    let reads: Vec<Vec<StepRead<'_>>> = vec![
        vec![
            StepRead::Output(ReadOutput {
                end: ProcessEnd::Exited { code: 0 },
                stdout: &named,
                truncated: true,
            }),
            active(),
        ],
        vec![
            StepRead::Output(ReadOutput {
                end: ProcessEnd::DidNotFinish,
                stdout: &named,
                truncated: false,
            }),
            active(),
        ],
        vec![
            StepRead::Output(ReadOutput {
                end: ProcessEnd::Exited { code: 1 },
                stdout: &named,
                truncated: false,
            }),
            active(),
        ],
        vec![StepRead::SiteRefused, active()],
        vec![StepRead::SiteUnavailable, active()],
        vec![
            StepRead::TransportFailed(ErrorId::ProviderTimeout),
            active(),
        ],
        vec![StepRead::Output(finished(&named))],
        vec![],
    ];
    for read in &reads {
        assert_eq!(
            destination_still_verified(
                &run,
                &reverify,
                token(&run),
                read,
                &owned_hold(run.plan(), &reverify)
            ),
            Err(Precondition::DestinationNotReverified),
            "{read:?}"
        );
    }
}

#[test]
fn invariant_01_destination_still_verified_requires_exactly_verified() {
    // Any disposition but `Verified` is a refusal — an unevaluable verify step
    // included: a re-read step whose check this build cannot answer.
    let run = forced_run(MP).map_plan(|p| {
        let verify = position(p, StepKind::VerifyDestination);
        p.steps[verify].verification.clear();
    });
    let plan = run.plan();
    let reverify =
        reverify_for(plan, position(plan, StepKind::RemoveSource)).expect("a removal is re-read");
    let outputs = destination_outputs(plan);
    assert!(matches!(
        certify_destination(
            &run,
            reverify.verify_position(),
            token(&run),
            &passing(&outputs),
            &[]
        ),
        Disposition::Halted { .. }
    ));
    assert_eq!(
        destination_still_verified(
            &run,
            &reverify,
            token(&run),
            &passing(&outputs),
            &owned_hold(run.plan(), &reverify)
        ),
        Err(Precondition::DestinationNotReverified)
    );
}

#[test]
fn invariant_01_a_destination_token_for_the_wrong_provider_is_refused() {
    let run = forced_run(MP);
    let plan = run.plan();
    let reverify =
        reverify_for(plan, position(plan, StepKind::RemoveSource)).expect("a removal is re-read");
    let outputs = destination_outputs(plan);
    let source = plan.source.clone();
    assert_eq!(
        destination_still_verified(
            &run,
            &reverify,
            token_for(&run, &source),
            &passing(&outputs),
            &owned_hold(run.plan(), &reverify),
        ),
        Err(Precondition::WrongProviderToken)
    );
}

#[test]
fn invariant_01_a_reread_command_whose_elevation_is_not_none_is_refused() {
    // L4: the elevation is checked on the plan's field and re-derived from the
    // compiled table; here the plan's field says a prompt is needed.
    let run = forced_run(MP).map_plan(|p| {
        let verify = position(p, StepKind::VerifyDestination);
        for c in &mut p.steps[verify].commands {
            c.elevation = Elevation::Required { reason: "x".into() };
        }
    });
    let plan = run.plan();
    let reverify =
        reverify_for(plan, position(plan, StepKind::RemoveSource)).expect("a removal is re-read");
    let outputs = destination_outputs(plan);
    assert_eq!(
        destination_still_verified(
            &run,
            &reverify,
            token(&run),
            &passing(&outputs),
            &owned_hold(run.plan(), &reverify)
        ),
        Err(Precondition::DestinationNotReverified)
    );
    // The activation read alone needing a prompt is refused as well: the
    // check covers every command, not the first.
    let second_only = forced_run(MP).map_plan(|p| {
        let verify = position(p, StepKind::VerifyDestination);
        p.steps[verify].commands[1].elevation = Elevation::Required { reason: "x".into() };
    });
    let plan = second_only.plan();
    let reverify =
        reverify_for(plan, position(plan, StepKind::RemoveSource)).expect("a removal is re-read");
    let outputs = destination_outputs(plan);
    assert_eq!(
        destination_still_verified(
            &second_only,
            &reverify,
            token(&second_only),
            &passing(&outputs),
            &owned_hold(second_only.plan(), &reverify),
        ),
        Err(Precondition::DestinationNotReverified)
    );
    // And the table's own answer: a GUI app's read toward MacPorts is of
    // unknown elevation, so the re-derived elevation is not `NONE` even with
    // the plan's field left at `NONE`.
    let toward_macports = forced_run(HB);
    let mut snap = toward_macports.before().clone();
    for p in snap.providers.values_mut() {
        for row in &mut p.inventory.installed {
            row.kind = PackageKind::GuiApp;
        }
    }
    let gui = PlannedRun::new(toward_macports.plan().clone(), snap);
    // The control: with the GUI app's reads present, everything but the
    // re-derived elevation would certify.
    let verify = position(gui.plan(), StepKind::VerifyDestination);
    let outputs_gui = destination_outputs(gui.plan());
    assert!(
        matches!(
            certify_destination(
                &gui,
                verify,
                token(&gui),
                &passing(&outputs_gui),
                &all_hold(gui.plan(), verify)
            ),
            Disposition::VerifiedWithUnchecked { .. }
        ),
        "the GUI-app control certifies"
    );
    let plan = gui.plan();
    let reverify =
        reverify_for(plan, position(plan, StepKind::RemoveSource)).expect("a removal is re-read");
    assert_eq!(
        destination_still_verified(
            &gui,
            &reverify,
            token(&gui),
            &passing(&outputs_gui),
            &owned_hold(gui.plan(), &reverify)
        ),
        Err(Precondition::DestinationNotReverified)
    );
}

#[test]
fn an_installed_read_never_answers_provider_reports_active() {
    // D11: installed is not active.
    use crate::execution_gate::answers;
    use macportsbrew_domain::SuccessSemantic;
    for semantic in [
        SuccessSemantic::ExitStatus,
        SuccessSemantic::StdoutNamesPackage,
        SuccessSemantic::StdoutOmitsPackage,
        SuccessSemantic::Unknown,
    ] {
        assert!(
            !answers(&VerificationCheck::ProviderReportsActive, semantic),
            "{semantic:?}"
        );
    }
    for semantic in [
        SuccessSemantic::StdoutMarksPackageActive,
        SuccessSemantic::JsonReportsPackageLinked,
    ] {
        assert!(answers(&VerificationCheck::ProviderReportsActive, semantic));
        assert!(!answers(&VerificationCheck::PackageNewlyPresent, semantic));
        assert!(!answers(&VerificationCheck::PackageAbsent, semantic));
    }
}

#[test]
fn a_removal_with_no_step_to_reread_is_never_dispatched() {
    // Defence in depth behind `destination_verified_before`: a removal the
    // application can name no re-read for halts instead of dispatching.
    for from in [HB, MP] {
        let run = forced_run(from);
        let plan = run.plan();
        let remove = position(plan, StepKind::RemoveSource);
        assert_eq!(
            dispatch_for(plan, remove),
            Action::DispatchMutating {
                reverify: reverify_for(plan, remove)
            },
            "{from}: the control"
        );
        let mut bare = plan.clone();
        bare.steps.retain(|s| s.kind != StepKind::VerifyDestination);
        let remove = position(&bare, StepKind::RemoveSource);
        assert_eq!(
            dispatch_for(&bare, remove),
            Action::Halt(HaltReason::UnexecutedMutation),
            "{from}"
        );
        // An add is dispatched with no re-read.
        let add = position(plan, StepKind::AddDestination);
        assert_eq!(
            dispatch_for(plan, add),
            Action::DispatchMutating { reverify: None },
            "{from}"
        );
    }
}

// --- the primary decision and VERIFY_RESOLUTION (work order 16 part F-3) -----

fn exe(name: &str) -> ExecutableName {
    ExecutableName::parse(name).unwrap()
}

/// A run of the planner's plan from Homebrew toward MacPorts with its decision
/// replaced by `decision`, and, when it states a list, a `VERIFY_RESOLUTION`
/// with one `RESOLVES_TO` for `gnu-sed` after `SET_PRIMARY`.
fn primary_run(decision: PrimaryDecision) -> PlannedRun {
    let (snap, package, to) = install_needed(HB);
    planned(&configured(snap), package, to).map_plan(|plan| {
        let at = plan
            .steps
            .iter()
            .position(|s| s.kind == StepKind::SetPrimary)
            .unwrap();
        plan.steps.retain(|s| s.kind != StepKind::VerifyResolution);
        let lists = !decision.executables().is_empty();
        plan.steps[at].primary = Some(decision);
        if lists {
            plan.steps.insert(
                at + 1,
                macportsbrew_domain::MigrationStep {
                    index: 0,
                    kind: StepKind::VerifyResolution,
                    provider: None,
                    commands: Vec::new(),
                    verification: vec![VerificationCheck::ResolvesTo {
                        executable: exe("gnu-sed"),
                        provider: plan.destination.clone(),
                        package: plan.destination_package.clone().unwrap(),
                    }],
                    primary: None,
                    reason: macportsbrew_domain::Reason::new("reason.verify_resolution"),
                },
            );
        }
        for (i, step) in plan.steps.iter_mut().enumerate() {
            step.index = u32::try_from(i).unwrap() + 1;
        }
    })
}

fn path_used() -> Vec<String> {
    vec!["/opt/dest/bin".into(), "/usr/bin".into()]
}

fn nothing_to_change() -> PrimaryDecision {
    PrimaryDecision::NothingToChange {
        executables: [exe("gnu-sed")].into_iter().collect(),
        path_used: path_used(),
    }
}

fn found(owner: ExecutableOwner, authoritative: bool, index: Option<u32>) -> ResolutionState {
    ResolutionState::Found {
        resolution: macportsbrew_domain::Resolution {
            executable: "gnu-sed".into(),
            // Joined, not spelt: a literal program path in production source is
            // what scripts/check-no-shell.sh refuses.
            path: Some(PathBuf::from("/opt/dest/bin").join("gnu-sed")),
            canonical_path: None,
            owner,
            authoritative,
        },
        path_index: index,
    }
}

fn resolution_position(run: &PlannedRun) -> usize {
    run.plan()
        .steps
        .iter()
        .position(|s| s.kind == StepKind::VerifyResolution)
        .unwrap()
}

/// Records for the steps before `position`, none of them a halt: what
/// `next_action` needs to answer about the step at `position`.
fn fake_records(plan: &MigrationPlan, position: usize) -> Vec<StepRecord> {
    plan.steps[..position]
        .iter()
        .map(|s| StepRecord {
            index: s.index,
            kind: s.kind,
            disposition: Disposition::NotPerformedInThisBuild,
        })
        .collect()
}

#[test]
fn nothing_to_change_is_answered_only_for_a_command_less_set_primary_that_completes() {
    // Mutant 5: `NOTHING_TO_CHANGE` carried on another kind or decision.
    for (decision, expected) in [
        (nothing_to_change(), Disposition::NothingToChange),
        (PrimaryDecision::NoExecutables, Disposition::NothingToChange),
        (PrimaryDecision::Unknown, Disposition::NotDispatchedMutating),
        (
            PrimaryDecision::Refused {
                executables: BTreeSet::new(),
                reasons: BTreeMap::new(),
                package: Some(PrimaryReason::ExecutablesUnknown),
                path_used: Vec::new(),
            },
            Disposition::NotDispatchedMutating,
        ),
    ] {
        let run = primary_run(decision.clone());
        let plan = run.plan();
        let at = plan
            .steps
            .iter()
            .position(|s| s.kind == StepKind::SetPrimary)
            .unwrap();
        let records = fake_records(plan, at);
        assert_eq!(
            next_action(plan, at, &records),
            Action::PassOver(expected.clone()),
            "{decision:?}"
        );
        // Under an allowance too: never a dispatch.
        assert_eq!(
            next_action_with(
                &run,
                at,
                &records,
                Some(&MutationAllowance::forced_in_tests())
            ),
            Action::PassOver(expected),
            "{decision:?}"
        );
    }
    // A command-less `ADD_DESTINATION` carrying a decision is passed over, not
    // completed: only `SET_PRIMARY` completes this way.
    let run = primary_run(nothing_to_change()).map_plan(|plan| {
        let add = plan
            .steps
            .iter()
            .position(|s| s.kind == StepKind::AddDestination)
            .unwrap();
        plan.steps[add].commands.clear();
        plan.steps[add].primary = Some(PrimaryDecision::NoExecutables);
    });
    let add = run
        .plan()
        .steps
        .iter()
        .position(|s| s.kind == StepKind::AddDestination)
        .unwrap();
    assert_eq!(
        next_action(run.plan(), add, &fake_records(run.plan(), add)),
        Action::PassOver(Disposition::NotDispatchedMutating)
    );
    // And a `SET_PRIMARY` with a command still halts, decision or not.
    let run = primary_run(nothing_to_change()).map_plan(|plan| {
        let add = plan
            .steps
            .iter()
            .position(|s| s.kind == StepKind::AddDestination)
            .unwrap();
        let at = plan
            .steps
            .iter()
            .position(|s| s.kind == StepKind::SetPrimary)
            .unwrap();
        let command = plan.steps[add].commands[0].clone();
        plan.steps[at].commands.push(command);
    });
    let at = run
        .plan()
        .steps
        .iter()
        .position(|s| s.kind == StepKind::SetPrimary)
        .unwrap();
    assert_eq!(
        next_action(run.plan(), at, &fake_records(run.plan(), at)),
        Action::Halt(HaltReason::UnexecutedMutation)
    );
}

#[test]
fn verify_resolution_passes_only_on_the_destination_package_and_authoritative() {
    // SR-3, mutants 3 (executor half) and 16.
    let run = primary_run(nothing_to_change());
    let at = resolution_position(&run);
    let destination = run.plan().destination.clone();
    let package = run.plan().destination_package.clone().unwrap();
    let pass = judge_resolution(
        &run,
        at,
        &[found(
            ExecutableOwner::Provider {
                provider: destination.clone(),
                package: Some(package.clone()),
            },
            true,
            Some(0),
        )],
        &path_used(),
    );
    assert_eq!(
        pass,
        Disposition::Resolved {
            executables: vec![exe("gnu-sed")],
            path_differs: false,
        }
    );
    for (owner, authoritative, observed) in [
        (
            ExecutableOwner::Provider {
                provider: destination.clone(),
                package: None,
            },
            true,
            ObservedWinner::AnotherPackage,
        ),
        (
            ExecutableOwner::Provider {
                provider: destination.clone(),
                package: Some("other".into()),
            },
            true,
            ObservedWinner::AnotherPackage,
        ),
        (
            ExecutableOwner::Provider {
                provider: destination.clone(),
                package: Some(package.clone()),
            },
            false,
            ObservedWinner::NotAuthoritative,
        ),
        (
            ExecutableOwner::System,
            false,
            ObservedWinner::NotAProviderFile,
        ),
        (
            ExecutableOwner::Unknown,
            false,
            ObservedWinner::NotAProviderFile,
        ),
    ] {
        assert_eq!(
            judge_resolution(
                &run,
                at,
                &[found(owner.clone(), authoritative, Some(0))],
                &path_used()
            ),
            Disposition::Halted {
                reason: HaltReason::ResolutionMismatch {
                    executable: exe("gnu-sed"),
                    observed,
                    path_differs: false,
                },
                evidence: Vec::new(),
                ends: Vec::new(),
            },
            "{owner:?} {authoritative}"
        );
    }
}

#[test]
fn verify_resolution_halts_when_the_source_still_wins() {
    let run = primary_run(nothing_to_change());
    let at = resolution_position(&run);
    let source = run.plan().source.clone();
    let disposition = judge_resolution(
        &run,
        at,
        &[found(
            ExecutableOwner::Provider {
                provider: source,
                package: Some("gnu-sed".into()),
            },
            true,
            Some(0),
        )],
        &path_used(),
    );
    assert!(matches!(
        disposition,
        Disposition::Halted {
            reason: HaltReason::ResolutionMismatch {
                observed: ObservedWinner::AnotherProvider,
                ..
            },
            ..
        }
    ));
    // Nothing on `PATH` is a mismatch too, never a pass.
    assert!(matches!(
        judge_resolution(&run, at, &[ResolutionState::NotOnPath], &path_used()),
        Disposition::Halted {
            reason: HaltReason::ResolutionMismatch {
                observed: ObservedWinner::NotOnPath,
                ..
            },
            ..
        }
    ));
}

#[test]
fn verify_resolution_under_replay_is_unevaluable() {
    // INV-15, mutant 6: an unobserved resolution is never a pass.
    let run = primary_run(nothing_to_change());
    let at = resolution_position(&run);
    let disposition = judge_resolution(&run, at, &[ResolutionState::NotObserved], &[]);
    assert_eq!(
        disposition,
        Disposition::Halted {
            reason: HaltReason::Unevaluable {
                check: Some(CheckKind::ResolvesTo),
                why: Unevaluable::ResolutionNotObserved,
            },
            evidence: Vec::new(),
            ends: Vec::new(),
        }
    );
    // A winner whose index the run-time `PATH` does not establish (a relative
    // entry before it, SR-4) is unevaluable too; and a missing answer is.
    let destination = run.plan().destination.clone();
    let package = run.plan().destination_package.clone().unwrap();
    assert!(matches!(
        judge_resolution(
            &run,
            at,
            &[found(
                ExecutableOwner::Provider {
                    provider: destination,
                    package: Some(package),
                },
                true,
                None
            )],
            &path_used()
        ),
        Disposition::Halted {
            reason: HaltReason::Unevaluable {
                why: Unevaluable::WinnerIndexUnknown,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        judge_resolution(&run, at, &[], &path_used()),
        Disposition::Halted {
            reason: HaltReason::Unevaluable {
                why: Unevaluable::MissingRead,
                ..
            },
            ..
        }
    ));
}

#[test]
fn the_run_time_path_is_redacted_before_it_is_compared() {
    // SG2-N5, mutant 18d: the shell hands over the run-time `PATH` redacted the
    // same way as `pathUsed`, so an equal one is not "different"; a different
    // one is stated, whichever way the step ends.
    let run = primary_run(nothing_to_change());
    let at = resolution_position(&run);
    let destination = run.plan().destination.clone();
    let package = run.plan().destination_package.clone().unwrap();
    let pass = found(
        ExecutableOwner::Provider {
            provider: destination,
            package: Some(package),
        },
        true,
        Some(0),
    );
    assert!(matches!(
        judge_resolution(&run, at, std::slice::from_ref(&pass), &path_used()),
        Disposition::Resolved {
            path_differs: false,
            ..
        }
    ));
    assert!(matches!(
        judge_resolution(&run, at, &[pass], &["/elsewhere".to_string()]),
        Disposition::Resolved {
            path_differs: true,
            ..
        }
    ));
    assert!(matches!(
        judge_resolution(
            &run,
            at,
            &[ResolutionState::NotOnPath],
            &["/elsewhere".to_string()]
        ),
        Disposition::Halted {
            reason: HaltReason::ResolutionMismatch {
                path_differs: true,
                ..
            },
            ..
        }
    ));
}

#[test]
fn a_shim_refusal_is_refused_with_and_without_an_allowance() {
    // Mutant 22: `PRIMARY_NEEDS_SHIM` is a gate refusal regardless of
    // allowance.
    let run = primary_run(PrimaryDecision::Refused {
        executables: [exe("gnu-sed")].into_iter().collect(),
        reasons: [(exe("gnu-sed"), PrimaryReason::PrimaryNeedsShim)]
            .into_iter()
            .collect(),
        package: None,
        path_used: path_used(),
    });
    for allowance in [None, Some(MutationAllowance::forced_in_tests())] {
        let report = execution_refusals_for(&run, allowance.as_ref());
        assert!(
            report
                .refusals()
                .iter()
                .any(|r| matches!(r, ExecutionRefusal::PrimaryNeedsShim { .. })),
            "{allowance:?}"
        );
    }
}
/// `consolidation_run` — the one run of this build that reaches an unelevated
/// `REMOVE_SOURCE` under an allowance — with its decision set to
/// `NOTHING_TO_CHANGE` for `gnu-sed` and a `VERIFY_RESOLUTION` for it placed
/// after `SET_PRIMARY` and before the removal, as `check_ordering` requires.
fn removal_guarded_by_a_resolution() -> PlannedRun {
    consolidation_run().map_plan(|plan| {
        let at = position(plan, StepKind::SetPrimary);
        plan.steps[at].primary = Some(nothing_to_change());
        plan.steps.insert(
            at + 1,
            macportsbrew_domain::MigrationStep {
                index: 0,
                kind: StepKind::VerifyResolution,
                provider: None,
                commands: Vec::new(),
                verification: vec![VerificationCheck::ResolvesTo {
                    executable: exe("gnu-sed"),
                    provider: plan.destination.clone(),
                    package: plan.destination_package.clone().unwrap(),
                }],
                primary: None,
                reason: macportsbrew_domain::Reason::new("reason.verify_resolution"),
            },
        );
        for (i, step) in plan.steps.iter_mut().enumerate() {
            step.index = u32::try_from(i).unwrap() + 1;
        }
    })
}

#[test]
fn invariant_03_a_missing_destination_executable_ends_partial_with_the_source_kept() {
    // SG-4: the plan **includes** a `REMOVE_SOURCE`, under the test allowance,
    // and it never runs. The destination does not provide `gnu-sed`, so
    // nothing on `PATH` answers for it: `VERIFY_RESOLUTION` halts, the removal
    // is never reached, and the run ends partial with the source installed.
    // Once part H evaluates `EXECUTABLES_OWNED_BY_DESTINATION` the same run
    // fails earlier, at `VERIFY_DESTINATION`.
    let allowance = MutationAllowance::forced_in_tests();
    let run = removal_guarded_by_a_resolution();
    let plan = run.plan();
    plan.check_ordering()
        .expect("the resolution guards the removal");
    let resolution = position(plan, StepKind::VerifyResolution);
    let remove = position(plan, StepKind::RemoveSource);
    assert!(resolution < remove);

    let mut records = records_until(&run, resolution);
    assert_eq!(
        next_action_with(&run, resolution, &records, Some(&allowance)),
        Action::Resolve
    );
    let halted = judge_resolution(
        &run,
        resolution,
        &[ResolutionState::NotOnPath],
        &path_used(),
    );
    assert!(matches!(
        halted,
        Disposition::Halted {
            reason: HaltReason::ResolutionMismatch {
                observed: ObservedWinner::NotOnPath,
                ..
            },
            ..
        }
    ));
    records.push(StepRecord {
        index: plan.steps[resolution].index,
        kind: StepKind::VerifyResolution,
        disposition: halted,
    });
    // The removal, and every step after the halt, is never dispatched.
    for at in resolution + 1..plan.steps.len() {
        assert!(
            !matches!(
                next_action_with(&run, at, &records, Some(&allowance)),
                Action::DispatchMutating { .. }
            ),
            "step {at}"
        );
        records.push(StepRecord {
            index: plan.steps[at].index,
            kind: plan.steps[at].kind,
            disposition: Disposition::NotReached,
        });
    }
    let outcome = outcome_of(plan, &records);
    assert!(
        matches!(
            outcome,
            RunOutcome::PartialSuccess {
                reason: HaltReason::ResolutionMismatch { .. },
                ..
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(outcome.exit_code(), 3);
    assert_eq!(records[remove].disposition, Disposition::NotReached);

    // And the guard does not lean on the halt alone (security review of F-3,
    // M-1): a resolution recorded any way but `RESOLVED` keeps the removal
    // from being dispatched even where the loop would go on.
    let mut records = records_until(&run, remove);
    assert!(matches!(
        records[resolution].disposition,
        Disposition::Resolved { .. }
    ));
    assert!(matches!(
        next_action_with(&run, remove, &records, Some(&allowance)),
        Action::DispatchMutating { .. }
    ));
    records[resolution].disposition = Disposition::NotPerformedInThisBuild;
    assert_eq!(
        next_action_with(&run, remove, &records, Some(&allowance)),
        Action::Halt(HaltReason::UnexecutedMutation)
    );
}
#[test]
fn invariant_01_a_removal_waits_on_a_resolution_recorded_resolved() {
    // INV-01 for the primary (spec-guardian review of F-3; security M-1):
    // `REMOVE_SOURCE` is dispatched only once every earlier
    // `VERIFY_RESOLUTION` was recorded `RESOLVED`. Every other way the
    // resolution can be recorded short of a halt keeps it from being
    // dispatched, even where the loop would carry on.
    let allowance = MutationAllowance::forced_in_tests();
    let run = removal_guarded_by_a_resolution();
    let plan = run.plan();
    let resolution = position(plan, StepKind::VerifyResolution);
    let remove = position(plan, StepKind::RemoveSource);
    let good = records_until(&run, remove);
    assert!(matches!(
        next_action_with(&run, remove, &good, Some(&allowance)),
        Action::DispatchMutating { .. }
    ));
    for other in [
        Disposition::NotPerformedInThisBuild,
        Disposition::NothingToChange,
        verified(),
        Disposition::NotDispatchedMutating,
        Disposition::Performed { ends: Vec::new() },
        Disposition::SnapshotTaken,
        Disposition::RecordPendingReadBack,
    ] {
        let mut records = good.clone();
        records[resolution].disposition = other.clone();
        assert_eq!(
            next_action_with(&run, remove, &records, Some(&allowance)),
            Action::Halt(HaltReason::UnexecutedMutation),
            "{other:?}"
        );
    }
}

// --- the providers' own locks (DP-030 (b)) -----------------------------------

/// A machine with both providers, one broken if asked.
fn lock_snapshot(broken: bool) -> Snapshot {
    let mut snap = snapshot(&[], &[]);
    if broken {
        let provider = snap.providers.values_mut().next().expect("a provider");
        provider.detection.presence = ProviderPresence::Broken {
            reason: "scratch".to_string(),
        };
    }
    snap
}

#[test]
fn only_none_observed_clears_a_run_and_each_refusal_says_what_it_is() {
    use macportsbrew_domain::{ConcurrentProviderActivity as A, UndeterminedCause as C};
    use std::collections::{BTreeMap, BTreeSet};
    let snapshot = lock_snapshot(false);
    assert_eq!(provider_locks_clear(A::NoneObserved, &snapshot), Ok(()));
    let undetermined = BTreeMap::from([(ProviderId::new("a"), C::Changed)]);
    let held = A::Observed {
        providers: BTreeSet::from([ProviderId::new("b")]),
        undetermined: undetermined.clone(),
    };
    let refusal = provider_locks_clear(held, &snapshot).expect_err("held");
    assert_eq!(refusal.precondition(), Precondition::ProviderLockHeld);
    let error = refusal.error();
    assert_eq!(error.id, ErrorId::ProviderLockHeld);
    assert!(
        error.detail.contains('b') && error.detail.contains("a=CHANGED"),
        "{}",
        error.detail
    );
    for other in [
        A::NotChecked,
        A::Unknowable,
        A::NotDetermined { undetermined },
    ] {
        let refusal = provider_locks_clear(other, &snapshot).expect_err("not a check");
        assert_eq!(
            refusal.precondition(),
            Precondition::ProviderLocksNotDetermined
        );
        let error = refusal.error();
        assert_eq!(error.id, ErrorId::ProviderLocksNotDetermined);
        assert!(!error.detail.is_empty() && !error.detail.contains(' '));
    }
}

#[test]
fn a_broken_provider_does_not_clear_a_run_even_with_every_lock_free() {
    use macportsbrew_domain::ConcurrentProviderActivity as A;
    let refusal = provider_locks_clear(A::NoneObserved, &lock_snapshot(true))
        .expect_err("a provider that did not answer");
    assert_eq!(
        refusal.precondition(),
        Precondition::ProviderLocksNotDetermined
    );
    assert!(refusal.error().detail.contains("PROVIDER_BROKEN"));
}

#[test]
fn a_precondition_about_a_providers_lock_keeps_its_own_exit_code() {
    let reason = |why| HaltReason::PreconditionFailed { command: 0, why };
    assert_eq!(
        reason(Precondition::ProviderLockHeld).error_id(),
        ErrorId::ProviderLockHeld
    );
    assert_eq!(
        reason(Precondition::ProviderLocksNotDetermined).error_id(),
        ErrorId::ProviderLocksNotDetermined
    );
    assert_eq!(
        reason(Precondition::LockLost).error_id(),
        ErrorId::SafetyBlocked
    );
}

// --- The destination's answers (work order 16 part H) ----------------------

use macportsbrew_domain::{
    CheckFailure, DestinationCheck, DestinationFacts, LinkageFacts, OwnershipFacts, UncheckedCause,
    UncheckedEntry,
};

/// The whole run of questions, each answered by `site`.
fn answered(
    plan: &MigrationPlan,
    position: usize,
    site: impl Fn(&Question) -> CheckAnswer,
) -> Vec<DestinationAnswer> {
    let mut answers = Vec::new();
    loop {
        let question = next_destination_question(plan, position, &answers);
        if question == Question::Done {
            return answers;
        }
        let answer = site(&question);
        answers.push(DestinationAnswer { question, answer });
    }
}

/// `VERIFY_DESTINATION` certified with the reads passing and `site` answering.
fn certified(run: &PlannedRun, site: impl Fn(&Question) -> CheckAnswer) -> Disposition {
    let plan = run.plan();
    let verify = position(plan, StepKind::VerifyDestination);
    let outputs = destination_outputs(plan);
    let reads = passing(&outputs);
    certify_destination(
        run,
        verify,
        token(run),
        &reads,
        &answered(plan, verify, site),
    )
}

fn unchecked_by(disposition: &Disposition) -> &[UncheckedEntry] {
    match disposition {
        Disposition::VerifiedWithUnchecked { unchecked, .. } => unchecked,
        other => panic!("not unchecked: {other:?}"),
    }
}

#[test]
fn a_check_with_no_observation_is_unchecked_never_holds() {
    let run = full_run();
    let outcome = certified(&run, |_| {
        CheckAnswer::Unchecked(UncheckedCause::NotObserved)
    });
    let unchecked = unchecked_by(&outcome);
    // Ownership, integrity and linkage; the probe was not run, since ownership
    // did not hold, and says so.
    let checks: Vec<DestinationCheck> = unchecked.iter().map(|e| e.check).collect();
    assert_eq!(
        checks,
        [
            DestinationCheck::ExecutablesOwnedByDestination,
            DestinationCheck::ProviderIntegrityCheck,
            DestinationCheck::NoCrossProviderLinkage,
            DestinationCheck::LivenessProbe,
        ]
    );
    assert_eq!(unchecked[0].cause, UncheckedCause::NotObserved);
    assert_eq!(unchecked[3].cause, UncheckedCause::OwnershipNotTold);
}

#[test]
fn verified_needs_all_four_to_hold_and_each_unchecked_alone_denies_it() {
    let run = full_run();
    assert!(matches!(
        certified(&run, |_| CheckAnswer::Holds),
        Disposition::Verified { .. }
    ));
    let is_integrity = |q: &Question| *q == Question::Integrity;
    let is_linkage = |q: &Question| *q == Question::Linkage;
    let is_owned = |q: &Question| matches!(q, Question::Owned { .. });
    let is_probe = |q: &Question| matches!(q, Question::Probe { .. });
    let kinds: [&dyn Fn(&Question) -> bool; 4] = [&is_integrity, &is_linkage, &is_owned, &is_probe];
    for which in kinds {
        let outcome = certified(&run, |q| {
            if which(q) {
                CheckAnswer::Unchecked(UncheckedCause::NotObserved)
            } else {
                CheckAnswer::Holds
            }
        });
        assert!(
            matches!(outcome, Disposition::VerifiedWithUnchecked { .. }),
            "{outcome:?}"
        );
    }
}

#[test]
fn a_skip_answers_unchecked_whatever_the_site_says_and_is_never_asked_about() {
    let run = full_run().map_plan(|plan| {
        let verify = position(plan, StepKind::VerifyDestination);
        for check in &mut plan.steps[verify].verification {
            if let VerificationCheck::LivenessProbe { executable, .. } = check {
                *check = VerificationCheck::LivenessProbeSkipped {
                    executable: Some(executable.clone()),
                    cause: macportsbrew_domain::ProbeSkipCause::NoCuratedProbe,
                };
            }
        }
    });
    let outcome = certified(&run, |q| {
        assert!(
            !matches!(q, Question::Probe { .. }),
            "a skip was asked: {q:?}"
        );
        CheckAnswer::Holds
    });
    assert_eq!(
        unchecked_by(&outcome)[0].cause,
        UncheckedCause::NoCuratedProbe
    );
}

#[test]
fn a_package_with_no_executable_never_passes_vacuously() {
    let run = full_run().map_plan(|plan| {
        let verify = position(plan, StepKind::VerifyDestination);
        let checks = &mut plan.steps[verify].verification;
        checks.retain(|c| !matches!(c, VerificationCheck::LivenessProbe { .. }));
        for check in checks.iter_mut() {
            if let VerificationCheck::ExecutablesOwnedByDestination { executables } = check {
                executables.clear();
            }
        }
        checks.push(VerificationCheck::LivenessProbeSkipped {
            executable: None,
            cause: macportsbrew_domain::ProbeSkipCause::NoExecutableToProbe,
        });
    });
    let outcome = certified(&run, |_| CheckAnswer::Holds);
    assert_eq!(
        unchecked_by(&outcome),
        [UncheckedEntry {
            check: DestinationCheck::LivenessProbe,
            executable: None,
            cause: UncheckedCause::NoExecutableToProbe,
        }]
    );
}

#[test]
fn a_fails_halts_names_the_check_and_stops_what_follows() {
    let run = full_run();
    let plan = run.plan();
    let verify = position(plan, StepKind::VerifyDestination);
    for (failing, expected) in [
        (
            Question::Integrity,
            DestinationCheck::ProviderIntegrityCheck,
        ),
        (Question::Linkage, DestinationCheck::NoCrossProviderLinkage),
    ] {
        let site = |q: &Question| {
            if *q == failing {
                CheckAnswer::Fails(CheckFailure::PathMissing)
            } else {
                CheckAnswer::Holds
            }
        };
        let answers = answered(plan, verify, site);
        assert!(
            !answers
                .iter()
                .any(|a| matches!(a.question, Question::Probe { .. })),
            "a probe after a Fails"
        );
        match certified(&run, site) {
            Disposition::Halted {
                reason: HaltReason::DestinationCheckFailed { check, cause, .. },
                ..
            } => {
                assert_eq!(check, expected);
                assert_eq!(cause, CheckFailure::PathMissing);
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn a_probe_is_asked_only_of_a_leaf_owned_and_only_when_no_file_was_malformed() {
    let run = full_run();
    let plan = run.plan();
    let verify = position(plan, StepKind::VerifyDestination);
    let asked_a_probe = |site: &dyn Fn(&Question) -> CheckAnswer| {
        answered(plan, verify, site)
            .iter()
            .any(|a| matches!(a.question, Question::Probe { .. }))
    };
    assert!(asked_a_probe(&|_| CheckAnswer::Holds));
    assert!(!asked_a_probe(&|q| {
        if matches!(q, Question::Owned { .. }) {
            CheckAnswer::Unchecked(UncheckedCause::NotObserved)
        } else {
            CheckAnswer::Holds
        }
    }));
    assert!(!asked_a_probe(&|q| {
        if *q == Question::Linkage {
            CheckAnswer::Unchecked(UncheckedCause::MalformedFile)
        } else {
            CheckAnswer::Holds
        }
    }));
}

/// A probe runs the destination's program, and whatever linkage could not tell may
/// be what that program loads. So every linkage answer but `Holds` stops the probe,
/// and the entry says it was that cause and not the ownership's.
#[test]
fn every_linkage_answer_but_holds_stops_the_probe_and_is_the_reason_given() {
    for cause in [
        UncheckedCause::NotObserved,
        UncheckedCause::MalformedFile,
        UncheckedCause::WalkOverBound,
        UncheckedCause::UnreadableFile,
        UncheckedCause::FileNotExamined,
        UncheckedCause::SpellingsDisagree,
        UncheckedCause::UnderUndetectedPrefix,
        UncheckedCause::RpathUnresolved,
        UncheckedCause::InterpreterResolvedAtRunTime,
        UncheckedCause::ReadFailed,
    ] {
        let run = full_run();
        let plan = run.plan();
        let verify = position(plan, StepKind::VerifyDestination);
        let site = |q: &Question| {
            if *q == Question::Linkage {
                CheckAnswer::Unchecked(cause)
            } else {
                CheckAnswer::Holds
            }
        };
        assert!(
            !answered(plan, verify, site)
                .iter()
                .any(|a| matches!(a.question, Question::Probe { .. })),
            "{cause:?}"
        );
        let outcome = certified(&run, site);
        let probe = unchecked_by(&outcome)
            .iter()
            .find(|e| e.check == DestinationCheck::LivenessProbe)
            .expect("the probe not run says so");
        assert_eq!(probe.cause, cause);
    }
    // A probe not run for ownership keeps ownership's reason.
    let run = full_run();
    let outcome = certified(&run, |q| {
        if matches!(q, Question::Owned { .. }) {
            CheckAnswer::Unchecked(UncheckedCause::NotObserved)
        } else {
            CheckAnswer::Holds
        }
    });
    let probe = unchecked_by(&outcome)
        .iter()
        .find(|e| e.check == DestinationCheck::LivenessProbe)
        .expect("the probe not run says so");
    assert_eq!(probe.cause, UncheckedCause::OwnershipNotTold);
}

/// A destination holding the package only as an inactive image: no active version
/// to pin, so nothing can say this is the version the plan saw; the planner's
/// `destination_holds_it` counts the row, so the plan claims no `PACKAGE_NEWLY_PRESENT`
/// and a link already there is not called this run's.
#[test]
fn a_destination_holding_only_an_inactive_row_is_held_and_never_pins_a_version() {
    let mut snap = configured(snapshot(&[pkg("gnu-sed")], &[pkg("gsed")]));
    let provider = snap
        .providers
        .get_mut(&ProviderId::new(MP))
        .expect("provider");
    for row in &mut provider.inventory.installed {
        row.active = false;
    }
    let run = planned(&snap, "gnu-sed", MP);
    let plan = run.plan();
    let verify = position(plan, StepKind::VerifyDestination);
    assert!(
        !plan.steps[verify]
            .verification
            .iter()
            .any(|check| matches!(check, VerificationCheck::PackageNewlyPresent)),
        "the row counts as held"
    );
    assert!(crate::destination_judges::held_row(&run).is_none());
    let owner = DestinationFacts::Ownership(OwnershipFacts::Owner {
        name: "gsed".to_owned(),
        version: "4.9".to_owned(),
        active: Some("4.9".to_owned()),
        versions_present: 1,
        held: None,
    });
    assert!(matches!(
        crate::destination_judges::answer_from_facts(&run, &owner),
        CheckAnswer::Unchecked(_)
    ));
    let linkage = DestinationFacts::Linkage(LinkageFacts {
        links_detected_provider: true,
        ..LinkageFacts::default()
    });
    assert_eq!(
        crate::destination_judges::answer_from_facts(&run, &linkage),
        CheckAnswer::Fails(CheckFailure::LinkPresentBeforeTheRun)
    );
}

#[test]
fn a_site_answered_check_is_site_answered_only_on_verify_destination() {
    for check in [
        VerificationCheck::ProviderIntegrityCheck,
        VerificationCheck::NoCrossProviderLinkage,
    ] {
        assert!(crate::execution_gate::is_site_answered(
            StepKind::VerifyDestination,
            &check
        ));
        assert!(!crate::execution_gate::is_site_answered(
            StepKind::VerifyRemoval,
            &check
        ));
    }
}

#[test]
fn answers_that_are_not_the_questions_asked_certify_nothing() {
    let run = full_run();
    let plan = run.plan();
    let verify = position(plan, StepKind::VerifyDestination);
    let good = all_hold(plan, verify);
    let outputs = destination_outputs(plan);
    let reads = passing(&outputs);
    let missing = &good[..good.len() - 1];
    let mut reordered = good.clone();
    reordered.reverse();
    let mut extra = good.clone();
    extra.push(good[0].clone());
    for answers in [&[][..], missing, &reordered, &extra] {
        let outcome = certify_destination(&run, verify, token(&run), &reads, answers);
        assert!(
            matches!(
                outcome,
                Disposition::Halted {
                    reason: HaltReason::Unevaluable {
                        why: Unevaluable::AnswersDoNotMatchQuestions,
                        ..
                    },
                    ..
                }
            ),
            "{outcome:?}"
        );
    }
}

#[test]
fn a_step_missing_one_of_the_four_is_not_certified_however_the_site_answers() {
    let run = full_run();
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let drop_one: [fn(&VerificationCheck) -> bool; 3] = [
        |c| matches!(c, VerificationCheck::ProviderIntegrityCheck),
        |c| matches!(c, VerificationCheck::NoCrossProviderLinkage),
        |c| matches!(c, VerificationCheck::LivenessProbe { .. }),
    ];
    for dropped in drop_one {
        let narrowed =
            full_run().map_plan(|p| p.steps[verify].verification.retain(|c| !dropped(c)));
        let outputs = destination_outputs(narrowed.plan());
        let reads = passing(&outputs);
        let answers = all_hold(narrowed.plan(), verify);
        let outcome = certify_destination(&narrowed, verify, token(&narrowed), &reads, &answers);
        assert!(
            matches!(
                outcome,
                Disposition::Halted {
                    reason: HaltReason::Unevaluable {
                        why: Unevaluable::ContractIncomplete,
                        ..
                    },
                    ..
                }
            ),
            "{outcome:?}"
        );
    }
}

fn unchecked_entry() -> UncheckedEntry {
    UncheckedEntry {
        check: DestinationCheck::ProviderIntegrityCheck,
        executable: None,
        cause: UncheckedCause::NotObserved,
    }
}

fn with_unchecked() -> Disposition {
    Disposition::VerifiedWithUnchecked {
        evidence: Vec::new(),
        unchecked: vec![unchecked_entry()],
    }
}

/// Records up to `position` of a removal run, with its destination unchecked.
fn records_unchecked_until(run: &PlannedRun, position_of: usize) -> Vec<StepRecord> {
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let mut records = records_until(run, position_of);
    records[verify].disposition = with_unchecked();
    records
}

#[test]
fn invariant_01_an_unchecked_destination_is_never_removed_in_either_direction() {
    let allowance = MutationAllowance::forced_in_tests();
    let run = consolidation_run();
    let plan = run.plan();
    let remove = position(plan, StepKind::RemoveSource);
    let records = records_unchecked_until(&run, remove);
    assert_eq!(
        next_action_with(&run, remove, &records, Some(&allowance)),
        Action::PassOver(Disposition::SourceKeptUnchecked)
    );
    // The same rule in the other direction (INV-17): one judge, no path to a
    // dispatch, and the comparison reads no provider.
    let (snap, package, to) = install_needed(MP);
    let other = planned(&configured(snap), package, to).map_plan(|p| *p = without_add(p));
    assert_ne!(other.plan().destination, run.plan().destination);
    let remove_other = position(other.plan(), StepKind::RemoveSource);
    let records = records_unchecked_until(&other, remove_other);
    assert_eq!(
        next_action_with(&other, remove_other, &records, Some(&allowance)),
        Action::PassOver(Disposition::SourceKeptUnchecked)
    );
}

#[test]
fn a_resolution_that_did_not_hold_still_halts_a_removal_passed_over() {
    let allowance = MutationAllowance::forced_in_tests();
    let run = consolidation_run().map_plan(|plan| {
        let at = position(plan, StepKind::RemoveSource);
        let mut resolution = plan.steps[at].clone();
        resolution.kind = StepKind::VerifyResolution;
        resolution.commands.clear();
        plan.steps.insert(at, resolution);
        for (i, step) in plan.steps.iter_mut().enumerate() {
            step.index = u32::try_from(i).expect("small") + 1;
        }
    });
    let remove = position(run.plan(), StepKind::RemoveSource);
    let verify = position(run.plan(), StepKind::VerifyDestination);
    let mut records: Vec<StepRecord> = run
        .plan()
        .steps
        .iter()
        .take(remove)
        .map(|s| StepRecord {
            index: s.index,
            kind: s.kind,
            disposition: match s.kind {
                StepKind::SnapshotBefore => Disposition::SnapshotTaken,
                StepKind::VerifyResolution => Disposition::NotReached,
                _ => Disposition::NotDispatchedMutating,
            },
        })
        .collect();
    records[verify].disposition = with_unchecked();
    // A record that is not a resolution held: halts, not a pass-over. Not
    // `NotReached`, which the loop refuses first as records that do not match.
    let at = position(run.plan(), StepKind::VerifyResolution);
    records[at].disposition = Disposition::NotPerformedInThisBuild;
    assert_eq!(
        next_action_with(&run, remove, &records, Some(&allowance)),
        Action::Halt(HaltReason::UnexecutedMutation)
    );
}

#[test]
fn after_a_kept_source_its_removal_check_and_review_are_passed_over_and_only_then() {
    let allowance = MutationAllowance::forced_in_tests();
    let run = consolidation_run();
    let plan = run.plan();
    let remove = position(plan, StepKind::RemoveSource);
    let check = position(plan, StepKind::VerifyRemoval);
    let mut records = records_unchecked_until(&run, remove);
    records.push(StepRecord {
        index: plan.steps[remove].index,
        kind: StepKind::RemoveSource,
        disposition: Disposition::SourceKeptUnchecked,
    });
    assert_eq!(
        next_action_with(&run, check, &records, Some(&allowance)),
        Action::PassOver(Disposition::SourceKeptUnchecked)
    );
    // The review behind a kept source too, command-less or not.
    let review = run.clone().map_plan(|plan| {
        let mut step = plan.steps[check].clone();
        step.kind = StepKind::CleanupReview;
        step.commands.clear();
        step.verification.clear();
        plan.steps[check] = step;
    });
    assert_eq!(
        next_action_with(&review, check, &records, Some(&allowance)),
        Action::PassOver(Disposition::SourceKeptUnchecked)
    );
    let mut performed = records.clone();
    performed[remove].disposition = Disposition::Performed { ends: vec![] };
    assert_ne!(
        next_action_with(&review, check, &performed, Some(&allowance)),
        Action::PassOver(Disposition::SourceKeptUnchecked)
    );
    // A removal performed: the check is judged as ever.
    records[remove].disposition = Disposition::Performed { ends: vec![] };
    assert_ne!(
        next_action_with(&run, check, &records, Some(&allowance)),
        Action::PassOver(Disposition::SourceKeptUnchecked)
    );
}

fn step_records(
    run: &PlannedRun,
    disposition: impl Fn(StepKind) -> Disposition,
) -> Vec<StepRecord> {
    run.plan()
        .steps
        .iter()
        .map(|s| StepRecord {
            index: s.index,
            kind: s.kind,
            disposition: disposition(s.kind),
        })
        .collect()
}

/// A run that stayed clean except for what it could not tell.
fn unchecked_shape(kind: StepKind) -> Disposition {
    match kind {
        StepKind::VerifyDestination => with_unchecked(),
        StepKind::RemoveSource | StepKind::VerifyRemoval | StepKind::CleanupReview => {
            Disposition::SourceKeptUnchecked
        }
        StepKind::SnapshotBefore => Disposition::SnapshotTaken,
        StepKind::RecordHistory => Disposition::RecordPendingReadBack,
        StepKind::SetPrimary => Disposition::NothingToChange,
        StepKind::AddDestination | StepKind::VerifyResolution | StepKind::ManualReview => {
            Disposition::Performed { ends: Vec::new() }
        }
    }
}

#[test]
fn completed_with_unchecked_is_its_own_outcome_and_exits_three() {
    let run = consolidation_run();
    let outcome = outcome_of(run.plan(), &step_records(&run, unchecked_shape));
    assert_eq!(
        outcome,
        RunOutcome::CompletedWithUnchecked {
            unchecked: vec![unchecked_entry()],
            source_kept: true,
        }
    );
    assert_eq!(outcome.exit_code(), PARTIAL_SUCCESS_EXIT_CODE);
}

#[test]
fn a_halt_or_a_step_not_reached_outranks_it_and_the_unchecked_list_survives() {
    let run = consolidation_run();
    let halted = HaltReason::SnapshotNotTaken;
    for last in [
        Disposition::Halted {
            reason: halted.clone(),
            evidence: Vec::new(),
            ends: Vec::new(),
        },
        Disposition::NotReached,
        Disposition::NotPerformedInThisBuild,
    ] {
        let records = step_records(&run, |kind| {
            if kind == StepKind::RecordHistory {
                last.clone()
            } else {
                unchecked_shape(kind)
            }
        });
        match outcome_of(run.plan(), &records) {
            RunOutcome::PartialSuccess { unchecked, .. } => {
                assert_eq!(unchecked, vec![unchecked_entry()]);
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn a_pass_over_is_accepted_only_in_its_exact_shape() {
    let run = consolidation_run();
    // A kept source with nothing unchecked before it.
    let records = step_records(&run, |kind| {
        if kind == StepKind::VerifyDestination {
            Disposition::Verified {
                evidence: Vec::new(),
            }
        } else {
            unchecked_shape(kind)
        }
    });
    assert!(matches!(
        outcome_of(run.plan(), &records),
        RunOutcome::PartialSuccess { .. }
    ));
    // On a step it may not stand on.
    let records = step_records(&run, |kind| {
        if kind == StepKind::SetPrimary {
            Disposition::SourceKeptUnchecked
        } else {
            unchecked_shape(kind)
        }
    });
    assert!(matches!(
        outcome_of(run.plan(), &records),
        RunOutcome::PartialSuccess { .. }
    ));
}

#[test]
fn the_re_read_before_a_removal_has_its_own_judge_and_unchecked_is_a_halt() {
    let run = consolidation_run().map_plan(|p| *p = with_probes(p));
    let plan = run.plan();
    let remove = position(plan, StepKind::RemoveSource);
    let reverify = reverify_for(plan, remove).expect("a re-read");
    let outputs = destination_outputs(plan);
    let reads = passing(&outputs);
    let good = owned_hold(plan, &reverify);
    assert!(!good.is_empty());
    assert_eq!(
        destination_still_verified(&run, &reverify, token(&run), &reads, &good),
        Ok(())
    );
    let mut unchecked = good.clone();
    unchecked[0].answer = CheckAnswer::Unchecked(UncheckedCause::NotObserved);
    let mut fails = good.clone();
    fails[0].answer = CheckAnswer::Fails(CheckFailure::NoPlacedEntry);
    for owned in [&unchecked[..], &fails[..], &[][..]] {
        assert_eq!(
            destination_still_verified(&run, &reverify, token(&run), &reads, owned),
            Err(Precondition::DestinationNotReverified)
        );
    }
    // An empty list asks nothing, and that passes nothing: this is the last
    // guard before a removal.
    let verify = reverify.verify_position();
    let nothing_listed = run.clone().map_plan(|plan| {
        for check in &mut plan.steps[verify].verification {
            if let VerificationCheck::ExecutablesOwnedByDestination { executables } = check {
                executables.clear();
            }
        }
    });
    let outputs = destination_outputs(nothing_listed.plan());
    let reads = passing(&outputs);
    assert_eq!(
        destination_still_verified(
            &nothing_listed,
            &reverify,
            token(&nothing_listed),
            &reads,
            &[]
        ),
        Err(Precondition::DestinationNotReverified)
    );
}

#[test]
fn invariant_17_one_judge_gives_one_answer_in_both_directions() {
    let toward = |from: &str| {
        let (snap, package, to) = install_needed(from);
        planned(&configured(snap), package, to).map_plan(|p| *p = with_probes(p))
    };
    let (one, other) = (toward(HB), toward(MP));
    assert_ne!(one.plan().destination, other.plan().destination);
    let shape = |d: &Disposition| match d {
        Disposition::Verified { .. } => "verified".to_owned(),
        Disposition::VerifiedWithUnchecked { unchecked, .. } => {
            format!("unchecked {}", unchecked.len())
        }
        Disposition::Halted { reason, .. } => format!("{reason:?}"),
        other => panic!("{other:?}"),
    };
    let sites: [&dyn Fn(&Question) -> CheckAnswer; 3] = [
        &|_| CheckAnswer::Holds,
        &|_| CheckAnswer::Unchecked(UncheckedCause::NotObserved),
        &|q| {
            if *q == Question::Linkage {
                CheckAnswer::Fails(CheckFailure::LinksAnotherProvider)
            } else {
                CheckAnswer::Holds
            }
        },
    ];
    for site in sites {
        assert_eq!(
            shape(&certified(&one, site)),
            shape(&certified(&other, site))
        );
    }
}

#[test]
fn a_read_that_did_not_pass_lets_nothing_be_asked() {
    let run = full_run();
    let plan = run.plan();
    let verify = position(plan, StepKind::VerifyDestination);
    let outputs = destination_outputs(plan);
    assert!(reads_let_the_site_be_asked(
        &run,
        verify,
        &passing(&outputs)
    ));
    // Finished, and not naming the package: a read that failed, with an output.
    let wrong: Vec<String> = outputs.iter().map(|_| String::new()).collect();
    assert!(!reads_let_the_site_be_asked(&run, verify, &passing(&wrong)));
    assert!(!reads_let_the_site_be_asked(&run, verify, &[]));
}

#[test]
fn a_probe_passes_the_gate_only_for_the_tables_row() {
    // Here, in the unit test build, the table is the shipped one plus a fixture row
    // (`TEST_PROBE_EXECUTABLE`); a row the table does not hold is refused, here and
    // in the shipped build, where the twelve of `liveness-probes.toml` are all there is.
    let refused_for = |executable: &str| {
        let plan = with_probes(consolidation_run().plan());
        let verify = position(&plan, StepKind::VerifyDestination);
        let mut plan = plan;
        for check in &mut plan.steps[verify].verification {
            match check {
                VerificationCheck::ExecutablesOwnedByDestination { executables } => {
                    *executables = vec![executable.to_owned()];
                }
                VerificationCheck::LivenessProbe { executable: e, .. } => {
                    *e = executable.to_owned();
                }
                _ => {}
            }
        }
        crate::execution_refusals(&plan)
            .refusals()
            .iter()
            .any(|r| matches!(r, ExecutionRefusal::ProbeIsNotTheTables { .. }))
    };
    assert!(!refused_for(crate::execution_gate::TEST_PROBE_EXECUTABLE));
    assert!(refused_for("not-the-fixture-row"));
}

/// The row exists, and this argv is not its argv: the table is the security decision
/// and `--init` is the shape of what it keeps out.
#[test]
fn a_probe_with_another_argv_than_its_row_is_not_the_tables() {
    let mut plan = with_probes(consolidation_run().plan());
    let verify = position(&plan, StepKind::VerifyDestination);
    let row = crate::execution_gate::TEST_PROBE_EXECUTABLE;
    for check in &mut plan.steps[verify].verification {
        match check {
            VerificationCheck::ExecutablesOwnedByDestination { executables } => {
                *executables = vec![row.to_owned()];
            }
            VerificationCheck::LivenessProbe { executable, argv } => {
                *executable = row.to_owned();
                *argv = vec!["--init".to_owned()];
            }
            _ => {}
        }
    }
    assert!(crate::execution_refusals(&plan)
        .refusals()
        .iter()
        .any(|r| matches!(r, ExecutionRefusal::ProbeIsNotTheTables { .. })));
}

/// No facts, and no package to judge them against, are never a yes.
#[test]
fn no_facts_and_no_destination_package_are_never_a_yes() {
    let holds = DestinationFacts::Ownership(OwnershipFacts::Owner {
        name: "gsed".to_owned(),
        version: "1.0".to_owned(),
        active: Some("1.0".to_owned()),
        versions_present: 1,
        held: None,
    });
    let unchecked = CheckAnswer::Unchecked(UncheckedCause::NotObserved);
    assert_eq!(
        crate::destination_judges::answer_from_facts(
            &consolidation_run(),
            &DestinationFacts::NotObserved
        ),
        unchecked
    );
    let no_package = consolidation_run().map_plan(|plan| plan.destination_package = None);
    assert_eq!(
        crate::destination_judges::answer_from_facts(&no_package, &holds),
        unchecked
    );
}
