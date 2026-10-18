<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 5 — plan: the brackets performed, `history`, `restore --plan`, and the stranded marker

Work order: [`prompts/08-slice-5-history-and-restore.md`](../../prompts/08-slice-5-history-and-restore.md).
Backlog: SE-092's slice 4 decision D15 ("Carried"), the "Provisional" note on
`RECORD_HISTORY`, D-M3/D-M4, L4 and N6, N7, and the CLI half of
`invariant_17_retained_run_recognition_is_direction_neutral`.

Revision 2, 2026-10-17. Revision 1 was reviewed by `spec-guardian` and
`security-reviewer` **before any code**; §7 records each finding and what was
done with it. The decisions below are already the revised ones. Neither
reviewer found a Tier 1 or Tier 2 problem.

`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`. While it is `false` the shipped
binary behaves as it does today: the gate refuses every plan with a mutating
step before anything below is reached. **Nothing here executes a restore.**

## 1. The shape, in one paragraph

Four parts, in order, one commit each. **A** makes a wired run (reachable only
through the test-only `MutationAllowance`) perform both brackets: it writes the
snapshot from the capture taken under the lock, before the marker and before the
first change, and it records itself with a representation that does not certify
its own write. **B** adds `history` and `history show <id>`, read-only, with
restorability derived from the snapshot's presence and from the ends. **C** adds
`restore --plan --run <id>`: an ordinary `MigrationPlan`, built by the same
planner in the other direction and judged by the same gate; `restore` without
`--plan` goes to that gate and is refused. **D** makes a stranded marker visible
in `doctor` and gives the owner one explicit way to deal with it
(`history acknowledge <id> --yes`), replacing D-M3's by-hand remedy everywhere.

## 2. Decisions

### Part A

**D1 — the snapshot is the capture taken under the lock.**
- The work order says "`before()`, the capture taken under the lock". Those are
  two captures: `PlannedRun::before()` is taken before the lock, `now` (the
  recapture in `run_with_mutations`) under it, and `plan_still_holds` proves the
  two plan identically. **The snapshot written is `now`:** nearest the instant
  before the first change, already in hand, so not a third capture. This reads
  the work order's stated intent over its parenthetical; declared in SE-092.
- **Versions in the record** come from `before()` (`run_record`); plan equality
  implies equal source rows, and a test proves the record's source version equals
  the snapshot's for the same package.
- **Where.** `<state>/snapshots/snapshot-<stamp>.json`, the existing grammar
  (`is_snapshot_file_name`). A run id cannot be in the name (it contains `-`,
  outside a grammar a reader trusts; widening it is a trust-boundary change for
  no gain). `Context::recapture` stamps with the process start, so the file's
  `capturedAt` is the **process start**; D8 requires it to equal the record's
  `startedAt`, which ties file to run. Two captures in one second (a manual
  `snapshot` command in the same second) make the run halt `SAFETY_BLOCKED`
  before anything changes: failing closed, stated in the schema.
- **How.** `snapshot_store::save_snapshot_durable` publishes with
  `write_new_durable` (a link that can never replace, directory synced); it does
  not copy `save_snapshot`'s `symlink_metadata` pre-check. The file is **read
  back through `load_state_snapshot` (D8a) before the marker is written**, so a
  run never changes the machine behind a snapshot its own reader refuses.
- **When.** `SNAPSHOT_BEFORE` is plan step 1: after the marker has been looked
  for (a stranded marker refuses before any file is written) and before it is
  written (D-M1: this step writes only this tool's state). A failure halts the
  run before anything changes and records nothing. A leftover snapshot from a
  run that then halted is harmless and, per 04, never pruned.
- **Structural guard.** `mutating_action` refuses `DispatchMutating` unless
  `SNAPSHOT_BEFORE` was recorded `SnapshotTaken` (the same pattern as
  `destination_verified_before`), so 04's "before the first mutating step" holds
  by construction and not only because the loop stops at a halt.
- **Never under `--replay` / `--from-snapshot`**: the live-scope refusal (D13 of
  slice 4) stays first.
- **The marker names the snapshot.** The marker is written after the snapshot
  step, so it can carry the snapshot's file name: **`run-marker.v2`** (adds
  `snapshotFile`), which lets "started, not recorded" point at the one trace
  that exists. The reader refuses any other version; no build ever wrote a v1.

**D2 — the brackets are performed through the application's actions.**
- `Action::TakeSnapshot` and `Action::RecordHistory` are answered by
  `next_action_with` only under an allowance, for `SNAPSHOT_BEFORE` and
  `RECORD_HISTORY`. Without an allowance `next_action` still answers
  `PassOver(NotPerformedInThisBuild)`: the shipped read-only loop and
  `every_plan_this_planner_builds_is_refused…` are untouched.
- New dispositions: `SnapshotTaken` and `RecordPendingReadBack`. **Both count as
  carried out** in `outcome_of` (one function computes the stored ending and the
  in-memory outcome: the run did all its steps). Whether the record is
  *confirmed* is a separate fact the report carries (D3), not a halt: a failed
  write or read-back keeps the run's outcome and turns a 0 into `IO`, exactly
  slice 4's D8 rule (`RunReport::exit_code`). No new halt reason.
- The shell holds no decision. The read-back comparison is an application
  function, `application::history::read_back_matches(written, read)`, comparing
  **canonical bytes**; the shell performs the I/O and hands the bytes back.
  New observer boundaries: `SnapshotWritten` (between `MarkerRead` and
  `MarkerWritten`) and `RecordReadBack` (between `RecordWritten` and
  `MarkerCleared`).
- **One write site for the record.** The in-loop `RecordHistory` action only
  records the pending disposition; the existing post-loop `write_record` is the
  only write.

**D3 — `RECORD_HISTORY`: pending, judged on read-back (settles SE-092's
"Provisional"); standing; when the marker clears.**
- A record never certifies its own write. The stored `RECORD_HISTORY` result is
  `RECORD_PENDING_READ_BACK`. The record is **confirmed** only when
  `write_record` returned `Ok` (durable) **and** a read-back of the published
  file under the same lock equals what was written. After `Err` or `Exists`
  there is no read-back and no promotion: whatever stands at that id is not this
  run's confirmed record.
- **The marker is the confirmation.** It is cleared only on a confirmed record
  (or at once when nothing mutating was dispatched). A record's **standing** is
  `CONFIRMED` (no marker names it, and the marker was readable), `UNCONFIRMED`
  (the marker names its id) or `UNKNOWN` (the marker is unreadable, or the root
  is not private). `UNKNOWN` is never restorable and never recognised.
- The stored status treats the pending step as carried out, so it reads
  `SUCCESS`; the schema says in words that a `SUCCESS` record is confirmed only
  by the absence of a marker for its id.
- **Recognition (INV-16).** `HistoryView` carries each record's standing;
  `recognise_retained_run` recognises only `CONFIRMED`
  (`NotRecognised::RecordNotConfirmed` otherwise). The marker is peeked at
  planning and again under the lock, **before** the recognition, so a re-run can
  never exit 0 on a record not known to be durable (04: "never exits 0").
- Every ending still records (slice 4 D8): a halted run's `RECORD_HISTORY` is
  `NOT_REACHED`, and the write and read-back happen all the same.
- `left()` sets `marker_left` only if a marker was written, and a test makes a
  record without a marker impossible to report as "marker left".
- A read-back that fails is reported `FAILED`, one that differs `DISAGREED`
  (`recordReadBack` in `migrate.v1`); both keep the marker and exit non-zero.

**D4 — schema `history.v2`; v1 is superseded, never written.**
- New results `SNAPSHOT_TAKEN` (only on `SNAPSHOT_BEFORE`) and
  `RECORD_PENDING_READ_BACK` (only on `RECORD_HISTORY`). **The stored
  `restorability` is dropped:** it depends on the snapshot's presence on disk,
  which is not the record's to claim and changes after the write. It is derived
  on read (D8).
- **`from_stored` refuses explicitly:** `VERIFIED` on `RECORD_HISTORY` (the
  "Provisional" defect itself); a `SNAPSHOT_TAKEN` without a `snapshotPath` and
  the reverse; a record with a dispatched mutating step whose `SNAPSHOT_BEFORE`
  is not `SNAPSHOT_TAKEN` (`NOT_PERFORMED_IN_THIS_BUILD` stays valid only where
  no mutation was dispatched); `SUCCESS` with `RECORD_HISTORY` not pending; a
  `restorability` key.
- `HISTORY_SCHEMA_VERSION` becomes 2; the reader refuses any other version, so
  neither reader accepts the other's file. No shipped build wrote a v1 record.
  `history.v1.md` gets a "superseded, never written" banner; `history.v2.md`
  carries the sample, read back byte for byte by the existing sample test.
- The 04 edits (lines on History and Rollback) land in the same commit.

**D5 — the order of a wired run.**
`probe → lock → recapture/replan → marker read (refuse if stranded) →
SNAPSHOT_BEFORE (write, read back) → [read steps] → marker written (before the
first mutating command, naming the snapshot) → per command: still-current,
re-read, recheck, dispatch → verification → record written → record read back →
marker cleared`. The order test gains the two boundaries. Each is made to fail
in turn: snapshot write fails → nothing dispatched, nothing recorded, no marker;
write fails → marker kept, exit 1; read-back fails or differs → marker kept,
exit non-zero.

**D6 — a forced run reaches `SUCCESS`; the flip tripwire turns positive.**
- Toward Homebrew (the slice 4 narrowing removes `VERIFY_RESOLUTION` and
  `SET_PRIMARY`'s command) a forced run ends `Completed`, exit 0, `SUCCESS`,
  snapshot present and named, marker cleared. Toward MacPorts the add is
  narrowed away as before.
- `the_switch_needs_snapshot_before_and_record_history_performed` stops looking
  for the pass-over text. It becomes a check scoped to the **body of
  `run_with_mutations`** and ordered, like the other wiring tripwires:
  `save_snapshot_durable(` before `run_marker::write(`, `read_back` before
  `run_marker::clear(` on the record path, and the application answering
  `TakeSnapshot`/`RecordHistory` under a forced allowance. Whitespace-insensitive,
  test modules cut, fails while the constant is `true` if any part is missing and
  fails as a stale detection if the match is lost.
- **The CLI half of `invariant_17_retained_run_recognition_is_direction_neutral`:**
  a forced retain-source run writes a `SUCCESS` record; the same command line
  again is recognised as done (INV-16), in both directions, on fixtures; an
  `UNCONFIRMED` record is **not** recognised (the same test).

### Part B

**D7 — `history` and `history show <id>`: read-only.**
- **Nothing in the state directory** is created: no directory, lock or file.
  An unusable store or record is `HISTORY_UNUSABLE` (exit 8), never "no
  history". Refused first, before the state directory is resolved, under
  `--replay` / `--from-snapshot` (INV-15) and as root (INV-06).
- **List.** `history_store::list_records(state, euid, limit)`: newest first (by
  id stamp; two runs in one second are ordered by their random suffix, which is
  deterministic but not chronological, and the output says so). `--limit`
  defaults to 20 and is capped at 200 (above: usage, exit 2). Each record goes
  through `from_stored`; an unusable one in the page fails the command naming
  its file (exit 8). Per run: id, start, package, direction, status,
  restorability with its reason, the snapshot file name, and the record's
  standing. The list's snapshot look is metadata-level (regular file, owner,
  mode, size); the full parse is in `show` and `restore`, and the output says
  `PRESENT` is metadata-level in the list.
- **Show.** The id passes `domain::is_run_id` and `history_store::record_file_name`
  before it becomes a path: not a run id → usage, exit 2. A valid id with no
  record → a new `ErrorId::RunNotFound` (exit 8, an existing code) with its own
  remediation ("`history` lists the runs this state directory holds"); never a
  guess. Single-record reads apply `private_directory` to the state root and
  `history/`, as `listed_ids` does.
- `--json`: **`history-list.v1`** and **`history-show.v1`** (the record's `v2`
  document wrapped with the derived `restorability` and `standing`), each
  documented in `docs/schemas/` with an SE entry.
- Every text through the catalogue; every record-derived value through
  `escape_controls` (Cf characters, bidi included, pass `from_stored`); home as
  `~`.

**D8 — restorability is derived, never trusted** (`application::history::restorability`, pure; N7).
Inputs: the record, the snapshot's state (`PRESENT` / `MISSING` / `UNREADABLE`,
by `load_state_snapshot`, D8a, and `capturedAt == record.startedAt`) and the
record's standing. First match:
1. no mutating step dispatched → `NOTHING_TO_RESTORE`;
2. standing not `CONFIRMED` → `NOT(RECORD_NOT_CONFIRMED)` for `UNCONFIRMED`,
   `NOT(STANDING_UNKNOWN)` for `UNKNOWN` (04: never offered for restore);
3. no `snapshotPath` → `NOT(NO_SNAPSHOT_TAKEN)`;
4. snapshot missing / unreadable / not this run's → `NOT(SNAPSHOT_MISSING | SNAPSHOT_UNREADABLE | SNAPSHOT_NOT_THIS_RUNS)`;
5. a dispatched mutating step with no known exit-0 end → `PARTIALLY(OUTCOME_UNCERTAIN)`;
6. otherwise **`FROM_SNAPSHOT`** (not `FULLY`: 04 says undo must not read as
   more reversible than it is).

`PARTIALLY` and `FROM_SNAPSHOT` both still produce a plan (04: a partial success
"is precisely when it is needed"); `restore --plan` takes its own capture, so
rule 5's uncertainty is something the plan's review handles, not a refusal.
**Declared against 04.** 04 says a restore step whose artifact is gone becomes a
`MANUAL_REVIEW` step and `history` reports the run partially restorable.
`history` is read-only and starts no provider, so it cannot know availability;
and the **planner does not raise `MANUAL_REVIEW` for an unavailable artifact**
(a destination that is not prebuilt adds only a non-blocking
`destination_source_build` risk, `planner.rs`), so that MUST is **unmet today
for every plan, migrate included**. This slice does not change the planner
(`migrate` behaviour would change); it declares the gap in 04 and SE-092, shows
the planner's risks on the restore plan, and carries the blocking rule as a
Tier 3 follow-up. The reviewers are asked at the end whether that is acceptable.

**D8a — the snapshot read.** `snapshot_store::load_state_snapshot(state, name,
euid)`: refuse a symbolic reference, owner = the effective user, no group or
other access, `snapshots/` and the root private, size capped, errors name the
file and never `path.display()`. `load_snapshot` stays for user-named paths.

### Part C

**D9 — `restore --plan --run <id>`: an ordinary plan.**
- **Intent from the record and its snapshot.** A pure
  `application::restore::restore_request(record, snapshot) -> Result<RestoreIntent, RestoreRefusal>`
  builds the reverse `PlanRequest`: package = the destination package, `to` =
  the original source provider. **`retain_source` is false (the destination
  package is removed) only if** the record shows `ADD_DESTINATION` dispatched
  with an exit-0 end **and** the snapshot shows the destination did not hold the
  package before the run; otherwise the request keeps the destination (verify or
  reinstall the original source, remove nothing). A run made with
  `--retain-source` is the cheap case through the same rule.
- **Built through `plan_outcome`,** not `plan_run`, on a **live capture** (the
  registry parts and `Context::recapture` the migration path uses), so a machine
  already back in its pre-run state is a normal outcome — "nothing to restore",
  exit 0 (INV-16) — and not a usage error.
- **Same package, or a stop.** The plan's destination package is compared with
  the record's source package and the snapshot; a mismatch (a one-to-many or
  one-way mapping) is reported as a refusal naming both packages and is never a
  silent substitute. A snapshot missing or unreadable is exit 8 with the reason.
- **Notes live outside the plan.** `--json` is **`restore.v1`**:
  `{ restoreOf: { runId, snapshotFile, standing, restorability }, notes, plan }`
  with `plan` exactly `plan.v1`. Notes carry the version differences from the
  snapshot (not version-exact, per 04) and, for a keep-the-destination plan, that
  its `VERIFY_DESTINATION` carries no check this build can evaluate, so it is
  refused by the gate in this build (D11 of slice 4).
- **INV-01 / 04 / 19 by construction,** not re-implemented: `check_ordering`,
  the reverse-dependency rule and the `STATEFUL`/`SERVICE` rule are the
  planner's. Tests prove them on restore plans.
- **An unconfirmed record** is planned, marked "record not confirmed" in text
  and `restoreOf.standing`, because the user must see the undo plan for the
  stranded run **before** they acknowledge it; a future executing `restore`
  refuses it. `UNKNOWN` standing is refused.
- Reads only: no lock, no file. Refused under `--replay` / `--from-snapshot`
  and as root, before the state directory is resolved.

**D10 — what `restore` refuses, and exit codes.**
- `restore --run <id>` without `--plan`: builds the same plan and hands it to
  the **same `execution_refusals` gate** `migrate --yes` reaches, which refuses
  it (exit 4, exactly as `migrate --yes` does today). It dispatches nothing and
  takes no lock. When restore executes, this exit does not change.
- `restore --plan --snapshot <file>` (additive whole-machine reconcile): **not
  built**; refuses with `NOT_IMPLEMENTED_IN_THIS_PASS` (exit 1) and a clear "not
  in this build"; 04 gets a Tier 3 pointer.
- **A restore plan holding `MANUAL_REVIEW` exits 4,** as `plan` and `migrate` do
  (a restore-only rule would be the special case INV-17 forbids).

**D11 — direction neutrality (INV-17), on fixtures both ways.** Restore plans
are built both directions from a recorded run and a fixture capture; the
mirror snapshot `both-providers-intel-macos26-live.json` is the
MacPorts-to-Homebrew route. The restore code names no provider.

### Part D

**D12 — `doctor` shows a stranded marker** (`/add-doctor-check`).
- `doctor(snapshot, policy, today)` stays pure. A new input `RunFacts`
  (`NOT_CHECKED`, `NONE`, `IN_PROGRESS`, `POSSIBLY_IN_PROGRESS`,
  `STRANDED { run_id, snapshot_file, record: NONE | UNCONFIRMED | UNREADABLE, changed }`, `UNREADABLE`)
  is supplied by the shell. The marker is read **without a lock and creating
  nothing** through a new `run_marker::peek` (same checks as `read`, with
  `private_directory` on the root). Under `--replay` / `--from-snapshot` and as
  root the facts are `NOT_CHECKED` and the check says so.
- **Stranded or in progress?** `run.lock` is opened read-only (no create,
  refuse a symbolic reference) and `try_lock`ed: held → "a run is in progress";
  missing or untestable → "possibly in progress". A brief hold can give a
  starting `migrate` exit 7; the output says so.
- The check id is `interrupted_run`. It states the run id, the snapshot file the
  marker names, whether a record exists and in which of the three states of D14,
  what the run changed (step kinds dispatched, from the record, never plan
  text), and what to do. `doctor`'s `--json` and `--report` schema change is
  documented.
- **Tripwires.** `the_switch_needs_doctor_to_show_a_stranded_marker` is
  satisfied by the check existing **and** a wiring tripwire that the live doctor
  path calls `run_marker::peek`, so a doctor that always passes `NOT_CHECKED`
  cannot satisfy it by name.

**D13 — `history acknowledge <id> --yes`, and the change to D-M4.**
- D-M4 said no path deletes a marker but the run's own `clear`; this slice adds
  exactly one. Without `--yes` it shows what it knows and what it would do and
  changes nothing (the default for anything destructive, 06).
- **Order.** Validate the id (usage, exit 2) → refuse replay and root → peek
  the marker; **no marker: "nothing to acknowledge", exit 0 (INV-16), no lock,
  no state directory created** → take the lock (held: exit 7) → read the marker
  under it; **it names another run: refuse, exit 4, naming both ids** (`NotOurs`).
- **It never promotes an unproven record (SG V1).** For a marker whose record
  exists, acknowledge first makes the record durable under the lock (opens it,
  `fsync`s it and `history/`, new `history_store::make_durable`) and reads it
  back through `from_stored`; only then does it clear the marker. A record that
  is unreadable, or cannot be made durable, **fails closed**: the marker stays.
  For a marker with no record, `--yes` prints what is lost — the marker (and the
  snapshot file it names) is the only trace of a run that may have left the
  provider half-changed — and then clears it.
- Clearing is `run_marker::clear` on an `InterruptedRun` built from the typed
  id (no second delete path). It deletes no record and no snapshot.
- `restore --plan --run <id>` for the marker's run shows the undo plan and does
  **not** clear the marker: a plan is not an action.
- Added to 06's command list with an SE entry, `wiki/CLI-Reference.md` and the
  README; `--json` is `history-acknowledge.v1`.

**D14 — one description of each stranded state.** From disk there are three,
and `doctor`, `history`, `history show` and `acknowledge` use the same text:
1. **Recorded, not confirmed** (record present, parses, marker names it — a
   failed marker clear, an unconfirmed directory sync, or a read-back that
   failed or differed): *the record exists; whether it reached the disk is not
   confirmed; nothing is deleted; `history show <id>`, then
   `history acknowledge <id> --yes`, which makes it durable and checks it first.*
2. **Recorded, unreadable** (record present but refused by `from_stored`, or a
   different file at that id): *the record cannot be trusted; the marker stays;
   acknowledge refuses; look at the file by hand.*
3. **Started, not recorded** (no record): *the run began and left no record; the
   provider may be half-changed; the marker names the snapshot taken first
   (`snapshots/<file>`), which is the only trace; check the provider by hand;
   then acknowledge.*
A record that is not durable is never deleted, in all three texts.

**D15 — D-M3's by-hand remedy is replaced everywhere:** `locales/en.toml`,
`locales/pt-BR.toml`, `docs/schemas/run-marker.v1.md` (now `v2`),
`wiki/Restoring-After-a-Migration.md`, the `migrate.history.*` remedy texts. The
warning that a non-durable record is the only trace and is never deleted stays.

**D16 — what a mutating run prints at the end** (04 "Undoing a migration"). The
snapshot file name and the exact command for its own id
(`macportsbrew restore --run <id> --plan`), **whatever the outcome and standing**
(04: "whatever its outcome"); when the record is unconfirmed, also the
acknowledge line after it says so. `RunReport` gains `runId`, `snapshotFile` and
`recordReadBack`; `migrate.v1` gains them additively. The new values are named
in the SE entry (`SNAPSHOT_TAKEN`, `RECORD_PENDING_READ_BACK`,
`recordReadBack: FAILED | DISAGREED`).

### Cross-cutting

**D17 — layering and words.** Decisions are pure in `application`; the `cli`
shell performs I/O; adapters hold file code; no provider name above
`crates/adapters/`; no mutating verb as a literal in `crates/*/src` (the neutral
`"verb"`; fixture files for real command text); `unsafe` stays forbidden; no new
dependency (`File::try_lock` is std); no new spawn site; `Cargo.lock` unchanged.

**D18 — carried, each behind its flip tripwire, untouched.** Consent wiring; the
executed plan tied to the one shown (obligation 8); the window before removal;
`PROVIDER_REPORTS_ACTIVE`; release builds free of `test-support`; the
observed/unchecked-provider refusal (DP-030); shim-backed recognition; the umask
test (DP-029); a dedicated "provider command failed" identifier; typed-ahead
consent input and the token carrying its program. **Newly carried by this
slice:** the planner's blocking rule for an unavailable restore artifact (D8).

## 3. Where the code goes

| Change | File |
|---|---|
| `TakeSnapshot`, `RecordHistory`; `SnapshotTaken`, `RecordPendingReadBack`; the `mutating_action` snapshot guard; `outcome_of` | `crates/application/src/executor.rs` (+ `executor/tests.rs`) |
| `restorability`, standing, `read_back_matches`, `NotRecognised::RecordNotConfirmed`, `run_record` | `crates/application/src/history.rs` |
| `restore_request`, `RestoreIntent`, `RestoreRefusal` | `crates/application/src/restore.rs` (new), `lib.rs` |
| `interrupted_run` check, `RunFacts` | `crates/application/src/doctor.rs` |
| new step results, schema 2, drop stored restorability, new `from_stored` refusals, `Restorability` reasons, `ErrorId::RunNotFound` | `crates/domain/src/{history,error}.rs` |
| `save_snapshot_durable`, `load_state_snapshot` | `crates/adapters/src/persistence/snapshot_store.rs` |
| `read_back`, `list_records`, `make_durable`, standing | `crates/adapters/src/persistence/history_store.rs` |
| `peek`, marker v2 (`snapshotFile`) | `crates/adapters/src/persistence/run_marker.rs`, `InterruptedRun` |
| the actions performed; `SnapshotWritten`/`RecordReadBack`; `RunReport` fields | `crates/cli/src/executor/mod.rs` |
| `history`, `history show`, `history acknowledge`, `restore` | `crates/cli/src/app/{mod,history,restore}.rs` (new), `human.rs`, `context.rs` |
| catalogue | `locales/en.toml`, `locales/pt-BR.toml` |
| order test, success, failing boundaries, both directions | `crates/cli/tests/executor_wiring.rs` |
| flip tripwire positive; the doctor peek tripwire | `crates/cli/tests/history_preconditions.rs` |
| schemas | `docs/schemas/{history.v1,history.v2,history-list.v1,history-show.v1,history-acknowledge.v1,restore.v1,run-marker.v1,run-marker.v2,migrate.v1,doctor.v1}.md` |
| spec and rationale | `docs/spec/04`, `06`; `docs/spec-evolution.md` (SE-092 + entries); `docs/decisions-pending.md` if an owner decision turns up |
| user docs | `wiki/CLI-Reference.md`, `wiki/Restoring-After-a-Migration.md`, `README.md`, `CHANGELOG.md` |

No guarded path changes. No dependency; `Cargo.lock` unchanged;
`docs/spec/01-invariants.md` untouched. Nothing under
`crates/adapters/src/process/` changes.

## 4. Tests

Invariant tests are named `invariant_NN_*` (find them with
`grep -rn "fn invariant_" crates/*/tests crates/*/src`).
- **A:** forced Homebrew run → `Completed`, exit 0, `SUCCESS`, snapshot present
  and named by the record **and** the marker, marker cleared, record reads back;
  the order test with the two new boundaries; each boundary failing in turn;
  the snapshot is `now`, asserted on content that differs between the captures;
  the record's source version equals the snapshot's;
  `invariant_03_a_pending_record_history_is_never_a_confirmed_success` (a run
  whose write or read-back failed has a non-zero exit and `recordReadBack`);
  `VERIFIED` on `RECORD_HISTORY`, a v1 file, a v2 file with `restorability`, a
  mutating record without `SNAPSHOT_TAKEN`: each refused by `from_stored`;
  no read-back after `Err`/`Exists`; `DispatchMutating` refused without
  `SnapshotTaken`; an `UNCONFIRMED` record not recognised, the CLI half of
  `invariant_17_retained_run_recognition_is_direction_neutral` both ways;
  `invariant_16_a_confirmed_re_run_is_nothing_further`.
- **B:** list/show on a scratch store: order, limit and the cap, an unusable
  record in the page → 8, invalid id → 2, valid missing id → `RUN_NOT_FOUND`
  (8), nothing created, root and replay refused first
  (`invariant_15_history_restore_and_acknowledge_refuse_under_replay`,
  `invariant_06_history_restore_and_acknowledge_refuse_root`); every row of D8
  with its mutant; a planted symlink or foreign-owner snapshot is not `PRESENT`;
  JSON against its schema; `doctor` under `--from-snapshot` byte-identical with
  `NOT_CHECKED`.
- **C:** restore plans in both directions
  (`invariant_17_restore_is_direction_neutral`,
  `invariant_01_a_restore_plan_adds_before_it_removes`, `invariant_04_*` and
  `invariant_19_*` on restore plans); the retain-source cheap case; a run whose
  destination already held the package, and one whose add failed, plan no
  removal; a one-to-many mapping is refused by name; an already-restored machine
  is "nothing to restore" (`invariant_16_*`); missing/unreadable snapshot → 8;
  an unconfirmed record planned and marked, `UNKNOWN` refused; `restore` without
  `--plan` reaches the gate and exits 4; `--snapshot` exits 1; a plan with
  `MANUAL_REVIEW` exits 4;
  `invariant_02_every_restore_plan_is_refused_for_execution_in_this_build`.
- **D:** the `doctor` check in `STRANDED` × three record states, `IN_PROGRESS`,
  `POSSIBLY_IN_PROGRESS`, `UNREADABLE`, `NOT_CHECKED`; acknowledge: refuses
  `NotOurs`, refuses under a held lock (7), does nothing without `--yes`,
  nothing-to-acknowledge exits 0 without creating anything, makes the record
  durable and reads it back **before** clearing (a mutant that clears first
  survives nothing), fails closed on an unreadable record, deletes no record;
  the three states described identically in all four places.

## 5. Docs in the same commits

04 (Rollback and snapshots; History: representation, status, restorability,
the availability gap; Concurrency: D-M4 changed, D-M3 replaced); 06 (command
list gains `history acknowledge`, the `restore` refusals and exit mapping); the
schemas in §3; SE-092 (settling "Provisional", D1's reading, D4, D8's gap, D-M4)
and an SE entry per new JSON surface and command; the wiki, README ("What this
build does *not* do"), CHANGELOG `[Unreleased]`.

## 6. Order of work and the proof

1. This plan, copied to `docs/passes/08-slice-5-plan.md`; the tooling re-check
   notes saved and the read dates updated (guarded files are not touched; their
   recommendations go to the report).
2. Part A → commit. 3. Part B → commit. 4. Part C → commit. 5. Part D → commit.
   Each commit passes the commit gate (`validate.sh`, about 5 minutes warm), run
   in the background with nothing else compiling.
3. **Mutation proof** in an isolated worktree through a subagent: swap each
   adjacent pair of boundary operations, drop each, snapshot the first capture
   instead of `now`, skip the snapshot read-back, clear the marker before the
   read-back, clear it after a non-durable write, read back after `Exists`,
   treat pending as verified, invert the read-back comparison, recognise an
   `UNCONFIRMED` record, every row of D8 (swap two, drop one, invert), plan the
   removal without the add-with-exit-0 check, accept a different package,
   `acknowledge` clearing before `make_durable`, skipping `NotOurs`, skipping
   the lock, ignoring `--yes`, skipping id validation, `doctor` ignoring the
   lock test, each flip tripwire with the constant `true`. **I read the
   survivors**; each non-equivalent one gets a test checked against its mutant,
   or an argument in the report.
4. End-of-slice review by `spec-guardian`, `security-reviewer`, `docs-auditor`
   over `main...HEAD`; each finding resolved or accepted in writing (§10).
5. Coverage at or above the floors in `docs/spec/07-quality-and-testing.md`;
   `scripts/validate.sh`; "As built" here; the report in
   `docs/passes/08-slice-5-report.md`; push the topic branch
   `pass/08-slice-5-history-and-restore`; open the PR; **never merge**.

## 7. Review resolution (revision 1 → 2)

SR = `security-reviewer`, SG = `spec-guardian`.

| Finding | Disposition |
|---|---|
| SR H1 / SG R1 — an unconfirmed `SUCCESS` record makes a retain-source re-run "already done" | Fixed: D3, standing travels in `HistoryView`; only `CONFIRMED` is recognised; the marker is peeked at planning and under the lock |
| SR H2 — restore plans the removal of a package that predates the run or whose add never ran | Fixed: D9, the removal needs the add dispatched with an exit-0 end and the destination absent in the snapshot |
| SR M1 — no bounded, owner-only snapshot read exists | Fixed: D8a `load_state_snapshot` |
| SR M2 / SG V3 — the marker's own run could not be planned before acknowledging | Fixed: D9, an unconfirmed record is planned and marked |
| SR M3 — `doctor` cannot tell a stranded marker from a run in progress | Fixed: D12, a read-only `try_lock` |
| SR M4 — removing the marker upgrades a record | Fixed: D13, `acknowledge` makes the record durable and reads it back first, and never promotes an unreadable one (also SG V1) |
| SR M5 — marker-clear condition on a non-durable write | Fixed: D3, `Ok` write and matching read-back; none after `Err`/`Exists` |
| SR M6 / SG U6 — `restore` under replay; the Verification text | Fixed: D9, D13, §4, Verification |
| SR M7 / SG V4 — the reverse plan re-maps | Fixed: D9, package comparison; `plan_outcome` for "already restored" |
| SR M8 — an unreadable marker has no standing | Fixed: D3, `UNKNOWN` |
| SR M9 — `--limit` unbounded; a snapshot parsed per row | Fixed: D7, cap 200, metadata-level look in the list |
| SR L1 — both captures share `captured_at` | Fixed: D1, content-based assertion; `capturedAt` is the process start, declared |
| SR L2 — collisions; the marker holds no snapshot name | Fixed: D1 (`run-marker.v2` names the snapshot; same-second collision fails closed); D8 ties the file to the run |
| SR L3 — the run never reads back its snapshot | Fixed: D1 |
| SR L4 — `acknowledge` can create the state directory | Fixed: D13, peek before the lock |
| SR L5 — directory checks on single reads and `peek` | Fixed: D7, D12 |
| SR L6 — "reads create nothing" | Fixed: "nothing in the state directory" |
| SR L7 — Cf characters in record values | Fixed: D7, `escape_controls` everywhere |
| SR L8 — home paths in errors and `--report` | Fixed: D8a (file names only); `doctor` schema in §3 |
| SR L9 — read-back by `PartialEq` can pass vacuously | Fixed: D2/D3, canonical bytes |
| SG V2 — the planner raises no `MANUAL_REVIEW` for an unavailable artifact | Declared, not fixed: D8 — the MUST is unmet for every plan; recorded in 04 and SE-092, carried as a Tier 3 planner follow-up; the planner's risks are shown on the restore plan |
| SG V5 — exit 0 for a shown restore plan with `MANUAL_REVIEW` | Fixed: D10, exit 4 like `plan` and `migrate` |
| SG U1 — D2 changed slice 4's exit rule | Fixed: D2, the outcome stays `Completed`; a failed write or read-back turns a 0 into `IO` as in slice 4 |
| SG U2 — which function computes the stored ending | Fixed: D2, `outcome_of` alone; in the mutation proof |
| SG U3 — D16 dropped the command for unconfirmed records | Fixed: D16, always printed |
| SG U4 — wrong remedy for a missing record | Fixed: D7, `ErrorId::RunNotFound` |
| SG U5 — `restore --run` without `--plan` should exit 4 | Fixed: D10, through the same gate; `--snapshot` stays exit 1 |
| SG U7 — D12/D14 inconsistent; the marker holds no snapshot | Fixed: D12, D14 (three states), `run-marker.v2` |
| SG U8 — notes inside the plan; the consolidation's verify | Fixed: D9, notes outside, with the evaluability note |
| SG (1) — record version vs snapshot | Fixed: D1 test |
| SG (2) — `from_stored` refusals; `left()` | Fixed: D4, D3 |
| SG (3) — `FULLY` label; partial still plans | Fixed: D8 `FROM_SNAPSHOT`, partial plans |
| SG (6) — acknowledge with no marker | Fixed: D13, exit 0 |
| SG (8) — the tripwire is too loose; decision in the shell; one write site; doctor by name; structural snapshot guard | Fixed: D6, D2, D12, D1 |
| SG R2, R3 — order wording; additive enum values | Fixed: D7, D16 |
| SG — missing invariant-named tests | Fixed: §4 |

## 8. Second review (revision 2 → 3)

_Filled in if the revised decisions need another read before code._

## 9. As built

_Written at the end of the slice._

## 10. End-of-slice review

_Written at the end of the slice._

## Verification

`scripts/validate.sh` (default mode; `--full` before the PR), `cargo clippy
--workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all`,
`cargo test --workspace`, the coverage run, and the mutation proof of §6. The
read commands are exercised against a **scratch state directory** through the
test harness (`MACPORTSBREW_STATE_DIR`), never the developer's own and never
under `--replay`, where `history`, `restore` and `acknowledge` are only shown to
refuse.
