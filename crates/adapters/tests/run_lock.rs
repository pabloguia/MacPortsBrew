// SPDX-License-Identifier: Apache-2.0

//! The single-instance lock (`persistence::run_lock`, pass 08 slice 2): a held
//! lock is exit 7, a killed or finished holder frees it, and whatever sits at
//! the lock's path that this writer would not have left is `HISTORY_UNUSABLE`
//! rather than locked on. Every test runs in a scratch state directory (INV-05).

use macportsbrew_adapters::persistence::run_lock::{acquire, LOCK_FILE};
use macportsbrew_adapters::persistence::StateDir;
use macportsbrew_domain::ErrorId;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn scratch(tag: &str) -> (PathBuf, StateDir) {
    let base = std::env::temp_dir().join(format!(
        "mpb-lock-{tag}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::remove_dir_all(&base).ok();
    std::fs::create_dir_all(&base).unwrap();
    let root = base.join("state");
    let state = StateDir::resolve(None, Some(root.to_str().unwrap())).unwrap();
    state.ensure().unwrap();
    (base, state)
}

fn euid(base: &Path) -> u32 {
    std::fs::metadata(base).unwrap().uid()
}

fn lock_path(state: &StateDir) -> PathBuf {
    state.root().join(LOCK_FILE)
}

#[test]
fn a_second_run_is_refused_while_the_first_holds_the_lock_and_admitted_after() {
    let (base, state) = scratch("exclusive");
    let uid = euid(&base);
    let first = acquire(&state, uid).unwrap();
    let refused = acquire(&state, uid).unwrap_err();
    assert_eq!(refused.id, ErrorId::RunLockHeld);
    assert_eq!(refused.id.exit_code(), 7);
    // Taking it again while held changed nothing for the holder.
    assert!(first.verify_still_current().is_ok());
    drop(first);
    // No unlock was called: letting go of the handle is all a killed process
    // does, which is how a killed holder frees it (R-18 observed the kill).
    let second = acquire(&state, uid).unwrap();
    drop(second);
    // INV-16: a run after a finished run is admitted, again and again.
    drop(acquire(&state, uid).unwrap());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn the_lock_file_is_private_single_named_and_persists_after_release() {
    let (base, state) = scratch("persists");
    let uid = euid(&base);
    drop(acquire(&state, uid).unwrap());
    let meta = std::fs::metadata(lock_path(&state)).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    assert_eq!(meta.nlink(), 1);
    assert_eq!(meta.len(), 0);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn an_existing_lock_file_is_never_truncated() {
    let (base, state) = scratch("no-truncate");
    let uid = euid(&base);
    std::fs::write(lock_path(&state), b"keep").unwrap();
    std::fs::set_permissions(lock_path(&state), std::fs::Permissions::from_mode(0o600)).unwrap();
    drop(acquire(&state, uid).unwrap());
    assert_eq!(std::fs::read(lock_path(&state)).unwrap(), b"keep");
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_file_left_0400_by_an_odd_umask_is_repaired_and_locks() {
    let (base, state) = scratch("repair");
    let uid = euid(&base);
    std::fs::write(lock_path(&state), b"").unwrap();
    std::fs::set_permissions(lock_path(&state), std::fs::Permissions::from_mode(0o400)).unwrap();
    let held = acquire(&state, uid).unwrap();
    assert_eq!(
        std::fs::metadata(lock_path(&state))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(acquire(&state, uid).unwrap_err().id, ErrorId::RunLockHeld);
    drop(held);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn an_entry_open_to_others_or_with_two_names_is_refused_not_repaired() {
    let (base, state) = scratch("strangers");
    let uid = euid(&base);
    let path = lock_path(&state);
    std::fs::write(&path, b"").unwrap();
    for mode in [0o644, 0o660, 0o606] {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        let error = acquire(&state, uid).unwrap_err();
        assert_eq!(error.id, ErrorId::HistoryUnusable, "{mode:o}");
        // Not repaired by being looked at.
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            mode
        );
    }
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let other = state.root().join("second-name");
    std::fs::hard_link(&path, &other).unwrap();
    assert_eq!(
        acquire(&state, uid).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    std::fs::remove_file(&other).unwrap();
    assert!(acquire(&state, uid).is_ok());
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_directory_at_the_lock_path_is_refused() {
    let (base, state) = scratch("directory");
    std::fs::create_dir(lock_path(&state)).unwrap();
    assert_eq!(
        acquire(&state, euid(&base)).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_symbolic_reference_is_refused_and_its_target_is_never_created() {
    let (base, state) = scratch("symlink");
    let uid = euid(&base);
    let target = base.join("elsewhere");
    // Dangling: creating the lock file must not create the target.
    std::os::unix::fs::symlink(&target, lock_path(&state)).unwrap();
    assert_eq!(
        acquire(&state, uid).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    assert!(
        !target.exists(),
        "the reference was followed to create its target"
    );
    // To an existing file: not locked through it.
    std::fs::write(&target, b"theirs").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        acquire(&state, uid).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"theirs");
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn a_lock_file_removed_or_replaced_under_a_run_is_noticed() {
    let (base, state) = scratch("swapped");
    let uid = euid(&base);
    let held = acquire(&state, uid).unwrap();
    assert!(held.verify_still_current().is_ok());
    std::fs::remove_file(lock_path(&state)).unwrap();
    assert_eq!(
        held.verify_still_current().unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    // A new file at the same name is another object, and a second run could
    // lock it: the first run's check refuses.
    std::fs::write(lock_path(&state), b"").unwrap();
    assert_eq!(
        held.verify_still_current().unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    drop(held);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn invariant_06_the_run_lock_is_never_taken_as_root() {
    let (base, state) = scratch("root");
    std::fs::remove_file(lock_path(&state)).ok();
    let error = acquire(&state, 0).unwrap_err();
    assert_eq!(error.id, ErrorId::SafetyBlocked);
    assert!(!lock_path(&state).exists(), "nothing is created for root");
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn an_error_names_the_entry_and_never_the_users_directory() {
    let (base, state) = scratch("redaction");
    let uid = euid(&base);
    std::fs::write(lock_path(&state), b"").unwrap();
    std::fs::set_permissions(lock_path(&state), std::fs::Permissions::from_mode(0o644)).unwrap();
    let detail = acquire(&state, uid).unwrap_err().detail;
    assert!(detail.contains(LOCK_FILE), "{detail}");
    assert!(!detail.contains(base.to_str().unwrap()), "{detail}");
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn the_lock_open_passes_no_flag_that_could_clear_close_on_exec() {
    // `std` opens with O_CLOEXEC; `custom_flags` is the one way to take it
    // away, and a provider child holding the descriptor would keep the lock
    // for as long as it lives. The only flags passed are the shared helper's.
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/persistence/run_lock.rs"),
    )
    .unwrap();
    let code: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(code.contains("let flags = open_flags(OpenPolicy::RefuseSymlink);"));
    let uses = code.matches(".custom_flags(").count();
    assert_eq!(uses, code.matches(".custom_flags(flags)").count());
    assert!(uses >= 3, "the opens use the shared flags: {uses}");
}

#[test]
fn a_state_directory_that_cannot_be_made_whole_is_a_refusal_that_changed_nothing() {
    let (base, state) = scratch("not-whole");
    let uid = euid(&base);
    // A file where a directory of the state tree belongs: `ensure` refuses.
    std::fs::remove_dir_all(state.history()).unwrap();
    std::fs::write(state.history(), b"in the way").unwrap();
    let error = acquire(&state, uid).unwrap_err();
    assert_eq!(error.id, ErrorId::SafetyBlocked);
    assert!(
        !lock_path(&state).exists(),
        "no lock file for a refused run"
    );
    assert!(
        !error.detail.contains(base.to_str().unwrap()),
        "{}",
        error.detail
    );
    std::fs::remove_dir_all(&base).ok();
}
