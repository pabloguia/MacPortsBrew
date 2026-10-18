// SPDX-License-Identifier: Apache-2.0

//! `history` and `history show <id>`, end to end: the built binary over a
//! scratch state directory (pass 08 slice 5, Part B; D7, D8).
//!
//! Nothing here touches a real state directory or a live installation: every
//! run names `MACPORTSBREW_STATE_DIR`, and the records and snapshots are written
//! with the same stores a mutating run uses.

use macportsbrew_adapters::persistence::history_store::{record_file_name, write_record};
use macportsbrew_adapters::persistence::run_lock::acquire;
use macportsbrew_adapters::persistence::run_marker;
use macportsbrew_adapters::persistence::snapshot_store::{load_snapshot, save_snapshot_durable};
use macportsbrew_adapters::persistence::state_dir::StateDir;
use macportsbrew_adapters::system::machine::effective_uid;
use macportsbrew_domain::{
    Direction, ErrorId, InterruptedRun, Outcome, PackageIdentity, ProviderId, RecordedStep,
    RecordedVersions, RunEnding, RunRecord, RunStart, RunSubject, StepKind, StepResult, Version,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");
const SNAPSHOT_FIXTURE: &str = "fixtures/snapshots/both-providers-intel-macos26-live.json";
const RECORDING: &str = "fixtures/recordings/both-providers-arm64-macos15.json";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "macportsbrew-history-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temporary directory");
        Self(dir)
    }

    fn state_dir(&self) -> PathBuf {
        self.0.join("state")
    }

    /// The state directory made as the tool makes it.
    fn state(&self) -> StateDir {
        let state = StateDir::resolve(None, Some(self.state_dir().to_str().unwrap()))
            .expect("a state directory");
        state.ensure().expect("created");
        state
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn euid() -> u32 {
    effective_uid().expect("the effective user")
}

fn run(scratch: &Scratch, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(root())
        .env("MACPORTSBREW_STATE_DIR", scratch.state_dir())
        .env_remove("MACPORTSBREW_LANG")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .expect("spawning the binary")
}

fn json_of(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON on stdout")
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("an exit code")
}

fn id_of(stamp: &str) -> String {
    format!("{stamp}-00000000000000aa")
}

fn started_at(stamp: &str) -> String {
    // 20261003T120000Z -> 2026-10-03T12:00:00Z
    format!(
        "{}-{}-{}T{}:{}:{}Z",
        &stamp[0..4],
        &stamp[4..6],
        &stamp[6..8],
        &stamp[9..11],
        &stamp[11..13],
        &stamp[13..15]
    )
}

fn subject() -> RunSubject {
    RunSubject {
        plan_schema_version: 1,
        policy_version: "2026-10-01".into(),
        package: PackageIdentity::new(ProviderId::new("homebrew"), "htop"),
        direction: Direction {
            source: ProviderId::new("homebrew"),
            destination: ProviderId::new("macports"),
        },
        destination_package: Some("htop".into()),
        versions: RecordedVersions {
            source: Some(Version::new("3.4.1")),
            destination: None,
        },
        action: Outcome::MigrateTo {
            provider: ProviderId::new("macports"),
        },
        overrode: None,
        retain_source: true,
        primary_after: ProviderId::new("homebrew"),
    }
}

fn start(stamp: &str) -> RunStart {
    RunStart {
        id: id_of(stamp),
        started_at: started_at(stamp),
        tool_version: "0.1.0".into(),
    }
}

/// A run that changed nothing: the gate refused it, so no snapshot was taken.
fn refused_run(stamp: &str) -> RunRecord {
    let step = |index, kind| {
        RecordedStep::new(
            index,
            kind,
            None,
            StepResult::NotPerformedInThisBuild,
            false,
        )
    };
    RunRecord::new(
        start(stamp),
        subject(),
        vec![
            step(0, StepKind::SnapshotBefore),
            step(1, StepKind::RecordHistory),
        ],
        RunEnding::Refused { refusals: 1 },
        None,
    )
}

/// A run that added the package and verified it, with the snapshot it took
/// first. `halted` makes the add end with a status that is not 0.
fn mutating_run(stamp: &str, snapshot: &str) -> RunRecord {
    let step = |index, kind, result| RecordedStep::new(index, kind, None, result, false);
    RunRecord::new(
        start(stamp),
        subject(),
        vec![
            step(0, StepKind::SnapshotBefore, StepResult::SnapshotTaken),
            step(
                1,
                StepKind::AddDestination,
                StepResult::Performed {
                    ends: vec![macportsbrew_domain::CommandEnd::Exited { code: 0 }],
                },
            ),
            step(
                2,
                StepKind::VerifyDestination,
                StepResult::Verified {
                    evidence: Vec::new(),
                },
            ),
            step(
                3,
                StepKind::RecordHistory,
                StepResult::RecordPendingReadBack,
            ),
        ],
        RunEnding::Completed,
        Some(snapshot.to_owned()),
    )
}

/// Writes the snapshot a run named, captured at the run's own start.
fn seed_snapshot(scratch: &Scratch, stamp: &str) -> String {
    let mut snapshot = load_snapshot(&root().join(SNAPSHOT_FIXTURE)).expect("the fixture");
    snapshot.provenance.captured_at = started_at(stamp);
    save_snapshot_durable(&scratch.state().snapshots(), &snapshot).expect("the snapshot")
}

fn seed_record(scratch: &Scratch, record: &RunRecord) {
    write_record(&scratch.state(), record, euid()).expect("the record");
}

#[test]
fn invariant_15_history_refuses_a_recording_and_a_snapshot_file_before_reading_anything() {
    let scratch = Scratch::new("replay");
    seed_record(&scratch, &refused_run("20261001T100000Z"));
    let id = id_of("20261001T100000Z");
    let recording = root().join(RECORDING);
    let snapshot = root().join(SNAPSHOT_FIXTURE);
    for scope in [
        ["--replay", recording.to_str().unwrap()],
        ["--from-snapshot", snapshot.to_str().unwrap()],
    ] {
        for command in [vec!["history"], vec!["history", "show", id.as_str()]] {
            let argv: Vec<&str> = scope.iter().copied().chain(command).collect();
            let output = run(&scratch, &argv);
            assert_eq!(
                code(&output),
                ErrorId::SafetyBlocked.exit_code(),
                "{argv:?}"
            );
            assert!(output.stdout.is_empty(), "{argv:?}");
        }
    }
}

#[test]
fn history_on_a_machine_with_no_state_directory_says_so_and_creates_nothing() {
    let scratch = Scratch::new("empty");
    let output = run(&scratch, &["history"]);
    assert_eq!(code(&output), 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("No runs are recorded"));
    let document = json_of(&run(&scratch, &["--json", "history"]));
    assert_eq!(document["total"], 0);
    assert_eq!(document["runs"].as_array().unwrap().len(), 0);
    assert!(
        !scratch.state_dir().exists(),
        "reading history must not create the state directory"
    );
}

#[test]
fn the_list_is_newest_first_limited_and_counts_what_it_did_not_show() {
    let scratch = Scratch::new("order");
    for stamp in ["20261002T100000Z", "20261001T100000Z", "20261003T100000Z"] {
        seed_record(&scratch, &refused_run(stamp));
    }
    let document = json_of(&run(&scratch, &["--json", "history", "--limit", "2"]));
    assert_eq!(document["schemaVersion"], 1);
    assert_eq!(document["total"], 3);
    assert_eq!(document["limit"], 2);
    let ids: Vec<&str> = document["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["runId"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            id_of("20261003T100000Z").as_str(),
            id_of("20261002T100000Z").as_str()
        ]
    );
    let text = run(&scratch, &["history", "--limit", "2"]);
    let text = String::from_utf8_lossy(&text.stdout).into_owned();
    assert!(text.contains("2 of 3 runs"), "{text}");
}

#[test]
fn a_limit_outside_one_to_two_hundred_is_a_usage_error() {
    let scratch = Scratch::new("limit");
    for limit in ["0", "201", "-1", "many"] {
        let output = run(&scratch, &["history", "--limit", limit]);
        assert_eq!(code(&output), 2, "--limit {limit}");
    }
    assert_eq!(code(&run(&scratch, &["history", "--limit", "200"])), 0);
}

#[test]
fn an_id_that_is_not_a_run_id_is_usage_and_a_valid_one_with_no_record_is_not_found() {
    let scratch = Scratch::new("ids");
    for bad in ["../x", "run-1", "", "20261001T100000Z-zzzzzzzzzzzzzzzz"] {
        let output = run(&scratch, &["history", "show", bad]);
        assert_eq!(code(&output), 2, "{bad:?}");
    }
    let missing = id_of("20261001T100000Z");
    let output = run(&scratch, &["history", "show", &missing]);
    assert_eq!(code(&output), ErrorId::RunNotFound.exit_code());
    assert!(String::from_utf8_lossy(&output.stderr).contains("macportsbrew history"));
    assert!(!scratch.state_dir().exists(), "nothing is created");
}

#[test]
fn an_unusable_record_in_the_page_fails_the_command_instead_of_shortening_the_list() {
    let scratch = Scratch::new("unusable");
    seed_record(&scratch, &refused_run("20261001T100000Z"));
    let bad = id_of("20261002T100000Z");
    let file = scratch
        .state()
        .history()
        .join(record_file_name(&bad).expect("a record file name"));
    std::fs::write(&file, "{ not a record").unwrap();
    std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let output = run(&scratch, &["history"]);
    assert_eq!(code(&output), ErrorId::HistoryUnusable.exit_code());
    assert!(String::from_utf8_lossy(&output.stderr).contains(&bad));
}

#[test]
fn restorability_is_derived_from_the_ends_the_snapshot_and_the_standing() {
    let scratch = Scratch::new("restorability");
    let stamp = "20261003T120000Z";
    let snapshot = seed_snapshot(&scratch, stamp);
    seed_record(&scratch, &mutating_run(stamp, &snapshot));
    let id = id_of(stamp);

    // A confirmed run with its snapshot: a plan can be built.
    let shown = json_of(&run(&scratch, &["--json", "history", "show", &id]));
    assert_eq!(shown["schemaVersion"], 1);
    assert_eq!(shown["restorability"]["restorable"], "FROM_SNAPSHOT");
    assert_eq!(shown["standing"], "CONFIRMED");
    assert_eq!(shown["snapshot"], "PRESENT");
    assert_eq!(shown["snapshotChecked"], "PARSED");
    // The stored record is the history.v2 document, and states no restorability.
    assert_eq!(shown["record"]["schemaVersion"], 2);
    assert!(shown["record"].get("restorability").is_none());

    // The list reports the same, from the file's details alone.
    let listed = json_of(&run(&scratch, &["--json", "history"]));
    assert_eq!(listed["runs"][0]["restorable"], "FROM_SNAPSHOT");
    assert_eq!(listed["runs"][0]["snapshotChecked"], "METADATA");

    // The marker still names it: not confirmed, never offered for restore.
    let state = scratch.state();
    let lock = acquire(&state, euid()).expect("the run lock");
    let marker = InterruptedRun::new(&id)
        .and_then(|m| m.with_snapshot(&snapshot))
        .expect("a marker");
    run_marker::write(&lock, &state, &marker, euid()).expect("the marker");
    drop(lock);
    let shown = json_of(&run(&scratch, &["--json", "history", "show", &id]));
    assert_eq!(shown["standing"], "UNCONFIRMED");
    assert_eq!(shown["restorability"]["restorable"], "NOT");
    assert_eq!(shown["restorability"]["reason"], "RECORD_NOT_CONFIRMED");
    let text = run(&scratch, &["history"]);
    assert!(String::from_utf8_lossy(&text.stdout).contains("[record not confirmed]"));
}

#[test]
fn a_missing_foreign_or_replaced_snapshot_is_never_present() {
    let scratch = Scratch::new("snapshot-states");
    let stamp = "20261003T120000Z";
    let snapshot = seed_snapshot(&scratch, stamp);
    seed_record(&scratch, &mutating_run(stamp, &snapshot));
    let id = id_of(stamp);
    let file = scratch.state().snapshots().join(&snapshot);
    let reason = |scratch: &Scratch| {
        let shown = json_of(&run(scratch, &["--json", "history", "show", &id]));
        (
            shown["snapshot"].as_str().unwrap().to_owned(),
            shown["restorability"]["reason"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
        )
    };

    // Another run's snapshot under this run's name: readable, but not tied.
    let mut other = load_snapshot(&root().join(SNAPSHOT_FIXTURE)).unwrap();
    other.provenance.captured_at = started_at("20260101T000000Z");
    let text = macportsbrew_adapters::persistence::snapshot_to_json(&other).unwrap();
    std::fs::write(&file, text).unwrap();
    std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    assert_eq!(
        reason(&scratch),
        ("PRESENT".to_owned(), "SNAPSHOT_NOT_THIS_RUNS".to_owned())
    );

    // A symbolic reference in its place is not a snapshot, wherever it points.
    let target = scratch.0.join("elsewhere.json");
    std::fs::rename(&file, &target).unwrap();
    std::os::unix::fs::symlink(&target, &file).unwrap();
    assert_eq!(
        reason(&scratch),
        ("UNREADABLE".to_owned(), "SNAPSHOT_UNREADABLE".to_owned())
    );

    // Open to others: not what this tool writes.
    std::fs::remove_file(&file).unwrap();
    std::fs::rename(&target, &file).unwrap();
    std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
    assert_eq!(
        reason(&scratch),
        ("UNREADABLE".to_owned(), "SNAPSHOT_UNREADABLE".to_owned())
    );

    // Gone.
    std::fs::remove_file(&file).unwrap();
    assert_eq!(
        reason(&scratch),
        ("MISSING".to_owned(), "SNAPSHOT_MISSING".to_owned())
    );
}

#[test]
fn a_run_that_changed_nothing_has_nothing_to_restore_whatever_is_on_disk() {
    let scratch = Scratch::new("nothing");
    seed_record(&scratch, &refused_run("20261001T100000Z"));
    let shown = json_of(&run(
        &scratch,
        &["--json", "history", "show", &id_of("20261001T100000Z")],
    ));
    assert_eq!(shown["restorability"]["restorable"], "NOTHING_TO_RESTORE");
    assert_eq!(shown["snapshot"], "NOT_TAKEN");
    assert_eq!(shown["record"]["mutationDispatched"], false);
}

#[test]
fn the_human_text_names_the_run_its_steps_and_whether_it_can_be_put_back() {
    let scratch = Scratch::new("human");
    let stamp = "20261003T120000Z";
    let snapshot = seed_snapshot(&scratch, stamp);
    seed_record(&scratch, &mutating_run(stamp, &snapshot));
    let output = run(&scratch, &["history", "show", &id_of(stamp)]);
    assert_eq!(code(&output), 0);
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    for expected in [
        id_of(stamp).as_str(),
        "htop",
        "success",
        "a plan can be built from the snapshot",
        snapshot.as_str(),
        "SNAPSHOT_BEFORE: SNAPSHOT_TAKEN",
        "RECORD_HISTORY: RECORD_PENDING_READ_BACK",
        "confirmed (no interrupted-run marker names it)",
    ] {
        assert!(
            text.contains(expected),
            "{expected:?} missing from:\n{text}"
        );
    }
}
