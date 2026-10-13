// SPDX-License-Identifier: Apache-2.0

//! Nothing personal in a committed fixture (pass 08 slice 3, D-F1).
//!
//! Recordings and snapshots are captured on real machines, and the repository
//! is public. A capture redacts the home directory to `~`, records which
//! configuration files exist without their keys' values, and keeps the
//! environment's `reported` values out; this test fails if a committed fixture
//! carries any of them anyway — a home path with a user name, the name of the
//! account running the test, a `reported` value, or a configuration key list.
//! `PATH` entries and the package inventory are deliberately public
//! (`fixtures/snapshots/README.md`).

use serde_json::Value;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("fixtures directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// A home directory named with an account: `/Users/<name>` or `/home/<name>`,
/// where `<name>` is anything but a placeholder in angle brackets.
fn home_paths(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for root in ["/Users/", "/home/"] {
        for (at, _) in text.match_indices(root) {
            let rest = &text[at + root.len()..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
                .collect();
            if !name.is_empty() && name != "Shared" {
                found.push(format!("{root}{name}"));
            }
        }
    }
    found
}

/// Every `reported` object that is not empty and every non-empty `keys` list.
fn values_that_must_be_empty(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let here = format!("{path}.{key}");
                match (key.as_str(), child) {
                    ("reported", Value::Object(m)) if !m.is_empty() => out.push(here.clone()),
                    ("keys", Value::Array(a)) if !a.is_empty() => out.push(here.clone()),
                    _ => {}
                }
                values_that_must_be_empty(child, &here, out);
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                values_that_must_be_empty(child, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

/// The account running the test, when it is long enough to be a name rather
/// than a common word.
fn current_user() -> Option<String> {
    std::env::var("USER")
        .ok()
        .filter(|u| u.len() >= 4 && u != "root")
}

#[test]
fn no_committed_fixture_carries_a_home_path_a_user_name_or_a_reported_value() {
    let mut all = Vec::new();
    files(&fixtures(), &mut all);
    assert!(
        all.iter()
            .any(|p| p.extension().is_some_and(|e| e == "json")),
        "no fixture was found under {}",
        fixtures().display()
    );
    let user = current_user();
    let mut violations = Vec::new();
    for file in &all {
        let name = file.strip_prefix(fixtures()).unwrap_or(file).display();
        // Every fixture is text; a file that is not would escape every check
        // below, so it is a violation rather than something to skip.
        let Ok(text) = std::fs::read_to_string(file) else {
            violations.push(format!("{name}: not readable as UTF-8 text"));
            continue;
        };
        for path in home_paths(&text) {
            violations.push(format!("{name}: home path {path}"));
        }
        if let Some(user) = &user {
            for shape in [
                format!("/{user}/"),
                format!("\"{user}\""),
                format!("~{user}"),
            ] {
                if text.contains(&shape) {
                    violations.push(format!("{name}: the current user's name"));
                }
            }
        }
        if file.extension().is_some_and(|e| e == "json") {
            let value: Value =
                serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name} is not JSON: {e}"));
            let mut found = Vec::new();
            values_that_must_be_empty(&value, "", &mut found);
            for at in found {
                violations.push(format!("{name}: {at} must be empty"));
            }
        }
    }
    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
fn the_privacy_checks_find_what_they_look_for() {
    assert_eq!(home_paths("x /Users/alice/bin y"), ["/Users/alice"]);
    assert_eq!(home_paths("/home/bob.smith"), ["/home/bob.smith"]);
    assert!(home_paths("/Users/<user>/bin ~/bin /Users/Shared/x").is_empty());
    let mut found = Vec::new();
    let doc: Value = serde_json::from_str(
        r#"{"environment":{"reported":{"HOMEBREW_X":"1"}},
            "configuration":{"files":[{"keys":["A"]},{"keys":[]}]},
            "other":{"reported":{}}}"#,
    )
    .expect("JSON");
    values_that_must_be_empty(&doc, "", &mut found);
    found.sort();
    assert_eq!(
        found,
        [".configuration.files[0].keys", ".environment.reported"]
    );
}
