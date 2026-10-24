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


## Part B, 2026-10-23 — the guarded writes

Two armed messages from the owner (`MPB_ALLOW_GUARD_EDIT=1`). The first closed
part-way: a background review's hand-back arrives as a message and ends the
escape, so a write was refused and the session stopped and said so; the second
re-armed it, and every guarded file was written to its final text before the
second review's hand-back arrived. No hook was bypassed. Each commit was built
from the index (the working tree already held the final text, which the commit
gate validated) and then validated `--quick` on its own in a detached worktree.

| Commit | Change |
|---|---|
| `fbc270b` | three release actions allow-listed; `MPB_FIXTURE_BIN`, announced when set |
| `2823259` | `check-no-secrets.sh` and 40 gate cases |
| `f4fc487` | the staged machinery as staged; two `validate.sh` stages; the structural test |
| `3ad50fa` | `mpb` in the payload (DP-033 b); README's release-notes table |
| `33eb543` | the `security -i` signing step (DP-034 b) |
| `c40a920` | the dated CI note in the support matrix; two pin comments |

**Every guarded file written under the escape (the disclosed-escape rule).**
`.claude/.guard-edit-log` records only Edit and Write tool writes; the files
below marked *cp* were written by `cp` or `python3` from verified scratch copies
and appear in no log, so this list is the record:

- `scripts/check-action-pins.sh` (Edit) — `ALLOWED_REPOS`.
- `scripts/run-fixture-matrix.sh` (Edit, twice) — the `BIN` line and the announcement.
- `scripts/test-gates.sh` (Edit, twice; then *cp*/python) — the pins cases, the
  SC2016 directive, the two staged blocks, the `mpb` cases, the `check-no-secrets` cases.
- `scripts/check-no-secrets.sh` (*cp*) — new.
- `scripts/check-release-workflow.sh` (*cp*) — new, as staged.
- `scripts/build-release.sh` (*cp*) — new, the `mpb` revision (the staged text went
  into `f4fc487` from the index).
- `scripts/validate.sh` (*cp*) — two stages, the header, the `unset`.
- `scripts/support-matrix.sh` (*cp*) — the banner and the exact `--check`.
- `.github/workflows/release.yml` (*cp*) — new, the DP-034 revision (the staged text
  went into `f4fc487` from the index).
- `.github/workflows/repository-validation.yml` (*cp*) — two comments.

Not touched: `.claude/settings.json`, any hook, `CLAUDE.md`, `AGENTS.md`, the
executor, gate, planner, verification and process code; `THIS_BUILD_DISPATCHES_MUTATIONS`
is still `false`; `Cargo.lock` unchanged; no dependency added. No workflow was
dispatched, no tag, release, secret or setting touched.

**Verification.** All six staged blocks matched their SHA-256s. shellcheck clean
over `.claude/hooks/*.sh` and `scripts/*.sh`; actionlint clean on both workflows;
`test-gates.sh` 190 cases; `.claude/hooks/test-hooks.sh` 196/196; `scripts/validate.sh`
default mode passed on the final tree (5 min 8 s), the fixture matrix included.
Unsigned dry run of `build-release.sh` for `x86_64-apple-darwin`: exit 0, both
names in the `.pkg` and the `.dmg`, four verified `SHA256SUMS` entries; the
containers were also opened by hand (`pkgutil --expand`, `hdiutil attach`).
arm64 and the signed path were not exercised; whether the stdin-built keychain
signs, and whether the executable's notarisation ticket covers the bare `mpb`, are
first shown by the owner's dry run and the first signed release.

**Reviews.** `security-reviewer` (Opus), twice. First (commits A and B): High H1,
`check-no-secrets.sh` scanned only the working tree — fixed (it reads the index
too); Medium A1, a silent `MPB_FIXTURE_BIN` — fixed (announced; `validate.sh`
unsets it); the Mediums on encodings, base64 and team spellings — fixed; the Lows
— fixed except `Developer ID …: Name (TEAM)` (false positives on R-9's quotations).
Second (everything else): no High; **M1** the named sign-path test does not exist
and **M2** the commit gate cannot see a credential added in the same command —
accepted, recorded as follow-ups (`prompts/ROADMAP-to-first-release.md`, 2a), not
fixed because the escape had closed; the Lows likewise.

`spec-guardian` on the docs: **H1** the bare `mpb` file goes beyond DP-033 (b)
("the bare-file route still installs one name") and ADR-0005's "three artifacts" —
it was built because the owner's message asked for `SHA256SUMS` to gain the second
file; recorded as **DP-052** (recommendation: drop it), the build as committed being
the reversible default, and spec 08 no longer credits DP-033 for it — **ruled (a),
keep it, by the owner on 2026-10-23**: ADR-0005's Decision amended to four
artifacts, and `SIGNING-RECORD.txt` carrying the `mpb` fields joins follow-up 2a; **H2** the
spec 08 edit had dropped the clause binding the `.dmg` copy target and the `cp`
line to the prefix exclusion — restored; **M1** the per-artifact notarisation
record has no entry for the bare `mpb` — recorded in SE-104, and the
`SIGNING-RECORD.txt` follow-up moved before the tag; **M2** SE-052 amended in place
for the dated CI note; **M3** spec 08 now scopes the `PATH`-order guard to the
`.pkg` and `~/.macportsbrew/bin`; Lows fixed (DP-033's propagation list, the
roadmap's ordering and commit range, CONTRIBUTING on the no-binary rule) except
`build-release.sh`'s comment naming the missing test (guarded; follow-up 2a).

`docs-auditor`: the "see below" pointer (fixed here); spec 08, ADR-0005 and
`wiki/Getting-Started.md` still counted three artifacts (amended); the notarisation
of the bare `mpb` stated as fact (hedged everywhere it appears: README, staged
README, SE-104; `build-release.sh`'s comment is guarded, follow-up 2a); README's
`SIGNING-RECORD.txt` sentence (corrected); the `mpb` row's `PATH` claim scoped, and
a `cp` line added for the short name on the terminal route; the roadmap's undated
"now" and "Done" (fixed). Left, guarded: the matrix banner's opening ("CI has run
once that this file names" reads as "once") and the comments in `validate.sh` and
`check-licence-text.sh` that still say CI never ran (follow-up 2a). Not taken:
a non-affiliation line in the release notes (L8, optional; the owner's call).

## Follow-ups 2a and DP-052, 2026-10-23

The owner ruled DP-052 (a), keep the bare `mpb`: ADR-0005's Decision now says
four artifacts (commit `d68b4d8`, docs only). A third armed message then did
follow-ups 2a. Every guarded write was made before any background task was
started, so no hand-back could close the escape mid-way.

**Guarded files written under that escape (disclosed-escape rule):**
`scripts/check-no-secrets.sh`, `scripts/test-gates.sh`, `scripts/build-release.sh`
(comments only), `scripts/validate.sh` (comments only), `scripts/check-licence-text.sh`
(a comment), `scripts/support-matrix.sh` (the banner's opening),
`.github/workflows/release.yml`. All by `python3` in place, so none is in
`.claude/.guard-edit-log`, except one later Edit of `scripts/test-gates.sh`: the new
`check-no-secrets.sh` refused that file's own sign-path stub environment, which
spelled a team-identifier-shaped literal; it is now built at run time.

- **The sign-path test and the binding refusals** (the review's M1): 8 cases. The
  real script runs from a throwaway committed tree with PATH stubs; the input
  writes a marker if it is ever executed; each case also requires `codesign` to
  have been reached, so a run that died early cannot pass. Five mutants of
  `build-release.sh` (the guard removed, `--from-binary` dropped from it, the
  binding check removed, the hex check made case-insensitive, a hash without
  `--sign` accepted) each killed.
- **`check-no-secrets.sh`** (M2 and Lows): untracked, non-ignored files are scanned
  by every rule, so `git add <key> && git commit` is refused by the commit gate;
  the unarmoured P-256 PKCS#8 and SEC1 EC key forms; `GIT_DIR` and its kin cleared.
  Seven new cases.
- **`release.yml`**: `RUNNER_TEMP` asserted free of characters `security -i` splits
  on, before any keychain exists; `SIGNING-RECORD.txt` carries `payload_names`,
  `mpb_sha256`, `mpb_cdhash` (DP-052 (a)).
- **Text**: `build-release.sh` now says the bare `mpb`'s notarisation is expected,
  not shown; the support-matrix banner opens "The CI run on `main` this file
  names"; `validate.sh` and `check-licence-text.sh` say CI *had* never run.

**Review** (`security-reviewer`, after every guarded write and before the commits): no
High. **Medium**: on an arm64 host the sign-path test cannot see a removed guard (the
script then takes its "host is not x86_64" branch), so on two of the four CI runners
only the text check holds it, and that check knows one spelling; the dynamic run stops
at the first `codesign`. **Lows**: `git add -f` of an ignored key, untracked scratch files
blocking the gates (fails safe), the OpenSSL P-256 and P-384 key forms, `GIT_CONFIG_*`,
and the test trees trusting the caller's git environment. The escape had closed when
the review returned, so none is fixed here; all are follow-up 2b in
`prompts/ROADMAP-to-first-release.md`, before the tag. On this x86_64 host the
sign-path test is a real test (five mutants killed).

Commits: `3e836ec` (`check-no-secrets.sh` and its cases; the roadmap rides along
because its old text quoted one of the new prefixes), `ef24a85` (the sign-path and
binding tests; `build-release.sh` comments), `59e959d` (`release.yml`), `678bbb8`
(banner and comments). One commit attempt was refused by the commit gate with a
`validate.sh` failure that did not reproduce: `--quick`, `cargo test` and the full
default mode then passed. The retry committed only the staged roadmap, because the
refused command had been blocked whole, before its staging ran; the commit was
amended, before any push, to hold the two files its message names.

## Follow-up 2b, 2026-10-24

A fourth armed message; every guarded write was made before any background task.
**Guarded files written:** `scripts/check-no-secrets.sh` and `scripts/test-gates.sh`
(both by `python3` in place, not in `.claude/.guard-edit-log`). A stray copy,
`scripts/.tg-arm.sh`, was written by mistake during the arm64 simulation and deleted
at once; it was never staged.

- **The sign-path test is real on arm64** (the 2a review's Medium). A `uname` stub,
  and a canary first: over a copy of the script with the guard replaced by `if false`,
  the harness must see the input run. Observed: with the stub reporting arm64 (what an
  Apple Silicon host did before), the canary fails loudly; with the committed stub and
  the guard removed, the input is caught executed.
- **The text check** works on logical lines and catches the input run as `"$bin"`,
  `${bin}` or bare after a separator, `exec`, `env`, `!`, `$(`, and the listed copies in
  command position. Three mutants (a copy run, an extra `exec`, a run inside the guard
  branch) killed. **It is not complete**, and its own comment says more than it does: the
  review found spellings it misses (`"$from_binary"`, `"$built"`, subshells, braces,
  sourcing, backticks, `arch`, a single `&`, a renamed copy) and that it does not check the
  two runs sit inside the guard branch. Follow-up 2c replaces it with an allowlist.
- **`check-no-secrets.sh`**: name rules over ignored entries (`--directory`, so
  `target/` is one entry; the review showed this also hides a key *inside* an ignored
  directory such as `docs/research/.scratch/`, follow-up 2c), hits labelled untracked or ignored, the PKCS#8 EC form for
  P-256 and P-384 (prefixes observed with `openssl genpkey`), P-384 SEC1, the
  `GIT_CONFIG_*` environment cleared. Five mutants killed.
- **`test-gates.sh`** clears the git environment and runs no hook on its throwaway commits.

213 gate cases pass.

**Review** (`security-reviewer`, after the writes, before the commit): no High; the
two Mediums above (the incomplete text check, keys inside ignored directories) and two
Lows (name-rule advice that leads to a second refusal; the gate cases reading the
developer's global git config) are follow-up 2c. The escape had closed when the review
returned. What is committed is stronger than what it replaces in every respect, and
the report and roadmap say where it stops.
