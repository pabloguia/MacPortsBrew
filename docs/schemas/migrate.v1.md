# `migrate` schema, version 1

Emitted by `macportsbrew migrate <package> --to <provider> [--retain-source]
[--yes] [--dry-run] --json`. Built in `crates/cli/src/app/migrate.rs`; the run's
types are in `crates/application/src/executor.rs` and
`crates/cli/src/executor/mod.rs`. The compatibility rule is in
[`README.md`](README.md).

**In this build nothing executes a plan.** `migrate --yes` reaches a refusal
gate (`crates/application/src/execution_gate.rs`) that refuses every plan this
build produces, before any read site is built; the read steps behind it exist,
are tested against recordings, and are unreachable from the binary. So every
`execution` this build writes is `REFUSED` with `steps` absent, and exits 4. The fields below that
only a run past the gate fills — `steps`, `siteRefusal`, the other outcomes —
are documented because the type writes them, not because this build reaches
them.

## Modes

| Flags | `mode` | What happens |
|---|---|---|
| none, or `--dry-run`, or `--yes --dry-run` | `RENDER` | The plan is shown; nothing is run. `--dry-run` wins over `--yes` |
| `--yes` without `--dry-run` | `EXECUTE` | The plan is put to the execution gate — only a plan the gate passes would reach the read-step loop — and then shown with the outcome; everything is decided before anything is written |

`--yes` with `--from-snapshot` exits **2** (`USAGE`) before any output: a
snapshot file has no runner and records no provenance an execution could
check. Rendering from a snapshot file is allowed, and every answer it gives —
*nothing to move* included — is about the input, not the machine.

## Root

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `1` |
| `mode` | string | `RENDER` or `EXECUTE` |
| `nothingToMove` | object, optional | `{ "package": string, "provider": string, "notDetected": [string] }`. Present exactly when there is nothing to move, and then `plan` and `execution` are absent. Exit 0 (INV-16). `notDetected` is always written: the other providers that were not detected, in identifier order; empty when every other provider was detected and read (DP-026) |
| `retainedRunDone` | object, optional | `{ "package": string, "provider": string, "runId": string }`. Present exactly when a live `--retain-source` re-run is one this machine's history and a fresh capture both show done; then `plan` and `execution` are absent and nothing is written. Exit 0 (INV-16). Additive (pass 08) |
| `retainedRunNotRecognised` | object, optional | `{ "reason": string, "runId"?: string }` beside a `plan`, when a live `--retain-source` re-run consulted history, history had a record for the migration, and it was not enough to call it done. `reason` is one of `LATEST_RUN_NOT_SUCCESS`, `LATEST_RUN_REMOVED_SOURCE`, `TIE`, `RECORD_FROM_THE_FUTURE`, `DESTINATION_NOT_ACTIVE`, `PRIMARY_IS_SHIM`, `PRIMARY_NOT_CONFIRMED`. Additive (pass 08) |
| `plan` | `MigrationPlan`, optional | The plan, exactly as [`plan.v1.md`](plan.v1.md) documents it, less the `plan` command's root `concurrentProviderActivity`: the flag is written once, at this document's root. Present whenever a plan was built |
| `execution` | `Execution`, optional | Present in `EXECUTE` mode when a plan was built |
| `concurrentProviderActivity` | `ConcurrentProviderActivity` | Whether a provider may have been changing the machine while the input was read. Always written by this build, in every shape above, *nothing to move* included. |

## `ConcurrentProviderActivity`

`04-migration-and-safety.md` lets read-only analysis proceed while a provider
runs, *flagged as possibly inconsistent*; this is that flag (`plan.v1.md`,
obligation 5). Its rule is that it never claims a check that was not made.
The same object, written by the same type (the domain's `ConcurrencyFlag`), is
the root `concurrentProviderActivity` of [`plan.v1.md`](plan.v1.md) and of
[`analysis.v1.md`](analysis.v1.md) — so of `status --json` and `list --json`
as well — and those documents point here for its definition. There it was
added after those schemas were first documented, without a `schemaVersion`
bump, which the compatibility rule allows for an additive field; every
document this build writes carries it, and a consumer reads its absence — a
document from an earlier development build — as *possibly inconsistent*,
never as a check that saw nothing.

| Field | Type | Notes |
|---|---|---|
| `activity` | string | The tag, below |
| `providers` | array of string, only with `OBSERVED` | The providers seen running, sorted |
| `possiblyInconsistent` | boolean | `false` only for `NONE_OBSERVED`; an unchecked answer is never a "no" |

| `activity` | Meaning | Written by this build |
|---|---|---|
| `NOT_CHECKED` | Nobody looked | A live run: this build has no detection mechanism |
| `UNKNOWABLE` | Nothing could have looked | `--replay` and `--from-snapshot`: a recording or a snapshot file carries no process table, and a replay must not read this host's (INV-15) |
| `NONE_OBSERVED` | A check ran and saw no provider running | Never |
| `OBSERVED` | A check ran and saw `providers` running | Never |

**What *nothing to move* establishes, and what it does not.** It is answered
when the destination is the package's only holder in the input; every other
provider this build knows was either **detected** and its inventory read, or
**not detected** and named in `notDetected`; and no
package another provider holds has a mapping toward the destination that names
this package as a candidate without selecting it. That is all it claims: no
other provider *in the input* holds the package, nor anything the mappings name
as possibly the same package. A copy under a name no mapping connects is not
seen. With a provider in `notDetected` the answer is only as strong as
detection, which looks where a provider is documented to live; a copy under a
provider installed elsewhere is not seen either, and a script that needs the
stronger claim checks that the array is empty (DP-026, ruled 2026-10-03). A
provider that is broken (`PROVIDER_UNHEALTHY`, exit 5), missing from the input
or unread (`SAFETY_UNVERIFIABLE`, exit 4), and a package that may be this one
under another name (`SAFETY_BLOCKED`, exit 4) are refusals on stderr with empty
stdout — never *nothing to move* (SE-092, S6 addendum).
`PROVIDER_PRESENCE_UNPROVEN` is reserved: no longer raised since DP-026.

## `Execution`

| Field | Type | Notes |
|---|---|---|
| `outcome` | `RunOutcome` | How the run ended. Tagged by `outcome` |
| `steps` | array of `StepRecord`, optional | One per plan step, in plan order, when the gate passed the plan; absent when it refused |
| `gateRefusals` | array of `ExecutionRefusal`, optional | The gate's refusals in its own order (step index, then kind), each tagged by `refusal` with its fields. Every string in them has the home redacted and control and format characters and the line and paragraph separators escaped: a check's fields are plan text. A refusal that could not be serialised — not reachable for the derived types today — is kept as `{ "refusal": "UNSERIALISABLE" }` so the list keeps one entry per refusal |
| `siteRefusal` | object, optional | `{ "step": integer, "refusal": string, "values"?: object }` when a read site refused a command before it ran. `refusal` is one of `NOT_A_PROVIDER_COMMAND`, `UNSAFE_PREFIX`, `PROGRAM_DISAGREES`, `NOT_A_PERMITTED_READ`; `values` holds the quoted values by name, cleaned like `gateRefusals` |

### `RunOutcome` (tag `outcome`)

| `outcome` | Fields | Exit |
|---|---|---|
| `COMPLETED` | — | 0 |
| `REFUSED` | `refusals`: integer, the length of `gateRefusals` | 4 |
| `HALTED` | `step`: integer (0 for the run as a whole), `reason`: `HaltReason` | the reason's error code: 4 for a safety or unverifiable stop, 1 for records that do not match the plan, the runner's own for a transport failure |
| `PARTIAL_SUCCESS` | `step`, `reason`, as `HALTED` | 3 (INV-03). **Not a partial migration**: no run of this build dispatches a mutating step |

### `StepRecord`

| Field | Type | Notes |
|---|---|---|
| `index` | integer | The plan step's `index` |
| `kind` | string | The plan step's `kind` |
| `disposition` | string | The tag of the step's disposition, with its fields beside it: `VERIFIED` (`evidence`), `NOT_PERFORMED_IN_THIS_BUILD`, `NOT_DISPATCHED_MUTATING`, `HALTED` (`reason`, `evidence`), `NOT_REACHED` |

`evidence` is an array of `ReadEvidence`, one per command that ran:
`{ "passed": boolean, "lineCount": integer, "truncated": boolean, "cause"?:
string }`. **There is no field for a read's output**, by construction: a removal
check's output is the machine's whole inventory (`plan.v1.md`, obligation 3).

`HaltReason` is tagged by `halt`: `MANUAL_REVIEW` (`step`),
`UNEXECUTED_MUTATION`, `NOT_PERFORMED_IN_THIS_BUILD`, `SITE_UNAVAILABLE`
(`command`), `SITE_REFUSED` (`command`), `TRANSPORT_FAILED` (`command`, `error`),
`EXPECTATION` (`command`, `error`), `VERIFICATION_FAILED` (`command`, `cause`),
`ELEVATION_NOT_NONE` (`command`), `UNEVALUABLE` (`check`?, `why`) and
`RECORDS_DO_NOT_MATCH_PLAN`. `command` counts from 0 within the step. A
`check` here is the check's kind only, never its fields.

**When history is consulted.** Only by a live `migrate --retain-source` whose
planner built a plan with no `MANUAL_REVIEW` step: `--replay` and
`--from-snapshot` never read history (INV-15), a run without `--retain-source`
never does, and every refusal and *nothing to move* keeps its precedence. A
history the store cannot trust is `HISTORY_UNUSABLE` (exit 8) with empty stdout,
never read as "no record" ([`history.v1.md`](history.v1.md)). **In this build
nothing writes a record**, so a re-run is recognised only from records a later
build writes; and because every plan sets its primary through MacPortsBrew's
shim, which no capture can yet trace to a provider, such a re-run is planned
again with `PRIMARY_IS_SHIM` until shims record their target.

## Exit codes

As `plan` in `RENDER` mode: 0, or **4** when the plan stops at
`MANUAL_REVIEW`. In `EXECUTE` mode, the outcome's code above. A planner refusal
(no plan built) exits with its own code and writes nothing to stdout, so an exit
4 with a document is distinguishable from one without.

## Samples

Nothing to move, under `--yes`:

<!-- sample: --replay fixtures/recordings/both-providers-arm64-macos15.json --json migrate nginx --to macports --yes --today 2026-09-02 -->
```json
{
  "schemaVersion": 1,
  "mode": "EXECUTE",
  "nothingToMove": {
    "package": "nginx",
    "provider": "macports",
    "notDetected": []
  },
  "concurrentProviderActivity": {
    "activity": "UNKNOWABLE",
    "possiblyInconsistent": true
  }
}
```

`--dry-run` wins over `--yes`:

<!-- sample: --replay fixtures/recordings/both-providers-arm64-macos15.json --json migrate htop --to macports --yes --dry-run --today 2026-09-02 at=/mode -->
```json
"RENDER"
```

Executed: the gate refuses the plan, so nothing runs and there are no step
records. Step 2 is refused because it changes the machine, though it carries no
command (`htop` is already under MacPorts); the checks are refused because this
build cannot evaluate them. Every mutating step is also refused as
`PROVIDER_CONFIGURATION`: a replay cannot record either provider's own
configuration, so each side carries an unknown `provider_configuration` risk,
and the gate refuses every mutating step against it.

<!-- sample: --replay fixtures/recordings/both-providers-arm64-macos15.json --json migrate htop --to macports --yes --today 2026-09-02 at=/execution exit=4 -->
```json
{
  "outcome": {
    "outcome": "REFUSED",
    "refusals": 11
  },
  "gateRefusals": [
    {
      "refusal": "MUTATION_NOT_IN_THIS_BUILD",
      "step": 2
    },
    {
      "refusal": "PROVIDER_CONFIGURATION",
      "step": 2,
      "provider": "macports"
    },
    {
      "refusal": "CHECK_NOT_EVALUABLE_IN_THIS_BUILD",
      "step": 3,
      "check": {
        "check": "PROVIDER_INTEGRITY_CHECK"
      }
    },
    {
      "refusal": "CHECK_NOT_EVALUABLE_IN_THIS_BUILD",
      "step": 3,
      "check": {
        "check": "EXECUTABLES_OWNED_BY_DESTINATION",
        "executables": [
          "htop"
        ]
      }
    },
    {
      "refusal": "CHECK_NOT_EVALUABLE_IN_THIS_BUILD",
      "step": 3,
      "check": {
        "check": "LIVENESS_PROBE",
        "executable": "htop",
        "flag": "--version"
      }
    },
    {
      "refusal": "CHECK_NOT_EVALUABLE_IN_THIS_BUILD",
      "step": 3,
      "check": {
        "check": "NO_CROSS_PROVIDER_LINKAGE"
      }
    },
    {
      "refusal": "CHECK_NOT_EVALUABLE_IN_THIS_BUILD",
      "step": 4,
      "check": {
        "check": "PROVIDER_REPORTS_ACTIVE"
      }
    },
    {
      "refusal": "PROVIDER_CONFIGURATION",
      "step": 4,
      "provider": "macports"
    },
    {
      "refusal": "VERIFICATION_ASSERTS_NOTHING",
      "step": 5
    },
    {
      "refusal": "PROVIDER_CONFIGURATION",
      "step": 6,
      "provider": "homebrew"
    },
    {
      "refusal": "PROVIDER_CONFIGURATION",
      "step": 8,
      "provider": "homebrew"
    }
  ]
}
```

## Not in this version

- **A concurrency check, and the single-instance lock** (`plan.v1.md`,
  obligation 5): `concurrentProviderActivity` reports that no check was made,
  never what one would have found. Whether another MacPortsBrew instance is
  running is not reported at all; it is owed with the lock, which lands with the
  mutation it guards.
- **Writing history** (obligation 12): no run of this build is a mutating run,
  so no record is written. The store and its read-back exist
  ([`history.v1.md`](history.v1.md)); reading serves the recognition above.
