<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 15 — readiness for the first tag, part A: report

Branch `pass/15-first-tag-readiness`, cut from `main` at `ad5a95d`. Work order:
[`prompts/15-first-tag-readiness.md`](../../prompts/15-first-tag-readiness.md).
Run on 2026-10-22, autonomously, at the owner's request to do everything that
does not wait on one of their decisions. **Part A only**; part B (the guarded
writes) waits for the owner's armed message and for DP-033, DP-034, DP-035 and
DP-045. `security-reviewer`, `spec-guardian` and `docs-auditor` ran on the
strongest model available (Opus) before the commits; their findings are
resolved below.

**Gates.** `scripts/validate.sh` in default mode (tests and the fixture matrix)
is run by the commit gate on every commit that touches code; its result is in
the pull request's CI. `/tooling-recheck` **ran** first (three notes, no
contract break). `scripts/check-coverage.sh` was not run here; CI is the
authority on coverage.

## What shipped

Nothing that can change a machine: `THIS_BUILD_DISPATCHES_MUTATIONS` is still
`false`, and no file under the executor, gate, planner, verification or
`process/` changed. For a user, one thing changed:

- **`restore --run <id>` for a run nobody recorded no longer asks the providers**
  (SE-103). It answers `RUN_NOT_FOUND` (exit 8) from the state directory before
  a capture; before, every unrecorded id — which on 0.1.0 is every id — made
  both providers read, and on an empty Homebrew cache made Homebrew download its
  catalogue into `$HOME` (731 files where it was seen). Found by the docs audit.

For the next reader of the repository:

- **`CHANGELOG.md` and `README.md` say what 0.1.0 does** under the reversible
  default of DP-045 (a): `history`, `history show`, `history acknowledge` and
  `restore --plan --run` ship, each described by what it prints when nothing was
  ever recorded (observed on the built binary with an empty `HOME`: exit 0 and
  "No runs are recorded"; exit 8 `RUN_NOT_FOUND` for `history show`,
  `restore --plan --run` and `restore --run`; exit 0 "No interrupted run is
  recorded" for `history acknowledge`; `doctor`'s `interrupted_run` `OK`). The
  the markers were removed on 2026-10-23 when the owner ruled DP-045 (a) and DP-033 (b) (SE-104). Stale statements were
  corrected: "no CI job has run" (now dated, with the run), the "not yet
  implemented" list, the schema versions, the repository location.
- **The staged release files are reconciled with `main`** and ready for the armed
  message (`docs/passes/09-staged/README.md`, addendum of 2026-10-22): action
  pins re-verified against each tag (two changed), the new SHA-256s, the amended
  "Applying them", and the new test texts.
- **The `test-support` release check is designed, staged and proved** (SE-102):
  `cargo tree … -e normal,build -f '{p} {f}'` with a whole-word match made with
  a here-string, refusal of compiler wrappers, rule 10 in the workflow check,
  20 + 44 gate cases, 16 + 12 mutants killed, a structural tripwire that matches
  the whole command line. The patched script was run end to end, unsigned, for
  `x86_64-apple-darwin`, twice, exit 0.
- **Tooling re-verified** against the 2026-10-22 documentation: three notes in
  `docs/research/tooling/`; wording corrected in `docs/spec/11-agentic-sdlc.md`
  and `.claude/README.md`.
- **Branch hygiene** (before the pass): the seven merged remote topic branches
  were deleted on the owner's own command; only `main` remained.

## Research findings

No item of `docs/spec/09-research-checklist.md` was in scope. Facts read, with
the date (2026-10-22): each GitHub Action pin against its own tag (`gh api`);
`cargo tree`'s behaviour on this tree, including that `-e features` hides a
feature set on the root package; the Claude Code documentation (three notes).

## Decisions

- **The check asks cargo, not the script's text**, and asks it in a form that
  cannot be narrowed unseen: the stand-in `cargo` in the gate cases records its
  arguments, and the tripwire matches the whole command line (the security
  review showed that `-f '{p}'`, `--depth 0` or another `-p` blinded the first
  version while every case passed).
- **Rule 10 bans `cargo` from `release.yml` outright**, plus long feature flags
  anywhere and `-F` on a `build-release.sh` line (the review found real
  spellings the first, narrower rule missed).
- **The tripwire cannot be replaced on this branch.** The structural test reads
  `scripts/build-release.sh`, which does not exist until part B, so it is staged
  and lands with the scripts.
- **Reserved for the owner, with the default taken**: DP-045 (a); DP-035 values as
  staged; DP-033 no text change; DP-034 and DP-036 untouched.
- **Tooling recommendations not adopted** (each changes `.claude/` behaviour the
  owner has not asked to change, and `CLAUDE.md`/`.claude/` are not edited on an
  agent's recommendation): `onFailure: "block"` on the three guard hooks, the
  sandbox, the hook `if` field, tightening no-space allow rules, the two
  `Edit(./…)` anchors in `settings.json`, the reading-order contradiction between
  `CLAUDE.md` and `AGENTS.md`, `CLAUDE.md:57-58` ("a shell reader loads
  nothing"), and the `<pasted_content>` delimiter in `arm-guard-edit.sh`. All are
  in the three notes with the documented fix, and listed under SE-102 "Not done".
- No ADR written or changed.

### Review findings and what became of them

- **Security review.** M1 (a `grep -q` pipeline under `pipefail` can return 141
  and read as "no match" above ~64 KB of graph): fixed, with a 100 KB case and a
  mutant. M2 (a narrowed `cargo tree` query blinds the check): fixed. L1 (rule 10
  missed `-rF`, quoted flags, YAML multi-line scalars, a flag in a variable) and
  L2 (the tripwire's "builds only through" heuristic): fixed by the stricter
  rule. L3 (a compiler wrapper adds the cfg without cargo seeing it): refused in
  the environment and in the two cargo configuration files; a `.cargo/config` in a
  parent directory of the checkout is **not** checked (SE-102). The README
  addendum's claim that `actions/checkout` v7.0.2 exists was wrong: removed.
- **Spec review.** `restore --run` without `--plan` gives exit 8, not 4, on a
  0.1.0 machine: fixed (two places). `mpb` lines unmarked: marked. `history
  acknowledge` called read-only: reworded. SE-102 status/wording and the
  trailing-newline mismatch in two recorded SHA-256s: fixed (every hash is now
  of the block as extracted, verified by extracting all five). The stale
  `dtolnay/rust-toolchain` comment on `main`: recorded for part B.
- **Docs audit.** The "not yet implemented" list, three "no CI has run"
  statements, three development-history lines, the CHANGELOG's overstated "the
  0.1.0 section describes them" (it omitted `--dry-run`): fixed. The generated
  `docs/support-matrix.md` banner (source: `scripts/support-matrix.sh`, guarded):
  left for part B. `restore --plan --run` reading providers first: fixed in code
  (SE-103) rather than documented.

## Specification changes

Made: **SE-102** (the release check and the reconciliation) and **SE-103**
(`restore` looks the run up before it captures). Wording corrected, with the read
date, in `docs/spec/11-agentic-sdlc.md` and `.claude/README.md`. Deferred:
nothing. Rejected: nothing.

## What it does not do

It installs nothing guarded: no `scripts/build-release.sh`,
`check-release-workflow.sh`, `check-no-secrets.sh`, no `release.yml`, no
`validate.sh` stage, no `test-gates.sh` case, no change to the Rust tripwire. It
does not write `check-no-secrets.sh` (there is no staged text; part B writes it
from the plan). It does not touch `CLAUDE.md`, `AGENTS.md`,
`.claude/settings.json` or any hook. It does not tag, sign, publish or handle a
credential.

## Tests

Product code changed in one place (SE-103): `restore_of_a_run_nobody_recorded_asks_no_provider`
(fails with the call removed on a machine with Homebrew, `left ["Library"] in
HOME`) and the extended `invariant_06_history_restore_and_acknowledge_refuse_root`
(fails with the call removed). Staged tests, from the scratchpad against the
staged scripts: 20 + 44 cases passing; 16 + 12 mutants killed (one first
survived and got a case); the structural tripwire passes in a scratch copy of the
tree with the staged scripts installed and fails when the command line is
narrowed, a flag is inserted or the wrapper loop is shortened. End to end:
`build-release.sh x86_64-apple-darwin <dir>` unsigned, exit 0, payloads asserted,
twice; the arm64 build and the signed path were not exercised. Coverage floors
were not measured here.

## CI

`Repository validation` passed on `main` at `ad5a95d` (run 39578935218,
2026-10-21) on macos-26, macos-26-intel, macos-15 and macos-15-intel. This
branch's run is on its pull request. `release.yml` has never run: it is not
installed.

## Known limitations and risks discovered

- The gate-test texts are bash with single-quoted shell on purpose; installing
  them needs `# shellcheck disable=SC2016` after the header of `test-gates.sh`.
- The 44 workflow-check cases are reconstructed from the description of the
  original 24; the 2026-10-11 harness was not kept.
- `repository-validation.yml` carries two stale comments (`taiki-e/install-action`
  `# v2` is v2.87.28; `dtolnay/rust-toolchain` "resolved 2026-09-02" is a commit
  of 2026-10-16), and the generated support matrix still says no CI job has run:
  three guarded edits for part B.
- Newer patch releases exist for `actions/upload-artifact` (v7.0.2) and
  `download-artifact` (v8.0.2); a pin moves by decision, not here.
- The binary's top-level `--help` calls `history` "Reads only" while
  `history acknowledge --yes` can write (a catalogue string, out of this pass).
- The unverified items listed in the three tooling notes.
- The two behavioural claims about Homebrew's cache are observed on one machine.

## Recommended next tasks

1. **Owner**: rule DP-033, DP-034, DP-035, DP-045 (recommendations in
   `prompts/ROADMAP-to-first-release.md`); then send the armed message for part B
   (focused implementation pass, Opus preferred).
2. **Owner, optional**: the `.claude/` fixes in the tooling notes, as one armed
   message (a configuration pass).
3. **Small, unguarded**: reword the `history` catalogue line in `--help` so it
   does not say "Reads only" (an i18n-parity change).
4. **Pass 08 slice 6** (verification before removal), after the release track.

## Addendum, 2026-10-23 — the owner's rulings

DP-033 (b), DP-034 (b) first with (a) as the fallback, DP-035 as recommended and
DP-045 (a) were ruled and propagated (SE-104, `docs/decisions-pending.md`, the
staged README, spec 06, ADR-0003, ADR-0005, README, two wiki pages, the roadmap).
Propagating DP-033 found that **no install route here ever detected an existing
`mpb`**, contrary to what spec 06, ADR-0003, the README and the wiki had said; the
amended texts say so. What is left is the list in `prompts/ROADMAP-to-first-release.md`
("Remaining work after the rulings").

### Addendum, 2026-10-23 — the DP-034 research, and the guarded write it needed

At the owner's armed message (one message, `MPB_ALLOW_GUARD_EDIT=1`) a throwaway
branch `research/dp034-security-stdin` was cut from `main` carrying one guarded
file, `.github/workflows/dp034-security-stdin.yml` (SHA-256 `adb1d736…`, verified
before the commit). It ran once, through pull request #14, on four hosted runners;
no secret was involved and nothing was posted to the pull request. Result: `security -i`
holds (R-19 addendum, SE-105, DP-034 resolved). The pull request is closed
unmerged; the remote branch is the owner's to delete. The repository's own
"Repository validation" workflow also started by itself on that push, as it does
on any push; no other workflow was started by this session. The independent
`security-reviewer` read of the workflow was declined by the permission
classifier and not repeated; the owner accepted the self-review.

