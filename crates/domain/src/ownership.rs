// SPDX-License-Identifier: Apache-2.0

//! Ownership: a fact about a provider, not about the machine
//! (`02-domain-model.md`, "Ownership").

use serde::{Deserialize, Serialize};

/// Why a package is installed, from the provider's point of view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Ownership {
    /// The user asked for it.
    TopLevel,
    /// Installed to satisfy something else that is still installed.
    Dependency,
    /// Installed as a dependency; nothing installed depends on it now.
    Orphan,
    /// The provider could not say.
    Unknown,
}

impl Ownership {
    /// Whether the provider's own record says the user asked for this.
    ///
    /// Strictly `TOP_LEVEL`. This is what `topLevelCount` reports, and its
    /// schema definition — "those the provider's own record says the user
    /// asked for" — is why an `UNKNOWN` must not be counted here.
    pub fn is_top_level(self) -> bool {
        matches!(self, Ownership::TopLevel)
    }

    /// Whether a normal-mode view shows this package (INV-13).
    ///
    /// Wider than [`is_top_level`](Self::is_top_level), and deliberately: a
    /// package whose ownership is **unknown** is not a package known to be a
    /// dependency, and hiding it behind `--advanced` would make the default
    /// view quieter on exactly the rows nobody has established anything about.
    /// INV-13 says `--advanced` reveals and never hides; that decides the
    /// direction. Recorded as the owner's call, DP-012, 2026-09-05.
    ///
    /// The two answers coincided until `Ownership::Unknown` became reachable,
    /// which is why one method used to do both jobs.
    pub fn shown_in_normal_mode(self) -> bool {
        matches!(self, Ownership::TopLevel | Ownership::Unknown)
    }
}

/// Which mechanism decided the ownership, and how far to trust it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnershipEvidence {
    /// The provider record or inference used, as a stable identifier
    /// (`installed_on_request`, `requested_pseudo_portname`,
    /// `dependency_graph_inference`).
    pub mechanism: String,
    /// `true` when the provider's own intent record answered; `false` when
    /// the answer was inferred from a dependency graph.
    pub authoritative: bool,
}

/// Pure ownership rule shared by every provider.
///
/// `requested` is the provider's own record when it has one; `has_dependents`
/// is whether anything installed in the same provider depends on the
/// package. When the record is absent the graph decides, and the evidence
/// says so.
pub fn classify_ownership(
    requested: Option<bool>,
    has_dependents: bool,
    mechanism: &str,
) -> (Ownership, OwnershipEvidence) {
    match requested {
        Some(true) => (
            Ownership::TopLevel,
            OwnershipEvidence {
                mechanism: mechanism.to_string(),
                authoritative: true,
            },
        ),
        Some(false) => (
            if has_dependents {
                Ownership::Dependency
            } else {
                Ownership::Orphan
            },
            OwnershipEvidence {
                mechanism: mechanism.to_string(),
                authoritative: true,
            },
        ),
        // No record: `UNKNOWN`, and never a graph inference. `AGENTS.md`,
        // `.claude/rules/adapters.md` and `02-domain-model.md` have all said so
        // since the foundation pass while this arm quietly answered `TopLevel`
        // — sixteen per cent of the reviewer's default view, on evidence the
        // code itself marked non-authoritative (DP-012, SE-046).
        //
        // `has_dependents` is deliberately unread. Inferring from the graph is
        // the thing the rule forbids, not a fallback it permits, and the
        // mechanism name says which of the two happened.
        None => (
            Ownership::Unknown,
            OwnershipEvidence {
                mechanism: "no_provider_record".to_string(),
                authoritative: false,
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_record_wins_over_the_graph() {
        let (o, e) = classify_ownership(Some(true), true, "installed_on_request");
        assert_eq!(o, Ownership::TopLevel);
        assert!(e.authoritative);
        let (o, _) = classify_ownership(Some(false), true, "installed_on_request");
        assert_eq!(o, Ownership::Dependency);
        let (o, _) = classify_ownership(Some(false), false, "installed_on_request");
        assert_eq!(o, Ownership::Orphan);
    }

    #[test]
    fn no_record_is_unknown_and_the_graph_does_not_get_a_vote() {
        // This test used to be `inference_is_marked_as_such`, and asserted that
        // an absent record produced `TopLevel` (or `Dependency` when something
        // depended on it) marked non-authoritative. DP-012 removed the
        // inference rather than the marking: three documents had said since the
        // foundation pass that ownership is never inferred from a dependency
        // graph, while this arm inferred it.
        let (o, e) = classify_ownership(None, false, "ignored");
        assert_eq!(o, Ownership::Unknown);
        assert!(!e.authoritative);
        assert_eq!(e.mechanism, "no_provider_record");

        // The same answer with dependents. `has_dependents` is the graph, and
        // the graph is the input the rule refuses — an assertion, because the
        // obvious "improvement" is to use it as a fallback.
        let (o, e) = classify_ownership(None, true, "ignored");
        assert_eq!(o, Ownership::Unknown);
        assert_eq!(e.mechanism, "no_provider_record");
    }

    #[test]
    fn an_unknown_ownership_is_shown_and_is_not_a_recorded_top_level() {
        // The two predicates that used to be one. DP-012, decided 2026-09-05.
        assert!(Ownership::Unknown.shown_in_normal_mode());
        assert!(!Ownership::Unknown.is_top_level());
        assert!(Ownership::TopLevel.shown_in_normal_mode());
        assert!(Ownership::TopLevel.is_top_level());
        for hidden in [Ownership::Dependency, Ownership::Orphan] {
            assert!(!hidden.shown_in_normal_mode(), "{hidden:?}");
            assert!(!hidden.is_top_level(), "{hidden:?}");
        }
    }
}
