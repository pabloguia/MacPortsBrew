// SPDX-License-Identifier: Apache-2.0

//! Building a `MigrationPlan` as data (`04-migration-and-safety.md`).
//! Dry-run output is the deliverable; there is no executor in this pass.

use crate::analysis::{analyze, AnalysisOptions, AnalysisSource};
use crate::provider_commands::{self, ProviderCommands};
use macportsbrew_domain::plan::PLAN_SCHEMA_VERSION;
use macportsbrew_domain::{
    Confidence, DomainError, Elevation, ErrorId, MigrationPlan, MigrationStep, Outcome,
    PackageIdentity, PackageKind, PlanCommand, PlanRisk, ProviderId, ProviderPresence, Reason,
    Snapshot, StepKind, VerificationCheck,
};
use macportsbrew_policy::PolicyData;
use std::collections::BTreeMap;

/// Every reason identifier the planner emits, so the UI can assert its
/// catalogue covers them (the policy engine lists its own in
/// `macportsbrew_policy::reasons`).
pub const REASON_IDS: &[&str] = &[
    "reason.destination_provider_absent",
    "reason.class_requires_review",
    "reason.mapping_confidence_below_threshold",
    "reason.reverse_dependencies_present",
    "reason.destination_deprecated",
    "reason.availability_unknown",
    "reason.snapshot_before_mutation",
    "reason.add_under_destination",
    "reason.verify_destination",
    "reason.set_primary_shim",
    "reason.verify_resolution",
    "reason.remove_from_source",
    "reason.verify_removal",
    "reason.cleanup_review",
    "reason.record_history",
];

/// What the user asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanRequest {
    /// Package name (the source provider's spelling).
    pub package: String,
    /// Destination provider.
    pub to: ProviderId,
    /// Keep the source package (`--retain-source`).
    pub retain_source: bool,
}

/// Builds the plan. Never mutates; never elevates; produces a plan whose
/// first mutating step is preceded by the automatic snapshot, whose
/// destination is added and verified before any removal (INV-01), and which
/// stops at a `MANUAL_REVIEW` step whenever a safety condition holds.
pub fn build_plan(
    snapshot: &Snapshot,
    policy: &PolicyData,
    options: &AnalysisOptions,
    request: &PlanRequest,
) -> Result<MigrationPlan, DomainError> {
    let opts = AnalysisOptions {
        advanced: true,
        only: [request.package.clone()].into_iter().collect(),
        ..options.clone()
    };
    let analysis = analyze(snapshot, policy, &opts, AnalysisSource::Snapshot);
    let unit = analysis
        .packages
        .iter()
        .find(|p| p.name == request.package || p.rows.values().any(|r| r.name == request.package))
        .ok_or_else(|| {
            DomainError::new(
                ErrorId::PackageNotFound,
                format!(
                    "{} is not installed under any detected provider",
                    request.package
                ),
            )
        })?;
    let source = unit.source.clone();
    if source == request.to {
        return Err(DomainError::new(
            ErrorId::Usage,
            format!("{} is already held by {}", unit.name, request.to),
        ));
    }
    let dest_snapshot = snapshot.providers.get(&request.to).ok_or_else(|| {
        DomainError::new(
            ErrorId::ProviderNotFound,
            format!("{} is not a known provider", request.to),
        )
    })?;
    let commands = provider_commands::load()?;
    let mapping = unit
        .mappings
        .iter()
        .find(|m| m.destination == request.to)
        .cloned();
    let destination_package = mapping
        .as_ref()
        .and_then(|m| m.selected.as_ref().map(|c| c.package.name.clone()));
    let source_row = &unit.rows[&source];
    let kind = source_row
        .installed
        .first()
        .map_or(PackageKind::Package, |r| r.kind);

    let mut risks = Vec::new();
    let mut manual: Vec<Reason> = Vec::new();
    let push_block =
        |risks: &mut Vec<PlanRisk>, manual: &mut Vec<Reason>, id: &str, reason: Reason| {
            risks.push(PlanRisk {
                id: id.into(),
                reason: reason.clone(),
                blocking: true,
            });
            manual.push(reason);
        };

    if !matches!(dest_snapshot.detection.presence, ProviderPresence::Present) {
        push_block(
            &mut risks,
            &mut manual,
            "destination_absent",
            Reason::new("reason.destination_provider_absent").with("provider", &request.to),
        );
    }
    if unit.class.forces_manual_review() {
        push_block(
            &mut risks,
            &mut manual,
            "class",
            Reason::new("reason.class_requires_review")
                .with("package", &unit.name)
                .with("class", &format!("{:?}", unit.class.dominant())),
        );
    }
    let confidence = mapping.as_ref().map_or(Confidence::None, |m| m.confidence);
    if !confidence.is_strong() {
        push_block(
            &mut risks,
            &mut manual,
            "mapping",
            Reason::new("reason.mapping_confidence_below_threshold")
                .with("destination", &request.to)
                .with("confidence", &format!("{confidence:?}"))
                .with("threshold", &"Strong")
                .with(
                    "candidates",
                    &mapping.as_ref().map_or(0, |m| m.candidates.len()),
                ),
        );
    }
    if !request.retain_source && source_row.reverse_dependency_count > 0 {
        push_block(
            &mut risks,
            &mut manual,
            "reverse_dependencies",
            Reason::new("reason.reverse_dependencies_present")
                .with("count", &source_row.reverse_dependency_count)
                .with("provider", &source),
        );
    }
    if let Some(dest_row) = unit.rows.get(&request.to) {
        if dest_row.deprecated || dest_row.disabled {
            push_block(
                &mut risks,
                &mut manual,
                "destination_deprecated",
                Reason::new("reason.destination_deprecated")
                    .with("destination", &request.to)
                    .with(
                        "state",
                        &if dest_row.disabled {
                            "disabled"
                        } else {
                            "deprecated"
                        },
                    ),
            );
        }
        if !dest_row.availability.is_prebuilt() {
            risks.push(PlanRisk {
                id: "destination_source_build".into(),
                reason: Reason::new("reason.availability_unknown")
                    .with("provider", &request.to)
                    .with("package", &unit.name),
                blocking: false,
            });
        }
    }

    let overrode = (unit.recommendation.outcome
        != Outcome::MigrateTo {
            provider: request.to.clone(),
        })
    .then(|| unit.recommendation.outcome.clone());

    let mut steps = Vec::new();
    let mut index = 0u32;
    let mut next = |kind, provider: Option<ProviderId>, commands, verification, reason| {
        index += 1;
        MigrationStep {
            index,
            kind,
            provider,
            commands,
            verification,
            reason,
        }
    };

    if manual.is_empty() {
        steps.push(next(
            StepKind::SnapshotBefore,
            None,
            vec![],
            vec![],
            Reason::new("reason.snapshot_before_mutation"),
        ));
        let dest_name = destination_package.clone().unwrap_or_default();
        steps.push(next(
            StepKind::AddDestination,
            Some(request.to.clone()),
            vec![command(
                &commands,
                dest_snapshot,
                &request.to,
                "add",
                &dest_name,
                kind,
            )],
            vec![],
            Reason::new("reason.add_under_destination")
                .with("package", &dest_name)
                .with("provider", &request.to),
        ));
        steps.push(next(
            StepKind::VerifyDestination,
            Some(request.to.clone()),
            vec![command(
                &commands,
                dest_snapshot,
                &request.to,
                "verify",
                &dest_name,
                kind,
            )],
            vec![
                VerificationCheck::ProviderReportsActive,
                VerificationCheck::ProviderIntegrityCheck,
                VerificationCheck::ExecutablesOwnedByDestination {
                    executables: vec![unit.name.clone()],
                },
                VerificationCheck::LivenessProbe {
                    executable: unit.name.clone(),
                    flag: "--version".into(),
                },
                VerificationCheck::NoCrossProviderLinkage,
            ],
            Reason::new("reason.verify_destination"),
        ));
        // The primary is set through the MacPortsBrew-managed directory in
        // the user's home (SE-036), by a command of this tool's own, so the
        // step is enumerated with its command and its elevation: none.
        steps.push(next(
            StepKind::SetPrimary,
            Some(request.to.clone()),
            vec![PlanCommand {
                program: "macportsbrew".into(),
                argv: vec!["primary".into(), unit.name.clone(), request.to.to_string()],
                environment: BTreeMap::new(),
                elevation: Elevation::None,
            }],
            vec![VerificationCheck::ProviderReportsActive],
            Reason::new("reason.set_primary_shim").with("provider", &request.to),
        ));
        steps.push(next(
            StepKind::VerifyResolution,
            None,
            vec![],
            vec![],
            Reason::new("reason.verify_resolution").with("executable", &unit.name),
        ));
        if !request.retain_source {
            let src_snapshot = &snapshot.providers[&source];
            steps.push(next(
                StepKind::RemoveSource,
                Some(source.clone()),
                vec![command(
                    &commands,
                    src_snapshot,
                    &source,
                    "remove",
                    &unit.name,
                    kind,
                )],
                vec![],
                Reason::new("reason.remove_from_source")
                    .with("package", &unit.name)
                    .with("provider", &source),
            ));
            steps.push(next(
                StepKind::VerifyRemoval,
                Some(source.clone()),
                vec![],
                vec![VerificationCheck::ProviderReportsActive],
                Reason::new("reason.verify_removal"),
            ));
            steps.push(next(
                StepKind::CleanupReview,
                Some(source.clone()),
                vec![],
                vec![],
                Reason::new("reason.cleanup_review"),
            ));
        }
        steps.push(next(
            StepKind::RecordHistory,
            None,
            vec![],
            vec![],
            Reason::new("reason.record_history"),
        ));
    } else {
        for reason in manual {
            steps.push(next(StepKind::ManualReview, None, vec![], vec![], reason));
        }
    }

    let elevated_steps = steps
        .iter()
        .filter(|s| {
            s.commands
                .iter()
                .any(|c| !matches!(c.elevation, Elevation::None))
        })
        .map(|s| s.index)
        .collect();
    let plan = MigrationPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        package: PackageIdentity::new(source.clone(), unit.name.clone()),
        source,
        destination: request.to.clone(),
        destination_package,
        retain_source: request.retain_source,
        primary_after: request.to.clone(),
        recommendation: Outcome::MigrateTo {
            provider: request.to.clone(),
        },
        overrode,
        steps,
        risks,
        elevated_steps,
        policy_version: policy.version().to_string(),
        dry_run_only: true,
    };
    plan.check_ordering()
        .map_err(|e| DomainError::new(ErrorId::Internal, e))?;
    Ok(plan)
}

fn command(
    table: &BTreeMap<String, ProviderCommands>,
    provider: &macportsbrew_domain::ProviderSnapshot,
    id: &ProviderId,
    operation: &str,
    package: &str,
    kind: PackageKind,
) -> PlanCommand {
    let entry = table.get(id.as_str());
    let program = provider.detection.prefix.as_ref().zip(entry).map_or_else(
        || format!("<{id} prefix>/bin/{id}"),
        |(prefix, e)| prefix.join(&e.executable).to_string_lossy().into_owned(),
    );
    let mut argv: Vec<String> = entry
        .and_then(|e| e.verbs.get(operation).cloned())
        .unwrap_or_else(|| vec![format!("<{operation}>")]);
    argv.push(package.to_string());
    let elevation_key = match kind {
        PackageKind::GuiApp => format!("{operation}_gui_app"),
        PackageKind::Package => operation.to_string(),
    };
    let elevation = match entry.and_then(|e| e.elevation.get(&elevation_key).map(String::as_str)) {
        Some("none") => Elevation::None,
        Some("required") => Elevation::Required {
            reason: entry
                .and_then(|e| e.elevation_reason.clone())
                .unwrap_or_else(|| "provider prefix is not user-writable".into()),
        },
        _ => Elevation::Unknown,
    };
    PlanCommand {
        program,
        argv,
        environment: provider.invocation_environment.clone(),
        elevation,
    }
}
