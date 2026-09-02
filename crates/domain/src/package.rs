// SPDX-License-Identifier: Apache-2.0

//! Packages as installed and as described by a provider.

use crate::provider::ProviderId;
use crate::version::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::path::PathBuf;

/// What kind of thing a provider calls a package. `GuiApp` is the Homebrew
/// cask concept; the other provider has no equivalent (R-1, SE-012).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PackageKind {
    /// A formula, a port: built or prebuilt software under the prefix.
    Package,
    /// A prebuilt, vendor-signed application (a cask). Out of migration scope.
    GuiApp,
}

/// A package name within one provider.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageIdentity {
    /// Owning provider.
    pub provider: ProviderId,
    /// Name as the provider spells it (`python@3.12`, `python312`).
    pub name: String,
}

impl PackageIdentity {
    /// Creates an identity.
    pub fn new(provider: ProviderId, name: impl Into<String>) -> Self {
        Self {
            provider,
            name: name.into(),
        }
    }
}

impl fmt::Display for PackageIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.provider, self.name)
    }
}

/// One installed row. A provider may report several rows for one name -
/// an active version and inactive ones (R-3) - so this is a row, not a
/// summary.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPackage {
    /// Which package.
    pub identity: PackageIdentity,
    /// Package or GUI application.
    pub kind: PackageKind,
    /// Installed version.
    pub version: Version,
    /// Provider-side revision, when the provider has the concept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Build variants or options selected at install time, sorted.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub variants: BTreeSet<String>,
    /// Whether this row is the active (linked, selected) one.
    pub active: bool,
    /// Whether the provider recorded this as installed on the user's
    /// request. `None` when the provider keeps no such record for this kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested: Option<bool>,
    /// Whether the prebuilt artifact was used at install time, when the
    /// provider records it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_from_prebuilt: Option<bool>,
    /// Where the provider says the package lives, when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_path: Option<PathBuf>,
}

/// A directed edge: `from` depends on `to`, both within one provider.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyEdge {
    /// Dependent.
    pub from: String,
    /// Dependency.
    pub to: String,
    /// Build-time only, when the provider distinguishes.
    #[serde(default)]
    pub build_only: bool,
}

/// Descriptive metadata about a package, provider-neutral.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageMetadata {
    /// One-line description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Upstream homepage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// Licence expression as the provider states it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Newest version the provider offers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_version: Option<Version>,
    /// Origin: a tap name, a source, a local file. `None` means the
    /// provider's core catalogue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Deprecated by the provider, with the reason it gives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// Disabled (no longer installable) with the reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<String>,
    /// The provider says another package replaces this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<String>,
    /// Pinned or held by the user.
    #[serde(default)]
    pub pinned: bool,
    /// Kept out of the default executable path by the provider's design,
    /// with the provider's stated reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_on_default_path_reason: Option<String>,
    /// The provider exposes a background service definition.
    #[serde(default)]
    pub defines_service: bool,
    /// Executables the package provides, sorted, when known.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub executables: BTreeSet<String>,
    /// Alternative names the provider accepts, sorted.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub aliases: BTreeSet<String>,
    /// Versioned siblings (`python@3.11`, `python@3.13`), sorted.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub versioned_siblings: BTreeSet<String>,
    /// Text the provider shows after install (caveats, notes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}
