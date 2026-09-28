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
//! mutating step carrying a command while [`THIS_BUILD_DISPATCHES_MUTATIONS`]
//! is `false`, a `VERIFY_DESTINATION` carrying checks this build cannot
//! evaluate, and a `VERIFY_RESOLUTION` that asserts nothing. Those refusals
//! are true statements about the build, not placeholders for ones to come,
//! and the mutating pass removes each by meeting it — never by deleting the
//! refusal.
//!
//! **Exactly which fields of a command the gate checks.** It refuses a command
//! on a step kind the planner writes none on. On `VERIFY_DESTINATION` and
//! `VERIFY_REMOVAL` it checks, for every command: `argv` and `success`, both
//! equal to what the planner derives from the provider command table for that
//! side's provider and operation (`planned_argv`, `planned_success` — the
//! planner's own functions); and, through `verification::expectation_for`,
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

use crate::planner::{planned_argv, planned_success};
use crate::provider_commands;
use macportsbrew_domain::plan::THIS_BUILD_DISPATCHES_MUTATIONS;
use macportsbrew_domain::{
    expectation_for, ErrorId, ExpectationError, MigrationPlan, StepKind, VerificationCheck,
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
    /// The plan fails `MigrationPlan::check_ordering` (INV-01). The planner
    /// never emits one; a plan from anywhere else can be one.
    Ordering,
    /// The steps' indices are not `1..=n` in order. Every refusal names a step
    /// by its index, and a person approves a plan by those numbers, so a plan
    /// whose numbers are repeated, skipped or out of order cannot be refused —
    /// or approved — by a name that means one step.
    StepIndicesNotSequential,
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
    /// command table for that side's provider: the `verify` verb with the
    /// destination name appended, or the `verify_absent` verb with no name, and
    /// that operation's semantic. A verification step is where a read is
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
    /// The first mutating step that carries a command. This build dispatches
    /// no mutating step ([`THIS_BUILD_DISPATCHES_MUTATIONS`]).
    MutationNotInThisBuild {
        /// The step.
        step: u32,
    },
    /// The plan's `dryRunOnly` disagrees with this build: it is not
    /// `!THIS_BUILD_DISPATCHES_MUTATIONS`, the value the planner writes. In
    /// this build that is a plan claiming `dryRunOnly: false` — built by
    /// something that can run mutating steps, so not a plan of this build's.
    PlanNotDryRunOnly,
}

impl ExecutionRefusal {
    /// The ordering key: the step the refusal names (0 for a refusal about the
    /// plan as a whole, the smallest for one naming several), then the variant
    /// in declaration order.
    fn key(&self) -> (u32, u8) {
        let first = |steps: &[u32]| steps.iter().copied().min().unwrap_or(0);
        match self {
            Self::Ordering => (0, 0),
            Self::StepIndicesNotSequential => (0, 1),
            Self::ManualReview { step } => (*step, 2),
            Self::Placeholder { steps } => (first(steps), 3),
            Self::UnrecognisedCheck { steps } => (first(steps), 4),
            Self::CheckNotEvaluableInThisBuild { step, .. } => (*step, 5),
            Self::VerificationAssertsNothing { step } => (*step, 6),
            Self::UnknownSuccess { steps } => (first(steps), 7),
            Self::CommandOnStepThePlannerWritesNoneOn { step } => (*step, 8),
            Self::CommandIsNotThePlannersRead { step } => (*step, 9),
            Self::Expectation { step, .. } => (*step, 10),
            Self::SameProviderOnBothSides => (0, 11),
            Self::ElevationRequired { steps } => (first(steps), 12),
            Self::ElevationUnknown { steps } => (first(steps), 13),
            Self::MutationNotInThisBuild { step } => (*step, 14),
            Self::PlanNotDryRunOnly => (0, 15),
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

/// Whether this build has an evaluation for `check` on a step of `kind`.
///
/// A check is answered by its step's own provider read through
/// `verification::evaluate`, so evaluability is per (step kind, check), not
/// per check. `true` for three pairs only: `PROVIDER_REPORTS_ACTIVE` and
/// `PACKAGE_NEWLY_PRESENT` on `VERIFY_DESTINATION`, and `PACKAGE_ABSENT` on
/// `VERIFY_REMOVAL`. The same check on any other step — `SET_PRIMARY` carries
/// `PROVIDER_REPORTS_ACTIVE` and reads nothing — has no read to answer it.
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
        VerificationCheck::ProviderReportsActive | VerificationCheck::PackageNewlyPresent => {
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

/// Every reason `plan` may not be executed by this build.
///
/// Never refuses rendering: a caller showing a plan does not consult this.
#[must_use]
pub fn execution_refusals(plan: &MigrationPlan) -> GateReport {
    let mut refusals = Vec::new();

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
        let expected = match step.kind {
            StepKind::VerifyDestination => {
                let entry = table.get(plan.destination.as_str());
                plan.destination_package.as_deref().map(|name| {
                    (
                        planned_argv(entry, "verify", name),
                        planned_success(entry, "verify"),
                    )
                })
            }
            StepKind::VerifyRemoval => {
                let entry = table.get(plan.source.as_str());
                Some((
                    planned_argv(entry, "verify_absent", &plan.package.name),
                    planned_success(entry, "verify_absent"),
                ))
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
        let derived = expected.filter(|(argv, _)| !argv.iter().any(|a| a.starts_with('<')));
        if step.commands.iter().any(|c| {
            derived
                .as_ref()
                .is_none_or(|(argv, success)| *argv != c.argv || *success != c.success)
        }) {
            refusals.push(ExecutionRefusal::CommandIsNotThePlannersRead { step: step.index });
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
    if !THIS_BUILD_DISPATCHES_MUTATIONS {
        if let Some(step) = plan
            .steps
            .iter()
            .find(|s| s.kind.is_mutating() && !s.commands.is_empty())
        {
            refusals.push(ExecutionRefusal::MutationNotInThisBuild { step: step.index });
        }
    }
    // What the planner writes; a plan saying anything else is not this build's.
    let dry_run_only = !THIS_BUILD_DISPATCHES_MUTATIONS;
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
