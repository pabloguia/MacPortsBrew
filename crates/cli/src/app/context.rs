// SPDX-License-Identifier: Apache-2.0

//! Building the snapshot the commands analyse: from this machine, from a
//! replayed recording, or from a snapshot file. The one clock read of the
//! program happens here and becomes a value.

use super::Cli;
use crate::i18n::{resolve, Messages};
use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::persistence::{
    load_recording, load_snapshot, save_snapshot as persist_snapshot, snapshot_to_json,
    state_dir::StateDir, time, write_atomic,
};
use macportsbrew_adapters::process::{ProcessRunner, ReplayRunner, SystemRunner};
use macportsbrew_adapters::registry::{self, RegistryContext};
use macportsbrew_adapters::system::machine::{
    from_process_environment, language_preference, state_dir_override, SystemMachine,
};
use macportsbrew_application::{
    analyze, capture, Analysis, AnalysisOptions, AnalysisSource, CaptureContext, DoctorReport,
};
use macportsbrew_domain::{
    DomainError, ErrorId, HomeRedaction, Machine, MachineRead, ProviderId, Snapshot,
};
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
    /// The real home directory, when known. Locates the state directory; it is
    /// never written to output, which is what `redaction` is for.
    pub home: Option<PathBuf>,
    /// How home paths are redacted out of anything the user might share, and
    /// why — the two reasons a home can be absent need opposite treatment.
    pub redaction: HomeRedaction,
    /// What the capture's adapters were built from — the runner among it —
    /// kept past the capture so an execution reads through the same runner,
    /// prefix answers and home (SE-050, "Not closed" 7). `None` for a snapshot
    /// file, which has no runner and must not be executed against. Private:
    /// [`Context::execution_registry`] is the one way to it, and `migrate` is
    /// its one caller (SE-092; a word scan in `crates/cli/tests/migrate.rs`).
    registry: Option<Registry>,
    /// Whether this run may consult MacPortsBrew's migration history
    /// (pass 08 slice 1). Decided once, beside every other fact the input's
    /// origin decides, by [`HistoryScope::of`].
    pub(crate) history: HistoryScope,
    /// This run's compact UTC stamp, from the one clock read: what a history
    /// record must not be later than to be trusted for ordering.
    pub(crate) run_stamp: String,
    /// The same instant, RFC 3339 UTC: a run record's start, whose id must
    /// name it (`RunRecord::from_stored`).
    pub(crate) started_at: String,
    /// `MACPORTSBREW_STATE_DIR`, read once here beside `home`, so every use of
    /// the state directory in a run resolves it from the same values (slice 4
    /// review, L2).
    pub(crate) state_override: Option<String>,
}

/// Whether a run may read MacPortsBrew's own history.
///
/// **Only a live run does.** A recording is another machine and a snapshot
/// file is another moment; history is this machine's, now (INV-15). The
/// state directory can still be resolved under `--replay` — through
/// `MACPORTSBREW_STATE_DIR`, with no home at all — which is why the answer is
/// a field decided here rather than whatever happens to resolve: a user who
/// exports that variable must not have a replay read their real store
/// (security review, M5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryScope {
    /// History may be read for this machine.
    ThisMachine,
    /// History is not consulted.
    NotConsulted,
}

impl HistoryScope {
    /// The scope of a run built from a recording (`replay`), a snapshot file
    /// (`from_snapshot`), or neither.
    #[must_use]
    pub fn of(replay: bool, from_snapshot: bool) -> Self {
        if replay || from_snapshot {
            Self::NotConsulted
        } else {
            Self::ThisMachine
        }
    }
}

/// Everything a [`RegistryContext`] is built from, owned, so that the capture
/// and an execution build theirs through one function ([`Registry::context`])
/// from the same values rather than from two constructions that could drift.
struct Registry {
    runner: Rc<dyn ProcessRunner>,
    filesystem: FilesystemScope,
    home: Option<PathBuf>,
    machine: Machine,
    prefix_override: Option<Vec<(ProviderId, PathBuf)>>,
    /// What the machine reader was built from, kept so a capture under the run
    /// lock reads exactly as the first did (pass 08 slice 4, D6).
    path_env: Option<String>,
    variables: std::collections::BTreeMap<String, String>,
    redaction: HomeRedaction,
    replay: bool,
}

impl Registry {
    /// The machine reader, built from what the first capture's was.
    fn machine_reader(&self) -> SystemMachine {
        SystemMachine::new(
            Box::new(Rc::clone(&self.runner)),
            self.path_env.clone(),
            self.variables.clone(),
            self.redaction.clone(),
        )
    }

    /// A capture through these parts, stamped `captured_at` (never a second
    /// read of the clock, `invariant_14_the_wall_clock_is_read_in_exactly_one_place`).
    fn capture(
        &self,
        machine: &SystemMachine,
        policy: &PolicyData,
        captured_at: String,
        stderr: Option<&mut dyn Write>,
    ) -> Result<Snapshot, DomainError> {
        let providers = registry::build(&self.context(policy));
        let provider_refs: Vec<&dyn macportsbrew_domain::ProviderRead> =
            providers.iter().map(AsRef::as_ref).collect();
        if let Some(stderr) = stderr {
            let _ = writeln!(
                stderr,
                "macportsbrew: capturing {} providers",
                provider_refs.len()
            );
        }
        capture(&CaptureContext {
            machine,
            providers: provider_refs,
            policy_version: policy.version().to_string(),
            captured_at,
            captured_by: format!(
                "macportsbrew {}",
                if self.replay { "--replay" } else { "capture" }
            ),
            home: self.redaction.clone(),
            environment_contracts: registry::environment_contracts(),
        })
    }

    fn context<'a>(&'a self, policy: &'a PolicyData) -> RegistryContext<'a> {
        RegistryContext {
            runner: &*self.runner,
            home: self.home.clone(),
            platform_tags: Box::new(move |id| policy.artifact_tags_for(id, &self.machine)),
            filesystem: self.filesystem,
            prefix_override: self.prefix_override.clone(),
        }
    }
}

/// The language preference from the process environment, collapsed the way
/// `06-cli-and-ux.md` orders it.
pub fn env_language() -> Option<String> {
    let (app, lc_all, lang) = language_preference();
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
            // `load_snapshot` runs every check, schema included.
            let snapshot = load_snapshot(file)?;
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
                // A snapshot file was redacted when it was captured, so it is
                // read back on the same terms as a recording.
                redaction: HomeRedaction::Recorded,
                registry: None,
                history: HistoryScope::of(false, true),
                run_stamp: time::compact_stamp(now),
                started_at: time::rfc3339_utc(now),
                state_override: state_dir_override(),
            });
        }

        let today = cli.today.clone().unwrap_or_else(|| time::today_from(now));
        let (runner, prefix_override, replay): (Rc<dyn ProcessRunner>, _, bool) =
            if let Some(file) = &cli.replay {
                let recording = load_recording(file)?;
                let overrides = macportsbrew_adapters::registry::prefix_overrides_from(&recording);
                (Rc::new(ReplayRunner::new(recording)), Some(overrides), true)
            } else {
                (Rc::new(SystemRunner::new()), None, false)
            };
        // Why there is no home matters, so it is stated once here and carried
        // as a reason rather than as an absent value. Under replay the recording
        // was redacted when it was captured; on a live run an absent `HOME` means
        // the home is unknown, which is not the same thing and must not be
        // treated as "nothing to redact".
        let home_redaction = if replay {
            HomeRedaction::Recorded
        } else {
            HomeRedaction::live(home.clone())
        };
        let path_env = if replay { None } else { path_env };
        let variables = if replay {
            std::collections::BTreeMap::default()
        } else {
            variables
        };
        let machine = SystemMachine::new(
            Box::new(Rc::clone(&runner)),
            path_env.clone(),
            variables.clone(),
            home_redaction.clone(),
        );
        let parts = Registry {
            runner: Rc::clone(&runner),
            // Set here, next to every other decision `--replay` drives, so
            // the one place that knows the machine is a recording is the
            // one place that says so (INV-15, F-25).
            filesystem: if replay {
                FilesystemScope::Elsewhere
            } else {
                FilesystemScope::ThisMachine
            },
            home: if replay { None } else { home.clone() },
            machine: machine.machine()?,
            prefix_override,
            path_env,
            variables,
            redaction: home_redaction.clone(),
            replay,
        };
        let snapshot = parts.capture(
            &machine,
            &policy,
            time::rfc3339_utc(now),
            cli.verbose.then_some(stderr),
        )?;
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
            redaction: home_redaction,
            registry: Some(parts),
            history: HistoryScope::of(replay, false),
            run_stamp: time::compact_stamp(now),
            started_at: time::rfc3339_utc(now),
            state_override: state_dir_override(),
        })
    }

    /// The machine captured again through the same runner, prefix sources,
    /// home and environment as the first capture — what a mutating run plans
    /// against under its lock (pass 08 slice 4, D6). Stamped with the run's
    /// own start, never a second clock read. `None` for a snapshot file.
    ///
    /// # Errors
    /// The capture's own errors; `INTERNAL` for a context with no registry.
    pub(crate) fn recapture(&self) -> Result<Snapshot, DomainError> {
        let parts = self.registry.as_ref().ok_or_else(|| {
            DomainError::new(ErrorId::Internal, "a capture was asked of a snapshot file")
        })?;
        parts.capture(
            &parts.machine_reader(),
            &self.policy,
            self.started_at.clone(),
            None,
        )
    }

    /// The registry context the capture's adapters were built from, rebuilt
    /// from the same parts for an execution — the runner the snapshot was read
    /// through, the same prefix sources and the same home — or `None` when
    /// the snapshot came from a file. A prefix is re-detected from those
    /// sources, not reused from capture: one that changed in between makes a
    /// plan's program disagree with the derived one, and the read is refused
    /// as `ProgramDisagrees`. **`app/migrate.rs` is the one caller**:
    /// it hands this to the run loop, which builds read sites from it only
    /// after the execution gate passed the plan (SE-092).
    pub fn execution_registry(&self) -> Option<RegistryContext<'_>> {
        self.registry
            .as_ref()
            .map(|parts| parts.context(&self.policy))
    }

    /// Runs the analysis on the context's snapshot.
    pub fn analyze(&self, options: &AnalysisOptions) -> Analysis {
        analyze(&self.snapshot, &self.policy, options, self.source)
    }
}

/// Writes `bytes` to a file the user named. A path that already exists is
/// replaced only when it is itself a document this tool writes (a JSON object
/// carrying `schemaVersion` and `toolVersion`), so re-running a command
/// refreshes its own output while a mistyped `--out` cannot replace an
/// unrelated file in the home folder. The check and the write are not one
/// operation, which leaves a window only another process of the same user
/// could use; `write_atomic` still refuses a symbolic reference at the target.
/// A hard-link publish would close the window but cannot write to exFAT or a
/// network share.
fn write_user_named(path: &Path, bytes: &[u8]) -> Result<(), DomainError> {
    if std::fs::symlink_metadata(path).is_ok() && !is_own_document(path) {
        return Err(DomainError::new(
            ErrorId::Usage,
            format!(
                "{} already exists and is not a MacPortsBrew document; choose another name or remove it first",
                path.display()
            ),
        ));
    }
    write_atomic(path, bytes)
}

/// Whether `path` is a regular file holding a document this tool writes.
/// Bounded like every other read of a file the user can point at, and read
/// from one open handle that refuses a symlink and never blocks, so what is
/// checked is what is read.
fn is_own_document(path: &Path) -> bool {
    const LIMIT: u64 = 64 * 1024 * 1024;
    macportsbrew_adapters::persistence::read_regular_file_for_replacement(path, LIMIT)
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .is_some_and(|doc| doc.get("schemaVersion").is_some() && doc.get("toolVersion").is_some())
}

/// Saves the context's snapshot, to `out` or to the state directory.
pub fn save_snapshot(ctx: &Context, out: Option<&Path>) -> Result<PathBuf, DomainError> {
    if let Some(path) = out {
        write_user_named(path, snapshot_to_json(&ctx.snapshot)?.as_bytes())?;
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
    // Which redaction the paths in this file went through, so the reader is not
    // left to assume. Three states rather than two: `AS_CAPTURED` says the file
    // was built from a recording or a snapshot, which carries no record of how
    // it was redacted — claiming "grounded" there would be a guess, and it would
    // be wrong precisely when the snapshot was itself captured with no
    // resolvable home.
    let redaction = ctx.redaction.label();
    let report = serde_json::json!({
        "schemaVersion": doc.schema_version,
        "toolVersion": doc.tool_version,
        "policyVersion": doc.policy_version,
        "redaction": redaction,
        "machine": ctx.snapshot.machine,
        "pathEntries": ctx.snapshot.environment.path.len(),
        "providers": providers,
        "findings": doc.findings,
    });
    let text = serde_json::to_string_pretty(&report)
        .map_err(|e| DomainError::new(ErrorId::Internal, e.to_string()))?;
    write_user_named(path, text.as_bytes())?;
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use macportsbrew_adapters::process::{
        ExitStatus, Invocation, RecordedInvocation, Recording, RecordingProvenance,
    };
    use macportsbrew_adapters::registry::prefix_overrides_from;

    fn recording(answers: &[(&str, &[&str], &str)]) -> Recording {
        Recording {
            schema_version: 1,
            provenance: RecordingProvenance {
                macos: "26.0.0".into(),
                architecture: "arm64".into(),
                provider_versions: Vec::new(),
                captured_at: "2026-09-02T00:00:00Z".into(),
                captured_by: "context tests".into(),
                origin: "synthesised".into(),
                note: None,
            },
            invocations: answers
                .iter()
                .map(|(program, argv, stdout)| RecordedInvocation {
                    invocation: Invocation {
                        program: (*program).to_string(),
                        argv: argv.iter().map(|a| (*a).to_string()).collect(),
                        environment: std::collections::BTreeMap::new(),
                        status: ExitStatus::Exited { code: 0 },
                        stdout: (*stdout).to_string(),
                        stderr: String::new(),
                        truncated: false,
                        duration_ms: 1,
                    },
                })
                .collect(),
        }
    }

    #[test]
    fn only_a_live_run_consults_history() {
        assert_eq!(HistoryScope::of(false, false), HistoryScope::ThisMachine);
        assert_eq!(HistoryScope::of(true, false), HistoryScope::NotConsulted);
        assert_eq!(HistoryScope::of(false, true), HistoryScope::NotConsulted);
        assert_eq!(HistoryScope::of(true, true), HistoryScope::NotConsulted);
    }

    #[test]
    fn a_recordings_own_answers_become_the_prefixes() {
        let overrides = prefix_overrides_from(&recording(&[
            ("brew", &["--prefix"], "/opt/homebrew\n"),
            ("port", &["dir"], "/opt/local/var/macports/sources/x/y\n"),
        ]));
        assert_eq!(
            overrides,
            vec![
                (ProviderId::new("homebrew"), PathBuf::from("/opt/homebrew")),
                (ProviderId::new("macports"), PathBuf::from("/opt/local")),
            ]
        );
    }

    #[test]
    fn a_prefix_that_is_not_a_safe_path_is_dropped_rather_than_repaired() {
        // A recording is a file somebody may have been handed, and its
        // contents become the directory an executable path is built from.
        // Dropping the answer falls back to the documented candidate, which is
        // what a recording that never answered already does.
        for bad in [
            "relative/homebrew",
            "/opt/../etc",
            "/opt/home\nbrew",
            "", // `--prefix` answered with nothing
        ] {
            let overrides = prefix_overrides_from(&recording(&[("brew", &["--prefix"], bad)]));
            assert!(
                overrides.is_empty(),
                "{bad:?} must not become a prefix, got {overrides:?}"
            );
        }
    }

    #[test]
    fn an_unsuccessful_or_unrecognised_answer_yields_nothing() {
        assert!(prefix_overrides_from(&recording(&[
            ("brew", &["--cellar"], "/opt/homebrew/Cellar\n"),
            ("port", &["dir"], "/opt/local/share/nothing\n"),
            // A path component, not a substring: `macportsfoo` is not it.
            ("port", &["dir"], "/x/var/macportsfoo/y\n"),
        ]))
        .is_empty());
    }
}
