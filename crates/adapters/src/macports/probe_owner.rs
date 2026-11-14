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
//! The owner rule is a query on this provider, one path per call (R-26): the
//! entry **and** the file it resolves to are each asked, and each must name the
//! package. An empty answer is not the package's (R-26 E-3), and a failed read is
//! not "not owned" (R-26 E-8). The asymmetry with the peer's path rule is the
//! provider's, declared here (INV-17).

use super::{MacPortsAdapter, PROVIDER_ID};
use crate::process::{ProbeOwner, ProcessRunner};
use macportsbrew_domain::{
    ExecutableName, OwnershipFacts, PackageIdentity, ProviderId, UncheckedCause,
};
use std::path::Path;

impl<R: ProcessRunner> ProbeOwner for MacPortsAdapter<R> {
    fn provider(&self) -> &str {
        PROVIDER_ID
    }

    fn is_this_machine(&self) -> bool {
        self.filesystem.is_this_machine()
    }

    fn runner(&self) -> &dyn ProcessRunner {
        &self.runner
    }

    fn prefix(&self) -> &Path {
        &self.prefix
    }

    /// The same rule `EXECUTABLES_OWNED_BY_DESTINATION` reads, on `placed`.
    fn entry_owner(&self, package: &str, _leaf: &ExecutableName, placed: &Path) -> OwnershipFacts {
        let pkg = PackageIdentity::new(ProviderId::new(PROVIDER_ID), package);
        self.entry_ownership(&pkg, placed, None)
    }

    /// The registry names `package` for `placed` and for `canonical`, each asked
    /// on its own and compared, never joined.
    fn target_is_packages(
        &self,
        package: &str,
        placed: &Path,
        canonical: &Path,
    ) -> Result<bool, UncheckedCause> {
        let names_package = |path: &Path| -> Result<bool, UncheckedCause> {
            Ok(self.registered_owner(path)?.as_deref() == Some(package))
        };
        Ok(names_package(placed)? && names_package(canonical)?)
    }
}
