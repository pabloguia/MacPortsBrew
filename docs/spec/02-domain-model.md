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
inventory()            -> everything installed
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

**MUST.** Capabilities are *detected*, not assumed from a version string where
detection is cheap. A provider whose installed version lacks a capability
degrades gracefully with a named reason; it never crashes and never guesses.

## Ownership

**MUST.** Classify every installed package as one of:

`TOP_LEVEL` · `DEPENDENCY` · `ORPHAN_DEPENDENCY` · `UNKNOWN`

**MUST.** Use the provider's own record of user intent where it exists. Do not
infer top-level status from a dependency graph when the provider can answer the
question directly. Research the current authoritative mechanism for each
provider (see `09-research-checklist.md`); record which mechanism was used and
its confidence.

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

**MUST.** MacPorts has no concept equivalent to a Homebrew cask (R-1 confirms
the cask semantics this relies on). Casks appear in inventory and in `doctor`,
are clearly labelled, and are excluded from migration planning. Do not invent
a mapping for them.

## Artifact availability

**MUST.** Model "will this install as a prebuilt artifact or compile from
source?" **symmetrically for both providers**. The original brief modelled this
as a Homebrew bottle question; it is not. The design assumes MacPorts binary
archives are also unavailable in real cases — non-default variants and
licence-restricted ports compiling locally — and R-4 verifies which cases
those are.

```
BINARY_AVAILABLE | SOURCE_BUILD_REQUIRED | UNAVAILABLE
| UNSUPPORTED_PLATFORM | DEPRECATED | DISABLED | UNKNOWN
```

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
