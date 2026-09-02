# JSON schemas

Every read command supports `--json`, and every payload carries a
`schemaVersion`. Human-readable output may change freely; **`--json` output is
a compatibility surface.**

| Command | Schema | Version |
|---|---|---|
| `macportsbrew analyze --json`, `status --json`, `list --json` | [`analysis.v1.md`](analysis.v1.md) | 1 |
| `macportsbrew explain <pkg> --json` | [`explain.v1.md`](explain.v1.md) | 1 |
| `macportsbrew plan <pkg> --to <provider> --json` | [`plan.v1.md`](plan.v1.md) | 1 |
| `macportsbrew doctor --json` | [`doctor.v1.md`](doctor.v1.md) | 1 |
| `macportsbrew snapshot` (the file it writes) | [`snapshot.v1.md`](snapshot.v1.md) | 1 |

## The compatibility rule

**Backward-compatible** — no version bump, but note it in
[`../../CHANGELOG.md`](../../CHANGELOG.md):

- adding an optional field;
- adding a value to an enumeration whose consumers already handle an unknown
  value (`UNKNOWN` exists in most of them precisely so that they can).

**Breaking** — requires bumping `schemaVersion`, adding a `*.v2.md` document
beside the `v1` one, and an entry in
[`../spec-evolution.md`](../spec-evolution.md) written in the same change:

- removing or renaming a field;
- changing a field's type or its cardinality;
- removing an enumeration value, or changing what an existing one means;
- changing the shape of a tagged union, including its tag name.

The `v1` document is not deleted when `v2` arrives. A consumer pinned to `v1`
needs to be able to read what it agreed to.

## Conventions across all five

- **Field names are `camelCase`.** Enumeration values are
  `SCREAMING_SNAKE_CASE`, except `machine.architecture`, which is `snake_case`
  because that is how both providers spell it (`x86_64`, `arm64`).
- **Tagged unions carry their tag inside the object.** The tag field is named
  per union — `state`, `kind`, `owner`, `elevation`, `check` — and is
  documented with the union.
- **Optional fields are omitted, not null**, except where a table says
  otherwise. Empty collections are usually omitted too.
- **Maps are ordered by key** and arrays are sorted where the domain has an
  order, so the same input produces the same bytes (INV-15).
- **JSON goes to stdout and nothing else does.** Progress, warnings and prompts
  go to stderr.
- **Nothing here is localised.** Identifiers, reason ids, error ids and rule
  ids are stable English regardless of `--language`; the locale affects human
  output only.
- **`reason` objects carry no text.** A `Reason` is
  `{ id, params?, rule? }` — a stable identifier, its named string parameters,
  and the rule that emitted it where one did. The consumer renders it, or
  displays the identifier.

## Exit codes

Documented once, in
[`../spec/06-cli-and-ux.md`](../spec/06-cli-and-ux.md), and implemented in
`crates/domain/src/error.rs`:

| Code | Meaning | `ErrorId` |
|---|---|---|
| 0 | success | — |
| 1 | generic failure | `PACKAGE_NOT_FOUND`, `NOT_IMPLEMENTED_IN_THIS_PASS`, `IO`, `INTERNAL` |
| 2 | usage error | `USAGE` |
| 3 | partial success | — (execution only; not reachable in this build) |
| 4 | blocked by a safety check | `SAFETY_BLOCKED` |
| 5 | provider unavailable or unhealthy | `PROVIDER_NOT_FOUND`, `PROVIDER_UNHEALTHY`, `PROVIDER_OUTPUT_CHANGED`, `PROVIDER_TIMEOUT`, `CAPABILITY_MISSING` |
| 6 | user aborted | — (interactive only; not reachable in this build) |
| 7 | lock held by another instance | — (execution only; not reachable in this build) |
| 8 | stale or unusable input | `SNAPSHOT_UNUSABLE`, `POLICY_DATA_INVALID` |

`plan` exits **4** when the plan it produced contains a `MANUAL_REVIEW` step,
and prints the plan anyway. That is the normal outcome for anything the policy
engine will not act on unattended, not an error in the tool.
