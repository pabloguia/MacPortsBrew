// SPDX-License-Identifier: Apache-2.0

//! Which installed ports define a background service.
//!
//! MacPorts has no `port info --startupitem` (R-8, E-8 and Finding 10): the
//! `info` field vocabulary is the PortIndex's, and `startupitem` is not in
//! it. The offline, no-subprocess mechanism R-8 names first is the launch
//! machinery a StartupItem writes inside the prefix:
//! `${prefix}/etc/LaunchDaemons/org.macports.<name>/org.macports.<name>.plist`
//! (R-8 E-7, quoting `guide.macports.org/chunked/reference.startupitems.html`,
//! read 2026-09-02).
//!
//! Caveat, from R-8 Finding 11: the directory is named after
//! `startupitem.name`, which *defaults* to the subport but can be set
//! explicitly, so a name match is a strong signal rather than a proof.
//! `port contents <name>` is the authoritative form and costs one invocation
//! per port; this adapter does not spend that.

use super::names::MAX_ROWS;
use std::collections::BTreeSet;
use std::path::Path;

/// The prefix-relative directory a StartupItem writes into.
pub const SERVICE_DIRECTORY: &str = "etc/LaunchDaemons";

/// The prefix every StartupItem directory and plist carries.
pub const SERVICE_PREFIX: &str = "org.macports.";

/// The result of looking for service definitions under a prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceScan {
    /// The directory was read; these names define a service. An empty set
    /// means "read, and none of them do".
    Scanned(BTreeSet<String>),
    /// The directory could not be read, so nothing is known either way.
    /// Never collapsed into an empty set.
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

/// Scans `<prefix>/etc/LaunchDaemons` for MacPorts service definitions.
///
/// One directory read, no subprocess and no network. A directory counts only
/// when it also holds the plist the cited path pair names, so a leftover
/// empty directory is not read as a service.
pub fn scan(prefix: &Path) -> ServiceScan {
    let root = prefix.join(SERVICE_DIRECTORY);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return ServiceScan::Unreadable;
    };
    let mut found = BTreeSet::new();
    for entry in entries.flatten().take(MAX_ROWS) {
        let file_name = entry.file_name();
        let Some(dir_name) = file_name.to_str() else {
            continue;
        };
        let Some(port) = dir_name.strip_prefix(SERVICE_PREFIX) else {
            continue;
        };
        if port.is_empty() || !super::names::is_safe_name(port) {
            continue;
        }
        if root
            .join(dir_name)
            .join(format!("{dir_name}.plist"))
            .is_file()
        {
            found.insert(port.to_string());
        }
    }
    ServiceScan::Scanned(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn an_unreadable_directory_is_not_an_empty_one() {
        let scan = scan(&PathBuf::from("/nonexistent-prefix-for-a-test"));
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
}
