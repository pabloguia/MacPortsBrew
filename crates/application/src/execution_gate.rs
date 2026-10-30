// SPDX-License-Identifier: Apache-2.0

//! Whether a plan may be executed, and every reason it may not
//! (`plan.v1.md`, "What an executor inherits").
//!
//! **Consulted only for execution, never for rendering.** A plan that this
//! gate refuses is still a plan: `plan` shows it, `--json` serialises it, and
//! nothing here is on that path. What the gate answers is the narrower
//! question an executor must ask before it dispatches a single command, and it
//! answers it as a list of named refusals rather than a boolean, so that the
//! person reading the result is told *which* obligation the plan does not meet.
//!
//! Pure: a plan goes in, a [`GateReport`] comes out. No step is run, no
//! provider is asked anything, and nothing is read but the plan and the
//! provider command table compiled into this build.
//!
//! **In this build the gate refuses every plan the planner produces.** A plan
//! that stops at `MANUAL_REVIEW` is refused as such; every other one has a
//! mutating step while [`THIS_BUILD_DISPATCHES_MUTATIONS`] is `false`, a
//! `VERIFY_DESTINATION` carrying checks this build cannot
//! evaluate, and a `VERIFY_RESOLUTION` that asserts nothing. Those refusals
//! are true statements about the build, not placeholders for ones to come,
//! and the mutating pass removes each by meeting it — never by deleting the
//! refusal.
//!
//! **Exactly which fields of a command the gate checks.** It refuses a command
//! on a step kind the planner writes none on. On `VERIFY_DESTINATION` and
//! `VERIFY_REMOVAL` it checks the commands **by position and in number**:
//! command `i` carries the `argv` and `success` the planner derives for the
//! step's `i`-th operation from the provider command table, and the step
//! carries exactly as many commands as there are operations
//! (`provider_commands::verify_destination_reads`, the rule the planner builds
//! the step from; `verify_absent` alone for a removal); and, through
//! `verification::expectation_for`,
//! `target` (a provider), `placeholder` (false), the step's provider (the side
//! its kind reads) and the safety of the name it reads. So a forged `VERIFY_*`
//! step carrying a mutating verb, or a read whose semantic was swapped, is
//! refused here.
//!
//! **It does not check `program`, `environment` or `elevation`**, and trusts
//! none of them. The spawn site (the S4 slice) re-derives the program and the
//! environment from live detection and never reads them from the plan, and its
//! allowlist remains the last line: whatever a plan says, it decides whether a
//! command is one this build may run at all. Elevation is refused wholesale
//! below; when a consent path exists, whether to ask comes from the table and
//! the live capability probe, never from the plan's field (`plan.v1.md`,
//! obligation 9).
//!
//! **Some causes are reported twice**: a placeholder, an unrecognised check or
//! an unknown success semantic on a verification step also makes
//! `expectation_for` fail, so the report carries both the dedicated refusal
//! and an `EXPECTATION` refusal for the same cause. Both are true; neither is
//! filtered.

use crate::executor::PlannedRun;
use crate::planner::{elevation_for, package_kind_in, planned_argv, planned_success};
use crate::provider_commands;
use macportsbrew_domain::plan::{PLAN_SCHEMA_VERSION, THIS_BUILD_DISPATCHES_MUTATIONS};
use macportsbrew_domain::{
    expectation_for, Elevation, ErrorId, ExpectationError, MigrationPlan, PackageKind, ProviderId,
    StepKind, SuccessSemantic, VerificationCheck,
};
use serde::Serialize;

/// One reason a plan may not be executed by this build.
///
/// Step numbers are [`macportsbrew_domain::MigrationStep::index`] — the
/// number a person reads in the rendered plan — never a vector position. They
/// are the plan's own data, so a list of them is in plan order and is not
/// promised to be ascending or unique.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "refusal")]
pub enum ExecutionRefusal {
    /// The plan's `schemaVersion` is not [`PLAN_SCHEMA_VERSION`], the one this
    /// build's planner writes. Every other refusal reads the plan through this
    /// build's meaning of each field, and a plan of another schema version
    /// may mean something else by any of them; it is refused rather than read
    /// under a meaning its writer did not give it.
    SchemaVersionNotThisBuilds {
        /// The version the plan carries.
        found: u32,
        /// The version this build writes.
        expected: u32,
    },
    /// The plan fails `MigrationPlan::check_ordering` (INV-01). The planner
    /// never emits one; a plan from anywhere else can be one.
    Ordering,
    /// The steps' indices are not `1..=n` in order. Every refusal names a step
    /// by its index, and a person approves a plan by those numbers, so a plan
    /// whose numbers are repeated, skipped or out of order cannot be refused —
    /// or approved — by a name that means one step.
    StepIndicesNotSequential,
    /// The plan has no steps. There is nothing to execute, and a run of
    /// nothing would report success for a migration that never happened; the
    /// planner never emits one, so a plan with no steps is not this build's.
    EmptyPlan,
    /// A mutating step ([`StepKind::is_mutating`]) with no `SNAPSHOT_BEFORE`
    /// step before it (obligation 11). The snapshot is what a failed
    /// migration is restored from and what the run is compared against, so a
    /// change it does not precede is one nothing can undo or account for.
    /// Names the first mutating step, the one the snapshot would have to
    /// precede.
    ///
    /// **Never fires alone.** `MigrationPlan::check_ordering` already requires
    /// `SNAPSHOT_BEFORE` as the first step of any plan with a mutating step,
    /// so every plan refused here is refused as [`Self::Ordering`] too. What
    /// this refusal adds is the name of the unguarded step.
    NoSnapshotBeforeFirstMutation {
        /// The first mutating step.
        step: u32,
    },
    /// A plan with a mutating step whose last step is not `RECORD_HISTORY`
    /// (obligation 11). The final `RECORD_HISTORY` step is the *success
    /// path*: the record written when every step before it ran and verified.
    /// It is not the only place a record is written — the executor writes one
    /// on every way a mutating run ends, a failure, a partial run and a
    /// `MANUAL_REVIEW` halt among them (obligation 12). A mutating plan whose
    /// success path does not end with the record has a change after the last
    /// point a completed run is recorded, so history would disagree with the
    /// machine.
    NoRecordHistoryAtEnd,
    /// More than one `SNAPSHOT_BEFORE` step (obligation 11). The planner
    /// writes exactly one, and `check_ordering` looks only at the first, so a
    /// second one is a shape no rule was written for: which snapshot a failed
    /// run restores from would be the executor's guess.
    MoreThanOneSnapshotBefore {
        /// Every `SNAPSHOT_BEFORE` step, in plan order.
        steps: Vec<u32>,
    },
    /// More than one `RECORD_HISTORY` step (obligation 11). One run is one
    /// history record on its success path; a second would record a run that
    /// did not happen, or record the same one twice.
    MoreThanOneRecordHistory {
        /// Every `RECORD_HISTORY` step, in plan order.
        steps: Vec<u32>,
    },
    /// The plan stops at a `MANUAL_REVIEW` step (INV-19). Nothing outranks
    /// it, so it is refused whatever else the plan says.
    ManualReview {
        /// The step that stops the plan.
        step: u32,
    },
    /// Steps carrying an illustration rather than a command that can be run
    /// (obligation 1; `PlanCommand::placeholder`).
    Placeholder {
        /// The steps, in plan order.
        steps: Vec<u32>,
    },
    /// Steps carrying a verification check this build does not recognise,
    /// which can never count as satisfied (obligation 2).
    UnrecognisedCheck {
        /// The steps, in plan order.
        steps: Vec<u32>,
    },
    /// A check this build recognises and cannot evaluate on the step carrying
    /// it (obligation 6).
    ///
    /// Recognising a check is not being able to perform it, and evaluability
    /// is per step kind and check ([`evaluable_in_this_build`]): a check is
    /// answered by its step's own provider read, so the same check on a step
    /// that reads nothing — `PROVIDER_REPORTS_ACTIVE` on `SET_PRIMARY` — has
    /// nothing to answer it. A step cannot pass on the checks it can read
    /// while ignoring the ones it cannot, for the reason an unrecognised check
    /// cannot count as satisfied.
    CheckNotEvaluableInThisBuild {
        /// The step carrying it.
        step: u32,
        /// The check.
        check: VerificationCheck,
    },
    /// A verification step with no command to read, or no check to read it
    /// against (obligation 7). Such a step can only pass by asserting nothing,
    /// and a verification step that asserts nothing is a gate that is open.
    VerificationAssertsNothing {
        /// The step.
        step: u32,
    },
    /// Steps carrying a command that does not say how its success is
    /// established (obligation 4).
    UnknownSuccess {
        /// The steps, in plan order.
        steps: Vec<u32>,
    },
    /// A command on a step kind the planner writes no command on:
    /// `SNAPSHOT_BEFORE`, `VERIFY_RESOLUTION`, `CLEANUP_REVIEW`,
    /// `RECORD_HISTORY` or `MANUAL_REVIEW`. Such a command is not one this
    /// build derived, whatever its argv says.
    CommandOnStepThePlannerWritesNoneOn {
        /// The step.
        step: u32,
    },
    /// A command of a `VERIFY_DESTINATION` or `VERIFY_REMOVAL` step whose argv
    /// or success semantic is not the one the planner derives from the provider
    /// command table for that side's provider **at that position**, or a step
    /// whose commands are more or fewer than the operations it reads: for a
    /// destination, `provider_commands::verify_destination_reads` (the
    /// `verify` verb with the destination name appended, then `verify_active`
    /// with none when the table holds it); for a removal, the `verify_absent`
    /// verb with no name; each with its operation's semantic. A missing, extra,
    /// swapped or reordered command is refused. A verification step is where a read is
    /// dispatched, so one carrying any other argv — a mutating verb among them
    /// — or a swapped semantic is a forgery of the one step kind this build
    /// would run. `EXIT_STATUS` on a MacPorts read is the case that matters:
    /// that verb exits 0 for an absent port.
    CommandIsNotThePlannersRead {
        /// The step.
        step: u32,
    },
    /// A command of a `VERIFY_DESTINATION` or `VERIFY_REMOVAL` step from
    /// which no expectation can be derived (`verification::expectation_for`).
    /// A missing destination name lands here as `NO_DESTINATION_NAME`.
    Expectation {
        /// The step.
        step: u32,
        /// Why no expectation could be derived.
        error: ExpectationError,
    },
    /// The plan's source and destination are one provider. There is nothing
    /// to move, and a destination check would read the provider the package
    /// is leaving (INV-08).
    SameProviderOnBothSides,
    /// Steps carrying a command known to need administrator rights.
    ///
    /// **A refusal about this build, like [`Self::MutationNotInThisBuild`],
    /// not about the step.** INV-06 does not forbid elevation: it requires it
    /// to be requested per operation, enumerated in the plan and shown to the
    /// user, and `04-migration-and-safety.md` ("Privilege") has an executor
    /// *ask* before any step whose elevation is not `NONE`. This build has no
    /// such consent path, so it refuses; the mutating pass owes one, or a
    /// migration toward a provider whose writes elevate could never run
    /// (INV-17).
    ElevationRequired {
        /// The steps, in plan order.
        steps: Vec<u32>,
    },
    /// Steps carrying a command whose elevation nobody has established.
    ///
    /// A build-state refusal for the same reason as
    /// [`Self::ElevationRequired`]: an elevation nobody has ruled out is one
    /// the executor must ask about, and this build has nowhere to ask.
    ElevationUnknown {
        /// The steps, in plan order.
        steps: Vec<u32>,
    },
    /// The first mutating step, whether or not it carries a command. This
    /// build dispatches no mutating step ([`THIS_BUILD_DISPATCHES_MUTATIONS`]),
    /// and a mutating kind with no command — `CLEANUP_REVIEW`, or an
    /// `ADD_DESTINATION` the destination already satisfies — is still a step
    /// the executor would be running as a change to the machine. Keyed on the
    /// kind alone, so a plan cannot pass by leaving its commands out.
    MutationNotInThisBuild {
        /// The step.
        step: u32,
    },
    /// The plan's `dryRunOnly` disagrees with this build: it is not
    /// `!THIS_BUILD_DISPATCHES_MUTATIONS`, the value the planner writes. In
    /// this build that is a plan claiming `dryRunOnly: false` — built by
    /// something that can run mutating steps, so not a plan of this build's.
    PlanNotDryRunOnly,
    /// A mutating step that would change a provider whose own configuration
    /// overrides how this tool runs it — the plan carries a
    /// `provider_configuration` risk naming it (DP-027 (b)).
    ///
    /// **Not a refusal about this build.** Unlike
    /// [`Self::MutationNotInThisBuild`], it holds when mutation lands: nothing
    /// an invocation sets can outrank a provider's own configuration file
    /// (R-15, R-16), which is why DP-027 (b) refuses here rather than trusting
    /// the environment. The risk is raised for a file that sets a key, one that
    /// could not be read, and a configuration this input could not record - an
    /// unknown is never a yes. A step with no provider named is refused whenever
    /// any provider carries the risk, for the same reason.
    ProviderConfiguration {
        /// The step.
        step: u32,
        /// The provider the risk names.
        provider: ProviderId,
    },
    /// Under a mutation allowance only: an `ADD_DESTINATION` or
    /// `REMOVE_SOURCE` command whose argv, success semantic or elevation is not
    /// what the planner derives from the provider command table for that
    /// side, or that names another provider (pass 08 slice 4, D2). The
    /// mutating counterpart of [`Self::CommandIsNotThePlannersRead`]: what is
    /// dispatched is what the table says, never what a plan claims.
    MutationIsNotThePlannersCommand {
        /// The step.
        step: u32,
    },
    /// Whenever mutations would be dispatched: an `ADD_DESTINATION` carrying a
    /// command with no `VERIFY_DESTINATION` after it, or a `REMOVE_SOURCE`
    /// with no `VERIFY_REMOVAL` after it (slice 4 review, INV-03). A
    /// mutation's success is never its exit status, so a commanded change with
    /// nothing after it to judge it could otherwise end the run a success.
    MutationNotVerifiedAfter {
        /// The step.
        step: u32,
    },
    /// A check that is evaluable on its verification step in this build
    /// ([`evaluable_in_this_build`]) but that no command of that step carries
    /// a semantic to answer (work order 16 part S, M2b). Evaluable means
    /// evaluable **by this plan**: a step whose reads cannot answer one of its
    /// checks is refused here rather than halted at run time.
    CheckHasNoAnsweringRead {
        /// The step carrying it.
        step: u32,
        /// The check.
        check: VerificationCheck,
    },
}

impl ExecutionRefusal {
    /// The ordering key: the step the refusal names (0 for a refusal about the
    /// plan as a whole, the smallest for one naming several), then the variant
    /// in declaration order.
    fn key(&self) -> (u32, u8) {
        let first = |steps: &[u32]| steps.iter().copied().min().unwrap_or(0);
        match self {
            Self::SchemaVersionNotThisBuilds { .. } => (0, 0),
            Self::Ordering => (0, 1),
            Self::StepIndicesNotSequential => (0, 2),
            Self::EmptyPlan => (0, 3),
            Self::NoSnapshotBeforeFirstMutation { step } => (*step, 4),
            Self::NoRecordHistoryAtEnd => (0, 5),
            Self::MoreThanOneSnapshotBefore { steps } => (first(steps), 6),
            Self::MoreThanOneRecordHistory { steps } => (first(steps), 7),
            Self::ManualReview { step } => (*step, 8),
            Self::Placeholder { steps } => (first(steps), 9),
            Self::UnrecognisedCheck { steps } => (first(steps), 10),
            Self::CheckNotEvaluableInThisBuild { step, .. } => (*step, 11),
            Self::VerificationAssertsNothing { step } => (*step, 12),
            Self::UnknownSuccess { steps } => (first(steps), 13),
            Self::CommandOnStepThePlannerWritesNoneOn { step } => (*step, 14),
            Self::CommandIsNotThePlannersRead { step } => (*step, 15),
            Self::Expectation { step, .. } => (*step, 16),
            Self::SameProviderOnBothSides => (0, 17),
            Self::ElevationRequired { steps } => (first(steps), 18),
            Self::ElevationUnknown { steps } => (first(steps), 19),
            Self::MutationNotInThisBuild { step } => (*step, 20),
            Self::PlanNotDryRunOnly => (0, 21),
            Self::ProviderConfiguration { step, .. } => (*step, 22),
            Self::MutationIsNotThePlannersCommand { step } => (*step, 23),
            Self::MutationNotVerifiedAfter { step } => (*step, 24),
            Self::CheckHasNoAnsweringRead { step, .. } => (*step, 25),
        }
    }
}

/// What the gate concluded about one plan.
///
/// No public constructor: [`execution_refusals`] is the only way to obtain
/// one, so a report cannot say a plan may run without the gate having looked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateReport {
    refusals: Vec<ExecutionRefusal>,
}

impl GateReport {
    /// Every refusal, ordered by the step it names and then by kind, so the
    /// same plan gives the same list on every run (INV-15).
    #[must_use]
    pub fn refusals(&self) -> &[ExecutionRefusal] {
        &self.refusals
    }

    /// Whether the plan may be executed: only when nothing was refused.
    #[must_use]
    pub fn may_execute(&self) -> bool {
        self.refusals.is_empty()
    }

    /// The exit code a command reports when it consulted the gate: `0` when
    /// the plan may run, and the safety-blocked code (4) whenever anything was
    /// refused.
    ///
    /// **Always 4, whichever refusal it was**, the pattern `plan` already
    /// follows for a `MANUAL_REVIEW` stop (SE-009). A lesser code for the
    /// refusal that only says "not in this build" was considered and rejected:
    /// a script would learn that code as "fine, just not yet", and the day a
    /// safety refusal joined it on the same plan the script would read the
    /// lesser one.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        if self.may_execute() {
            0
        } else {
            ErrorId::SafetyBlocked.exit_code()
        }
    }
}

/// The proof that this build may dispatch mutating steps (pass 08 slice 4, D1).
///
/// **Not a feature of the binary.** The field is private and this file is the
/// only place one is made: [`mutation_allowance`] makes one only when
/// [`THIS_BUILD_DISPATCHES_MUTATIONS`] is `true` (it is not), and
/// [`MutationAllowance::forced_in_tests`] exists only in test builds and under
/// the non-default `test-support` feature, which only the command-line crate's
/// tests enable. A word scan keeps that constructor, any other construction
/// and any trait implementation for this type out of product source
/// (`crates/cli/tests/executor.rs`). It has no `Clone`, `Copy`, `Default`,
/// `From` or serde implementation, by design.
#[must_use]
pub struct MutationAllowance(());

impl MutationAllowance {
    /// An allowance for a test that exercises the mutating path against fakes.
    ///
    /// **Test builds only.** A declared deviation from the letter of the work
    /// order, which asks for `#[cfg(test)]` or a file under `crates/*/tests/`:
    /// the command-line crate's tests cannot reach a private field across the
    /// crate boundary otherwise (SE-092, slice 4 D1).
    #[cfg(any(test, feature = "test-support"))]
    pub fn forced_in_tests() -> Self {
        Self(())
    }
}

impl std::fmt::Debug for MutationAllowance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MutationAllowance")
    }
}

/// Compile-time proof that a type is **not** `Clone` and **not** `Default`
/// (slice 4 review, M2): with either implemented, the blanket impls below make
/// the item ambiguous and this file stops compiling. No dependency.
mod not_clone_nor_default {
    pub(super) trait AmbiguousIfClone<A> {
        fn item() {}
    }
    impl<T: ?Sized> AmbiguousIfClone<()> for T {}
    impl<T: Clone> AmbiguousIfClone<u8> for T {}
    pub(super) trait AmbiguousIfDefault<A> {
        fn item() {}
    }
    impl<T: ?Sized> AmbiguousIfDefault<()> for T {}
    impl<T: Default> AmbiguousIfDefault<u8> for T {}
}
const _: fn() = || {
    use not_clone_nor_default::{AmbiguousIfClone, AmbiguousIfDefault};
    <MutationAllowance as AmbiguousIfClone<_>>::item();
    <MutationAllowance as AmbiguousIfDefault<_>>::item();
    <GateClear as AmbiguousIfClone<_>>::item();
    <GateClear as AmbiguousIfDefault<_>>::item();
};

/// The allowance this build grants: one only when it dispatches mutations.
/// The single product producer; `migrate` asks here rather than choosing.
#[must_use]
pub fn mutation_allowance() -> Option<MutationAllowance> {
    THIS_BUILD_DISPATCHES_MUTATIONS.then_some(MutationAllowance(()))
}

/// The witness that the gate refused nothing for a run, and under which
/// allowance (D2). It **owns** the run and the allowance it was cleared under
/// and hands out only borrows, so a witness cannot accompany another plan, and
/// one cleared without an allowance cannot be paired with one. Only
/// [`clear`] makes one.
pub struct GateClear {
    run: PlannedRun,
    allowance: Option<MutationAllowance>,
}

impl GateClear {
    /// The run the gate cleared.
    #[must_use]
    pub fn run(&self) -> &PlannedRun {
        &self.run
    }

    /// The allowance it was cleared under.
    #[must_use]
    pub fn allowance(&self) -> Option<&MutationAllowance> {
        self.allowance.as_ref()
    }
}

impl std::fmt::Debug for GateClear {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GateClear")
            .field("allowance", &self.allowance.is_some())
            .finish_non_exhaustive()
    }
}

/// The gate's verdict as a witness: [`GateClear`] when it refused nothing,
/// and otherwise the run back with the report.
///
/// # Errors
/// The run and the gate's report, when anything was refused.
#[allow(clippy::result_large_err)]
pub fn clear(
    run: PlannedRun,
    allowance: Option<MutationAllowance>,
) -> Result<GateClear, (PlannedRun, GateReport)> {
    let report = execution_refusals_for(&run, allowance.as_ref());
    if report.may_execute() {
        Ok(GateClear { run, allowance })
    } else {
        Err((run, report))
    }
}

/// Whether this build has an evaluation for `check` on a step of `kind`.
///
/// A check is answered by its step's own provider read through
/// `verification::evaluate`, so evaluability is per (step kind, check), not
/// per check. `true` for three pairs only: `PACKAGE_NEWLY_PRESENT` and
/// `PROVIDER_REPORTS_ACTIVE` on `VERIFY_DESTINATION`, and `PACKAGE_ABSENT` on
/// `VERIFY_REMOVAL`. The same check on any other step has no read to answer
/// it.
///
/// **`PROVIDER_REPORTS_ACTIVE` is answered by an active read only** (work
/// order 16 part S; SE-107). It used to be answered by the destination's
/// presence read, which establishes *installed*, not *active* — certifying one
/// as the other is a false verification guarding INV-01 (pass 08 slice 4, D11;
/// SE-094). Both providers now carry a `verify_active` cell, so the check is
/// evaluable in both directions under one rule (INV-17); a step whose reads
/// include no active read is still refused, by
/// [`ExecutionRefusal::CheckHasNoAnsweringRead`].
///
/// The other checks need evidence this build does not gather — an integrity
/// verb, an ownership query, a linkage scan — and `LIVENESS_PROBE` would run
/// the package's own executable, a program outside the executor's
/// `PERMITTED_PROGRAMS`, which is a security decision and not a detail
/// (obligation 6). `UNKNOWN` is never evaluable.
///
/// Exhaustive on purpose: a check added to the domain must be classified
/// here before this compiles, rather than falling into a wildcard.
#[must_use]
pub fn evaluable_in_this_build(kind: StepKind, check: &VerificationCheck) -> bool {
    let reads = match check {
        VerificationCheck::PackageNewlyPresent | VerificationCheck::ProviderReportsActive => {
            StepKind::VerifyDestination
        }
        VerificationCheck::PackageAbsent => StepKind::VerifyRemoval,
        VerificationCheck::ProviderIntegrityCheck
        | VerificationCheck::ExecutablesOwnedByDestination { .. }
        | VerificationCheck::LivenessProbe { .. }
        | VerificationCheck::NoCrossProviderLinkage
        | VerificationCheck::Unknown => return false,
    };
    kind == reads
}

/// Whether a passed read of `semantic` answers `check`. Presence is answered
/// by a destination read, absence by an inventory read, `PROVIDER_REPORTS_ACTIVE`
/// by an active read alone; nothing else is answered by any read. An
/// "installed" read never answers `PROVIDER_REPORTS_ACTIVE` (D11): installed is
/// not active.
///
/// Shared by the gate ([`ExecutionRefusal::CheckHasNoAnsweringRead`]) and the
/// executor, so "evaluable by this plan" and "answered at run time" are one
/// rule.
pub(crate) fn answers(check: &VerificationCheck, semantic: SuccessSemantic) -> bool {
    match check {
        VerificationCheck::PackageNewlyPresent => matches!(
            semantic,
            SuccessSemantic::ExitStatus | SuccessSemantic::StdoutNamesPackage
        ),
        VerificationCheck::PackageAbsent => semantic == SuccessSemantic::StdoutOmitsPackage,
        VerificationCheck::ProviderReportsActive => matches!(
            semantic,
            SuccessSemantic::StdoutMarksPackageActive | SuccessSemantic::JsonReportsPackageLinked
        ),
        VerificationCheck::ProviderIntegrityCheck
        | VerificationCheck::ExecutablesOwnedByDestination { .. }
        | VerificationCheck::LivenessProbe { .. }
        | VerificationCheck::NoCrossProviderLinkage
        | VerificationCheck::Unknown => false,
    }
}

/// Every reason `plan` may not be executed by this build.
///
/// Never refuses rendering: a caller showing a plan does not consult this.
#[must_use]
pub fn execution_refusals(plan: &MigrationPlan) -> GateReport {
    refusals(plan, None, PackageKind::GuiApp)
}

/// [`execution_refusals`] for a run under `allowance` (D2): the pins on
/// mutating steps re-derive elevation for the package's kind as the run's own
/// snapshot records it, never from the plan.
#[must_use]
pub fn execution_refusals_for(
    run: &PlannedRun,
    allowance: Option<&MutationAllowance>,
) -> GateReport {
    refusals(
        run.plan(),
        allowance,
        package_kind_in(run.plan(), run.before()),
    )
}

/// The gate. An allowance changes the two build-state refusals; whenever
/// mutations would be dispatched — under an allowance, or in a build whose
/// constant is `true` — it **adds** pins on mutating steps; it removes no
/// other refusal. `kind` is the package's, for the elevation pin.
fn refusals(
    plan: &MigrationPlan,
    allowance: Option<&MutationAllowance>,
    kind: PackageKind,
) -> GateReport {
    let dispatches = THIS_BUILD_DISPATCHES_MUTATIONS || allowance.is_some();
    let mut refusals = Vec::new();

    if plan.schema_version != PLAN_SCHEMA_VERSION {
        refusals.push(ExecutionRefusal::SchemaVersionNotThisBuilds {
            found: plan.schema_version,
            expected: PLAN_SCHEMA_VERSION,
        });
    }
    if plan.check_ordering().is_err() {
        refusals.push(ExecutionRefusal::Ordering);
    }
    if !plan
        .steps
        .iter()
        .zip(1u32..)
        .all(|(step, expected)| step.index == expected)
    {
        refusals.push(ExecutionRefusal::StepIndicesNotSequential);
    }
    if plan.steps.is_empty() {
        refusals.push(ExecutionRefusal::EmptyPlan);
    }
    // Bracketing (obligation 11), by position rather than by index: the
    // executor runs steps in the order the plan lists them, and an index is
    // checked above. A plan with no mutating step needs neither bracket.
    if let Some(first) = plan.steps.iter().position(|s| s.kind.is_mutating()) {
        if !plan.steps[..first]
            .iter()
            .any(|s| s.kind == StepKind::SnapshotBefore)
        {
            refusals.push(ExecutionRefusal::NoSnapshotBeforeFirstMutation {
                step: plan.steps[first].index,
            });
        }
        if plan.steps.last().map(|s| s.kind) != Some(StepKind::RecordHistory) {
            refusals.push(ExecutionRefusal::NoRecordHistoryAtEnd);
        }
    }
    // One of each bracket, on every plan: `check_ordering` reads only the
    // first `SNAPSHOT_BEFORE`, and nothing else counts either kind.
    let of_kind = |kind: StepKind| -> Vec<u32> {
        plan.steps
            .iter()
            .filter(|s| s.kind == kind)
            .map(|s| s.index)
            .collect()
    };
    let snapshots = of_kind(StepKind::SnapshotBefore);
    if snapshots.len() > 1 {
        refusals.push(ExecutionRefusal::MoreThanOneSnapshotBefore { steps: snapshots });
    }
    let records = of_kind(StepKind::RecordHistory);
    if records.len() > 1 {
        refusals.push(ExecutionRefusal::MoreThanOneRecordHistory { steps: records });
    }
    for step in &plan.steps {
        if step.kind == StepKind::ManualReview {
            refusals.push(ExecutionRefusal::ManualReview { step: step.index });
        }
    }
    let placeholders = plan.steps_with_placeholders();
    if !placeholders.is_empty() {
        refusals.push(ExecutionRefusal::Placeholder {
            steps: placeholders,
        });
    }
    let unrecognised: Vec<u32> = plan
        .steps
        .iter()
        .filter(|s| !s.verification.iter().all(VerificationCheck::is_satisfiable))
        .map(|s| s.index)
        .collect();
    if !unrecognised.is_empty() {
        refusals.push(ExecutionRefusal::UnrecognisedCheck {
            steps: unrecognised,
        });
    }
    for step in &plan.steps {
        for check in &step.verification {
            if check.is_satisfiable() && !evaluable_in_this_build(step.kind, check) {
                refusals.push(ExecutionRefusal::CheckNotEvaluableInThisBuild {
                    step: step.index,
                    check: check.clone(),
                });
            }
        }
    }
    for step in &plan.steps {
        if is_verification(step.kind) && (step.commands.is_empty() || step.verification.is_empty())
        {
            refusals.push(ExecutionRefusal::VerificationAssertsNothing { step: step.index });
        }
    }
    let unknown_success = plan.steps_with_unknown_success();
    if !unknown_success.is_empty() {
        refusals.push(ExecutionRefusal::UnknownSuccess {
            steps: unknown_success,
        });
    }
    for step in &plan.steps {
        if !planner_writes_commands_on(step.kind) && !step.commands.is_empty() {
            refusals
                .push(ExecutionRefusal::CommandOnStepThePlannerWritesNoneOn { step: step.index });
        }
    }
    // Loaded here rather than passed in, so no caller can hand the gate a
    // table other than the one the planner reads. It is compiled into the
    // build; a table that does not parse derives nothing, and every
    // verification command is then refused below.
    let table = provider_commands::load().unwrap_or_default();
    for step in &plan.steps {
        let expected: Option<Vec<(Vec<String>, SuccessSemantic)>> = match step.kind {
            StepKind::VerifyDestination => plan.destination_package.as_deref().map(|name| {
                provider_commands::verify_destination_reads(&table, plan.destination.as_str(), name)
                    .into_iter()
                    .map(|(_, argv, success)| (argv, success))
                    .collect()
            }),
            StepKind::VerifyRemoval => {
                let entry = table.get(plan.source.as_str());
                Some(vec![(
                    planned_argv(entry, "verify_absent", &plan.package.name),
                    planned_success(entry, "verify_absent"),
                )])
            }
            StepKind::SnapshotBefore
            | StepKind::AddDestination
            | StepKind::SetPrimary
            | StepKind::VerifyResolution
            | StepKind::RemoveSource
            | StepKind::CleanupReview
            | StepKind::RecordHistory
            | StepKind::ManualReview => continue,
        };
        // A provider the table does not hold derives an illustration, never a
        // read, so nothing matches it.
        let derived = expected.filter(|reads| {
            reads
                .iter()
                .all(|(argv, _)| !argv.iter().any(|a| a.starts_with('<')))
        });
        // By position and in number: a missing, extra, swapped or reordered
        // command is not the planner's (V2). A step with no command at all is
        // refused as asserting nothing, above, and is not refused twice.
        let disagrees = !step.commands.is_empty()
            && derived.as_ref().is_none_or(|reads| {
                reads.len() != step.commands.len()
                    || reads
                        .iter()
                        .zip(&step.commands)
                        .any(|((argv, success), c)| *argv != c.argv || *success != c.success)
            });
        if disagrees {
            refusals.push(ExecutionRefusal::CommandIsNotThePlannersRead { step: step.index });
        }
    }
    // "Evaluable" is "evaluable by this plan" (M2b): a check this build can
    // evaluate on the step, that none of the step's commands can answer, is
    // refused here rather than halted at run time.
    for step in &plan.steps {
        if step.commands.is_empty()
            || !matches!(
                step.kind,
                StepKind::VerifyDestination | StepKind::VerifyRemoval
            )
        {
            continue;
        }
        for check in &step.verification {
            if evaluable_in_this_build(step.kind, check)
                && !step.commands.iter().any(|c| answers(check, c.success))
            {
                refusals.push(ExecutionRefusal::CheckHasNoAnsweringRead {
                    step: step.index,
                    check: check.clone(),
                });
            }
        }
    }
    for (position, step) in plan.steps.iter().enumerate() {
        if !matches!(
            step.kind,
            StepKind::VerifyDestination | StepKind::VerifyRemoval
        ) {
            continue;
        }
        let mut seen: Vec<ExpectationError> = Vec::new();
        for command in 0..step.commands.len() {
            if let Err(error) = expectation_for(plan, position, command) {
                if !seen.contains(&error) {
                    seen.push(error);
                    refusals.push(ExecutionRefusal::Expectation {
                        step: step.index,
                        error,
                    });
                }
            }
        }
    }
    if plan.source == plan.destination {
        refusals.push(ExecutionRefusal::SameProviderOnBothSides);
    }
    let required = plan.steps_requiring_elevation();
    if !required.is_empty() {
        refusals.push(ExecutionRefusal::ElevationRequired { steps: required });
    }
    let unknown = plan.steps_with_unknown_elevation();
    if !unknown.is_empty() {
        refusals.push(ExecutionRefusal::ElevationUnknown { steps: unknown });
    }
    if !dispatches {
        if let Some(step) = plan.steps.iter().find(|s| s.kind.is_mutating()) {
            refusals.push(ExecutionRefusal::MutationNotInThisBuild { step: step.index });
        }
    }
    // Under an allowance, a mutating step that would be dispatched must carry
    // exactly what the planner derives from the table — argv, success and
    // elevation (D2). For a mutating operation the table alone decides
    // elevation, so this needs no snapshot.
    if dispatches {
        for (position, step) in plan.steps.iter().enumerate() {
            let judged_by = match step.kind {
                StepKind::AddDestination if !step.commands.is_empty() => {
                    Some(StepKind::VerifyDestination)
                }
                StepKind::RemoveSource => Some(StepKind::VerifyRemoval),
                _ => None,
            };
            if let Some(kind) = judged_by {
                if !plan.steps[position + 1..].iter().any(|s| s.kind == kind) {
                    refusals.push(ExecutionRefusal::MutationNotVerifiedAfter { step: step.index });
                }
            }
        }
        for step in &plan.steps {
            let (side, operation, name) = match step.kind {
                StepKind::AddDestination => (
                    &plan.destination,
                    "add",
                    plan.destination_package.as_deref(),
                ),
                StepKind::RemoveSource => {
                    (&plan.source, "remove", Some(plan.package.name.as_str()))
                }
                _ => continue,
            };
            let entry = table.get(side.as_str());
            let derived = name.map(|name| {
                (
                    planned_argv(entry, operation, name),
                    planned_success(entry, operation),
                    elevation_for(entry, operation, kind, None),
                )
            });
            let disagrees = step.commands.iter().any(|c| {
                derived.as_ref().is_none_or(|(argv, success, elevation)| {
                    *argv != c.argv
                        || argv.iter().any(|a| a.starts_with('<'))
                        || *success != c.success
                        || *elevation != c.elevation
                        || step.provider.as_ref() != Some(side)
                })
            });
            if disagrees {
                refusals
                    .push(ExecutionRefusal::MutationIsNotThePlannersCommand { step: step.index });
            }
            if derived
                .as_ref()
                .is_some_and(|(_, _, elevation)| *elevation != Elevation::None)
                && !step.commands.is_empty()
            {
                refusals.push(ExecutionRefusal::ElevationRequired {
                    steps: vec![step.index],
                });
            }
        }
    }
    // DP-027 (b). Read from the plan's risks, so the gate still reads nothing
    // but the plan: the planner reports the configuration it saw, and a step
    // that would change that provider is refused whatever this build can do.
    let configured: std::collections::BTreeSet<ProviderId> = plan
        .risks
        .iter()
        .filter(|r| r.id == crate::planner::PROVIDER_CONFIGURATION_RISK)
        .filter_map(|r| r.reason.params.get("provider"))
        .map(ProviderId::new)
        .collect();
    for step in plan.steps.iter().filter(|s| s.kind.is_mutating()) {
        let affected: Vec<&ProviderId> = match &step.provider {
            Some(provider) => configured.iter().filter(|c| *c == provider).collect(),
            None => configured.iter().collect(),
        };
        for provider in affected {
            refusals.push(ExecutionRefusal::ProviderConfiguration {
                step: step.index,
                provider: provider.clone(),
            });
        }
    }
    // What the planner writes; a plan saying anything else is not this build's.
    let dry_run_only = !dispatches;
    if plan.dry_run_only != dry_run_only {
        refusals.push(ExecutionRefusal::PlanNotDryRunOnly);
    }

    // Stable, so two refusals with one key keep the plan's own order.
    refusals.sort_by_key(ExecutionRefusal::key);
    GateReport { refusals }
}

/// Whether the planner writes commands on a step of this kind. It writes them
/// on these five and no other, so a command anywhere else is not one it
/// derived.
///
/// Exhaustive, so a new kind is classified rather than defaulted.
fn planner_writes_commands_on(kind: StepKind) -> bool {
    match kind {
        StepKind::AddDestination
        | StepKind::VerifyDestination
        | StepKind::SetPrimary
        | StepKind::RemoveSource
        | StepKind::VerifyRemoval => true,
        StepKind::SnapshotBefore
        | StepKind::VerifyResolution
        | StepKind::CleanupReview
        | StepKind::RecordHistory
        | StepKind::ManualReview => false,
    }
}

/// Whether a step kind is one whose purpose is to establish something.
///
/// Exhaustive, like [`evaluable_in_this_build`], so a new kind is classified
/// rather than defaulted.
fn is_verification(kind: StepKind) -> bool {
    match kind {
        StepKind::VerifyDestination | StepKind::VerifyResolution | StepKind::VerifyRemoval => true,
        StepKind::SnapshotBefore
        | StepKind::AddDestination
        | StepKind::SetPrimary
        | StepKind::RemoveSource
        | StepKind::CleanupReview
        | StepKind::RecordHistory
        | StepKind::ManualReview => false,
    }
}
