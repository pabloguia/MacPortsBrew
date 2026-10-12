// SPDX-License-Identifier: Apache-2.0

//! `-V` and `--version` as the release job and a bug report read them
//! (pass 09, D-V1).

use macportsbrew_policy::PolicyData;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env_remove("MACPORTSBREW_LANG")
        .output()
        .unwrap_or_else(|e| panic!("spawning {BIN}: {e}"))
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn short_version_is_name_and_semver() {
    let out = run(&["-V"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        stdout(&out),
        format!("macportsbrew {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn long_version_names_policy_target_and_commit() {
    let out = run(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().collect();
    // Four lines, and a fifth `build <n>` only for a build CI numbered.
    let numbered = option_env!("MPB_BUILD_NUMBER");
    assert_eq!(lines.len(), 4 + usize::from(numbered.is_some()), "{text}");
    assert_eq!(
        lines[0],
        format!("macportsbrew {}", env!("CARGO_PKG_VERSION"))
    );
    let policy = PolicyData::load_embedded().expect("embedded policy");
    assert_eq!(lines[1], format!("policy {}", policy.version()));
    let target = lines[2].strip_prefix("target ").expect("target line");
    assert!(target.ends_with("-apple-darwin"), "{target}");
    assert!(
        target.starts_with(std::env::consts::ARCH),
        "{target} is not this machine's architecture"
    );
    let commit = lines[3].strip_prefix("commit ").expect("commit line");
    match option_env!("MPB_BUILD_COMMIT") {
        Some(expected) => assert_eq!(commit, expected),
        None => assert_eq!(commit, "unknown (not a release build)"),
    }
    match numbered {
        Some(n) => assert_eq!(lines[4], format!("build {n}")),
        None => assert!(!text.contains("\nbuild ")),
    }
}

#[test]
fn a_usage_error_keeps_its_exit_code() {
    // clap's usage-error code, unchanged by building the command by hand to
    // give it the long version.
    let out = run(&["--no-such-flag"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty());
}
