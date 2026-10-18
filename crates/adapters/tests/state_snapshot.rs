// SPDX-License-Identifier: Apache-2.0

//! The snapshot a run takes first, inside the state directory (pass 08 slice 5,
//! D1 and D8a): published durably and never over an existing file, and read
//! back without being trusted. Every test runs in a scratch state directory
//! (INV-05).

use macportsbrew_adapters::persistence::run_lock::acquire;
use macportsbrew_adapters::persistence::snapshot_store::{
    save_snapshot_durable, state_snapshot, state_snapshot_file, SnapshotFile, StateSnapshot,
    MAX_DOCUMENT_BYTES,
};
use macportsbrew_adapters::persistence::{load_snapshot, snapshot_file_name, StateDir};
use macportsbrew_domain::{ErrorId, Snapshot};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Scratch {
    base: PathBuf,
    state: StateDir,
    uid: u32,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "mpb-snapshot-{tag}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&base).unwrap();
        let uid = std::fs::metadata(&base).unwrap().uid();
        let state = StateDir::resolve(None, Some(base.join("state").to_str().unwrap())).unwrap();
        state.ensure().unwrap();
        Self { base, state, uid }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // A test may have left a directory unreadable on purpose.
        std::fs::set_permissions(self.state.root(), std::fs::Permissions::from_mode(0o700)).ok();
        std::fs::remove_dir_all(&self.base).ok();
    }
}

/// A snapshot a real capture could have written: the checked-in live one.
fn snapshot() -> Snapshot {
    load_snapshot(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/snapshots/both-providers-intel-macos26-live.json"),
    )
    .expect("the fixture snapshot loads")
}

#[test]
fn a_snapshot_is_published_durably_and_read_back_as_the_one_written() {
    let s = Scratch::new("roundtrip");
    let taken = snapshot();
    let name = save_snapshot_durable(&s.state.snapshots(), &taken).unwrap();
    assert_eq!(
        name,
        snapshot_file_name(&taken.provenance.captured_at).unwrap()
    );
    let mode = std::fs::metadata(s.state.snapshots().join(&name)).unwrap();
    assert_eq!(
        mode.permissions().mode() & 0o777,
        0o600,
        "private to this user"
    );
    assert_eq!(
        state_snapshot_file(&s.state, &name, s.uid),
        SnapshotFile::Present
    );
    match state_snapshot(&s.state, &name, s.uid) {
        StateSnapshot::Present(back) => {
            assert_eq!(back.provenance.captured_at, taken.provenance.captured_at);
            assert_eq!(back.providers.len(), taken.providers.len());
        }
        other => panic!("expected the snapshot back, got {other:?}"),
    }
}

#[test]
fn a_snapshot_is_never_replaced() {
    let s = Scratch::new("never-replaced");
    let taken = snapshot();
    let name = save_snapshot_durable(&s.state.snapshots(), &taken).unwrap();
    let path = s.state.snapshots().join(&name);
    let before = std::fs::read(&path).unwrap();
    // Two captures in one second, or a `snapshot` command in the same second as
    // a run's start: the run refuses, and the first file is untouched.
    let error = save_snapshot_durable(&s.state.snapshots(), &taken).unwrap_err();
    assert_eq!(error.id, ErrorId::SafetyBlocked);
    assert!(
        error.detail.contains("nothing was changed"),
        "{}",
        error.detail
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn a_snapshot_that_is_not_there_is_missing_and_nothing_is_created() {
    let s = Scratch::new("missing");
    let name = "snapshot-20261017T030000Z.json";
    assert!(matches!(
        state_snapshot(&s.state, name, s.uid),
        StateSnapshot::Missing
    ));
    assert_eq!(
        state_snapshot_file(&s.state, name, s.uid),
        SnapshotFile::Missing
    );
    // Nor does a state directory that does not exist at all get created.
    let elsewhere = StateDir::resolve(None, Some(s.base.join("never").to_str().unwrap())).unwrap();
    assert!(matches!(
        state_snapshot(&elsewhere, name, s.uid),
        StateSnapshot::Missing
    ));
    assert!(!elsewhere.root().exists());
    assert!(
        std::fs::read_dir(s.state.snapshots())
            .unwrap()
            .next()
            .is_none(),
        "reading wrote nothing"
    );
}

#[test]
fn only_a_snapshot_file_name_is_ever_turned_into_a_path() {
    let s = Scratch::new("names");
    for bad in [
        "",
        "../snapshot-1.json",
        "/etc/passwd",
        "snapshot-1.json/",
        "snapshots/snapshot-1.json",
        "snapshot-x.y.json",
    ] {
        assert!(
            matches!(
                state_snapshot(&s.state, bad, s.uid),
                StateSnapshot::Unreadable(_)
            ),
            "{bad:?}"
        );
        assert!(
            matches!(
                state_snapshot_file(&s.state, bad, s.uid),
                SnapshotFile::Unreadable(_)
            ),
            "{bad:?}"
        );
    }
}

#[test]
fn something_this_tool_did_not_write_is_unreadable_never_present() {
    let s = Scratch::new("planted");
    let good = snapshot();
    let name = save_snapshot_durable(&s.state.snapshots(), &good).unwrap();
    let path = s.state.snapshots().join(&name);
    let text = std::fs::read_to_string(&path).unwrap();

    // Open to others.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(
        state_snapshot(&s.state, &name, s.uid),
        StateSnapshot::Unreadable(_)
    ));
    assert!(matches!(
        state_snapshot_file(&s.state, &name, s.uid),
        SnapshotFile::Unreadable(_)
    ));
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

    // Garbage that is not a snapshot.
    std::fs::write(&path, "not json").unwrap();
    assert!(matches!(
        state_snapshot(&s.state, &name, s.uid),
        StateSnapshot::Unreadable(_)
    ));

    // A symbolic reference to a perfectly good snapshot.
    std::fs::remove_file(&path).unwrap();
    let elsewhere = s.base.join("elsewhere.json");
    std::fs::write(&elsewhere, &text).unwrap();
    std::fs::set_permissions(&elsewhere, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &path).unwrap();
    assert!(matches!(
        state_snapshot(&s.state, &name, s.uid),
        StateSnapshot::Unreadable(_)
    ));
    assert!(matches!(
        state_snapshot_file(&s.state, &name, s.uid),
        SnapshotFile::Unreadable(_)
    ));
    std::fs::remove_file(&path).unwrap();

    // Another user's file cannot be made here, so the rule is exercised through
    // a different effective uid: nothing is owned by it.
    std::fs::write(&path, &text).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(matches!(
        state_snapshot(&s.state, &name, s.uid.wrapping_add(1)),
        StateSnapshot::Unreadable(_)
    ));

    // Too large to be one.
    let big = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    big.set_len(MAX_DOCUMENT_BYTES + 1).unwrap();
    assert!(matches!(
        state_snapshot_file(&s.state, &name, s.uid),
        SnapshotFile::Unreadable(_)
    ));
    assert!(matches!(
        state_snapshot(&s.state, &name, s.uid),
        StateSnapshot::Unreadable(_)
    ));
}

#[test]
fn a_snapshot_that_fails_the_checks_any_snapshot_passes_is_unreadable() {
    // The environment a provider states must be one this tool builds
    // (`Snapshot::check`): a file that was edited to say otherwise is refused.
    let s = Scratch::new("checks");
    let mut tampered = snapshot();
    for provider in tampered.providers.values_mut() {
        provider
            .invocation_environment
            .insert("SUDO_ASKPASS".into(), "/tmp/x".into());
    }
    let name = save_snapshot_durable(&s.state.snapshots(), &tampered).unwrap();
    assert!(matches!(
        state_snapshot(&s.state, &name, s.uid),
        StateSnapshot::Unreadable(_)
    ));
}

#[test]
fn a_state_directory_or_snapshots_directory_others_control_is_unreadable_not_empty() {
    let s = Scratch::new("directories");
    let name = save_snapshot_durable(&s.state.snapshots(), &snapshot()).unwrap();
    // snapshots/ open to others.
    std::fs::set_permissions(s.state.snapshots(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        state_snapshot(&s.state, &name, s.uid),
        StateSnapshot::Unreadable(_)
    ));
    assert!(matches!(
        state_snapshot_file(&s.state, &name, s.uid),
        SnapshotFile::Unreadable(_)
    ));
    std::fs::set_permissions(s.state.snapshots(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        state_snapshot_file(&s.state, &name, s.uid),
        SnapshotFile::Present
    );
    // The state root open to others.
    std::fs::set_permissions(s.state.root(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        state_snapshot(&s.state, &name, s.uid),
        StateSnapshot::Unreadable(_)
    ));
    std::fs::set_permissions(s.state.root(), std::fs::Permissions::from_mode(0o700)).unwrap();
    // snapshots/ as a symbolic reference to a good directory.
    let real = s.base.join("real-snapshots");
    std::fs::rename(s.state.snapshots(), &real).unwrap();
    std::os::unix::fs::symlink(&real, s.state.snapshots()).unwrap();
    assert!(matches!(
        state_snapshot(&s.state, &name, s.uid),
        StateSnapshot::Unreadable(_)
    ));
}

#[test]
fn an_error_names_the_file_and_never_a_path() {
    let s = Scratch::new("no-paths");
    let name = save_snapshot_durable(&s.state.snapshots(), &snapshot()).unwrap();
    std::fs::write(s.state.snapshots().join(&name), "not json").unwrap();
    std::fs::set_permissions(
        s.state.snapshots().join(&name),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let StateSnapshot::Unreadable(why) = state_snapshot(&s.state, &name, s.uid) else {
        panic!("expected unreadable");
    };
    assert!(why.contains(&name), "{why}");
    assert!(!why.contains(s.base.to_str().unwrap()), "{why}");
    // The lock is not needed to look.
    let _lock = acquire(&s.state, s.uid).unwrap();
    assert!(matches!(
        state_snapshot(&s.state, &name, s.uid),
        StateSnapshot::Unreadable(_)
    ));
}
