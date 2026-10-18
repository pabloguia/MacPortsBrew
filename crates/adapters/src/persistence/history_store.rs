// SPDX-License-Identifier: Apache-2.0

//! The history store: one file per mutating run, `history/run-<id>.json`
//! (`04-migration-and-safety.md`, History; pass 08 slice 1, whose plan is
//! `docs/passes/08-the-mutating-executor-plan.md`).
//!
//! **Append-only by construction.** A record is published with
//! [`write_new_atomic`], which links and never renames, so no record is ever
//! replaced; nothing here deletes one.
//!
//! **Reading creates nothing and trusts nothing.** A missing `history/` is a
//! history with no records. Everything else that is not exactly what this
//! writer leaves is `HISTORY_UNUSABLE` — never read as "no record", because a
//! record that cannot be read could be the later failure that withdraws a
//! recognition (D1). Every test of a record is made on its opened handle
//! (`bounded_read`, `RefuseSymlink`): a regular file, no group or other
//! access, owned by the effective uid. `nlink` is **not** tested: a kill
//! between the link and the unlink leaves a published record at two links,
//! and a link attack needs this user's own uid, which is outside the threat
//! model (security review, M2).
//!
//! **Never as root.** An elevated process can keep the invoking user's
//! `HOME` (whether it does depends on how it was elevated, so this is not
//! relied on either way), and then a root process would
//! read, and trust, records the user can write, and would leave root-owned
//! records in the user's home. Both directions refuse at euid 0.
//!
//! **Where it is called from.** Only a live run: `--replay` and
//! `--from-snapshot` never consult history, which the CLI's `HistoryScope`
//! decides once (INV-15).

use super::atomic::{write_new_atomic, write_new_durable, PublishError};
use super::bounded_read::{read_bounded, BoundedReadError, OpenPolicy};
use super::state_dir::StateDir;
use macportsbrew_domain::history::{is_run_id, run_id_stamp};
use macportsbrew_domain::{
    DomainError, ErrorId, HistoryView, MigrationKey, RunRecord, StoredRunRecord,
};
use std::fs::Metadata;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

/// The largest record read back. A record holds a plan's steps and their
/// evidence counts, never provider output, so this is generous.
pub const MAX_RECORD_BYTES: u64 = 256 * 1024;

/// The most entries `history/` may hold before reading refuses. Nothing
/// prunes the store, and every `migrate --retain-source` reads it.
pub const MAX_ENTRIES: usize = 10_000;

/// A fresh run id: the run's compact UTC stamp, then 16 hex digits from
/// `RandomState`, which `std` seeds from the operating system. It has to be
/// unique, not secret; [`write_record`] refuses a collision.
pub fn new_run_id(compact_stamp: &str) -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write(compact_stamp.as_bytes());
    format!("{compact_stamp}-{:016x}", hasher.finish())
}

/// The file name for run `id`, or `None` when `id` is not a run id — the one
/// check a user-typed id passes before it becomes part of a path.
pub fn record_file_name(id: &str) -> Option<String> {
    is_run_id(id).then(|| format!("run-{id}.json"))
}

fn unusable(detail: String) -> DomainError {
    DomainError::new(ErrorId::HistoryUnusable, detail)
}

fn refuse_root(euid: u32) -> Result<(), DomainError> {
    if euid == 0 {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            "the migration history is never read or written as root: an elevated \
             process can keep the invoking user's home, so it would trust records that user \
             can write and leave records that user cannot",
        ));
    }
    Ok(())
}

/// Proves the store can publish a record **before** anything is changed, so
/// a run never finds out after a mutation that it cannot record it (security
/// review, H1): creates the tree, then publishes and removes a probe exactly
/// as a record is published. A failure is `SafetyBlocked` — the caller
/// dispatches nothing.
pub fn probe(state: &StateDir, euid: u32) -> Result<(), DomainError> {
    refuse_root(euid)?;
    state.ensure().map_err(|e| {
        DomainError::new(
            ErrorId::SafetyBlocked,
            format!(
                "the state directory cannot hold a record, so nothing was changed: {}",
                e.detail
            ),
        )
    })?;
    let target = state
        .history()
        .join(format!(".probe-{}", std::process::id()));
    let published = write_new_atomic(&target, b"");
    match published {
        Ok(()) => {
            // Only a probe this call published is removed; a name taken
            // before it is not ours.
            std::fs::remove_file(&target).ok();
            Ok(())
        }
        Err(PublishError::Exists) => Err(DomainError::new(
            ErrorId::SafetyBlocked,
            "the history store's probe name was already taken, most likely a leftover \
             from a run that was killed; nothing was changed",
        )),
        Err(PublishError::Failed(e)) => Err(DomainError::new(
            ErrorId::SafetyBlocked,
            format!(
                "the history store cannot publish a record without replacing one, so \
                 nothing was changed: {}",
                e.detail
            ),
        )),
    }
}

/// Publishes `record` **durably**: the directory entry reaches the medium
/// before this returns `Ok` (pass 08 slice 4, D8). The run loop clears the
/// interrupted-run marker only after this returns `Ok`, and the marker and the
/// record live in different directories whose entries are not ordered with
/// each other, so a best-effort record followed by a durable marker removal
/// could leave neither after a power loss.
///
/// Never replaces: a record already at its id is refused, not overwritten.
/// The id is fixed before the run's first step, so it is not changed here to
/// dodge a collision.
///
/// # Errors
/// An error when the record is not known to be durable. **The record may
/// nonetheless exist**: when the directory could not be synced the entry is
/// published and is not removed, because the store is append-only. The caller
/// reports it as "not known to be written durably", keeps the marker, and
/// exits non-zero.
pub fn write_record(state: &StateDir, record: &RunRecord, euid: u32) -> Result<(), DomainError> {
    refuse_root(euid)?;
    let name = record_file_name(record.id())
        .ok_or_else(|| DomainError::new(ErrorId::Internal, "a run record's id is not a run id"))?;
    state.ensure()?;
    let text = record_text(record)?;
    match write_new_durable(&state.history().join(&name), text.as_bytes()) {
        Ok(()) => Ok(()),
        Err(PublishError::Exists) => Err(DomainError::new(
            ErrorId::Io,
            format!("a record named {name} already exists and was not replaced"),
        )),
        Err(PublishError::Failed(e)) => Err(e),
    }
}

/// The exact text a record is stored as: pretty-printed, one trailing newline.
/// The one serialisation, so what a writer wrote and what a read-back parsed
/// and wrote again can be compared byte for byte (pass 08 slice 5, D2).
///
/// # Errors
/// `INTERNAL` when the record cannot be serialised.
pub fn record_text(record: &RunRecord) -> Result<String, DomainError> {
    let mut text = serde_json::to_string_pretty(&record.stored()).map_err(|e| {
        DomainError::new(
            ErrorId::Internal,
            format!("a run record could not be serialised: {e}"),
        )
    })?;
    text.push('\n');
    Ok(text)
}

/// The directory `history/`, after the checks every single-record read makes
/// (the state root and `history/` private to `euid`, neither a symbolic
/// reference), or `None` when it does not exist. Creates nothing.
fn checked_history_directory(
    state: &StateDir,
    euid: u32,
) -> Result<Option<std::path::PathBuf>, DomainError> {
    let directory = state.history();
    match std::fs::symlink_metadata(&directory) {
        Ok(meta) => private_directory(&meta, euid)
            .map_err(|why| unusable(format!("the history directory {why}")))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(unusable(format!(
                "the history directory could not be inspected: {e}"
            )))
        }
    }
    let root_meta = std::fs::symlink_metadata(state.root())
        .map_err(|e| unusable(format!("the state directory could not be inspected: {e}")))?;
    private_directory(&root_meta, euid)
        .map_err(|why| unusable(format!("the state directory {why}")))?;
    Ok(Some(directory))
}

/// One record by id, checked as every record is; `None` when there is no such
/// record (a missing `history/` included). **Creates nothing.** An id that is
/// not a run id is refused, never turned into a path.
///
/// # Errors
/// `HISTORY_UNUSABLE` for anything this writer would not have left;
/// `SAFETY_BLOCKED` as root.
pub fn read_one(state: &StateDir, id: &str, euid: u32) -> Result<Option<RunRecord>, DomainError> {
    refuse_root(euid)?;
    let name = record_file_name(id)
        .ok_or_else(|| unusable("the record asked for is not named by a run id".into()))?;
    let Some(directory) = checked_history_directory(state, euid)? else {
        return Ok(None);
    };
    match std::fs::symlink_metadata(directory.join(&name)) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(unusable(format!("{name} could not be inspected: {e}"))),
    }
    read_record(&directory, id, euid).map(Some)
}

/// The record at `id`, read back and written again as text: what the run
/// compares with the text it wrote (`application::history::read_back_matches`).
/// Call it only after [`write_record`] returned `Ok` — after an error, or a
/// refused name, whatever stands at that id is not this run's record.
///
/// # Errors
/// As [`read_one`]; a record that is not there is `HISTORY_UNUSABLE` too.
pub fn read_back(state: &StateDir, id: &str, euid: u32) -> Result<String, DomainError> {
    match read_one(state, id, euid)? {
        Some(record) => record_text(&record),
        None => Err(unusable(
            "the record that was just written is not there to read back".into(),
        )),
    }
}

/// Makes the record at `id` durable and reads it back: the file and then
/// `history/` are synced (`F_FULLFSYNC` on Apple targets), and the record is
/// read again and must be the record that was read first. **This is what
/// `history acknowledge` does before it clears a marker** (pass 08 slice 5,
/// D13): a marker that names a record is how the record is known not to be
/// confirmed, so it is removed only once the record is known to have reached the
/// disk, and never for one that cannot be read.
///
/// # Errors
/// `HISTORY_UNUSABLE` when the record is not there, cannot be read, cannot be
/// synced, or changed while being synced; `SAFETY_BLOCKED` as root.
pub fn make_durable(state: &StateDir, id: &str, euid: u32) -> Result<RunRecord, DomainError> {
    let first = read_one(state, id, euid)?
        .ok_or_else(|| unusable("the record to make durable is not there".into()))?;
    let name = record_file_name(id)
        .ok_or_else(|| unusable("the record asked for is not named by a run id".into()))?;
    let directory = state.history();
    for path in [directory.join(&name), directory] {
        std::fs::File::open(&path)
            .and_then(|handle| handle.sync_all())
            .map_err(|e| {
                unusable(format!(
                    "the record could not be made durable ({:?}); it is still there and was not \
                     changed",
                    e.kind()
                ))
            })?;
    }
    let second = read_one(state, id, euid)?
        .ok_or_else(|| unusable("the record vanished while it was made durable".into()))?;
    if record_text(&first)? != record_text(&second)? {
        return Err(unusable(
            "the record changed while it was made durable".into(),
        ));
    }
    Ok(second)
}

/// The newest records, for `history`: what [`list_records`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordList {
    /// At most `limit` records, newest first (by the stamp in the id; two runs in
    /// one second are ordered by the random suffix of their ids, which is
    /// deterministic and not chronological).
    pub records: Vec<RunRecord>,
    /// How many records the store holds, including those not read.
    pub total: usize,
}

/// The newest `limit` records, each read and checked as every record is.
/// **Creates nothing**; a missing `history/` is an empty list. An unusable
/// record among those read is `HISTORY_UNUSABLE` naming its file, never a
/// shorter list: a list that skipped it would say a run did not happen.
///
/// # Errors
/// As [`read_one`]; and `HISTORY_UNUSABLE` for an unusable listing.
pub fn list_records(state: &StateDir, euid: u32, limit: usize) -> Result<RecordList, DomainError> {
    refuse_root(euid)?;
    let Some(ids) = listed_ids(state.root(), &state.history(), euid)? else {
        return Ok(RecordList {
            records: Vec::new(),
            total: 0,
        });
    };
    let directory = state.history();
    let records = ids
        .iter()
        .take(limit)
        .map(|id| read_record(&directory, id, euid))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RecordList {
        records,
        total: ids.len(),
    })
}

/// Reads what history says about `key`: newest first, stopping once every
/// record carrying the key's newest stamp has been read. Every entry met on
/// the way is read and checked, so an unusable record newer than the key's
/// latest fails closed — it could be a later run for the key.
///
/// A missing `history/` is [`HistoryView::empty`]; nothing is created.
pub fn read_view(
    state: &StateDir,
    key: &MigrationKey,
    euid: u32,
) -> Result<HistoryView, DomainError> {
    refuse_root(euid)?;
    let directory = state.history();
    let Some(ids) = listed_ids(state.root(), &directory, euid)? else {
        return Ok(HistoryView::empty());
    };
    let marker = super::run_marker::marker_state(state, euid);
    let newest_stamp = ids
        .first()
        .and_then(|id| run_id_stamp(id))
        .map(str::to_owned);
    let mut latest: Vec<RunRecord> = Vec::new();
    for id in &ids {
        if let Some(found) = latest.first() {
            if run_id_stamp(id) != run_id_stamp(found.id()) {
                break;
            }
        }
        let record = read_record(&directory, id, euid)?;
        if key.matches(&record) {
            latest.push(record);
        }
    }
    Ok(HistoryView::new(newest_stamp, latest, marker))
}

/// The ids in `history/`, newest first, or `None` when it does not exist.
fn listed_ids(
    root: &Path,
    directory: &Path,
    euid: u32,
) -> Result<Option<Vec<String>>, DomainError> {
    let meta = match std::fs::symlink_metadata(directory) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(unusable(format!(
                "the history directory could not be inspected: {e}"
            )))
        }
    };
    private_directory(&meta, euid)
        .map_err(|why| unusable(format!("the history directory {why}")))?;
    // The root holds it, so the root is held to the same rule: a history
    // inside a directory somebody else controls is not this user's history.
    let root_meta = std::fs::symlink_metadata(root)
        .map_err(|e| unusable(format!("the state directory could not be inspected: {e}")))?;
    private_directory(&root_meta, euid)
        .map_err(|why| unusable(format!("the state directory {why}")))?;

    let entries = std::fs::read_dir(directory)
        .map_err(|e| unusable(format!("the history directory could not be listed: {e}")))?;
    let mut ids = Vec::new();
    for (count, entry) in entries.enumerate() {
        if count >= MAX_ENTRIES {
            return Err(unusable(format!(
                "the history directory holds more than {MAX_ENTRIES} entries, hidden \
                 leftovers of killed runs included"
            )));
        }
        let entry = entry
            .map_err(|e| unusable(format!("the history directory could not be listed: {e}")))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(unusable(
                "the history directory holds an entry whose name is not text".into(),
            ));
        };
        // A dot-entry is a temporary this writer leaves on a kill, or a probe.
        if name.starts_with('.') {
            continue;
        }
        match name
            .strip_prefix("run-")
            .and_then(|r| r.strip_suffix(".json"))
        {
            Some(id) if is_run_id(id) => ids.push(id.to_owned()),
            _ => {
                return Err(unusable(format!(
                    "the history directory holds an entry this version did not write: {}",
                    macportsbrew_domain::text::escape_hostile(name)
                )))
            }
        }
    }
    // Ids begin with a fixed-width stamp, so the byte order is time order.
    ids.sort_unstable_by(|a, b| b.cmp(a));
    Ok(Some(ids))
}

/// Why a directory is not private to `euid`, if it is not.
pub(super) fn private_directory(meta: &Metadata, euid: u32) -> Result<(), &'static str> {
    if meta.file_type().is_symlink() {
        return Err("is a symbolic reference");
    }
    if !meta.is_dir() {
        return Err("is not a directory");
    }
    if meta.uid() != euid {
        return Err("is owned by somebody else");
    }
    if meta.permissions().mode() & 0o077 != 0 {
        return Err("is readable or writable by others");
    }
    Ok(())
}

/// Reads and checks one record.
fn read_record(directory: &Path, id: &str, euid: u32) -> Result<RunRecord, DomainError> {
    let name = record_file_name(id)
        .ok_or_else(|| unusable("the history directory names a record that is not a run".into()))?;
    let accept = |meta: &Metadata| -> Result<(), &'static str> {
        if meta.uid() != euid {
            return Err("is owned by somebody else");
        }
        if meta.permissions().mode() & 0o077 != 0 {
            return Err("is readable or writable by others");
        }
        Ok(())
    };
    let text = read_bounded(
        &directory.join(&name),
        MAX_RECORD_BYTES,
        OpenPolicy::RefuseSymlink,
        accept,
    )
    .map_err(|e| {
        unusable(match e {
            BoundedReadError::Open(e)
            | BoundedReadError::Inspect(e)
            | BoundedReadError::Read(e) => {
                format!("{name} could not be read: {e}")
            }
            BoundedReadError::NotAFile => format!("{name} is not a regular file"),
            BoundedReadError::Rejected(why) => format!("{name} {why}"),
            BoundedReadError::TooLarge { len } => {
                format!("{name} is {len} bytes, above the {MAX_RECORD_BYTES}-byte limit")
            }
            BoundedReadError::Grew => format!("{name} grew while it was read"),
        })
    })?;
    // serde's message can quote the file's own values; only where the
    // problem is, and what kind it is, are kept.
    let stored: StoredRunRecord = serde_json::from_str(&text).map_err(|e| {
        unusable(format!(
            "{name} is not a record this version reads ({:?} at line {}, column {})",
            e.classify(),
            e.line(),
            e.column()
        ))
    })?;
    let record =
        RunRecord::from_stored(stored).map_err(|e| unusable(format!("{name}: {}", e.detail)))?;
    if record.id() != id {
        return Err(unusable(format!("{name} states another run's id")));
    }
    Ok(record)
}
