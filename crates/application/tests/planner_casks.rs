// SPDX-License-Identifier: Apache-2.0

//! Whether a cask reaches a command at all (F-22).
//!
//! F-22 found that `brew list --versions <cask>` **exits 1 on a correctly
//! installed cask** — `--cask` is required once a name is given — and that
//! `planner.rs` takes the verb from `verbs[operation]` while taking only the
//! *elevation* key from the package kind. So if a cask ever reached a
//! `VERIFY_DESTINATION` step, that step would report failure for a cask that is
//! installed exactly as intended, and `VERIFY_DESTINATION` is what guards
//! `REMOVE_SOURCE` under INV-01.
//!
//! The finding left a disjunction it could not close: either `GuiApp` forces
//! `MANUAL_REVIEW` and no cask ever reaches a verify step — making the four
//! `*_gui_app` elevation cells dead — or it does reach one and the verbs table
//! must become kind-aware. "It cannot be both, and nothing currently tests
//! which."
//!
//! These tests are that missing test. The answer is the first branch: a cask
//! halts at `MANUAL_REVIEW` with **no commands at all**, so the defective verb
//! is never emitted. That makes F-22 latent rather than live — and latent is
//! not fixed. Anyone making casks plannable must make the verb kind-aware in
//! the same change, because these tests will start failing and this comment is
//! why.

use macportsbrew_application::{build_plan, AnalysisOptions, PlanRequest};
use macportsbrew_domain::{
    Architecture, CommandLineTools, EnvironmentSnapshot, InstalledPackage, Machine, MacosVersion,
    PackageIdentity, PackageKind, PackageMetadata, Provenance, ProviderCapabilities,
    ProviderDetection, ProviderId, ProviderInventory, ProviderPresence, ProviderSnapshot,
    RequestedSet, Snapshot, StepKind, Translation, Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const HB: &str = "homebrew";
const MP: &str = "macports";
/// Shared by both sides, so the mapping engine finds the counterpart on
/// evidence that has nothing to do with the package kind under test.
const HOME: &str = "https://www.gimp.org";

fn holding(id: &str, prefix: &str, name: &str, kind: PackageKind) -> ProviderSnapshot {
    let pid = ProviderId::new(id);
    let mut inventory = ProviderInventory::default();
    inventory.installed.push(InstalledPackage {
        identity: PackageIdentity::new(pid.clone(), name),
        kind,
        version: Version::new("1.0"),
        revision: None,
        variants: BTreeSet::new(),
        active: true,
        requested: Some(true),
        installed_from_prebuilt: Some(true),
        install_path: None,
    });
    inventory.requested = RequestedSet {
        names: [name.to_string()].into_iter().collect(),
        mechanism: "test".into(),
    };
    inventory.metadata.insert(
        name.to_string(),
        PackageMetadata {
            homepage: Some(HOME.to_string()),
            ..PackageMetadata::default()
        },
    );
    ProviderSnapshot {
        detection: ProviderDetection {
            provider: pid,
            presence: ProviderPresence::Present,
            version: Some(Version::new("9.9")),
            prefix: Some(PathBuf::from(prefix)),
            executable: None,
            architecture: Some(Architecture::Arm64),
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

/// Homebrew holds `gimp` as `source_kind`; MacPorts holds a plain package of
/// the same name. Only the source kind differs between the two tests.
fn snapshot(source_kind: PackageKind) -> Snapshot {
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-06T00:00:00Z".into(),
            captured_by: "planner_casks.rs".into(),
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
        providers: vec![
            (
                ProviderId::new(HB),
                holding(HB, "/opt/homebrew", "gimp", source_kind),
            ),
            (
                ProviderId::new(MP),
                holding(MP, "/opt/local", "gimp", PackageKind::Package),
            ),
        ]
        .into_iter()
        .collect(),
    }
}

fn plan(source_kind: PackageKind) -> macportsbrew_domain::MigrationPlan {
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    build_plan(
        &snapshot(source_kind),
        &policy,
        &AnalysisOptions::default(),
        &PlanRequest {
            package: "gimp".to_string(),
            to: ProviderId::new(MP),
            retain_source: false,
        },
    )
    .expect("the request is coherent and must produce a plan")
}

#[test]
fn a_cask_halts_at_manual_review_and_emits_no_command() {
    let plan = plan(PackageKind::GuiApp);

    let kinds: Vec<StepKind> = plan.steps.iter().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        vec![StepKind::ManualReview],
        "a cask must halt at MANUAL_REVIEW and reach no other step (INV-19)"
    );

    let commands: usize = plan.steps.iter().map(|s| s.commands.len()).sum();
    assert_eq!(
        commands, 0,
        "MANUAL_REVIEW halts before the command builder runs, so a cask emits \
         nothing — this is what makes F-22's `brew list --versions <cask>` \
         latent rather than live"
    );
}

#[test]
fn the_same_package_that_is_not_a_cask_does_reach_the_command_builder() {
    // The control. Without it the test above passes for any reason at all —
    // no counterpart found, no policy match, a refusal upstream — and would
    // keep passing if casks became plannable tomorrow.
    let plan = plan(PackageKind::Package);

    let kinds: Vec<StepKind> = plan.steps.iter().map(|s| s.kind).collect();
    assert!(
        kinds.contains(&StepKind::VerifyDestination),
        "a plain package must reach VERIFY_DESTINATION, or the cask test above \
         proves nothing about the package kind: {kinds:?}"
    );

    let commands: usize = plan.steps.iter().map(|s| s.commands.len()).sum();
    assert!(
        commands > 0,
        "a plain package must emit commands: {commands}"
    );
}

#[test]
fn no_step_of_a_cask_plan_can_carry_the_verb_that_would_fail() {
    // Stated as the property rather than the mechanism, so it survives a
    // refactor of *why* casks halt. If any future change lets a cask reach a
    // command, this fails and names the reason it must not ship as-is:
    // `list --versions <cask>` exits 1 on a correctly installed cask, and a
    // VERIFY_DESTINATION that always fails is worse than none under INV-01.
    for step in &plan(PackageKind::GuiApp).steps {
        for command in &step.commands {
            assert!(
                !command.argv.iter().any(|a| a == "--versions"),
                "a cask reached a `--versions` command, which fails on an \
                 installed cask: the verbs table must become kind-aware \
                 (F-22) before this is allowed: {:?}",
                command.argv
            );
        }
    }
}
