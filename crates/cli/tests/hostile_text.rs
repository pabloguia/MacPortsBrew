// SPDX-License-Identifier: Apache-2.0

//! One definition of hostile text, applied at both ends (SE-092).
//!
//! `Snapshot::check` is the one door a snapshot passes, on the capture route
//! and on the load route; it refuses the control class and keeps format
//! characters and the line separators. Every human renderer escapes all three
//! classes. These tests drive both routes with the real adapters and the real
//! binary, which is why they live in the command-line crate: the layering lets
//! only this crate see the capture, the loader and the renderer together.

use macportsbrew_adapters::persistence::snapshot_store::load_snapshot;
use macportsbrew_adapters::registry;
use macportsbrew_application::{capture, CaptureContext};
use macportsbrew_domain::{
    Architecture, CommandLineTools, DomainError, EnvironmentSnapshot, ErrorId, HomeRedaction,
    HostileClass, Machine, MachineRead, MacosVersion, PathFinding, PathFindingKind, Snapshot,
    Translation,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");

/// A machine that reports whatever environment the test gives it.
struct Reporting(EnvironmentSnapshot);

impl MachineRead for Reporting {
    fn machine(&self) -> Result<Machine, DomainError> {
        Ok(Machine {
            macos: MacosVersion::new(26, 0, 0),
            architecture: Architecture::Arm64,
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Missing,
        })
    }

    fn environment(&self) -> Result<EnvironmentSnapshot, DomainError> {
        Ok(self.0.clone())
    }

    fn which(&self, _executable: &str) -> Option<PathBuf> {
        None
    }
}

fn captured(environment: EnvironmentSnapshot) -> Result<Snapshot, DomainError> {
    capture(&CaptureContext {
        machine: &Reporting(environment),
        providers: Vec::new(),
        policy_version: "test".into(),
        captured_at: "2026-10-02T00:00:00Z".into(),
        captured_by: "hostile_text.rs".into(),
        home: HomeRedaction::Recorded,
        environment_contracts: registry::environment_contracts(),
    })
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "macportsbrew-hostile-text-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

fn write(snapshot: &Snapshot, dir: &Path) -> PathBuf {
    let file = dir.join("snapshot.json");
    std::fs::write(
        &file,
        serde_json::to_string_pretty(snapshot).expect("a snapshot serialises"),
    )
    .expect("the scratch file is writable");
    file
}

/// H1: `reported` was the one display field the check did not read, and
/// `doctor` prints its names back. A control character in a variable **name**
/// is refused, and refused **identically** on the capture route and on the load
/// route, so `snapshot` followed by `--from-snapshot` cannot disagree with a
/// live run (INV-15). A control character in a **value** - a tab or a newline
/// in `CPPFLAGS` is a correct machine - is kept as recorded by both routes.
#[test]
fn invariant_15_a_hostile_reported_name_is_refused_identically_by_capture_and_by_load() {
    let clean = captured(EnvironmentSnapshot::default()).expect("a clean machine captures");
    let dir = scratch("reported");

    let hostile_name = EnvironmentSnapshot {
        reported: BTreeMap::from([("CPP\u{1b}[2JFLAGS".to_string(), "-I/x".to_string())]),
        ..EnvironmentSnapshot::default()
    };
    let at_capture = captured(hostile_name.clone()).expect_err("capture refuses the name");
    // The capture route relabels the identifier, deliberately and only that
    // (`capture.rs`); the words are the check's own.
    assert_eq!(at_capture.id, ErrorId::InputUnusable);
    let mut file = clean.clone();
    file.environment = hostile_name;
    let at_load = load_snapshot(&write(&file, &dir)).expect_err("load refuses the name");
    assert_eq!(at_load.id, ErrorId::SnapshotUnusable);
    assert_eq!(
        at_capture.detail, at_load.detail,
        "the two routes must refuse with one check"
    );

    let control_in_value = EnvironmentSnapshot {
        reported: BTreeMap::from([("CPPFLAGS".to_string(), "-I/x\t-I/y\n-DZ".to_string())]),
        ..EnvironmentSnapshot::default()
    };
    let kept = captured(control_in_value.clone()).expect("capture keeps the value");
    assert_eq!(kept.environment.reported, control_in_value.reported);
    let mut file = clean;
    file.environment = control_in_value.clone();
    let loaded = load_snapshot(&write(&file, &dir)).expect("load keeps the value");
    assert_eq!(loaded.environment.reported, control_in_value.reported);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The other half: a format character is kept by the loader - a snapshot is the
/// record, and legitimate names carry them - and escaped by the renderer, here
/// `doctor` printing a `PATH` finding and a `reported` variable straight out of
/// the file.
#[test]
fn a_format_character_a_snapshot_keeps_is_escaped_when_doctor_prints_it() {
    let dir = scratch("doctor");
    let mut snapshot = captured(EnvironmentSnapshot::default()).expect("a clean machine captures");
    snapshot.environment = EnvironmentSnapshot {
        installations: std::collections::BTreeMap::default(),
        path: vec!["/opt/a\u{202E}b".into(), "/usr/bin".into()],
        reported: BTreeMap::new(),
        path_findings: vec![PathFinding {
            entry: "/opt/a\u{202E}b".into(),
            index: 0,
            kind: PathFindingKind::Relative,
        }],
    };
    let file = write(&snapshot, &dir);
    load_snapshot(&file).expect("a format character is kept, not refused");
    let run = Command::new(BIN)
        .args([
            "--from-snapshot",
            file.to_str().expect("UTF-8 path"),
            "doctor",
            "--today",
            "2026-10-02",
        ])
        .env("MACPORTSBREW_STATE_DIR", &dir)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8(run.stdout).expect("UTF-8 output");
    assert!(
        stdout.contains("/opt/a\\u{202e}b"),
        "the finding is not shown escaped:\n{stdout}"
    );
    assert!(
        !stdout
            .lines()
            .any(|line| line.chars().any(macportsbrew_domain::is_hostile)),
        "a hostile character reached the terminal:\n{stdout:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every hostile character a snapshot keeps (the Format and LineSeparator
/// samples of the shared table), in one string.
fn kept_hostile() -> String {
    macportsbrew_domain::CHARACTER_SAMPLES
        .iter()
        .filter(|(_, class)| {
            matches!(
                class,
                Some(HostileClass::Format | HostileClass::LineSeparator)
            )
        })
        .map(|(c, _)| *c)
        .collect()
}

/// Puts `kept_hostile()` into every display field a snapshot keeps it in, and a
/// control character into a reported value, the one place a control is kept.
fn poison(snapshot: &mut serde_json::Value) {
    use serde_json::Value;
    let h = kept_hostile();
    let env = &mut snapshot["environment"];
    env["path"]
        .as_array_mut()
        .expect("a PATH")
        .push(Value::String(format!("/opt/p{h}q/bin")));
    let index = env["path"].as_array().map_or(0, Vec::len) - 1;
    env["pathFindings"]
        .as_array_mut()
        .expect("PATH findings")
        .push(serde_json::json!({
            "entry": format!("/opt/p{h}q/bin"),
            "index": index,
            "kind": "MISSING",
        }));
    env["reported"]
        .as_object_mut()
        .expect("reported variables")
        .insert(
            "CPPFLAGS".into(),
            Value::String(format!("-I/opt/r{h}\u{1b}[2J\t-Is")),
        );
    for (_, provider) in snapshot["providers"]
        .as_object_mut()
        .expect("providers")
        .iter_mut()
    {
        let detection = &mut provider["detection"];
        if let Some(Value::String(version)) = detection.get_mut("version") {
            version.push_str(&h);
        }
        if let Some(Value::String(executable)) = detection.get_mut("executable") {
            executable.push_str(&h);
        }
        if let Some(installed) = provider
            .get_mut("inventory")
            .and_then(|i| i.get_mut("installed"))
            .and_then(Value::as_array_mut)
        {
            for row in installed {
                if let Some(variants) = row.get_mut("variants").and_then(Value::as_array_mut) {
                    variants.push(Value::String(format!("+v{h}")));
                }
                if let Some(Value::String(path)) = row.get_mut("installPath") {
                    path.push_str(&h);
                }
            }
        }
        // Evidence detail, which `explain` prints (security re-review,
        // 2026-10-02).
        if let Some(availability) = provider
            .get_mut("inventory")
            .and_then(|i| i.get_mut("availability"))
            .and_then(Value::as_object_mut)
        {
            for (_, assessment) in availability.iter_mut() {
                if let Some(evidence) = assessment.get_mut("evidence").and_then(Value::as_array_mut)
                {
                    for item in evidence {
                        if let Some(Value::String(detail)) = item.get_mut("detail") {
                            detail.push_str(&h);
                        }
                    }
                }
            }
        }
        if let Some(resolutions) = provider
            .get_mut("resolutions")
            .and_then(Value::as_object_mut)
        {
            for (_, resolution) in resolutions.iter_mut() {
                if let Some(Value::String(path)) = resolution.get_mut("path") {
                    path.push_str(&h);
                }
            }
        }
    }
}

/// The reviewers' reproduction, kept: a snapshot derived from a `--replay`
/// capture, with a kept hostile character in every display field that keeps
/// one, read by every human command. None of them may reach the terminal: the
/// loader keeps them (a snapshot is the record) and the renderer escapes them.
#[test]
fn no_human_command_prints_a_hostile_character_a_snapshot_keeps() {
    let dir = scratch("every-command");
    let recording = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings/both-providers-intel-macos26.json");
    let original = dir.join("original.json");
    let capture = Command::new(BIN)
        .args([
            "--replay",
            recording.to_str().expect("UTF-8 path"),
            "snapshot",
            "--out",
            original.to_str().expect("UTF-8 path"),
            "--today",
            "2026-10-02",
        ])
        .env("MACPORTSBREW_STATE_DIR", &dir)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .expect("the binary runs");
    assert!(capture.status.success(), "{capture:?}");
    let mut document: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&original).expect("the snapshot was written"),
    )
    .expect("a snapshot is JSON");
    poison(&mut document);
    let file = dir.join("poisoned.json");
    std::fs::write(&file, serde_json::to_string_pretty(&document).unwrap()).unwrap();
    let loaded = load_snapshot(&file).expect("every kept character is kept, not refused");
    assert!(
        loaded
            .environment
            .path
            .iter()
            .any(|e| e.contains(&kept_hostile())),
        "the poisoned snapshot is the one being read"
    );

    let file = file.to_str().expect("UTF-8 path");
    let mut shown_escaped = String::new();
    for command in [
        vec!["status"],
        vec!["analyze"],
        vec!["analyze", "--reasons"],
        vec!["list"],
        vec!["explain", "htop"],
        vec!["plan", "htop", "--to", "macports"],
        vec!["plan", "htop", "--to", "homebrew"],
        vec!["migrate", "htop", "--to", "macports"],
        vec!["migrate", "htop", "--to", "homebrew"],
        vec!["doctor"],
        vec!["--verbose", "explain", "htop"],
        vec!["--verbose", "migrate", "htop", "--to", "macports"],
    ] {
        let mut argv = vec!["--from-snapshot", file];
        argv.extend(command.iter().copied());
        argv.extend(["--today", "2026-10-02"]);
        let run = Command::new(BIN)
            .args(&argv)
            .env("MACPORTSBREW_STATE_DIR", &dir)
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .output()
            .expect("the binary runs");
        shown_escaped.push_str(&String::from_utf8_lossy(&run.stdout));
        for (stream, bytes) in [("stdout", &run.stdout), ("stderr", &run.stderr)] {
            let text = String::from_utf8_lossy(bytes);
            for line in text.split('\n') {
                assert!(
                    !line.chars().any(macportsbrew_domain::is_hostile),
                    "{argv:?} put a hostile character on {stream}: {line:?}"
                );
            }
        }
    }
    // Not vacuous: the poisoned fields were printed, escaped.
    let marker = '\u{202E}'.escape_unicode().to_string();
    assert!(
        shown_escaped.contains(&marker),
        "no command printed a poisoned field, so this test proves nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
