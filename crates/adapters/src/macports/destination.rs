// SPDX-License-Identifier: Apache-2.0

//! What this provider observes for the destination checks (work order 16 part H).
//!
//! **Declared asymmetries (INV-17):** the list is a stored record (R-25 finding
//! 1), so a missing file is found; the owner is a query, one path per call
//! (R-26); no readable record of this installation's dependencies exists (R-25
//! finding 14); a listed path outside the prefix is legitimate and not tested
//! (R-25 finding 7). One judge above reads what comes out.

use super::parse::{self, InstalledRow};
use super::{names, MacPortsAdapter, ARGV_CONTENTS, ARGV_INSTALLED, ARGV_PROVIDES};
use crate::destination_reads::{self, CheckedList, Outside};
use crate::listing::is_under;
use crate::process::ProcessRunner;
use macportsbrew_domain::{
    DependencyRecord, DestinationFacts, DestinationQuery, ExecutableName, InstallationRecord,
    IntegrityBasis, IntegrityFacts, LinkageFacts, OwnershipFacts, PackageIdentity, PrefixTable,
    UncheckedCause,
};
use std::path::Path;

/// The strict reading of `port -q provides <one path>` (R-26 E-3, finding 22):
/// exactly zero lines, or one line that is a port name. Anything else is a shape
/// this does not know, which is not "not owned". `RESOLVES_TO`'s reader stays as
/// it was, with its own judge.
pub fn parse_one_owner(stdout: &str) -> Result<Option<String>, UncheckedCause> {
    let body = stdout.strip_suffix('\n').unwrap_or(stdout);
    if body.is_empty() {
        return Ok(None);
    }
    if body.contains('\n') || !names::is_safe_name(body) {
        return Err(UncheckedCause::ReadFailed);
    }
    Ok(Some(body.to_owned()))
}

/// A row's version as this adapter compares it: `@5.8.3_0` is `5.8.3_0`. The
/// variants are not part of it, so two rows of one version are still two rows.
#[must_use]
pub fn row_version(row: &InstalledRow) -> String {
    match &row.revision {
        Some(revision) => format!("{}_{revision}", row.version),
        None => row.version.clone(),
    }
}

impl<R: ProcessRunner> MacPortsAdapter<R> {
    /// The answer for `query`, on this machine only.
    pub(super) fn observed(
        &self,
        pkg: &PackageIdentity,
        query: &DestinationQuery<'_>,
    ) -> DestinationFacts {
        if !self.filesystem.is_this_machine() || self.runner.is_replay() {
            return DestinationFacts::NotObserved;
        }
        match query {
            DestinationQuery::Ownership { leaf } => {
                DestinationFacts::Ownership(self.ownership(pkg, leaf))
            }
            DestinationQuery::Integrity => DestinationFacts::Integrity(self.integrity(pkg)),
            DestinationQuery::Linkage { prefixes } => {
                DestinationFacts::Linkage(self.linkage(pkg, prefixes))
            }
        }
    }

    /// Who the registry says owns one path. **A failed read is not "not owned"**
    /// (`provides` conflates them, R-26 E-8).
    fn owner_read(&self, path: &Path) -> Result<Option<String>, UncheckedCause> {
        if !names::is_safe_path(path) {
            return Err(UncheckedCause::ReadFailed);
        }
        let text = path.to_str().ok_or(UncheckedCause::ReadFailed)?;
        let mut argv: Vec<String> = ARGV_PROVIDES.iter().map(|s| (*s).to_string()).collect();
        argv.push(text.to_owned());
        let invocation = self
            .run_argv(argv)
            .map_err(|_| UncheckedCause::ReadFailed)?;
        let invocation = invocation
            .require_success()
            .map_err(|_| UncheckedCause::ReadFailed)?;
        parse_one_owner(&invocation.stdout)
    }

    /// The rows of `port -q installed <name>`, fresh.
    fn installed_rows(&self, name: &str) -> Result<Vec<InstalledRow>, UncheckedCause> {
        if !crate::process::is_port_name(name) {
            return Err(UncheckedCause::ReadFailed);
        }
        let mut argv: Vec<String> = ARGV_INSTALLED.iter().map(|s| (*s).to_string()).collect();
        argv.push(name.to_owned());
        let invocation = self
            .run_argv(argv)
            .map_err(|_| UncheckedCause::ReadFailed)?;
        let invocation = invocation
            .require_success()
            .map_err(|_| UncheckedCause::ReadFailed)?;
        let rows =
            parse::parse_installed(&invocation.stdout).map_err(|_| UncheckedCause::ReadFailed)?;
        Ok(rows.into_iter().filter(|row| row.name == name).collect())
    }

    /// The entry, and what it finally resolves to: both must answer the package,
    /// and the answer is compared, never joined (R-26 findings 1, 20, 22).
    fn ownership(&self, pkg: &PackageIdentity, leaf: &ExecutableName) -> OwnershipFacts {
        let entry = match destination_reads::placed_entry(&self.prefix, leaf.as_str()) {
            Ok(Some(entry)) => entry,
            Ok(None) => return OwnershipFacts::NoEntry,
            Err(cause) => return OwnershipFacts::CouldNotTell(cause),
        };
        // Existence is a filesystem read: an empty answer means several things,
        // and a dangling registered link would pass (R-26 finding 20).
        let Ok(real) = std::fs::canonicalize(&entry) else {
            return OwnershipFacts::NoEntry;
        };
        let real_prefix =
            std::fs::canonicalize(&self.prefix).unwrap_or_else(|_| self.prefix.clone());
        if !is_under(&real, &real_prefix) {
            return OwnershipFacts::NotOwned;
        }
        let owner = match self.owner_read(&entry) {
            Ok(Some(owner)) => owner,
            Ok(None) => return OwnershipFacts::NotOwned,
            Err(cause) => return OwnershipFacts::CouldNotTell(cause),
        };
        let another = |name: String| OwnershipFacts::Owner {
            name,
            version: String::new(),
            active: None,
            versions_present: 0,
        };
        if owner != pkg.name {
            return another(owner);
        }
        if real != entry {
            match self.owner_read(&real) {
                Ok(Some(last)) if last == pkg.name => {}
                Ok(Some(last)) => return another(last),
                Ok(None) => return OwnershipFacts::NotOwned,
                Err(cause) => return OwnershipFacts::CouldNotTell(cause),
            }
        }
        let rows = match self.installed_rows(&pkg.name) {
            Ok(rows) => rows,
            Err(cause) => return OwnershipFacts::CouldNotTell(cause),
        };
        let active = rows.iter().find(|row| row.active).map(row_version);
        // `provides` is active only, so the owner is the active image, within the
        // window between the two reads (R-26 finding 5). It names no version of
        // its own, so an absent active row fails on the comparison below.
        OwnershipFacts::Owner {
            name: owner,
            version: active.clone().unwrap_or_default(),
            active,
            versions_present: rows.len(),
        }
    }

    /// The port's list, through every test before a `stat`. Only for a port with
    /// one version, active, as the executable list does (R-24; SG F-6).
    fn file_list(&self, pkg: &PackageIdentity) -> Result<CheckedList, UncheckedCause> {
        let rows = self.installed_rows(&pkg.name)?;
        if rows.len() != 1 || !rows.iter().all(|row| row.active) {
            return Err(UncheckedCause::FileListNotKnown);
        }
        let mut argv: Vec<String> = ARGV_CONTENTS.iter().map(|s| (*s).to_string()).collect();
        argv.push(pkg.name.clone());
        let stdout = destination_reads::list_stdout(self.run_argv(argv))?;
        destination_reads::checked_list(
            &stdout,
            "  ",
            "macports: `-q contents`",
            &self.prefix,
            Outside::IsNotTested,
        )
    }

    fn integrity(&self, pkg: &PackageIdentity) -> IntegrityFacts {
        let mut facts = IntegrityFacts {
            basis: IntegrityBasis::RecordedFileList,
            list_unusable: None,
            missing: 0,
            dangling: 0,
            stat_failed: 0,
            outside_prefix: 0,
            record: InstallationRecord::NotKept,
            dependencies: DependencyRecord::NotKept,
        };
        match self.file_list(pkg) {
            Err(cause) => facts.list_unusable = Some(cause),
            Ok(list) => {
                let counts = destination_reads::stat_counts(&list.paths);
                facts.missing = counts.missing;
                facts.dangling = counts.dangling;
                facts.stat_failed = counts.failed;
                facts.outside_prefix = list.outside;
            }
        }
        facts
    }

    fn linkage(&self, pkg: &PackageIdentity, prefixes: &PrefixTable) -> LinkageFacts {
        match self.file_list(pkg) {
            Err(cause) => LinkageFacts {
                list_unusable: Some(cause),
                ..LinkageFacts::default()
            },
            Ok(list) if list.outside > 0 => LinkageFacts {
                // What lies outside was not tested, so it cannot be called clean.
                list_unusable: Some(UncheckedCause::ListsOutsidePrefix),
                ..destination_reads::examine(
                    &list.paths,
                    &[self.prefix.join("bin"), self.prefix.join("sbin")],
                    prefixes,
                )
            },
            Ok(list) => destination_reads::examine(
                &list.paths,
                &[self.prefix.join("bin"), self.prefix.join("sbin")],
                prefixes,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owner_read_is_one_name_or_nothing() {
        // R-26 E-3, the forms `-q` prints.
        assert_eq!(parse_one_owner("xz\n"), Ok(Some("xz".to_owned())));
        assert_eq!(parse_one_owner(""), Ok(None));
        // Not `-q`'s form, and not a name, so not a guess either way.
        assert_eq!(
            parse_one_owner("/opt/local/bin/xz is provided by: xz\n"),
            Err(UncheckedCause::ReadFailed)
        );
        assert_eq!(
            parse_one_owner("xz\nzlib\n"),
            Err(UncheckedCause::ReadFailed)
        );
    }

    #[test]
    fn a_row_version_carries_its_revision_and_not_its_variants() {
        let rows = parse::parse_installed("  xz @5.8.3_0+universal (active)\n").unwrap();
        assert_eq!(row_version(&rows[0]), "5.8.3_0");
    }
}
