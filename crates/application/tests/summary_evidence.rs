// SPDX-License-Identifier: Apache-2.0

//! The summary says when evidence was unavailable (F-39).
//!
//! Analysing one capture live and replayed gave 56 top-level packages and 49,
//! with nothing in the output saying why. The seven were casks: a cask's
//! `installed_on_request` lives in an on-disk receipt, and a replayed reader
//! must not consult this host's filesystem (INV-15, DP-012, F-25). The
//! behaviour is right — `UNKNOWN` is the honest answer to "what does this
//! recording say about cask intent" — and presenting the smaller number with
//! the same confidence as the larger one is not.
//!
//! Third of a family: F-29 degrades service status to "not a service", F-34
//! degraded `PATH` to "prefix not on `PATH`", this one degrades ownership to
//! `UNKNOWN`. The rule they share: **when a replayed analysis cannot see a
//! class of evidence, the output says so — the count is not the place to
//! discover it.**

use macportsbrew_application::{analyze, AnalysisOptions, AnalysisSource};
use macportsbrew_domain::{
    Architecture, CommandLineTools, EnvironmentSnapshot, InstalledPackage, Machine, MacosVersion,
    PackageIdentity, PackageKind, Provenance, ProviderCapabilities, ProviderDetection, ProviderId,
    ProviderInventory, ProviderPresence, ProviderSnapshot, RequestedSet, Snapshot, Translation,
    Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// One provider holding one package whose recorded intent is `requested`.
/// `None` is the case this test exists for: the provider kept no record, which
/// is what a replayed cask read produces.
fn snapshot_with(requested: Option<bool>) -> Snapshot {
    let pid = ProviderId::new("homebrew");
    let mut inventory = ProviderInventory::default();
    inventory.installed.push(InstalledPackage {
        identity: PackageIdentity::new(pid.clone(), "gimp"),
        kind: PackageKind::GuiApp,
        version: Version::new("1.0"),
        revision: None,
        variants: BTreeSet::new(),
        active: true,
        requested,
        installed_from_prebuilt: Some(true),
        install_path: None,
    });
    if requested == Some(true) {
        inventory.requested.names.insert("gimp".to_string());
    }
    inventory.requested = RequestedSet {
        names: inventory.requested.names.clone(),
        mechanism: "test".into(),
    };
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-07T00:00:00Z".into(),
            captured_by: "summary_evidence.rs".into(),
            origin: "synthesised".into(),
            note: None,
        },
        machine: Machine {
            architecture: Architecture::X86_64,
            macos: MacosVersion::new(26, 0, 0),
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Installed {
                path: PathBuf::from("/Library/Developer/CommandLineTools"),
            },
        },
        environment: EnvironmentSnapshot::default(),
        providers: [(
            pid.clone(),
            ProviderSnapshot {
                detection: ProviderDetection {
                    provider: pid,
                    presence: ProviderPresence::Present,
                    version: Some(Version::new("9.9")),
                    prefix: Some(PathBuf::from("/usr/local")),
                    executable: None,
                    architecture: Some(Architecture::X86_64),
                    prefix_redacted: false,
                },
                capabilities: ProviderCapabilities::default(),
                inventory,
                environment: BTreeMap::new(),
                invocation_environment: BTreeMap::new(),
                resolutions: BTreeMap::new(),
                configuration: macportsbrew_domain::ProviderConfiguration::NotRecorded,
            },
        )]
        .into_iter()
        .collect(),
    }
}

fn unknown_rows(requested: Option<bool>) -> u32 {
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    analyze(
        &snapshot_with(requested),
        &policy,
        &AnalysisOptions::default(),
        AnalysisSource::Snapshot,
    )
    .summary
    .ownership_unknown
}

#[test]
fn a_row_with_no_recorded_intent_is_counted_rather_than_folded_into_the_total() {
    assert_eq!(
        unknown_rows(None),
        1,
        "a provider that kept no record of intent leaves a gap the summary must name"
    );
}

/// A second provider, present and empty, so a curated mapping can name a
/// package in it that is not installed: the `MAPPED` row SE-097 is about.
fn with_an_empty_peer(mut snapshot: Snapshot, peer: &str) -> Snapshot {
    let mut provider = snapshot
        .providers
        .values()
        .next()
        .expect("the snapshot has one provider")
        .clone();
    let pid = ProviderId::new(peer);
    provider.detection.provider = pid.clone();
    provider.detection.prefix = Some(PathBuf::from("/opt/local"));
    provider.inventory = ProviderInventory::default();
    provider.inventory.requested = RequestedSet {
        names: BTreeSet::new(),
        mechanism: "test".into(),
    };
    snapshot.providers.insert(pid, provider);
    snapshot
}

#[test]
fn a_mapped_row_that_is_not_installed_is_not_an_ownership_gap() {
    // SE-097: `status` said "the provider kept no record" of rows that name a
    // package nobody installed. A mapping target has no installation to keep a
    // record of, so it is not missing evidence.
    let policy = PolicyData::load_embedded().expect("the embedded policy data loads");
    let mut snapshot = with_an_empty_peer(snapshot_with(Some(true)), "macports");
    let pid = ProviderId::new("homebrew");
    let source = snapshot.providers.get_mut(&pid).expect("homebrew");
    // `node@20` -> `nodejs20` is a curated row, so the mapping exists whatever
    // the heuristics do; the kind is a formula, not a cask.
    source.inventory.installed[0].identity = PackageIdentity::new(pid.clone(), "node@20");
    source.inventory.installed[0].kind = PackageKind::Package;
    source.inventory.requested.names = ["node@20".to_string()].into_iter().collect();
    let analysis = analyze(
        &snapshot,
        &policy,
        &AnalysisOptions::default(),
        AnalysisSource::Snapshot,
    );
    let mapped = analysis
        .packages
        .iter()
        .flat_map(|p| p.rows.values())
        .filter(|r| r.presence == macportsbrew_application::RowPresence::Mapped)
        .count();
    assert!(
        mapped > 0,
        "precondition: the analysis holds a mapped row, or this test proves nothing"
    );
    assert_eq!(
        analysis.summary.ownership_unknown, 0,
        "a mapped, not-installed row is not a row whose ownership evidence went missing"
    );
}

#[test]
fn a_row_whose_intent_is_recorded_leaves_no_gap_to_report() {
    // Both controls, because a counter that is always 1 would pass the test
    // above and say nothing. `Some(true)` and `Some(false)` are different
    // answers and neither is an absence.
    assert_eq!(unknown_rows(Some(true)), 0, "a requested package is known");
    assert_eq!(unknown_rows(Some(false)), 0, "a dependency is known too");
}
