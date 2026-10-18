<!-- SPDX-License-Identifier: Apache-2.0 -->

# `history show <id> --json` schema, version 1

What `macportsbrew history show <id> --json` prints on stdout: one run's record
exactly as [`history.v2`](history.v2.md) stores it, with what is derived about it
when it is read. Built in `crates/cli/src/app/history.rs`. The compatibility
rule is in [`README.md`](README.md). The same refusals as
[`history-list.v1`](history-list.v1.md) apply, and no flag about provider
activity is carried.

## Shape

```json
{
  "schemaVersion": 1,
  "record": { "schemaVersion": 2, "id": "20261003T120000Z-00000000000000aa", "...": "history.v2" },
  "restorability": { "restorable": "FROM_SNAPSHOT" },
  "standing": "CONFIRMED",
  "snapshot": "PRESENT",
  "snapshotChecked": "PARSED"
}
```

| Field | Meaning |
|---|---|
| `record` | The `history.v2` document, unchanged. It states no `restorability` |
| `restorability` | `{ "restorable": "NOTHING_TO_RESTORE" \| "FROM_SNAPSHOT" }`, or `{ "restorable": "PARTIALLY" \| "NOT", "reason": ... }` with the reasons listed in [`history-list.v1`](history-list.v1.md). Derived, in order, from the record, the snapshot and the standing: nothing dispatched; record not confirmed; no snapshot named; snapshot missing, unreadable or not this run's; a dispatched command without a known exit 0 |
| `standing` | As in the list |
| `snapshot` | `NOT_TAKEN`, `PRESENT`, `MISSING` or `UNREADABLE` |
| `snapshotChecked` | `PARSED`: the snapshot was opened without following a symbolic reference, checked (owner, mode, size), parsed, and its `capturedAt` compared with the record's `startedAt`. A snapshot that is another run's is `PRESENT` and `restorability` says `SNAPSHOT_NOT_THIS_RUNS` |

`FROM_SNAPSHOT` is not a promise that every artifact is still available or that
versions match: a plan built from the snapshot shows where they differ
(`04-migration-and-safety.md`, "Undoing a migration").

## Errors

A value that is not shaped like a run id is a usage error (exit 2) and is never
used as a path. A well-formed id with no record is `RUN_NOT_FOUND` (exit 8). A
record that cannot be read is `HISTORY_UNUSABLE` (exit 8).
