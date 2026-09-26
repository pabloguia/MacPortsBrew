# FAQ

## Which is better, Homebrew or MacPorts?

Neither, globally. That is not diplomacy — it is the position the whole tool is
built on. Which one is better depends on your machine, the specific package,
and what each provider currently offers for it. `macportsbrew analyze` answers
it per package, and `macportsbrew explain <package>` shows the reasoning.

No rule inside MacPortsBrew names a provider as preferable. Provider identifiers
flow through the policy engine as values, and a test checks that exchanging them
in the input exchanges them in the output and changes nothing else.

## Does the name mean it prefers MacPorts?

No. The name pairs the two and yields the alias `mpb`, which is also a nod to
*Música Popular Brasileira*. Leading with "MacPorts" is an accident of which
order makes a better word.

## Will it break my setup?

Not in this build — it executes nothing. When execution lands, these hold:

- Nothing is removed until its replacement is installed **and verified**.
- Every destructive action comes from a plan you have seen; nothing is derived
  at execution time.
- If a step fails, the machine is left at least as usable as before *whenever
  that is technically possible*, and partial success is reported as partial
  success — never as success.
- Reverse dependencies are checked before any removal.
- Packages that hold data or run as services are never migrated automatically.

That third one carries a qualifier, and it is not padding. Neither provider
documents a transactional install or uninstall, and MacPorts' `uninstall` is
documented as two actions in one — *"Deactivate and uninstall portname"*
(`port(1)`, read 2026-09-02) — so a step that dies in the middle of its own work
can leave a port deactivated but still present. What MacPortsBrew guarantees is
that it does not *create* that risk: the replacement is installed and verified
before anything is removed. What it cannot guarantee is undoing a provider's own
half-finished write. When that happens you are told which step and what it left,
rather than being told it worked. There is more on this in
[Dependencies and Safety](Dependencies-and-Safety.md#what-is-guaranteed-and-what-is-not).

Each of the five above is an invariant with a test named after it.

## Do I need to give it my password?

No, and this build has no code path that could ask. Discovery, inventory,
mapping, analysis, planning and dry-run are all unprivileged by design (INV-06).

When execution lands, some operations will need administrator rights —
MacPorts' `/opt/local` is root-owned, so its add and remove operations do,
while Homebrew's user-owned prefix subdirectories mean formula operations do
not. Those steps are enumerated in the plan, per operation, visible before
anything runs, and never cached or inherited.

## Does it send anything anywhere?

No. No telemetry, no analytics, no network call that reports anything about
you. `analyze` works with no network at all, marking network-dependent facts
`UNKNOWN` rather than guessing.

`doctor --report` writes a **local** redacted file — no usernames, home paths,
environment variable values or tokens — which you read and choose to share.
MacPortsBrew never uploads it.

## Do I need Homebrew or MacPorts installed?

No. MacPortsBrew reports what it finds, including neither. It will never
install a package manager implicitly — that needs your explicit consent and is
a separate step (INV-05).

## Is this only for Intel Macs?

No. The Intel support timeline is why the project started, and it is not what
the project is. MacPorts-to-Homebrew is a first-class direction with the same
planner, coexistence is a supported end state, and on Apple Silicon Homebrew is
very likely the better provider for most of what you have. See
[Intel Mac Homebrew Support](Intel-Mac-Homebrew-Support.md).

## Why does it say "needs a human decision" so often?

Because acting wrongly on a package is expensive and acting wrongly on your
database is worse. The usual causes are reverse dependencies, a package that
holds data or runs as a service, a mapping below `Strong` confidence, an
availability fact that could not be determined, or the destination provider not
being installed.

`macportsbrew explain <package>` names which one, every time. It is a
considered refusal, not a shrug.

## Can I override a recommendation?

Yes. It is your machine. `macportsbrew plan <pkg> --to <provider>` builds the
plan whether or not the engine agrees, and says so:

```
This plan overrides the recommendation (Needs a human decision); the override
would be recorded with the run.
```

The reasons that produced the original recommendation survive in the plan as
risks and as steps. Nothing is hidden by overriding.

## Why does it keep telling me to use a version manager for Python?

Because language runtimes are used in a way neither package manager fits well:
you generally want several versions, per project, switchable. Both providers
can host a runtime; a version manager usually serves you better.

It is a recommendation and nothing more — MacPortsBrew does not manage
runtimes, does not install version managers, and will not migrate your Python.

## Why are most of my packages hidden?

Normal mode shows only packages **you asked for**. The rest are dependencies
that a provider installed on their behalf and manages automatically. On the
example machine, 189 Homebrew packages, 56 of them top-level.

`macportsbrew analyze --advanced` shows them, with a warning about what they
are. The safety checks do not relax in advanced mode.

## Can it consolidate the duplicate libraries?

No, and it never will. Duplication across providers is **correct**: it is what
keeps each provider able to reason about, upgrade and remove its own tree
safely. One provider's package never satisfies the other's dependency (INV-08),
and consolidating is precisely the operation that breaks one of the two trees
(INV-09).

## Can it manage casks and Mac App Store apps?

Casks are inventoried, labelled, and **excluded from migration planning** —
MacPorts has no equivalent concept, and a cask can install launch machinery
under a vendor label its metadata does not name, so removing one cleanly is not
something MacPortsBrew can promise. Mac App Store apps and arbitrary `.app`
bundles are out of scope entirely.

## What if the mapping is wrong?

Report it, with a source. The
[package mapping template](../.github/ISSUE_TEMPLATE/package-mapping.yml) asks
for the correct name and **where you read it and when**; with those, a curated
row can be added directly. Without a source it cannot, because the file's
admission rule is that every row was read from the destination provider's own
index on a recorded date.

## What is the "policy version"?

The identifier of the dated provider facts a recommendation was computed from —
`2026.09.19-2`. It is stamped on every recommendation and carried in every
`--json` payload, so an answer is always attributable to a specific set of
facts. It moves independently of the tool version, because the tool changing
and what it believes about a provider changing are different events.

## Why is a date hardcoded in my recommendation?

It is not hardcoded. Every provider fact — support tiers, end dates, bottle
tags, elevation requirements — lives in `data/policy/*.toml` with the source it
was read from and the date it was read (INV-20). No `if` statement in the
program contains September 2027. `macportsbrew doctor` prints the source and
date alongside the claim, and tells you to re-check it.

## Can I add a third package manager?

The architecture permits it: a provider is an adapter plus one registry entry,
and nothing above the adapter layer names a provider. Actually shipping one is
explicitly out of scope for the initial releases. The recipe, if you want to
try, is in [`../CONTRIBUTING.md`](../CONTRIBUTING.md).

## Where is the release?

There is not one yet. The repository will be public on GitHub (DP-007, decided
2026-09-02) but the remote does not exist yet. Release signing is decided —
Developer ID plus notarisation (DP-004, ADR-0005). Build from source — see
[Getting Started](Getting-Started.md).

## Is it affiliated with Homebrew or MacPorts?

No. MacPortsBrew is independent and **not affiliated with, endorsed by, or
sponsored by** either project. It interacts with both only through their
public, supported command-line interfaces: it never vendors, forks, patches or
scrapes them, and never generates load on their infrastructure beyond what your
own commands would.
