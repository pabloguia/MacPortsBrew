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
//! the file is read, a parse failure is `SnapshotUnusable` rather than a
//! panic, the schema version is checked, and the capture timestamp — which is
//! data from a file — is scrubbed to an alphanumeric stamp before it is
//! allowed anywhere near a file name.

use crate::persistence::atomic::write_atomic;
use crate::process::Recording;
use macportsbrew_domain::{DomainError, ErrorId, Snapshot};
use std::fs::File;
use std::io::Read;
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
    let text = read_bounded(path)?;
    let snapshot: Snapshot = serde_json::from_str(&text).map_err(|e| {
        DomainError::new(
            ErrorId::SnapshotUnusable,
            format!("{} is not a readable snapshot: {e}", path.display()),
        )
    })?;
    snapshot.check_schema()?;
    snapshot.check_invocation_environments(&crate::registry::environment_contracts())?;
    Ok(snapshot)
}

/// Reads a recording — a fixture — with the same bounds as a snapshot.
pub fn load_recording(path: &Path) -> Result<Recording, DomainError> {
    let text = read_bounded(path)?;
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
fn read_bounded(path: &Path) -> Result<String, DomainError> {
    let unusable = |detail: String| DomainError::new(ErrorId::SnapshotUnusable, detail);
    let file = File::open(path)
        .map_err(|e| unusable(format!("{} could not be read: {e}", path.display())))?;
    let meta = file
        .metadata()
        .map_err(|e| unusable(format!("{} could not be inspected: {e}", path.display())))?;
    if !meta.is_file() {
        return Err(unusable(format!("{} is not a file", path.display())));
    }
    if meta.len() > MAX_DOCUMENT_BYTES {
        return Err(unusable(format!(
            "{} is {} bytes, above the {MAX_DOCUMENT_BYTES}-byte limit",
            path.display(),
            meta.len()
        )));
    }
    let mut text = String::new();
    file.take(MAX_DOCUMENT_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| unusable(format!("{} could not be read: {e}", path.display())))?;
    if text.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(unusable(format!(
            "{} grew past the {MAX_DOCUMENT_BYTES}-byte limit while being read",
            path.display()
        )));
    }
    Ok(text)
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
    fn reading_something_that_is_not_there_fails_closed() {
        let missing = Path::new("/nonexistent/macportsbrew/snapshot.json");
        assert_eq!(
            load_snapshot(missing).unwrap_err().id,
            ErrorId::SnapshotUnusable
        );
        assert_eq!(
            load_recording(missing).unwrap_err().id,
            ErrorId::SnapshotUnusable
        );
        // A directory is not a document either.
        assert_eq!(
            load_snapshot(Path::new("/")).unwrap_err().id,
            ErrorId::SnapshotUnusable
        );
    }
}
