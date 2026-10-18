// SPDX-License-Identifier: Apache-2.0

//! The live branch of `migrate --retain-source` (pass 08 slice 1, H3).
//! Every binary-level test runs under `--replay`, which never consults
//! history, so this is where reading history, recognising a re-run and
//! rendering the answer are exercised: a context is built from a captured
//! recording, its scope is set to [`HistoryScope::ThisMachine`] here, and
//! history is read from a scratch store. Nothing touches the real one.

use super::*;
use clap::Parser;
use macportsbrew_adapters::persistence::history_store::write_record;
use macportsbrew_domain::{
    Direction, ExecutableOwner, Outcome, RecordedStep, RecordedVersions, Resolution, RunEnding,
    RunRecord, RunStart, RunSubject, StepKind, StepResult, Version,
};
use std::cell::Cell;
use std::collections::BTreeSet;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const RECORDING: &str = "both-providers-arm64-macos15.json";
const RUN_STAMP: &str = "20261004T130000Z";
const RECORD_ID: &str = "20261004T120000Z-00000000000000a1";

fn recording() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings")
        .join(RECORDING)
}

fn cli(package: &str, to: &str, json: bool) -> Cli {
    let recording = recording();
    let mut args = vec![
        "macportsbrew".to_string(),
        "--replay".into(),
        recording.to_string_lossy().into_owned(),
        "--today".into(),
        "2026-10-04".into(),
    ];
    if json {
        args.push("--json".into());
    }
    args.extend(["migrate", package, "--to", to, "--retain-source"].map(String::from));
    Cli::try_parse_from(args).expect("the arguments parse")
}

/// A context from the recording, made to consult history as a live run
/// does.
fn live(cli: &Cli) -> Context {
    let mut ctx = Context::build(cli, &mut std::io::sink()).expect("the context builds");
    assert_eq!(
        ctx.history,
        HistoryScope::NotConsulted,
        "a replay never consults"
    );
    ctx.history = HistoryScope::ThisMachine;
    ctx.run_stamp = RUN_STAMP.into();
    ctx
}

fn request(package: &str, to: &str) -> PlanRequest {
    PlanRequest {
        package: package.into(),
        to: ProviderId::new(to),
        retain_source: true,
    }
}

fn options(ctx: &Context) -> AnalysisOptions {
    AnalysisOptions {
        today: ctx.today.clone(),
        advanced: false,
        only: BTreeSet::new(),
    }
}

/// The plan this context builds for the request, without history.
fn plan_of(ctx: &Context, request: &PlanRequest) -> MigrationPlan {
    match migration_start(
        &ctx.snapshot,
        &ctx.policy,
        &options(ctx),
        request,
        &macportsbrew_adapters::registry::known_provider_ids(),
        HistoryConsultation::NotConsulted,
    )
    .expect("a start")
    {
        MigrationStart::Run(run, _) => run.plan().clone(),
        other => panic!("expected a plan, got {other:?}"),
    }
}

/// A record of a run of `plan` that ended `ending`.
fn record_of(plan: &MigrationPlan, ending: RunEnding) -> RunRecord {
    let steps = plan
        .steps
        .iter()
        .map(|s| {
            // The two brackets state their own results (history schema 2).
            let result = match s.kind {
                StepKind::SnapshotBefore => StepResult::SnapshotTaken,
                StepKind::RecordHistory => StepResult::RecordPendingReadBack,
                _ => StepResult::Verified {
                    evidence: Vec::new(),
                },
            };
            RecordedStep::new(s.index, s.kind, s.provider.clone(), result, false)
        })
        .collect();
    RunRecord::new(
        RunStart {
            id: RECORD_ID.into(),
            started_at: "2026-10-04T12:00:00Z".into(),
            tool_version: "0.1.0".into(),
        },
        RunSubject {
            plan_schema_version: plan.schema_version,
            policy_version: plan.policy_version.clone(),
            package: plan.package.clone(),
            direction: Direction {
                source: plan.source.clone(),
                destination: plan.destination.clone(),
            },
            destination_package: plan.destination_package.clone(),
            versions: RecordedVersions {
                source: Some(Version::new("1.0")),
                destination: None,
            },
            action: Outcome::MigrateTo {
                provider: plan.destination.clone(),
            },
            overrode: None,
            retain_source: true,
            primary_after: plan.primary_after.clone(),
        },
        steps,
        ending,
        Some("snapshot-20261004T120000Z.json".to_string()),
    )
}

/// Makes the context's capture show `plan`'s executable resolving to the
/// destination: what a live capture after the run would read. A recording
/// has no `PATH`, so it carries no resolution of its own.
fn resolve_to_destination(ctx: &mut Context, plan: &MigrationPlan) {
    let name = plan
        .destination_package
        .clone()
        .unwrap_or_else(|| plan.package.name.clone());
    let destination = ctx
        .snapshot
        .providers
        .get_mut(&plan.destination)
        .expect("the destination is in the capture");
    destination.resolutions.insert(
        name.clone(),
        Resolution {
            executable: name,
            path: None,
            canonical_path: None,
            owner: ExecutableOwner::Provider {
                provider: plan.destination.clone(),
                package: None,
            },
            authoritative: true,
        },
    );
}

struct Store {
    base: PathBuf,
    state: StateDir,
}

impl Store {
    fn new(tag: &str) -> Self {
        let base =
            std::env::temp_dir().join(format!("mpb-migrate-history-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&base).unwrap();
        let state = StateDir::resolve(None, Some(base.join("state").to_str().unwrap())).unwrap();
        Self { base, state }
    }

    fn uid(&self) -> u32 {
        std::fs::metadata(&self.base).unwrap().uid()
    }

    fn write(&self, record: &RunRecord) {
        write_record(&self.state, record, self.uid()).unwrap();
    }

    fn entries(&self) -> usize {
        std::fs::read_dir(self.state.history()).map_or(0, Iterator::count)
    }

    fn read(&self, key: &MigrationKey) -> Result<HistoryView, DomainError> {
        read_view(&self.state, key, self.uid())
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.base).ok();
    }
}

fn run(
    ctx: &Context,
    cli: &Cli,
    request: &PlanRequest,
    store: &Store,
) -> (Result<i32, DomainError>, String) {
    let mut out = Vec::new();
    let code = migrate_with(ctx, cli, request, &options(ctx), &mut out, &|key| {
        store.read(key)
    });
    (code, String::from_utf8(out).unwrap())
}

#[test]
fn a_retained_re_run_history_and_the_capture_show_done_is_reported_done() {
    for json in [true, false] {
        let cli = cli("htop", "macports", json);
        let mut ctx = live(&cli);
        let request = request("htop", "macports");
        let plan = plan_of(&ctx, &request);
        resolve_to_destination(&mut ctx, &plan);
        let store = Store::new(&format!("done-{json}"));
        store.write(&record_of(&plan, RunEnding::Completed));

        let (code, out) = run(&ctx, &cli, &request, &store);
        assert_eq!(code.unwrap(), 0);
        if json {
            let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
            assert_eq!(doc["retainedRunDone"]["runId"], RECORD_ID);
            assert_eq!(doc["retainedRunDone"]["provider"], "macports");
            assert!(doc.get("plan").is_none(), "nothing is planned");
        } else {
            assert!(out.contains(RECORD_ID), "{out}");
            assert!(out.contains("Already done"), "{out}");
        }
        // A recognised re-run writes nothing.
        assert_eq!(store.entries(), 1);
    }
}

#[test]
fn a_retained_re_run_that_cannot_be_confirmed_is_planned_again_and_says_why() {
    // Without the resolution: what was read does not show the command
    // running from the run's primary.
    let cli = cli("htop", "macports", true);
    let ctx = live(&cli);
    let request = request("htop", "macports");
    let plan = plan_of(&ctx, &request);
    let store = Store::new("unconfirmed");
    store.write(&record_of(&plan, RunEnding::Completed));

    let (code, out) = run(&ctx, &cli, &request, &store);
    assert_eq!(code.unwrap(), 0);
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        doc["retainedRunNotRecognised"]["reason"],
        "PRIMARY_NOT_CONFIRMED"
    );
    assert_eq!(doc["retainedRunNotRecognised"]["runId"], RECORD_ID);
    assert!(doc.get("plan").is_some(), "the migration is planned again");

    let human = super::super::Cli { json: false, ..cli };
    let (_, text) = run(&ctx, &human, &request, &store);
    assert!(text.contains("does not show the command running"), "{text}");
}

#[test]
fn an_unusable_history_stops_a_retained_re_run() {
    let cli = cli("htop", "macports", true);
    let ctx = live(&cli);
    let request = request("htop", "macports");
    let store = Store::new("unusable");
    store.state.ensure().unwrap();
    std::fs::write(store.state.history().join("notes.txt"), b"").unwrap();
    let (code, out) = run(&ctx, &cli, &request, &store);
    assert_eq!(code.unwrap_err().id, ErrorId::HistoryUnusable);
    assert!(out.is_empty(), "nothing is printed before the refusal");
}

#[test]
fn history_is_read_only_by_a_live_retained_run() {
    let called = Cell::new(false);
    let reader = |_: &MigrationKey| -> Result<HistoryView, DomainError> {
        called.set(true);
        Ok(HistoryView::empty())
    };
    let options_for = |ctx: &Context| options(ctx);

    // A replay: the scope the context was built with.
    let cli_r = cli("htop", "macports", true);
    let replayed = Context::build(&cli_r, &mut std::io::sink()).unwrap();
    let mut out = Vec::new();
    migrate_with(
        &replayed,
        &cli_r,
        &request("htop", "macports"),
        &options_for(&replayed),
        &mut out,
        &reader,
    )
    .unwrap();
    assert!(!called.get(), "a replay read history");

    // A live run that does not retain the source.
    let ctx = live(&cli_r);
    let plain = PlanRequest {
        retain_source: false,
        ..request("htop", "macports")
    };
    migrate_with(
        &ctx,
        &cli_r,
        &plain,
        &options_for(&ctx),
        &mut Vec::new(),
        &reader,
    )
    .unwrap();
    assert!(!called.get(), "a run without --retain-source read history");

    // A live retained run does.
    migrate_with(
        &ctx,
        &cli_r,
        &request("htop", "macports"),
        &options_for(&ctx),
        &mut Vec::new(),
        &reader,
    )
    .unwrap();
    assert!(called.get(), "a live retained run did not read history");
}

#[test]
fn invariant_19_recognition_never_replaces_a_manual_review_plan() {
    let cli = cli("ncurses", "macports", true);
    let mut ctx = live(&cli);
    let request = request("ncurses", "macports");
    let plan = plan_of(&ctx, &request);
    assert!(plan.requires_manual_review());
    resolve_to_destination(&mut ctx, &plan);
    let store = Store::new("manual");
    store.write(&record_of(&plan, RunEnding::Completed));
    let (code, out) = run(&ctx, &cli, &request, &store);
    assert_eq!(code.unwrap(), ErrorId::SafetyBlocked.exit_code());
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(doc.get("retainedRunDone").is_none());
    assert!(doc.get("plan").is_some());
}

#[test]
fn a_manual_review_plan_never_reads_history() {
    // The stop keeps its precedence over anything history could say: an
    // unusable store does not turn the exit-4 plan into an exit 8.
    let cli = cli("ncurses", "macports", true);
    let ctx = live(&cli);
    let request = request("ncurses", "macports");
    let store = Store::new("manual-poisoned");
    store.state.ensure().unwrap();
    std::fs::write(store.state.history().join("notes.txt"), b"").unwrap();
    let (code, out) = run(&ctx, &cli, &request, &store);
    assert_eq!(code.unwrap(), ErrorId::SafetyBlocked.exit_code());
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(doc.get("plan").is_some());
}

// --- Pass 08 slice 5: what a wired run writes is recognised, in both directions ---
//
// `record_of` has the shape a wired run leaves (`SNAPSHOT_TAKEN`, the pending
// `RECORD_HISTORY`, the snapshot named; `crates/cli/tests/executor_wiring.rs`
// reads the record a real wired run writes and checks that shape), written to a
// scratch store and read by `migrate` as a live run reads it.

const MIRROR: &str = "both-providers-intel-macos26-live.json";

fn cli_from_snapshot(package: &str, to: &str, json: bool) -> Cli {
    let snapshot = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/snapshots")
        .join(MIRROR);
    let mut args = vec![
        "macportsbrew".to_string(),
        "--from-snapshot".into(),
        snapshot.to_string_lossy().into_owned(),
        "--today".into(),
        "2026-10-04".into(),
    ];
    if json {
        args.push("--json".into());
    }
    args.extend(["migrate", package, "--to", to, "--retain-source"].map(String::from));
    Cli::try_parse_from(args).expect("the arguments parse")
}

/// Makes the capture show the destination holding the package, as it does once
/// a run has installed it: the source row, moved to the destination.
fn hold_at_destination(ctx: &mut Context, plan: &MigrationPlan) {
    let name = plan.package.name.clone();
    let mut row = ctx.snapshot.providers[&plan.source]
        .inventory
        .installed
        .iter()
        .find(|r| r.identity.name == name)
        .expect("the source holds the package")
        .clone();
    row.identity = macportsbrew_domain::PackageIdentity::new(plan.destination.clone(), name);
    ctx.snapshot
        .providers
        .get_mut(&plan.destination)
        .expect("the destination is in the capture")
        .inventory
        .installed
        .push(row);
}

#[test]
fn invariant_17_a_wired_runs_record_is_recognised_from_the_command_line_in_both_directions() {
    for (direction, source, to, json) in [
        (
            "homebrew to macports",
            cli("htop", "macports", true),
            "macports",
            true,
        ),
        (
            "macports to homebrew",
            cli_from_snapshot("htop", "homebrew", true),
            "homebrew",
            true,
        ),
    ] {
        let mut ctx = live(&source);
        let request = request("htop", to);
        let plan = plan_of(&ctx, &request);
        if direction.starts_with("macports") {
            hold_at_destination(&mut ctx, &plan);
        }
        resolve_to_destination(&mut ctx, &plan);
        let store = Store::new(&format!("both-{to}"));
        store.write(&record_of(&plan, RunEnding::Completed));
        let (code, out) = run(&ctx, &source, &request, &store);
        assert_eq!(code.unwrap(), 0, "{direction}: {out}");
        let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(
            doc["retainedRunDone"]["runId"], RECORD_ID,
            "{direction}: {out}"
        );
        assert_eq!(doc["retainedRunDone"]["provider"], to, "{direction}");
        assert_eq!(
            store.entries(),
            1,
            "{direction}: a recognised re-run writes nothing"
        );
        let _ = json;
    }
}

#[test]
fn invariant_16_a_record_the_marker_names_is_planned_again_from_the_command_line() {
    use macportsbrew_adapters::persistence::run_lock::acquire;
    use macportsbrew_adapters::persistence::run_marker::write;
    use macportsbrew_domain::InterruptedRun;
    let cli = cli("htop", "macports", true);
    let mut ctx = live(&cli);
    let request = request("htop", "macports");
    let plan = plan_of(&ctx, &request);
    resolve_to_destination(&mut ctx, &plan);
    let store = Store::new("marker-names-it");
    store.write(&record_of(&plan, RunEnding::Completed));
    // The run's marker was never cleared: the record is published, not confirmed.
    let lock = acquire(&store.state, store.uid()).unwrap();
    write(
        &lock,
        &store.state,
        &InterruptedRun::new(RECORD_ID).unwrap(),
        store.uid(),
    )
    .unwrap();
    drop(lock);

    let (code, out) = run(&ctx, &cli, &request, &store);
    assert_eq!(code.unwrap(), 0, "{out}");
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(
        doc.get("retainedRunDone").is_none(),
        "never done on this record: {out}"
    );
    assert_eq!(
        doc["retainedRunNotRecognised"]["reason"], "RECORD_NOT_CONFIRMED",
        "{out}"
    );
    assert_eq!(doc["retainedRunNotRecognised"]["runId"], RECORD_ID);
    assert!(doc.get("plan").is_some(), "the migration is planned again");
}

#[test]
fn every_run_that_handed_a_mutating_step_over_prints_the_command_to_plan_the_way_back() {
    // 04, "Undoing a migration": the snapshot it took first and the exact
    // command for its own id, whatever the outcome.
    let c = cli("htop", "macports", false);
    let ctx = live(&c);
    let plan = plan_of(&ctx, &request("htop", "macports"));
    let report = |receipt: RunReceipt, outcome: RunOutcome| RunReport {
        outcome,
        records: Vec::new(),
        refusal: None,
        gate_refusals: Vec::new(),
        history_error: None,
        marker_left: None,
        record_written: true,
        receipt,
    };
    for outcome in [
        RunOutcome::Completed,
        RunOutcome::Halted {
            step: 2,
            reason: macportsbrew_application::HaltReason::UnexecutedMutation,
        },
    ] {
        let text = human::run_report(
            &ctx,
            &plan,
            &report(
                RunReceipt {
                    run_id: Some(RECORD_ID.into()),
                    snapshot_file: Some("snapshot-20261004T120000Z.json".into()),
                    record_read_back: None,
                },
                outcome,
            ),
        );
        assert!(
            text.contains(&format!("macportsbrew restore --run {RECORD_ID} --plan")),
            "{text}"
        );
        assert!(text.contains("snapshot-20261004T120000Z.json"), "{text}");
    }
    // A run that handed nothing over has no run to undo.
    let none = human::run_report(
        &ctx,
        &plan,
        &report(RunReceipt::default(), RunOutcome::Completed),
    );
    assert!(!none.contains("restore --run"), "{none}");
}
