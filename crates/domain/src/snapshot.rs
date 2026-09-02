// SPDX-License-Identifier: Apache-2.0

//! Snapshots (`02-domain-model.md`, "Snapshot"): a complete, serialisable
//! description of the machine and both providers, sufficient to reproduce an
//! analysis offline. The input to `analyze --from-snapshot` (INV-15), the
//! basis of every fixture, and the restore safety net of later passes.

use crate::machine::Machine;
use crate::provider::{ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory};
use crate::resolution::Resolution;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The snapshot schema version. Loading a different major fails loudly.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Everything captured about one provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSnapshot {
    /// Detection result.
    pub detection: ProviderDetection,
    /// Capability table.
    pub capabilities: ProviderCapabilities,
    /// The inventory. Empty when the provider is absent or broken.
    pub inventory: ProviderInventory,
    /// Reported environment (paths, variables the prefix implies). Names
    /// facts about the installation; not what the executor sets.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, String>,
    /// The sanitised environment the adapter invokes the provider with
    /// (INV-10): built from scratch, provider-quieting variables set,
    /// build-flag variables absent. A plan command carries exactly this.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub invocation_environment: BTreeMap<String, String>,
    /// Resolutions of the executables the inventory names, keyed by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resolutions: BTreeMap<String, Resolution>,
}

/// The shell environment as observed, redacted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSnapshot {
    /// `PATH` entries in order, with the home directory replaced by `~`.
    #[serde(default)]
    pub path: Vec<String>,
    /// The build-affecting variables `05-environment` says to report, when
    /// set, redacted the same way. Values of other variables are never
    /// captured.
    #[serde(default)]
    pub reported: BTreeMap<String, String>,
}

/// Provenance of a capture (`07-quality-and-testing.md`, "Fixtures").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    /// Capture timestamp, RFC 3339 UTC.
    pub captured_at: String,
    /// The command that captured it.
    pub captured_by: String,
    /// `captured` from a live machine, or `synthesised` from documentation
    /// with the note that says so.
    pub origin: String,
    /// Free-text note (which machine, which documents).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Schema version.
    pub schema_version: u32,
    /// MacPortsBrew version that captured it.
    pub tool_version: String,
    /// Policy version in effect at capture.
    pub policy_version: String,
    /// Provenance.
    pub provenance: Provenance,
    /// The machine.
    pub machine: Machine,
    /// The shell environment, redacted.
    #[serde(default)]
    pub environment: EnvironmentSnapshot,
    /// One entry per provider the tool knows, present or not.
    pub providers: BTreeMap<ProviderId, ProviderSnapshot>,
}

impl Snapshot {
    /// Fails when the schema version is not the one this build reads.
    pub fn check_schema(&self) -> Result<(), crate::error::DomainError> {
        if self.schema_version == SNAPSHOT_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(crate::error::DomainError::new(
                crate::error::ErrorId::SnapshotUnusable,
                format!(
                    "snapshot schema version {} is not the supported {}",
                    self.schema_version, SNAPSHOT_SCHEMA_VERSION
                ),
            ))
        }
    }
}
