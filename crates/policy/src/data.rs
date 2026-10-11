// SPDX-License-Identifier: Apache-2.0

//! The versioned policy data set: everything the rules know about a provider,
//! held as dated data rather than as code (INV-20;
//! `docs/spec/03-policy-and-recommendations.md`, "Policy data").
//!
//! The files live in `data/policy/` at the repository root and are embedded
//! at build time. That is the only file access this crate performs, it
//! happens at compile time, and there is none at run time.
//!
//! Two things this module deliberately does not do. It does not read a clock:
//! `today` is an argument, because a staleness answer that depends on when it
//! was asked cannot be reproduced from a snapshot (INV-15). And it does not
//! decide anything: it answers questions about data, and the rules decide.

use macportsbrew_domain::{
    Architecture, Confidence, DatedFact, DomainError, ErrorId, Machine, MacosVersion, ProviderId,
    SupportState, SupportTier,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The six data files, as text. Separating the sources from the parser keeps
/// the crate free of run-time I/O while still letting a test build a data set
/// of its own — which is how the symmetry test (INV-17) gets two providers
/// whose data is identical.
#[derive(Clone, Copy, Debug)]
pub struct PolicySources<'a> {
    /// `policy.toml`: version and the staleness threshold.
    pub manifest: &'a str,
    /// `provider-support.toml`.
    pub support: &'a str,
    /// `bottle-tags.toml`: prebuilt-artifact variant naming per provider.
    pub artifact_tags: &'a str,
    /// `stateful-packages.toml`.
    pub stateful: &'a str,
    /// `runtime-managers.toml`.
    pub runtime_managers: &'a str,
    /// `curated-mappings.toml`.
    pub curated_mappings: &'a str,
}

/// The data files shipped with this build.
pub fn embedded_sources() -> PolicySources<'static> {
    PolicySources {
        manifest: include_str!("../../../data/policy/policy.toml"),
        support: include_str!("../../../data/policy/provider-support.toml"),
        artifact_tags: include_str!("../../../data/policy/bottle-tags.toml"),
        stateful: include_str!("../../../data/policy/stateful-packages.toml"),
        runtime_managers: include_str!("../../../data/policy/runtime-managers.toml"),
        curated_mappings: include_str!("../../../data/policy/curated-mappings.toml"),
    }
}

// ---------------------------------------------------------------- entries

/// Which machines an entry applies to on the architecture axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArchitectureScope {
    /// Every architecture.
    Any,
    /// Exactly one.
    One(Architecture),
}

impl ArchitectureScope {
    fn parse(raw: &str) -> Option<Self> {
        if raw.trim().eq_ignore_ascii_case("any") {
            return Some(ArchitectureScope::Any);
        }
        Architecture::parse(raw).map(ArchitectureScope::One)
    }

    fn covers(self, arch: Architecture) -> bool {
        match self {
            ArchitectureScope::Any => true,
            ArchitectureScope::One(a) => a == arch,
        }
    }
}

/// One provider-support row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportEntry {
    /// Stable identifier, quoted back in [`SupportState::entry`].
    pub id: String,
    /// Provider id this row describes.
    pub provider: String,
    /// `x86_64`, `arm64` or `any`.
    #[serde(default = "scope_any")]
    pub architecture: String,
    /// Inclusive lower macOS bound, `MAJOR.MINOR`.
    #[serde(default)]
    pub macos_min: Option<String>,
    /// Inclusive upper macOS bound, `MAJOR.MINOR`.
    #[serde(default)]
    pub macos_max: Option<String>,
    /// The support tier.
    pub tier: SupportTier,
    /// The provider's own name for the state.
    #[serde(default)]
    pub provider_label: Option<String>,
    /// A stated date after which support ends, `YYYY-MM-DD`.
    #[serde(default)]
    pub end_date: Option<String>,
    /// Whether the provider publishes prebuilt artifacts for such a machine.
    #[serde(default)]
    pub prebuilt_artifacts: Option<bool>,
    /// Where this was read.
    pub source: String,
    /// When, `YYYY-MM-DD`.
    pub read_on: String,
    /// What the source said, in English.
    #[serde(default)]
    pub note: String,
}

fn scope_any() -> String {
    "any".to_string()
}

impl SupportEntry {
    /// How narrowly this row is scoped. A row naming an architecture beats
    /// one that says `any`; bounded ranges beat unbounded ones.
    fn specificity(&self) -> u32 {
        let mut score = 0;
        if !matches!(
            ArchitectureScope::parse(&self.architecture),
            Some(ArchitectureScope::Any) | None
        ) {
            score += 4;
        }
        if self.macos_min.is_some() {
            score += 1;
        }
        if self.macos_max.is_some() {
            score += 1;
        }
        score
    }

    fn covers(&self, machine: &Machine) -> bool {
        let Some(scope) = ArchitectureScope::parse(&self.architecture) else {
            return false;
        };
        if !scope.covers(machine.architecture) {
            return false;
        }
        let here = machine.macos.release();
        if let Some(min) = self.macos_min.as_deref().and_then(parse_release) {
            if here < min {
                return false;
            }
        }
        if let Some(max) = self.macos_max.as_deref().and_then(parse_release) {
            if here > max {
                return false;
            }
        }
        true
    }

    fn to_state(&self) -> SupportState {
        SupportState {
            tier: self.tier,
            provider_label: self.provider_label.clone(),
            end_date: self.end_date.clone(),
            prebuilt_artifacts: self.prebuilt_artifacts,
            fact: DatedFact {
                source: self.source.clone(),
                read_on: self.read_on.clone(),
            },
            entry: self.id.clone(),
        }
    }
}

/// How one provider names the prebuilt-artifact variant that fits a machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTagSet {
    /// Provider id.
    pub provider: String,
    /// The platform-independent variant name, when the provider has one.
    #[serde(default)]
    pub universal_tag: Option<String>,
    /// Whether a variant built for an older macOS of the same architecture is
    /// accepted on this machine.
    #[serde(default)]
    pub older_compatible_same_architecture: bool,
    /// Per-architecture name templates.
    #[serde(default)]
    pub pattern: Vec<ArtifactTagPattern>,
    /// The macOS release names this provider uses inside a variant name.
    #[serde(default)]
    pub release: Vec<ArtifactReleaseName>,
    /// Where this was read.
    pub source: String,
    /// When, `YYYY-MM-DD`.
    pub read_on: String,
    /// What the source said.
    #[serde(default)]
    pub note: String,
}

/// One architecture's variant-name template.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTagPattern {
    /// `x86_64` or `arm64`.
    pub architecture: String,
    /// `{release}` is replaced by the release name.
    pub template: String,
}

/// What a provider calls one macOS release inside a variant name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReleaseName {
    /// `MAJOR.MINOR`.
    pub macos: String,
    /// The name.
    pub name: String,
}

/// A family of packages that owns persistent data outside its own prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatefulFamily {
    /// Stable lower-case identifier for the upstream software.
    pub family: String,
    /// Package names per provider. A name may contain at most one `*`.
    #[serde(default)]
    pub names: BTreeMap<String, Vec<String>>,
    /// Data paths per provider, relative to that provider's own prefix.
    #[serde(default)]
    pub data_paths: BTreeMap<String, Vec<String>>,
    /// Where this was read.
    pub source: String,
    /// When, `YYYY-MM-DD`.
    pub read_on: String,
    /// What the software keeps and why moving it is not mechanical.
    #[serde(default)]
    pub rationale: String,
}

impl StatefulFamily {
    /// True when `name` under `provider` belongs to this family.
    pub fn covers(&self, provider: &str, name: &str) -> bool {
        self.names
            .get(provider)
            .is_some_and(|patterns| patterns.iter().any(|p| pattern_matches(p, name)))
    }
}

/// A version manager to suggest for a runtime, never to act with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeManager {
    /// The tool's own name.
    pub id: String,
    /// Package families it manages.
    #[serde(default)]
    pub for_families: Vec<String>,
    /// The project's own site.
    pub source: String,
    /// When that site was fetched.
    pub read_on: String,
    /// Whether the site answered on `read_on`.
    #[serde(default)]
    pub confirmed: bool,
    /// What the site says the tool does.
    #[serde(default)]
    pub note: String,
}

/// One curated equivalence between two providers' package names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CuratedMapping {
    /// Provider the row was written from.
    pub source_provider: String,
    /// Package name there.
    pub source_name: String,
    /// Provider the row points at.
    pub destination_provider: String,
    /// Package name there.
    pub destination_name: String,
    /// Always `EXACT` for a curated row.
    pub confidence: Confidence,
    /// Where both names were read.
    pub source: String,
    /// When, `YYYY-MM-DD`.
    pub read_on: String,
    /// Why a heuristic does not reach this pair.
    #[serde(default)]
    pub rationale: String,
}

// ---------------------------------------------------------------- file shapes

#[derive(Debug, Deserialize)]
struct Manifest {
    version: String,
    stale_after_days: u32,
}

#[derive(Debug, Deserialize)]
struct SupportFile {
    #[serde(default)]
    entry: Vec<SupportEntry>,
}

#[derive(Debug, Deserialize)]
struct TagFile {
    #[serde(default)]
    tagset: Vec<ArtifactTagSet>,
}

#[derive(Debug, Deserialize)]
struct StatefulFile {
    #[serde(default)]
    entry: Vec<StatefulFamily>,
}

#[derive(Debug, Deserialize)]
struct ManagerFile {
    #[serde(default)]
    entry: Vec<RuntimeManager>,
}

#[derive(Debug, Deserialize)]
struct MappingFile {
    #[serde(default)]
    entry: Vec<CuratedMapping>,
}

// ---------------------------------------------------------------- the data set

/// The whole policy data set, parsed and indexed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyData {
    version: String,
    stale_after_days: u32,
    newest_read_on: String,
    support: Vec<SupportEntry>,
    tag_sets: Vec<ArtifactTagSet>,
    stateful: Vec<StatefulFamily>,
    managers: Vec<RuntimeManager>,
    mappings: Vec<CuratedMapping>,
    equivalences: BTreeMap<(String, String, String), BTreeSet<String>>,
}

impl PolicyData {
    /// Parses the data set shipped with this build.
    pub fn load_embedded() -> Result<Self, DomainError> {
        Self::load_from(embedded_sources())
    }

    /// Parses a data set from text. The only entry point that exists so that
    /// a test can supply a data set of its own; it performs no I/O either.
    pub fn load_from(sources: PolicySources<'_>) -> Result<Self, DomainError> {
        let manifest: Manifest = parse(sources.manifest, "policy.toml")?;
        let support: SupportFile = parse(sources.support, "provider-support.toml")?;
        let tags: TagFile = parse(sources.artifact_tags, "bottle-tags.toml")?;
        let stateful: StatefulFile = parse(sources.stateful, "stateful-packages.toml")?;
        let managers: ManagerFile = parse(sources.runtime_managers, "runtime-managers.toml")?;
        let mappings: MappingFile = parse(sources.curated_mappings, "curated-mappings.toml")?;

        let mut provenance: Vec<(&str, &str, &str)> = Vec::new();
        for e in &support.entry {
            provenance.push((&e.id, &e.source, &e.read_on));
        }
        for t in &tags.tagset {
            provenance.push((&t.provider, &t.source, &t.read_on));
        }
        for s in &stateful.entry {
            provenance.push((&s.family, &s.source, &s.read_on));
        }
        for m in &managers.entry {
            provenance.push((&m.id, &m.source, &m.read_on));
        }
        for m in &mappings.entry {
            provenance.push((&m.destination_name, &m.source, &m.read_on));
        }
        let newest = newest_provenance(&provenance)?;
        check_support_bounds(&support.entry)?;
        check_curated_names(&mappings.entry)?;
        check_artifact_tags(&tags.tagset)?;
        let equivalences = index_equivalences(&mappings.entry);

        let mut support_entries = support.entry;
        support_entries.sort_by(|a, b| a.id.cmp(&b.id));
        let mut tag_sets = tags.tagset;
        tag_sets.sort_by(|a, b| a.provider.cmp(&b.provider));
        let mut stateful_families = stateful.entry;
        stateful_families.sort_by(|a, b| a.family.cmp(&b.family));
        let mut manager_entries = managers.entry;
        manager_entries.sort_by(|a, b| a.id.cmp(&b.id));
        let mut mapping_entries = mappings.entry;
        mapping_entries.sort_by(|a, b| {
            (
                &a.source_provider,
                &a.source_name,
                &a.destination_provider,
                &a.destination_name,
            )
                .cmp(&(
                    &b.source_provider,
                    &b.source_name,
                    &b.destination_provider,
                    &b.destination_name,
                ))
        });

        Ok(Self {
            version: manifest.version,
            stale_after_days: manifest.stale_after_days,
            newest_read_on: newest,
            support: support_entries,
            tag_sets,
            stateful: stateful_families,
            managers: manager_entries,
            mappings: mapping_entries,
            equivalences,
        })
    }

    /// The policy version every recommendation carries.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The staleness threshold, in days.
    pub fn stale_after_days(&self) -> u32 {
        self.stale_after_days
    }

    /// The newest read date across **the files this crate loads**, `YYYY-MM-DD`.
    ///
    /// Not the whole of `data/policy/`, and the distinction became visible on
    /// 2026-09-19: `provider-commands.toml` is read by the application layer
    /// and `provider-prefixes.toml` by the adapters (DP-014), so a read date
    /// moving in either is invisible here. `policy.toml`'s `version` *does*
    /// span the directory, so the two legitimately differ and a payload can
    /// carry a `policyVersion` dated 2026-09-19 beside a `policyReadOn` of
    /// 2026-09-04 (as it did when this was written).
    ///
    /// The narrower set is kept deliberately rather than widened. This value
    /// drives the staleness notice, and the older date warns **earlier** — a
    /// data set is as fresh as its stalest half, not its freshest. Widening it
    /// would also mean this crate reading files it does not own, which is the
    /// layering those files were placed by.
    pub fn newest_read_on(&self) -> &str {
        &self.newest_read_on
    }

    /// Whether the data set is older than its own threshold, as of `today`.
    ///
    /// `today` is an argument and never a clock read (INV-14). A `today` that
    /// is not a `YYYY-MM-DD` date answers `false`: this drives a non-blocking
    /// notice, and an unreadable input is not evidence of staleness.
    pub fn is_stale(&self, today: &str) -> bool {
        days_between(&self.newest_read_on, today)
            .is_some_and(|days| days > i64::from(self.stale_after_days))
    }

    /// Every support row, sorted by id.
    pub fn support_entries(&self) -> &[SupportEntry] {
        &self.support
    }

    /// The support state of `provider` on `machine`.
    ///
    /// The most specific matching row wins; ties go to the lowest id, which
    /// makes the answer independent of file order (INV-15). No match is
    /// `UNKNOWN`, never a positive.
    pub fn support_state(&self, provider: &ProviderId, machine: &Machine) -> SupportState {
        let mut best: Option<&SupportEntry> = None;
        for entry in &self.support {
            if entry.provider != provider.as_str() || !entry.covers(machine) {
                continue;
            }
            let better = best.is_none_or(|b| entry.specificity() > b.specificity());
            if better {
                best = Some(entry);
            }
        }
        best.map_or_else(
            || SupportState::unknown("no-matching-entry"),
            SupportEntry::to_state,
        )
    }

    /// The prebuilt-artifact variant names that would satisfy `machine` under
    /// `provider`, most specific first: the exact name for this machine, then
    /// the older same-architecture names the provider accepts, then the
    /// platform-independent one. Empty when the provider has no such data.
    pub fn artifact_tags_for(&self, provider: &str, machine: &Machine) -> Vec<String> {
        let Some(set) = self.tag_sets.iter().find(|t| t.provider == provider) else {
            return Vec::new();
        };
        let arch = machine.architecture.to_string();
        let Some(pattern) = set.pattern.iter().find(|p| p.architecture == arch) else {
            return Vec::new();
        };

        let mut releases: Vec<((u32, u32), &str)> = set
            .release
            .iter()
            .filter_map(|r| parse_release(&r.macos).map(|rel| (rel, r.name.as_str())))
            .collect();
        releases.sort_by_key(|entry| std::cmp::Reverse(entry.0));

        let here = machine.macos.release();
        let mut tags: Vec<String> = Vec::new();
        for (release, name) in &releases {
            let usable = if set.older_compatible_same_architecture {
                *release <= here
            } else {
                *release == here
            };
            if usable {
                tags.push(pattern.template.replace("{release}", name));
            }
        }
        if let Some(universal) = &set.universal_tag {
            tags.push(universal.clone());
        }
        tags.dedup();
        tags
    }

    /// Every curated stateful family, sorted.
    pub fn stateful_families(&self) -> &[StatefulFamily] {
        &self.stateful
    }

    /// The curated family `name` belongs to under `provider`, when it does.
    pub fn stateful_family_of(&self, provider: &str, name: &str) -> Option<&StatefulFamily> {
        self.stateful.iter().find(|f| f.covers(provider, name))
    }

    /// The version managers that name `family`, sorted by id.
    pub fn runtime_managers_for(&self, family: &str) -> Vec<&RuntimeManager> {
        self.managers
            .iter()
            .filter(|m| m.for_families.iter().any(|f| f == family))
            .collect()
    }

    /// Every curated mapping row, sorted.
    pub fn curated_mappings(&self) -> &[CuratedMapping] {
        &self.mappings
    }

    /// The curated equivalents of `name` under `from`, expressed in `to`.
    /// Answers in both directions from the same rows (INV-17).
    pub fn curated_targets(&self, from: &str, name: &str, to: &str) -> Vec<String> {
        self.equivalences
            .get(&(from.to_string(), name.to_string(), to.to_string()))
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------- helpers

fn parse<T: for<'de> Deserialize<'de>>(text: &str, what: &str) -> Result<T, DomainError> {
    toml::from_str(text).map_err(|e| invalid(format!("{what}: {e}")))
}

/// Checks that every entry carries a source and a usable read date, and
/// returns the newest of those dates.
///
/// This is INV-20 enforced at load time rather than asserted in a test: an
/// entry with no provenance is a provider fact stated from memory, and the
/// data set that contains one does not load at all.
fn newest_provenance(entries: &[(&str, &str, &str)]) -> Result<String, DomainError> {
    let mut newest = String::new();
    for (what, source, read_on) in entries {
        if source.trim().is_empty() {
            return Err(invalid(format!("entry {what} carries no source (INV-20)")));
        }
        if parse_date(read_on).is_none() {
            return Err(invalid(format!(
                "entry {what} carries no usable read date: {read_on:?} (INV-20)"
            )));
        }
        if *read_on > newest.as_str() {
            newest = (*read_on).to_string();
        }
    }
    if newest.is_empty() {
        return Err(invalid("the data set carries no dated entry at all"));
    }
    Ok(newest)
}

/// Checks that every support row's architecture scope, macOS bounds and end
/// date can be read, so that a typo is an error rather than a row that
/// silently never matches.
fn check_support_bounds(entries: &[SupportEntry]) -> Result<(), DomainError> {
    for e in entries {
        if ArchitectureScope::parse(&e.architecture).is_none() {
            return Err(invalid(format!(
                "support entry {} names an unknown architecture scope {:?}",
                e.id, e.architecture
            )));
        }
        for bound in [e.macos_min.as_deref(), e.macos_max.as_deref()]
            .into_iter()
            .flatten()
        {
            if parse_release(bound).is_none() {
                return Err(invalid(format!(
                    "support entry {} names an unreadable macOS bound {bound:?}",
                    e.id
                )));
            }
        }
        if let Some(date) = e.end_date.as_deref() {
            if parse_date(date).is_none() {
                return Err(invalid(format!(
                    "support entry {} names an unreadable end date {date:?}",
                    e.id
                )));
            }
        }
    }
    Ok(())
}

/// Indexes the curated rows in both directions.
///
/// A curated pair is an equivalence between two package names, and an
/// equivalence has no preferred direction (INV-17). Only the citation has
/// one, and it stays on the row where it was written.
fn index_equivalences(
    entries: &[CuratedMapping],
) -> BTreeMap<(String, String, String), BTreeSet<String>> {
    let mut index: BTreeMap<(String, String, String), BTreeSet<String>> = BTreeMap::new();
    for m in entries {
        index
            .entry((
                m.source_provider.clone(),
                m.source_name.clone(),
                m.destination_provider.clone(),
            ))
            .or_default()
            .insert(m.destination_name.clone());
        index
            .entry((
                m.destination_provider.clone(),
                m.destination_name.clone(),
                m.source_provider.clone(),
            ))
            .or_default()
            .insert(m.source_name.clone());
    }
    index
}

/// Every artifact tag is one component of a file name, not a path.
///
/// `artifact_tags_for` builds a tag by substituting a release name into a
/// template, and the first tag reaches
/// `homebrew::offline_index_path` as `packages.{tag}.jws.json`, joined onto the
/// Homebrew cache directory. A `/` or a `..` in a template, a release name or the
/// universal tag therefore walks out of that directory, and the result is
/// `is_file()`-probed with its path quoted into a capability reason. Bounded -
/// it needs a commit to this repository's own data, and replay answers `Unknown`
/// without touching the filesystem - but that is the same threat model
/// `check_curated_names` below is defending, so it is defended the same way.
///
/// The condition is stated directly rather than borrowed from a package-name
/// predicate: what this value must be is one path component, and the check says
/// so. Found by `security-reviewer` while reviewing that one (F-48).
fn check_artifact_tags(sets: &[ArtifactTagSet]) -> Result<(), DomainError> {
    let component = |what: &str, provider: &str, value: &str| {
        if value.is_empty()
            || value.len() > 64
            || value.contains('/')
            || value.contains('\\')
            || value.contains("..")
            // Every hostile class, not the control half a snapshot refuses: this
            // is the repository's own data, never a name a machine reported, and
            // it becomes a file name quoted into a capability reason, so there is
            // no legitimate name to keep and nothing is lost by refusing a
            // format character too (`macportsbrew_domain::text`).
            || value.chars().any(macportsbrew_domain::is_hostile)
        {
            return Err(invalid(format!(
                "artifact tag {what} {value:?} for {provider:?} is not a single file-name \
                 component; it would build a path outside the provider's cache directory"
            )));
        }
        Ok(())
    };
    for set in sets {
        if let Some(universal) = &set.universal_tag {
            component("universal_tag", &set.provider, universal)?;
        }
        for pattern in &set.pattern {
            component("template", &set.provider, &pattern.template)?;
        }
        for release in &set.release {
            component("release name", &set.provider, &release.name)?;
        }
    }
    Ok(())
}

/// Every curated package name is one a provider could have reported.
///
/// A curated destination reaches `planner::command` without ever passing
/// `Snapshot::check`, which only ever sees what a provider said. Until this
/// existed, the sole gate on those names was the planner's own guard - and that
/// guard answers `InputUnusable`, which tells the user to check their capture,
/// snapshot or recording for a defect that would be in the build's own data
/// (F-48). Refusing here puts the failure where the data is owned, says it is a
/// build defect, and makes the planner's guard the second line it was already
/// being described as.
fn check_curated_names(entries: &[CuratedMapping]) -> Result<(), DomainError> {
    for e in entries {
        for (what, name) in [
            ("source", &e.source_name),
            ("destination", &e.destination_name),
        ] {
            if !macportsbrew_domain::is_safe_provider_name(name) {
                return Err(invalid(format!(
                    "curated mapping {:?}/{:?} names a {what} {name:?} that no provider \
                     could have reported; it would be read as a flag or a path",
                    e.source_provider, e.source_name
                )));
            }
        }
    }
    Ok(())
}

fn invalid(detail: impl Into<String>) -> DomainError {
    DomainError::new(ErrorId::PolicyDataInvalid, detail)
}

/// True when `name` matches `pattern`. A pattern may contain at most one
/// `*`, which stands for any run of characters; everything else is literal.
/// A trailing `*` is the common case and is the degenerate form of this.
fn pattern_matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((prefix, suffix)) => {
            name.len() >= prefix.len() + suffix.len()
                && name.starts_with(prefix)
                && name.ends_with(suffix)
        }
    }
}

fn parse_release(raw: &str) -> Option<(u32, u32)> {
    MacosVersion::parse(raw).map(|v| v.release())
}

/// Parses a `YYYY-MM-DD` date into its parts, rejecting anything else.
fn parse_date(raw: &str) -> Option<(i64, i64, i64)> {
    let raw = raw.trim();
    let bytes = raw.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    // Digits only: `parse` would otherwise accept a sign (`2026-+1-05`).
    if !bytes
        .iter()
        .enumerate()
        .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return None;
    }
    let year: i64 = raw[0..4].parse().ok()?;
    let month: i64 = raw[5..7].parse().ok()?;
    let day: i64 = raw[8..10].parse().ok()?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some((year, month, day))
}

/// The length of a month, so that a date nobody could have read a source on
/// is rejected rather than quietly normalised into a neighbouring one.
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since the civil epoch, by Howard Hinnant's `days_from_civil`. Chosen
/// because it is exact, needs no table and no dependency, and is the same
/// arithmetic on every platform — which is what INV-15 asks of it.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The inverse of [`days_from_civil`], same source.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (if month <= 2 { y + 1 } else { y }, month, day)
}

/// Whole days from `from` to `to`, both `YYYY-MM-DD`. Negative when `to` is
/// the earlier date; `None` when either is not a date.
///
/// This is the only date arithmetic in the crate, and it takes both ends as
/// arguments: no rule may ask what day it is (INV-14).
pub fn days_between(from: &str, to: &str) -> Option<i64> {
    let (fy, fm, fd) = parse_date(from)?;
    let (ty, tm, td) = parse_date(to)?;
    Some(days_from_civil(ty, tm, td) - days_from_civil(fy, fm, fd))
}

/// The `YYYY-MM-DD` date `days` after `date`, or before it when negative.
///
/// Exists so that a test can say "ninety days from the day these facts were
/// assembled" instead of writing a date, which keeps provider deadlines out
/// of source entirely — the thing INV-20 asks for.
pub fn shift_date(date: &str, days: i64) -> Option<String> {
    let (y, m, d) = parse_date(date)?;
    let (year, month, day) = civil_from_days(days_from_civil(y, m, d) + days);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use macportsbrew_domain::{CommandLineTools, Translation};

    fn machine(arch: Architecture, macos: &str) -> Machine {
        Machine {
            architecture: arch,
            macos: MacosVersion::parse(macos).unwrap(),
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Unknown,
        }
    }

    #[test]
    fn the_shipped_data_set_parses_and_is_dated() {
        let data = PolicyData::load_embedded().unwrap();
        assert!(!data.version().is_empty());
        assert!(parse_date(data.newest_read_on()).is_some());
        assert!(!data.support_entries().is_empty());
        assert!(!data.stateful_families().is_empty());
        assert!(!data.curated_mappings().is_empty());
        for entry in data.support_entries() {
            assert!(
                !entry.source.trim().is_empty(),
                "{} has no source",
                entry.id
            );
            assert!(parse_date(&entry.read_on).is_some(), "{}", entry.id);
        }
    }

    #[test]
    fn the_most_specific_support_row_wins() {
        let data = PolicyData::load_embedded().unwrap();
        let one = ProviderId::new("homebrew");

        let intel = data.support_state(&one, &machine(Architecture::X86_64, "26.6.2"));
        assert_eq!(intel.tier, SupportTier::Degraded);
        assert!(intel.end_date.is_some());

        let silicon = data.support_state(&one, &machine(Architecture::Arm64, "26.0"));
        assert_eq!(silicon.tier, SupportTier::Supported);

        // The `any` row below the floor beats nothing more specific there.
        let ancient = data.support_state(&one, &machine(Architecture::X86_64, "10.14.6"));
        assert_eq!(ancient.tier, SupportTier::Unsupported);
    }

    #[test]
    fn an_unmatched_machine_is_unknown_not_a_positive() {
        let data = PolicyData::load_embedded().unwrap();
        let nobody = ProviderId::new("not-a-provider");
        let state = data.support_state(&nobody, &machine(Architecture::Arm64, "26.0"));
        assert_eq!(state.tier, SupportTier::Unknown);
        assert_eq!(state.entry, "no-matching-entry");
        assert!(state.prebuilt_artifacts.is_none());
    }

    #[test]
    fn artifact_tags_are_most_specific_first() {
        let data = PolicyData::load_embedded().unwrap();

        let intel = data.artifact_tags_for("homebrew", &machine(Architecture::X86_64, "26.6.2"));
        assert_eq!(intel.first().map(String::as_str), Some("tahoe"));
        assert!(intel.contains(&"sonoma".to_string()));
        assert_eq!(intel.last().map(String::as_str), Some("all"));
        assert!(!intel.contains(&"golden_gate".to_string()));

        let silicon = data.artifact_tags_for("homebrew", &machine(Architecture::Arm64, "15.1"));
        assert_eq!(silicon.first().map(String::as_str), Some("arm64_sequoia"));
        assert!(!silicon.contains(&"arm64_tahoe".to_string()));

        // A provider that accepts no older release gets exactly one name
        // plus its universal one.
        let other = data.artifact_tags_for("macports", &machine(Architecture::X86_64, "26.6.2"));
        assert_eq!(other, vec!["darwin_25.x86_64", "noarch"]);

        assert!(data
            .artifact_tags_for("not-a-provider", &machine(Architecture::Arm64, "26.0"))
            .is_empty());
    }

    #[test]
    fn curated_equivalences_answer_in_both_directions() {
        let data = PolicyData::load_embedded().unwrap();
        let forward = data.curated_targets("homebrew", "postgresql@17", "macports");
        assert_eq!(forward, vec!["postgresql17", "postgresql17-server"]);
        let back = data.curated_targets("macports", "postgresql17-server", "homebrew");
        assert_eq!(back, vec!["postgresql@17"]);
        assert!(data
            .curated_targets("homebrew", "nothing-here", "macports")
            .is_empty());
    }

    #[test]
    fn stateful_patterns_match_one_wildcard() {
        assert!(pattern_matches("postgresql@*", "postgresql@17"));
        assert!(pattern_matches("mysql*", "mysql@8.4"));
        assert!(!pattern_matches("mysql", "mysql@8.4"));
        assert!(pattern_matches("redis", "redis"));
        assert!(!pattern_matches("postgresql@*", "postgresql17"));

        let data = PolicyData::load_embedded().unwrap();
        let family = data
            .stateful_family_of("homebrew", "postgresql@17")
            .expect("the curated seed covers this family");
        assert_eq!(family.family, "postgresql");
        assert!(data.stateful_family_of("homebrew", "ripgrep").is_none());
    }

    #[test]
    fn runtime_managers_are_looked_up_by_family() {
        let data = PolicyData::load_embedded().unwrap();
        let for_python: Vec<&str> = data
            .runtime_managers_for("python")
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        assert!(for_python.contains(&"pyenv"));
        assert!(for_python.contains(&"mise"));
        assert!(!for_python.contains(&"rbenv"));
        assert!(data.runtime_managers_for("not-a-runtime").is_empty());
    }

    #[test]
    fn staleness_is_measured_against_a_given_day_not_a_clock() {
        let data = PolicyData::load_embedded().unwrap();
        let newest = data.newest_read_on().to_string();
        let threshold = i64::from(data.stale_after_days());
        assert!(threshold > 0);

        assert!(!data.is_stale(&newest));
        assert!(!data.is_stale(&shift_date(&newest, threshold).unwrap()));
        assert!(data.is_stale(&shift_date(&newest, threshold + 1).unwrap()));
        assert!(!data.is_stale("this is not a date"));
    }

    #[test]
    fn date_arithmetic_round_trips_and_rejects_non_dates() {
        let anchor = "2026-09-02";
        for days in [-400_i64, -1, 0, 1, 365, 366, 1_000] {
            let shifted = shift_date(anchor, days).unwrap();
            assert_eq!(days_between(anchor, &shifted), Some(days), "{days}");
            assert_eq!(shift_date(&shifted, -days).as_deref(), Some(anchor));
        }
        assert_eq!(days_between("2026-02-30", "2026-03-01"), None);
        assert_eq!(days_between("not a date", "2026-03-01"), None);
        assert_eq!(shift_date("nope", 1), None);
        // A sign is not a digit, though `parse` would take it.
        assert_eq!(days_between("2026-+1-05", "2026-03-01"), None);
        assert_eq!(days_between("+026-01-05", "2026-03-01"), None);
        // A leap day: the arithmetic is calendar-correct, not 365-day.
        assert_eq!(days_between("2024-02-28", "2024-03-01"), Some(2));
        assert_eq!(days_between("2026-02-28", "2026-03-01"), Some(1));
    }

    /// F-48: an artifact tag becomes part of a file name, so it must be one
    /// path component.
    ///
    /// `homebrew::offline_index_path` joins `packages.{tag}.jws.json` onto the
    /// cache directory, so a `/` or a `..` anywhere in the template, a release
    /// name or the universal tag walks out of it. Reachable only by committing
    /// to this repository's own data — the same threat model as the curated
    /// names below, defended the same way.
    #[test]
    fn an_artifact_tag_that_is_a_path_is_refused_where_the_data_lives() {
        let tagset = |universal: &str, template: &str, release_name: &str| {
            format!(
                "[[tagset]]\nprovider=\"homebrew\"\nuniversal_tag=\"{universal}\"\n\
                 older_compatible_same_architecture=true\nsource=\"s\"\n\
                 read_on=\"2026-09-02\"\n\
                 [[tagset.pattern]]\narchitecture=\"arm64\"\ntemplate=\"{template}\"\n\
                 [[tagset.release]]\nmacos=\"26\"\nname=\"{release_name}\"\n"
            )
        };

        let mut sources = embedded_sources();
        let good = tagset("all", "arm64_{release}", "tahoe");
        sources.artifact_tags = &good;
        PolicyData::load_from(sources).expect("an ordinary tagset loads");

        for (universal, template, release_name) in [
            ("all", "../../../etc/{release}", "tahoe"),
            ("all", "arm64_{release}", "../../../etc/passwd"),
            ("../../../etc/passwd", "arm64_{release}", "tahoe"),
            ("all", "a/b_{release}", "tahoe"),
            ("", "arm64_{release}", "tahoe"),
            // Any hostile class, not only the control half a snapshot refuses
            // (`macportsbrew_domain::text`): TOML's own escapes, so the bytes
            // reach the check as the characters.
            ("a\\u001Bb", "arm64_{release}", "tahoe"),
            ("a\\u200Bb", "arm64_{release}", "tahoe"),
            ("all", "arm64_{release}", "ta\\u202Ehoe"),
        ] {
            let mut sources = embedded_sources();
            let bad = tagset(universal, template, release_name);
            sources.artifact_tags = &bad;
            match PolicyData::load_from(sources) {
                Ok(_) => panic!(
                    "{universal:?} / {template:?} / {release_name:?} loaded; a tag that is a \
                     path must be refused"
                ),
                Err(e) => {
                    assert_eq!(e.id, ErrorId::PolicyDataInvalid);
                    assert!(
                        e.detail.contains("artifact tag"),
                        "this must be the tag check: {}",
                        e.detail
                    );
                }
            }
        }

        // And the tags this build ships pass, or the check is too strict to keep.
        PolicyData::load_embedded().expect("the shipped tag sets");
    }

    /// F-48: a curated destination reaches `planner::command` without ever
    /// passing `Snapshot::check`, which only sees what a provider reported.
    ///
    /// The planner's own guard did catch it, and answered `InputUnusable` —
    /// telling the user to check their capture, snapshot or recording for a
    /// defect that would be in the build's own compiled-in data. Refusing at the
    /// layer that owns the data puts the blame where it belongs and makes the
    /// planner's guard the second line the F-43 entry had wrongly claimed it was.
    #[test]
    fn a_curated_name_no_provider_could_report_is_refused_where_the_data_lives() {
        let row = |source_name: &str, destination_name: &str| {
            format!(
                "[[entry]]\nsource_provider=\"homebrew\"\nsource_name=\"{source_name}\"\n\
                 destination_provider=\"macports\"\ndestination_name=\"{destination_name}\"\n\
                 confidence=\"EXACT\"\nsource=\"s\"\nread_on=\"2026-09-02\"\n"
            )
        };

        // The shape itself must load, or the cases below prove nothing.
        let mut sources = embedded_sources();
        let good = row("wget", "wget");
        sources.curated_mappings = &good;
        PolicyData::load_from(sources).expect("an ordinary curated row loads");

        for (source_name, destination_name) in [
            ("wget", "--force"),
            ("--force", "wget"),
            ("wget", "../../../bin/sh"),
            ("wget", ""),
            ("wget", "foo bar"),
        ] {
            let mut sources = embedded_sources();
            let bad = row(source_name, destination_name);
            sources.curated_mappings = &bad;
            // Matched rather than `expect_err`: the Ok side is the whole
            // `PolicyData`, and a panic that prints it buries the one line
            // whoever is debugging this needs.
            match PolicyData::load_from(sources) {
                Ok(_) => panic!(
                    "{source_name:?} -> {destination_name:?} loaded; a curated name that \
                     would read as a flag or a path must be refused"
                ),
                Err(e) => {
                    assert_eq!(
                        e.id,
                        ErrorId::PolicyDataInvalid,
                        "{source_name:?} -> {destination_name:?} is the build's defect, \
                         not the user's"
                    );
                    // The identifier is what every failure in this module
                    // carries, so on its own it cannot tell this check from a
                    // parse error or from `check_support_bounds`.
                    assert!(
                        e.detail.contains("curated mapping"),
                        "this must be the curated-name check: {}",
                        e.detail
                    );
                }
            }
        }

        // And the data this build actually ships passes.
        PolicyData::load_embedded().expect("the shipped curated table");
    }

    #[test]
    fn a_broken_data_set_is_an_error_not_a_default() {
        let mut sources = embedded_sources();
        sources.support = "this is not toml at all [[[";
        let err = PolicyData::load_from(sources).unwrap_err();
        assert_eq!(err.id, ErrorId::PolicyDataInvalid);

        let mut sources = embedded_sources();
        sources.support = "[[entry]]\nid=\"x\"\nprovider=\"p\"\ntier=\"SUPPORTED\"\nsource=\"\"\nread_on=\"2026-09-02\"\n";
        assert_eq!(
            PolicyData::load_from(sources).unwrap_err().id,
            ErrorId::PolicyDataInvalid
        );

        let mut sources = embedded_sources();
        sources.support = "[[entry]]\nid=\"x\"\nprovider=\"p\"\ntier=\"SUPPORTED\"\nsource=\"s\"\nread_on=\"whenever\"\n";
        assert_eq!(
            PolicyData::load_from(sources).unwrap_err().id,
            ErrorId::PolicyDataInvalid
        );
    }
}
