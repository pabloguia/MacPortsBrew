<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 8 — `SET_PRIMARY` and `VERIFY_RESOLUTION`, so a run can complete

Work order for one session. Parent: [`08-the-mutating-executor.md`](08-the-mutating-executor.md);
its §3 binds unchanged. Written 2026-10-18. Run it **after slices 6 and 7 have
merged**. **`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`.**

Every plan the planner builds today halts at `SET_PRIMARY` (it carries a
`macportsbrew primary` command that does not exist) or at `VERIFY_RESOLUTION`
(which asserts nothing), so a run ends `PARTIAL` and `REMOVE_SOURCE` is never
reached. Tests reach `COMPLETED` only by deleting those steps. This slice makes
the two steps honest **without writing anything outside the providers**: the
first release reports the primary and verifies what resolves; it does not build
the shim directory or touch shell configuration.

## 0. Read first, in this order

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/01-invariants.md` — INV-01, 03, 11, 12,
   15 and 17. Tier 1: if one looks wrong, stop and ask the owner.
2. `docs/decisions-pending.md`: **DP-047** (and DP-046, DP-048, DP-051). If the owner
   ruled DP-047 (a) — build layer 4 — **stop**: this work order does not cover
   it, and a new one must be written in plan mode.
3. **[`docs/passes/08-pre-flip-design-notes.md`](../docs/passes/08-pre-flip-design-notes.md),
   "Slice 8".** The starting design, with `file:line`.
4. `docs/spec/05-environment-and-primary-provider.md` in full (it is short), and
   SE-036.
5. `docs/spec/04-migration-and-safety.md`: "Verification contract", "Undoing a
   migration" (restore reverts a primary it changed).
6. `docs/schemas/plan.v1.md`, `history.v2.md`, `migrate.v1.md`.
7. `docs/spec-evolution.md`: SE-092 ("Not closed (2)", `PRIMARY_IS_SHIM`, S8 D3).
8. The code, opened with **Read**: `crates/application/src/planner.rs`,
   `execution_gate.rs`, `executor.rs`, `executor/tests.rs`, `capture.rs`,
   `history.rs`; `crates/domain/src/plan.rs`, `resolution.rs`;
   `crates/cli/src/executor/mod.rs`; `crates/adapters/src/system/machine.rs`,
   `crates/adapters/src/filesystem.rs`.

## 1. What this slice delivers

As the design notes' "Slice 8 — Design", under DP-047 (b):

1. **Spec first.** An SE entry ("the first release performs layer 1 in the
   run"), addenda to SE-036 and SE-092, and the edits to 05 and 04 that say
   so. Have `spec-guardian` review the SE entry **before code**. The INV-17
   question — Homebrew→MacPorts refused on a machine whose resolution favours
   Homebrew — is the owner's, under DP-047; `spec-guardian`'s view is advisory,
   and the SE entry records the reading as the owner ruled it, or as **pending
   DP-047**, never as settled by the agent.
2. **Domain:** `VerificationCheck::ResolvesTo`, `Disposition::NothingToChange`,
   whatever carries the primary mechanism on `SET_PRIMARY`. `SET_PRIMARY` stays
   a mutating kind.
3. **Planner:** the decision from the captured resolution (not from comparing
   `bin` indices; each provider's `bin` from the snapshot, never a literal); the
   `TOOL` command and `SET_PRIMARY`'s unevaluable `PROVIDER_REPORTS_ACTIVE`
   removed, said in the SE entry; `primary_after` from the prediction.
4. **Gate:** the new evaluable check; `PRIMARY_NEEDS_SHIM` refused before any
   install (exit 4), with remediation naming layers 1 and 2 and that a `PATH`
   reorder — global priority, a separate confirmed operation under INV-12 — is not offered in this release.
5. **Executor:** a `ResolutionSite` seam beside `ReadSite`, evaluated in process
   (no spawn), unevaluable — never a pass — under replay or snapshot (INV-15);
   `NothingToChange` carried on `SET_PRIMARY` only.
6. **History:** `primaryAfter` is what was observed to resolve; decide with
   `spec-guardian` whether that is `history.v3`; re-run recognition for plans
   that use no shim.
7. **Cut the test narrowing back** in `executor/tests.rs` so it no longer
   deletes `SET_PRIMARY` or `VERIFY_RESOLUTION`; it may still strip the
   `VERIFY_DESTINATION` checks DP-051 leaves unevaluable, and says so.

A package whose executable is not its own name is refused for now; record the
decision in the SE entry. Shim recognition waits for layer 4.

**Stop condition.** If making a run reach `COMPLETED` would need the planner to
stop emitting any check of 04's verification contract, stop and write a DP: that
is a deviation from a MUST, and once removal is on, from INV-01.

## 2. Hard boundary

Slice 5 §2 holds in full. In addition:

- **Nothing writes under the user's home except the state directory**, proven by
  `no_run_writes_outside_the_state_directory_or_the_providers` (a temporary home
  in which no shim directory and no shell file appears).
- `git diff --stat main -- crates/adapters/src/process/` is empty: no new spawn,
  no new permitted program.
- No fixture is invented: a permuted `PATH` is an invented fixture. Use the
  captured `fixtures/snapshots/both-providers-intel-macos26-live.json`; if a
  second machine's order is needed, say so in the report (the owner captures it).
- Shell configuration is denied to your tools on purpose; a Rust test may create
  a temporary dotfile at runtime only to prove it was *not* changed.

## 3. How to work

Slice 5 §3 applies, except that step 3's order is this work order's §1 (plan at `docs/passes/08-slice-8-plan.md`, report at
`docs/passes/08-slice-8-report.md`), and slice 6 §3's extra rules (reviewers on
the plan and on every commit, on the strongest model available).

The mutation proof includes, at least: a shadowing non-provider entry ignored;
relative entries counted; the `authoritative` test dropped; a provider
comparison negated; `NothingToChange` carried on every kind; the replay case
answering a pass. Name every test that proves an invariant `invariant_NN_*`.

## 4. Definition of done

- [ ] Under the test allowance, a planner-built retain-source plan passes
      `SET_PRIMARY` and `VERIFY_RESOLUTION` with no step removed — and, once
      DP-051 lets every `VERIFY_DESTINATION` check be evaluated, reaches
      `COMPLETED`; until then the report says it halts there and nowhere later.
      A plan that needs a shim, or whose name a non-provider entry shadows, is
      refused before any install.
- [ ] Every rule has a named test proved by mutation; survivors argued.
- [ ] §2 holds, each item shown by a command or a test.
- [ ] Reviewers on the plan, on each commit and at the end; findings resolved or
      accepted in writing.
- [ ] 05, 04, schemas, SE entries, CHANGELOG, wiki, both locales; "As built";
      report; topic branch pushed and a pull request opened; merged only with every required check green (SE-108).
