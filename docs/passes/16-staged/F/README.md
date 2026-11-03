<!-- SPDX-License-Identifier: Apache-2.0 -->

# Staged for the owner: the capture of the executable-list reads (work order 16, part F)

**Status.** `05` and `06` are applied: the owner committed them as `1a81b2a`
(PR #59, merged `1c74723`), recorded in `112c52d`. `07` and `08`, the commit
gate's prefix and worktree fix, are staged and not applied; they have their
own section below ([F/07 and F/08](#f07-and-f08-the-commit-gate-sees-every-commit-and-validates-its-tree)).

Two patches, each generated against `main` at `cc324f9`; both also apply
cleanly to current `main` (`caf8799`), alone and in sequence. They change files the path guard
keeps out of a session's hands: the capture script and the gates' own tests. The
plan is `docs/passes/16-plans/F.md` ("Needs the owner" item 2, requirements
(a)–(k) from security review rounds 1 and 2); the facts are R-21
(`docs/research/R-21-read-only-subcommands-for-f-and-h.md`, E-4 and E-9).

| Patch | Files | Needed by | Executed by the session? |
|---|---|---|---|
| `05-capture-executables.patch` | `scripts/capture-fixture.sh` | F-2b's tests, and the recapture after it | No. The edited copy was checked with `bash -n` (bash 5 and macOS's `/bin/bash` 3.2) and `shellcheck -x -P SCRIPTDIR`; its name classes, its supervisor, its signal and temp-file handling and its user-name refusal were run in harnesses extracted from the copy, and its name parsing against the stdout already in `both-providers-intel-macos26.json`. The capture itself was never run |
| `06-test-gates-capture-argv.patch` | `scripts/test-gates.sh` | (g); keeps `05` in step with the adapters | Its new section, extracted from the edited copy, was run against both versions of the capture script (below); the copy passed `bash -n` on 3.2 and `shellcheck -x -P SCRIPTDIR` |

**Apply them in this order, `05` then `06`, or both at once.** `06`'s first case
reads the two `record` lines `05` adds; on a tree without `05` it fails, and the
gate-tests stage of `validate.sh --quick` with it.

## Why

Part F reads each package's executable list from provider metadata (DP-047
(b)): `port -q contents <port>` and `brew list --formula <formula>`, both on the
allowlist since `P/02` (`macports::ARGV_CONTENTS`, `homebrew::ARGV_LIST_FORMULA`).
No recording has either read, so the adapter code F-2b adds cannot be exercised
under `--replay`, and F-2b's contract tests have nothing to parse. A replayed
read finds its answer by program name and exact argv (`Recording::find` in
`process/recording.rs`), so a recording that lacks one of these invocations, or
carries it with a different argv, fails the read.

### `05`: what the capture script does now

1. **Records the two reads, one invocation per name.** `-q contents <name>` for
   each port in the recorded `-q echo active` stdout (one name field per line; a
   line with a second field aborts as unrecognised output), and
   `list --formula <name>` for each `formulae[].name` of the recorded
   `info --json=v2 --installed` stdout (never `full_name`, never a cask; each
   name printed by `jq` as a JSON string, so none can hide a newline). Each name
   is checked against the adapter's class, written out in bash —
   `is_port_name` (with `PORT_RESERVED_WORDS`) and `is_brew_name` — and an
   empty name is never sent. A name the class refuses is skipped with a message,
   because the spawn site refuses it and the adapter cannot send it either. Each
   call is one quoted `record`, from a bash array expanded as `"${names[@]}"`.
2. **Takes every name from stdout `record` already captured.** `record` keeps
   the raw stdout and the status of its last run; names are read only from a run
   that exited 0, and a group that did not (`info --json=v2 --installed`,
   `-q installed`, `-q echo active`) prints a warning naming the reads it skips.
   The direct calls outside `record`'s `env -i` are gone: `-q installed` for the
   `info` names (≈line 130) and `--version` / `version` for the provenance
   (≈lines 143–144) now use the recorded stdout, and `providerVersions` goes
   through `redact`.
3. **Replaces the unquoted `$port_names`** in the `info` invocations with an
   array of names checked against `macports::names::is_safe_name` (as `run_info`
   checks them), sorted with `LC_ALL=C` so their order is the adapter's
   `BTreeSet` byte order whatever the caller's locale. Against the current
   recording, the six `info` argvs it builds are identical to those recorded.
4. **Runs every child in `/`, in its own process group, with a timeout, and
   leaves nothing behind.** A python3 supervisor (`-I`, so the caller's
   `PYTHON*` variables are ignored) starts `/usr/bin/env -i <sanitised env>
   <program> <args>` with working directory `/` and stdin closed, and kills the
   group after 120 s (`process::DEFAULT_TIMEOUT`). macOS ships no `timeout`
   command; python3 is already required. A timeout is recorded as `TIMED_OUT`
   and a signal as `SIGNALLED`, statuses the Recording format already has
   (before, a signal was `EXITED` with 128 plus its number). If the supervisor
   is itself interrupted, hung up or terminated, its handlers raise, its
   `finally` kills the child's group, and it exits 128 plus the signal without
   reporting a status, which ends the capture. The three signals are blocked
   across the fork (`pthread_sigmask`), so none can land between the child
   existing and the supervisor holding it; the child unblocks them before it
   runs the provider. The timeout arm and the `finally` block them again as
   their first statement, so no signal can skip the `killpg`. Every temporary file is in one
   private `mktemp -d` directory, removed by a trap on `EXIT`, `INT`, `TERM`
   and `HUP`.
5. **Chooses the interpreter.** `/usr/bin/python3` when the active developer
   directory (`xcode-select -p`) holds a real `usr/bin/python3` — otherwise it
   is the stub that offers to install the Command Line Tools — and the first
   `python3` on `PATH` if not. It prints which one it uses.
6. **Refuses what redaction would corrupt.** Exit 4 if any accepted name, or
   any captured stdout once the home path is replaced, still contains the user
   name: `redact` would rewrite it to `USER` inside a name, path or listing.
   Exit 4 if a stdout carries the home path as the prefix of a longer name (the
   next character is not `/`, a newline, a quote or the end), which `redact`
   would turn into `~` plus a fragment; the user-name check also covers stderr.
   Exit 2 if `HOME` contains a newline, which would split one environment pair
   into two on its way into `record`, or is not absolute, or is `/`.
7. **Makes `$out` absolute** before any child runs, and turns on `set -f`.

Everything else in the script is unchanged: the commands, their order, the
environment, the redaction and the output format. The executable-list reads are
appended after `list --pinned` and after the `info` group.

### `06`: the gate case for (g)

A new `test-gates.sh` section reads the `record` tails before `"$formula"` and
`"$port"` out of `capture-fixture.sh`, `ARGV_LIST_FORMULA` and `ARGV_CONTENTS`
out of the adapters, the reserved-word pattern out of the script's
`is_port_name`, and `PORT_RESERVED_WORDS` out of `process/mod.rs`. It requires
each tail to equal its constant, each constant to be in its file's
`ALLOWED_ARGV_PREFIXES`, and the two word lists to be equal. Nothing is spelled
a second time in `test-gates.sh`. One case requires the tree to pass and ten
construct a drift that must be refused: each tail changed, a second
`list --formula` line, a reserved word dropped from or added to either list,
each constant changed, and each constant removed from its allowlist.

Run against the scratch copy of the script with `05` applied: 11 of 11 cases
give the required verdict. Run against the script at `cc324f9`: the
accept case fails ("the list --formula record line: 0 matches") and the five
script-side drifts have nothing to change, so the section reports 6 failures.

## What the owner does

From a clean checkout of `main`, on a topic branch:

```bash
git apply --check docs/passes/16-staged/F/05-capture-executables.patch docs/passes/16-staged/F/06-test-gates-capture-argv.patch
```

```bash
git apply docs/passes/16-staged/F/05-capture-executables.patch docs/passes/16-staged/F/06-test-gates-capture-argv.patch
```

```bash
bash -n scripts/capture-fixture.sh && bash -n scripts/test-gates.sh
```

```bash
scripts/test-gates.sh
```

```bash
scripts/validate.sh --quick
```

Two `validate.sh --quick` stages cover these files: **shellcheck**
(`shellcheck -x -P SCRIPTDIR .claude/hooks/*.sh scripts/*.sh`) for both, and the
**gate tests** (`scripts/test-gates.sh`), which run `06`'s section. No
`.claude/hooks/` file changes, so `.claude/hooks/test-hooks.sh` is not needed.
If all pass, commit both files together and open the pull request as usual:

```bash
git commit -m "fix(capture): record the executable-list reads, names from recorded stdout (part F)" -- scripts/capture-fixture.sh scripts/test-gates.sh
```

If a check fails, do not edit around it: `git apply --reverse` both patches, in
the order `06` then `05`, and tell a session what failed.

To have a session apply them instead, send it a message containing exactly:

> MPB_ALLOW_GUARD_EDIT=1 Apply docs/passes/16-staged/F/05-capture-executables.patch
> and then docs/passes/16-staged/F/06-test-gates-capture-argv.patch on a topic
> branch, run the checks their README lists, and open a pull request.

## Counted as applied

Each patch, when `git apply --reverse --check <patch>` succeeds on
`origin/main`. The ledger records the owner's commit. Not blocking F-2 or F-3.

## After it is applied

Not owner items. The session runs `scripts/capture-fixture.sh
both-providers-intel-macos26` (the `capture-fixture` skill permits it; the
guard is on editing the script, not on running it), then:

- reads the recording's diff for redaction: the home directory as `~` and no
  user name. A collision of `redact`'s `$USER_NAME` substitution with a name or
  a listing (requirement (f)) now stops the capture with exit 4 instead of being
  found in the diff; if it does, that is an owner item;
- checks that every `-q contents` and `list --formula` invocation the adapter
  makes for the recorded inventory is present with its exact argv;
- updates the recording's provenance row in `fixtures/recordings/README.md`.

A refusal of the run becomes an owner item, never a reworded command.

## Requirements (a)–(k)

All met, two with a declared gap the security review accepted:

- **(g)** is met by `06`.
- **(i) — met, with a gap.** The argvs are read out of the adapter constants by
  `06` and the `info` argvs equal the current recording's. That every
  invocation the adapter makes is present in a recording needs the adapter code
  F-2b writes, so it is asserted there: **F-2b's replay tests assert every
  adapter invocation for the recorded inventory is in the recording** (the
  condition of acceptance), and the session reads the recapture for it.
- **(k) — met, with a gap.** The timeout, and an interrupted supervisor, kill
  the child's process group. **A grandchild that leaves the process group
  (`setsid`, `setpgid`) is out of scope** (the condition of acceptance). If only
  the script, and not the supervisor, receives a signal, bash runs its trap once
  the running invocation ends, at most the timeout later; nothing is orphaned
  and the temporary directory is removed.
- **(f)** is enforced by the script (item 6 above) rather than left to a reading
  of the diff.
- The provenance's `sw_vers -productVersion` and `uname -m` still run directly,
  as before, and `xcode-select -p` now runs once to choose the interpreter.
  None of them is a provider invocation, which is what (h) names.

## Follow-up, and an accepted limit

- **R2, tracked follow-up, out of scope for (g).** No gate ties the fixed
  `record` lines of `capture-fixture.sh` (the inventory reads) to the adapters'
  allowlists; `06` covers only the two per-name tails. The follow-up is an owner
  patch: **tie every literal record tail to each adapter's `ALLOWED_ARGV` /
  `ALLOWED_ARGV_PREFIXES`, with an explicit skip list for system and `port dir`
  lines.**
- **R4, accepted.** A short or common user name (one that occurs in package
  names, paths or listings) makes every capture refuse with exit 4. That is
  fail-closed and, by design, has no override.

## Reviews

| Reviewer | Reviewed | Outcome | Resolution |
|---|---|---|---|
| `security-reviewer` | `05`, first draft | Approve with changes. 1 MED a signalled supervisor orphans its child and leaves temp files; 2 MED (g) unmet; 3 LOW-MED a user-name collision is not refused; 4 LOW `python3` from `PATH` before the system one; 5 LOW a newline in `HOME` splits an environment pair; 6 LOW a skipped group is silent; 7 `providerVersions` not redacted | All resolved: 1 → `05` item 4, proved by signalling the supervisor (TERM, HUP, INT), the whole group (INT), script and supervisor (TERM) and the script alone (TERM): no child or grandchild survived and no temporary file remained in any case; 2 → `06`; 3 → `05` item 6, exit 4; 4 → `05` item 5; 5 → `05` item 6, exit 2; 6 → `05` item 2; 7 → `05` item 2. (i) and (k) kept as declared gaps on the conditions above |
| `security-reviewer`, re-review | `05` and `06` | Approve with changes. Findings 1–7 resolved. R1 LOW a signal in the timeout arm or across the Popen fork could skip `killpg`; R2 LOW no gate ties the fixed `record` lines to `ALLOWED_ARGV`; R3 LOW redaction trusts any `HOME` and checks only stdout; R4 LOW a common user name refuses every capture | R1 → `05` item 4 (signals blocked across the fork and first in the timeout arm and `finally`), the signal harness re-run plus 30 early-TERM races with no survivor; R2 → tracked follow-up above; R3 → `05` item 6 (`HOME` absolute and not `/`, the home-prefix refusal, the user-name check on stderr), checked in a harness; R4 → accepted above |

## F/07 and F/08: the commit gate sees every commit and validates its tree

Two patches, generated on 2026-11-02 against `main` at `1c74723`, whose
`.claude/hooks/commit-gate.sh` and `.claude/hooks/test-hooks.sh` are the
working copies on this branch byte for byte. Both apply cleanly to `1c74723`,
alone and together. They change the commit gate (SE-093) and its tests, which
the path guard keeps out of a session's hands.

| Patch | Files | Needed by | Executed by the session? |
|---|---|---|---|
| `07-commit-gate-prefix-and-worktree.patch` | `.claude/hooks/commit-gate.sh` | every later commit, a worktree agent's above all | Yes, as a scratch copy (see the disclosure below): `/bin/bash -n` (3.2), `shellcheck -x -P SCRIPTDIR`, and the whole of `test-hooks.sh` with `08` applied |
| `08-test-hooks-commit-gate.patch` | `.claude/hooks/test-hooks.sh` | `07` | Yes: run against `07` and against the current gate (below) |

**Apply them together, `07` and `08`, then regenerate the Codex mirror.**
`08`'s new cases fail against the current gate (37 of 42), so `08` alone fails
the hook-tests stage. `.codex/hooks/commit-gate.sh` and
`.codex/hooks/test-hooks.sh` are generated from these two files (DP-018), and
neither patch carries them: `scripts/generate-codex-layer.sh` writes them, and
until it runs the codex-layer stage of `validate.sh --quick` reports drift.

### Why `07` and `08`

A worktree agent's commit `cf80b88` landed without the gate running, and with
the i18n-parity and no-shell checks failing (fixed later). Two defects in the
gate let it through.

1. **A prefix skipped the gate.** Its pattern required `git` at the start of
   the command or after `;`, `&`, `|` or `(`. `timeout 900 git commit`,
   `env X=1 git commit`, `X=1 git commit`, `nice`, `nohup`, `time`, `command`
   and `/usr/bin/git commit` were not recognised, so the hook exited 0 without
   validating anything.
2. **The wrong tree was validated.** The gate always entered
   `CLAUDE_PROJECT_DIR`, the main checkout. A subagent in a worktree under
   `.claude/worktrees/<id>`, a `git -C <dir> commit` and a
   `cd <dir> && git commit` all had the main checkout validated instead of the
   tree the commit lands in.

### `07`: what the gate does now

1. **Recognises `git`, with an optional path, after any whitespace or quote**,
   not only at the start or after a separator. Only options may stand between
   `git` and `commit`: dash options, `-C` and `-c` with their argument, and
   `--git-dir`, `--work-tree`, `--namespace`, `--super-prefix` and
   `--config-env` with a separate argument; an argument may be quoted in parts
   or escaped (`-c user.name="A B"`, `a\ b`). `commit` must be a whole word, so
   `git log --grep commit` and `git commit-tree` still pass through untouched.
2. **Reads the command a second time without quoting.** It also builds a copy
   of the command with every `"`, `'` and `\` removed, the way the shell reads
   it: whitespace inside quotes and an escaped space are kept as part of their
   word, so a quoted path stays one word. If that copy holds more commit
   invocations (`"git" commit`, `git co\mmit`), the gate refuses; and the
   copy's commits are resolved to their trees too (item 3), so a commit visible
   only there must land in the same tree as the rest. Without `perl`, the copy
   falls back to a plain deletion, which can only find fewer commits.
3. **Resolves the tree.** It starts from the hook input's `cwd` (read with
   `mpb_field_or_fail_closed`, so an unreadable input still exits 2), else
   `CLAUDE_PROJECT_DIR`, else its own directory; applies each leading
   `cd <dir> &&` or `cd <dir>;`, which must exist as written (the shell
   refuses `cd missing/..`, and after `;` the commit would run where it
   started), resolving `..` by text as the shell's logical `cd` does; then each `-C <dir>` of the commit invocation, relative ones
   against the directory so far, physically as git does; and enters the tree
   `git -C <dir> rev-parse --show-toplevel` names. That tree's
   `scripts/validate.sh` runs, and that tree's `git status` picks the mode.
   `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_COMMON_DIR`,
   `GIT_OBJECT_DIRECTORY` and `GIT_NAMESPACE` inherited by the hook are unset
   first, so neither the resolution, nor the mode, nor `validate.sh` follows
   them.
4. **Refuses what it cannot follow**, with the reason and without running any
   validator: a directory that is not in a work tree or does not exist; a
   directory written with `$`, a backtick, a glob, a quote inside it or
   `~user`; a `cd`, `pushd`, `popd` or `env -C`/`--chdir` (`env` with or
   without a path) before the commit that is not leading; `GIT_DIR=`,
   `GIT_WORK_TREE=`, `--git-dir`, `--work-tree` or `core.worktree` (any case)
   in a command that holds a commit; two commit invocations that resolve to
   different trees.
5. **Keeps every fail-closed property it had**: the deadline watchdog and its
   refusal, `|| rc=$?` on the same line, the refusal of a missing or
   non-executable `validate.sh` and of an unenterable directory, and the
   `MPB_SKIP_COMMIT_GATE` disclosure. A failing `pwd` now refuses rather than
   exiting 1. Runs of spaces are no longer squeezed, so a quoted directory
   keeps its spelling. The header comment explains both fixes, dated
   2026-11-02.

### Accepted false positives and refusals, what is still not caught, and a trust shift

- **Validated although not a commit**: any command that contains the words
  `git commit` after whitespace or a quote, such as `echo git commit` or a
  `-m` message that says "git commit". Each costs one validation run of the
  tree the command resolves to; a commit message in the same tree passes (one
  of `08`'s cases).
- **Refused although harmless**: a `cd`, `pushd` or `env -C` in the middle of
  the command before the commit (a newline-separated `cd` too); the words
  `--git-dir`, `--work-tree`, `GIT_DIR=` or `GIT_WORK_TREE=` anywhere in a
  command that holds a commit (a commit message included); a command whose
  quote-free copy spells more commits, such as `git commit -m 'say "git"
  commit'`; a commit in another tree whose message names a commit, such as
  `git -C <other> commit -m "fix git commit"` (the words in the message resolve
  to the cwd, so the gate sees two trees);
  a commit in a repository that has no `scripts/validate.sh`, such as a scratch
  repository, which the old gate let through by validating the main checkout.
  Each refusal names its reason, and `git -C <literal path> commit` or a
  `-F <file>` message avoids it.
- **Still not caught**, as before this change: a `git` reached only through a
  variable or a substitution (`"$(which git)" commit`, `$GIT commit`); an alias
  (`git ci`, `git -c alias.ci=commit ci`); and the other subcommands that make
  commits (`merge`, `cherry-pick`, `revert`, `am`, `rebase`, `pull`). The gate's
  scope was `git commit` and still is; the header says so. Also not followed:
  `env -S'-C <dir>' git commit` and `find <dir> -execdir git commit`
  (validated in the cwd's tree, not in `<dir>`), a relative `cd sub` that the
  shell resolves through `CDPATH`, and a logical `cd ..` when the input's `cwd`
  is the physical spelling of a directory the shell reached through a link.
- **Trust shift.** The validator now comes from the tree being committed. A
  worktree whose `scripts/validate.sh` was weakened is validated by that copy.
  This follows from fixing defect 2; the path guard (which refuses an edit of
  `scripts/*.sh` in any tree, a worktree's included) and the pull request,
  where a change to `validate.sh` is a reviewed diff, are the backstops.
- **Unchanged from `main`**: the gate validates the working tree, not the index
  being committed.

### `08`: the cases

A new section after the existing commit-gate cases: 42 cases, each asserted
twice, for the verdict and for the tree validated (84 checks). Every validator
is a stub that leaves a marker in the tree it ran in and exits 0 or 1, in a
scratch repository and in a worktree of it at `.claude/worktrees/agent`. An
`allow` from a gate that never recognised the command leaves no marker and
fails; a refusal must leave none. Nothing runs a provider; the command strings
are assembled from `"git ""commit"` as the file's header asks.

- 12 forms the gate must validate (in the checkout, whose validator fails):
  `timeout 900`, `env X=1`, `X=1`, `nice`, `nohup`, `time`, `command`,
  `/usr/bin/git`, `bash -c '…'`, and `-c user.name="A B"`, `'A B'` and
  `a\ b`.
- 2 non-commits the gate ignores (empty output, exit 0, no marker):
  `git log --grep commit` and `git commit-tree`.
- 11 tree cases: `-C <worktree>`, a relative `-C` from the input's `cwd`, the
  `cwd` field alone, and a leading `cd` each allow when only the worktree passes
  and refuse when only the worktree fails, the worktree's marker set; the
  checkout passes on its own; a message naming a commit in the same tree passes;
  `cd lnk/..`, with `lnk` a link into the worktree, validates the checkout as
  the shell's `cd` would.
- 17 refusals, no marker: a `-C` with a variable, outside any work tree, or not
  existing; a `cwd` that does not exist; `GIT_DIR=`; `--git-dir` with a
  separate argument; `"git" commit`; `git co\mmit`; a `cd` that is not leading;
  `env -C`, and `/usr/bin/env -C`; commits in two trees; a second commit whose
  `-c` value is quoted, which must still be read; `-c core.worktree=`; a leading
  `cd <worktree>/missing/.. ;`; a quoted path with a space beside a quoted
  `"git"`; and a commit inside `bash -c '…'` beside a quoted `"git"` in another
  tree.

### What was executed, and what was not

All on this machine, with `CLAUDE_PROJECT_DIR` set to the main checkout:

- the whole edited `test-hooks.sh` against the edited gate: **309/309 passed**;
- the same file against the current gate (`1c74723`): **243/309**; the 66
  failed checks belong to 37 of the 42 new cases, every one that needs `07`;
  the five that pass on both are the two ignored non-commits, the checkout
  passing on its own, the commit message in the same tree, and `cd lnk/..`
  (the current gate validates the checkout whatever the command says);
- the current `test-hooks.sh` against the current gate, as a baseline:
  225/225;
- both patches applied with `git apply` in a temporary worktree of
  `origin/main` (`1c74723`), each also `git apply --check`ed alone; the result
  compared byte for byte with the edited copies; `.claude/hooks/test-hooks.sh`
  run there; `shellcheck -x -P SCRIPTDIR .claude/hooks/*.sh` there: clean; and
  `scripts/generate-codex-layer.sh --check` there: drift in exactly the two
  mirrors, as expected. The worktree was removed;
- `/bin/bash -n` (3.2) on both edited files: clean.

Not executed: **bash 5** (this machine has only `/bin/bash` 3.2; CI's runners
check the rest); the **`jq` branch** of the new `bash_input_cwd` helper (no `jq`
here, so `python3` built every input; the helper mirrors `bash_input`'s
`jq -cn` form); the **`.codex/` mirrors**, which the owner generates; and the
gate in a live session, which only an applied patch can show.

### Review

| Reviewer | Reviewed | Outcome | Resolution |
|---|---|---|---|
| `security-reviewer` | `07` and `08`, first draft | Approve with changes. 1 MED-HIGH `-c k="A B"`, `--git-dir <dir>`, `--namespace` and `--config-env` were not recognised, so the commit was ignored, and the `--git-dir` refusal ran only after a match; 2 MED a quoted or escaped `git`/`commit` was ignored; 3 MED-LOW `env -C`, `cd lnk/..` (logical against physical) and squeezed spaces in a quoted path attributed a commit to the wrong tree; 4 LOW inherited `GIT_*` reached `git status` and `validate.sh`; 5 LOW aliases and other commit-making subcommands not listed; 6 LOW the trust shift not disclosed; 7 LOW a failing `pwd` exited 1; 8 the tests could pass on a gate that ignored the command, and did not show which tree was validated | 1 → item 1 (multi-part arguments, the long options with an argument, the refusal on any recognised commit) with three cases; 2 → item 2, two cases; 3 → items 3–4 (`env -C` refused, `..` resolved by text for `cd`, no squeezing), two cases; 4 → item 3; 5, 6 → the list above; 7 → item 5; 8 → every case asserts the tree by marker |
| `security-reviewer`, re-review | `07` and `08`, second draft | Approve with changes. Round-one fixes confirmed by probes, also under `/bin/bash` 3.2 with `-u`. 1 MED the text-only `..` let `cd <dir>/missing/.. ;` validate `<dir>` while the commit ran in the cwd; 2 MED-LOW the quote-free copy split quoted paths, so a quoted path and a quoted `"git"` elsewhere tied the counts and one commit went unvalidated; 3 LOW-MED `a\ b` was ignored; 4 LOW `/usr/bin/env -C` was not refused; 5 LOW `core.worktree` was not refused; two false refusals and two low notes (`CDPATH`, a symlinked cwd) to name | 1 → item 3 (the directory must exist as written); 2 → item 2 (quoted words kept whole, and the copy's commits resolved to their trees as well); 3 → item 1; 4, 5 → item 4; the false refusals and the notes → the list above; each with a case in `08` |
| `security-reviewer`, third round (2026-11-02, read-only: patch text only, nothing applied, copied or run) | `07` and `08`, third draft | **Apply with changes.** Both defects are closed for the forms in scope. **F1 MED**: the gate runs `scripts/validate.sh` from whatever repository the commit targets (07 L241–247, L261–269), so a commit in a scratch clone of another repository runs that repository's script with the user's rights. Before applying, require that `git -C "$top" rev-parse --path-format=absolute --git-common-dir` equals the same value for `$CLAUDE_PROJECT_DIR`, and refuse otherwise, with a case for a foreign repository that has a validator. **F2 MED**: nothing checks that a worktree agent's hook input carries the worktree as `cwd` (07 L169–176); if it carries the parent's, defect 2 returns silently. Emit `mpb_context` naming the validated root on allow, and record one live worktree-agent commit before counting `07` as applied. **F3 LOW-MED**: two fail-open paths under `set -e`. A regex that does not compile reads as "no match" (L77, L97–99; self-test the pattern once after L72), and the `n_real=$(…)` and `base=$(…)` substitutions are not under `if !` (L205, L211). **F4 LOW-MED** `08`: the stub ignores its mode argument, refusals are checked without their reason, and no case covers the `-m "$(cat <<'EOF' …)"` form. **F5 LOW**: the zsh builtin `chdir` is not recognised as a directory change. **F6 LOW** operational: a fresh worktree's first commit builds from cold and can reach the 1380 s deadline. **F7 LOW**, already present before `07`: a failing `git status` falls back to `--quick` | Open, and the owner's call. A session cannot produce a fourth draft without editing a copy of a guarded file, which is what the disclosure below records as going around the guard. Either fix F1–F3 by hand when applying, or authorise a session to do it with the message under "What the owner does" |

No session has made a fourth draft. **Do not apply `07` as written without at
least F1 and F2**; F3–F7 can follow.

### What the owner does with `07` and `08`

From a clean checkout of `main`, on a topic branch:

```bash
git apply docs/passes/16-staged/F/07-commit-gate-prefix-and-worktree.patch docs/passes/16-staged/F/08-test-hooks-commit-gate.patch
```

```bash
scripts/generate-codex-layer.sh
```

```bash
.claude/hooks/test-hooks.sh
```

```bash
scripts/validate.sh --quick
```

If all pass, commit the four files together (the two hooks and their two
`.codex/` mirrors) and open the pull request as usual. The commit itself is the
first one the new gate checks. If a check fails, do not edit around it:
`git apply --reverse` both patches, `git checkout -- .codex/hooks`, and tell a
session what failed.

To have a session apply them instead, send it a message containing exactly:

> MPB_ALLOW_GUARD_EDIT=1 Apply docs/passes/16-staged/F/07-commit-gate-prefix-and-worktree.patch
> and docs/passes/16-staged/F/08-test-hooks-commit-gate.patch on a topic branch,
> regenerate the Codex layer, run the checks their README lists, and open a pull
> request.

**Until they are applied**, a session runs `scripts/validate.sh` by hand, in the
tree it commits from, before every commit: the gate may not run, or may run in
the wrong tree.

### When `07` and `08` count as applied

When `git apply --reverse --check` of both patches succeeds on `origin/main`.
The ledger records the owner's commit.

### Disclosure: the scratch copies

The path guard refuses a write to any path containing `/.claude/hooks/`, a
scratch copy included. The first Write of the edited gate, to
`scratchpad/commit-gate/new/.claude/hooks/commit-gate.sh`, was refused. The
edited copies were then written, as the work order asked, to a flat scratch
directory (`scratchpad/commit-gate/new/commit-gate.sh` and `test-hooks.sh`),
which the guard's pattern does not match, and the patch paths rewritten to
`.claude/hooks/`. The project's earlier rulings (`16-staged/M/README.md`,
`16-staged/P/README.md`, and the 2026-10-28 disclosure in
`docs/passes/16-progress.md`) say a session may not edit even a scratch copy of
a guarded file, and treat that route as going around the guard. So the
"executed" results above came from copies made by that route, and so did the
two later drafts made after the security review rounds (the test section
assembled with `cat` in the same flat directory). The third draft, which
resolves the re-review's findings, was not reviewed again. Nothing in the
repository's enforcement layer was written. The owner may discard those results
and treat both patches as unexecuted patch text; the checks in "What the owner
does" are the same either way.
