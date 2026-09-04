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
}
