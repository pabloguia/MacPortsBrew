// SPDX-License-Identifier: Apache-2.0

//! Mapping a package in one provider to its equivalent in another
//! (`02-domain-model.md`, "Mapping"). Never by name alone; evidence is
//! recorded and displayable; `AMBIGUOUS` and `NONE` are first-class results.

use crate::package::PackageIdentity;
use crate::provider::ProviderId;
use serde::{Deserialize, Serialize};

/// How sure the mapping is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Confidence {
    /// No candidate at all.
    None,
    /// Several candidates, none clearly better.
    Ambiguous,
    /// One candidate, weak evidence.
    Probable,
    /// One candidate, strong converging evidence.
    Strong,
    /// A curated entry or an identical upstream identity.
    Exact,
}

impl Confidence {
    /// True at `STRONG` or better: the threshold policy rules use.
    pub fn is_strong(self) -> bool {
        self >= Confidence::Strong
    }
}

/// One piece of evidence for a candidate.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingEvidence {
    /// Stable kind: `normalised_name`, `homepage`, `upstream_repository`,
    /// `description_similarity`, `executables`, `upstream_version`,
    /// `curated`, `versioned_family`.
    pub kind: String,
    /// What matched, as text.
    pub detail: String,
    /// The contribution of this evidence to the score, for `explain`.
    pub weight: u32,
}

/// One candidate equivalent.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingCandidate {
    /// The candidate package in the destination provider.
    pub package: PackageIdentity,
    /// The candidate's own confidence.
    pub confidence: Confidence,
    /// The evidence, sorted. Never empty.
    pub evidence: Vec<MappingEvidence>,
}

/// The mapping of one source package toward one destination provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageMapping {
    /// The source package.
    pub source: PackageIdentity,
    /// The destination provider.
    pub destination: ProviderId,
    /// All candidates, best first.
    pub candidates: Vec<MappingCandidate>,
    /// The chosen candidate, when confidence suffices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<MappingCandidate>,
    /// Overall confidence.
    pub confidence: Confidence,
}

impl PackageMapping {
    /// The "no equivalent" result.
    pub fn none(source: PackageIdentity, destination: ProviderId) -> Self {
        Self {
            source,
            destination,
            candidates: Vec::new(),
            selected: None,
            confidence: Confidence::None,
        }
    }

    /// Builds a mapping from scored candidates (best first): a single best
    /// candidate is selected when it is `PROBABLE` or better and clearly
    /// ahead of the runner-up; two candidates at the same confidence make
    /// the mapping `AMBIGUOUS`.
    pub fn from_candidates(
        source: PackageIdentity,
        destination: ProviderId,
        mut candidates: Vec<MappingCandidate>,
    ) -> Self {
        candidates.sort_by(|a, b| {
            b.confidence
                .cmp(&a.confidence)
                .then_with(|| a.package.name.cmp(&b.package.name))
        });
        let (selected, confidence) = match candidates.as_slice() {
            [] => (None, Confidence::None),
            [best] => (
                (best.confidence >= Confidence::Probable).then(|| best.clone()),
                best.confidence,
            ),
            [best, second, ..] => {
                if best.confidence > second.confidence && best.confidence >= Confidence::Probable {
                    (Some(best.clone()), best.confidence)
                } else {
                    (None, Confidence::Ambiguous)
                }
            }
        };
        Self {
            source,
            destination,
            candidates,
            selected,
            confidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(name: &str, c: Confidence) -> MappingCandidate {
        MappingCandidate {
            package: PackageIdentity::new(ProviderId::new("b"), name),
            confidence: c,
            evidence: vec![MappingEvidence {
                kind: "test".into(),
                detail: name.into(),
                weight: 1,
            }],
        }
    }

    #[test]
    fn two_equal_candidates_are_ambiguous() {
        let m = PackageMapping::from_candidates(
            PackageIdentity::new(ProviderId::new("a"), "x"),
            ProviderId::new("b"),
            vec![
                cand("x1", Confidence::Strong),
                cand("x2", Confidence::Strong),
            ],
        );
        assert_eq!(m.confidence, Confidence::Ambiguous);
        assert!(m.selected.is_none());
        assert_eq!(m.candidates.len(), 2);
    }

    #[test]
    fn a_clear_winner_is_selected() {
        let m = PackageMapping::from_candidates(
            PackageIdentity::new(ProviderId::new("a"), "x"),
            ProviderId::new("b"),
            vec![
                cand("x2", Confidence::Probable),
                cand("x1", Confidence::Exact),
            ],
        );
        assert_eq!(m.confidence, Confidence::Exact);
        assert_eq!(m.selected.unwrap().package.name, "x1");
    }

    #[test]
    fn no_candidates_is_none() {
        let m = PackageMapping::from_candidates(
            PackageIdentity::new(ProviderId::new("a"), "x"),
            ProviderId::new("b"),
            vec![],
        );
        assert_eq!(m.confidence, Confidence::None);
        assert!(!Confidence::Probable.is_strong());
        assert!(Confidence::Exact.is_strong());
    }
}
