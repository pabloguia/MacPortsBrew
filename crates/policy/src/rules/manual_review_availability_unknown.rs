// SPDX-License-Identifier: Apache-2.0

//! `MANUAL_REVIEW_AVAILABILITY_UNKNOWN`: an unknown is an unknown.

use super::PRIORITY_MANUAL_REVIEW;
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{ArtifactAvailability, Outcome, Reason};

/// Fires when a reachable destination cannot say whether adding the package
/// there would use a prebuilt artifact or compile from source
/// (`03-policy-and-recommendations.md`, `MANUAL_REVIEW`: availability
/// `UNKNOWN`).
///
/// This is expected to fire often, and that is the point. `R-4` finding 9
/// establishes that one provider's availability is not determinable offline
/// in the positive direction at all, and finding 10 that it "must say
/// `UNKNOWN` — never `BINARY_AVAILABLE` — for everything else". A rule that
/// treated an unknown as a negative would turn a missing answer into a
/// recommendation, which is the failure mode the whole availability model
/// exists to prevent.
///
/// It is scoped to destinations that actually have a candidate and answer on
/// this machine. A provider with no equivalent has nothing to be unknown
/// about, and one that is not here has nothing to be moved to.
pub struct ManualReviewAvailabilityUnknown;

impl Rule for ManualReviewAvailabilityUnknown {
    fn id(&self) -> &'static str {
        "MANUAL_REVIEW_AVAILABILITY_UNKNOWN"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "docs/spec/03-policy-and-recommendations.md, Starting policy, MANUAL_REVIEW; docs/research/R-4-macports-binary-archives.md findings 9 and 10"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_MANUAL_REVIEW
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        let destination = facts.mapped_destinations().into_iter().find(|id| {
            facts.at(id).is_some_and(|per| {
                matches!(per.availability.availability, ArtifactAvailability::Unknown)
            })
        })?;

        Some(RuleVerdict::new(
            Outcome::ManualReview,
            Reason::new(reasons::AVAILABILITY_UNKNOWN)
                .with("provider", &destination)
                .with("package", &facts.package.name),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::testing;
    use macportsbrew_domain::Confidence;

    fn data() -> PolicyData {
        PolicyData::load_embedded().unwrap()
    }

    #[test]
    fn a_known_destination_does_not_fire_it() {
        assert!(ManualReviewAvailabilityUnknown
            .evaluate(&testing::baseline(), &data())
            .is_none());
    }

    #[test]
    fn an_unknown_destination_fires_it() {
        let mut facts = testing::baseline();
        facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap()
            .availability = testing::availability(ArtifactAvailability::Unknown);
        let verdict = ManualReviewAvailabilityUnknown
            .evaluate(&facts, &data())
            .unwrap();
        assert_eq!(verdict.outcome, Outcome::ManualReview);
        assert_eq!(
            verdict.reason.params.get("provider").map(String::as_str),
            Some("beta")
        );
    }

    #[test]
    fn a_provider_with_no_candidate_has_nothing_to_be_unknown_about() {
        let mut facts = testing::baseline();
        let per = facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap();
        per.availability = testing::availability(ArtifactAvailability::Unknown);
        per.mapping_confidence = Confidence::None;
        assert!(ManualReviewAvailabilityUnknown
            .evaluate(&facts, &data())
            .is_none());
    }

    #[test]
    fn a_provider_that_is_not_on_this_machine_is_not_a_destination() {
        let mut facts = testing::baseline();
        facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap()
            .availability = testing::availability(ArtifactAvailability::Unknown);
        assert!(ManualReviewAvailabilityUnknown
            .evaluate(&facts, &data())
            .is_some());

        facts.providers.get_mut(&testing::beta()).unwrap().presence =
            macportsbrew_domain::ProviderPresence::Absent;
        assert!(ManualReviewAvailabilityUnknown
            .evaluate(&facts, &data())
            .is_none());

        facts.providers.get_mut(&testing::beta()).unwrap().presence =
            macportsbrew_domain::ProviderPresence::Present;
        facts.providers.get_mut(&testing::beta()).unwrap().healthy = false;
        assert!(ManualReviewAvailabilityUnknown
            .evaluate(&facts, &data())
            .is_none());
    }

    #[test]
    fn an_unknown_on_the_side_holding_the_package_is_not_this_rules_business() {
        let mut facts = testing::baseline();
        facts
            .package
            .per_provider
            .get_mut(&testing::alpha())
            .unwrap()
            .availability = testing::availability(ArtifactAvailability::Unknown);
        assert!(ManualReviewAvailabilityUnknown
            .evaluate(&facts, &data())
            .is_none());
    }
}
