// SPDX-License-Identifier: Apache-2.0

//! `MANUAL_REVIEW_SOURCE_WITHDRAWN`: the provider holding the package has
//! withdrawn it.

use super::PRIORITY_MANUAL_REVIEW;
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, Reason};

/// Fires when the source provider marks the installed package deprecated,
/// disabled or superseded. The package still works today, but its provider
/// will not replace it, so "nothing to do" would be the wrong answer: the
/// user has a decision to make, and the fixture matrix showed a disabled
/// formula being reported as `NO_ACTION` before this rule existed.
pub struct ManualReviewSourceWithdrawn;

impl Rule for ManualReviewSourceWithdrawn {
    fn id(&self) -> &'static str {
        "MANUAL_REVIEW_SOURCE_WITHDRAWN"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "docs/spec/03-policy-and-recommendations.md, MANUAL_REVIEW is the default whenever availability is uncertain; docs/research/R-1-homebrew-read-interfaces.md (deprecated/disabled fields)"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_MANUAL_REVIEW
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        let per = facts.source_package()?;
        let withdrawn = per.deprecated || per.disabled || per.replaced_by.is_some();
        if !withdrawn {
            return None;
        }
        let state = if per.disabled {
            "DISABLED"
        } else if per.deprecated {
            "DEPRECATED"
        } else {
            "REPLACED"
        };
        let mut reason = Reason::new(reasons::SOURCE_WITHDRAWN)
            .with("provider", &facts.package.source)
            .with("package", &facts.package.name)
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
    fn a_healthy_source_package_does_not_fire() {
        let facts = testing::baseline();
        assert!(ManualReviewSourceWithdrawn
            .evaluate(&facts, &data())
            .is_none());
    }

    #[test]
    fn a_disabled_source_package_is_a_review_item() {
        let mut facts = testing::baseline();
        let source = facts.package.source.clone();
        facts
            .package
            .per_provider
            .get_mut(&source)
            .unwrap()
            .disabled = true;
        let verdict = ManualReviewSourceWithdrawn
            .evaluate(&facts, &data())
            .unwrap();
        assert_eq!(verdict.outcome, Outcome::ManualReview);
        assert_eq!(
            verdict.reason.params.get("state").map(String::as_str),
            Some("DISABLED")
        );
    }

    #[test]
    fn a_replaced_source_package_names_its_successor() {
        let mut facts = testing::baseline();
        let source = facts.package.source.clone();
        facts
            .package
            .per_provider
            .get_mut(&source)
            .unwrap()
            .replaced_by = Some("newer".into());
        let verdict = ManualReviewSourceWithdrawn
            .evaluate(&facts, &data())
            .unwrap();
        assert_eq!(
            verdict.reason.params.get("replacedBy").map(String::as_str),
            Some("newer")
        );
    }
}
