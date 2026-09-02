// SPDX-License-Identifier: Apache-2.0

//! The error taxonomy: stable identifiers, each mapping to an exit code and
//! to a user-facing remediation message rendered by the UI
//! (`06-cli-and-ux.md`, "Output contract").

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable error identifiers. Adding one is a compatibility change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorId {
    /// A command-line usage error.
    Usage,
    /// The provider executable was not found.
    ProviderNotFound,
    /// The provider exists but did not answer or answered with an error.
    ProviderUnhealthy,
    /// The provider's output did not have the expected shape.
    ProviderOutputChanged,
    /// A provider invocation exceeded its timeout.
    ProviderTimeout,
    /// The requested package is not installed under that provider.
    PackageNotFound,
    /// The provider lacks a capability the operation needs.
    CapabilityMissing,
    /// A snapshot could not be read or has an incompatible schema.
    SnapshotUnusable,
    /// Policy data could not be loaded or is inconsistent.
    PolicyDataInvalid,
    /// A safety check blocked the operation (INV-01, INV-04, INV-19).
    SafetyBlocked,
    /// The operation exists in the specification but not in this build
    /// (the foundation pass ships no mutating code path).
    NotImplementedInThisPass,
    /// An I/O failure outside a provider invocation.
    Io,
    /// Anything else. Should not appear in user-facing output.
    Internal,
}

impl ErrorId {
    /// The stable exit code (`06-cli-and-ux.md`).
    pub fn exit_code(self) -> i32 {
        match self {
            ErrorId::Usage => 2,
            ErrorId::SafetyBlocked => 4,
            ErrorId::ProviderNotFound
            | ErrorId::ProviderUnhealthy
            | ErrorId::ProviderOutputChanged
            | ErrorId::ProviderTimeout
            | ErrorId::CapabilityMissing => 5,
            ErrorId::SnapshotUnusable | ErrorId::PolicyDataInvalid => 8,
            ErrorId::PackageNotFound
            | ErrorId::NotImplementedInThisPass
            | ErrorId::Io
            | ErrorId::Internal => 1,
        }
    }

    /// The message identifier the UI renders for this error.
    pub fn message_id(self) -> &'static str {
        match self {
            ErrorId::Usage => "error.usage",
            ErrorId::ProviderNotFound => "error.provider_not_found",
            ErrorId::ProviderUnhealthy => "error.provider_unhealthy",
            ErrorId::ProviderOutputChanged => "error.provider_output_changed",
            ErrorId::ProviderTimeout => "error.provider_timeout",
            ErrorId::PackageNotFound => "error.package_not_found",
            ErrorId::CapabilityMissing => "error.capability_missing",
            ErrorId::SnapshotUnusable => "error.snapshot_unusable",
            ErrorId::PolicyDataInvalid => "error.policy_data_invalid",
            ErrorId::SafetyBlocked => "error.safety_blocked",
            ErrorId::NotImplementedInThisPass => "error.not_implemented_in_this_pass",
            ErrorId::Io => "error.io",
            ErrorId::Internal => "error.internal",
        }
    }

    /// Every identifier, for tests that assert catalogue coverage.
    pub fn all() -> &'static [ErrorId] {
        &[
            ErrorId::Usage,
            ErrorId::ProviderNotFound,
            ErrorId::ProviderUnhealthy,
            ErrorId::ProviderOutputChanged,
            ErrorId::ProviderTimeout,
            ErrorId::PackageNotFound,
            ErrorId::CapabilityMissing,
            ErrorId::SnapshotUnusable,
            ErrorId::PolicyDataInvalid,
            ErrorId::SafetyBlocked,
            ErrorId::NotImplementedInThisPass,
            ErrorId::Io,
            ErrorId::Internal,
        ]
    }
}

/// An error with its identifier and a detail string for diagnostics. The
/// detail is English and never localised; the UI renders the identifier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainError {
    /// Which error.
    pub id: ErrorId,
    /// Diagnostic detail (a command, a path, a parser message).
    pub detail: String,
}

impl DomainError {
    /// Creates an error.
    pub fn new(id: ErrorId, detail: impl Into<String>) -> Self {
        Self {
            id,
            detail: detail.into(),
        }
    }

    /// The exit code for this error.
    pub fn exit_code(&self) -> i32 {
        self.id.exit_code()
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.id.message_id(), self.detail)
    }
}

impl std::error::Error for DomainError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_the_contract() {
        assert_eq!(ErrorId::Usage.exit_code(), 2);
        assert_eq!(ErrorId::SafetyBlocked.exit_code(), 4);
        assert_eq!(ErrorId::ProviderUnhealthy.exit_code(), 5);
        assert_eq!(ErrorId::SnapshotUnusable.exit_code(), 8);
        assert_eq!(ErrorId::Internal.exit_code(), 1);
    }

    #[test]
    fn every_id_has_a_distinct_message() {
        let ids: std::collections::BTreeSet<_> =
            ErrorId::all().iter().map(|e| e.message_id()).collect();
        assert_eq!(ids.len(), ErrorId::all().len());
    }
}
