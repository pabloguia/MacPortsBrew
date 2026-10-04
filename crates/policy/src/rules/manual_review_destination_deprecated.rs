// SPDX-License-Identifier: Apache-2.0

//! `MANUAL_REVIEW_DESTINATION_DEPRECATED`: the other side has withdrawn the
//! equivalent.

use super::{labels, PRIORITY_MANUAL_REVIEW};
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, Reason};

/// Fires when a reachable destination marks its equivalent deprecated,
/// disabled, or superseded by another package
/// (`03-policy-and-recommendations.md`, `MANUAL_REVIEW`: deprecated or
/// disabled on the destination side).
///
/// Only providers that answer on this machine are considered: a withdrawn
/// package under a provider the user does not have is not a decision anyone
/// can take. [`Facts::mapped_destinations`] draws that line.
///
/// A superseding name is reported rather than followed. Both providers
/// publish such a field, and in both it is a pointer for a person, not an
/// instruction: the successor may have a different major version, different
/// defaults, or a different data layout, and none of that is visible from the
/// pointer itself.
pub struct ManualReviewDestinationDeprecated;

impl Rule for ManualReviewDestinationDeprecated {
    fn id(&self) -> &'static str {
        "MANUAL_REVIEW_DESTINATION_DEPRECATED"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "docs/spec/03-policy-and-recommendations.md, Starting policy, MANUAL_REVIEW; docs/research/R-4-macports-binary-archives.md finding 10"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_MANUAL_REVIEW
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        let (destination, per) = facts.mapped_destinations().into_iter().find_map(|id| {
            facts.at(&id).and_then(|per| {
                let withdrawn = per.deprecated || per.disabled || per.replaced_by.is_some();
                withdrawn.then(|| (id.clone(), per))
            })
        })?;

        let state = labels::withdrawal_state(per.disabled, per.deprecated);

        let mut reason = Reason::new(reasons::DESTINATION_DEPRECATED)
            .with("destination", &destination)
            .with("state", &state);
        if let Some(successor) = per.replaced_by.as_deref() {
            reason = reason.with("replacedBy", &successor);
        }
        Some(RuleVerdict::new(Outcome::ManualReview, reason))
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
    fn a_current_destination_does_not_fire_it() {
        assert!(ManualReviewDestinationDeprecated
            .evaluate(&testing::baseline(), &data())
            .is_none());
    }

    /// Which of the three withdrawal markers to set on the other side.
    enum Withdrawal {
        Deprecated,
        Disabled,
        Replaced,
    }

    fn withdrawn(kind: &Withdrawal) -> Facts {
        let mut facts = testing::baseline();
        let per = facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap();
        match kind {
            Withdrawal::Deprecated => per.deprecated = true,
            Withdrawal::Disabled => per.disabled = true,
            Withdrawal::Replaced => per.replaced_by = Some("successor".to_string()),
        }
        facts
    }

    #[test]
    fn each_withdrawn_state_fires_it_with_its_own_label() {
        for (kind, expected) in [
            (Withdrawal::Deprecated, "DEPRECATED"),
            (Withdrawal::Disabled, "DISABLED"),
            (Withdrawal::Replaced, "REPLACED"),
        ] {
            let facts = withdrawn(&kind);
            let verdict = ManualReviewDestinationDeprecated
                .evaluate(&facts, &data())
                .unwrap();
            assert_eq!(verdict.outcome, Outcome::ManualReview);
            assert_eq!(
                verdict.reason.params.get("state").map(String::as_str),
                Some(expected)
            );
        }
    }

    #[test]
    fn a_successor_is_reported_and_not_followed() {
        let mut facts = testing::baseline();
        facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap()
            .replaced_by = Some("successor".to_string());
        let verdict = ManualReviewDestinationDeprecated
            .evaluate(&facts, &data())
            .unwrap();
        assert_eq!(
            verdict.reason.params.get("replacedBy").map(String::as_str),
            Some("successor")
        );
        assert_eq!(verdict.outcome, Outcome::ManualReview);
    }

    #[test]
    fn a_withdrawn_package_under_a_provider_that_is_not_here_is_not_a_destination() {
        let mut facts = withdrawn(&Withdrawal::Deprecated);
        assert!(ManualReviewDestinationDeprecated
            .evaluate(&facts, &data())
            .is_some());

        facts.providers.get_mut(&testing::beta()).unwrap().presence =
            macportsbrew_domain::ProviderPresence::Absent;
        assert!(ManualReviewDestinationDeprecated
            .evaluate(&facts, &data())
            .is_none());
    }

    #[test]
    fn a_withdrawn_package_with_no_candidate_is_not_a_destination() {
        let mut facts = testing::baseline();
        let per = facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap();
        per.deprecated = true;
        per.mapping_confidence = macportsbrew_domain::Confidence::None;
        assert!(ManualReviewDestinationDeprecated
            .evaluate(&facts, &data())
            .is_none());
    }
}
