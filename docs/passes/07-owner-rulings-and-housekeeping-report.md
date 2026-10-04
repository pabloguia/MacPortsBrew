<!-- SPDX-License-Identifier: Apache-2.0 -->

# 07-OWNER-RULINGS-AND-HOUSEKEEPING — pass report

**Date:** 2026-10-03 to 2026-10-04 · **Work order:**
[`../../prompts/07-owner-rulings-and-housekeeping.md`](../../prompts/07-owner-rulings-and-housekeeping.md) ·
**Branch:** `pass/02-architecture-review` · **Commits:** `75d0fd9`..`9f64c90`
and the report's own, on base `5c40b11`

**Preflight, 2026-10-04.** `.claude/hooks/stop-gate.sh` exits 0;
`scripts/validate.sh --full` passes (every stage, the fixture matrix's 198
checks over 7 fixtures, and the release build for both architectures recording
the 10.12 / 11.0 floors); the commit gate ran the default mode before every
code commit; `scripts/check-coverage.sh` meets every floor (below). Section 3
of the work order is demonstrated: `git diff 5c40b11 HEAD -- Cargo.lock
.claude/hooks .claude/settings.json scripts .github` is empty,
`THIS_BUILD_DISPATCHES_MUTATIONS` is `false`, no entry was added to either
spawn-site allowlist, and the gate tests are unweakened. `spec-guardian`,
`security-reviewer` and `docs-auditor` reviewed every slice; their findings are
resolved in the commits that follow them, or listed below as carried.
`MPB_SKIP_STOP_GATE`, `MPB_SKIP_COMMIT_GATE` and `MPB_ALLOW_GUARD_EDIT` were
never used. **One correction to pass 06's report:** it called
`validate.sh --full` "the coverage floors"; `--full` adds the release build, and
the floors are `scripts/check-coverage.sh`, which CI runs.

**Authority.** The owner ruled DP-025, DP-026 and DP-027 on 2026-10-03 by
adopting the recommendations, granted autonomy over the guarded paths, and
asked for maintenance and evolution to continue autonomously. The guarded-path
escape is armed only by `MPB_ALLOW_GUARD_EDIT=1` at the start of a line of the
owner's own message; that message did not carry it, so **no hook, setting,
script or workflow was edited** — the agent did not forge the marker, which
would have turned the escape's audit trail into the agent's word. What that
leaves is listed under "Owed in guarded paths".

## What shipped

- **`migrate` answers *nothing to move* on a one-provider machine** (DP-026
  (b)): exit 0, naming every other provider that was not detected, in the text
  and in `migrate.v1`'s `nothingToMove.notDetected`. The INV-16 re-run after a
  completed migration whose source provider was then uninstalled now says
  "nothing to do" rather than "blocked".
- **A provider's own configuration is detected and reported, and a mutation
  under it is refused** (DP-027 (b)/(c)). Capture records which Homebrew
  `brew.env` files and which MacPorts user `macports.conf` exist and every key
  the provider's reader would accept from them — names only, never values;
  `doctor` reports it per provider; a plan carries a non-blocking
  `provider_configuration` risk; the execution gate refuses
  (`PROVIDER_CONFIGURATION`). A replay records `UNKNOWABLE`, which is itself a
  risk the gate refuses.
- **`explain` and `doctor` carry the concurrency flag** in `--json` and in
  words, like `plan`, `analyze` and `migrate`.
- **Fixes:** a cask's plan reads "classed as Application", not "GuiApp"; a
  withdrawal sentence reads "disabled", not `DISABLED`; the top-level error
  path redacts the home; `error.provider_not_found` points at `doctor`.
- **DP-025 (a):** the charter's example clause reads "plans migrations and
  executes none of them".

Nothing a user runs changes the machine; `THIS_BUILD_DISPATCHES_MUTATIONS` is
`false`.

## Research findings

- **R-16 answered** (`../research/R-16-macports-user-configuration.md`,
  `provider-researcher`): a user `macports.conf` overlays the system file key by
  key under the real `HOME`, cannot undo `-q`/`-N`, can move what a read reports
  (`portdbpath`, `sources_conf`, `prefix`) and steers a mutating install broadly;
  no read was found to go online or evaluate a Portfile; `user.conf` is opened
  and accepts no key in 2.12.6. Unresolved: which file an elevated `port` reads
  (it may take the home from `SUDO_USER`), and whether reads stay offline under
  a hostile file (needs a traffic capture).
- **Tooling recheck**, all three groups
  (`../research/tooling/2026-10-03-*.md`). No contract the configuration relies
  on disappeared. Found: four defects in guarded paths (below), the
  start-directory and multi-repository-cloud conditions under which no hook
  runs (now stated in `11-agentic-sdlc.md`'s enforcement-layer section), and that `allowed-tools` grants apply in untrusted `-p` runs (to be
  accepted with the public-repository decision).
- **R-15's residuals** (cold cache with no network; altered catalogue data) are
  carried.

## Decisions

- **Owner's, ruled 2026-10-03:** DP-025 (a), DP-026 (b), DP-027 (b) for
  mutation and (c) for reads. Recorded in `../decisions-pending.md` and SE-092 /
  SE-016.
- **Raised for the owner: DP-028**, the two questions DP-027 put above the
  specification (whether INV-10 covers what a provider reads; whether a
  provider's own network fetch and analytics are consistent with the charter's
  non-goals). The first recording of DP-027's ruling read them as answered;
  `spec-guardian` showed that overstepped, and they are open with a
  recommendation ((c), say precisely what holds) and the reversible default
  (a).
- **Agent's, recorded in SE-016 and DP-027:** the detection counts *every* key
  a provider's reader accepts, not a reviewed list — wider than the
  recommendation's wording. Both reviewers showed a list let `SUDO_ASKPASS` and
  `HOMEBREW_XDG_CONFIG_HOME` through as "nothing found". Its cost: any
  `brew.env` setting (a GitHub token) refuses a mutation against Homebrew. The
  owner can narrow it by naming keys that outrank nothing.
- **Agent's:** an unrecorded configuration is a plan risk the gate refuses,
  because `migrate --replay --yes` reaches the gate (`spec-guardian`).
- **Agent's:** the guarded-path escape was not self-armed (see Authority).
- No ADR was written or changed.

## Specification changes

All in `../spec-evolution.md`:

- **SE-092** — DP-026's rule (an undetected provider is named, not refused);
  `PROVIDER_PRESENCE_UNPROVEN` reserved and no longer raised; the charter
  clause; the consolidation `spec-guardian` asked for.
- **SE-016** — DP-027 implemented: the capability, the widening, the unknown
  risk, the tests, and what is not closed. `04-migration-and-safety.md`
  (Process execution) gains the MUST; `06-cli-and-ux.md` points the open
  question at DP-028.
- **SE-015** — `explain` and `doctor` carry the flag.
- **SE-064** — the `{class}` and `{state}` siblings closed; the schemas README
  drops its two exceptions.
- **SE-054** — the top-level error path redacts the home.
- **Rejected:** a reviewed-key denylist for DP-027 (SE-016, reasons above);
  self-arming the guarded-path escape (this report).

## What it does not do

- Dispatch any mutating step.
- Read a provider's configuration again immediately before a step (the
  check-then-use window SE-016 leaves to the mutating pass).
- Model `HOMEBREW_SYSTEM_ENV_TAKES_PRIORITY`'s ordering — it changes which
  value wins, not which keys are set, and every key already refuses.
- Read the MacPorts system `macports.conf` (root-owned, the baseline) or a file
  `PORTSRC` names (never passed).
- Give `ConcurrentProviderActivity` a fallback for an unknown value — still the
  detector's to add (SE-015).
- Keep a Homebrew read offline on an empty cache (R-15 residual).

## Tests and coverage

Coverage, `scripts/check-coverage.sh`, 2026-10-04 (lines): domain 97.25%,
policy 97.60%, application **95.25%** (floor 95 — the narrowest margin; the
next feature in this crate should bring its tests with it), adapters 95.79%,
cli 93.10% (reported, no floor), overall 95.68% (floor 90). The whole suite
passes under `validate.sh`.

Invariants this pass touched, by test:

- **INV-16** — `invariant_16_migrate_reports_nothing_to_move_naming_a_provider_that_was_not_detected`,
  `invariant_16_migrate_reports_nothing_to_move_where_another_provider_was_not_detected`
  (binary, both renderings), `invariant_16_migrating_a_package_already_at_its_destination_changes_nothing_and_says_so`.
- **INV-17** — `invariant_17_an_undetected_provider_is_named_in_the_mirror_direction_too`,
  `invariant_17_the_destination_side_raises_it_too`,
  `invariant_17_the_mirror_direction_raises_it_on_both_sides`,
  `invariant_17_the_source_side_is_refused_the_same_way`.
- **INV-15** — `invariant_15_a_reader_of_another_machine_answers_nothing_from_this_disk`
  now covers both configuration readers; the flag tests cover `explain` and
  `doctor` under replay and from a snapshot file.
- **INV-02 / §3** — `every_plan_this_planner_builds_is_refused_for_execution_in_this_build`
  and its fixture counterpart unmodified and green.

Every new rule was proved by mutation (restored by copy, `cmp`, and `touch`):
the `ABSENT` arm; the explain/doctor wrapper; the class and state spellings;
the error redaction; the gate's push, the planner's push (found and unknown),
the loader's key check, the replay early return, and `+=`.

## CI

Not run: the repository has no remote (DP-007; `git push` is denied). Local
gate on one Intel Mac (macOS 26). The recheck found the CI workflow would run
the commit gate's `cargo test` with `ANTHROPIC_API_KEY` in its environment
(D4 below); it is dormant until the remote exists, and must be fixed first.

## Known limitations and risks discovered

- **Owed in guarded paths** (the owner arms the escape, or applies them):
  - **D4** `.github/workflows/claude.yml`: set `MPB_SKIP_COMMIT_GATE: "1"`
    beside `MPB_SKIP_STOP_GATE`, so an `@claude` commit does not build and test
    a PR head with the API key in the environment. *Before the remote exists.*
  - **D2** `.claude/hooks/commit-gate.sh`: a failed `cd` exits 1 and a missing
    `validate.sh` exits 0 — both let a commit through; deny instead. **D2b**: a
    `validate.sh` that outlives the 1500 s hook timeout fails open.
  - **D3** `.claude/hooks/arm-guard-edit.sh`: exit 2 on `UserPromptSubmit`
    rejects the owner's whole prompt; exit 0 with context instead.
  - **D1** `.claude/settings.json`: `Write(./rust-toolchain.toml)` and
    `Write(./docs/spec/01-invariants.md)` are never consulted; the `Edit(...)`
    rules beside them already cover both.
  - `migrate` in `scripts/run-fixture-matrix.sh`.
  - Recommendations from the recheck (not defects): `SessionEnd` to delete the
    guard marker; a `ConfigChange` hook; `background_tasks` in the `Stop` gate;
    the cross-session envelope in the arm filter.
- **A `brew.env` with any setting refuses a mutation against Homebrew** — the
  widening above; the owner may narrow it.
- **Carried from `docs-auditor`, minor:** `doctor.provider_absent` says "is
  not installed" where DP-026 distinguishes "not detected"; the new sentences
  render `homebrew's` in some places; the MacPorts-to-Homebrew wiki page has no
  sentence on the configuration risk.
- Assumptions to attack: that every key a provider's reader accepts is all that
  matters (a provider could read a file this data does not name — R-16 found
  `PORTSRC`, which this tool never passes); that a plan risk is the right
  carrier for a gate input (SE-092 keeps the gate reading only the plan).

## Recommended next tasks

1. **Arm the escape for the guarded edits above** (`MPB_ALLOW_GUARD_EDIT=1` at
   the start of a line in a message), D4 first. *Owner, then maintenance.*
2. **Rule on DP-028.** *Owner.* It blocks nothing in this build.
3. **Create the GitHub remote and run CI** (DP-007 is decided), after D4.
   *Owner.*
4. **The mutating half of the executor** — the history store, lock and
   detector, the configuration re-read before each step, the `--retain-source`
   re-run, a MacPorts-to-Homebrew recording that yields a plan, and the
   positional half of the spawn-site pin. *Focused implementation, its own
   reviewed pass.* DP-026 and DP-027 no longer block it.
5. **R-15 residuals and R-16's elevated-home question.** *Research.*
