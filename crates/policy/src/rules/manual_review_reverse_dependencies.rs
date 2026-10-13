// SPDX-License-Identifier: Apache-2.0

//! `MANUAL_REVIEW_REVERSE_DEPENDENCIES`: something depends on this.

use super::PRIORITY_MANUAL_REVIEW;
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, Reason};

/// Fires when anything inside the same provider depends on the package
/// (`03-policy-and-recommendations.md`, `MANUAL_REVIEW`: reverse-dependency
/// blocker present).
///
/// The count is within the source provider only. INV-04 is explicit that
/// reverse dependencies are resolved inside one provider and INV-08 that one
/// provider's package never satisfies another's requirement, so a
/// cross-provider count would not merely be unhelpful, it would be a
/// statement the domain forbids.
///
/// Any non-zero count fires it. The facts carry a count and nothing finer, so
/// "blocker present" can only mean "there is at least one", and rounding that
/// toward silence would be reading approval into the absence of information.
pub struct ManualReviewReverseDependencies;

impl Rule for ManualReviewReverseDependencies {
    fn id(&self) -> &'static str {
        "MANUAL_REVIEW_REVERSE_DEPENDENCIES"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "INV-04; docs/spec/03-policy-and-recommendations.md, Starting policy, MANUAL_REVIEW"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_MANUAL_REVIEW
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        // The source side's own count, read from the per-provider map. The
        // provider named in the reason is therefore the provider the number
        // came from, which was not true while a single cross-provider maximum
        // stood in for it.
        let count = facts.source_package()?.reverse_dependency_count;
        if count == 0 {
            return None;
        }
        Some(RuleVerdict::new(
            Outcome::ManualReview,
            Reason::new(reasons::REVERSE_DEPENDENCIES_PRESENT)
                .with("count", &count)
                .with("provider", &facts.package.source),
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
    fn nothing_depending_on_it_does_not_fire_it() {
        assert!(ManualReviewReverseDependencies
            .evaluate(&testing::baseline(), &data())
            .is_none());
    }

    #[test]
    fn one_dependent_is_enough() {
        let mut facts = testing::baseline();
        testing::with_reverse_dependencies(&mut facts, 1);
        let verdict = ManualReviewReverseDependencies
            .evaluate(&facts, &data())
            .unwrap();
        assert_eq!(verdict.outcome, Outcome::ManualReview);
        assert_eq!(
            verdict.reason.params.get("count").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            verdict.reason.params.get("provider").map(String::as_str),
            Some("alpha")
        );
    }
}
