// SPDX-License-Identifier: Apache-2.0

//! A provider's own configuration, end to end (DP-027).
//!
//! A replay cannot carry configuration files - its answer is `UNKNOWABLE` - so
//! the found case is driven from a snapshot file: one is written from a
//! recording, a captured configuration is put into it the way capture would,
//! and `doctor` and `plan` are run on it. Nothing touches a live installation
//! (INV-05).

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");
const RECORDING: &str = "both-providers-arm64-macos15";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "macportsbrew-provider-configuration-{label}-{}-{:?}",
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

fn run(scratch: &Scratch, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .args(["--today", "2026-09-02"])
        .current_dir(root())
        .env("MACPORTSBREW_STATE_DIR", scratch.0.join("state"))
        .env_remove("MACPORTSBREW_LANG")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .unwrap_or_else(|e| panic!("spawning {BIN}: {e}"))
}

/// The recording's snapshot, with `configuration` set on `provider`.
fn snapshot_with(scratch: &Scratch, provider: &str, configuration: Value) -> String {
    let path = scratch.0.join("snapshot.json");
    let recording = root().join(format!("fixtures/recordings/{RECORDING}.json"));
    let out = run(
        scratch,
        &[
            "--replay",
            recording.to_str().expect("UTF-8"),
            "snapshot",
            "--out",
            path.to_str().expect("UTF-8"),
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut doc: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("the snapshot")).expect("JSON");
    assert_eq!(
        doc["providers"][provider]["configuration"],
        json!({ "state": "UNKNOWABLE" }),
        "a replay records that it could not know"
    );
    doc["providers"][provider]["configuration"] = configuration;
    std::fs::write(&path, serde_json::to_vec_pretty(&doc).expect("JSON")).expect("write");
    path.to_str().expect("UTF-8").to_string()
}

fn found(keys: &[&str]) -> Value {
    json!({
        "state": "READ",
        "files": [{
            "path": "~/.homebrew/brew.env",
            "scope": "USER",
            "readable": true,
            "keys": keys,
        }]
    })
}

#[test]
fn doctor_names_the_file_and_the_keys_and_never_a_value() {
    let scratch = Scratch::new("doctor");
    let file = snapshot_with(&scratch, "homebrew", found(&["HOMEBREW_NO_ANALYTICS"]));
    let out = run(&scratch, &["--from-snapshot", &file, "doctor"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "a warning: {text}");
    assert!(
        text.contains("homebrew: ~/.homebrew/brew.env sets HOMEBREW_NO_ANALYTICS."),
        "{text}"
    );
    let json_out = run(&scratch, &["--from-snapshot", &file, "--json", "doctor"]);
    let doc: Value = serde_json::from_slice(&json_out.stdout).expect("JSON");
    let ids: Vec<&str> = doc["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|f| f["id"].as_str())
        .filter(|id| id.starts_with("provider_configuration"))
        .collect();
    assert!(ids.contains(&"provider_configuration_outranks"), "{ids:?}");
    // MacPorts was not touched in the document: a replay's unknown.
    assert!(ids.contains(&"provider_configuration_unknown"), "{ids:?}");
}

#[test]
fn a_plan_against_that_provider_carries_the_risk_and_still_plans() {
    let scratch = Scratch::new("plan");
    let file = snapshot_with(&scratch, "homebrew", found(&["HOMEBREW_CACHE"]));
    let out = run(
        &scratch,
        &[
            "--from-snapshot",
            &file,
            "--json",
            "plan",
            "htop",
            "--to",
            "macports",
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: Value = serde_json::from_slice(&out.stdout).expect("JSON");
    let risk = doc["risks"]
        .as_array()
        .expect("risks")
        .iter()
        .find(|r| r["id"] == "provider_configuration")
        .unwrap_or_else(|| panic!("no configuration risk: {doc}"));
    assert_eq!(risk["blocking"], json!(false));
    assert_eq!(risk["reason"]["params"]["provider"], json!("homebrew"));
    assert_eq!(risk["reason"]["params"]["keys"], json!("HOMEBREW_CACHE"));
}

#[test]
fn a_snapshot_whose_configuration_key_is_not_a_key_is_refused() {
    let scratch = Scratch::new("hostile");
    for bad in [
        found(&["HOMEBREW_X\u{1b}[31m"]),
        json!({ "state": "READ", "files": [{
            "path": "relative/brew.env", "scope": "USER", "readable": true, "keys": []
        }]}),
    ] {
        let file = snapshot_with(&scratch, "homebrew", bad.clone());
        let out = run(&scratch, &["--from-snapshot", &file, "doctor"]);
        assert_ne!(out.status.code(), Some(0), "{bad}");
        assert!(out.stdout.is_empty(), "{bad}");
    }
}
