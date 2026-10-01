// SPDX-License-Identifier: Apache-2.0

//! Whether a provider was changing the machine while it was read.
//!
//! `04-migration-and-safety.md` ("Concurrency and locking") lets read-only
//! analysis proceed under a concurrently running provider, *flagged as
//! possibly inconsistent*. This type is that flag, and its one rule is that it
//! never claims a check that was not made: a run that did not look says
//! [`ConcurrentProviderActivity::NotChecked`], and a run that could not look —
//! a replayed recording or a snapshot file, which carry nothing about this
//! host's processes (INV-15) — says [`ConcurrentProviderActivity::Unknowable`].
//! Only a check that ran may answer `NoneObserved` or `Observed`, and this
//! build has no such check (`plan.v1.md`, obligation 5; SE-015).

use crate::provider::ProviderId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// What is known about providers running alongside a read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "activity", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConcurrentProviderActivity {
    /// Nobody looked. What a live run of this build says.
    NotChecked,
    /// Nothing could have looked: the input is a recording or a snapshot
    /// file, not this host's process table.
    Unknowable,
    /// A check ran and saw no provider running.
    NoneObserved,
    /// A check ran and saw these providers running.
    Observed {
        /// The providers seen, ordered (INV-15).
        providers: BTreeSet<ProviderId>,
    },
}

impl ConcurrentProviderActivity {
    /// Whether what was read may have been changing underneath the read.
    /// True for everything but a check that ran and saw nothing: an
    /// unchecked or unknowable answer is never a "no" (an unknown is never a
    /// yes, and here the favourable answer is the "no").
    #[must_use]
    pub fn possibly_inconsistent(&self) -> bool {
        match self {
            Self::NoneObserved => false,
            Self::NotChecked | Self::Unknowable | Self::Observed { .. } => true,
        }
    }

    /// Whether a check was actually made.
    #[must_use]
    pub fn was_checked(&self) -> bool {
        match self {
            Self::NoneObserved | Self::Observed { .. } => true,
            Self::NotChecked | Self::Unknowable => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every() -> Vec<ConcurrentProviderActivity> {
        vec![
            ConcurrentProviderActivity::NotChecked,
            ConcurrentProviderActivity::Unknowable,
            ConcurrentProviderActivity::NoneObserved,
            ConcurrentProviderActivity::Observed {
                providers: BTreeSet::from([ProviderId::new("p")]),
            },
        ]
    }

    #[test]
    fn only_a_check_that_saw_nothing_is_consistent() {
        for activity in every() {
            assert_eq!(
                activity.possibly_inconsistent(),
                activity != ConcurrentProviderActivity::NoneObserved,
                "{activity:?}"
            );
        }
    }

    #[test]
    fn an_unchecked_or_unknowable_answer_is_not_a_check() {
        for activity in every() {
            let checked = matches!(
                activity,
                ConcurrentProviderActivity::NoneObserved
                    | ConcurrentProviderActivity::Observed { .. }
            );
            assert_eq!(activity.was_checked(), checked, "{activity:?}");
        }
    }

    #[test]
    fn it_serialises_tagged_and_ordered() {
        let observed = ConcurrentProviderActivity::Observed {
            providers: BTreeSet::from([ProviderId::new("b"), ProviderId::new("a")]),
        };
        assert_eq!(
            serde_json::to_string(&observed).expect("serialises"),
            r#"{"activity":"OBSERVED","providers":["a","b"]}"#
        );
        assert_eq!(
            serde_json::to_string(&ConcurrentProviderActivity::NotChecked).expect("serialises"),
            r#"{"activity":"NOT_CHECKED"}"#
        );
        for activity in every() {
            let text = serde_json::to_string(&activity).expect("serialises");
            let back: ConcurrentProviderActivity =
                serde_json::from_str(&text).expect("deserialises");
            assert_eq!(back, activity);
        }
    }
}
