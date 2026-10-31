// SPDX-License-Identifier: Apache-2.0

//! `migrate`, end to end: the built binary over the captured recordings, plus
//! the composition it runs — `migration_start`, then the run loop — driven in
//! process where what must be observed is which invocations a runner was
//! asked for (`docs/schemas/migrate.v1.md`; SE-092, S6 addendum).
//!
//! Nothing here touches a live installation (INV-05): every run is
//! `--replay` or `--from-snapshot`, and the in-process runs use a
//! `ReplayRunner`. Verbs are read from `provider-commands.toml` through
//! `provider_commands::load`, never written here.

use macportsbrew::executor;
use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::persistence::load_snapshot;
use macportsbrew_adapters::process::{
    ExitStatus, Invocation, RecordedInvocation, Recording, ReplayRunner,
};
use macportsbrew_adapters::registry::{self, RegistryContext};
use macportsbrew_application::{
    migration_start, provider_commands, AnalysisOptions, HistoryConsultation, MigrationStart,
    PlanRequest, RunOutcome,
};
use macportsbrew_domain::{HomeRedaction, ProviderId};
use macportsbrew_policy::PolicyData;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");
const TODAY: &str = "2026-09-02";
const HB: &str = "homebrew";
const MP: &str = "macports";

/// Both providers, Apple Silicon: `htop` held by both, `nginx` by MacPorts
/// alone, `node` a formula whose mapping toward the ports is ambiguous
/// between `nodejs22` and `nodejs24`.
const BOTH: &str = "both-providers-arm64-macos15";
/// Both providers, Intel: `python@3.12 --to macports` adds the port, an
/// install the provider command table marks as needing elevation.
const BOTH_INTEL: &str = "both-providers-intel-macos26";
/// MacPorts alone; Homebrew was not detected.
const MACPORTS_ONLY: &str = "macports-only-intel-macos12";
/// A formula the recordings' JSON declares a service for. Under `--replay` a
/// source row answers the service question only from what it declares (DP-032,
/// part D): this one answers `YES` and plans, to `MANUAL_REVIEW`; one that
/// declares none answers `UNKNOWN` and is refused (`htop`).
const REPLAYED_SERVICE: &str = "emacs";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

fn fixture(name: &str) -> PathBuf {
    root().join(format!("fixtures/recordings/{name}.json"))
}

/// A temporary directory unique to one test, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "macportsbrew-migrate-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temporary directory");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn state_dir(&self) -> PathBuf {
        self.0.join("state")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_with_env(scratch: &Scratch, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(BIN);
    command
        .args(args)
        .current_dir(root())
        .env("MACPORTSBREW_STATE_DIR", scratch.state_dir())
        .env_remove("MACPORTSBREW_LANG")
        .env("LC_ALL", "C")
        .env("LANG", "C");
    for (k, v) in env {
        command.env(k, v);
    }
    command
        .output()
        .unwrap_or_else(|e| panic!("spawning {BIN}: {e}"))
}

fn run(scratch: &Scratch, args: &[&str]) -> Output {
    run_with_env(scratch, args, &[])
}

/// `migrate` over a recording, with `extra` flags before the subcommand.
fn migrate(scratch: &Scratch, recording: &Path, extra: &[&str], package: &str, to: &str) -> Output {
    let recording = recording.to_str().expect("UTF-8 path");
    let mut argv = vec!["--replay", recording];
    argv.extend_from_slice(extra);
    argv.extend_from_slice(&["migrate", package, "--to", to, "--today", TODAY]);
    run(scratch, &argv)
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn document(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", stdout(out)))
}

/// The snapshot a replay of `name` captures, written into `scratch`.
fn snapshot_file(scratch: &Scratch, name: &str) -> PathBuf {
    let path = scratch.path().join(format!("{name}.snapshot.json"));
    let out = run(
        scratch,
        &[
            "--replay",
            fixture(name).to_str().expect("UTF-8"),
            "snapshot",
            "--out",
            path.to_str().expect("UTF-8"),
            "--today",
            TODAY,
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    path
}

fn from_snapshot(
    scratch: &Scratch,
    file: &Path,
    extra: &[&str],
    package: &str,
    to: &str,
) -> Output {
    let file = file.to_str().expect("UTF-8");
    let mut argv = vec!["--from-snapshot", file];
    argv.extend_from_slice(extra);
    argv.extend_from_slice(&["migrate", package, "--to", to, "--today", TODAY]);
    run(scratch, &argv)
}

fn table() -> BTreeMap<String, provider_commands::ProviderCommands> {
    provider_commands::load().expect("the provider command table loads")
}

/// An executable's file name, as a recording keys it.
fn leaf(path: &str) -> String {
    Path::new(path)
        .file_name()
        .expect("an executable has a file name")
        .to_string_lossy()
        .into_owned()
}

/// Every mutating verb in the provider command table, as the `leaf argv...`
/// prefix a replay runner keys an invocation by: every operation that is not
/// a verification read.
fn mutating_prefixes() -> Vec<String> {
    let mut out = Vec::new();
    for entry in table().values() {
        for (operation, argv) in &entry.verbs {
            if !operation.starts_with("verify") {
                out.push(format!("{} {}", leaf(&entry.executable), argv.join(" ")));
            }
        }
    }
    assert!(
        out.len() >= 4,
        "the table carries both providers' add and remove: {out:?}"
    );
    out
}

#[test]
fn migrate_dry_run_renders_every_command_including_elevated_ones() {
    // `04-migration-and-safety.md`: `--dry-run` produces the plan and every
    // command that would run, and mutates nothing — including no `sudo`.
    // From the live capture: a replayed source row cannot answer the service
    // question, so a plan with commands is built from a capture (part D).
    let scratch = Scratch::new("dry-run-renders");
    let file = live_snapshot();
    let json = from_snapshot(&scratch, &file, &["--json", "--dry-run"], "python@3.12", MP);
    assert_eq!(json.status.code(), Some(0), "{}", stderr(&json));
    let doc = document(&json);
    assert_eq!(doc["schemaVersion"], 1);
    assert_eq!(doc["mode"], "RENDER");
    assert!(doc.get("execution").is_none(), "a render runs nothing");
    let human = from_snapshot(&scratch, &file, &["--dry-run"], "python@3.12", MP);
    assert_eq!(human.status.code(), Some(0), "{}", stderr(&human));
    let text = stdout(&human);
    let mut commands = 0;
    let mut elevated = 0;
    for step in doc["plan"]["steps"].as_array().expect("steps") {
        for command in step["commands"].as_array().into_iter().flatten() {
            // This tool's own command is not one this version has; the view
            // says what to do instead of showing a `$` line that would fail.
            if command["target"]["target"] == "TOOL" {
                assert!(
                    text.contains("Not a command in this version."),
                    "the tool's own step is explained:\n{text}"
                );
                assert!(!text.contains("$ macportsbrew "), "{text}");
                continue;
            }
            let argv: Vec<&str> = command["argv"]
                .as_array()
                .expect("argv")
                .iter()
                .map(|a| a.as_str().expect("string"))
                .collect();
            let line = format!(
                "$ {} {}",
                command["program"].as_str().expect("program"),
                argv.join(" ")
            );
            assert!(text.contains(&line), "{line:?} is not rendered:\n{text}");
            commands += 1;
            if command["elevation"]["elevation"] == "REQUIRED" {
                elevated += 1;
            }
        }
    }
    assert!(commands >= 4, "the plan carries its commands: {commands}");
    assert_eq!(elevated, 1, "the install of the port needs elevation");
    assert!(text.contains("elevation required"), "{text}");
    for output in [&json, &human] {
        for stream in [stdout(output), stderr(output)] {
            assert!(
                !stream.contains("sudo"),
                "a dry run never asks to elevate: {stream}"
            );
        }
    }
}

#[test]
fn migrate_yes_is_refused_for_execution_and_still_renders_the_plan() {
    // Execute mode needs a runner, so it is shown over a recording; and over a
    // recording only a source row that declares a service can be planned (part
    // D): its plan stops at `MANUAL_REVIEW`, which the gate refuses by name.
    // The mutating-step refusal (`MUTATION_NOT_IN_THIS_BUILD`) of a plan with
    // commands is rendered in process, `crates/cli/tests/executor.rs`.
    let scratch = Scratch::new("yes-refused");
    let file = fixture(BOTH);
    let human = migrate(&scratch, &file, &["--yes"], REPLAYED_SERVICE, MP);
    assert_eq!(human.status.code(), Some(4), "{}", stderr(&human));
    let text = stdout(&human);
    assert!(
        text.contains("Plan: move emacs from homebrew to macports"),
        "{text}"
    );
    assert!(
        text.contains("This version never changes your machine, so the plan was not run."),
        "{text}"
    );
    assert!(
        text.contains("To do this yourself, run the commands shown under each step"),
        "what to do by hand follows the reasons: {text}"
    );
    assert!(
        text.contains("Step 1 needs a human decision, and nothing outranks it."),
        "{text}"
    );
    // A row that declares none is refused before any plan or gate.
    let refused = migrate(&scratch, &file, &["--yes"], "htop", MP);
    assert_eq!(refused.status.code(), Some(4), "{}", stderr(&refused));
    assert!(refused.stdout.is_empty(), "{}", stdout(&refused));
    assert!(
        stderr(&refused).contains("whether homebrew package htop defines a background service"),
        "{}",
        stderr(&refused)
    );
    // No identifier reaches a person: neither a refusal tag nor a key.
    for raw in [
        "MUTATION_NOT_IN_THIS_BUILD",
        "REFUSED",
        "migrate.",
        "refusal.",
    ] {
        assert!(!text.contains(raw), "{raw} reached the user:\n{text}");
    }
    let json = migrate(&scratch, &file, &["--json", "--yes"], REPLAYED_SERVICE, MP);
    assert_eq!(json.status.code(), Some(4), "{}", stderr(&json));
    let doc = document(&json);
    assert_eq!(doc["mode"], "EXECUTE");
    assert_eq!(doc["plan"]["package"]["name"], REPLAYED_SERVICE);
    assert_eq!(doc["execution"]["outcome"]["outcome"], "REFUSED");
    let refusals = doc["execution"]["gateRefusals"]
        .as_array()
        .expect("the refusals are in the document");
    assert_eq!(
        doc["execution"]["outcome"]["refusals"],
        refusals.len(),
        "the count is the list's"
    );
    assert!(refusals
        .iter()
        .any(|r| r["refusal"] == "MANUAL_REVIEW" && r["step"] == 1));
    assert!(
        doc["execution"].get("steps").is_none(),
        "a refused plan runs no step: {}",
        doc["execution"]
    );
}

#[test]
fn migrate_dry_run_wins_over_yes() {
    let scratch = Scratch::new("dry-run-wins");
    let file = fixture(BOTH);
    let plain = migrate(&scratch, &file, &["--json"], REPLAYED_SERVICE, MP);
    let both = migrate(
        &scratch,
        &file,
        &["--json", "--yes", "--dry-run"],
        REPLAYED_SERVICE,
        MP,
    );
    // A plan that stops at `MANUAL_REVIEW` renders with exit 4 (part D: the
    // one a recording can still plan).
    assert_eq!(both.status.code(), Some(4), "{}", stderr(&both));
    assert_eq!(document(&both)["mode"], "RENDER");
    assert!(document(&both).get("execution").is_none());
    assert_eq!(
        plain.stdout, both.stdout,
        "--dry-run with --yes is the render"
    );
}

#[test]
fn migrate_from_snapshot_refuses_to_execute() {
    // A snapshot file has no runner and no provenance an execution could
    // check (SE-050): execute mode is a usage error before any output.
    let scratch = Scratch::new("from-snapshot");
    let file = live_snapshot();
    let refused = from_snapshot(&scratch, &file, &["--yes"], "htop", MP);
    assert_eq!(refused.status.code(), Some(2), "{}", stderr(&refused));
    assert!(
        refused.stdout.is_empty(),
        "nothing is rendered: {}",
        stdout(&refused)
    );
    let text = stderr(&refused);
    assert_eq!(
        text.lines().count(),
        1,
        "the refusal and nothing else: {text}"
    );
    assert!(text.contains("--from-snapshot"), "{text}");
    assert!(
        !text.contains("error."),
        "an identifier reached the user: {text}"
    );
    // Even for an advanced view, whose notice would otherwise come first.
    let advanced = from_snapshot(&scratch, &file, &["--advanced", "--yes"], "htop", MP);
    assert_eq!(advanced.status.code(), Some(2));
    assert_eq!(
        stderr(&advanced).lines().count(),
        1,
        "{}",
        stderr(&advanced)
    );
    // Rendering from a snapshot file is fine, with or without --dry-run.
    for extra in [&["--json"][..], &["--json", "--yes", "--dry-run"][..]] {
        let shown = from_snapshot(&scratch, &file, extra, "htop", MP);
        assert_eq!(
            shown.status.code(),
            Some(0),
            "{extra:?}: {}",
            stderr(&shown)
        );
        assert_eq!(document(&shown)["mode"], "RENDER");
    }
}

#[test]
fn invariant_16_migrating_a_package_already_at_its_destination_changes_nothing_and_says_so() {
    let scratch = Scratch::new("nothing-to-move");
    let file = fixture(BOTH);
    for extra in [&[][..], &["--yes"][..], &["--dry-run"][..]] {
        let out = migrate(&scratch, &file, extra, "nginx", MP);
        assert_eq!(out.status.code(), Some(0), "{extra:?}: {}", stderr(&out));
        assert_eq!(
            stdout(&out),
            "Nothing to move: nginx is already under macports. In what was read, no other \
             provider holds it, nor anything its mappings name as possibly the same package. \
             Nothing was changed.\n\nPossibly inconsistent: this was read from a recording \
             or a snapshot file, which cannot show whether a package manager was changing the \
             machine it came from while it was read.\n",
            "{extra:?}"
        );
    }
    let json = migrate(&scratch, &file, &["--json", "--yes"], "nginx", MP);
    assert_eq!(json.status.code(), Some(0));
    let doc = document(&json);
    assert_eq!(doc["mode"], "EXECUTE");
    assert_eq!(doc["nothingToMove"]["package"], "nginx");
    assert_eq!(doc["nothingToMove"]["provider"], MP);
    assert_eq!(doc["nothingToMove"]["notDetected"], serde_json::json!([]));
    assert!(
        doc.get("plan").is_none() && doc.get("execution").is_none(),
        "{doc}"
    );
    assert!(
        !scratch.state_dir().exists(),
        "nothing was written to the state directory"
    );
}

#[test]
fn invariant_16_repeated_migrate_dry_runs_are_byte_identical() {
    let scratch = Scratch::new("repeated");
    let file = live_snapshot();
    for extra in [&[][..], &["--json"][..]] {
        let first = from_snapshot(&scratch, &file, extra, "htop", MP);
        let second = from_snapshot(&scratch, &file, extra, "htop", MP);
        assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
        assert_eq!(first.stdout, second.stdout, "{extra:?}");
        assert_eq!(first.stderr, second.stderr, "{extra:?}");
    }
}

/// The concurrency flag every `migrate` document carries, as JSON.
fn activity(doc: &Value) -> &Value {
    doc.get("concurrentProviderActivity")
        .unwrap_or_else(|| panic!("no concurrency flag in {doc}"))
}

/// Every shape a `migrate` document takes over `input`: a rendered plan, an
/// executed one (when `execute` is allowed), and nothing to move.
fn every_migrate_shape(execute: bool) -> Vec<(&'static [&'static str], &'static str, i32)> {
    // The plan is the one a recording can still build: a declared service,
    // stopping at `MANUAL_REVIEW` (exit 4; part D).
    let mut shapes: Vec<(&'static [&'static str], &'static str, i32)> = vec![
        (&["--json"][..], REPLAYED_SERVICE, 4),
        (&["--json", "--dry-run"][..], "nginx", 0),
    ];
    if execute {
        shapes.push((&["--json", "--yes"][..], REPLAYED_SERVICE, 4));
        shapes.push((&["--json", "--yes"][..], "nginx", 0));
    }
    shapes
}

fn assert_unknowable(doc: &Value, context: &str) {
    assert_eq!(
        activity(doc),
        &serde_json::json!({ "activity": "UNKNOWABLE", "possiblyInconsistent": true }),
        "{context}: {doc}"
    );
}

fn assert_unknowable_in_words(text: &str, context: &str) {
    assert!(
        text.contains("Possibly inconsistent: this was read from a recording or a snapshot file"),
        "{context}: {text}"
    );
    for raw in ["UNKNOWABLE", "NOT_CHECKED", "concurrency."] {
        assert!(
            !text.contains(raw),
            "{context}: {raw} reached the user:\n{text}"
        );
    }
}

#[test]
fn invariant_15_a_replayed_run_cannot_observe_this_hosts_processes() {
    // A recording carries no process table, and a replay must not consult
    // this host's (INV-15): every answer is `UNKNOWABLE`, never a check that
    // was not made, whatever this host's environment says.
    let scratch = Scratch::new("replay-concurrency");
    let file = fixture(BOTH);
    for (extra, package, code) in every_migrate_shape(true) {
        let out = migrate(&scratch, &file, extra, package, MP);
        assert_eq!(
            out.status.code(),
            Some(code),
            "{extra:?} {package}: {}",
            stderr(&out)
        );
        assert_unknowable(&document(&out), &format!("{extra:?} {package}"));
        let recording = file.to_str().expect("UTF-8");
        let mut argv = vec!["--replay", recording];
        argv.extend_from_slice(extra);
        argv.extend_from_slice(&["migrate", package, "--to", MP, "--today", TODAY]);
        let elsewhere = run_with_env(
            &scratch,
            &argv,
            &[
                ("HOME", "/nonexistent/elsewhere"),
                ("PATH", "/usr/bin:/bin"),
            ],
        );
        assert_eq!(out.stdout, elsewhere.stdout, "{extra:?} {package}");
        let human = migrate(&scratch, &file, &extra[1..], package, MP);
        assert_unknowable_in_words(&stdout(&human), &format!("{extra:?} {package}"));
    }
}

#[test]
fn a_migrate_from_a_snapshot_file_cannot_observe_this_hosts_processes_either() {
    // A snapshot file is the same as a recording here: whatever ran beside the
    // capture it came from is not in it, so the answer is `UNKNOWABLE`, not
    // the `NOT_CHECKED` of a live run that simply did not look.
    let scratch = Scratch::new("snapshot-concurrency");
    let file = snapshot_file(&scratch, BOTH);
    for (extra, package, code) in every_migrate_shape(false) {
        let out = from_snapshot(&scratch, &file, extra, package, MP);
        assert_eq!(
            out.status.code(),
            Some(code),
            "{extra:?} {package}: {}",
            stderr(&out)
        );
        assert_unknowable(&document(&out), &format!("{extra:?} {package}"));
        let human = from_snapshot(&scratch, &file, &extra[1..], package, MP);
        assert_unknowable_in_words(&stdout(&human), &format!("{extra:?} {package}"));
    }
}

#[test]
fn invariant_15_migrate_under_replay_is_a_function_of_the_recording() {
    // Executed against a recording, the document depends on the recording and
    // the flags alone: not on this host's home, path or working directory.
    let scratch = Scratch::new("replay-function");
    let file = fixture(BOTH);
    let args = [
        "--replay",
        file.to_str().expect("UTF-8"),
        "--json",
        "--yes",
        "migrate",
        REPLAYED_SERVICE,
        "--to",
        MP,
        "--today",
        TODAY,
    ];
    let first = run(&scratch, &args);
    let elsewhere = run_with_env(
        &scratch,
        &args,
        &[
            ("HOME", "/nonexistent/elsewhere"),
            ("PATH", "/usr/bin:/bin"),
        ],
    );
    assert_eq!(first.status.code(), Some(4), "{}", stderr(&first));
    assert_eq!(first.status.code(), elsewhere.status.code());
    assert_eq!(first.stdout, elsewhere.stdout);
    assert_eq!(first.stdout, run(&scratch, &args).stdout, "and run again");
}

#[test]
fn invariant_06_migrate_never_elevates() {
    // A plan with a step that needs administrator rights is refused for
    // execution by name; nothing is run, so nothing can ask for a password.
    //
    // Execute mode needs a runner, and over a recording a plan with commands
    // is no longer built (part D: a replayed source row cannot answer the
    // service question). So the plan comes from the live capture, and the gate
    // the binary's execute mode consults is asked of it in process.
    let scratch = Scratch::new("never-elevates");
    let snapshot = load_snapshot(&live_snapshot()).expect("the live capture loads");
    let start = migration_start(
        &snapshot,
        &PolicyData::load_embedded().expect("policy"),
        &AnalysisOptions {
            today: TODAY.to_string(),
            ..AnalysisOptions::default()
        },
        &PlanRequest {
            package: "python@3.12".to_string(),
            to: ProviderId::new(MP),
            retain_source: false,
        },
        &registry::known_provider_ids(),
        HistoryConsultation::NotConsulted,
    )
    .expect("a plan");
    let MigrationStart::Run(planned, _) = start else {
        panic!("python@3.12 has something to move");
    };
    let refusals = macportsbrew_application::execution_refusals(planned.plan());
    assert!(
        refusals.refusals().iter().any(|r| matches!(
            r,
            macportsbrew_application::ExecutionRefusal::ElevationRequired { steps }
                if steps == &[2]
        )),
        "{:?}",
        refusals.refusals()
    );
    // The binary renders the elevated step and asks for nothing.
    let human = from_snapshot(
        &scratch,
        &live_snapshot(),
        &["--dry-run"],
        "python@3.12",
        MP,
    );
    assert_eq!(human.status.code(), Some(0), "{}", stderr(&human));
    assert!(
        stdout(&human).contains("elevation required"),
        "{}",
        stdout(&human)
    );
    // And over a recording the same request is refused before any plan.
    let replayed = migrate(
        &scratch,
        &fixture(BOTH_INTEL),
        &["--yes"],
        "python@3.12",
        MP,
    );
    assert_eq!(replayed.status.code(), Some(4), "{}", stderr(&replayed));
    for output in [&human, &replayed] {
        assert!(!stdout(output).contains("sudo") && !stderr(output).contains("sudo"));
    }
}
/// `recording` with an answer for every provider command `plan_for` would
/// run, mutating ones included, so that a dispatch of one would *succeed* —
/// and be counted by the runner — rather than fail for want of an answer.
fn answering_every_plan_command(
    recording: &Recording,
    commands: &[(String, Vec<String>)],
) -> Recording {
    let providers: BTreeSet<String> = table().values().map(|e| leaf(&e.executable)).collect();
    let mut rec = recording.clone();
    for (program, argv) in commands {
        let program = leaf(program);
        if !providers.contains(&program)
            || rec
                .invocations
                .iter()
                .any(|r| r.invocation.program == program && &r.invocation.argv == argv)
        {
            continue;
        }
        rec.invocations.push(RecordedInvocation {
            invocation: Invocation {
                program,
                argv: argv.clone(),
                environment: BTreeMap::new(),
                status: ExitStatus::Exited { code: 0 },
                stdout: "  answered 1.0\n".to_string(),
                stderr: String::new(),
                truncated: false,
                duration_ms: 1,
            },
        });
    }
    rec
}

/// The prefixes a recording answers, as `--replay` takes them.
fn recorded_overrides(rec: &Recording) -> Vec<(ProviderId, PathBuf)> {
    let mut out = Vec::new();
    for (id, entry) in table() {
        let program = leaf(&entry.executable);
        let answered = rec
            .invocations
            .iter()
            .find(|r| r.invocation.program == program && r.invocation.argv == ["--prefix"]);
        let prefix = answered.map_or_else(
            || registry::candidate_prefixes(&ProviderId::new(&id))[0].clone(),
            |r| PathBuf::from(r.invocation.stdout.trim()),
        );
        out.push((ProviderId::new(&id), prefix));
    }
    out
}

#[test]
fn migrate_yes_against_a_recording_dispatches_no_mutating_invocation() {
    let scratch = Scratch::new("no-mutation");
    let policy = PolicyData::load_embedded().expect("policy");
    let mutating = mutating_prefixes();
    let mut answered_mutations = 0;
    // The plans come from the live capture (part D: a replayed source row cannot
    // answer the service question); the recording of the same machine answers
    // their commands, so a dispatch would succeed and be counted.
    for (fixture_name, package, to) in [(BOTH_INTEL, "htop", MP), (BOTH_INTEL, "python@3.12", MP)] {
        let original =
            Recording::from_json(&std::fs::read_to_string(fixture(fixture_name)).expect("reads"))
                .expect("parses");
        // The plan's commands, from the document `migrate` renders.
        let shown = from_snapshot(&scratch, &live_snapshot(), &["--json"], package, to);
        let doc = document(&shown);
        let commands: Vec<(String, Vec<String>)> = doc["plan"]["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .flat_map(|s| s["commands"].as_array().cloned().unwrap_or_default())
            .map(|c| {
                (
                    c["program"].as_str().expect("program").to_string(),
                    c["argv"]
                        .as_array()
                        .expect("argv")
                        .iter()
                        .map(|a| a.as_str().expect("string").to_string())
                        .collect(),
                )
            })
            .collect();
        let rec = answering_every_plan_command(&original, &commands);
        answered_mutations += rec
            .invocations
            .iter()
            .filter(|r| {
                let key = format!("{} {}", r.invocation.program, r.invocation.argv.join(" "));
                mutating.iter().any(|m| key.starts_with(m.as_str()))
            })
            .count();
        let file = scratch
            .path()
            .join(format!("{fixture_name}-{package}.json"));
        std::fs::write(&file, rec.to_json()).expect("write");

        // Through the binary, over the recording: refused before any plan, so
        // no step was run at all.
        let out = migrate(&scratch, &file, &["--json", "--yes"], package, to);
        assert_eq!(out.status.code(), Some(4), "{package}: {}", stderr(&out));
        assert!(out.stdout.is_empty(), "{package}: {}", stdout(&out));

        // In process, the composition `migrate` runs, over a runner that
        // counts what it was asked: nothing mutating, and in this build
        // nothing at all.
        let snapshot = load_snapshot(&live_snapshot()).expect("snapshot");
        let start = migration_start(
            &snapshot,
            &policy,
            &AnalysisOptions {
                today: TODAY.to_string(),
                ..AnalysisOptions::default()
            },
            &PlanRequest {
                package: package.to_string(),
                to: ProviderId::new(to),
                retain_source: false,
            },
            &registry::known_provider_ids(),
            HistoryConsultation::NotConsulted,
        )
        .expect("a plan");
        let MigrationStart::Run(planned, _) = start else {
            panic!("{package} has something to move");
        };
        let runner = ReplayRunner::new(rec.clone());
        let report = executor::run(
            &planned,
            &RegistryContext {
                runner: &runner,
                home: None,
                platform_tags: Box::new(|_| Vec::new()),
                filesystem: FilesystemScope::Elsewhere,
                prefix_override: Some(recorded_overrides(&rec)),
            },
            &HomeRedaction::Recorded,
        );
        assert!(
            matches!(report.outcome, RunOutcome::Refused { .. }),
            "{package}"
        );
        for key in runner.consulted() {
            assert!(
                !mutating.iter().any(|m| key.starts_with(m.as_str())),
                "{package}: a mutating invocation was dispatched: {key}"
            );
        }
        assert!(
            runner.consulted().is_empty(),
            "{package}: {:?}",
            runner.consulted()
        );
    }
    assert!(
        answered_mutations >= 3,
        "the recordings answered the plans' mutating commands, so a dispatch would have \
         succeeded and been seen: {answered_mutations}"
    );
}

/// The snapshot a replay of the recording at `file` captures.
fn snapshot_file_from(scratch: &Scratch, file: &Path) -> PathBuf {
    // Into the scratch directory, never beside the recording: a fixture
    // directory is input, and a stray file there would be read as one.
    let path = scratch.path().join(format!(
        "{}.snapshot.json",
        file.file_stem().expect("a file name").to_string_lossy()
    ));
    let out = run(
        scratch,
        &[
            "--replay",
            file.to_str().expect("UTF-8"),
            "snapshot",
            "--out",
            path.to_str().expect("UTF-8"),
            "--today",
            TODAY,
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    path
}

/// SE-092's S3 addendum, for `migrate`'s call site: a package only its
/// destination holds is not *nothing to move* on a document that carries no
/// entry for the other provider. A `known` taken from the document's own keys
/// would switch that check off, and only the binary shows what is passed.
#[test]
fn migrate_does_not_report_nothing_to_move_on_an_inventory_nobody_read() {
    let scratch = Scratch::new("deleted-provider");
    let file = snapshot_file(&scratch, BOTH);
    let control = from_snapshot(&scratch, &file, &[], "nginx", MP);
    assert_eq!(control.status.code(), Some(0), "{}", stderr(&control));
    assert!(
        stdout(&control).starts_with("Nothing to move"),
        "{}",
        stdout(&control)
    );

    let mut doc: Value =
        serde_json::from_slice(&std::fs::read(&file).expect("read")).expect("JSON");
    doc["providers"]
        .as_object_mut()
        .expect("providers")
        .remove(HB)
        .expect("the fixture carries homebrew");
    let missing = scratch.path().join("missing.json");
    std::fs::write(&missing, serde_json::to_vec(&doc).expect("serialise")).expect("write");
    let out = from_snapshot(&scratch, &missing, &[], "nginx", MP);
    let text = stderr(&out);
    assert_eq!(out.status.code(), Some(4), "{text}");
    assert!(text.contains(HB), "the refusal names it: {text}");
    assert!(out.stdout.is_empty(), "a refusal writes nothing to stdout");
}

#[test]
fn invariant_16_migrate_reports_nothing_to_move_where_another_provider_was_not_detected() {
    // MacPorts alone was detected. `gnutls` is held by it alone: nothing to
    // move, exit 0 (INV-16) - qualified, because a Homebrew installed where
    // detection does not look would read the same (DP-026 (b)).
    let scratch = Scratch::new("undetected");
    let out = migrate(&scratch, &fixture(MACPORTS_ONLY), &[], "gnutls", MP);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.starts_with("Nothing to move: gnutls is already under macports."),
        "{text}"
    );
    assert!(
        text.contains(
            "homebrew was not detected in what was read, so this is as far as \
                       detection can see"
        ) && text.contains("with any copy of gnutls under it, would not be found"),
        "{text}"
    );
    assert!(
        !text.contains("migrate."),
        "an identifier reached the user: {text}"
    );
    assert!(stderr(&out).is_empty(), "{}", stderr(&out));
    let json = migrate(&scratch, &fixture(MACPORTS_ONLY), &["--json"], "gnutls", MP);
    assert_eq!(json.status.code(), Some(0), "{}", stderr(&json));
    let doc = document(&json);
    assert_eq!(doc["nothingToMove"]["package"], "gnutls");
    assert_eq!(
        doc["nothingToMove"]["notDetected"],
        serde_json::json!(["homebrew"])
    );
}

#[test]
fn migrate_does_not_report_nothing_to_move_while_another_name_may_be_the_package() {
    // `node` maps ambiguously to `nodejs22` and `nodejs24`, so it sits in a
    // unit of its own and `nodejs22` reads as held by its destination alone.
    let scratch = Scratch::new("other-name");
    let out = migrate(&scratch, &fixture(BOTH), &[], "nodejs22", MP);
    let text = stderr(&out);
    assert_eq!(out.status.code(), Some(4), "{text}");
    assert!(out.stdout.is_empty(), "{}", stdout(&out));
    assert!(
        text.contains("homebrew holds node, which may be nodejs22 under another name"),
        "{text}"
    );
}

/// Every word of a source file, lower-cased.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .map(str::to_lowercase)
        .collect()
}

/// Whether `text` reaches the run loop's entry points through the executor
/// module: a path `executor::run`/`executor::run_with`, with any spacing, or an
/// import list from it naming either — so a caller cannot hide behind
/// `use crate::executor::run;` and a bare `run(...)`.
fn names_the_loop(text: &str) -> bool {
    let entry = |rest: &str| {
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        name == "run" || name == "run_with"
    };
    let mut rest = text;
    while let Some(i) = rest.find("executor") {
        rest = &rest[i + "executor".len()..];
        let after = rest.trim_start();
        let Some(after) = after.strip_prefix("::") else {
            continue;
        };
        let after = after.trim_start();
        if entry(after) {
            return true;
        }
        if let Some(list) = after.strip_prefix('{') {
            let list = &list[..list.find('}').unwrap_or(list.len())];
            if list.split(',').any(|item| entry(item.trim())) {
                return true;
            }
        }
    }
    false
}

#[test]
fn the_loop_scan_sees_every_way_of_naming_the_entry_points() {
    for caller in [
        "executor::run(&planned, &r, &h)",
        "crate::executor :: run_with(p, f, h)",
        "use crate::executor::run;",
        "use crate::executor::{self, run_with};",
        "use crate::executor::{\n    RunReport,\n    run,\n};",
    ] {
        assert!(names_the_loop(caller), "{caller}");
    }
    for other in [
        "use crate::executor::{RunReport, SiteRefusalKind};",
        "executor::render_refusal(r, h)",
        "executor::running",
    ] {
        assert!(!names_the_loop(other), "{other}");
    }
}

fn command_line_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root().join("crates/cli/src")];
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            for entry in std::fs::read_dir(&p).expect("readable") {
                stack.push(entry.expect("entry").path());
            }
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}

#[test]
fn the_context_runner_and_the_run_loop_are_reached_from_migrate_alone() {
    // SE-092: only `app/migrate.rs` reaches the context's runner, through
    // `Context::execution_registry`, and only it calls the run loop.
    let migrate_rs = root().join("crates/cli/src/app/migrate.rs");
    let context_rs = root().join("crates/cli/src/app/context.rs");
    let executor_dir = root().join("crates/cli/src/executor");
    let mut registry_users = BTreeSet::new();
    let mut loop_callers = BTreeSet::new();
    for path in command_line_sources() {
        let text = std::fs::read_to_string(&path).expect("UTF-8");
        if words(&text).iter().any(|w| w == "execution_registry") {
            registry_users.insert(path.clone());
        }
        if !path.starts_with(&executor_dir) && names_the_loop(&text) {
            loop_callers.insert(path.clone());
        }
    }
    assert_eq!(
        registry_users,
        [context_rs, migrate_rs.clone()].into_iter().collect(),
        "the context's runner is reached from migrate alone"
    );
    assert_eq!(
        loop_callers,
        [migrate_rs].into_iter().collect(),
        "migrate is the run loop's one caller"
    );
}

#[test]
fn invariant_02_migrate_runs_only_a_plan_tied_to_the_one_shown() {
    // INV-02 wants the plan a human has seen (`plan.v1.md`, obligation 8). In
    // `execute_mutating`, the plan is confirmed after the gate cleared it and
    // before the state directory is resolved, and the witness is what the
    // `MutatingRun` carries; the loop refuses a plan it does not cover
    // (`invariant_02_a_plan_other_than_the_one_shown_dispatches_nothing_and_takes_no_lock`).
    let source =
        std::fs::read_to_string(root().join("crates/cli/src/app/migrate.rs")).expect("migrate.rs");
    let body = &source[source
        .find("fn execute_mutating(")
        .expect("execute_mutating")..];
    let body = &body[..body.find("\n}\n").expect("its end")];
    let at = |t: &str| {
        body.find(t)
            .unwrap_or_else(|| panic!("execute_mutating calls {t}"))
    };
    assert!(at("clear(planned") < at("let shown = confirm_the_plan("));
    assert!(at("let shown = confirm_the_plan(") < at("state_and_user("));
    assert!(
        body.contains("            shown,\n"),
        "the MutatingRun carries the witness"
    );
    assert!(
        source.contains("fn confirm_the_plan(") && source.contains("crate::consent::confirm_plan("),
        "the witness comes from the consent prompt"
    );
    // The product code source is the random device, with no fallback (R7;
    // security review of E-2, 2): a constant code would let a line typed
    // ahead confirm the plan.
    let confirm = &source[source
        .find("fn confirm_the_plan(")
        .expect("confirm_the_plan")..];
    let confirm = &confirm[..confirm.find("\n}\n").expect("its end")];
    assert!(confirm.contains("let mut code = macportsbrew_adapters::entropy::fresh_bytes;\n"));
    assert_eq!(confirm.matches("code:").count(), 1, "one code source");
    assert!(confirm.contains("code: &mut code,"));
    for banned in [".or(", "unwrap_or", "Some([", "from_bytes("] {
        assert!(
            !confirm.contains(banned),
            "a fallback code source: {banned}"
        );
    }
    // The screen a person confirms on carries the plan's own banner, never
    // the dry-run sentence for a plan that will run (review 1).
    let human =
        std::fs::read_to_string(root().join("crates/cli/src/app/human.rs")).expect("human.rs");
    assert!(human.contains("let banner = if plan.dry_run_only {"));
}

#[test]
fn invariant_02_a_plan_the_gate_refuses_is_never_asked_about() {
    // R1, structurally: the witness can only be minted from a `GateClear`
    // (`confirm_shown` and `confirm_plan` take one), `clear` is the only
    // producer of one, and in `execute_mutating` the gate's refusal arm
    // returns before the question is asked, with the cleared run the only
    // thing the question is about. A behavioural case needs a dispatching
    // build, which this one is not.
    let plan_tie = std::fs::read_to_string(root().join("crates/application/src/plan_tie.rs"))
        .expect("plan_tie.rs");
    assert!(plan_tie.contains("pub fn confirm_shown(cleared: &GateClear,"));
    let consent =
        std::fs::read_to_string(root().join("crates/cli/src/consent.rs")).expect("consent.rs");
    assert!(consent.contains("pub fn confirm_plan(\n    cleared: &GateClear,"));
    let source =
        std::fs::read_to_string(root().join("crates/cli/src/app/migrate.rs")).expect("migrate.rs");
    let body = &source[source
        .find("fn execute_mutating(")
        .expect("execute_mutating")..];
    let body = &body[..body.find("\n}\n").expect("its end")];
    let refused = body
        .find("Err((_, report)) => return Ok(executor::refused_report(")
        .expect("the gate's refusal returns");
    let asked = body
        .find("let shown = confirm_the_plan(ctx, &cleared, stderr)?;")
        .expect("the question is about the cleared run");
    assert!(refused < asked);
    assert_eq!(body.matches("confirm_the_plan(").count(), 1, "asked once");
}

#[test]
fn migrate_shows_a_plan_that_stops_at_manual_review_and_refuses_to_execute_it() {
    // `node` has no strong counterpart among the ports and dependents of its
    // own, so its plan is MANUAL_REVIEW throughout (INV-19): shown, exit 4, from
    // the live capture. Under --yes, over a recording, the plan a recording can
    // still build (a declared service, part D) is refused by name.
    let scratch = Scratch::new("manual-review");
    let file = fixture(BOTH);
    let shown = from_snapshot(&scratch, &live_snapshot(), &["--json"], "node", MP);
    assert_eq!(shown.status.code(), Some(4), "{}", stderr(&shown));
    let doc = document(&shown);
    assert_eq!(doc["mode"], "RENDER");
    let kinds: Vec<&str> = doc["plan"]["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| s["kind"].as_str().expect("kind"))
        .collect();
    assert!(kinds.iter().all(|k| *k == "MANUAL_REVIEW"), "{kinds:?}");
    let human = from_snapshot(&scratch, &live_snapshot(), &[], "node", MP);
    assert_eq!(human.status.code(), Some(4));
    assert!(
        stdout(&human).contains("Plan: move node"),
        "{}",
        stdout(&human)
    );

    let replayed = document(&migrate(&scratch, &file, &["--json"], REPLAYED_SERVICE, MP));
    let kinds: Vec<&str> = replayed["plan"]["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| s["kind"].as_str().expect("kind"))
        .collect();
    assert!(kinds.iter().all(|k| *k == "MANUAL_REVIEW"), "{kinds:?}");
    let executed = migrate(&scratch, &file, &["--json", "--yes"], REPLAYED_SERVICE, MP);
    assert_eq!(executed.status.code(), Some(4), "{}", stderr(&executed));
    let doc = document(&executed);
    assert_eq!(doc["mode"], "EXECUTE");
    assert!(doc["plan"]["steps"].is_array(), "the plan is on stdout");
    let refusals = doc["execution"]["gateRefusals"]
        .as_array()
        .expect("refusals");
    assert_eq!(refusals.len(), kinds.len(), "{refusals:?}");
    for (i, refusal) in refusals.iter().enumerate() {
        assert_eq!(refusal["refusal"], "MANUAL_REVIEW", "{refusal}");
        assert_eq!(refusal["step"], i + 1, "{refusal}");
    }
    let human = migrate(&scratch, &file, &["--yes"], REPLAYED_SERVICE, MP);
    assert!(
        stdout(&human).contains("Step 1 needs a human decision, and nothing outranks it."),
        "{}",
        stdout(&human)
    );
}

#[test]
fn invariant_17_the_name_candidate_refusal_names_the_holder_in_the_mirror_direction() {
    // MacPorts holds `postgresql17-server`, whose mapping toward Homebrew
    // names `postgresql@17`: asked for at Homebrew alone, `postgresql@17` is
    // refused, and the refusal names the copy elsewhere, not the destination.
    let scratch = Scratch::new("other-name-mirror");
    let out = migrate(&scratch, &fixture(BOTH), &[], "postgresql@17", HB);
    let text = stderr(&out);
    assert_eq!(out.status.code(), Some(4), "{text}");
    assert!(out.stdout.is_empty(), "{}", stdout(&out));
    assert!(
        text.contains("macports holds postgresql17-server, which may be postgresql@17"),
        "{text}"
    );
}

#[test]
fn every_gate_refusal_of_every_plan_on_every_fixture_is_kept_as_an_object() {
    // The shell keeps the gate's refusals as cleaned documents; one that did
    // not serialise would be `UNSERIALISABLE`, never `null` or dropped. Every
    // plan the planner builds for a default-view package, toward each
    // provider, on every recording.
    let scratch = Scratch::new("refusals-serialise");
    let policy = PolicyData::load_embedded().expect("policy");
    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/recordings"))
        .expect("readable")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    fixtures.sort();
    let mut inputs: Vec<(PathBuf, PathBuf)> = fixtures
        .iter()
        .map(|file| (snapshot_file_from(&scratch, file), file.clone()))
        .collect();
    // Part D: a replayed source row that declares no service is refused before
    // the gate, so the plans with commands come from the live capture, run over
    // the recording of the same machine.
    inputs.push((live_snapshot(), fixture(BOTH_INTEL)));
    let mut checked = 0;
    for (snapshot_path, file) in inputs {
        let rec =
            Recording::from_json(&std::fs::read_to_string(&file).expect("reads")).expect("parses");
        let snapshot = load_snapshot(&snapshot_path).expect("snapshot");
        // Every unit of the default view: the packages a person would ask to
        // migrate. Dependency-level units are left out to keep this test
        // fast; their plans go through the same planner and gate.
        let names: BTreeSet<String> = macportsbrew_application::analyze(
            &snapshot,
            &policy,
            &AnalysisOptions {
                today: TODAY.to_string(),
                ..AnalysisOptions::default()
            },
            macportsbrew_application::AnalysisSource::Snapshot,
        )
        .packages
        .into_iter()
        .map(|u| u.name)
        .collect();
        let runner = ReplayRunner::new(rec.clone());
        for name in &names {
            for to in registry::known_provider_ids() {
                let Ok(MigrationStart::Run(planned, _)) = migration_start(
                    &snapshot,
                    &policy,
                    &AnalysisOptions {
                        today: TODAY.to_string(),
                        ..AnalysisOptions::default()
                    },
                    &PlanRequest {
                        package: name.clone(),
                        to,
                        retain_source: false,
                    },
                    &registry::known_provider_ids(),
                    HistoryConsultation::NotConsulted,
                ) else {
                    continue;
                };
                let report = executor::run(
                    &planned,
                    &RegistryContext {
                        runner: &runner,
                        home: None,
                        platform_tags: Box::new(|_| Vec::new()),
                        filesystem: FilesystemScope::Elsewhere,
                        prefix_override: Some(recorded_overrides(&rec)),
                    },
                    &HomeRedaction::Recorded,
                );
                assert!(!report.gate_refusals.is_empty(), "{name}: refused");
                for refusal in &report.gate_refusals {
                    assert!(
                        refusal.is_object()
                            && refusal["refusal"].is_string()
                            && refusal["refusal"] != "UNSERIALISABLE",
                        "{name}: {refusal}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(
            runner.consulted().is_empty(),
            "the gate refused before any read"
        );
    }
    assert!(checked > 20, "the scan reached the plans: {checked}");
}

#[test]
fn command_line_text_quoted_in_an_error_cannot_drive_the_terminal() {
    // A `--to` value reaches stderr inside the usage error; a control or format
    // character in it never reaches the terminal. clap's own rendering drops
    // an ANSI sequence outright; anything else is escaped as everywhere else
    // quoted text is shown.
    let scratch = Scratch::new("escaped-error");
    for (to, escaped) in [("mac\u{1b}[31mports", false), ("mac\u{202e}ports", true)] {
        let out = migrate(&scratch, &fixture(BOTH), &["--verbose"], "htop", to);
        let text = stderr(&out);
        assert_eq!(out.status.code(), Some(2), "{text}");
        assert!(
            !text.contains('\u{1b}') && !text.contains('\u{202e}'),
            "{text:?}"
        );
        if escaped {
            assert!(text.contains("mac\\u{202e}ports"), "{text:?}");
        }
    }
}

/// Fills `scratch`'s state directory with a history the store refuses
/// outright — a stranger entry, in a history open to others — so that any
/// read of it would be `HISTORY_UNUSABLE` and any write would show.
fn poison_history(scratch: &Scratch) {
    use std::os::unix::fs::PermissionsExt;
    let history = scratch.state_dir().join("history");
    std::fs::create_dir_all(&history).expect("history directory");
    std::fs::write(history.join("notes.txt"), b"not a record").expect("stranger entry");
    std::fs::set_permissions(&history, std::fs::Permissions::from_mode(0o755))
        .expect("history mode");
}

/// Every path under `dir` with its contents, for "nothing changed".
fn tree(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path.clone());
                out.insert(path, Vec::new());
            } else {
                out.insert(path.clone(), std::fs::read(&path).expect("file"));
            }
        }
    }
    out
}

#[test]
fn invariant_15_a_replayed_migrate_neither_reads_nor_writes_history() {
    // A recording is another machine, so its history is not this one's, even
    // with MACPORTSBREW_STATE_DIR exported and pointing at a real store
    // (security review, M5): a history any read would refuse changes nothing.
    let scratch = Scratch::new("history-replay");
    poison_history(&scratch);
    let before = tree(&scratch.state_dir());
    // The plan a recording can still build: a declared service, stopping at
    // `MANUAL_REVIEW` (part D). `--retain-source` still asks for history.
    let recording = fixture(BOTH);
    let recording = recording.to_str().unwrap();
    for mode in [&["--json"][..], &["--json", "--yes"][..]] {
        let mut extra = mode.to_vec();
        extra.push("--retain-source");
        let argv: Vec<&str> = ["--replay", recording]
            .into_iter()
            .chain([
                "migrate",
                REPLAYED_SERVICE,
                "--to",
                "macports",
                "--today",
                TODAY,
            ])
            .chain(extra)
            .collect();
        let out = run(&scratch, &argv);
        assert_ne!(
            out.status.code(),
            Some(8),
            "history was read: {}",
            stderr(&out)
        );
        let doc = document(&out);
        assert!(doc.get("plan").is_some(), "{doc}");
        assert!(doc.get("retainedRunNotRecognised").is_none(), "{doc}");
    }
    assert_eq!(tree(&scratch.state_dir()), before, "history was written");
}

#[test]
fn invariant_15_a_replayed_migrate_creates_neither_the_lock_nor_the_marker() {
    // A tripwire, not a proof: it holds for any build in which nothing takes
    // the lock, and starts to mean something when the executor shell does.
    // The single-instance lock and the interrupted-run marker belong to a run
    // that changes this machine. A recording is another machine, so a replayed
    // run, mutating flag or not, takes no lock and leaves no marker, and
    // neither file is created in the state directory (pass 08 slice 2).
    let scratch = Scratch::new("lock-replay");
    let recording = fixture(BOTH);
    let recording = recording.to_str().unwrap();
    for yes in [false, true] {
        let mut argv = vec![
            "--replay",
            recording,
            "migrate",
            REPLAYED_SERVICE,
            "--to",
            "macports",
            "--today",
            TODAY,
            "--json",
            "--retain-source",
        ];
        if yes {
            argv.push("--yes");
        }
        let out = run(&scratch, &argv);
        assert_ne!(
            out.status.code(),
            Some(7),
            "the lock was taken: {}",
            stderr(&out)
        );
        assert!(document(&out).get("plan").is_some());
        let root = scratch.state_dir();
        for name in ["run.lock", "interrupted-run.json"] {
            assert!(
                !root.join(name).exists(),
                "{name} was created under --replay"
            );
        }
    }
}

#[test]
fn invariant_15_analyze_and_plan_output_is_independent_of_history() {
    let clean = Scratch::new("history-clean");
    let poisoned = Scratch::new("history-poisoned");
    poison_history(&poisoned);
    let recording = fixture(BOTH);
    let recording = recording.to_str().unwrap();
    for args in [
        vec!["--replay", recording, "--today", TODAY, "--json", "analyze"],
        vec![
            "--replay",
            recording,
            "--today",
            TODAY,
            "--json",
            "plan",
            "htop",
            "--to",
            "macports",
            "--retain-source",
        ],
        vec!["--replay", recording, "--today", TODAY, "status"],
    ] {
        let a = run(&clean, &args);
        let b = run(&poisoned, &args);
        assert_eq!(a.status.code(), b.status.code(), "{args:?}");
        // The capture instant is the one wall-clock value in the output; two
        // runs a second apart differ there and nowhere else.
        let stable = |out: &Output| -> String {
            stdout(out)
                .lines()
                .filter(|l| !l.contains("capturedAt"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(
            stable(&a) == stable(&b),
            "history changed the output of {args:?}"
        );
    }
}

#[test]
fn invariant_15_a_migrate_from_a_snapshot_file_neither_reads_nor_writes_history() {
    // A snapshot file is another moment, so its history is not this one's:
    // the branch that builds its context says so, and this holds it.
    let scratch = Scratch::new("history-snapshot");
    let file = live_snapshot();
    poison_history(&scratch);
    let before = tree(&scratch.state_dir());
    let out = run(
        &scratch,
        &[
            "--from-snapshot",
            file.to_str().expect("UTF-8"),
            "--json",
            "migrate",
            "htop",
            "--to",
            "macports",
            "--retain-source",
            "--today",
            TODAY,
        ],
    );
    assert_ne!(
        out.status.code(),
        Some(8),
        "history was read: {}",
        stderr(&out)
    );
    assert!(document(&out).get("plan").is_some());
    assert_eq!(tree(&scratch.state_dir()), before, "history was written");
}

/// The live capture of a machine holding both providers
/// (`fixtures/snapshots/README.md`).
fn live_snapshot() -> PathBuf {
    root().join("fixtures/snapshots/both-providers-intel-macos26-live.json")
}

#[test]
fn invariant_17_a_macports_to_homebrew_plan_is_built_from_a_live_capture() {
    // A replay with MacPorts as the source is refused by design (DP-016(c));
    // a snapshot the binary took on a live machine is the input that can show
    // the MacPorts→Homebrew plan is first-class. Its step kinds mirror the
    // Homebrew→MacPorts plan for the same package, with the providers swapped,
    // and nothing in either needs a human decision.
    let scratch = Scratch::new("live-mirror");
    let file = live_snapshot();
    let file = file.to_str().expect("UTF-8");
    let plan = |to: &str| -> Vec<(String, String, Vec<String>)> {
        let out = run(
            &scratch,
            &[
                "--from-snapshot",
                file,
                "migrate",
                "htop",
                "--to",
                to,
                "--json",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let doc = document(&out);
        assert_eq!(doc["plan"]["destination"], to, "{doc}");
        doc["plan"]["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .map(|s| {
                (
                    s["kind"].as_str().expect("kind").to_string(),
                    s["provider"].as_str().unwrap_or("").to_string(),
                    s["verification"]
                        .as_array()
                        .map(|checks| {
                            checks
                                .iter()
                                .map(|c| c["check"].as_str().unwrap_or("").to_string())
                                .collect()
                        })
                        .unwrap_or_default(),
                )
            })
            .collect()
    };
    let to_homebrew = plan(HB);
    let to_macports = plan(MP);
    let kinds = |p: &[(String, String, Vec<String>)]| {
        p.iter().map(|(k, _, _)| k.clone()).collect::<Vec<_>>()
    };
    assert_eq!(kinds(&to_homebrew), kinds(&to_macports));
    assert!(
        kinds(&to_homebrew).iter().all(|k| k != "MANUAL_REVIEW"),
        "{to_homebrew:?}"
    );
    let remove = kinds(&to_homebrew)
        .iter()
        .position(|k| k == "REMOVE_SOURCE")
        .expect("the source is removed");
    let verify = kinds(&to_homebrew)
        .iter()
        .position(|k| k == "VERIFY_DESTINATION")
        .expect("the destination is verified");
    assert!(verify < remove, "{to_homebrew:?}");
    let swap = |p: &str| match p {
        HB => MP.to_string(),
        MP => HB.to_string(),
        other => other.to_string(),
    };
    // Same kinds, providers swapped, and the same checks at every step.
    let mirrored: Vec<(String, String, Vec<String>)> = to_macports
        .iter()
        .map(|(k, p, c)| (k.clone(), swap(p), c.clone()))
        .collect();
    assert_eq!(to_homebrew, mirrored);
    assert_eq!(to_homebrew[remove].1, MP);
}
