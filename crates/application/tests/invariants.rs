// SPDX-License-Identifier: Apache-2.0

//! Invariant tests for the application layer, one per invariant this layer
//! upholds (`docs/spec/01-invariants.md`). Each builds a synthetic snapshot
//! with two providers and a handful of packages, and asserts the property
//! on the analysis or the plan, never on a live machine.

use macportsbrew_application::{
    analyze, build_plan, provider_commands, AnalysisOptions, AnalysisSource, PlanRequest,
};
use macportsbrew_domain::{
    is_build_flag, Architecture, ArtifactAvailability, AvailabilityAssessment,
    AvailabilityEvidence, Capability, CapabilityState, CommandLineTools, CommandTarget,
    ConcurrencyFlag, DependencyEdge, Elevation, EnvironmentSnapshot, ExecutableOwner,
    InstalledPackage, Machine, MacosVersion, MigrationPlan, Outcome, PackageIdentity, PackageKind,
    PackageMetadata, Provenance, ProviderCapabilities, ProviderDetection, ProviderId,
    ProviderInventory, ProviderPresence, ProviderSnapshot, RequestedSet, Resolution, Snapshot,
    StepKind, SuccessSemantic, Translation, VerificationCheck, Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
            prefix_redacted: false,
        },
        capabilities: ProviderCapabilities::default(),
        inventory,
        environment: BTreeMap::new(),
        invocation_environment: [("PATH".to_string(), format!("{prefix}/bin:/usr/bin:/bin"))]
            .into_iter()
            .collect(),
        resolutions: BTreeMap::new(),
        configuration: macportsbrew_domain::ProviderConfiguration::NotRecorded,
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
        &snap.providers.keys().cloned().collect::<Vec<_>>(),
    )
    .unwrap()
}

#[test]
fn a_deferred_row_the_other_side_does_not_claim_is_still_reported() {
    // Mappings are scored per direction and are not symmetric. The curated
    // table maps `macports/postgresql17-server` to a single Homebrew candidate,
    // so it selects; `homebrew/postgresql@17` maps to two MacPorts candidates,
    // so it selects neither. When the MacPorts row defers to Homebrew on the
    // strength of the first, the second means Homebrew never pulls it back in.
    //
    // Deferring unconditionally therefore dropped an installed, user-requested
    // port on the floor: reported under no unit, as neither a unit nor a row,
    // in either mode, with `plan` stating it was not installed under any
    // provider. No shipped recording carries a `resolutions` map, so nothing
    // else in the suite enters this branch at all.
    let mut snap = both_present();
    let mp = snap.providers.get_mut(&ProviderId::new(MP)).unwrap();
    let mut row = mp.inventory.installed[0].clone();
    row.identity = macportsbrew_domain::PackageIdentity::new(
        ProviderId::new(MP),
        "postgresql17-server".to_string(),
    );
    row.requested = Some(true);
    mp.inventory.installed.push(row);
    // The resolution is what makes the other side drive the unit (SE-042).
    mp.resolutions.insert(
        "postgresql17-server".into(),
        Resolution {
            executable: "postgresql17-server".into(),
            path: Some(PathBuf::from("/opt/homebrew/bin/postgres")),
            canonical_path: None,
            owner: ExecutableOwner::Provider {
                provider: ProviderId::new(HB),
                package: Some("postgresql@17".to_string()),
            },
            authoritative: true,
        },
    );

    for advanced in [false, true] {
        let analysis = analyze(
            &snap,
            &policy(),
            &AnalysisOptions {
                advanced,
                ..options()
            },
            AnalysisSource::Snapshot,
        );
        let found = analysis.packages.iter().any(|u| {
            u.name == "postgresql17-server"
                || u.rows.values().any(|r| r.name == "postgresql17-server")
        });
        assert!(
            found,
            "an installed, requested port must be reported somewhere \
             (advanced = {advanced})"
        );
    }
}

#[test]
fn invariant_17_a_dual_held_package_plans_in_either_direction() {
    // The asymmetry this test exists for: every one of the suite's plan
    // invocations pointed at the same provider, so nothing exercised the
    // direction the charter calls first-class. For a package both providers
    // hold, `--to` used to be compared against the unit's *inferred* source -
    // which falls back to id order - so one direction produced a plan and the
    // mirror produced a usage error. `--to` names the destination now, and the
    // other holder is the source (SE-047).
    // `both_present` is deliberately asymmetric: under MacPorts, `other`
    // depends on `tool`, and under Homebrew nothing does. So the two directions
    // *should* differ - one removal breaks a dependent and the other does not -
    // and both must still be accepted rather than refused as a usage error,
    // which is the defect. Assert that first, on the fixture as it stands.
    let asymmetric = both_present();
    for dest in [MP, HB] {
        let plan = plan_for(&asymmetric, "tool", dest, false);
        assert!(
            plan.check_ordering().is_ok(),
            "a plan toward {dest} is produced rather than refused"
        );
    }
    // Then remove the one asymmetric fact, and the two directions must be the
    // same plan with the providers exchanged.
    let mut snap = both_present();
    snap.providers
        .get_mut(&ProviderId::new(MP))
        .unwrap()
        .inventory
        .dependencies
        .clear();
    let to_mp = plan_for(&snap, "tool", MP, false);
    let to_hb = plan_for(&snap, "tool", HB, false);

    for (plan, dest) in [(&to_mp, MP), (&to_hb, HB)] {
        assert!(
            plan.check_ordering().is_ok(),
            "ordering holds toward {dest}"
        );
        let kinds: Vec<StepKind> = plan.steps.iter().map(|s| s.kind).collect();
        assert!(
            kinds.contains(&StepKind::RemoveSource),
            "a migration toward {dest} removes from the other side: {kinds:?}"
        );
        let verify = kinds
            .iter()
            .position(|k| *k == StepKind::VerifyDestination)
            .expect("the destination is verified");
        let remove = kinds
            .iter()
            .position(|k| *k == StepKind::RemoveSource)
            .expect("the source is removed");
        assert!(verify < remove, "INV-01 holds toward {dest}");
    }
    // Mirrored, not merely both non-empty: the same step kinds in the same
    // order, with the providers exchanged.
    let kinds = |p: &macportsbrew_domain::MigrationPlan| -> Vec<StepKind> {
        p.steps.iter().map(|s| s.kind).collect()
    };
    assert_eq!(
        kinds(&to_mp),
        kinds(&to_hb),
        "the two directions must be the same plan with the providers swapped"
    );
    // And what each step *asserts*, not only which steps there are. The
    // verification a step carries became conditional (F-06), and the mirror
    // compared kinds alone - so an asymmetry introduced into that condition
    // would have been caught by nothing here.
    let checks = |p: &macportsbrew_domain::MigrationPlan| -> Vec<Vec<VerificationCheck>> {
        p.steps.iter().map(|s| s.verification.clone()).collect()
    };
    assert_eq!(
        checks(&to_mp),
        checks(&to_hb),
        "each step asserts the same thing in both directions"
    );
}

#[test]
fn a_destination_that_already_holds_the_package_is_told_so_rather_than_reinstalled() {
    // `both_present` has the counterpart installed under the destination, so
    // there is nothing to add. The step stays - a plan is a document a human
    // reads, and the absence of work is worth stating - but it carries no
    // command, and it used to carry an install that would do nothing plus a
    // warning about whether the destination could supply a prebuilt artifact
    // for something already built and installed there.
    let plan = plan_for(&both_present(), "tool", MP, false);
    let add = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::AddDestination)
        .expect("the step is present even when there is nothing to add");
    assert!(
        add.commands.is_empty(),
        "nothing is installed over an installed package: {:?}",
        add.commands
    );
    assert_eq!(add.reason.id.as_str(), "reason.already_under_destination");
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
fn invariant_01_a_verification_step_asserts_what_its_name_says() {
    // `VERIFY_REMOVAL` used to carry `ProviderReportsActive`, documented as
    // "the destination provider reports the package installed and active". As
    // data, the plan said: verify the removal by confirming the package is
    // still there. Nothing executes yet so nothing acted on it — an executor
    // would have had to either silently invert it or find nothing to run — but
    // the plan is the contract (INV-02) and INV-01 rests on these steps meaning
    // what they say (F-06).
    let mut snap = both_present();
    let hb = snap.providers.get_mut(&ProviderId::new(HB)).unwrap();
    hb.inventory
        .installed
        .retain(|p| p.identity.name.as_str() != "postgresql@17");
    let plan = plan_for(&snap, "tool", MP, false);
    let removal = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::VerifyRemoval)
        .expect("a plan that removes verifies the removal");
    assert_eq!(
        removal.verification,
        vec![VerificationCheck::PackageAbsent],
        "verifying a removal is asserting absence, not presence"
    );
    assert!(
        !removal
            .verification
            .contains(&VerificationCheck::ProviderReportsActive),
        "the check that asserts the opposite must not be here"
    );

    // And a check an executor cannot perform is a check in name only, which is
    // what this step was until 2026-09-19: `PACKAGE_ABSENT` with no command at
    // all, the last of `plan.v1.md`'s executor-debt items to have no data
    // behind it.
    let command = match removal.commands.as_slice() {
        [one] => one,
        other => panic!("verifying a removal needs exactly one command, got {other:?}"),
    };
    assert_eq!(
        command.success,
        SuccessSemantic::StdoutOmitsPackage,
        "absence is the name missing from the answer, never an exit status"
    );

    // **The assertion this test exists for.** The argv must not name the
    // package, and that is a safety property rather than a formatting one.
    // Observed 2026-09-19 on both providers: asked about a single name, each
    // returns the *same* empty stdout for a package that was removed, a name
    // that never was one, and a provider that failed to run. An executor
    // handed `list --versions tool` and told "absent if the name is missing"
    // would read a crashed provider as a successful removal — on the step that
    // certifies the removal. Asking for the whole inventory makes the answer a
    // large positive output instead, so an empty one is refused rather than
    // believed.
    assert!(
        !command.argv.iter().any(|a| a == "tool"),
        "the absence command must ask for the inventory, not for one name: {:?}",
        command.argv
    );

    // Four more, each pinning a mutation the first draft of this test let
    // through. `spec-guardian` found them on 2026-09-19 by mutating the code
    // rather than by reading it, and the first three are safety properties, not
    // tidiness. The assertion they replace was `!command.argv.is_empty()`, which
    // no reachable path can violate — `argv` is either the verb from the table
    // or the literal `["<verify_absent>"]` fallback — so it was the F-19 class
    // in the test added to close an F-19-class hole.
    let removal_command = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::RemoveSource)
        .and_then(|s| s.commands.first())
        .expect("a plan that verifies a removal performed one");

    // 1. **It reads the provider the package was removed from.** Swapping
    //    `src_snapshot`/`source` for the destination's passes every assertion
    //    above: the destination's argv is also non-empty and also omits "tool".
    //    The plan would then certify a removal from one provider by reading the
    //    *other* provider's inventory, where the name is guaranteed absent —
    //    a cross-provider satisfaction of the kind INV-07 and INV-08 forbid,
    //    on the step that certifies the removal.
    assert_eq!(
        command.program, removal_command.program,
        "the absence check must read the provider the removal acted on"
    );
    assert_eq!(
        removal.provider,
        plan.steps
            .iter()
            .find(|s| s.kind == StepKind::RemoveSource)
            .expect("a removal step")
            .provider,
        "and the step must name that provider too"
    );

    // 2. **It asks for the WHOLE inventory** — the same verb as the presence
    //    check, one argument shorter. Nothing pinned this, so a data-only edit
    //    to `["-q", "installed", "requested"]` or `["list", "--versions",
    //    "--formula"]` would still pass: a non-empty answer that structurally
    //    omits dependencies or casks, and so a certified removal that never
    //    happened. That would reopen by data edit the exact hole the nine-cask
    //    observation was added to close.
    //    Anchored to the table rather than to another step of this plan. The
    //    first attempt compared against `VERIFY_DESTINATION`'s command and
    //    guarded the comparison with `|| command.program != presence.program` —
    //    and in this plan the two sides *are* different providers, so the guard
    //    short-circuited and the assertion never ran. It passed the mutation it
    //    was written for. Caught by running that mutation rather than by
    //    re-reading the assertion, which is the only way this class is ever
    //    caught.
    let table = provider_commands::load().expect("the shipped commands table");
    for (id, entry) in &table {
        let presence = entry.verbs.get("verify").expect("every provider verifies");
        let absence = entry
            .verbs
            .get("verify_absent")
            .expect("every provider certifies a removal");
        assert_eq!(
            absence, presence,
            "{id}: the absence verb must be the presence verb — the planner appends no \
             package to it, so equal argv here means 'the whole inventory, one argument \
             shorter' at the command. A narrowed form like `--formula` or `requested` \
             answers non-empty while structurally omitting what it does not list, which \
             certifies a removal that never happened"
        );
    }

    // 3. **It is a command an executor could run**, not an illustration.
    assert!(
        !command.placeholder,
        "a placeholder cannot certify anything"
    );

    // 4. **Reading an inventory never elevates.** `verify_absent = "required"`
    //    in the elevation table would put an administrator prompt on a read,
    //    which is the INV-06 shape F-10 was filed about, and nothing here
    //    noticed.
    assert_eq!(
        command.elevation,
        Elevation::None,
        "a step whose job is to read must not elevate (INV-06)"
    );
}

#[test]
fn invariant_01_a_verification_command_states_how_it_could_fail() {
    // INV-01 is "nothing is removed before its replacement is verified", and
    // `check_ordering` enforces the *order*. Order is worth nothing if the step
    // it puts first cannot fail: observed 2026-09-08, `port -q installed <name>`
    // exits 0 for an installed port, an absent one, and a name that is not a
    // port at all, so a `VERIFY_DESTINATION` toward MacPorts driven by exit
    // status passes for a package that was never installed — and then
    // `REMOVE_SOURCE` runs (F-33).
    //
    // Asserted per direction, because the two providers genuinely differ and a
    // single blanket rule is the mistake this closes. Toward Homebrew exit
    // status is the right answer; asserting it here is what keeps the MacPorts
    // cell a fact rather than a workaround.
    // The same fixture `invariant_17_a_dual_held_package_plans_in_either_
    // direction` uses for its mirrored half: `both_present` is deliberately
    // asymmetric, and clearing the one dependency is what makes both
    // directions produce a full plan rather than a halt.
    let mut snap = both_present();
    snap.providers
        .get_mut(&ProviderId::new(MP))
        .unwrap()
        .inventory
        .dependencies
        .clear();

    for (to, expected) in [
        (MP, SuccessSemantic::StdoutNamesPackage),
        (HB, SuccessSemantic::ExitStatus),
    ] {
        let plan = plan_for(&snap, "tool", to, false);
        let verify = plan
            .steps
            .iter()
            .find(|s| s.kind == StepKind::VerifyDestination)
            .expect("a migration verifies its destination");
        let command = verify
            .commands
            .first()
            .expect("and that verification runs a command");
        assert_eq!(
            command.success, expected,
            "verifying toward {to} must state how the command reports failure"
        );
        assert!(
            plan.steps_with_unknown_success().is_empty(),
            "no step of a plan this build produced may leave it unstated: {:?}",
            plan.steps_with_unknown_success()
        );
    }
}

#[test]
fn invariant_01_a_destination_already_holding_the_package_claims_no_proof_it_cannot_give() {
    // Every check in the contract is satisfiable by state that predates the
    // plan. Where the destination already has the package, all of them are true
    // before a single step runs — so a failed or no-op install cleared the gate
    // INV-01 depends on, and nothing in the plan distinguished the two (F-06).
    //
    // Where the destination was absent, `PackageNewlyPresent` is a check a
    // no-op install cannot satisfy. Where it was present, the plan asserts only
    // what is true. A weaker guard stated beats a stronger one implied.
    let checks = |snap: &Snapshot| {
        plan_for(snap, "tool", MP, true)
            .steps
            .iter()
            .find(|s| s.kind == StepKind::VerifyDestination)
            .map(|s| s.verification.clone())
    };

    let present = both_present();
    let present = checks(&present).expect("a plan verifies its destination");
    assert!(
        !present.contains(&VerificationCheck::PackageNewlyPresent),
        "the destination already holds it; nothing here proves an install happened"
    );

    // `install_needed(HB)` is the destination-absent case with a mapping strong
    // enough to plan on: `gnu-sed` under Homebrew, `gsed` under MacPorts, paired
    // by the curated table rather than by both sides being installed. Removing
    // the destination copy from `both_present()` does not work — the mapping
    // evidence there *is* the MacPorts side, so the plan halts at manual review
    // and there is no verification step to inspect.
    let absent = install_needed(HB);
    // The whole contract, not only the conditional member. Four of these six
    // appear in no other test in the workspace, so deleting one from the
    // planner would have been caught by nothing - which is F-06's own thesis
    // applied to F-06's own fix. Making the set conditional is what raised the
    // cost of that gap enough to close it here.
    let contract = |extra: Option<VerificationCheck>| -> Vec<VerificationCheck> {
        let mut v = vec![VerificationCheck::ProviderReportsActive];
        v.extend(extra);
        v.extend([
            VerificationCheck::ProviderIntegrityCheck,
            // The *destination's* name. `gnu-sed` under Homebrew maps to
            // `gsed` under MacPorts, and what gets verified is what will be
            // installed - which this pinned by getting it wrong first.
            VerificationCheck::ExecutablesOwnedByDestination {
                executables: vec!["gsed".into()],
            },
            VerificationCheck::LivenessProbe {
                executable: "gsed".into(),
                flag: "--version".into(),
            },
            VerificationCheck::NoCrossProviderLinkage,
        ]);
        v
    };
    let checks = plan_for(&absent, "gnu-sed", MP, true)
        .steps
        .iter()
        .find(|s| s.kind == StepKind::VerifyDestination)
        .map_or_else(
            || panic!("this fixture is the one that plans a real install"),
            |s| s.verification.clone(),
        );
    assert_eq!(
        checks,
        contract(Some(VerificationCheck::PackageNewlyPresent)),
        "the destination was absent, so presence afterwards is the proof - and \
         the rest of the contract is unchanged, not replaced"
    );
}

#[test]
fn invariant_02_plans_are_data_and_nothing_in_this_layer_can_execute_them() {
    let plan = plan_for(&both_present(), "tool", MP, true);
    assert!(plan.dry_run_only);
    let json = serde_json::to_string(&plan).unwrap();
    let back: macportsbrew_domain::MigrationPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(plan, back);
    // No process execution exists in the application crate at all. The walk is
    // recursive: it used to read `src/` one level deep and call
    // `read_to_string` on every entry, so the first subdirectory would have
    // panicked the test rather than been scanned (F12).
    let mut stack = vec![PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
    let mut scanned = 0;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            scanned += 1;
            assert!(
                !text.contains("ProcessRunner"),
                "application must not run processes: {}",
                path.display()
            );
            for token in ["std::process", "process::Command", "Command::new"] {
                assert!(
                    !text.contains(token),
                    "application must not run processes: {} contains {token}",
                    path.display()
                );
            }
        }
    }
    assert!(scanned > 0, "the scan read the application sources");
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
fn the_analysis_carries_the_concurrency_flag_its_source_decides() {
    // The flag is the source's answer, never a constant: the same snapshot
    // analysed as a live capture and as a loaded file says two different
    // things, and both say possibly inconsistent (SE-015).
    let snap = both_present();
    for source in [AnalysisSource::Live, AnalysisSource::Snapshot] {
        let analysis = analyze(&snap, &policy(), &options(), source);
        assert_eq!(
            analysis.concurrent_provider_activity,
            ConcurrencyFlag::of(source.concurrent_provider_activity()),
            "{source:?}"
        );
        assert!(analysis
            .concurrent_provider_activity
            .possibly_inconsistent());
    }
    let live = analyze(&snap, &policy(), &options(), AnalysisSource::Live);
    let loaded = analyze(&snap, &policy(), &options(), AnalysisSource::Snapshot);
    assert_ne!(
        live.concurrent_provider_activity,
        loaded.concurrent_provider_activity
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
fn invariant_11_setting_the_primary_transfers_no_ownership() {
    // "Choosing which `git` runs does not change which provider owns `git`'s
    // dependencies." The policy crate's test argues this from the types -
    // `Outcome::KeepBoth` carries a `primary` and nothing else - which is true
    // and is not where selection is implemented (F-19).
    //
    // Nothing persists a primary in this build, so there is no before-and-after
    // to compare; asserting one against an unchanged snapshot is a tautology,
    // and the first version of this test did exactly that. What *can* fail is
    // the plan itself: the mechanism by which a primary is selected is the
    // reason ownership cannot move, and a change to that mechanism is the way
    // this invariant would actually break.
    let snap = both_present();
    let plan = plan_for(&snap, "tool", MP, true);
    assert_eq!(plan.primary_after, ProviderId::new(MP));
    assert!(
        !plan.steps.iter().any(|s| s.kind == StepKind::RemoveSource),
        "`--retain-source` keeps the source copy, so ownership cannot move by removal"
    );

    let primary = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::SetPrimary)
        .expect("this plan is the one that selects a primary");

    // The load-bearing assertion. Selection goes through MacPortsBrew's own
    // managed directory (SE-036), never through a provider's mechanism - so
    // neither provider's record of what it owns is touched. A `SetPrimary` that
    // ran `brew link` or `port select` instead would transfer exactly what
    // INV-11 forbids, and this is what would catch it.
    assert!(!primary.commands.is_empty(), "the step names its command");
    for c in &primary.commands {
        assert_eq!(
            c.target,
            macportsbrew_domain::CommandTarget::Tool,
            "selection is this tool's own operation, not a provider's: {c:?}"
        );
        assert_eq!(
            c.elevation,
            macportsbrew_domain::Elevation::None,
            "and it stays off the elevating path (SE-036): {c:?}"
        );
    }

    // No step touches anything but the package named. `dep` is a dependency of
    // `tool` under Homebrew, and a plan that reached for it would be moving
    // ownership by the back door. It is visible only under `advanced`, which is
    // why the first version of this assertion - written against the default
    // options - never ran at all.
    let advanced = AnalysisOptions {
        advanced: true,
        ..options()
    };
    let seen = analyze(&snap, &policy(), &advanced, AnalysisSource::Snapshot);
    let dep = seen
        .packages
        .iter()
        .find(|p| p.name == "dep")
        .expect("advanced mode shows the dependency; the default view does not");
    assert_eq!(
        dep.source,
        ProviderId::new(HB),
        "the dependency belongs to Homebrew before any of this"
    );
    for step in &plan.steps {
        for c in &step.commands {
            assert!(
                !c.argv.iter().any(|a| a == "dep"),
                "a plan for `tool` named `dep`: {step:?}"
            );
        }
    }

    // And the serialised plan says nothing about ownership - the same assertion
    // the policy crate makes about a recommendation, at the layer that emits
    // the step.
    let json = serde_json::to_string(&plan).unwrap();
    assert!(
        !json.contains("ownership") && !json.contains("requested"),
        "a plan that selects a primary must not restate ownership: {json}"
    );
}

#[test]
fn invariant_17_which_answer_wins_never_depends_on_the_providers_names() {
    // `resolution_for` asks every provider who owns an executable, because that
    // is a machine-wide question. Each answers for itself, so two can disagree
    // — and returning the first hit in map order made provider *id order*
    // decide (F-20).
    //
    // The first fix promoted `Provider` and left every other pair falling
    // through to that order. A review ran the mirror and it was not one:
    // MacPorts answers `System` for a `/usr/bin` path where Homebrew answers
    // `Unknown`, so those two swapped answers when the providers swapped roles.
    // This test therefore asserts *both orientations of every pair*, which is
    // what the single-orientation version could not do.
    let resolution = |owner: ExecutableOwner, authoritative: bool| Resolution {
        executable: "tool".into(),
        path: Some(PathBuf::from("/usr/local/bin/tool")),
        canonical_path: None,
        owner,
        authoritative,
    };
    let owned = || {
        (
            ExecutableOwner::Provider {
                provider: ProviderId::new(MP),
                package: Some("tool".into()),
            },
            true,
        )
    };
    let heuristic = || {
        (
            ExecutableOwner::Provider {
                provider: ProviderId::new(HB),
                package: None,
            },
            false,
        )
    };

    // Each pair is (stronger, weaker). The stronger answer must win whichever
    // provider carries it.
    let pairs: Vec<((ExecutableOwner, bool), (ExecutableOwner, bool))> = vec![
        (owned(), (ExecutableOwner::Unknown, false)),
        (owned(), (ExecutableOwner::System, false)),
        // The one that matters most: a prefix heuristic marked as one must not
        // outrank a provider's own record just by sorting first.
        (owned(), heuristic()),
        (
            (ExecutableOwner::System, false),
            (ExecutableOwner::Unknown, false),
        ),
        (
            (ExecutableOwner::MacportsbrewShim, false),
            (ExecutableOwner::Unknown, false),
        ),
    ];

    for (stronger, weaker) in pairs {
        for (first, second) in [(HB, MP), (MP, HB)] {
            let mut snap = both_present();
            for (id, (owner, authoritative)) in
                [(first, stronger.clone()), (second, weaker.clone())]
            {
                snap.providers
                    .get_mut(&ProviderId::new(id))
                    .unwrap()
                    .resolutions
                    .insert("tool".into(), resolution(owner, authoritative));
            }
            let analysis = analyze(&snap, &policy(), &options(), AnalysisSource::Snapshot);
            let unit = analysis.packages.iter().find(|p| p.name == "tool").unwrap();
            let got = &unit
                .resolution
                .as_ref()
                .expect("the unit resolves `tool`")
                .owner;
            assert_eq!(
                got, &stronger.0,
                "with {stronger:?} under {first} and {weaker:?} under {second}, the \
                 stronger answer must win - `homebrew` sorting before `macports` is \
                 not a fact about the machine"
            );
        }
    }
}

#[test]
fn a_command_no_executor_could_run_says_so() {
    // A plan is the contract. A provider with no detected prefix yields
    // `<macports prefix>/bin/macports` as the program and `<verify>` as the
    // verb - illustrations that read exactly like commands, in a plan that
    // serialised, diffed, reviewed and passed `check_ordering`, which never
    // looks at commands (F-20).
    //
    // Unreachable through the CLI today, because a destination that is not
    // installed halts at `MANUAL_REVIEW` first. "Unreachable today" is a
    // property of the caller; this is a property of the plan.
    let mut snap = both_present();
    snap.providers
        .get_mut(&ProviderId::new(MP))
        .unwrap()
        .detection
        .prefix = None;
    let plan = plan_for(&snap, "tool", MP, true);

    let marked = plan.steps_with_placeholders();
    assert!(
        !marked.is_empty(),
        "a plan built against a provider with no prefix carries illustrations"
    );
    for step in &plan.steps {
        for c in &step.commands {
            let looks_like_one =
                c.program.starts_with('<') || c.argv.iter().any(|a| a.starts_with('<'));
            assert_eq!(
                c.placeholder, looks_like_one,
                "the flag and the text must agree, or the flag is decoration: {c:?}"
            );
        }
    }

    // And the ordinary plan carries none, so the flag means something.
    let real = plan_for(&both_present(), "tool", MP, true);
    assert!(
        real.steps_with_placeholders().is_empty(),
        "a plan against detected providers has no illustrations"
    );

    // It survives the round trip, and is written even when false - the default
    // is the permissive value, so a reader that looks must never find it
    // missing.
    let json = serde_json::to_string(&plan).unwrap();
    let back: macportsbrew_domain::MigrationPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(back.steps_with_placeholders(), marked);
    assert!(json.contains("\"placeholder\":true"), "{json}");
    let ordinary = serde_json::to_string(&real).unwrap();
    assert!(
        ordinary.contains("\"placeholder\":false"),
        "the safe reading of this flag is `true`, so `false` is stated rather \
         than left to a default: {ordinary}"
    );
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
    // And the direction decides which count is consulted, which is the part
    // the assertion here used to miss: it checked one direction against the
    // planner's own value and called that "facts fed to the engine count the
    // source side only" (F-19). The fixture makes the two directions differ, so
    // asserting both pins the claim.
    let risk = |to: &str| -> bool {
        plan_for(&snap, "tool", to, false)
            .risks
            .iter()
            .any(|r| r.id == "reverse_dependencies")
    };
    assert!(
        !risk(MP),
        "moving to macports removes from homebrew, where nothing depends on `tool`"
    );
    assert!(
        risk(HB),
        "moving to homebrew removes from macports, where `other` does - and a \
         count that crossed providers would make these two agree"
    );
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

/// A migration that genuinely installs something: Homebrew holds `gnu-sed`,
/// MacPorts does not hold `gsed`, and `data/policy/curated-mappings.toml` maps
/// the two at `EXACT`, in both directions, so the fixture works either way.
///
/// The plan reaches the mutating path because none of the planner's blocking
/// conditions fires - not because anything overrides a halt. The policy's
/// recommendation for this pair is nonetheless `MANUAL_REVIEW`, since the
/// destination's artifact availability is unknown, and the plan records that in
/// `overrode`. The assertion below pins the recommendation rather than the
/// completion: if the policy stopped saying `MANUAL_REVIEW` here, the fixture
/// would be describing a different situation than the one it was chosen for,
/// and that should be visible rather than silent.
fn install_needed(source: &str) -> Snapshot {
    let held = |id: &str, name: &'static str| {
        (id == source).then_some(Pkg {
            name,
            requested: true,
            homepage: None,
            service: false,
        })
    };
    let other = || Pkg {
        name: "other",
        requested: true,
        homepage: None,
        service: false,
    };
    snapshot(
        provider(
            HB,
            "/opt/homebrew",
            true,
            &[held(HB, "gnu-sed").unwrap_or_else(other)],
            &[],
        ),
        provider(
            MP,
            "/opt/local",
            true,
            &[held(MP, "gsed").unwrap_or_else(other)],
            &[],
        ),
    )
}

/// Every provider prefix this snapshot detected, so the leak check below names
/// no literal. A MacPorts install at a non-default prefix would otherwise make
/// the check vacuous - it would look for `/opt/local` in a `PATH` that never
/// had it - and a third provider would make it wrong. `registry.rs` is meant to
/// be the only place that knows where a provider lives; a test should not be a
/// second one.
fn detected_prefixes(snap: &Snapshot) -> Vec<PathBuf> {
    snap.providers
        .values()
        .filter_map(|p| p.detection.prefix.clone())
        .collect()
}

/// The prefix a command's program actually lives under: the longest detected
/// prefix that contains it, compared by path component rather than by string,
/// so a provider at `/opt/local-arm` is not read as living under `/opt/local`.
/// That mistake would have been quiet rather than loud - a command counted as
/// belonging to both prefixes has no *other* prefix left to leak, so its check
/// would pass by having nothing to check.
fn owning_prefix<'a>(program: &str, prefixes: &'a [PathBuf]) -> Option<&'a PathBuf> {
    let p = Path::new(program);
    prefixes
        .iter()
        .filter(|prefix| p.starts_with(prefix))
        .max_by_key(|prefix| prefix.components().count())
}

fn check_invocation_environment(plan: &MigrationPlan, snap: &Snapshot, expected_tools: usize) {
    // Stated rather than assumed: `SET_PRIMARY` is only emitted when nothing
    // sent the plan to manual review, so a manual plan has no commands at all
    // and the counts below would fail for a reason that is not a defect.
    assert!(
        plan.steps.iter().all(|s| s.kind != StepKind::ManualReview),
        "this helper describes a plan that reaches the mutating path"
    );
    let prefixes = detected_prefixes(snap);
    assert!(
        prefixes.len() > 1,
        "the leak check needs another prefix to look for"
    );
    // What a provider invocation is *called*, so a provider command relabelled
    // as the tool is caught by what it runs rather than by where it lives. Where
    // it lives cannot be the test: once the executor resolves the
    // self-invocation, most likely with `current_exe()`, a MacPortsBrew installed *by* either
    // provider is legitimately under that provider's prefix.
    //
    // From the same table the planner builds commands out of, not from this
    // plan's own commands. Plan-local was the first attempt and it had a
    // reachable hole: a retain-source plan never invokes the source provider, so
    // that provider's program was missing from the set and a command named after
    // it would have passed.
    let table = provider_commands::load().expect("the provider command table");
    // `file_name`, because the table stores `bin/brew` and `bin/port` while a
    // command's program is a full path. Comparing the two raw forms is a check
    // that can never fire, which is what the first attempt did.
    let provider_programs: BTreeSet<&str> = table
        .values()
        .filter_map(|e| {
            Path::new(&e.executable)
                .file_name()
                .and_then(|n| n.to_str())
        })
        .collect();
    assert!(
        !provider_programs.is_empty(),
        "an empty set would make the check below vacuous"
    );
    let (mut tools, mut commands) = (0usize, 0usize);
    for step in &plan.steps {
        for c in &step.commands {
            commands += 1;
            // Which provider a command belongs to is decided by where its
            // program lives, not by its target - so a mislabelled command is
            // still measured against the right prefix.
            let own = owning_prefix(&c.program, &prefixes);
            // INV-10 in its own words, for every command in the plan: no other
            // provider's prefix reaches the environment this one runs with.
            // `contains`, not a `PATH`-shaped split: `LDFLAGS` and `CPPFLAGS`
            // are named by INV-10 and their values look like
            // `-I/opt/local/include`, which no split on `:` would find.
            for other in prefixes.iter().filter(|p| Some(*p) != own) {
                let other = other.to_string_lossy();
                for (k, v) in &c.environment {
                    assert!(!v.contains(other.as_ref()), "{other} reaches {k} of {c:?}");
                }
            }
            match c.target {
                CommandTarget::Provider => {
                    assert!(
                        !c.program.starts_with('<'),
                        "the prefix was not detected, so this is a display string \
                         rather than a program: {c:?}"
                    );
                    assert!(
                        Path::new(&c.program).is_absolute(),
                        "a provider program is absolute so `PATH` cannot decide it: {c:?}"
                    );
                    assert!(
                        own.is_some(),
                        "a provider command runs out of a detected prefix: {c:?}"
                    );
                    assert!(c.environment.contains_key("PATH"), "{c:?}");
                    for k in c.environment.keys() {
                        assert!(
                            !is_build_flag(k),
                            "INV-10 names {k}, so a provider command must not carry it: {c:?}"
                        );
                    }
                    // The descriptive environment_info keys never leak into a command.
                    assert!(
                        !c.environment.contains_key("prefix") && !c.environment.contains_key("bin")
                    );
                }
                CommandTarget::Tool => {
                    tools += 1;
                    // Both halves of the contract `plan.v1.md` states, because
                    // the type alone does not stop a provider command being
                    // given the other label: a `TOOL` command hangs off
                    // `SET_PRIMARY`, and it is not one of the programs this plan
                    // invokes providers with.
                    assert_eq!(
                        step.kind,
                        StepKind::SetPrimary,
                        "the only step that invokes this tool is primary selection: {c:?}"
                    );
                    let name = Path::new(&c.program)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default();
                    assert!(
                        !provider_programs.contains(name),
                        "{name} is how a provider is invoked, so this is a provider command: {c:?}"
                    );
                    assert_eq!(
                        c.argv.first().map(String::as_str),
                        Some("primary"),
                        "the only thing this tool invokes itself for is primary selection"
                    );
                    assert_eq!(
                        c.elevation,
                        Elevation::None,
                        "primary selection stays off the elevating path (SE-036)"
                    );
                }
            }
        }
    }
    assert!(commands > 0);
    // Not `<= 1`: zero would mean the `TOOL` arm above is dead code and the
    // suite green anyway, which is the shape of the defect this test was
    // rewritten to remove.
    assert_eq!(tools, expected_tools, "self-invocations in {plan:?}");
}
#[test]
fn invariant_10_plan_commands_carry_the_sanitised_invocation_environment() {
    // This test used to read `.filter(|c| c.program != "macportsbrew")`, which
    // excluded the one command in a plan that INV-10 does not reach - the
    // primary-selection step, which carried no environment at all and a relative
    // program the executor refuses by design. A gate
    // that is green because its exception was written into it is worse than an
    // absent one, because it is cited as evidence - and this suite's green
    // status is cited in the report, the README and the wiki.
    //
    // Nothing is skipped now. Each command is checked against the rule that
    // actually governs it, and which rule that is comes from the type rather
    // than from a string comparison that a rename would silently defeat. What
    // this asserts about a `TOOL` command is only what INV-10 has to say about
    // one - that it is not a provider invocation wearing the other label. The
    // *shape* of the self-invocation is pinned in its own test below, because a
    // test named after an invariant should not be what a later fix has to edit.
    //
    // Two installing plans, in both directions (INV-17), because every shipped
    // fixture has the destination already holding the package, so its
    // `ADD_DESTINATION` carries no command - and a suite that never sees an
    // install step is not checking the environment of the one command that
    // installs software.
    for (source, name, to, installed) in [(HB, "gnu-sed", MP, "gsed"), (MP, "gsed", HB, "gnu-sed")]
    {
        let snap = install_needed(source);
        let plan = plan_for(&snap, name, to, false);
        assert!(
            plan.steps.iter().any(|s| s.kind == StepKind::AddDestination
                && s.commands
                    .iter()
                    .any(|c| c.argv.iter().any(|a| a == installed))),
            "the plan toward {to} must add {installed} for real"
        );
        assert_eq!(
            plan.overrode,
            Some(Outcome::ManualReview),
            "the recommendation this fixture was chosen for"
        );
        check_invocation_environment(&plan, &snap, 1);
    }
    let snap = both_present();
    check_invocation_environment(&plan_for(&snap, "tool", MP, false), &snap, 1);
    // With the source retained, the plan never invokes the source provider. That
    // is the case that decided where the name set comes from - derived from this
    // plan's own commands it was short by one provider - so it is the case the
    // suite has to keep exercising.
    check_invocation_environment(&plan_for(&snap, "tool", MP, true), &snap, 1);
}

/// F-04 found three things wrong with the primary-selection command: the INV-10
/// test filtered it out, its program is relative where `process/system.rs`
/// refuses relative programs, and a bare name is resolved through `PATH` - the
/// shadowing hazard `doctor` exists to warn about. Removing the filter fixed the
/// first. The other two are the executor pass's to settle, most likely by
/// running the binary this process is already running rather than spawning a
/// name at all (SE-056).
///
/// So the current shape is pinned here and not in the test named after INV-10.
/// This is a record of what is true today; changing it should mean editing a
/// test that says so, rather than one whose name would lend the old shape an
/// authority the invariant never gave it.
#[test]
fn the_self_invocation_has_the_shape_the_executor_pass_will_replace() {
    let plan = plan_for(&both_present(), "tool", MP, false);
    let tool: Vec<_> = plan
        .steps
        .iter()
        .flat_map(|s| s.commands.iter())
        .filter(|c| c.target == CommandTarget::Tool)
        .collect();
    assert_eq!(tool.len(), 1);
    let c = tool[0];
    assert_eq!(c.program, "macportsbrew");
    assert!(
        !Path::new(&c.program).is_absolute(),
        "not yet resolved to this process's own binary"
    );
    assert!(c.environment.is_empty(), "no environment of its own yet");
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
    // The assertion this test was missing. Renaming the *contents* of the two
    // providers leaves the map keys alone, so iteration order never changes and
    // anything that silently depends on it is unexercised - which is how a
    // whole class of direction asymmetry hid behind a passing INV-17 test
    // (F-19). `source` is the field that class lands on: which provider the
    // unit is considered to be *on*.
    assert_eq!(
        flip(&unit_a.source),
        unit_b.source,
        "swapping the two providers must swap which one the unit is sourced from"
    );
    // And the same for what each row says, not merely how many there are. The
    // provider id *inside* an installed package is excluded deliberately: the
    // homebrew row names homebrew in both analyses, which is the one thing that
    // must not mirror.
    for (id, row_a) in &unit_a.rows {
        let row_b = &unit_b.rows[&flip(id)];
        let shape = |r: &macportsbrew_application::ProviderRow| {
            (
                r.installed
                    .iter()
                    .map(|p| (p.identity.name.clone(), p.version.clone(), p.active))
                    .collect::<Vec<_>>(),
                r.dependency_count,
                r.reverse_dependency_count,
                r.availability.clone(),
            )
        };
        assert_eq!(
            shape(row_a),
            shape(row_b),
            "the row for {id} must mirror the row for {}",
            flip(id)
        );
    }
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

#[test]
fn invariant_06_a_step_whose_job_is_to_read_never_elevates() {
    // INV-06 stated where it is a property of the *plan*, not of a rendering,
    // and asserted in both directions because the data differs per provider
    // while the code does not (INV-17).
    //
    // The CLI test of the same name checks that the summary does not *present*
    // a read as elevating. `spec-guardian` showed by mutation that this is not
    // enough: flip one provider's `verify` elevation cell to `required` and the
    // binary prints "Steps that need administrator rights: 3, 6" with step 3 a
    // read - INV-06's exact failure, rendered to a user - while every test in
    // the repository stays green. This assertion fails on that mutation, which
    // was confirmed by making it and watching this test go red rather than by
    // reasoning that it would.
    //
    // The property is about a step's *job*, not about the plan as a whole. A
    // plan may legitimately contain elevating steps, so "nothing here elevates"
    // would be a claim about the fixture. A verification step reads, and a read
    // never needs administrator rights.
    let mut checked = 0usize;
    for (source, to, package) in [(HB, MP, "gnu-sed"), (MP, HB, "gsed")] {
        let snapshot = install_needed(source);
        // **What this test is exercising, asserted rather than assumed.** As of
        // 2026-09-19 a detected `UnprivilegedRead` relaxes a read's elevation
        // whatever the table says, so a fixture carrying it would satisfy every
        // assertion below without the stored cell ever being consulted — and
        // the mutation this test was proved against (flip a `verify` cell to
        // `required`) would stop reaching it. These snapshots carry an empty
        // capability table, which is what makes the assertions about the data
        // file. If that ever changes, this fails here and says why rather than
        // going quietly vacuous (`spec-guardian`).
        for (id, provider) in &snapshot.providers {
            assert!(
                !provider
                    .capabilities
                    .capabilities
                    .contains_key(&Capability::UnprivilegedRead),
                "{id}: this fixture must not pre-empt the stored elevation, or this test \
                 stops testing what it is named for"
            );
        }
        let plan = plan_for(&snapshot, package, to, false);
        for step in &plan.steps {
            if !matches!(
                step.kind,
                StepKind::VerifyDestination | StepKind::VerifyResolution | StepKind::VerifyRemoval
            ) {
                continue;
            }
            for command in &step.commands {
                checked += 1;
                assert!(
                    matches!(command.elevation, Elevation::None),
                    "{source} -> {to}: {:?} runs `{}` and claims elevation {:?}; a step whose \
                     job is to read must never need administrator rights (INV-06)",
                    step.kind,
                    command.argv.join(" "),
                    command.elevation
                );
            }
        }
    }
    assert!(
        checked >= 2,
        "a verification command must have been examined in each direction, or \
         this test passes by looking at nothing: {checked}"
    );
}

/// INV-20's second sentence, the half that is visible from outside this crate:
/// **detection may relax a read and must never touch a mutation.**
///
/// "Where behaviour can be detected at runtime, detection wins over stored
/// knowledge" is a dangerous sentence if taken further than it was meant. The
/// preference added for F-35 is scoped to read operations precisely because
/// relaxing `add` or `remove` would be a rule rewriting the elevation of a
/// *mutating* step from an observation about a *read*. MacPorts' shipped cells
/// for those two are `required`, so this is assertable against the real data.
///
/// The other half — a detected capability beating a stored `required` on a
/// read — is not reachable from here: `build_plan` loads the commands table
/// itself, and no shipped cell disagrees with detection, so the override has no
/// observable effect on today's data. It is tested in `planner.rs`'s own module,
/// where `command()` takes the table as an argument, and the reason it lives
/// there is written beside it.
#[test]
fn invariant_20_detection_relaxes_a_read_and_never_a_mutation() {
    let mut snap = install_needed(HB);
    for provider in snap.providers.values_mut() {
        provider
            .capabilities
            .capabilities
            .insert(Capability::UnprivilegedRead, CapabilityState::Available);
    }
    let plan = plan_for(&snap, "gnu-sed", MP, false);

    let mut reads = 0usize;
    let mut mutations = 0usize;
    for step in &plan.steps {
        for command in &step.commands {
            match step.kind {
                StepKind::VerifyDestination | StepKind::VerifyRemoval => {
                    reads += 1;
                    assert_eq!(
                        command.elevation,
                        Elevation::None,
                        "a provider that demonstrated it can be read unprivileged must \
                         carry no elevation on {:?}",
                        step.kind
                    );
                }
                // The destination only. `REMOVE_SOURCE` acts on the *source*,
                // which here is Homebrew, whose `remove` cell is `none` — so
                // asserting `Required` on it asserted the wrong provider's data.
                // Caught by running the test rather than by reading it.
                StepKind::AddDestination => {
                    mutations += 1;
                    assert!(
                        matches!(command.elevation, Elevation::Required { .. }),
                        "adding under MacPorts is `required` in the shipped data, and an \
                         observation about a *read* must not relax a *mutation*: {:?}",
                        command.elevation
                    );
                }
                _ => {}
            }
        }
    }
    assert!(
        reads > 0 && mutations > 0,
        "both kinds must have been examined or this proves nothing: \
         {reads} read(s), {mutations} mutation(s)"
    );

    // **Which provider's capability decides.** `command()` reads it from the
    // `ProviderSnapshot` it is handed, and the unit tests pin that. What they
    // cannot see is whether `build_plan` hands it the *right* one: a caller
    // reading `snapshot.providers.values().any(..)` instead of the passed
    // provider would let one provider's observation relax the other's read —
    // the INV-07/INV-08 shape — and every assertion above would stay green,
    // because this fixture sets the capability on both (`spec-guardian`).
    //
    // Pinned by program, which is the only thing in a command that names the
    // provider it will run. `VERIFY_REMOVAL`'s counterpart is asserted in
    // `invariant_01_a_verification_step_asserts_what_its_name_says`.
    let program_of = |kind: StepKind| {
        plan.steps
            .iter()
            .find(|s| s.kind == kind)
            .and_then(|s| s.commands.first())
            .map(|c| c.program.clone())
    };
    assert_eq!(
        program_of(StepKind::VerifyDestination),
        program_of(StepKind::AddDestination),
        "the destination's verification must read the provider it was added under"
    );
    assert_ne!(
        program_of(StepKind::VerifyDestination),
        program_of(StepKind::RemoveSource),
        "and must not be the provider the package is being removed from"
    );
}

#[test]
fn invariant_04_removal_with_dependents_is_blocked_and_explained() {
    // Moving `tool` to Homebrew would remove it from MacPorts, where `other`
    // depends on it. The plan may be built, but the removal is blocked: a
    // manual-review step stands before any removal, and the reason names the
    // dependents. The other direction removes from Homebrew, where nothing
    // depends on `tool`, and is not blocked for that reason.
    let snap = both_present();
    let plan = plan_for(&snap, "tool", HB, false);
    let kinds: Vec<StepKind> = plan.steps.iter().map(|s| s.kind).collect();
    let review = kinds.iter().position(|k| *k == StepKind::ManualReview);
    let removal = kinds.iter().position(|k| *k == StepKind::RemoveSource);
    assert!(review.is_some(), "the removal is blocked: {kinds:?}");
    if let (Some(r), Some(x)) = (review, removal) {
        assert!(r < x, "the review stands before the removal: {kinds:?}");
    }
    assert!(
        plan.risks
            .iter()
            .any(|r| r.id == "reverse_dependencies" && r.blocking),
        "and it is explained: {:?}",
        plan.risks
    );
    let mirror = plan_for(&snap, "tool", MP, false);
    assert!(
        !mirror.risks.iter().any(|r| r.id == "reverse_dependencies"),
        "nothing under homebrew depends on `tool`: {:?}",
        mirror.risks
    );
}
