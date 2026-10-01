// SPDX-License-Identifier: Apache-2.0

//! The command-line surface (`docs/spec/06-cli-and-ux.md`).
//!
//! `main.rs` parses arguments and calls [`run`]; everything else lives here
//! so the same code is reachable from tests without spawning a process.
//! JSON goes to stdout and nothing else does; notices go to stderr.

mod context;
mod human;
mod migrate;

use crate::i18n::Messages;
use clap::{Parser, Subcommand};
use context::Context;
use macportsbrew_application::{AnalysisOptions, DoctorReport, PlanRequest, Severity};
use macportsbrew_domain::{DomainError, ErrorId};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;

/// MacPortsBrew: safely migrate and manage packages between Homebrew and
/// MacPorts on macOS.
///
/// The global flags are the ones `06-cli-and-ux.md` lists; several are
/// accepted for forward compatibility and change nothing in this build.
#[derive(Parser, Debug)]
#[allow(clippy::struct_excessive_bools)]
#[command(
    name = "macportsbrew",
    version,
    about = "Safely migrate and manage packages between Homebrew and MacPorts on macOS.",
    long_about = "MacPortsBrew analyses what is installed under Homebrew and MacPorts, maps \
equivalent packages, explains which provider is the better choice for each package on this \
machine, and builds reviewable migration plans. `mpb` is the same program under a shorter name.\n\n\
This build changes nothing on the machine. `migrate --yes` reaches a refusal gate that refuses \
every plan this build produces; the read-step loop behind it exists, is tested against \
recordings, and is unreachable from this binary."
)]
pub struct Cli {
    /// Machine-readable output with a versioned schema, on stdout only.
    #[arg(long, global = true)]
    pub json: bool,
    /// Locale for human output (`en`, `qps` pseudolocale).
    #[arg(long, global = true, value_name = "TAG")]
    pub language: Option<String>,
    /// Plain output without colour.
    #[arg(long, global = true)]
    pub no_color: bool,
    /// More detail on stderr.
    #[arg(long, global = true)]
    pub verbose: bool,
    /// Do not use the network. Analysis never does in this build; accepted
    /// so scripts written now keep working.
    #[arg(long, global = true)]
    pub no_network: bool,
    /// Bypass caches. There is no cache in this build; accepted for the
    /// same reason.
    #[arg(long, global = true)]
    pub refresh: bool,
    /// Show what would be done and run nothing. Wins over --yes. Every
    /// command but `migrate --yes` is a dry run in this build anyway.
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// Confirm: `migrate` puts its plan to the execution gate instead of only
    /// showing it. The gate refuses every plan this build produces, so
    /// nothing runs; nothing in this build prompts.
    #[arg(long, global = true)]
    pub yes: bool,
    /// Analyse a captured snapshot instead of this machine. The analysis is
    /// computed as of the snapshot's capture date unless --today is given.
    #[arg(long, global = true, value_name = "FILE")]
    pub from_snapshot: Option<PathBuf>,
    /// Replay a recorded fixture as if it were this machine (testing).
    #[arg(long, global = true, value_name = "FILE", hide = true)]
    pub replay: Option<PathBuf>,
    /// The date to analyse for, YYYY-MM-DD (testing; defaults to today, or
    /// to the snapshot's capture date with --from-snapshot).
    #[arg(long, global = true, value_name = "DATE", hide = true)]
    pub today: Option<String>,
    /// Include dependency-level packages, which are managed automatically by
    /// their provider (INV-13).
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
        /// Only packages held by this provider.
        #[arg(long)]
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
        /// Destination provider id.
        #[arg(long)]
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
        /// Destination provider id.
        #[arg(long)]
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
    /// Capture both providers' state to a snapshot file.
    Snapshot {
        /// Where to write; defaults to the state directory.
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
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
            // package name), so it is escaped like any other quoted text.
            let _ = writeln!(
                stderr,
                "macportsbrew: {}",
                crate::executor::escape_controls(&messages.render_error(&e))
            );
            if cli.verbose {
                let _ = writeln!(stderr, "  {}", crate::executor::escape_controls(&e.detail));
            }
            e.exit_code()
        }
    }
}

fn run_inner(
    cli: &Cli,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<i32, DomainError> {
    // Before anything is built or printed: nothing executes against a
    // snapshot file (`04-migration-and-safety.md`, "Process execution").
    if matches!(cli.command, Some(Command::Migrate { .. }))
        && migrate::Mode::of(cli) == migrate::Mode::Execute
        && cli.from_snapshot.is_some()
    {
        return Err(migrate::refuse_execution_from_a_snapshot_file());
    }
    let ctx = Context::build(cli, stderr)?;
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
        params.insert(
            "url".to_string(),
            "https://github.com/macportsbrew/macportsbrew".to_string(),
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
            if cli.json {
                writeln!(stdout, "{}", json(&explanation)?).map_err(io)?;
            } else {
                write!(stdout, "{}", human::explanation(&ctx, &explanation)).map_err(io)?;
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
            if cli.json {
                writeln!(stdout, "{}", json(&plan)?).map_err(io)?;
            } else {
                write!(stdout, "{}", human::plan(&ctx, &plan)).map_err(io)?;
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
            let doc: DoctorReport =
                macportsbrew_application::doctor(&ctx.snapshot, &ctx.policy, &ctx.today);
            if let Some(path) = report {
                let written = context::write_diagnostic_report(&ctx, &doc, path)?;
                let _ = writeln!(
                    stderr,
                    "{}",
                    ctx.messages.render(
                        "doctor.report_written",
                        &[("path".to_string(), written.display().to_string())]
                            .into_iter()
                            .collect()
                    )
                );
            }
            if cli.json {
                writeln!(stdout, "{}", json(&doc)?).map_err(io)?;
            } else {
                write!(stdout, "{}", human::doctor(&ctx, &doc)).map_err(io)?;
            }
            Ok(match doc.worst() {
                Severity::Error => 5,
                Severity::Warning | Severity::Ok => 0,
            })
        }
        Command::Snapshot { out } => {
            let path = context::save_snapshot(&ctx, out.as_deref())?;
            let _ = writeln!(
                stderr,
                "{}",
                ctx.messages.render(
                    "snapshot.written",
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

fn json<T: serde::Serialize>(value: &T) -> Result<String, DomainError> {
    serde_json::to_string_pretty(value)
        .map_err(|e| DomainError::new(ErrorId::Internal, format!("serialising output: {e}")))
}
