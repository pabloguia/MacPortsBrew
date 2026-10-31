// SPDX-License-Identifier: Apache-2.0

//! `migrate <package> --to <id> [--retain-source]`: the one caller of the run
//! loop (SE-092), and the command whose `--json` is `docs/schemas/migrate.v1.md`.
//!
//! **Two modes.** It renders by default, as `--dry-run` asks and
//! `06-cli-and-ux.md` requires of anything destructive until explicitly
//! confirmed. It executes only with `--yes` and without `--dry-run`, and
//! `--dry-run` wins when both are given. Executing means putting the plan to
//! the execution gate and, only if the gate passes it, running the plan's
//! verification reads; the gate refuses every plan this build produces, so in
//! this build execution ends at the gate with exit 4 and the refusals in the
//! document.
//!
//! **The flow, in order.**
//!
//! 1. `--from-snapshot` with execute mode is a usage error before anything is
//!    printed (`run_inner` checks it before the context is built): a snapshot
//!    file carries no provenance and no runner, so nothing may execute
//!    against it (`04-migration-and-safety.md`; SE-050).
//! 2. [`migration_start`] with the registry's provider list. *Nothing to
//!    move* is reported and exits 0 (INV-16) — only where it can be shown;
//!    everything else that is not a plan is the planner's own refusal.
//! 3. The plan is rendered. Rendering is never refused.
//! 4. In execute mode the run loop gets the plan, paired with the snapshot it
//!    was built from, and the registry context the capture's adapters were
//!    built from — the same runner, prefix sources and home. The loop
//!    re-detects each prefix rather than reusing capture's answer; a prefix
//!    that changed since capture makes the plan's program disagree with the
//!    derived one, and the read is refused as `ProgramDisagrees`. The exit
//!    code is the run's.
//! 5. Whenever a plan was built, stdout carries it, so an exit 4 with a
//!    document (a refusal or a manual review) is distinguishable from the
//!    planner's own exit 4 with empty stdout. **Except** a refusal before
//!    anything ran that is about the plan's confirmation (work order 16 part
//!    E, R2): the plan not confirmed (`USER_ABORTED`, exit 6), no terminal to
//!    confirm it at (`USAGE`, exit 2), or the plan to run not the one
//!    confirmed (`PLAN_NOT_SHOWN`, exit 4). Stdout is empty for each, and the
//!    error's identifier, not stdout, tells them from the planner's.
//!
//! **INV-02.** Under a mutation allowance, the plan executed is the plan a
//! person was shown and confirmed (`plan.v1.md`, obligation 8): after the
//! gate clears it, the plan is shown on standard error and `run plan <code>`
//! is asked for; the run loop refuses a plan the confirmation does not cover.
//! `--yes` means "execute" and never answers a question. Without an
//! allowance (this build) the read-only loop runs and nobody is asked.
//!
//! **Rules for this file** (they are the run loop's, and the loop's word scans
//! read it): it names no provider; it never holds a read site, a runner or a
//! command spec — `executor::run` builds the sites from the context itself,
//! after the gate; and it reads neither the build's mutation constant nor the
//! plan's claim about it.

use super::context::{Context, HistoryScope};
use super::{human, json, Cli};
use crate::executor::{self, RunReceipt, RunReport, SiteRefusalKind};
use macportsbrew_adapters::persistence::history_store::read_view;
use macportsbrew_adapters::persistence::state_dir::StateDir;
use macportsbrew_adapters::system::machine::effective_uid;
use macportsbrew_application::{
    clear, migration_key, migration_start, mutation_allowance, plan_question_error,
    AnalysisOptions, GateClear, HistoryConsultation, MigrationStart, MutationAllowance,
    NotRecognised, PlanRequest, PlannedRun, RunOutcome, ShownPlan, StepRecord,
};
use macportsbrew_domain::{
    ConcurrencyFlag, DomainError, ErrorId, HistoryView, MigrationKey, MigrationPlan, ProviderId,
    RunStart,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Write;

/// The version of `migrate.v1.md`.
pub const MIGRATE_SCHEMA_VERSION: u32 = 1;

/// Whether `migrate` renders or executes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Mode {
    /// Show the plan and run nothing (the default, and `--dry-run`).
    Render,
    /// Put the plan to the execution gate (`--yes` without `--dry-run`).
    Execute,
}

impl Mode {
    /// The mode the flags ask for: execute only on `--yes`, and `--dry-run`
    /// wins over it.
    #[must_use]
    pub fn of(cli: &Cli) -> Self {
        if cli.yes && !cli.dry_run {
            Self::Execute
        } else {
            Self::Render
        }
    }
}

/// The refusal `run_inner` gives before building anything: execute mode on a
/// snapshot file.
pub fn refuse_execution_from_a_snapshot_file() -> DomainError {
    DomainError::new(
        ErrorId::Usage,
        "`migrate --yes` cannot execute against --from-snapshot: a snapshot file has no \
         runner and records no provenance an execution could check. Use --dry-run to show \
         the plan, or run without --from-snapshot",
    )
}

/// `migrate.v1`'s document.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'a> {
    schema_version: u32,
    mode: Mode,
    #[serde(skip_serializing_if = "Option::is_none")]
    nothing_to_move: Option<NothingToMove<'a>>,
    /// A `--retain-source` re-run history and a fresh capture show done
    /// (INV-16; pass 08 slice 1). Additive to v1.
    #[serde(skip_serializing_if = "Option::is_none")]
    retained_run_done: Option<RetainedRunDone<'a>>,
    /// Why a consulted history did not recognise this re-run as done, when it
    /// had a record to say something about. Additive to v1.
    #[serde(skip_serializing_if = "Option::is_none")]
    retained_run_not_recognised: Option<NotRecognisedDoc<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<&'a MigrationPlan>,
    /// DP-037: derived from the plan's `enteredBy`. Additive to v1.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<super::PlanWarning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution: Option<Execution<'a>>,
    /// Always written: whether a provider may have been changing the machine
    /// while it was read, and never a check this build did not make
    /// (`plan.v1.md`, obligation 5).
    concurrent_provider_activity: ConcurrencyFlag,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NothingToMove<'a> {
    package: &'a str,
    provider: &'a ProviderId,
    /// Always written: the other providers that were not detected, so a
    /// script tells "seen to be done" from "done as far as detection can
    /// see" without parsing text (DP-026).
    not_detected: &'a [ProviderId],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RetainedRunDone<'a> {
    package: &'a str,
    provider: &'a ProviderId,
    run_id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NotRecognisedDoc<'a> {
    reason: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<&'a str>,
}

/// The stable name of a reason, as the document and the catalogue key it.
/// Exhaustive: a new reason is a compile error here, not a silent gap.
fn not_recognised(why: &NotRecognised) -> NotRecognisedDoc<'_> {
    let (reason, run_id) = match why {
        NotRecognised::RecordNotConfirmed { run_id } => ("RECORD_NOT_CONFIRMED", Some(run_id)),
        NotRecognised::LatestRunNotSuccess { run_id } => ("LATEST_RUN_NOT_SUCCESS", Some(run_id)),
        NotRecognised::LatestRunRemovedSource { run_id } => {
            ("LATEST_RUN_REMOVED_SOURCE", Some(run_id))
        }
        NotRecognised::Tie => ("TIE", None),
        NotRecognised::RecordFromTheFuture => ("RECORD_FROM_THE_FUTURE", None),
        NotRecognised::DestinationNotActive { run_id } => ("DESTINATION_NOT_ACTIVE", Some(run_id)),
        NotRecognised::PrimaryIsShim { run_id } => ("PRIMARY_IS_SHIM", Some(run_id)),
        NotRecognised::PrimaryNotConfirmed { run_id } => ("PRIMARY_NOT_CONFIRMED", Some(run_id)),
    };
    NotRecognisedDoc {
        reason,
        run_id: run_id.map(String::as_str),
    }
}

/// Reads what this machine's history says about `key`, through the store.
/// The state directory is resolved as `snapshot` resolves it; nothing is
/// created by reading.
fn read_history(ctx: &Context, key: &MigrationKey) -> Result<HistoryView, DomainError> {
    let (state, euid) = state_and_user(ctx)?;
    read_view(&state, key, euid)
}

/// The state directory and the effective user, from the values the context
/// read once (slice 4 review, L2). A user this process cannot name refuses:
/// history cannot be checked, and a run that changes the machine never starts
/// without knowing who it runs as (S8 item 6; `refuse_if_root`).
pub(super) fn state_and_user(ctx: &Context) -> Result<(StateDir, u32), DomainError> {
    let state = StateDir::resolve(ctx.home.as_deref(), ctx.state_override.as_deref())?;
    let euid = effective_uid().ok_or_else(|| {
        DomainError::new(
            ErrorId::SafetyBlocked,
            "who this process runs as could not be established, so its migration history \
             cannot be checked and nothing was changed",
        )
    })?;
    Ok((state, euid))
}

/// Executes `planned` under `allowance` — **unreachable while this build grants
/// none** (`mutation_allowance` answers `None`), and through a mutation site
/// that refuses every command even if it were reached (pass 08 slice 4).
///
/// The live-scope refusal, the state directory resolved once, the machine
/// captured again and re-planned under the lock — history read again there too
/// (slice 4 review, N9) — and the configuration re-read through the registry,
/// whose re-detected prefix must derive each step's planned program (D7).
fn execute_mutating(
    ctx: &Context,
    request: &PlanRequest,
    options: &AnalysisOptions,
    planned: PlannedRun,
    allowance: MutationAllowance,
    registry: &macportsbrew_adapters::registry::RegistryContext<'_>,
    stderr: &mut dyn Write,
) -> Result<RunReport, DomainError> {
    // The live-scope refusal first, before the state directory is resolved
    // (D13; INV-15): a replay must not fail on a missing home instead.
    if ctx.history != HistoryScope::ThisMachine {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            "a run that changes the machine runs only against this machine, never a recording \
             or a snapshot file; nothing was changed",
        ));
    }
    let cleared = match clear(planned, Some(allowance)) {
        Ok(cleared) => cleared,
        // The refusals under the allowance, not a read-only re-run's (slice 4
        // review, security 5).
        Err((_, report)) => return Ok(executor::refused_report(&report, &ctx.redaction)),
    };
    // Only a plan the gate cleared is shown and confirmed (E plan, R1), and
    // before the state directory is touched.
    let shown = confirm_the_plan(ctx, &cleared, stderr)?;
    let (state, euid) = state_and_user(ctx)?;
    let ids = macportsbrew_adapters::registry::known_provider_ids();
    let recapture = Box::new(|| {
        let now = ctx.recapture()?;
        let replan =
            |consultation| migration_start(&now, &ctx.policy, options, request, &ids, consultation);
        let start = if ctx.history == HistoryScope::ThisMachine && request.retain_source {
            // Under the lock, from the state directory resolved once above.
            let view = read_view(&state, &migration_key(cleared.run().plan()), euid)?;
            replan(HistoryConsultation::Consulted {
                view: &view,
                run_stamp: &ctx.run_stamp,
            })?
        } else {
            replan(HistoryConsultation::NotConsulted)?
        };
        match start {
            MigrationStart::Run(fresh, _) => Ok((now.clone(), fresh.plan().clone())),
            _ => Err(DomainError::new(
                ErrorId::SafetyBlocked,
                "the machine changed after the plan was made, so there is no longer a plan to \
                 run; nothing was changed",
            )),
        }
    });
    let reread = |provider: &ProviderId, program: &str| {
        macportsbrew_adapters::registry::reread_configuration(registry, provider, program)
    };
    executor::run_mutating(
        &cleared,
        registry,
        executor::MutatingRun {
            live: ctx.history == HistoryScope::ThisMachine,
            state: &state,
            euid,
            start: RunStart {
                id: macportsbrew_adapters::persistence::history_store::new_run_id(&ctx.run_stamp),
                started_at: ctx.started_at.clone(),
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
            },
            site: &executor::NotInThisBuild,
            recapture,
            reread: &reread,
            read_back: &macportsbrew_adapters::persistence::history_store::read_back,
            observer: &executor::Unobserved,
            shown,
        },
        &ctx.redaction,
    )
}

/// Shows the cleared plan on standard error and asks for `run plan <code>`,
/// the interactive route of DP-049 (b); each refusal is its own error, so a
/// script tells it from the planner's (R2).
fn confirm_the_plan(
    ctx: &Context,
    cleared: &GateClear,
    stderr: &mut dyn Write,
) -> Result<ShownPlan, DomainError> {
    use std::io::IsTerminal;
    // The prompt is the caller's standard error; both terminal answers are
    // the process's own two streams, which are the ones a person reads and
    // types at (`run_inner` hands `migrate` its real standard error).
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut code = macportsbrew_adapters::entropy::fresh_bytes;
    let render = |plan: &MigrationPlan| human::plan(ctx, plan);
    let shown = crate::consent::confirm_plan(
        cleared,
        &render,
        crate::consent::ConsentStreams {
            input_is_terminal: std::io::stdin().is_terminal(),
            prompt_is_terminal: std::io::stderr().is_terminal(),
            input: &mut input,
            prompt: stderr,
            code: &mut code,
        },
        &ctx.messages,
    );
    shown.map_err(plan_question_error)
}

/// What the run loop returned, as the document carries it: no read output and
/// no raw plan text — the gate's and the site's refusals arrive cleaned.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Execution<'a> {
    outcome: &'a RunOutcome,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    steps: &'a [StepRecord],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    gate_refusals: &'a [serde_json::Value],
    #[serde(skip_serializing_if = "Option::is_none")]
    site_refusal: Option<SiteRefusal<'a>>,
    /// What went wrong with the run's history, when something did (slice 4, D8).
    #[serde(skip_serializing_if = "Option::is_none")]
    history_error: Option<ErrorId>,
    /// The run id whose interrupted-run marker was left in place (slice 4
    /// review, N10): the next run refuses until it is dealt with.
    #[serde(skip_serializing_if = "Option::is_none")]
    marker_left: Option<&'a str>,
    /// The run's id, the snapshot it took first and how its record's read-back
    /// went (pass 08 slice 5, D16): what a run that handed a mutating step over
    /// is found by afterwards. Each is left out when there is nothing to say.
    #[serde(flatten)]
    receipt: &'a RunReceipt,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteRefusal<'a> {
    step: u32,
    refusal: SiteRefusalKind,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    values: &'a BTreeMap<&'static str, String>,
}

pub(super) fn execution(report: &RunReport) -> Execution<'_> {
    Execution {
        outcome: &report.outcome,
        steps: &report.records,
        gate_refusals: &report.gate_refusals,
        site_refusal: report.refusal.as_ref().map(|r| SiteRefusal {
            step: r.step,
            refusal: r.refusal,
            values: &r.values,
        }),
        history_error: report.history_error,
        marker_left: report.marker_left.as_deref(),
        receipt: &report.receipt,
    }
}

/// Runs `migrate` and returns its exit code. Everything is decided before
/// anything is written, so a refusal leaves stdout empty.
pub fn migrate(
    ctx: &Context,
    cli: &Cli,
    request: &PlanRequest,
    options: &AnalysisOptions,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<i32, DomainError> {
    migrate_with(ctx, cli, request, options, stdout, stderr, &|key| {
        read_history(ctx, key)
    })
}

/// [`migrate`], with the history read through `history`: the store on every
/// real run, a scratch store in this module's tests — the one way the live
/// branch is exercised, since every binary-level test runs under `--replay`,
/// which never consults history (pass 08 plan, H3).
///
/// History is read only when the context's scope is
/// [`HistoryScope::ThisMachine`], the request retains the source, and the
/// planner built a plan with no `MANUAL_REVIEW` step: a plain `migrate`, a
/// refusal, a manual review and *nothing to move* never touch it, so a stray
/// file in the store blocks nothing else (pass 08 plan, L5). A dry run does
/// read it, so that what it shows is what `--yes` would do (D5, SE-092).
fn migrate_with(
    ctx: &Context,
    cli: &Cli,
    request: &PlanRequest,
    options: &AnalysisOptions,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    history: &dyn Fn(&MigrationKey) -> Result<HistoryView, DomainError>,
) -> Result<i32, DomainError> {
    let mode = Mode::of(cli);
    // Decided by where the input came from, never by looking: a live run did
    // not check, and a recording or a snapshot file could not have (INV-15).
    let concurrency = ConcurrencyFlag::of(ctx.source.concurrent_provider_activity());
    let io = |e: std::io::Error| DomainError::new(ErrorId::Io, e.to_string());
    let start = migration_start(
        &ctx.snapshot,
        &ctx.policy,
        options,
        request,
        // The planner names no provider; the registry is what knows which
        // ones a complete input must carry (SE-092, S3 addendum).
        &macportsbrew_adapters::registry::known_provider_ids(),
        HistoryConsultation::NotConsulted,
    )?;
    let start = match start {
        MigrationStart::Run(planned, _)
            if ctx.history == HistoryScope::ThisMachine
                && request.retain_source
                && !planned.plan().requires_manual_review() =>
        {
            let view = history(&migration_key(planned.plan()))?;
            // Planning is pure, so asking again with the history is the same
            // plan, now judged against it.
            migration_start(
                &ctx.snapshot,
                &ctx.policy,
                options,
                request,
                &macportsbrew_adapters::registry::known_provider_ids(),
                HistoryConsultation::Consulted {
                    view: &view,
                    run_stamp: &ctx.run_stamp,
                },
            )?
        }
        other => other,
    };
    let (planned, note) = match start {
        MigrationStart::NothingToMove {
            package,
            provider,
            not_detected,
        } => {
            if cli.json {
                let doc = Document {
                    schema_version: MIGRATE_SCHEMA_VERSION,
                    mode,
                    nothing_to_move: Some(NothingToMove {
                        package: &package,
                        provider: &provider,
                        not_detected: &not_detected,
                    }),
                    retained_run_done: None,
                    retained_run_not_recognised: None,
                    plan: None,
                    warnings: Vec::new(),
                    execution: None,
                    concurrent_provider_activity: concurrency.clone(),
                };
                writeln!(stdout, "{}", json(&doc)?).map_err(io)?;
            } else {
                write!(
                    stdout,
                    "{}",
                    human::nothing_to_move(ctx, &package, &provider, &not_detected)
                )
                .map_err(io)?;
                write!(
                    stdout,
                    "{}",
                    human::concurrency(ctx, concurrency.activity())
                )
                .map_err(io)?;
            }
            return Ok(0);
        }
        MigrationStart::RetainedRunDone {
            package,
            provider,
            run_id,
        } => {
            // Nothing is planned and nothing is written: the run that did it
            // is already in history (INV-16).
            if cli.json {
                let doc = Document {
                    schema_version: MIGRATE_SCHEMA_VERSION,
                    mode,
                    nothing_to_move: None,
                    retained_run_done: Some(RetainedRunDone {
                        package: &package,
                        provider: &provider,
                        run_id: &run_id,
                    }),
                    retained_run_not_recognised: None,
                    plan: None,
                    warnings: Vec::new(),
                    execution: None,
                    concurrent_provider_activity: concurrency.clone(),
                };
                writeln!(stdout, "{}", json(&doc)?).map_err(io)?;
            } else {
                write!(
                    stdout,
                    "{}",
                    human::retained_run_done(ctx, &package, &provider, &run_id)
                )
                .map_err(io)?;
                write!(
                    stdout,
                    "{}",
                    human::concurrency(ctx, concurrency.activity())
                )
                .map_err(io)?;
            }
            return Ok(0);
        }
        MigrationStart::Run(planned, note) => (planned, note),
    };
    let note_doc = note.as_ref().map(not_recognised);
    // DP-037: shown before anything runs, never after (security-reviewer,
    // part C): the notice is what a person reads before the stronger
    // confirmation part K will ask for.
    super::named_entry_notice(ctx, planned.plan(), stderr);
    let report = match mode {
        Mode::Render => None,
        Mode::Execute => {
            // `run_inner` refuses execute mode on a snapshot file before the
            // context exists, so a context without a registry here is a
            // defect, not a user's mistake; it still refuses rather than runs.
            let registry = ctx.execution_registry().ok_or_else(|| {
                DomainError::new(
                    ErrorId::Internal,
                    "execute mode reached a context with no registry",
                )
            })?;
            // The gate is the one producer of an allowance, and this build's
            // gives none: the read-only loop, exactly as before.
            Some(match mutation_allowance() {
                None => executor::run(&planned, &registry, &ctx.redaction),
                Some(allowance) => execute_mutating(
                    ctx,
                    request,
                    options,
                    (*planned).clone(),
                    allowance,
                    &registry,
                    stderr,
                )?,
            })
        }
    };
    let plan = planned.plan();
    if cli.json {
        let doc = Document {
            schema_version: MIGRATE_SCHEMA_VERSION,
            mode,
            nothing_to_move: None,
            retained_run_done: None,
            retained_run_not_recognised: note_doc,
            plan: Some(plan),
            warnings: super::plan_warnings(plan),
            execution: report.as_ref().map(execution),
            concurrent_provider_activity: concurrency.clone(),
        };
        writeln!(stdout, "{}", json(&doc)?).map_err(io)?;
    } else {
        if let Some(why) = &note {
            write!(stdout, "{}", human::retained_run_not_recognised(ctx, why)).map_err(io)?;
        }
        write!(stdout, "{}", human::plan(ctx, plan)).map_err(io)?;
        match &report {
            None => write!(stdout, "{}", human::render_only(ctx)).map_err(io)?,
            Some(report) => {
                write!(stdout, "{}", human::run_report(ctx, plan, report)).map_err(io)?;
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

// The live branch's tests name the providers of the recording they replay, so
// they live in their own file: this one is scanned word by word for a
// provider name (`invariant_17_the_executor_names_no_provider`).
#[cfg(test)]
#[path = "migrate_history_tests.rs"]
mod tests;
