// SPDX-License-Identifier: Apache-2.0

//! The Homebrew read adapter (foundation pass).
//!
//! This module implements [`ProviderRead`] over the central process executor.
//! It reads and never writes: the eight commands it may run are a code-level
//! allowlist ([`ALLOWED_ARGV`]), and every one of them is a read that works
//! offline once Homebrew's API cache is present (R-1 finding 11, R-6 finding
//! 15; on an empty cache a Ruby read downloads it first, and a `brew.env` can
//! outrank the quieting environment - R-15). The executable is resolved from
//! a prefix the caller detected, never from `PATH` — a path-prefix lookup is
//! the executable-shadowing item of the threat model
//! (`04-migration-and-safety.md`).
//!
//! Three decisions here are worth knowing before reading the code:
//!
//! * **The platform tag is not computed here.** R-2 finding 1 shows the tag
//!   is a function of the macOS release *codename* and the architecture, and
//!   finding 2 shows no `brew` subcommand prints it. The caller supplies the
//!   tags that satisfy this machine, most specific first, so the
//!   codename table stays in policy data where it can be dated and revised.
//! * **`bottle.stable.files` is a resolution result, not a publication
//!   record** (R-2 finding 3). An empty `bottle` therefore cannot distinguish
//!   "nothing published anywhere" from "nothing published for this platform",
//!   and the evidence attached to every `SOURCE_BUILD_REQUIRED` answer says
//!   so rather than letting the caller assume the stronger claim.
//! * **`installed[]` is oldest keg first** (R-1 finding 7). The row that
//!   matters is the one whose version equals `linked_keg`, or the newest when
//!   nothing is on the default path — reading element zero reports every
//!   package as prebuilt and is a silent-misparse bug.

pub mod json;

use crate::filesystem::FilesystemScope;
use crate::process::{
    sanitised_environment, CommandSpec, EnvironmentSpec, ExitStatus, Invocation, ProcessRunner,
};
use json::{parse_installed_json, Cask, Formula, Observed, Parsed};
use macportsbrew_domain::availability::SourceBuildReason;
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, Capability,
    CapabilityState, DependencyEdge, DomainError, ErrorId, ExecutableOwner, InstalledPackage,
    Machine, PackageIdentity, PackageKind, PackageMetadata, ProviderCapabilities,
    ProviderDetection, ProviderId, ProviderInventory, ProviderPresence, ProviderRead,
    ProviderSnapshot, RequestedSet, Resolution, Version,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The provider id this adapter answers for.
pub const PROVIDER_ID: &str = "homebrew";

/// Where the executable sits under the prefix.
///
/// One constant, not two. A separate `EXECUTABLE_NAME` used by the presence
/// check while this one was used by the spawn would let the tool check one path
/// and run another the day they diverged; MacPorts always had one.
pub(crate) const EXECUTABLE_RELATIVE: &str = "bin/brew";

/// The mechanism string recorded on [`RequestedSet`]: Homebrew's own record
/// of user intent, and the only one that is authoritative (R-1 finding 4).
const REQUESTED_MECHANISM: &str = "installed_on_request";

/// Taps whose packages are the provider's own catalogue rather than a
/// third-party origin.
const CORE_TAPS: &[&str] = &["homebrew/core", "homebrew/cask"];

const ARGV_VERSION: &[&str] = &["--version"];
const ARGV_PREFIX: &[&str] = &["--prefix"];
const ARGV_CELLAR: &[&str] = &["--cellar"];
const ARGV_CASKROOM: &[&str] = &["--caskroom"];
const ARGV_CACHE: &[&str] = &["--cache"];
const ARGV_CONFIG: &[&str] = &["config"];
const ARGV_INFO: &[&str] = &["info", "--json=v2", "--installed"];
const ARGV_PINNED: &[&str] = &["list", "--pinned"];

/// Every argument vector this adapter may run. Nothing outside this list
/// reaches the executor, and no value read from provider output or from a
/// user ever becomes an element of one
/// (`04-migration-and-safety.md`, "Process execution").
pub const ALLOWED_ARGV: &[&[&str]] = &[
    ARGV_VERSION,
    ARGV_PREFIX,
    ARGV_CELLAR,
    ARGV_CASKROOM,
    ARGV_CACHE,
    ARGV_CONFIG,
    ARGV_INFO,
    ARGV_PINNED,
];

fn changed(detail: impl Into<String>) -> DomainError {
    DomainError::new(ErrorId::ProviderOutputChanged, detail)
}

/// Returns the first candidate prefix that holds an executable, or `None`.
///
/// The caller passes the candidates in preference order — on macOS
/// `/opt/homebrew` then `/usr/local`. Resolving the prefix here and the
/// executable from it is what keeps a shadowing entry on `PATH` out of the
/// picture.
pub fn detect_prefix(candidates: &[PathBuf], filesystem: FilesystemScope) -> Option<PathBuf> {
    if !filesystem.is_this_machine() {
        // The recording is the machine, and this host's disk says nothing
        // about which prefix it has. The caller supplies one (INV-15, F-25).
        return None;
    }
    candidates
        .iter()
        .find(|prefix| executable_in(prefix).is_file())
        .cloned()
}

fn executable_in(prefix: &Path) -> PathBuf {
    prefix.join(EXECUTABLE_RELATIVE)
}

/// The variables this adapter adds to a sanitised environment, and the only
/// ones a snapshot may state for Homebrew. Named here so the reader that
/// validates a captured environment and the builder that makes one cannot
/// disagree about the list.
pub const QUIETING_KEYS: &[&str] = &[
    "HOMEBREW_NO_AUTO_UPDATE",
    "HOMEBREW_NO_ANALYTICS",
    "HOMEBREW_NO_ENV_HINTS",
    "HOMEBREW_NO_INSTALL_CLEANUP",
    "HOMEBREW_NO_COLOR",
    "HOMEBREW_NO_EMOJI",
];

/// The environment every invocation of this provider runs in.
///
/// `HOMEBREW_NO_AUTO_UPDATE` is the load-bearing one: it keeps a read from
/// updating Homebrew (R-1 finding 2). The rest suppress analytics, hints,
/// colour and emoji so the output parses stably. Nothing is inherited —
/// [`sanitised_environment`] builds the whole environment, drops
/// build-affecting variables, and pins the locale (INV-10).
///
/// This is what the tool asks for, not what `brew` is guaranteed to run
/// with (R-15, Homebrew 7.0.4, read 2026-10-02): a read still downloads the
/// API data when Homebrew's cache is missing, and a `brew.env` in
/// `/etc/homebrew/`, the prefix or the user's configuration directory is
/// exported after this environment, so a line there outranks or deletes any
/// of these keys. No variable turns the files off, so capture records which
/// of those files set a reviewed key ([`crate::configuration`]), and the
/// execution gate refuses a mutating step against them (DP-027).
pub fn quiet_environment(prefix: &Path, home: Option<PathBuf>) -> BTreeMap<String, String> {
    let mut extra = BTreeMap::new();
    for key in QUIETING_KEYS {
        extra.insert((*key).to_string(), "1".to_string());
    }
    sanitised_environment(&EnvironmentSpec {
        leading_path: vec![prefix.join("bin")],
        home,
        extra,
    })
}

/// Parses the first line of `--version`, which reads `Homebrew 6.0.21`.
fn parse_version_line(stdout: &str) -> Result<Version, DomainError> {
    let first = stdout.lines().next().unwrap_or_default().trim();
    let rest = first
        .strip_prefix("Homebrew")
        .ok_or_else(|| {
            changed(format!(
                "`--version` first line is not recognisable: {first:?}"
            ))
        })?
        .trim();
    if rest.is_empty() {
        return Err(changed(format!(
            "`--version` first line carries no version: {first:?}"
        )));
    }
    Ok(Version::new(rest))
}

/// Parses the `Key: value` lines of `config` into a map.
fn parse_config(stdout: &str) -> BTreeMap<String, String> {
    stdout
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

/// Reads the architecture out of `config`'s `macOS: 26.6.2-x86_64` line.
///
/// A missing key is a shape change and fails loudly. A value whose
/// architecture half is not one the domain recognises yields `None`: the
/// domain models architecture as optional, and `None` states honestly that
/// it was not determined rather than defaulting to either answer.
fn architecture_from_config(
    config: &BTreeMap<String, String>,
) -> Result<Option<Architecture>, DomainError> {
    let value = config
        .get("macOS")
        .ok_or_else(|| changed("`config` output has no `macOS` line"))?;
    Ok(value
        .rsplit_once('-')
        .and_then(|(_, arch)| Architecture::parse(arch)))
}

/// The inventory and the capability table, built together from one parse.
///
/// The capability table survives a document this adapter cannot parse: a
/// provider whose output changed degrades with a named reason rather than
/// crashing (`02-domain-model.md`, "Capabilities are detected").
#[derive(Clone, Debug)]
struct Analysis {
    inventory: Result<ProviderInventory, DomainError>,
    capabilities: ProviderCapabilities,
}

/// The Homebrew read adapter.
///
/// The runner is borrowed rather than owned so a test can keep hold of a
/// [`crate::process::ReplayRunner`] and assert afterwards which recorded
/// invocations the adapter actually consulted.
pub struct HomebrewAdapter<'r> {
    runner: &'r dyn ProcessRunner,
    id: ProviderId,
    /// Whether this host's disk describes the machine being read. Separate
    /// from `runner.is_replay()`, which answers a different question; see
    /// [`crate::filesystem`] (F-25).
    filesystem: FilesystemScope,
    prefix: PathBuf,
    home: Option<PathBuf>,
    platform_tags: Vec<String>,
    environment: BTreeMap<String, String>,
    runs: RefCell<BTreeMap<Vec<String>, Invocation>>,
    analysis: RefCell<Option<Analysis>>,
    detection: RefCell<Option<ProviderDetection>>,
}

impl<'r> HomebrewAdapter<'r> {
    /// Builds an adapter for an installation at `prefix`.
    ///
    /// `home` is where the provider keeps its metadata cache; `None` omits
    /// `HOME` from the invocation environment entirely and makes the
    /// offline-metadata capability `Unknown` rather than guessed.
    /// `platform_tags` are the artifact tags that satisfy this machine, most
    /// specific first (for example `tahoe`, `sequoia`, `sonoma`, `all`);
    /// they come from dated policy data, never from this adapter (R-2).
    /// `filesystem` says whether this host's disk describes the machine being
    /// read; where it does not, every filesystem answer is the unknown one
    /// (INV-15, F-25).
    pub fn new(
        runner: &'r dyn ProcessRunner,
        prefix: PathBuf,
        home: Option<PathBuf>,
        platform_tags: Vec<String>,
        filesystem: FilesystemScope,
    ) -> Self {
        let environment = quiet_environment(&prefix, home.clone());
        Self {
            runner,
            id: ProviderId::new(PROVIDER_ID),
            filesystem,
            prefix,
            home,
            platform_tags,
            environment,
            runs: RefCell::new(BTreeMap::new()),
            analysis: RefCell::new(None),
            detection: RefCell::new(None),
        }
    }

    /// The executable this adapter runs, resolved from the prefix.
    pub fn executable(&self) -> PathBuf {
        executable_in(&self.prefix)
    }

    /// Runs one allow-listed command, at most once per analysis.
    ///
    /// The allow-list is checked at run time, not with a `debug_assert!`: a
    /// release build is exactly where the check has to hold, and an assertion
    /// that compiles out of the shipped binary is a guarantee the shipped
    /// binary does not make (`04-migration-and-safety.md`, "Process
    /// execution"). Reaching this with something else is a programming error,
    /// so it is `Internal` rather than a provider fault.
    fn run(&self, argv: &[&str]) -> Result<Invocation, DomainError> {
        if !ALLOWED_ARGV.contains(&argv) {
            return Err(DomainError::new(
                ErrorId::Internal,
                format!("homebrew: {argv:?} is not an allow-listed argument vector"),
            ));
        }
        let key: Vec<String> = argv.iter().map(|a| (*a).to_string()).collect();
        if let Some(hit) = self.runs.borrow().get(&key) {
            return Ok(hit.clone());
        }
        let spec = CommandSpec::provider(
            &self.prefix,
            EXECUTABLE_RELATIVE,
            key.clone(),
            self.environment.clone(),
        );
        let invocation = self.runner.run(&spec)?;
        self.runs.borrow_mut().insert(key, invocation.clone());
        Ok(invocation)
    }

    /// Runs a command that answers with one line, and requires it to succeed.
    fn one_line(&self, argv: &[&str], field: &str) -> Result<String, DomainError> {
        let invocation = self.run(argv)?;
        invocation.require_success()?;
        let value = invocation.stdout.trim();
        if value.is_empty() {
            return Err(changed(format!("`{field}` answered with nothing")));
        }
        Ok(value.to_string())
    }

    /// Runs a command that answers with one absolute path.
    fn one_path(&self, argv: &[&str], field: &str) -> Result<PathBuf, DomainError> {
        let value = self.one_line(argv, field)?;
        let path = PathBuf::from(&value);
        if !path.is_absolute() {
            return Err(changed(format!(
                "`{field}` answered with a path that is not absolute: {value:?}"
            )));
        }
        Ok(path)
    }

    fn cellar(&self) -> Result<PathBuf, DomainError> {
        self.one_path(ARGV_CELLAR, "--cellar")
    }

    fn caskroom(&self) -> Result<PathBuf, DomainError> {
        self.one_path(ARGV_CASKROOM, "--caskroom")
    }

    fn config(&self) -> Result<BTreeMap<String, String>, DomainError> {
        let invocation = self.run(ARGV_CONFIG)?;
        invocation.require_success()?;
        Ok(parse_config(&invocation.stdout))
    }

    /// The names the user holds at their current version.
    fn pinned(&self) -> Result<BTreeSet<String>, DomainError> {
        let invocation = self.run(ARGV_PINNED)?;
        invocation.require_success()?;
        Ok(invocation
            .stdout
            .split_whitespace()
            .map(ToString::to_string)
            .collect())
    }

    /// The path whose presence proves the provider can answer metadata
    /// questions with no network (R-2 finding 2).
    fn offline_index_path(&self) -> Option<PathBuf> {
        let home = self.home.as_ref()?;
        let tag = self.platform_tags.first()?;
        Some(
            home.join("Library/Caches/Homebrew/api/internal")
                .join(format!("packages.{tag}.jws.json")),
        )
    }

    /// Parses the inventory document and builds the capability table, once.
    fn analysis(&self) -> Result<Analysis, DomainError> {
        if let Some(hit) = self.analysis.borrow().as_ref() {
            return Ok(hit.clone());
        }
        let invocation = self.run(ARGV_INFO)?;
        invocation.require_success()?;
        let parsed = parse_installed_json(&invocation.stdout);
        let capabilities = self.capability_table(parsed.as_ref());
        let inventory = match &parsed {
            Ok(document) => self.build_inventory(document),
            Err(error) => Err(error.clone()),
        };
        let analysis = Analysis {
            inventory,
            capabilities,
        };
        *self.analysis.borrow_mut() = Some(analysis.clone());
        Ok(analysis)
    }

    /// Detects what this installation can do. Nothing here is assumed from
    /// the version string.
    fn capability_table(&self, parsed: Result<&Parsed, &DomainError>) -> ProviderCapabilities {
        let mut capabilities = BTreeMap::new();
        let document = match parsed {
            Ok(document) => {
                capabilities.insert(
                    Capability::MachineReadableInventory,
                    CapabilityState::Available,
                );
                document
            }
            Err(error) => {
                let unavailable = CapabilityState::Unavailable {
                    reason: error.id.message_id().to_string(),
                };
                for capability in [
                    Capability::MachineReadableInventory,
                    Capability::RequestedRecord,
                    Capability::Dependencies,
                    Capability::ReverseDependencies,
                    Capability::ArtifactAvailability,
                    Capability::DeprecationState,
                    Capability::ServiceMetadata,
                ] {
                    capabilities.insert(capability, unavailable.clone());
                }
                capabilities.insert(Capability::FileOwnership, CapabilityState::Unknown);
                capabilities.insert(Capability::OfflineMetadata, self.offline_metadata_state());
                // Not `Unavailable`, even though every capability beside it is.
                // A read that failed does not say whether it failed because the
                // user may not read, or because there was nothing to read: a
                // missing binary and a permission denial arrive here the same
                // way. Claiming "this read needs elevation" from that is the
                // guess INV-06 forbids, so the honest answer is that nothing
                // was established (F-35).
                capabilities.insert(Capability::UnprivilegedRead, CapabilityState::Unknown);
                return ProviderCapabilities { capabilities };
            }
        };
        let observed = |seen: bool| {
            if seen {
                CapabilityState::Available
            } else {
                CapabilityState::Unknown
            }
        };
        capabilities.insert(
            Capability::RequestedRecord,
            observed(document.saw(Observed::RequestedRecord)),
        );
        let dependencies = observed(document.saw(Observed::DependencyRecords));
        capabilities.insert(Capability::Dependencies, dependencies.clone());
        capabilities.insert(Capability::ReverseDependencies, dependencies);
        // `bottle` is a required key, so a parsed document always carries it.
        capabilities.insert(Capability::ArtifactAvailability, CapabilityState::Available);
        capabilities.insert(
            Capability::DeprecationState,
            observed(document.saw(Observed::DeprecationState)),
        );
        // A machine on which no package defines a background service cannot
        // demonstrate that the field exists, so the honest answer is Unknown.
        capabilities.insert(
            Capability::ServiceMetadata,
            observed(document.saw(Observed::ServiceMetadata)),
        );
        // Homebrew has no "which package owns this file" query (R-1).
        capabilities.insert(Capability::FileOwnership, CapabilityState::Unknown);
        // The read that produced `document` ran as the invoking user and came
        // back, which is the observation. Note it is `info --json=v2
        // --installed` and the verb it relaxes is `list --versions`: same
        // binary, same prefix, same user, so the inference is sound — but it is
        // an inference here, where on MacPorts the two argvs are identical. No new
        // probe, nothing stat-ed, no permission inspected — INV-20's "detection
        // wins over stored knowledge" answered from a read already performed
        // (F-24, F-35).
        capabilities.insert(Capability::UnprivilegedRead, CapabilityState::Available);
        capabilities.insert(Capability::OfflineMetadata, self.offline_metadata_state());
        ProviderCapabilities { capabilities }
    }

    fn offline_metadata_state(&self) -> CapabilityState {
        if !self.filesystem.is_this_machine() {
            // Stat'ing the replaying host's cache would answer a question
            // about the wrong machine, and the answer would differ between
            // two hosts replaying one recording (INV-15, F-25).
            return CapabilityState::Unknown;
        }
        match self.offline_index_path() {
            None => CapabilityState::Unknown,
            Some(path) if path.is_file() => CapabilityState::Available,
            Some(path) => CapabilityState::Unavailable {
                reason: format!("no cached index at {}", path.display()),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Mapping the document onto the domain.
// ---------------------------------------------------------------------------

impl HomebrewAdapter<'_> {
    fn build_inventory(&self, document: &Parsed) -> Result<ProviderInventory, DomainError> {
        let cellar = self.cellar()?;
        let caskroom = self.caskroom()?;
        let pinned = self.pinned()?;
        let formula_names: BTreeSet<String> =
            document.formulae.iter().map(|f| f.name.clone()).collect();

        let mut installed = Vec::new();
        let mut requested = BTreeSet::new();
        let mut metadata = BTreeMap::new();
        let mut availability = BTreeMap::new();

        for formula in &document.formulae {
            self.push_formula_rows(formula, &cellar, &mut installed, &mut requested);
            metadata.insert(formula.name.clone(), formula_metadata(formula, &pinned));
            availability.insert(formula.name.clone(), self.formula_availability(formula));
        }
        for cask in &document.casks {
            installed.push(cask_row(cask, &caskroom, self.filesystem));
            metadata.insert(cask.token.clone(), cask_metadata(cask));
            availability.insert(cask.token.clone(), cask_availability());
        }
        installed.sort();

        Ok(ProviderInventory {
            installed,
            requested: RequestedSet {
                names: requested,
                mechanism: REQUESTED_MECHANISM.to_string(),
            },
            dependencies: dependency_edges(document, &formula_names),
            metadata,
            availability,
        })
    }

    fn push_formula_rows(
        &self,
        formula: &Formula,
        cellar: &Path,
        installed: &mut Vec<InstalledPackage>,
        requested: &mut BTreeSet<String>,
    ) {
        let active_version = formula.active_keg().map(|keg| keg.version.clone());
        for keg in &formula.installed {
            if keg.installed_on_request == Some(true) {
                requested.insert(formula.name.clone());
            }
            installed.push(InstalledPackage {
                identity: PackageIdentity::new(self.id.clone(), formula.name.clone()),
                kind: PackageKind::Package,
                version: Version::new(keg.version.clone()),
                revision: (formula.revision != 0).then(|| formula.revision.to_string()),
                variants: keg.used_options.iter().cloned().collect(),
                active: active_version.as_deref() == Some(keg.version.as_str()),
                requested: keg.installed_on_request,
                installed_from_prebuilt: keg.poured_from_bottle,
                install_path: Some(cellar.join(&formula.name).join(&keg.version)),
            });
        }
    }

    /// Assesses one formula against the tags the caller says this machine
    /// accepts. Disabled outranks deprecated, which outranks the artifact
    /// question: a package the provider refuses to place is not made
    /// installable by the existence of an artifact, but the artifact fact is
    /// kept in the evidence so nothing is lost.
    fn formula_availability(&self, formula: &Formula) -> AvailabilityAssessment {
        let tags = formula.bottle_tags();
        let matched = self
            .platform_tags
            .iter()
            .find(|wanted| tags.iter().any(|tag| tag == &wanted.as_str()));
        let artifact = AvailabilityEvidence {
            source: "bottle.stable.files".to_string(),
            detail: match matched {
                Some(tag) => tag.clone(),
                None if tags.is_empty() => "no tag offered for this machine".to_string(),
                None => format!("tags offered: {}", tags.join(", ")),
            },
            offline: true,
        };
        if formula.disabled == Some(true) {
            return assessment(
                ArtifactAvailability::Disabled,
                vec![
                    state_evidence("disabled", formula.disable_reason.as_deref()),
                    artifact,
                ],
            );
        }
        if formula.deprecated == Some(true) {
            return assessment(
                ArtifactAvailability::Deprecated,
                vec![
                    state_evidence("deprecated", formula.deprecation_reason.as_deref()),
                    artifact,
                ],
            );
        }
        if matched.is_some() {
            return assessment(ArtifactAvailability::BinaryAvailable, vec![artifact]);
        }
        assessment(
            ArtifactAvailability::SourceBuildRequired {
                reason: SourceBuildReason::NoArtifactForThisPlatform,
            },
            vec![artifact, cached_index_caveat()],
        )
    }
}

fn assessment(
    availability: ArtifactAvailability,
    evidence: Vec<AvailabilityEvidence>,
) -> AvailabilityAssessment {
    AvailabilityAssessment {
        availability,
        evidence,
        // The whole-inventory answer is served from the provider's own
        // metadata cache with no network access (R-1 finding 3).
        cached: true,
    }
}

fn state_evidence(source: &str, reason: Option<&str>) -> AvailabilityEvidence {
    AvailabilityEvidence {
        source: source.to_string(),
        detail: reason.unwrap_or("no reason given").to_string(),
        offline: true,
    }
}

/// The caveat that must travel with every "would compile" answer: the
/// document is generated for one platform tag, so it represents the two
/// distinct states identically (R-1 finding 6, R-2 finding 5).
fn cached_index_caveat() -> AvailabilityEvidence {
    AvailabilityEvidence {
        source: "cached metadata index".to_string(),
        detail: "the index is generated for one platform tag, so it cannot separate \
                 `no artifact published anywhere` from `none published for this platform`; \
                 distinguishing them needs the tap on disk or the network"
            .to_string(),
        offline: true,
    }
}

/// A cask is a vendor-built, vendor-signed application that the provider
/// places and then steps aside from (R-1 finding 10). The prebuilt question
/// this adapter models is about the *provider* building, so the honest
/// answer for a cask is `Unknown`, and casks are out of migration scope.
fn cask_availability() -> AvailabilityAssessment {
    AvailabilityAssessment::unknown("cask", "cask: prebuilt by vendor, out of migration scope")
}

/// The bytes of a cask receipt this reader will look at.
///
/// Provider output is untrusted input and a receipt is a file inside a
/// provider's prefix, which is a directory this tool does not own. The one
/// observed here was 1268 bytes; a megabyte is four hundred times that and
/// still cheap to refuse.
const MAX_RECEIPT_BYTES: u64 = 1024 * 1024;

/// The cask's own record of whether the user asked for it.
///
/// `brew info --json=v2 --cask` carries no intent field — 41 keys on Homebrew
/// 6.0.21, re-verified 2026-09-04 — but Homebrew writes `installed_on_request`
/// into `<caskroom>/<token>/.metadata/INSTALL_RECEIPT.json`, and 7 of the 9 casks
/// on the machine this was measured on carry one (R-12, DP-012).
///
/// **Every failure answers `None`, which is `UNKNOWN` upstream.** Absent, a
/// symlink or a FIFO, hardlinked, swapped between the check and the open, grown
/// past the cap while being read, unreadable, not JSON, or JSON whose
/// `installed_on_request` is missing or is not a boolean: none of those is
/// evidence that the user did not ask for the package, and folding them into
/// `false` would make a dependency out of a file this reader could not open.
pub fn cask_requested(caskroom: &Path, token: &str, filesystem: FilesystemScope) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
    if !filesystem.is_this_machine() {
        // The recording is the machine, and it carries no receipt: a
        // filesystem answer here would describe the replaying host (INV-15).
        return None;
    }
    let path = caskroom
        .join(token)
        .join(".metadata")
        .join("INSTALL_RECEIPT.json");

    // Refuse a non-regular file *before* opening. Two of the three reasons for
    // this check are permanent; the third has moved, and saying which is the
    // difference between a guard someone maintains and one someone deletes.
    //
    //   1. `before` is the dev/ino baseline the identity check below compares
    //      against. Remove this line and the swap detector goes with it.
    //   2. A character device's open can have side effects. Not opening a
    //      non-regular object at all is stronger than opening and rejecting.
    //   3. It used to be the *only* thing standing between `analyze` and a
    //      permanent hang on a FIFO. It is not any more: this caller passes
    //      `O_NONBLOCK`, so the open returns whatever is there and the
    //      regular-file test inside `read_bounded` refuses it. The blocking
    //      argument now belongs to the flags (F-31).
    let before = std::fs::symlink_metadata(&path).ok()?;
    if !before.is_file() {
        return None;
    }

    // From here the name is resolved exactly once, by `read_bounded`. The first
    // version of this function stat'd the path and then opened it again by
    // name — the shape that reader's documentation already called wrong, three
    // modules away, in this same crate. Prose did not reach here; the shared
    // function does (F-30).
    //
    // Both checks below run on the *opened* handle, before any byte is read.
    let text = crate::persistence::bounded_read::read_bounded(
        &path,
        MAX_RECEIPT_BYTES,
        // The pre-check above refuses a symlink present when it looks; this
        // covers the window between that look and the open, for the final
        // component only (F-31).
        crate::persistence::bounded_read::OpenPolicy::RefuseSymlink,
        |opened| {
            if (opened.dev(), opened.ino()) != (before.dev(), before.ino()) {
                // Not the object the pre-check inspected: something was swapped
                // in between the two calls.
                return Err("the receipt changed between the pre-check and the open");
            }
            if opened.nlink() > 1 {
                // More than one name for these bytes is not something a receipt
                // needs, and is a signal the file is not what it appears to be.
                return Err("the receipt has more than one name");
            }
            Ok(())
        },
    )
    .ok()?;

    // Every refusal is `None`, deliberately: an unreadable receipt is not
    // evidence that the user did not ask for the package, so it must not
    // become `false`. The reasons are distinguished inside `read_bounded` for
    // the caller that reports them; this one has nothing to report them to.
    let document: serde_json::Value = serde_json::from_str(&text).ok()?;
    document.get("installed_on_request")?.as_bool()
}

fn cask_row(cask: &Cask, caskroom: &Path, filesystem: FilesystemScope) -> InstalledPackage {
    InstalledPackage {
        identity: PackageIdentity::new(ProviderId::new(PROVIDER_ID), cask.token.clone()),
        kind: PackageKind::GuiApp,
        version: Version::new(cask.installed.clone()),
        revision: None,
        variants: BTreeSet::new(),
        active: true,
        // The cask JSON carries no intent field: 41 keys on Homebrew 6.0.21,
        // none of them `installed_on_request` (R-1 finding 4, re-verified
        // 2026-09-04). Homebrew records it on disk instead, and since DP-012
        // this reads it — `None` where there is no receipt, which is `UNKNOWN`
        // upstream and never an inferred `TOP_LEVEL`.
        requested: cask_requested(caskroom, &cask.token, filesystem),
        // Nor any record of how it was built; the vendor built it.
        installed_from_prebuilt: None,
        install_path: Some(caskroom.join(&cask.token).join(&cask.installed)),
    }
}

fn origin(tap: Option<&String>) -> Option<String> {
    tap.filter(|t| !CORE_TAPS.contains(&t.as_str())).cloned()
}

fn state_reason(flag: Option<bool>, reason: Option<&String>) -> Option<String> {
    if flag == Some(true) {
        Some(
            reason
                .cloned()
                .unwrap_or_else(|| "no reason given".to_string()),
        )
    } else {
        None
    }
}

fn formula_metadata(formula: &Formula, pinned: &BTreeSet<String>) -> PackageMetadata {
    PackageMetadata {
        description: formula.desc.clone(),
        homepage: formula.homepage.clone(),
        license: formula.license.clone(),
        available_version: formula.versions.stable.clone().map(Version::new),
        origin: origin(formula.tap.as_ref()),
        deprecated: state_reason(formula.deprecated, formula.deprecation_reason.as_ref()),
        disabled: state_reason(formula.disabled, formula.disable_reason.as_ref()),
        replaced_by: formula.replaced_by.clone(),
        pinned: formula.pinned || pinned.contains(&formula.name),
        not_on_default_path_reason: keg_only_reason(formula),
        defines_service: formula.has_service,
        executables: BTreeSet::new(),
        aliases: formula.aliases.iter().cloned().collect(),
        versioned_siblings: formula.versioned_formulae.iter().cloned().collect(),
        notes: formula.caveats.clone(),
    }
}

/// The prose reason when the provider gives one, and the symbol it uses
/// otherwise: 14 of the 19 keg-only formulae observed carry an empty
/// `explanation` and only a `reason` such as `:provided_by_macos` (R-6
/// finding 9), so reading `explanation` alone loses the answer.
fn keg_only_reason(formula: &Formula) -> Option<String> {
    if !formula.keg_only {
        return None;
    }
    let reason = formula.keg_only_reason.as_ref();
    let explanation = reason.map(|r| r.explanation.trim()).unwrap_or_default();
    if !explanation.is_empty() {
        return Some(explanation.to_string());
    }
    let symbol = reason.map(|r| r.reason.trim()).unwrap_or_default();
    Some(if symbol.is_empty() {
        "kept off the default path, no reason given".to_string()
    } else {
        symbol.to_string()
    })
}

fn cask_metadata(cask: &Cask) -> PackageMetadata {
    let notes = match cask.auto_updates {
        Some(true) => Some("the vendor updates this application itself".to_string()),
        _ => None,
    };
    PackageMetadata {
        description: cask.desc.clone(),
        homepage: cask.homepage.clone(),
        license: None,
        available_version: cask.version.clone().map(Version::new),
        origin: origin(cask.tap.as_ref()),
        deprecated: state_reason(cask.deprecated, cask.deprecation_reason.as_ref()),
        disabled: state_reason(cask.disabled, cask.disable_reason.as_ref()),
        replaced_by: None,
        pinned: cask.pinned,
        not_on_default_path_reason: None,
        defines_service: false,
        executables: BTreeSet::new(),
        aliases: BTreeSet::new(),
        versioned_siblings: BTreeSet::new(),
        notes,
    }
}

/// Edges between packages that are actually present. The runtime record of
/// the active keg is preferred over the formula's declared list, because it
/// is what this machine resolved; the declared list is the fallback for a
/// keg written before the record existed.
fn dependency_edges(document: &Parsed, present: &BTreeSet<String>) -> Vec<DependencyEdge> {
    let mut edges = Vec::new();
    for formula in &document.formulae {
        let runtime: BTreeSet<String> = formula
            .active_keg()
            .map(|keg| {
                keg.runtime_dependencies
                    .iter()
                    .map(|d| d.full_name.clone())
                    .collect()
            })
            .filter(|set: &BTreeSet<String>| !set.is_empty())
            .unwrap_or_else(|| formula.dependencies.iter().cloned().collect());
        for name in runtime.iter().filter(|n| present.contains(*n)) {
            edges.push(DependencyEdge {
                from: formula.name.clone(),
                to: name.clone(),
                build_only: false,
            });
        }
        for name in &formula.build_dependencies {
            if present.contains(name) && !runtime.contains(name) {
                edges.push(DependencyEdge {
                    from: formula.name.clone(),
                    to: name.clone(),
                    build_only: true,
                });
            }
        }
    }
    edges.sort();
    edges.dedup();
    edges
}

// ---------------------------------------------------------------------------
// The read contract.
// ---------------------------------------------------------------------------

impl ProviderRead for HomebrewAdapter<'_> {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    fn detect(&self) -> Result<ProviderDetection, DomainError> {
        if let Some(hit) = self.detection.borrow().as_ref() {
            return Ok(hit.clone());
        }
        let detection = self.detect_uncached()?;
        *self.detection.borrow_mut() = Some(detection.clone());
        Ok(detection)
    }

    fn capabilities(&self) -> Result<ProviderCapabilities, DomainError> {
        Ok(self.analysis()?.capabilities)
    }

    fn inventory(&self) -> Result<ProviderInventory, DomainError> {
        self.analysis()?.inventory
    }

    fn artifact_availability(
        &self,
        pkg: &PackageIdentity,
        _machine: &Machine,
    ) -> Result<AvailabilityAssessment, DomainError> {
        // The machine is already accounted for: the caller supplied the
        // platform tags that satisfy it when the adapter was built (R-2).
        Ok(self
            .inventory()?
            .availability
            .get(&pkg.name)
            .cloned()
            .unwrap_or_else(|| {
                AvailabilityAssessment::unknown(
                    "inventory",
                    "not among the packages this provider reports as present",
                )
            }))
    }

    fn resolution(&self, executable: &Path) -> Result<Resolution, DomainError> {
        let name = executable
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let unresolved = || Resolution {
            executable: name.clone(),
            path: Some(executable.to_path_buf()),
            canonical_path: None,
            owner: ExecutableOwner::Unknown,
            authoritative: false,
        };
        // Only following the symlink needs this host's disk. The Cellar and
        // the Caskroom come from the recording, so where the disk is not the
        // machine the classification is still answerable and is still done -
        // refusing it threw away recorded evidence and made this adapter
        // degrade differently from its peer, which is the INV-17 shape the
        // F-25 review caught.
        let canonical = if self.filesystem.is_this_machine() {
            let Ok(resolved) = std::fs::canonicalize(executable) else {
                return Ok(unresolved());
            };
            Some(resolved)
        } else {
            None
        };
        let target = canonical.as_deref().unwrap_or(executable);
        let owner = self.owner_of(target)?;
        Ok(Resolution {
            executable: name,
            path: Some(executable.to_path_buf()),
            // A Cellar hit is authoritative only when the link was actually
            // followed. Computed from the path as written it is a heuristic,
            // and `analysis::resolution_for` ranks the two differently on
            // purpose.
            authoritative: canonical.is_some() && matches!(owner, ExecutableOwner::Provider { .. }),
            canonical_path: canonical.filter(|c| c != executable),
            owner,
        })
    }

    fn environment_info(&self) -> Result<BTreeMap<String, String>, DomainError> {
        let mut info = BTreeMap::new();
        info.insert(
            "prefix".to_string(),
            self.one_path(ARGV_PREFIX, "--prefix")?
                .display()
                .to_string(),
        );
        info.insert("cellar".to_string(), self.cellar()?.display().to_string());
        info.insert(
            "caskroom".to_string(),
            self.caskroom()?.display().to_string(),
        );
        info.insert("cache".to_string(), self.one_line(ARGV_CACHE, "--cache")?);
        info.insert(
            "bin".to_string(),
            self.prefix.join("bin").display().to_string(),
        );
        Ok(info)
    }

    fn snapshot(&self) -> Result<ProviderSnapshot, DomainError> {
        let detection = self.detect()?;
        let present = matches!(detection.presence, ProviderPresence::Present);
        Ok(ProviderSnapshot {
            detection,
            capabilities: if present {
                self.capabilities()?
            } else {
                // Nine unknowns, not an empty table. A provider that is not
                // here cannot be asked what it can do, and `Unknown` is what
                // "could not determine" means; an empty table says the
                // question was never put, which is a different claim and not
                // one this adapter is entitled to make.
                //
                // It is also what the MacPorts adapter has always done. The
                // two are peer providers and this was the one place they
                // disagreed about the shape of "absent" - invisible until a
                // rule about documents had to be written around it, and the
                // rule it forced left INV-19's refusal bypassable by deleting
                // the table rather than a key (F-29, spec-guardian
                // 2026-09-18).
                ProviderCapabilities {
                    capabilities: Capability::all()
                        .iter()
                        .map(|c| (*c, CapabilityState::Unknown))
                        .collect(),
                }
            },
            inventory: if present {
                self.inventory()?
            } else {
                ProviderInventory::default()
            },
            environment: if present {
                self.environment_info()?
            } else {
                BTreeMap::new()
            },
            invocation_environment: self.environment.clone(),
            resolutions: BTreeMap::new(),
            // Only for a provider that is here: an absent one is never
            // invoked, so nothing of its configuration can outrank anything.
            configuration: if present {
                crate::configuration::read(
                    PROVIDER_ID,
                    &self.prefix,
                    self.home.as_deref(),
                    self.filesystem,
                )
            } else {
                macportsbrew_domain::ProviderConfiguration::NotRecorded
            },
        })
    }
}

impl HomebrewAdapter<'_> {
    fn detect_uncached(&self) -> Result<ProviderDetection, DomainError> {
        // Where this host's disk is the machine, an absent executable is
        // answered without spawning anything. Where it is not, the recording
        // is the machine and the local filesystem is not consulted at all.
        // This asks the filesystem question, not `runner.is_replay()`: a
        // contract test replays subprocesses while reading a scratch tree
        // that really is its subject (F-25).
        if self.filesystem.is_this_machine() && !self.executable().is_file() {
            return Ok(self.absent());
        }
        let invocation = self.run(ARGV_VERSION)?;
        match &invocation.status {
            ExitStatus::NotStarted { .. } => return Ok(self.absent()),
            status if !status.success() => {
                return Ok(ProviderDetection {
                    provider: self.id.clone(),
                    presence: ProviderPresence::Broken {
                        reason: ErrorId::ProviderUnhealthy.message_id().to_string(),
                    },
                    version: None,
                    prefix: Some(self.prefix.clone()),
                    executable: Some(self.executable()),
                    architecture: None,
                    // Detection never redacts - `capture` does, and is the only
                    // thing that may say so (F-49, SE-080).
                    prefix_redacted: false,
                });
            }
            _ => {}
        }
        Ok(ProviderDetection {
            provider: self.id.clone(),
            presence: ProviderPresence::Present,
            version: Some(parse_version_line(&invocation.stdout)?),
            prefix: Some(self.one_path(ARGV_PREFIX, "--prefix")?),
            executable: Some(self.executable()),
            architecture: architecture_from_config(&self.config()?)?,
            prefix_redacted: false,
        })
    }

    fn absent(&self) -> ProviderDetection {
        ProviderDetection {
            provider: self.id.clone(),
            presence: ProviderPresence::Absent,
            version: None,
            prefix: None,
            executable: None,
            architecture: None,
            prefix_redacted: false,
        }
    }

    /// Ownership from the *resolved* path. A path merely under the prefix
    /// proves nothing — `/usr/local/bin` holds files no formula owns (R-6
    /// finding 14) — so only a path inside the Cellar or the Caskroom names
    /// a package, and only that answer is marked authoritative.
    fn owner_of(&self, canonical: &Path) -> Result<ExecutableOwner, DomainError> {
        for root in [self.cellar()?, self.caskroom()?] {
            let root = if self.filesystem.is_this_machine() {
                std::fs::canonicalize(&root).unwrap_or(root)
            } else {
                root
            };
            if let Ok(rest) = canonical.strip_prefix(&root) {
                let package = rest
                    .components()
                    .next()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned());
                return Ok(ExecutableOwner::Provider {
                    provider: self.id.clone(),
                    package,
                });
            }
        }
        let prefix = if self.filesystem.is_this_machine() {
            std::fs::canonicalize(&self.prefix).unwrap_or_else(|_| self.prefix.clone())
        } else {
            self.prefix.clone()
        };
        if canonical.starts_with(&prefix) {
            return Ok(ExecutableOwner::External);
        }
        Ok(ExecutableOwner::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch Caskroom holding one cask, with whatever receipt bytes the
    /// case wants — or none.
    fn caskroom_with(token: &str, receipt: Option<&[u8]>) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "mpb-caskroom-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let meta = root.join(token).join(".metadata");
        std::fs::create_dir_all(&meta).expect("scratch caskroom");
        if let Some(bytes) = receipt {
            std::fs::write(meta.join("INSTALL_RECEIPT.json"), bytes).expect("receipt");
        }
        root
    }

    #[test]
    fn a_cask_receipt_answers_the_intent_the_json_does_not_carry() {
        // DP-012 / R-12. `brew info --json=v2 --cask` has no
        // `installed_on_request`; the on-disk receipt does, and 7 of the 9
        // casks measured carried one.
        for (recorded, expected) in [("true", Some(true)), ("false", Some(false))] {
            let body = format!(r#"{{"installed_on_request": {recorded}}}"#);
            let root = caskroom_with("meld", Some(body.as_bytes()));
            assert_eq!(
                cask_requested(&root, "meld", FilesystemScope::ThisMachine),
                expected,
                "a receipt saying {recorded}"
            );
            std::fs::remove_dir_all(&root).ok();
        }
    }

    #[test]
    fn every_way_a_receipt_can_fail_is_unknown_and_never_false() {
        // `false` would mean "the user did not ask for this", which makes a
        // dependency out of a file this reader could not open. Each of these
        // must be `None`, which is `UNKNOWN` upstream.
        let oversized = vec![b' '; usize::try_from(MAX_RECEIPT_BYTES + 1).expect("64-bit")];
        let cases: Vec<(&str, Option<&[u8]>)> = vec![
            ("no receipt at all", None),
            ("not JSON", Some(b"this is not json")),
            ("JSON without the field", Some(br#"{"arch": "x86_64"}"#)),
            (
                "the field is not a boolean",
                Some(br#"{"installed_on_request": "yes"}"#),
            ),
            (
                "the field is null",
                Some(br#"{"installed_on_request": null}"#),
            ),
            ("empty file", Some(b"")),
            ("oversized", Some(oversized.as_slice())),
        ];
        for (what, bytes) in cases {
            let root = caskroom_with("meld", bytes);
            assert_eq!(
                cask_requested(&root, "meld", FilesystemScope::ThisMachine),
                None,
                "{what} must be unknown, not a negative"
            );
            std::fs::remove_dir_all(&root).ok();
        }
    }

    #[test]
    fn a_receipt_is_not_read_for_a_machine_this_disk_does_not_describe() {
        // INV-15. The recording carries no receipt, so reading this host's
        // would answer about the wrong machine (F-25).
        let root = caskroom_with("meld", Some(br#"{"installed_on_request": true}"#));
        assert_eq!(
            cask_requested(&root, "meld", FilesystemScope::Elsewhere),
            None
        );
        assert_eq!(
            cask_requested(&root, "meld", FilesystemScope::ThisMachine),
            Some(true),
            "the same tree, declared as the machine, must answer"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_receipt_that_grew_past_the_cap_is_refused() {
        // `metadata.len()` is a snapshot. The read is capped independently, so
        // a file that grows between the two does not defeat the bound - the
        // failure the first version of this reader had.
        let root = caskroom_with("meld", None);
        let receipt = root
            .join("meld")
            .join(".metadata")
            .join("INSTALL_RECEIPT.json");
        let oversized = vec![b' '; usize::try_from(MAX_RECEIPT_BYTES + 2).expect("64-bit")];
        std::fs::write(&receipt, &oversized).expect("oversized receipt");
        assert_eq!(
            cask_requested(&root, "meld", FilesystemScope::ThisMachine),
            None
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_argument_vector_outside_the_allow_list_is_refused_at_run_time() {
        // The check used to be a `debug_assert!`, which compiles out of the
        // shipped binary — a guarantee the release build did not make. It is
        // a programming error, so `Internal` rather than a provider fault.
        use crate::process::{Recording, RecordingProvenance, ReplayRunner};
        let runner = ReplayRunner::new(Recording {
            schema_version: 1,
            provenance: RecordingProvenance {
                macos: "26.0.0".into(),
                architecture: "arm64".into(),
                provider_versions: Vec::new(),
                captured_at: "2026-09-02T00:00:00Z".into(),
                captured_by: "unit test".into(),
                origin: "synthesised".into(),
                note: None,
            },
            invocations: Vec::new(),
        });
        let adapter = HomebrewAdapter::new(
            &runner,
            PathBuf::from("/opt/homebrew"),
            None,
            Vec::new(),
            FilesystemScope::ThisMachine,
        );
        let error = adapter
            .run(&["outdated"])
            .expect_err("nothing outside the allow-list reaches the executor");
        assert_eq!(error.id, ErrorId::Internal);
        assert!(
            error.detail.contains("allow-listed"),
            "the refusal must name its reason: {}",
            error.detail
        );
        // An allow-listed vector gets past the check and reaches the runner,
        // which has no recorded answer for it. That the failure comes from
        // somewhere else is the point.
        let error = adapter.run(ARGV_VERSION).unwrap_err();
        assert!(
            !error.detail.contains("allow-listed"),
            "an allow-listed vector must not be refused here: {}",
            error.detail
        );
    }

    #[test]
    fn the_version_line_is_parsed_and_asserted() {
        let version =
            parse_version_line("Homebrew 6.0.21\nHomebrew/homebrew-core (git…)\n").unwrap();
        assert_eq!(version.as_str(), "6.0.21");
        for bad in ["", "Macports 2.11.5\n", "6.0.21\n", "Homebrew\n"] {
            let error = parse_version_line(bad).unwrap_err();
            assert_eq!(error.id, ErrorId::ProviderOutputChanged);
        }
    }

    #[test]
    fn config_lines_become_a_map_and_yield_the_architecture() {
        let config = parse_config(
            "HOMEBREW_VERSION: 6.0.21\nGit: 2.55.0 => /usr/local/bin/git\nmacOS: 26.6.2-x86_64\nCLT: 26.6.0\n",
        );
        assert_eq!(config["HOMEBREW_VERSION"], "6.0.21");
        // A value containing a colon keeps everything after the first one.
        assert_eq!(config["Git"], "2.55.0 => /usr/local/bin/git");
        assert_eq!(
            architecture_from_config(&config).unwrap(),
            Some(Architecture::X86_64)
        );
    }

    #[test]
    fn a_missing_macos_line_is_a_shape_change_and_an_unknown_arch_is_not_guessed() {
        let error = architecture_from_config(&parse_config("HOMEBREW_VERSION: 6\n")).unwrap_err();
        assert_eq!(error.id, ErrorId::ProviderOutputChanged);
        assert!(error.detail.contains("macOS"), "{}", error.detail);

        let future = parse_config("macOS: 30.0-riscv64\n");
        assert_eq!(architecture_from_config(&future).unwrap(), None);
    }

    #[test]
    fn the_environment_is_the_quiet_offline_one_and_inherits_nothing() {
        let env = quiet_environment(Path::new("/usr/local"), Some(PathBuf::from("/Users/x")));
        assert_eq!(env["HOMEBREW_NO_AUTO_UPDATE"], "1");
        assert_eq!(env["HOMEBREW_NO_ANALYTICS"], "1");
        assert_eq!(env["HOMEBREW_NO_ENV_HINTS"], "1");
        assert_eq!(env["HOMEBREW_NO_COLOR"], "1");
        assert_eq!(env["HOMEBREW_NO_EMOJI"], "1");
        assert!(env.contains_key("HOMEBREW_NO_INSTALL_CLEANUP"));
        assert!(env["PATH"].starts_with("/usr/local/bin:"));
        assert_eq!(env["LC_ALL"], "C");
        assert_eq!(env["HOME"], "/Users/x");
        assert!(!env["PATH"].contains("/opt/local"));
    }

    #[test]
    fn the_command_allowlist_is_the_documented_set() {
        assert_eq!(ALLOWED_ARGV.len(), 8);
        assert!(ALLOWED_ARGV.contains(&ARGV_INFO));
        // Nothing in the allowlist takes a value from provider output.
        for argv in ALLOWED_ARGV {
            assert!(!argv.is_empty());
        }
    }

    #[test]
    fn an_absent_prefix_detects_as_none() {
        assert_eq!(
            detect_prefix(
                &[PathBuf::from("/nonexistent-prefix")],
                FilesystemScope::ThisMachine
            ),
            None
        );
        assert_eq!(detect_prefix(&[], FilesystemScope::ThisMachine), None);
    }
}
