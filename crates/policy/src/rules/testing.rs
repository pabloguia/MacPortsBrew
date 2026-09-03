// SPDX-License-Identifier: Apache-2.0

//! Fixtures for the tests of this crate and of the layers above it.
//!
//! These are pure constructors: no I/O, no clock, no provider names except
//! the two deliberately neutral ids `alpha` and `beta`. Using neutral ids in
//! the fixtures is not decoration — it is how the symmetry checks (INV-17)
//! stay honest, because a fixture that named the real providers would let a
//! rule quietly acquire a preference and still pass.

use crate::facts::{Facts, PackageFacts, PackageProviderFacts, ProviderFacts};
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, ClassSet,
    CommandLineTools, Confidence, DatedFact, Machine, MacosVersion, Outcome, Ownership, ProviderId,
    ProviderPresence, SupportState, SupportTier, Translation, Version,
};
use std::collections::BTreeMap;

/// The date every fixture is assembled against.
pub const TODAY: &str = "2026-09-02";

/// The id of the side a fixture package starts on.
pub fn alpha() -> ProviderId {
    ProviderId::new("alpha")
}

/// The id of the other side.
pub fn beta() -> ProviderId {
    ProviderId::new("beta")
}

/// A machine.
pub fn machine(architecture: Architecture, macos: &str) -> Machine {
    Machine {
        architecture,
        macos: MacosVersion::parse(macos).expect("fixture macOS version"),
        translation: Translation::Native,
        command_line_tools: CommandLineTools::Unknown,
    }
}

/// A support state with the given tier and no stated end.
pub fn support(tier: SupportTier, prebuilt: bool) -> SupportState {
    SupportState {
        tier,
        provider_label: None,
        end_date: None,
        prebuilt_artifacts: Some(prebuilt),
        fact: DatedFact {
            source: "fixture".into(),
            read_on: TODAY.into(),
        },
        entry: "fixture".into(),
    }
}

/// A support state with a stated end date.
pub fn support_ending(tier: SupportTier, end_date: &str, prebuilt: bool) -> SupportState {
    let mut state = support(tier, prebuilt);
    state.end_date = Some(end_date.to_string());
    state
}

/// A healthy, present provider with the given support state.
pub fn provider(support: SupportState) -> ProviderFacts {
    ProviderFacts {
        presence: ProviderPresence::Present,
        version: Some(Version::new("1.0")),
        prefix: None,
        support,
        healthy: true,
    }
}

/// An availability assessment carrying one piece of evidence.
pub fn availability(state: ArtifactAvailability) -> AvailabilityAssessment {
    AvailabilityAssessment {
        availability: state,
        evidence: vec![AvailabilityEvidence {
            source: "fixture".into(),
            detail: "fixture".into(),
            offline: true,
        }],
        cached: false,
    }
}

/// The package as one provider sees it.
pub fn package_at(
    present: bool,
    state: ArtifactAvailability,
    confidence: Confidence,
) -> PackageProviderFacts {
    PackageProviderFacts {
        installed: present,
        available_version: Some(Version::new("1.0")),
        availability: availability(state),
        reverse_dependency_count: 0,
        deprecated: false,
        disabled: false,
        replaced_by: None,
        mapping_confidence: confidence,
        mapped_name: Some("sample".to_string()),
    }
}

/// The benign case: two healthy, fully supported providers, a plain package
/// held by `alpha`, a strong equivalent available under `beta`, prebuilt
/// artifacts on both sides and nothing depending on it. Every rule test
/// starts here and changes exactly the fact it is about.
pub fn baseline() -> Facts {
    let mut providers = BTreeMap::new();
    providers.insert(alpha(), provider(support(SupportTier::Supported, true)));
    providers.insert(beta(), provider(support(SupportTier::Supported, true)));

    let mut per_provider = BTreeMap::new();
    per_provider.insert(
        alpha(),
        package_at(
            true,
            ArtifactAvailability::BinaryAvailable,
            Confidence::Exact,
        ),
    );
    per_provider.insert(
        beta(),
        package_at(
            false,
            ArtifactAvailability::BinaryAvailable,
            Confidence::Strong,
        ),
    );

    Facts {
        machine: machine(Architecture::X86_64, "26.6.2"),
        today: TODAY.to_string(),
        policy_version: "fixture".to_string(),
        providers,
        package: PackageFacts {
            name: "sample".to_string(),
            source: alpha(),
            ownership: Ownership::TopLevel,
            classes: ClassSet::simple(),
            installed_version: Some(Version::new("1.0")),
            per_provider,
            coexistence_requested: false,
        },
    }
}

/// Renames `a` to `b` and `b` to `a` everywhere in the facts.
///
/// The engine's answer must survive this untouched except for the names
/// themselves. That is INV-17 stated as a property rather than as prose: if
/// any rule held a preference for one side, this would surface it.
pub fn swap_providers(facts: &Facts, a: &ProviderId, b: &ProviderId) -> Facts {
    let rename = |id: &ProviderId| -> ProviderId {
        if id == a {
            b.clone()
        } else if id == b {
            a.clone()
        } else {
            id.clone()
        }
    };

    let mut swapped = facts.clone();
    swapped.providers = facts
        .providers
        .iter()
        .map(|(id, p)| (rename(id), p.clone()))
        .collect();
    swapped.package.source = rename(&facts.package.source);
    swapped.package.per_provider = facts
        .package
        .per_provider
        .iter()
        .map(|(id, p)| (rename(id), p.clone()))
        .collect();
    swapped
}

/// Applies the same renaming to an outcome, so the two can be compared.
pub fn swap_outcome(outcome: &Outcome, a: &ProviderId, b: &ProviderId) -> Outcome {
    let rename = |id: &ProviderId| -> ProviderId {
        if id == a {
            b.clone()
        } else if id == b {
            a.clone()
        } else {
            id.clone()
        }
    };
    match outcome {
        Outcome::Keep { provider } => Outcome::Keep {
            provider: rename(provider),
        },
        Outcome::MigrateTo { provider } => Outcome::MigrateTo {
            provider: rename(provider),
        },
        Outcome::KeepBoth { primary } => Outcome::KeepBoth {
            primary: rename(primary),
        },
        other => other.clone(),
    }
}

/// Sets how many packages depend on this one **inside the source provider**.
///
/// The count is per-provider (DP-010, SE-045), so a test has to say which side
/// it means; every existing caller means the source.
pub fn with_reverse_dependencies(facts: &mut Facts, count: u32) {
    let source = facts.package.source.clone();
    if let Some(per) = facts.package.per_provider.get_mut(&source) {
        per.reverse_dependency_count = count;
    }
}
