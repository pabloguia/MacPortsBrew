# `interrupted-run` marker schema, version 2

The file a mutating run leaves in the state root, `interrupted-run.json`, from
just before its first provider change until its record is published
(`04-migration-and-safety.md`, Concurrency and locking). Derived from
`StoredInterruptedRun` in `crates/domain/src/interrupted_run.rs`; written, read
and cleared by `crates/adapters/src/persistence/run_marker.rs`. The
compatibility rule is in [`README.md`](README.md). **Version 2 supersedes
[version 1](run-marker.v1.md), which no build ever wrote** (pass 08 slice 5):
the marker now names the snapshot the run took first.

**This build writes none.** `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`, so no
run reaches the point where a marker is written; the format and the store are
fixed and tested now so the pass that turns mutations on writes this shape.

## Shape

Exactly this text, compact, with one trailing newline:

```json
{"schemaVersion":2,"runId":"20261005T120000Z-00000000000000aa","snapshotFile":"snapshot-20261005T120000Z.json"}
```

| Field | Type | Meaning |
|---|---|---|
| `schemaVersion` | integer | Always `2`. |
| `runId` | string | The run's identifier, `<compact UTC stamp>-<16 lower-case hex digits>` — the grammar of `history.v2.md`'s `id`. |
| `snapshotFile` | string \| null | The **file name** (never a path) of the snapshot the run took first, `snapshot-<1–32 ASCII alphanumerics>.json`, inside the same state directory's `snapshots/`. `null` only for a marker built from an identifier alone. |

The marker is written after the snapshot step (`SNAPSHOT_BEFORE`) and before the
first provider change, so it can name the one trace a run that never recorded
itself leaves. Nothing else is recorded: no other path, package or time that
could be read back as authority.

## How it is written, read and cleared

- **Written under the single-instance lock** (`run.lock`, in the same
  directory), after the lock is held and the history store has proved it can
  publish, immediately before the first step that can change a provider. It is
  published by a hard link that cannot replace an existing file, and its
  directory entry must reach the medium before the call returns, so a power loss
  after the first change cannot lose it.
- **Cleared by the run that wrote it**, on every ending that returns control: after
  its record is published when it dispatched anything, at once when it
  dispatched nothing. A run whose record could not be published keeps it. A run whose marker could not be made durable (its directory
  sync failed) changed nothing, so it takes the marker back before reporting
  `SAFETY_BLOCKED`, and says so only if that worked. Only
  its own marker is removed; another run's is left in place. There is no other
  deletion by this tool.
- **Read without being trusted.** A missing marker is "no earlier run" and
  reading creates nothing. Anything present that is not exactly the text above
  — another schema (version 1 included), an unknown or repeated field, a `snapshotFile` that is not a snapshot file name, a byte order mark, trailing
  data, an id outside the grammar, a symbolic reference, a directory, a file
  larger than 4 KiB, owned by another user or open to group or other access —
  is `HISTORY_UNUSABLE` (exit 8), never "no marker". The link count is not
  checked: a kill between the link and its removal leaves it at two.
- **A marker found by a mutating command refuses it**: `RUN_INTERRUPTED`,
  exit **4**, naming the run. Analysis, plan and dry runs do not read it and are
  unaffected. A lock held by another run is `RUN_LOCK_HELD`, exit **7**, and
  is reported before the marker is looked at.
- **Never as root**, and never under `--replay` or `--from-snapshot` (INV-15):
  the lock refuses euid 0, and every marker function that writes or deletes
  takes the lock. `read` takes it too. **`peek` is the one read without the
  lock**, for `history` and `doctor` (it creates nothing, and requires the state
  root to be private to the user); because it takes none, what it finds is a
  fact about a moment, and may be the marker of a run still in progress.

## Dealing with a stranded marker

`macportsbrew doctor` reports a marker nobody holds the lock for (the
`interrupted_run` finding, [`doctor.v1`](doctor.v1.md)), and says which of three
things it found; `history show <id>` and `history acknowledge <id>` say the same
words:

- **Recorded, not confirmed.** The run's record exists and reads, and the marker
  still names it: whether the record reached the disk is not confirmed. Nothing
  is deleted.
- **Recorded, unreadable.** Something is at the record's id that cannot be read
  as a record. The marker stays and `acknowledge` refuses.
- **Started, not recorded.** The run began and left no record. The provider may
  be half-changed; the marker and the snapshot it names are the only trace.

`macportsbrew history acknowledge <id> --yes` is the one deliberate way to
remove a marker other than the run's own `clear`
([`history-acknowledge.v1`](history-acknowledge.v1.md)). It takes the run lock
(held: exit 7), refuses a marker that names another run (exit 4), makes the
run's record durable and reads it back **before** it removes the marker, and
leaves the marker in place for a record it cannot read. It deletes no record and
no snapshot. Until it is run, every command that changes packages refuses with
exit 4 naming the run.
