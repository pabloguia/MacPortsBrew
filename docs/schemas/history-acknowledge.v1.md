<!-- SPDX-License-Identifier: Apache-2.0 -->

# `history acknowledge --json` schema, version 1

What `macportsbrew history acknowledge <id> [--yes] --json` prints on stdout
(`04-migration-and-safety.md`, Concurrency and locking; pass 08 slice 5, D13).
Built in `crates/cli/src/app/interrupted.rs`. The compatibility rule is in
[`README.md`](README.md).

It is refused, with an empty stdout, under `--replay` and `--from-snapshot`
(`SAFETY_BLOCKED`, exit 4), as root, and for a value that is not a run id (usage,
exit 2).

## Shape

```json
{
  "schemaVersion": 1,
  "runId": "20261003T120000Z-00000000000000aa",
  "outcome": "WOULD_ACKNOWLEDGE",
  "record": "UNCONFIRMED",
  "snapshotFile": "snapshot-20261003T120000Z.json"
}
```

| Field | Meaning |
|---|---|
| `outcome` | `NOTHING_TO_ACKNOWLEDGE` (no marker; nothing is created; exit 0), `WOULD_ACKNOWLEDGE` (without `--yes`, or with `--dry-run`: nothing was changed), or `ACKNOWLEDGED` (the marker was removed) |
| `record` | The state of the run's record: `NONE` (the run left none), `UNCONFIRMED` (it exists, reads, and the marker still names it) or `UNREADABLE`. Absent with `NOTHING_TO_ACKNOWLEDGE` |
| `snapshotFile` | The snapshot file the marker names, when it names one |

## What `--yes` does, in order

1. The marker is looked at without the lock; another run's marker is refused
   (`SAFETY_BLOCKED`, exit 4, naming both runs).
2. The run lock is taken (held by a run in progress: `RUN_LOCK_HELD`, exit 7).
3. The marker is read again under the lock.
4. If the run has a record, it is **made durable and read back** (file and
   directory synced, read again, equal). A record that cannot be read is
   `HISTORY_UNUSABLE` (exit 8) and **the marker stays**: a record is never
   promoted to confirmed because it could not be examined.
5. The marker is removed. No record and no snapshot is deleted.
