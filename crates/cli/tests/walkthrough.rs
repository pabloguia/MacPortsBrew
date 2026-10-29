// SPDX-License-Identifier: Apache-2.0

//! A user's first ten minutes, on every committed machine (pass 10, item 2).
//!
//! `wiki/Getting-Started.md` walks a new user through `status`, `analyze`,
//! `list`, `explain`, `doctor` and `plan`. Each step below runs on every
//! recording in `fixtures/recordings/` - Homebrew only, MacPorts only, both,
//! neither, a broken MacPorts, an older macOS - and pins its exit code and the
//! first line a person reads, so an empty state, a broken provider or a refusal
//! cannot drift into something confusing without a test failing.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Exit code, first line of stdout, first line of stderr.
fn run(fixture: &str, args: &[&str]) -> (i32, String, String) {
    let state = std::env::temp_dir().join(format!(
        "macportsbrew-walkthrough-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let out = Command::new(BIN)
        .arg("--replay")
        .arg(root().join(format!("fixtures/recordings/{fixture}.json")))
        .args(args)
        .current_dir(root())
        .env("MACPORTSBREW_STATE_DIR", &state)
        .env_remove("MACPORTSBREW_LANG")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .unwrap_or_else(|e| panic!("spawning {BIN}: {e}"));
    let _ = std::fs::remove_dir_all(&state);
    let first = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .lines()
            .next()
            .unwrap_or("")
            .to_string()
    };
    (
        out.status.code().expect("an exit code"),
        first(&out.stdout),
        first(&out.stderr),
    )
}

const FIXTURES: &[&str] = &[
    "both-providers-arm64-macos15",
    "both-providers-intel-macos26",
    "homebrew-deprecated-and-tap-intel-macos26",
    "homebrew-only-intel-macos26",
    "macports-broken",
    "macports-only-intel-macos12",
    "neither-provider-intel-macos13",
];

#[test]
fn status_opens_with_the_machine_and_ends_with_a_plain_summary() {
    let summaries = [
        (
            "both-providers-arm64-macos15",
            "63 packages in the default view; 22 need manual review.",
        ),
        (
            "both-providers-intel-macos26",
            "59 packages in the default view; 16 need manual review.",
        ),
        (
            "homebrew-deprecated-and-tap-intel-macos26",
            "56 packages in the default view; 17 need manual review.",
        ),
        (
            "homebrew-only-intel-macos26",
            "56 packages in the default view; 15 need manual review.",
        ),
        (
            "macports-broken",
            "0 packages in the default view; 0 need manual review.",
        ),
        (
            "macports-only-intel-macos12",
            "5 packages in the default view; 3 need manual review.",
        ),
        (
            "neither-provider-intel-macos13",
            "0 packages in the default view; 0 need manual review.",
        ),
    ];
    for (fixture, summary) in summaries {
        let (code, first, _) = run(fixture, &["status"]);
        assert_eq!(code, 0, "{fixture}");
        assert_eq!(first, "Machine", "{fixture}");
        let full = Command::new(BIN)
            .arg("--replay")
            .arg(root().join(format!("fixtures/recordings/{fixture}.json")))
            .arg("status")
            .env("LC_ALL", "C")
            .env_remove("MACPORTSBREW_LANG")
            .output()
            .expect("status");
        let text = String::from_utf8_lossy(&full.stdout);
        assert!(text.contains(summary), "{fixture}: {text}");
    }
    assert_eq!(FIXTURES.len(), summaries.len(), "every fixture is walked");
}

#[test]
fn analyze_and_list_open_with_the_table_or_say_why_there_is_none() {
    const NEITHER: &str = "Neither Homebrew nor MacPorts answered here, so there is nothing to \
                           analyse. `macportsbrew doctor` shows where it looked.";
    const BROKEN: &str = "macports is installed but did not answer correctly, so its packages \
                          could not be read. Run `macportsbrew doctor`.";
    for fixture in FIXTURES {
        for command in ["analyze", "list"] {
            let (code, first, _) = run(fixture, &[command]);
            assert_eq!(code, 0, "{fixture} {command}");
            match *fixture {
                "neither-provider-intel-macos13" => assert_eq!(first, NEITHER),
                "macports-broken" => assert_eq!(first, BROKEN),
                _ => assert!(
                    first.starts_with("Package ") && first.ends_with("Recommendation"),
                    "{fixture} {command}: {first}"
                ),
            }
        }
    }
}

#[test]
fn doctor_says_what_is_wrong_first_and_fails_only_on_an_error() {
    const NO_PATH: &str = "[Warning] This analysis carries no PATH, so nothing can be said about \
                           which provider's executables win. A recording does not capture the \
                           environment; run `doctor` live for the real answer.";
    for fixture in FIXTURES {
        let (code, first, _) = run(fixture, &["doctor"]);
        if *fixture == "macports-broken" {
            assert_eq!(code, 5, "{fixture}");
            assert_eq!(
                first,
                "[Error] macports is present but not working. See Troubleshooting in the \
                 MacPortsBrew wiki."
            );
        } else {
            assert_eq!(code, 0, "{fixture}");
            assert_eq!(first, NO_PATH, "{fixture}");
        }
    }
}

#[test]
fn explain_and_plan_name_the_package_or_say_plainly_why_not() {
    // (fixture, args, exit code, first line of stdout, start of stderr)
    let package = ["c", "url"].concat();
    let steps: Vec<(&str, Vec<&str>, i32, String, &str)> = vec![
        (
            "both-providers-intel-macos26",
            vec!["explain", "htop"],
            0,
            "htop (held by homebrew)".into(),
            "",
        ),
        // A recording cannot list a keg, so a source row that declares no
        // service answers the service question `UNKNOWN`, and its plan is
        // refused, and says so, in either direction (DP-032, part D). A plan is
        // walked from the live capture (`a_hostile_prefix_in_a_snapshot_*`, below).
        (
            "both-providers-intel-macos26",
            vec!["plan", "htop", "--to", "macports"],
            4,
            String::new(),
            "macportsbrew: A safety rule could not be evaluated from this input",
        ),
        (
            "both-providers-arm64-macos15",
            vec!["explain", "htop"],
            0,
            "htop (held by homebrew)".into(),
            "",
        ),
        (
            "both-providers-arm64-macos15",
            vec!["plan", "htop", "--to", "macports"],
            4,
            String::new(),
            "macportsbrew: A safety rule could not be evaluated from this input",
        ),
        (
            "homebrew-only-intel-macos26",
            vec!["explain", &package],
            0,
            format!("{package} (held by homebrew)"),
            "",
        ),
        (
            "homebrew-only-intel-macos26",
            vec!["plan", &package, "--to", "macports"],
            4,
            String::new(),
            "macportsbrew: A safety rule could not be evaluated from this input",
        ),
        (
            "homebrew-deprecated-and-tap-intel-macos26",
            vec!["explain", &package],
            0,
            format!("{package} (held by homebrew)"),
            "",
        ),
        (
            "homebrew-deprecated-and-tap-intel-macos26",
            vec!["plan", &package, "--to", "macports"],
            4,
            String::new(),
            "macportsbrew: A safety rule could not be evaluated from this input",
        ),
        (
            "macports-only-intel-macos12",
            vec!["explain", "gnutls"],
            0,
            "gnutls (held by macports)".into(),
            "",
        ),
        // The same refusal from a MacPorts source (DP-016, DP-032).
        (
            "macports-only-intel-macos12",
            vec!["plan", "gnutls", "--to", "homebrew"],
            4,
            String::new(),
            "macportsbrew: A safety rule could not be evaluated from this input",
        ),
        (
            "macports-broken",
            vec!["explain", "jq"],
            1,
            String::new(),
            "macportsbrew: That package is not present under that provider",
        ),
        (
            "neither-provider-intel-macos13",
            vec!["explain", "jq"],
            1,
            String::new(),
            "macportsbrew: That package is not present under that provider",
        ),
    ];
    for (fixture, args, code, first, err) in &steps {
        let (got_code, got_first, got_err) = run(fixture, args);
        assert_eq!(got_code, *code, "{fixture} {args:?}: {got_err}");
        assert_eq!(&got_first, first, "{fixture} {args:?}");
        assert!(got_err.starts_with(err), "{fixture} {args:?}: {got_err}");
    }
    let walked: std::collections::BTreeSet<&str> = steps.iter().map(|s| s.0).collect();
    assert_eq!(walked.len(), FIXTURES.len(), "every fixture is walked");
}

#[test]
fn an_unknown_provider_is_an_error_not_an_empty_list() {
    let (code, first, err) = run(
        "both-providers-intel-macos26",
        &["list", "--provider", "brew"],
    );
    assert_eq!(code, 2, "{err}");
    assert_eq!(first, "");
    assert!(
        err.contains("not a provider this build knows; use one of: homebrew, macports"),
        "{err}"
    );
    let (code, _, err) = run(
        "both-providers-intel-macos26",
        &["plan", "htop", "--to", "brew"],
    );
    assert_eq!(code, 2, "{err}");
    let (code, _, err) = run(
        "both-providers-intel-macos26",
        &["--today", "2026-13-01", "status"],
    );
    assert_eq!(code, 2, "{err}");
    let (code, _, _) = run(
        "both-providers-intel-macos26",
        &["list", "--provider", "macports"],
    );
    assert_eq!(code, 0);
}

#[test]
fn an_empty_fact_in_explain_reads_none_rather_than_a_blank() {
    let out = Command::new(BIN)
        .arg("--replay")
        .arg(root().join("fixtures/recordings/both-providers-intel-macos26.json"))
        .args(["explain", "htop"])
        .env("LC_ALL", "C")
        .env_remove("MACPORTSBREW_LANG")
        .output()
        .expect("explain");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("  package.classes: none\n"), "{text}");
}

#[test]
fn stale_policy_data_warns_once_and_never_refuses() {
    // The staleness threshold is 180 days (`data/policy/policy.toml`): past
    // it, every command still answers, and says once, on stderr, that the
    // provider facts may have changed (pass 10, item 5).
    let fixture = root().join("fixtures/recordings/both-providers-intel-macos26.json");
    for args in [
        vec!["status"],
        vec!["analyze"],
        // A plan a recording can still build (a declared service, part D):
        // it stops at manual review, exit 4, which is not a refusal.
        vec!["plan", "jupyterlab", "--to", "macports"],
    ] {
        let out = Command::new(BIN)
            .arg("--replay")
            .arg(&fixture)
            .args(["--today", "2030-01-01"])
            .args(&args)
            .env("LC_ALL", "C")
            .env_remove("MACPORTSBREW_LANG")
            .output()
            .expect("run");
        let want = if args[0] == "plan" { 4 } else { 0 };
        assert_eq!(out.status.code(), Some(want), "{args:?}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(!stdout.is_empty(), "{args:?}: it answers");
        let stderr = String::from_utf8_lossy(&out.stderr);
        let notice = "Policy data was last verified on";
        assert_eq!(stderr.matches(notice).count(), 1, "{args:?}: {stderr}");
        assert!(!stdout.contains(notice), "{args:?}: said twice");
        assert!(!stderr.contains("http"), "{args:?}: no address is invented");
    }
}

#[test]
fn snapshot_and_report_replace_only_their_own_documents() {
    let dir = std::env::temp_dir().join(format!(
        "macportsbrew-no-replace-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    let existing = dir.join("notes.txt");
    std::fs::write(&existing, "keep me").expect("write");
    let path = existing.to_str().expect("UTF-8");
    for args in [
        vec!["snapshot", "--out", path],
        vec!["doctor", "--report", path],
    ] {
        let (code, _, err) = run("both-providers-intel-macos26", &args);
        assert_eq!(code, 2, "{args:?}: {err}");
        assert!(err.contains("already exists"), "{args:?}: {err}");
        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "keep me");
    }
    // A new name is written.
    let fresh = dir.join("snapshot.json");
    let (code, _, err) = run(
        "both-providers-intel-macos26",
        &["snapshot", "--out", fresh.to_str().expect("UTF-8")],
    );
    assert_eq!(code, 0, "{err}");
    assert!(fresh.exists());
    // Running it again refreshes its own document rather than refusing.
    let (code, _, err) = run(
        "both-providers-intel-macos26",
        &["snapshot", "--out", fresh.to_str().expect("UTF-8")],
    );
    assert_eq!(code, 0, "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn a_hostile_prefix_in_a_snapshot_is_rendered_as_one_quoted_word() {
    // A snapshot is a file anyone can hand over. Its prefix reaches the
    // commands a plan invites a person to type, so each word is quoted.
    let original = std::fs::read_to_string(
        root().join("fixtures/snapshots/both-providers-intel-macos26-live.json"),
    )
    .expect("snapshot");
    let hostile = original.replace("/opt/local", "/opt/x $(touch y)");
    let dir = std::env::temp_dir().join(format!(
        "macportsbrew-hostile-prefix-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    let file = dir.join("hostile.json");
    std::fs::write(&file, hostile).expect("write");
    let out = Command::new(BIN)
        .arg("--from-snapshot")
        .arg(&file)
        .args(["plan", "htop", "--to", "homebrew"])
        .env("LC_ALL", "C")
        .env_remove("MACPORTSBREW_LANG")
        .output()
        .expect("plan");
    let text = String::from_utf8_lossy(&out.stdout);
    let _ = std::fs::remove_dir_all(&dir);
    // The plan must be built, or nothing here tests the quoting.
    assert_eq!(out.status.code(), Some(0), "{text}");
    let quoted = "$ '/opt/x $(touch y)/bin/port' ";
    assert!(text.contains(quoted), "{text}");
    assert!(!text.contains("$ /opt/x $(touch y)"), "{text}");
}
