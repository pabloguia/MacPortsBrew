# `analysis` schema, version 1

Emitted by `macportsbrew analyze --json`, `macportsbrew status --json` and
`macportsbrew list --json`. All three render the same `Analysis` object; the
human views differ, the payload does not.

Derived from `Analysis` in `crates/application/src/analysis.rs` and the types
it references. The compatibility rule is in [`README.md`](README.md).

## Root

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `1` |
| `toolVersion` | string | The MacPortsBrew version, e.g. `"0.1.0"` |
| `policyVersion` | string | Stamped on every recommendation, e.g. `"2026.10.12-2"` |
| `policyReadOn` | string | `YYYY-MM-DD`; the newest `read_on` across **the files the policy engine loads** — not the whole of `data/policy/`. `provider-commands.toml` is read by the application layer and `provider-prefixes.toml` by the adapters (DP-014), so a read date moving in either does not move this. `policyVersion` *does* span the directory, so the two can differ: a newer read date in one of those other files moves `policyVersion` and not this. The narrower set is deliberate — it drives the staleness notice below. A curated row read from a local index carries `index_as_of`, the index's own last-update date, and counts at that date rather than at the day it was read (DP-043), so reading rows today from an old index does not make older facts look newer |
| `policyStale` | boolean | True when `today` minus `policyReadOn` exceeds the threshold in `data/policy/policy.toml`. Advisory: recommendations are still produced |
| `today` | string | `YYYY-MM-DD`. An input, never a clock read inside the analysis |
| `source` | `AnalysisSource` | `LIVE` or `SNAPSHOT` |
| `snapshotProvenance` | `Provenance`, optional | Present only when `source` is `SNAPSHOT` |
| `concurrentProviderActivity` | `ConcurrentProviderActivity`, defined in [`migrate.v1.md`](migrate.v1.md) | Whether a provider may have been changing the machine while the input was read (`04-migration-and-safety.md`, "Concurrency and locking"; `plan.v1.md`, obligation 5). Decided from `source`, never by looking: `LIVE` says `NOT_CHECKED`, `SNAPSHOT` (a snapshot file or a replayed recording) says `UNKNOWABLE`, and both are `possiblyInconsistent`. The same object reaches `status --json` and `list --json`, which render this payload; their human views do not repeat it, while `analyze`'s ends with it in words. Added to `schemaVersion` 1 after the schema was first documented, without a bump: additive, always written by this build; how to read its absence is in [`migrate.v1.md`](migrate.v1.md#concurrentprovideractivity) |
| `machine` | `Machine` | |
| `providers` | map of provider id to `ProviderReport` | Ordered by provider id |
| `packages` | array of `PackageReport` | Ordered by package name |
| `summary` | `Summary` | |

## `Machine`

| Field | Type | Notes |
|---|---|---|
| `architecture` | string | `x86_64` or `arm64`. `snake_case`, matching how both providers spell it |
| `macos` | object | `{ "major": int, "minor": int, "patch": int }`. Before macOS 11 the release is `10.x`; from 11 on it is the major |
| `translation` | string | `NATIVE`, `TRANSLATED` (running under Rosetta), `UNKNOWN` |
| `commandLineTools` | tagged union, tag `state` | `INSTALLED` (with `path`), `MISSING`, `UNKNOWN` |

## `ProviderReport`

| Field | Type | Notes |
|---|---|---|
| `detection` | `ProviderDetection` | |
| `capabilities` | `ProviderCapabilities` | |
| `support` | `SupportState` | |
| `packageCount` | integer | Every installed package |
| `topLevelCount` | integer | Those the provider's own record says the user asked for |
| `guiAppCount` | integer | Casks and equivalents. Reported, never migrated |

### `ProviderDetection`

| Field | Type | Notes |
|---|---|---|
| `provider` | string | The provider id, e.g. `"homebrew"` |
| `presence` | tagged union, tag `state` | `PRESENT`, `ABSENT`, `BROKEN` (with a reason) |
| `version` | string, nullable | As the provider reports it |
| `prefix` | string, nullable | The detected prefix, e.g. `"/usr/local"` |
| `executable` | string, nullable | Resolved from the prefix, not from `PATH` |
| `architecture` | string, nullable | The architecture the provider itself is built for |

### `ProviderCapabilities`

`{ "capabilities": { <Capability>: <CapabilityState> } }`.

Capability keys: `MACHINE_READABLE_INVENTORY`, `REQUESTED_RECORD`,
`DEPENDENCIES`, `REVERSE_DEPENDENCIES`, `ARTIFACT_AVAILABILITY`,
`OFFLINE_METADATA`, `FILE_OWNERSHIP`, `DEPRECATION_STATE`, `SERVICE_METADATA`,
`UNPRIVILEGED_READ`.

`UNPRIVILEGED_READ` is the one that feeds a decision rather than a report:
`AVAILABLE` means the provider's own inventory came back while running as the
invoking user, which is proof its read operations need no administrator rights,
and the planner prefers it over the stored elevation in
`data/policy/provider-commands.toml` for read steps only (INV-20: detection wins
over stored knowledge). It is never `UNAVAILABLE` — a read that failed does not
say whether it failed for want of permission or for want of a binary — so it can
confirm that no elevation is needed and can never assert that one is.

`CapabilityState` is a tagged union on `state`: `AVAILABLE`, `UNAVAILABLE`
(with a reason), `UNKNOWN`. **`UNKNOWN` is a first-class answer** and never
means "assume available" — a consumer that treats it as available is reading
the schema wrong.

**Every provider reports every key**, whether or not it is installed. An
absent provider's capabilities are `UNKNOWN` throughout — it cannot be asked what it
can do, and that is what `UNKNOWN` means. An analysis of a *snapshot document*
reports keys the document did not carry for the same reason: they are recorded
as `UNKNOWN` when it loads, because a capture writes every answer and a
document that omits one does not say what answer it would have given. The
reasoning and the case it closes are in
[`snapshot.v1.md`](snapshot.v1.md) (SE-084).

### `SupportState`

| Field | Type | Notes |
|---|---|---|
| `tier` | string | `SUPPORTED`, `DEGRADED`, `UNSUPPORTED`, `UNKNOWN` |
| `providerLabel` | string, optional | The provider's own name for the state, e.g. `"tier3"` |
| `endDate` | string, optional | `YYYY-MM-DD`; a stated date after which support ends |
| `prebuiltArtifacts` | boolean, optional | Whether the provider publishes prebuilt artifacts for this machine at all. Per-package availability is detected separately and always wins |
| `fact` | object | `{ "source": string, "readOn": "YYYY-MM-DD" }`. **Both are mandatory** — this is INV-20's dated-data guarantee rendered into the payload |
| `entry` | string | The identifier of the row in `data/policy/provider-support.toml` this came from |

## `PackageReport`

The unit is the *package across providers*, not the provider row.

| Field | Type | Notes |
|---|---|---|
| `name` | string | The user-facing name, on the source side |
| `source` | string | The provider id currently holding it |
| `rows` | map of provider id to `ProviderRow` | One row per provider that has it installed or offers a mapping candidate |
| `class` | `ClassSet` | |
| `resolution` | `Resolution`, optional | Which executable actually runs, when the package provides one |
| `mappings` | array of `PackageMapping` | |
| `recommendation` | `Recommendation` | |
| `advancedOnly` | boolean | True for dependency-level packages, hidden unless `--advanced` (INV-13) |
| `ownershipSplitDefaulted` | array of provider ids, optional | The providers whose `DEPENDENCY` answer for this package was a **default**: the provider's own record says nobody asked for it, but whether anything installed still depends on it could not be read, so the dependency/orphan split was not answered and `DEPENDENCY` is the cautious label, not a finding. Omitted when every answer was read. Kept beside `rows` rather than inside `ownershipEvidence` so no existing shape changes (added 2026-10-12, pass 12; `schemaVersion` unchanged). `explain` says it in words |

### `ProviderRow`

| Field | Type | Notes |
|---|---|---|
| `name` | string | The name under *this* provider, which may differ from the unit's name |
| `installed` | array of `InstalledPackage` | More than one entry means several versions are installed |
| `ownership` | string | `TOP_LEVEL`, `DEPENDENCY`, `ORPHAN`, `UNKNOWN` |
| `ownershipEvidence` | object | `{ "mechanism": string, "authoritative": boolean }`. The mechanism is the provider's own, e.g. `"installed_on_request"` or `"requested"` |
| `availableVersion` | string, optional | What the provider currently offers |
| `availability` | `AvailabilityAssessment` | |
| `deprecated` | boolean | |
| `disabled` | boolean | No longer obtainable |
| `dependencyCount` | integer | Summarised, never enumerated in normal mode (INV-13) |
| `reverseDependencyCount` | integer | **Within this provider only** (INV-04) |
| `presence` | string | `INSTALLED`, or `MAPPED` when the row is a mapping target rather than an installation |

### `InstalledPackage`

| Field | Type | Notes |
|---|---|---|
| `identity` | object | `{ "provider": string, "name": string }` |
| `kind` | string | `PACKAGE` or `GUI_APP` |
| `version` | string | As the provider spells it |
| `revision` | string, optional | |
| `variants` | array of string, optional | MacPorts variants |
| `active` | boolean | |
| `requested` | boolean, optional | The provider's own record of user intent. For a formula or a port it comes from the read interface; for a cask the JSON carries no such field, so it comes from the cask's on-disk receipt (R-12; DP-012). Absent where no record was found — under `--replay` that is every cask, because a recording carries no receipt. Absence is never an inference |
| `installedFromPrebuilt` | boolean, optional | |
| `installPath` | string, optional | |

### `AvailabilityAssessment`

| Field | Type | Notes |
|---|---|---|
| `availability` | tagged union, tag `state` | See below |
| `evidence` | array | `[{ "source": string, "detail": string, "offline": boolean }]`. **An availability or mapping judgement with no recorded evidence is a bug** |
| `cached` | boolean | Whether this came from a cache rather than a fresh read |

`state` values: `BINARY_AVAILABLE`; `SOURCE_BUILD_REQUIRED` (with a `reason` of
`NO_ARTIFACT_FOR_THIS_PLATFORM`, `NON_DEFAULT_OPTIONS`, `LICENCE_RESTRICTED` or
`FORCED_BY_CONFIGURATION`); `UNAVAILABLE`; `UNSUPPORTED_PLATFORM`;
`DEPRECATED`; `DISABLED`; `UNKNOWN`.

`UNKNOWN` is what an offline run reports for a fact that needs the network. It
is never silently a positive.

### `ClassSet`

`{ "classes": [PackageClass], "evidence": [{ "class", "rule", "detail" }] }`.

A package carries a **set** of classes, not one. Class values: `SIMPLE`,
`STATEFUL`, `SERVICE`, `RUNTIME`, `GUI_APP`, `THIRD_PARTY_SOURCED`,
`PROVIDER_INTERNAL`. Default handling is the strictest any member demands, so a
`RUNTIME` classification can never suppress the `MANUAL_REVIEW` an INV-19 class
requires. Every member is reported, not only the dominant one.

### `Resolution`

| Field | Type | Notes |
|---|---|---|
| `executable` | string | |
| `path` | string, optional | What `PATH` resolves to |
| `canonicalPath` | string, optional | After following symlinks |
| `owner` | tagged union, tag `owner` | `SYSTEM`, `PROVIDER` (with the provider id and optionally the package), `MACPORTSBREW_SHIM`, `EXTERNAL`, `UNKNOWN` |
| `authoritative` | boolean | False when ownership came from prefix matching rather than from asking the provider. Users relocate prefixes, so the fallback is marked as one |

### `PackageMapping`

| Field | Type | Notes |
|---|---|---|
| `source` | object | `{ "provider", "name" }` |
| `destination` | string | Destination provider id |
| `candidates` | array | `[{ "package": {provider,name}, "confidence": Confidence, "evidence": [{ "kind", "detail", "weight" }] }]` |
| `selected` | candidate, optional | Absent when the confidence is insufficient |
| `confidence` | string | `NONE`, `AMBIGUOUS`, `PROBABLE`, `STRONG`, `EXACT` |

One-to-one, one-to-many, many-to-one and *no equivalent* are all first-class
results. `AMBIGUOUS` and `NONE` never produce an automatic action: they produce
`MANUAL_REVIEW` with the candidate list.

### `Recommendation`

| Field | Type | Notes |
|---|---|---|
| `outcome` | tagged union, tag `kind` | See below |
| `reasons` | array of `Reason` | Each carries the `rule` that emitted it. The `confidence` and `threshold` parameters of `reason.mapping_confidence_below_threshold` are `Confidence` identifiers; the planner's `class` and `state` parameters are known exceptions ([README](README.md), SE-064) |
| `rulesNotFired` | array of string, optional | Rule ids considered that did not fire. Sorted |
| `policyVersion` | string | |

`kind` values: `KEEP` (with `provider`), `MIGRATE_TO` (with `provider`),
`KEEP_BOTH` (with `primary`), `USE_RUNTIME_MANAGER`, `MANUAL_REVIEW`,
`NO_ACTION`.

`MANUAL_REVIEW` cannot be outvoted by any other outcome, and the engine
enforces that by outcome rather than by priority number (INV-19). `KEEP_BOTH`
is a supported end state, not a failure to decide (INV-18).

## `Summary`

| Field | Type | Notes |
|---|---|---|
| `topLevelUnits` | integer | Units shown in normal mode: ownership `TOP_LEVEL` **or** `UNKNOWN`. Wider than `topLevelCount`, which counts only what a provider's own record claims (DP-012) |
| `dependencyUnits` | integer | |
| `byOutcome` | map of outcome name to integer | Ordered by key |
| `manualReview` | integer | |

## `Provenance`

Present as `snapshotProvenance` when the analysis came from a file.

| Field | Type | Notes |
|---|---|---|
| `capturedAt` | string | |
| `capturedBy` | string | The tool version that captured it |
| `origin` | string | `capture` for a live capture, `--replay` for a recording |
| `note` | string, optional | |

## Worked example

`macportsbrew status --json` on an Intel Mac with Homebrew and no MacPorts,
abridged to the shape rather than the whole package list:

**Abridged, and therefore unchecked** — deliberately, and said here rather than
left to be discovered. Every other JSON sample under `docs/schemas/` carries a
`<!-- sample: ... -->` marker and is compared with the binary on every run
(F-50). This one cannot be: `packages` is emptied and `commandLineTools.path` is
a literal `"..."`, so no byte comparison can pass. Closing it means regenerating
from a recording with a small enough inventory to print whole, or teaching the
scan a key-structure-only comparison; until one of those happens this block is
maintained by hand and may drift, which is exactly the condition F-27 was built
to end.

```json
{
  "schemaVersion": 1,
  "toolVersion": "0.1.0",
  "policyVersion": "2026.10.12-2",
  "policyReadOn": "2026-09-04",
  "policyStale": false,
  "today": "2026-09-04",
  "source": "LIVE",
  "concurrentProviderActivity": { "activity": "NOT_CHECKED", "possiblyInconsistent": true },
  "machine": {
    "architecture": "x86_64",
    "macos": { "major": 26, "minor": 6, "patch": 2 },
    "translation": "NATIVE",
    "commandLineTools": { "state": "INSTALLED", "path": "..." }
  },
  "providers": {
    "homebrew": {
      "detection": {
        "provider": "homebrew",
        "presence": { "state": "PRESENT" },
        "version": "6.0.21",
        "prefix": "/usr/local",
        "architecture": "x86_64"
      },
      "capabilities": { "capabilities": { "FILE_OWNERSHIP": { "state": "UNKNOWN" } } },
      "support": {
        "tier": "DEGRADED",
        "providerLabel": "tier3",
        "endDate": "2027-09-01",
        "fact": { "source": "https://docs.brew.sh/Support-Tiers", "readOn": "2026-09-02" },
        "entry": "homebrew-intel-tier3"
      },
      "packageCount": 189,
      "topLevelCount": 47,
      "guiAppCount": 0
    }
  },
  "packages": [],
  "summary": { "topLevelUnits": 56, "dependencyUnits": 133, "byOutcome": {}, "manualReview": 15 }
}
```

The `endDate` above is a **dated fact carried as data**, not a hardcoded
deadline: it comes from `data/policy/provider-support.toml` with the `source`
and `readOn` shown beside it, and a consumer should re-check the source rather
than trust the date indefinitely (INV-20).

## Not in this version

- **A concurrency check** (`plan.v1.md`, obligation 5; SE-015). The flag is
  here — `concurrentProviderActivity`, above — and it reports that no check was
  made, never what one would have found: this build has no detection
  mechanism, so it never writes `NONE_OBSERVED` or `OBSERVED`.
