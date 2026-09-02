// SPDX-License-Identifier: Apache-2.0

//! Shape checks and bounds for text that comes back from `port`.
//!
//! Provider output is untrusted input (`04-migration-and-safety.md`, threat
//! model: "Malicious or malformed provider metadata"). Nothing parsed here
//! becomes a flag or a path without passing one of these checks first, and
//! every collection this adapter builds is bounded.

use std::path::Path;

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
pub const MAX_TEXT_LEN: usize = 4096;

/// Cap on a path this adapter is willing to hand to `port` as an argument.
pub const MAX_PATH_LEN: usize = 4096;

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

/// True when `path` is one this adapter will pass to `port` as an argument.
///
/// Absolute, bounded, free of control characters and of `..` components. The
/// executor takes argv arrays, so quoting is not the concern; a relative or
/// traversing path reaching a provider query is.
pub fn is_safe_path(path: &Path) -> bool {
    let Some(text) = path.to_str() else {
        return false;
    };
    if text.len() > MAX_PATH_LEN || !text.starts_with('/') {
        return false;
    }
    if text.chars().any(char::is_control) {
        return false;
    }
    !path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

/// Trims a free-text field and bounds it, keeping whole characters.
pub fn bounded_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() <= MAX_TEXT_LEN {
        return trimmed.to_string();
    }
    trimmed.chars().take(MAX_TEXT_LEN).collect()
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
}
