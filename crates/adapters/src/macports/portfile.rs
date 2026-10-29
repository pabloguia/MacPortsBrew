// SPDX-License-Identifier: Apache-2.0

//! The declared half of the service question for a port (DP-032): does the
//! Portfile the port was **installed from** declare a startup item?
//!
//! **Which file.** MacPorts keeps the installed Portfile in its registry, at
//! `<portdbpath>/registry/portfiles/<name>-<version>_<revision>/<sha256>-<size>/Portfile`,
//! and the PortGroups it used at
//! `<portdbpath>/registry/portgroups/<sha256>-<size>/<name>-<version>.tcl`
//! (R-8 addendum 2026-10-28, A-1; MacPorts 2.12.6 `registry2.0/portimage.tcl`
//! sha256 `d8ecb2b3736a587ee58d806c8048f86b560ea920b2cec5821b8f3462560cbb5c`,
//! lines 1083-1122). Never the ports tree, which describes the current port
//! and not what was installed. Re-check when MacPorts' minor version changes.
//!
//! **What "declares" means** (F-A2): an item whose `type` is not `none`, or
//! whose `create` is true, or which has a `custom_file`. A Portfile is Tcl, and
//! this reader evaluates nothing: [`recognise`] accepts a small set of
//! top-level, literal forms as `YES` and answers `NO` only for a file that
//! contains no `startupitem` and no `source` at all (and whose PortGroups pass
//! [`stored_groups`]). Everything else is `UNKNOWN`, which refuses a plan
//! (INV-19). **`YES` wins**: one recognised declaration makes the answer `YES`
//! whatever else the file holds, because over-approximating reaches
//! `MANUAL_REVIEW`, the safe side.
//!
//! **Threat model (L4).** This interprets Portfiles written in good faith. It
//! is not a control against a hostile one (`startup\x69tem`, `eval`, `[string
//! cat ...]`): a hostile Portfile already ran as root at install. An unindented
//! declaration inside a block reads as top-level and so only ever adds a false
//! `YES` — the safe side; do not "fix" it toward `NO`.
//!
//! **Reading.** Only where this host's disk is the machine described (INV-15).
//! Every path is composed from checked leaves, never from Portfile text; every
//! directory is `lstat`-checked; each file is opened once through
//! [`crate::persistence::bounded_read::read_bounded`], refusing a symlink at
//! the open and checking the handle against the `lstat` baseline, its link
//! count and the size its directory name records. Directories above the file
//! are only `lstat`-checked, the residual `bounded_read.rs` already accepts
//! under ADR-0006. Files are read one at a time and dropped; after
//! [`MAX_TOTAL_BYTES`] the remaining ports are `UNKNOWN`.

use crate::filesystem::FilesystemScope;
use macportsbrew_domain::Answer;
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path};

/// The registry directory under `portdbpath` that holds installed Portfiles.
pub const PORTFILES_DIRECTORY: &str = "registry/portfiles";

/// The registry directory under `portdbpath` that holds stored PortGroups.
pub const PORTGROUPS_DIRECTORY: &str = "registry/portgroups";

/// The most entries a `<name>-<version>_<revision>` directory may hold. One
/// hash directory is the norm (A-1); a handful of dot-files is Finder.
pub const MAX_VERSION_ENTRIES: usize = 16;

/// The most stored PortGroup directories (A-1 observed 9 on one machine).
pub const MAX_PORTGROUP_ENTRIES: usize = 256;

/// The largest single Portfile or PortGroup this reader opens. The largest
/// installed one R-8 observed is under 10 KiB.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;

/// The most bytes read across every Portfile and PortGroup in one capture.
pub const MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024;

/// What the stored PortGroups say about a Portfile that uses one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Groups {
    /// Every stored group was read, and none mentions `startupitem`. Holds
    /// the file names found (`<name>-<version>.tcl`), as data: a Portfile's
    /// `PortGroup <name> <version>` line is compared against them, never
    /// joined into a path (security review L3).
    Clean(BTreeSet<String>),
    /// A stored group mentions `startupitem`, or one could not be read or
    /// listed: a Portfile that uses a PortGroup cannot be answered `NO`.
    Unanswerable,
}

impl Groups {
    /// `Clean` with the given stored file names.
    #[must_use]
    pub fn clean<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        Self::Clean(names.into_iter().map(str::to_string).collect())
    }

    /// Whether every `PortGroup` line in `bytes` names a group stored here.
    ///
    /// A line counts wherever it is (indented or not) when its first word is
    /// `PortGroup`, compared case-insensitively. It must be exactly
    /// `PortGroup <literal> <literal>`, and `<name>-<version>.tcl` must be one
    /// of the stored names. Any other line mentioning the token, unless it is a
    /// comment line, is `false` too: a group loaded from inside another
    /// command is one this check cannot name. Anything else, or
    /// `Unanswerable`, is `false`.
    fn cover(&self, bytes: &[u8]) -> bool {
        let Self::Clean(names) = self else {
            return false;
        };
        for raw in bytes.split(|b| *b == b'\n') {
            let Ok(line) = std::str::from_utf8(raw) else {
                return false;
            };
            let words: Vec<&str> = line
                .split([' ', '\t', '\r'])
                .filter(|w| !w.is_empty())
                .collect();
            match words.as_slice() {
                // A comment line is skipped; any other line that mentions the
                // token without being the plain form — `x; PortGroup a 1.0`,
                // `if {$x} { PortGroup a 1.0 }` — is a group 4b cannot name
                // (security-reviewer R1).
                [first, ..] if first.starts_with('#') => {}
                [first, ..] if !first.eq_ignore_ascii_case("portgroup") => {
                    if mentions(raw, b"portgroup") {
                        return false;
                    }
                }
                [] => {}
                [_, name, version] if is_literal(name) && is_literal(version) => {
                    if !names.contains(&format!("{name}-{version}.tcl")) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        true
    }
}

/// A byte budget shared by every read of one capture (L7).
#[derive(Debug)]
pub struct Budget(u64);

impl Budget {
    /// A fresh budget of [`MAX_TOTAL_BYTES`].
    #[must_use]
    pub const fn new() -> Self {
        Self(MAX_TOTAL_BYTES)
    }

    /// Whether `n` more bytes may be read. An exhausted budget stays so.
    fn allows(&mut self, n: u64) -> bool {
        if n > self.0 {
            self.0 = 0;
            false
        } else {
            true
        }
    }

    /// Spends `n` bytes, or says there are not that many left.
    fn spend(&mut self, n: u64) -> bool {
        if let Some(left) = self.0.checked_sub(n) {
            self.0 = left;
            true
        } else {
            self.0 = 0;
            false
        }
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::new()
    }
}

/// One installed entry, as the registry reports it.
#[derive(Clone, Copy, Debug)]
pub struct Entry<'a> {
    /// The port name.
    pub name: &'a str,
    /// Its version, without the revision.
    pub version: &'a str,
    /// Its revision. `None` is `UNKNOWN`, never assumed `0` (L1).
    pub revision: Option<&'a str>,
}

/// The declared half for every installed port, combining several installed
/// entries of one port: any `YES` gives `YES`, else any `UNKNOWN` gives
/// `UNKNOWN`, else `NO` (L1, R5). `portdbpath` is `None` when it is not the
/// default ([`crate::configuration::macports_portdbpath`]), which makes every
/// answer `UNKNOWN`.
pub fn declared(
    portdbpath: Option<&Path>,
    entries: &[Entry<'_>],
    filesystem: FilesystemScope,
) -> BTreeMap<String, Answer> {
    let mut out: BTreeMap<String, Answer> = BTreeMap::new();
    let readable = filesystem.is_this_machine().then_some(portdbpath).flatten();
    let mut budget = Budget::new();
    let groups = readable.map(|root| stored_groups(root, filesystem, &mut budget));
    for entry in entries {
        let answer = match (readable, &groups) {
            (Some(root), Some(groups)) => entry_answer(root, entry, groups, &mut budget),
            _ => Answer::Unknown,
        };
        let combined = out.entry(entry.name.to_string()).or_insert(Answer::No);
        *combined = match (*combined, answer) {
            (Answer::Yes, _) | (_, Answer::Yes) => Answer::Yes,
            (Answer::Unknown, _) | (_, Answer::Unknown) => Answer::Unknown,
            (Answer::No, Answer::No) => Answer::No,
        };
    }
    out
}

/// Whether `value` is a version or revision this reader will compose into a
/// path: `[A-Za-z0-9._+~-]`, not `.` or `..`, no leading `-` or `.`.
fn is_safe_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with(['-', '.'])
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'~' | b'-'))
}

/// Whether `leaf` is exactly one `Normal` path component.
fn is_one_component(leaf: &str) -> bool {
    matches!(
        Path::new(leaf).components().collect::<Vec<_>>().as_slice(),
        [Component::Normal(c)] if *c == std::ffi::OsStr::new(leaf)
    )
}

/// Whether `path` is a real directory, decided without following a link.
fn is_real_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

/// The size a `<sha256>-<size>` directory name records, when the name has
/// exactly that shape: 64 lower-case hex digits, `-`, 1 to 10 decimal digits
/// that fit a `u64`.
fn hash_dir_size(name: &str) -> Option<u64> {
    let (hash, size) = name.split_once('-')?;
    let hex = hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    let digits = (1..=10).contains(&size.len()) && size.bytes().all(|b| b.is_ascii_digit());
    if hex && digits {
        size.parse().ok()
    } else {
        None
    }
}

/// The real `<sha256>-<size>` directories in `dir`, with their sizes, skipping
/// regular dot-files (Finder's `.DS_Store`, as `service::scan` does). `None`
/// for anything else in the listing, a listing that cannot be read, or more
/// than `bound` entries.
fn hash_dirs(dir: &Path, bound: usize) -> Option<Vec<(std::path::PathBuf, u64)>> {
    let mut found = Vec::new();
    for (seen, entry) in std::fs::read_dir(dir).ok()?.enumerate() {
        if seen >= bound {
            return None;
        }
        let entry = entry.ok()?;
        let name = entry.file_name().into_string().ok()?;
        let kind = entry.file_type().ok()?;
        if name.starts_with('.') && kind.is_file() {
            continue;
        }
        let size = hash_dir_size(&name)?;
        if !kind.is_dir() {
            return None;
        }
        found.push((entry.path(), size));
    }
    Some(found)
}

/// Reads one file the way this module reads every file: `lstat` baseline, one
/// open refusing a symlink, the handle checked against the baseline, a single
/// name, and — when `size` is given — exactly that length, on the handle and
/// again in the bytes read (security review L2). Spends the budget by the
/// bytes actually read.
fn read_checked(path: &Path, size: Option<u64>, budget: &mut Budget) -> Option<String> {
    let before = std::fs::symlink_metadata(path).ok()?;
    if !before.file_type().is_file() || before.len() > MAX_FILE_BYTES {
        return None;
    }
    if !budget.allows(before.len()) {
        return None;
    }
    let text = crate::persistence::bounded_read::read_bounded(
        path,
        MAX_FILE_BYTES,
        crate::persistence::bounded_read::OpenPolicy::RefuseSymlink,
        |opened| {
            if (opened.dev(), opened.ino()) != (before.dev(), before.ino()) {
                return Err("the file changed between the pre-check and the open");
            }
            if opened.nlink() != 1 {
                return Err("the file has more than one name");
            }
            if size.is_some_and(|s| s != opened.len()) {
                return Err("the file's length is not the one its directory records");
            }
            Ok(())
        },
    )
    .ok()?;
    let read = text.len() as u64;
    if !budget.spend(read) || read != size.unwrap_or(before.len()) {
        return None;
    }
    Some(text)
}

/// Whether `haystack` contains `needle`, ASCII case-insensitively, on raw
/// bytes (before any decoding).
fn mentions(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle))
}

/// Whether `haystack` contains `needle` as a whole word, ASCII
/// case-insensitively, on raw bytes: neither neighbour is a letter, a digit,
/// `_` or `-`. So `;source`, `[source x]` and `eval source` count, and
/// `sourceforge`, `sources` and `open-source` do not.
fn mentions_word(haystack: &[u8], needle: &[u8]) -> bool {
    let word = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-');
    haystack.windows(needle.len()).enumerate().any(|(at, w)| {
        w.eq_ignore_ascii_case(needle)
            && (at == 0 || !word(haystack[at - 1]))
            && haystack.get(at + needle.len()).is_none_or(|b| !word(*b))
    })
}

/// The stored PortGroups, all of them (4b, M1, D2): no path is ever built from
/// Portfile text. `Clean` only if the directory is a real one, holds at most
/// [`MAX_PORTGROUP_ENTRIES`] real `<sha256>-<size>` directories, and every
/// `*.tcl` in each was read and does not mention `startupitem`. Only where
/// this host's disk is the machine described (INV-15).
pub fn stored_groups(
    portdbpath: &Path,
    filesystem: FilesystemScope,
    budget: &mut Budget,
) -> Groups {
    if !filesystem.is_this_machine() {
        return Groups::Unanswerable;
    }
    let mut names = BTreeSet::new();
    let root = portdbpath.join(PORTGROUPS_DIRECTORY);
    if !is_real_directory(&root) {
        return Groups::Unanswerable;
    }
    let Some(dirs) = hash_dirs(&root, MAX_PORTGROUP_ENTRIES) else {
        return Groups::Unanswerable;
    };
    for (dir, _) in dirs {
        let Ok(listing) = std::fs::read_dir(&dir) else {
            return Groups::Unanswerable;
        };
        for (seen, entry) in listing.enumerate() {
            if seen >= MAX_VERSION_ENTRIES {
                return Groups::Unanswerable;
            }
            let Ok(entry) = entry else {
                return Groups::Unanswerable;
            };
            let Ok(name) = entry.file_name().into_string() else {
                return Groups::Unanswerable;
            };
            if !Path::new(&name)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("tcl"))
            {
                continue;
            }
            match read_checked(&entry.path(), None, budget) {
                Some(text) if !mentions(text.as_bytes(), b"startupitem") => {}
                _ => return Groups::Unanswerable,
            }
            names.insert(name);
        }
    }
    Groups::Clean(names)
}

/// One installed entry's answer.
fn entry_answer(
    portdbpath: &Path,
    entry: &Entry<'_>,
    groups: &Groups,
    budget: &mut Budget,
) -> Answer {
    let Some(revision) = entry.revision else {
        return Answer::Unknown;
    };
    if !super::names::is_safe_name(entry.name)
        || !is_safe_part(entry.version)
        || !is_safe_part(revision)
    {
        return Answer::Unknown;
    }
    let leaf = format!("{}-{}_{revision}", entry.name, entry.version);
    if !is_one_component(&leaf) {
        return Answer::Unknown;
    }
    let version_dir = portdbpath.join(PORTFILES_DIRECTORY).join(&leaf);
    if !is_real_directory(&version_dir) {
        return Answer::Unknown;
    }
    // Exactly one hash directory: two is the variants case of A-1, a known
    // false unknown.
    let Some(dirs) = hash_dirs(&version_dir, MAX_VERSION_ENTRIES) else {
        return Answer::Unknown;
    };
    let [(dir, size)] = dirs.as_slice() else {
        return Answer::Unknown;
    };
    if *size > MAX_FILE_BYTES {
        return Answer::Unknown;
    }
    match read_checked(&dir.join("Portfile"), Some(*size), budget) {
        Some(text) => recognise(text.as_bytes(), groups),
        None => Answer::Unknown,
    }
}

/// Whether `token` is a literal Tcl word: printable ASCII with none of the
/// characters that substitute, quote, group or end a command.
fn is_literal(token: &str) -> bool {
    !token.is_empty()
        && token.bytes().all(|b| {
            b.is_ascii_graphic()
                && !matches!(b, b'$' | b'[' | b']' | b'{' | b'}' | b'"' | b'\\' | b';')
        })
}

/// Whether one top-level line, split into words, is a recognised declaration.
fn declares(words: &[&str]) -> bool {
    match words {
        ["startupitem.create", value] => ["yes", "true", "1", "on"]
            .iter()
            .any(|t| value.eq_ignore_ascii_case(t)),
        ["startupitem.type", value] => is_literal(value) && *value != "none",
        ["startupitem.custom_file", value] => is_literal(value),
        ["startupitems", "name", value, ..] => is_literal(value),
        _ => false,
    }
}

/// **The recogniser** (part D §4c): a pure function of a Portfile's bytes and
/// what its stored PortGroups say.
///
/// | Portfile holds | Answer |
/// |---|---|
/// | A top-level `startupitem.create yes\|true\|1` | `YES` |
/// | A top-level `startupitem.type <literal>` other than `none` | `YES` |
/// | A top-level `startupitem.custom_file <literal>` | `YES` |
/// | A top-level `startupitems` line whose first key is `name <literal>` | `YES` |
/// | No `startupitem` anywhere, no `source` as a word anywhere, and the PortGroups pass | `NO` |
/// | Anything else | `UNKNOWN` |
///
/// A line counts as top-level only when it has no leading whitespace, does
/// not start with `#`, and does not continue the line before it (`\`). The
/// token searches run on raw bytes, case-insensitively, and count comments:
/// a comment mentioning `startupitem` still blocks `NO`. `source` blocks it
/// only as a whole word (`;source`, `[source`, `eval source`), not inside
/// another (`sourceforge`, `open-source`); an English "open source" still
/// blocks it, the safe side (spec-guardian, part D review, risk 4). The
/// PortGroups pass when the stored groups are clean and every `PortGroup
/// <name> <version>` line names a stored `<name>-<version>.tcl` (L3). `YES`
/// wins.
pub fn recognise(bytes: &[u8], groups: &Groups) -> Answer {
    let mut continued = false;
    for raw in bytes.split(|b| *b == b'\n') {
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        let top_level = !continued
            && raw
                .first()
                .is_some_and(|b| !matches!(b, b' ' | b'\t' | b'#'));
        continued = raw.ends_with(b"\\");
        if !top_level {
            continue;
        }
        let Ok(line) = std::str::from_utf8(raw) else {
            continue;
        };
        let words: Vec<&str> = line.split([' ', '\t']).filter(|w| !w.is_empty()).collect();
        if declares(&words) {
            return Answer::Yes;
        }
    }
    let token_free = !mentions(bytes, b"startupitem") && !mentions_word(bytes, b"source");
    let groups_pass = !mentions(bytes, b"portgroup") || groups.cover(bytes);
    if token_free && groups_pass {
        Answer::No
    } else {
        Answer::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------
    // The recogniser, on in-code text.
    // ---------------------------------------------------------------------

    /// The stored groups the in-code texts use: `github 1.0`, as on the machine
    /// R-8 observed (A-1).
    fn github() -> Groups {
        Groups::clean(["github-1.0.tcl"])
    }

    fn answer(text: &str) -> Answer {
        recognise(text.as_bytes(), &github())
    }

    #[test]
    fn every_row_of_the_decision_table() {
        for (text, want) in [
            ("startupitem.create yes\n", Answer::Yes),
            ("startupitem.create      true\n", Answer::Yes),
            ("startupitem.create\t1\n", Answer::Yes),
            ("startupitem.type launchd\n", Answer::Yes),
            ("startupitem.custom_file org.example.plist\n", Answer::Yes),
            (
                "startupitems name svc location LaunchDaemons\n",
                Answer::Yes,
            ),
            ("PortSystem 1.0\nname jq\n", Answer::No),
            // Anything else.
            ("startupitem.create no\n", Answer::Unknown),
            ("startupitem.type none\n", Answer::Unknown),
            (
                "variant server {\n    startupitem.create yes\n}\n",
                Answer::Unknown,
            ),
            ("startupitem.create \\\n    yes\n", Answer::Unknown),
            ("startupitem.type ${kind}\n", Answer::Unknown),
            ("startupitem.custom_file [file join a b]\n", Answer::Unknown),
            (
                "startupitem.custom_file \\\n    ${worksrcpath}/x.plist\n",
                Answer::Unknown,
            ),
            ("startupitems-append name svc\n", Answer::Unknown),
            ("startupitems name ${svc}\n", Answer::Unknown),
            ("startupitem.create-delete yes\n", Answer::Unknown),
            ("source ${filespath}/more.tcl\n", Answer::Unknown),
            ("description a source of truth\n", Answer::Unknown),
            // `source` only as a whole word (risk 4).
            ("set x [source ${filespath}/more.tcl]\n", Answer::Unknown),
            ("post-destroot { eval source x.tcl }\n", Answer::Unknown),
            ("x;source y.tcl\n", Answer::Unknown),
            ("homepage https://sourceforge.net/p/x\n", Answer::No),
            ("description an open-source tool with sources\n", Answer::No),
        ] {
            assert_eq!(answer(text), want, "{text:?}");
        }
    }

    #[test]
    fn create_yes_with_type_none_is_yes() {
        // V2: by F-A2's test it is not a declaration of nothing, and YES is
        // the safe reading.
        assert_eq!(
            answer("startupitem.type none\nstartupitem.create yes\n"),
            Answer::Yes
        );
    }

    #[test]
    fn yes_wins_over_anything_else_in_the_file() {
        assert_eq!(
            answer("source x.tcl\nstartupitem.create yes\nstartupitem.create no\n"),
            Answer::Yes
        );
    }

    #[test]
    fn a_comment_mentioning_the_token_still_blocks_no() {
        assert_eq!(answer("# startupitem.create yes\n"), Answer::Unknown);
        assert_eq!(answer("PortSystem 1.0 ;# StartupItem\n"), Answer::Unknown);
    }

    #[test]
    fn a_continued_line_is_not_top_level_even_unindented() {
        assert_eq!(
            answer("notes \\\nstartupitem.create yes\n"),
            Answer::Unknown
        );
    }

    #[test]
    fn a_portgroup_is_unanswerable_only_when_the_stored_groups_are() {
        let text = b"PortSystem 1.0\nPortGroup github 1.0\n";
        assert_eq!(recognise(text, &github()), Answer::No);
        assert_eq!(recognise(text, &Groups::Unanswerable), Answer::Unknown);
        assert_eq!(
            recognise(b"PortSystem 1.0\n", &Groups::Unanswerable),
            Answer::No
        );
        assert_eq!(
            recognise(
                b"PortGroup x 1.0\nstartupitem.create yes\n",
                &Groups::Unanswerable
            ),
            Answer::Yes
        );
    }

    #[test]
    fn every_portgroup_line_must_name_a_stored_group() {
        // Security review L3: the stored groups are checked as a set, and the
        // groups a Portfile names are compared against the names found there,
        // never joined into a path.
        let stored = Groups::clean(["github-1.0.tcl", "muniversal-1.1.tcl"]);
        for (text, want) in [
            (
                "PortGroup github 1.0\nPortGroup muniversal 1.1\n",
                Answer::No,
            ),
            // Indented lines are data too.
            (
                "PortGroup github 1.0\nif {1} {\n    PortGroup muniversal 1.1\n}\n",
                Answer::No,
            ),
            // A group not among the stored names: its copy was not read.
            (
                "PortGroup github 1.0\nPortGroup cmake 1.1\n",
                Answer::Unknown,
            ),
            ("PortGroup github 1.1\n", Answer::Unknown),
            // A shape this reader does not interpret.
            ("PortGroup ${group} 1.0\n", Answer::Unknown),
            ("PortGroup github\n", Answer::Unknown),
            ("PortGroup github 1.0 extra\n", Answer::Unknown),
            // A comment mentioning one is not a line naming one.
            ("# PortGroup cmake has trouble here\n", Answer::No),
            ("    #PortGroup cmake 1.1\n", Answer::No),
            // A line that loads a group without being the plain form: even a
            // stored group, it is a shape this reader does not interpret
            // (security-reviewer R1).
            ("if {$x} { PortGroup github 1.0 }\n", Answer::Unknown),
            ("x; PortGroup github 1.0\n", Answer::Unknown),
            ("set g 1; portgroup cmake 1.1\n", Answer::Unknown),
        ] {
            assert_eq!(recognise(text.as_bytes(), &stored), want, "{text:?}");
        }
    }

    #[test]
    fn a_whole_word_source_blocks_no_and_a_mention_inside_a_word_does_not() {
        assert_eq!(answer("homepage https://sourceforge.net/x\n"), Answer::No);
        assert_eq!(answer("set x [source y.tcl]\n"), Answer::Unknown);
        assert_eq!(
            answer("# SOURCE the helpers\n"),
            Answer::Unknown,
            "comments count"
        );
    }

    // ---------------------------------------------------------------------
    // The six real excerpts and their oracle (R-8 addendum, A-6). Each is
    // the Portfile's lines verbatim from the ports tree synced on the machine
    // R-8 observed, `/opt/local/var/macports/sources/rsync.macports.org/
    // macports/release/tarballs/ports/<path>/Portfile`, read 2026-10-28.
    // ---------------------------------------------------------------------

    /// `www/nginx`, sha256
    /// `970cb16f5a1ed15dec53c18398e180aeab68902051df8f147dc059f058b42362`,
    /// lines 107-109.
    const NGINX: &str = "startupitem.create      yes\n\
startupitem.pidfile     auto ${nginx_pidfile}\n\
startupitem.executable  ${prefix}/sbin/nginx -g \"daemon off;\"\n";

    /// `databases/redis`, sha256
    /// `52b09851ba46dc682d8b9c8fe7da4708ae7dafbcbcef53b42e2f7ccc4cba3708`,
    /// lines 109-110.
    const REDIS: &str = "startupitem.create      yes\n\
startupitem.executable  ${prefix}/bin/redis-server ${prefix}/etc/redis.conf\n";

    /// `devel/dbus`, sha256
    /// `c5e92c79a8fe40d84ae5a2e8da30937924340d20687c0f54062576c84805c244`,
    /// lines 57-62.
    const DBUS: &str = "startupitem.type    launchd\n\
startupitem.create  no\n\
startupitems        name        dbus-system \\\n                    location    LaunchDaemons \\\n                    uniquename  ${daemon_uniquename} \\\n                    plist       ${daemon_uniquename}.plist \\\n";

    /// `sysutils/logrotate`, sha256
    /// `a2810386df357d92be108efee1e88e76eab909b77b2e60ab1aaaedfd92fd24e6`,
    /// lines 74-77.
    const LOGROTATE: &str = "startupitem.type    launchd\n\
startupitem.create  no\n\
startupitem.custom_file \\\n                    ${worksrcpath}/org.macports.logrotate.plist.example\n";

    /// `databases/mysql8`, sha256
    /// `23177b63d2a0b858e27be30abb700dc26f7c0a219cf553549f8985dc4114c3e9`,
    /// lines 19, 267, 287-288.
    const MYSQL8: &str = "set name_mysql          ${name}\n\
subport ${name_mysql}-server {\n    startupitem.create      yes\n    startupitem.executable  ${prefix}/lib/${name_mysql}/bin/mysqld\n";

    /// `sysutils/jq`, sha256
    /// `bc6d69841dd2407dde9475ba2e0f29e1607315254ca1f5ee672a479ad27e7408`
    /// (identical to the installed copy, A-1), every non-blank code line: no
    /// `startupitem`, no `source`, one PortGroup.
    const JQ: &str = "PortSystem          1.0\n\
PortGroup           github 1.0\n\
github.setup        jqlang jq 1.8.2 jq-\n\
github.tarball_from releases\n\
revision            0\n\
categories          sysutils\n\
license             MIT\n\
depends_lib-append  port:oniguruma6\n\
configure.args      --disable-docs \\\n                    --disable-silent-rules \\\n                    --with-oniguruma=${prefix} \\\n                    --disable-maintainer-mode\n\
test.run            yes\n\
test.target         check\n";

    #[test]
    fn the_six_real_excerpts_answer_their_oracle() {
        for (port, text, want) in [
            ("nginx", NGINX, Answer::Yes),
            ("redis", REDIS, Answer::Yes),
            ("dbus", DBUS, Answer::Yes),
            ("logrotate", LOGROTATE, Answer::Yes),
            ("mysql8", MYSQL8, Answer::Unknown),
            ("jq", JQ, Answer::No),
        ] {
            assert_eq!(answer(text), want, "{port}");
        }
    }

    #[test]
    fn the_byte_budget_makes_the_rest_unknown() {
        let mut budget = Budget::new();
        assert!(budget.spend(MAX_TOTAL_BYTES - 1));
        assert!(!budget.spend(2));
        assert!(!budget.spend(1), "an exhausted budget stays exhausted");
    }
}
