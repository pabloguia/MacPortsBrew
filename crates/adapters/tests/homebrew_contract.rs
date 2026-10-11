// SPDX-License-Identifier: Apache-2.0

//! The Homebrew read adapter against a captured machine.
//!
//! Every assertion here is checked against
//! `fixtures/recordings/homebrew-only-intel-macos26.json`, a recording of the
//! read-only commands on an Intel Mac running macOS 26.6.2 with Homebrew
//! 6.0.21 and no MacPorts. Nothing in this file runs a provider
//! (`07-quality-and-testing.md`: fixtures are captured, tests never touch a
//! live installation), and the numbers come from `docs/research/R-1` and
//! `docs/research/R-2`, which were written from the same machine.

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::homebrew::{self, detect_prefix, HomebrewAdapter, PROVIDER_ID};
use macportsbrew_adapters::process::{
    ExitStatus, Invocation, ProcessRunner, RecordedInvocation, Recording, RecordingProvenance,
    ReplayRunner,
};
use macportsbrew_domain::availability::SourceBuildReason;
use macportsbrew_domain::machine::CommandLineTools;
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, Capability, CapabilityState, ErrorId, ExecutableOwner,
    Machine, MacosVersion, PackageIdentity, PackageKind, ProviderId, ProviderPresence,
    ProviderRead, Translation,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const FIXTURE: &str = include_str!("../../../fixtures/recordings/homebrew-only-intel-macos26.json");

/// The tags this Intel macOS 26 machine accepts, most specific first. The
/// caller computes these from dated policy data, never the adapter (R-2
/// finding 1); the older-release fallback is why `sonoma` is in the list.
fn platform_tags() -> Vec<String> {
    ["tahoe", "sequoia", "sonoma", "ventura", "all"]
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn prefix() -> PathBuf {
    PathBuf::from("/usr/local")
}

fn recording() -> Recording {
    Recording::from_json(FIXTURE).expect("the captured fixture parses")
}

fn adapter(runner: &dyn ProcessRunner) -> HomebrewAdapter<'_> {
    // The fixture recording is the machine; this host's disk is not it.
    HomebrewAdapter::new(
        runner,
        prefix(),
        None,
        platform_tags(),
        FilesystemScope::Elsewhere,
    )
}

fn machine() -> Machine {
    Machine {
        architecture: Architecture::X86_64,
        macos: MacosVersion::new(26, 6, 2),
        translation: Translation::Native,
        command_line_tools: CommandLineTools::Unknown,
    }
}

fn identity(name: &str) -> PackageIdentity {
    PackageIdentity::new(ProviderId::new(PROVIDER_ID), name)
}

/// The seven formulae whose newest keg was built from source because no
/// Intel artifact was offered (R-1 finding 6, R-2 finding 9).
const SOURCE_BUILT: &[&str] = &[
    "mise",
    "openssl@3",
    "pcre2",
    "php@8.4",
    "pydantic",
    "simdjson",
    "tree-sitter",
];

// ---------------------------------------------------------------------------
// Detection and capabilities
// ---------------------------------------------------------------------------

#[test]
fn detection_reports_the_recorded_installation() {
    let runner = ReplayRunner::new(recording());
    let detection = adapter(&runner).detect().unwrap();
    assert_eq!(detection.presence, ProviderPresence::Present);
    assert_eq!(detection.provider.as_str(), "homebrew");
    assert_eq!(detection.version.unwrap().as_str(), "6.0.21");
    assert_eq!(detection.prefix.unwrap(), prefix());
    assert_eq!(
        detection.executable.unwrap(),
        PathBuf::from("/usr/local/bin/brew")
    );
    assert_eq!(detection.architecture, Some(Architecture::X86_64));
}

#[test]
fn an_executable_that_cannot_be_started_detects_as_absent() {
    let runner = ReplayRunner::new(synthetic(&[(
        &["--version"],
        "",
        ExitStatus::NotStarted {
            error: "No such file or directory (os error 2)".into(),
        },
    )]));
    let detection = adapter(&runner).detect().unwrap();
    assert_eq!(detection.presence, ProviderPresence::Absent);
    assert!(detection.version.is_none());
    assert!(detection.prefix.is_none());
    assert!(detection.executable.is_none());
}

#[test]
fn an_executable_that_answers_with_an_error_detects_as_broken() {
    let runner = ReplayRunner::new(synthetic(&[(
        &["--version"],
        "",
        ExitStatus::Exited { code: 1 },
    )]));
    let detection = adapter(&runner).detect().unwrap();
    match detection.presence {
        ProviderPresence::Broken { reason } => assert_eq!(reason, "error.provider_unhealthy"),
        other => panic!("expected a broken state, got {other:?}"),
    }
    assert!(detection.version.is_none());
    // A broken state still names what was inspected, so `doctor` can point.
    assert_eq!(detection.prefix, Some(prefix()));
}

#[test]
fn capabilities_are_detected_from_the_document_and_never_assumed() {
    let runner = ReplayRunner::new(recording());
    let capabilities = adapter(&runner).capabilities().unwrap();
    for capability in [
        Capability::MachineReadableInventory,
        Capability::RequestedRecord,
        Capability::Dependencies,
        Capability::ReverseDependencies,
        Capability::ArtifactAvailability,
        Capability::DeprecationState,
        Capability::ServiceMetadata,
    ] {
        assert!(capabilities.has(capability), "expected {capability:?}");
    }
    // Homebrew has no "which package owns this file" query, and with no home
    // directory supplied the cached index cannot be looked for. Neither is
    // reported as absent.
    assert_eq!(
        capabilities.capabilities[&Capability::FileOwnership],
        CapabilityState::Unknown
    );
    assert_eq!(
        capabilities.capabilities[&Capability::OfflineMetadata],
        CapabilityState::Unknown
    );
}

#[test]
fn the_offline_metadata_capability_follows_the_cached_index() {
    let home = scratch("offline-metadata");
    let index = home.join("Library/Caches/Homebrew/api/internal");
    std::fs::create_dir_all(&index).unwrap();
    let runner = ReplayRunner::new(recording());

    let absent = HomebrewAdapter::new(
        &runner,
        prefix(),
        Some(home.clone()),
        platform_tags(),
        FilesystemScope::ThisMachine,
    );
    assert!(matches!(
        absent.capabilities().unwrap().capabilities[&Capability::OfflineMetadata],
        CapabilityState::Unavailable { .. }
    ));

    std::fs::write(index.join("packages.tahoe.jws.json"), "{}").unwrap();
    let runner = ReplayRunner::new(recording());
    let present = HomebrewAdapter::new(
        &runner,
        prefix(),
        Some(home.clone()),
        platform_tags(),
        FilesystemScope::ThisMachine,
    );
    assert_eq!(
        present.capabilities().unwrap().capabilities[&Capability::OfflineMetadata],
        CapabilityState::Available
    );
    std::fs::remove_dir_all(&home).ok();
}

// ---------------------------------------------------------------------------
// Inventory
// ---------------------------------------------------------------------------

#[test]
fn inventory_counts_match_the_captured_machine() {
    let runner = ReplayRunner::new(recording());
    let inventory = adapter(&runner).inventory().unwrap();

    let formulae: Vec<_> = inventory
        .installed
        .iter()
        .filter(|p| p.kind == PackageKind::Package)
        .collect();
    let casks: Vec<_> = inventory
        .installed
        .iter()
        .filter(|p| p.kind == PackageKind::GuiApp)
        .collect();
    // 180 formulae over 192 kegs, and 9 casks (R-1 §E-1).
    assert_eq!(formulae.len(), 192, "one row per keg, not per formula");
    assert_eq!(
        formulae
            .iter()
            .map(|p| p.identity.name.clone())
            .collect::<BTreeSet<_>>()
            .len(),
        180
    );
    assert_eq!(casks.len(), 9);
    assert_eq!(inventory.installed.len(), 201);

    // Homebrew's own record of user intent, not an inference (R-1 finding 4).
    assert_eq!(inventory.requested.names.len(), 47);
    assert_eq!(inventory.requested.mechanism, "installed_on_request");
    assert!(inventory.requested.names.contains("wget"));
}

#[test]
fn the_active_row_is_the_linked_keg_or_the_newest_one() {
    let runner = ReplayRunner::new(recording());
    let inventory = adapter(&runner).inventory().unwrap();
    let rows = |name: &str| -> Vec<_> {
        inventory
            .installed
            .iter()
            .filter(|p| p.identity.name == name)
            .collect()
    };

    // Two kegs, one on the default path.
    let pcre2 = rows("pcre2");
    assert_eq!(pcre2.len(), 2);
    let active: Vec<_> = pcre2.iter().filter(|p| p.active).collect();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].version.as_str(), "10.48");

    // Two kegs and nothing on the default path: the newest is active. Reading
    // element zero would report the older, prebuilt keg (R-1 finding 7).
    let php = rows("php@8.4");
    assert_eq!(php.len(), 2);
    let active: Vec<_> = php.iter().filter(|p| p.active).collect();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].version.as_str(), "8.4.25");
    assert_eq!(active[0].installed_from_prebuilt, Some(false));

    let wget = rows("wget");
    assert_eq!(wget.len(), 1);
    assert!(wget[0].active);
    assert_eq!(wget[0].requested, Some(true));
    assert_eq!(wget[0].installed_from_prebuilt, Some(true));
    assert_eq!(
        wget[0].install_path.as_deref(),
        Some(Path::new("/usr/local/Cellar/wget/1.25.0"))
    );
}

#[test]
fn casks_are_reported_as_applications_and_carry_no_record_of_intent() {
    let runner = ReplayRunner::new(recording());
    let inventory = adapter(&runner).inventory().unwrap();
    let gimp = inventory
        .installed
        .iter()
        .find(|p| p.identity.name == "gimp")
        .expect("the captured machine has a gimp cask");
    assert_eq!(gimp.kind, PackageKind::GuiApp);
    assert_eq!(gimp.version.as_str(), "3.2.4");
    // The cask JSON has no intent field (R-1 finding 4, re-verified
    // 2026-09-04). Homebrew records one in the on-disk receipt, which this
    // adapter reads only where the filesystem is the machine, and this test
    // replays: the recording carries no receipt. `gimp` is also one of the two
    // casks on the captured machine that has no receipt at all (R-12).
    assert_eq!(gimp.requested, None);
    assert_eq!(
        gimp.install_path.as_deref(),
        Some(Path::new("/usr/local/Caskroom/gimp/3.2.4"))
    );
    // No cask is ever counted as requested.
    assert!(!inventory.requested.names.contains("gimp"));

    let metadata = &inventory.metadata["gimp"];
    assert_eq!(
        metadata.description.as_deref(),
        Some("Free and open-source image editor")
    );
    // homebrew/cask is the core catalogue, not a third-party origin.
    assert_eq!(metadata.origin, None);

    // A cask that updates itself outside the provider says so.
    assert!(inventory.metadata["visualdiffer"]
        .notes
        .as_deref()
        .unwrap_or_default()
        .contains("vendor"));
}

#[test]
fn dependency_edges_stay_inside_the_provider_and_answer_both_directions() {
    let runner = ReplayRunner::new(recording());
    let inventory = adapter(&runner).inventory().unwrap();
    let installed = inventory.installed_names();

    // Every edge names packages that are actually present (INV-04).
    for edge in &inventory.dependencies {
        assert!(installed.contains(&edge.from), "{}", edge.from);
        assert!(installed.contains(&edge.to), "{}", edge.to);
    }
    // Sorted and deduplicated, so the serialised form is stable.
    let mut sorted = inventory.dependencies.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted, inventory.dependencies);

    let dependents = inventory.reverse_dependencies_of("pcre2");
    assert!(dependents.len() > 5, "{dependents:?}");
    for expected in ["git", "php@8.4", "httpd"] {
        assert!(dependents.contains(expected), "{expected} depends on pcre2");
    }
    assert!(inventory.dependencies_of("wget").contains("libidn2"));

    // A build-time-only edge is marked as such rather than dropped.
    assert!(inventory
        .dependencies
        .iter()
        .any(|e| e.build_only && e.to == "pkgconf"));
}

#[test]
fn metadata_carries_the_facts_a_migration_decision_needs() {
    let runner = ReplayRunner::new(recording());
    let inventory = adapter(&runner).inventory().unwrap();

    let php = &inventory.metadata["php@8.4"];
    assert!(php.defines_service, "php@8.4 defines a service (R-8)");
    assert!(!php.versioned_siblings.is_empty());
    assert!(php.versioned_siblings.contains("php@8.3"));
    // Kept off the default path by design, with the provider's own reason.
    assert_eq!(
        php.not_on_default_path_reason.as_deref(),
        Some(":versioned_formula")
    );
    assert!(php.notes.is_some(), "php@8.4 has caveats");

    // 14 of the 19 keg-only formulae carry only a symbol; one carries prose.
    assert_eq!(
        inventory.metadata["apr"]
            .not_on_default_path_reason
            .as_deref(),
        Some("Apple's CLT provides apr")
    );
    // A formula that is on the default path has no such reason.
    assert_eq!(inventory.metadata["wget"].not_on_default_path_reason, None);

    let wget = &inventory.metadata["wget"];
    assert_eq!(wget.description.as_deref(), Some("Internet file retriever"));
    assert_eq!(wget.license.as_deref(), Some("GPL-3.0-or-later"));
    assert_eq!(wget.available_version.as_ref().unwrap().as_str(), "1.25.0");
    assert_eq!(wget.origin, None, "homebrew/core is not a third-party tap");
    assert!(!wget.pinned, "nothing is pinned on the captured machine");

    let deprecated = &inventory.metadata["gemini-cli"];
    assert_eq!(deprecated.deprecated.as_deref(), Some("unsupported"));
    assert_eq!(deprecated.replaced_by.as_deref(), Some("antigravity-cli"));
    assert_eq!(deprecated.disabled, None);
}

// ---------------------------------------------------------------------------
// Artifact availability
// ---------------------------------------------------------------------------

#[test]
fn a_matching_tag_is_a_prebuilt_artifact_and_names_the_tag() {
    let runner = ReplayRunner::new(recording());
    let adapter = adapter(&runner);
    let inventory = adapter.inventory().unwrap();

    for (name, tag) in [
        ("wget", "sonoma"),
        ("coreutils", "tahoe"),
        ("ca-certificates", "all"),
    ] {
        let assessed = &inventory.availability[name];
        assert_eq!(
            assessed.availability,
            ArtifactAvailability::BinaryAvailable,
            "{name}"
        );
        let evidence = assessed
            .evidence
            .iter()
            .find(|e| e.source == "bottle.stable.files")
            .unwrap_or_else(|| panic!("{name} has no artifact evidence"));
        assert_eq!(evidence.detail, tag, "{name}");
        assert!(evidence.offline, "the answer needed no network");
        assert!(assessed.cached, "it came from the provider's own cache");
    }

    // The same answer through the per-package entry point.
    let direct = adapter
        .artifact_availability(&identity("wget"), &machine())
        .unwrap();
    assert_eq!(direct, inventory.availability["wget"]);
}

#[test]
fn no_matching_tag_is_a_source_build_and_says_what_it_cannot_tell() {
    let runner = ReplayRunner::new(recording());
    let inventory = adapter(&runner).inventory().unwrap();

    for name in SOURCE_BUILT {
        let assessed = &inventory.availability[*name];
        assert_eq!(
            assessed.availability,
            ArtifactAvailability::SourceBuildRequired {
                reason: SourceBuildReason::NoArtifactForThisPlatform
            },
            "{name}"
        );
        // The evidence must not let a caller conclude "nothing is published":
        // the cached index cannot separate that from "none for this platform"
        // (R-1 finding 6, R-2 finding 5).
        assert!(
            assessed
                .evidence
                .iter()
                .any(|e| e.detail.contains("cannot separate")),
            "{name} is missing the caveat"
        );
    }

    // The captured machine carries 18 formulae with no artifact for it: the
    // seven above, which were rebuilt from source, plus eleven that were
    // poured before the artifact stopped being offered (R-2 §E3).
    let source_builds = inventory
        .availability
        .values()
        .filter(|a| {
            matches!(
                a.availability,
                ArtifactAvailability::SourceBuildRequired { .. }
            )
        })
        .count();
    assert_eq!(source_builds, 18);
}

#[test]
fn a_deprecated_formula_reports_the_state_and_keeps_the_artifact_fact() {
    let runner = ReplayRunner::new(recording());
    let inventory = adapter(&runner).inventory().unwrap();
    let assessed = &inventory.availability["gemini-cli"];
    assert_eq!(assessed.availability, ArtifactAvailability::Deprecated);
    assert!(assessed
        .evidence
        .iter()
        .any(|e| e.source == "deprecated" && e.detail == "unsupported"));
    // The artifact fact is not lost just because the state outranks it.
    assert!(assessed
        .evidence
        .iter()
        .any(|e| e.source == "bottle.stable.files" && e.detail == "sonoma"));
}

#[test]
fn a_cask_is_out_of_scope_rather_than_assessed() {
    let runner = ReplayRunner::new(recording());
    let inventory = adapter(&runner).inventory().unwrap();
    let assessed = &inventory.availability["gimp"];
    assert_eq!(assessed.availability, ArtifactAvailability::Unknown);
    assert!(assessed.evidence[0]
        .detail
        .contains("out of migration scope"));
}

#[test]
fn a_package_this_provider_does_not_have_is_unknown_not_negative() {
    let runner = ReplayRunner::new(recording());
    let assessed = adapter(&runner)
        .artifact_availability(&identity("not-a-real-package"), &machine())
        .unwrap();
    assert_eq!(assessed.availability, ArtifactAvailability::Unknown);
    assert!(!assessed.evidence.is_empty());
}

// ---------------------------------------------------------------------------
// Shape assertion, command surface, determinism
// ---------------------------------------------------------------------------

#[test]
fn a_document_missing_a_required_key_fails_loudly() {
    let mut mutated = recording();
    let info = mutated
        .invocations
        .iter_mut()
        .find(|r| r.invocation.argv == ["info", "--json=v2", "--installed"])
        .expect("the fixture records the inventory command");
    // Remove the key from the first formula only: a silent misparse of one
    // row is exactly the data-loss-class bug 07-quality-and-testing.md names.
    let before = info.invocation.stdout.clone();
    info.invocation.stdout = before.replacen("\"installed\": [", "\"absent\": [", 1);
    assert_ne!(info.invocation.stdout, before, "the fixture shape changed");

    let runner = ReplayRunner::new(mutated);
    let adapter = adapter(&runner);
    let error = adapter.inventory().unwrap_err();
    assert_eq!(error.id, ErrorId::ProviderOutputChanged);
    assert!(error.detail.contains("installed"), "{}", error.detail);
    assert_eq!(error.exit_code(), 5);

    // The provider still reports what it can: the capability table degrades
    // with a named reason rather than the adapter crashing.
    let capabilities = adapter.capabilities().unwrap();
    assert_eq!(
        capabilities.capabilities[&Capability::MachineReadableInventory],
        CapabilityState::Unavailable {
            reason: "error.provider_output_changed".to_string()
        }
    );
}

/// The captured recording with one invocation's stdout rewritten in memory -
/// a labelled mutation of a captured document, not an invented fixture.
fn with_stdout(argv: &[&str], rewrite: impl FnOnce(&str) -> String) -> Recording {
    let mut mutated = recording();
    let record = mutated
        .invocations
        .iter_mut()
        .find(|r| r.invocation.argv == argv)
        .expect("the fixture records the command");
    let before = record.invocation.stdout.clone();
    record.invocation.stdout = rewrite(&before);
    assert_ne!(
        record.invocation.stdout, before,
        "the fixture shape changed"
    );
    mutated
}

#[test]
fn a_formula_row_missing_its_service_key_fails_rather_than_reading_as_no_service() {
    // Every formula row in the capture carries `service` (R-8). Drop the key
    // from one row while the five non-null blocks elsewhere still mark
    // ServiceMetadata Available: before this, that row read as "defines no
    // service" under a capability that vouched for the answer.
    let mutated = with_stdout(&["info", "--json=v2", "--installed"], |s| {
        s.replacen("\"service\": null", "\"renamed_service\": null", 1)
    });
    let runner = ReplayRunner::new(mutated);
    let adapter = adapter(&runner);
    let error = adapter.inventory().unwrap_err();
    assert_eq!(error.id, ErrorId::ProviderOutputChanged);
    assert!(error.detail.contains("`service`"), "{}", error.detail);
    assert_eq!(
        adapter.capabilities().unwrap().capabilities[&Capability::ServiceMetadata],
        CapabilityState::Unavailable {
            reason: "error.provider_output_changed".to_string()
        }
    );
}

#[test]
fn a_pinned_name_is_validated_like_every_other_provider_name() {
    // A valid pinned name reaches the metadata.
    let pinned = with_stdout(&["list", "--pinned"], |_| "wget\n".to_string());
    let runner = ReplayRunner::new(pinned);
    assert!(adapter(&runner).inventory().unwrap().metadata["wget"].pinned);

    // One that could not be a package name is a shape change, not a set member.
    for hostile in ["../../etc/passwd", "-rf", "wgét"] {
        let mutated = with_stdout(&["list", "--pinned"], |_| format!("wget\n{hostile}\n"));
        let runner = ReplayRunner::new(mutated);
        let error = adapter(&runner).inventory().unwrap_err();
        assert_eq!(error.id, ErrorId::ProviderOutputChanged, "{hostile}");
        assert!(error.detail.contains("list --pinned"), "{}", error.detail);
    }
}

#[test]
fn only_the_documented_read_commands_are_consulted() {
    let runner = ReplayRunner::new(recording());
    let adapter = adapter(&runner);
    adapter.snapshot().unwrap();

    let expected: BTreeSet<String> = [
        "brew --cache",
        "brew --caskroom",
        "brew --cellar",
        "brew --prefix",
        "brew --version",
        "brew config",
        "brew info --json=v2 --installed",
        "brew list --pinned",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();
    assert_eq!(runner.consulted(), expected);
    assert_eq!(expected.len(), 8);
}

#[test]
fn each_command_runs_once_however_often_the_contract_is_asked() {
    let counter = CountingRunner::new(recording());
    let adapter = adapter(&counter);
    for _ in 0..3 {
        adapter.detect().unwrap();
        adapter.inventory().unwrap();
        adapter.capabilities().unwrap();
        adapter.requested().unwrap();
        adapter.environment_info().unwrap();
    }
    assert_eq!(
        counter.total(),
        8,
        "one invocation per allow-listed command"
    );
}

#[test]
fn the_inventory_serialises_to_the_same_bytes_every_time() {
    let first = {
        let runner = ReplayRunner::new(recording());
        serde_json::to_string(&adapter(&runner).inventory().unwrap()).unwrap()
    };
    let second = {
        let runner = ReplayRunner::new(recording());
        serde_json::to_string(&adapter(&runner).inventory().unwrap()).unwrap()
    };
    assert_eq!(first, second);
    assert!(first.len() > 10_000);
}

#[test]
fn the_reported_environment_is_the_provider_prefix_layout() {
    let runner = ReplayRunner::new(recording());
    let info = adapter(&runner).environment_info().unwrap();
    assert_eq!(info["prefix"], "/usr/local");
    assert_eq!(info["cellar"], "/usr/local/Cellar");
    assert_eq!(info["caskroom"], "/usr/local/Caskroom");
    assert_eq!(info["bin"], "/usr/local/bin");
    assert!(info["cache"].ends_with("Library/Caches/Homebrew"));
}

#[test]
fn the_snapshot_carries_everything_needed_to_replay_the_analysis() {
    let runner = ReplayRunner::new(recording());
    let snapshot = adapter(&runner).snapshot().unwrap();
    assert_eq!(snapshot.detection.presence, ProviderPresence::Present);
    assert_eq!(snapshot.inventory.installed.len(), 201);
    assert!(snapshot
        .capabilities
        .has(Capability::MachineReadableInventory));
    assert_eq!(snapshot.environment["prefix"], "/usr/local");
}

#[test]
fn an_absent_provider_snapshots_as_empty_rather_than_failing() {
    let runner = ReplayRunner::new(synthetic(&[(
        &["--version"],
        "",
        ExitStatus::NotStarted {
            error: "No such file or directory (os error 2)".into(),
        },
    )]));
    let snapshot = adapter(&runner).snapshot().unwrap();
    assert_eq!(snapshot.detection.presence, ProviderPresence::Absent);
    assert!(snapshot.inventory.installed.is_empty());
    assert!(snapshot.environment.is_empty());
}

// ---------------------------------------------------------------------------
// Ownership of a path
// ---------------------------------------------------------------------------

#[test]
fn ownership_comes_from_the_resolved_path_not_the_containing_prefix() {
    let root = scratch("resolution");
    let cellar = root.join("Cellar");
    let caskroom = root.join("Caskroom");
    let keg = cellar.join("wget/1.25.0/bin");
    std::fs::create_dir_all(&keg).unwrap();
    std::fs::create_dir_all(caskroom.join("gimp/3.2.4")).unwrap();
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::fs::write(keg.join("wget"), "").unwrap();
    std::fs::write(caskroom.join("gimp/3.2.4/app"), "").unwrap();
    std::fs::write(root.join("bin/pnpm"), "").unwrap();

    let runner = ReplayRunner::new(synthetic(&[
        (&["--cellar"], &cellar.display().to_string(), ok()),
        (&["--caskroom"], &caskroom.display().to_string(), ok()),
    ]));
    let adapter = HomebrewAdapter::new(
        &runner,
        root.clone(),
        None,
        platform_tags(),
        FilesystemScope::ThisMachine,
    );

    let owned = adapter.resolution(&keg.join("wget")).unwrap();
    assert_eq!(owned.executable, "wget");
    assert!(owned.authoritative, "a keg path names the package outright");
    match owned.owner {
        ExecutableOwner::Provider { provider, package } => {
            assert_eq!(provider.as_str(), "homebrew");
            assert_eq!(package.as_deref(), Some("wget"));
        }
        other => panic!("expected provider ownership, got {other:?}"),
    }

    let cask = adapter
        .resolution(&caskroom.join("gimp/3.2.4/app"))
        .unwrap();
    assert!(matches!(
        cask.owner,
        ExecutableOwner::Provider { ref package, .. } if package.as_deref() == Some("gimp")
    ));

    // Under the prefix but owned by nothing the provider placed: R-6 finding
    // 14 measured this heuristic failing, so the answer is not authoritative.
    let foreign = adapter.resolution(&root.join("bin/pnpm")).unwrap();
    assert_eq!(foreign.owner, ExecutableOwner::External);
    assert!(!foreign.authoritative);

    // Outside the prefix entirely, and a path that does not exist.
    assert_eq!(
        adapter
            .resolution(Path::new("/usr/bin/curl"))
            .unwrap()
            .owner,
        ExecutableOwner::Unknown
    );
    let missing = adapter.resolution(&root.join("bin/absent")).unwrap();
    assert_eq!(missing.owner, ExecutableOwner::Unknown);
    assert!(!missing.authoritative);
    assert!(missing.canonical_path.is_none());

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_prefix_is_detected_only_where_an_executable_exists() {
    let root = scratch("prefix");
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::fs::write(root.join("bin/brew"), "").unwrap();
    let candidates = vec![PathBuf::from("/nonexistent-prefix"), root.clone()];
    assert_eq!(
        detect_prefix(&candidates, FilesystemScope::ThisMachine),
        Some(root.clone())
    );
    assert_eq!(
        detect_prefix(
            &[PathBuf::from("/nonexistent-prefix")],
            FilesystemScope::ThisMachine
        ),
        None
    );
    std::fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ok() -> ExitStatus {
    ExitStatus::Exited { code: 0 }
}

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "macportsbrew-homebrew-{label}-{}",
        std::process::id()
    ));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A recording assembled for one narrow case, with the same provenance
/// discipline a captured fixture has: it says it was synthesised.
fn synthetic(entries: &[(&[&str], &str, ExitStatus)]) -> Recording {
    Recording {
        schema_version: 1,
        provenance: RecordingProvenance {
            macos: "26.6.2".into(),
            architecture: "x86_64".into(),
            provider_versions: vec![],
            captured_at: "2026-09-02T00:00:00Z".into(),
            captured_by: "crates/adapters/tests/homebrew_contract.rs".into(),
            origin: "synthesised".into(),
            note: Some("built in-test for one case; not a machine capture".into()),
        },
        invocations: entries
            .iter()
            .map(|(argv, stdout, status)| RecordedInvocation {
                invocation: Invocation {
                    program: "brew".into(),
                    argv: argv.iter().map(ToString::to_string).collect(),
                    environment: BTreeMap::new(),
                    status: status.clone(),
                    stdout: (*stdout).to_string(),
                    stderr: String::new(),
                    truncated: false,
                    duration_ms: 0,
                },
            })
            .collect(),
    }
}

/// Counts invocations so the caching contract can be asserted: "the result
/// must not change between calls within one analysis" is only cheap if each
/// command runs once.
struct CountingRunner {
    inner: ReplayRunner,
    count: std::cell::Cell<usize>,
}

impl CountingRunner {
    fn new(recording: Recording) -> Self {
        Self {
            inner: ReplayRunner::new(recording),
            count: std::cell::Cell::new(0),
        }
    }

    fn total(&self) -> usize {
        self.count.get()
    }
}

impl ProcessRunner for CountingRunner {
    fn run(
        &self,
        spec: &macportsbrew_adapters::process::CommandSpec,
    ) -> Result<Invocation, macportsbrew_domain::DomainError> {
        self.count.set(self.count.get() + 1);
        self.inner.run(spec)
    }

    fn is_replay(&self) -> bool {
        true
    }
}

// -- the cask receipt, where the reader must refuse ------------------------
//
// These two live here rather than beside the other receipt tests in
// `homebrew/mod.rs` because they name `symlink` and `hard_link`, and the stop
// gate refuses those tokens anywhere under `src/` — a Rust module inside `src/`
// is compiled into the shipped binary whatever the directory is called. An
// integration test is a separate binary that no release carries, which is the
// placement the guard's own message points at. Not a workaround: the guard is
// right about where this code belongs.

/// A scratch Caskroom holding one cask, with whatever receipt bytes the case
/// wants — or none.
fn caskroom_with(token: &str, receipt: Option<&[u8]>) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "mpb-caskroom-contract-{}-{}",
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

fn receipt_path(root: &std::path::Path, token: &str) -> PathBuf {
    root.join(token)
        .join(".metadata")
        .join("INSTALL_RECEIPT.json")
}

#[test]
fn a_receipt_with_a_second_name_is_refused() {
    // A hard link means these bytes are reachable under a name this tool did
    // not check. One boolean is an oracle rather than a disclosure, but the
    // cheap refusal is the right default in a directory the tool does not own.
    let root = caskroom_with("meld", Some(br#"{"installed_on_request": true}"#));
    std::fs::hard_link(receipt_path(&root, "meld"), root.join("second-name"))
        .expect("a second name");
    assert_eq!(
        homebrew::cask_requested(&root, "meld", FilesystemScope::ThisMachine),
        None,
        "a receipt with more than one name was read"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_receipt_reached_through_a_link_is_refused() {
    // The pre-check refuses a non-regular file before the open. Its blocking
    // rationale moved to the open flags (F-31) - this caller passes
    // `O_NONBLOCK` now - but the check is still what refuses the link here,
    // and still supplies the dev/ino baseline the identity test compares.
    let real = caskroom_with("real", Some(br#"{"installed_on_request": true}"#));
    let root = caskroom_with("meld", None);
    std::os::unix::fs::symlink(receipt_path(&real, "real"), receipt_path(&root, "meld"))
        .expect("a link");
    assert_eq!(
        homebrew::cask_requested(&root, "meld", FilesystemScope::ThisMachine),
        None,
        "a linked receipt was followed"
    );
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&real).ok();
}
