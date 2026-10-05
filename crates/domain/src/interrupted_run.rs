// SPDX-License-Identifier: Apache-2.0

//! The interrupted-run marker's content (`04-migration-and-safety.md`,
//! Concurrency and locking; pass 08 slice 2, `docs/schemas/run-marker.v1.md`).
//!
//! A mutating run writes a marker naming itself before its first provider
//! change and clears it once its record is published. A marker found later is
//! a run that did not finish being accounted for, and the next mutating
//! command refuses and names it (`06-cli-and-ux.md`, exit 4).
//!
//! The marker holds the run's identifier and nothing else: no path, package or
//! time that could be read back as authority. What reads it back re-checks the
//! grammar ([`InterruptedRun::from_stored`]) and refuses anything else —
//! never "no marker" (fail closed).

use crate::history::is_run_id;
use serde::{Deserialize, Serialize};

/// The marker file's schema version.
pub const MARKER_SCHEMA_VERSION: u32 = 1;

/// A run that was started and not accounted for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterruptedRun {
    run_id: String,
}

/// The serialised marker. Strict on read: an unknown field, a second schema
/// or an id that is not a run id is refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredInterruptedRun {
    /// Always [`MARKER_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The run's identifier.
    pub run_id: String,
}

impl InterruptedRun {
    /// A marker for run `run_id`, which must be a run id: the one check an id
    /// passes before it is written or becomes part of a message.
    pub fn new(run_id: &str) -> Result<Self, String> {
        if is_run_id(run_id) {
            Ok(Self {
                run_id: run_id.to_string(),
            })
        } else {
            Err("the marker names something that is not a run id".to_string())
        }
    }

    /// The run's identifier.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// The shape to write.
    #[must_use]
    pub fn to_stored(&self) -> StoredInterruptedRun {
        StoredInterruptedRun {
            schema_version: MARKER_SCHEMA_VERSION,
            run_id: self.run_id.clone(),
        }
    }

    /// Re-validates what was read: the schema version and the id grammar.
    pub fn from_stored(stored: &StoredInterruptedRun) -> Result<Self, String> {
        if stored.schema_version != MARKER_SCHEMA_VERSION {
            return Err(format!(
                "the marker is schema version {}, and this version reads {MARKER_SCHEMA_VERSION}",
                stored.schema_version
            ));
        }
        Self::new(&stored.run_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "20261005T120000Z-00000000000000aa";

    #[test]
    fn a_run_id_round_trips_through_the_stored_shape() {
        let marker = InterruptedRun::new(ID).unwrap();
        assert_eq!(InterruptedRun::from_stored(&marker.to_stored()), Ok(marker));
    }

    #[test]
    fn anything_that_is_not_a_run_id_is_refused() {
        for bad in ["", "..", "a/b", "run-1", "20261005T120000Z-ABC", " "] {
            assert!(InterruptedRun::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn another_schema_version_is_refused() {
        let mut stored = InterruptedRun::new(ID).unwrap().to_stored();
        stored.schema_version = 2;
        assert!(InterruptedRun::from_stored(&stored).is_err());
        let mut stored = InterruptedRun::new(ID).unwrap().to_stored();
        stored.schema_version = 0;
        assert!(InterruptedRun::from_stored(&stored).is_err());
    }

    #[test]
    fn an_unknown_field_is_refused_when_the_marker_is_read() {
        let text = format!(r#"{{"schemaVersion":1,"runId":"{ID}","extra":true}}"#);
        assert!(serde_json::from_str::<StoredInterruptedRun>(&text).is_err());
    }
}
