// SPDX-License-Identifier: Apache-2.0

//! The history record's rules (`crates/domain/src/history.rs`): partial is
//! never success (INV-03), a run that dispatched no mutation is never a
//! partial migration (SE-092), and the serialised shape is fixed and ordered
//! (INV-15). The record built from a real run is tested in
//! `crates/application/tests/history.rs`.

use macportsbrew_domain::{
    Direction, ErrorId, NotRestorable, Outcome, PackageIdentity, ProcessEnd, ProviderId,
    ReadEvidence, ReadOutput, RecordedStep, RecordedVersions, Restorability, RunEnding, RunRecord,
    RunStart, RunStatus, RunSubject, StepKind, StepResult, Verdict, Version,
};

fn every_ending() -> Vec<RunEnding> {
    vec![
        RunEnding::Completed,
        RunEnding::PartialSuccess {
            step: 3,
            error: ErrorId::SafetyBlocked,
        },
        RunEnding::Halted {
            step: 2,
            error: ErrorId::SafetyUnverifiable,
        },
        RunEnding::Refused { refusals: 1 },
    ]
}

fn start() -> RunStart {
    RunStart {
        id: "run-1".into(),
        started_at: "2026-10-01T00:00:00Z".into(),
        tool_version: "0.1.0".into(),
    }
}

fn subject() -> RunSubject {
    let source = ProviderId::new("alpha");
    let destination = ProviderId::new("beta");
    RunSubject {
        plan_schema_version: 1,
        policy_version: "2026.09.30".into(),
        package: PackageIdentity::new(source.clone(), "tool"),
        direction: Direction {
            source,
            destination: destination.clone(),
        },
        destination_package: Some("tool".into()),
        versions: RecordedVersions {
            source: Some(Version::new("1.0")),
            destination: None,
        },
        action: Outcome::MigrateTo {
            provider: destination.clone(),
        },
        overrode: None,
        retain_source: false,
        primary_after: destination,
    }
}

fn step(index: u32, kind: StepKind, result: StepResult, after_dispatch: bool) -> RecordedStep {
    RecordedStep::new(
        index,
        kind,
        Some(ProviderId::new("beta")),
        result,
        after_dispatch,
    )
}

fn halted() -> StepResult {
    StepResult::Halted {
        ends: Vec::new(),
        error: ErrorId::SafetyBlocked,
        evidence: Vec::new(),
    }
}

/// A run whose `ADD_DESTINATION` was handed to a runner.
fn mutating_steps() -> Vec<RecordedStep> {
    vec![
        step(
            1,
            StepKind::SnapshotBefore,
            StepResult::NotPerformedInThisBuild,
            false,
        ),
        step(2, StepKind::AddDestination, halted(), true),
        step(
            3,
            StepKind::VerifyDestination,
            StepResult::NotReached,
            false,
        ),
    ]
}

/// The same run as this build produces it: the mutation never left the guard.
fn guarded_steps() -> Vec<RecordedStep> {
    vec![
        step(
            1,
            StepKind::SnapshotBefore,
            StepResult::NotPerformedInThisBuild,
            false,
        ),
        step(
            2,
            StepKind::AddDestination,
            StepResult::NotDispatchedMutating,
            false,
        ),
        step(
            3,
            StepKind::VerifyDestination,
            StepResult::Verified {
                evidence: Vec::new(),
            },
            false,
        ),
        step(
            4,
            StepKind::RecordHistory,
            StepResult::NotPerformedInThisBuild,
            false,
        ),
    ]
}

#[test]
fn invariant_03_the_status_rule_never_records_partial_as_success() {
    let partial = RunEnding::PartialSuccess {
        step: 3,
        error: ErrorId::SafetyBlocked,
    };
    assert_eq!(RunStatus::of(&partial, true), RunStatus::Partial);
    let record = RunRecord::new(start(), subject(), mutating_steps(), partial, None);
    assert_eq!(record.status(), RunStatus::Partial);
    assert_ne!(record.status(), RunStatus::Success);
    // Success is the completed mutating run and nothing else.
    for ending in every_ending() {
        let status = RunStatus::of(&ending, true);
        assert_eq!(
            status == RunStatus::Success,
            ending == RunEnding::Completed,
            "{ending:?} recorded as {status:?}"
        );
    }
    assert_eq!(
        RunStatus::of(
            &RunEnding::Halted {
                step: 2,
                error: ErrorId::SafetyBlocked
            },
            true
        ),
        RunStatus::Failure
    );
}

#[test]
fn a_run_that_dispatched_no_mutation_is_never_recorded_as_a_partial_migration() {
    for ending in every_ending() {
        assert_eq!(
            RunStatus::of(&ending, false),
            RunStatus::NoChange,
            "{ending:?}"
        );
        let record = RunRecord::new(start(), subject(), guarded_steps(), ending, None);
        assert!(!record.mutation_dispatched());
        assert_eq!(record.status(), RunStatus::NoChange);
    }
}

#[test]
fn only_a_dispatched_mutating_step_makes_a_run_a_mutating_one() {
    // A dispatched read is not a mutation.
    let reads_only = RunRecord::new(
        start(),
        subject(),
        guarded_steps(),
        RunEnding::PartialSuccess {
            step: 1,
            error: ErrorId::SafetyBlocked,
        },
        None,
    );
    let read = &reads_only.steps()[2];
    assert_eq!(
        (read.index(), read.kind()),
        (3, StepKind::VerifyDestination)
    );
    assert!(matches!(read.result(), StepResult::Verified { .. }));
    assert!(read.dispatched());
    assert_eq!(reads_only.subject(), &subject());
    assert!(matches!(
        reads_only.ending(),
        RunEnding::PartialSuccess { step: 1, .. }
    ));
    assert!(!reads_only.mutation_dispatched());

    let mutated = RunRecord::new(
        start(),
        subject(),
        mutating_steps(),
        RunEnding::Halted {
            step: 2,
            error: ErrorId::SafetyBlocked,
        },
        None,
    );
    assert!(mutated.mutation_dispatched());
    assert_eq!(mutated.status(), RunStatus::Failure);
}

#[test]
fn whether_a_step_was_dispatched_is_fixed_by_its_result_where_the_result_says() {
    let verified = StepResult::Verified {
        evidence: Vec::new(),
    };
    assert!(step(1, StepKind::AddDestination, verified, false).dispatched());
    for not_run in [
        StepResult::NotPerformedInThisBuild,
        StepResult::NotDispatchedMutating,
        StepResult::NotReached,
    ] {
        assert!(!step(1, StepKind::AddDestination, not_run, true).dispatched());
    }
    assert!(!step(1, StepKind::AddDestination, halted(), false).dispatched());
    // A halt that kept a read was dispatched, whatever the caller says.
    let read = ReadOutput {
        end: ProcessEnd::Exited { code: 0 },
        stdout: "tool 1.0\n",
        truncated: false,
    };
    let with_read = StepResult::Halted {
        ends: Vec::new(),
        error: ErrorId::SafetyBlocked,
        evidence: vec![ReadEvidence::of(&read, Verdict::Passed)],
    };
    assert!(step(1, StepKind::AddDestination, with_read, false).dispatched());
    assert!(step(1, StepKind::AddDestination, halted(), true).dispatched());
}

#[test]
fn a_record_stores_no_restorability_it_is_derived_when_read() {
    // History schema 2: whether a run can be put back depends on a file that
    // can disappear after the record was written, so the record cannot state it.
    let record = RunRecord::new(
        start(),
        subject(),
        mutating_steps(),
        RunEnding::Halted {
            step: 2,
            error: ErrorId::SafetyBlocked,
        },
        None,
    );
    let text = serde_json::to_string(&record).expect("serialises");
    assert!(!text.contains("restorab"), "{text}");
    assert_eq!(record.snapshot_path(), None);
}

#[test]
fn the_derived_restorability_serialises_in_one_fixed_shape() {
    for (value, expected) in [
        (
            Restorability::NothingToRestore,
            r#"{"restorable":"NOTHING_TO_RESTORE"}"#,
        ),
        (
            Restorability::FromSnapshot,
            r#"{"restorable":"FROM_SNAPSHOT"}"#,
        ),
        (
            Restorability::Partially {
                reason: NotRestorable::OutcomeUncertain,
            },
            r#"{"restorable":"PARTIALLY","reason":"OUTCOME_UNCERTAIN"}"#,
        ),
        (
            Restorability::Not {
                reason: NotRestorable::RecordNotConfirmed,
            },
            r#"{"restorable":"NOT","reason":"RECORD_NOT_CONFIRMED"}"#,
        ),
    ] {
        assert_eq!(serde_json::to_string(&value).unwrap(), expected);
    }
}

#[test]
fn the_start_time_is_the_one_supplied() {
    let record = RunRecord::new(
        start(),
        subject(),
        guarded_steps(),
        RunEnding::Completed,
        None,
    );
    assert_eq!(record.started_at(), "2026-10-01T00:00:00Z");
    assert_eq!(record.id(), "run-1");
}

#[test]
fn a_run_record_serialises_in_one_fixed_order() {
    let record = || {
        RunRecord::new(
            start(),
            subject(),
            guarded_steps(),
            RunEnding::PartialSuccess {
                step: 2,
                error: ErrorId::SafetyBlocked,
            },
            None,
        )
    };
    let text = serde_json::to_string(&record()).expect("serialises");
    assert_eq!(text, serde_json::to_string(&record()).expect("serialises"));
    // The fields `04-migration-and-safety.md` (History) lists, in its order.
    let keys = [
        "\"id\"",
        "\"startedAt\"",
        "\"toolVersion\"",
        "\"planSchemaVersion\"",
        "\"policyVersion\"",
        "\"package\"",
        "\"direction\"",
        "\"destinationPackage\"",
        "\"versions\"",
        "\"action\"",
        "\"overrode\"",
        "\"retainSource\"",
        "\"primaryAfter\"",
        "\"environmentChanges\"",
        "\"steps\"",
        "\"ending\"",
        "\"status\"",
        "\"mutationDispatched\"",
        "\"snapshotPath\"",
    ];
    let mut from = 0;
    for key in keys {
        let at = text[from..]
            .find(key)
            .unwrap_or_else(|| panic!("{key} missing or out of order in {text}"));
        from += at + key.len();
    }
    assert!(text.contains("\"status\":\"NO_CHANGE\""), "{text}");
    assert!(
        text.contains("\"result\":\"NOT_DISPATCHED_MUTATING\""),
        "{text}"
    );
    assert!(text.contains("\"environmentChanges\":[]"), "{text}");
}

#[test]
fn a_completion_the_steps_do_not_bear_out_is_never_success() {
    // A stale or wrong ending, supplied beside steps that halted after a
    // mutation was dispatched: INV-03 holds against the caller.
    let record = RunRecord::new(
        start(),
        subject(),
        mutating_steps(),
        RunEnding::Completed,
        None,
    );
    assert!(record.mutation_dispatched());
    assert_eq!(record.status(), RunStatus::Partial);
    let empty = RunRecord::new(start(), subject(), Vec::new(), RunEnding::Completed, None);
    assert_ne!(empty.status(), RunStatus::Success);
    // Every step verified, the mutating one included: success.
    let verified = || StepResult::Verified {
        evidence: Vec::new(),
    };
    let done = RunRecord::new(
        start(),
        subject(),
        vec![
            step(1, StepKind::AddDestination, verified(), true),
            step(2, StepKind::VerifyDestination, verified(), true),
        ],
        RunEnding::Completed,
        None,
    );
    assert_eq!(done.status(), RunStatus::Success);
}

#[test]
fn the_marker_gives_a_record_its_standing_and_an_unreadable_one_gives_none() {
    use macportsbrew_domain::{MarkerState, RecordStanding};
    let id = "20261004T120000Z-0123456789abcdef";
    let other = "20261004T130000Z-0123456789abcdef";
    assert_eq!(
        MarkerState::Absent.standing_of(id),
        RecordStanding::Confirmed
    );
    assert_eq!(
        MarkerState::Names(id.into()).standing_of(id),
        RecordStanding::Unconfirmed
    );
    // A marker that names another run says nothing against this record.
    assert_eq!(
        MarkerState::Names(other.into()).standing_of(id),
        RecordStanding::Confirmed
    );
    // Fail closed: what cannot be read confirms nothing.
    assert_eq!(
        MarkerState::Unreadable.standing_of(id),
        RecordStanding::Unknown
    );
}
