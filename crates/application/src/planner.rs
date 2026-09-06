// SPDX-License-Identifier: Apache-2.0

//! Building a `MigrationPlan` as data (`04-migration-and-safety.md`).
//! Dry-run output is the deliverable; there is no executor in this pass.

use crate::analysis::{analyze, AnalysisOptions, AnalysisSource};
use crate::provider_commands::{self, ProviderCommands};
use macportsbrew_domain::plan::PLAN_SCHEMA_VERSION;
use macportsbrew_domain::{
    CommandTarget, Confidence, DomainError, Elevation, ErrorId, MigrationPlan, MigrationStep,
    Outcome, PackageIdentity, PackageKind, PlanCommand, PlanRisk, ProviderId, ProviderPresence,
    Reason, Snapshot, StepKind, VerificationCheck,
};
use macportsbrew_policy::PolicyData;
use std::collections::BTreeMap;
use std::path::Path;

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
    "reason.already_under_destination",
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
    // Analyse the whole machine and pick the unit afterwards, rather than
    // filtering the analysis down to the requested name.
    //
    // `only` does not merely hide rows: which unit a row ends up in depends on
    // what else was built, because the merge marks counterpart identities as
    // seen. Scoping the analysis to one name therefore *changed the unit*, and
    // the planner then reasoned about a unit the user's view does not contain.
    // Measured on the shipped fixture: `plan python@3.13 --to macports` and
    // `plan python312 --to macports` - the two spellings of one package -
    // produced plans for different packages, the second naming `python@3.12`.
    // Two others produced a plan by one spelling and nothing by the other.
    //
    // Advanced, because a plan may legitimately be asked for a package that is
    // a dependency somewhere: naming it is the explicit entry INV-13 requires.
    // With SE-048 that no longer changes a unit's identity either, so the
    // planner and the view the user was shown now genuinely agree.
    let opts = AnalysisOptions {
        advanced: true,
        only: std::collections::BTreeSet::default(),
        ..options.clone()
    };
    let analysis = analyze(snapshot, policy, &opts, AnalysisSource::Snapshot);
    // An exact unit name wins outright; only if nothing matches by name do we
    // look at what each provider calls its rows.
    let matches: Vec<&_> = analysis
        .packages
        .iter()
        .filter(|p| p.name == request.package)
        .collect();
    let matches = if matches.is_empty() {
        analysis
            .packages
            .iter()
            .filter(|p| p.rows.values().any(|r| r.name == request.package))
            .collect()
    } else {
        matches
    };
    let unit = match matches.as_slice() {
        [] => {
            return Err(DomainError::new(
                ErrorId::PackageNotFound,
                format!(
                    "{} is not installed under any detected provider",
                    request.package
                ),
            ))
        }
        [only] => *only,
        // One provider's name can be the counterpart of several of another's:
        // `python312` is what MacPorts calls the match for `python@3.12`,
        // `python@3.13` and `python@3.14`. Taking the first was silent and
        // wrong - asking to plan `python312` produced a plan for `python@3.12`,
        // a different package. Ambiguity never acts automatically here; it is
        // reported with the choices, like every other ambiguous match.
        many => {
            let mut names: Vec<&str> = many.iter().map(|u| u.name.as_str()).collect();
            names.sort_unstable();
            return Err(DomainError::new(
                ErrorId::Usage,
                format!(
                    "{} is how {} names the counterpart of more than one package: {}. \
                     Name the one you mean.",
                    request.package,
                    many[0]
                        .rows
                        .iter()
                        .find(|(_, r)| r.name == request.package)
                        .map_or("that provider", |(id, _)| id.as_str()),
                    names.join(", ")
                ),
            ));
        }
    };
    // `--to X` names the **destination**; the source is the other side that
    // actually holds the package. It used to name neither: the guard compared
    // the destination against the unit's inferred source, so for a package both
    // providers hold - where that inference falls back to id order - one
    // direction produced a plan and the mirrored one an error (INV-17, SE-047).
    let holders: Vec<&ProviderId> = unit
        .rows
        .iter()
        .filter(|(_, r)| !r.installed.is_empty())
        .map(|(id, _)| id)
        .collect();
    let Some(source) = holders
        .iter()
        .find(|id| **id != &request.to)
        .map(|id| (*id).clone())
    else {
        // Nothing holds it but the destination, so there is nothing to move.
        return Err(DomainError::new(
            ErrorId::Usage,
            format!("{} is already held by {}", unit.name, request.to),
        ));
    };
    // Whether the destination already has it decides the shape of the plan
    // rather than whether there is one: consolidating onto a provider that
    // already holds the package is a removal from the other side, not an
    // install that would be a no-op.
    let destination_holds_it = holders.iter().any(|id| **id == request.to);
    let dest_snapshot = snapshot.providers.get(&request.to).ok_or_else(|| {
        DomainError::new(
            ErrorId::ProviderNotFound,
            format!("{} is not a known provider", request.to),
        )
    })?;
    let commands = provider_commands::load()?;
    // A unit's mappings point away from its own source, so when the destination
    // *is* that source - consolidating onto the side the unit is attributed to -
    // there is no mapping toward it to find, and asking for one yielded no
    // destination package and a `NONE` confidence that blocked the plan at
    // manual review. The destination's own row already says what the package is
    // called there, and an installed counterpart is a fact rather than a guess.
    let mapping = unit
        .mappings
        .iter()
        .find(|m| m.destination == request.to)
        .cloned();
    let destination_row = unit.rows.get(&request.to);
    let destination_package = if request.to == unit.source {
        destination_row.map(|r| r.name.clone())
    } else {
        mapping
            .as_ref()
            .and_then(|m| m.selected.as_ref().map(|c| c.package.name.clone()))
    };
    let source_row = &unit.rows[&source];
    // What the package is called **on the side being removed from**. It is not
    // always `unit.name`: a unit is keyed by the spelling under the provider it
    // is attributed to, and now that `--to` may name that very provider, the
    // source is the other side, whose row often carries a different name -
    // `node` for `nodejs24`, `python312` for `python@3.13`. Addressing the
    // removal by the unit key emitted a command against a package that side does
    // not have, or - worse, and reachable - against a *different* package that it
    // does, one never analysed, mapped or counted for dependents (INV-02: nothing
    // outside the plan happens; here the plan itself named the wrong thing).
    let source_name = source_row.name.clone();
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
    // Only the unit's own side is exempt from the mapping threshold, and only
    // because it is not a mapping: when the destination *is* the provider the
    // unit is attributed to, the "counterpart" is the package itself, so there
    // is no equivalence to be unsure about and no mapping toward it to score.
    //
    // Deliberately narrower than "the destination holds something". A weak or
    // ambiguous mapping whose candidate happens to be installed is still a
    // guess about whether two differently-named packages are the same software,
    // and treating it as certain because a name matched something on disk would
    // walk straight past `MANUAL_REVIEW_MAPPING_CONFIDENCE` — the guard that
    // exists for exactly that guess.
    // The threshold always scores the guess that identifies the package being
    // *removed*, because that is the destructive half.
    //
    // Normally that is the mapping toward the destination. When the destination
    // is the unit's own side there is no such mapping - and no equivalence to be
    // unsure about, since the destination is the package itself - but the
    // *source* row got there by a mapping, and that is now the guess that says
    // what will be uninstalled. Waiving the threshold outright in that branch
    // would have exempted exactly the wrong side: `python@3.13`'s counterpart is
    // selected at `PROBABLE`, and Python 3.13 and 3.12 are not the same software.
    let confidence = if request.to == unit.source {
        unit.mappings
            .iter()
            .find(|m| m.destination == source)
            .map_or(Confidence::None, |m| m.confidence)
    } else {
        mapping.as_ref().map_or(Confidence::None, |m| m.confidence)
    };
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
        // The step stays in the plan when the destination already holds the
        // package, and carries no command. Consolidating onto a provider that
        // already has it is a removal from the other side, and the plan used to
        // open by proposing an install that would do nothing - and to warn that
        // it could not tell whether the destination would supply a prebuilt
        // artifact for something already built and installed there.
        //
        // Saying "already present, nothing to add" is better than omitting the
        // step: the plan is a document a human reads before agreeing to it, so
        // the absence of work is itself worth stating, and every plan keeps the
        // same shape for `check_ordering` and for whatever executes it.
        let (add_commands, add_reason) = if destination_holds_it {
            (
                vec![],
                Reason::new("reason.already_under_destination")
                    .with("package", &dest_name)
                    .with("provider", &request.to),
            )
        } else {
            (
                vec![command(
                    &commands,
                    dest_snapshot,
                    &request.to,
                    "add",
                    &dest_name,
                    kind,
                )?],
                Reason::new("reason.add_under_destination")
                    .with("package", &dest_name)
                    .with("provider", &request.to),
            )
        };
        steps.push(next(
            StepKind::AddDestination,
            Some(request.to.clone()),
            add_commands,
            vec![],
            add_reason,
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
            )?],
            {
                // Every one of the contract's checks is satisfiable by state
                // that predates the plan. For a package the destination already
                // holds, all five are true before a single step runs - so a
                // failed or no-op install cleared the gate INV-01 depends on,
                // and nothing in the plan said which of the two had happened
                // (F-06).
                //
                // The planner knows which case this is, and now says so. Where
                // the destination was absent, `PackageNewlyPresent` is a check
                // an install that did nothing cannot satisfy.
                //
                // Where it was already there, nothing is dropped: the full
                // contract of `04-migration-and-safety.md` still applies, and
                // the step simply makes no claim about *this run* having caused
                // the package to be present, because it did not. The contract is
                // a list of necessary conditions, so the absent case is a
                // strengthening of it rather than a different contract - and the
                // add step above already reads "already installed; nothing to
                // add". A weaker guard stated beats a stronger one implied.
                let mut checks = vec![VerificationCheck::ProviderReportsActive];
                if !destination_holds_it {
                    checks.push(VerificationCheck::PackageNewlyPresent);
                }
                checks.extend([
                    VerificationCheck::ProviderIntegrityCheck,
                    VerificationCheck::ExecutablesOwnedByDestination {
                        executables: vec![dest_name.clone()],
                    },
                    VerificationCheck::LivenessProbe {
                        executable: dest_name.clone(),
                        flag: "--version".into(),
                    },
                    VerificationCheck::NoCrossProviderLinkage,
                ]);
                checks
            },
            Reason::new("reason.verify_destination"),
        ));
        // The primary is set through the MacPortsBrew-managed directory in
        // the user's home (SE-036), by a command of this tool's own, so the
        // step is enumerated with its command and its elevation: none.
        steps.push(next(
            StepKind::SetPrimary,
            Some(request.to.clone()),
            // Marked as a self-invocation rather than left to be recognised by
            // its program string. It is the one command in a plan that is not a
            // provider invocation, so INV-10's sanitised environment is not what
            // governs it - and the test named after INV-10 used to skip it by
            // comparing `program` to a literal, which meant nobody noticed it
            // carried no environment and a relative name at all. The type says
            // which rule applies; an executor resolves its own binary instead of
            // searching `PATH` for this one, which is the hazard `doctor` exists
            // to warn about.
            vec![PlanCommand {
                // This tool's own command; it is always the real one.
                placeholder: false,
                target: CommandTarget::Tool,
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
                    &source_name,
                    kind,
                )?],
                vec![],
                Reason::new("reason.remove_from_source")
                    .with("package", &source_name)
                    .with("provider", &source),
            ));
            steps.push(next(
                StepKind::VerifyRemoval,
                Some(source.clone()),
                vec![],
                // Not `ProviderReportsActive`, which is documented as "reports
                // the package installed and active" - as data, that told an
                // executor to verify a removal by confirming the package is
                // still there (F-06).
                vec![VerificationCheck::PackageAbsent],
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
        package: PackageIdentity::new(source.clone(), source_name.clone()),
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
) -> Result<PlanCommand, DomainError> {
    let entry = table.get(id.as_str());
    // A provider that was not detected has no prefix, so the program and the
    // verb are both written as illustrations. They read like a command, and
    // until `placeholder` existed nothing said otherwise (F-20).
    let resolved = provider.detection.prefix.as_ref().zip(entry);
    let placeholder = resolved.is_none();
    let program = resolved.map_or_else(
        || format!("<{id} prefix>/bin/{id}"),
        |(prefix, e)| prefix.join(&e.executable).to_string_lossy().into_owned(),
    );
    // The one element of this argv that is data rather than a verb from the
    // table. A verb may legitimately begin with a dash - `-q` is how MacPorts is
    // quieted - so the finished argv cannot be checked as a whole; the guard
    // belongs here, where the package is still known to be a package.
    //
    // `--from-snapshot` validates the same names at the boundary now, so this is
    // the second of two. It is worth having: it is the one that holds for a
    // name that reached the planner by some other route, and it fails closed
    // where an empty `dest_name` would otherwise have become an empty argument
    // (F-09).
    if entry.is_some_and(|e| {
        e.reserved_names
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(package))
    }) {
        return Err(DomainError::new(
            ErrorId::Usage,
            format!(
                "{package:?} is a word {id} reads as a set of ports rather than as \
                 one package, so a step naming it would reach far more than the plan says"
            ),
        ));
    }
    if package.is_empty() {
        // Not a bad snapshot: an empty name here means the planner reached the
        // mutating path without a destination, which the mapping-confidence
        // block is supposed to prevent. Telling the user to recapture a file
        // that was fine would send them after the wrong thing.
        return Err(DomainError::new(
            ErrorId::Internal,
            format!("the plan reached a {operation} step for {id} with no package named"),
        ));
    }
    if !macportsbrew_domain::is_safe_provider_name(package) {
        return Err(DomainError::new(
            ErrorId::SnapshotUnusable,
            format!(
                "{package:?} is not a package name this tool will pass to a provider; \
                 it would be read as a flag or a path rather than as a package"
            ),
        ));
    }
    // And the program itself, last. The prefix is validated at the boundary, but
    // a relative program is never correct whatever produced it: `Command::new`
    // resolves a string containing a slash against the working directory, so
    // this is the difference between running the provider and running whatever
    // is under the directory the user happened to be in.
    if !program.starts_with('<') && !Path::new(&program).is_absolute() {
        return Err(DomainError::new(
            ErrorId::SnapshotUnusable,
            format!(
                "the plan would run {program:?}, which is not an absolute program: \
                 the prefix it came from names no directory this tool can resolve"
            ),
        ));
    }
    // F-22: the verb comes from `verbs[operation]` and the *elevation* from the
    // package kind, so a cask would be verified with a formula-only command
    // (`list --versions <cask>` exits 1 on a correctly installed cask). That is
    // latent, not live: `PackageClass::GuiApp` forces `MANUAL_REVIEW`, a plan
    // that halts there emits no command, and this function is never reached for
    // a cask. `tests/planner_casks.rs` pins that and fails if it changes.
    //
    // Making casks plannable therefore requires a kind-aware *verb* here, which
    // `[entry.verbs]` cannot express today. Do not read the `GuiApp` arm below
    // as evidence that the rest of this function handles casks — only the
    // elevation lookup is kind-aware, and that asymmetry is the defect.
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
    Ok(PlanCommand {
        placeholder,
        target: CommandTarget::Provider,
        program,
        argv,
        environment: provider.invocation_environment.clone(),
        elevation,
    })
}
