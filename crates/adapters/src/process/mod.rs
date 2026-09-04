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
pub use macportsbrew_domain::MAX_PATH_LEN;

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
pub use macportsbrew_domain::is_safe_path;

/// Every program this tool may spawn, by the name at the end of the path.
///
/// This is the control, and it is an allowlist on purpose. A review defeated
/// every weaker version of it in one sitting: `Box::leak` was not the only way
/// to manufacture a `&'static str` (a module-level `OnceLock` does it with no
/// `leak`, no `unsafe`, and nothing a grep sees); the interpreter *denylist* in
/// `check-no-shell.sh` was missing `tclsh` — which is what `port` itself is
/// written in — along with `xcrun`, `arch`, `awk` and a dozen others; and the
/// literal could be split with `concat!`, escaped as `"/bin/\x73h"`, or broken
/// across lines past a line-oriented check.
///
/// None of that matters here. This runs on the `PathBuf` about to be handed to
/// the kernel, whatever produced it, and there are exactly six programs this
/// tool has any business running. A denylist of interpreters cannot be
/// completed; this list can be read in one line.
///
/// Divergence from what the adapters actually name is caught by
/// `invariant_10_every_program_an_adapter_names_is_on_the_allowlist`, not by
/// whoever edits one of the two places remembering the other.
pub(crate) const PERMITTED_PROGRAMS: &[&str] = &[
    // The two providers, resolved as `<detected prefix>/bin/<name>`.
    "brew",
    "port",
    // The four macOS probes, at absolute paths written out in `machine.rs`.
    "sw_vers",
    "uname",
    "sysctl",
    "xcode-select",
];

/// The programs the executor's own tests spawn, and only in a test build.
///
/// `a_grandchild_holding_the_pipe_does_not_hang_the_runner` spawns `/bin/sh -c`
/// for the express purpose of proving the runner survives a process that
/// outlives its parent; `timeout_kills_the_child` needs something that sleeps.
/// A test that cannot name the thing it is testing is not a test.
///
/// This extends the list rather than bypassing the check, so the tests run the
/// same code the shipped binary runs, and
/// `the_shipped_binary_may_spawn_exactly_six_programs` asserts that the
/// production list is untouched by it.
#[cfg(test)]
pub(crate) const PERMITTED_IN_TESTS: &[&str] = &[
    "sh",
    "sleep",
    "env",
    "false",
    "yes",
    "echo",
    "mpb-test-program",
];

/// What the runner asks before it spawns: the shipped rule, plus - in a test
/// build only - the handful of programs the executor's own tests name.
///
/// Kept separate from [`is_permitted_program`] so that
/// `an_interpreter_is_refused_however_it_was_named` asserts the *shipped* rule
/// rather than the widened one. A test that can only see the wider list cannot
/// tell you anything about what ships.
pub(in crate::process) fn may_spawn(program: &std::path::Path) -> bool {
    if is_permitted_program(program) {
        return true;
    }
    #[cfg(test)]
    if program.is_absolute()
        && program
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| PERMITTED_IN_TESTS.contains(&n))
    {
        return true;
    }
    false
}

/// Whether `program` is one this tool may spawn.
///
/// Absolute, no `..` anywhere in it, and a file name on
/// [`PERMITTED_PROGRAMS`]. A provider's executable must additionally sit
/// directly in a `bin` directory, which is where both providers put theirs and
/// where a prefix-shaped path can be checked to lead.
pub(crate) fn is_permitted_program(program: &std::path::Path) -> bool {
    use std::path::Component;
    if !program.is_absolute() || program.components().any(|c| c == Component::ParentDir) {
        return false;
    }
    let Some(name) = program.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !PERMITTED_PROGRAMS.contains(&name) {
        return false;
    }
    if matches!(name, "brew" | "port") {
        return program
            .parent()
            .and_then(std::path::Path::file_name)
            .is_some_and(|dir| dir == "bin");
    }
    true
}

/// What to run.
///
/// Every field is private to this module, and that is the point rather than
/// tidiness. A constructor that cannot be called from outside controls only
/// *construction*; with public fields, `spec.program = whatever` after the fact
/// reaches the same place, and so does
/// `spec.environment.insert("DYLD_INSERT_LIBRARIES", ..)` - which would undo
/// INV-10 by one line that no lint, no clippy and no test would notice. That
/// was measured, not imagined: a review built it and every gate stayed green.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    /// The program, as an absolute path resolved by the caller from a
    /// detected prefix - never a bare name looked up on `PATH` when
    /// correctness matters.
    pub(in crate::process) program: PathBuf,
    /// Arguments, one element each.
    pub(in crate::process) argv: Vec<String>,
    /// The complete environment. Nothing is inherited.
    pub(in crate::process) environment: BTreeMap<String, String>,
    /// Working directory; `None` means the process's own.
    pub(in crate::process) cwd: Option<PathBuf>,
    /// Timeout.
    pub(in crate::process) timeout: Duration,
    /// Cap per output stream.
    pub(in crate::process) max_output_bytes: usize,
}

impl CommandSpec {
    /// A provider's own executable, resolved from the prefix it was detected
    /// at.
    ///
    /// `executable` is `&'static str` on purpose: it is a literal in this
    /// source, so nothing a provider said, nothing in a snapshot and nothing
    /// off the machine can decide which program runs. The only variable part
    /// is the prefix, which is validated where a snapshot enters
    /// (`Snapshot::check`).
    pub fn provider(
        prefix: &std::path::Path,
        executable: &'static str,
        argv: Vec<String>,
        environment: BTreeMap<String, String>,
    ) -> Self {
        Self::new(prefix.join(executable), argv, environment)
    }

    /// A macOS system tool, named by a literal.
    ///
    /// Same reasoning as [`CommandSpec::provider`], with no prefix to join: the
    /// four probes this tool runs are `sw_vers`, `uname`, `sysctl` and
    /// `xcode-select`, at absolute paths written out here.
    pub fn system(
        program: &'static str,
        argv: Vec<String>,
        environment: BTreeMap<String, String>,
    ) -> Self {
        Self::new(program, argv, environment)
    }

    /// A spec with default timeout and output cap.
    ///
    /// Private to this module. Everything above it goes through
    /// [`CommandSpec::provider`] or [`CommandSpec::system`], which is what
    /// makes "no value off the machine becomes a program" a property of the
    /// type rather than of each adapter remembering. The threat model called
    /// the no-shell guarantee "enforced by a lint rule, not by discipline",
    /// and the lint enforced *where* a subprocess is spawned rather than
    /// *what* - `/bin/sh` is absolute and would have passed every gate
    /// (F-08).
    pub(in crate::process) fn new(
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

    #[test]
    fn the_shipped_binary_may_spawn_exactly_six_programs() {
        // Not a tautology: it is the assertion that makes PERMITTED_IN_TESTS
        // safe to have. If a test name ever migrates into the production list,
        // this is what says so.
        assert_eq!(
            PERMITTED_PROGRAMS,
            ["brew", "port", "sw_vers", "uname", "sysctl", "xcode-select"]
        );
        for name in PERMITTED_IN_TESTS {
            assert!(
                !PERMITTED_PROGRAMS.contains(name),
                "{name} is a test-only program and has reached the shipped list"
            );
        }
    }

    #[test]
    fn every_program_an_adapter_names_is_on_the_allowlist() {
        // The comment above PERMITTED_PROGRAMS says divergence is caught here
        // rather than by whoever edits one place remembering the other. This is
        // that check, and it is the reason the list may be written out by hand:
        // an adapter that starts naming a seventh program fails here rather
        // than at run time on a user's machine, where the symptom would be a
        // read that refuses itself.
        let named = [
            crate::homebrew::EXECUTABLE_RELATIVE,
            crate::macports::EXECUTABLE_RELATIVE,
            crate::system::machine::SW_VERS,
            crate::system::machine::UNAME,
            crate::system::machine::SYSCTL,
            crate::system::machine::XCODE_SELECT,
        ];
        for program in named {
            let leaf = std::path::Path::new(program)
                .file_name()
                .and_then(|n| n.to_str())
                .expect("a program constant names a file");
            assert!(
                PERMITTED_PROGRAMS.contains(&leaf),
                "{program} is named by an adapter and is not on the allowlist"
            );
        }
        assert_eq!(
            named.len(),
            PERMITTED_PROGRAMS.len(),
            "the allowlist and the constants the adapters name have diverged"
        );
    }

    #[test]
    fn an_interpreter_is_refused_however_it_was_named() {
        // Each of these defeated some earlier version of the control: a
        // denylist that had never heard of `tclsh` - which `port` itself is
        // written in - a lint that a `concat!` or a `\x73` escape walked
        // straight past, and a `&'static str` manufactured at run time by a
        // module-level `OnceLock`, with no `leak` and nothing to grep for. The
        // check runs on the finished path, so none of that reaches it.
        //
        // The elevating binary is not in this list, and that is not an
        // oversight: `invariant_06_reading_the_machine_needs_no_elevation`
        // refuses its name anywhere in non-comment source, and it caught this
        // test when the name was here. Rewording it to slip past would be the
        // wrong answer twice over - the invariant is right, and an allowlist
        // refuses every name not on it, so that one needs no separate case.
        for bad in [
            "/bin/sh",
            "/bin/bash",
            "/usr/bin/tclsh",
            "/usr/bin/xcrun",
            "/usr/bin/arch",
            "/usr/bin/awk",
            "/usr/bin/perl5.34",
            "/opt/homebrew/bin/bash",
        ] {
            assert!(
                !is_permitted_program(std::path::Path::new(bad)),
                "{bad} was permitted"
            );
        }
    }

    #[test]
    fn a_permitted_name_is_not_permitted_from_anywhere() {
        // The name alone is not the rule. A provider's executable comes from a
        // `bin` directory under a detected prefix, and nothing reaches the
        // kernel by way of `..`.
        for bad in [
            "/tmp/brew",
            "/usr/local/brew",
            "/opt/local/sbin/port",
            "brew",
            "/opt/homebrew/bin/../../../tmp/bin/brew",
        ] {
            assert!(
                !is_permitted_program(std::path::Path::new(bad)),
                "{bad} was permitted"
            );
        }
        for good in [
            "/opt/homebrew/bin/brew",
            "/usr/local/bin/brew",
            "/opt/local/bin/port",
            "/usr/bin/sw_vers",
            "/usr/bin/uname",
            "/usr/sbin/sysctl",
            "/usr/bin/xcode-select",
        ] {
            assert!(
                is_permitted_program(std::path::Path::new(good)),
                "{good} was refused"
            );
        }
    }

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
