# H/01 — curated probe admission: build notes (review round 1 applied)

Built in a detached scratch worktree on `origin/main` = `54b56eb`
(`scratchpad/h01`, its own `target/`). Nothing applied to the main tree, nothing
committed.

## Artefacts

| File | What | sha256 |
|---|---|---|
| `01-curated-probe-admission.patch` | **The owner's patch.** `crates/adapters/src/process/` only: `mod.rs`, `system.rs`, `recording.rs`, new `probe.rs` (2298 lines, about 60% tests) | `40540c2e5527701bda65a3f0c7f4b90a5be5d89653f24edf95c26ad979ddc991` |
| `00-session-pr-run-lock-path.patch` | **Not the owner's.** A 6-line, unfrozen, pure-plumbing hunk in `crates/adapters/src/persistence/run_lock.rs`: `pub(crate) fn path(&self) -> &Path` on `RunLock`. It belongs in the session's own PR and **must be merged before H/01 is applied**: H/01 calls it (review item 5) and does not compile without it | `18bdb64dd4abd9223d4dc5d49349d8b761baf1be95ef5c4d80852362bedc877d` |

The round-0 patch (`7997598b…`) is superseded.

## Review round 1: what changed, item by item

1. **The sweep no longer deletes anything it did not make (security M1).**
   `sweep_leftovers(probes, euid)` first scans every entry under `probes`. An entry
   counts as a leftover only if its name is exactly `scratch_name`'s shape,
   `probe-<decimal pid>-<six lowercase hex>` (`is_scratch_name`), **and** `lstat`
   shows a directory owned by the euid at mode `0700`. If any entry fails either
   test, nothing at all is removed and the probe answers `PROBE_NOT_STARTED`.
   Only after the whole scan passes are the leftovers removed (`remove_dir_all`,
   which follows no link inside). Tests: `anything_else_under_probes_is_left_alone_and_refuses_the_probe`
   plants, one at a time, a file `notes.txt`, a directory `mine/inside`, a
   scratch-named directory at `0755`, a private (`0700`) directory with another name, and a
   scratch-named link. Each answers `PROBE_NOT_STARTED`, starts nothing, survives
   intact, and a conforming leftover beside it is not removed either.
   `only_a_scratch_shaped_leftover_is_swept_and_a_link_is_not_followed` shows a
   conforming leftover holding a link to an outside directory removed, outside
   kept. `is_scratch_name` is tested on the live `scratch_name()` and on 8 near-misses.
2. **Reachability.** `process/mod.rs` now carries
   `pub(crate) use probe::{run_curated_probe, ProbeOwner};` under
   `#[allow(unused_imports, reason = "...")]`. That allow is unconditional, not
   `cfg_attr(not(test))`, because the tests call the items in place, so the
   re-export is unused in a test build too. It also carries
   `pub use probe::{curated_probe_rows, CuratedProbeRow, ProbeRun};`.
   **`curated_probe_rows()` is `pub`**, because the agreement test lives in
   `crates/cli/tests`. It takes no parameter and returns
   `Result<Vec<CuratedProbeRow>, String>`, a copy of the parsed rows: provider,
   package, executable, argv, observed version, source, read on. Nothing admits
   against a `CuratedProbeRow`: `admit` reads its own private table. Tested:
   `the_shipped_table_parses_and_holds_only_closed_flags` asserts the copy equals
   the private rows, field by field, in order. H-3 needs no second hunk in a
   frozen file.
3. **The owner rule judges the file that is canonicalised, and the canonical
   path is floored under the prefix.** `ProbeOwner::entry_owner(package, leaf,
   placed)` now takes the `placed` path `admit` built and then canonicalises.
   Condition 8 adds `lies_under(&canonical, owner.prefix())`: equal to, or under
   by component, the prefix as written or canonicalised. Otherwise the answer is
   `PROBE_TARGET_NOT_THE_PACKAGES`, whatever the rule says. Tests:
   `a_probe_that_passes…` asserts the rule was asked of exactly
   `<prefix>/bin/<leaf>`. `condition_8_…` places a link to an admissible file
   outside the prefix and gets `PROBE_TARGET_NOT_THE_PACKAGES` with the rule saying
   yes.
4. **No euid parameter.** `admit` reads `crate::system::machine::effective_uid()`
   itself, after condition 1. `None` answers `PROBE_NOT_STARTED`. The uid is kept
   on `AdmittedProbe` for conditions 9, the `probes` check, the sweep and the
   scratch owner. **Test hook:** a `#[cfg(test)]` thread-local `TREAT_AS_ROOT`
   makes condition 2's `is_root` answer yes. In a non-test build `is_root` is
   `euid == 0`. So the test for condition 2 runs with the real uid everywhere
   else, and the mutant "drop condition 2" now reaches the runner (killed by
   `invariant_06_…`, which counts starts).
5. **The lock must be the state root's.** `lock_is_this_roots`:
   `lock.path() == state.root().join(run_lock::LOCK_FILE)`, else
   `PROBE_NOT_STARTED`. This needs `RunLock::path()`, which is the separate session-PR hunk
   above. Test: `the_lock_must_be_this_state_roots` holds a real lock on another
   state root and gets `PROBE_NOT_STARTED`, with nothing started.
6. **A recording runner refuses at condition 1, before anything is read or
   written.** New trait method `ProcessRunner::may_run_probes()`, default `false`
   and forwarded by `&T` and `Rc<T>`. `SystemRunner` answers `!self.recording`.
   Condition 1 is now
   `!is_this_machine || runner.is_replay() || !runner.may_run_probes()` →
   `NOT_OBSERVED`. `run_probe` keeps its own recording refusal as a second line.
   Test: `condition_1_a_recording_runner_touches_nothing_of_the_state_directory`
   puts a conforming leftover in `probes` first; after the recording-runner probe
   it is still there (no sweep, no scratch), and nothing was started or recorded.
7. **Condition 11 judges the interpreter.** On a `#!` line with an absolute,
   non-`env` interpreter:
   - If the interpreter's leaf, **as written or canonically**, is
     `is_excluded_leaf`, the answer is **`PROBE_PROGRAM_EXCLUDED`**. That is
     condition 10's cause, and it is condition 10's judgement applied to the
     program the kernel would really run. The canonical leaf matters: a link
     named `runner` pointing at `/bin/sh` is a shell.
   - If the interpreter, as written or canonically, lies under any known
     provider prefix (detected, or any provider's documented candidates) and not
     under the destination's own prefix, the answer is
     **`PROBE_TARGET_NOT_THE_PACKAGES`**. That is condition 8's cause: what would
     run is not the destination package's file. `PROBE_TARGET_IS_ENV_SCRIPT`
     would say something false about the shebang.
   - An `env`, relative or unfinished line stays `PROBE_TARGET_IS_ENV_SCRIPT`.

   Tests cover each case: `#!/usr/bin/env sh`, `#!sh`, `#!/bin/sh`, a link to
   `/bin/sh` under another name, an unresolvable `#!/mpb-nowhere/bash`,
   `#!/opt/local/bin/…` and `#!/usr/local/bin/…`.

   **Consequence for tests:** shell scripts can no longer be admitted. The
   admission fixtures now use `#!/usr/bin/true` (or `#!/usr/bin/touch` for the
   self-change case). The runner's own behaviour is tested on hand-built
   `AdmittedProbe`s (`direct`): the exact environment, `arg0`, every ending, the
   timeout. Those tests bypass `admit`, which they are not about.
8. **Condition 11's own failure has condition 11's cause.** A failure to open,
   `fstat` or read the head answers **`PROBE_TARGET_IS_ENV_SCRIPT`**, no longer
   condition 9's `PROBE_TARGET_MODE_REFUSED`. Reason: condition 9 already passed
   on the `lstat`, so its cause would say something false. Condition 11 is the
   condition that could not be established, and its cause is the one that means
   "what runs could not be read off the file". A handle whose `(dev, ino)` is not
   the `lstat`'s stays `PROBE_TARGET_CHANGED`. Test: a target at mode `0311`,
   executable but unreadable by its owner.

Kept as asked: returning `CheckAnswer`; `run_probe` producing no `Invocation`;
version checked after ownership; a changed target overriding `Fails`;
`CFFIXED_USER_HOME` unset.

## As built (for the design note)

**Admission (`AdmittedProbe::admit`), in evaluation order, each refusal a named cause, none a spawn:**

| # | Check | Cause |
|---|---|---|
| 1 | `owner.is_this_machine()`, `!runner.is_replay()`, `runner.may_run_probes()` (false for any runner but a non-recording `SystemRunner`) | `NOT_OBSERVED` |
| — | `effective_uid()` read (the crate's own, a file just made) | `None` → `PROBE_NOT_STARTED` |
| 2 | uid is not 0 | `PROBE_AS_ROOT` |
| 3 | `ExecutableName::parse(leaf)` | `PROBE_LEAF_REFUSED` |
| 4 | a row for exactly `(owner.provider(), package, leaf)` in the embedded table; then `argv` equals the row's | `NO_CURATED_PROBE`; `PROBE_ARGV_NOT_THE_ROWS` |
| 5 | `argv` is exactly one of `--version`, `-version`, `-V`, `-v` (also enforced by the reader and the site) | `PROBE_ARGV_NOT_PERMITTED` |
| 7 | `placed_entry(prefix, leaf)` (bin, then sbin, typed parts) exists; `owner.entry_owner(package, leaf, placed)` is `Owner { name == package }` | `PROBE_ENTRY_NOT_THE_PACKAGES` |
| 6 | that owner answer's version equals `observed_version` exactly | `PROBE_VERSION_NOT_QUALIFIED` |
| 8 | `canonicalize(placed)` succeeds, is a safe path, lies under `owner.prefix()` by component (written or canonical), and `owner.target_is_packages(package, placed, canonical) == Ok(true)` | `PROBE_TARGET_NOT_THE_PACKAGES` |
| 9 | one `lstat` of canonical: regular file, no `04000`/`02000`, uid = euid or 0, no `022` | `PROBE_TARGET_MODE_REFUSED` |
| 10 | neither the placed leaf nor the canonical leaf is a provider program, a system probe, or a shell/interpreter (with version suffix, case-folded) | `PROBE_PROGRAM_EXCLUDED` |
| 11 | open, `fstat`, and read the head of canonical | open/`fstat`/read failure → `PROBE_TARGET_IS_ENV_SCRIPT`; handle `(dev, ino)` ≠ the `lstat`'s → `PROBE_TARGET_CHANGED` |
| 11 | head is no `#!`, or an absolute non-`env` interpreter | `env`, relative, unfinished → `PROBE_TARGET_IS_ENV_SCRIPT` |
| 11 | that interpreter's leaf, written or canonical, is not excluded | `PROBE_PROGRAM_EXCLUDED` |
| 11 | that interpreter, written or canonical, is not under another provider's prefix | `PROBE_TARGET_NOT_THE_PACKAGES` |

**After admission, before spawn (`probe_with`). Every failure here is `PROBE_NOT_STARTED`:**
- `lock.verify_still_current()`;
- `lock.path()` is `<state root>/run.lock`;
- the state root is neither equal to nor under any known prefix (written or canonical);
- `<state root>/probes` is a real directory owned by the euid with no `077` bits;
- every entry under `probes` is a scratch-shaped, euid-owned, `0700` directory, or the probe stops and nothing is removed; those leftovers are removed;
- `probes/probe-<pid>-<6 hex>` is created at `0700` (fails if anything is there), then re-checked: a directory, owner = euid ≠ 0, mode `0700`.

**At the site (`system.rs`):**
- `run` refuses any curated spec, unrecorded.
- `run_probe`: a non-probe spec → refused (`PROBE_NOT_STARTED`); a recording runner → `NOT_OBSERVED`; replay-only or a failed shape check → refused; the target's `lstat` identity is re-read and any difference → `PROBE_TARGET_CHANGED`; a spawn failure → `PROBE_NOT_STARTED`.
- The shape check: program and scratch are safe paths with no `..`; program and `arg0` leaves are not excluded; `arg0` is a valid `ExecutableName`; one closed-set flag; timeout ≤ 30 s; cap ≤ 64 KiB; exactly the eight environment keys with the probe's values.
- `ReplayRunner::run` refuses any curated spec. `run_probe` and `may_run_probes` keep their defaults.

**Answer (`answer`), exhaustive on `ExitStatus`:**

| How it ended | Answer |
|---|---|
| exit 0, both streams complete | `HOLDS` |
| exit 0, a stream held open past the drain | `PROBE_LEFT_A_PROCESS` |
| exit 0, a stream over the cap | `PROBE_OUTPUT_OVER_CAP` |
| exit 0, a read error on a stream | `PROBE_END_NOT_READ` |
| non-zero exit, or a real signal | `FAILS (PROBE_FAILED)` |
| signal `None` (the wait failed), or `Cancelled` | `PROBE_END_NOT_READ` |
| timeout | `PROBE_TIMED_OUT` |
| not started, or refused at the site | `PROBE_NOT_STARTED` |
| target changed before spawn | `PROBE_TARGET_CHANGED` |
| runner not observing | `NOT_OBSERVED` |
| target identity differs after exit | turns `HOLDS` or `FAILS` into `PROBE_TARGET_CHANGED` |

A malformed embedded table answers `NO_CURATED_PROBE`.

## Deviations from the design note still standing (with reasons)

- **`ProbeOwner` (`pub(crate)` trait) carries the owner rule.** The rule lives
  outside the frozen module. H-3 implements it for both adapters:
  - `entry_owner`: the existing ownership read, on the placed path given.
  - `target_is_packages`: Homebrew `keg_of(canonical, real_cellar)`; MacPorts
    `owner_read(placed)` and `owner_read(canonical)`.
- **`CheckAnswer` is the return type, not `ProbeObservation`.** Kept, per the
  coordinator; the design note is being amended.
- **`run_probe` / `ProbeRun` / `may_run_probes` are trait methods instead of
  `run`.** No `Invocation` exists for a probe, so nothing can be recorded. Kept,
  per the coordinator.
- **Condition 5 is backed by the reader and the site,** so its test feeds `admit`
  a hand-built row inside the unit test, with no seam.
- **Condition 7 is evaluated before condition 6.**
- **A changed target overrides `FAILS` as well as `HOLDS`.** Kept.
- **`CFFIXED_USER_HOME` is not set.** Kept.
- **The stateful exclusion is not in the adapter's reader.** It needs
  `stateful-packages.toml`. The policy loader refuses to load on a stateful row,
  and H-3's agreement test through `curated_probe_rows` keeps the two readers equal.
- **The leftover sweep runs at the start of each probe,** under the lock the
  caller proves, and only over scratch-shaped, owned, `0700` directories.

## Tests

All are in `process/` under `cfg(test)`; there is no `pub` seam that accepts a
table or a prefix.

**Named:**
- `invariant_06_a_probe_is_never_admitted_as_root`
- `invariant_10_a_probe_runs_in_exactly_its_environment`
- `invariant_15_a_replay_never_runs_a_probe_at_the_spawn_site`
- `invariant_20_every_probe_row_is_dated`

**Admission:** one test per condition: 1 (three tests: scope/replay, the runner
itself, a recording runner), 3, 4, 5, 6, 7, 8 (with the floor), 9, 10, 11 (seven
shebang cases plus an unreadable head). Each test asserts the cause, that the
counting runner started nothing, and that no scratch was left.

**Probe flow:**
- the lock of another root;
- the scratch directory itself;
- sweep: positive case and foreign entries;
- a planted `probes`: a link to a non-empty directory, a file, a `0755`
  directory, a link to an empty directory;
- a state root under the prefix;
- a target changed by the run;
- the passing probe, which asserts what the rule was asked.

**Runner (on hand-built admissions):**
- the exact environment;
- `arg0`, via `/bin/test` as `[`;
- exit 1, `SIGTERM`, over cap, held pipe;
- timeout;
- a target changed between admission and spawn;
- a link re-pointed after admission still runs what was admitted.

**Site and runners:**
- the site refuses 18 widenings;
- the allowlist refuses a probe-shaped spec;
- `run` refuses a probe, unrecorded;
- `run_probe` refuses allowlist and replay-only specs;
- replay and recording runners answer `NOT_OBSERVED` and `may_run_probes() == false`.

**Answer table and table reader:**
- every row of the answer table;
- the shipped table parses and the `pub` copy equals it;
- the reader fails closed (12 cases);
- the whole probe reads only the embedded table.

**No provider program runs.** Targets are `/usr/bin/true`- and
`/usr/bin/touch`-interpreted files written in the test, `#!/bin/sh` scripts used
only on the runner path, and `/bin/test` for `arg0`.

## Results (final tree, round 1)

- `cargo build`: ok.
- `cargo clippy --all-targets --all-features -- -D warnings`: ok.
- `cargo test --workspace --all-features`: **2043 passed, 0 failed.**
- `scripts/validate.sh --quick`: `validate: ok (--quick)`. Its `prune-target`
  step warned at the end of round 0 that `target/` was over its 5 GiB limit.

## Mutation pass (round 1)

59 mutants, scripted, applied one at a time, each run against
`cargo test -p macportsbrew-adapters --lib process::`. The script and logs are in
`scratchpad/h01-mut/` (`mutate.py`, `run2.log`).

**All 59 are killed.** Three survived the first run and were closed with tests:

| Survivor | Why it survived | Closed by |
|---|---|---|
| `c11-interpreter-leaf-drop` (only the canonical interpreter leaf checked) | `/bin/sh` is caught canonically too | `#!/mpb-nowhere/bash`, unresolvable, so only the written leaf catches it |
| `probes-dir-unchecked` | the sweep refused the planted cases anyway | a `0755` `probes` directory, and a link to an empty directory, both of which the sweep would pass |
| `sweep-name-unchecked` | every non-matching entry also failed the owner/mode test | a private `0700` directory named `mine-too` |

Killed outright:
- condition 1 (drop; replay not asked; recording not asked);
- 2, 3;
- 4 (package; provider; argv);
- 5;
- 6 (drop; prefix match);
- 7 (owner dropped; placed not required; rule not asked of placed);
- 8 (drop; placed/placed; floor dropped);
- 9 (drop; write bits; run-as bits; owner);
- 10 (drop; canonical leaf);
- 11 (drop; `env` allowed; canonical interpreter leaf dropped; foreign prefix
  dropped; unreadable as mode);
- `Fails` as `Holds`; held pipe as `Holds`; over cap as `Holds`; timeout as `Fails`;
- the real `HOME`; the placed path for the canonical; no `arg0`; probe cwd `/`;
- site: timeout, cap, environment, flag unchecked; always true;
- identity re-read skipped, before spawn and after exit;
- recording runner may probe (`run_probe`'s own check, and `may_run_probes`);
- `run` accepts a probe; replay answers a probe;
- scratch owner unchecked; state root unchecked; lock root unchecked;
- sweep dropped; sweep owner/mode unchecked; sweep mode unchecked; sweep skips
  foreign entries; sweep removes everything;
- scratch not removed.

## Outside `process/`

- **`00-session-pr-run-lock-path.patch`** (`run_lock.rs`, `RunLock::path()`):
  required for H/01 to compile, and pure plumbing. It goes in the session's PR
  and must merge before the owner applies H/01. If the coordinator prefers no
  prerequisite, the fallback inside `process/` is `run_lock::probe(state, euid)
  == LockProbe::Held` (a second open + shared try-lock on
  `<root>/run.lock`). That is weaker: a different process holding that root's
  lock would also pass.
- **For H-3 (unfrozen):**
  - the two `ProbeOwner` implementations;
  - the `DestinationSites` call to `run_curated_probe(owner, &state, &lock,
    package, leaf, argv)`;
  - the agreement test against the policy loader through
    `process::curated_probe_rows()`.

  No further frozen hunk is needed.

## `git apply` checks

Against a clean detached worktree at `origin/main` (`54b56eb`):
- `git apply --check 01-curated-probe-admission.patch` alone → ok (textually; it
  does not compile without the prerequisite).
- `git apply --check 00-session-pr-run-lock-path.patch` → ok.
- Both applied (prerequisite first): all five files are byte-identical (`cmp`)
  to the built worktree's, which built, linted and tested as above.
- `git apply --reverse --check 01-…` → ok. Both reversed → `git status` clean.
  That worktree was removed.
