// SPDX-License-Identifier: Apache-2.0

//! The interrupted-run marker: `interrupted-run.json` in the state root names a
//! run that began changing packages and has not been accounted for
//! (`04-migration-and-safety.md`, Concurrency and locking; pass 08 slice 2,
//! `docs/passes/08-slice-2-plan.md`, `docs/schemas/run-marker.v1.md`).
//!
//! **Every function takes the [`RunLock`]**, so no marker is read for a refusal,
//! written or cleared by a run that does not hold the lock: code that touches
//! the marker without having taken it does not compile.
//!
//! **Written** immediately before the first provider change, published by a
//! link that can never replace one, and **durably** — the marker exists to
//! survive a power loss, so its directory entry must reach the medium before
//! anything is changed. **Cleared** by the run that wrote it, after its record
//! is published; only that run, only its own marker.
//!
//! **Read without being trusted.** A missing marker is `None` and reading
//! creates nothing. Anything present that is not exactly what [`write`] leaves —
//! a symbolic reference, group or other access, another owner, a different
//! size, an unknown field, a BOM, trailing data, a second spelling of the same
//! JSON — is `HISTORY_UNUSABLE`, never "no marker": reading a marker as absent
//! is how a run that did not finish gets a successor on top of it. `nlink` is
//! not tested, because a kill between the link and its removal leaves a
//! published marker at two links.
//!
//! **Where the delete is.** [`clear`] is the tool's only delete. It opens the
//! marker with `O_NOFOLLOW`, reads it, and compares the handle's device and
//! inode with the path immediately before it unlinks. The window between that
//! comparison and the unlink remains; it is a same-uid actor's, and under the
//! held lock the only other writer is this process. A user removing the file by
//! hand (the documented remedy until `restore` exists) is not a path of this
//! tool.

use super::atomic::{write_new_durable, PublishError};
use super::bounded_read::{open_flags, read_bounded, BoundedReadError, OpenPolicy};
use super::run_lock::RunLock;
use super::state_dir::StateDir;
use macportsbrew_domain::{DomainError, ErrorId, InterruptedRun, StoredInterruptedRun};
use std::fs::{Metadata, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// The marker's name inside the state root. Errors name it, never its absolute
/// path.
pub const MARKER_FILE: &str = "interrupted-run.json";

/// The most a marker may hold. It holds a version and a run id.
pub const MAX_MARKER_BYTES: u64 = 4096;

/// What [`clear`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearOutcome {
    /// The run's own marker was removed.
    Cleared,
    /// There was none: a hand removal does not turn a finished run into an
    /// error, and the caller says so.
    AlreadyAbsent,
    /// Another run's marker is there and was left alone. Under the held lock
    /// that is an anomaly, and the caller surfaces it rather than treating the
    /// clear as done.
    NotOurs,
}

fn unusable(detail: String) -> DomainError {
    DomainError::new(ErrorId::HistoryUnusable, detail)
}

/// `{MARKER_FILE} {what}: {kind}`: the kind of the I/O error, never its text,
/// which can hold a path.
/// `result`, or [`io_unusable`] naming `what`: one place for the failure, so the
/// call sites carry no branch of their own.
fn or_unusable<T>(result: std::io::Result<T>, what: &str) -> Result<T, DomainError> {
    result.map_err(|e| io_unusable(what, &e))
}

fn io_unusable(what: &str, e: &std::io::Error) -> DomainError {
    unusable(format!("{MARKER_FILE} {what}: {}", e.kind()))
}

/// The exact bytes [`write`] leaves for `run`: compact JSON and a newline.
fn canonical(run: &InterruptedRun) -> String {
    let mut text =
        serde_json::to_string(&run.to_stored()).expect("a version and a string always serialise");
    text.push('\n');
    text
}

/// A marker's text, or why it is not one this writer leaves. Pure: the grammar
/// is tested on text, the file checks on files.
fn parse(text: &str) -> Result<InterruptedRun, String> {
    let stored: StoredInterruptedRun = serde_json::from_str(text).map_err(|e| {
        // serde's message can quote the file's own values; only where the
        // problem is, and what kind it is, are kept.
        format!(
            "{MARKER_FILE} is not a marker this version reads ({:?} at line {}, column {})",
            e.classify(),
            e.line(),
            e.column()
        )
    })?;
    let run = InterruptedRun::from_stored(&stored).map_err(|e| format!("{MARKER_FILE}: {e}"))?;
    if text != canonical(&run) {
        return Err(format!(
            "{MARKER_FILE} is not exactly what this version writes"
        ));
    }
    Ok(run)
}

/// Why a marker's opened handle is not one this writer leaves.
fn accept(meta: &Metadata, euid: u32) -> Result<(), &'static str> {
    accept_facts(meta.uid(), meta.permissions().mode(), euid)
}

/// [`accept`] on the facts alone, so the owner rule is tested without a second
/// user to own a file.
fn accept_facts(uid: u32, mode: u32, euid: u32) -> Result<(), &'static str> {
    if uid != euid {
        return Err("is owned by somebody else");
    }
    if mode & 0o077 != 0 {
        return Err("is readable or writable by others");
    }
    Ok(())
}

/// The marker a previous run left, if any. Creates nothing.
pub fn read(
    _lock: &RunLock,
    state: &StateDir,
    euid: u32,
) -> Result<Option<InterruptedRun>, DomainError> {
    let path = state.root().join(MARKER_FILE);
    match std::fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_unusable("could not be inspected", &e)),
    }
    let text = match read_bounded(&path, MAX_MARKER_BYTES, OpenPolicy::RefuseSymlink, |meta| {
        accept(meta, euid)
    }) {
        Ok(text) => text,
        // Gone between the look and the open: a hand removal, so absent.
        Err(BoundedReadError::Open(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None)
        }
        Err(e) => {
            return Err(unusable(match e {
                BoundedReadError::Open(e)
                | BoundedReadError::Inspect(e)
                | BoundedReadError::Read(e) => {
                    format!("{MARKER_FILE} could not be read: {}", e.kind())
                }
                BoundedReadError::NotAFile => format!("{MARKER_FILE} is not a regular file"),
                BoundedReadError::Rejected(why) => format!("{MARKER_FILE} {why}"),
                BoundedReadError::TooLarge { len } => {
                    format!("{MARKER_FILE} is {len} bytes, above the {MAX_MARKER_BYTES}-byte limit")
                }
                BoundedReadError::Grew => format!("{MARKER_FILE} grew while it was read"),
            }))
        }
    };
    parse(&text).map(Some).map_err(unusable)
}

/// Writes the marker for `run`, durably and without replacing anything. Called
/// once, immediately before the first provider change.
///
/// A marker already there is an earlier run's: [`ErrorId::RunInterrupted`]
/// naming it, or `HISTORY_UNUSABLE` if it cannot be read. A failure to publish
/// durably is `SAFETY_BLOCKED`: nothing was changed, and the caller dispatches
/// nothing.
pub fn write(
    lock: &RunLock,
    state: &StateDir,
    run: &InterruptedRun,
    euid: u32,
) -> Result<(), DomainError> {
    write_with(lock, state, run, euid, write_new_durable)
}

/// [`write`] with the publishing call passed in, so a test can make it publish
/// and then report that the directory could not be synced. Private: no
/// production caller supplies another.
fn write_with(
    lock: &RunLock,
    state: &StateDir,
    run: &InterruptedRun,
    euid: u32,
    publish: impl FnOnce(&Path, &[u8]) -> Result<(), PublishError>,
) -> Result<(), DomainError> {
    lock.verify_still_current()?;
    let path = state.root().join(MARKER_FILE);
    match publish(&path, canonical(run).as_bytes()) {
        Ok(()) => Ok(()),
        Err(PublishError::Exists) => match read(lock, state, euid)? {
            Some(earlier) => Err(DomainError::new(
                ErrorId::RunInterrupted,
                earlier.run_id().to_string(),
            )),
            None => Err(DomainError::new(
                ErrorId::SafetyBlocked,
                format!("{MARKER_FILE} appeared and went again while it was written, so nothing was changed"),
            )),
        },
        // A failed durable publish may have left the marker: it is published
        // before its directory is synced. This run changed nothing, so it takes
        // its own marker back — otherwise the next run would be refused for a
        // run that did nothing — and says so only if that worked.
        Err(PublishError::Failed(_)) => match clear(lock, state, run, euid) {
            Ok(ClearOutcome::Cleared | ClearOutcome::AlreadyAbsent) => Err(DomainError::new(
                ErrorId::SafetyBlocked,
                format!(
                    "{MARKER_FILE} could not be written durably, so nothing was changed. Check \
                     that the state directory has room and is on a local volume"
                ),
            )),
            Ok(ClearOutcome::NotOurs) | Err(_) => Err(DomainError::new(
                ErrorId::SafetyBlocked,
                format!(
                    "{MARKER_FILE} could not be written durably and could not be taken back: \
                     nothing was changed, but the file is still there and will refuse the next \
                     run until it is removed by hand. Check that the state directory has room \
                     and is on a local volume"
                ),
            )),
        },
    }
}

/// Removes `run`'s own marker, once its record is published (or at once when it
/// dispatched nothing). Never another run's, never one it cannot read.
pub fn clear(
    lock: &RunLock,
    state: &StateDir,
    run: &InterruptedRun,
    euid: u32,
) -> Result<ClearOutcome, DomainError> {
    clear_with(lock, state, run, euid, |_| {})
}

/// [`clear`] with a seam that runs between the last check and the unlink, so a
/// test can swap the file at that instant. Private: no production caller
/// supplies another.
fn clear_with(
    lock: &RunLock,
    state: &StateDir,
    run: &InterruptedRun,
    euid: u32,
    before_unlink: impl FnOnce(&Path),
) -> Result<ClearOutcome, DomainError> {
    lock.verify_still_current()?;
    let path = state.root().join(MARKER_FILE);
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(open_flags(OpenPolicy::RefuseSymlink))
        .open(&path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ClearOutcome::AlreadyAbsent)
        }
        Err(e) => return Err(io_unusable("could not be opened as a plain file", &e)),
    };
    let meta = or_unusable(file.metadata(), "could not be inspected")?;
    accept(&meta, euid).map_err(|why| unusable(format!("{MARKER_FILE} {why}")))?;
    let mut text = String::new();
    or_unusable(
        (&mut file)
            .take(MAX_MARKER_BYTES + 1)
            .read_to_string(&mut text),
        "could not be read",
    )?;
    // Nothing longer than the marker's canonical text parses, so the read being
    // bounded by `take` is the whole size rule; a device or a pipe reads as
    // something that is not that text.
    let found = parse(&text).map_err(unusable)?;
    if found.run_id() != run.run_id() {
        return Ok(ClearOutcome::NotOurs);
    }

    before_unlink(&path);
    match std::fs::symlink_metadata(&path) {
        Ok(on_disk) if on_disk.dev() == meta.dev() && on_disk.ino() == meta.ino() => {}
        Ok(_) => {
            return Err(unusable(format!(
                "{MARKER_FILE} was replaced while it was being cleared, and was left in place"
            )))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ClearOutcome::AlreadyAbsent)
        }
        Err(e) => {
            return Err(io_unusable(
                "could not be inspected before it was cleared",
                &e,
            ))
        }
    }
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(ClearOutcome::Cleared),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ClearOutcome::AlreadyAbsent),
        Err(e) => Err(io_unusable("could not be removed", &e)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::run_lock::acquire;
    use super::*;

    const ID: &str = "20261005T120000Z-00000000000000aa";

    fn run(id: &str) -> InterruptedRun {
        InterruptedRun::new(id).unwrap()
    }

    #[test]
    fn the_canonical_text_is_a_marker_and_nothing_else_is() {
        let good = canonical(&run(ID));
        assert_eq!(parse(&good).unwrap(), run(ID));
        for bad in [
            good.trim_end().to_string(),
            format!("{good}\n"),
            format!(" {good}"),
            format!("\u{feff}{good}"),
            good.replace('}', ",\"extra\":1}"),
            good.replace(ID, "../etc"),
            good.replace("\"schemaVersion\":1", "\"schemaVersion\":2"),
            format!("{}{}", good.trim_end(), "\n{}"),
            good.replace('{', "{\"runId\":\"20261005T120000Z-00000000000000aa\","),
            good.replace("00aa", "00a\u{0}"),
            String::new(),
            "{}".to_string(),
            "null".to_string(),
        ] {
            assert!(parse(&bad).is_err(), "{bad:?}");
        }
    }

    fn scratch(tag: &str) -> (std::path::PathBuf, StateDir, u32) {
        let base =
            std::env::temp_dir().join(format!("mpb-marker-unit-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&base).unwrap();
        let euid = std::fs::metadata(&base).unwrap().uid();
        let state = StateDir::resolve(None, Some(base.join("state").to_str().unwrap())).unwrap();
        (base, state, euid)
    }

    fn published_then_failed(path: &Path, bytes: &[u8]) -> Result<(), PublishError> {
        super::super::atomic::write_new_atomic(path, bytes).unwrap();
        Err(PublishError::Failed(DomainError::new(ErrorId::Io, "sync")))
    }

    #[test]
    fn a_marker_published_and_then_failed_is_taken_back_by_the_run_that_wrote_it() {
        let (base, state, euid) = scratch("taken-back");
        let lock = acquire(&state, euid).unwrap();
        let error = write_with(&lock, &state, &run(ID), euid, published_then_failed).unwrap_err();
        assert_eq!(error.id, ErrorId::SafetyBlocked);
        assert!(
            error.detail.contains("nothing was changed"),
            "{}",
            error.detail
        );
        assert!(!error.detail.contains("still there"), "{}", error.detail);
        assert!(!state.root().join(MARKER_FILE).exists());
        // The next run is not refused for a run that did nothing.
        assert!(write(&lock, &state, &run(ID), euid).is_ok());
        drop(lock);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_marker_that_cannot_be_taken_back_says_it_is_still_there() {
        let (base, state, euid) = scratch("stuck");
        let lock = acquire(&state, euid).unwrap();
        // Somebody else's marker is in the way when the failure is handled.
        let other = InterruptedRun::new("20261005T130000Z-00000000000000bb").unwrap();
        let swap = |path: &Path, _: &[u8]| {
            std::fs::write(path, canonical(&other)).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
            Err(PublishError::Failed(DomainError::new(ErrorId::Io, "sync")))
        };
        let error = write_with(&lock, &state, &run(ID), euid, swap).unwrap_err();
        assert_eq!(error.id, ErrorId::SafetyBlocked);
        assert!(error.detail.contains("still there"), "{}", error.detail);
        assert!(state.root().join(MARKER_FILE).exists());
        drop(lock);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_marker_that_cannot_be_inspected_is_not_reported_as_cleared() {
        let (base, state, euid) = scratch("uninspectable");
        let lock = acquire(&state, euid).unwrap();
        write(&lock, &state, &run(ID), euid).unwrap();
        let root = state.root().to_path_buf();
        let error = clear_with(&lock, &state, &run(ID), euid, |_| {
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o000)).unwrap();
        })
        .unwrap_err();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(error.id, ErrorId::HistoryUnusable);
        assert!(state.root().join(MARKER_FILE).exists());
        drop(lock);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_state_root_that_cannot_be_looked_into_makes_the_marker_unreadable_not_absent() {
        let (base, state, euid) = scratch("unlookable");
        let lock = acquire(&state, euid).unwrap();
        let root = state.root().to_path_buf();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o000)).unwrap();
        let error = read(&lock, &state, euid).unwrap_err();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(error.id, ErrorId::HistoryUnusable);
        drop(lock);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_marker_that_cannot_be_removed_is_reported_and_left() {
        let (base, state, euid) = scratch("unremovable");
        let lock = acquire(&state, euid).unwrap();
        write(&lock, &state, &run(ID), euid).unwrap();
        let root = state.root().to_path_buf();
        let error = clear_with(&lock, &state, &run(ID), euid, |_| {
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o500)).unwrap();
        })
        .unwrap_err();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(error.id, ErrorId::HistoryUnusable);
        assert!(
            error.detail.contains("could not be removed"),
            "{}",
            error.detail
        );
        assert!(state.root().join(MARKER_FILE).exists());
        drop(lock);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_marker_that_appeared_and_went_while_it_was_written_is_a_refusal_that_changed_nothing() {
        let (base, state, euid) = scratch("came-and-went");
        let lock = acquire(&state, euid).unwrap();
        let error = write_with(&lock, &state, &run(ID), euid, |_, _| {
            Err(PublishError::Exists)
        })
        .unwrap_err();
        assert_eq!(error.id, ErrorId::SafetyBlocked);
        assert!(
            error.detail.contains("nothing was changed"),
            "{}",
            error.detail
        );
        assert!(!state.root().join(MARKER_FILE).exists());
        drop(lock);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_marker_removed_by_hand_while_it_is_being_cleared_is_already_absent() {
        let (base, state, euid) = scratch("hand-removed");
        let lock = acquire(&state, euid).unwrap();
        write(&lock, &state, &run(ID), euid).unwrap();
        let outcome = clear_with(&lock, &state, &run(ID), euid, |path| {
            std::fs::remove_file(path).unwrap();
        })
        .unwrap();
        assert_eq!(outcome, ClearOutcome::AlreadyAbsent);
        drop(lock);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_failed_call_is_unusable_by_name_and_a_good_one_passes_through() {
        assert_eq!(or_unusable(Ok(7), "x").unwrap(), 7);
        let failed: std::io::Result<u8> = Err(std::io::ErrorKind::NotFound.into());
        let error = or_unusable(failed, "could not be read").unwrap_err();
        assert_eq!(error.id, ErrorId::HistoryUnusable);
        assert!(error
            .detail
            .ends_with("could not be read: entity not found"));
    }

    #[test]
    fn an_io_failure_is_named_by_its_kind_and_never_its_text() {
        let denied = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "/Users/somebody/Library/secret",
        );
        let error = io_unusable("could not be read", &denied);
        assert_eq!(error.id, ErrorId::HistoryUnusable);
        assert_eq!(
            error.detail,
            "interrupted-run.json could not be read: permission denied"
        );
    }

    #[test]
    fn a_marker_owned_by_somebody_else_or_open_to_others_is_refused() {
        assert_eq!(accept_facts(501, 0o600, 501), Ok(()));
        assert_eq!(accept_facts(501, 0o400, 501), Ok(()));
        assert_eq!(
            accept_facts(0, 0o600, 501),
            Err("is owned by somebody else")
        );
        assert_eq!(
            accept_facts(501, 0o640, 501),
            Err("is readable or writable by others")
        );
        assert_eq!(
            accept_facts(501, 0o602, 501),
            Err("is readable or writable by others")
        );
    }

    #[test]
    fn a_swap_before_the_unlink_is_refused_and_the_swapped_file_stays() {
        let base = std::env::temp_dir().join(format!("mpb-marker-unit-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&base).unwrap();
        let euid = std::fs::metadata(&base).unwrap().uid();
        let state = StateDir::resolve(None, Some(base.join("state").to_str().unwrap())).unwrap();
        let lock = acquire(&state, euid).unwrap();
        write(&lock, &state, &run(ID), euid).unwrap();
        let error = clear_with(&lock, &state, &run(ID), euid, |path| {
            std::fs::remove_file(path).unwrap();
            std::fs::write(path, canonical(&run(ID))).unwrap();
        })
        .unwrap_err();
        assert_eq!(error.id, ErrorId::HistoryUnusable);
        assert!(state.root().join(MARKER_FILE).exists());
        drop(lock);
        std::fs::remove_dir_all(&base).ok();
    }
}
