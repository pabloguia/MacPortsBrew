# MacPorts to Homebrew Migration

This page covers moving a package from MacPorts to Homebrew. The opposite
direction is [Homebrew to MacPorts](Homebrew-to-MacPorts-Migration.md), and the
two are the same code: one planner, one policy engine, one set of safety
checks, with the provider identifiers exchanged. Exchanging them in the input
exchanges them in the output and changes nothing else, and a test asserts it
(`invariant_17_swapping_the_providers_swaps_the_analysis`).

If a page ever reads as though this direction is an afterthought, that is a
documentation bug worth reporting.

## Before you start: running a plan is for a later release

**This build plans. It does not migrate.** `macportsbrew plan` prints the
ordered steps, the commands that would run, the risks, and where a human would
have to decide — and then stops. `macportsbrew migrate` does the same, and
with `--yes` reaches a refusal gate that refuses every plan version 0.1.0
produces; nothing here asks for a password, and nothing on your machine changes.

## When this direction is the right one

Moving to Homebrew is often the better answer, and the policy engine will say
so on its own when the facts support it:

- **You are on Apple Silicon.** Homebrew's Support Tiers page puts Apple
  Silicon on macOS 14, 15 and 26 in Tier 1, its fully supported tier
  (<https://docs.brew.sh/Support-Tiers>, read 2026-09-04; recorded in
  `data/policy/provider-support.toml`), and its prebuilt-artifact coverage
  there is materially better: on 2026-09-02, 7277 of 8586 core formulae carried an `arm64_sonoma`
  bottle against 6680 with the Intel `sonoma` one
  ([`../docs/research/F-1-homebrew-support-tiers.md`](../docs/research/F-1-homebrew-support-tiers.md)).
  A package that MacPorts would compile and Homebrew would pour is a real
  difference in your afternoon.
- **The package has a prebuilt artifact on the Homebrew side and not on the
  MacPorts side.** That is a rule, `SOURCE_NO_PREBUILT_DESTINATION_PREBUILT`,
  and it fires in whichever direction the facts point.
- **You want a tool in the ecosystem where its maintainers are.** A legitimate
  reason the tool cannot see; ask for the plan anyway, and it says that it goes
  against the recommendation.

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
macportsbrew: A safety rule could not be evaluated from this input, so no plan was built and nothing was changed: whether macports package gnutls defines a background service is not known, so the rule that forces manual review for a service package cannot be evaluated for gnutls. This does not necessarily mean anything is wrong on your machine — the fact is absent from the input, or could not be read where it lives. `analyze`, `status` and `explain` do not depend on it and still work.
```

Whether a package defines a background service is two questions: does it
**declare** one, and did its provider **write** one on disk. For a port, both
answers are on the recorded machine's disk — the Portfile it was installed from,
and its launch items — so a recording answers neither, and every plan out of
MacPorts from a replayed recording is refused as unverifiable, by design: a
safety rule that cannot be evaluated is a refusal, never a pass. The same holds
for a Homebrew formula that declares no service, whose keg a recording cannot
show. The refusal is about the recording, not about this direction.

A snapshot taken on a live machine does record that fact, and from one this
direction plans in full. Here is the plan from the committed live capture of an
Intel Mac running both providers, where `htop` is installed under both — which
matters: in 0.1.0 a package is matched only against what the destination already
has installed, or the small curated table, so a port Homebrew has not installed
stops at *Needs a human decision* with "no equivalent found" (see
[Package Mapping](Package-Mapping.md)). The command and its complete output,
which exits 0 (the plan digest is shortened here: its 64 digits change with each
version, and a digest is not something to publish):

```sh
macportsbrew --from-snapshot fixtures/snapshots/both-providers-intel-macos26-live.json migrate htop --to homebrew
```

```
Plan: move htop from macports to homebrew
  This version changes nothing on your machine. These steps are what a migration would do; if you carry them out yourself, run each command in order, check its verification before going on, and keep the source with --retain-source until you are sure. You run them at your own risk.
  This plan goes against the recommendation (Keep both, homebrew first).

Steps, in order
  1. Take a snapshot - Take a snapshot of both package managers before the first change. This version does not take it for you: run `macportsbrew snapshot --out before.json` first.
  2. Add under the destination - htop is already installed under homebrew; nothing to add.
  3. Verify the destination - Check that the destination reports it installed and active, that its own integrity check passes, that the expected executables belong to it, that each one starts, and that nothing links across the two package managers.
       $ /usr/local/bin/brew list --versions htop
       $ /usr/local/bin/brew info --json=v2 --installed  [passes when this whole list marks exactly one installation of the package as the active one; refused unless it exited cleanly, was not truncated and came back non-empty]
       ? the provider reports it installed and active
       ? the provider's own integrity check passes
       ? the expected executables belong to the destination: htop
       ? htop --version exits successfully
       ? no new cross-provider linkage
  4. Set the primary - Make homebrew's copy the one that runs.
       Not a command in this version. To make the copy under homebrew the one that runs, put its bin directory ahead of the other package manager's on PATH, or call it by its full path.
       ? the provider reports it installed and active
  5. Verify what resolves - Check which copy now runs: `command -v htop` should print a path under the destination's prefix.
  6. Remove from the source - Remove htop from macports. This happens only after the destination is verified.
       $ /opt/local/bin/port uninstall htop  [elevation required: The default MacPorts prefix /opt/local is created by the installer and owned by root (guide.macports.org, read 2026-09-02).]
  7. Verify the removal - Verify the removal.
       $ /opt/local/bin/port -q installed  [passes when this list does NOT name the package; refused unless it exited cleanly, was not truncated and came back non-empty]
       ? the source no longer reports it installed
  8. Review what is left behind - Show cleanup candidates from the provider's own mechanism for review; nothing is removed automatically.
  9. Record the run - Record what was done. This version executes nothing, so it records no run; `history` and `restore --plan` read the runs a later version records. Take your own before-snapshot (`macportsbrew snapshot --out before.json`).

Steps that need administrator rights: 6.

Plan digest: sha256:…

Shown only; nothing was run. With --yes (and without --dry-run) the plan is put to the execution gate, which refuses every plan this build produces.

Possibly inconsistent: this was read from a recording or a snapshot file, which cannot show whether a package manager was changing the machine it came from while it was read.
```

Read it the way you would read the other direction's: steps in order, each
command with how to check it, the one step that needs administrator rights
named, and nothing removed from MacPorts until step 3 has verified the Homebrew
copy. Step 4 is not a command in this version; it tells you what to do by hand.
The last line says the snapshot file cannot vouch for what was running beside
it when it was captured; `--json` carries the same answer as
`concurrentProviderActivity`.

The [Homebrew to MacPorts](Homebrew-to-MacPorts-Migration.md) page's example
is replayed from a recording, and a test holds that block to the program's
output. The block above is copied from a real run but is not checked by that
test, which accepts only replayed recordings. What the tests establish about
symmetry is stated where it is checked:
`invariant_17_swapping_the_providers_swaps_the_analysis` swaps the providers in
the input and asserts the analysis swaps with them;
`invariant_17_a_macports_to_homebrew_plan_is_built_from_a_live_capture` builds
this plan from the live capture and checks it mirrors the other direction. The
test that puts every plan from the replayed recordings to the execution gate
reaches only Homebrew-to-MacPorts plans, for the reason above; the
MacPorts-to-Homebrew direction of the gate is covered by a synthesised snapshot
in `crates/application/tests/execution_gate.rs`.

A package that owns data is never migrated automatically, in either direction.
No committed input holds such a package under MacPorts with its service facts
recorded, so this page cannot show one moving to Homebrew. The same rule in the
other direction, from `plan postgresql17-server --to macports` replayed from
`both-providers-arm64-macos15` (the plan's first three risk lines; `…` marks
what is left out):

```
…
Risks detected
  ! postgresql17-server is classed as Holds data, and packages in that class are never migrated automatically. A person decides this one.
  - Whether macports would supply postgresql17-server ready-built or compile it here could not be determined. An unknown is not a yes.
…
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
9. **Record history** — designed; 0.1.0 keeps no history.

**Nothing is removed from MacPorts until the Homebrew package is installed and
verified** (a test enforces that ordering), and because of that ordering a
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
planning never elevate.

## Doing it by hand in the meantime

The plan is a reviewed description of exactly what to do, and you can follow it
yourself, at your own risk. If you do, follow it in order, and keep the one
rule that matters: **install and verify under Homebrew before removing anything
from MacPorts.** Take `macportsbrew snapshot --out before.json` first, so you
can see afterwards what changed. Leaving the MacPorts copy installed until you
are sure is the safe pattern — it is what `--retain-source` will do once a
release runs plans.

## Variants have no Homebrew equivalent

MacPorts ports can be built with variants — optional build configurations
chosen at install time, which also change which prebuilt archive applies
([research note R-4](../docs/research/R-4-macports-binary-archives.md), read
2026-09-02). A port installed with non-default variants may not have a Homebrew
equivalent that does what your build does. MacPortsBrew reads the variant set
of the active installed port, and reports a port built with `+universal` or
with a variant explicitly turned off as a source build caused by non-default
options; other variants are not flagged, and it does not compare your variants
with anything on the Homebrew side. Check what your port was built with before accepting a plan.

## See also

- [Homebrew to MacPorts Migration](Homebrew-to-MacPorts-Migration.md).
- [Understanding Bottles and Source Builds](Understanding-Bottles-and-Source-Builds.md)
  — the artifact question that often decides this direction.
- [Restoring After a Migration](Restoring-After-a-Migration.md) — what you can
  record before you change anything.
