// SPDX-License-Identifier: Apache-2.0

//! Reading a stored run record back (`RunRecord::from_stored`, pass 08 slice
//! 1). A file never supplies its own status, dispatch or restorability
//! (INV-03): each is derived again and a record that disagrees is refused as
//! `HISTORY_UNUSABLE`, never silently corrected. Nothing a record states may
//! become a path outside the snapshot directory.

use macportsbrew_domain::history::{is_run_id, is_snapshot_file_name};
use macportsbrew_domain::{
    Direction, ErrorId, Outcome, PackageIdentity, ProviderId, RecordedStep, RecordedVersions,
    RunEnding, RunRecord, RunStart, RunStatus, RunSubject, StepKind, StepResult, StoredRunRecord,
    Version,
};
use serde_json::Value;

const ID: &str = "20261004T120000Z-0123456789abcdef";
const STARTED_AT: &str = "2026-10-04T12:00:00Z";

fn start() -> RunStart {
    RunStart {
        id: ID.into(),
        started_at: STARTED_AT.into(),
        tool_version: "0.1.0".into(),
    }
}

fn subject(package: &str) -> RunSubject {
    let source = ProviderId::new("alpha");
    let destination = ProviderId::new("beta");
    RunSubject {
        plan_schema_version: 1,
        policy_version: "2026.10.03-1".into(),
        package: PackageIdentity::new(source.clone(), package),
        direction: Direction {
            source,
            destination: destination.clone(),
        },
        destination_package: Some(package.into()),
        versions: RecordedVersions {
            source: Some(Version::new("1.0")),
            destination: None,
        },
        action: Outcome::MigrateTo {
            provider: destination.clone(),
        },
        overrode: Some(Outcome::ManualReview),
        retain_source: true,
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

fn verified() -> StepResult {
    StepResult::Verified {
        evidence: Vec::new(),
    }
}

/// Every step verified: a run the status rule calls a success.
fn completed_steps() -> Vec<RecordedStep> {
    vec![
        step(
            1,
            StepKind::SnapshotBefore,
            StepResult::SnapshotTaken,
            false,
        ),
        step(2, StepKind::AddDestination, verified(), false),
        step(3, StepKind::VerifyDestination, verified(), false),
        step(
            4,
            StepKind::RecordHistory,
            StepResult::RecordPendingReadBack,
            false,
        ),
    ]
}

/// The mutation was dispatched and the next step never reached.
fn halted_steps() -> Vec<RecordedStep> {
    vec![
        step(
            1,
            StepKind::SnapshotBefore,
            StepResult::SnapshotTaken,
            false,
        ),
        step(
            2,
            StepKind::AddDestination,
            StepResult::Halted {
                ends: Vec::new(),
                error: ErrorId::SafetyBlocked,
                evidence: Vec::new(),
            },
            true,
        ),
        step(
            3,
            StepKind::VerifyDestination,
            StepResult::NotReached,
            false,
        ),
    ]
}

/// The mutation never left the guard.
fn guarded_steps() -> Vec<RecordedStep> {
    vec![
        step(
            1,
            StepKind::AddDestination,
            StepResult::NotDispatchedMutating,
            false,
        ),
        step(2, StepKind::VerifyDestination, verified(), false),
    ]
}

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

const SNAPSHOT: &str = "snapshot-20261004T120000Z.json";

/// The snapshot a record names is fixed by its steps: one that took a snapshot
/// names it, and none other does (`from_stored` refuses the two to disagree).
fn snapshot_of(steps: &[RecordedStep]) -> Option<String> {
    steps
        .iter()
        .any(|s| matches!(s.result(), StepResult::SnapshotTaken))
        .then(|| SNAPSHOT.to_owned())
}

fn record(steps: Vec<RecordedStep>, ending: RunEnding, _snapshot: Option<&str>) -> RunRecord {
    let snapshot = snapshot_of(&steps);
    RunRecord::new(start(), subject("tool"), steps, ending, snapshot)
}

fn stored_json(record: &RunRecord) -> Value {
    serde_json::to_value(record.stored()).expect("a record serialises")
}

fn read_back(value: Value) -> Result<RunRecord, ErrorId> {
    let stored: StoredRunRecord =
        serde_json::from_value(value).map_err(|_| ErrorId::HistoryUnusable)?;
    RunRecord::from_stored(stored).map_err(|e| e.id)
}

#[test]
fn every_record_this_build_can_write_reads_back_as_itself() {
    for steps in [
        completed_steps(),
        halted_steps(),
        guarded_steps(),
        Vec::new(),
    ] {
        for ending in every_ending() {
            for snapshot in [None, Some(SNAPSHOT)] {
                let original = record(steps.clone(), ending.clone(), snapshot);
                let back = read_back(stored_json(&original)).expect("reads back");
                assert_eq!(back, original);
            }
        }
    }
}

#[test]
fn the_stored_record_carries_its_schema_version_first() {
    let json =
        serde_json::to_string(&record(completed_steps(), RunEnding::Completed, None).stored())
            .unwrap();
    assert!(json.starts_with(r#"{"schemaVersion":2,"id":""#), "{json}");
}

#[test]
fn invariant_03_a_stored_record_never_supplies_its_own_status_dispatch_or_restorability() {
    // A halted run: status FAILURE, a mutation dispatched.
    let original = record(
        halted_steps(),
        RunEnding::Halted {
            step: 2,
            error: ErrorId::SafetyBlocked,
        },
        None,
    );
    assert_eq!(original.status(), RunStatus::Failure);
    let base = stored_json(&original);

    let tamper = |edit: &dyn Fn(&mut Value)| {
        let mut value = base.clone();
        edit(&mut value);
        read_back(value)
    };
    // A file claiming success, a dispatch the steps contradict, a
    // restorability it has no business stating (derived on read, schema 2), or
    // a step dispatched whose result says it never left the loop: each
    // refused, none corrected.
    for (what, outcome) in [
        ("status", tamper(&|v| v["status"] = "SUCCESS".into())),
        ("no change", tamper(&|v| v["status"] = "NO_CHANGE".into())),
        (
            "mutationDispatched",
            tamper(&|v| v["mutationDispatched"] = false.into()),
        ),
        (
            "restorability",
            tamper(&|v| {
                v["restorability"] = serde_json::json!({"restorable": "NOTHING_TO_RESTORE"});
            }),
        ),
        (
            "a never-reached step dispatched",
            tamper(&|v| v["steps"][2]["dispatched"] = true.into()),
        ),
        (
            "a halted step undispatched",
            tamper(&|v| v["steps"][1]["dispatched"] = false.into()),
        ),
    ] {
        assert_eq!(outcome, Err(ErrorId::HistoryUnusable), "{what}");
    }
}

#[test]
fn invariant_03_a_partial_run_is_written_as_partial() {
    // The loop said "completed", but a step is not verified: written and read
    // back as PARTIAL, and a file that says SUCCESS is refused.
    let mut steps = completed_steps();
    steps[3] = step(4, StepKind::RecordHistory, StepResult::NotReached, false);
    steps[1] = step(
        2,
        StepKind::AddDestination,
        StepResult::Halted {
            ends: Vec::new(),
            error: ErrorId::SafetyBlocked,
            evidence: Vec::new(),
        },
        true,
    );
    let original = record(steps, RunEnding::Completed, None);
    assert_eq!(original.status(), RunStatus::Partial);
    let value = stored_json(&original);
    assert_eq!(value["status"], "PARTIAL");
    assert_eq!(read_back(value.clone()), Ok(original));

    let mut claimed = value;
    claimed["status"] = "SUCCESS".into();
    assert_eq!(read_back(claimed), Err(ErrorId::HistoryUnusable));
}

#[test]
fn a_record_of_another_schema_version_or_with_unknown_fields_is_unusable() {
    let base = stored_json(&record(completed_steps(), RunEnding::Completed, None));
    let tamper = |edit: &dyn Fn(&mut Value)| {
        let mut value = base.clone();
        edit(&mut value);
        read_back(value)
    };
    for (what, outcome) in [
        ("schema 1", tamper(&|v| v["schemaVersion"] = 1.into())),
        ("schema 3", tamper(&|v| v["schemaVersion"] = 3.into())),
        ("schema 0", tamper(&|v| v["schemaVersion"] = 0.into())),
        ("top level", tamper(&|v| v["extra"] = 1.into())),
        ("package", tamper(&|v| v["package"]["extra"] = 1.into())),
        ("direction", tamper(&|v| v["direction"]["extra"] = 1.into())),
        ("versions", tamper(&|v| v["versions"]["extra"] = 1.into())),
        ("step", tamper(&|v| v["steps"][0]["extra"] = 1.into())),
        ("ending", tamper(&|v| v["ending"]["extra"] = 1.into())),
        ("action", tamper(&|v| v["action"]["extra"] = 1.into())),
        (
            "a unit outcome",
            tamper(&|v| v["overrode"] = serde_json::json!({"kind": "MANUAL_REVIEW", "extra": 1})),
        ),
        (
            "a stored restorability",
            tamper(&|v| v["restorability"] = serde_json::json!({"restorable": "FROM_SNAPSHOT"})),
        ),
        (
            "a unit step result",
            tamper(&|v| {
                v["steps"][0]["result"] = serde_json::json!({"result": "NOT_REACHED", "extra": 1});
            }),
        ),
        (
            "missing field",
            tamper(&|v| {
                v.as_object_mut().unwrap().remove("snapshotPath");
            }),
        ),
        (
            "an environment change",
            tamper(&|v| v["environmentChanges"] = serde_json::json!(["SHIM"])),
        ),
    ] {
        assert_eq!(outcome, Err(ErrorId::HistoryUnusable), "{what}");
    }
}

#[test]
fn evidence_whose_result_disagrees_with_its_cause_is_unusable() {
    let base = stored_json(&record(completed_steps(), RunEnding::Completed, None));
    let with_evidence = |evidence: Value| {
        let mut value = base.clone();
        value["steps"][2]["result"]["evidence"] = serde_json::json!([evidence]);
        read_back(value)
    };
    assert!(
        with_evidence(serde_json::json!({"passed": true, "lineCount": 1, "truncated": false}))
            .is_ok()
    );
    // A failed read is legal evidence for a halt, never for a verified step:
    // the writer verifies a step only when every read passed (INV-03).
    let failed = serde_json::json!({"passed": false, "lineCount": 0, "truncated": false, "cause": "NAME_ABSENT"});
    let mut halted = stored_json(&record(halted_steps(), RunEnding::Completed, None));
    halted["steps"][1]["result"]["evidence"] = serde_json::json!([failed.clone()]);
    assert!(read_back(halted).is_ok());
    for evidence in [
        failed,
        serde_json::json!({"passed": true, "lineCount": 1, "truncated": false, "cause": "NAME_ABSENT"}),
        serde_json::json!({"passed": false, "lineCount": 1, "truncated": false}),
        serde_json::json!({"passed": true, "lineCount": 1, "truncated": false, "stdout": "x"}),
    ] {
        assert_eq!(
            with_evidence(evidence.clone()),
            Err(ErrorId::HistoryUnusable),
            "{evidence}"
        );
    }
}

#[test]
fn the_run_id_grammar_is_a_stamp_and_sixteen_lowercase_hex() {
    for good in [ID, "19700101T000000Z-ffffffffffffffff"] {
        assert!(is_run_id(good), "{good}");
    }
    for bad in [
        "",
        "run-1",
        "20261004T120000Z",
        "20261004T120000Z-",
        "20261004T120000Z-0123456789ABCDEF",
        "20261004T120000Z-0123456789abcde",
        "20261004T120000Z-0123456789abcdef0",
        "20261004X120000Z-0123456789abcdef",
        "20261004T120000Y-0123456789abcdef",
        "2026100aT120000Z-0123456789abcdef",
        "../../20261004T120000Z-0123456789abcdef",
        "20261004T120000Z-0123456789abcdef/..",
        "20261004T120000Z-0123456789abcdeg",
        "20261004T120000Z_0123456789abcdef",
    ] {
        assert!(!is_run_id(bad), "{bad}");
    }
}

#[test]
fn an_id_must_be_the_runs_own_start_time() {
    let base = stored_json(&record(completed_steps(), RunEnding::Completed, None));
    let tamper = |edit: &dyn Fn(&mut Value)| {
        let mut value = base.clone();
        edit(&mut value);
        read_back(value)
    };
    for (what, outcome) in [
        ("bad id", tamper(&|v| v["id"] = "run-1".into())),
        (
            "another instant",
            tamper(&|v| v["id"] = "20261004T120001Z-0123456789abcdef".into()),
        ),
        (
            "start time not RFC 3339",
            tamper(&|v| v["startedAt"] = "2026-10-04 12:00:00".into()),
        ),
        (
            "start time with an offset",
            tamper(&|v| v["startedAt"] = "2026-10-04T12:00:00+00:00".into()),
        ),
        (
            "the right instant, a bad suffix",
            tamper(&|v| v["id"] = "20261004T120000Z-NOT-HEX".into()),
        ),
        (
            "a start time whose zone is not Z",
            tamper(&|v| v["startedAt"] = "2026-10-04T12:00:00!".into()),
        ),
        (
            "start time agreeing with another id",
            tamper(&|v| v["startedAt"] = "2026-10-04T12:00:01Z".into()),
        ),
    ] {
        assert_eq!(outcome, Err(ErrorId::HistoryUnusable), "{what}");
    }
}

#[test]
fn a_snapshot_path_is_only_ever_a_snapshot_file_name() {
    assert!(is_snapshot_file_name("snapshot-20261004T120000Z.json"));
    assert!(is_snapshot_file_name(&format!(
        "snapshot-{}.json",
        "a".repeat(32)
    )));
    for bad in [
        "",
        "snapshot-.json",
        "snapshot-x.json/",
        "../snapshot-x.json",
        "snapshots/snapshot-x.json",
        "/tmp/snapshot-x.json",
        "snapshot-x/..json",
        "snapshot-x.y.json",
        "snapshot-x.JSON",
        "Snapshot-x.json",
        "snapshot-\u{1b}.json",
        "snapshot-x.json\0",
    ] {
        assert!(!is_snapshot_file_name(bad), "{bad:?}");
    }
    assert!(!is_snapshot_file_name(&format!(
        "snapshot-{}.json",
        "a".repeat(33)
    )));

    let base = stored_json(&record(halted_steps(), RunEnding::Completed, None));
    for bad in ["../snapshot-x.json", "/tmp/snapshot-x.json", "history"] {
        let mut value = base.clone();
        value["snapshotPath"] = bad.into();
        assert_eq!(read_back(value), Err(ErrorId::HistoryUnusable), "{bad}");
    }
}

#[test]
fn a_control_character_anywhere_is_unusable_and_a_format_character_is_not() {
    // A joiner is a legal part of a package name, as in a snapshot (pass 06
    // S3): refusing it would make `migrate --retain-source` unusable for that
    // package forever.
    let joined = RunRecord::new(
        start(),
        subject("a\u{200d}b"),
        completed_steps(),
        RunEnding::Completed,
        snapshot_of(&completed_steps()),
    );
    assert_eq!(read_back(stored_json(&joined)), Ok(joined));

    let base = stored_json(&record(completed_steps(), RunEnding::Completed, None));
    let tamper = |edit: &dyn Fn(&mut Value)| {
        let mut value = base.clone();
        edit(&mut value);
        read_back(value)
    };
    for (what, outcome) in [
        (
            "package name",
            tamper(&|v| v["package"]["name"] = "a\u{1b}b".into()),
        ),
        (
            "provider",
            tamper(&|v| v["direction"]["source"] = "al\npha".into()),
        ),
        (
            "tool version",
            tamper(&|v| v["toolVersion"] = "0.1\r".into()),
        ),
        (
            "policy version",
            tamper(&|v| v["policyVersion"] = "\u{7f}".into()),
        ),
        (
            "destination package",
            tamper(&|v| v["destinationPackage"] = "x\u{85}".into()),
        ),
        (
            "version",
            tamper(&|v| v["versions"]["source"] = "1\t0".into()),
        ),
        (
            "action",
            tamper(&|v| v["action"]["provider"] = "be\u{9b}ta".into()),
        ),
        (
            "step provider",
            tamper(&|v| v["steps"][0]["provider"] = "b\0".into()),
        ),
        ("primary", tamper(&|v| v["primaryAfter"] = "b\u{1}".into())),
    ] {
        assert_eq!(outcome, Err(ErrorId::HistoryUnusable), "{what}");
    }
}

#[test]
fn a_refusal_names_the_field_never_its_value() {
    let mut value = stored_json(&record(completed_steps(), RunEnding::Completed, None));
    value["package"]["name"] = "secret\u{1b}name".into();
    let stored: StoredRunRecord = serde_json::from_value(value).unwrap();
    let error = RunRecord::from_stored(stored).unwrap_err();
    assert_eq!(error.id, ErrorId::HistoryUnusable);
    assert!(!error.detail.contains("secret"), "{}", error.detail);
}

#[test]
fn a_manual_review_stored_as_carried_out_is_unusable() {
    let mut steps = completed_steps();
    steps[1] = step(2, StepKind::ManualReview, verified(), false);
    let base = stored_json(&record(steps, RunEnding::Completed, None));
    assert_eq!(read_back(base), Err(ErrorId::HistoryUnusable));
}

// --- A performed mutation and a mutating halt (pass 08 slice 4, D4) -------

use macportsbrew_domain::CommandEnd;

fn performed() -> StepResult {
    StepResult::Performed {
        ends: vec![CommandEnd::Exited { code: 0 }],
    }
}

/// A mutation performed and its verification halted; a second mutation that
/// halted on its second command, keeping the first command's end.
fn mutating_steps() -> Vec<Vec<RecordedStep>> {
    let snapshot = || {
        step(
            0,
            StepKind::SnapshotBefore,
            StepResult::SnapshotTaken,
            false,
        )
    };
    vec![
        vec![
            snapshot(),
            step(1, StepKind::AddDestination, performed(), false),
            step(
                2,
                StepKind::VerifyDestination,
                StepResult::Halted {
                    error: ErrorId::SafetyBlocked,
                    evidence: Vec::new(),
                    ends: Vec::new(),
                },
                true,
            ),
        ],
        vec![
            snapshot(),
            step(
                1,
                StepKind::RemoveSource,
                StepResult::Halted {
                    error: ErrorId::SafetyBlocked,
                    evidence: Vec::new(),
                    ends: vec![
                        CommandEnd::Exited { code: 0 },
                        CommandEnd::Exited { code: 1 },
                    ],
                },
                false,
            ),
        ],
        vec![
            snapshot(),
            step(1, StepKind::AddDestination, performed(), false),
        ],
    ]
}

#[test]
fn a_performed_mutation_and_a_mutating_halt_read_back_as_themselves() {
    for steps in mutating_steps() {
        for ending in every_ending() {
            let original = record(steps.clone(), ending.clone(), None);
            assert!(original.mutation_dispatched());
            let back = read_back(stored_json(&original)).expect("reads back");
            assert_eq!(back, original);
        }
    }
}

#[test]
fn a_mutating_halt_with_an_end_is_dispatched_whatever_the_caller_says() {
    let steps = &mutating_steps()[1];
    assert!(steps[1].dispatched());
}

#[test]
fn invariant_03_only_every_step_carried_out_is_success_and_performed_counts() {
    let all = record(
        vec![
            step(1, StepKind::AddDestination, performed(), false),
            step(2, StepKind::VerifyDestination, verified(), false),
        ],
        RunEnding::Completed,
        None,
    );
    assert_eq!(all.status(), RunStatus::Success);
    let one_short = record(mutating_steps()[0].clone(), RunEnding::Completed, None);
    assert_eq!(one_short.status(), RunStatus::Partial);
}

#[test]
fn a_performed_step_whose_ends_are_not_all_zero_or_empty_is_unusable() {
    let original = record(mutating_steps()[2].clone(), RunEnding::Completed, None);
    // The control: the path below is where the ends live, so a good value
    // reads back and only the bad ones are refused (the mutation proof found
    // this test once wrote to a path the reader never looked at).
    let mut control = stored_json(&original);
    assert!(
        control["steps"][1]["result"]["ends"].is_array(),
        "{control}"
    );
    control["steps"][1]["result"]["ends"] = serde_json::json!([{"end": "EXITED", "code": 0}]);
    assert!(read_back(control).is_ok());
    for ends in [
        serde_json::json!([]),
        serde_json::json!([{"end": "EXITED", "code": 1}]),
        serde_json::json!([{"end": "DID_NOT_FINISH"}]),
        serde_json::json!([{"end": "EXITED", "code": 0, "extra": true}]),
    ] {
        let mut value = stored_json(&original);
        value["steps"][1]["result"]["ends"] = ends.clone();
        assert_eq!(
            read_back(value).unwrap_err(),
            ErrorId::HistoryUnusable,
            "{ends}"
        );
    }
}

#[test]
fn a_read_halt_is_stored_without_an_ends_field() {
    let original = record(
        halted_steps(),
        RunEnding::Halted {
            step: 2,
            error: ErrorId::SafetyBlocked,
        },
        None,
    );
    let value = stored_json(&original);
    assert!(value["steps"][1]["result"].is_object(), "{value}");
    assert!(value["steps"][1]["result"].get("ends").is_none(), "{value}");
}

#[test]
fn a_mutations_result_on_a_step_no_mutation_runs_on_is_unusable() {
    // Slice 4 review, security 4: the writer puts PERFORMED, or a halt's
    // ends, only on ADD_DESTINATION and REMOVE_SOURCE.
    for kind in [
        StepKind::RecordHistory,
        StepKind::VerifyDestination,
        StepKind::SetPrimary,
    ] {
        for result in [
            performed(),
            StepResult::Halted {
                error: ErrorId::SafetyBlocked,
                evidence: Vec::new(),
                ends: vec![CommandEnd::Exited { code: 0 }],
            },
        ] {
            let original = record(
                vec![step(1, kind, result, false)],
                RunEnding::Completed,
                None,
            );
            assert_eq!(
                read_back(stored_json(&original)).unwrap_err(),
                ErrorId::HistoryUnusable,
                "{kind:?}"
            );
        }
    }
}

// --- The two brackets, and a record that never certifies its own write ------
// (pass 08 slice 5, D3 and D4)

#[test]
fn invariant_03_a_record_never_certifies_its_own_write() {
    let base = stored_json(&record(completed_steps(), RunEnding::Completed, None));
    // The control: the record this version writes reads back, and its last step
    // is the pending one.
    assert_eq!(
        base["steps"][3]["result"]["result"], "RECORD_PENDING_READ_BACK",
        "{base}"
    );
    assert!(read_back(base.clone()).is_ok());
    let tamper = |edit: &dyn Fn(&mut Value)| {
        let mut value = base.clone();
        edit(&mut value);
        read_back(value)
    };
    for (what, outcome) in [
        (
            "RECORD_HISTORY stored verified",
            tamper(&|v| {
                v["steps"][3]["result"] = serde_json::json!({"result": "VERIFIED", "evidence": []});
            }),
        ),
        (
            "a pending record on another step",
            tamper(&|v| {
                v["steps"][2]["result"] = serde_json::json!({"result": "RECORD_PENDING_READ_BACK"});
            }),
        ),
        (
            "a snapshot taken by another step",
            tamper(&|v| {
                v["steps"][2]["result"] = serde_json::json!({"result": "SNAPSHOT_TAKEN"});
            }),
        ),
        (
            "a snapshot name no step took",
            tamper(&|v| v["steps"][0]["result"] = serde_json::json!({"result": "NOT_REACHED"})),
        ),
        (
            "a snapshot taken and no name",
            tamper(&|v| v["snapshotPath"] = Value::Null),
        ),
    ] {
        assert_eq!(outcome, Err(ErrorId::HistoryUnusable), "{what}");
    }
}

#[test]
fn a_mutation_dispatched_without_a_snapshot_taken_first_is_unusable() {
    // 04: a snapshot is taken before the first mutating step. A record that
    // dispatched one without it is not a record this version writes.
    let steps = vec![
        step(
            1,
            StepKind::SnapshotBefore,
            StepResult::NotPerformedInThisBuild,
            false,
        ),
        step(2, StepKind::AddDestination, performed(), false),
    ];
    let original = RunRecord::new(
        start(),
        subject("tool"),
        steps,
        RunEnding::Halted {
            step: 2,
            error: ErrorId::SafetyBlocked,
        },
        None,
    );
    assert!(original.mutation_dispatched());
    assert_eq!(
        read_back(stored_json(&original)),
        Err(ErrorId::HistoryUnusable)
    );
    // The same steps with the snapshot step honestly taken read back.
    let mut taken = mutating_steps()[2].clone();
    taken.push(step(
        2,
        StepKind::RecordHistory,
        StepResult::RecordPendingReadBack,
        false,
    ));
    let honest = record(taken, RunEnding::Completed, None);
    assert_eq!(read_back(stored_json(&honest)), Ok(honest));
}

#[test]
fn a_read_only_loop_record_may_state_the_brackets_not_performed() {
    // `NOT_PERFORMED_IN_THIS_BUILD` stays legal where no mutation was dispatched.
    let original = record(
        vec![
            step(
                1,
                StepKind::SnapshotBefore,
                StepResult::NotPerformedInThisBuild,
                false,
            ),
            step(2, StepKind::VerifyDestination, verified(), false),
            step(
                3,
                StepKind::RecordHistory,
                StepResult::NotPerformedInThisBuild,
                false,
            ),
        ],
        RunEnding::Completed,
        None,
    );
    assert!(!original.mutation_dispatched());
    assert_eq!(read_back(stored_json(&original)), Ok(original));
}
