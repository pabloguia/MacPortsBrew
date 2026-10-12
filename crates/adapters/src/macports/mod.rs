// SPDX-License-Identifier: Apache-2.0

//! The MacPorts read adapter.
//!
//! MacPorts is a **peer provider**, not a fallback (INV-17): this adapter
//! implements the same [`ProviderRead`] contract as the other one, over the
//! same executor, and nothing here is shaped by which direction a migration
//! might go.
//!
//! # Where the shapes come from
//!
//! **MacPorts was not installed on the machine this adapter was written on**
//! (R-3 E-0, R-4 E-0, R-8: `/opt/local` absent, `port` not on `PATH`). Every
//! output shape parsed here is therefore taken from the documentation R-3
//! cites, and every fixture under `fixtures/recordings/macports-*.json` is
//! **synthesised** and says so in its provenance. The shapes R-3 itself marks
//! "needs observation" carry an `UNVERIFIED (R-3)` comment at the parser that
//! depends on them; they are the first thing to re-check on a machine that
//! has MacPorts.
//!
//! # The read set
//!
//! Eleven invocations at most, whatever the size of the installation - the
//! count does not scale with the number of ports:
//!
//! | Purpose | argv |
//! |---|---|
//! | detect | `version` |
//! | inventory rows | `-q installed` |
//! | user intent | `-q echo requested` |
//! | orphan candidates | `-q echo leaves` |
//! | active set (cross-check) | `-q echo active` |
//! | version / revision / successor | `-q info --index --line --name --version --revision --replaced_by <names>` |
//! | homepage / licence | `-q info --index --line --name --homepage --license <names>` |
//! | description | `-q info --index --line --name --description <names>` |
//! | library deps | `-q info --index --line --name --depends_lib <names>` |
//! | runtime deps | `-q info --index --line --name --depends_run <names>` |
//! | build deps | `-q info --index --line --name --depends_build <names>` |
//!
//! plus `-q provides <path>` on demand, and only for a path under the prefix.
//!
//! The metadata fields are split across several invocations rather than
//! gathered in one because `--line` documents its separator only as "Fields
//! are separated by spaces" and the one official `--line` example in the
//! documentation shows a field that itself contains a space (R-3 E-8). Each
//! invocation above therefore has **at most one variable-width field, and it
//! is last**, so no record needs a separator that is not documented.
//!
//! Every one of these reads local state - the registry, the PortIndex,
//! `macports.conf` - so `analyze` stays offline (INV-15). `selfupdate`,
//! `sync` and `livecheck` are the only actions documented to use the network
//! (R-3 E-14) and this adapter never names them. Quieting MacPorts is done
//! with argv (`-q`, which implies `-N`), **not** with environment variables:
//! R-3 Finding 12 found no `MACPORTS_*` variable in the pages read, which is
//! why [`MacPortsAdapter::environment`] contributes an empty `extra` map
//! while still building the prefix-scoped `PATH` INV-10 requires.
//!
//! # Reverse dependencies
//!
//! Derived by inverting the dependency edges among installed names in one
//! offline pass - the mechanism recorded as
//! [`REVERSE_DEPENDENCY_MECHANISM`] - rather than by running `port
//! dependents` once per installed port. The two are not identical: R-3
//! Finding 7 records that `dependents` reads relations the registry recorded
//! *at install time*, while `--index` reads the PortIndex and ignores variant
//! effects. The trade is one bounded read against N invocations, and it is
//! stated rather than hidden.

pub mod availability;
pub mod names;
pub mod parse;
pub mod service;

use crate::filesystem::FilesystemScope;
use crate::process::{CommandSpec, EnvironmentSpec, ExitStatus, Invocation, ProcessRunner};
use macportsbrew_domain::{
    ArtifactAvailability, AvailabilityAssessment, Capability, CapabilityState, DependencyEdge,
    DomainError, ErrorId, ExecutableOwner, InstalledPackage, Machine, PackageIdentity, PackageKind,
    PackageMetadata, ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory,
    ProviderPresence, ProviderRead, ProviderSnapshot, RequestedSet, Resolution, Version,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub use availability::{archive_query_url, ARCHIVE_QUERY_URL_FORMAT};

/// The provider id this adapter answers for.
pub const PROVIDER_ID: &str = "macports";

/// The prefix-relative path of the `port` executable.
pub(crate) const EXECUTABLE_RELATIVE: &str = "bin/port";

/// How ownership is decided: the registry's `requested` flag, read through
/// the `requested` pseudo-portname (R-3 Finding 3). Not an inference from the
/// dependency graph, and it must not be replaced by one.
pub const REQUESTED_MECHANISM: &str = "requested_pseudo_portname";

/// How reverse dependencies are produced: by inverting the `depends_*` edges
/// among installed names, offline, in one pass.
pub const REVERSE_DEPENDENCY_MECHANISM: &str = "depends_fields_inverted";

const ARGV_VERSION: &[&str] = &["version"];
const ARGV_INSTALLED: &[&str] = &["-q", "installed"];
const ARGV_ECHO_REQUESTED: &[&str] = &["-q", "echo", "requested"];
const ARGV_ECHO_LEAVES: &[&str] = &["-q", "echo", "leaves"];
const ARGV_ECHO_ACTIVE: &[&str] = &["-q", "echo", "active"];
pub(crate) const INFO_BASE: &[&str] = &["-q", "info", "--index", "--line", "--name"];
const INFO_FIELDS_VERSION: &[&str] = &["--version", "--revision", "--replaced_by"];
const INFO_FIELDS_HOMEPAGE: &[&str] = &["--homepage", "--license"];
const INFO_FIELDS_DESCRIPTION: &[&str] = &["--description"];
const INFO_FIELDS_DEPENDS_LIB: &[&str] = &["--depends_lib"];
const INFO_FIELDS_DEPENDS_RUN: &[&str] = &["--depends_run"];
const INFO_FIELDS_DEPENDS_BUILD: &[&str] = &["--depends_build"];
pub(crate) const ARGV_PROVIDES: &[&str] = &["-q", "provides"];

/// Every field tail [`INFO_BASE`] may be extended with, one entry per `info`
/// read the snapshot makes. [`is_allowed_argv`] refuses an `info` argument
/// vector whose fields are not exactly one of these, so the list holds on every
/// run, live or replayed, rather than by a test reading this file; and the
/// spawn-site read allowlist is tested against each of them rather than a
/// sample (`every_long_option_a_read_passes_is_permitted_and_no_other`).
pub(crate) const INFO_FIELDS: &[&[&str]] = &[
    INFO_FIELDS_VERSION,
    INFO_FIELDS_HOMEPAGE,
    INFO_FIELDS_DESCRIPTION,
    INFO_FIELDS_DEPENDS_LIB,
    INFO_FIELDS_DEPENDS_RUN,
    INFO_FIELDS_DEPENDS_BUILD,
];

/// Every complete argument vector this adapter may run.
///
/// The Homebrew adapter's [`crate::homebrew::ALLOWED_ARGV`] asked the same
/// question, and it is asked here for the same reason: nothing outside this
/// list, and nothing outside [`ALLOWED_ARGV_PREFIXES`], reaches the executor
/// (`04-migration-and-safety.md`, "Process execution").
pub const ALLOWED_ARGV: &[&[&str]] = &[
    ARGV_VERSION,
    ARGV_INSTALLED,
    ARGV_ECHO_REQUESTED,
    ARGV_ECHO_LEAVES,
    ARGV_ECHO_ACTIVE,
];

/// The argument vectors this adapter appends validated values to.
///
/// Unlike Homebrew's, two of this adapter's reads carry data: `info` takes the
/// port names it is asking about, and `provides` takes a path. Those cannot be
/// enumerated, so what is fixed is their *leading* portion — the subcommand
/// and the flags — and each appended element has already passed
/// [`names::is_safe_name`] or [`names::is_safe_path`]. A prefix match is a
/// weaker guarantee than an exact one, which is why it is a separate list: an
/// argument vector matched here is one whose tail was validated elsewhere.
pub const ALLOWED_ARGV_PREFIXES: &[&[&str]] = &[INFO_BASE, ARGV_PROVIDES];

/// True when `argv` is an exact allow-list entry, or extends an allow-listed
/// prefix - and, for `info`, extends [`INFO_BASE`] with exactly one
/// [`INFO_FIELDS`] tail followed by nothing option-shaped, so the fields an
/// `info` read asks for are a list here and not whatever a caller appended.
fn is_allowed_argv(argv: &[String]) -> bool {
    let after = |head: &[&str]| strip_head(argv, head);
    let info_tail = |rest: &[String]| {
        INFO_FIELDS.iter().any(|tail| {
            strip_head(rest, tail).is_some_and(|names| names.iter().all(|n| !n.starts_with('-')))
        })
    };
    ALLOWED_ARGV
        .iter()
        .any(|allowed| allowed.len() == argv.len() && after(allowed).is_some())
        || ALLOWED_ARGV_PREFIXES.iter().any(|head| match after(head) {
            Some(rest) if *head == INFO_BASE => info_tail(rest),
            Some(_) => true,
            None => false,
        })
}

/// What follows `head` in `argv`, when `argv` starts with exactly `head`.
fn strip_head<'a>(argv: &'a [String], head: &[&str]) -> Option<&'a [String]> {
    (argv.len() >= head.len() && head.iter().zip(argv).all(|(a, b)| *a == b.as_str()))
        .then(|| &argv[head.len()..])
}

/// The MacPorts read adapter over any [`ProcessRunner`] - the live machine or
/// a recorded fixture.
pub struct MacPortsAdapter<R: ProcessRunner> {
    runner: R,
    prefix: PathBuf,
    id: ProviderId,
    /// Whether this host's disk describes the machine being read. Separate
    /// from `runner.is_replay()`; see [`crate::filesystem`] (F-25).
    filesystem: FilesystemScope,
    machine: Option<Machine>,
    /// The user's home, or `None` under replay. `port` cannot start without
    /// `HOME`; see [`Self::environment`].
    home: Option<PathBuf>,
    /// Every prefix this provider could be installed under, for the search
    /// [`ProviderRead::installations`] makes. Empty unless the registry sets it.
    installation_candidates: Vec<PathBuf>,
    detection: RefCell<Option<ProviderDetection>>,
    inventory: RefCell<Option<ProviderInventory>>,
    /// The service scan the cached inventory's metadata was built from. The
    /// `ServiceMetadata` capability is read from this same value, never from a
    /// second scan: two scans can disagree, and a capability from a clean one
    /// beside per-package answers from an unreadable one would say "known, and
    /// nothing is a service" (security review and spec-guardian, pass 08
    /// slice 3, M2).
    services: RefCell<Option<service::ServiceScan>>,
}

impl<R: ProcessRunner> MacPortsAdapter<R> {
    /// Builds an adapter for a MacPorts installation at `prefix`.
    ///
    /// The executable is always `<prefix>/bin/port`, resolved from the
    /// prefix and never looked up on `PATH` (threat model: executable
    /// shadowing).
    /// `filesystem` says whether this host's disk describes the machine being
    /// read; where it does not, every filesystem answer is the unknown one
    /// (INV-15, F-25).
    pub fn new(
        runner: R,
        prefix: PathBuf,
        home: Option<PathBuf>,
        filesystem: FilesystemScope,
    ) -> Self {
        Self {
            runner,
            prefix,
            id: ProviderId::new(PROVIDER_ID),
            filesystem,
            machine: None,
            home,
            installation_candidates: Vec::new(),
            detection: RefCell::new(None),
            inventory: RefCell::new(None),
            services: RefCell::new(None),
        }
    }

    /// Names every prefix this provider could be installed under, so a second
    /// installation is reported rather than hidden behind the first. They come
    /// from dated policy data through the registry, never from this adapter.
    #[must_use]
    pub fn with_installation_candidates(mut self, candidates: Vec<PathBuf>) -> Self {
        self.installation_candidates = candidates;
        self
    }

    /// Attaches the machine, so availability evidence can name the platform
    /// tuple an archive filename would be keyed on (R-4 Finding 2).
    #[must_use]
    pub fn with_machine(mut self, machine: Machine) -> Self {
        self.machine = Some(machine);
        self
    }

    /// The first candidate prefix that holds a `port` executable.
    ///
    /// Filesystem only: no subprocess runs, so this is safe to call before
    /// anything is known about the installation.
    pub fn detect_prefix(candidates: &[PathBuf], filesystem: FilesystemScope) -> Option<PathBuf> {
        if !filesystem.is_this_machine() {
            // The recording is the machine; this host's disk says nothing
            // about which prefix it has. The caller supplies one (INV-15).
            return None;
        }
        candidates
            .iter()
            .find(|p| p.join(EXECUTABLE_RELATIVE).is_file())
            .cloned()
    }

    /// The prefix this adapter reads.
    pub fn prefix(&self) -> &Path {
        &self.prefix
    }

    /// The runner this adapter reads through, so a caller can ask a replay
    /// runner which invocations were actually consulted.
    pub fn runner(&self) -> &R {
        &self.runner
    }

    /// The executable this adapter runs.
    pub fn executable(&self) -> PathBuf {
        self.prefix.join(EXECUTABLE_RELATIVE)
    }

    /// The environment every invocation gets (INV-10: nothing inherited).
    ///
    /// `extra` is empty on purpose: R-3 Finding 12 found no `MACPORTS_*`
    /// variable in the documentation read, and quieting is argv (`-q`, which
    /// implies `-N`).
    ///
    /// **`home` was `None` and that broke every MacPorts read.** The reason
    /// given was that "no read here is documented to need it" — a
    /// documentation-derived inference, and false. `port` runs `mportinit`
    /// before any subcommand, and `mportinit` resolves the user's home through
    /// `dscl . -read /Users/$(id -un) NFSHomeDirectory`. Without `HOME` that
    /// pipeline fails and **`port` exits 1 whatever you asked it for**, so
    /// detection reported a healthy MacPorts as `Broken` (F-37, observed
    /// 2026-09-07 — the first time this repository had MacPorts installed).
    ///
    /// It is `Option` rather than required because it is `None` under replay,
    /// where no process runs; `registry.rs` says so at `ctx.home`.
    pub fn environment(&self) -> EnvironmentSpec {
        environment_spec(&self.prefix, self.home.clone())
    }

    fn spec(&self, argv: Vec<String>) -> CommandSpec {
        CommandSpec::provider(
            &self.prefix,
            EXECUTABLE_RELATIVE,
            argv,
            invocation_environment(&self.prefix, self.home.clone()),
        )
    }

    /// Runs one argument vector, refusing anything the allow-list does not
    /// cover.
    ///
    /// Checked at run time rather than with a `debug_assert!`, so the shipped
    /// binary makes the same guarantee the tests do.
    fn run_argv(&self, argv: Vec<String>) -> Result<Invocation, DomainError> {
        if !is_allowed_argv(&argv) {
            return Err(DomainError::new(
                ErrorId::Internal,
                format!("macports: {argv:?} is not an allow-listed argument vector"),
            ));
        }
        self.runner.run(&self.spec(argv))
    }

    fn run(&self, argv: &[&str]) -> Result<Invocation, DomainError> {
        self.run_argv(argv.iter().map(|s| (*s).to_string()).collect())
    }

    /// Runs one `info` invocation over `names`, refusing any name that could
    /// be read as an option.
    fn run_info(&self, fields: &[&str], names: &BTreeSet<String>) -> Result<String, DomainError> {
        let mut argv: Vec<String> = INFO_BASE.iter().map(|s| (*s).to_string()).collect();
        argv.extend(fields.iter().map(|s| (*s).to_string()));
        for name in names {
            if !names::is_safe_name(name) {
                return Err(DomainError::new(
                    ErrorId::ProviderOutputChanged,
                    format!("macports: refusing to pass `{name}` as an argument"),
                ));
            }
            argv.push(name.clone());
        }
        let inv = self.run_argv(argv)?;
        Ok(inv.require_success()?.stdout.clone())
    }

    fn run_text(&self, argv: &[&str]) -> Result<String, DomainError> {
        let inv = self.run(argv)?;
        Ok(inv.require_success()?.stdout.clone())
    }

    /// The `leaves` pseudo-portname: unrequested ports with no dependents,
    /// which is MacPorts' own answer to "what is an orphan" (R-3 Finding 4).
    ///
    /// `leaves` rather than `rleaves` deliberately: it is "removable right
    /// now without cascading", which is what the domain's
    /// [`macportsbrew_domain::Ownership::Orphan`] means.
    pub fn leaves(&self) -> Result<BTreeSet<String>, DomainError> {
        parse::parse_name_list(&self.run_text(ARGV_ECHO_LEAVES)?, "echo leaves")
    }

    fn detection(&self) -> Result<ProviderDetection, DomainError> {
        if let Some(cached) = self.detection.borrow().clone() {
            return Ok(cached);
        }
        let invocation = self.run(ARGV_VERSION)?;
        let detection = match &invocation.status {
            ExitStatus::NotStarted { .. } => ProviderDetection {
                provider: self.id.clone(),
                presence: ProviderPresence::Absent,
                version: None,
                prefix: None,
                executable: None,
                architecture: None,
                // Detection never redacts - `capture` does, and is the only
                // thing that may say so (F-49, SE-080).
                prefix_redacted: false,
            },
            status if status.success() => ProviderDetection {
                provider: self.id.clone(),
                presence: ProviderPresence::Present,
                version: Some(Version::new(parse::parse_version(&invocation.stdout)?)),
                prefix: Some(self.prefix.clone()),
                executable: Some(self.executable()),
                // R-4 Finding 11: `port -v installed` prints the archs a port
                // was built for, but nothing reports an architecture for the
                // base installation, and `-v` is not in this adapter's read
                // set. Left undetermined rather than guessed.
                architecture: None,
                prefix_redacted: false,
            },
            _ => ProviderDetection {
                provider: self.id.clone(),
                presence: ProviderPresence::Broken {
                    reason: ErrorId::ProviderUnhealthy.message_id().to_string(),
                },
                version: None,
                prefix: Some(self.prefix.clone()),
                executable: Some(self.executable()),
                architecture: None,
                prefix_redacted: false,
            },
        };
        *self.detection.borrow_mut() = Some(detection.clone());
        Ok(detection)
    }

    fn build_inventory(&self) -> Result<ProviderInventory, DomainError> {
        let rows = parse::parse_installed(&self.run_text(ARGV_INSTALLED)?)?;
        let requested =
            parse::parse_name_list(&self.run_text(ARGV_ECHO_REQUESTED)?, "echo requested")?;
        let leaves = parse::parse_name_list(&self.run_text(ARGV_ECHO_LEAVES)?, "echo leaves")?;
        let active = parse::parse_name_list(&self.run_text(ARGV_ECHO_ACTIVE)?, "echo active")?;

        let names: BTreeSet<String> = rows.iter().map(|r| r.name.clone()).collect();
        cross_check(&names, &active, &leaves, &requested, &rows)?;

        let (installed, active_variants) = installed_rows(&rows, &requested, &self.id);
        let services = service::scan(
            &self.prefix,
            self.filesystem,
            &activity(&rows),
            &|plist: &Path| self.provides(plist),
        );
        *self.services.borrow_mut() = Some(services.clone());
        if names.is_empty() {
            return Ok(ProviderInventory {
                installed,
                requested: RequestedSet {
                    names: requested,
                    mechanism: REQUESTED_MECHANISM.to_string(),
                },
                ..ProviderInventory::default()
            });
        }

        let index = parse::parse_index_version(&self.run_info(INFO_FIELDS_VERSION, &names)?)?;
        let homepage =
            parse::parse_index_homepage_license(&self.run_info(INFO_FIELDS_HOMEPAGE, &names)?)?;
        let description =
            parse::parse_index_description(&self.run_info(INFO_FIELDS_DESCRIPTION, &names)?)?;
        let depends_lib = parse::parse_index_depends(
            &self.run_info(INFO_FIELDS_DEPENDS_LIB, &names)?,
            "depends_lib",
        )?;
        let depends_run = parse::parse_index_depends(
            &self.run_info(INFO_FIELDS_DEPENDS_RUN, &names)?,
            "depends_run",
        )?;
        let depends_build = parse::parse_index_depends(
            &self.run_info(INFO_FIELDS_DEPENDS_BUILD, &names)?,
            "depends_build",
        )?;

        let dependencies = edges(&names, &depends_lib, &depends_run, &depends_build);
        let metadata = metadata(&names, &index, &homepage, &description, &services);
        let availability = availability(&names, &index, &active_variants, self.machine.as_ref());

        Ok(ProviderInventory {
            installed,
            requested: RequestedSet {
                names: requested,
                mechanism: REQUESTED_MECHANISM.to_string(),
            },
            dependencies,
            metadata,
            availability,
        })
    }

    /// Asks `port` which port owns a path.
    ///
    /// The path is validated here rather than only at the call site. The
    /// caller does check it, but a function that turns its argument into a
    /// process argument is where the check has to hold: the next caller will
    /// not read the doc comment, and a check that lives in the caller is a
    /// check the callee does not have.
    fn provides(&self, path: &Path) -> Option<String> {
        if !names::is_safe_path(path) {
            return None;
        }
        let mut argv: Vec<String> = ARGV_PROVIDES.iter().map(|s| (*s).to_string()).collect();
        argv.push(path.to_str()?.to_string());
        let invocation = self.run_argv(argv).ok()?;
        if !invocation.status.success() {
            return None;
        }
        parse::parse_provides(&invocation.stdout)
    }
}

/// Rejects an installed listing that disagrees with the pseudo-portname sets.
///
/// The three sets come from the same registry, so a disagreement is a shape
/// change in one of them and not something to reconcile silently. In
/// particular it is how this adapter finds out that `-q installed` stopped
/// printing the `(active)` marker whose exact rendering R-3 could not verify.
fn cross_check(
    names: &BTreeSet<String>,
    active: &BTreeSet<String>,
    leaves: &BTreeSet<String>,
    requested: &BTreeSet<String>,
    rows: &[parse::InstalledRow],
) -> Result<(), DomainError> {
    let disagreement = |what: &str, name: &str| {
        DomainError::new(
            ErrorId::ProviderOutputChanged,
            format!("macports: `{name}` {what}"),
        )
    };
    for name in active {
        if !names.contains(name) {
            return Err(disagreement(
                "is in the active set but has no installed row",
                name,
            ));
        }
        if !rows.iter().any(|r| &r.name == name && r.active) {
            return Err(disagreement(
                "is in the active set but no installed row is marked active",
                name,
            ));
        }
    }
    for row in rows.iter().filter(|r| r.active) {
        if !active.contains(&row.name) {
            return Err(disagreement(
                "has an active installed row but is not in the active set",
                &row.name,
            ));
        }
    }
    for name in leaves {
        if !names.contains(name) {
            return Err(disagreement("is a leaf but has no installed row", name));
        }
        if requested.contains(name) {
            return Err(disagreement("is both requested and a leaf", name));
        }
    }
    for name in requested {
        if !names.contains(name) {
            return Err(disagreement("is requested but has no installed row", name));
        }
    }
    Ok(())
}

/// Each installed port name, and whether any of its installed versions is
/// active: what the service scan needs to attribute an item and to know that
/// an absent one proves something.
fn activity(rows: &[parse::InstalledRow]) -> BTreeMap<String, bool> {
    let mut by_name: BTreeMap<String, bool> = BTreeMap::new();
    for row in rows {
        *by_name.entry(row.name.clone()).or_default() |= row.active;
    }
    by_name
}

/// Turns parsed rows into domain rows, and collects the variant set of each
/// name's active row (the one an archive filename would be keyed on).
fn installed_rows(
    rows: &[parse::InstalledRow],
    requested: &BTreeSet<String>,
    id: &ProviderId,
) -> (Vec<InstalledPackage>, BTreeMap<String, BTreeSet<String>>) {
    let mut installed = Vec::with_capacity(rows.len());
    let mut active_variants: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in rows {
        if row.active {
            active_variants.insert(row.name.clone(), row.variants.clone());
        }
        installed.push(InstalledPackage {
            identity: PackageIdentity::new(id.clone(), row.name.clone()),
            // MacPorts has no cask concept (R-1/SE-012): every row is a port.
            kind: PackageKind::Package,
            version: Version::new(row.version.clone()),
            revision: row.revision.clone(),
            variants: row.variants.clone(),
            active: row.active,
            requested: Some(requested.contains(&row.name)),
            // R-4 Finding 11: the registry has no column distinguishing a
            // downloaded image from a locally built one, so this is never
            // knowable for an already-installed port.
            installed_from_prebuilt: None,
            install_path: None,
        });
    }
    installed.sort();
    (installed, active_variants)
}

/// Inverts nothing yet - it only records the forward edges, restricted to
/// installed names. [`ProviderInventory::reverse_dependencies_of`] does the
/// inversion, which is what [`REVERSE_DEPENDENCY_MECHANISM`] names.
fn edges(
    names: &BTreeSet<String>,
    lib: &BTreeMap<String, BTreeSet<String>>,
    run: &BTreeMap<String, BTreeSet<String>>,
    build: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<DependencyEdge> {
    let mut runtime: BTreeSet<(String, String)> = BTreeSet::new();
    for source in [lib, run] {
        for (from, tos) in source {
            for to in tos {
                if names.contains(from) && names.contains(to) && from != to {
                    runtime.insert((from.clone(), to.clone()));
                }
            }
        }
    }
    let mut out: Vec<DependencyEdge> = runtime
        .iter()
        .map(|(from, to)| DependencyEdge {
            from: from.clone(),
            to: to.clone(),
            build_only: false,
        })
        .collect();
    for (from, tos) in build {
        for to in tos {
            if names.contains(from)
                && names.contains(to)
                && from != to
                && !runtime.contains(&(from.clone(), to.clone()))
            {
                out.push(DependencyEdge {
                    from: from.clone(),
                    to: to.clone(),
                    build_only: true,
                });
            }
        }
    }
    out.sort();
    out
}

fn metadata(
    names: &BTreeSet<String>,
    index: &BTreeMap<String, parse::IndexVersion>,
    homepage: &BTreeMap<String, parse::IndexOrigin>,
    description: &BTreeMap<String, String>,
    services: &service::ServiceScan,
) -> BTreeMap<String, PackageMetadata> {
    let mut out = BTreeMap::new();
    for name in names {
        let origin = homepage.get(name).cloned().unwrap_or_default();
        let entry = index.get(name);
        out.insert(
            name.clone(),
            PackageMetadata {
                description: description.get(name).cloned(),
                homepage: origin.homepage,
                license: origin.license,
                available_version: entry
                    .map(|e| Version::new(format!("{}_{}", e.version, e.revision))),
                replaced_by: entry.and_then(|e| e.replaced_by.clone()),
                // R-3 Finding 8: `replaced_by` and `obsolete` are two
                // independent states and must not collapse into one flag.
                // `obsolete` is not read here, so `deprecated` stays unset.
                defines_service: services.defines_service(name),
                ..PackageMetadata::default()
            },
        );
    }
    out
}

fn availability(
    names: &BTreeSet<String>,
    index: &BTreeMap<String, parse::IndexVersion>,
    active_variants: &BTreeMap<String, BTreeSet<String>>,
    machine: Option<&Machine>,
) -> BTreeMap<String, AvailabilityAssessment> {
    let empty = BTreeSet::new();
    names
        .iter()
        .map(|name| {
            let entry = index.get(name);
            let facts = availability::Facts {
                name,
                in_index: entry.is_some(),
                replaced_by: entry.and_then(|e| e.replaced_by.as_deref()),
                variants: active_variants.get(name).unwrap_or(&empty),
            };
            (name.clone(), availability::assess(&facts, machine))
        })
        .collect()
}

impl<R: ProcessRunner> ProviderRead for MacPortsAdapter<R> {
    fn id(&self) -> &ProviderId {
        &self.id
    }

    fn detect(&self) -> Result<ProviderDetection, DomainError> {
        self.detection()
    }

    fn capabilities(&self) -> Result<ProviderCapabilities, DomainError> {
        let present = matches!(self.detection()?.presence, ProviderPresence::Present);
        let mut capabilities = BTreeMap::new();
        if !present {
            // `Capability::all()` rather than the nine names written out, which
            // is what stood here: the list carries a drift guard now, and a
            // capability added later cannot quietly go unwritten and be read
            // back from a document as "never asked about" (F-29, F-43).
            for &capability in Capability::all() {
                capabilities.insert(capability, CapabilityState::Unknown);
            }
            return Ok(ProviderCapabilities { capabilities });
        }
        // R-3 Finding 1: no JSON, but `info` with explicit field flags is a
        // field-selected machine-readable path, which is what the capability
        // means.
        capabilities.insert(
            Capability::MachineReadableInventory,
            CapabilityState::Available,
        );
        capabilities.insert(Capability::RequestedRecord, CapabilityState::Available);
        capabilities.insert(Capability::Dependencies, CapabilityState::Available);
        // Derived, not native: see REVERSE_DEPENDENCY_MECHANISM and the
        // module documentation. `ProviderCapabilities` carries no free-text
        // slot on an available state, so the mechanism is reported through
        // `environment_info()` as well.
        capabilities.insert(Capability::ReverseDependencies, CapabilityState::Available);
        // R-4 Finding 9: offline, availability is decidable only in the
        // negative direction. The capability the domain means - assessing an
        // artifact without installing - needs the network.
        capabilities.insert(
            Capability::ArtifactAvailability,
            CapabilityState::Unavailable {
                reason: "network_required_ports_macports_builds_api".to_string(),
            },
        );
        // The inventory is answered from the local ports index, and
        // `port provides` answers from the registry's file map (R-3): both are
        // read by this adapter, so both are reported as what they are.
        capabilities.insert(Capability::OfflineMetadata, CapabilityState::Available);
        capabilities.insert(Capability::FileOwnership, CapabilityState::Available);
        // Only `replaced_by` is read. `obsolete` is a separate state (R-3
        // Finding 8) that this adapter never asks for, and `deprecated` stays
        // unset in `metadata()`, so the deprecation state is not exposed by
        // what this adapter reads. An unprobed capability is `Unknown`, never
        // `Available`.
        capabilities.insert(Capability::DeprecationState, CapabilityState::Unknown);
        capabilities.insert(
            Capability::ServiceMetadata,
            if self.inventory().is_ok()
                && self
                    .services
                    .borrow()
                    .as_ref()
                    .is_some_and(service::ServiceScan::is_known)
            {
                CapabilityState::Available
            } else {
                CapabilityState::Unknown
            },
        );
        // Whether reading this provider needs administrator rights, answered by
        // reading it. `inventory()` runs `port -q installed` — the verify verb
        // itself — and adds no probe of its own. It caches on *success* only,
        // so a provider whose inventory fails re-runs it here; that is one call
        // site, and saying "memoises" without the qualifier was wrong
        // (`spec-guardian`). A registry is created with the installing
        // user's umask, so the stored `none` in `provider-commands.toml` is true
        // of the machine it was observed on and of no other; this is the same
        // question asked of *this* machine (INV-20, F-24, F-35).
        //
        // `Available` or `Unknown`, never `Unavailable`: a failed read does not
        // say why it failed, and inferring "needs elevation" from it would put
        // an administrator prompt on a read, which is the INV-06 defect SE-059
        // recorded.
        capabilities.insert(
            Capability::UnprivilegedRead,
            if self.inventory().is_ok() {
                CapabilityState::Available
            } else {
                CapabilityState::Unknown
            },
        );
        Ok(ProviderCapabilities { capabilities })
    }

    fn inventory(&self) -> Result<ProviderInventory, DomainError> {
        if let Some(cached) = self.inventory.borrow().clone() {
            return Ok(cached);
        }
        let inventory = if matches!(self.detection()?.presence, ProviderPresence::Present) {
            self.build_inventory()?
        } else {
            ProviderInventory::default()
        };
        *self.inventory.borrow_mut() = Some(inventory.clone());
        Ok(inventory)
    }

    fn artifact_availability(
        &self,
        pkg: &PackageIdentity,
        machine: &Machine,
    ) -> Result<AvailabilityAssessment, DomainError> {
        let inventory = self.inventory()?;
        let metadata = inventory.metadata.get(&pkg.name);
        let variants = inventory
            .installed
            .iter()
            .find(|p| p.identity.name == pkg.name && p.active)
            .map(|p| p.variants.clone())
            .unwrap_or_default();
        if metadata.is_none() && !inventory.installed_names().contains(&pkg.name) {
            return Ok(AvailabilityAssessment::unknown(
                "port info --index",
                format!("`{}` is not installed under this provider", pkg.name),
            ));
        }
        let facts = availability::Facts {
            name: &pkg.name,
            in_index: metadata
                .and_then(|m| m.available_version.as_ref())
                .is_some(),
            replaced_by: metadata.and_then(|m| m.replaced_by.as_deref()),
            variants: &variants,
        };
        let assessment = availability::assess(&facts, Some(machine));
        debug_assert_ne!(
            assessment.availability,
            ArtifactAvailability::BinaryAvailable
        );
        Ok(assessment)
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
        // Following the symlink reads this host's disk, which is only done
        // when that disk is the machine being described (INV-15). Elsewhere
        // the path is classified as written, from the recording, exactly as
        // the Homebrew adapter does in the same scope.
        //
        // On this machine a path that cannot be canonicalised (a dangling
        // link, an unreadable directory) answers `Unknown`. Classifying it
        // from the path as written would call `<prefix>/bin/x` MacPorts-owned
        // when the link is broken or points out of the prefix, and following
        // the link is precisely what stops `<prefix>/bin/x -> /usr/bin/x`
        // being claimed. The Homebrew adapter answers `Unknown` here too, so
        // the two peers degrade the same way (INV-17).
        let canonical = if self.filesystem.is_this_machine() {
            let Ok(resolved) = std::fs::canonicalize(executable) else {
                return Ok(unresolved());
            };
            Some(resolved)
        } else {
            None
        };
        let target = canonical.as_deref().unwrap_or(executable);
        let (owner, authoritative) = if target.starts_with(&self.prefix) {
            // R-3: `port provides <file>` answers from the registry's file
            // map, which is the authoritative form. Anything else about this
            // path is a prefix heuristic and is marked as one.
            let package = names::is_safe_path(target)
                .then(|| self.provides(target))
                .flatten();
            let authoritative = package.is_some();
            (
                ExecutableOwner::Provider {
                    provider: self.id.clone(),
                    package,
                },
                authoritative,
            )
        } else if is_system_path(target) {
            (ExecutableOwner::System, false)
        } else {
            // Not under this prefix and not part of macOS. This adapter
            // knows nothing about any other provider's prefix (INV-04), so
            // it says so instead of guessing `External`.
            (ExecutableOwner::Unknown, false)
        };
        Ok(Resolution {
            executable: name,
            path: Some(executable.to_path_buf()),
            canonical_path: canonical.filter(|c| c != executable),
            owner,
            authoritative,
        })
    }

    fn environment_info(&self) -> Result<BTreeMap<String, String>, DomainError> {
        let text = |p: PathBuf| p.to_string_lossy().into_owned();
        let mut out = BTreeMap::new();
        out.insert("prefix".to_string(), text(self.prefix.clone()));
        out.insert("bin".to_string(), text(self.prefix.join("bin")));
        out.insert("sbin".to_string(), text(self.prefix.join("sbin")));
        // R-3 E-12: `${portdbpath}/registry/registry.db`, and `portdbpath`
        // defaults to `${prefix}/var/macports`. A non-default `portdbpath` in
        // `macports.conf` moves it; this is the documented default, not a
        // reading of the configuration file.
        out.insert(
            "registry".to_string(),
            text(self.prefix.join("var/macports/registry/registry.db")),
        );
        // R-3 E-12: the absolute path of the default tree's `PortIndex` is
        // not stated verbatim by any page R-3 read - it is composed from
        // `portdbpath` and the `sources.conf` entry - so it is described
        // rather than asserted.
        out.insert(
            "portindex_note".to_string(),
            format!(
                "the ports tree, and its PortIndex, are named in {}; the absolute path is \
                 runtime detection, not a documented constant (R-3 E-12)",
                text(self.prefix.join("etc/macports/sources.conf"))
            ),
        );
        out.insert(
            "reverse_dependencies".to_string(),
            REVERSE_DEPENDENCY_MECHANISM.to_string(),
        );
        Ok(out)
    }

    fn installations(&self) -> macportsbrew_domain::Installations {
        crate::installations::read(
            &self.installation_candidates,
            EXECUTABLE_RELATIVE,
            self.filesystem,
        )
    }

    fn snapshot(&self) -> Result<ProviderSnapshot, DomainError> {
        let detection = self.detect()?;
        // Only for a provider that is here, as the Homebrew adapter does: an
        // absent one is never invoked.
        let configuration = if matches!(detection.presence, ProviderPresence::Present) {
            crate::configuration::read(
                PROVIDER_ID,
                &self.prefix,
                self.home.as_deref(),
                self.filesystem,
            )
        } else {
            macportsbrew_domain::ProviderConfiguration::NotRecorded
        };
        Ok(ProviderSnapshot {
            detection,
            capabilities: self.capabilities()?,
            inventory: self.inventory()?,
            environment: self.environment_info()?,
            invocation_environment: invocation_environment(&self.prefix, self.home.clone()),
            resolutions: BTreeMap::new(),
            configuration,
        })
    }
}

/// What this provider contributes to a sanitised environment, for a prefix and
/// a home. The adapter's own invocations and a plan read the registry derives
/// (DP-023) both go through this one function, so the two cannot drift; see
/// [`MacPortsAdapter::environment`] for why `home` matters to `port`.
pub(crate) fn environment_spec(prefix: &Path, home: Option<PathBuf>) -> EnvironmentSpec {
    EnvironmentSpec {
        leading_path: vec![prefix.join("bin"), prefix.join("sbin")],
        home,
        extra: BTreeMap::new(),
    }
}

/// The complete environment every invocation of this provider runs in: the
/// counterpart of the Homebrew adapter's `quiet_environment`, with the same
/// signature, so the registry derives both providers through one code path
/// (INV-17).
pub fn invocation_environment(prefix: &Path, home: Option<PathBuf>) -> BTreeMap<String, String> {
    crate::process::sanitised_environment(&environment_spec(prefix, home))
}

fn is_system_path(path: &Path) -> bool {
    [
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
        "/System",
        "/usr/libexec",
    ]
    .iter()
    .any(|root| path.starts_with(root))
}

#[cfg(test)]
mod tests {
    fn row(name: &str, version: &str, active: bool) -> parse::InstalledRow {
        parse::InstalledRow {
            name: name.to_string(),
            version: version.to_string(),
            revision: None,
            variants: BTreeSet::new(),
            active,
        }
    }

    #[test]
    fn a_port_is_active_when_any_installed_version_is_and_inactive_when_none_is() {
        // The service scan's "every installed port is active" rests on this:
        // an older inactive version beside the active one does not make the
        // port inactive, and a port with only inactive versions is.
        let rows = [
            row("jq", "1.8", true),
            row("jq", "1.7", false),
            row("postgresql17-server", "17.1", false),
        ];
        let by_name = activity(&rows);
        assert_eq!(by_name.get("jq"), Some(&true));
        assert_eq!(by_name.get("postgresql17-server"), Some(&false));
    }

    use super::*;
    use crate::process::{Recording, RecordingProvenance, ReplayRunner};

    fn empty_runner() -> ReplayRunner {
        ReplayRunner::new(Recording {
            // The recording schema version; the executor's own constant is
            // not re-exported and this module does not modify the executor.
            schema_version: 1,
            provenance: RecordingProvenance {
                macos: "12.7.6".into(),
                architecture: "x86_64".into(),
                provider_versions: vec![],
                captured_at: "2026-09-02T00:00:00Z".into(),
                captured_by: "unit test".into(),
                origin: "synthesised".into(),
                note: None,
            },
            invocations: vec![],
        })
    }

    #[test]
    fn an_info_read_asks_only_for_a_listed_field_tail() {
        // The adapter's own check, which runs on every invocation, replayed or
        // live: an `info` read is the head, exactly one `INFO_FIELDS` tail and
        // validated names - not the head and whatever follows it.
        let argv =
            |parts: &[&str]| -> Vec<String> { parts.iter().map(|s| (*s).to_string()).collect() };
        let with = |fields: &[&str], names: &[&str]| {
            let mut v = argv(INFO_BASE);
            v.extend(argv(fields));
            v.extend(argv(names));
            v
        };
        for tail in INFO_FIELDS {
            assert!(is_allowed_argv(&with(tail, &["libpng"])), "{tail:?}");
            assert!(is_allowed_argv(&with(tail, &[])), "{tail:?}");
        }
        for (fields, names) in [
            // A field no read asks for.
            (&["--maintainers"][..], &["libpng"][..]),
            // A listed field, but not a listed tail.
            (&["--version"][..], &["libpng"][..]),
            // A listed tail, and a field more.
            (&["--description", "--maintainers"][..], &["libpng"][..]),
            (&["--description"][..], &["libpng", "--maintainers"][..]),
            // No tail at all.
            (&[][..], &["libpng"][..]),
        ] {
            assert!(
                !is_allowed_argv(&with(fields, names)),
                "{fields:?} {names:?} was allowed"
            );
        }
    }

    #[test]
    fn the_allow_list_admits_the_fixed_vectors_and_the_two_that_carry_data() {
        let argv =
            |parts: &[&str]| -> Vec<String> { parts.iter().map(|s| (*s).to_string()).collect() };
        for allowed in ALLOWED_ARGV {
            assert!(is_allowed_argv(&argv(allowed)), "{allowed:?}");
        }
        // `info` and `provides` carry validated data after a fixed head.
        assert!(is_allowed_argv(&argv(&[
            "-q",
            "info",
            "--index",
            "--line",
            "--name",
            "--description",
            "libpng"
        ])));
        assert!(is_allowed_argv(&argv(&[
            "-q",
            "provides",
            "/opt/local/bin/curl"
        ])));
        // A prefix is a head, not a substring: an allow-listed vector must not
        // license one that merely ends with it.
        assert!(!is_allowed_argv(&argv(&["-q", "sync", "info"])));
        assert!(!is_allowed_argv(&argv(&["-q"])));
        assert!(!is_allowed_argv(&argv(&["version", "extra"])));
        assert!(!is_allowed_argv(&argv(&[])));
    }

    #[test]
    fn an_argument_vector_outside_the_allow_list_is_refused_at_run_time() {
        // A runtime check and not a `debug_assert!`: the release binary is
        // where the guarantee has to hold.
        let adapter = MacPortsAdapter::new(
            empty_runner(),
            PathBuf::from("/opt/local"),
            None,
            FilesystemScope::ThisMachine,
        );
        let error = adapter
            .run_argv(vec!["-q".into(), "not-a-read".into()])
            .expect_err("nothing outside the allow-list reaches the executor");
        assert_eq!(error.id, ErrorId::Internal);
    }

    #[test]
    fn a_path_that_is_not_safe_is_never_asked_about() {
        // `provides` validates its own argument rather than trusting the
        // caller to have done it.
        let adapter = MacPortsAdapter::new(
            empty_runner(),
            PathBuf::from("/opt/local"),
            None,
            FilesystemScope::ThisMachine,
        );
        for bad in ["relative/bin/curl", "/opt/local/../etc/passwd"] {
            assert_eq!(
                adapter.provides(Path::new(bad)),
                None,
                "{bad} must not reach the executor"
            );
        }
    }

    #[test]
    fn the_executable_comes_from_the_prefix_and_never_from_path() {
        let adapter = MacPortsAdapter::new(
            empty_runner(),
            PathBuf::from("/opt/local"),
            None,
            FilesystemScope::ThisMachine,
        );
        assert_eq!(adapter.executable(), PathBuf::from("/opt/local/bin/port"));
        assert_eq!(adapter.prefix(), Path::new("/opt/local"));
        assert_eq!(adapter.id().as_str(), PROVIDER_ID);
    }

    #[test]
    fn the_environment_is_prefix_scoped_and_carries_no_variables() {
        let adapter = MacPortsAdapter::new(
            empty_runner(),
            PathBuf::from("/opt/local"),
            None,
            FilesystemScope::ThisMachine,
        );
        let spec = adapter.environment();
        assert_eq!(
            spec.leading_path,
            vec![
                PathBuf::from("/opt/local/bin"),
                PathBuf::from("/opt/local/sbin")
            ]
        );
        assert!(spec.home.is_none());
        assert!(spec.extra.is_empty());
        let env = crate::process::sanitised_environment(&spec);
        assert!(env["PATH"].starts_with("/opt/local/bin:/opt/local/sbin:"));
        assert!(!env["PATH"].contains("/usr/local"));
        assert!(!env.contains_key("HOME"));
    }

    #[test]
    fn prefix_detection_looks_for_the_executable() {
        assert_eq!(
            MacPortsAdapter::<ReplayRunner>::detect_prefix(
                &[PathBuf::from("/nonexistent-prefix-for-a-test")],
                FilesystemScope::ThisMachine
            ),
            None
        );
        // The registry is the only place that knows where a provider installs
        // itself, and this adapter held a second, unused copy of that path -
        // found by the widened INV-20 check rather than by anyone noticing
        // (F-19). The assertion it replaced compared that copy to itself.
        assert_eq!(
            crate::registry::candidate_prefixes(&ProviderId::new(PROVIDER_ID)),
            vec![PathBuf::from("/opt/local")],
            "the documented default prefix (R-3 E-11, guide.macports.org, read 2026-09-02)"
        );
    }

    #[test]
    fn environment_info_reports_the_documented_paths() {
        let adapter = MacPortsAdapter::new(
            empty_runner(),
            PathBuf::from("/opt/local"),
            None,
            FilesystemScope::ThisMachine,
        );
        let info = adapter.environment_info().unwrap();
        assert_eq!(info["prefix"], "/opt/local");
        assert_eq!(info["bin"], "/opt/local/bin");
        assert_eq!(info["sbin"], "/opt/local/sbin");
        assert_eq!(
            info["registry"],
            "/opt/local/var/macports/registry/registry.db"
        );
        assert!(info["portindex_note"].contains("sources.conf"));
        assert_eq!(info["reverse_dependencies"], REVERSE_DEPENDENCY_MECHANISM);
    }

    #[test]
    fn build_edges_stay_inside_the_installed_set_and_never_shadow_runtime_ones() {
        let names: BTreeSet<String> = ["a", "b", "c"].iter().map(|s| (*s).to_string()).collect();
        let mut lib = BTreeMap::new();
        lib.insert(
            "a".to_string(),
            ["b".to_string(), "elsewhere".to_string()]
                .into_iter()
                .collect::<BTreeSet<String>>(),
        );
        let mut build = BTreeMap::new();
        build.insert(
            "a".to_string(),
            ["b".to_string(), "c".to_string()]
                .into_iter()
                .collect::<BTreeSet<String>>(),
        );
        let out = edges(&names, &lib, &BTreeMap::new(), &build);
        assert_eq!(out.len(), 2);
        assert!(out.iter().any(|e| e.to == "b" && !e.build_only));
        assert!(out.iter().any(|e| e.to == "c" && e.build_only));
        assert!(!out.iter().any(|e| e.to == "elsewhere"));
    }

    #[test]
    fn system_paths_are_recognised() {
        assert!(is_system_path(Path::new("/usr/bin/curl")));
        assert!(is_system_path(Path::new("/bin/sh")));
        assert!(!is_system_path(Path::new("/usr/local/bin/wget")));
        assert!(!is_system_path(Path::new("/opt/local/bin/wget")));
    }
}
