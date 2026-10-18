// SPDX-License-Identifier: Apache-2.0

//! `restore --plan --run <id>`: the plan that puts back what a run changed
//! (pass 08 slice 5, Part C; D9, D10).
//!
//! **A restore is a migration the other way, and nothing here executes
//! anything.** The plan is an ordinary `MigrationPlan` built by the one planner
//! from the live capture, so the ordering (INV-01), the reverse-dependency rule
//! (INV-04) and the stateful/service stop (INV-19) are the planner's and are
//! not restated. What this module adds is deciding *what to ask for* from the
//! run's record and the snapshot it took first (`application::restore`),
//! reading both without trusting either, and saying what the plan cannot.
//!
//! `restore --run <id>` without `--plan` builds the same plan and puts it to the
//! same execution gate `migrate --yes` reaches, which refuses it: exit 4,
//! nothing dispatched, no lock taken. `restore --snapshot <file>` is not built.
//!
//! Refused first - before the state directory is resolved - under `--replay`
//! and `--from-snapshot` (INV-15: history is this machine's, now) and as root
//! (INV-06, `run_inner`'s refusal).

use super::context::{Context, HistoryScope};
use super::history::Shown;
use super::migrate::{execution, state_and_user, Execution, Mode};
use super::{human, json, Cli};
use crate::executor::refused_report;
use macportsbrew_adapters::persistence::history_store;
use macportsbrew_adapters::persistence::run_marker;
use macportsbrew_adapters::persistence::snapshot_store::{state_snapshot, StateSnapshot};
use macportsbrew_application::{
    check_same_package, execution_refusals, plan_outcome, restorability, restore_notes,
    restore_request, AnalysisOptions, KeepReason, PlanOutcome, RestoreIntent, RestoreNote,
    RestoreRefusal, SnapshotLook,
};
use macportsbrew_domain::{
    is_run_id, ConcurrencyFlag, DomainError, ErrorId, MigrationPlan, NotRestorable, RecordStanding,
    Restorability, RunRecord,
};
use serde::Serialize;
use std::io::Write;
use std::path::Path;

/// The version of `restore.v1.md`.
pub const RESTORE_SCHEMA_VERSION: u32 = 1;

/// The refusals that come before a context is built: the scope, the arguments.
/// Nothing is read or resolved for any of them.
pub(super) fn refuse_before_context(
    cli: &Cli,
    run: Option<&str>,
    snapshot: Option<&Path>,
) -> Result<(), DomainError> {
    if cli.replay.is_some() || cli.from_snapshot.is_some() {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            "restore puts back what this machine's own history says a run changed, so it is \
             not built for a recording or a snapshot file; nothing was read",
        ));
    }
    if snapshot.is_some() {
        return Err(DomainError::new(
            ErrorId::NotImplementedInThisPass,
            "restore --snapshot <file> (the additive whole-machine reconcile) is not built in \
             this version; restore --plan --run <id> plans the undo of one run",
        ));
    }
    match run {
        None => Err(DomainError::new(
            ErrorId::Usage,
            "restore needs --run <id>, as `macportsbrew history` lists them",
        )),
        Some(id) if !is_run_id(id) => Err(DomainError::new(
            ErrorId::Usage,
            "that is not a run id: they look like 20260101T000000Z-0123456789abcdef, and \
             `macportsbrew history` lists them",
        )),
        Some(_) => Ok(()),
    }
}

/// Why there is nothing to restore.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum NothingToRestoreReason {
    /// The run dispatched no step that changes the machine.
    RunChangedNothing,
    /// The machine is already as the run found it.
    AlreadyRestored,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RestoreOf<'a> {
    run_id: &'a str,
    snapshot_file: Option<&'a str>,
    standing: RecordStanding,
    #[serde(flatten)]
    restorability: Restorability,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'a> {
    schema_version: u32,
    mode: Mode,
    restore_of: RestoreOf<'a>,
    notes: &'a [RestoreNote],
    #[serde(skip_serializing_if = "Option::is_none")]
    nothing_to_restore: Option<NothingToRestoreReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<&'a MigrationPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution: Option<Execution<'a>>,
    concurrent_provider_activity: ConcurrencyFlag,
}

/// Runs `restore` and returns its exit code. Everything is decided before
/// anything is written, so a refusal leaves stdout empty.
pub(super) fn restore(
    ctx: &Context,
    cli: &Cli,
    plan_only: bool,
    run_id: &str,
    options: &AnalysisOptions,
    stdout: &mut dyn Write,
) -> Result<i32, DomainError> {
    // The scope again, from what the context is, for the one caller that is not
    // the command line (a test builds a context from a recording).
    if ctx.history != HistoryScope::ThisMachine {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            "restore reads this machine's own history, never a recording or a snapshot file; \
             nothing was read",
        ));
    }
    let (state, euid) = state_and_user(ctx)?;
    let record = history_store::read_one(&state, run_id, euid)?.ok_or_else(|| {
        DomainError::new(
            ErrorId::RunNotFound,
            format!("run {run_id} is not recorded"),
        )
    })?;
    let standing = run_marker::marker_state(&state, euid).standing_of(record.id());
    if standing == RecordStanding::Unknown {
        return Err(DomainError::new(
            ErrorId::HistoryUnusable,
            "the interrupted-run marker could not be read, so whether run {run_id}'s record \
             is confirmed is not known and no restore plan is offered"
                .replace("{run_id}", record.id()),
        ));
    }
    let (look, before) = match record.snapshot_path() {
        None => (SnapshotLook::Missing, None),
        Some(name) => match state_snapshot(&state, name, euid) {
            StateSnapshot::Present(snapshot) => (
                SnapshotLook::Present {
                    captured_at: snapshot.provenance.captured_at.clone(),
                },
                Some(snapshot),
            ),
            StateSnapshot::Missing => (SnapshotLook::Missing, None),
            StateSnapshot::Unreadable(_) => (SnapshotLook::Unreadable, None),
        },
    };
    // What is shown is the record's real standing; whether a plan can be
    // *built* ignores it, because the person must see the undo plan of a run the
    // marker still names before they decide what to do about the marker.
    let shown = restorability(&record, &look, standing);
    let plannable = restorability(&record, &look, RecordStanding::Confirmed);
    match plannable {
        Restorability::NothingToRestore => {
            return nothing_to_restore(
                ctx,
                cli,
                plan_only,
                &record,
                standing,
                shown,
                NothingToRestoreReason::RunChangedNothing,
                stdout,
            );
        }
        Restorability::Not { reason } => return Err(unplannable(&record, reason)),
        Restorability::FromSnapshot | Restorability::Partially { .. } => {}
    }
    let before = before.ok_or_else(|| unplannable(&record, NotRestorable::SnapshotUnreadable))?;
    let intent = match restore_request(&record, &before) {
        Ok(intent) => intent,
        Err(RestoreRefusal::NothingDispatched) => {
            return nothing_to_restore(
                ctx,
                cli,
                plan_only,
                &record,
                standing,
                shown,
                NothingToRestoreReason::RunChangedNothing,
                stdout,
            );
        }
        Err(refusal) => return Err(not_the_runs_package(&refusal)),
    };
    let outcome = plan_outcome(
        &ctx.snapshot,
        &ctx.policy,
        options,
        &intent.request,
        &macportsbrew_adapters::registry::known_provider_ids(),
    )?;
    let plan = match outcome {
        PlanOutcome::AlreadyAtDestination { .. } => {
            return nothing_to_restore(
                ctx,
                cli,
                plan_only,
                &record,
                standing,
                shown,
                NothingToRestoreReason::AlreadyRestored,
                stdout,
            );
        }
        PlanOutcome::Plan(plan) => *plan,
    };
    check_same_package(&intent, &plan).map_err(|refusal| not_the_runs_package(&refusal))?;
    let notes = restore_notes(
        &intent,
        matches!(plannable, Restorability::Partially { .. }),
        standing == RecordStanding::Unconfirmed,
    );
    // Executing a record not known to be durable is refused before the gate is
    // asked: the gate would refuse it anyway in this build, and this is the
    // reason the person is owed.
    if !plan_only && standing == RecordStanding::Unconfirmed {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            format!(
                "run {run_id}'s record is not confirmed (the interrupted-run marker still \
                 names it), so it is not offered for restore; `restore --plan --run {run_id}` \
                 shows the plan for review"
            ),
        ));
    }
    // The same gate `migrate --yes` reaches. It refuses every plan this build
    // produces, so nothing is dispatched; a plan that passed it would be a
    // defect, and is reported as one rather than run.
    let report = if plan_only {
        None
    } else {
        let gate = execution_refusals(&plan);
        let report = refused_report(&gate, &ctx.redaction);
        if report.gate_refusals.is_empty() {
            return Err(DomainError::new(
                ErrorId::Internal,
                "the execution gate passed a restore plan, and this build cannot execute one",
            ));
        }
        Some(report)
    };
    let concurrency = ConcurrencyFlag::of(ctx.source.concurrent_provider_activity());
    let io = |e: std::io::Error| DomainError::new(ErrorId::Io, e.to_string());
    let mode = if plan_only {
        Mode::Render
    } else {
        Mode::Execute
    };
    if cli.json {
        let document = Document {
            schema_version: RESTORE_SCHEMA_VERSION,
            mode,
            restore_of: RestoreOf {
                run_id: record.id(),
                snapshot_file: record.snapshot_path(),
                standing,
                restorability: shown,
            },
            notes: &notes,
            nothing_to_restore: None,
            plan: Some(&plan),
            execution: report.as_ref().map(execution),
            concurrent_provider_activity: concurrency.clone(),
        };
        writeln!(stdout, "{}", json(&document)?).map_err(io)?;
    } else {
        let text = Shown {
            messages: &ctx.messages,
            redaction: ctx.redaction.clone(),
        };
        write!(
            stdout,
            "{}",
            header(&text, &record, standing, shown, &notes, &intent)
        )
        .map_err(io)?;
        write!(stdout, "{}", human::plan(ctx, &plan)).map_err(io)?;
        match &report {
            None => write!(
                stdout,
                "\n{}\n",
                ctx.messages.render_id("restore.render_only")
            )
            .map_err(io)?,
            Some(report) => {
                write!(stdout, "{}", human::run_report(ctx, &plan, report)).map_err(io)?;
            }
        }
        write!(
            stdout,
            "{}",
            human::concurrency(ctx, concurrency.activity())
        )
        .map_err(io)?;
    }
    Ok(match &report {
        Some(report) => report.exit_code(),
        // As `plan`: a plan stopping at manual review is shown, and exits 4.
        None if plan.requires_manual_review() => ErrorId::SafetyBlocked.exit_code(),
        None => 0,
    })
}

/// Reports that there is nothing to put back, and exits 0 (INV-16).
#[allow(clippy::too_many_arguments)]
fn nothing_to_restore(
    ctx: &Context,
    cli: &Cli,
    plan_only: bool,
    record: &RunRecord,
    standing: RecordStanding,
    shown: Restorability,
    reason: NothingToRestoreReason,
    stdout: &mut dyn Write,
) -> Result<i32, DomainError> {
    let concurrency = ConcurrencyFlag::of(ctx.source.concurrent_provider_activity());
    let io = |e: std::io::Error| DomainError::new(ErrorId::Io, e.to_string());
    if cli.json {
        let document = Document {
            schema_version: RESTORE_SCHEMA_VERSION,
            mode: if plan_only {
                Mode::Render
            } else {
                Mode::Execute
            },
            restore_of: RestoreOf {
                run_id: record.id(),
                snapshot_file: record.snapshot_path(),
                standing,
                restorability: shown,
            },
            notes: &[],
            nothing_to_restore: Some(reason),
            plan: None,
            execution: None,
            concurrent_provider_activity: concurrency.clone(),
        };
        writeln!(stdout, "{}", json(&document)?).map_err(io)?;
    } else {
        let text = Shown {
            messages: &ctx.messages,
            redaction: ctx.redaction.clone(),
        };
        let id = match reason {
            NothingToRestoreReason::RunChangedNothing => "restore.nothing_run_changed_nothing",
            NothingToRestoreReason::AlreadyRestored => "restore.nothing_already_restored",
        };
        let subject = record.subject();
        let line = text.say(
            id,
            &[
                ("id", record.id().to_owned()),
                ("package", text.plain(&subject.package.name)),
                ("source", text.plain(subject.direction.source.as_str())),
            ],
        );
        writeln!(stdout, "{line}").map_err(io)?;
        write!(
            stdout,
            "{}",
            human::concurrency(ctx, concurrency.activity())
        )
        .map_err(io)?;
    }
    Ok(0)
}

/// The error for a run that cannot be planned: its snapshot is the reason, and
/// "missing or unreadable" is exit 8 (`06-cli-and-ux.md`).
fn unplannable(record: &RunRecord, reason: NotRestorable) -> DomainError {
    let why = match reason {
        NotRestorable::NoSnapshotTaken => "no snapshot was taken before its first change",
        NotRestorable::SnapshotMissing => {
            "the snapshot its record names is not in the state directory"
        }
        NotRestorable::SnapshotUnreadable => {
            "the snapshot its record names cannot be trusted or read"
        }
        NotRestorable::SnapshotNotThisRuns => {
            "the snapshot its record names is not the one it took"
        }
        NotRestorable::RecordNotConfirmed => "its record is not confirmed",
        NotRestorable::StandingUnknown => "whether its record is confirmed cannot be established",
        NotRestorable::OutcomeUncertain => "what it did is not known well enough",
    };
    DomainError::new(
        ErrorId::SnapshotUnusable,
        format!("run {} cannot be planned for restore: {why}", record.id()),
    )
}

fn not_the_runs_package(refusal: &RestoreRefusal) -> DomainError {
    match refusal {
        RestoreRefusal::NotTheRunsPackage { planned, recorded } => DomainError::new(
            ErrorId::SafetyBlocked,
            format!(
                "the plan the planner built would restore {planned}, and the run moved \
                 {recorded}; a restore never substitutes another package, so nothing is offered"
            ),
        ),
        RestoreRefusal::NothingDispatched => DomainError::new(
            ErrorId::Internal,
            "a restore request was refused as nothing to restore after it was asked for",
        ),
    }
}

/// What the person reads before the plan: which run, what is known about its
/// record, and what the plan cannot say.
fn header(
    text: &Shown<'_>,
    record: &RunRecord,
    standing: RecordStanding,
    shown: Restorability,
    notes: &[RestoreNote],
    intent: &RestoreIntent,
) -> String {
    let subject = record.subject();
    let mut out = String::new();
    let mut line = |s: String| {
        out.push_str(&s);
        out.push('\n');
    };
    line(text.say(
        "restore.header",
        &[
            ("id", record.id().to_owned()),
            ("package", text.plain(&subject.package.name)),
            ("source", text.plain(subject.direction.source.as_str())),
            (
                "destination",
                text.plain(subject.direction.destination.as_str()),
            ),
        ],
    ));
    line(text.say(
        "history.show_standing",
        &[("standing", text.standing(standing))],
    ));
    line(text.say(
        "history.show_restorability",
        &[("restorability", text.restorability(shown))],
    ));
    let at = text.plain(intent.restore_to.as_str());
    let present = text.plain(&intent.request.package);
    for note in notes {
        line(match note {
            RestoreNote::NotVersionExact {
                package,
                recorded_version: Some(version),
            } => text.say(
                "restore.note.not_version_exact",
                &[
                    ("provider", at.clone()),
                    ("package", text.plain(package)),
                    ("version", text.plain(version)),
                ],
            ),
            RestoreNote::NotVersionExact {
                package,
                recorded_version: None,
            } => text.say(
                "restore.note.not_version_exact_unstated",
                &[("provider", at.clone()), ("package", text.plain(package))],
            ),
            RestoreNote::KeepsDestination { reason } => text.say(
                match reason {
                    KeepReason::AddNotPerformed => "restore.note.keeps.add_not_performed",
                    KeepReason::DestinationHeldItBefore => {
                        "restore.note.keeps.destination_held_it_before"
                    }
                    KeepReason::BeforeStateNotEstablished => {
                        "restore.note.keeps.before_state_not_established"
                    }
                },
                &[
                    ("provider", text.plain(intent.restore_from.as_str())),
                    ("package", present.clone()),
                ],
            ),
            RestoreNote::RecordNotConfirmed => text.say(
                "restore.note.record_not_confirmed",
                &[("id", record.id().to_owned())],
            ),
            RestoreNote::OutcomeUncertain => {
                text.messages.render_id("restore.note.outcome_uncertain")
            }
        });
    }
    out
}

// The tests name the providers of the recording they replay, so they live in
// their own file: this one is scanned word by word for a provider name.
#[cfg(test)]
#[path = "restore_tests.rs"]
mod tests;
