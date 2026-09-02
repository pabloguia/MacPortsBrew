// SPDX-License-Identifier: Apache-2.0

//! Package version strings as the providers report them.
//!
//! A version is kept as text. Both providers use schemes that are not
//! semantic versions (`2.52.0_1`, `1.0+universal`, epochs), so the domain
//! stores what the provider said and compares only for equality and for a
//! loose "same upstream version" test used as mapping evidence.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A version string exactly as the provider reported it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Version(String);

impl Version {
    /// Wraps a provider-reported version string.
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The raw text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The upstream part of the version: everything before a provider-side
    /// revision (`_1` in Homebrew, `_1` after `@version` in MacPorts) or a
    /// variant suffix (`+universal`).
    pub fn upstream(&self) -> &str {
        let s = self.0.as_str();
        let end = s.find(['_', '+']).unwrap_or(s.len());
        &s[..end]
    }

    /// True when the upstream parts agree. Used as mapping evidence
    /// (`02-domain-model.md`, "upstream version agreement"), never as proof.
    pub fn same_upstream(&self, other: &Version) -> bool {
        !self.upstream().is_empty() && self.upstream() == other.upstream()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_strips_provider_revision_and_variants() {
        assert_eq!(Version::new("2.52.0_1").upstream(), "2.52.0");
        assert_eq!(Version::new("1.0+universal").upstream(), "1.0");
        assert_eq!(Version::new("3.1").upstream(), "3.1");
    }

    #[test]
    fn same_upstream_ignores_revisions() {
        assert!(Version::new("2.52.0_1").same_upstream(&Version::new("2.52.0")));
        assert!(!Version::new("2.52.0").same_upstream(&Version::new("2.51.0")));
        assert!(!Version::new("").same_upstream(&Version::new("")));
    }
}
