// SPDX-License-Identifier: Apache-2.0

//! What this adapter answers about itself for a curated liveness probe
//! (work order 16 part H-3; `docs/passes/16-liveness-design.md`).
//!
//! **Dangerous to change** (AGENTS.md): the admission's conditions 1, 4 and 6-8
//! are only as good as these answers. Every one comes from the adapter's own
//! fields: its id, its scope, its runner and the prefix detection found. Nothing
//! here takes a prefix, a table or a provider from a caller, and nothing here is
//! reachable from `crates/*/tests`.
//!
//! The owner rule is a path rule on this provider (`Keg.for`, R-26 finding 8):
//! the entry and the file it resolves to are judged by where they lie, inside
//! one keg of the package, never by a string prefix.

use super::destination::keg_of;
use super::{HomebrewAdapter, PROVIDER_ID};
use crate::process::{ProbeOwner, ProcessRunner};
use macportsbrew_domain::{
    ExecutableName, OwnershipFacts, PackageIdentity, ProviderId, UncheckedCause,
};
use std::path::Path;

impl ProbeOwner for HomebrewAdapter<'_> {
    fn provider(&self) -> &str {
        PROVIDER_ID
    }

    fn is_this_machine(&self) -> bool {
        self.filesystem.is_this_machine()
    }

    fn runner(&self) -> &dyn ProcessRunner {
        self.runner
    }

    fn prefix(&self) -> &Path {
        &self.prefix
    }

    /// The same rule `EXECUTABLES_OWNED_BY_DESTINATION` reads, on `placed`.
    fn entry_owner(&self, package: &str, _leaf: &ExecutableName, placed: &Path) -> OwnershipFacts {
        let pkg = PackageIdentity::new(ProviderId::new(PROVIDER_ID), package);
        self.entry_ownership(&pkg, placed, None)
    }

    /// `canonical` lies in a keg of `package` under the canonical Cellar: not the
    /// Caskroom, not `var/`, not the Homebrew repository, not another keg. The
    /// path rule needs only the final file, which is what `placed` resolves to.
    fn target_is_packages(
        &self,
        package: &str,
        _placed: &Path,
        canonical: &Path,
    ) -> Result<bool, UncheckedCause> {
        let cellar = self.cellar().map_err(|_| UncheckedCause::ReadFailed)?;
        let real_cellar = std::fs::canonicalize(cellar).map_err(|_| UncheckedCause::ReadFailed)?;
        Ok(keg_of(canonical, &real_cellar).is_some_and(|(name, _)| name == package))
    }
}

/// A test's way to give this adapter its kegs, `(name, linked version)`, so the
/// owner rule can be asked of a scratch tree without an inventory read. Its tests
/// live in `process/probe/tests.rs`, beside the admission they serve.
#[cfg(test)]
impl HomebrewAdapter<'_> {
    pub(crate) fn with_linked_kegs_for_test(self, kegs: &[(&str, &str)]) -> Self {
        use super::{Analysis, KegFacts};
        use macportsbrew_domain::{ProviderCapabilities, ProviderInventory};
        *self.analysis.borrow_mut() = Some(Analysis {
            inventory: Ok(ProviderInventory::default()),
            capabilities: ProviderCapabilities::default(),
            kegs: kegs
                .iter()
                .map(|(name, version)| {
                    (
                        (*name).to_owned(),
                        KegFacts {
                            versions: vec![(*version).into()],
                            linked_keg: Some((*version).into()),
                            keg_only: None,
                            dependencies: std::collections::BTreeMap::new(),
                        },
                    )
                })
                .collect(),
            casks: std::collections::BTreeSet::new(),
            installed_names: std::collections::BTreeSet::new(),
        });
        self
    }
}
