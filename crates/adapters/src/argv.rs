// SPDX-License-Identifier: Apache-2.0

//! Argument-vector helpers both provider adapters share.
//!
//! Each adapter checks every vector it builds against its own allowlist before
//! it reaches the process executor (`04-migration-and-safety.md`, "Process
//! execution"). The head match is one function here rather than a copy per
//! adapter, so the two allowlist checks cannot disagree about what "starts
//! with" means (`docs/passes/16-plans/F.md` §2, SR-7). It lives outside
//! `process/`, which is the spawn site and is not edited by adapter work.

/// What follows `head` in `argv`, when `argv` starts with exactly `head`,
/// element by element. A head is a prefix of elements, never of characters.
pub(crate) fn strip_head<'a>(argv: &'a [String], head: &[&str]) -> Option<&'a [String]> {
    (argv.len() >= head.len() && head.iter().zip(argv).all(|(a, b)| *a == b.as_str()))
        .then(|| &argv[head.len()..])
}

/// True when `argv` is exactly `head` followed by exactly one element that
/// passes `name_class`.
pub(crate) fn is_head_and_one_name(
    argv: &[String],
    head: &[&str],
    name_class: fn(&str) -> bool,
) -> bool {
    matches!(strip_head(argv, head), Some([name]) if name_class(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_head_is_matched_by_element_and_never_by_substring() {
        assert_eq!(
            strip_head(&v(&["-q", "contents", "xz"]), &["-q", "contents"]),
            Some(&v(&["xz"])[..])
        );
        assert_eq!(
            strip_head(&v(&["-q", "contents"]), &["-q", "contents"]),
            Some(&[][..])
        );
        assert_eq!(strip_head(&v(&["-q"]), &["-q", "contents"]), None);
        assert_eq!(
            strip_head(&v(&["-q", "contentsx", "xz"]), &["-q", "contents"]),
            None
        );
        assert_eq!(
            strip_head(&v(&["x", "-q", "contents"]), &["-q", "contents"]),
            None
        );
    }

    #[test]
    fn exactly_one_name_of_the_class() {
        let class = |s: &str| s.chars().all(|c| c.is_ascii_alphanumeric());
        let head = &["list", "--formula"];
        assert!(is_head_and_one_name(
            &v(&["list", "--formula", "xz"]),
            head,
            class
        ));
        assert!(!is_head_and_one_name(
            &v(&["list", "--formula"]),
            head,
            class
        ));
        assert!(!is_head_and_one_name(
            &v(&["list", "--formula", "xz", "jq"]),
            head,
            class
        ));
        assert!(!is_head_and_one_name(
            &v(&["list", "--formula", "x-z"]),
            head,
            class
        ));
        assert!(!is_head_and_one_name(&v(&["list", "xz"]), head, class));
    }
}
