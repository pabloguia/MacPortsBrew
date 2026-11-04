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
//! Only a check that ran may answer `NoneObserved` or `Observed`
//! (`plan.v1.md`, obligation 5; SE-015), and a check that ran but could not
//! answer for some provider says [`ConcurrentProviderActivity::NotDetermined`].
//! The check is a [`ProviderLockProbe`]: a sample of the providers' own
//! mutation locks at one instant (R-17), never a claim that a provider is idle.
//! [`ConcurrencyFlag`] is the one shape every `--json` document writes it in.

use crate::provider::ProviderId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// What is known about providers running alongside a read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "activity", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConcurrentProviderActivity {
    /// Nobody looked. What a live run of this build says.
    NotChecked,
    /// Nothing could have looked: the input is a recording or a snapshot
    /// file, not this host's locks.
    Unknowable,
    /// A check ran and no held provider lock was observed at that instant.
    /// Never "idle" and never "safe": a provider can mutate without holding a
    /// lock the check reads, and can take one the instant after (R-17).
    NoneObserved,
    /// A check ran and a lock on one of these providers' lock files was held
    /// at that instant.
    Observed {
        /// The providers whose lock was held, ordered (INV-15).
        providers: BTreeSet<ProviderId>,
        /// Any other provider the check could not answer for: a held lock
        /// decides the outcome, and does not hide this one.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        undetermined: BTreeMap<ProviderId, UndeterminedCause>,
    },
    /// A check ran and could not answer for these providers: a lock location
    /// it could not resolve, list, open or query, or one that was not what it
    /// must be. Not a check (an unknown is never a "no").
    NotDetermined {
        /// The providers it could not answer for, and why, ordered (INV-15).
        undetermined: BTreeMap<ProviderId, UndeterminedCause>,
    },
}

/// Why a check could not answer for a provider: a closed set of tokens, so a
/// refusal can say what to do without ever carrying a path or a file name that
/// came from a directory listing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UndeterminedCause {
    /// Where this provider keeps its locks could not be resolved on this
    /// machine, or the place it must be does not exist.
    Unresolved,
    /// Something that is not a plain file sits where a lock file is read, or a
    /// lock directory is not a real directory.
    UnexpectedShape,
    /// The lock directory holds more entries than a sample reads.
    OverBound,
    /// A listing, an open or the lock query failed.
    Unreadable,
    /// The lock directory changed while it was being read.
    Changed,
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
            Self::NotChecked
            | Self::Unknowable
            | Self::Observed { .. }
            | Self::NotDetermined { .. } => true,
        }
    }

    /// Whether a check was actually made: only a definite answer is one.
    #[must_use]
    pub fn was_checked(&self) -> bool {
        match self {
            Self::NoneObserved | Self::Observed { .. } => true,
            Self::NotChecked | Self::Unknowable | Self::NotDetermined { .. } => false,
        }
    }
}

/// A sample of the providers' own mutation locks (R-17; DP-030 (b)).
///
/// One call is one instant: the answer says what was held then, and nothing
/// about the instant after (R-17 rule 4). It names no provider and no path; an
/// implementation that cannot look at this host — a replayed recording, a
/// snapshot file — answers [`ConcurrentProviderActivity::Unknowable`] without
/// looking (INV-15).
pub trait ProviderLockProbe {
    /// The locks as they stand now.
    fn sample(&self) -> ConcurrentProviderActivity;
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
                undetermined: BTreeMap::new(),
            },
            ConcurrentProviderActivity::NotDetermined {
                undetermined: BTreeMap::from([(
                    ProviderId::new("p"),
                    UndeterminedCause::Unresolved,
                )]),
            },
        ]
    }

    #[test]
    fn not_determined_is_inconsistent_unchecked_and_serialised_with_its_providers() {
        let undetermined = ConcurrentProviderActivity::NotDetermined {
            undetermined: BTreeMap::from([
                (ProviderId::new("b"), UndeterminedCause::OverBound),
                (ProviderId::new("a"), UndeterminedCause::Unresolved),
            ]),
        };
        assert!(undetermined.possibly_inconsistent());
        assert!(!undetermined.was_checked());
        assert_eq!(
            serde_json::to_string(&ConcurrencyFlag::of(undetermined)).expect("serialises"),
            r#"{"activity":"NOT_DETERMINED","undetermined":{"a":"UNRESOLVED","b":"OVER_BOUND"},"possiblyInconsistent":true}"#
        );
    }

    #[test]
    fn a_held_lock_carries_any_undetermined_provider_and_omits_an_empty_set() {
        let observed = |undetermined| ConcurrentProviderActivity::Observed {
            providers: BTreeSet::from([ProviderId::new("a")]),
            undetermined,
        };
        assert_eq!(
            serde_json::to_string(&ConcurrencyFlag::of(observed(BTreeMap::new())))
                .expect("serialises"),
            r#"{"activity":"OBSERVED","providers":["a"],"possiblyInconsistent":true}"#
        );
        assert_eq!(
            serde_json::to_string(&ConcurrencyFlag::of(observed(BTreeMap::from([(
                ProviderId::new("b"),
                UndeterminedCause::Changed
            )]))))
            .expect("serialises"),
            r#"{"activity":"OBSERVED","providers":["a"],"undetermined":{"b":"CHANGED"},"possiblyInconsistent":true}"#
        );
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
            undetermined: BTreeMap::new(),
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
