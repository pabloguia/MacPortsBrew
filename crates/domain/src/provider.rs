// SPDX-License-Identifier: Apache-2.0

//! The provider abstraction and its read contract.
//!
//! A provider is identified by an opaque id, never by an enum: the charter
//! forbids hardcoding "two providers", and INV-17 forbids either direction
//! being a special case. The known ids are constants in the adapters that
//! implement them; the domain treats every id alike.

use crate::availability::AvailabilityAssessment;
use crate::error::DomainError;
use crate::machine::Machine;
use crate::package::{DependencyEdge, InstalledPackage, PackageIdentity, PackageMetadata};
use crate::resolution::Resolution;
use crate::snapshot::ProviderSnapshot;
use crate::version::Version;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

/// Stable identifier of a provider (`homebrew`, `macports`, ...).
///
/// Lower-case ASCII, used verbatim in JSON output, snapshots and policy data.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    /// Creates an id. The text is normalised to lower case.
    pub fn new(id: impl AsRef<str>) -> Self {
        Self(id.as_ref().trim().to_ascii_lowercase())
    }

    /// The id text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The four presence states of `02-domain-model.md` plus the broken one,
/// which that chapter makes its own state rather than "absent".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "state")]
pub enum ProviderPresence {
    /// The executable was found and answered.
    Present,
    /// Nothing of the provider was found.
    Absent,
    /// Something of the provider exists but it does not work; `doctor` is
    /// the next step. The reason is a stable identifier.
    Broken {
        /// Why it is considered broken (an [`crate::ErrorId`] text).
        reason: String,
    },
}

/// What `detect()` establishes about an installation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDetection {
    /// Which provider.
    pub provider: ProviderId,
    /// Presence state.
    pub presence: ProviderPresence,
    /// Provider version, when it answered.
    pub version: Option<Version>,
    /// Installation prefix, when known.
    pub prefix: Option<PathBuf>,
    /// The executable that was actually used, resolved from the prefix rather
    /// than from `PATH` (threat model: executable shadowing).
    pub executable: Option<PathBuf>,
    /// The architecture the installation is built for, when detectable.
    pub architecture: Option<crate::machine::Architecture>,
}

/// One capability a provider's installed version may or may not offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Capability {
    /// Machine-readable inventory (JSON or field-selected output).
    MachineReadableInventory,
    /// The provider records user intent (requested vs dependency).
    RequestedRecord,
    /// Dependency queries.
    Dependencies,
    /// Reverse-dependency queries.
    ReverseDependencies,
    /// Prebuilt-artifact availability can be assessed without installing.
    ArtifactAvailability,
    /// Metadata answers without the network (a local index or cache).
    OfflineMetadata,
    /// The provider can say which package owns a file.
    FileOwnership,
    /// Deprecation / disabled / replaced state is exposed.
    DeprecationState,
    /// Background services are exposed in metadata.
    ServiceMetadata,
}

/// Whether a capability was detected, absent, or could not be determined.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "state")]
pub enum CapabilityState {
    /// Detected on this installation.
    Available,
    /// Detected as absent; the reason is a stable identifier.
    Unavailable {
        /// Why (an [`crate::ErrorId`] or reason text).
        reason: String,
    },
    /// Detection was not possible; never treated as either of the above.
    Unknown,
}

/// The capability table of one installation. Detected, not assumed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    /// Per-capability state.
    pub capabilities: BTreeMap<Capability, CapabilityState>,
}

impl ProviderCapabilities {
    /// True when the capability was detected as available.
    pub fn has(&self, cap: Capability) -> bool {
        matches!(
            self.capabilities.get(&cap),
            Some(CapabilityState::Available)
        )
    }
}

/// The provider's own record of what the user asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestedSet {
    /// Names the provider records as explicitly requested.
    pub names: BTreeSet<String>,
    /// Which provider mechanism produced this (for example a JSON field or a
    /// pseudo-portname), so the confidence can be judged.
    pub mechanism: String,
}

/// Everything the read contract learns about one provider, in one value.
///
/// Adapters fill it from however many commands their provider needs;
/// application code reads it uniformly. It is also what a snapshot stores.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInventory {
    /// Installed rows. A name may appear more than once: one provider keeps
    /// inactive versions installed alongside the active one (R-3).
    pub installed: Vec<InstalledPackage>,
    /// What the user asked for, from the provider's own record.
    pub requested: RequestedSet,
    /// Dependency edges between installed packages, sorted.
    pub dependencies: Vec<DependencyEdge>,
    /// Metadata per package name.
    pub metadata: BTreeMap<String, PackageMetadata>,
    /// Artifact availability per package name, as assessed at read time.
    pub availability: BTreeMap<String, AvailabilityAssessment>,
}

impl ProviderInventory {
    /// Names that are installed (active or not), deduplicated and sorted.
    pub fn installed_names(&self) -> BTreeSet<String> {
        self.installed
            .iter()
            .map(|p| p.identity.name.clone())
            .collect()
    }

    /// Direct dependencies of `name`, sorted.
    pub fn dependencies_of(&self, name: &str) -> BTreeSet<String> {
        self.dependencies
            .iter()
            .filter(|e| e.from == name)
            .map(|e| e.to.clone())
            .collect()
    }

    /// Direct reverse dependencies of `name` within this provider (INV-04:
    /// never across providers), sorted.
    pub fn reverse_dependencies_of(&self, name: &str) -> BTreeSet<String> {
        self.dependencies
            .iter()
            .filter(|e| e.to == name)
            .map(|e| e.from.clone())
            .collect()
    }

    /// Transitive reverse dependencies, sorted, excluding `name` itself.
    pub fn transitive_reverse_dependencies_of(&self, name: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![name.to_string()];
        while let Some(current) = stack.pop() {
            for parent in self.reverse_dependencies_of(&current) {
                if parent != name && seen.insert(parent.clone()) {
                    stack.push(parent);
                }
            }
        }
        seen
    }
}

/// The read contract every provider implements (`02-domain-model.md`).
///
/// The foundation pass implements only this contract. The write contract
/// (install, uninstall, cleanup, verify) is a later pass and is deliberately
/// not declared here, so that no shipped code path can call it.
pub trait ProviderRead {
    /// The provider's id.
    fn id(&self) -> &ProviderId;

    /// Presence, version, prefix, architecture of the installation.
    fn detect(&self) -> Result<ProviderDetection, DomainError>;

    /// What this installed version can actually do.
    fn capabilities(&self) -> Result<ProviderCapabilities, DomainError>;

    /// Everything installed, plus requested, dependencies, metadata and
    /// availability. Adapters may cache; the result must not change between
    /// calls within one analysis.
    fn inventory(&self) -> Result<ProviderInventory, DomainError>;

    /// The user's explicit requests (authoritative, not inferred).
    fn requested(&self) -> Result<RequestedSet, DomainError> {
        Ok(self.inventory()?.requested)
    }

    /// Direct dependencies of a package.
    fn dependencies(&self, pkg: &PackageIdentity) -> Result<BTreeSet<String>, DomainError> {
        Ok(self.inventory()?.dependencies_of(&pkg.name))
    }

    /// Direct reverse dependencies of a package inside this provider.
    fn reverse_dependencies(&self, pkg: &PackageIdentity) -> Result<BTreeSet<String>, DomainError> {
        Ok(self.inventory()?.reverse_dependencies_of(&pkg.name))
    }

    /// Metadata for one package, when known.
    fn describe(&self, pkg: &PackageIdentity) -> Result<Option<PackageMetadata>, DomainError> {
        Ok(self.inventory()?.metadata.get(&pkg.name).cloned())
    }

    /// Prebuilt-artifact availability for one package on this machine.
    fn artifact_availability(
        &self,
        pkg: &PackageIdentity,
        machine: &Machine,
    ) -> Result<AvailabilityAssessment, DomainError>;

    /// Which package, if any, owns an executable path.
    fn resolution(&self, executable: &Path) -> Result<Resolution, DomainError>;

    /// The environment variables and paths this provider's prefix implies,
    /// reported (never managed) per `05-environment-and-primary-provider.md`.
    fn environment_info(&self) -> Result<BTreeMap<String, String>, DomainError>;

    /// The serialisable state of this provider for `analyze --from-snapshot`.
    fn snapshot(&self) -> Result<ProviderSnapshot, DomainError>;
}
