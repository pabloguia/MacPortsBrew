<!-- SPDX-License-Identifier: Apache-2.0 -->

# `history --json` schema, version 1

What `macportsbrew history --json` prints on stdout: the newest runs this
MacPortsBrew has made on this machine, newest first (`06-cli-and-ux.md`;
`04-migration-and-safety.md`, History). Built in
`crates/cli/src/app/history.rs`. The compatibility rule is in
[`README.md`](README.md).

**This command reads and creates nothing.** It is refused under `--replay` and
`--from-snapshot` (exit 4) and when run as root. It carries no
`concurrentProviderActivity` flag, because it asks no provider anything.

## Shape

```json
{
  "schemaVersion": 1,
  "total": 3,
  "limit": 2,
  "runs": [
    {
      "runId": "20261003T120000Z-00000000000000aa",
      "startedAt": "2026-10-03T12:00:00Z",
      "package": "htop",
      "source": "homebrew",
      "destination": "macports",
      "status": "SUCCESS",
      "restorable": "FROM_SNAPSHOT",
      "standing": "CONFIRMED",
      "snapshotFile": "snapshot-20261003T120000Z.json",
      "snapshot": "PRESENT",
      "snapshotChecked": "METADATA"
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `total` | How many records the store holds, including those not listed |
| `limit` | The page size asked for (`--limit`, 1 to 200, default 20) |
| `runs` | At most `limit` runs, newest first by the stamp in the id. Two runs started in the same second are in a fixed order that is not chronological |
| `status` | `SUCCESS`, `PARTIAL`, `FAILURE` or `NO_CHANGE`, as `history.v2` states it |
| `restorable` | `NOTHING_TO_RESTORE`, `FROM_SNAPSHOT`, `PARTIALLY` or `NOT`; the last two carry a `reason` (below) beside it. **Derived when the record is read**, never stored |
| `standing` | `CONFIRMED` (no interrupted-run marker names the record), `UNCONFIRMED` (the marker names it: published, not known to be durable) or `UNKNOWN` (the marker could not be read) |
| `snapshotFile` | The snapshot file name the record names, or `null` when none was taken |
| `snapshot` | `NOT_TAKEN`, `PRESENT`, `MISSING` or `UNREADABLE` |
| `snapshotChecked` | Always `METADATA` here: `PRESENT` means a regular file this user owns, closed to others, within the size cap. It was not read; [`history show`](history-show.v1.md) reads it and ties it to the run |

`reason`, when present: `NO_SNAPSHOT_TAKEN`, `SNAPSHOT_MISSING`,
`SNAPSHOT_UNREADABLE`, `SNAPSHOT_NOT_THIS_RUNS`, `RECORD_NOT_CONFIRMED`,
`STANDING_UNKNOWN` or `OUTCOME_UNCERTAIN`. Consumers should accept a value they
do not know (additive values do not bump the version).

## What is never in it

A record that cannot be read is not skipped and not summarised: the command
fails with `HISTORY_UNUSABLE` (exit 8) naming the file. A store that does not
exist is `"total": 0` and no runs, and nothing is created by asking.
