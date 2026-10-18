# `history` record schema, version 2

The file a mutating run leaves in the state directory, `history/run-<id>.json`
(`04-migration-and-safety.md`, History). Derived from `RunRecord` in
`crates/domain/src/history.rs`; written and read by
`crates/adapters/src/persistence/history_store.rs`. The compatibility rule is
in [`README.md`](README.md). **Version 2 supersedes [version 1](history.v1.md),
which no build ever wrote** (pass 08 slice 5; SE-092).

**This build writes none to a user's machine.** A record is written iff the run
dispatched a mutating step (`mutationDispatched`), and the gate refuses every
plan with one while `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`; the format is
fixed and tested now so the pass that turns mutations on writes this shape.

## What changed from version 1, and why

- **`RECORD_HISTORY` is stored pending, never verified.** A record cannot
  certify its own write. The step's result is `RECORD_PENDING_READ_BACK`; what
  confirms the record is a read-back of the published file under the run lock,
  and then the interrupted-run marker being cleared. **A `SUCCESS` record is
  confirmed only by the absence of a marker naming its id** (see "Standing").
- **`SNAPSHOT_BEFORE` is performed.** Its result is `SNAPSHOT_TAKEN`, and the
  snapshot's file name is `snapshotPath`. A record that dispatched a mutating
  step without it is refused.
- **`restorability` is no longer stored.** Whether a run can be put back depends
  on a snapshot that can disappear after the record was written, so it is
  derived when the record is read (`history`), from the record, the snapshot and
  the record's standing. A file that states one is `HISTORY_UNUSABLE`.
- Why a new version rather than additive values: the reader of one version
  refuses the other (`schemaVersion` is checked first), so neither can accept
  the other's file silently. No shipped build wrote a version 1 record, so
  nothing needs migrating.

## How it is written and read

- **One file per run, never replaced.** Published by a hard link that cannot
  replace an existing file; nothing deletes a record.
- **Read back without being trusted.** `RunRecord::from_stored` derives the
  steps' `dispatched`, `status` and `mutationDispatched` again and refuses a
  file that states anything else (INV-03). The one exception is a `HALTED` step
  with no evidence and no `ends`, whose `dispatched` is the record's own
  statement: nothing else in the record can settle it.
- **Published durably:** the record's directory entry reaches the medium before
  the writer reports success, because the run clears its interrupted-run marker
  only after. A write whose directory could not be synced is reported as not
  known to be durable — the record may exist, and is never deleted.
- **Anything this writer would not have left is `HISTORY_UNUSABLE`** (exit 8),
  never "no record": another `schemaVersion`, a field this version does not
  know (at any level, `restorability` included), an `id` outside the grammar
  below or naming another instant than `startedAt`, a Control-class character
  in any string, a `snapshotPath` that is not a snapshot file name, evidence
  whose `passed` disagrees with its `cause`, a `VERIFIED` step with failed
  evidence, a `MANUAL_REVIEW` step stated as carried out, a `VERIFIED`
  `RECORD_HISTORY`, a result on a step it does not belong to, a `SNAPSHOT_TAKEN`
  without a `snapshotPath` or the reverse, a dispatched mutating step without
  `SNAPSHOT_TAKEN`, a record under another run's file name, a file that is a
  symbolic reference, larger than 256 KiB, owned by another user or open to
  group or other access, a directory entry in `history/` that is not a record
  or whose name is not text, a `history/` or state directory that is a symbolic
  reference, not a directory, another user's or open to others, and more than
  10,000 entries in `history/` — hidden ones included, since nothing prunes the
  store yet. Dot-entries are the writer's temporaries and are otherwise ignored.
- **Never read or written as root** (`SAFETY_BLOCKED`, exit 4), and never under
  `--replay` or `--from-snapshot`. A write that meets an existing record of the
  same id is refused (`IO`) rather than given another id: the id is fixed before
  the run's first step, and nothing is read back after such a refusal.

## Standing: is the record confirmed?

A record is **confirmed** when no interrupted-run marker names its id and the
marker could be read. The run clears its marker only after the record was
written durably **and** a read-back under the lock equals what it wrote. So:

| Marker | Standing | Meaning |
|---|---|---|
| none | `CONFIRMED` | published, durable, read back |
| names this record's id | `UNCONFIRMED` | published; the run could not confirm it (the marker clear failed, the directory sync was not confirmed, or the read-back failed or differed). Nothing is deleted |
| unreadable | `UNKNOWN` | nothing is known; never recognised, never offered for restore |

An `UNCONFIRMED` record is never offered for restore and never recognises a
`--retain-source` re-run as done. `history acknowledge <id> --yes` makes it
durable under the lock, reads it back, and only then clears the marker, so
acknowledging it makes the record count as confirmed because it is.

## Root

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `2` |
| `id` | string | `<YYYYMMDDTHHMMSSZ>-<16 lowercase hex>`; the stamp is `startedAt`'s instant, and the file is `run-<id>.json` |
| `startedAt` | string | RFC 3339 UTC, `YYYY-MM-DDTHH:MM:SSZ` |
| `toolVersion` | string | |
| `planSchemaVersion` | integer | The plan the run executed |
| `policyVersion` | string | The policy data the plan was built under |
| `package` | object | `provider`, `name`: the package under its source provider |
| `direction` | object | `source`, `destination` |
| `destinationPackage` | string \| null | |
| `versions` | object | `source`, `destination`: string \| null — null is "not established", never "none" |
| `action` | `Outcome` | As in [`plan.v1.md`](plan.v1.md) |
| `overrode` | `Outcome` \| null | The recommendation the plan overrode |
| `retainSource` | boolean | `--retain-source` |
| `primaryAfter` | string | The provider the plan made primary |
| `environmentChanges` | array | Always empty in this version |
| `steps` | array of `Step` | In plan order |
| `ending` | object | `kind`: `COMPLETED`, `PARTIAL_SUCCESS` (`step`, `error`), `HALTED` (`step`, `error`), `REFUSED` (`refusals`) |
| `status` | string | Derived: `SUCCESS`, `PARTIAL`, `FAILURE`, `NO_CHANGE`. `SUCCESS` only when every step is carried out, the pending `RECORD_HISTORY` included |
| `mutationDispatched` | boolean | Derived |
| `snapshotPath` | string \| null | The snapshot's **file name** only, `snapshot-<1–32 ASCII alphanumerics>.json`, inside the same state directory's `snapshots/`. Non-null exactly when a step is `SNAPSHOT_TAKEN`. A run is restorable only from the state directory it was recorded in |

## `Step`

| Field | Type | Notes |
|---|---|---|
| `index` | integer | As the plan numbers it |
| `kind` | string | A `StepKind` from [`plan.v1.md`](plan.v1.md) |
| `provider` | string \| null | |
| `dispatched` | boolean | Derived from `result`, except for a halt with no evidence |
| `result` | object | `result`: `VERIFIED` (`evidence`), `NOT_PERFORMED_IN_THIS_BUILD`, `SNAPSHOT_TAKEN`, `RECORD_PENDING_READ_BACK`, `NOT_DISPATCHED_MUTATING`, `PERFORMED` (`ends`), `HALTED` (`error`, `evidence`, and `ends` when any mutating command ran), `NOT_REACHED` |

`SNAPSHOT_TAKEN` is `SNAPSHOT_BEFORE` performed: the snapshot was published
durably and read back before the marker was written and before the first
change. It is not a *dispatch* (it writes only this tool's state).
`RECORD_PENDING_READ_BACK` is `RECORD_HISTORY` performed and pending, as above.
`NOT_PERFORMED_IN_THIS_BUILD` stays legal for a bracket on a run that
dispatched no mutation.

`PERFORMED` is a mutating step every command of which exited 0: carried out,
and **not thereby a success** — an exit status is never verification, so the
verification step after it judges. Its `ends` hold one
`{"end": "EXITED", "code": 0}` per command; a record whose `PERFORMED` step has
no end, or an end that is not exit 0, is `HISTORY_UNUSABLE`. A `HALTED` step's
`ends` (left out when empty) are the ends of the mutating commands handed over
before it stopped — `EXITED` with its `code`, or `DID_NOT_FINISH` — and a halt
with any end is dispatched whatever else the record says.

Evidence keeps a read's result, its line count and its truncation flag —
`passed`, `lineCount`, `truncated`, and `cause` when it failed — **never its
output**, so a record cannot carry the machine's inventory (`plan.v1.md`
obligation 3).

## Restorability is derived, not stored

`history` derives it from the record, the snapshot named by `snapshotPath` and
the record's standing, in this order (first match):

1. no mutating step dispatched: `NOTHING_TO_RESTORE`;
2. standing `UNCONFIRMED`: `NOT` (`RECORD_NOT_CONFIRMED`); `UNKNOWN`: `NOT`
   (`STANDING_UNKNOWN`);
3. no `snapshotPath`: `NOT` (`NO_SNAPSHOT_TAKEN`);
4. the snapshot is missing, unreadable or not this run's: `NOT`
   (`SNAPSHOT_MISSING`, `SNAPSHOT_UNREADABLE`, `SNAPSHOT_NOT_THIS_RUNS`);
5. a dispatched mutating step has no known exit-0 end: `PARTIALLY`
   (`OUTCOME_UNCERTAIN`);
6. otherwise `FROM_SNAPSHOT`: a reinstall plan can be built from the snapshot.
   It does not promise that every artifact is still available or that versions
   match; `restore --plan` shows where they differ.

## Sample

An illustrative record of a retain-source migration whose steps were all
carried out, as the mutating pass will write it. (A plan this build builds also
carries `SET_PRIMARY` and `VERIFY_RESOLUTION`, which this build does not perform,
so a record of a real run would be `PARTIAL`; this one is of a plan without
them.) Checked by `the_documented_sample_is_a_record_this_version_reads`
(`crates/adapters/tests/history_store.rs`), which reads it back and
re-serialises it byte for byte.

<!-- history-sample -->
```json
{
  "schemaVersion": 2,
  "id": "20261004T120000Z-3f9c2a7b1d0e4c58",
  "startedAt": "2026-10-04T12:00:00Z",
  "toolVersion": "0.1.0",
  "planSchemaVersion": 1,
  "policyVersion": "2026.10.03-2",
  "package": {
    "provider": "macports",
    "name": "wget"
  },
  "direction": {
    "source": "macports",
    "destination": "homebrew"
  },
  "destinationPackage": "wget",
  "versions": {
    "source": "1.25.0_0",
    "destination": null
  },
  "action": {
    "kind": "MIGRATE_TO",
    "provider": "homebrew"
  },
  "overrode": null,
  "retainSource": true,
  "primaryAfter": "homebrew",
  "environmentChanges": [],
  "steps": [
    {
      "index": 1,
      "kind": "SNAPSHOT_BEFORE",
      "provider": null,
      "dispatched": false,
      "result": {
        "result": "SNAPSHOT_TAKEN"
      }
    },
    {
      "index": 2,
      "kind": "ADD_DESTINATION",
      "provider": "homebrew",
      "dispatched": true,
      "result": {
        "result": "PERFORMED",
        "ends": [
          {
            "end": "EXITED",
            "code": 0
          }
        ]
      }
    },
    {
      "index": 3,
      "kind": "VERIFY_DESTINATION",
      "provider": "homebrew",
      "dispatched": true,
      "result": {
        "result": "VERIFIED",
        "evidence": [
          {
            "passed": true,
            "lineCount": 1,
            "truncated": false
          }
        ]
      }
    },
    {
      "index": 4,
      "kind": "RECORD_HISTORY",
      "provider": null,
      "dispatched": false,
      "result": {
        "result": "RECORD_PENDING_READ_BACK"
      }
    }
  ],
  "ending": {
    "kind": "COMPLETED"
  },
  "status": "SUCCESS",
  "mutationDispatched": true,
  "snapshotPath": "snapshot-20261004T120000Z.json"
}
```
