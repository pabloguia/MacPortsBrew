// SPDX-License-Identifier: Apache-2.0

//! `doctor` reports what the `PATH` audit found, and decides how bad it is.
//!
//! The audit itself (`adapters::system::path_audit`) is provider-neutral and
//! stops at "this entry is writable, and it is at position N". The half of the
//! threat-model item that needs an opinion — a writable directory *preceding*
//! the system directories is where a planted binary wins — is a comparison of
//! positions, and it belongs to this layer. These tests pin that boundary: a
//! synthetic snapshot in, findings out, no filesystem consulted.

use macportsbrew_application::{doctor, Severity};
use macportsbrew_domain::{
    Architecture, CommandLineTools, EnvironmentSnapshot, Machine, MacosVersion, PathFinding,
    PathFindingKind, Provenance, Snapshot, Translation, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::BTreeMap;

fn snapshot(path: &[&str], findings: Vec<PathFinding>) -> Snapshot {
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-02T00:00:00Z".into(),
            captured_by: "doctor_path.rs".into(),
            origin: "synthesised".into(),
            note: None,
        },
        machine: Machine {
            macos: MacosVersion::new(26, 0, 0),
            architecture: Architecture::Arm64,
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Installed {
                path: "/Library/Developer/CommandLineTools".into(),
            },
        },
        environment: EnvironmentSnapshot {
            path: path.iter().map(|e| (*e).to_string()).collect(),
            reported: BTreeMap::new(),
            path_findings: findings,
        },
        providers: BTreeMap::new(),
    }
}

fn finding(entry: &str, index: usize, kind: PathFindingKind) -> PathFinding {
    PathFinding {
        entry: entry.into(),
        index,
        kind,
    }
}

fn severity_of(snapshot: &Snapshot, id: &str) -> Option<Severity> {
    let report = doctor(
        snapshot,
        &PolicyData::load_embedded().unwrap(),
        "2026-09-02",
    );
    report
        .findings
        .iter()
        .find(|f| f.id == id)
        .map(|f| f.severity)
}

#[test]
fn a_world_writable_entry_before_the_system_directories_is_an_error() {
    let snap = snapshot(
        &["/opt/open", "/usr/bin", "/bin"],
        vec![finding("/opt/open", 0, PathFindingKind::WorldWritable)],
    );
    assert_eq!(
        severity_of(&snap, "path_world_writable_precedes_system"),
        Some(Severity::Error),
        "anybody with an account can decide what a command means, and the shell \
         reaches their copy first"
    );
}

#[test]
fn the_same_entry_after_the_system_directories_is_only_a_warning() {
    let snap = snapshot(
        &["/usr/bin", "/bin", "/opt/open"],
        vec![finding("/opt/open", 2, PathFindingKind::WorldWritable)],
    );
    assert_eq!(
        severity_of(&snap, "path_world_writable_precedes_system"),
        None
    );
    assert_eq!(
        severity_of(&snap, "path_world_writable"),
        Some(Severity::Warning),
        "still worth saying: the entry shadows anything further down the list"
    );
}

#[test]
fn a_path_with_no_system_directory_at_all_cannot_be_preceded() {
    // `first_system` is `None`, and "before nothing" must not read as "before
    // position zero". A machine whose `PATH` has lost `/usr/bin` has a
    // different problem, and this check is not the one to invent an error for
    // it.
    let snap = snapshot(
        &["/opt/open"],
        vec![finding("/opt/open", 0, PathFindingKind::WorldWritable)],
    );
    assert_eq!(
        severity_of(&snap, "path_world_writable_precedes_system"),
        None
    );
    assert_eq!(
        severity_of(&snap, "path_world_writable"),
        Some(Severity::Warning)
    );
}

#[test]
fn group_writable_missing_relative_and_duplicate_are_warnings() {
    let snap = snapshot(
        &["/usr/local/bin", "/nowhere", "bin", "/usr/bin", "/usr/bin"],
        vec![
            finding("/usr/local/bin", 0, PathFindingKind::GroupWritable),
            finding("/nowhere", 1, PathFindingKind::Missing),
            finding("bin", 2, PathFindingKind::Relative),
            finding("/usr/bin", 4, PathFindingKind::Duplicate),
        ],
    );
    for id in [
        "path_group_writable",
        "path_missing",
        "path_relative",
        "path_duplicate",
    ] {
        assert_eq!(
            severity_of(&snap, id),
            Some(Severity::Warning),
            "{id} should be reported as a warning"
        );
    }
}

#[test]
fn a_snapshot_with_no_findings_says_nothing_about_path_entries() {
    // Includes an old snapshot, whose `pathFindings` key is simply absent:
    // `#[serde(default)]` gives an empty list, and an empty list must not be
    // read as "audited and clean" *or* produce noise.
    let snap = snapshot(&["/usr/bin", "/bin"], Vec::new());
    let report = doctor(&snap, &PolicyData::load_embedded().unwrap(), "2026-09-02");
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.id.starts_with("path_") && f.id != "path_order"),
        "no audit, no path findings"
    );
}

#[test]
fn the_finding_carries_the_redacted_entry_it_was_made_about() {
    // The audit is handed the redacted `PATH`, so the entry a user sees in a
    // diagnostic is the redacted one (`06-cli-and-ux.md`, "Privacy and
    // diagnostics"). Nothing in this layer re-expands it.
    let snap = snapshot(
        &["~/bin", "/usr/bin"],
        vec![finding("~/bin", 0, PathFindingKind::WorldWritable)],
    );
    let report = doctor(&snap, &PolicyData::load_embedded().unwrap(), "2026-09-02");
    let found = report
        .findings
        .iter()
        .find(|f| f.id == "path_world_writable_precedes_system")
        .expect("the entry precedes /usr/bin");
    assert_eq!(
        found.reason.params.get("entry").map(String::as_str),
        Some("~/bin")
    );
}
