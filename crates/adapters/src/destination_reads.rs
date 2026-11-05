// SPDX-License-Identifier: Apache-2.0

//! The filesystem half of the destination checks, which both providers share
//! (work order 16 part H; R-25, R-27): a checked file list, `stat`s of it, and
//! the walk of what its files load. Everything here observes; `application`
//! judges. No provider is named: each adapter hands in its list and its roots.
//!
//! **Nothing is `stat`ed before** the caller's `FilesystemScope` test, the
//! parser, a length bound and a component-wise `is_under` ([`checked_list`]). A
//! path a provider printed reaches `lstat`, `stat` and the open below, and
//! nothing else: never an argv, never a join.
//!
//! **No clock and no cancellation:** the walk's bounds are numbers, because an
//! adapter reads no clock and no token is wired to one.

use crate::listing;
use crate::macho::{self, Classified, ExaminedType, LoadKind, Shebang, SlicePlan, SliceRange};
use crate::persistence::bounded_read::{open_flags, OpenPolicy};
use crate::process::{Invocation, MAX_PATH_LEN};
use macportsbrew_domain::{
    normalise, DomainError, LinkageFacts, PrefixClass, PrefixTable, UncheckedCause, SYSTEM_PATH,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// The most files one check reads the head of. R-27's worst closure is 340
/// Mach-O files; this counts the package's own list as well, so a package with
/// more regular files than this answers `Unchecked` by design.
pub const MAX_WALK_FILES: usize = 4096;

/// The most bytes one check reads.
pub const MAX_WALK_BYTES: u64 = 256 * 1024 * 1024;

/// The most references and candidates one check considers, repeats included. A
/// crafted file can multiply `LC_RPATH` entries by `@rpath` names, and each new
/// candidate is a filesystem walk of its ancestors.
pub const MAX_WALK_REFERENCES: usize = 65_536;

/// The longest path the system opens, its terminator included. A path this long
/// names nothing, so it is unresolved before any filesystem call.
#[cfg(target_os = "macos")]
const PATH_MAX: usize = 1024;
#[cfg(not(target_os = "macos"))]
const PATH_MAX: usize = 4096;

/// What `man dyld` (read 2026-11-04) names for the system's own libraries: full paths
/// under `/usr/lib`, and `/System/Library/Frameworks` on its fallback list; the
/// private frameworks are their sibling. **An allow-list:** anything else answers
/// `Unchecked`, so that a firmlink a later macOS adds opens nothing (`/System/Volumes/Data`
/// is the whole writable volume, `canonicalize` keeps that spelling, and dyld honours it).
const SYSTEM_ROOTS: [&str; 3] = [
    "/usr/lib",
    "/System/Library/Frameworks",
    "/System/Library/PrivateFrameworks",
];

/// Where dyld looks after a name resolves to nothing, from
/// `data/policy/dyld-fallbacks.toml`, whose comment quotes `man dyld`.
#[derive(Clone, Debug)]
pub struct Fallbacks {
    /// Searched for a dylib's leaf.
    pub libraries: Vec<PathBuf>,
    /// Searched for a framework, from its `.framework` component on.
    pub frameworks: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FallbacksFile {
    source: String,
    read_on: String,
    libraries: Vec<PathBuf>,
    frameworks: Vec<PathBuf>,
}

impl Fallbacks {
    /// The defaults `man dyld` states.
    ///
    /// # Panics
    /// If the embedded file is refused, which a test holds.
    #[must_use]
    pub fn dyld() -> Self {
        Self::parse(include_str!("../../../data/policy/dyld-fallbacks.toml"))
            .unwrap_or_else(|why| panic!("data/policy/dyld-fallbacks.toml: {why}"))
    }

    /// An empty list would silently stop the look, so it is refused, and so is a file
    /// with no provenance (INV-20).
    fn parse(text: &str) -> Result<Self, String> {
        let file: FallbacksFile = toml::from_str(text).map_err(|e| e.to_string())?;
        if file.source.trim().is_empty() {
            return Err("no source".to_owned());
        }
        let date = file.read_on.as_bytes();
        let shaped = date.len() == 10
            && date.iter().enumerate().all(|(at, byte)| {
                if at == 4 || at == 7 {
                    *byte == b'-'
                } else {
                    byte.is_ascii_digit()
                }
            });
        if !shaped {
            return Err(format!("read_on is not YYYY-MM-DD: {:?}", file.read_on));
        }
        if file.libraries.is_empty() || file.frameworks.is_empty() {
            return Err("an empty list stops the look".to_owned());
        }
        if !file
            .libraries
            .iter()
            .chain(&file.frameworks)
            .all(|place| place.is_absolute())
        {
            return Err("a relative location means nothing to dyld's search".to_owned());
        }
        Ok(Self {
            libraries: file.libraries,
            frameworks: file.frameworks,
        })
    }
}

const ENOENT: i32 = 2;
const ENOTDIR: i32 = 20;
#[cfg(target_os = "macos")]
const ELOOP: i32 = 62;
#[cfg(not(target_os = "macos"))]
const ELOOP: i32 = 40;

/// The standard output of a list read, or why it says nothing. Over the cap is
/// its own answer: a truncated line must never reach a `stat` (R-25 finding 9).
pub(crate) fn list_stdout(run: Result<Invocation, DomainError>) -> Result<String, UncheckedCause> {
    let invocation = run.map_err(|_| UncheckedCause::ReadFailed)?;
    if invocation.truncated {
        return Err(UncheckedCause::ListOverCap);
    }
    invocation
        .require_success()
        .map(|i| i.stdout.clone())
        .map_err(|_| UncheckedCause::ReadFailed)
}

/// What a listed path outside the root means, which is the provider's own fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outside {
    /// Nothing legitimate produces one: the list is not of this package.
    IsAnotherShape,
    /// Legitimate (an application bundle), and not tested (R-25 finding 7).
    IsNotTested,
}

/// A parsed, bounded file list: what may be `stat`ed. Only [`checked_list`] builds
/// one, and [`stat_counts`] takes nothing else, so the order is the compiler's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedList {
    paths: Vec<PathBuf>,
    outside: usize,
}

impl CheckedList {
    /// Listed paths under the root.
    #[must_use]
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// How many listed paths were outside it, not tested.
    #[must_use]
    pub fn outside(&self) -> usize {
        self.outside
    }
}

/// The parser, the length bound and `is_under`, in that order, before anything
/// touches the disk.
pub fn checked_list(
    stdout: &str,
    indent: &str,
    read: &str,
    root: &Path,
    outside: Outside,
) -> Result<CheckedList, UncheckedCause> {
    let parsed = listing::parse(stdout, indent, read).map_err(|_| UncheckedCause::ReadFailed)?;
    let mut list = CheckedList {
        paths: Vec::with_capacity(parsed.len()),
        outside: 0,
    };
    for path in parsed {
        if path.as_os_str().len() > MAX_PATH_LEN {
            return Err(UncheckedCause::ReadFailed);
        }
        if listing::is_under(&path, root) {
            list.paths.push(path);
        } else if outside == Outside::IsAnotherShape {
            return Err(UncheckedCause::ReadFailed);
        } else {
            list.outside += 1;
        }
    }
    if list.paths.is_empty() {
        return Err(UncheckedCause::ListsOutsidePrefix);
    }
    Ok(list)
}

/// What `stat`ing a list found (R-25 finding 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatCounts {
    /// `lstat` says it does not exist.
    pub missing: usize,
    /// `lstat` succeeds and `stat` says it leads nowhere.
    pub dangling: usize,
    /// Any other errno, which proves nothing about the file.
    pub failed: usize,
}

/// `lstat`, then `stat`, of every path of a checked list. Never opens one.
#[must_use]
pub fn stat_counts(list: &CheckedList) -> StatCounts {
    let mut counts = StatCounts::default();
    for path in list.paths() {
        match std::fs::symlink_metadata(path) {
            Err(error) if errno_in(&error, &[ENOENT, ENOTDIR]) => counts.missing += 1,
            Err(_) => counts.failed += 1,
            Ok(_) => match std::fs::metadata(path) {
                Ok(_) => {}
                Err(error) if errno_in(&error, &[ENOENT, ENOTDIR, ELOOP]) => {
                    counts.dangling += 1;
                }
                Err(_) => counts.failed += 1,
            },
        }
    }
    counts
}

fn errno_in(error: &io::Error, among: &[i32]) -> bool {
    error.raw_os_error().is_some_and(|e| among.contains(&e))
}

/// Whether a resolution failed because there is nothing there. Any other errno
/// proves nothing about the file.
#[must_use]
pub fn is_gone(error: &io::Error) -> bool {
    errno_in(error, &[ENOENT, ENOTDIR, ELOOP])
}

/// The placed entry of `leaf`: `bin`, then `sbin`, the first that exists,
/// `lstat`ed only (R-26 finding 18). `Err` when looking failed for a reason that
/// proves nothing.
pub fn placed_entry(prefix: &Path, leaf: &str) -> Result<Option<PathBuf>, UncheckedCause> {
    for directory in ["bin", "sbin"] {
        let entry = prefix.join(directory).join(leaf);
        match std::fs::symlink_metadata(&entry) {
            Ok(_) => return Ok(Some(entry)),
            Err(error) if errno_in(&error, &[ENOENT, ENOTDIR]) => {}
            Err(_) => return Err(UncheckedCause::StatFailed),
        }
    }
    Ok(None)
}

/// Where a link's first hop points, lexically, when `entry` is a link.
pub(crate) fn first_hop(entry: &Path) -> Option<PathBuf> {
    let target = std::fs::read_link(entry).ok()?;
    let parent = entry.parent()?;
    let normal = normalise(
        Some(parent.as_os_str().as_bytes()),
        target.as_os_str().as_bytes(),
    )?;
    Some(PathBuf::from(OsStr::from_bytes(&normal)))
}

/// What a walk cost, so a test can hold the dedupe (item: bounded work).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WalkWork {
    /// References and candidates considered, repeats included.
    pub operations: usize,
    /// Those that reached the filesystem.
    pub resolutions: usize,
    /// Bytes read of the files examined.
    pub bytes: u64,
}

/// Examines a package: every file of `list` by magic, its links by `lstat` and
/// by where they finally lead, and a bounded walk of the destination's own files
/// that its load commands reach (R-27 findings 7, 8, 14). `executable_dirs` says
/// which entries are the package's executable list.
#[must_use]
pub fn examine(
    list: &CheckedList,
    executable_dirs: &[PathBuf],
    prefixes: &PrefixTable,
) -> LinkageFacts {
    examine_counted(list, executable_dirs, prefixes).0
}

/// [`examine`], and what it cost.
#[must_use]
pub fn examine_counted(
    list: &CheckedList,
    executable_dirs: &[PathBuf],
    prefixes: &PrefixTable,
) -> (LinkageFacts, WalkWork) {
    examine_with(list, executable_dirs, prefixes, &Fallbacks::dyld())
}

/// [`examine_counted`] with the fallback locations given, so a test need not write
/// into `/usr/local`.
#[must_use]
pub fn examine_with(
    list: &CheckedList,
    executable_dirs: &[PathBuf],
    prefixes: &PrefixTable,
    fallbacks: &Fallbacks,
) -> (LinkageFacts, WalkWork) {
    let mut identities = HashMap::new();
    for (path, class) in prefixes.paths() {
        if let Ok(metadata) = std::fs::metadata(OsStr::from_bytes(path)) {
            // First added wins: the registry adds what is strongest first.
            identities
                .entry((metadata.dev(), metadata.ino()))
                .or_insert(class);
        }
    }
    let mut on_list: HashSet<PathBuf> = executable_dirs.iter().cloned().collect();
    // `bin -> libexec/bin`: what is listed may be under the directory's real name.
    on_list.extend(
        executable_dirs
            .iter()
            .filter_map(|dir| std::fs::canonicalize(dir).ok()),
    );
    let mut walk = Walk {
        prefixes,
        identities,
        fallbacks,
        fallback_names: HashSet::new(),
        executable_dirs: on_list,
        parents: HashMap::new(),
        facts: LinkageFacts::default(),
        work: WalkWork::default(),
        seen: HashMap::new(),
        visited: HashMap::new(),
        files: 0,
        bytes: 0,
        queue: Vec::new(),
        queued: HashSet::new(),
        rpaths: Vec::new(),
        rpath_keys: HashSet::new(),
        names: Vec::new(),
        name_keys: HashSet::new(),
        loadable: HashSet::new(),
        pending: Vec::new(),
        system_files_read: HashSet::new(),
    };
    for path in list.paths() {
        if walk.facts.walk_over_bound {
            break;
        }
        walk.own_entry(path);
    }
    // A name is tried against every rpath the walk has seen, and a candidate that
    // exists can join the walk and bring more. Only when every candidate has been read
    // can it be said which dyld would take, and what that leaves for the fallback brings
    // more again: round until nothing new is left.
    loop {
        while let Some(next) = walk.queue.pop() {
            if walk.facts.walk_over_bound {
                break;
            }
            walk.read_file(&next, Reached::ByLoadCommand);
        }
        if walk.facts.walk_over_bound {
            break;
        }
        if !walk.try_rpath_names() && !walk.decide_pending_loads() {
            break;
        }
    }
    if !walk.facts.walk_over_bound {
        walk.settle_rpath_names();
    }
    walk.work.bytes = walk.bytes;
    (walk.facts, walk.work)
}

/// A reference as written: `raw` is the text joined and untouched, `..` intact,
/// and `normal` the same with dots collapsed. They name one file unless a
/// directory link lies under a `..`, and that is why both are kept.
#[derive(Clone)]
struct Spelled {
    raw: Vec<u8>,
    normal: Vec<u8>,
}

/// `None` for what the file alone cannot name, and for a joined path over
/// [`PATH_MAX`], which names nothing: before any filesystem call, any `seen` entry,
/// or the work of normalising it.
fn spelled(base: Option<&[u8]>, path: &[u8]) -> Option<Spelled> {
    let joined = if path.first() == Some(&b'/') {
        path.len()
    } else {
        base.map_or(path.len(), |b| b.len() + 1 + path.len())
    };
    if joined >= PATH_MAX {
        return None;
    }
    let normal = normalise(base, path)?;
    let raw = if path.first() == Some(&b'/') {
        path.to_vec()
    } else {
        let mut joined = base?.to_vec();
        joined.push(b'/');
        joined.extend_from_slice(path);
        joined
    };
    Some(Spelled { raw, normal })
}

fn has_parent_component(path: &[u8]) -> bool {
    path.split(|b| *b == b'/').any(|c| c == b"..")
}

/// Where a reference leads, once every spelling of it has been measured.
#[derive(Clone, Debug)]
struct Seen {
    /// By where it resolves: the canonical path, or by inode.
    class: Option<PrefixClass>,
    /// `None` for something that does not exist.
    canonical: Option<PathBuf>,
    /// Device and inode of what `canonical` names.
    identity: Option<(u64, u64)>,
}

/// How a file came to be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reached {
    /// An entry of the package's list, off its executable list.
    Listed,
    /// An entry of its executable list, or the target of a link in it.
    OnExecutableList,
    /// A load command led here.
    ByLoadCommand,
}

/// What a path in a shebang line is to the script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Read {
    /// What the script runs.
    Interpreter,
    /// Anything else a word names.
    Word,
}

/// An `@rpath` name a file of the walk loads, and how far it was tried.
struct RpathName {
    rest: Vec<u8>,
    /// How many of the walk's rpaths it was tried against.
    tried: usize,
    /// A candidate lies under the system's roots, with no file on disk.
    in_system: bool,
}

/// One load, waiting to be judged once its candidates have been read: what dyld would
/// try for this file alone. Another file's rpath does not help it, since the run path
/// list runs up one chain and not across siblings.
struct PendingLoad {
    /// What dyld's fallback looks for, if none of the candidates can be taken.
    name: Vec<u8>,
    candidates: Vec<Spelled>,
}

struct Walk<'a> {
    prefixes: &'a PrefixTable,
    /// Each prefix by `(dev, ino)`: a firmlink or a Unicode fold spells a prefix
    /// no string comparison sees, and the inode is the same.
    identities: HashMap<(u64, u64), PrefixClass>,
    fallbacks: &'a Fallbacks,
    /// The names whose fallback locations were already looked at.
    fallback_names: HashSet<Vec<u8>>,
    /// The executable directories, as given and canonically.
    executable_dirs: HashSet<PathBuf>,
    /// Each parent directory asked about, and whether it is one of them.
    parents: HashMap<PathBuf, bool>,
    facts: LinkageFacts,
    work: WalkWork,
    seen: HashMap<Vec<u8>, Seen>,
    /// Each file read, and whether its interpreter is found at run time, so a
    /// second name for it on the executable list still counts.
    visited: HashMap<(u64, u64), bool>,
    files: usize,
    bytes: u64,
    queue: Vec<PathBuf>,
    queued: HashSet<PathBuf>,
    /// Every rpath the walk has seen. dyld's run path list belongs to the chain of
    /// loaders, not to one file, and this is as much of that chain as the walk holds.
    rpaths: Vec<Spelled>,
    rpath_keys: HashSet<Vec<u8>>,
    names: Vec<RpathName>,
    name_keys: HashSet<Vec<u8>>,
    /// Each file a read showed a slice dyld could take for a dylib in this build.
    loadable: HashSet<(u64, u64)>,
    pending: Vec<PendingLoad>,
    /// Each file under the system's roots whose head `can_be_taken` read.
    system_files_read: HashSet<(u64, u64)>,
}

impl Walk<'_> {
    /// Whether `path` sits in an executable directory, by either of its names.
    fn is_on_list(&mut self, path: &Path) -> bool {
        let Some(parent) = path.parent() else {
            return false;
        };
        if self.executable_dirs.contains(parent) {
            return true;
        }
        if let Some(known) = self.parents.get(parent) {
            return *known;
        }
        let answer =
            std::fs::canonicalize(parent).is_ok_and(|real| self.executable_dirs.contains(&real));
        self.parents.insert(parent.to_path_buf(), answer);
        answer
    }

    fn own_entry(&mut self, path: &Path) {
        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            self.facts.unreadable_file = true;
            return;
        };
        let on_list = self.is_on_list(path);
        if metadata.is_symlink() {
            self.own_link(path, on_list);
            return;
        }
        if !metadata.is_file() {
            return;
        }
        // `O_NOFOLLOW` guards the last component only, so a directory link
        // inside the destination must not make another prefix's file read as
        // the destination's own: the canonical path is what is tested and opened.
        let Some(seen) =
            spelled(None, path.as_os_str().as_bytes()).and_then(|written| self.resolve(&written))
        else {
            self.facts.unreadable_file |= !self.facts.walk_over_bound;
            return;
        };
        let Some(canonical) = seen.canonical else {
            self.facts.unreadable_file = true;
            return;
        };
        match seen.class {
            Some(PrefixClass::Destination) => {
                let how = if on_list {
                    Reached::OnExecutableList
                } else {
                    Reached::Listed
                };
                self.read_file(&canonical, how);
            }
            // A listed file nobody can place was examined by nobody, and that
            // is never a clean answer. A relocated store answers this, accepted.
            None => self.facts.file_not_examined = true,
            Some(PrefixClass::DetectedOther | PrefixClass::UndetectedCandidate) => {}
        }
    }

    /// A link is judged by its whole resolution and is not opened to do so. Only
    /// a link on the executable list is followed to read, into the destination,
    /// because what it leads to is what runs under that name.
    fn own_link(&mut self, path: &Path, on_list: bool) {
        let Ok(target) = std::fs::read_link(path) else {
            self.facts.unreadable_file = true;
            return;
        };
        let parent = path
            .parent()
            .map_or(&b"/"[..], |p| p.as_os_str().as_bytes());
        let Some(hop) = spelled(Some(parent), target.as_os_str().as_bytes()) else {
            self.facts.unreadable_file = true;
            return;
        };
        let Some(seen) = self.resolve(&hop) else {
            return;
        };
        let Some(canonical) = seen.canonical else {
            // Dangling: the integrity check names it, and its spelling was noted.
            return;
        };
        match seen.class {
            Some(PrefixClass::Destination) if on_list => {
                self.read_file(&canonical, Reached::OnExecutableList);
            }
            None => self.facts.file_not_examined = true,
            Some(_) => {}
        }
    }

    /// Records what a class means for the check.
    fn note(&mut self, class: Option<PrefixClass>) {
        match class {
            Some(PrefixClass::DetectedOther) => self.facts.links_detected_provider = true,
            Some(PrefixClass::UndetectedCandidate) => self.facts.under_undetected_prefix = true,
            Some(PrefixClass::Destination) | None => {}
        }
    }

    /// The deepest existing ancestor of `path` that **is** a prefix, by inode.
    fn class_by_identity(&self, path: &Path) -> Option<PrefixClass> {
        path.ancestors()
            .filter(|ancestor| ancestor.parent().is_some())
            .find_map(|ancestor| {
                let metadata = std::fs::metadata(ancestor).ok()?;
                self.identities
                    .get(&(metadata.dev(), metadata.ino()))
                    .copied()
            })
    }

    /// Whether `path` is the system's own, by where it really is: `/usr/lib/cron` is a
    /// link out of the sealed volume, and an absent file's text says nothing of that.
    fn is_system(&mut self, path: &Path) -> bool {
        let real = match std::fs::canonicalize(path) {
            Ok(real) => real,
            Err(_) => self.through_existing_ancestor(path),
        };
        is_under_system_root(&real)
    }

    /// Counts these operations, and says whether the bound still holds.
    fn count(&mut self, operations: usize) -> bool {
        self.work.operations = self.work.operations.saturating_add(operations);
        if self.work.operations > MAX_WALK_REFERENCES {
            self.facts.walk_over_bound = true;
            return false;
        }
        true
    }

    /// `path`, which does not exist, as its deepest existing ancestor really is:
    /// an absent file through a link below a prefix is still under that prefix.
    fn through_existing_ancestor(&mut self, path: &Path) -> PathBuf {
        let mut failed = 0;
        let found = path.ancestors().skip(1).find_map(|ancestor| {
            let real = std::fs::canonicalize(ancestor).ok();
            failed += usize::from(real.is_none());
            real.map(|real| (real, ancestor))
        });
        self.count(failed);
        match found {
            Some((real, ancestor)) => path
                .strip_prefix(ancestor)
                .map_or_else(|_| path.to_path_buf(), |rest| real.join(rest)),
            None => path.to_path_buf(),
        }
    }

    /// Canonicalises what exists, and measures the spelling, the canonical path
    /// and the inodes of its ancestors, noting every answer. A candidate seen
    /// before costs a count and no filesystem access. `None` once the bound is
    /// passed.
    fn resolve(&mut self, written: &Spelled) -> Option<Seen> {
        if !self.count(1) {
            return None;
        }
        let parent = has_parent_component(&written.raw);
        let key = seen_key(written);
        if let Some(seen) = self.seen.get(key) {
            return Some(seen.clone());
        }
        self.work.resolutions += 1;
        self.note(self.prefixes.class_of(&written.normal));
        let normal = Path::new(OsStr::from_bytes(&written.normal));
        let real = std::fs::canonicalize(normal);
        if parent {
            // Precedent: `is_safe_path` refuses `..` for this reason. A collapse is
            // only text, and a directory link under the `..` makes it another file.
            let raw = Path::new(OsStr::from_bytes(&written.raw));
            let raw_real = std::fs::canonicalize(raw);
            match (&real, &raw_real) {
                (Ok(a), Ok(b)) if a == b => {}
                (Err(a), Err(b)) if is_gone(a) && is_gone(b) => {}
                _ => self.facts.spellings_disagree = true,
            }
            if let Ok(other) = &raw_real {
                self.note(self.prefixes.class_of(other.as_os_str().as_bytes()));
                self.note(self.class_by_identity(other));
            }
        }
        let canonical = match real {
            Ok(path) => Some(path),
            Err(error) => {
                self.facts.unreadable_file |= !is_gone(&error);
                None
            }
        };
        let measured = match &canonical {
            Some(path) => path.clone(),
            None => self.through_existing_ancestor(normal),
        };
        // Each ancestor is a `stat`: a deep tree must cost what it costs.
        self.count(measured.ancestors().count());
        let by_text = self.prefixes.class_of(measured.as_os_str().as_bytes());
        let by_inode = self.class_by_identity(&measured);
        self.note(by_text);
        self.note(by_inode);
        let identity = canonical
            .as_ref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| (metadata.dev(), metadata.ino()));
        let seen = Seen {
            class: by_inode.or(by_text),
            canonical,
            identity,
        };
        let key = key.to_vec();
        self.seen.insert(key, seen.clone());
        Some(seen)
    }

    /// `path` is canonical. One open, every test on the handle, and windows
    /// rather than the file.
    fn read_file(&mut self, path: &Path, how: Reached) {
        let opened = OpenOptions::new()
            .read(true)
            .custom_flags(open_flags(OpenPolicy::RefuseSymlink))
            .open(path);
        let Ok(file) = opened else {
            self.facts.unreadable_file = true;
            return;
        };
        let Ok(metadata) = file.metadata() else {
            self.facts.unreadable_file = true;
            return;
        };
        if !metadata.is_file() {
            // A listed FIFO has no load commands to hide, but a load command that
            // names something it cannot load is not therefore clean.
            self.facts.file_not_examined |= how == Reached::ByLoadCommand;
            return;
        }
        let identity = (metadata.dev(), metadata.ino());
        if let Some(indeterminate) = self.visited.get(&identity) {
            // Another name for a file already read: a hardlink, or a link on the
            // executable list to a script read earlier under its own name.
            self.facts.env_shebang_on_list |= *indeterminate && how == Reached::OnExecutableList;
            return;
        }
        self.visited.insert(identity, false);
        self.files += 1;
        if self.files > MAX_WALK_FILES {
            self.facts.walk_over_bound = true;
            return;
        }
        let length = metadata.len();
        let Some(head) = self.window(&file, 0, macho::HEAD_BYTES as u64, length) else {
            return;
        };
        match macho::classify(&head, length) {
            Err(_) => self.facts.malformed_file = true,
            Ok(Classified::NotObject | Classified::Skipped(_)) => {}
            Ok(Classified::Script(shebang)) => {
                let indeterminate = self.shebang(&shebang, &head);
                self.visited.insert(identity, indeterminate);
                self.facts.env_shebang_on_list |= indeterminate && how == Reached::OnExecutableList;
            }
            Ok(Classified::Thin(range)) => self.slices(&file, path, identity, &[range]),
            Ok(Classified::Fat(ranges)) => self.slices(&file, path, identity, &ranges),
        }
    }

    /// Up to `want` bytes from `offset`, inside `end`, against the byte bound.
    /// `None` when it could not be read, or the bound was passed.
    fn window(&mut self, file: &File, offset: u64, want: u64, end: u64) -> Option<Vec<u8>> {
        let available = end.saturating_sub(offset);
        let length = want.min(available);
        self.bytes = self.bytes.saturating_add(length);
        if self.bytes > MAX_WALK_BYTES {
            self.facts.walk_over_bound = true;
            return None;
        }
        let mut buffer = vec![0u8; usize::try_from(length).ok()?];
        if file.read_exact_at(&mut buffer, offset).is_err() {
            self.facts.unreadable_file = true;
            return None;
        }
        Some(buffer)
    }

    fn slices(&mut self, file: &File, path: &Path, identity: (u64, u64), ranges: &[SliceRange]) {
        let mut loadable = false;
        let mut read: Vec<SliceRange> = Vec::new();
        for range in ranges {
            // A crafted fat table can name one range many times.
            if read.contains(range) {
                continue;
            }
            read.push(*range);
            let end = range.offset.saturating_add(range.size);
            let Some(header) =
                self.window(file, range.offset, macho::SLICE_HEADER_BYTES as u64, end)
            else {
                return;
            };
            match macho::plan_slice(&header, range.size) {
                Err(_) => self.facts.malformed_file = true,
                Ok(SlicePlan::Skipped(_)) => {}
                Ok(SlicePlan::Window { len }) => {
                    let Some(commands) = self.window(file, range.offset, len, end) else {
                        return;
                    };
                    match macho::read_slice(&commands) {
                        Err(_) => self.facts.malformed_file = true,
                        Ok(refs) => {
                            loadable = true;
                            if refs.is_dylib_for_this_build() {
                                self.loadable.insert(identity);
                            }
                            self.references(path, &refs);
                        }
                    }
                }
            }
        }
        if loadable {
            self.facts.loadable_files += 1;
        }
    }

    /// Answers whether what the script runs is found at run time. The interpreter,
    /// and an absolute word of `env`, count as a load command does. Any other absolute
    /// path in a word counts by where it is, and is read if it is a regular file of the
    /// destination; a directory on a search path is not a program, and says nothing.
    /// What a script's body does is not read.
    ///
    /// A word that is no flag and does not start at the root is found by `PATH` or from
    /// the directory the script runs in, whatever the interpreter: `nohup python3`,
    /// `nohup bin/x`, `sh ./x`. So there is no list of programs that run others, only
    /// `env`, whose `-Spython3` has no such word.
    fn shebang(&mut self, shebang: &Shebang, head: &[u8]) -> bool {
        let is_env = matches!(shebang, Shebang::Env(_));
        let words = macho::shebang_arguments(head);
        let found_at_run_time = words
            .iter()
            .any(|word| !matches!(word.first(), Some(b'-' | b'/')));
        for word in words {
            if is_env && word.first() == Some(&b'/') {
                self.lexical(word, Read::Interpreter);
            } else {
                self.shebang_word(word);
            }
        }
        match shebang {
            Shebang::Absolute(path) => {
                self.lexical(path, Read::Interpreter);
                found_at_run_time
            }
            // `env`'s own path is still an answer; what it finds is not.
            Shebang::Env(path) => {
                self.lexical(path, Read::Interpreter);
                true
            }
            Shebang::Relative(_) => true,
        }
    }

    /// The program is handed the word whole, so a path holding a CR or a separator is
    /// measured whole, from where the first absolute path in it starts. The pieces are
    /// measured too, since a CRLF line's word ends in a CR the script's author did not
    /// mean, and `PATH=/x:/y` names two. The tail of a relative `bin/x` is no path. A
    /// flag's attached value starts at its first `/`, since `-iS/x` ran `/x` and flags
    /// cannot be told from a relative value in `-cbin/x`, whose tail is then measured
    /// for nothing.
    fn shebang_word(&mut self, word: &[u8]) {
        let mut at = 0;
        let mut whole = None;
        for piece in word.split(|b| matches!(b, b':' | b'=' | b'"' | b'\'' | b'\r')) {
            let start = if at == 0 && piece.first() == Some(&b'-') {
                piece.iter().position(|b| *b == b'/')
            } else if piece.first() == Some(&b'/') {
                Some(0)
            } else {
                None
            };
            if let Some(start) = start {
                whole.get_or_insert(at + start);
                self.lexical(&piece[start..], Read::Word);
            }
            at += piece.len() + 1;
        }
        if let Some(start) = whole {
            self.lexical(&word[start..], Read::Word);
        }
    }

    /// A path a script names. What runs is read as a load command's target would be.
    /// Anywhere else it counts by where it is and is never read, and a real file that
    /// nobody can place, outside the system's own programs and libraries, was examined
    /// by nobody.
    fn lexical(&mut self, path: &[u8], read: Read) {
        let Some(written) = spelled(None, path) else {
            self.facts.rpath_unresolved = true;
            return;
        };
        let Some(seen) = self.resolve(&written) else {
            return;
        };
        match (seen.canonical, seen.class) {
            (Some(canonical), Some(PrefixClass::Destination)) => {
                let runs = read == Read::Interpreter
                    || std::fs::metadata(&canonical).is_ok_and(|m| m.is_file());
                if runs && self.queued.insert(canonical.clone()) {
                    self.queue.push(canonical);
                }
            }
            (Some(canonical), None) => {
                let system_program = SYSTEM_PATH.iter().any(|dir| canonical.starts_with(dir));
                if !system_program && !self.is_system(&canonical) {
                    self.facts.file_not_examined = true;
                }
            }
            _ => {}
        }
    }

    /// Every reference of one slice. An rpath entry is tested whether or not a
    /// name uses it: a loader's entry can serve another file (R-27 finding 4).
    fn references(&mut self, loader: &Path, refs: &macho::SliceRefs) {
        let directory = loader
            .parent()
            .map_or(&b"/"[..], |d| d.as_os_str().as_bytes())
            .to_vec();
        let mut own = Vec::with_capacity(refs.rpaths.len());
        for rpath in &refs.rpaths {
            let Some(written) = expand(rpath, &directory, refs.file_type) else {
                self.facts.rpath_unresolved = true;
                continue;
            };
            let Some(seen) = self.resolve(&written) else {
                return;
            };
            // A real directory nobody can place may serve any file of the chain.
            if let (Some(canonical), None) = (&seen.canonical, seen.class) {
                if !self.is_system(canonical) {
                    self.facts.file_not_examined = true;
                }
            }
            if self.rpath_keys.insert(written.raw.clone()) {
                self.rpaths.push(written.clone());
            }
            own.push(written);
        }
        for reference in &refs.loads {
            if self.facts.walk_over_bound {
                return;
            }
            match reference.kind {
                LoadKind::DyldEnvironment => self.dyld_environment(&reference.name, &directory),
                LoadKind::Load
                | LoadKind::Weak
                | LoadKind::Reexport
                | LoadKind::Lazy
                | LoadKind::Upward => {
                    self.load(&reference.name, &directory, refs.file_type, &own, true);
                }
                // What the kernel starts is no dylib, so no file there needs to be one.
                LoadKind::Dylinker => {
                    self.load(&reference.name, &directory, refs.file_type, &own, false);
                }
            }
        }
    }

    /// `NAME=value`, whose value is a `:`-separated list of paths. What it names is
    /// measured, and the search rules it changes are not modelled, so any such command
    /// leaves the check unable to say what loads.
    fn dyld_environment(&mut self, text: &[u8], directory: &[u8]) {
        self.facts.dyld_environment = true;
        let value = text
            .iter()
            .position(|b| *b == b'=')
            .and_then(|at| text.get(at + 1..))
            .unwrap_or(text);
        for piece in value.split(|b| *b == b':').filter(|p| !p.is_empty()) {
            match expand(piece, directory, ExaminedType::Dylib) {
                Some(written) => {
                    self.resolve(&written);
                }
                None => self.facts.rpath_unresolved = true,
            }
        }
    }

    fn load(
        &mut self,
        name: &[u8],
        directory: &[u8],
        file_type: ExaminedType,
        own: &[Spelled],
        dylib: bool,
    ) {
        if let Some(rest) = name.strip_prefix(b"@rpath/") {
            // dyld joins it, however many slashes it starts with.
            let rest = without_leading_slashes(rest).to_vec();
            let mut candidates = Vec::with_capacity(own.len());
            for rpath in own {
                let Some(candidate) = spelled(Some(&rpath.raw), &rest) else {
                    continue;
                };
                if self.resolve(&candidate).is_none() {
                    return;
                }
                candidates.push(candidate);
            }
            if self.name_keys.insert(rest.clone()) {
                self.names.push(RpathName {
                    rest: rest.clone(),
                    tried: 0,
                    in_system: false,
                });
            }
            self.pending.push(PendingLoad {
                name: rest,
                candidates,
            });
            return;
        }
        match expand(name, directory, file_type) {
            Some(written) => {
                if self.reached(&written, false).is_some() && dylib {
                    self.pending.push(PendingLoad {
                        name: written.normal.clone(),
                        candidates: vec![written],
                    });
                }
            }
            None => self.facts.rpath_unresolved = true,
        }
    }

    /// Tries each `@rpath` name against every rpath it has not met. `false` when there
    /// was nothing new, or the bound was passed.
    fn try_rpath_names(&mut self) -> bool {
        let mut any = false;
        for index in 0..self.names.len() {
            while self.names[index].tried < self.rpaths.len() {
                let at = self.names[index].tried;
                self.names[index].tried += 1;
                any = true;
                let Some(candidate) = spelled(Some(&self.rpaths[at].raw), &self.names[index].rest)
                else {
                    self.facts.rpath_unresolved = true;
                    if !self.count(1) {
                        return false;
                    }
                    continue;
                };
                // One candidate among several need not exist.
                let Some(seen) = self.reached(&candidate, true) else {
                    return false;
                };
                if seen.canonical.is_none() && self.is_system(path_of(&candidate)) {
                    self.names[index].in_system = true;
                }
            }
        }
        any
    }

    /// Whether dyld could take the file `candidate` names, as far as a read can say: a
    /// read of it showed a dylib for this build. A real file under the system's own roots
    /// was not read by the walk, so its head is read here; its dylibs in the shared cache
    /// are no file at all, and never reach this. Asked once every candidate has been
    /// read, and it charges bytes only, since `resolve` charged the rest.
    fn can_be_taken(&mut self, candidate: &Spelled) -> bool {
        let Some(seen) = self.seen.get(seen_key(candidate)).cloned() else {
            return false;
        };
        let Some(identity) = seen.identity else {
            return false;
        };
        if !self.loadable.contains(&identity) {
            if let Some(real) = seen.canonical.filter(|real| is_under_system_root(real)) {
                self.read_system_file(&real, identity);
            }
        }
        self.loadable.contains(&identity)
    }

    /// What a system file says of itself, once: never a fact about the package, so
    /// nothing it holds is walked and nothing about it is flagged.
    fn read_system_file(&mut self, path: &Path, identity: (u64, u64)) {
        if !self.system_files_read.insert(identity) {
            return;
        }
        let Ok(file) = File::open(path) else {
            return;
        };
        let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
            return;
        };
        let Some(head) = self.window(&file, 0, macho::HEAD_BYTES as u64, length) else {
            return;
        };
        let ranges = match macho::classify(&head, length) {
            Ok(Classified::Thin(range)) => vec![range],
            Ok(Classified::Fat(ranges)) => ranges,
            _ => return,
        };
        for range in ranges {
            let end = range.offset.saturating_add(range.size);
            let Some(header) =
                self.window(&file, range.offset, macho::SLICE_HEADER_BYTES as u64, end)
            else {
                return;
            };
            let Ok(SlicePlan::Window { len }) = macho::plan_slice(&header, range.size) else {
                continue;
            };
            let Some(commands) = self.window(&file, range.offset, len, end) else {
                return;
            };
            if macho::read_slice(&commands).is_ok_and(|refs| refs.is_dylib_for_this_build()) {
                self.loadable.insert(identity);
                return;
            }
        }
    }

    /// Every load none of whose own candidates dyld could take may go on to its
    /// fallback, whatever another file's rpath found: `man dyld` builds the run path
    /// list from the images "in the dependency chain that lead to the current dylib
    /// load". The walk holds no load graph, so a name found only through a real
    /// ancestor's rpath also looks here, an over-approximation that can only add
    /// `UNCHECKED`. `true` when that gave the walk something new to read.
    fn decide_pending_loads(&mut self) -> bool {
        for load in std::mem::take(&mut self.pending) {
            let mut taken = false;
            for candidate in &load.candidates {
                taken = taken || self.can_be_taken(candidate);
            }
            if !taken {
                self.fallback(&load.name);
            }
        }
        !self.queue.is_empty()
    }

    /// Zero candidates must not read as clean, and neither must candidates none of
    /// which dyld could take: the name loads from nothing this walk says, whatever a
    /// candidate that cannot be taken is spelled like. The exception is the system's own,
    /// whose dylibs live in the shared cache with no file. A candidate that exists
    /// elsewhere has already been measured, and answers more strongly.
    fn settle_rpath_names(&mut self) {
        for index in 0..self.names.len() {
            let mut found = self.names[index].in_system;
            for at in 0..self.rpaths.len() {
                if found {
                    break;
                }
                if let Some(candidate) =
                    spelled(Some(&self.rpaths[at].raw), &self.names[index].rest)
                {
                    found = self.can_be_taken(&candidate);
                }
            }
            self.facts.rpath_unresolved |= !found;
        }
    }

    /// A name resolved to nothing, so dyld may try its fallback locations. Whether it
    /// does depends on the binary's age, which the reader cannot tell, and which loads
    /// depends on what is in the shared cache, so another provider's file there is a
    /// doubt and never a failure. The destination's own file there joins the walk, and
    /// a real file nobody can place was examined by nobody, as `reached` says.
    fn fallback(&mut self, name: &[u8]) {
        if !self.fallback_names.insert(name.to_vec()) {
            return;
        }
        let path = Path::new(OsStr::from_bytes(name));
        let mut places: Vec<PathBuf> = Vec::new();
        if let Some(leaf) = path.file_name() {
            places.extend(self.fallbacks.libraries.iter().map(|d| d.join(leaf)));
        }
        let framework = path.iter().position(|part| {
            Path::new(part)
                .extension()
                .is_some_and(|e| e == "framework")
        });
        if let Some(from) = framework {
            let suffix: PathBuf = path.iter().skip(from).collect();
            places.extend(self.fallbacks.frameworks.iter().map(|d| d.join(&suffix)));
        }
        for place in places {
            if !self.count(place.ancestors().count()) {
                return;
            }
            let real = match std::fs::canonicalize(&place) {
                Ok(real) => real,
                Err(error) if is_gone(&error) => continue,
                // A location that cannot be looked at proves nothing.
                Err(_) => {
                    self.facts.unreadable_file = true;
                    continue;
                }
            };
            let by_identity = self.class_by_identity(&real);
            let by_text = self.prefixes.class_of(real.as_os_str().as_bytes());
            let classes = [
                self.prefixes.class_of(place.as_os_str().as_bytes()),
                by_text,
                by_identity,
            ];
            self.facts.fallback_under_other_prefix |= classes.iter().any(|class| {
                matches!(
                    class,
                    Some(PrefixClass::DetectedOther | PrefixClass::UndetectedCandidate)
                )
            });
            match by_identity.or(by_text) {
                Some(PrefixClass::Destination) => {
                    if self.queued.insert(real.clone()) {
                        self.queue.push(real);
                    }
                }
                None => {
                    if !self.is_system(&real) {
                        self.facts.file_not_examined = true;
                    }
                }
                Some(_) => {}
            }
        }
    }

    /// A load command reaches `written`: what counts applies to every spelling of
    /// it, and a destination file joins the walk. A destination file that cannot be
    /// examined is not therefore fine, unless it is `optional`, and neither is a
    /// real file in no known prefix outside the system's own roots: it may be
    /// another provider's, relocated, and nobody read it.
    fn reached(&mut self, written: &Spelled, optional: bool) -> Option<Seen> {
        let seen = self.resolve(written)?;
        match (&seen.canonical, seen.class) {
            (Some(canonical), Some(PrefixClass::Destination)) => {
                if self.queued.insert(canonical.clone()) {
                    self.queue.push(canonical.clone());
                }
            }
            (None, Some(PrefixClass::Destination)) => {
                self.facts.walk_over_bound |= !optional;
            }
            (Some(canonical), None) if !self.is_system(canonical) => {
                self.facts.file_not_examined = true;
            }
            _ => {}
        }
        Some(seen)
    }
}

/// `name` with its token expanded, or `None` when the file alone cannot say:
/// `@executable_path` outside an executable depends on who loads it, another token
/// is not known, and a relative name is the loader's search.
fn expand(name: &[u8], directory: &[u8], file_type: ExaminedType) -> Option<Spelled> {
    if let Some(rest) = after_token(name, b"@loader_path") {
        return spelled(Some(directory), rest);
    }
    if let Some(rest) = after_token(name, b"@executable_path") {
        return (file_type == ExaminedType::Execute)
            .then(|| spelled(Some(directory), rest))
            .flatten();
    }
    if name.first() == Some(&b'/') {
        return spelled(None, name);
    }
    None
}

/// What follows `token` as a relative path. `@loader_path//x` is `<dir>/x` to dyld,
/// never `/x`, so every leading slash goes. A longer word is another token.
fn after_token<'n>(name: &'n [u8], token: &[u8]) -> Option<&'n [u8]> {
    let rest = name.strip_prefix(token)?;
    (rest.is_empty() || rest.first() == Some(&b'/')).then(|| without_leading_slashes(rest))
}

fn without_leading_slashes(path: &[u8]) -> &[u8] {
    let from = path.iter().position(|b| *b != b'/').unwrap_or(path.len());
    &path[from..]
}

/// What `Walk::seen` keeps `written` under.
fn seen_key(written: &Spelled) -> &[u8] {
    if has_parent_component(&written.raw) {
        &written.raw
    } else {
        &written.normal
    }
}

fn path_of(written: &Spelled) -> &Path {
    Path::new(OsStr::from_bytes(&written.normal))
}

/// Whether a real path lies under the system's own roots. By text, on what
/// `is_system` resolved: nothing else qualifies, including anything spelled in another
/// case, which is the safe direction.
#[must_use]
pub fn is_under_system_root(path: &Path) -> bool {
    SYSTEM_ROOTS.iter().any(|root| path.starts_with(root))
}

#[cfg(test)]
mod tests {
    use super::Fallbacks;

    const GOOD: &str = "source = \"man dyld\"\nread_on = \"2026-11-04\"\n\
                        libraries = [\"/a\"]\nframeworks = [\"/b\"]\n";

    /// As `prefixes.rs` holds its own file: provenance, and nothing silently empty.
    #[test]
    fn the_fallback_file_carries_its_provenance_and_refuses_what_stops_the_look() {
        assert!(Fallbacks::parse(GOOD).is_ok());
        let text = include_str!("../../../data/policy/dyld-fallbacks.toml");
        let embedded = Fallbacks::parse(text).expect("the embedded file loads");
        assert!(!embedded.libraries.is_empty() && !embedded.frameworks.is_empty());
        for broken in [
            GOOD.replace("source = \"man dyld\"\n", ""),
            GOOD.replace("read_on = \"2026-11-04\"\n", ""),
            GOOD.replace("man dyld", " "),
            GOOD.replace("2026-11-04", "soon"),
            GOOD.replace("2026-11-04", "2026-1-104"),
            GOOD.replace("2026-11-04", "2026-11-0"),
            GOOD.replace("2026-11-04", "2026-11-041"),
            GOOD.replace("2026-11-04", "2026-11-0x"),
            GOOD.replace("/a", "a"),
            GOOD.replace("/b", "b"),
            GOOD.replace("[\"/a\"]", "[]"),
            GOOD.replace("[\"/b\"]", "[]"),
            format!("{GOOD}unknown = 1\n"),
        ] {
            assert!(Fallbacks::parse(&broken).is_err(), "{broken}");
        }
    }
}
