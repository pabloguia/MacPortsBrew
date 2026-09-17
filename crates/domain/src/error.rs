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
    /// A recording could not be read or has an incompatible schema.
    ///
    /// Separate from [`ErrorId::SnapshotUnusable`] only because the remediation
    /// differs: a snapshot is produced by `macportsbrew snapshot` and consumed
    /// by `--from-snapshot`, a recording by `scripts/capture-fixture.sh` and
    /// consumed by `--replay`. One reader serves both (F-30), so before this
    /// existed a bad `--replay` told the user to take a fresh snapshot and
    /// point a flag they had not used at a different file (F-41). The exit code
    /// is deliberately the same 8: `06-cli-and-ux.md:68` is the category
    /// "8 stale/unusable input", and a recording is one.
    RecordingUnusable,
    /// The input this run was built from cannot be used.
    ///
    /// Separate from [`ErrorId::SnapshotUnusable`] and
    /// [`ErrorId::RecordingUnusable`] for the same reason those are separate
    /// from each other - the remediation - but in the opposite direction: this
    /// one names **no** route. The planner is handed a `Snapshot` whose values
    /// may have come from a live capture, a snapshot file or a recording, and
    /// `planner.rs` refused them all with the snapshot remediation, telling a
    /// `--replay` user to take a fresh snapshot and point a flag they had not
    /// used at a different file (F-43, the same defect as F-41 one layer up).
    ///
    /// The route is *visible* - `Provenance.captured_by` carries it - but only
    /// as a free-text display string assembled in the CLI, and branching an
    /// error identifier on one of those is the wrong instrument. An identifier
    /// that presumes nothing needs no branch. Exit code 8 for the reason
    /// `RecordingUnusable` has it: `06-cli-and-ux.md:68` is the category
    /// "8 stale/unusable input", and this is one.
    InputUnusable,
    /// Policy data could not be loaded or is inconsistent.
    PolicyDataInvalid,
    /// A safety check blocked the operation (INV-01, INV-04, INV-19).
    SafetyBlocked,
    /// A safety rule could not be evaluated, because the input does not carry
    /// the fact it needs.
    ///
    /// Separate from [`ErrorId::SafetyBlocked`] for the reason
    /// [`ErrorId::RecordingUnusable`] is separate from
    /// [`ErrorId::SnapshotUnusable`] — the remediation. `SafetyBlocked` means a
    /// rule fired and the user can read the finding, resolve it and re-run.
    /// Here no rule fired and nothing the user does resolves it: the fact is
    /// absent from the document, so there is no plan to correct. Same exit code
    /// 4, because `06-cli-and-ux.md:67` is the category "blocked by a safety
    /// check" and being unable to run one is why this operation stopped
    /// (DP-016).
    SafetyUnverifiable,
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
            ErrorId::SafetyBlocked | ErrorId::SafetyUnverifiable => 4,
            ErrorId::ProviderNotFound
            | ErrorId::ProviderUnhealthy
            | ErrorId::ProviderOutputChanged
            | ErrorId::ProviderTimeout
            | ErrorId::CapabilityMissing => 5,
            ErrorId::SnapshotUnusable
            | ErrorId::RecordingUnusable
            | ErrorId::InputUnusable
            | ErrorId::PolicyDataInvalid => 8,
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
            ErrorId::RecordingUnusable => "error.recording_unusable",
            ErrorId::InputUnusable => "error.input_unusable",
            ErrorId::PolicyDataInvalid => "error.policy_data_invalid",
            ErrorId::SafetyBlocked => "error.safety_blocked",
            ErrorId::SafetyUnverifiable => "error.safety_unverifiable",
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
            ErrorId::RecordingUnusable,
            ErrorId::InputUnusable,
            ErrorId::PolicyDataInvalid,
            ErrorId::SafetyBlocked,
            ErrorId::SafetyUnverifiable,
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
        // Same bucket, deliberately: a safety rule that cannot be evaluated is
        // why the operation stopped, which is what category 4 names (DP-016).
        assert_eq!(ErrorId::SafetyUnverifiable.exit_code(), 4);
        assert_eq!(ErrorId::ProviderUnhealthy.exit_code(), 5);
        assert_eq!(ErrorId::SnapshotUnusable.exit_code(), 8);
        // Same bucket, deliberately: `06-cli-and-ux.md:68` is "8 stale/unusable
        // input", a category, and a recording is one. Asserted here rather than
        // only in the adapter that reads recordings, so re-bucketing the
        // identifier fails where the contract lives (F-41).
        assert_eq!(ErrorId::RecordingUnusable.exit_code(), 8);
        // And the planner's, which names no route at all (F-43).
        assert_eq!(ErrorId::InputUnusable.exit_code(), 8);
        assert_eq!(ErrorId::Internal.exit_code(), 1);
    }

    #[test]
    fn all_is_the_whole_enum_and_not_a_stale_copy_of_it() {
        // `all()` is a hand-written mirror of the enum, and `crates/cli/tests/
        // i18n.rs` iterates it to prove every identifier has a catalogue
        // message, a `{detail}` slot and a remediation sentence - the MUST at
        // `06-cli-and-ux.md:76`. Forget to extend `all()` and every one of
        // those tests still passes while an identifier ships with no message
        // at all, which that MUST calls never acceptable (F-43).
        //
        // Rust cannot enumerate variants without a derive, and ADR-0004 makes
        // a dependency for that a decision. What is available is a chain, and
        // it closes: `exit_code` and `message_id` are exhaustive matches with
        // no wildcard, so a new variant is a **compile error** in this file -
        // the author is already here - and this count then fails until `all()`
        // is extended too. `every_id_has_a_distinct_message` below rules out
        // padding the count with a duplicate.
        //
        // What it does not prove: that `all()` lists the *right* identifiers,
        // only that it lists as many as exist. Stated because a count that is
        // mistaken for a set is how the next version of F-43 gets written.
        assert_eq!(
            ErrorId::all().len(),
            NUMBER_OF_ERROR_IDS,
            "a variant was added without extending all(); the i18n coverage \
             tests iterate all() and would have stayed green"
        );
    }

    /// Update this **and** `all()` when adding an identifier. The exhaustive
    /// matches above will already have refused to compile.
    const NUMBER_OF_ERROR_IDS: usize = 16;

    #[test]
    fn every_id_has_a_distinct_message() {
        let ids: std::collections::BTreeSet<_> =
            ErrorId::all().iter().map(|e| e.message_id()).collect();
        assert_eq!(ids.len(), ErrorId::all().len());
    }
}
