// SPDX-License-Identifier: Apache-2.0

//! `MANUAL_REVIEW_MAPPING_CONFIDENCE`: a candidate exists but the evidence is
//! too thin to act on.

use super::{labels, PRIORITY_MANUAL_REVIEW};
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Confidence, Outcome, Reason};

/// Fires when some other provider offers a candidate equivalent whose
/// confidence is below `STRONG` (`03-policy-and-recommendations.md`,
/// `MANUAL_REVIEW`).
///
/// The threshold is `Confidence::is_strong`, which lives in the domain so
/// that policy and mapping cannot drift apart on what "strong" means.
///
/// `NONE` is deliberately excluded. `NONE` is not weak evidence, it is the
/// absence of a candidate: there is nothing for a human to adjudicate, and
/// the fallback rule keeping the package where it is already says the right
/// thing. `AMBIGUOUS` and `PROBABLE` are the cases where something plausible
/// exists and a person has to look at it.
///
/// A provider that is not on this machine is excluded for the same kind of
/// reason: a shaky candidate under a provider the user does not have is not a
/// decision anyone can take. [`Facts::mapped_destinations`] draws that line,
/// and every rule that speaks about the other side reads it.
pub struct ManualReviewMappingConfidence;

impl Rule for ManualReviewMappingConfidence {
    fn id(&self) -> &'static str {
        "MANUAL_REVIEW_MAPPING_CONFIDENCE"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "docs/spec/03-policy-and-recommendations.md, Starting policy, MANUAL_REVIEW; docs/spec/02-domain-model.md, Mapping"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_MANUAL_REVIEW
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        let (destination, per) = facts.mapped_destinations().into_iter().find_map(|id| {
            facts
                .at(&id)
                .and_then(|per| (!per.mapping_confidence.is_strong()).then(|| (id.clone(), per)))
        })?;

        Some(RuleVerdict::new(
            Outcome::ManualReview,
            Reason::new(reasons::MAPPING_CONFIDENCE_BELOW_THRESHOLD)
                .with("destination", &destination)
                .with("confidence", &labels::confidence(per.mapping_confidence))
                .with("threshold", &labels::confidence(Confidence::Strong)),
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

    fn with_confidence(confidence: Confidence) -> Facts {
        let mut facts = testing::baseline();
        facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap()
            .mapping_confidence = confidence;
        facts
    }

    #[test]
    fn strong_and_exact_do_not_fire_it() {
        for confidence in [Confidence::Strong, Confidence::Exact] {
            assert!(ManualReviewMappingConfidence
                .evaluate(&with_confidence(confidence), &data())
                .is_none());
        }
    }

    #[test]
    fn weak_evidence_fires_it() {
        for confidence in [Confidence::Ambiguous, Confidence::Probable] {
            let verdict = ManualReviewMappingConfidence
                .evaluate(&with_confidence(confidence), &data())
                .unwrap();
            assert_eq!(verdict.outcome, Outcome::ManualReview);
            assert_eq!(
                verdict.reason.params.get("confidence").map(String::as_str),
                Some(labels::confidence(confidence))
            );
        }
    }

    #[test]
    fn a_candidate_under_a_provider_that_is_not_here_is_not_a_review_item() {
        let mut facts = with_confidence(Confidence::Probable);
        assert!(ManualReviewMappingConfidence
            .evaluate(&facts, &data())
            .is_some());

        facts.providers.get_mut(&testing::beta()).unwrap().presence =
            macportsbrew_domain::ProviderPresence::Absent;
        assert!(ManualReviewMappingConfidence
            .evaluate(&facts, &data())
            .is_none());
    }

    #[test]
    fn no_candidate_at_all_is_not_a_review_item() {
        assert!(ManualReviewMappingConfidence
            .evaluate(&with_confidence(Confidence::None), &data())
            .is_none());
    }
}
