// SPDX-License-Identifier: Apache-2.0

//! Every decision the run loop makes, as pure functions (DP-024, SE-092).
//!
//! The loop itself — build a read, run it, hand back what came out — is an I/O
//! shell in the command-line crate (`crates/cli/src/executor/`). It decides
//! nothing: it asks [`next_action`] what to do with each step, runs only what
//! that answers [`Action::Dispatch`] for, asks [`judge_step`] what the reads
//! established, and asks [`outcome_of`] how the run ended. Nothing here runs
//! anything, reads a clock or names a provider (INV-17).
//!
//! **The rules, in the order they apply.**
//!
//! 1. The execution gate is consulted before the first step and a plan it
//!    refused runs nothing (`execution_gate`, SE-050 "Not closed" 1). That is
//!    the shell's first line; everything below is defence in depth behind it,
//!    because in this build the gate refuses every plan the planner builds.
//! 2. A plan carrying `MANUAL_REVIEW` halts before any dispatch, whichever step
//!    is asked about (INV-19): [`next_action`] checks the whole plan first.
//! 3. **The dispatch guard comes before every other question about a step.** A
//!    mutating step is never answered [`Action::Dispatch`]: with a command it
//!    halts ([`HaltReason::UnexecutedMutation`]), without one it is passed over
//!    as [`Disposition::NotDispatchedMutating`]. The guard is keyed on
//!    `StepKind::is_mutating` alone and reads neither the build's mutation
//!    constant nor the plan's own claim about it, so flipping either cannot
//!    reach a provider (A11).
//! 4. `SNAPSHOT_BEFORE` and `RECORD_HISTORY` are not performed by this build
//!    ([`Disposition::NotPerformedInThisBuild`]), and a run that skipped one is
//!    never [`RunOutcome::Completed`] (A6).
//! 5. A verification step passes only when every check it carries is evaluable
//!    on that step kind and backed by a command whose read passed; a
//!    `PACKAGE_NEWLY_PRESENT` also needs the destination package absent from the
//!    snapshot taken before the run. Anything else halts as
//!    [`HaltReason::Unevaluable`] — never a pass. `VERIFY_RESOLUTION` has no
//!    evaluation at all and halts the same way.
//! 6. A read is judged by `verification::evaluate`, never by a process's exit
//!    status alone (`04-migration-and-safety.md`, "The install command exited
//!    0 is not verification"): a read that did not finish, or was cut at the
//!    cap, certifies nothing.
//! 7. What is kept of a read is [`ReadEvidence`] — the result, the line count
//!    and the truncation flag. There is no field for output, so a record
//!    cannot carry a machine's inventory (`plan.v1.md`, obligation 3).
//!
//! **History (obligation 12).** A mutating run writes a history record on
//! every way it ends. This build dispatches no mutating step, so no run it can
//! execute is a mutating run and nothing is persisted: what the loop returns
//! is the per-step [`StepRecord`] list and the [`RunOutcome`], which is the
//! material a record is built from. The record type and its append-only store
//! are owed by the history slice (S8).

use crate::execution_gate::{evaluable_in_this_build, GateReport};
use macportsbrew_domain::{
    evaluate, expectation_for, Capability, CapabilityState, Elevation, ErrorId, ExpectationError,
    FailureCause, MigrationPlan, ProviderPresence, ReadEvidence, ReadOutput, Snapshot, StepKind,
    SuccessSemantic, Verdict, VerificationCheck,
};
use serde::Serialize;

/// The exit code of a partial success (`06-cli-and-ux.md`; INV-03). No
/// `ErrorId` carries it: a partial success is an outcome, not an error.
pub const PARTIAL_SUCCESS_EXIT_CODE: i32 = 3;

/// What became of one step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "disposition")]
pub enum Disposition {
    /// A verification step whose every check was evaluated and passed.
    Verified {
        /// One per command, in order.
        evidence: Vec<ReadEvidence>,
    },
    /// `SNAPSHOT_BEFORE` or `RECORD_HISTORY`: not performed by this build.
    NotPerformedInThisBuild,
    /// A mutating step with no command, passed over and not dispatched.
    NotDispatchedMutating,
    /// The step stopped the run.
    Halted {
        /// Why.
        reason: HaltReason,
        /// The reads made before it stopped, one per command that ran.
        evidence: Vec<ReadEvidence>,
    },
    /// The run stopped before this step.
    NotReached,
}

/// Why a step stopped the run. Command positions count from 0 in the step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "halt")]
pub enum HaltReason {
    /// The plan stops at a `MANUAL_REVIEW` step (INV-19), named here.
    ManualReview {
        /// The `MANUAL_REVIEW` step's index.
        step: u32,
    },
    /// A mutating step carrying a command: never handed to a runner.
    UnexecutedMutation,
    /// A step this build does not perform, on a run that carried out nothing
    /// else — so not a success of any kind.
    NotPerformedInThisBuild,
    /// The step's provider has no read site: it was not detected.
    SiteUnavailable {
        /// The command.
        command: usize,
    },
    /// The read site refused to build the command (program, prefix, argv).
    SiteRefused {
        /// The command.
        command: usize,
    },
    /// No invocation could be recorded at all.
    TransportFailed {
        /// The command.
        command: usize,
        /// The error the runner reported.
        error: ErrorId,
    },
    /// No expectation could be derived for the command.
    Expectation {
        /// The command.
        command: usize,
        /// Why.
        error: ExpectationError,
    },
    /// A read was judged and failed — including one that did not finish or
    /// was truncated.
    VerificationFailed {
        /// The command.
        command: usize,
        /// The check's failure.
        cause: FailureCause,
    },
    /// A verification command whose elevation is not `NONE` (INV-06). A read
    /// is never run elevated, and one the plan says needs elevation, or whose
    /// elevation nobody established, is not run at all: fail closed. The gate
    /// refuses the same plan first (`ELEVATION_REQUIRED`, `ELEVATION_UNKNOWN`).
    ElevationNotNone {
        /// The command.
        command: usize,
    },
    /// A check that cannot be shown to hold. Never a pass.
    Unevaluable {
        /// The check's kind, when one is to blame — the kind only: a check's
        /// own fields are plan text, and a halt reason is recorded.
        check: Option<CheckKind>,
        /// Why it cannot be evaluated.
        why: Unevaluable,
    },
    /// The records handed back do not match the plan (out of order, missing
    /// or extra). Fails closed.
    RecordsDoNotMatchPlan,
}

/// A verification check's kind, without the plan text some checks carry
/// (executables, a probe's flag), so a halt reason can be recorded and shown
/// without redaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckKind {
    /// `PROVIDER_REPORTS_ACTIVE`.
    ProviderReportsActive,
    /// `PROVIDER_INTEGRITY_CHECK`.
    ProviderIntegrityCheck,
    /// `EXECUTABLES_OWNED_BY_DESTINATION`.
    ExecutablesOwnedByDestination,
    /// `LIVENESS_PROBE`.
    LivenessProbe,
    /// `NO_CROSS_PROVIDER_LINKAGE`.
    NoCrossProviderLinkage,
    /// `PACKAGE_ABSENT`.
    PackageAbsent,
    /// `PACKAGE_NEWLY_PRESENT`.
    PackageNewlyPresent,
    /// A check this build does not recognise.
    Unknown,
}

impl From<&VerificationCheck> for CheckKind {
    fn from(check: &VerificationCheck) -> Self {
        match check {
            VerificationCheck::ProviderReportsActive => Self::ProviderReportsActive,
            VerificationCheck::ProviderIntegrityCheck => Self::ProviderIntegrityCheck,
            VerificationCheck::ExecutablesOwnedByDestination { .. } => {
                Self::ExecutablesOwnedByDestination
            }
            VerificationCheck::LivenessProbe { .. } => Self::LivenessProbe,
            VerificationCheck::NoCrossProviderLinkage => Self::NoCrossProviderLinkage,
            VerificationCheck::PackageAbsent => Self::PackageAbsent,
            VerificationCheck::PackageNewlyPresent => Self::PackageNewlyPresent,
            VerificationCheck::Unknown => Self::Unknown,
        }
    }
}

/// Why a check, or a step, cannot be evaluated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Unevaluable {
    /// The step kind has no evaluation in this build (`VERIFY_RESOLUTION`).
    NoEvaluationForStepKind,
    /// A verification step with no command to read.
    NoCommand,
    /// A verification step with no check to read it against.
    NoCheck,
    /// A check this build cannot evaluate on this step kind.
    CheckHasNoEvaluation,
    /// No command whose read passed answers this check.
    NoPassingRead,
    /// Fewer reads came back than the step has commands.
    MissingRead,
    /// `PACKAGE_NEWLY_PRESENT`, but the snapshot before the run cannot say
    /// whether the destination package was absent then.
    PresenceBeforeUnknown,
    /// `PACKAGE_NEWLY_PRESENT`, but the destination package was already
    /// present before the run: presence now proves nothing happened.
    PresentBeforeTheRun,
}

/// One step's record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepRecord {
    /// The step's index, as the plan numbers it.
    pub index: u32,
    /// The step's kind.
    pub kind: StepKind,
    /// What became of it.
    pub disposition: Disposition,
}

/// What the loop does with a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Run the step's commands through its provider's read site.
    Dispatch,
    /// Record this and go on without running anything.
    PassOver(Disposition),
    /// Record this as the step's halt and stop.
    Halt(HaltReason),
}

/// One command's read, as the shell obtained it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepRead<'a> {
    /// The process ran (to whatever end) and this is what came out.
    Output(ReadOutput<'a>),
    /// No read site for the step's provider.
    SiteUnavailable,
    /// The read site refused the command.
    SiteRefused,
    /// No invocation could be recorded.
    TransportFailed(ErrorId),
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "outcome")]
pub enum RunOutcome {
    /// Every step was carried out and every check passed.
    Completed,
    /// The execution gate refused the plan; nothing ran. The refusals
    /// themselves quote plan text, so they are rendered by the caller with
    /// the home redacted, never carried here ([`refused`]).
    Refused {
        /// How many refusals the gate reported.
        refusals: usize,
    },
    /// The run stopped, having carried out nothing.
    Halted {
        /// The step it stopped at (0 for the run as a whole).
        step: u32,
        /// Why.
        reason: HaltReason,
    },
    /// Some steps were carried out and some were not (INV-03): never success.
    ///
    /// **Not a partial migration in this build.** No run here dispatches a
    /// mutating step, so a partial success is a verification that passed
    /// beside one that did not, or a skipped bracket; history must not record
    /// it as a migration left half done (SE-020; `plan.v1.md` obligation 12).
    PartialSuccess {
        /// The step that stopped the run, or else the first step that was
        /// not carried out.
        step: u32,
        /// Why that step was not carried out.
        reason: HaltReason,
    },
}

impl RunOutcome {
    /// The exit code: 0 completed, 3 partial success, 4 for a gate refusal
    /// (as `GateReport::exit_code`), and for a halt the code of its error.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Completed => 0,
            Self::Refused { .. } => ErrorId::SafetyBlocked.exit_code(),
            Self::Halted { reason, .. } => reason.error_id().exit_code(),
            Self::PartialSuccess { .. } => PARTIAL_SUCCESS_EXIT_CODE,
        }
    }
}

impl HaltReason {
    /// The error a halt reports. A step this build cannot perform is a safety
    /// block, like the gate's refusal of it — one code, for the reason
    /// `GateReport::exit_code` gives; a check that cannot be shown to hold is
    /// unverifiable; a transport failure keeps the runner's own error.
    #[must_use]
    pub fn error_id(&self) -> ErrorId {
        match self {
            Self::ManualReview { .. }
            | Self::UnexecutedMutation
            | Self::NotPerformedInThisBuild
            | Self::SiteUnavailable { .. }
            | Self::SiteRefused { .. }
            | Self::ElevationNotNone { .. }
            | Self::VerificationFailed { .. } => ErrorId::SafetyBlocked,
            Self::Expectation { .. } | Self::Unevaluable { .. } => ErrorId::SafetyUnverifiable,
            Self::TransportFailed { error, .. } => *error,
            Self::RecordsDoNotMatchPlan => ErrorId::Internal,
        }
    }
}

/// The run outcome of a plan the execution gate refused, or `None` when the
/// gate passed it: the one place a refusal becomes an outcome.
#[must_use]
pub fn refused(report: &GateReport) -> Option<RunOutcome> {
    if report.may_execute() {
        None
    } else {
        Some(RunOutcome::Refused {
            refusals: report.refusals().len(),
        })
    }
}

/// A plan together with the snapshot it was planned from.
///
/// **The fields are private and only the planner constructs one**
/// (`planner::plan_run`), so the snapshot a run judges `PACKAGE_NEWLY_PRESENT`
/// against is the one the plan was built from, never one a caller supplies
/// beside it (security review of S5, M2). The plan may be narrowed for a test
/// through [`PlannedRun::map_plan`] — the gate judges whatever plan results —
/// but the snapshot cannot be replaced, and the method exists only in test
/// builds.
#[derive(Clone, Debug)]
pub struct PlannedRun {
    plan: MigrationPlan,
    before: Snapshot,
}

impl PlannedRun {
    /// Pairs a plan with the snapshot it was planned from. Crate-private:
    /// the planner is the only caller.
    pub(crate) fn new(plan: MigrationPlan, before: Snapshot) -> Self {
        Self { plan, before }
    }

    /// The plan.
    #[must_use]
    pub fn plan(&self) -> &MigrationPlan {
        &self.plan
    }

    /// The snapshot the plan was built from.
    #[must_use]
    pub fn before(&self) -> &Snapshot {
        &self.before
    }

    /// The same run with its plan changed by `edit`; the snapshot is kept.
    ///
    /// **Test builds only** (and the non-default `test-support` feature, which
    /// only the command-line crate's tests enable): in the shipped build the
    /// snapshot can be paired with no plan but the one the planner built from
    /// it. A word scan keeps every use of it out of product source.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn map_plan(mut self, edit: impl FnOnce(&mut MigrationPlan)) -> Self {
        edit(&mut self.plan);
        self
    }
}

/// What the loop does with the step at `position` (from 0) of `plan`, given
/// the records of the steps before it.
///
/// Pure, and the only source of [`Action::Dispatch`]. Fails closed: records
/// that are not exactly one per earlier step, or that already hold a halt,
/// halt the run.
#[must_use]
pub fn next_action(plan: &MigrationPlan, position: usize, records: &[StepRecord]) -> Action {
    let in_step = records.len() == position
        && records
            .iter()
            .zip(&plan.steps)
            .all(|(r, s)| r.index == s.index && r.kind == s.kind)
        && !records.iter().any(|r| {
            matches!(
                r.disposition,
                Disposition::Halted { .. } | Disposition::NotReached
            )
        });
    let Some(step) = plan.steps.get(position).filter(|_| in_step) else {
        return Action::Halt(HaltReason::RecordsDoNotMatchPlan);
    };
    // INV-19, over the whole plan, before anything about this step.
    if let Some(review) = plan.steps.iter().find(|s| s.kind == StepKind::ManualReview) {
        return Action::Halt(HaltReason::ManualReview { step: review.index });
    }
    // The dispatch guard: before every other question about the step.
    if step.kind.is_mutating() {
        return guard(step.commands.is_empty());
    }
    match step.kind {
        StepKind::SnapshotBefore | StepKind::RecordHistory => {
            Action::PassOver(Disposition::NotPerformedInThisBuild)
        }
        StepKind::VerifyResolution => Action::Halt(HaltReason::Unevaluable {
            check: None,
            why: Unevaluable::NoEvaluationForStepKind,
        }),
        StepKind::VerifyDestination | StepKind::VerifyRemoval => {
            if step.commands.is_empty() {
                return Action::Halt(unevaluable(None, Unevaluable::NoCommand));
            }
            if step.verification.is_empty() {
                return Action::Halt(unevaluable(None, Unevaluable::NoCheck));
            }
            if let Some(check) = step
                .verification
                .iter()
                .find(|c| !evaluable_in_this_build(step.kind, c))
            {
                return Action::Halt(unevaluable(Some(check), Unevaluable::CheckHasNoEvaluation));
            }
            // INV-06: a read runs unelevated or not at all.
            if let Some(command) = step
                .commands
                .iter()
                .position(|c| c.elevation != Elevation::None)
            {
                return Action::Halt(HaltReason::ElevationNotNone { command });
            }
            Action::Dispatch
        }
        // Handled by the guard above; listed so a new kind must be placed.
        StepKind::AddDestination
        | StepKind::SetPrimary
        | StepKind::RemoveSource
        | StepKind::CleanupReview => guard(step.commands.is_empty()),
        StepKind::ManualReview => Action::Halt(HaltReason::ManualReview { step: step.index }),
    }
}

/// The dispatch guard's answer for a mutating step: never a dispatch.
fn guard(commandless: bool) -> Action {
    if commandless {
        Action::PassOver(Disposition::NotDispatchedMutating)
    } else {
        Action::Halt(HaltReason::UnexecutedMutation)
    }
}

fn unevaluable(check: Option<&VerificationCheck>, why: Unevaluable) -> HaltReason {
    HaltReason::Unevaluable {
        check: check.map(CheckKind::from),
        why,
    }
}

/// What the reads of the step at `position` of `run`'s plan established,
/// judged against the snapshot that plan was built from. `reads` are the
/// step's commands' reads in order, ending early only where a read could not
/// be obtained.
#[must_use]
pub fn judge_step(run: &PlannedRun, position: usize, reads: &[StepRead<'_>]) -> Disposition {
    let (plan, before) = (run.plan(), run.before());
    let halted = |reason, evidence| Disposition::Halted { reason, evidence };
    let Some(step) = plan.steps.get(position) else {
        return halted(HaltReason::RecordsDoNotMatchPlan, Vec::new());
    };
    let mut evidence = Vec::new();
    let mut passed = Vec::new();
    for (command, read) in reads.iter().enumerate() {
        let output = match *read {
            StepRead::Output(output) => output,
            StepRead::SiteUnavailable => {
                return halted(HaltReason::SiteUnavailable { command }, evidence)
            }
            StepRead::SiteRefused => return halted(HaltReason::SiteRefused { command }, evidence),
            StepRead::TransportFailed(error) => {
                return halted(HaltReason::TransportFailed { command, error }, evidence)
            }
        };
        let expectation = match expectation_for(plan, position, command) {
            Ok(expectation) => expectation,
            Err(error) => return halted(HaltReason::Expectation { command, error }, evidence),
        };
        let verdict = evaluate(&expectation, &output);
        evidence.push(ReadEvidence::of(&output, verdict));
        if let Verdict::Failed(cause) = verdict {
            return halted(HaltReason::VerificationFailed { command, cause }, evidence);
        }
        passed.push(expectation.semantic());
    }
    if reads.len() != step.commands.len() {
        return halted(unevaluable(None, Unevaluable::MissingRead), evidence);
    }
    if step.verification.is_empty() {
        return halted(unevaluable(None, Unevaluable::NoCheck), evidence);
    }
    for check in &step.verification {
        if !evaluable_in_this_build(step.kind, check) {
            return halted(
                unevaluable(Some(check), Unevaluable::CheckHasNoEvaluation),
                evidence,
            );
        }
        if !passed.iter().any(|s| answers(check, *s)) {
            return halted(
                unevaluable(Some(check), Unevaluable::NoPassingRead),
                evidence,
            );
        }
        if *check == VerificationCheck::PackageNewlyPresent {
            match destination_before(plan, before) {
                Before::Absent => {}
                Before::Present => {
                    return halted(
                        unevaluable(Some(check), Unevaluable::PresentBeforeTheRun),
                        evidence,
                    )
                }
                Before::Unknown => {
                    return halted(
                        unevaluable(Some(check), Unevaluable::PresenceBeforeUnknown),
                        evidence,
                    )
                }
            }
        }
    }
    Disposition::Verified { evidence }
}

/// Whether a passed read of `semantic` answers `check`. Presence is answered
/// by a destination read, absence by an inventory read; nothing else is
/// answered by any read (`evaluable_in_this_build` has already refused it).
fn answers(check: &VerificationCheck, semantic: SuccessSemantic) -> bool {
    match check {
        VerificationCheck::ProviderReportsActive | VerificationCheck::PackageNewlyPresent => {
            matches!(
                semantic,
                SuccessSemantic::ExitStatus | SuccessSemantic::StdoutNamesPackage
            )
        }
        VerificationCheck::PackageAbsent => semantic == SuccessSemantic::StdoutOmitsPackage,
        VerificationCheck::ProviderIntegrityCheck
        | VerificationCheck::ExecutablesOwnedByDestination { .. }
        | VerificationCheck::LivenessProbe { .. }
        | VerificationCheck::NoCrossProviderLinkage
        | VerificationCheck::Unknown => false,
    }
}

enum Before {
    Absent,
    Present,
    Unknown,
}

/// Whether the destination package was held by the destination provider in
/// the snapshot taken before the run. Only a readable inventory answers: a
/// broken provider, one whose inventory is not recorded as readable, a
/// provider with no entry, and a plan with no destination name are all
/// `Unknown`, and an unknown is never a yes.
fn destination_before(plan: &MigrationPlan, before: &Snapshot) -> Before {
    let Some(name) = plan.destination_name() else {
        return Before::Unknown;
    };
    let Some(provider) = before.providers.get(&plan.destination) else {
        return Before::Unknown;
    };
    match provider.detection.presence {
        ProviderPresence::Absent => Before::Absent,
        ProviderPresence::Broken { .. } => Before::Unknown,
        ProviderPresence::Present => {
            if provider
                .capabilities
                .capabilities
                .get(&Capability::MachineReadableInventory)
                != Some(&CapabilityState::Available)
            {
                Before::Unknown
            } else if provider
                .inventory
                .installed
                .iter()
                .any(|p| p.identity.name == name.as_str())
            {
                Before::Present
            } else {
                Before::Absent
            }
        }
    }
}

/// How a run whose steps ended as `records` ended.
///
/// Completed only when every step of the plan was carried out: a verification
/// passed, nothing halted, and nothing was passed over or skipped. A run that
/// carried out some steps and not others is a partial success (INV-03), and
/// one that carried out none is halted — never completed.
#[must_use]
pub fn outcome_of(plan: &MigrationPlan, records: &[StepRecord]) -> RunOutcome {
    let matches_plan = records.len() == plan.steps.len()
        && records
            .iter()
            .zip(&plan.steps)
            .all(|(r, s)| r.index == s.index && r.kind == s.kind);
    if records.is_empty() || !matches_plan {
        return RunOutcome::Halted {
            step: 0,
            reason: HaltReason::RecordsDoNotMatchPlan,
        };
    }
    let Some(first_skipped) = records
        .iter()
        .find(|r| !matches!(r.disposition, Disposition::Verified { .. }))
    else {
        return RunOutcome::Completed;
    };
    let halt = records.iter().find_map(|r| match &r.disposition {
        Disposition::Halted { reason, .. } => Some((r.index, reason.clone())),
        _ => None,
    });
    let carried_out = records
        .iter()
        .any(|r| matches!(r.disposition, Disposition::Verified { .. }));
    if carried_out {
        let (step, reason) =
            halt.unwrap_or((first_skipped.index, HaltReason::NotPerformedInThisBuild));
        return RunOutcome::PartialSuccess { step, reason };
    }
    let (step, reason) = halt.unwrap_or((first_skipped.index, HaltReason::NotPerformedInThisBuild));
    RunOutcome::Halted { step, reason }
}

#[cfg(test)]
mod tests;
