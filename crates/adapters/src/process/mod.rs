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
    /// Fails with `ProviderUnhealthy` unless the exit code was 0. Adapters
    /// call this before parsing.
    pub fn require_success(&self) -> Result<&Invocation, DomainError> {
        if self.status.success() {
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
