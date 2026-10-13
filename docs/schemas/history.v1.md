# `history` record schema, version 1

The file a mutating run leaves in the state directory, `history/run-<id>.json`
(`04-migration-and-safety.md`, History). Derived from `RunRecord` in
`crates/domain/src/history.rs`; written and read by
`crates/adapters/src/persistence/history_store.rs`. The compatibility rule is
in [`README.md`](README.md).

**This build writes none.** A record is written iff the run dispatched a
mutating step (`mutationDispatched`), and this build dispatches none; the
format is fixed and tested now so the pass that turns mutations on writes
this shape. The store landed in pass 08, slice 1.

## How it is written and read

- **One file per run, never replaced.** Published by a hard link that cannot
  replace an existing file; nothing deletes a record.
- **Read back without being trusted.** `RunRecord::from_stored` derives the
  steps' `dispatched`, `status`, `mutationDispatched` and `restorability` again
  and refuses a file that states anything else (INV-03). The one exception is
  a `HALTED` step with no evidence, whose `dispatched` is the record's own
  statement: nothing else in the record can settle it.
- **Anything this writer would not have left is `HISTORY_UNUSABLE`** (exit 8),
  never "no record": another `schemaVersion`, a field this version does not
  know (at any level), an `id` outside the grammar below or naming another
  instant than `startedAt`, a Control-class character in any string, a
  `snapshotPath` that is not a snapshot file name, evidence whose `passed`
  disagrees with its `cause`, a `VERIFIED` step with failed evidence, a
  `MANUAL_REVIEW` step stated as carried out, a record under another run's
  file name, a file that is a symbolic reference, larger than 256 KiB, owned
  by another user or open to group or other access, a directory entry in
  `history/` that is not a record or whose name is not text, a `history/` or
  state directory that is a symbolic reference, not a directory, another
  user's or open to others, and more than 10,000 entries in `history/` —
  hidden ones included, since nothing prunes the store yet. Dot-entries are
  the writer's temporaries and are otherwise ignored.
- **Never read or written as root** (`SAFETY_BLOCKED`, exit 4), and never
  under `--replay` or `--from-snapshot`. A write that meets an existing record
  of the same id is refused (`IO`) rather than given another id: the id is
  fixed before the run's first step.

## Root

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `1` |
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
| `status` | string | Derived: `SUCCESS`, `PARTIAL`, `FAILURE`, `NO_CHANGE` |
| `mutationDispatched` | boolean | Derived |
| `snapshotPath` | string \| null | The snapshot's **file name** only, `snapshot-<1–32 ASCII alphanumerics>.json`, inside the same state directory's `snapshots/`. A run is restorable only from the state directory it was recorded in |
| `restorability` | object | Derived. `restorable`: `NOTHING_TO_RESTORE`, `FULLY`, `PARTIALLY` (`reason`), `NOT` (`reason`) |

## `Step`

| Field | Type | Notes |
|---|---|---|
| `index` | integer | As the plan numbers it |
| `kind` | string | A `StepKind` from [`plan.v1.md`](plan.v1.md) |
| `provider` | string \| null | |
| `dispatched` | boolean | Derived from `result`, except for a halt with no evidence |
| `result` | object | `result`: `VERIFIED` (`evidence`), `NOT_PERFORMED_IN_THIS_BUILD`, `NOT_DISPATCHED_MUTATING`, `HALTED` (`error`, `evidence`), `NOT_REACHED` |

Evidence keeps a read's result, its line count and its truncation flag —
`passed`, `lineCount`, `truncated`, and `cause` when it failed — **never its
output**, so a record cannot carry the machine's inventory (`plan.v1.md`
obligation 3).

## Sample

An illustrative record of a retain-source migration that completed, as the
mutating pass will write it — no build writes one yet, and this build records
`SNAPSHOT_BEFORE` and `RECORD_HISTORY` as `NOT_PERFORMED_IN_THIS_BUILD`.
**`RECORD_HISTORY`'s representation is provisional**: the plan has the record
never certify its own write, which a `VERIFIED` `RECORD_HISTORY` inside the
record does; settling it (a pending result judged on read-back, or a revised
v1 before any build writes records) is owed with the write call (SE-092). Checked by
`the_documented_sample_is_a_record_this_version_reads`
(`crates/adapters/tests/history_store.rs`), which reads it back and
re-serialises it byte for byte.

<!-- history-sample -->
```json
{
  "schemaVersion": 1,
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
      "dispatched": true,
      "result": {
        "result": "VERIFIED",
        "evidence": []
      }
    },
    {
      "index": 2,
      "kind": "ADD_DESTINATION",
      "provider": "homebrew",
      "dispatched": true,
      "result": {
        "result": "VERIFIED",
        "evidence": []
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
      "kind": "SET_PRIMARY",
      "provider": null,
      "dispatched": true,
      "result": {
        "result": "VERIFIED",
        "evidence": []
      }
    },
    {
      "index": 5,
      "kind": "VERIFY_RESOLUTION",
      "provider": null,
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
      "index": 6,
      "kind": "RECORD_HISTORY",
      "provider": null,
      "dispatched": true,
      "result": {
        "result": "VERIFIED",
        "evidence": []
      }
    }
  ],
  "ending": {
    "kind": "COMPLETED"
  },
  "status": "SUCCESS",
  "mutationDispatched": true,
  "snapshotPath": "snapshot-20261004T120000Z.json",
  "restorability": {
    "restorable": "NOT",
    "reason": "RESTORE_NOT_IN_THIS_BUILD"
  }
}
```
