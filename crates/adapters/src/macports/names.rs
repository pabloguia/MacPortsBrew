// SPDX-License-Identifier: Apache-2.0

//! Shape checks and bounds for text that comes back from `port`.
//!
//! Provider output is untrusted input (`04-migration-and-safety.md`, threat
//! model: "Malicious or malformed provider metadata"). Nothing parsed here
//! becomes a flag or a path without passing one of these checks first, and
//! every collection this adapter builds is bounded.

/// Cap on the bytes of one captured stream this adapter will parse. The
/// registry of a large installation is a few hundred kilobytes of text; a
/// stream larger than this is a sign of something other than `port` output.
pub const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

/// Cap on rows parsed from one stream.
pub const MAX_ROWS: usize = 8192;

/// Cap on the length of a port name.
pub const MAX_NAME_LEN: usize = 128;

/// Cap on a free-text field (description, licence) kept from provider
/// output.
///
/// The domain's number, not a second copy of it: `is_safe_provider_text`
/// enforces the same bound where a snapshot enters, so a value this parser
/// keeps is one that boundary accepts. Two constants holding the same number is
/// how they come to hold different ones.
pub const MAX_TEXT_LEN: usize = macportsbrew_domain::MAX_TEXT_LEN;

/// Cap on a path this adapter is willing to hand to `port` as an argument.
///
/// The check and its bound live beside the executor
/// ([`crate::process::is_safe_path`]) because the command line needs the same
/// rule for a prefix read out of a recording, and two copies of "safe path"
/// is one copy too many.
pub use crate::process::{is_safe_path, MAX_PATH_LEN};

/// True when `name` is a port name this adapter will pass as an argument.
///
/// The character set is `[A-Za-z0-9_.+-]`, which covers every port name in
/// the MacPorts tree, and a leading `-` is refused outright so that a name
/// can never be read by `port` as an option.
pub fn is_safe_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return false;
    }
    if name.starts_with('-') || name == "." || name == ".." {
        return false;
    }
    name.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'+' | b'-'))
}

/// Trims a free-text field and bounds it to at most [`MAX_TEXT_LEN`] bytes,
/// cutting at the last character boundary at or below the bound so that no
/// character is split.
///
/// The bound is in bytes because the snapshot boundary
/// (`is_safe_provider_text`) measures bytes: a cut counted in characters
/// keeps up to four times as many bytes for non-ASCII text, and the snapshot
/// would then refuse a value this parser had already accepted.
pub fn bounded_text(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed[..trimmed.floor_char_boundary(MAX_TEXT_LEN)].to_string()
}

/// Rejects a stream that is too large to be `port` output before parsing it.
pub fn within_output_bound(stream: &str) -> bool {
    stream.len() <= MAX_OUTPUT_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn names_that_could_become_flags_are_refused() {
        assert!(is_safe_name("wget"));
        assert!(is_safe_name("postgresql17-server"));
        assert!(is_safe_name("py312-pygments"));
        assert!(is_safe_name("openssl3"));
        assert!(is_safe_name("gtk2+"));
        assert!(!is_safe_name("-v"));
        assert!(!is_safe_name("--force"));
        assert!(!is_safe_name(""));
        assert!(!is_safe_name(".."));
        assert!(!is_safe_name("a b"));
        assert!(!is_safe_name("a;b"));
        assert!(!is_safe_name("a/b"));
        assert!(!is_safe_name(&"x".repeat(MAX_NAME_LEN + 1)));
    }

    #[test]
    fn paths_must_be_absolute_and_free_of_traversal() {
        assert!(is_safe_path(&PathBuf::from("/opt/local/bin/wget")));
        assert!(!is_safe_path(&PathBuf::from("relative/bin/wget")));
        assert!(!is_safe_path(&PathBuf::from("/opt/local/../etc/passwd")));
        assert!(!is_safe_path(&PathBuf::from("/opt/local/bin/\nwget")));
    }

    #[test]
    fn free_text_is_bounded() {
        assert_eq!(bounded_text("  hello  "), "hello");
        assert_eq!(
            bounded_text(&"a".repeat(MAX_TEXT_LEN + 10)).len(),
            MAX_TEXT_LEN
        );
        assert!(within_output_bound("short"));
    }

    #[test]
    fn free_text_is_bounded_in_bytes_and_never_splits_a_character() {
        // Synthetic text: a three-byte character repeated past the bound.
        // Counted in characters, the old cut kept three times the byte bound.
        let long = "\u{20ac}".repeat(MAX_TEXT_LEN);
        let cut = bounded_text(&long);
        assert!(cut.len() <= MAX_TEXT_LEN);
        assert_eq!(cut.len(), MAX_TEXT_LEN - MAX_TEXT_LEN % 3);
        assert!(cut.chars().all(|c| c == '\u{20ac}'));
        // A bound falling inside a character drops that character whole.
        let offset = format!("a{}", "\u{20ac}".repeat(MAX_TEXT_LEN));
        let cut = bounded_text(&offset);
        assert!(cut.len() <= MAX_TEXT_LEN);
        assert!(cut.len() > MAX_TEXT_LEN - 3);
        assert!(cut.starts_with('a'));
    }
}
