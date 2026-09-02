# 02 — Domain model

**Status:** Tier 3 (MUST) / Tier 5 (HYPOTHESIS) as marked.

## Shape

The core is provider-neutral. There is no `migrateFromBrewToMacPorts()`.
There is one planner and one executor parameterised by source and destination.

**HYPOTHESIS — concept names.** `Provider`, `PackageIdentity`,
`InstalledPackage`, `Ownership`, `ProviderCapability`, `PackageMapping`,
`SupportAssessment`, `Recommendation`, `Reason`, `MigrationPlan`,
`MigrationStep`, `VerificationResult`, `Resolution`, `Snapshot`.
Rename freely if the implementation language suggests better names; keep the
distinctions.

## Provider contract

**MUST.** Every provider implements a read contract and, separately, a write
contract. The foundation pass implements only the read contract.

Read:

```
detect()               -> presence, version, prefix, architecture of the install
capabilities()         -> what this provider's installed version can actually do
inventory()            -> every installed row (several per name where the provider allows it)
requested()            -> what the user explicitly asked for (authoritative, not inferred)
dependencies(pkg)
reverseDependencies(pkg)
search(query) / describe(pkg)
artifactAvailability(pkg) -> see below
resolution(executable) -> which provider owns a binary on PATH
environmentInfo()
snapshot()             -> serialisable state for reproducibility and restore
```

Write (later passes): `installPlan/install`, `uninstallPlan/uninstall`,
`cleanupCandidates`, `verify`.

**MUST.** `inventory()` returns installed **rows**, not one row per name. A row
is `(name, version, revision, variant/option set, active?)`, and several rows
may share a name. This is the normal state of a MacPorts machine rather than an
edge case: several versions of a port can be installed at once, exactly one is
active, and `port upgrade` leaves the superseded one installed but inactive by
default (R-3, 2026-09-02). Homebrew's analogue is a rack holding several kegs
with one `linked_keg`; there, reading the first entry of `installed[]` answers
about the *oldest* keg and is wrong (R-1, 2026-09-02). Any "is X installed" or
disk-usage logic written against one row per name is wrong on both providers.

**MUST.** Capabilities are *detected*, not assumed from a version string where
detection is cheap. A provider whose installed version lacks a capability
degrades gracefully with a named reason; it never crashes and never guesses.

## Ownership

**MUST.** Classify every installed package as one of:

`TOP_LEVEL` · `DEPENDENCY` · `ORPHAN_DEPENDENCY` · `UNKNOWN`

**MUST.** Use the provider's own record of user intent where it exists. Do not
infer top-level status from a dependency graph when the provider can answer the
question directly. The authoritative mechanism is named per provider, verified
2026-09-02:

- **Homebrew** — `installed_on_request`, and nothing else. It is present on
  every keg in `brew info --json=v2`. `installed_as_dependency` does **not**
  exist in Homebrew 6.0.21's JSON — it is a legacy receipt field written by
  older majors and must not be read. Casks carry no intent field at all, so a
  cask's ownership is `UNKNOWN`, never inferred (R-1).
- **MacPorts** — the registry `requested` flag, read through the `requested`,
  `unrequested`, `leaves` and `rleaves` pseudo-portnames. It is a flag stored
  at install time, not a graph inference, and it is user-mutable, so it records
  intent *as last asserted* (R-3).

**MUST.** `ORPHAN_DEPENDENCY` means **unrequested with no dependents** — the
`leaves` definition. Homebrew's `brew leaves` is a *different* set and is not a
substitute for user intent: it is "has no remaining dependents", so it includes
packages installed as dependencies and hides packages the user did request. On
one machine the three counts were 47 requested, 49 leaves, 44 both (R-1).
MacPorts' `rleaves` — unrequested and unreachable from any requested port — is
a wider set again; it may be reported, never silently substituted (R-3).

Record which mechanism produced each classification, and its confidence.

**MUST.** Ownership is a fact about a provider, not about the machine. The same
name may be `TOP_LEVEL` in one provider and `DEPENDENCY` in the other.

## Package classes

**MUST.** Beyond ownership, classify packages by what migrating them *means*:

| Class | Meaning | Default handling |
|---|---|---|
| `SIMPLE` | Self-contained tool or library | Normal migration candidate |
| `STATEFUL` | Owns persistent data outside its prefix (databases, indexes, caches the user cares about) | `MANUAL_REVIEW` — INV-19 |
| `SERVICE` | Runs in the background under provider-managed launch machinery | `MANUAL_REVIEW` — INV-19 |
| `RUNTIME` | Language runtime or toolchain with its own ecosystem (`python@X`, `node@X`, `ruby`, `openjdk`, `go`, `rust`) | Consider `USE_RUNTIME_MANAGER` |
| `GUI_APP` | Homebrew cask or equivalent | Out of migration scope; reported, not migrated |
| `THIRD_PARTY_SOURCED` | Installed from a non-core tap or a local Portfile | No mapping assumed; `MANUAL_REVIEW` |
| `PROVIDER_INTERNAL` | Belongs to the provider itself | Never touched |

**MUST.** A package carries a **set** of classes, not one. `php@8.4` is
`RUNTIME` *and* `SERVICE`; `postgresql@17` is `STATEFUL` *and* `SERVICE`
(R-8, 2026-09-02). Default handling is the strictest any member class demands,
under a fixed precedence:

`STATEFUL` · `SERVICE` · `GUI_APP` · `THIRD_PARTY_SOURCED` dominate
`PROVIDER_INTERNAL`, which dominates `RUNTIME`, which dominates `SIMPLE`

so a `RUNTIME` classification can never suppress the `MANUAL_REVIEW` that an
INV-19 class requires of the same package. Every member class is displayed, not
only the winning one.

**MUST.** MacPorts has no concept equivalent to a Homebrew cask (R-1, 2026-09-02,
confirms the cask semantics this relies on, and adds a safety reason: a cask may
plant launch machinery under a vendor label that no cask metadata names). Casks appear in inventory and in `doctor`,
are clearly labelled, and are excluded from migration planning. Do not invent
a mapping for them.

## Artifact availability

**MUST.** Model "will this install as a prebuilt artifact or compile from
source?" **symmetrically for both providers**. The original brief modelled this
as a Homebrew bottle question; it is not. The design assumes MacPorts binary
archives are also unavailable in real cases — non-default variants and
licence-restricted ports compiling locally — and R-4 (2026-09-02) confirms both
with a mechanism: the published archive's filename encodes the canonical variant
set, so any non-default variant names a file that was never built; and only
binary-distributable ports are staged for upload, which the port's own licence
and its whole dependency tree decide.

```
BINARY_AVAILABLE | SOURCE_BUILD_REQUIRED | UNAVAILABLE
| UNSUPPORTED_PLATFORM | DEPRECATED | DISABLED | UNKNOWN
```

**MUST.** `SOURCE_BUILD_REQUIRED` carries a **reason** from a closed set, because
the four causes carry different advice to the user and different durability:

`NO_ARTIFACT_FOR_PLATFORM` · `NON_DEFAULT_OPTIONS` ·
`LICENCE_NOT_REDISTRIBUTABLE` · `FORCED_BY_CONFIGURATION`

`NON_DEFAULT_OPTIONS` and `FORCED_BY_CONFIGURATION` are the user's own choice
and are reversible; `LICENCE_NOT_REDISTRIBUTABLE` never resolves for that
version; `NO_ARTIFACT_FOR_PLATFORM` is the one that moves with provider policy
(R-2, R-4, 2026-09-02).

**MUST.** Two determinations are **not** answerable offline and degrade to
`UNKNOWN` rather than to a negative:

- "no artifact published anywhere" versus "an artifact exists but not for this
  platform". Under Homebrew's default API mode the cached index is generated for
  a single bottle tag, so both states are represented identically as an empty
  `bottle` object; separating them needs the tap on disk or the network
  (R-1, R-2, 2026-09-02).
- `BINARY_AVAILABLE` under MacPorts, ever. Nothing in the local registry, the
  PortIndex or `port` output records whether a published archive exists. Several
  `SOURCE_BUILD_REQUIRED` reasons *are* decidable offline — licence, non-default
  variant, forced configuration, platform exclusion — but the positive is not
  (R-4, 2026-09-02).

**MUST.** The assessment is per `(package, provider, machine, variant/options)`,
carries the evidence that produced it, and degrades to `UNKNOWN` rather than
guessing. `UNKNOWN` is a legitimate, displayable state — never silently
downgraded to a negative or positive.

**MUST.** Determining availability must not mutate anything, must not trigger a
provider auto-update, and must work — with reduced confidence and a clear
marker — with no network.

## Mapping

**MUST.** Never map by name alone. Support `1→1`, `1→many`, `many→1`,
versioned→versioned, and *no equivalent* as a first-class result.

```
PackageMapping {
  source: (provider, package)
  destination: provider
  candidates: [ { package, confidence, evidence[] } ]
  selected: candidate?          // null when confidence is insufficient
  confidence: EXACT | STRONG | PROBABLE | AMBIGUOUS | NONE
}
```

**MUST.** Evidence is recorded and displayable: normalised name, homepage,
upstream repository, description similarity, provided executables, upstream
version agreement, installed file overlap, curated entry. A mapping with no
recorded evidence is a bug.

**MUST.** Two mapping sources: a small **curated** table for the cases
heuristics get wrong, and **discovered** mappings from provider metadata.
Curated entries are versioned data with a rationale. Do not build a large
hardcoded mapping database — curate only what heuristics demonstrably miss, and
record why each entry exists so it can be retired.

**MUST.** `AMBIGUOUS` and `NONE` never produce an automatic action.
They produce `MANUAL_REVIEW` with the candidate list.

**SHOULD.** Versioned families (`python@3.12`, `postgresql@16`, `llvm@18`) are
matched on the family *and* the version constraint. A version mismatch lowers
confidence rather than being ignored.

## Inventory

**MUST.** The unit shown to the user is the *package across providers*, not the
provider row:

```
git
  homebrew:  installed 2.52.0 · TOP_LEVEL · formula · SOURCE_BUILD_REQUIRED · tier3
  macports:  installed 2.52.0 · TOP_LEVEL · BINARY_AVAILABLE · supported
  resolution: /opt/local/bin/git  (macports)
  class: SIMPLE
  recommendation: KEEP_BOTH + PRIMARY_MACPORTS
  reasons: [...]
```

**MUST.** Dependency counts are summarised, never enumerated, in normal mode (INV-13).

## Snapshot

**MUST.** A snapshot is a complete, serialisable, provider-metadata-plus-machine
description sufficient to reproduce an analysis offline. It is:

- the input to `analyze --from-snapshot`, which is how INV-15 is proven;
- the basis of test fixtures — fixtures are captured snapshots, not hand-written;
- the safety net referenced in `04-migration-and-safety.md`;
- the payload of a redacted diagnostic bundle for bug reports.

**MUST.** Snapshots carry a schema version, the tool version, the policy
version, and capture timestamp. Loading a snapshot with an incompatible schema
fails loudly.

## Provider presence states

**MUST.** All four states are first-class and tested; none is an error path.

| State | Behaviour |
|---|---|
| Homebrew only | Inventory and assess Homebrew. Determine this machine's Homebrew support state. Find MacPorts equivalents and their availability. Recommend where useful — after assessing whether MacPorts supports this macOS at all (its own floor is a research item), since on a very old or very new macOS it may not yet. Offer MacPorts installation with consent (INV-05). |
| MacPorts only | Inventory and assess MacPorts. Determine this machine's MacPorts support state. Find Homebrew equivalents and their availability. Recommend where useful — after assessing whether Homebrew is supported *and useful* on this machine, since on an Intel Mac it may be neither. Offer Homebrew installation with consent (INV-05). |
| Both | Full orchestration: overlap, divergence, which implementation currently resolves, conflicts, migration in either direction, coexistence, primary selection. |
| Neither | Explain both providers, assess platform compatibility for each, offer either or both with consent. Never install silently. |

**MUST.** A broken or partially installed provider is its own state, not
"absent". Report it, degrade the analysis honestly, and point at `doctor`.
