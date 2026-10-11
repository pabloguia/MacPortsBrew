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
//! ```
//!
//! The block is a fixed, machine-read English contract (the release job reads
//! it), which is why it stays outside the i18n catalogue; the exemption is
//! recorded in `docs/spec-evolution.md`. The target and commit come from
//! `MPB_BUILD_TARGET` and `MPB_BUILD_COMMIT` at compile time, which the release
//! script sets; a developer build prints the target its architecture implies
//! and says it is not a release build. No build script is involved.

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

/// The text clap prints after `macportsbrew ` for `--version`: the version on
/// the first line, then one line each for the policy data, target and commit.
#[must_use]
pub fn long_version(policy: &str) -> String {
    format!(
        "{}\npolicy {policy}\ntarget {}\ncommit {}",
        env!("CARGO_PKG_VERSION"),
        target(),
        commit()
    )
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
        let text = long_version("2026.01.01-1");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4, "{text}");
        assert_eq!(lines[0], env!("CARGO_PKG_VERSION"));
        assert_eq!(lines[1], "policy 2026.01.01-1");
        assert!(lines[2].starts_with("target ") && lines[2].ends_with("-apple-darwin"));
        assert!(lines[3].starts_with("commit "));
    }
}
