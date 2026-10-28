<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 9 — the owner's detector ruling, and the proposal to turn mutations on

> **Run through work order 16 (SE-109); its §1 item 8 overrides this file's
> plan-mode, merge, push, dependency and stop instructions.**
> (`prompts/16-complete-mutating-product.md`)

Work order for one session. Parent: [`08-the-mutating-executor.md`](08-the-mutating-executor.md),
item 2's unbuilt half and item 7. Its §3 binds unchanged. Written as "slice 6"
and renumbered on 2026-10-18, when slices 6–8 (the checks before removal, the
plan tie and consent, the primary and resolution steps) were inserted before it.
**Do not start this slice until slices 6, 7 and 8 have merged and the owner has
ruled on DP-030** (and DP-028, which the proposal depends on); if not, stop and
say so.

## 0. Read first, in this order

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/01-invariants.md`.
2. `docs/decisions-pending.md` — DP-028, DP-029, DP-030, and DP-046 to DP-051,
   with their rulings.
3. `docs/research/R-17-provider-mutation-locks.md`, `R-18-*`; under DP-030 (b),
   also `docs/adr/0004-dependency-gate.md` and
   `docs/adr/0006-no-libc-and-the-timeout-kills-one-process.md`.
4. `docs/spec-evolution.md`: SE-015 (the slice 2 addendum), SE-092 (all
   "owed" lists), SE-017, and the entries slices 6–8 wrote.
5. Every "As built" section of `docs/passes/08-*-plan.md`, and
   `docs/passes/08-pre-flip-design-notes.md` ("What stands between `main` and
   the flip").
6. `crates/cli/tests/history_preconditions.rs` — every tripwire the flip must
   satisfy.

## 1. What this slice delivers

### Part A — the detector, as the owner ruled DP-030

- **(a) stay owed:** nothing is built; Part B's proposal says the flip is
  blocked by `mutations_cannot_dispatch_while_a_live_run_still_says_not_checked`
  and asks the owner to rule again.
- **(b) `F_GETLK`:** an ADR for the `libc` dependency (ADR-0004's gate; it must supersede or amend ADR-0006, which rejected `libc`) and the
  one reviewed `unsafe` call, scoped to one adapter module with
  `#[allow(unsafe_code)]` on that item only; a provider-neutral detector trait in
  `domain`, provider lock paths from `data/policy/` (never above `adapters/`);
  claims exactly R-17's admissible ones — "a lock is held now", never "idle";
  every `open`/`F_GETLK` error is the unknown fallback; `Unknowable` under
  replay (INV-15); INV-17 review of the providers' asymmetry. Then the
  application-level refusal on `Observed` and on the unknown fallback.
- **(c)** only as the owner worded it.

### Part B — the flip proposal (item 7)

A written proposal in `docs/decisions-pending.md` (`/decision-pending`) for
setting `THIS_BUILD_DISPATCHES_MUTATIONS = true`, containing: every tripwire and
what satisfies it (file:line); every invariant the mutating path touches and its
test by name; what is still owed and why the owner may accept it; the first
release's defaults as the owner ruled DP-046 (elevated steps), DP-047 (primary
mechanism) and DP-048 (removal) — absent a ruling, their reversible defaults:
no elevated step, `SET_PRIMARY` report-only with `PRIMARY_NEEDS_SHIM` refused,
`--retain-source` forced and `REMOVE_SOURCE` refused at the gate; a plain
statement of which direction and which packages that leaves runnable on the
captured fixtures, and how that squares with INV-17 (`spec-guardian` reviews
the argument; the reading of a Tier 1 invariant is the owner's, ruled under
DP-046); the acceptance protocol pass 11 will run;
and the rollback plan if the release misbehaves. The proposal must also say,
from `docs/passes/08-pre-flip-design-notes.md`'s table, that the
`test-support` release check (work order 15) is in place. **The agent never
flips the switch.** The owner does, in a message, after reading it.

## 2. Hard boundary

Slice 2 §2. A new dependency only through its ADR in this slice and only under
(b). The switch stays `false`.

## 3. Plan, build, prove, review, record

As slice 5 §3, with `security-reviewer` reviewing any `unsafe` line by line.
Write the plan to `docs/passes/08-slice-9-plan.md`. This slice's report is the
pass 08 report, `docs/passes/08-the-mutating-executor-report.md` (§4), not a
separate slice report.

## 4. Definition of done

- [ ] DP-030 implemented as ruled, or recorded as still owed.
- [ ] The flip proposal is in `docs/decisions-pending.md`, every claim in it
      pointing at a file:line or a test name.
- [ ] Reviewers run on the slice **and on the proposal**; "As built" written;
      `/pass-report` for pass 08 to `docs/passes/08-the-mutating-executor-report.md`.
