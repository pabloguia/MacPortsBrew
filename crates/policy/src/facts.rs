// SPDX-License-Identifier: Apache-2.0

//! The engine's input.
//!
//! `docs/spec/03-policy-and-recommendations.md` lists the facts the engine
//! consumes and then says: "Nothing else. If a rule needs a new fact, the
//! fact is added to the input type — rules never reach out for data
//! themselves." This module is that input type. A rule receives a `&Facts`
//! and a `&PolicyData` and has no other way to learn anything, which is what
//! makes INV-14 and INV-15 structural rather than a matter of discipline.
//!
//! `today` is a field for the same reason: the current date is an input.

use crate::data::PolicyData;
use macportsbrew_domain::{
    AvailabilityAssessment, ClassSet, Confidence, Machine, Ownership, ProviderId, ProviderPresence,
    SupportState, Version,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Everything the engine may read about one package on one machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    /// Architecture, macOS version, translation state, developer tools.
    pub machine: Machine,
    /// The current date, `YYYY-MM-DD`. An input, never a clock read.
    pub today: String,
    /// The policy version these facts were assembled against.
    pub policy_version: String,
    /// Every provider known on this machine, by id.
    pub providers: BTreeMap<ProviderId, ProviderFacts>,
    /// The package under consideration.
    pub package: PackageFacts,
}

/// What is known about one provider installation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderFacts {
    /// Present, absent or broken.
    pub presence: ProviderPresence,
    /// The provider's own version, when it answered.
    pub version: Option<Version>,
    /// Its prefix, when known.
    pub prefix: Option<PathBuf>,
    /// Its support state for this machine, resolved from the policy data or
    /// detected at run time (detection wins; INV-20).
    pub support: SupportState,
    /// Whether it answered its own health check.
    pub healthy: bool,
}

/// What is known about the package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFacts {
    /// The user-facing package name, on the source side.
    pub name: String,
    /// The provider it is currently held by.
    pub source: ProviderId,
    /// Why it is there, from that provider's own record.
    pub ownership: Ownership,
    /// Its classes. Empty means a plain, self-contained package.
    pub classes: ClassSet,
    /// The version currently in place, when known.
    pub installed_version: Option<Version>,
    /// One row per provider: the source itself, and every candidate the
    /// mapping produced. Both directions use the same map, so neither is a
    /// special case (INV-17).
    pub per_provider: BTreeMap<ProviderId, PackageProviderFacts>,
    /// How many packages inside the source provider depend on this one
    /// (INV-04: never counted across providers).
    pub reverse_dependency_count: u32,
    /// Whether the user asked for the package to exist under both providers.
    pub coexistence_requested: bool,
}

/// What is known about the package under one particular provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageProviderFacts {
    /// Whether it is in place under this provider.
    pub installed: bool,
    /// The version this provider offers, when known.
    pub available_version: Option<Version>,
    /// Whether adding it here would use a prebuilt artifact or compile.
    pub availability: AvailabilityAssessment,
    /// The provider marks it deprecated.
    pub deprecated: bool,
    /// The provider marks it disabled: no longer obtainable.
    pub disabled: bool,
    /// The package this provider says supersedes it.
    pub replaced_by: Option<String>,
    /// How sure the mapping to this provider is. `EXACT` for the source
    /// itself, because the source is not a mapping.
    pub mapping_confidence: Confidence,
    /// The name the package carries under this provider, when known.
    pub mapped_name: Option<String>,
}

impl Facts {
    /// The provider facts for `id`.
    pub fn provider(&self, id: &ProviderId) -> Option<&ProviderFacts> {
        self.providers.get(id)
    }

    /// The package facts under `id`.
    pub fn at(&self, id: &ProviderId) -> Option<&PackageProviderFacts> {
        self.package.per_provider.get(id)
    }

    /// The provider facts for the side the package is currently held by.
    pub fn source_provider(&self) -> Option<&ProviderFacts> {
        self.providers.get(&self.package.source)
    }

    /// The package facts on the source side.
    pub fn source_package(&self) -> Option<&PackageProviderFacts> {
        self.package.per_provider.get(&self.package.source)
    }

    /// The support state of `id`, or `UNKNOWN` when nothing is known.
    pub fn support(&self, id: &ProviderId) -> SupportState {
        self.providers.get(id).map_or_else(
            || SupportState::unknown("no-provider-facts"),
            |p| p.support.clone(),
        )
    }

    /// Every provider other than the source, sorted. These are the candidate
    /// destinations, and the list is built the same way whichever side the
    /// package starts on (INV-17).
    pub fn destinations(&self) -> Vec<ProviderId> {
        self.package
            .per_provider
            .keys()
            .filter(|id| **id != self.package.source)
            .cloned()
            .collect()
    }

    /// Destinations a package could actually be sent to: another provider
    /// that answers on this machine and that some mapping reaches, sorted.
    ///
    /// Both halves matter. A candidate under a provider that is not installed
    /// is a fact about the world, not a destination, and a rule that treats
    /// it as one asks the user to review a move they cannot make. Every rule
    /// that speaks about "the other side" reads this rather than
    /// [`Facts::destinations`], so they all draw the line in the same place.
    pub fn mapped_destinations(&self) -> Vec<ProviderId> {
        self.destinations()
            .into_iter()
            .filter(|id| {
                self.is_usable(id)
                    && self
                        .at(id)
                        .is_some_and(|p| p.mapping_confidence > Confidence::None)
            })
            .collect()
    }

    /// Whether a provider answered and is usable.
    pub fn is_usable(&self, id: &ProviderId) -> bool {
        self.providers
            .get(id)
            .is_some_and(|p| p.healthy && matches!(p.presence, ProviderPresence::Present))
    }

    /// The curated equivalents of this package under `destination`, from the
    /// policy data. A convenience so that a rule reads data through one door.
    pub fn curated_targets(&self, data: &PolicyData, destination: &ProviderId) -> Vec<String> {
        data.curated_targets(
            self.package.source.as_str(),
            &self.package.name,
            destination.as_str(),
        )
    }
}

impl PackageFacts {
    /// The family part of the package name: everything before a version
    /// marker, lower-cased.
    ///
    /// Both providers write versions into names and neither writes them the
    /// same way, so this cuts at the first `@` and then drops a trailing run
    /// of digits and dots. `python@3.12` and `python312` both give `python`;
    /// `postgresql17-server` gives itself back, which is correct — it is not
    /// a runtime family and nothing should treat it as one.
    pub fn family(&self) -> String {
        let head = self.name.split('@').next().unwrap_or(&self.name);
        let trimmed = head.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        let trimmed = trimmed.trim_end_matches(['-', '_']);
        let chosen = if trimmed.is_empty() { head } else { trimmed };
        chosen.to_ascii_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::testing;

    #[test]
    fn families_are_cut_at_the_version_marker() {
        let mut facts = testing::baseline();
        for (name, expected) in [
            ("python@3.12", "python"),
            ("python312", "python"),
            ("node@22", "node"),
            ("nodejs22", "nodejs"),
            ("php@8.4", "php"),
            ("ripgrep", "ripgrep"),
            ("postgresql17-server", "postgresql17-server"),
            ("3.12", "3.12"),
        ] {
            facts.package.name = name.to_string();
            assert_eq!(facts.package.family(), expected, "{name}");
        }
    }

    #[test]
    fn destinations_exclude_the_side_the_package_is_on() {
        let facts = testing::baseline();
        let destinations = facts.destinations();
        assert_eq!(destinations.len(), 1);
        assert!(!destinations.contains(&facts.package.source));
        assert!(facts.source_package().is_some());
        assert!(facts.source_provider().is_some());
        assert!(facts.is_usable(&facts.package.source));
        assert_eq!(
            facts.support(&ProviderId::new("nobody")).tier,
            macportsbrew_domain::SupportTier::Unknown
        );
    }
}
