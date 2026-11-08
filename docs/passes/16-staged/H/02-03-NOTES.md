# Staged for the owner: work order 16 part H, patches 02 and 03

Both patches are generated against `origin/main` at `54b56eb`. They change
guarded scripts (`scripts/*.sh`), so the session did not edit them in the main
tree: the edits were made on full copies (`work/`, with the untouched originals
in `orig/`) and the patches produced with `git diff --no-index`, relabelled to
the repository paths. The path guard refused an Edit under `scripts/` even in
the scratch worktree (`h02/`), so this fallback was used as instructed.

| Patch | Files |
|---|---|
| `02-capture-ownership-reads.patch` | `scripts/capture-fixture.sh`, `scripts/test-gates.sh` |
| `03-release-workflow-pipe.patch` | `scripts/check-release-workflow.sh` |

They are independent; each applies alone and both apply together.

## 02: the capture records part H's MacPorts owner reads

H-2's ownership check (`crates/adapters/src/macports/destination.rs`) sends two
reads no recording holds:

* `owner_read`: `ARGV_PROVIDES` (`["-q", "provides"]`, `macports/mod.rs`) plus
  one path, the placed entry `<prefix>/bin/<leaf>` or `<prefix>/sbin/<leaf>`
  (`destination_reads::placed_entry`), and the entry's canonical target when it
  differs; the spawn site admits it through `ALLOWED_ARGV_PREFIXES`.
* `installed_rows`: `ARGV_INSTALLED` (`["-q", "installed"]`) plus one name of
  `process::is_port_name`'s class, admitted by the
  `is_head_and_one_name(argv, ARGV_INSTALLED, crate::process::is_port_name)`
  clause of `is_allowed_argv`.

Homebrew's side needs nothing new: `--cellar` and `info --json=v2 --installed`
are already recorded.

### `capture-fixture.sh`

1. **Keeps the executables each `-q contents` listing names.** Only from a
   `-q contents` run that exited 0 and whose every line is two spaces and an
   absolute path (`listing::parse`'s shape; an empty stdout is refused there
   too). A listing in another shape gives the adapter no executable, so the
   script takes none from it and prints a warning. An executable is a direct
   child of `<prefix>/bin` or `<prefix>/sbin`, compared as a whole parent
   (`listing::direct_children`), with `<prefix>` taken from the `port`
   executable the script already resolves (`/opt/local`), within
   `MAX_PATH_LEN`, and whose leaf passes `ExecutableName::parse` narrowed to
   printable ASCII (no locale decides what a control or format character is;
   no `/`, no `\`, not `.`/`..`, no leading `-`, at most 255 bytes). A leaf
   outside that class is skipped with a message.
2. **Records `-q provides <path>`, one path per invocation**, for those paths,
   deduplicated and sorted with `LC_ALL=C`. A path is an argument, and
   arguments are never redacted, so a path containing the home path or the user
   name stops the capture with exit 4 (the same exit as F/05's refusals).
3. **Records `-q installed <name>`, one name per invocation**, for every name
   of the recorded `-q installed` stdout (already checked against
   `is_safe_name` and the user name by the existing `info` block) that also
   passes `is_port_name`; a name the spawn site refuses is skipped with a
   message. Only when that `-q installed` run exited 0, as for `info`.
4. Each call is one quoted `record` from an array expanded as `"${a[@]}"`,
   guarded by a length test (bash 3.2 and `set -u`), after `-q contents`.
   Nothing else in the script changes.

Not recorded: the second owner read on an entry's **canonical target** when the
entry is a link. The listing does not say where a link resolves, and resolving
it would be a filesystem read the capture does not otherwise make. Under a
recording, an executable that is a link would find its first `provides` and not
its second. Left for the owner to decide (it is one `realpath` per path if
wanted).

The bounds are F/05's: every name and path from recorded stdout, checked
against the adapter's class first; every child under the supervisor's 120 s
timeout, in `/`, in its own process group. There is no count cap (F/05 has
none either); on this machine the live trial below made 84 `provides` reads for
18 ports.

### `test-gates.sh`

The F/06 section is extended rather than duplicated. `cap_check` now takes a
fifth file, `macports/destination.rs`, and for each of the two reads requires:
the script's `record` tail (the line ending `"$owned_path"` or
`"$installed_port"`) equals the constant read out of `macports/mod.rs`; the
constant is the one the adapter function (`owner_read`, `installed_rows`) builds
its argv from, with exactly one `argv.push(`; and the spawn site admits it with
one appended element (`ARGV_PROVIDES` in `ALLOWED_ARGV_PREFIXES`; the
`ARGV_INSTALLED` clause present once in `is_allowed_argv`). Nothing is spelled
twice in `test-gates.sh`. Ten new drift cases: each tail changed, a second
`provides` line, the `installed <name>` line dropped, each constant changed,
`ARGV_PROVIDES` out of the allowlist, the `ARGV_INSTALLED` clause removed,
`owner_read` building from another constant, `installed_rows` pushing twice.

## 03: no early-exiting reader behind a pipe in `check-release-workflow.sh`

Under `set -o pipefail`, `printf '%s\n' "$x" | grep -q ...` fails when `grep -q`
exits on its first match while `printf` still has output to write: `printf`
gets SIGPIPE/EPIPE (`printf: write error: Broken pipe`) and the pipeline status
is non-zero, so `if ! ...` reports a FAIL for a workflow that holds. Three such
pipelines, all now fed by a here-string, which has the same trailing newline as
`printf '%s\n'`:

* line 137, rule 5: `grep -Eqx '    tags: ...' <<<"$push_block"`;
* line 171, rule 7/8: `head -1 <<<"$job" | cut -d: -f1` (`head` is the same
  early reader; the old form sat in `first="$(...)"` under `set -e`, so the
  failure would have ended the script);
* line 229, rule 11: `grep -qE "...needs: gate..." <<<"$tagged"` (the CI false
  FAIL).

A comment after `hits_of` states the rule. Every other pipeline in the script
ends in a reader that consumes its whole input (`grep -E`, `grep -c`, `grep -v`,
`awk`, `sed`, `tail`, `sort`), so it cannot see SIGPIPE.

## Checks run, and results

Bash: only macOS `/bin/bash` 3.2.57 is on this machine (no bash 5 at
`/opt/homebrew/bin`, `/usr/local/bin` or on `PATH`), so the bash 5 syntax check
could not be run.

**Patch 02**

* `/bin/bash -n` on both edited copies: ok. `shellcheck -x -P SCRIPTDIR`: clean.
* The new parsing, extracted from the edited copy into a harness under
  `set -euo pipefail` and `set -f` on bash 3.2 (`harness/funcs.sh`,
  `harness/body.sh`, `harness/run.sh`), against `port -q contents` stdout for
  this machine's 18 active ports (read-only, captured into `sample/`): 84
  `provides` paths, equal to `grep -cE '^  /opt/local/s?bin/[^/]+$'` over the
  same files; 18 `installed <name>` reads. Edge cases (`harness/edge/`):
  `bin/sub/deep`, `binx/`, `share/` ignored; `-dash`, a backslash and a
  non-ASCII leaf skipped with a message; a leaf with a space kept as one
  argument; duplicates once; a listing with a non-path line, an empty listing
  and a failed `-q contents` give no paths, with warnings; a reserved word
  refused for `installed`; a `HOME` that is a prefix of the path, and a user
  name inside a path, stop with exit 4.
* The F/06 section of the edited `test-gates.sh`, extracted
  (`harness/gate-section.sh`) and run on bash 3.2 against two trees that differ
  only in `capture-fixture.sh`: with the **new** script, 21 of 21 cases give
  the required verdict; with the **old** (`54b56eb`) script, 5 failures — the
  accept case ("the -q provides record line: 0 matches") and the four
  script-side drifts, which have nothing to change. So the gate and the script
  must land together, as F/05 and F/06 did.
* **Live run, read-only, Homebrew excluded.** I read every invocation the
  edited script makes: `sw_vers`, `uname`, `sysctl -n`, `xcode-select -p`,
  `brew --version/--prefix/--cellar/--caskroom/--cache/config`,
  `info --json=v2 --installed`, `list --pinned`, `list --formula <f>`, and
  `port version`, `-q installed [<name>]`, `-q echo ...`, `dir`, `-q info ...`,
  `-q contents <p>`, `-q provides <path>` — all reads. The full script against
  the live providers stopped in the **Homebrew** part, before any MacPorts
  read, with `/usr/bin/jq: Argument list too long` (see "Found, not fixed"). A
  copy with Homebrew's candidate path pointed at a nonexistent file
  (`harness/capture-nobrew.sh`, a two-line difference) completed:
  147 invocations, every `port` read `EXITED 0` except `port dir` (exit 1, as
  before); 84 `-q provides` with one port name each (e.g. `aclocal` →
  `automake`) and 19 `-q installed` (one bare, 18 with a name, e.g.
  `  autoconf @2.73_0 (active)`). Output in `live/h02-trial.json`, scratch only,
  not a fixture.
* `git apply --check` of `02` in the worktree at `54b56eb`: ok; applied, the
  three files are byte-identical to the edited copies; the full
  `scripts/test-gates.sh` run there is reported below; `git apply --reverse
  --check` after applying: see below.

**Patch 03**

* `/bin/bash -n`: ok. `shellcheck -x -P SCRIPTDIR`: clean.
* Reproduction (`harness/repro-pipe.sh`, `harness/repro-pipe-n.sh`): the old
  and new forms of the rule 11 test, 300 runs each, match on the first line of
  a larger input. On macOS the threshold is the 64 KiB pipe buffer: 4.5 KB,
  14 KB and 47 KB inputs give 0 false failures for both forms; 71 KB, 96 KB,
  145 KB and 998 KB give **300/300 false failures for the old form and 0/300
  for the new**. (Linux CI, where bash's stdio writes in smaller chunks and the
  reader can win the race, fails below that threshold; the CI log shows it.)
* The old and new scripts against the tree (worktree at `54b56eb`): both pass
  ("release workflow rules hold", exit 0).
* A copy of `.github/workflows` with `needs: gate` removed from `claude.yml`
  (`badwf/`, `MPB_WORKFLOW_DIR`): both fail with the `needs: gate` message,
  exit 1.
* A copy with `claude.yml` padded past 64 KiB by a 3000-line `run:` block in
  the `claude` job (`padwf/`, 235 KB): the **old** script FAILs 10/10 runs with
  the false "does not 'needs: gate'"; the **new** one passes 10/10.
* A copy with `tags: ["x*"]` in `release.yml` (`badrel/`): both fail rule 5.
  `.github/` itself was not edited.
* `git apply --check` and, after applying, `--reverse --check`: see below.

**Applying in the worktree (`h02/`, detached at `54b56eb`)**

* `git apply --check` of `02`, of `03`, and of both together: ok.
* Both applied: the three files byte-identical to `work/`. The full
  `scripts/test-gates.sh` on bash 3.2 there: exit 0, "every constructed case
  produced the required verdict", 341 `ok`, no `FAIL`/`MISS` (log:
  `test-gates-full.log`), including its existing
  `check-release-workflow.sh: each rule can say no` section against the new
  script. `scripts/check-release-workflow.sh` there: rules hold, exit 0.
* `git apply --reverse --check` of `02` and of `03` after applying: ok; reversed,
  the worktree is clean again. Each patch also applied alone and
  reverse-checked alone: ok.
* `validate.sh` was not run on the patched worktree (its default mode builds
  and tests the Rust tree; neither patch touches it). The owner's commit runs
  it through `commit-gate.sh`.

## Found, not fixed

* **`observed` returns `NotObserved` under replay.**
  `MacPortsAdapter::observed` (`destination.rs`, line 65) returns early when
  `!self.filesystem.is_this_machine() || self.runner.is_replay()`, so a
  `--replay` run never reaches `owner_read` or `installed_rows` today, whatever
  the recording holds. The new invocations serve contract tests that drive the
  adapter over a recorded runner with a this-machine filesystem scope, or a
  later change to that guard; worth a line in the H plan.
* **F/05's `record` cannot keep a stdout larger than `ARG_MAX`.** It passes
  each redacted stdout to `jq` as `--arg o "..."`. On this machine
  `brew list --formula codeburn` prints 1,113,822 bytes (ARG_MAX is 1,048,576),
  and the capture dies with `jq: Argument list too long` (exit 126). Not part
  of either patch; the fix is to hand `jq` the stdout and stderr through files
  (`--rawfile`) instead of arguments. Until then, a recapture on a machine with
  a large formula listing fails before the MacPorts reads.
* The bash 5 `bash -n` could not be run (none installed).

## Other guarded scripts with the same shape

`printf '%s' ... | grep -q` (or `| head`) under `pipefail`, not patched — the
pattern is the same, but the inputs differ and there are many sites, so each is
the owner's call. The risk is only real when the input can exceed a pipe
buffer:

* `scripts/check-no-secrets.sh` 106, 112: `$untracked`, `$ignored` (file lists;
  the likeliest to grow past 64 KiB).
* `scripts/build-release.sh` 139, 400–402, 429, 435–437, 444, 455–457, 596,
  649, 716–717, 721, 729, 733 (`codesign`/`spctl`/`pkgutil` output, `--version`
  text, `hdiutil attach` output; small today), and 777 (`ld -v | head -1`,
  inside `echo "...$(...)"`, harmless).
* `scripts/check-action-pins.sh` 71, 77 (one SHA, the repo allowlist; tiny).
* `scripts/check-deployment-target.sh` 56 (`$out` of a linker/loader check).
* `scripts/check-no-shell.sh` 83 (`$allowlist`; small).
* `scripts/test-gates.sh` 283, 468–476, 994, 1133–1141, 1361–1368, 1433–1441,
  1457, 1785, 1818, 1842–1846, 1950 (captured script output in `$out`/`$diag`,
  and `head`/`grep -m1` in messages). Line 501 (`yes | head -4000` inside
  `$(...)`) is deliberate and runs without `set -e`.
* `.claude/hooks/commit-gate.sh` 34, `guard-provider-mutation.sh` 54–189,
  `test-hooks.sh` 51–303: one command line or one hook reply each, far below a
  buffer; and `.claude/` is out of this task's scope.

## Files

* `02-capture-ownership-reads.patch`, `03-release-workflow-pipe.patch`
* `orig/`, `work/`: the three scripts before and after.
* `harness/`, `sample/`, `badwf/`, `padwf/`, `badrel/`, `live/`: the checks
  above.
* The scratch worktree `h02/` is left in place (detached at `54b56eb`).
