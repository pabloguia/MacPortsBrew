<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 6 — plan

Work order: [`prompts/08-slice-6-verification-before-removal.md`](../../prompts/08-slice-6-verification-before-removal.md).
Design: [`08-pre-flip-design-notes.md`](08-pre-flip-design-notes.md), "Slice 6".
Written 2026-10-26 on branch `feat/pass-08-slice-6`, cut from `main` at `ccd95f0`.
**`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false` throughout.**

## 0. Scope of this session

The owner asked on 2026-10-26 for every item that does not wait on them. This
slice is that item. Its parts have different blockers:

| Part | Blocked on | This session |
|---|---|---|
| A — `MANUAL_REVIEW` for an unavailable artifact | nothing (DP-050's default) | built |
| D — removal refused by release policy | nothing (DP-048's default), but see §6 finding 1 | **not built**: after C, with its own test waiver |
| B — `PROVIDER_REPORTS_ACTIVE` evaluable | `security-reviewer`'s ruling on moving `brew info --json=v2 --installed` into the run loop (R-20; pass 14 report) | **ruled for, with conditions** (§6); next session |
| C — re-read the destination before removal | B's check (it reuses the verify step's commands, which B changes) | after B |

*Superseded by the plan review (§6, finding 1):* this plan first proposed
building D before B and C (D9). The reviewers showed D's refusal would stop the
shell's removal tests, and C's own order test, from reaching dispatch, so D lands
last as the work order orders.

Next free identifiers: **SE-107**, R-21, DP-054.

## 1. Part A — an unavailable artifact is `MANUAL_REVIEW`

- **D1.** In `planner.rs`, inside `if let Some(dest_row)`, after the
  deprecated/disabled block, an `else if matches!(dest_row.availability,
  Unavailable | UnsupportedPlatform | Disabled)` pushes a blocking reason through
  `push_block` with risk id `destination_artifact_unavailable` and reason
  `reason.destination_artifact_unavailable` (`destination`, `package`, `state`).
  The `else` keeps a destination already blocked as withdrawn to **one** review:
  a disabled formula sets both `disabled` and `Disabled`.
- **D2.** Pushed after every existing blocking reason, so every existing plan's
  `MANUAL_REVIEW` order is unchanged. Proved by a test that builds the planner's
  fixture plans and compares their step kinds and reason ids before and after.
- **D3.** `SourceBuildRequired` and `Unknown` stay non-blocking: DP-050's
  reversible default. A source build is not an unavailable artifact, and
  `Unknown` is every restore's state; blocking on `!is_prebuilt()` would stop all
  of them. `Deprecated` is not added: a deprecated package installs, and it is
  already blocked through the `deprecated` flag.
- **D4.** The `state` parameter is a label the policy crate already owns for
  availability (`labels::availability` if it exists, else a new
  `labels::artifact_availability` in `crates/policy/src/labels.rs`), so the
  reason has no `Debug` spelling in it. Both locales gain the reason and the
  labels.
- **D5.** 04 gains a declared deviation under its MUST ("a restore step whose
  artifact is no longer available"): the planner raises it for `UNAVAILABLE`,
  `UNSUPPORTED_PLATFORM` and `DISABLED`; the example "the Homebrew formula an
  Intel Mac can no longer bottle" is a source build under DP-050's default and is
  **not** raised until the owner rules. The MUST's text is not rewritten.
- **D6.** The tripwire `the_switch_needs_manual_review_for_an_unavailable_artifact`
  becomes a positive check held whatever the switch is: it builds a plan whose
  destination row is `UNAVAILABLE` and asserts a `MANUAL_REVIEW` step carrying
  the reason, and a plan whose row is `SOURCE_BUILD_REQUIRED` and asserts none.
- **Tests:** `unavailable_or_unsupported_destination_is_manual_review` (both
  states); `source_build_and_unknown_are_not_manual_review`;
  `a_disabled_destination_raises_one_manual_review`;
  `existing_blocking_order_is_unchanged`; a gate test, refused with exit 4.
- **Effect on fixtures:** none. No adapter emits `UNAVAILABLE` or
  `UNSUPPORTED_PLATFORM` today, and a disabled row already blocks.

## 2. Part D — removal refused by release policy

- **D7.** `crates/domain/src/plan.rs` gains, beside the switch,
  `pub const THIS_BUILD_REMOVES_SOURCES: bool = false;`, documented as DP-048's
  reversible default.
- **D8.** `execution_gate::refusals` gains `ExecutionRefusal::SourceRemovalNotInThisBuild
  { step }` (`SOURCE_REMOVAL_NOT_IN_THIS_BUILD`), pushed for **every**
  `REMOVE_SOURCE` step while the constant is `false`, **whether or not an
  allowance is present**. That is the point of the part: the switch and the
  allowance concern dispatch; this one is the release's policy, and it must
  still refuse the day the switch turns on (and today, under
  `MutationAllowance::forced_in_tests`). Keyed on the kind alone, so a plan
  cannot pass by leaving the step's commands out. Ordering key `(*step, 25)`,
  after every existing variant. Exit code unchanged (4).
- `migrate` without `--retain-source` plans a `REMOVE_SOURCE`, so it is refused
  at the same point with no change to `migrate` itself.
- Rendered by `human.rs` through `migrate.refusal.source_removal_not_in_this_build`
  in both locales; `plan.v1.md` and `migrate.v1.md` list the refusal.
- **Schema.** The gate report is part of `migrate --json`. A new refusal value is
  additive; `migrate.v1.md` already tells readers to treat an unknown refusal as
  a refusal. No version bump — to be confirmed by the reviewers (§5, question 3).
- **Tests:** `invariant_01_no_plan_with_a_removal_passes_the_gate_while_removal_is_off`
  (under a forced allowance, a plan with `REMOVE_SOURCE` is refused with this
  refusal, and the same plan with `--retain-source` is not refused with it);
  `this_build_removes_no_sources` (a pin of the constant); and a flip tripwire
  in `history_preconditions.rs`, `the_switch_needs_source_removal_decided`, that
  fails while the switch is on and the constant is off, naming DP-048 — so
  turning on mutation forces a decision about removal rather than shipping with
  removal silently off or silently on.
- **D9.** Built before B and C (§0). Its tests do not depend on them.

## 3. Parts B and C — outline, pending the ruling

As the design notes: a `verify_active` read verb on both providers over reads
already permitted (`brew info --json=v2 --installed`, `port -q installed`); two
success semantics that fail closed (`StdoutMarksPackageActive`,
`JsonReportsPackageLinked`); `FailureCause::ActiveNotReported`; the gate pin by
position; `evaluable_in_this_build` true for `VerifyDestination` only; the
tripwire widened to every check; then C's `destination_still_verified` in the
shell for `REMOVE_SOURCE`, immediately before dispatch, with the verify step's
own pinned commands.

**The security record to weigh** (SE-016 item 3, SE-050 item 6, R-15 E7, R-20):
`brew info --json=v2 --installed` loads every installed formula's Ruby, can load
untrusted tap Ruby through a `brew.env`, and writes to the user's cache; today's
verify read is `list --versions <name>`. Moving it into the run loop, and running
it a second time before a removal, is what the reviewer rules on.

Mitigations already in the build, to be confirmed against the code by the
reviewer: DP-027 (b) refuses every mutating step against a provider whose own
configuration (a `brew.env`) sets a key or could not be read; the process
executor's sanitised environment; the allowlist already permits this exact argv
for capture.

If the ruling is against: Part B is not built, a DP records the alternatives
(a narrower read that grows the allowlist — the owner's), and C waits. If the
ruling is for: B then C, each with its own review before the next.

## 4. What does not change

- `git diff --stat main -- crates/adapters/src/process/` empty.
- The planner's step-kind sequence (a test compares it, D2).
- `every_plan_this_planner_builds_is_refused_for_execution_in_this_build` and its
  fixture counterpart, not weakened.
- No new dependency; `Cargo.lock` unchanged; no guarded path; invariants
  read-only; no elevation; no mutating command anywhere.

## 5. Questions for the plan review

1. **`security-reviewer`: the ruling on Part B** (§3). For, against, or for with
   conditions.
2. `spec-guardian`: D5 — is a declared deviation under 04's MUST the right
   vehicle while DP-050 is open, and does D3 (no block for a source build) fit
   INV-01 and INV-19?
3. Both: does `SOURCE_REMOVAL_NOT_IN_THIS_BUILD` need a schema version bump of
   `migrate.v1`?
4. Both: is refusing removal under a forced allowance (D8) correct, given that
   the slice-4 tests exercise `REMOVE_SOURCE` dispatch against fakes under that
   allowance? Those tests would now be refused at the gate; they should keep
   exercising the executor's own removal path below the gate, not through it.

## 6. Plan review (2026-10-26, `spec-guardian` and `security-reviewer`, Opus)

**Question 1, `security-reviewer`: part B is ruled for, with five conditions**,
recorded in SE-107: re-check the configuration before the certifying read (C1); a
real JSON parser in the application layer, exactly one `formulae[]` entry, a
`caveats` spoof test (C2); nothing from the output becomes evidence, a path or a
flag (C3); truncation and timeout fail closed, tested (C4); SE-050 item 6 amended
(C5). The read is already run by capture and by the recapture under the lock.

**Findings, and what became of them:**

| # | Reviewer, severity | Finding | Resolution |
|---|---|---|---|
| 1 | both, High | D8 refuses every `REMOVE_SOURCE` under `forced_in_tests()`, and the shell's removal tests (`executor_wiring.rs` `Direction::cleared()`, `directions()`, `invariant_01_a_destination_that_is_not_there_stops_the_removal`; `executor/tests.rs` `the_narrowed_plan_clears_the_gate_only_under_an_allowance`) reach dispatch only through `clear()`. No route below the gate exists; a second `GateClear` constructor would bypass the whole gate. | **D is not built in this session.** D9's reorder is withdrawn: D lands after C, as the work order says, with its own test-only waiver of the removal refusal (a field on the allowance set only by a new `forced_in_tests_with_removal()`, counted by the scan in `cli/tests/executor.rs`), and the wiring tests switched to it. |
| 2 | both, Medium/Low | The planned tripwire fails in exactly the state DP-048's default ships. | Replaced in the design: a tripwire that fails when `THIS_BUILD_REMOVES_SOURCES` is true and part C's re-check is not wired or DP-048 is open; a check that the shell never reads the constant. Built with D. |
| 3 | `security-reviewer`, Medium | D8 also refuses every `restore` that would undo an install. | To be stated in DP-048, 04 and restore's rendering, with a restore test, when D is built. Recorded in SE-107. |
| 4 | `spec-guardian`, Medium | D5/D6 overstated part A: the MUST stays unmet in every case a real snapshot reaches. | Slice 5's note in 04 rewritten in place (SE-032) to name each unmet clause; the tripwire **kept failing** with the switch on, its message updated; SE-107 lands in part A's commit. |
| 5 | `spec-guardian`, Medium | No `invariant_17_*` test for part A. | `invariant_17_an_unavailable_destination_blocks_identically_in_both_directions`. |
| 6 | `spec-guardian`, Low-Medium | The `Disabled` arm cannot be reached with today's data, so dropping it survives. | Tested with `DISABLED` and the withdrawal flag off; deprecated-and-unavailable reviewed for the withdrawal, tested, and said in the code comment. |
| 7 | `spec-guardian`, Low | `existing_blocking_order_is_unchanged` had nothing to compare against. | The full step sequence is pinned literally in `source_build_and_unknown_are_not_manual_review`; the order test asserts the absent-provider reason first and the new one last. |
| 8 | both, Low | No schema bump needed, but the "open set" sentence the plan cited does not exist. | `plan.v1.md` now says risk ids are an open set; the refusal sentence lands with D. CHANGELOG line added. |
| 9 | `spec-guardian`, Low | D4's path was wrong; `REASON_IDS` not mentioned. | `labels::availability` (`crates/policy/src/rules/labels.rs`) used; the id added to `REASON_IDS`. |
| — | built, not reviewed above | `state` is the withdrawal family in the renderer, and reusing it would have shown `UNAVAILABLE` in a sentence. | A new `artifact` family (`artifact.*`, both locales) and a test that only this reason interpolates it. |

## 7. As built (this session)

Part A only, on `feat/pass-08-slice-6`: `crates/application/src/planner.rs`,
`crates/cli/src/i18n/mod.rs` (the family), both locales, 04, `plan.v1.md`,
SE-107, CHANGELOG, the tripwire's message, the tests listed in SE-107. Parts B
(ruled for), C and D are the next session's, in that order. Not done here: the
mutation proof the work order asks for (a subagent in a worktree), the
end-of-slice review over the branch, and the slice report.
