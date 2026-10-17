// SPDX-License-Identifier: Apache-2.0

//! Building a `MigrationPlan` as data (`04-migration-and-safety.md`).
//! Dry-run output is the deliverable; whether a plan may be executed is
//! `execution_gate`'s question, never this module's.

use crate::analysis::{analyze, AnalysisOptions, AnalysisSource};
use crate::provider_commands::{self, ProviderCommands};
use crate::report::ProviderRow;
use macportsbrew_domain::plan::{PLAN_SCHEMA_VERSION, THIS_BUILD_DISPATCHES_MUTATIONS};
use macportsbrew_domain::{
    Capability, CapabilityState, CommandTarget, Confidence, DomainError, Elevation, ErrorId,
    MigrationPlan, MigrationStep, Outcome, PackageIdentity, PackageKind, PlanCommand, PlanRisk,
    ProviderId, ProviderPresence, ProviderSnapshot, Reason, Snapshot, StepKind, SuccessSemantic,
    VerificationCheck,
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
    "reason.provider_configuration_outranks",
    "reason.provider_configuration_unreadable",
    "reason.provider_configuration_unknown",
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

/// The id of the risk a provider's own configuration raises (DP-027). Its
/// reason's `provider` parameter names the provider; the execution gate reads
/// both to refuse a step that would change it.
pub const PROVIDER_CONFIGURATION_RISK: &str = "provider_configuration";

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

/// What planning a request produced, when it produced anything.
///
/// A package that only its requested destination holds is not a failure of
/// the request: it is the state a successful migration leaves behind, and
/// INV-16 needs a re-run to recognise it and report "nothing further" rather
/// than read it as an error. `plan` still reports it as `ErrorId::Usage`
/// through [`build_plan`]; this type is what lets a caller tell the two apart
/// without matching on a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanOutcome {
    /// There is something to move, and this is the plan for it. Boxed because
    /// a plan is large and the other variant is two names (clippy's
    /// `large_enum_variant`); it is built once per invocation.
    Plan(Box<MigrationPlan>),
    /// The destination holds the package and no other provider does.
    AlreadyAtDestination {
        /// The unit's name, as `plan` reports it.
        package: String,
        /// The requested destination, which is the only holder.
        provider: ProviderId,
    },
}

/// Builds the plan. Never mutates; never elevates; produces a plan whose
/// first mutating step is preceded by the automatic snapshot, whose
/// destination is added and verified before any removal (INV-01), and which
/// stops at a `MANUAL_REVIEW` step whenever a safety condition holds.
///
/// [`plan_outcome`] is the same function with a package already at its
/// destination kept as an outcome; here that outcome is reported as
/// `ErrorId::Usage`, the refusal `plan` has always given for it. A request
/// `plan_outcome` refuses instead — another provider broken, unrecorded or
/// missing from the input — is refused here with the same error.
///
/// `known` is every provider id this build knows. The planner does not name
/// providers, so the caller supplies them; a known provider the snapshot
/// omits is one whose inventory this input does not carry.
pub fn build_plan(
    snapshot: &Snapshot,
    policy: &PolicyData,
    options: &AnalysisOptions,
    request: &PlanRequest,
    known: &[ProviderId],
) -> Result<MigrationPlan, DomainError> {
    match plan_outcome(snapshot, policy, options, request, known)? {
        PlanOutcome::Plan(plan) => Ok(*plan),
        PlanOutcome::AlreadyAtDestination { package, provider } => {
            Err(already_held(&package, &provider))
        }
    }
}

/// [`build_plan`], with the plan paired with the snapshot it was built from
/// ([`crate::executor::PlannedRun`]). The only constructor of that pairing, so
/// the run loop judges a `PACKAGE_NEWLY_PRESENT` against the snapshot the plan
/// came from and never one handed to it separately. The `migrate` slice owes
/// the same pairing for `plan_outcome` if it reports "nothing to move".
pub fn plan_run(
    snapshot: &Snapshot,
    policy: &PolicyData,
    options: &AnalysisOptions,
    request: &PlanRequest,
    known: &[ProviderId],
) -> Result<crate::executor::PlannedRun, DomainError> {
    build_plan(snapshot, policy, options, request, known)
        .map(|plan| crate::executor::PlannedRun::new(plan, snapshot.clone()))
}

/// What `migrate` starts from: a run to render (and, with `--yes`, to put to
/// the execution gate), or the INV-16 answer that there is nothing to move.
#[derive(Clone, Debug)]
pub enum MigrationStart {
    /// There is something to move: the plan, paired with the snapshot it was
    /// built from. Boxed for the reason [`PlanOutcome::Plan`] is. The second
    /// field is why a consulted history did not recognise a retain-source
    /// re-run as done, when it had a record to say something about.
    Run(
        Box<crate::executor::PlannedRun>,
        Option<crate::history::NotRecognised>,
    ),
    /// A `--retain-source` re-run that history and a fresh capture both show
    /// done (INV-16): nothing is planned, nothing is written
    /// ([`crate::history::recognise_retained_run`]).
    RetainedRunDone {
        /// The unit's name.
        package: String,
        /// The destination, holding it active and resolving it.
        provider: ProviderId,
        /// The run that did it.
        run_id: String,
    },
    /// The destination holds the package, no other provider does, and
    /// nothing that could hide a copy elsewhere stands in the way
    /// ([`migration_start`]).
    NothingToMove {
        /// The unit's name.
        package: String,
        /// The destination, the only holder.
        provider: ProviderId,
        /// The other providers that were not detected, in identifier order.
        /// Empty when every other provider was detected and read; otherwise
        /// the answer is only as strong as detection, which looks where a
        /// provider is documented to live (DP-026).
        not_detected: Vec<ProviderId>,
    },
}

/// [`plan_outcome`] for `migrate`: a plan comes back paired with its snapshot
/// (as [`plan_run`] pairs it), and "already at the destination" is accepted
/// as *nothing to move* only when it can be shown — which is stricter than
/// `plan_outcome`'s answer, because `migrate` is the command that reports it
/// as done (INV-16) and exits 0.
///
/// **Another provider was not detected.** `plan_outcome` takes
/// `ProviderPresence::Absent` as proof that provider holds nothing, which is
/// only as reliable as prefix detection: a provider installed where no
/// documented candidate points reads as absent and could still hold the
/// package. *Nothing to move* is still the answer — it changes nothing, which
/// is what INV-16 asks, and a re-run after a completed migration whose source
/// provider was then uninstalled must say so rather than "blocked" — but it
/// names every provider that was not detected in `not_detected`, so the
/// claim's strength travels with it (DP-026, ruled (b) on 2026-10-03). Any
/// other state of another provider — broken, missing from the input, or with
/// an inventory nobody read — never reaches here: `plan_outcome` has refused
/// it first (`PROVIDER_UNHEALTHY`, `SAFETY_UNVERIFIABLE`).
/// [`ErrorId::ProviderPresenceUnproven`] is therefore no longer raised; the
/// identifier is kept because exit codes are a documented contract.
///
/// One more refusal, exit 4, naming what stands in the way:
///
/// - **Another unit may be the same package under another name**
///   ([`ErrorId::SafetyBlocked`]: a finding the user can read and act on). The
///   analysis merges a package's rows across providers only through a
///   selected mapping; a row another provider holds whose mapping toward the
///   destination merely *names* the destination package as a candidate
///   (ambiguous, or below the confidence a selection needs) sits in a unit of
///   its own and is invisible to `plan_outcome`. Any such row refuses.
///
/// **What the answer establishes.** No other provider in the input holds the
/// package, nor any package whose mapping toward the destination names it as
/// a candidate. A copy under a name no mapping connects is not seen; and from
/// a snapshot file the answer is about that input, not the machine.
///
/// **A `--retain-source` re-run.** After one, the source still holds the
/// package, so `plan_outcome` builds a plan again. With a consulted
/// `history`, that plan is replaced by [`MigrationStart::RetainedRunDone`]
/// when [`crate::history::recognise_retained_run`] recognises it — and only
/// a plan with no `MANUAL_REVIEW` step is ever replaced: every refusal above
/// and every manual review keeps its precedence, so a safety stop is never
/// hidden behind "already done" (pass 08 plan, M3).
pub fn migration_start(
    snapshot: &Snapshot,
    policy: &PolicyData,
    options: &AnalysisOptions,
    request: &PlanRequest,
    known: &[ProviderId],
    history: crate::history::HistoryConsultation<'_>,
) -> Result<MigrationStart, DomainError> {
    match plan_outcome(snapshot, policy, options, request, known)? {
        PlanOutcome::Plan(plan) => {
            let mut note = None;
            if let crate::history::HistoryConsultation::Consulted { view, run_stamp } = history {
                if plan.retain_source && !plan.requires_manual_review() {
                    match crate::history::recognise_retained_run(view, run_stamp, &plan, snapshot) {
                        crate::history::Recognition::Recognised { run_id } => {
                            return Ok(MigrationStart::RetainedRunDone {
                                package: plan.package.name.clone(),
                                provider: plan.destination.clone(),
                                run_id,
                            });
                        }
                        crate::history::Recognition::NotRecognised(why) => note = Some(why),
                        crate::history::Recognition::NoRecord => {}
                    }
                }
            }
            Ok(MigrationStart::Run(
                Box::new(crate::executor::PlannedRun::new(*plan, snapshot.clone())),
                note,
            ))
        }
        PlanOutcome::AlreadyAtDestination { package, provider } => {
            let not_detected =
                nothing_to_move_is_shown(snapshot, policy, options, known, &package, &provider)?;
            Ok(MigrationStart::NothingToMove {
                package,
                provider,
                not_detected,
            })
        }
    }
}

/// The refusals [`migration_start`] adds to `plan_outcome`'s "already at the
/// destination", and the providers the answer could not see.
fn nothing_to_move_is_shown(
    snapshot: &Snapshot,
    policy: &PolicyData,
    options: &AnalysisOptions,
    known: &[ProviderId],
    package: &str,
    to: &ProviderId,
) -> Result<Vec<ProviderId>, DomainError> {
    let not_shown = |id: ErrorId, detail: String| {
        DomainError::new(
            id,
            format!("{detail}; {package} is not reported as already at {to} with nothing to move"),
        )
    };
    let others: std::collections::BTreeSet<&ProviderId> = known
        .iter()
        .chain(snapshot.providers.keys())
        .filter(|id| *id != to)
        .collect();
    let mut not_detected = Vec::new();
    for id in others {
        // `plan_outcome` has already refused a missing entry, a broken
        // provider and an unreadable inventory; what is left is an absence,
        // which detection could have missed and which the answer names
        // rather than refuses (DP-026). The last arm is unreachable for that
        // reason, and fails closed as the defect it would be.
        match snapshot.providers.get(id).map(|p| &p.detection.presence) {
            Some(ProviderPresence::Present) => {}
            Some(ProviderPresence::Absent) => not_detected.push(id.clone()),
            _ => {
                return Err(not_shown(
                    ErrorId::Internal,
                    format!("{id} reached the nothing-to-move check unread"),
                ));
            }
        }
    }
    let opts = AnalysisOptions {
        advanced: true,
        only: std::collections::BTreeSet::default(),
        ..options.clone()
    };
    // Only `packages` is read; `Snapshot` decides nothing that is shown (the
    // command's own flag comes from its context's source).
    let analysis = analyze(snapshot, policy, &opts, AnalysisSource::Snapshot);
    let Some(name) = analysis
        .packages
        .iter()
        .find(|u| u.name == package)
        .and_then(|u| u.rows.get(to))
        .map(|r| r.name.clone())
    else {
        return Err(DomainError::new(
            ErrorId::Internal,
            format!("the analysis has no {to} row for {package}, which it reported as there"),
        ));
    };
    for unit in &analysis.packages {
        if unit.name == package {
            continue;
        }
        // A row at the destination reaches another unit only through a
        // mapping whose candidates name it, so the first clause is defence in
        // depth behind the second (a mutation dropping it survives); and the
        // holder search only chooses which row the refusal names.
        let names_it = unit.rows.get(to).is_some_and(|r| r.name == name)
            || unit.mappings.iter().any(|m| {
                m.destination == *to && m.candidates.iter().any(|c| c.package.name == name)
            });
        if !names_it {
            continue;
        }
        if let Some((holder, row)) = unit
            .rows
            .iter()
            .find(|(id, r)| *id != to && !r.installed.is_empty())
        {
            return Err(not_shown(
                ErrorId::SafetyBlocked,
                format!(
                    "{holder} holds {}, which may be {name} under another name",
                    row.name
                ),
            ));
        }
    }
    Ok(not_detected)
}

/// The refusal `plan` gives for a package nothing needs to move.
fn already_held(package: &str, provider: &ProviderId) -> DomainError {
    DomainError::new(
        ErrorId::Usage,
        format!("{package} is already held by {provider}"),
    )
}

/// Decides, for a unit no provider other than the destination holds, whether
/// that is "already at the destination" or an error.
///
/// "No other provider holds it" is only as good as each other provider's
/// inventory. A broken provider's inventory is empty because it could not be
/// read, not because it holds nothing (`capture::broken`), so a package it
/// still holds would read as already moved. Every other provider must
/// therefore be either genuinely absent or present with an inventory that
/// could be read; otherwise this refuses and names it. A provider in `known`
/// that the input omits altogether is the same unread inventory: a document
/// with a provider's entry deleted must not read as that provider holding
/// nothing.
///
/// The destination must also hold an **active** copy. An inactive copy is on
/// disk but is not what runs, and a migration is not done while it is the
/// only one; when every other provider's inventory was read, that case keeps
/// the refusal `plan` has always given (the checks above run first).
///
/// A unit with no installed row at all is not a package already moved.
/// `analyze` builds every unit from an installed row, so it cannot be reached
/// from a snapshot, and it is reported as the internal error it would be.
fn nothing_to_move(
    providers: &BTreeMap<ProviderId, ProviderSnapshot>,
    known: &[ProviderId],
    package: &str,
    rows: &BTreeMap<ProviderId, ProviderRow>,
    to: &ProviderId,
) -> Result<PlanOutcome, DomainError> {
    let Some(destination) = rows.get(to).filter(|r| !r.installed.is_empty()) else {
        return Err(DomainError::new(
            ErrorId::Internal,
            format!("the analysis produced a unit for {package} that no provider holds"),
        ));
    };
    if let Some(missing) = known
        .iter()
        .find(|id| *id != to && !providers.contains_key(*id))
    {
        return Err(DomainError::new(
            ErrorId::SafetyUnverifiable,
            format!(
                "this input carries no entry for {missing}, so whether it still holds {package} \
                 cannot be read; {package} is not reported as already at {to}"
            ),
        ));
    }
    for (id, provider) in providers {
        if id == to {
            continue;
        }
        match &provider.detection.presence {
            ProviderPresence::Absent => {}
            ProviderPresence::Broken { .. } => {
                return Err(DomainError::new(
                    ErrorId::ProviderUnhealthy,
                    format!(
                        "{id} is broken, so whether it still holds {package} cannot \
                         be read; {package} is not reported as already at {to}"
                    ),
                ));
            }
            ProviderPresence::Present => {
                // Only a recorded `Available` is a yes: `Unknown`, `Unavailable`
                // and an entry that is missing altogether all leave the
                // inventory unread, and an absent entry is never a yes.
                if provider
                    .capabilities
                    .capabilities
                    .get(&Capability::MachineReadableInventory)
                    != Some(&CapabilityState::Available)
                {
                    return Err(DomainError::new(
                        ErrorId::SafetyUnverifiable,
                        format!(
                            "this input does not record {id}'s inventory, so whether it still \
                             holds {package} cannot be read; {package} is not reported as \
                             already at {to}"
                        ),
                    ));
                }
            }
        }
    }
    if !destination.installed.iter().any(|p| p.active) {
        return Err(already_held(package, to));
    }
    Ok(PlanOutcome::AlreadyAtDestination {
        package: package.to_string(),
        provider: to.clone(),
    })
}

/// [`build_plan`], with a package already at its destination returned as
/// [`PlanOutcome::AlreadyAtDestination`] rather than as an error. Every other
/// refusal, and every plan, is exactly what `build_plan` produces; the two
/// share the refusals `nothing_to_move` adds. `known` is as in
/// [`build_plan`].
pub fn plan_outcome(
    snapshot: &Snapshot,
    policy: &PolicyData,
    options: &AnalysisOptions,
    request: &PlanRequest,
    known: &[ProviderId],
) -> Result<PlanOutcome, DomainError> {
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
    // Only `packages` is read; `Snapshot` decides nothing that is shown (the
    // command's own flag comes from its context's source).
    let analysis = analyze(snapshot, policy, &opts, AnalysisSource::Snapshot);
    let unit = select_unit(&analysis.packages, &request.package)?;
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
        return nothing_to_move(
            &snapshot.providers,
            known,
            &unit.name,
            &unit.rows,
            &request.to,
        );
    };
    // DP-016(c). INV-19 forces `MANUAL_REVIEW` on a service package, and
    // `classify` decides that from `metadata.defines_service` — a bool that the
    // source provider fills. Where the adapter could not look, it reports
    // `Capability::ServiceMetadata: Unknown` and that bool is `false`, so the
    // rule silently does not fire and the plan looks ordinary. Under `--replay`
    // that is every MacPorts package, because the scan answers `Unreadable`
    // wherever this host's filesystem is not the machine being described (F-25,
    // F-29).
    //
    // The owner ruled (c): refuse the plan rather than weaken the rule. (a) —
    // treat `Unknown` as a service and force review — would have made every
    // replayed MacPorts plan a `MANUAL_REVIEW`, which costs `--replay` the one
    // thing it exists to demonstrate. Closing the path that needs the missing
    // fact costs only that path: `analyze`, `status` and `explain` never ask
    // this question and keep working.
    //
    // Asked of the **source**, because that is whose metadata classifies the
    // package being moved. A destination that cannot answer says nothing about
    // what is being removed.
    if snapshot.providers.get(&source).is_some_and(|p| {
        p.capabilities
            .cannot_answer(macportsbrew_domain::Capability::ServiceMetadata)
    }) {
        return Err(DomainError::new(
            ErrorId::SafetyUnverifiable,
            format!(
                "this input does not record whether {source} packages define services, so the \
                 rule that forces manual review for a service package cannot be evaluated for \
                 {}",
                unit.name
            ),
        ));
    }
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
                .with(
                    "class",
                    &macportsbrew_policy::labels::class(unit.class.dominant()),
                ),
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
        // The same parameters, in the same identifier spelling, as the policy
        // rule that raises this reason: `Debug` output is not a wire format.
        let reason = macportsbrew_policy::labels::mapping_confidence_params(confidence)
            .into_iter()
            .fold(
                Reason::new("reason.mapping_confidence_below_threshold")
                    .with("destination", &request.to),
                |reason, (name, value)| reason.with(name, &value),
            )
            .with(
                "candidates",
                &mapping.as_ref().map_or(0, |m| m.candidates.len()),
            );
        push_block(&mut risks, &mut manual, "mapping", reason);
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
                        &macportsbrew_policy::labels::withdrawal_state(
                            dest_row.disabled,
                            dest_row.deprecated,
                        ),
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
    // DP-027 (c): a provider whose own configuration overrides how this tool
    // would run it is reported on the plan, for both sides of it. Not
    // blocking - the plan is still the plan - but the execution gate refuses
    // every mutating step against that provider while the risk stands (DP-027
    // (b)). A present side whose configuration was not read - a replay's
    // `UNKNOWABLE`, a document from before the field - raises it too, as an
    // unknown: `migrate --replay --yes` reaches the gate, and an unknown is
    // never a yes (spec-guardian, 2026-10-04).
    let mut sides = vec![source.clone(), request.to.clone()];
    sides.dedup();
    for side in sides {
        let Some(snap) = snapshot.providers.get(&side) else {
            continue;
        };
        let reasons: Vec<Reason> = match &snap.configuration {
            macportsbrew_domain::ProviderConfiguration::Read { files } => files
                .iter()
                .filter_map(|file| {
                    if !file.readable {
                        Some(
                            Reason::new("reason.provider_configuration_unreadable")
                                .with("provider", &side)
                                .with("file", &file.path),
                        )
                    } else if file.keys.is_empty() {
                        None
                    } else {
                        let keys: Vec<&str> = file.keys.iter().map(String::as_str).collect();
                        Some(
                            Reason::new("reason.provider_configuration_outranks")
                                .with("provider", &side)
                                .with("file", &file.path)
                                .with("keys", &keys.join(", ")),
                        )
                    }
                })
                .collect(),
            macportsbrew_domain::ProviderConfiguration::Unknowable
            | macportsbrew_domain::ProviderConfiguration::NotRecorded => {
                if matches!(snap.detection.presence, ProviderPresence::Present) {
                    vec![Reason::new("reason.provider_configuration_unknown")
                        .with("provider", &side)]
                } else {
                    // An absent destination is its own blocking risk, and an
                    // absent provider is never run.
                    Vec::new()
                }
            }
        };
        for reason in reasons {
            risks.push(PlanRisk {
                id: PROVIDER_CONFIGURATION_RISK.into(),
                reason,
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
                // Stated here rather than read from `provider-commands.toml`,
                // for the same reason the elevation beside it is: this is not a
                // provider command, so that table does not describe it. This
                // tool's own exit status is the answer, and it is this tool
                // that will have to keep that true.
                success: SuccessSemantic::ExitStatus,
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
                // Carried nothing until 2026-09-19, which made this the only
                // verification step in the plan with a check and no way to
                // perform it - executor-debt item 3 in `plan.v1.md`. The
                // semantic is the mirror of what SE-083 gave
                // `VERIFY_DESTINATION`, and it is deliberately not that one
                // negated: see `SuccessSemantic::StdoutOmitsPackage` for the
                // three-cases-one-answer observation that rules out asking
                // about a single name on either provider.
                vec![command(
                    &commands,
                    src_snapshot,
                    &source,
                    "verify_absent",
                    &source_name,
                    kind,
                )?],
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
        dry_run_only: !THIS_BUILD_DISPATCHES_MUTATIONS,
    };
    plan.check_ordering()
        .map_err(|e| DomainError::new(ErrorId::Internal, e))?;
    Ok(PlanOutcome::Plan(Box::new(plan)))
}

/// The operations whose job is to read, and the only ones a detected
/// `Capability::UnprivilegedRead` may relax the elevation of.
///
/// A list rather than a predicate on the verb string, so that adding a read
/// verb is a decision made here rather than one that happens by spelling.
const READ_OPERATIONS: &[&str] = &["verify", "verify_absent"];

/// Operations whose argv carries no package name.
///
/// One today. It is a function rather than a `const` list inline so the reason
/// sits with the rule: see `SuccessSemantic::StdoutOmitsPackage`.
pub(crate) fn names_no_package(operation: &str) -> bool {
    operation == "verify_absent"
}

/// Whether `package` is one of a provider's reserved words — compared without
/// case and **without an `@…` suffix**, because MacPorts reads `installed@1` as
/// the pseudo-port `installed` with a version override (`/opt/local/bin/port`,
/// `element`, the `^(installed)(@.*)?$` patterns, MacPorts 2.12.6, read
/// 2026-10-10; security review, pass 08 slice 3, H2).
fn is_reserved(reserved: &[String], package: &str) -> bool {
    let word = package.split('@').next().unwrap_or(package);
    reserved.iter().any(|r| r.eq_ignore_ascii_case(word))
}

/// The success semantic the planner writes for `operation`, from the
/// provider's table entry; `Unknown` without one.
///
/// Shared with `execution_gate` for the reason [`planned_argv`] is: the gate
/// refuses a verification command whose semantic is not this, because a read
/// whose semantic was swapped — `EXIT_STATUS` on a verb that exits 0 for an
/// absent package — certifies nothing while looking like a check.
pub(crate) fn planned_success(
    entry: Option<&ProviderCommands>,
    operation: &str,
) -> SuccessSemantic {
    match entry.and_then(|e| e.success.get(operation).map(String::as_str)) {
        Some("exit_status") => SuccessSemantic::ExitStatus,
        Some("stdout_names_package") => SuccessSemantic::StdoutNamesPackage,
        Some("stdout_omits_package") => SuccessSemantic::StdoutOmitsPackage,
        _ => SuccessSemantic::Unknown,
    }
}

/// The argv the planner writes for `operation` on `package`, from the
/// provider's table entry — or, without one, an illustration.
///
/// The one derivation, shared with `execution_gate`, which refuses a
/// verification command whose argv is not this: a gate that re-derived it
/// would be a second spelling that could drift from the first.
pub(crate) fn planned_argv(
    entry: Option<&ProviderCommands>,
    operation: &str,
    package: &str,
) -> Vec<String> {
    let mut argv: Vec<String> = entry
        .and_then(|e| e.verbs.get(operation).cloned())
        .unwrap_or_else(|| vec![format!("<{operation}>")]);
    // `verify_absent` asks for the whole inventory and names no package, which
    // is the entire reason it can answer at all: on both providers, asking
    // about *one* name returns the same empty stdout for a package that was
    // removed, a name that never was one, and a provider that could not run
    // (observed 2026-09-19; the table states it beside both cells). So absence
    // is the name missing from a non-empty answer, and an empty one is refused.
    //
    // The name still travels in the plan - the executor matches stdout against
    // it - so every guard in `command` still applies to it. What changes is
    // only that it is not an argument.
    if !names_no_package(operation) {
        argv.push(package.to_string());
    }
    argv
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
    if entry.is_some_and(|e| is_reserved(&e.reserved_names, package)) {
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
            // Not `SnapshotUnusable`: this value may have come from a live
            // capture, a snapshot file or a recording, and the snapshot
            // remediation told a `--replay` user to go and take a fresh
            // snapshot (F-43).
            //
            // An installed name reaches here already refused by `check_names`.
            // The route that was *not* covered was the curated mapping table,
            // whose targets nothing validated - and for those this identifier
            // blamed the wrong party, because the defect would be in the build's
            // own data rather than in anything the user supplied. That was F-48's
            // first half and it is closed: `check_curated_names` refuses such a
            // row at load as `PolicyDataInvalid`, so a curated target can no
            // longer reach this arm. The comment stays because the reasoning for
            // *this* identifier still depends on it - what remains here is the
            // user-supplied route, which is the party this identifier names.
            ErrorId::InputUnusable,
            format!(
                "{package:?} is not a package name this tool will pass to a provider; \
                 it would be read as a flag or a path rather than as a package"
            ),
        ));
    }
    // And the program itself, last. This is not defence in depth: it is the
    // designed enforcement point, and `snapshot::prefix_shape_error` says so -
    // a `~`-rooted prefix is "allowed *here* and refused where it matters",
    // because it is an absolute path with the home elided by redaction and the
    // privacy machinery exists so a home-installed provider can be captured at
    // all. So the boundary passes `~/homebrew` and this guard is what stops it:
    // Spawning a program resolves a string containing a slash against the working
    // directory, which is the difference between running the provider and
    // running whatever is under the directory the user happened to be in.
    //
    // Reached by an ordinary machine, not only a doctored file: a user whose
    // Homebrew lives under `$HOME` can `analyze` and cannot `plan` (F-48).
    if !program.starts_with('<') && !Path::new(&program).is_absolute() {
        // The message said "the prefix it came from names no directory this
        // tool can resolve" — the symptom, for a machine whose only defect is
        // that its provider lives under `$HOME`, where the cause is the
        // redaction that removed the home, and the tool did the redacting.
        // Telling a user their prefix is unresolvable when this build made it
        // so is the misattribution F-48's other half was filed about, one layer
        // up (DP-021).
        //
        // **One message, because there is one reachable case.**
        // `is_safe_captured_path` admits a prefix only if it starts with `/`,
        // is `~`, or is `~/`-rooted, and `executable` is refused at load if it
        // is absolute — so a `program` that fails the absolute test above is
        // `~`-rooted, always. A first version branched on
        // `prefix_redacted || program.starts_with('~')` and kept a second arm
        // for the other case. `spec-guardian` showed the first term could be
        // deleted with every test still passing, because a *redacted absolute*
        // prefix never reaches this guard at all: the pairing `doctor` needs is
        // not the pairing this site needs, and shipping an untested term under
        // a comment calling it load-bearing is the F-19 class exactly.
        //
        // What holds the message honest is not a branch but
        // `is_safe_captured_path_admits_only_absolute_or_home_rooted`, which
        // pins the premise in the crate that owns it. Widen that and the test
        // fails naming this site.
        return Err(DomainError::new(
            // `InputUnusable` is still the wrong party, and it stays, because
            // the right one costs more than this change may spend.
            // `spec-guardian` proposed `CapabilityMissing` "same exit code 8" —
            // it is **5** (`error.rs:95`), so the swap is an exit-code change,
            // and `06-cli-and-ux.md` makes those a documented contract.
            // Its catalogue string also blames the provider, which is no more
            // true than blaming the input.
            //
            // Rewording `error.input_unusable` is not available either: its two
            // other callers — an unsafe package name at `:659` and a snapshot
            // that failed `check` (`capture.rs:148`) — are genuinely about the
            // input, and "check whatever produced it" is right for both.
            //
            // So the detail says the part the envelope gets wrong, in the
            // sentence the user actually reads, and the identifier question
            // goes to the owner with DP-021, where the rest of this decision
            // already is.
            ErrorId::InputUnusable,
            format!(
                "the plan would run {program:?}, and this build cannot build a step that \
                 runs a provider installed under the home directory: capture removes the \
                 home from the prefix, so nothing here can resolve it back to a program. \
                 This is a limit of this build, not a defect in anything you supplied, so \
                 nothing you change about the input will alter it. Analysis of this \
                 machine is unaffected"
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
    let argv = planned_argv(entry, operation, package);
    let elevation = elevation_for(
        entry,
        operation,
        kind,
        provider
            .capabilities
            .capabilities
            .get(&Capability::UnprivilegedRead),
    );
    // Unknown when the table does not say, and the fallback is the refusing
    // value on purpose: an executor that meets it must refuse the plan rather
    // than fall back to exit status, which on one of the two providers cannot
    // fail (F-33). The loader refuses an entry that omits the cell, so a plan
    // this build produces never carries `Unknown` from a provider it has an
    // entry for.
    //
    // No entry at all does reach here, and that case is always a placeholder
    // too - `placeholder` is `(prefix, entry)` being absent, so no-entry
    // implies it. Not the converse: a provider with an entry and no detected
    // prefix is a placeholder with a stated semantic, which is the ordinary
    // "what would happen if you installed it" plan.
    //
    // One narrower gap, recorded rather than closed: the loader keys this
    // check on `verbs` alone, where F-10 forced the elevation check onto the
    // union of both tables. An operation that lives only in `[entry.elevation]`
    // - `set_primary` is the one - would reach here with no cell and no loader
    // complaint. It cannot today, because `set_primary` does not route through
    // this function, and the fallback fails closed if it ever does.
    let success = planned_success(entry, operation);
    Ok(PlanCommand {
        placeholder,
        target: CommandTarget::Provider,
        program,
        argv,
        environment: provider.invocation_environment.clone(),
        elevation,
        success,
    })
}

/// The kind of the package `plan` moves, as the snapshot it was planned from
/// records it — the input the planner keys elevation on. A package that
/// snapshot does not hold is a `GUI_APP` here, whose mutating cells are
/// `unknown` in every table entry, so an undeterminable kind fails closed
/// (slice 4 review: a kind-blind re-derivation read a cask's `UNKNOWN` as
/// `NONE`).
pub(crate) fn package_kind_in(plan: &MigrationPlan, before: &Snapshot) -> PackageKind {
    before
        .providers
        .get(&plan.source)
        .and_then(|p| {
            p.inventory
                .installed
                .iter()
                .find(|r| r.identity.name == plan.package.name)
        })
        .map_or(PackageKind::GuiApp, |r| r.kind)
}

/// The elevation a command for `operation` needs: the table's cell for the
/// package kind, relaxed to `NONE` **only for a read operation** whose
/// provider detected `UNPRIVILEGED_READ` as available (INV-20; obligation 9).
///
/// The one derivation, shared by the planner, the execution gate's pin on
/// mutating steps, the run loop and the consent requests, so none of them reads
/// elevation from a plan's own field (pass 08 slice 4, D3). For a mutating
/// operation `unprivileged_read` changes nothing, so a caller that has no
/// snapshot may pass `None`.
pub(crate) fn elevation_for(
    entry: Option<&ProviderCommands>,
    operation: &str,
    kind: PackageKind,
    unprivileged_read: Option<&CapabilityState>,
) -> Elevation {
    let elevation_key = match kind {
        PackageKind::GuiApp => format!("{operation}_gui_app"),
        PackageKind::Package => operation.to_string(),
    };
    let stored = match entry.and_then(|e| e.elevation.get(&elevation_key).map(String::as_str)) {
        Some("none") => Elevation::None,
        Some("required") => Elevation::Required {
            reason: entry
                .and_then(|e| e.elevation_reason.clone())
                .unwrap_or_else(|| "provider prefix is not user-writable".into()),
        },
        _ => Elevation::Unknown,
    };
    // **INV-20's second sentence, which had no implementation until 2026-09-19.**
    // "Where behaviour can be detected at runtime, detection wins over stored
    // knowledge." Whether reading a provider needs administrator rights was a
    // string in `provider-commands.toml`, true of the one machine it was
    // observed on — and the registry a provider reads is created with the
    // installing user's umask, so a machine with a restrictive one gets a stored
    // answer that is wrong and nothing notices (F-24 closed the value, F-35 kept
    // the obligation open).
    //
    // Three deliberate narrowings, because "detection wins" is a dangerous
    // sentence taken further than this:
    //
    // 1. **Read operations only.** Relaxing an elevation is safe exactly where
    //    INV-06 already says the answer must be `None` — a step whose job is to
    //    read never elevates. Letting a detected capability relax `add` or
    //    `remove` would be this rule rewriting the answer for a *mutating* step,
    //    which is not what it observed.
    // 2. **Only `Available` wins.** The capability is never `Unavailable` (the
    //    adapters cannot tell a permission denial from a missing binary), so
    //    detection can confirm "no elevation needed" and can never assert the
    //    opposite. It cannot add a prompt nobody established.
    // 3. **`Unknown` falls back to the cell**, and does not fold to `Required`.
    //    SE-059 recorded that rendering an unknown as "needs administrator
    //    rights" is itself the INV-06 defect, because it trains a user to
    //    approve a password prompt for a read. Under `--replay` and
    //    `--from-snapshot` of a document written before this capability existed,
    //    `Unknown` is what is there, so those runs answer exactly as before.
    // 4. **Keyed on `elevation_key`, not on `operation`.** The stored value is
    //    looked up per package kind — `verify_gui_app` for a GUI application —
    //    and the first version of this branch tested the *un*-suffixed name, so
    //    a detected capability would have overridden a cell it never consulted.
    //    MacPorts states `verify_gui_app = "unknown"` precisely "so the planner
    //    never reaches a value by omission", and this would have turned that
    //    into a confident `none` for a package kind the provider does not have.
    //    Latent — `PackageClass::GuiApp` halts at `MANUAL_REVIEW` before
    //    `command()` — but F-22's note twenty lines up says in terms that only
    //    the elevation lookup is kind-aware and that the asymmetry is the
    //    defect; a second kind-blind consumer of a kind-aware lookup is that
    //    defect again (`spec-guardian`, 2026-09-19).
    if READ_OPERATIONS.contains(&elevation_key.as_str())
        && unprivileged_read == Some(&CapabilityState::Available)
    {
        Elevation::None
    } else {
        stored
    }
}

/// The unit `name` designates in an analysis: an exact unit name wins
/// outright; only if nothing matches by name are the names each provider
/// gives its rows consulted. Shared by `plan`, `migrate` and `explain`, so the
/// three never disagree about which package a name means.
///
/// One provider's name can be the counterpart of several of another's:
/// `python312` is what MacPorts calls the match for `python@3.12`,
/// `python@3.13` and `python@3.14`. Taking the first was silent and wrong, so
/// ambiguity is reported with the choices, like every other ambiguous match.
pub(crate) fn select_unit<'a>(
    packages: &'a [crate::report::PackageReport],
    name: &str,
) -> Result<&'a crate::report::PackageReport, DomainError> {
    let exact: Vec<&_> = packages.iter().filter(|p| p.name == name).collect();
    let matches = if exact.is_empty() {
        packages
            .iter()
            .filter(|p| p.rows.values().any(|r| r.name == name))
            .collect()
    } else {
        exact
    };
    match matches.as_slice() {
        [] => Err(DomainError::new(
            ErrorId::PackageNotFound,
            format!("{name} is not installed under any detected provider"),
        )),
        [only] => Ok(*only),
        many => {
            let mut names: Vec<String> = many
                .iter()
                .map(|u| format!("{} (under {})", u.name, u.source))
                .collect();
            names.sort_unstable();
            names.dedup();
            let holder = many[0]
                .rows
                .iter()
                .find(|(_, r)| r.name == name)
                .map_or("that provider", |(id, _)| id.as_str());
            Err(DomainError::new(
                ErrorId::Usage,
                format!(
                    "{name} is how {holder} names more than one package: {}. Name the one you mean.",
                    names.join(", ")
                ),
            ))
        }
    }
}

/// The one property of this module that is not reachable from
/// `crates/application/tests/`.
///
/// `build_plan` loads the commands table itself, so an integration test cannot
/// vary a stored elevation cell — and the shipped cells that disagree with
/// detection are the two MacPorts `_gui_app` ones, which no plan reaches
/// because a GUI application halts at `MANUAL_REVIEW` first. So the override
/// below changes no answer on any machine this project can test. That is exactly why it needs a test: a branch whose effect
/// is invisible in the fixtures is a branch that can be broken without anything
/// going red.
///
/// `command()` takes the table as an argument, so here it can. This is the third
/// `fn invariant_*` living in `src/` — beside `invariant_01` in
/// `domain/src/plan.rs` and `invariant_10` in `adapters/src/process/mod.rs` —
/// which is not the same as the third *test* there: F-40 counts 62 modules and
/// 346 tests in `src/`, and this is a 63rd module. The reason is the one F-40
/// itself names, a property not reachable without changing production
/// visibility, and the cost is real.
#[cfg(test)]
mod tests {
    #[test]
    fn a_reserved_word_is_reserved_whatever_its_case_or_version_suffix() {
        let reserved: Vec<String> = ["installed", "all", "not"]
            .iter()
            .map(|w| (*w).to_string())
            .collect();
        for word in [
            "installed",
            "INSTALLED",
            "installed@1",
            "all@",
            "Not",
            "not@x",
        ] {
            assert!(is_reserved(&reserved, word), "{word}");
        }
        for name in ["jq", "installedx", "x@installed", "python@3.12", ""] {
            assert!(!is_reserved(&reserved, name), "{name}");
        }
    }

    // Named `tests` and not for tidiness. The INV-20 textual scan in
    // `crates/policy/tests/invariants.rs` skips a test module by matching
    // `mod tests` literally — deliberately, because an earlier version latched
    // on any `#[cfg(test)]` and silently excluded 27% of the workspace. So a
    // module called anything else is read as production source, and the
    // `/opt/local` fixture below is then flagged as a provider prefix written
    // into code. It errs loud, which is the right direction, and this comment
    // is here because the first draft of this module was called
    // `elevation_preference` and failed that gate.
    use super::*;
    use macportsbrew_domain::{
        Architecture, ProviderCapabilities, ProviderDetection, ProviderInventory, ProviderPresence,
        Version,
    };
    use std::path::PathBuf;

    // The no-holder arm of `nothing_to_move` is tested here rather than
    // through `plan_outcome` because it cannot be reached from a snapshot:
    // `analyze` builds every unit from an installed row. The guard still has
    // to hold, so it is exercised on the function that carries it.
    #[test]
    fn a_unit_held_by_nobody_is_an_internal_error_not_already_at_its_destination() {
        let dest = ProviderId::new("dest");
        let err = nothing_to_move(&BTreeMap::new(), &[], "pkg", &BTreeMap::new(), &dest)
            .expect_err("nobody holds it");
        assert_eq!(err.id, ErrorId::Internal);
        assert!(err.detail.contains("no provider holds"), "{}", err.detail);

        // A destination *row* is not a destination *copy*: `analyze` also
        // writes a row for a mapped counterpart that is not installed. Such a
        // row with nothing in it must not pass for the package being there.
        let mapped = ProviderRow {
            name: "pkg".into(),
            installed: Vec::new(),
            ownership: macportsbrew_domain::Ownership::Unknown,
            ownership_evidence: macportsbrew_domain::OwnershipEvidence {
                mechanism: "not_installed".into(),
                authoritative: true,
            },
            available_version: None,
            availability: macportsbrew_domain::AvailabilityAssessment::unknown("test", "test"),
            deprecated: false,
            disabled: false,
            dependency_count: 0,
            reverse_dependency_count: 0,
            presence: crate::report::RowPresence::Mapped,
        };
        let rows = BTreeMap::from([(dest.clone(), mapped)]);
        let err = nothing_to_move(&BTreeMap::new(), &[], "pkg", &rows, &dest)
            .expect_err("an empty destination row holds nothing");
        assert_eq!(err.id, ErrorId::Internal, "{}", err.detail);
    }

    /// A minimal snapshot of one provider, with the capability set as given.
    fn provider(detected: Option<CapabilityState>) -> macportsbrew_domain::ProviderSnapshot {
        let id = ProviderId::new("macports");
        let mut capabilities = BTreeMap::new();
        if let Some(state) = detected {
            capabilities.insert(Capability::UnprivilegedRead, state);
        }
        macportsbrew_domain::ProviderSnapshot {
            detection: ProviderDetection {
                provider: id,
                presence: ProviderPresence::Present,
                version: Some(Version::new("2.12.6")),
                prefix: Some(PathBuf::from("/opt/local")),
                executable: None,
                architecture: Some(Architecture::Arm64),
                prefix_redacted: false,
            },
            capabilities: ProviderCapabilities { capabilities },
            inventory: ProviderInventory::default(),
            environment: BTreeMap::new(),
            invocation_environment: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            configuration: macportsbrew_domain::ProviderConfiguration::NotRecorded,
        }
    }

    /// The shipped table with both read cells forced to `required`, which no
    /// real entry says — that is the point. Without it, "detection wins" is a
    /// claim about a value that agrees with detection anyway.
    /// The two read verbs, named **literally** rather than taken from
    /// `READ_OPERATIONS`.
    ///
    /// The first version of this module drew both the forced cells and the
    /// asserted operations from that constant, so deleting `"verify_absent"`
    /// from it shrank the test with it and every assertion stayed green — while
    /// a stored `required` on the removal-verification read would have survived
    /// detection and put an administrator prompt on the step that closes
    /// INV-01's loop. A constant must not both define and grade its own test
    /// (`spec-guardian`, 2026-09-19).
    const READS: &[&str] = &["verify", "verify_absent"];

    fn table_demanding_elevation_for_reads() -> BTreeMap<String, ProviderCommands> {
        let mut table = provider_commands::load().expect("the shipped table parses");
        for entry in table.values_mut() {
            for op in READS {
                entry
                    .elevation
                    .insert((*op).to_string(), "required".to_string());
            }
        }
        table
    }

    fn elevation_of(operation: &str, detected: Option<CapabilityState>) -> Elevation {
        elevation_of_kind(operation, detected, PackageKind::Package)
    }

    fn elevation_of_kind(
        operation: &str,
        detected: Option<CapabilityState>,
        kind: PackageKind,
    ) -> Elevation {
        command(
            &table_demanding_elevation_for_reads(),
            &provider(detected),
            &ProviderId::new("macports"),
            operation,
            "htop",
            kind,
        )
        .expect("the command builds")
        .elevation
    }

    /// `elevation_for` is what the gate, the run loop and the consent requests
    /// re-derive elevation with (pass 08 slice 4, D3): a detected capability
    /// never relaxes a **mutating** operation, so a caller with no snapshot
    /// gets the same answer as the planner for every write, and the planner's
    /// commands agree with it for every operation and capability.
    #[test]
    fn invariant_06_a_capability_never_relaxes_a_mutating_operation() {
        let table = provider_commands::load().expect("the shipped table parses");
        for (id, entry) in &table {
            for operation in ["add", "remove"] {
                for kind in [PackageKind::Package, PackageKind::GuiApp] {
                    let stored = elevation_for(Some(entry), operation, kind, None);
                    for detected in [
                        None,
                        Some(CapabilityState::Unknown),
                        Some(CapabilityState::Available),
                    ] {
                        assert_eq!(
                            elevation_for(Some(entry), operation, kind, detected.as_ref()),
                            stored,
                            "{id} {operation} {kind:?} {detected:?}"
                        );
                    }
                }
            }
            for operation in ["add", "remove", "verify", "verify_absent"] {
                for detected in [None, Some(CapabilityState::Available)] {
                    let planned = command(
                        &table,
                        &provider(detected.clone()),
                        &ProviderId::new(id.as_str()),
                        operation,
                        "htop",
                        PackageKind::Package,
                    )
                    .expect("the command builds")
                    .elevation;
                    assert_eq!(
                        planned,
                        elevation_for(
                            Some(entry),
                            operation,
                            PackageKind::Package,
                            detected.as_ref()
                        ),
                        "{id} {operation}"
                    );
                }
            }
        }
    }

    #[test]
    fn invariant_20_a_detected_read_beats_a_stored_required() {
        // The fallback first, or the other half proves nothing: with no
        // capability recorded — every snapshot and recording written before it
        // existed, and so every `--replay` run — the stored cell decides.
        assert!(
            matches!(elevation_of("verify", None), Elevation::Required { .. }),
            "with nothing detected the table must decide, or there is no fallback"
        );
        assert!(
            matches!(
                elevation_of("verify", Some(CapabilityState::Unknown)),
                Elevation::Required { .. }
            ),
            "and `Unknown` is not a detection: folding it to `none` would be reading an \
             unknown as a yes, and folding it to `required` is the INV-06 defect SE-059 \
             recorded — either way the cell decides"
        );

        // The constant is the thing under test, so assert what it contains
        // rather than iterating it and calling that coverage.
        assert_eq!(
            READ_OPERATIONS, READS,
            "the operations detection may relax are the read verbs and nothing else"
        );

        // And the sentence itself.
        for operation in READS {
            assert_eq!(
                elevation_of(operation, Some(CapabilityState::Available)),
                Elevation::None,
                "{operation}: a machine that demonstrated it can be read unprivileged must \
                 beat a stored `required` (INV-20: detection wins over stored knowledge)"
            );
        }
    }

    #[test]
    fn a_detected_read_never_relaxes_a_cell_it_did_not_consult() {
        // The stored value is looked up per package kind; the override must be
        // too. MacPorts states `verify_gui_app = "unknown"` so the planner
        // "never reaches a value by omission", and an override keyed on the
        // un-suffixed `verify` turned that into a confident `none` for a
        // package kind this provider does not have. The forced table above
        // touches only the un-suffixed keys, so the `_gui_app` cells are the
        // shipped `unknown` — which is exactly the value that must survive.
        for operation in READS {
            assert_eq!(
                elevation_of_kind(
                    operation,
                    Some(CapabilityState::Available),
                    PackageKind::GuiApp
                ),
                Elevation::Unknown,
                "{operation}_gui_app is `unknown` in the shipped data and detection says \
                 nothing about it; overriding it would be reaching a value by omission"
            );
        }
    }

    #[test]
    fn a_detected_read_never_relaxes_a_mutation() {
        // The narrowing that makes the override safe. `add` and `remove` are
        // `required` for MacPorts in the shipped data; an observation about a
        // *read* says nothing about them, and a rule that relaxed them would be
        // rewriting the elevation of a mutating step from evidence it does not
        // have.
        for operation in ["add", "remove"] {
            assert!(
                matches!(
                    elevation_of(operation, Some(CapabilityState::Available)),
                    Elevation::Required { .. }
                ),
                "{operation} must keep its stored elevation however the read behaves"
            );
        }
    }
}
