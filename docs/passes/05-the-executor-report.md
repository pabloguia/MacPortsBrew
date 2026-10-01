<!-- SPDX-License-Identifier: Apache-2.0 -->

# 05-THE-EXECUTOR (dry-run half) — pass report

**Date:** 2026-10-01 · **Work order:**
[`../../prompts/05-the-executor.md`](../../prompts/05-the-executor.md) ·
**Branch:** `pass/02-architecture-review` · **Commits:** `0916112`..`7b84fab`,
thirteen

**Preflight, 2026-10-01.** `.claude/hooks/stop-gate.sh` exits 0, and
`scripts/validate.sh` in default mode passed on a clean build. The pass changed
one file under `.claude/` — `README.md`'s verified-version line, in the
tooling recheck (`0916112`) — and no hook and no `settings.json`, so
`test-hooks.sh` was not required; `Cargo.lock` is unchanged, so no dependency
was added and the deployment floor is not in question.

**The definition of done is not fully met, and the unmet item is the owner's.**
`.claude/rules/executor.md` and its `CLAUDE.md` path-scoped entry (§8, item 8)
do not exist: `.claude/` is a guarded path. The rules that file would hold are
written into the shell's module comment, `crates/cli/src/executor/mod.rs`
(SE-092, S5 addendum). Every other item is met or restated as owed in
`plan.v1.md`, below.

## What shipped

**`macportsbrew migrate <pkg> --to <provider> [--retain-source]`.** It builds a
migration plan from a live or replayed capture and renders it — human or
`--json`, the contract in [`../schemas/migrate.v1.md`](../schemas/migrate.v1.md).
With `--yes` (and without `--dry-run`, which wins) it puts the plan to the
execution gate, **which refuses every plan this build produces**: exit 4, the
refusals named in the document, nothing dispatched. **Nothing in this pass
changes the machine.** Execute mode from `--from-snapshot` is `USAGE`, exit 2,
before any output.

`migrate` answers *nothing to move*, exit 0, only when it can be shown: every
other provider detected, present and with a readable inventory, and no other
analysis unit naming the destination package. Otherwise it refuses — an
undetected provider as the new `PROVIDER_PRESENCE_UNPROVEN`, exit 4 (DP-026);
another name that may be the package as `SAFETY_BLOCKED`, naming the holder.

Behind the gate, and reached only from `migrate`, the read-step loop exists:
`crates/cli/src/executor/` as the I/O shell, every decision a pure function in
`crates/application/src/executor.rs` and the gate in
`crates/application/src/execution_gate.rs`. Its dispatch guard answers
`Dispatch` only for `VERIFY_DESTINATION` and `VERIFY_REMOVAL`; a read's program
is re-derived at the spawn site from the detected prefix and a literal (DP-023).
It is tested against recordings and reaches no read site in the binary, because
the gate refuses first.

Slice by slice (each recorded where the last column points):

| Commit | Slice | Recorded in |
|---|---|---|
| `0916112` | tooling recheck, Claude Code 2.1.278 | `docs/research/tooling/2026-09-27-*` |
| `810e0d0` | S0 — placement and plan source | SE-092, DP-023, DP-024 |
| `56bfc8c` | S1 — verification evaluated against the plan, step, command and provider it belongs to | SE-089 (Implemented 2026-09-27) |
| `699d173` | S2 — the execution gate; `dryRunOnly` written from a build constant | SE-009 and SE-020 (Extended 2026-09-27) |
| `9bc5b7c` | S3 — `PlanOutcome::AlreadyAtDestination` | SE-092, S3 addendum |
| `e3c1be9` | S2 follow-up — schema version, unbracketed mutations, malformed shapes | SE-020 (Extended 2026-09-30) |
| `bd517fc` | S4 — `plan_read_spec` and `DetectedPrefix` (DP-023) | SE-050 |
| `2c3e59a` | S5 — the run loop | SE-020, SE-050, SE-092 S5 addendum |
| `d1b5de3` | S6 — `migrate` | SE-092, S6 addendum |
| `a8343b6` | flaky spawning tests, root-caused | SE-057 (diagnosis revised) |
| `4cc19f2` | S7 — the concurrency flag, no detector | SE-015 addendum |
| `8a6ddba` | S8 — `RunRecord`; the store deferred | SE-092, S8 extension |
| `7b84fab` | end-of-pass review fixes | SE-092 amendment, SE-057 |

**The flake was not what it looked like.** The adapters' spawning tests had
been failing intermittently under load, diagnosed earlier as scheduling and
"fixed" by a longer drain timeout. The cause was a pipe: on macOS a child's
pipes are not created close-on-exec atomically, so a sibling test spawning in
that window handed its long-lived child a copy of another test's write end.
The spawning tests now run one at a time; no assertion was weakened and nothing
retries (SE-057). Nothing in the product calls the runner from two threads.

## Research findings

No blocking item of `docs/spec/09-research-checklist.md` was opened or closed
by this pass; R-11, R-13 and R-14 stay as they were, and none blocks the
executor. Facts observed on this machine were recorded where they bind:

- `sysctl kern.proc.all` cannot print the process table (2026-10-01), so a
  concurrency detector needs `ps` on the spawn allowlist, provider lock files,
  or `sysctl(3)` through FFI — each deferred with its reason (SE-015, S7).
- `brew list --versions <names>`, Homebrew 7.0.4, with the quieting variables
  set, printed "Downloading Homebrew API data" (2026-09-30):
  `HOMEBREW_NO_AUTO_UPDATE` does not keep a verification read offline. Accepted
  as a known network side effect the mutating pass inherits (SE-050, "Not
  closed" item 6 and its closure).

**Owed before the mutating pass relies on it:** whether a `brew.env` file under
the user's `HOME` can override the quieting environment the process executor
sets. Unestablished; a `provider-researcher` note is owed (SE-092 amendment,
item (c)).

## Decisions

**The owner's, implemented.** Both ruled 2026-09-27 and moved to Resolved in
`docs/decisions-pending.md`:

- **DP-023** — a plan command becomes runnable by **re-deriving** the program
  inside `process/` from the detected prefix and a literal; the plan supplies
  argv only, still checked by the spawn-site allowlist, and a disagreeing
  `program` is a refusal (`PROGRAM_DISAGREES`).
- **DP-024** — the executor is a **module of the command-line crate**, not a new
  crate, because a new crate means editing two guarded scripts. Built so that
  moving it is a directory move and two manifest lines.

**Filed for the owner, with the reversible default taken:**

- **DP-025** — the charter's example of a surface differing from the identity
  ("has no `migrate` command") is now stale. Tier 2, so untouched; the
  surfaces below it were rewritten.
- **DP-026** — what `migrate` answers on a machine where the other provider was
  not detected. Default: refuse, exit 4. Recommendation: exit 0 with a
  qualified, machine-readable answer. **The default cannot outlive the mutating
  pass** without an INV-16 answer — a re-run after the source provider is
  uninstalled would say "blocked" for something done.

**Made in this pass** (each with its rationale in SE-092 unless noted):
`migrate` re-plans in process and never deserialises a `MigrationPlan`, so a
plan's `environment` is never input and the plan schema stays at v1; a refused
execution is exit 4 whichever refusal it was (SE-009); `NO_CHANGE` and
`NOTHING_TO_RESTORE` added to the history status and restorability lists as a
Tier 3 extension of `04-migration-and-safety.md`; `RunRecord` derives its
outcome itself rather than taking one; a halt whose dispatch is uncertain is
recorded as dispatched. No ADR was written or changed.

## Specification changes

**Made.** All in `docs/spec-evolution.md`:

- **SE-092** (new) — placement, plan source, the INV-16 prerequisite (S3), the
  run loop (S5), `migrate` (S6), the history record (S8), and the end-of-pass
  amendment. Includes the one new `ErrorId`, `PROVIDER_PRESENCE_UNPROVEN`, and
  the revision of the three sentences that said nothing executes
  (`plan.v1.md`, `crates/domain/src/plan.rs`, `locales/en.toml`), which the
  work order required in the same change.
- **SE-009**, **SE-020**, **SE-050**, **SE-089**, **SE-015** — extended for the
  gate's exit code, `dryRunOnly` and the gate's refusals, plan reads, the
  verification evaluation, and the concurrency flag.
- **SE-057** — diagnosis revised: a leaked pipe, not scheduling.
- Contract documents: `migrate.v1.md` (new), `plan.v1.md` (twelve obligations,
  each enforced, discharged as data, or restated as owed), `analysis.v1.md`
  (the concurrency flag listed under "Not in this version"),
  `docs/schemas/README.md`; `04-migration-and-safety.md`, `06-cli-and-ux.md`.

**Deferred**, each with its reason on the record:

- The concurrency lock and detector — land with the mutation they guard; the
  refusal path is unreachable and untestable until then (SE-015, S7).
- The history store, read-back, `snapshotPath` redaction, obligation 12's
  test, the lock and interrupted-run marker keyed on `mutationDispatched`, and
  recognising a `--retain-source` re-run as done (SE-092, S8 "Owed by the
  mutating pass").
- Obligation 8, the tie between the plan shown and the plan executed (SE-092,
  "What this does not settle"), pinned by a tripwire test that fails the day a
  mutating step can be dispatched while `--yes` executes its own plan.
- The per-subcommand long-option allowlist (SE-050, "Not closed" item 1).

**Rejected** — do not reopen without new evidence:

- **A new `crates/executor/`** (the work order's §5) and the executor in
  `application` or `adapters` — DP-024's options A, C and D.
- **Deserialising a plan document** for `migrate` — SE-092.
- **Exit 1 for "mutation not in this build"** — a script would learn it as
  "fine, just not yet" (SE-009, S2).
- **Comparing the plan's environment with the derived one** at the read site —
  buys no safety and refuses honest plans (SE-050, S4).
- **Reusing `SAFETY_UNVERIFIABLE`** for an undetected provider — its
  remediation is false on a live machine (SE-092, S6).
- **`RunRecord` taking the outcome as an argument** — a stale outcome could
  record a partial migration as success (SE-092, S8).
- **Choosing a concurrency detector now** — `ps`, lock files and `sysctl` are
  each set aside with the reason (SE-015, S7).

## What it does not do

- **Change the machine, by any route.** No mutating step is dispatched; the
  gate refuses every plan before a read site is built, and behind it the
  dispatch guard refuses a mutating step by explicit branch.
- **Run a verification read from the binary.** The loop's reads are exercised
  only in tests, against recordings.
- **Detect concurrent provider activity.** Every answer is flagged possibly
  inconsistent: `NOT_CHECKED` live, `UNKNOWABLE` under `--replay` or
  `--from-snapshot`. `plan` and `analyze` do not carry the flag yet.
- **Write or read history.** `RunRecord` is a type; there is no store.
- **Recognise a `--retain-source` re-run as done** — it plans again.
- **Answer *nothing to move* on a one-provider machine** — it refuses (DP-026).
- **Exercise `migrate`'s composition in process in the MacPorts-to-Homebrew
  direction.** No recording yields such a plan (every one refuses on the
  unrecorded service fact); the loop's direction symmetry is proved by
  `invariant_17_the_loop_treats_both_directions_identically`, and the
  composition is not. A gap, stated rather than closed (SE-092, S6).
- **See a copy under a name no mapping connects.** *Nothing to move* says what
  it establishes and no more.

## Tests and coverage

`scripts/check-coverage.sh`, run on this machine on 2026-10-01 on the final
tree (`cargo-llvm-cov`, line coverage; branch coverage needs a nightly
toolchain and is not enforced):

| Layer | Lines | Floor | |
|---|---|---|---|
| `domain` | 97.65% (1663/1703) | 95% | ok |
| `policy` | 97.44% (2398/2461) | 95% | ok |
| `application` | 95.64% (3068/3208) | 95% | ok |
| `adapters` | 95.68% (4985/5210) | 95% | ok |
| `cli` | 89.22% (2367/2653) | none | reported |
| overall | 95.05% (14481/15235) | 90% backstop | ok |

`application` clears its floor by 0.64 points. `cli` has no floor, which is why
SE-092 keeps every executor decision out of it; `crates/cli/tests/executor.rs`
stands in for the shell's sequencing. The denominator still includes the
in-`src/` test modules (F-40, DP-015), so every figure is somewhat flattered.

`cargo test --workspace --locked --all-features`, same tree: **773 pass, none
fail, none ignored** — `domain` 72, `policy` 103, `application` 167, `adapters`
244, `cli` 183 (library, both binaries and integration tests), doc-tests 4.

**Invariant tests** (`grep -rn "fn invariant_" crates/*/tests crates/*/src`):
every one of INV-01 to INV-20 has at least one. Added by this pass:

| INV | Tests added |
|---|---|
| 01 | 7 — the loop never dispatches a mutating step; a removal needs a clean exit, a non-empty and untruncated inventory and a safe source name; a destination is not certified by a substring collision or an unsafe name |
| 02 | 2 — every dispatched invocation is a plan command; the obligation-8 tripwire |
| 03 | 3 names, 4 tests — partial success is its own outcome and exit code; a partial run is never recorded as success |
| 06 | 3 — elevation refuses execution and never rendering; a read is never dispatched elevated; `migrate` never elevates |
| 07, 08 | 1 each — a check reads only its own provider's name; a destination check never runs against the source |
| 10 | 1 — a plan read never adopts the environment it carries |
| 15 | 2 — `migrate` under replay is a function of the recording; a replayed run cannot observe this host's processes |
| 16 | 4 — nothing to move is an outcome, reported only when it can be shown; repeated dry runs are byte-identical |
| 17 | 6 — the executor names no provider; the loop treats both directions identically; four mirror-direction refusals |
| 19 | 2 names, 3 tests — `MANUAL_REVIEW` halts before any dispatch and refuses execution, naming the step |

The work order's §7 asked for INV-03, 10, 16, 17 and 19 tests; each exists.
INV-14 (the clock read in one place) was pointed at, unchanged and passing.
Every new rule was proved by mutation in a scratch copy; the tables are in
SE-092 (S3, S6, S8, amendment), SE-015 (S7) and SE-050. Three S6 mutations
survive and are recorded there, not hidden.

**Not covered by the suite:** a live run's concurrency answer (no test can
drive one, INV-05); `scripts/run-fixture-matrix.sh` does not enumerate
`migrate` — a guarded edit — and `crates/cli/tests/migrate.rs` covers the
fixtures instead.

## CI

`.github/workflows/repository-validation.yml` runs an `ubuntu-latest` job
(hook tests, the `Stop` gate, licences, cargo-deny), a macOS matrix of
`macos-26`, `macos-26-intel`, `macos-15` and `macos-15-intel` running
`scripts/validate.sh --full` and the hook tests, and the coverage job on
`macos-26-intel`.

**CI has not run on this pass.** There is no git remote (DP-007). Everything
above was run on this one Intel macOS 26 machine; native arm64 execution,
macOS 15, and a second run of the coverage floors are what CI would add. No
support claim was added by this pass.

## Known limitations and risks discovered

- **INV-02 holds only because nothing destructive is dispatched.** `migrate
  --yes` renders and executes in one process, so no human saw that object
  first. The mutating pass must tie the two (obligation 8) or take a Tier 1
  question to the owner (SE-092).
- **DP-026's default does not survive the mutating pass** (above).
- **An execution re-detects prefixes** rather than reusing the capture's; a
  prefix that changed between is refused as `PROGRAM_DISAGREES`, never run
  under another binary (SE-092, S6 "Not closed" (3)).
- **The name check reads other units' mappings only** — sufficient while the
  mapping heuristics are symmetric; an asymmetric one owes a check of the
  package's own mappings (SE-092, S6 "Not closed" (1)).
- **The single-caller scan is textual.** A module alias or glob import passes
  it (SE-092, S6).
- **`is_safe_provider_text` and `escape_controls` disagree** about Unicode
  format characters; one shared definition is owed (SE-092 amendment, (a)).
- **The read allowlist accepts any `--long` option after a permitted
  subcommand** — `brew info --github` opens a browser. Only the gate's
  `COMMAND_IS_NOT_THE_PLANNERS_READ` pins a plan's argv (SE-050).
- **A verification read may fetch Homebrew API data** (Research findings).
- **Owed to the owner, in guarded paths:** `.claude/rules/executor.md` and its
  `CLAUDE.md` entry; `migrate` in `run-fixture-matrix.sh`; and the stray
  untracked `.agents/skills/tooling-recheck/`, which keeps reappearing and
  fails the codex-layer stage of `validate.sh` until removed (absent when this
  report was written; its history is in `02-ARCHITECTURE-REVIEW-findings.md`).
- **Pre-existing, found by `docs-auditor`:** the migration wiki pages' sample
  output says "only a None match", which reads poorly; and `wiki/` carries 50
  relative links that leave it and break on publication (SE-066, DP-007).

**Reviews.** During the pass, `spec-guardian` and `security-reviewer` reviewed
the plan and the slices; their findings are in the entries above (S6 had two
rounds: nothing Critical or High, one Medium resolved). At the end of the pass,
`spec-guardian`, `security-reviewer` and `docs-auditor` over the whole range
found no Tier 1 violation and no path from any command to a change on the
machine; their findings are closed in `7b84fab` and recorded in SE-092's
amendment.

## Recommended next tasks

1. **The owner's guarded edits** — `.claude/rules/executor.md` and its
   `CLAUDE.md` entry, `migrate` in the fixture matrix, the stray skill removed.
   *Maintenance, owner.* Verifiable: the rule loads on a touch of
   `crates/cli/src/executor/`; the matrix lists `migrate`; `validate.sh`'s
   codex-layer stage passes.
2. **Rule on DP-025 and DP-026.** *Owner.* DP-026 blocks the mutating pass.
3. **The per-subcommand long-option allowlist** in `PERMITTED_SUBCOMMANDS`, with
   a two-way test against the adapters' argv. *Focused implementation*, under
   `security-reviewer`.
4. **One definition of hostile text** shared by `is_safe_provider_text` and
   `escape_controls`. *Focused implementation.*
5. **The `brew.env` research note.** *Research*, `provider-researcher`.
6. **The concurrency flag on `plan` and `analyze`**, from the same
   `AnalysisSource`; `analysis.v1.md` updated. *Focused implementation.*
7. **The mutating half of the executor** — obligations 8 and 12, the history
   store and its schema, the lock and detector, the `--retain-source` re-run,
   and a MacPorts-to-Homebrew recording that yields a plan, so the composition
   is exercised in that direction. *Focused implementation, its own reviewed
   pass*, after 1–5.
8. **The "None match" wording** in the mapping refusal sentence and the wiki
   pages that quote it. *Maintenance.*
