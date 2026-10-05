# `interrupted-run` marker schema, version 1

The file a mutating run leaves in the state root, `interrupted-run.json`, from
just before its first provider change until its record is published
(`04-migration-and-safety.md`, Concurrency and locking). Derived from
`StoredInterruptedRun` in `crates/domain/src/interrupted_run.rs`; written, read
and cleared by `crates/adapters/src/persistence/run_marker.rs`. The
compatibility rule is in [`README.md`](README.md).

**This build writes none.** `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`, so no
run reaches the point where a marker is written; the format and the store are
fixed and tested now so the pass that turns mutations on writes this shape.

## Shape

Exactly this text, compact, with one trailing newline:

```json
{"schemaVersion":1,"runId":"20261005T120000Z-00000000000000aa"}
```

| Field | Type | Meaning |
|---|---|---|
| `schemaVersion` | integer | Always `1`. |
| `runId` | string | The run's identifier, `<compact UTC stamp>-<16 lower-case hex digits>` — the grammar of `history.v1.md`'s `id`. |

Nothing else is recorded: no path, package or time that could be read back as
authority.

## How it is written, read and cleared

- **Written under the single-instance lock** (`run.lock`, in the same
  directory), after the lock is held and the history store has proved it can
  publish, immediately before the first step that can change a provider. It is
  published by a hard link that cannot replace an existing file, and its
  directory entry must reach the medium before the call returns, so a power loss
  after the first change cannot lose it.
- **Cleared by the run that wrote it**, on every ending that returns control: after
  its record is published when it dispatched anything, at once when it
  dispatched nothing. A run whose record could not be published keeps it. Only
  its own marker is removed; another run's is left in place. There is no other
  deletion by this tool.
- **Read without being trusted.** A missing marker is "no earlier run" and
  reading creates nothing. Anything present that is not exactly the text above
  — another schema, an unknown or repeated field, a byte order mark, trailing
  data, an id outside the grammar, a symbolic reference, a directory, a file
  larger than 4 KiB, owned by another user or open to group or other access —
  is `HISTORY_UNUSABLE` (exit 8), never "no marker". The link count is not
  checked: a kill between the link and its removal leaves it at two.
- **A marker found by a mutating command refuses it**: `RUN_INTERRUPTED`,
  exit **4**, naming the run. Analysis, plan and dry runs do not read it and are
  unaffected. A lock held by another run is `RUN_LOCK_HELD`, exit **7**, and
  is reported before the marker is looked at.
- **Never as root**, and never under `--replay` or `--from-snapshot` (INV-15):
  the lock refuses euid 0, and every marker function takes the lock.

## Clearing a stranded marker by hand

Until `history` and `restore` exist (pass 08, item 7), the remedy is the user's
own act: check whether the history holds a record for the named run; if not,
look at what that run changed; then, **only when no other MacPortsBrew run is
active**, remove the single file `interrupted-run.json` from the state directory
(`MACPORTSBREW_STATE_DIR`, or `Application Support/macportsbrew` under the
user's Library). The remedy text names the file and the directory separately,
because the default path contains a space.
