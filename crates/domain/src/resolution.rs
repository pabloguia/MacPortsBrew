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
