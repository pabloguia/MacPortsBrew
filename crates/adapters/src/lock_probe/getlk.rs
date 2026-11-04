// SPDX-License-Identifier: Apache-2.0

//! The one query: is a lock held on this open file right now?
//!
//! ADR-0007's single `unsafe` site, and the only one in the workspace
//! (`crates/cli/tests/unsafe_scan.rs` holds it to exactly one). The query is
//! R-17 observation C: `fcntl(F_GETLK)` with a whole-file write-lock template.
//! It reports a conflicting `flock` or `fcntl` lock held through any other
//! open file description, acquires nothing and changes nothing.

#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]

use std::os::fd::{AsRawFd, BorrowedFd};

/// What the kernel answered for one open file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LockState {
    /// A lock conflicting with a whole-file write lock is held: a shared or
    /// an exclusive one, by anyone else.
    Held,
    /// No conflicting lock is held at this instant.
    NotHeld,
    /// The call failed (any `-1`, whatever `errno`), or answered a lock type
    /// that is none of the three. Never read as either answer above.
    Unknown,
}

/// Asks whether any lock that would conflict with a whole-file write lock is
/// held on `fd`.
///
/// The template is ADR-0007's, exactly: `l_type = F_WRLCK` (a shared holder
/// conflicts with it; `F_RDLCK` would miss one, observation C), `l_whence =
/// SEEK_SET`, `l_start = 0`, `l_len = 0` (to the end of the file). `l_pid` is
/// set because the literal must be complete and is never read back (it is -1
/// for a `flock` holder).
///
/// Closing the descriptor afterwards would drop any POSIX record lock *this*
/// process holds on the file; it holds none, so the caller's close is
/// harmless.
#[allow(unsafe_code)]
pub(super) fn held(fd: BorrowedFd<'_>) -> LockState {
    let Ok(whence) = libc::c_short::try_from(libc::SEEK_SET) else {
        return LockState::Unknown;
    };
    let mut fl = libc::flock {
        l_start: 0,
        l_len: 0,
        l_pid: 0,
        l_type: libc::F_WRLCK,
        l_whence: whence,
    };
    let query: *mut libc::flock = &raw mut fl;
    // SAFETY: `libc::flock` is the kernel's `struct flock` for this target, so
    // the third, variadic argument is the pointer `F_GETLK` reads and writes
    // and nothing else. The kernel reads and writes that one struct during the
    // call and keeps no pointer to it past the return. `fl` is a fully
    // initialised local with no other borrow while `query` is live. The
    // `BorrowedFd` keeps the descriptor open for the duration of the call.
    let status = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETLK, query) };
    if status == -1 {
        return LockState::Unknown;
    }
    classify(fl.l_type)
}

/// What a returned `l_type` means: a closed set, anything else unknown.
fn classify(l_type: libc::c_short) -> LockState {
    match l_type {
        libc::F_UNLCK => LockState::NotHeld,
        libc::F_RDLCK | libc::F_WRLCK => LockState::Held,
        _ => LockState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::fd::AsFd;

    fn scratch(name: &str) -> (std::path::PathBuf, File) {
        let dir = std::env::temp_dir().join(format!("mpb-getlk-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("lock");
        let file = File::create(&path).expect("scratch file");
        (dir, file)
    }

    #[test]
    fn a_returned_lock_type_outside_the_three_is_unknown_either_way() {
        // Mutants 3 and 3b: an unexpected value read as held, or as not held.
        assert_eq!(classify(libc::F_UNLCK), LockState::NotHeld);
        assert_eq!(classify(libc::F_RDLCK), LockState::Held);
        assert_eq!(classify(libc::F_WRLCK), LockState::Held);
        assert_eq!(classify(0), LockState::Unknown);
        assert_eq!(classify(99), LockState::Unknown);
    }

    #[test]
    fn an_unlocked_file_is_not_held() {
        let (dir, _) = scratch("free");
        let probe = File::open(dir.join("lock")).expect("opens");
        assert_eq!(held(probe.as_fd()), LockState::NotHeld);
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn an_exclusive_flock_through_another_description_is_held() {
        let (dir, holder) = scratch("exclusive");
        holder.lock().expect("the test locks its own scratch file");
        let probe = File::open(dir.join("lock")).expect("opens");
        assert_eq!(held(probe.as_fd()), LockState::Held);
        holder.unlock().expect("unlocks");
        assert_eq!(held(probe.as_fd()), LockState::NotHeld);
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn a_shared_flock_through_another_description_is_held() {
        // The case `F_RDLCK` in the template would miss (observation C).
        let (dir, holder) = scratch("shared");
        holder
            .lock_shared()
            .expect("the test locks its own scratch file");
        let probe = File::open(dir.join("lock")).expect("opens");
        assert_eq!(held(probe.as_fd()), LockState::Held);
        // `unlock`, not `drop`: a test elsewhere in this binary that spawns a
        // process forks a copy of this descriptor until its `exec`, and a
        // `flock` lasts until every copy of the description is closed.
        holder.unlock().expect("unlocks");
        assert_eq!(held(probe.as_fd()), LockState::NotHeld);
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn a_descriptor_the_kernel_refuses_is_unknown_not_free() {
        // A pipe: `F_GETLK` fails on it, and a failure is never "not held"
        // (mutant 2, a -1 read as not held).
        let (reader, _writer) = std::io::pipe().expect("a pipe");
        assert_eq!(held(reader.as_fd()), LockState::Unknown);
    }

    #[test]
    fn a_directory_descriptor_answers_not_held_which_is_why_the_shape_is_checked_first() {
        // Observed on macOS 26.7.2, x86_64, 2026-11-03: the kernel answers
        // `F_UNLCK` for a directory, so `held` cannot tell a directory from a
        // free lock file; `fstat` on the handle must (mutant 5).
        let (dir, _) = scratch("dir");
        let handle = File::open(&dir).expect("a directory opens read-only");
        assert_eq!(held(handle.as_fd()), LockState::NotHeld);
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}
