// SPDX-License-Identifier: Apache-2.0

//! A reason parameter that carries a domain identifier reaches `--json` as
//! that identifier and a person as its label, whichever emitter raised it.
//!
//! The planner raises `reason.class_requires_review` and
//! `reason.destination_deprecated` alongside the policy rules, and used to
//! spell their `class` and `state` with `format!("{:?}")` and a lower-case
//! word, so a cask's plan read "classed as GuiApp" (SE-064). Every run here is
//! a replay of a captured recording (INV-05).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

fn run(recording: &str, args: &[&str]) -> Output {
    let file = root().join(format!("fixtures/recordings/{recording}.json"));
    let state = std::env::temp_dir().join(format!(
        "macportsbrew-reason-params-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let out = Command::new(BIN)
        .arg("--replay")
        .arg(&file)
        .args(args)
        .args(["--today", "2026-09-02"])
        .current_dir(root())
        .env("MACPORTSBREW_STATE_DIR", &state)
        .env_remove("MACPORTSBREW_LANG")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .unwrap_or_else(|e| panic!("spawning {BIN}: {e}"));
    let _ = std::fs::remove_dir_all(&state);
    out
}

#[test]
fn a_planned_class_is_the_class_identifier_and_reads_as_its_label() {
    // `jupyterlab` declares a service in this recording: `SERVICE`, which forces
    // manual review. (A cask, `copilot-cli`, stood here; since part D a cask
    // row answers the service question `UNKNOWN` and is refused before any plan.)
    let json = run(
        "both-providers-arm64-macos15",
        &["--json", "plan", "jupyterlab", "--to", "macports"],
    );
    let doc: serde_json::Value = serde_json::from_slice(&json.stdout)
        .unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&json.stderr)));
    let class = doc["risks"]
        .as_array()
        .expect("risks")
        .iter()
        .find(|r| r["reason"]["id"] == "reason.class_requires_review")
        .map(|r| r["reason"]["params"]["class"].clone());
    assert_eq!(class, Some(serde_json::json!("SERVICE")), "{doc}");

    let human = run(
        "both-providers-arm64-macos15",
        &["plan", "jupyterlab", "--to", "macports"],
    );
    let text = String::from_utf8_lossy(&human.stdout);
    assert!(text.contains("classed as Background service"), "{text}");
    assert!(!text.contains("SERVICE"), "{text}");
}

#[test]
fn a_withdrawal_state_reads_as_its_label() {
    // `bvi` is disabled by Homebrew in this recording.
    let out = run(
        "homebrew-deprecated-and-tap-intel-macos26",
        &["explain", "bvi"],
    );
    let text = String::from_utf8_lossy(&out.stdout);
    // The sentence, not the facts dump: `explain` also lists the raw
    // availability field, which is an identifier on purpose.
    let sentence = text
        .lines()
        .find(|l| l.contains("marks bvi"))
        .unwrap_or_else(|| panic!("no withdrawal sentence:\n{text}"));
    assert!(
        sentence.contains("homebrew marks bvi disabled"),
        "{sentence}"
    );
    assert!(!sentence.contains("DISABLED"), "{sentence}");
}
