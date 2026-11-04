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
use std::collections::HashSet;
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

/// Examines a package: every file of `list` by magic, its symlinks by `lstat`
/// and lexically, and a bounded walk of the destination's own files that its load
/// commands reach (R-27 findings 7, 8, 14). `executable_dirs` says which entries
/// are the package's executable list.
pub fn examine(
    list: &[PathBuf],
    executable_dirs: &[PathBuf],
    prefixes: &PrefixTable,
) -> LinkageFacts {
    let mut walk = Walk {
        prefixes,
        facts: LinkageFacts::default(),
        visited: HashSet::new(),
        files: 0,
        bytes: 0,
        queue: Vec::new(),
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
        walk.read_file(&next, false);
    }
    walk.facts
}

struct Walk<'a> {
    prefixes: &'a PrefixTable,
    facts: LinkageFacts,
    visited: HashSet<(u64, u64)>,
    files: usize,
    bytes: u64,
    queue: Vec<PathBuf>,
}

impl Walk<'_> {
    fn own_entry(&mut self, path: &Path, executable_dirs: &[PathBuf]) {
        let Ok(metadata) = std::fs::symlink_metadata(path) else {
            self.facts.unreadable_file = true;
            return;
        };
        if metadata.is_symlink() {
            // Tested and never followed to read: a link's target is a claim
            // about where it points, and that is all this asks of it.
            match first_hop(path) {
                Some(target) => {
                    self.note_outside_destination(target.as_os_str().as_bytes());
                }
                None => self.facts.unreadable_file = true,
            }
            return;
        }
        if !metadata.is_file() {
            return;
        }
        // `O_NOFOLLOW` guards the last component only, so a directory link
        // inside the destination must not make another prefix's file read as
        // the destination's own: the canonical path is what is tested and opened.
        let Ok(canonical) = std::fs::canonicalize(path) else {
            self.facts.unreadable_file = true;
            return;
        };
        if self.prefixes.class_of(canonical.as_os_str().as_bytes())
            != Some(PrefixClass::Destination)
        {
            self.note_outside_destination(canonical.as_os_str().as_bytes());
            return;
        }
        let on_list = path
            .parent()
            .is_some_and(|parent| executable_dirs.iter().any(|d| d == parent));
        self.read_file(&canonical, on_list);
    }

    /// Records what a path under a prefix other than the destination's means.
    /// Answers whether it is under the destination's own.
    fn note_outside_destination(&mut self, path: &[u8]) -> bool {
        match self.prefixes.class_of(path) {
            Some(PrefixClass::Destination) => true,
            Some(PrefixClass::DetectedOther) => {
                self.facts.links_detected_provider = true;
                false
            }
            Some(PrefixClass::UndetectedCandidate) => {
                self.facts.under_undetected_prefix = true;
                false
            }
            None => false,
        }
    }

    /// `path` is canonical. One open, every test on the handle, and windows
    /// rather than the file.
    fn read_file(&mut self, path: &Path, on_list: bool) {
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
        if !metadata.is_file() || !self.visited.insert((metadata.dev(), metadata.ino())) {
            return;
        }
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
            Ok(Classified::NotObject) => {}
            Ok(Classified::Skipped(_)) => self.facts.skipped_files += 1,
            Ok(Classified::Script(shebang)) => self.shebang(&shebang, on_list),
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
        let mut skipped = false;
        for range in ranges {
            let end = range.offset.saturating_add(range.size);
            let Some(header) =
                self.window(file, range.offset, macho::SLICE_HEADER_BYTES as u64, end)
            else {
                return;
            };
            match macho::plan_slice(&header, range.size) {
                Err(_) => self.facts.malformed_file = true,
                Ok(SlicePlan::Skipped(_)) => skipped = true,
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
        } else if skipped {
            self.facts.skipped_files += 1;
        }
    }

    fn shebang(&mut self, shebang: &Shebang, on_list: bool) {
        let indeterminate = match shebang {
            // An absolute interpreter counts the same as a load command.
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
        };
        if indeterminate {
            if on_list {
                self.facts.env_shebang_on_list = true;
            } else {
                self.facts.env_shebang_elsewhere += 1;
            }
        }
    }

    fn lexical(&mut self, path: &[u8]) {
        match normalise(None, path) {
            Some(normal) => {
                self.note_outside_destination(&normal);
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
        let rpaths: Vec<Option<Vec<u8>>> = refs
            .rpaths
            .iter()
            .map(|rpath| expand(rpath, &directory, refs.file_type))
            .collect();
        for rpath in &rpaths {
            match rpath {
                Some(path) => {
                    self.note_outside_destination(path);
                }
                None => self.facts.rpath_unresolved = true,
            }
        }
        for reference in &refs.loads {
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
                Some(path) => {
                    self.note_outside_destination(&path);
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
        rpaths: &[Option<Vec<u8>>],
    ) {
        if let Some(rest) = name.strip_prefix(b"@rpath/") {
            if rpaths.is_empty() {
                // Zero candidates must not read as clean.
                self.facts.rpath_unresolved = true;
            }
            for rpath in rpaths.iter().flatten() {
                let Some(candidate) = normalise(Some(rpath), rest) else {
                    self.facts.rpath_unresolved = true;
                    continue;
                };
                // One candidate among several need not exist.
                self.reached(&candidate, true);
            }
            return;
        }
        match expand(name, directory, file_type) {
            Some(path) => self.reached(&path, false),
            None => self.facts.rpath_unresolved = true,
        }
    }

    /// A load command reaches `path`: what counts applies to the lexical path and
    /// the canonical one (a directory link inside the destination can lead into
    /// another prefix), and a destination file joins the walk. A destination file
    /// that cannot be examined is not therefore fine, unless it is `optional`.
    fn reached(&mut self, path: &[u8], optional: bool) {
        if !self.note_outside_destination(path) {
            return;
        }
        let Ok(canonical) = std::fs::canonicalize(OsStr::from_bytes(path)) else {
            if !optional {
                self.facts.walk_over_bound = true;
            }
            return;
        };
        if self.note_outside_destination(canonical.as_os_str().as_bytes()) {
            self.queue.push(canonical);
        }
    }
}

/// `name` with its token expanded and normalised, or `None` when the file alone
/// cannot say: `@executable_path` outside an executable depends on who loads it,
/// another token is not known, and a relative name is the loader's search.
fn expand(name: &[u8], directory: &[u8], file_type: ExaminedType) -> Option<Vec<u8>> {
    if let Some(rest) = name.strip_prefix(b"@loader_path") {
        return normalise(Some(directory), rest.strip_prefix(b"/").unwrap_or(rest));
    }
    if let Some(rest) = name.strip_prefix(b"@executable_path") {
        return (file_type == ExaminedType::Execute)
            .then(|| normalise(Some(directory), rest.strip_prefix(b"/").unwrap_or(rest)))
            .flatten();
    }
    if name.first() == Some(&b'/') {
        return normalise(None, name);
    }
    None
}
