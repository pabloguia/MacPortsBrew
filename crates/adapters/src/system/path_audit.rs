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
//! directories, so its findings are facts about *this* host and must never be
//! recomputed during a replayed analysis: running it again on another machine
//! would produce different findings from the same snapshot, which is the
//! INV-15 break `analyze --from-snapshot` exists to catch.
//!
//! It is therefore called once, from
//! [`MachineRead::environment`](macportsbrew_domain::MachineRead::environment)
//! at capture time, and what it finds is written into
//! [`EnvironmentSnapshot::path_findings`]. `doctor` reads that field and never
//! calls this; a snapshot replayed on another machine carries the findings of
//! the machine it came from, which is the point.
//!
//! It takes an [`EnvironmentSnapshot`] rather than a bare list so that the
//! entries it reports are the redacted ones by construction — a `PathFinding`
//! is `Serialize` and headed for a snapshot and a diagnostic file, and a home
//! path must not reach it (`06-cli-and-ux.md`, "Privacy and diagnostics").
//! `home` is what a leading `~` is expanded with before a directory is
//! inspected; without it, a redacted entry is reported as unresolvable rather
//! than silently skipped.

use macportsbrew_domain::{EnvironmentSnapshot, PathFinding, PathFindingKind};
use std::collections::BTreeSet;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;

/// Audits the entries of a redacted `PATH`, in order.
///
/// One entry may produce several findings. They come back in entry order and,
/// within an entry, in the order the checks run: `Duplicate`, then `Relative`,
/// then `Missing`, then `WorldWritable`, `GroupWritable` and `OwnerWritable`. A repeat is
/// reported before the entry is resolved, because a repeated relative entry is
/// both things. An entry that is not absolute is reported and then left alone:
/// resolving it is the attack. The same input on the same machine always
/// produces the same list.
pub fn audit_path(environment: &EnvironmentSnapshot, me: Option<u32>) -> Vec<PathFinding> {
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
        // The entries are raw, as the shell gave them, so "absolute" is a
        // property of the entry itself and needs nothing resolved against it.
        // This function must never be handed redacted entries: a `~` it cannot
        // expand becomes a false `Relative` that skips every check below.
        if !entry.starts_with('/') {
            push(PathFindingKind::Relative);
            continue;
        }
        let absolute = PathBuf::from(entry);
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
        // Owner-writable is only a finding when the owner is *this* user.
        // Without the ownership half it would fire on `/usr/bin`, which is
        // `0755 root:wheel` and owner-writable like almost everything else, and
        // a check that fires on every entry says nothing about any of them.
        // With it, the entry is one this account can plant a binary in - which
        // is the shadowing case the other two kinds miss, because it needs no
        // unusual permissions at all.
        //
        // `me` is `None` when the home could not be resolved, and then this
        // check is skipped rather than guessed: reporting every owner-writable
        // directory, or none, would both be wrong, and a run with no `HOME`
        // already carries `HomeRedaction::Unresolved` through the report.
        if me.is_some_and(|me| meta.uid() == me) && mode & 0o200 != 0 {
            push(PathFindingKind::OwnerWritable);
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::os::unix::fs::PermissionsExt;
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
            installations: std::collections::BTreeMap::default(),
            path: entries.iter().map(|e| (*e).to_string()).collect(),
            reported: BTreeMap::new(),
            path_findings: Vec::new(),
        }
    }

    #[test]
    fn a_directory_this_user_owns_is_the_shadowing_case_the_other_kinds_miss() {
        // `~/bin` at 0755 owned by the user. No unusual permission is involved,
        // which is exactly why it produced no finding at all: a `curl | sh`
        // installer or an npm postinstall already runs as this account.
        let dir = scratch("owner-writable");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let entry = dir.to_string_lossy().into_owned();
        let me = std::fs::metadata(&dir).unwrap().uid();

        assert_eq!(
            kinds(&audit_path(&environment(&[&entry]), Some(me)), &entry),
            vec![PathFindingKind::OwnerWritable],
        );
        // Owned by somebody else, so this account cannot plant anything: the
        // ownership half is the whole check. Without it every entry on every
        // machine would report, `/usr/bin` at 0755 root:wheel included.
        assert!(kinds(&audit_path(&environment(&[&entry]), Some(me + 1)), &entry).is_empty());
        // And with no home to read a uid from, the check is skipped rather than
        // guessed in either direction.
        assert!(kinds(&audit_path(&environment(&[&entry]), None), &entry).is_empty());
    }

    #[test]
    fn a_directory_the_user_owns_but_cannot_write_is_not_reported() {
        let dir = scratch("owner-readonly");
        let me = std::fs::metadata(&dir).unwrap().uid();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let entry = dir.to_string_lossy().into_owned();
        let found = kinds(&audit_path(&environment(&[&entry]), Some(me)), &entry);
        // Put it back before asserting, so a failure does not leave an
        // unwritable directory behind in the temporary tree.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(found.is_empty(), "{found:?}");
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
    fn the_audit_sees_raw_entries_and_never_a_redacted_one() {
        // The contract this function now has, and the reason it has it.
        //
        // It used to take the home and expand a redacted `~` back into a path
        // to stat. That round trip broke in both directions: with no home to
        // expand against, every `~` entry became a false `Relative` and skipped
        // the writable checks entirely — on exactly the runs where `HOME` is
        // unset — and a username redacted outside the home turned a real
        // directory into a `USER` path that does not exist, reported as
        // `Missing`. The caller now audits the raw entries and redacts each
        // finding's `entry` afterwards, so neither is reachable.
        let home = scratch("audit-raw");
        let bin = home.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o777)).unwrap();
        let raw = bin.to_str().expect("path is UTF-8").to_string();

        // A raw absolute entry is inspected, and the mode is seen.
        assert!(
            kinds(&audit_path(&environment(&[&raw]), None), &raw)
                .contains(&PathFindingKind::WorldWritable),
            "a raw entry must be stat'd, not classified as relative"
        );

        // A redacted entry is not something this function can inspect, and it
        // says so rather than guessing. Handing it one is the caller's bug.
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
