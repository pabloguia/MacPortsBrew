// SPDX-License-Identifier: Apache-2.0

//! What a restore asks the planner for (pass 08 slice 5, Part C; D9).
//!
//! **A restore is a migration in the other direction, not a special case**
//! (`04-migration-and-safety.md`, "Undoing a migration"; INV-17). So nothing
//! here plans: this module turns a run's record and the snapshot it took into
//! the [`PlanRequest`] the one planner is given, and checks that what the
//! planner returned is the package the run touched. Step ordering (INV-01),
//! reverse dependencies within one provider (INV-04) and the `STATEFUL` /
//! `SERVICE` stop (INV-19) are the planner's, and are not re-implemented here.
//!
//! It names no provider: the direction comes from the record.
//!
//! **What the request removes.** Undoing a run that added a package removes
//! what the run added, and only that. The destination package is removed only
//! if the record shows the add was dispatched and ended in a known exit 0
//! **and** the snapshot, taken before the first change, shows the destination
//! did not already hold it. Anything less keeps it: removing a package that
//! predates the run, or one the run never added, would be the worst thing an
//! undo could do.

use crate::planner::PlanRequest;
use macportsbrew_domain::{
    MigrationPlan, ProviderPresence, RunRecord, Snapshot, StepKind, StepResult,
};
use serde::Serialize;

/// Why a restore plan keeps the package the run added.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KeepReason {
    /// The record does not show the add ending in a known exit 0, so the run
    /// is not known to have added anything.
    AddNotPerformed,
    /// The snapshot shows the destination already held the package before the
    /// run: it is not the run's to remove.
    DestinationHeldItBefore,
    /// The snapshot does not establish what the destination held (the provider
    /// was not present and read), so the package is not known to be the run's.
    BeforeStateNotEstablished,
}

/// Whether the plan removes the destination package.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Removal {
    /// The run added it and the snapshot shows it was not there before.
    Removes,
    /// It stays, for this reason.
    Keeps(KeepReason),
}

/// What the planner is asked for, and what the plan must turn out to be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreIntent {
    /// The request: the package as the run's destination spells it, back to the
    /// run's source provider, keeping the destination unless it is the run's to
    /// remove.
    pub request: PlanRequest,
    /// The package the run moved, as the run's source provider spells it: what
    /// the plan's destination package must be.
    pub expected_package: String,
    /// The provider the run moved it from: where the plan must put it back.
    pub restore_to: macportsbrew_domain::ProviderId,
    /// The provider the run moved it to: where the plan must take it from.
    pub restore_from: macportsbrew_domain::ProviderId,
    /// Whether the destination package is removed.
    pub removal: Removal,
    /// The version the run's source held, as the record states it. **Not a
    /// promise**: the provider installs what it offers now.
    pub recorded_version: Option<String>,
}

/// Why no restore request exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreRefusal {
    /// The run dispatched no mutating step: there is nothing to put back.
    NothingDispatched,
    /// The plan the planner built is not for the package the run touched
    /// (a one-to-many or one-way mapping, or a different holder). Never a
    /// silent substitute.
    NotTheRunsPackage {
        /// What the plan would restore.
        planned: String,
        /// What the run moved.
        recorded: String,
    },
}

/// The request that puts back what `record` changed, from `snapshot`, the
/// snapshot the run took before its first change.
///
/// # Errors
/// [`RestoreRefusal::NothingDispatched`] for a run that changed nothing.
pub fn restore_request(
    record: &RunRecord,
    snapshot: &Snapshot,
) -> Result<RestoreIntent, RestoreRefusal> {
    if !record.mutation_dispatched() {
        return Err(RestoreRefusal::NothingDispatched);
    }
    let subject = record.subject();
    let source = &subject.direction.source;
    let destination = &subject.direction.destination;
    // The package as the destination spells it; a run with no selected mapping
    // kept one name on both sides.
    let at_destination = subject
        .destination_package
        .clone()
        .unwrap_or_else(|| subject.package.name.clone());
    let added = record.steps().iter().any(|step| {
        step.kind() == StepKind::AddDestination
            && step.dispatched()
            && matches!(step.result(), StepResult::Performed { .. })
    });
    let removal = if added {
        match snapshot.providers.get(destination) {
            Some(provider) if provider.detection.presence == ProviderPresence::Present => {
                if provider
                    .inventory
                    .installed_names()
                    .contains(&at_destination)
                {
                    Removal::Keeps(KeepReason::DestinationHeldItBefore)
                } else {
                    Removal::Removes
                }
            }
            _ => Removal::Keeps(KeepReason::BeforeStateNotEstablished),
        }
    } else {
        Removal::Keeps(KeepReason::AddNotPerformed)
    };
    Ok(RestoreIntent {
        request: PlanRequest {
            package: at_destination,
            to: source.clone(),
            retain_source: removal != Removal::Removes,
        },
        expected_package: subject.package.name.clone(),
        restore_to: source.clone(),
        restore_from: destination.clone(),
        removal,
        recorded_version: subject
            .versions
            .source
            .as_ref()
            .map(|v| v.as_str().to_owned()),
    })
}

/// Checks the plan the planner built restores the package the run moved, from
/// where the run put it, to where it came from.
///
/// # Errors
/// [`RestoreRefusal::NotTheRunsPackage`], naming both.
pub fn check_same_package(
    intent: &RestoreIntent,
    plan: &MigrationPlan,
) -> Result<(), RestoreRefusal> {
    let planned = plan
        .destination_package
        .clone()
        .unwrap_or_else(|| plan.package.name.clone());
    if planned == intent.expected_package
        && plan.destination == intent.restore_to
        && plan.source == intent.restore_from
    {
        return Ok(());
    }
    Err(RestoreRefusal::NotTheRunsPackage {
        planned: format!("{} ({} -> {})", planned, plan.source, plan.destination),
        recorded: format!(
            "{} ({} -> {})",
            intent.expected_package, intent.restore_from, intent.restore_to
        ),
    })
}

/// Something the person reviewing a restore plan should know that the plan
/// itself does not say. Kept **outside** the plan, which stays exactly a
/// `MigrationPlan` (`restore.v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "note", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RestoreNote {
    /// The provider installs what it offers now, not what the snapshot held.
    #[serde(rename_all = "camelCase")]
    NotVersionExact {
        /// The package, as the run's source spelled it.
        package: String,
        /// The version the record states, when it states one.
        recorded_version: Option<String>,
    },
    /// The destination package stays, and why.
    KeepsDestination {
        /// Why.
        reason: KeepReason,
    },
    /// The run's record is not confirmed (the interrupted-run marker still names
    /// it); the plan is shown so it can be reviewed before the marker is dealt
    /// with, and is not offered for execution.
    RecordNotConfirmed,
    /// A command the run dispatched did not end in a known exit 0, so the
    /// machine's state after it is not known; the plan was built from the live
    /// machine, which is what it is judged against.
    OutcomeUncertain,
}

/// The notes for a restore of this run: `uncertain` is whether a dispatched
/// command's end is unknown, `unconfirmed` whether the record is not confirmed.
#[must_use]
pub fn notes(intent: &RestoreIntent, uncertain: bool, unconfirmed: bool) -> Vec<RestoreNote> {
    let mut notes = vec![RestoreNote::NotVersionExact {
        package: intent.expected_package.clone(),
        recorded_version: intent.recorded_version.clone(),
    }];
    if let Removal::Keeps(reason) = intent.removal {
        notes.push(RestoreNote::KeepsDestination { reason });
    }
    if unconfirmed {
        notes.push(RestoreNote::RecordNotConfirmed);
    }
    if uncertain {
        notes.push(RestoreNote::OutcomeUncertain);
    }
    notes
}
