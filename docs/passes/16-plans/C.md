<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part C — naming a package enters advanced mode for that package (DP-037)

Plan written 2026-10-27, before any code. Reviewed by `spec-guardian` and
`security-reviewer`; their review is the approval (work order 16 §4).

## The ruling

DP-037 (b), both halves. Naming a package outside the default view is the
explicit entry into advanced mode **for that package only**:

- `advanced.warning` is printed, and carried in `--json` as an identifier;
- the plan records how the row was entered;
- any destructive step for it needs the stronger confirmation (part K defines
  the levels; **that half lands in K** and is marked pending here by name);
- the named package's transitive dependencies are never added implicitly.

## Where it lives today

- `planner::plan_outcome` (`crates/application/src/planner.rs` ≈452) analyses
  with `advanced: true` and selects the named unit; each unit carries
  `advanced_only` (`analysis.rs` ≈262): true when no provider records it as
  top-level, i.e. it is outside the default view.
- `AnalysisOptions::advanced` carries the user's `--advanced`.
- `cli::app` prints `advanced.warning` to stderr only when `--advanced` is
  given (`crates/cli/src/app/mod.rs` ≈348).
- `MigrationPlan` (`crates/domain/src/plan.rs`) has no record of entry.

## Design (revised after both plan reviews, see Reviews)

1. **What decides the entry: the source row the plan acts on.** A unit is
   visible in the default view when *any* provider records it as top-level, so
   unit visibility is the wrong input: a unit top-level under the destination
   and a dependency under the source would plan `REMOVE_SOURCE` for a
   dependency with no warning (spec-guardian 1, security-reviewer M2). The row
   is outside the default view iff
   `!unit.rows[source].ownership.shown_in_normal_mode() || unit.advanced_only`,
   with the row looked up by `.get(&source)`; a missing row is
   `ErrorId::Internal`, never a quiet `DEFAULT_VIEW` (security-reviewer N1).
   `UNKNOWN` ownership counts as in the default view, as the current spec says
   (an `UNKNOWN` row appears in the default view); that is flagged for part K
   as a deliberate decision, not an accident.
2. **Domain.** `MigrationPlan` gains one field, `entered_by: Option<EntryMode>`,
   serialised `enteredBy`. `EntryMode` is `DEFAULT_VIEW`, `ADVANCED` (outside
   the default view, `--advanced` given) or `NAMED` (outside it, entered only
   by naming it). **Every plan this build produces carries `Some`**, and
   `DEFAULT_VIEW` is serialised explicitly, never collapsed into absence.
   `Option` exists only so a plan document from before the field loads; for
   part K, an absent value means the **strictest** confirmation, never the
   normal one. Additive, `serde(default)`: `plan.v1` stays v1.
3. **No `warnings` on the domain type.** The warning is derived from
   `enteredBy` at the `--json` document level (`warnings: ["advanced.warning"]`
   when `NAMED` or `ADVANCED`, omitted otherwise), the way `plan.v1` already
   carries `concurrentProviderActivity`. So there is one stored fact, the
   domain carries no catalogue key, and a forged identifier cannot reach the
   renderer from a plan (security-reviewer M1/L1, spec-guardian 4). In the
   program the set is closed: an enum rendered to its identifier.
4. **Planner.** `plan_outcome` computes `entered_by` from the user's
   `options.advanced` (never the internal `opts.advanced`, which is always
   true) and the rule in 1. `restore` builds its plan through the same
   `plan_outcome`, so restore plans record entry the same way: there is no
   special case. A restore of a run originally entered by `NAMED` recomputes
   from the current snapshot; K records that.
5. **Transitive dependencies.** The plan's steps name only the selected unit;
   asserted by a test and a mutant.
6. **CLI.** For `plan`, `migrate` and `restore --plan`:
   - rendered: when `NAMED`, print `advanced.warning` to stderr, then a new
     key `plan.entered_by_named` ("{package} is outside the default view, so
     this plan acts on it in advanced mode, for this package only. Its own
     dependencies are not added."), worded to fit `restore --plan`, which
     names a run rather than a package (security-reviewer N2). With `--advanced` the warning was already
     printed once at the start and is not repeated;
   - `--json`: `enteredBy` inside the plan, `warnings` at the document level;
   - `en` and `pt-BR`; i18n parity; the new message added to
     `crates/cli/tests/hostile_text.rs` (security-reviewer L3).
7. **Schemas.** `plan.v1.md`, `migrate.v1.md` and `restore.v1.md`; documented
   samples regenerated from the program's output.
8. **Spec.** 06's advanced-mode paragraph: naming a package whose source row
   is outside the default view enters advanced mode for that package (DP-037);
   the stronger confirmation for its destructive steps is part K's. One SE-109
   propagation bullet.
9. **Contract handed to parts K and E** (written into SE-109 and the ledger):
   (a) the confirmation level is a pure function of the in-memory plan the
   planner just built from a live capture; a plan read from a file never feeds
   the gate, the confirmation or the dispatch guard; (b) absent `enteredBy`
   means the strictest level; (c) `warnings` is never an input to a decision;
   (d) part E's canonical digest form covers `enteredBy` and keeps
   `DEFAULT_VIEW` distinct from absence; (e) C's confirmation half is a
   **blocking** acceptance item of part K.

## Tests

- `invariant_13_a_named_dependency_warns_and_needs_the_stronger_confirmation`
  (application): a dependency source row named without `--advanced` yields
  `NAMED`; with `--advanced`, `ADVANCED`; a top-level row with or without
  `--advanced`, `DEFAULT_VIEW`; the steps name only that package. For its
  confirmation half it asserts the closest present truth, that the execution
  gate refuses the `NAMED` plan; the stronger level itself is pending until
  part K (doc comment and the ledger's K row, blocking).
- The split case: source row a dependency, destination row top-level, yields
  `NAMED` — in both directions (`invariant_17_*`), on in-code test snapshots
  as the application tests already build them (no recording is invented).
- Every plan the planner produces carries `Some(entered_by)`.
- `plan_still_holds` refuses when only `enteredBy` differs (security-reviewer M3).
- CLI end-to-end for `plan`, `migrate` and `restore --plan`, rendered and
  `--json` (spec-guardian, second round); if `restore --plan` cannot reach a
  named-dependency case from an existing recorded run, the test says why.
- A plan document without the field loads as `None`.

## Mutants (run in an isolated worktree)

1. `entered_by` always `DefaultView`.
2. Decide from `unit.advanced_only` alone (the split case).
3. Read `opts.advanced` instead of `options.advanced`.
4. Swap `ADVANCED` and `NAMED`.
5. Print the rendered warning for `DEFAULT_VIEW`, or twice for `ADVANCED`.
6. Omit the document-level `warnings` when `NAMED`.
7. Append a step for one of the unit's dependencies.

Each must be killed.

## What this part does not do

No confirmation level (part K). No change to which rows the default view
shows, to `analysis`, or to the executor, the gate or verification.
`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`.

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
| spec-guardian | plan, first draft | not approved: entry decided by the unit not the source row (1); restore shares `plan_outcome` (2); `None` must fail closed for K (3); `warnings` as free strings in the domain (4); `migrate`/`restore` schemas and renderers missing (5); the confirmation half invisible (6); INV-17 test under-specified (7); `UNKNOWN` (8); a dependency-adding mutant (9) | All folded into the revised design, tests and mutants above |
| security-reviewer | plan, first draft | approve with fixes: closed set for `warnings` (M1); source row decides (M2); K's contract that the level is derived and never trusted (M3); two copies can disagree (L1); E's digest covers the field (L2); hostile-text test (L3); restore (L4); `UNKNOWN` (L5); `options` vs `opts` (L6) | All folded in; `warnings` moved off the domain type entirely |
| spec-guardian | revised plan | **approved**; low: add a `restore --plan` end-to-end case | Added to Tests |
| security-reviewer | revised plan | **approved**; low: N1 no indexing panic, N2 message wording for `restore` | Folded into Design §1 and §6 |
| security-reviewer | `59a4824` | **approved**; medium (latent): `migrate` printed the notice after execution; low: hostile-text comment overstated | Notice moved before the execution block; comment and As-built corrected |
| spec-guardian | `59a4824` | not yet: F1 vacuous dependency check; F2 no restore test, wrong reason; F3 mutants unrecorded; F4 notice under `--json`; F5 field order | All resolved above |
| spec-guardian | `6fefd45` | **approved**; low: three mutants guarded by reasoning, not executed (recorded above) | Accepted; part K may execute them |

## As built (2026-10-28)

- Implemented as designed.
- `restore --plan`: `invariant_13_a_restore_that_acts_on_a_dependency_warns`
  (`restore_tests.rs`) makes the current machine hold the restored row as a
  dependency, the way those tests already build their records in code, and
  asserts `NAMED`, the `--json` warning and the stderr notice (spec-guardian,
  code review F2; the first draft of this note wrongly said it could not be
  reached).
- The "dependencies are never added" check now plans `tool`, which depends on
  `dep`, and asserts its steps never name `dep` (spec-guardian F1; the first
  version checked a reverse dependency and was vacuous).
- `a_plan_whose_entry_moved_under_the_lock_is_refused` pins `plan_still_holds`.
- `hostile_text.rs` runs `plan` and `migrate` on a named dependency, so the
  notice's template is in the escaping check; its `{package}` parameter is
  escaped structurally by `render` (no captured name is hostile).
- `migrate` prints the notice before the execution block, not after it
  (security-reviewer, code review).
- Under `--json` the notice is also on stderr, as `--advanced`'s warning
  already is in every mode; DP-037 says the warning is "printed (and carried in
  `--json`)" (spec-guardian F4).

### Mutation proof (isolated worktree, at `59a4824`, removed afterwards)

| Mutant | Killed by |
|---|---|
| 1 `entered_by` always `DefaultView` | invariant_13, invariant_17, end-to-end |
| 2 decide from `unit.advanced_only` alone | invariant_17 only |
| 3 `opts.advanced` for `options.advanced` | invariant_13, invariant_17, end-to-end |
| 4 swap `Advanced`/`Named` | invariant_13, invariant_17, end-to-end |
| 5a notice on `DefaultView` | end-to-end only |
| 5b notice on any advanced entry (repeats under `--advanced`) | end-to-end only |
| 6 `plan_warnings` always empty | end-to-end only |
| 6b `migrate`'s document drops `warnings` | end-to-end only |

No survivors. The planner mutant that appends a step for a dependency was not
in that run: the planner has no single insertion point for it, and its guard is
the strengthened assertion above, which fails on any step naming `dep`. The
restore-only mutants (dropping the notice or the warnings in `restore.rs`) are
guarded by the new restore test, added after the run.
