<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 6 — the checks that guard a removal

Work order for one session. Parent: [`08-the-mutating-executor.md`](08-the-mutating-executor.md);
its §3 binds unchanged. Written 2026-10-18, after slice 5 merged (PR #10).
**`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false` for the whole slice.**

Three of the tests that fail the build the day the switch is turned on guard the
same thing — that nothing is removed unless the replacement is installed,
active and still there (INV-01). This slice closes all three:

- a destination whose artifact is unavailable becomes `MANUAL_REVIEW` (04,
  "Undoing a migration");
- `VERIFY_DESTINATION` can evaluate `PROVIDER_REPORTS_ACTIVE`, the check 04's
  verification contract names first;
- the destination is read again immediately before `REMOVE_SOURCE` is
  dispatched;

and builds DP-048's default, that the first mutating release refuses
`REMOVE_SOURCE` by release policy rather than by coincidence.

A real plan still halts after this slice: `VERIFY_DESTINATION` carries four more
checks no build can evaluate yet (DP-051), and `SET_PRIMARY` and
`VERIFY_RESOLUTION` halt next (slice 8). The destination-contract tripwire is
**widened** here to every check, so it keeps failing until DP-051 is settled.
Say all of this in the report; a tripwire passing is not a removal working.

## 0. Read first, in this order

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/01-invariants.md` — INV-01, 02, 03, 15,
   17 and 19 in particular. Tier 1: if one looks wrong, stop and ask the owner.
2. **[`docs/passes/08-pre-flip-design-notes.md`](../docs/passes/08-pre-flip-design-notes.md),
   "What stands between `main` and the flip" and "Slice 6".** It is the starting
   design, with `file:line` for everything; re-read each line before relying on
   it.
3. `docs/spec/04-migration-and-safety.md`: "Verification contract" (including
   "Verifying a removal"), "Undoing a migration", the residual windows next to
   DP-030.
4. `docs/schemas/plan.v1.md`: success semantics, the checks table, "What an
   executor inherits".
5. `docs/spec-evolution.md`: SE-094 (the softened `PROVIDER_REPORTS_ACTIVE`
   claim and decision D11), SE-092 (slice 4 review M6a; slice 5 D8).
6. `docs/research/R-20-*.md`, written by work order 14 (item A). **If it does
   not exist, Part B cannot start**: do Part A, then stop and say so. Do not
   write a second R-20 here.
7. `docs/decisions-pending.md`: DP-046, DP-048, DP-050 and DP-051.
   `docs/spec-evolution.md` SE-016 item 3 and SE-050 item 6, and
   `docs/research/R-15-*`: the security record on `brew info --json=v2`.
8. The code, opened with **Read** so `.claude/rules/` loads the chapter:
   `crates/application/src/planner.rs`, `execution_gate.rs`, `executor.rs`,
   `executor/tests.rs`, `provider_commands.rs`;
   `crates/domain/src/plan.rs`, `verification.rs`, `availability.rs`;
   `crates/cli/src/executor/mod.rs`; `data/policy/provider-commands.toml`;
   `crates/cli/tests/history_preconditions.rs`, `executor_wiring.rs`.

## 1. What this slice delivers

Four parts, in this order: A is small and independent, B needs R-20, C reuses
B's check, D is independent but lands last so its tests see the others.

### Part A — `MANUAL_REVIEW` for an unavailable artifact

As the design notes' Part A: one blocking reason for `Unavailable`,
`UnsupportedPlatform` and `Disabled`, pushed after the existing ones; a
`SourceBuildRequired` destination stays non-blocking (DP-050's default) and 04
gains a **declared deviation** note pointing at DP-050 — its MUST example is not
rewritten before the owner rules; then the tripwire becomes a positive check
held whatever the switch is.

### Part B — `PROVIDER_REPORTS_ACTIVE` evaluable on `VERIFY_DESTINATION`

As the design notes' Part B: a `verify_active` read verb over reads that are
**already permitted** (`brew info --json=v2 --installed`, `port -q installed`),
two success semantics that fail closed, `ActiveNotReported` for everything they
cannot prove (keg-only formulae and casks halt), the gate pin by position,
`evaluable_in_this_build` true for `VerifyDestination` only, and the tripwire
widened to every check a planner-built `VERIFY_DESTINATION` carries.

**Before code**, the plan weighs the security record on `brew info --json=v2
--installed` (SE-016 item 3, SE-050 item 6, R-15 E7: it can load tap Ruby
through a `brew.env`, and it writes to the user's cache) against today's `list
--versions <name>`, and `security-reviewer` rules on moving it into the run loop.

**Stop conditions.** If R-20 shows that neither read can prove "active" for one
provider, do not ship the check for the other alone (INV-17): record the finding
in this slice's SE entry, leave the tripwire failing, and write a DP. If the only way forward
grows the process allowlist (`crates/adapters/src/process/`), that is the
owner's decision: write a DP, do not edit the allowlist.

### Part C — re-verify the destination before removal

As the design notes' Part C: a pure `destination_still_verified` in
`application/src/executor.rs`, and the shell runs the verify step's own pinned
commands for `RemoveSource` only, after the lock and configuration re-checks and
immediately before dispatch. The tripwire becomes a structural check. Record
DP-048's residual in 04 next to DP-030's.

### Part D — removal refused by release policy

As the design notes' Part D: a build-level constant beside the switch that makes
the gate refuse any plan holding `REMOVE_SOURCE`, with a named reason and its
own pin test and flip tripwire, so DP-048's default no longer rests on the
coincidence that removal elevates. If the owner has ruled DP-048 (a), stop and
ask whether Part D is still wanted.

Name every test that proves an invariant `invariant_NN_*`.

### Not in this slice

The plan digest and consent (slice 7); `SET_PRIMARY`/`VERIFY_RESOLUTION`
(slice 8); the detector and the flip proposal (slice 9); any change to the
process allowlist; any elevation.

## 2. Hard boundary

Slice 5 §2 holds in full (the switch, no dispatch, no mutating command, no
elevation, writes only to a scratch state directory in tests, no new dependency,
`Cargo.lock` unchanged, no guarded path, invariants read-only). In addition:

- `git diff --stat main -- crates/adapters/src/process/` is empty at the end.
- The plan order in `planner.rs` (the sequence of step kinds) is unchanged; a
  test that builds the fixture plans and compares step kinds before and after
  proves it.
- `every_plan_this_planner_builds_is_refused_for_execution_in_this_build` and
  its fixture counterpart are not weakened.

## 3. How to work

Slice 5 §3 applies — "The protocol" (steps 1–8, except that step 3's order is this work order's §1, with this slice's plan
at `docs/passes/08-slice-6-plan.md` and report at
`docs/passes/08-slice-6-report.md`) and "How this repository bites". Also:

- **Every file you change here is on `AGENTS.md`'s "Dangerous to change" list.**
  Run `spec-guardian` and `security-reviewer` on the plan before any code, and
  again on each part's commit before starting the next — not only at the end.
  If you are not running on the strongest model available, start both reviewers
  on it (the Agent tool's `model` parameter).
- One commit per part, plus one for the docs.
- The mutation proof includes, at least: each state dropped from the `matches!`;
  the new push moved first; the marker check removed; a substring instead of a
  token match; the gate pin checking one command; `evaluable` on every step kind;
  an "installed" read answering the active check; the widened tripwire checking
  one check only; the re-verify dropped, moved before the lock check, or using
  another step's commands; the removal refusal dropped or keyed on the wrong
  step kind.
- Record a new SE entry — the next free identifier (SE-101 on 2026-10-18; passes
  14 and 15 may take it first) — for the decisions of this slice, settling
  SE-094's softened claim, with `/spec-evolution`; update 04, `plan.v1.md`,
  `provider-commands.toml` provenance and both locales in the same changes.

## 4. Definition of done

- [ ] With the switch set to `true` — in an isolated worktree through a
      subagent, as the mutation proof runs, never in this checkout and never
      committed — the unavailable-artifact and window tripwires pass, the widened
      destination-contract tripwire fails naming exactly DP-051's four checks,
      and the report lists every other test that still fails there.
- [ ] Each rule has a named test proved by mutation; survivors argued.
- [ ] §2 holds, each item shown by a command or a test.
- [ ] Reviewers on the plan, on each part and at the end; findings resolved or
      accepted in writing in the plan's §10.
- [ ] The SE entry, 04, `plan.v1.md`, CHANGELOG updated; "As built" written; report
      written; topic branch pushed and a pull request opened; merged only with every required check green (SE-108).
