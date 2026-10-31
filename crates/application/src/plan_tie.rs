// SPDX-License-Identifier: Apache-2.0

//! The executed plan tied to the one shown (pass 08 slice 7, work order 16
//! part E, DP-049; INV-02).
//!
//! A [`ShownPlan`] is the witness that a person was shown a plan and confirmed
//! it. It **holds the plan itself**, so the check inside the process is value
//! equality with what was shown and needs no hash; a digest (DP-049 (a),
//! E-4) exists only to carry "what was shown" across a process boundary, and
//! mints the same witness.
//!
//! - **Minted only here**, by [`confirm_shown`] (the interactive route,
//!   DP-049 (b)), from a run the gate has already cleared: a plan the gate
//!   refuses is never asked about (E plan, R1). A word scan holds the callers
//!   of `confirm_shown(` to the command-line crate's consent module.
//! - **Neither `Clone` nor `Default`**, and it derives `Debug` only, so it
//!   cannot be copied onto another run or read from a file.
//! - **The phrase** is `run plan <code>`, ASCII, never translated, with a code
//!   drawn fresh for the question (SR 10), so a line typed before the plan was
//!   shown cannot confirm it.

use crate::consent::ConsentCode;
use crate::execution_gate::GateClear;
use macportsbrew_domain::MigrationPlan;

/// The witness that this plan was shown to a person and confirmed.
#[derive(Debug)]
#[must_use]
pub struct ShownPlan {
    plan: MigrationPlan,
}

impl ShownPlan {
    /// Whether `plan` is, value for value, the plan that was shown.
    #[must_use]
    pub fn covers(&self, plan: &MigrationPlan) -> bool {
        self.plan == *plan
    }
}

/// Compile-time proof that [`ShownPlan`] is **not** `Clone` and **not**
/// `Default`: with either implemented, the blanket impls make the item
/// ambiguous and this file stops compiling (the idiom of `execution_gate.rs`,
/// repeated so that file does not change).
mod not_clone_nor_default {
    pub(super) trait AmbiguousIfClone<A> {
        fn item() {}
    }
    impl<T: ?Sized> AmbiguousIfClone<()> for T {}
    impl<T: Clone> AmbiguousIfClone<u8> for T {}
    pub(super) trait AmbiguousIfDefault<A> {
        fn item() {}
    }
    impl<T: ?Sized> AmbiguousIfDefault<()> for T {}
    impl<T: Default> AmbiguousIfDefault<u8> for T {}
}
const _: fn() = || {
    use not_clone_nor_default::{AmbiguousIfClone, AmbiguousIfDefault};
    <ShownPlan as AmbiguousIfClone<_>>::item();
    <ShownPlan as AmbiguousIfDefault<_>>::item();
};

/// The phrase a person types to confirm the plan they were shown.
#[must_use]
pub fn plan_phrase(code: &ConsentCode) -> String {
    format!("run plan {code}")
}

/// The witness for `cleared`'s plan when `typed` is exactly its phrase with
/// `code`, after removing a single trailing `\n` or `\r\n` and nothing else.
/// Taking the [`GateClear`] means only a plan the gate cleared can be
/// confirmed.
#[must_use]
pub fn confirm_shown(cleared: &GateClear, code: &ConsentCode, typed: &str) -> Option<ShownPlan> {
    let typed = typed
        .strip_suffix("\r\n")
        .or_else(|| typed.strip_suffix('\n'))
        .unwrap_or(typed);
    (typed == plan_phrase(code)).then(|| ShownPlan {
        plan: cleared.run().plan().clone(),
    })
}
