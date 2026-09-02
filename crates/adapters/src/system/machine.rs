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
    MachineRead, MacosVersion, Translation,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `sw_vers`, absolute: correctness here must not depend on `PATH`
/// (`04-migration-and-safety.md`, threat model, "Executable shadowing").
const SW_VERS: &str = "/usr/bin/sw_vers";
/// `uname`, absolute.
const UNAME: &str = "/usr/bin/uname";
/// `sysctl`, absolute.
const SYSCTL: &str = "/usr/sbin/sysctl";
/// `xcode-select`, absolute.
const XCODE_SELECT: &str = "/usr/bin/xcode-select";

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
/// The set below mirrors `process::environment::is_build_flag`, which this
/// module cannot call because `process` does not re-export it. Re-exporting it
/// and deleting this copy is the right end state; until then the two are kept
/// in step by hand, and `system_contract.rs` asserts the mirror is complete.
pub fn is_reported(name: &str) -> bool {
    matches!(
        name,
        "MANPATH"
            | "CPATH"
            | "C_INCLUDE_PATH"
            | "CPLUS_INCLUDE_PATH"
            | "LIBRARY_PATH"
            | "LD_LIBRARY_PATH"
            | "PKG_CONFIG_PATH"
            | "PKG_CONFIG_LIBDIR"
            | "LDFLAGS"
            | "CPPFLAGS"
            | "CFLAGS"
            | "CXXFLAGS"
            | "OBJCFLAGS"
            | "ACLOCAL_PATH"
            | "CMAKE_PREFIX_PATH"
    ) || name.starts_with("DYLD_")
}

/// Replaces the home directory with `~`, wherever it appears.
///
/// `06-cli-and-ux.md` ("Privacy and diagnostics") forbids home paths and
/// usernames in anything the user might attach to an issue, and a captured
/// snapshot is exactly that. Redaction happens at capture, so a home path is
/// never written down in the first place.
///
/// Every occurrence is replaced, not just a leading one: a `PATH` entry is a
/// whole path, but `CPPFLAGS` is `-I/Users/ada/.local/include` and
/// `PKG_CONFIG_PATH` is a colon-separated list, and a leading-only rule would
/// let the home path through in both. The match must end a path component, so
/// `/Users/adamant` is not inside `/Users/ada`. Where the two rules disagree
/// this errs toward redacting: over-redaction costs a reader some context,
/// under-redaction publishes a username.
///
/// `06-cli-and-ux.md` names usernames separately from home paths, so the home
/// directory's own last component is then replaced wherever it appears as a
/// whole component — `/Volumes/build/ada/include` keeps a username that
/// redacting the home path alone would miss. This matches what
/// `scripts/capture-fixture.sh` does with `id -un`, so the two capture routes
/// have the same privacy strength.
pub fn redact_home(value: &str, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return value.to_string();
    };
    let home = home.to_string_lossy();
    let home = home.trim_end_matches('/');
    if home.is_empty() {
        return value.to_string();
    }
    let redacted = replace_component(value, home, "~");
    match home.rsplit('/').next().filter(|u| u.len() > 1) {
        Some(user) => replace_component(&redacted, user, "USER"),
        None => redacted,
    }
}

/// Replaces `needle` with `replacement` wherever `needle` is a whole path
/// component, so a longer name that merely contains it survives:
/// `/Users/adamant` is not `/Users/ada`, and `/opt/prada` is not the user
/// `ada`.
///
/// A needle that starts with `/` carries its own leading boundary, which is
/// what lets `-I/Users/ada/x` be redacted even though `I` precedes the match.
/// A bare name needs a boundary on both sides.
fn replace_component(value: &str, needle: &str, replacement: &str) -> String {
    let needs_leading_boundary = !needle.starts_with('/');
    let mut redacted = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find(needle) {
        let before_ok = !needs_leading_boundary
            || rest[..at]
                .chars()
                .next_back()
                .is_none_or(is_component_break);
        let after = &rest[at + needle.len()..];
        let after_ok = after.chars().next().is_none_or(is_component_break);
        if before_ok && after_ok {
            redacted.push_str(&rest[..at]);
            redacted.push_str(replacement);
            rest = after;
        } else {
            let consumed = at + needle.len();
            redacted.push_str(&rest[..consumed]);
            rest = &rest[consumed..];
        }
    }
    redacted.push_str(rest);
    redacted
}

/// True for a character that cannot be part of the same path component.
fn is_component_break(c: char) -> bool {
    !c.is_alphanumeric() && c != '_' && c != '-' && c != '.'
}

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
    let home = std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    (path, variables, home)
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
    home: Option<PathBuf>,
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
        home: Option<PathBuf>,
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
    pub fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }

    /// Runs one probe. A probe that could not be started is not an error here:
    /// each caller decides what a missing tool means for the fact it is after.
    fn probe(&self, program: &str, argv: &[&str]) -> Result<Invocation, DomainError> {
        let spec = CommandSpec::new(
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

    /// The hardware architecture. `hw.optional.arm64` is asked first because
    /// it is the only one of the three that is true of the machine rather than
    /// of the process.
    fn detect_architecture(&self) -> Result<Architecture, DomainError> {
        if let Ok(inv) = self.sysctl("hw.optional.arm64") {
            if inv.status.success() && inv.stdout.trim() == "1" {
                return Ok(Architecture::Arm64);
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
                path: PathBuf::from(redact_home(path, self.home.as_deref())),
            }
        }
    }
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
        let home = self.home.as_deref();
        let path = self
            .path_entries()
            .iter()
            .map(|e| redact_home(e, home))
            .collect();
        let mut reported = BTreeMap::new();
        for (name, value) in &self.variables {
            if is_reported(name) {
                reported.insert(name.clone(), redact_home(value, home));
            }
        }
        Ok(EnvironmentSnapshot { path, reported })
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
            None,
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
    fn architecture_falls_back_to_uname_when_no_sysctl_answers() {
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
        let m = reader(invocations).machine().unwrap();
        assert_eq!(m.architecture, Architecture::X86_64);
        assert_eq!(m.macos, MacosVersion::new(10, 12, 6));
        assert_eq!(m.translation, Translation::Unknown);
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
            Some(PathBuf::from("/Users/ada")),
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
        assert_eq!(reader.home(), Some(Path::new("/Users/ada")));
    }

    #[test]
    fn every_variable_the_executor_strips_is_one_doctor_reports() {
        // The mirror of `process::environment::is_build_flag`. If that list
        // grows, this fails until the copy here grows too.
        for name in [
            "CPATH",
            "C_INCLUDE_PATH",
            "CPLUS_INCLUDE_PATH",
            "LIBRARY_PATH",
            "LD_LIBRARY_PATH",
            "PKG_CONFIG_PATH",
            "PKG_CONFIG_LIBDIR",
            "LDFLAGS",
            "CPPFLAGS",
            "CFLAGS",
            "CXXFLAGS",
            "OBJCFLAGS",
            "ACLOCAL_PATH",
            "CMAKE_PREFIX_PATH",
            "DYLD_LIBRARY_PATH",
            "DYLD_FALLBACK_FRAMEWORK_PATH",
            "MANPATH",
        ] {
            assert!(is_reported(name), "{name} is stripped but not reported");
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
        assert_eq!(redact_home("/Users/ada", Some(&home)), "~");
        assert_eq!(redact_home("/Users/ada/bin", Some(&home)), "~/bin");
        assert_eq!(redact_home("/usr/bin", Some(&home)), "/usr/bin");
        assert_eq!(redact_home("/Users/ada", None), "/Users/ada");
        // A longer name that merely starts with the home path is not it.
        assert_eq!(redact_home("/Users/adamant", Some(&home)), "/Users/adamant");
        assert_eq!(
            redact_home("/Users/adamant/Users/ada/x", Some(&home)),
            "/Users/adamant~/x"
        );
        // Embedded and repeated occurrences, which is what a compiler-flag
        // variable and a colon-separated list actually look like.
        assert_eq!(
            redact_home("-I/Users/ada/.local/include", Some(&home)),
            "-I~/.local/include"
        );
        assert_eq!(
            redact_home("/Users/ada/a:/opt/local/b:/Users/ada", Some(&home)),
            "~/a:/opt/local/b:~"
        );
        // A home of `/` would redact the whole filesystem; it is left alone.
        assert_eq!(redact_home("/usr/bin", Some(Path::new("/"))), "/usr/bin");
        assert_eq!(
            redact_home("/Users/ada/x", Some(Path::new("/Users/ada/"))),
            "~/x"
        );
    }

    #[test]
    fn the_user_name_is_redacted_even_outside_the_home_directory() {
        // 06-cli-and-ux.md names usernames separately from home paths, and
        // `scripts/capture-fixture.sh` already replaces `id -un`. A build
        // directory on another volume is where the two rules differ.
        let home = PathBuf::from("/Users/ada");
        assert_eq!(
            redact_home("-I/Volumes/build/ada/include", Some(&home)),
            "-I/Volumes/build/USER/include"
        );
        assert_eq!(redact_home("/opt/ada", Some(&home)), "/opt/USER");
        // A longer word that merely contains the name is left alone.
        assert_eq!(
            redact_home("/opt/adabas/lib", Some(&home)),
            "/opt/adabas/lib"
        );
        assert_eq!(redact_home("/opt/prada", Some(&home)), "/opt/prada");
        // The home path is still replaced first, so `~` wins where both apply.
        assert_eq!(redact_home("/Users/ada/ada", Some(&home)), "~/USER");
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
            None,
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
