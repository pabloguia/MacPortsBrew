// SPDX-License-Identifier: Apache-2.0

//! Which installed ports define a background service.
//!
//! MacPorts has no `port info --startupitem` (R-8, E-8 and Finding 10): the
//! `info` field vocabulary is the PortIndex's, and `startupitem` is not in
//! it. The offline mechanism R-8 names first is the launch machinery a
//! StartupItem writes inside the prefix, read here and attributed with one
//! `port -q provides` per item found (none on an install with no item):
//! `${prefix}/etc/<location>/<uniquename>/<plist>` (R-8 E-7). MacPorts 2.12.6
//! `port1.0/portstartupitem.tcl` (sha256
//! `cc4d2c99b1e303d4677f9d562f8c3a9f076bea2d8e28263f099b805f0ee592b3`, read
//! 2026-10-10): the location defaults to `LaunchDaemons` and launchd loads it
//! from `LaunchDaemons` or `LaunchAgents` (lines 108, 290, 546-560); the
//! uniquename defaults to `org.macports.<subport>` and the plist to
//! `<uniquename>.plist`, every one of them overridable per port, and a port may
//! declare several items (lines 112-121, 158-166). The directory is created
//! only when an item is (lines 130-133, 513).
//!
//! **Attributed by the registry.** Each item directory must hold a plist of
//! the same name, and `port provides` must name the installed port that owns
//! that plist; the item is that port's, whatever it is called (`dbus`'s
//! `org.freedesktop.dbus-system`, a subport's parent-named item). The name is
//! never trusted on its own (security review, pass 08 slice 3, M1).
//!
//! **Fail-closed.** This answers "these ports define a service" only when the
//! whole answer can be attributed. A stray entry, a directory without its
//! plist, a plist the registry does not own or attributes to a port that is not
//! installed, an entry that cannot be read or named, or more entries than are
//! ever expected makes the answer **unknown** — never "not a service" —
//! because a service reported as none is a package INV-19 would let through
//! (security review, pass 08 slice 3, H1).
//! An absent location is "no item there" only under a real MacPorts prefix,
//! and only when every installed port is active: deactivating a port deletes
//! its item directory (`registry2.0/portimage.tcl` `_deactivate_files`), so the
//! absence proves nothing about an inactive one (spec-guardian, H1).
//!
//! What it answers is the item MacPorts *materialised*. A port built with
//! `startupitem.type none` materialises none and is reported as defining none;
//! Homebrew answers what a formula *declares*. Whether the two must agree is
//! DP-032.

use super::names::MAX_ROWS;
use crate::filesystem::FilesystemScope;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The prefix-relative directories launchd reads a StartupItem from.
pub const SERVICE_DIRECTORIES: &[&str] = &["etc/LaunchDaemons", "etc/LaunchAgents"];

/// The prefix-relative directory every MacPorts installation has, whose
/// presence says the prefix is MacPorts' and not an empty or wrong tree.
pub const MACPORTS_CONFIGURATION_DIRECTORY: &str = "etc/macports";

/// The prefix every default StartupItem directory and plist carries.
pub const SERVICE_PREFIX: &str = "org.macports.";

/// The result of looking for service definitions under a prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceScan {
    /// Every item was read and attributed; these names define a service. An
    /// empty set means "read, and none of them do".
    Scanned(BTreeSet<String>),
    /// No answer: something could not be read or attributed, a port is
    /// inactive, or this host's filesystem is not the machine being described
    /// (INV-15, F-25). Never collapsed into an empty set — a scan that did not
    /// succeed is not a scan that found nothing.
    Unreadable,
}

impl ServiceScan {
    /// Whether `name` defines a service. `false` for an unreadable scan, in
    /// which case the caller reports the capability as unknown rather than
    /// as absent.
    pub fn defines_service(&self, name: &str) -> bool {
        match self {
            ServiceScan::Scanned(names) => names.contains(name),
            ServiceScan::Unreadable => false,
        }
    }

    /// Whether the scan produced an answer at all.
    pub fn is_known(&self) -> bool {
        matches!(self, ServiceScan::Scanned(_))
    }
}

/// Which installed port owns a file, as the provider's own registry answers
/// it (`port provides`), or `None` when it cannot say.
pub type Owner<'a> = &'a dyn Fn(&Path) -> Option<String>;

/// Scans the prefix's StartupItem locations, attributing every item to one of
/// `installed` (each port name, and whether any of its installed versions is
/// active). One directory read per location, no network; `owner` is asked
/// about each item's plist, and the item is attributed to the port the
/// registry says installed it - never to the port its name resembles, because
/// a subport may set `startupitem.name` to its parent's name (`mysql5-server`
/// writes `org.macports.mysql5`; security review, pass 08 slice 3, M1), and a
/// port may name its item outside `org.macports.` altogether (`dbus`).
pub fn scan(
    prefix: &Path,
    filesystem: FilesystemScope,
    installed: &BTreeMap<String, bool>,
    owner: Owner<'_>,
) -> ServiceScan {
    if !filesystem.is_this_machine() {
        return ServiceScan::Unreadable;
    }
    // A real MacPorts prefix, decided without following a symbolic reference.
    let is_real_directory = |relative: &str| {
        std::fs::symlink_metadata(prefix.join(relative)).is_ok_and(|m| m.file_type().is_dir())
    };
    if !is_real_directory("etc") || !is_real_directory(MACPORTS_CONFIGURATION_DIRECTORY) {
        return ServiceScan::Unreadable;
    }
    // An inactive port's item directory is deleted with its files.
    if installed.values().any(|active| !active) {
        return ServiceScan::Unreadable;
    }
    let mut found = BTreeSet::new();
    for location in SERVICE_DIRECTORIES {
        match scan_location(&prefix.join(location), installed, owner) {
            Some(names) => found.extend(names),
            None => return ServiceScan::Unreadable,
        }
    }
    ServiceScan::Scanned(found)
}

/// The installed ports with an item in `root`, or `None` when anything there
/// cannot be attributed. An absent location contributes nothing.
fn scan_location(
    root: &Path,
    installed: &BTreeMap<String, bool>,
    owner: Owner<'_>,
) -> Option<BTreeSet<String>> {
    match std::fs::symlink_metadata(root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(BTreeSet::new()),
        Err(_) => return None,
        Ok(meta) if !meta.file_type().is_dir() => return None,
        Ok(_) => {}
    }
    let mut found = BTreeSet::new();
    for (seen, entry) in std::fs::read_dir(root).ok()?.enumerate() {
        if seen >= MAX_ROWS {
            return None;
        }
        let entry = entry.ok()?;
        let dir_name = entry.file_name().into_string().ok()?;
        let item = entry.path();
        let real_dir = std::fs::symlink_metadata(&item).is_ok_and(|m| m.file_type().is_dir());
        let plist = item.join(format!("{dir_name}.plist"));
        let real_plist = std::fs::symlink_metadata(&plist).is_ok_and(|m| m.file_type().is_file());
        if !real_dir || !real_plist {
            return None;
        }
        // The port the registry says installed the plist, whatever the item is
        // named: `org.macports.<port>` by default, `org.freedesktop.*` for
        // `dbus`, a parent's name for a subport.
        let port = owner(&plist)?;
        if !installed.contains_key(&port) || !super::names::is_safe_name(&port) {
            return None;
        }
        found.insert(port);
    }
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn an_unreadable_directory_is_not_an_empty_one() {
        let scan = scan(
            &PathBuf::from("/nonexistent-prefix-for-a-test"),
            FilesystemScope::ThisMachine,
            &BTreeMap::new(),
            &by_name,
        );
        assert_eq!(scan, ServiceScan::Unreadable);
        assert!(!scan.is_known());
        assert!(!scan.defines_service("nginx"));
    }

    #[test]
    fn a_scanned_set_answers_per_name() {
        let scan = ServiceScan::Scanned(["nginx".to_string()].into_iter().collect());
        assert!(scan.is_known());
        assert!(scan.defines_service("nginx"));
        assert!(!scan.defines_service("wget"));
    }

    /// A registry that agrees with every item's name.
    fn by_name(plist: &Path) -> Option<String> {
        let file = plist.file_name()?.to_str()?;
        Some(
            file.strip_prefix(SERVICE_PREFIX)?
                .strip_suffix(".plist")?
                .to_string(),
        )
    }

    fn scratch(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("mpb-service-unit-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn a_location_that_cannot_be_inspected_is_unknown_and_not_absent() {
        use std::os::unix::fs::PermissionsExt;
        let parent = scratch("uninspectable");
        let locked = parent.join("locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o600)).unwrap();
        let answer = scan_location(&locked.join("LaunchDaemons"), &BTreeMap::new(), &by_name);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(answer, None, "a permission error is not an absent location");
        assert_eq!(
            scan_location(&parent.join("absent"), &BTreeMap::new(), &by_name),
            Some(BTreeSet::new())
        );
        std::fs::remove_dir_all(&parent).ok();
    }

    #[test]
    fn more_items_than_are_ever_expected_is_unknown_not_truncated() {
        let root = scratch("too-many");
        let location = root.join("LaunchDaemons");
        let mut installed = BTreeMap::new();
        for n in 0..=MAX_ROWS {
            let name = format!("p{n}");
            let dir = location.join(format!("{SERVICE_PREFIX}{name}"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{SERVICE_PREFIX}{name}.plist")), "").unwrap();
            installed.insert(name, true);
        }
        assert_eq!(scan_location(&location, &installed, &by_name), None);
        std::fs::remove_dir_all(&root).ok();
    }
}
