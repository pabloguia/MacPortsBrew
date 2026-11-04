// SPDX-License-Identifier: Apache-2.0

//! What this provider observes for the destination checks (work order 16 part H).
//!
//! **Declared asymmetries (INV-17):** the list is `find` on the keg at call time
//! (R-25 finding 1), so a file absent from it is not detected; the owner is a rule
//! on the path and not a read (`Keg.for`, R-26 finding 8); a dependency record
//! exists to check. One judge above reads what comes out.
//!
//! Each call reads the document afresh on a fresh adapter: the registry builds
//! one per question, so nothing from before a step's install answers after it.

use super::{check_allowed, HomebrewAdapter, ARGV_LIST_FORMULA};
use crate::destination_reads::{self, CheckedList, Outside};
use crate::listing::is_under;
use macportsbrew_domain::{
    DependencyRecord, DestinationFacts, DestinationQuery, ExecutableName, InstallationRecord,
    IntegrityBasis, IntegrityFacts, LinkageFacts, OwnershipFacts, PackageIdentity, PrefixTable,
    UncheckedCause,
};
use std::path::{Component, Path, PathBuf};

/// The package and version a path lies in: `<cellar>/<name>/<version>/<rest>`,
/// with something in `<rest>`. Pure, on paths the caller has resolved; **never a
/// string prefix** (`Keg.for`; R-26 finding 8).
#[must_use]
pub fn keg_of(path: &Path, cellar: &Path) -> Option<(String, String)> {
    if !is_under(path, cellar) {
        return None;
    }
    let mut rest = path.strip_prefix(cellar).ok()?.components();
    let name = normal(rest.next()?)?;
    let version = normal(rest.next()?)?;
    rest.next()?;
    Some((name, version))
}

fn normal(component: Component<'_>) -> Option<String> {
    match component {
        Component::Normal(text) => text.to_str().map(str::to_owned),
        _ => None,
    }
}

/// `<keg>/INSTALL_RECEIPT.json` is listed, with at least one entry that is not
/// metadata (R-25 E-5, finding 17.6).
#[must_use]
pub fn installation_record(paths: &[PathBuf], keg: &Path) -> InstallationRecord {
    let receipt = keg.join("INSTALL_RECEIPT.json");
    let metadata = |path: &&PathBuf| {
        **path == receipt
            || is_under(path, &keg.join(".brew"))
            || *path == &keg.join("sbom.spdx.json")
    };
    if paths.contains(&receipt) && paths.iter().any(|p| !metadata(&p)) {
        InstallationRecord::Present
    } else {
        InstallationRecord::Absent
    }
}

impl HomebrewAdapter<'_> {
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

    /// The placed entry, never the keg's list path: `openssl@4` ships
    /// `bin/openssl` while `<prefix>/bin/openssl` is `openssl@3`'s (R-26 E-6).
    fn ownership(&self, pkg: &PackageIdentity, leaf: &ExecutableName) -> OwnershipFacts {
        let entry = match destination_reads::placed_entry(&self.prefix, leaf.as_str()) {
            Ok(Some(entry)) => entry,
            Ok(None) => return OwnershipFacts::NoEntry,
            Err(cause) => return OwnershipFacts::CouldNotTell(cause),
        };
        let Ok(cellar) = self.cellar() else {
            return OwnershipFacts::CouldNotTell(UncheckedCause::ReadFailed);
        };
        let Ok(real_cellar) = std::fs::canonicalize(&cellar) else {
            return OwnershipFacts::CouldNotTell(UncheckedCause::ReadFailed);
        };
        // A link to nothing is no entry on either provider (R-26 finding 22).
        let Ok(real) = std::fs::canonicalize(&entry) else {
            return OwnershipFacts::NoEntry;
        };
        let Some((name, version)) = keg_of(&real, &real_cellar) else {
            // `Keg.for` says no, and `Keg#unlink`'s one-hop rule may say yes: the
            // two disagree on `npm` and `npx`. The source is kept (R-26 finding 14).
            let first = destination_reads::first_hop(&entry);
            let in_keg = first
                .as_deref()
                .and_then(|hop| keg_of(hop, &cellar).or_else(|| keg_of(hop, &real_cellar)));
            return match in_keg {
                Some((owner, _)) => OwnershipFacts::OwnerLeavesItsDirectory { owner },
                None => OwnershipFacts::NotOwned,
            };
        };
        if name != pkg.name {
            return OwnershipFacts::Owner {
                name,
                version,
                active: None,
                versions_present: 0,
            };
        }
        let Ok(analysis) = self.analysis() else {
            return OwnershipFacts::CouldNotTell(UncheckedCause::ReadFailed);
        };
        if analysis.inventory.is_err() {
            return OwnershipFacts::CouldNotTell(UncheckedCause::ReadFailed);
        }
        let (active, versions_present) = analysis.kegs.get(&pkg.name).map_or((None, 0), |facts| {
            (facts.linked_keg.clone(), facts.versions.len())
        });
        OwnershipFacts::Owner {
            name,
            version,
            active,
            versions_present,
        }
    }

    /// The keg's list, and the keg, through every test before a `stat`.
    fn file_list(&self, pkg: &PackageIdentity) -> Result<(CheckedList, PathBuf), UncheckedCause> {
        let analysis = self.analysis().map_err(|_| UncheckedCause::ReadFailed)?;
        if analysis.inventory.is_err() {
            return Err(UncheckedCause::ReadFailed);
        }
        let facts = analysis
            .kegs
            .get(&pkg.name)
            .ok_or(UncheckedCause::FileListNotKnown)?;
        let version = facts
            .listed()
            .map_err(|_| UncheckedCause::FileListNotKnown)?;
        if !crate::process::is_brew_name(&pkg.name) {
            return Err(UncheckedCause::ReadFailed);
        }
        let keg = self
            .cellar()
            .map_err(|_| UncheckedCause::ReadFailed)?
            .join(&pkg.name)
            .join(version);
        let mut argv: Vec<String> = ARGV_LIST_FORMULA.iter().map(|a| (*a).to_string()).collect();
        argv.push(pkg.name.clone());
        check_allowed(&argv).map_err(|_| UncheckedCause::ReadFailed)?;
        let stdout = destination_reads::list_stdout(self.run_argv(argv))?;
        let list = destination_reads::checked_list(
            &stdout,
            "",
            "homebrew: `list --formula`",
            &keg,
            Outside::IsAnotherShape,
        )?;
        Ok((list, keg))
    }

    fn integrity(&self, pkg: &PackageIdentity) -> IntegrityFacts {
        let unusable = |cause| IntegrityFacts {
            basis: IntegrityBasis::GeneratedFileList,
            list_unusable: Some(cause),
            missing: 0,
            dangling: 0,
            stat_failed: 0,
            outside_prefix: 0,
            record: InstallationRecord::NotKept,
            dependencies: DependencyRecord::NotKept,
        };
        let (list, keg) = match self.file_list(pkg) {
            Ok(found) => found,
            Err(cause) => return unusable(cause),
        };
        let Ok(analysis) = self.analysis() else {
            return unusable(UncheckedCause::ReadFailed);
        };
        let counts = destination_reads::stat_counts(&list.paths);
        let dependencies =
            analysis
                .kegs
                .get(&pkg.name)
                .map_or(DependencyRecord::NotTold, |facts| {
                    match facts.listed().ok().and_then(|v| facts.dependencies.get(v)) {
                        Some(Some(names)) => {
                            if names.iter().all(|n| analysis.installed_names.contains(n)) {
                                DependencyRecord::AllPresent
                            } else {
                                DependencyRecord::Missing
                            }
                        }
                        Some(None) | None => DependencyRecord::NotTold,
                    }
                });
        IntegrityFacts {
            basis: IntegrityBasis::GeneratedFileList,
            list_unusable: None,
            missing: counts.missing,
            dangling: counts.dangling,
            stat_failed: counts.failed,
            outside_prefix: list.outside,
            record: installation_record(&list.paths, &keg),
            dependencies,
        }
    }

    fn linkage(&self, pkg: &PackageIdentity, prefixes: &PrefixTable) -> LinkageFacts {
        let (list, keg) = match self.file_list(pkg) {
            Ok(found) => found,
            Err(cause) => {
                return LinkageFacts {
                    list_unusable: Some(cause),
                    ..LinkageFacts::default()
                }
            }
        };
        let executables = [keg.join("bin"), keg.join("sbin")];
        destination_reads::examine(&list.paths, &executables, prefixes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keg_is_by_component_and_needs_something_inside_it() {
        let cellar = Path::new("/usr/local/Cellar");
        assert_eq!(
            keg_of(Path::new("/usr/local/Cellar/xz/5.8.3/bin/xz"), cellar),
            Some(("xz".to_owned(), "5.8.3".to_owned()))
        );
        assert_eq!(
            keg_of(Path::new("/usr/local/Cellar2/xz/5/bin/xz"), cellar),
            None
        );
        assert_eq!(
            keg_of(Path::new("/usr/local/Cellar/xz/5.8.3"), cellar),
            None
        );
        // R-26 E-6: `npm`'s final file is under the prefix and in no keg.
        assert_eq!(
            keg_of(Path::new("/usr/local/lib/node_modules/npm/bin/npm"), cellar),
            None
        );
        // The Caskroom is not a keg, whatever `owner_of` says of it.
        assert_eq!(keg_of(Path::new("/usr/local/Caskroom/a/1/b"), cellar), None);
    }

    #[test]
    fn a_keg_with_only_its_metadata_has_no_record() {
        let keg = Path::new("/c/x/1");
        let of = |names: &[&str]| -> Vec<PathBuf> { names.iter().map(|n| keg.join(n)).collect() };
        assert_eq!(
            installation_record(&of(&["INSTALL_RECEIPT.json", "bin/x"]), keg),
            InstallationRecord::Present
        );
        assert_eq!(
            installation_record(
                &of(&["INSTALL_RECEIPT.json", ".brew/x.rb", "sbom.spdx.json"]),
                keg
            ),
            InstallationRecord::Absent
        );
        assert_eq!(
            installation_record(&of(&["bin/x"]), keg),
            InstallationRecord::Absent
        );
    }
}
