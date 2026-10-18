// SPDX-License-Identifier: Apache-2.0

//! `restorability`: derived from the record, the snapshot's look and the
//! record's standing, in the order `docs/spec/04-migration-and-safety.md`
//! (History) and the slice 5 plan (D8) give. Each row has its own case, and the
//! order is asserted by cases in which two rows would both match.

use macportsbrew_application::{restorability, SnapshotLook};
use macportsbrew_domain::{
    CommandEnd, Direction, ErrorId, NotRestorable, Outcome, PackageIdentity, ProviderId,
    RecordStanding, RecordedStep, RecordedVersions, Restorability, RunEnding, RunRecord, RunStart,
    RunSubject, StepKind, StepResult, Version,
};

const STARTED: &str = "2026-10-03T12:00:00Z";

fn record(steps: Vec<RecordedStep>, snapshot: Option<&str>) -> RunRecord {
    RunRecord::new(
        RunStart {
            id: "20261003T120000Z-00000000000000aa".into(),
            started_at: STARTED.into(),
            tool_version: "0.1.0".into(),
        },
        RunSubject {
            plan_schema_version: 1,
            policy_version: "2026-10-01".into(),
            package: PackageIdentity::new(ProviderId::new("a"), "pkg"),
            direction: Direction {
                source: ProviderId::new("a"),
                destination: ProviderId::new("b"),
            },
            destination_package: None,
            versions: RecordedVersions {
                source: Some(Version::new("1.0")),
                destination: None,
            },
            action: Outcome::MigrateTo {
                provider: ProviderId::new("b"),
            },
            overrode: None,
            retain_source: true,
            primary_after: ProviderId::new("a"),
        },
        steps,
        RunEnding::Completed,
        snapshot.map(str::to_owned),
    )
}

fn step(kind: StepKind, result: StepResult) -> RecordedStep {
    RecordedStep::new(0, kind, None, result, false)
}

fn performed() -> StepResult {
    StepResult::Performed {
        ends: vec![CommandEnd::Exited { code: 0 }],
    }
}

/// A run whose one mutating step ended as `result`.
fn mutating(result: StepResult, snapshot: Option<&str>) -> RunRecord {
    record(
        vec![
            step(StepKind::SnapshotBefore, StepResult::SnapshotTaken),
            step(StepKind::AddDestination, result),
        ],
        snapshot,
    )
}

fn present() -> SnapshotLook {
    SnapshotLook::Present {
        captured_at: STARTED.into(),
    }
}

const SNAP: Option<&str> = Some("snapshot-20261003T120000Z.json");

#[test]
fn a_clean_confirmed_run_with_its_own_snapshot_can_be_planned_from_it() {
    let r = mutating(performed(), SNAP);
    assert_eq!(
        restorability(&r, &present(), RecordStanding::Confirmed),
        Restorability::FromSnapshot
    );
}

#[test]
fn a_run_that_dispatched_nothing_has_nothing_to_restore_whatever_else_is_true() {
    let r = record(
        vec![step(
            StepKind::AddDestination,
            StepResult::NotDispatchedMutating,
        )],
        None,
    );
    for standing in [
        RecordStanding::Confirmed,
        RecordStanding::Unconfirmed,
        RecordStanding::Unknown,
    ] {
        for look in [
            present(),
            SnapshotLook::Missing,
            SnapshotLook::Unreadable,
            SnapshotLook::Present {
                captured_at: "2000-01-01T00:00:00Z".into(),
            },
        ] {
            assert_eq!(
                restorability(&r, &look, standing),
                Restorability::NothingToRestore,
                "{look:?} {standing:?}"
            );
        }
    }
}

#[test]
fn a_record_that_is_not_confirmed_is_never_offered_for_restore() {
    let r = mutating(performed(), SNAP);
    assert_eq!(
        restorability(&r, &present(), RecordStanding::Unconfirmed),
        Restorability::Not {
            reason: NotRestorable::RecordNotConfirmed
        }
    );
    assert_eq!(
        restorability(&r, &present(), RecordStanding::Unknown),
        Restorability::Not {
            reason: NotRestorable::StandingUnknown
        }
    );
}

#[test]
fn a_standing_that_is_not_confirmed_outranks_a_missing_snapshot() {
    // Two rows match; the earlier one names the reason.
    let r = mutating(performed(), SNAP);
    assert_eq!(
        restorability(&r, &SnapshotLook::Missing, RecordStanding::Unconfirmed),
        Restorability::Not {
            reason: NotRestorable::RecordNotConfirmed
        }
    );
    let no_snapshot = mutating(performed(), None);
    assert_eq!(
        restorability(&no_snapshot, &present(), RecordStanding::Unknown),
        Restorability::Not {
            reason: NotRestorable::StandingUnknown
        }
    );
}

#[test]
fn a_record_that_names_no_snapshot_has_no_snapshot_to_restore_from() {
    let r = mutating(performed(), None);
    // Even a snapshot that happens to be there is not the record's.
    assert_eq!(
        restorability(&r, &present(), RecordStanding::Confirmed),
        Restorability::Not {
            reason: NotRestorable::NoSnapshotTaken
        }
    );
}

#[test]
fn a_snapshot_that_is_gone_unreadable_or_another_runs_is_not_restored_from() {
    let r = mutating(performed(), SNAP);
    for (look, reason) in [
        (SnapshotLook::Missing, NotRestorable::SnapshotMissing),
        (SnapshotLook::Unreadable, NotRestorable::SnapshotUnreadable),
        (
            SnapshotLook::Present {
                captured_at: "2026-10-03T12:00:01Z".into(),
            },
            NotRestorable::SnapshotNotThisRuns,
        ),
    ] {
        assert_eq!(
            restorability(&r, &look, RecordStanding::Confirmed),
            Restorability::Not { reason },
            "{look:?}"
        );
    }
}

#[test]
fn a_command_that_did_not_end_in_a_known_zero_leaves_the_run_partly_restorable() {
    for result in [
        StepResult::Halted {
            error: ErrorId::ProviderTimeout,
            evidence: Vec::new(),
            ends: vec![CommandEnd::DidNotFinish],
        },
        StepResult::Halted {
            error: ErrorId::Io,
            evidence: Vec::new(),
            ends: vec![CommandEnd::Exited { code: 1 }],
        },
    ] {
        let r = mutating(result, SNAP);
        assert_eq!(
            restorability(&r, &present(), RecordStanding::Confirmed),
            Restorability::Partially {
                reason: NotRestorable::OutcomeUncertain
            }
        );
    }
}

#[test]
fn a_missing_snapshot_outranks_an_uncertain_outcome() {
    // Partial is a statement that a plan can still be built; without a snapshot
    // none can, so the stronger answer wins.
    let r = mutating(
        StepResult::Halted {
            error: ErrorId::Io,
            evidence: Vec::new(),
            ends: vec![CommandEnd::DidNotFinish],
        },
        SNAP,
    );
    assert_eq!(
        restorability(&r, &SnapshotLook::Missing, RecordStanding::Confirmed),
        Restorability::Not {
            reason: NotRestorable::SnapshotMissing
        }
    );
}

#[test]
fn a_record_that_names_no_snapshot_is_told_so_whatever_the_look_of_the_file_it_does_not_name() {
    // Mutation proof (slice 5): the shell looks `Missing` for a record that names
    // none, so the row "names no snapshot" must come before the rows that read the
    // look, or `history` would report a missing file where there never was one.
    let r = mutating(performed(), None);
    for look in [
        SnapshotLook::Missing,
        SnapshotLook::Unreadable,
        SnapshotLook::Present {
            captured_at: "2000-01-01T00:00:00Z".into(),
        },
    ] {
        assert_eq!(
            restorability(&r, &look, RecordStanding::Confirmed),
            Restorability::Not {
                reason: NotRestorable::NoSnapshotTaken
            },
            "{look:?}"
        );
    }
}

#[test]
fn only_a_dispatched_mutating_step_can_leave_the_outcome_uncertain() {
    // Mutation proof (slice 5): a read that halted after it was dispatched, and a
    // mutating step that was never dispatched, say nothing about whether a change
    // the run made is known.
    let snapshot = step(StepKind::SnapshotBefore, StepResult::SnapshotTaken);
    let added = step(StepKind::AddDestination, performed());
    let halted_read = RecordedStep::new(
        0,
        StepKind::VerifyDestination,
        None,
        StepResult::Halted {
            error: ErrorId::Io,
            evidence: Vec::new(),
            ends: Vec::new(),
        },
        true,
    );
    assert!(halted_read.dispatched() && !halted_read.kind().is_mutating());
    let r = record(vec![snapshot.clone(), added.clone(), halted_read], SNAP);
    assert_eq!(
        restorability(&r, &present(), RecordStanding::Confirmed),
        Restorability::FromSnapshot
    );
    for result in [StepResult::NotReached, StepResult::NotDispatchedMutating] {
        let never = step(StepKind::RemoveSource, result);
        assert!(!never.dispatched() && never.kind().is_mutating());
        let r = record(vec![snapshot.clone(), added.clone(), never], SNAP);
        assert_eq!(
            restorability(&r, &present(), RecordStanding::Confirmed),
            Restorability::FromSnapshot
        );
    }
}
