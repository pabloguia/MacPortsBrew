// SPDX-License-Identifier: Apache-2.0

//! The persistence adapter against a real filesystem.
//!
//! These live outside `src/` deliberately. `04-migration-and-safety.md`
//! ("Threat model") requires that a write never follows a symbolic link, and
//! the only honest way to test that is to create one and watch the code refuse
//! it. The `Stop` gate forbids the symbolic-link creation call anywhere under
//! `crates/*/src`, precisely so that no shipped code path can create one; a
//! top-level integration test is where the adversarial case belongs, and
//! `std::os::unix::fs::symlink` appears here and nowhere else in the
//! workspace.

use macportsbrew_adapters::persistence::{
    load_recording, load_snapshot, save_snapshot, snapshot_file_name, snapshot_to_json,
    write_atomic, StateDir, FILE_MODE, STATE_DIR_MODE,
};
use macportsbrew_domain::{
    Architecture, CommandLineTools, EnvironmentSnapshot, ErrorId, Machine, MacosVersion,
    Provenance, Snapshot, Translation, SNAPSHOT_SCHEMA_VERSION,
};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A private working directory. No `tempfile` dependency is available and none
/// is being added for this.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "macportsbrew-it-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn snapshot(captured_at: &str) -> Snapshot {
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "0.1.0".into(),
        policy_version: "0".into(),
        provenance: Provenance {
            captured_at: captured_at.into(),
            captured_by: "crates/adapters/tests/persistence.rs".into(),
            origin: "synthesised".into(),
            note: Some("round-trip test".into()),
        },
        machine: Machine {
            architecture: Architecture::X86_64,
            macos: MacosVersion::new(26, 6, 2),
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Installed {
                path: PathBuf::from("/Applications/Xcode.app/Contents/Developer"),
            },
        },
        environment: EnvironmentSnapshot {
            path: vec!["~/bin".into(), "/usr/local/bin".into(), "/usr/bin".into()],
            reported: BTreeMap::from([("MANPATH".to_string(), "/opt/local/share/man".to_string())]),
            path_findings: Vec::new(),
        },
        providers: BTreeMap::new(),
    }
}

fn mode_of(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

// --- The state directory ------------------------------------------------

#[test]
fn ensure_creates_a_private_tree_and_is_idempotent() {
    let home = scratch("state-home");
    let dir = StateDir::resolve(Some(&home), None).unwrap();
    dir.ensure().unwrap();

    for directory in dir.directories() {
        assert!(
            directory.is_dir(),
            "{} is not a directory",
            directory.display()
        );
        assert_eq!(
            mode_of(&directory),
            STATE_DIR_MODE,
            "{} is not private",
            directory.display()
        );
    }
    assert!(dir
        .root()
        .ends_with("Library/Application Support/macportsbrew"));

    // INV-16: running it again changes nothing and reports so.
    write_atomic(&dir.snapshots().join("keep.json"), b"{}").unwrap();
    dir.ensure().unwrap();
    assert_eq!(
        std::fs::read(dir.snapshots().join("keep.json")).unwrap(),
        b"{}"
    );

    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn a_loose_state_directory_is_tightened() {
    let home = scratch("state-loose");
    let dir = StateDir::resolve(Some(&home), None).unwrap();
    dir.ensure().unwrap();
    std::fs::set_permissions(dir.root(), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::set_permissions(dir.cache(), std::fs::Permissions::from_mode(0o777)).unwrap();

    dir.ensure().unwrap();
    assert_eq!(mode_of(dir.root()), STATE_DIR_MODE);
    assert_eq!(mode_of(&dir.cache()), STATE_DIR_MODE);

    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn a_state_directory_that_is_a_symbolic_link_is_refused() {
    let base = scratch("state-symbolic");
    let elsewhere = base.join("elsewhere");
    let home = base.join("home");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::create_dir_all(home.join("Library/Application Support")).unwrap();

    let dir = StateDir::resolve(Some(&home), None).unwrap();
    std::os::unix::fs::symlink(&elsewhere, dir.root()).unwrap();

    let err = dir.ensure().unwrap_err();
    assert_eq!(err.id, ErrorId::SafetyBlocked);
    // Nothing was created behind it.
    assert!(!elsewhere.join("snapshots").exists());

    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_subdirectory_that_is_a_symbolic_link_is_refused_too() {
    let base = scratch("state-sub-symbolic");
    let elsewhere = base.join("elsewhere");
    let home = base.join("home");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let dir = StateDir::resolve(Some(&home), None).unwrap();
    dir.ensure().unwrap();
    std::fs::remove_dir(dir.snapshots()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, dir.snapshots()).unwrap();

    assert_eq!(dir.ensure().unwrap_err().id, ErrorId::SafetyBlocked);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_state_path_that_is_a_file_is_refused() {
    let base = scratch("state-file");
    let home = base.join("home");
    std::fs::create_dir_all(home.join("Library/Application Support")).unwrap();
    let dir = StateDir::resolve(Some(&home), None).unwrap();
    std::fs::write(dir.root(), b"not a directory").unwrap();

    assert_eq!(dir.ensure().unwrap_err().id, ErrorId::Io);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn an_override_is_created_where_it_says() {
    let base = scratch("state-override");
    let target = base.join("relocated");
    let dir = StateDir::resolve(None, Some(&target.to_string_lossy())).unwrap();
    dir.ensure().unwrap();
    assert_eq!(dir.root(), target);
    assert_eq!(mode_of(&dir.history()), STATE_DIR_MODE);
    std::fs::remove_dir_all(&base).ok();
}

// --- Atomic writes ------------------------------------------------------

#[test]
fn writing_through_a_symbolic_link_is_refused_and_the_target_is_untouched() {
    let dir = scratch("atomic-symbolic");
    let real = dir.join("real.json");
    let alias = dir.join("alias.json");
    std::fs::write(&real, b"original").unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();

    let err = write_atomic(&alias, b"replaced").unwrap_err();
    assert_eq!(err.id, ErrorId::SafetyBlocked);
    assert_eq!(std::fs::read(&real).unwrap(), b"original");
    // No partial file was left next to it.
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_dangling_symbolic_link_is_refused_as_well() {
    let dir = scratch("atomic-dangling");
    let alias = dir.join("alias.json");
    std::os::unix::fs::symlink(dir.join("nothing-here"), &alias).unwrap();

    assert_eq!(
        write_atomic(&alias, b"x").unwrap_err().id,
        ErrorId::SafetyBlocked
    );
    assert!(!dir.join("nothing-here").exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_published_file_is_private_and_complete() {
    let dir = scratch("atomic-publish");
    let target = dir.join("state.json");
    write_atomic(&target, b"first version, longer").unwrap();
    write_atomic(&target, b"second").unwrap();

    assert_eq!(std::fs::read(&target).unwrap(), b"second");
    assert_eq!(mode_of(&target), FILE_MODE);
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("state.json")]);

    std::fs::remove_dir_all(&dir).ok();
}

// --- Snapshots and recordings -------------------------------------------

#[test]
fn a_snapshot_round_trips_byte_identically() {
    let home = scratch("snapshot-round-trip");
    let dir = StateDir::resolve(Some(&home), None).unwrap();
    dir.ensure().unwrap();

    let original = snapshot("2026-09-02T13:31:48Z");
    let path = save_snapshot(&dir.snapshots(), &original).unwrap();
    assert_eq!(
        path.file_name().unwrap(),
        std::ffi::OsStr::new("snapshot-20260902T133148Z.json")
    );

    let written = std::fs::read(&path).unwrap();
    let read_back = load_snapshot(&path).unwrap();
    assert_eq!(read_back, original);

    // INV-15: the same snapshot produces the same bytes, every time. Written
    // somewhere else, because saving over the first one is refused.
    let elsewhere = dir.cache();
    let again = save_snapshot(&elsewhere, &read_back).unwrap();
    assert_eq!(again.file_name(), path.file_name());
    assert_eq!(std::fs::read(&again).unwrap(), written);
    assert_eq!(
        snapshot_to_json(&read_back).unwrap().as_bytes(),
        &written[..]
    );
    assert!(written.ends_with(b"\n"));
    // A third pass over the re-read value still lands on the same bytes.
    assert_eq!(
        snapshot_to_json(&load_snapshot(&again).unwrap()).unwrap(),
        snapshot_to_json(&original).unwrap()
    );

    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn two_captures_do_not_overwrite_each_other() {
    let dir = scratch("snapshot-distinct");
    save_snapshot(&dir, &snapshot("2026-09-02T13:31:48Z")).unwrap();
    save_snapshot(&dir, &snapshot("2026-09-02T13:31:49Z")).unwrap();
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);

    // Two captures in the same second share a name. The safety net refuses
    // rather than replacing the older one — 04 says automatic snapshots are
    // never pruned automatically, and a silent overwrite is pruning.
    let mut second = snapshot("2026-09-02T13:31:48Z");
    second.tool_version = "0.2.0".into();
    let err = save_snapshot(&dir, &second).unwrap_err();
    assert_eq!(err.id, ErrorId::SafetyBlocked);
    assert_eq!(
        load_snapshot(&dir.join("snapshot-20260902T133148Z.json"))
            .unwrap()
            .tool_version,
        "0.1.0",
        "the first capture must survive"
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_timestamp_that_cannot_name_one_file_uniquely_is_refused() {
    // The stamp is data read out of a file. Truncating a long one would let
    // two different captures share a name, which is the same failure as an
    // overwrite; refusing is the only safe answer.
    assert!(snapshot_file_name(&"9".repeat(33)).is_err());
    assert!(snapshot_file_name(&"9".repeat(32)).is_ok());
    assert!(snapshot_file_name("///").is_err());

    let dir = scratch("snapshot-unnameable");
    let mut unnameable = snapshot("////");
    unnameable.tool_version = "0.1.0".into();
    assert_eq!(
        save_snapshot(&dir, &unnameable).unwrap_err().id,
        ErrorId::SnapshotUnusable
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn writing_into_a_directory_reached_through_a_symbolic_link_still_lands_privately() {
    // The refusal is about the target of the write, not its parent: a user who
    // has relocated a directory behind a symbolic link keeps working. What
    // must hold is that the file arrives complete and at 0o600.
    let base = scratch("atomic-parent-symbolic");
    let real = base.join("real");
    let alias = base.join("alias");
    std::fs::create_dir_all(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();

    write_atomic(&alias.join("state.json"), b"content").unwrap();
    assert_eq!(std::fs::read(real.join("state.json")).unwrap(), b"content");
    assert_eq!(mode_of(&real.join("state.json")), FILE_MODE);
    assert_eq!(std::fs::read_dir(&real).unwrap().count(), 1);

    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn corrupt_state_fails_closed() {
    let dir = scratch("snapshot-corrupt");

    let garbage = dir.join("garbage.json");
    std::fs::write(&garbage, b"{ this is not json").unwrap();
    assert_eq!(
        load_snapshot(&garbage).unwrap_err().id,
        ErrorId::SnapshotUnusable
    );

    // A schema this build does not read is refused, not guessed at.
    let mut future = snapshot_to_json(&snapshot("2026-09-02T13:31:48Z")).unwrap();
    future = future.replace(
        &format!("\"schemaVersion\": {SNAPSHOT_SCHEMA_VERSION}"),
        "\"schemaVersion\": 99",
    );
    let ahead = dir.join("ahead.json");
    std::fs::write(&ahead, future.as_bytes()).unwrap();
    assert_eq!(
        load_snapshot(&ahead).unwrap_err().id,
        ErrorId::SnapshotUnusable
    );

    // The recording route fails closed the same way and names its own input.
    // This assertion read `SnapshotUnusable` until 2026-09-08, which is how
    // F-41 survived a review: the wrong identifier was pinned as correct, so
    // the message telling a `--replay` user to take a fresh snapshot had a
    // green test under it.
    assert_eq!(
        load_recording(&garbage).unwrap_err().id,
        ErrorId::RecordingUnusable
    );

    // Both failure modes, not just the parse: a schema the build does not read
    // is refused on this route too, and by the same identifier.
    //
    // The version is patched into a *real* fixture, the way the snapshot case
    // eight lines above does. The first version of this test wrote
    // `{"schemaVersion": 99, "invocations": []}` and asserted the same thing -
    // and passed for the wrong reason, because `Recording::provenance` is
    // required with no default, so that document dies at `serde_json::from_str`
    // and never reaches the version check. Two reviewers found it
    // independently. A test whose comment claims one arm and exercises another
    // is worse than no test: it is a green light over an uncovered line, which
    // is what this whole finding is about.
    let ahead_recording = dir.join("ahead-recording.json");
    let real = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/recordings/homebrew-only-intel-macos26.json"),
    )
    .unwrap();
    std::fs::write(
        &ahead_recording,
        real.replace("\"schemaVersion\": 1", "\"schemaVersion\": 99")
            .as_bytes(),
    )
    .unwrap();
    let err = load_recording(&ahead_recording).unwrap_err();
    assert_eq!(err.id, ErrorId::RecordingUnusable);
    assert!(
        err.detail.contains("schema 99"),
        "this must reach the version check, not the parse: {}",
        err.detail
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_captured_fixture_loads_as_a_recording() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings/homebrew-only-intel-macos26.json");
    let recording = load_recording(&path).unwrap();
    assert_eq!(recording.provenance.origin, "captured");
    assert_eq!(recording.provenance.architecture, "x86_64");
    assert!(!recording.invocations.is_empty());
}

#[test]
fn a_snapshot_is_stored_under_a_name_derived_from_its_capture_time() {
    assert_eq!(
        snapshot_file_name("2026-09-02T13:31:48Z").unwrap(),
        "snapshot-20260902T133148Z.json"
    );
    let dir = scratch("snapshot-naming");
    let path = save_snapshot(&dir, &snapshot("../escape")).unwrap();
    assert_eq!(path.parent().unwrap(), dir);
    assert_eq!(
        path.file_name().unwrap(),
        std::ffi::OsStr::new("snapshot-escape.json")
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_state_directory_somebody_else_owns_is_refused() {
    // The ownership check is against this process's real effective uid, read
    // from a probe file rather than from a `libc` call. `/` is owned by root
    // and is not writable, so it stands in for "a directory this user does not
    // own" without needing a second account.
    let dir = StateDir::resolve(None, Some("/macportsbrew-must-not-exist")).unwrap();
    let err = dir.ensure().unwrap_err();
    assert!(
        matches!(err.id, ErrorId::Io | ErrorId::SafetyBlocked),
        "unexpected {err:?}"
    );
    assert!(!Path::new("/macportsbrew-must-not-exist").exists());
}
