// SPDX-License-Identifier: Apache-2.0

//! The central process executor.
//!
//! Argv arrays only - there is no shell to inject into. Every invocation
//! runs in an environment built from scratch (INV-10), has a timeout, can be
//! cancelled, and is recorded in full: argv, environment, exit status,
//! stdout, stderr, duration. A [`Recording`] of invocations is what a
//! fixture is; the [`ReplayRunner`] serves one back so adapters can be
//! tested against real provider output without a real provider.

mod environment;
mod recording;
mod system;

pub use environment::{sanitised_environment, EnvironmentSpec};
pub use recording::{RecordedInvocation, Recording, RecordingProvenance, ReplayRunner};
pub use system::{CancelToken, SystemRunner};

use macportsbrew_domain::DomainError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

/// Default per-invocation timeout. Provider commands that read an index
/// finish in seconds; anything longer is a sign of the auto-update or the
/// network path the specification forbids during analysis.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Default cap on captured output per stream. A package inventory is at most
/// a few megabytes; an unbounded read is a threat-model item.
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

/// Cap on the length of a path this workspace will hand to a provider, or
/// accept from one as a prefix.
pub const MAX_PATH_LEN: usize = 4096;

/// True when `path` is one that may be handed to a provider as an argument, or
/// trusted as a prefix to build an executable path from.
///
/// Absolute, bounded, free of control characters and of `..` components. The
/// executor takes argv arrays, so quoting is not the concern; a relative or
/// traversing path reaching a provider query, or becoming the directory an
/// executable is resolved from, is.
///
/// It lives beside the executor because every one of those uses ends at a
/// process this module spawns, and because the two callers that need it — the
/// MacPorts adapter, for a path it passes to `port`, and the command line, for
/// a prefix read out of a recording — are in different layers and must not
/// each carry their own idea of "safe".
pub fn is_safe_path(path: &std::path::Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    if text.len() > MAX_PATH_LEN || !text.starts_with('/') {
        return false;
    }
    if text.chars().any(char::is_control) {
        return false;
    }
    !path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

/// What to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    /// The program, as an absolute path resolved by the caller from a
    /// detected prefix - never a bare name looked up on `PATH` when
    /// correctness matters.
    pub program: PathBuf,
    /// Arguments, one element each.
    pub argv: Vec<String>,
    /// The complete environment. Nothing is inherited.
    pub environment: BTreeMap<String, String>,
    /// Working directory; `None` means the process's own.
    pub cwd: Option<PathBuf>,
    /// Timeout.
    pub timeout: Duration,
    /// Cap per output stream.
    pub max_output_bytes: usize,
}

impl CommandSpec {
    /// A spec with default timeout and output cap.
    pub fn new(
        program: impl Into<PathBuf>,
        argv: Vec<String>,
        environment: BTreeMap<String, String>,
    ) -> Self {
        Self {
            program: program.into(),
            argv,
            environment,
            cwd: None,
            timeout: DEFAULT_TIMEOUT,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }

    /// Sets the timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// The program's file name, which is what a recording is keyed on
    /// (the prefix differs between machines).
    pub fn program_name(&self) -> String {
        self.program
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// How an invocation ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "status")]
pub enum ExitStatus {
    /// Exited with a code.
    Exited {
        /// The code.
        code: i32,
    },
    /// Killed by a signal.
    Signalled {
        /// The signal number, when known.
        signal: Option<i32>,
    },
    /// Killed by the executor after the timeout.
    TimedOut,
    /// The program could not be started (not found, not executable).
    NotStarted {
        /// The OS error text.
        error: String,
    },
    /// Cancelled through a [`CancelToken`].
    Cancelled,
}

impl ExitStatus {
    /// True for exit code 0.
    pub fn success(&self) -> bool {
        matches!(self, ExitStatus::Exited { code: 0 })
    }
}

/// The full record of one invocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invocation {
    /// Program file name (not the full path: recordings move between
    /// machines).
    pub program: String,
    /// Arguments.
    pub argv: Vec<String>,
    /// The environment that was set. Values are recorded because they are
    /// constructed by this executor and contain nothing secret (INV-10).
    pub environment: BTreeMap<String, String>,
    /// Outcome.
    pub status: ExitStatus,
    /// Captured stdout, UTF-8 with replacement, possibly truncated.
    pub stdout: String,
    /// Captured stderr, likewise.
    pub stderr: String,
    /// True when either stream hit the cap.
    #[serde(default)]
    pub truncated: bool,
    /// Wall-clock duration.
    pub duration_ms: u64,
}

impl Invocation {
    /// Fails with `ProviderUnhealthy` unless the exit code was 0, and with
    /// `ProviderOutputChanged` when the output was cut off at the cap.
    /// Adapters call this before parsing.
    ///
    /// Truncation is a failure and not a warning. Every parser above this
    /// point reads the captured text as the provider's complete answer, and
    /// a truncated stream is a *plausible* one: a JSON document cut at 64 MiB
    /// fails to parse, but a line-oriented inventory cut mid-stream parses
    /// perfectly into an inventory that is silently missing packages, and an
    /// analysis of a machine whose packages are half missing is worse than no
    /// analysis. `04-migration-and-safety.md` ("Malicious or malformed
    /// provider metadata") is the same rule: output that is not what the
    /// provider would have said is refused, not interpreted.
    pub fn require_success(&self) -> Result<&Invocation, DomainError> {
        if self.status.success() {
            if self.truncated {
                return Err(DomainError::new(
                    macportsbrew_domain::ErrorId::ProviderOutputChanged,
                    format!(
                        "{} {}: provider output was truncated at {} bytes",
                        self.program,
                        self.argv.join(" "),
                        self.stdout.len().max(self.stderr.len())
                    ),
                ));
            }
            return Ok(self);
        }
        let id = match self.status {
            ExitStatus::TimedOut => macportsbrew_domain::ErrorId::ProviderTimeout,
            ExitStatus::NotStarted { .. } => macportsbrew_domain::ErrorId::ProviderNotFound,
            _ => macportsbrew_domain::ErrorId::ProviderUnhealthy,
        };
        Err(DomainError::new(
            id,
            format!(
                "{} {} ended with {:?}: {}",
                self.program,
                self.argv.join(" "),
                self.status,
                self.stderr.trim().chars().take(400).collect::<String>()
            ),
        ))
    }
}

/// The one way to run a subprocess.
pub trait ProcessRunner {
    /// Runs the command to completion (or timeout) and returns the record.
    /// Only a failure to *record* is an `Err`; a non-zero exit is a
    /// successful `Ok(Invocation)` with its status.
    fn run(&self, spec: &CommandSpec) -> Result<Invocation, DomainError>;

    /// Whether this runner is a replay of recorded output rather than the
    /// live machine, so analysis output can be labelled.
    fn is_replay(&self) -> bool {
        false
    }
}

impl<T: ProcessRunner + ?Sized> ProcessRunner for &T {
    fn run(&self, spec: &CommandSpec) -> Result<Invocation, DomainError> {
        (**self).run(spec)
    }

    fn is_replay(&self) -> bool {
        (**self).is_replay()
    }
}

impl<T: ProcessRunner + ?Sized> ProcessRunner for std::rc::Rc<T> {
    fn run(&self, spec: &CommandSpec) -> Result<Invocation, DomainError> {
        (**self).run(spec)
    }

    fn is_replay(&self) -> bool {
        (**self).is_replay()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macportsbrew_domain::ErrorId;

    fn invocation(truncated: bool, code: i32) -> Invocation {
        Invocation {
            program: "brew".into(),
            argv: vec!["list".into()],
            environment: BTreeMap::new(),
            status: ExitStatus::Exited { code },
            stdout: "aaaa".into(),
            stderr: String::new(),
            truncated,
            duration_ms: 1,
        }
    }

    #[test]
    fn a_truncated_stream_is_a_failure_even_when_the_command_succeeded() {
        // The dangerous case, and the reason this is an error rather than a
        // flag on the record: exit code 0, plausible-looking output, and a
        // parser above that would read it as the complete inventory.
        let error = invocation(true, 0)
            .require_success()
            .expect_err("truncated output is not a usable answer");
        assert_eq!(error.id, ErrorId::ProviderOutputChanged);
        assert!(
            error.detail.contains("truncated at 4 bytes"),
            "the detail must say how much was captured: {}",
            error.detail
        );
    }

    #[test]
    fn an_untruncated_success_still_passes() {
        assert!(invocation(false, 0).require_success().is_ok());
    }

    #[test]
    fn a_failed_command_keeps_reporting_the_failure_it_had() {
        // Truncation does not mask the exit status: a command that failed
        // failed, and `ProviderUnhealthy` is the more useful diagnosis.
        let error = invocation(true, 1).require_success().unwrap_err();
        assert_eq!(error.id, ErrorId::ProviderUnhealthy);
    }

    #[test]
    fn a_path_is_safe_only_when_it_is_absolute_bounded_and_plain() {
        use std::path::Path;
        assert!(is_safe_path(Path::new("/opt/local")));
        assert!(!is_safe_path(Path::new("opt/local")));
        assert!(!is_safe_path(Path::new("/opt/../etc/passwd")));
        assert!(!is_safe_path(Path::new("/opt/lo\ncal")));
        assert!(!is_safe_path(Path::new(&format!(
            "/{}",
            "a".repeat(MAX_PATH_LEN)
        ))));
    }
}
