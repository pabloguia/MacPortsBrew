// SPDX-License-Identifier: Apache-2.0

//! `MANUAL_REVIEW_CLASS`: a class on the package means a human decides.

use super::{labels, PRIORITY_MANUAL_REVIEW};
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, Reason};

/// A package whose class set contains `STATEFUL`, `SERVICE`, `GUI_APP` or
/// `THIRD_PARTY_SOURCED` is never moved on the engine's own authority
/// (INV-19; `03-policy-and-recommendations.md`, `MANUAL_REVIEW`).
///
/// The class precedence lives in the domain, not here: `ClassSet::dominant`
/// already puts a class that forces review ahead of a benign one, so a
/// runtime that also defines a service cannot have its service hidden.
pub struct ManualReviewClass;

impl Rule for ManualReviewClass {
    fn id(&self) -> &'static str {
        "MANUAL_REVIEW_CLASS"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "INV-19; docs/spec/03-policy-and-recommendations.md, Starting policy, MANUAL_REVIEW"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_MANUAL_REVIEW
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        if !facts.package.classes.forces_manual_review() {
            return None;
        }
        let dominant = facts.package.classes.dominant();
        Some(RuleVerdict::new(
            Outcome::ManualReview,
            Reason::new(reasons::CLASS_REQUIRES_REVIEW)
                .with("class", &labels::class(dominant))
                .with("package", &facts.package.name),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::testing;
    use macportsbrew_domain::PackageClass;

    fn data() -> PolicyData {
        PolicyData::load_embedded().unwrap()
    }

    #[test]
    fn a_plain_package_does_not_fire_it() {
        assert!(ManualReviewClass
            .evaluate(&testing::baseline(), &data())
            .is_none());
    }

    #[test]
    fn each_reviewable_class_fires_it() {
        for class in [
            PackageClass::Stateful,
            PackageClass::Service,
            PackageClass::GuiApp,
            PackageClass::ThirdPartySourced,
        ] {
            let mut facts = testing::baseline();
            facts.package.classes.add(class, "fixture", "fixture");
            let verdict = ManualReviewClass
                .evaluate(&facts, &data())
                .unwrap_or_else(|| panic!("{class:?} must force review"));
            assert_eq!(verdict.outcome, Outcome::ManualReview);
            assert_eq!(verdict.reason.id.as_str(), reasons::CLASS_REQUIRES_REVIEW);
        }
    }

    #[test]
    fn a_benign_class_does_not_hide_a_reviewable_one() {
        let mut facts = testing::baseline();
        facts
            .package
            .classes
            .add(PackageClass::Runtime, "fixture", "fixture");
        assert!(ManualReviewClass.evaluate(&facts, &data()).is_none());

        facts
            .package
            .classes
            .add(PackageClass::Service, "fixture", "fixture");
        let verdict = ManualReviewClass.evaluate(&facts, &data()).unwrap();
        assert_eq!(
            verdict.reason.params.get("class").map(String::as_str),
            Some("SERVICE")
        );
    }
}
