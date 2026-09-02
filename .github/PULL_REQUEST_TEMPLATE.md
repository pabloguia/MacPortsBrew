<!-- SPDX-License-Identifier: Apache-2.0 -->

## What this changes

<!-- One paragraph. What the change does and why it is the right change. -->

## Invariants

<!--
Name every invariant this change touches, and the test that defends it.
`grep -rn "fn invariant_" crates/*/tests crates/*/src` finds them.
A change that touches an invariant and cannot point at its test is incomplete.
-->

- Invariants touched:
- Tests that defend them:

## Specification

<!--
Tier 1 (`docs/spec/01-invariants.md`) is not changeable here — if one looks
wrong, stop and raise it. Tier 2 (`docs/spec/00-charter.md`) needs an explicit
human decision. Tiers 3 to 5 may be improved, never silently: a material change
lands with its `docs/spec-evolution.md` entry in the same commit.
-->

- [ ] No specification change
- [ ] Tier 3–5 changed, and `docs/spec-evolution.md` has the entry (id: `SE-___`)
- [ ] A decision that belongs to the human is recorded in `docs/decisions-pending.md` (id: `DP-___`)
- [ ] An ADR was added or superseded (`docs/adr/____`)

## Provider claims

<!--
Every new statement about Homebrew or MacPorts behaviour is (a) observed on a
machine with a read-only command, (b) cited from official documentation with
the date read, or (c) runtime capability detection. There is no fourth option.
Dated facts live in `data/policy/*.toml` with `source` and `read_on`, never in
control flow (INV-20).
-->

- [ ] This change makes no new claim about provider behaviour
- [ ] Every new claim carries its source and read date, or is conditional, or is capability detection
- [ ] Any dated fact went into `data/policy/*.toml`, not into code
- [ ] `data/policy/policy.toml` `version` was bumped if a recommendation changed

## Both directions

<!--
Homebrew and MacPorts are peers (INV-17). Anything that works one way works the
other, through the same planner and the same policy engine.
-->

- [ ] Not applicable
- [ ] This change is symmetric, and `invariant_17_*` still passes

## Checks

- [ ] `scripts/validate.sh` passes
- [ ] New or changed user-facing text goes through the i18n catalogue, and `scripts/check-i18n-parity.sh` passes
- [ ] Fixtures used by new tests are captured, carry provenance, and are redacted
- [ ] `README.md` and `wiki/` are accurate for any user-visible change
- [ ] `CHANGELOG.md` updated under `Unreleased` if this is user-visible

## Dangerous areas

<!--
Tick anything this touches. These get a closer read, and the security and
specification reviews before merge.
-->

- [ ] The process executor (`crates/adapters/src/process/`)
- [ ] Planner step ordering (`crates/application/src/planner.rs`)
- [ ] Verification or safety logic
- [ ] Privilege, elevation or the state directory
- [ ] The gate scripts (`scripts/`, `.claude/hooks/`)
- [ ] None of these
