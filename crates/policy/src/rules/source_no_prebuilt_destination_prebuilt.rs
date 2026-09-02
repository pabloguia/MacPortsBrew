// SPDX-License-Identifier: Apache-2.0

//! `SOURCE_NO_PREBUILT_DESTINATION_PREBUILT`: one side would compile, the
//! other has it ready.

use super::{labels, PRIORITY_ARTIFACT_ADVANTAGE};
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{ArtifactAvailability, Outcome, ProviderId, Reason, SupportTier};

/// Fires when adding the package on the side that holds it would compile from
/// source, and another provider offers a strong equivalent with a prebuilt
/// artifact for this machine (`03-policy-and-recommendations.md`, Starting
/// policy: the artifact-availability arm, stated there in both directions).
///
/// This is the arm of the policy that is genuinely symmetric in the field.
/// One provider's artifact supply for Intel is frozen and eroding (`F-1`
/// finding 5); the other's is an exact five-part filename match where a
/// single non-default variant means a source build (`R-4` findings 2 and 3).
/// Either side can be the one compiling, for entirely different reasons, and
/// the rule cannot tell which is which — it reads the two assessments and
/// compares them.
///
/// The cause of the source build travels into the reason. `R-4` records that
/// the causes carry different advice: a licence restriction will never
/// produce an artifact, while a missing platform build might appear tomorrow,
/// and a user told only "it will compile" cannot tell those apart.
pub struct SourceNoPrebuiltDestinationPrebuilt;

impl Rule for SourceNoPrebuiltDestinationPrebuilt {
    fn id(&self) -> &'static str {
        "SOURCE_NO_PREBUILT_DESTINATION_PREBUILT"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "docs/spec/03-policy-and-recommendations.md, Starting policy; docs/research/R-2-homebrew-artifact-availability.md finding 5; docs/research/R-4-macports-binary-archives.md findings 2, 3 and 5"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_ARTIFACT_ADVANTAGE
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        let here = facts.source_package()?;
        let ArtifactAvailability::SourceBuildRequired { reason: cause } =
            here.availability.availability
        else {
            return None;
        };

        let destination = facts.mapped_destinations().into_iter().find(|id| {
            let usable = facts.at(id).is_some_and(|per| {
                per.mapping_confidence.is_strong() && per.availability.is_prebuilt()
            });
            usable && welcoming(facts, id)
        })?;

        Some(RuleVerdict::new(
            Outcome::MigrateTo {
                provider: destination.clone(),
            },
            Reason::new(reasons::DESTINATION_OFFERS_PREBUILT)
                .with("source", &facts.package.source)
                .with("destination", &destination)
                .with("cause", &labels::source_build_reason(cause)),
        ))
    }
}

fn welcoming(facts: &Facts, id: &ProviderId) -> bool {
    facts.is_usable(id)
        && matches!(
            facts.support(id).tier,
            SupportTier::Supported | SupportTier::Degraded
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::testing;
    use macportsbrew_domain::availability::SourceBuildReason;
    use macportsbrew_domain::Confidence;

    fn data() -> PolicyData {
        PolicyData::load_embedded().unwrap()
    }

    fn compiling_here(cause: SourceBuildReason) -> Facts {
        let mut facts = testing::baseline();
        facts
            .package
            .per_provider
            .get_mut(&testing::alpha())
            .unwrap()
            .availability =
            testing::availability(ArtifactAvailability::SourceBuildRequired { reason: cause });
        facts
    }

    #[test]
    fn a_prebuilt_artifact_on_both_sides_does_not_fire_it() {
        assert!(SourceNoPrebuiltDestinationPrebuilt
            .evaluate(&testing::baseline(), &data())
            .is_none());
    }

    #[test]
    fn compiling_here_and_ready_there_points_there() {
        let facts = compiling_here(SourceBuildReason::NoArtifactForThisPlatform);
        let verdict = SourceNoPrebuiltDestinationPrebuilt
            .evaluate(&facts, &data())
            .unwrap();
        assert_eq!(
            verdict.outcome,
            Outcome::MigrateTo {
                provider: testing::beta()
            }
        );
        assert_eq!(
            verdict.reason.params.get("cause").map(String::as_str),
            Some("NO_ARTIFACT_FOR_THIS_PLATFORM")
        );
    }

    #[test]
    fn each_cause_travels_into_the_reason() {
        for cause in [
            SourceBuildReason::NoArtifactForThisPlatform,
            SourceBuildReason::NonDefaultOptions,
            SourceBuildReason::LicenceRestricted,
            SourceBuildReason::ForcedByConfiguration,
        ] {
            let verdict = SourceNoPrebuiltDestinationPrebuilt
                .evaluate(&compiling_here(cause), &data())
                .unwrap();
            assert_eq!(
                verdict.reason.params.get("cause").map(String::as_str),
                Some(labels::source_build_reason(cause))
            );
        }
    }

    #[test]
    fn compiling_on_both_sides_is_not_an_advantage() {
        let mut facts = compiling_here(SourceBuildReason::NoArtifactForThisPlatform);
        facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap()
            .availability = testing::availability(ArtifactAvailability::SourceBuildRequired {
            reason: SourceBuildReason::LicenceRestricted,
        });
        assert!(SourceNoPrebuiltDestinationPrebuilt
            .evaluate(&facts, &data())
            .is_none());
    }

    #[test]
    fn weak_evidence_is_not_enough_to_move_on() {
        let mut facts = compiling_here(SourceBuildReason::NoArtifactForThisPlatform);
        facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap()
            .mapping_confidence = Confidence::Probable;
        assert!(SourceNoPrebuiltDestinationPrebuilt
            .evaluate(&facts, &data())
            .is_none());
    }

    #[test]
    fn the_same_facts_with_the_sides_exchanged_point_the_other_way() {
        let facts = compiling_here(SourceBuildReason::NoArtifactForThisPlatform);
        let first = SourceNoPrebuiltDestinationPrebuilt
            .evaluate(&facts, &data())
            .unwrap();
        let swapped = testing::swap_providers(&facts, &testing::alpha(), &testing::beta());
        let second = SourceNoPrebuiltDestinationPrebuilt
            .evaluate(&swapped, &data())
            .unwrap();
        assert_eq!(
            second.outcome,
            testing::swap_outcome(&first.outcome, &testing::alpha(), &testing::beta())
        );
    }
}
