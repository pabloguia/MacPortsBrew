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
| `VERIFY_REMOVAL` | no | Carries `PACKAGE_ABSENT` **and, since 2026-09-19, a command**: the provider's inventory verb with **no package argument**, and `success: STDOUT_OMITS_PACKAGE`. Absence is the name missing from an answer that exited cleanly, was not truncated and is non-empty — four conditions, not one; the exit status is a precondition here rather than the answer (F-06, F-22) |
| `CLEANUP_REVIEW` | no | What is left behind, for a human to decide about |
| `RECORD_HISTORY` | no | |
| `MANUAL_REVIEW` | no | A human must look. Execution stops here |

**Ordering is part of the contract, not a rendering detail.**
`MigrationPlan::check_ordering` asserts that no `REMOVE_SOURCE` precedes the
`VERIFY_DESTINATION` that guards it. It establishes that by comparing
*positions*, which is the same thing only while a plan carries one package — a
step does not name the package it acts on — so it also refuses outright any plan
carrying more than one `ADD_DESTINATION`, `VERIFY_DESTINATION` or
`REMOVE_SOURCE`.

That refuses one *shape* of the ambiguity, not the ambiguity: a plan with
exactly one of each kind whose removal concerns a different package than the
verification would still pass. No such plan is expressible today — `selected` is
a single candidate, `destinationPackage` a single value, and `1→many` resolves to
`MANUAL_REVIEW` — and the pairwise check is owed when one is, and
`invariant_01_destination_is_added_and_verified_before_the_source_is_removed`,
`invariant_01_an_interleaved_plan_is_refused_rather_than_mis_ordered` and
`invariant_01_a_verification_step_asserts_what_its_name_says` are the tests that
defend it.

### What an executor inherits

Five obligations are recorded in this document and enforced by nothing today,
because nothing executes. Two of the five — items 3 and 4 — are now discharged
as *data*: the plan carries what an executor needs, and what remains is an
executor reading it. They are listed together so the executor pass inherits
a list rather than three doc comments:

1. **Refuse a plan carrying a `placeholder` command.** It cannot be run; see
   `PlanCommand.placeholder` above.
2. **Refuse a plan carrying a `check` this build does not recognise.** An
   unknown check can never count as satisfied
   (`MigrationPlan::verification_is_performable`).
3. **Perform a `VERIFY_REMOVAL` only on an inventory that is complete.** **Discharged
   as data 2026-09-19**, and what is left is the executor's half, which is one
   sentence rather than a design. `PACKAGE_ABSENT` now carries the provider's
   inventory verb with no package argument and `success: STDOUT_OMITS_PACKAGE`.
   What this entry said before, and which is why the field is shaped this way:
   the step carried no command at all, and "nonzero exit means absent" is
   verification by exit status with the sign flipped — `brew list --versions`
   exits 1 for a correctly installed cask (F-06, F-22).

   Observed on this machine 2026-09-19, and it rules out the obvious design.
   Asked about a **single name**, each provider gives the same empty stdout for
   a package that was removed, a name that never was one, and a provider that
   could not run:

   | | installed | absent | not a package |
   |---|---|---|---|
   | `port -q installed <n>` | exit 0, names it | exit 0, empty | exit 0, empty |
   | `brew list --versions <n>` | exit 0, names it | exit 1, empty | exit 1, empty |

   So "nothing came back" would mean "successfully removed", on the step that
   certifies a removal — an answer carrying no information read as a yes.
   Asking for the **whole inventory** instead makes the answer a large positive
   output (18 and 201 lines here). **The executor's obligation is four
   conditions, not one**, because this is the one semantic satisfied by saying
   nothing, so every way of saying nothing must be excluded first:

   1. the process exited **0**;
   2. its output was **not truncated**;
   3. stdout is **non-empty**;
   4. only then: the name is not the first whitespace-separated token of any
      line.

   This list said only (3) when first written, justified by the claim that a
   crashed or truncated read "produces an empty stdout". **It does not** —
   `read_bounded` returns the first `cap` bytes and sets a separate flag, and a
   killed process leaves whatever it flushed. A `brew list --versions` cut off
   at line 40 of 201 is non-empty, omits the name, and would have certified a
   removal it never saw. Found by `security-reviewer` 2026-09-19; the loader now
   also refuses this semantic on any verb the planner appends a name to.

   **And the stdout is never recorded.** It is the machine's complete installed
   inventory — a strong fingerprint that routinely names internal tooling. What
   may be written to history, a log or a report is the boolean result, the line
   count and the truncation flag.

   The cost is stated rather than discovered: removing the last package a
   provider holds leaves an empty inventory and condition (3) refuses it, which
   is a false negative and the direction this project chooses every time. It
   needs its own message — that case is the single-package migration, and "could
   not verify removal" reads as "the removal failed", whose natural response is
   to remove it again.
4. **Refuse a plan carrying a command whose `success` is `UNKNOWN`** — and
   honour the semantic on the ones that state it. **Discharged as data
   2026-09-18**, and what is left is the executor's half. The semantic now sits
   beside the verb in `provider-commands.toml`, the loader refuses an entry that
   omits one, and every command in a plan carries it (see `PlanCommand.success`
   above). What this entry said before, and which still explains why the field
   exists: observed on this machine 2026-09-08, the MacPorts read verb exits
   **0** for an installed port, for a real port that is absent, and for a name
   that does not exist at all — three cases, one exit status. So a
   `VERIFY_DESTINATION` toward MacPorts cannot fail on exit status, and under
   INV-01 that step is precisely what guards a `REMOVE_SOURCE`. Homebrew's
   `brew list --versions` does exit 1 when absent, so the two providers cannot
   share one rule and the check must read output. The executor's obligation is
   now the same shape as items 1 and 2: read the field, refuse what it cannot
   perform (F-33).
5. **A single-instance lock, and a check for a concurrently running `brew` or
   `port`.** `04-migration-and-safety.md:201` is a **MUST** and nothing
   implements it — not even a type, which every other deferred capability here
   has. It is listed rather than stubbed because the shape of the lock is the
   executor's design and not this pass's to guess; what this pass owes is that
   the obligation cannot be lost, which is what this list is for. Read-only
   analysis may proceed under a concurrent provider run, flagged as possibly
   inconsistent — and it is not flagged today either (F-20).

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
| `success` | tagged union, tag `success` | How an executor establishes that this command succeeded: `EXIT_STATUS`, `STDOUT_NAMES_PACKAGE`, `STDOUT_OMITS_PACKAGE`, or `UNKNOWN`. **Exit status is not the universal answer, and assuming it is fails open.** Observed 2026-09-08: the MacPorts read verb exits 0 for an installed port, for a real port that is absent, and for a name that is not a port at all, so a `VERIFY_DESTINATION` toward MacPorts driven by exit status can never fail - and INV-01 puts that step in front of `REMOVE_SOURCE` (F-33). `STDOUT_NAMES_PACKAGE` means the package's own name must appear in stdout and an empty stdout is a failure however the process exited. **"Appear" means equal to the first whitespace-separated token of some line, not a substring** — measured 2026-09-19, 18 of 201 installed formulae are a substring of another's name (`gh` inside `ghidra`), so a substring rule lets a failed install pass the step INV-01 puts in front of `REMOVE_SOURCE` (F-51). A prefix test does not close it either: `ghidra` begins with `gh`. `STDOUT_OMITS_PACKAGE` is how a **removal** is certified and is deliberately not that one negated: the name must be missing from a **non-empty** stdout, because asked about one name each provider answers an absent package, a name that never was one, and a provider that failed to run with the same empty output (observed 2026-09-19). The command therefore names no package and asks for the whole inventory, and an empty answer is refused rather than read as a removal. It uses the same whole-token definition of "names" as `STDOUT_NAMES_PACKAGE`, negated — one definition for the pair, because under a substring rule the two fail in opposite directions. Unlike `placeholder`, the permissive value is not the default one: a missing or unrecognised `success` deserialises to `UNKNOWN`, which an executor must refuse, so a document written before this field existed fails closed |
| `placeholder` | boolean, always present | The command is an **illustration**, not something an executor could run. A plan built for a provider that was not detected has no prefix to resolve, so the program reads `<macports prefix>/bin/macports` and the verb `<verify>`. Until this field existed nothing distinguished those from a real command, and `check_ordering` never looks at commands at all (F-20). **An executor refuses a plan carrying one** rather than running the steps around it. **Read this field before executing anything.** It is always written, but `false` is the permissive value, so a reader that ignores it — or a document produced before the field existed — sees a real command where an illustration is. Unlike an unrecognised `check`, which deserialises to a value that cannot be satisfied, a boolean has no shape that fails closed on a reader who never looks |

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

<!-- sample: --replay fixtures/recordings/both-providers-arm64-macos15.json --json plan htop --to macports at=/steps/2/commands/0 -->
```json
{
  "target": {
    "target": "PROVIDER"
  },
  "program": "/opt/local/bin/port",
  "argv": [
    "-q",
    "installed",
    "htop"
  ],
  "environment": {
    "LANG": "C",
    "LC_ALL": "C",
    "NO_COLOR": "1",
    "PATH": "/opt/local/bin:/opt/local/sbin:/usr/bin:/bin:/usr/sbin:/sbin",
    "TERM": "dumb"
  },
  "elevation": {
    "elevation": "NONE"
  },
  "success": {
    "success": "STDOUT_NAMES_PACKAGE"
  },
  "placeholder": false
}
```

<!-- sample: --replay fixtures/recordings/both-providers-arm64-macos15.json --json plan htop --to macports at=/steps/3/commands/0 -->
```json
{
  "target": {
    "target": "TOOL"
  },
  "program": "macportsbrew",
  "argv": [
    "primary",
    "htop",
    "macports"
  ],
  "elevation": {
    "elevation": "NONE"
  },
  "success": {
    "success": "EXIT_STATUS"
  },
  "placeholder": false
}
```

And the step the absence contract is about, whole, because the command and the
check it satisfies are only meaningful together — this is what an executor
inherits from item 3 above. Note `argv` carries **no package name**: the
`htop` this plan removes appears nowhere in it.

<!-- sample: --replay fixtures/recordings/both-providers-arm64-macos15.json --json plan htop --to macports at=/steps/6 -->
```json
{
  "index": 7,
  "kind": "VERIFY_REMOVAL",
  "provider": "homebrew",
  "commands": [
    {
      "target": {
        "target": "PROVIDER"
      },
      "program": "/opt/homebrew/bin/brew",
      "argv": [
        "list",
        "--versions"
      ],
      "environment": {
        "HOMEBREW_NO_ANALYTICS": "1",
        "HOMEBREW_NO_AUTO_UPDATE": "1",
        "HOMEBREW_NO_COLOR": "1",
        "HOMEBREW_NO_EMOJI": "1",
        "HOMEBREW_NO_ENV_HINTS": "1",
        "HOMEBREW_NO_INSTALL_CLEANUP": "1",
        "LANG": "C",
        "LC_ALL": "C",
        "NO_COLOR": "1",
        "PATH": "/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin",
        "TERM": "dumb"
      },
      "elevation": {
        "elevation": "NONE"
      },
      "success": {
        "success": "STDOUT_OMITS_PACKAGE"
      },
      "placeholder": false
    }
  ],
  "verification": [
    {
      "check": "PACKAGE_ABSENT"
    }
  ],
  "reason": {
    "id": "reason.verify_removal"
  }
}
```

Added 2026-09-19 because `spec-guardian` observed that the shape this pass
introduced appeared in **no golden and no documented sample** — the goldens are
`analyze` output, and the whole-plan sample below is all `MANUAL_REVIEW` and so
carries no `VERIFY_REMOVAL`. By this project's own rule a documented sample is
the contract and prose is a claim, so until this block existed the contract was
a claim plus one unit test.

These two were hand-formatted until 2026-09-19 — same content, arrays and tags
folded onto one line — and are now written exactly as the binary serialises
them, because that is what a marker can check. The verbosity is the price of
the check, and this is a contract an executor is built against rather than
prose. The pointers are `/steps/2/commands/0` and `/steps/3/commands/0` for this
recording and this package; they are positions in one output, not part of the
schema, so a plan whose steps change moves them and the marker fails loudly
rather than silently comparing the wrong command.

**Both samples were stale before this field was added to them**, and in two
ways that are worth naming rather than quietly fixing. The first said
`"elevation": { "elevation": "UNKNOWN" }` for the MacPorts read verb; observing
that verb needs no elevation turned that cell into a `none` when F-24 closed,
and the sample was never re-run. And neither carried `placeholder`, which the
row above calls always present. Both are now the binary's own output for the
invocation named above, re-generated 2026-09-18. A sample that says it came
from a command is a claim about that command, and this one had been wrong for
a fortnight in the document an executor is meant to build against.

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
| `PACKAGE_ABSENT` | — | The source provider reports the package **no longer** installed. `VERIFY_REMOVAL` carries this; it used to carry `PROVIDER_REPORTS_ACTIVE`, which asserts the opposite (F-06) |
| `PACKAGE_NEWLY_PRESENT` | — | The destination reports the package installed **and did not at plan time**. Present only where the destination was absent when the plan was built — which is exactly when presence afterwards proves the install happened rather than predating it. Its *absence* from a `VERIFY_DESTINATION` therefore says something: the destination already held the package, and the step cannot prove more than that it still does (F-06) |

**A reader that meets a `check` it does not recognise treats it as unknown
rather than refusing the plan — and can never treat it as satisfied.** Both
halves matter, and the second is the one with teeth. Tolerance is what lets a
later build add a check without every earlier one rejecting the whole document.
But this union is the gate INV-01 rests on: a reader that skipped what it did
not recognise, found the rest satisfied and cleared the gate would re-open the
very defect the new checks were added to close — a build that does not know
`PACKAGE_NEWLY_PRESENT` treating state that predates the plan as proof of an
install. So a `VERIFY_*` step carrying an unrecognised check **fails closed**,
and an executor refuses a plan it cannot fully verify rather than running the
part it understands. An unknown is never a yes.

`PACKAGE_NEWLY_PRESENT` means: absent in the state `SNAPSHOT_BEFORE` records,
present after `ADD_DESTINATION`. If the package is installed by some other means
between the plan being built and being run, the check is unsatisfiable and the
run stops — which is the correct outcome for a plan whose premise no longer
holds, not a fault to work around.

**On the version.** No bump, and not because of the tolerance above: that
tolerance is *prospective*, added in the same change as the values it protects,
so it does nothing for a reader built earlier. What makes it safe is narrower and
worth stating plainly — nothing in this workspace deserialises a
`MigrationPlan`, so plan JSON is write-only output with no reader to break, and
nothing has been released. `docs/schemas/README.md`'s rule (a value may be added
to an enumeration whose consumers *already* handle an unknown one) will apply to
the *next* value added, which is the point of adding the tolerant reader now.

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

<!-- sample: --replay fixtures/recordings/homebrew-only-intel-macos26.json --json plan curl --to macports exit=4 -->
```json
{
  "schemaVersion": 1,
  "package": {
    "provider": "homebrew",
    "name": "curl"
  },
  "source": "homebrew",
  "destination": "macports",
  "retainSource": false,
  "primaryAfter": "macports",
  "recommendation": {
    "kind": "MIGRATE_TO",
    "provider": "macports"
  },
  "overrode": {
    "kind": "MANUAL_REVIEW"
  },
  "steps": [
    {
      "index": 1,
      "kind": "MANUAL_REVIEW",
      "reason": {
        "id": "reason.destination_provider_absent",
        "params": {
          "provider": "macports"
        }
      }
    },
    {
      "index": 2,
      "kind": "MANUAL_REVIEW",
      "reason": {
        "id": "reason.mapping_confidence_below_threshold",
        "params": {
          "candidates": "0",
          "confidence": "None",
          "destination": "macports",
          "threshold": "Strong"
        }
      }
    },
    {
      "index": 3,
      "kind": "MANUAL_REVIEW",
      "reason": {
        "id": "reason.reverse_dependencies_present",
        "params": {
          "count": "2",
          "provider": "homebrew"
        }
      }
    }
  ],
  "risks": [
    {
      "id": "destination_absent",
      "reason": {
        "id": "reason.destination_provider_absent",
        "params": {
          "provider": "macports"
        }
      },
      "blocking": true
    },
    {
      "id": "mapping",
      "reason": {
        "id": "reason.mapping_confidence_below_threshold",
        "params": {
          "candidates": "0",
          "confidence": "None",
          "destination": "macports",
          "threshold": "Strong"
        }
      },
      "blocking": true
    },
    {
      "id": "reverse_dependencies",
      "reason": {
        "id": "reason.reverse_dependencies_present",
        "params": {
          "count": "2",
          "provider": "homebrew"
        }
      },
      "blocking": true
    }
  ],
  "policyVersion": "2026.09.19-3",
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
