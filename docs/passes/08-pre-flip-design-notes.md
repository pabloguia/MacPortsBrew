<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08 — design notes for the work before the flip (slices 6, 7 and 8)

Written 2026-10-18, after slice 5 merged (PR #10), from four read-only design
studies of `main` at `e463ca5` (the studies themselves were not kept; this file is what remains of them). It is the **starting design** for
[`prompts/08-slice-6-verification-before-removal.md`](../../prompts/08-slice-6-verification-before-removal.md),
[`08-slice-7-plan-tie-and-consent.md`](../../prompts/08-slice-7-plan-tie-and-consent.md) and
[`08-slice-8-primary-and-resolution.md`](../../prompts/08-slice-8-primary-and-resolution.md).

**How to use this file.** It is not a plan. Each slice still writes its own
`docs/passes/08-slice-N-plan.md`, reviewed by `spec-guardian` and
`security-reviewer` before any code. Every `file:line` here was true at
`e463ca5`; **re-read each one before relying on it**, because lines move. Where
this file and the specification disagree, the specification wins and this file
is the bug. Owner decisions are DP-046..DP-050 in `docs/decisions-pending.md`.

## What stands between `main` and the flip

`THIS_BUILD_DISPATCHES_MUTATIONS` is `false`. The tests in
`crates/cli/tests/history_preconditions.rs` that fail when it becomes `true`,
and who closes each:

| Tripwire | Closed by |
|---|---|
| `the_switch_needs_the_destination_contract_evaluable` (≈1269) — today it asks only about `PROVIDER_REPORTS_ACTIVE` | slice 6 part B makes that check evaluable **and widens the tripwire to every check a planner-built `VERIFY_DESTINATION` carries**; it then stays failing until DP-051 is settled |
| `the_switch_needs_the_window_before_removal_closed_or_accepted` (≈1244) | slice 6, part C (+ DP-048) — **done** (work order 16 part S-1): replaced by the positive `the_destination_is_reread_before_removal`, and the widened destination tripwire moved to `crates/cli/tests/executor.rs` |
| `the_switch_needs_manual_review_for_an_unavailable_artifact` (≈1221) | slice 6, part A |
| `the_switch_needs_the_executed_plan_tied_to_the_one_shown` (≈1233), and `crates/cli/tests/migrate.rs` ≈957 | slice 7, part A (+ DP-049) |
| `the_switch_needs_consent_wired` (≈1161) | slice 7, part B |
| `the_switch_needs_release_builds_free_of_the_test_support_feature` (≈1256) | work order 15 (a guarded-script check) |
| `mutations_cannot_dispatch_while_a_live_run_still_says_not_checked` (≈482) | slice 9 (DP-030) |

Plus `this_build_does_not_dispatch_mutations`, which the flip edits on purpose.

**The gap no tripwire names well: the destination contract.** The planner puts
six checks on `VERIFY_DESTINATION` (`planner.rs` ≈861–875):
`PROVIDER_REPORTS_ACTIVE`, `PACKAGE_NEWLY_PRESENT`, `PROVIDER_INTEGRITY_CHECK`,
`EXECUTABLES_OWNED_BY_DESTINATION`, `LIVENESS_PROBE` and
`NO_CROSS_PROVIDER_LINKAGE`. The gate can evaluate only `PACKAGE_NEWLY_PRESENT`
(and `PACKAGE_ABSENT` for a removal) (`execution_gate.rs` ≈509–521), and the
executor halts on any other (`executor.rs` ≈605–611); `plan.v1.md` obligation 6
(≈275–290) and SE-094 already say so. Slice 6 makes the first evaluable. The
other four are **DP-051**: `LIVENESS_PROBE` runs a program outside
`PERMITTED_PROGRAMS`, and an integrity, ownership or linkage read probably grows
the allowlist — both the owner's. **No slice may make a run complete by having
the planner stop emitting a check**: that is an undeclared deviation from 04's
MUST (≈47–58) and, once removal is on, an INV-01 violation.

So today, every planner-built plan halts at `VERIFY_DESTINATION` first, then at
`SET_PRIMARY`/`VERIFY_RESOLUTION` (slice 8, DP-047). A slice that closes a
tripwire says in its report what still halts a real run.

Order: 6 → 7 → 8 → 9. Slice 6 first because its check is reused by its own
part C; slice 7 is independent of 6 in code but touches the same executor files,
so never run them in parallel; slice 8 changes what a run can complete, so it
goes after the guards exist.

---

## Slice 6 — the checks that guard a removal

### Common facts

- Plan order when nothing blocks (`crates/application/src/planner.rs` ≈781–978):
  `SNAPSHOT_BEFORE, ADD_DESTINATION, VERIFY_DESTINATION, SET_PRIMARY,
  VERIFY_RESOLUTION, REMOVE_SOURCE, VERIFY_REMOVAL, CLEANUP_REVIEW,
  RECORD_HISTORY`. Any blocking reason makes the plan `MANUAL_REVIEW` steps only
  (≈979–983). **This ordering is INV-01 and must not change.**
- Next free identifiers at writing: SE-101, R-20.

### Part A — `MANUAL_REVIEW` for an unavailable artifact

- `ArtifactAvailability` (`crates/domain/src/availability.rs` ≈13–32) has seven
  values. The planner reads `dest_row.availability` only for a non-blocking
  `destination_source_build` risk (`planner.rs` ≈693–701); `Disabled` and
  `Deprecated` already block through metadata flags (≈677–692). No adapter
  emits `Unavailable` or `UnsupportedPlatform` today.
- **Design.** Inside `if let Some(dest_row)` (≈676), after the
  deprecated/disabled block, an `else if matches!(…, Unavailable |
  UnsupportedPlatform | Disabled)` that pushes a blocking reason
  `destination_artifact_unavailable` (`reason.artifact_unavailable`, with
  provider, package, state). The `else` keeps a disabled formula to one review.
  Add the id to `REASON_IDS` (≈24–44) and both locales. Push it **after** the
  existing ones so every existing plan's `MANUAL_REVIEW` order is unchanged.
- Rejected: blocking on `!is_prebuilt()` (every source build and every
  `Unknown`, i.e. every restore, would stop); restore-only logic (04 says a
  restore is not a special case). 04's own example — "an Intel Mac can no
  longer bottle", a MUST (≈170–175) — is DP-050: under its default, add a
  **declared deviation** note in 04 pointing at DP-050 and the slice's SE entry
  (as 04 ≈164–167 does for slice 5); do not rewrite the example before the owner
  rules.
- Tests: `unavailable_or_unsupported_destination_is_manual_review` (both
  states); `source_build_and_unknown_are_not_manual_review`;
  `a_disabled_destination_raises_one_manual_review`;
  `existing_blocking_order_is_unchanged`; a gate test (refused, exit 4). Then
  turn the tripwire into a positive check held whatever the switch is.
- Expected effect on fixtures: none (no adapter emits those states).

### Part B — `PROVIDER_REPORTS_ACTIVE` evaluable on `VERIFY_DESTINATION`

- Today `evaluable_in_this_build` (`execution_gate.rs` ≈509–521) and `answers`
  (`executor.rs` ≈858–872) return false for it. The current verify reads cannot
  show "active": Homebrew's is `list --versions <name>` (exit status), MacPorts'
  is `-q installed <name>` (`data/policy/provider-commands.toml` ≈25, 49, 202,
  230). The adapters' own `active` flag is partly **inferred** (Homebrew's
  `active_keg` falls back to the newest keg when `linked_keg` is null,
  `homebrew/json.rs` ≈421–426) — rebuilding the check from capture rows brings
  back the false certification SE-094 removed.
- Two reads already permitted carry "active", so **the process allowlist does
  not grow**: `brew info --json=v2 --installed` (`ARGV_INFO`,
  `homebrew/mod.rs` ≈75, 89; permitted at `process/mod.rs` ≈133–138) and bare
  `port -q installed` (`ARGV_INSTALLED`, `macports/mod.rs` ≈104; permitted
  ≈184–189).
- **The security record on that Homebrew read, which the plan must weigh.**
  SE-016 item 3 (≈583–592) records that `brew info --json=v2 --installed` can
  load untrusted tap Ruby through a `brew.env`; R-15 (E7) refutes SE-050 item
  6's premise for verification reads (the fetch writes about 30 MB to the
  user's cache). Today's verify read is `list --versions <name>`; this design
  moves the heavier read into the run loop, and Part C runs it again right
  before a removal. The slice plan must cite SE-016(3), SE-050(6) and R-15, and
  `security-reviewer` rules on them before code. If the ruling is against it,
  the alternatives are an owner decision (a narrower read grows the allowlist).
- **Prerequisite: R-20** (`docs/research/R-20-what-a-read-shows-as-active.md`,
  written 2026-10-18 by work order 14; read it, it is the full answer). What
  corrects the first version of this design — observed: MacPorts prints an
  active row as `␣␣<name>␣@<version>_<revision><variants>␣(active)` with no
  trailing whitespace, and `-v` (never in our argv) would add text after
  `(active)`; read from the 2.12.6 source, not observed: an **inactive row has
  nothing after the variants** — there is no `(inactive)` output marker.
  Homebrew (observed): `linked_keg` is the version
  `var/homebrew/linked/<name>` points to, or `null`; when a string, it equals
  one `installed[].version` including the revision suffix; **`keg_only` does not
  predict it** (`openssl@3` is keg-only and linked, `curl` keg-only and
  unlinked, `openssl@4` and `pydantic` not keg-only and unlinked); casks carry no
  such field. Not observed: the state right after a fresh install, and a real
  inactive MacPorts row (the only one in the fixtures is synthesised, so a
  replay test of the inactive case tests invented data — say so, or capture
  one).
- **Design.** A third read verb `verify_active`, like `verify_absent` naming no
  package, on both providers (INV-17). Two new `SuccessSemantic` values
  (`crates/domain/src/plan.rs` ≈110) evaluated in `verification::evaluate`
  (`verification.rs` ≈336), both failing closed on did-not-finish, truncation
  and non-zero exit:
  - `StdoutMarksPackageActive`: a line whose first token is the name and whose
    last token is exactly `(active)`;
  - `JsonReportsPackageLinked`: the `formulae[]` entry whose `name` equals the
    destination has a string `linked_keg` equal to one of its
    `installed[].version`.
  - Anything else is a new `FailureCause::ActiveNotReported` (`linked_keg`
    null, a cask, a missing entry, unparseable). **An unlinked formula or a cask
    stays unverifiable and halts** — fail closed. Whether a keg-only formula
    that *is* linked (`openssl@3`) passes is the slice's decision: the predicate
    above certifies it; adding `keg_only == false` would refuse it. Decide,
    record it in the slice's SE entry, and note that 04's "installed and active"
    has no Homebrew meaning yet — writing "linked" into 04 is a Tier 3 change
    with its SE line.
  - The planner adds the command after `verify` on `VERIFY_DESTINATION`
    (≈833–840); `names_no_package` (≈1028), `READ_OPERATIONS` (≈1022), the
    read-verb check (`provider_commands.rs` ≈247), `planned_success`
    (≈1053–1058), elevation cells `verify_active[_gui_app]` = none, with
    provenance.
  - The gate pin (`execution_gate.rs` ≈666–703) compares commands **by
    position** (0 = `verify`, 1 = `verify_active`) and refuses a missing,
    extra or reordered command.
  - `evaluable_in_this_build`: true for `VerifyDestination` only. `answers`:
    only the two new semantics answer it; an "installed" read never does.
- Rejected: a fresh capture's rows (inferred flag); `brew info --json=v2 <name>`
  (a name-carrying `info` grows the allowlist — an owner decision for no gain);
  MacPorts only (INV-17).
- Tests: `active_marker_must_be_on_the_named_line`, `active_read_fails_closed`,
  `linked_keg_equal_to_an_installed_keg_is_active` (and `gh` is not answered by
  `ghidra`), `unlinked_kegonly_cask_or_malformed_is_not_active`,
  `provider_reports_active_is_evaluable_on_verify_destination_only`,
  `an_installed_read_never_answers_provider_reports_active`,
  `active_read_swapped_or_missing_is_not_the_planners_read`, a planner test of
  the two commands' order, and replay assertions over the recorded outputs in
  `fixtures/recordings/*.json`.
- **Widen the tripwire, do not let it go green.** Today it asks only about
  `PROVIDER_REPORTS_ACTIVE`, while SE-092 (≈7180) records its purpose as "while
  04's destination contract is not evaluable". Rewrite it to build the planner's
  `VERIFY_DESTINATION` and require every check on it to be evaluable. After this
  part it still fails with the switch on, now naming the four checks of DP-051.
- Reviewer question: does a new success semantic need a `PLAN_SCHEMA_VERSION`
  bump? Older readers deserialise it as `UNKNOWN`, which fails closed — decide
  and record.

### Part C — re-verify the destination right before removal

- Today `REMOVE_SOURCE` checks only that the earlier `VERIFY_DESTINATION`
  *record* says `Verified` (`executor.rs` ≈661–664, 698–711). Before spawning,
  the shell checks the lock (`crates/cli/src/executor/mod.rs` ≈583), re-reads
  the configuration (≈588–601) and dispatches (≈604). Nothing re-reads the
  destination (04 ≈314–319; slice 4 review M6a).
- **Design.** A pure `destination_still_verified(run, remove_position, reads)
  -> Result<(), Precondition>` in `application/src/executor.rs`: Ok only if
  `judge_step` on the `VERIFY_DESTINATION` position is `Verified`; new
  `Precondition::DestinationNotReverified`. In the shell, for `RemoveSource`
  only, after the configuration re-check and immediately before
  `mutation_dispatch`, run `read_commands` with the **verify step's own
  gate-pinned commands** (no new argv, nothing derived at execution time —
  INV-02) and refuse on Err. It inherits part B's check automatically.
- Rejected: a second `VERIFY_DESTINATION` step (`check_ordering` refuses it,
  `plan.rs` ≈616–632, and it moves INV-01's enforcement point); accepting the
  residual without a check (safe only while removal is unreachable, and that
  guarantee disappears silently once slice 8 lands).
- Tests: `removal_refused_when_destination_vanished_after_verify`,
  `removal_refused_when_reread_truncated_or_failed`,
  `reverification_uses_the_verify_steps_own_commands`, and an
  `executor_wiring.rs` order test through the `Boundary` observer
  (`ConfigurationRechecked` → re-verify → `Dispatched`). Replace the tripwire
  with a structural search for `destination_still_verified(` in the shell, like
  `the_switch_needs_consent_wired`.
- The residual after the re-read is DP-048.
- Name the tests that prove INV-01 `invariant_01_*`.

### Part D — removal refused by release policy (DP-048's default)

DP-048's reversible default — `--retain-source` forced and `REMOVE_SOURCE`
refused at the gate in the first mutating release — is built by no slice
otherwise; today it holds only by coincidence (removal elevates: MacPorts'
`remove = "required"` in `provider-commands.toml`, and the gate refuses
elevation). If DP-046 were ruled (b), removal would become reachable without it.
**Design:** a build-level constant beside the switch, for example
`THIS_BUILD_REMOVES_SOURCES = false`, read by the gate, which refuses any plan
holding a `REMOVE_SOURCE` step with a named reason (`SOURCE_REMOVAL_NOT_IN_THIS_BUILD`,
exit 4), and makes `migrate` without `--retain-source` refuse at the same point.
Tests: `invariant_01_no_plan_with_a_removal_passes_the_gate_while_removal_is_off`,
a pin test of the constant, and its own flip tripwire that names DP-048.

---

## Slice 7 — what the user agreed to is what runs

### Current state of the plan tie and consent

- Obligation 8 (`docs/schemas/plan.v1.md` ≈297–306): "the same in-memory object
  shown and then confirmed, or under `--yes` a digest of a previously rendered
  plan that must match". SE-092 settles that `migrate` re-plans in process and
  **never deserialises** a `MigrationPlan`.
- In Execute mode `migrate` runs before anything is rendered
  (`crates/cli/src/app/migrate.rs` ≈483–527; its module comment ≈36–38 admits
  it). `run_with_mutations` (`crates/cli/src/executor/mod.rs` ≈471–714) does
  probe, lock, recapture and `plan_still_holds`, marker read, loop; nothing in it
  takes a shown plan.
- `cli::consent::ask` (`crates/cli/src/consent.rs` ≈65–139) and
  `application::consent` (`elevation_requests` ≈130–181, `confirm` ≈111–121) have
  no product caller. Slice 4 D10: ask "after D6, under the lock".
- Elevation is refused at the gate (`execution_gate.rs` ≈727–734, ≈797) and
  halted by the guard (`executor.rs` ≈613–619, 667–676), so no cleared run has
  elevation requests today (DP-046).
- **SR 9 is the configuration token, not the consent token.**
  `ConfigurationRechecked` holds only the provider
  (`application/src/configuration_recheck.rs` ≈67–69); `mutation_dispatch`
  checks only the provider (`cli/src/executor/mod.rs` ≈743).
- **SR 10**: `ask` reads buffered stdin (`consent.rs` ≈123), so a line typed
  before the prompt answers it; the phrase is a fixed `allow step N`
  (`application/src/consent.rs` ≈102–104). `tcflush` is impossible with
  `unsafe_code = "forbid"` and no `libc`.
- No `ErrorId` for exit 6 ("user aborted", `06-cli-and-ux.md`)
  (`crates/domain/src/error.rs` ≈16–45, 128–145).

### Part A — the plan digest (blocked on DP-049 for the dependency)

- New `application/src/plan_tie.rs`: `PlanDigest` = SHA-256 of the canonical
  `serde_json` serialisation of the in-memory `MigrationPlan`, written
  `sha256:<64 hex>` — never of the redacted document. `ShownPlan` is a witness
  with a private field, neither `Clone` nor `Default` (reuse the
  `not_clone_nor_default` check, `execution_gate.rs` ≈420–426), minted only by
  `tie(run: &PlannedRun, echoed: &PlanDigest) -> Result<ShownPlan, PlanNotShown>`;
  `ShownPlan::covers(&MigrationPlan)`.
- `migrate` in Render mode prints the digest and adds a root `planDigest`
  (additive). `migrate --yes` gains `--plan-digest <d>`: missing or malformed is
  a usage error (exit 2) once an allowance exists; a mismatch is
  `SAFETY_BLOCKED`, "the plan changed since you saw it" (exit 4).
- `MutatingRun` gains `shown: ShownPlan`; `run_with_mutations` checks
  `io.shown.covers(plan)` **before** `history_store::probe`, so a refused tie
  takes no lock. A future `restore --run` inherits it.
- Determinism: confirm every map in `MigrationPlan` is ordered and the plan
  carries no timestamp. For `--retain-source`, `migrate` consults history
  (≈380–401), so print the digest from `migrate`, never from `plan`.
- Tests: `plan_digest_is_stable_and_covers_every_field`,
  `a_shown_plan_cannot_be_minted_from_another_digest`,
  `invariant_02_a_digest_of_another_plan_dispatches_nothing_and_takes_no_lock`,
  `the_digest_migrate_shows_is_the_one_yes_checks`,
  `yes_requires_a_well_formed_plan_digest_when_mutations_dispatch`.

### Part B — consent wired

- Seam: `MutatingRun.consent: &dyn Fn(&[ElevationRequest]) -> ConsentDecision`;
  the product closure in `migrate.rs` calls `consent::ask` with real
  stdin/stderr and `std::io::IsTerminal`.
- In `run_with_mutations`, after `Boundary::Replanned` and the marker read and
  before the loop: `elevation_requests(plan, planned.before())`; `PlanDisagrees`
  halts with nothing dispatched; ask only when there are requests; `Declined`
  halts with a new `ErrorId::UserAborted` (exit 6); `NotATerminal` halts
  `SAFETY_BLOCKED` (4). No dispatch, no marker, no record on any refusal. New
  `Boundary::ConsentAsked`.
- Token plumbing only: `next_action_with` may gain `consents: &[ConsentGiven]`
  and `mutation_dispatch` may hand a covering token to `MutationSite::perform(…,
  consent: Option<ConsentGiven>)`, but **the guard keeps halting every elevated
  command with `ElevationNotNone`** in this slice. Today two independent layers
  refuse elevated dispatch, the gate and this guard; relaxing the guard now would
  leave one, for no gain, since no elevated spawn mechanism exists (DP-046).
  Slice 5's table placed the relaxation with "the pass that executes elevated
  steps"; it stays there. Pin the halt with
  `invariant_06_the_guard_halts_an_elevated_command_whatever_the_tokens`.
- **Do not lift the gate's elevation refusals in this slice** (DP-046).
  `every_plan_this_planner_builds_is_refused_for_execution_in_this_build` must
  not be weakened. Declared gap: the shell-level prompt is unreachable in a
  shipped build until DP-046 (b).
- Reviewer question: re-run D6 after a slow consent answer? Decide and record.
- Tests: `invariant_06_the_guard_halts_an_elevated_command_whatever_the_tokens`,
  `invariant_06_a_declined_or_unseen_question_halts_before_the_marker`, and a
  test that a token is carried to the site (the plumbing the later pass relies
  on). `invariant_06_only_a_covering_token_lets_an_elevated_command_through`
  belongs to the pass that relaxes the guard.

### Part C — SR 10, a code in the phrase

- `allow step N <code>`, the code 6 lowercase hex characters read from
  `/dev/urandom` through `std::fs`, one per request, through a nonce seam in
  `ConsentStreams`; no code → refuse without reading. `confirm(request, code,
  typed)`. New `consent.*` keys in en and pt-BR.
- Rejected: `tcflush` through a new dependency (still races a paste after the
  flush).
- Tests: `a_line_typed_before_the_question_is_never_consent`,
  `each_question_has_its_own_code`, `without_a_code_nothing_is_asked`.

### Part D — SR 9, the configuration token carries its program

- `ConfigurationRechecked` gains `program`;
  `recheck_provider_configuration(provider, program, snapshot, fresh)`;
  `mutation_dispatch` refuses `Precondition::WrongProgramToken` on mismatch; the
  `MutationSite` doc requires the site to compare its own program.
- Tests: `a_recheck_records_the_program_it_was_made_for`,
  `a_token_for_another_program_is_refused_and_nothing_is_handed_over`.

### Tripwires, rewritten as positive structural checks

`io.shown.covers(plan)` before `history_store::probe(`; `(io.consent)(` after the
recapture and before `run_marker::write(` (add to `wiring_violations`' order);
`migrate.rs` ≈957 rewritten to require `shown:` in `MutatingRun`; `ShownPlan`
constructed only in its own file.

Order of commits: D, C, then A (after DP-049), then B, then tripwires and docs —
the low-risk token changes first, so the reviewers see the consent wiring in the executor
alone.

---

## Slice 8 — `SET_PRIMARY` and `VERIFY_RESOLUTION`, so a run can complete

### Current state of the primary steps

- Spec 05 (≈33–121): layer 1 report only; layer 2 versioned names; layer 3
  `port select` proposed, never run; layer 4 the shim directory
  `~/.local/share/macportsbrew/bin` plus one managed `PATH` prepend, with
  detect/locate/diff/preview/confirm/back up/write/re-read and an
  `env --revert` that does not exist. Reordering `/usr/local/bin` against
  `/opt/local/bin` is global priority: under INV-12 a separate, explicitly confirmed operation, never a package primary.
- `SET_PRIMARY` (`planner.rs` ≈883–912) carries `macportsbrew primary <name>
  <to>` — no such subcommand (`cli/src/app/mod.rs` ≈100–183) — and
  `PROVIDER_REPORTS_ACTIVE`. `VERIFY_RESOLUTION` (≈913–919) carries nothing.
  `primary_after` is the requested provider, not an observed one (≈1001).
- The gate refuses both (`execution_gate.rs` ≈152–174); the executor halts
  (`executor.rs` ≈594–597, 643–647, 760–765); `outcome_of` needs every step
  (≈1043–1054). Tests reach `COMPLETED` only by deleting these steps
  (`executor/tests.rs` ≈503–525, 1196–1219).
- Capture canonicalises `which` hits and attributes the owner from the target
  (`capture.rs` ≈73–107), so a shim would be classified `PROVIDER`;
  `MacportsbrewShim` (`domain/src/resolution.rs` ≈25) exists only in tests, and
  `PRIMARY_IS_SHIM` is unreachable from a real capture — spec 05 ≈20–26 is
  unmet.
- Constraint for tests: shell dotfiles are denied to the agent's Write/Edit
  anywhere (settings and `guard-protected-paths.sh`), including `fixtures/` and
  the scratchpad. Rust tests that create one at runtime in a temporary
  directory are not affected.

### Design (DP-047 option (b), the reversible default)

- Planner: decide from the **captured resolution** of the name, not from
  comparing two `bin` indices — in the fixture, `/usr/bin` (index 10) and user
  directories (0–5) precede `/opt/local/bin` (25), so an index rule mispredicts
  any name a system or user entry shadows. Each provider's `bin` comes from the
  snapshot (`ProviderSnapshot.environment.bin`), never a literal (INV-20,
  layering); relative `PATH` entries are skipped, as
  `adapters/src/system/machine.rs` ≈541–543 does. Destination already resolves →
  `SET_PRIMARY` with **no command** and a new carried disposition
  `NothingToChange`; a non-provider entry wins, or the resolution is unknown →
  refused with a named reason. `SET_PRIMARY` also stops carrying
  `PROVIDER_REPORTS_ACTIVE` (`planner.rs` ≈910), which is not evaluable there;
  say so in the SE entry. A shim would be needed → the gate refuses
  before any install, `PRIMARY_NEEDS_SHIM` (exit 4), naming layers 1 and 2 and
  saying that reordering `PATH` is global priority (INV-12), a separate confirmed operation this release does not offer.
- `VERIFY_RESOLUTION` gains `VerificationCheck::ResolvesTo {executable,
  provider}`, evaluated **in process** from a fresh `which` plus a resolution read
  (no spawn; `PERMITTED_PROGRAMS` unchanged), passing only on an authoritative
  `PROVIDER{destination}` owner, and unevaluable — never a pass — under
  `FilesystemScope::Elsewhere` (INV-15). A `ResolutionSite` seam beside
  `ReadSite` in the shell.
- History: `primaryAfter` records the provider observed after the run (decide
  with `spec-guardian` whether the meaning change is `history.v3`). Re-run
  recognition becomes honest for plans that use no shim.
- A package whose executable is not its own name (`ripgrep`/`rg`): refused for
  now; provider metadata later. Record in SE.
- Shim recognition waits for layer 4 (no shim exists to capture).

### Tests

On `fixtures/snapshots/both-providers-intel-macos26-live.json` (Homebrew's
`bin` at `PATH` index 6, MacPorts' at 25; eight names installed in both —
autoconf, gettext, htop, libtool, m4, ncurses, tree, xz — of which five resolve to
Homebrew, `libtool` and `m4` to `/usr/bin` (owner unknown), and `ncurses` has no
resolution): `a_plan_whose_destination_already_wins_sets_no_primary` (htop
MacPorts→Homebrew); `a_plan_needing_a_shim_is_refused_before_any_install` (htop
Homebrew→MacPorts); `a_name_a_system_entry_shadows_is_refused` (`m4`, which
resolves to `/usr/bin`); `verify_resolution_passes_only_on_an_authoritative_destination_owner`;
`verify_resolution_halts_when_the_source_still_wins`;
`verify_resolution_under_replay_is_unevaluable`;
`nothing_to_change_is_carried_only_on_set_primary`;
`a_real_retain_source_plan_completes_once_its_checks_are_evaluable` (no step removed; it can pass only after DP-051 — until then it is written and marked as expected to halt at `VERIFY_DESTINATION`);
`primary_after_is_what_resolved`; `a_rerun_after_a_no_shim_primary_is_recognised`;
`no_run_writes_outside_the_state_directory_or_the_providers`. Then cut the
test narrowing back to the unevaluable checks only (stop deleting `SET_PRIMARY` and `VERIFY_RESOLUTION`); never make the planner stop emitting a check. **A permuted `PATH` is an invented fixture**: capture an arm64
machine instead (owner).

### Risks

The process's `PATH` and shell `hash` can differ from the user's other shells —
the report must name the `PATH` it read. A wrong prediction is caught by
`VERIFY_RESOLUTION` and ends `PARTIAL` with the source kept. A user refused on
Homebrew→MacPorts may ask for a `PATH` reorder — global priority, which INV-12 makes a separate, explicitly confirmed operation and this release does not offer; the refusal
text must say so.
