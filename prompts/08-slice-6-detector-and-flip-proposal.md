<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 6 — the owner's detector ruling, and the proposal to turn mutations on

Work order for one session. Parent: [`08-the-mutating-executor.md`](08-the-mutating-executor.md),
item 2's unbuilt half and item 7. Its §3 binds unchanged. **Do not start this
slice until the owner has ruled on DP-030** (and DP-028, which the proposal
depends on); if they have not, stop and say so.

## 0. Read first, in this order

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/01-invariants.md`.
2. `docs/decisions-pending.md` — DP-028, DP-029, DP-030 and their rulings.
3. `docs/research/R-17-provider-mutation-locks.md`, `R-18-*`.
4. `docs/spec-evolution.md`: SE-015 (the slice 2 addendum), SE-092 (all
   "owed" lists), SE-017.
5. Every "As built" section of `docs/passes/08-*-plan.md`.
6. `crates/cli/tests/history_preconditions.rs` — every tripwire the flip must
   satisfy.

## 1. What this slice delivers

### Part A — the detector, as the owner ruled DP-030

- **(a) stay owed:** nothing is built; Part B's proposal says the flip is
  blocked by `mutations_cannot_dispatch_while_a_live_run_still_says_not_checked`
  and asks the owner to rule again.
- **(b) `F_GETLK`:** an ADR for the `libc` dependency (ADR-0001 gates) and the
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
release's defaults (`--retain-source` on; which step kinds are enabled — propose
the smallest set, e.g. `ADD_DESTINATION`/`SET_PRIMARY` only with
`REMOVE_SOURCE` still refused); the acceptance protocol pass 11 will run; and
the rollback plan if the release misbehaves. **The agent never flips the
switch.** The owner does, in a message, after reading it.

## 2. Hard boundary

Slice 2 §2. A new dependency only through its ADR in this slice and only under
(b). The switch stays `false`.

## 3. Plan, build, prove, review, record

As slice 4 §3, with `security-reviewer` reviewing any `unsafe` line by line.

## 4. Definition of done

- [ ] DP-030 implemented as ruled, or recorded as still owed.
- [ ] The flip proposal is in `docs/decisions-pending.md`, every claim in it
      pointing at a file:line or a test name.
- [ ] Reviewers run on the slice **and on the proposal**; "As built" written;
      `/pass-report` for pass 08 to `docs/passes/08-the-mutating-executor-report.md`.
