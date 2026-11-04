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
        // A row that says the destination's own package wins while the
        // destination is not installed is inconsistent, and refused the same
        // way in both directions (spec-guardian review of F-3, R-1).
        assert_eq!(
            reason_for(&decide(&snap, &question(from, "htop", to, false)), "htop"),
            Some(PrimaryReason::PrimaryDestinationNotInstalled),
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
fn the_index_comparison_never_predicts_a_win_against_the_capture() {
    // `d > w` a shim, `d == w` the same directory (mutant 4: the comparison
    // negated). `d < w` with a winner that is not the destination's own, and
    // a miss while the destination says it is linked on `PATH`, are the
    // capture contradicting the placement: unpredictable, never a predicted
    // win (security review of F-3, L-2).
    let (mp, hb) = (id(MP), id(HB));
    for (w, expected) in [
        (30, Some(PrimaryReason::PrimaryUnpredictable)),
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
    let mut snap = htop_both_ways();
    snap.environment
        .resolutions
        .insert(name("htop"), ResolutionState::NotOnPath);
    assert_eq!(
        reason_for(&decide(&snap, &question(&mp, "htop", &hb, true)), "htop"),
        Some(PrimaryReason::PrimaryUnpredictable),
        "a miss against a placement on PATH"
    );
}

#[test]
fn a_destination_list_not_known_is_unpredictable_even_when_not_linked() {
    // Security review of F-3, L-1: `NOT_LINKED` says nothing about `e` when
    // the destination's list is not known, so it is not `PRIMARY_NEEDS_SHIM`.
    let mut snap = htop_both_ways();
    let meta = snap
        .providers
        .get_mut(&id(HB))
        .unwrap()
        .inventory
        .metadata
        .get_mut("htop")
        .unwrap();
    meta.set_executables(
        ExecutableList::NotRecorded,
        Placement::NotLinked {
            reason: "keg-only".into(),
        },
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
        Some(PrimaryReason::PrimaryUnpredictable)
    );
}

#[test]
fn a_destination_winner_without_a_recorded_index_is_unpredictable() {
    // Mutant 15b; security review of F-3, M-2: "already wins" with no index.
    let mut snap = htop_both_ways();
    found(
        &mut snap,
        "htop",
        "/usr/local/bin",
        owned(HB, Some("htop")),
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
    // Nothing stays with the source, as `history.v3` records it (fix round).
    assert_eq!(plan.primary_after, id(HB));
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
#[test]
fn invariant_12_setting_a_primary_never_reorders_path() {
    // "PATH is never reordered implicitly." The decision reads the one
    // captured `PATH` and writes nothing back: `SET_PRIMARY` carries no
    // command; a command's environment carries only the sanitised `PATH` its
    // provider's invocation already ran with (INV-10), never one the plan
    // built, and no argv names `PATH`; and the `PATH` the decision records is
    // the snapshot's,
    // entry for entry, in both directions — a predicted win and a refusal
    // that only a reorder (or a shim) would answer alike.
    let snap = htop_both_ways();
    let before = snap.environment.path.clone();
    for to in [HB, MP] {
        let plan = plan(&snap, "htop", to);
        for step in &plan.steps {
            if step.kind == StepKind::SetPrimary {
                assert!(step.commands.is_empty(), "toward {to}: {step:?}");
            }
            for command in &step.commands {
                let recorded: Vec<Option<&String>> = snap
                    .providers
                    .values()
                    .map(|p| p.invocation_environment.get("PATH"))
                    .collect();
                assert!(
                    recorded.contains(&command.environment.get("PATH")),
                    "toward {to}: {command:?}"
                );
                assert!(
                    !command.argv.iter().any(|a| a.contains("PATH")),
                    "toward {to}: {command:?}"
                );
            }
        }
        let decision = plan
            .steps
            .iter()
            .find_map(|s| s.primary.as_ref())
            .expect("one decision");
        let used = match decision {
            PrimaryDecision::NothingToChange { path_used, .. }
            | PrimaryDecision::Refused { path_used, .. } => path_used,
            other => panic!("toward {to}: {other:?}"),
        };
        assert_eq!(
            used, &before,
            "toward {to}: the order read is the order kept"
        );
    }
    // Toward MacPorts only a reorder or a shim would make its copy win; the
    // plan offers neither and says so.
    let toward_mp = plan(&snap, "htop", MP);
    assert!(toward_mp
        .steps
        .iter()
        .filter_map(|s| s.primary.as_ref())
        .any(|d| d.needing_a_shim().contains(&name("htop"))));
    assert_eq!(snap.environment.path, before);
}

#[test]
fn the_path_a_decision_used_is_home_redacted_in_plan_history_and_report() {
    // SR-12. Capture writes `PATH` home-redacted (`capture_redaction.rs`); the
    // decision keeps that redacted copy as `pathUsed`, the plan document — the
    // report `migrate` prints — carries nothing else of it, and the run record
    // carries no `PATH` at all, only whether the run-time one differed.
    const HOME: &str = "/Users/testuser";
    let mut snap = htop_both_ways();
    snap.environment.path.push("~/.local/bin".into());
    let run = macportsbrew_application::plan_run(
        &snap,
        &PolicyData::load_embedded().unwrap(),
        &AnalysisOptions {
            today: "2026-11-01".into(),
            advanced: false,
            only: BTreeSet::new(),
        },
        &PlanRequest {
            package: "htop".into(),
            to: id(HB),
            retain_source: true,
        },
        &snap.providers.keys().cloned().collect::<Vec<_>>(),
    )
    .expect("a plan");
    let plan = run.plan();
    let Some(PrimaryDecision::NothingToChange { path_used, .. }) =
        plan.steps.iter().find_map(|s| s.primary.as_ref())
    else {
        panic!("htop already wins toward Homebrew: {plan:?}");
    };
    assert_eq!(path_used, &snap.environment.path);
    assert_eq!(path_used.last().map(String::as_str), Some("~/.local/bin"));

    let document = serde_json::to_string(plan).expect("a plan serialises");
    assert!(
        document.contains("~/.local/bin"),
        "the redacted entry is kept"
    );
    assert!(!document.contains(HOME) && !document.contains("testuser"));

    let records: Vec<macportsbrew_application::StepRecord> = plan
        .steps
        .iter()
        .map(|s| macportsbrew_application::StepRecord {
            index: s.index,
            kind: s.kind,
            disposition: match s.kind {
                StepKind::VerifyResolution => macportsbrew_application::Disposition::Resolved {
                    executables: vec![name("htop")],
                    path_differs: true,
                },
                _ => macportsbrew_application::Disposition::NotPerformedInThisBuild,
            },
        })
        .collect();
    let record = macportsbrew_application::run_record(
        &run,
        &records,
        macportsbrew_domain::RunStart {
            id: "20261101T120000Z-0000000000000001".into(),
            started_at: "2026-11-01T12:00:00Z".into(),
            tool_version: "0.1.0".into(),
        },
        None,
    );
    let text = serde_json::to_string(&record).expect("a record serialises");
    for entry in &snap.environment.path {
        assert!(
            !text.contains(&format!("\"{entry}\"")),
            "the record copies the PATH entry {entry}: {text}"
        );
    }
    assert!(!text.contains(HOME) && !text.contains("testuser"));
    // Whether the run-time `PATH` differed is said in the run report
    // (`migrate.v2`'s `RESOLVED`), as a flag and never as the `PATH` itself.
    let at = plan
        .steps
        .iter()
        .position(|s| s.kind == StepKind::VerifyResolution)
        .expect("a resolution step");
    let said = serde_json::to_string(&records[at].disposition).expect("a disposition serialises");
    assert!(said.contains("\"pathDiffers\":true"), "{said}");
    for entry in &snap.environment.path {
        assert!(!said.contains(&format!("\"{entry}\"")), "{said}");
    }
}
#[test]
fn primary_after_is_the_source_when_resolution_did_not_hold() {
    // Mutant 28c: `history.v3`'s `primaryAfter` is what resolved. A completed
    // `SET_PRIMARY` (`NOTHING_TO_CHANGE`) followed by a halted
    // `VERIFY_RESOLUTION` leaves the source primary; a `RESOLVED` one names
    // the destination.
    let snap = htop_both_ways();
    let run = macportsbrew_application::plan_run(
        &snap,
        &PolicyData::load_embedded().unwrap(),
        &AnalysisOptions {
            today: "2026-11-01".into(),
            advanced: false,
            only: BTreeSet::new(),
        },
        &PlanRequest {
            package: "htop".into(),
            to: id(HB),
            retain_source: true,
        },
        &snap.providers.keys().cloned().collect::<Vec<_>>(),
    )
    .expect("a plan");
    let plan = run.plan();
    let records_with = |resolution: macportsbrew_application::Disposition| {
        plan.steps
            .iter()
            .map(|s| macportsbrew_application::StepRecord {
                index: s.index,
                kind: s.kind,
                disposition: match s.kind {
                    StepKind::SetPrimary => macportsbrew_application::Disposition::NothingToChange,
                    StepKind::VerifyResolution => resolution.clone(),
                    _ => macportsbrew_application::Disposition::NotPerformedInThisBuild,
                },
            })
            .collect::<Vec<_>>()
    };
    let start = || macportsbrew_domain::RunStart {
        id: "20261101T120000Z-0000000000000001".into(),
        started_at: "2026-11-01T12:00:00Z".into(),
        tool_version: "0.1.0".into(),
    };
    let halted = macportsbrew_application::Disposition::Halted {
        reason: macportsbrew_application::HaltReason::ResolutionMismatch {
            executable: name("htop"),
            observed: macportsbrew_application::ObservedWinner::AnotherProvider,
            path_differs: false,
        },
        evidence: Vec::new(),
        ends: Vec::new(),
    };
    let record = macportsbrew_application::run_record(&run, &records_with(halted), start(), None);
    assert_eq!(record.subject().primary_after, plan.source);
    let resolved = macportsbrew_application::Disposition::Resolved {
        executables: vec![name("htop")],
        path_differs: false,
    };
    let record = macportsbrew_application::run_record(&run, &records_with(resolved), start(), None);
    assert_eq!(record.subject().primary_after, plan.destination);
}
#[test]
fn invariant_17_no_executables_plans_the_same_toward_either_provider() {
    // INV-17, spec-guardian review of F-3 (fix round 2, F-5): the
    // `NO_EXECUTABLES` shape, and `primaryAfter` naming the destination, are
    // the same whichever provider the package moves toward.
    for (from, to) in [(MP, HB), (HB, MP)] {
        let mut snap = htop_both_ways();
        list(&mut snap, from, "htop", &[], Placement::default());
        let plan = plan(&snap, "htop", to);
        let set_primary = plan
            .steps
            .iter()
            .find(|s| s.kind == StepKind::SetPrimary)
            .unwrap();
        assert_eq!(
            set_primary.primary,
            Some(PrimaryDecision::NoExecutables),
            "toward {to}"
        );
        assert!(set_primary.commands.is_empty(), "toward {to}");
        assert!(
            !plan
                .steps
                .iter()
                .any(|s| s.kind == StepKind::VerifyResolution),
            "toward {to}"
        );
        assert_eq!(plan.primary_after, id(to), "toward {to}");
        assert_eq!(plan.destination, id(to));
    }
}
