// SPDX-License-Identifier: Apache-2.0

//! Reading and writing snapshots and recordings.
//!
//! A snapshot is the safety net (`04-migration-and-safety.md`, "Rollback and
//! snapshots"), the input to `analyze --from-snapshot` that proves INV-15, and
//! the basis of every fixture. So the bytes matter: a snapshot written and
//! read back and written again is byte-identical, because `serde_json` keeps
//! insertion order, every map in the domain is a `BTreeMap`, and the structs
//! serialise in declaration order.
//!
//! Everything read here is treated as untrusted input
//! (`04-migration-and-safety.md`, threat model): the size is bounded before
//! the file is read, a parse failure is `SnapshotUnusable` - or
//! `RecordingUnusable` on the `--replay` route, because the remedies differ
//! (F-41) - rather than a
//! panic, the schema version is checked, and the capture timestamp — which is
//! data from a file — is scrubbed to an alphanumeric stamp before it is
//! allowed anywhere near a file name.

use crate::persistence::atomic::{write_atomic, write_new_durable, PublishError};
use crate::persistence::bounded_read::{
    read_bounded as read_bounded_file, BoundedReadError, OpenPolicy,
};
use crate::persistence::history_store::private_directory;
use crate::persistence::state_dir::StateDir;
use crate::process::Recording;
use macportsbrew_domain::history::is_snapshot_file_name;
use macportsbrew_domain::{DomainError, ErrorId, Snapshot};
use std::fs::Metadata;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The largest snapshot or recording that will be read. A snapshot of a very
/// full machine is a few megabytes; anything approaching this is not one, and
/// an unbounded read is a threat-model item in its own right.
pub const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;

/// The longest stamp a file name will carry. A well-formed RFC 3339 instant
/// reduces to sixteen characters; anything near this is not one.
const MAX_STAMP_CHARS: usize = 32;

/// The file name a snapshot captured at `captured_at` is stored under:
/// `snapshot-<YYYYMMDDTHHMMSSZ>.json`, which sorts chronologically as text.
///
/// `captured_at` comes out of a file, so only its alphanumerics survive; a
/// timestamp is never allowed to become a path (`../`) or a flag (`-`).
///
/// A timestamp that reduces to nothing, or to more than [`MAX_STAMP_CHARS`],
/// is refused rather than defaulted or truncated. Both would make two
/// different captures share a name, and a snapshot silently overwritten by
/// another is the safety net failing exactly when it is needed.
pub fn snapshot_file_name(captured_at: &str) -> Result<String, DomainError> {
    let stamp: String = captured_at
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    if stamp.is_empty() || stamp.len() > MAX_STAMP_CHARS {
        return Err(DomainError::new(
            ErrorId::SnapshotUnusable,
            format!("the capture timestamp {captured_at:?} cannot name a file"),
        ));
    }
    Ok(format!("snapshot-{stamp}.json"))
}

/// Serialises a snapshot exactly as it is stored: pretty-printed, one trailing
/// newline.
pub fn snapshot_to_json(snapshot: &Snapshot) -> Result<String, DomainError> {
    let mut text = serde_json::to_string_pretty(snapshot).map_err(|e| {
        DomainError::new(
            ErrorId::Internal,
            format!("a snapshot could not be serialised: {e}"),
        )
    })?;
    text.push('\n');
    Ok(text)
}

/// Writes a snapshot into `directory` and returns the path it was written to.
///
/// The write is atomic and private (see [`write_atomic`]). The directory must
/// already exist; `StateDir::ensure` is what creates it, and doing that here
/// would make every caller's mistake silently succeed.
///
/// Refuses to replace an existing snapshot. The name is a function of the
/// capture time to the second, so two captures in the same second collide —
/// and `04-migration-and-safety.md` says automatic snapshots are never pruned
/// automatically, which a silent overwrite would violate while leaving
/// `history` reporting the run as restorable from a file that now describes
/// something else. The caller re-stamps and retries.
pub fn save_snapshot(directory: &Path, snapshot: &Snapshot) -> Result<PathBuf, DomainError> {
    let path = directory.join(snapshot_file_name(&snapshot.provenance.captured_at)?);
    if path.symlink_metadata().is_ok() {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            format!(
                "{} already exists; refusing to replace a snapshot",
                path.display()
            ),
        ));
    }
    write_atomic(&path, snapshot_to_json(snapshot)?.as_bytes())?;
    Ok(path)
}

/// Publishes `snapshot` in `directory` **durably** and without ever replacing a
/// file (pass 08 slice 5, D1): the entry reaches the medium before this returns
/// `Ok`, because a run changes the machine behind this file. Returns the file
/// name (never a path) the record and the marker name it by.
///
/// Not [`save_snapshot`]'s best-effort write, and without its
/// `symlink_metadata` look first: a look and then a rename is a window, and a
/// link that cannot replace is not.
///
/// # Errors
/// `SAFETY_BLOCKED` when a file of that name exists — two captures in one
/// second, or a `snapshot` command in the same second as a run's start — and
/// the underlying error when it cannot be published durably. Either way the
/// run changes nothing.
pub fn save_snapshot_durable(directory: &Path, snapshot: &Snapshot) -> Result<String, DomainError> {
    let name = snapshot_file_name(&snapshot.provenance.captured_at)?;
    match write_new_durable(
        &directory.join(&name),
        snapshot_to_json(snapshot)?.as_bytes(),
    ) {
        Ok(()) => Ok(name),
        Err(PublishError::Exists) => Err(DomainError::new(
            ErrorId::SafetyBlocked,
            format!(
                "{name} already exists; refusing to replace a snapshot, so nothing was changed"
            ),
        )),
        Err(PublishError::Failed(e)) => Err(e),
    }
}

/// What a snapshot file inside the state directory is, looked at without being
/// trusted (pass 08 slice 5, D8a).
#[derive(Clone, Debug)]
pub enum StateSnapshot {
    /// It is there, is what this tool writes and parses as a snapshot.
    Present(Box<Snapshot>),
    /// There is no such file.
    Missing,
    /// Something is there that this tool did not write, or that cannot be
    /// read as a snapshot. The text names the file and the kind of problem,
    /// never a path or a value from the file.
    Unreadable(String),
}

/// Whether a snapshot file is there, from its metadata alone (what `history`'s
/// list shows without parsing up to 64 MiB per row).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotFile {
    /// A regular file this user owns, closed to others, within the size cap.
    Present,
    /// There is no such file.
    Missing,
    /// Something is there that is not that.
    Unreadable(String),
}

/// The path of snapshot `name` in `state`, after the checks both lookups make:
/// `name` is a snapshot file name (never a path), the state root and
/// `snapshots/` are private to `euid` and are not symbolic references. `None`
/// when `snapshots/` or the file does not exist. Creates nothing.
fn state_snapshot_path(state: &StateDir, name: &str, euid: u32) -> Result<Option<PathBuf>, String> {
    if !is_snapshot_file_name(name) {
        return Err("the snapshot named is not a snapshot file name".to_string());
    }
    let directory = state.snapshots();
    let look = |path: &Path, what: &str| -> Result<Option<Metadata>, String> {
        match std::fs::symlink_metadata(path) {
            Ok(meta) => Ok(Some(meta)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{what} could not be inspected: {}", e.kind())),
        }
    };
    let Some(root_meta) = look(state.root(), "the state directory")? else {
        return Ok(None);
    };
    private_directory(&root_meta, euid).map_err(|why| format!("the state directory {why}"))?;
    let Some(dir_meta) = look(&directory, "the snapshots directory")? else {
        return Ok(None);
    };
    private_directory(&dir_meta, euid).map_err(|why| format!("the snapshots directory {why}"))?;
    let path = directory.join(name);
    Ok(look(&path, name)?.map(|_| path))
}

fn owner_only(meta: &Metadata, euid: u32) -> Result<(), &'static str> {
    if meta.uid() != euid {
        return Err("is owned by somebody else");
    }
    if meta.permissions().mode() & 0o077 != 0 {
        return Err("is readable or writable by others");
    }
    Ok(())
}

/// [`SnapshotFile`] for snapshot `name` in `state`.
#[must_use]
pub fn state_snapshot_file(state: &StateDir, name: &str, euid: u32) -> SnapshotFile {
    let path = match state_snapshot_path(state, name, euid) {
        Ok(Some(path)) => path,
        Ok(None) => return SnapshotFile::Missing,
        Err(why) => return SnapshotFile::Unreadable(why),
    };
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            SnapshotFile::Unreadable(format!("{name} is a symbolic reference"))
        }
        Ok(meta) if !meta.is_file() => {
            SnapshotFile::Unreadable(format!("{name} is not a regular file"))
        }
        Ok(meta) if meta.len() > MAX_DOCUMENT_BYTES => SnapshotFile::Unreadable(format!(
            "{name} is {} bytes, above the {MAX_DOCUMENT_BYTES}-byte limit",
            meta.len()
        )),
        Ok(meta) => match owner_only(&meta, euid) {
            Ok(()) => SnapshotFile::Present,
            Err(why) => SnapshotFile::Unreadable(format!("{name} {why}")),
        },
        Err(e) => SnapshotFile::Unreadable(format!("{name} could not be inspected: {}", e.kind())),
    }
}

/// Reads snapshot `name` out of the state directory, **trusting nothing**
/// (pass 08 slice 5, D8a): the name is a snapshot file name, the state root and
/// `snapshots/` are private, the file is a regular file opened without
/// following a symbolic reference, owned by `euid`, closed to group and other,
/// within [`MAX_DOCUMENT_BYTES`], and parses and passes the same checks as any
/// snapshot. Every message names the file and the kind of problem, never a path.
/// [`load_snapshot`] stays for a file the user named.
#[must_use]
pub fn state_snapshot(state: &StateDir, name: &str, euid: u32) -> StateSnapshot {
    let path = match state_snapshot_path(state, name, euid) {
        Ok(Some(path)) => path,
        Ok(None) => return StateSnapshot::Missing,
        Err(why) => return StateSnapshot::Unreadable(why),
    };
    let text = match read_bounded_file(
        &path,
        MAX_DOCUMENT_BYTES,
        OpenPolicy::RefuseSymlink,
        |meta| owner_only(meta, euid),
    ) {
        Ok(text) => text,
        Err(
            BoundedReadError::Open(e) | BoundedReadError::Inspect(e) | BoundedReadError::Read(e),
        ) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return StateSnapshot::Missing;
            }
            return StateSnapshot::Unreadable(format!("{name} could not be read: {}", e.kind()));
        }
        Err(BoundedReadError::NotAFile) => {
            return StateSnapshot::Unreadable(format!("{name} is not a regular file"))
        }
        Err(BoundedReadError::Rejected(why)) => {
            return StateSnapshot::Unreadable(format!("{name} {why}"))
        }
        Err(BoundedReadError::TooLarge { len }) => {
            return StateSnapshot::Unreadable(format!(
                "{name} is {len} bytes, above the {MAX_DOCUMENT_BYTES}-byte limit"
            ))
        }
        Err(BoundedReadError::Grew) => {
            return StateSnapshot::Unreadable(format!("{name} grew while it was read"))
        }
    };
    let mut snapshot: Snapshot = match serde_json::from_str(&text) {
        Ok(snapshot) => snapshot,
        Err(e) => {
            return StateSnapshot::Unreadable(format!(
                "{name} is not a snapshot this version reads ({:?} at line {}, column {})",
                e.classify(),
                e.line(),
                e.column()
            ))
        }
    };
    snapshot.mark_unrecorded_capabilities_unknown();
    if snapshot
        .check(&crate::registry::environment_contracts())
        .is_err()
    {
        return StateSnapshot::Unreadable(format!("{name} fails this version's snapshot checks"));
    }
    StateSnapshot::Present(Box::new(snapshot))
}

/// Reads a snapshot, checking its size, its schema version, and the invocation
/// environments it states.
///
/// Every failure is `SnapshotUnusable`, which is the "fails closed" of the
/// threat model: a snapshot that cannot be trusted produces no plan, rather
/// than a partial one.
///
/// The environment check runs here because this is where a snapshot enters the
/// program. It is *also* run on a captured snapshot, because "constructed"
/// turned out not to mean "safe": a recording that changes what `brew --prefix`
/// answers moves the other provider's prefix into a legitimately built
/// environment, which a review demonstrated. One rule, both routes. F-21.
pub fn load_snapshot(path: &Path) -> Result<Snapshot, DomainError> {
    let text = read_bounded(path, ErrorId::SnapshotUnusable)?;
    let mut snapshot: Snapshot = serde_json::from_str(&text).map_err(|e| {
        DomainError::new(
            ErrorId::SnapshotUnusable,
            format!("{} is not a readable snapshot: {e}", path.display()),
        )
    })?;
    // A capability this document does not mention is one this document cannot
    // answer, which is not the same thing as one nobody asked about - see
    // `Snapshot::mark_unrecorded_capabilities_unknown`. Here rather than inside
    // `check` because it is the only route where the distinction exists: every
    // producer of a `ProviderSnapshot` in this build answers all nine
    // capabilities - both adapters, installed or not, and `capture::broken`
    // for a provider whose read failed - so on the capture route there is
    // never anything to fill, and `check` is shared with it (F-29, DP-016).
    // That `broken` was writing an empty table, and so was a third shape the
    // rule below is documented as never meeting, was found by
    // `security-reviewer` on 2026-09-18 rather than by this comment.
    //
    // One consequence of filling *before* validating, worth knowing when
    // `check` grows: any capability validation added there is pre-satisfied by
    // this call and would be silently vacuous on the load route.
    //
    // Before `check`, so that what the checks see is what the rest of the
    // program will see.
    snapshot.mark_unrecorded_capabilities_unknown();
    snapshot.check(&crate::registry::environment_contracts())?;
    Ok(snapshot)
}

/// Reads a recording — a fixture — with the same bounds as a snapshot.
///
/// Same reader, same bounds, **different identifier**. The two routes share one
/// implementation (F-30) and that unification quietly preserved a message
/// written for only one of them: a bad `--replay` told the user to take a fresh
/// snapshot and point `--from-snapshot` at something else (F-41).
pub fn load_recording(path: &Path) -> Result<Recording, DomainError> {
    let text = read_bounded(path, ErrorId::RecordingUnusable)?;
    Recording::from_json(&text).map_err(|e| {
        DomainError::new(
            e.id,
            format!(
                "{} is not a readable recording: {}",
                path.display(),
                e.detail
            ),
        )
    })
}

/// Reads a file, bounded.
///
/// The name is resolved exactly once: the size and the "is this a regular
/// file" test are made against the open handle, and the read itself is capped
/// as well. Checking a path and then opening it again would leave the bound
/// applying to an object that need not be the one read — and an unbounded read
/// of a file that grew, or of a device or a FIFO, is the threat-model item
/// this cap exists for.
fn read_bounded(path: &Path, id: ErrorId) -> Result<String, DomainError> {
    use crate::persistence::bounded_read::{any_regular_file, BoundedReadError};
    // The identifier is the caller's, not this reader's. Every message below is
    // about *a file*, and which file it is decides what the user should do
    // about it - the distinction F-30's unification lost and F-41 found.
    let unusable = |detail: String| DomainError::new(id, detail);
    // The mechanism moved to `bounded_read` so the receipt reader could share it
    // (F-30); this wrapper exists to keep every message below exactly where it
    // was, because they are what a user sees when a snapshot will not load.
    crate::persistence::bounded_read::read_bounded(
        path,
        MAX_DOCUMENT_BYTES,
        // The user pointed at this file by name; a symlink is their
        // arrangement, not an attack. Blocking is a separate question and is
        // never chosen here - the reader never blocks, for anyone (F-31).
        crate::persistence::bounded_read::OpenPolicy::FollowAsNamed,
        any_regular_file,
    )
    .map_err(|e| match e {
        BoundedReadError::Open(e) | BoundedReadError::Read(e) => {
            unusable(format!("{} could not be read: {e}", path.display()))
        }
        BoundedReadError::Inspect(e) => {
            unusable(format!("{} could not be inspected: {e}", path.display()))
        }
        BoundedReadError::NotAFile => unusable(format!("{} is not a file", path.display())),
        BoundedReadError::TooLarge { len } => unusable(format!(
            "{} is {len} bytes, above the {MAX_DOCUMENT_BYTES}-byte limit",
            path.display()
        )),
        BoundedReadError::Grew => unusable(format!(
            "{} grew past the {MAX_DOCUMENT_BYTES}-byte limit while being read",
            path.display()
        )),
        // This caller passes `any_regular_file`, so nothing reaches here.
        // Mapped rather than panicked: a snapshot that will not load is a
        // diagnostic, never a crash.
        BoundedReadError::Rejected(reason) => {
            unusable(format!("{} was refused: {reason}", path.display()))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_name_is_a_sortable_stamp_and_never_a_path() {
        assert_eq!(
            snapshot_file_name("2026-09-02T13:31:48Z").unwrap(),
            "snapshot-20260902T133148Z.json"
        );
        // Ordering as text follows ordering in time.
        let mut names = [
            snapshot_file_name("2026-09-02T13:31:48Z").unwrap(),
            snapshot_file_name("2025-01-01T00:00:00Z").unwrap(),
            snapshot_file_name("2026-09-02T13:31:47Z").unwrap(),
        ];
        names.sort();
        assert_eq!(names[0], "snapshot-20250101T000000Z.json");
        assert_eq!(names[2], "snapshot-20260902T133148Z.json");

        // A hostile timestamp cannot escape the directory or grow a flag.
        let hostile = snapshot_file_name("../../etc/-rf 2026").unwrap();
        assert_eq!(hostile, "snapshot-etcrf2026.json");
        assert!(!hostile.contains('/'));
        assert_eq!(
            snapshot_file_name("///").unwrap_err().id,
            ErrorId::SnapshotUnusable
        );
        assert_eq!(
            snapshot_file_name("").unwrap_err().id,
            ErrorId::SnapshotUnusable
        );
    }

    #[test]
    fn reading_something_that_is_not_there_fails_closed_and_says_which_input() {
        // This test used to assert `SnapshotUnusable` for *both* loaders, which
        // is how F-41 stayed invisible: the defect was pinned as the expected
        // answer. Failing closed is still the property - neither loader returns
        // a partial document - and the identifier now says which of the two
        // inputs failed, because the remedies are different commands.
        let missing = Path::new("/nonexistent/macportsbrew/snapshot.json");
        assert_eq!(
            load_snapshot(missing).unwrap_err().id,
            ErrorId::SnapshotUnusable
        );
        assert_eq!(
            load_recording(missing).unwrap_err().id,
            ErrorId::RecordingUnusable
        );
        // A directory is not a document either, on either route.
        assert_eq!(
            load_snapshot(Path::new("/")).unwrap_err().id,
            ErrorId::SnapshotUnusable
        );
        assert_eq!(
            load_recording(Path::new("/")).unwrap_err().id,
            ErrorId::RecordingUnusable
        );
        // Same exit code either way: `06-cli-and-ux.md:68` is the category
        // "8 stale/unusable input", and a new identifier must not quietly move
        // one off it.
        assert_eq!(ErrorId::RecordingUnusable.exit_code(), 8);
        assert_eq!(ErrorId::SnapshotUnusable.exit_code(), 8);
    }
}
