---
paths:
  - "crates/policy/**"
---

# The policy engine — rules, not opinions

Read `docs/spec/03-policy-and-recommendations.md` and INV-14, INV-15, INV-17 and
INV-20 in `docs/spec/01-invariants.md` before changing anything here.

## A rule is a function of `(facts, data)`

Nothing else. No I/O, no filesystem, no environment, no network, and **no
clock** — the current date is `facts.today`, supplied by the caller, because a
rule that reads a clock is a rule whose output cannot be reproduced from a
snapshot (INV-14, INV-15).

`invariant_14_no_llm_no_io_engine_is_pure` and
`invariant_14_the_current_date_is_an_input_not_a_clock_read` defend this.

## No rule may name a provider

Provider identifiers are values a rule reads out of the facts and passes
through. **None is written down anywhere in this crate.** Exchanging the two
providers in the input must exchange them in the output and change nothing else
(INV-17), which
`invariant_17_every_rule_individually_follows_the_rename` checks rule by rule
rather than only in aggregate.

If you cannot express a rule without naming a provider, the fact you need is
data. It belongs in `data/policy/*.toml` with its source and read date, and the
rule reads it from there.

## Every rule declares its provenance

`id` (stable `SCREAMING_SNAKE_CASE`, and changing one is a compatibility change
because it appears in `explain` output and in every reason), `since` (the policy
version it arrived in), `source` (a specification section, an invariant or a
research note — **a rule with no source is an opinion**), `reviewed_on` (when
that reasoning was last checked), `priority`, `evaluate`.

`evaluate` returning `None` means the rule did not fire, which is a first-class
answer that `explain` reports. "Why did it *not* recommend X" must stay
answerable.

## Ordering is a contract, not an implementation detail

Rules evaluate in identifier order; outcomes resolve by priority; **ties break
by identifier**, so the answer never depends on declaration order (INV-15).
`all()` sorts, so where you put a rule in the literal does not matter.

Take `priority()` from the constants in `rules/mod.rs`, never a bare number:
`PRIORITY_MANUAL_REVIEW` (1000), `PRIORITY_RUNTIME` (950),
`PRIORITY_DESTINATION_VETO` (900), `PRIORITY_SUPPORT_ENDING` (700),
`PRIORITY_ARTIFACT_ADVANTAGE` (600), `PRIORITY_COEXISTENCE` (500),
`PRIORITY_NOTHING_TO_DO` (200), `PRIORITY_FALLBACK` (100).

**Nothing outranks `MANUAL_REVIEW`**, and the engine enforces that by *outcome*
rather than by number (INV-19). Do not attempt to express "usually review, but
not here" as a priority; express it as the rule not firing.

## Dated provider facts are data (INV-20)

No end-of-support date, bottle tag, prefix, verb or elevation requirement is
written into Rust. It lives in `data/policy/*.toml` with `source` and `read_on`,
and `invariant_20_no_provider_deadline_is_written_into_source` greps for
exactly that mistake. Runtime detection beats the data file wherever detection
is possible; the file is the fallback and the explanation.

## Reasons carry no text

A `Reason` is an identifier plus named parameters. The English template lives in
`locales/en.toml`; the UI renders it. A new identifier goes in `reasons.rs`
*and* the catalogue, or `scripts/check-i18n-parity.sh` and
`crates/cli/tests/reasons_catalogue.rs` fail. Adding one is a compatibility
change.

## Never reach across providers

Availability, dependencies and reverse dependencies are read **within one
provider** (INV-04, INV-07, INV-08).
`invariant_08_no_rule_reads_availability_or_dependents_across_providers` checks
it.

## Adding a rule

Follow the `add-policy-rule` skill, or the matching section of
`CONTRIBUTING.md`; they are the same steps. Bump `version` in
`data/policy/policy.toml` if an existing recommendation changes, and record it
in `docs/spec-evolution.md`.
