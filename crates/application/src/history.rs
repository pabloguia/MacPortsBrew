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
//! 12 (`plan.v1.md`) writes a record only for those ([`must_record`]). The
//! append-only store exists (`persistence::history_store`, pass 08 slice 1);
//! the call that writes through it lands with the mutations.
//!
//! [`recognise_retained_run`] is the other pure rule here: whether a
//! `migrate --retain-source` re-run is one history already shows done
//! (INV-16). It never trusts a record alone — a fresh capture must show the
//! end state — and it fails closed on everything it cannot order or confirm,
//! saying why ([`NotRecognised`]).

use crate::executor::{
    outcome_of, Disposition, HaltReason, PlannedRun, RunOutcome, StepRecord, Unevaluable,
};
use macportsbrew_domain::{
    Direction, ExecutableOwner, HistoryView, MigrationKey, MigrationPlan, ProviderId, RecordedStep,
    RecordedVersions, RunEnding, RunRecord, RunStart, RunStatus, RunSubject, Snapshot, StepResult,
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

/// Whether `record` is written to history: iff the run dispatched a mutating
/// step (`plan.v1.md` obligation 12; D2 of pass 08's plan). A gate-refused,
/// `NO_CHANGE` or nothing-to-move run leaves nothing.
#[must_use]
pub fn must_record(record: &RunRecord) -> bool {
    record.mutation_dispatched()
}

/// What `migrate` knows of history for this run.
#[derive(Clone, Copy, Debug)]
pub enum HistoryConsultation<'a> {
    /// History was not consulted: a replay, a snapshot file, a run without
    /// `--retain-source`, or this build's wiring (INV-15).
    NotConsulted,
    /// History was read for this migration.
    Consulted {
        /// What the store returned for the migration's key.
        view: &'a HistoryView,
        /// This run's own compact stamp, from the one clock read.
        run_stamp: &'a str,
    },
}

/// The migration a plan is, as history keys it.
#[must_use]
pub fn migration_key(plan: &MigrationPlan) -> MigrationKey {
    MigrationKey {
        package: plan.package.clone(),
        destination: plan.destination.clone(),
        destination_package: plan.destination_package.clone(),
    }
}

/// Why a re-run was not recognised as done, when history had something to
/// say about it. Each is a fail-closed answer: the run is planned again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotRecognised {
    /// The key's latest record is not a success.
    LatestRunNotSuccess {
        /// That run.
        run_id: String,
    },
    /// The key's latest record did not retain the source.
    LatestRunRemovedSource {
        /// That run.
        run_id: String,
    },
    /// More than one record for the key shares the newest stamp, so which is
    /// latest cannot be told.
    Tie,
    /// A record is stamped later than this run: the clock moved, and the
    /// order of records cannot be trusted.
    RecordFromTheFuture,
    /// The fresh capture does not show the destination holding the package
    /// active.
    DestinationNotActive {
        /// The run history shows.
        run_id: String,
    },
    /// The executable resolves through MacPortsBrew's own shim, and nothing
    /// yet records which provider a shim points at (D3; SE-092 S6 "Not
    /// closed (2)" stays open).
    PrimaryIsShim {
        /// The run history shows.
        run_id: String,
    },
    /// The executable does not resolve to the record's primary, or its
    /// resolution is unknown or absent.
    PrimaryNotConfirmed {
        /// The run history shows.
        run_id: String,
    },
}

/// The answer to "has this retain-source migration already been done?".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recognition {
    /// History holds nothing about this migration.
    NoRecord,
    /// History and a fresh capture agree it is done.
    Recognised {
        /// The run that did it.
        run_id: String,
    },
    /// History had something to say, and it was not enough.
    NotRecognised(NotRecognised),
}

/// Whether the migration `plan` describes is one `view` shows done and
/// `fresh` (a live capture taken for this run) still shows done (INV-16).
///
/// Recognised only when: no record is stamped later than `run_stamp`; the
/// key's latest stamp carries exactly one record; that record is a
/// retain-source `SUCCESS`; `fresh` shows the destination holding the
/// destination package active; and the executable the plan verifies resolves
/// to the record's `primaryAfter`, authoritatively or not, and to nothing
/// else. A shim resolution is [`NotRecognised::PrimaryIsShim`].
///
/// The executable is the destination package's name, the one the plan's
/// `VERIFY_DESTINATION` probes; a package with no executable of that name is
/// never recognised, so INV-16 then rests on `ADD_DESTINATION` being
/// idempotent (owed by a later slice).
#[must_use]
pub fn recognise_retained_run(
    view: &HistoryView,
    run_stamp: &str,
    plan: &MigrationPlan,
    fresh: &Snapshot,
) -> Recognition {
    let latest = view.latest_for_key();
    if view.newest_stamp().is_some_and(|newest| newest > run_stamp) {
        return Recognition::NotRecognised(NotRecognised::RecordFromTheFuture);
    }
    let record = match latest {
        [] => return Recognition::NoRecord,
        [one] => one,
        _ => return Recognition::NotRecognised(NotRecognised::Tie),
    };
    let run_id = record.id().to_owned();
    if record.status() != RunStatus::Success {
        return Recognition::NotRecognised(NotRecognised::LatestRunNotSuccess { run_id });
    }
    if !record.subject().retain_source {
        return Recognition::NotRecognised(NotRecognised::LatestRunRemovedSource { run_id });
    }
    let name = plan
        .destination_package
        .clone()
        .unwrap_or_else(|| plan.package.name.clone());
    let active = fresh.providers.get(&plan.destination).is_some_and(|p| {
        p.inventory
            .installed
            .iter()
            .any(|row| row.identity.name == name && row.active)
    });
    if !active {
        return Recognition::NotRecognised(NotRecognised::DestinationNotActive { run_id });
    }
    match resolves_to(fresh, &name, &record.subject().primary_after) {
        Resolved::Primary => Recognition::Recognised { run_id },
        Resolved::Shim => Recognition::NotRecognised(NotRecognised::PrimaryIsShim { run_id }),
        Resolved::Other => {
            Recognition::NotRecognised(NotRecognised::PrimaryNotConfirmed { run_id })
        }
    }
}

enum Resolved {
    Primary,
    Shim,
    Other,
}

/// What every provider's capture says `executable` resolves to. A shim
/// anywhere is a shim; otherwise every resolution found must name `primary`,
/// and at least one must exist.
fn resolves_to(fresh: &Snapshot, executable: &str, primary: &ProviderId) -> Resolved {
    let owners: Vec<&ExecutableOwner> = fresh
        .providers
        .values()
        .filter_map(|p| p.resolutions.get(executable))
        .map(|r| &r.owner)
        .collect();
    if owners
        .iter()
        .any(|o| matches!(o, ExecutableOwner::MacportsbrewShim))
    {
        return Resolved::Shim;
    }
    let primary_owns = |o: &&ExecutableOwner| matches!(o, ExecutableOwner::Provider { provider, .. } if provider == primary);
    if !owners.is_empty() && owners.iter().all(primary_owns) {
        Resolved::Primary
    } else {
        Resolved::Other
    }
}
