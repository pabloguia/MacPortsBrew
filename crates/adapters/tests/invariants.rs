// SPDX-License-Identifier: Apache-2.0

//! Invariant tests for the adapter layer.
//!
//! `01-invariants.md` requires every invariant to have at least one test named
//! after it, and a change that touches one to be able to point at that test.
//! `crates/policy/tests/invariants.rs` does this for the engine; this file
//! does it for the adapters, whose invariants are mostly about what the code
//! is *not* allowed to reach for.
//!
//! The source scans below follow the pattern of
//! `invariant_20_no_provider_deadline_is_written_into_source` in the policy
//! crate: a doc comment saying "this is the only clock read" is precisely the
//! kind of claim that cannot fail a test, so it is asserted against the source
//! text instead.

use macportsbrew_adapters::persistence::StateDir;
use macportsbrew_adapters::process::ReplayRunner;
use macportsbrew_adapters::system::{is_reported, SystemMachine};
use macportsbrew_domain::{HomeRedaction, MachineRead};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "macportsbrew-inv-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The workspace root, from this crate's manifest directory.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root must be reachable")
}

/// Every `.rs` file under every crate's `src/`, as `(path, text)`.
fn sources() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let crates = workspace().join("crates");
    let mut stack = vec![crates];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                && path.components().any(|c| c.as_os_str() == "src")
            {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    found.push((path, text));
                }
            }
        }
    }
    assert!(found.len() > 10, "the source scan found almost nothing");
    found
}

/// Lines of `text` that contain `needle` and are not a comment.
fn code_lines<'a>(text: &'a str, needle: &str) -> Vec<&'a str> {
    text.lines()
        .filter(|line| line.contains(needle))
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with('*')
        })
        .collect()
}

#[test]
fn invariant_14_the_wall_clock_is_read_in_exactly_one_place() {
    // INV-14 / INV-15: a recommendation is a pure function of its inputs. A
    // second clock read would make an analysis depend on when it ran, and no
    // other test would notice.
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        let is_the_one_place = path.ends_with("persistence/time.rs");
        for line in code_lines(&text, "SystemTime::now") {
            if !is_the_one_place {
                offenders.push(format!("{}: {}", path.display(), line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the wall clock is read outside persistence::time::now_secs:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn invariant_14_the_environment_is_read_in_exactly_one_place() {
    // Same reasoning: an adapter that consulted the shell directly would make
    // a replayed snapshot depend on the shell replaying it.
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        let is_the_one_place = path.ends_with("system/machine.rs");
        for line in code_lines(&text, "env::var") {
            if !is_the_one_place {
                offenders.push(format!("{}: {}", path.display(), line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the environment is read outside system::machine:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn invariant_15_a_replayed_reader_never_consults_the_live_filesystem() {
    // `which` resolves against the machine's own `PATH`. Under replay the
    // machine is a recording, and the filesystem is the replaying host's, so
    // answering from it would make the same snapshot produce different output
    // on two machines.
    let dir = scratch("replay-which");
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("tool"), b"#!/bin/sh\n").unwrap();
    std::fs::set_permissions(
        bin.join("tool"),
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .unwrap();

    let fixture = workspace().join("fixtures/recordings/homebrew-only-intel-macos26.json");
    let recording =
        macportsbrew_adapters::persistence::load_recording(&fixture).expect("fixture must load");
    let replayed = SystemMachine::new(
        Box::new(ReplayRunner::new(recording)),
        Some(bin.to_string_lossy().into_owned()),
        BTreeMap::new(),
        HomeRedaction::Recorded,
    );
    assert!(replayed.is_replay());
    assert_eq!(
        replayed.which("tool"),
        None,
        "a replayed reader answered from the replaying host's filesystem"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn invariant_16_ensuring_the_state_directory_is_idempotent() {
    // Re-running a successful operation changes nothing further: same tree,
    // same modes, same contents, nothing accumulated. The ownership probe
    // `ensure` writes and removes does move the root's mtime, which is why
    // that is not what is compared.
    let home = scratch("idempotent");
    let dir = StateDir::resolve(Some(&home), None).unwrap();
    dir.ensure().unwrap();

    let marker = dir.history().join("run.json");
    macportsbrew_adapters::persistence::write_atomic(&marker, b"{}").unwrap();

    let listing = |d: &StateDir| -> Vec<(PathBuf, u32, Vec<String>)> {
        d.directories()
            .iter()
            .map(|path| {
                let mode = <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::mode(
                    &std::fs::metadata(path).unwrap().permissions(),
                ) & 0o777;
                let mut names: Vec<String> = std::fs::read_dir(path)
                    .unwrap()
                    .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                (path.clone(), mode, names)
            })
            .collect()
    };

    let before = listing(&dir);
    dir.ensure().unwrap();
    dir.ensure().unwrap();
    let after = listing(&dir);

    assert_eq!(
        before, after,
        "a repeated ensure() changed a tree it had already made correct"
    );
    assert_eq!(std::fs::read(&marker).unwrap(), b"{}");
    // The ownership probe is not left behind.
    for (path, _, names) in &after {
        assert!(
            !names.iter().any(|n| n.contains("owner-probe")),
            "{} kept a probe file",
            path.display()
        );
    }

    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn invariant_10_reading_the_machine_leaks_no_build_flag_into_a_probe() {
    // Every invocation runs in an environment built from scratch. A probe that
    // inherited `PKG_CONFIG_PATH` would be the contamination INV-10 forbids,
    // and these probes have no reason to carry one.
    let fixture = workspace().join("fixtures/recordings/homebrew-only-intel-macos26.json");
    let recording =
        macportsbrew_adapters::persistence::load_recording(&fixture).expect("fixture must load");
    for recorded in &recording.invocations {
        let invocation = &recorded.invocation;
        if !matches!(
            invocation.program.as_str(),
            "sw_vers" | "uname" | "sysctl" | "xcode-select"
        ) {
            continue;
        }
        for name in invocation.environment.keys() {
            assert!(
                !is_reported(name) || name == "MANPATH",
                "{} ran with {name} set",
                invocation.program
            );
        }
    }
}

#[test]
fn invariant_06_reading_the_machine_needs_no_elevation() {
    // Discovery must work with no elevation at all. The four probes are
    // unprivileged system tools, and nothing in the adapter layer names an
    // elevation mechanism outside a comment.
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        for needle in ["sudo", "doas", "setuid", "seteuid", "AuthorizationExecute"] {
            for line in code_lines(&text, needle) {
                offenders.push(format!("{}: {}", path.display(), line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "an elevation mechanism appears in non-comment source:\n{}",
        offenders.join("\n")
    );

    let fixture = workspace().join("fixtures/recordings/homebrew-only-intel-macos26.json");
    let recording =
        macportsbrew_adapters::persistence::load_recording(&fixture).expect("fixture must load");
    let reader = SystemMachine::new(
        Box::new(ReplayRunner::new(recording)),
        None,
        BTreeMap::new(),
        HomeRedaction::Recorded,
    );
    reader.machine().expect("the machine reads unprivileged");
}
