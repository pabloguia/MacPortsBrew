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
//! 4. `SNAPSHOT_BEFORE` and `RECORD_HISTORY` are performed only under a
//!    [`MutationAllowance`] ([`Action::TakeSnapshot`], [`Action::RecordHistory`]);
//!    the read-only loop passes them over
//!    ([`Disposition::NotPerformedInThisBuild`]), and a run that skipped one is
//!    never [`RunOutcome::Completed`] (A6). A mutating command is never
//!    dispatched unless `SNAPSHOT_BEFORE` was recorded taken.
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
//! 8. **A `VERIFY_DESTINATION` is certified only after its provider's
//!    configuration was re-checked** (SE-107 C1): [`next_action`] answers
//!    [`Action::Certify`] for it, [`judge_step`] refuses it
//!    ([`Unevaluable::NotCertifiedHere`]) whatever its reads say, and
//!    [`certify_destination`] — which takes a [`ConfigurationRechecked`] token
//!    by value, constructed only by the re-check — is the only path to its
//!    `Verified`.
//! 9. **The destination is read again immediately before `REMOVE_SOURCE`**
//!    (slice 6 part C; work order 16 part S, §6): [`reverify_for`] names the
//!    `VERIFY_DESTINATION` whose own commands are re-read, carried in
//!    [`Action::DispatchMutating`], and [`destination_still_verified`] must
//!    answer `Ok` before each removal command is handed over.
//!
//! **History (obligation 12).** A mutating run writes a history record on
//! every way it ends — but only under a [`MutationAllowance`], which no shipped
//! build has, so what the read-only loop returns is the per-step
//! [`StepRecord`] list and the [`RunOutcome`], the material a record is built
//! from (`crate::history::run_record`). Under an allowance the shell writes the
//! snapshot first ([`Action::TakeSnapshot`]) and the record last
//! ([`Action::RecordHistory`], stored pending: a record never certifies its own
//! write, and what confirms it is a read-back, `crate::history::read_back_matches`).

use crate::configuration_recheck::ConfigurationRechecked;
use crate::execution_gate::{answers, evaluable_in_this_build, GateReport, MutationAllowance};
use crate::planner::{elevation_for, package_kind_in};
use crate::provider_commands;
use crate::verification_json::linked_report;
use macportsbrew_domain::{
    evaluate_with, expectation_for, Capability, CapabilityState, CommandEnd, CommandTarget,
    Elevation, ErrorId, ExpectationError, FailureCause, MigrationPlan, PackageKind, ProviderId,
    ProviderPresence, ReadEvidence, ReadOutput, Snapshot, StepKind, Verdict, VerificationCheck,
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
    /// `SNAPSHOT_BEFORE` or `RECORD_HISTORY` on the read-only loop, which
    /// performs neither (only a run under a [`MutationAllowance`] does).
    NotPerformedInThisBuild,
    /// `SNAPSHOT_BEFORE` performed: the snapshot was written durably and read
    /// back, before the marker and before the first change (pass 08 slice 5,
    /// D1).
    SnapshotTaken,
    /// `RECORD_HISTORY` performed: the step the record is built with. **Pending
    /// by design** (D3): a record never certifies its own write, so whether it
    /// was written and read back is the report's fact, not the step's.
    RecordPendingReadBack,
    /// A mutating step with no command, passed over and not dispatched.
    NotDispatchedMutating,
    /// A mutating step dispatched under a [`MutationAllowance`] whose every
    /// command exited 0. Carried out, and **not thereby a success**: an exit
    /// status is never verification (`04-migration-and-safety.md`), so the
    /// `VERIFY_*` step after it judges.
    Performed {
        /// One per command, in order; every one exited 0.
        ends: Vec<CommandEnd>,
    },
    /// The step stopped the run.
    Halted {
        /// Why.
        reason: HaltReason,
        /// The reads made before it stopped, one per command that ran.
        evidence: Vec<ReadEvidence>,
        /// How each mutating command that was handed to the site before it
        /// stopped ended (D4). Empty for a read step and before any dispatch.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        ends: Vec<CommandEnd>,
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
    /// A mutating command exited non-zero or did not finish. The machine may
    /// have changed: the step counts as dispatched.
    MutationFailed {
        /// The command.
        command: usize,
    },
    /// The mutation site refused the command. Counted as dispatched, failing
    /// toward a record (D4): the site was handed it.
    MutationRefused {
        /// The command.
        command: usize,
    },
    /// A mutating step carrying its own verification checks: this build
    /// judges a check only on a `VERIFY_*` step, and a check is never
    /// silently dropped (D3).
    MutatingStepCarriesVerification,
    /// A check made immediately before a mutating command did not hold, so
    /// that command was not handed to the site (D7). Commands before it may
    /// have run: their ends are on the disposition, and decide the dispatch.
    PreconditionFailed {
        /// The command that was not handed over.
        command: usize,
        /// Which check.
        why: Precondition,
    },
    /// `SNAPSHOT_BEFORE` could not be performed: the snapshot could not be
    /// published durably, or did not read back as the one that was written. The
    /// run changed nothing and records nothing (pass 08 slice 5, D1).
    SnapshotNotTaken,
    /// The fresh capture taken under the lock does not plan to the plan this
    /// run validated (D6). Nothing was dispatched.
    StateChanged {
        /// The first fact that moved.
        fact: StateFact,
    },
}

/// A check made before each mutating command (D7), by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Precondition {
    /// The step names no provider.
    NoProvider,
    /// The run lock is no longer the one this run took.
    LockLost,
    /// The provider's configuration could not be re-read, or the provider
    /// detected now is not the one the plan was built for.
    ConfigurationUnreadable,
    /// The provider's configuration, re-read, can change how it runs, or
    /// could not be shown not to (DP-027 (b)).
    ConfigurationDrift,
    /// The configuration token is for another provider.
    WrongProviderToken,
    /// The destination, read again with its `VERIFY_DESTINATION`'s own
    /// commands immediately before a removal command, did not certify: the
    /// package vanished or is no longer active, a read was truncated, failed
    /// or could not be judged, or a re-read command's elevation, re-derived
    /// from the command table, is not `NONE` (work order 16 part S, §6, D5).
    DestinationNotReverified,
}

/// Which fact moved between the capture a plan was built from and the one
/// taken under the lock (D6) — an identifier, never plan text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StateFact {
    /// A provider's presence or health.
    ProviderState,
    /// A provider's prefix or executable.
    ProviderLocation,
    /// A provider's capabilities, elevation included.
    ProviderCapability,
    /// The source package's presence or version.
    SourcePackage,
    /// The destination package's presence.
    DestinationPackage,
    /// A package that depends on the source package (INV-04).
    ReverseDependency,
    /// The package's classification, a service included (INV-19).
    Classification,
    /// The plans differ and no fact above explains it.
    PlanDiffers,
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
    /// `PROVIDER_REPORTS_ACTIVE` on a step that does not also carry
    /// `PACKAGE_NEWLY_PRESENT`, while the snapshot before the run shows the
    /// destination did not already hold the package: "active" is read from a
    /// whole inventory and does not prove this run installed it (04, check 1;
    /// SE-107). Only a consolidation — the package already held — is guarded
    /// by "active" alone.
    ActiveAloneAfterAnInstall,
    /// A `VERIFY_DESTINATION` handed to [`judge_step`]: it is certified only by
    /// [`certify_destination`], after a configuration re-check (SE-107 C1).
    NotCertifiedHere,
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
    /// Run the step's commands through its provider's read site, and judge them
    /// with [`judge_step`]. Never answered for a `VERIFY_DESTINATION`.
    Dispatch,
    /// A `VERIFY_DESTINATION`: re-read the destination provider's
    /// configuration, re-check it (`recheck_provider_configuration`), run the
    /// step's commands through its read site, and judge them with
    /// [`certify_destination`] and the re-check's token (SE-107 C1). A re-read
    /// that fails halts `PRECONDITION_FAILED` / `CONFIGURATION_UNREADABLE`, a
    /// re-check that fails `CONFIGURATION_DRIFT`, and nothing is read.
    Certify,
    /// Hand the step's commands to the mutation site, one at a time, each
    /// behind the lock check and the configuration re-check (D7). Answered
    /// only under a [`MutationAllowance`].
    ///
    /// `reverify` is `Some` for `REMOVE_SOURCE` only ([`reverify_for`]): then
    /// **before each command**, the destination's configuration is re-read and
    /// re-checked, the destination is read again with the named
    /// `VERIFY_DESTINATION`'s own commands, and [`destination_still_verified`]
    /// must answer `Ok` — before the lock check and the source's configuration
    /// re-check, so those two cheap confirmations are the freshest facts at
    /// dispatch (M1).
    DispatchMutating {
        /// The destination re-read a removal needs, when it is one.
        reverify: Option<Reverify>,
    },
    /// Write the snapshot of the capture taken under the lock (`SNAPSHOT_BEFORE`,
    /// D1). Answered only under a [`MutationAllowance`].
    TakeSnapshot,
    /// Record the run (`RECORD_HISTORY`, D2/D3): the step is recorded pending;
    /// the one write of the record and its read-back follow the loop. Answered
    /// only under a [`MutationAllowance`].
    RecordHistory,
    /// Record this and go on without running anything.
    PassOver(Disposition),
    /// Record this as the step's halt and stop.
    Halt(HaltReason),
}

/// The destination re-read that precedes each `REMOVE_SOURCE` command.
///
/// Opaque: [`reverify_for`] is its only constructor, so which step is re-read
/// and toward which provider is the application's decision, never the shell's
/// (V4, L5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reverify {
    verify_position: usize,
    provider: ProviderId,
}

impl Reverify {
    /// The position (from 0) of the `VERIFY_DESTINATION` whose own commands are
    /// read again.
    #[must_use]
    pub fn verify_position(&self) -> usize {
        self.verify_position
    }

    /// The provider read: the plan's destination.
    #[must_use]
    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }
}

/// The destination re-read the step at `position` needs before each of its
/// commands is dispatched: `Some` for a `REMOVE_SOURCE` with a
/// `VERIFY_DESTINATION` before it, naming that step and `plan.destination`;
/// `None` for every other step, `ADD_DESTINATION` included.
#[must_use]
pub fn reverify_for(plan: &MigrationPlan, position: usize) -> Option<Reverify> {
    let step = plan.steps.get(position)?;
    if step.kind != StepKind::RemoveSource {
        return None;
    }
    // The nearest one before the removal: the one guarding it (INV-01).
    let verify_position = plan.steps[..position]
        .iter()
        .rposition(|s| s.kind == StepKind::VerifyDestination)?;
    Some(Reverify {
        verify_position,
        provider: plan.destination.clone(),
    })
}

/// One mutating command's end, as the shell obtained it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationResult {
    /// The process ran, to this end.
    Ended(CommandEnd),
    /// The mutation site refused the command.
    Refused,
    /// No invocation could be recorded.
    TransportFailed(ErrorId),
    /// A check before the command did not hold; it was not handed over.
    PreconditionFailed(Precondition),
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
    /// beside one that did not, or a skipped bracket; history records it as
    /// `NO_CHANGE`, never as a migration left half done (SE-020; `plan.v1.md`
    /// obligation 12; `macportsbrew_domain::RunStatus`).
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
            | Self::VerificationFailed { .. }
            | Self::MutationFailed { .. }
            | Self::MutationRefused { .. }
            | Self::MutatingStepCarriesVerification
            | Self::PreconditionFailed { .. }
            | Self::SnapshotNotTaken
            | Self::StateChanged { .. } => ErrorId::SafetyBlocked,
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
    next_action_guarded(plan, position, records, None)
}

/// [`next_action`], and under a [`MutationAllowance`] the one source of
/// [`Action::DispatchMutating`] (D3). Without one it answers exactly as
/// [`next_action`] always has.
///
/// With one, a mutating step is dispatched only when **all** hold: it is
/// `ADD_DESTINATION` or `REMOVE_SOURCE`; it carries commands, each targeting a
/// provider; it carries no verification of its own; every command's
/// elevation, re-derived from the command table and never read from the
/// plan's field (obligation 9), is `NONE` and the plan's field agrees; and,
/// for `REMOVE_SOURCE`, the `VERIFY_DESTINATION` before it was recorded
/// verified (INV-01). `SET_PRIMARY` is a self-invocation of a layer that does
/// not exist and `CLEANUP_REVIEW` is a review: neither is ever dispatched.
#[must_use]
pub fn next_action_with(
    run: &PlannedRun,
    position: usize,
    records: &[StepRecord],
    allowance: Option<&MutationAllowance>,
) -> Action {
    let kind = package_kind_in(run.plan(), run.before());
    next_action_guarded(run.plan(), position, records, allowance.map(|a| (a, kind)))
}

/// The one implementation behind [`next_action`] and [`next_action_with`].
fn next_action_guarded(
    plan: &MigrationPlan,
    position: usize,
    records: &[StepRecord],
    allowance: Option<(&MutationAllowance, PackageKind)>,
) -> Action {
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
        return match allowance {
            None => guard(step.commands.is_empty()),
            Some((_, kind)) => mutating_action(plan, position, records, kind),
        };
    }
    match step.kind {
        StepKind::SnapshotBefore if allowance.is_some() => Action::TakeSnapshot,
        StepKind::RecordHistory if allowance.is_some() => Action::RecordHistory,
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
            if step.kind == StepKind::VerifyDestination {
                Action::Certify
            } else {
                Action::Dispatch
            }
        }
        // Handled by the guard above; listed so a new kind must be placed.
        StepKind::AddDestination
        | StepKind::SetPrimary
        | StepKind::RemoveSource
        | StepKind::CleanupReview => guard(step.commands.is_empty()),
        StepKind::ManualReview => Action::Halt(HaltReason::ManualReview { step: step.index }),
    }
}

/// What a mutating step gets under an allowance (D3). Every refusal here is a
/// halt or a pass-over; nothing below answers a dispatch unless every
/// condition held.
fn mutating_action(
    plan: &MigrationPlan,
    position: usize,
    records: &[StepRecord],
    kind: PackageKind,
) -> Action {
    let step = &plan.steps[position];
    let operation = match step.kind {
        StepKind::AddDestination => "add",
        StepKind::RemoveSource => "remove",
        // Never dispatched: a layer that does not exist, and a review.
        StepKind::SetPrimary | StepKind::CleanupReview => {
            return guard(step.commands.is_empty());
        }
        _ => return Action::Halt(HaltReason::RecordsDoNotMatchPlan),
    };
    if step.commands.is_empty() {
        return Action::PassOver(Disposition::NotDispatchedMutating);
    }
    if !step.verification.is_empty() {
        return Action::Halt(HaltReason::MutatingStepCarriesVerification);
    }
    // 04: a snapshot is taken before the first mutating step. Structural, so
    // it holds by construction and not only because the loop stops at a halt.
    if !snapshot_taken_before(plan, position, records) {
        return Action::Halt(HaltReason::UnexecutedMutation);
    }
    if step.kind == StepKind::RemoveSource && !destination_verified_before(plan, position, records)
    {
        return Action::Halt(HaltReason::UnexecutedMutation);
    }
    let table = provider_commands::load().unwrap_or_default();
    let entry = step.provider.as_ref().and_then(|p| table.get(p.as_str()));
    for (command, c) in step.commands.iter().enumerate() {
        if c.target != CommandTarget::Provider {
            return Action::Halt(HaltReason::UnexecutedMutation);
        }
        // Re-derived, never read from the plan; a mutating operation is
        // never relaxed by a capability, so no snapshot is consulted.
        let derived = elevation_for(entry, operation, kind, None);
        if derived != Elevation::None || c.elevation != Elevation::None {
            return Action::Halt(HaltReason::ElevationNotNone { command });
        }
    }
    dispatch_for(plan, position)
}

/// The dispatch of the mutating step at `position`, with its destination
/// re-read: a `REMOVE_SOURCE` for which [`reverify_for`] names no
/// `VERIFY_DESTINATION` halts rather than being dispatched without a re-read.
/// Defence in depth: [`destination_verified_before`] already requires that
/// step, so no plan reaches the halt today (spec-guardian, review of S-1).
fn dispatch_for(plan: &MigrationPlan, position: usize) -> Action {
    let reverify = reverify_for(plan, position);
    let removal = plan
        .steps
        .get(position)
        .is_some_and(|s| s.kind == StepKind::RemoveSource);
    if removal && reverify.is_none() {
        return Action::Halt(HaltReason::UnexecutedMutation);
    }
    Action::DispatchMutating { reverify }
}

/// Whether the `SNAPSHOT_BEFORE` before the step at `position` was recorded
/// taken (04, "Rollback and snapshots"). A missing, halted or passed-over
/// record is a no.
fn snapshot_taken_before(plan: &MigrationPlan, position: usize, records: &[StepRecord]) -> bool {
    plan.steps[..position]
        .iter()
        .position(|s| s.kind == StepKind::SnapshotBefore)
        .and_then(|at| records.get(at))
        .is_some_and(|r| {
            r.kind == StepKind::SnapshotBefore
                && matches!(r.disposition, Disposition::SnapshotTaken)
        })
}

/// Whether the `VERIFY_DESTINATION` that guards the step at `position` was
/// recorded verified (INV-01): it must come before, and its record must say
/// `Verified`. A missing, halted or passed-over record is a no.
fn destination_verified_before(
    plan: &MigrationPlan,
    position: usize,
    records: &[StepRecord],
) -> bool {
    plan.steps[..position]
        .iter()
        .position(|s| s.kind == StepKind::VerifyDestination)
        .and_then(|at| records.get(at))
        .is_some_and(|r| {
            r.kind == StepKind::VerifyDestination
                && matches!(r.disposition, Disposition::Verified { .. })
        })
}

/// What the ends of a dispatched mutating step's commands amount to (D4).
///
/// `results` are in command order, ending early only where a command did not
/// exit 0. Every command exited 0 → [`Disposition::Performed`]. Anything else
/// halts, carrying the ends of every command handed to the site — so a step
/// stopped on its second command still records the first (H3).
#[must_use]
pub fn judge_mutation(
    plan: &MigrationPlan,
    position: usize,
    results: &[MutationResult],
) -> Disposition {
    let mut ends = Vec::new();
    let halted = |reason, ends| Disposition::Halted {
        reason,
        evidence: Vec::new(),
        ends,
    };
    let Some(step) = plan.steps.get(position) else {
        return halted(HaltReason::RecordsDoNotMatchPlan, ends);
    };
    for (command, result) in results.iter().enumerate() {
        match *result {
            MutationResult::Ended(end) => {
                ends.push(end);
                if !end.exited_zero() {
                    return halted(HaltReason::MutationFailed { command }, ends);
                }
            }
            MutationResult::Refused => {
                return halted(HaltReason::MutationRefused { command }, ends)
            }
            MutationResult::TransportFailed(error) => {
                return halted(HaltReason::TransportFailed { command, error }, ends)
            }
            MutationResult::PreconditionFailed(why) => {
                return halted(HaltReason::PreconditionFailed { command, why }, ends)
            }
        }
    }
    if results.len() != step.commands.len() || ends.is_empty() {
        return halted(unevaluable(None, Unevaluable::MissingRead), ends);
    }
    Disposition::Performed { ends }
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
///
/// **Never certifies a `VERIFY_DESTINATION`**: for one it answers
/// [`Unevaluable::NotCertifiedHere`] before looking at the reads, so reads that
/// would pass do not change it. [`certify_destination`] is that step's only
/// path to `Verified` (SE-107 C1).
#[must_use]
pub fn judge_step(run: &PlannedRun, position: usize, reads: &[StepRead<'_>]) -> Disposition {
    match run.plan().steps.get(position).map(|s| s.kind) {
        Some(StepKind::VerifyDestination) => Disposition::Halted {
            reason: unevaluable(None, Unevaluable::NotCertifiedHere),
            evidence: Vec::new(),
            ends: Vec::new(),
        },
        _ => judge_reads(run, position, reads),
    }
}

/// Certifies the `VERIFY_DESTINATION` at `position` from its reads, which the
/// shell obtained **after** re-reading and re-checking the destination
/// provider's configuration: `checked` is that re-check's token, taken by value
/// so one re-check certifies one step (SE-107 C1; L1).
///
/// A token for any provider but the plan's destination is refused
/// (`PRECONDITION_FAILED` / `WRONG_PROVIDER_TOKEN`), as is a position that is
/// not a `VERIFY_DESTINATION` (`RECORDS_DO_NOT_MATCH_PLAN`).
#[must_use]
// By value on purpose: a re-check certifies one step and is spent doing so (L1);
// the token is neither `Clone` nor `Copy`, so it cannot be kept for another.
#[allow(clippy::needless_pass_by_value)]
pub fn certify_destination(
    run: &PlannedRun,
    position: usize,
    checked: ConfigurationRechecked,
    reads: &[StepRead<'_>],
) -> Disposition {
    let plan = run.plan();
    let halted = |reason| Disposition::Halted {
        reason,
        evidence: Vec::new(),
        ends: Vec::new(),
    };
    if plan.steps.get(position).map(|s| s.kind) != Some(StepKind::VerifyDestination) {
        return halted(HaltReason::RecordsDoNotMatchPlan);
    }
    if checked.provider() != &plan.destination {
        return halted(HaltReason::PreconditionFailed {
            command: 0,
            why: Precondition::WrongProviderToken,
        });
    }
    judge_reads(run, position, reads)
}

/// Whether the destination, read again immediately before a removal command,
/// still certifies (work order 16 part S, §6): `Ok` only when
/// [`certify_destination`] over `reads` at `reverify`'s position is exactly
/// `Verified`.
///
/// `checked` is the destination configuration re-check's token, by value, so a
/// token certifies one removal command and no other (L1). Refused, each as a
/// precondition the removal command fails: a token for any provider but the
/// plan's destination ([`Precondition::WrongProviderToken`]); a re-read command
/// whose elevation, re-derived from the compiled command table and never read
/// from the plan, is not `NONE`, or whose plan field is not `NONE` (L4); a
/// re-read step whose commands are not the operations the table derives; and
/// any disposition but `Verified` ([`Precondition::DestinationNotReverified`]).
///
/// # Errors
/// The precondition that did not hold.
pub fn destination_still_verified(
    run: &PlannedRun,
    reverify: &Reverify,
    checked: ConfigurationRechecked,
    reads: &[StepRead<'_>],
) -> Result<(), Precondition> {
    let plan = run.plan();
    if checked.provider() != &plan.destination || reverify.provider() != &plan.destination {
        return Err(Precondition::WrongProviderToken);
    }
    let Some(step) = plan.steps.get(reverify.verify_position()) else {
        return Err(Precondition::DestinationNotReverified);
    };
    let table = provider_commands::load().unwrap_or_default();
    let entry = table.get(plan.destination.as_str());
    let operations = provider_commands::verify_destination_operations(entry);
    let kind = package_kind_in(plan, run.before());
    if operations.len() != step.commands.len() {
        return Err(Precondition::DestinationNotReverified);
    }
    for (operation, command) in operations.iter().zip(&step.commands) {
        if elevation_for(entry, operation, kind, None) != Elevation::None
            || command.elevation != Elevation::None
        {
            return Err(Precondition::DestinationNotReverified);
        }
    }
    match certify_destination(run, reverify.verify_position(), checked, reads) {
        Disposition::Verified { .. } => Ok(()),
        Disposition::Halted {
            reason:
                HaltReason::PreconditionFailed {
                    why: Precondition::WrongProviderToken,
                    ..
                },
            ..
        } => Err(Precondition::WrongProviderToken),
        Disposition::NotPerformedInThisBuild
        | Disposition::SnapshotTaken
        | Disposition::RecordPendingReadBack
        | Disposition::NotDispatchedMutating
        | Disposition::Performed { .. }
        | Disposition::Halted { .. }
        | Disposition::NotReached => Err(Precondition::DestinationNotReverified),
    }
}

/// The judgement behind [`judge_step`] and [`certify_destination`].
fn judge_reads(run: &PlannedRun, position: usize, reads: &[StepRead<'_>]) -> Disposition {
    let (plan, before) = (run.plan(), run.before());
    let halted = |reason, evidence| Disposition::Halted {
        reason,
        evidence,
        ends: Vec::new(),
    };
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
        let verdict = evaluate_with(&expectation, &output, &linked_report);
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
        if *check == VerificationCheck::ProviderReportsActive
            && !step
                .verification
                .contains(&VerificationCheck::PackageNewlyPresent)
        {
            match destination_before(plan, before) {
                Before::Present => {}
                Before::Absent => {
                    return halted(
                        unevaluable(Some(check), Unevaluable::ActiveAloneAfterAnInstall),
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

/// Whether the plan this run validated still holds against a capture taken
/// **under the lock** (D6, D-L3): it holds only when planning that capture
/// with the same request, policy and provider ids gave exactly the same plan.
///
/// Re-planning, not a hand-picked subset, so every planner input is covered —
/// a dependent that appeared (INV-04), a service that started (INV-19),
/// ownership, mapping, configuration, capability. When the plans differ the
/// answer names the first fact that moved, by comparing the two captures;
/// that is diagnosis only, after the comparison has already refused.
///
/// # Errors
/// The first fact that moved, when the plans differ.
pub fn plan_still_holds(
    validated: &MigrationPlan,
    fresh: &MigrationPlan,
    before: &Snapshot,
    now: &Snapshot,
) -> Result<(), StateFact> {
    if validated == fresh {
        Ok(())
    } else {
        Err(first_moved_fact(validated, before, now))
    }
}

/// The first fact, in a fixed order, that differs between `before` and `now`
/// for the providers and the package `plan` is about.
fn first_moved_fact(plan: &MigrationPlan, before: &Snapshot, now: &Snapshot) -> StateFact {
    let sides = [&plan.source, &plan.destination];
    let pair =
        |id: &macportsbrew_domain::ProviderId| (before.providers.get(id), now.providers.get(id));
    for id in sides {
        match pair(id) {
            (Some(a), Some(b)) => {
                if a.detection.presence != b.detection.presence {
                    return StateFact::ProviderState;
                }
            }
            (None, None) => {}
            _ => return StateFact::ProviderState,
        }
    }
    for id in sides {
        if let (Some(a), Some(b)) = pair(id) {
            if a.detection.prefix != b.detection.prefix
                || a.detection.executable != b.detection.executable
            {
                return StateFact::ProviderLocation;
            }
        }
    }
    for id in sides {
        if let (Some(a), Some(b)) = pair(id) {
            if a.capabilities != b.capabilities {
                return StateFact::ProviderCapability;
            }
        }
    }
    let rows = |snap: &Snapshot, id: &macportsbrew_domain::ProviderId, name: &str| {
        snap.providers.get(id).map(|p| {
            p.inventory
                .installed
                .iter()
                .filter(|r| r.identity.name == name)
                .map(|r| (r.version.clone(), r.active, r.requested))
                .collect::<Vec<_>>()
        })
    };
    let name = plan.package.name.as_str();
    if rows(before, &plan.source, name) != rows(now, &plan.source, name) {
        return StateFact::SourcePackage;
    }
    if let Some(dest) = plan.destination_name() {
        let presence =
            |snap: &Snapshot| rows(snap, &plan.destination, dest.as_str()).map(|r| !r.is_empty());
        if presence(before) != presence(now) {
            return StateFact::DestinationPackage;
        }
    }
    let dependents = |snap: &Snapshot| {
        snap.providers.get(&plan.source).map(|p| {
            let mut from: Vec<String> = p
                .inventory
                .dependencies
                .iter()
                .filter(|e| e.to == name)
                .map(|e| e.from.clone())
                .collect();
            from.sort_unstable();
            from
        })
    };
    if dependents(before) != dependents(now) {
        return StateFact::ReverseDependency;
    }
    let metadata = |snap: &Snapshot| {
        snap.providers
            .get(&plan.source)
            .and_then(|p| p.inventory.metadata.get(name))
            .cloned()
    };
    if metadata(before) != metadata(now) {
        return StateFact::Classification;
    }
    StateFact::PlanDiffers
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
    let carried = |r: &&StepRecord| {
        matches!(
            r.disposition,
            Disposition::Verified { .. }
                | Disposition::Performed { .. }
                | Disposition::SnapshotTaken
                | Disposition::RecordPendingReadBack
        )
    };
    let Some(first_skipped) = records.iter().find(|r| !carried(r)) else {
        return RunOutcome::Completed;
    };
    let halt = records.iter().find_map(|r| match &r.disposition {
        Disposition::Halted { reason, .. } => Some((r.index, reason.clone())),
        _ => None,
    });
    let carried_out = records.iter().any(|r| carried(&r));
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
