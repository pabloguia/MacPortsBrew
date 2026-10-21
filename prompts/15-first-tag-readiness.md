<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — Readiness for the first tag, v0.1.0 (15)

Written 2026-10-18. Pass 09 built the release machinery and **staged** its
guarded files in [`docs/passes/09-staged/`](../docs/passes/09-staged/README.md);
pass 10 wrote the owner's release-candidate checklist; none of the staged files
is installed yet (`scripts/` has no `build-release.sh`,
`check-release-workflow.sh` or `check-no-secrets.sh`; `.github/workflows/` has
no `release.yml`). Since then, pass 08 slices 4 and 5 merged, the action pins on
`main` moved, and the policy version changed. This pass installs the machinery,
corrects what drifted, adds the check that a release build is free of the
`test-support` feature, and makes the 0.1.0 text true — so that what is left is
only the owner's: certificates, repository settings, the tag.

**Nothing here signs, tags, publishes, or handles a credential.**

---

## 1. Read this first

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/01-invariants.md`.
2. `docs/passes/09-staged/README.md` in full — the staged blocks, their
   SHA-256s, "Applying them", and the owner checklist.
3. `docs/passes/09-release-engineering-plan.md` (the secret-material check,
   D-T2, "Named tests") and `09-release-engineering-report.md`.
4. `docs/passes/10-read-only-release-candidate-report.md`, the release-candidate
   checklist.
5. `docs/spec/08-distribution-and-project.md`; `docs/adr/0005-*`.
6. `docs/decisions-pending.md`: **DP-033, DP-034, DP-035, DP-036, DP-045**.
7. `scripts/validate.sh`'s header (the one stage list), `check-action-pins.sh`,
   `run-fixture-matrix.sh`, `test-gates.sh`, `prune-target.sh`.

## 2. Authority and precedence

As `TEMPLATE-PASS.md` §2.

## 3. Prerequisites and hard boundary

**Part B starts only when** the owner has ruled DP-033, DP-034, DP-035 and
DP-045 (DP-036 may keep its default). Part A may run before that, under each
open decision's reversible default, with every line that depends on one marked
`<!-- DP-0NN pending -->` so it can be found and corrected after the ruling;
DP-033's default is "change no text", so the `mpb` lines wait.

**Do not run this pass in parallel with work order 14**: both edit
`CHANGELOG.md`, `README.md`, `docs/spec-evolution.md` and `.claude/current-pass`.

**The guarded writes need the owner's armed message.** `scripts/*.sh` and
`.github/workflows/` are guarded (SE-093). The owner sends one message that
starts with `MPB_ALLOW_GUARD_EDIT=1` and asks for this work order's part B; it
covers that message only. Prepare everything first (part A), so that the armed
message is spent on writing, not on thinking. Never reword a command to get
past a guard.

Hard boundary:

- `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`; no file under the executor,
  gate, planner, verification or `crates/adapters/src/process/` changes.
- No tag, no release, no secret, no certificate, no repository setting, no
  `gh secret`, no `workflow_dispatch` run. Pushing the topic branch and opening
  a pull request are allowed (SE-099).
- No new dependency; `Cargo.lock` unchanged.
- Every staged block is written **as staged** and checked against its SHA-256,
  except where a drift below forces a change: then write the change, record the
  new SHA-256 and the reason in the staged README and in SE, and have
  `security-reviewer` read the diff against the staged text.
- `validate.sh`'s stage list changes in its own header and nowhere else.

## 4. Working protocol

As `TEMPLATE-PASS.md` §4. `security-reviewer` runs on the strongest model
available on **every** guarded file this pass writes, before the commit that
lands it, and `spec-guardian` on the docs. Weakening a gate is invisible until it
ships something it would have caught: when in doubt, the stricter reading.

## 5. Order of work

### A. Before the armed message (no guarded path)

1. **Reconcile the staged instructions with `main`**, and write the result as a
   dated addendum at the top of `docs/passes/09-staged/README.md`:
   - the staged `release.yml`'s action pins against `main`'s
     (`actions/checkout`, `dtolnay/rust-toolchain`, and the three new actions,
     whose versions are unverified — verify each SHA against the action's
     tagged release, with a read date);
   - the identifier `build-release.sh` hard-codes, against DP-035's ruling (if
     it changes, the staged SHA-256 changes, and its "derived from the
     workspace `repository`" comment is stale either way);
   - `validate.sh`'s header ("fifteen stages") becoming seventeen;
   - `prune-target.sh` now runs at the end of `validate.sh` (SE-100); its
     default mode only trims stale object files in `target/*/deps` and never
     runs `cargo clean` from a gate: confirm that `release.yml`'s build after
     validation is unaffected;
   - stale lines in the staged README's owner checklist (DP-031, the repository
     condition already present) and in pass 10's checklist (DP-031, the
     2026-10-10 tooling note, the policy version, "no CI job has ever run").
2. **Design the `test-support` release check** (tripwire
   `the_switch_needs_release_builds_free_of_the_test_support_feature`): in
   `build-release.sh`, refuse `--features`, `--all-features` and `-F`, and
   assert with `cargo tree -e features` that the application crate resolves
   without `test-support`; in `check-release-workflow.sh`, refuse those flags on
   `build-release.sh` and `cargo build` lines without catching `validate.sh`'s
   own legitimate `cargo test --all-features`. Write the gate tests for it.
   Replace the unconditional tripwire with a structural test that the check
   exists, in the same change that lands the check.
3. **Make the 0.1.0 text true**, per DP-045 and DP-033: `CHANGELOG.md` (it says
   0.1.0 keeps no history and restores nothing, while `README.md` documents
   `history` and `restore --plan`), the README's `mpb` lines, `history
   acknowledge` and the `interrupted_run` finding, each saying what it prints on
   a machine where nothing was ever recorded. `docs-auditor` reviews.
4. Stage every guarded file's final text in the scratchpad or in the staged
   README, with its SHA-256, ready for part B.

### B. Inside the armed message

Follow "Applying them" in the staged README, amended by part A: `build-release.sh`,
the `run-fixture-matrix.sh` line, `check-action-pins.sh`'s `ALLOWED_REPOS`,
`check-no-secrets.sh` (written from the plan's design — there is no staged text),
`check-release-workflow.sh`, the two `validate.sh` stages, the `test-gates.sh`
tests (including the 24 workflow-check cases and the `test-support` ones),
`release.yml`; then `.claude/hooks/test-hooks.sh`, `scripts/validate.sh`, and
`actionlint` if present. Item 9 of
`docs/research/tooling/2026-10-17-hooks-permissions-ci.md` (the `settings.json`
deny anchors) may be applied in the same message if the owner names it; if the
auto-mode classifier refuses it again, leave it, say so, and offer the owner
the exact edit to apply by hand.

### C. After

Record every guarded file written (the disclosed-escape rule) in the report; an
SE entry for the release-check and the drift decisions; update
`prompts/ROADMAP-to-first-release.md`'s owner sheet with what is left.

## 6. Definition of done

- [ ] Every staged file installed, each matching its recorded SHA-256 (staged or
      re-recorded with its reason).
- [ ] The `test-support` check exists, has gate tests proved by mutation, and the
      tripwire is replaced by a structural test.
- [ ] `validate.sh` passes in default mode, and the hook and gate tests pass.
- [ ] CHANGELOG and README say exactly what 0.1.0 does, per DP-045 and DP-033.
- [ ] The owner's remaining list (certificates, environment, ruleset, immutable
      releases, the dispatch dry run, the `.pkg` and Gatekeeper protocols, the
      tag) is current in one place.
- [ ] Reviewers ran; findings resolved or accepted in writing; topic branch
      pushed and a pull request opened (never merged).

## 7. End-of-pass report

`/pass-report` to `docs/passes/15-first-tag-readiness-report.md`, listing every
guarded file written in the armed message.
