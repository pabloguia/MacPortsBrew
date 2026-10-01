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
//!    planner's own exit 4 with empty stdout.
//!
//! **INV-02, and only in this build.** The plan executed is the plan built
//! in this process, not one a human saw first; that is compatible with INV-02
//! only because nothing destructive is dispatched (`plan.v1.md`, obligation 8).
//!
//! **Rules for this file** (they are the run loop's, and the loop's word scans
//! read it): it names no provider; it never holds a read site, a runner or a
//! command spec — `executor::run` builds the sites from the context itself,
//! after the gate; and it reads neither the build's mutation constant nor the
//! plan's claim about it.

use super::context::Context;
use super::{human, json, Cli};
use crate::executor::{self, RunReport, SiteRefusalKind};
use macportsbrew_application::{
    migration_start, AnalysisOptions, MigrationStart, PlanRequest, RunOutcome, StepRecord,
};
use macportsbrew_domain::{
    ConcurrentProviderActivity, DomainError, ErrorId, MigrationPlan, ProviderId,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<&'a MigrationPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    execution: Option<Execution<'a>>,
    /// Always written: whether a provider may have been changing the machine
    /// while it was read, and never a check this build did not make
    /// (`plan.v1.md`, obligation 5).
    concurrent_provider_activity: Concurrency<'a>,
}

/// The flag `04-migration-and-safety.md` asks read-only analysis to carry:
/// the activity as its tag, plus whether that leaves the read possibly
/// inconsistent, so a consumer need not re-derive the rule.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Concurrency<'a> {
    #[serde(flatten)]
    activity: &'a ConcurrentProviderActivity,
    possibly_inconsistent: bool,
}

fn concurrency(activity: &ConcurrentProviderActivity) -> Concurrency<'_> {
    Concurrency {
        activity,
        possibly_inconsistent: activity.possibly_inconsistent(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NothingToMove<'a> {
    package: &'a str,
    provider: &'a ProviderId,
}

/// What the run loop returned, as the document carries it: no read output and
/// no raw plan text — the gate's and the site's refusals arrive cleaned.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Execution<'a> {
    outcome: &'a RunOutcome,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    steps: &'a [StepRecord],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    gate_refusals: &'a [serde_json::Value],
    #[serde(skip_serializing_if = "Option::is_none")]
    site_refusal: Option<SiteRefusal<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SiteRefusal<'a> {
    step: u32,
    refusal: SiteRefusalKind,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    values: &'a BTreeMap<&'static str, String>,
}

fn execution(report: &RunReport) -> Execution<'_> {
    Execution {
        outcome: &report.outcome,
        steps: &report.records,
        gate_refusals: &report.gate_refusals,
        site_refusal: report.refusal.as_ref().map(|r| SiteRefusal {
            step: r.step,
            refusal: r.refusal,
            values: &r.values,
        }),
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
) -> Result<i32, DomainError> {
    let mode = Mode::of(cli);
    // Decided by where the input came from, never by looking: a live run did
    // not check, and a recording or a snapshot file could not have (INV-15).
    let activity = ctx.source.concurrent_provider_activity();
    let io = |e: std::io::Error| DomainError::new(ErrorId::Io, e.to_string());
    let start = migration_start(
        &ctx.snapshot,
        &ctx.policy,
        options,
        request,
        // The planner names no provider; the registry is what knows which
        // ones a complete input must carry (SE-092, S3 addendum).
        &macportsbrew_adapters::registry::known_provider_ids(),
    )?;
    let planned = match start {
        MigrationStart::NothingToMove { package, provider } => {
            if cli.json {
                let doc = Document {
                    schema_version: MIGRATE_SCHEMA_VERSION,
                    mode,
                    nothing_to_move: Some(NothingToMove {
                        package: &package,
                        provider: &provider,
                    }),
                    plan: None,
                    execution: None,
                    concurrent_provider_activity: concurrency(&activity),
                };
                writeln!(stdout, "{}", json(&doc)?).map_err(io)?;
            } else {
                write!(
                    stdout,
                    "{}",
                    human::nothing_to_move(ctx, &package, &provider)
                )
                .map_err(io)?;
                write!(stdout, "{}", human::concurrency(ctx, &activity)).map_err(io)?;
            }
            return Ok(0);
        }
        MigrationStart::Run(planned) => planned,
    };
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
            Some(executor::run(&planned, &registry, &ctx.redaction))
        }
    };
    let plan = planned.plan();
    if cli.json {
        let doc = Document {
            schema_version: MIGRATE_SCHEMA_VERSION,
            mode,
            nothing_to_move: None,
            plan: Some(plan),
            execution: report.as_ref().map(execution),
            concurrent_provider_activity: concurrency(&activity),
        };
        writeln!(stdout, "{}", json(&doc)?).map_err(io)?;
    } else {
        write!(stdout, "{}", human::plan(ctx, plan)).map_err(io)?;
        match &report {
            None => write!(stdout, "{}", human::render_only(ctx)).map_err(io)?,
            Some(report) => {
                write!(stdout, "{}", human::run_report(ctx, plan, report)).map_err(io)?;
            }
        }
        write!(stdout, "{}", human::concurrency(ctx, &activity)).map_err(io)?;
    }
    Ok(match &report {
        Some(report) => report.exit_code(),
        // As `plan`: a plan stopping at manual review is shown, and exits 4.
        None if plan.requires_manual_review() => ErrorId::SafetyBlocked.exit_code(),
        None => 0,
    })
}
