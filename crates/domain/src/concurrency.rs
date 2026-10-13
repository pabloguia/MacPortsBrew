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
//! build has no such check (`plan.v1.md`, obligation 5; SE-015). [`ConcurrencyFlag`]
//! is the one shape every `--json` document writes it in.

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

/// The flag as every `--json` document carries it: the activity as its tag,
/// plus whether that leaves the read possibly inconsistent, so a consumer need
/// not re-derive the rule.
///
/// It is the one serialised shape of the flag, `{ "activity": …,
/// "possiblyInconsistent": bool }`, and `analyze` (with `status` and `list`,
/// which render the same object), `plan` and `migrate` all write it through
/// this type (SE-015). The two fields cannot disagree: the only constructor
/// derives the second from the first, and a document whose
/// `possiblyInconsistent` contradicts its `activity` is refused on reading
/// rather than believed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "FlagShape")]
pub struct ConcurrencyFlag {
    #[serde(flatten)]
    activity: ConcurrentProviderActivity,
    possibly_inconsistent: bool,
}

impl ConcurrencyFlag {
    /// The flag for `activity`.
    #[must_use]
    pub fn of(activity: ConcurrentProviderActivity) -> Self {
        let possibly_inconsistent = activity.possibly_inconsistent();
        Self {
            activity,
            possibly_inconsistent,
        }
    }

    /// What is known about providers running alongside the read.
    #[must_use]
    pub fn activity(&self) -> &ConcurrentProviderActivity {
        &self.activity
    }

    /// Whether what was read may have been changing underneath the read.
    #[must_use]
    pub fn possibly_inconsistent(&self) -> bool {
        self.possibly_inconsistent
    }
}

impl From<ConcurrentProviderActivity> for ConcurrencyFlag {
    fn from(activity: ConcurrentProviderActivity) -> Self {
        Self::of(activity)
    }
}

/// What a document says, before it is checked against the rule.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FlagShape {
    #[serde(flatten)]
    activity: ConcurrentProviderActivity,
    possibly_inconsistent: bool,
}

impl TryFrom<FlagShape> for ConcurrencyFlag {
    type Error = String;

    fn try_from(shape: FlagShape) -> Result<Self, Self::Error> {
        let flag = Self::of(shape.activity);
        if flag.possibly_inconsistent == shape.possibly_inconsistent {
            Ok(flag)
        } else {
            Err(format!(
                "possiblyInconsistent is {} but the activity makes it {}",
                shape.possibly_inconsistent, flag.possibly_inconsistent
            ))
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

    #[test]
    fn the_flag_serialises_flat_with_the_rule_derived_from_the_activity() {
        assert_eq!(
            serde_json::to_string(&ConcurrencyFlag::of(ConcurrentProviderActivity::Unknowable))
                .expect("serialises"),
            r#"{"activity":"UNKNOWABLE","possiblyInconsistent":true}"#
        );
        for activity in every() {
            let flag = ConcurrencyFlag::of(activity.clone());
            assert_eq!(flag.activity(), &activity);
            assert_eq!(
                flag.possibly_inconsistent(),
                activity.possibly_inconsistent()
            );
            let text = serde_json::to_string(&flag).expect("serialises");
            let back: ConcurrencyFlag = serde_json::from_str(&text).expect("deserialises");
            assert_eq!(back, flag);
        }
    }

    #[test]
    fn a_flag_that_contradicts_its_activity_is_refused_on_reading() {
        for text in [
            r#"{"activity":"UNKNOWABLE","possiblyInconsistent":false}"#,
            r#"{"activity":"NONE_OBSERVED","possiblyInconsistent":true}"#,
        ] {
            assert!(
                serde_json::from_str::<ConcurrencyFlag>(text).is_err(),
                "{text}"
            );
        }
    }
}
