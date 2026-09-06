// SPDX-License-Identifier: Apache-2.0

//! `doctor` findings that no test reached.
//!
//! `doctor_path.rs` covers the `PATH` findings thoroughly. Four other branches
//! had no test at all, and the first coverage run this repository has ever
//! done found them (DP-015): a provider built for a different architecture
//! than the machine, the `PATH` order *between* two providers, Command Line
//! Tools missing or unknown, and policy data old enough to warn about.
//!
//! Each is something a user is shown and acts on, which is the argument for
//! testing them rather than the line count. Every assertion here is on the
//! finding's identifier and severity — never on English, which lives in the
//! catalogue and is free to change.

use macportsbrew_application::{doctor, Severity};
use macportsbrew_domain::{
    Architecture, CommandLineTools, EnvironmentSnapshot, Machine, MacosVersion, Provenance,
    ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory, ProviderPresence,
    ProviderSnapshot, Snapshot, Translation, Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::BTreeMap;
use std::path::PathBuf;

const HB: &str = "homebrew";
const MP: &str = "macports";

/// A present provider at `prefix`, reporting `arch`.
fn provider(id: &str, prefix: &str, arch: Architecture) -> ProviderSnapshot {
    ProviderSnapshot {
        detection: ProviderDetection {
            provider: ProviderId::new(id),
            presence: ProviderPresence::Present,
            version: Some(Version::new("9.9")),
            prefix: Some(PathBuf::from(prefix)),
            executable: None,
            architecture: Some(arch),
        },
        capabilities: ProviderCapabilities::default(),
        inventory: ProviderInventory::default(),
        environment: BTreeMap::new(),
        invocation_environment: BTreeMap::new(),
        resolutions: BTreeMap::new(),
    }
}

fn snapshot(
    machine_arch: Architecture,
    tools: CommandLineTools,
    path: &[&str],
    providers: Vec<(&str, ProviderSnapshot)>,
) -> Snapshot {
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-05T00:00:00Z".into(),
            captured_by: "doctor_findings.rs".into(),
            origin: "synthesised".into(),
            note: None,
        },
        machine: Machine {
            macos: MacosVersion::new(26, 0, 0),
            architecture: machine_arch,
            translation: Translation::Native,
            command_line_tools: tools,
        },
        environment: EnvironmentSnapshot {
            path: path.iter().map(|e| (*e).to_string()).collect(),
            reported: BTreeMap::new(),
            path_findings: Vec::new(),
        },
        providers: providers
            .into_iter()
            .map(|(id, p)| (ProviderId::new(id), p))
            .collect(),
    }
}

fn installed() -> CommandLineTools {
    CommandLineTools::Installed {
        path: "/Library/Developer/CommandLineTools".into(),
    }
}

/// Every finding with `id`, as `(severity, reason identifier)`.
fn findings(snapshot: &Snapshot, today: &str, id: &str) -> Vec<(Severity, String)> {
    let report = doctor(
        snapshot,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        today,
    );
    report
        .findings
        .iter()
        .filter(|f| f.id == id)
        .map(|f| (f.severity, f.reason.id.as_str().to_string()))
        .collect()
}

const TODAY: &str = "2026-09-05";

#[test]
fn a_provider_built_for_another_architecture_is_a_warning_naming_both() {
    // The case this exists for: an Intel Homebrew surviving under Rosetta on an
    // Apple Silicon Mac. It runs, so it is not an error — but every bottle it
    // pours is for the wrong architecture, and nothing else in the output says
    // so.
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &[],
        vec![(HB, provider(HB, "/usr/local", Architecture::X86_64))],
    );
    let found = findings(&snap, TODAY, "provider_architecture_mismatch");
    assert_eq!(found.len(), 1, "expected exactly one mismatch finding");
    assert_eq!(found[0].0, Severity::Warning);
    assert_eq!(found[0].1, "doctor.provider_architecture_mismatch");
}

#[test]
fn a_provider_matching_the_machine_says_nothing_about_architecture() {
    // The other half. Without it the test above would pass on a `doctor` that
    // warned unconditionally.
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &[],
        vec![(HB, provider(HB, "/opt/homebrew", Architecture::Arm64))],
    );
    assert!(findings(&snap, TODAY, "provider_architecture_mismatch").is_empty());
}

#[test]
fn the_path_order_between_two_providers_is_reported_when_both_are_on_it() {
    // Reported, never changed: which provider's `bin` comes first decides
    // which copy of a shared executable runs, and that is the user's to
    // arrange (INV-12 — this is not a global priority the tool sets).
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &["/opt/local/bin", "/opt/homebrew/bin", "/usr/bin"],
        vec![
            (HB, provider(HB, "/opt/homebrew", Architecture::Arm64)),
            (MP, provider(MP, "/opt/local", Architecture::Arm64)),
        ],
    );
    let found = findings(&snap, TODAY, "path_order");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, Severity::Ok);
}

#[test]
fn one_provider_on_the_path_is_not_an_order_to_report() {
    // An order needs two. This is the boundary the `>= 2` guard draws, and
    // without it the test above proves only that the branch runs.
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &["/opt/homebrew/bin", "/usr/bin"],
        vec![
            (HB, provider(HB, "/opt/homebrew", Architecture::Arm64)),
            (MP, provider(MP, "/opt/local", Architecture::Arm64)),
        ],
    );
    assert!(findings(&snap, TODAY, "path_order").is_empty());
}

#[test]
fn command_line_tools_missing_or_unknown_are_both_warnings_and_are_distinguishable() {
    // Two different facts, and the difference matters: *missing* means a
    // source build will fail, *unknown* means the tool could not tell. Folding
    // them together would report a certainty nobody has.
    for (tools, expected) in [
        (
            CommandLineTools::Missing,
            "doctor.command_line_tools_missing",
        ),
        (
            CommandLineTools::Unknown,
            "doctor.command_line_tools_unknown",
        ),
    ] {
        let snap = snapshot(Architecture::Arm64, tools, &[], vec![]);
        let found = findings(&snap, TODAY, "command_line_tools");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, Severity::Warning);
        assert_eq!(found[0].1, expected, "the two must stay distinguishable");
    }

    let snap = snapshot(Architecture::Arm64, installed(), &[], vec![]);
    let found = findings(&snap, TODAY, "command_line_tools");
    assert_eq!(found[0].0, Severity::Ok);
    assert_eq!(found[0].1, "doctor.command_line_tools_present");
}

#[test]
fn policy_data_older_than_the_staleness_window_warns_and_says_when_it_was_read() {
    // `stale_after_days` is 180. The notice does not refuse to answer — the
    // recommendations still stand — it says the facts behind them are old,
    // which is the whole point of every entry carrying a read date (INV-20).
    let far_future = "2030-01-01";
    let snap = snapshot(Architecture::Arm64, installed(), &[], vec![]);

    let stale = findings(&snap, far_future, "policy_stale");
    assert_eq!(stale.len(), 1, "the data must be stale in 2030");
    assert_eq!(stale[0].0, Severity::Warning);
    assert_eq!(stale[0].1, "policy.stale_notice");

    assert!(
        findings(&snap, TODAY, "policy_stale").is_empty(),
        "the data must not be stale on the day it was read, or this asserts nothing"
    );
}
