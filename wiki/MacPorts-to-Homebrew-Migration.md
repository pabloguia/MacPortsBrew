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
  reason the tool cannot see; override the recommendation, and the plan
  records the override — which the run would record with it, once a build
  executes plans.

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

**From the committed recordings, this direction is refused before a plan
exists.** Replaying `macports-only-intel-macos12`, the one committed recording
with MacPorts and no Homebrew, prints this on stderr, nothing on stdout, and
exits 4:

```
macportsbrew: A safety rule could not be evaluated from this input, so no plan was built and nothing was changed: this input does not record whether macports packages define services, so the rule that forces manual review for a service package cannot be evaluated for gnutls. This is not something to fix on your machine — the fact is absent from the document itself. `analyze`, `status` and `explain` do not depend on it and still work.
```

No committed recording captures whether a MacPorts port defines a service, so
every plan out of MacPorts from a recording is refused as unverifiable — a
safety rule that cannot be evaluated is a refusal, never a pass (INV-19). The
refusal is about the recording, not about this direction.

On a live machine with MacPorts and no Homebrew, where that fact is read, the
plan has the shape below. **This block is illustrative**: no committed
recording produces it, so no test holds it to what the program prints, and the
labels are written the way the program prints them.

```
Plan: move gnutls from macports to homebrew
  This build changes nothing: `migrate --yes` reaches a refusal gate that refuses every plan this build produces, so nothing here is executed and nothing asks for elevation.
  This plan overrides the recommendation (Nothing to do); the override would be recorded with the run.

Risks detected
  ! homebrew is not installed on this machine; installing a provider needs explicit consent and is a separate step.
  ! No equivalent under homebrew reaches confidence level Strong: the best is at level None. Check whether homebrew has an equivalent before deciding.

Steps, in order
  1. Needs a human decision - homebrew is not installed on this machine; installing a provider needs explicit consent and is a separate step.
       Execution would stop here for a human decision (exit code 4).
  2. Needs a human decision - No equivalent under homebrew reaches confidence level Strong: the best is at level None. Check whether homebrew has an equivalent before deciding.
       Execution would stop here for a human decision (exit code 4).

Possibly inconsistent: whether a package manager was changing this machine while it was read was not checked. This build has no such check, so if one was running, rerun this once it has finished.
```

On a live machine the last line says the read was *not checked*; the replayed
example on the other page says it *cannot be known*. `--json` carries both as
`concurrentProviderActivity`.

The [Homebrew to MacPorts](Homebrew-to-MacPorts-Migration.md) page's example
is replayed from a recording and tested; this one is not. What the tests do
establish about symmetry is narrower and stated where it is checked:
`invariant_17_swapping_the_providers_swaps_the_analysis` swaps the providers in
the input and asserts the analysis swaps with them, and the test that puts
every plan the committed recordings produce to the execution gate reaches only
Homebrew-to-MacPorts plans, for the reason above. The MacPorts-to-Homebrew
direction of the gate is covered by a synthesised snapshot in
`crates/application/tests/execution_gate.rs`.

A package that owns data is refused the same way from the recording. On a live
machine its plan carries a stateful-class risk — again illustrative, and
abbreviated:

```
Plan: move postgresql17-server from macports to homebrew
  ...
Risks detected
  ! homebrew is not installed on this machine; installing a provider needs explicit consent and is a separate step.
  ! postgresql17-server is classed as Holds data, and packages in that class are never migrated automatically. A person decides this one.
  - Whether homebrew would supply postgresql17-server ready-built or compile it here could not be determined. An unknown is not a yes.
```

"An unknown is not a yes" is the rule, not a turn of phrase: a fact that could
not be determined is never quietly treated as favourable.

On a live machine, a `~/.macports/macports.conf` that sets anything, or a
Homebrew `brew.env` that does, adds one more risk naming that provider and the
file: its own configuration overrides how this tool would run it, and a step
that changes it is refused while it does. `macportsbrew doctor` lists the same
files. The same holds in the other direction.

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
