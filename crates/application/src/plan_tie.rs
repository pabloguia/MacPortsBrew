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
//! - **It carries the level it was confirmed at** (work order 16 part K, D5).
//!   [`confirm_shown`] and [`tie`] mint [`ConfirmationLevel::Plan`];
//!   [`confirm_removal`] and [`tie_removal`] consume a `PLAN` witness by value
//!   and return `REMOVAL`; [`confirm_advanced_removal`] consumes a `REMOVAL`
//!   witness by value and returns `ADVANCED_REMOVAL`. So no level can be
//!   minted without the one below it, and [`ShownPlan::admits`] is what the run
//!   loop asks: the plan shown, at a level at least the plan's own.

use crate::consent::{ConsentCode, ConsentRefusal};
use crate::execution_gate::GateClear;
use crate::executor::PlannedRun;
use macportsbrew_domain::{ConfirmationLevel, DomainError, ErrorId, MigrationPlan};
use sha2::{Digest, Sha256};
use std::fmt;

/// The witness that this plan was shown to a person and confirmed.
#[derive(Debug)]
#[must_use]
pub struct ShownPlan {
    /// Boxed so a question can hand the witness back by value without making
    /// its `Result` large.
    plan: Box<MigrationPlan>,
    level: ConfirmationLevel,
}

impl ShownPlan {
    /// Whether `plan` is, value for value, the plan that was shown.
    #[must_use]
    pub fn covers(&self, plan: &MigrationPlan) -> bool {
        *self.plan == *plan
    }

    /// The level this plan was confirmed at.
    #[must_use]
    pub fn level(&self) -> ConfirmationLevel {
        self.level
    }

    /// Whether `plan` may run on this witness: it is the plan that was shown,
    /// **and** it was confirmed at a level at least the one `plan` derives
    /// itself, derived again from the plan about to run (D5). The decision is
    /// the application's; the run loop only asks (SE-092).
    #[must_use]
    pub fn admits(&self, plan: &MigrationPlan) -> bool {
        self.covers(plan) && self.level >= plan.confirmation_level()
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

/// The phrase a person types to confirm that the source is removed (Q1, D5).
#[must_use]
pub fn removal_phrase(code: &ConsentCode) -> String {
    format!("remove source {code}")
}

/// The phrase a person types to confirm a removal outside the plain case (Q2,
/// D5): the source row is not in the default view, is not recorded as the
/// user's own request, or its entry or ownership is not recorded.
#[must_use]
pub fn advanced_removal_phrase(code: &ConsentCode) -> String {
    format!("confirm advanced removal {code}")
}

/// Whether `typed` is exactly `phrase`, after removing a single trailing
/// `\n` or `\r\n` and nothing else.
fn typed_is(typed: &str, phrase: &str) -> bool {
    typed
        .strip_suffix("\r\n")
        .or_else(|| typed.strip_suffix('\n'))
        .unwrap_or(typed)
        == phrase
}

/// The witness for `cleared`'s plan when `typed` is exactly its phrase with
/// `code`, after removing a single trailing `\n` or `\r\n` and nothing else.
/// Taking the [`GateClear`] means only a plan the gate cleared can be
/// confirmed. The witness is at [`ConfirmationLevel::Plan`].
#[must_use]
pub fn confirm_shown(cleared: &GateClear, code: &ConsentCode, typed: &str) -> Option<ShownPlan> {
    typed_is(typed, &plan_phrase(code)).then(|| ShownPlan {
        plan: Box::new(cleared.run().plan().clone()),
        level: ConfirmationLevel::Plan,
    })
}

/// Raises `shown` from one level to the next when `typed` is that level's
/// phrase. Consumes the witness, so the lower one cannot be kept beside the
/// higher one, and gives it back unchanged when the answer is anything else.
fn raise(
    shown: ShownPlan,
    from: ConfirmationLevel,
    to: ConfirmationLevel,
    typed_matches: bool,
) -> Result<ShownPlan, ShownPlan> {
    if shown.level == from && typed_matches {
        Ok(ShownPlan {
            plan: shown.plan,
            level: to,
        })
    } else {
        Err(shown)
    }
}

/// Q1 (D5): the witness at [`ConfirmationLevel::Removal`] when `shown` is at
/// [`ConfirmationLevel::Plan`] and `typed` is exactly [`removal_phrase`] with
/// `code`, a code drawn for this question alone. Anything else gives `shown`
/// back unchanged.
///
/// # Errors
///
/// `shown`, unchanged, when it is not at `PLAN` or the answer is not the
/// phrase.
pub fn confirm_removal(
    shown: ShownPlan,
    code: &ConsentCode,
    typed: &str,
) -> Result<ShownPlan, ShownPlan> {
    let matches = typed_is(typed, &removal_phrase(code));
    raise(
        shown,
        ConfirmationLevel::Plan,
        ConfirmationLevel::Removal,
        matches,
    )
}

/// Q2 (D5): the witness at [`ConfirmationLevel::AdvancedRemoval`] when
/// `shown` is at [`ConfirmationLevel::Removal`] and `typed` is exactly
/// [`advanced_removal_phrase`] with `code`. Anything else gives `shown` back
/// unchanged.
///
/// # Errors
///
/// `shown`, unchanged, when it is not at `REMOVAL` or the answer is not the
/// phrase.
pub fn confirm_advanced_removal(
    shown: ShownPlan,
    code: &ConsentCode,
    typed: &str,
) -> Result<ShownPlan, ShownPlan> {
    let matches = typed_is(typed, &advanced_removal_phrase(code));
    raise(
        shown,
        ConfirmationLevel::Removal,
        ConfirmationLevel::AdvancedRemoval,
        matches,
    )
}

/// What a digest is taken over, before the plan: domain separation and the
/// build's version, so a digest is valid only for builds with the same version
/// string and plan serialisation, and a mismatch across versions fails closed.
const DIGEST_DOMAIN: &str = concat!(
    "macportsbrew-plan-digest/1 ",
    env!("CARGO_PKG_VERSION"),
    "\n"
);

/// The removal digest's domain (D5): a second domain string over the same
/// plan bytes, so a removal digest never equals the plan digest and a script
/// that echoes `.planDigest` back can never confirm a removal.
const REMOVAL_DIGEST_DOMAIN: &str = concat!(
    "macportsbrew-removal-digest/1 ",
    env!("CARGO_PKG_VERSION"),
    "\n"
);

/// SHA-256 over `domain` and the plan's own serialisation.
fn digest_bytes(domain: &str, plan: &MigrationPlan) -> [u8; 32] {
    let body = serde_json::to_vec(plan).expect("a migration plan always serialises");
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(&body);
    hasher.finalize().into()
}

/// Parses `sha256:` and 64 characters of `[0-9a-f]`.
fn parse_digest(text: &str) -> Option<[u8; 32]> {
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
    Some(bytes)
}

/// Writes `sha256:<64 lowercase hex>`.
fn write_digest(bytes: &[u8; 32], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(DIGEST_PREFIX)?;
    bytes.iter().try_for_each(|b| write!(f, "{b:02x}"))
}

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
        Self(digest_bytes(DIGEST_DOMAIN, plan))
    }

    /// Parses a digest as written: exactly `sha256:` and 64 characters of
    /// `[0-9a-f]`. Upper case, whitespace, and a shorter or longer value are
    /// malformed, and answer `None`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        parse_digest(text).map(Self)
    }
}

impl fmt::Display for PlanDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_digest(&self.0, f)
    }
}

/// What `migrate` prints beside the plan digest for a `REMOVAL` plan, and
/// `--removal-digest` echoes back (D5): SHA-256 over
/// [`REMOVAL_DIGEST_DOMAIN`] and the same plan bytes as [`PlanDigest`],
/// written the same way. A type of its own, so one digest cannot be passed
/// where the other is expected. Like the plan digest it certifies that the
/// plan was printed, not that it was read (SE-111).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemovalDigest([u8; 32]);

impl RemovalDigest {
    /// The removal digest of `plan`.
    ///
    /// # Panics
    ///
    /// Never in practice, for the reason [`PlanDigest::of`] gives.
    #[must_use]
    pub fn of(plan: &MigrationPlan) -> Self {
        Self(digest_bytes(REMOVAL_DIGEST_DOMAIN, plan))
    }

    /// Parses a digest as written, by [`PlanDigest::parse`]'s rule.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        parse_digest(text).map(Self)
    }
}

impl fmt::Display for RemovalDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_digest(&self.0, f)
    }
}

impl serde::Serialize for RemovalDigest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
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
            plan: Box::new(run.plan().clone()),
            level: ConfirmationLevel::Plan,
        })
    } else {
        Err(DomainError::new(
            ErrorId::PlanNotShown,
            "its digest is not the one given to --plan-digest",
        ))
    }
}

/// The script route for a removal (D5): consumes the `PLAN` witness [`tie`]
/// minted for `run` and returns it at [`ConfirmationLevel::Removal`] when
/// `echoed` is `run`'s [`RemovalDigest`].
///
/// # Errors
///
/// - [`ErrorId::Usage`] (exit 2) for a plan at
///   [`ConfirmationLevel::AdvancedRemoval`]: an echoed value cannot show that
///   a person saw why the row needs the strictest confirmation (INV-13), so
///   that level has no script route.
/// - [`ErrorId::PlanNotShown`] (exit 4) when `shown` is not a `PLAN` witness
///   for `run`'s plan, or `echoed` is not its removal digest.
pub fn tie_removal(
    shown: ShownPlan,
    run: &PlannedRun,
    echoed: &RemovalDigest,
) -> Result<ShownPlan, DomainError> {
    let plan = run.plan();
    if plan.confirmation_level() == ConfirmationLevel::AdvancedRemoval {
        return Err(DomainError::new(
            ErrorId::Usage,
            "this removal needs a person at a terminal to confirm it; --removal-digest cannot",
        ));
    }
    let digest_matches = RemovalDigest::of(plan) == *echoed;
    if !shown.covers(plan) {
        return Err(DomainError::new(
            ErrorId::PlanNotShown,
            "the removal digest was given for a plan other than the one tied to --plan-digest",
        ));
    }
    raise(
        shown,
        ConfirmationLevel::Plan,
        ConfirmationLevel::Removal,
        digest_matches,
    )
    .map_err(|_| {
        DomainError::new(
            ErrorId::PlanNotShown,
            "its removal digest is not the one given to --removal-digest",
        )
    })
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

    fn a_plan(retain_source: bool) -> MigrationPlan {
        use macportsbrew_domain::plan::PLAN_SCHEMA_VERSION;
        use macportsbrew_domain::{EntryMode, Outcome, Ownership, PackageIdentity, ProviderId};
        MigrationPlan {
            schema_version: PLAN_SCHEMA_VERSION,
            package: PackageIdentity::new(ProviderId::new("a"), "x"),
            source: ProviderId::new("a"),
            destination: ProviderId::new("b"),
            destination_package: Some("x".into()),
            retain_source,
            entered_by: Some(EntryMode::DefaultView),
            source_ownership: Some(Ownership::TopLevel),
            primary_after: ProviderId::new("b"),
            recommendation: Outcome::MigrateTo {
                provider: ProviderId::new("b"),
            },
            overrode: None,
            steps: vec![],
            risks: vec![],
            elevated_steps: vec![],
            policy_version: "0".into(),
            dry_run_only: true,
        }
    }

    fn at(plan: MigrationPlan, level: ConfirmationLevel) -> ShownPlan {
        ShownPlan {
            plan: Box::new(plan),
            level,
        }
    }

    #[test]
    fn admits_needs_the_plan_and_a_level_at_least_its_own() {
        use ConfirmationLevel::{AdvancedRemoval, Plan, Removal};
        // Mutants 14 and 15: coverage alone, or `>`/`==` for `>=`.
        let removal = a_plan(false);
        assert_eq!(removal.confirmation_level(), ConfirmationLevel::Removal);
        let mut advanced = a_plan(false);
        advanced.entered_by = Some(macportsbrew_domain::EntryMode::Named);
        assert_eq!(
            advanced.confirmation_level(),
            ConfirmationLevel::AdvancedRemoval
        );
        let kept = a_plan(true);
        for (plan, admitted) in [
            (&kept, [true, true, true]),
            (&removal, [false, true, true]),
            (&advanced, [false, false, true]),
        ] {
            for (level, admits) in [Plan, Removal, AdvancedRemoval].into_iter().zip(admitted) {
                let witness = at(plan.clone(), level);
                assert!(witness.covers(plan));
                assert_eq!(witness.admits(plan), admits, "{level:?}: {plan:?}");
            }
        }
        // Another plan is never admitted, at any level.
        assert!(!at(removal.clone(), AdvancedRemoval).admits(&advanced));
    }

    #[test]
    fn the_removal_digest_is_never_the_plan_digest() {
        // Mutant 18: a second domain over the same bytes.
        for plan in [a_plan(false), a_plan(true)] {
            assert_ne!(
                RemovalDigest::of(&plan).to_string(),
                PlanDigest::of(&plan).to_string()
            );
            assert_ne!(RemovalDigest::of(&plan).0, PlanDigest::of(&plan).0);
        }
        assert_ne!(DIGEST_DOMAIN, REMOVAL_DIGEST_DOMAIN);
        assert!(REMOVAL_DIGEST_DOMAIN.starts_with("macportsbrew-removal-digest/1 "));
    }
}
