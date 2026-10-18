<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 5 — the brackets performed, `history`, `restore --plan`, and the stranded marker

Work order for one session. Parent: [`08-the-mutating-executor.md`](08-the-mutating-executor.md);
its §3 binds unchanged, and slice 2's §3 "How this repository bites" applies
verbatim. Rewritten on 2026-10-18, after slice 4 merged (PR #8), to carry what
slice 4 left owed. **`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false` for the
whole slice.** Nothing here makes the shipped binary change a provider.

Slice 4 wired the mutating run loop behind a test-only allowance. A wired run
today can never end in `SUCCESS`: `SNAPSHOT_BEFORE` and `RECORD_HISTORY` are
passed over as `NOT_PERFORMED_IN_THIS_BUILD`, so every forced run is
`PARTIAL_SUCCESS`. With no snapshot there is nothing to restore from, and with
no restore the only remedy for a stranded interrupted-run marker is the by-hand
text of D-M3. This slice closes that loop. The run takes its snapshot and
records itself honestly. The user can list runs, look at one, get a reviewable
plan that undoes one, and see and deal with a stranded marker.

## 0. Read first, in this order

Do not load all of `docs/spec/` up front. Read these, then open others when a
step needs them.

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/01-invariants.md`, in particular INV-01,
   02, 03, 04, 11, 15, 16, 17 and 19. Tier 1: if one looks wrong, stop and ask
   the owner.
2. `docs/spec/04-migration-and-safety.md`:
   - "Rollback and snapshots" and "Undoing a migration". **This is the contract
     for `restore`; follow its wording, not this file's, where they differ.**
   - "History": what a record holds and what the store guarantees.
   - "Concurrency and locking": the lock and the marker.
   - "Verification contract", including "Verifying a removal".
3. `docs/spec/06-cli-and-ux.md`:
   - The command list, which fixes the surface: `history [show <id>]`,
     `restore --plan (--run <id> | --snapshot <file>)`.
   - The output contract and the exit codes: 8 for a missing or unreadable
     snapshot; 4 for a restore plan holding `MANUAL_REVIEW` when it is
     *executed*, which is not this slice.
4. `docs/schemas/history.v1.md`, `run-marker.v1.md`, `snapshot.v1.md`,
   `plan.v1.md` ("What an executor inherits": obligations 8, 11 and 12).
5. `docs/spec-evolution.md`:
   - SE-092: read the whole entry. In it, the "**Provisional**" note on
     `RECORD_HISTORY` (settle it here), D-M1 to D-M5, and the slice 4
     decisions D1 to D15, especially **D15 "Carried"**, which is this slice's
     backlog together with §1 below.
   - SE-016 (consent) and SE-094 (`PROVIDER_REPORTS_ACTIVE`): both stay out of
     scope (§1, "Not in this slice"), but you need to know why.
   - SE-100: the build housekeeping.
6. `docs/passes/08-slice-4-plan.md`: §9 "As built" and §10 "End-of-slice
   review". `docs/passes/08-slice-4-report.md`: the mutation proof and the
   handoff.
7. The code you will touch, opened with **Read** so `.claude/rules/` loads the
   relevant chapter:
   - `crates/application/src/executor.rs`: `next_action_guarded`, which passes
     the two brackets over; `judge_step`; `judge_mutation`; `plan_still_holds`.
   - `crates/application/src/executor/tests.rs`.
   - `crates/application/src/history.rs` (`run_record`) and
     `crates/domain/src/history.rs` (`RunRecord`, `from_stored`, `StepResult`,
     `CommandEnd`).
   - `crates/cli/src/executor/mod.rs`: `run_mutating`, `RunReport`, the
     `Boundary` observer.
   - `crates/cli/src/app/migrate.rs` (`execute_mutating`), `context.rs`,
     `human.rs`, `mod.rs` (command dispatch).
   - `crates/adapters/src/persistence/`: `history_store.rs` (`probe`,
     `write_record`, `read_view`, `record_file_name`, `new_run_id`),
     `run_marker.rs` (`read`, `write`, `clear`), `run_lock.rs`,
     `snapshot_store.rs` (`save_snapshot`, `snapshot_file_name`),
     `atomic.rs` (`write_new_durable`), `state_dir.rs`.
   - `crates/application/src/doctor.rs`.
   - `crates/cli/tests/executor_wiring.rs`: the fakes, `Scratch`, the order
     test.
   - `crates/cli/tests/history_preconditions.rs`: the wiring tripwires and the
     flip tripwires. Two of the flip tripwires are this slice's to satisfy;
     see §1.
   - `locales/en.toml`, `locales/pt-BR.toml`.

## 1. What this slice delivers

Four parts. Do them in this order, because each needs the one before.

### Part A — `SNAPSHOT_BEFORE` and `RECORD_HISTORY` performed, behind the switch

A wired run, reached only through the test-only `MutationAllowance`, now
performs both brackets.

- **`SNAPSHOT_BEFORE`.** Write the snapshot the run validated — the
  `PlannedRun`'s `before()`, the capture taken *under the lock* in D6, not a
  third capture — into the state directory before the marker is written and
  before the first provider change.
  - Write it through `snapshot_store` and a durable write. A snapshot that
    cannot be written durably halts the run before anything changes.
  - It writes only this tool's state (D-M1). The marker therefore still goes
    immediately before the first *mutating* command, after this step.
  - Record its **file name only** (04, History: "the snapshot path is the
    snapshot's file name only").
  - It never runs under `--replay` or `--from-snapshot` (INV-15). The
    live-scope refusal (D13) already stops those runs before the state
    directory; keep it first.
  - Decide and declare where it lives — for example `<state>/snapshots/` with
    the run id in the name — and that, per 04, automatic snapshots are never
    pruned.
- **`RECORD_HISTORY`: settle the representation.** SE-092 calls the current
  shape provisional because a `VERIFIED` `RECORD_HISTORY` has a record
  certifying its own write. Decide, declare in SE-092, and reflect in
  `history.v1.md` and `RunRecord::from_stored`. Recommended unless you find
  better: the step is stored as performed-pending, and is judged
  `VERIFIED` only when a read-back of the published file, under the same lock,
  parses through `from_stored` and matches what was written. A read-back that
  fails keeps the marker and turns a 0 into `IO`, as a non-durable write does
  today (D8).
  - If the representation changes the stored shape, bump the schema
    (`history.v1` to `v2`) or justify, in SE-092, why it stays compatible. A
    reader of v1 must not accept a v2 record silently, nor the reverse.
- **Then a forced run can reach `SUCCESS`.** Add to
  `crates/cli/tests/executor_wiring.rs`:
  - a forced run toward Homebrew that ends `Completed`, exit 0, record status
    `SUCCESS`, snapshot file present and named in the record, marker cleared;
  - the same through the order test, which gains `SnapshotWritten` (or
    similar) and `RecordVerified` boundaries in the right places;
  - each new boundary made to fail in turn: snapshot write fails → nothing
    dispatched, nothing recorded, no marker left; read-back fails → marker
    kept, exit is not 0.
- **Satisfy the flip tripwire
  `the_switch_needs_snapshot_before_and_record_history_performed`.** Update its
  detection in the same change that performs the brackets, as its own message
  says. Do not delete it: turn it into a positive check that the brackets are
  performed, so a regression is still caught.
- **The CLI half of `invariant_17_retained_run_recognition_is_direction_neutral`**
  (SE-092, mirror-fixture addendum, amended by D12): now that a wired run can
  write a `SUCCESS` retain-source record, prove recognition from the command
  line in both directions, on fixtures.

### Part B — `history` and `history show <id>` (read-only)

- `macportsbrew history` lists the runs in the store, newest first. Per run:
  id, timestamp, package, direction, outcome, whether it is restorable (fully,
  partially, not) with the reason, and the snapshot file name (04, History;
  06).
- `macportsbrew history show <id>` prints one run's record and its executed
  steps. The id passes `history_store::record_file_name` **before** it becomes
  a path. An id that is not a valid run id is a usage error (exit 2); a valid
  id with no record is reported as such, without a guess.
- `--json` with a **new versioned schema**, `docs/schemas/history-list.v1.md`
  for the list; `show` reuses the record's own schema or gets its own — decide
  and document. A JSON surface is a compatibility surface (06): it needs its
  schema document and its `docs/spec-evolution.md` entry.
- Reading creates nothing: no state directory, no lock, no file. An unusable
  store or record is `HISTORY_UNUSABLE`, exit 8, **never** "no history". It
  never runs as root, nor under `--replay` or `--from-snapshot` (04, History;
  INV-15).
- **Restorability is derived, never trusted.** It depends on whether the
  snapshot the record names is present and readable in the same state
  directory. It never comes from the record's `mutation_dispatched` flag
  alone: it comes from the ends and the halt reason (D15's carried **N7**). A
  record not known to be durable is never offered for restore (D8).
- Every text through the i18n catalogue (`en.toml` source, `pt-BR.toml`
  translation; `scripts/check-i18n-parity.sh` holds them together). Every
  value from a record is escaped and the home shown as `~`, as the run report
  does.

### Part C — `restore --plan --run <id>` (a plan, never executed here)

04 "Undoing a migration" is the contract. In short:

- A restore plan is an **ordinary `MigrationPlan`**, built by the same planner
  and judged by the same execution gate as a migration. Undo is a migration in
  the other direction (INV-17), not a special case.
- It returns the packages **that run touched** to their state before it, from
  the record and its snapshot. INV-01 applies: reinstall what was removed and
  verify it before removing what was added. So do INV-04 and INV-19: never
  touch a `STATEFUL` or `SERVICE` package's data or services.
- A `--retain-source` run is the cheap case. The plan verifies the retained
  source package, restores it as primary only if the run changed that (today
  nothing changes the primary: report only), and only then removes the
  destination package. The order is the same; only the first step is cheaper.
- A step whose artifact is no longer available becomes a `MANUAL_REVIEW` step
  with the reason, and `history` reports the run as partially restorable.
- A missing or unreadable snapshot: exit 8, and the run is not restorable,
  with the reason. Never restored from a guess.
- **`restore --run <id>` without `--plan` executes, and is not this slice**
  (06: "[later pass, with migrate]"). Without `--plan` it refuses, saying
  execution is not in this build, exactly as `migrate --yes` does today; it
  dispatches nothing.
- `restore --plan --snapshot <file>` (additive whole-machine reconcile, 04
  "Scope"): **do not build it** unless Parts A to D are done with time to
  spare. If you skip it, it refuses with a clear "not in this build" and the
  spec stays as written.
- Prove direction neutrality (INV-17) on fixtures, both directions. Slice 3's
  mirror snapshot, `fixtures/snapshots/both-providers-intel-macos26-live.json`,
  is the MacPorts-to-Homebrew route.

### Part D — the stranded marker: `doctor`, and the explicit ways out

- **`doctor` shows a stranded interrupted-run marker.** Run id, whether a
  record for that run exists and whether it is durable, what that run changed
  (from the record, if any), and what to do.
  - Use `/add-doctor-check`.
  - Reading the marker takes no lock and creates nothing. An unreadable marker
    is reported, never treated as absent (D-M2).
  - This satisfies the flip tripwire
    `the_switch_needs_doctor_to_show_a_stranded_marker`, whose detection is
    that `crates/application/src/doctor.rs` contains `interrupted_run`.
    Satisfy it honestly, with a real check that has its own tests, not a name
    in a comment. The tripwire strips comments anyway.
- **Explicit ways to clear a marker other than the run's own `clear`.** D-M4
  says no code path deletes a marker but the run's own `clear`; this slice
  adds exactly the owner-facing paths, and declares the change to D-M4 in
  SE-092:
  - `restore --plan --run <id>` for the marker's own run, which shows what
    would undo it. Whether building the plan also clears the marker is a
    design decision: recommended **no**, because a plan is not an action.
  - An explicit acknowledgement, for example `history acknowledge <id>`, which
    the user runs after looking. It is a new command: add it to 06's command
    list with an SE entry, `wiki/CLI-Reference.md`, and the README.
  - Each clearing path takes the run lock, refuses a marker that names another
    run (`NotOurs`), refuses when another run holds the lock (exit 7), and is
    never available under `--replay` or `--from-snapshot`.
- **Reconcile the two carried cases** (D15's **L4** and **N6**):
  - a run whose record was published but whose marker stayed;
  - a run whose record was published with an unconfirmed directory sync.

  `doctor`, `history` and the acknowledgement must say what each of these is
  and what is safe, consistently.
- **Replace D-M3's by-hand remedy** everywhere it is written —
  `locales/en.toml`, `locales/pt-BR.toml`, `docs/schemas/run-marker.v1.md`,
  `wiki/Restoring-After-a-Migration.md`, `migrate.history.*` remedy texts — with
  the commands. Keep the warning that a non-durable record is the only trace
  and is never deleted.

### Not in this slice (stay carried, each behind its flip tripwire)

Say so in the report; do not start them.

| Item | Tripwire | Where it goes |
|---|---|---|
| Elevation consent wired to dispatch (INV-06, SE-016) | `the_switch_needs_consent_wired` | the pass that executes elevated steps |
| The executed plan tied to the plan shown (`plan.v1.md` obligation 8) | `the_switch_needs_the_executed_plan_tied_to_the_one_shown` | slice 6 or the flip proposal |
| The window between `VERIFY_DESTINATION` and `REMOVE_SOURCE` | `the_switch_needs_the_window_before_removal_closed_or_accepted` | slice 6, next to DP-030 |
| `PROVIDER_REPORTS_ACTIVE` evaluable, with a `provider-researcher` note for SE-094's softened Homebrew claim | `the_switch_needs_the_destination_contract_evaluable` | slice 6 |
| Release builds free of the `test-support` feature | `the_switch_needs_release_builds_free_of_the_test_support_feature` | the release pass (it needs `scripts/`, a guarded path) |
| The refusal on an observed or unchecked provider (DP-030) | `mutations_cannot_dispatch_while_a_live_run_still_says_not_checked` | slice 6, after the owner rules |
| Shim-backed recognition (INV-16) | none | later |
| The umask test (DP-029) | none | the owner's ruling |
| A dedicated error identifier for a failed provider command | none | an exit-code proposal: exit codes are a documented contract (06) |
| Typed-ahead consent input (slice 4 review SR 10); the token carrying its program (SR 9) | none | with consent wiring |

## 2. Hard boundary

Slice 2 §2 holds in full. In particular:

- `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`. The test pinning it, and
  `every_plan_this_planner_builds_is_refused_for_execution_in_this_build` with
  its fixture counterpart, are never weakened.
- **No restore path dispatches anything.** `restore` without `--plan` refuses.
- No mutating `brew` or `port` subcommand anywhere, including tests: tests use
  captured fixtures and the fakes in `executor_wiring.rs`. No elevation
  (INV-06). Nothing writes into either provider's prefix, shell configuration
  or launch machinery.
- Writes go only into the tool's own state directory, and in tests only into a
  scratch state directory (`Scratch` in `executor_wiring.rs`). Never the
  developer's real `~/.local/state/macportsbrew` or equivalent.
- No new dependency; `Cargo.lock` unchanged. `docs/spec/01-invariants.md` is
  read-only.
- No guarded path: `.claude/hooks/`, `.claude/settings.json`, `scripts/*.sh`,
  `.github/workflows/`, `.codex/`, `.agents/skills/`. If you believe one needs
  to change, stop and ask the owner. Their `MPB_ALLOW_GUARD_EDIT=1` must come
  in their own message, and it covers that message only.
- New commands appear in `wiki/CLI-Reference.md`, and the README's "What this build does *not*
  do" section is updated in the same change.

## 3. How to work

### The protocol

1. `/tooling-recheck` first.
2. **Plan before code**, in plan mode. Write the plan to
   `docs/passes/08-slice-5-plan.md`, with numbered decisions (D1, D2, ...) and
   the slice 4 plan's structure as the model. Have **`spec-guardian` and
   `security-reviewer` review the plan before any code**, fold their findings
   in, and record each one in the plan, as slice 4's §7 and §8 did.
3. Build Part A, then B, then C, then D. **One coherent commit per part**,
   Conventional Commits, each ending with the attribution line the harness
   gives.
4. **Docs in the same change as the code they describe:** 04, 06, the schemas,
   SE-092 (and a new SE entry for each new JSON surface and each new command),
   the wiki, the README, the CHANGELOG.
5. **Mutation proof** of every rule and ordering edge you add: swap two calls,
   drop one, fail each in turn, invert each comparison. Run it in an isolated
   worktree through a subagent, as slice 4 did. **Read the survivors**: slice
   4's proof found a test that passed without testing anything (a JSON path
   the reader never looked at). Kill each non-equivalent survivor with a new
   test checked against its mutant, or argue it equivalent in the report.
6. **End-of-slice review** by `spec-guardian`, `security-reviewer` and
   `docs-auditor` over `main...HEAD`. Resolve or accept each finding in
   writing.
7. Coverage at or above the floors in `docs/spec/07-quality-and-testing.md`.
   Write "As built" in the plan and the report in
   `docs/passes/08-slice-5-report.md`.
8. Push the topic branch and open a pull request. **Never merge**: the owner
   merges.

### How this repository bites (read before the first edit)

- **The `Stop` gate refuses a quoted mutating verb** anywhere under
  `crates/*/src`, test modules included: install, uninstall, reinstall,
  upgrade, link, unlink, selfupdate, activate, deactivate. It also refuses the
  token `symlink`. Use a neutral word such as `"verb"` in samples and
  fixtures; put real command text in a fixture file, as
  `.claude/hooks/test-hooks.sh` does. **Never reword a command to slip past a
  guard**: that is a specification violation in itself.
- **The commit gate runs `scripts/validate.sh`** in its default mode — the test
  suite and the fixture matrix — whenever the commit touches `crates`, `src`,
  `tests`, `fixtures`, `locales`, `data`, `scripts`, `.claude`, `Cargo.lock`
  or `Cargo.toml`. It refuses after 1380 s. A warm run takes about 5 minutes.
  Commit when nothing else is compiling (no subagent building, no coverage
  run), and run long commits in the background so the app does not time out.
- **`target/` housekeeping (SE-100).** Every validation ends with
  `scripts/prune-target.sh`. If its output warns that a `deps` directory is
  over 20000 entries, run `scripts/prune-target.sh --clean` while no build is
  running. If validation suddenly turns slow, count
  `ls target/debug/deps | wc -l` first: that was the cause of a 25-minute
  validation on 2026-10-17.
- **Agent worktrees** live under `.claude/worktrees/`, which is git-ignored.
  Never commit one.
- **Open a file with Read before editing it.** The path-scoped rules in
  `.claude/rules/` load the right chapter only through Read, Edit or Write; a
  shell `cat` loads nothing.
- **Clippy runs with `-D warnings` and pedantic lints**; run
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  before committing. `cargo fmt --all` before every commit.
- **The executor is dangerous to change** (`AGENTS.md`): run
  `security-reviewer` and `spec-guardian` on any change to
  `crates/cli/src/executor/`, `crates/application/src/executor*`,
  `execution_gate.rs` or `verification.rs` before calling it done.
- **No provider name above `crates/adapters/`** (INV-17), in code or in prose
  comments. `scripts/check-layering.sh` and the layering tests enforce it.
- **Exit codes are a documented contract** (`crates/domain/src/error.rs`, 06).
  Adding an `ErrorId` maps it to an existing code; a new code is a proposal for
  the owner, not your decision.
- **Never commit** `.claude/settings.local.json` or `CLAUDE.local.md`.
- **A decision that is the owner's** — anything that trades away audience,
  spends money, publishes, changes the charter or would relax an invariant —
  goes to `docs/decisions-pending.md` through `/decision-pending`. Take the
  reversible default and keep going.

## 4. Definition of done

- [ ] `SNAPSHOT_BEFORE` and `RECORD_HISTORY` are performed in a wired run. The
      `RECORD_HISTORY` representation is settled in SE-092 and
      `history.v1.md` (or v2). A forced run reaches `SUCCESS`, exit 0, with
      its snapshot written and named.
- [ ] `the_switch_needs_snapshot_before_and_record_history_performed` is
      updated in the same change, still detecting a regression;
      `the_switch_needs_doctor_to_show_a_stranded_marker` is satisfied by a
      real, tested `doctor` check.
- [ ] The CLI half of `invariant_17_retained_run_recognition_is_direction_neutral`
      exists and passes in both directions.
- [ ] `history`, `history show <id>`, `restore --plan --run <id>` and the
      explicit acknowledgement exist, with `--json` schemas documented, exit
      codes per 06, and tests named for what they prove. Every new invariant
      test is named `invariant_NN_*`.
- [ ] `restore --plan` builds an ordinary `MigrationPlan` through the same
      planner and gate, honours INV-01, 04 and 19, and is proved in both
      directions (INV-17). `restore` without `--plan` refuses and dispatches
      nothing.
- [ ] Restorability derives from the snapshot's presence and from the ends and
      halt reason, never from the dispatched flag alone (N7). A non-durable
      record is never offered for restore.
- [ ] The L4 and N6 cases are reconciled and described the same way in
      `doctor`, `history` and the acknowledgement. D-M3's by-hand remedy is
      replaced by the commands everywhere it was written.
- [ ] Each "Not in this slice" item is listed in the report as still carried,
      and its tripwire is untouched.
- [ ] The mutation proof is run, every non-equivalent survivor killed or
      argued. The three reviewers have run, with findings resolved or accepted
      in writing. Coverage floors met; `scripts/validate.sh` passes; "As
      built" and the report written; PR open, not merged.
