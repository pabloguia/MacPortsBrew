// SPDX-License-Identifier: Apache-2.0

//! Elevation consent's decisions (`crates/application/src/consent.rs`, pass 08
//! slice 4, D10): which operations ask, from the command table and never the
//! plan's field (obligation 9), and that consent exists only for the exact
//! phrase; and `consent_for`, the run's decision of when to ask (work order 16
//! part E-3). Wired into the run loop, still unreachable: the gate refuses
//! elevation until part J, which the last test pins.

use macportsbrew_application::{
    build_plan, confirm, confirmation_phrase, consent_for, elevation_requests, execution_refusals,
    plan_run, AnalysisOptions, ConsentCode, ConsentDecision, ConsentGiven, ConsentHalt,
    ConsentRefusal, ElevationRequest, ExecutionRefusal, PlanRequest, PlannedRun,
};
use macportsbrew_domain::ErrorId;
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

fn run_for(snap: &Snapshot, package: &str, to: &str, retain_source: bool) -> PlannedRun {
    plan_run(
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
            retain_source,
        },
        &snap.providers.keys().cloned().collect::<Vec<_>>(),
    )
    .expect("the planner builds this run")
}

/// A planned run whose plan elevates: toward MacPorts its add does (the
/// table says so). Uncleared — no cleared run reaches consent in this build —
/// so the application's decision is testable today (E plan, part B).
fn elevating_run() -> PlannedRun {
    let (snap, package, to) = install_needed(HB);
    run_for(&snap, package, to, true)
}

/// An elevating run in each direction (INV-17): toward MacPorts its add
/// elevates; toward Homebrew, with the source removed, MacPorts' removal does.
/// Data, not code: the command table decides both.
fn elevating_runs() -> [PlannedRun; 2] {
    let (snap, package, to) = install_needed(MP);
    let toward_homebrew = run_for(&snap, package, to, false);
    let remove = toward_homebrew
        .plan()
        .steps
        .iter()
        .find(|s| s.kind == StepKind::RemoveSource)
        .expect("a removal")
        .index;
    assert!(
        elevation_requests(toward_homebrew.plan(), toward_homebrew.before())
            .expect("requests")
            .iter()
            .any(|r| r.step == remove),
        "toward Homebrew, the removal from MacPorts asks"
    );
    [elevating_run(), toward_homebrew]
}

/// Every request confirmed with its exact phrase.
fn confirm_all(requests: &[ElevationRequest]) -> Vec<ConsentGiven> {
    let code = ConsentCode::from_bytes([1, 2, 3]);
    requests
        .iter()
        .map(|r| confirm(r, &code, &confirmation_phrase(r, &code)).expect("the exact phrase"))
        .collect()
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
fn consent_for_asks_only_when_there_is_something_to_ask() {
    // Toward Homebrew with the source kept nothing elevates: the question is
    // never put.
    let (snap, package, to) = install_needed(MP);
    let run = run_for(&snap, package, to, true);
    let mut tokens = consent_for(&run, |_| panic!("asked with nothing to ask")).expect("tokens");
    assert!(!tokens.asked());
    for step in &run.plan().steps {
        assert!(tokens.take(step.index).is_none());
    }
    // When something elevates, in either direction, the person is asked about
    // exactly that.
    for run in elevating_runs() {
        let expected = elevation_requests(run.plan(), run.before()).expect("requests");
        assert!(!expected.is_empty());
        let asked = std::cell::Cell::new(0);
        let tokens = consent_for(&run, |requests| {
            asked.set(asked.get() + 1);
            assert_eq!(requests, expected.as_slice());
            ConsentDecision::Given(confirm_all(requests))
        })
        .expect("given");
        assert_eq!(
            asked.get(),
            1,
            "one question per run, one line per operation"
        );
        assert!(tokens.asked());
    }
}

#[test]
fn consent_for_maps_every_decision_by_name() {
    // A person's refusal is exit 6; a question nobody could see and a missing
    // code are safety stops, exit 4; a refusal the prompt never gives for a
    // non-empty list is a defect, exit 1 (E plan, R2 and R5). Both directions
    // (INV-17).
    for run in elevating_runs() {
        for (why, halt, id, code) in [
            (
                ConsentRefusal::Declined,
                ConsentHalt::Declined,
                ErrorId::UserAborted,
                6,
            ),
            (
                ConsentRefusal::NotATerminal,
                ConsentHalt::Unseen,
                ErrorId::SafetyBlocked,
                4,
            ),
            (
                ConsentRefusal::CodeUnavailable,
                ConsentHalt::CodeUnavailable,
                ErrorId::SafetyBlocked,
                4,
            ),
            (
                ConsentRefusal::PlanDisagrees,
                ConsentHalt::Malformed,
                ErrorId::Internal,
                1,
            ),
            (
                ConsentRefusal::NoRequests,
                ConsentHalt::Malformed,
                ErrorId::Internal,
                1,
            ),
        ] {
            let got = consent_for(&run, |_| ConsentDecision::Refused(why)).expect_err("halted");
            assert_eq!(got, halt, "{why:?}");
            let error = got.error();
            assert_eq!((error.id, error.exit_code()), (id, code), "{why:?}");
        }
    }
}

#[test]
fn consent_for_refuses_a_token_count_that_is_not_one_per_request() {
    for run in elevating_runs() {
        let requests = elevation_requests(run.plan(), run.before()).expect("requests");
        // None, one too many, and the right count for the wrong operation.
        let other = {
            let mut r = requests[0].clone();
            r.step += 100;
            r
        };
        let cases: Vec<Vec<ConsentGiven>> = vec![
            Vec::new(),
            confirm_all(&[requests.as_slice(), &requests[..1]].concat()),
            {
                let mut tokens = confirm_all(&requests);
                tokens[0] = confirm_all(std::slice::from_ref(&other)).remove(0);
                tokens
            },
        ];
        for tokens in cases {
            let count = tokens.len();
            let halt =
                consent_for(&run, move |_| ConsentDecision::Given(tokens)).expect_err("refused");
            assert_eq!(halt, ConsentHalt::Malformed, "{count} tokens");
            assert_eq!(halt.error().exit_code(), 1, "never a person's refusal (R5)");
        }
    }
    let run = elevating_run();
    let requests = elevation_requests(run.plan(), run.before()).expect("requests");
    // One token per request: each step's taken once, never another's.
    let mut tokens = consent_for(&run, |r| ConsentDecision::Given(confirm_all(r))).expect("given");
    let step = requests[0].step;
    assert!(tokens.take(step + 100).is_none());
    let token = tokens.take(step).expect("its token");
    assert!(token.covers(step, &requests[0].program, &requests[0].argv));
    assert!(tokens.take(step).is_none(), "spent once");
}

#[test]
fn invariant_06_a_declined_or_unseen_question_halts_before_the_marker() {
    // The application half (E plan, part B): a refusal is a halt, never an
    // empty grant, so the shell's `?` returns before the marker is written.
    // The shell half is structural (`the_switch_needs_consent_wired`); the
    // end-to-end case needs a cleared elevated run, which part J's gate makes.
    for run in elevating_runs() {
        for why in [ConsentRefusal::Declined, ConsentRefusal::NotATerminal] {
            assert!(consent_for(&run, |_| ConsentDecision::Refused(why)).is_err());
        }
    }
}

#[test]
fn the_gate_still_refuses_elevation_until_part_j() {
    let (snap, package, to) = install_needed(HB);
    let plan = plan_for(&snap, package, to, true);
    assert!(execution_refusals(&plan)
        .refusals()
        .iter()
        .any(|r| matches!(r, ExecutionRefusal::ElevationRequired { .. })));
}
