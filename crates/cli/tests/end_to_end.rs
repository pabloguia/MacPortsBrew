// SPDX-License-Identifier: Apache-2.0

//! End-to-end tests: the built binary, over the fixture machines, asserting on
//! `--json` (`docs/spec/07-quality-and-testing.md`, "Testing").
//!
//! These spawn the real executable rather than calling `run()` in-process, so
//! what is asserted is what a user or a script actually gets: the exit code,
//! stdout, and the promise of `06-cli-and-ux.md` that JSON goes to stdout and
//! nothing else does.
//!
//! Every run is given a `MACPORTSBREW_STATE_DIR` inside a per-test temporary
//! directory. Nothing here may reach the developer's real state directory, and
//! `invariant_16_idempotent_read_commands` asserts that nothing was written
//! there at all.
//!
//! One field is not deterministic and the tests say so where it matters:
//! `--replay` re-captures a snapshot on every run, and a capture stamps itself
//! with the wall clock, so `.snapshotProvenance.capturedAt` differs between two
//! runs of the same command. It is the only such field. INV-15 is asserted on
//! the `--from-snapshot` path the invariant itself names, where nothing is
//! normalised at all.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");
const MPB: &str = env!("CARGO_BIN_EXE_mpb");
const TODAY: &str = "2026-09-02";

const BOTH: &str = "both-providers-arm64-macos15";
const HOMEBREW_ONLY: &str = "homebrew-only-intel-macos26";
const MACPORTS_ONLY: &str = "macports-only-intel-macos12";
const BROKEN: &str = "macports-broken";
const NEITHER: &str = "neither-provider-intel-macos13";
const TAPPED: &str = "homebrew-deprecated-and-tap-intel-macos26";

/// The repository root: this crate is `crates/cli`.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/cli has a grandparent")
        .to_path_buf()
}

fn fixture(name: &str) -> PathBuf {
    root()
        .join("fixtures/recordings")
        .join(format!("{name}.json"))
}

/// A temporary directory unique to one test, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "macportsbrew-e2e-{label}-{}-{:?}",
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

/// Runs a binary with the state directory redirected into `scratch`.
fn run_bin(binary: &str, scratch: &Scratch, args: &[&str]) -> Output {
    Command::new(binary)
        .args(args)
        .current_dir(root())
        .env("MACPORTSBREW_STATE_DIR", scratch.state_dir())
        .env_remove("MACPORTSBREW_LANG")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .unwrap_or_else(|e| panic!("spawning {binary}: {e}"))
}

fn run(scratch: &Scratch, args: &[&str]) -> Output {
    run_bin(BIN, scratch, args)
}

/// Runs a `--json --replay` command and returns its parsed stdout, asserting
/// the exit code and that the schema version is the one the CLI contract
/// promises.
///
/// `notices` says whether this invocation is one of the documented ones that
/// writes a notice to stderr. `06-cli-and-ux.md` puts notices on stderr
/// precisely so stdout stays machine-readable, so the invariant asserted
/// everywhere is about *stdout*; a silent invocation that starts writing
/// notices should still fail, which is why the default is `false`.
fn json_run(scratch: &Scratch, name: &str, extra: &[&str], notices: bool) -> Value {
    let path = fixture(name);
    let path = path.to_str().expect("fixture path is UTF-8");
    let mut argv = vec!["--replay", path, "--json"];
    argv.extend_from_slice(extra);
    argv.extend_from_slice(&["--today", TODAY]);
    let out = run(scratch, &argv);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{argv:?} exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    if !notices {
        assert!(
            out.stderr.is_empty(),
            "{argv:?} wrote to stderr under --json: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let value: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{argv:?} stdout is not JSON: {e}"));
    assert_eq!(value["schemaVersion"], 1, "{argv:?}");
    value
}

fn json_ok(scratch: &Scratch, name: &str, args: &[&str]) -> Value {
    json_run(scratch, name, args, false)
}

fn presence(analysis: &Value, provider: &str) -> String {
    analysis["providers"][provider]["detection"]["presence"]["state"]
        .as_str()
        .unwrap_or("<missing>")
        .to_string()
}

fn units(analysis: &Value) -> &Vec<Value> {
    analysis["packages"].as_array().expect("packages is a list")
}

fn unit<'a>(analysis: &'a Value, name: &str) -> &'a Value {
    units(analysis)
        .iter()
        .find(|u| u["name"] == name)
        .unwrap_or_else(|| panic!("no unit named {name}"))
}

/// The reason identifiers a unit's recommendation carries, with the rule that
/// produced each: the rule that decided the outcome is not the only one that
/// fired, and a rule firing is itself a fact worth asserting.
fn rules_fired(unit: &Value) -> Vec<String> {
    unit["recommendation"]["reasons"]
        .as_array()
        .map(|rs| {
            rs.iter()
                .map(|r| r["rule"].as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Replaces the one wall-clock field so two runs of a `--replay` command can be
/// compared. Everything else is left exactly as the binary wrote it.
fn without_capture_time(bytes: &[u8]) -> Value {
    let mut value: Value = serde_json::from_slice(bytes).expect("JSON");
    if let Some(p) = value.get_mut("snapshotProvenance") {
        if let Some(obj) = p.as_object_mut() {
            obj.insert("capturedAt".into(), Value::from("<captured-at>"));
        }
    }
    value
}

// ---------------------------------------------------------------------------
// Provider presence, per fixture
// ---------------------------------------------------------------------------

#[test]
fn every_fixture_reports_the_provider_states_it_was_built_for() {
    let scratch = Scratch::new("presence");
    for (name, homebrew, macports) in [
        (BOTH, "PRESENT", "PRESENT"),
        (HOMEBREW_ONLY, "PRESENT", "ABSENT"),
        (TAPPED, "PRESENT", "ABSENT"),
        (MACPORTS_ONLY, "ABSENT", "PRESENT"),
        (NEITHER, "ABSENT", "ABSENT"),
        (BROKEN, "ABSENT", "BROKEN"),
    ] {
        let a = json_ok(&scratch, name, &["analyze"]);
        assert_eq!(presence(&a, "homebrew"), homebrew, "{name}: homebrew");
        assert_eq!(presence(&a, "macports"), macports, "{name}: macports");
    }
}

#[test]
fn the_machine_facts_are_the_ones_each_fixture_records() {
    let scratch = Scratch::new("machine");
    for (name, arch, major) in [
        (BOTH, "arm64", 15),
        (HOMEBREW_ONLY, "x86_64", 26),
        (MACPORTS_ONLY, "x86_64", 12),
        (NEITHER, "x86_64", 13),
    ] {
        let a = json_ok(&scratch, name, &["analyze"]);
        assert_eq!(a["machine"]["architecture"], arch, "{name}");
        assert_eq!(a["machine"]["macos"]["major"], major, "{name}");
    }
}

#[test]
fn counts_match_the_units_the_analysis_lists() {
    let scratch = Scratch::new("counts");
    for name in [BOTH, HOMEBREW_ONLY, MACPORTS_ONLY, TAPPED, NEITHER, BROKEN] {
        let a = json_ok(&scratch, name, &["analyze"]);
        let listed = units(&a).len() as u64;
        let top = a["summary"]["topLevelUnits"].as_u64().expect("count");
        let dep = a["summary"]["dependencyUnits"].as_u64().expect("count");
        assert_eq!(
            top + dep,
            listed,
            "{name}: summary does not count its units"
        );
        assert_eq!(dep, 0, "{name}: dependency units are hidden in normal mode");
        let by_outcome: u64 = a["summary"]["byOutcome"]
            .as_object()
            .expect("byOutcome")
            .values()
            .filter_map(Value::as_u64)
            .sum();
        assert_eq!(by_outcome, listed, "{name}: byOutcome does not add up");
    }

    // The empty fixtures are empty for two different reasons, and both are
    // reported rather than failing.
    for name in [NEITHER, BROKEN] {
        let a = json_ok(&scratch, name, &["analyze"]);
        assert!(units(&a).is_empty(), "{name} has no package units");
    }
    let both = json_ok(&scratch, BOTH, &["analyze"]);
    assert!(
        both["providers"]["macports"]["packageCount"]
            .as_u64()
            .unwrap_or(0)
            > 0,
        "the both-providers fixture holds MacPorts packages"
    );
}

// ---------------------------------------------------------------------------
// Mapping and coexistence
// ---------------------------------------------------------------------------

#[test]
fn the_both_providers_fixture_has_a_package_held_by_both_providers() {
    // `htop` is installed under both providers in this fixture, with a STRONG
    // mapping (a shared normalised name and a shared homepage). That is the
    // coexistence case of INV-18, and KEEP_BOTH_HEALTHY is the rule that
    // recognises it.
    //
    // The outcome is KEEP_BOTH: an unknown archive availability on a side
    // that already holds the package blocks nothing (SE-039), so the
    // availability rule stays quiet and coexistence wins (INV-18).
    let scratch = Scratch::new("keepboth");
    let a = json_ok(&scratch, BOTH, &["analyze"]);
    let htop = unit(&a, "htop");

    assert!(
        htop["rows"]["homebrew"]["presence"] == "INSTALLED"
            && htop["rows"]["macports"]["presence"] == "INSTALLED",
        "htop is installed under both providers: {}",
        htop["rows"]
    );
    assert_eq!(htop["mappings"][0]["confidence"], "STRONG");
    assert!(
        rules_fired(htop).contains(&"KEEP_BOTH_HEALTHY".to_string()),
        "KEEP_BOTH_HEALTHY did not fire for htop: {:?}",
        rules_fired(htop)
    );
    let coexistence = htop["recommendation"]["reasons"]
        .as_array()
        .expect("reasons")
        .iter()
        .find(|r| r["rule"] == "KEEP_BOTH_HEALTHY")
        .expect("the coexistence reason");
    assert_eq!(coexistence["id"], "reason.present_under_both_providers");
    assert_eq!(coexistence["params"]["other"], "macports");
    assert_eq!(
        htop["recommendation"]["outcome"]["kind"], "KEEP_BOTH",
        "a package held by both healthy providers is a coexistence outcome (INV-18)"
    );
}

#[test]
fn a_curated_equivalence_maps_exactly_and_both_sides_are_reported() {
    let scratch = Scratch::new("exact");
    let a = json_ok(&scratch, BOTH, &["analyze"]);
    let python = unit(&a, "python@3.12");
    assert_eq!(python["mappings"][0]["confidence"], "EXACT");
    assert_eq!(
        python["mappings"][0]["selected"]["package"]["name"],
        "python312"
    );
    assert_eq!(python["rows"]["macports"]["name"], "python312");
    assert_eq!(python["rows"]["macports"]["presence"], "INSTALLED");
}

#[test]
fn a_formula_with_two_equally_scored_ports_is_ambiguous_and_selects_neither() {
    // `node` scores `nodejs22` and `nodejs24` identically: both share
    // nodejs.org as their homepage and neither normalises to `node`, so no
    // evidence separates them. AMBIGUOUS is a first-class result, not a
    // failure, and nothing is selected.
    let scratch = Scratch::new("ambiguous");
    let a = json_ok(&scratch, BOTH, &["analyze"]);
    let node = unit(&a, "node");
    let mapping = &node["mappings"][0];
    assert_eq!(mapping["confidence"], "AMBIGUOUS");
    assert!(
        mapping.get("selected").is_none() || mapping["selected"].is_null(),
        "an ambiguous mapping selects nothing: {mapping}"
    );
    let candidates: Vec<&str> = mapping["candidates"]
        .as_array()
        .expect("candidates")
        .iter()
        .filter_map(|c| c["package"]["name"].as_str())
        .collect();
    assert_eq!(candidates, ["nodejs22", "nodejs24"], "{mapping}");
}

#[test]
fn a_package_with_no_counterpart_maps_to_none() {
    let scratch = Scratch::new("none");
    let a = json_ok(&scratch, HOMEBREW_ONLY, &["analyze"]);
    let ghidra = unit(&a, "ghidra");
    assert_eq!(ghidra["mappings"][0]["confidence"], "NONE");
    assert!(ghidra["rows"].get("macports").is_none());
}

#[test]
fn the_derived_fixture_carries_the_three_provider_states_the_capture_lacked() {
    // A deprecated formula, a disabled formula and a formula from a non-core
    // tap: docs/spec/07-quality-and-testing.md asks the matrix to cover all
    // three, and the captured machine only had the first.
    let scratch = Scratch::new("tapped");
    let a = json_ok(&scratch, TAPPED, &["analyze"]);
    assert_eq!(
        unit(&a, "gemini-cli")["rows"]["homebrew"]["availability"]["availability"]["state"],
        "DEPRECATED"
    );
    assert_eq!(
        unit(&a, "bvi")["rows"]["homebrew"]["availability"]["availability"]["state"],
        "DISABLED"
    );
    let codeburn = unit(&a, "codeburn");
    assert!(
        codeburn["class"]["classes"]
            .as_array()
            .expect("classes")
            .contains(&Value::from("THIRD_PARTY_SOURCED")),
        "a non-core tap makes a formula third-party sourced: {}",
        codeburn["class"]
    );
}

#[test]
fn both_artifact_answers_are_present_on_the_apple_silicon_fixture() {
    // The fixture was derived so that most formulae carry an arm64 tag the
    // machine accepts and a named handful keep only their Intel release tag.
    let scratch = Scratch::new("artifacts");
    let a = json_ok(&scratch, BOTH, &["analyze"]);
    let mut prebuilt = 0;
    let mut source_build = 0;
    for u in units(&a) {
        let state = &u["rows"]["homebrew"]["availability"]["availability"]["state"];
        if state == "BINARY_AVAILABLE" {
            prebuilt += 1;
        } else if state == "SOURCE_BUILD_REQUIRED" {
            source_build += 1;
        }
    }
    assert!(prebuilt > 0, "no prebuilt artifact on the arm64 fixture");
    assert!(source_build > 0, "no source build on the arm64 fixture");
}

// ---------------------------------------------------------------------------
// INV-13: normal mode is about user intent
// ---------------------------------------------------------------------------

#[test]
fn invariant_13_dependency_units_appear_only_with_advanced() {
    let scratch = Scratch::new("inv13");
    let normal = json_ok(&scratch, BOTH, &["analyze"]);
    assert!(
        units(&normal).iter().all(|u| u["advancedOnly"] == false),
        "normal mode listed a dependency unit"
    );
    assert_eq!(normal["summary"]["dependencyUnits"], 0);

    // `--advanced` prints the safety notice INV-13 exists for, on stderr, in
    // every mode. That is a documented notice, not output: stdout stays the
    // JSON document, which is what `json_run` still asserts.
    let advanced = json_run(&scratch, BOTH, &["--advanced", "analyze"], true);
    let hidden = units(&advanced)
        .iter()
        .filter(|u| u["advancedOnly"] == true)
        .count();
    assert!(hidden > 0, "--advanced revealed no dependency unit");
    assert_eq!(
        advanced["summary"]["dependencyUnits"].as_u64().unwrap_or(0),
        hidden as u64
    );
    assert!(units(&advanced).len() > units(&normal).len());
}

// ---------------------------------------------------------------------------
// Exit codes (06-cli-and-ux.md)
// ---------------------------------------------------------------------------

#[test]
fn invariant_19_a_stateful_package_stops_the_plan_at_manual_review() {
    // `postgresql@17` is stateful on both sides (a curated family, INV-19).
    // 4 is the safety-check code: the plan exists and says it stops.
    let scratch = Scratch::new("inv19");
    let path = fixture(BOTH);
    let out = run(
        &scratch,
        &[
            "--replay",
            path.to_str().expect("path"),
            "--json",
            "plan",
            "postgresql@17",
            "--to",
            "macports",
            "--today",
            TODAY,
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(4),
        "stateful package must stop the plan"
    );
    let plan: Value = serde_json::from_slice(&out.stdout).expect("plan JSON");
    assert_eq!(plan["schemaVersion"], 1);
}

#[test]
fn planning_toward_a_provider_that_is_not_installed_stops_at_manual_review() {
    let scratch = Scratch::new("absent-destination");
    let path = fixture(HOMEBREW_ONLY);
    let out = run(
        &scratch,
        &[
            "--replay",
            path.to_str().expect("path"),
            "--json",
            "plan",
            "ghidra",
            "--to",
            "macports",
            "--today",
            TODAY,
        ],
    );
    assert_eq!(out.status.code(), Some(4));
    assert!(out.stderr.is_empty());
}

#[test]
fn doctor_exits_five_on_a_broken_provider_and_zero_elsewhere() {
    let scratch = Scratch::new("doctor");
    for (name, code) in [
        (BROKEN, 5),
        (BOTH, 0),
        (HOMEBREW_ONLY, 0),
        (MACPORTS_ONLY, 0),
        (NEITHER, 0),
    ] {
        let path = fixture(name);
        let out = run(
            &scratch,
            &[
                "--replay",
                path.to_str().expect("path"),
                "--json",
                "doctor",
                "--today",
                TODAY,
            ],
        );
        assert_eq!(out.status.code(), Some(code), "{name}: doctor exit code");
        let doc: Value = serde_json::from_slice(&out.stdout).expect("doctor JSON");
        assert_eq!(doc["schemaVersion"], 1, "{name}");
    }
}

#[test]
fn an_unknown_flag_is_a_usage_error() {
    let scratch = Scratch::new("usage");
    let out = run(&scratch, &["--json", "--not-a-flag", "status"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        out.stdout.is_empty(),
        "a usage error writes no JSON to stdout"
    );
}

#[test]
fn a_snapshot_from_an_unsupported_schema_is_refused() {
    let scratch = Scratch::new("schema");
    let path = scratch.path().join("future.json");
    std::fs::write(&path, b"{\"schemaVersion\": 99}\n").expect("write");
    let out = run(
        &scratch,
        &[
            "--json",
            "--from-snapshot",
            path.to_str().expect("path"),
            "analyze",
        ],
    );
    assert_eq!(out.status.code(), Some(8), "snapshot-unusable is exit 8");
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty(), "the refusal is explained on stderr");
}

// ---------------------------------------------------------------------------
// The two binaries, and the two determinism invariants
// ---------------------------------------------------------------------------

#[test]
fn mpb_is_the_same_program_under_a_shorter_name() {
    // Compared on the `--from-snapshot` path, where nothing is derived from the
    // clock, so "byte-identical" means byte-identical with no qualification.
    let scratch = Scratch::new("mpb");
    let snapshot = scratch.path().join("snapshot.json");
    let path = fixture(BOTH);
    let captured = run(
        &scratch,
        &[
            "--replay",
            path.to_str().expect("path"),
            "snapshot",
            "--out",
            snapshot.to_str().expect("path"),
            "--today",
            TODAY,
        ],
    );
    assert_eq!(captured.status.code(), Some(0));

    let args = [
        "--json",
        "--from-snapshot",
        snapshot.to_str().expect("path"),
        "status",
        "--today",
        TODAY,
    ];
    let long = run_bin(BIN, &scratch, &args);
    let short = run_bin(MPB, &scratch, &args);
    assert_eq!(long.status.code(), Some(0));
    assert_eq!(short.status.code(), Some(0));
    assert_eq!(
        long.stdout, short.stdout,
        "mpb and macportsbrew must produce the same bytes"
    );
    assert!(long.stderr.is_empty() && short.stderr.is_empty());
}

#[test]
fn invariant_15_reproducible_recommendations_end_to_end() {
    let scratch = Scratch::new("inv15");
    for name in [BOTH, HOMEBREW_ONLY, MACPORTS_ONLY] {
        let path = fixture(name);
        let snapshot = scratch.path().join(format!("{name}.snapshot.json"));

        // The invariant's own enforcement: the same snapshot file, analysed
        // twice, is byte for byte the same document. Nothing is normalised.
        let captured = run(
            &scratch,
            &[
                "--replay",
                path.to_str().expect("path"),
                "snapshot",
                "--out",
                snapshot.to_str().expect("path"),
                "--today",
                TODAY,
            ],
        );
        assert_eq!(captured.status.code(), Some(0), "{name}: snapshot");

        let args = [
            "--json",
            "--from-snapshot",
            snapshot.to_str().expect("path"),
            "analyze",
            "--today",
            TODAY,
        ];
        let first = run(&scratch, &args);
        let second = run(&scratch, &args);
        assert_eq!(first.status.code(), Some(0), "{name}");
        assert_eq!(
            first.stdout, second.stdout,
            "{name}: two --from-snapshot analyses are not byte-identical"
        );

        // The replay path is the same computation over a snapshot captured
        // twice. The captures differ in their timestamp and in nothing else,
        // so the analyses are compared with that one field replaced. If any
        // other field moved, this fails.
        let replay_args = [
            "--replay",
            path.to_str().expect("path"),
            "--json",
            "analyze",
            "--today",
            TODAY,
        ];
        let a = run(&scratch, &replay_args);
        let b = run(&scratch, &replay_args);
        assert_eq!(a.status.code(), Some(0), "{name}");
        assert_eq!(
            without_capture_time(&a.stdout),
            without_capture_time(&b.stdout),
            "{name}: two --replay analyses differ in more than the capture time"
        );

        // And the snapshot round trip is the same analysis as the replay,
        // modulo where it says it came from.
        let mut replayed = without_capture_time(&a.stdout);
        let mut from_file = without_capture_time(&first.stdout);
        for v in [&mut replayed, &mut from_file] {
            let obj = v.as_object_mut().expect("object");
            obj.remove("snapshotProvenance");
            obj.insert("source".into(), Value::from("<source>"));
        }
        assert_eq!(
            replayed, from_file,
            "{name}: analysing the snapshot file differs from analysing the replay"
        );
    }
}

#[test]
fn invariant_16_idempotent_read_commands() {
    let scratch = Scratch::new("inv16");
    let path = fixture(BOTH);
    let path = path.to_str().expect("path");

    for command in ["status", "analyze", "list", "doctor"] {
        let args = ["--replay", path, "--json", command, "--today", TODAY];
        let first = run(&scratch, &args);
        let second = run(&scratch, &args);
        assert_eq!(first.status.code(), second.status.code(), "{command}");
        assert_eq!(
            without_capture_time(&first.stdout),
            without_capture_time(&second.stdout),
            "{command}: a second run produced a different answer"
        );
    }

    // A read command writes nothing anywhere, and the state directory is the
    // one place it plausibly could.
    let state = scratch.state_dir();
    let created: Vec<PathBuf> = std::fs::read_dir(&state)
        .map(|entries| entries.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(
        created.is_empty(),
        "read commands created {created:?} under MACPORTSBREW_STATE_DIR"
    );
}

#[test]
fn json_goes_to_stdout_and_nothing_else_does() {
    let scratch = Scratch::new("streams");
    let path = fixture(BOTH);
    let path = path.to_str().expect("path");
    for command in [
        vec!["status"],
        vec!["analyze"],
        vec!["list"],
        vec!["doctor"],
        vec!["explain", "htop"],
    ] {
        let mut argv = vec!["--replay", path, "--json"];
        argv.extend(command.iter().copied());
        argv.extend_from_slice(&["--today", TODAY]);
        let out = run(&scratch, &argv);
        assert!(
            out.stderr.is_empty(),
            "{argv:?} wrote to stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<Value>(&out.stdout)
            .unwrap_or_else(|e| panic!("{argv:?} stdout is not a single JSON document: {e}"));
    }

    // `snapshot` is the documented exception: the path it wrote is a notice,
    // and notices go to stderr so the JSON on stdout stays machine-readable.
    let snapshot = scratch.path().join("s.json");
    let out = run(
        &scratch,
        &[
            "--replay",
            path,
            "--json",
            "snapshot",
            "--out",
            snapshot.to_str().expect("path"),
            "--today",
            TODAY,
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(!out.stderr.is_empty(), "the written path is announced");
    serde_json::from_slice::<Value>(&out.stdout).expect("stdout is still the snapshot JSON");
}

#[test]
fn every_installed_package_is_reported_somewhere() {
    // The assertion the suite never had, and the one that would have caught a
    // package disappearing. Making the unit merge defer to the provider whose
    // executable resolves is right, but mappings are scored per direction and
    // are not symmetric, so the other side does not always claim the row back -
    // and an installed, user-requested port was then reported nowhere at all,
    // in either mode, while `plan` said it was not installed under any
    // provider. Tidiness is not worth a package vanishing.
    //
    // Stated over the inventory rather than over the units, because that is the
    // direction the defect ran: units looked fine, one input had no output.
    let scratch = Scratch::new("completeness");
    for name in [BOTH, HOMEBREW_ONLY, MACPORTS_ONLY, TAPPED, BROKEN, NEITHER] {
        let advanced = json_run(&scratch, name, &["analyze", "--advanced"], true);
        let reported: std::collections::BTreeSet<(String, String)> = units(&advanced)
            .iter()
            .flat_map(|u| {
                u["rows"].as_object().into_iter().flat_map(|rows| {
                    rows.iter().filter_map(|(provider, row)| {
                        row["installed"].as_array().filter(|i| !i.is_empty())?;
                        Some((provider.clone(), row["name"].as_str()?.to_string()))
                    })
                })
            })
            .collect();
        for (provider, p) in advanced["providers"].as_object().expect("providers") {
            if p["detection"]["presence"]["state"] != "PRESENT" {
                continue;
            }
            for row in p["inventory"]["installed"].as_array().into_iter().flatten() {
                let installed = row["identity"]["name"].as_str().unwrap_or_default();
                assert!(
                    reported.contains(&(provider.clone(), installed.to_string())),
                    "{name}: {provider} has {installed} installed and the analysis \
                     reports it under no unit, as neither a unit nor a row"
                );
            }
        }
    }
}

#[test]
fn invariant_13_advanced_mode_only_ever_adds_to_the_top_level_view() {
    // One assertion, and it is the one that was missing. The suite asserted
    // only that advanced mode yields *more* units, which stayed true while
    // `--advanced` was quietly removing a user-requested package from the
    // top-level set and flipping its source with it - the foundation pass
    // recorded that as a count that "differs by one … Known, not fixed".
    //
    // The property that actually holds: visibility is applied to finished
    // units, so the top-level set is identical in both modes and advanced mode
    // only reveals dependencies (SE-048).
    let scratch = Scratch::new("advanced-additive");
    for name in [BOTH, HOMEBREW_ONLY, MACPORTS_ONLY, TAPPED] {
        let plain = json_ok(&scratch, name, &["analyze"]);
        // `--advanced` writes its documented notice to stderr, which is the
        // point of notices being on stderr at all.
        let advanced = json_run(&scratch, name, &["analyze", "--advanced"], true);

        let top = |v: &Value| -> Vec<String> {
            units(v)
                .iter()
                .filter(|u| u["advancedOnly"] != true)
                .map(|u| u["name"].as_str().unwrap_or_default().to_string())
                .collect()
        };
        assert_eq!(
            top(&plain),
            top(&advanced),
            "{name}: the top-level set must not change with a visibility flag"
        );
        assert!(
            units(&advanced).len() >= units(&plain).len(),
            "{name}: advanced mode reveals, it does not hide"
        );
        // And the unit's identity is the same in both, not only its presence.
        for u in units(&plain) {
            let n = u["name"].as_str().unwrap_or_default();
            let same = units(&advanced)
                .iter()
                .find(|a| a["name"] == u["name"])
                .unwrap_or_else(|| panic!("{name}: {n} vanished under --advanced"));
            assert_eq!(u["source"], same["source"], "{name}: {n} changed source");
        }
    }
}

#[test]
fn invariant_15_a_replayed_or_snapshotted_run_never_redacts_against_this_host() {
    // The redaction policy is chosen in `Context::build`, and nothing else
    // observes the choice — so without this the mapping could be flipped from
    // `Recorded` to `Unresolved` and no test would notice: no fixture contains a
    // `/Users/<name>` string, so the shape redaction is a no-op over all six
    // recordings and every golden file stays byte-identical. That is a gate
    // blind to exactly the regression this policy makes possible.
    //
    // `AS_CAPTURED` is the observable proof that a replayed or snapshotted run
    // redacts nothing against *this* host (INV-15). It is deliberately not
    // `GROUNDED`: neither a recording nor a snapshot records how it was redacted
    // when it was captured, and one captured with no resolvable home was
    // redacted heuristically.
    let scratch = Scratch::new("redaction-policy");
    let fixture_path = fixture(HOMEBREW_ONLY);
    let path = fixture_path.to_str().expect("fixture path is UTF-8");

    let replayed = scratch.path().join("replayed.json");
    let out = run(
        &scratch,
        &[
            "--replay",
            path,
            "doctor",
            "--report",
            replayed.to_str().expect("path"),
            "--today",
            TODAY,
        ],
    );
    assert!(
        out.status.code() == Some(0) || out.status.code() == Some(5),
        "doctor exits 0, or 5 on a broken provider; got {:?}",
        out.status.code()
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(&replayed).expect("the report was written"))
            .expect("the report is JSON");
    assert_eq!(
        report["redaction"], "AS_CAPTURED",
        "a --replay run must not redact against this host"
    );

    // The same for a snapshot file, which reaches `Context::build` by a
    // different branch and returns before a machine is ever built.
    let snapshot = scratch.path().join("s.json");
    let out = run(
        &scratch,
        &[
            "--replay",
            path,
            "snapshot",
            "--out",
            snapshot.to_str().expect("path"),
            "--today",
            TODAY,
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let from_snapshot = scratch.path().join("from-snapshot.json");
    let out = run(
        &scratch,
        &[
            "--from-snapshot",
            snapshot.to_str().expect("path"),
            "doctor",
            "--report",
            from_snapshot.to_str().expect("path"),
            "--today",
            TODAY,
        ],
    );
    assert!(
        out.status.code() == Some(0) || out.status.code() == Some(5),
        "got {:?}",
        out.status.code()
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(&from_snapshot).expect("the report was written"))
            .expect("the report is JSON");
    assert_eq!(
        report["redaction"], "AS_CAPTURED",
        "a --from-snapshot run must not redact against this host"
    );
}

#[test]
fn every_fixture_matches_its_committed_golden_analysis() {
    // The golden files are the committed proof of INV-15 across the matrix.
    // scripts/run-fixture-matrix.sh --update regenerates them; this fails when
    // a change moved an answer without the golden file being updated.
    let scratch = Scratch::new("golden");
    let dir = root().join("fixtures/recordings");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("fixtures/recordings")
        .filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            (p.extension().and_then(|s| s.to_str()) == Some("json"))
                .then(|| p.file_stem()?.to_str().map(str::to_string))
                .flatten()
        })
        .collect();
    names.sort();
    assert!(names.len() >= 6, "the matrix has at least six fixtures");

    for name in names {
        let produced = json_ok(&scratch, &name, &["analyze"]);
        let golden_path = root()
            .join("fixtures/golden")
            .join(format!("{name}.analyze.json"));
        let golden: Value = serde_json::from_slice(
            &std::fs::read(&golden_path)
                .unwrap_or_else(|e| panic!("{}: {e}", golden_path.display())),
        )
        .expect("golden JSON");

        let mut produced = produced;
        if let Some(obj) = produced
            .get_mut("snapshotProvenance")
            .and_then(Value::as_object_mut)
        {
            obj.insert("capturedAt".into(), Value::from("<captured-at>"));
        }
        assert_eq!(
            produced, golden,
            "{name}: the analysis differs from its golden file; \
             run scripts/run-fixture-matrix.sh --update if the change is intended"
        );
    }
}
