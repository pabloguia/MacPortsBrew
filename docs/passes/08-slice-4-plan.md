<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 4 — plan: wiring the run loop behind the switch, and elevation consent

Work order: [`prompts/08-slice-4-wiring-and-consent.md`](../../prompts/08-slice-4-wiring-and-consent.md).
Backlog: SE-092's "Owed with the wiring" lists (slice 1 addendum, D4, D-L3,
D-M1, the slice 2 review additions, the mirror-fixture addendum), SE-016 D-R1′,
SE-050's "S5 must assert the gate passed", SE-094's `PROVIDER_REPORTS_ACTIVE`.

Revision 3, 2026-10-17. Revision 1 was reviewed by `spec-guardian` and
`security-reviewer` **before any code**. Both found holes the forced-allowance
tests would have certified and the flip would then have shipped. §7 records
each finding and what was done with it. Revision 2 had a second read; §8 records
those findings. The decisions below are already the revised ones.

`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`. While it is `false`, the
shipped binary behaves exactly as it does today: the gate refuses every plan
that has a mutating step before anything below is reached.

## 1. The shape, in one paragraph

Whether a mutating step may be dispatched stops being a constant read in two
places and becomes a value: a `MutationAllowance`, a proof type with a private
field. In product code the execution gate is the only producer, and it produces
one only when the build constant is `true` (it is not). A test can obtain one
through a constructor that exists only under
`cfg(any(test, feature = "test-support"))`. That is a declared deviation from
the letter of the work order, recorded in its own SE-092 entry (D1). A word scan
keeps the constructor out of product source.

The run loop takes the allowance only together with a `GateClear` witness, which
the gate hands out when it refuses nothing. Without both, the loop is today's
read-only loop, behaving exactly as it does now. With both, it runs the full
mutating sequence:

- the live-scope check;
- the probe;
- the lock;
- re-planning from a fresh capture under the lock, compared with the validated
  plan;
- reading the marker;
- writing the marker before the first mutating dispatch;
- for each mutating step: `verify_still_current`, the configuration re-read, the
  configuration token, the dispatch and the judgement;
- the durable record on every ending, with the marker then cleared or kept.

Mutating dispatches go through a `MutationSite` trait. Its **only product
implementation refuses**, so no mutating command spec is built anywhere in
shipped source and the spawn-site allowlist does not grow.

## 2. Decisions

**D1 — `MutationAllowance` is the seam, not a feature of the binary.**
- **The type.** `application::execution_gate::MutationAllowance`, defined in the
  gate's own file so the gate constructs it directly and no crate-visible
  constructor exists. It is a zero-sized
  `#[must_use]` type with a private field. It implements none of `Clone`,
  `Copy`, `Default`, `Serialize`, `Deserialize` or any `From`. A hand-written
  compile-time check asserts it is not `Clone` and not `Default`, without a new
  dependency.
- **Producers.**
  - `execution_gate::mutation_allowance() -> Option<MutationAllowance>`, which is
    `THIS_BUILD_DISPATCHES_MUTATIONS.then(...)`. The gate file is already on the
    constant's three-file allowlist.
  - `MutationAllowance::forced_in_tests()`, under
    `cfg(any(test, feature = "test-support"))`.
- **The word scan,** in `crates/cli/tests/executor.rs` beside the `map_plan`
  scan, fails on any of these in a `crates/*/src` file other than the type's
  definition and the gate's producer:
  - `forced_in_tests`;
  - `MutationAllowance(`;
  - an `impl` of a trait for `MutationAllowance`, or a `derive(` attribute on it;
  - the same three for `GateClear` and `ConsentGiven`.

  The exemption names exact files, not a directory.
- **`migrate` asks the gate.** It obtains its allowance from
  `execution_gate::mutation_allowance()`, never from a literal `None`, so there
  is one switch. The shell never reads the constant (module rule 2).
- **Declared deviation.** The work order (§2) says the forcing entry is
  `#[cfg(test)]` or lives under `crates/*/tests/`. A `test-support` feature is
  neither, and resolver 2 compiles it into the `macportsbrew` binary that
  `cargo test` builds for the integration tests. It is dead code there, with no
  call site. It is the only way to cross the crate boundary to a private field.
  It differs from `map_plan`: `map_plan` edits a plan the gate still judges,
  while this grants the power to mutate. So it gets its own SE-092 entry, with
  `cargo tree -e features` evidence.
- **Release builds.** A release build must not unify the feature in. That rule
  is written down (SE-092), and the release slice enforces it (§7, L1).

**D2 — the gate takes the allowance and hands out the witness.**
- **Signature.** `execution_refusals_with(plan, Option<&MutationAllowance>)`.
  `execution_refusals(plan)` is `_with(plan, None)`, and every existing caller
  and test keeps calling it.
- **The two build-state refusals** read
  `dispatches = THIS_BUILD_DISPATCHES_MUTATIONS || allowance.is_some()`:
  - `MutationNotInThisBuild` applies only when `!dispatches`;
  - `PlanNotDryRunOnly` applies when `plan.dry_run_only != !dispatches`.
- **Under an allowance the gate also refuses:**
  - a mutating step whose argv or `success` differs from the planner's
    derivation from `provider-commands.toml` (`planned_argv` and
    `planned_success`, which today pin only `VERIFY_*` steps);
  - a mutating step whose plan `elevation` field differs from the elevation
    re-derived for it (D3). For a mutating operation the command table alone
    decides, because a capability may relax only a read (obligation 9). So the
    pin needs no snapshot, and the gate gains no input beyond the allowance.
- **Everything else is unchanged.** Elevation, placeholders, unknown checks,
  unevaluable checks and the configuration refusal (DP-027 (b)) all still refuse
  under a forced allowance.
- **The witness.** `execution_gate::clear(run: PlannedRun, allowance: Option<MutationAllowance>) -> Result<GateClear, (PlannedRun, Vec<ExecutionRefusal>)>`
  takes both **by value** and returns the witness only when the refusals are
  empty. `GateClear` owns the run and the allowance it was cleared under and
  hands out only borrows (`run()`, `allowance()`). It has a private
  constructor, no `Clone` and no derives. `run_with_mutations` takes only the
  witness, so a witness cannot accompany another plan, and one cleared without
  an allowance cannot be paired with one: the argv and elevation pins have
  always run on what is dispatched. Read sites, the `MutationSite` and the
  recapture are built only after the gate passed. That is structural, and it
  closes SE-050 "Not closed" (1).
- **The input change.** Adding an input to the gate is a design change for
  SE-092 (`.claude/rules/executor.md`), and it is recorded there.

**D3 — what `next_action_with` may dispatch, and where elevation comes from.**
- **Signature.** `next_action_with(plan, position, records, Option<&MutationAllowance>)`.
  `next_action` is `_with(.., None)`. Without an allowance nothing changes, and
  the guard's existing tests stand.
- **When it answers `Action::DispatchMutating`.** Only when all of these hold,
  after the existing manual-review and record-consistency checks:
  - the step is `ADD_DESTINATION` or `REMOVE_SOURCE`;
  - every command has `CommandTarget::Provider`;
  - every command's **re-derived** elevation is `NONE`;
  - the step's own `verification` list is empty.
- **Where elevation comes from.** It is re-derived by a product function,
  `planner::elevation_for(entry, operation, kind, detected)`. That function is
  extracted from the inline derivation in the planner's `command()`: the stored
  cell keyed by kind, then relaxation **only** for read operations. The planner,
  the gate pin, `next_action_with` and `elevation_requests` all use it. A test
  shows the planner's elevation and the re-derived one agree for every operation
  and kind. The `detected` capability comes from the validated plan's snapshot;
  D6's equal plans make the fresh capture equivalent. Elevation is never read
  from the plan's field, which is plan.v1 obligation 9.
- **When it halts instead:**
  - `UNKNOWN` counts as elevating, so it halts `ElevationNotNone`;
  - a non-empty verification list on a mutating step halts
    `MutatingStepCarriesVerification`, so it is never silently dropped.
- **Steps that are never dispatched.**
  - `SET_PRIMARY` is a `CommandTarget::Tool` self-invocation of SE-036 layer 4,
    which does not exist. `CLEANUP_REVIEW` is a review.
  - Both stay `PassOver(NotDispatchedMutating)`, as does a commandless mutating
    step.
  - `SNAPSHOT_BEFORE` and `RECORD_HISTORY` stay `NotPerformedInThisBuild`.
- **The test.** A plan edited so that an elevating table entry says `NONE` is
  refused by the gate (D2), still halted by the loop, and still listed by
  `elevation_requests` (D10).
- **INV-01 under an allowance.** `invariant_01_a_removal_is_dispatched_only_after_its_destination_verified`
  is a new application-level test with a forced allowance: `REMOVE_SOURCE` is
  answered `DispatchMutating` only when the `VERIFY_DESTINATION` guarding it is
  recorded `Verified`, and never after a halted, passed-over or missing record.
  It is mutation-proved. `invariant_01_the_loop_never_dispatches_a_mutating_step`
  (without an allowance) stays unchanged.

**D4 — a dispatched mutating step's disposition, and what counts as dispatched.**
- **`Disposition::Performed { ends: Vec<ProcessEndEvidence> }`:** every command
  exited 0. The step's success is not inferred from that, because 04 says never
  from an exit status alone; the `VERIFY_*` step after it judges.
- **A halt partway** carries the evidence of every command that already ran:
  `Halted { reason, ends_before: Vec<ProcessEndEvidence> }` for:
  - `MutationFailed { command }`, a non-zero exit;
  - `TransportFailed { command, error: ErrorId }`;
  - `MutationRefused { command }`.
- **What counts as dispatched.** A mutating step counts as dispatched when any
  command was handed to the site, including a refusal of command 0. This fails
  toward dispatched, as a read site's `SiteRefused` already does
  (`history::halted_after_dispatch`).
- **What halt reasons may carry.** A command index and an `ErrorId`; never error
  text, argv or anything a process printed (rule 5).
- **Domain changes.**
  - `StepResult::Performed { ends }`, additive in `history.v1.md`.
  - `RecordedStep::new` counts `ends_before` as dispatched.
  - `RunRecord::new` counts `Performed` as carried out, beside `Verified`, for
    `Success`. `from_stored` re-derives it, and `outcome_of` follows.
- **Tests:**
  - a performed mutation whose verification halted is `Partial`, never
    `Success` (INV-03);
  - a refusal, a non-zero exit and a transport failure on the **second**
    command each produce a record. These are **application-level** tests (the
    disposition, `halted_after_dispatch`, `RecordedStep::new`, `RunRecord`).
    Under an allowance the gate refuses any mutating argv that is not the
    planner's, and the planner writes one command per mutating step, so the
    shell cannot be reached with two. The shell test covers a single-command
    refusal, and no further seam is added.
- **For slice 5's backlog.** `restore` decides from `ends_before` plus the halt
  reason, never from the dispatched flag. A refusal of command 0 is recorded as
  dispatched (fail toward dispatched), but nothing was added.

**D5 — the order, and the tripwire rule this slice changes (declared).**
- **When the marker is written.** Immediately before the **first dispatched
  mutating command** (D-M1). Reads may come before it, because a read changes no
  provider state (SE-050 NC6's network side effect is noted). That is true of
  every retain-source consolidation plan: `ADD_DESTINATION` has no commands
  there, so `VERIFY_DESTINATION` reads run before `REMOVE_SOURCE` or nothing.
- **The pinning test** states the rule exactly that way.
- **The renames.** The read helper becomes `read_commands`. The trait method is
  `MutationSite::perform`, and the only function that calls it is
  `mutation_dispatch`.
- **Why the tripwire's token has to move.** `calls_of` ignores a match glued to
  an identifier character. So after the renames, `dispatch(` names no call at
  all. Revision 1 claimed it would then name only mutating dispatches, and that
  was wrong.
- **The tripwire change.**
  - `DISPATCH` moves to `mutation_dispatch(`.
  - The `GOOD` stub and every mutant stub are updated.
- **New inverse tripwires:**
  - `MutationSite::perform` is called only inside `mutation_dispatch`;
  - `read_commands` never calls it;
  - no function in the shell, free, inherent or trait, is named bare `dispatch`.
- **Running `wiring_violations`.** Once this slice lands it runs
  **unconditionally** against the shell. It no longer waits for the constant,
  because the sequence it checks now exists.
- **The record.** All of this goes in a dated SE-092 entry of its own: the old
  and new token, why `calls_of` forces the change, why reads before the marker
  are safe, the inverse tripwires, and the mutants killed (the read helper
  renamed `dispatch`; a `MutationSite` call outside `mutation_dispatch`).

**D6 — under the lock, re-plan and compare (D-L3).**
- **The closure.** The caller supplies
  `recapture_and_replan: FnOnce() -> Result<(Snapshot, MigrationPlan), DomainError>`,
  run **after** the lock is held:
  - `migrate` captures through the same registry and runner, reads the history
    view **again** under the lock, and calls `migration_start` with the same
    request, policy and provider ids;
  - a test supplies one that applies the same narrowing it applied to the
    validated plan.
- **The comparison.** A pure `application::executor::plan_still_holds(validated: &MigrationPlan, fresh: &MigrationPlan, before: &Snapshot, fresh_snapshot: &Snapshot) -> Result<(), StateChanged>`
  passes only when the two plans are equal. Any capture-identity field that
  turns up is normalised, and named in the doc comment.
- **When they differ.** It refuses `SafetyBlocked` and dispatches nothing. It
  then names the first fact that moved, as an identifier and never plan text:
  provider presence or state, source presence or version, destination presence,
  a reverse dependency (INV-04), classification (INV-19), ownership, mapping, a
  configuration risk, elevation capability, prefix, executable. When no listed
  fact explains the difference, it names `PLAN_DIFFERS`.
- **Tests named for what they protect.** Between the plan and the lock:
  - a dependent appears in the source provider: `invariant_04_*`, refused, the
    fact named;
  - a service starts: `invariant_19_*`, refused, the fact named;
  - the plans differ and no listed fact explains it: `PLAN_DIFFERS`.

  Each is in the mutation list.
- **What this covers.** Re-planning covers every planner input, which a
  hand-picked subset cannot. Naming the fact is the diagnosis that follows, so
  nothing is hidden by comparing first.

**D7 — per mutating step: `verify_still_current`, the re-read, the token.**
- **The order.** Before every mutating dispatch:
  1. `lock.verify_still_current()`. A refusal stops the run with nothing
     dispatched.
  2. The provider's configuration is re-read through
     `adapters::registry::reread_configuration(provider_id, planned_program, home, scope)`.
     This is a new adapters function outside `process/`. It re-detects the
     prefix, refuses when the program derived from it is not the step's planned
     program (as the site does, DP-023), and returns the fresh
     `ProviderConfiguration`. The configuration that was checked and the command
     that runs therefore share one prefix. The CLI therefore
     never holds a prefix, and the existing ban on the word `detected_prefix` in
     `crates/cli/src` holds unchanged.
  3. `recheck_provider_configuration(provider, planned.before(), &fresh)` checks
     against the validated plan's own snapshot, not the D6 capture. It returns
     the `ConfigurationRechecked` token or `ConfigurationDrift`.
- **`home`** is the one captured at context build (SE-016 D-R1′), never read
  from the environment at this point.
- **The token.** It is neither `Clone` nor `Copy`, so one step cannot reuse
  another's. `MutationSite::perform` itself requires the token **by value**
  together with `&RunLock`; the shell helper is not the only place that does. A
  token whose `provider()` is not the step's provider is refused
  (`SafetyBlocked`, not dispatched).
- **The trait's documented obligations,** owed by any implementation, which
  plan.v1 obligations 9 and 10 require:
  - the program is re-derived from the detected prefix, which must equal the
    prefix D6 verified, and the site refuses otherwise;
  - the argv is checked against the allowlist;
  - `PlanCommand.program` and `PlanCommand.environment` are never spawned.
- **A scan** allows exactly one `impl MutationSite for` under `crates/*/src`.
  Its body must name no runner, no registry and no `process::` item.

**D8 — the record on every ending, and the marker (D4, D-M1, D-M3, D-M5).**
- **When a record is written.** If anything mutating was dispatched (D4's rule),
  `history::run_record` builds the record and `history_store::write_record`
  writes it **durably**, on every ending: completed, partial, halted, or refused
  under the lock. The record now uses `write_new_durable`, so the
  `Durability::BestEffort` justification "the marker notices a power loss before
  the sync" stops being relied on. An unconfirmed directory sync counts as "not
  written". A test makes the sync fail after the record is published, and the
  marker must stay. In that case the record may exist and is not known to be
  durable: the report says exactly that, never "was not written", and the record
  is not deleted, because the store is append-only.
- **Record durably written:** `run_marker::clear`.
  - `NotOurs` and `AlreadyAbsent` both become `HistoryUnusable`. The run is not
    reported as done, and the marker, if present, stays. Under a held lock, the
    marker this run wrote cannot have gone.
  - The `ClearOutcome::AlreadyAbsent` doc comment is updated to say so, and the
    change is recorded in SE-092.
- **`clear` returns `Err`,** for example when `verify_still_current` fails
  inside it because `run.lock` was deleted:
  - the marker is stranded;
  - the exit is non-zero, `HistoryUnusable`, which takes precedence over the
    run's own code unless that code is already non-zero;
  - the report names the run id and the D-M3 remedy, and the `migrate.v1`
    document gains `markerLeft: <run id>` so a JSON consumer sees it too;
  - no restore is offered.
- **Record and marker both left behind (L4).** The next run refuses
  `RunInterrupted` for a run that was in fact recorded. That reconciliation is
  slice 5's, carried explicitly, and it also covers a record published with an
  unconfirmed sync. The §4 test matrix includes it.
- **Record not written:**
  - the marker is kept;
  - the exit is non-zero (a 0 becomes `IO`);
  - the report says the history was not written, and names the snapshot path
    when one exists;
  - `restore --run` is never offered for that id (SE-092 D4).
- **Nothing mutating dispatched:** no record (`must_record`). If the marker was
  written, it is cleared at once. Under this rule that happens only when a
  refusal came after the marker and before any command reached the site.
- **The lock** is released by drop at the end of the function, on every path.

**D9 — `RECORD_HISTORY` stays `NotPerformedInThisBuild`, and SE-092's
commitment is amended rather than met.**
- SE-092's provisional note says the `RECORD_HISTORY` representation is settled
  "before any build writes records … with the write call".
- This slice lands the write call, but behind a switch no shipped build turns
  on, so no **shipped** build writes records. Forced test builds write to a
  scratch store. The note is amended: settled before the
  constant is `true`, with the representation carried to slice 5. Slice 5 reads
  records back for `history` and `restore`, which judging the step needs.
- A flip tripwire enforces this (D14).
- **Consequence:** while `RECORD_HISTORY` is not performed, every forced run
  ends `PARTIAL` (exit 3). `VERIFY_RESOLUTION` still halts
  `NoEvaluationForStepKind` too.

**D10 — consent (Part B), built and not wired.**
- **What a request holds.** `application::consent::elevation_requests(plan, snapshot) -> Vec<ElevationRequest>`
  lists every command whose **re-derived** elevation is not `NONE`, with
  `UNKNOWN` counted as elevating (04 Privilege). For each, a request carries:
  - the step index;
  - the provider;
  - the **re-derived** program and argv. When the plan's text differs from them,
    the result is a refusal, not a request.
- **The decision type.** `ConsentDecision { Given(Vec<ConsentGiven>), Refused(ConsentRefusal) }`,
  with `ConsentRefusal { NotATerminal, Declined, NoRequests }`.
- **`ConsentGiven`** is one token per step:
  - minted only by `application::consent::confirm(&ElevationRequest, typed: &str) -> Option<ConsentGiven>`,
    which itself checks the typed phrase against the request, so consent cannot
    exist without a matching phrase. A word scan limits callers of `confirm` to
    `crates/cli/src/consent.rs`;
  - neither `Clone` nor `Copy`;
  - holds the step index and the re-derived program and argv themselves, compared
    by equality. No hash is defined, and none is needed;
  - is consumed and checked by the (future) elevated dispatch, the way the
    configuration token is.
  - Consent is per operation: never cached, never inherited (INV-06, 04).
- **When to ask.** The wiring that slice 6 or the flip owes asks **after D6,
  under the lock**, so consent covers the state that was verified.
- **The prompt,** `cli::consent::ask(requests, streams: ConsentStreams, messages)`:
  - **Terminal check.** It counts as a terminal only when both stdin and the
    stream the prompt is written to are terminals. Otherwise it refuses
    `NotATerminal` without reading input.
  - **Output.** The prompt goes to stderr, never stdout, so `--json` stays
    clean.
  - **Input.** Read with a bound (`take`). EOF means declined.
  - **The confirmation** is an ASCII phrase that includes the step number. It is
    not translated, and is compared exactly after removing only the trailing
    `\n` or `\r\n`. Never `y`, never an empty line.
  - **Rendering.** Every request is rendered through the catalogue (new keys
    `consent.*`, in English and pt-BR). Each argv element goes on its own line,
    quoted and passed through `escape_hostile`, never truncated. The program
    shown is the re-derived one.
  - **Home.** The invoking user's captured home appears as `~`, which is
    unambiguous because the prompt states it is that user's home (shell rule 5,
    SE-050 NC3). Anything persisted or written to `--json` is redacted as
    everywhere else.
- **`--yes`.** It is not a parameter of `ask`, so it cannot imply consent. A
  test drives `migrate --yes` on an elevated plan and shows the gate still
  refuses.
- **What stays as it is:**
  - the gate's `ElevationRequired` and `ElevationUnknown` refusals are
    untouched;
  - no elevated command is built or run;
  - MacPorts version selection stays "proposed, never run" (SE-036).
- **Carried residuals.** The two elevated residuals of 04 (Provider
  configuration) stay **carried**, because no elevated step is dispatched until
  consent is wired.

**D11 — `PROVIDER_REPORTS_ACTIVE` (SE-094).**
- **The change.** `answers` no longer lets an "installed" read (`ExitStatus`,
  `StdoutNamesPackage`) answer `ProviderReportsActive`, and
  `evaluable_in_this_build` reports the check unevaluable. Keeping it would
  certify "installed" as "active", a false verification guarding INV-01.
- **What stays carried.** Evaluating it honestly. **No read the plan carries
  establishes "active" for both providers**, and evaluating it for one provider
  only would be a direction asymmetry (INV-17). Two alternatives, both recorded
  in SE-094 with why each was not taken:
  - **An inventory binding.** Capture's inventory read yields an `active` field
    per row (`run_record` reads `row.active`). Using it would bind a check to a
    fresh inventory read rather than to the step's own commands, which the
    verification path does not do today. It is sized with slice 5's read-back.
  - **A new success semantic** over MacPorts' existing verify read, whose output
    is believed to mark the active version. That is unobserved here, so a
    `provider-researcher` note is owed. Homebrew's verify read shows no
    linkage, so this would answer for one provider only.
- **Effect.** The gate adds one `CheckNotEvaluableInThisBuild` refusal per plan
  that carries the check. The shipped binary already refuses every plan, and the
  goldens and checked samples are regenerated.
- **DoD.** SE-094's DoD box ("asserts what its name says") is **carried, not
  done**, and SE-094 says so.

**D12 — the CLI half of `invariant_17_retained_run_recognition_is_direction_neutral`
is carried, and its DoD box stays unticked.**
- `recognise_retained_run` needs the latest record to be `SUCCESS`. Under D9 no
  wired run can reach `SUCCESS`, so a record a wired run wrote can never be
  recognised.
- The test lands with `RECORD_HISTORY` performed (slice 5). SE-092's mirror
  addendum is amended with this reason.
- **What the INV-17 coverage becomes instead.** The behavioural test (§4) runs
  both directions:
  - toward Homebrew, a plan with an `ADD_DESTINATION` command;
  - toward MacPorts, a retain-source consolidation plan with no `ADD`
    commands, because MacPorts `ADD_DESTINATION` elevates and stops at
    `ElevationNotNone`.

  The asymmetry is data, not code, and the test says so.
- **What the forced runs do and do not cover**, declared:
  - the →MacPorts consolidation run dispatches nothing mutating, so it stops
    before the marker and writes no record. It proves the reads, the gate, the
    lock and the re-plan, and that nothing mutating is reached;
  - in every direction `VERIFY_RESOLUTION` halts before `REMOVE_SOURCE`, so
    `REMOVE_SOURCE` **cannot be reached in this build even when forced**;
  - dispatching twice in one run (the per-step re-check and token across two
    dispatches) is proved only by the text scan and by unit tests, never by a
    shell run.

**D13 — the mutating path's entry refusals.**
- These come before the probe, and the order is tested:
  - `ctx.history != HistoryScope::ThisMachine` (under `--replay` or
    `--from-snapshot`) refuses at once. It touches no state directory, and the
    observer sees nothing (INV-15).
  - `refuse_if_root` has already refused euid 0, which closes S8 item 6. Its
    `Ok(false)` ("could not learn the user") refuses on the mutating path.
- **One resolution.** `StateDir` and `euid` are resolved **once** in `migrate`.
  The same values reach the history read, the probe, the lock, the marker and
  the record, and the state-directory override is read at context build like
  `home`.

**D14 — what blocks the flip is enforced by tests, not by prose.**
- **The constant.** A test pins `THIS_BUILD_DISPATCHES_MUTATIONS == false` in
  `crates/application/tests/execution_gate.rs` (work order §2).
- **Flip tripwires,** next to SE-015's. Each fails while the constant is `true`
  and its item is outstanding:
  - `SNAPSHOT_BEFORE` not performed (plan.v1 obligation 11);
  - `RECORD_HISTORY` not performed and settled (D9);
  - obligation 8, tying the executed plan to the one shown, unbuilt;
  - consent not wired;
  - the stranded marker not shown by `doctor`;
  - the window between `VERIFY_DESTINATION` and `REMOVE_SOURCE` (§7, M6a);
  - DP-030, which is already covered.
- **How each is detected:**
  - `SNAPSHOT_BEFORE` and `RECORD_HISTORY`: `next_action` still answers
    `NotPerformedInThisBuild` for the kind;
  - consent: no caller of `consent::ask` exists in `crates/*/src` outside
    `consent.rs` and its tests;
  - the stranded marker: `doctor`'s check catalogue has no marker check id;
  - obligation 8, the M6a window and the release-build check: no structural
    fact exists yet. Each is an **unconditional failure while the constant is
    `true`**, named after its item, and removable only in the change that
    delivers the item with its evidence.
- **Proved to fire.** The mutation proof sets the constant to `true` in a
  scratch copy and shows each tripwire fails.

**D15 — what is carried, each with its reason.**
- **A stranded marker shown by `doctor`.** `doctor` reads only the snapshot, and
  showing the marker needs capture to read the state directory on a live run.
  That is an adapters change sized for slice 5 with `history`.
- **The refusal on an *observed* or *unchecked* provider.** DP-030 is unruled,
  and the detector is slice 6.
- **Shim-backed recognition.** SE-036 layer 4 does not exist.
- **The 0277 umask test** (DP-029).
- **`SNAPSHOT_BEFORE` performed.** It lands with `restore` in slice 5.
- **Obligation 8.** Tying the executed plan to the one shown needs a plan file
  and digest route, which nothing has yet.
- **L4's reconciliation** of a recorded run whose marker stayed behind, in
  slice 5.
- **The release-build feature check** (§7, L1), in the release slice.

Every one of these except the umask test has a flip tripwire (D14).

## 3. Where the code goes

| Change | File |
|---|---|
| `MutationAllowance`, `Action::DispatchMutating`, `Disposition::Performed`, the new halt reasons with `ends_before`, `next_action_with`, `plan_still_holds` | `crates/application/src/executor.rs` (+ `executor/tests.rs`) |
| `execution_refusals_with`, `clear`/`GateClear`, `mutation_allowance`, argv/elevation pins on mutating steps, D11 | `crates/application/src/execution_gate.rs` |
| `elevation_for` extracted from `command()` and shared `pub(crate)` | `crates/application/src/planner.rs` (no ordering change) |
| `halted_after_dispatch` for the new reasons, `Performed` | `crates/application/src/history.rs` |
| `elevation_requests`, `ConsentDecision`, `ConsentGiven` | `crates/application/src/consent.rs` (new) |
| `StepResult::Performed`, status rule, read-back | `crates/domain/src/history.rs`, `docs/schemas/history.v1.md` |
| `reread_configuration` | `crates/adapters/src/registry.rs` |
| `write_record` made durable; `ClearOutcome` doc; L3's symlink check in `clear` | `crates/adapters/src/persistence/{history_store,run_marker}.rs` |
| `MutationSite` (product impl refuses), `run_with_mutations`, `read_commands`/`mutation_dispatch`, `Observer` (product no-op) | `crates/cli/src/executor/mod.rs` |
| `consent::ask`, catalogue keys | `crates/cli/src/consent.rs` (new), `locales/en.toml`, `locales/pt-BR.toml` |
| state directory and euid once, entry refusals, closures | `crates/cli/src/app/migrate.rs`, `crates/cli/src/app/context.rs` |
| behavioural run-order test | `crates/cli/tests/executor_wiring.rs` (new) |
| D5 tripwire change and inverse tripwires; unconditional `wiring_violations`; flip tripwires (D14) | `crates/cli/tests/history_preconditions.rs` |
| `forced_in_tests` / constructor / `impl MutationSite` scans | `crates/cli/tests/executor.rs` |
| constant pin | `crates/application/tests/execution_gate.rs` |

No guarded path changes. No dependency; `Cargo.lock` unchanged. Nothing under
`crates/adapters/src/process/` changes.

## 4. The behavioural test (the definition of done's first box)

`crates/cli/tests/executor_wiring.rs` builds a `PlannedRun` from a recording.

**The narrowed plan.** It is narrowed with `map_plan`, and each edit is listed
in the test with its reason:
- `dry_run_only = false`;
- the four non-presence checks on `VERIFY_DESTINATION` are removed, keeping
  `PACKAGE_NEWLY_PRESENT`;
- `PROVIDER_REPORTS_ACTIVE` is removed from `SET_PRIMARY`;
- `VERIFY_RESOLUTION` gets the planner's presence check on the destination, so
  the gate's `VERIFICATION_ASSERTS_NOTHING` is not met. The loop still halts it
  `NoEvaluationForStepKind`.
- `check_ordering` must still pass.

It is a narrowed plan, not one the planner builds, and the test says so.

**How it runs.** The test obtains `forced_in_tests()` and `clear(...)`, then
runs `run_with_mutations` with:
- a scratch `StateDir`;
- a `MutationSite` fake;
- a `ReadSite` fake;
- recapture-and-replan and re-read closures;
- an `Observer`.

**The `Observer`:**
- Its methods return `()` and take only `Copy` identifiers: a step index and a
  boundary enum. Never a lock, a state directory, a token, plan text or a path.
- The no-op is the only product implementation, with no CLI or environment
  switch.
- Failures are injected only through the seams, never through the observer.

**The order is backed by real state checked inside the fakes**, not by observer
calls alone:
- When the `MutationSite` fake is called, the marker file exists and parses to
  this run's id, and `run.lock` cannot be locked from another file descriptor.
- The re-read closure runs after the lock check for its step.
- The recapture closure runs after the lock and before the marker.

**Assertions,** in both directions (D12):
1. The exact order, for a run that ends at `VERIFY_RESOLUTION` (`PARTIAL`, exit
   3), toward Homebrew. Toward MacPorts, the order up to the re-plan, then no
   marker, no dispatch and no record (D12).
2. Each boundary in turn made to fail: nothing dispatches after it, and the
   ending is the documented one, with its error id.
3. Endings against the marker (D8):
   - record durably written → marker cleared;
   - record write failed, or its sync failed → marker kept, exit non-zero, no
     restore offer;
   - nothing dispatched → marker cleared at once, no record;
   - `NotOurs` or `AlreadyAbsent` → `HistoryUnusable`, marker kept;
   - `clear` errs → stranded, non-zero, run id named;
   - partial-command halts (D4) → recorded.
4. A token for the wrong provider is refused, and nothing is dispatched.
5. Under `--replay`, with a forced allowance: refused before the probe, the
   state directory stays empty, and the observer sees nothing.
6. Without the allowance or the witness, the same plan gives today's refusal,
   and the observer sees nothing.
7. A plan edited so that elevation says `NONE` (D3) is refused.

**Mutation proof.** Every mutant must be killed, or argued equivalent in the
report:
- swap each adjacent pair of boundary operations (not only the observer calls);
- drop each operation;
- invert the token's provider comparison;
- clear the marker when the record failed;
- write the record with best-effort durability;
- count `MutationRefused` on command 1 as not dispatched;
- skip the live-scope refusal;
- read elevation from the plan field;
- dispatch with `None`;
- a witness from a different plan (a compile-time impossibility, argued in the
  report);
- the D6 cases of D6's tests (`invariant_04_*`, `invariant_19_*`,
  `PLAN_DIFFERS`);
- each flip tripwire, with the constant `true`.

## 5. Docs in the same changes

- `04-migration-and-safety.md`:
  - Concurrency and locking status;
  - History, durable record;
  - the Provider configuration residuals;
  - Privilege, for the consent path;
  - the M6a window as a residual that blocks the flip.
- `plan.v1.md` obligations: 5, 8 (carried), 9 (elevation re-derived, now
  enforced under an allowance), 10, 11 and 12.
- `history.v1.md`: `PERFORMED`.
- SE-092:
  - D1's deviation, D2's new input, the D5 tripwire entry, D8, and D9's amended
    commitment;
  - D12 carried, D13, D14, D15.
- SE-016: consent built, not wired.
- SE-050: NC (1) closed by `GateClear`.
- SE-094: D11, with its box carried.
- CHANGELOG: `[Unreleased]`.

## 6. Order of work

1. This plan, revised after review.
2. Domain and application types with their unit tests (D1–D4, D6, D11, D14's
   constant pin).
3. Adapters: the durable record, `reread_configuration`, L3.
4. The shell, the entry refusals and the behavioural test (D5, D7, D8, D13),
   with the tripwire changes.
5. Consent (D10).
6. The flip tripwires (D14).
7. Mutation proof.
8. Docs, reviewers, coverage, `validate.sh`, "As built".

One commit per step that compiles and passes the commit gate.

## 7. Review resolution (revision 1 → 2)

SR = `security-reviewer`, SG = `spec-guardian`.

| Finding | Disposition |
|---|---|
| SR H1 / SG 1 — elevation and dispatch decided from plan fields | Fixed: D2 (argv and elevation pins under an allowance), D3 (re-derived elevation, provider-target ADD/REMOVE only), D10 (requests from re-derived commands) |
| SG 2 — D6 must re-plan | Fixed: D6 re-plans and compares; the named fact is diagnosis only |
| SR H2 — best-effort record before a durable marker removal | Fixed: D8, the record is written with `write_new_durable`; an unconfirmed sync counts as not written |
| SR H3 / SG 3 — a partial halt counted as not dispatched | Fixed: D4, `ends_before`, and any command handed to the site counts as dispatched |
| SR M1 / SG 14 — `--replay` reaching the real state directory | Fixed: D13 entry refusal before the probe; §4 assertion 5 |
| SR M2 — constructor visible crate-wide; derives | Fixed: D1 scans and the no-derive rule, with a compile-time check |
| SR M3 / SG 17 — consent unbound; prompt underspecified | Fixed: D10 `ConsentGiven` per step, asked under the lock; TTY both ways; stderr; bounded read; exact ASCII phrase; escaping; home as `~` only for the invoking user |
| SR M4 — observer proves call order only | Fixed: §4 state checks inside the fakes; observer constrained |
| SR M5 / SG 8 (scan vs D12) | Moot: D12 carried, so no `forced_in_tests` call in `crates/*/src` |
| SR M6 (a) — no package re-check before `REMOVE_SOURCE` | Carried as a 04 residual with a flip tripwire (D14), next to DP-030 |
| SR M6 (b) — the prefix the site spawns under | Fixed: D7, the site refuses a prefix other than the one D6 verified |
| SR M6 (c) — which snapshot the re-check uses | Fixed: D7 uses `planned.before()` |
| SR M7 / SG 10, 12 — the flip is not enforced | Fixed: D14 constant pin and flip tripwires; `wiring_violations` unconditional (D5) |
| SG 4 — D12 impossible under D9 | D12 carried with its reason; SE-092's "with the write call" amended (D9) |
| SG 5 — the behavioural test cannot pass the gate | Fixed: §4 lists the narrowing edits; the run ends `PARTIAL` |
| SG 6 — the D5 token claim is false | Fixed: D5 moves `DISPATCH` to `mutation_dispatch(`, adds inverse tripwires and a dated SE-092 entry |
| SG 7 — `detected_prefix` ban | Fixed: D7 `registry::reread_configuration`; the CLI never holds a prefix |
| SG 8 — D1 deviates from §2's letter | Declared: D1, its own SE-092 entry with `cargo tree` evidence |
| SG 9 — `AlreadyAbsent` contract; `clear` errors | Fixed: D8 records the change and updates the doc; specifies the `Err` ending |
| SG 11 — obligation 8 missing | Carried (D15) with a flip tripwire (D14) |
| SG 13 — S8 item 6, root | Fixed: D13 cites `refuse_if_root`; `Ok(false)` refuses on the mutating path |
| SG 15 — assert the gate passed | Fixed: D2 `GateClear` witness |
| SG 16 — a mutating step's own verification dropped | Fixed: D3 halts `MutatingStepCarriesVerification` |
| SG D11 — the reason given | Revised: carried because the verification path has no binding to an inventory read, not because of the allowlist; DoD box carried |
| SG INV-17 risk — both directions | Fixed: D12 / §4 run both directions |
| SG §3 table — scan location | Fixed: the scan is in `crates/cli/tests/executor.rs` |
| SR L1 — `test-support` in an all-features release build | Carried to the release slice. Enforcement belongs in the release job, which is a guarded workflow staged under `docs/passes/09-staged/` and is the owner's to arm. A `compile_error!` under `not(debug_assertions)` is rejected, because it would break any `cargo test --release --all-features` a contributor runs |
| SR L2 — resolve the state directory once | Fixed: D13 |
| SR L3 — `clear` follows a symlink off Darwin | Fixed: §3, the same pre-open check `read` has |
| SR L4 — record and marker both present | Specified in D8; reconciliation carried to slice 5 (D15) |
| SR L5 — D2 is an SE-092 design change | Recorded: D2 |
| SR L6 — what halt reasons may hold | Fixed: D4; the record's `snapshot_path` home redaction stays owed (spec-evolution "Not closed" (b)) and is named in the report |
| SR (1) — `migrate` passing a literal `None` | Fixed: D1, `migrate` asks the gate |

## 8. Second review (revision 2 → 3)

SR = `security-reviewer`, SG = `spec-guardian`. Both found no Tier 1 or Tier 2
problem. Each found every first-round finding resolved in design, apart from the
items below.

| Finding | Disposition |
|---|---|
| SR N1 / SG N4 — `GateClear` not bound to its plan or allowance | Fixed: D2, `clear` takes both by value and the witness owns them |
| SR N2 — `ConsentGiven` mintable by any `cli` code | Fixed: D10, minted only by `consent::confirm`, which checks the phrase; callers scanned; values, not a digest |
| SR N3 — D3 pointed at a test helper | Fixed: D3, `elevation_for` extracted from the planner's `command()`, read-only relaxation, an agreement test |
| SR N4 — constructor gaps | Fixed: D1, the type lives in `execution_gate.rs`; the scan covers `derive(` |
| SR N5 / SG N5 — tripwire detections | Fixed: D14 names each detection, otherwise an unconditional failure; proved to fire |
| SR N6 — a record published with an unconfirmed sync | Fixed: D8 wording; never deleted; covered by the L4 reconciliation |
| SR N7 — restore after a refusal of command 0 | Carried to slice 5's backlog (D4) |
| SR N8 / SG 7 (low) — the re-read's prefix | Fixed: D7, `reread_configuration` takes the planned program and refuses disagreement |
| SR N9 — the history view read before the lock | Fixed: D6, re-read under the lock |
| SR N10 — a stranded marker invisible to `--json` | Fixed: D8, `markerLeft` in `migrate.v1` |
| SG N1 — no forced mutation toward MacPorts; `REMOVE_SOURCE` unreachable | Declared in D12 and §4; the application-level INV-01 test under an allowance is added (D3); the existing INV-01 test stays unchanged |
| SG N2 — second-command tests cannot reach the shell | Fixed: D4, application-level; one shell test on a single-command refusal; no new seam |
| SG N3 — `MutationSite::dispatch` contradicts the inverse tripwire | Fixed: D5 and D7, the method is `perform` |
| SG N6 — D11's reason incomplete | Fixed: D11 gives the complete reason and both alternatives; a `provider-researcher` note on MacPorts' verify output is owed |
| SG N7 — which snapshot; the gate needs none | Fixed: D2 and D3 |
| SG N8 — D6 tests by invariant | Fixed: D6 and §4's mutation list |
| SG D9 wording | Fixed: "no shipped build" |

## 9. As built — where the implementation departs from revision 3

Each departure was found while building and is recorded here rather than
fixed silently in the text above.

- **What the forced runs reach (D12, §4).** Revision 3 had the →MacPorts run
  be a consolidation. D11 made `PROVIDER_REPORTS_ACTIVE` unevaluable, and a
  consolidation's `VERIFY_DESTINATION` carried no other evaluable check, so the
  gate now refuses it as asserting nothing. Toward MacPorts the behavioural test
  narrows a plan's add away instead — as if the package had been installed by
  hand, because MacPorts' writes elevate and consent is not wired — which
  leaves `VERIFY_DESTINATION` with `PACKAGE_NEWLY_PRESENT` and Homebrew's
  unelevated removal. So **`REMOVE_SOURCE` is reached** by a forced run, unlike
  revision 3's claim, and `invariant_01_a_removal_is_dispatched_only_after_its_destination_verified`
  uses the same narrowing. `VERIFY_RESOLUTION` is removed by the narrowing
  rather than given a check: the gate refuses a command on it
  (`COMMAND_ON_STEP_THE_PLANNER_WRITES_NONE_ON`).
- **`SET_PRIMARY` with a command halts** (`UNEXECUTED_MUTATION`) rather than
  being passed over: passing over a step that carries a command would skip a
  planned change silently. The narrowing removes its command.
- **A check before a command that fails** — the lock lost, the configuration
  unreadable or drifted, a token for another provider — is a halt reason of its
  own, `PRECONDITION_FAILED { command, why }`, decided in the application
  (`MutationResult::PreconditionFailed`); the command named was not handed over,
  and the ends before it decide the dispatch.
- **The record write is not injectable.** The wiring tripwire wants the literal
  `history_store::write_record(` in the shell, so the behavioural test makes it
  fail for real (a file already at the record's name); a directory sync that
  fails after publish is tested at the adapter (`atomic.rs`'s seam), not in the
  shell.
- **`RunReport` gained `historyError` and `markerLeft`**, carried into
  `migrate.v1` (N10), and its exit code is the outcome's unless that is 0 and
  the history failed.
- **The recapture** is `Context::recapture`, which captures again through the
  same registry parts and machine-reader inputs as the first capture, stamped
  with the run's start (no second clock read). The state-directory override
  and the start instant are read once, at context build.
- **A failed provider command is `SAFETY_BLOCKED`.** No error identifier says
  "a provider command failed", and adding one is an exit-code contract change;
  carried (D15).
