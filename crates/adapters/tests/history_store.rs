// SPDX-License-Identifier: Apache-2.0

//! The history store (`persistence::history_store`, pass 08 slice 1): writing
//! never replaces, reading creates nothing, and anything this writer would not
//! have left is `HISTORY_UNUSABLE` rather than "no record". Every test runs in
//! a scratch state directory (INV-05).

use macportsbrew_adapters::persistence::history_store::{
    new_run_id, probe, read_view, record_file_name, write_record, MAX_ENTRIES, MAX_RECORD_BYTES,
};
use macportsbrew_adapters::persistence::StateDir;
use macportsbrew_domain::history::is_run_id;
use macportsbrew_domain::{
    Direction, ErrorId, MigrationKey, Outcome, PackageIdentity, ProviderId, RecordedStep,
    RecordedVersions, RunEnding, RunRecord, RunStart, RunStatus, RunSubject, StepKind, StepResult,
    StoredRunRecord, Version,
};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// A state directory under the system temporary directory, not yet created.
fn scratch(tag: &str) -> (PathBuf, StateDir) {
    let base = std::env::temp_dir().join(format!(
        "mpb-history-{tag}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::remove_dir_all(&base).ok();
    std::fs::create_dir_all(&base).unwrap();
    let root = base.join("state");
    let state = StateDir::resolve(None, Some(root.to_str().unwrap())).unwrap();
    (base, state)
}

fn uid_of(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().uid()
}

/// This process's uid, as the owner of the scratch base it just made.
fn euid(base: &Path) -> u32 {
    uid_of(base)
}

const SOURCE: &str = "alpha";
const DESTINATION: &str = "beta";

fn key(package: &str) -> MigrationKey {
    MigrationKey {
        package: PackageIdentity::new(ProviderId::new(SOURCE), package),
        destination: ProviderId::new(DESTINATION),
        destination_package: Some(package.into()),
    }
}

fn verified() -> StepResult {
    StepResult::Verified {
        evidence: Vec::new(),
    }
}

fn record(id: &str, package: &str, ending: RunEnding) -> RunRecord {
    let stamp = &id[..16];
    let started_at = format!(
        "{}-{}-{}T{}:{}:{}Z",
        &stamp[0..4],
        &stamp[4..6],
        &stamp[6..8],
        &stamp[9..11],
        &stamp[11..13],
        &stamp[13..15]
    );
    let destination = ProviderId::new(DESTINATION);
    let steps = [
        StepKind::SnapshotBefore,
        StepKind::AddDestination,
        StepKind::VerifyDestination,
        StepKind::RecordHistory,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, kind)| {
        // The two brackets state their own results (history schema 2).
        let result = match kind {
            StepKind::SnapshotBefore => StepResult::SnapshotTaken,
            StepKind::RecordHistory => StepResult::RecordPendingReadBack,
            _ => verified(),
        };
        RecordedStep::new(
            u32::try_from(i + 1).unwrap(),
            kind,
            Some(destination.clone()),
            result,
            false,
        )
    })
    .collect();
    RunRecord::new(
        RunStart {
            id: id.into(),
            started_at,
            tool_version: "0.1.0".into(),
        },
        RunSubject {
            plan_schema_version: 1,
            policy_version: "2026.10.03-1".into(),
            package: PackageIdentity::new(ProviderId::new(SOURCE), package),
            direction: Direction {
                source: ProviderId::new(SOURCE),
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
            overrode: None,
            retain_source: true,
            primary_after: destination,
        },
        steps,
        ending,
        Some("snapshot-20261004T120000Z.json".into()),
    )
}

/// A way of spoiling what a test just wrote.
type Spoil<'a, T> = &'a dyn Fn(&T);

const OLD: &str = "20261004T110000Z-0000000000000001";
const NEW: &str = "20261004T120000Z-0000000000000002";

fn file(state: &StateDir, id: &str) -> PathBuf {
    state.history().join(record_file_name(id).unwrap())
}

#[test]
fn a_written_record_is_read_back_as_itself() {
    let (base, state) = scratch("roundtrip");
    let uid = euid(&base);
    let written = record(NEW, "tool", RunEnding::Completed);
    assert_eq!(written.status(), RunStatus::Success);
    write_record(&state, &written, uid).unwrap();

    let meta = std::fs::metadata(file(&state, NEW)).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    let view = read_view(&state, &key("tool"), uid).unwrap();
    assert_eq!(view.latest_for_key(), &[written]);
    assert_eq!(view.newest_stamp(), Some("20261004T120000Z"));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn reading_a_history_that_is_not_there_creates_nothing() {
    let (base, state) = scratch("absent");
    let view = read_view(&state, &key("tool"), euid(&base)).unwrap();
    assert!(view.latest_for_key().is_empty());
    assert_eq!(view.newest_stamp(), None);
    assert!(
        !state.root().exists(),
        "reading created the state directory"
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_record_is_never_replaced() {
    let (base, state) = scratch("replace");
    let uid = euid(&base);
    write_record(&state, &record(NEW, "tool", RunEnding::Completed), uid).unwrap();
    let before = std::fs::read(file(&state, NEW)).unwrap();
    let other = record(
        NEW,
        "tool",
        RunEnding::Halted {
            step: 2,
            error: ErrorId::SafetyBlocked,
        },
    );
    assert_eq!(
        write_record(&state, &other, uid).unwrap_err().id,
        ErrorId::Io
    );
    assert_eq!(std::fs::read(file(&state, NEW)).unwrap(), before);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn the_newest_record_for_the_key_is_what_is_returned() {
    let (base, state) = scratch("newest");
    let uid = euid(&base);
    write_record(&state, &record(OLD, "tool", RunEnding::Completed), uid).unwrap();
    let failed = record(
        NEW,
        "tool",
        RunEnding::Halted {
            step: 2,
            error: ErrorId::SafetyBlocked,
        },
    );
    write_record(&state, &failed, uid).unwrap();
    let view = read_view(&state, &key("tool"), uid).unwrap();
    assert_eq!(view.latest_for_key(), &[failed]);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn another_keys_newer_record_sets_the_newest_stamp_and_is_not_returned() {
    let (base, state) = scratch("other-key");
    let uid = euid(&base);
    let mine = record(OLD, "tool", RunEnding::Completed);
    write_record(&state, &mine, uid).unwrap();
    write_record(&state, &record(NEW, "other", RunEnding::Completed), uid).unwrap();
    let view = read_view(&state, &key("tool"), uid).unwrap();
    assert_eq!(view.latest_for_key(), &[mine]);
    assert_eq!(view.newest_stamp(), Some("20261004T120000Z"));
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn two_records_for_the_key_in_one_second_are_both_returned() {
    let (base, state) = scratch("tie");
    let uid = euid(&base);
    let a = record(
        "20261004T120000Z-000000000000000a",
        "tool",
        RunEnding::Completed,
    );
    let b = record(
        "20261004T120000Z-000000000000000b",
        "tool",
        RunEnding::Halted {
            step: 2,
            error: ErrorId::SafetyBlocked,
        },
    );
    write_record(&state, &a, uid).unwrap();
    write_record(&state, &b, uid).unwrap();
    let view = read_view(&state, &key("tool"), uid).unwrap();
    assert_eq!(
        view.latest_for_key().len(),
        2,
        "a tie is reported, not ordered"
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn reading_stops_at_the_keys_latest_record_and_fails_closed_before_it() {
    let (base, state) = scratch("stop");
    let uid = euid(&base);
    write_record(&state, &record(NEW, "tool", RunEnding::Completed), uid).unwrap();
    // An unreadable record older than the key's latest is never reached.
    std::fs::write(file(&state, OLD), b"not json").unwrap();
    std::fs::set_permissions(file(&state, OLD), std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(read_view(&state, &key("tool"), uid).is_ok());
    // A newer one is reached first, and could be a later run for the key.
    let newer = "20261004T130000Z-0000000000000003";
    std::fs::copy(file(&state, OLD), file(&state, newer)).unwrap();
    assert_eq!(
        read_view(&state, &key("tool"), uid).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_dot_entry_is_ignored_and_any_other_stranger_is_unusable() {
    let (base, state) = scratch("strangers");
    let uid = euid(&base);
    write_record(&state, &record(NEW, "tool", RunEnding::Completed), uid).unwrap();
    std::fs::write(state.history().join(".run-x.json.1.partial"), b"half").unwrap();
    assert!(read_view(&state, &key("tool"), uid).is_ok());
    for stranger in ["notes.txt", "run-1.json", "run-20261004T120000Z-ABC.json"] {
        let path = state.history().join(stranger);
        std::fs::write(&path, b"{}").unwrap();
        assert_eq!(
            read_view(&state, &key("tool"), uid).unwrap_err().id,
            ErrorId::HistoryUnusable,
            "{stranger}"
        );
        std::fs::remove_file(&path).unwrap();
    }
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_record_that_is_not_exactly_what_this_writer_leaves_is_unusable() {
    let cases: [(&str, Spoil<Path>); 6] = [
        ("a symbolic reference", &|p| {
            let target = p.with_extension("real");
            std::fs::rename(p, &target).unwrap();
            std::os::unix::fs::symlink(&target, p).unwrap();
        }),
        ("readable by others", &|p| {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o644)).unwrap();
        }),
        ("a directory", &|p| {
            std::fs::remove_file(p).unwrap();
            std::fs::create_dir(p).unwrap();
        }),
        ("oversized", &|p| {
            let mut text = std::fs::read_to_string(p).unwrap();
            text.push_str(&" ".repeat(usize::try_from(MAX_RECORD_BYTES).unwrap()));
            std::fs::write(p, text).unwrap();
        }),
        ("tampered", &|p| {
            let text = std::fs::read_to_string(p).unwrap();
            std::fs::write(p, text.replace("\"SUCCESS\"", "\"PARTIAL\"")).unwrap();
        }),
        ("not JSON", &|p| std::fs::write(p, b"{").unwrap()),
    ];
    for (what, spoil) in cases {
        let (base, state) = scratch("spoiled");
        let uid = euid(&base);
        write_record(&state, &record(NEW, "tool", RunEnding::Completed), uid).unwrap();
        spoil(&file(&state, NEW));
        assert_eq!(
            read_view(&state, &key("tool"), uid).unwrap_err().id,
            ErrorId::HistoryUnusable,
            "{what}"
        );
        std::fs::remove_dir_all(&base).ok();
    }
}

#[test]
fn a_record_under_another_runs_name_is_unusable() {
    let (base, state) = scratch("renamed");
    let uid = euid(&base);
    write_record(&state, &record(OLD, "tool", RunEnding::Completed), uid).unwrap();
    std::fs::rename(file(&state, OLD), file(&state, NEW)).unwrap();
    assert_eq!(
        read_view(&state, &key("tool"), uid).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_history_directory_that_is_not_private_or_not_a_directory_is_unusable() {
    let spoil: [(&str, Spoil<StateDir>); 3] = [
        ("open to others", &|s| {
            std::fs::set_permissions(s.history(), std::fs::Permissions::from_mode(0o755)).unwrap();
        }),
        ("a symbolic reference", &|s| {
            let elsewhere = s.root().join("elsewhere");
            std::fs::rename(s.history(), &elsewhere).unwrap();
            std::os::unix::fs::symlink(&elsewhere, s.history()).unwrap();
        }),
        ("a file", &|s| {
            std::fs::remove_dir_all(s.history()).unwrap();
            std::fs::write(s.history(), b"").unwrap();
        }),
    ];
    for (what, spoil) in spoil {
        let (base, state) = scratch("dir");
        let uid = euid(&base);
        write_record(&state, &record(NEW, "tool", RunEnding::Completed), uid).unwrap();
        spoil(&state);
        assert_eq!(
            read_view(&state, &key("tool"), uid).unwrap_err().id,
            ErrorId::HistoryUnusable,
            "{what}"
        );
        std::fs::remove_dir_all(&base).ok();
    }
}

#[test]
fn a_state_root_open_to_others_makes_the_history_unusable() {
    let (base, state) = scratch("root-open");
    let uid = euid(&base);
    write_record(&state, &record(NEW, "tool", RunEnding::Completed), uid).unwrap();
    std::fs::set_permissions(state.root(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        read_view(&state, &key("tool"), uid).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn records_owned_by_another_uid_are_unusable_and_root_is_refused() {
    let (base, state) = scratch("uid");
    let uid = euid(&base);
    write_record(&state, &record(NEW, "tool", RunEnding::Completed), uid).unwrap();
    // An existing tree with no record is still not somebody else's to read.
    let (empty_base, empty) = scratch("uid-empty");
    empty.ensure().unwrap();
    assert_eq!(
        read_view(&empty, &key("tool"), uid + 1).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    std::fs::remove_dir_all(&empty_base).ok();
    // Read as somebody else: the tree is not theirs.
    assert_eq!(
        read_view(&state, &key("tool"), uid + 1).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    // Root neither reads nor writes, whatever is there.
    assert_eq!(
        read_view(&state, &key("tool"), 0).unwrap_err().id,
        ErrorId::SafetyBlocked
    );
    let other = record(OLD, "tool", RunEnding::Completed);
    assert_eq!(
        write_record(&state, &other, 0).unwrap_err().id,
        ErrorId::SafetyBlocked
    );
    assert!(!file(&state, OLD).exists());
    assert_eq!(probe(&state, 0).unwrap_err().id, ErrorId::SafetyBlocked);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn too_many_entries_is_unusable() {
    let (base, state) = scratch("cap");
    let uid = euid(&base);
    state.ensure().unwrap();
    for i in 0..=MAX_ENTRIES {
        std::fs::write(state.history().join(format!(".filler-{i}")), b"").unwrap();
    }
    assert_eq!(
        read_view(&state, &key("tool"), uid).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn the_probe_publishes_nothing_and_refuses_where_it_cannot_publish() {
    let (base, state) = scratch("probe");
    let uid = euid(&base);
    probe(&state, uid).unwrap();
    assert_eq!(std::fs::read_dir(state.history()).unwrap().count(), 0);

    std::fs::remove_dir_all(state.history()).unwrap();
    std::fs::write(state.history(), b"").unwrap();
    assert_eq!(probe(&state, uid).unwrap_err().id, ErrorId::SafetyBlocked);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_new_run_id_is_a_run_id_and_a_typed_id_is_checked_before_it_is_a_path() {
    let id = new_run_id("20261004T120000Z");
    assert!(is_run_id(&id), "{id}");
    assert_ne!(new_run_id("20261004T120000Z"), id);
    assert_eq!(
        record_file_name(NEW).as_deref(),
        Some("run-20261004T120000Z-0000000000000002.json")
    );
    for typed in [
        "../../x",
        "20261004T120000Z-0000000000000002/..",
        "",
        "20261004T120000Z-00000000000000AB",
    ] {
        assert_eq!(record_file_name(typed), None, "{typed}");
    }
}

#[test]
fn the_documented_sample_is_a_record_this_version_reads() {
    let doc = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schemas/history.v2.md"),
    )
    .unwrap();
    let marker = "<!-- history-sample -->";
    let start = doc.find(marker).expect("the document marks its sample");
    let block = &doc[start + marker.len()..];
    let block = &block[block.find("```json").unwrap() + "```json".len()..];
    let json = &block[..block.find("```").unwrap()];
    let stored: StoredRunRecord = serde_json::from_str(json).expect("the sample parses");
    let back = RunRecord::from_stored(stored).expect("the sample is a usable record");
    // And it is exactly what this version writes for that record.
    let rewritten = serde_json::to_string_pretty(&back.stored()).unwrap();
    assert_eq!(rewritten.trim(), json.trim());
}

#[test]
fn a_new_write_never_writes_through_a_symbolic_reference() {
    use macportsbrew_adapters::persistence::{write_new_atomic, PublishError};
    let (base, _) = scratch("publish-through-reference");
    let elsewhere = base.join("elsewhere");
    let target = base.join("run.json");
    std::os::unix::fs::symlink(&elsewhere, &target).unwrap();
    assert!(matches!(
        write_new_atomic(&target, b"ours"),
        Err(PublishError::Exists)
    ));
    assert!(!elsewhere.exists(), "the reference was not followed");
    std::fs::remove_dir_all(&base).ok();
}

/// A kill between the publishing link and the temporary's removal leaves the
/// record at two links beside a hidden dot-temporary. The store must still read
/// it: `nlink` is deliberately not tested, and a dot-entry is never a stranger
/// (SE-092, owed by slice 1).
#[test]
fn a_crash_after_the_link_leaves_a_readable_store() {
    let (base, state) = scratch("crash-after-link");
    let uid = euid(&base);
    let written = record(NEW, "tool", RunEnding::Completed);
    write_record(&state, &written, uid).unwrap();
    let leftover = state.history().join(format!(
        ".{}.4242.00000000deadbeef.partial",
        record_file_name(NEW).unwrap()
    ));
    std::fs::hard_link(file(&state, NEW), &leftover).unwrap();
    assert_eq!(std::fs::metadata(&leftover).unwrap().nlink(), 2);

    let view = read_view(&state, &key("tool"), uid).unwrap();
    assert_eq!(view.latest_for_key(), &[written]);
    // The next run still publishes beside it.
    write_record(&state, &record(OLD, "tool", RunEnding::Completed), uid).unwrap();
    assert!(read_view(&state, &key("tool"), uid).is_ok());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_probe_name_left_by_a_killed_run_is_a_refusal_that_changed_nothing() {
    let (base, state) = scratch("probe-collision");
    let uid = euid(&base);
    probe(&state, uid).unwrap();
    let taken = state
        .history()
        .join(format!(".probe-{}", std::process::id()));
    std::fs::write(&taken, b"left behind").unwrap();
    let error = probe(&state, uid).unwrap_err();
    assert_eq!(error.id, ErrorId::SafetyBlocked);
    assert!(error.detail.contains("probe name"), "{}", error.detail);
    // Not ours, so not removed.
    assert_eq!(std::fs::read(&taken).unwrap(), b"left behind");
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_state_root_that_is_a_file_cannot_be_made_a_state_directory() {
    let (base, state) = scratch("root-is-a-file");
    std::fs::write(state.root(), b"a file").unwrap();
    let error = state.ensure().unwrap_err();
    assert_eq!(error.id, ErrorId::Io);
    assert_eq!(std::fs::read(state.root()).unwrap(), b"a file");
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn the_record_is_written_durably_not_best_effort() {
    // A text tripwire, because no test reaches the difference: the durable and
    // the best-effort write part only when the directory's sync fails after
    // the link, and that seam is private to `atomic.rs`. The mutation proof
    // of slice 4 found swapping them survived every behavioural test.
    let source = include_str!("../src/persistence/history_store.rs");
    let start = source
        .find("pub fn write_record(")
        .expect("write_record is defined");
    let body = &source[start..];
    let end = body.find("\n}\n").expect("write_record ends");
    let body = &body[..end];
    assert!(body.contains("write_new_durable("), "{body}");
    assert!(!body.contains("write_new_atomic("), "{body}");
}

// --- One record by id, the read-back, and the marker in the view (slice 5) ---

#[test]
fn one_record_by_id_is_returned_or_none_and_nothing_is_created() {
    use macportsbrew_adapters::persistence::history_store::read_one;
    let (base, state) = scratch("read-one");
    let uid = euid(&base);
    // No state directory at all: no record, and still none afterwards.
    assert_eq!(read_one(&state, NEW, uid).unwrap(), None);
    assert!(!state.root().exists());

    let written = record(NEW, "tool", RunEnding::Completed);
    write_record(&state, &written, uid).unwrap();
    assert_eq!(read_one(&state, NEW, uid).unwrap(), Some(written));
    assert_eq!(
        read_one(&state, OLD, uid).unwrap(),
        None,
        "a valid id with no record"
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn an_id_that_is_not_a_run_id_never_becomes_a_path_and_root_is_refused() {
    use macportsbrew_adapters::persistence::history_store::read_one;
    let (base, state) = scratch("read-one-ids");
    let uid = euid(&base);
    write_record(&state, &record(NEW, "tool", RunEnding::Completed), uid).unwrap();
    for bad in [
        "",
        "..",
        "../state/history/run-x",
        "run-1",
        "20261004T120000Z-ABC",
    ] {
        assert_eq!(
            read_one(&state, bad, uid).unwrap_err().id,
            ErrorId::HistoryUnusable,
            "{bad:?}"
        );
    }
    assert_eq!(
        read_one(&state, NEW, 0).unwrap_err().id,
        ErrorId::SafetyBlocked
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_single_record_read_holds_the_directories_to_the_same_rules_as_the_listing() {
    use macportsbrew_adapters::persistence::history_store::read_one;
    let (base, state) = scratch("read-one-dirs");
    let uid = euid(&base);
    write_record(&state, &record(NEW, "tool", RunEnding::Completed), uid).unwrap();
    for (what, directory) in [
        ("history", state.history()),
        ("state", state.root().to_path_buf()),
    ] {
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            read_one(&state, NEW, uid).unwrap_err().id,
            ErrorId::HistoryUnusable,
            "{what} open to others"
        );
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    assert!(read_one(&state, NEW, uid).unwrap().is_some());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn the_read_back_is_the_text_that_was_written_and_a_different_record_is_not() {
    use macportsbrew_adapters::persistence::history_store::{read_back, record_text};
    let (base, state) = scratch("read-back");
    let uid = euid(&base);
    let written = record(NEW, "tool", RunEnding::Completed);
    write_record(&state, &written, uid).unwrap();
    let text = record_text(&written).unwrap();
    assert!(text.ends_with("}\n"));
    assert_eq!(read_back(&state, NEW, uid).unwrap(), text);

    // Another record at the same id (the file replaced behind the run's back):
    // it reads, it parses, and it is not what was written.
    let other = record(NEW, "another-package", RunEnding::Completed);
    let path = file(&state, NEW);
    std::fs::write(&path, record_text(&other).unwrap()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let back = read_back(&state, NEW, uid).unwrap();
    assert_ne!(back, text);
    assert_eq!(back, record_text(&other).unwrap());

    // A record that is not there to read back is an error, never an empty text.
    assert_eq!(
        read_back(&state, OLD, uid).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn the_view_carries_what_the_marker_says_and_an_unreadable_marker_is_not_absent() {
    use macportsbrew_adapters::persistence::run_lock::acquire;
    use macportsbrew_adapters::persistence::run_marker::{write, MARKER_FILE};
    use macportsbrew_domain::{InterruptedRun, MarkerState, RecordStanding};
    let (base, state) = scratch("view-marker");
    let uid = euid(&base);
    let written = record(NEW, "tool", RunEnding::Completed);
    write_record(&state, &written, uid).unwrap();

    let view = read_view(&state, &key("tool"), uid).unwrap();
    assert_eq!(view.marker(), &MarkerState::Absent);
    assert_eq!(view.standing_of(&written), RecordStanding::Confirmed);

    let lock = acquire(&state, uid).unwrap();
    write(&lock, &state, &InterruptedRun::new(NEW).unwrap(), uid).unwrap();
    let view = read_view(&state, &key("tool"), uid).unwrap();
    assert_eq!(view.marker(), &MarkerState::Names(NEW.into()));
    assert_eq!(view.standing_of(&written), RecordStanding::Unconfirmed);

    // Something in the marker's place that is not a marker: nothing is known.
    let marker = state.root().join(MARKER_FILE);
    std::fs::remove_file(&marker).unwrap();
    std::fs::write(&marker, "not a marker").unwrap();
    std::fs::set_permissions(&marker, std::fs::Permissions::from_mode(0o600)).unwrap();
    let view = read_view(&state, &key("tool"), uid).unwrap();
    assert_eq!(view.marker(), &MarkerState::Unreadable);
    assert_eq!(view.standing_of(&written), RecordStanding::Unknown);
    drop(lock);
    std::fs::remove_dir_all(&base).ok();
}
