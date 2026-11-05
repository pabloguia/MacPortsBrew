# `history` record schema, version 4

The file a mutating run leaves in the state directory, `history/run-<id>.json`
(`04-migration-and-safety.md`, History). Derived from `RunRecord` in
`crates/domain/src/history.rs`; written and read by
`crates/adapters/src/persistence/history_store.rs`. The compatibility rule is
in [`README.md`](README.md). **Version 4 is what this build writes**; a
[version 3](history.v3.md) or [version 2](history.v2.md) record still reads (below). Introduced by
work order 16 part H-1, 2026-11-04 (SE-113); nothing has been released under it.

**This build writes none to a user's machine.** A record is written iff the run
dispatched a mutating step (`mutationDispatched`), and the gate refuses every
plan with one while `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`.

Everything in version 2 holds unless this document says otherwise: how a record
is written and read, its standing, restorability, and the `Step` and root fields
it does not list here.

## What changed from version 3, and why

Work order 16 part H-1. A check on `VERIFY_DESTINATION` now answers `HOLDS`, `FAILS` or `UNCHECKED`, and a run can end with something unchecked and the source kept. `StoredEnding` and the step results refuse unknown keys and values, so a new value is not additive, and this is a new version.

- `Step.result` gains `VERIFIED_WITH_UNCHECKED` (`evidence`, `unchecked`: on `VERIFY_DESTINATION` only, `unchecked` never empty, every read passed) and `SOURCE_KEPT_UNCHECKED` (on `REMOVE_SOURCE`, `VERIFY_REMOVAL` or `CLEANUP_REVIEW` only, after a `VERIFIED_WITH_UNCHECKED`; not dispatched).
- `ending` gains `{ "kind": "COMPLETED_WITH_UNCHECKED", "sourceKept": boolean }`, read only when the steps agree: something unchecked, no step left undone, and `sourceKept` true exactly when a `SOURCE_KEPT_UNCHECKED` step exists. The other way too: a record with something unchecked that ends `COMPLETED`, or that states a removal performed after it, or a `VERIFY_REMOVAL` or `CLEANUP_REVIEW` kept without a kept `REMOVE_SOURCE`, is refused (`HISTORY_UNUSABLE`).
- `status` for that ending is `PARTIAL` when a mutation was dispatched and `NO_CHANGE` otherwise, **never `SUCCESS`** (`04-migration-and-safety.md`). So a re-run re-verifies an unchecked destination rather than recognising it as done (`recognise_retained_run` still refuses anything but `SUCCESS`, INV-16).
- `UncheckedEntry` is as in [`migrate.v2.md`](migrate.v2.md).

A version 3 record reads as before, and one stating anything above is refused (`HISTORY_UNUSABLE`). Part H-2's unchecked causes are in [`migrate.v2.md`](migrate.v2.md)'s list, and belong to this version: nothing has been released under it. A version 2 record still reads as [`history.v3.md`](history.v3.md) says.

**What did not change** is everything else: the rest of this document is version 3's. Its own history of how version 3 differed from version 2 follows.

## What changed from version 2, and why

- **`primaryAfter` is what was observed to resolve**, not what the plan asked
  for: the destination when the run's `VERIFY_RESOLUTION` ended `RESOLVED`
  (every executable resolved to the destination package by the provider's own
  records), or when the package installs no executable (`NO_EXECUTABLES`: an
  empty `primaryExecutables` and its `SET_PRIMARY` carried out as
  `NOTHING_TO_CHANGE`), and otherwise the source. A version 3 record naming the
  destination without one of the two is `HISTORY_UNUSABLE`. Version 2's
  `primaryAfter` was the request, which no step had checked.
- **`primaryExecutables`** (array of executable names, sorted, no duplicates):
  the executables the plan's primary decision was about — the source package's
  list. Empty for `NO_EXECUTABLES` or a refusal about the package as a whole.
  Required in version 3; a version 3 record without it is `HISTORY_UNUSABLE`, so
  is one whose list is unsorted or repeats a name, and so is a name outside the
  executable-name class.
- **Two step results**: `NOTHING_TO_CHANGE`, only on `SET_PRIMARY` (a
  command-less step whose decision left nothing to set; carried out, not
  dispatched), and `RESOLVED` (`executables`), only on `VERIFY_RESOLUTION`
  (carried out, dispatched: the resolution site asked the providers), naming
  exactly `primaryExecutables`. Either on another step, or a `RESOLVED` naming
  other executables, is `HISTORY_UNUSABLE`.
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

A `--retain-source` re-run is recognised as already done only when all of these
hold, besides version 2's conditions:

- the fresh snapshot's source list is `KNOWN` (else `EXECUTABLES_UNKNOWN`) and
  equals `primaryExecutables` exactly (else `PRIMARY_NOT_CONFIRMED`: a source
  that now ships another executable has one no run resolved);
- the record's `primaryAfter` is the destination (else `PRIMARY_NOT_CONFIRMED`);
- every `primaryExecutables` entry, resolved in the fresh snapshot through
  `resolution_of`, is the destination package's, and the resolution is
  authoritative — by the provider's own records, not inferred from the path
  (else `PRIMARY_NOT_CONFIRMED`); one that resolves to MacPortsBrew's shim
  answers `PRIMARY_IS_SHIM`.

Restore has nothing to revert for the primary: this build sets none.

## Root (the fields that differ from version 2)

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `4` |
| `primaryAfter` | string | The provider observed to resolve the executables: the destination only after `RESOLVED`, or for `NO_EXECUTABLES` after its `SET_PRIMARY` was carried out |
| `primaryExecutables` | array of string | Sorted, unique, each an executable name |
| `checkBases` | array of string, optional | Work order 16 part H-2. What a destination check that held rests on, as `migrate.v2` defines the values, each at most once, in that document's order. Absent when neither integrity nor linkage held. It may accompany a halted step, since a check that held before another failed still held. A record with it under another `schemaVersion`, out of order, or with no `VERIFY_DESTINATION` step that is verified, verified with unchecked, or halted is unusable |

## `Step.result` (the values added to version 2's)

| `result` | Fields | On |
|---|---|---|
| `NOTHING_TO_CHANGE` | — | `SET_PRIMARY` only |
| `RESOLVED` | `executables`: array of string | `VERIFY_RESOLUTION` only |
| `VERIFIED_WITH_UNCHECKED` | `evidence`, `unchecked` | `VERIFY_DESTINATION` only; version 4 |
| `SOURCE_KEPT_UNCHECKED` | — | `REMOVE_SOURCE`, `VERIFY_REMOVAL`, `CLEANUP_REVIEW`; version 4 |

## Sample

An illustrative record of a retain-source migration whose steps were all
carried out, as the mutating pass will write it once all four checks of DP-051 are observed to hold, which this build does not do: its primary decision was
`NOTHING_TO_CHANGE` and `VERIFY_RESOLUTION` resolved `wget` to the destination
package. Checked by `the_documented_sample_is_a_record_this_version_reads`
(`crates/adapters/tests/history_store.rs`), which reads it back and
re-serialises it byte for byte.

<!-- history-sample -->
```json
{
  "schemaVersion": 4,
  "id": "20261004T120000Z-3f9c2a7b1d0e4c58",
  "startedAt": "2026-10-04T12:00:00Z",
  "toolVersion": "0.1.0",
  "planSchemaVersion": 2,
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
