// SPDX-License-Identifier: Apache-2.0

//! The primary decision rule (F.md Design §4; 05's MUST step list 1–6;
//! SE-112), unit by unit.
//!
//! Every case starts from the **captured** live snapshot
//! (`fixtures/snapshots/both-providers-intel-macos26-live.json`, 2026-10-28) and
//! its real `PATH` — `/usr/local/bin` at 6, `/usr/local/sbin` 7, `/usr/bin` 10,
//! `/opt/local/bin` 25 — with the package rows the case needs constructed **in
//! test code** and never written to a file (SG2-N7). The file is read, not
//! changed.

use macportsbrew_application::primary::{
    decide, destination_index, system_tools_keeping_their_names, DirectoryIndex, PrimaryQuestion,
};
use macportsbrew_application::{build_plan, AnalysisOptions, PlanRequest};
use macportsbrew_domain::{
    ExecutableList, ExecutableName, ExecutableOwner, Placement, PrimaryDecision, PrimaryReason,
    ProviderId, Resolution, ResolutionState, Snapshot, StepKind, VerificationCheck,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const HB: &str = "homebrew";
const MP: &str = "macports";

fn live() -> Snapshot {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/snapshots/both-providers-intel-macos26-live.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("the captured snapshot"))
        .expect("it parses")
}

fn name(n: &str) -> ExecutableName {
    ExecutableName::parse(n).unwrap()
}

fn id(p: &str) -> ProviderId {
    ProviderId::new(p)
}

/// The captured `PATH` before any case touches it; the indices the cases rely
/// on, checked once so a recapture that moves them fails here, by name.
fn path_of(snap: &Snapshot) -> Vec<String> {
    let path = snap.environment.path.clone();
    assert_eq!(path[6], "/usr/local/bin");
    assert_eq!(path[7], "/usr/local/sbin");
    assert_eq!(path[10], "/usr/bin");
    assert_eq!(path[25], "/opt/local/bin");
    path
}

/// Gives `package` under `provider` the list `names`, placed as `placement`.
fn list(snap: &mut Snapshot, provider: &str, package: &str, names: &[&str], placement: Placement) {
    let meta = snap
        .providers
        .get_mut(&id(provider))
        .unwrap()
        .inventory
        .metadata
        .entry(package.to_string())
        .or_default();
    meta.set_executables(
        ExecutableList::from_leaves(names.iter().copied()),
        placement,
    );
}

fn on(dir: &str, names: &[&str]) -> Placement {
    Placement::OnPathDir {
        dirs: names
            .iter()
            .map(|n| (name(n), dir.to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// Records `executable` as resolving to `owner` at `index`.
fn found(
    snap: &mut Snapshot,
    executable: &str,
    dir: &str,
    owner: ExecutableOwner,
    authoritative: bool,
    index: Option<u32>,
) {
    snap.environment.resolutions.insert(
        name(executable),
        ResolutionState::Found {
            resolution: Resolution {
                executable: executable.into(),
                path: Some(PathBuf::from(dir).join(executable)),
                canonical_path: None,
                owner,
                authoritative,
            },
            path_index: index,
        },
    );
}

fn owned(provider: &str, package: Option<&str>) -> ExecutableOwner {
    ExecutableOwner::Provider {
        provider: id(provider),
        package: package.map(str::to_string),
    }
}

fn question<'a>(
    source: &'a ProviderId,
    package: &'a str,
    destination: &'a ProviderId,
    installed: bool,
) -> PrimaryQuestion<'a> {
    PrimaryQuestion {
        source,
        source_package: package,
        destination,
        destination_package: Some(package),
        destination_installed: installed,
        one_to_one: true,
    }
}

/// The reason for `executable`, or `None` when it is planned.
fn reason_for(decision: &PrimaryDecision, executable: &str) -> Option<PrimaryReason> {
    match decision {
        PrimaryDecision::Refused { reasons, .. } => reasons.get(&name(executable)).copied(),
        _ => None,
    }
}

/// `htop` in both providers, as captured: Homebrew's linked into
/// `/usr/local/bin` and winning at 6 by its own records; MacPorts' in
/// `/opt/local/bin` (25).
fn htop_both_ways() -> Snapshot {
    let mut snap = live();
    let _ = path_of(&snap);
    list(
        &mut snap,
        HB,
        "htop",
        &["htop"],
        on("/usr/local/bin", &["htop"]),
    );
    list(
        &mut snap,
        MP,
        "htop",
        &["htop"],
        on("/opt/local/bin", &["htop"]),
    );
    found(
        &mut snap,
        "htop",
        "/usr/local/bin",
        owned(HB, Some("htop")),
        true,
        Some(6),
    );
    snap
}

#[test]
fn a_destination_that_already_wins_has_nothing_to_change() {
    let snap = htop_both_ways();
    let (mp, hb) = (id(MP), id(HB));
    let decision = decide(&snap, &question(&mp, "htop", &hb, true));
    assert_eq!(
        decision,
        PrimaryDecision::NothingToChange {
            executables: [name("htop")].into_iter().collect(),
            path_used: snap.environment.path.clone(),
        }
    );
}

#[test]
fn a_destination_later_on_path_needs_a_shim() {
    let snap = htop_both_ways();
    let (mp, hb) = (id(MP), id(HB));
    let decision = decide(&snap, &question(&hb, "htop", &mp, true));
    assert_eq!(
        reason_for(&decision, "htop"),
        Some(PrimaryReason::PrimaryNeedsShim),
        "d = 25 > w = 6: {decision:?}"
    );
    assert_eq!(
        decision.needing_a_shim(),
        [name("htop")].into_iter().collect()
    );
}

#[test]
fn invariant_17_the_decision_rule_is_the_same_for_both_directions() {
    // One rule over each provider's row of one snapshot, with its one captured
    // `PATH` (SG-14): no permuted `PATH`, no rule that names a provider. The
    // outcomes differ only because the captured placements do (DP-047's
    // asymmetry, answered by part I).
    let mut snap = htop_both_ways();
    // m4: `/usr/bin` (10) wins today, a file no provider owns.
    list(
        &mut snap,
        HB,
        "m4",
        &["m4"],
        Placement::NotLinked {
            reason: "keg-only".into(),
        },
    );
    list(&mut snap, MP, "m4", &["m4"], on("/opt/local/bin", &["m4"]));
    found(
        &mut snap,
        "m4",
        "/usr/bin",
        ExecutableOwner::Unknown,
        false,
        Some(10),
    );
    let (mp, hb) = (id(MP), id(HB));
    for (from, to) in [(&mp, &hb), (&hb, &mp)] {
        assert_eq!(
            reason_for(&decide(&snap, &question(from, "m4", to, true)), "m4"),
            Some(PrimaryReason::NonProviderEntryWins),
            "m4 toward {to}"
        );
        assert_eq!(
            reason_for(&decide(&snap, &question(from, "htop", to, false)), "htop"),
            if to == &hb {
                // Already the destination's own: step 3 settles it before the
                // destination's installation is ever asked about.
                None
            } else {
                Some(PrimaryReason::PrimaryDestinationNotInstalled)
            },
            "an uninstalled destination toward {to}"
        );
    }
    assert!(matches!(
        decide(&snap, &question(&mp, "htop", &hb, true)),
        PrimaryDecision::NothingToChange { .. }
    ));
    assert_eq!(
        reason_for(&decide(&snap, &question(&hb, "htop", &mp, true)), "htop"),
        Some(PrimaryReason::PrimaryNeedsShim)
    );
}

#[test]
fn an_uninstalled_destination_is_refused_not_installed_in_both_directions() {
    // DP-055, default (a). The winner is the source's own copy, so step 3
    // does not settle it and step 5 asks about the destination.
    let mut snap = live();
    list(
        &mut snap,
        HB,
        "tree",
        &["tree"],
        on("/usr/local/bin", &["tree"]),
    );
    list(
        &mut snap,
        MP,
        "tree",
        &["tree"],
        on("/opt/local/bin", &["tree"]),
    );
    let (mp, hb) = (id(MP), id(HB));
    for (from, to, dir, idx) in [
        (&hb, &mp, "/usr/local/bin", 6),
        (&mp, &hb, "/opt/local/bin", 25),
    ] {
        let mut snap = snap.clone();
        found(
            &mut snap,
            "tree",
            dir,
            owned(from.as_str(), Some("tree")),
            true,
            Some(idx),
        );
        assert_eq!(
            reason_for(&decide(&snap, &question(from, "tree", to, false)), "tree"),
            Some(PrimaryReason::PrimaryDestinationNotInstalled),
            "toward {to}"
        );
    }
}

#[test]
fn a_keg_only_destination_needs_a_shim() {
    // SG-5: unit-only, as the capture may hold no such executable. The source
    // wins at `/opt/local/bin` and the destination keeps its copy off its
    // linked directories.
    let mut snap = live();
    list(
        &mut snap,
        MP,
        "ncurses",
        &["ncursesw6-config"],
        on("/opt/local/bin", &["ncursesw6-config"]),
    );
    list(
        &mut snap,
        HB,
        "ncurses",
        &["ncursesw6-config"],
        Placement::NotLinked {
            reason: "macOS already provides this software".into(),
        },
    );
    found(
        &mut snap,
        "ncursesw6-config",
        "/opt/local/bin",
        owned(MP, Some("ncurses")),
        true,
        Some(25),
    );
    let (mp, hb) = (id(MP), id(HB));
    assert_eq!(
        reason_for(
            &decide(&snap, &question(&mp, "ncurses", &hb, true)),
            "ncursesw6-config"
        ),
        Some(PrimaryReason::PrimaryNeedsShim)
    );
}

#[test]
fn a_relative_entry_below_the_destination_is_unpredictable() {
    // SR-4: a shell would search the relative entry first.
    let mut snap = htop_both_ways();
    snap.environment.path.insert(3, "relative/bin".into());
    // The winner's recorded index is only an index; the relative entry sits
    // below the destination's directory, which is what this rule reads.
    found(
        &mut snap,
        "htop",
        "/opt/local/bin",
        owned(MP, Some("htop")),
        true,
        Some(26),
    );
    let (mp, hb) = (id(MP), id(HB));
    assert_eq!(
        reason_for(&decide(&snap, &question(&mp, "htop", &hb, true)), "htop"),
        Some(PrimaryReason::PrimaryUnpredictable)
    );
    assert_eq!(
        destination_index(&snap.environment.path, "/usr/local/bin"),
        DirectoryIndex::Unpredictable
    );
    // An empty entry counts as relative too.
    assert_eq!(
        destination_index(&[String::new(), "/a".into()], "/a"),
        DirectoryIndex::Unpredictable
    );
    // `~/…` is a redacted absolute entry, compared by component.
    assert_eq!(
        destination_index(&["~/bin".into(), "/a/./".into()], "/a"),
        DirectoryIndex::At(1)
    );
    assert_eq!(
        destination_index(&["/b".into()], "/a"),
        DirectoryIndex::Absent
    );
}

#[test]
fn a_missing_path_index_is_unpredictable() {
    // SR-5, and mutant 15: a winner with no recorded index is never index 0.
    // With `w` read as 0, `d = 6 > 0` would say "needs a shim"; the honest
    // answer is that the comparison cannot be made.
    let mut snap = htop_both_ways();
    found(
        &mut snap,
        "htop",
        "/opt/local/bin",
        owned(MP, Some("htop")),
        true,
        None,
    );
    let (mp, hb) = (id(MP), id(HB));
    assert_eq!(
        reason_for(&decide(&snap, &question(&mp, "htop", &hb, true)), "htop"),
        Some(PrimaryReason::PrimaryUnpredictable)
    );
}

#[test]
fn the_index_comparison_predicts_a_win_only_below_the_winner() {
    // `d < w` is a predicted win, `d > w` a shim, `d == w` the same directory
    // (mutant 4: the comparison negated).
    let (mp, hb) = (id(MP), id(HB));
    for (w, expected) in [
        (30, None),
        (3, Some(PrimaryReason::PrimaryNeedsShim)),
        (6, Some(PrimaryReason::SameDirectory)),
    ] {
        let mut snap = htop_both_ways();
        found(
            &mut snap,
            "htop",
            "/x",
            owned(MP, Some("htop")),
            true,
            Some(w),
        );
        assert_eq!(
            reason_for(&decide(&snap, &question(&mp, "htop", &hb, true)), "htop"),
            expected,
            "w = {w}"
        );
    }
    // Only an observed miss predicts a win (SG-2).
    let mut snap = htop_both_ways();
    snap.environment
        .resolutions
        .insert(name("htop"), ResolutionState::NotOnPath);
    assert!(matches!(
        decide(&snap, &question(&mp, "htop", &hb, true)),
        PrimaryDecision::NothingToChange { .. }
    ));
}

#[test]
fn owned_by_another_package_is_refused() {
    // SR-3, and mutant 16: `package: None` is not the destination package.
    let (mp, hb) = (id(MP), id(HB));
    for package in [Some("not-htop"), None] {
        let mut snap = htop_both_ways();
        found(
            &mut snap,
            "htop",
            "/usr/local/bin",
            owned(HB, package),
            true,
            Some(6),
        );
        assert_eq!(
            reason_for(&decide(&snap, &question(&mp, "htop", &hb, true)), "htop"),
            Some(PrimaryReason::OwnedByAnotherPackage),
            "{package:?}"
        );
    }
}

#[test]
fn a_non_authoritative_provider_winner_is_winner_not_authoritative() {
    // SG2-N4, mutant 18c: never called a system tool, never a pass.
    let (mp, hb) = (id(MP), id(HB));
    for owner in [MP, HB] {
        let mut snap = htop_both_ways();
        found(
            &mut snap,
            "htop",
            "/usr/local/bin",
            owned(owner, Some("htop")),
            false,
            Some(6),
        );
        assert_eq!(
            reason_for(&decide(&snap, &question(&mp, "htop", &hb, true)), "htop"),
            Some(PrimaryReason::WinnerNotAuthoritative),
            "owned by {owner}, by prefix"
        );
    }
}

#[test]
fn a_name_a_system_entry_shadows_is_refused() {
    // Step 4 (mutant 1): `/usr/bin` wins and is not displaced.
    let (mp, hb) = (id(MP), id(HB));
    for owner in [
        ExecutableOwner::System,
        ExecutableOwner::External,
        ExecutableOwner::Unknown,
    ] {
        let mut snap = htop_both_ways();
        found(
            &mut snap,
            "htop",
            "/usr/bin",
            owner.clone(),
            false,
            Some(10),
        );
        assert_eq!(
            reason_for(&decide(&snap, &question(&mp, "htop", &hb, true)), "htop"),
            Some(PrimaryReason::NonProviderEntryWins),
            "{owner:?}"
        );
    }
}

#[test]
fn invariant_15_a_replayed_plan_is_refused_primary_resolution_unknown() {
    // A replayed capture records nothing for any name: NOT_OBSERVED, never
    // NOT_ON_PATH (mutant 14), so the decision refuses rather than predicts.
    let mut snap = htop_both_ways();
    snap.environment.resolutions.clear();
    for p in snap.providers.values_mut() {
        p.resolutions.clear();
    }
    let (mp, hb) = (id(MP), id(HB));
    assert_eq!(
        reason_for(&decide(&snap, &question(&mp, "htop", &hb, true)), "htop"),
        Some(PrimaryReason::PrimaryResolutionUnknown)
    );
}

#[test]
fn an_unknown_list_is_never_the_package_name() {
    // Step 1, mutant 9: the package name is not a list.
    let snap = live();
    let (mp, hb) = (id(MP), id(HB));
    assert_eq!(
        decide(&snap, &question(&mp, "htop", &hb, true)),
        PrimaryDecision::Refused {
            executables: BTreeSet::new(),
            reasons: BTreeMap::new(),
            package: Some(PrimaryReason::ExecutablesUnknown),
            path_used: snap.environment.path.clone(),
        }
    );
}

#[test]
fn a_mapping_that_is_not_one_to_one_is_refused() {
    let snap = htop_both_ways();
    let (mp, hb) = (id(MP), id(HB));
    let mut q = question(&mp, "htop", &hb, true);
    q.one_to_one = false;
    assert!(matches!(
        decide(&snap, &q),
        PrimaryDecision::Refused {
            package: Some(PrimaryReason::PrimaryMappingNotOneToOne),
            ..
        }
    ));
}

#[test]
fn the_decision_is_all_or_nothing_and_lists_every_refused_executable() {
    // Mutant 17: one executable refused refuses the package, and the refusal
    // names each.
    let mut snap = htop_both_ways();
    list(
        &mut snap,
        MP,
        "htop",
        &["htop", "htop-extra"],
        on("/opt/local/bin", &["htop", "htop-extra"]),
    );
    list(
        &mut snap,
        HB,
        "htop",
        &["htop", "htop-extra"],
        on("/usr/local/bin", &["htop", "htop-extra"]),
    );
    found(
        &mut snap,
        "htop-extra",
        "/usr/bin",
        ExecutableOwner::System,
        false,
        Some(10),
    );
    let (mp, hb) = (id(MP), id(HB));
    let decision = decide(&snap, &question(&mp, "htop", &hb, true));
    match &decision {
        PrimaryDecision::Refused {
            executables,
            reasons,
            package: None,
            ..
        } => {
            assert_eq!(executables.len(), 2, "the whole list stays on the decision");
            assert_eq!(
                reasons
                    .iter()
                    .map(|(e, r)| (e.as_str(), *r))
                    .collect::<Vec<_>>(),
                [("htop-extra", PrimaryReason::NonProviderEntryWins)]
            );
        }
        other => panic!("refused, not {other:?}"),
    }
}

#[test]
fn a_destination_that_lacks_the_executable_is_refused() {
    let mut snap = htop_both_ways();
    list(
        &mut snap,
        HB,
        "htop",
        &["not-htop"],
        on("/usr/local/bin", &["not-htop"]),
    );
    found(
        &mut snap,
        "htop",
        "/opt/local/bin",
        owned(MP, Some("htop")),
        true,
        Some(25),
    );
    let (mp, hb) = (id(MP), id(HB));
    assert_eq!(
        reason_for(&decide(&snap, &question(&mp, "htop", &hb, true)), "htop"),
        Some(PrimaryReason::DestinationLacksExecutable)
    );
}

#[test]
fn an_executable_not_named_after_its_package_is_decided_by_its_own_name() {
    // DP-047 (b): the list, not the package name, is what is decided. `xz`
    // ships `lzcat`; the decision is about `lzcat`.
    let mut snap = live();
    list(
        &mut snap,
        HB,
        "xz",
        &["lzcat"],
        on("/usr/local/bin", &["lzcat"]),
    );
    list(
        &mut snap,
        MP,
        "xz",
        &["lzcat"],
        on("/opt/local/bin", &["lzcat"]),
    );
    found(
        &mut snap,
        "lzcat",
        "/usr/local/bin",
        owned(HB, Some("xz")),
        true,
        Some(6),
    );
    let (mp, hb) = (id(MP), id(HB));
    assert_eq!(
        decide(&snap, &question(&mp, "xz", &hb, true)),
        PrimaryDecision::NothingToChange {
            executables: [name("lzcat")].into_iter().collect(),
            path_used: snap.environment.path.clone(),
        }
    );
}

#[test]
fn an_installed_destination_s_extra_system_names_are_listed() {
    // SG-8: a name the destination ships outside the source's list, won today
    // by a file no provider owns, is stated as staying the system's.
    let mut snap = htop_both_ways();
    list(
        &mut snap,
        HB,
        "htop",
        &["htop", "m4"],
        on("/usr/local/bin", &["htop", "m4"]),
    );
    found(
        &mut snap,
        "m4",
        "/usr/bin",
        ExecutableOwner::Unknown,
        false,
        Some(10),
    );
    let (mp, hb) = (id(MP), id(HB));
    assert_eq!(
        system_tools_keeping_their_names(&snap, &question(&mp, "htop", &hb, true)),
        [name("m4")].into_iter().collect()
    );
    assert!(system_tools_keeping_their_names(&snap, &question(&mp, "htop", &hb, false)).is_empty());
}

// --- the plan the planner builds from the decision ---------------------------

fn plan(snap: &Snapshot, package: &str, to: &str) -> macportsbrew_domain::MigrationPlan {
    build_plan(
        snap,
        &PolicyData::load_embedded().unwrap(),
        &AnalysisOptions {
            today: "2026-11-01".into(),
            advanced: false,
            only: BTreeSet::new(),
        },
        &PlanRequest {
            package: package.into(),
            to: id(to),
            retain_source: false,
        },
        &snap.providers.keys().cloned().collect::<Vec<_>>(),
    )
    .expect("a plan")
}

#[test]
fn nothing_to_change_is_carried_only_on_set_primary() {
    let snap = htop_both_ways();
    let plan = plan(&snap, "htop", HB);
    for step in &plan.steps {
        assert_eq!(
            step.primary.is_some(),
            step.kind == StepKind::SetPrimary,
            "{step:?}"
        );
    }
    let set_primary = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::SetPrimary)
        .unwrap();
    assert!(set_primary.commands.is_empty() && set_primary.verification.is_empty());
    assert!(matches!(
        set_primary.primary,
        Some(PrimaryDecision::NothingToChange { .. })
    ));
    let resolution: Vec<&VerificationCheck> = plan
        .steps
        .iter()
        .filter(|s| s.kind == StepKind::VerifyResolution)
        .flat_map(|s| &s.verification)
        .collect();
    assert_eq!(
        resolution,
        [&VerificationCheck::ResolvesTo {
            executable: name("htop"),
            provider: id(HB),
            package: "htop".into(),
        }]
    );
    // `primaryAfter` is the destination only on `NOTHING_TO_CHANGE`.
    assert_eq!(plan.primary_after, id(HB));
    let toward_mp = self::plan(&snap, "htop", MP);
    assert_eq!(
        toward_mp.primary_after, toward_mp.source,
        "a refused decision keeps the source"
    );
}

#[test]
fn no_executables_plans_a_command_less_set_primary_and_no_verify_resolution() {
    // SR-2, SG2-N2: `KNOWN{}` is no primary to set, carried on a command-less
    // `SET_PRIMARY`, with an empty list everywhere.
    let mut snap = htop_both_ways();
    list(&mut snap, MP, "htop", &[], Placement::default());
    let plan = plan(&snap, "htop", HB);
    let set_primary = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::SetPrimary)
        .unwrap();
    assert_eq!(set_primary.primary, Some(PrimaryDecision::NoExecutables));
    assert!(set_primary.commands.is_empty());
    assert!(!plan
        .steps
        .iter()
        .any(|s| s.kind == StepKind::VerifyResolution));
    let verify = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::VerifyDestination)
        .unwrap();
    assert!(verify
        .verification
        .contains(&VerificationCheck::ExecutablesOwnedByDestination {
            executables: Vec::new()
        }));
    assert!(!verify
        .verification
        .iter()
        .any(|c| matches!(c, VerificationCheck::LivenessProbe { .. })));
}
