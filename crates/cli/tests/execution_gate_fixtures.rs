// SPDX-License-Identifier: Apache-2.0

//! The execution gate over every captured recording, in every direction the
//! recording can plan (`crates/application/src/execution_gate.rs`).
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
}

fn check_recording(recording: &Path, policy: &PolicyData, options: &AnalysisOptions) -> Tally {
    let snapshot = snapshot_of(recording);
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
                        &snapshot,
                        policy,
                        options,
                        &PlanRequest {
                            package: (*name).to_string(),
                            to: to.clone(),
                            retain_source: retain,
                        },
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
                        assert!(report
                            .refusals()
                            .iter()
                            .any(|r| matches!(r, ExecutionRefusal::MutationNotInThisBuild { .. })));
                    }
                    // What the planner wrote from the real table is never
                    // refused as a forged command.
                    assert!(
                        !report.refusals().iter().any(|r| matches!(
                            r,
                            ExecutionRefusal::CommandOnStepThePlannerWritesNoneOn { .. }
                                | ExecutionRefusal::CommandIsNotThePlannersRead { .. }
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

    // One thread per recording: the planner analyses the whole machine for
    // every package it is asked about, and the recordings hold hundreds.
    let tallies: Vec<Tally> = std::thread::scope(|scope| {
        let handles: Vec<_> = recordings
            .iter()
            .map(|r| scope.spawn(|| check_recording(r, &policy, &options)))
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
    // Both directions are *asked* (INV-17). Not both are *planned*: no
    // recording captures whether a MacPorts port defines a service, so the
    // planner refuses every plan out of MacPorts from a fixture as
    // unverifiable (INV-19) — and a plan that was never built has nothing to
    // execute. `execution_gate.rs` in the application crate covers the
    // MacPorts-to-Homebrew direction with a synthesised snapshot.
    let planned: BTreeSet<_> = tallies
        .iter()
        .flat_map(|t| t.planned.iter().cloned())
        .collect();
    assert!(
        planned.contains(&(ProviderId::new("homebrew"), ProviderId::new("macports"))),
        "a Homebrew-to-MacPorts plan was built, not only asked for: {planned:?}"
    );
    let reversed: BTreeSet<_> = asked.iter().map(|(a, b)| (b.clone(), a.clone())).collect();
    assert!(
        asked.intersection(&reversed).next().is_some(),
        "both directions are asked: {asked:?}"
    );
}
