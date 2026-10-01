// SPDX-License-Identifier: Apache-2.0

//! A run's history record, built from what the run loop returns (S8).
//!
//! [`run_record`] is pure: it takes the run, the per-step records the loop
//! handed back and a [`RunStart`] the caller supplies, and derives the outcome
//! itself with [`crate::executor::outcome_of`] — never one a caller hands
//! beside the records, which could disagree with them (spec-guardian, S8). The start time in it comes from the one
//! wall-clock read in the workspace — `persistence::time::now_secs`, which
//! `Context::build` already makes — never from a clock read here
//! (`invariant_14_the_wall_clock_is_read_in_exactly_one_place`).
//!
//! **Nothing calls it in the shipped binary.** This build dispatches no
//! mutating step, so no run it can execute is a mutating run, and obligation
//! 12 (`plan.v1.md`) writes a record only for those; the append-only store is
//! deferred to the mutating pass with the rest of the write path
//! (`04-migration-and-safety.md`, History).

use crate::executor::{
    outcome_of, Disposition, HaltReason, PlannedRun, RunOutcome, StepRecord, Unevaluable,
};
use macportsbrew_domain::{
    Direction, RecordedStep, RecordedVersions, RunEnding, RunRecord, RunStart, RunSubject,
    StepResult,
};

/// The history record of `run`, whose steps ended as `records`.
///
/// `records` are kept in the order given. A step's provider is taken from the
/// plan step with the same index and kind; a record matching none carries no
/// provider. No snapshot path is recorded: this build performs no
/// `SNAPSHOT_BEFORE`.
#[must_use]
pub fn run_record(run: &PlannedRun, records: &[StepRecord], start: RunStart) -> RunRecord {
    let plan = run.plan();
    let steps = records
        .iter()
        .map(|record| {
            let provider = plan
                .steps
                .iter()
                .find(|s| s.index == record.index && s.kind == record.kind)
                .and_then(|s| s.provider.clone());
            let (result, after_dispatch) = result_of(&record.disposition);
            RecordedStep::new(record.index, record.kind, provider, result, after_dispatch)
        })
        .collect();
    let source_version = run.before().providers.get(&plan.source).and_then(|p| {
        p.inventory
            .installed
            .iter()
            .find(|row| row.active && row.identity.name == plan.package.name)
            .map(|row| row.version.clone())
    });
    let subject = RunSubject {
        plan_schema_version: plan.schema_version,
        policy_version: plan.policy_version.clone(),
        package: plan.package.clone(),
        direction: Direction {
            source: plan.source.clone(),
            destination: plan.destination.clone(),
        },
        destination_package: plan.destination_package.clone(),
        versions: RecordedVersions {
            source: source_version,
            destination: None,
        },
        action: plan.recommendation.clone(),
        overrode: plan.overrode.clone(),
        retain_source: plan.retain_source,
        primary_after: plan.primary_after.clone(),
    };
    RunRecord::new(
        start,
        subject,
        steps,
        ending_of(&outcome_of(plan, records)),
        None,
    )
}

/// How the loop's outcome is recorded: the halt reason becomes its error
/// identifier, never its text.
fn ending_of(outcome: &RunOutcome) -> RunEnding {
    match outcome {
        RunOutcome::Completed => RunEnding::Completed,
        RunOutcome::Refused { refusals } => RunEnding::Refused {
            refusals: *refusals,
        },
        RunOutcome::Halted { step, reason } => RunEnding::Halted {
            step: *step,
            error: reason.error_id(),
        },
        RunOutcome::PartialSuccess { step, reason } => RunEnding::PartialSuccess {
            step: *step,
            error: reason.error_id(),
        },
    }
}

/// A step's result, and for a halt whether the step had been dispatched.
fn result_of(disposition: &Disposition) -> (StepResult, bool) {
    match disposition {
        Disposition::Verified { evidence } => (
            StepResult::Verified {
                evidence: evidence.clone(),
            },
            true,
        ),
        Disposition::NotPerformedInThisBuild => (StepResult::NotPerformedInThisBuild, false),
        Disposition::NotDispatchedMutating => (StepResult::NotDispatchedMutating, false),
        Disposition::Halted { reason, evidence } => (
            StepResult::Halted {
                error: reason.error_id(),
                evidence: evidence.clone(),
            },
            halted_after_dispatch(reason),
        ),
        Disposition::NotReached => (StepResult::NotReached, false),
    }
}

/// Whether a step that halted for `reason` had been handed to a runner.
///
/// Exhaustive, and **fails toward "dispatched"**: a reason both
/// `next_action` (before dispatch) and `judge_step` (after it) can give, and
/// records that do not match the plan, count as dispatched. Recording a change
/// that did not happen costs a lock and a marker; recording none when one
/// happened loses the run a restore needs.
fn halted_after_dispatch(reason: &HaltReason) -> bool {
    match reason {
        // Given only before a step is dispatched.
        HaltReason::ManualReview { .. }
        | HaltReason::UnexecutedMutation
        | HaltReason::NotPerformedInThisBuild
        | HaltReason::ElevationNotNone { .. } => false,
        // Given only by `judge_step`, after the dispatch.
        HaltReason::SiteUnavailable { .. }
        | HaltReason::SiteRefused { .. }
        | HaltReason::TransportFailed { .. }
        | HaltReason::Expectation { .. }
        | HaltReason::VerificationFailed { .. } => true,
        HaltReason::Unevaluable { why, .. } => match why {
            Unevaluable::NoEvaluationForStepKind | Unevaluable::NoCommand => false,
            Unevaluable::NoCheck
            | Unevaluable::CheckHasNoEvaluation
            | Unevaluable::NoPassingRead
            | Unevaluable::MissingRead
            | Unevaluable::PresenceBeforeUnknown
            | Unevaluable::PresentBeforeTheRun => true,
        },
        HaltReason::RecordsDoNotMatchPlan => true,
    }
}
