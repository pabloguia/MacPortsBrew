// SPDX-License-Identifier: Apache-2.0

//! What the planner refuses to plan, and why.
//!
//! `planner.rs` had **no unit tests at all** — every line it covered was
//! covered incidentally, by integration tests asking for plans that succeed.
//! The first coverage measurement this repository has ever run put the file at
//! 83.99%, and the uncovered lines were almost entirely its `return Err` arms
//! (DP-015).
//!
//! That is the wrong half to leave untested. `07-quality-and-testing.md` puts
//! planning at a 95% floor for a stated reason — "this is the code that can
//! destroy a machine" — and a planner's refusals *are* its safety behaviour: a
//! plan that should never have been produced is the failure mode, not a plan
//! that is slightly wrong. Each test here names the refusal rather than the
//! line, so it stays meaningful if the code moves.
//!
//! The fixtures are deliberately minimal — no metadata, no availability, no
//! dependency edges — because a refusal happens before any of that is read.
//! The richer two-provider fixture lives in `invariants.rs` and is built for a
//! different question.

use macportsbrew_application::{build_plan, explain_package, AnalysisOptions, PlanRequest};
use macportsbrew_domain::{
    Architecture, CommandLineTools, EnvironmentSnapshot, ErrorId, InstalledPackage, Machine,
    MacosVersion, PackageIdentity, PackageKind, PackageMetadata, Provenance, ProviderCapabilities,
    ProviderDetection, ProviderId, ProviderInventory, ProviderPresence, ProviderSnapshot,
    RequestedSet, Snapshot, Translation, Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const HB: &str = "homebrew";
const MP: &str = "macports";

/// A provider holding exactly the named packages, all requested.
///
/// `(name, homepage)`: a shared homepage is how the mapping engine discovers a
/// counterpart, and therefore how one provider's single name comes to be the
/// counterpart of several of another's — the ambiguity the planner refuses.
fn holding(id: &str, prefix: &str, names: &[(&str, Option<&str>)]) -> ProviderSnapshot {
    let pid = ProviderId::new(id);
    let mut inventory = ProviderInventory::default();
    for (name, homepage) in names {
        inventory.installed.push(InstalledPackage {
            identity: PackageIdentity::new(pid.clone(), *name),
            kind: PackageKind::Package,
            version: Version::new("1.0"),
            revision: None,
            variants: BTreeSet::new(),
            active: true,
            requested: Some(true),
            installed_from_prebuilt: Some(true),
            install_path: None,
        });
        inventory.requested.names.insert((*name).to_string());
        inventory.metadata.insert(
            (*name).to_string(),
            PackageMetadata {
                homepage: homepage.map(str::to_string),
                ..PackageMetadata::default()
            },
        );
    }
    inventory.requested = RequestedSet {
        names: inventory.requested.names.clone(),
        mechanism: "test".into(),
    };
    ProviderSnapshot {
        detection: ProviderDetection {
            provider: pid,
            presence: ProviderPresence::Present,
            version: Some(Version::new("9.9")),
            prefix: Some(PathBuf::from(prefix)),
            executable: None,
            architecture: Some(Architecture::Arm64),
        },
        capabilities: ProviderCapabilities::default(),
        inventory,
        environment: BTreeMap::new(),
        invocation_environment: [("PATH".to_string(), format!("{prefix}/bin:/usr/bin:/bin"))]
            .into_iter()
            .collect(),
        resolutions: BTreeMap::new(),
    }
}

fn snapshot_of(providers: Vec<(&str, ProviderSnapshot)>) -> Snapshot {
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-05T00:00:00Z".into(),
            captured_by: "planner_refusals.rs".into(),
            origin: "synthesised".into(),
            note: None,
        },
        machine: Machine {
            architecture: Architecture::Arm64,
            macos: MacosVersion::new(15, 6, 1),
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Installed {
                path: PathBuf::from("/Library/Developer/CommandLineTools"),
            },
        },
        environment: EnvironmentSnapshot::default(),
        providers: providers
            .into_iter()
            .map(|(id, p)| (ProviderId::new(id), p))
            .collect(),
    }
}

fn both(hb: &[(&str, Option<&str>)], mp: &[(&str, Option<&str>)]) -> Snapshot {
    snapshot_of(vec![
        (HB, holding(HB, "/opt/homebrew", hb)),
        (MP, holding(MP, "/opt/local", mp)),
    ])
}

fn plan_for(snapshot: &Snapshot, package: &str, to: &str) -> Result<(), (ErrorId, String)> {
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    let options = AnalysisOptions::default();
    build_plan(
        snapshot,
        &policy,
        &options,
        &PlanRequest {
            package: package.to_string(),
            to: ProviderId::new(to),
            retain_source: false,
        },
    )
    .map(|_| ())
    .map_err(|e| (e.id, e.detail))
}

#[test]
fn a_package_no_provider_holds_is_refused_by_name() {
    let snap = both(&[("wget", None)], &[("curl", None)]);
    let (id, detail) = plan_for(&snap, "nothing-like-this", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::PackageNotFound);
    assert!(
        detail.contains("nothing-like-this") && detail.contains("not installed"),
        "the refusal must name the package the user asked for: {detail}"
    );
}

#[test]
fn a_package_only_the_destination_holds_is_refused_as_nothing_to_move() {
    // Not an error about the package: the request is coherent and there is
    // simply nothing to do. Producing an empty plan here would be worse — it
    // would read as "this migration is safe" for a migration that is not
    // happening.
    let snap = both(&[], &[("curl", None)]);
    let (id, detail) = plan_for(&snap, "curl", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::Usage);
    assert!(
        detail.contains("already held by") && detail.contains(MP),
        "the refusal must say who already holds it: {detail}"
    );
}

#[test]
fn a_destination_the_snapshot_does_not_know_is_refused() {
    // A snapshot carrying one provider, asked to plan toward another. The
    // guard exists because a missing destination would otherwise be read as a
    // destination holding nothing, which is exactly the shape of a valid
    // migration.
    let snap = snapshot_of(vec![(HB, holding(HB, "/opt/homebrew", &[("wget", None)]))]);
    let (id, detail) = plan_for(&snap, "wget", MP).expect_err("must refuse");
    assert_eq!(id, ErrorId::ProviderNotFound);
    assert!(
        detail.contains(MP),
        "the refusal must name the destination it does not know: {detail}"
    );
}

#[test]
fn a_name_that_is_the_counterpart_of_several_packages_is_refused_with_the_choices() {
    // The regression this arm was written for: one provider's single name can
    // be another's several. Taking the first was silent and produced a plan
    // for a *different package* than the one named. Ambiguity never acts
    // automatically — it reports the choices, like every other ambiguous match
    // in this project.
    //
    // Built the way the analysis produces it: two Homebrew units whose
    // MacPorts counterpart row carries the same name.
    let snap = both(
        &[
            ("python@3.13", Some("https://www.python.org")),
            ("python@3.14", Some("https://www.python.org")),
        ],
        &[("python312", Some("https://www.python.org"))],
    );
    let outcome = plan_for(&snap, "python312", HB);
    let Err((id, detail)) = outcome else {
        // If the curated data stops mapping these two onto one name, this test
        // is no longer exercising ambiguity and must be rebuilt rather than
        // quietly passing.
        panic!(
            "expected an ambiguous match; the fixture no longer produces one, \
             so this test asserts nothing"
        );
    };
    assert_eq!(id, ErrorId::Usage);
    assert!(
        detail.contains("more than one package") && detail.contains("python312"),
        "the refusal must name the ambiguous spelling and the choices: {detail}"
    );
}

// -- explain ----------------------------------------------------------------

#[test]
fn explaining_a_package_no_provider_holds_is_refused_by_name() {
    // `explain` is the command a user reaches for when the answer surprises
    // them, so its own failure has to be legible: the name they typed, and
    // that nothing holds it — not an empty explanation, which would read as
    // "there is nothing to say about this package".
    let snap = both(&[("wget", None)], &[("curl", None)]);
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    let err = explain_package(
        &snap,
        &policy,
        &AnalysisOptions::default(),
        "nothing-like-this",
    )
    .expect_err("must refuse");
    assert_eq!(err.id, ErrorId::PackageNotFound);
    assert!(
        err.detail.contains("nothing-like-this"),
        "the refusal must name the package the user asked for: {}",
        err.detail
    );
}

#[test]
fn explaining_a_package_by_the_other_providers_name_finds_it() {
    // The `or` half of the same lookup: a unit is findable by its own name or
    // by what a provider calls its row. Without this the test above passes on
    // an `explain` that finds nothing at all.
    let snap = both(&[("wget", None)], &[]);
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    explain_package(&snap, &policy, &AnalysisOptions::default(), "wget")
        .expect("a held package must be explainable");
}
