// SPDX-License-Identifier: Apache-2.0

//! `MANUAL_REVIEW_PERSISTENT_DATA`: the curated list is the authority on what
//! owns data, and a hit on it stops the engine on its own.

use super::PRIORITY_MANUAL_REVIEW;
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, Reason};

/// A package named by `data/policy/stateful-packages.toml` under any
/// provider forces manual review, whether or not the classifier put
/// `STATEFUL` on it.
///
/// `R-8` finding 7 measured the metadata heuristic and found it misses
/// packages that are unambiguously stateful, because their data directories
/// are established in code the provider's metadata never exposes. Finding 8
/// therefore names curated data the primary authority and the heuristic
/// additive only. This rule is that split: it fires on the data alone, so a
/// classifier that misses a database cannot cost a user their database.
///
/// It is deliberately checked on every provider in the facts, not only the
/// one holding the package. The data is owned by the software, not by the
/// side it currently sits on, and a move in either direction meets it
/// (INV-17).
pub struct ManualReviewPersistentData;

impl Rule for ManualReviewPersistentData {
    fn id(&self) -> &'static str {
        "MANUAL_REVIEW_PERSISTENT_DATA"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "INV-19; docs/research/R-8-stateful-and-service-packages.md findings 7, 8 and 9"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_MANUAL_REVIEW
    }

    fn evaluate(&self, facts: &Facts, data: &PolicyData) -> Option<RuleVerdict> {
        let mut candidates: Vec<(&str, &str)> =
            vec![(facts.package.source.as_str(), facts.package.name.as_str())];
        for (id, per) in &facts.package.per_provider {
            if let Some(name) = per.mapped_name.as_deref() {
                candidates.push((id.as_str(), name));
            }
        }
        candidates.sort_unstable();
        candidates.dedup();

        let (provider, name, family) = candidates.into_iter().find_map(|(provider, name)| {
            data.stateful_family_of(provider, name)
                .map(|family| (provider, name, family))
        })?;

        Some(RuleVerdict::new(
            Outcome::ManualReview,
            Reason::new(reasons::PACKAGE_OWNS_PERSISTENT_DATA)
                .with("family", &family.family)
                .with("package", &name)
                .with("provider", &provider),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::testing;

    fn data() -> PolicyData {
        PolicyData::load_embedded().unwrap()
    }

    #[test]
    fn a_package_outside_the_curated_list_does_not_fire_it() {
        assert!(ManualReviewPersistentData
            .evaluate(&testing::baseline(), &data())
            .is_none());
    }

    #[test]
    fn a_curated_family_fires_it_from_the_side_holding_the_package() {
        let mut facts = testing::baseline();
        facts.package.source = macportsbrew_domain::ProviderId::new("homebrew");
        facts.package.name = "postgresql@17".to_string();
        let verdict = ManualReviewPersistentData
            .evaluate(&facts, &data())
            .unwrap();
        assert_eq!(verdict.outcome, Outcome::ManualReview);
        assert_eq!(
            verdict.reason.params.get("family").map(String::as_str),
            Some("postgresql")
        );
    }

    #[test]
    fn it_fires_from_the_other_side_too() {
        // The same family reached through the mapped name on a destination:
        // the data belongs to the software, not to a direction (INV-17).
        let mut facts = testing::baseline();
        let other = macportsbrew_domain::ProviderId::new("macports");
        let mut per = testing::package_at(
            false,
            macportsbrew_domain::ArtifactAvailability::BinaryAvailable,
            macportsbrew_domain::Confidence::Strong,
        );
        per.mapped_name = Some("postgresql17-server".to_string());
        facts.package.per_provider.insert(other, per);
        assert!(ManualReviewPersistentData
            .evaluate(&facts, &data())
            .is_some());
    }
}
