// SPDX-License-Identifier: Apache-2.0

//! The analysis output: the unit shown to the user is the *package across
//! providers*, not the provider row (`02-domain-model.md`, "Inventory").
//! This is the versioned `--json` payload of `analyze`, `list` and `status`
//! (`docs/schemas/analysis.v1.md`).

use macportsbrew_domain::{
    AvailabilityAssessment, ClassSet, InstalledPackage, Ownership, OwnershipEvidence,
    PackageMapping, ProviderCapabilities, ProviderDetection, ProviderId, Recommendation,
    Resolution, SupportState, Version,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What one provider reports about one package unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRow {
    /// The name under this provider.
    pub name: String,
    /// Installed rows (active and inactive versions).
    pub installed: Vec<InstalledPackage>,
    /// Ownership under this provider.
    pub ownership: Ownership,
    /// How ownership was decided.
    pub ownership_evidence: OwnershipEvidence,
    /// Newest version the provider offers, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_version: Option<Version>,
    /// Artifact availability on this machine.
    pub availability: AvailabilityAssessment,
    /// Deprecated by the provider.
    #[serde(default)]
    pub deprecated: bool,
    /// Disabled by the provider.
    #[serde(default)]
    pub disabled: bool,
    /// Direct dependency count within this provider (summarised, INV-13).
    pub dependency_count: u32,
    /// Direct reverse-dependency count within this provider.
    pub reverse_dependency_count: u32,
    /// How this row came to be in the unit: `installed` or `mapped`.
    pub presence: RowPresence,
}

/// Whether the row is an installed package or a mapping target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RowPresence {
    /// Installed under this provider.
    Installed,
    /// Not installed; the name is the selected mapping candidate.
    Mapped,
}

/// One package across providers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageReport {
    /// Display name: the source provider's spelling.
    pub name: String,
    /// The provider whose row drives the recommendation (the one that owns
    /// the resolved executable, else the first installed by provider id).
    pub source: ProviderId,
    /// Rows by provider.
    pub rows: BTreeMap<ProviderId, ProviderRow>,
    /// Classes, from the source row.
    pub class: ClassSet,
    /// Which provider's executable actually runs, when one was found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<Resolution>,
    /// Mappings from the source toward every other provider.
    pub mappings: Vec<PackageMapping>,
    /// The recommendation with its reasons.
    pub recommendation: Recommendation,
    /// True when this unit is hidden in normal mode (not top-level, INV-13).
    pub advanced_only: bool,
}

/// What one provider reports about itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderReport {
    /// Detection.
    pub detection: ProviderDetection,
    /// Capabilities.
    pub capabilities: ProviderCapabilities,
    /// Support for this machine, from policy data.
    pub support: SupportState,
    /// Installed package names (deduplicated).
    pub package_count: u32,
    /// Top-level packages.
    pub top_level_count: u32,
    /// GUI applications (casks) - reported, out of migration scope.
    pub gui_app_count: u32,
}

/// Counts for the dashboard.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// Units shown in normal mode.
    pub top_level_units: u32,
    /// Units hidden behind `--advanced`.
    pub dependency_units: u32,
    /// Units per outcome kind, by the JSON `kind` string.
    pub by_outcome: BTreeMap<String, u32>,
    /// Units needing manual review.
    pub manual_review: u32,
}
