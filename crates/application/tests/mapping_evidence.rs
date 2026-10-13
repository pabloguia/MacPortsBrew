// SPDX-License-Identifier: Apache-2.0

//! Two mapping signals that no test exercised.
//!
//! A mapping without evidence is a bug (`AGENTS.md`), so every signal that can
//! raise a candidate's score has to be reachable and has to record *why*. The
//! first coverage run this repository has ever done (DP-015) found two that
//! nothing reached: matching **executables**, and a versioned family whose
//! versions differ.
//!
//! Neither stands alone. `CANDIDATE_THRESHOLD` is 30; matching executables
//! weigh 20 and a family whose version differs weighs 10, so both are
//! *contributions* to a candidate another signal already raised — deliberately,
//! and the comment above the constant says why: listing a differing version at
//! the same confidence as a real match would make every versioned family
//! ambiguous. The first version of these tests asserted each signal produced a
//! candidate by itself, which is not what the code does and never was.
//!
//! So each test pairs the signal with a homepage match (30) and asserts the
//! contribution is *recorded*, not that it wins. The evidence is the contract;
//! the weights are a tuning decision.

use macportsbrew_application::mapping::{map_package, normalise};
use macportsbrew_domain::{
    InstalledPackage, PackageIdentity, PackageKind, PackageMetadata, ProviderId, ProviderInventory,
    Version,
};
use macportsbrew_policy::PolicyData;
use std::collections::BTreeSet;

const HB: &str = "homebrew";
const MP: &str = "macports";

/// A destination inventory holding one package with the given metadata.
fn destination_with(name: &str, meta: PackageMetadata) -> ProviderInventory {
    let mut inv = ProviderInventory::default();
    inv.installed.push(InstalledPackage {
        identity: PackageIdentity::new(ProviderId::new(MP), name),
        kind: PackageKind::Package,
        version: Version::new("1.0"),
        revision: None,
        variants: BTreeSet::new(),
        active: true,
        requested: Some(true),
        installed_from_prebuilt: Some(true),
        install_path: None,
    });
    inv.metadata.insert(name.to_string(), meta);
    inv
}

fn kinds_of(
    source_name: &str,
    source_meta: &PackageMetadata,
    dest: &ProviderInventory,
) -> Vec<String> {
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    let mapping = map_package(
        &PackageIdentity::new(ProviderId::new(HB), source_name),
        Some(source_meta),
        &ProviderId::new(MP),
        Some(dest),
        &policy,
    );
    mapping
        .candidates
        .iter()
        .flat_map(|c| c.evidence.iter().map(|e| e.kind.clone()))
        .collect()
}

#[test]
fn two_packages_installing_the_same_executables_are_evidence_of_one_package() {
    // The strongest name-independent signal there is: whatever the two
    // providers call it, a package that puts the same programs on disk is the
    // same software. Names differ here on purpose, so nothing else can be
    // producing the match.
    let exes: BTreeSet<String> = ["ag", "ack"].iter().map(|s| (*s).to_string()).collect();
    let home = Some("https://example.org/ag".to_string());
    let dest = destination_with(
        "the-silver-searcher",
        PackageMetadata {
            executables: exes.clone(),
            homepage: home.clone(),
            ..PackageMetadata::default()
        },
    );
    let kinds = kinds_of(
        "ag",
        &PackageMetadata {
            executables: exes,
            homepage: home,
            ..PackageMetadata::default()
        },
        &dest,
    );
    assert!(
        kinds.iter().any(|k| k == "executables"),
        "matching executables must be recorded as evidence: {kinds:?}"
    );
}

#[test]
fn an_empty_executable_set_is_not_a_match_with_another_empty_one() {
    // The guard that makes the signal mean something. Most packages report no
    // executables at all, and `{} == {}` would otherwise map every one of them
    // to every other.
    // Paired with the same homepage, so the candidate exists and the only
    // question is whether `executables` appears in its evidence.
    let home = Some("https://example.org/ag".to_string());
    let dest = destination_with(
        "unrelated",
        PackageMetadata {
            homepage: home.clone(),
            ..PackageMetadata::default()
        },
    );
    let kinds = kinds_of(
        "ag",
        &PackageMetadata {
            homepage: home,
            ..PackageMetadata::default()
        },
        &dest,
    );
    assert!(
        kinds.iter().any(|k| k == "homepage"),
        "the pairing signal must have produced a candidate, or this proves nothing: {kinds:?}"
    );
    assert!(
        !kinds.iter().any(|k| k == "executables"),
        "two packages that report no executables are not evidence of anything: {kinds:?}"
    );
}

#[test]
fn a_family_whose_versions_differ_is_weaker_evidence_but_still_evidence() {
    // `python@3.13` against `python312`: the same family, a different version.
    // Ten points — not enough to raise a candidate alone, which is the point of
    // the branch existing separately from the exact-version one. Paired with a
    // homepage match here so the candidate exists and the weaker signal can be
    // seen in its evidence.
    let home = Some("https://www.python.org".to_string());
    let dest = destination_with(
        "python312",
        PackageMetadata {
            homepage: home.clone(),
            ..PackageMetadata::default()
        },
    );
    let kinds = kinds_of(
        "python@3.13",
        &PackageMetadata {
            homepage: home,
            ..PackageMetadata::default()
        },
        &dest,
    );
    assert!(
        kinds.iter().any(|k| k == "versioned_family"),
        "a family match with a differing version must still be recorded: {kinds:?}"
    );
}

#[test]
fn normalise_is_what_makes_the_two_spellings_comparable() {
    // The function every branch above leans on, asserted directly so a change
    // to it fails here rather than somewhere downstream.
    assert_eq!(
        normalise("The-Silver_Searcher"),
        normalise("thesilversearcher")
    );
}
