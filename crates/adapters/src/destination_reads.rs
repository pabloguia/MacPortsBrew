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
    normalise, DomainError, LinkageFacts, PrefixClass, PrefixTable, UncheckedCause,
};
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

/// A parsed, bounded file list: what may be `stat`ed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedList {
    /// Listed paths under the root.
    pub paths: Vec<PathBuf>,
    /// Listed paths outside it, not tested.
    pub outside: usize,
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

/// `lstat`, then `stat`, of every path. Never opens one.
pub fn stat_counts(paths: &[PathBuf]) -> StatCounts {
    let mut counts = StatCounts::default();
    for path in paths {
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
}

/// Examines a package: every file of `list` by magic, its links by `lstat` and
/// by where they finally lead, and a bounded walk of the destination's own files
/// that its load commands reach (R-27 findings 7, 8, 14). `executable_dirs` says
/// which entries are the package's executable list.
pub fn examine(
    list: &[PathBuf],
    executable_dirs: &[PathBuf],
    prefixes: &PrefixTable,
) -> LinkageFacts {
    examine_counted(list, executable_dirs, prefixes).0
}

/// [`examine`], and what it cost.
pub fn examine_counted(
    list: &[PathBuf],
    executable_dirs: &[PathBuf],
    prefixes: &PrefixTable,
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
    let mut walk = Walk {
        prefixes,
        identities,
        facts: LinkageFacts::default(),
        work: WalkWork::default(),
        seen: HashMap::new(),
        visited: HashMap::new(),
        files: 0,
        bytes: 0,
        queue: Vec::new(),
        queued: HashSet::new(),
    };
    for path in list {
        if walk.facts.walk_over_bound {
            break;
        }
        walk.own_entry(path, executable_dirs);
    }
    while let Some(next) = walk.queue.pop() {
        if walk.facts.walk_over_bound {
            break;
        }
        walk.read_file(&next, Reached::ByLoadCommand);
    }
    (walk.facts, walk.work)
}

/// A reference as written: `raw` is the text joined and untouched, `..` intact,
/// and `normal` the same with dots collapsed. They name one file unless a
/// directory link lies under a `..`, and that is why both are kept.
struct Spelled {
    raw: Vec<u8>,
    normal: Vec<u8>,
}

fn spelled(base: Option<&[u8]>, path: &[u8]) -> Option<Spelled> {
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
    /// By its spelling alone.
    spelled_class: Option<PrefixClass>,
    /// By where it resolves: the canonical path, or by inode.
    class: Option<PrefixClass>,
    /// `None` for something that does not exist.
    canonical: Option<PathBuf>,
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

struct Walk<'a> {
    prefixes: &'a PrefixTable,
    /// Each prefix by `(dev, ino)`: a firmlink or a Unicode fold spells a prefix
    /// no string comparison sees, and the inode is the same.
    identities: HashMap<(u64, u64), PrefixClass>,
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
}

impl Walk<'_> {
    fn own_entry(&mut self, path: &Path, executable_dirs: &[PathBuf]) {
        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            self.facts.unreadable_file = true;
            return;
        };
        let on_list = path
            .parent()
            .is_some_and(|parent| executable_dirs.iter().any(|d| d == parent));
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

    /// Canonicalises what exists, and measures the spelling, the canonical path
    /// and the inodes of its ancestors, noting every answer. A candidate seen
    /// before costs a count and no filesystem access. `None` once the bound is
    /// passed.
    fn resolve(&mut self, written: &Spelled) -> Option<Seen> {
        self.work.operations += 1;
        if self.work.operations > MAX_WALK_REFERENCES {
            self.facts.walk_over_bound = true;
            return None;
        }
        let parent = has_parent_component(&written.raw);
        let key = if parent {
            &written.raw
        } else {
            &written.normal
        };
        if let Some(seen) = self.seen.get(key) {
            return Some(seen.clone());
        }
        self.work.resolutions += 1;
        let spelled_class = self.prefixes.class_of(&written.normal);
        self.note(spelled_class);
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
        let measured = canonical.as_deref().unwrap_or(normal);
        let by_text = self.prefixes.class_of(measured.as_os_str().as_bytes());
        let by_inode = self.class_by_identity(measured);
        self.note(by_text);
        self.note(by_inode);
        let seen = Seen {
            spelled_class,
            class: by_inode.or(by_text),
            canonical,
        };
        self.seen.insert(key.clone(), seen.clone());
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
            Ok(Classified::Thin(range)) => self.slices(&file, path, &[range]),
            Ok(Classified::Fat(ranges)) => self.slices(&file, path, &ranges),
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

    fn slices(&mut self, file: &File, path: &Path, ranges: &[SliceRange]) {
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

    /// Answers whether what the script runs is found at run time. An absolute
    /// interpreter counts as a load command does, and so does an absolute word
    /// after it (`env /x/python3`); neither is ever read.
    fn shebang(&mut self, shebang: &Shebang, head: &[u8]) -> bool {
        for word in macho::shebang_arguments(head) {
            if word.first() == Some(&b'/') {
                self.lexical(word);
            }
        }
        match shebang {
            Shebang::Absolute(path) => {
                self.lexical(path);
                false
            }
            // `env`'s own path is still an answer; what it finds is not.
            Shebang::Env(path) => {
                self.lexical(path);
                true
            }
            Shebang::Relative(_) => true,
        }
    }

    fn lexical(&mut self, path: &[u8]) {
        match spelled(None, path) {
            Some(written) => {
                self.resolve(&written);
            }
            None => self.facts.rpath_unresolved = true,
        }
    }

    /// Every reference of one slice. An rpath entry is tested whether or not a
    /// name uses it: a loader's entry can serve another file (R-27 finding 4).
    fn references(&mut self, loader: &Path, refs: &macho::SliceRefs) {
        let directory = loader
            .parent()
            .map_or(&b"/"[..], |d| d.as_os_str().as_bytes())
            .to_vec();
        let rpaths: Vec<Option<Spelled>> = refs
            .rpaths
            .iter()
            .map(|rpath| expand(rpath, &directory, refs.file_type))
            .collect();
        for rpath in &rpaths {
            match rpath {
                Some(written) => {
                    if self.resolve(written).is_none() {
                        return;
                    }
                }
                None => self.facts.rpath_unresolved = true,
            }
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
                | LoadKind::Upward
                | LoadKind::Dylinker => {
                    self.load(&reference.name, &directory, refs.file_type, &rpaths);
                }
            }
        }
    }

    /// `NAME=value`, whose value is a `:`-separated list of paths.
    fn dyld_environment(&mut self, text: &[u8], directory: &[u8]) {
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
        rpaths: &[Option<Spelled>],
    ) {
        if let Some(rest) = name.strip_prefix(b"@rpath/") {
            // Zero candidates must not read as clean, and neither must candidates
            // none of which is there or could have been the destination's.
            let mut answered = false;
            for rpath in rpaths.iter().flatten() {
                let Some(candidate) = spelled(Some(&rpath.raw), rest) else {
                    self.facts.rpath_unresolved = true;
                    continue;
                };
                // One candidate among several need not exist.
                let Some(seen) = self.reached(&candidate, true) else {
                    return;
                };
                answered |=
                    seen.canonical.is_some() || seen.class == Some(PrefixClass::Destination);
            }
            self.facts.rpath_unresolved |= !answered;
            return;
        }
        match expand(name, directory, file_type) {
            Some(written) => {
                self.reached(&written, false);
            }
            None => self.facts.rpath_unresolved = true,
        }
    }

    /// A load command reaches `written`: what counts applies to every spelling of
    /// it, and a destination file joins the walk. A destination file that cannot be
    /// examined is not therefore fine, unless it is `optional`, and neither is a
    /// name under the destination that leads outside every known prefix.
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
            (Some(_), None) if seen.spelled_class == Some(PrefixClass::Destination) => {
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
    if let Some(rest) = name.strip_prefix(b"@loader_path") {
        return spelled(Some(directory), rest.strip_prefix(b"/").unwrap_or(rest));
    }
    if let Some(rest) = name.strip_prefix(b"@executable_path") {
        return (file_type == ExaminedType::Execute)
            .then(|| spelled(Some(directory), rest.strip_prefix(b"/").unwrap_or(rest)))
            .flatten();
    }
    if name.first() == Some(&b'/') {
        return spelled(None, name);
    }
    None
}
