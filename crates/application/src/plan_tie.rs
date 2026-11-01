// SPDX-License-Identifier: Apache-2.0

//! The executed plan tied to the one shown (pass 08 slice 7, work order 16
//! part E, DP-049; INV-02).
//!
//! A [`ShownPlan`] is the witness that a person was shown a plan and confirmed
//! it. It **holds the plan itself**, so the check inside the process is value
//! equality with what was shown and needs no hash; a [`PlanDigest`] (DP-049
//! (a), work order 16 part E-4) exists only to carry "what was shown" across a
//! process boundary, and mints the same witness.
//!
//! - **Minted only here**, by [`confirm_shown`] (the interactive route,
//!   DP-049 (b)), from a run the gate has already cleared: a plan the gate
//!   refuses is never asked about (E plan, R1); and by [`tie`] (the script
//!   route, DP-049 (a)), from the digest a person echoed back, whatever the
//!   allowance. Word scans hold the callers of `confirm_shown(` to the
//!   command-line crate's consent module and of `tie(` to `migrate`.
//! - **Neither `Clone` nor `Default`**, and it derives `Debug` only, so it
//!   cannot be copied onto another run or read from a file.
//! - **The phrase** is `run plan <code>`, ASCII, never translated, with a code
//!   drawn fresh for the question (SR 10), so a line typed before the plan was
//!   shown cannot confirm it.

use crate::consent::{ConsentCode, ConsentRefusal};
use crate::execution_gate::GateClear;
use crate::executor::PlannedRun;
use macportsbrew_domain::{DomainError, ErrorId, MigrationPlan};
use sha2::{Digest, Sha256};
use std::fmt;

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

/// What a digest is taken over, before the plan: domain separation and the
/// build's version, so a digest is valid only for builds with the same version
/// string and plan serialisation, and a mismatch across versions fails closed.
const DIGEST_DOMAIN: &str = concat!(
    "macportsbrew-plan-digest/1 ",
    env!("CARGO_PKG_VERSION"),
    "\n"
);

/// What `migrate` prints for a plan and `--plan-digest` echoes back: SHA-256
/// over [`DIGEST_DOMAIN`] and the plan's own serialisation, written
/// `sha256:<64 lowercase hex>`.
///
/// Over the in-memory plan — the same value the `migrate` document carries:
/// redaction happens at capture, not when the document is written, so
/// anyone who edits a document's `plan` must drop its digest too. The
/// serialisation is
/// `serde_json::to_vec` of the struct, never through `serde_json::Value`
/// (whose `preserve_order` maps would make order a matter of insertion); the
/// plan holds no unordered collection and no float, and carries no timestamp
/// (INV-15). Not a secret: compared as bytes. It has no public constructor
/// from bytes, only [`PlanDigest::of`] and [`PlanDigest::parse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanDigest([u8; 32]);

/// The prefix a written digest carries.
const DIGEST_PREFIX: &str = "sha256:";

impl PlanDigest {
    /// The digest of `plan`.
    ///
    /// # Panics
    ///
    /// Never in practice: a [`MigrationPlan`] is plain data whose
    /// serialisation cannot fail (no map with non-string keys, no custom
    /// serialiser that errors).
    #[must_use]
    pub fn of(plan: &MigrationPlan) -> Self {
        let body = serde_json::to_vec(plan).expect("a migration plan always serialises");
        let mut hasher = Sha256::new();
        hasher.update(DIGEST_DOMAIN.as_bytes());
        hasher.update(&body);
        Self(hasher.finalize().into())
    }

    /// Parses a digest as written: exactly `sha256:` and 64 characters of
    /// `[0-9a-f]`. Upper case, whitespace, and a shorter or longer value are
    /// malformed, and answer `None`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix(DIGEST_PREFIX)?;
        if hex.len() != 64 || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            return None;
        }
        let mut bytes = [0u8; 32];
        let (pairs, _) = hex.as_bytes().as_chunks::<2>();
        for (byte, [high, low]) in bytes.iter_mut().zip(pairs) {
            let nibble = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
            *byte = (nibble(*high) << 4) | nibble(*low);
        }
        Some(Self(bytes))
    }
}

impl fmt::Display for PlanDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(DIGEST_PREFIX)?;
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl serde::Serialize for PlanDigest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// The witness for `run`'s plan when `echoed` is its digest — the script
/// route (DP-049 (a)): a person ran `migrate`, read the plan and its digest,
/// and passed the digest back with `--yes --plan-digest`. Any other digest is
/// [`ErrorId::PlanNotShown`] (exit 4): the plan changed since it was shown,
/// and nothing was changed.
pub fn tie(run: &PlannedRun, echoed: &PlanDigest) -> Result<ShownPlan, DomainError> {
    if PlanDigest::of(run.plan()) == *echoed {
        Ok(ShownPlan {
            plan: run.plan().clone(),
        })
    } else {
        Err(DomainError::new(
            ErrorId::PlanNotShown,
            "its digest is not the one given to --plan-digest",
        ))
    }
}

/// The error a refused plan question ends the run with (work order 16 part E,
/// R2 and R5): each refusal its own identifier, so a script tells a person's
/// refusal (exit 6) from a missing terminal (2) and from a safety stop (4),
/// none of which writes to stdout. The application decides; the shell only
/// returns it (SE-092).
#[must_use]
pub fn plan_question_error(why: ConsentRefusal) -> DomainError {
    match why {
        ConsentRefusal::Declined => {
            DomainError::new(ErrorId::UserAborted, "the plan was not confirmed")
        }
        ConsentRefusal::NotATerminal => DomainError::new(
            ErrorId::Usage,
            "--yes needs a terminal to confirm the plan it shows, or --plan-digest",
        ),
        ConsentRefusal::CodeUnavailable => DomainError::new(
            ErrorId::SafetyBlocked,
            "no confirmation code could be drawn, so the plan was not asked about",
        ),
        ConsentRefusal::NoRequests | ConsentRefusal::PlanDisagrees => DomainError::new(
            ErrorId::Internal,
            "the plan question answered with an elevation refusal",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariant_02_a_declined_plan_question_exits_6_before_anything_runs() {
        // Every refusal by name: only a person's refusal is exit 6.
        let cases = [
            (ConsentRefusal::Declined, ErrorId::UserAborted, 6),
            (ConsentRefusal::NotATerminal, ErrorId::Usage, 2),
            (ConsentRefusal::CodeUnavailable, ErrorId::SafetyBlocked, 4),
            (ConsentRefusal::NoRequests, ErrorId::Internal, 1),
            (ConsentRefusal::PlanDisagrees, ErrorId::Internal, 1),
        ];
        for (why, id, code) in cases {
            let error = plan_question_error(why);
            assert_eq!((error.id, error.exit_code()), (id, code), "{why:?}");
        }
    }
}
