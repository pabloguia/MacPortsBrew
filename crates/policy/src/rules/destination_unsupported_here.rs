// SPDX-License-Identifier: Apache-2.0

//! `DESTINATION_UNSUPPORTED_HERE`: never send a package somewhere its
//! provider says it will not run.

use super::{labels, PRIORITY_DESTINATION_VETO};
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, Reason, SupportTier};

/// Fires when a provider that would otherwise be a destination states it does
/// not support this machine, and keeps the package where it is.
///
/// The specification's tier vocabulary makes `UNSUPPORTED` a statement by the
/// provider that it will not run without third-party patches, which is a
/// different kind of claim from "degraded" or "not tested": it is the
/// provider declining. Recommending a move into that is not a trade-off with
/// a downside, it is a recommendation that cannot work.
///
/// It outranks both moving rules by priority, and the engine additionally
/// refuses any winning outcome that names an unsupported provider. The
/// duplication is deliberate: a veto that only works because it happens to
/// have the larger number is one refactor away from not working, and this one
/// protects a user from being told to move onto a system that will not run.
pub struct DestinationUnsupportedHere;

impl Rule for DestinationUnsupportedHere {
    fn id(&self) -> &'static str {
        "DESTINATION_UNSUPPORTED_HERE"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "INV-20; docs/spec/03-policy-and-recommendations.md, Facts the engine consumes; docs/research/F-1-homebrew-support-tiers.md E1"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_DESTINATION_VETO
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        let destination = facts.mapped_destinations().into_iter().find(|id| {
            facts
                .at(id)
                .is_some_and(|per| per.mapping_confidence.is_strong())
                && facts.support(id).tier == SupportTier::Unsupported
        })?;
        let state = facts.support(&destination);

        Some(RuleVerdict::new(
            Outcome::Keep {
                provider: facts.package.source.clone(),
            },
            Reason::new(reasons::DESTINATION_UNSUPPORTED_HERE)
                .with("destination", &destination)
                .with("tier", &labels::tier(state.tier))
                .with("entry", &state.entry),
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

    fn other_side(tier: SupportTier) -> Facts {
        let mut facts = testing::baseline();
        facts.providers.get_mut(&testing::beta()).unwrap().support = testing::support(tier, false);
        facts
    }

    #[test]
    fn a_supported_or_degraded_other_side_does_not_fire_it() {
        for tier in [
            SupportTier::Supported,
            SupportTier::Degraded,
            SupportTier::Unknown,
        ] {
            assert!(DestinationUnsupportedHere
                .evaluate(&other_side(tier), &data())
                .is_none());
        }
    }

    #[test]
    fn an_unsupported_other_side_keeps_the_package_where_it_is() {
        let verdict = DestinationUnsupportedHere
            .evaluate(&other_side(SupportTier::Unsupported), &data())
            .unwrap();
        assert_eq!(
            verdict.outcome,
            Outcome::Keep {
                provider: testing::alpha()
            }
        );
        assert_eq!(
            verdict.reason.params.get("tier").map(String::as_str),
            Some("UNSUPPORTED")
        );
    }

    #[test]
    fn a_side_with_no_candidate_is_not_vetoed_it_is_simply_not_a_destination() {
        let mut facts = other_side(SupportTier::Unsupported);
        facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap()
            .mapping_confidence = Confidence::None;
        assert!(DestinationUnsupportedHere
            .evaluate(&facts, &data())
            .is_none());
    }

    #[test]
    fn the_same_facts_with_the_sides_exchanged_veto_the_other_way() {
        let facts = other_side(SupportTier::Unsupported);
        let first = DestinationUnsupportedHere
            .evaluate(&facts, &data())
            .unwrap();
        let swapped = testing::swap_providers(&facts, &testing::alpha(), &testing::beta());
        let second = DestinationUnsupportedHere
            .evaluate(&swapped, &data())
            .unwrap();
        assert_eq!(
            second.outcome,
            testing::swap_outcome(&first.outcome, &testing::alpha(), &testing::beta())
        );
    }
}
