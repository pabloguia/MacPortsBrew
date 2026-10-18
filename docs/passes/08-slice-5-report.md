<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 5 — the brackets performed, `history`, `restore --plan` and the stranded marker: report

Branch `pass/08-slice-5-history-and-restore`. Plan: [`08-slice-5-plan.md`](08-slice-5-plan.md)
(decisions D1–D18, the plan-time review in §7, "As built" in §9, the end-of-slice
review in §10). Rationale: SE-092 in `docs/spec-evolution.md`.

**`THIS_BUILD_DISPATCHES_MUTATIONS` is still `false`.** Nothing in this slice
executes anything: a restore is a plan, `restore --run` without `--plan` is
refused by the same gate `migrate --yes` reaches, and the performed brackets are
reachable only through the test-only allowance. No guarded path, `Cargo.lock` or
`docs/spec/01-invariants.md` changed; no dependency, spawn site or `unsafe` was
added; no mutating `brew` or `port` command was run, in a test or otherwise.

## Progress

| Part | Commit | What |
|---|---|---|
| plan | `b7a6152` | plan revision 2 after `spec-guardian` and `security-reviewer`, and the 2026-10-17 tooling re-check |
| A | `3c73cc7` | `SNAPSHOT_BEFORE` and `RECORD_HISTORY` performed behind the switch; `history.v2`, `run-marker.v2`; a record never certifies its own write |
| B | `c842ad8` | `history`, `history show <id>`; restorability derived on read; `RUN_NOT_FOUND` (exit 8) |
| C | `e790948` | `restore --plan --run <id>`; `restore.v1`; `--snapshot` not built (exit 1) |
| D | `d72a785` | `doctor` shows a stranded run; `history acknowledge <id> --yes`; the by-hand remedy removed |
| review | `67ff1a4` | findings of the three end-of-slice reviews |
| mutation | `7bd6b93` | tests from the mutation proof; a mutating step stored `VERIFIED` is refused |

Every commit passed `scripts/validate.sh`. Coverage at the end: application
95.72%, adapters 95.23% (floors 95); cli 87.59% (reported, no floor); overall
94.13% (floor 90). The domain and policy lines of that run were not read in this
session; CI is the authority.

## What a person can now do

`macportsbrew history` and `history show <id>` read the runs this tool recorded
and say whether each can be put back, derived from the record, the snapshot and
whether the interrupted-run marker still names it. `restore --plan --run <id>`
shows the way back as an ordinary plan with notes about what it cannot say.
`doctor` reports an interrupted run in one of three described states, and
`history acknowledge <id> --yes` is the one deliberate way to remove a marker.
Because nothing executes yet, a user's machine has no runs to show.

## Findings worth the owner's attention

1. **The reverse mapping needs the same evidence as the forward one.** A restore
   of a run that removed its source stops at `MANUAL_REVIEW` unless a curated row
   covers the package. Declared in 04 and SE-092; pinned by a test. Letting the
   record attest the mapping is a planner change, hence carried.
2. **The 04 MUST that an unavailable restore artifact becomes `MANUAL_REVIEW` is
   not met for any plan, `migrate` included** (the planner raises none). Declared;
   held by `the_switch_needs_manual_review_for_an_unavailable_artifact`.
3. **A real run still ends `PARTIAL`.** `SET_PRIMARY` and `VERIFY_RESOLUTION` are
   not performed, so only a narrowed plan reaches `COMPLETED`; the flip proposal
   inherits it.
4. **D-M4 is amended** (slice 2): `history acknowledge` is the one path other than
   a run's own `clear` that removes a marker. Recorded in SE-092 D13 and 04.
5. **A hand-edited history file could claim a known exit 0** (found by the
   mutation proof): the reader now refuses a mutating step stored `VERIFIED`.

## Mutation proof

126 mutants, applied one at a time in an isolated worktree by a subagent, which
ran the narrow tests, then the crate's, and reverted each. Seven real gaps; each
is now killed by a new test, shown to fail against the mutant and pass without it
(restorability row order and content, the request's package spelling, the plan's
destination provider, a record stored `VERIFIED` on the record step, and a
preview of acknowledging another run's marker). I read the survivors; the ones
that stay are equivalent or need a fault-injection seam that does not exist:

- the `captured_at` equality in the snapshot read-back (a single process always
  reads back what it wrote; the check is defence in depth);
- in `make_durable`, the second read, the equality check and both fsyncs, and
  "clear the marker before `make_durable`" (a file's `sync_all` cannot be made to
  fail in a test; this is the only survivor I would pin if a seam were added,
  modelled on `ReadBack`);
- the `flock` error arm and the `metadata()` failure arm in `probe`;
- the second `refuse_other_run` and the second marker read under the lock (they
  defend a race between two processes).

"Skip the lock acquire" does not compile (`read` and `clear` take the lock by
type). With the switch set to `true`, 39 tests fail across the workspace,
including every `the_switch_needs_` test that reads it; two do not read it and
are justified in the proof (the doctor check is a positive fact held whatever the
switch is; the brackets test is a positive check of what this slice delivered and
its name over-promises).

## End-of-slice reviews

`spec-guardian`, `security-reviewer` and `docs-auditor` over `main...HEAD`: no
invariant violation, no path by which a restore executes, no High or Medium
security finding. Every finding and its disposition is in plan §10. Accepted, not
fixed: orphan snapshots from a run that stops before any change (security L-4);
the removal decision resting on a snapshot of unbounded age (L-5, for the
executing slice); the `readBack` flag a marker would need to tell a disagreeing
read-back apart (L-3, `run-marker.v3`).

## Still carried (each behind its tripwire, untouched)

Consent wiring; the executed plan tied to the one shown (obligation 8); the window
between `VERIFY_DESTINATION` and `REMOVE_SOURCE`; `PROVIDER_REPORTS_ACTIVE`
evaluable; release builds free of test-support; the observed/unchecked-provider
refusal (DP-030); shim-backed recognition; the umask test (DP-029); a dedicated
provider-command-failed identifier; typed-ahead consent input and the token
carrying its program. No owner decision arose that needed `decisions-pending.md`.

## Tooling re-check (2026-10-17, `/tooling-recheck`)

Re-verified all three groups; notes are in `docs/research/tooling/2026-10-17-*.md`
and the read dates are updated. Thirteen items drifted in hooks, permissions and
CI. **Every one is in a guarded path, so none was edited**; they are
recommendations for the owner, in that note. Two matter most: the Stop hook's
block semantics and the `stop-gate.sh` error handling, and the review workflow
granting tools through `settings`. This session also hit one consequence in
practice: the Stop gate scans `crates/*/src` for the token `symlink` including
inside test modules, so tests that need a symbolic reference live under
`crates/*/tests/`.

## Guarded-path edits

None. `MPB_ALLOW_GUARD_EDIT` was never used.
