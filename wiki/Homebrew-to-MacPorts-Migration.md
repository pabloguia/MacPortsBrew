# Homebrew to MacPorts Migration

This page covers moving a package from Homebrew to MacPorts. The opposite
direction is [MacPorts to Homebrew](MacPorts-to-Homebrew-Migration.md), and it
is the same procedure with the two names exchanged — not a second-class path.

## Before you start: execution is a later pass

**This build plans. It does not migrate.** `macportsbrew plan` prints the
ordered steps, the commands that would run, the risks, and where a human would
have to decide — and then stops. There is no `migrate` command in version
0.1.0, nothing here asks for a password, and nothing on your machine changes.

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

On a machine with Homebrew and no MacPorts, that produces:

```
Plan: move curl from homebrew to macports
  This build only shows plans. Nothing here is executed, and nothing asks for elevation.
  This plan overrides the recommendation (Needs a human decision); the override would be recorded with the run.

Risks detected
  ! macports is not installed on this machine; installing a provider needs explicit consent and is a separate step.
  ! The equivalent under macports is only a None match, and acting on one needs Strong or better. Look at the candidate before deciding.
  ! 2 packages under homebrew depend on this one, and removing or moving it on its own can break them.

Steps, in order
  1. ManualReview - macports is not installed on this machine; installing a provider needs explicit consent and is a separate step.
       Execution would stop here for a human decision (exit code 4).
  2. ManualReview - The equivalent under macports is only a None match, and acting on one needs Strong or better. Look at the candidate before deciding.
       Execution would stop here for a human decision (exit code 4).
  3. ManualReview - 2 packages under homebrew depend on this one, and removing or moving it on its own can break them.
       Execution would stop here for a human decision (exit code 4).
```

Three things to notice.

**The plan records that it overrode the recommendation.** You asked to move
something the engine would not have moved. That is allowed — it is your
machine — and the override is recorded rather than quietly accepted.

**MacPorts being absent is a step, not an assumption.** MacPortsBrew never
installs a package manager implicitly. Installing one is a separate action that
needs your explicit consent.

**Each blocking risk becomes a step.** A plan is not a summary of a decision;
it is the decision, in order, with the reasons attached. Executing a plan that
contains a `ManualReview` step exits 4 and names the step.

Add `--json` for the machine-readable form — see
[`../docs/schemas/plan.v1.md`](../docs/schemas/plan.v1.md).

## What a full migration plan will contain

When both providers are present and the mapping is confident, the plan follows
this order, and the order is enforced rather than conventional:

1. **Snapshot before** — both providers' state, so the run is restorable.
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
9. **Record history.**

**Nothing is removed from Homebrew until the MacPorts package is installed and
verified** (INV-01, with a test named after it), and because of that ordering a
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
— that is INV-06, and this build has no code path that elevates.

## Doing it by hand in the meantime

The plan is a reviewed description of exactly what to do, and you can follow it
yourself. If you do, follow it in order, and keep the one rule that matters:
**install and verify under MacPorts before removing anything from Homebrew.**
Take `macportsbrew snapshot --out before.json` first, so you can see afterwards
what changed.

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
- [Restoring After a Migration](Restoring-After-a-Migration.md) — the way back.
