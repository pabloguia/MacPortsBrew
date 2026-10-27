# `plan` schema, version 1

Emitted by `macportsbrew plan <package> --to <provider> --json`. Derived from
`MigrationPlan` in `crates/domain/src/plan.rs`. The compatibility rule is in
[`README.md`](README.md).

**A plan is the contract.** It is serialisable, diffable, reviewable and
re-executable, and it is what a human reviews before anything mutates. The
executor runs plan steps and nothing else (INV-02): anything not in the plan
does not happen, and a failing step is not improvised around.

**In this build nothing executes it.** `migrate --yes` reaches a refusal gate
(`crates/application/src/execution_gate.rs`) that refuses every plan this build
produces; the read-step loop behind it exists, is tested against recordings,
and reads nothing, because the gate refuses first. `dryRunOnly` is always `true` — the planner
writes the negation of a compile-time constant,
`THIS_BUILD_DISPATCHES_MUTATIONS`, which is `false` — and there is no code path
in the binary that runs a mutating provider subcommand. `migrate` carries this
document under its own `plan` field ([`migrate.v1.md`](migrate.v1.md)) — every
field below but the root's `concurrentProviderActivity`, which `migrate` writes
once at its own root; see "What an executor inherits".

## Root

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `1` (`PLAN_SCHEMA_VERSION` in `crates/domain/src/plan.rs`). A plan carrying any other value is refused for execution (`SCHEMA_VERSION_NOT_THIS_BUILDS`, naming `found` and `expected`): every other refusal reads a field under this build's meaning of it |
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
| `dryRunOnly` | boolean | `true` in this build: whether the build that wrote the plan can run its mutating steps. A plan carrying `false` is refused for execution (`PLAN_NOT_DRY_RUN_ONLY`). It describes the build and is not the boundary — the gate and the absence of any mutating path are |
| `concurrentProviderActivity` | `ConcurrentProviderActivity`, defined in [`migrate.v1.md`](migrate.v1.md) | Whether a provider may have been changing the machine while the input was read (obligation 5). Added to `schemaVersion` 1 after the schema was first documented, without a bump: additive, and every `plan --json` this build writes carries it, always last, after `dryRunOnly`; how to read its absence is in [`migrate.v1.md`](migrate.v1.md#concurrentprovideractivity). It belongs to the `plan` command's document, not to `MigrationPlan`: `migrate` carries the plan under its own `plan` key *without* it and writes the flag once at its own root, and the execution gate neither reads nor requires it |

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
| `CLEANUP_REVIEW` | yes | What is left behind, for a human to decide about. Counted as mutating so the gate fails closed; cleanup itself stays opt-in and is never a side effect of a migration (spec 04, Cleanup) |
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

Twelve obligations are recorded here. **Nothing executes a provider command
from a plan in this build**, so none is met by an executor; what exists since
2026-09-27 is the refusal gate every execution must pass,
`execution_refusals` in `crates/application/src/execution_gate.rs`, which
returns every reason a plan may not run as a named refusal (tag `refusal`) and
reports exit **4** (`GateReport::exit_code`) whenever it refuses anything.
Since 2026-10-01 `migrate --yes` consults it and exits with it, the refusals in
its document ([`migrate.v1.md`](migrate.v1.md)); the read-step loop behind it
exists, is tested against recordings, and reads nothing, because the gate
refuses first. It
enforces items 1, 2, 6, 7 and 11 and the refusal half of item 4, and it refuses
every plan this build produces: a plan stopping at `MANUAL_REVIEW` is refused as
such, and every other one carries a mutating step
(`MUTATION_NOT_IN_THIS_BUILD`, keyed on the step's kind, whether or not it
carries a command — `a_mutating_step_with_no_command_is_refused_in_this_build`),
checks this build cannot evaluate, and a
`VERIFY_RESOLUTION` that asserts nothing. It also refuses a plan whose step
indices are not `1..=n` in order (`STEP_INDICES_NOT_SEQUENTIAL`), since every
refusal and every approval names a step by that number, one whose
`schemaVersion` is not the one this build writes
(`SCHEMA_VERSION_NOT_THIS_BUILDS`,
`a_plan_of_another_schema_version_is_refused`), since it reads every other field
under this build's meaning of it, and a plan with no steps at all
(`EMPTY_PLAN`, `a_plan_with_no_steps_is_refused`), which the planner never
writes and which would otherwise pass as a run that did nothing.

**Which fields of a command the gate checks, exactly.** It refuses any command
on a step kind the planner writes none on
(`COMMAND_ON_STEP_THE_PLANNER_WRITES_NONE_ON`). On `VERIFY_DESTINATION` and
`VERIFY_REMOVAL` it checks `argv` and `success` against what the planner's own
functions derive from the provider command table for that side and operation
(`COMMAND_IS_NOT_THE_PLANNERS_READ` — so a forged verification step carrying a
mutating verb, or a MacPorts read relabelled `EXIT_STATUS`, is refused), and,
through `verification::expectation_for`, `target`, `placeholder`, the step's
provider side and the safety of the name read. **It does not check `program`,
`environment` or `elevation`**: the spawn site re-derives the first two from
live detection (item 10), the third is refused wholesale, and the spawn-site
allowlist remains the last line. `ELEVATION_REQUIRED` and
`ELEVATION_UNKNOWN` are refusals about this build, not about the step: INV-06
has elevation requested per operation and shown to the user, and an executor
*asks* before a step whose elevation is not `NONE` (`04-migration-and-safety.md`,
"Privilege") — this build has no such consent path. Some causes are reported
twice, under their own refusal and as an `EXPECTATION` refusal. The gate is
consulted for execution only — it never refuses *showing* a plan. Items 3 and 4 are also
discharged as *data*: the plan carries what an executor needs. They are listed
together so the executor pass inherits a list rather than scattered doc
comments:

1. **Refuse a plan carrying a `placeholder` command.** It cannot be run; see
   `PlanCommand.placeholder` above. **Enforced by the gate** (`PLACEHOLDER`),
   tested by `a_placeholder_command_is_refused` in
   `crates/application/tests/execution_gate.rs`.
2. **Refuse a plan carrying a `check` this build does not recognise.** An
   unknown check can never count as satisfied
   (`MigrationPlan::verification_is_performable`). **Enforced by the gate**
   (`UNRECOGNISED_CHECK`), tested by `an_unrecognised_check_is_refused`.
3. **Perform a `VERIFY_REMOVAL` only on an inventory that is complete.** **Discharged
   as data 2026-09-19**, and what is left is the executor's half, which is one
   sentence rather than a design. `PACKAGE_ABSENT` now carries the provider's
   inventory verb with no package argument and `success: STDOUT_OMITS_PACKAGE`.
   **The evaluation is implemented** (2026-09-27, `crates/domain/src/verification.rs`,
   tested in `crates/domain/tests/verification.rs`), and since 2026-09-30 the
   run loop runs the command and calls it (`application::executor::judge_step`,
   SE-050). `migrate --yes` calls the loop (`executor::run`, SE-092), and the
   gate refuses every plan this build produces before any read site is built,
   so no `VERIFY_REMOVAL` read runs yet; the loop keeps `ReadEvidence` only.
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
   also refuses this semantic on any verb the planner appends a name to. That is
   the general case, over the data. `expectation_for` adds the case over the
   plan in hand: a removal check whose argv names either of **this plan's**
   packages — the source's spelling or the destination's — is refused
   (`ArgvNamesAPackage`), and so is one whose source name is empty or fails
   `is_safe_provider_name`, because an empty name is no line's first token and
   would pass on any inventory (`UnsafePackageName`).

   **And the stdout is never recorded.** It is the machine's complete installed
   inventory — a strong fingerprint that routinely names internal tooling. What
   may be written to history, a log or a report is the boolean result, the line
   count and the truncation flag.
   Since 2026-10-01 that is a type, not a convention: the history record
   (`RunRecord`, `crates/domain/src/history.rs`) takes a read only as
   `ReadEvidence` and has no field a provider read could occupy
   (`a_run_record_cannot_carry_an_inventory`,
   `crates/application/tests/history.rs`). The store
   ([`history.v2.md`](history.v2.md)) keeps the same evidence-only shape;
   writing a record is owed (obligation 12).

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
   perform (F-33). **The refusal half is enforced by the gate**
   (`UNKNOWN_SUCCESS`), tested by `a_command_of_unknown_success_is_refused`;
   honouring a stated semantic is `verification::evaluate`'s, which the run
   loop calls (`judge_step`). `migrate --yes` calls that loop, and the gate
   refuses every plan this build produces before any read site is built, so no
   semantic is evaluated against a live answer yet.
5. **A single-instance lock, and a check for a concurrently running `brew` or
   `port`.** `04-migration-and-safety.md` ("Concurrency and locking") is a
   **MUST** in two halves, and this build owes them differently.
   *Read-only analysis may proceed, flagged as possibly inconsistent* — that
   half is met for `migrate`: every document carries
   `concurrentProviderActivity` (`migrate.v1.md`), the domain's
   `ConcurrentProviderActivity`, and its rule is that it never claims a check
   that was not made. A live run says `NOT_CHECKED`, because nothing looks;
   `--replay` and `--from-snapshot` say `UNKNOWABLE`, because a recording or a
   snapshot file carries no process table and a replay must not read this
   host's (INV-15). Both are possibly inconsistent, and the human output says
   so in words (`the_concurrency_flag_never_claims_a_check_it_did_not_make`,
   `invariant_15_a_replayed_run_cannot_observe_this_hosts_processes`).
   `plan` and `analyze` carry the same flag from the same source: `plan --json`
   writes it at its root after the plan's own fields, `analysis.v1` carries it
   as a root field (so `status --json` and `list --json` do too), and the human
   output of `plan` and `analyze` ends with the sentence `migrate`'s does;
   `explain` and `doctor` carry it the same way since 2026-10-03. One type, the
   domain's `ConcurrencyFlag`, writes it for every command, so the shape cannot
   drift between them
   (`every_command_that_carries_the_flag_shares_the_one_serialiser`,
   `invariant_15_a_replayed_plan_or_analysis_cannot_observe_this_hosts_processes`,
   `invariant_15_a_plan_or_analysis_from_a_snapshot_file_is_a_function_of_the_file`).
   *A single-instance lock, and detection before mutating, refusing rather
   than interleaving* — the lock and the
   interrupted-run marker are built (pass 08 slice 2; `04`, Concurrency and
   locking; `run-marker.v2.md`) and **wired behind a mutation allowance** (pass
   08 slice 4): the mutating loop probes the store, takes the lock, re-plans
   under it, reads the marker and writes it before the first mutating command.
   No shipped build takes the lock, because no shipped build has an allowance;
   the wiring's order is checked on every build, and its behaviour by
   `crates/cli/tests/executor_wiring.rs` against fakes. **Detection before mutating is
   unbuilt and inherited whole by the mutating pass**: no detection mechanism
   exists, so nothing can answer `NONE_OBSERVED` or `OBSERVED`, and the run
   loop's verification reads take no lock. Why each candidate mechanism was
   deferred rather than chosen is SE-015 and DP-030.
6. **Recognised is not evaluable.** A check this build can parse is not
   thereby one it can perform, and a step cannot pass on the checks it can
   read while ignoring the rest. Two have an evaluation today —
   `PACKAGE_NEWLY_PRESENT` and `PACKAGE_ABSENT` — and the gate refuses a step
   carrying any other (`CHECK_NOT_EVALUABLE_IN_THIS_BUILD`,
   `a_recognised_check_is_not_thereby_evaluable`). Evaluability is per step
   kind and check, because a check is answered by its own step's read: the
   first only on `VERIFY_DESTINATION`, the second only on `VERIFY_REMOVAL`.
   `PROVIDER_REPORTS_ACTIVE` had one until pass 08 slice 4 (D11; SE-094): it
   was answered by a read that establishes *installed*, not *active*, and is
   now refused wherever it appears, so a consolidation's `VERIFY_DESTINATION`,
   whose other checks are not evaluable either, asserts nothing. Each of the rest needs
   evidence this build does not gather, and `LIVENESS_PROBE` would run the
   package's own executable — a program outside the executor's
   `PERMITTED_PROGRAMS`, so adding it is a security decision, not an
   implementation detail.
7. **A verification step that asserts nothing is refused.** A `VERIFY_*` step
   with no command to read, or no check to read it against, could only pass by
   saying nothing, which makes the gate it stands for an open one
   (`VERIFICATION_ASSERTS_NOTHING`,
   `a_verification_step_that_asserts_nothing_is_refused`). `VERIFY_RESOLUTION`
   is such a step in every plan this build produces.
8. **Tie the executed plan to the one the human saw** (SE-092). INV-02 wants
   the executor to run a plan object a human has seen. `migrate --yes`
   renders and executes in one process, which is compatible with INV-02 only
   while nothing destructive is dispatched. The mutating pass owes the tie —
   the same in-memory object shown and then confirmed, or under `--yes` a
   digest of a previously rendered plan that must match — before it dispatches
   a single mutating step. **Still owed after pass 08 slice 4**, which re-plans
   under the lock and refuses on any difference from the validated plan (D6) —
   that ties the run to the machine, not to what the human saw. A flip tripwire
   fails the build that turns mutations on while this is unbuilt.
9. **When a consent path lands, the decision to ask comes from the provider
   command table plus the live capability probe, never from the plan's
   `elevation` field.** The field is what the user is *shown*; the question
   whether a step elevates is answered where the planner answers it
   (`provider-commands.toml`, relaxed only by a detected capability for a
   read), re-derived at execution. A plan edited to say `NONE` must not skip
   the question. **Built, not wired** (pass 08 slice 4, D3 and D10):
   `planner::elevation_for` is the one derivation, shared by the planner, the
   gate's pin on mutating steps under an allowance, the run loop (which
   dispatches a mutating command only when the re-derived elevation is `NONE`)
   and `consent::elevation_requests` (which lists `REQUIRED` and `UNKNOWN`
   alike); a plan whose field disagrees is refused under an allowance. The
   lookup is **for the package's kind**, read from the snapshot the plan was
   built from (a `GUI_APP`'s mutating cells are `unknown` in every entry), and
   a package that snapshot does not hold counts as a `GUI_APP`, so an
   undeterminable kind fails closed. The live probe is the capability capture
   records; a mutating operation is never relaxed by it. The gate still
   refuses every elevated step.
10. **The spawn site re-derives `program` and `environment` from live
    detection. It reads `PlanCommand.program` only to refuse a plan whose
    program disagrees with the derived one (DP-023), and never reads
    `PlanCommand.environment`.** The gate checks neither (see above): the
    program comes from the provider's detected prefix and the table's
    executable, the environment from the adapter's sanitised construction
    (`sanitised_environment`, INV-10), and the plan's copies are what a person
    reads, not what runs. **Built, not yet reachable** (2026-09-30, SE-050):
    `registry::plan_read_spec` takes argv alone from a `PROVIDER` command and
    derives the program and the environment from a `DetectedPrefix`, which
    only the registry's detection constructs, so neither the prefix nor the
    home can come from a plan or a snapshot. It refuses a plan whose `program`
    disagrees with the derived one (`ProgramDisagrees`) and checks the argv
    against the shipped read allowlist at construction (`NotAPermittedRead`).
    There is no refusal about the environment, because the environment is
    never read: refusing a tampered copy would add no safety over ignoring it,
    and comparing it would refuse honest plans, since capture redacts `HOME`
    to `~` and a replayed plan carries none. Tests:
    `crates/adapters/tests/plan_reads.rs`, among them
    `invariant_10_a_plan_read_never_adopts_the_environment_it_carries`.
    The run loop (2026-09-30) calls it only through
    `registry::PlanReadSites`, bound to the runner detection used, and only
    after the gate has passed the plan. `migrate --yes` calls the loop
    (`executor::run`, SE-092), and the gate refuses every plan this build
    produces before any read site is built. **For mutating commands** (pass 08
    slice 4, D7) the same obligation is written on the `MutationSite` trait,
    whose only product implementation refuses every command, so no mutating
    command spec is built; the configuration re-read before each one
    (`registry::reread_configuration`) re-detects the prefix and refuses
    unless it derives the step's planned program.
11. **Mutating steps are bracketed: a `SNAPSHOT_BEFORE` precedes the first,
    and the plan ends with `RECORD_HISTORY`.** The snapshot is what a failed
    migration is restored from and what the run is compared against, and the
    record is what keeps history in agreement with the machine; a change
    outside either bracket is one nothing can undo or account for. "Mutating"
    is `StepKind::is_mutating` — `ADD_DESTINATION`, `SET_PRIMARY`,
    `REMOVE_SOURCE`, `CLEANUP_REVIEW` — by kind, whether or not the step
    carries a command, and "precedes" and "ends" are by position in `steps`,
    the order an executor runs them. A plan with no mutating step needs neither
    bracket. Each bracket appears **at most once**, mutating plan or not.
    **Enforced by the gate**, with one part already enforced below it:
    `MigrationPlan::check_ordering` requires `SNAPSHOT_BEFORE` as the *first*
    step of any plan with a mutating step, so a plan missing the snapshot
    bracket is always refused as `ORDERING`. `NO_SNAPSHOT_BEFORE_FIRST_MUTATION`
    is the refusal that *names the unguarded step* — the first mutating one —
    and it never fires alone
    (`a_mutation_with_no_snapshot_before_it_is_refused`).
    `NO_RECORD_HISTORY_AT_END`
    (`a_mutating_plan_that_does_not_end_with_record_history_is_refused`,
    including a mutating plan that ends at `MANUAL_REVIEW`, refused for both).
    `MORE_THAN_ONE_SNAPSHOT_BEFORE` and `MORE_THAN_ONE_RECORD_HISTORY`, each
    naming every such step (`a_repeated_snapshot_or_history_record_is_refused`):
    `check_ordering` reads only the first snapshot, so a second is a shape no
    rule covers, and a second record would record a run twice. The closing
    bracket is the run's **success path**, not the only place a record is
    written — obligation 12. **And, whenever mutations would be dispatched**
    (pass 08 slice 4), a commanded `ADD_DESTINATION` must have a
    `VERIFY_DESTINATION` after it and a `REMOVE_SOURCE` a `VERIFY_REMOVAL`
    (`MUTATION_NOT_VERIFIED_AFTER`), so no change ends a run judged by its exit
    status alone (INV-03), and each mutating command must be the planner's own
    (`MUTATION_IS_NOT_THE_PLANNERS_COMMAND`). Every plan the planner builds meets it, which
    `every_plan_this_planner_builds_is_refused_for_execution_in_this_build` and
    `crates/cli/tests/execution_gate_fixtures.rs` assert over the synthesised
    snapshot and every recording. **Performed behind a mutation allowance by
    pass 08 slice 5** (D1, D2): `next_action_with` answers `TAKE_SNAPSHOT` for
    `SNAPSHOT_BEFORE` and `RECORD_HISTORY` only under one, the shell writes the
    capture taken under the run lock durably into the state directory and reads
    it back **before the marker is written**, and `mutating_action` refuses to
    dispatch a command unless the snapshot step was recorded taken. The
    read-only loop still passes both over. A snapshot that cannot be written
    and read back halts the run before anything changes.
12. **A mutating run writes a history record on every way it ends.** Success
    is the plan's final `RECORD_HISTORY` step; a step that fails, a run that
    ends in partial success, and a halt at `MANUAL_REVIEW` after a change was
    made each end the run *before* that step, and each still writes a record
    with its outcome (success / partial / failure), the snapshot it took and
    whether it is restorable (`04-migration-and-safety.md`, History and
    "Every mutating run records, in history, the snapshot it took first, the
    plan it executed and the outcome"). An executor that writes history only
    from `RECORD_HISTORY` loses exactly the runs a restore is needed for.
    Obligation 11 fixes where the success path records; this one fixes that
    it is not the only path that does. **Not enforceable by the gate**, which
    reads a plan and not a run; owed by the executor's dispatch loop, and its
    test is owed with it. **What the loop does today** (2026-09-30): it
    dispatches no mutating step, so no run it can execute is a mutating run
    and nothing is written; it returns, on every way a run ends, the per-step
    records (`ReadEvidence`, never output) and the outcome, refused, halted,
    partial or completed, from which the record is built. The record type and
    its append-only store were owed by the history slice (S8). **The record
    type landed 2026-10-01**: `RunRecord` (`crates/domain/src/history.rs`),
    built by `application::history::run_record` from the run and those
    records — it derives the outcome with `outcome_of` rather than accept one
    beside them — with a status of `SUCCESS`, `PARTIAL`, `FAILURE` or
    `NO_CHANGE`, and success only when every step was verified. A partial success of a run that dispatched a mutating step is
    `PARTIAL`, never `SUCCESS`
    (`invariant_03_a_partial_run_is_recorded_as_partial_never_success`,
    `crates/application/tests/history.rs`), and a
    run that dispatched none is `NO_CHANGE` whatever its ending, so it is never
    a partial migration
    (`a_partial_success_that_dispatched_no_mutation_is_not_a_partial_migration`);
    `mutationDispatched` is the one fact a store may key a lock or an
    interrupted-run marker on, and it fails toward true for a halt that cannot
    be placed before or after a dispatch. **The store landed 2026-10-04**
    (pass 08 slice 1, [`history.v2.md`](history.v2.md)) with the write
    predicate `application::history::must_record` (iff `mutationDispatched`)
    and the `--retain-source` re-run recognition `04-migration-and-safety.md`
    (History) states. **Still owed by the pass that dispatches mutations**,
    and named preconditions of its proposal: the call that writes a record on
    every way a run ends, with this obligation's test over success, partial,
    a halt after a dispatch, a `MANUAL_REVIEW` after a change and a refusal
    contradicted by dispatched steps; `SNAPSHOT_BEFORE` and `RECORD_HISTORY`
    performed, the latter judged by reading the published record back; and
    the store's probe before the first mutating step. **Wired behind a
    mutation allowance by pass 08 slice 4** (D8): once anything mutating was
    handed to the mutation site — a refused command included — the loop
    writes the record **durably** on every ending, then clears the marker; a
    record not known to be durable keeps the marker and never exits 0. **`SNAPSHOT_BEFORE` and
    `RECORD_HISTORY` are performed by pass 08 slice 5 (D2, D3)**, and
    `RECORD_HISTORY` is settled: a record never certifies its own write, so it
    is stored `RECORD_PENDING_READ_BACK` ([`history.v2.md`](history.v2.md)) and
    the run reads the published file back under the lock, comparing the
    canonical text with what it wrote; the marker is cleared only when the
    write was durable and the read-back matches, so **a record is confirmed by
    the absence of a marker naming it**. A forced run on a narrowed plan now
    ends `COMPLETED`; a real plan cannot until `SET_PRIMARY` and
    `VERIFY_RESOLUTION` are performed (carried).

A plan containing a `MANUAL_REVIEW` step is refused for execution, naming the
step (`MANUAL_REVIEW`,
`invariant_19_manual_review_refuses_execution_and_names_the_step`), and the gate
reports exit **4** for it — and for every other refusal: the gate has one exit
code, so a script cannot learn a lesser one as "fine, just not yet".

`plan --json` is a different thing and does not consult the gate. It exits
**4** only when the plan it prints contains a `MANUAL_REVIEW` step, and **0**
for any other plan it builds — including one the gate would refuse. The plan is
the useful output even when it cannot be run unattended.

### `PlanCommand`

| Field | Type | Notes |
|---|---|---|
| `target` | tagged union, tag `target` | `PROVIDER` or `TOOL`. Added in v1; additive, and a reader that does not know the field should treat a missing one as `PROVIDER` |
| `program` | string | For `PROVIDER`, an absolute path resolved from the provider's detected prefix, never from `PATH` — or, where the prefix was not detected, a `<provider prefix>/bin/...` placeholder, which is a display string and not runnable. For `TOOL`, the name a person reads in the plan — an executor must not resolve it through `PATH`; how it obtains the binary instead is the executor pass's to settle |
| `argv` | array of string | The full argument vector. **There is no shell and no string interpolation**: package names and paths are arguments, never fragments of a command line |
| `environment` | map of string to string, optional | The sanitised environment the command would run in. Omitted when empty, so its absence does not by itself mean `TOOL`: a `PROVIDER` command whose source snapshot carried no environment is written without it too (see F-21) |
| `elevation` | tagged union, tag `elevation` | `NONE`, `REQUIRED` (with a `reason` — the provider's own statement or a stable identifier), `UNKNOWN` |
| `success` | tagged union, tag `success` | How an executor establishes that this command succeeded: `EXIT_STATUS`, `STDOUT_NAMES_PACKAGE`, `STDOUT_OMITS_PACKAGE`, or `UNKNOWN`. **Exit status is not the universal answer, and assuming it is fails open.** Observed 2026-09-08: the MacPorts read verb exits 0 for an installed port, for a real port that is absent, and for a name that is not a port at all, so a `VERIFY_DESTINATION` toward MacPorts driven by exit status can never fail - and INV-01 puts that step in front of `REMOVE_SOURCE` (F-33). `STDOUT_NAMES_PACKAGE` means the package's own name must appear in stdout and an empty stdout is a failure however the process exited. **"Appear" means equal to the first whitespace-separated token of some line, not a substring** — measured 2026-09-19, 18 of 201 installed formulae are a substring of another's name (`gh` inside `ghidra`), so a substring rule lets a failed install pass the step INV-01 puts in front of `REMOVE_SOURCE` (F-51). A prefix test does not close it either: `ghidra` begins with `gh`. `STDOUT_OMITS_PACKAGE` is how a **removal** is certified and is deliberately not that one negated: the name must be missing from a **non-empty** stdout, because asked about one name each provider answers an absent package, a name that never was one, and a provider that failed to run with the same empty output (observed 2026-09-19). The command therefore names no package and asks for the whole inventory, and an empty answer is refused rather than read as a removal. It uses the same whole-token definition of "names" as `STDOUT_NAMES_PACKAGE`, negated — one definition for the pair, because under a substring rule the two fail in opposite directions. Unlike `placeholder`, the permissive value is not the default one: a missing or unrecognised `success` deserialises to `UNKNOWN`, which an executor must refuse, so a document written before this field existed fails closed. **A truncated read fails every semantic** — `STDOUT_NAMES_PACKAGE` and `EXIT_STATUS` included, not only `STDOUT_OMITS_PACKAGE`: an answer cut short at the capture bound is not the answer the provider gave — a line cut mid-name can read `gh` where the provider printed `ghidra` — and a clean exit does not vouch for output nobody read whole (SE-089) |
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

The set of `id` values is open: a consumer treats an id it does not know by its
`blocking` field alone.

**`destination_artifact_unavailable`** (since 2026-10-26; pass 08 slice 6, SE-107).
Blocking. Raised when the destination records its counterpart as `UNAVAILABLE`,
`UNSUPPORTED_PLATFORM` or `DISABLED`, the destination is not already blocked as
withdrawn (`destination_deprecated`), and it does not already hold the package. Its reason is
`reason.destination_artifact_unavailable`, parameters `destination`, `package` and
`artifact` (one of those three identifiers). A destination that would build from
source, or whose availability is `UNKNOWN`, raises the non-blocking
`destination_source_build` instead (DP-050).

**`provider_configuration`** (since 2026-10-04; DP-027, ruled 2026-10-03). Raised for the plan's
source and its destination — whether or not the plan ends up changing the source
(`--retain-source`) — once per configuration file that sets a key the provider's
own reader accepts (`reason.provider_configuration_outranks`, parameters
`provider`, `file`, `keys`) or that exists and could not be read
(`reason.provider_configuration_unreadable`, `provider`, `file`), and once for a
present provider whose configuration the input did not record — a replayed
recording, a snapshot from before the field
(`reason.provider_configuration_unknown`, `provider`). Key names only; a value is
never captured. **Not blocking** — the plan is still the plan and still renders —
but the execution gate refuses every mutating step that would change that
provider (`PROVIDER_CONFIGURATION`, below), whatever a later build can dispatch:
a provider's own configuration applies settings this tool did not choose, and
this tool does not override them (R-15, R-16), and `migrate --replay --yes` reaches the gate, so an unknown is refused
like a finding.

The gate's refusal is `{ "refusal": "PROVIDER_CONFIGURATION", "step": n,
"provider": id }`, once per mutating step naming that provider, and for a
mutating step that names no provider whenever any provider carries the risk.

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
          "confidence": "NONE",
          "destination": "macports",
          "threshold": "STRONG"
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
          "confidence": "NONE",
          "destination": "macports",
          "threshold": "STRONG"
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
    },
    {
      "id": "provider_configuration",
      "reason": {
        "id": "reason.provider_configuration_unknown",
        "params": {
          "provider": "homebrew"
        }
      },
      "blocking": false
    }
  ],
  "policyVersion": "2026.10.12-4",
  "dryRunOnly": true,
  "concurrentProviderActivity": {
    "activity": "UNKNOWABLE",
    "possiblyInconsistent": true
  }
}
```

The `confidence` and `threshold` parameters above are `Confidence`
identifiers; what a reason parameter carries is stated once in
[`README.md`](README.md).

Note `recommendation` and `overrode` together: the engine's own answer was
`MANUAL_REVIEW`, the user asked to migrate anyway, and the plan records both.
Nothing is hidden by the override — the reasons that produced `MANUAL_REVIEW`
survive as blocking risks and as steps.

## Symmetry

The same schema and the same planner serve `--to homebrew` and `--to macports`.
Exchanging the two provider identifiers in the input exchanges them in the
output and changes nothing else (INV-17). A consumer must not special-case
either direction.
