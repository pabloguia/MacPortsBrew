<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — Hardening before mutation (06-HARDENING-BEFORE-MUTATION)

The executor's dry-run half is done (pass 05, report
`../docs/passes/05-the-executor-report.md`). `migrate` renders a plan, and
`--yes` puts it to an execution gate that refuses every plan this build
produces. Nothing changes the machine. Before any pass is allowed to dispatch a
mutating step, five gaps that pass found must be closed. Each is small,
independently verifiable, and does not need the owner's open decisions. This
pass closes them. It does not start the mutating half.

---

## 1. Read this first

In this order, before doing anything else:

1. `AGENTS.md` — orientation.
2. `docs/spec/00-charter.md` — what MacPortsBrew is, who for, what it is not.
3. `docs/spec/01-invariants.md` — Tier 1. Non-negotiable.
4. `docs/spec-evolution.md` — what has already been argued. For this pass,
   SE-050 (the spawn-site allowlist and its "Not closed" list), SE-092 (the
   executor, its S6 addendum and the end-of-pass amendment), SE-015 (the
   concurrency flag) and SE-057.
5. `docs/spec/11-agentic-sdlc.md` — how work is shaped here.
6. `docs/passes/05-the-executor-report.md` — "Known limitations" and
   "Recommended next tasks". This pass is tasks 3, 4, 5, 6 and 8 of that list.
7. `docs/spec/04-migration-and-safety.md` (Process execution, Threat model),
   `docs/schemas/analysis.v1.md`, `docs/schemas/plan.v1.md`.

Read the rest on demand.

## 2. Authority and precedence

| Tier | Source | Changeable? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | No. If one seems wrong, **stop and ask the human.** |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. |
| 3 | **MUST** in `docs/spec/02..11` | Only with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Change freely; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

You are expected to improve tiers 3–5 where evidence or sound engineering
justifies it. Never silently. Use `/spec-evolution` when you do.

## 3. Hard boundary for this pass

- **Nothing in the binary dispatches a mutating step.**
  `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`, and
  `every_plan_this_planner_builds_is_refused_for_execution_in_this_build` and
  its fixture counterpart in `crates/cli/tests/execution_gate_fixtures.rs` stay
  green and unweakened.
- **The spawn-site allowlist only narrows.** No entry is added to
  `PERMITTED_PROGRAMS`. A change to `PERMITTED_SUBCOMMANDS` may only remove
  what an adapter read does not use, proved by a test that every argv the
  adapters and the planner build still passes.
- **No new dependency.** `Cargo.lock` is unchanged at the end of the pass.
- **No guarded path is edited**: `.claude/`, `scripts/*.sh`, `.codex/`,
  `.agents/skills/`, `.github/workflows/`. If one needs changing, say so in the
  report as owed to the owner.
- **DP-025 and DP-026 stay the owner's.** Their reversible defaults stay in
  force.

## 4. Working protocol

- **Verify, do not assume.** Observed with a read-only command, cited with a
  read date, or runtime detection. No fourth option.
- **Never touch the developer's real package managers.** Reading is allowed;
  writing is a failure.
- **Re-verify the tooling first.** `/tooling-recheck`; record drift.
- **Delegate by default.** Research, sweeps and reviews go to subagents, in
  parallel where independent; the main session synthesises and edits.
- **Use the reviewers.** `spec-guardian` on every slice; `security-reviewer`
  on anything under `crates/adapters/src/process/`, `registry.rs`, the snapshot
  loader or output escaping; `docs-auditor` on anything user-visible.
- **Prove each new rule by mutation.** Break it, see a named test fail,
  restore. Run mutations in a scratch copy of the workspace with its own
  `CARGO_TARGET_DIR`, restore by rewriting the file and `touch`ing it, and
  `diff -r` the copy against the tree at the end. Pass 05 lost a review to a
  stale mutant build that a mtime-preserving restore left in `target/`.
- **Watch the Stop gate's textual scans.** Never write a mutating provider verb,
  `sudo`, `symlink` or `launchctl` as a literal in `crates/*/src`, nor a
  four-digit number INV-20's deadline scan reads as a year (write `0x2028` or
  `8232`, not `'\u{2028}'`). Assemble verbs at run time in tests, as
  `.claude/hooks/test-hooks.sh` does.
- **Commit incrementally.** Conventional commits, one coherent change each,
  each with `scripts/validate.sh` passing.
- **When blocked on a human decision**, use `/decision-pending`: options,
  recommendation, reversible default. Then keep going.

## 5. Order of work

1. **Research first: `brew.env` under `HOME`** (task 5). `provider-researcher`
   answers whether a `brew.env` in the user's real `HOME` can override the
   quieting environment the adapters set (`HOMEBREW_NO_AUTO_UPDATE` and the
   rest), and whether the `HOME` the process executor passes reaches it. Dated
   note in `docs/research/`. It goes first because its answer may change what
   item 2 has to cover.
2. **The per-subcommand long-option allowlist** (task 3). Today the allowlist
   accepts any `--long` option after a permitted subcommand, so `brew info
   --github` passes it and only the gate stands between a plan and a browser.
   Give each permitted read subcommand the long options it may take; refuse the
   rest at construction and at the spawn site. Two-way test: every argv the
   adapters and the planner build passes, and a table of off-list options is
   refused through `plan_read_spec`. If item 1 finds an environment route,
   close it here too. Close SE-050's "Not closed" item for it.
3. **One definition of hostile text** (task 4). `is_safe_provider_text` (the
   snapshot loader) accepts Unicode format characters that `escape_controls`
   (the executor's output) treats as hostile, and neither shares the other's
   definition. Put one definition in `domain`, use it in both, test it against
   the same table (Cc, Cf, U+2028, U+2029). Decide in writing whether the
   loader refuses or keeps-and-escapes, by INV-15 and what a snapshot is.
4. **The concurrency flag on `plan` and `analyze`** (task 6), from the same
   `AnalysisSource` `migrate` uses; `analysis.v1.md` and `plan.v1.md` updated;
   `invariant_15` coverage for replay and from-snapshot.
5. **The "None match" wording** (task 8): the mapping refusal sentence that
   prints a confidence label as a bare word, and the wiki pages that quote it.
   Catalogue and samples together; `documented_samples` stays green.

Items 2–5 are independent once item 1 is in; run their implementers in parallel
only if they touch disjoint files.

## 6. Definition of done

Demonstrate each; do not assert it.

- [ ] A dated research note answers the `brew.env` question, with the commands
      run and the documentation read.
- [ ] An off-list long option is refused at construction and at the spawn
      site, for both providers, by named tests proved by mutation; every
      adapter and planner argv still passes.
- [ ] One hostile-text definition is used by the loader and by output
      escaping, with one shared test table.
- [ ] `plan` and `analyze` carry the concurrency flag, and their schemas say
      so.
- [ ] No user-facing text prints a bare confidence label.
- [ ] Section 3 holds: the gate tests are green and unweakened, `Cargo.lock`
      is unchanged, no guarded path is edited.
- [ ] `/preflight` passes, or the report names what was skipped and why.
- [ ] Every invariant this pass touched can be pointed at by name to its test.
- [ ] `docs/spec-evolution.md` records every material deviation, and the items
      closed from SE-050's and the pass-05 report's lists are marked closed
      there.
- [ ] The reviewers have run and their findings are resolved or accepted in
      writing.

## 7. End-of-pass report

Write it with `/pass-report` to
`docs/passes/06-hardening-before-mutation-report.md`. Every material deviation
it names must already be in `docs/spec-evolution.md`. Carry forward, as still
owed, everything from the pass-05 report this pass did not close — in
particular the owner's items (DP-025, DP-026, `.claude/rules/executor.md`) and
the mutating half of the executor.
