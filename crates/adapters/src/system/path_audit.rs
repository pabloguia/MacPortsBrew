// SPDX-License-Identifier: Apache-2.0

//! Auditing `PATH` for `doctor`.
//!
//! `04-migration-and-safety.md` ("Threat model", executable shadowing) asks
//! for one specific report: writable directories that precede system paths.
//! This module produces the *generic* findings a shell search path can have —
//! a directory that is not there, one somebody else can write into, a repeat,
//! an entry that is not absolute — and stops.
//!
//! It has no provider knowledge on purpose. Whether Homebrew's `bin` ought to
//! come before MacPorts' is a question about two peer providers and the user's
//! primary-provider choice (`05-environment-and-primary-provider.md`), it is
//! answered in the application layer, and answering it here would put a
//! preference between the two providers inside a module neither of them owns
//! (INV-17). The "precede system paths" half of the threat-model item is
//! therefore *not* done here: this reports which entries are writable, and a
//! caller compares positions.
//!
//! # This reads the live machine
//!
//! An audit is a `doctor` diagnosis of the machine it runs on. It stats real
//! directories, so its findings are facts about *this* host and must never
//! feed a reproducible analysis: replaying a snapshot on another machine would
//! produce different findings from the same input, which is the INV-15 break
//! `analyze --from-snapshot` exists to catch. Snapshot-driven code paths do
//! not call this.
//!
//! It takes an [`EnvironmentSnapshot`] rather than a bare list so that the
//! entries it reports are the redacted ones by construction — a `PathFinding`
//! is `Serialize` and headed for a diagnostic file, and a home path must not
//! reach it (`06-cli-and-ux.md`, "Privacy and diagnostics"). `home` is what a
//! leading `~` is expanded with before a directory is inspected; without it, a
//! redacted entry is reported as unresolvable rather than silently skipped.

use macportsbrew_domain::EnvironmentSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// What is wrong with one `PATH` entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PathFindingKind {
    /// The same entry appeared earlier. Harmless in itself, but it hides which
    /// copy actually resolves and it is what a duplicated shell block leaves
    /// behind (INV-16).
    Duplicate,
    /// The entry is not an absolute path, so it could not be inspected. The
    /// shell resolves it against the working directory, which makes what runs
    /// depend on where the user stands — the shadowing case in the threat
    /// model. An empty entry (`::`, or a trailing `:`) is this finding: the
    /// shell reads it as the working directory. A `~` entry with no home to
    /// expand it against lands here too.
    Relative,
    /// No directory exists at the entry, or what is there is not a directory.
    Missing,
    /// Every user on the machine may create files in the directory, so anybody
    /// may decide what `git` means for this user.
    WorldWritable,
    /// Every member of the directory's group may create files in it. This is
    /// the ordinary macOS case rather than the exotic one: `/usr/local` and
    /// `/usr/local/bin` are commonly `root:admin` and group-writable, which is
    /// where Homebrew's Intel prefix lives, so every admin account on the
    /// machine can place a binary there.
    GroupWritable,
}

/// One finding about one entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathFinding {
    /// The entry as it appears in `PATH`, redacted.
    pub entry: String,
    /// Its zero-based position, so a caller can reason about order.
    pub index: usize,
    /// What is wrong with it.
    pub kind: PathFindingKind,
}

/// Audits the entries of a redacted `PATH`, in order.
///
/// One entry may produce several findings. They come back in entry order and,
/// within an entry, in the order the checks run: `Duplicate`, then `Relative`,
/// then `Missing`, then `WorldWritable` and `GroupWritable`. A repeat is
/// reported before the entry is resolved, because a repeated relative entry is
/// both things. An entry that is not absolute is reported and then left alone:
/// resolving it is the attack. The same input on the same machine always
/// produces the same list.
pub fn audit_path(environment: &EnvironmentSnapshot, home: Option<&Path>) -> Vec<PathFinding> {
    let mut findings = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (index, entry) in environment.path.iter().enumerate() {
        let mut push = |kind| {
            findings.push(PathFinding {
                entry: entry.clone(),
                index,
                kind,
            });
        };
        if !seen.insert(entry.as_str()) {
            push(PathFindingKind::Duplicate);
        }
        let Some(absolute) = expand_home(entry, home) else {
            push(PathFindingKind::Relative);
            continue;
        };
        // `metadata` follows a symbolic reference, which is right here: what
        // matters is the directory that would actually be searched.
        let Ok(meta) = std::fs::metadata(&absolute) else {
            push(PathFindingKind::Missing);
            continue;
        };
        if !meta.is_dir() {
            push(PathFindingKind::Missing);
            continue;
        }
        let mode = meta.permissions().mode();
        // The sticky bit is not an exemption: it stops one user deleting
        // another's files, not creating new ones, so a sticky world-writable
        // directory on `PATH` is still somewhere a `git` can be planted.
        if mode & 0o002 != 0 {
            push(PathFindingKind::WorldWritable);
        }
        if mode & 0o020 != 0 {
            push(PathFindingKind::GroupWritable);
        }
    }
    findings
}

/// Turns a redacted entry back into a path to inspect, or `None` when the
/// entry is not absolute (and so must not be resolved).
fn expand_home(entry: &str, home: Option<&Path>) -> Option<PathBuf> {
    if entry.starts_with('/') {
        return Some(PathBuf::from(entry));
    }
    let home = home?;
    if entry == "~" {
        return Some(home.to_path_buf());
    }
    let rest = entry.strip_prefix("~/")?;
    Some(home.join(rest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "macportsbrew-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn environment(entries: &[&str]) -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            path: entries.iter().map(|e| (*e).to_string()).collect(),
            reported: BTreeMap::new(),
        }
    }

    fn kinds(findings: &[PathFinding], entry: &str) -> Vec<PathFindingKind> {
        findings
            .iter()
            .filter(|f| f.entry == entry)
            .map(|f| f.kind)
            .collect()
    }

    #[test]
    fn every_kind_of_finding_is_reported_with_its_position() {
        let dir = scratch("audit");
        let good = dir.join("good");
        let open = dir.join("open");
        let shared = dir.join("shared");
        for d in [&good, &open, &shared] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::set_permissions(&good, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o777)).unwrap();
        // The `/usr/local` shape on Intel: group-writable, not world-writable.
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o775)).unwrap();
        let absent = dir.join("absent");
        let file = dir.join("a-file");
        std::fs::write(&file, b"not a directory").unwrap();

        let entries = [
            open.to_string_lossy().into_owned(),
            shared.to_string_lossy().into_owned(),
            good.to_string_lossy().into_owned(),
            absent.to_string_lossy().into_owned(),
            file.to_string_lossy().into_owned(),
            "relative/bin".to_string(),
            String::new(),
            good.to_string_lossy().into_owned(),
        ];
        let refs: Vec<&str> = entries.iter().map(String::as_str).collect();
        let findings = audit_path(&environment(&refs), None);

        assert_eq!(
            kinds(&findings, &open.to_string_lossy()),
            vec![
                PathFindingKind::WorldWritable,
                PathFindingKind::GroupWritable
            ]
        );
        assert_eq!(
            kinds(&findings, &shared.to_string_lossy()),
            vec![PathFindingKind::GroupWritable],
            "the common macOS /usr/local shape must not be silent"
        );
        assert_eq!(
            kinds(&findings, &absent.to_string_lossy()),
            vec![PathFindingKind::Missing]
        );
        assert_eq!(
            kinds(&findings, &file.to_string_lossy()),
            vec![PathFindingKind::Missing],
            "a file on PATH is no directory to search"
        );
        assert_eq!(
            kinds(&findings, "relative/bin"),
            vec![PathFindingKind::Relative]
        );
        assert_eq!(kinds(&findings, ""), vec![PathFindingKind::Relative]);
        assert_eq!(
            kinds(&findings, &good.to_string_lossy()),
            vec![PathFindingKind::Duplicate],
            "the first occurrence is clean, the repeat is the finding"
        );

        // Findings stay in entry order, so a caller can compare positions.
        let indices: Vec<usize> = findings.iter().map(|f| f.index).collect();
        assert!(indices.windows(2).all(|w| w[0] <= w[1]));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_repeated_relative_entry_is_both_things_in_the_documented_order() {
        let findings = audit_path(&environment(&["bin", "bin"]), None);
        let kinds: Vec<PathFindingKind> = findings.iter().map(|f| f.kind).collect();
        assert_eq!(
            kinds,
            vec![
                PathFindingKind::Relative,
                PathFindingKind::Duplicate,
                PathFindingKind::Relative
            ]
        );
    }

    #[test]
    fn a_clean_path_produces_nothing() {
        let dir = scratch("audit-clean");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let entry = dir.to_string_lossy().into_owned();
        assert!(audit_path(&environment(&[&entry]), None).is_empty());
        assert!(audit_path(&environment(&[]), None).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_redacted_entry_is_expanded_against_the_home_it_was_redacted_with() {
        let home = scratch("audit-home");
        let bin = home.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(audit_path(&environment(&["~/bin"]), Some(&home)).is_empty());
        assert_eq!(
            kinds(
                &audit_path(&environment(&["~/absent"]), Some(&home)),
                "~/absent"
            ),
            vec![PathFindingKind::Missing]
        );
        // Without a home the entry cannot be inspected, and is reported rather
        // than quietly passed.
        assert_eq!(
            kinds(&audit_path(&environment(&["~/bin"]), None), "~/bin"),
            vec![PathFindingKind::Relative]
        );
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_finding_serialises_with_a_stable_shape() {
        let finding = PathFinding {
            entry: "~/bin".into(),
            index: 0,
            kind: PathFindingKind::GroupWritable,
        };
        let text = serde_json::to_string(&finding).unwrap();
        assert_eq!(
            text,
            r#"{"entry":"~/bin","index":0,"kind":"GROUP_WRITABLE"}"#
        );
        assert_eq!(serde_json::from_str::<PathFinding>(&text).unwrap(), finding);
    }
}
