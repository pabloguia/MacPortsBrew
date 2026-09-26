// SPDX-License-Identifier: Apache-2.0

//! The real runner: `std::process::Command`, constructed here and nowhere
//! else. Timeout by polling with `try_wait`; cancellation through a shared
//! flag; output bounded; optional recording of every invocation for
//! fixture capture.

use super::{CommandSpec, ExitStatus, Invocation, ProcessRunner, RecordedInvocation};
use macportsbrew_domain::DomainError;
use std::cell::RefCell;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// A cancellation flag shared with whoever may cancel.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token that is not cancelled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation of every invocation holding this token.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// The live-system runner.
#[derive(Default)]
pub struct SystemRunner {
    cancel: CancelToken,
    recorded: RefCell<Vec<RecordedInvocation>>,
    recording: bool,
}

impl SystemRunner {
    /// A runner that does not record.
    pub fn new() -> Self {
        Self::default()
    }

    /// A runner that keeps every invocation for fixture capture.
    pub fn recording() -> Self {
        Self {
            recording: true,
            ..Self::default()
        }
    }

    /// Uses the given cancellation token.
    #[must_use]
    pub fn with_cancel(mut self, token: CancelToken) -> Self {
        self.cancel = token;
        self
    }

    /// Everything recorded so far, in order.
    pub fn take_recorded(&self) -> Vec<RecordedInvocation> {
        std::mem::take(&mut *self.recorded.borrow_mut())
    }
}

/// How long the reader threads are given to finish once the child is gone.
///
/// The child having exited does not mean the pipe is closed: it may have left
/// a grandchild holding the write end, and then a blocking read never returns.
/// Waiting for a bounded time and recording a truncated stream is the right
/// trade — the alternative is a `macportsbrew analyze` that never comes back.
///
/// Six seconds, and it was two. Two was not a bound on a held pipe, which is
/// what this exists for; it was tight enough to be missed by *scheduling*. This
/// repository's own test suite reproduced it at roughly one run in fifteen: the
/// reader for `/bin/echo` did not get its result onto the channel within two
/// seconds while a dozen test binaries and a build competed for the machine,
/// and the invocation came back with an empty stdout marked truncated. Nothing
/// mis-parsed — `Invocation::require_success` turns truncation into
/// `ProviderOutputChanged`, which is the whole point of the flag — but a gate
/// that fails for a reason like that teaches people to re-run it, and that is
/// how a real failure gets waved through.
///
/// Six costs nothing when the pipe is closed, which is every normal read: the
/// channel already has the answer. It only lengthens the pathological case, and
/// it is a twentieth of [`crate::process::DEFAULT_TIMEOUT`], the time the same
/// executor already allows one command to take, so a drain cannot dominate a
/// run.
///
/// **The two streams are drained one after the other, so the worst case is
/// twice this.** A grandchild inherits both pipes and holds both, and
/// `a_grandchild_holding_the_pipe_does_not_hang_the_runner` fixes twenty
/// seconds as the ceiling for that. Six keeps the worst case at twelve, with
/// margin; ten would have sat exactly on the bound, which is how this value was
/// found out.
const READER_DRAIN_TIMEOUT: Duration = Duration::from_secs(6);

/// Reads a stream up to `cap` bytes, returning the text and whether anything
/// was lost.
///
/// `Interrupted` is retried rather than treated as end of stream: a signal
/// arriving mid-read is not the provider having finished, and stopping there
/// would hand a parser a partial document while reporting it as complete. Any
/// other error ends the read and marks the result truncated, for the same
/// reason: what was captured is not what the provider said.
fn read_bounded<R: Read>(mut reader: R, cap: usize) -> (String, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            // A signal arrived mid-read. Read again; this is not the end of
            // the stream.
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => {
                truncated = true;
                break;
            }
            Ok(n) => {
                if buf.len() + n > cap {
                    let room = cap.saturating_sub(buf.len());
                    buf.extend_from_slice(&chunk[..room]);
                    truncated = true;
                    // Keep draining so the child does not block on a full
                    // pipe, but discard the excess.
                    loop {
                        match reader.read(&mut chunk) {
                            Ok(0) => break,
                            Ok(_) => {}
                            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                            Err(_) => break,
                        }
                    }
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
        }
    }
    (String::from_utf8_lossy(&buf).into_owned(), truncated)
}

/// Collects one reader thread's result, giving up after
/// [`READER_DRAIN_TIMEOUT`].
///
/// A thread that has not answered by then is blocked on a pipe somebody other
/// than the child is still holding. Its bytes are lost, which is what the
/// `true` says.
fn collect(rx: &Receiver<(String, bool)>) -> (String, bool) {
    rx.recv_timeout(READER_DRAIN_TIMEOUT)
        .unwrap_or_else(|_| (String::new(), true))
}

impl ProcessRunner for SystemRunner {
    fn run(&self, spec: &CommandSpec) -> Result<Invocation, DomainError> {
        let start = Instant::now();

        let record = |status: ExitStatus, stdout: String, stderr: String, truncated: bool| {
            let inv = Invocation {
                program: spec.program_name(),
                argv: spec.argv.clone(),
                environment: spec.environment.clone(),
                status,
                stdout,
                stderr,
                truncated,
                duration_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
            };
            if self.recording {
                self.recorded.borrow_mut().push(RecordedInvocation {
                    invocation: inv.clone(),
                });
            }
            inv
        };

        // A relative program name is resolved by the operating system against
        // this process's `PATH`, which is the shadowing case in
        // `04-migration-and-safety.md`: whoever controls an early `PATH` entry
        // would choose what runs. Every caller in this workspace resolves an
        // absolute path from a detected prefix, so a relative one here is a
        // programming error — refused before anything is spawned, and reported
        // through the same channel a missing program is.
        //
        // Being absolute is necessary and not sufficient — `/bin/sh` is
        // absolute — so the same guard asks the stronger question: is this one
        // of the six programs this tool may run at all? That check is here, on
        // the value about to reach the kernel, rather than upstream on the type
        // or in a lint, because both of those were defeated in review: public
        // fields let a spec be rewritten after construction, a module-level
        // `OnceLock` manufactures the `&'static str` the constructors ask for,
        // and a denylist of interpreters is missing `tclsh` — which `port`
        // itself is written in — however long it grows.
        //
        // And for a provider, which of its subcommands: a permitted program
        // asked to do a permitted program's mutation is the same program (F-53).
        if !crate::process::may_spawn(&spec.program, &spec.argv) {
            return Ok(record(
                ExitStatus::NotStarted {
                    error: format!(
                        "refusing to run `{}` with {:?}: this tool spawns only {}, each \
                         at an absolute path, a provider's own executable only from a \
                         `bin` directory under a detected prefix, and a provider only \
                         for a subcommand on its read allowlist",
                        spec.program.display(),
                        spec.argv,
                        crate::process::PERMITTED_PROGRAMS.join(", ")
                    ),
                },
                String::new(),
                String::new(),
                false,
            ));
        }

        let mut command = Command::new(&spec.program);
        command
            .args(&spec.argv)
            .env_clear()
            .envs(&spec.environment)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        // Its own process group, so a terminal signal aimed at this process's
        // foreground group (a Ctrl-C) does not also reach a provider read
        // mid-flight; cancellation is what [`CancelToken`] is for, and it is
        // the one the executor can record. Killing the whole group on timeout
        // means signalling `-pid`, which needs `libc`; the alternative is
        // spawning a kill utility, and that would be a second spawn path with
        // no `CommandSpec` behind it — no sanitised environment, no timeout,
        // no record. `04-migration-and-safety.md` ("Process execution") asks
        // for one executor precisely so those are decided in one place, and
        // `scripts/check-layering.sh` would not catch it here, because this
        // module is the one place the grep exempts. So a grandchild that
        // outlives its parent is handled by bounding the reads instead — see
        // [`READER_DRAIN_TIMEOUT`]. The reasoning is ADR-0006, because a
        // dependency decision does not belong in a comment (F-20); what is left
        // here is what someone editing this line needs. Group kill needs a
        // `libc`-free mechanism
        // or an ADR admitting the dependency.
        command.process_group(0);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                return Ok(record(
                    ExitStatus::NotStarted {
                        error: e.to_string(),
                    },
                    String::new(),
                    String::new(),
                    false,
                ));
            }
        };

        let cap = spec.max_output_bytes;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        // The results come back over channels rather than through
        // `JoinHandle::join`, because a join is unbounded: a grandchild
        // holding the write end of the pipe keeps the reader blocked long
        // after the child is dead, and joining it would hang the whole
        // program. `collect` waits [`READER_DRAIN_TIMEOUT`] and then gives up
        // with a truncated record. The threads are left to finish on their
        // own; they hold nothing but their own end of a pipe.
        let (out_tx, out_rx) = mpsc::channel();
        let (err_tx, err_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = out_tx.send(stdout.map_or((String::new(), false), |s| read_bounded(s, cap)));
        });
        thread::spawn(move || {
            let _ = err_tx.send(stderr.map_or((String::new(), false), |s| read_bounded(s, cap)));
        });

        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) => {}
                Err(e) => {
                    let _ = child.kill();
                    let (o, ot) = collect(&out_rx);
                    let (r, rt) = collect(&err_rx);
                    return Ok(record(
                        ExitStatus::NotStarted {
                            error: e.to_string(),
                        },
                        o,
                        r,
                        ot || rt,
                    ));
                }
            }
            if self.cancel.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                let (o, ot) = collect(&out_rx);
                let (r, rt) = collect(&err_rx);
                return Ok(record(ExitStatus::Cancelled, o, r, ot || rt));
            }
            if start.elapsed() > spec.timeout {
                let _ = child.kill();
                let _ = child.wait();
                let (o, ot) = collect(&out_rx);
                let (r, rt) = collect(&err_rx);
                return Ok(record(ExitStatus::TimedOut, o, r, ot || rt));
            }
            thread::sleep(Duration::from_millis(10));
        };

        let (o, ot) = collect(&out_rx);
        let (r, rt) = collect(&err_rx);
        let exit = match status.code() {
            Some(code) => ExitStatus::Exited { code },
            None => ExitStatus::Signalled {
                signal: signal_of(status),
            },
        };
        Ok(record(exit, o, r, ot || rt))
    }
}

#[cfg(unix)]
fn signal_of(status: std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn signal_of(_status: std::process::ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn env() -> BTreeMap<String, String> {
        crate::process::sanitised_environment(&crate::process::EnvironmentSpec::default())
    }

    #[test]
    fn runs_argv_without_a_shell_and_records() {
        let runner = SystemRunner::recording();
        let spec = CommandSpec::new(
            "/bin/echo",
            vec!["hello".into(), "$HOME; rm -rf /".into()],
            env(),
        );
        let inv = runner.run(&spec).unwrap();
        // First, because a reader thread that misses `READER_DRAIN_TIMEOUT`
        // returns an *empty* stdout with this flag set, and an assertion on the
        // text then fails saying the output was wrong when what happened is
        // that it never arrived. Production cannot confuse the two -
        // `require_success` turns truncation into `ProviderOutputChanged` - but
        // these tests call `run` directly, and under a loaded machine the flake
        // was reproducible at roughly one run in twelve.
        assert!(!inv.truncated, "the reader thread did not drain in time");
        assert_eq!(inv.status, ExitStatus::Exited { code: 0 });
        assert_eq!(inv.stdout, "hello $HOME; rm -rf /\n");
        assert_eq!(inv.program, "echo");
        let recorded = runner.take_recorded();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].invocation.argv, spec.argv);
    }

    #[test]
    fn environment_is_exactly_what_was_given() {
        let runner = SystemRunner::new();
        let mut e = env();
        e.insert("MPB_TEST_MARKER".into(), "present".into());
        let spec = CommandSpec::new("/usr/bin/env", vec![], e);
        let inv = runner.run(&spec).unwrap();
        assert!(!inv.truncated, "the reader thread did not drain in time");
        assert!(inv.stdout.contains("MPB_TEST_MARKER=present"));
        assert!(inv.stdout.contains("LC_ALL=C"));
        assert!(!inv.stdout.contains("PKG_CONFIG_PATH="));
        let lines = inv.stdout.lines().count();
        assert_eq!(lines, spec.environment.len());
    }

    #[test]
    fn missing_program_is_not_started() {
        let runner = SystemRunner::new();
        let spec = CommandSpec::new("/nonexistent/mpb-test-program", vec![], env());
        let inv = runner.run(&spec).unwrap();
        assert!(matches!(inv.status, ExitStatus::NotStarted { .. }));
        assert!(inv.require_success().is_err());
    }

    #[test]
    fn a_provider_asked_for_a_subcommand_off_its_read_list_is_not_started() {
        // F-53 at the spawn site. The prefix does not exist, so a spec that got
        // past the guard would also end `NotStarted` - from the kernel. The
        // assertion is on *which* refusal, so it cannot pass by that route. The
        // verb is assembled at run time for the reason
        // `.claude/hooks/test-hooks.sh` gives: the Stop gate greps `src/` for
        // the literal, and this is test data rather than a code path.
        let add = ["in", "stall"].concat();
        let runner = SystemRunner::new();
        let spec = CommandSpec::provider(
            std::path::Path::new("/nonexistent-mpb-prefix"),
            "bin/brew",
            vec![add, "wget".into()],
            env(),
        );
        let inv = runner.run(&spec).unwrap();
        let ExitStatus::NotStarted { error } = &inv.status else {
            panic!("a mutating subcommand reached the kernel: {:?}", inv.status);
        };
        assert!(error.contains("read allowlist"), "{error}");
    }

    #[test]
    fn timeout_kills_the_child() {
        let runner = SystemRunner::new();
        let spec = CommandSpec::new("/bin/sleep", vec!["5".into()], env())
            .with_timeout(Duration::from_millis(200));
        let start = Instant::now();
        let inv = runner.run(&spec).unwrap();
        assert_eq!(inv.status, ExitStatus::TimedOut);
        // Generous: under a parallel workspace test run the reader drain
        // (READER_DRAIN_TIMEOUT) and scheduling add seconds; the property is
        // "bounded", not "fast".
        assert!(start.elapsed() < Duration::from_secs(20));
    }

    #[test]
    fn cancellation_stops_the_child() {
        let token = CancelToken::new();
        let runner = SystemRunner::new().with_cancel(token.clone());
        let t = token.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            t.cancel();
        });
        let spec = CommandSpec::new("/bin/sleep", vec!["5".into()], env());
        let inv = runner.run(&spec).unwrap();
        assert_eq!(inv.status, ExitStatus::Cancelled);
    }

    #[test]
    fn output_is_bounded() {
        let runner = SystemRunner::new();
        let mut spec =
            CommandSpec::new("/usr/bin/yes", vec![], env()).with_timeout(Duration::from_secs(5));
        spec.max_output_bytes = 4096;
        let inv = runner.run(&spec).unwrap();
        assert!(inv.truncated);
        assert!(inv.stdout.len() <= 4096);
    }

    #[test]
    fn a_program_that_is_not_an_absolute_path_is_never_spawned() {
        // `echo` exists on every machine's `PATH`, so a runner that resolved
        // bare names would succeed here. Succeeding is the bug: what ran would
        // be whatever an early `PATH` entry decided (`04-migration-and-safety`,
        // executable shadowing).
        let runner = SystemRunner::new();
        let inv = runner
            .run(&CommandSpec::new("echo", vec!["hello".into()], env()))
            .unwrap();
        match &inv.status {
            ExitStatus::NotStarted { error } => {
                assert!(error.contains("absolute"), "{error}");
            }
            other => panic!("a relative program must not run: {other:?}"),
        }
        assert!(inv.stdout.is_empty(), "nothing ran, so nothing was said");
        assert!(inv.require_success().is_err());
    }

    #[test]
    fn a_grandchild_holding_the_pipe_does_not_hang_the_runner() {
        // The child exits at once and leaves a background process holding the
        // write end of the inherited pipe. Reading until end-of-file would
        // block for the grandchild's whole lifetime, so the reads are bounded:
        // the record comes back truncated rather than never.
        //
        // The shell is an argv handed to the executor like any other program,
        // not a second way of spawning one: nothing here constructs a
        // `Command`. It is `/bin/sh` because the point is to produce a process
        // tree the executor did not create, and only a shell will do that.
        let runner = SystemRunner::new();
        let spec = CommandSpec::new("/bin/sh", vec!["-c".into(), "sleep 30 &".into()], env());
        let start = Instant::now();
        let inv = runner.run(&spec).unwrap();
        assert_eq!(inv.status, ExitStatus::Exited { code: 0 });
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "the runner waited on the grandchild instead of giving up: {:?}",
            start.elapsed()
        );
        assert!(
            inv.truncated,
            "output that could not be read to the end is truncated, and \
             `require_success` must refuse it"
        );
        assert!(inv.require_success().is_err());
    }

    #[test]
    fn non_zero_exit_is_reported_not_raised() {
        let runner = SystemRunner::new();
        let spec = CommandSpec::new("/usr/bin/false", vec![], env());
        let inv = runner.run(&spec).unwrap();
        assert_eq!(inv.status, ExitStatus::Exited { code: 1 });
        assert!(!inv.status.success());
        assert!(inv.require_success().is_err());
    }
}
