// SPDX-License-Identifier: Apache-2.0

//! Building the snapshot the commands analyse: from this machine, from a
//! replayed recording, or from a snapshot file. The one clock read of the
//! program happens here and becomes a value.

use super::Cli;
use crate::i18n::{resolve, Messages};
use macportsbrew_adapters::persistence::{
    load_recording, load_snapshot, save_snapshot as persist_snapshot, snapshot_to_json,
    state_dir::StateDir, time, write_atomic,
};
use macportsbrew_adapters::process::{ProcessRunner, ReplayRunner, SystemRunner};
use macportsbrew_adapters::registry::{self, RegistryContext};
use macportsbrew_adapters::system::machine::{
    from_process_environment, state_dir_override, SystemMachine,
};
use macportsbrew_application::{
    analyze, capture, Analysis, AnalysisOptions, AnalysisSource, CaptureContext, DoctorReport,
};
use macportsbrew_domain::{DomainError, ErrorId, MachineRead, ProviderId, Snapshot};
use macportsbrew_policy::PolicyData;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// What every command works from.
pub struct Context {
    /// The snapshot being analysed.
    pub snapshot: Snapshot,
    /// Policy data.
    pub policy: PolicyData,
    /// Messages in the resolved locale.
    pub messages: Messages,
    /// The date the analysis is computed for.
    pub today: String,
    /// Live capture, replayed recording, or a snapshot file.
    pub source: AnalysisSource,
    /// Home directory, when known (never written to output).
    pub home: Option<PathBuf>,
}

/// The language preference from the process environment, collapsed the way
/// `06-cli-and-ux.md` orders it.
pub fn env_language() -> Option<String> {
    let app = std::env::var("MACPORTSBREW_LANG").ok();
    let lc_all = std::env::var("LC_ALL").ok();
    let lang = std::env::var("LANG").ok();
    crate::i18n::env_preference(app.as_deref(), lc_all.as_deref(), lang.as_deref())
        .map(str::to_string)
}

impl Context {
    /// Builds the context per the flags.
    pub fn build(cli: &Cli, stderr: &mut dyn Write) -> Result<Self, DomainError> {
        let policy = PolicyData::load_embedded()?;
        let messages =
            Messages::for_locale(&resolve(cli.language.as_deref(), env_language().as_deref()));
        let now = time::now_secs();
        let (path_env, variables, home) = from_process_environment();

        if let Some(file) = &cli.from_snapshot {
            let snapshot = load_snapshot(file)?;
            snapshot.check_schema()?;
            let today = cli
                .today
                .clone()
                .unwrap_or_else(|| snapshot.provenance.captured_at.chars().take(10).collect());
            return Ok(Self {
                snapshot,
                policy,
                messages,
                today,
                source: AnalysisSource::Snapshot,
                home,
            });
        }

        let today = cli.today.clone().unwrap_or_else(|| time::today_from(now));
        let (runner, prefix_override, replay): (Rc<dyn ProcessRunner>, _, bool) =
            if let Some(file) = &cli.replay {
                let recording = load_recording(file)?;
                let overrides = prefix_overrides_from(&recording);
                (Rc::new(ReplayRunner::new(recording)), Some(overrides), true)
            } else {
                (Rc::new(SystemRunner::new()), None, false)
            };
        let machine = SystemMachine::new(
            Box::new(Rc::clone(&runner)),
            if replay { None } else { path_env },
            if replay {
                std::collections::BTreeMap::default()
            } else {
                variables
            },
            if replay { None } else { home.clone() },
        );
        let machine_facts = machine.machine()?;
        let snapshot = {
            let policy_ref = &policy;
            let providers = registry::build(&RegistryContext {
                runner: &*runner,
                home: if replay { None } else { home.clone() },
                platform_tags: Box::new(move |id| policy_ref.artifact_tags_for(id, &machine_facts)),
                prefix_override,
            });
            let provider_refs: Vec<&dyn macportsbrew_domain::ProviderRead> =
                providers.iter().map(AsRef::as_ref).collect();
            if cli.verbose {
                let _ = writeln!(
                    stderr,
                    "macportsbrew: capturing {} providers",
                    provider_refs.len()
                );
            }
            capture(&CaptureContext {
                machine: &machine,
                providers: provider_refs,
                policy_version: policy.version().to_string(),
                captured_at: time::rfc3339_utc(now),
                captured_by: format!(
                    "macportsbrew {}",
                    if replay { "--replay" } else { "capture" }
                ),
            })?
        };
        Ok(Self {
            snapshot,
            policy,
            messages,
            today,
            source: if replay {
                AnalysisSource::Snapshot
            } else {
                AnalysisSource::Live
            },
            home,
        })
    }

    /// Runs the analysis on the context's snapshot.
    pub fn analyze(&self, options: &AnalysisOptions) -> Analysis {
        analyze(&self.snapshot, &self.policy, options, self.source)
    }
}

/// Under replay, the prefixes come from the recording's own `--prefix` and
/// `dir` answers rather than from this machine's filesystem.
fn prefix_overrides_from(
    recording: &macportsbrew_adapters::process::Recording,
) -> Vec<(ProviderId, PathBuf)> {
    let mut out = Vec::new();
    for inv in recording.invocations.iter().map(|r| &r.invocation) {
        if inv.program == "brew" && inv.argv == ["--prefix"] && inv.status.success() {
            out.push((
                ProviderId::new("homebrew"),
                PathBuf::from(inv.stdout.trim()),
            ));
        }
        if inv.program == "port" && inv.argv == ["dir"] && inv.status.success() {
            // `port dir` prints the port directory, not the prefix; the
            // prefix is its ancestor above `var/macports/...`. Fall back to
            // the default when it does not parse.
            let text = inv.stdout.trim();
            if let Some(idx) = text.find("/var/macports") {
                out.push((ProviderId::new("macports"), PathBuf::from(&text[..idx])));
            }
        }
    }
    out
}

/// Saves the context's snapshot, to `out` or to the state directory.
pub fn save_snapshot(ctx: &Context, out: Option<&Path>) -> Result<PathBuf, DomainError> {
    if let Some(path) = out {
        write_atomic(path, snapshot_to_json(&ctx.snapshot)?.as_bytes())?;
        return Ok(path.to_path_buf());
    }
    let dir = StateDir::resolve(ctx.home.as_deref(), state_dir_override().as_deref())?;
    dir.ensure()?;
    persist_snapshot(&dir.snapshots(), &ctx.snapshot)
}

/// Writes the redacted diagnostic file (`06-cli-and-ux.md`, "Privacy and
/// diagnostics"): versions, architecture, macOS version, provider prefixes,
/// package names, findings. Never environment values, home paths or user
/// names.
pub fn write_diagnostic_report(
    ctx: &Context,
    doc: &DoctorReport,
    path: &Path,
) -> Result<PathBuf, DomainError> {
    let providers: serde_json::Map<String, serde_json::Value> = ctx
        .snapshot
        .providers
        .iter()
        .map(|(id, p)| {
            (
                id.to_string(),
                serde_json::json!({
                    "presence": p.detection.presence,
                    "version": p.detection.version,
                    "prefix": p.detection.prefix,
                    "packages": p.inventory.installed_names(),
                }),
            )
        })
        .collect();
    let report = serde_json::json!({
        "schemaVersion": doc.schema_version,
        "toolVersion": doc.tool_version,
        "policyVersion": doc.policy_version,
        "machine": ctx.snapshot.machine,
        "pathEntries": ctx.snapshot.environment.path.len(),
        "providers": providers,
        "findings": doc.findings,
    });
    let text = serde_json::to_string_pretty(&report)
        .map_err(|e| DomainError::new(ErrorId::Internal, e.to_string()))?;
    write_atomic(path, text.as_bytes())?;
    Ok(path.to_path_buf())
}
