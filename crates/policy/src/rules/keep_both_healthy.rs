// SPDX-License-Identifier: Apache-2.0

//! `KEEP_BOTH_HEALTHY`: coexistence is an end state, not a step.

use super::PRIORITY_COEXISTENCE;
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, ProviderId, Reason, SupportTier};

/// Fires when the package is in place under two providers that both answer,
/// or when the user has asked for it to be, and names one of them primary
/// (`03-policy-and-recommendations.md`, `KEEP_BOTH`; INV-18).
///
/// Both branches ask the same two questions of the other side — does it
/// answer, and does it say it supports this machine. They have to ask the
/// same questions: when one branch checked the support tier and the other did
/// not, a package held under two healthy providers could name an unsupported
/// one primary, and the outcome then had to be rescued downstream. Rescuing a
/// `KEEP_BOTH` downstream is how a first-class end state (INV-18) turns into
/// an intermediate one, so the check belongs here, once, in both branches.
///
/// "Primary" here means only which one the user's shell should reach first.
/// It transfers no ownership (INV-11) and reorders nothing for any other
/// package (INV-12), and the duplicate dependency trees underneath stay
/// duplicated, because INV-09 says duplication is correct and consolidating
/// it is not this tool's business.
///
/// The primary is chosen on two facts, in this order: a prebuilt artifact for
/// this machine, then a provider that says it supports this machine. If both
/// score alike the side already holding the package keeps the position.
///
/// That last tie-break is the whole of INV-17 in one line. Breaking a tie on
/// the provider id would be stable and would look harmless, and it would mean
/// that on every tie one particular provider won — a preference by
/// alphabetical accident, which is exactly what "neither direction is a
/// special case" forbids. Breaking it on which side already holds the package
/// reads a fact instead, so mirroring the facts mirrors the answer.
pub struct KeepBothHealthy;

impl Rule for KeepBothHealthy {
    fn id(&self) -> &'static str {
        "KEEP_BOTH_HEALTHY"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "INV-18; INV-09; docs/spec/03-policy-and-recommendations.md, Starting policy, KEEP_BOTH"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_COEXISTENCE
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        if !facts.is_usable(&facts.package.source) {
            return None;
        }

        // `destinations` rather than `mapped_destinations`: a package already
        // present under the other provider does not need a mapping to have got
        // there. The other two questions are the same ones the branch below
        // asks.
        let both_hold_it = facts.destinations().into_iter().find(|id| {
            facts.at(id).is_some_and(|per| per.installed)
                && facts.is_usable(id)
                && facts.support(id).tier != SupportTier::Unsupported
        });

        let (other, trigger) = if let Some(other) = both_hold_it {
            (other, reasons::PRESENT_UNDER_BOTH_PROVIDERS)
        } else if facts.package.coexistence_requested {
            // `mapped_destinations` has already established that this side
            // answers on this machine, so only the two questions it does not
            // ask are asked here.
            let asked_for = facts.mapped_destinations().into_iter().find(|id| {
                facts
                    .at(id)
                    .is_some_and(|per| per.mapping_confidence.is_strong())
                    && facts.support(id).tier != SupportTier::Unsupported
            })?;
            (asked_for, reasons::COEXISTENCE_REQUESTED)
        } else {
            return None;
        };

        let source = facts.package.source.clone();
        let primary = if rank(facts, &other) > rank(facts, &source) {
            other.clone()
        } else {
            source.clone()
        };
        let secondary = if primary == source { other } else { source };

        let mut reason = Reason::new(trigger)
            .with("primary", &primary)
            .with("other", &secondary);
        if trigger == reasons::PRESENT_UNDER_BOTH_PROVIDERS {
            reason = reason.with("criterion", &criterion(facts, &primary));
        }
        Some(RuleVerdict::new(Outcome::KeepBoth { primary }, reason))
    }
}

/// A prebuilt artifact counts for more than a support claim, because it is an
/// observation about this machine and the claim is a policy about a class of
/// them. A provider that states it does not support this machine scores zero
/// and so can never be named primary: whichever side the tie-break would
/// otherwise favour, sending the user's shell at a provider that says it will
/// not run here is not a first choice.
fn rank(facts: &Facts, id: &ProviderId) -> u32 {
    if facts.support(id).tier == SupportTier::Unsupported {
        return 0;
    }
    let mut score = 1;
    if facts
        .at(id)
        .is_some_and(|per| per.availability.is_prebuilt())
    {
        score += 2;
    }
    if facts.support(id).tier == SupportTier::Supported {
        score += 1;
    }
    score
}

fn criterion(facts: &Facts, primary: &ProviderId) -> &'static str {
    if facts
        .at(primary)
        .is_some_and(|per| per.availability.is_prebuilt())
    {
        "PREBUILT_ARTIFACT"
    } else if facts.support(primary).tier == SupportTier::Supported {
        "SUPPORTED_HERE"
    } else {
        "ALREADY_IN_PLACE"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::testing;
    use macportsbrew_domain::ArtifactAvailability;

    fn data() -> PolicyData {
        PolicyData::load_embedded().unwrap()
    }

    fn held_by_both() -> Facts {
        let mut facts = testing::baseline();
        facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap()
            .installed = true;
        facts
    }

    #[test]
    fn one_side_only_does_not_fire_it() {
        assert!(KeepBothHealthy
            .evaluate(&testing::baseline(), &data())
            .is_none());
    }

    #[test]
    fn both_sides_holding_it_is_an_end_state() {
        let verdict = KeepBothHealthy.evaluate(&held_by_both(), &data()).unwrap();
        assert!(matches!(verdict.outcome, Outcome::KeepBoth { .. }));
        assert_eq!(
            verdict.reason.id.as_str(),
            reasons::PRESENT_UNDER_BOTH_PROVIDERS
        );
    }

    #[test]
    fn the_primary_is_the_side_with_the_artifact() {
        let mut facts = held_by_both();
        facts
            .package
            .per_provider
            .get_mut(&testing::alpha())
            .unwrap()
            .availability = testing::availability(ArtifactAvailability::Unavailable);
        let verdict = KeepBothHealthy.evaluate(&facts, &data()).unwrap();
        assert_eq!(
            verdict.outcome,
            Outcome::KeepBoth {
                primary: testing::beta()
            }
        );
        assert_eq!(
            verdict.reason.params.get("criterion").map(String::as_str),
            Some("PREBUILT_ARTIFACT")
        );
    }

    #[test]
    fn an_unhealthy_other_side_is_not_coexistence() {
        let mut facts = held_by_both();
        facts.providers.get_mut(&testing::beta()).unwrap().healthy = false;
        assert!(KeepBothHealthy.evaluate(&facts, &data()).is_none());
    }

    #[test]
    fn asking_for_it_is_enough_even_before_it_is_in_place() {
        let mut facts = testing::baseline();
        facts.package.coexistence_requested = true;
        let verdict = KeepBothHealthy.evaluate(&facts, &data()).unwrap();
        assert!(matches!(verdict.outcome, Outcome::KeepBoth { .. }));
        assert_eq!(verdict.reason.id.as_str(), reasons::COEXISTENCE_REQUESTED);
    }

    #[test]
    fn a_tie_leaves_the_side_already_holding_it_in_front() {
        let facts = held_by_both();
        let first = KeepBothHealthy.evaluate(&facts, &data()).unwrap();
        let again = KeepBothHealthy.evaluate(&facts, &data()).unwrap();
        assert_eq!(first, again);
        assert_eq!(
            first.outcome,
            Outcome::KeepBoth {
                primary: facts.package.source.clone()
            }
        );

        // And mirroring the facts mirrors the answer, which a tie broken on
        // the provider id would not do.
        let swapped = testing::swap_providers(&facts, &testing::alpha(), &testing::beta());
        let mirrored = KeepBothHealthy.evaluate(&swapped, &data()).unwrap();
        assert_eq!(
            mirrored.outcome,
            Outcome::KeepBoth {
                primary: testing::beta()
            }
        );
    }

    #[test]
    fn the_same_facts_with_the_sides_exchanged_swap_the_primary() {
        let mut facts = held_by_both();
        facts
            .package
            .per_provider
            .get_mut(&testing::alpha())
            .unwrap()
            .availability = testing::availability(ArtifactAvailability::Unavailable);
        let first = KeepBothHealthy.evaluate(&facts, &data()).unwrap();
        let swapped = testing::swap_providers(&facts, &testing::alpha(), &testing::beta());
        let second = KeepBothHealthy.evaluate(&swapped, &data()).unwrap();
        assert_eq!(
            second.outcome,
            testing::swap_outcome(&first.outcome, &testing::alpha(), &testing::beta())
        );
    }
}
