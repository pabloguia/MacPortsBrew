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
| `--yes --plan-digest <d>` without `--dry-run` | `EXECUTE` | As `--yes`, after checking that `<d>` is the digest of the plan about to be put to the gate (work order 16 part E-4, DP-049 (a)): any other digest stops with `PLAN_NOT_SHOWN` (exit 4) and stdout empty, in every build. `<d>` is written as `planDigest` is; anything else is `USAGE` (exit 2) before any output, and so is `--plan-digest` without `--yes`. With `--yes --dry-run` it is checked for form and otherwise unused |

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
| `retainedRunDone` | object, optional | `{ "package": string, "provider": string, "runId": string }`. Present exactly when a live `--retain-source` re-run is one this machine's history and a fresh capture both show done; then `plan` and `execution` are absent and nothing is written. Exit 0 (INV-16). Additive (pass 08). **Not reachable in this build**: nothing writes a record, and every plan's primary is a shim (`PRIMARY_IS_SHIM`) |
| `retainedRunNotRecognised` | object, optional | `{ "reason": string, "runId"?: string }` beside a `plan`, when a live `--retain-source` re-run consulted history and history had something to say: a record for the migration that was not enough to call it done, or any record stamped after this run (`RECORD_FROM_THE_FUTURE`, which carries no `runId`). `reason` is one of `RECORD_NOT_CONFIRMED` (pass 08 slice 5, additive: the interrupted-run marker names the record, or could not be read), `LATEST_RUN_NOT_SUCCESS`, `LATEST_RUN_REMOVED_SOURCE`, `TIE`, `RECORD_FROM_THE_FUTURE`, `DESTINATION_NOT_ACTIVE`, `PRIMARY_IS_SHIM`, `PRIMARY_NOT_CONFIRMED`. Additive (pass 08) |
| `plan` | `MigrationPlan`, optional | The plan, exactly as [`plan.v1.md`](plan.v1.md) documents it, less the `plan` command's root `concurrentProviderActivity`: the flag is written once, at this document's root. Present whenever a plan was built |
| `planDigest` | string, optional | Present whenever `plan` is: `sha256:` and 64 lowercase hexadecimal digits, the digest `--yes --plan-digest` takes back to run exactly this plan. Taken over the **in-memory** plan, not over this document's redacted `plan`, with a domain-separated prefix that carries the build's version: a digest is valid only for builds with the same version string and plan serialisation. **A fingerprint of unredacted data**: someone holding it can confirm a guessed home path, so remove it before publishing a document; `doctor --report` never contains it. Additive (work order 16 part E-4) |
| `warnings` | array, optional | As in [`plan.v1.md`](plan.v1.md): `["advanced.warning"]` when the plan's `enteredBy` is `ADVANCED` or `NAMED`, at this document's root, derived and never stored on the plan. Since 2026-10-27, additive (DP-037) |
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
| `historyError` | string, optional | Present only when a run under a mutation allowance could not finish its own history (pass 08 slice 4): `IO` when its record is not known to have been written durably — it may exist, and its marker then stays as possibly the only trace — and `HISTORY_UNUSABLE` when its interrupted-run marker could not be confirmed cleared (after a record was written, or when nothing mutating ran and no record was needed). A run whose outcome would exit 0 exits with this error's code instead. As of pass 08 slice 4 (2026-10-17) no shipped build reaches this: no shipped build dispatches a mutating step |
| `runId` | string, optional | Pass 08 slice 5 (additive). The run's identifier, present when it handed a mutating step over (and so has a record, or a marker standing for one). `macportsbrew history show <id>` is how to look at it |
| `snapshotFile` | string, optional | The **file name** (never a path) of the snapshot the run took first, inside the state directory's `snapshots/`; present when one was taken |
| `recordReadBack` | string, optional | `FAILED` or `DISAGREED`, present when the published record could not be read back or read back as something other than what was written. Beside `historyError: IO`; the marker stays and the exit is non-zero |
| `markerLeft` | string, optional | Beside `historyError`: the run id whose interrupted-run marker may still be in place — it was kept, or its clear could not be confirmed. The next command that changes packages refuses until it is dealt with ([`run-marker.v2.md`](run-marker.v2.md)) |

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
| `disposition` | string | The tag of the step's disposition, with its fields beside it: `VERIFIED` (`evidence`), `NOT_PERFORMED_IN_THIS_BUILD`, `SNAPSHOT_TAKEN` and `RECORD_PENDING_READ_BACK` (pass 08 slice 5, additive: `SNAPSHOT_BEFORE` and `RECORD_HISTORY` performed — the latter pending until the published record reads back), `NOT_DISPATCHED_MUTATING`, `PERFORMED` (`ends`), `HALTED` (`reason`, `evidence`, and `ends` when any mutating command ran), `NOT_REACHED`. `PERFORMED` and `ends` are reached only under a mutation allowance, which no shipped build has; `ends` are as in [`history.v2.md`](history.v2.md) |

`evidence` is an array of `ReadEvidence`, one per command that ran:
`{ "passed": boolean, "lineCount": integer, "truncated": boolean, "cause"?:
string }`. **There is no field for a read's output**, by construction: a removal
check's output is the machine's whole inventory (`plan.v1.md`, obligation 3).

`HaltReason` is tagged by `halt`: `MANUAL_REVIEW` (`step`),
`UNEXECUTED_MUTATION`, `NOT_PERFORMED_IN_THIS_BUILD`, `SITE_UNAVAILABLE`
(`command`), `SITE_REFUSED` (`command`), `TRANSPORT_FAILED` (`command`, `error`),
`EXPECTATION` (`command`, `error`), `VERIFICATION_FAILED` (`command`, `cause`),
`ELEVATION_NOT_NONE` (`command`), `UNEVALUABLE` (`check`?, `why`),
`RECORDS_DO_NOT_MATCH_PLAN`, and — reached only under a mutation allowance —
`SNAPSHOT_NOT_TAKEN` (pass 08 slice 5: the snapshot taken first could not be
written durably and read back; nothing was changed), `MUTATION_FAILED` (`command`), `MUTATION_REFUSED` (`command`),
`MUTATING_STEP_CARRIES_VERIFICATION`, `PRECONDITION_FAILED` (`command`, `why`:
`NO_PROVIDER`, `LOCK_LOST`, `CONFIGURATION_UNREADABLE`, `CONFIGURATION_DRIFT`,
`WRONG_PROVIDER_TOKEN`, `DESTINATION_NOT_REVERIFIED`, `WRONG_PROGRAM_TOKEN`, `WRONG_CONSENT_TOKEN` — an open set) and `STATE_CHANGED` (`fact`, on `HALTED` with step 0:
`PROVIDER_STATE`, `PROVIDER_LOCATION`, `PROVIDER_CAPABILITY`, `SOURCE_PACKAGE`,
`DESTINATION_PACKAGE`, `REVERSE_DEPENDENCY`, `CLASSIFICATION`, `PLAN_DIFFERS`).
`command` counts from 0 within the step. A `check` here is the check's kind
only, never its fields.

Work order 16 part S adds, each to an open set: the halt causes
`ACTIVE_NOT_REPORTED` (`VERIFICATION_FAILED`: the read does not show exactly one
installation of the package as the active one, none or two) and
`OUTPUT_SHAPE_UNKNOWN` (the provider's answer is not in the shape this build
knows: an unreadable JSON document, or a row of the package in an unknown
shape); the `UNEVALUABLE` reason
`NOT_CERTIFIED_HERE` (a `VERIFY_DESTINATION` is certified only after its
provider's configuration was re-checked) and `ACTIVE_ALONE_AFTER_AN_INSTALL`
(`check`: `PROVIDER_REPORTS_ACTIVE`, on a step without `PACKAGE_NEWLY_PRESENT`
whose destination did not already hold the package before the run; 04, check
1; when the snapshot cannot say, the step halts with the existing
`PRESENCE_BEFORE_UNKNOWN`); the `PRECONDITION_FAILED` reason
`DESTINATION_NOT_REVERIFIED` (the destination, read again with its
`VERIFY_DESTINATION`'s own commands immediately before a removal command, did
not certify, so the source was not removed and the run ends `PARTIAL_SUCCESS`,
exit 3, after the destination was added); and the gate refusal
`CHECK_HAS_NO_ANSWERING_READ` (`step`, `check`). A `VERIFY_DESTINATION` whose
provider's configuration could not be re-read or has drifted halts
`PRECONDITION_FAILED` with `CONFIGURATION_UNREADABLE` or `CONFIGURATION_DRIFT`
and command 0 before any read; under `--replay` it always does.

**When history is consulted.** Only by a live `migrate --retain-source` whose
planner built a plan with no `MANUAL_REVIEW` step — in either mode, so a dry run
shows what `--yes` would do. `--replay` and `--from-snapshot` never read
history (INV-15), a run without `--retain-source` never does, and every
refusal, every manual review and *nothing to move* keeps its precedence. A
history the store cannot trust is `HISTORY_UNUSABLE` (exit 8) with empty
stdout, never read as "no record" ([`history.v2.md`](history.v2.md)); a run as
root is refused as `SAFETY_BLOCKED` (exit 4) before history is read. **In a shipped
build nothing writes a record** (the switch is off), so a re-run is recognised
only from records a later build writes — and only from a **confirmed** one: a
record the interrupted-run marker still names, or whose marker cannot be read,
recognises nothing (`retainedRunNotRecognised.reason` is `RECORD_NOT_CONFIRMED`,
pass 08 slice 5); and because every plan sets its primary through
MacPortsBrew's shim, which no capture can yet trace to a provider, such a
re-run is planned again with `PRIMARY_IS_SHIM` until shims record their target.

## Exit codes

As `plan` in `RENDER` mode: 0, or **4** when the plan stops at
`MANUAL_REVIEW`. In `EXECUTE` mode, the outcome's code above. A planner refusal
(no plan built) exits with its own code and writes nothing to stdout, so an exit
4 with a document is distinguishable from one without. `--plan-digest` with a
digest that is not the plan's stops before the gate in every build with
`PLAN_NOT_SHOWN` (4) and nothing on stdout; a matching one goes on to the gate,
which in this build refuses with the document on stdout. Under a mutation
allowance and without `--plan-digest`, the plan's confirmation is asked before
anything runs, and a refusal there writes nothing to stdout either: `USER_ABORTED` (6, declined), `USAGE`
(2, no terminal to confirm at), `PLAN_NOT_SHOWN` (4, the plan to run is not the
one confirmed); the error's identifier, on stderr, tells them apart. The
elevation question (wired in part E-3, reachable after part J) is asked under
the lock before anything is written, and a stop there writes nothing to stdout
either: `USER_ABORTED` (6, declined), `INTERNAL` (1, an answer the prompt never
gives), or `SAFETY_BLOCKED` (4: no terminal, no code, or a command that is not
the table's), which shares its identifier with the planner's own refusal and is
told apart by its message.

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

`--dry-run` wins over `--yes` (rendered from the committed live capture, the
input a plan with commands comes from; see below):

<!-- sample: --from-snapshot fixtures/snapshots/both-providers-intel-macos26-live.json --json migrate htop --to macports --yes --dry-run --today 2026-09-02 at=/mode -->
```json
"RENDER"
```

Executed: `EXECUTE` mode needs a runner, so it is shown over a recording, and
since work order 16 part D a recording can plan only a source row that
**declares** a background service: a keg or a registry Portfile cannot be read
under `--replay`, so any other row answers the service question `UNKNOWN` and
its plan is refused before the gate (`snapshot.v1.md`, `ServiceDefinition`).
`jupyterlab` declares one, so its plan stops at `MANUAL_REVIEW`, and the gate
refuses each such step by name; nothing runs and there are no step records. The
refusals a plan with commands draws — `MUTATION_NOT_IN_THIS_BUILD`,
`CHECK_NOT_EVALUABLE_IN_THIS_BUILD`, and `PROVIDER_CONFIGURATION` wherever a
provider's own configuration is unknown — are listed above.

<!-- sample: --replay fixtures/recordings/both-providers-arm64-macos15.json --json migrate jupyterlab --to macports --yes --today 2026-09-02 at=/execution exit=4 -->
```json
{
  "outcome": {
    "outcome": "REFUSED",
    "refusals": 2
  },
  "gateRefusals": [
    {
      "refusal": "MANUAL_REVIEW",
      "step": 1
    },
    {
      "refusal": "MANUAL_REVIEW",
      "step": 2
    }
  ]
}
```

## Not in this version

- **A concurrency check, and the single-instance lock** (`plan.v1.md`,
  obligation 5): `concurrentProviderActivity` reports that no check was made,
  never what one would have found. Whether another MacPortsBrew instance is
  running is not reported at all. The single-instance lock exists (`RUN_LOCK_HELD`,
  exit 7; `docs/spec/04-migration-and-safety.md`, Concurrency and locking) and
  nothing in this build takes it, because nothing dispatches a mutation.
- **Writing history** (obligation 12): no run of this build is a mutating run,
  so no record is written. The store and its read-back exist
  ([`history.v2.md`](history.v2.md)); reading serves the recognition above.
