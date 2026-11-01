// SPDX-License-Identifier: Apache-2.0

//! Which provider owns an executable on `PATH`
//! (`05-environment-and-primary-provider.md`, "Executable ownership").

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The owner classes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "owner")]
pub enum ExecutableOwner {
    /// Part of macOS.
    System,
    /// A provider package, named.
    Provider {
        /// Provider id.
        provider: crate::provider::ProviderId,
        /// Package name, when the provider could say.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        package: Option<String>,
    },
    /// MacPortsBrew's own shim directory (no shim exists in this pass; the
    /// state exists so `doctor` can name it).
    MacportsbrewShim,
    /// Something else the user installed.
    External,
    /// Could not be determined.
    Unknown,
}

/// The resolution of one executable name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    /// The name looked up.
    pub executable: String,
    /// The path `PATH` resolves it to, when found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// The path after following symbolic links, when it differs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_path: Option<PathBuf>,
    /// Owner.
    pub owner: ExecutableOwner,
    /// `true` when the owner was decided by asking the provider; `false`
    /// when a path-prefix heuristic decided (marked, per the specification).
    pub authoritative: bool,
}

impl Resolution {
    /// How much this answer settles, for choosing between two providers'
    /// answers about one name: the provider named the package from its own
    /// records (4), claimed it by prefix and said so (3), settled it as not a
    /// provider's package (2), or could not tell (1). An unknown is never a
    /// yes (`.claude/rules/domain.md`).
    ///
    /// One rule for every reader: [`crate::Snapshot::resolution_of`] ranks an
    /// older document's per-provider answers with it, which is the rule the
    /// analysis applied to them before it moved onto that accessor (F.md §3,
    /// SG2-Q3), so the analysis output did not change.
    #[must_use]
    pub fn rank(&self) -> u8 {
        match (&self.owner, self.authoritative) {
            (ExecutableOwner::Provider { .. }, true) => 4,
            (ExecutableOwner::Provider { .. }, false) => 3,
            (
                ExecutableOwner::System
                | ExecutableOwner::External
                | ExecutableOwner::MacportsbrewShim,
                _,
            ) => 2,
            (ExecutableOwner::Unknown, _) => 1,
        }
    }
}

/// What capture established about one executable name on the captured
/// machine's `PATH` (`docs/passes/16-plans/F.md`, Design §3).
///
/// Three answers, kept apart because folding "not found" into "never looked"
/// is the defect SG-2 found: only [`Self::NotOnPath`] says a search ran and
/// nothing was there. A name absent from the snapshot's map is
/// [`Self::NotObserved`], which is also what a replayed capture records for
/// every name; it is never written to a document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolutionState {
    /// `PATH` resolved the name, and a provider said who owns the file.
    Found {
        /// The resolution, as the provider map records it.
        resolution: Resolution,
        /// The index of the first absolute `PATH` entry equal, component-wise,
        /// to the hit's parent, taken on the unredacted `PATH`; `None` when no
        /// entry matches or a relative or empty entry was skipped before the
        /// match ([`path_index`]).
        #[serde(rename = "pathIndex", default, skip_serializing_if = "Option::is_none")]
        path_index: Option<u32>,
    },
    /// A search of the live `PATH` ran and found nothing.
    NotOnPath,
    /// Never resolved: replay, a snapshot file, a name outside the class, or
    /// a hit no provider would answer about.
    NotObserved,
}

/// The index of the `PATH` entry `hit` was found in, by the rule of F.md §3
/// (SR-5): the first **absolute** entry equal, component-wise, to the hit's
/// parent. `None` when no entry matches, or when an empty or relative entry
/// comes before the match - a shell would have searched it first, so where
/// the hit stands in the search order is not established.
///
/// Called on the **unredacted** `PATH` (SR2-N7): a home entry redacted to `~/…`
/// no longer reads as absolute.
#[must_use]
pub fn path_index(entries: &[String], hit: &std::path::Path) -> Option<u32> {
    let parent = hit.parent()?;
    let mut skipped = false;
    for (index, entry) in entries.iter().enumerate() {
        if !entry.starts_with('/') {
            skipped = true;
            continue;
        }
        if std::path::Path::new(entry)
            .components()
            .eq(parent.components())
        {
            return if skipped {
                None
            } else {
                u32::try_from(index).ok()
            };
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn entries(list: &[&str]) -> Vec<String> {
        list.iter().map(|e| (*e).to_string()).collect()
    }

    #[test]
    fn the_index_is_the_first_absolute_entry_equal_by_component() {
        let path = entries(&["/a/bin", "/usr/local/bin/", "/usr/local/bin", "/usr/bin"]);
        assert_eq!(
            path_index(&path, Path::new("/usr/local/bin/xz")),
            Some(1),
            "a trailing slash is the same directory, and the first copy counts"
        );
        assert_eq!(path_index(&path, Path::new("/usr/bin/m4")), Some(3));
        assert_eq!(
            path_index(&path, Path::new("/usr/local/binx/xz")),
            None,
            "a sibling with the entry as a string prefix is not the entry"
        );
        assert_eq!(path_index(&path, Path::new("/opt/local/bin/xz")), None);
    }

    #[test]
    fn a_relative_or_empty_entry_before_the_match_makes_the_index_unknown() {
        for skipped in ["", "bin", "./bin", "~/bin"] {
            let path = entries(&["/a/bin", skipped, "/usr/bin"]);
            assert_eq!(
                path_index(&path, Path::new("/usr/bin/m4")),
                None,
                "{skipped:?} comes first"
            );
            assert_eq!(
                path_index(&path, Path::new("/a/bin/m4")),
                Some(0),
                "a match before it is unaffected"
            );
        }
    }

    #[test]
    fn the_rank_orders_what_each_answer_settles() {
        let with = |owner: ExecutableOwner, authoritative: bool| Resolution {
            executable: "x".into(),
            path: None,
            canonical_path: None,
            owner,
            authoritative,
        };
        let provider = || ExecutableOwner::Provider {
            provider: crate::ProviderId::new("p"),
            package: None,
        };
        assert_eq!(with(provider(), true).rank(), 4);
        assert_eq!(with(provider(), false).rank(), 3);
        assert_eq!(with(ExecutableOwner::System, true).rank(), 2);
        assert_eq!(with(ExecutableOwner::External, false).rank(), 2);
        assert_eq!(with(ExecutableOwner::MacportsbrewShim, false).rank(), 2);
        assert_eq!(with(ExecutableOwner::Unknown, true).rank(), 1);
    }

    #[test]
    fn the_state_is_tagged_and_not_observed_has_a_spelling() {
        let found = ResolutionState::Found {
            resolution: Resolution {
                executable: "xz".into(),
                path: Some("/usr/local/bin/xz".into()),
                canonical_path: None,
                owner: ExecutableOwner::Unknown,
                authoritative: false,
            },
            path_index: Some(6),
        };
        let text = serde_json::to_string(&found).unwrap();
        assert!(
            text.starts_with(r#"{"state":"FOUND","resolution":"#),
            "{text}"
        );
        assert!(text.ends_with(r#""pathIndex":6}"#), "{text}");
        assert_eq!(
            serde_json::from_str::<ResolutionState>(&text).unwrap(),
            found
        );
        assert_eq!(
            serde_json::to_string(&ResolutionState::NotOnPath).unwrap(),
            r#"{"state":"NOT_ON_PATH"}"#
        );
    }
}
