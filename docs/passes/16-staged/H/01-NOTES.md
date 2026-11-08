<!-- SPDX-License-Identifier: Apache-2.0 -->

# H/01 — curated probe admission: build notes (review rounds 1 to 4 applied)

Built in a detached scratch worktree on `origin/main` = `54b56eb`
(`scratchpad/h01`, its own `target/`). Nothing applied to the main tree, nothing
committed.

## Artefacts

| File | What | sha256 |
|---|---|---|
| `01-curated-probe-admission.patch` | **The owner's patch.** `crates/adapters/src/process/` only: `mod.rs`, `system.rs`, `recording.rs`, new `probe.rs` | `21b5ac9c5af5bbb49d7ea4871a49a0f42301b5077a969af6485d2519278f8835` |

**Prerequisite, not a patch file:** `RunLock::path()` landed in PR #70 (`wo16/H-4-stage`, at
`b2a2351`) in `crates/adapters/src/persistence/run_lock.rs`, with `#[allow(dead_code)]` until H/01
is applied and the test `the_lock_names_the_file_of_its_own_state_root`. H/01 calls it (item 5
of round 1) and does not compile without it. Earlier patches (`7997598b…`, `40540c2e…`, `c2fde761…`, `3c109424…`) are
superseded.

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
7. **Condition 11 judges the interpreter.** *(Superseded in round 2: every `#!` target is now refused; see below.)* On a `#!` line with an absolute,
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

## Review round 2

- *(Superseded in round 3 by the Mach-O allowlist.)* **L1 (security) fixed: every `#!` target is refused** at condition 11, whatever its interpreter
  (absolute, `env`, relative, unfinished), with the reserved `PROBE_TARGET_IS_ENV_SCRIPT`; no cause
  added. The interpreter-leaf and foreign-prefix checks are **removed** (`target_head_admits` now
  reads two bytes). Admission fixtures are copies of `/usr/bin/true` (a Mach-O) placed in the test
  prefix. A copy of `/usr/bin/touch` (and of `/bin/test`) is killed by the system at exec (observed
  2026-11-08, exit 137), so the "target changed by the run" test uses a counting runner double that
  rewrites the target after the probe ends instead of a touching target. One case per old shebang
  stays, each refused: `#!/usr/bin/env sh`, `#!sh`, `#!/bin/sh`, an unresolvable `#!/mpb-nowhere/bash`,
  a link to `/bin/sh` under another name, `#!/opt/local/bin/…`, `#!/usr/local/bin/…`, and now
  `#!/usr/bin/true`. Runner-path tests stay on hand-built admissions (they use `#!/bin/sh` scripts,
  which `admit` would refuse). New: `condition_11_reads_the_head_only_of_the_file_lstat_described`
  (the handle check, which no test reached before; its mutant survived round 2's first run).
- **F6 (spec) fixed:** the condition-10 test is now `invariant_05_a_probe_never_starts_a_provider_program`:
  at `admit`, the placed leaf (`bash`, `brew`, `sw_vers`, `Python3.12`) and the canonical leaf
  (`perl5.34`, `brew`); at the site, the program (`…/brew`, `…/port`, `/usr/sbin/sysctl`) and `arg0`
  (`brew`, `port`, `sw_vers`, `bash`).
- **I2:** a comment at `sweep_leftovers`: it assumes probes run one at a time in this process;
  running them in parallel would need it to skip this pid's own scratch names.
- **Recorded, not fixed (open for H-3 / part L), as decided:**
  - **L2:** the euid oracle (`system::machine::effective_uid`) creates its probe file under
    `TMPDIR`; a pre-existing pattern in the crate, unchanged.
  - **L3:** `known_prefixes` holds the destination's detected prefix and every provider's
    documented candidates, but not another provider's *detected custom* prefix. With L1 the
    interpreter half is gone; the state-root half remains (a state root under another provider's
    custom prefix is not refused).
  - **I1:** the euid is read (condition 2) before the table lookup (condition 4); order kept.
- **I3, I4:** for H-3.

## Review round 3

- *(Tightened in round 4.)* **High finding fixed: condition 11 is now an allowlist.** Refusing only heads that start `#!` was
  not enough: on `ENOEXEC` macOS's spawn falls back to `/bin/sh`, so a text file with no `#!`, a
  BOM or whitespace or newline before `#!`, an empty file and a one-byte file were admitted and run
  by `sh` (the empty and one-byte ones giving a false `HOLDS`), and with `PATH = <prefix>/bin:` plus
  the system path such a file can start `brew` or `port` by bare name. Now **only a Mach-O
  executable for this build's architecture is admitted**: `is_executable_for_this_build` reads,
  with `pread` on the already-opened, identity-checked handle, the head (bounded by
  `macho::HEAD_BYTES`), runs `macho::classify`, and for each slice of a thin or fat file
  (`MAX_SLICES` at most) reads its header (`SLICE_HEADER_BYTES`), `plan_slice`, its window (at most
  `MAX_SIZEOFCMDS` plus a header) and `read_slice`, admitting when one slice is `MH_EXECUTE` with
  `cputype == THIS_BUILD_CPUTYPE`. Everything else, including a read error, is
  `PROBE_TARGET_IS_ENV_SCRIPT`. Only what `crate::macho` already exposes is used; **`macho.rs` is
  unchanged** and no hunk outside `process/` is added.
- **Cause naming (for H-3):** `PROBE_TARGET_IS_ENV_SCRIPT` now names the refusal of anything that
  is not this build's Mach-O executable; no cause added or renamed here.
- **Tests:** `condition_11_only_a_macho_executable_for_this_build_is_admitted` refuses, with the
  counting runner at 0, a no-`#!` text file, BOM + `#!`, leading space + `#!`, leading newline +
  `#!`, an empty file, a one-byte file, a hand-built `MH_DYLIB` for this build's `cputype`, and a
  hand-built `MH_EXECUTE` for the other architecture; plus the eight `#!` heads of round 2 and an
  unreadable head. Controls admitted by `admit`: the copy of `/usr/bin/true` (fat here: `x86_64` and
  `arm64e`, both `MH_EXECUTE`; this build is `x86_64-apple-darwin`) and the same hand-built header
  with this build's `cputype`, so the other-architecture refusal is for its architecture alone.
  `invariant_05_a_probe_never_starts_a_provider_program` gains a no-`#!` text target naming
  `brew --version` and `port version`, refused with nothing started.
- **Mutants (changed lines), all killed:** check bypassed (anything admitted); `MH_EXECUTE` not
  required; `cputype` not required; fat refused; thin refused; every slice required (`all` for
  `any`); unreadable as condition 9's cause; handle unchecked; condition 11 dropped; plus
  re-runs of the condition-10, placed-for-canonical and post-exit-identity mutants.

## Review round 4

- **High finding fixed (reproduced by the reviewer).** (a) `macho::classify` also reads
  little-endian fat (`FAT_CIGAM`/`FAT_CIGAM_64`), which the kernel does not exec: `ENOEXEC`, then
  `/bin/sh`. A `be ba fe ca 0a 00 00 00` header (nfat 10, `\n` at byte 4) whose slices pointed at a
  real executable slice, with `echo …; exit 0` in the padding, was admitted and run by the shell.
  (b) "any slice qualifies" is not the kernel's choice: it prefers `x86_64h` over `x86_64` on this
  CPU, and picks the native slice for a child of an `x86_64` build under Rosetta, so a good slice
  beside a text or dylib slice was admitted and handed to `/bin/sh`.
- **Now:** the raw magic is read by this module, not taken from `classify`'s answer. **Thin:** only
  native `MH_MAGIC_64` (`NATIVE_THIN_MAGIC`). **Fat:** only big-endian `FAT_MAGIC`/`FAT_MAGIC_64`
  (`KERNEL_FAT_MAGICS`), and **every** slice must be a thin native `MH_MAGIC_64` that passes
  `plan_slice` and `read_slice` as `MH_EXECUTE` (so no nested fat, no dylib, no text), with at least
  one slice for `THIS_BUILD_CPUTYPE`. Any failure refuses. `macho.rs` unchanged; nothing outside
  `process/`. A truncated thin `MH_EXECUTE` gives `EBADMACHO` or a `SIGKILL` at exec (no shell either way), per the reviewers, so a
  thin file with the real magic needs nothing more.
- **Regression tests** (in `condition_11_only_a_macho_executable_for_this_build_is_admitted`, all
  refused with the counter at 0): `FAT_CIGAM` nfat 10 with a shell payload; a byte-swapped thin
  `MH_EXECUTE`; a big-endian fat with one byte-swapped slice; good this-arch + text `x86_64h`; good
  this-arch + dylib `x86_64h`; good this-arch + junk other-arch (arm64 here); good this-arch +
  nested fat. **Controls admitted:** the copy of `/usr/bin/true`; a big-endian fat of two
  hand-built executables, one for this build; and the thin hand-built header for this build. A
  comment states the hand-built ones carry no `LC_MAIN`, so the kernel would refuse them
  (observed in review round 4's confirmation: the spawn succeeds and the kernel kills the child, `SIGKILL`, no shell); they are only admitted, never run.
- **Mutants (changed lines):** little-endian fat accepted, killed; any instead of every, killed;
  slice magic unchecked, killed; `MH_EXECUTE` unchecked, killed; `cputype` unchecked, killed; fat
  refused, killed; thin refused, killed; the whole check bypassed, condition 11 dropped,
  unreadable as condition 9's cause, handle unchecked: all killed. **One equivalent:** dropping the
  thin file's head-magic guard survives, because a thin file's one slice starts at offset 0 and
  the per-slice check reads the same four bytes and refuses; the guard stays as the stated rule.

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
| 11 | open and `fstat` canonical; the handle is the `lstat`ed file | open/`fstat` failure → `PROBE_TARGET_IS_ENV_SCRIPT`; handle `(dev, ino)` ≠ the `lstat`'s → `PROBE_TARGET_CHANGED` |
| 11 | **allowlist**: through that handle, a `pread` of the head (≤ `macho::HEAD_BYTES`), each slice header (≤ `SLICE_HEADER_BYTES`) and each window (as `plan_slice` answers) shows either a **thin file whose raw magic is native `MH_MAGIC_64`**, an `MH_EXECUTE` for `THIS_BUILD_CPUTYPE`; or a **fat file whose raw magic is big-endian `FAT_MAGIC`/`FAT_MAGIC_64`** (≤ `MAX_SLICES`), **every** slice a thin native `MH_MAGIC_64` `MH_EXECUTE` (no nested fat, no dylib, no text) and **one** slice for `THIS_BUILD_CPUTYPE` | anything else (text with no `#!`, any script, empty, one byte, dylib, bundle, object, archive, another architecture, a byte-swapped thin or fat header, a fat file with any non-executable slice, malformed or over-limit, a read error) → `PROBE_TARGET_IS_ENV_SCRIPT` |

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
- **Condition 11 is an allowlist** (rounds 3 and 4): admitted is a thin native 64-bit Mach-O executable for this build's architecture, or a big-endian fat file every slice of which is a Mach-O executable and one of which is for this build's architecture, far wider than the note's `env`-shebang refusal; same reserved cause, whose name now under-describes it (a cause-naming item for H-3; not renamed here).
- **Condition 7 is evaluated before condition 6.**
- **A changed target overrides `FAILS` as well as `HOLDS`.** Kept.
- **`CFFIXED_USER_HOME` is not set.** Kept.
- **The stateful exclusion is not in the adapter's reader.** It needs
  `stateful-packages.toml`. The policy loader refuses to load on a stateful row,
  and H-3's agreement test through `curated_probe_rows` keeps the two readers equal.
- **The leftover sweep runs at the start of each probe,** under the lock the
  caller proves, and only over scratch-shaped, owned, `0700` directories.

## Tests

All in `process/` under `cfg(test)`; no `pub` seam accepts a table or a prefix. Named:
`invariant_05_a_probe_never_starts_a_provider_program`, `invariant_06_a_probe_is_never_admitted_as_root`,
`invariant_10_a_probe_runs_in_exactly_its_environment`, `invariant_15_a_replay_never_runs_a_probe_at_the_spawn_site`,
`invariant_20_every_probe_row_is_dated`.

- **Admission** (targets are copies of `/usr/bin/true`, a fat Mach-O executable, in the test prefix; a counting runner shows
  nothing started): one test per condition: 1 ×3 (scope and replay; a runner claiming replay; a
  recording runner that leaves the state directory untouched), 2, 3, 4, 5, 6, 7 (rule asked of the
  placed path), 8 (with the prefix floor), 9, 10 (as `invariant_05_…`, also at the site), 11 (eight
  `#!` heads, no-`#!` text, BOM/space/newline before `#!`, empty, one byte, a dylib, another
  architecture, a byte-swapped thin, little-endian fat with a payload, fat with a byte-swapped,
  text, dylib, junk or nested-fat slice, an unreadable head; admitted: the `/usr/bin/true` copy, a
  big-endian fat of two executables and a this-architecture thin header; the handle check).
- **Flow:** the lock of another root; the scratch directory; the sweep (a conforming leftover with
  a link inside removed, outside kept; five foreign entries left intact, each refusing); a planted
  `probes` (link, file, `0755` directory, link to an empty directory); a state root under the
  prefix; a target changed by the run; the passing probe.
- **Runner** (hand-built admissions, `#!/bin/sh` scripts): exact environment; `arg0` via `/bin/test`
  as `[`; exit 1, `SIGTERM`, over cap, held pipe; timeout; target changed between admission and
  spawn; link re-pointed after admission.
- **Site and runners:** 18 widenings refused; allowlist refuses a probe-shaped spec; `run` refuses
  a probe, unrecorded; `run_probe` refuses allowlist and replay-only specs; replay and recording
  runners `NOT_OBSERVED`, `may_run_probes() == false`.
- **Answer table** (every row) and **table** (shipped parses, `pub` copy equals it, reader fails
  closed in 12 cases, the whole probe reads only the embedded table).

No provider program runs.

## Results (final tree, round 4)

In `scratchpad/h01` (which carries an equivalent `RunLock::path()`):
- `cargo build`: ok. `cargo clippy --all-targets --all-features -- -D warnings`: ok.
- `cargo test --workspace --all-features`: **2044 passed, 0 failed.**
- `scripts/validate.sh --quick`: `validate: ok (--quick)`.

In a clean `54b56eb` worktree with **PR #70's own `run_lock.rs` hunk applied first**, then this
patch (shared target directory): `cargo build` ok; `cargo clippy --all-targets --all-features --
-D warnings` ok; `cargo test -p macportsbrew-adapters --all-features`: **788 passed, 0 failed.**

## Mutation pass (round 2)

58 mutants (the round-1 set with the condition-11 interpreter mutants replaced), applied one at a
time, `cargo test -p macportsbrew-adapters --lib process::`; script and logs in
`scratchpad/h01-mut/` (`mutate.py`, `run3.log`). **All 58 killed.** One survived the first run and
was closed: `c11-handle-unchecked` (dropping the handle-is-the-`lstat`ed-file check) — no test
reached it; closed by `condition_11_reads_the_head_only_of_the_file_lstat_described`.

Changed-line mutants and their killers: `c11-drop`, `c11-scripts-allowed` (no `#!` check),
`c11-absolute-allowed` (absolute interpreters allowed), `c11-unreadable-as-mode` →
`condition_11_every_script_is_refused_whatever_its_interpreter`; `c11-handle-unchecked` → the new
handle test; `c10-drop`, `c10-canonical-drop` → `invariant_05_…`; `post-exit-identity-skipped` →
`a_target_changed_by_the_run_is_never_holds`; `placed-for-canonical` → `a_probe_that_passes_…`,
`a_link_re_pointed_…` and others. The rest of the round-1 set (conditions 1-10, answer table,
site, identity, recording, replay, scratch, lock, state root, sweep) remains killed on the Mach-O
fixtures.

## Outside `process/`

- `RunLock::path()`: in PR #70, not in this patch (see Artefacts).
- For H-3 (unfrozen): the two `ProbeOwner` implementations (`entry_owner` on the placed path given;
  `target_is_packages`: Homebrew `keg_of(canonical, real_cellar)`, MacPorts `owner_read` of placed and
  canonical); the `DestinationSites` call to `run_curated_probe(owner, &state, &lock, package, leaf,
  argv)`; the agreement test against the policy loader through `process::curated_probe_rows()`.
  No further frozen hunk is needed.

## `git apply` checks

Against a clean detached worktree at `54b56eb`, with PR #70's `run_lock.rs` hunk
(`git diff 54b56eb b2a2351 -- crates/adapters/src/persistence/run_lock.rs`) applied first:
- `git apply --check 01-curated-probe-admission.patch` → ok.
- `git apply` → the four `process/` files are byte-identical (`cmp`) to the built worktree's; that
  tree built, linted and tested as above.
- `git apply --reverse --check` → ok; both reversed → `git status` clean. The worktree was removed.

## Round 4 confirmation (2026-11-08)

`security-reviewer` confirmed the round-4 fix at `58a9e52`: little-endian fat, a text or dylib
preferred slice, a 32-bit slice and an empty fat table are refused, and no file condition 11 admits
can reach `ENOEXEC`, so no shell runs (XNU hands a file to another loader only for a magic or a
filetype this rule now excludes, for the file and every slice). Two notes, not changed in the
patch so that it stays the tree that was built and tested:

- **A stale test comment.** The comment above the hand-built controls in
  `condition_11_only_a_macho_executable_for_this_build_is_admitted` says the kernel refuses them with
  `EBADMACHO`, so `PROBE_NOT_STARTED`. Observed: the spawn succeeds and the child is killed with
  `SIGKILL`. H-3 corrects the comment.
- **For H-3: a kill at exec answers `FAILS`.** An admitted `MH_EXECUTE` that is malformed past the
  header (an unknown command in place of `LC_MAIN`, a segment past the end of the file) is killed by
  the kernel at exec, which the answer table reads as a real signal: `FAILS (PROBE_FAILED)`. That is
  the safe direction (the step halts and the source is kept, never `HOLDS`), but it reports a
  package that never ran as failing. Telling an exec-time kill from the package's own failure is
  H-3's.
- Optional, not taken: checking each fat entry's `cputype` against its slice header, and slice
  overlap. The kernel refuses both without a shell (`EBADARCH`, `EBADMACHO`).
