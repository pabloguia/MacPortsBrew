// SPDX-License-Identifier: Apache-2.0

//! `analyze`: a pure function of (snapshot, policy data, options). The
//! live path captures a snapshot first and calls this; `--from-snapshot`
//! loads one and calls this. Same function, same bytes (INV-15).

use crate::classify::classify;
use crate::facts::{package_facts, provider_facts, PackageInputs};
use crate::mapping::map_package;
use crate::report::{PackageReport, ProviderReport, ProviderRow, RowPresence, Summary};
use crate::{ANALYSIS_SCHEMA_VERSION, TOOL_VERSION};
use macportsbrew_domain::{
    classify_ownership_for, ExecutableOwner, Machine, Ownership, PackageIdentity, PackageKind,
    ProviderId, ProviderPresence, ProviderSnapshot, Resolution, Snapshot,
};
use macportsbrew_policy::{recommend, PolicyData};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Options that change what the analysis includes, never what it decides.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnalysisOptions {
    /// The current date, `YYYY-MM-DD`, passed in (INV-14).
    pub today: String,
    /// Include dependency-level units (INV-13: hidden by default).
    pub advanced: bool,
    /// Restrict to these package names (any provider's spelling).
    pub only: BTreeSet<String>,
}

/// Where the snapshot came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnalysisSource {
    /// Captured from this machine in the same run.
    Live,
    /// Loaded from a snapshot file.
    Snapshot,
}

/// The analysis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    /// Output schema version.
    pub schema_version: u32,
    /// Tool version.
    pub tool_version: String,
    /// Policy version.
    pub policy_version: String,
    /// Newest read date in the policy data.
    pub policy_read_on: String,
    /// Whether the policy data is older than its staleness threshold on
    /// `today`.
    pub policy_stale: bool,
    /// The date the analysis was computed for.
    pub today: String,
    /// Live or snapshot.
    pub source: AnalysisSource,
    /// The snapshot's provenance note, when loaded from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_provenance: Option<macportsbrew_domain::Provenance>,
    /// The machine.
    pub machine: Machine,
    /// Providers.
    pub providers: BTreeMap<ProviderId, ProviderReport>,
    /// Package units, sorted by name then source provider.
    pub packages: Vec<PackageReport>,
    /// Counts.
    pub summary: Summary,
}

/// Runs the analysis.
pub fn analyze(
    snapshot: &Snapshot,
    policy: &PolicyData,
    options: &AnalysisOptions,
    source: AnalysisSource,
) -> Analysis {
    let providers = provider_reports(snapshot, policy);
    let mut packages = Vec::new();
    let mut seen: BTreeSet<PackageIdentity> = BTreeSet::new();
    let installed_names: BTreeMap<&ProviderId, BTreeSet<String>> = snapshot
        .providers
        .iter()
        .map(|(id, p)| (id, p.inventory.installed_names()))
        .collect();

    for (id, p) in &snapshot.providers {
        if !matches!(p.detection.presence, ProviderPresence::Present) {
            continue;
        }
        for name in p.inventory.installed_names() {
            let identity = PackageIdentity::new(id.clone(), name.clone());
            if seen.contains(&identity) {
                continue;
            }
            if !options.only.is_empty() && !options.only.contains(&name) {
                continue;
            }
            let (ownership, evidence) = ownership_of(p, &name);
            let advanced_only = !ownership.is_top_level();
            if advanced_only && !options.advanced {
                continue;
            }
            let kind = p
                .inventory
                .installed
                .iter()
                .find(|r| r.identity.name == name)
                .map_or(PackageKind::Package, |r| r.kind);
            let meta = p.inventory.metadata.get(&name);
            let classes = classify(id, &name, kind, meta, policy);

            // Mappings toward every other provider, present or not: a
            // curated entry maps even to an absent provider.
            let mut mappings = Vec::new();
            for (dest, dp) in &snapshot.providers {
                if dest == id {
                    continue;
                }
                let dest_inv = matches!(dp.detection.presence, ProviderPresence::Present)
                    .then_some(&dp.inventory);
                mappings.push(map_package(&identity, meta, dest, dest_inv, policy));
            }

            let resolution = resolution_for(snapshot, id, &name);
            let source_id = source_for(id, &mappings, &installed_names, resolution.as_ref());
            // If another provider's installed counterpart drives the unit
            // (its executable resolves), that provider's row is the source.
            if &source_id != id {
                // Mark this identity as seen; the unit will be built when
                // the source provider's loop reaches it, unless it already
                // did - in which case merge by skipping.
                seen.insert(identity.clone());
                if let Some(existing) = packages.iter_mut().find(|u: &&mut PackageReport| {
                    u.source == source_id
                        && u.rows.get(id).is_some_and(|r: &ProviderRow| r.name == name)
                }) {
                    existing
                        .rows
                        .entry(id.clone())
                        .or_insert_with(|| installed_row(p, &name, ownership, evidence.clone()));
                    continue;
                }
            }

            let installed_version = p
                .inventory
                .installed
                .iter()
                .filter(|r| r.identity.name == name && r.active)
                .map(|r| r.version.clone())
                .next()
                .or_else(|| {
                    p.inventory
                        .installed
                        .iter()
                        .find(|r| r.identity.name == name)
                        .map(|r| r.version.clone())
                });
            // Reverse dependencies are counted within each provider that
            // holds the package and the largest count is what the engine
            // sees: removing the package from either side is what the
            // count guards (INV-04), and choosing one side by name would
            // make the answer depend on provider ids (INV-17).
            let mut reverse = p.inventory.reverse_dependencies_of(&name).len();
            for m in &mappings {
                if let Some(sel) = &m.selected {
                    let dp = &snapshot.providers[&m.destination];
                    if installed_names[&m.destination].contains(&sel.package.name) {
                        reverse = reverse.max(
                            dp.inventory
                                .reverse_dependencies_of(&sel.package.name)
                                .len(),
                        );
                    }
                }
            }
            let reverse = u32::try_from(reverse).unwrap_or(u32::MAX);
            let facts = package_facts(
                snapshot,
                policy,
                &options.today,
                &PackageInputs {
                    name: &name,
                    source: id,
                    ownership,
                    classes: &classes,
                    installed_version,
                    reverse_dependency_count: reverse,
                    mappings: &mappings,
                },
            );
            let recommendation = recommend(&facts, policy);

            let mut rows = BTreeMap::new();
            rows.insert(id.clone(), installed_row(p, &name, ownership, evidence));
            for m in &mappings {
                if let Some(sel) = &m.selected {
                    let dest = &m.destination;
                    let dp = &snapshot.providers[dest];
                    let dest_name = &sel.package.name;
                    if installed_names[dest].contains(dest_name) {
                        let (o, e) = ownership_of(dp, dest_name);
                        seen.insert(PackageIdentity::new(dest.clone(), dest_name.clone()));
                        rows.insert(dest.clone(), installed_row(dp, dest_name, o, e));
                    } else {
                        rows.insert(dest.clone(), mapped_row(dp, dest_name));
                    }
                }
            }
            seen.insert(identity);
            packages.push(PackageReport {
                name: name.clone(),
                source: id.clone(),
                rows,
                class: classes,
                resolution,
                mappings,
                recommendation,
                advanced_only,
            });
        }
    }

    packages.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.source.cmp(&b.source)));
    let summary = summarise(&packages);
    Analysis {
        schema_version: ANALYSIS_SCHEMA_VERSION,
        tool_version: TOOL_VERSION.to_string(),
        policy_version: policy.version().to_string(),
        policy_read_on: policy.newest_read_on().to_string(),
        policy_stale: policy.is_stale(&options.today),
        today: options.today.clone(),
        source,
        snapshot_provenance: (source == AnalysisSource::Snapshot)
            .then(|| snapshot.provenance.clone()),
        machine: snapshot.machine.clone(),
        providers,
        packages,
        summary,
    }
}

/// Per-provider reports.
pub fn provider_reports(
    snapshot: &Snapshot,
    policy: &PolicyData,
) -> BTreeMap<ProviderId, ProviderReport> {
    let facts = provider_facts(snapshot, policy);
    snapshot
        .providers
        .iter()
        .map(|(id, p)| {
            let names = p.inventory.installed_names();
            let top = names
                .iter()
                .filter(|n| ownership_of(p, n).0.is_top_level())
                .count();
            let gui = p
                .inventory
                .installed
                .iter()
                .filter(|r| r.kind == PackageKind::GuiApp)
                .map(|r| &r.identity.name)
                .collect::<BTreeSet<_>>()
                .len();
            (
                id.clone(),
                ProviderReport {
                    detection: p.detection.clone(),
                    capabilities: p.capabilities.clone(),
                    support: facts[id].support.clone(),
                    package_count: u32::try_from(names.len()).unwrap_or(u32::MAX),
                    top_level_count: u32::try_from(top).unwrap_or(u32::MAX),
                    gui_app_count: u32::try_from(gui).unwrap_or(u32::MAX),
                },
            )
        })
        .collect()
}

fn ownership_of(
    p: &ProviderSnapshot,
    name: &str,
) -> (Ownership, macportsbrew_domain::OwnershipEvidence) {
    let requested = p
        .inventory
        .installed
        .iter()
        .filter(|r| r.identity.name == name)
        .find_map(|r| r.requested);
    let has_dependents = !p.inventory.reverse_dependencies_of(name).is_empty();
    classify_ownership_for(requested, has_dependents, &p.inventory.requested.mechanism)
}

fn installed_row(
    p: &ProviderSnapshot,
    name: &str,
    ownership: Ownership,
    evidence: macportsbrew_domain::OwnershipEvidence,
) -> ProviderRow {
    let meta = p.inventory.metadata.get(name);
    ProviderRow {
        name: name.to_string(),
        installed: p
            .inventory
            .installed
            .iter()
            .filter(|r| r.identity.name == name)
            .cloned()
            .collect(),
        ownership,
        ownership_evidence: evidence,
        available_version: meta.and_then(|m| m.available_version.clone()),
        availability: p
            .inventory
            .availability
            .get(name)
            .cloned()
            .unwrap_or_else(|| {
                macportsbrew_domain::AvailabilityAssessment::unknown(
                    "inventory",
                    "no assessment recorded",
                )
            }),
        deprecated: meta.is_some_and(|m| m.deprecated.is_some()),
        disabled: meta.is_some_and(|m| m.disabled.is_some()),
        dependency_count: u32::try_from(p.inventory.dependencies_of(name).len())
            .unwrap_or(u32::MAX),
        reverse_dependency_count: u32::try_from(p.inventory.reverse_dependencies_of(name).len())
            .unwrap_or(u32::MAX),
        presence: RowPresence::Installed,
    }
}

fn mapped_row(p: &ProviderSnapshot, name: &str) -> ProviderRow {
    let meta = p.inventory.metadata.get(name);
    ProviderRow {
        name: name.to_string(),
        installed: Vec::new(),
        ownership: Ownership::Unknown,
        ownership_evidence: macportsbrew_domain::OwnershipEvidence {
            mechanism: "not_installed".into(),
            authoritative: true,
        },
        available_version: meta.and_then(|m| m.available_version.clone()),
        availability: p
            .inventory
            .availability
            .get(name)
            .cloned()
            .unwrap_or_else(|| {
                macportsbrew_domain::AvailabilityAssessment::unknown(
                    "destination_catalogue",
                    "the destination's catalogue is not consulted offline in this pass",
                )
            }),
        deprecated: meta.is_some_and(|m| m.deprecated.is_some()),
        disabled: meta.is_some_and(|m| m.disabled.is_some()),
        dependency_count: 0,
        reverse_dependency_count: 0,
        presence: RowPresence::Mapped,
    }
}

fn resolution_for(snapshot: &Snapshot, provider: &ProviderId, name: &str) -> Option<Resolution> {
    // The provider that captured the resolution keyed it by executable name;
    // a package's primary executable is assumed to share its name unless
    // metadata says otherwise. Absence is not an error: many packages are
    // libraries.
    let p = &snapshot.providers[provider];
    let candidates: Vec<&str> = p
        .inventory
        .metadata
        .get(name)
        .filter(|m| !m.executables.is_empty())
        .map_or_else(
            || vec![name],
            |m| m.executables.iter().map(String::as_str).collect(),
        );
    for exe in candidates {
        for ps in snapshot.providers.values() {
            if let Some(r) = ps.resolutions.get(exe) {
                return Some(r.clone());
            }
        }
    }
    None
}

/// Which provider's row drives the unit: the one whose package the resolved
/// executable belongs to, when the counterpart is installed there too;
/// otherwise the provider being iterated.
fn source_for(
    current: &ProviderId,
    mappings: &[macportsbrew_domain::PackageMapping],
    installed: &BTreeMap<&ProviderId, BTreeSet<String>>,
    resolution: Option<&Resolution>,
) -> ProviderId {
    if let Some(Resolution {
        owner: ExecutableOwner::Provider { provider, .. },
        ..
    }) = resolution
    {
        if provider != current {
            let counterpart_installed = mappings.iter().any(|m| {
                &m.destination == provider
                    && m.selected.as_ref().is_some_and(|c| {
                        installed
                            .get(provider)
                            .is_some_and(|set| set.contains(&c.package.name))
                    })
            });
            if counterpart_installed {
                return provider.clone();
            }
        }
    }
    current.clone()
}

fn summarise(packages: &[PackageReport]) -> Summary {
    let mut s = Summary::default();
    for p in packages {
        if p.advanced_only {
            s.dependency_units += 1;
        } else {
            s.top_level_units += 1;
        }
        let kind = serde_json::to_value(&p.recommendation.outcome)
            .ok()
            .and_then(|v| v.get("kind").and_then(|k| k.as_str().map(String::from)))
            .unwrap_or_default();
        if kind == "MANUAL_REVIEW" {
            s.manual_review += 1;
        }
        *s.by_outcome.entry(kind).or_insert(0) += 1;
    }
    s
}
