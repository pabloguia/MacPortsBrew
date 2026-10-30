<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part S — finish slice 6: `PROVIDER_REPORTS_ACTIVE` evaluable, and the destination re-read before removal

Plan written 2026-10-28, before any code; revised the same day after the first
round of `spec-guardian` and `security-reviewer` (see Reviews). Their review is
the approval (work order 16 §4). **`THIS_BUILD_DISPATCHES_MUTATIONS` stays
`false` throughout** (§3 item 1). No mutating `brew` or `port` command runs
anywhere; every test runs on in-code plans and outputs or on
`fixtures/recordings/*.json` (§3 item 9).

## The ruling

Work order 16, part S: run `prompts/08-slice-6-verification-before-removal.md`
parts B and C.

- **Slice 6 part B.** `VERIFY_DESTINATION` can evaluate `PROVIDER_REPORTS_ACTIVE`,
  the check 04's verification contract names first, for **both** providers
  (INV-17), with reads already on the process allowlist. Ruled *for, with
  conditions* by `security-reviewer` at slice 6's plan review (SE-107, C1–C5).
  The facts are R-20 (`docs/research/R-20-what-a-read-shows-as-active.md`).
- **Slice 6 part C.** The destination is read again immediately before
  `REMOVE_SOURCE` is dispatched, with the verify step's own gate-pinned commands,
  under the run lock. DP-048 (c) accepts only the residual *after* that read.
- **The tripwire** `the_switch_needs_the_destination_contract_evaluable` is
  widened to every check a planner-built `VERIFY_DESTINATION` carries.
- **Slice 6 part D is superseded**, not built: DP-048 (c) and part K make removal
  opt-in. A switch-on tripwire naming DP-048 (c) and part K replaces part D's
  planned flip tripwire (Design §8). Recorded in SE-107, DP-048 and the slice 6
  plan (§9).

## Where it lives today

- **Semantics.** `SuccessSemantic` (`crates/domain/src/plan.rs` ≈110):
  `EXIT_STATUS`, `STDOUT_NAMES_PACKAGE`, `STDOUT_OMITS_PACKAGE`, `UNKNOWN`
  (`#[serde(other)]`; the gate refuses it as `UNKNOWN_SUCCESS`).
- **Judging.** `verification::expectation_for` (`crates/domain/src/verification.rs`
  ≈192) maps (step kind, semantic) to a private `Kind`; `evaluate` (≈336) fails
  every semantic on did-not-finish and truncation first. The domain has no JSON
  dependency.
- **Executor.** `executor::judge_step` (`crates/application/src/executor.rs`
  ≈780, public) judges each read, then requires every check to be
  `evaluable_in_this_build` and answered by a passed semantic (`answers`, ≈858;
  `false` for `PROVIDER_REPORTS_ACTIVE`, D11). `REMOVE_SOURCE` is gated only on
  the earlier `VERIFY_DESTINATION` **record** (`destination_verified_before`,
  ≈698, used by `mutating_action`, ≈661). `Action::DispatchMutating` carries no
  data.
- **Gate.** `evaluable_in_this_build` (`execution_gate.rs` ≈509): only
  `PACKAGE_NEWLY_PRESENT` on `VERIFY_DESTINATION` and `PACKAGE_ABSENT` on
  `VERIFY_REMOVAL`. The pin (≈665–703) derives **one** `(argv, success)` per
  verify step from the compiled table and refuses any command that differs
  (`COMMAND_IS_NOT_THE_PLANNERS_READ`) — every command against that one.
- **Shell.** `run_with` (≈234, read-only loop, takes the `RegistryContext`) and
  `run_with_mutations` (≈471–626). `Action::Dispatch` reads with `read_commands`
  and calls `judge_step`; `Action::DispatchMutating` per command: lock
  (`StillCurrent`) → the step's own provider's configuration re-read and
  re-check (`ConfigurationRechecked`, token by value) → `mutation_dispatch`.
  Nothing re-reads the destination, and no configuration re-check precedes the
  certifying `VERIFY_DESTINATION` read (SE-107 C1 unmet).
- **Read construction.** `registry::plan_read_spec` (`crates/adapters/src/registry.rs`
  ≈183) re-derives program and environment and checks argv against
  `PERMITTED_SUBCOMMANDS`; safe only after the gate's pin.
  `reread_configuration` (≈213).
- **Allowlist (frozen).** `process/mod.rs` already permits `brew info --json=v2
  --installed` (no positional) and `port -q installed [<name>]`. Every recording
  carries their output.
- **Commands table (frozen, §3 item 3).** `data/policy/provider-commands.toml`
  has `verify` and `verify_absent`; no active read. Its loader
  (`provider_commands.rs`) refuses: a success string other than the three known
  semantics (≈305–319); a provenance cell whose `read_on` is later than its
  entry's `read_on` (≈373–377; both entries say `2026-09-19`).
- **Policy version.** `data/policy/policy.toml` `version = "2026.10.12-4"`;
  `invariant_20_the_policy_version_names_the_newest_read_date_in_the_data`
  (`crates/policy/tests/invariants.rs` ≈554) ties it to the newest `read_on` in
  `data/policy/`. The version string is pinned in `fixtures/golden/*.analyze.json`
  (7), `docs/schemas/{analysis,doctor,explain,plan}.v1.md`, `README.md`,
  `CHANGELOG.md`, `wiki/{Getting-Started,FAQ}.md`, four
  `.github/ISSUE_TEMPLATE/*.yml` (not a guarded path), and part D's
  `docs/passes/16-plans/D-implementation.patch` (context lines, on
  `origin/wo16/D-service-definition`).
- **Tripwires** (`crates/cli/tests/history_preconditions.rs`):
  `the_switch_needs_the_destination_contract_evaluable` (≈1345, one check);
  `the_switch_needs_the_window_before_removal_closed_or_accepted` (≈1251, `!switch`).

## Design

### 1. The two reads (no allowlist growth; one frozen-file hunk, the owner's)

| Provider | `verify_active` argv | Success | Basis |
|---|---|---|---|
| Homebrew | `info --json=v2 --installed` (names no package) | `json_reports_package_linked` | R-20 finding 20 (E7, E8): `linked_keg` is Homebrew's record of the linked keg; `list --versions` shows none (finding 14) |
| MacPorts | `-q installed` (names no package) | `stdout_marks_package_active` | R-20 finding 19 (E3, E4, E5): `(active)` is the registry's record of activation |

- Both name no package: one shape for both providers (INV-17); governed by
  `names_no_package`; and replayable on outputs every recording already holds.
- **Rejected:** the snapshot's `active` flag (an inference, R-20 finding 12,
  SE-094); a name-carrying `brew info` (allowlist growth); MacPorts only
  (INV-17); a side data file outside `provider-commands.toml` (routing around
  §3 item 3; INV-20 puts provider argv in the one dated table).
- **The cells** (written by the session as patch text, applied as the owner's
  hunk, §10): per entry `verbs.verify_active`, `success.verify_active`,
  `elevation.verify_active`, `elevation.verify_active_gui_app`; Homebrew
  `none`/`none`, MacPorts `none`/`unknown` (mirroring `verify_absent_gui_app`,
  which the loader's paired `_gui_app` rule requires to exist — checked against
  the loader before the patch is written); provenance `source` citing R-20 E7/E8
  for Homebrew's success **and elevation** (E7: capture runs that read unelevated
  as uid 501), R-20 E3/E4 and R-5 E22–E24 for MacPorts; `read_on = "2026-10-18"`;
  and **each entry's own `read_on` moved to `2026-10-18`**, because the loader
  refuses a cell newer than its entry (≈373–377).

### 2. Domain: two semantics, one failure cause, JSON kept out (C2, C4)

- `SuccessSemantic` gains `StdoutMarksPackageActive`, `JsonReportsPackageLinked`.
  An older reader deserialises either as `UNKNOWN`, which every executor must
  refuse (`plan.v1.md` success field), so `plan.v1` does not bump.
- `FailureCause::ActiveNotReported`.
- `expectation_for`: `(VerifyDestination, StdoutMarksPackageActive)` →
  `Kind::Active(name)`, `(VerifyDestination, JsonReportsPackageLinked)` →
  `Kind::Linked(name)`, **only when argv names neither package**
  (`ArgvNamesAPackage` otherwise) and the destination name is safe; any other
  step kind → `SemanticNotValidForStep`.
- `evaluate` for both: did-not-finish and truncation first (shared, C4); then
  non-zero exit → `NonZeroExit`; no token line → `EmptyOutput`; then the
  **one rule for both providers (INV-17, D2): exactly one record of the package
  is marked active by the provider; a record of it in the provider's own
  not-active shape is ignored; two marked → not active; a record of it in any
  other shape → not active.**
  - `Active` (MacPorts): consider every row whose first token equals the name
    **literally** (no case folding). An **active row** is exactly three tokens,
    the second beginning with `@`, the third exactly `(active)`. An **inactive
    sibling** is exactly two tokens, the second beginning with `@` — the shape
    R-20 finding 7 reads from the source — and **only that exact shape is
    ignored** (security-reviewer, second round, Fix 1). Every other row of the
    name — four or more tokens (verbose output, R-20 finding 5), a third token
    other than `(active)`, a second token without `@`, one token — means the
    provider's output is not the shape this build knows, and the check fails
    `ActiveNotReported` however many active rows there are. Then exactly one
    active row passes; zero or two → `ActiveNotReported`.
  - `Linked` (Homebrew): `evaluate_with(expectation, output, linked: &dyn
    Fn(&str, &str) -> LinkedReport)`; the parser-less `evaluate` answers
    `ActiveNotReported` for `Linked`. `LinkedReport` (domain enum): `Linked`,
    `NotLinked`, `Ambiguous`, `Unparseable`; only `Linked` passes.
- **Deliberate fail-closed choices** (L7, recorded in SE-107): an inactive
  MacPorts image never passes (there is no positive inactive marker, R-20
  finding 2); the comparison is the registry's own spelling, so a plan whose
  destination name differs in case from the registry's halts rather than
  matching (R-20 finding 6).
- Rejected: a `serde_json` dependency in `domain` (layering).

### 3. Application: the parser, `answers`, the shared command rule, the gate

- **Parser (L3, C2, C3).** `application::verification_json::linked_report(stdout,
  name) -> LinkedReport`, private `#[derive(Deserialize)]` structs
  (`Document { formulae: Vec<Formula>, casks: IgnoredAny }`,
  `Formula { name: String, linked_keg: Option<String>, installed:
  Vec<Installed { version: String }> }`, unknown fields ignored) via
  `serde_json::from_str` over **the whole stdout** — leading or trailing data is
  a parse error; a duplicate `linked_keg` (or any duplicate field) is a parse
  error; a non-string `linked_keg` or `version` is a parse error. All →
  `Unparseable`. Then: entries whose `name` equals the destination exactly;
  **exactly one** (else `Ambiguous`, or `NotLinked` when none — a cask or not
  installed); its `linked_keg` must equal one of **its own**
  `installed[].version` (never `versions.stable`, never another entry's). Stderr
  is never read. `casks` is never consulted (R-20 finding 13). Nothing from the
  output becomes evidence, a path or a flag (C3): `ReadEvidence` keeps line
  count, truncation and cause only.
- **Keg-only (R-20 finding 10).** A keg-only formula that *is* linked passes; an
  unlinked formula (keg-only or not) halts `ActiveNotReported`. The effect is
  **direction-dependent** and stated in SE-107: it bites only MacPorts→Homebrew
  (Homebrew is the destination), where a freshly installed keg-only formula is
  not linked, so its `VERIFY_DESTINATION` halts; Homebrew→MacPorts is unaffected.
- **GUI apps (where they fail, stated in SE-107).** Toward MacPorts, a GUI-app
  `VERIFY_DESTINATION` is refused at the **gate**, because MacPorts'
  `verify_active_gui_app` elevation is `unknown` (`ELEVATION_UNKNOWN`). Toward
  Homebrew (a cask), the gate passes it and it halts at **run time**: the JSON
  has no linked record for a cask (`NotLinked` → `ActiveNotReported`). Both fail
  closed; part H owns turning either into a per-package "unchecked" outcome.
- `answers`: `PROVIDER_REPORTS_ACTIVE` is answered only by the two new
  semantics; `ExitStatus` and `StdoutNamesPackage` never (D11 kept).
- **One rule for planner and gate (V2, M2a).**
  `provider_commands::verify_destination_reads(table, provider, name) ->
  Vec<(argv, success)>` = `[verify] ++ [verify_active, if the compiled table
  holds that verb for the provider]`. The planner builds `VERIFY_DESTINATION`'s
  commands from it; the gate's pin derives its expected list from it and
  compares **element by element and in length** (a missing, extra, swapped or
  reordered command is `COMMAND_IS_NOT_THE_PLANNERS_READ`). In S-1 no table holds
  the verb, so every plan and every gate decision is exactly today's.
- **A new gate refusal (M2b).** `CHECK_HAS_NO_ANSWERING_READ { step, check }`,
  its own `ExecutionRefusal::key` ordinal (the next), matched in
  `crates/application/tests/execution_gate.rs`: refused when a check is
  `evaluable_in_this_build` on a verify step but no command of that step carries
  a semantic that `answers` it. It makes "evaluable" mean "evaluable by this
  plan", so a table that loses `verify_active` is refused at the gate, not halted
  at run time.
- `evaluable_in_this_build`: `PROVIDER_REPORTS_ACTIVE` → `VerifyDestination`
  only. Lands in S-2, with the cells.
- **Loader (V1, coupling check).** `provider_commands.rs` accepts the two new
  success strings (S-1 code, inert without cells), and refuses at load: either
  new semantic on a verb that is not a no-package verb (`names_no_package`); a
  `verify_active` without a paired `verify_active_gui_app` elevation cell; a
  `verify_active` sharing a non-flag word with the entry's `add`/`remove` (04's
  MUST). `READ_OPERATIONS` and `names_no_package` include `verify_active`.

### 4. Planner (inert until the cells land)

- `VERIFY_DESTINATION`'s commands come from `verify_destination_reads` (§3).
  **The step-kind sequence does not change** (INV-01 ordering untouched); a test
  pins it for every fixture plan.
- In S-2 every `VERIFY_DESTINATION` in `plan --json` gains a second command;
  documented samples are regenerated from the program's output.

### 5. C1: the certifying read follows a configuration re-check (D1)

- **Signature that makes it hold.** `judge_step` stays public for
  `VERIFY_REMOVAL` only: for a `VerifyDestination` position it returns
  `Halted { Unevaluable { why: NotCertifiedHere } }` **before looking at the
  reads**, so reads that would pass do not change it (test
  `judge_step_never_verifies_a_verify_destination_even_with_passing_reads`). A new public
  `certify_destination(run, position, checked: ConfigurationRechecked, reads) ->
  Disposition` is the only path to a `Verified` `VERIFY_DESTINATION`; it refuses
  (`PreconditionFailed { command: 0, why: WrongProviderToken }`) a token whose
  `provider()` is not `plan.destination` (L1). `ConfigurationRechecked` is
  constructed only by `recheck_provider_configuration`, so a certification
  without a fresh re-check does not compile.
- **The seam.** `next_action` answers a new `Action::Certify` for a
  `VERIFY_DESTINATION` (instead of `Dispatch`). Both shells sequence it the same
  way: re-read the destination's configuration →
  `recheck_provider_configuration` → `read_commands` → `certify_destination`.
  `run_with_mutations` gets the re-read through its existing `io.reread`.
  **`run_with` does not hold the `RegistryContext`** — it takes `sites: impl
  FnOnce() -> S` (`crates/cli/src/executor/mod.rs` ≈234; spec-guardian, second
  round) — so it gains a re-read seam parameter of the same type as `io.reread`
  (`impl Fn(&ProviderId, &str) -> Result<ProviderConfiguration, DomainError>`);
  `run` passes `registry::reread_configuration` bound to its `ctx`, and the
  `run_with` spy tests pass a fake for it (one returning drift, one unreadable,
  one clean). On a failed re-read the
  step halts `PreconditionFailed { command: 0, why: ConfigurationUnreadable }`;
  on drift, `ConfigurationDrift` (D5); nothing is read.
- Under `--replay` the re-read answers `UNKNOWABLE` and the re-check refuses, so
  a replayed certifying read halts (fail closed; stated in `plan.v1.md`
  obligation 6). No replayed run reaches it today.

### 6. Part C: the destination re-read immediately before `REMOVE_SOURCE`

- **The application decides which step and which commands (V4, L5).**
  `Action::DispatchMutating { reverify: Option<Reverify> }`, where
  `Reverify { verify_position, provider }` is computed by a pure, unit-tested
  `reverify_for(plan, position)`: `Some` for `REMOVE_SOURCE` only, naming the
  `VERIFY_DESTINATION` position before it and `plan.destination`; `None` for
  `ADD_DESTINATION`. The shell reads `plan.steps[verify_position].commands` —
  the verify step's own gate-pinned commands — and never calls `planned_argv`,
  `provider_commands` or the table (a word scan, §Tests).
- **Pure check.** `destination_still_verified(run, reverify: &Reverify,
  checked: ConfigurationRechecked, reads) -> Result<(), Precondition>` (token by
  value, L1): refuses a token for any provider but `plan.destination`
  (`WrongProviderToken`); re-derives each re-read command's elevation from the
  compiled table and errs unless `NONE` (L4, `DestinationNotReverified`); `Ok`
  only when `certify_destination` at `verify_position` is **exactly**
  `Verified` — any other disposition is `DestinationNotReverified`.
- **Shell order, `REMOVE_SOURCE` only, per command (M1):**
  1. destination configuration re-read and re-check (C1) — `ConfigurationUnreadable`
     / `ConfigurationDrift` on failure (D5);
  2. destination re-read with the verify step's commands, then
     `destination_still_verified` — `DestinationNotReverified` on failure;
     `Boundary::DestinationReverified` on success;
  3. `lock.verify_still_current()` — `LockLost`; `StillCurrent`;
  4. the source's configuration re-read and re-check — `ConfigurationRechecked`;
  5. `mutation_dispatch` — `Dispatched`.

  The slow read (Homebrew's JSON) is first, so the two cheap confirmations are
  the freshest facts at dispatch. **Each removal command gets its own steps 1–2**:
  `destination_still_verified` consumes its `ConfigurationRechecked` token by
  value, so a token cannot certify a second command, and a step with two
  removal commands re-reads the destination twice (test
  `each_removal_command_is_preceded_by_its_own_reread`). This rewords the slice 6 work order's "after
  the lock and configuration re-checks"; recorded in SE-107 (M1). All of it runs
  under the run lock taken at the run's start.
- **D5, identifiers.** A destination whose configuration cannot be re-read or
  has drifted keeps `CONFIGURATION_UNREADABLE` / `CONFIGURATION_DRIFT` (the
  halt's `command` and step say which provider); `DESTINATION_NOT_REVERIFIED`
  means the re-read ran and did not certify (vanished, inactive, truncated,
  failed, wrong token, non-`NONE` elevation).
- **On failure.** The removal command is not handed to the site;
  `judge_mutation` records `Halted { PRECONDITION_FAILED, command, why }`
  (`SAFETY_BLOCKED`); `ADD_DESTINATION` ran, so the outcome is `PARTIAL_SUCCESS`,
  exit 3 (INV-03). **D4: the re-read's evidence is not carried** —
  `MutationResult::PreconditionFailed` and `judge_mutation`'s halt have no
  evidence channel, and adding one is not needed for the decision; the halt's
  `why` is the record. (A later part may add it; not claimed here.)
  `DESTINATION_NOT_REVERIFIED` goes into `migrate.v1.md`'s `why` list (open set)
  and both locales.
- It inherits part B automatically: in S-2 the re-read runs both commands.
- **Rejected:** a second `VERIFY_DESTINATION` step (`check_ordering`, and it
  moves INV-01's enforcement point); trusting the earlier record alone; the
  slow read nearest the dispatch.
- **Hand-off to K (L6):** any elevation prompt for `REMOVE_SOURCE` must come
  **before** step 2, or 04's residual sentence must name the prompt's duration;
  no credential caching. Written into SE-107.

### 7. Docs

- **04**, check 1, per provider (Tier 3, in SE-107): MacPorts — exactly one
  image of the port marked `(active)` by the registry; Homebrew formulae — the
  keg Homebrew records as linked (`linked_keg`); a cask has no such record and
  cannot pass. **Check 1 is satisfied only together with `PACKAGE_NEWLY_PRESENT`
  on the same step** (L8): "active" is read from a whole inventory and does not
  by itself prove this run's install. The residual-window "Not yet met"
  sentence: the re-read exists (S); G, H and K do not yet.
- **`plan.v1.md`**: the two semantics; the checks table; obligation 6; the
  positional pin and the shared rule; `CHECK_HAS_NO_ANSWERING_READ`; C1 and
  `--replay`.
- **`migrate.v1.md`**: `DESTINATION_NOT_REVERIFIED`, `CHECK_HAS_NO_ANSWERING_READ`,
  `ACTIVE_NOT_REPORTED`. History schemas only if they enumerate them (checked).
- **Locales** `en`, `pt-BR`; i18n parity.
- **CHANGELOG**; SE-094's "believed" clauses cited as observed by R-20.

### 8. Tripwires

- **`the_switch_needs_the_destination_contract_evaluable`, widened:** builds the
  planner's `VERIFY_DESTINATION` in **each direction** and asserts, with the
  switch on, every check on it is `evaluable_in_this_build`, naming the ones that
  are not. After S-2, with the switch on, it fails naming exactly DP-051's four
  (part H). A companion assertion held whatever the switch is pins
  `PACKAGE_NEWLY_PRESENT` (and after S-2 `PROVIDER_REPORTS_ACTIVE`) evaluable.
- **`the_switch_needs_the_window_before_removal_closed_or_accepted`** becomes
  `the_destination_is_reread_before_removal` — a positive structural check held
  whatever the switch is: in the shell's `DispatchMutating` handling, the branch
  taken on `reverify: Some(..)` calls `destination_still_verified(` before
  `verify_still_current(` and before `mutation_dispatch(` (the shell has no
  `RemoveSource` arm of its own: which step is re-read is the application's
  `reverify_for`, spec-guardian, second round); and 04 carries the DP-048
  residual paragraph.
- **`the_switch_needs_removal_opt_in` (new, V3):** fails while the switch is on,
  naming DP-048 (c) and work order 16 part K (removal opt-in, the stronger
  confirmation, K's dispatch condition). It replaces slice 6 part D's planned
  flip tripwire; part K turns it into a positive check. Recorded in SE-107.
- `every_plan_this_planner_builds_is_refused_for_execution_in_this_build` and
  its fixture counterpart are not weakened.

### 9. The record

- **SE-107 amended in place** (SE-032, one topic): status `accepted (A, B, C
  built; D superseded)`; part B's design; the one INV-17 rule (D2); the
  deliberate fail-closed cases (L7); the keg-only decision and its
  direction-dependent effect; where GUI apps fail; C1–C5, each with its test;
  part C and the M1 order (rewording the work order); D5's identifiers; the new
  refusal; **part D superseded by DP-048 (c) and part K**, with
  `the_switch_needs_removal_opt_in` replacing its flip tripwire and K's tests
  replacing its `invariant_01_*`; L6's hand-off to K; the policy version bump
  and the owner's applied hunk (commit). If a reviewer prefers a new entry, it
  takes the next free identifier.
- **SE-050 item 6** amended (C5): the nameless `info --json=v2 --installed` in
  the run loop is an accepted residual, mitigated by C1 and DP-027 (b).
- **SE-109**: one propagation bullet.
- **DP-048** (D6): its "Reversible default taken" line gets "superseded by the
  ruling (c); slice 6 part D is not built (work order 16 part S, SE-107)", and
  next to it one line: "the re-read runs before the lock and source
  configuration re-checks, not after them (M1; SE-107)".
- **`docs/passes/08-slice-6-plan.md` §0** (D6): part D's row reads "superseded,
  not built (DP-048 (c), work order 16 part K; SE-107)", and B and C's rows
  point at `docs/passes/16-plans/S.md`.

### 10. Phasing, and how the frozen hunk lands (V1)

The table change cannot land on `main` alone: the loader refuses the new
success strings until S-1's code is there (≈305–319), the cells need their
entries' `read_on` moved (≈373–377), and `2026-10-18` then becomes the newest
date in `data/policy/`, failing
`invariant_20_the_policy_version_names_the_newest_read_date_in_the_data` until
`policy.toml`'s version moves `2026.10.12-4 → 2026.10.18-1` — which changes
every pinned copy (goldens, schema samples, README, CHANGELOG, wiki, issue
templates). So the owner's hunk and the session's bump arrive **on one topic
branch, `wo16/S-2-active`, cut after S-1 has merged**. (Checked 2026-10-28:
the newest `read_on` on `origin/main` is `2026-10-12`; `provider-configuration.toml`'s
`analytics_read_on = "2026-10-27"` is not counted by that test, whose scan
takes lines that start with `read_on` — a gap the test's own doc does not name,
recorded as a follow-up, not fixed here.)

| Step | Branch / PR | Contents | Gate |
|---|---|---|---|
| S-0 | `wo16/S-plan`, docs-only | this plan as `docs/passes/16-plans/S.md`; `docs/passes/16-staged/S/README.md` and `01-verify-active-cells.patch` (the `provider-commands.toml` hunk only, written against `origin/main` at writing; S-1 does not touch that file, so it still applies after S-1; checked with `git apply --check` on a fresh `git archive origin/main`, alone and after P's four patches) | — |
| S-1 | `wo16/S-1-reread` | §2, §3 (all but `evaluable` true), §5, §6, the loader acceptance and coupling checks, §8's two structural tripwires and the widened destination tripwire (still listing five), docs for what landed. Inert without the cells | merged; ledger `waiting on owner docs/passes/16-staged/S/` |
| S-2 | `wo16/S-2-active` | **only after S-1 has merged.** Commit 1, the owner's (default route): the owner applies and commits the staged patch himself on `wo16/S-2-active`. Commit 2, the session's: `policy.toml` to `2026.10.18-1` and every pinned copy regenerated from the program's output. Then `evaluable` true, the planner tests, replay assertions, samples, SE-107 final | `git apply --reverse --check docs/passes/16-staged/S/01-verify-active-cells.patch` succeeds on the S-2 head and, after merge, on `origin/main` |

- **The owner's commit is red on its own, and that is expected:** without the
  bump, `invariant_20_the_policy_version_names_the_newest_read_date_in_the_data`
  fails at commit 1. The commit gate is a Claude Code hook and does not run on
  the owner's own terminal commit; the session's commit 2 runs it (default mode)
  and must pass; CI judges the pull request's head.
- **The armed variant, only if the owner chooses it** (security-reviewer, second
  round, Fix 2): the owner sends the armed sentence the README gives, and in
  that window the session (a) writes `data/policy/provider-commands.toml` only
  by `git apply docs/passes/16-staged/S/01-verify-active-cells.patch` — no other
  write to that file, by any tool; (b) before committing, runs `git apply
  --check` of the patch on the parent commit and checks that `git diff <parent>
  HEAD -- data/policy/provider-commands.toml` is byte-equal to the patch's diff
  for that file, and records both results with the commit in the ledger; (c)
  makes every other change of that commit (the bump, the regenerated copies) as
  ordinary edits to unguarded files. The ledger quotes the owner's message and
  names the commit, and `security-reviewer` confirms (b) before the part
  continues. The same byte-equality check is run on the owner's commit in the
  default route and recorded the same way.

- **Reconciling with part D.** `D-implementation.patch` carries
  `2026.10.12-4` in context lines. Whichever of D and S-2 merges second rebases
  onto the other's version: if D lands first, S-2 bumps from D's version; if
  S-2 lands first, D's patch is regenerated against the bumped files before it
  is applied. If D also adds a newer `read_on`, the later part takes the newest
  date. The ledger records which.
- S-1 is safe alone: no table holds `verify_active`, so `verify_destination_reads`
  yields `[verify]`, plans and gate decisions are today's, `PROVIDER_REPORTS_ACTIVE`
  stays refused as not evaluable, and C's re-read runs the single pinned `verify`.

## Tests

Every invariant test named `invariant_NN_*`; in-code plans and outputs, or
recorded outputs.

**Domain**
- `active_marker_must_be_on_the_named_line` (`xz` not answered by
  `xz-bootstrap`; `gh` not by `ghidra`); `the_name_is_compared_without_case_folding`
  (`XZ` vs `xz` halts, L7).
- `an_inactive_sibling_is_ignored_and_a_lone_inactive_row_is_not_active`
  (documentation-derived shape, R-20 finding 2; the test says so);
  `a_verbose_row_is_not_active`; `a_second_token_without_at_is_not_active`;
  `any_row_of_the_name_in_an_unknown_shape_fails_even_beside_an_active_row`
  (one token, four tokens, a third token other than `(active)`, a two-token row
  whose second token lacks `@` — each beside a valid active row; Fix 1).
- `two_active_rows_for_one_name_are_not_active`.
- `active_read_fails_closed` (did-not-finish, truncated, non-zero, empty).
- `a_verify_active_read_that_names_a_package_has_no_expectation`;
  `the_new_semantics_are_valid_on_verify_destination_only`;
  `the_parserless_evaluate_never_passes_a_linked_read`.
- `invariant_17_exactly_one_record_marked_active_is_the_rule_for_both_providers`
  — the same four cases (one marked; one marked plus an unmarked sibling; none;
  two marked) through each provider's semantic, same verdicts.

**Application**
- `linked_keg_equal_to_its_own_installed_version_is_linked` (revision suffix;
  never `versions.stable`); `a_linked_keg_matching_another_entrys_installed_is_not_linked`.
- `unlinked_cask_missing_or_malformed_is_not_linked` (null, number, no entry,
  cask only, truncated, non-JSON); `leading_or_trailing_text_is_unparseable`;
  `a_duplicate_linked_keg_is_unparseable` (L3).
- `a_keg_only_formula_that_is_linked_is_active`; `two_entries_of_one_name_are_ambiguous`;
  `a_linked_keg_spoofed_in_caveats_is_not_read` (C2).
- `an_installed_read_never_answers_provider_reports_active` (D11).
- `planner_and_gate_share_one_verify_destination_rule` (V2): with the shipped
  table in S-1 the list is `[verify]`; the pin refuses a missing, extra,
  swapped or reordered command, including a shorter list (zip-without-length).
- `a_check_evaluable_but_unanswered_by_the_steps_reads_is_refused` (M2b).
- Loader: `the_new_semantics_require_a_no_package_verb`;
  `verify_active_requires_its_gui_app_elevation_cell`.
- `certify_destination_refuses_a_token_for_another_provider` (L1);
  `judge_step_never_verifies_a_verify_destination_even_with_passing_reads` (D1);
  `each_removal_command_is_preceded_by_its_own_reread` (a two-command removal
  step in the shell's fake: two re-reads, two tokens).
- `reverify_for_names_the_verify_step_for_remove_source_only` (V4).
- `invariant_01_removal_refused_when_the_destination_vanished_after_verify` —
  **both directions**; `invariant_01_removal_refused_when_the_reread_is_truncated_failed_or_unjudged`;
  `invariant_01_destination_still_verified_requires_exactly_verified`;
  `invariant_01_a_destination_token_for_the_wrong_provider_is_refused`;
  `invariant_01_a_reread_command_whose_elevation_is_not_none_is_refused` (L4).
- `a_certifying_read_follows_a_configuration_recheck` (C1: unreadable and drift
  each halt before any read).
- `the_planner_step_sequence_is_unchanged` (INV-01 ordering).
- **S-2:** `provider_reports_active_is_evaluable_on_verify_destination_only`
  (replaces the `false` rows in `tests/execution_gate.rs` ≈343–352);
  `the_shipped_table_holds_verify_active_for_every_provider` (V2);
  `invariant_20_every_cell_with_its_own_evidence_says_so_in_a_field` updated
  with the new cells and dates; `invariant_20_the_policy_version_names_the_newest_read_date_in_the_data`
  passing at `2026.10.18-1`.
- **S-2 replay, `invariant_15_*`** over each recording's `brew info --json=v2
  --installed` and `port -q installed` outputs: for each recorded package the
  verdict equals the recording's own `linked_keg` / `(active)` field, a cask
  never passes, and the expected counts are **pinned literally** (counted once
  from the recording when the test is written). **Labelled
  documentation-derived** where the recording is synthesised or derived (R-20
  E12; fixtures README): `macports-only-intel-macos12.json` and
  `both-providers-arm64-macos15.json`. `both-providers-intel-macos26.json` and
  `homebrew-only-intel-macos26.json` are labelled by what the README says of
  them.

**CLI**
- `executor_wiring.rs` order test (M1): `ConfigurationRechecked` (destination)
  → `DestinationReverified` → `StillCurrent` → `ConfigurationRechecked` (source)
  → `Dispatched` for `REMOVE_SOURCE`; none of the first two on
  `ADD_DESTINATION`.
- `invariant_01_the_reread_uses_the_verify_steps_own_commands` (shell half):
  the fake site's received argv for the re-read equals the verify step's
  commands; a plan whose verify step carries different commands is re-read with
  those, not with any derived argv; **`the_shell_never_derives_a_command`**: a
  word scan that `crates/cli/src/executor/` never names `planned_argv`,
  `provider_commands` or `verify_destination_reads` (kills mutant 18
  structurally).
- `invariant_03_a_removal_whose_destination_was_not_reverified_ends_partial` —
  no `Dispatched`, `PARTIAL_SUCCESS`, exit 3, `why: DESTINATION_NOT_REVERIFIED`
  in `--json`.
- The three tripwires of §8; i18n parity and `hostile_text.rs` for the new keys.
- In S-2 the `Direction::cleared()` fakes answer the second command; the
  `.retain(evaluable_in_this_build)` narrowing in `executor_wiring.rs`,
  `cli/tests/executor.rs` and `application/tests/executor.rs` then keeps
  `PROVIDER_REPORTS_ACTIVE`.

## Mutants (isolated worktree, by a subagent; removed afterwards)

Each must be killed; a survivor is argued in writing.

1. Drop the `(active)` third-token test.
2. Substring or prefix name match. 3. Case-folded name comparison.
4. Accept a two-token row as active. 5. Accept two active rows.
6. Drop the `@` test on the second token.
7. Truncation / did-not-finish moved after the semantic match.
8. Drop the exit-status check for the new semantics.
9. `expectation_for` drops "argv names no package" for the new semantics.
10. `linked_report`: compare to `versions.stable`.
11. `linked_report`: match against another entry's `installed[]`.
12. `linked_report`: accept a non-string `linked_keg` (untyped parse).
13. `linked_report`: first of several same-name entries.
14. `linked_report`: search raw text (caveats spoof).
15. `linked_report`: a cask entry counts as linked.
16. `answers`: an installed read answers `PROVIDER_REPORTS_ACTIVE`.
17. `evaluable_in_this_build` true on every step kind.
18. Re-read with another step's commands or with table-derived argv (killed
    structurally by `the_shell_never_derives_a_command`, plus the shell test).
19. Gate pin checks command 0 only / compares as a set / zips without a length
    check.
20. Drop `CHECK_HAS_NO_ANSWERING_READ`.
21. Widened tripwire checks one check only.
22. Drop C1's re-check; move it after the certifying read.
23. `certify_destination` ignores the token's provider.
24. Drop the re-verify before `REMOVE_SOURCE`.
25. Drop the destination configuration re-check before the re-read.
26. Move the slow re-read after the source configuration re-check (M1).
27. `destination_still_verified` ignores the token's provider.
28. `destination_still_verified` accepts anything not `Halted` instead of
    exactly `Verified`.
29. `destination_still_verified` skips the elevation re-derivation.
30. `reverify_for` returns `Some` for `ADD_DESTINATION` (wrong step kind).
31. Planner emits `verify_active` before `verify`.
32. Loader accepts a new semantic on a verb that names a package.
33. Ignore any row of the name that is not the active shape (instead of only
    the exact inactive-sibling shape).
34. `judge_step` evaluates the reads for a `VERIFY_DESTINATION` and returns
    `Verified` when they pass (the `NotCertifiedHere` early return removed).
35. The shell re-reads once per removal step instead of once per command.

## Needs the owner

1. **Apply the staged commands-table change for S-2** (blocks S-2, and so part
   E). After S-1 merges, by default: the owner checks out `wo16/S-2-active`
   (the session pushes it cut from `origin/main`), runs `git apply
   docs/passes/16-staged/S/01-verify-active-cells.patch`, and commits it
   himself (`docs/passes/16-staged/S/README.md` gives the exact commands). The
   patch touches only `data/policy/provider-commands.toml` — a verb, no new
   program or subcommand, so `process/mod.rs` is unchanged. Alternatively the
   owner may choose the armed variant (§10). Applied iff `git apply --reverse
   --check` succeeds on `origin/main` after merge; the ledger records the commit
   and the byte-equality check.
2. **Optional capture, not blocking:** an inactive MacPorts image and a fresh
   `brew install` of a keg-only and a non-keg-only formula, on a disposable
   machine, with `scripts/capture-fixture.sh` (R-20 "Cannot determine"). Until
   then those cases are labelled documentation-derived; nothing is invented.

## Handed to later parts

- **H:** `ACTIVE_NOT_REPORTED` for a cask or an unlinked formula, and MacPorts'
  gate refusal of a GUI app, are DP-051's "cannot be evaluated for this
  package"; H's unchecked outcome applies.
- **K:** "S's re-read ran" is `destination_still_verified` returning `Ok`; K's
  INV-01 test cites `invariant_01_removal_refused_when_the_destination_vanished_after_verify`;
  `the_switch_needs_removal_opt_in` becomes positive; L6 (elevation prompt
  before the re-read, or named in 04; no credential caching).

## What this part does not do

No change to `THIS_BUILD_DISPATCHES_MUTATIONS` or to `process/mod.rs`; no edit
to `provider-commands.toml` except the owner's hunk in S-2; no new dependency,
`Cargo.lock` unchanged; no guarded path; the planner's step-kind sequence and
`check_ordering` untouched (INV-01); no release-policy removal refusal (slice 6
part D, superseded); none of DP-051's four checks (H); no lock probe (G); no
opt-in removal or confirmation levels (K); no elevation; no mutating command;
`01-invariants.md` not edited.

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
| spec-guardian | plan, first draft | **not approved.** V1 the staged table change cannot apply alone (loader refuses the semantics; entry `read_on`; policy version and every pinned copy; D's patch); V2 one planner/gate rule, S-1 unchanged, S-2 table test; V3 keep a switch-on assertion naming DP-048 (c) and K; V4 the application decides which step and which commands; D1 the signature behind C1 and `run_with`'s seam; D2 one INV-17 "exactly one active" rule and its test; D3 label derived recordings, pin counts; D4 evidence channel; D5 identifiers; D6 supersession notes in DP-048 and the slice 6 plan; plus: mutant 18 equivalent, six more mutants, `invariant_03_*` name, both directions for the vanished test, S-2 `invariant_20_*` update, load-time coupling check, the "different argv" half to the shell, where GUI apps fail, the direction-dependent keg-only effect | V1 §1 (entry `read_on`), §10 (S-2 after S-1, owner's hunk and the bump in one commit, D reconciled); V2 §3 `verify_destination_reads`, test `planner_and_gate_share_one_verify_destination_rule`, S-2 `the_shipped_table_holds_verify_active_for_every_provider`; V3 §8 `the_switch_needs_removal_opt_in`, §9; V4 §6 `DispatchMutating { reverify }`, `reverify_for`; D1 §5 `certify_destination`, `Action::Certify`; D2 §2 rule and `invariant_17_exactly_one_record_marked_active_*`; D3 Tests (labels, literal counts); D4 claim dropped (§6); D5 §6; D6 §9; the rest in Tests and Mutants (18 killed structurally; 3, 9, 19, 22, 25, 28 added), §3 (GUI apps, keg-only) |
| security-reviewer | plan, first draft | **approve with fixes.** M1 slow read first, cheap checks nearest dispatch, record the rewording, reverse mutant 17, pin the order; M2 one rule + a gate refusal for an evaluable check no read answers; L1 token by value, provider checked in both places; L2 two parser/row mutants; L3 typed whole-document parse, duplicates and leading/trailing data refused, no stderr; L4 re-derive elevation in the re-read; L5 = V4; L6 K hand-off on elevation prompts and caching; L7 inactive image and case as deliberate fail-closed; L8 check 1 depends on `PACKAGE_NEWLY_PRESENT` in 04; cite R-20 E7 for Homebrew elevation; check the loader's semantic strings and `_gui_app` rule | M1 §6 order, SE-107, mutant 26, wiring test; M2 §3 shared rule and `CHECK_HAS_NO_ANSWERING_READ`; L1 §5, §6, mutants 23, 27; L2 mutants 6, 11, 12; L3 §3 parser, three tests; L4 §6, test, mutant 29; L5 §6; L6 §6, Handed to K; L7 §2, SE-107; L8 §7; E7 cited in §1; loader in §3 |
| spec-guardian | revised plan | **approved**; four lows: (1) DP-048 pointer for the M1 reorder; (2) the README's default route is the owner applying and committing the patch himself, the armed variant only by his choice, with the ledger quoting his message and commit and `security-reviewer` confirming; (3) `run_with` takes `sites: impl FnOnce() -> S`, not the `RegistryContext`, so it needs a re-read seam; (4) word the structural tripwire for the `reverify: Some(..)` branch | (1) §9 DP-048 line; (2) §10, Needs the owner, `S/README.md`; (3) §5, the seam parameter and its fakes; (4) §8 |
| security-reviewer | revised plan | **approved, two low fixes**: Fix 1 only the exact inactive-sibling shape is ignored, every other row of the name fails (test, mutant); Fix 2 the frozen file changes by exactly the staged patch ((a) only `git apply`; (b) `git apply --check` on the parent and a byte-equal `git diff`, in the ledger; (c) every other write ordinary); plus a test that `judge_step` returns `NotCertifiedHere` with passing reads (mutant), and one destination re-check per removal command | Fix 1 §2, test `any_row_of_the_name_in_an_unknown_shape_fails_even_beside_an_active_row`, mutant 33; Fix 2 §10 (armed variant, and the same check on the owner's default commit); §5 and test `judge_step_never_verifies_a_verify_destination_even_with_passing_reads`, mutant 34; §6 and test `each_removal_command_is_preceded_by_its_own_reread`, mutant 35 |
| spec-guardian | S-1 code, `a6f6263` (plus the uncommitted docs) | **no Tier 1 violation**; two to fix: (1) mutant 35 survived (`each_removal_command_is_preceded_by_its_own_reread` cannot tell once per step from once per command with one removal command); (2) SE-107 cited a "Mutation proof" section that did not exist; lows: stale 04 "still owed" sentence, an application-level `invariant_17_*` for the JSON parse, stale pointers to the moved tripwire, the shell half tested one way, the residual window understated in 04 | (1) `reread_violations` requires the re-read inside `for command in &step.commands`, with a mutant case; mutant 35 now killed; (2) this section; the rest fixed in S-1's second commit (04 both sentences, `invariant_17_exactly_one_record_marked_active_holds_for_the_json_parse`, the module header and `08-pre-flip-design-notes.md`, SE-107's one-way note); deviations (a)–(d) judged accepted |
| security-reviewer | S-1 code, `a6f6263` | **no critical, high or medium**; L-1 = mutant 35; L-2 the order test silent if no removal ran; L-3 the loader accepts a non-active semantic on `verify_active`; Info-1 `reverify_for` takes the first `VERIFY_DESTINATION`; Info-2 the `DESTINATION_NOT_REVERIFIED` text narrower than the precondition | L-1 as above; L-2 the order test asserts a removal ran in one direction; L-3 the loader now requires an active semantic on `verify_active` and allows the two only there (`verify_active_takes_an_active_semantic_and_only_it_does`, mutant 36); Info-1 `rposition`, the nearest before the removal; Info-2 both locales reworded |

| security-reviewer | PR #33 | **approved**; L-1 a distinct cause for a changed output shape; L-2 `linked_keg` required but nullable; I-3 the drift test both ways if cheap | L-1 `OUTPUT_SHAPE_UNKNOWN` (both locales, both schemas), `an_unknown_shape_is_its_own_cause_and_inactive_is_another`, mutants 37 and 38; L-2 `a_formula_without_the_linked_keg_key_is_unparseable`, mutant 39; I-3 declined (As built, deviation 5) |
| spec-guardian | PR #33 | **approved**; (1) the lock-lost test lost its full assertion; (2) the step-sequence test pins two in-code plans; (3) mutant 27's wording; (4) halt a removal with no step to re-read | (1) restored and strengthened; (2) As built, deviation 6; (3) row 27; (4) `dispatch_for`, `a_removal_with_no_step_to_reread_is_never_dispatched`, mutant 40 |

## Mutation proof

Run 2026-10-28 in an isolated copy of the S-1 tree (one copy, its own target
directory, `cargo clean`ed and deleted afterwards), by a throwaway harness
outside the repository: each mutant applied alone, the named crate's tests run,
the file restored. Killed means at least one test failed (none needed a compile
error). Round 1 ran mutants 1–34 on `a6f6263`; five survived, the tests below
were added, and round 2 re-ran the survivors and the three new mutants on the
second commit.

| # | Mutant | Result | Killed by |
|---|---|---|---|
| 1 | drop the `(active)` third-token test | killed (round 2) | `a_lone_row_with_another_third_token_is_not_active` (added: beside a valid row the mutant was masked by the two-marked rule) |
| 2 | substring name match | killed | `active_marker_must_be_on_the_named_line` |
| 3 | case-folded comparison | killed | `the_name_is_compared_without_case_folding` |
| 4 | a two-token row active | killed | `an_inactive_sibling_is_ignored_and_a_lone_inactive_row_is_not_active`, `invariant_17_*` |
| 5 | two active rows accepted | killed | `two_active_rows_for_one_name_are_not_active`, `invariant_17_*` |
| 6 | drop the `@` test | killed | `a_second_token_without_at_is_not_active`, `any_row_of_the_name_in_an_unknown_shape_fails_even_beside_an_active_row` |
| 7 | truncation not failed first (dropped) | killed | `active_read_fails_closed` |
| 8 | drop the exit-status check | killed | `active_read_fails_closed` |
| 9 | `expectation_for` drops "names no package" | killed | `a_verify_active_read_that_names_a_package_has_no_expectation` |
| 10 | any `linked_keg`, not its own installed version | killed | `linked_keg_equal_to_its_own_installed_version_is_linked`, `a_linked_keg_matching_another_entrys_installed_is_not_linked` |
| 11 | another entry's `installed[]` | killed | `a_linked_keg_matching_another_entrys_installed_is_not_linked` |
| 12 | a non-string `linked_keg` accepted | killed | `unlinked_cask_missing_or_malformed_is_not_linked` |
| 13 | first of several same-name entries | killed | `two_entries_of_one_name_are_ambiguous` |
| 14 | raw-text search | killed | `a_linked_keg_spoofed_in_caveats_is_not_read` and three more |
| 15 | a cask counts | killed | `unlinked_cask_missing_or_malformed_is_not_linked` |
| 16 | an installed read answers `PROVIDER_REPORTS_ACTIVE` | killed | `an_installed_read_never_answers_provider_reports_active` |
| 17 | `evaluable_in_this_build` true everywhere | killed | `a_recognised_check_is_not_thereby_evaluable` |
| 18 | re-read with another step's commands | killed | `invariant_01_the_reread_uses_the_verify_steps_own_commands`, the order test; and structurally `the_shell_never_derives_a_command` |
| 19 | gate pin zips without a length check | killed | `planner_and_gate_share_one_verify_destination_rule`, `a_forged_verification_command_is_refused` |
| 20 | drop `CHECK_HAS_NO_ANSWERING_READ` | killed | `a_check_evaluable_but_unanswered_by_the_steps_reads_is_refused` |
| 21 | widened tripwire checks one check | killed | `the_switch_needs_the_destination_contract_evaluable` |
| 22 | drop C1's re-read before the certifying read | killed | `a_certifying_read_follows_a_configuration_recheck`, `a_replayed_certifying_read_halts_before_it_is_made` |
| 23 | `certify_destination` ignores the token's provider | killed | `certify_destination_refuses_a_token_for_another_provider` |
| 24 | drop the re-verify before `REMOVE_SOURCE` | killed | `invariant_03_a_removal_whose_destination_was_not_reverified_ends_partial` and three more |
| 25 | drop the destination configuration re-check before the re-read | killed | `a_destination_configuration_that_drifted_stops_the_removal_before_the_reread` (added for it) |
| 26 | slow re-read after the lock and source re-checks | killed | the order test; and structurally `the_destination_is_reread_before_removal` |
| 27 | `destination_still_verified` ignores the token's provider | **survives, safety-equivalent** | it differs only in the reported reason when the elevation check also fails: the token is passed on to `certify_destination`, which refuses it and is mapped back to `WRONG_PROVIDER_TOKEN`, so the removal is refused either way. `invariant_01_a_destination_token_for_the_wrong_provider_is_refused` pins the observable behaviour |
| 28 | `destination_still_verified` accepts anything not `Halted` | **survives, equivalent** | `certify_destination` returns only `Verified` or `Halted` (`judge_reads` has no other ending), so the extra arms are unreachable; the exhaustive match is kept so a new ending must be placed |
| 29 | skip the elevation re-derivation | killed (round 2) | `invariant_01_a_reread_command_whose_elevation_is_not_none_is_refused` (its GUI-app half was paired with the wrong snapshot in round 1; fixed, with a control that the same reads certify) |
| 30 | `reverify_for` answers `ADD_DESTINATION` | killed (round 2) | `reverify_for_names_the_verify_step_for_remove_source_only` (a doctored add after the verify step added; in planner plans the add precedes the verify step, so round 1 could not see it) |
| 31 | `verify_active` before `verify` | killed | `the_new_semantics_are_accepted_on_a_no_package_verb` |
| 32 | loader accepts an active semantic on a name-taking verb | killed | `the_new_semantics_require_a_no_package_verb` |
| 33 | any row of the name in another shape ignored | killed | `any_row_of_the_name_in_an_unknown_shape_fails_even_beside_an_active_row` |
| 34 | `judge_step` certifies a `VERIFY_DESTINATION` | killed | `judge_step_never_verifies_a_verify_destination_even_with_passing_reads` |
| 35 | the re-read once per step | killed (round 2) | `a_destination_configuration_that_drifted_stops_the_removal_before_the_reread`, `invariant_03_*`; and structurally `reread_violations` (the re-read inside `for command in &step.commands`, its mutant case in `the_reread_tripwire_fires_on_its_mutants`) |
| 36 | loader accepts an installed semantic on `verify_active` (security-reviewer L-3) | killed | `verify_active_takes_an_active_semantic_and_only_it_does` |
| 37 | a row of the name in an unknown shape reported `ACTIVE_NOT_REPORTED` (PR #33 review, L-1) | killed | `an_unknown_shape_is_its_own_cause_and_inactive_is_another` and two more |
| 38 | an unreadable JSON document reported `ACTIVE_NOT_REPORTED` (L-1) | killed | `the_parserless_evaluate_never_passes_a_linked_read` |
| 39 | `linked_keg` optional again (L-2) | killed | `a_formula_without_the_linked_keg_key_is_unparseable` |
| 40 | a removal dispatched with no step to re-read (spec-guardian 4) | killed | `a_removal_with_no_step_to_reread_is_never_dispatched` |

Mutants 37–40 were run in this worktree, each applied alone with its file
restored afterwards, against the crate's own tests.

## As built (S-1)

Branch `wo16/S-1-code`. `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`; no
change to `data/`, `fixtures/`, `crates/adapters/`, `Cargo.toml` or
`Cargo.lock`; the staged table patch is untouched and still the owner's (S-2).

- **As designed:** §2 (domain), §3 except `evaluable` (S-2), §4, §5, §6, the
  loader checks, §8's three tripwires, and the docs of §7 for what landed (04,
  `plan.v1.md`, `migrate.v1.md`, both locales, CHANGELOG) and §9's records
  (SE-107 amended in place, SE-050 item 6, SE-109, DP-048, the slice 6 plan
  §0).
- **Deviations, each reviewed and accepted by spec-guardian:**
  1. `the_switch_needs_the_destination_contract_evaluable` lives in
     `crates/cli/tests/executor.rs`, not `history_preconditions.rs`: it plans
     in each direction from in-code snapshots, because the recordings do not
     record MacPorts' service facts and the planner refuses to plan
     MacPorts→Homebrew from them. It pins the five checks it names today; S-2
     makes that four.
  2. `each_removal_command_is_preceded_by_its_own_reread` counts re-reads
     against handed commands with a one-command removal (the gate refuses a
     two-command removal step); "per command" is held structurally by
     `reread_violations` and behaviourally by mutant 35's kills.
  3. Under `--replay` the product's `run` halts every certifying read
     (`a_replayed_certifying_read_halts_before_it_is_made`), so the shell tests
     that exercise reads past that seam use `run_with` with a clean re-read.
     S-2's replay `invariant_15_*` tests must go through `run_with` too.
  4. The in-code test snapshots record their configuration as read and empty
     instead of `NotRecorded`, so a certifying read can follow a re-check.
  5. The shell half of the re-read is reachable toward MacPorts only until
     part K (a removal from MacPorts elevates); the application half is tested
     both ways (SE-107). For the same reason
     `a_destination_configuration_that_drifted_stops_the_removal_before_the_reread`
     runs toward MacPorts only: toward Homebrew the removal is MacPorts' and
     the gate refuses it (security-reviewer I-3, declined as not cheap).
  6. `the_planner_step_sequence_is_unchanged` pins the two in-code install
     plans, one per direction — not every fixture plan, as the Tests section
     says. `every_plan_this_planner_builds_from_every_fixture_is_refused_for_execution`
     still covers the fixture plans for the gate, and S-1 changes no step
     kind (spec-guardian, PR #33 review, 2).
- **From the PR #33 review (both reviewers approved; low fixes):** a
  distinct cause `OUTPUT_SHAPE_UNKNOWN` ("the provider output changed") for an
  unreadable JSON document and for a row of the name in an unknown shape;
  `ACTIVE_NOT_REPORTED` stays for none or two marked, for `NotLinked`, and —
  the session's call — for `Ambiguous`, the JSON form of two marked records
  (L-1). `linked_keg` is required but nullable, so a formula without the key
  is unparseable (L-2). The lock-lost test again forbids any re-read of the
  halted step's provider, and any re-read at all when no destination re-read
  certified (spec-guardian 1). A `REMOVE_SOURCE` with no step to re-read
  halts in `dispatch_for` (spec-guardian 4, defence in depth).
- **Beyond the plan, from the code reviews:** the loader requires an active
  semantic on `verify_active` and allows the two only there (L-3);
  `reverify_for` takes the nearest `VERIFY_DESTINATION` before the removal
  (Info-1); the order test fails if no removal ran (L-2); an application-level
  `invariant_17_*` for the JSON parse.
- **Next:** S-1 merges; then the owner applies `docs/passes/16-staged/S/`
  on `wo16/S-2-active` (Needs the owner, item 1), and S-2 follows.

## As built (S-2, first commit, `073678c`)

On `wo16/S-2-active`, cut from `origin/main` at `6a81fc4`, by the **armed
variant** of §10. The owner's message, 2026-10-30: "Apply
docs/passes/16-staged/S/01-verify-active-cells.patch on wo16/S-2-active with
git apply, check it byte for byte as the part S plan says, and commit it
together with the policy version bump."

- **Fix 2 (a):** `data/policy/provider-commands.toml` was written only by
  `git apply` of the staged patch; no other tool wrote it.
- **Fix 2 (b):** `git apply --check` succeeded on `6a81fc4`'s copy of the file.
  `git diff 6a81fc4 -- data/policy/provider-commands.toml` equals the patch byte
  for byte once two things are dropped from both sides: the `diff --git` and
  `index` lines, and git's function-context text after the second `@@` of each
  hunk header. The staged patch carries neither, so dropping them only removes
  what `git apply` ignores; no content line can start with either. Both sides
  are 111 lines, sha256 `79219618f161e5d7ca1ded49c4e911a8d4efa9e7dfaa13a30946d733f6ad9aa9`.
  Independently, applying the patch to the parent's file reproduces the working
  file exactly (sha256 `bbefcd789989ed76a9a7ba096e7425f545780c051bec2a86bd82db517ae5a505`),
  and `git apply --reverse --check` succeeds on the working tree.
  security-reviewer confirmed (b) with these commands before the commit.
- **Fix 2 (c):** every other change is an ordinary edit: `policy.toml` to
  `2026.10.18-1`, the pinned copies (issue templates, README, the four schema
  samples, `wiki/`, the CHANGELOG summary), and the goldens regenerated by
  `scripts/run-fixture-matrix.sh --update` (only `policyVersion` lines moved).
  Left at `2026.10.12-4` on purpose: CHANGELOG's pass 12 and 13 history bullets,
  SE-098's compatibility note, and the captured snapshot
  `fixtures/snapshots/both-providers-intel-macos26-live.json`.
- **Deviation from §10 (recorded, both reviewers):** §10 expected this commit
  to pass the commit gate alone. It cannot: with the cells, every
  `VERIFY_DESTINATION` carries two commands, and 29 tests that pinned S-1's
  single-read step failed. The owner chose, 2026-10-30 ("Include S-2 test
  fixes"), to land the S-2 test and fake updates in the same commit:
  `provider_commands.rs` tests (`the_shipped_table_holds_verify_active_for_every_provider`
  replaces `the_shipped_table_derives_verify_alone`, with the provider set
  pinned; the loader tests now replace the shipped MacPorts cells rather than
  add a duplicate; the `invariant_20_*` rows and dates), `executor/tests.rs`
  (both reads answered; each refusal case keeps a passing activation read so it
  refuses for the reason it names; new: present but inactive, both directions;
  the activation command alone elevating), `tests/execution_gate.rs` (the
  minimal plan's two commands; the shared rule's `[verify, verify_active]`,
  with a shorter and a flipped list refused), and the CLI fakes in
  `executor.rs` and `executor_wiring.rs`, which answer each command in its own
  shape. The replay recording in `cli/tests/executor.rs` replaces the captured
  activation read, which is also the capture's inventory read, with the
  captured answer plus the package active; its doc says so.
- **Not in this commit:** `evaluable_in_this_build` for
  `PROVIDER_REPORTS_ACTIVE`, the replay `invariant_15_*` tests, the samples, and
  SE-107's final amendment. The `analytics_read_on = "2026-10-27"` gap in the
  policy-version test stays a follow-up.

## As built (S-2, second pull request, `wo16/S-2-evaluable`)

Cut from `origin/main` at `f6f3494`. No change to `data/`, `fixtures/`,
`crates/adapters/`, `Cargo.toml`, `Cargo.lock` or any guarded path;
`THIS_BUILD_DISPATCHES_MUTATIONS` is `false`.

- **`evaluable` true.** `evaluable_in_this_build` maps
  `PROVIDER_REPORTS_ACTIVE` to `VERIFY_DESTINATION`, in the same arm as
  `PACKAGE_NEWLY_PRESENT`. Tests: `provider_reports_active_is_evaluable_on_verify_destination_only`
  (every step kind), and the `a_recognised_check_is_not_thereby_evaluable`
  table's row moved to `true`. `answers` is unchanged, so an installed read still
  never answers it (D11).
- **The widened tripwire** names exactly DP-051's four checks as a set, and its
  held half pins both destination checks evaluable whatever the switch is.
- **Replay `invariant_15_*`** (`crates/application/tests/replay_active.rs`):
  `invariant_15_a_recorded_json_inventory_is_judged_by_each_entrys_own_linked_keg`,
  `invariant_15_a_recorded_installed_list_is_judged_by_each_names_own_marker`,
  `invariant_15_a_package_the_recording_does_not_hold_is_never_active`. Counts
  pinned literally (2026-10-30), recordings labelled by provenance.
- **Docs:** 04's check-1 paragraph, `plan.v1.md` obligation 6, CHANGELOG,
  SE-107 (status and the S-2 bullet).
- **Deviation 7 (recorded in SE-107):** the replay tests judge the recorded text
  with the product's judge (`expectation_for` + `evaluate_with` over
  `linked_report`) at the application level, not through `run_with` as
  deviation 3 anticipated: under `--replay` a certifying read halts before it is
  made (C1), so a shell run never reaches the judge, and `run_with` with a clean
  re-read would test the fake, not the recording.
- **Deviation 8:** of each JSON inventory's linked formulae, one in sixteen is
  judged (every unlinked formula and every cask is). Judging all of them took
  84 s in a debug build, because each judgement parses the whole ~900 KB
  document; the stride takes it to 18 s. The pinned count still covers every
  entry.
- **Samples:** no schema sample or golden lists `PROVIDER_REPORTS_ACTIVE` as
  refused (checked by `grep`), so none moved.
- **Mutation proof for this delta:** listed in the Reviews table once run.
- Part S is complete with this pull request. The `analytics_read_on` gap in the
  policy-version test stays a follow-up.
