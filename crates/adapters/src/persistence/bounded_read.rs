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

use std::fs::{File, Metadata};
use std::io::Read;
use std::path::Path;

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
    accept: impl FnOnce(&Metadata) -> Result<(), &'static str>,
) -> Result<String, BoundedReadError> {
    let file = File::open(path).map_err(BoundedReadError::Open)?;
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
            read_bounded(&p, 1024, any_regular_file).expect("within the cap"),
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
            read_bounded(&at, 8, any_regular_file)
                .expect("at the cap")
                .len(),
            8
        );

        let over = write(&dir, "over.txt", &[b'x'; 9]);
        assert!(
            matches!(
                read_bounded(&over, 8, any_regular_file),
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
            read_bounded(&p, u64::MAX, any_regular_file).expect("no cap can refuse this"),
            "hello",
            "an unbounded cap must read the file, not wrap to an empty one"
        );
    }

    #[test]
    fn a_directory_is_not_a_file_however_readable_it_is() {
        let dir = temp_dir("dir");
        assert!(matches!(
            read_bounded(&dir, 1024, any_regular_file),
            Err(BoundedReadError::NotAFile)
        ));
    }

    #[test]
    fn a_missing_path_fails_at_the_open_and_says_so() {
        let dir = temp_dir("missing");
        assert!(matches!(
            read_bounded(&dir.join("nope"), 1024, any_regular_file),
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
        let err = read_bounded(&p, 1024, |_| Err("no")).expect_err("must be refused");
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
            read_bounded(&p, 1024, any_regular_file),
            Err(BoundedReadError::Read(_))
        ));
    }
}
