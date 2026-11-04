// SPDX-License-Identifier: Apache-2.0

//! The provider lock probe (DP-030 (b); R-17; ADR-0007).
//!
//! A sample of each detected provider's own mutation locks at one instant:
//! "a lock on this provider's lock files was held", "no held lock was
//! observed", or "could not be determined" — never "the provider is idle".
//! Where the locks are is data (`data/policy/provider-locks.toml`, read only by
//! [`crate::registry`]), resolved there into a [`LockSite`]; this module
//! branches on the site's shape and never on which provider it belongs to
//! (INV-17), and it is the only module that names `libc`.
//!
//! **It never creates, writes, truncates or unlinks anything** (R-17 rules
//! 2–3). Every lock file is opened read-only with `O_NOFOLLOW | O_NONBLOCK`,
//! checked through `fstat` on the handle to be a regular file on the lock
//! directory's device, and queried with `F_GETLK` ([`getlk`]). Nothing here
//! needs elevation: a read-only open of a `0444` or `root:wheel` file works
//! (R-17 finding 4; INV-06).
//!
//! **The lock directory itself is never followed** (G item 1). Its
//! `symlink_metadata` must be a real directory, and its `(st_dev, st_ino)` is
//! checked again after the listing and after every open; a change is "not
//! determined". An ancestor that is a symlink is followed, as the provider
//! follows it (declared in 04). Every error, every unexpected shape and every
//! surprise is "not determined"; the only "missing" is `NotFound`.

mod getlk;

use getlk::LockState;
use macportsbrew_domain::{
    ConcurrentProviderActivity, ProviderId, ProviderLockProbe, UndeterminedCause,
};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::os::fd::AsFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// The most entries a lock directory may hold before the listing is "not
/// determined". **A safety limit, not a provider fact** (G item 17): it bounds
/// the work one sample does on a directory anyone with write access could
/// fill. R-17 E2 saw a few hundred entries; a directory over the bound refuses
/// rather than being half-read.
const MAX_ENTRIES: usize = 4096;

/// Where one provider keeps its lock files, as the registry resolved it for
/// this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LockSite {
    /// Every entry of this directory is a lock file, whatever its name. A
    /// missing directory holds no lock file, so nothing is held.
    Directory(PathBuf),
    /// One lock file, the leaf `name` inside `dir`. The directory always
    /// exists on a working installation, so a missing `dir` is "not
    /// determined"; a missing file inside a present `dir` is nothing held.
    File {
        /// The directory holding the file; its identity is checked.
        dir: PathBuf,
        /// The file's single-component name.
        name: OsString,
    },
    /// The location could not be resolved on this machine (for example, its
    /// base directory is not the default one). Always "not determined".
    Unresolved,
}

/// One provider's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Answer {
    NotHeld,
    Held,
    NotDetermined(UndeterminedCause),
}

/// A directory's identity, `(st_dev, st_ino)`, as `lstat` reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
}

/// What the probe does when it is asked.
enum Mode {
    /// The input is not this host: answer `Unknowable` without looking.
    Unknowable,
    /// Sample these providers' sites.
    Sites(Vec<(ProviderId, LockSite)>),
}

/// The probe over this machine's detected providers.
///
/// Built only by [`crate::registry::lock_probe`], from what detection and the
/// providers' own configuration resolved: no public constructor takes a path, so no
/// value from a plan, a snapshot or a flag can point it at another directory
/// (G item 13).
pub struct SystemLockProbe {
    mode: Mode,
}

impl std::fmt::Debug for SystemLockProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.mode {
            Mode::Unknowable => f.write_str("SystemLockProbe(Unknowable)"),
            Mode::Sites(sites) => f
                .debug_struct("SystemLockProbe")
                .field(
                    "providers",
                    &sites.iter().map(|(p, _)| p).collect::<Vec<_>>(),
                )
                .finish_non_exhaustive(),
        }
    }
}

impl SystemLockProbe {
    /// A probe that answers `Unknowable` and touches nothing: a replay, or a
    /// filesystem that is not the machine described (INV-15).
    pub(crate) fn unknowable() -> Self {
        Self {
            mode: Mode::Unknowable,
        }
    }

    /// A probe over `sites`, one per detected provider.
    pub(crate) fn over(sites: Vec<(ProviderId, LockSite)>) -> Self {
        Self {
            mode: Mode::Sites(sites),
        }
    }

    /// Whether this probe answers without looking. For the registry's tests.
    #[cfg(test)]
    pub(crate) fn is_unknowable(&self) -> bool {
        matches!(self.mode, Mode::Unknowable)
    }

    /// The sites it samples. For the registry's tests.
    #[cfg(test)]
    pub(crate) fn sites(&self) -> &[(ProviderId, LockSite)] {
        match &self.mode {
            Mode::Unknowable => &[],
            Mode::Sites(sites) => sites,
        }
    }
}

impl ProviderLockProbe for SystemLockProbe {
    fn sample(&self) -> ConcurrentProviderActivity {
        match &self.mode {
            Mode::Unknowable => ConcurrentProviderActivity::Unknowable,
            Mode::Sites(sites) => combine(
                sites
                    .iter()
                    .map(|(provider, site)| (provider.clone(), sample_site(site, &|_| {}))),
            ),
        }
    }
}

/// Whether a provider that detection did not find is really absent: its
/// executable answers `NotFound` under every candidate. Detection asks
/// `is_file`, which turns every error into "no", so a loop, a refused search
/// or an I/O error reaches here looking like absence, and is not.
pub(crate) fn absence_is_certain(candidates: &[PathBuf], executable: &str) -> bool {
    candidates.iter().all(|prefix| {
        matches!(
            fs::symlink_metadata(prefix.join(executable)),
            Err(e) if e.kind() == ErrorKind::NotFound
        )
    })
}

/// The site of a provider detection did not find: none if it is really absent,
/// and `Unresolved` if it may only have failed to be checked, so that an error
/// is sampled as "not determined" and never left out.
pub(crate) fn undetected_site(candidates: &[PathBuf], executable: &str) -> Option<LockSite> {
    (!absence_is_certain(candidates, executable)).then_some(LockSite::Unresolved)
}

/// One activity from the providers' answers: any held lock is `Observed` (it
/// is what matters most, and it is a definite answer), still naming whoever
/// was not determined; otherwise any provider not determined makes the whole
/// `NotDetermined`; only every provider answering "not held" is `NoneObserved`.
fn combine(answers: impl Iterator<Item = (ProviderId, Answer)>) -> ConcurrentProviderActivity {
    let mut held = BTreeSet::new();
    let mut undetermined = BTreeMap::new();
    for (provider, answer) in answers {
        match answer {
            Answer::Held => {
                held.insert(provider);
            }
            Answer::NotDetermined(cause) => {
                undetermined.insert(provider, cause);
            }
            Answer::NotHeld => {}
        }
    }
    if !held.is_empty() {
        ConcurrentProviderActivity::Observed {
            providers: held,
            undetermined,
        }
    } else if !undetermined.is_empty() {
        ConcurrentProviderActivity::NotDetermined { undetermined }
    } else {
        ConcurrentProviderActivity::NoneObserved
    }
}

/// Samples one site. `between` runs after a directory has been listed and
/// before any entry is opened: a no-op in production, the race seam in tests.
pub(crate) fn sample_site(site: &LockSite, between: &dyn Fn(&Path)) -> Answer {
    match site {
        LockSite::Directory(dir) => sample_directory(dir, between),
        LockSite::File { dir, name } => sample_file(dir, name, between),
        LockSite::Unresolved => Answer::NotDetermined(UndeterminedCause::Unresolved),
    }
}

/// The directory's identity, if it is a real directory (not followed).
/// `Ok(None)` is `NotFound`, the only "missing"; every other error is `Err`.
fn directory_identity(dir: &Path) -> Result<Option<Identity>, UndeterminedCause> {
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_dir() => Ok(Some(Identity {
            dev: meta.dev(),
            ino: meta.ino(),
        })),
        Ok(_) => Err(UndeterminedCause::UnexpectedShape),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(UndeterminedCause::Unreadable),
    }
}

/// Whether `dir` is still the directory first seen.
fn still(dir: &Path, identity: Identity) -> bool {
    matches!(directory_identity(dir), Ok(Some(now)) if now == identity)
}

/// What a directory's listing yielded: the regular files in it, and the
/// strongest thing met that is not a plain answer. The strongest, so the cause
/// does not depend on the order the entries came in (INV-15).
#[derive(Debug, Default, PartialEq, Eq)]
struct Listing {
    names: Vec<OsString>,
    surprise: Option<UndeterminedCause>,
}

/// Reads a listing of `(name, is it a regular file)`, up to [`MAX_ENTRIES`].
/// An entry that cannot be read is never skipped (G item 20 N2).
fn read_listing<I>(items: I) -> Listing
where
    I: IntoIterator<Item = std::io::Result<(OsString, std::io::Result<bool>)>>,
{
    let mut listing = Listing::default();
    for (seen, item) in items.into_iter().enumerate() {
        if seen == MAX_ENTRIES {
            listing.surprise = listing.surprise.max(Some(UndeterminedCause::OverBound));
            break;
        }
        match item {
            Ok((name, Ok(true))) => listing.names.push(name),
            // A link, a subdirectory, a device node or a FIFO is refused here,
            // without being opened (G item 1).
            Ok((_, Ok(false))) => {
                listing.surprise = listing
                    .surprise
                    .max(Some(UndeterminedCause::UnexpectedShape));
            }
            Ok((_, Err(_))) => {
                listing.surprise = listing.surprise.max(Some(UndeterminedCause::Unreadable));
            }
            Err(_) => {
                listing.surprise = listing.surprise.max(Some(UndeterminedCause::Unreadable));
                break;
            }
        }
    }
    listing
}

/// Every entry of `dir` is a lock file.
///
/// A definite "held" outranks "not determined", as it does across providers
/// ([`combine`]), so a stray entry cannot hide a running provider behind
/// advice to clean the directory. The directory's identity outranks both: if
/// it changed, no answer about it can be trusted.
fn sample_directory(dir: &Path, between: &dyn Fn(&Path)) -> Answer {
    let identity = match directory_identity(dir) {
        Ok(Some(identity)) => identity,
        // No lock directory, so no lock file exists to be held.
        Ok(None) => return Answer::NotHeld,
        Err(cause) => return Answer::NotDetermined(cause),
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return Answer::NotDetermined(UndeterminedCause::Unreadable);
    };
    // `file_type` does not follow.
    let listing = read_listing(entries.map(|item| {
        item.map(|entry| {
            (
                entry.file_name(),
                entry.file_type().map(|kind| kind.is_file()),
            )
        })
    }));
    if !still(dir, identity) {
        return Answer::NotDetermined(UndeterminedCause::Changed);
    }
    between(dir);
    let mut held = false;
    let mut undetermined = listing.surprise;
    for name in &listing.names {
        match probe_file(dir, name, identity) {
            Answer::Held => held = true,
            Answer::NotDetermined(cause) => undetermined = undetermined.max(Some(cause)),
            Answer::NotHeld => {}
        }
    }
    if !still(dir, identity) {
        Answer::NotDetermined(UndeterminedCause::Changed)
    } else if held {
        Answer::Held
    } else {
        undetermined.map_or(Answer::NotHeld, Answer::NotDetermined)
    }
}

/// One named lock file inside a directory that must exist.
fn sample_file(dir: &Path, name: &OsStr, between: &dyn Fn(&Path)) -> Answer {
    // A working installation always has this directory, so its absence is
    // not "nothing held" (G item 20 N2).
    let identity = match directory_identity(dir) {
        Ok(Some(identity)) => identity,
        Ok(None) => return Answer::NotDetermined(UndeterminedCause::Unresolved),
        Err(cause) => return Answer::NotDetermined(cause),
    };
    between(dir);
    probe_file(dir, name, identity)
}

/// Whether `name` is one path component naming an entry of its directory:
/// not empty, not `.` or `..`, no `/` (§3 item 10). The name may be untrusted
/// (a directory listing), so it is checked before it is joined.
fn is_leaf(name: &OsStr) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty() && bytes != b"." && bytes != b".." && !bytes.contains(&b'/')
}

/// Opens `dir/name` read-only, never following and never blocking, and asks
/// whether it is locked.
fn probe_file(dir: &Path, name: &OsStr, identity: Identity) -> Answer {
    if !is_leaf(name) {
        return Answer::NotDetermined(UndeterminedCause::UnexpectedShape);
    }
    let opened = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(dir.join(name));
    let answer = match opened {
        Ok(file) => query(&file, identity),
        // Gone between the listing and the open: a provider's cleanup may
        // unlink lock files (R-17 E1). No longer a lock file, so not held — provided
        // the directory is still the one listed, which is checked below.
        Err(e) if e.kind() == ErrorKind::NotFound => Answer::NotHeld,
        // A link fails `O_NOFOLLOW`; a swap after the listing is how it gets here.
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => {
            Answer::NotDetermined(UndeterminedCause::UnexpectedShape)
        }
        Err(_) => Answer::NotDetermined(UndeterminedCause::Unreadable),
    };
    if still(dir, identity) {
        answer
    } else {
        Answer::NotDetermined(UndeterminedCause::Changed)
    }
}

/// `fstat` on the handle, then the query.
fn query(file: &File, identity: Identity) -> Answer {
    let Ok(meta) = file.metadata() else {
        return Answer::NotDetermined(UndeterminedCause::Unreadable);
    };
    if !meta.file_type().is_file() || meta.dev() != identity.dev {
        return Answer::NotDetermined(UndeterminedCause::UnexpectedShape);
    }
    match getlk::held(file.as_fd()) {
        LockState::Held => Answer::Held,
        LockState::NotHeld => Answer::NotHeld,
        LockState::Unknown => Answer::NotDetermined(UndeterminedCause::Unreadable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A fresh scratch directory under the test's temp dir. Never a provider's.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mpb-lock-probe-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(dir);
    }

    fn directory(dir: &Path) -> Answer {
        sample_site(&LockSite::Directory(dir.to_path_buf()), &|_| {})
    }

    fn file_site(dir: &Path, name: &str) -> LockSite {
        LockSite::File {
            dir: dir.to_path_buf(),
            name: OsString::from(name),
        }
    }

    fn touch(path: &Path) -> File {
        File::create(path).expect("scratch file")
    }

    #[test]
    fn a_missing_lock_directory_holds_nothing() {
        let root = scratch("missing-dir");
        assert_eq!(directory(&root.join("absent")), Answer::NotHeld);
        cleanup(&root);
    }

    #[test]
    fn an_empty_or_unlocked_directory_holds_nothing() {
        let root = scratch("unlocked");
        assert_eq!(directory(&root), Answer::NotHeld);
        touch(&root.join("a.formula.lock"));
        touch(&root.join("update"));
        assert_eq!(directory(&root), Answer::NotHeld);
        cleanup(&root);
    }

    #[test]
    fn a_lock_held_on_any_entry_is_held_whatever_its_suffix() {
        let root = scratch("held");
        touch(&root.join("a.formula.lock"));
        let holder = touch(&root.join("update"));
        holder.lock().expect("the test locks its own scratch file");
        assert_eq!(directory(&root), Answer::Held);
        holder.unlock().expect("unlocks");
        assert_eq!(directory(&root), Answer::NotHeld);
        cleanup(&root);
    }

    #[test]
    fn a_subdirectory_entry_is_not_determined() {
        let root = scratch("subdir-entry");
        fs::create_dir(root.join("nested")).expect("subdir");
        assert_eq!(
            directory(&root),
            Answer::NotDetermined(UndeterminedCause::UnexpectedShape)
        );
        cleanup(&root);
    }

    #[test]
    fn a_listing_over_the_bound_is_not_determined_and_at_it_is_read() {
        let root = scratch("bound");
        for i in 0..MAX_ENTRIES {
            touch(&root.join(format!("{i}.lock")));
        }
        assert_eq!(directory(&root), Answer::NotHeld, "exactly the bound");
        touch(&root.join("one-more.lock"));
        assert_eq!(
            directory(&root),
            Answer::NotDetermined(UndeterminedCause::OverBound),
            "over the bound"
        );
        cleanup(&root);
    }

    #[test]
    fn a_lock_directory_that_is_a_file_is_not_determined() {
        let root = scratch("dir-is-file");
        touch(&root.join("locks"));
        assert_eq!(
            directory(&root.join("locks")),
            Answer::NotDetermined(UndeterminedCause::UnexpectedShape)
        );
        cleanup(&root);
    }

    #[test]
    fn an_unlistable_directory_is_not_determined_never_empty() {
        // Mutant 7: a listing error read as "no lock file".
        let root = scratch("unlistable");
        let locks = root.join("locks");
        fs::create_dir(&locks).expect("dir");
        touch(&locks.join("x.lock"));
        fs::set_permissions(&locks, fs::Permissions::from_mode(0o000)).expect("chmod");
        assert_eq!(
            directory(&locks),
            Answer::NotDetermined(UndeterminedCause::Unreadable)
        );
        cleanup(&locks);
        cleanup(&root);
    }

    #[test]
    fn an_entry_gone_between_the_listing_and_its_open_is_no_longer_a_lock_file() {
        // G item 2: a provider's cleanup may unlink lock files (R-17 E1).
        let root = scratch("enoent-race");
        touch(&root.join("gone.lock"));
        let answer = sample_site(&LockSite::Directory(root.clone()), &|dir| {
            fs::remove_file(dir.join("gone.lock")).expect("the race, played");
        });
        assert_eq!(answer, Answer::NotHeld);
        cleanup(&root);
    }

    #[test]
    fn a_directory_swapped_after_its_listing_is_not_determined() {
        // The identity re-check after an open. The directory is replaced
        // by another with the same entry, which the open would then follow.
        let root = scratch("identity-swap");
        let locks = root.join("locks");
        fs::create_dir(&locks).expect("dir");
        touch(&locks.join("x.lock"));
        let answer = sample_site(&LockSite::Directory(locks.clone()), &|dir| {
            fs::rename(dir, dir.with_file_name("old")).expect("moved away");
            fs::create_dir(dir).expect("a new directory in its place");
            touch(&dir.join("x.lock"));
        });
        assert_eq!(answer, Answer::NotDetermined(UndeterminedCause::Changed));
        cleanup(&root);
    }

    #[test]
    fn a_named_lock_file_is_held_free_or_missing() {
        let root = scratch("named");
        let site = file_site(&root, "the.lock");
        assert_eq!(sample_site(&site, &|_| {}), Answer::NotHeld, "missing file");
        let holder = touch(&root.join("the.lock"));
        assert_eq!(sample_site(&site, &|_| {}), Answer::NotHeld, "free");
        holder.lock().expect("locks");
        assert_eq!(sample_site(&site, &|_| {}), Answer::Held, "held");
        cleanup(&root);
    }

    #[test]
    fn a_named_lock_files_missing_directory_is_not_determined() {
        // G item 20 N2: a working installation always has the
        // directory, so its absence is not "nothing held".
        let root = scratch("named-missing-dir");
        let site = file_site(&root.join("absent"), "the.lock");
        assert_eq!(
            sample_site(&site, &|_| {}),
            Answer::NotDetermined(UndeterminedCause::Unresolved)
        );
        cleanup(&root);
    }

    #[test]
    fn a_name_that_is_not_one_leaf_is_refused_before_it_is_joined() {
        for bad in ["", ".", "..", "a/b", "/abs", "../up"] {
            assert!(!is_leaf(OsStr::new(bad)), "{bad:?}");
        }
        for good in [".registry-like.lock", "update", "a.b.c"] {
            assert!(is_leaf(OsStr::new(good)), "{good:?}");
        }
        let root = scratch("not-a-leaf");
        assert_eq!(
            sample_site(&file_site(&root, ".."), &|_| {}),
            Answer::NotDetermined(UndeterminedCause::UnexpectedShape)
        );
        cleanup(&root);
    }

    #[test]
    fn only_not_found_under_every_candidate_is_a_certain_absence() {
        // The dangling and looping cases are in
        // `crates/adapters/tests/lock_probe_shapes.rs`.
        let root = scratch("absence");
        let closed = root.join("closed");
        fs::create_dir_all(closed.join("bin")).expect("a scratch prefix");
        fs::create_dir_all(root.join("empty")).expect("a scratch prefix");
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o000)).expect("chmod");
        let empty = root.join("empty");
        assert!(absence_is_certain(std::slice::from_ref(&empty), "bin/tool"));
        assert!(absence_is_certain(&[root.join("missing")], "bin/tool"));
        assert!(!absence_is_certain(&[empty, closed.clone()], "bin/tool"));
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o755)).expect("chmod");
        cleanup(&root);
    }

    #[test]
    fn an_undetected_provider_that_could_not_be_checked_is_sampled_unresolved() {
        // Mutant 23: an error reaching here looking like absence.
        let root = scratch("undetected");
        let closed = root.join("closed");
        fs::create_dir_all(closed.join("bin")).expect("a scratch prefix");
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o000)).expect("chmod");
        assert_eq!(
            undetected_site(std::slice::from_ref(&closed), "bin/tool"),
            Some(LockSite::Unresolved)
        );
        assert_eq!(undetected_site(&[root.join("missing")], "bin/tool"), None);
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o755)).expect("chmod");
        cleanup(&root);
    }

    #[test]
    fn a_named_lock_files_directory_swapped_before_the_open_is_not_determined() {
        // Mutant 16: for a named file this re-check is the only one after the
        // open. Without it the emptied replacement would read as nothing held.
        let root = scratch("named-swapped");
        let locks = root.join("locks");
        fs::create_dir(&locks).expect("a lock directory");
        touch(&locks.join("x.lock"));
        let moved = root.join("moved");
        let answer = sample_site(&file_site(&locks, "x.lock"), &|_| {
            fs::rename(&locks, &moved).expect("a swap");
            fs::create_dir(&locks).expect("a replacement");
        });
        assert_eq!(answer, Answer::NotDetermined(UndeterminedCause::Changed));
        cleanup(&root);
    }

    #[test]
    fn an_empty_directory_swapped_after_its_listing_is_not_determined() {
        // Mutant 16b: with nothing to open, the final re-check is the only one.
        let root = scratch("empty-swapped");
        let locks = root.join("locks");
        fs::create_dir(&locks).expect("a lock directory");
        let moved = root.join("moved");
        let answer = sample_site(&LockSite::Directory(locks.clone()), &|_| {
            fs::rename(&locks, &moved).expect("a swap");
            fs::create_dir(&locks).expect("a replacement");
        });
        assert_eq!(answer, Answer::NotDetermined(UndeterminedCause::Changed));
        cleanup(&root);
    }

    #[test]
    fn an_unresolved_site_is_not_determined() {
        assert_eq!(
            sample_site(&LockSite::Unresolved, &|_| {}),
            Answer::NotDetermined(UndeterminedCause::Unresolved)
        );
    }

    #[test]
    fn held_wins_and_keeps_the_undetermined_then_not_determined_then_none_observed() {
        let a = ProviderId::new("a");
        let b = ProviderId::new("b");
        let unresolved = Answer::NotDetermined(UndeterminedCause::Unresolved);
        let a_unresolved = BTreeMap::from([(a.clone(), UndeterminedCause::Unresolved)]);
        assert_eq!(
            combine([(a.clone(), unresolved), (b.clone(), Answer::Held)].into_iter()),
            ConcurrentProviderActivity::Observed {
                providers: BTreeSet::from([b.clone()]),
                undetermined: a_unresolved.clone(),
            }
        );
        assert_eq!(
            combine([(a.clone(), unresolved), (b.clone(), Answer::NotHeld)].into_iter()),
            ConcurrentProviderActivity::NotDetermined {
                undetermined: a_unresolved,
            }
        );
        assert_eq!(
            combine([(a, Answer::NotHeld), (b, Answer::NotHeld)].into_iter()),
            ConcurrentProviderActivity::NoneObserved
        );
        assert_eq!(
            combine(std::iter::empty()),
            ConcurrentProviderActivity::NoneObserved
        );
    }

    #[test]
    fn the_probe_samples_every_site_and_an_unknowable_one_none() {
        let root = scratch("probe");
        let holder = touch(&root.join("x.lock"));
        holder.lock().expect("locks");
        let probe = SystemLockProbe::over(vec![
            (ProviderId::new("a"), LockSite::Directory(root.clone())),
            (ProviderId::new("b"), LockSite::Unresolved),
        ]);
        assert_eq!(
            probe.sample(),
            ConcurrentProviderActivity::Observed {
                providers: BTreeSet::from([ProviderId::new("a")]),
                undetermined: BTreeMap::from([(
                    ProviderId::new("b"),
                    UndeterminedCause::Unresolved
                )]),
            }
        );
        assert_eq!(
            SystemLockProbe::unknowable().sample(),
            ConcurrentProviderActivity::Unknowable
        );
        cleanup(&root);
    }

    #[test]
    fn a_held_lock_outranks_a_stray_entry_or_a_failed_open_in_its_directory() {
        // The refusal must say "held", not send someone to tidy the directory
        // while a provider is running (security review of G-2, F4).
        let root = scratch("held-outranks");
        let holder = touch(&root.join("x.lock"));
        holder.lock().expect("locks");
        fs::create_dir(root.join("stray")).expect("a subdirectory");
        assert_eq!(directory(&root), Answer::Held);
        holder.unlock().expect("unlocks");
        assert_eq!(
            directory(&root),
            Answer::NotDetermined(UndeterminedCause::UnexpectedShape)
        );
        cleanup(&root);
    }

    #[test]
    fn a_directory_swapped_while_a_lock_is_held_in_it_is_not_determined() {
        let root = scratch("held-swapped");
        let locks = root.join("locks");
        fs::create_dir(&locks).expect("a lock directory");
        let holder = touch(&locks.join("x.lock"));
        holder.lock().expect("locks");
        let moved = root.join("moved");
        let answer = sample_site(&LockSite::Directory(locks.clone()), &|_| {
            fs::rename(&locks, &moved).expect("a swap");
            fs::create_dir(&locks).expect("a replacement");
        });
        assert_eq!(answer, Answer::NotDetermined(UndeterminedCause::Changed));
        cleanup(&root);
    }

    #[test]
    fn a_socket_entry_is_not_determined() {
        // A short path: a Unix socket's name is limited to about a hundred
        // bytes, and a temporary directory's can use most of that.
        let root = PathBuf::from(format!("/tmp/mpb-probe-{}-s", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch dir");
        let _socket = std::os::unix::net::UnixListener::bind(root.join("s")).expect("a socket");
        assert_eq!(
            directory(&root),
            Answer::NotDetermined(UndeterminedCause::UnexpectedShape)
        );
        cleanup(&root);
    }

    #[test]
    fn a_file_on_another_device_than_its_directory_is_not_determined() {
        let root = scratch("device");
        let path = root.join("x.lock");
        touch(&path);
        let file = File::open(&path).expect("opens");
        let meta = file.metadata().expect("metadata");
        let same = Identity {
            dev: meta.dev(),
            ino: 0,
        };
        assert_eq!(query(&file, same), Answer::NotHeld);
        let other = Identity {
            dev: meta.dev().wrapping_add(1),
            ino: 0,
        };
        assert_eq!(
            query(&file, other),
            Answer::NotDetermined(UndeterminedCause::UnexpectedShape)
        );
        cleanup(&root);
    }

    #[test]
    fn a_listing_item_that_cannot_be_read_is_a_surprise_and_ends_the_listing() {
        let failure = || std::io::Error::other("an unreadable entry");
        let listing = read_listing([
            Ok((OsString::from("a"), Ok(true))),
            Err(failure()),
            Ok((OsString::from("b"), Ok(true))),
        ]);
        assert_eq!(listing.surprise, Some(UndeterminedCause::Unreadable));
        assert_eq!(listing.names, [OsString::from("a")]);
        let listing = read_listing([Ok((OsString::from("a"), Err(failure())))]);
        assert_eq!(listing.surprise, Some(UndeterminedCause::Unreadable));
        assert!(listing.names.is_empty());
    }

    #[test]
    fn an_entry_that_is_not_a_regular_file_is_never_listed_to_be_opened() {
        // Mutant 17: without this filter the open and the `fstat` still answer
        // "not determined", so only the list shows it was never opened.
        let listing = read_listing([
            Ok((OsString::from("device"), Ok(false))),
            Ok((OsString::from("a"), Ok(true))),
        ]);
        assert_eq!(listing.names, [OsString::from("a")]);
        assert_eq!(listing.surprise, Some(UndeterminedCause::UnexpectedShape));
    }

    #[test]
    fn the_cause_does_not_depend_on_the_order_of_the_listing() {
        let forward = read_listing([
            Ok((OsString::from("a"), Ok(false))),
            Ok((OsString::from("b"), Err(std::io::Error::other("x")))),
        ]);
        let backward = read_listing([
            Ok((OsString::from("b"), Err(std::io::Error::other("x")))),
            Ok((OsString::from("a"), Ok(false))),
        ]);
        assert_eq!(forward.surprise, backward.surprise);
    }

    #[test]
    fn invariant_06_the_lock_probe_needs_no_elevation() {
        // A `0444` file, locked and sampled as the test user: a read-only open
        // is all the probe needs (R-17 finding 4).
        let root = scratch("no-elevation");
        let path = root.join("x.lock");
        touch(&path);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).expect("chmod");
        let holder = File::open(&path).expect("a read-only open of a 0444 file");
        assert_eq!(directory(&root), Answer::NotHeld);
        holder.lock().expect("a flock needs no write access");
        assert_eq!(directory(&root), Answer::Held);
        assert_eq!(
            sample_site(&file_site(&root, "x.lock"), &|_| {}),
            Answer::Held
        );
        drop(holder);
        cleanup(&root);
    }

    /// Every entry under `dir`, with its kind, length, mode and modification
    /// time, recursively, in order.
    fn inventory(dir: &Path) -> Vec<(PathBuf, bool, u64, u32, i64, i64)> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for entry in fs::read_dir(&d).expect("lists") {
                let path = entry.expect("an entry").path();
                let meta = fs::symlink_metadata(&path).expect("stats");
                if meta.is_dir() {
                    stack.push(path.clone());
                }
                out.push((
                    path,
                    meta.is_dir(),
                    meta.len(),
                    meta.mode(),
                    meta.mtime(),
                    meta.mtime_nsec(),
                ));
            }
        }
        out.sort();
        out
    }

    #[test]
    fn invariant_05_the_lock_probe_creates_nothing() {
        // A lock directory and a named lock file's directory, the named file
        // missing: after a sample of each, and a sample answering "held", nothing
        // was created, written, truncated or removed.
        let root = scratch("creates-nothing");
        let locks = root.join("locks");
        let registry = root.join("registry");
        fs::create_dir(&locks).expect("dir");
        fs::create_dir(&registry).expect("dir");
        fs::write(locks.join("a.lock"), b"kept as it is").expect("writes");
        touch(&locks.join("update"));
        let before = inventory(&root);
        let probe = SystemLockProbe::over(vec![
            (ProviderId::new("a"), LockSite::Directory(locks.clone())),
            (ProviderId::new("b"), file_site(&registry, "absent.lock")),
            (
                ProviderId::new("c"),
                LockSite::Directory(root.join("absent")),
            ),
        ]);
        assert_eq!(probe.sample(), ConcurrentProviderActivity::NoneObserved);
        assert_eq!(inventory(&root), before);
        let holder = File::open(locks.join("update")).expect("opens");
        holder.lock().expect("locks");
        assert!(matches!(
            probe.sample(),
            ConcurrentProviderActivity::Observed { .. }
        ));
        holder.unlock().expect("unlocks");
        assert_eq!(inventory(&root), before);
        assert!(!registry.join("absent.lock").exists());
        assert!(!root.join("absent").exists());
        cleanup(&root);
    }
}
