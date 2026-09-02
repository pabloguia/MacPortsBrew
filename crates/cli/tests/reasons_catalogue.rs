// SPDX-License-Identifier: Apache-2.0

//! The UI's catalogue must cover the policy engine's reason vocabulary.
//!
//! `scripts/check-i18n-parity.sh` checks catalogue files against each other
//! and against message identifiers written as literals in code. It cannot see
//! this: the reason identifiers are `pub const`s in another crate, assembled
//! at run time, and a rule that emits one this catalogue lacks would degrade
//! to `reason.unknown` in front of a user without any check failing.
//!
//! `06-cli-and-ux.md` requires every reason to be renderable as a human
//! sentence in every supported locale, and `03-policy-and-recommendations.md`
//! that every recommendation carries its reasons. Together those mean the
//! catalogue is not allowed to lag the engine, and this is where that is
//! enforced.

use std::collections::BTreeSet;

use macportsbrew::i18n::Catalogue;
use macportsbrew_policy::reasons;

#[test]
fn english_covers_every_reason_the_policy_engine_can_emit() {
    let catalogue = Catalogue::english();
    let keys: BTreeSet<&str> = catalogue.keys().collect();

    let absent: Vec<&str> = reasons::all()
        .iter()
        .copied()
        .filter(|id| !keys.contains(id))
        .collect();

    assert!(
        absent.is_empty(),
        "locales/en.toml has no sentence for {absent:?}; a reason with no \
         translation reaches the user as reason.unknown"
    );
}

#[test]
fn the_catalogue_carries_no_orphaned_reason_key() {
    // The other direction: a `reason.*` key with no rule behind it is a
    // sentence nobody can trigger, and it will be translated into every
    // locale before anyone notices. `reason.unknown` is the one deliberate
    // exception — it is the fallback the renderer uses for an identifier the
    // catalogue does not carry, so by construction no rule emits it.
    let catalogue = Catalogue::english();
    let known: BTreeSet<&str> = reasons::all().iter().copied().collect();

    let orphaned: Vec<&str> = catalogue
        .keys()
        .filter(|k| k.starts_with("reason."))
        .filter(|k| **k != *"reason.unknown")
        .filter(|k| !known.contains(*k))
        .collect();

    assert!(
        orphaned.is_empty(),
        "locales/en.toml carries {orphaned:?}, which no rule emits"
    );
}
