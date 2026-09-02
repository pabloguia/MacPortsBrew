// SPDX-License-Identifier: Apache-2.0

//! `NO_ACTION_ALREADY_THERE`: there is nowhere else on this machine.

use super::PRIORITY_NOTHING_TO_DO;
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, Reason};

/// Fires when the package is in place and no other provider is present on the
/// machine, so it is already in the only place it can be.
///
/// The distinction this draws is between `NO_ACTION` and `KEEP`, which
/// `03-policy-and-recommendations.md` lists as separate outcomes. `KEEP` is a
/// decision — the alternatives were weighed and this side won. `NO_ACTION` is
/// the absence of one: there was nothing to weigh. Collapsing them would make
/// the tool sound as though it had considered a choice the user does not
/// have, and `explain` would have nothing to show for it.
///
/// It sits below coexistence on purpose. A package held under two providers
/// is not "nowhere to go" — it has already gone, and INV-18 makes that a
/// first-class end state that this rule must not overwrite.
pub struct NoActionAlreadyThere;

impl Rule for NoActionAlreadyThere {
    fn id(&self) -> &'static str {
        "NO_ACTION_ALREADY_THERE"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "docs/spec/03-policy-and-recommendations.md, Outcomes, NO_ACTION"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_NOTHING_TO_DO
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        if !facts.source_package().is_some_and(|per| per.installed) {
            return None;
        }
        let somewhere_else = facts
            .providers
            .keys()
            .any(|id| *id != facts.package.source && facts.is_usable(id));
        if somewhere_else {
            return None;
        }

        Some(RuleVerdict::new(
            Outcome::NoAction,
            Reason::new(reasons::NO_ALTERNATIVE_PROVIDER_PRESENT)
                .with("provider", &facts.package.source)
                .with("package", &facts.package.name),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::testing;
    use macportsbrew_domain::ProviderPresence;

    fn data() -> PolicyData {
        PolicyData::load_embedded().unwrap()
    }

    #[test]
    fn another_provider_being_present_does_not_fire_it() {
        assert!(NoActionAlreadyThere
            .evaluate(&testing::baseline(), &data())
            .is_none());
    }

    #[test]
    fn nowhere_else_to_go_is_nothing_to_do() {
        let mut facts = testing::baseline();
        facts.providers.get_mut(&testing::beta()).unwrap().presence = ProviderPresence::Absent;
        let verdict = NoActionAlreadyThere.evaluate(&facts, &data()).unwrap();
        assert_eq!(verdict.outcome, Outcome::NoAction);
        assert_eq!(
            verdict.reason.id.as_str(),
            reasons::NO_ALTERNATIVE_PROVIDER_PRESENT
        );
    }

    #[test]
    fn a_broken_other_provider_is_not_somewhere_to_go() {
        let mut facts = testing::baseline();
        facts.providers.get_mut(&testing::beta()).unwrap().presence = ProviderPresence::Broken {
            reason: "PROVIDER_UNHEALTHY".to_string(),
        };
        assert!(NoActionAlreadyThere.evaluate(&facts, &data()).is_some());
    }

    #[test]
    fn a_package_not_in_place_has_something_to_do() {
        let mut facts = testing::baseline();
        facts.providers.get_mut(&testing::beta()).unwrap().presence = ProviderPresence::Absent;
        facts
            .package
            .per_provider
            .get_mut(&testing::alpha())
            .unwrap()
            .installed = false;
        assert!(NoActionAlreadyThere.evaluate(&facts, &data()).is_none());
    }
}
