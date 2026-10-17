// SPDX-License-Identifier: Apache-2.0

//! The run loop's I/O shell (DP-024, SE-092): it runs a plan's verification
//! reads and nothing else, and decides as little as sequencing allows.
//!
//! Every decision is a pure function in `macportsbrew_application::executor`;
//! this module obtains reads and hands them over. Its one caller is
//! `app/migrate.rs` (`migrate --yes`), and the execution gate refuses every
//! plan this build's planner produces, so past the gate the loop is
//! exercised by tests against recordings and synthesised reads only — it is
//! unreachable from the binary.
//!
//! **Rules for this module.** `.claude/rules/executor.md` loads them with the
//! specification chapters they come from; they are kept here as well because
//! this is where a reader of the code meets them.
//!
//! 1. **The gate first, structurally.** [`run`] takes the registry context,
//!    not read sites: it consults `execution_refusals` and builds the sites
//!    only when the gate passed the plan, so no caller holds a read site for
//!    a plan the gate refused (SE-050, "Not closed" 1). [`run_with`] takes a
//!    factory for the same reason — it is called after the gate or not at
//!    all. A word scan holds the read-site constructors to this module.
//! 2. **The guard before everything else about a step.** A step is dispatched
//!    only when `next_action` answers `Dispatch`, which it never does for a
//!    mutating step, for a read whose elevation is not `NONE`, or for any step
//!    of a plan carrying `MANUAL_REVIEW`. This module never reads the build's
//!    mutation constant or the plan's own claim about it (A11); a textual
//!    test holds it to that.
//! 3. **No spec is built here.** A read goes through [`ReadSite::read`], whose
//!    production implementation is the adapters' read sites: the program and
//!    the environment are re-derived from the provider's detected prefix, the
//!    argv is checked against the shipped read allowlist, and the read runs
//!    through the very runner the prefix was detected through, so a prefix a
//!    recording answered never reaches a live spawn (DP-023; SE-050, "Not
//!    closed" 2 and 7). Nothing in this crate constructs a command spec or
//!    calls the plan-read constructor; a textual test holds that too.
//! 4. **Step success is judged, never inferred from an exit status alone**
//!    (`04-migration-and-safety.md`): `judge_step` evaluates every read
//!    against the snapshot the plan was built from, which arrives paired with
//!    the plan (`PlannedRun`) and cannot be supplied beside it.
//! 5. **Nothing a read printed is kept, and no plan text is kept raw.** A
//!    record carries `ReadEvidence` — result, line count, truncation flag — a
//!    halt reason carries a check's kind and never its fields, and the texts
//!    this module keeps (a read site's refusal, the gate's refusals) are
//!    rendered with the home redacted and control and format characters
//!    escaped (SE-050, "Not closed" 3).
//! 6. **No provider is named** here, in code or in prose (INV-17): the loop
//!    treats both directions identically because it cannot tell them apart.
//! 7. No verb is written as a literal, and no elevation, link or launch token
//!    appears in code (the Stop gate's rules).
//!
//! **What this shell still decides** (SE-092): that a step's reads stop at the
//! first command whose read could not be obtained, and how a read-site error
//! maps onto `StepRead`. Both are sequencing; `crates/cli/tests/executor.rs`
//! tests them in place of a coverage floor.
//!
//! **Not offline.** A destination read that names a package can make one
//! provider fetch its catalogue data despite the quieting environment
//! (SE-050, "Not closed" 6). A verification read only reads, so this is a
//! network side effect and not a change to the machine; the mutating pass
//! inherits it as a recorded fact, and this build cannot reach it.

use macportsbrew_adapters::persistence::run_lock::{self, RunLock};
use macportsbrew_adapters::persistence::run_marker::{self, ClearOutcome};
use macportsbrew_adapters::persistence::{history_store, StateDir};
use macportsbrew_adapters::process::{Invocation, PlanReadRefusal};
use macportsbrew_adapters::registry::{
    plan_read_sites, PlanReadError, PlanReadSites, RegistryContext,
};
use macportsbrew_application::{
    execution_refusals, judge_mutation, judge_step, must_record, next_action, next_action_with,
    outcome_of, plan_still_holds, recheck_provider_configuration, refused, run_record, Action,
    ConfigurationRechecked, Disposition, GateClear, HaltReason, MutationResult, PlannedRun,
    Precondition, RunOutcome, StepRead, StepRecord,
};
use macportsbrew_domain::{
    redact_home, CommandEnd, DomainError, ErrorId, HomeRedaction, InterruptedRun, MigrationPlan,
    PlanCommand, ProviderConfiguration, ProviderId, RunStart, Snapshot,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Where a plan's command is read.
///
/// The production implementation is the adapters' read sites; a test double
/// answers from synthesised invocations. Either way this module never sees a
/// runner or a command spec.
pub trait ReadSite {
    /// Reads `command` from `provider`.
    fn read(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError>;
}

impl ReadSite for PlanReadSites<'_> {
    fn read(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError> {
        self.run(provider, command)
    }
}

impl<T: ReadSite + ?Sized> ReadSite for &T {
    fn read(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError> {
        (**self).read(provider, command)
    }
}

/// A read site's refusal, as it may be shown or recorded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedRefusal {
    /// The step it stopped.
    pub step: u32,
    /// Which refusal it is: what a renderer keys its sentence on.
    pub refusal: SiteRefusalKind,
    /// The values the refusal quotes, by name, each cleaned like `text`.
    pub values: BTreeMap<&'static str, String>,
    /// The refusal, home redacted and control characters escaped.
    pub text: String,
}

/// A read site's refusal, by kind and without the plan text it quotes:
/// `PlanReadRefusal`'s variants, one for one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SiteRefusalKind {
    /// The command does not name a provider.
    NotAProviderCommand,
    /// The detected prefix is not a safe path.
    UnsafePrefix,
    /// The plan's program is not the one derived from the detected prefix.
    ProgramDisagrees,
    /// The argv is not on the shipped read allowlist.
    NotAPermittedRead,
}

/// What a run produced. Holds no read output and no raw plan text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    /// How the run ended.
    pub outcome: RunOutcome,
    /// One per plan step when the gate passed the plan; empty when it refused.
    pub records: Vec<StepRecord>,
    /// The read site's refusal, when one stopped the run.
    pub refusal: Option<RenderedRefusal>,
    /// The gate's refusals in the gate's own order, each the refusal's
    /// document (its `refusal` tag and its fields) with every string in it
    /// cleaned like `refusal`'s text: a check's fields are plan text.
    pub gate_refusals: Vec<serde_json::Value>,
    /// What went wrong with the run's history, when something did (D8): `IO`
    /// when the record is not known to be written durably, `HISTORY_UNUSABLE`
    /// when the marker could not be cleared after it. Absent from the document
    /// otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_error: Option<ErrorId>,
    /// The run id whose interrupted-run marker was left in place, when one
    /// was: the next run refuses until it is dealt with (D8). Absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marker_left: Option<String>,
    /// Whether this run's record was written durably — what the human report
    /// needs to say which remedy applies. Not in the document: `historyError`
    /// already says it there.
    #[serde(skip)]
    pub record_written: bool,
}

impl RunReport {
    /// The exit code the run reports: the outcome's, and when that is 0 but
    /// the history went wrong, the history's (a 0 never hides an unwritten
    /// record or a marker left behind, D8).
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match (self.outcome.exit_code(), self.history_error) {
            (0, Some(error)) => error.exit_code(),
            (code, _) => code,
        }
    }
}

/// Runs `planned`'s verification reads through the read sites `ctx` detects —
/// built only if the execution gate passes the plan. `home` is how any text
/// the report keeps is redacted.
pub fn run(planned: &PlannedRun, ctx: &RegistryContext<'_>, home: &HomeRedaction) -> RunReport {
    run_with(planned, || plan_read_sites(ctx), home)
}

/// [`run`], with the read sites from `sites`, which is called only after the
/// gate passed the plan and never otherwise.
pub fn run_with<S: ReadSite>(
    planned: &PlannedRun,
    sites: impl FnOnce() -> S,
    home: &HomeRedaction,
) -> RunReport {
    let plan = planned.plan();
    let gate = execution_refusals(plan);
    if refused(&gate).is_some() {
        return refused_report(&gate, home);
    }
    let site = sites();
    let mut records: Vec<StepRecord> = Vec::new();
    let mut refusal = None;
    for (position, step) in plan.steps.iter().enumerate() {
        let disposition = match next_action(plan, position, &records) {
            Action::PassOver(disposition) => disposition,
            Action::Halt(reason) => Disposition::Halted {
                reason,
                evidence: Vec::new(),
                ends: Vec::new(),
            },
            // Not answered without an allowance, and this loop has none: if it
            // ever were, nothing is dispatched.
            Action::DispatchMutating => Disposition::Halted {
                reason: macportsbrew_application::HaltReason::UnexecutedMutation,
                evidence: Vec::new(),
                ends: Vec::new(),
            },
            Action::Dispatch => {
                let results = read_commands(step.provider.as_ref(), &step.commands, &site);
                if let Some(Err(PlanReadError::Refused(r))) = results.last() {
                    refusal = Some(rendered_refusal(step.index, r, home));
                }
                let reads: Vec<StepRead<'_>> = results.iter().map(step_read).collect();
                judge_step(planned, position, &reads)
            }
        };
        let halted = matches!(disposition, Disposition::Halted { .. });
        records.push(StepRecord {
            index: step.index,
            kind: step.kind,
            disposition,
        });
        if halted {
            break;
        }
    }
    for step in &plan.steps[records.len()..] {
        records.push(StepRecord {
            index: step.index,
            kind: step.kind,
            disposition: Disposition::NotReached,
        });
    }
    RunReport {
        outcome: outcome_of(plan, &records),
        records,
        refusal,
        gate_refusals: Vec::new(),
        history_error: None,
        marker_left: None,
        record_written: false,
    }
}

/// Where a mutating command would be performed (pass 08 slice 4, D7).
///
/// **The only product implementation, [`NotInThisBuild`], refuses every
/// command**, so no mutating command spec is built anywhere in shipped source
/// and the spawn site's allowlist does not grow. A test double records what it
/// is handed.
///
/// **What any implementation owes** (`plan.v1.md` obligations 9 and 10), for
/// the day one is written: the program is re-derived from the provider's
/// detected prefix, which must be the prefix the run re-planned under, and the
/// call is refused otherwise; the argv is checked against an allowlist; and
/// `PlanCommand::program` and `PlanCommand::environment` are never spawned. A
/// word scan allows exactly one implementation under `crates/*/src`.
pub trait MutationSite {
    /// Performs `command` against `provider`. Takes the configuration
    /// re-check's token **by value** and the run lock, so a command cannot be
    /// performed without a fresh re-check or outside the lock.
    ///
    /// # Errors
    /// Why the command was not performed.
    fn perform(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
        checked: ConfigurationRechecked,
        lock: &RunLock,
    ) -> Result<CommandEnd, MutationRefusal>;
}

/// Why a mutation site did not perform a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationRefusal {
    /// The site refused it.
    Refused,
    /// No invocation could be recorded.
    Transport(ErrorId),
}

/// The product mutation site: this build performs no mutating command.
pub struct NotInThisBuild;

impl MutationSite for NotInThisBuild {
    fn perform(
        &self,
        _provider: &ProviderId,
        _command: &PlanCommand,
        _checked: ConfigurationRechecked,
        _lock: &RunLock,
    ) -> Result<CommandEnd, MutationRefusal> {
        Err(MutationRefusal::Refused)
    }
}

/// A boundary of the mutating run, as an [`Observer`] is told of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    /// The history store was probed.
    Probed,
    /// The run lock was taken.
    Locked,
    /// The machine was captured again under the lock and re-planned, and the
    /// plan still holds.
    Replanned,
    /// The interrupted-run marker was looked for and none was there.
    MarkerRead,
    /// The marker was written, before the first mutating command.
    MarkerWritten,
    /// The lock was confirmed still this run's, before a mutating command.
    StillCurrent,
    /// The provider's configuration was re-read and re-checked.
    ConfigurationRechecked,
    /// A mutating command was handed to the mutation site.
    Dispatched,
    /// The run's record was written.
    RecordWritten,
    /// The marker was cleared.
    MarkerCleared,
}

/// Told of each boundary as the mutating run passes it, for a test that pins
/// the order. **Takes only `Copy` identifiers** — never the lock, the state
/// directory, a token, plan text or a path — so it can neither leak nor change
/// what the run does; failures are never injected through it. [`Unobserved`]
/// is the only product implementation, with no switch that selects another.
pub trait Observer {
    /// The run passed `boundary`, at the step numbered `step` when there is one.
    fn at(&self, boundary: Boundary, step: Option<u32>);
}

/// The product observer: nothing.
pub struct Unobserved;

impl Observer for Unobserved {
    fn at(&self, _boundary: Boundary, _step: Option<u32>) {}
}

/// The machine captured again and the plan re-built from it, under the lock.
pub type Recapture<'a> = Box<dyn FnOnce() -> Result<(Snapshot, MigrationPlan), DomainError> + 'a>;

/// A provider's configuration re-read for a step whose planned program is the
/// second argument (`registry::reread_configuration` in the product).
pub type Reread<'a> = &'a dyn Fn(&ProviderId, &str) -> Result<ProviderConfiguration, DomainError>;

/// What only the caller can supply to a mutating run.
pub struct MutatingRun<'a, M: MutationSite> {
    /// Whether this run reads the machine it is on (`HistoryScope::ThisMachine`).
    /// Anything else — a recording, a snapshot file — is refused before the
    /// state directory is touched (INV-15).
    pub live: bool,
    /// The state directory, resolved once by the caller.
    pub state: &'a StateDir,
    /// The effective user, learned once by the caller.
    pub euid: u32,
    /// The run's id and start, fixed before its first step.
    pub start: RunStart,
    /// Where mutating commands go.
    pub site: &'a M,
    /// Captures the machine again and re-plans, under the lock (D6).
    pub recapture: Recapture<'a>,
    /// Re-reads a provider's configuration before a mutating command (D7).
    pub reread: Reread<'a>,
    /// Told of each boundary.
    pub observer: &'a dyn Observer,
}

/// [`run_with_mutations`] with the read sites `ctx` detects, built only after
/// the gate cleared the run — so the caller never holds a read site.
///
/// # Errors
/// As [`run_with_mutations`].
pub fn run_mutating<M: MutationSite>(
    cleared: &GateClear,
    ctx: &RegistryContext<'_>,
    io: MutatingRun<'_, M>,
    home: &HomeRedaction,
) -> Result<RunReport, DomainError> {
    run_with_mutations(cleared, || plan_read_sites(ctx), io, home)
}

/// Runs a plan the gate cleared **under a mutation allowance** (pass 08
/// slice 4): the live-scope check; the history probe; the lock; the machine
/// captured again and re-planned under it, refusing on any difference; the
/// marker looked for; the read steps; and, before the first mutating command,
/// the marker written — then per mutating command the lock confirmed, the
/// provider's configuration re-read and re-checked, and the command handed to
/// the mutation site with the token; then the record written durably on every
/// ending and the marker cleared, or kept, by D8's rule.
///
/// Every decision is the application's: `next_action_with` answers what to do
/// with each step, `judge_mutation` and `judge_step` what came of it,
/// `plan_still_holds` whether to go on, `must_record` whether to record.
///
/// # Errors
/// A refusal before anything was dispatched: a witness without an allowance,
/// a run that is not live, the probe, the lock, the recapture, an interrupted
/// run's marker, or a marker that could not be written. Nothing mutating ran.
pub fn run_with_mutations<S: ReadSite, M: MutationSite>(
    cleared: &GateClear,
    sites: impl FnOnce() -> S,
    io: MutatingRun<'_, M>,
    home: &HomeRedaction,
) -> Result<RunReport, DomainError> {
    let Some(allowance) = cleared.allowance() else {
        return Err(DomainError::new(
            ErrorId::Internal,
            "the mutating run was entered without a mutation allowance",
        ));
    };
    if !io.live {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            "a run that changes the machine runs only against this machine, never a recording \
             or a snapshot file; nothing was changed",
        ));
    }
    let planned = cleared.run();
    let plan = planned.plan();
    let (state, euid) = (io.state, io.euid);
    history_store::probe(state, euid)?;
    io.observer.at(Boundary::Probed, None);
    let lock = run_lock::acquire(state, euid)?;
    io.observer.at(Boundary::Locked, None);
    let (now, fresh) = (io.recapture)()?;
    if let Err(fact) = plan_still_holds(plan, &fresh, planned.before(), &now) {
        return Ok(RunReport {
            outcome: RunOutcome::Halted {
                step: 0,
                reason: HaltReason::StateChanged { fact },
            },
            records: Vec::new(),
            refusal: None,
            gate_refusals: Vec::new(),
            history_error: None,
            marker_left: None,
            record_written: false,
        });
    }
    io.observer.at(Boundary::Replanned, None);
    if let Some(earlier) = run_marker::read(&lock, state, euid)? {
        return Err(DomainError::new(
            ErrorId::RunInterrupted,
            earlier.run_id().to_string(),
        ));
    }
    io.observer.at(Boundary::MarkerRead, None);
    let marker = InterruptedRun::new(&io.start.id)
        .map_err(|why| DomainError::new(ErrorId::Internal, why))?;
    let site = sites();
    let mut marker_written = false;
    let mut records: Vec<StepRecord> = Vec::new();
    let mut refusal = None;
    for (position, step) in plan.steps.iter().enumerate() {
        let disposition = match next_action_with(planned, position, &records, Some(allowance)) {
            Action::PassOver(disposition) => disposition,
            Action::Halt(reason) => Disposition::Halted {
                reason,
                evidence: Vec::new(),
                ends: Vec::new(),
            },
            Action::Dispatch => {
                let results = read_commands(step.provider.as_ref(), &step.commands, &site);
                if let Some(Err(PlanReadError::Refused(r))) = results.last() {
                    refusal = Some(rendered_refusal(step.index, r, home));
                }
                let reads: Vec<StepRead<'_>> = results.iter().map(step_read).collect();
                judge_step(planned, position, &reads)
            }
            Action::DispatchMutating => {
                if !marker_written {
                    run_marker::write(&lock, state, &marker, euid)?;
                    marker_written = true;
                    io.observer.at(Boundary::MarkerWritten, Some(step.index));
                }
                let mut results: Vec<MutationResult> = Vec::new();
                for command in &step.commands {
                    let Some(provider) = step.provider.as_ref() else {
                        results.push(MutationResult::PreconditionFailed(Precondition::NoProvider));
                        break;
                    };
                    if lock.verify_still_current().is_err() {
                        results.push(MutationResult::PreconditionFailed(Precondition::LockLost));
                        break;
                    }
                    io.observer.at(Boundary::StillCurrent, Some(step.index));
                    let Ok(configuration) = (io.reread)(provider, &command.program) else {
                        results.push(MutationResult::PreconditionFailed(
                            Precondition::ConfigurationUnreadable,
                        ));
                        break;
                    };
                    let checked =
                        recheck_provider_configuration(provider, planned.before(), &configuration);
                    let Ok(checked) = checked else {
                        results.push(MutationResult::PreconditionFailed(
                            Precondition::ConfigurationDrift,
                        ));
                        break;
                    };
                    io.observer
                        .at(Boundary::ConfigurationRechecked, Some(step.index));
                    let result = mutation_dispatch(io.site, provider, command, checked, &lock);
                    if !matches!(result, MutationResult::PreconditionFailed(_)) {
                        io.observer.at(Boundary::Dispatched, Some(step.index));
                    }
                    let stop = !matches!(result, MutationResult::Ended(end) if end.exited_zero());
                    results.push(result);
                    if stop {
                        break;
                    }
                }
                judge_mutation(plan, position, &results)
            }
        };
        let halted = matches!(disposition, Disposition::Halted { .. });
        records.push(StepRecord {
            index: step.index,
            kind: step.kind,
            disposition,
        });
        if halted {
            break;
        }
    }
    for step in &plan.steps[records.len()..] {
        records.push(StepRecord {
            index: step.index,
            kind: step.kind,
            disposition: Disposition::NotReached,
        });
    }
    let record = run_record(planned, &records, io.start.clone());
    let mut history_error = None;
    let mut marker_left = None;
    let mut record_written = false;
    let mut left = |error: ErrorId| {
        history_error = Some(error);
        marker_left = Some(marker.run_id().to_string());
    };
    if must_record(&record) {
        match history_store::write_record(state, &record, euid) {
            Ok(()) => {
                record_written = true;
                io.observer.at(Boundary::RecordWritten, None);
                match run_marker::clear(&lock, state, &marker, euid) {
                    Ok(ClearOutcome::Cleared) => io.observer.at(Boundary::MarkerCleared, None),
                    // Under the lock this run holds, the marker it wrote cannot
                    // have gone or changed hands: either is an anomaly to show.
                    Ok(ClearOutcome::AlreadyAbsent | ClearOutcome::NotOurs) | Err(_) => {
                        left(ErrorId::HistoryUnusable);
                    }
                }
            }
            // Not known to be durable — it may exist — so the marker stays and
            // the run says so; nothing offers to restore it.
            Err(_) => left(ErrorId::Io),
        }
    } else if marker_written {
        match run_marker::clear(&lock, state, &marker, euid) {
            Ok(ClearOutcome::Cleared) => io.observer.at(Boundary::MarkerCleared, None),
            Ok(ClearOutcome::AlreadyAbsent | ClearOutcome::NotOurs) | Err(_) => {
                left(ErrorId::HistoryUnusable);
            }
        }
    }
    Ok(RunReport {
        outcome: outcome_of(plan, &records),
        records,
        refusal,
        gate_refusals: Vec::new(),
        history_error,
        marker_left,
        record_written,
    })
}

/// Hands one mutating command to the mutation site with its re-check token —
/// the only place [`MutationSite::perform`] is called. A token for another
/// provider is refused here and nothing is handed over.
fn mutation_dispatch(
    site: &dyn MutationSite,
    provider: &ProviderId,
    command: &PlanCommand,
    checked: ConfigurationRechecked,
    lock: &RunLock,
) -> MutationResult {
    if checked.provider() != provider {
        return MutationResult::PreconditionFailed(Precondition::WrongProviderToken);
    }
    match site.perform(provider, command, checked, lock) {
        Ok(end) => MutationResult::Ended(end),
        Err(MutationRefusal::Refused) => MutationResult::Refused,
        Err(MutationRefusal::Transport(error)) => MutationResult::TransportFailed(error),
    }
}

/// The report of a run the gate refused, its refusals cleaned like every
/// other plan text it keeps. A report the gate passed gives a `COMPLETED`
/// outcome with no steps, which no caller asks for.
#[must_use]
pub fn refused_report(
    gate: &macportsbrew_application::GateReport,
    home: &HomeRedaction,
) -> RunReport {
    RunReport {
        outcome: refused(gate).unwrap_or(RunOutcome::Completed),
        records: Vec::new(),
        refusal: None,
        gate_refusals: gate
            .refusals()
            .iter()
            .map(|r| clean_value(refusal_document(r), home))
            .collect(),
        history_error: None,
        marker_left: None,
        record_written: false,
    }
}

/// Reads each command in order, stopping at the first that could not be read.
fn read_commands(
    provider: Option<&ProviderId>,
    commands: &[PlanCommand],
    site: &dyn ReadSite,
) -> Vec<Result<Invocation, PlanReadError>> {
    let mut results = Vec::new();
    for command in commands {
        let result = match provider {
            Some(provider) => site.read(provider, command),
            None => Err(PlanReadError::NotDetected),
        };
        let stop = result.is_err();
        results.push(result);
        if stop {
            break;
        }
    }
    results
}

fn step_read(result: &Result<Invocation, PlanReadError>) -> StepRead<'_> {
    match result {
        Ok(invocation) => StepRead::Output(invocation.read_output()),
        Err(PlanReadError::NotDetected) => StepRead::SiteUnavailable,
        Err(PlanReadError::Refused(_)) => StepRead::SiteRefused,
        Err(PlanReadError::Transport(error)) => StepRead::TransportFailed(error.id),
    }
}

/// A refusal's text with the home redacted from every value it quotes and
/// every control and format character escaped. The values are plan text,
/// verbatim.
#[must_use]
pub fn render_refusal(refusal: &PlanReadRefusal, home: &HomeRedaction) -> String {
    let clean = |value: &str| clean(value, home);
    match refusal {
        PlanReadRefusal::NotAProviderCommand => "NOT_A_PROVIDER_COMMAND".to_string(),
        PlanReadRefusal::UnsafePrefix { prefix } => {
            format!("UNSAFE_PREFIX prefix={}", clean(prefix))
        }
        PlanReadRefusal::ProgramDisagrees { planned, derived } => format!(
            "PROGRAM_DISAGREES planned={} derived={}",
            clean(planned),
            clean(derived)
        ),
        PlanReadRefusal::NotAPermittedRead { argv } => format!(
            "NOT_A_PERMITTED_READ argv=[{}]",
            argv.iter().map(|a| clean(a)).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// A refusal as the report keeps it: its stable identifier, the values it
/// quotes by name, and [`render_refusal`]'s text, every value cleaned.
fn rendered_refusal(step: u32, refusal: &PlanReadRefusal, home: &HomeRedaction) -> RenderedRefusal {
    let clean = |value: &str| clean(value, home);
    let (id, values): (SiteRefusalKind, Vec<(&'static str, String)>) = match refusal {
        PlanReadRefusal::NotAProviderCommand => (SiteRefusalKind::NotAProviderCommand, Vec::new()),
        PlanReadRefusal::UnsafePrefix { prefix } => (
            SiteRefusalKind::UnsafePrefix,
            vec![("prefix", clean(prefix))],
        ),
        PlanReadRefusal::ProgramDisagrees { planned, derived } => (
            SiteRefusalKind::ProgramDisagrees,
            vec![("planned", clean(planned)), ("derived", clean(derived))],
        ),
        PlanReadRefusal::NotAPermittedRead { argv } => (
            SiteRefusalKind::NotAPermittedRead,
            vec![(
                "argv",
                argv.iter().map(|a| clean(a)).collect::<Vec<_>>().join(" "),
            )],
        ),
    };
    RenderedRefusal {
        step,
        refusal: id,
        values: values.into_iter().collect(),
        text: render_refusal(refusal, home),
    }
}

/// Plan text as it may be kept: the home redacted, then every control and
/// format character escaped.
fn clean(value: &str, home: &HomeRedaction) -> String {
    escape_controls(&redact_home(value, home))
}

/// A gate refusal as a document. Serialising a derived enum of numbers,
/// strings and lists cannot fail; if it ever did, the refusal is kept as
/// `UNSERIALISABLE` rather than dropped or turned into `null`, so the list
/// still has one entry per refusal and says which one it could not show.
fn refusal_document(refusal: &macportsbrew_application::ExecutionRefusal) -> serde_json::Value {
    match serde_json::to_value(refusal) {
        Ok(value @ serde_json::Value::Object(_)) => value,
        _ => serde_json::json!({ "refusal": "UNSERIALISABLE" }),
    }
}

/// [`clean`], applied to every string a document holds, at any depth.
fn clean_value(value: serde_json::Value, home: &HomeRedaction) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::String(s) => Value::String(clean(&s, home)),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(|v| clean_value(v, home)).collect())
        }
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(k, v)| (k, clean_value(v, home)))
                .collect(),
        ),
        other => other,
    }
}

/// Escapes every character of every [`macportsbrew_domain::HostileClass`]:
/// controls, Unicode format characters (Cf) and the line and paragraph
/// separators, so a value cannot move a cursor, clear or break a line, start a
/// terminal sequence, or reorder what is shown around it (U+202E and the other
/// bidirectional overrides). It hides nothing as far as format characters go
/// (zero-width characters, tag characters); default-ignorable characters
/// outside Cf - Hangul fillers, variation selectors - are not covered.
///
/// The classification is the domain's, one definition shared with the snapshot
/// check (SE-092): the check refuses the control class and keeps the rest,
/// every renderer escapes all of it.
pub(crate) fn escape_controls(value: &str) -> String {
    macportsbrew_domain::escape_hostile(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_zero_never_hides_an_unwritten_record_or_a_marker_left_behind() {
        // D8: the outcome's code, unless it is 0 and the history went wrong.
        let report = |outcome, history_error| RunReport {
            outcome,
            records: Vec::new(),
            refusal: None,
            gate_refusals: Vec::new(),
            history_error,
            marker_left: None,
            record_written: false,
        };
        assert_eq!(report(RunOutcome::Completed, None).exit_code(), 0);
        for error in [ErrorId::Io, ErrorId::HistoryUnusable] {
            assert_eq!(
                report(RunOutcome::Completed, Some(error)).exit_code(),
                error.exit_code()
            );
            assert_ne!(error.exit_code(), 0);
        }
        let partial = RunOutcome::PartialSuccess {
            step: 1,
            reason: HaltReason::NotPerformedInThisBuild,
        };
        assert_eq!(report(partial, Some(ErrorId::Io)).exit_code(), 3);
    }

    #[test]
    fn a_refusal_is_rendered_with_the_home_redacted_and_controls_escaped() {
        let home = HomeRedaction::Known(PathBuf::from("/Users/ada"));
        let text = render_refusal(
            &PlanReadRefusal::ProgramDisagrees {
                planned: "/Users/ada/evil/bin/x\u{1b}[2J\n".into(),
                derived: "/opt/somewhere/bin/x".into(),
            },
            &home,
        );
        assert!(!text.contains("/Users/ada"), "{text}");
        assert!(!text.contains("ada"), "{text}");
        assert!(
            !text.chars().any(macportsbrew_domain::is_hostile),
            "{text:?}"
        );
        assert!(
            text.contains("\\u{1b}") && text.contains("\\u{a}"),
            "{text}"
        );
        let argv = render_refusal(
            &PlanReadRefusal::NotAPermittedRead {
                argv: vec!["--x=/Users/ada/f\r".into()],
            },
            &home,
        );
        assert!(
            !argv.contains("ada") && !argv.chars().any(macportsbrew_domain::is_hostile),
            "{argv:?}"
        );
        let prefix = render_refusal(
            &PlanReadRefusal::UnsafePrefix {
                prefix: "/Users/ada\t/p".into(),
            },
            &home,
        );
        assert!(
            !prefix.contains("ada") && !prefix.chars().any(macportsbrew_domain::is_hostile),
            "{prefix:?}"
        );
    }

    #[test]
    fn a_format_character_cannot_reorder_or_hide_what_is_shown() {
        // U+202E would render the rest of the line right to left, so a path
        // could display as another; the others hide or isolate text. The
        // characters are the domain's shared table, every class and every Cf
        // range's endpoints, so this test and the snapshot check's read one
        // list.
        let home = HomeRedaction::Recorded;
        let mut hostile = 0;
        for &(c, class) in macportsbrew_domain::CHARACTER_SAMPLES {
            let text = render_refusal(
                &PlanReadRefusal::UnsafePrefix {
                    prefix: format!("/opt/a{c}b"),
                },
                &home,
            );
            if class.is_some() {
                hostile += 1;
                assert!(!text.contains(c), "{c:?} survived: {text:?}");
                assert!(text.contains(&format!("{}", c.escape_unicode())), "{text}");
            } else {
                // Ordinary text, non-ASCII included, is left alone.
                assert!(
                    text.contains(&format!("/opt/a{c}b")),
                    "{c:?} was altered: {text:?}"
                );
            }
        }
        assert!(hostile > 0, "the table names hostile characters");
        // And the names a snapshot keeps are shown with their joiners escaped.
        for name in macportsbrew_domain::LEGITIMATE_TEXT_WITH_FORMAT {
            let shown = escape_controls(name);
            assert!(
                !shown.chars().any(macportsbrew_domain::is_hostile),
                "{shown:?}"
            );
        }
        assert_eq!(escape_controls("/opt/local/é"), "/opt/local/é");
    }
}
