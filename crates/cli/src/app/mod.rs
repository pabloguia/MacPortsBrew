// SPDX-License-Identifier: Apache-2.0

//! The command-line surface (`docs/spec/06-cli-and-ux.md`).
//!
//! `main.rs` parses arguments and calls [`run`]; everything else lives here
//! so the same code is reachable from tests without spawning a process.
//! JSON goes to stdout and nothing else does; notices go to stderr.

mod context;
mod history;
mod human;
mod interrupted;
mod migrate;
mod restore;
pub mod version;

use crate::i18n::Messages;
use clap::{Parser, Subcommand};
use context::Context;
use macportsbrew_application::{AnalysisOptions, DoctorReport, PlanRequest, Severity};
use macportsbrew_domain::{redact_home, ConcurrencyFlag, DomainError, ErrorId, HomeRedaction};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;

/// MacPortsBrew: see what Homebrew and MacPorts have installed, compare them
/// package by package, and plan a migration in either direction.
///
/// The global flags are the ones `06-cli-and-ux.md` lists; several are
/// accepted for forward compatibility and change nothing in this build.
#[derive(Parser, Debug)]
#[allow(clippy::struct_excessive_bools)]
#[command(
    name = "macportsbrew",
    version,
    about = "See what Homebrew and MacPorts have installed, compare them package by package, \
and plan a migration in either direction; the two are treated as peers. This version reads and \
plans; it changes nothing.",
    long_about = "MacPortsBrew analyses what is installed under Homebrew and MacPorts, maps \
equivalent packages, explains which provider is the better choice for each package on this \
machine, and builds reviewable migration plans. `mpb` is the same program under a shorter name.\n\n\
This version changes nothing on your machine: `migrate --yes` shows why each plan is refused, \
and nothing runs. A plan lists the commands a migration would run, in order, with how to check \
each one, so you can carry it out yourself."
)]
pub struct Cli {
    /// Machine-readable output with a versioned schema, on stdout only.
    #[arg(long, global = true)]
    pub json: bool,
    /// Locale for human output (`en`, `pt-BR`, `qps` pseudolocale).
    #[arg(long, global = true, value_name = "TAG")]
    pub language: Option<String>,
    /// Plain output without colour.
    #[arg(long, global = true)]
    pub no_color: bool,
    /// More detail on stderr.
    #[arg(long, global = true)]
    pub verbose: bool,
    /// Accepted so scripts keep working; this build makes no network call of
    /// its own, and it does not stop a Homebrew read fetching its catalogue
    /// on an empty cache.
    #[arg(long, global = true)]
    pub no_network: bool,
    /// Bypass caches. There is no cache in this build; accepted for the
    /// same reason.
    #[arg(long, global = true)]
    pub refresh: bool,
    /// Show what would be done and change nothing: no output file and no
    /// state is written (`snapshot` and `doctor --report` say where they would
    /// have written).
    /// Wins over --yes. Nothing in this build changes a provider anyway.
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// Confirm: `migrate` puts its plan to the execution gate instead of only
    /// showing it. The gate refuses every plan this build produces, so
    /// nothing runs; nothing in this build prompts.
    #[arg(long, global = true)]
    pub yes: bool,
    /// Analyse a captured snapshot instead of this machine, as of the date it
    /// was captured.
    #[arg(long, global = true, value_name = "FILE", conflicts_with = "replay")]
    pub from_snapshot: Option<PathBuf>,
    /// Replay a recorded fixture as if it were this machine (testing).
    #[arg(long, global = true, value_name = "FILE", hide = true)]
    pub replay: Option<PathBuf>,
    /// The date to analyse for, YYYY-MM-DD (testing; defaults to today, or
    /// to the snapshot's capture date with --from-snapshot).
    #[arg(long, global = true, value_name = "DATE", hide = true, value_parser = calendar_date)]
    pub today: Option<String>,
    /// Include dependency-level packages, which are managed automatically by
    /// their provider.
    #[arg(long, global = true)]
    pub advanced: bool,
    /// The command. Without one, `status` runs.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The commands of this build. Every one is a read but `migrate --yes`, which
/// reaches a refusal gate that refuses every plan this build produces.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Machine and provider dashboard.
    Status,
    /// Inventory, mapping, assessment and recommendations.
    Analyze {
        /// Restrict to these packages.
        packages: Vec<String>,
    },
    /// Packages, one unit per package across providers.
    List {
        /// Only packages held by this provider, by the id `status` lists.
        #[arg(long, value_parser = known_provider)]
        provider: Option<String>,
    },
    /// The full derivation of one recommendation.
    Explain {
        /// Package name, in either provider's spelling.
        package: String,
    },
    /// Build a migration plan. Nothing is executed in this build.
    Plan {
        /// Package name.
        package: String,
        /// Destination provider, by the id `status` lists it under.
        #[arg(long, value_parser = known_provider)]
        to: String,
        /// Keep the source package after migration.
        #[arg(long)]
        retain_source: bool,
    },
    /// Show a migration plan; with --yes, put it to the execution gate. The
    /// gate refuses every plan this build produces, so nothing is changed.
    Migrate {
        /// Package name.
        package: String,
        /// Destination provider, by the id `status` lists it under.
        #[arg(long, value_parser = known_provider)]
        to: String,
        /// Keep the source package after migration.
        #[arg(long)]
        retain_source: bool,
    },
    /// Structural checks: prefixes, PATH order, leaked build variables.
    Doctor {
        /// Write a redacted diagnostic file for bug reports.
        #[arg(long, value_name = "FILE")]
        report: Option<PathBuf>,
        /// Inspect binaries for cross-prefix links (not in this build).
        #[arg(long)]
        deep: bool,
    },
    /// The runs this MacPortsBrew has made, newest first; `history show <id>`
    /// shows one. Reads only, and only this machine's own state directory.
    History {
        /// What to show; without one, the list.
        #[command(subcommand)]
        action: Option<HistoryAction>,
        /// How many runs to list, at most 200.
        #[arg(long, default_value_t = history::DEFAULT_LIMIT,
              value_parser = clap::value_parser!(u16).range(1..=i64::from(history::MAX_LIMIT)))]
        limit: u16,
    },
    /// The plan that puts back what one run changed (`--plan --run <id>`).
    /// Nothing is executed in this build; without `--plan` the plan is put to
    /// the execution gate, which refuses it.
    Restore {
        /// Show the plan and run nothing.
        #[arg(long)]
        plan: bool,
        /// The run to put back, by the id `history` lists.
        #[arg(long, value_name = "ID", conflicts_with = "snapshot")]
        run: Option<String>,
        /// Reinstall what a snapshot has and the machine lacks (not in this
        /// build).
        #[arg(long, value_name = "FILE")]
        snapshot: Option<PathBuf>,
    },
    /// Capture both providers' state to a snapshot file.
    Snapshot {
        /// Where to write; defaults to the state directory.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
    },
}

/// What `history` can show.
#[derive(Subcommand, Debug)]
pub enum HistoryAction {
    /// One run: its steps, its snapshot, and whether it can be put back.
    Show {
        /// The run's id, as `history` lists it.
        id: String,
    },
    /// Deal with the interrupted-run marker a run left. Without `--yes` it
    /// shows what it knows and what it would do, and changes nothing; with
    /// `--yes` it makes the run's record durable, checks it reads back, and
    /// removes the marker.
    Acknowledge {
        /// The run the marker names.
        id: String,
    },
}

/// Runs the program and returns the exit code (`06-cli-and-ux.md`).
pub fn run(cli: &Cli, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    match run_inner(cli, stdout, stderr) {
        Ok(code) => code,
        Err(e) => {
            let messages = Messages::for_locale(&crate::i18n::resolve(
                cli.language.as_deref(),
                context::env_language().as_deref(),
            ));
            // The detail can quote the command line (a `--to` value, a
            // package name), so it is escaped like any other quoted text. It
            // can also carry a path under this host's home - a snapshot file
            // given on the command line, the state directory - and an error is
            // what a person pastes into a bug report, so both lines are
            // redacted against this host's home the way `doctor --report` is
            // (`06-cli-and-ux.md`, privacy). Always against *this* host, even
            // under replay: an error is about the run, not the recording, and
            // the analysis output INV-15 governs is never written here.
            let (_, _, home) = macportsbrew_adapters::system::machine::from_process_environment();
            let redaction = HomeRedaction::live(home);
            let shown =
                |text: &str| crate::executor::escape_controls(&redact_home(text, &redaction));
            let _ = writeln!(
                stderr,
                "macportsbrew: {}",
                shown(&messages.render_error(&e))
            );
            if cli.verbose {
                let _ = writeln!(stderr, "  {}", shown(&e.detail));
            }
            e.exit_code()
        }
    }
}

/// A provider id this build knows, for `--to` and `--provider`; anything else
/// is a usage error naming the valid ids, rather than an empty view or a
/// "not found on this machine" that reads like a fact about the machine.
/// The ids come from the registry, so this layer names no provider.
fn known_provider(value: &str) -> Result<String, String> {
    let known = macportsbrew_adapters::registry::known_provider_ids();
    if known.iter().any(|id| id.as_str() == value) {
        return Ok(value.to_string());
    }
    let names: Vec<&str> = known
        .iter()
        .map(macportsbrew_domain::ProviderId::as_str)
        .collect();
    Err(format!(
        "not a provider this build knows; use one of: {}",
        names.join(", ")
    ))
}

/// `--today`: a calendar date, `YYYY-MM-DD`. A malformed value used to switch
/// the staleness notice off silently and reach the JSON as it was typed.
fn calendar_date(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    let field = |range: std::ops::Range<usize>| value[range].parse::<u32>().unwrap_or(0);
    if shape && (1..=12).contains(&field(5..7)) && (1..=31).contains(&field(8..10)) {
        Ok(value.to_string())
    } else {
        Err("expected a date written YYYY-MM-DD".to_string())
    }
}

/// Refuses to run as root: nothing MacPortsBrew does needs it (INV-06).
/// `Ok(false)` when the user could not be learned: the check did not run, and
/// the caller says so rather than proceeding as if it had passed.
pub(crate) fn refuse_if_root(euid: Option<u32>) -> Result<bool, DomainError> {
    match euid {
        Some(0) => Err(DomainError::new(
            ErrorId::SafetyBlocked,
            "macportsbrew never runs as root; run it as your own user, without elevation"
                .to_string(),
        )),
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

fn run_inner(
    cli: &Cli,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<i32, DomainError> {
    // Never wholly root (INV-06): run elevated, the process keeps the
    // invoking user's home, so it would read providers as root and leave
    // root-owned state under that home which later runs as the user refuse.
    // An answer of `None` means the probe could not run: that is said on
    // stderr once the catalogue is loaded, and the state directory's own
    // owner check still stands behind it (a read-only run writes no state).
    let root_checked = refuse_if_root(macportsbrew_adapters::system::machine::effective_uid())?;
    // Before anything is built or printed: nothing executes against a
    // snapshot file (`04-migration-and-safety.md`, "Process execution").
    if matches!(cli.command, Some(Command::Migrate { .. }))
        && migrate::Mode::of(cli) == migrate::Mode::Execute
        && cli.from_snapshot.is_some()
    {
        return Err(migrate::refuse_execution_from_a_snapshot_file());
    }
    // `history` reads this machine's own state directory and asks no provider,
    // so it takes no capture and builds no `Context`.
    if let Some(Command::History { action, limit }) = &cli.command {
        let request = match action {
            Some(HistoryAction::Show { id }) => history::Request::Show { id },
            Some(HistoryAction::Acknowledge { id }) => history::Request::Acknowledge {
                id,
                execute: cli.yes && !cli.dry_run,
            },
            None => history::Request::List {
                limit: usize::from(*limit),
            },
        };
        return history::run(cli, &request, stdout);
    }
    // The scope and the arguments of `restore` are refused before a capture is
    // taken or a state directory is resolved.
    if let Some(Command::Restore { run, snapshot, .. }) = &cli.command {
        restore::refuse_before_context(cli, run.as_deref(), snapshot.as_deref())?;
        // A run this machine never recorded is answered from the state
        // directory, before a capture asks the providers anything.
        if let Some(run) = run.as_deref() {
            restore::refuse_unrecorded_run(run)?;
        }
    }
    let ctx = Context::build(cli, stderr)?;
    if !root_checked {
        let _ = writeln!(
            stderr,
            "{}",
            ctx.messages.render_id("privilege.user_unknown")
        );
    }
    let advanced = cli.advanced;
    if advanced {
        let _ = writeln!(stderr, "{}", ctx.messages.render_id("advanced.warning"));
    }
    let options = |only: BTreeSet<String>| AnalysisOptions {
        today: ctx.today.clone(),
        advanced,
        only,
    };
    if ctx.policy.is_stale(&ctx.today) {
        let mut params = std::collections::BTreeMap::new();
        params.insert(
            "readOn".to_string(),
            ctx.policy.newest_read_on().to_string(),
        );
        let _ = writeln!(
            stderr,
            "{}",
            ctx.messages.render("policy.stale_notice", &params)
        );
    }
    let io = |e: std::io::Error| DomainError::new(ErrorId::Io, e.to_string());

    match cli.command.as_ref().unwrap_or(&Command::Status) {
        Command::Status => {
            let analysis = ctx.analyze(&options(BTreeSet::new()));
            if cli.json {
                writeln!(stdout, "{}", json(&analysis)?).map_err(io)?;
            } else {
                write!(stdout, "{}", human::status(&ctx, &analysis)).map_err(io)?;
            }
            Ok(0)
        }
        Command::Analyze { packages } => {
            let analysis = ctx.analyze(&options(packages.iter().cloned().collect()));
            if cli.json {
                writeln!(stdout, "{}", json(&analysis)?).map_err(io)?;
            } else {
                write!(stdout, "{}", human::analysis(&ctx, &analysis, true)).map_err(io)?;
                // The analysis's own flag, the one its `--json` carries.
                let activity = analysis.concurrent_provider_activity.activity();
                write!(stdout, "{}", human::concurrency(&ctx, activity)).map_err(io)?;
            }
            Ok(0)
        }
        Command::List { provider } => {
            let mut analysis = ctx.analyze(&options(BTreeSet::new()));
            if let Some(p) = provider {
                let id = macportsbrew_domain::ProviderId::new(p);
                analysis.packages.retain(|u| u.rows.contains_key(&id));
            }
            if cli.json {
                writeln!(stdout, "{}", json(&analysis)?).map_err(io)?;
            } else {
                write!(stdout, "{}", human::analysis(&ctx, &analysis, false)).map_err(io)?;
            }
            Ok(0)
        }
        Command::Explain { package } => {
            let explanation = macportsbrew_application::explain_package(
                &ctx.snapshot,
                &ctx.policy,
                &options(BTreeSet::new()),
                package,
            )?;
            let concurrency = ConcurrencyFlag::of(ctx.source.concurrent_provider_activity());
            if cli.json {
                let document = Flagged {
                    body: &explanation,
                    concurrent_provider_activity: concurrency,
                };
                writeln!(stdout, "{}", json(&document)?).map_err(io)?;
            } else {
                write!(stdout, "{}", human::explanation(&ctx, &explanation)).map_err(io)?;
                write!(
                    stdout,
                    "{}",
                    human::concurrency(&ctx, concurrency.activity())
                )
                .map_err(io)?;
            }
            Ok(0)
        }
        Command::Plan {
            package,
            to,
            retain_source,
        } => {
            let plan = macportsbrew_application::build_plan(
                &ctx.snapshot,
                &ctx.policy,
                &options(BTreeSet::new()),
                &PlanRequest {
                    package: package.clone(),
                    to: macportsbrew_domain::ProviderId::new(to),
                    retain_source: *retain_source,
                },
                // The planner names no provider; the registry is what knows
                // which ones a complete input must carry.
                &macportsbrew_adapters::registry::known_provider_ids(),
            )?;
            let concurrency = ConcurrencyFlag::of(ctx.source.concurrent_provider_activity());
            if cli.json {
                let document = Flagged {
                    body: &plan,
                    concurrent_provider_activity: concurrency,
                };
                writeln!(stdout, "{}", json(&document)?).map_err(io)?;
            } else {
                write!(stdout, "{}", human::plan(&ctx, &plan)).map_err(io)?;
                write!(
                    stdout,
                    "{}",
                    human::concurrency(&ctx, concurrency.activity())
                )
                .map_err(io)?;
            }
            // A plan that stops at manual review is reported with the
            // safety-check code so scripts can tell (06-cli-and-ux.md).
            Ok(if plan.requires_manual_review() { 4 } else { 0 })
        }
        Command::Migrate {
            package,
            to,
            retain_source,
        } => migrate::migrate(
            &ctx,
            cli,
            &PlanRequest {
                package: package.clone(),
                to: macportsbrew_domain::ProviderId::new(to),
                retain_source: *retain_source,
            },
            &options(BTreeSet::new()),
            stdout,
        ),
        Command::Doctor { report, deep } => {
            if *deep && !cli.json {
                let _ = writeln!(
                    stderr,
                    "{}",
                    ctx.messages.render_id("doctor.deep_not_in_this_build")
                );
            }
            // The state directory is looked at here, by the shell, and handed in:
            // `doctor` itself stays a pure function of the snapshot and the policy.
            let doc: DoctorReport =
                macportsbrew_application::doctor(&ctx.snapshot, &ctx.policy, &ctx.today)
                    .with_run_finding(&interrupted::gather(&ctx));
            if let Some(path) = report {
                // `--dry-run` writes nothing, a report file included (pass 14
                // security review, M1).
                let (key, shown) = if cli.dry_run {
                    context::refuse_unrelated_file(path)?;
                    ("doctor.report_would_write", path.clone())
                } else {
                    (
                        "doctor.report_written",
                        context::write_diagnostic_report(&ctx, &doc, path)?,
                    )
                };
                let _ = writeln!(
                    stderr,
                    "{}",
                    ctx.messages.render(
                        key,
                        &[("path".to_string(), shown.display().to_string())]
                            .into_iter()
                            .collect()
                    )
                );
            }
            let concurrency = ConcurrencyFlag::of(ctx.source.concurrent_provider_activity());
            if cli.json {
                let document = Flagged {
                    body: &doc,
                    concurrent_provider_activity: concurrency,
                };
                writeln!(stdout, "{}", json(&document)?).map_err(io)?;
            } else {
                write!(stdout, "{}", human::doctor(&ctx, &doc)).map_err(io)?;
                write!(
                    stdout,
                    "{}",
                    human::concurrency(&ctx, concurrency.activity())
                )
                .map_err(io)?;
            }
            Ok(match doc.worst() {
                Severity::Error => 5,
                Severity::Warning | Severity::Ok => 0,
            })
        }
        Command::History { .. } => unreachable!("`history` returns before a context is built"),
        Command::Restore { plan, run, .. } => restore::restore(
            &ctx,
            cli,
            *plan,
            run.as_deref()
                .ok_or_else(|| DomainError::new(ErrorId::Usage, "restore needs --run <id>"))?,
            &options(BTreeSet::new()),
            stdout,
        ),
        Command::Snapshot { out } => {
            let (key, path) = match context::save_snapshot(&ctx, out.as_deref(), cli.dry_run)? {
                context::Saved::Written(path) => ("snapshot.written", path),
                context::Saved::WouldWrite(path) => ("snapshot.would_write", path),
            };
            let _ = writeln!(
                stderr,
                "{}",
                ctx.messages.render(
                    key,
                    &[("path".to_string(), path.display().to_string())]
                        .into_iter()
                        .collect()
                )
            );
            if cli.json {
                writeln!(stdout, "{}", json(&ctx.snapshot)?).map_err(io)?;
            }
            Ok(0)
        }
    }
}

/// What `plan`, `explain` and `doctor` write with `--json`: the payload
/// exactly as its schema documents it, then the concurrency flag at the same
/// root (`plan.v1.md`, obligation 5; SE-015).
///
/// The payload types are not given the field: `MigrationPlan` is the contract
/// the planner, the execution gate and the run loop share, and `migrate`
/// carries it under its own `plan` key with the flag at *its* root; the
/// explanation and the doctor report are the application's answers, and where
/// the input came from is the command's to say. The payload is flattened
/// first, so every byte it wrote before this field keeps its place and the
/// flag is appended after it.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Flagged<'a, T: serde::Serialize> {
    #[serde(flatten)]
    body: &'a T,
    concurrent_provider_activity: ConcurrencyFlag,
}

fn json<T: serde::Serialize>(value: &T) -> Result<String, DomainError> {
    serde_json::to_string_pretty(value)
        .map_err(|e| DomainError::new(ErrorId::Internal, format!("serialising output: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariant_06_the_tool_refuses_to_run_wholly_as_root() {
        let refused = refuse_if_root(Some(0)).unwrap_err();
        assert_eq!(refused.id, ErrorId::SafetyBlocked);
        assert!(
            refused.detail.contains("never runs as root"),
            "{}",
            refused.detail
        );
        assert!(refuse_if_root(Some(501)).expect("an ordinary user runs"));
        // A user that could not be learned is "not checked", never "passed".
        assert!(!refuse_if_root(None).expect("not a refusal"));
    }

    #[test]
    fn invariant_06_history_restore_and_acknowledge_refuse_root() {
        // `history` skips the capture, so it must not skip the refusal: both
        // commands read a state directory a root-run process would read as root.
        let source = include_str!("mod.rs");
        let source = source.split("#[cfg(test)]").next().unwrap();
        let body = &source[source.find("fn run_inner(").expect("run_inner")..];
        let root = body.find("refuse_if_root(").expect("the root refusal");
        let history = body.find("history::run(").expect("the history command");
        assert!(
            root < history,
            "history would be read before the root check"
        );
        // `restore` comes after it too: the refusal precedes the restore
        // arguments and any context.
        let restore = body
            .find("restore::refuse_before_context(")
            .expect("restore");
        let context = body.find("Context::build(").expect("the context");
        assert!(
            root < restore && root < context,
            "a command would run before the root check"
        );
        // An unrecorded run is answered before the capture, after the arguments.
        let unrecorded = body
            .find("restore::refuse_unrecorded_run(")
            .expect("the unrecorded-run refusal");
        assert!(
            restore < unrecorded && unrecorded < context,
            "restore must look the run up before it builds a context"
        );
    }

    #[test]
    fn invariant_06_a_root_check_that_did_not_run_is_said() {
        // The notice the caller prints when the user could not be learned.
        let messages = crate::i18n::Messages::for_locale(&crate::i18n::resolve(Some("en"), None));
        let text = messages.render_id("privilege.user_unknown");
        assert!(text.contains("could not tell"), "{text}");
    }
}
