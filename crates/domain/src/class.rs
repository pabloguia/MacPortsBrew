// SPDX-License-Identifier: Apache-2.0

//! Package classes: what migrating a package *means*
//! (`02-domain-model.md`, "Package classes").
//!
//! A package carries a *set* of classes, not one: `php@8.4` is a runtime
//! that also defines a service, `postgresql@17` is stateful and a service
//! (R-8). The handling precedence is fixed here so that a class demanding
//! manual review (INV-19) can never be hidden by a more benign one.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PackageClass {
    /// Self-contained tool or library.
    Simple,
    /// Owns persistent data outside its prefix.
    Stateful,
    /// Runs under provider-managed launch machinery.
    Service,
    /// Language runtime or toolchain with its own ecosystem.
    Runtime,
    /// A GUI application (cask). Out of migration scope.
    GuiApp,
    /// From a non-core source: third-party tap, local port file.
    ThirdPartySourced,
    /// Part of the provider itself.
    ProviderInternal,
}

impl PackageClass {
    /// Classes whose presence forces `MANUAL_REVIEW` (INV-19; `03-policy`).
    pub fn forces_manual_review(self) -> bool {
        matches!(
            self,
            PackageClass::Stateful
                | PackageClass::Service
                | PackageClass::GuiApp
                | PackageClass::ThirdPartySourced
        )
    }
}

/// The class set of one package plus the evidence for each member.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassSet {
    /// Members, sorted. Empty means `SIMPLE`.
    pub classes: BTreeSet<PackageClass>,
    /// Why each member is there, sorted by class.
    pub evidence: Vec<ClassEvidence>,
}

/// Why a class was assigned.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassEvidence {
    /// The class this evidence supports.
    pub class: PackageClass,
    /// Stable identifier of the detection rule or curated entry.
    pub rule: String,
    /// What was observed, as text (a field name, a path, a curated id).
    pub detail: String,
}

impl ClassSet {
    /// An empty set, meaning `SIMPLE`.
    pub fn simple() -> Self {
        Self::default()
    }

    /// Adds a class with its evidence.
    pub fn add(&mut self, class: PackageClass, rule: impl Into<String>, detail: impl Into<String>) {
        self.classes.insert(class);
        self.evidence.push(ClassEvidence {
            class,
            rule: rule.into(),
            detail: detail.into(),
        });
        self.evidence.sort();
    }

    /// True when the set contains the class.
    pub fn has(&self, class: PackageClass) -> bool {
        self.classes.contains(&class)
    }

    /// True when the set is empty: a plain, self-contained package.
    pub fn is_simple(&self) -> bool {
        self.classes.is_empty()
    }

    /// The class that decides default handling, by precedence: anything
    /// forcing manual review first, then provider-internal, then runtime,
    /// then simple.
    pub fn dominant(&self) -> PackageClass {
        const PRECEDENCE: [PackageClass; 6] = [
            PackageClass::ProviderInternal,
            PackageClass::Stateful,
            PackageClass::Service,
            PackageClass::GuiApp,
            PackageClass::ThirdPartySourced,
            PackageClass::Runtime,
        ];
        PRECEDENCE
            .iter()
            .copied()
            .find(|c| self.classes.contains(c))
            .unwrap_or(PackageClass::Simple)
    }

    /// True when any member forces manual review.
    pub fn forces_manual_review(&self) -> bool {
        self.classes.iter().any(|c| c.forces_manual_review())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_review_classes_dominate_runtime() {
        let mut set = ClassSet::simple();
        set.add(PackageClass::Runtime, "versioned_formula", "php@8.4");
        assert_eq!(set.dominant(), PackageClass::Runtime);
        set.add(PackageClass::Service, "service_block", "service != null");
        assert_eq!(set.dominant(), PackageClass::Service);
        assert!(set.forces_manual_review());
    }

    #[test]
    fn empty_set_is_simple() {
        let set = ClassSet::simple();
        assert!(set.is_simple());
        assert_eq!(set.dominant(), PackageClass::Simple);
        assert!(!set.forces_manual_review());
    }
}
