// SPDX-License-Identifier: Apache-2.0

//! The execution gate over every captured recording and every committed live
//! capture, in every direction each can plan
//! (`crates/application/src/execution_gate.rs`).
//!
//! Lives here rather than beside the gate's own tests because replaying a
//! recording needs the adapters, and `scripts/check-layering.sh` forbids
//! `application` that edge even as a dev-dependency. The binary replays each
//! recording into a snapshot once; every plan is then built in process with
//! the real planner, so the claim covers every installed package of every
//! fixture toward every other provider the fixture knows — detected or not,
//! which is how placeholder plans are reached too.

use macportsbrew_application::{
    build_plan, execution_refusals, AnalysisOptions, ExecutionRefusal, PlanRequest,
};
use macportsbrew_domain::{ProviderId, Snapshot};
use macportsbrew_policy::PolicyData;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");
const TODAY: &str = "2026-09-02";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/cli has a grandparent")
        .to_path_buf()
}

/// Replays `recording` into a snapshot through the real binary, with the
/// state directory inside a scratch directory that is removed afterwards.
fn snapshot_of(recording: &Path) -> Snapshot {
    let stem = recording
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("a recording name");
    let scratch = std::env::temp_dir().join(format!(
        "macportsbrew-gate-{stem}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).expect("scratch directory");
    let out = scratch.join("snapshot.json");
    let run = Command::new(BIN)
        .args([
            "--replay",
            recording.to_str().expect("UTF-8 path"),
            "snapshot",
            "--out",
            out.to_str().expect("UTF-8 path"),
            "--today",
            TODAY,
        ])
        .current_dir(root())
        .env("MACPORTSBREW_STATE_DIR", scratch.join("state"))
        .env_remove("MACPORTSBREW_LANG")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .expect("spawning the binary");
    assert_eq!(
        run.status.code(),
        Some(0),
        "{stem}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let text = std::fs::read_to_string(&out).expect("the snapshot was written");
    let _ = std::fs::remove_dir_all(&scratch);
    serde_json::from_str(&text).expect("the snapshot reads back")
}

/// What one recording produced: the directions asked for, the directions
/// that yielded a plan, and how many plans were checked.
#[derive(Default)]
struct Tally {
    asked: BTreeSet<(ProviderId, ProviderId)>,
    planned: BTreeSet<(ProviderId, ProviderId)>,
    plans: usize,
    /// Plans that do not stop at `MANUAL_REVIEW`, so reach the
    /// `MUTATION_NOT_IN_THIS_BUILD` assertion below.
    unstopped: usize,
}

/// A committed live capture, read as `--from-snapshot` reads it: through the
/// checked load path, so a capture that fails `Snapshot::check` fails here.
fn captured(path: &Path) -> Snapshot {
    macportsbrew_adapters::persistence::load_snapshot(path).expect("the capture loads")
}

fn check_recording(recording: &Path, policy: &PolicyData, options: &AnalysisOptions) -> Tally {
    check_snapshot(recording, &snapshot_of(recording), policy, options)
}

fn check_snapshot(
    recording: &Path,
    snapshot: &Snapshot,
    policy: &PolicyData,
    options: &AnalysisOptions,
) -> Tally {
    let mut tally = Tally::default();
    for (source, held) in &snapshot.providers {
        let names: BTreeSet<&str> = held
            .inventory
            .installed
            .iter()
            .map(|p| p.identity.name.as_str())
            .collect();
        for to in snapshot.providers.keys().filter(|to| *to != source) {
            for name in &names {
                tally.asked.insert((source.clone(), to.clone()));
                for retain in [false, true] {
                    let Ok(plan) = build_plan(
                        snapshot,
                        policy,
                        options,
                        &PlanRequest {
                            package: (*name).to_string(),
                            to: to.clone(),
                            retain_source: retain,
                        },
                        &macportsbrew_adapters::registry::known_provider_ids(),
                    ) else {
                        // A refusal to plan is the planner's answer, and
                        // `planner_refusals.rs` owns it; this test is about
                        // the plans that were built.
                        continue;
                    };
                    let report = execution_refusals(&plan);
                    assert!(
                        !report.may_execute(),
                        "{}: {name} -> {to} may execute",
                        recording.display()
                    );
                    assert_eq!(report.exit_code(), 4);
                    if !plan.requires_manual_review() {
                        // Named by the first mutating step, command or not.
                        let first = plan
                            .steps
                            .iter()
                            .find(|s| s.kind.is_mutating())
                            .expect("a plan that does not stop has a mutating step")
                            .index;
                        assert!(report.refusals().iter().any(|r| matches!(
                            r,
                            ExecutionRefusal::MutationNotInThisBuild { step } if *step == first
                        )));
                        tally.unstopped += 1;
                    }
                    // What the planner wrote from the real table is never
                    // refused as a forged command, as another schema version's
                    // plan, as empty, or as mutations left unbracketed or
                    // bracketed twice (obligation 11).
                    assert!(
                        !report.refusals().iter().any(|r| matches!(
                            r,
                            ExecutionRefusal::CommandOnStepThePlannerWritesNoneOn { .. }
                                | ExecutionRefusal::CommandIsNotThePlannersRead { .. }
                                | ExecutionRefusal::SchemaVersionNotThisBuilds { .. }
                                | ExecutionRefusal::NoSnapshotBeforeFirstMutation { .. }
                                | ExecutionRefusal::NoRecordHistoryAtEnd
                                | ExecutionRefusal::EmptyPlan
                                | ExecutionRefusal::MoreThanOneSnapshotBefore { .. }
                                | ExecutionRefusal::MoreThanOneRecordHistory { .. }
                        )),
                        "{}: {name} -> {to}: {:?}",
                        recording.display(),
                        report.refusals()
                    );
                    tally
                        .planned
                        .insert((plan.source.clone(), plan.destination.clone()));
                    tally.plans += 1;
                }
            }
        }
    }
    tally
}

#[test]
fn every_plan_this_planner_builds_from_every_fixture_is_refused_for_execution() {
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    let options = AnalysisOptions {
        today: TODAY.into(),
        advanced: false,
        only: BTreeSet::new(),
    };
    let mut recordings: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/recordings"))
        .expect("fixtures/recordings")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    recordings.sort();
    assert!(!recordings.is_empty());
    let mut captures: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/snapshots"))
        .expect("fixtures/snapshots")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    captures.sort();
    assert!(!captures.is_empty());

    // One thread per recording: the planner analyses the whole machine for
    // every package it is asked about, and the recordings hold hundreds.
    let tallies: Vec<Tally> = std::thread::scope(|scope| {
        let handles: Vec<_> = recordings
            .iter()
            .map(|r| scope.spawn(|| check_recording(r, &policy, &options)))
            .chain(
                captures
                    .iter()
                    .map(|c| scope.spawn(|| check_snapshot(c, &captured(c), &policy, &options))),
            )
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a recording's check panicked"))
            .collect()
    });
    let plans: usize = tallies.iter().map(|t| t.plans).sum();
    let asked: BTreeSet<_> = tallies
        .iter()
        .flat_map(|t| t.asked.iter().cloned())
        .collect();
    assert!(plans > 0, "the fixtures produced plans to check");
    // A replayed source row cannot answer the service question (DP-032,
    // INV-15), so from a recording only a row that declares a service is
    // planned, and its plan stops at `MANUAL_REVIEW`. The live captures are
    // what reach a plan that does not stop: without them the branch that
    // checks `MUTATION_NOT_IN_THIS_BUILD` would be asserted over nothing.
    let unstopped: usize = tallies.iter().map(|t| t.unstopped).sum();
    assert!(
        unstopped > 0,
        "a plan that does not stop at MANUAL_REVIEW reached the gate"
    );
    // Both directions are *asked* (INV-17), and both are *planned*: the live
    // captures answer the service question for each provider's rows, so each
    // direction yields plans.
    let planned: BTreeSet<_> = tallies
        .iter()
        .flat_map(|t| t.planned.iter().cloned())
        .collect();
    assert!(
        planned.contains(&(ProviderId::new("homebrew"), ProviderId::new("macports"))),
        "a Homebrew-to-MacPorts plan was built, not only asked for: {planned:?}"
    );
    assert!(
        planned.contains(&(ProviderId::new("macports"), ProviderId::new("homebrew"))),
        "a MacPorts-to-Homebrew plan was built, not only asked for: {planned:?}"
    );
    let reversed: BTreeSet<_> = asked.iter().map(|(a, b)| (b.clone(), a.clone())).collect();
    assert!(
        asked.intersection(&reversed).next().is_some(),
        "both directions are asked: {asked:?}"
    );
}
