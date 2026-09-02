// SPDX-License-Identifier: Apache-2.0

//! Recommendations and outcomes (`03-policy-and-recommendations.md`).
//!
//! The outcome vocabulary in the specification names the two providers
//! (`KEEP_HOMEBREW`, `MIGRATE_TO_MACPORTS`). Here the same outcomes are
//! parameterised by provider id so that neither direction is a special case
//! (INV-17) and a third provider needs no new variants. The JSON rendering
//! carries `kind` plus the provider ids.

use crate::provider::ProviderId;
use crate::reason::Reason;
use serde::{Deserialize, Serialize};

/// What the policy engine recommends for one package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "kind")]
pub enum Outcome {
    /// Keep the package where it is, under this provider.
    Keep {
        /// The provider to keep.
        provider: ProviderId,
    },
    /// Migrate to this provider.
    MigrateTo {
        /// Destination.
        provider: ProviderId,
    },
    /// Keep it under both providers, with one as primary.
    KeepBoth {
        /// The primary provider.
        primary: ProviderId,
    },
    /// The package is a runtime; suggest a runtime manager, never act.
    UseRuntimeManager,
    /// A human must look. The reasons say why.
    ManualReview,
    /// Nothing to do.
    NoAction,
}

impl Outcome {
    /// True when the outcome would, in a later pass, produce a mutating plan.
    pub fn is_actionable(&self) -> bool {
        matches!(self, Outcome::MigrateTo { .. } | Outcome::KeepBoth { .. })
    }
}

/// The recommendation for one package, with every reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    /// The outcome.
    pub outcome: Outcome,
    /// Why. Never empty (`03-policy`: a recommendation with zero reasons is
    /// a bug); enforced by [`Recommendation::new`].
    pub reasons: Vec<Reason>,
    /// Rules considered that did not fire, by id, sorted, for `explain`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules_not_fired: Vec<String>,
    /// The policy version that produced it.
    pub policy_version: String,
}

impl Recommendation {
    /// Builds a recommendation. Panics if `reasons` is empty, because that
    /// is a bug in the caller, not a runtime condition.
    pub fn new(
        outcome: Outcome,
        reasons: Vec<Reason>,
        rules_not_fired: Vec<String>,
        policy_version: impl Into<String>,
    ) -> Self {
        assert!(
            !reasons.is_empty(),
            "a recommendation with zero reasons is a bug (03-policy-and-recommendations.md)"
        );
        Self {
            outcome,
            reasons,
            rules_not_fired,
            policy_version: policy_version.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "zero reasons")]
    fn a_recommendation_needs_a_reason() {
        let _ = Recommendation::new(Outcome::NoAction, vec![], vec![], "0");
    }

    #[test]
    fn outcomes_serialise_with_provider_ids() {
        let o = Outcome::MigrateTo {
            provider: ProviderId::new("macports"),
        };
        let json = serde_json::to_string(&o).unwrap();
        assert_eq!(json, r#"{"kind":"MIGRATE_TO","provider":"macports"}"#);
        assert!(o.is_actionable());
        assert!(!Outcome::ManualReview.is_actionable());
    }
}
