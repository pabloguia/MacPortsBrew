<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 5 — `history`, `restore`, and the stranded marker in `doctor`

Work order for one session. Parent: [`08-the-mutating-executor.md`](08-the-mutating-executor.md);
its §3 binds unchanged; slice 2's §3 "How this repository bites" applies. The
by-hand remedy for a stranded interrupted-run marker (D-M3) exists only until
this slice: here the user gets commands for it.

## 0. Read first, in this order

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/01-invariants.md` — INV-01, 03, 11, 15,
   16, 19.
2. `docs/spec/04-migration-and-safety.md`: Rollback and snapshots, Undoing a
   migration, History; `docs/spec/06-cli-and-ux.md` (how `history` and `restore`
   render; exit 8 for a missing snapshot, exit 4 for a `MANUAL_REVIEW` restore).
3. `docs/schemas/history.v1.md`, `run-marker.v1.md`; `wiki/Restoring-After-a-Migration.md`.
4. `docs/spec-evolution.md`: SE-092 (D1–D6, D-M3, D-M4), the `RECORD_HISTORY`
   "Provisional" note (settle it here).
5. `docs/passes/08-slice-4-plan.md` "As built".

## 1. What this slice delivers

- **`history`** (read-only): lists runs from the store, newest first; `history
  <run-id>` shows one; `--json` with a new `history-list.v1` schema. A typed id
  passes `record_file_name` before it is a path. Reading creates nothing;
  `HISTORY_UNUSABLE` is never "no history".
- **`restore --run <id>`**: builds a **plan** that reverses a recorded run from
  its record and its snapshot, through the same planner and the same gate as a
  migration — restore is a migration in reverse and is held to INV-01 (nothing
  removed before its replacement is verified) and INV-03. It dispatches nothing
  while the switch is off; with `--dry-run` it shows the plan. A missing
  snapshot is exit 8; a `MANUAL_REVIEW` step exit 4 (06).
- **The stranded marker**: `doctor` reports it (run id, whether a record for it
  exists, what to do); `restore --run <id>` and a new explicit
  `history --acknowledge <id>` are the only paths that clear a marker other than
  the run's own `clear`, each under the run lock, each refusing a marker that is
  not that run's. Replace D-M3's by-hand text in `locales/en.toml` and
  `run-marker.v1.md`.
- **Settle `RECORD_HISTORY`'s representation** (SE-092 "Provisional"): a record
  never certifies its own write.

## 2. Hard boundary

Slice 2 §2. New commands are added to `wiki/CLI-Reference.md` and the README's
"does not do" section is updated in the same change. No restore path dispatches
anything while the switch is off.

## 3. Plan, build, prove, review, record

As slice 4 §3. INV-17: restore is direction-neutral — test both directions on
fixtures (slice 3's mirror recording).

## 4. Definition of done

- [ ] `history`, `restore` (plan only), `doctor`'s marker report and the
      acknowledge path exist with named tests proved by mutation.
- [ ] `RECORD_HISTORY` representation settled in SE-092 and `history.v1.md`.
- [ ] §2 holds; reviewers run; "As built" written.
