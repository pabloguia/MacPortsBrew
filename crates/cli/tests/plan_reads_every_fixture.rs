// SPDX-License-Identifier: Apache-2.0

//! Every read the real planner writes, from every captured recording, passes
//! the spawn-site read allowlist at construction (SE-050).
//!
//! The allowlist names, per subcommand, the exact long options a read may
//! carry. Its own unit tests tie it to the adapters' argument vectors and to
//! `provider-commands.toml`; this ties it to what the planner actually emits,
//! so a planner change that adds an option to a read fails here rather than
//! refusing itself at the spawn site on a user's machine.
//!
//! Lives in the command-line crate's tests because building a plan needs the
//! application crate and building a spec needs the adapters, and
//! `scripts/check-layering.sh` gives only this crate both edges. It is an
//! integration test, outside `crates/cli/src`, so it is not a read site in the
//! sense `the_command_line_crate_builds_no_command_spec_and_no_read_site_outside_the_loop`
//! scans for: it builds specs and runs none of them, through any runner.

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::process::{PlanReadRefusal, Recording, ReplayRunner};
use macportsbrew_adapters::registry::{self, detected_prefix, plan_read_spec, RegistryContext};
use macportsbrew_application::{build_plan, AnalysisOptions, PlanRequest};
use macportsbrew_domain::{CommandTarget, ProviderId, Snapshot};
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
        "macportsbrew-plan-reads-{stem}-{}-{:?}",
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

/// What one recording produced: the providers whose read was checked, and how
/// many read commands were.
#[derive(Default)]
struct Tally {
    providers: BTreeSet<ProviderId>,
    reads: usize,
}

fn check_recording(recording: &Path, policy: &PolicyData, options: &AnalysisOptions) -> Tally {
    check(&snapshot_of(recording), recording, policy, options)
}

/// Every read the planner writes from `snapshot`, checked against the sites
/// `recording` answers.
fn check(
    snapshot: &Snapshot,
    recording: &Path,
    policy: &PolicyData,
    options: &AnalysisOptions,
) -> Tally {
    // The prefixes this snapshot detected, as a replay-only override: the
    // runner is a recording, so nothing here can reach a live spawn, and no
    // spec built below is run at all.
    let overrides: Vec<(ProviderId, PathBuf)> = snapshot
        .providers
        .iter()
        .filter_map(|(id, held)| held.detection.prefix.clone().map(|p| (id.clone(), p)))
        .collect();
    let text = std::fs::read_to_string(recording).expect("the recording");
    let runner = ReplayRunner::new(Recording::from_json(&text).expect("the recording parses"));
    let ctx = RegistryContext {
        runner: &runner,
        home: None,
        platform_tags: Box::new(|_| Vec::new()),
        filesystem: FilesystemScope::Elsewhere,
        prefix_override: Some(overrides),
    };
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
                        &registry::known_provider_ids(),
                    ) else {
                        // A refusal to plan is the planner's answer, owned by
                        // `planner_refusals.rs`.
                        continue;
                    };
                    for step in &plan.steps {
                        let Some(provider) = &step.provider else {
                            continue;
                        };
                        for command in &step.commands {
                            if command.target != CommandTarget::Provider || command.placeholder {
                                continue;
                            }
                            let site = detected_prefix(&ctx, provider).unwrap_or_else(|| {
                                panic!(
                                    "{}: {name} -> {to}: step {} names {provider:?}, which the \
                                     snapshot did not detect, without a placeholder",
                                    recording.display(),
                                    step.index
                                )
                            });
                            let built = plan_read_spec(&site, command);
                            if step.kind.is_mutating() {
                                // The other direction: a mutation is no read.
                                assert!(
                                    matches!(built, Err(PlanReadRefusal::NotAPermittedRead { .. })),
                                    "{}: {name} -> {to}: step {} {:?} {:?}: {built:?}",
                                    recording.display(),
                                    step.index,
                                    step.kind,
                                    command.argv
                                );
                            } else {
                                assert!(
                                    built.is_ok(),
                                    "{}: {name} -> {to}: step {} {:?} {:?}: {built:?}",
                                    recording.display(),
                                    step.index,
                                    step.kind,
                                    command.argv
                                );
                                tally.providers.insert(provider.clone());
                                tally.reads += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    tally
}

#[test]
fn every_read_the_planner_writes_from_every_fixture_passes_the_read_allowlist() {
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

    let mut tallies: Vec<Tally> = std::thread::scope(|scope| {
        let handles: Vec<_> = recordings
            .iter()
            .map(|r| scope.spawn(|| check_recording(r, &policy, &options)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a recording's check panicked"))
            .collect()
    });
    // Part D: a replayed source row that declares no service is refused before
    // any step, so the plans with reads come from the live capture, checked
    // against the recording of the same machine.
    let live: Snapshot = serde_json::from_str(
        &std::fs::read_to_string(
            root().join("fixtures/snapshots/both-providers-intel-macos26-live.json"),
        )
        .expect("the live capture"),
    )
    .expect("the live capture parses");
    tallies.push(check(
        &live,
        &root().join("fixtures/recordings/both-providers-intel-macos26.json"),
        &policy,
        &options,
    ));
    let reads: usize = tallies.iter().map(|t| t.reads).sum();
    let providers: BTreeSet<ProviderId> = tallies
        .iter()
        .flat_map(|t| t.providers.iter().cloned())
        .collect();
    assert!(reads > 0, "the fixtures produced reads to check");
    // A Homebrew-to-MacPorts plan reads both: the destination's verify and
    // the source's inventory (INV-17 holds for the reads, whichever way the
    // fixtures let the planner build).
    assert_eq!(
        providers,
        registry::known_provider_ids().into_iter().collect(),
        "every provider's read was checked"
    );
}
