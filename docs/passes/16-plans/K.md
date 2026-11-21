<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part K — removal becomes opt-in (DP-048 (c)), DP-037's confirmation levels, DP-050

Plan written 2026-11-15, at `4875d89`, for `prompts/16-complete-mutating-product.md`
§5 K, before any code. Reviewed by `spec-guardian` and `security-reviewer`
before any code; their review is the approval (§4). Every line cited below was
re-read at `4875d89`, and again on 2026-11-15 at `9fc9ae0`, the merge of
`origin/wo16/H-8-observe` (H-5, `afc5d5b`, on top of H-4 L1) into this branch,
for every file H-4 L1 or H-5 changed; files they did not touch read as they did
at `4875d89`.

**Timing.** H-4 (PR #73) is merged (`4875d89`), and H-4 L1 (PR #75) is merged
as `e41785b`: it was applied directly on the owner's authorisation, so the
planned H-stage-2 staging (`16-staged/H/04`) never happened (H.md "As built
(H-4)" item 5). H-5 (PR #76) is merged as `26fe181`, and this branch contains
it; the ledger PR #77 turns H's row (`docs/passes/16-progress.md:31`) to
*merged*. §5 runs S..L one at a time, each after the previous merges, and the
ledger's K row depends on H. **Decided: K-0 stays unmerged until H's ledger row
reads `merged`** (the dependency column, §4.1), and no other K slice opens
before then. The *(H)* citations were re-read against H as built on 2026-11-15;
the drift found is in the Reviews table, and a re-check at `26fe181` found none
more (H-5's review rounds changed only the ledger after `afc5d5b`).

The plan was revised after both reviews of `9c9d859`; each finding's
resolution is in the Reviews table, and the sections it changed carry
*(rev. SG-n / SR-n)*.

## The ruling, and the blocking contract

- **DP-048 (c)** (`docs/decisions-pending.md:524-535`): the source is retained
  by default; `--remove-source` is chosen per run with a confirmation stronger
  than an install's; `REMOVE_SOURCE` is dispatched only when the re-read ran,
  DP-030's probe answered, and all six destination checks were evaluated for
  that package. The flip changes 06's synopsis, `retainSource` in the plan and
  history schemas, and `restore`.
- **DP-037 (b)** (`docs/decisions-pending.md:337-347`): a destructive step for
  a row entered through advanced mode needs the stronger confirmation 06
  requires; K defines both levels and how each works under `--yes`.
- **DP-050 (a), plus a toolchain check** (`docs/decisions-pending.md:599-607`):
  a source build is a cost the plan names in its default view, with what the
  build needs; if research shows the toolchain's absence can be detected with
  no new spawn, absence makes that step `MANUAL_REVIEW`.
- **Part C's contract, blocking** (`docs/passes/16-plans/C.md` §9; SE-109's
  part C bullet, `docs/spec-evolution.md:8992-9002`): (a) the level is a pure
  function of the in-memory plan just built from a live capture, never of a plan
  read from a file; (b) an absent `enteredBy` is the strictest level; (c)
  `warnings` is never an input to a decision; (d) E's digest covers `enteredBy`;
  (e) C's confirmation half is a blocking acceptance item of K. C left one
  choice to K: `UNKNOWN` ownership counts as the default view
  (`crates/domain/src/ownership.rs:43-45`), "a choice K confirms or changes".
- **Handed to K by E** (SE-111, `docs/spec-evolution.md:1732-1736`): the
  digest certifies that a plan was printed, not that a person read it, so "part
  K must gate this route as well as `confirm_plan`". And E-4's two switch-on
  tests (`docs/passes/16-plans/E.md:1220-1231`), handed "with part K" to the
  part that makes the destination contract evaluable.

## Where it lives today

**The flag and the request.**

- `plan` and `migrate` take `--retain-source`
  (`crates/cli/src/app/mod.rs:128-130`, `:140-142`); `--yes` is global
  (`:74-78`); `--plan-digest` is `migrate`'s (`:143-147`), checked before
  anything is built (`:324-332`, `crates/cli/src/app/migrate.rs:120-134`).
  Both commands build `PlanRequest { retain_source }` (`mod.rs:463-472`,
  `:503-515`; `crates/cli/src/app/context.rs:254-271`).
- `PlanRequest.retain_source: bool` (`crates/application/src/planner.rs:61-62`).
  **Removal is the default**: the planner appends `REMOVE_SOURCE`,
  `VERIFY_REMOVAL` and `CLEANUP_REVIEW` when it is false (`planner.rs:1048-1099`)
  and copies it to the plan (`:1128`). The reverse-dependency block applies
  only to a removing plan (`:687-696`, INV-04).
- `MigrationPlan.retain_source: bool` (`crates/domain/src/plan.rs:529-530`),
  `entered_by: Option<EntryMode>` (`:537`, enum at `:495`),
  `PLAN_SCHEMA_VERSION = 2` (`:607`). The planner decides `entered_by` from the
  source row (`planner.rs:523`) and always writes `Some` (`:1129`).
- History records it (`crates/domain/src/history.rs:73-74`,
  `crates/application/src/history.rs:83`; `HISTORY_SCHEMA_VERSION = 4`,
  `history.rs:603`). History is consulted, and a re-run recognised as done,
  **only for a retain-source plan** (`planner.rs:208-240`;
  `crates/cli/src/app/migrate.rs:291`, `:517`; `application/src/history.rs:495`).
- **Restore** derives its removal from the record, not a flag: the destination
  package is removed only when the record shows the add performed and the
  snapshot shows it was not there before (`crates/application/src/restore.rs:111-133`),
  as `retain_source: removal != Removal::Removes` (`:138`). `restore` does not
  execute in this build (`crates/cli/src/app/restore.rs:288`).

**The confirmation.**

- One level exists. Interactive: the cleared plan is shown on stderr and
  `run plan <code>` is asked for (`crates/cli/src/consent.rs:188-232`,
  `crates/application/src/plan_tie.rs:70-87`), from
  `confirm_the_plan` (`migrate.rs:343-373`), after the gate clears and before
  the state directory is touched (`migrate.rs:278-285`). Script:
  `--yes --plan-digest <d>` is checked against the plan by `tie`
  (`plan_tie.rs:171-182`) in every build, before the gate (`migrate.rs:620-627`);
  the digest is SHA-256 over a domain string and the plan's serialisation
  (`plan_tie.rs:92-96`, `:126-132`).
- The witness `ShownPlan` holds the plan and is neither `Clone` nor `Default`
  (`plan_tie.rs:31-66`); the run loop refuses a plan it does not cover before
  the history probe and the lock (`crates/cli/src/executor/mod.rs:897-905`).
  Elevation consent is asked inside the run, before anything is written
  (`executor/mod.rs:929-949`).
- 06 promises a stronger confirmation in advanced mode and defers its
  definition to K (`docs/spec/06-cli-and-ux.md:218-236`); its synopsis lists
  `[--retain-source]` (`:31-35`).

**When `REMOVE_SOURCE` is dispatched today** (under a forced allowance; the
switch is off). `mutating_action` (`crates/application/src/executor.rs:1033-1093`):
the snapshot was taken (`:1060`); the guarding `VERIFY_DESTINATION` is recorded
`Verified` (`:1063-1067`, `destination_record_before` `:1139-1166`) — a
`VerifiedWithUnchecked` passes the removal over (`:1076`); every resolution
held (`:1071`); every command's elevation, re-derived, is `NONE` (`:1081-1091`);
and `dispatch_for` refuses a removal with no re-read to name (`:1100-1110`).
Then the shell, per command (`crates/cli/src/executor/mod.rs:1030-1137`):
destination configuration re-read and re-check; the destination re-read with
the verify step's own commands, judged by `destination_still_verified`
(`application/src/executor.rs:1742-1788`, which answers `Result<(), Precondition>`)
— *(H)* the reads plus a fresh check 3 per executable and no other: checks 2,
4 and 5 are not re-run, a window declared in H's plan only
(`docs/passes/16-plans/H.md:226`; `reverify_questions`,
`crates/application/src/executor.rs:1694-1707`, whose comment calls it a
window "the specification declares" though no spec text does yet); K-6
declares it in 04 and SE-114;
the run lock still current (`:1098`); the source's configuration re-check
(`:1110-1129`); the provider-lock sample (`:1133-1137`, `provider_locks_clear`
`application/src/executor.rs:450-458`, `Result<(), ProviderLockRefusal>`); then
`mutation_dispatch` (`executor/mod.rs:1140-1148`, `:1359-1367`), which takes the
source's `ConfigurationRechecked` token by value and nothing else for the
removal. The gate checks the contract's shape (`destination_contract_complete`,
`crates/application/src/execution_gate.rs:706`) and that each removal is
verified after (`:1070-1084`). **So all three of DP-048's conditions are met by
the order of statements in the shell; none is a value the dispatch requires.**
*(H, as built)* Since H-5 the liveness observer is on
(`THIS_BUILD_OBSERVES_LIVENESS_PROBE = true`, `crates/domain/src/plan.rs:604`;
`observed_by_this_build`, `execution_gate.rs:644-660`; SE-113's "As built,
H-5"), so past the gate all four of DP-051's checks are observed and
`the_switch_needs_the_destination_contract_observed`
(`crates/cli/tests/executor.rs:1810`) is positive. A removal is dispatched only
behind a `VERIFY_DESTINATION` recorded `Verified`, every check `Holds`; a
`VerifiedWithUnchecked` still passes it over as `SOURCE_KEPT_UNCHECKED`
(`crates/application/src/executor.rs:1076-1077`). H expects most live probe answers to be `UNCHECKED` —
a package with no row is skipped, a row answers only at its exact version — so
most removals will keep the source (H.md, "What H-5 does not claim").
`THIS_BUILD_DISPATCHES_MUTATIONS` (`plan.rs:584`) stays `false` and is the
single build-level barrier before a curated probe starts (H.md "As built
(H-5)" item 2; accepted in writing in H's Reviews, H-5).

**H-5's security M-1, for K** (H.md "As built (H-5)" item 2, and its Reviews
row). After the switch flips, the mutation site does not stop a probe: on a
plan whose destination already holds the package, `ADD_DESTINATION` has no
command and is passed over as `NOT_DISPATCHED_MUTATING`
(`application/src/executor.rs:1052-1054`, reached from `next_action_with`,
`:932`), and `VERIFY_DESTINATION` then certifies under the lock and **probes,
before and without any mutation** that `NotInThisBuild` would refuse. The kill
switch is `THIS_BUILD_OBSERVES_LIVENESS_PROBE` set back to `false` first. This
matters to K wherever it touches `VERIFY_DESTINATION` or such a plan: D6's
`guarding_verify` and `clear_removal` read that step's record, which on such a
plan was certified by a probe no mutation preceded; D7's toolchain block never
applies to such a plan, so it adds no refusal ahead of the probe; and D8 makes
coexistence the default, so after a flip a default `migrate` onto a destination
that already holds the package probes and dispatches nothing. None of this
changes K's slices; it is a sentence K-3 writes into SE-114 and K-6 keeps, and part
L's proposal inherits H's checklist item for it (H.md "When the switch flips").

**DP-050 today.** A non-prebuilt destination raises the non-blocking
`destination_source_build` risk (`planner.rs:744-752`) — but with the reason
`reason.availability_unknown` (`locales/en.toml:119`, "could not be
determined") **for every non-prebuilt state, a known source build included**,
and nothing in it says what the build needs. Risks are rendered in the default
view (`crates/cli/src/app/human.rs:474-480`). The only test,
`source_build_and_unknown_are_not_manual_review`
(`crates/application/tests/invariants.rs:1850-1893`), asserts the identifier
and non-blocking, not the text. So **`destination_source_build` does not yet do
what DP-050 asks**, and 04's sentence that it names "what the build needs"
(`docs/spec/04-migration-and-safety.md:262-268`) is not met. The toolchain is
already captured: `xcode-select -p` is a permitted system probe
(`crates/adapters/src/process/mod.rs:90`, `:403`), read by
`detect_command_line_tools` (`crates/adapters/src/system/machine.rs:433-451`)
into `machine.command_line_tools` (`crates/domain/src/machine.rs:121-131`,
`:144`), and `doctor` already reports it (`crates/application/src/doctor.rs:575-594`;
`locales/en.toml:665-667`).

## Decisions the work order leaves to the session

Each is made here, with its reason, and lands in SE-114 (new, one topic: the
removal opt-in and its confirmation) or, for DP-050, as a propagation bullet
under SE-109 with R-29. **SE-114 is written in K-3 and amended in the same pull
request by every later slice that changes what it says** (K-4, K-5, K-6, K-7;
*rev. SG-M4*).

**D1. `--retain-source` is an accepted no-op, with a deprecation notice,
throughout the v0.2.x line** (*rev. SG-L3*); removing it is a later release's
change, with its own SE entry. Given with `--remove-source` it is a usage error
(exit 2, before anything is built). The notice goes to stderr only
(`cli.retain_source_deprecated`); the plan, its `--json` and its digest are
byte-identical with and without the flag. *Reason:* the tool itself tells
people to type it (`locales/en.toml:324`, `:464`), 06 and three wiki pages
document it, and no tag exists yet (`git tag` is empty; `CHANGELOG.md:238`
"not yet tagged"), so no release depends on it but anyone following `main`'s
advice does. A no-op costs nothing; a refusal would break advice the tool gave.
Keeping it out of `warnings` keeps the document a function of the plan.

**D2. The request names the fate, not a negation.** `PlanRequest.retain_source:
bool` becomes `source: SourceFate` with `Keep` and `Remove`, and no `Default`.
*Reason:* about 49 call sites in tests and code write `retain_source: false`
today, meaning "remove". With an enum every one is rewritten explicitly and the
compiler finds them all; a flipped `bool` would compile and silently invert.

**D3. Schemas: no version bump.** `retainSource` keeps its name, type and
meaning — the source is kept — in `plan.v2` and `history.v4`; what changes is
the command line's default, and the field descriptions say so
(`docs/schemas/plan.v2.md:34`; `history.v4.md`'s example `:138` stands).
`docs/schemas/README.md`'s rule makes this compatible: no field is removed,
renamed, retyped, or given another meaning. Additive, each noted in the
CHANGELOG:
- `plan.v2`: `sourceOwnership` (`TOP_LEVEL` | `DEPENDENCY` | `ORPHAN` |
  `UNKNOWN`), **written explicitly by every plan this build produces,
  `TOP_LEVEL` included**, and absent only in a document from before the field,
  where it means the strictest level; E's digest keeps `TOP_LEVEL` distinct
  from absence, as it does `DEFAULT_VIEW` (*rev. SG-M2*). At the document root,
  beside `warnings`: `confirmation` (`PLAN` | `REMOVAL` | `ADVANCED_REMOVAL`),
  **optional**, derived when the document is written and never stored on the
  plan, like `warnings`; absent in every document from before the field, so a
  consumer must not read its absence as `PLAN` (*rev. SG-L7*).
- `migrate.v2`: the same `confirmation`, and `removalDigest` (D5) when the
  level is `REMOVAL`; the new gate refusals (D5) in its `gateRefusals` list
  (*rev. SG-M4*). `restore.v2`: `confirmation`.
- `plan.v1`, `migrate.v1`, `restore.v1` and `history.v1`–`v3` are superseded or
  only read; their `retainSource` means the same, and they are not edited.
- 06's synopsis (`:31-35`) is a Tier 3 MUST: changed with SE-114.

**D4. `restore` keeps deciding removal from the record**
(`application/src/restore.rs:111-133`) and gains no `--remove-source`. Its plan
is confirmed at the level its own steps derive (D5), never lower. *Reason:* an
undo that leaves what the run added is not an undo; INV-01's order is
unchanged (`invariant_01_a_restore_plan_adds_before_it_removes`); and the
interaction SE-107 noted (`docs/spec-evolution.md:8735-8736`: a removal
*refused* by release policy would refuse every undo of an install) does not
arise, because K makes removal opt-in for `migrate`, not refused. Since the
default now keeps the source, 04's "cheap case" (`04:242-247`) becomes the
common one. Restore's execution stays unbuilt (`cli/src/app/restore.rs:288`);
K renders its level and wires nothing. **A removing restore has no script
route** until restore execution is designed: `restore` takes no
`--plan-digest` today, and K adds none; SE-114 says so (*rev. SG-L8*).

**D5. Two new confirmation levels above today's, ordered by inclusion.**
`PLAN` is today's level, the one an install gets; **K defines the two above
it**, `REMOVAL` and `ADVANCED_REMOVAL`, and 06's paragraph says it the same way
(*rev. SG-L1*).

| Level | When (derived from the plan) | A person at a terminal | `--yes --plan-digest` |
|---|---|---|---|
| `PLAN` (today's) | no removal | Q0 `run plan <c0>` | `--plan-digest <d>` |
| `REMOVAL` | a removal, and exactly `enteredBy: Some(DEFAULT_VIEW)` with `sourceOwnership: Some(TOP_LEVEL)` | Q0, then Q1 `remove source <c1>` | `--plan-digest <d>` **and** `--removal-digest <r>` |
| `ADVANCED_REMOVAL` | a removal, and **every other pair**: `ADVANCED`, `NAMED` or absent `enteredBy`; `DEPENDENCY`, `ORPHAN`, `UNKNOWN` or absent `sourceOwnership` | Q0, Q1, then Q2 `confirm advanced removal <c2>` | **refused**: exit 2 (`USAGE`), stdout empty, nothing run |

- **The derivation is total** (*rev. SG-M2, SR-L1*): one exhaustive `match`
  over `(Option<EntryMode>, Option<Ownership>)` with no wildcard arm, so a new
  variant of either enum fails to compile until it is placed. Only
  `(Some(DefaultView), Some(TopLevel))` is `REMOVAL`.
- **"A removal"** is `!retain_source` **or** any `REMOVE_SOURCE`,
  `VERIFY_REMOVAL` or `CLEANUP_REVIEW` step: any signal raises the level.
- **Two new gate refusals**, both defence in depth, since the planner builds
  neither: `RETENTION_DISAGREES_WITH_STEPS` when `retain_source` is false with
  no `REMOVE_SOURCE`, or true with a `REMOVE_SOURCE`, `VERIFY_REMOVAL` or
  `CLEANUP_REVIEW` step (*rev. SR-L5*), since `history.retainSource` would then
  lie; and `ENTRY_DISAGREES_WITH_OWNERSHIP` for the pairs the planner's rule
  (`planner.rs:523`, `ownership.rs:43-45`) cannot produce: `DEFAULT_VIEW` with
  `DEPENDENCY` or `ORPHAN`, and `ADVANCED` or `NAMED` with `TOP_LEVEL` (*rev.
  SG-M2*). Both are documented in `migrate.v2.md`'s `gateRefusals` and the
  CHANGELOG in K-4, and listed in `every_gate_refusal`.
  *(As built, K-4; SE-114.)* Both are raised only whenever mutations would
  be dispatched and never for a plan carrying `MANUAL_REVIEW`: the planner
  does build a `--remove-source` plan blocked into review with
  `retainSource: false` and no removal step. And `ADVANCED` or `NAMED` with
  `TOP_LEVEL` is **not** refused: the planner builds it for a source row
  attached by a weak mapping (`analysis.rs`'s `advanced_only`), the pair is
  consistent, and its level is the strictest. Only `DEFAULT_VIEW` with
  `DEPENDENCY` or `ORPHAN` is refused.
- **"Stronger" is a checkable property:** whatever confirms a level also
  confirms every lower one, and something that confirms the lower one does not
  confirm it. Each interactive level asks every question of the level below
  plus one, each with its own fresh code (the code rule of `plan_tie.rs:20-22`);
  so 06's "stronger than in normal mode" holds by construction, and normal-mode
  removal is stronger than an install's.
- **The questions** are asked in `confirm_the_plan` (`migrate.rs:343-373`), all
  before the state directory is touched, never inside the loop. Q1 lists each
  `REMOVE_SOURCE` step: package and provider, escaped as every rendered
  provider value is (`crates/cli/tests/hostile_text.rs`). Q2 says why the row
  needs it: for `NAMED` and `ADVANCED`, `advanced.warning` and
  `plan.entered_by_named`; **for unknown intent, its own key,
  `confirm.removal_intent_unknown`, and no `advanced.warning`**, because the
  row is in the default view (*rev. SG-L2*); for a plan without the fields,
  `confirm.removal_entry_unrecorded`. The phrases are ASCII and never
  translated, as `run plan` is; nothing provider-derived is in a phrase.
- **The script route, `REMOVAL`.** `removalDigest` is SHA-256 over a second
  domain string (`macportsbrew-removal-digest/1 <version>\n`) and the same plan
  bytes, so it never equals `planDigest`. `migrate` prints it beside
  `planDigest` only for a `REMOVAL` plan. `--removal-digest` requires `--yes`,
  `--plan-digest` and `--remove-source`, and is checked in every build where
  the plan digest is (`migrate.rs:620-627`); missing or wrong, it is
  `PLAN_NOT_SHOWN` (exit 4, stdout empty), as a wrong plan digest is. *Reason
  and limit, stated in SE-114:* like the plan digest it certifies printing,
  not reading (SE-111); what it adds is that a script written for installs —
  one that pipes `.planDigest` back — can never remove anything.
- **`ADVANCED_REMOVAL` has no script route.** *Reason:* INV-13 asks for an
  explicitly entered advanced mode, and an echoed value cannot show that a
  person saw the warning. The refusal sits **at the tie point**
  (`migrate.rs:624-627`), in every build, before anything is printed: exit 2,
  like a missing terminal (`plan_tie.rs:195-198`), because the route is
  structurally unable to confirm it (*rev. SR-L2*). Reversible: a later SE
  entry may add one.
- **`UNKNOWN` ownership: changed for removal, kept for the view.** C's choice
  stands for what the default view lists and for `enteredBy` (an `UNKNOWN` row
  is `DEFAULT_VIEW`, no warning). For a removal it is the strictest level.
  *Reason:* with no record of intent (a cask with no receipt, R-12) nothing
  shows the row is not something another package needs, which is the case
  INV-13 guards. So the level stays a function of the plan alone (C §9 (a)),
  the plan carries the source row's ownership as `sourceOwnership`; E's digest
  and `plan_still_holds` then cover it, so an ownership that moves between
  showing and running refuses the run, as a moved `enteredBy` does
  (`a_plan_whose_entry_moved_under_the_lock_is_refused`). **K-3 records this in
  SE-109's part C bullet and in the ledger's K row**, beside C's "a choice K
  confirms or changes" (*rev. SG-L8*).
- **Where the level is enforced.** `ShownPlan` gains the level it was confirmed
  at. `confirm_shown` and `tie` mint `PLAN`; `confirm_removal` and `tie_removal`
  **consume the `PLAN` witness by value** and return `REMOVAL`;
  `confirm_advanced_removal` consumes the `REMOVAL` witness by value and
  returns `ADVANCED_REMOVAL` (*rev. SR-L2*), so no question can be minted
  without the one below it. All live in `plan_tie.rs`, with the word scans
  extended to their callers. The run loop's check at `executor/mod.rs:900`
  becomes `ShownPlan::admits(plan)`: covers **and** level at least
  `confirmation_level(plan)`, derived again from the plan about to run. The
  decision is the application's (SE-092); the shell only asks.
- **06's "until then no plan executes"** (`:230-231`) is kept by K-5 while the
  switch is false; K-5 adds the levels' definition beside it, and only the flip
  (part L's proposal) may remove it (*rev. SR-I3*).

**D6. The dispatch requires a removal witness per command** (*rev. SG-M1,
SR-M1, SR-M2, SR-I1, SR-I2*).

- **What is required, and who decides it.** `application::executor` decides
  that a command needs a witness from `plan.steps[position].kind ==
  StepKind::RemoveSource` — not from `reverify.is_some()`, not from argv. Any
  other mutating step that uninstalls would be refused by the gate until it is
  defined: today only `ADD_DESTINATION` and `REMOVE_SOURCE` are dispatched
  (`executor.rs:1040-1051`; `CLEANUP_REVIEW` and `SET_PRIMARY` never are), and
  K-6 adds a test that pins that set.
- **One function picks the guarding `VERIFY_DESTINATION`.** Today
  `reverify_for` takes the nearest one before the removal (`.rposition`,
  `executor.rs:672-674`) while `destination_record_before` takes the first
  (`.position`, `:1144-1146`). One function, `guarding_verify(plan, position)`,
  serves both, so the step re-read and the step whose record is checked cannot
  differ.
- **The chain, minted per command, each consuming the one before** (the order
  of SE-107 M1 is kept: slow reads first, cheap confirmations freshest at
  dispatch):
  1. `destination_still_verified(...) -> Result<DestinationReverified,
     Precondition>` (today `Result<(), _>`, `executor.rs:1742-1788`), naming
     the step, command index and guarding verify position.
  2. `dependents_still_absent(run, position, reverified, inventory) ->
     Result<DependentsRechecked, Precondition>` (D9), consuming (1).
  3. The run lock still current (`executor/mod.rs:1098`) and the source's
     configuration re-check (`:1110-1129`), which yields the source's
     `ConfigurationRechecked`, as today.
  4. `clear_removal(run, position, records, dependents, &source_checked,
     sample) -> Result<RemovalCleared, Precondition>`, consuming (2) and
     borrowing (3): it checks the guarding verify step is recorded `Verified`
     and `destination_contract_complete` holds for it (the six checks — active,
     newly present when the plan adds, integrity, ownership, linkage, liveness
     — `execution_gate.rs:706-756`; H made `Verified` require all four of
     DP-051's checks to `Hold`, and since H-5 all four are observed, so a
     recorded `Verified` now carries a liveness answer, which on a plan whose
     destination already held the package came from a probe no mutation
     preceded: M-1 above), that every resolution held, and judges **this** command's
     provider-lock sample through `provider_locks_clear`. So the lock sample
     cannot be judged for a removal before the re-read and the dependents
     check exist. `provider_locks_clear` itself is unchanged for the four
     run-level samples (`executor/mod.rs:913-947`), which have no re-read.
  5. `mutation_dispatch` (`executor/mod.rs:1359`) takes `Option<RemovalCleared>`
     **by value**, and an application function checks it `covers(step,
     program, argv)` the way `ConsentGiven::covers` does
     (`application/src/consent.rs:133`); a removal command without one, or a
     witness on any other step or command, is `PreconditionFailed`.
  None of the four tokens is `Clone`, `Default` or constructible outside
  `application` (the compile-time idiom of `plan_tie.rs:46-66`).
- **What is held by type, and what by test** (*rev. SR-M1*). By type: no
  removal command is handed over without a `RemovalCleared` for that command;
  it cannot be minted without the destination judgement and the dependents
  judgement for that command, and the source's re-check token; and the lock
  sample for a removal is judged only inside `clear_removal`. *(H)* What
  `DestinationReverified` certifies is H's re-read judge: the reads and check 3
  per executable; checks 2, 4 and 5 come from the guarding step's record, not
  from a re-run (`reverify_questions`, `crates/application/src/executor.rs:1694-1707`), and K keeps
  that window rather than widening the re-read: a fresh liveness probe before
  every removal command would start a curated program again per command, and
  DP-048 (c) asks that the six checks were *evaluated* for that package, not
  re-run. 04 does not name that window yet, so K-6 names it in 04's residual
  paragraph beside dependents, and corrects the `reverify_questions` comment
  to cite 04. **Not by type:**
  that the `StepRead`s `destination_still_verified` judged were *fresh* — the
  shell builds them (`executor/mod.rs:1065-1069`), so the token proves the
  judgement, not that the reads were made just then — and that the run lock
  was checked between. Those are held by a test of the per-command boundary
  order, named in SE-114:
  `invariant_01_each_removal_command_passes_its_boundaries_in_order`
  — `DestinationReverified < DependentsRechecked < StillCurrent <
  ConfigurationRechecked (source) < ProviderLocksSampled < Dispatched`
  (`Boundary`, `executor/mod.rs:701-736`; `DependentsRechecked` is new) — and
  by the mutants below, one of which judges the re-verify on the verify step's
  earlier reads.

**D7. DP-050: name the cost now; block on absence only if R-29 proves it.**
`destination_source_build` takes its reason from the availability: a known
`SOURCE_BUILD_REQUIRED` gets `reason.destination_source_build` (the cause —
`SourceBuildReason`, `crates/domain/src/availability.rs:37-43` — and what the
build needs: the developer tools, and time), and `UNKNOWN` keeps
`reason.availability_unknown`. If R-29 shows that `xcode-select -p`'s non-zero
exit, or a filesystem read with no spawn, proves the tools absent, a
source-build `ADD_DESTINATION` on a machine whose snapshot says so becomes
`MANUAL_REVIEW` (`destination_toolchain_missing`), in both directions, for
`restore` too (not a special case), and never when the destination already
holds the package (*(H)* such a plan's `ADD_DESTINATION` has no command, and
its `VERIFY_DESTINATION` probes with nothing dispatched before it: M-1 above;
the block, keyed on a source-build `ADD_DESTINATION`, cannot stand in front of
that probe and is not meant to). `CommandLineTools::Unknown` stays non-blocking with text
that says it could not be told. If R-29 shows neither proves it, nothing
blocks and the risk text says why.
- **A filesystem read** (*rev. SG-L5, SR-L4*), if R-29 needs one, goes through
  the machine reader that `--replay` records and replays
  (`crates/adapters/src/system/machine.rs`), so a recording that lacks it
  answers `Unknown` — non-blocking — and
  `invariant_15_the_toolchain_answer_is_the_snapshots_under_replay` covers
  that route. If the recorder cannot carry such a read, the read is not built,
  and recapturing with it becomes an owner capture item.
- **`doctor` already reports absence** (*rev. SG-L4*): `doctor.rs:575-594`,
  tested by `command_line_tools_missing_or_unknown_are_both_warnings_and_are_distinguishable`
  (`crates/application/tests/doctor_findings.rs:184`). K-7 records "already
  met" in SE-109's DP-050 propagation bullet. No new check is added unless R-29
  changes what `Installed` means (a selected Xcode is not the Command Line
  Tools, `docs/research/R-27-no-cross-provider-linkage.md:5`); then the wording
  changes under `/add-doctor-check`'s procedure.

**D8. Coexistence is the default outcome (INV-18).** A plan without
`--remove-source` ends after `VERIFY_RESOLUTION` with `RECORD_HISTORY`; the
rendered plan says the source is kept and how to remove it (`plan.source_kept`).
A consequence, recorded: every default live `migrate` now consults history
(`migrate.rs:291`, `:517`), so a default re-run is recognised as done
(INV-16), and `migrate.v2.md:217-220` changes from "only by
`--retain-source`" to "unless `--remove-source`". A second consequence, from
H-5's M-1 (above): a default plan still carries `VERIFY_DESTINATION`, so once
the switch flips a default `migrate` onto a destination that already holds the
package probes under the lock and dispatches nothing; with the source kept by
default that is the common shape of such a run, not an edge. SE-114 says so,
and K-8 records it for part L's proposal beside H's checklist item.

**D9. Reverse dependents are re-read before each removal command: option (a)**
(*rev. SG-M3*). INV-04's check is today the planner's, over the capture
(`planner.rs:687-696`, `reverse_dependency_count` from the inventory,
`application/src/analysis.rs:474`), repeated under the lock by the re-plan
(`invariant_04_a_dependent_that_appeared_under_the_lock_refuses_the_run`). An
`ADD_DESTINATION` can take minutes — a source build longer — and a package
installed into the source meanwhile, depending on the one being removed, is
not seen. **(a) is reasonable here:** the reverse dependents come from the
source adapter's own inventory read, which capture already runs (no new
program, subcommand or allowlist row; §3 item 3 holds), so the shell gains one
seam, `reread_dependents(provider)`, returning the source's fresh inventory,
and the application judges it (`dependents_still_absent`, D6 step 2): any
reverse dependent of the source package inside the source provider is
`Precondition::DependentsAppeared`, the command is not handed over, and the run
ends `PARTIAL_SUCCESS` with the source kept (INV-03). It sits with the
destination re-read, before the cheap confirmations. *Cost, stated in SE-114:*
one inventory read of the source provider per removal command, inside the
window DP-048 accepts; the residual between that read and the provider's
uninstall is the same residual, now named for dependents too in 04
(`:482-491`).

## The DP-050 research note (R-29)

**Needed.** `docs/research/` has no note answering the question.
R-27 finding 24 (`R-27-no-cross-provider-linkage.md:433`) shows only that
`xcode-select -p` succeeding does not prove a tool lookup cannot raise the
install dialog, which is a different question; R-28 and R-27 record this
machine's answer (both Xcode and the Command Line Tools present, `R-28:5`,
`R-27:5`); `09-research-checklist.md` has no item for it (the last is R-28,
`:526`). K-1 adds `R-29 · Whether a read proves the Command Line Tools present
or absent` to the checklist and writes the note (`provider-researcher`).

**The one question:** does `xcode-select -p`'s answer, or a filesystem read of
the Command Line Tools location, prove the build tools present — and does its
non-zero exit prove them absent?

**What the note may run — an allowlist, nothing else** (*rev. SR-L3*):

1. `/usr/bin/xcode-select -p`, exit status and stdout (already permitted, and
   in every recording, `R-28:40`).
2. `readlink /var/db/xcode_select_link` — whether the selection is a stored
   link, and whether `-p` reports it when its target is gone (a lead to
   confirm, not a fact).
3. `stat` of `/Library/Developer/CommandLineTools`, `usr/bin/clang`,
   `usr/bin/make` and `SDKs/` under it, and of the selected Xcode's
   `Contents/Developer/usr/bin` — stat only; nothing found is executed.
4. `pkgutil --pkg-info`, `--pkgs` and `--files`, read-only verbs, for the
   Command Line Tools receipt. **Research only** (*rev. SG-L6*): `pkgutil` is
   not on the product's allowlist, which is frozen (§3 item 3), so nothing K
   builds may depend on it; if the answer needs it, K-7 builds only the risk
   text.
5. `man xcode-select` (local, already quoted at `R-27:368`), and documentation
   fetches, each dated: Apple's "Installing the command-line tools", Homebrew's
   installation requirements (`F-1-homebrew-support-tiers.md:206`: the Command
   Line Tools or Xcode), and MacPorts' install guide on what a source build
   needs.

**Never:** any other program under `/usr/bin` (no `xcrun`, no developer shim —
`clang`, `cc`, `make`, `git`, `python3` — since R-27 finding 24 shows a lookup
can raise the install dialog); `xcode-select --install`, `-s`, `--switch`,
`--reset`; any elevation; any `brew` or `port` mutation; setting
`DEVELOPER_DIR`, `SDKROOT`, `TOOLCHAINS` or any `xcrun_*` variable; deleting
or renaming anything.

**What this machine cannot show:** absence. It has both, and producing absence
is a mutation. So the absent case comes from documentation, or is an owner
capture on a disposable machine; the note says which, and K-7 builds the block
only on what the note can support.

## Slices

| Slice | Branch | What | Depends on |
|---|---|---|---|
| K-0 | `wo16/K-0-plan` | This plan and its reviews. Docs only. Stays unmerged until H's row is `merged` | H merged |
| K-1 | `wo16/K-1-toolchain-note` | R-29: the note and its checklist entry. Docs only, `provider-researcher`, the allowlist above | K-0 |
| K-2 | `wo16/K-2-source-fate` | D2: `SourceFate` replaces `PlanRequest.retain_source`, every site mapped explicitly, **no behaviour change** (the CLI still maps an absent flag to `Remove`) | K-0 |
| K-3 | `wo16/K-3-opt-in` | D1, D3 (`retainSource` text), D4, D8: `--remove-source` on `plan` and `migrate`, the default `Keep`, `--retain-source` the no-op with its notice and the conflict; 06's synopsis; 04's restore paragraph; catalogue `en` and `pt-BR` (`plan.dry_run_only`, `migrate.by_hand`, `doctor.provider_prefix_is_not_absolute`, `history.show_retain_source`, `plan.source_kept`, `cli.retain_source_deprecated`); documented samples regenerated; wiki's three pages; SE-114 written, with M-1's sentence (D8); the `UNKNOWN` decision in SE-109's part C bullet and the ledger | K-2 |
| K-4 | `wo16/K-4-levels` | D5's application and domain half: `sourceOwnership`, `ConfirmationLevel`, `confirmation_level`, the two gate refusals documented in `migrate.v2.md` and the CHANGELOG, `ShownPlan`'s level, `admits`, the consuming mints, `tie_removal` and the removal digest, the run loop's check; **SE-114 amended in this PR** | K-3 |
| K-5 | `wo16/K-5-confirm` | D5's command-line half: Q1 and Q2 and their keys, `--removal-digest`, the tie-point refusal of `ADVANCED_REMOVAL`, `confirmation` and `removalDigest` in the documents, the schemas, 06's advanced-mode MUST and its confirmation paragraph (keeping "until then no plan executes"), C's invariant test completed; E's two handed tests tried in a scratch switch-on worktree, outcome recorded; SE-114 amended | K-4 |
| K-6 | `wo16/K-6-witness` | D6 and D9: `guarding_verify`, the four tokens, `reread_dependents`, `DependentsRechecked`, `clear_removal`, `mutation_dispatch`'s parameter, the boundary-order test; 04's "Not yet met" (`:492-498`; since H-5 it already says DP-051's four checks exist and names only the opt-in, and ties the switch to the owner's ruling on part L's proposal) for the opt-in and the dispatch conditions becomes met, keeping that last clause, and the residual names dependents and the re-read's unrerun checks 2, 4 and 5; **SE-114 amended in this PR**, with M-1's sentence | K-5 |
| K-7 | `wo16/K-7-source-build` | D7, on R-29's answer: the risk's reason, the toolchain block if supported, `doctor`'s wording if changed; 04's source-build sentence (`:262-268`); SE-109 propagation bullet ("already met" for `doctor`) | K-1, K-6 |
| K-8 | `wo16/K-8-close` | `the_switch_needs_removal_opt_in` turned positive; D8's M-1 consequence recorded for part L's proposal beside H's checklist item; ledger, `16-report.md`'s K section, CHANGELOG; `docs-auditor` | K-7 |

Until K-5 merges a `--remove-source` plan is confirmed by Q0 alone. Nothing
runs: the switch is off (`grep -n THIS_BUILD_DISPATCHES_MUTATIONS`).

## Tests, per invariant, by name

New tests are marked *new*; the rest are extended.

- **INV-01.** *new* `invariant_01_a_removal_dispatches_only_with_its_witness`
  (application: no witness, a witness for another step or command, a witness
  from a re-read of another verify position — each refused; both directions);
  *new* `invariant_01_a_removal_witness_needs_every_check_of_the_contract`
  (a `Verified` record on a step the contract check fails, and a
  `VerifiedWithUnchecked`, mint nothing); *new*
  `invariant_01_each_removal_command_passes_its_boundaries_in_order` (D6;
  named in SE-114); *new*
  `invariant_01_the_record_checked_and_the_step_reread_are_the_same_step` (a
  plan with two `VERIFY_DESTINATION` steps); *new*
  `invariant_01_only_add_and_remove_are_ever_dispatched`; extended
  `invariant_01_no_removal_dispatches_unless_the_lock_probe_answered_immediately_before_it`
  and `invariant_01_a_destination_that_is_not_there_stops_the_removal`
  (`crates/cli/tests/executor_wiring.rs`) to assert through the witness;
  `invariant_01_a_restore_plan_adds_before_it_removes` unchanged and still
  green after D2/D4.
- **INV-02.** *new* `invariant_02_the_run_refuses_a_plan_confirmed_at_a_lower_level`
  (a `PLAN` witness on a `REMOVAL` plan, a `REMOVAL` witness on an
  `ADVANCED_REMOVAL` plan: `PLAN_NOT_SHOWN`, before the probe and the lock);
  *new* `invariant_02_a_removal_needs_its_own_echoed_digest`;
  *new* `invariant_02_a_script_cannot_confirm_an_advanced_removal` (exit 2,
  stdout empty, at the tie point, in this build); extended
  `invariant_02_a_declined_plan_question_exits_6_before_anything_runs`
  (`plan_tie.rs:215`) to Q1 and Q2.
- **INV-03.** *new* `invariant_03_a_removal_without_its_witness_ends_partial_with_the_source_kept`;
  extended `invariant_03_a_removal_whose_destination_was_not_reverified_ends_partial`.
- **INV-04.** Extended `invariant_04_removal_with_dependents_is_blocked_and_explained`
  (`crates/application/tests/invariants.rs`): dependents block a `Remove`
  plan and do not block a `Keep` plan; extended
  `invariant_04_a_dependent_that_appeared_under_the_lock_refuses_the_run` to a
  removing run; *new*
  `invariant_04_a_dependent_that_appeared_before_the_removal_stops_it` (D9: a
  dependent present in the fresh inventory read and absent from the capture
  under the lock; the command is not handed over, `PARTIAL_SUCCESS`, source
  kept; both directions).
- **INV-13 (C's blocking half).** `invariant_13_a_named_dependency_warns_and_needs_the_stronger_confirmation`
  (`invariants.rs:2034-2064`) loses its "pending" doc comment and asserts: a
  `NAMED` removal is `ADVANCED_REMOVAL`, strictly above a `DEFAULT_VIEW`
  removal, strictly above an install; a `NAMED` plan that keeps the source is
  `PLAN`; and the run refuses it with a `REMOVAL` witness. *new*
  `invariant_13_every_entry_and_ownership_pair_but_one_is_the_strictest`
  (every pair of the two `Option`s, absences included; only
  `(DEFAULT_VIEW, TOP_LEVEL)` is `REMOVAL`); *new*
  `invariant_13_an_inconsistent_entry_and_ownership_is_refused`.
- **INV-16.** Extended `invariant_16_*` recognition tests: a default (`Keep`)
  re-run is recognised as done; a `--remove-source` run consults no history.
- **INV-17.** *new* `invariant_17_confirmation_levels_are_direction_neutral`;
  *new* `invariant_17_a_source_build_names_its_cost_in_either_direction`;
  and, if K-7 builds the block,
  *new* `invariant_17_a_missing_toolchain_blocks_a_source_build_in_either_direction`.
- **INV-18.** *new* `invariant_18_a_migration_keeps_the_source_unless_removal_is_asked_for`
  (application, both directions: no `REMOVE_SOURCE`, `VERIFY_REMOVAL` or
  `CLEANUP_REVIEW`, `retainSource: true`); *new*
  `invariant_18_migrate_without_remove_source_plans_coexistence` (command line,
  over the committed recordings and snapshots); policy's
  `invariant_18_coexistence_is_a_first_class_outcome` unchanged.
- **INV-15.** *new* `invariant_15_the_toolchain_answer_is_the_snapshots_under_replay`
  if K-7 builds the block (the recordings carry `xcode-select -p`; a recording
  without a read K-7 adds answers `Unknown`).
- **Tripwire.** `the_switch_needs_removal_opt_in`
  (`crates/cli/tests/history_preconditions.rs:2530-2546`) becomes positive and
  held whatever the switch: the CLI's default plans keep the source in both
  directions, and `mutation_dispatch` cannot take a removal command without a
  witness (structural scan). `source_build_and_unknown_are_not_manual_review`
  (`invariants.rs:1855`) is rewritten: its pinned sequence loses the three
  removal steps under `Keep`, and a `Remove` case keeps them.
- **E's two handed tests** (E.md:1220-1231): handed to "the part that makes the
  destination contract evaluable", under the tripwire's earlier name
  `the_switch_needs_the_destination_contract_evaluable`. *(H, as built)* H-5
  made all four checks observed and that tripwire, now
  `the_switch_needs_the_destination_contract_observed`
  (`crates/cli/tests/executor.rs:1810`), positive; but E's blockers were not
  only the contract: under `--replay` `execute_mutating` still refuses the live
  scope first (H.md "As built (H-5)" item 3), and a replayed probe is
  `NOT_OBSERVED`. K-5 tries them with the constant flipped in a scratch,
  uncommitted worktree, and nothing switch-on is committed
  (`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`); K-5's PR records the
  outcome either way, and if either blocker still holds K-8 records them as
  still handed, with the reason.

## Mutants each slice must kill

Run in an isolated worktree by a subagent; the session reads every survivor;
the worktree is removed afterwards.

- **K-2:** (1) restore maps `Removes` to `Keep`; (2) `migrate` maps the flag
  inverted. Both behaviour-preserving slices must show the existing tests kill
  them.
- **K-3:** (3) the default back to `Remove`; (4) the conflict between the two
  flags dropped; (5) `--retain-source` read as `Remove`; (6) the reverse
  dependency block applied to a `Keep` plan; (7) history consulted only under
  `--remove-source`; (8) the deprecation notice on stdout, or missing; (9) the
  plan's bytes differ with and without `--retain-source`.
- **K-4:** (10) the level from `retain_source` alone, ignoring a removal step;
  (11) absent `enteredBy` treated as `DEFAULT_VIEW`; (12) `UNKNOWN`,
  `DEPENDENCY`, `ORPHAN` or absent `sourceOwnership` treated as `TOP_LEVEL`,
  one mutant each (*rev. SG-M2*); (13) `ADVANCED` treated as `DEFAULT_VIEW`;
  (14) `admits` checks coverage only; (15) `admits` with `>` or `==` for `>=`;
  (16) `tie` mints `REMOVAL`; (17) any minting reaches `ADVANCED_REMOVAL`
  without Q2; (18) the removal digest's domain equal to the plan digest's;
  (19) the gate accepts a retention that disagrees with the steps, including a
  `VERIFY_REMOVAL` or `CLEANUP_REVIEW` under `retainSource: true`; (20)
  `sourceOwnership` left out of `plan_still_holds`' comparison; (20a) the gate
  accepts `DEFAULT_VIEW` with `DEPENDENCY`; (20b) `TOP_LEVEL` serialised as
  absence; (20c) `confirm_removal` or `tie_removal` minted without a `PLAN`
  witness ("Q1 minted without Q0", *rev. SR-L2*).
- **K-5:** (21) Q1 skipped; (22) Q1 reuses Q0's code; (23) Q2 skipped for
  `NAMED`, for unknown ownership, or for an absent field; (24) the plan phrase
  accepted for Q1; (25) `--removal-digest` accepted without `--remove-source`
  or `--plan-digest`; (26) the document's `confirmation` always `PLAN`; (27)
  `removalDigest` printed for an `ADVANCED_REMOVAL` plan; (28) a provider value
  unescaped in Q1; (28a) `tie_removal` accepted on an `ADVANCED_REMOVAL` plan
  (*rev. SR-L2*); (28b) `advanced.warning` shown for unknown intent.
- **K-6:** (29) the shell's re-read branch skipped; (30) the lock sample
  skipped before a removal; (30a) the lock sample moved before the re-read
  (*rev. SG-M1, SR-M1*); (30b) the re-verify judged on the verify step's
  earlier reads (*rev. SR-M1*); (31) a witness minted from a
  `VerifiedWithUnchecked` record; (32) a witness minted without
  `destination_contract_complete`; (33) a witness accepted for another step or
  command; (34) a witness from a re-read of another verify position; (34a)
  `guarding_verify` uses the first verify step for one use and the nearest for
  the other; (35) `mutation_dispatch` ignores a missing witness; (35a) the
  witness requirement keyed on `reverify.is_some()` instead of the step's kind
  (*rev. SR-M2*); (35b) one witness reused for a step's second command; (35c)
  the dependents re-read skipped, or a dependent ignored. Plus the
  compile-time proof that none of the four tokens is `Clone` or `Default`.
- **K-7:** (36) a known source build rendered with the unknown text; (37)
  `Missing` not blocking (if built); (38) `Unknown` blocking; (39) the block
  toward one provider only; (40) a destination already holding the package
  blocked by the toolchain; (41) a recording without the read answering
  anything but `Unknown`.

## Security notes for the reviewers

- K adds no program, no subcommand, no allowlist row and no dependency
  (`sha2` exists, ADR-0007); D9 re-uses capture's own inventory read. The
  frozen paths (§3 item 3) are untouched, and no guarded file changes; if a
  slice finds it must change one, it stops and uses the staging procedure.
- Nothing provider-derived enters a confirmation phrase; Q1 and Q2 render it
  escaped, and the new templates join `hostile_text.rs`.
- `ShownPlan` and the new tokens stay out of reach of a plan read from a file
  (C §9 (a)): every level is derived from the in-memory plan the run is about to
  execute, and `warnings` decides nothing.
- **Handed from H-5 (security M-1, accepted in writing in H's Reviews):** in
  this build `THIS_BUILD_DISPATCHES_MUTATIONS == false` is the single
  build-level barrier before a curated probe starts, and once it flips a plan
  with no `ADD_DESTINATION` command is probed before, and without, any
  mutation. K adds no barrier in front of that probe and removes none: its
  levels and witnesses guard `REMOVE_SOURCE`, not `VERIFY_DESTINATION`; and
  every K test that reaches a `VERIFY_DESTINATION` must use a test double whose
  `run_probe` starts nothing, as H's tests do (H.md "As built (H-5)" item 5). A K reviewer
  checks that no slice changes either fact.
- **Handed from S (SE-107, L6):** elevation consent is asked before the loop
  (`executor/mod.rs:929-949`), so it precedes every re-read; the credential
  prompt of the elevation program itself, if any, is part J's, and J either
  places it before the re-read or 04's residual sentence names its duration.

## What is not claimed

- No restore execution, no confirmation wired into `restore`, and no script
  route for a removing restore (D4).
- No history field for the level a run was confirmed at.
- The script route proves printing, not reading (SE-111), for both digests.
- That the reads a re-verify judged were fresh is held by the boundary-order
  test, not by type (D6).
- The residual window between the last re-check and the provider's uninstall
  stays accepted and declared (`04:482-491`); K bounds it as DP-048 says, it
  does not close it.
- The toolchain check does not see outdated Command Line Tools
  (`F-1-homebrew-support-tiers.md:76`) or an unaccepted Xcode licence.
- *(H, as built)* That a removal will often dispatch once the switch flips:
  since H-5 the four checks are observed, but H expects most live probe answers
  to be `UNCHECKED`, which keeps the source. While
  `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`, K's dispatch path is reached
  only under the test allowance.
- That the re-read before a removal re-runs checks 2, 4 or 5 (D6; H's declared
  window).
- `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`; no `v0.2.0` tag.

## Needs the owner (expected)

Nothing blocks K. Two items may appear:

- **A capture on a machine without a developer directory**, if R-29 cannot
  settle what `xcode-select -p` answers there from documentation, or if a
  filesystem read K-7 needs must be recorded: run `scripts/capture-fixture.sh`
  on such a disposable machine and commit the recording. K-7 then builds only
  the risk text until it exists.
- **Informational:** `v0.1.0` is not tagged. If it is tagged after K-3, the
  first release already ships opt-in removal, and SE-109's compatibility
  sentence ("if v0.1.0 ships before part K") no longer applies; SE-114 says so.

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
| spec-guardian | `9c9d859` | **approve with changes.** M1 D6's order not held by type: the lock sample must consume the re-read's token; one witness per command; a mutant moving the lock sample first. M2 the level derivation not total: `REMOVAL` only for `(Some(DEFAULT_VIEW), Some(TOP_LEVEL))`, exhaustive match; gate refuses an inconsistent pair; mutant 12 to `DEPENDENCY`/`ORPHAN`; `TOP_LEVEL` written explicitly, distinct from absence in the digest. M3 dependents appearing between the lock and the removal: (a) re-read or (b) declare. M4 SE-114 amended in K-4 and K-6's own PRs; `RETENTION_DISAGREES_WITH_STEPS` in `migrate.v2.md` and the CHANGELOG in K-4. M5 K-0's timing. L1 say `PLAN` is today's level and K defines two. L2 `UNKNOWN`'s own key, no `advanced.warning`. L3 one phrasing for `--retain-source`'s life. L4 name `doctor`'s absence test; "already met" in SE-109. L5 a filesystem read needs replay coverage or an owner capture. L6 `pkgutil` research-only. L7 `confirmation` optional or justified. L8 a removing restore has no script route (SE-114); cross-reference C's `UNKNOWN` decision from SE-109 and the ledger | M1: D6 chain, `clear_removal` consumes the dependents token (which consumes `DestinationReverified`), borrows the source's `ConfigurationRechecked` and judges the removal's lock sample; per-command witness; mutant 30a. Deviation, stated: `provider_locks_clear` keeps its signature for the four run-level samples, which have no re-read; for a removal it is called only inside `clear_removal`. M2: D5's table and total-derivation bullet, `ENTRY_DISAGREES_WITH_OWNERSHIP`, mutants 12/20a/20b, D3. M3: **(a)**, D9 — the dependents come from capture's existing inventory read, so no allowlist growth; new INV-04 test. M4: D's preamble, D3, slices K-4/K-6. M5: Timing paragraph. L1: D5 heading and lead. L2: D5 questions bullet, mutant 28b. L3: D1 "throughout the v0.2.x line". L4: D7. L5: D7 filesystem-read bullet, Needs the owner. L6: R-29 item 4. L7: D3. L8: D4, D5 `UNKNOWN` bullet, K-3 row |
| security-reviewer | `9c9d859` | **approve with changes.** M1 as SG-M1, and `destination_still_verified` judges caller-built reads, so the token proves the judgement, not freshness: an explicit per-command boundary-order test named in SE-114; mutants "lock sample before re-read" and "re-verify on earlier reads"; say what is by type and what by test. M2 the witness requirement decided from the step's kind in `application`, not `reverify.is_some()` or argv; a future dispatched uninstalling step covered or refused. L1 as SG-M2. L2 the higher mints consume the lower `ShownPlan` by value; the `ADVANCED_REMOVAL` digest refusal at the tie point, every build, stdout empty; mutants "Q1 without Q0" and "`tie_removal` on `ADVANCED_REMOVAL`". L3 R-29 by allowlist; no developer variables set. L4 a CLT `stat` through the recorded machine reader; missing in a recording is `Unknown`; INV-15 covers it. L5 the retention refusal also for `VERIFY_REMOVAL`/`CLEANUP_REVIEW` under `retainSource: true`. I1 one function picks the guarding verify step. I2 `RemovalCleared` per command, consumed by value like `ConsentGiven::covers`. I3 K-5 keeps 06's "until then no plan executes" while the switch is false | M1: D6 "What is held by type, and what by test", `invariant_01_each_removal_command_passes_its_boundaries_in_order`, mutants 30a/30b. M2: D6 first bullet, `invariant_01_only_add_and_remove_are_ever_dispatched`, mutant 35a. L1: as SG-M2. L2: D5 enforcement and script bullets, mutants 20c/28a. L3: R-29 allowlist and "Never". L4: D7 filesystem-read bullet, mutant 41. L5: D5 gate bullet, mutant 19. I1: D6 `guarding_verify` (today `.rposition` vs `.position`), test and mutant 34a. I2: D6 step 5, mutant 35b. I3: D5 last bullet; the sentence "until K-5 replaces it" removed |
| Citation refresh against H as built (Timing) | `9fc9ae0` (merge of `origin/wo16/H-8-observe`, `afc5d5b`) | Drift: H-stage-2 never happened (L1 merged directly, `e41785b`); the liveness observer is on, so "every removal is `SOURCE_KEPT_UNCHECKED`" was stale; line shifts in `execution_gate.rs` (+1 from `:646`), `plan.rs` (+7 from `:592`), 04 (the "Not yet met" sentence, now `:492-498`, rewritten by H-5) and `spec-evolution.md` (+4 from `:1847`); E's tripwire name; H's re-read asks check 3 only, which K's D6 had not said; security M-1 (H-5) unmentioned | Timing paragraph; "When `REMOVE_SOURCE` is dispatched today" (observer, re-read scope, new M-1 paragraph); D6 step 4 and its by-type bullet; D7; D8; K-6 row; E's handed tests; Security notes; What is not claimed. **No slice changed:** K-6's row gains the re-read window in 04's residual and M-1's sentence in SE-114, both within its existing scope |
| spec-guardian | `a6a0c47` (at `c93f253`, with `main` at `26fe181`) | **approve with changes.** B-1 K-0's own rule: H's ledger row not yet `merged`. S-1 Timing still said H-5 in review. S-2 the re-read window is declared only in H's plan, not in 04 or SE-113, while `reverify_questions`'s comment says "the specification". S-3 D8's K-8 record and K-3's M-1 sentence unscheduled. S-4 "switch-on worktree" undefined and unscheduled. S-5 a bare `executor.rs:1076-1077` read as the test file. N-1 bare `executor.rs:1694-1707`. N-2 the `next_action_with` chain is indirect. N-3 the no-spawn test-double rule held by review only. All citations otherwise correct | B-1: K-0 merges after the ledger PR #77. S-1, S-2 (K-6 declares the window in 04 and SE-114 and fixes the comment), S-3 (K-3 and K-8 rows), S-4 (scratch, uncommitted; K-5's row), S-5 and N-1 fixed. N-2 kept, matching H's wording. N-3 kept as a review rule, as in H item 5; a structural check is a candidate for part L |
| spec-guardian (K-3) | `83634b5` | **approve with changes.** 1 `plan.source_kept` reached `restore`'s output, advising a flag it does not take (D4); 2 INV-16's test not named after the invariant; 3 the home-rooted destination test lost its removing case; 4 the notice untested on `plan`; 5 the tripwire names K-6, the row K-8; 6 SE-109's compatibility line stale, SE-114 lacked the v0.1.0 sentence; 7 stale comments | All folded in `c0710b1`: `human::source_kept` for `plan` and `migrate` only, the banner names no flag, `a_restore_that_keeps_what_the_run_added_never_advises_remove_source`; `invariant_16_a_default_re_run_consults_history_and_remove_source_does_not`; the loop over `--remove-source`; the notice test over both commands; K-8; SE-109 superseded, SE-114 extended |
| docs-auditor (K-3) | `83634b5` | **no blocking defect.** H1 Dependencies-and-Safety's dependents line named a command that no longer prints it; H2 04 said the opt-in was not built; M1-M5, L1-L8 wording | All folded in `c0710b1` but L4's FAQ line, left as true of `explain` |
| mutation proof (K-3) | `83634b5` | Mutants (3)-(10) killed, two-site ones split: **5p** (`--retain-source` read as Remove at the `plan` site only) and **7a** (history skipped in the re-plan under the lock, `migrate.rs:292`) survived | 5p killed by `c0710b1`'s notice test over `plan`. **7a stands, handed to K-6:** the re-plan runs only past the execution gate, which refuses every plan in this build, so no test reaches it without the run-loop harness K-6 builds for the witness |
| spec-guardian (K-4) | `21eaa85` | **approve with changes.** No invariant violated; both deviations (refusals scoped to dispatching, non-`MANUAL_REVIEW` plans; `NAMED`/`ADVANCED` with `TOP_LEVEL` not refused) correct and recorded. 1 INV-02's test lacked a `REMOVAL` witness on an `ADVANCED_REMOVAL` plan; 2 no direction-neutrality test for the levels; 3 the script-refusal test not named after its invariant; 4 SE-114's second deviation argued by planner behaviour, not pair consistency, and its `MANUAL_REVIEW` remark unconditional | All folded in `7a2f5ee`: the `REMOVAL`-on-`NAMED` case in `invariant_02_the_run_refuses_a_plan_confirmed_at_a_lower_level`; `invariant_17_confirmation_levels_are_direction_neutral`; `invariant_02_a_script_cannot_confirm_an_advanced_removal`; SE-114 reworded, the remark conditional on today's two providers; an "as built, K-4" note under D5 |
| security-reviewer (K-4) | `21eaa85` | **sound.** L1 and informational notes only; no injection, elevation or state-integrity finding | L1 and the infos folded in `7a2f5ee` |
| docs-auditor (K-4) | `21eaa85` | M1, M2 and lows on `plan.v2.md`, `migrate.v2.md`, the CHANGELOG and SE-114 wording; L2 04's "Not yet met"; L5 `tie_removal`'s `Usage` framing | M1, M2 and the lows folded in `7a2f5ee`. **L2 handed to K-6** (04's sentence changes with the witness); **L5 handed to K-5** (the CLI surface that prints it) |
| mutation proof (K-4) | `21eaa85` | Mutants 10-21 (12a-d, 15a/b, 19a/b, 20/20a-c counted apart): **all killed**, every one compiling. 18 killed by behaviour as well as by the domain-string assertion. Thin kills, one test each: 15b, 17, 20a, 20c | Accepted as adequate in writing. K-5's CLI tests for Q1/Q2 add a second kill for 17 and 20c (the phrase refused at the wrong level); 20a's second kill is K-6's run harness, where a `(DEFAULT_VIEW, DEPENDENCY)` plan can reach dispatch |
| spec-guardian (K-5) | `de6df78` | **approve with changes.** 1 `invariant_02_a_declined_plan_question_exits_6_before_anything_runs` not extended to Q1/Q2; 2 E's handed tests' outcome unrecorded; 3 SE-114 Status garbled; 4 stale K-4 text in SE-114; 5 the exit-4-before-2 order for `ADVANCED_REMOVAL` stated only in SE-114; 6 `tie_to_plan_level(` outside the word scan; 7 the first deviation's reason misstated; 9 Q1/Q2 tested toward MacPorts only; 10 field order; 11 `CodeUnavailable`'s text after Q0 | Folded in the review commit: 1 the exit-6 mapping asserted in both Q1/Q2 tests; 2 recorded below and in SE-114's evidence; 3, 4, 7 SE-114 rewritten; 5 removed at the root, `script_route_open` before `tie` (no deviation left); 6 `tie_to_plan_level(` and `script_route_open(` held to `migrate.rs`; 10 `migrate`'s document writes `warnings` before `confirmation`; 11 "nothing further was asked". 9 accepted: the prompt and `plan_tie.rs` name no provider, and the derivation is pinned both ways by `invariant_17_confirmation_levels_are_direction_neutral` |
| security-reviewer (K-5) | `de6df78` | **no blocking findings.** L1 `tie_to_plan_level(` unscanned; L2 an `ADVANCED_REMOVAL` plan with a stale digest exits 4, not 2; I1 the order scan depends on formatting; I2 values escaped twice, idempotently | L1 and L2 as spec-guardian 6 and 5; I1 the scan now finds `)?)` after `echoed.removal.as_ref(),` across any whitespace; I2 no change |
| docs-auditor (K-5) | `de6df78` | M1 SE-114 Status; M2 exit 2 in five documents where a stale digest gave 4; M3 a plan-digest hint printed for `ADVANCED_REMOVAL`; M4 (K-4's L5) the no-script-route refusal framed as `USAGE` "not valid", in English only; L1 Troubleshooting's set too narrow; L2 present-tense prompts that this build never asks; L3 Q2's text over a `TOP_LEVEL` or `UNKNOWN` row under `ADVANCED`/`NAMED`; L4 `restore.v2` example; L5 field order; L6 pt-BR "etapa", "removê-la"; L7 clap help; L8 `confirm.*` beside `consent.*` | M1-M4 folded: M2 by the code order; M3 the hint withheld at `ADVANCED_REMOVAL`, `planDigest`'s row says so; M4 **a new identifier `NO_SCRIPT_ROUTE`** (exit 2, `error.no_script_route` in both catalogues, says nothing was changed and where it can be confirmed). L1, L2, L4-L7 folded. L3 accepted in SE-114, for part L with the questions' first real use. L8 kept: `confirm.*` is the removal's own questions, `consent.*` the plan's and elevation's |
| mutation proof (K-5) | `de6df78` | 17 mutants (21, 22, 23a-c, 24, 25a-c, 26a/b, 27, 28 package and provider, 28a, 28b, 29x), every one compiling: **13 killed, 4 survived**. 26b, `plan --json`'s `confirmation` forced to `PLAN`, was a real gap. Three are equivalent: 25c, because `--plan-digest`'s own check refuses a missing `--yes` first and clap requires it beside `--removal-digest`; 28 (both values), because `Messages::render` escapes every parameter, so the call-site escape in `removal_steps` is a second, idempotent one (security-reviewer I2). Thin kills, one test each: 25a, 25b, 26a, 27, 28b, 29x | 26b killed by the new `the_plan_document_says_how_strongly_the_plan_must_be_confirmed` (`crates/cli/tests/migrate.rs`). 25c and 28 accepted as equivalent, and the call-site escape kept as defence in depth for a renderer that stops escaping. Thin kills accepted: each is the one command-line test that exercises that surface; mutant 28's test asserts what reaches the screen, which is the property, whoever escapes it |
| E's handed tests (K-5, scratch switch-on worktree, uncommitted) | `de6df78` | (1) `--yes` with no terminal and no `--plan-digest` is `USAGE` (exit 2), stdout empty, nothing written: **buildable and passes**, but only on an edited snapshot (a replay context with the committed live capture, history on this machine, and the source's executable list set known and empty), because a replayed row is `SAFETY_UNVERIFIABLE` and the live capture's `SET_PRIMARY` is `PRIMARY_REFUSED` (`EXECUTABLES_UNKNOWN`, then `PRIMARY_UNPREDICTABLE`). (2) an interactively confirmed plan reaching the run loop: **not buildable through `migrate_with`** (`confirm_the_plan` reads the process's own `is_terminal()`, no seam); one level down, `consent::confirm_plan` with scripted streams then `execute_mutating` passes `admits`, the history probe and the run lock, and halts at the first provider-lock sample, `PROVIDER_LOCKS_NOT_DETERMINED` (a replayed lock probe answers `UNKNOWABLE`). With the switch on, 46 existing tests fail, each a test that pins this build's refusals | Recorded; nothing committed. **Still handed, to K-8:** (1) needs a fixture whose `SET_PRIMARY` is decidable, (2) a terminal seam in `confirm_the_plan` and a lock answer a replay can give; the destination contract is no longer the blocker |
