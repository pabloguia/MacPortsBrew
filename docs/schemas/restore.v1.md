<!-- SPDX-License-Identifier: Apache-2.0 -->

# `restore --json` schema, version 1

What `macportsbrew restore --run <id> [--plan] --json` prints on stdout: the plan
that puts back what one recorded run changed, with what the plan itself does not
say (`04-migration-and-safety.md`, "Undoing a migration"). Built in
`crates/cli/src/app/restore.rs`. The compatibility rule is in
[`README.md`](README.md).

**A restore plan is an ordinary [`plan.v1`](plan.v1.md) `MigrationPlan`** - the
same planner, in the other direction (INV-17), and the same execution gate. The
document wraps it and never changes it: notes live beside the plan, not in it.
**Nothing is executed in this build.** `--plan` shows the plan; without it the
plan is put to the same gate `migrate --yes` reaches, which refuses it (exit 4).

Refused before anything is read, with an empty stdout: under `--replay` and
`--from-snapshot` (`SAFETY_BLOCKED`, exit 4; history is this machine's own, now),
as root, for a value that is not a run id (usage, exit 2), and for
`--snapshot <file>` (`NOT_IMPLEMENTED_IN_THIS_PASS`, exit 1).

## Shape

```json
{
  "schemaVersion": 1,
  "mode": "RENDER",
  "restoreOf": {
    "runId": "20261004T120000Z-00000000000000a1",
    "snapshotFile": "snapshot-20261004T120000Z.json",
    "standing": "CONFIRMED",
    "restorable": "FROM_SNAPSHOT"
  },
  "notes": [
    { "note": "NOT_VERSION_EXACT", "package": "htop", "recordedVersion": "3.4.1" }
  ],
  "plan": { "schemaVersion": 1, "...": "plan.v1" },
  "concurrentProviderActivity": { "...": "plan.v1, obligation 5" }
}
```

| Field | Meaning |
|---|---|
| `mode` | `RENDER` with `--plan`; `EXECUTE` without it |
| `restoreOf` | Which run, the snapshot it took, the record's `standing` and the derived `restorable` (with a `reason` for `PARTIALLY` and `NOT`), as [`history-show.v1`](history-show.v1.md) states them. For a record the interrupted-run marker still names this says `UNCONFIRMED` and `NOT` with `RECORD_NOT_CONFIRMED`, and the plan is **still shown** so it can be reviewed before the marker is dealt with |
| `notes` | What a person reviewing the plan should know. `NOT_VERSION_EXACT` always (`package`, and `recordedVersion` when the record states one); `KEEPS_DESTINATION` (`reason`: `ADD_NOT_PERFORMED`, `DESTINATION_HELD_IT_BEFORE`, `BEFORE_STATE_NOT_ESTABLISHED`) when the package the run added is not removed; `RECORD_NOT_CONFIRMED`; `OUTCOME_UNCERTAIN` when a dispatched command did not end in a known exit 0 |
| `nothingToRestore` | Present instead of `plan` when there is nothing to put back: `RUN_CHANGED_NOTHING` (the run dispatched no mutating step) or `ALREADY_RESTORED` (the machine is as the run found it). Exit 0 (INV-16) |
| `plan` | The `plan.v1` plan, unchanged. Absent with `nothingToRestore` |
| `warnings` | As in [`plan.v1.md`](plan.v1.md): `["advanced.warning"]` when the plan's `enteredBy` is `ADVANCED` or `NAMED`; omitted otherwise. Since 2026-10-27, additive (DP-037) |
| `execution` | With `EXECUTE` only: `outcome`, and the `gateRefusals` that ended it, exactly as [`migrate.v1`](migrate.v1.md) states them |

New values may be added to `notes` and `reason` without a version bump
(additive); a consumer should accept one it does not know.

## What the plan removes

The package the run added (under the run's destination provider) is removed only
if the record shows the add dispatched and ended in a known exit 0 **and** the
snapshot taken before the first change shows the destination did not already
hold it. Anything less keeps it, with a `KEEPS_DESTINATION` note: removing a
package that predates the run, or one the run never added, would be the worst an
undo could do. A run made with `--retain-source` is the cheap case: nothing is
reinstalled and the plan verifies the retained package before removing what the
run added (INV-01).

## Exit codes

`0` a plan or nothing to restore; `4` the plan stops at `MANUAL_REVIEW` (as
`plan` does), the gate refused it, the record is not confirmed and execution was
asked for, or the planner built a plan for a different package than the run
moved (never a silent substitute); `8` the run is not recorded (`RUN_NOT_FOUND`),
its record or the interrupted-run marker cannot be read (`HISTORY_UNUSABLE`), or
its snapshot is missing, unreadable or not the run's (`SNAPSHOT_UNUSABLE`).

## Known limit

The plan is built by the same planner as `migrate`, so a mapping needs the same
evidence. When the run removed the package from its source, nothing installed
there remains to show the mapping, and unless the curated table covers the
package the plan stops at `MANUAL_REVIEW` (`reason.mapping_confidence_below_threshold`),
exit 4. Declared in `04-migration-and-safety.md` and SE-092.
