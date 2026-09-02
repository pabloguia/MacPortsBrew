# `snapshot` schema, version 1

The file written by `macportsbrew snapshot [--out <file>]`, and read back by
`--from-snapshot <file>`. Derived from `Snapshot` in
`crates/domain/src/snapshot.rs`. The compatibility rule is in
[`README.md`](README.md).

A snapshot is the whole input to an analysis. **`analyze --from-snapshot` runs
the same function as a live analysis** — capture produces a `Snapshot`, and
analysis is a pure function of that snapshot plus the policy data — which is
how reproducibility is proven rather than asserted
(`invariant_15_analysis_is_a_pure_function_of_the_snapshot`).

It is also the rollback record. Every mutating run will capture one first, and
`restore --plan --snapshot <file>` produces a reviewable plan back to that
state.

`Snapshot::check_schema` rejects a file whose `schemaVersion` this build does
not understand, and the command exits **8** (stale or unusable input) rather
than analysing something it has misread.

## Root

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `1` |
| `toolVersion` | string | The version that captured it |
| `policyVersion` | string | The policy data in force at capture. An analysis of an old snapshot under new policy data is legitimate and the difference is visible |
| `provenance` | `Provenance` | |
| `machine` | `Machine` | Same shape as in [`analysis.v1.md`](analysis.v1.md) |
| `environment` | `EnvironmentSnapshot` | |
| `providers` | map of provider id to `ProviderSnapshot` | Ordered by provider id |

## `Provenance`

| Field | Type | Notes |
|---|---|---|
| `capturedAt` | string | |
| `capturedBy` | string | The tool version |
| `origin` | string | `capture` for a live capture, `--replay` when the snapshot was produced from a recording |
| `note` | string, optional | |

A snapshot without provenance cannot be trusted once a provider changes its
output format, which is why the fields are not optional.

## `EnvironmentSnapshot`

| Field | Type | Notes |
|---|---|---|
| `path` | array of string, optional | `PATH` entries in order, redacted — the home directory becomes `~` |
| `reported` | map of string to string, optional | The build and provider variables that were **named**, for the leaked-variable check. Values that could carry secrets are not recorded |

## `ProviderSnapshot`

| Field | Type | Notes |
|---|---|---|
| `detection` | `ProviderDetection` | See [`analysis.v1.md`](analysis.v1.md) |
| `capabilities` | `ProviderCapabilities` | See [`analysis.v1.md`](analysis.v1.md) |
| `inventory` | `ProviderInventory` | |
| `environment` | map of string to string, optional | The sanitised environment this provider was invoked in |
| `resolutions` | map of executable name to `Resolution`, optional | |

### `ProviderInventory`

| Field | Type | Notes |
|---|---|---|
| `installed` | array of `InstalledPackage` | See [`analysis.v1.md`](analysis.v1.md) |
| `requested` | object | `{ "names": [string], "mechanism": string }` — the provider's own record of user intent, and which mechanism produced it |
| `dependencies` | array | `[{ "from": string, "to": string, "buildOnly": boolean }]`. Edges are **within one provider only**; there is no cross-provider edge in this schema and there never will be (INV-07, INV-08) |
| `metadata` | map of package name to `PackageMetadata` | |
| `availability` | map of package name to `AvailabilityAssessment` | See [`analysis.v1.md`](analysis.v1.md) |

The `requested` `mechanism` is the provider's own and is recorded rather than
assumed: Homebrew's is `installed_on_request`, MacPorts' is the registry
`requested` flag. Neither is inferred from the dependency graph when the
provider can answer directly.

### `PackageMetadata`

All fields are optional; absent means the provider did not say, which is not
the same as false.

| Field | Type | Notes |
|---|---|---|
| `description` | string | |
| `homepage` | string | Mapping evidence |
| `license` | string | |
| `availableVersion` | string | What the provider currently offers |
| `origin` | string | The tap or the port tree it came from. A non-core origin makes a package `THIRD_PARTY_SOURCED` |
| `deprecated` | string | The provider's own deprecation note |
| `disabled` | string | No longer obtainable |
| `replacedBy` | string | What the provider says supersedes it |
| `pinned` | boolean | |
| `notOnDefaultPathReason` | string | Why the package's executables are not on the default `PATH` |
| `definesService` | boolean | Contributes the `SERVICE` class, which forces `MANUAL_REVIEW` (INV-19) |
| `executables` | array of string | Mapping evidence, and what a `LIVENESS_PROBE` would run |
| `aliases` | array of string | |
| `versionedSiblings` | array of string | The rest of a versioned family, e.g. the other `python@3.x` |

## Reading a snapshot instead of a machine

```sh
macportsbrew snapshot --out /path/to/before.json
macportsbrew analyze --from-snapshot /path/to/before.json
```

The analysis is computed **as of the snapshot's capture date**, not today, so
that a support deadline is weighed the way it was weighed when the snapshot was
taken. The hidden `--today <DATE>` flag overrides that for testing.

Two properties follow, and both are tested:

- **The same snapshot always produces the same answer** — every map is ordered,
  every array with a domain order is sorted, and no rule reads a clock or the
  environment (INV-15).
- **A replayed or snapshot-driven run never consults the live filesystem**
  (`invariant_15_a_replayed_reader_never_consults_the_live_filesystem`), which
  is what makes a captured recording a complete substitute for a machine.

## Snapshots and fixtures are not the same thing

A **snapshot** is the domain state: what is installed, under which provider,
with what metadata. A **recording** (`fixtures/recordings/*.json`, replayed
with `--replay`) is one layer below: the raw invocations and their exact
stdout, stderr and exit status. A recording produces a snapshot when it is
replayed; a snapshot cannot produce a recording. Fixtures for adapter tests are
recordings; snapshots are what users capture and what restore plans are built
against.
