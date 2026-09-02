# Undoing a Migration

**In this build there is nothing to undo.** Version 0.1.0 executes nothing, so
no migration has happened. `macportsbrew snapshot` works today and is worth
using; everything else on this page describes the restore path as designed, and
says which parts are not yet written.

## Snapshots work now

```sh
macportsbrew snapshot --out before.json
```

captures both providers' complete state: what is installed, at which versions,
with which ownership and metadata, plus your machine facts and the relevant
parts of your environment. It is a plain JSON file — see
[`../docs/schemas/snapshot.v1.md`](../docs/schemas/snapshot.v1.md) — and you can
read it, keep it, and diff two of them.

You can then analyse it instead of your machine:

```sh
macportsbrew analyze --from-snapshot before.json
```

**The analysis is computed as of the snapshot's capture date**, not today, so a
support deadline is weighed the way it was weighed when the snapshot was taken.
This is the same code path a live analysis runs — capture produces a snapshot,
and analysis is a pure function of one — which is why the same snapshot always
produces the same answer.

Taking a snapshot before you change anything by hand is the single most useful
thing you can do with this build.

Snapshots are redacted the same way everything else is: no usernames, no home
paths, no environment variable values.

## What the restore path will be

Every mutating run will begin by taking a snapshot, and record it against the
run. Then:

```sh
macportsbrew history                     # find the run
macportsbrew history show <id>           # what it did, what completed
macportsbrew restore --run <id> --plan   # a reviewable plan back
```

The undo path is **the ordinary path**. `restore --plan` produces a migration
plan like any other: ordered steps, every command with its full argv, the
risks, the elevation, and a human review before anything happens. There is no
separate "rollback engine" with its own semantics to learn and its own bugs to
find.

`restore --plan --snapshot <file>` does the same thing against a snapshot file
rather than a recorded run.

A run that dies mid-flight leaves a recoverable record, and the next mutating
run refuses to start until you have looked at it — exiting with the
safety-check code and naming the run. Recovery is the same two commands:
`history show <id>` to see what completed, then either restore or a fresh plan
forward. There is no `resume` command, because there is nothing separate to
resume.

## What a restore can and cannot promise

**Can:** reinstall packages that were removed, remove packages that were added,
and put the primary-provider selection back — all as a reviewed plan, in an
order that never removes before it has installed and verified.

**Cannot:** bring back data that was outside a package's prefix. This is
exactly why packages that own persistent data are never migrated automatically
in the first place — see
[Dependencies and Safety](Dependencies-and-Safety.md). A database's cluster is
not in the snapshot, because it is not something a package manager owns.

**Cannot:** restore a version a provider no longer offers. If a package was
removed and upstream has since withdrawn that version, the plan will say so
rather than silently installing a different one.

Every run will report, when it finishes, whether it is **fully restorable,
partially restorable, or not restorable, with the reason** — before you need to
know.

## Meanwhile, doing it by hand

If you follow a plan manually:

1. `macportsbrew snapshot --out before.json` first. Always.
2. Follow the plan **in order**. The order is not cosmetic: install and verify
   under the destination before removing anything from the source.
3. `macportsbrew snapshot --out after.json` when you are done, and compare. The
   two files are ordinary JSON and `diff` works on them.

If something went wrong, `before.json` is a complete record of what was
installed and at which version, which is what you need to put it back.

## See also

- [Dependencies and Safety](Dependencies-and-Safety.md) — what makes a
  migration reversible in the first place.
- [Troubleshooting](Troubleshooting.md) — when the machine is already in a
  state you did not want.
