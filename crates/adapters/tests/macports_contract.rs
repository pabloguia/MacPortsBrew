// SPDX-License-Identifier: Apache-2.0

//! Contract tests for the MacPorts read adapter, against recorded fixtures.
//!
//! The two `macports-*` fixtures are **synthesised**, not captured: MacPorts
//! is installed on no machine available to this project (R-3 E-0), so these
//! tests pin the adapter against the output formats
//! `docs/research/R-3-macports-read-interfaces.md` cites, not against
//! observed bytes. Each fixture says so in its provenance, and this file
//! asserts that it does.

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::macports::{
    archive_query_url, availability, service, MacPortsAdapter, ARCHIVE_QUERY_URL_FORMAT,
    PROVIDER_ID, REQUESTED_MECHANISM, REVERSE_DEPENDENCY_MECHANISM,
};
use macportsbrew_adapters::process::{sanitised_environment, Recording, ReplayRunner};
use macportsbrew_domain::availability::SourceBuildReason;
use macportsbrew_domain::ownership::classify_ownership;
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, Capability, CapabilityState, CommandLineTools, ErrorId,
    Machine, MacosVersion, Ownership, PackageIdentity, ProviderId, ProviderInventory,
    ProviderPresence, ProviderRead, Translation,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const MACPORTS_FIXTURE: &str = "macports-only-intel-macos12";
const BROKEN_FIXTURE: &str = "macports-broken";
const ABSENT_FIXTURE: &str = "homebrew-only-intel-macos26";

const PREFIX: &str = "/opt/local";

/// The names the fixture reports as installed, in the order the adapter
/// passes them as arguments.
const NAMES: &str = "bzip2 gettext-runtime gnutls libiconv libidn2 ncurses nginx openssl3 \
                     postgresql17-server py-pygments wget zlib";

fn fixture_text(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings")
        .join(format!("{name}.json"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn adapter(name: &str) -> MacPortsAdapter<ReplayRunner> {
    let recording = Recording::from_json(&fixture_text(name)).expect("fixture parses");
    MacPortsAdapter::new(
        ReplayRunner::new(recording),
        PathBuf::from(PREFIX),
        None,
        FilesystemScope::Elsewhere,
    )
    .with_machine(machine())
}

fn machine() -> Machine {
    Machine {
        architecture: Architecture::X86_64,
        macos: MacosVersion::new(12, 7, 6),
        translation: Translation::Native,
        command_line_tools: CommandLineTools::Unknown,
    }
}

fn identity(name: &str) -> PackageIdentity {
    PackageIdentity::new(ProviderId::new(PROVIDER_ID), name)
}

// -- provenance ------------------------------------------------------------

#[test]
fn both_macports_fixtures_declare_themselves_synthesised() {
    for name in [MACPORTS_FIXTURE, BROKEN_FIXTURE] {
        let recording = Recording::from_json(&fixture_text(name)).unwrap();
        assert_eq!(
            recording.provenance.origin, "synthesised",
            "{name} must not claim to be captured"
        );
        let note = recording.provenance.note.unwrap_or_default();
        assert!(note.contains("R-3"), "{name} must name the research note");
        assert!(
            note.contains("SYNTHESISED"),
            "{name} must say so in plain words"
        );
    }
}

// -- detect ----------------------------------------------------------------

#[test]
fn detect_reports_present_with_the_version_prefix_and_executable() {
    let detection = adapter(MACPORTS_FIXTURE).detect().unwrap();
    assert_eq!(detection.presence, ProviderPresence::Present);
    assert_eq!(detection.provider.as_str(), PROVIDER_ID);
    assert_eq!(detection.version.unwrap().as_str(), "2.10.7");
    assert_eq!(detection.prefix, Some(PathBuf::from(PREFIX)));
    assert_eq!(
        detection.executable,
        Some(PathBuf::from("/opt/local/bin/port"))
    );
    // R-4 Finding 11: nothing reports an architecture for the base install.
    assert_eq!(detection.architecture, None);
}

#[test]
fn detect_reports_absent_when_the_executable_never_started() {
    let detection = adapter(ABSENT_FIXTURE).detect().unwrap();
    assert_eq!(detection.presence, ProviderPresence::Absent);
    assert_eq!(detection.version, None);
    assert_eq!(detection.prefix, None);
    assert_eq!(detection.executable, None);
}

#[test]
fn detect_reports_broken_when_the_executable_answers_with_an_error() {
    let detection = adapter(BROKEN_FIXTURE).detect().unwrap();
    assert_eq!(
        detection.presence,
        ProviderPresence::Broken {
            reason: "error.provider_unhealthy".to_string()
        }
    );
    assert_eq!(detection.version, None);
    // Broken is not absent: the prefix and executable are known.
    assert_eq!(detection.prefix, Some(PathBuf::from(PREFIX)));
    assert!(detection.executable.is_some());
}

#[test]
fn an_absent_or_broken_provider_has_an_empty_inventory_and_unknown_capabilities() {
    for name in [ABSENT_FIXTURE, BROKEN_FIXTURE] {
        let adapter = adapter(name);
        assert_eq!(adapter.inventory().unwrap(), ProviderInventory::default());
        let capabilities = adapter.capabilities().unwrap();
        assert!(capabilities
            .capabilities
            .values()
            .all(|s| *s == CapabilityState::Unknown));
        assert!(!capabilities.has(Capability::RequestedRecord));
    }
}

// -- inventory -------------------------------------------------------------

#[test]
fn every_installed_row_is_returned_and_a_name_may_have_several() {
    let inventory = adapter(MACPORTS_FIXTURE).inventory().unwrap();
    assert_eq!(inventory.installed.len(), 13, "13 rows over 12 names");
    assert_eq!(inventory.installed_names().len(), 12);

    let rows: Vec<_> = inventory
        .installed
        .iter()
        .filter(|p| p.identity.name == "openssl3")
        .collect();
    assert_eq!(rows.len(), 2, "R-3 Finding 5: multiple versions coexist");
    assert_eq!(rows.iter().filter(|p| p.active).count(), 1);
    let active = rows.iter().find(|p| p.active).unwrap();
    let inactive = rows.iter().find(|p| !p.active).unwrap();
    assert_eq!(active.version.as_str(), "3.2.1");
    assert_eq!(inactive.version.as_str(), "3.1.5");
    assert_eq!(active.revision.as_deref(), Some("0"));
    // R-4 Finding 11: the registry does not record archive provenance.
    assert_eq!(active.installed_from_prebuilt, None);
}

#[test]
fn variants_keep_the_sign_the_provider_printed() {
    let inventory = adapter(MACPORTS_FIXTURE).inventory().unwrap();
    let gnutls = inventory
        .installed
        .iter()
        .find(|p| p.identity.name == "gnutls")
        .unwrap();
    assert!(gnutls.variants.contains("+universal"));
    let wget = inventory
        .installed
        .iter()
        .find(|p| p.identity.name == "wget")
        .unwrap();
    assert!(wget.variants.contains("+gnutls"));
    assert_eq!(wget.version.as_str(), "1.25.0");
    assert_eq!(wget.revision.as_deref(), Some("1"));
}

#[test]
fn requested_comes_from_the_providers_own_record_not_from_the_graph() {
    let inventory = adapter(MACPORTS_FIXTURE).inventory().unwrap();
    assert_eq!(inventory.requested.mechanism, REQUESTED_MECHANISM);
    assert_eq!(inventory.requested.names.len(), 5);
    assert!(inventory.requested.names.contains("wget"));
    assert!(!inventory.requested.names.contains("zlib"));
    for row in &inventory.installed {
        assert_eq!(
            row.requested,
            Some(inventory.requested.names.contains(&row.identity.name))
        );
    }
}

#[test]
fn requested_and_leaves_are_the_ownership_inputs() {
    let adapter = adapter(MACPORTS_FIXTURE);
    let inventory = adapter.inventory().unwrap();
    let leaves = adapter.leaves().unwrap();
    assert_eq!(
        leaves,
        ["bzip2", "gettext-runtime", "ncurses"]
            .iter()
            .map(|s| (*s).to_string())
            .collect::<BTreeSet<String>>()
    );

    let ownership = |name: &str| {
        let row = inventory
            .installed
            .iter()
            .find(|p| p.identity.name == name)
            .unwrap();
        classify_ownership(
            row.requested,
            !inventory.reverse_dependencies_of(name).is_empty(),
            REQUESTED_MECHANISM,
        )
    };

    // A requested port with no dependents: still top level, because the
    // provider's own record wins over the graph.
    let (owner, evidence) = ownership("nginx");
    assert_eq!(owner, Ownership::TopLevel);
    assert!(evidence.authoritative);
    assert_eq!(evidence.mechanism, REQUESTED_MECHANISM);

    // Every leaf is unrequested with no dependents, so the shared rule and
    // the provider's own `leaves` answer agree.
    for leaf in &leaves {
        assert_eq!(ownership(leaf).0, Ownership::Orphan, "{leaf}");
    }

    assert_eq!(ownership("zlib").0, Ownership::Dependency);
    assert_eq!(ownership("libiconv").0, Ownership::Dependency);
}

#[test]
fn dependency_edges_are_restricted_to_installed_names_and_keep_build_only_apart() {
    let inventory = adapter(MACPORTS_FIXTURE).inventory().unwrap();

    assert_eq!(
        inventory.dependencies_of("wget"),
        ["libidn2", "openssl3", "zlib"]
            .iter()
            .map(|s| (*s).to_string())
            .collect::<BTreeSet<String>>()
    );
    // `port:postgresql17` and `port:python312` are named by the index but are
    // not installed, so they are not edges (INV-04: within one provider, and
    // only over what is installed).
    assert!(!inventory
        .dependencies_of("postgresql17-server")
        .contains("postgresql17"));
    assert!(!inventory
        .dependencies_of("py-pygments")
        .contains("python312"));

    let build_only: Vec<_> = inventory
        .dependencies
        .iter()
        .filter(|e| e.build_only)
        .collect();
    assert_eq!(build_only.len(), 1);
    assert_eq!(build_only[0].from, "wget");
    assert_eq!(build_only[0].to, "zlib");
    // The runtime edges never carry the build-only flag.
    assert!(inventory
        .dependencies
        .iter()
        .any(|e| e.from == "gnutls" && e.to == "zlib" && !e.build_only));
}

#[test]
fn reverse_dependencies_are_the_inverted_edges() {
    let inventory = adapter(MACPORTS_FIXTURE).inventory().unwrap();
    assert_eq!(
        inventory.reverse_dependencies_of("openssl3"),
        ["nginx", "postgresql17-server", "wget"]
            .iter()
            .map(|s| (*s).to_string())
            .collect::<BTreeSet<String>>()
    );
    assert_eq!(
        inventory.reverse_dependencies_of("libiconv"),
        ["gettext-runtime", "libidn2"]
            .iter()
            .map(|s| (*s).to_string())
            .collect::<BTreeSet<String>>()
    );
    assert!(inventory.reverse_dependencies_of("bzip2").is_empty());
    assert!(inventory
        .transitive_reverse_dependencies_of("libiconv")
        .contains("wget"));
}

#[test]
fn metadata_survives_the_one_variable_field_per_invocation_scheme() {
    let inventory = adapter(MACPORTS_FIXTURE).inventory().unwrap();
    let libidn2 = &inventory.metadata["libidn2"];
    assert_eq!(
        libidn2.homepage.as_deref(),
        Some("https://www.gnu.org/software/libidn/")
    );
    // A licence expression containing spaces is why it is queried last.
    assert_eq!(libidn2.license.as_deref(), Some("LGPL-3+ or GPL-2+"));
    assert_eq!(
        libidn2.description.as_deref(),
        Some("Library for internationalized domain names")
    );
    assert_eq!(
        inventory.metadata["wget"]
            .available_version
            .as_ref()
            .unwrap()
            .as_str(),
        "1.25.0_1"
    );
    assert_eq!(
        inventory.metadata["py-pygments"].replaced_by.as_deref(),
        Some("py312-pygments")
    );
    // R-3 Finding 8: `replaced_by` and `obsolete` are two independent states
    // and this adapter does not collapse them into one flag.
    assert_eq!(inventory.metadata["py-pygments"].deprecated, None);
    assert_eq!(inventory.metadata.len(), 12);
}

// -- availability ----------------------------------------------------------

#[test]
fn a_universal_variant_forces_a_source_build() {
    let assessment = adapter(MACPORTS_FIXTURE)
        .artifact_availability(&identity("gnutls"), &machine())
        .unwrap();
    assert_eq!(
        assessment.availability,
        ArtifactAvailability::SourceBuildRequired {
            reason: SourceBuildReason::NonDefaultOptions
        }
    );
    assert!(!assessment.evidence.is_empty());
    assert!(assessment.evidence.iter().all(|e| e.offline));
    assert!(assessment.evidence[0].detail.contains("+universal"));
}

#[test]
fn a_replaced_port_is_deprecated() {
    let assessment = adapter(MACPORTS_FIXTURE)
        .artifact_availability(&identity("py-pygments"), &machine())
        .unwrap();
    assert_eq!(assessment.availability, ArtifactAvailability::Deprecated);
    assert!(assessment.evidence[0].detail.contains("py312-pygments"));
}

#[test]
fn a_plain_port_is_unknown_and_says_why() {
    let assessment = adapter(MACPORTS_FIXTURE)
        .artifact_availability(&identity("zlib"), &machine())
        .unwrap();
    assert_eq!(assessment.availability, ArtifactAvailability::Unknown);
    let evidence = &assessment.evidence[0];
    assert_eq!(evidence.source, "archive_sites");
    assert!(evidence.detail.contains("requires the network"));
    assert!(evidence.detail.contains("darwin_12.x86_64"));
    assert!(evidence.offline);
}

#[test]
fn nothing_is_ever_reported_as_binary_available_offline() {
    // R-4 Finding 9: availability is not determinable offline in the
    // positive direction, so this state must never be produced here.
    let inventory = adapter(MACPORTS_FIXTURE).inventory().unwrap();
    assert_eq!(inventory.availability.len(), 12);
    for (name, assessment) in &inventory.availability {
        assert_ne!(
            assessment.availability,
            ArtifactAvailability::BinaryAvailable,
            "{name}"
        );
        assert!(!assessment.evidence.is_empty(), "{name}");
        assert!(assessment.evidence.iter().all(|e| e.offline), "{name}");
    }
}

#[test]
fn a_package_that_is_not_installed_is_unknown_rather_than_unavailable() {
    let assessment = adapter(MACPORTS_FIXTURE)
        .artifact_availability(&identity("not-installed-here"), &machine())
        .unwrap();
    assert_eq!(assessment.availability, ArtifactAvailability::Unknown);
}

#[test]
fn the_archive_query_url_is_built_but_never_fetched() {
    assert_eq!(
        archive_query_url("wget").as_deref(),
        Some("https://ports.macports.org/api/v1/builds/?port_name=wget")
    );
    assert_eq!(archive_query_url("-flag"), None);
    assert!(ARCHIVE_QUERY_URL_FORMAT.starts_with("https://ports.macports.org/api/v1/builds/"));
    // Nothing in this pass reaches the network: the fixture holds no such
    // invocation, and the adapter never runs one.
    let adapter = adapter(MACPORTS_FIXTURE);
    adapter.inventory().unwrap();
    assert!(adapter
        .runner()
        .consulted()
        .iter()
        .all(|c| !c.contains("ports.macports.org")));
}

// -- capabilities, environment, snapshot -----------------------------------

#[test]
fn capabilities_are_detected_not_assumed() {
    let capabilities = adapter(MACPORTS_FIXTURE).capabilities().unwrap();
    assert!(capabilities.has(Capability::RequestedRecord));
    assert!(capabilities.has(Capability::Dependencies));
    assert!(capabilities.has(Capability::ReverseDependencies));
    assert!(capabilities.has(Capability::OfflineMetadata));
    assert!(capabilities.has(Capability::FileOwnership));
    assert!(capabilities.has(Capability::MachineReadableInventory));
    // Only `replaced_by` is read; `obsolete` never is, so the deprecation
    // state is unprobed and is reported as unknown, never as available.
    assert_eq!(
        capabilities.capabilities[&Capability::DeprecationState],
        CapabilityState::Unknown
    );
    // R-4 Finding 9 again, as a capability rather than as a state.
    assert_eq!(
        capabilities.capabilities[&Capability::ArtifactAvailability],
        CapabilityState::Unavailable {
            reason: "network_required_ports_macports_builds_api".to_string()
        }
    );
    // No MacPorts prefix exists on the machine running this test, so the
    // service directory cannot be read and the answer is unknown, never
    // "no services".
    assert_eq!(
        capabilities.capabilities[&Capability::ServiceMetadata],
        CapabilityState::Unknown
    );
}

#[test]
fn environment_info_reports_the_documented_paths_and_the_derivation() {
    let info = adapter(MACPORTS_FIXTURE).environment_info().unwrap();
    assert_eq!(info["prefix"], PREFIX);
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
fn the_snapshot_carries_detection_capabilities_inventory_and_environment() {
    let snapshot = adapter(MACPORTS_FIXTURE).snapshot().unwrap();
    assert_eq!(snapshot.detection.presence, ProviderPresence::Present);
    assert_eq!(snapshot.inventory.installed.len(), 13);
    assert!(!snapshot.environment.is_empty());
    assert!(snapshot.capabilities.has(Capability::RequestedRecord));
}

// -- resolution ------------------------------------------------------------

#[test]
fn a_path_under_the_prefix_is_answered_by_the_provider_and_marked_authoritative() {
    let adapter = adapter(MACPORTS_FIXTURE);
    let resolution = adapter
        .resolution(Path::new("/opt/local/bin/wget"))
        .unwrap();
    assert_eq!(resolution.executable, "wget");
    assert!(resolution.authoritative);
    match resolution.owner {
        macportsbrew_domain::ExecutableOwner::Provider { provider, package } => {
            assert_eq!(provider.as_str(), PROVIDER_ID);
            assert_eq!(package.as_deref(), Some("wget"));
        }
        other => panic!("expected a provider owner, got {other:?}"),
    }
    assert!(adapter
        .runner()
        .consulted()
        .contains("port -q provides /opt/local/bin/wget"));
}

#[test]
fn a_path_under_the_prefix_with_no_provider_answer_is_not_authoritative() {
    let adapter = adapter(MACPORTS_FIXTURE);
    // The fixture records no `provides` answer for this path, which is what
    // a provider that could not say looks like.
    let resolution = adapter
        .resolution(Path::new("/opt/local/bin/curl"))
        .unwrap();
    assert!(!resolution.authoritative);
    assert_eq!(
        resolution.owner,
        macportsbrew_domain::ExecutableOwner::Provider {
            provider: ProviderId::new(PROVIDER_ID),
            package: None
        }
    );
}

#[test]
fn a_traversing_path_never_reaches_the_provider() {
    let adapter = adapter(MACPORTS_FIXTURE);
    let resolution = adapter
        .resolution(Path::new("/opt/local/bin/../../../etc/passwd"))
        .unwrap();
    assert!(!resolution.authoritative);
    assert!(!adapter
        .runner()
        .consulted()
        .iter()
        .any(|c| c.contains("passwd")));
}

#[test]
fn paths_outside_the_prefix_are_system_or_unknown_never_another_provider() {
    let adapter = adapter(MACPORTS_FIXTURE);
    assert_eq!(
        adapter
            .resolution(Path::new("/usr/bin/curl"))
            .unwrap()
            .owner,
        macportsbrew_domain::ExecutableOwner::System
    );
    // /usr/local is the other provider's prefix on Intel; this adapter must
    // not claim to know that (INV-04).
    assert_eq!(
        adapter
            .resolution(Path::new("/usr/local/bin/wget"))
            .unwrap()
            .owner,
        macportsbrew_domain::ExecutableOwner::Unknown
    );
}

#[test]
fn a_link_that_cannot_be_followed_on_this_machine_is_unknown_not_macports() {
    // A scratch prefix on this host's disk. `<prefix>/bin/broken` is a
    // dangling symlink: it may point anywhere, so the path as written is no
    // evidence that MacPorts owns what runs.
    let root = std::fs::canonicalize(macports_prefix("dangling-bin")).unwrap();
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::os::unix::fs::symlink(root.join("nowhere"), root.join("bin/broken")).unwrap();
    std::fs::write(root.join("bin/present"), "#!/bin/sh\n").unwrap();
    let adapter = adapter_on_disk(&root, &[]);

    let broken = adapter.resolution(&root.join("bin/broken")).unwrap();
    assert_eq!(broken.owner, macportsbrew_domain::ExecutableOwner::Unknown);
    assert!(!broken.authoritative);
    assert_eq!(broken.canonical_path, None);
    assert_eq!(broken.path, Some(root.join("bin/broken")));
    assert!(
        adapter
            .runner()
            .consulted()
            .iter()
            .all(|c| !c.contains("provides")),
        "a path that was not resolved is never asked about"
    );

    // The control: a file that does resolve under the prefix is still the
    // provider's, so the refusal above is about the broken link alone.
    let present = adapter.resolution(&root.join("bin/present")).unwrap();
    assert!(matches!(
        present.owner,
        macportsbrew_domain::ExecutableOwner::Provider { .. }
    ));
    std::fs::remove_dir_all(&root).ok();
}

// -- the command set -------------------------------------------------------

#[test]
fn the_adapter_consults_exactly_the_commands_it_documents() {
    let adapter = adapter(MACPORTS_FIXTURE);
    adapter.snapshot().unwrap();
    adapter
        .resolution(Path::new("/opt/local/bin/wget"))
        .unwrap();

    let expected: BTreeSet<String> = [
        "port version".to_string(),
        "port -q installed".to_string(),
        "port -q echo requested".to_string(),
        "port -q echo leaves".to_string(),
        "port -q echo active".to_string(),
        format!("port -q info --index --line --name --version --revision --replaced_by {NAMES}"),
        format!("port -q info --index --line --name --homepage --license {NAMES}"),
        format!("port -q info --index --line --name --description {NAMES}"),
        format!("port -q info --index --line --name --depends_lib {NAMES}"),
        format!("port -q info --index --line --name --depends_run {NAMES}"),
        format!("port -q info --index --line --name --depends_build {NAMES}"),
        "port -q provides /opt/local/bin/wget".to_string(),
    ]
    .into_iter()
    .collect();

    assert_eq!(adapter.runner().consulted(), expected);
}

#[test]
fn the_read_set_does_not_grow_with_the_number_of_ports() {
    let adapter = adapter(MACPORTS_FIXTURE);
    adapter.inventory().unwrap();
    assert_eq!(
        adapter.runner().consulted().len(),
        11,
        "eleven invocations, whatever the size of the installation"
    );
}

// -- failure modes ---------------------------------------------------------

#[test]
fn an_installed_line_without_a_version_tuple_fails_loudly() {
    let broken = fixture_text(MACPORTS_FIXTURE)
        .replace("wget @1.25.0_1+gnutls (active)", "wget 1.25.0_1 (active)");
    let recording = Recording::from_json(&broken).unwrap();
    let adapter = MacPortsAdapter::new(
        ReplayRunner::new(recording),
        PathBuf::from(PREFIX),
        None,
        FilesystemScope::Elsewhere,
    );
    let error = adapter.inventory().unwrap_err();
    assert_eq!(error.id, ErrorId::ProviderOutputChanged);
    assert!(error.detail.contains("wget"));
}

#[test]
fn an_installed_listing_that_disagrees_with_the_active_set_fails_loudly() {
    // The `(active)` marker's exact rendering is UNVERIFIED (R-3), so the
    // adapter cross-checks it against `port -q echo active` rather than
    // trusting one source silently.
    let broken = fixture_text(MACPORTS_FIXTURE).replace(
        "  wget @1.25.0_1+gnutls (active)",
        "  wget @1.25.0_1+gnutls",
    );
    let recording = Recording::from_json(&broken).unwrap();
    let adapter = MacPortsAdapter::new(
        ReplayRunner::new(recording),
        PathBuf::from(PREFIX),
        None,
        FilesystemScope::Elsewhere,
    );
    let error = adapter.inventory().unwrap_err();
    assert_eq!(error.id, ErrorId::ProviderOutputChanged);
    assert!(error.detail.contains("wget"));
}

// -- determinism -----------------------------------------------------------

#[test]
fn the_inventory_serialises_identically_twice_and_across_adapters() {
    let first = adapter(MACPORTS_FIXTURE);
    let once = serde_json::to_string(&first.inventory().unwrap()).unwrap();
    let twice = serde_json::to_string(&first.inventory().unwrap()).unwrap();
    assert_eq!(once, twice);

    let second = adapter(MACPORTS_FIXTURE);
    assert_eq!(
        once,
        serde_json::to_string(&second.inventory().unwrap()).unwrap()
    );
    assert_eq!(
        serde_json::to_string(&first.snapshot().unwrap()).unwrap(),
        serde_json::to_string(&second.snapshot().unwrap()).unwrap()
    );
}

// -- service detection -----------------------------------------------------

/// A scratch MacPorts prefix: `etc/macports` and nothing else.
fn macports_prefix(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "macportsbrew-service-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(root.join("etc/macports")).unwrap();
    root
}

/// A default-named StartupItem for `port` under `location`.
fn item(root: &Path, location: &str, port: &str) {
    let dir = root.join(location).join(format!("org.macports.{port}"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("org.macports.{port}.plist")), "<plist/>").unwrap();
}

fn active(names: &[&str]) -> BTreeMap<String, bool> {
    names.iter().map(|n| ((*n).to_string(), true)).collect()
}

/// A change made to a scratch prefix before scanning it.
type TreeChange = Box<dyn Fn(&Path)>;

fn scan_of(root: &Path, installed: &BTreeMap<String, bool>) -> service::ServiceScan {
    service::scan(
        root,
        FilesystemScope::ThisMachine,
        installed,
        &registry_agrees,
    )
}

/// A registry that names each item's port after the item itself, as MacPorts'
/// does for a port that keeps the default `startupitem.name`.
fn registry_agrees(plist: &Path) -> Option<String> {
    let file = plist.file_name()?.to_str()?;
    Some(
        file.strip_prefix("org.macports.")?
            .strip_suffix(".plist")?
            .to_string(),
    )
}

#[test]
fn a_service_is_detected_from_the_launch_machinery_inside_the_prefix() {
    // R-8 E-7; portstartupitem.tcl 2.12.6: an item is
    // ${prefix}/etc/<location>/org.macports.<name>/org.macports.<name>.plist,
    // under LaunchDaemons or LaunchAgents.
    let root = macports_prefix("detected");
    item(&root, "etc/LaunchDaemons", "nginx");
    item(&root, "etc/LaunchAgents", "syncthing");
    let scan = scan_of(&root, &active(&["nginx", "syncthing", "wget"]));
    assert!(scan.is_known());
    assert!(scan.defines_service("nginx"));
    assert!(
        scan.defines_service("syncthing"),
        "a LaunchAgents item is a service"
    );
    assert!(!scan.defines_service("wget"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_prefix_with_no_startup_item_is_known_to_define_none() {
    // The ordinary install: no port has an item, so neither location exists.
    // Before pass 08 slice 3 this was "unknown", which made MacPorts unplannable
    // as a source on every such machine.
    let root = macports_prefix("none");
    let scan = scan_of(&root, &active(&["jq", "htop"]));
    assert_eq!(scan, service::ServiceScan::Scanned(BTreeSet::new()));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn invariant_19_a_service_under_a_custom_uniquename_is_never_reported_serviceless() {
    // dbus's Portfile names its items org.freedesktop.dbus-system and
    // org.freedesktop.dbus-session; nothing ties them to a port by name, so the
    // whole answer is unknown rather than "dbus defines none".
    let root = macports_prefix("custom-uniquename");
    let dir = root.join("etc/LaunchDaemons/org.freedesktop.dbus-system");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("org.freedesktop.dbus-system.plist"), "<plist/>").unwrap();
    let scan = scan_of(&root, &active(&["dbus"]));
    assert!(!scan.is_known());
    assert!(!scan.defines_service("dbus"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn anything_the_scan_cannot_attribute_makes_the_answer_unknown() {
    let cases: Vec<(&str, TreeChange)> = vec![
        (
            "a directory without its plist",
            Box::new(|r: &Path| {
                std::fs::create_dir_all(r.join("etc/LaunchDaemons/org.macports.nginx")).unwrap();
            }),
        ),
        (
            "a plist renamed inside its directory",
            Box::new(|r: &Path| {
                let d = r.join("etc/LaunchDaemons/org.macports.nginx");
                std::fs::create_dir_all(&d).unwrap();
                std::fs::write(d.join("nginx.plist"), "<plist/>").unwrap();
            }),
        ),
        (
            "a stray plist in the location",
            Box::new(|r: &Path| {
                std::fs::create_dir_all(r.join("etc/LaunchAgents")).unwrap();
                std::fs::write(r.join("etc/LaunchAgents/stray.plist"), "<plist/>").unwrap();
            }),
        ),
        (
            "an item for a port that is not installed",
            Box::new(|r: &Path| {
                item(r, "etc/LaunchDaemons", "postgresql17-server");
            }),
        ),
        (
            "a location that is a symbolic reference",
            Box::new(|r: &Path| {
                std::fs::create_dir_all(r.join("elsewhere")).unwrap();
                std::os::unix::fs::symlink(r.join("elsewhere"), r.join("etc/LaunchDaemons"))
                    .unwrap();
            }),
        ),
        (
            "a location that is a dangling symbolic reference",
            Box::new(|r: &Path| {
                std::os::unix::fs::symlink(r.join("nowhere"), r.join("etc/LaunchAgents")).unwrap();
            }),
        ),
        (
            "a location that is a file",
            Box::new(|r: &Path| {
                std::fs::write(r.join("etc/LaunchDaemons"), "x").unwrap();
            }),
        ),
    ];
    for (what, make) in cases {
        let root = macports_prefix("unattributable");
        make(&root);
        let scan = scan_of(&root, &active(&["nginx", "jq"]));
        assert!(!scan.is_known(), "{what} was read as a known answer");
        std::fs::remove_dir_all(&root).ok();
    }
}

#[test]
fn invariant_19_an_inactive_service_port_is_not_reported_as_serviceless() {
    // Deactivation deletes the item directory, so with an inactive port the
    // absent location proves nothing about it.
    let root = macports_prefix("inactive");
    let mut installed = active(&["jq"]);
    installed.insert("postgresql17-server".to_string(), false);
    assert!(!scan_of(&root, &installed).is_known());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_macports_directory_that_is_a_symbolic_reference_is_not_a_prefix() {
    let root = macports_prefix("symlinked-etc-macports");
    std::fs::remove_dir_all(root.join("etc/macports")).unwrap();
    std::fs::create_dir_all(root.join("real")).unwrap();
    std::os::unix::fs::symlink(root.join("real"), root.join("etc/macports")).unwrap();
    assert!(!scan_of(&root, &active(&["jq"])).is_known());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_tree_that_is_not_a_macports_prefix_is_unknown_not_empty() {
    let root = macports_prefix("not-macports");
    std::fs::remove_dir_all(root.join("etc/macports")).unwrap();
    assert!(!scan_of(&root, &active(&["jq"])).is_known());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn an_unreadable_service_directory_is_unknown_not_empty() {
    let scan = service::scan(
        Path::new("/opt/local-that-does-not-exist"),
        FilesystemScope::ThisMachine,
        &active(&["nginx"]),
        &registry_agrees,
    );
    assert!(!scan.is_known());
    assert!(!scan.defines_service("nginx"));
}

// -- prefix discovery ------------------------------------------------------

#[test]
fn prefix_discovery_looks_for_the_executable_under_each_candidate() {
    assert_eq!(
        MacPortsAdapter::<ReplayRunner>::detect_prefix(
            &[
                PathBuf::from("/opt/local-that-does-not-exist"),
                PathBuf::from("/another-that-does-not-exist"),
            ],
            FilesystemScope::ThisMachine,
        ),
        None
    );
    assert_eq!(
        macportsbrew_adapters::registry::candidate_prefixes(&ProviderId::new("macports")),
        vec![PathBuf::from(PREFIX)]
    );
}

#[test]
fn a_variant_set_alone_does_not_prove_a_source_build() {
    // R-4 E-5: `wget-1.25.0_1+gnutls...` is published by the build farm, so
    // `+gnutls` is a default variant and proves nothing offline.
    let plain: BTreeSet<String> = ["+gnutls".to_string()].into_iter().collect();
    assert!(availability::forced_source_build(&plain).is_none());
    let universal: BTreeSet<String> = ["+universal".to_string()].into_iter().collect();
    assert!(availability::forced_source_build(&universal).is_some());
    let negated: BTreeSet<String> = ["-x11".to_string()].into_iter().collect();
    assert!(availability::forced_source_build(&negated).is_some());
}

#[test]
fn the_environment_carries_home_because_port_cannot_start_without_it() {
    // F-37, and the most expensive assumption in this adapter's history.
    //
    // `environment()` used to hardcode `home: None`, reasoning that "no read
    // here is documented to need it". `port` runs `mportinit` before any
    // subcommand, and `mportinit` resolves the user's home through
    // `dscl . -read /Users/$(id -un) NFSHomeDirectory`. With no `HOME` that
    // pipeline fails and **every** `port` invocation exits 1 — so live
    // detection reported a healthy MacPorts 2.12.6 as `Broken`, and had done
    // since the adapter was written. Nothing caught it because no test and no
    // fixture ever ran the real binary under the real executor environment:
    // `scripts/capture-fixture.sh` sets `HOME`, so recordings show a MacPorts
    // that works and the live path could not reproduce them.
    //
    // Asserted on the *sanitised environment*, not on the spec field, because
    // the defect is what reaches the process. `EnvironmentSpec::home` being
    // `Some` is worth nothing if `sanitised_environment` drops it.
    let with_home = MacPortsAdapter::new(
        ReplayRunner::new(
            Recording::from_json(&fixture_text("macports-only-intel-macos12"))
                .expect("fixture parses"),
        ),
        PathBuf::from(PREFIX),
        Some(PathBuf::from("/Users/example")),
        FilesystemScope::Elsewhere,
    );
    let env = sanitised_environment(&with_home.environment());
    assert_eq!(
        env.get("HOME").map(String::as_str),
        Some("/Users/example"),
        "port cannot initialise without HOME, so the environment must carry it: {env:?}"
    );

    // And the replay case, which is the reason the field is an `Option` at all:
    // under replay nothing is executed and there is no user whose home it would
    // be, so omitting it is correct rather than a smaller version of the bug.
    let replayed = MacPortsAdapter::new(
        ReplayRunner::new(
            Recording::from_json(&fixture_text("macports-only-intel-macos12"))
                .expect("fixture parses"),
        ),
        PathBuf::from(PREFIX),
        None,
        FilesystemScope::Elsewhere,
    );
    assert!(
        !sanitised_environment(&replayed.environment()).contains_key("HOME"),
        "with no home known, HOME is omitted rather than invented"
    );
}

/// The MacPorts fixture's adapter, reading a scratch prefix on this host's
/// disk, with `port -q provides <plist>` answering `owner` for each `item`.
fn adapter_on_disk(root: &Path, items: &[(&str, &str, &str)]) -> MacPortsAdapter<ReplayRunner> {
    let mut doc: serde_json::Value =
        serde_json::from_str(&fixture_text(MACPORTS_FIXTURE)).expect("fixture JSON");
    let invocations = doc["invocations"].as_array_mut().expect("invocations");
    let template = invocations
        .iter()
        .find(|i| i["argv"][1] == "provides")
        .expect("the fixture records a provides read")
        .clone();
    for (location, item_name, owner) in items {
        let plist = root
            .join(location)
            .join(format!("org.macports.{item_name}"))
            .join(format!("org.macports.{item_name}.plist"));
        let plist = plist.to_str().expect("UTF-8");
        let mut invocation = template.clone();
        invocation["argv"] = serde_json::json!(["-q", "provides", plist]);
        invocation["stdout"] = serde_json::json!(format!("{plist} is provided by: {owner}\n"));
        invocations.push(invocation);
    }
    let recording = Recording::from_json(&doc.to_string()).expect("recording");
    MacPortsAdapter::new(
        ReplayRunner::new(recording),
        root.to_path_buf(),
        None,
        FilesystemScope::ThisMachine,
    )
}

fn service_capability(adapter: &MacPortsAdapter<ReplayRunner>) -> CapabilityState {
    adapter
        .capabilities()
        .unwrap()
        .capabilities
        .get(&Capability::ServiceMetadata)
        .cloned()
        .expect("the capability is reported")
}

fn defines_service(adapter: &MacPortsAdapter<ReplayRunner>, name: &str) -> bool {
    adapter
        .inventory()
        .unwrap()
        .metadata
        .get(name)
        .is_some_and(|m| m.defines_service)
}

#[test]
fn invariant_19_an_item_is_attributed_to_the_port_the_registry_names() {
    // A subport may set `startupitem.name` to its parent's name: `mysql5-server`
    // writes `org.macports.mysql5`, and `mysql5` is installed beside it. The
    // item's name alone would make the client the service and the server not
    // one, so the scan asks the registry who installed the plist (security
    // review, pass 08 slice 3, M1). Here `org.macports.nginx` is owned by
    // `wget`: wget is the service, nginx is not.
    let root = macports_prefix("registry-owner");
    item(&root, "etc/LaunchDaemons", "nginx");
    let adapter = adapter_on_disk(&root, &[("etc/LaunchDaemons", "nginx", "wget")]);
    assert_eq!(service_capability(&adapter), CapabilityState::Available);
    assert!(defines_service(&adapter, "wget"));
    assert!(!defines_service(&adapter, "nginx"));

    // A port may name its item outside `org.macports.` (`dbus`'s
    // `org.freedesktop.dbus-system`): the registry still attributes it.
    let named = macports_prefix("registry-named");
    let dir = named.join("etc/LaunchAgents/org.freedesktop.agent");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("org.freedesktop.agent.plist"), "<plist/>").unwrap();
    let mut doc: serde_json::Value =
        serde_json::from_str(&fixture_text(MACPORTS_FIXTURE)).expect("fixture JSON");
    let invocations = doc["invocations"].as_array_mut().expect("invocations");
    let mut answer = invocations
        .iter()
        .find(|i| i["argv"][1] == "provides")
        .expect("a provides read")
        .clone();
    let plist = dir.join("org.freedesktop.agent.plist");
    let plist = plist.to_str().expect("UTF-8");
    answer["argv"] = serde_json::json!(["-q", "provides", plist]);
    answer["stdout"] = serde_json::json!(format!("{plist} is provided by: postgresql17-server\n"));
    invocations.push(answer);
    let adapter = MacPortsAdapter::new(
        ReplayRunner::new(Recording::from_json(&doc.to_string()).expect("recording")),
        named.clone(),
        None,
        FilesystemScope::ThisMachine,
    );
    assert_eq!(service_capability(&adapter), CapabilityState::Available);
    assert!(defines_service(&adapter, "postgresql17-server"));

    // The registry names a port that is not installed, or cannot say: unknown.
    let stranger = adapter_on_disk(&root, &[("etc/LaunchDaemons", "nginx", "mysql5")]);
    assert_eq!(service_capability(&stranger), CapabilityState::Unknown);
    let silent = adapter_on_disk(&root, &[]);
    assert_eq!(service_capability(&silent), CapabilityState::Unknown);
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&named).ok();
}

#[test]
fn the_service_capability_comes_from_the_scan_the_metadata_came_from() {
    // One scan per capture: the capability is read from the scan that built
    // the inventory, so a change between two scans cannot leave "known"
    // beside per-package answers from an unknown scan (security review and
    // spec-guardian, pass 08 slice 3, M2).
    let root = macports_prefix("one-scan");
    item(&root, "etc/LaunchDaemons", "nginx");
    let adapter = adapter_on_disk(&root, &[("etc/LaunchDaemons", "nginx", "nginx")]);
    assert!(defines_service(&adapter, "nginx"));
    // The tree changes after the inventory was built: a stray entry that a
    // second scan would refuse to attribute.
    std::fs::create_dir_all(root.join("etc/LaunchDaemons/org.example.stray")).unwrap();
    assert_eq!(service_capability(&adapter), CapabilityState::Available);
    assert!(defines_service(&adapter, "nginx"));

    // And the other order: an inventory built over the stray entry is unknown
    // throughout, its capability included.
    let fresh = adapter_on_disk(&root, &[("etc/LaunchDaemons", "nginx", "nginx")]);
    assert!(!defines_service(&fresh, "nginx"));
    assert_eq!(service_capability(&fresh), CapabilityState::Unknown);
    std::fs::remove_dir_all(&root).ok();
}
