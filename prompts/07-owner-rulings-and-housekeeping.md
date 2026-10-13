<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — Owner rulings and housekeeping (07-OWNER-RULINGS-AND-HOUSEKEEPING)

Pass 06 (report `../docs/passes/06-hardening-before-mutation-report.md`) closed
the gaps found before mutation and left three decisions with the owner. On
2026-10-03 the owner ruled on all three by adopting the recommendations
recorded in `../docs/decisions-pending.md`: DP-025 (a), DP-026 (b), DP-027 (b)
for mutating steps and (c) for reads. This pass implements those rulings,
answers R-16 so DP-027's mechanism is one provider capability rather than a
Homebrew special case, and clears the maintenance list of the pass-06 report.
It does not start the mutating half of the executor.

---

## 1. Read this first

1. `AGENTS.md` — orientation.
2. `docs/spec/00-charter.md`, `docs/spec/01-invariants.md` (INV-10, INV-16,
   INV-17, INV-20).
3. `docs/spec-evolution.md` — SE-016, SE-050, SE-064, SE-015, SE-092.
4. `docs/decisions-pending.md` — DP-025, DP-026, DP-027 as resolved.
5. `docs/passes/06-hardening-before-mutation-report.md` — "Known limitations"
   and "Recommended next tasks". This pass is tasks 2–8 of that list.
6. `docs/research/R-15-homebrew-brew-env.md` and, once written,
   `docs/research/R-16-macports-user-configuration.md`.
7. `docs/schemas/migrate.v1.md`, `docs/spec/06-cli-and-ux.md` (exit codes).

## 2. Authority and precedence

| Tier | Source | Changeable? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | No. If one seems wrong, **stop and ask the human.** |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. |
| 3 | **MUST** in `docs/spec/02..11` | Only with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Change freely; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

The charter edit DP-025 (a) is the owner's explicit decision of 2026-10-03 and
is the only Tier 2 change this pass makes. No Tier 1 text changes.

## 3. Hard boundary for this pass

- **Nothing in the binary dispatches a mutating step.**
  `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`;
  `every_plan_this_planner_builds_is_refused_for_execution_in_this_build` and
  its fixture counterpart stay green and unweakened.
- **The spawn-site allowlist only narrows.** No entry is added to
  `PERMITTED_PROGRAMS` or `PERMITTED_SUBCOMMANDS`.
- **Detecting provider configuration never runs a provider.** It reads files
  that already exist, never creates one, and never follows provider metadata
  into a path. File locations and key lists are data in `data/policy/*.toml`
  with `source` and `read_on` (INV-20).
- **No new dependency.** `Cargo.lock` is unchanged at the end of the pass.
- **A guarded path is edited only under the disclosed escape** the owner arms
  (`MPB_ALLOW_GUARD_EDIT=1`), and every such write is named in the report from
  `.claude/.guard-edit-log`.

## 4. Working protocol

As pass 06, section 4: verify, never touch the real package managers,
`/tooling-recheck` first, delegate research and review, prove each new rule by
mutation in a scratch copy, keep mutating verbs and four-digit years out of
`crates/*/src`, commit incrementally with `scripts/validate.sh` passing.

## 5. Order of work

1. **Record the rulings** (`/decision-pending`): DP-025, DP-026 and DP-027 move
   to Resolved with what was decided and what it obliges.
2. **R-16** (`provider-researcher`), in parallel with items 3–5.
3. **DP-025 (a)**: the charter's example clause.
4. **DP-026 (b)**: `migrate` on a package already at its destination, with the
   other provider not detected, answers *nothing to move* with exit 0 and a
   qualification field in `migrate.v1`'s `nothingToMove`, so a script tells the
   two apart without parsing text. Fixture and schema test.
5. **Maintenance**: the `error.provider_not_found` hint; `{class}` and
   `{state}` reason parameters through labels (SE-064); the concurrency flag
   in `explain` and `doctor` (SE-015); home redaction on the top-level
   `--verbose` error path; the unknown-value fallback of
   `ConcurrentProviderActivity`.
6. **DP-027 (c) for reads**, after R-16: capture records, as a provider fact,
   which provider configuration files exist that can outrank the invocation and
   which reviewed keys they set; `doctor` and the plan's risks report it. The
   mechanism is framed as a capability of any provider (INV-17).
   **DP-027 (b) for mutation**: the execution gate refuses a mutating step for
   a provider whose configuration sets a reviewed key — a refusal reason the
   gate already in force makes testable today.
7. **Guarded housekeeping**, under the escape: `migrate` in
   `scripts/run-fixture-matrix.sh`; `.claude/rules/executor.md` and its
   `CLAUDE.md` entry; `.claude/README.md` dates; skills' `allowed-tools`
   wording.
8. **Coverage measured** with `scripts/validate.sh --full`.

## 6. Definition of done

- [ ] DP-025, DP-026 and DP-027 are in Resolved, each with its obligations.
- [ ] A dated R-16 note exists and the checklist entry says answered.
- [ ] `migrate` answers the one-provider case as DP-026 (b), with a schema
      field and a fixture test proved by mutation.
- [ ] Provider configuration that can outrank the invocation is captured and
      reported for both providers from data, with tests, and a mutating step
      against it is refused by a named gate test.
- [ ] Each maintenance item is closed with a test or stated as carried.
- [ ] Section 3 holds.
- [ ] `/preflight` passes, or the report names what was skipped and why.
- [ ] Every invariant touched points at its test.
- [ ] `docs/spec-evolution.md` records every material change.
- [ ] The reviewers have run and their findings are resolved or accepted.

## 7. End-of-pass report

`/pass-report` to `docs/passes/07-owner-rulings-and-housekeeping-report.md`.
Carry forward the mutating half of the executor (pass-06 report, task 9) and
R-15's residuals.
