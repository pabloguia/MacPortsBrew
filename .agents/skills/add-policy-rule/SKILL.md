<!-- GENERATED FILE - do not edit. Generated from .claude/skills/add-policy-rule/SKILL.md
     by scripts/generate-codex-layer.sh; `.claude/` is the source (DP-018). -->
---
name: add-policy-rule
description: Add a named rule to the MacPortsBrew policy engine under crates/policy/src/rules/. Use when a recommendation should change for a reason the existing rules do not express, when a new provider fact must influence an outcome, and whenever the answer would otherwise be a nested conditional somewhere in the application layer.
argument-hint: "[RULE_ID or what the rule should decide]"
allowed-tools: Read, Grep, Glob, Write, Edit, Bash(cargo test:*), Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(${CLAUDE_PROJECT_DIR}/scripts/validate.sh*), Bash(${CLAUDE_PROJECT_DIR}/scripts/check-i18n-parity.sh), Bash(${CLAUDE_PROJECT_DIR}/target/debug/macportsbrew*)
license: Apache-2.0
---

# Add a policy rule

Rule: $ARGUMENTS

This procedure and the "Adding a policy rule" section of `CONTRIBUTING.md` are
the same steps. If you change one, change the other.

1. **Read `docs/spec/03-policy-and-recommendations.md` and INV-14, INV-15,
   INV-17 and INV-19.** A rule is a function of `(facts, data)`: no I/O, no
   clock — the current date is `facts.today` — and no provider name written
   down anywhere.
2. **Create `crates/policy/src/rules/<snake_case_id>.rs`**, declare the module
   in `crates/policy/src/rules/mod.rs`, and add the rule to `all()`. That
   vector is sorted by identifier afterwards, so its position in the literal
   does not affect behaviour.
3. **Implement `Rule`.** `id()` is stable `SCREAMING_SNAKE_CASE` and appears in
   `explain` output and in every reason the rule emits, so changing one is a
   compatibility change. `since()` is the policy version it arrives in.
   `source()` names a specification section, an invariant or a research note —
   a rule with no source is an opinion. `reviewed_on()` is when you last
   checked the reasoning against that source. `evaluate()` returns `None` when
   the rule does not fire, which is a first-class answer that `explain` reports.
4. **Take `priority()` from a constant in `crates/policy/src/rules/mod.rs`,
   never a bare number:** `PRIORITY_MANUAL_REVIEW` (1000), `PRIORITY_RUNTIME`
   (950), `PRIORITY_DESTINATION_VETO` (900), `PRIORITY_SUPPORT_ENDING` (700),
   `PRIORITY_ARTIFACT_ADVANTAGE` (600), `PRIORITY_COEXISTENCE` (500),
   `PRIORITY_NOTHING_TO_DO` (200), `PRIORITY_FALLBACK` (100). Ties break by
   identifier, so the answer never depends on declaration order (INV-15).
   Nothing can outvote `MANUAL_REVIEW`, and the engine enforces that by
   outcome rather than by number (INV-19).
5. **Add the reason identifier in two places or the build fails:** the constant
   in `crates/policy/src/reasons.rs`, with its parameters listed in the doc
   comment, and the English text in `locales/en.toml`.
   `scripts/check-i18n-parity.sh` and `crates/cli/tests/reasons_catalogue.rs`
   assert that the catalogue covers everything the engine can emit. Adding an
   identifier is a compatibility change.
6. **Write the rule's tests in the same file:** the case that fires, the cases
   that do not, and the tie-break if the rule can collide with another.
7. **Check the cross-rule invariants still hold.**
   `cargo test -p macportsbrew-policy` runs
   `invariant_17_every_rule_individually_follows_the_rename`,
   `invariant_15_reproducible_recommendations` and
   `invariant_19_manual_review_cannot_be_outvoted` across every rule at once.
8. **Bump `version` in `data/policy/policy.toml`** if the rule changes an
   existing recommendation, and record the change in `docs/spec-evolution.md`.
9. **Verify:** `scripts/validate.sh`, then `macportsbrew explain <package>` and
   confirm your rule appears under "Rules that fired" or "Rules considered that
   did not fire".

## Refusals

- **No rule may encode "provider X is better".** If you cannot write the rule
  without naming a provider in the code, the fact you need belongs in
  `data/policy/*.toml` with its source and read date, and the rule reads it
  from there.
- **No rule reads a clock, the environment or the filesystem.** Its two
  arguments are all it gets. A rule that reads a clock is a rule whose output
  cannot be reproduced from a snapshot.

## Verification commands

```sh
cargo test -p macportsbrew-policy
scripts/check-i18n-parity.sh
scripts/validate.sh
target/debug/macportsbrew explain <package>
```
