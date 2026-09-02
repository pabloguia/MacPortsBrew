// SPDX-License-Identifier: Apache-2.0

//! The engine: a pure function from facts and data to a recommendation.
//!
//! `docs/spec/03-policy-and-recommendations.md` requires "a pure function
//! `(facts, policyVersion) -> (Recommendation, [Reason])`. No I/O, no clock
//! reads, no environment access inside the engine." That is enforced by
//! construction here — [`recommend`] takes two references and returns a
//! value, and there is nothing else in scope for it to reach.
//!
//! How a winner is chosen, in order:
//!
//! 1. Every rule is evaluated, in identifier order. Every one of them, every
//!    time: a rule that does not fire is reported, because the specification
//!    makes "the rules that were considered and did not fire" part of
//!    `explain`, and a rule silently skipped is a rule nobody audits.
//! 2. If any rule returned `MANUAL_REVIEW`, that is the answer. The test is
//!    the outcome, not the rule's name or its number, so a new rule that
//!    needs a human cannot be outvoted by forgetting to give it a priority.
//! 3. Otherwise the highest priority wins, ties going to the lower rule id.
//! 4. Every firing rule's reason is attached, stamped with its rule id — not
//!    only the winner's. The losing reasons are what make an explanation an
//!    explanation rather than an assertion.
//!
//! What the engine deliberately does **not** do is edit the winning outcome.
//! Everything a recommendation says comes from a rule that `explain` lists by
//! id; there is no step here that overrules one, and no reason is manufactured
//! without a rule behind it. Refusing an unsupported destination is a rule
//! (`DESTINATION_UNSUPPORTED_HERE`) and every rule that can name a destination
//! checks for itself, so an outcome naming a provider that will not run here
//! cannot be produced in the first place — `invariant_18_…` in
//! `tests/invariants.rs` asserts that across the scenario matrix.

use crate::data::PolicyData;
use crate::facts::Facts;
use crate::rules::{self, labels};
use macportsbrew_domain::{Outcome, Reason, Recommendation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One rule that fired, as `explain` reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiredRule {
    /// The rule's identifier.
    pub rule: String,
    /// What it would have the system do.
    pub outcome: Outcome,
    /// Why.
    pub reason: Reason,
}

/// The full derivation behind one recommendation: what was read, what fired,
/// what did not, what came out and under which policy version
/// (`03-policy-and-recommendations.md`, Explainability).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Derivation {
    /// The facts the engine read, as sorted name/value pairs.
    pub facts_used: Vec<(String, String)>,
    /// The rules that fired, in identifier order.
    pub fired: Vec<FiredRule>,
    /// The rules that were considered and did not fire, sorted.
    pub not_fired: Vec<String>,
    /// The result.
    pub recommendation: Recommendation,
    /// The policy version that produced it.
    pub policy_version: String,
}

/// The recommendation for one package.
pub fn recommend(facts: &Facts, data: &PolicyData) -> Recommendation {
    let (fired, not_fired) = evaluate(facts, data);
    decide(data, &fired, not_fired)
}

/// Turns one evaluation into a recommendation.
///
/// Separate from [`evaluate`] so that [`explain`] can report the very
/// evaluation its recommendation came from rather than a second one. The two
/// would agree — the engine is pure — but an explanation assembled from a
/// different run of the rules than the answer it explains is only accidentally
/// true, and this crate should not be relying on its own purity to stay
/// honest.
fn decide(data: &PolicyData, fired: &[(FiredRule, u32)], not_fired: Vec<String>) -> Recommendation {
    let manual = fired
        .iter()
        .find(|(entry, _)| matches!(entry.outcome, Outcome::ManualReview));

    let outcome = if let Some((review, _)) = manual {
        review.outcome.clone()
    } else {
        // Highest priority wins. `fired` is already in identifier order and
        // the comparison is strict, so the first rule at the winning priority
        // is the one that decides and the answer does not depend on anything
        // but the rules themselves (INV-15).
        let mut best: Option<&(FiredRule, u32)> = None;
        for entry in fired {
            if best.is_none_or(|(_, priority)| entry.1 > *priority) {
                best = Some(entry);
            }
        }
        best.map_or(Outcome::NoAction, |(entry, _)| entry.outcome.clone())
    };

    let reasons_out: Vec<Reason> = fired
        .iter()
        .map(|(entry, _)| entry.reason.clone().from_rule(&entry.rule))
        .collect();

    // `Recommendation::new` asserts that this list is not empty, and the
    // fallback rule guarantees it. Nothing is synthesised here to make that
    // true: a reason with no rule behind it would be an unattributable
    // sentence in front of a user, and `explain` could not account for it.
    Recommendation::new(outcome, reasons_out, not_fired, data.version())
}

/// The recommendation plus everything behind it.
pub fn explain(facts: &Facts, data: &PolicyData) -> Derivation {
    let (fired, not_fired) = evaluate(facts, data);
    let recommendation = decide(data, &fired, not_fired.clone());
    Derivation {
        facts_used: facts_used(facts),
        fired: fired.into_iter().map(|(entry, _)| entry).collect(),
        not_fired,
        recommendation,
        policy_version: data.version().to_string(),
    }
}

/// Every rule, evaluated once, in identifier order. The priority travels
/// alongside each verdict so that choosing a winner needs no second pass over
/// the rule list and cannot see a different one.
fn evaluate(facts: &Facts, data: &PolicyData) -> (Vec<(FiredRule, u32)>, Vec<String>) {
    let mut fired = Vec::new();
    let mut not_fired = Vec::new();
    for rule in rules::all() {
        match rule.evaluate(facts, data) {
            Some(verdict) => fired.push((
                FiredRule {
                    rule: rule.id().to_string(),
                    outcome: verdict.outcome,
                    reason: verdict.reason,
                },
                rule.priority(),
            )),
            None => not_fired.push(rule.id().to_string()),
        }
    }
    not_fired.sort();
    (fired, not_fired)
}

/// The facts the engine read, flattened for display. Built through a
/// `BTreeMap` so the order is the same on every run and on every platform.
fn facts_used(facts: &Facts) -> Vec<(String, String)> {
    let mut used: BTreeMap<String, String> = BTreeMap::new();
    used.insert(
        "machine.architecture".into(),
        facts.machine.architecture.to_string(),
    );
    used.insert("machine.macos".into(), facts.machine.macos.to_string());
    used.insert(
        "machine.translation".into(),
        labels::translation(facts.machine.translation).to_string(),
    );
    used.insert("today".into(), facts.today.clone());
    used.insert("policyVersion".into(), facts.policy_version.clone());
    used.insert("package.name".into(), facts.package.name.clone());
    used.insert("package.source".into(), facts.package.source.to_string());
    used.insert(
        "package.ownership".into(),
        labels::ownership(facts.package.ownership).to_string(),
    );
    used.insert(
        "package.classes".into(),
        facts
            .package
            .classes
            .classes
            .iter()
            .map(|c| labels::class(*c))
            .collect::<Vec<_>>()
            .join(","),
    );
    used.insert(
        "package.reverseDependencyCount".into(),
        facts.package.reverse_dependency_count.to_string(),
    );
    used.insert(
        "package.coexistenceRequested".into(),
        facts.package.coexistence_requested.to_string(),
    );

    for (id, provider) in &facts.providers {
        used.insert(
            format!("provider.{id}.support"),
            labels::tier(provider.support.tier).to_string(),
        );
        used.insert(
            format!("provider.{id}.healthy"),
            provider.healthy.to_string(),
        );
        if let Some(end) = &provider.support.end_date {
            used.insert(format!("provider.{id}.supportEndDate"), end.clone());
        }
    }
    for (id, per) in &facts.package.per_provider {
        used.insert(
            format!("package.{id}.availability"),
            labels::availability(&per.availability.availability).to_string(),
        );
        used.insert(
            format!("package.{id}.mappingConfidence"),
            labels::confidence(per.mapping_confidence).to_string(),
        );
        used.insert(format!("package.{id}.present"), per.installed.to_string());
    }

    used.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::shift_date;
    use crate::reasons;
    use crate::rules::{testing, SUPPORT_END_HORIZON_DAYS};
    use macportsbrew_domain::SupportTier;
    use macportsbrew_domain::{ArtifactAvailability, PackageClass};

    fn data() -> PolicyData {
        PolicyData::load_embedded().unwrap()
    }

    #[test]
    fn the_benign_case_keeps_the_package_where_it_is() {
        let recommendation = recommend(&testing::baseline(), &data());
        assert_eq!(
            recommendation.outcome,
            Outcome::Keep {
                provider: testing::alpha()
            }
        );
        assert!(!recommendation.reasons.is_empty());
        assert!(!recommendation.rules_not_fired.is_empty());
        assert_eq!(recommendation.policy_version, data().version());
    }

    #[test]
    fn manual_review_beats_everything_below_it() {
        let mut facts = testing::baseline();
        facts.providers.get_mut(&testing::alpha()).unwrap().support = testing::support_ending(
            SupportTier::Degraded,
            &shift_date(testing::TODAY, 30).unwrap(),
            true,
        );
        facts
            .package
            .classes
            .add(PackageClass::Service, "fixture", "fixture");

        let recommendation = recommend(&facts, &data());
        assert_eq!(recommendation.outcome, Outcome::ManualReview);
        // The overruled rule's reason is still attached: an explanation that
        // hides what it decided against is not one.
        let ids: Vec<&str> = recommendation
            .reasons
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        assert!(ids.contains(&reasons::SOURCE_SUPPORT_ENDING));
        assert!(ids.contains(&reasons::CLASS_REQUIRES_REVIEW));
    }

    #[test]
    fn every_attached_reason_names_the_rule_that_emitted_it() {
        let recommendation = recommend(&testing::baseline(), &data());
        for reason in &recommendation.reasons {
            assert!(reason.rule.is_some(), "{:?}", reason.id);
        }
    }

    #[test]
    fn the_engine_refuses_a_destination_that_will_not_run_here() {
        let mut facts = testing::baseline();
        facts
            .package
            .per_provider
            .get_mut(&testing::alpha())
            .unwrap()
            .availability = testing::availability(ArtifactAvailability::SourceBuildRequired {
            reason: macportsbrew_domain::availability::SourceBuildReason::NoArtifactForThisPlatform,
        });
        facts.providers.get_mut(&testing::beta()).unwrap().support =
            testing::support(SupportTier::Unsupported, false);

        let recommendation = recommend(&facts, &data());
        assert_eq!(
            recommendation.outcome,
            Outcome::Keep {
                provider: testing::alpha()
            }
        );
        assert!(recommendation
            .reasons
            .iter()
            .any(|r| r.id.as_str() == reasons::DESTINATION_UNSUPPORTED_HERE));
    }

    #[test]
    fn a_stated_end_of_support_moves_the_package() {
        let mut facts = testing::baseline();
        facts.providers.get_mut(&testing::alpha()).unwrap().support = testing::support_ending(
            SupportTier::Degraded,
            &shift_date(testing::TODAY, SUPPORT_END_HORIZON_DAYS / 2).unwrap(),
            true,
        );
        let recommendation = recommend(&facts, &data());
        assert_eq!(
            recommendation.outcome,
            Outcome::MigrateTo {
                provider: testing::beta()
            }
        );
    }

    #[test]
    fn explain_reports_what_fired_and_what_did_not() {
        let derivation = explain(&testing::baseline(), &data());
        assert!(!derivation.facts_used.is_empty());
        assert!(!derivation.fired.is_empty());
        assert!(!derivation.not_fired.is_empty());
        assert_eq!(derivation.policy_version, data().version());

        let total = derivation.fired.len() + derivation.not_fired.len();
        assert_eq!(total, crate::rules::all().len());

        let mut keys: Vec<&str> = derivation
            .facts_used
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        let sorted = {
            let mut copy = keys.clone();
            copy.sort_unstable();
            copy
        };
        assert_eq!(keys, sorted);
        keys.dedup();
        assert_eq!(keys.len(), derivation.facts_used.len());
    }

    #[test]
    fn every_reason_the_engine_emits_is_in_the_catalogue() {
        let mut facts = testing::baseline();
        facts.package.coexistence_requested = true;
        facts.package.reverse_dependency_count = 2;
        facts
            .package
            .classes
            .add(PackageClass::Runtime, "fixture", "fixture");
        for recommendation in [
            recommend(&testing::baseline(), &data()),
            recommend(&facts, &data()),
        ] {
            for reason in &recommendation.reasons {
                assert!(
                    reasons::all().contains(&reason.id.as_str()),
                    "{} is missing from the catalogue",
                    reason.id.as_str()
                );
            }
        }
    }
}
