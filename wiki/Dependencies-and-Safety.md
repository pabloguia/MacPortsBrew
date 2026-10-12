# Dependencies and Safety

Most of what a package manager has installed is not what you asked for. It is
what your requests dragged in. On the machine these examples come from,
Homebrew had 189 packages, **47 of them top-level**; most of the rest are
dependencies. The default view shows 56 rows, because packages whose ownership
is unknown are shown too.

Understanding that split is most of understanding why MacPortsBrew refuses to
do things.

## Top-level, dependency, orphan

MacPortsBrew classifies every installed package by **why it is there**, using
the provider's own record of your intent rather than inferring it from a graph:

| Ownership | Meaning |
|---|---|
| **Top-level** | You asked for it. |
| **Dependency** | A provider installed it on behalf of something you asked for. When the provider could not say what depends on it, a package nobody asked for is still shown as a dependency, never as an orphan, and `explain` says that was a default. |
| **Orphan** | Unrequested, and nothing depends on it any more. |
| **Unknown** | The provider has no record of intent for this one, or the row is only a counterpart on the other provider that is not installed. |

The mechanisms are named and recorded, not guessed. Homebrew's is
`installed_on_request`; MacPorts' is the registry `requested` flag, read
through its `requested`, `unrequested`, `leaves` and `rleaves` pseudo-portnames
([`../docs/research/R-1-homebrew-read-interfaces.md`](../docs/research/R-1-homebrew-read-interfaces.md)
and
[`../docs/research/R-3-macports-read-interfaces.md`](../docs/research/R-3-macports-read-interfaces.md),
both 2026-09-02).

Two subtleties MacPortsBrew is careful about:

- **`brew leaves` is not the same set as "what you asked for".** It means "has
  no remaining dependents", so it includes things installed as dependencies and
  hides things you did request. On one Intel Mac running Homebrew 6.0.21, read
  on 2026-09-02: 47 requested, 49 leaves, 44 both
  ([`R-1`](../docs/research/R-1-homebrew-read-interfaces.md)). MacPortsBrew never substitutes one for the other.
- **Ownership is a fact about a provider, not about your machine.** The same
  name can be top-level under one provider and a dependency under the other.

## Normal mode hides dependencies, deliberately

`macportsbrew analyze` shows top-level packages, and any whose ownership is
unknown. Dependencies are managed
automatically by their provider, and presenting them as things you might move
individually invites exactly the mistake this tool exists to prevent.

```sh
macportsbrew analyze --advanced   # include them
```

Entering advanced mode displays a warning saying what those packages are. In
advanced mode the safety checks do not relax: reverse dependencies are still
checked and dependencies are still never preselected. When execution lands,
destructive actions in advanced mode will require a stronger confirmation than
in normal mode.

## Reverse dependencies block removals

Before anything is removed, MacPortsBrew resolves what **inside that same
provider** depends on it. If removal would break them, it is blocked by default
and explained — here as it appears in `plan curl --to macports`, replayed from
the committed Intel recording:

```
  ! 2 packages under homebrew depend on this one, and removing or moving it on its own can break them.
```

Two things about that check:

**It is within one provider only.** Homebrew's dependents are Homebrew's
business and MacPorts' are MacPorts'. Counting across providers would be
meaningless, because neither provider's package can satisfy the other's
dependency in the first place.

**Counts are summarised, never enumerated** in normal mode. "2 packages depend
on this" is the useful fact; a list of 40 names is noise you would scroll past.

## Classes: what migrating a package actually means

Beyond ownership, packages are classified by what moving them would involve. A
package carries a **set** of classes, not one — `php@8.4` is a runtime *and* a
service; `postgresql@17` holds data *and* is a service.

| Class | What it is | Default |
|---|---|---|
| **Simple** | A self-contained tool or library | Normal migration candidate |
| **Holds data** | Owns persistent data outside its own prefix — databases, indexes, caches you care about | **Needs a human decision** |
| **Background service** | Runs under provider-managed launch machinery | **Needs a human decision** |
| **Language runtime** | Python, Node, Ruby, Java, Go, Rust and friends | Suggest a version manager |
| **GUI app** | A Homebrew cask or equivalent | Reported, never migrated |
| **Third-party sourced** | From a non-core tap or a local Portfile | No mapping assumed |
| **Provider internal** | Belongs to the provider itself | Never touched |

**The strictest class wins.** A package that is both a runtime and a service is
handled as a service, because the runtime handling can never be allowed to
suppress a review that a data-owning or service class requires. Every class is
displayed, not only the winning one.

## Why "holds data" is never migrated automatically

Because the data is not in the prefix. A database's cluster, an index, a cache
you have spent a week warming — these live outside anything the provider will
move or copy, and the two providers lay them out differently. Reinstalling the
package under the other provider gives you the software and not the state.

MacPortsBrew will not do that unattended, in either direction, ever. From
`plan postgresql17-server --to macports`, replayed from the committed Apple
Silicon recording:

```
  ! postgresql17-server is classed as Holds data, and packages in that class are never migrated automatically. A person decides this one.
```

Background services are the same argument in a different shape: they are
registered with provider-managed launch machinery, and a migration that leaves
a stale registration behind produces a service that either does not start or
starts the wrong copy.

## Language runtimes

MacPortsBrew will generally tell you to use a version manager rather than
either provider:

```
Package             Provider    Version      Ownership  Availability                                                Class               Recommendation
…
python@3.12         homebrew    3.12.14      Requested  Prebuilt                                                    Language runtime    Use a language runtime manager instead
…
```

(One row of `analyze`, replayed from the committed Intel recording; `…` marks
rows left out.)

That is a recommendation, not an action. MacPortsBrew does not manage runtimes,
does not install version managers, and will not migrate your Python for you.
Both providers can host a runtime perfectly well; a version manager is usually
a better fit for the way runtimes are actually used, and saying so is more
useful than picking a provider.

## What is guaranteed, and what is not

Behind all of the above are rules the project holds itself to, each with a
test (the list, for the curious, is
[`../docs/spec/01-invariants.md`](../docs/spec/01-invariants.md)). In 0.1.0
nothing executes, so the ones about running a plan describe the plans it
prints and the executor a later release will add:

- **Nothing is removed before its replacement is installed and verified.**
- **Every destructive action comes from a plan a human has seen.**
  Nothing is derived at execution time.
- **Failure preserves the prior working state *whenever technically possible***
  — and the qualifier is part of the rule, not a hedge on it.
  Partial success is reported as partial success, never as success.
- **Reverse dependencies are checked before removal.**
- **A package manager is never installed implicitly.** This build never
  installs a package manager; install it yourself first.
- **Nothing needs elevation to analyse, inventory, map, plan or dry-run.**
- **Packages that own data or run as services are never migrated blind.**

**What is not guaranteed: undoing a provider's own half-finished write.**
MacPortsBrew will not promise what it cannot deliver, and neither provider
documents transactional install or uninstall. MacPorts' `uninstall` is
documented as two actions in one — *"Deactivate and uninstall portname"*
([`port(1)`](https://guide.macports.org/#using.port), read 2026-09-02; recorded
in [`R-5`](../docs/research/R-5-privilege-requirements.md)) — so a run that dies
between them leaves a port deactivated but present. We have found no equivalent
statement either way for Homebrew, so treat a `brew link` or `brew uninstall`
interrupted partway as recoverable only by hand until something says otherwise.

So *"left at least as usable as before"* is what the plan is **ordered** to
achieve — nothing is removed before its replacement is installed and verified,
which is the ordering the planner enforces — and not a claim that any failure can be
rewound. Where a step cannot be recovered from, you are told which step and what
state it left, which is the difference between a tool you can act on and one
that says it succeeded.

## See also

- [Package Mapping](Package-Mapping.md) — the other half of whether a move is
  safe.
- [Restoring After a Migration](Restoring-After-a-Migration.md) — taking your
  own snapshot first, and the restore design.
