// SPDX-License-Identifier: Apache-2.0

//! The sanitised environment (INV-10).
//!
//! Nothing is inherited from the calling shell. The environment a provider
//! sees is built from an explicit `PATH`, a stable locale, `HOME`, and the
//! variables the caller passes - which for a provider are the ones that keep
//! it quiet during analysis. Build-affecting variables (`CPATH`,
//! `LIBRARY_PATH`, `PKG_CONFIG_PATH`, `LDFLAGS`, `CPPFLAGS`, `CFLAGS`,
//! `DYLD_*`) never appear, so one provider's prefix cannot leak into the
//! other's invocation.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// The system directories every invocation may see.
use macportsbrew_domain::{PINNED_ENVIRONMENT, SYSTEM_PATH};

/// What the caller contributes to the environment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnvironmentSpec {
    /// Directories placed *before* the system path - the provider's own
    /// `bin`, and nothing else.
    pub leading_path: Vec<PathBuf>,
    /// The user's home directory, when the provider needs it (Homebrew
    /// keeps its cache there). `None` omits `HOME` entirely.
    pub home: Option<PathBuf>,
    /// Provider-specific variables (quieting, no-analytics, no-hints).
    pub extra: BTreeMap<String, String>,
}

/// Builds the environment. Variables in `extra` cannot override `PATH`,
/// `LANG` or `LC_ALL`, and cannot introduce a build-flag variable.
pub fn sanitised_environment(spec: &EnvironmentSpec) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    let mut path: Vec<String> = spec
        .leading_path
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    for p in SYSTEM_PATH {
        if !path.iter().any(|x| x == p) {
            path.push((*p).to_string());
        }
    }
    env.insert("PATH".into(), path.join(":"));
    for (key, value) in PINNED_ENVIRONMENT {
        env.insert((*key).to_string(), (*value).to_string());
    }
    if let Some(home) = &spec.home {
        env.insert("HOME".into(), home.to_string_lossy().into_owned());
    }
    for (k, v) in &spec.extra {
        if is_reserved(k) || is_build_flag(k) {
            continue;
        }
        env.insert(k.clone(), v.clone());
    }
    env
}

fn is_reserved(key: &str) -> bool {
    key == "PATH" || PINNED_ENVIRONMENT.iter().any(|(k, _)| *k == key)
}

/// Variables that make one prefix visible to the other's build system.
/// The list itself is `domain`, so the filter here, the reporting superset in
/// `system::machine` and the INV-10 test all read one definition rather than
/// three copies kept in step by hand.
pub use macportsbrew_domain::is_build_flag;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_inherited_and_build_flags_are_dropped() {
        let mut extra = BTreeMap::new();
        extra.insert("HOMEBREW_NO_AUTO_UPDATE".to_string(), "1".to_string());
        extra.insert(
            "PKG_CONFIG_PATH".to_string(),
            "/opt/local/lib/pkgconfig".to_string(),
        );
        extra.insert(
            "DYLD_LIBRARY_PATH".to_string(),
            "/opt/local/lib".to_string(),
        );
        extra.insert("PATH".to_string(), "/evil".to_string());
        let env = sanitised_environment(&EnvironmentSpec {
            leading_path: vec![PathBuf::from("/usr/local/bin")],
            home: Some(PathBuf::from("/Users/x")),
            extra,
        });
        assert_eq!(env["PATH"], "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin");
        assert_eq!(env["LC_ALL"], "C");
        assert_eq!(env["HOME"], "/Users/x");
        assert_eq!(env["HOMEBREW_NO_AUTO_UPDATE"], "1");
        assert!(!env.contains_key("PKG_CONFIG_PATH"));
        assert!(!env.contains_key("DYLD_LIBRARY_PATH"));
        assert!(!env.values().any(|v| v.contains("/evil")));
    }

    #[test]
    fn the_other_prefix_is_never_on_path() {
        let env = sanitised_environment(&EnvironmentSpec {
            leading_path: vec![PathBuf::from("/opt/local/bin")],
            home: None,
            extra: BTreeMap::new(),
        });
        assert!(!env["PATH"].contains("/usr/local"));
        assert!(!env.contains_key("HOME"));
    }
}
