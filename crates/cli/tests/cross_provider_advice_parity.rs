// SPDX-License-Identifier: Apache-2.0

//! Every provider the registry knows has exactly one row of cross-provider
//! advice, and the advice names no provider the registry does not (DP-041).
//!
//! `doctor` looks a provider up with `PolicyData::build_defence_of` and treats a
//! provider with no row as *unadvised*, a distinct finding, never as a default.
//! So a typo in `data/policy/cross-provider-advice.toml`, or a provider added to
//! the registry without a row, would load cleanly and quietly degrade the advice
//! to "unadvised". Nothing in `policy` can catch that: it cannot read the
//! registry, and should not. This is the one crate that sees both
//! (`registry_commands_parity.rs` makes the same argument for the commands table).

use macportsbrew_adapters::registry::known_provider_ids;
use macportsbrew_policy::PolicyData;

#[test]
fn every_registry_provider_has_cross_provider_advice() {
    let data = PolicyData::load_embedded().expect("the embedded policy data loads");
    let known = known_provider_ids();
    assert!(
        !known.is_empty(),
        "the registry must know at least one provider, or this test asserts nothing"
    );
    let missing: Vec<String> = known
        .iter()
        .filter(|id| data.build_defence_of(id).is_none())
        .map(ToString::to_string)
        .collect();
    assert!(
        missing.is_empty(),
        "these providers are in registry.rs and have no row in \
         cross-provider-advice.toml, so doctor would call their exposure \
         unadvised: {missing:?}"
    );
}

#[test]
fn the_advice_names_no_provider_the_registry_does_not() {
    let data = PolicyData::load_embedded().expect("the embedded policy data loads");
    let known: Vec<String> = known_provider_ids()
        .iter()
        .map(ToString::to_string)
        .collect();
    let extra: Vec<&str> = data
        .cross_provider_advice()
        .iter()
        .map(|a| a.provider.as_str())
        .filter(|p| !known.iter().any(|k| k == p))
        .collect();
    assert!(
        extra.is_empty(),
        "cross-provider-advice.toml has rows for providers registry.rs does not know: {extra:?}"
    );
}
