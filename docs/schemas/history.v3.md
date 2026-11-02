# `history` record schema, version 3

The file a mutating run leaves in the state directory, `history/run-<id>.json`
(`04-migration-and-safety.md`, History). Derived from `RunRecord` in
`crates/domain/src/history.rs`; written and read by
`crates/adapters/src/persistence/history_store.rs`. The compatibility rule is
in [`README.md`](README.md). **Version 3 is what this build writes**; a
[version 2](history.v2.md) record still reads (below). Shipped with work order
16 part F-3 (`docs/passes/16-plans/F.md`, Design §5; SG-12).

**This build writes none to a user's machine.** A record is written iff the run
dispatched a mutating step (`mutationDispatched`), and the gate refuses every
plan with one while `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`.

Everything in version 2 holds unless this document says otherwise: how a record
is written and read, its standing, restorability, and the `Step` and root fields
it does not list here.

## What changed from version 2, and why

- **`primaryAfter` is what was observed to resolve**, not what the plan asked
  for: the destination only when the run's `VERIFY_RESOLUTION` ended `RESOLVED`
  (every executable resolved to the destination package by the provider's own
  records), and otherwise the source. Version 2's `primaryAfter` was the
  request, which no step had checked.
- **`primaryExecutables`** (array of executable names, sorted, no duplicates):
  the executables the plan's primary decision was about — the source package's
  list. Empty for `NO_EXECUTABLES` or a refusal about the package as a whole.
  Required in version 3; a version 3 record without it is `HISTORY_UNUSABLE`, so
  is one whose list is unsorted or repeats a name, and so is a name outside the
  executable-name class.
- **Two step results**: `NOTHING_TO_CHANGE`, only on `SET_PRIMARY` (a
  command-less step whose decision left nothing to set; carried out, not
  dispatched), and `RESOLVED` (`executables`), only on `VERIFY_RESOLUTION`
  (carried out, dispatched: the resolution site asked the providers). Either on
  another step is `HISTORY_UNUSABLE`.
- Why a new version: recognition reads `primaryExecutables`, and a reader of
  version 2 would refuse the new results and field (it denies unknown fields),
  so the change is not additive.

## Reading a version 2 record

A version 2 record still reads, with no `primaryExecutables` (a version 2
record that states one is `HISTORY_UNUSABLE`, as is one stating either new
result). **Recognition never answers "recognised" for it** — it answers
`PRIMARY_NOT_CONFIRMED` (`invariant_16_a_v2_record_is_never_recognised`):
version 2 never established which executables its primary was about.

## Recognition

A `--retain-source` re-run is recognised as already done only when every
`primaryExecutables` entry, resolved in the fresh snapshot through
`resolution_of`, is the destination package's by the provider's own records. A
fresh snapshot whose source list is not `KNOWN` answers `EXECUTABLES_UNKNOWN`,
never recognised. Restore has nothing to revert for the primary: no build sets
one.

## Root (the fields that differ from version 2)

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `3` |
| `primaryAfter` | string | The provider observed to resolve the executables: the destination only after `RESOLVED` |
| `primaryExecutables` | array of string | Sorted, unique, each an executable name |

## `Step.result` (the values added to version 2's)

| `result` | Fields | On |
|---|---|---|
| `NOTHING_TO_CHANGE` | — | `SET_PRIMARY` only |
| `RESOLVED` | `executables`: array of string | `VERIFY_RESOLUTION` only |

## Sample

An illustrative record of a retain-source migration whose steps were all
carried out, as the mutating pass will write it: its primary decision was
`NOTHING_TO_CHANGE` and `VERIFY_RESOLUTION` resolved `wget` to the destination
package. Checked by `the_documented_sample_is_a_record_this_version_reads`
(`crates/adapters/tests/history_store.rs`), which reads it back and
re-serialises it byte for byte.

<!-- history-sample -->
```json
{
  "schemaVersion": 3,
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
  "primaryExecutables": [
    "wget"
  ],
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
      "kind": "SET_PRIMARY",
      "provider": "homebrew",
      "dispatched": false,
      "result": {
        "result": "NOTHING_TO_CHANGE"
      }
    },
    {
      "index": 5,
      "kind": "VERIFY_RESOLUTION",
      "provider": null,
      "dispatched": true,
      "result": {
        "result": "RESOLVED",
        "executables": [
          "wget"
        ]
      }
    },
    {
      "index": 6,
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
