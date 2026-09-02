// SPDX-License-Identifier: Apache-2.0

//! The real runner: `std::process::Command`, constructed here and nowhere
//! else. Timeout by polling with `try_wait`; cancellation through a shared
//! flag; output bounded; optional recording of every invocation for
//! fixture capture.

use super::{CommandSpec, ExitStatus, Invocation, ProcessRunner, RecordedInvocation};
use macportsbrew_domain::DomainError;
use std::cell::RefCell;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
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

fn read_bounded<R: Read>(mut reader: R, cap: usize) -> (String, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf.len() + n > cap {
                    let room = cap.saturating_sub(buf.len());
                    buf.extend_from_slice(&chunk[..room]);
                    truncated = true;
                    // Keep draining so the child does not block on a full
                    // pipe, but discard the excess.
                    while let Ok(m) = reader.read(&mut chunk) {
                        if m == 0 {
                            break;
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

impl ProcessRunner for SystemRunner {
    fn run(&self, spec: &CommandSpec) -> Result<Invocation, DomainError> {
        let start = Instant::now();
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
        let out_thread =
            thread::spawn(move || stdout.map_or((String::new(), false), |s| read_bounded(s, cap)));
        let err_thread =
            thread::spawn(move || stderr.map_or((String::new(), false), |s| read_bounded(s, cap)));

        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) => {}
                Err(e) => {
                    let _ = child.kill();
                    let (o, ot) = out_thread.join().unwrap_or_default();
                    let (r, rt) = err_thread.join().unwrap_or_default();
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
                let (o, ot) = out_thread.join().unwrap_or_default();
                let (r, rt) = err_thread.join().unwrap_or_default();
                return Ok(record(ExitStatus::Cancelled, o, r, ot || rt));
            }
            if start.elapsed() > spec.timeout {
                let _ = child.kill();
                let _ = child.wait();
                let (o, ot) = out_thread.join().unwrap_or_default();
                let (r, rt) = err_thread.join().unwrap_or_default();
                return Ok(record(ExitStatus::TimedOut, o, r, ot || rt));
            }
            thread::sleep(Duration::from_millis(10));
        };

        let (o, ot) = out_thread.join().unwrap_or_default();
        let (r, rt) = err_thread.join().unwrap_or_default();
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
    fn timeout_kills_the_child() {
        let runner = SystemRunner::new();
        let spec = CommandSpec::new("/bin/sleep", vec!["5".into()], env())
            .with_timeout(Duration::from_millis(200));
        let start = Instant::now();
        let inv = runner.run(&spec).unwrap();
        assert_eq!(inv.status, ExitStatus::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(3));
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
    fn non_zero_exit_is_reported_not_raised() {
        let runner = SystemRunner::new();
        let spec = CommandSpec::new("/usr/bin/false", vec![], env());
        let inv = runner.run(&spec).unwrap();
        assert_eq!(inv.status, ExitStatus::Exited { code: 1 });
        assert!(!inv.status.success());
        assert!(inv.require_success().is_err());
    }
}
