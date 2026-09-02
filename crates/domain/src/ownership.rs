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
    /// Whether a normal-mode view shows this package (INV-13).
    pub fn is_top_level(self) -> bool {
        matches!(self, Ownership::TopLevel)
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
        None => (
            if has_dependents {
                Ownership::Dependency
            } else {
                Ownership::TopLevel
            },
            OwnershipEvidence {
                mechanism: "dependency_graph_inference".to_string(),
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
    fn inference_is_marked_as_such() {
        let (o, e) = classify_ownership(None, false, "ignored");
        assert_eq!(o, Ownership::TopLevel);
        assert!(!e.authoritative);
        assert_eq!(e.mechanism, "dependency_graph_inference");
        let (o, _) = classify_ownership(None, true, "ignored");
        assert_eq!(o, Ownership::Dependency);
    }
}
