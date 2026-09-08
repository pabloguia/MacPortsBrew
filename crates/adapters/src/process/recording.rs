// SPDX-License-Identifier: Apache-2.0

//! Recordings: captured invocations that replay as fixtures.
//!
//! A fixture is a recording plus provenance (`07-quality-and-testing.md`).
//! The [`ReplayRunner`] answers a [`CommandSpec`] with the recorded
//! invocation whose program name and argv match; a spec with no recorded
//! answer is an error, never a guess.

use super::{CommandSpec, Invocation, ProcessRunner};
use macportsbrew_domain::{DomainError, ErrorId};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeSet;

/// Where a recording came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingProvenance {
    /// macOS version of the capturing machine, as text.
    pub macos: String,
    /// Architecture.
    pub architecture: String,
    /// Provider versions observed, as text, sorted by provider.
    pub provider_versions: Vec<String>,
    /// RFC 3339 capture time.
    pub captured_at: String,
    /// The command that captured it.
    pub captured_by: String,
    /// `captured` from a live machine or `synthesised` from documentation.
    pub origin: String,
    /// A note (which research note, which edits were made to a derived
    /// fixture).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One recorded invocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedInvocation {
    /// The record.
    #[serde(flatten)]
    pub invocation: Invocation,
}

/// A set of recorded invocations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    /// Recording format version.
    pub schema_version: u32,
    /// Provenance.
    pub provenance: RecordingProvenance,
    /// The invocations, in capture order.
    pub invocations: Vec<RecordedInvocation>,
}

/// The recording schema version.
pub const RECORDING_SCHEMA_VERSION: u32 = 1;

impl Recording {
    /// Parses a recording from JSON.
    pub fn from_json(text: &str) -> Result<Self, DomainError> {
        // `RecordingUnusable`, not `SnapshotUnusable`: same exit code 8, and a
        // remediation that names `scripts/capture-fixture.sh` and `--replay`
        // rather than `macportsbrew snapshot` and `--from-snapshot` (F-41).
        let rec: Recording = serde_json::from_str(text)
            .map_err(|e| DomainError::new(ErrorId::RecordingUnusable, format!("recording: {e}")))?;
        if rec.schema_version != RECORDING_SCHEMA_VERSION {
            return Err(DomainError::new(
                ErrorId::RecordingUnusable,
                format!(
                    "recording schema {} is not the supported {}",
                    rec.schema_version, RECORDING_SCHEMA_VERSION
                ),
            ));
        }
        Ok(rec)
    }

    /// Serialises, pretty-printed and stable.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Finds the invocation for a spec by program name and argv.
    pub fn find(&self, spec: &CommandSpec) -> Option<&Invocation> {
        let name = spec.program_name();
        self.invocations
            .iter()
            .map(|r| &r.invocation)
            .find(|i| i.program == name && i.argv == spec.argv)
    }
}

/// A runner that answers from a recording.
pub struct ReplayRunner {
    recording: Recording,
    consulted: RefCell<BTreeSet<String>>,
}

impl ReplayRunner {
    /// Wraps a recording.
    pub fn new(recording: Recording) -> Self {
        Self {
            recording,
            consulted: RefCell::new(BTreeSet::new()),
        }
    }

    /// The recording.
    pub fn recording(&self) -> &Recording {
        &self.recording
    }

    /// Which recorded invocations were consulted (program + argv), sorted,
    /// so a test can assert an adapter used exactly the commands it claims.
    pub fn consulted(&self) -> BTreeSet<String> {
        self.consulted.borrow().clone()
    }
}

impl ProcessRunner for ReplayRunner {
    fn run(&self, spec: &CommandSpec) -> Result<Invocation, DomainError> {
        let key = format!("{} {}", spec.program_name(), spec.argv.join(" "));
        match self.recording.find(spec) {
            Some(inv) => {
                self.consulted.borrow_mut().insert(key);
                let mut inv = inv.clone();
                // The environment is the caller's construction, not the
                // recording's: replay reports what would have been set.
                inv.environment.clone_from(&spec.environment);
                Ok(inv)
            }
            None => Err(DomainError::new(
                ErrorId::Internal,
                format!("no recorded invocation for `{key}` in this fixture"),
            )),
        }
    }

    fn is_replay(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ExitStatus;
    use std::collections::BTreeMap;

    fn recording() -> Recording {
        Recording {
            schema_version: RECORDING_SCHEMA_VERSION,
            provenance: RecordingProvenance {
                macos: "26.6.2".into(),
                architecture: "x86_64".into(),
                provider_versions: vec![],
                captured_at: "2026-09-02T00:00:00Z".into(),
                captured_by: "test".into(),
                origin: "synthesised".into(),
                note: None,
            },
            invocations: vec![RecordedInvocation {
                invocation: Invocation {
                    program: "brew".into(),
                    argv: vec!["--prefix".into()],
                    environment: BTreeMap::new(),
                    status: ExitStatus::Exited { code: 0 },
                    stdout: "/usr/local\n".into(),
                    stderr: String::new(),
                    truncated: false,
                    duration_ms: 1,
                },
            }],
        }
    }

    #[test]
    fn replay_matches_on_program_name_and_argv() {
        let runner = ReplayRunner::new(recording());
        let spec = CommandSpec::new(
            "/opt/homebrew/bin/brew",
            vec!["--prefix".into()],
            BTreeMap::new(),
        );
        let inv = runner.run(&spec).unwrap();
        assert_eq!(inv.stdout, "/usr/local\n");
        assert!(runner.is_replay());
        assert!(runner.consulted().contains("brew --prefix"));
        let missing = CommandSpec::new(
            "/usr/local/bin/brew",
            vec!["--cellar".into()],
            BTreeMap::new(),
        );
        assert!(runner.run(&missing).is_err());
    }

    #[test]
    fn json_round_trip_is_stable() {
        let rec = recording();
        let text = rec.to_json();
        let back = Recording::from_json(&text).unwrap();
        assert_eq!(rec, back);
        assert_eq!(back.to_json(), text);
        assert!(Recording::from_json("{\"schemaVersion\":99}").is_err());
    }
}
