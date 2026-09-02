# `explain` schema, version 1

Emitted by `macportsbrew explain <package> --json`. Derived from `Explanation`
in `crates/application/src/explain.rs` and `Derivation` in
`crates/policy/src/engine.rs`. The compatibility rule is in
[`README.md`](README.md).

This is the explainability surface: **every recommendation can be reproduced
from what this payload contains.** It reports the facts the engine read, the
rules that fired, the rules that were considered and did not, and the policy
version that produced the answer. A rule that did not fire is a first-class
result, not an omission — "why did it *not* recommend X" is answerable from
here.

## Root

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `1`. Shares the analysis schema version, because `package` is an analysis object |
| `package` | `PackageReport` | The full unit, exactly as in [`analysis.v1.md`](analysis.v1.md) |
| `facts` | `Facts` | The engine's entire input, besides the policy data |
| `derivation` | `Derivation` | |

## `Facts`

The complete input to `recommend(facts, data)`. There is nothing else: no I/O,
no clock, no environment (INV-14).

| Field | Type | Notes |
|---|---|---|
| `machine` | `Machine` | See [`analysis.v1.md`](analysis.v1.md) |
| `today` | string | `YYYY-MM-DD`. **An input, never a clock read** — which is what makes the same snapshot give the same answer forever (INV-15) |
| `policyVersion` | string | |
| `providers` | map of provider id to `ProviderFacts` | |
| `package` | `PackageFacts` | |

### `ProviderFacts`

| Field | Type | Notes |
|---|---|---|
| `presence` | tagged union, tag `state` | `PRESENT`, `ABSENT`, `BROKEN` |
| `version` | string, nullable | |
| `prefix` | string, nullable | |
| `support` | `SupportState` | Resolved from the policy data or detected at run time; **detection wins** (INV-20) |
| `healthy` | boolean | Whether the provider answered its own health check |

### `PackageFacts`

| Field | Type | Notes |
|---|---|---|
| `name` | string | On the source side |
| `source` | string | The provider holding it |
| `ownership` | string | `TOP_LEVEL`, `DEPENDENCY`, `ORPHAN`, `UNKNOWN` |
| `classes` | `ClassSet` | An empty set means a plain, self-contained package |
| `installedVersion` | string, nullable | |
| `perProvider` | map of provider id to `PackageProviderFacts` | **The source and every mapping candidate use the same map**, so neither direction is a special case (INV-17) |
| `reverseDependencyCount` | integer | **Within the source provider only.** Never counted across providers (INV-04) |
| `coexistenceRequested` | boolean | Whether the user asked for it under both |

### `PackageProviderFacts`

| Field | Type | Notes |
|---|---|---|
| `installed` | boolean | |
| `availableVersion` | string, nullable | |
| `availability` | `AvailabilityAssessment` | Whether adding it here would use a prebuilt artifact or compile |
| `deprecated` | boolean | |
| `disabled` | boolean | |
| `replacedBy` | string, nullable | |
| `mappingConfidence` | string | `NONE`, `AMBIGUOUS`, `PROBABLE`, `STRONG`, `EXACT`. `EXACT` for the source itself, because the source is not a mapping |
| `mappedName` | string, nullable | The name under this provider, when known |

## `Derivation`

| Field | Type | Notes |
|---|---|---|
| `factsUsed` | array of `[name, value]` pairs | Sorted by name. Every fact the engine actually read, flattened to strings |
| `fired` | array of `FiredRule` | In rule-identifier order |
| `notFired` | array of string | Rule ids considered that did not fire. Sorted |
| `recommendation` | `Recommendation` | The result. See [`analysis.v1.md`](analysis.v1.md) |
| `policyVersion` | string | |

### `FiredRule`

| Field | Type | Notes |
|---|---|---|
| `rule` | string | The rule id, `SCREAMING_SNAKE_CASE` |
| `outcome` | `Outcome` | What *this rule* would have the system do, which is not necessarily the final answer |
| `reason` | `Reason` | |

Several rules can fire with different outcomes. The engine resolves them by
priority, breaking ties by rule identifier so the answer never depends on
declaration order (INV-15), and **nothing outranks `MANUAL_REVIEW`** — that is
enforced by outcome, not by priority number (INV-19). `fired` shows all of
them, so a reader can see what was overruled and why.

## Worked example

`macportsbrew explain curl --json`, abridged — the real payload carries every
fact and every rule the engine knows, and the roster grows as rules are added.
Three rules fired here with three different outcomes; `MANUAL_REVIEW` is the
answer:

```json
{
  "schemaVersion": 1,
  "package": { "name": "curl", "source": "homebrew" },
  "facts": {
    "today": "2026-09-02",
    "policyVersion": "2026.09.02-1",
    "providers": {
      "homebrew": {
        "presence": { "state": "PRESENT" },
        "version": "6.0.21",
        "prefix": "/usr/local",
        "support": {
          "tier": "DEGRADED",
          "endDate": "2027-09-01",
          "fact": { "source": "https://docs.brew.sh/Support-Tiers", "readOn": "2026-09-02" }
        },
        "healthy": true
      },
      "macports": { "presence": { "state": "ABSENT" }, "healthy": false }
    },
    "package": {
      "name": "curl",
      "source": "homebrew",
      "ownership": "TOP_LEVEL",
      "classes": { "classes": [], "evidence": [] },
      "installedVersion": "8.21.0",
      "perProvider": {
        "homebrew": { "installed": true, "mappingConfidence": "EXACT" },
        "macports": {
          "installed": false,
          "availability": { "availability": { "state": "UNKNOWN" },
            "evidence": [{ "source": "mapping", "detail": "no equivalent identified", "offline": true }],
            "cached": false },
          "mappingConfidence": "NONE",
          "mappedName": null
        }
      },
      "reverseDependencyCount": 2,
      "coexistenceRequested": false
    }
  },
  "derivation": {
    "factsUsed": [
      ["machine.architecture", "x86_64"],
      ["machine.macos", "26.6.2"],
      ["package.reverseDependencyCount", "2"],
      ["provider.homebrew.supportEndDate", "2027-09-01"]
    ],
    "fired": [
      { "rule": "KEEP_SOURCE_DEFAULT",
        "outcome": { "kind": "KEEP", "provider": "homebrew" },
        "reason": { "id": "reason.keep_source_default", "params": { "package": "curl", "provider": "homebrew" } } },
      { "rule": "MANUAL_REVIEW_REVERSE_DEPENDENCIES",
        "outcome": { "kind": "MANUAL_REVIEW" },
        "reason": { "id": "reason.reverse_dependencies_present", "params": { "count": "2", "provider": "homebrew" } } },
      { "rule": "NO_ACTION_ALREADY_THERE",
        "outcome": { "kind": "NO_ACTION" },
        "reason": { "id": "reason.no_alternative_provider_present", "params": { "package": "curl", "provider": "homebrew" } } }
    ],
    "notFired": [
      "DESTINATION_UNSUPPORTED_HERE",
      "KEEP_BOTH_HEALTHY",
      "MANUAL_REVIEW_MAPPING_CONFIDENCE",
      "SOURCE_SUPPORT_ENDING"
    ],
    "recommendation": {
      "outcome": { "kind": "MANUAL_REVIEW" },
      "rulesNotFired": ["DESTINATION_UNSUPPORTED_HERE"],
      "policyVersion": "2026.09.02-1"
    },
    "policyVersion": "2026.09.02-1"
  }
}
```

`SOURCE_SUPPORT_ENDING` is in `notFired` even though `homebrew` here has a
stated end date, and the payload shows why: the rule also requires a strong
equivalent with a prebuilt artifact on the other side, and `macports` is absent
with a `NONE` mapping. That is the kind of question this schema exists to
answer without reading the source.

## Rendering the reasons

Every `Reason` is `{ id, params?, rule? }` with no display text. English
templates live in `locales/en.toml`, keyed by the same identifiers, with a
`{placeholder}` per parameter. A consumer either renders through that catalogue
or displays the identifier; **the identifiers themselves are never localised**,
regardless of `--language`.
