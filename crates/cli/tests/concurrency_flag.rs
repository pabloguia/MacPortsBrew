// SPDX-License-Identifier: Apache-2.0

//! The concurrency flag on `plan` and `analyze`, end to end, and the one
//! serialiser all three commands share (`plan.v1.md`, obligation 5; SE-015).
//!
//! `04-migration-and-safety.md` lets read-only analysis proceed while a
//! provider runs, *flagged as possibly inconsistent*. `migrate` carried the
//! flag first (`crates/cli/tests/migrate.rs`); this file pins it on the other
//! two commands that answer a question a person acts on, from the same
//! `AnalysisSource::concurrent_provider_activity`, in the same shape.
//!
//! Nothing here touches a live installation (INV-05): every run is `--replay`
//! or `--from-snapshot`, which is also why every answer pinned here is
//! `UNKNOWABLE`. A live run's `NOT_CHECKED` cannot be driven end to end, and
//! SE-015 records that as open.

use macportsbrew_application::AnalysisSource;
use macportsbrew_domain::ConcurrencyFlag;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");
const TODAY: &str = "2026-09-02";
const MP: &str = "macports";

/// Both providers: `htop --to macports` plans and exits 0, `nginx --to
/// macports` is refused because MacPorts already holds it.
const BOTH: &str = "both-providers-arm64-macos15";
/// Homebrew alone: `curl --to macports` is all `MANUAL_REVIEW`, exit 4.
const HOMEBREW_ONLY: &str = "homebrew-only-intel-macos26";

/// The sentence `UNKNOWABLE` renders to, from the catalogue.
const UNKNOWABLE_IN_WORDS: &str =
    "Possibly inconsistent: this was read from a recording or a snapshot file";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

fn fixture(name: &str) -> String {
    root()
        .join(format!("fixtures/recordings/{name}.json"))
        .to_str()
        .expect("UTF-8 path")
        .to_string()
}

/// A temporary directory unique to one test, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "macportsbrew-concurrency-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temporary directory");
        Self(dir)
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
        .env("MACPORTSBREW_STATE_DIR", scratch.0.join("state"))
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

/// The same command, under a home and a path that are not this host's.
fn run_elsewhere(scratch: &Scratch, args: &[&str]) -> Output {
    run_with_env(
        scratch,
        args,
        &[
            ("HOME", "/nonexistent/elsewhere"),
            ("PATH", "/usr/bin:/bin"),
        ],
    )
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

/// `args` with the input flags in front and `--today` behind.
fn argv<'a>(input: &[&'a str], args: &[&'a str]) -> Vec<&'a str> {
    let mut full = input.to_vec();
    full.extend_from_slice(args);
    full.extend_from_slice(&["--today", TODAY]);
    full
}

/// The snapshot a replay of `name` captures, written into `scratch`.
fn snapshot_file(scratch: &Scratch, name: &str) -> String {
    let path = scratch.0.join(format!("{name}.snapshot.json"));
    let path = path.to_str().expect("UTF-8").to_string();
    let recording = fixture(name);
    let out = run(
        scratch,
        &argv(&["--replay", &recording], &["snapshot", "--out", &path]),
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    path
}

/// The `plan` and `analyze` invocations this file pins, with their exit
/// codes: a plan that exits 0, a plan that stops at manual review (exit 4),
/// and an analysis.
fn invocations() -> Vec<(&'static str, Vec<&'static str>, i32)> {
    vec![
        (BOTH, vec!["plan", "htop", "--to", MP], 0),
        (HOMEBREW_ONLY, vec!["plan", "curl", "--to", MP], 4),
        (BOTH, vec!["analyze"], 0),
        (HOMEBREW_ONLY, vec!["analyze", "curl"], 0),
    ]
}

fn assert_unknowable(doc: &Value, context: &str) {
    assert_eq!(
        doc.get("concurrentProviderActivity"),
        Some(&serde_json::json!({ "activity": "UNKNOWABLE", "possiblyInconsistent": true })),
        "{context}: {doc}"
    );
}

/// The human output ends with the flag in words, and no raw identifier or
/// catalogue key reaches the person reading it.
fn assert_unknowable_in_words(text: &str, context: &str) {
    let last = text.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("");
    assert!(
        last.starts_with(UNKNOWABLE_IN_WORDS),
        "{context}: the last line is not the flag:\n{text}"
    );
    for raw in [
        "UNKNOWABLE",
        "NOT_CHECKED",
        "possiblyInconsistent",
        "concurrency.",
    ] {
        assert!(
            !text.contains(raw),
            "{context}: {raw} reached the user:\n{text}"
        );
    }
}

/// `capturedAt` is the one field a replay stamps from the wall clock (see
/// `end_to_end.rs`), so it is blanked before two replayed documents are
/// compared byte for byte. Nothing else is touched.
fn without_capture_time(text: &str, json: bool) -> String {
    // Structured, as `end_to_end.rs` does: only `.snapshotProvenance.capturedAt`,
    // the one field a replay stamps from the clock, is blanked, so a second
    // field of that name anywhere else would still be compared (spec-guardian,
    // 2026-10-03). Human output is compared as it is.
    if !json {
        return text.to_string();
    }
    let mut value: serde_json::Value = serde_json::from_str(text).expect("--json prints JSON");
    if let Some(obj) = value
        .get_mut("snapshotProvenance")
        .and_then(serde_json::Value::as_object_mut)
    {
        obj.insert(
            "capturedAt".into(),
            serde_json::Value::from("<captured-at>"),
        );
    }
    serde_json::to_string_pretty(&value).expect("JSON re-serialises")
}

#[test]
fn plan_and_analyze_from_a_recording_say_the_flag_is_unknowable() {
    let scratch = Scratch::new("replay");
    for (recording, args, code) in invocations() {
        let file = fixture(recording);
        let input = ["--replay", file.as_str()];
        let context = format!("{recording} {args:?}");
        let json = run(&scratch, &argv(&[&input[..], &["--json"]].concat(), &args));
        assert_eq!(
            json.status.code(),
            Some(code),
            "{context}: {}",
            stderr(&json)
        );
        assert_unknowable(&document(&json), &context);
        let human = run(&scratch, &argv(&input, &args));
        assert_eq!(
            human.status.code(),
            Some(code),
            "{context}: {}",
            stderr(&human)
        );
        assert_unknowable_in_words(&stdout(&human), &context);
    }
}

#[test]
fn plan_and_analyze_from_a_snapshot_file_say_the_flag_is_unknowable() {
    // A snapshot file carries nothing about whatever ran beside the capture
    // it came from: `UNKNOWABLE`, not the `NOT_CHECKED` of a live run.
    let scratch = Scratch::new("snapshot");
    for (recording, args, code) in invocations() {
        let file = snapshot_file(&scratch, recording);
        let input = ["--from-snapshot", file.as_str()];
        let context = format!("{recording} {args:?}");
        let json = run(&scratch, &argv(&[&input[..], &["--json"]].concat(), &args));
        assert_eq!(
            json.status.code(),
            Some(code),
            "{context}: {}",
            stderr(&json)
        );
        assert_unknowable(&document(&json), &context);
        let human = run(&scratch, &argv(&input, &args));
        assert_eq!(
            human.status.code(),
            Some(code),
            "{context}: {}",
            stderr(&human)
        );
        assert_unknowable_in_words(&stdout(&human), &context);
    }
}

#[test]
fn invariant_15_a_replayed_plan_or_analysis_cannot_observe_this_hosts_processes() {
    // A recording carries no process table and a replay must not consult this
    // host's (INV-15): run again under another home and path, the documents
    // and the sentences are the same bytes. Only `capturedAt`, which a replay
    // stamps from the clock, is blanked.
    let scratch = Scratch::new("replay-elsewhere");
    for (recording, args, code) in invocations() {
        let file = fixture(recording);
        let input = ["--replay", file.as_str()];
        let context = format!("{recording} {args:?}");
        for json in [true, false] {
            let full = if json {
                argv(&[&input[..], &["--json"]].concat(), &args)
            } else {
                argv(&input, &args)
            };
            let here = run(&scratch, &full);
            let elsewhere = run_elsewhere(&scratch, &full);
            assert_eq!(
                here.status.code(),
                Some(code),
                "{context}: {}",
                stderr(&here)
            );
            assert_eq!(here.status.code(), elsewhere.status.code(), "{context}");
            assert_eq!(
                without_capture_time(&stdout(&here), json),
                without_capture_time(&stdout(&elsewhere), json),
                "{context} json={json}"
            );
        }
    }
}

#[test]
fn invariant_15_a_plan_or_analysis_from_a_snapshot_file_is_a_function_of_the_file() {
    // Nothing is normalised here: the file fixes every byte, the flag included,
    // whatever this host's home and path say.
    let scratch = Scratch::new("snapshot-elsewhere");
    for (recording, args, code) in invocations() {
        let file = snapshot_file(&scratch, recording);
        let input = ["--from-snapshot", file.as_str()];
        let context = format!("{recording} {args:?}");
        for json in [true, false] {
            let full = if json {
                argv(&[&input[..], &["--json"]].concat(), &args)
            } else {
                argv(&input, &args)
            };
            let here = run(&scratch, &full);
            let elsewhere = run_elsewhere(&scratch, &full);
            assert_eq!(
                here.status.code(),
                Some(code),
                "{context}: {}",
                stderr(&here)
            );
            assert_eq!(here.status.code(), elsewhere.status.code(), "{context}");
            assert_eq!(here.stdout, elsewhere.stdout, "{context} json={json}");
        }
    }
}

#[test]
fn the_plan_document_appends_the_flag_after_every_field_it_already_had() {
    // The plan is flattened first, so every byte `plan --json` wrote before
    // the field keeps its place; and a refused plan writes nothing to stdout,
    // so there is no document there to carry a flag.
    let scratch = Scratch::new("plan-order");
    let file = fixture(BOTH);
    let out = run(
        &scratch,
        &argv(
            &["--replay", &file, "--json"],
            &["plan", "htop", "--to", MP],
        ),
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let doc = document(&out);
    let keys: Vec<&String> = doc.as_object().expect("an object").keys().collect();
    assert_eq!(
        keys.last().map(|k| k.as_str()),
        Some("concurrentProviderActivity"),
        "{keys:?}"
    );
    assert_eq!(keys.first().map(|k| k.as_str()), Some("schemaVersion"));
    assert!(
        stdout(&out).contains("  \"dryRunOnly\": true,\n  \"concurrentProviderActivity\": {"),
        "{}",
        stdout(&out)
    );

    let refused = run(
        &scratch,
        &argv(
            &["--replay", &file, "--json"],
            &["plan", "nginx", "--to", MP],
        ),
    );
    assert_eq!(refused.status.code(), Some(2), "{}", stderr(&refused));
    assert!(refused.stdout.is_empty(), "{}", stdout(&refused));
}

/// The raw bytes of the flag in a pretty-printed root document, from its key
/// to the brace that closes it.
fn fragment(text: &str) -> String {
    let start = text
        .find("  \"concurrentProviderActivity\": {")
        .unwrap_or_else(|| panic!("no flag in:\n{text}"));
    let rest = &text[start..];
    let end = rest.find("\n  }").expect("the flag closes") + "\n  }".len();
    rest[..end].to_string()
}

#[test]
fn analyze_plan_and_migrate_share_the_one_concurrency_serialiser() {
    // One type writes the flag for every command (`ConcurrencyFlag`), so the
    // same input yields the same bytes under the same key in every document:
    // `analyze`, `status` and `list` (one `Analysis`), `plan`, `migrate`. A
    // command that grew its own shape — another field order, a hard-coded
    // `possiblyInconsistent` — would differ here.
    let scratch = Scratch::new("shared");
    let file = fixture(BOTH);
    let input = ["--replay", file.as_str(), "--json"];
    let commands: [&[&str]; 5] = [
        &["analyze"],
        &["status"],
        &["list"],
        &["plan", "htop", "--to", MP],
        &["migrate", "htop", "--to", MP],
    ];
    let expected = {
        let flag = ConcurrencyFlag::of(AnalysisSource::Snapshot.concurrent_provider_activity());
        let wrapped = serde_json::json!({ "concurrentProviderActivity": flag });
        let text = serde_json::to_string_pretty(&wrapped).expect("serialises");
        fragment(&text)
    };
    for args in commands {
        let out = run(&scratch, &argv(&input, args));
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        assert_eq!(fragment(&stdout(&out)), expected, "{args:?}");
    }

    // And in words: the three commands that end with the flag end with the
    // one sentence; `status` and `list` carry it in `--json` only (SE-015).
    let human = ["--replay", file.as_str()];
    let mut last_lines = Vec::new();
    for args in &commands[..] {
        let out = run(&scratch, &argv(&human, args));
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        let text = stdout(&out);
        let ends_with_flag = text
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .is_some_and(|l| l.starts_with(UNKNOWABLE_IN_WORDS));
        let overview = matches!(args.first(), Some(&"status" | &"list"));
        assert_eq!(ends_with_flag, !overview, "{args:?}:\n{text}");
        assert_eq!(
            text.contains(UNKNOWABLE_IN_WORDS),
            !overview,
            "{args:?}:\n{text}"
        );
        if !overview {
            last_lines.push(
                text.lines()
                    .rfind(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .to_string(),
            );
        }
    }
    assert_eq!(last_lines.len(), 3);
    assert!(
        last_lines.windows(2).all(|w| w[0] == w[1]),
        "{last_lines:?}"
    );
}
