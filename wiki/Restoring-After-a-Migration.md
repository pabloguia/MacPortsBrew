# Restoring After a Migration

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

A snapshot is written without your user name or home folder path, but it
carries your `PATH` (with your home folder shown as `~`) and the full list of
installed packages. Read it before you share it. `snapshot --out` refuses to
replace a file that is not a MacPortsBrew document; giving each snapshot a new name keeps the earlier ones.

## What the restore path will be

**None of the commands in this section exists in 0.1.0** — `history` and
`restore` are designed and not yet written, and typing them today prints an
"unrecognized subcommand" error. What follows is the design.

Every mutating run will begin by taking a snapshot, and record it against the
run. Then:

```sh
macportsbrew history                     # find the run
macportsbrew history show <id>           # what it did, what completed
macportsbrew restore --run <id> --plan   # a reviewable plan back
```

**The command is `restore`, and it is deliberately not called `undo`.** An undo
puts things back the way they were. This does not do that, and could not: what
it produces is a *reinstall plan* — the same kind of plan as any other
migration, run forward. It asks the provider for the packages you had, and the
provider installs what it offers today. Nothing is rewound; something is built
again. Calling that "undo" would promise a reversal, and the name is the first
place a false promise gets made.

That it is an ordinary plan is the point, not a limitation. `restore --plan`
produces ordered steps, every command with its full argv, the risks, the
elevation, and a human review before anything happens. There is no separate
"rollback engine" with its own semantics to learn and its own bugs to find.

`restore --plan --snapshot <file>` does the same thing against a snapshot file
rather than a recorded run.

A run that dies mid-flight leaves a recoverable record, and the next mutating
run refuses to start until you have looked at it — exiting with the
safety-check code and naming the run. Recovery is the same two commands:
`history show <id>` to see what completed, then either restore or a fresh plan
forward. There is no `resume` command, because there is nothing separate to
resume.

## What a restore can and cannot promise

This, too, describes the design; 0.1.0 has no restore.

**Can:** reinstall packages that were removed, remove packages that were added,
and put back the primary-provider selection *for the packages that run changed*
— all as a reviewed plan, in an order that never removes before it has
installed and verified. A selection you made yourself, before or after, is not
touched: the record is of what that run did, not of how your machine was
configured.

**Cannot:** bring back data that was outside a package's prefix. This is
exactly why packages that own persistent data are never migrated automatically
in the first place — see
[Dependencies and Safety](Dependencies-and-Safety.md). A database's cluster is
not in the snapshot, because it is not something a package manager owns.

**Cannot:** reconstruct the versions you had. This is the limit people are most
often surprised by, and it runs in both directions. If upstream has withdrawn a
version, the plan says so rather than silently installing a different one — and
wherever upstream has published a newer version since, a restore gives you that
one, not the version you had. Both are shown in
the plan, per package, before you approve it. What a restore guarantees is that
the packages are there again, not that the machine is byte-for-byte what it was.

**Cannot:** reverse anything that happened outside MacPortsBrew. If you ran
`brew upgrade` by hand, edited a configuration file, or removed a port yourself
between the run and the restore, none of it is undone or accounted for — the
snapshot records what MacPortsBrew observed at that moment, and the restore
plans against it. Where a package is no longer in the state the snapshot
recorded, you see that in the plan rather than having it silently overwritten.

**Cannot:** run at all from a snapshot that is missing or unreadable. Such a run
is listed as **not restorable, with the reason**, rather than restored from a
guess about what it probably contained.

**The case this project exists for.** A step whose artifact is no longer
available on *this* machine — the Homebrew formula an Intel Mac can no longer
get a bottle for, a provider that has dropped your macOS version — does not
quietly disappear from the plan and does not quietly fail in the middle of it.
It becomes a step marked for manual review, shown in the plan with the reason,
and the run is reported as **partially restorable** before you start. Running
such a plan stops at that step with the safety-check exit code rather than
continuing past it. This is exactly the situation in which the word "undo" would
have been a lie, which is a large part of why the command is not called that.

Every run reports, when it finishes, whether it is **fully restorable, partially
restorable, or not restorable, with the reason** — before you need to know.

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

- [Dependencies and Safety](Dependencies-and-Safety.md) — the ordering that
  keeps a failed run recoverable, and what it does not promise
- [Troubleshooting](Troubleshooting.md) — when the machine is already in a
  state you did not want.
