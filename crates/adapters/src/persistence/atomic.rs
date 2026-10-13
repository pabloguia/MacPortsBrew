// SPDX-License-Identifier: Apache-2.0

//! Writing a file so that a reader never sees half of one.
//!
//! `04-migration-and-safety.md` ("Threat model", symlink attacks and unsafe
//! temp files) asks for three things together: temporary files created with
//! restrictive permissions in a private directory, never following a symbolic
//! link when writing, and an atomic write. [`write_atomic`] is all three.
//!
//! - The temporary file is a sibling of the target, so it is inside whatever
//!   private directory the caller chose — usually the state directory, which
//!   `state_dir` keeps at `0o700` — and so the rename that follows cannot
//!   cross a filesystem boundary and degrade into a copy.
//! - It is opened with `create_new`, which is `O_CREAT | O_EXCL`: the call
//!   fails if anything is already at that path, including a symbolic
//!   reference, so this code can never be redirected into writing somewhere
//!   else. Its mode is `0o600` from the moment it exists, not set afterwards.
//! - The contents are flushed to disk before the rename, and the rename is the
//!   single step that publishes them. A reader sees the old file or the new
//!   one.
//! - A failure this function *returns from* removes its temporary file. A
//!   half-written file left in the state directory is the "corrupt state"
//!   the threat model says must not happen. A kill or a power loss between
//!   creation and rename is the exception nothing can close from here: it
//!   leaves a hidden `.partial` sibling, which no reader will mistake for a
//!   snapshot but which nothing yet sweeps. Any listing of these directories
//!   must ignore names starting with a dot.

use macportsbrew_domain::{DomainError, ErrorId};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The mode a temporary file is created with, and therefore the mode the
/// published file ends up with.
pub const FILE_MODE: u32 = 0o600;

/// Distinguishes two temporary files made by one process at the same instant.
/// Not a clock read: it only has to be unique, and `create_new` catches a
/// collision anyway.
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to `path`, atomically, at mode `0o600`.
///
/// Refuses when the target is a symbolic reference rather than following it.
/// Replacing an ordinary existing file is fine and is the normal case.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), DomainError> {
    if std::fs::read_link(path).is_ok() {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            format!(
                "{} is a symbolic reference; refusing to write through it",
                path.display()
            ),
        ));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| {
            DomainError::new(
                ErrorId::Io,
                format!("{} has no directory to write into", path.display()),
            )
        })?;
    let temporary = temporary_sibling(path, parent);

    let write = || -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(FILE_MODE)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()
    };
    if let Err(e) = write() {
        // `AlreadyExists` means the temporary name was taken before this call
        // — a leftover from a killed run, or something planted. That file is
        // not ours, so it is not deleted; every other failure happened after
        // this call created the file, and that one must not be left over.
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            std::fs::remove_file(&temporary).ok();
        }
        return Err(DomainError::new(
            ErrorId::Io,
            format!("{} could not be written: {e}", temporary.display()),
        ));
    }
    if let Err(e) = std::fs::rename(&temporary, path) {
        std::fs::remove_file(&temporary).ok();
        return Err(DomainError::new(
            ErrorId::Io,
            format!("{} could not be published: {e}", path.display()),
        ));
    }
    // Best effort: the rename is durable once the directory entry reaches
    // disk. A filesystem that will not let a directory be opened for this is
    // not a reason to report a successful write as a failure.
    if let Ok(directory) = File::open(parent) {
        directory.sync_all().ok();
    }
    Ok(())
}

/// Why [`write_new_atomic`] did not publish.
#[derive(Debug)]
pub enum PublishError {
    /// Something is already at the target and it is not this call's file.
    /// The caller chooses another name; nothing was replaced.
    Exists,
    /// Anything else, including a filesystem that cannot publish without
    /// replacing. Nothing was replaced and nothing of this call's is left.
    Failed(DomainError),
}

/// Writes `bytes` to `path` **only if nothing is there**, atomically, at mode
/// `0o600`: the publish an append-only store needs (pass 08, the history
/// store), where [`write_atomic`]'s rename would replace.
///
/// The temporary sibling is written and flushed, then `link`ed to the target.
/// `link` never replaces and never follows a symbolic reference at the
/// target, so no check before it is needed or made — one would only open a
/// window between the check and the write.
///
/// - The mode is set on the open handle (`fchmod`) after creation, so an
///   unusual umask cannot leave a record `0400` and unreadable forever.
/// - `sync_all` is `fcntl(F_FULLFSYNC)` on Apple targets
///   (`library/std/src/sys/fs/unix.rs`, `File::fsync`, rust-lang/rust master,
///   read 2026-10-04): the bytes reach the medium before the name exists.
/// - **It never falls back to `rename`.** A `link` error other than "already
///   exists" means this filesystem cannot publish without replacing (exFAT, a
///   network share), and that is reported, not worked around.
/// - "Already exists" with the temporary at two links means the link landed
///   and only its reply was lost (a network filesystem's retransmission):
///   that is success. Otherwise it is [`PublishError::Exists`].
/// - The temporary is a dot-entry, which every listing of the state directory
///   ignores; a kill between the link and its removal leaves it hidden.
pub fn write_new_atomic(path: &Path, bytes: &[u8]) -> Result<(), PublishError> {
    write_new_with(
        path,
        bytes,
        |from, to| std::fs::hard_link(from, to),
        Durability::BestEffort,
        sync_directory,
    )
}

/// [`write_new_atomic`], but the directory entry must reach the medium before
/// this returns `Ok`: the interrupted-run marker exists to survive a power
/// loss, and a lost entry after the first provider change would leave neither
/// a marker nor a record. When the directory cannot be synced the result is
/// [`PublishError::Failed`] **though the entry is published**: the caller
/// treats the write as not having happened (it dispatches nothing) and clears
/// what it wrote.
pub fn write_new_durable(path: &Path, bytes: &[u8]) -> Result<(), PublishError> {
    write_new_with(
        path,
        bytes,
        |from, to| std::fs::hard_link(from, to),
        Durability::Required,
        sync_directory,
    )
}

/// Whether a failed directory sync fails the write.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Durability {
    /// A record: the marker notices a power loss before the sync.
    BestEffort,
    /// The marker itself: nothing notices if it is lost.
    Required,
}

fn sync_directory(directory: &Path) -> std::io::Result<()> {
    File::open(directory)?.sync_all()
}

/// [`write_new_atomic`] with the publishing call and the directory sync passed
/// in, so a test can make the first lose its reply the way a network filesystem
/// does (the entry lands, the caller is told it exists) and the second fail.
/// Private: no production caller supplies another.
fn write_new_with(
    path: &Path,
    bytes: &[u8],
    publish: impl Fn(&Path, &Path) -> std::io::Result<()>,
    durability: Durability,
    sync: impl Fn(&Path) -> std::io::Result<()>,
) -> Result<(), PublishError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let failed = |what: String| PublishError::Failed(DomainError::new(ErrorId::Io, what));
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| failed(format!("{} has no directory to write into", name_of(path))))?;
    let temporary = temporary_sibling(path, parent);

    let write = || -> std::io::Result<File> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(FILE_MODE)
            .open(&temporary)?;
        file.set_permissions(std::fs::Permissions::from_mode(FILE_MODE))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(file)
    };
    let file = match write() {
        Ok(file) => file,
        Err(e) => {
            // As in `write_atomic`: a name taken before this call is not ours.
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                std::fs::remove_file(&temporary).ok();
            }
            return Err(failed(format!(
                "{} could not be written: {}",
                name_of(&temporary),
                e.kind()
            )));
        }
    };
    let linked = publish(&temporary, path);
    let outcome = match linked {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => match file.metadata() {
            Ok(meta) if lost_reply(meta.nlink()) => Ok(()),
            _ => Err(PublishError::Exists),
        },
        Err(e) => Err(failed(format!(
            "{} could not be published without replacing: {}",
            name_of(path),
            e.kind()
        ))),
    };
    std::fs::remove_file(&temporary).ok();
    if outcome.is_ok() {
        // Best effort for a record, as in `write_atomic`: the entry is durable
        // once the directory reaches disk; a power loss before then can lose
        // the record, which the interrupted-run marker exists to notice. For
        // the marker itself the sync must succeed.
        if let Err(e) = sync(parent) {
            if durability == Durability::Required {
                return Err(failed(format!(
                    "{} was published but its directory could not be synced: {}",
                    name_of(path),
                    e.kind()
                )));
            }
        }
    }
    outcome
}

/// A path's last component, for an error text: the full path holds the user's
/// name, and these details can be quoted into output meant for an issue.
fn name_of(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// Whether an "already exists" from the publishing call was this call's own
/// link, only its reply lost: the temporary then has exactly two links, its own
/// name and the target. Anything else is somebody else's entry. Separate and
/// pure so the rule is tested without a network filesystem to lose a reply on.
fn lost_reply(links_to_temporary: u64) -> bool {
    links_to_temporary == 2
}

/// A name no other writer will pick, next to the target.
///
/// The process id and the sequence keep two of *our* writers apart. The random
/// component keeps somebody else's writer out: `create_new` means a temporary
/// name an adversary has already taken makes the write fail, and with a
/// predictable name they can take every one of them in advance. Inside the
/// state directory that is impossible — it is `0700` and nobody else can create
/// anything there — but `write_atomic` also serves `snapshot --out` and
/// `doctor --report`, whose paths the user chooses and which are frequently in
/// a shared directory. Denial of service only, because `create_new` and the
/// ownership check mean the write refuses rather than being redirected; it is
/// still a denial of service that costs one line to remove (F-20).
///
/// The randomness is `RandomState`, which `std` seeds from the operating
/// system. Not a clock — a clock is guessable and this file is not a place to
/// introduce nondeterminism — and not a new dependency, which would be an ADR.
fn temporary_sibling(path: &Path, parent: &Path) -> PathBuf {
    use std::hash::{BuildHasher, Hasher};
    let stem = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(SEQUENCE.fetch_add(1, Ordering::Relaxed));
    parent.join(format!(
        ".{stem}.{}.{:016x}.partial",
        std::process::id(),
        hasher.finish()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn the_temporary_name_is_not_predictable() {
        // Two calls with identical inputs must not agree, or an adversary who
        // can write to the target's directory can pre-create the name and make
        // every write fail. `create_new` is what makes this a denial of service
        // rather than a redirection, and it is also what makes the collision
        // fatal rather than harmless.
        let target = Path::new("/tmp/example/report.json");
        let parent = Path::new("/tmp/example");
        //
        // "The names differ" is not the property, and asserting it was this
        // test's first shape: the old `.{stem}.{pid}.{seq}.partial` satisfied
        // it too, because the sequence advances. What distinguishes the two is
        // *spread*. A counter starting at zero leaves the top bits of its field
        // empty for the life of the process; an OS-seeded hash fills them. So
        // the assertion is that the variable component ranges over the whole
        // 64-bit space, which no counter this process could reach does.
        let variable = |p: &Path| -> u64 {
            let text = p.file_name().unwrap().to_string_lossy().into_owned();
            let field = text
                .strip_suffix(".partial")
                .and_then(|t| t.rsplit('.').next().map(str::to_string))
                .expect("the name has a variable field");
            u64::from_str_radix(&field, 16).expect("the field is hexadecimal")
        };
        let names: Vec<PathBuf> = (0..64).map(|_| temporary_sibling(target, parent)).collect();
        let values: std::collections::BTreeSet<u64> = names.iter().map(|p| variable(p)).collect();
        assert_eq!(values.len(), 64, "every temporary name differs");
        assert!(
            values.iter().copied().max().unwrap() > u64::from(u32::MAX),
            "the variable field is a counter, not a random value: {values:?}"
        );
        for name in &names {
            let text = name.to_string_lossy();
            assert!(text.starts_with("/tmp/example/.report.json."), "{text}");
            assert!(text.ends_with(".partial"), "{text}");
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "macportsbrew-{tag}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_write_lands_privately_and_leaves_nothing_behind() {
        let dir = scratch("atomic");
        let target = dir.join("state.json");
        write_atomic(&target, b"first").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"first");
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            FILE_MODE
        );
        let after: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(after, vec![std::ffi::OsString::from("state.json")]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_second_write_replaces_the_first_entirely() {
        let dir = scratch("atomic-replace");
        let target = dir.join("state.json");
        write_atomic(&target, b"a longer first version").unwrap();
        write_atomic(&target, b"short").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"short");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn writing_into_a_directory_that_is_not_there_fails_without_a_leftover() {
        let dir = scratch("atomic-absent");
        let target = dir.join("no-such-directory").join("state.json");
        assert_eq!(write_atomic(&target, b"x").unwrap_err().id, ErrorId::Io);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_empty_payload_is_a_valid_write() {
        let dir = scratch("atomic-empty");
        let target = dir.join("empty");
        write_atomic(&target, b"").unwrap();
        assert!(std::fs::read(&target).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn each_temporary_name_is_hidden_and_distinct() {
        let a = temporary_sibling(Path::new("/s/state.json"), Path::new("/s"));
        let b = temporary_sibling(Path::new("/s/state.json"), Path::new("/s"));
        assert_ne!(a, b);
        for p in [&a, &b] {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            assert!(name.starts_with(".state.json."), "{name}");
            assert!(name.ends_with(".partial"), "{name}");
        }
    }

    #[test]
    fn a_new_write_lands_privately_and_leaves_nothing_behind() {
        let dir = scratch("atomic-new");
        let target = dir.join("run.json");
        write_new_atomic(&target, b"first").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"first");
        let meta = std::fs::metadata(&target).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, FILE_MODE);
        assert_eq!(std::os::unix::fs::MetadataExt::nlink(&meta), 1);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_new_write_never_replaces_what_is_there() {
        let dir = scratch("atomic-new-exists");
        let target = dir.join("run.json");
        std::fs::write(&target, b"theirs").unwrap();
        assert!(matches!(
            write_new_atomic(&target, b"ours"),
            Err(PublishError::Exists)
        ));
        assert_eq!(std::fs::read(&target).unwrap(), b"theirs");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no leftover");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_new_write_into_a_directory_that_is_not_there_fails_without_a_leftover() {
        let dir = scratch("atomic-new-absent");
        let target = dir.join("no-such-directory").join("run.json");
        assert!(matches!(
            write_new_atomic(&target, b"x"),
            Err(PublishError::Failed(e)) if e.id == ErrorId::Io
        ));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_lost_reply_is_only_a_temporary_at_exactly_two_links() {
        // The link landed and its reply was lost: the temporary is the target.
        assert!(lost_reply(2));
        // Our link did not land (the temporary is alone), or somebody else has
        // also linked the entry: not ours, so a collision.
        assert!(!lost_reply(1));
        assert!(!lost_reply(3));
        assert!(!lost_reply(0));
    }

    /// The link lands; the caller is told the name exists.
    fn lost(from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::hard_link(from, to)?;
        Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists))
    }

    #[test]
    fn a_publish_whose_reply_was_lost_is_a_success_and_leaves_nothing_behind() {
        let dir = scratch("atomic-lost-reply");
        let target = dir.join("run.json");
        write_new_with(
            &target,
            b"ours",
            lost,
            Durability::BestEffort,
            sync_directory,
        )
        .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"ours");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no leftover");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_exists_that_was_not_this_calls_link_is_a_collision() {
        let dir = scratch("atomic-not-ours");
        let target = dir.join("run.json");
        std::fs::write(&target, b"theirs").unwrap();
        let refused =
            |_: &Path, _: &Path| Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
        assert!(matches!(
            write_new_with(
                &target,
                b"ours",
                refused,
                Durability::BestEffort,
                sync_directory
            ),
            Err(PublishError::Exists)
        ));
        assert_eq!(std::fs::read(&target).unwrap(), b"theirs");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_publish_error_that_is_not_exists_fails_without_replacing() {
        let dir = scratch("atomic-cannot-publish");
        let target = dir.join("run.json");
        let cannot =
            |_: &Path, _: &Path| Err(std::io::Error::from(std::io::ErrorKind::Unsupported));
        assert!(matches!(
            write_new_with(&target, b"x", cannot, Durability::BestEffort, sync_directory),
            Err(PublishError::Failed(e)) if e.id == ErrorId::Io
        ));
        assert!(!target.exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "no leftover");
        std::fs::remove_dir_all(&dir).ok();
    }

    fn failing_sync(_: &Path) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }

    fn link(from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::hard_link(from, to)
    }

    #[test]
    fn a_required_directory_sync_that_fails_fails_the_write() {
        let dir = scratch("atomic-sync-required");
        let target = dir.join("marker.json");
        let result = write_new_with(&target, b"m", link, Durability::Required, failing_sync);
        assert!(matches!(result, Err(PublishError::Failed(e)) if e.id == ErrorId::Io));
        // Published all the same: the caller treats it as not written and clears it.
        assert!(target.exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temporary");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_best_effort_directory_sync_that_fails_does_not_fail_the_write() {
        let dir = scratch("atomic-sync-best-effort");
        let target = dir.join("run.json");
        write_new_with(&target, b"r", link, Durability::BestEffort, failing_sync).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"r");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_durable_write_lands_privately_and_never_replaces() {
        let dir = scratch("atomic-durable");
        let target = dir.join("marker.json");
        write_new_durable(&target, b"first").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"first");
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            FILE_MODE
        );
        assert!(matches!(
            write_new_durable(&target, b"second"),
            Err(PublishError::Exists)
        ));
        assert_eq!(std::fs::read(&target).unwrap(), b"first");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_path_with_no_directory_part_is_refused_by_both_writers_without_a_leftover() {
        let bare = Path::new("no-directory-part.json");
        assert!(write_atomic(bare, b"x").is_err());
        assert!(matches!(
            write_new_atomic(bare, b"x"),
            Err(PublishError::Failed(e)) if e.detail.contains("no directory")
        ));
        assert!(matches!(
            write_new_durable(bare, b"x"),
            Err(PublishError::Failed(_))
        ));
        assert!(!bare.exists());
    }

    #[test]
    fn a_write_over_a_directory_fails_and_leaves_no_temporary() {
        let dir = scratch("atomic-over-directory");
        let target = dir.join("is-a-directory");
        std::fs::create_dir(&target).unwrap();
        assert!(write_atomic(&target, b"x").is_err());
        assert!(target.is_dir());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temporary");
        std::fs::remove_dir_all(&dir).ok();
    }
}
