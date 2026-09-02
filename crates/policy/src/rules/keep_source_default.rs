// SPDX-License-Identifier: Apache-2.0

//! `KEEP_SOURCE_DEFAULT`: when nothing argues for a change, nothing changes.

use super::PRIORITY_FALLBACK;
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, Reason};

/// Always fires, at the lowest priority, keeping the package where it is.
///
/// It exists for two reasons. The first is stated in
/// `03-policy-and-recommendations.md`: "a recommendation with zero reasons is
/// a bug". A fallback that always fires means the engine can never produce
/// one, and `Recommendation::new` asserts on it rather than trusting that.
///
/// The second is that doing nothing is the right default. Every other rule
/// has to earn its outcome from a fact about this machine and this package;
/// this one is what remains when none of them does, and it names the side the
/// package is already on rather than choosing between providers.
pub struct KeepSourceDefault;

impl Rule for KeepSourceDefault {
    fn id(&self) -> &'static str {
        "KEEP_SOURCE_DEFAULT"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "docs/spec/03-policy-and-recommendations.md, Shape: every recommendation carries its reasons"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_FALLBACK
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        Some(RuleVerdict::new(
            Outcome::Keep {
                provider: facts.package.source.clone(),
            },
            Reason::new(reasons::KEEP_SOURCE_DEFAULT)
                .with("provider", &facts.package.source)
                .with("package", &facts.package.name),
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
    fn it_always_fires_and_names_the_side_the_package_is_on() {
        let facts = testing::baseline();
        let verdict = KeepSourceDefault.evaluate(&facts, &data()).unwrap();
        assert_eq!(
            verdict.outcome,
            Outcome::Keep {
                provider: testing::alpha()
            }
        );
        assert_eq!(
            verdict.reason.params.get("provider").map(String::as_str),
            Some("alpha")
        );
    }

    #[test]
    fn it_reads_the_side_out_of_the_facts_rather_than_holding_one() {
        let facts = testing::baseline();
        let swapped = testing::swap_providers(&facts, &testing::alpha(), &testing::beta());
        let verdict = KeepSourceDefault.evaluate(&swapped, &data()).unwrap();
        assert_eq!(
            verdict.outcome,
            Outcome::Keep {
                provider: testing::beta()
            }
        );
    }
}
