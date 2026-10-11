// SPDX-License-Identifier: Apache-2.0

//! The command-line crate names no provider (AGENTS.md: provider names appear
//! in `crates/adapters` and nowhere above; `registry.rs` is the one place that
//! knows which providers exist).
//!
//! `scripts/check-layering.sh` scans `domain` and `policy` for identifiers; a
//! replay helper in this crate once parsed `brew --prefix` and `port dir`
//! itself, which a third provider added by the recipe would silently miss
//! (spec-guardian, pre-release review, M1). This holds the presentation crate
//! to the rule: no provider id or program name as a string literal in non-test
//! code.

use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The part of a source file before its first `#[cfg(test)]`, and nothing of
/// a file that exists only for tests.
fn non_test_code(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with("_tests.rs") {
        return String::new();
    }
    let text = std::fs::read_to_string(path).expect("source");
    match text.find("#[cfg(test)]") {
        Some(at) => text[..at].to_string(),
        None => text,
    }
}

#[test]
fn the_command_line_crate_names_no_provider_in_its_own_code() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(!files.is_empty());
    let words = ["\"homebrew\"", "\"macports\"", "\"brew\"", "\"port\""];
    let mut found = Vec::new();
    for file in &files {
        for (n, line) in non_test_code(file).lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for word in words {
                if code.contains(word) {
                    found.push(format!("{}:{}: {word}", file.display(), n + 1));
                }
            }
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn invariant_14_nothing_in_the_build_can_open_a_network_connection() {
    // No LLM or any other service at run time (INV-14), and no telemetry
    // (the charter): no network crate in the lock file and no socket type in
    // any crate's own code. A Homebrew read can still fetch Homebrew's
    // catalogue; that is Homebrew's process, not this one.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).expect("Cargo.lock");
    let network_crates = [
        "reqwest",
        "hyper",
        "ureq",
        "curl",
        "isahc",
        "surf",
        "rustls",
        "native-tls",
        "openssl",
        "tokio",
        "async-std",
        "mio",
        "socket2",
        "h2",
        "attohttpc",
    ];
    let mut found: Vec<String> = lock
        .lines()
        .filter_map(|l| l.strip_prefix("name = \""))
        .map(|n| n.trim_end_matches('"').to_string())
        .filter(|n| network_crates.contains(&n.as_str()))
        .collect();
    let mut files = Vec::new();
    for krate in ["domain", "policy", "application", "adapters", "cli"] {
        sources(&root.join("crates").join(krate).join("src"), &mut files);
    }
    for file in &files {
        let text = std::fs::read_to_string(file).expect("source");
        for word in ["std::net", "TcpStream", "UdpSocket", "TcpListener"] {
            if text.contains(word) {
                found.push(format!("{}: {word}", file.display()));
            }
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}
