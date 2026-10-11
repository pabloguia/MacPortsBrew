// SPDX-License-Identifier: Apache-2.0

//! Reading this machine: architecture, macOS version, translation state,
//! developer tools, and the shell environment as `05-environment-and-primary-
//! provider.md` says to *report* it.
//!
//! Everything here goes through the central executor, so the same code reads a
//! live machine and replays a captured fixture. No provider knowledge appears
//! in this module: it answers the domain's [`MachineRead`] contract and
//! nothing else.
//!
//! # Detection order
//!
//! The architecture reported is the **hardware's**, not the process's. On
//! Apple Silicon a translated process sees `x86_64` from both `uname -m` and
//! `hw.machine`, while `hw.optional.arm64` is `1` regardless — so that sysctl
//! is asked first and the `uname` / `hw.machine` pair is the fallback for a
//! machine that does not have it.
//!
//! Translation is read from `sysctl.proc_translated`. Apple documents the
//! contract for the equivalent `sysctlbyname` call: `0` means the process runs
//! natively, `1` means it is translated, `-1` means the call failed, and a
//! failure with `ENOENT` — the OID does not exist, which is what an Intel Mac
//! reports — is to be treated as native.
//! Source: <https://developer.apple.com/documentation/apple-silicon/about-the-rosetta-translation-environment>,
//! read 2026-09-02. `sysctl(8)` surfaces that same `ENOENT` as exit status 1
//! with `unknown oid` on stderr, which is what this module matches; the
//! observation is in `fixtures/recordings/homebrew-only-intel-macos26.json`
//! and was reproduced on this machine on 2026-09-02.
//!
//! Capability detection, not version comparison
//! (`08-distribution-and-project.md`, "Compatibility posture"): nothing here
//! asks whether macOS is new enough to have a sysctl, it asks whether the
//! sysctl answered.

use crate::process::{
    sanitised_environment, CommandSpec, EnvironmentSpec, ExitStatus, Invocation, ProcessRunner,
};
use macportsbrew_domain::{
    Architecture, CommandLineTools, DomainError, EnvironmentSnapshot, ErrorId, Machine,
    MachineRead, MacosVersion, PathFinding, Translation,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `sw_vers`, absolute: correctness here must not depend on `PATH`
/// (`04-migration-and-safety.md`, threat model, "Executable shadowing").
pub(crate) const SW_VERS: &str = "/usr/bin/sw_vers";
/// `uname`, absolute.
pub(crate) const UNAME: &str = "/usr/bin/uname";
/// `sysctl`, absolute.
pub(crate) const SYSCTL: &str = "/usr/sbin/sysctl";
/// `xcode-select`, absolute.
pub(crate) const XCODE_SELECT: &str = "/usr/bin/xcode-select";

/// These probes answer instantly or not at all; a long wait means something is
/// wrong with the machine, not that the answer is coming.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// The environment variable that relocates the state directory. Read here, so
/// that the one environment-reading surface stays one surface; consumed by
/// [`crate::persistence::StateDir::resolve`].
pub const STATE_DIR_VARIABLE: &str = "MACPORTSBREW_STATE_DIR";

/// Whether a variable is one `05-environment-and-primary-provider.md` says to
/// *report* (never to manage).
///
/// It is deliberately the same set the executor *strips* from every provider
/// invocation under INV-10, plus `MANPATH`. One list, asked two ways: a
/// variable dangerous enough to be removed from a provider's environment is
/// exactly a variable the user needs told about, and two hand-maintained lists
/// would drift into a `doctor` that reports a clean machine while the executor
/// is busy defusing `CMAKE_PREFIX_PATH`. `PATH` is excluded because
/// [`EnvironmentSnapshot::path`] carries it in order.
///
/// Provider-specific variables are deliberately absent: this module is
/// provider-neutral, and a provider's own variables belong to that provider's
/// snapshot (`ProviderSnapshot::environment`). Keeping them out is also what
/// keeps the two providers symmetric here (INV-17).
///
/// The values captured are redacted of the home directory and the user name.
/// The diagnostic file `doctor --report` writes applies the stricter rule in
/// `06-cli-and-ux.md` ("Privacy and diagnostics") — no values at all — which
/// is the renderer's job, not the adapter's: an analysis needs to know that
/// `CPPFLAGS` points at the other provider's prefix, and a bug report does
/// not. That split is not yet recorded in `docs/spec-evolution.md`, and it
/// needs to be before `--report` ships.
///
/// The build-flag list itself is [`macportsbrew_domain::is_build_flag`], so a
/// variable the executor strips is reported here without anyone keeping two
/// lists in step by hand. `MANPATH` is the deliberate extra: it is not stripped,
/// because it changes no build, but a `MANPATH` pointing at the other prefix is
/// worth telling a reader about.
pub fn is_reported(name: &str) -> bool {
    name == "MANPATH" || macportsbrew_domain::is_build_flag(name)
}

/// Replaces the home directory with `~`, wherever it appears.
///
/// The implementation is [`macportsbrew_domain::redact_home`]. It moved to the
/// domain when the application layer's capture began redacting the paths a
/// provider hands back — two layers redacting with two copies of the rule is
/// two rules waiting to disagree, and the one that disagrees quietly is the one
/// that publishes a username.
pub use macportsbrew_domain::{redact_home, HomeRedaction};

/// Reads the process environment.
///
/// This is the **only** place in the workspace that consults
/// `std::env::var`. Everything else receives the values as inputs, so that a
/// snapshot replays identically whatever the shell that runs it looks like
/// (INV-15). The tuple is `PATH`, the reported variables that are set, and the
/// home directory when it is an absolute path.
pub fn from_process_environment() -> (Option<String>, BTreeMap<String, String>, Option<PathBuf>) {
    let path = std::env::var("PATH").ok();
    // Enumerated rather than looked up name by name, so that every `DYLD_*`
    // the executor would strip is also one `doctor` can report.
    let variables = std::env::vars()
        .filter(|(name, _)| is_reported(name))
        .collect();
    let home = std::env::var("HOME").ok().as_deref().and_then(usable_home);
    (path, variables, home)
}

/// Trailing slashes are trimmed and `/` is rejected outright. `HOME` is what
/// redaction substitutes and what a provider reads its own configuration
/// from, and `/Users/x/` is a shape shells and launchd hand out freely: it
/// redacts to `~/` rather than `~`, which made every live run refuse its own
/// snapshot until this normalised. Doing it at the one place that reads the
/// environment fixes redaction everywhere rather than teaching each consumer
/// the same trick.
pub fn usable_home(value: &str) -> Option<PathBuf> {
    let trimmed = PathBuf::from(value.trim_end_matches('/'));
    (trimmed.is_absolute() && trimmed.components().count() > 1).then_some(trimmed)
}

/// This process's effective uid, read as the owner of a file it just created.
///
/// Not from `$HOME`, which was the first attempt and was wrong in both
/// directions: the environment decides what `$HOME` says, so a home the process
/// does not own makes every directory this user *cannot* write look writable —
/// a run with `HOME=/private/var` reported `/usr/bin` as "belongs to you" —
/// while a home that cannot be stat'd silently switches the check off, which is
/// one line in a shell profile away for exactly the adversary who put `~/bin`
/// on the `PATH` in the first place. It is also not this process under `sudo`,
/// where the euid is 0 and `$HOME` still names the invoking user.
///
/// The owner of a file this process just made is this process, and no variable
/// can redirect that. `std` exposes no `getuid`; it does expose the owner of a
/// file. The technique is the one `persistence::state_dir` already uses. A
/// directory that will not take the probe yields `None`, and the caller reports
/// that the check did not run rather than reporting nothing.
///
/// The name is never removed before use: a file already there, whoever made
/// it, is someone else's, and the probe moves to the next name instead.
/// Another user pre-creating every name to blind the check would have to
/// guess sixteen of them per process.
pub fn effective_uid() -> Option<u32> {
    let dir = std::env::temp_dir();
    for attempt in 0..16u32 {
        let probe = dir.join(format!(
            ".macportsbrew-uid.{}.{attempt}",
            std::process::id()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&probe)
        {
            Ok(file) => {
                let uid = file.metadata().map(|m| m.uid()).ok();
                drop(file);
                std::fs::remove_file(&probe).ok();
                return uid;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return None,
        }
    }
    None
}

/// Reads the locale preference variables in the order `06-cli-and-ux.md`
/// gives them: the application override, `LC_ALL`, `LANG`. Part of the same
/// single environment-reading surface as [`from_process_environment`].
pub fn language_preference() -> (Option<String>, Option<String>, Option<String>) {
    (
        std::env::var("MACPORTSBREW_LANG").ok(),
        std::env::var("LC_ALL").ok(),
        std::env::var("LANG").ok(),
    )
}

/// Reads the state-directory override, [`STATE_DIR_VARIABLE`].
///
/// Part of the same single environment-reading surface as
/// [`from_process_environment`]; kept separate only because it answers a
/// different question and belongs to a different caller.
pub fn state_dir_override() -> Option<String> {
    std::env::var(STATE_DIR_VARIABLE)
        .ok()
        .filter(|v| !v.trim().is_empty())
}

/// Reads the machine through a [`ProcessRunner`].
pub struct SystemMachine {
    runner: Box<dyn ProcessRunner>,
    path_env: Option<String>,
    variables: BTreeMap<String, String>,
    home: HomeRedaction,
    /// The probes are pure with respect to a given machine, so the answer is
    /// computed once. A failure is cached too: re-running a probe that already
    /// failed would produce a second, differently worded error for one cause.
    cached: RefCell<Option<Result<Machine, DomainError>>>,
}

impl SystemMachine {
    /// Builds a reader over a runner and an already-observed environment.
    ///
    /// The environment is passed in rather than read here so the adapter is
    /// testable and so that replay is reproducible; see
    /// [`from_process_environment`] for the one function that observes it.
    pub fn new(
        runner: Box<dyn ProcessRunner>,
        path_env: Option<String>,
        variables: BTreeMap<String, String>,
        home: HomeRedaction,
    ) -> Self {
        Self {
            runner,
            path_env,
            variables,
            home,
            cached: RefCell::new(None),
        }
    }

    /// True when the answers come from a recording rather than this machine.
    pub fn is_replay(&self) -> bool {
        self.runner.is_replay()
    }

    /// The `PATH` entries as observed, unredacted and in order. `which` and a
    /// path audit need the real directories; [`MachineRead::environment`]
    /// carries the redacted form.
    pub fn path_entries(&self) -> Vec<String> {
        split_path(self.path_env.as_deref().unwrap_or_default())
    }

    /// The home directory this reader redacts against, when one is known.
    pub fn home(&self) -> &HomeRedaction {
        &self.home
    }

    /// Runs one probe. A probe that could not be started is not an error here:
    /// each caller decides what a missing tool means for the fact it is after.
    fn probe(&self, program: &'static str, argv: &[&str]) -> Result<Invocation, DomainError> {
        let spec = CommandSpec::system(
            program,
            argv.iter().map(|a| (*a).to_string()).collect(),
            sanitised_environment(&EnvironmentSpec::default()),
        )
        .with_timeout(PROBE_TIMEOUT);
        self.runner.run(&spec)
    }

    /// `sysctl -n <name>`. The whole [`Invocation`] comes back so a caller can
    /// tell "answered nothing" from "the OID does not exist".
    fn sysctl(&self, name: &str) -> Result<Invocation, DomainError> {
        self.probe(SYSCTL, &["-n", name])
    }

    fn detect(&self) -> Result<Machine, DomainError> {
        let macos = self.detect_macos()?;
        let architecture = self.detect_architecture()?;
        let translation = self.detect_translation(architecture);
        let command_line_tools = self.detect_command_line_tools();
        Ok(Machine {
            architecture,
            macos,
            translation,
            command_line_tools,
        })
    }

    /// The macOS version. The domain has no `Unknown` for it and every support
    /// decision keys on it, so a machine that will not answer is an error
    /// rather than a guess.
    fn detect_macos(&self) -> Result<MacosVersion, DomainError> {
        let inv = self.probe(SW_VERS, &["-productVersion"])?;
        if let ExitStatus::NotStarted { error } = &inv.status {
            return Err(DomainError::new(
                ErrorId::Io,
                format!("{SW_VERS} could not be started: {error}"),
            ));
        }
        if !inv.status.success() {
            return Err(DomainError::new(
                ErrorId::Io,
                format!(
                    "{SW_VERS} -productVersion ended with {:?}: {}",
                    inv.status,
                    inv.stderr.trim()
                ),
            ));
        }
        MacosVersion::parse(&inv.stdout).ok_or_else(|| {
            DomainError::new(
                ErrorId::Io,
                format!(
                    "{SW_VERS} -productVersion did not report a version: {:?}",
                    inv.stdout.trim()
                ),
            )
        })
    }

    /// What one `sysctl -n` probe established, and nothing more.
    fn sysctl_answer(&self, name: &str) -> SysctlAnswer {
        let Ok(inv) = self.sysctl(name) else {
            return SysctlAnswer::NoAnswer;
        };
        if inv.status.success() {
            return SysctlAnswer::Value(inv.stdout.trim().to_string());
        }
        // `sysctl(8)` surfaces `ENOENT` as an exit with `unknown oid` on
        // stderr (module documentation). Anything else that did not exit 0 -
        // not started, timed out, signalled, another error - answered nothing.
        let unknown_oid = matches!(inv.status, ExitStatus::Exited { .. })
            && inv.stderr.to_ascii_lowercase().contains("unknown oid");
        if unknown_oid {
            SysctlAnswer::UnknownOid
        } else {
            SysctlAnswer::NoAnswer
        }
    }

    /// The hardware architecture. `hw.optional.arm64` is asked first because
    /// it is the only one of the three that is true of the machine rather than
    /// of the process.
    ///
    /// `hw.machine` and `uname -m` describe the **process**: a translated
    /// process on Apple Silicon sees `x86_64` from both. So they are consulted
    /// only once the hardware question has a clean answer that is not "arm64"
    /// (`hw.optional.arm64` reported `0` or does not exist) or, when that
    /// probe failed, once `sysctl.proc_translated` has ruled translation out.
    /// A probe that failed is never read as "not Apple Silicon".
    fn detect_architecture(&self) -> Result<Architecture, DomainError> {
        match self.sysctl_answer("hw.optional.arm64") {
            SysctlAnswer::Value(value) if value == "1" => return Ok(Architecture::Arm64),
            SysctlAnswer::Value(value) if value == "0" => {}
            SysctlAnswer::UnknownOid => {}
            SysctlAnswer::Value(_) | SysctlAnswer::NoAnswer => {
                match self.sysctl_answer("sysctl.proc_translated") {
                    // Only Apple Silicon translates.
                    SysctlAnswer::Value(value) if value == "1" => {
                        return Ok(Architecture::Arm64);
                    }
                    // A native process: what it sees is the hardware. An
                    // absent OID is an Intel Mac - it always exists on Apple
                    // Silicon (see `detect_translation`).
                    SysctlAnswer::Value(value) if value == "0" => {}
                    SysctlAnswer::UnknownOid => {}
                    SysctlAnswer::Value(_) | SysctlAnswer::NoAnswer => {
                        return Err(DomainError::new(
                            ErrorId::Io,
                            "hw.optional.arm64 did not answer and sysctl.proc_translated could \
                             not rule out a translated process, so hw.machine and uname -m \
                             cannot be trusted to name the hardware",
                        ));
                    }
                }
            }
        }
        for (program, argv) in [(SYSCTL, &["-n", "hw.machine"][..]), (UNAME, &["-m"][..])] {
            if let Ok(inv) = self.probe(program, argv) {
                if inv.status.success() {
                    if let Some(arch) = Architecture::parse(&inv.stdout) {
                        return Ok(arch);
                    }
                }
            }
        }
        Err(DomainError::new(
            ErrorId::Io,
            "no architecture could be read from hw.optional.arm64, hw.machine or uname -m",
        ))
    }

    /// Translation state, per Apple's documented `sysctl.proc_translated`
    /// contract (see the module documentation for the citation and date).
    fn detect_translation(&self, architecture: Architecture) -> Translation {
        let Ok(inv) = self.sysctl("sysctl.proc_translated") else {
            return Translation::Unknown;
        };
        if inv.status.success() {
            return match inv.stdout.trim() {
                "1" => Translation::Translated,
                "0" => Translation::Native,
                _ => Translation::Unknown,
            };
        }
        // The OID is absent. Apple's guidance is that `ENOENT` means native,
        // and an Intel Mac is where that happens. On Apple Silicon the OID
        // always exists, so its absence there is unexplained and is reported
        // as such rather than assumed away.
        let unknown_oid = matches!(inv.status, ExitStatus::Exited { .. })
            && inv.stderr.to_ascii_lowercase().contains("unknown oid");
        if unknown_oid && architecture == Architecture::X86_64 {
            Translation::Native
        } else {
            Translation::Unknown
        }
    }

    /// The active developer directory. A non-zero exit is `xcode-select`
    /// saying there is none; a tool that would not start says nothing either
    /// way.
    ///
    /// The path is redacted like every other captured path. A developer
    /// selects a toolchain under their home directory often enough
    /// (`xcode-select -s ~/Applications/…`) that an unredacted answer here
    /// would put a username into every snapshot and every fixture captured
    /// through this code (`06-cli-and-ux.md`, "Privacy and diagnostics").
    /// A relative answer is untrusted output that would become a path, and is
    /// refused rather than stored.
    fn detect_command_line_tools(&self) -> CommandLineTools {
        let Ok(inv) = self.probe(XCODE_SELECT, &["-p"]) else {
            return CommandLineTools::Unknown;
        };
        if matches!(inv.status, ExitStatus::NotStarted { .. }) {
            return CommandLineTools::Unknown;
        }
        if !inv.status.success() {
            return CommandLineTools::Missing;
        }
        let path = inv.stdout.trim();
        if path.is_empty() || !path.starts_with('/') {
            CommandLineTools::Unknown
        } else {
            CommandLineTools::Installed {
                path: PathBuf::from(redact_home(path, &self.home)),
            }
        }
    }
}

/// The three things a `sysctl -n <name>` probe can establish. A failure to
/// answer is its own case, never folded into "the OID does not exist".
enum SysctlAnswer {
    /// Exited 0; the value, trimmed.
    Value(String),
    /// Exited reporting `unknown oid`: the documented `ENOENT`.
    UnknownOid,
    /// Not started, timed out, signalled, or failed some other way.
    NoAnswer,
}

impl MachineRead for SystemMachine {
    fn machine(&self) -> Result<Machine, DomainError> {
        let mut cached = self.cached.borrow_mut();
        if cached.is_none() {
            *cached = Some(self.detect());
        }
        cached.as_ref().expect("populated above").clone()
    }

    fn environment(&self) -> Result<EnvironmentSnapshot, DomainError> {
        let home = &self.home;
        let mut reported = BTreeMap::new();
        for (name, value) in &self.variables {
            if is_reported(name) {
                reported.insert(name.clone(), redact_home(value, home));
            }
        }
        // Audit first, on the **raw** entries, and redact afterwards.
        //
        // The order is the whole point. Auditing a redacted entry means asking
        // the filesystem about a path that has been rewritten, and the rewrite
        // is not always reversible: `~` cannot be expanded when the home is
        // unresolved, so every home-relative entry was classified `Relative` and
        // never stat'd — silently dropping the world- and group-writable checks
        // on exactly the runs (`HOME` unset: cron, launchd, CI) that the
        // redaction fallback exists for, and reporting an absolute path as
        // "not absolute" while doing it. The same round trip turned a username
        // outside the home — `/tmp/build/ada/bin` — into `/tmp/build/USER/bin`,
        // which is absolute, so it *was* stat'd, at a path that does not exist:
        // a spurious `Missing` in place of a real finding.
        //
        // The audit stats real directories, so it belongs to the capture and not
        // to the analysis: what it finds is a fact about *this* host, it is
        // written into the snapshot here, and everything downstream reads the
        // recording rather than the filesystem (INV-15). A replayed reader has
        // no `PATH`, so it produces no findings at all.
        let raw = EnvironmentSnapshot {
            path: self.path_entries().clone(),
            reported,
            path_findings: Vec::new(),
        };
        let me = effective_uid();
        let path_findings = crate::system::path_audit::audit_path(&raw, me)
            .into_iter()
            .map(|f| PathFinding {
                entry: redact_home(&f.entry, home),
                ..f
            })
            .collect();
        Ok(EnvironmentSnapshot {
            path: raw.path.iter().map(|e| redact_home(e, home)).collect(),
            path_findings,
            ..raw
        })
    }

    /// Resolves a name against the observed `PATH`.
    ///
    /// **Live machines only.** A replayed reader answers `None` for
    /// everything: the filesystem this would consult is the replaying host's,
    /// not the captured machine's, and letting a host fact into a replayed
    /// analysis is exactly the INV-15 break `analyze --from-snapshot` exists
    /// to catch. A caller working from a snapshot reads
    /// `ProviderSnapshot::resolutions`, which is where the captured answers
    /// live.
    fn which(&self, executable: &str) -> Option<PathBuf> {
        if self.runner.is_replay() || !is_plain_executable_name(executable) {
            return None;
        }
        for entry in self.path_entries() {
            // A `PATH` entry that is not absolute is the shadowing attack in
            // `04-migration-and-safety.md`, not a directory to search. It is
            // reported by `path_audit`, never followed. A real shell *would*
            // search it, so a hit here can still be shadowed at run time by a
            // relative entry that precedes it; whatever builds a `Resolution`
            // has to join this answer to the audit's `Relative` finding.
            if !entry.starts_with('/') {
                continue;
            }
            let candidate = Path::new(&entry).join(executable);
            // `metadata` resolves a symbolic reference to its target, so what
            // is tested is the file the shell would actually run. The path
            // returned is the one `PATH` produced, never a canonicalised one.
            // `05-environment-and-primary-provider.md` asks for the *resolved*
            // path when asking a provider who owns an executable, so that
            // caller must canonicalise this answer first — this returns what
            // `PATH` says, not what the link points at.
            //
            // The mode test is "some class may execute", not "this process
            // may": `access(X_OK)` needs libc. A root-owned `0o700` binary in
            // an early entry is therefore reported as the winner although
            // running it would fail.
            if let Ok(meta) = std::fs::metadata(&candidate) {
                if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
                    return Some(candidate);
                }
            }
        }
        None
    }
}

/// Splits a `PATH` string. Empty components are kept: the shell reads one as
/// the working directory, which is a finding, and dropping it here would hide
/// it from the audit.
fn split_path(path: &str) -> Vec<String> {
    if path.is_empty() {
        return Vec::new();
    }
    path.split(':').map(ToString::to_string).collect()
}

/// A name `which` will look up: no separator, no leading dash, not empty.
/// A name that could become a path or a flag is exactly what the threat model
/// forbids letting through.
fn is_plain_executable_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.starts_with('-')
        && !name.contains('\0')
        && name != "."
        && name != ".."
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{RecordedInvocation, Recording, RecordingProvenance, ReplayRunner};
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Mirrors `process::recording::RECORDING_SCHEMA_VERSION`, which the
    /// module does not re-export.
    const SCHEMA: u32 = 1;

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A private working directory for a test. No `tempfile` dependency is
    /// available and none is being added for this.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "macportsbrew-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn ok(program: &str, argv: &[&str], stdout: &str) -> RecordedInvocation {
        record(program, argv, ExitStatus::Exited { code: 0 }, stdout, "")
    }

    fn failed(program: &str, argv: &[&str], code: i32, stderr: &str) -> RecordedInvocation {
        record(program, argv, ExitStatus::Exited { code }, "", stderr)
    }

    fn absent(program: &str, argv: &[&str]) -> RecordedInvocation {
        let status = ExitStatus::NotStarted {
            error: "No such file or directory (os error 2)".into(),
        };
        record(program, argv, status, "", "")
    }

    fn record(
        program: &str,
        argv: &[&str],
        status: ExitStatus,
        stdout: &str,
        stderr: &str,
    ) -> RecordedInvocation {
        RecordedInvocation {
            invocation: Invocation {
                program: program.to_string(),
                argv: argv.iter().map(|a| (*a).to_string()).collect(),
                environment: BTreeMap::new(),
                status,
                stdout: stdout.to_string(),
                stderr: stderr.to_string(),
                truncated: false,
                duration_ms: 1,
            },
        }
    }

    fn recording(invocations: Vec<RecordedInvocation>) -> Recording {
        Recording {
            schema_version: SCHEMA,
            provenance: RecordingProvenance {
                macos: "26.6.2".into(),
                architecture: "x86_64".into(),
                provider_versions: vec![],
                captured_at: "2026-09-02T00:00:00Z".into(),
                captured_by: "unit test".into(),
                origin: "synthesised".into(),
                note: None,
            },
            invocations,
        }
    }

    fn reader(invocations: Vec<RecordedInvocation>) -> SystemMachine {
        SystemMachine::new(
            Box::new(ReplayRunner::new(recording(invocations))),
            None,
            BTreeMap::new(),
            HomeRedaction::Recorded,
        )
    }

    /// Drops every recorded answer for one program so a test can substitute
    /// its own.
    fn without(mut invocations: Vec<RecordedInvocation>, argv: &[&str]) -> Vec<RecordedInvocation> {
        invocations.retain(|r| r.invocation.argv != argv);
        invocations
    }

    /// An Intel Mac: neither optional sysctl exists. Reproduced from
    /// `fixtures/recordings/homebrew-only-intel-macos26.json`.
    fn intel_native() -> Vec<RecordedInvocation> {
        vec![
            ok("sw_vers", &["-productVersion"], "26.6.2"),
            ok("uname", &["-m"], "x86_64"),
            ok("sysctl", &["-n", "hw.machine"], "x86_64"),
            failed(
                "sysctl",
                &["-n", "hw.optional.arm64"],
                1,
                "sysctl: unknown oid 'hw.optional.arm64'",
            ),
            failed(
                "sysctl",
                &["-n", "sysctl.proc_translated"],
                1,
                "sysctl: unknown oid 'sysctl.proc_translated'",
            ),
            ok(
                "xcode-select",
                &["-p"],
                "/Applications/Xcode.app/Contents/Developer",
            ),
        ]
    }

    /// Apple Silicon running natively: both sysctls exist and
    /// `proc_translated` is 0.
    fn apple_silicon_native() -> Vec<RecordedInvocation> {
        vec![
            ok("sw_vers", &["-productVersion"], "26.6.2"),
            ok("uname", &["-m"], "arm64"),
            ok("sysctl", &["-n", "hw.machine"], "arm64"),
            ok("sysctl", &["-n", "hw.optional.arm64"], "1"),
            ok("sysctl", &["-n", "sysctl.proc_translated"], "0"),
            ok(
                "xcode-select",
                &["-p"],
                "/Library/Developer/CommandLineTools",
            ),
        ]
    }

    /// The case the whole ordering exists for: Apple Silicon hardware seen
    /// from a translated process, where `uname -m` and `hw.machine` both
    /// report the process's architecture.
    fn apple_silicon_translated() -> Vec<RecordedInvocation> {
        vec![
            ok("sw_vers", &["-productVersion"], "15.6"),
            ok("uname", &["-m"], "x86_64"),
            ok("sysctl", &["-n", "hw.machine"], "x86_64"),
            ok("sysctl", &["-n", "hw.optional.arm64"], "1"),
            ok("sysctl", &["-n", "sysctl.proc_translated"], "1"),
            ok(
                "xcode-select",
                &["-p"],
                "/Library/Developer/CommandLineTools",
            ),
        ]
    }

    #[test]
    fn intel_machine_is_native_with_developer_tools() {
        let m = reader(intel_native()).machine().unwrap();
        assert_eq!(m.architecture, Architecture::X86_64);
        assert_eq!(m.macos, MacosVersion::new(26, 6, 2));
        assert_eq!(m.translation, Translation::Native);
        assert_eq!(
            m.command_line_tools,
            CommandLineTools::Installed {
                path: PathBuf::from("/Applications/Xcode.app/Contents/Developer"),
            }
        );
    }

    #[test]
    fn apple_silicon_native_machine_reads_arm64() {
        let m = reader(apple_silicon_native()).machine().unwrap();
        assert_eq!(m.architecture, Architecture::Arm64);
        assert_eq!(m.translation, Translation::Native);
        assert_eq!(
            m.command_line_tools,
            CommandLineTools::Installed {
                path: PathBuf::from("/Library/Developer/CommandLineTools"),
            }
        );
    }

    #[test]
    fn translated_process_reports_the_hardware_not_the_process() {
        let m = reader(apple_silicon_translated()).machine().unwrap();
        assert_eq!(m.architecture, Architecture::Arm64);
        assert_eq!(m.translation, Translation::Translated);
        assert_eq!(m.macos, MacosVersion::new(15, 6, 0));
    }

    #[test]
    fn an_unknown_oid_on_apple_silicon_is_not_assumed_native() {
        let mut invocations = without(apple_silicon_native(), &["-n", "sysctl.proc_translated"]);
        invocations.push(failed(
            "sysctl",
            &["-n", "sysctl.proc_translated"],
            1,
            "sysctl: unknown oid 'sysctl.proc_translated'",
        ));
        assert_eq!(
            reader(invocations).machine().unwrap().translation,
            Translation::Unknown
        );
    }

    #[test]
    fn unreadable_translation_is_unknown_rather_than_a_guess() {
        let mut invocations = without(intel_native(), &["-n", "sysctl.proc_translated"]);
        invocations.push(absent("sysctl", &["-n", "sysctl.proc_translated"]));
        assert_eq!(
            reader(invocations).machine().unwrap().translation,
            Translation::Unknown
        );

        let mut noise = without(intel_native(), &["-n", "sysctl.proc_translated"]);
        noise.push(ok("sysctl", &["-n", "sysctl.proc_translated"], "maybe"));
        assert_eq!(
            reader(noise).machine().unwrap().translation,
            Translation::Unknown
        );
    }

    #[test]
    fn developer_tools_missing_and_undeterminable_are_distinct() {
        let mut missing = without(intel_native(), &["-p"]);
        missing.push(failed(
            "xcode-select",
            &["-p"],
            2,
            "xcode-select: error: unable to get active developer directory",
        ));
        assert_eq!(
            reader(missing).machine().unwrap().command_line_tools,
            CommandLineTools::Missing
        );

        let mut unknown = without(intel_native(), &["-p"]);
        unknown.push(absent("xcode-select", &["-p"]));
        assert_eq!(
            reader(unknown).machine().unwrap().command_line_tools,
            CommandLineTools::Unknown
        );

        let mut empty = without(intel_native(), &["-p"]);
        empty.push(ok("xcode-select", &["-p"], "   "));
        assert_eq!(
            reader(empty).machine().unwrap().command_line_tools,
            CommandLineTools::Unknown
        );
    }

    #[test]
    fn a_machine_that_will_not_name_its_macos_is_an_error() {
        let mut invocations = without(intel_native(), &["-productVersion"]);
        invocations.push(absent("sw_vers", &["-productVersion"]));
        assert_eq!(reader(invocations).machine().unwrap_err().id, ErrorId::Io);

        let mut garbled = without(intel_native(), &["-productVersion"]);
        garbled.push(ok("sw_vers", &["-productVersion"], "not a version"));
        assert_eq!(reader(garbled).machine().unwrap_err().id, ErrorId::Io);

        let mut broken = without(intel_native(), &["-productVersion"]);
        broken.push(failed("sw_vers", &["-productVersion"], 1, "boom"));
        assert_eq!(reader(broken).machine().unwrap_err().id, ErrorId::Io);
    }

    #[test]
    fn architecture_falls_back_to_uname_once_the_hardware_question_is_answered() {
        // `hw.optional.arm64` absent (an Intel Mac) and `hw.machine` gone:
        // `uname -m` is a native process's answer, so it names the hardware.
        let mut invocations = without(intel_native(), &["-n", "hw.machine"]);
        invocations.push(absent("sysctl", &["-n", "hw.machine"]));
        let m = reader(invocations).machine().unwrap();
        assert_eq!(m.architecture, Architecture::X86_64);
        assert_eq!(m.translation, Translation::Native);
    }

    #[test]
    fn a_failed_arm64_probe_on_a_translated_process_never_reads_x86_64() {
        // Synthetic, labelled: Apple Silicon seen from a Rosetta process, the
        // `hw.optional.arm64` probe failing in every way it can fail. `uname -m`
        // and `hw.machine` say `x86_64` there; before the fix any of these
        // failures fell straight through to them.
        let failures = [
            absent("sysctl", &["-n", "hw.optional.arm64"]),
            record(
                "sysctl",
                &["-n", "hw.optional.arm64"],
                ExitStatus::TimedOut,
                "",
                "",
            ),
            failed(
                "sysctl",
                &["-n", "hw.optional.arm64"],
                1,
                "sysctl: some error",
            ),
            ok("sysctl", &["-n", "hw.optional.arm64"], "garbled"),
        ];
        for failure in failures {
            let mut translated = without(apple_silicon_translated(), &["-n", "hw.optional.arm64"]);
            translated.push(failure.clone());
            let m = reader(translated).machine().unwrap();
            assert_eq!(m.architecture, Architecture::Arm64, "{failure:?}");
            assert_eq!(m.translation, Translation::Translated, "{failure:?}");

            // And when translation cannot be ruled out either, the answer is
            // an error, not the process's x86_64.
            let mut blind = without(apple_silicon_translated(), &["-n", "hw.optional.arm64"]);
            blind = without(blind, &["-n", "sysctl.proc_translated"]);
            blind.push(failure.clone());
            blind.push(absent("sysctl", &["-n", "sysctl.proc_translated"]));
            assert_eq!(
                reader(blind).machine().unwrap_err().id,
                ErrorId::Io,
                "{failure:?}"
            );
        }
    }

    #[test]
    fn a_failed_arm64_probe_falls_back_once_translation_is_ruled_out() {
        // A native process (`proc_translated` 0) sees the hardware itself.
        let mut native = without(apple_silicon_native(), &["-n", "hw.optional.arm64"]);
        native.push(absent("sysctl", &["-n", "hw.optional.arm64"]));
        assert_eq!(
            reader(native).machine().unwrap().architecture,
            Architecture::Arm64
        );
        // An absent `proc_translated` is an Intel Mac.
        let mut intel = without(intel_native(), &["-n", "hw.optional.arm64"]);
        intel.push(absent("sysctl", &["-n", "hw.optional.arm64"]));
        assert_eq!(
            reader(intel).machine().unwrap().architecture,
            Architecture::X86_64
        );
    }

    #[test]
    fn a_machine_whose_sysctl_will_not_start_names_no_architecture() {
        // `uname -m` alone cannot tell Intel from a translated process.
        let invocations = vec![
            ok("sw_vers", &["-productVersion"], "10.12.6"),
            ok("uname", &["-m"], "x86_64"),
            absent("sysctl", &["-n", "hw.machine"]),
            absent("sysctl", &["-n", "hw.optional.arm64"]),
            absent("sysctl", &["-n", "sysctl.proc_translated"]),
            ok(
                "xcode-select",
                &["-p"],
                "/Library/Developer/CommandLineTools",
            ),
        ];
        assert_eq!(reader(invocations).machine().unwrap_err().id, ErrorId::Io);
    }

    #[test]
    fn no_architecture_at_all_is_an_error() {
        let invocations = vec![
            ok("sw_vers", &["-productVersion"], "26.6.2"),
            ok("uname", &["-m"], "ppc"),
            ok("sysctl", &["-n", "hw.machine"], "ppc"),
            absent("sysctl", &["-n", "hw.optional.arm64"]),
            absent("sysctl", &["-n", "sysctl.proc_translated"]),
            absent("xcode-select", &["-p"]),
        ];
        assert_eq!(reader(invocations).machine().unwrap_err().id, ErrorId::Io);
    }

    #[test]
    fn the_answer_is_cached_and_a_failure_stays_the_same_failure() {
        let working = reader(intel_native());
        assert_eq!(working.machine().unwrap(), working.machine().unwrap());
        assert!(working.is_replay());

        let mut invocations = without(intel_native(), &["-productVersion"]);
        invocations.push(absent("sw_vers", &["-productVersion"]));
        let failing = reader(invocations);
        assert_eq!(
            failing.machine().unwrap_err(),
            failing.machine().unwrap_err()
        );
    }

    #[test]
    fn the_environment_is_reported_redacted_and_filtered() {
        let mut variables = BTreeMap::new();
        variables.insert("CPPFLAGS".to_string(), "-I/Users/ada/.local/include".into());
        variables.insert("MANPATH".to_string(), "/opt/local/share/man".into());
        // Every variable the executor strips is one doctor can report, and the
        // `DYLD_*` rule is a prefix, not a list.
        variables.insert("CMAKE_PREFIX_PATH".to_string(), "/opt/local".into());
        variables.insert(
            "DYLD_INSERT_LIBRARIES".to_string(),
            "/opt/local/lib/x".into(),
        );
        variables.insert("SECRET_TOKEN".to_string(), "hunter2".into());
        let reader = SystemMachine::new(
            Box::new(ReplayRunner::new(recording(intel_native()))),
            Some("/Users/ada/bin:/usr/local/bin:/usr/bin".to_string()),
            variables,
            HomeRedaction::Known(PathBuf::from("/Users/ada")),
        );
        let env = reader.environment().unwrap();
        assert_eq!(env.path, vec!["~/bin", "/usr/local/bin", "/usr/bin"]);
        assert_eq!(env.reported["CPPFLAGS"], "-I~/.local/include");
        assert_eq!(env.reported["MANPATH"], "/opt/local/share/man");
        assert_eq!(env.reported["CMAKE_PREFIX_PATH"], "/opt/local");
        assert_eq!(env.reported["DYLD_INSERT_LIBRARIES"], "/opt/local/lib/x");
        assert!(!env.reported.contains_key("SECRET_TOKEN"));
        assert!(!env.reported.contains_key("PATH"));
        // Unredacted entries stay available to the code that must search them.
        assert_eq!(reader.path_entries()[0], "/Users/ada/bin");
        assert_eq!(
            reader.home(),
            &HomeRedaction::Known(PathBuf::from("/Users/ada"))
        );
    }

    #[test]
    fn a_home_with_a_trailing_slash_is_the_same_home() {
        // Not cosmetic. `HOME` is what redaction substitutes, and `/Users/x/`
        // redacts to `~/` rather than `~`, which made the tool refuse the very
        // snapshot it had just written - in every subcommand, on any machine
        // whose `HOME` carried a trailing slash, a shape shells and launchd hand
        // out freely. The guard that stops that lives in one expression, so it
        // gets a test of its own.
        assert_eq!(
            usable_home("/Users/ada/"),
            Some(PathBuf::from("/Users/ada"))
        );
        assert_eq!(
            usable_home("/Users/ada///"),
            Some(PathBuf::from("/Users/ada"))
        );
        assert_eq!(usable_home("/Users/ada"), Some(PathBuf::from("/Users/ada")));
        // A home of `/` names no directory of the user's, and redacting against
        // it would rewrite every absolute path in the snapshot.
        assert_eq!(usable_home("/"), None);
        assert_eq!(usable_home("//"), None);
        assert_eq!(usable_home("relative"), None);
        assert_eq!(usable_home(""), None);
    }

    #[test]
    fn every_variable_the_executor_strips_is_one_doctor_reports() {
        // A property now, not a copy: whatever the executor strips, `doctor`
        // reports. The list this iterates is the one the executor filters on.
        assert!(is_reported("MANPATH"), "the deliberate extra");
        for name in [
            "CPATH",
            "LDFLAGS",
            "CPPFLAGS",
            "CXXFLAGS",
            "CMAKE_PREFIX_PATH",
            "DYLD_LIBRARY_PATH",
        ] {
            assert!(
                macportsbrew_domain::is_build_flag(name) && is_reported(name),
                "{name} is stripped but not reported"
            );
        }
        // `PATH` has its own ordered field, and nothing else is captured.
        assert!(!is_reported("PATH"));
        assert!(!is_reported("HOME"));
        assert!(!is_reported("SECRET_TOKEN"));
        // Provider variables stay out of the provider-neutral module (INV-17).
        assert!(!is_reported("HOMEBREW_PREFIX"));
        assert!(!is_reported("MACPORTS_PREFIX"));
    }

    #[test]
    fn home_redaction_replaces_whole_components_wherever_they_appear() {
        let home = PathBuf::from("/Users/ada");
        let known = HomeRedaction::Known(home.clone());
        assert_eq!(redact_home("/Users/ada", &known), "~");
        assert_eq!(redact_home("/Users/ada/bin", &known), "~/bin");
        assert_eq!(redact_home("/usr/bin", &known), "/usr/bin");
        assert_eq!(
            redact_home("/Users/ada", &HomeRedaction::Recorded),
            "/Users/ada"
        );
        // A longer name that merely starts with the home path is not it.
        assert_eq!(redact_home("/Users/adamant", &known), "/Users/adamant");
        assert_eq!(
            redact_home("/Users/adamant/Users/ada/x", &known),
            "/Users/adamant~/x"
        );
        // Embedded and repeated occurrences, which is what a compiler-flag
        // variable and a colon-separated list actually look like.
        assert_eq!(
            redact_home("-I/Users/ada/.local/include", &known),
            "-I~/.local/include"
        );
        assert_eq!(
            redact_home("/Users/ada/a:/opt/local/b:/Users/ada", &known),
            "~/a:/opt/local/b:~"
        );
        // A home of `/` would redact the whole filesystem; it is left alone.
        assert_eq!(
            redact_home("/usr/bin", &HomeRedaction::Known(PathBuf::from("/"))),
            "/usr/bin"
        );
        assert_eq!(
            redact_home(
                "/Users/ada/x",
                &HomeRedaction::Known(PathBuf::from("/Users/ada/"))
            ),
            "~/x"
        );
    }

    #[test]
    fn a_live_run_with_no_resolvable_home_still_redacts_by_shape() {
        // The regression test for the leak this variant exists to close. With
        // `HOME` unset, `doctor --report` wrote the real home path 39 times into
        // a file `06-cli-and-ux.md` invites the user to attach to an issue,
        // because "no home" was treated as "nothing to redact".
        let live = HomeRedaction::Unresolved;
        assert_eq!(redact_home("/Users/ada", &live), "~");
        assert_eq!(redact_home("/Users/ada/bin", &live), "~/bin");
        assert_eq!(
            redact_home("-I/Users/ada/.local/include", &live),
            "-I~/.local/include"
        );
        // Once a name has been seen under /Users it is redacted elsewhere in the
        // same value too, on the same terms as the grounded path.
        assert_eq!(
            redact_home("/Users/ada/a -I/Volumes/build/ada/x", &live),
            "~/a -I/Volumes/build/USER/x"
        );
        // And the limit of the heuristic, stated rather than glossed: a name
        // that never appears under /Users cannot be recognised as a user name,
        // because nothing here knows it is one. This is exactly why the
        // diagnostic report records `heuristic` instead of implying the file was
        // redacted against a known home.
        assert_eq!(
            redact_home("/Volumes/build/ada/x", &live),
            "/Volumes/build/ada/x"
        );
        // Two different users in one value are both redacted.
        assert_eq!(redact_home("/Users/ada/a:/Users/bo/b", &live), "~/a:~/b");
        // Directories macOS puts under /Users that name no user are left alone,
        // and so is anything outside /Users.
        assert_eq!(redact_home("/Users/Shared/x", &live), "/Users/Shared/x");
        // A one-character home is still a home: the length filter belongs to the
        // bare-name pass, not to the path itself, which is where the grounded
        // path puts it too.
        assert_eq!(redact_home("/Users/j/bin", &live), "~/bin");
        // A multi-word directory yields a fragment, and rewriting a fragment
        // mangles the path while leaving the real name behind. Left alone
        // instead, which is what `HEURISTIC` on the artifact is for.
        assert_eq!(
            redact_home("/Users/Deleted Users/bob/x", &live),
            "/Users/Deleted Users/bob/x"
        );
        // Multi-byte names do not panic and are redacted like any other.
        assert_eq!(redact_home("/Users/Jos\u{e9}/lib", &live), "~/lib");
        assert_eq!(redact_home("/usr/local/bin", &live), "/usr/local/bin");
        // Already redacted input is unchanged, so replaying our own output is
        // not a second rewrite.
        assert_eq!(redact_home("~/bin", &live), "~/bin");
        // The cost of guessing, asserted rather than discovered later: a home
        // whose last component collides with an ordinary directory name takes
        // that name with it everywhere in the value. `/Users/runner` is the
        // home on a GitHub-hosted macOS runner, so this is the realistic case
        // and not a contrived one. It errs toward redacting, which is the right
        // direction for privacy and the wrong one for a legible report - which
        // is what the `HEURISTIC` label on the artifact exists to tell a reader.
        assert_eq!(
            redact_home("/Users/runner/w:/opt/runner/bin", &live),
            "~/w:/opt/USER/bin"
        );
        // But a name that is an ordinary path component far more often than it
        // is a user name keeps its home redacted and skips the bare-name pass:
        // rewriting `/opt/local/bin` to `/opt/USER/bin` would destroy the single
        // most load-bearing fact in a diagnostic report to hide a name that is
        // very probably not a name.
        assert_eq!(
            redact_home("/opt/local/bin:/Users/local/x", &live),
            "/opt/local/bin:~/x"
        );
    }

    #[test]
    fn a_recorded_home_is_left_exactly_as_captured() {
        // The other half of the distinction. A recording was redacted when it
        // was captured, and redacting it again against *this* host would be a
        // host fact reaching a replayed analysis (INV-15).
        let recorded = HomeRedaction::Recorded;
        assert_eq!(redact_home("/Users/ada/bin", &recorded), "/Users/ada/bin");
        // Three distinct labels. `Recorded` must not claim to be grounded: a
        // recording carries no record of how it was redacted, and a snapshot
        // captured with no resolvable home was redacted heuristically.
        assert_eq!(recorded.label(), "AS_CAPTURED");
        assert_eq!(HomeRedaction::Unresolved.label(), "HEURISTIC");
        assert_eq!(
            HomeRedaction::Known(PathBuf::from("/Users/ada")).label(),
            "GROUNDED"
        );
    }

    #[test]
    fn the_user_name_is_redacted_even_outside_the_home_directory() {
        // 06-cli-and-ux.md names usernames separately from home paths, and
        // `scripts/capture-fixture.sh` already replaces `id -un`. A build
        // directory on another volume is where the two rules differ.
        let home = PathBuf::from("/Users/ada");
        let known = HomeRedaction::Known(home.clone());
        assert_eq!(
            redact_home("-I/Volumes/build/ada/include", &known),
            "-I/Volumes/build/USER/include"
        );
        assert_eq!(redact_home("/opt/ada", &known), "/opt/USER");
        // A longer word that merely contains the name is left alone.
        assert_eq!(redact_home("/opt/adabas/lib", &known), "/opt/adabas/lib");
        assert_eq!(redact_home("/opt/prada", &known), "/opt/prada");
        // The home path is still replaced first, so `~` wins where both apply.
        assert_eq!(redact_home("/Users/ada/ada", &known), "~/USER");
    }

    #[test]
    fn which_finds_the_first_executable_and_refuses_anything_path_like() {
        let dir = scratch("which");
        let first = dir.join("first");
        let second = dir.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        std::fs::write(first.join("tool"), b"#!/bin/sh\n").unwrap();
        std::fs::write(second.join("tool"), b"#!/bin/sh\n").unwrap();
        std::fs::write(first.join("data"), b"x").unwrap();
        // Only the second copy carries the executable bit, so the first entry
        // is passed over rather than returned.
        std::fs::set_permissions(first.join("tool"), std::fs::Permissions::from_mode(0o644))
            .unwrap();
        std::fs::set_permissions(second.join("tool"), std::fs::Permissions::from_mode(0o755))
            .unwrap();

        let path = format!(
            "{}:relative/bin:{}",
            first.to_string_lossy(),
            second.to_string_lossy()
        );
        // A live runner: `which` reads the filesystem, and a replayed reader
        // deliberately answers `None` for everything (see its documentation).
        // No process is spawned by `which`, so nothing here touches the
        // machine beyond the scratch directory above.
        let reader = SystemMachine::new(
            Box::new(crate::process::SystemRunner::new()),
            Some(path),
            BTreeMap::new(),
            HomeRedaction::Recorded,
        );
        assert!(!reader.is_replay());
        assert_eq!(reader.which("tool"), Some(second.join("tool")));
        assert_eq!(reader.which("data"), None);
        assert_eq!(reader.which("first"), None, "a directory is not executable");
        assert_eq!(reader.which("nothing-here"), None);
        assert_eq!(reader.which("../tool"), None);
        assert_eq!(reader.which("/bin/sh"), None);
        assert_eq!(reader.which("-rf"), None);
        assert_eq!(reader.which(""), None);
        assert_eq!(reader.which("."), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_empty_path_yields_no_entries_and_an_empty_component_survives() {
        assert!(split_path("").is_empty());
        assert_eq!(split_path("/a::/b"), vec!["/a", "", "/b"]);
    }
}
