// SPDX-License-Identifier: Apache-2.0

//! The single-instance lock: one mutating MacPortsBrew run at a time
//! (`04-migration-and-safety.md`, Concurrency and locking; pass 08 slice 2,
//! whose plan is `docs/passes/08-slice-2-plan.md`; R-17 and R-18).
//!
//! **What it is.** An exclusive advisory `flock` (`File::try_lock`, stable since
//! 1.89.0, `flock(2)` on Apple targets) on `run.lock` in the state root, held
//! for the life of the run. The kernel drops it when the holder dies, `SIGKILL`
//! included, so there is no stale lock and nothing to clean up. A second run
//! gets [`ErrorId::RunLockHeld`] (exit 7) at once; it never waits, because a
//! run that waits is a run that interleaves later.
//!
//! **What it is not.** Not the concurrent-provider detector: a MacPortsBrew
//! lock excludes nothing about `brew` or `port`, which take their own (R-17),
//! and it protects against no other uid. Where `flock` is host-local on a
//! volume shared between hosts it excludes nothing across them; only a lock that
//! does nothing at all is caught ([`lock_is_real`]).
//!
//! **The file is never unlinked, truncated or written.** An unlink while
//! another process opens the same path leaves two inodes that lock at once
//! (R-18, observed), so nothing here removes it and [`RunLock::verify_still_current`]
//! refuses a run whose lock file was removed or replaced under it. Deleting
//! `run.lock` during a run is unsupported.
//!
//! **The open sequence** is built for an adversary who can write the state
//! directory only as this user (it is `0700`), and for a umask that is not
//! `022` (R-18: under `0277` a created file is `0400`, and a read-write reopen
//! then fails). The file is created with `O_EXCL`, so a symbolic reference
//! planted where it will go is never followed to create its target; an existing
//! one is opened **read-only** (`flock` needs no write access) with
//! `O_NOFOLLOW | O_NONBLOCK`; every test is made on the handle; a mode that
//! gives group or other access is refused rather than repaired (it is somebody's
//! file, not a umask accident); and only then is `0600` set on the handle.
//!
//! **Close-on-exec.** `std` opens every file with `O_CLOEXEC` and ORs
//! `custom_flags` in after it, so the descriptor is not inherited by a spawned
//! child; the flags passed here are [`open_flags`]'s and a test pins that, as a
//! second line. It matters because a provider child that held the descriptor
//! would keep the lock for as long as it lives and every later mutating run
//! would exit 7 with nothing this tool could clear. (R-18 did not observe it.)
//!
//! **Never as root**, for the reason the history store gives: an elevated
//! process can keep the invoking user's home.

use super::atomic::FILE_MODE;
use super::bounded_read::{open_flags, OpenPolicy};
use super::state_dir::StateDir;
use macportsbrew_domain::{DomainError, ErrorId};
use std::fs::{File, Metadata, OpenOptions, TryLockError};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The lock file's name inside the state root. Errors name it, never its
/// absolute path: that holds the user's name.
pub const LOCK_FILE: &str = "run.lock";

/// A held lock. Dropping it releases it.
#[derive(Debug)]
pub struct RunLock {
    file: File,
    path: PathBuf,
}

/// `result`, or [`io_blocked`] naming `what`. One place for the failure, so the
/// call sites carry no branch of their own.
fn or_blocked<T>(result: std::io::Result<T>, what: &str) -> Result<T, DomainError> {
    result.map_err(|e| io_blocked(what, &e))
}

fn io_unusable(what: &str, e: &std::io::Error) -> DomainError {
    unusable(format!("{LOCK_FILE} {what}: {}", e.kind()))
}

fn unusable(detail: String) -> DomainError {
    DomainError::new(ErrorId::HistoryUnusable, detail)
}

fn blocked(detail: String) -> DomainError {
    DomainError::new(ErrorId::SafetyBlocked, detail)
}

/// `{LOCK_FILE} {what}: {kind}` as a refusal that changed nothing. The kind of
/// the I/O error, never its text: that can hold a path.
fn io_blocked(what: &str, e: &std::io::Error) -> DomainError {
    blocked(format!("{LOCK_FILE} {what}: {}", e.kind()))
}

/// Why an entry at the lock's path is not the file this writer leaves, from the
/// facts of its opened handle. Pure, so each reason is tested without a second
/// user to own a file.
fn entry_problem(
    is_file: bool,
    uid: u32,
    nlink: u64,
    mode: u32,
    euid: u32,
) -> Option<&'static str> {
    if !is_file {
        return Some("is not a regular file");
    }
    if uid != euid {
        return Some("is owned by somebody else");
    }
    if mode & 0o077 != 0 {
        return Some("is readable or writable by others");
    }
    if nlink != 1 {
        return Some("has more than one name");
    }
    None
}

fn refuse_entry(meta: &Metadata, euid: u32) -> Result<(), DomainError> {
    match entry_problem(
        meta.is_file(),
        meta.uid(),
        meta.nlink(),
        meta.permissions().mode(),
        euid,
    ) {
        None => Ok(()),
        Some(why) => Err(unusable(format!("{LOCK_FILE} {why}"))),
    }
}

/// Whether a second, separate open of the lock file was refused: the only
/// evidence that `flock` does anything on this volume. `Ok` means it was
/// granted, so locking is a no-op here and the lock excludes nothing.
fn lock_is_real(second: &Result<(), TryLockError>) -> bool {
    matches!(second, Err(TryLockError::WouldBlock))
}

fn same_object(handle: &Metadata, path: &Path) -> Result<(), DomainError> {
    match std::fs::symlink_metadata(path) {
        Ok(on_disk) if on_disk.dev() == handle.dev() && on_disk.ino() == handle.ino() => Ok(()),
        _ => Err(unusable(format!(
            "{LOCK_FILE} was removed or replaced while the run held it"
        ))),
    }
}

fn refuse_root(euid: u32) -> Result<(), DomainError> {
    if euid == 0 {
        return Err(blocked(
            "the run lock is never taken as root: an elevated process can keep the \
             invoking user's home, so it would lock in a directory that user owns"
                .to_string(),
        ));
    }
    Ok(())
}

/// Takes the lock, or says why not. Nothing is changed on any failure.
///
/// - another MacPortsBrew run holds it: [`ErrorId::RunLockHeld`] (exit 7);
/// - what is at the path is not what this writer leaves, or was swapped while
///   it was taken: [`ErrorId::HistoryUnusable`] (exit 8);
/// - the lock cannot be established or does nothing on this volume:
///   [`ErrorId::SafetyBlocked`] (exit 4), because nothing was changed.
pub fn acquire(state: &StateDir, euid: u32) -> Result<RunLock, DomainError> {
    acquire_with(state, euid, |_| {}, File::try_lock)
}

/// [`acquire`] with two seams for the two checks that cannot otherwise be
/// reached on a local volume: `after_lock` runs once the lock is held (a swap
/// of the file at that instant), and `second_lock` is how the separate second
/// open is locked (a volume where locking does nothing grants it). Private: no
/// production caller supplies another.
fn acquire_with(
    state: &StateDir,
    euid: u32,
    after_lock: impl FnOnce(&Path),
    second_lock: impl FnOnce(&File) -> Result<(), TryLockError>,
) -> Result<RunLock, DomainError> {
    refuse_root(euid)?;
    // `ensure`'s own detail names an absolute path, so it is not quoted.
    state.ensure().map_err(|_| {
        blocked(
            "the state directory cannot hold the run lock, so nothing was changed: it is not \
             the private directory this version creates"
                .to_string(),
        )
    })?;
    let path = state.root().join(LOCK_FILE);
    let flags = open_flags(OpenPolicy::RefuseSymlink);

    let file = match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(FILE_MODE)
        .custom_flags(flags)
        .open(&path)
    {
        Ok(created) => created,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open(&path)
            .map_err(|e| {
                io_unusable(
                    "could not be opened as a plain file (a symbolic reference, or unreadable)",
                    &e,
                )
            })?,
        Err(e) => {
            return Err(io_blocked(
                "could not be created, so nothing was changed",
                &e,
            ))
        }
    };

    let meta = or_blocked(file.metadata(), "could not be inspected")?;
    refuse_entry(&meta, euid)?;
    // The umask can leave a created file `0400`; the owner, mode and links are
    // already known good, so this only ever repairs.
    or_blocked(
        file.set_permissions(std::fs::Permissions::from_mode(FILE_MODE)),
        "could not be made private",
    )?;

    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return Err(DomainError::new(
                ErrorId::RunLockHeld,
                "the run lock is held by another run",
            ))
        }
        Err(TryLockError::Error(e)) => {
            return Err(io_blocked(
                "could not be locked, so nothing was changed",
                &e,
            ))
        }
    }

    after_lock(&path);
    // Held. The path must still be this object: a file unlinked or replaced
    // between the open and the lock would let a second run lock a second inode.
    same_object(&meta, &path)?;

    // Is the lock real here? A separate open of the same path must be refused.
    let probe = or_blocked(
        OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open(&path),
        "could not be opened a second time to check that locking works",
    )?;
    let second = second_lock(&probe);
    if second.is_ok() {
        probe.unlock().ok();
    }
    if !lock_is_real(&second) {
        return Err(blocked(
            "locking does nothing on the volume that holds the state directory, so a second \
             run could not be told apart from this one and nothing was changed. Point \
             MACPORTSBREW_STATE_DIR at a local volume"
                .to_string(),
        ));
    }
    drop(probe);

    Ok(RunLock { file, path })
}

impl RunLock {
    /// Re-checks that the lock file is still the object this run locked. Called
    /// when the marker is written, the last point before a provider change:
    /// deleting `run.lock` mid-run is unsupported and this is what notices.
    pub fn verify_still_current(&self) -> Result<(), DomainError> {
        let meta = or_blocked(self.file.metadata(), "could not be inspected")?;
        same_object(&meta, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EUID: u32 = 501;

    #[test]
    fn an_entry_that_is_not_what_the_writer_leaves_is_named_for_its_reason() {
        assert_eq!(entry_problem(true, EUID, 1, 0o600, EUID), None);
        assert_eq!(
            entry_problem(true, EUID, 1, 0o400, EUID),
            None,
            "0400 is repaired"
        );
        assert_eq!(
            entry_problem(false, EUID, 1, 0o600, EUID),
            Some("is not a regular file")
        );
        assert_eq!(
            entry_problem(true, 0, 1, 0o600, EUID),
            Some("is owned by somebody else")
        );
        for mode in [0o644, 0o660, 0o604, 0o666, 0o640, 0o602] {
            assert_eq!(
                entry_problem(true, EUID, 1, mode, EUID),
                Some("is readable or writable by others"),
                "{mode:o}"
            );
        }
        assert_eq!(
            entry_problem(true, EUID, 2, 0o600, EUID),
            Some("has more than one name")
        );
        assert_eq!(
            entry_problem(true, EUID, 0, 0o600, EUID),
            Some("has more than one name")
        );
    }

    #[test]
    fn an_io_failure_is_named_by_its_kind_and_never_its_text() {
        let denied = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "/Users/somebody/Library/secret",
        );
        let blocked = io_blocked("could not be inspected", &denied);
        assert_eq!(blocked.id, ErrorId::SafetyBlocked);
        assert_eq!(
            blocked.detail,
            "run.lock could not be inspected: permission denied"
        );
        let unusable = io_unusable("could not be opened", &denied);
        assert_eq!(unusable.id, ErrorId::HistoryUnusable);
        assert!(!unusable.detail.contains("somebody"), "{}", unusable.detail);
    }

    #[test]
    fn a_failed_call_is_a_refusal_that_names_what_failed_and_a_good_one_passes_through() {
        assert_eq!(or_blocked(Ok(7), "x").unwrap(), 7);
        let failed: std::io::Result<u8> = Err(std::io::ErrorKind::NotFound.into());
        assert_eq!(
            or_blocked(failed, "could not be inspected")
                .unwrap_err()
                .detail,
            "run.lock could not be inspected: entity not found"
        );
    }

    #[test]
    fn only_a_refused_second_open_shows_that_locking_works() {
        assert!(lock_is_real(&Err(TryLockError::WouldBlock)));
        assert!(!lock_is_real(&Ok(())));
        assert!(!lock_is_real(&Err(TryLockError::Error(
            std::io::Error::from(std::io::ErrorKind::Unsupported)
        ))));
    }

    fn scratch(tag: &str) -> (PathBuf, StateDir, u32) {
        let base = std::env::temp_dir().join(format!("mpb-lock-unit-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&base).unwrap();
        let uid = std::fs::metadata(&base).unwrap().uid();
        let state = StateDir::resolve(None, Some(base.join("state").to_str().unwrap())).unwrap();
        (base, state, uid)
    }

    #[test]
    fn a_lock_file_swapped_at_the_instant_the_lock_is_taken_is_refused() {
        let (base, state, uid) = scratch("swap");
        let error = acquire_with(
            &state,
            uid,
            |path| {
                std::fs::remove_file(path).unwrap();
                std::fs::write(path, b"").unwrap();
            },
            File::try_lock,
        )
        .unwrap_err();
        assert_eq!(error.id, ErrorId::HistoryUnusable);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_volume_where_locking_does_nothing_is_refused_and_says_where_to_look() {
        let (base, state, uid) = scratch("noop");
        let error = acquire_with(&state, uid, |_| {}, |_| Ok(())).unwrap_err();
        assert_eq!(error.id, ErrorId::SafetyBlocked);
        assert!(
            error.detail.contains("MACPORTSBREW_STATE_DIR"),
            "{}",
            error.detail
        );
        std::fs::remove_dir_all(&base).ok();
    }
}
