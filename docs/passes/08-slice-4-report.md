<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 4 — wiring the run loop, and elevation consent: report

Work order: [`prompts/08-slice-4-wiring-and-consent.md`](../../prompts/08-slice-4-wiring-and-consent.md).
Branch `pass/08-slice-4-wiring-and-consent`, from `main` at `428df05`.
`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`. Run unattended overnight on
2026-10-17 at the owner's request ("evolua tudo que puder sem minha decisão").
Nothing is merged, tagged or released by this session; it opens a pull request.

## Progress

- Plan to `docs/passes/08-slice-4-plan.md`: done (revision 3)
- Plan reviewed by `spec-guardian` and `security-reviewer` before code: done,
  twice (§7 and §8 of the plan)
- Part A, the wiring behind the switch, with the behavioural run-order test:
  done (`crates/cli/tests/executor_wiring.rs`, both directions)
- Part B, elevation consent (gate still refusing): done, not wired
- Owed items from SE-092 done or carried: done or carried, each carried item
  behind a flip tripwire (plan D14, D15)
- End-of-slice review by `spec-guardian`, `security-reviewer`, `docs-auditor`:
  done; no Tier 1 or Tier 2 finding; dispositions in plan §10 and SE-092
- Mutation proof of every rule and ordering edge: done (below); every
  non-equivalent survivor now killed
- Docs (04, `plan.v1.md`, `migrate.v1.md`, SE-092, SE-094, CHANGELOG): done
- Coverage: domain 96.43%, policy 97.62%, application 95.61%, adapters 95.46%,
  overall 94.57% (floors in `07-quality-and-testing.md` met)
- "As built": plan §9

## Mutation proof

Run by a subagent in an isolated worktree on `0856190` (before the review
fixes), each mutant against its narrowest test target, every survivor confirmed
against `cargo test --workspace --all-features --no-fail-fast`, every file
restored. **52 mutants: 41 killed, 11 survived**, plus five orderings and drops
that cannot compile (argued below). The survivors were then killed by new
tests, each re-checked against its mutant here, or argued equivalent.

### What cannot compile (the types carry the ordering)

- Swapping the configuration re-read and the re-check, or the re-check and the
  dispatch: the re-check needs the re-read's output, and `mutation_dispatch`
  needs a `ConfigurationRechecked`, whose field is private to the function that
  re-checks.
- Dropping the lock or the re-check: `RunLock` has no constructor but
  `acquire`, and the marker and the dispatch both take it.
- Swapping the dispatch and the record write: the record is built from every
  step's record, after the loop.
- A witness from a different plan: `GateClear` owns its run and its allowance
  together.

### Survivors, and what became of them

| Mutant | Why it survived | Now |
|---|---|---|
| Read-back accepts a `PERFORMED` step with a non-zero or empty end (two mutants) | **The test was vacuous**: it wrote the ends to `steps[0].ends`, where the stored shape has `steps[0].result.ends`, so every case was refused for an unknown field before the check ran. `a_read_halt_is_stored_without_an_ends_field` had the same wrong path and passed vacuously | Killed: both paths corrected, and a control asserts the path holds an array and a good value reads back |
| `judge_mutation` drops the ends on `PRECONDITION_FAILED` | No case put a precondition failure after a command that had run. Not equivalent: the first command's end would be lost and the step read as not dispatched, so a performed change would go unrecorded (obligation 12) | Killed: a case in `a_halt_on_the_second_command_keeps_the_first_and_is_recorded_as_dispatched` |
| `SET_PRIMARY` dispatched as an add | Only planner plans were tested, whose `SET_PRIMARY` targets the tool. Not equivalent for a doctored plan | Killed: `a_set_primary_step_carrying_a_provider_command_halts_under_an_allowance` |
| The still-current check dropped; the check moved after the re-read | No test ever lost the lock | Both killed: `a_lock_lost_before_the_command_stops_the_step_before_the_re_read` replaces `run.lock` once the marker is written, and asserts `LOCK_LOST`, nothing handed to the site and the re-read never called |
| `RunReport::exit_code` ignores the history error | Unreachable through a run in this build: every mutating run ends partial (D9) | Killed: `a_completed_run_whose_history_went_wrong_never_exits_zero`, on the report directly |
| `ProviderLocation` and `ProviderCapability` swapped in the diagnosis | The run is refused either way; no test named either fact | Killed: `a_provider_that_moved_or_changed_what_it_can_do_under_the_lock_is_named` |
| The record written best-effort instead of durably | The two writes part only when the directory's sync fails after the link, a seam private to `atomic.rs` | Killed by a text tripwire, `the_record_is_written_durably_not_best_effort` — the only kind of test that reaches it without making the sync injectable |
| Recapture-and-replan swapped with the marker read | Only a run with both a stale marker and a changed machine tells them apart | **Argued, kept.** Both refuse before the marker is written or anything is dispatched; only which refusal is reported changes. Plan §4 orders them; no safety property depends on it |
| Elevation taken from the derivation only, the plan field's check dropped | Under an allowance the gate already pins the plan's elevation to the derived one (`MUTATION_IS_NOT_THE_PLANNERS_COMMAND`) | **Argued equivalent** behind the gate: the guard's check is defence in depth |

### Weaker than behavioural

Some mutants die only to text tripwires in `crates/cli/tests/history_preconditions.rs`:
dropping the probe, dropping the marker read, and moving the marker write after
the re-check. And a removal dispatched before its destination verified dies only
to the application test `invariant_01_a_removal_is_dispatched_only_after_its_destination_verified`,
not to the wiring tests. Each is killed; the wiring tests' fakes cannot tell
those orderings apart without failing the probe or the marker read, which the
history store's own tests already do. Carried as a note for the slice that
performs the brackets, when a run can complete.

## Handoff (2026-10-17)

For the session that picks this up. State: four commits on
`pass/08-slice-4-wiring-and-consent` (`650e623`, `0856190`, `8d837cc`,
`0351754`), pushed; pull request
[pabloguia/MacPortsBrew#8](https://github.com/pabloguia/MacPortsBrew/pull/8)
open, CI green (12 of 12), **not to be merged without the owner's
authorization**. PR #7 (closes DP-031 and DP-038) is green and waits the same
way. The working tree is clean but for `.claude/worktrees/`, the mutation
agent's leftover, which is never committed.

**Next, in order:**

1. If review comments or a CI failure arrive on #8, address them on this
   branch.
2. ~~Find why `scripts/validate.sh` (default) takes ~25 minutes.~~ **Found
   (2026-10-17).** On macOS the dev profile's `split-debuginfo = "unpacked"`
   keeps every codegen unit's `.o` in `target/debug/deps` for the debugger,
   and Cargo never removes them when a crate rebuilds under a new hash. The
   night's rebuilds (mutation proof, coverage) left 605,518 of them, 9.8 GB.
   Launching a test binary from that directory then blocked in exec, with no
   CPU used, for 20–100 s — the same bytes started in 0.9 s from
   `target/debug/` — because something on the exec path walks the
   executable's directory (an Endpoint Security extension, Kaspersky here, or
   `syspolicyd`). With ~60 test binaries that was the 25 minutes. After
   `cargo clean`: the default validation from cold, rebuild included, in
   449 s; a test binary starts in 0.07 s. **When validation turns slow,
   count `target/debug/deps` (`ls target/debug/deps | wc -l`) and
   `cargo clean` if it is in the hundreds of thousands.** The owner then
   chose `split-debuginfo = "packed"` in `[profile.dev]` (`Cargo.toml`):
   after a full build `deps` holds 626 entries and no object files (6,986
   before), the cold default validation takes 478 s and a warm one 274 s.
   Stale binaries, `.dSYM` bundles and libraries still accumulate, about 180
   entries per full rebuild; a prune after every validation
   (`scripts/prune-target.sh`, drafted) waits for the owner's guarded-path
   authorisation, since it lives in `scripts/` and runs from `validate.sh`.
   Separately, an untracked `.claude/worktrees/` made the
   commit gate treat a docs-only commit as touching `.claude` and run the
   full validation; it is now git-ignored.
3. After #8 merges, slice 5 needs a work order in `prompts/` before code: the
   carried items, each behind a flip tripwire — `SNAPSHOT_BEFORE` and
   `RECORD_HISTORY` performed, consent wired to the gate, `doctor` showing a
   stranded marker, the executed plan tied to the one shown, the window before
   removal closed or accepted, release builds free of `test-support`. Also owed:
   a `provider-researcher` note for SE-094's softened Homebrew claim, the
   typed-ahead consent input (SR 10), the token carrying its program (SR 9).
   Writing the work order is a planning task; start it in plan mode and have
   `spec-guardian` and `security-reviewer` review the plan before code.

**What trips a session here:** the `Stop` gate refuses a quoted mutating verb
(and `symlink`) anywhere under `crates/*/src`, test modules included — use a
neutral word in samples. Commit when nothing else is compiling, or the commit
gate times out. Open a file with Read before editing it, so the path-scoped
rule in `.claude/rules/` loads. `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`
until a work order says otherwise.
