<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 7 — what the user agreed to is what runs

> **Run through work order 16 (SE-109); its §1 item 8 overrides this file's
> plan-mode, merge, push, dependency and stop instructions.**
> (`prompts/16-complete-mutating-product.md`)

Work order for one session. Parent: [`08-the-mutating-executor.md`](08-the-mutating-executor.md);
its §3 binds unchanged. Written 2026-10-18. Run it **after slice 6 has merged**:
both slices change `crates/cli/src/executor/mod.rs` and
`crates/application/src/executor.rs`. **`THIS_BUILD_DISPATCHES_MUTATIONS` stays
`false`.**

Today `migrate --yes` executes a plan nobody has seen (obligation 8 of
`plan.v1.md`), elevation consent is built but never asked, a line typed before
the consent prompt can answer it (slice 4 security review SR 10), and the
configuration token does not say which program it was re-read for (SR 9). This
slice closes the four.

## 0. Read first, in this order

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/01-invariants.md` — INV-02, 03, 06, 15
   and 17. Tier 1: if one looks wrong, stop and ask the owner.
2. **[`docs/passes/08-pre-flip-design-notes.md`](../docs/passes/08-pre-flip-design-notes.md),
   "Slice 7".** The starting design, with `file:line`; re-read each line before
   relying on it.
3. `docs/schemas/plan.v1.md`, "What an executor inherits": obligations 8, 9
   and 10. `docs/schemas/migrate.v1.md`.
4. `docs/spec/04-migration-and-safety.md`, "Privilege"; `docs/spec/06-cli-and-ux.md`,
   the command list and the exit codes (6 is "user aborted").
5. `docs/spec-evolution.md`: SE-092 (the decision that `migrate` re-plans and
   never deserialises a plan; slice 4 D10; SR 9 and SR 10), SE-016.
6. `docs/passes/08-slice-4-plan.md` §9 "As built" and §10.
7. `docs/decisions-pending.md`: **DP-046 and DP-049.** Read their status first.
8. `docs/adr/0004-dependency-gate.md` (the dependency gate).
9. The code, opened with **Read**: `crates/cli/src/app/migrate.rs`,
   `crates/cli/src/consent.rs`, `crates/cli/src/executor/mod.rs`;
   `crates/application/src/consent.rs`, `configuration_recheck.rs`,
   `executor.rs`, `execution_gate.rs`; `crates/domain/src/error.rs`;
   `crates/cli/tests/history_preconditions.rs`, `migrate.rs`,
   `executor_wiring.rs`.

## 1. What this slice delivers

Four parts, in this order, so the reviewers see the consent wiring in the executor
alone and last.

### Part D — SR 9: the configuration token carries its program

As the design notes' Part D.

### Part C — SR 10: a code in the consent phrase

As the design notes' Part C: `allow step N <code>`, a fresh code per request
from `/dev/urandom` through `std::fs`, no code → refuse without reading; new
`consent.*` keys in en and pt-BR.

### Part A — the plan digest

**Gated on DP-049.** If the owner has ruled (a): write the ADR for `sha2` under
ADR-0004's gate (licence, maintenance, `cargo deny`, what it pulls in),
`/adr`, then build as the design notes' Part A. If DP-049 is still open: write
the ADR as **Proposed**, do not add the dependency, leave Part A's tripwire
failing, and say so in the report. If the owner ruled (b): build the interactive
route instead and record why in SE.

### Part B — consent wired

As the design notes' Part B: the consent seam, asked under the lock after the
recapture and before the marker; `ErrorId::UserAborted` (exit 6); a covering
token carried to the site. **Both refusals of elevated dispatch stay**: the
gate's (DP-046's default) and the guard's `ElevationNotNone` halt, pinned by a
new `invariant_06_*` test — relaxing the guard belongs to the pass that builds an
elevated spawn mechanism, not to this one. The report declares that the prompt
is unreachable in a shipped build. Decide in the plan whether to re-run the
recapture after a slow consent answer.

### Then

Rewrite the tripwires as positive structural checks (design notes, "Tripwires,
rewritten"), and the `crates/cli/tests/migrate.rs` obligation-8 test, which
fails on `migration_start(` regardless of the tie.

## 2. Hard boundary

Slice 5 §2 holds in full, except that **one** new dependency may enter, only
through Part A's ADR and only if DP-049 is ruled (a). In addition:

- `execution_gate.rs`'s refusals of elevated steps are unchanged:
  `git diff main -- crates/application/src/execution_gate.rs` shows no change to
  them, and the tests that pin them pass unchanged.
- No `sudo`, no new permitted program, no new spawn site.
- `migrate.v1` changes are additive (`planDigest`), documented and in a checked
  golden.

## 3. How to work

Slice 5 §3 applies, except that step 3's order is this work order's §1 (plan at `docs/passes/08-slice-7-plan.md`, report at
`docs/passes/08-slice-7-report.md`), and slice 6 §3's extra rules: reviewers on
the plan and on every part's commit, on the strongest model available;
`security-reviewer` reads Part B and any `unsafe`-adjacent text line by line.

The mutation proof includes, at least: the program comparison removed; the code
dropped from the phrase or reused across requests; a failing nonce falling back
to an empty code; `tie` always `Ok`; the digest over a subset of fields or over
the redacted document; the tie check moved after the lock; the guard letting an elevated command through when a token is present; a
declined answer that still writes the marker.

Record the slice in a new SE entry (cross-referencing SE-092 and SE-016) and
update `plan.v1.md` obligations 8–10, `migrate.v1.md`, 04 "Privilege", 06 (the
flag and exit 6), CHANGELOG, the wiki CLI reference and both locales in the same
changes.

## 4. Definition of done

- [ ] `the_switch_needs_consent_wired` and (if DP-049 is ruled) the two
      obligation-8 tests pass with the switch set to `true` — in an isolated
      worktree through a subagent, never in this checkout and never committed.
- [ ] Every rule has a named test proved by mutation; survivors argued.
- [ ] §2 holds, each item shown by a command or a test.
- [ ] Reviewers on the plan, on each part and at the end; findings resolved or
      accepted in writing.
- [ ] Docs, SE entry and "As built" written; report written; topic branch pushed
      and a pull request opened; merged only with every required check green (SE-108).
