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
    // The writable entry sits *after* the system directories here. Before
    // them it is an error now, which is the case below: this fixture used to
    // put `/usr/local/bin` first and still call it a warning, which is the
    // Intel Homebrew machine and the defect F-07 named.
    let snap = snapshot(
        &["/usr/bin", "/usr/local/bin", "/nowhere", "bin", "/usr/bin"],
        vec![
            finding("/usr/local/bin", 1, PathFindingKind::GroupWritable),
            finding("/nowhere", 2, PathFindingKind::Missing),
            finding("bin", 3, PathFindingKind::Relative),
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
fn an_error_is_a_search_path_somebody_else_decides() {
    // `ERROR` is reserved, here and in `doctor.v1.md`, for a setup that will
    // produce wrong answers. Two shapes qualify: a directory *anyone on the
    // machine* can write to, ahead of the system ones, and an entry whose
    // meaning changes with the working directory.
    let world = snapshot(
        &["/opt/open", "/usr/bin"],
        vec![finding("/opt/open", 0, PathFindingKind::WorldWritable)],
    );
    assert_eq!(
        severity_of(&world, "path_world_writable_precedes_system"),
        Some(Severity::Error)
    );
    let empty = snapshot(
        &["", "/usr/bin"],
        vec![finding("", 0, PathFindingKind::Relative)],
    );
    assert_eq!(
        severity_of(&empty, "path_relative_precedes_system"),
        Some(Severity::Error),
        "an empty entry is the working directory, and any repository you clone \
         can carry a `git`"
    );
}

#[test]
fn a_directory_you_own_is_reported_but_is_not_an_error() {
    // The finding is that this produced *nothing at all* before, not that it
    // is a broken machine. `~/bin` ahead of `/usr/bin` is how developers
    // arrange their machines, and `/usr/local/bin` is `admin`-writable because
    // that is how Homebrew installs itself - on the very prefix `doctor`
    // separately warns you about *not* having on your `PATH`. Escalating these
    // made `doctor` exit 5 on a stock install, and an error everybody has is an
    // error nobody reads.
    for kind in [
        PathFindingKind::OwnerWritable,
        PathFindingKind::GroupWritable,
    ] {
        let before = snapshot(&["~/bin", "/usr/bin"], vec![finding("~/bin", 0, kind)]);
        let after = snapshot(&["/usr/bin", "~/bin"], vec![finding("~/bin", 1, kind)]);
        for snap in [&before, &after] {
            let id = if kind == PathFindingKind::OwnerWritable {
                "path_owner_writable"
            } else {
                "path_group_writable"
            };
            assert_eq!(
                severity_of(snap, id),
                Some(Severity::Warning),
                "{id} is reported wherever it sits, and is not an error"
            );
        }
    }
}

#[test]
fn one_directory_yields_one_writable_finding() {
    // `/usr/local/bin` on a stock Intel Homebrew is owned by the installing
    // user *and* group-writable by `admin`, so the audit reports both kinds for
    // one entry. One directory with one remedy counted twice is how a finding
    // count stops meaning anything; the strongest writer is the one reported.
    let snap = snapshot(
        &["/usr/bin", "/usr/local/bin"],
        vec![
            finding("/usr/local/bin", 1, PathFindingKind::GroupWritable),
            finding("/usr/local/bin", 1, PathFindingKind::OwnerWritable),
        ],
    );
    assert_eq!(
        severity_of(&snap, "path_group_writable"),
        Some(Severity::Warning)
    );
    assert_eq!(
        severity_of(&snap, "path_owner_writable"),
        None,
        "the weaker writer is not reported a second time for the same entry"
    );
}

#[test]
fn a_trailing_slash_is_still_a_system_directory() {
    // `/usr/bin/` works, and matching by string equality meant one character
    // bypassed the escalation and fired a no-system-directory error while all
    // four were present and searched.
    let snap = snapshot(
        &["/opt/open", "/usr/bin/", "/bin/"],
        vec![finding("/opt/open", 0, PathFindingKind::WorldWritable)],
    );
    assert_eq!(severity_of(&snap, "path_has_no_system_directory"), None);
    assert_eq!(
        severity_of(&snap, "path_world_writable_precedes_system"),
        Some(Severity::Error)
    );
}

#[test]
fn a_finding_kind_from_a_newer_build_is_reported_rather_than_refused() {
    // Adding a kind must stay additive. Without the unknown-tolerant variant
    // the whole snapshot is rejected on an older build and no analysis happens
    // at all, which is a breaking schema change wearing an additive face.
    let snap = snapshot(
        &["/opt/whatever", "/usr/bin"],
        vec![finding("/opt/whatever", 0, PathFindingKind::Unknown)],
    );
    assert_eq!(
        severity_of(&snap, "path_finding_unknown"),
        Some(Severity::Warning)
    );
}

#[test]
fn an_owner_writable_entry_after_the_system_directories_is_a_warning() {
    let snap = snapshot(
        &["/usr/bin", "/bin", "~/bin"],
        vec![finding("~/bin", 2, PathFindingKind::OwnerWritable)],
    );
    assert_eq!(
        severity_of(&snap, "path_owner_writable_precedes_system"),
        None
    );
    assert_eq!(
        severity_of(&snap, "path_owner_writable"),
        Some(Severity::Warning)
    );
}

#[test]
fn a_path_with_no_system_directory_is_itself_reported() {
    // The other half of "before nothing is not before position zero": the
    // escalations below correctly do not fire, and that used to be the end of
    // it, so the machine with the strangest search path got the quietest
    // report. It is a finding in its own right now.
    let snap = snapshot(
        &["/opt/open"],
        vec![finding("/opt/open", 0, PathFindingKind::WorldWritable)],
    );
    assert_eq!(
        severity_of(&snap, "path_has_no_system_directory"),
        Some(Severity::Error)
    );
    let ordinary = snapshot(&["/usr/bin", "/bin"], vec![]);
    assert_eq!(severity_of(&ordinary, "path_has_no_system_directory"), None);
    // And an empty `PATH` is not this finding: there is nothing to be wrong
    // about, and a snapshot from a machine that reported none should not grow
    // an error out of an absence.
    let empty = snapshot(&[], vec![]);
    assert_eq!(severity_of(&empty, "path_has_no_system_directory"), None);
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
