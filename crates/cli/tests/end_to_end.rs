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
fn an_unknown_ownership_is_shown_in_the_default_view_and_not_counted_as_requested() {
    // DP-012, decided by the owner 2026-09-05, and until now held only by
    // golden files - which the next fixture recapture would rewrite without
    // anyone noticing the decision had been reversed.
    //
    // Two halves. A package whose ownership is `UNKNOWN` is **shown**: it is
    // not a package known to be a dependency, and hiding it would make normal
    // mode quietest about the rows nobody has established anything about,
    // which is the opposite of INV-13. And it is **not counted** as one the
    // provider's own record claims, because that is what `topLevelCount`
    // means.
    let scratch = Scratch::new("unknown-ownership");
    let a = json_ok(&scratch, HOMEBREW_ONLY, &["analyze"]);

    // Ownership is per provider row, not per unit: one unit can be recorded by
    // one provider and unknown to another.
    let listed = units(&a);
    let unknown: Vec<&Value> = listed
        .iter()
        .filter(|u| {
            u["rows"].as_object().is_some_and(|rows| {
                rows.values()
                    .any(|r| r["ownershipEvidence"]["mechanism"] == "no_provider_record")
            })
        })
        .collect();
    assert!(
        !unknown.is_empty(),
        "this fixture must carry an UNKNOWN ownership or the assertions below \
         prove nothing - the casks in it have no receipt under replay"
    );
    for unit in &unknown {
        assert_eq!(
            unit["advancedOnly"], false,
            "an UNKNOWN ownership was hidden behind --advanced: {}",
            unit["name"]
        );
        for row in unit["rows"].as_object().expect("rows").values() {
            if row["ownershipEvidence"]["mechanism"] != "no_provider_record" {
                // `not_installed` is the other route to `UNKNOWN` and is a
                // different fact: the package is not there at all.
                continue;
            }
            assert_eq!(
                row["ownership"], "UNKNOWN",
                "an absent record produced something other than UNKNOWN"
            );
            assert_eq!(
                row["ownershipEvidence"]["authoritative"], false,
                "an absent record was reported as authoritative"
            );
        }
    }

    let counted: u64 = a["providers"]["homebrew"]["topLevelCount"]
        .as_u64()
        .expect("topLevelCount");
    let shown: u64 = a["summary"]["topLevelUnits"]
        .as_u64()
        .expect("topLevelUnits");
    assert!(
        counted < shown,
        "topLevelCount ({counted}) must be narrower than topLevelUnits ({shown}) \
         while any unit is UNKNOWN - they mean different things (DP-012)"
    );
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

    // The subset property F-19 asks for is not asserted here, and deliberately:
    // `invariant_13_advanced_mode_only_ever_adds_to_the_top_level_view` already
    // does it, across four fixtures, comparing the top-level sets themselves and
    // each unit's `source`. Restating it here would be a second copy to keep
    // true. What this test owns is the count and the flag.
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
fn a_broken_provider_is_explained_to_a_person_without_naming_an_identifier() {
    // F-38's second half. `ProviderPresence::Broken` carries a `reason`, and
    // every construction of it - both adapters and `capture::broken` - sets that
    // to an `ErrorId::*.message_id()`. It is an identifier, never prose. The
    // human template used to interpolate it, so a user diagnosing a broken
    // install read "macports is present but not working: error.provider_unhealthy".
    //
    // Both halves are asserted here, and the second is the one that matters:
    // without it, someone reads "the message no longer uses reason" and deletes
    // the parameter, silently breaking a contract `docs/schemas/doctor.v1.md`
    // documents. A machine wants the stable identifier; a person does not.
    let scratch = Scratch::new("broken-reason");
    let path = fixture(BROKEN);
    let args = |json: bool| {
        let mut v = vec!["--replay", path.to_str().expect("path")];
        if json {
            v.push("--json");
        }
        v.extend(["doctor", "--today", TODAY]);
        v
    };

    let human = run(&scratch, &args(false));
    let text = String::from_utf8(human.stdout).expect("utf-8");
    assert!(
        text.contains("not working"),
        "the fixture must actually reach the broken branch: {text}"
    );
    assert!(
        !text.contains("error."),
        "a message identifier reached the user: {text}"
    );

    let out = run(&scratch, &args(true));
    let doc: Value = serde_json::from_slice(&out.stdout).expect("doctor JSON");
    let broken = doc["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .find(|f| f["id"] == "provider_broken")
        .expect("the fixture is the broken one");
    assert_eq!(
        broken["reason"]["params"]["reason"], "error.provider_unhealthy",
        "the JSON contract keeps the identifier the human text drops: {broken}"
    );
}

#[test]
fn a_bad_recording_is_not_diagnosed_as_a_bad_snapshot() {
    // F-41. `--from-snapshot` and `--replay` share one bounded reader (F-30),
    // and that unification preserved a message written for only one of them:
    // every failure was `SnapshotUnusable`, so pointing `--replay` at a
    // directory told the user to run `macportsbrew snapshot` and aim
    // `--from-snapshot` - a command that cannot produce a recording, and a flag
    // they had not used.
    //
    // The exit code is asserted as *unchanged* on purpose. A new identifier
    // that quietly moved an input error off 8 would break the contract in
    // `06-cli-and-ux.md` while looking like a wording fix.
    let scratch = Scratch::new("bad-recording");
    let dir = scratch.path().join("not-a-file");
    std::fs::create_dir_all(&dir).expect("a directory is unreadable as either");
    let arg = dir.to_str().expect("path").to_string();

    for (flag, wanted, unwanted) in [
        ("--replay", "capture-fixture", "--from-snapshot"),
        (
            "--from-snapshot",
            "macportsbrew snapshot",
            "capture-fixture",
        ),
    ] {
        let out = run(&scratch, &[flag, &arg, "status"]);
        assert_eq!(
            out.status.code(),
            Some(8),
            "{flag}: an unreadable input still exits 8"
        );
        assert!(
            out.stdout.is_empty(),
            "{flag}: an unreadable input produces no document, not a partial one"
        );
        let text = String::from_utf8(out.stderr).expect("utf-8");
        assert!(
            text.contains(wanted),
            "{flag} must name the remedy that produces its own input: {text}"
        );
        assert!(
            !text.contains(unwanted),
            "{flag} must not send the user to the other route: {text}"
        );
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

    // A *real* snapshot with its version patched, not `{"schemaVersion": 99}`.
    // That document is refused by the parse arm — `toolVersion` and the rest are
    // required with no default — so it never reaches the version check, and this
    // test asserted exit 8 and a non-empty stderr over a line it never executed.
    // Both are true of the parse failure too, which is what made it a green
    // light over an uncovered line (F-43).
    let clean = scratch.path().join("clean.json");
    let captured = run(
        &scratch,
        &[
            "--replay",
            fixture(BOTH).to_str().expect("path"),
            "--json",
            "snapshot",
            "--out",
            clean.to_str().expect("path"),
            "--today",
            TODAY,
        ],
    );
    assert_eq!(captured.status.code(), Some(0), "capturing the snapshot");

    let real = std::fs::read_to_string(&clean).expect("read");
    let mut doc: Value = serde_json::from_str(&real).expect("the capture is JSON");
    doc["schemaVersion"] = serde_json::json!(99);
    let path = scratch.path().join("future.json");
    std::fs::write(&path, doc.to_string().as_bytes()).expect("write");

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
    // The whole phrase, not just "99": the parse arm quotes the file path, and a
    // scratch directory that happened to contain those digits would satisfy a
    // bare `contains("99")` from the wrong arm. The parse arm's message for the
    // old document was "missing field `toolVersion`", which this cannot match.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("schema version 99 is not the supported"),
        "this must reach the version check, not the parse: {stderr}"
    );

    // And the untampered capture loads, or the case above proves nothing.
    // `--today` is pinned like every other run in this file: without it the date
    // comes from the snapshot's `capturedAt`, which capture writes from the real
    // clock whatever `--today` said, so this control would start emitting a
    // staleness notice on whatever day `data/policy`'s newest `read_on` ages out.
    let ok = run(
        &scratch,
        &[
            "--json",
            "--from-snapshot",
            clean.to_str().expect("path"),
            "analyze",
            "--today",
            TODAY,
        ],
    );
    assert_eq!(
        ok.status.code(),
        Some(0),
        "the untampered snapshot must load"
    );
}

/// F-43 item 1 at the boundary: `check` also runs on a capture.
///
/// F-41 gave `--replay` its own identifier in the reader. `Snapshot::check` is
/// the other place a replayed run can be refused, because a capture runs it and
/// a capture under `--replay` is built from a recording - so this run used to
/// answer "Take a fresh one with `macportsbrew snapshot`, or point
/// --from-snapshot at a readable file" to a user who had done neither. A live
/// capture got the same sentence, and it was wrong there too; only
/// `--from-snapshot` was ever being told the truth, and that route keeps
/// `SnapshotUnusable`.
///
/// Reproduced by answering `brew --prefix` with the other provider's prefix,
/// which is the lever F-21's replay half already pulls.
#[test]
fn a_replayed_run_is_not_sent_after_a_snapshot_it_never_had() {
    let scratch = Scratch::new("f43");
    let mut recording: Value =
        serde_json::from_slice(&std::fs::read(fixture(BOTH)).expect("fixture")).expect("JSON");
    let mut patched = 0;
    for invocation in recording["invocations"]
        .as_array_mut()
        .expect("invocations")
    {
        if invocation["program"] == serde_json::json!("brew")
            && invocation["argv"] == serde_json::json!(["--prefix"])
        {
            invocation["stdout"] = serde_json::json!("/opt/local\n");
            patched += 1;
        }
    }
    assert_eq!(patched, 1, "the fixture must still record `brew --prefix`");

    let doctored = scratch.path().join("overlap.json");
    std::fs::write(
        &doctored,
        serde_json::to_vec(&recording).expect("serialise"),
    )
    .expect("write");
    let out = run(
        &scratch,
        &[
            "--replay",
            doctored.to_str().expect("path"),
            "--json",
            "analyze",
            "--today",
            TODAY,
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(8),
        "an overlapping prefix is refused"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("overlaps"),
        "the refusal must still say what is wrong: {stderr}"
    );
    // The point of the finding is not which identifier fires, but that the
    // remediation does not send this user after a file and a flag they never used.
    for route in ["macportsbrew snapshot", "--from-snapshot"] {
        assert!(
            !stderr.contains(route),
            "a --replay run must not be sent after {route}: {stderr}"
        );
    }
    // And that it still says something. Without this, an empty remediation - or
    // a regression to the bare detail - passes the two checks above.
    assert!(
        stderr.contains("macportsbrew doctor"),
        "the neutral remediation must still be rendered: {stderr}"
    );
}

/// F-43 item 1 at the planner, which is not a second line but the designed
/// enforcement point - and is reachable by an ordinary machine.
///
/// I filed this as unreachable, having fed a snapshot a *relative* prefix and
/// watched the boundary refuse it. That was the wrong probe.
/// `snapshot::prefix_shape_error` says in its own comment that a `~`-rooted
/// prefix is "allowed *here* and refused where it matters", because it is an
/// absolute path with the home elided by redaction - the privacy machinery
/// exists so a home-installed provider can be captured at all - and that
/// `planner::command` is what refuses it. So the boundary passes `~/homebrew`,
/// `analyze` works, and `plan` stops at `planner.rs`. `security-reviewer` found
/// this and reproduced it; the claim had been written into four places.
///
/// No boundary refusal can produce this message, which is what makes the
/// assertion a test of the planner rather than of the exit code. The F-09 test
/// asserts exit 8, which the boundary already produces, so it could never have
/// told the two apart.
#[test]
fn a_home_redacted_prefix_is_refused_by_the_planner_and_not_by_the_boundary() {
    let scratch = Scratch::new("f43b");
    let clean = scratch.path().join("clean.json");
    assert_eq!(
        run(
            &scratch,
            &[
                "--replay",
                fixture(BOTH).to_str().expect("path"),
                "--json",
                "snapshot",
                "--out",
                clean.to_str().expect("path"),
                "--today",
                TODAY,
            ],
        )
        .status
        .code(),
        Some(0),
        "capturing the clean snapshot"
    );

    let mut doc: Value =
        serde_json::from_slice(&std::fs::read(&clean).expect("read")).expect("JSON");
    // Both, because the boundary requires the `PATH` leader to be the detected
    // prefix (INV-10): a home-installed provider's snapshot is coherent, not
    // half-edited, and editing only one field tests the boundary rather than the
    // planner.
    doc["providers"]["homebrew"]["detection"]["prefix"] = serde_json::json!("~/homebrew");
    let path = doc["providers"]["homebrew"]["invocationEnvironment"]["PATH"]
        .as_str()
        .expect("a PATH");
    let rest = path
        .split_once(':')
        .expect("more than one entry")
        .1
        .to_string();
    doc["providers"]["homebrew"]["invocationEnvironment"]["PATH"] =
        serde_json::json!(format!("~/homebrew/bin:{rest}"));
    let home_rooted = scratch.path().join("home-rooted.json");
    std::fs::write(&home_rooted, serde_json::to_vec(&doc).expect("serialise")).expect("write");

    let analyze = run(
        &scratch,
        &[
            "--json",
            "--from-snapshot",
            home_rooted.to_str().expect("path"),
            "analyze",
            "--today",
            TODAY,
        ],
    );
    assert_eq!(
        analyze.status.code(),
        Some(0),
        "the boundary allows a `~`-rooted prefix on purpose, or this test proves nothing"
    );

    let planned = run(
        &scratch,
        &[
            "--json",
            "--from-snapshot",
            home_rooted.to_str().expect("path"),
            "plan",
            "htop",
            "--to",
            "homebrew",
        ],
    );
    assert_eq!(planned.status.code(), Some(8), "the planner refuses it");
    let stderr = String::from_utf8_lossy(&planned.stderr);
    assert!(
        stderr.contains("is not an absolute program"),
        "this must be the planner's refusal, which no boundary check emits: {stderr}"
    );
}

/// F-21: a snapshot must not be able to state the environment a provider
/// command runs in.
///
/// The finding's second half was that no test could see it — the INV-10 plan
/// test writes an environment into its own fixture and the planner clones it,
/// so it never crosses a boundary. This one crosses both: `--from-snapshot`,
/// where the environment is read from a file, and `--replay`, where it is
/// *constructed* from a recording that decides what `brew --prefix` answers.
/// The second was the surprise. "Constructed" was taken to mean "safe", and a
/// review showed it does not: `sanitised_environment` never sees the other
/// provider, so it cannot notice that this one has moved on top of it.
///
/// Every case runs against both providers, because a guard that only holds one
/// way is not a guard (INV-17) — an earlier version alternated them by index
/// and, as it turned out, exercised no rule in both directions.
#[test]
fn invariant_10_a_snapshot_cannot_dictate_a_provider_command_environment() {
    let scratch = Scratch::new("f21");
    let clean = scratch.path().join("clean.json");
    let out = run(
        &scratch,
        &[
            "--replay",
            fixture(BOTH).to_str().expect("path"),
            "--json",
            "snapshot",
            "--out",
            clean.to_str().expect("path"),
            "--today",
            TODAY,
        ],
    );
    assert_eq!(out.status.code(), Some(0), "capturing the clean snapshot");

    let load = |file: &Path| {
        run(
            &scratch,
            &[
                "--json",
                "--from-snapshot",
                file.to_str().expect("path"),
                "analyze",
            ],
        )
    };
    let base: Value = serde_json::from_slice(&std::fs::read(&clean).expect("read"))
        .expect("the captured snapshot is JSON");
    assert_eq!(
        load(&clean).status.code(),
        Some(0),
        "the untampered snapshot must still load, or this test proves nothing"
    );

    let write = |doc: &Value, name: &str| {
        let file = scratch.path().join(name);
        std::fs::write(&file, serde_json::to_vec(doc).expect("serialise")).expect("write");
        file
    };

    for (id, own, other) in [
        ("homebrew", "/opt/homebrew", "/opt/local"),
        ("macports", "/opt/local", "/opt/homebrew"),
    ] {
        let ok_path = format!("{own}/bin:/usr/bin:/bin:/usr/sbin:/sbin");
        // Each case names a fragment the refusal must contain. Without that a
        // case can be refused for another case's reason and the test stays
        // green — which is how the first version of this test hid the fact
        // that four of its cases were tripping the same rule.
        let cases: Vec<(&str, Value, &str)> = vec![
            (
                "a variable that runs a program of the attacker's choosing",
                serde_json::json!({"PATH": ok_path, "SUDO_ASKPASS": "/tmp/evil/askpass.sh"}),
                "SUDO_ASKPASS",
            ),
            (
                "a variable that injects code into the provider's own runtime",
                serde_json::json!({"PATH": ok_path, "RUBYOPT": "-r/tmp/evil.rb"}),
                "RUBYOPT",
            ),
            (
                "a plausible-looking provider variable that is still not on the list",
                serde_json::json!({"PATH": ok_path, "HOMEBREW_ARTIFACT_DOMAIN": "http://elsewhere.example"}),
                "HOMEBREW_ARTIFACT_DOMAIN",
            ),
            (
                "a dynamic linker injection",
                serde_json::json!({"PATH": ok_path, "DYLD_INSERT_LIBRARIES": "/tmp/x.dylib"}),
                "DYLD_INSERT_LIBRARIES",
            ),
            (
                "a build flag",
                serde_json::json!({"PATH": ok_path, "CPPFLAGS": format!("-I{own}/include")}),
                "CPPFLAGS",
            ),
            (
                "a pinned variable given another value",
                serde_json::json!({"PATH": ok_path, "LANG": "en_US.UTF-8"}),
                "LANG",
            ),
            (
                "a PATH entry reaching through `..`",
                serde_json::json!({"PATH": format!("/opt/../{}/bin:{ok_path}", other.trim_start_matches('/'))}),
                "..",
            ),
            (
                "a writable directory first on PATH",
                serde_json::json!({"PATH": format!("/tmp/evil/bin:{ok_path}")}),
                "not a directory it was detected at",
            ),
            (
                "the other provider's prefix on PATH",
                serde_json::json!({"PATH": format!("{other}/bin:{ok_path}")}),
                "is under",
            ),
            (
                "a real path that is nobody's prefix",
                serde_json::json!({"PATH": format!("/private{own}/bin:{ok_path}")}),
                "not a directory it was detected at",
            ),
            (
                "an empty PATH entry",
                serde_json::json!({"PATH": format!("{own}/bin::{ok_path}")}),
                "empty, relative",
            ),
            (
                "a relative PATH entry",
                serde_json::json!({"PATH": format!(".:{ok_path}")}),
                "empty, relative",
            ),
            (
                "no PATH at all",
                serde_json::json!({"LANG": "C"}),
                "states no PATH",
            ),
            (
                "a home of the snapshot's choosing",
                serde_json::json!({"PATH": ok_path, "HOME": "/tmp/evil"}),
                "HOME=",
            ),
            (
                "a PATH that does not end with the system directories",
                serde_json::json!({"PATH": format!("{own}/bin")}),
                "does not end with the system directories",
            ),
            (
                "a pinned variable left out",
                serde_json::json!({"PATH": ok_path}),
                "states no",
            ),
        ];

        for (what, mut environment, names) in cases {
            // Every case but the two that are *about* the pinned variables
            // carries them, so each reaches the rule it is written for rather
            // than tripping an earlier one - which is how an earlier version of
            // this test came to exercise four cases against one rule.
            if !matches!(names, "LANG" | "states no") {
                for (key, value) in [
                    ("LANG", "C"),
                    ("LC_ALL", "C"),
                    ("TERM", "dumb"),
                    ("NO_COLOR", "1"),
                ] {
                    environment[key] = serde_json::json!(value);
                }
                // And whatever the adapter itself adds, which the reader now
                // requires by value and not only by name.
                if id == "homebrew" {
                    for key in [
                        "HOMEBREW_NO_ANALYTICS",
                        "HOMEBREW_NO_AUTO_UPDATE",
                        "HOMEBREW_NO_COLOR",
                        "HOMEBREW_NO_EMOJI",
                        "HOMEBREW_NO_ENV_HINTS",
                        "HOMEBREW_NO_INSTALL_CLEANUP",
                    ] {
                        environment[key] = serde_json::json!("1");
                    }
                }
            }
            let mut doc = base.clone();
            doc["providers"][id]["invocationEnvironment"] = environment;
            let out = load(&write(&doc, "tampered.json"));
            assert_eq!(
                out.status.code(),
                Some(8),
                "{id}: {what} must be refused as an unusable snapshot"
            );
            assert!(
                out.stdout.is_empty(),
                "{id}: {what}: a refusal prints no analysis"
            );
            let stderr = String::from_utf8_lossy(&out.stderr);
            assert!(
                stderr.contains(names),
                "{id}: {what}: the refusal should name {names}, said: {stderr}"
            );
        }

        // The prefix a provider declares widens what it may put on its `PATH`,
        // so it is not taken on trust: claiming the other provider's is how a
        // tampered recording would have laundered a leak through the
        // *constructed* route.
        let mut doc = base.clone();
        doc["providers"][id]["detection"]["prefix"] = serde_json::json!(other);
        doc["providers"][id]["invocationEnvironment"] =
            serde_json::json!({"PATH": format!("{other}/bin:/usr/bin:/bin:/usr/sbin:/sbin")});
        let out = load(&write(&doc, "claimed.json"));
        assert_eq!(
            out.status.code(),
            Some(8),
            "{id} claiming {other} must be refused"
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("overlaps"),
            "{id}: the refusal names whose prefix it overlaps"
        );
    }

    // A case-folded prefix. The boot volume is case-insensitive, so `/OPT/Local`
    // and `/opt/local` are one directory while a byte comparison calls them two -
    // the same class as `..`, the string and the kernel disagreeing.
    let mut folded = base.clone();
    folded["providers"]["macports"]["detection"]["prefix"] = serde_json::json!("/OPT/Homebrew");
    folded["providers"]["macports"]["invocationEnvironment"] = serde_json::json!({
        "PATH": "/OPT/Homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
        "LANG": "C", "LC_ALL": "C", "TERM": "dumb", "NO_COLOR": "1"});
    let out = load(&write(&folded, "folded.json"));
    assert_eq!(
        out.status.code(),
        Some(8),
        "a case-folded prefix is the same prefix"
    );

    // A prefix reaching through `..`, which would otherwise become a program.
    let mut dotdot = base.clone();
    dotdot["providers"]["homebrew"]["detection"]["prefix"] =
        serde_json::json!("/opt/homebrew/../../tmp/evil");
    assert_eq!(
        load(&write(&dotdot, "dotdot.json")).status.code(),
        Some(8),
        "a prefix reaching through `..` is not a usable directory"
    );

    // `/usr/local` is a prefix the registry knows for Homebrew, and
    // `/usr/local/bin` is group-writable on every Mac - `doctor` calls that an
    // error in its own right. A Homebrew detected at `/opt/homebrew` may not
    // have it on the PATH just because the registry knows the name.
    let mut known = base.clone();
    known["providers"]["homebrew"]["invocationEnvironment"] = serde_json::json!({
        "PATH": "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
        "LANG": "C", "LC_ALL": "C", "TERM": "dumb", "NO_COLOR": "1"});
    assert_eq!(
        load(&write(&known, "known.json")).status.code(),
        Some(8),
        "a known prefix is not the detected one"
    );

    // A prefix shallow enough to contain other things switches the containment
    // rule off entirely: `/opt` makes the other provider's whole tree "mine",
    // and `/` makes everything mine. This was a working bypass of the rule the
    // whole check exists to enforce, found by review after two rebuilds.
    for (id, prefix, entry) in [
        ("homebrew", "/opt", "/opt/local/bin"),
        ("homebrew", "/", "/tmp/evil/bin"),
        ("homebrew", "/usr", "/usr/local/bin"),
        ("macports", "/opt", "/opt/homebrew/bin"),
    ] {
        let mut shallow = base.clone();
        shallow["providers"][id]["detection"]["prefix"] = serde_json::json!(prefix);
        let mut environment = serde_json::json!({
            "PATH": format!("{entry}:/usr/bin:/bin:/usr/sbin:/sbin"),
            "LANG": "C", "LC_ALL": "C", "TERM": "dumb", "NO_COLOR": "1"});
        if id == "homebrew" {
            for key in [
                "HOMEBREW_NO_ANALYTICS",
                "HOMEBREW_NO_AUTO_UPDATE",
                "HOMEBREW_NO_COLOR",
                "HOMEBREW_NO_EMOJI",
                "HOMEBREW_NO_ENV_HINTS",
                "HOMEBREW_NO_INSTALL_CLEANUP",
            ] {
                environment[key] = serde_json::json!("1");
            }
        }
        shallow["providers"][id]["invocationEnvironment"] = environment;
        assert_eq!(
            load(&write(&shallow, "shallow.json")).status.code(),
            Some(8),
            "{id} declaring {prefix} must not thereby own {entry}"
        );
    }

    // A quieting variable turned off, or dropped. `HOMEBREW_NO_AUTO_UPDATE` is
    // what makes a read-only command read-only, and permitting the key by name
    // rather than by value let a snapshot set it to "0" while the rule claimed
    // to assert what the builder emits.
    for (what, mutate) in [
        (
            "a quieting variable turned off",
            serde_json::json!({"HOMEBREW_NO_AUTO_UPDATE": "0"}),
        ),
        (
            "a quieting variable dropped",
            serde_json::json!({"HOMEBREW_NO_AUTO_UPDATE": null}),
        ),
    ] {
        let mut quiet = base.clone();
        let environment = quiet["providers"]["homebrew"]["invocationEnvironment"]
            .as_object_mut()
            .expect("environment object");
        for (key, value) in mutate.as_object().expect("object") {
            if value.is_null() {
                environment.remove(key);
            } else {
                environment.insert(key.clone(), value.clone());
            }
        }
        assert_eq!(
            load(&write(&quiet, "quiet.json")).status.code(),
            Some(8),
            "{what} must be refused"
        );
    }

    // A provider the file calls present while stating no prefix. Every provider
    // this tool detects reports where it was found, so this is not a document it
    // wrote - and honouring it would hand back the registry's whole list as
    // "its own", which is the narrowing the containment rule exists for.
    let mut headless = base.clone();
    headless["providers"]["homebrew"]["detection"]["prefix"] = Value::Null;
    headless["providers"]["homebrew"]["invocationEnvironment"]["PATH"] =
        serde_json::json!("/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    assert_eq!(
        load(&write(&headless, "headless.json")).status.code(),
        Some(8),
        "a present provider states where it was found"
    );

    // Two providers claiming the same place. Neither is a registry candidate,
    // so only the other provider's own claim can catch it.
    let mut both = base.clone();
    for id in ["homebrew", "macports"] {
        both["providers"][id]["detection"]["prefix"] = serde_json::json!("~/shared");
        both["providers"][id]["invocationEnvironment"] = serde_json::json!({
            "PATH": "~/shared/bin:/usr/bin:/bin:/usr/sbin:/sbin",
            "LANG": "C", "LC_ALL": "C", "TERM": "dumb", "NO_COLOR": "1"});
    }
    assert_eq!(
        load(&write(&both, "both.json")).status.code(),
        Some(8),
        "two providers cannot both own one directory"
    );

    // And two shapes that must keep loading, because refusing a document this
    // tool wrote is the failure mode a check like this actually causes.
    let mut home_install = base.clone();
    home_install["providers"]["homebrew"]["detection"]["prefix"] = serde_json::json!("~/homebrew");
    home_install["providers"]["homebrew"]["invocationEnvironment"] = serde_json::json!({
        "PATH": "~/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
        "LANG": "C", "LC_ALL": "C", "TERM": "dumb", "NO_COLOR": "1",
        "HOMEBREW_NO_ANALYTICS": "1", "HOMEBREW_NO_AUTO_UPDATE": "1",
        "HOMEBREW_NO_COLOR": "1", "HOMEBREW_NO_EMOJI": "1",
        "HOMEBREW_NO_ENV_HINTS": "1", "HOMEBREW_NO_INSTALL_CLEANUP": "1"});
    assert_eq!(
        load(&write(&home_install, "home.json")).status.code(),
        Some(0),
        "a Homebrew installed under $HOME is redacted to `~`, and must still load"
    );

    let mut broken = base.clone();
    broken["providers"]["macports"]["detection"]["presence"] =
        serde_json::json!({"state": "BROKEN", "reason": "error.provider_unhealthy"});
    broken["providers"]["macports"]["detection"]["prefix"] = Value::Null;
    broken["providers"]["macports"]
        .as_object_mut()
        .expect("provider object")
        .remove("invocationEnvironment");
    assert_eq!(
        load(&write(&broken, "broken.json")).status.code(),
        Some(0),
        "a provider whose capture failed states no environment, and `capture::broken` \
         writes exactly that - refusing it would reject the snapshot taken on the \
         broken machine someone is asking for help with"
    );
}

/// The other half of F-21, and the one that was missed: a *constructed*
/// environment is not automatically a safe one.
///
/// `sanitised_environment` builds Homebrew's `PATH` by prepending
/// `<prefix>/bin`, and the prefix is whatever `brew --prefix` answered - which
/// under `--replay` comes from the recording, and on a live run comes from the
/// provider, which the threat model treats as untrusted either way. So a file
/// can move one provider on top of the other without ever touching an
/// `invocationEnvironment`. Nothing objected before this check ran on captured
/// snapshots too.
#[test]
fn invariant_10_a_recording_cannot_move_one_provider_onto_the_other() {
    let scratch = Scratch::new("f21-replay");
    let mut recording: Value =
        serde_json::from_slice(&std::fs::read(fixture(BOTH)).expect("read")).expect("recording");
    let mut patched = 0;
    for invocation in recording["invocations"]
        .as_array_mut()
        .expect("invocations")
        .iter_mut()
    {
        let is_prefix_query = invocation["argv"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v == "--prefix"));
        if is_prefix_query {
            invocation["stdout"] = serde_json::json!("/opt/local\n");
            patched += 1;
        }
    }
    assert!(patched > 0, "the recording must contain a prefix query");
    let file = scratch.path().join("evil-recording.json");
    std::fs::write(&file, serde_json::to_vec(&recording).expect("serialise")).expect("write");

    let out = run(
        &scratch,
        &[
            "--json",
            "--replay",
            file.to_str().expect("path"),
            "analyze",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(8),
        "a recording that makes one provider claim the other's prefix is unusable"
    );
    assert!(out.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("overlaps"),
        "the refusal names whose prefix it overlaps"
    );
}

/// F-09: `--from-snapshot` was an unvalidated source of plan argv.
///
/// The adapters apply `is_safe_name` / `checked_name` to everything a provider
/// says, and the planner turns names into argv elements and executable names
/// without passing any allow-list — because the allow-lists live in the
/// adapters, and a snapshot never met one. "Here is a snapshot from my machine,
/// what should I migrate?" is a workflow the documentation invites; a package in
/// that file called `--force` is a flag the executor would hand to `brew`.
///
/// Both providers, and every field that reaches a command: the package name, the
/// dependency edges, the resolved executables, and the paths.
#[test]
fn a_snapshot_cannot_turn_provider_metadata_into_a_flag_or_a_path() {
    let scratch = Scratch::new("f09");
    let clean = scratch.path().join("clean.json");
    assert_eq!(
        run(
            &scratch,
            &[
                "--replay",
                fixture(BOTH).to_str().expect("path"),
                "--json",
                "snapshot",
                "--out",
                clean.to_str().expect("path"),
                "--today",
                TODAY,
            ],
        )
        .status
        .code(),
        Some(0),
    );
    let base: Value = serde_json::from_slice(&std::fs::read(&clean).expect("read")).expect("json");
    let load = |doc: &Value, name: &str| {
        let file = scratch.path().join(name);
        std::fs::write(&file, serde_json::to_vec(doc).expect("serialise")).expect("write");
        run(
            &scratch,
            &[
                "--json",
                "--from-snapshot",
                file.to_str().expect("path"),
                "plan",
                "wget",
                "--to",
                "macports",
            ],
        )
    };
    assert_ne!(
        load(&base, "clean.json").status.code(),
        Some(8),
        "the untampered snapshot must not be refused, or this test proves nothing"
    );

    for id in ["homebrew", "macports"] {
        for hostile in [
            "--force",
            "-o/etc/anything",
            "../../../bin/sh",
            "foo bar",
            "foo;rm -rf /",
            "",
        ] {
            let mut doc = base.clone();
            let inventory = &mut doc["providers"][id]["inventory"];
            let previous = inventory["installed"][0]["identity"]["name"]
                .as_str()
                .expect("a name")
                .to_string();
            inventory["installed"][0]["identity"]["name"] = serde_json::json!(hostile);
            for map in ["metadata", "availability"] {
                if let Some(object) = inventory[map].as_object_mut() {
                    if let Some(value) = object.remove(&previous) {
                        object.insert(hostile.to_string(), value);
                    }
                }
            }
            let out = load(&doc, "hostile.json");
            assert_eq!(
                out.status.code(),
                Some(8),
                "{id}: a package named {hostile:?} must not reach a command"
            );
            assert!(out.stdout.is_empty(), "{id}: {hostile:?} produced a plan");
        }

        // The same names by the other routes into a command.
        let mut edge = base.clone();
        edge["providers"][id]["inventory"]["dependencies"]
            .as_array_mut()
            .expect("dependencies")
            .push(serde_json::json!({"from": "--force", "to": "wget", "buildOnly": false}));
        assert_eq!(
            load(&edge, "edge.json").status.code(),
            Some(8),
            "{id}: a dependency edge is a name too"
        );

        // `detection.executable` is where the provider's own binary was found.
        // It is printed and never run - the planner builds a program from the
        // prefix and the verb table - so `..` in it is not a defect, and
        // refusing it would be the strict rule applied where it does not belong.
        // Escape codes are refused, because it reaches a terminal.
        let mut executable = base.clone();
        executable["providers"][id]["detection"]["executable"] =
            serde_json::json!("/opt/homebrew/../bin/brew");
        assert_ne!(
            load(&executable, "executable.json").status.code(),
            Some(8),
            "{id}: a reported executable path is not a program"
        );
        executable["providers"][id]["detection"]["executable"] =
            serde_json::json!("/opt/homebrew/bin/\u{1b}[31mbrew\u{1b}[0m");
        assert_eq!(
            load(&executable, "painted-executable.json").status.code(),
            Some(8),
            "{id}: but it is printed, so it carries no escape codes"
        );

        // The prefix is the one field that becomes a command's `program`, and
        // it was the field the first version of this fix left out - while
        // validating `detection.executable`, which no command uses. A provider
        // whose capture failed is skipped by the environment rule, and can
        // still be the source side of a plan, so these reached a `REMOVE_SOURCE`
        // step that elevates.
        for hostile in [
            "relative/evil",
            ".",
            "/opt/../tmp/evil",
            "/tmp/\u{1b}[31mPWNED\u{1b}[0m",
            "/usr",
        ] {
            let mut doc = base.clone();
            doc["providers"][id]["detection"]["presence"] = serde_json::json!({"state": "ABSENT"});
            doc["providers"][id]["detection"]["prefix"] = serde_json::json!(hostile);
            doc["providers"][id]
                .as_object_mut()
                .expect("provider")
                .remove("invocationEnvironment");
            assert_eq!(
                load(&doc, "prefix.json").status.code(),
                Some(8),
                "{id}: a prefix of {hostile:?} becomes the program of every command"
            );
        }
    }

    // Words `port` reads as a set of ports. A provider-neutral predicate can say
    // "not a flag"; it cannot say "not a keyword", so the list is data with a
    // source, and the guard sits where the argv is built.
    for word in ["installed", "all", "INACTIVE"] {
        let mut doc = base.clone();
        for id in ["homebrew", "macports"] {
            let inventory = &mut doc["providers"][id]["inventory"];
            let previous = inventory["installed"][0]["identity"]["name"]
                .as_str()
                .expect("a name")
                .to_string();
            inventory["installed"][0]["identity"]["name"] = serde_json::json!(word);
            for map in ["metadata", "availability"] {
                if let Some(object) = inventory[map].as_object_mut() {
                    if let Some(value) = object.remove(&previous) {
                        object.insert(word.to_string(), value);
                    }
                }
            }
        }
        let file = scratch.path().join("pseudo.json");
        std::fs::write(&file, serde_json::to_vec(&doc).expect("serialise")).expect("write");
        let out = run(
            &scratch,
            &[
                "--json",
                "--from-snapshot",
                file.to_str().expect("path"),
                "plan",
                word,
                "--to",
                "macports",
            ],
        );
        assert_ne!(
            out.status.code(),
            Some(0),
            "a package named {word:?} must not become an argument"
        );
    }

    // What reaches a reader is bounded and printable; what becomes a command is
    // held to the stricter rule. Getting that split wrong in either direction is
    // a defect: escape codes painted into `doctor`'s output on one side, and a
    // refused capture on any machine whose `PATH` contains `.../bin/../bin` on
    // the other, because `which` answers with the entry as written.
    let mut painted = base.clone();
    painted["environment"]["path"]
        .as_array_mut()
        .expect("path")
        .insert(0, serde_json::json!("/tmp/\u{1b}[31mPWNED\u{1b}[0m/bin"));
    assert_eq!(
        load(&painted, "painted.json").status.code(),
        Some(8),
        "a snapshot must not paint a warning in someone's terminal"
    );

    let resolution = |name: &str, path: &str| {
        serde_json::json!({name: {"executable": name, "path": path,
                                  "owner": {"owner": "SYSTEM"}, "authoritative": false}})
    };
    let mut indirect = base.clone();
    indirect["providers"]["homebrew"]["resolutions"] =
        resolution("wget", "/usr/local/bin/../bin/wget");
    assert_ne!(
        load(&indirect, "indirect.json").status.code(),
        Some(8),
        "a resolution is where `PATH` said something was, not a program"
    );
    let mut flagged = base.clone();
    flagged["providers"]["homebrew"]["resolutions"] = resolution("--force", "/usr/local/bin/wget");
    assert_eq!(
        load(&flagged, "flagged.json").status.code(),
        Some(8),
        "the executable name in a resolution is a name"
    );

    // A `~`-rooted prefix is a redacted absolute path, so it reads; but it
    // cannot become a program, because `Command::new` would resolve it against
    // the working directory. Refused where it matters, not where it is read.
    let mut home_prefix = base.clone();
    home_prefix["providers"]["macports"]["detection"]["prefix"] = serde_json::json!("~/evil");
    home_prefix["providers"]["macports"]["invocationEnvironment"]["PATH"] =
        serde_json::json!("~/evil/bin:/usr/bin:/bin:/usr/sbin:/sbin");
    let file = scratch.path().join("home-prefix.json");
    std::fs::write(&file, serde_json::to_vec(&home_prefix).expect("serialise")).expect("write");
    assert_eq!(
        run(
            &scratch,
            &[
                "--json",
                "--from-snapshot",
                file.to_str().expect("path"),
                "analyze",
            ],
        )
        .status
        .code(),
        Some(0),
        "a redacted home prefix is readable"
    );
    assert_ne!(
        run(
            &scratch,
            &[
                "--json",
                "--from-snapshot",
                file.to_str().expect("path"),
                "plan",
                "wget",
                "--to",
                "macports",
            ],
        )
        .status
        .code(),
        Some(0),
        "but it cannot become the program of a command"
    );

    // And the shapes a correct machine reports that earlier versions of this
    // rule refused outright.
    let mut variant = base.clone();
    variant["providers"]["macports"]["inventory"]["installed"][0]["variants"] =
        serde_json::json!(["-x11", "+universal"]);
    assert_ne!(
        load(&variant, "variant.json").status.code(),
        Some(8),
        "MacPorts records a deselected variant with its sign (R-3 E-1)"
    );
    // The same field, filled by the other provider with something else
    // entirely: Homebrew copies a keg's `used_options`, which are option flags.
    // A predicate designed from the MacPorts shape refused these at *capture*,
    // so the tool declined the machine in front of it.
    variant["providers"]["homebrew"]["inventory"]["installed"][0]["variants"] =
        serde_json::json!(["--with-openssl", "--HEAD", "--c++11"]);
    assert_ne!(
        load(&variant, "options.json").status.code(),
        Some(8),
        "Homebrew records a keg's used_options in the same field"
    );
    let mut shallow = base.clone();
    shallow["providers"]["homebrew"]["detection"]["version"] =
        serde_json::json!(">=4.3.0 (shallow or no git repository)");
    assert_ne!(
        load(&shallow, "shallow.json").status.code(),
        Some(8),
        "Homebrew answers this when installed from the .pkg or a shallow clone"
    );
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
fn invariant_06_a_read_is_never_summarised_as_needing_administrator_rights() {
    // Finding F-10, reproduced exactly as it was observed: the plan for
    // `htop --to macports` ended "Steps that would need administrator rights:
    // 2, 3", where step 3 is `port -q installed htop` - a read. It said that
    // because `verify` had no entry in `[entry.elevation]` and the planner maps
    // a missing key to `Elevation::Unknown`, which the summary then rendered as
    // "would need administrator rights".
    //
    // Two things fixed it. The elevation is stated in the data rather than
    // reached by omission; and where the answer is not known, the summary says
    // so instead of saying you will be asked for a password. Under INV-06 the
    // harm was never the fold toward caution, it was training someone to
    // approve a prompt for a read.
    //
    // **Updated 2026-09-06.** This test used to assert that step 3 carried
    // `elevation unknown`, because MacPorts was not installed on the machine
    // the research was done on and its documentation did not settle it. It is
    // installed now: `port -q installed` runs as a non-root user and exits 0,
    // the registry is world-readable, and the data says `none` with that
    // observation as its source (F-24). So the honest answer changed, and the
    // assertion has to change with it or it asserts the old world.
    //
    // **This test is the rendering check and nothing more.** An earlier version
    // of this rewrite claimed it asserted INV-06 "more strongly than before".
    // That was false, and `spec-guardian` proved it by mutation: flipping one
    // provider's `verify` cell to `required` makes the binary print "Steps that
    // need administrator rights: 3, 6" with step 3 a read, and every test in
    // the repository stayed green. The invariant itself is now asserted where
    // it is a property of the plan rather than of a string - in
    // `macportsbrew-application`, `invariant_06_a_step_whose_job_is_to_read_
    // never_elevates`, which fails under that mutation. What remains here is
    // that the *renderer* does not turn a non-elevating read into a password
    // prompt, and that a genuine elevation is still reported.
    //
    // One thing was lost and is recorded rather than papered over: with the
    // last reachable `unknown` gone, the renderer's "Steps where it is not
    // known" branch is no longer reachable through any plan, so nothing
    // exercises it. Manufacturing an `unknown` to keep that green would test a
    // path no user can reach. See F-24's closure note and F-35.
    let scratch = Scratch::new("f10-elevation");
    let path = fixture("both-providers-arm64-macos15");
    let path = path.to_str().expect("fixture path is UTF-8");
    let out = run(
        &scratch,
        &[
            "--replay", path, "plan", "htop", "--to", "macports", "--today", TODAY,
        ],
    );
    let text = String::from_utf8_lossy(&out.stdout);

    let verify = text
        .lines()
        .find(|l| l.contains("-q installed htop"))
        .expect("the verify step names its command");
    assert!(
        !verify.contains("elevation required"),
        "a read must never be annotated as elevating: {verify}"
    );

    // Deliberately *not* `summary.is_empty()`. That was this rewrite's second
    // mistake: this direction happens to have no elevating step only because
    // htop is already installed under MacPorts in this fixture, so
    // ADD_DESTINATION emits no command at all. Recapture the fixture with htop
    // absent and the destination's add verb appears, `required` with it, and a
    // test named after INV-06 fails on a plan that is entirely correct. The
    // property is about the verify step's own annotation, asserted above.

    // And the other direction, because the branch that reports a *real*
    // elevation was rewritten by the same change and nothing asserted it. A bug
    // that emptied `required` - an inverted `matches!`, an empty-command guard -
    // would hide a genuine administrator prompt, which is the INV-06-relevant
    // direction of failure, and the assertions above would still pass: they see
    // one summary line either way. Running both directions also exercises
    // INV-17: the same code renders both, and only the data differs.
    let out = run(
        &scratch,
        &[
            "--replay", path, "plan", "htop", "--to", "homebrew", "--today", TODAY,
        ],
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let reverse: Vec<&str> = text
        .lines()
        .filter(|l| l.contains("administrator rights"))
        .collect();
    // Exactly one line here, and it is the other one. Removing from MacPorts
    // elevates; the Homebrew destination's `verify` is a cited `none`, and the
    // removal-verification step carries no command at all - so nothing in this
    // direction is unknown. Asserting both lines appear was this test's first
    // shape and it was wrong, which is the useful kind of wrong: the assertion
    // was about what I expected rather than what the data says.
    assert_eq!(
        reverse.len(),
        1,
        "one summary line in this direction: {reverse:?}"
    );
    // The **exact** line, not "starts with the right words and contains some
    // digit". Those two assertions passed unchanged under the mutation that
    // added a read to this list, because "3, 6" starts identically and still
    // contains a digit. Step 6 is REMOVE_SOURCE and is the only step in this
    // direction that genuinely elevates; naming it is what makes the assertion
    // able to fail.
    assert_eq!(
        reverse[0].trim(),
        "Steps that need administrator rights: 6.",
        "only the removal elevates in this direction; any other step appearing \
         here is a read being presented as a password prompt (INV-06)"
    );
}

#[test]
fn no_rust_identifier_reaches_the_reader() {
    // Five sites rendered a domain enumeration with `{:?}`, so `SnapshotBefore`
    // and `ProviderReportsActive` reached a person as written in the source -
    // seen by no catalogue and marked by no pseudolocale (F-20, which listed
    // three of the five).
    //
    // The property, not the strings: no CamelCase variant name survives into
    // human output. That catches a sixth site nobody has written yet, which a
    // list of expected sentences would not.
    let scratch = Scratch::new("no-debug-render");
    let path = fixture(BOTH);
    let path = path.to_str().expect("fixture path is UTF-8");
    let mut seen = 0usize;
    for args in [
        vec!["plan", "htop", "--to", "macports"],
        vec!["status"],
        vec!["explain", "htop"],
    ] {
        let mut argv = vec!["--replay", path];
        argv.extend_from_slice(&args);
        argv.extend_from_slice(&["--today", TODAY]);
        let out = run(&scratch, &argv);
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(!text.is_empty(), "{args:?} printed nothing");
        seen += 1;
        for name in [
            "SnapshotBefore",
            "AddDestination",
            "VerifyDestination",
            "RemoveSource",
            "ManualReview",
            "ProviderReportsActive",
            "LivenessProbe",
            "NoCrossProviderLinkage",
            "PackageAbsent",
            "ExecutablesOwnedByDestination",
        ] {
            assert!(
                !text.contains(name),
                "{args:?} showed the reader the Rust identifier `{name}`"
            );
        }
    }
    assert_eq!(seen, 3, "all three surfaces were exercised");
}

#[test]
fn the_status_table_renders_through_the_catalogue() {
    // Both columns were built in code: the tier as
    // `format!("{:?}", tier).to_ascii_lowercase()` and the count as
    // `"{} ({} top-level)"` (F-20). Nothing asserted either, so swapping two
    // catalogue keys or reverting to `{:?}` would pass clippy, pass the parity
    // check - the keys exist and are referenced either way - and pass the suite.
    let scratch = Scratch::new("status-render");
    let path = fixture(BOTH);
    let path = path.to_str().expect("fixture path is UTF-8");
    let out = run(&scratch, &["--replay", path, "status", "--today", TODAY]);
    let text = String::from_utf8_lossy(&out.stdout);

    assert!(
        text.contains("Supported"),
        "the tier comes from the catalogue, capitalised as it writes it: {text}"
    );
    assert!(
        !text.contains(" supported ") && !text.lines().any(|l| l.contains("  supported")),
        "and not lower-cased from the enum: {text}"
    );
    assert!(
        text.contains("packages (") && text.contains("top-level)"),
        "the count is one catalogue string with a plural rule: {text}"
    );
    // The plural rule is the catalogue's, not a format string's: a provider
    // holding one package says "package", not "1 packages".
    assert!(
        !text.contains("1 packages"),
        "the plural rule did not apply: {text}"
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
