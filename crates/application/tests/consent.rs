// SPDX-License-Identifier: Apache-2.0

//! Elevation consent's decisions (`crates/application/src/consent.rs`, pass 08
//! slice 4, D10): which operations ask, from the command table and never the
//! plan's field (obligation 9), and that consent exists only for the exact
//! phrase. Built, not wired: the gate still refuses elevation, which the last
//! test pins.

use macportsbrew_application::{
    build_plan, confirm, confirmation_phrase, elevation_requests, execution_refusals,
    AnalysisOptions, ConsentCode, ConsentRefusal, ExecutionRefusal, PlanRequest,
};
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, Capability,
    CapabilityState, CommandLineTools, Elevation, EnvironmentSnapshot, InstalledPackage, Machine,
    MacosVersion, MigrationPlan, PackageIdentity, PackageKind, PackageMetadata, Provenance,
    ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory, ProviderPresence,
    ProviderSnapshot, RequestedSet, Snapshot, StepKind, Translation, Version,
    SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const HB: &str = "homebrew";
const MP: &str = "macports";

struct Pkg {
    name: &'static str,
    homepage: Option<&'static str>,
    service: bool,
}

fn pkg(name: &'static str) -> Pkg {
    Pkg {
        name,
        homepage: None,
        service: false,
    }
}

fn provider(id: &str, prefix: &str, packages: &[Pkg]) -> ProviderSnapshot {
    let pid = ProviderId::new(id);
    let mut inventory = ProviderInventory::default();
    for p in packages {
        inventory.installed.push(InstalledPackage {
            identity: PackageIdentity::new(pid.clone(), p.name),
            kind: PackageKind::Package,
            version: Version::new("1.0"),
            revision: None,
            variants: BTreeSet::new(),
            active: true,
            requested: Some(true),
            installed_from_prebuilt: Some(true),
            install_path: None,
        });
        inventory.requested.names.insert(p.name.to_string());
        inventory.metadata.insert(
            p.name.to_string(),
            PackageMetadata {
                homepage: p.homepage.map(str::to_string),
                defines_service: p.service,
                service: Some(macportsbrew_domain::ServiceDefinition::both(
                    macportsbrew_domain::Answer::from(p.service),
                )),
                ..PackageMetadata::default()
            },
        );
        inventory.availability.insert(
            p.name.to_string(),
            AvailabilityAssessment {
                availability: ArtifactAvailability::BinaryAvailable,
                evidence: vec![AvailabilityEvidence {
                    source: "test".into(),
                    detail: "prebuilt".into(),
                    offline: true,
                }],
                cached: false,
            },
        );
    }
    inventory.requested = RequestedSet {
        names: inventory.requested.names.clone(),
        mechanism: "test".into(),
    };
    let mut capabilities = ProviderCapabilities::default();
    capabilities.capabilities.insert(
        Capability::MachineReadableInventory,
        CapabilityState::Available,
    );
    ProviderSnapshot {
        detection: ProviderDetection {
            provider: pid,
            presence: ProviderPresence::Present,
            version: Some(Version::new("9.9")),
            prefix: Some(PathBuf::from(prefix)),
            executable: None,
            architecture: Some(Architecture::Arm64),
            prefix_redacted: false,
        },
        capabilities,
        inventory,
        environment: BTreeMap::new(),
        invocation_environment: BTreeMap::new(),
        resolutions: BTreeMap::new(),
        configuration: macportsbrew_domain::ProviderConfiguration::NotRecorded,
    }
}

fn snapshot(hb: &[Pkg], mp: &[Pkg]) -> Snapshot {
    let mut providers = BTreeMap::new();
    providers.insert(ProviderId::new(HB), provider(HB, "/opt/homebrew", hb));
    providers.insert(ProviderId::new(MP), provider(MP, "/opt/local", mp));
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-30T00:00:00Z".into(),
            captured_by: "executor.rs".into(),
            origin: "synthesised".into(),
            note: None,
        },
        machine: Machine {
            architecture: Architecture::Arm64,
            macos: MacosVersion::new(15, 6, 1),
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Installed {
                path: PathBuf::from("/Library/Developer/CommandLineTools"),
            },
        },
        environment: EnvironmentSnapshot::default(),
        providers,
    }
}

/// A package held by `source` only, paired with a different name on the
/// other side by the curated table, so its plan carries a real add in either
/// direction (INV-17).
fn install_needed(source: &str) -> (Snapshot, &'static str, &'static str) {
    if source == HB {
        (snapshot(&[pkg("gnu-sed")], &[pkg("other")]), "gnu-sed", MP)
    } else {
        (snapshot(&[pkg("other")], &[pkg("gsed")]), "gsed", HB)
    }
}

fn plan_for(snap: &Snapshot, package: &str, to: &str, retain: bool) -> MigrationPlan {
    build_plan(
        snap,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        &AnalysisOptions {
            today: "2026-09-30".into(),
            advanced: false,
            only: BTreeSet::new(),
        },
        &PlanRequest {
            package: package.into(),
            to: ProviderId::new(to),
            retain_source: retain,
        },
        &snap.providers.keys().cloned().collect::<Vec<_>>(),
    )
    .expect("the planner builds this plan")
}

fn add_position(plan: &MigrationPlan) -> usize {
    plan.steps
        .iter()
        .position(|s| s.kind == StepKind::AddDestination)
        .expect("an add")
}

#[test]
fn an_elevating_add_asks_and_an_unelevated_one_does_not() {
    // Toward MacPorts its add elevates (the table says so); toward Homebrew it
    // does not. Data, not code: INV-17.
    let (snap, package, to) = install_needed(HB);
    let plan = plan_for(&snap, package, to, true);
    let requests = elevation_requests(&plan, &snap).expect("requests");
    let step = plan.steps[add_position(&plan)].index;
    assert!(
        requests.iter().any(|r| r.step == step && r.known),
        "{requests:?}"
    );
    let (snap, package, to) = install_needed(MP);
    let plan = plan_for(&snap, package, to, true);
    assert!(elevation_requests(&plan, &snap)
        .expect("requests")
        .is_empty());
}

#[test]
fn a_plan_edited_to_say_none_is_still_asked_about() {
    // Obligation 9: the plan's field never decides whether to ask.
    let (snap, package, to) = install_needed(HB);
    let mut plan = plan_for(&snap, package, to, true);
    let add = add_position(&plan);
    for c in &mut plan.steps[add].commands {
        c.elevation = Elevation::None;
    }
    let requests = elevation_requests(&plan, &snap).expect("requests");
    assert!(requests.iter().any(|r| r.step == plan.steps[add].index));
}

#[test]
fn a_request_shows_the_re_derived_command_and_a_forged_one_is_refused() {
    let (snap, package, to) = install_needed(HB);
    let plan = plan_for(&snap, package, to, true);
    let add = add_position(&plan);
    let requests = elevation_requests(&plan, &snap).expect("requests");
    let request = requests
        .iter()
        .find(|r| r.step == plan.steps[add].index)
        .expect("one");
    assert_eq!(request.program, plan.steps[add].commands[0].program);
    assert_eq!(request.argv, plan.steps[add].commands[0].argv);
    for forge in [
        |p: &mut MigrationPlan, add: usize| p.steps[add].commands[0].argv.push("--x".into()),
        |p: &mut MigrationPlan, add: usize| p.steps[add].commands[0].program = "/tmp/evil".into(),
    ] {
        let mut forged = plan.clone();
        forge(&mut forged, add);
        assert_eq!(
            elevation_requests(&forged, &snap),
            Err(ConsentRefusal::PlanDisagrees)
        );
    }
}

#[test]
fn consent_exists_only_for_the_exact_phrase_and_covers_only_its_operation() {
    let (snap, package, to) = install_needed(HB);
    let plan = plan_for(&snap, package, to, true);
    let requests = elevation_requests(&plan, &snap).expect("requests");
    let request = &requests[0];
    let code = ConsentCode::from_bytes([0x0a, 0xbc, 0xde]);
    let phrase = confirmation_phrase(request, &code);
    assert!(phrase.is_ascii() && phrase.contains(&request.step.to_string()));
    assert!(phrase.ends_with(" 0abcde"), "{phrase}");
    for typed in [
        "y",
        "",
        "\n",
        "yes",
        " allow step",
        &format!("allow step {}", request.step),
        &format!(" {phrase}"),
        &format!("{phrase} "),
        &format!("{phrase}\n\n"),
        &phrase.to_uppercase(),
        "allow step 999 0abcde",
    ] {
        assert!(confirm(request, &code, typed).is_none(), "{typed:?}");
    }
    for typed in [
        phrase.clone(),
        format!("{phrase}\n"),
        format!("{phrase}\r\n"),
    ] {
        let given = confirm(request, &code, &typed).expect("the exact phrase");
        assert_eq!(given.step(), request.step);
        assert!(given.covers(request.step, &request.program, &request.argv));
        assert!(!given.covers(request.step + 1, &request.program, &request.argv));
        let mut other = request.argv.clone();
        other.push("--x".into());
        assert!(!given.covers(request.step, &request.program, &other));
    }
}

#[test]
fn confirm_requires_the_code_and_the_step() {
    let (snap, package, to) = install_needed(HB);
    let plan = plan_for(&snap, package, to, true);
    let requests = elevation_requests(&plan, &snap).expect("requests");
    let request = &requests[0];
    let code = ConsentCode::from_bytes([1, 2, 3]);
    let other = ConsentCode::from_bytes([1, 2, 4]);
    let typed = confirmation_phrase(request, &code);
    assert!(confirm(request, &code, &typed).is_some());
    assert!(confirm(request, &other, &typed).is_none(), "another code");
    let mut later = request.clone();
    later.step += 1;
    assert!(confirm(&later, &code, &typed).is_none(), "another step");
}

#[test]
fn a_code_is_six_lowercase_hex_and_nothing_else() {
    for (bytes, written) in [
        ([0u8, 0, 0], "000000"),
        ([0xff, 0xff, 0xff], "ffffff"),
        ([0x0a, 0xb0, 0x0c], "0ab00c"),
    ] {
        let code = ConsentCode::from_bytes(bytes).to_string();
        assert_eq!(code, written);
        assert!(
            code.len() == 6
                && code
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
    }
}

#[test]
fn the_gate_still_refuses_elevation_while_consent_is_not_wired() {
    let (snap, package, to) = install_needed(HB);
    let plan = plan_for(&snap, package, to, true);
    assert!(execution_refusals(&plan)
        .refusals()
        .iter()
        .any(|r| matches!(r, ExecutionRefusal::ElevationRequired { .. })));
}
