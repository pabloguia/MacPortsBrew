// SPDX-License-Identifier: Apache-2.0

//! Every provider the registry knows has a row in the commands table (F-36).
//!
//! `planner.rs` looks a provider up with `table.get(id.as_str())` and falls
//! through a missing entry to `Elevation::Unknown` with `placeholder = true` on
//! every command. Nothing asserted the two lists agree — while `AGENTS.md`
//! tells a contributor that adding a provider is "an adapter plus one registry
//! entry". Do exactly that and the first plan is a wall of `[elevation
//! unknown]` placeholders, with no test anywhere turning red.
//!
//! This lives in `cli` because it is the only crate that sees both: the
//! registry is in `adapters`, the table is loaded by `application`, and
//! neither may depend on the other. Asserting it here costs nothing and is the
//! only place the assertion is expressible without breaking the layering that
//! `scripts/check-layering.sh` enforces.

use macportsbrew_adapters::registry::known_provider_ids;
use macportsbrew_application::provider_commands;

#[test]
fn every_registry_provider_has_a_commands_entry() {
    let table = provider_commands::load().expect("the embedded commands table loads");
    let known = known_provider_ids();
    assert!(
        !known.is_empty(),
        "the registry must know at least one provider, or this test asserts nothing"
    );
    let missing: Vec<String> = known
        .iter()
        .filter(|id| !table.contains_key(id.as_str()))
        .map(ToString::to_string)
        .collect();
    assert!(
        missing.is_empty(),
        "these providers are in registry.rs and absent from provider-commands.toml, \
         so every command in a plan for them would be an unrunnable placeholder \
         carrying `elevation unknown` (F-36): {missing:?}"
    );
}

#[test]
fn the_commands_table_names_no_provider_the_registry_does_not() {
    // The other direction, and a weaker claim on purpose: an extra row is not
    // a safety problem the way a missing one is — it is dead data, not a
    // placeholder reaching a user. It is worth catching because the two lists
    // drifting apart in either direction means one of them was edited alone,
    // and the next edit is as likely to be the dangerous one.
    let table = provider_commands::load().expect("the embedded commands table loads");
    let known: Vec<String> = known_provider_ids()
        .iter()
        .map(ToString::to_string)
        .collect();
    let extra: Vec<&String> = table.keys().filter(|k| !known.contains(k)).collect();
    assert!(
        extra.is_empty(),
        "provider-commands.toml has rows for providers registry.rs does not know: {extra:?}"
    );
}
