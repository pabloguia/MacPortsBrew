// SPDX-License-Identifier: Apache-2.0

//! Provider support state for a machine, as dated data (INV-20).

use serde::{Deserialize, Serialize};

/// A fact about a provider with its provenance. Facts are data; every one
/// carries where it was read and when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatedFact {
    /// Where it was read (a URL or a command).
    pub source: String,
    /// When it was read, `YYYY-MM-DD`.
    pub read_on: String,
}

/// Support tier vocabulary. Homebrew publishes tiers; MacPorts publishes
/// builders and installers per macOS version, which the same three states
/// describe (R-3, R-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SupportTier {
    /// Fully supported: the provider builds and tests for this machine.
    Supported,
    /// Works but degraded: no new prebuilt artifacts, no fixes, or a stated
    /// end date.
    Degraded,
    /// The provider states it does not support this machine.
    Unsupported,
    /// Not determinable from data or detection.
    Unknown,
}

/// A provider's support state for one machine, with the evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportState {
    /// The tier.
    pub tier: SupportTier,
    /// The provider's own name for it, when it has one (`tier3`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_label: Option<String>,
    /// A stated date after which support ends, `YYYY-MM-DD`, when announced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Whether the provider publishes prebuilt artifacts for this machine
    /// at all, when the data says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prebuilt_artifacts: Option<bool>,
    /// Provenance.
    pub fact: DatedFact,
    /// A short stable identifier of the data entry that matched.
    pub entry: String,
}

impl SupportState {
    /// The state used when no data entry matches.
    pub fn unknown(entry: impl Into<String>) -> Self {
        Self {
            tier: SupportTier::Unknown,
            provider_label: None,
            end_date: None,
            prebuilt_artifacts: None,
            fact: DatedFact {
                source: "none".into(),
                read_on: "none".into(),
            },
            entry: entry.into(),
        }
    }
}
