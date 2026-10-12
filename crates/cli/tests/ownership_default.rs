// SPDX-License-Identifier: Apache-2.0

//! An ownership answer that was a default says so (pass 12, item C).
//!
//! No captured recording lacks the reverse-dependency capability, and fixtures
//! are captured, never invented, so the defaulted case is made the way
//! `end_to_end.rs` makes its other degraded inputs: a snapshot captured from a
//! recording, with one capability turned into an unknown. The control is the
//! same snapshot untouched.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");
const FIXTURE: &str = "homebrew-only-intel-macos26";
const TODAY: &str = "2026-09-02";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mpb-ownership-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(state: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(root())
        .env("MACPORTSBREW_STATE_DIR", state)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env_remove("MACPORTSBREW_LANG")
        .output()
        .unwrap_or_else(|e| panic!("spawning {BIN}: {e}"))
}

/// A captured snapshot, as JSON, and a Homebrew dependency in it (a package
/// its own record says nobody asked for).
fn captured(dir: &Path) -> (Value, String) {
    let file = dir.join("captured.json");
    let out = run(
        dir,
        &[
            "--replay",
            root()
                .join(format!("fixtures/recordings/{FIXTURE}.json"))
                .to_str()
                .unwrap(),
            "snapshot",
            "--out",
            file.to_str().unwrap(),
            "--today",
            TODAY,
        ],
    );
    assert_eq!(out.status.code(), Some(0), "capturing the snapshot");
    let doc: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    let dependency = doc["providers"]["homebrew"]["inventory"]["installed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["requested"] == Value::Bool(false))
        .and_then(|row| row["identity"]["name"].as_str())
        .expect("the fixture has a package nobody asked for")
        .to_string();
    (doc, dependency)
}

fn explain(dir: &Path, doc: &Value, label: &str, package: &str, json: bool) -> Output {
    let file = dir.join(format!("{label}.json"));
    std::fs::write(&file, serde_json::to_vec(doc).unwrap()).unwrap();
    let mut args = vec!["--from-snapshot", file.to_str().unwrap(), "--advanced"];
    if json {
        args.push("--json");
    }
    args.extend(["explain", package, "--today", TODAY]);
    run(dir, &args)
}

#[test]
fn a_dependency_reached_by_default_is_marked_in_explain_and_in_json() {
    let dir = scratch("marked");
    let (doc, dependency) = captured(&dir);

    // The spelling the degraded cases below overwrite really is there, answered.
    assert_eq!(
        doc["providers"]["homebrew"]["capabilities"]["capabilities"]["REVERSE_DEPENDENCIES"]
            ["state"],
        "AVAILABLE"
    );

    // Control: the capability was answered, so the split was a finding.
    let control = explain(&dir, &doc, "control", &dependency, true);
    let control: Value = serde_json::from_slice(&control.stdout).expect("JSON");
    assert!(
        control["package"].get("ownershipSplitDefaulted").is_none(),
        "an answered split must carry no marker: {control}"
    );

    // The capability unknown: the same package is still a dependency, and now
    // says that was the default.
    let mut unread = doc.clone();
    unread["providers"]["homebrew"]["capabilities"]["capabilities"]["REVERSE_DEPENDENCIES"] =
        serde_json::json!({ "state": "UNKNOWN" });
    let json = explain(&dir, &unread, "unread-json", &dependency, true);
    let json: Value = serde_json::from_slice(&json.stdout).expect("JSON");
    assert_eq!(
        json["package"]["ownershipSplitDefaulted"],
        serde_json::json!(["homebrew"])
    );
    assert_eq!(
        json["package"]["rows"]["homebrew"]["ownership"], "DEPENDENCY",
        "the marker explains the label; it does not change it"
    );

    let human = explain(&dir, &unread, "unread-human", &dependency, false);
    let human = String::from_utf8_lossy(&human.stdout);
    assert!(
        human.contains("treated as a dependency by default"),
        "explain must say the answer was a default: {human}"
    );
    let control_human = explain(&dir, &doc, "control-human", &dependency, false);
    assert!(
        !String::from_utf8_lossy(&control_human.stdout).contains("by default"),
        "an answered split must not say it was a default"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_package_somebody_asked_for_is_never_marked_whatever_is_unread() {
    let dir = scratch("asked");
    let (doc, _) = captured(&dir);
    let mut unread = doc.clone();
    unread["providers"]["homebrew"]["capabilities"]["capabilities"]["REVERSE_DEPENDENCIES"] =
        serde_json::json!({ "state": "UNKNOWN" });
    let out = explain(&dir, &unread, "asked", "wget", true);
    let json: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert_eq!(
        json["package"]["rows"]["homebrew"]["ownership"],
        "TOP_LEVEL"
    );
    assert!(json["package"].get("ownershipSplitDefaulted").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
