// SPDX-License-Identifier: Apache-2.0

//! The provider abstraction and its read contract.
//!
//! A provider is identified by an opaque id, never by an enum: the charter
//! forbids hardcoding "two providers", and INV-17 forbids either direction
//! being a special case. The known ids are constants in the adapters that
//! implement them; the domain treats every id alike.

use crate::availability::AvailabilityAssessment;
use crate::error::DomainError;
use crate::executable::{ExecutableList, Placement};
use crate::machine::Machine;
use crate::package::{DependencyEdge, InstalledPackage, PackageIdentity, PackageMetadata};
use crate::resolution::Resolution;
use crate::snapshot::{Installations, ProviderSnapshot};
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
    /// Whether capture changed `prefix` to remove the user's home.
    ///
    /// Recorded rather than re-derived. Redaction produces two shapes - `~` for
    /// the home itself, and the bare login name replaced wherever it is a whole
    /// path component, so `/opt/ada/brew` becomes `/opt/USER/brew` - and only
    /// the first is visible in the result. A consumer that guesses from the
    /// string catches the shape that fails closed and misses the one that fails
    /// open, which is what F-49 records and what this field removes the need to
    /// guess at.
    ///
    /// `#[serde(default)]`, so every snapshot and recording written before this
    /// field parses as `false` - meaning "not known to have been redacted", not
    /// "not redacted". A consumer must treat it as one half of the question and
    /// keep the shape test for the other, which is what `doctor` does.
    #[serde(default)]
    pub prefix_redacted: bool,
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
    /// The provider's **read** operations run as the invoking user.
    ///
    /// INV-20's second sentence — "where behaviour can be detected at runtime,
    /// detection wins over stored knowledge" — had no implementation for the one
    /// cell F-24 and F-35 are about: whether verifying a package needs
    /// administrator rights. That was a string in a data file, true of the one
    /// machine it was observed on. The registry a provider reads is created with
    /// the installing user's umask, so a restrictive umask makes the stored
    /// answer wrong, and no artefact noticed.
    ///
    /// **Detected from a read the adapter already performs**, not from a new
    /// probe: an inventory that came back is proof the read needs no elevation.
    /// Nothing is stat-ed and no permission is inspected.
    ///
    /// The strength of that evidence differs by provider, which is worth saying
    /// because the first version of this comment claimed an identity for both.
    /// For MacPorts it *is* one — the inventory argv and the verify argv are the
    /// same. For Homebrew the observed read is `info --json=v2 --installed` and
    /// the relaxed verb is `list --versions`: same binary, same prefix, same
    /// user, so the inference is sound, and it is an inference.
    ///
    /// **`Available` or `Unknown`, never `Unavailable`.** A read that fails
    /// does not say *why* it failed — a missing binary and a permission denial
    /// arrive the same way — so this can confirm "no elevation needed" and can
    /// never assert the opposite. That asymmetry is deliberate and is what makes
    /// consulting it safe: it can relax a stored `required` on a read to `none`
    /// when proven, and can never add an elevation prompt nobody established.
    /// SE-059 recorded that rendering an unknown as "needs administrator
    /// rights" is itself the INV-06 defect, because it trains a user to approve
    /// a password prompt for a read.
    UnprivilegedRead,
}

impl Capability {
    /// Every capability, for callers that must treat the set as a whole.
    ///
    /// The list is written out, and the `match` above it is what keeps it
    /// honest: adding a variant stops that compiling, and the compiler error
    /// lands next to the comment saying to extend the array too. This is the
    /// shape `ErrorId::all()` arrived at after F-43 item 2 found it as a
    /// hand-copied list with nothing beside it; `capability_all_names_every_
    /// variant` in this file's tests holds the other half.
    #[must_use]
    pub fn all() -> &'static [Capability] {
        const fn exhaustive(c: Capability) {
            // Extend the array below in the same change as this match.
            match c {
                Capability::MachineReadableInventory
                | Capability::RequestedRecord
                | Capability::Dependencies
                | Capability::ReverseDependencies
                | Capability::ArtifactAvailability
                | Capability::OfflineMetadata
                | Capability::FileOwnership
                | Capability::DeprecationState
                | Capability::ServiceMetadata
                | Capability::UnprivilegedRead => (),
            }
        }
        exhaustive(Capability::ServiceMetadata);

        &[
            Capability::MachineReadableInventory,
            Capability::RequestedRecord,
            Capability::Dependencies,
            Capability::ReverseDependencies,
            Capability::ArtifactAvailability,
            Capability::OfflineMetadata,
            Capability::FileOwnership,
            Capability::DeprecationState,
            Capability::ServiceMetadata,
            Capability::UnprivilegedRead,
        ]
    }
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

    /// True when the record says this capability cannot answer - by either
    /// spelling - and false when it is absent.
    ///
    /// [`Self::is_unknown`] asks "was it undetectable"; this asks the question
    /// a safety rule actually has, which is "may I conclude anything from what
    /// this provider reports". `Unavailable` is not an affirmative answer to
    /// that: a provider that does not offer service metadata cannot tell you
    /// whether a package defines a service any more than one whose scan failed
    /// can. Both are "I cannot establish it", and only `Available` is not.
    ///
    /// Filed by `security-reviewer` on 2026-09-18 and reproduced: with the
    /// planner keyed on `is_unknown`, a document whose `SERVICE_METADATA` read
    /// `UNAVAILABLE` planned normally - exit 0, a full plan with a
    /// `REMOVE_SOURCE` step - while the two spellings this pass had just
    /// closed refused. INV-19's rule was never evaluated, which is the whole
    /// of F-29 one enum variant over.
    ///
    /// **The absent case stays `false`, deliberately.** In memory a capability
    /// nobody reported was never asked about, and reading that as "could not
    /// be detected" would refuse every plan built from a hand-made snapshot.
    /// A *document* cannot reach here with an absent entry, because the load
    /// door fills every one (`Snapshot::mark_unrecorded_capabilities_unknown`)
    /// — the two rules answer different questions and both are needed.
    ///
    /// **This replaces `is_unknown`, which is gone rather than deprecated.**
    /// That predicate answered "was it undetectable", which is not the
    /// question any safety rule has, and it had exactly one caller: the
    /// planner, where choosing it over this one was the defect above. Two
    /// near-identical public predicates, where picking the wrong one is a
    /// data-loss-class mistake, is the shape that produced the bug — leaving
    /// the loser in place for a future caller to pick would have preserved it
    /// (`security-reviewer`, 2026-09-18).
    #[must_use]
    pub fn cannot_answer(&self, cap: Capability) -> bool {
        matches!(
            self.capabilities.get(&cap),
            Some(CapabilityState::Unknown | CapabilityState::Unavailable { .. })
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

    /// Every place this provider is installed on the machine being read, not
    /// only the one this tool runs. An adapter that cannot look says
    /// [`Installations::Unknowable`]; the default says nothing was recorded,
    /// which a reader must not take for "one".
    fn installations(&self) -> Installations {
        Installations::NotRecorded
    }

    /// The executables an installed package ships and where they land, from
    /// the provider's own file list (`docs/passes/16-plans/F.md`, Design §1).
    ///
    /// A read that answered badly is an `Ok` with an
    /// [`ExecutableList::Unknown`] for that package; an `Err` is a defect or a
    /// provider that vanished, and aborts the capture. The default says
    /// nothing was recorded, which a reader must not take for "none".
    fn executables(
        &self,
        pkg: &PackageIdentity,
    ) -> Result<(ExecutableList, Placement), DomainError> {
        let _ = pkg;
        Ok((ExecutableList::NotRecorded, Placement::default()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_all_is_the_whole_enum_and_not_a_stale_copy_of_it() {
        // Same chain `ErrorId::all` uses, and the same limitation. Rust cannot
        // enumerate variants without a derive, and ADR-0004 makes a dependency
        // for that a decision. What closes instead: the `exhaustive` match
        // inside `all()` has no wildcard, so adding a variant is a compile
        // error in this file — the author is already here — and this count then
        // fails until the array is extended too.
        //
        // It proves the list is as long as the enum, never that it names the
        // right members; `capability_all_has_no_duplicates` rules out padding
        // the count, which is the only way the two can disagree silently.
        assert_eq!(
            Capability::all().len(),
            NUMBER_OF_CAPABILITIES,
            "a variant was added without extending all(); a loaded snapshot \
             would then leave that capability unrecorded and it would read as \
             \"never asked about\" rather than as unknown"
        );
    }

    /// Update this **and** `all()` when adding a capability. The exhaustive
    /// match inside `all()` will already have refused to compile.
    const NUMBER_OF_CAPABILITIES: usize = 10;

    #[test]
    fn capability_all_has_no_duplicates() {
        let distinct: std::collections::BTreeSet<_> = Capability::all().iter().collect();
        assert_eq!(
            distinct.len(),
            Capability::all().len(),
            "a duplicate would pad the count and hide an omission"
        );
    }

    #[test]
    fn an_absent_capability_is_not_unknown_and_not_available() {
        // The distinction the whole of F-29 turns on, asserted on the accessors
        // themselves rather than only through the planner that consumes them.
        let empty = ProviderCapabilities::default();
        assert!(
            !empty.has(Capability::ServiceMetadata),
            "absent is not a yes"
        );
        assert!(
            !empty.cannot_answer(Capability::ServiceMetadata),
            "and absent is not undetectable either: in memory it means nobody \
             asked, which is what lets a hand-made snapshot still be planned"
        );
    }

    #[test]
    fn cannot_answer_is_true_for_both_spellings_of_could_not_establish() {
        // The carve-out above is load-bearing for the whole document rule, and
        // until 2026-09-18 it was asserted nowhere in the crate that defines
        // it - only indirectly, from the planner's tests and an end-to-end
        // arm. This is where a future simplification would break it.
        let with = |state: CapabilityState| ProviderCapabilities {
            capabilities: [(Capability::ServiceMetadata, state)].into_iter().collect(),
        };

        assert!(
            with(CapabilityState::Unknown).cannot_answer(Capability::ServiceMetadata),
            "undetectable is not an answer"
        );
        assert!(
            with(CapabilityState::Unavailable {
                reason: "provider_exposes_no_service_metadata".into(),
            })
            .cannot_answer(Capability::ServiceMetadata),
            "neither is 'this provider does not offer it' - a provider that \
             cannot tell you is a provider that cannot tell you, and reading \
             only the first spelling is how the planner let a document plan \
             that should have been refused"
        );
        assert!(
            !with(CapabilityState::Available).cannot_answer(Capability::ServiceMetadata),
            "and only Available is an answer, or the refusal is unconditional \
             and this test would pass for the wrong reason"
        );
    }
}
