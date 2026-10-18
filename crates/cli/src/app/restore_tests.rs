// SPDX-License-Identifier: Apache-2.0

//! `restore`, in process (pass 08 slice 5, Part C). Every binary-level test runs
//! under `--replay`, which `restore` refuses, so this is where the live branch
//! is exercised: a context is built from a captured recording, its scope is set
//! to [`HistoryScope::ThisMachine`] here, and history and snapshots are read
//! from a scratch store. Nothing touches the real one, and nothing is executed.

use super::*;
use clap::Parser;
use macportsbrew_adapters::persistence::history_store::write_record;
use macportsbrew_adapters::persistence::run_lock::acquire;
use macportsbrew_adapters::persistence::snapshot_store::save_snapshot_durable;
use macportsbrew_adapters::persistence::state_dir::StateDir;
use macportsbrew_domain::{
    CommandEnd, Direction, InterruptedRun, Outcome, PackageIdentity, ProviderId, RecordedStep,
    RecordedVersions, RunEnding, RunStart, RunSubject, StepKind, StepResult, Version,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const RECORDING: &str = "both-providers-arm64-macos15.json";
const RUN_ID: &str = "20261004T120000Z-00000000000000a1";
const STARTED_AT: &str = "2026-10-04T12:00:00Z";
const SNAPSHOT: &str = "snapshot-20261004T120000Z.json";

fn recording() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings")
        .join(RECORDING)
}

fn cli(extra: &[&str]) -> Cli {
    let mut args = vec![
        "macportsbrew".to_string(),
        "--replay".into(),
        recording().to_string_lossy().into_owned(),
        "--today".into(),
        "2026-10-04".into(),
        "--json".into(),
        "restore".into(),
    ];
    args.extend(extra.iter().map(|s| (*s).to_string()));
    Cli::try_parse_from(args).expect("the arguments parse")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "macportsbrew-restore-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temporary directory");
        Self(dir)
    }

    fn state(&self) -> StateDir {
        let state = StateDir::resolve(None, Some(self.0.join("state").to_str().unwrap()))
            .expect("a state directory");
        state.ensure().expect("created");
        state
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A context from the recording, made to consult history as a live run does,
/// reading the scratch store.
fn live(scratch: &Scratch, cli: &Cli) -> Context {
    let mut ctx = Context::build(cli, &mut std::io::sink()).expect("the context builds");
    assert_eq!(ctx.history, HistoryScope::NotConsulted);
    ctx.history = HistoryScope::ThisMachine;
    ctx.state_override = Some(scratch.0.join("state").to_string_lossy().into_owned());
    ctx
}

fn options(ctx: &Context) -> AnalysisOptions {
    AnalysisOptions {
        today: ctx.today.clone(),
        advanced: false,
        only: BTreeSet::new(),
    }
}

/// The machine as it was before a run that moved `htop` from MacPorts to
/// Homebrew and kept its source: MacPorts alone held it. (The direction is
/// this way because a replay cannot answer whether a MacPorts package defines a
/// service, which the planner requires of the provider a plan removes from.)
fn before_the_run(ctx: &Context) -> macportsbrew_domain::Snapshot {
    let mut snapshot = ctx.snapshot.clone();
    let destination = ProviderId::new("homebrew");
    let holder = snapshot.providers.get_mut(&destination).unwrap();
    holder
        .inventory
        .installed
        .retain(|p| p.identity.name != "htop");
    snapshot.provenance.captured_at = STARTED_AT.into();
    snapshot
}

/// A record of a run that added `htop` under Homebrew and kept the MacPorts
/// copy, with the add ending as given.
fn record(add: StepResult, id: &str) -> RunRecord {
    let step = |index, kind, result| RecordedStep::new(index, kind, None, result, false);
    RunRecord::new(
        RunStart {
            id: id.into(),
            started_at: STARTED_AT.into(),
            tool_version: "0.1.0".into(),
        },
        RunSubject {
            plan_schema_version: 1,
            policy_version: "test".into(),
            package: PackageIdentity::new(ProviderId::new("macports"), "htop"),
            direction: Direction {
                source: ProviderId::new("macports"),
                destination: ProviderId::new("homebrew"),
            },
            destination_package: Some("htop".into()),
            versions: RecordedVersions {
                source: Some(Version::new("3.4.1")),
                destination: None,
            },
            action: Outcome::MigrateTo {
                provider: ProviderId::new("homebrew"),
            },
            overrode: None,
            retain_source: true,
            primary_after: ProviderId::new("macports"),
        },
        vec![
            step(0, StepKind::SnapshotBefore, StepResult::SnapshotTaken),
            step(1, StepKind::AddDestination, add),
            step(
                2,
                StepKind::RecordHistory,
                StepResult::RecordPendingReadBack,
            ),
        ],
        RunEnding::Completed,
        Some(SNAPSHOT.into()),
    )
}

fn performed() -> StepResult {
    StepResult::Performed {
        ends: vec![CommandEnd::Exited { code: 0 }],
    }
}

/// Seeds the store with a run's record and the snapshot it took.
fn seed(scratch: &Scratch, ctx: &Context, add: StepResult) {
    let state = scratch.state();
    let euid = macportsbrew_adapters::system::machine::effective_uid().unwrap();
    save_snapshot_durable(&state.snapshots(), &before_the_run(ctx)).expect("the snapshot");
    write_record(&state, &record(add, RUN_ID), euid).expect("the record");
}

fn mark_interrupted(scratch: &Scratch) {
    let state = scratch.state();
    let euid = macportsbrew_adapters::system::machine::effective_uid().unwrap();
    let lock = acquire(&state, euid).expect("the run lock");
    let marker = InterruptedRun::new(RUN_ID)
        .and_then(|m| m.with_snapshot(SNAPSHOT))
        .expect("a marker");
    macportsbrew_adapters::persistence::run_marker::write(&lock, &state, &marker, euid)
        .expect("the marker");
}

fn run_restore(
    ctx: &Context,
    cli: &Cli,
    plan_only: bool,
    id: &str,
) -> (Result<i32, DomainError>, String) {
    let mut out = Vec::new();
    let code = restore(ctx, cli, plan_only, id, &options(ctx), &mut out);
    (code, String::from_utf8(out).unwrap())
}

/// Every path in the store, so "created nothing" is a comparison.
fn tree(scratch: &Scratch) -> Vec<String> {
    fn walk(dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            out.push(path.display().to_string());
            if path.is_dir() {
                walk(&path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&scratch.0, &mut out);
    out.sort();
    out
}

#[test]
fn a_confirmed_run_is_planned_and_the_plan_is_an_ordinary_one() {
    let scratch = Scratch::new("plan");
    let cli = cli(&["--plan", "--run", RUN_ID]);
    let ctx = live(&scratch, &cli);
    seed(&scratch, &ctx, performed());
    let (code, out) = run_restore(&ctx, &cli, true, RUN_ID);
    assert_eq!(code.expect("planned"), 0, "{out}");
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["schemaVersion"], 1);
    assert_eq!(doc["mode"], "RENDER");
    assert_eq!(doc["restoreOf"]["runId"], RUN_ID);
    assert_eq!(doc["restoreOf"]["snapshotFile"], SNAPSHOT);
    assert_eq!(doc["restoreOf"]["standing"], "CONFIRMED");
    assert_eq!(doc["restoreOf"]["restorable"], "FROM_SNAPSHOT");
    assert_eq!(doc["notes"][0]["note"], "NOT_VERSION_EXACT");
    // The plan is `plan.v1` exactly: put back where it came from, taken from
    // where the run put it, the destination removed only after it is verified.
    let plan = &doc["plan"];
    assert_eq!(plan["destination"], "macports");
    assert_eq!(plan["source"], "homebrew");
    let kinds: Vec<&str> = plan["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    let at = |kind| kinds.iter().position(|k| *k == kind).unwrap();
    assert!(at("VERIFY_DESTINATION") < at("REMOVE_SOURCE"), "{kinds:?}");
    assert!(doc.get("execution").is_none());
}

#[test]
fn invariant_02_every_restore_plan_is_refused_for_execution_in_this_build() {
    let scratch = Scratch::new("gate");
    let cli = cli(&["--run", RUN_ID]);
    let ctx = live(&scratch, &cli);
    seed(&scratch, &ctx, performed());
    let before = tree(&scratch);
    let (code, out) = run_restore(&ctx, &cli, false, RUN_ID);
    assert_eq!(code.expect("refused, not failed"), 4, "{out}");
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["mode"], "EXECUTE");
    assert!(!doc["execution"]["gateRefusals"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(doc["execution"]["outcome"]["outcome"], "REFUSED");
    // No lock, no marker, no record, no snapshot: nothing was taken or written.
    assert_eq!(tree(&scratch), before);
}

#[test]
fn a_machine_already_as_the_run_found_it_has_nothing_to_restore() {
    let scratch = Scratch::new("already");
    let cli = cli(&["--plan", "--run", RUN_ID]);
    let mut ctx = live(&scratch, &cli);
    seed(&scratch, &ctx, performed());
    // The live machine is the earlier one again.
    ctx.snapshot = before_the_run(&ctx);
    let (code, out) = run_restore(&ctx, &cli, true, RUN_ID);
    assert_eq!(
        code.expect("nothing to restore is not a failure"),
        0,
        "{out}"
    );
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["nothingToRestore"], "ALREADY_RESTORED");
    assert!(doc.get("plan").is_none());
}

#[test]
fn a_run_that_changed_nothing_has_nothing_to_restore() {
    let scratch = Scratch::new("nothing");
    let cli = cli(&["--plan", "--run", RUN_ID]);
    let ctx = live(&scratch, &cli);
    let euid = macportsbrew_adapters::system::machine::effective_uid().unwrap();
    let step = |index, kind, result| RecordedStep::new(index, kind, None, result, false);
    let refused = RunRecord::new(
        RunStart {
            id: RUN_ID.into(),
            started_at: STARTED_AT.into(),
            tool_version: "0.1.0".into(),
        },
        record(performed(), RUN_ID).subject().clone(),
        vec![step(
            1,
            StepKind::AddDestination,
            StepResult::NotDispatchedMutating,
        )],
        RunEnding::Refused { refusals: 1 },
        None,
    );
    write_record(&scratch.state(), &refused, euid).unwrap();
    let (code, out) = run_restore(&ctx, &cli, true, RUN_ID);
    assert_eq!(code.unwrap(), 0, "{out}");
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["nothingToRestore"], "RUN_CHANGED_NOTHING");
}

#[test]
fn a_record_the_marker_still_names_is_planned_for_review_and_never_offered_for_execution() {
    let scratch = Scratch::new("unconfirmed");
    let plan_cli = cli(&["--plan", "--run", RUN_ID]);
    let ctx = live(&scratch, &plan_cli);
    seed(&scratch, &ctx, performed());
    mark_interrupted(&scratch);
    let (code, out) = run_restore(&ctx, &plan_cli, true, RUN_ID);
    assert_eq!(code.unwrap(), 0, "{out}");
    let doc: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["restoreOf"]["standing"], "UNCONFIRMED");
    assert_eq!(doc["restoreOf"]["restorable"], "NOT");
    assert_eq!(doc["restoreOf"]["reason"], "RECORD_NOT_CONFIRMED");
    let notes: Vec<&str> = doc["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["note"].as_str().unwrap())
        .collect();
    assert!(notes.contains(&"RECORD_NOT_CONFIRMED"), "{notes:?}");
    assert!(doc.get("plan").is_some());
    // Executing it is refused for that reason, before the gate is asked.
    let run_cli = cli(&["--run", RUN_ID]);
    let (code, _) = run_restore(&ctx, &run_cli, false, RUN_ID);
    let refused = code.expect_err("an unconfirmed record is not offered for execution");
    assert_eq!(refused.id, ErrorId::SafetyBlocked);
    assert!(
        refused.detail.contains("not confirmed"),
        "{}",
        refused.detail
    );
}

#[test]
fn a_marker_that_cannot_be_read_gives_no_plan() {
    let scratch = Scratch::new("marker-unreadable");
    let cli = cli(&["--plan", "--run", RUN_ID]);
    let ctx = live(&scratch, &cli);
    seed(&scratch, &ctx, performed());
    std::fs::write(
        scratch.0.join("state/interrupted-run.json"),
        "{ not a marker",
    )
    .unwrap();
    let (code, out) = run_restore(&ctx, &cli, true, RUN_ID);
    assert_eq!(code.expect_err("no plan").id, ErrorId::HistoryUnusable);
    assert!(out.is_empty());
}

#[test]
fn a_snapshot_that_is_gone_or_replaced_is_exit_eight_and_names_the_run() {
    let scratch = Scratch::new("snapshot-gone");
    let cli = cli(&["--plan", "--run", RUN_ID]);
    let ctx = live(&scratch, &cli);
    seed(&scratch, &ctx, performed());
    let file = scratch.state().snapshots().join(SNAPSHOT);
    // Replaced by a symbolic reference to a real snapshot: not what was taken.
    let target = scratch.0.join("elsewhere.json");
    std::fs::rename(&file, &target).unwrap();
    std::os::unix::fs::symlink(&target, &file).unwrap();
    let (code, out) = run_restore(&ctx, &cli, true, RUN_ID);
    let error = code.expect_err("refused");
    assert_eq!(error.id, ErrorId::SnapshotUnusable);
    assert_eq!(error.id.exit_code(), 8);
    assert!(error.detail.contains(RUN_ID), "{}", error.detail);
    assert!(out.is_empty());
    // Gone.
    std::fs::remove_file(&file).unwrap();
    let (code, _) = run_restore(&ctx, &cli, true, RUN_ID);
    assert_eq!(code.expect_err("refused").id, ErrorId::SnapshotUnusable);
}

#[test]
fn a_run_that_is_not_recorded_is_not_found() {
    let scratch = Scratch::new("not-found");
    let cli = cli(&["--plan", "--run", RUN_ID]);
    let ctx = live(&scratch, &cli);
    let (code, _) = run_restore(&ctx, &cli, true, RUN_ID);
    assert_eq!(code.expect_err("refused").id, ErrorId::RunNotFound);
}

#[test]
fn invariant_15_restore_refuses_a_recording_and_a_snapshot_file_before_a_context_exists() {
    for flag in ["--replay", "--from-snapshot"] {
        let path = recording();
        let args = [
            "macportsbrew",
            flag,
            path.to_str().unwrap(),
            "restore",
            "--plan",
            "--run",
            RUN_ID,
        ];
        let cli = Cli::try_parse_from(args).expect("parses");
        let refused = refuse_before_context(&cli, Some(RUN_ID), None).expect_err("refused");
        assert_eq!(refused.id, ErrorId::SafetyBlocked, "{flag}");
    }
    // The context's own scope is checked again for any other caller.
    let scratch = Scratch::new("scope");
    let cli = cli(&["--plan", "--run", RUN_ID]);
    let ctx = Context::build(&cli, &mut std::io::sink()).unwrap();
    let (code, _) = run_restore(&ctx, &cli, true, RUN_ID);
    assert_eq!(code.expect_err("refused").id, ErrorId::SafetyBlocked);
    drop(scratch);
}

#[test]
fn the_arguments_are_checked_before_anything_is_read() {
    let live_cli = Cli::try_parse_from(["macportsbrew", "restore", "--plan", "--run", RUN_ID])
        .expect("parses");
    assert!(refuse_before_context(&live_cli, Some(RUN_ID), None).is_ok());
    assert_eq!(
        refuse_before_context(&live_cli, None, None).unwrap_err().id,
        ErrorId::Usage
    );
    for bad in ["../x", "", "run-1", "20261004T120000Z-ZZZZZZZZZZZZZZZZ"] {
        assert_eq!(
            refuse_before_context(&live_cli, Some(bad), None)
                .unwrap_err()
                .id,
            ErrorId::Usage,
            "{bad:?}"
        );
    }
    let file = PathBuf::from("snapshot.json");
    let refused = refuse_before_context(&live_cli, None, Some(&file)).unwrap_err();
    assert_eq!(refused.id, ErrorId::NotImplementedInThisPass);
    assert_eq!(refused.id.exit_code(), 1);
    // `--run` and `--snapshot` together are a usage error from the parser.
    assert!(Cli::try_parse_from([
        "macportsbrew",
        "restore",
        "--plan",
        "--run",
        RUN_ID,
        "--snapshot",
        "f.json"
    ])
    .is_err());
}

#[test]
fn the_human_text_leads_with_the_run_what_is_known_and_what_the_plan_cannot_say() {
    let scratch = Scratch::new("human");
    let mut args = vec![
        "macportsbrew".to_string(),
        "--replay".into(),
        recording().to_string_lossy().into_owned(),
        "--today".into(),
        "2026-10-04".into(),
        "restore".into(),
        "--plan".into(),
        "--run".into(),
        RUN_ID.into(),
    ];
    let human = Cli::try_parse_from(args.drain(..)).unwrap();
    let ctx = live(&scratch, &human);
    seed(&scratch, &ctx, performed());
    let (code, out) = run_restore(&ctx, &human, true, RUN_ID);
    assert_eq!(code.unwrap(), 0, "{out}");
    for expected in [
        "Restore plan for run",
        RUN_ID,
        "confirmed (no interrupted-run marker names it)",
        "a plan can be built from the snapshot",
        "not a version-exact one",
        "3.4.1",
        "Shown only; nothing was run",
    ] {
        assert!(out.contains(expected), "{expected:?} missing from:\n{out}");
    }
}
