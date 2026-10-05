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
    let scratch = Scratch::new("dry-run-renders");
    let file = fixture(BOTH_INTEL);
    let json = migrate(&scratch, &file, &["--json", "--dry-run"], "python@3.12", MP);
    assert_eq!(json.status.code(), Some(0), "{}", stderr(&json));
    let doc = document(&json);
    assert_eq!(doc["schemaVersion"], 1);
    assert_eq!(doc["mode"], "RENDER");
    assert!(doc.get("execution").is_none(), "a render runs nothing");
    let human = migrate(&scratch, &file, &["--dry-run"], "python@3.12", MP);
    assert_eq!(human.status.code(), Some(0), "{}", stderr(&human));
    let text = stdout(&human);
    let mut commands = 0;
    let mut elevated = 0;
    for step in doc["plan"]["steps"].as_array().expect("steps") {
        for command in step["commands"].as_array().into_iter().flatten() {
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
    let scratch = Scratch::new("yes-refused");
    let file = fixture(BOTH);
    let human = migrate(&scratch, &file, &["--yes"], "htop", MP);
    assert_eq!(human.status.code(), Some(4), "{}", stderr(&human));
    let text = stdout(&human);
    assert!(
        text.contains("Plan: move htop from homebrew to macports"),
        "{text}"
    );
    assert!(
        text.contains("The execution gate refused this plan"),
        "{text}"
    );
    assert!(
        text.contains("Step 2 changes the machine, and this build changes nothing."),
        "{text}"
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
    let json = migrate(&scratch, &file, &["--json", "--yes"], "htop", MP);
    assert_eq!(json.status.code(), Some(4), "{}", stderr(&json));
    let doc = document(&json);
    assert_eq!(doc["mode"], "EXECUTE");
    assert_eq!(doc["plan"]["package"]["name"], "htop");
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
        .any(|r| r["refusal"] == "MUTATION_NOT_IN_THIS_BUILD" && r["step"] == 2));
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
    let plain = migrate(&scratch, &file, &["--json"], "htop", MP);
    let both = migrate(
        &scratch,
        &file,
        &["--json", "--yes", "--dry-run"],
        "htop",
        MP,
    );
    assert_eq!(both.status.code(), Some(0), "{}", stderr(&both));
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
    let file = snapshot_file(&scratch, BOTH);
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
    let file = snapshot_file(&scratch, BOTH);
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
    let mut shapes: Vec<(&'static [&'static str], &'static str, i32)> = vec![
        (&["--json"][..], "htop", 0),
        (&["--json", "--dry-run"][..], "nginx", 0),
    ];
    if execute {
        shapes.push((&["--json", "--yes"][..], "htop", 4));
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
        "htop",
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
    let scratch = Scratch::new("never-elevates");
    let file = fixture(BOTH_INTEL);
    let out = migrate(&scratch, &file, &["--json", "--yes"], "python@3.12", MP);
    assert_eq!(out.status.code(), Some(4), "{}", stderr(&out));
    let doc = document(&out);
    let refusals = doc["execution"]["gateRefusals"]
        .as_array()
        .expect("refusals");
    assert!(
        refusals
            .iter()
            .any(|r| r["refusal"] == "ELEVATION_REQUIRED" && r["steps"] == serde_json::json!([2])),
        "{refusals:?}"
    );
    assert!(doc["execution"].get("steps").is_none());
    let human = migrate(&scratch, &file, &["--yes"], "python@3.12", MP);
    assert!(stdout(&human).contains(
        "Steps that need administrator rights, which this build has no way to ask for: 2."
    ));
    for output in [&out, &human] {
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
    for (fixture_name, package, to) in [(BOTH, "htop", MP), (BOTH_INTEL, "python@3.12", MP)] {
        let original =
            Recording::from_json(&std::fs::read_to_string(fixture(fixture_name)).expect("reads"))
                .expect("parses");
        // The plan's commands, from the document `migrate` renders.
        let shown = migrate(&scratch, &fixture(fixture_name), &["--json"], package, to);
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

        // Through the binary: refused, and no step was run at all.
        let out = migrate(&scratch, &file, &["--json", "--yes"], package, to);
        assert_eq!(out.status.code(), Some(4), "{package}: {}", stderr(&out));
        let doc = document(&out);
        assert_eq!(doc["execution"]["outcome"]["outcome"], "REFUSED");
        assert!(doc["execution"].get("steps").is_none());

        // In process, the composition `migrate` runs, over a runner that
        // counts what it was asked: nothing mutating, and in this build
        // nothing at all.
        let snapshot = load_snapshot(&snapshot_file_from(&scratch, &file)).expect("snapshot");
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
fn invariant_02_migrate_executes_a_plan_built_in_process_only_while_nothing_destructive_is_dispatched(
) {
    // INV-02 wants a plan object a human has seen. `migrate --yes` builds and
    // executes the plan in one process, which is compatible with it only
    // while nothing destructive is dispatched (SE-092; `plan.v1.md`,
    // obligation 8). This fails the day both hold at once: a build that can
    // dispatch a mutating step, and `migrate` still executing the plan it
    // built itself rather than one tied to what was shown.
    let source =
        std::fs::read_to_string(root().join("crates/cli/src/app/migrate.rs")).expect("migrate.rs");
    let builds_its_own_plan = source.contains("migration_start(");
    assert!(
        !(macportsbrew_domain::plan::THIS_BUILD_DISPATCHES_MUTATIONS && builds_its_own_plan),
        "a build that dispatches mutations must tie the executed plan to the one a human \
         saw (plan.v1.md obligation 8) before `migrate --yes` may run a plan it built itself"
    );
    assert!(
        builds_its_own_plan,
        "migrate no longer builds the plan it executes: revisit this tripwire and \
         obligation 8 together"
    );
}

#[test]
fn migrate_shows_a_plan_that_stops_at_manual_review_and_refuses_to_execute_it() {
    // `node` maps ambiguously to two ports, so its plan is MANUAL_REVIEW
    // throughout (INV-19): shown, exit 4, and refused by name under --yes.
    let scratch = Scratch::new("manual-review");
    let file = fixture(BOTH);
    let shown = migrate(&scratch, &file, &["--json"], "node", MP);
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
    let human = migrate(&scratch, &file, &[], "node", MP);
    assert_eq!(human.status.code(), Some(4));
    assert!(
        stdout(&human).contains("Plan: move node"),
        "{}",
        stdout(&human)
    );

    let executed = migrate(&scratch, &file, &["--json", "--yes"], "node", MP);
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
    let human = migrate(&scratch, &file, &["--yes"], "node", MP);
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
    let mut checked = 0;
    for file in fixtures {
        let rec =
            Recording::from_json(&std::fs::read_to_string(&file).expect("reads")).expect("parses");
        let snapshot = load_snapshot(&snapshot_file_from(&scratch, &file)).expect("snapshot");
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
    // A `--to` value reaches stderr inside the refusal; a control or format
    // character in it is escaped there as everywhere else plan text is shown.
    let scratch = Scratch::new("escaped-error");
    for to in ["mac\u{1b}[31mports", "mac\u{202e}ports"] {
        let out = migrate(&scratch, &fixture(BOTH), &["--verbose"], "htop", to);
        let text = stderr(&out);
        assert_ne!(out.status.code(), Some(0), "{text}");
        assert!(
            !text.contains('\u{1b}') && !text.contains('\u{202e}'),
            "{text:?}"
        );
        assert!(text.contains("mac\\u{"), "{text:?}");
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
    // `htop --to macports --retain-source` is a full plan in this recording.
    let recording = fixture(BOTH);
    let recording = recording.to_str().unwrap();
    for mode in [&["--json"][..], &["--json", "--yes"][..]] {
        let mut extra = mode.to_vec();
        extra.push("--retain-source");
        let argv: Vec<&str> = ["--replay", recording]
            .into_iter()
            .chain(["migrate", "htop", "--to", "macports", "--today", TODAY])
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
    let file = snapshot_file(&scratch, BOTH);
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
