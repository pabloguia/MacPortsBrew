// SPDX-License-Identifier: Apache-2.0

//! `PROVIDER_REPORTS_ACTIVE` judged over the recordings' own activation reads
//! (work order 16 part S-2; `docs/passes/16-plans/S.md`, Tests, "S-2 replay").
//!
//! Each recording holds the two reads the verify step's second command runs:
//! Homebrew's JSON inventory and MacPorts' installed list. For every package a
//! recording holds, the product's judge (`expectation_for` plus `evaluate_with`
//! over `verification_json::linked_report`) must answer exactly what the
//! recording's own field says — `linked_keg` equal to one of the entry's own
//! installed versions, or exactly one row of the name marked `(active)` — and a
//! cask must never pass. The judge answers from the recorded text alone, never
//! from the live machine (INV-15).
//!
//! The expected counts are **pinned literally**, counted once from each
//! recording when this test was written (2026-10-30), so a change to the
//! recording or to the rule fails here rather than moving both together.
//! Provenance (fixtures README, the recording's own `provenance.origin`):
//! `both-providers-intel-macos26` and `homebrew-only-intel-macos26` are
//! **captured**; `both-providers-arm64-macos15` and
//! `homebrew-deprecated-and-tap-intel-macos26` are **derived** (edited
//! captures); `macports-only-intel-macos12` is **synthesised**. A verdict over
//! a derived or synthesised recording is documentation-derived evidence, not
//! an observation (R-20 E12).

use macportsbrew_application::verification_json::linked_report;
use macportsbrew_domain::plan::PLAN_SCHEMA_VERSION;
use macportsbrew_domain::{
    evaluate_with, expectation_for, CommandTarget, Elevation, MigrationPlan, MigrationStep,
    Outcome, PackageIdentity, PlanCommand, ProcessEnd, ProviderId, ReadOutput, Reason, StepKind,
    SuccessSemantic, Verdict, VerificationCheck,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// A plan whose one step is the destination's verify step, carrying a single
/// activation read of `semantic` for `name`. The argv names no package, as the
/// shipped cells do (`a_verify_active_read_that_names_a_package_has_no_expectation`).
fn plan(name: &str, semantic: SuccessSemantic) -> MigrationPlan {
    MigrationPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        package: PackageIdentity::new(ProviderId::new("src-provider"), name),
        source: ProviderId::new("src-provider"),
        destination: ProviderId::new("dst-provider"),
        destination_package: Some(name.into()),
        retain_source: true,
        entered_by: Some(macportsbrew_domain::EntryMode::DefaultView),
        primary_after: ProviderId::new("dst-provider"),
        recommendation: Outcome::MigrateTo {
            provider: ProviderId::new("dst-provider"),
        },
        overrode: None,
        steps: vec![MigrationStep {
            index: 1,
            kind: StepKind::VerifyDestination,
            provider: Some(ProviderId::new("dst-provider")),
            commands: vec![PlanCommand {
                target: CommandTarget::Provider,
                program: "/p/bin/tool".into(),
                argv: vec!["installed".into()],
                environment: BTreeMap::new(),
                elevation: Elevation::None,
                placeholder: false,
                success: semantic,
            }],
            verification: vec![VerificationCheck::ProviderReportsActive],
            reason: Reason::new("test"),
        }],
        risks: vec![],
        elevated_steps: vec![],
        policy_version: "0".into(),
        dry_run_only: true,
    }
}

/// The product's verdict on `stdout` for `name`.
fn judged(name: &str, semantic: SuccessSemantic, stdout: &str) -> bool {
    let p = plan(name, semantic);
    let expectation = expectation_for(&p, 0, 0).expect("an activation expectation");
    let output = ReadOutput {
        end: ProcessEnd::Exited { code: 0 },
        stdout,
        truncated: false,
    };
    evaluate_with(&expectation, &output, &linked_report) == Verdict::Passed
}

/// The stdout of the recording's one exited invocation whose argv ends with
/// `tail` and whose program is named `program`, asserting it is untruncated.
fn recorded(recording: &str, program: &str, tail: &[&str]) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings")
        .join(format!("{recording}.json"));
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut found = doc["invocations"].as_array().unwrap().iter().filter(|i| {
        let argv: Vec<&str> = i["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap())
            .collect();
        Path::new(i["program"].as_str().unwrap()).file_name() == Some(program.as_ref())
            && argv.ends_with(tail)
    });
    let one = found.next().expect("the read is recorded");
    assert!(found.next().is_none(), "{recording}: one such read");
    assert_eq!(one["status"]["status"], "EXITED", "{recording}");
    assert_eq!(one["truncated"], false, "{recording}");
    one["stdout"].as_str().unwrap().to_owned()
}

/// One linked formula in this many is judged (see `json_inventory`).
const LINKED_STRIDE: usize = 16;

/// `(formulae, linked by the recording's own field, casks)`, after asserting
/// the judge agrees with that field on every formula and refuses every cask.
fn json_inventory(recording: &str) -> (usize, usize, usize) {
    let stdout = recorded(recording, "brew", &["info", "--json=v2", "--installed"]);
    let doc: Value = serde_json::from_str(&stdout).unwrap();
    let formulae = doc["formulae"].as_array().unwrap();
    let mut linked = 0;
    for f in formulae {
        let name = f["name"].as_str().unwrap();
        let own = f["linked_keg"].as_str().is_some_and(|keg| {
            f["installed"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["version"].as_str() == Some(keg))
        });
        // Every entry the recording does not mark linked is judged; of the
        // linked ones, a fixed stride. Each judgement parses the whole
        // document (~900 KB), so judging all of them costs over a minute in a
        // debug build; the count below still covers every entry.
        if !own || linked % LINKED_STRIDE == 0 {
            assert_eq!(
                judged(name, SuccessSemantic::JsonReportsPackageLinked, &stdout),
                own,
                "{recording}: {name}"
            );
        }
        linked += usize::from(own);
    }
    let casks = doc["casks"].as_array().unwrap();
    for c in casks {
        let token = c["token"].as_str().unwrap();
        assert!(
            !judged(token, SuccessSemantic::JsonReportsPackageLinked, &stdout),
            "{recording}: the cask {token} has no linked record and never passes"
        );
    }
    (formulae.len(), linked, casks.len())
}

/// `(names, active by the recording's own marker)`, after asserting the judge
/// agrees with the marker on every name the list holds.
fn installed_list(recording: &str) -> (usize, usize) {
    let stdout = recorded(recording, "port", &["-q", "installed"]);
    let mut rows: BTreeMap<&str, Vec<Vec<&str>>> = BTreeMap::new();
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        rows.entry(tokens[0]).or_default().push(tokens);
    }
    let mut active = 0;
    for (name, of_name) in &rows {
        let marked = of_name
            .iter()
            .filter(|r| r.len() == 3 && r[1].starts_with('@') && r[2] == "(active)")
            .count();
        let known = of_name.iter().all(|r| {
            r[1].starts_with('@') && (r.len() == 2 || (r.len() == 3 && r[2] == "(active)"))
        });
        let own = marked == 1 && known;
        assert_eq!(
            judged(name, SuccessSemantic::StdoutMarksPackageActive, &stdout),
            own,
            "{recording}: {name}"
        );
        active += usize::from(own);
    }
    (rows.len(), active)
}

#[test]
fn invariant_15_a_recorded_json_inventory_is_judged_by_each_entrys_own_linked_keg() {
    // (formulae, linked, casks), counted from each recording on 2026-10-30.
    for (recording, expected) in [
        ("both-providers-intel-macos26", (192, 172, 9)), // captured
        ("homebrew-only-intel-macos26", (180, 160, 9)),  // captured
        ("both-providers-arm64-macos15", (181, 160, 9)), // derived
        ("homebrew-deprecated-and-tap-intel-macos26", (180, 160, 9)), // derived
    ] {
        assert_eq!(json_inventory(recording), expected, "{recording}");
    }
}

#[test]
fn invariant_15_a_recorded_installed_list_is_judged_by_each_names_own_marker() {
    // (names, active), counted from each recording on 2026-10-30. The arm64
    // recording holds one name with an inactive sibling image beside its
    // active one (17 rows, 16 names), and so does the synthesised one.
    for (recording, expected) in [
        ("both-providers-intel-macos26", (18, 18)), // captured
        ("both-providers-arm64-macos15", (16, 16)), // derived
        ("macports-only-intel-macos12", (12, 12)),  // synthesised
    ] {
        assert_eq!(installed_list(recording), expected, "{recording}");
    }
}

/// The judge never consults anything but the text it is handed: an inventory
/// recorded on one machine says nothing about a package it does not hold, even
/// one this machine may have installed (INV-15).
#[test]
fn invariant_15_a_package_the_recording_does_not_hold_is_never_active() {
    {
        let name = "not-a-recorded-package";
        let json = recorded(
            "both-providers-intel-macos26",
            "brew",
            &["info", "--json=v2", "--installed"],
        );
        assert!(!judged(
            name,
            SuccessSemantic::JsonReportsPackageLinked,
            &json
        ));
        let list = recorded("both-providers-intel-macos26", "port", &["-q", "installed"]);
        assert!(!judged(
            name,
            SuccessSemantic::StdoutMarksPackageActive,
            &list
        ));
    }
}
