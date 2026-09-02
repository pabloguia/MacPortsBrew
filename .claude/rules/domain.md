---
paths:
  - "crates/domain/**"
---

# The domain — provider-neutral types and pure rules

Read `docs/spec/02-domain-model.md` before changing anything here. Two rules
govern this crate, and both are checked mechanically.

## No I/O, of any kind

No filesystem, no network, no subprocess, no environment read, **no clock**.
This crate depends on `serde` and nothing else in the workspace, and
`scripts/check-layering.sh` asserts that edge from `cargo metadata`. If a type
needs the current date, the date is a field the caller supplies — never a value
the type fetches.

`invariant_14_the_wall_clock_is_read_in_exactly_one_place` and
`invariant_14_the_environment_is_read_in_exactly_one_place` exist to keep this
honest.

## No provider name in any signature

Not `homebrew`, not `macports`, not in a type, a variant, a field, a function
name or a doc example that would read as one. A provider is a `ProviderId`
value; the two real ones appear in `crates/adapters/` and nowhere above.

This is what makes migration symmetric (INV-17) and a third provider addable
without redesigning the domain. A `match` on a provider id in this crate is a
design defect, not a shortcut.

## Everything serialised is ordered

`BTreeMap`, `BTreeSet`, or a `Vec` sorted before it is emitted. The same input
must produce the same bytes on every machine and every run (INV-15), because
`--json` is a compatibility surface, snapshots are diffed, and golden
comparisons are worthless against an incidental order.

## `UNKNOWN` is a real state, not a gap

`ArtifactAvailability::Unknown`, `CapabilityState::Unknown`,
`Ownership::Unknown`, `ExecutableOwner::Unknown`, `SupportTier::Unknown`,
`Confidence::None`, `ProviderPresence::Broken`. Keep them; do not fold one into
its favourable neighbour to simplify a `match`. An unknown is never a yes, and a
wildcard arm that swallows them is how that guarantee gets lost.

`match_same_arms` is deliberately allowed in this workspace for exactly this
reason: exhaustive handling of these enums is the point, and lint pressure
toward a wildcard is pressure in the wrong direction.

## The plan is data

`MigrationPlan` and everything under it is a serialisable, diffable,
re-executable *description*. Nothing in this crate executes anything, and
`invariant_02_plans_are_data_and_nothing_in_this_layer_can_execute_them`
asserts it. Step ordering is part of the type's contract:
`MigrationPlan::check_ordering` enforces that no `REMOVE_SOURCE` precedes the
`VERIFY_DESTINATION` guarding it (INV-01).

## Changing a serialised shape is a schema change

Field names are `camelCase`; enum values are `SCREAMING_SNAKE_CASE` except
`Architecture`, which is `snake_case` because that is how both providers spell
it. Renaming a field, changing a type, removing an enum value or changing a
tagged union's tag is **breaking**: bump `schemaVersion`, add the `v2` document
beside the `v1` one in `docs/schemas/`, and write the `docs/spec-evolution.md`
entry in the same change.

## Exit codes are a documented contract

`ErrorId::exit_code` in `error.rs` implements the table in
`docs/spec/06-cli-and-ux.md`. Every identifier also carries a `message_id` that
must exist in `locales/en.toml`. `"error: exit status 1"` is never acceptable
user-facing output.
