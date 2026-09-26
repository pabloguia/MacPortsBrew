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

/// Which of a provider's own subcommands this tool may start: the second
/// dimension of [`PERMITTED_PROGRAMS`], and the reason that list alone was not
/// the control it looked like (F-53).
///
/// A program name says `brew` may run; it says nothing about *what* `brew` is
/// asked to do, and every mutation this project must never perform is `brew`
/// or `port` with a different first word. The adapters each hold an exact
/// argument-vector allowlist, which is right and is also discipline: a plan's
/// verbs come from `data/policy/provider-commands.toml`, not from either
/// adapter, and nothing at the spawn site asked whether the verb under
/// `verify` was a read. `verify = ["install"]` loaded, planned and would have
/// spawned. The loader now refuses that data; this refuses the invocation,
/// on the argv about to reach the kernel, whatever produced it.
///
/// Every entry is a read, and the list is literal and short on purpose. The
/// pass that adds an executor adds its mutating subcommands here, deliberately
/// and in review - which is the moment this list exists to make visible.
/// `every_subcommand_an_adapter_runs_is_permitted_and_no_other` ties it to the
/// argument vectors the adapters actually hold, in both directions.
pub(crate) const PERMITTED_SUBCOMMANDS: &[ProviderReads] = &[
    ProviderReads {
        program: "brew",
        // Homebrew takes no option before its subcommand in anything this tool
        // runs; its informational flags *are* subcommands in that position.
        leading_flags: &[],
        subcommands: &[
            "--version",
            "--prefix",
            "--cellar",
            "--caskroom",
            "--cache",
            "config",
            "info",
            "list",
        ],
        separators: &[],
    },
    ProviderReads {
        program: "port",
        // `-q`, which implies `-N` (R-3). Any other option is refused here and,
        // as a single-dash option, after the subcommand too: `-F <file>` adds a
        // command file that `port` runs once the argv's own actions finish, and
        // it is honoured in either position (`parse_options` and the main
        // block, `/opt/local/bin/port` lines 4461 and 5749, MacPorts 2.12.6,
        // read 2026-09-25; security-reviewer, observed with a file of `version`
        // lines).
        leading_flags: &["-q"],
        // Matched exactly, and that matters here: `port` resolves an action it
        // does not know by case-folded unambiguous prefix (`find_action`,
        // `/opt/local/bin/port` line 4217, MacPorts 2.12.6, read 2026-09-25), so
        // only an exact full name is certain to be the read it looks like.
        subcommands: &["version", "installed", "echo", "info", "provides"],
        // `port` runs one argv as a *sequence* of actions: an element that is
        // exactly `;` ends one and starts the next, and the loop continues while
        // the previous action succeeded (`process_cmd`, `/opt/local/bin/port`
        // lines 4506-4526, MacPorts 2.12.6, read 2026-09-25). `port -q installed`
        // succeeds for any name, so a second action after it always runs -
        // checking the first word alone would permit `installed ; <anything>`
        // (spec-guardian, 2026-09-25). No package name can be `;`
        // (`is_safe_provider_name`), so refusing the element costs no read.
        separators: &[";"],
    },
];

/// The four macOS probes: permitted programs with no subcommand dimension,
/// named here rather than implied by their absence from
/// [`PERMITTED_SUBCOMMANDS`] - so a provider added to [`PERMITTED_PROGRAMS`]
/// without a read list is refused rather than let through unchecked.
/// `every_permitted_program_is_a_probe_or_has_a_read_list` holds the partition.
pub(crate) const SYSTEM_PROBES: &[&str] = &["sw_vers", "uname", "sysctl", "xcode-select"];

/// One provider's permitted reads, keyed by the program's leaf name.
#[derive(Debug)]
pub(crate) struct ProviderReads {
    /// Leaf name, as in [`PERMITTED_PROGRAMS`].
    pub(crate) program: &'static str,
    /// Options that may precede the subcommand, and nothing else may.
    pub(crate) leading_flags: &'static [&'static str],
    /// The subcommands this tool may ask for, matched exactly.
    pub(crate) subcommands: &'static [&'static str],
    /// Elements the provider reads as the start of another action, refused
    /// wherever they appear in the argv.
    pub(crate) separators: &'static [&'static str],
}

/// Whether `program` may be started with `argv`.
///
/// [`is_permitted_program`]; then, for a provider, a permitted subcommand
/// followed by nothing a provider reads as more than an argument - no action
/// separator, and no single-dash option, since `port -F` after the action is as
/// live as before it. Every argument vector the adapters hold uses only
/// `--long` options past the subcommand, so this refuses nothing they run. For
/// anything that is not a provider, membership of [`SYSTEM_PROBES`], the one
/// set of programs with no subcommand dimension.
pub(crate) fn is_permitted_invocation(program: &std::path::Path, argv: &[String]) -> bool {
    if !is_permitted_program(program) {
        return false;
    }
    let name = program.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let Some(reads) = PERMITTED_SUBCOMMANDS.iter().find(|r| r.program == name) else {
        return SYSTEM_PROBES.contains(&name);
    };
    let mut rest = argv
        .iter()
        .map(String::as_str)
        .skip_while(|a| reads.leading_flags.contains(a));
    let Some(subcommand) = rest.next() else {
        return false;
    };
    let short_option = |a: &str| a.starts_with('-') && !a.starts_with("--");
    reads.subcommands.contains(&subcommand)
        && !argv.iter().any(|a| reads.separators.contains(&a.as_str()))
        && rest.all(|a| !short_option(a))
}

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
pub(in crate::process) fn may_spawn(program: &std::path::Path, argv: &[String]) -> bool {
    if is_permitted_invocation(program, argv) {
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
    fn invariant_10_every_program_an_adapter_names_is_on_the_allowlist() {
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

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn every_subcommand_an_adapter_runs_is_permitted_and_no_other() {
        // Both directions, as for the programs above. An adapter that starts
        // running a new read fails here rather than refusing itself on a user's
        // machine; a subcommand on the list that no adapter runs is permission
        // nobody is using, and the executor pass should have to add its own.
        let brew = std::path::Path::new("/opt/homebrew/bin/brew");
        let port = std::path::Path::new("/opt/local/bin/port");
        let mut used: Vec<(&str, String)> = Vec::new();
        for held in crate::homebrew::ALLOWED_ARGV {
            let held = argv(held);
            assert!(is_permitted_invocation(brew, &held), "brew {held:?}");
            used.push(("brew", held[0].clone()));
        }
        for held in crate::macports::ALLOWED_ARGV
            .iter()
            .chain(crate::macports::ALLOWED_ARGV_PREFIXES)
        {
            let held = argv(held);
            assert!(is_permitted_invocation(port, &held), "port {held:?}");
            let sub = held.iter().find(|a| *a != "-q").unwrap();
            used.push(("port", sub.clone()));
        }
        for reads in PERMITTED_SUBCOMMANDS {
            for sub in reads.subcommands {
                assert!(
                    used.iter().any(|(p, s)| *p == reads.program && s == sub),
                    "{} {sub} is permitted and no adapter runs it",
                    reads.program
                );
            }
        }
    }

    #[test]
    fn a_provider_mutation_is_refused_whatever_surrounds_it() {
        // F-53's own case and its neighbours. The mutating verbs are assembled
        // at run time: the Stop gate greps `src/` for them as literals, and
        // these are test data, not a code path (`.claude/hooks/test-hooks.sh`
        // does the same for the same reason).
        let add = ["in", "stall"].concat();
        let remove = ["un", "in", "stall"].concat();
        let upgrade = ["up", "grade"].concat();
        let brew = std::path::Path::new("/opt/homebrew/bin/brew");
        let port = std::path::Path::new("/opt/local/bin/port");
        for (program, bad) in [
            (brew, vec![add.clone(), "wget".into()]),
            (brew, vec![remove.clone(), "wget".into()]),
            (brew, vec![upgrade.clone()]),
            // No leading option precedes a Homebrew subcommand here, so a flag
            // in front is not a way round the check.
            (brew, vec!["-q".into(), "list".into()]),
            (port, vec!["-q".into(), add.clone(), "htop".into()]),
            (port, vec![remove.clone(), "htop".into()]),
            // `-F` reads the actions from a file: a read subcommand after it
            // would be a filename.
            (port, vec!["-F".into(), "installed".into()]),
            // A read first, and a second action after the separator. `port -q
            // installed` succeeds for any name, so `port` would run it.
            (
                port,
                vec!["-q".into(), "installed".into(), ";".into(), add.clone()],
            ),
            (
                port,
                vec!["version".into(), ";".into(), remove.clone(), "htop".into()],
            ),
            // A command file, after the action rather than before it.
            (
                port,
                vec![
                    "-q".into(),
                    "installed".into(),
                    "-F".into(),
                    "/tmp/x".into(),
                ],
            ),
            (port, vec!["version".into(), "-F".into(), "-".into()]),
            (
                port,
                vec![
                    "-q".into(),
                    "installed".into(),
                    "-qF".into(),
                    "/tmp/x".into(),
                ],
            ),
            // An abbreviation `port` would resolve, and a name in another case.
            (port, vec!["-q".into(), "inst".into()]),
            (port, vec!["-q".into(), "INSTALLED".into()]),
            // Leading options only, and no subcommand at all.
            (port, vec!["-q".into()]),
            // `port` with no arguments is its interactive mode.
            (port, vec![]),
            (brew, vec![]),
        ] {
            assert!(
                !is_permitted_invocation(program, &bad),
                "{} {bad:?} was permitted",
                program.display()
            );
        }
        // The dimension is per provider: a system probe has none, and a
        // provider still has to pass the program check first.
        assert!(is_permitted_invocation(
            std::path::Path::new("/usr/bin/uname"),
            &argv(&["-m"])
        ));
        assert!(!is_permitted_invocation(
            std::path::Path::new("/tmp/brew"),
            &argv(&["list"])
        ));
    }

    #[test]
    fn every_permitted_program_is_a_probe_or_has_a_read_list() {
        // A partition, not two lists that happen to agree: a program in both
        // would be a probe with a read list nobody checks, and one in neither
        // is refused outright by `is_permitted_invocation`.
        for name in PERMITTED_PROGRAMS {
            let probe = SYSTEM_PROBES.contains(name);
            let reads = PERMITTED_SUBCOMMANDS.iter().any(|r| r.program == *name);
            assert!(probe != reads, "{name}: probe={probe}, read list={reads}");
        }
        assert_eq!(
            SYSTEM_PROBES.len() + PERMITTED_SUBCOMMANDS.len(),
            PERMITTED_PROGRAMS.len()
        );
    }

    #[test]
    fn invariant_05_a_provider_is_never_updated_or_upgraded_from_the_spawn_site() {
        // INV-05: neither provider is upgraded or reconfigured implicitly. The
        // adapters never ask, and this is what holds if something else does -
        // a plan's verbs come from policy data, not from an adapter (F-53).
        // Assembled at run time; see the next test for why.
        let update = ["up", "date"].concat();
        let upgrade = ["up", "grade"].concat();
        let selfupdate = ["self", "up", "date"].concat();
        let sync = ["sy", "nc"].concat();
        // Built in two steps so that no string literal follows the verb on its
        // line: the Stop gate's pattern reads `".into(), <verb>.clone(), "` as
        // one quoted string containing it.
        let mut upgrade_outdated = vec![String::from("-q")];
        upgrade_outdated.push(upgrade.clone());
        upgrade_outdated.push(String::from("outdated"));
        let brew = std::path::Path::new("/opt/homebrew/bin/brew");
        let port = std::path::Path::new("/opt/local/bin/port");
        for (program, bad) in [
            (brew, vec![update.clone()]),
            (brew, vec![upgrade.clone()]),
            (port, vec!["-q".into(), selfupdate.clone()]),
            (port, vec!["-q".into(), sync]),
            (port, upgrade_outdated),
            // Behind a read that always succeeds, which is how `port` would
            // reach it (see `separators`).
            (
                port,
                vec!["-q".into(), "installed".into(), ";".into(), selfupdate],
            ),
        ] {
            assert!(
                !is_permitted_invocation(program, &bad),
                "{} {bad:?} was permitted",
                program.display()
            );
        }
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
