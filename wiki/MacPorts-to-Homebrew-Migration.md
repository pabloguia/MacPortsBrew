# MacPorts to Homebrew Migration

This page covers moving a package from MacPorts to Homebrew. The opposite
direction is [Homebrew to MacPorts](Homebrew-to-MacPorts-Migration.md), and the
two are the same code: one planner, one policy engine, one set of safety
checks, with the provider identifiers exchanged. Exchanging them in the input
exchanges them in the output and changes nothing else, and a test asserts it
(`invariant_17_swapping_the_providers_swaps_the_analysis`).

If a page ever reads as though this direction is an afterthought, that is a
documentation bug worth reporting.

## Before you start: execution is a later pass

**This build plans. It does not migrate.** `macportsbrew plan` prints the
ordered steps, the commands that would run, the risks, and where a human would
have to decide — and then stops. `macportsbrew migrate` does the same, and
with `--yes` reaches a refusal gate that refuses every plan version 0.1.0
produces; nothing here asks for a password, and nothing on your machine changes.

## When this direction is the right one

Moving to Homebrew is often the better answer, and the policy engine will say
so on its own when the facts support it:

- **You are on Apple Silicon.** Homebrew classifies `arm64` macOS as a
  first-class target, and its prebuilt-artifact coverage there is materially
  better: on 2026-09-02, 7277 of 8586 core formulae carried an `arm64_sonoma`
  bottle against 6680 with the Intel `sonoma` one
  ([`../docs/research/F-1-homebrew-support-tiers.md`](../docs/research/F-1-homebrew-support-tiers.md)).
  A package that MacPorts would compile and Homebrew would pour is a real
  difference in your afternoon.
- **The package has a prebuilt artifact on the Homebrew side and not on the
  MacPorts side.** That is a rule, `SOURCE_NO_PREBUILT_DESTINATION_PREBUILT`,
  and it fires in whichever direction the facts point.
- **You want a tool in the ecosystem where its maintainers are.** A legitimate
  reason the tool cannot see; override the recommendation and it will record
  that you did.

## Should you move this package at all?

```sh
macportsbrew explain <package>
```

The same outcomes as the other direction, symmetric in every case:
*Nothing to do*, *Keep under macports*, *Move to homebrew*, *Keep both*,
*Use a language runtime manager instead*, *Needs a human decision*.

## Build the plan

```sh
macportsbrew plan gnutls --to homebrew
```

On a machine with MacPorts and no Homebrew:

```
Plan: move gnutls from macports to homebrew
  This build changes nothing: `migrate --yes` reaches a refusal gate that refuses every plan this build produces, so nothing here is executed and nothing asks for elevation.
  This plan overrides the recommendation (Nothing to do); the override would be recorded with the run.

Risks detected
  ! homebrew is not installed on this machine; installing a provider needs explicit consent and is a separate step.
  ! The equivalent under homebrew is only a None match, and acting on one needs Strong or better. Look at the candidate before deciding.

Steps, in order
  1. ManualReview - homebrew is not installed on this machine; installing a provider needs explicit consent and is a separate step.
       Execution would stop here for a human decision (exit code 4).
  2. ManualReview - The equivalent under homebrew is only a None match, and acting on one needs Strong or better. Look at the candidate before deciding.
       Execution would stop here for a human decision (exit code 4).
```

Compare that with the example on the
[Homebrew to MacPorts](Homebrew-to-MacPorts-Migration.md) page: same wording,
same structure, same exit code, the names swapped. That is what symmetry means
here in practice.

A package that owns data behaves the same way in both directions too:

```
Plan: move postgresql17-server from macports to homebrew
  ...
Risks detected
  ! homebrew is not installed on this machine; installing a provider needs explicit consent and is a separate step.
  ! postgresql17-server is classed as Stateful, and packages in that class are never migrated automatically. A person decides this one.
  - Whether homebrew would supply postgresql17-server ready-built or compile it here could not be determined. An unknown is not a yes.
```

"An unknown is not a yes" is the rule, not a turn of phrase: a fact that could
not be determined is never quietly treated as favourable.

Add `--json` for the machine-readable form — see
[`../docs/schemas/plan.v1.md`](../docs/schemas/plan.v1.md).

## What a full migration plan will contain

Identical to the other direction, because it is the same planner:

1. **Snapshot before.**
2. **Add destination** — install under Homebrew.
3. **Verify destination** — installed, active, executables owned by Homebrew,
   a liveness probe with a safe flag.
4. **Set primary** — see [Primary Provider](Primary-Provider.md).
5. **Verify resolution.**
6. **Remove source** — optional, only reachable after step 3 passed;
   `--retain-source` skips it.
7. **Verify removal.**
8. **Cleanup review.**
9. **Record history.**

**Nothing is removed from MacPorts until the Homebrew package is installed and
verified** (INV-01, with a test named after it), and because of that ordering a
failed step leaves the machine at least as usable as it was — *whenever that is
technically possible*. That qualifier is load-bearing, and
[Dependencies and Safety](Dependencies-and-Safety.md#what-is-guaranteed-and-what-is-not)
says why in one place rather than four.

## Elevation, in this direction

The asymmetry here is real and MacPortsBrew does not pretend otherwise.
Homebrew refuses to run as root and its prefix subdirectories are user-owned,
so installing a formula needs no elevation; **removing the MacPorts package
does**, because `/opt/local` is root-owned
([`../docs/research/R-5-privilege-requirements.md`](../docs/research/R-5-privilege-requirements.md),
2026-09-02).

So a MacPorts-to-Homebrew plan typically elevates on the *removal* step rather
than the *install* step — the mirror image of the other direction. The plan
lists which steps those are, per operation, before anything runs. Analysis and
planning never elevate (INV-06).

## Variants have no Homebrew equivalent

MacPorts ports can be built with variants; Homebrew formulae have options but
not the same concept. A port installed with non-default variants may not have a
Homebrew equivalent that does what your build does, and MacPortsBrew will
report the availability as needing a source build or as unknown rather than
assuming the plain formula is a substitute. Check what your port was built with
before accepting a plan.

## See also

- [Homebrew to MacPorts Migration](Homebrew-to-MacPorts-Migration.md).
- [Understanding Bottles and Source Builds](Understanding-Bottles-and-Source-Builds.md)
  — the artifact question that often decides this direction.
- [Restoring After a Migration](Restoring-After-a-Migration.md).
