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
    classify_ownership_for, ConcurrencyFlag, ConcurrentProviderActivity, ExecutableName,
    ExecutableOwner, Machine, Ownership, PackageIdentity, PackageKind, ProviderId,
    ProviderPresence, ProviderSnapshot, Resolution, ResolutionState, Snapshot,
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
    /// Loaded from a snapshot file, or captured from a replayed recording
    /// (`Context::build` gives `--replay` this source too).
    Snapshot,
}

impl AnalysisSource {
    /// What this build can say about a provider running while the input was
    /// read — and it never says more than it checked (`plan.v1.md`,
    /// obligation 5; SE-015).
    ///
    /// A live run says `NotChecked`: this build has no detection mechanism,
    /// so nobody looked. A snapshot file or a replayed recording says
    /// `Unknowable`: neither carries this host's processes, and a replay must
    /// not consult them (INV-15), so no check *could* have answered. Neither
    /// ever says `NoneObserved` or `Observed`; those belong to a check that
    /// ran, which lands with the lock it serves.
    #[must_use]
    pub fn concurrent_provider_activity(self) -> ConcurrentProviderActivity {
        match self {
            Self::Live => ConcurrentProviderActivity::NotChecked,
            Self::Snapshot => ConcurrentProviderActivity::Unknowable,
        }
    }
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
    /// Whether a provider may have been changing the machine while the input
    /// was read, decided by [`AnalysisSource::concurrent_provider_activity`]
    /// from `source` and never claiming a check that was not made. Written in
    /// the one shape `plan` and `migrate` also use (`analysis.v1.md`; SE-015).
    /// Required when deserialising: `Analysis` is not a reader of every v1
    /// document, and a consumer of an earlier build's document — which lacks
    /// the field — reads its absence as possibly inconsistent, as the schema
    /// says.
    pub concurrent_provider_activity: ConcurrencyFlag,
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
                // Another provider's installed copy drives this unit, because
                // its executable is the one that resolves (SE-042). Let that
                // provider build it and pull this row in through its mappings -
                // but only once we know it actually will.
                //
                // Mappings are scored per direction and are **not** symmetric.
                // `homebrew/postgresql@17` maps to two MacPorts candidates and
                // therefore selects neither (`AMBIGUOUS`), while
                // `macports/postgresql17-server` maps to one and selects it. An
                // unconditional deferral in that shape dropped the row on the
                // floor: nobody built it, and an installed, user-requested port
                // was reported nowhere, in either mode, with `plan` stating it
                // was not installed under any provider. Completeness is not
                // negotiable against tidiness, so the deferral asks first.
                let claimed_back = mappings
                    .iter()
                    .find(|m| m.destination == source_id)
                    .and_then(|m| m.selected.as_ref())
                    .is_some_and(|c| {
                        let sp = &snapshot.providers[&source_id];
                        let sname = &c.package.name;
                        map_package(
                            &PackageIdentity::new(source_id.clone(), sname.clone()),
                            sp.inventory.metadata.get(sname),
                            id,
                            Some(&p.inventory),
                            policy,
                        )
                        .selected
                        .is_some_and(|back| back.package.name == name)
                    });
                if claimed_back {
                    seen.insert(identity.clone());
                    if let Some(existing) = packages.iter_mut().find(|u: &&mut PackageReport| {
                        u.source == source_id
                            && u.rows.get(id).is_some_and(|r: &ProviderRow| r.name == name)
                    }) {
                        existing.rows.entry(id.clone()).or_insert_with(|| {
                            installed_row(p, &name, ownership, evidence.clone())
                        });
                    }
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
            // Reverse dependencies are no longer collapsed to one number here.
            // `package_facts` derives a count per provider, so the engine reads
            // the side it is reasoning about and no side has to be chosen at
            // fact-construction time — which is what let a maximum across
            // providers stand in for the source's own count (DP-010, SE-045).
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
            // Top-level under *any* provider that holds it means the unit is
            // top-level. Deciding this from the first row reached made it a
            // property of iteration order, and because the filter then ran
            // before the merge, `--advanced` could *remove* a user-requested
            // package from the top-level view and flip the unit's source with
            // it (SE-048). A row that is only a mapping candidate is not
            // installed anywhere and carries no intent, so it does not vote.
            let advanced_only = !rows.iter().any(|(provider, r)| {
                // Installed, top-level by that provider's own record, and
                // either the unit's own side or attached by a mapping strong
                // enough to say the two are the same software.
                //
                // The last clause is the point. Without it a guess decided
                // visibility: `python@3.14` is a Homebrew *dependency*, and it
                // was promoted into the default view by a MacPorts row attached
                // at `PROBABLE` - the same class of guess SE-047 refuses to let
                // waive the planner's mapping threshold. A row that is only
                // probably the same package should not be able to vote a
                // dependency into the view INV-13 reserves for what the user
                // asked for.
                !r.installed.is_empty()
                    && r.ownership.shown_in_normal_mode()
                    && (provider == id
                        || mappings
                            .iter()
                            .find(|m| &m.destination == provider)
                            .is_some_and(|m| m.confidence.is_strong()))
            });
            let ownership_split_defaulted = split_defaulted(snapshot, &rows);
            packages.push(PackageReport {
                name: name.clone(),
                source: id.clone(),
                rows,
                class: classes,
                resolution,
                mappings,
                recommendation,
                advanced_only,
                ownership_split_defaulted,
            });
        }
    }

    // Visibility is applied to finished units, not to rows on the way in, so a
    // unit's identity is the same in both modes and `--advanced` only ever adds.
    if !options.advanced {
        packages.retain(|u| !u.advanced_only);
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
        concurrent_provider_activity: ConcurrencyFlag::of(source.concurrent_provider_activity()),
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
    // The intent recorded on the active installation, when there is one: a
    // provider can hold several versions, each with its own record, and the
    // first row in sort order is not the one the user is running.
    let rows: Vec<_> = p
        .inventory
        .installed
        .iter()
        .filter(|r| r.identity.name == name)
        .collect();
    let requested = rows
        .iter()
        .find(|r| r.active)
        .and_then(|r| r.requested)
        .or_else(|| rows.iter().find_map(|r| r.requested));
    // "Nothing depends on it" is concluded only from a reverse-dependency
    // answer the provider could give. Without one, a package nobody asked for
    // is a dependency of something unknown, never an orphan: `ORPHAN` is the
    // label that invites removal. A snapshot that does not mention the
    // capability at all has not answered either.
    let dependents_known = dependents_known(p);
    let has_dependents = !dependents_known || !p.inventory.reverse_dependencies_of(name).is_empty();
    classify_ownership_for(requested, has_dependents, &p.inventory.requested.mechanism)
}

/// Whether the provider answered reverse dependencies, the one input that
/// splits `DEPENDENCY` from `ORPHAN`. A snapshot that does not mention the
/// capability has not answered.
fn dependents_known(p: &ProviderSnapshot) -> bool {
    p.capabilities
        .has(macportsbrew_domain::Capability::ReverseDependencies)
}

/// The providers, among a unit's installed rows, whose `DEPENDENCY` was reached
/// by default because reverse dependencies could not be read
/// ([`PackageReport::ownership_split_defaulted`]).
fn split_defaulted(
    snapshot: &Snapshot,
    rows: &BTreeMap<ProviderId, ProviderRow>,
) -> Vec<ProviderId> {
    rows.iter()
        .filter(|(provider, row)| {
            row.presence == crate::report::RowPresence::Installed
                && row.ownership == Ownership::Dependency
                && snapshot
                    .providers
                    .get(*provider)
                    .is_some_and(|p| !dependents_known(p))
        })
        .map(|(provider, _)| provider.clone())
        .collect()
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
    // The list, never the older set beside it, which is not read to decide
    // (F.md §1). A loaded document's set equals its list, so on every
    // document this build has written before F the two choices are one.
    let candidates: Vec<&str> = p
        .inventory
        .metadata
        .get(name)
        .and_then(|m| m.executable_list.known())
        .filter(|names| !names.is_empty())
        .map_or_else(
            || vec![name],
            |names| names.iter().map(ExecutableName::as_str).collect(),
        );
    // Ask every provider, and rank the answers by how much each one settles.
    //
    // Each provider records what *it* found for a name, so two can answer
    // differently about the same executable — and returning whichever came
    // first in map order made a cross-provider outcome depend on provider
    // *name*, which is the shape INV-17 exists to keep out of this layer
    // (F-20).
    //
    // Preferring `Provider` alone was not enough, and a review showed it by
    // running the mirror rather than by arguing: MacPorts answers `System` for
    // a `/usr/bin` path where Homebrew answers `Unknown`, and with only
    // `Provider` promoted those two swap answers when the providers swap roles.
    // The worse pair was `Provider` against `Provider`, where the
    // *non-authoritative* one — a prefix heuristic, marked as one exactly so it
    // can be weighed — beat an authoritative one on alphabetical order, and
    // `source_for` then drove the whole unit from it.
    //
    // So the rank is what each answer establishes, and `authoritative` is read
    // rather than merely recorded.
    //
    // `capture::owner_of` implements the same preference over the registry's
    // order rather than this map's. Two implementations of one rule is a place
    // for them to diverge; they are cross-referenced until one of them wins. Within a rank the two providers agree, or
    // they contradict each other about a canonical path, which no ordering here
    // can resolve.
    // Ranked within each candidate name and not across the list: the first
    // name any provider answers about at all wins, even with `Unknown`, before
    // a later name that would have been settled. The candidate order is the
    // package's own metadata, so no provider name enters it - which is why this
    // is a note and not the defect above.
    //
    // The ranking itself is `Resolution::rank`, and the per-provider reading
    // is `Snapshot::resolution_of`'s rule for a document without the PATH map
    // (F-2b, SG2-Q3): the one accessor every reader goes through. A document
    // with the map files each name under one provider, so the map's answer is
    // the one this ranking picked, and the output is byte-identical.
    for exe in candidates {
        let Ok(exe) = ExecutableName::parse(exe) else {
            // No capture ever resolved a name outside the class.
            continue;
        };
        match snapshot.resolution_of(&exe) {
            ResolutionState::Found { resolution, .. } => return Some(resolution),
            ResolutionState::NotOnPath | ResolutionState::NotObserved => {}
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
        // Counted across every installed row, not just the source row: the
        // question is "how much of this analysis rests on evidence that was not
        // available", and a dependency row's unknown ownership is as much a gap
        // as a top-level one (F-39). A mapped row names a package that is not
        // installed, so no provider was ever asked why it was installed; it is
        // not a gap in that evidence, and the sentence `status` prints ("the
        // provider kept no record") would be false of it (SE-097).
        s.ownership_unknown += u32::try_from(
            p.rows
                .values()
                .filter(|r| {
                    r.presence == RowPresence::Installed
                        && r.ownership == macportsbrew_domain::Ownership::Unknown
                })
                .count(),
        )
        .unwrap_or(u32::MAX);
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

#[cfg(test)]
mod tests {
    use super::*;
    use macportsbrew_domain::{CapabilityState, InstalledPackage, ProviderInventory};

    /// Every source there is; the `match` makes a new one a compile error
    /// here rather than a source this test silently skips.
    fn every_source() -> [AnalysisSource; 2] {
        let every = [AnalysisSource::Live, AnalysisSource::Snapshot];
        for source in every {
            match source {
                AnalysisSource::Live | AnalysisSource::Snapshot => {}
            }
        }
        every
    }

    fn row(name: &str, version: &str, active: bool, requested: Option<bool>) -> InstalledPackage {
        InstalledPackage {
            identity: PackageIdentity::new(ProviderId::new("alpha"), name),
            kind: macportsbrew_domain::PackageKind::Package,
            version: macportsbrew_domain::Version::new(version),
            revision: None,
            variants: BTreeSet::new(),
            active,
            requested,
            installed_from_prebuilt: None,
            install_path: None,
        }
    }

    /// A snapshot with no providers, for tests that supply their own.
    fn snapshot_shell() -> Snapshot {
        use macportsbrew_domain::{
            Architecture, CommandLineTools, EnvironmentSnapshot, Machine, MacosVersion, Provenance,
            Translation, SNAPSHOT_SCHEMA_VERSION,
        };
        Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            tool_version: "test".into(),
            policy_version: "test".into(),
            provenance: Provenance {
                captured_at: "2026-10-12T00:00:00Z".into(),
                captured_by: "analysis.rs".into(),
                origin: "synthesised".into(),
                note: None,
            },
            machine: Machine {
                macos: MacosVersion::new(26, 0, 0),
                architecture: Architecture::Arm64,
                translation: Translation::Native,
                command_line_tools: CommandLineTools::Missing,
            },
            environment: EnvironmentSnapshot::default(),
            providers: BTreeMap::new(),
        }
    }

    fn provider(rows: Vec<InstalledPackage>, reverse: CapabilityState) -> ProviderSnapshot {
        ProviderSnapshot {
            detection: macportsbrew_domain::ProviderDetection {
                provider: ProviderId::new("alpha"),
                presence: macportsbrew_domain::ProviderPresence::Present,
                version: None,
                prefix: None,
                executable: None,
                architecture: None,
                prefix_redacted: false,
            },
            capabilities: macportsbrew_domain::ProviderCapabilities {
                capabilities: [(
                    macportsbrew_domain::Capability::ReverseDependencies,
                    reverse,
                )]
                .into_iter()
                .collect(),
            },
            inventory: ProviderInventory {
                installed: rows,
                ..ProviderInventory::default()
            },
            environment: BTreeMap::new(),
            invocation_environment: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            configuration: macportsbrew_domain::ProviderConfiguration::NotRecorded,
        }
    }

    #[test]
    fn nothing_is_an_orphan_unless_its_dependents_could_be_read() {
        let rows = || vec![row("lib", "1", true, Some(false))];
        let read = provider(rows(), CapabilityState::Available);
        assert_eq!(ownership_of(&read, "lib").0, Ownership::Orphan);
        let unread = provider(rows(), CapabilityState::Unknown);
        assert_eq!(ownership_of(&unread, "lib").0, Ownership::Dependency);
        let unavailable = provider(rows(), CapabilityState::Unavailable { reason: "x".into() });
        assert_eq!(ownership_of(&unavailable, "lib").0, Ownership::Dependency);
        // A snapshot that never mentions the capability has not answered.
        let mut silent = provider(rows(), CapabilityState::Available);
        silent.capabilities.capabilities.clear();
        assert_eq!(ownership_of(&silent, "lib").0, Ownership::Dependency);
    }

    #[test]
    fn a_dependency_reached_by_default_is_marked_and_a_read_one_is_not() {
        let id = ProviderId::new("alpha");
        let rows_of = |p: &ProviderSnapshot, name: &str| {
            let (ownership, evidence) = ownership_of(p, name);
            BTreeMap::from([(id.clone(), installed_row(p, name, ownership, evidence))])
        };
        let snapshot_of = |p: ProviderSnapshot| Snapshot {
            providers: BTreeMap::from([(id.clone(), p)]),
            ..snapshot_shell()
        };
        let asked = |name| row(name, "1", true, Some(true));
        let not_asked = |name| row(name, "1", true, Some(false));

        // Reverse dependencies unread: nobody asked for it, and the split was
        // defaulted - marked.
        let unread = provider(vec![not_asked("lib")], CapabilityState::Unknown);
        assert_eq!(
            split_defaulted(&snapshot_of(unread.clone()), &rows_of(&unread, "lib")),
            vec![id.clone()]
        );
        // The same capability silent in the document has not answered either.
        let mut silent = provider(vec![not_asked("lib")], CapabilityState::Available);
        silent.capabilities.capabilities.clear();
        assert_eq!(
            split_defaulted(&snapshot_of(silent.clone()), &rows_of(&silent, "lib")),
            vec![id.clone()]
        );
        // Read, with a dependent: a real dependency, not marked.
        let mut read = provider(vec![not_asked("lib")], CapabilityState::Available);
        read.inventory
            .dependencies
            .push(macportsbrew_domain::DependencyEdge {
                from: "app".into(),
                to: "lib".into(),
                build_only: false,
            });
        assert_eq!(ownership_of(&read, "lib").0, Ownership::Dependency);
        assert!(split_defaulted(&snapshot_of(read.clone()), &rows_of(&read, "lib")).is_empty());
        // Read, with none: an orphan, which is an answer, not marked.
        let orphan = provider(vec![not_asked("lib")], CapabilityState::Available);
        assert!(split_defaulted(&snapshot_of(orphan.clone()), &rows_of(&orphan, "lib")).is_empty());
        // Asked for, whatever else is unread: top level, not marked.
        let top = provider(vec![asked("tool")], CapabilityState::Unknown);
        assert!(split_defaulted(&snapshot_of(top.clone()), &rows_of(&top, "tool")).is_empty());
    }

    #[test]
    fn intent_is_read_from_the_active_installation() {
        // The older, inactive version sorts first and was pulled in as a
        // dependency; the one in use was asked for.
        let rows = vec![
            row("tool", "1", false, Some(false)),
            row("tool", "2", true, Some(true)),
        ];
        let p = provider(rows, CapabilityState::Available);
        assert_eq!(ownership_of(&p, "tool").0, Ownership::TopLevel);
        // With no record on the active row, another row's record still counts.
        let rows = vec![
            row("tool", "1", false, Some(true)),
            row("tool", "2", true, None),
        ];
        let p = provider(rows, CapabilityState::Available);
        assert_eq!(ownership_of(&p, "tool").0, Ownership::TopLevel);
    }

    #[test]
    fn the_concurrency_flag_never_claims_a_check_it_did_not_make() {
        // This build has no detection mechanism, so no source may answer as
        // if one ran: never `NoneObserved`, never `Observed`, and always
        // possibly inconsistent (`04-migration-and-safety.md`).
        for source in every_source() {
            let activity = source.concurrent_provider_activity();
            assert!(!activity.was_checked(), "{source:?}: {activity:?}");
            assert!(activity.possibly_inconsistent(), "{source:?}: {activity:?}");
        }
        // And the two that remain are told apart: nobody looked on a live
        // run; nobody could have looked at a recording or a snapshot file.
        assert_eq!(
            AnalysisSource::Live.concurrent_provider_activity(),
            ConcurrentProviderActivity::NotChecked
        );
        assert_eq!(
            AnalysisSource::Snapshot.concurrent_provider_activity(),
            ConcurrentProviderActivity::Unknowable
        );
    }
}
