<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part K — removal becomes opt-in (DP-048 (c)), DP-037's confirmation levels, DP-050

Plan written 2026-11-15, at `4875d89`, for `prompts/16-complete-mutating-product.md`
§5 K, before any code. Reviewed by `spec-guardian` and `security-reviewer`
before any code; their review is the approval (§4). Every line cited below was
re-read at `4875d89`.

**Timing.** Part H is `in review` (PR #73) when this is written, with H-stage-2
and H-5 still to come (`docs/passes/16-progress.md:31`). §5 runs S..L one at a
time, each after the previous merges, and the ledger's K row depends on H. So
this plan is drafted early and **no K slice opens before H's row reads
`merged`**; on H's merge the session re-reads every citation marked *(H)*
below and records any drift in the Reviews table. Whether K-0 itself, being
docs only, may merge before H does is a question for the plan review, not taken
here.

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
  part C bullet, `docs/spec-evolution.md:8988-8998`): (a) the level is a pure
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
  `PLAN_SCHEMA_VERSION = 2` (`:600`). The planner decides `entered_by` from the
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
(`application/src/executor.rs:1742-1788`, which answers `Result<(), Precondition>`);
the run lock still current (`:1098`); the source's configuration re-check
(`:1110-1129`); the provider-lock sample (`:1133-1137`, `provider_locks_clear`
`application/src/executor.rs:450-458`, `Result<(), ProviderLockRefusal>`); then
`mutation_dispatch` (`executor/mod.rs:1140-1148`, `:1359-1367`), which takes the
source's `ConfigurationRechecked` token by value and nothing else for the
removal. The gate checks the contract's shape (`destination_contract_complete`,
`crates/application/src/execution_gate.rs:705`) and that each removal is
verified after (`:1069-1083`). **So all three of DP-048's conditions are met by
the order of statements in the shell; none is a value the dispatch requires.**
*(H)* Until H-5 flips the liveness observer
(`THIS_BUILD_OBSERVES_LIVENESS_PROBE = false`, SE-113's H-4 bullet), liveness
answers `Unchecked`, so every removal is `SOURCE_KEPT_UNCHECKED`.

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
under SE-109 with R-29.

**D1. `--retain-source` is an accepted no-op, with a deprecation notice, for the
v0.2.x line**, and is removed in the first release after v0.2.0 by its own SE
entry. Given with `--remove-source` it is a usage error (exit 2, before
anything is built). The notice goes to stderr only
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
  `UNKNOWN`, optional; absent means the strictest level, as for `enteredBy`);
  and at the document root, beside `warnings`, `confirmation` (`PLAN` |
  `REMOVAL` | `ADVANCED_REMOVAL`), derived when the document is written and
  never stored on the plan, like `warnings`.
- `migrate.v2`: the same `confirmation`, and `removalDigest` (D5) when the
  level is `REMOVAL`. `restore.v2`: `confirmation`.
- `plan.v1`, `migrate.v1`, `restore.v1` and `history.v1`–`v3` are superseded or
  only read; their `retainSource` means the same, and they are not edited.
- 06's synopsis (`:31-35`) is a Tier 3 MUST: changed with SE-114.

**D4. `restore` keeps deciding removal from the record**
(`application/src/restore.rs:111-133`) and gains no `--remove-source`. Its plan
is confirmed at the level its own steps derive (D5), never lower. *Reason:* an
undo that leaves what the run added is not an undo; INV-01's order is
unchanged (`invariant_01_a_restore_plan_adds_before_it_removes`); and the
interaction SE-107 noted (`docs/spec-evolution.md:8731-8732`: a removal
*refused* by release policy would refuse every undo of an install) does not
arise, because K makes removal opt-in for `migrate`, not refused. Since the
default now keeps the source, 04's "cheap case" (`04:242-247`) becomes the
common one. Restore's execution stays unbuilt (`cli/src/app/restore.rs:288`);
K renders its level and wires nothing.

**D5. Three confirmation levels, ordered by inclusion.**

| Level | When (derived from the plan) | A person at a terminal | `--yes --plan-digest` |
|---|---|---|---|
| `PLAN` | no removal | Q0 `run plan <c0>` (today's) | `--plan-digest <d>` (today's) |
| `REMOVAL` | a removal, `enteredBy: DEFAULT_VIEW`, `sourceOwnership` `TOP_LEVEL` | Q0, then Q1 `remove source <c1>` | `--plan-digest <d>` **and** `--removal-digest <r>` |
| `ADVANCED_REMOVAL` | a removal, and `enteredBy` `ADVANCED`, `NAMED` or absent, or `sourceOwnership` `UNKNOWN` or absent | Q0, Q1, then Q2 `confirm advanced removal <c2>` | **refused**: exit 2 (`USAGE`), stdout empty, nothing run |

- **"A removal"** is `!retain_source` **or** any `REMOVE_SOURCE` step: either
  signal raises the level. The gate also refuses a plan where the two disagree
  (`RETENTION_DISAGREES_WITH_STEPS`), since `history.retainSource` would then
  lie. The planner never builds one.
- **"Stronger" is a checkable property:** whatever confirms a level also
  confirms every lower one, and something that confirms the lower one does not
  confirm it. Each interactive level asks every question of the level below
  plus one, each with its own fresh code (the code rule of `plan_tie.rs:20-22`);
  so 06's "stronger than in normal mode" holds by construction, and normal-mode
  removal is stronger than an install's.
- **The questions** are asked in `confirm_the_plan` (`migrate.rs:343-373`), all
  before the state directory is touched, never inside the loop. Q1 lists each
  `REMOVE_SOURCE` step: package and provider, escaped as every rendered
  provider value is (`crates/cli/tests/hostile_text.rs`). Q2 re-shows
  `advanced.warning` and why the row is outside the default view (`NAMED`,
  `ADVANCED`, unknown intent, or a plan without the field). The phrases are
  ASCII and never translated, as `run plan` is; nothing provider-derived is in
  a phrase.
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
  person saw `advanced.warning`. Exit 2, like a missing terminal
  (`plan_tie.rs:195-198`), because the route is structurally unable to confirm
  it. Reversible: a later SE entry may add one.
- **`UNKNOWN` ownership: changed for removal, kept for the view.** C's choice
  stands for what the default view lists and for `enteredBy` (an `UNKNOWN` row
  is `DEFAULT_VIEW`, no warning). For a removal it is the strictest level.
  *Reason:* with no record of intent (a cask with no receipt, R-12) nothing
  shows the row is not something another package needs, which is the case
  INV-13 guards. So the level stays a function of the plan alone (C §9 (a)),
  the plan carries the source row's ownership as `sourceOwnership`; E's digest
  and `plan_still_holds` then cover it, so an ownership that moves between
  showing and running refuses the run, as a moved `enteredBy` does
  (`a_plan_whose_entry_moved_under_the_lock_is_refused`).
- **Where the level is enforced.** `ShownPlan` gains the level it was confirmed
  at; `confirm_shown` and `tie` mint `PLAN`, `confirm_removal` and
  `tie_removal` raise it to `REMOVAL`, `confirm_advanced_removal` to
  `ADVANCED_REMOVAL`, all in `plan_tie.rs`, with the word scans extended to
  their callers. The run loop's check at `executor/mod.rs:900` becomes
  `ShownPlan::admits(plan)`: covers **and** level at least
  `confirmation_level(plan)`, derived again from the plan about to run. The
  decision is the application's (SE-092); the shell only asks.

**D6. The dispatch requires a removal witness.** DP-048's three conditions
become one opaque value, `RemovalCleared`, minted only by
`application::executor::clear_removal(run, position, records, reverified,
locks)`, which requires: the step at `position` is a `REMOVE_SOURCE`; its
guarding `VERIFY_DESTINATION` is recorded `Verified` and
`destination_contract_complete` holds for that step (the six checks: active,
newly present when the plan adds, integrity, ownership, linkage, liveness —
`execution_gate.rs:705-740`; H made `Verified` require all four `Holds`);
`reverified: DestinationReverified`, which `destination_still_verified` now
returns instead of `()` and which names that step and verify position; and
`locks: ProviderLocksClear`, which `provider_locks_clear` now returns instead
of `()`. None of the three is `Clone`, `Default` or constructible outside
`application` (the compile-time idiom of `plan_tie.rs:46-66`).
`mutation_dispatch` (`executor/mod.rs:1359`) takes `Option<RemovalCleared>`
and an application function refuses a removal command without a witness for
that step, or a witness on any other step. The shell's order is unchanged
(re-read first, lock sample last, SE-107 M1); it now cannot reach dispatch out
of that order, because the second token is minted only after the first and the
witness needs both. *Reason:* "only when all of these hold" stops being a
property of statement order that a refactor can break silently.

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
holds the package. `CommandLineTools::Unknown` stays non-blocking with text
that says it could not be told. If R-29 shows neither proves it, nothing
blocks and the risk text says why. `doctor`'s `command_line_tools` finding
exists (`doctor.rs:575-594`), so no new check is added unless R-29 changes what
`Installed` means (a selected Xcode is not the Command Line Tools,
`docs/research/R-27-no-cross-provider-linkage.md:5`), in which case the wording
changes under `/add-doctor-check`'s procedure.

**D8. Coexistence is the default outcome (INV-18).** A plan without
`--remove-source` ends after `VERIFY_RESOLUTION` with `RECORD_HISTORY`; the
rendered plan says the source is kept and how to remove it (`plan.source_kept`).
A consequence, recorded: every default live `migrate` now consults history
(`migrate.rs:291`, `:517`), so a default re-run is recognised as done
(INV-16), and `migrate.v2.md:217-220` changes from "only by
`--retain-source`" to "unless `--remove-source`".

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

**Read-only observations it makes:**

1. `/usr/bin/xcode-select -p`, exit status and stdout (already permitted, and
   in every recording, `R-28:40`).
2. `readlink /var/db/xcode_select_link` — whether the selection is a stored
   link, and whether `-p` reports it when its target is gone (a lead to
   confirm, not a fact).
3. `stat` of `/Library/Developer/CommandLineTools`, `usr/bin/clang`,
   `usr/bin/make` and `SDKs/` under it, and of the selected Xcode's
   `Contents/Developer/usr/bin` — stat only; nothing is executed.
4. The Command Line Tools package receipt, read as a file
   (`/Library/Apple/System/Library/Receipts/` or `/var/db/receipts/`), or by
   `pkgutil --pkg-info`, a read-only verb.
5. Documentation, dated: `man xcode-select` (local, already quoted at
   `R-27:368`), Apple's "Installing the command-line tools", Homebrew's
   installation requirements (`F-1-homebrew-support-tiers.md:206`: the
   Command Line Tools or Xcode), and MacPorts' install guide on what a source
   build needs.

**Never run:** `xcode-select --install`, `-s`, `--switch`, `--reset`; any
elevation; `xcrun`; any `/usr/bin` developer shim (`clang`, `cc`, `make`,
`git`, `python3`), because R-27 finding 24 shows a lookup can raise the install
dialog; any `brew` or `port` mutation; no deletion or rename of anything.

**What this machine cannot show:** absence. It has both, and producing absence
is a mutation. So the absent case comes from documentation, or is an owner
capture on a disposable machine; the note says which, and K-7 builds the block
only on what the note can support.

## Slices

| Slice | Branch | What | Depends on |
|---|---|---|---|
| K-0 | `wo16/K-0-plan` | This plan and its reviews. Docs only | H merged |
| K-1 | `wo16/K-1-toolchain-note` | R-29: the note and its checklist entry. Docs only, `provider-researcher`, read-only | K-0 |
| K-2 | `wo16/K-2-source-fate` | D2: `SourceFate` replaces `PlanRequest.retain_source`, every site mapped explicitly, **no behaviour change** (the CLI still maps an absent flag to `Remove`) | K-0 |
| K-3 | `wo16/K-3-opt-in` | D1, D3 (`retainSource` text), D4, D8: `--remove-source` on `plan` and `migrate`, the default `Keep`, `--retain-source` the no-op with its notice and the conflict; 06's synopsis; 04's restore paragraph; catalogue `en` and `pt-BR` (`plan.dry_run_only`, `migrate.by_hand`, `doctor.provider_prefix_is_not_absolute`, `history.show_retain_source`, `plan.source_kept`, `cli.retain_source_deprecated`); documented samples regenerated; wiki's three pages; SE-114 first half | K-2 |
| K-4 | `wo16/K-4-levels` | D5's application and domain half: `sourceOwnership`, `ConfirmationLevel`, `confirmation_level`, the gate's `RETENTION_DISAGREES_WITH_STEPS`, `ShownPlan`'s level and `admits`, `tie_removal` and the removal digest, the run loop's check | K-3 |
| K-5 | `wo16/K-5-confirm` | D5's command-line half: Q1 and Q2, `--removal-digest`, `confirmation` and `removalDigest` in the documents, the schemas, 06's advanced-mode MUST and its confirmation paragraph, C's invariant test completed; SE-114 second half | K-4 |
| K-6 | `wo16/K-6-witness` | D6: `DestinationReverified`, `ProviderLocksClear`, `RemovalCleared`, `clear_removal`, `mutation_dispatch`'s parameter; 04's "Not yet met" (`:493-497`) for the opt-in and the dispatch conditions becomes met | K-5 |
| K-7 | `wo16/K-7-source-build` | D7, on R-29's answer: the risk's reason, the toolchain block if supported, `doctor`'s wording if changed; 04's source-build sentence (`:262-268`); SE-109 propagation bullet | K-1, K-6 |
| K-8 | `wo16/K-8-close` | `the_switch_needs_removal_opt_in` turned positive; ledger, `16-report.md`'s K section, CHANGELOG; `docs-auditor` | K-7 |

Until K-5 merges a `--remove-source` plan is confirmed by Q0 alone. Nothing
runs: the switch is off (`grep -n THIS_BUILD_DISPATCHES_MUTATIONS`), and 06's
"until then no plan executes" (`:230-231`) stays true until K-5 replaces it.

## Tests, per invariant, by name

New tests are marked *new*; the rest are extended.

- **INV-01.** *new* `invariant_01_a_removal_dispatches_only_with_its_witness`
  (application: no witness, a witness for another step, a witness from a
  re-read of another verify position — each refused; both directions);
  *new* `invariant_01_a_removal_witness_needs_every_check_of_the_contract`
  (a `Verified` record on a step the contract check fails, and a
  `VerifiedWithUnchecked`, mint nothing); extended
  `invariant_01_no_removal_dispatches_unless_the_lock_probe_answered_immediately_before_it`
  and `invariant_01_a_destination_that_is_not_there_stops_the_removal`
  (`crates/cli/tests/executor_wiring.rs`) to assert through the witness;
  `invariant_01_a_restore_plan_adds_before_it_removes` unchanged and still
  green after D2/D4.
- **INV-02.** *new* `invariant_02_the_run_refuses_a_plan_confirmed_at_a_lower_level`
  (a `PLAN` witness on a `REMOVAL` plan, a `REMOVAL` witness on an
  `ADVANCED_REMOVAL` plan: `PLAN_NOT_SHOWN`, before the probe and the lock);
  *new* `invariant_02_a_removal_needs_its_own_echoed_digest`;
  *new* `invariant_02_a_script_cannot_confirm_an_advanced_removal`;
  extended `invariant_02_a_declined_plan_question_exits_6_before_anything_runs`
  (`plan_tie.rs:215`) to Q1 and Q2.
- **INV-03.** *new* `invariant_03_a_removal_without_its_witness_ends_partial_with_the_source_kept`;
  extended `invariant_03_a_removal_whose_destination_was_not_reverified_ends_partial`.
- **INV-04.** Extended `invariant_04_removal_with_dependents_is_blocked_and_explained`
  (`crates/application/tests/invariants.rs`): dependents block a `Remove`
  plan and do not block a `Keep` plan; extended
  `invariant_04_a_dependent_that_appeared_under_the_lock_refuses_the_run` to a
  removing run.
- **INV-13 (C's blocking half).** `invariant_13_a_named_dependency_warns_and_needs_the_stronger_confirmation`
  (`invariants.rs:2034-2064`) loses its "pending" doc comment and asserts: a
  `NAMED` removal is `ADVANCED_REMOVAL`, strictly above a `DEFAULT_VIEW`
  removal, strictly above an install; a `NAMED` plan that keeps the source is
  `PLAN`; and the run refuses it with a `REMOVAL` witness. *new*
  `invariant_13_an_absent_entry_or_unknown_ownership_removal_is_the_strictest`.
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
  if K-7 builds the block (the recordings carry `xcode-select -p`).
- **Tripwire.** `the_switch_needs_removal_opt_in`
  (`crates/cli/tests/history_preconditions.rs:2530-2546`) becomes positive and
  held whatever the switch: the CLI's default plans keep the source in both
  directions, and `mutation_dispatch` cannot take a removal command without a
  witness (structural scan). `source_build_and_unknown_are_not_manual_review`
  (`invariants.rs:1855`) is rewritten: its pinned sequence loses the three
  removal steps under `Keep`, and a `Remove` case keeps them.
- **E's two handed tests** (E.md:1220-1231): K-5 builds them if the contract
  is evaluable by then (*(H)*: after H-5), otherwise K-8 records them as still
  handed, with the reason.

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
- **K-4:** (10) the level from `retain_source` alone, ignoring a
  `REMOVE_SOURCE` step; (11) absent `enteredBy` treated as `DEFAULT_VIEW`; (12)
  `UNKNOWN` or absent `sourceOwnership` treated as `TOP_LEVEL`; (13) `ADVANCED`
  treated as `DEFAULT_VIEW`; (14) `admits` checks coverage only; (15) `admits`
  with `>` or `==` for `>=`; (16) `tie` mints `REMOVAL`; (17) any minting
  reaches `ADVANCED_REMOVAL` without Q2; (18) the removal digest's domain equal
  to the plan digest's; (19) the gate accepts a retention that disagrees with
  the steps; (20) `sourceOwnership` left out of `plan_still_holds`' comparison
  (it is plan equality today; the mutant skips the field).
- **K-5:** (21) Q1 skipped; (22) Q1 reuses Q0's code; (23) Q2 skipped for
  `NAMED`, for unknown ownership, or for an absent field; (24) the plan phrase
  accepted for Q1; (25) `--removal-digest` accepted without `--remove-source`
  or `--plan-digest`; (26) the document's `confirmation` always `PLAN`; (27)
  `removalDigest` printed for an `ADVANCED_REMOVAL` plan; (28) a provider value
  unescaped in Q1.
- **K-6:** (29) the shell's re-read branch skipped; (30) the lock sample
  skipped before a removal; (31) a witness minted from a `VerifiedWithUnchecked`
  record; (32) a witness minted without `destination_contract_complete`; (33)
  a witness accepted for another step; (34) a witness from a re-read of another
  verify position; (35) `mutation_dispatch` ignores a missing witness. Plus the
  compile-time proof that none of the three tokens is `Clone` or `Default`.
- **K-7:** (36) a known source build rendered with the unknown text; (37)
  `Missing` not blocking (if built); (38) `Unknown` blocking; (39) the block
  toward one provider only; (40) a destination already holding the package
  blocked by the toolchain.

## Security notes for the reviewers

- K adds no program, no subcommand, no allowlist row and no dependency
  (`sha2` exists, ADR-0007). The frozen paths (§3 item 3) are untouched, and no
  guarded file changes; if a slice finds it must change one, it stops and uses
  the staging procedure.
- Nothing provider-derived enters a confirmation phrase; Q1 and Q2 render it
  escaped, and the new templates join `hostile_text.rs`.
- `ShownPlan` and the new tokens stay out of reach of a plan read from a file
  (C §9 (a)): every level is derived from the in-memory plan the run is about to
  execute, and `warnings` decides nothing.
- **Handed from S (SE-107, L6):** elevation consent is asked before the loop
  (`executor/mod.rs:929-949`), so it precedes every re-read; the credential
  prompt of the elevation program itself, if any, is part J's, and J either
  places it before the re-read or 04's residual sentence names its duration.

## What is not claimed

- No restore execution, and no confirmation wired into `restore` (D4).
- No history field for the level a run was confirmed at.
- The script route proves printing, not reading (SE-111), for both digests.
- The residual window between the last re-check and the provider's uninstall
  stays accepted and declared (`04:482-491`); K bounds it as DP-048 says, it
  does not close it.
- The toolchain check does not see outdated Command Line Tools
  (`F-1-homebrew-support-tiers.md:76`) or an unaccepted Xcode licence.
- *(H)* Until H-5, every live removal is passed over as unchecked; K's dispatch
  path is reached only under the test allowance.
- `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`; no `v0.2.0` tag.

## Needs the owner (expected)

Nothing blocks K. Two items may appear:

- **A capture on a machine without a developer directory**, if R-29 cannot
  settle what `xcode-select -p` answers there from documentation: run
  `scripts/capture-fixture.sh` on such a disposable machine and commit the
  recording. K-7 then builds only the risk text until it exists.
- **Informational:** `v0.1.0` is not tagged. If it is tagged after K-3, the
  first release already ships opt-in removal, and SE-109's compatibility
  sentence ("if v0.1.0 ships before part K") no longer applies; SE-114 says so.

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
