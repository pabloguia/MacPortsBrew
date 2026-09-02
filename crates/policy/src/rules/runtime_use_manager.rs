// SPDX-License-Identifier: Apache-2.0

//! `RUNTIME_USE_MANAGER`: a language runtime is not a packaging question.

use super::PRIORITY_RUNTIME;
use crate::data::PolicyData;
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, PackageClass, Reason};

/// Fires when the package's class set contains `RUNTIME`
/// (`03-policy-and-recommendations.md`: "Package class is `RUNTIME`. Suggest,
/// never act.").
///
/// The outcome is a suggestion and the engine has no other kind of runtime
/// answer: neither provider is named, because for a runtime the right tool is
/// usually neither of them. The managers come from
/// `data/policy/runtime-managers.toml`, which carries a review date because
/// the specification says recommendations here age fast; the rule emits their
/// ids and the UI renders them.
///
/// A runtime that is also a service or owns data still ends in manual review:
/// the manual-review rules fire alongside this one and dominate by outcome,
/// which is `R-8` finding 14's point that classes are not exclusive.
pub struct RuntimeUseManager;

impl Rule for RuntimeUseManager {
    fn id(&self) -> &'static str {
        "RUNTIME_USE_MANAGER"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "docs/spec/03-policy-and-recommendations.md, Starting policy, USE_RUNTIME_MANAGER; docs/research/R-8-stateful-and-service-packages.md findings 13 and 14"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_RUNTIME
    }

    fn evaluate(&self, facts: &Facts, data: &PolicyData) -> Option<RuleVerdict> {
        if !facts.package.classes.has(PackageClass::Runtime) {
            return None;
        }
        let family = facts.package.family();
        let managers = data
            .runtime_managers_for(&family)
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>()
            .join(",");

        Some(RuleVerdict::new(
            Outcome::UseRuntimeManager,
            Reason::new(reasons::RUNTIME_FAMILY_MANAGED_ELSEWHERE)
                .with("family", &family)
                .with("managers", &managers),
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
    fn a_plain_package_does_not_fire_it() {
        assert!(RuntimeUseManager
            .evaluate(&testing::baseline(), &data())
            .is_none());
    }

    #[test]
    fn a_runtime_gets_the_managers_that_name_its_family() {
        let mut facts = testing::baseline();
        facts.package.name = "python@3.12".to_string();
        facts
            .package
            .classes
            .add(PackageClass::Runtime, "fixture", "fixture");
        let verdict = RuntimeUseManager.evaluate(&facts, &data()).unwrap();
        assert_eq!(verdict.outcome, Outcome::UseRuntimeManager);
        assert_eq!(
            verdict.reason.params.get("family").map(String::as_str),
            Some("python")
        );
        let managers = verdict.reason.params.get("managers").unwrap();
        assert!(managers.contains("pyenv"), "{managers}");
        assert!(!managers.contains("rbenv"), "{managers}");
    }

    #[test]
    fn a_runtime_with_no_known_manager_still_gets_the_outcome() {
        let mut facts = testing::baseline();
        facts.package.name = "obscurelang".to_string();
        facts
            .package
            .classes
            .add(PackageClass::Runtime, "fixture", "fixture");
        let verdict = RuntimeUseManager.evaluate(&facts, &data()).unwrap();
        assert_eq!(verdict.outcome, Outcome::UseRuntimeManager);
        assert_eq!(
            verdict.reason.params.get("managers").map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn the_outcome_names_no_provider() {
        let mut facts = testing::baseline();
        facts
            .package
            .classes
            .add(PackageClass::Runtime, "fixture", "fixture");
        let verdict = RuntimeUseManager.evaluate(&facts, &data()).unwrap();
        assert!(!verdict.outcome.is_actionable());
    }
}
