// SPDX-License-Identifier: Apache-2.0

//! What `macportsbrew --version` prints (pass 09, D-V1).
//!
//! `-V` stays `macportsbrew <version>`, which scripts parse. `--version` adds
//! the policy data version, the target and the commit, so a bug report or a
//! release record names exactly what ran:
//!
//! ```text
//! macportsbrew 0.1.0
//! policy 2026.10.03-1
//! target x86_64-apple-darwin
//! commit <40-hex>
//! build <n>            (only when CI gave the build a number)
//! ```
//!
//! The block is a fixed, machine-read English contract (the release job reads
//! it), which is why it stays outside the i18n catalogue; the exemption is
//! recorded in `docs/spec-evolution.md`. The target and commit come from
//! `MPB_BUILD_TARGET` and `MPB_BUILD_COMMIT` at compile time, which the release
//! script sets; a developer build prints the target its architecture implies
//! and says it is not a release build. No build script is involved.
//!
//! **The build number** is the CI run's sequential integer, which never repeats
//! and is not part of the semantic version (the version stays in `Cargo.toml`).
//! It arrives as `MPB_BUILD_NUMBER` at compile time and is checked **at compile
//! time**: a value that is not a positive decimal integer without a leading
//! zero stops the build, because a release stamped with a malformed number would
//! say something false about itself. A build given none prints no `build` line,
//! so the block keeps its four lines and the release job's per-line checks are
//! unaffected by a fifth.

use macportsbrew_policy::PolicyData;
use std::sync::OnceLock;

/// The commit line of a build that was not given one.
pub const NOT_A_RELEASE_BUILD: &str = "unknown (not a release build)";

/// The target triple this binary was built for.
#[must_use]
pub fn target() -> String {
    option_env!("MPB_BUILD_TARGET").map_or_else(
        || format!("{}-apple-darwin", std::env::consts::ARCH),
        str::to_string,
    )
}

/// The commit this binary was built from, or [`NOT_A_RELEASE_BUILD`].
#[must_use]
pub fn commit() -> &'static str {
    option_env!("MPB_BUILD_COMMIT").unwrap_or(NOT_A_RELEASE_BUILD)
}

/// Parses a build number: a positive decimal integer, no sign, no leading
/// zero, no separators. `const` so a bad value fails the compile.
///
/// # Panics
///
/// On anything else. In a `const` context that is a compile error.
#[must_use]
pub const fn parse_build_number(text: &str) -> u64 {
    let bytes = text.as_bytes();
    assert!(!bytes.is_empty(), "MPB_BUILD_NUMBER is empty");
    assert!(
        bytes[0] != b'0',
        "MPB_BUILD_NUMBER must be positive, with no leading zero"
    );
    let mut value: u64 = 0;
    let mut i = 0;
    while i < bytes.len() {
        let digit = bytes[i];
        assert!(
            digit.is_ascii_digit(),
            "MPB_BUILD_NUMBER must be decimal digits only"
        );
        value = match value.checked_mul(10) {
            Some(v) => match v.checked_add((digit - b'0') as u64) {
                Some(v) => v,
                None => panic!("MPB_BUILD_NUMBER overflows a u64"),
            },
            None => panic!("MPB_BUILD_NUMBER overflows a u64"),
        };
        i += 1;
    }
    value
}

/// The CI build number this binary was stamped with, if it was given one.
const BUILD_NUMBER: Option<u64> = match option_env!("MPB_BUILD_NUMBER") {
    Some(text) => Some(parse_build_number(text)),
    None => None,
};

/// The CI build number, or `None` for a build that was not given one.
#[must_use]
pub fn build_number() -> Option<u64> {
    BUILD_NUMBER
}

/// The text clap prints after `macportsbrew ` for `--version`: the version on
/// the first line, then one line each for the policy data, target and commit,
/// and a `build` line when the build has a number.
#[must_use]
pub fn long_version(policy: &str) -> String {
    long_version_numbered(policy, build_number())
}

/// [`long_version`] for a given build number, so both shapes can be tested.
#[must_use]
pub fn long_version_numbered(policy: &str, build: Option<u64>) -> String {
    let mut text = format!(
        "{}\npolicy {policy}\ntarget {}\ncommit {}",
        env!("CARGO_PKG_VERSION"),
        target(),
        commit()
    );
    if let Some(number) = build {
        use std::fmt::Write as _;
        let _ = write!(text, "\nbuild {number}");
    }
    text
}

/// [`long_version`] with the embedded policy data's version, or `unknown`
/// when the embedded data cannot be loaded (which `doctor` reports). Built
/// once and kept for the process, because clap takes a `'static` string here
/// without its `string` feature.
#[must_use]
pub fn long_version_of_this_build() -> &'static str {
    static TEXT: OnceLock<String> = OnceLock::new();
    TEXT.get_or_init(|| {
        let policy = PolicyData::load_embedded()
            .map_or_else(|_| "unknown".to_string(), |data| data.version().to_string());
        long_version(&policy)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_long_version_is_four_fixed_lines_in_order() {
        let text = long_version_numbered("2026.01.01-1", None);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4, "{text}");
        assert_eq!(lines[0], env!("CARGO_PKG_VERSION"));
        assert_eq!(lines[1], "policy 2026.01.01-1");
        assert!(lines[2].starts_with("target ") && lines[2].ends_with("-apple-darwin"));
        assert!(lines[3].starts_with("commit "));
    }

    #[test]
    fn a_numbered_build_adds_one_build_line_after_the_four() {
        let text = long_version_numbered("2026.01.01-1", Some(4172));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 5, "{text}");
        assert_eq!(lines[4], "build 4172");
        assert_eq!(
            &lines[..4],
            long_version_numbered("2026.01.01-1", None)
                .lines()
                .collect::<Vec<_>>()
                .as_slice()
        );
    }

    #[test]
    fn a_good_build_number_parses_and_the_longest_u64_fits() {
        assert_eq!(parse_build_number("1"), 1);
        assert_eq!(parse_build_number("4172"), 4172);
        assert_eq!(parse_build_number("18446744073709551615"), u64::MAX);
    }

    #[test]
    fn a_malformed_build_number_is_refused() {
        for bad in [
            "",
            "0",
            "007",
            "-1",
            "+1",
            "1.5",
            "1_000",
            " 1",
            "1 ",
            "abc",
            "٣",
            "18446744073709551616",
        ] {
            assert!(
                std::panic::catch_unwind(|| parse_build_number(bad)).is_err(),
                "{bad:?} was accepted"
            );
        }
    }
}
