// SPDX-License-Identifier: Apache-2.0

//! The rules.
//!
//! `docs/spec/03-policy-and-recommendations.md` requires that
//! recommendations come from an explicit rule engine rather than nested
//! conditionals, and that each rule is a named, independently testable unit
//! carrying `since`, `source` and `reviewedOn`. Each rule here is one file
//! with its own tests.
//!
//! Two properties every rule in this module holds to, and which the tests in
//! `tests/invariants.rs` check across all of them at once:
//!
//! * **No rule may encode "provider X is better"** (03-policy, MUST NOT).
//!   Provider ids are values a rule reads out of the facts and passes
//!   through; none is written down anywhere in this crate.
//! * **A rule reads facts and data, and nothing else.** It cannot perform an
//!   action, consult a clock, or reach for information that is not in its two
//!   arguments.

use crate::data::PolicyData;
use crate::facts::Facts;
use macportsbrew_domain::{Outcome, Reason};

mod destination_unsupported_here;
mod keep_both_healthy;
mod keep_source_default;
pub(crate) mod labels;
mod manual_review_availability_unknown;
mod manual_review_class;
mod manual_review_destination_deprecated;
mod manual_review_mapping_confidence;
mod manual_review_persistent_data;
mod manual_review_reverse_dependencies;
mod no_action_already_there;
mod runtime_use_manager;
mod source_no_prebuilt_destination_prebuilt;
mod source_support_ending;

/// Test fixtures. Compiled for this crate's own tests, and for anything that
/// turns on the `testing` feature; absent from a default build, because a
/// library should not ship the scaffolding its tests are built on.
#[cfg(any(test, feature = "testing"))]
pub mod testing;

/// What a rule says when it fires: one outcome and the reason for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleVerdict {
    /// What the rule would have the system do.
    pub outcome: Outcome,
    /// Why. The engine stamps the rule id onto it.
    pub reason: Reason,
}

impl RuleVerdict {
    /// Builds a verdict.
    pub fn new(outcome: Outcome, reason: Reason) -> Self {
        Self { outcome, reason }
    }
}

/// One named, independently testable policy rule.
pub trait Rule {
    /// Stable identifier, `SCREAMING_SNAKE_CASE`. Appears in `explain`
    /// output and in every reason the rule emits, so changing one is a
    /// compatibility change.
    fn id(&self) -> &'static str;

    /// The policy version the rule was introduced in.
    fn since(&self) -> &'static str;

    /// Where the rule comes from: a specification section, an invariant, or a
    /// research note. A rule with no source is an opinion.
    fn source(&self) -> &'static str;

    /// When the rule's reasoning was last checked against its source.
    fn reviewed_on(&self) -> &'static str;

    /// How strongly the rule's outcome is held, when several fire. Higher
    /// decides; ties go to the lower rule id, so the answer never depends on
    /// declaration order (INV-15).
    fn priority(&self) -> u32;

    /// The rule. `None` means it did not fire, which is a first-class answer
    /// and is reported by `explain`.
    fn evaluate(&self, facts: &Facts, data: &PolicyData) -> Option<RuleVerdict>;
}

/// A human must look. Nothing below this can override it, and the engine
/// enforces that by outcome rather than by number (INV-19).
pub const PRIORITY_MANUAL_REVIEW: u32 = 1000;
/// A runtime is a runtime whatever the providers say about it.
pub const PRIORITY_RUNTIME: u32 = 950;
/// Refusing to name an unsupported destination outranks wanting to move.
pub const PRIORITY_DESTINATION_VETO: u32 = 900;
/// A stated end of support outranks a mere artifact advantage.
pub const PRIORITY_SUPPORT_ENDING: u32 = 700;
/// Compiling from source where the other side has an artifact ready.
pub const PRIORITY_ARTIFACT_ADVANTAGE: u32 = 600;
/// Keeping both is an end state, not a step (INV-18).
pub const PRIORITY_COEXISTENCE: u32 = 500;
/// There is nowhere else to go.
pub const PRIORITY_NOTHING_TO_DO: u32 = 200;
/// The fallback, which always fires so that a recommendation always has a
/// reason (03-policy: a recommendation with zero reasons is a bug).
pub const PRIORITY_FALLBACK: u32 = 100;

/// How far ahead a stated end of support starts to weigh, in days. Two
/// years: long enough that a user has room to plan, short enough that the
/// warning still means something when it appears. The date itself is data
/// (INV-20); only this horizon is a policy choice.
pub const SUPPORT_END_HORIZON_DAYS: i64 = 730;

/// Every rule, in identifier order. The order is part of the contract: the
/// engine evaluates in it and breaks ties by it (INV-15).
pub fn all() -> Vec<Box<dyn Rule>> {
    let mut rules: Vec<Box<dyn Rule>> = vec![
        Box::new(destination_unsupported_here::DestinationUnsupportedHere),
        Box::new(keep_both_healthy::KeepBothHealthy),
        Box::new(keep_source_default::KeepSourceDefault),
        Box::new(manual_review_availability_unknown::ManualReviewAvailabilityUnknown),
        Box::new(manual_review_class::ManualReviewClass),
        Box::new(manual_review_destination_deprecated::ManualReviewDestinationDeprecated),
        Box::new(manual_review_mapping_confidence::ManualReviewMappingConfidence),
        Box::new(manual_review_persistent_data::ManualReviewPersistentData),
        Box::new(manual_review_reverse_dependencies::ManualReviewReverseDependencies),
        Box::new(no_action_already_there::NoActionAlreadyThere),
        Box::new(runtime_use_manager::RuntimeUseManager),
        Box::new(source_no_prebuilt_destination_prebuilt::SourceNoPrebuiltDestinationPrebuilt),
        Box::new(source_support_ending::SourceSupportEnding),
    ];
    rules.sort_by_key(|r| r.id());
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_rule_is_named_sourced_and_dated() {
        let rules = all();
        assert!(rules.len() >= 12);
        let ids: BTreeSet<&str> = rules.iter().map(|r| r.id()).collect();
        assert_eq!(ids.len(), rules.len(), "rule ids must be distinct");
        for rule in &rules {
            let id = rule.id();
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'),
                "{id} must be SCREAMING_SNAKE_CASE"
            );
            assert!(!rule.since().is_empty(), "{id} has no since");
            assert!(!rule.source().is_empty(), "{id} has no source");
            assert_eq!(rule.reviewed_on().len(), 10, "{id} review date");
            assert!(rule.priority() > 0, "{id} has no priority");
        }
    }

    #[test]
    fn the_rule_list_is_ordered_by_id() {
        let ids: Vec<&str> = all().iter().map(|r| r.id()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }
}
