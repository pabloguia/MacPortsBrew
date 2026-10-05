<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 2 — the single-instance lock, the interrupted-run marker, the concurrent-provider detector

Work order for one session. Parent work order:
[`08-the-mutating-executor.md`](08-the-mutating-executor.md), item 2 (its
section 3, the hard boundary, binds here unchanged). Written 2026-10-04 at the
end of slice 1, so that a fresh session can carry on without that session's
memory. Everything a step needs is named here or linked.

## 0. Read first, in this order

1. `AGENTS.md`, `CLAUDE.md` (both are loaded for you), then
   `docs/spec/01-invariants.md` — Tier 1, not yours to change.
2. `docs/spec/04-migration-and-safety.md`: "Concurrency and locking", "History",
   "Threat model". `docs/spec/06-cli-and-ux.md`: the exit-code paragraph (an
   interrupted run "exits 4 and names the run"; a held lock is exit 7).
3. `docs/spec-evolution.md`: **SE-015** (concurrency) and **SE-092**, especially
   its last two bullets — the history-store addendum and its amendment dated
   2026-10-04. They list what slice 1 left owed *to this slice*.
4. `docs/passes/08-the-mutating-executor-plan.md` — slice 1's reviewed plan, as
   a model of the plan you will write.
5. Code you build on: `crates/adapters/src/persistence/{history_store,atomic,state_dir,bounded_read}.rs`,
   `crates/domain/src/{history,concurrency,error}.rs`,
   `crates/cli/src/app/{context,migrate}.rs`,
   `crates/cli/tests/history_preconditions.rs` (the tripwires you will extend).

## 1. What this slice delivers

### Part A — slice 1's owed tests (do this first; it is mechanical)

SE-092's amendment lists three tests slice 1 owes. Write them:

- **A lost reply is success**: `write_new_atomic` meets `EEXIST` while its own
  temporary already has two links. Simulate it without a real network
  filesystem — the cleanest way is to split the `EEXIST` decision into a small
  private pure function (`fn lost_reply(nlink: u64) -> bool` or similar) and
  unit-test it, *plus* keep the existing integration test. Then re-run the
  mutation "remove the rule (always `Exists`)" and confirm it is now killed.
- **A crash after the link leaves a readable store**: a published record next
  to a leftover dot-temporary at two links reads back fine (`read_view`).
- **A `0277` umask still yields a readable record**: the umask is
  process-wide, so this test must run in a child process (e.g. a test that
  re-executes the test binary with an env var selecting a single test, or an
  `#[ignore]`d test driven by a separate integration-test binary). If you
  cannot do it cleanly, record why in SE-092 and leave it owed — do not fake it.

Correct SE-092's "overclaims one rule" sentence once the lost-reply mutation is
killed. Commit: `test(persistence): …`.

### Part B — research before design (delegate to `provider-researcher`)

Two dated, sourced research notes in `docs/research/` (next free `R-NN`
number; see the existing notes for format). Read-only observation on this
machine plus official documentation only; **never run a mutating `brew`/`port`
subcommand** (hooks block them anyway).

1. **Where each provider holds its own lock while it mutates**, and whether a
   stale lock is distinguishable from a live one (Homebrew: its `locks`
   directory under the prefix, what files appear during `install`/`upgrade`
   per the docs and source; MacPorts: the registry lock / `port`'s lockfile).
   Answer: can a *read of the filesystem alone* tell "a provider is mutating
   right now"? With what false positives and negatives?
2. **Whether `std::fs::File::lock`/`try_lock` (advisory `flock`) is available
   on the pinned toolchain** (`rust-toolchain.toml`) and how it behaves on
   macOS: released on process death, behaviour across `fork`, on network
   volumes. Cite the std docs with the read date.

These answers decide the design. Do not design the detector before they exist.

### Part C — the plan, reviewed before code

Enter plan mode. Write `docs/passes/08-slice-2-plan.md` in the shape of slice
1's plan (facts, design, commits, named tests, decisions). Run `spec-guardian`
and `security-reviewer` on it **in parallel**, fold their findings in (cite
each as `[SG H1]` / `[SR M2]` as slice 1 did), commit the plan, then build.

The design must answer at least these, each as a decision for SE-092/SE-015:

- **Lock mechanism.** Recommended starting point (verify with Part B): an
  advisory `try_lock` on a file in the state directory (`StateDir::root()`),
  held for the life of a mutating run, so a killed process releases it and
  there is no stale-lock cleanup to get wrong. A held lock is exit **7**
  (06). The lock is taken only for a mutating run (`mutationDispatched` is the
  key, per 04 History: "a run that dispatched no mutating step … takes no
  lock"), after the gate passed and the history probe succeeded, before the
  first mutating step. Reading (analysis, plan, dry run) never takes it.
- **Interrupted-run marker.** Written (atomically, private, via the existing
  persistence helpers) before the first mutating step with the run id;
  removed only after the run's record is published. A mutating command that
  finds a marker refuses: exit **4**, naming the run (06). Decide and record
  how a user clears it — this slice may only *document* the remedy if the
  `history` / `restore` commands (item 7) do not exist yet; never invent an
  unreviewed delete path. Reading the marker creates nothing; a marker that
  is not exactly what the writer leaves is unusable, fail closed (mirror
  `HISTORY_UNUSABLE`'s rules: owner, mode, symlink, size, grammar).
- **Concurrent-provider detector.** Only if Part B shows a filesystem read can
  answer it: an adapter-level check (provider names live **only** in
  `crates/adapters/`) that yields `ConcurrentProviderActivity::NoneObserved` /
  `Observed`, with the unknown-value fallback SE-015 asks for. If the answer
  is "the filesystem cannot tell", the detector stays `NotChecked`, the
  refusal-before-mutating stays owed, and you record that — **do not add a new
  process invocation** (`ps`, `pgrep`, `lsof`): growing the spawn-site
  allowlist needs an owner-approved, security-reviewed change (parent work
  order §3). Under `--replay`/`--from-snapshot` it must stay `Unknowable`
  (INV-15).
- **Reachability.** Exactly as slice 1: nothing dispatches a mutation, so the
  lock and marker calls are not wired into the executor shell yet. Instead
  **extend `crates/cli/tests/history_preconditions.rs`** with tripwires that
  fail the build which sets `THIS_BUILD_DISPATCHES_MUTATIONS` without the
  executor shell taking the lock and writing/clearing the marker. Name them
  for what they guard.
- **Error identifiers.** A held lock and an unusable marker may need new
  `ErrorId`s. Adding one touches: `crates/domain/src/error.rs` (`exit_code`,
  `message_id`, `all()`, `NUMBER_OF_ERROR_IDS`, the exit-code test),
  `crates/application/tests/executor.rs` (its exhaustive list),
  `locales/en.toml` (message + remediation + `{detail}`),
  `docs/schemas/README.md` (exit table). Exit 7 currently has no identifier —
  check the table first.

### Part D — build, prove, review, record

- One coherent Conventional Commit per step; every new rule unit-tested and
  **proved by mutation** (procedure in §3).
- Invariant tests named `invariant_NN_*` after the invariant they defend
  (INV-15 for "replay never locks or reads the marker", INV-17 for direction
  neutrality of anything provider-facing, INV-06 if anything touches root).
- Docs in the same change: 04 "Concurrency and locking" (status of what
  exists), `docs/schemas/*.v1.md` for any new output field, SE-015 and SE-092
  entries, CHANGELOG.
- Before reporting done: `spec-guardian`, `security-reviewer` and
  `docs-auditor` on the full slice range, in parallel; fix or record every
  finding in writing; `scripts/check-coverage.sh`; `scripts/validate.sh`.

## 2. Hard boundary (from the parent work order, restated so it is not missed)

- `THIS_BUILD_DISPATCHES_MUTATIONS` stays **`false`**. Do not edit
  `crates/cli/src/executor/`, `crates/application/src/executor.rs`,
  `execution_gate.rs` or `verification.rs` beyond what a reviewed plan says,
  and never weaken a gate test.
- **No live provider is touched** (INV-05); no elevation (INV-06); no new
  dependency; `Cargo.lock` unchanged.
- **Guarded paths are off-limits** in this session: `.claude/hooks/`,
  `.claude/settings*.json`, `scripts/*.sh`, `.github/workflows/`, `.codex/`,
  `.agents/skills/`. Only the owner can arm an edit there; never write the arm
  token yourself. If something there blocks legitimate work, stop and say so.
- `docs/spec/01-invariants.md` is read-only. Decisions that spend the owner's
  money, licence or audience go to `docs/decisions-pending.md` with a
  recommendation; take the reversible default and continue.
- `git push` is denied (no remote yet). Commit locally.

## 3. How this repository bites (learned the hard way in slice 1)

- **The commit gate** runs `scripts/validate.sh` (tests + fixture matrix,
  ~5–10 min) before any `git commit` that touches code, and refuses on
  failure. Run `cargo fmt` in a *separate* command before committing — the
  gate checks the tree before your command runs.
- **The Stop gate** scans shipped source (`crates/*/src`, comments excluded)
  for provider-mutating subcommand words inside string literals (`install`,
  `upgrade`, `link`, `activate`, …) and for elevation/symlink tokens (the
  privilege-escalation command, `symlink`, `launchctl`, `setuid`, …). A test
  that needs one of those goes under `crates/<crate>/tests/`, not in a
  `#[cfg(test)]` module in `src/`. Error messages must avoid those words too.
- **The Bash guard** rejects any command line containing an elevation word —
  even inside a Python heredoc doing a text replacement. Put such scripts in a
  file under the scratchpad and run the file.
- **The toolchain pin**: a shell command line that names `rust-toolchain.toml`
  is blocked (ADR-0002 guards it). Read it with the Read tool instead.
- **Provider-name scans**: nothing above `crates/adapters/` names a provider,
  and `crates/cli/tests/executor.rs` (`invariant_17_the_executor_names_no_provider`)
  scans `crates/cli/src/executor/` and `crates/cli/src/app/migrate.rs` word by
  word, prose included. Tests that must name providers live in a separate
  file (see `crates/cli/src/app/migrate_history_tests.rs`, included with
  `#[cfg(test)] #[path = …]`).
- **serde**: on an internally tagged enum, *unit* variants ignore unknown keys
  even under `deny_unknown_fields`; mirror them as empty struct variants
  (`Variant {}`), as `StoredEnding`/`StoredOutcome` do.
- **Wall clock in output**: `snapshotProvenance.capturedAt` differs between two
  runs a second apart; compare outputs with that line filtered out.
- **i18n**: render message keys as string literals (a `match`), never
  `format!`-built keys, or `scripts/check-i18n-parity.sh` cannot see them.
- **Reading files**: hooks deny whole-file `Read` of large code files; use the
  token-pilot tools (`smart_read`, `read_symbol`, `read_range`,
  `read_for_edit`) or bounded reads. Path-scoped rules in `.claude/rules/`
  load only when the **Read** tool opens a file under that subtree — open one
  file there with Read before editing it.
- **Replay fixtures have no `PATH`**: `Snapshot.providers[*].resolutions` is
  empty under `--replay`. To exercise a live-only branch, build the context
  from a recording in an in-crate test and adjust it (slice 1's
  `migrate_history_tests.rs` does exactly this).
- **Mutation testing procedure** (no extra tooling): copy the file to the
  scratchpad, apply one textual mutation with a small script that asserts the
  pattern exists, run the narrowest test target, record killed/survived,
  restore the copy, and at the end `cmp` every file against its backup and
  `touch` it. Run long batches with `run_in_background`. A survivor means a
  missing test — add it, or record why the mutant is equivalent.

## 4. Definition of done

- [ ] Part A: three tests written (or one recorded as owed with the reason);
      the lost-reply mutation killed; SE-092 corrected.
- [ ] Part B: two research notes, dated and sourced.
- [ ] Part C: `docs/passes/08-slice-2-plan.md` reviewed and committed before
      code.
- [ ] Part D: lock, marker and (if research allows) detector shipped with
      named tests proved by mutation, or carried in writing; tripwires
      extended; docs and SE entries in the same changes.
- [ ] `THIS_BUILD_DISPATCHES_MUTATIONS` still `false`; no gate test weakened;
      no guarded path edited; `Cargo.lock` unchanged.
- [ ] Reviewers run on the slice; every finding fixed or accepted in writing.
- [ ] A short report appended to `docs/passes/08-slice-2-plan.md` ("As built"),
      listing commits, decisions, what stays owed, and anything for the owner.
