// SPDX-License-Identifier: Apache-2.0

//! The machine: architecture, macOS version, translation state, developer
//! tools. Detected by an adapter; consumed by policy as facts.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::path::PathBuf;

/// CPU architecture as the providers name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    /// Intel.
    X86_64,
    /// Apple Silicon.
    Arm64,
}

impl Architecture {
    /// Parses `uname -m` / `hw.machine` style names.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "x86_64" | "i386" | "amd64" => Some(Architecture::X86_64),
            "arm64" | "aarch64" => Some(Architecture::Arm64),
            _ => None,
        }
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Architecture::X86_64 => "x86_64",
            Architecture::Arm64 => "arm64",
        })
    }
}

/// A macOS product version (`10.12`, `11.0`, `26.6.2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MacosVersion {
    /// Major.
    pub major: u32,
    /// Minor.
    pub minor: u32,
    /// Patch.
    pub patch: u32,
}

impl MacosVersion {
    /// Builds a version.
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parses `sw_vers -productVersion` output.
    pub fn parse(raw: &str) -> Option<Self> {
        let mut parts = raw.trim().split('.').map(|p| p.parse::<u32>().ok());
        let major = parts.next()??;
        let minor = parts.next().flatten().unwrap_or(0);
        let patch = parts.next().flatten().unwrap_or(0);
        Some(Self::new(major, minor, patch))
    }

    /// The `major.minor` release, which is what support tables key on.
    /// Before macOS 11 the release is `10.x`; from 11 on it is the major.
    pub fn release(&self) -> (u32, u32) {
        if self.major == 10 {
            (10, self.minor)
        } else {
            (self.major, 0)
        }
    }
}

impl PartialOrd for MacosVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MacosVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

impl fmt::Display for MacosVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.patch == 0 {
            write!(f, "{}.{}", self.major, self.minor)
        } else {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

/// Whether the process runs under binary translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Translation {
    /// Native.
    Native,
    /// Running under Rosetta 2 on Apple Silicon.
    Translated,
    /// Could not be determined.
    Unknown,
}

/// Command Line Tools state.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "state")]
pub enum CommandLineTools {
    /// Installed at the given developer directory.
    Installed {
        /// The active developer directory.
        path: PathBuf,
    },
    /// Not installed.
    Missing,
    /// Could not be determined.
    Unknown,
}

/// The machine facts the policy engine consumes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    /// Hardware architecture (the machine's, not the process's).
    pub architecture: Architecture,
    /// macOS version.
    pub macos: MacosVersion,
    /// Translation state of this process.
    pub translation: Translation,
    /// Command Line Tools.
    pub command_line_tools: CommandLineTools,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_versions_parse_and_order() {
        let a = MacosVersion::parse("10.12.6").unwrap();
        let b = MacosVersion::parse("11.0").unwrap();
        let c = MacosVersion::parse("26.6.2").unwrap();
        assert!(a < b && b < c);
        assert_eq!(a.release(), (10, 12));
        assert_eq!(c.release(), (26, 0));
        assert_eq!(c.to_string(), "26.6.2");
        assert_eq!(b.to_string(), "11.0");
        assert!(MacosVersion::parse("").is_none());
    }

    #[test]
    fn architectures_parse_provider_names() {
        assert_eq!(Architecture::parse("x86_64"), Some(Architecture::X86_64));
        assert_eq!(Architecture::parse("arm64"), Some(Architecture::Arm64));
        assert_eq!(Architecture::parse("ppc"), None);
    }
}
