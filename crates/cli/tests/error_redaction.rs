// SPDX-License-Identifier: Apache-2.0

//! The top-level error path redacts this host's home, on both the message and
//! the `--verbose` detail.
//!
//! An error is what a person pastes into a bug report, and its detail can
//! carry a path under the home - here, a snapshot file named on the command
//! line. `06-cli-and-ux.md` says no home path or user name is published; the
//! security review of pass 06 found this path the one that still did.

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");

#[test]
fn an_error_under_the_home_never_prints_the_home_or_the_user_name() {
    let base = std::env::temp_dir().join(format!(
        "macportsbrew-error-redaction-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let home: PathBuf = base.join("Users").join("ada-lovelace");
    std::fs::create_dir_all(&home).expect("a temporary home");
    let missing = home.join("snapshots").join("missing.json");
    let out = Command::new(BIN)
        .args(["--verbose", "--from-snapshot"])
        .arg(&missing)
        .arg("status")
        .env("HOME", &home)
        .env("MACPORTSBREW_STATE_DIR", base.join("state"))
        .env_remove("MACPORTSBREW_LANG")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .unwrap_or_else(|e| panic!("spawning {BIN}: {e}"));
    let _ = std::fs::remove_dir_all(&base);
    let text = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code(), Some(0), "{text}");
    assert_eq!(
        text.lines().count(),
        2,
        "the message and the detail: {text}"
    );
    assert!(
        text.lines().all(|l| l.contains("~/snapshots/missing.json")),
        "both lines name the file, redacted: {text}"
    );
    assert!(
        !text.contains(&*home.to_string_lossy()) && !text.contains("ada-lovelace"),
        "the home or the user name reached stderr: {text}"
    );
}
