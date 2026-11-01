# `snapshot` schema, version 1

The file written by `macportsbrew snapshot [--out <file>] [<package>...]`, and read back by
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
| `providers` | map of provider id to `ProviderSnapshot` | Ordered by provider id. Each id is an identifier — `[A-Za-z0-9_.@+-]`, no leading `-`, at most 128 bytes — and must equal its entry's `detection.provider`; validated on load and on capture. **Compatibility:** a v1 file whose id breaks either rule loaded before 2026-10-02 and is now refused; no capture writes one (SE-092) |

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
| `reported` | map of string to string, optional | The build and provider variables that were **named**, for the leaked-variable check. Values that could carry secrets are not recorded. Validated on load and on capture. A **name** is held to the display-field rule (below) — bounded and free of control characters — because `doctor` prints it back, and a capture only ever writes names from a fixed list. A **value** is kept as recorded and unbounded beyond the document's own size limit (64 MiB, applied before parsing): it is whatever the shell held, and a tab or newline in `CPPFLAGS`, or a very long `PKG_CONFIG_PATH`, is a correct machine. Any rendering of a value escapes it. **Compatibility:** a v1 file with a control character in a reported name — which a capture never writes — loaded before 2026-10-02 and is now refused; a tightening of the load check, as SE-051 tightened names (SE-092) |
| `pathFindings` | array of `{entry, index, kind}`, optional | What was wrong with each `PATH` entry at capture time, in entry order. `kind` is `DUPLICATE`, `RELATIVE`, `MISSING`, `WORLD_WRITABLE`, `GROUP_WRITABLE` or `OWNER_WRITABLE`; **a reader that meets a value it does not know reports it as unknown rather than refusing the snapshot**, which is what keeps adding one additive. `entry` is redacted like every other path |
| `installations` | object keyed by provider id, optional | Where each installed provider was found: every candidate prefix that holds its executable, not only the one this tool runs (a second installation is what a detection that stops at the first hit never mentions). A value is `{"state": "READ", "prefixes": [...]}` - the candidates that hold the executable, in detection order; one entry is one installation - or `{"state": "UNKNOWABLE"}` when the capture was a replay of a recording, which carries no filesystem facts (INV-15). **A provider with no entry, or a state a reader does not know, is *not recorded*, and is never read as "one"**; the field is omitted when empty, and a document written before it existed has none (added 2026-10-12, pass 12; `schemaVersion` unchanged). Each prefix is held to the same path rule as every other captured path, and a list longer than 16 is refused. The candidates come from `data/policy/provider-prefixes.toml`, never from the snapshot |
| `resolutions` | object keyed by executable name, optional | What `PATH` resolved each name to, with an explicit state per name (work order 16 part F-2b, added 2026-11-01; `schemaVersion` unchanged). A value is `{"state": "FOUND", "resolution": Resolution, "pathIndex"?: integer}` or `{"state": "NOT_ON_PATH"}`. **A name absent from the map is *not observed*** - never read as "not on `PATH`": a replayed capture searches nothing and records nothing, so the map is empty, and so is every document written before it. `pathIndex` is the index in `path` of the first absolute entry equal, component-wise, to the hit's parent, taken before redaction; it is absent when no entry matches or an empty or relative entry comes first. See "The two resolution maps" below |

## `ProviderSnapshot`

| Field | Type | Notes |
|---|---|---|
| `detection` | `ProviderDetection` | See [`analysis.v1.md`](analysis.v1.md) |
| `capabilities` | `ProviderCapabilities` | See [`analysis.v1.md`](analysis.v1.md). **On load, every capability this document does not mention is recorded as `UNKNOWN`** - see the note below |
| `inventory` | `ProviderInventory` | Validated on load and on capture, by where each value goes. A **name** becomes an argument, so it must be one a provider could have reported: `[A-Za-z0-9_.@+-]`, no leading `-`, not `.` or `..`, at most 128 bytes. Everything else — versions, **variants**, install paths, resolved paths — reaches a reader rather than a command, and is only bounded and free of control characters; two providers fill `variants` and they fill it with different things. "Control characters" is the control class of the one classification in `crates/domain/src/text.rs` (Unicode Cc). Unicode format characters (Cf) and U+2028/U+2029 are **kept** — legitimate names carry them (a ZWJ emoji sequence, ZWNJ in a Persian word) and a snapshot is the record of what was reported — and every human rendering escapes them as `\u{..}`, so they can neither reorder the text around them nor break a line (default-ignorable characters outside Cf, such as Hangul fillers and variation selectors, are not covered). `--json` output does not: it carries them as JSON escapes only where JSON requires one (SE-092). The twelve MacPorts pseudo-portnames are refused by the *planner*, not here, so a snapshot naming a port `installed` loads and is reported, and only a plan for it is refused. A snapshot naming a package `--force` is refused rather than rendered into a plan (SE-051, finding F-09) |
| `environment` | map of string to string, optional | What this provider reported about its own environment. Descriptive; nothing is run with it |
| `invocationEnvironment` | map of string to string, optional | The sanitised environment this provider's commands are stated to run in. **Validated on load and on capture** — a reader refuses a snapshot whose value is not one the tool builds: only the variables an adapter emits, all four pinned ones present and unchanged (`LANG`/`LC_ALL` = `C`, `TERM` = `dumb`, `NO_COLOR` = `1`), `HOME` exactly `~` when present, and a `PATH` ending in `/usr/bin:/bin:/usr/sbin:/sbin` whose leading entries are all under the prefix that provider was *detected* at — or, for a provider that was not detected, under a prefix the registry knows for it, which is the common case whenever one provider is absent. No leading entry may be under another provider's prefix whatever this one declares, and a declared prefix must name a directory of its own: at least two components, no overlap with another provider's, never an ancestor of a system directory. The cross-provider comparisons fold case, because the default boot volume is case-insensitive; a case-sensitive APFS volume is supported and there the fold is merely stricter than it needs to be. Absent only for a provider whose capture failed. Note the limit: "detected at" is read from `detection.prefix` in the same document, which this version does not otherwise validate (F-09). Adding a provider variable to an adapter changes what older builds accept without changing `schemaVersion` (SE-051, finding F-21) |
| `resolutions` | map of executable name to `Resolution`, optional | Installed package names only, each filed under the provider that answered best about it. Kept for older readers and written by the rule it has always had; when `environment.resolutions` is present this map decides nothing (below) |
| `configuration` | `ProviderConfiguration`, optional | Since 2026-10-04 (DP-027, ruled 2026-10-03); additive. Tagged by `state`: `READ` with `files`, each `{ "path", "scope", "readable", "keys", "analyticsKeys"? }` — only files that exist, `scope` one of `SYSTEM`, `PREFIX`, `USER`, `keys` the **names** of every key the provider's reader accepts from the file (never a value), and `analyticsKeys` (since 2026-10-27, additive, omitted when empty) the subset of `keys` the provider-configuration data names as deciding whether the provider sends its own analytics (DP-028); a scope from a newer build reads as `UNKNOWN`; `UNKNOWABLE` when the capture was a replay, which carries no files (INV-15). Absent for a provider that is not installed and in a document from before the field, which reads as not recorded — a state an unknown `state` value from a newer build also reads as. **Validated on load and on capture:** `path` follows the captured-path rule (absolute, or `~/`-rooted), and a key is `[A-Za-z0-9_]`, at most 128 bytes; every `analyticsKeys` entry obeys the same rule and must also be in that file's `keys`. A `~/` path never names the home; a prefix path is redacted like every other |

### An unrecorded capability, on load, is an unknown one

**A rule about documents, and the opposite of the rule about values in
memory.** In memory an absent capability entry means nobody asked: a snapshot
built by hand in a test states the few capabilities the test is about, and
reading the rest as "could not be detected" would refuse every such plan.

A document is not in that position. A capture asks every capability and writes
every answer, so a document that omits one was written by a build that did not
have the question, and nothing in the file says what answer it would have
given. Absence there means the record does not say, which is what `UNKNOWN`
means - so `load_snapshot` fills every missing entry with it before any check
runs.

**The rule is unconditional, and an empty table is not an exception.** Both
adapters write every capability for a provider that is not installed - for
a while Homebrew wrote none, which is the asymmetry that made this rule look
like it needed an exception, and fixing the adapter removed the need for one
(SE-084). So the rule is a no-op on every table this build writes, which is
what keeps the live and document routes identical (INV-15), and a document
whose table is empty or partial refuses to be planned from either way.

The case this rule was written for was `SERVICE_METADATA` (F-29). Since work
order 16 part D the planner no longer reads that capability: it asks the
**source row's** `service` answer and refuses with `SafetyUnverifiable` where it
is `UNKNOWN` (DP-016 (c), DP-032; see `PackageMetadata` below). The capability
is now informational - `AVAILABLE` only when every row of the provider answers
`YES` or `NO` - and a document predating it still loads and still cannot be
planned from, because its rows carry no `service`.

Two consequences worth stating, because each is a choice:

- **`schemaVersion` does not change.** Nothing about the shape moved; a
  document that parsed before parses now. What changed is that one that omits
  a capability can no longer be planned from - which is the point, and is a
  refusal with exit code 4 rather than a parse failure.
- **The other direction is a hard failure, and that is a cost rather than an
  oversight.** A document carrying a capability key, or a `CapabilityState`
  variant, that this build's enums do not have is rejected whole with
  `SnapshotUnusable` (exit 8) - so "someone sends you their snapshot" stops
  working across a capability addition, in the newer-to-older direction, while
  `schemaVersion` says the documents are compatible. It fails closed, which is
  the right default for a document that decides whether a package is removed,
  and it is stated here because the bullet above only considered the
  older-to-newer direction (security-reviewer, 2026-09-18).
- **Read-only analysis still works on such a document**, and reports the
  capability as `UNKNOWN`. The analysis degrades and says so rather than
  failing; only the planning path refuses.

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
| `definesService` | boolean | Kept for older readers: `true` exactly when `service`'s answer is `YES`. Decides nothing in this build |
| `service` | `ServiceDefinition` | The service question (DP-032), below. Absent from a document written before work order 16 part D |
| `executables` | array of string | Kept for older readers: written from `executableList` when that is `KNOWN`, empty otherwise, and never read to decide - an empty array cannot tell "ships none" from "never asked" |
| `executableList` | `ExecutableList` | Since work order 16 part F-2b (2026-11-01); additive. Below. Two rows (one per provider) with equal non-empty `KNOWN` lists are mapping evidence (`executables`, weight 20), and the analysis looks a row's resolution up by its list; both happen only in a snapshot that named the package |
| `placement` | `Placement` | Since part F-2b; additive. Below |
| `aliases` | array of string | |
| `versionedSiblings` | array of string | The rest of a versioned family, e.g. the other `python@3.x` |

### `ExecutableList` and `Placement`

What a package ships and where it lands, from its provider's own file list
(`docs/passes/16-plans/F.md`, Design §1). A capture reads them only for the
packages the command named (`snapshot <package>...`); every other row omits
both, which reads as not recorded.

| Value | Shape | Meaning |
|---|---|---|
| `ExecutableList` | `{"state": "NOT_RECORDED"}` | Never asked. The default, and what an absent field reads as |
| | `{"state": "UNKNOWN", "reason": REASON}` | Asked, and not established |
| | `{"state": "KNOWN", "executables": [name...]}` | Asked and answered. **An empty `KNOWN` is a real answer** - the package ships no executable - and only a non-empty, fully parsed listing produces it |
| `Placement` | `{"state": "ON_PATH_DIR", "dirs": {name: directory}}` | Each executable lands in the named directory, the provider's detected prefix joined with `bin` or `sbin` |
| | `{"state": "NOT_LINKED", "reason": string}` | The provider keeps the package off its linked directories, in its own words |
| | `{"state": "UNKNOWN", "reason": REASON}` | Not established. `UNKNOWN` with reason `NOT_RECORDED` is the default, and what an absent field reads as |

**`REASON` is a union, untagged** (SG F-7): either an identifier the adapter
names - `NOT_RECORDED`, `NOT_INSTALLED`, `INACTIVE`, `CASK_NOT_COVERED`,
`AMBIGUOUS_KEG`, `TOO_LARGE`, `NAME_CLASS`, `NOTHING_UNDER_PREFIX`,
`AMBIGUOUS_DIRECTORY`, `NOT_LINKED_UNESTABLISHED`,
`AMBIGUOUS_INSTALLED_VERSION`, `CONTRADICTED_BY_RESOLUTION` (`ListReason` in
`crates/domain/src/executable.rs`) - or the error identifier a read failed with
(`PROVIDER_TIMEOUT`, `PROVIDER_OUTPUT_CHANGED`, ... - `ErrorId`). The two sets
never collide (`no_reason_collides_with_an_error_id`), so the spelling alone
says which. `CONTRADICTED_BY_RESOLUTION` is a placement reason only: the
adapter placed an executable in a `PATH` entry and the capture's own search of
`PATH` found it nowhere, or only further along (SR L-1). That is a
contradiction check, not a confirmation: a surviving `ON_PATH_DIR` is the
adapter's statement. **Unlike `pathFindings`, a `REASON` this build does not
know is refused on load** (the union has no catch-all); every value above is
spelled the same by every build that wrote one.

**Load rules** (`SnapshotUnusable`, exit 8), on capture and on load:

- Every executable name - in a list, as a placed name, and as a key of
  `environment.resolutions` - is of the one name class: non-empty, no `/`, not
  `.` or `..`, no leading `-`, at most 255 bytes, no control, format or line
  separator character, no U+FFFD. One bad name fails the document; it is never
  skipped.
- A `KNOWN` list holds at most 4096 names.
- `executables` equals the `KNOWN` list, and is empty beside any other state.
- `ON_PATH_DIR` appears only beside a `KNOWN` list holding every name it
  places, and each directory is a captured path equal, component by component,
  to the provider's `detection.prefix` joined with `bin` or `sbin`.
- A `NOT_LINKED` reason is bounded and free of control characters. Like every
  provider text it is escaped wherever it is shown.

### The two resolution maps

`environment.resolutions` (one map, a state per name) is written beside each
provider's `resolutions` (the older per-provider map), so an older reader still
sees what it always saw. The per-provider map is derived from the same search
and never read to decide while the environment map is present. A reader asks
`Snapshot::resolution_of`:

- **With `environment.resolutions` present**, that map answers, and a name it
  does not hold is not observed. On load the two maps must agree, or the
  document is refused: a `FOUND` is keyed by its own `executable`; a `FOUND`
  for an installed package name has exactly one per-provider entry, equal to
  it and filed under its owner when a provider owns it; a list-only name and a
  `NOT_ON_PATH` name have none; every per-provider entry has its `FOUND`; a
  stored `NOT_OBSERVED` is refused (absence is its spelling); a key is an
  installed package name or a name in some `KNOWN` list (nothing else is
  searched); a `FOUND`'s `path` is a captured path whose leaf is the key; an
  owner provider is one the snapshot carries and an owner package a provider
  name; and a `pathIndex` names the entry equal, component by component, to
  the path's parent, with no earlier entry equal to it, empty or relative (a
  `~/...` entry counts as absolute: it was redacted from one). The
  per-provider map's owner package is held to the name rule too.
- **Without it** - a document from before the field, or a capture that
  observed nothing - the per-provider answers are read with a fixed precedence
  per name: an authoritative provider owner, then a non-authoritative one, then
  a settled non-provider owner (`SYSTEM`, `EXTERNAL`, the shim), then
  `UNKNOWN`; ties by provider id. They carry no index, so they read as `FOUND`
  with `pathIndex` absent.

**Compatibility (SG2-N6).** These rules tighten v1 for old documents: a v1
snapshot that loaded before 2026-11-01 is now refused if it carries a bad
executable name, an `executables` array no list backs, a placement no capture
could write, or an environment map that disagrees with its per-provider one.
No capture before part F-2b wrote any of those fields except `executables`,
which every adapter left empty; every file under `fixtures/snapshots/` and every
snapshot the fixture matrix writes from `fixtures/recordings/` loads under them
(`every_committed_snapshot_loads_under_the_fail_closed_rule`).

**Cost.** A capture searches every name in a named package's list, and a hit
can cost a provider call; the listed names searched in one capture are capped
at 4096, and any beyond stay not observed. `snapshot --from-snapshot <file>
<package>...` is a usage error: a file is read, not captured.

### `ServiceDefinition`

| Field | Type | Notes |
|---|---|---|
| `declared` | `YES` \| `NO` \| `UNKNOWN` | The package's own description declares a background service |
| `materialised` | `YES` \| `NO` \| `UNKNOWN` | The provider wrote launch machinery for it on disk |

**One rule decides, in the domain** (DP-032): the package defines a service if
either half is `YES`; it does not only if both are `NO`; otherwise the answer is
`UNKNOWN`. `YES` contributes the `SERVICE` class, which forces `MANUAL_REVIEW`
(INV-19). `UNKNOWN` refuses a plan whose source row it is, with
`SafetyUnverifiable` (exit 4): a plan built anyway would look ordinary.

**Absence is unknown.** A row without `service` answers `UNKNOWN`, whatever its
`definesService` says: that bool held one half only. So a document written
before this field can still be analysed and cannot be planned from.

**Two load checks** (`SnapshotUnusable`, exit 8), at the one validation point
every route goes through:

- A row carrying `service` carries `definesService` equal to "the answer is
  `YES`". `false` beside `UNKNOWN` is consistent.
- A provider carries `service` on **every** metadata row or on none. A capture
  writes every one; a document with it on some rows only has been edited, and a
  deleted `service` would otherwise read as an older document's absence on
  exactly the row a plan is about.

**The additive rule, and its one fail-closed break.** A value from a newer build
that this one does not know reads as `UNKNOWN`. If that newer document also says
`definesService: true`, this build refuses it on load (the bool disagrees with
`UNKNOWN`), rather than load a document whose two statements of the same fact
differ.

**Where the filesystem is not the machine** (`--replay`, or a snapshot of one),
every disk-backed half is `UNKNOWN` (INV-15): both MacPorts halves and the
Homebrew materialised half. Homebrew's declared half is read from the recorded
JSON. A replayed plan whose source row does not declare a service is therefore
refused in both directions (INV-17); a plan is demonstrated from a live
snapshot instead (`fixtures/snapshots/`).

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
