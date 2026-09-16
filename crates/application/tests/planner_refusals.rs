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

/// F-48: policy load must not accept a curated name the planner will refuse.
///
/// `check_curated_names` in `PolicyData::load_from` applies
/// `is_safe_provider_name`, which is the *last* of the three gates
/// `planner::command` runs on a package name. The first is the provider's
/// reserved-word list from `provider-commands.toml` — thirteen words for
/// MacPorts, `all`, `installed`, `active`, `leaves` among them. A curated row
/// whose destination is one of those loads, plans, and fails at the planner with
/// `ErrorId::Usage`: *"That command line is not valid … Run macportsbrew
/// --help"*. The user's command line was valid and `--help` cannot help them,
/// which is a worse misattribution than the one F-48 was filed about.
///
/// The two lists do not meet in either layer. `policy` *could* read
/// `provider-commands.toml` — it is in `data/policy/` beside the six files
/// `policy` already embeds — but the parse lives here, in
/// `provider_commands.rs`, together with the absoluteness check on `executable`,
/// so a copy in `policy` would be two spellings of one file. `build_plan` does
/// see both — it takes `&PolicyData` and calls `provider_commands::load()` — so
/// the containment could be checked at runtime; it is not, because a per-request
/// path should validate the request rather than rescan a compiled-in table on
/// every plan. The whole-table property belongs here. `security-reviewer` found
/// the gap while reviewing the fix for the other half.
///
/// This asserts containment, not the absence of a check: policy accepting a name
/// the planner rejects is the defect, and either list moving makes it fail.
///
/// **The reserved half has teeth on one side only, today.** Exactly one provider
/// declares `reserved_names` — MacPorts, thirteen words — and every shipped
/// curated row runs homebrew to macports, so the destination names are compared
/// against a real list and the source names against an empty one. That is the
/// half that matters (a destination name is what reaches `command()` as
/// `dest_name`), and SE-051 already records the asymmetry as open research
/// rather than a decision. It is said here because a reader would otherwise take
/// "asserted where both are visible" as covering both directions, and the
/// counted assertion below fails if the last real list is ever emptied.
#[test]
fn every_curated_name_is_one_the_planner_would_accept() {
    let data = macportsbrew_policy::PolicyData::load_embedded().expect("embedded policy data");
    let commands = macportsbrew_application::provider_commands::load().expect("provider commands");
    let rows = data.curated_mappings();
    assert!(!rows.is_empty(), "an empty table would assert nothing");
    let mut compared_against_a_real_list = 0usize;

    for row in rows {
        for (provider, name) in [
            (&row.source_provider, &row.source_name),
            (&row.destination_provider, &row.destination_name),
        ] {
            assert!(
                macportsbrew_domain::is_safe_provider_name(name),
                "curated {provider} name {name:?} is not one a provider could report"
            );
            assert!(
                !name.is_empty(),
                "curated {provider} name is empty, which the planner calls an internal error"
            );
            let entry = commands.get(provider.as_str()).unwrap_or_else(|| {
                panic!(
                    "curated row names provider {provider:?}, which provider-commands.toml \
                     does not describe - so the reserved-word half of this test would \
                     silently check nothing"
                )
            });
            {
                assert!(
                    !entry
                        .reserved_names
                        .iter()
                        .any(|reserved| reserved.eq_ignore_ascii_case(name)),
                    "curated {provider} name {name:?} is a word that provider reads as a set \
                     of ports; the planner refuses it as a usage error, blaming a command line \
                     that was correct (F-48)"
                );
                if !entry.reserved_names.is_empty() {
                    compared_against_a_real_list += 1;
                }
            }
        }
    }

    // Without this the reserved half can go quiet without failing: empty the one
    // provider list that has words in it, or add rows in a direction no provider
    // describes, and every assertion above still passes while checking nothing.
    assert!(
        compared_against_a_real_list > 0,
        "no curated name was compared against a non-empty reserved-word list, so \
         this test's reserved half asserted nothing"
    );
}
