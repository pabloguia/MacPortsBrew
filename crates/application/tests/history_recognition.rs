// SPDX-License-Identifier: Apache-2.0

//! Recognising a `migrate --retain-source` re-run as already done
//! (`recognise_retained_run`, `migration_start`; pass 08 slice 1). History is
//! never trusted alone: a fresh capture must show the end state, and every
//! case that cannot be ordered or confirmed fails closed — planned again, with
//! the reason said (INV-16). Recognition is direction-neutral (INV-17).

use macportsbrew_application::{
    migration_start, must_record, AnalysisOptions, HistoryConsultation, MigrationStart,
    NotRecognised, PlanRequest,
};
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, Capability,
    CapabilityState, CommandLineTools, Direction, EnvironmentSnapshot, ErrorId, ExecutableOwner,
    HistoryView, InstalledPackage, Machine, MacosVersion, MarkerState, Outcome, PackageIdentity,
    PackageKind, PackageMetadata, Provenance, ProviderCapabilities, ProviderDetection, ProviderId,
    ProviderInventory, ProviderPresence, ProviderSnapshot, RecordedStep, RecordedVersions,
    RequestedSet, Resolution, RunEnding, RunRecord, RunStart, RunSubject, Snapshot, StepKind,
    StepResult, Translation, Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const HB: &str = "homebrew";
const MP: &str = "macports";
const RUN_STAMP: &str = "20261004T130000Z";
const RECORD_ID: &str = "20261004T120000Z-0000000000000001";

/// The curated pair this suite migrates, from either side: `(source,
/// source name, destination, destination name)`.
fn pair(source: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    if source == MP {
        (MP, "gsed", HB, "gnu-sed")
    } else {
        (HB, "gnu-sed", MP, "gsed")
    }
}

fn installed(
    provider: &ProviderId,
    name: &str,
    active: bool,
    service: bool,
) -> (InstalledPackage, PackageMetadata) {
    (
        InstalledPackage {
            identity: PackageIdentity::new(provider.clone(), name),
            kind: PackageKind::Package,
            version: Version::new("1.0"),
            revision: None,
            variants: BTreeSet::new(),
            active,
            requested: Some(true),
            installed_from_prebuilt: Some(true),
            install_path: None,
        },
        PackageMetadata {
            defines_service: service,
            ..PackageMetadata::default()
        },
    )
}

fn provider(
    id: &str,
    prefix: &str,
    rows: Vec<(InstalledPackage, PackageMetadata)>,
) -> ProviderSnapshot {
    let pid = ProviderId::new(id);
    let mut inventory = ProviderInventory::default();
    let mut names = BTreeSet::new();
    for (row, metadata) in rows {
        let name = row.identity.name.clone();
        names.insert(name.clone());
        inventory.metadata.insert(name.clone(), metadata);
        inventory.availability.insert(
            name,
            AvailabilityAssessment {
                availability: ArtifactAvailability::BinaryAvailable,
                evidence: vec![AvailabilityEvidence {
                    source: "test".into(),
                    detail: "prebuilt".into(),
                    offline: true,
                }],
                cached: false,
            },
        );
        inventory.installed.push(row);
    }
    inventory.requested = RequestedSet {
        names,
        mechanism: "test".into(),
    };
    let mut capabilities = ProviderCapabilities::default();
    capabilities.capabilities.insert(
        Capability::MachineReadableInventory,
        CapabilityState::Available,
    );
    ProviderSnapshot {
        detection: ProviderDetection {
            provider: pid,
            presence: ProviderPresence::Present,
            version: Some(Version::new("9.9")),
            prefix: Some(PathBuf::from(prefix)),
            executable: None,
            architecture: Some(Architecture::Arm64),
            prefix_redacted: false,
        },
        capabilities,
        inventory,
        environment: BTreeMap::new(),
        invocation_environment: BTreeMap::new(),
        resolutions: BTreeMap::new(),
        configuration: macportsbrew_domain::ProviderConfiguration::NotRecorded,
    }
}

fn prefix(id: &str) -> &'static str {
    if id == HB {
        "/opt/homebrew"
    } else {
        "/opt/local"
    }
}

/// How the machine looks to the fresh capture.
struct After {
    destination_active: bool,
    owner: Option<ExecutableOwner>,
    service: bool,
}

impl After {
    fn done(destination: &str) -> Self {
        Self {
            destination_active: true,
            owner: Some(ExecutableOwner::Provider {
                provider: ProviderId::new(destination),
                package: None,
            }),
            service: false,
        }
    }
}

/// The machine after a retain-source run from `source`: the source still holds
/// its package and the destination holds the mapped one.
fn fresh(source: &str, after: &After) -> Snapshot {
    let (src, src_name, dst, dst_name) = pair(source);
    let mut source_snapshot = provider(
        src,
        prefix(src),
        vec![installed(
            &ProviderId::new(src),
            src_name,
            true,
            after.service,
        )],
    );
    let mut destination_snapshot = provider(
        dst,
        prefix(dst),
        vec![installed(
            &ProviderId::new(dst),
            dst_name,
            after.destination_active,
            after.service,
        )],
    );
    if let Some(owner) = &after.owner {
        let resolution = Resolution {
            executable: dst_name.into(),
            path: Some(PathBuf::from(format!("/somewhere/{dst_name}"))),
            canonical_path: None,
            owner: owner.clone(),
            authoritative: false,
        };
        destination_snapshot
            .resolutions
            .insert(dst_name.into(), resolution.clone());
        source_snapshot
            .resolutions
            .insert(dst_name.into(), resolution);
    }
    let mut providers = BTreeMap::new();
    providers.insert(ProviderId::new(src), source_snapshot);
    providers.insert(ProviderId::new(dst), destination_snapshot);
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-10-04T13:00:00Z".into(),
            captured_by: "history_recognition.rs".into(),
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
        providers,
    }
}

/// A record of a run from `source` that ended `ending`.
fn record(source: &str, id: &str, ending: RunEnding, retain_source: bool) -> RunRecord {
    let (src, src_name, dst, dst_name) = pair(source);
    let destination = ProviderId::new(dst);
    let steps = [
        StepKind::SnapshotBefore,
        StepKind::AddDestination,
        StepKind::VerifyDestination,
        StepKind::RecordHistory,
    ]
    .into_iter()
    .zip(1..)
    .map(|(kind, index)| {
        RecordedStep::new(
            index,
            kind,
            None,
            StepResult::Verified {
                evidence: Vec::new(),
            },
            false,
        )
    })
    .collect();
    let stamp = &id[..16];
    RunRecord::new(
        RunStart {
            id: id.into(),
            started_at: format!(
                "{}-{}-{}T{}:{}:{}Z",
                &stamp[0..4],
                &stamp[4..6],
                &stamp[6..8],
                &stamp[9..11],
                &stamp[11..13],
                &stamp[13..15]
            ),
            tool_version: "0.1.0".into(),
        },
        RunSubject {
            plan_schema_version: 1,
            policy_version: "test".into(),
            package: PackageIdentity::new(ProviderId::new(src), src_name),
            direction: Direction {
                source: ProviderId::new(src),
                destination: destination.clone(),
            },
            destination_package: Some(dst_name.into()),
            versions: RecordedVersions {
                source: Some(Version::new("1.0")),
                destination: None,
            },
            action: Outcome::MigrateTo {
                provider: destination.clone(),
            },
            overrode: None,
            retain_source,
            primary_after: destination,
        },
        steps,
        ending,
        None,
    )
}

fn success(source: &str) -> RunRecord {
    record(source, RECORD_ID, RunEnding::Completed, true)
}

fn start(
    source: &str,
    snapshot: &Snapshot,
    view: Option<&HistoryView>,
    retain_source: bool,
) -> MigrationStart {
    let (_, src_name, dst, _) = pair(source);
    let history = view.map_or(HistoryConsultation::NotConsulted, |view| {
        HistoryConsultation::Consulted {
            view,
            run_stamp: RUN_STAMP,
        }
    });
    migration_start(
        snapshot,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        &AnalysisOptions {
            today: "2026-10-04".into(),
            advanced: false,
            only: BTreeSet::new(),
        },
        &PlanRequest {
            package: src_name.into(),
            to: ProviderId::new(dst),
            retain_source,
        },
        &snapshot.providers.keys().cloned().collect::<Vec<_>>(),
        history,
    )
    .expect("migration_start answers")
}

fn view_of(records: Vec<RunRecord>) -> HistoryView {
    let newest = records.iter().map(|r| r.id()[..16].to_owned()).max();
    HistoryView::new(newest, records, MarkerState::Absent)
}

/// The note a re-run was planned with, or a panic naming what came back.
fn planned_with(start: MigrationStart) -> Option<NotRecognised> {
    match start {
        MigrationStart::Run(_, note) => note,
        other => panic!("expected a plan, got {other:?}"),
    }
}

#[test]
fn invariant_17_retained_run_recognition_is_direction_neutral() {
    for source in [MP, HB] {
        let (_, src_name, dst, _) = pair(source);
        let snapshot = fresh(source, &After::done(dst));
        match start(
            source,
            &snapshot,
            Some(&view_of(vec![success(source)])),
            true,
        ) {
            MigrationStart::RetainedRunDone {
                package,
                provider,
                run_id,
            } => {
                assert_eq!(package, src_name);
                assert_eq!(provider, ProviderId::new(dst));
                assert_eq!(run_id, RECORD_ID);
            }
            other => panic!("{source}: expected recognition, got {other:?}"),
        }
    }
}

#[test]
fn without_a_consulted_history_a_retained_re_run_is_planned_again() {
    for source in [MP, HB] {
        let (_, _, dst, _) = pair(source);
        let snapshot = fresh(source, &After::done(dst));
        assert_eq!(planned_with(start(source, &snapshot, None, true)), None);
        assert_eq!(
            planned_with(start(source, &snapshot, Some(&HistoryView::empty()), true)),
            None,
            "an empty history is no record, and says nothing"
        );
    }
}

#[test]
fn invariant_16_a_later_failure_withdraws_recognition() {
    let snapshot = fresh(MP, &After::done(HB));
    let later = record(
        MP,
        "20261004T125000Z-0000000000000002",
        RunEnding::Halted {
            step: 2,
            error: ErrorId::SafetyBlocked,
        },
        true,
    );
    let note = planned_with(start(MP, &snapshot, Some(&view_of(vec![later])), true));
    assert!(
        matches!(note, Some(NotRecognised::LatestRunNotSuccess { .. })),
        "{note:?}"
    );
}

#[test]
fn invariant_16_a_run_that_removed_its_source_is_not_a_retained_run() {
    let snapshot = fresh(MP, &After::done(HB));
    let removed = record(MP, RECORD_ID, RunEnding::Completed, false);
    let note = planned_with(start(MP, &snapshot, Some(&view_of(vec![removed])), true));
    assert!(
        matches!(note, Some(NotRecognised::LatestRunRemovedSource { .. })),
        "{note:?}"
    );
}

#[test]
fn invariant_16_two_records_in_one_second_are_not_recognised() {
    let snapshot = fresh(MP, &After::done(HB));
    let other = record(
        MP,
        "20261004T120000Z-0000000000000009",
        RunEnding::Completed,
        true,
    );
    let note = planned_with(start(
        MP,
        &snapshot,
        Some(&view_of(vec![success(MP), other])),
        true,
    ));
    assert_eq!(note, Some(NotRecognised::Tie));
}

#[test]
fn invariant_16_a_record_from_the_future_is_not_recognised() {
    let snapshot = fresh(MP, &After::done(HB));
    // The key's latest is a success, but some record is stamped after this
    // run began: the clock moved, so no order is trusted.
    let view = HistoryView::new(
        Some("20261004T140000Z".into()),
        vec![success(MP)],
        MarkerState::Absent,
    );
    let note = planned_with(start(MP, &snapshot, Some(&view), true));
    assert_eq!(note, Some(NotRecognised::RecordFromTheFuture));
}

#[test]
fn invariant_16_a_destination_that_is_not_active_is_not_recognised() {
    let snapshot = fresh(
        MP,
        &After {
            destination_active: false,
            ..After::done(HB)
        },
    );
    let note = planned_with(start(
        MP,
        &snapshot,
        Some(&view_of(vec![success(MP)])),
        true,
    ));
    assert!(
        matches!(note, Some(NotRecognised::DestinationNotActive { .. })),
        "{note:?}"
    );
}

#[test]
fn invariant_16_a_resolution_that_is_not_the_records_primary_is_not_recognised() {
    for owner in [
        Some(ExecutableOwner::Provider {
            provider: ProviderId::new(MP),
            package: None,
        }),
        Some(ExecutableOwner::Unknown),
        Some(ExecutableOwner::External),
        Some(ExecutableOwner::System),
        None,
    ] {
        let snapshot = fresh(
            MP,
            &After {
                owner: owner.clone(),
                ..After::done(HB)
            },
        );
        let note = planned_with(start(
            MP,
            &snapshot,
            Some(&view_of(vec![success(MP)])),
            true,
        ));
        assert!(
            matches!(note, Some(NotRecognised::PrimaryNotConfirmed { .. })),
            "{owner:?}: {note:?}"
        );
    }
}

#[test]
fn invariant_16_a_retained_run_whose_primary_is_the_shim_is_not_recognised_and_says_why() {
    // Every plan sets its primary through MacPortsBrew's shim, so this is the
    // realistic case after a real run, and it is why INV-16 stays open (D3).
    for source in [MP, HB] {
        let (_, _, dst, _) = pair(source);
        let snapshot = fresh(
            source,
            &After {
                owner: Some(ExecutableOwner::MacportsbrewShim),
                ..After::done(dst)
            },
        );
        let note = planned_with(start(
            source,
            &snapshot,
            Some(&view_of(vec![success(source)])),
            true,
        ));
        assert_eq!(
            note,
            Some(NotRecognised::PrimaryIsShim {
                run_id: RECORD_ID.into()
            })
        );
    }
}

#[test]
fn a_re_run_without_retain_source_is_never_recognised() {
    let snapshot = fresh(MP, &After::done(HB));
    let start = start(MP, &snapshot, Some(&view_of(vec![success(MP)])), false);
    assert!(matches!(start, MigrationStart::Run(_, None)), "{start:?}");
}

#[test]
fn recognition_never_replaces_a_plan_that_needs_manual_review() {
    // A service is a manual review: the safety stop keeps its precedence over
    // "already done", whatever history says (pass 08 plan, M3).
    let snapshot = fresh(
        MP,
        &After {
            service: true,
            ..After::done(HB)
        },
    );
    match start(MP, &snapshot, Some(&view_of(vec![success(MP)])), true) {
        MigrationStart::Run(run, note) => {
            assert!(run.plan().requires_manual_review());
            assert_eq!(note, None);
        }
        other => panic!("expected the manual-review plan, got {other:?}"),
    }
}

#[test]
fn only_a_run_that_dispatched_a_mutation_is_recorded() {
    assert!(must_record(&success(MP)));
    let guarded = RunRecord::new(
        RunStart {
            id: RECORD_ID.into(),
            started_at: "2026-10-04T12:00:00Z".into(),
            tool_version: "0.1.0".into(),
        },
        success(MP).subject().clone(),
        vec![RecordedStep::new(
            1,
            StepKind::AddDestination,
            None,
            StepResult::NotDispatchedMutating,
            false,
        )],
        RunEnding::Completed,
        None,
    );
    assert!(!must_record(&guarded));
}

#[test]
fn invariant_16_a_record_the_marker_names_or_cannot_confirm_is_not_recognised() {
    // Pass 08 slice 5, H1: a run is never reported done on a record not known to
    // be on disk (04: such a record never exits 0), in either direction.
    for source in [MP, HB] {
        let (_, _, dst, _) = pair(source);
        let snapshot = fresh(source, &After::done(dst));
        let stamp = Some(RECORD_ID[..16].to_owned());
        for (what, marker) in [
            ("the marker names it", MarkerState::Names(RECORD_ID.into())),
            ("the marker is unreadable", MarkerState::Unreadable),
        ] {
            let view = HistoryView::new(stamp.clone(), vec![success(source)], marker);
            let note = planned_with(start(source, &snapshot, Some(&view), true));
            assert!(
                matches!(&note, Some(NotRecognised::RecordNotConfirmed { run_id }) if run_id == RECORD_ID),
                "{source}, {what}: {note:?}"
            );
        }
        // The control: a marker naming some other run says nothing against it.
        let view = HistoryView::new(
            stamp,
            vec![success(source)],
            MarkerState::Names("20261004T130000Z-00000000000000ff".into()),
        );
        assert!(
            matches!(
                start(source, &snapshot, Some(&view), true),
                MigrationStart::RetainedRunDone { .. }
            ),
            "{source}"
        );
    }
}
