# Homebrew to MacPorts Migration

This page covers moving a package from Homebrew to MacPorts. The opposite
direction is [MacPorts to Homebrew](MacPorts-to-Homebrew-Migration.md), and it
is the same procedure with the two names exchanged — not a second-class path.

## Before you start: running a plan is for a later release

**This build plans. It does not migrate.** `macportsbrew plan` prints the
ordered steps, the commands that would run, the risks, and where a human would
have to decide — and then stops. `macportsbrew migrate` does the same, and
with `--yes` reaches a refusal gate that refuses every plan version 0.1.0
produces; nothing here asks for a password, and nothing on your machine changes.

That is worth doing anyway: the plan is the thing you review, and reviewing it
is how you find out whether the migration is a good idea before anyone acts on
it.

## Should you move this package at all?

Ask the tool.

```sh
macportsbrew explain <package>
```

Common answers and what they mean:

| Recommendation | What it means |
|---|---|
| **Nothing to do** | The package is fine where it is, and often there is nowhere else for it to go. |
| **Keep under homebrew** | Something was considered and Homebrew still wins for this package on this machine. |
| **Move to macports** | A stated support end date, a missing prebuilt artifact on this side and one available on the other, or both. |
| **Keep both** | A supported end state. See [Using Both Together](Using-Homebrew-and-MacPorts-Together.md). |
| **Use a language runtime manager instead** | It is a language runtime. Neither provider is the right tool; a version manager is. |
| **Needs a human decision** | Something in the package makes an unattended move unsafe. `explain` names it. |

"Needs a human decision" is a normal outcome, not a failure. The usual causes
are covered in [Dependencies and Safety](Dependencies-and-Safety.md).

## Build the plan

```sh
macportsbrew plan curl --to macports
```

On a machine with Homebrew and no MacPorts — here the committed recording
`homebrew-only-intel-macos26`, replayed, so a test holds this block to what the
program prints — that produces, and exits 4:

<!-- sample: --replay fixtures/recordings/homebrew-only-intel-macos26.json plan curl --to macports --today 2026-09-02 exit=4 -->
```
Plan: move curl from homebrew to macports
  This version changes nothing on your machine. These steps are what a migration would do; if you carry them out yourself, run each command in order, check its verification before going on, and keep the source with --retain-source until you are sure. You run them at your own risk.
  This plan goes against the recommendation (Needs a human decision).

Risks detected
  ! macports is not installed on this machine. MacPortsBrew does not install package managers; install macports yourself first if you want to move packages there.
  ! No equivalent under macports reaches confidence level Strong: the best is at level None. Check whether macports has an equivalent before deciding.
  ! 2 packages under homebrew depend on this one, and removing or moving it on its own can break them.
  - Whether the configuration of homebrew overrides the settings this tool would run it with is not known from this input - it was read from a recording or a snapshot that does not record it. A step that changes homebrew is refused while it is unknown.

Steps, in order
  1. Needs a human decision - macports is not installed on this machine. MacPortsBrew does not install package managers; install macports yourself first if you want to move packages there.
       Execution would stop here for a human decision (exit code 4).
  2. Needs a human decision - No equivalent under macports reaches confidence level Strong: the best is at level None. Check whether macports has an equivalent before deciding.
       Execution would stop here for a human decision (exit code 4).
  3. Needs a human decision - 2 packages under homebrew depend on this one, and removing or moving it on its own can break them.
       Execution would stop here for a human decision (exit code 4).

Possibly inconsistent: this was read from a recording or a snapshot file, which cannot show whether a package manager was changing the machine it came from while it was read.
```

Four things to notice.

**The plan says it goes against the recommendation.** You asked to move
something the engine would not have moved. That is allowed — it is your
machine — and the plan says so rather than quietly accepting it.

**MacPorts being absent is a step, not an assumption.** MacPortsBrew does not
install package managers. If you want the package under MacPorts, install
MacPorts yourself first, then build the plan again.

**Each blocking risk becomes a step.** A plan is not a summary of a decision;
it is the decision, in order, with the reasons attached. A plan with a step
that needs a human decision exits 4 and names the step.

**The last line says what the read cannot vouch for.** Whether a package
manager was changing the machine while it was read is not something this build
checks, so the output of `plan`, `analyze` and `migrate` ends by saying it may be
inconsistent and why — here, because a recording cannot show what was running
beside it. `--json` carries the same answer as `concurrentProviderActivity`.

Add `--json` for the machine-readable form — see
[`../docs/schemas/plan.v1.md`](../docs/schemas/plan.v1.md).

## What a full migration plan will contain

When both providers are present and the mapping is confident, the plan follows
this order, and the order is enforced rather than conventional:

1. **Snapshot before** — both providers' state, recorded before anything
   changes. A restore command that uses it is designed but is not in 0.1.0;
   today, take your own with `macportsbrew snapshot --out before.json`.
2. **Add destination** — install under MacPorts.
3. **Verify destination** — the package is installed and active, its
   executables are where MacPorts says they are, and a liveness probe with a
   safe flag exits cleanly.
4. **Set primary** — decide which copy actually runs. See
   [Primary Provider](Primary-Provider.md).
5. **Verify resolution** — confirm it does.
6. **Remove source** — *optional*, and only reachable after step 3 passed.
   `--retain-source` skips it entirely.
7. **Verify removal.**
8. **Cleanup review** — what was left behind, for you to decide about.
9. **Record history** — designed; 0.1.0 keeps no history.

**Nothing is removed from Homebrew until the MacPorts package is installed and
verified** (a test enforces that ordering), and because of that ordering a
failed step leaves the machine at least as usable as it was — *whenever that is
technically possible*. That qualifier is load-bearing, and
[Dependencies and Safety](Dependencies-and-Safety.md#what-is-guaranteed-and-what-is-not)
says why in one place rather than four.

## Elevation

MacPorts' default prefix `/opt/local` is created by the installer and owned by
root, so MacPorts add, remove, `select` and service operations need
administrator rights. Homebrew's prefix subdirectories are user-owned, so
formula operations do not
([`../docs/research/R-5-privilege-requirements.md`](../docs/research/R-5-privilege-requirements.md),
2026-09-02, citing `guide.macports.org` and `docs.brew.sh/FAQ`).

MacPortsBrew handles that by **enumerating** it: every step that would need
elevation is listed in the plan as needing it, per operation, visible before
anything runs. Analysis, planning and `doctor` need no elevation at all, ever
— and this build has no code path that elevates.

## Doing it by hand in the meantime

The plan is a reviewed description of exactly what to do, and you can follow it
yourself, at your own risk. If you do, follow it in order, and keep the one
rule that matters: **install and verify under MacPorts before removing anything
from Homebrew.** Take `macportsbrew snapshot --out before.json` first, so you
can see afterwards what changed. Leaving the Homebrew copy installed until you
are sure is the safe pattern — it is what `--retain-source` will do once a
release runs plans.

## The Intel case

If Homebrew reports `degraded` for your machine and you are on Intel, the
support timeline is probably why some packages are recommended for a move. That
is one dated fact among several, not a blanket instruction to migrate
everything — read [Intel Mac Homebrew Support](Intel-Mac-Homebrew-Support.md)
before deciding.

## See also

- [MacPorts to Homebrew Migration](MacPorts-to-Homebrew-Migration.md) — the
  same thing, the other way.
- [Package Mapping](Package-Mapping.md) — why the equivalent was or was not
  found.
- [Restoring After a Migration](Restoring-After-a-Migration.md) — what you can
  record before you change anything, and what a restore will look like.
