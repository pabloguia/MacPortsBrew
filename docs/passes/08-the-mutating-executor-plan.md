<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 1 — the history store: plan

Work order: [`../../prompts/08-the-mutating-executor.md`](../../prompts/08-the-mutating-executor.md),
item 1. Drafted 2026-10-04 by a planning subagent; reviewed by `spec-guardian`
and `security-reviewer` before any code. This revision folds in both reviews;
the finding each change answers is cited in brackets (`SG` spec-guardian, `SR`
security-reviewer). Sources: SE-092's S8 addendum and its "Owed by the mutating
pass" list; `plan.v1.md` obligations 11–12; `04-migration-and-safety.md`
(History, Rollback, Concurrency).

> **As built (2026-10-04).** What landed, what changed and why, and what
> stays owed are in SE-092's history-store addendum and its amendment. In
> short: the probe call and the D4 output were deferred with the write call
> (commit 6 below did not wire them); a collision is refused, not retried;
> reading stops after the key's newest stamp group, not at its first record;
> a dry run reads history (D5); a heuristic resolution may confirm (D6);
> there is no error table in `06`, so none was edited.

## Facts that shape it

1. `RunRecord` serialises but cannot be read back, and its derived fields
   (`status`, `mutationDispatched`, `restorability`, each step's `dispatched`)
   must never be trusted from a file (INV-03). `ReadEvidence` has one
   constructor a reader cannot call.
2. Every `migrate --yes` in this build ends at the gate: `NO_CHANGE`,
   `mutationDispatched: false`. `04` says only a mutating run is written, and
   two INV-16 tests assert the state directory is untouched.
3. Every CLI test of `migrate` runs under `--replay` with a scratch
   `MACPORTSBREW_STATE_DIR`; `Context::build` decides every replay fact once.
4. Every plan's `SET_PRIMARY` runs `macportsbrew primary <pkg> <to>`, so after a
   successful run the destination resolves through MacPortsBrew's shim, and no
   capture can yet say which provider a shim points at.
5. `write_atomic` publishes by `rename`, which replaces; an append-only store
   needs a publish that cannot.
6. sudo keeps `HOME` on macOS (`system/machine.rs`), and nothing in the product
   refuses euid 0.

## What this slice does and does not close [SG H1, H2]

- **Reachability.** With `THIS_BUILD_DISPATCHES_MUTATIONS = false`,
  `mutationDispatched` is never true: no record is written and no re-run is
  recognised by the shipped binary. S8 deferred the store as "unreachable
  code"; that rationale is superseded because the work order builds the
  executor's obligations before proposing the flip, and the store is reached
  by its own tests and by the application-level recognition tests. SE-092 says
  so in the same change.
- **`RECORD_HISTORY` and `SNAPSHOT_BEFORE`.** Their dispatch stays
  `NotPerformedInThisBuild`. Once mutations dispatch, `RECORD_HISTORY` is the
  store write and is judged by reading the published file back through
  `RunRecord::from_stored` — the record never asserts its own verification:
  it is written with that step `PENDING`, and its status is derived on read.
  Wiring both steps is owed by item 7 and is a **named precondition of the
  flag-flip proposal**.
- **INV-16 stays open.** D3 refuses to recognise a shim-backed primary, and
  every plan sets one, so SE-092 S6 "Not closed (2)" is **not** closed by this
  slice. Shim-backed recognition (the shim directory's own record of which
  provider each shim targets, read live) is a **blocking precondition** of the
  flip proposal; if item 7 proposes the flip with it still open, that is the
  owner's call and the proposal says so.

## Design

- **Owner:** `crates/adapters/src/persistence/history_store.rs`, free
  functions like `snapshot_store`; no domain trait. The domain owns read-back
  validation (`RunRecord::from_stored`); the application owns two pure rules,
  `history::must_record` and `history::recognise_retained_run`.
- **Layout:** one JSON file per run, `history/run-<id>.json`. The id is a
  strict grammar, `<compact UTC stamp>-[0-9a-f]{16}`, from the one clock read
  and `RandomState`, fixed before `SNAPSHOT_BEFORE`; it is checked in
  `from_stored` and on any user-supplied id before a `join` [SR H2, L4].
- **Publish without replace:** `atomic::write_new_atomic` — dot-named `O_EXCL`
  temp inside `history/`, `fchmod 0600` regardless of umask, `sync_all`
  (std's Darwin `F_FULLFSYNC`, cited with its read date in the comment),
  `hard_link` to the target, remove the temp, best-effort directory sync. It
  **never falls back to `rename`**: any `hard_link` error but `EEXIST` means the
  filesystem cannot publish without replacing. On `EEXIST`, a temp with
  `nlink == 2` means our link landed and only the reply was lost; otherwise
  retry with a fresh suffix, a bounded number of times. No pre-link check
  [SR H1, M1, M2, L3; SG L4].
- **Probe before dispatch:** `history_store::probe` runs `ensure()` and a real
  `write_new_atomic` + unlink in `history/`. The executor's I/O shell calls it
  after the gate and before the first mutating step; failure is
  `SafetyBlocked` and nothing dispatches. `state_dir::ensure` re-reads the mode
  after a repair and fails closed if `chmod` had no effect [SR H1].
- **Reading creates nothing.** Every check is made on the opened handle
  (`bounded_read`, `RefuseSymlink`, an `accept` callback), never a separate
  `lstat`: a regular file; `mode & 0o077 == 0`; the file's uid equals
  `history/`'s, the state root's, and the effective uid (from the existing
  `$TMPDIR` probe, which creates nothing in the state directory). `nlink` is
  **not** checked — a crash between link and unlink leaves `nlink == 2`, and a
  link attack needs our own uid. History is neither read nor written at euid 0
  [SR M1, M2].
- **What is unusable** (`HISTORY_UNUSABLE`, new, exit 8): a symlink, a
  non-directory, a failed ownership or mode check, an oversized record, an
  unknown non-dot entry, an unknown schema version, an id that disagrees with
  its file name or with `startedAt`, unknown fields (`deny_unknown_fields`), a
  Control-class character in any string (`domain::text::is_control_class`; a
  Format character such as ZWJ is legal), a bad `snapshotPath`, or stored
  derived fields that disagree with re-derivation — refused, never silently
  overwritten. The error names the entry with the home redacted [SG L2, L3;
  SR L1, L4, M4].
- **Bounds:** names are sorted newest first and reading stops at the first
  record for the key; an unusable record met before that point fails closed
  (it could be a later failure for the key). A hard entry cap refuses with its
  own message [SR M4].
- **Envelope:** `{ "schemaVersion": 1, …RunRecord }`, documented in
  `docs/schemas/history.v1.md`; a store test parses its sample.
- **`snapshotPath`** holds only the snapshot's file name, checked against the
  grammar `snapshot_file_name` emits (so no absolute path, `..`, separator,
  empty component or control character, on write and on read-back). It is
  rebuilt as `StateDir::snapshots().join(name)` and opened with
  `RefuseSymlink`. A run is restorable only from the state directory it was
  recorded in; changing `MACPORTSBREW_STATE_DIR` leaves its history behind,
  which `04` documents [SG M5; SR H2].
- **argv** does not join the record (S8 decision 6): the plan's steps already
  carry each `CommandTarget`, and argv would bring paths and values the
  record otherwise keeps out [SG M6 i].
- **What is written:** a record iff `mutationDispatched` (`must_record`).
  Gate-refused, `NO_CHANGE` and nothing-to-move runs write nothing.
- **When history is read:** only by `migrate --retain-source` on a live run
  (and by item 7's `history`/`restore`). A plain `migrate`, a dry run, `plan`,
  `analyze` and `--from-snapshot` never read it, so a stray file never blocks
  them; a dry run never creates the directory [SG L5; SR M4, M5].
- **INV-15:** `HistoryScope::{ThisMachine, NotConsulted}` is an explicit field
  of `Context`, set in all three branches by one pure function of (replay,
  from-snapshot) with its own unit test; replay and `--from-snapshot` get
  `NotConsulted` even with `MACPORTSBREW_STATE_DIR` exported. Recognition is
  not a recommendation, and nothing INV-15 covers (`analyze`, `plan`,
  `--from-snapshot`) consults history; SE-092 records that argument [SG L6;
  SR M5].
- **Testability of the live branch [SG H3]:** option (b). A
  `test-support`-only seam, reachable from dev-dependencies alone (the rule
  `the_plan_editing_test_hook_is_out_of_product_source` already enforces),
  forces `ThisMachine` against a scratch directory on a replayed run. Option
  (a) is rejected because it would make replay read a real store whenever the
  variable is exported. Mutations it must kill: scope inverted, read call
  removed, recognition result ignored.
- **INV-16 recognition:** `migration_start` takes a `HistoryView`. It may only
  replace a `MigrationStart::Run` whose plan has no `MANUAL_REVIEW` step;
  every refusal and `NothingToMove` keeps precedence [SG M3]. Recognised iff:
  the latest record for (package, source, destination, destination package) is
  a retain-source `SUCCESS`; no other record for the key shares its stamp; no
  record's stamp is later than the current run's; and a fresh capture shows
  the destination holding the package active and resolving to the record's
  `primaryAfter`. A shim, unknown or absent resolution is "cannot confirm",
  said in the output, not silence. Packages with no executable are never
  recognised, so INV-16 then rests on `ADD_DESTINATION` being idempotent,
  owed later [SG M2, H2; SR M3, L2]. Recognised →
  `MigrationStart::RetainedRunDone`, an additive `retainedRunDone` in
  `migrate.v1`; a recognised re-run writes nothing.
- **Out of scope, carried with owners:** the lock and the concurrent-provider
  detector (item 2); the interrupted-run marker, added to item 2's scope in
  SE-092 [SG M6 ii]; `macportsbrew history`, `restore --run <id>` and printing
  that line (item 7, with the flip) [SG M6 iii]; the euid-0 refusal for the
  whole process, which this slice applies to history only (raised in SE-092
  as owed by item 6, elevation consent).

## Commits

1. **domain** — `RunRecord::from_stored` (re-derive and compare,
   `deny_unknown_fields`, Control-class refusal, id grammar, `startedAt`
   agreement); `ReadEvidence::from_recorded` as `pub(crate)`, called only by
   `from_stored` [SG L1]; the snapshot-file-name predicate.
2. **domain** — `ErrorId::HistoryUnusable`, exit 8.
3. **persistence** — `write_new_atomic`, `ensure`'s mode re-read.
4. **persistence** — the store, `probe`, `history.v1.md`.
5. **application** — `must_record`, `recognise_retained_run`, `HistoryView`,
   `RetainedRunDone`; `migrate` passes `NotConsulted` (no behaviour change).
6. **cli** — `HistoryScope` in `Context`, the test-support seam, live-only
   consultation under `--retain-source`, the probe in the executor shell,
   human and JSON rendering, the D4 output.
7. **docs** — `04` History (state-directory scope); `plan.v1` obligation 12;
   `docs/schemas/README.md` row 8; `06` error table; `locales/en.toml`
   message ids; `migrate.v1.md` and its checked samples; SE-092; CHANGELOG
   [SG M6 iv].

## Named tests [SG M1]

- **INV-03:** `invariant_03_a_stored_record_never_supplies_its_own_status_dispatch_or_restorability`;
  `invariant_03_a_partial_run_is_written_as_partial`; obligation 12 for every
  way a run ends — success, partial, halt after a dispatch, `MANUAL_REVIEW`
  after a change, a refusal contradicted by dispatched steps.
- **INV-15:** `invariant_15_analyze_and_plan_output_is_independent_of_history`
  (records present vs absent, byte-identical);
  `invariant_15_a_replayed_migrate_neither_reads_nor_writes_history` (with
  the variable exported); the scope function's unit test.
- **INV-16:** recognised; a later failure withdraws it; a same-stamp tie is
  not recognised; a future stamp is not recognised; no matching fresh capture;
  destination not active; resolution not `primaryAfter`; a `MANUAL_REVIEW`
  plan is not replaced; a recognised re-run writes nothing;
  `invariant_16_a_retained_run_whose_primary_is_the_shim_is_not_recognised_and_says_why`.
- **INV-17:** `invariant_17_retained_run_recognition_is_direction_neutral`, both
  directions at application level; the CLI half waits for item 5's recording.
- **Store:** writing never replaces (`EEXIST`); a lost reply (`nlink == 2`) is
  success; a crash after temp creation and after the link both leave a
  readable store; reading creates nothing; symlink, wrong mode, foreign uid,
  oversized, unknown entry, unknown schema, id/file-name and id/`startedAt`
  mismatch, unknown field, bad `snapshotPath` (`..`, absolute, separator) each
  give `HISTORY_UNUSABLE`; the entry cap; a `0277` umask still yields a
  readable record; the probe refuses where linking fails.
- **D4:** human and `--json` report success-with-history-failure distinctly,
  with the snapshot path and no `restore --run` line.

## Decisions (Tier 3, for SE-092)

- **D1** "Latest" is the latest record for the key, which must itself be a
  retain-source `SUCCESS`; a later failure withdraws recognition, and so will a
  later restore once `restore` exists. A tie or a future stamp fails closed.
- **D2** The write predicate is `mutationDispatched`.
- **D3** The primary check fails closed on a shim, leaving "Not closed (2)"
  open; shim-backed recognition blocks the flip proposal.
- **D4** The probe makes an unwritable store a refusal before dispatch. A write
  that fails anyway after a mutating run keeps a non-zero exit, turns an exit
  0 into `IO`, and prints the real outcome, a separate "history not written"
  error and the snapshot path, never `restore --run` for the unwritten id. That
  run can then never be recognised: a known, fail-closed gap.

## Review

- **spec-guardian (2026-10-04):** H1–H3, M1–M6, L1–L6; no Tier 1/2 escalation,
  D1–D4 within authority once M2 and M4 are fixed and H2 is presented as
  leaving INV-16 open. All folded in above; option (b) chosen for H3.
- **security-reviewer (2026-10-04):** H1–H2, M1–M5, L1–L4; no blocker. All
  folded in above. Its observation outside the slice — nothing enforces "the
  process is never wholly root" — is carried as owed by item 6.
- **Conflict resolved:** SG H3 offered replay-reads-a-scratch-store; SR M5
  showed that reads a real store when the variable is exported. The seam wins.
