// SPDX-License-Identifier: Apache-2.0

//! The lock query against a holder in **another process** (G item 20 N1).
//!
//! The probe's own tests hold a scratch lock through a second file description
//! of the same process. A provider's lock is held by another process, so this
//! file proves that case with the same code: it compiles the site itself and
//! re-executes this test binary as the holder, spawning no other program. Only
//! ever a scratch file this test made (INV-05): the holder is ignored in a
//! normal run and refuses any path but such a file. The sampling around the
//! query (the shape checks, the directory identity) is tested in
//! `lock_probe/mod.rs`.

#[path = "../src/lock_probe/getlk.rs"]
mod getlk;

use getlk::LockState;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const PATH_VARIABLE: &str = "MPB_TEST_LOCK_HOLDER_PATH";
const MODE_VARIABLE: &str = "MPB_TEST_LOCK_HOLDER_MODE";
const HOLDER: &str = "the_holder_process";
const READY: &str = "HOLDER-READY";
const SCRATCH_PREFIX: &str = "mpb-lock-cross-process-";

/// Whether `path` is the lock file [`scratch`] makes, and so safe to lock.
fn is_scratch_lock(path: &Path) -> bool {
    let directory = path.parent();
    path.file_name().is_some_and(|name| name == "lock")
        && directory.and_then(Path::parent) == Some(std::env::temp_dir().as_path())
        && directory
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(SCRATCH_PREFIX))
}

/// Not a test of its own, so a normal run never enters it. In the child it
/// takes the lock, says so, and holds it until its stdin closes.
#[test]
#[ignore = "the holder process, spawned by the tests below"]
fn the_holder_process() {
    let path = PathBuf::from(std::env::var_os(PATH_VARIABLE).expect("the scratch path"));
    assert!(is_scratch_lock(&path), "not a scratch lock: {path:?}");
    let file = File::open(path).expect("the holder opens the scratch file");
    match std::env::var(MODE_VARIABLE).as_deref() {
        Ok("exclusive") => file.lock().expect("the holder takes an exclusive lock"),
        Ok("shared") => file.lock_shared().expect("the holder takes a shared lock"),
        other => panic!("no such mode: {other:?}"),
    }
    println!("{READY}");
    let mut rest = Vec::new();
    std::io::stdin()
        .read_to_end(&mut rest)
        .expect("the holder waits for its stdin to close");
}

/// A scratch directory holding an empty lock file.
fn scratch(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("{SCRATCH_PREFIX}{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let lock = dir.join("lock");
    File::create(&lock).expect("a scratch lock file");
    (dir, lock)
}

/// A running holder. Its stdout stays open until it ends: libtest prints after
/// the test, and a closed pipe would fail the holder after it released.
struct Holder {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

/// Starts the holder and returns once it has the lock.
fn spawn_holder(lock: &Path, mode: &str) -> Holder {
    let mut child = Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", HOLDER, "--ignored", "--nocapture"])
        .env(PATH_VARIABLE, lock)
        .env(MODE_VARIABLE, mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the holder starts");
    let stdin = child.stdin.take().expect("the holder's stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("the holder's stdout"));
    let mut line = String::new();
    while !line.contains(READY) {
        line.clear();
        let read = stdout.read_line(&mut line).expect("the holder's output");
        assert!(read > 0, "the holder ended without taking the lock");
    }
    Holder {
        child,
        stdin,
        stdout,
    }
}

fn assert_seen_then_released(mode: &str) {
    let (dir, lock) = scratch(mode);
    let probe = File::open(&lock).expect("the probe opens the scratch file");
    assert_eq!(getlk::held(probe.as_fd()), LockState::NotHeld, "before");

    let Holder {
        mut child,
        stdin,
        mut stdout,
    } = spawn_holder(&lock, mode);
    assert_eq!(getlk::held(probe.as_fd()), LockState::Held, "{mode}");

    drop(stdin);
    let mut rest = Vec::new();
    stdout.read_to_end(&mut rest).expect("the holder's output");
    let status = child.wait().expect("the holder ends");
    assert!(status.success(), "the holder failed: {status}");
    assert_eq!(getlk::held(probe.as_fd()), LockState::NotHeld, "released");
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn an_exclusive_lock_held_by_another_process_is_held_until_it_ends() {
    assert_seen_then_released("exclusive");
}

#[test]
fn a_shared_lock_held_by_another_process_is_held_until_it_ends() {
    // The case `F_RDLCK` in the template would miss (R-17 observation C).
    assert_seen_then_released("shared");
}

#[test]
fn the_holder_refuses_anything_but_its_own_scratch_file() {
    let (dir, lock) = scratch("guard");
    assert!(is_scratch_lock(&lock));
    assert!(!is_scratch_lock(Path::new("/tmp/lock")));
    assert!(!is_scratch_lock(&dir.join("other")));
    assert!(!is_scratch_lock(Path::new(
        "/opt/local/var/macports/registry/.registry.lock"
    )));
    std::fs::remove_dir_all(dir).expect("cleanup");
}
