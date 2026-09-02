// SPDX-License-Identifier: Apache-2.0

//! Assembling the policy engine's input from a snapshot. The engine reads
//! nothing else (`03-policy-and-recommendations.md`, "Facts the engine
//! consumes"); everything it needs is put here, once.

use macportsbrew_domain::{
    AvailabilityAssessment, ClassSet, Confidence, Ownership, PackageMapping, ProviderId,
    ProviderPresence, Snapshot, Version,
};
use macportsbrew_policy::{Facts, PackageFacts, PackageProviderFacts, PolicyData, ProviderFacts};
use std::collections::BTreeMap;

/// Provider facts for every provider in the snapshot.
pub fn provider_facts(
    snapshot: &Snapshot,
    policy: &PolicyData,
) -> BTreeMap<ProviderId, ProviderFacts> {
    snapshot
        .providers
        .iter()
        .map(|(id, p)| {
            let healthy = matches!(p.detection.presence, ProviderPresence::Present);
            (
                id.clone(),
                ProviderFacts {
                    presence: p.detection.presence.clone(),
                    version: p.detection.version.clone(),
                    prefix: p.detection.prefix.clone(),
                    support: policy.support_state(id, &snapshot.machine),
                    healthy,
                },
            )
        })
        .collect()
}

/// The inputs to one package's facts.
pub struct PackageInputs<'a> {
    /// Name under the source provider.
    pub name: &'a str,
    /// Source provider.
    pub source: &'a ProviderId,
    /// Ownership under the source.
    pub ownership: Ownership,
    /// Classes.
    pub classes: &'a ClassSet,
    /// Installed version under the source.
    pub installed_version: Option<Version>,
    /// Reverse dependencies within the source provider.
    pub reverse_dependency_count: u32,
    /// Mappings toward every other provider.
    pub mappings: &'a [PackageMapping],
}

/// Builds the engine input for one package.
pub fn package_facts(
    snapshot: &Snapshot,
    policy: &PolicyData,
    today: &str,
    inputs: &PackageInputs<'_>,
) -> Facts {
    let mut per_provider = BTreeMap::new();
    if let Some(p) = snapshot.providers.get(inputs.source) {
        let meta = p.inventory.metadata.get(inputs.name);
        per_provider.insert(
            inputs.source.clone(),
            PackageProviderFacts {
                installed: true,
                available_version: meta.and_then(|m| m.available_version.clone()),
                availability: p
                    .inventory
                    .availability
                    .get(inputs.name)
                    .cloned()
                    .unwrap_or_else(|| {
                        AvailabilityAssessment::unknown("inventory", "no assessment recorded")
                    }),
                deprecated: meta.is_some_and(|m| m.deprecated.is_some()),
                disabled: meta.is_some_and(|m| m.disabled.is_some()),
                replaced_by: meta.and_then(|m| m.replaced_by.clone()),
                mapping_confidence: Confidence::Exact,
                mapped_name: Some(inputs.name.to_string()),
            },
        );
    }
    for mapping in inputs.mappings {
        let dest = &mapping.destination;
        let Some(p) = snapshot.providers.get(dest) else {
            continue;
        };
        let mapped = mapping.selected.as_ref().map(|c| c.package.name.clone());
        let meta = mapped.as_ref().and_then(|n| p.inventory.metadata.get(n));
        let installed = mapped
            .as_ref()
            .is_some_and(|n| p.inventory.installed_names().contains(n));
        let availability = mapped
            .as_ref()
            .and_then(|n| p.inventory.availability.get(n).cloned())
            .unwrap_or_else(|| {
                if mapped.is_some() {
                    AvailabilityAssessment::unknown(
                        "destination_catalogue",
                        "the destination's catalogue is not consulted offline in this pass",
                    )
                } else {
                    AvailabilityAssessment::unknown("mapping", "no equivalent identified")
                }
            });
        per_provider.insert(
            dest.clone(),
            PackageProviderFacts {
                installed,
                available_version: meta.and_then(|m| m.available_version.clone()),
                availability,
                deprecated: meta.is_some_and(|m| m.deprecated.is_some()),
                disabled: meta.is_some_and(|m| m.disabled.is_some()),
                replaced_by: meta.and_then(|m| m.replaced_by.clone()),
                mapping_confidence: mapping.confidence,
                mapped_name: mapped,
            },
        );
    }
    Facts {
        machine: snapshot.machine.clone(),
        today: today.to_string(),
        policy_version: policy.version().to_string(),
        providers: provider_facts(snapshot, policy),
        package: PackageFacts {
            name: inputs.name.to_string(),
            source: inputs.source.clone(),
            ownership: inputs.ownership,
            classes: inputs.classes.clone(),
            installed_version: inputs.installed_version.clone(),
            per_provider,
            reverse_dependency_count: inputs.reverse_dependency_count,
            coexistence_requested: false,
        },
    }
}
