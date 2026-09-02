// SPDX-License-Identifier: Apache-2.0

//! Artifact availability, modelled symmetrically for every provider
//! (`02-domain-model.md`, SE-004): will this install as a prebuilt artifact
//! or compile from source, on this machine, with these options?

use serde::{Deserialize, Serialize};

/// The availability states. `Unknown` is a legitimate, displayable state and
/// is never silently downgraded to a positive or a negative.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "state")]
pub enum ArtifactAvailability {
    /// A prebuilt artifact for this machine and these options exists.
    BinaryAvailable,
    /// It would compile from source; the reason says which of the distinct
    /// causes applies (R-4: they carry different advice).
    SourceBuildRequired {
        /// Why no prebuilt artifact applies.
        reason: SourceBuildReason,
    },
    /// The provider cannot install it here at all.
    Unavailable,
    /// The provider does not support this platform for this package.
    UnsupportedPlatform,
    /// The provider marks it deprecated (or replaced).
    Deprecated,
    /// The provider marks it disabled: no longer installable.
    Disabled,
    /// Could not be determined; typically needs the network.
    Unknown,
}

/// The distinct causes of a source build.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceBuildReason {
    /// No prebuilt artifact for this platform. Whether one exists for other
    /// platforms is recorded in the evidence when known.
    NoArtifactForThisPlatform,
    /// Non-default variants or options were requested.
    NonDefaultOptions,
    /// The package's licence forbids redistributing a prebuilt artifact.
    LicenceRestricted,
    /// The provider is configured to build from source (a setting, a flag,
    /// a non-default prefix).
    ForcedByConfiguration,
}

/// Where an availability answer came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityEvidence {
    /// The provider field, command or data entry consulted.
    pub source: String,
    /// What it said, trimmed.
    pub detail: String,
    /// `true` when this evidence was obtained without the network.
    pub offline: bool,
}

/// One availability assessment: per (package, provider, machine, options).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityAssessment {
    /// The state.
    pub availability: ArtifactAvailability,
    /// The evidence that produced it. Never empty for a non-`Unknown` state.
    pub evidence: Vec<AvailabilityEvidence>,
    /// `true` when part of the answer came from a cache rather than a live
    /// query, so the UI can label it (`06-cli-and-ux.md`, caching).
    #[serde(default)]
    pub cached: bool,
}

impl AvailabilityAssessment {
    /// An `Unknown` assessment with the reason it is unknown.
    pub fn unknown(source: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            availability: ArtifactAvailability::Unknown,
            evidence: vec![AvailabilityEvidence {
                source: source.into(),
                detail: detail.into(),
                offline: true,
            }],
            cached: false,
        }
    }

    /// True when installing would use a prebuilt artifact.
    pub fn is_prebuilt(&self) -> bool {
        matches!(self.availability, ArtifactAvailability::BinaryAvailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_carries_its_reason() {
        let a = AvailabilityAssessment::unknown("network", "offline analysis");
        assert_eq!(a.availability, ArtifactAvailability::Unknown);
        assert_eq!(a.evidence.len(), 1);
        assert!(!a.is_prebuilt());
    }
}
