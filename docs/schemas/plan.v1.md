# `plan` schema, version 1

Emitted by `macportsbrew plan <package> --to <provider> --json`. Derived from
`MigrationPlan` in `crates/domain/src/plan.rs`. The compatibility rule is in
[`README.md`](README.md).

**A plan is the contract.** It is serialisable, diffable, reviewable and
re-executable, and it is what a human reviews before anything mutates. When the
executor lands, it will execute plan steps and nothing else (INV-02): anything
not in the plan does not happen, and a failing step is not improvised around.

**In this build nothing executes it.** `dryRunOnly` is always `true`, and there
is no code path in the binary that runs a mutating provider subcommand.

## Root

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `1` |
| `package` | object | `{ "provider": string, "name": string }` — the source identity |
| `source` | string | Provider id the package is held by |
| `destination` | string | Provider id it would move to |
| `destinationPackage` | string, optional | The name under the destination, when the mapping selected one |
| `retainSource` | boolean | `--retain-source`: keep the source package after migrating |
| `primaryAfter` | string | Which provider's copy resolves on `PATH` afterwards |
| `recommendation` | `Outcome` | What the policy engine recommends, independently of what was asked for |
| `overrode` | `Outcome`, optional | Present when the requested direction differs from the recommendation. The override is recorded with the run |
| `steps` | array of `MigrationStep` | In execution order |
| `risks` | array of `PlanRisk`, optional | |
| `elevatedSteps` | array of integer, optional | The `index` of every step whose elevation is not `NONE` — that is, `REQUIRED` **or** `UNKNOWN`. Consult each command's own `elevation` to distinguish them: an `UNKNOWN` is not a known requirement, and rendering this set as "needs administrator rights" is the defect F-10 recorded. It is nonetheless the set to gate consent on, because an elevation nobody has established is not one that will not happen |
| `policyVersion` | string | |
| `dryRunOnly` | boolean | `true` in this build |

## `MigrationStep`

| Field | Type | Notes |
|---|---|---|
| `index` | integer | 1-based, and the execution order |
| `kind` | string | See the state machine below |
| `provider` | string, optional | Which provider the step acts on |
| `commands` | array of `PlanCommand`, optional | Every command that would run, with its full argv |
| `verification` | array of `VerificationCheck`, optional | |
| `reason` | `Reason` | Why this step is in the plan |

### `kind` — the migration state machine

The values are the states of the machine in
[`../spec/04-migration-and-safety.md`](../spec/04-migration-and-safety.md), in
the order they occur:

| `kind` | Mutating | What it is |
|---|---|---|
| `SNAPSHOT_BEFORE` | no | Capture both providers' state so the run is restorable |
| `ADD_DESTINATION` | yes | Install under the destination |
| `VERIFY_DESTINATION` | no | The verification contract, before anything is removed |
| `SET_PRIMARY` | yes | Make the destination's copy the one that resolves |
| `VERIFY_RESOLUTION` | no | Confirm it actually does |
| `REMOVE_SOURCE` | yes | Only reachable after `VERIFY_DESTINATION` passed (INV-01) |
| `VERIFY_REMOVAL` | no | |
| `CLEANUP_REVIEW` | no | What is left behind, for a human to decide about |
| `RECORD_HISTORY` | no | |
| `MANUAL_REVIEW` | no | A human must look. Execution stops here |

**Ordering is part of the contract, not a rendering detail.**
`MigrationPlan::check_ordering` asserts that no `REMOVE_SOURCE` precedes the
`VERIFY_DESTINATION` that guards it, and
`invariant_01_destination_is_added_and_verified_before_the_source_is_removed`
is the test that defends it.

A plan containing a `MANUAL_REVIEW` step exits **4** when executed, naming the
step. `plan --json` exits 4 for the same reason while still printing the plan:
the plan is the useful output even when it cannot be run unattended.

### `PlanCommand`

| Field | Type | Notes |
|---|---|---|
| `target` | tagged union, tag `target` | `PROVIDER` or `TOOL`. Added in v1; additive, and a reader that does not know the field should treat a missing one as `PROVIDER` |
| `program` | string | For `PROVIDER`, an absolute path resolved from the provider's detected prefix, never from `PATH` — or, where the prefix was not detected, a `<provider prefix>/bin/...` placeholder, which is a display string and not runnable. For `TOOL`, the name a person reads in the plan — an executor must not resolve it through `PATH`; how it obtains the binary instead is the executor pass's to settle |
| `argv` | array of string | The full argument vector. **There is no shell and no string interpolation**: package names and paths are arguments, never fragments of a command line |
| `environment` | map of string to string, optional | The sanitised environment the command would run in. Omitted when empty, so its absence does not by itself mean `TOOL`: a `PROVIDER` command whose source snapshot carried no environment is written without it too (see F-21) |
| `elevation` | tagged union, tag `elevation` | `NONE`, `REQUIRED` (with a `reason` — the provider's own statement or a stable identifier), `UNKNOWN` |

**`TOOL` is not a provider invocation.** Exactly one step can carry one —
`SET_PRIMARY`, because the shim directory is MacPortsBrew's own (SE-036) — and
it is a distinct value rather than something inferred from the program string,
so the rule that applies to a command is a property a reader of `--json` can see. The rendered text plan does not yet show it. Deciding
whether an executor spawns it or performs the step in-process belongs to the
pass that writes the executor; what the plan settles is that it is not a
provider command and must not be resolved through `PATH`.

Both shapes as they actually serialise — `target`, like `elevation`, is an
internally tagged union, so the tag repeats the field name. Taken from
`plan htop --to macports --json` against
`fixtures/recordings/both-providers-arm64-macos15.json`:

```json
{
  "target": { "target": "PROVIDER" },
  "program": "/opt/local/bin/port",
  "argv": ["-q", "installed", "htop"],
  "environment": {
    "LANG": "C",
    "LC_ALL": "C",
    "NO_COLOR": "1",
    "PATH": "/opt/local/bin:/opt/local/sbin:/usr/bin:/bin:/usr/sbin:/sbin",
    "TERM": "dumb"
  },
  "elevation": { "elevation": "UNKNOWN" }
}
```

```json
{
  "target": { "target": "TOOL" },
  "program": "macportsbrew",
  "argv": ["primary", "htop", "macports"],
  "elevation": { "elevation": "NONE" }
}
```

An unrecognised `target` value fails to parse rather than reading as unknown.
That is a deliberate departure from this project's usual unknown-tolerance:
`UNKNOWN` is a real state everywhere it describes what a machine did not tell
us, but "what does this command invoke" is not an observation, and a reader
that cannot tell must not guess. The cost is that a third target would be a
breaking change; the alternative was a reader treating an unfamiliar target as
a provider invocation, or as neither.

`UNKNOWN` elevation is shown as unknown and never assumed away. Homebrew's
prefix subdirectories are user-owned so formula operations do not elevate,
while MacPorts' default prefix is root-owned so its add, remove, select and
service operations do; a Homebrew cask with a `sudo:` stanza does, which is why
cask *add* and *remove* are `UNKNOWN` until the planner can read the cask's own
metadata. Verifying one is not: `brew list` is refused as root outright,
whatever the package kind (R-5a, read 2026-09-04). All of that comes from `data/policy/provider-commands.toml`, with its
sources and read dates, not from Rust
([`../research/R-5-privilege-requirements.md`](../research/R-5-privilege-requirements.md),
2026-09-02).

### `VerificationCheck`

Tagged union on `check`:

| `check` | Payload | What it asserts |
|---|---|---|
| `PROVIDER_REPORTS_ACTIVE` | — | The destination provider reports the package installed and active |
| `PROVIDER_INTEGRITY_CHECK` | — | The provider's own integrity check passes, where it offers one |
| `EXECUTABLES_OWNED_BY_DESTINATION` | `executables` | The expected executables exist at paths the destination owns, confirmed by asking the provider |
| `LIVENESS_PROBE` | `executable`, `flag` | A non-mutating flag (`--version` by default) exits successfully |
| `NO_CROSS_PROVIDER_LINKAGE` | — | No new cross-provider linkage was introduced (INV-07) |

### `PlanRisk`

| Field | Type | Notes |
|---|---|---|
| `id` | string | Stable, e.g. `destination_absent`, `mapping`, `reverse_dependencies` |
| `reason` | `Reason` | |
| `blocking` | boolean | A blocking risk means the plan cannot run unattended |

## Worked example

`macportsbrew plan curl --to macports --json` on a machine with Homebrew and no
MacPorts. Every step is `MANUAL_REVIEW`, so the plan is entirely
non-mutating and the command exits 4:

```json
{
  "schemaVersion": 1,
  "package": { "provider": "homebrew", "name": "curl" },
  "source": "homebrew",
  "destination": "macports",
  "retainSource": false,
  "primaryAfter": "macports",
  "recommendation": { "kind": "MIGRATE_TO", "provider": "macports" },
  "overrode": { "kind": "MANUAL_REVIEW" },
  "steps": [
    {
      "index": 1,
      "kind": "MANUAL_REVIEW",
      "reason": {
        "id": "reason.destination_provider_absent",
        "params": { "provider": "macports" }
      }
    },
    {
      "index": 2,
      "kind": "MANUAL_REVIEW",
      "reason": {
        "id": "reason.mapping_confidence_below_threshold",
        "params": {
          "candidates": "0", "confidence": "None",
          "destination": "macports", "threshold": "Strong"
        }
      }
    },
    {
      "index": 3,
      "kind": "MANUAL_REVIEW",
      "reason": {
        "id": "reason.reverse_dependencies_present",
        "params": { "count": "2", "provider": "homebrew" }
      }
    }
  ],
  "risks": [
    { "id": "destination_absent", "reason": { "id": "reason.destination_provider_absent", "params": { "provider": "macports" } }, "blocking": true },
    { "id": "mapping", "reason": { "id": "reason.mapping_confidence_below_threshold", "params": { "destination": "macports", "confidence": "None", "threshold": "Strong", "candidates": "0" } }, "blocking": true },
    { "id": "reverse_dependencies", "reason": { "id": "reason.reverse_dependencies_present", "params": { "count": "2", "provider": "homebrew" } }, "blocking": true }
  ],
  "policyVersion": "2026.09.02-1",
  "dryRunOnly": true
}
```

Note `recommendation` and `overrode` together: the engine's own answer was
`MANUAL_REVIEW`, the user asked to migrate anyway, and the plan records both.
Nothing is hidden by the override — the reasons that produced `MANUAL_REVIEW`
survive as blocking risks and as steps.

## Symmetry

The same schema and the same planner serve `--to homebrew` and `--to macports`.
Exchanging the two provider identifiers in the input exchanges them in the
output and changes nothing else (INV-17). A consumer must not special-case
either direction.
