// SPDX-License-Identifier: Apache-2.0

//! The materialised half of the service question for a formula (DP-032): did
//! Homebrew write launch machinery into the keg?
//!
//! Homebrew writes the plist at install, from the formula as it was then, into
//! the keg itself — `formula_installer.rb` `install_service` writes
//! `formula.launchd_service_path`, which is `<keg>/<plist_name>.plist`
//! (`formula.rb` lines 1535-1542), and a `homebrew.<name>.service` systemd unit
//! beside it (R-8 addendum 2026-10-28, A-5). It outlives the declaration: a
//! formula that later drops its `service` block keeps the plist in every keg
//! already installed. So this half catches a service the JSON no longer shows.
//!
//! **Which names count.** The plist's name is a dated provider fact (INV-20):
//! [`CANONICAL_LABEL_PREFIX`] since Homebrew 6.0.22, [`LEGACY_LABEL_PREFIX`]
//! before, and an explicit `name macos:` overrides both. A keg can hold any of
//! them, and an old custom label may no longer be in the JSON at all, so this
//! reader does not match names: **any regular top-level `*.plist` or
//! `*.service` file** is `YES` (security review, M3 and note 4). The label from
//! `service.name.macos` is never joined into a path.
//!
//! **Fail closed.** `UNKNOWN`, never `NO`, for: this host's disk not being the
//! machine described (INV-15); a cellar that is not a safe absolute path; a
//! name or version that is not a single plain path component; a cellar,
//! formula directory or keg that is not a real directory (absent included — an
//! absent keg is not an empty one, M2); a listing that cannot be read or holds
//! more than [`MAX_KEG_ENTRIES`]; an entry whose name is not UTF-8; and a
//! `*.plist` or `*.service` entry that is not a regular file (a symlink).
//! Nothing here is read but directory listings and `lstat`; no file is opened.

use crate::filesystem::FilesystemScope;
use macportsbrew_domain::Answer;
use std::path::{Component, Path};

/// The label prefix Homebrew gives a generated plist **since 6.0.22**:
/// `sh.brew.<name>`. `Library/Homebrew/service.rb` at Homebrew 7.0.4 (sha256
/// `6da2207bb5f7e72dfb8477f19ab53aaab8d6a9de9807a9f9f5a1711e4c3c7cb7`, lines
/// 86-105, `canonical_plist_name`), introduced by commit `963b69f634`
/// (2026-09-02), first tagged `6.0.22`. Read 2026-10-28 (R-8 addendum, A-5).
/// Re-check when that file's sha256 changes. Documentation only: the reader
/// accepts any regular `*.plist`.
pub const CANONICAL_LABEL_PREFIX: &str = "sh.brew.";

/// The label prefix Homebrew gave a generated plist **before 6.0.22**, and
/// still recognises: `homebrew.mxcl.<name>` (`service.rb` 7.0.4,
/// `legacy_plist_name`, same sha256 and lines as
/// [`CANONICAL_LABEL_PREFIX`]). Every keg plist on the machine R-8 observed
/// carries it. DP-032's wording names only this one; the intent is "the plist
/// Homebrew materialised" (SE-109). Read 2026-10-28.
pub const LEGACY_LABEL_PREFIX: &str = "homebrew.mxcl.";

/// The name prefix of the systemd unit Homebrew writes beside the plist,
/// `homebrew.<name>.service` (R-8 addendum, A-5, observed 2026-10-28).
/// Documentation only: the reader accepts any regular `*.service`.
pub const SERVICE_UNIT_PREFIX: &str = "homebrew.";

/// The most top-level entries a keg listing may have. The kegs R-8 observed
/// hold a dozen; more than this is not a keg this reader was written for, and
/// a truncated listing would be a guessed `NO`.
pub const MAX_KEG_ENTRIES: usize = 512;

/// Whether `value` is one plain path component a provider could have written:
/// non-empty, not `.` or `..`, no separator, not option-shaped, nothing that
/// would make the joined path add anything but one `Normal` component.
fn is_leaf(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !value.starts_with(['-', '.'])
        && !value.contains(['/', '\0'])
        && matches!(
            Path::new(value).components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        )
}

/// Whether `path` is a real directory, decided without following a link.
fn is_real_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

/// The materialised half for a formula named `name` with `versions` installed
/// under `cellar`. Versions combine as installed entries do in R-8's rule: any
/// `YES` is `YES`, else any `UNKNOWN` is `UNKNOWN`, else `NO`.
pub fn materialised(
    cellar: &Path,
    name: &str,
    versions: &[&str],
    filesystem: FilesystemScope,
) -> Answer {
    if !filesystem.is_this_machine() {
        // The recording is the machine, and it carries no keg (INV-15).
        return Answer::Unknown;
    }
    if versions.is_empty()
        || !crate::process::is_safe_path(cellar)
        || !cellar.is_absolute()
        || !is_leaf(name)
    {
        return Answer::Unknown;
    }
    let formula_dir = cellar.join(name);
    if !is_real_directory(cellar) || !is_real_directory(&formula_dir) {
        return Answer::Unknown;
    }
    let mut answer = Answer::No;
    for version in versions {
        match keg(&formula_dir, version) {
            Answer::Yes => return Answer::Yes,
            Answer::Unknown => answer = Answer::Unknown,
            Answer::No => {}
        }
    }
    answer
}

/// One keg's answer.
fn keg(formula_dir: &Path, version: &str) -> Answer {
    if !is_leaf(version) {
        return Answer::Unknown;
    }
    let keg = formula_dir.join(version);
    // Exactly one more `Normal` component than the formula directory.
    if keg.components().count() != formula_dir.components().count() + 1 || !is_real_directory(&keg)
    {
        return Answer::Unknown;
    }
    let Ok(entries) = std::fs::read_dir(&keg) else {
        return Answer::Unknown;
    };
    // Decided only once the whole listing has been seen, so an over-long or
    // unreadable listing is `UNKNOWN` whatever order the entries come in
    // (spec-guardian, part D review, risk 5).
    let mut answer = Answer::No;
    for (seen, entry) in entries.enumerate() {
        if seen >= MAX_KEG_ENTRIES {
            return Answer::Unknown;
        }
        let Ok(entry) = entry else {
            return Answer::Unknown;
        };
        let Ok(file_name) = entry.file_name().into_string() else {
            return Answer::Unknown;
        };
        let launch_file = Path::new(&file_name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("plist") || e.eq_ignore_ascii_case("service"));
        if !launch_file {
            continue;
        }
        // `DirEntry::file_type` does not follow a symbolic link.
        match entry.file_type() {
            Ok(kind) if kind.is_file() => answer = Answer::Yes,
            _ if answer == Answer::Yes => {}
            _ => answer = Answer::Unknown,
        }
    }
    answer
}
