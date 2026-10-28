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
    let passed = judge(&run, verify, &[StepRead::Output(finished(&named))]);
    assert!(
        matches!(passed, Disposition::Verified { ref evidence } if evidence.len() == 1),
        "{passed:?}"
    );
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
    // A step whose checks no command answers: no command, nothing passed.
    let bare = run.clone().map_plan(|p| p.steps[verify].commands.clear());
    assert!(matches!(
        judge(&bare, verify, &[]),
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
        judge(&extra, verify, &[StepRead::Output(finished(&named))]),
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
        judge(&misplaced, verify, &[StepRead::Output(finished(&named))]),
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
    match judge(&run, verify, &[StepRead::Output(finished(&named))]) {
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

// --- Under a mutation allowance (pass 08 slice 4) -------------------------

use crate::execution_gate::{clear, execution_refusals_for, ExecutionRefusal, MutationAllowance};
use crate::history::run_record;
use macportsbrew_domain::{RunStart, RunStatus};

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
        &configured(run.before().clone()),
        &macportsbrew_domain::ProviderConfiguration::Read { files: Vec::new() },
    )
    .expect("an empty configuration outranks nothing")
}

/// How the shells judge the step at `position`: a `VERIFY_DESTINATION` by
/// `certify_destination` with a fresh token, every other step by `judge_step`.
fn judge(run: &PlannedRun, position: usize, reads: &[StepRead<'_>]) -> Disposition {
    if run.plan().steps[position].kind == StepKind::VerifyDestination {
        certify_destination(run, position, token(run), reads)
    } else {
        judge_step(run, position, reads)
    }
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
/// cannot stand in: since D11 its `VERIFY_DESTINATION` carries no check this
/// build can evaluate, and the gate refuses it as asserting nothing.
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
    // under an allowance, because consent is not wired.
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
/// (`PROVIDER_REPORTS_ACTIVE` and four more are refused by the gate), so the
/// narrowed plan carries `PACKAGE_NEWLY_PRESENT` only; a flip tripwire holds
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
    now.providers
        .get_mut(&ProviderId::new(HB))
        .expect("homebrew")
        .inventory
        .metadata
        .get_mut("gnu-sed")
        .expect("metadata")
        .defines_service = true;
    let fresh = replanned(&now);
    assert_ne!(*run.plan(), fresh, "the planner sees the service");
    assert_eq!(
        plan_still_holds(run.plan(), &fresh, run.before(), &now),
        Err(StateFact::Classification)
    );
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
        p.steps.retain(|s| {
            !matches!(
                s.kind,
                StepKind::SetPrimary
                    | StepKind::RemoveSource
                    | StepKind::VerifyRemoval
                    | StepKind::CleanupReview
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
    let named = naming_destination(run.plan());
    let reads = [StepRead::Output(finished(&named))];
    assert!(matches!(
        certify_destination(&run, verify, token(&run), &reads),
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
            &[StepRead::Output(finished(&named))]
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
        certify_destination(&run, removal, token(&run), &[]),
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
        let run = forced_run(from);
        let plan = run.plan();
        let remove = position(plan, StepKind::RemoveSource);
        let reverify = reverify_for(plan, remove).expect("a removal is re-read");
        let named = naming_destination(plan);
        assert_eq!(
            destination_still_verified(
                &run,
                &reverify,
                token(&run),
                &[StepRead::Output(finished(&named))]
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
        assert_eq!(
            destination_still_verified(&run, &reverify, token(&run), &[StepRead::Output(gone)]),
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
                    &[StepRead::Output(finished("  unrelated 1.0\n"))]
                ),
                Err(Precondition::DestinationNotReverified),
                "{from}"
            );
        }
    }
}

#[test]
fn invariant_01_removal_refused_when_the_reread_is_truncated_failed_or_unjudged() {
    let run = forced_run(MP);
    let plan = run.plan();
    let reverify =
        reverify_for(plan, position(plan, StepKind::RemoveSource)).expect("a removal is re-read");
    let named = naming_destination(plan);
    let reads: Vec<Vec<StepRead<'_>>> = vec![
        vec![StepRead::Output(ReadOutput {
            end: ProcessEnd::Exited { code: 0 },
            stdout: &named,
            truncated: true,
        })],
        vec![StepRead::Output(ReadOutput {
            end: ProcessEnd::DidNotFinish,
            stdout: &named,
            truncated: false,
        })],
        vec![StepRead::Output(ReadOutput {
            end: ProcessEnd::Exited { code: 1 },
            stdout: &named,
            truncated: false,
        })],
        vec![StepRead::SiteRefused],
        vec![StepRead::SiteUnavailable],
        vec![StepRead::TransportFailed(ErrorId::ProviderTimeout)],
        vec![],
    ];
    for read in &reads {
        assert_eq!(
            destination_still_verified(&run, &reverify, token(&run), read),
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
    let named = naming_destination(plan);
    assert!(matches!(
        certify_destination(
            &run,
            reverify.verify_position(),
            token(&run),
            &[StepRead::Output(finished(&named))]
        ),
        Disposition::Halted { .. }
    ));
    assert_eq!(
        destination_still_verified(
            &run,
            &reverify,
            token(&run),
            &[StepRead::Output(finished(&named))]
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
    let named = naming_destination(plan);
    let source = plan.source.clone();
    assert_eq!(
        destination_still_verified(
            &run,
            &reverify,
            token_for(&run, &source),
            &[StepRead::Output(finished(&named))]
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
    let named = naming_destination(plan);
    assert_eq!(
        destination_still_verified(
            &run,
            &reverify,
            token(&run),
            &[StepRead::Output(finished(&named))]
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
    let named_gui = naming_destination(gui.plan());
    assert!(
        matches!(
            certify_destination(
                &gui,
                verify,
                token(&gui),
                &[StepRead::Output(finished(&named_gui))]
            ),
            Disposition::Verified { .. }
        ),
        "the GUI-app control certifies"
    );
    let plan = gui.plan();
    let reverify =
        reverify_for(plan, position(plan, StepKind::RemoveSource)).expect("a removal is re-read");
    let named = naming_destination(plan);
    assert_eq!(
        destination_still_verified(
            &gui,
            &reverify,
            token(&gui),
            &[StepRead::Output(finished(&named))]
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
