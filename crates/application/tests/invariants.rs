// SPDX-License-Identifier: Apache-2.0

//! Invariant tests for the application layer, one per invariant this layer
//! upholds (`docs/spec/01-invariants.md`). Each builds a synthetic snapshot
//! with two providers and a handful of packages, and asserts the property
//! on the analysis or the plan, never on a live machine.

use macportsbrew_application::{analyze, build_plan, AnalysisOptions, AnalysisSource, PlanRequest};
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence,
    CommandLineTools, DependencyEdge, EnvironmentSnapshot, ExecutableOwner, InstalledPackage,
    Machine, MacosVersion, Outcome, PackageIdentity, PackageKind, PackageMetadata, Provenance,
    ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory, ProviderPresence,
    ProviderSnapshot, RequestedSet, Resolution, Snapshot, StepKind, Translation, Version,
    SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const HB: &str = "homebrew";
const MP: &str = "macports";

fn machine() -> Machine {
    Machine {
        architecture: Architecture::Arm64,
        macos: MacosVersion::new(15, 6, 1),
        translation: Translation::Native,
        command_line_tools: CommandLineTools::Installed {
            path: PathBuf::from("/Library/Developer/CommandLineTools"),
        },
    }
}

fn prebuilt() -> AvailabilityAssessment {
    AvailabilityAssessment {
        availability: ArtifactAvailability::BinaryAvailable,
        evidence: vec![AvailabilityEvidence {
            source: "test".into(),
            detail: "prebuilt".into(),
            offline: true,
        }],
        cached: false,
    }
}

struct Pkg {
    name: &'static str,
    requested: bool,
    homepage: Option<&'static str>,
    service: bool,
}

fn provider(
    id: &str,
    prefix: &str,
    present: bool,
    packages: &[Pkg],
    deps: &[(&str, &str)],
) -> ProviderSnapshot {
    let pid = ProviderId::new(id);
    let mut inventory = ProviderInventory::default();
    if present {
        for p in packages {
            inventory.installed.push(InstalledPackage {
                identity: PackageIdentity::new(pid.clone(), p.name),
                kind: PackageKind::Package,
                version: Version::new("1.0"),
                revision: None,
                variants: BTreeSet::new(),
                active: true,
                requested: Some(p.requested),
                installed_from_prebuilt: Some(true),
                install_path: None,
            });
            if p.requested {
                inventory.requested.names.insert(p.name.to_string());
            }
            inventory.metadata.insert(
                p.name.to_string(),
                PackageMetadata {
                    homepage: p.homepage.map(str::to_string),
                    defines_service: p.service,
                    ..PackageMetadata::default()
                },
            );
            inventory
                .availability
                .insert(p.name.to_string(), prebuilt());
        }
        inventory.requested = RequestedSet {
            names: inventory.requested.names.clone(),
            mechanism: "test".into(),
        };
        for (from, to) in deps {
            inventory.dependencies.push(DependencyEdge {
                from: (*from).to_string(),
                to: (*to).to_string(),
                build_only: false,
            });
        }
    }
    ProviderSnapshot {
        detection: ProviderDetection {
            provider: pid,
            presence: if present {
                ProviderPresence::Present
            } else {
                ProviderPresence::Absent
            },
            version: present.then(|| Version::new("9.9")),
            prefix: present.then(|| PathBuf::from(prefix)),
            executable: None,
            architecture: present.then_some(Architecture::Arm64),
        },
        capabilities: ProviderCapabilities::default(),
        inventory,
        environment: BTreeMap::new(),
        invocation_environment: [("PATH".to_string(), format!("{prefix}/bin:/usr/bin:/bin"))]
            .into_iter()
            .collect(),
        resolutions: BTreeMap::new(),
    }
}

fn snapshot(hb: ProviderSnapshot, mp: ProviderSnapshot) -> Snapshot {
    let mut providers = BTreeMap::new();
    providers.insert(ProviderId::new(HB), hb);
    providers.insert(ProviderId::new(MP), mp);
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-02T00:00:00Z".into(),
            captured_by: "test".into(),
            origin: "synthesised".into(),
            note: None,
        },
        machine: machine(),
        environment: EnvironmentSnapshot::default(),
        providers,
    }
}

fn both_present() -> Snapshot {
    // `tool` maps STRONG (same name + homepage); `dep` is a dependency of
    // `tool` under Homebrew; `postgresql@17` is stateful by curated data.
    // `tool` on PATH resolves to the MacPorts copy, which is the fact that
    // decides which side drives the unit.
    let mut snap = snapshot(
        provider(
            HB,
            "/opt/homebrew",
            true,
            &[
                Pkg {
                    name: "tool",
                    requested: true,
                    homepage: Some("https://example.org/tool"),
                    service: false,
                },
                Pkg {
                    name: "dep",
                    requested: false,
                    homepage: None,
                    service: false,
                },
                Pkg {
                    name: "postgresql@17",
                    requested: true,
                    homepage: None,
                    service: true,
                },
            ],
            &[("tool", "dep")],
        ),
        provider(
            MP,
            "/opt/local",
            true,
            &[
                Pkg {
                    name: "tool",
                    requested: true,
                    homepage: Some("https://example.org/tool"),
                    service: false,
                },
                Pkg {
                    name: "other",
                    requested: true,
                    homepage: None,
                    service: false,
                },
            ],
            &[("other", "tool")],
        ),
    );
    snap.providers
        .get_mut(&ProviderId::new(MP))
        .unwrap()
        .resolutions
        .insert(
            "tool".into(),
            Resolution {
                executable: "tool".into(),
                path: Some(PathBuf::from("/opt/local/bin/tool")),
                canonical_path: None,
                owner: ExecutableOwner::Provider {
                    provider: ProviderId::new(MP),
                    package: Some("tool".into()),
                },
                authoritative: true,
            },
        );
    snap
}

fn options() -> AnalysisOptions {
    AnalysisOptions {
        today: "2026-09-02".into(),
        advanced: false,
        only: BTreeSet::new(),
    }
}

fn policy() -> PolicyData {
    PolicyData::load_embedded().unwrap()
}

fn plan_for(
    snap: &Snapshot,
    package: &str,
    to: &str,
    retain: bool,
) -> macportsbrew_domain::MigrationPlan {
    build_plan(
        snap,
        &policy(),
        &options(),
        &PlanRequest {
            package: package.into(),
            to: ProviderId::new(to),
            retain_source: retain,
        },
    )
    .unwrap()
}

#[test]
fn invariant_01_destination_is_added_and_verified_before_the_source_is_removed() {
    // A package only under Homebrew with a STRONG mapping to an installed
    // MacPorts counterpart is the case where a plan has every step.
    let mut snap = both_present();
    let hb = snap.providers.get_mut(&ProviderId::new(HB)).unwrap();
    hb.inventory
        .installed
        .retain(|r| r.identity.name != "postgresql@17");
    hb.inventory.installed.retain(|r| r.identity.name != "dep");
    hb.inventory.dependencies.clear();
    let plan = plan_for(&snap, "tool", MP, false);
    assert!(plan.check_ordering().is_ok());
    let kinds: Vec<StepKind> = plan.steps.iter().map(|s| s.kind).collect();
    let pos = |k| kinds.iter().position(|x| *x == k).unwrap();
    assert!(pos(StepKind::AddDestination) < pos(StepKind::VerifyDestination));
    assert!(pos(StepKind::VerifyDestination) < pos(StepKind::RemoveSource));
    assert_eq!(kinds[0], StepKind::SnapshotBefore);
}

#[test]
fn invariant_02_plans_are_data_and_nothing_in_this_layer_can_execute_them() {
    let plan = plan_for(&both_present(), "tool", MP, true);
    assert!(plan.dry_run_only);
    let json = serde_json::to_string(&plan).unwrap();
    let back: macportsbrew_domain::MigrationPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(plan, back);
    // No process execution exists in the application crate at all.
    let src = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src")).unwrap();
    for entry in src {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
        assert!(
            !text.contains("ProcessRunner"),
            "application must not run processes"
        );
        assert!(
            !text.contains("std::process"),
            "application must not run processes"
        );
    }
}

#[test]
fn invariant_03_a_broken_provider_degrades_the_analysis_instead_of_failing_it() {
    let mut snap = both_present();
    let mp = snap.providers.get_mut(&ProviderId::new(MP)).unwrap();
    mp.detection.presence = ProviderPresence::Broken {
        reason: "error.provider_unhealthy".into(),
    };
    mp.inventory = ProviderInventory::default();
    let analysis = analyze(&snap, &policy(), &options(), AnalysisSource::Snapshot);
    assert!(matches!(
        analysis.providers[&ProviderId::new(MP)].detection.presence,
        ProviderPresence::Broken { .. }
    ));
    assert!(
        !analysis.packages.is_empty(),
        "the healthy provider is still analysed"
    );
}

#[test]
fn invariant_05_an_absent_provider_is_never_installed_implicitly() {
    let mut snap = both_present();
    snap.providers.insert(
        ProviderId::new(MP),
        provider(MP, "/opt/local", false, &[], &[]),
    );
    let plan = plan_for(&snap, "tool", MP, false);
    assert!(plan.requires_manual_review());
    assert!(!plan.has_mutating_steps());
    assert!(plan
        .risks
        .iter()
        .any(|r| r.id == "destination_absent" && r.blocking));
}

#[test]
fn invariant_07_08_dependencies_never_cross_providers() {
    let snap = both_present();
    let analysis = analyze(&snap, &policy(), &options(), AnalysisSource::Snapshot);
    let unit = analysis.packages.iter().find(|p| p.name == "tool").unwrap();
    // Under MacPorts `other` depends on `tool`; under Homebrew nothing does.
    // Each row counts only its own provider's edges.
    assert_eq!(unit.rows[&ProviderId::new(HB)].reverse_dependency_count, 0);
    assert_eq!(unit.rows[&ProviderId::new(MP)].reverse_dependency_count, 1);
    assert_eq!(unit.rows[&ProviderId::new(HB)].dependency_count, 1);
    assert_eq!(unit.rows[&ProviderId::new(MP)].dependency_count, 0);
    // Facts fed to the engine count the source side only.
    let plan = plan_for(&snap, "tool", MP, false);
    assert!(!plan.risks.iter().any(|r| r.id == "reverse_dependencies"));
}

#[test]
fn invariant_09_duplication_is_reported_and_never_consolidated() {
    let analysis = analyze(
        &both_present(),
        &policy(),
        &options(),
        AnalysisSource::Snapshot,
    );
    let unit = analysis.packages.iter().find(|p| p.name == "tool").unwrap();
    assert_eq!(unit.rows.len(), 2);
    assert!(unit.rows.values().all(|r| !r.installed.is_empty()));
    assert!(!matches!(
        unit.recommendation.outcome,
        Outcome::MigrateTo { .. }
    ));
}

#[test]
fn invariant_10_plan_commands_carry_the_sanitised_invocation_environment() {
    let plan = plan_for(&both_present(), "tool", MP, false);
    let commands: Vec<_> = plan
        .steps
        .iter()
        .flat_map(|s| s.commands.iter())
        .filter(|c| c.program != "macportsbrew")
        .collect();
    assert!(!commands.is_empty());
    for c in commands {
        assert!(c.environment.contains_key("PATH"), "{c:?}");
        assert!(!c
            .environment
            .keys()
            .any(|k| k == "PKG_CONFIG_PATH" || k.starts_with("DYLD_")));
        // The descriptive environment_info keys never leak into a command.
        assert!(!c.environment.contains_key("prefix") && !c.environment.contains_key("bin"));
    }
}

#[test]
fn invariant_13_dependencies_are_hidden_unless_advanced_mode_is_entered() {
    let snap = both_present();
    let normal = analyze(&snap, &policy(), &options(), AnalysisSource::Snapshot);
    assert!(normal.packages.iter().all(|p| !p.advanced_only));
    assert!(!normal.packages.iter().any(|p| p.name == "dep"));
    let advanced = analyze(
        &snap,
        &policy(),
        &AnalysisOptions {
            advanced: true,
            ..options()
        },
        AnalysisSource::Snapshot,
    );
    let dep = advanced.packages.iter().find(|p| p.name == "dep").unwrap();
    assert!(dep.advanced_only);
    assert_eq!(advanced.summary.dependency_units, 1);
}

#[test]
fn invariant_15_analysis_is_a_pure_function_of_the_snapshot() {
    let snap = both_present();
    let a = serde_json::to_string(&analyze(
        &snap,
        &policy(),
        &options(),
        AnalysisSource::Snapshot,
    ))
    .unwrap();
    let b = serde_json::to_string(&analyze(
        &snap,
        &policy(),
        &options(),
        AnalysisSource::Snapshot,
    ))
    .unwrap();
    assert_eq!(a, b);
}

#[test]
fn invariant_17_swapping_the_providers_swaps_the_analysis() {
    let snap = both_present();
    let mut swapped = snap.clone();
    let hb = swapped.providers.remove(&ProviderId::new(HB)).unwrap();
    let mp = swapped.providers.remove(&ProviderId::new(MP)).unwrap();
    let rename = |mut p: ProviderSnapshot, id: &str| {
        let pid = ProviderId::new(id);
        p.detection.provider = pid.clone();
        for row in &mut p.inventory.installed {
            row.identity.provider = pid.clone();
        }
        for r in p.resolutions.values_mut() {
            if let ExecutableOwner::Provider { provider, .. } = &mut r.owner {
                *provider = pid.clone();
            }
        }
        p
    };
    swapped
        .providers
        .insert(ProviderId::new(HB), rename(mp, HB));
    swapped
        .providers
        .insert(ProviderId::new(MP), rename(hb, MP));
    let a = analyze(&snap, &policy(), &options(), AnalysisSource::Snapshot);
    let b = analyze(&swapped, &policy(), &options(), AnalysisSource::Snapshot);
    let unit_a = a.packages.iter().find(|p| p.name == "tool").unwrap();
    let unit_b = b.packages.iter().find(|p| p.name == "tool").unwrap();
    let swap_outcome = |o: &Outcome| match o {
        Outcome::Keep { provider } => Outcome::Keep {
            provider: flip(provider),
        },
        Outcome::MigrateTo { provider } => Outcome::MigrateTo {
            provider: flip(provider),
        },
        Outcome::KeepBoth { primary } => Outcome::KeepBoth {
            primary: flip(primary),
        },
        other => other.clone(),
    };
    assert_eq!(
        swap_outcome(&unit_a.recommendation.outcome),
        unit_b.recommendation.outcome
    );
    assert_eq!(unit_a.rows.len(), unit_b.rows.len());
}

fn flip(id: &ProviderId) -> ProviderId {
    if id.as_str() == HB {
        ProviderId::new(MP)
    } else {
        ProviderId::new(HB)
    }
}

#[test]
fn invariant_19_stateful_and_service_packages_are_never_planned_blind() {
    let plan = plan_for(&both_present(), "postgresql@17", MP, false);
    assert!(plan.requires_manual_review());
    assert!(!plan.has_mutating_steps());
    assert!(plan.risks.iter().any(|r| r.id == "class" && r.blocking));
    let analysis = analyze(
        &both_present(),
        &policy(),
        &options(),
        AnalysisSource::Snapshot,
    );
    let unit = analysis
        .packages
        .iter()
        .find(|p| p.name == "postgresql@17")
        .unwrap();
    assert_eq!(unit.recommendation.outcome, Outcome::ManualReview);
}
