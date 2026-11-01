<!-- SPDX-License-Identifier: Apache-2.0 -->

# Staged for the owner: the capture of the executable-list reads (work order 16, part F)

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
