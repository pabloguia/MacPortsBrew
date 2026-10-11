# Package Mapping

Before MacPortsBrew can say anything about moving a package, it has to know
what that package is called on the other side. That is **mapping**, and it is
harder than it looks.

## Names are not enough

`postgresql@17` under Homebrew is not `postgresql@17` under MacPorts. It is not
even one port: MacPorts splits it into `postgresql17` and
`postgresql17-server`, and only the `-server` half carries the startup item and
the data cluster. A name heuristic normalises `postgresql@17` to `postgresql17`,
stops there, and silently drops the half that holds your data.

So **MacPortsBrew never maps by name alone**, and it supports every shape the
real world produces:

| Shape | Example |
|---|---|
| one to one | the ordinary case |
| one to many | `postgresql@17` becoming two MacPorts ports |
| many to one | two formulae the other provider ships as a single port |
| versioned to versioned | `python@3.12`, matched on family *and* version |
| **no equivalent** | a first-class result, not a failure |

## Confidence, and what each level allows

Every mapping carries a confidence:

| Confidence | Meaning |
|---|---|
| **Exact** | A curated entry with a citation, or a match nothing else could explain. |
| **Strong** | Multiple independent pieces of evidence agree. |
| **Probable** | Evidence agrees, but thinly. |
| **Ambiguous** | Several candidates, none clearly right. |
| **None** | No equivalent identified. |

**Acting needs Strong or better.** `Probable`, `Ambiguous` and `None` never
produce an automatic action; a plan that depends on one stops at "needs a human
decision". `macportsbrew explain <package>` lists the candidates that were
considered, each with its confidence and the evidence for it, so you can look
at what was weighed. In a plan the stop appears as the line below, copied from
`plan curl --to macports` replayed from the committed Intel recording (the
whole plan is on [Homebrew to MacPorts](Homebrew-to-MacPorts-Migration.md#build-the-plan)):

```
  ! No equivalent under macports reaches confidence level Strong: the best is at level None. Check whether macports has an equivalent before deciding.
```

## Evidence

Every candidate records why it is a candidate. A mapping with no recorded
evidence is a bug, not a shortcut. In 0.1.0 the evidence kinds are: a curated
entry, the normalised name, a versioned family (`python@3.12` and `python312`),
the homepage, and the set of executables each package provides — each compared
against the packages the destination provider **already has installed**. The
upstream repository, description similarity, upstream version agreement and
installed file overlap are designed as further evidence but are not built in
this version.

You can read all of it:

```sh
macportsbrew explain <package>
```

The "Mappings and their evidence" section lists each candidate, its confidence
and the evidence behind it. When there is nothing, it says so:

```
Mappings and their evidence
  -> macports: None
```

That is an honest answer. It does not mean no equivalent exists in the world;
it means MacPortsBrew found no evidence good enough to act on, which is a
different and more useful statement. In 0.1.0 it most often means the other
provider simply does not have the package installed: discovery compares only
against what is already installed there, and searching the other provider's
full catalogue is planned for a later release.

## Where mappings come from

**Discovered** mappings compare a package against the destination provider's
**installed** packages only, using the normalised name, the versioned family,
the homepage and the executables each provides. A package the destination has
not installed cannot be discovered in 0.1.0; it needs a curated row, or it stops
at "needs a human decision".

**Curated** mappings are a small table for the cases the heuristics
demonstrably get wrong — `data/policy/curated-mappings.toml`. Every row carries
where both names were read and the date they were read, plus a rationale
explaining why a heuristic does not get there. That last field matters: it is
what lets a row be retired when the heuristics improve, instead of accumulating
forever.

The admission rule for that file is strict: **a row exists only when the
destination name was read from the destination provider's own index on the
recorded date.** Candidates that could not be confirmed were dropped rather
than guessed.

Entries are written in one direction, because a citation has a direction — you
read a name from one provider on one day. The loader builds the reverse index
from the same rows, so both directions are answerable and neither provider is a
special case.

## Casks are not mapped

MacPorts has no cask format. It does ship GUI applications as ordinary ports —
1332 in its `aqua` category — but a port with the same name as a cask is often a
different version, a different vendor's build, or installs to a different place
([`../docs/research/R-12-cask-migratability.md`](../docs/research/R-12-cask-migratability.md),
cited 2026-09-04). Casks appear in your inventory and in `doctor`, are clearly
labelled, and are **excluded from migration planning**. MacPortsBrew does not
invent a mapping for them.

There is a safety reason as well: a cask may install launch machinery under a
vendor label that no cask metadata names, so removing one cleanly is not a
thing MacPortsBrew can promise
([`../docs/research/R-1-homebrew-read-interfaces.md`](../docs/research/R-1-homebrew-read-interfaces.md),
2026-09-02).

## Versioned families

`python@3.12`, `postgresql@16`, `llvm@18` are matched on the family **and** the
version. A version mismatch lowers the confidence rather than being ignored —
mapping `python@3.12` to a `python313` port is not a mapping, it is a different
package.

## When the mapping is wrong

That is a reportable bug and often an easy fix. The
[package mapping issue template](../.github/ISSUE_TEMPLATE/package-mapping.yml)
asks for the `explain` output, the correct name if you know it, and **where you
read that name and when**. With a source and a date, a curated row can be added
directly; without one, it cannot.

If you want to add it yourself, the recipe is in
[`../CONTRIBUTING.md`](../CONTRIBUTING.md) under "Adding a package mapping".

## See also

- [Dependencies and Safety](Dependencies-and-Safety.md) — a good mapping is not
  on its own enough to make a move safe.
- [Understanding Bottles and Source Builds](Understanding-Bottles-and-Source-Builds.md)
  — availability is judged per mapped package.
