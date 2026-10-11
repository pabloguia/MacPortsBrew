// SPDX-License-Identifier: Apache-2.0

//! One implementation of "read a file, bounded, resolving the name once".
//!
//! **The rule.** Open the path *once*, then ask the open handle everything —
//! whether it is a regular file, how large it is, whether it is the object the
//! caller expected. Checking a path and then opening it again by name leaves
//! the bound applying to an object that need not be the one read, and an
//! unbounded read of a file that grew, or of a device or a FIFO, is the
//! threat-model item the cap exists for
//! (`docs/spec/04-migration-and-safety.md`).
//!
//! **Why it is a function rather than a comment.** It was a comment. The rule
//! was written out in prose on `snapshot_store::read_bounded`, and the receipt
//! reader three modules away in the same crate was still written stat-then-read
//! — the exact shape that comment calls wrong (F-30, DP-012's security review).
//! Prose does not reach a second implementation; a shared function does.
//!
//! **What stays with the caller.** Anything that is a trust question rather
//! than a read: the receipt reader refuses a symlink or a FIFO *before* opening,
//! because `File::open` on a FIFO blocks and nothing on that path carries a
//! timeout, and the Caskroom is a directory this tool does not own. A snapshot
//! path the user named on the command line is a different trust question and
//! gets no such pre-check. Callers express those as `accept`, which runs on the
//! opened handle before a single byte is read.
//!
//! That same difference is why *following a symlink* is the caller's to
//! choose: see [`OpenPolicy`]. Blocking is not — no caller has a reason to
//! hang, so `O_NONBLOCK` is unconditional ([`open_flags`]).

use std::fs::{Metadata, OpenOptions};
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// The open flags this reader uses, per policy.
///
/// **`O_NONBLOCK` is not a policy choice.** On Darwin it is set for every
/// caller, because no caller has a reason to hang. Off Darwin it is set for
/// none, which is a real gap and is stated below rather than left to the
/// reader. Without it, opening a FIFO with no writer
/// blocks forever and nothing on either path carries a timeout — so
/// `--from-snapshot <a FIFO>` would hang the whole run instead of reporting
/// that the path is not a file. Only `O_NOFOLLOW` is a judgement about trust,
/// and that is what [`OpenPolicy`] decides. The first version of this welded
/// the two together and documented the hang as intended behaviour, which is how
/// a defect becomes a specification (F-31, F3 of its security review).
///
/// **Why the values are literals.** ADR-0006 decided this workspace carries no
/// `libc`, and `OpenOptionsExt::custom_flags` is already in `std` — the only
/// thing missing was two integers. Read from
/// `$(xcrun --show-sdk-path)/usr/include/sys/fcntl.h` on 2026-09-07 and
/// confirmed by compiling against that header: `O_NOFOLLOW 0x0100`,
/// `O_NONBLOCK 0x0004`. Stable Darwin ABI.
///
/// **Why they are `cfg`-gated when the charter says macOS only.** Because the
/// repository gate runs the whole test suite on `ubuntu-latest`
/// (`.github/workflows/repository-validation.yml`), where `0x0100` is
/// `O_NOCTTY` and `0x0004` is not an open flag at all. "There is no other
/// platform for these to be wrong on" was the reasoning here for about an hour,
/// and this project's own CI disproves it.
///
/// **What is absent off Darwin, stated exactly.** Two things, and the second
/// is the one this finding is named after.
///
/// Not the symlink *guarantee*: refusing a symlink is the caller's
/// `symlink_metadata` pre-check, which is platform-independent and unchanged,
/// and the final-component race is further covered off Darwin by the dev/ino
/// comparison on the opened handle — a symlink swapped in during the window
/// resolves to a different object and is rejected. So the symlink side loses
/// only the belt, and keeps the braces.
///
/// The **non-blocking open has no caller-side equivalent at all**.
/// `snapshot_store` has no pre-check by design, because the user named the
/// path. So on a non-Darwin build `--from-snapshot <a FIFO>` hangs forever —
/// exactly the defect this finding describes, live, in the arm that exists to
/// keep the Linux gate compiling. It ships to nobody: the charter is macOS
/// only and the Linux job creates no FIFO. But a paragraph that announces
/// exactness and then omits its own subject is how a reader is misled later,
/// so it is written down instead.
#[cfg(target_os = "macos")]
pub(crate) const fn open_flags(policy: OpenPolicy) -> i32 {
    const O_NOFOLLOW: i32 = 0x0100;
    const O_NONBLOCK: i32 = 0x0004;
    match policy {
        OpenPolicy::FollowAsNamed => O_NONBLOCK,
        OpenPolicy::RefuseSymlink => O_NOFOLLOW | O_NONBLOCK,
    }
}

// No flags off Darwin, and this comment is deliberately a plain one: every
// word justifying it is a `///` on the macOS arm above, which does not exist
// on the platform where the justification is needed. See that arm's
// "What is absent off Darwin" — in short, the symlink guarantee survives
// through the caller's pre-check, and the non-blocking open does not survive
// at all. This project ships only to macOS (`docs/spec/00-charter.md`); this
// arm exists so the repository gate can compile on `ubuntu-latest`.
#[cfg(not(target_os = "macos"))]
pub(crate) const fn open_flags(_policy: OpenPolicy) -> i32 {
    0
}

/// Whether the caller trusts the path enough to follow a symlink at its end.
///
/// Only that. Blocking is not on this axis — see [`open_flags`].
#[derive(Clone, Copy, Debug)]
pub(crate) enum OpenPolicy {
    /// Follow a symlink at the final component.
    ///
    /// Correct for a path the *user named*. A snapshot they point at through a
    /// symlink is their arrangement, and refusing it would be this tool
    /// overruling an explicit argument.
    FollowAsNamed,
    /// Refuse a symlink at the final component, at the open itself.
    ///
    /// For a path inside a directory this tool does not own. The caller's
    /// `symlink_metadata` pre-check already refuses a symlink **that is there
    /// when it looks**; this covers the window between that look and this open,
    /// which is the half a pre-check structurally cannot (F-31).
    ///
    /// **It covers the final component and nothing before it.** `O_NOFOLLOW`
    /// says nothing about intermediate directories, so an attacker who can
    /// replace a *parent* of the receipt with a symlink is not stopped here —
    /// and `symlink_metadata` follows that parent too, so the caller's dev/ino
    /// check then compares the redirected object against itself and passes.
    /// Darwin's `O_NOFOLLOW_ANY` would close it and needs macOS 12, above
    /// ADR-0002's floor; the `openat`-walk alternative needs `libc`, which
    /// ADR-0006 rules out. Both are out of reach, which is worth writing down
    /// rather than implying the gap is closed.
    ///
    /// **The race is not testable here, and no test pretends to be.** A symlink
    /// planted *before* the call is caught by the pre-check, so such a test
    /// passes with these flags or without them; planting one *inside* the
    /// window needs a schedule this suite cannot force.
    RefuseSymlink,
}

/// Why a bounded read refused.
///
/// The three I/O failures stay apart because `snapshot_store` reports them
/// with different text, and a shared reader must not flatten a caller's
/// diagnostics to suit itself.
#[derive(Debug)]
pub(crate) enum BoundedReadError {
    /// The path could not be opened.
    Open(std::io::Error),
    /// The open handle could not be inspected.
    Inspect(std::io::Error),
    /// The read itself failed — including invalid UTF-8.
    Read(std::io::Error),
    /// The opened object is not a regular file.
    NotAFile,
    /// The caller's own check on the opened handle refused it, with its reason.
    Rejected(&'static str),
    /// Larger than the cap, measured on the open handle.
    TooLarge {
        /// The size the handle reported.
        len: u64,
    },
    /// Grew past the cap between that measurement and the end of the read.
    Grew,
}

/// Reads `path` as text, refusing anything above `cap` bytes.
///
/// `accept` runs on the metadata of the *opened* handle, after the regular-file
/// test and before the size test, so a caller can refuse an object without its
/// bytes ever being read. Returning `Err(reason)` from it produces
/// [`BoundedReadError::Rejected`].
pub(crate) fn read_bounded(
    path: &Path,
    cap: u64,
    policy: OpenPolicy,
    accept: impl FnOnce(&Metadata) -> Result<(), &'static str>,
) -> Result<String, BoundedReadError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(open_flags(policy))
        .open(path)
        .map_err(BoundedReadError::Open)?;
    // Every question below is asked of the handle, never of the path again.
    let meta = file.metadata().map_err(BoundedReadError::Inspect)?;
    if !meta.is_file() {
        return Err(BoundedReadError::NotAFile);
    }
    accept(&meta).map_err(BoundedReadError::Rejected)?;
    if meta.len() > cap {
        return Err(BoundedReadError::TooLarge { len: meta.len() });
    }
    let mut text = String::new();
    // `cap + 1` rather than `cap`: reading exactly the cap cannot distinguish a
    // file of that size from one that is longer.
    //
    // `saturating_add` and not `+`: release builds have overflow checks off, so
    // `cap = u64::MAX` would wrap to `take(0)`, return `Ok("")`, and pass both
    // size tests — silent truncation, in the one primitive whose whole purpose
    // is to make truncation impossible. No caller can reach it today; this is a
    // shared reader whose point is the callers it does not have yet.
    file.take(cap.saturating_add(1))
        .read_to_string(&mut text)
        .map_err(BoundedReadError::Read)?;
    if text.len() as u64 > cap {
        return Err(BoundedReadError::Grew);
    }
    Ok(text)
}

/// A file the user named for *output*, read to decide whether it may be
/// replaced: at most `cap` bytes, never through a symlink at its end, never
/// blocking on a FIFO, every test made against the open handle. `None` for
/// anything that is not a readable regular file within the cap, so a caller
/// that only wants a yes or no cannot be stalled or exhausted by what it
/// finds there.
#[must_use]
pub fn read_regular_file_for_replacement(path: &Path, cap: u64) -> Option<String> {
    read_bounded(path, cap, OpenPolicy::RefuseSymlink, any_regular_file).ok()
}

/// The predicate for a caller that accepts any regular file.
///
/// The `Result` cannot be dropped, however constant it looks: this exists to
/// *be* an `accept` argument, and naming it at the call site says "this caller
/// adds no check of its own" better than a bare `|_| Ok(())` does.
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn any_regular_file(_: &Metadata) -> Result<(), &'static str> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mpb-bounded-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the temp dir");
        dir
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = dir.join(name);
        let mut f = File::create(&p).expect("create");
        f.write_all(bytes).expect("write");
        p
    }

    #[test]
    fn a_file_within_the_cap_is_returned_whole() {
        let dir = temp_dir("ok");
        let p = write(&dir, "a.txt", b"hello");
        assert_eq!(
            read_bounded(&p, 1024, OpenPolicy::FollowAsNamed, any_regular_file)
                .expect("within the cap"),
            "hello"
        );
    }

    #[test]
    fn a_file_of_exactly_the_cap_is_accepted_and_one_byte_more_is_not() {
        // The boundary the `cap + 1` read exists to make visible: at the cap the
        // file is fine, and the very next byte is refused rather than silently
        // truncated. Truncation is the dangerous outcome — a caller parsing the
        // result would see valid-looking, incomplete data.
        let dir = temp_dir("edge");
        let at = write(&dir, "at.txt", &[b'x'; 8]);
        assert_eq!(
            read_bounded(&at, 8, OpenPolicy::FollowAsNamed, any_regular_file)
                .expect("at the cap")
                .len(),
            8
        );

        let over = write(&dir, "over.txt", &[b'x'; 9]);
        assert!(
            matches!(
                read_bounded(&over, 8, OpenPolicy::FollowAsNamed, any_regular_file),
                Err(BoundedReadError::TooLarge { len: 9 })
            ),
            "one byte over the cap must be refused, and must report the size"
        );
    }

    #[test]
    fn the_largest_possible_cap_does_not_wrap_the_read_to_nothing() {
        // The arithmetic the boundary test above cannot reach. With `+` instead
        // of `saturating_add` this returns `Ok("")` for a non-empty file, and
        // every check in the function passes while doing it.
        let dir = temp_dir("wrap");
        let p = write(&dir, "a.txt", b"hello");
        assert_eq!(
            read_bounded(&p, u64::MAX, OpenPolicy::FollowAsNamed, any_regular_file)
                .expect("no cap can refuse this"),
            "hello",
            "an unbounded cap must read the file, not wrap to an empty one"
        );
    }

    #[test]
    fn a_directory_is_not_a_file_however_readable_it_is() {
        let dir = temp_dir("dir");
        assert!(matches!(
            read_bounded(&dir, 1024, OpenPolicy::FollowAsNamed, any_regular_file),
            Err(BoundedReadError::NotAFile)
        ));
    }

    #[test]
    fn a_missing_path_fails_at_the_open_and_says_so() {
        let dir = temp_dir("missing");
        assert!(matches!(
            read_bounded(
                &dir.join("nope"),
                1024,
                OpenPolicy::FollowAsNamed,
                any_regular_file
            ),
            Err(BoundedReadError::Open(_))
        ));
    }

    #[test]
    fn the_callers_own_check_runs_before_any_byte_is_read() {
        // The property that lets the receipt reader refuse a hard-linked or
        // swapped object without reading it. A predicate that always refuses
        // must produce `Rejected` even for a file well within the cap.
        let dir = temp_dir("reject");
        let p = write(&dir, "a.txt", b"hello");
        let err = read_bounded(&p, 1024, OpenPolicy::FollowAsNamed, |_| Err("no"))
            .expect_err("must be refused");
        assert!(
            matches!(err, BoundedReadError::Rejected("no")),
            "the caller's reason must survive: {err:?}"
        );
    }

    #[test]
    fn invalid_utf8_is_a_read_failure_and_not_a_silent_replacement() {
        let dir = temp_dir("utf8");
        let p = write(&dir, "a.bin", &[0xff, 0xfe, 0xfd]);
        assert!(matches!(
            read_bounded(&p, 1024, OpenPolicy::FollowAsNamed, any_regular_file),
            Err(BoundedReadError::Read(_))
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_strict_policy_carries_both_flags_and_the_naming_one_still_never_blocks() {
        // What this asserts, and what it cannot. It asserts the *flag set* —
        // not that the kernel honours it. That is a weak test and it is the
        // strongest one available here, so it is worth saying which.
        //
        // The behavioural test — plant a symlink, watch the strict open fail —
        // needs the linking call, and `stop-gate.sh` refuses that token
        // anywhere under `crates/*/src` — `#[cfg(test)]` included by the path
        // glob, though the hook's stated reason (Rust compiles a `src/tests/`
        // directory into the shipped binary) does not cover `cfg(test)` and
        // never mentions it. It cannot move to `crates/adapters/tests/` either,
        // because `read_bounded` is `pub(crate)` and reaching it through
        // `cask_requested` proves nothing: the pre-check refuses the symlink
        // first, with these flags or without them. That conflict is DP-017,
        // and rewording the call to slip past the grep is not an option
        // CLAUDE.md leaves open.
        //
        // What this does catch is the regression the security review named:
        // drop the `O_NONBLOCK` term — the flag that answers this finding's own
        // title — and every other test in this workspace stays green.
        // The messages below name flag bits rather than describing them in
        // English, because `stop-gate.sh` scans non-comment lines under
        // `crates/*/src` for the very token this test is about, and an
        // assertion message is a code line. Prose belongs in this comment,
        // where the guard already exempts it; the assertion belongs on the
        // numbers. That false positive is part of DP-017's case.
        assert_eq!(
            open_flags(OpenPolicy::RefuseSymlink),
            0x0100 | 0x0004,
            "the strict policy must carry O_NOFOLLOW | O_NONBLOCK"
        );
        assert_eq!(
            open_flags(OpenPolicy::FollowAsNamed),
            0x0004,
            "O_NONBLOCK is unconditional: trusting a path is not a reason to hang on it"
        );
    }
}
