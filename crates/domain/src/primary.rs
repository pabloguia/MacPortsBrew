// SPDX-License-Identifier: Apache-2.0

//! The primary decision a plan carries on its `SET_PRIMARY` step
//! (`05-environment-and-primary-provider.md`, "Setting the primary"; F.md
//! Design §4; SE-112).
//!
//! The decision is made from the snapshot alone, per executable of the source
//! package's list, and is carried **as data**: the step that holds it has no
//! command, and the execution gate refuses a plan whose decision is not
//! `NOTHING_TO_CHANGE` or `NO_EXECUTABLES` before any step runs. Nothing here
//! names a provider (INV-17) or reorders `PATH` (INV-12).

use crate::executable::ExecutableName;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Why the primary for one executable, or for the package as a whole, is not
/// planned. Every value is a refusal; the step list of 05 (MUST, SE-112)
/// decides which.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrimaryReason {
    /// The source package's list is not `KNOWN` (step 1). Never the package
    /// name in place of a list.
    ExecutablesUnknown,
    /// The mapping is not 1→1: which source executable answers which
    /// destination package has no principled rule yet.
    PrimaryMappingNotOneToOne,
    /// The executable was never resolved: replay, a snapshot file, a name not
    /// searched (step 2). Never read as "not on `PATH`".
    PrimaryResolutionUnknown,
    /// The winner is owned by the destination provider, but by another package
    /// or by no package it could name (step 3).
    OwnedByAnotherPackage,
    /// The winner is not a provider's file: a system tool or something else the
    /// user installed (step 4).
    NonProviderEntryWins,
    /// The winner's owner is a provider decided by a path prefix, not by the
    /// provider's own records (step 4).
    WinnerNotAuthoritative,
    /// Only a shim could make the destination's copy win: it is kept off its
    /// linked directories, its directory is not on `PATH`, or it comes after
    /// the winner (steps 5, 6). Offered by a later part, refused here.
    PrimaryNeedsShim,
    /// The destination is not installed, so where its executables land and
    /// which it ships cannot be read (DP-055, default (a); both directions).
    PrimaryDestinationNotInstalled,
    /// The outcome cannot be predicted: the destination's placement is not
    /// established, a relative or empty `PATH` entry comes before its
    /// directory, or the winner's `PATH` index was not recorded (steps 5, 6).
    PrimaryUnpredictable,
    /// The installed destination's `KNOWN` list does not hold the executable
    /// (step 5).
    DestinationLacksExecutable,
    /// The destination's directory is the winner's own `PATH` entry (step 6).
    SameDirectory,
    /// A reason written by a build newer than this one. Tolerated on the way
    /// in, never a decision this build acts on.
    #[serde(other)]
    Unknown,
}

impl PrimaryReason {
    /// Every reason, in declaration order, for catalogue coverage.
    #[must_use]
    pub fn all() -> &'static [PrimaryReason] {
        &[
            Self::ExecutablesUnknown,
            Self::PrimaryMappingNotOneToOne,
            Self::PrimaryResolutionUnknown,
            Self::OwnedByAnotherPackage,
            Self::NonProviderEntryWins,
            Self::WinnerNotAuthoritative,
            Self::PrimaryNeedsShim,
            Self::PrimaryDestinationNotInstalled,
            Self::PrimaryUnpredictable,
            Self::DestinationLacksExecutable,
            Self::SameDirectory,
            Self::Unknown,
        ]
    }

    /// The identifier, as serialised and as the catalogue keys it
    /// (`primary.reason.<identifier in lower case>`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExecutablesUnknown => "EXECUTABLES_UNKNOWN",
            Self::PrimaryMappingNotOneToOne => "PRIMARY_MAPPING_NOT_ONE_TO_ONE",
            Self::PrimaryResolutionUnknown => "PRIMARY_RESOLUTION_UNKNOWN",
            Self::OwnedByAnotherPackage => "OWNED_BY_ANOTHER_PACKAGE",
            Self::NonProviderEntryWins => "NON_PROVIDER_ENTRY_WINS",
            Self::WinnerNotAuthoritative => "WINNER_NOT_AUTHORITATIVE",
            Self::PrimaryNeedsShim => "PRIMARY_NEEDS_SHIM",
            Self::PrimaryDestinationNotInstalled => "PRIMARY_DESTINATION_NOT_INSTALLED",
            Self::PrimaryUnpredictable => "PRIMARY_UNPREDICTABLE",
            Self::DestinationLacksExecutable => "DESTINATION_LACKS_EXECUTABLE",
            Self::SameDirectory => "SAME_DIRECTORY",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// What `SET_PRIMARY` decided, from the snapshot alone. All or nothing per
/// package: one executable refused refuses the plan, and the refusal lists
/// every executable with its reason (05 MUST, SE-112).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrimaryDecision {
    /// Every executable of the source's list already resolves to the
    /// destination package, so there is nothing to change. Completed with no
    /// dispatch; `VERIFY_RESOLUTION` checks it at run time.
    NothingToChange {
        /// The source package's list, every one predicted to resolve to the
        /// destination package.
        executables: BTreeSet<ExecutableName>,
        /// The home-redacted `PATH` the decision used (SR-6, SR-12).
        #[serde(rename = "pathUsed")]
        path_used: Vec<String>,
    },
    /// The source package's list is `KNOWN` and empty: there is no primary to
    /// set. Carried on a command-less `SET_PRIMARY`, with no
    /// `VERIFY_RESOLUTION` (SG2-N2).
    NoExecutables,
    /// Not planned. `executables` is the source's whole list and `reasons`
    /// every one of them that is refused, with why; `package` is set instead
    /// when no list could be read or the mapping is not 1→1, and then both are
    /// empty.
    Refused {
        /// The source package's list, refused or not.
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        executables: BTreeSet<ExecutableName>,
        /// The refused executables, with why.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        reasons: BTreeMap<ExecutableName, PrimaryReason>,
        /// A reason about the package as a whole.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        package: Option<PrimaryReason>,
        /// The home-redacted `PATH` the decision used.
        #[serde(rename = "pathUsed")]
        path_used: Vec<String>,
    },
    /// A decision written by a build newer than this one, or one that did not
    /// survive whatever produced this plan. Never acted on.
    #[serde(other)]
    Unknown,
}

impl PrimaryDecision {
    /// The executables the decision is about: the source's list; empty for
    /// `NO_EXECUTABLES`, a package-level refusal and `UNKNOWN`.
    #[must_use]
    pub fn executables(&self) -> BTreeSet<ExecutableName> {
        match self {
            Self::NothingToChange { executables, .. } | Self::Refused { executables, .. } => {
                executables.clone()
            }
            Self::NoExecutables | Self::Unknown => BTreeSet::new(),
        }
    }

    /// Whether the decision leaves nothing for this build to change: the only
    /// two decisions a run may complete (`NOTHING_TO_CHANGE`,
    /// `NO_EXECUTABLES`).
    #[must_use]
    pub fn completes_without_dispatch(&self) -> bool {
        matches!(self, Self::NothingToChange { .. } | Self::NoExecutables)
    }

    /// The executables refused with `PRIMARY_NEEDS_SHIM`, the input a later
    /// part's shim offer starts from.
    #[must_use]
    pub fn needing_a_shim(&self) -> BTreeSet<ExecutableName> {
        match self {
            Self::Refused { reasons, .. } => reasons
                .iter()
                .filter(|(_, r)| **r == PrimaryReason::PrimaryNeedsShim)
                .map(|(e, _)| e.clone())
                .collect(),
            Self::NothingToChange { .. } | Self::NoExecutables | Self::Unknown => BTreeSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> ExecutableName {
        ExecutableName::parse(s).unwrap()
    }

    #[test]
    fn the_decision_is_tagged_and_an_unknown_one_reads_as_unknown() {
        let decision = PrimaryDecision::Refused {
            executables: [name("htop")].into_iter().collect(),
            reasons: [(name("htop"), PrimaryReason::PrimaryNeedsShim)]
                .into_iter()
                .collect(),
            package: None,
            path_used: vec!["/usr/bin".into()],
        };
        let text = serde_json::to_string(&decision).unwrap();
        assert_eq!(
            text,
            r#"{"decision":"REFUSED","executables":["htop"],"reasons":{"htop":"PRIMARY_NEEDS_SHIM"},"pathUsed":["/usr/bin"]}"#
        );
        assert_eq!(
            serde_json::from_str::<PrimaryDecision>(&text).unwrap(),
            decision
        );
        assert_eq!(
            serde_json::from_str::<PrimaryDecision>(r#"{"decision":"LATER"}"#).unwrap(),
            PrimaryDecision::Unknown
        );
        assert_eq!(
            serde_json::to_string(&PrimaryDecision::NoExecutables).unwrap(),
            r#"{"decision":"NO_EXECUTABLES"}"#
        );
    }

    #[test]
    fn a_bad_executable_name_in_a_decision_fails_to_load() {
        assert!(serde_json::from_str::<PrimaryDecision>(
            r#"{"decision":"NOTHING_TO_CHANGE","executables":["../x"],"pathUsed":[]}"#
        )
        .is_err());
    }

    #[test]
    fn every_reason_spells_as_it_serialises() {
        for reason in PrimaryReason::all() {
            assert_eq!(
                serde_json::to_string(reason).unwrap(),
                format!("\"{}\"", reason.as_str())
            );
        }
    }

    #[test]
    fn only_nothing_to_change_and_no_executables_complete() {
        let set: BTreeSet<_> = [name("a")].into_iter().collect();
        assert!(PrimaryDecision::NothingToChange {
            executables: set,
            path_used: vec![]
        }
        .completes_without_dispatch());
        assert!(PrimaryDecision::NoExecutables.completes_without_dispatch());
        assert!(!PrimaryDecision::Unknown.completes_without_dispatch());
        assert!(!PrimaryDecision::Refused {
            executables: BTreeSet::new(),
            reasons: BTreeMap::new(),
            package: Some(PrimaryReason::ExecutablesUnknown),
            path_used: vec![]
        }
        .completes_without_dispatch());
    }
}
