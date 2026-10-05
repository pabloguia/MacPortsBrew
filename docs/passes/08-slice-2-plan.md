<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 2 — the single-instance lock, the interrupted-run marker, the concurrent-provider detector: plan

Work order: [`../../prompts/08-slice-2-lock-marker-detector.md`](../../prompts/08-slice-2-lock-marker-detector.md)
(parent: [`../../prompts/08-the-mutating-executor.md`](../../prompts/08-the-mutating-executor.md),
item 2). Drafted 2026-10-05. Reviewed by `spec-guardian` and
`security-reviewer` **before any code of the lock, marker or detector**; the
finding each change answers is cited in brackets (`SG` spec-guardian, `SR`
security-reviewer). Sources: [R-17](../research/R-17-provider-mutation-locks.md),
[R-18](../research/R-18-advisory-file-locks-on-the-pinned-toolchain.md),
`04-migration-and-safety.md` (Concurrency and locking, History),
`06-cli-and-ux.md` (exit codes), SE-015, SE-092.

## Facts that shape it

1. **`File::try_lock` exists on the pinned toolchain** (1.98.0; stable since
   1.89.0), maps to `flock(2)` on Apple targets, conflicts across separate
   opens of one path, and is released by the kernel when the holder dies — also
   by `SIGKILL` (R-18, observed). So a held lock has no stale state to clean up.
2. **A lock file's creation is subject to the umask**: under `0277` it is
   created `0400` and the same user's next read-write open fails (R-18,
   observed). The mode must be set on the open handle after creation, as
   `write_new_atomic` already does. The file must **never be unlinked**: after
   an unlink and re-create two inodes lock at once (R-18, observed).
3. **Network volumes are unknown** (R-18: `man flock` is silent, every volume
   on the research machine is APFS). The lock can therefore refuse
   (`try_lock` error → fail closed) or, worst case, silently lock locally only.
4. **Neither provider has a stale lock, and the file's presence proves
   nothing** (R-17): Homebrew 7.0.4 holds an `flock` on
   `<prefix>/var/homebrew/locks/<name>.<type>.lock` that is never unlinked;
   MacPorts 2.12.6 holds one on `<prefix>/var/macports/registry/.registry.lock`
   (`root:wheel 0644`), taken only by a `port` that can write the registry, so
   a non-root `port` takes none and a cask path takes none (both read
   from source, R-17; none was observed holding a lock).
5. **A filesystem-only detector needs `fcntl(F_GETLK)`.** That is the only
   probe that acquires nothing and so cannot disturb a provider (R-17,
   observed). `flock(SH|NB)` is unsafe against Homebrew: a concurrent
   exclusive taker is refused and Homebrew aborts the operation. `std` has no
   `F_GETLK`, `unsafe_code` is `forbid` for the workspace, and `libc` is a new
   dependency, so **the detector cannot be built in this slice** without an
   owner decision.
6. **The executor shell is unreachable** (`THIS_BUILD_DISPATCHES_MUTATIONS =
   false`): nothing calls the lock or the marker from shipped code, exactly as
   slice 1's store before its call.
7. **Slice 1's store gives the shape**: private state directory (`StateDir`),
   `write_new_atomic` (links, never replaces), `read_bounded` (checks on the
   handle, refuses a symbolic reference), `refuse_root`, `HISTORY_UNUSABLE`
   for anything the writer would not have left.

## What this slice does and does not close

- **Closed:** the single-instance lock (MacPortsBrew against MacPortsBrew) and
  the interrupted-run marker exist as tested adapters with a typed marker
  grammar, error identifiers with exit codes, and tripwires that fail the build
  which sets `THIS_BUILD_DISPATCHES_MUTATIONS` without the executor shell
  taking the lock, writing the marker, publishing the record and only then
  clearing it, in that order.
- **Not closed — the detector (SE-015's "detect a concurrently running `brew`
  or `port` before mutating and refuse").** It stays `NOT_CHECKED` (live) and
  `UNKNOWABLE` (replay, INV-15). **A tripwire fails the build that flips the
  switch while a live run still says `NOT_CHECKED`**, so the refusal is a gate
  and not a checklist item; an owner who accepts flipping without it changes
  that test knowingly. The owner's options (a dependency and one `unsafe`
  call, a half detector, or none) are DP-030.
- **Not wired:** the executor shell does not call any of it (fact 6). Two things the wiring must still do are owed and named as preconditions of the flag-flip proposal: re-run the state-dependent part of the gate **under the lock** [SR M4], and surface a stranded marker in `doctor` [SR L1].

## Design

### The lock — `adapters::persistence::run_lock`

- Module function `run_lock::acquire(state: &StateDir, euid: u32) -> Result<RunLock, DomainError>`
  [SG M1: a module function, so the tripwire token is unambiguous]. The file is
  `<state root>/run.lock`. `RunLock` owns the `File`; dropping it releases the
  lock, and the kernel does on death. Exclusive, `try_lock`, never blocking.
  `euid` is the caller's, derived once by the CLI as the store's is; a wrong
  value would weaken the owner check and the root refusal, which is why the CLI
  derives it from the state directory's own probe, not from a flag [SR L4].
- **Open sequence** [SR M1, M2, SG L2]:
  1. refuse euid 0; `state.ensure()`;
  2. `create_new` (`O_EXCL`, mode `0600`) — a dangling symbolic reference
     planted at the path makes this fail with `AlreadyExists`, it is never
     followed to create its target;
  3. on `AlreadyExists`, open the existing entry **read-only** with
     `O_NOFOLLOW | O_NONBLOCK` (`bounded_read`'s `open_flags`, made
     `pub(crate)`): `flock` needs no write access, so a file left `0400` by a
     `0277` umask or by a crash between create and chmod still opens;
  4. on the handle only: regular file, owner `euid`, `nlink == 1`, and **no
     group or other access — refused here, before any repair** (a `0644` file
     is somebody's, not a umask accident); the owner is checked before the
     `fchmod`, so the classification is deterministic [SG L2];
  5. `set_permissions(0600)` on the handle (repairs `0400`; the umask, fact 2);
  6. `try_lock`; then `lstat` the path again and require the same device and
     inode as the handle, so a file unlinked or replaced after the open is
     refused — `RunLock::verify_still_current()` repeats this check and is
     called when the marker is written, the last point before a mutation
     [SR M3]. Deleting `run.lock` during a run is unsupported and said so in 04;
  7. **is the lock real?** open the same path a second time in-process and
     `try_lock` it: it must be `WouldBlock` (R-18 observed that separate opens
     conflict). If it succeeds, `flock` is a no-op on this volume: drop, fail
     closed, name the state directory and the override that relocates it
     [SR M7]. The decision is a pure function, unit-tested, because no local
     volume can be made to misbehave.
- Outcomes: acquired; `WouldBlock` → `ErrorId::RunLockHeld` (exit **7**, no pid
  named, none is recorded); an I/O failure or an unreal lock → `SafetyBlocked`
  (exit 4: the lock cannot be established so nothing is changed — chosen over
  `Io` because the remediation is "nothing was changed, relocate the state
  directory", recorded in SE-015 [SG L1]); an entry that is not what this writer
  leaves (symbolic reference, other owner, group/other access, not a regular
  file, `nlink != 1`) → `HistoryUnusable` (exit 8). Error detail names the entry
  **relative to the state root**, never an absolute path or a uid [SR L2].
- Never unlinked by this tool, never truncated, never written; content empty.
- **Taken** only by a run that will dispatch a provider-mutating step, after the
  gate and the history probe, before the marker. Reading never takes it; replay
  and snapshot runs never reach it (`HistoryScope`, INV-15). A held lock is
  exit 7 and the run changes nothing.
- **Close-on-exec.** `std` opens files with `O_CLOEXEC`, and `Command` closes
  such descriptors on exec; a tripwire fails if `run_lock.rs` ever passes
  `custom_flags` other than the two named, which is the only way to clear it
  [SR L3]. The stated consequence is not "errs towards refusing": a provider
  child that daemonised with the descriptor would hold the lock as long as it
  lives and every later mutating run would exit 7 with nothing this tool can
  clear. That is why the property is pinned, not argued. (R-18 did not observe
  it; not claimed as tested.)
- **What it is not:** it does not exclude `brew` or `port` (R-17), protects
  against no other uid, and gives no exclusion where `flock` is host-local on a
  shared volume (unreal-lock check above catches only a no-op; cross-host is not
  detected). The state root's ancestors are not checked — the gap `state_dir.rs`
  already documents [SR L4]. All of it goes to 04.

### The interrupted-run marker — `domain::history` + `adapters::persistence::run_marker`

- Domain: `InterruptedRun { run_id }`; stored shape
  `StoredInterruptedRun { schemaVersion: 1, runId }`, `deny_unknown_fields`,
  re-validated by `InterruptedRun::from_stored` (id grammar, `is_run_id`).
  Nothing else in it.
- Adapter, file `<state root>/interrupted-run.json`, **every function takes
  `&RunLock`** [SR H1, M6]: no marker can be written, read-for-refusal or cleared
  without the witness that the lock is held, and `write` calls
  `verify_still_current()`.
  - `run_marker::read(&RunLock, state, euid) -> Result<Option<InterruptedRun>, DomainError>`
    creates nothing; absent → `None`; present → bounded read, `RefuseSymlink`,
    ≤ 4 KiB, owner `euid`, no group/other access, strict grammar — duplicate
    keys, trailing data, a BOM and a NUL byte are each tested. Anything else is
    `HistoryUnusable`, never "no marker" (`HISTORY_UNUSABLE`'s rules) [SR L4].
  - `run_marker::write(&RunLock, state, &InterruptedRun, euid)` publishes by
    link and never replaces, **and the directory sync must succeed** before it
    returns: the marker exists to survive a power loss, so a lost directory
    entry after the first mutation would leave neither marker nor record. This
    is `write_new_atomic`'s `Durability::Required` mode; the best-effort
    behaviour of the record stays as it is [SR M5]. `Exists` after `read` said
    absent (not expected under the lock) is `RunInterrupted` naming the
    existing run; the lost-reply branch is not taken for a pre-existing marker
    (tested) [SR L4].
  - `run_marker::clear(&RunLock, state, &InterruptedRun, euid) -> ClearOutcome`
    (`Cleared`, `AlreadyAbsent`, `NotOurs`) — the tool's only delete. It opens
    the marker `O_NOFOLLOW`, reads it, compares device and inode of the handle
    with an `lstat` taken immediately before `remove_file`, and removes only if
    the run id is this run's. `AlreadyAbsent` is success with a warning, so a
    hand removal never turns a completed run into an error; `NotOurs` and an
    unusable marker are left in place and reported. The remaining window
    between the check and the unlink is a same-uid actor and is documented, not
    claimed closed; under the held lock the only other writer is this process
    [SR M6, SG L5]. No delete by a user's typed id exists in this slice.
- **When the marker is written and cleared** [SG H1]. Written immediately
  before the first step that can change a provider (not before
  `SNAPSHOT_BEFORE`, which only writes this tool's state), after the lock and
  the history probe. **Cleared on every ending that returns control** — a
  success, a halt, a failure, a `NO_CHANGE` — in this order: the record is
  published first when the run dispatched anything (obligation 12), then the
  marker is cleared. A run that dispatched nothing is never written to history
  (04), so it clears its marker at once, and a run whose record could not be
  published **keeps its marker** (the run is unrecorded, which is exactly
  what an interrupted run is). Only a process that dies leaves one in the
  ordinary case. A refusal before the marker leaves none. Test:
  `a_run_that_dispatched_nothing_leaves_no_marker`.
- **Order of a mutating run**, asserted on the shell: gate → `history_store::probe`
  → `run_lock::acquire` → `run_marker::read` (present refuses: exit **4**,
  `RunInterrupted`, names the run) → `run_marker::write` → first dispatch →
  steps → `history_store::write_record` → `run_marker::clear` → release.
- **How a user clears it** — documented, not implemented [SG M3, SR L1]. The
  remedy names the run and the marker file and its directory **separately**
  (the default path has a space; no command line with a path is printed), says
  to first look for a record of that run in the history (a marker can outlive a
  completed run if `clear` failed after the record was published), to look at
  what the run changed (its snapshot, **if** `SNAPSHOT_BEFORE` persisted one —
  the step is not performed in this build, so the text does not promise one),
  and then to remove that one file **only when no other MacPortsBrew run is
  active**. Analysis, plan and dry run still work meanwhile, and the text says
  so. The by-hand step is temporary: `history` and `restore` (item 7) replace
  it. Recorded as D-M3 for the owner to overrule. Wording is checked against the
  stop gate's literal list before `en.toml` is written [SG M3].
- The marker is not read by non-mutating commands. Surfacing a stranded marker
  in `doctor` is owed (a new output field and catalogue text) and named as a
  precondition of the flag-flip proposal [SR L1].

### Error identifiers

- `RunLockHeld` — exit **7**; `RunInterrupted` — exit **4**, `{detail}` is the run
  id only. An unusable lock or marker reuses `HistoryUnusable` (exit 8), whose
  English message is **generalised once** to say "an entry of MacPortsBrew's own
  state" rather than "migration history … record" [SG L1].
- Touches: `crates/domain/src/error.rs` (`exit_code`, `message_id`, `all()`,
  `NUMBER_OF_ERROR_IDS` 18 → 20, `exit_codes_follow_the_contract` gains 7 and
  4), the exhaustive list in `crates/application/tests/executor.rs`,
  `locales/en.toml`, `docs/schemas/README.md` (row 7 gains `RUN_LOCK_HELD` and keeps
  "not reachable in this build", which is true of the code; row 4 gains one), `06-cli-and-ux.md`.

### The detector — not built; SE-015 records it in the same commit as the tripwires [SG M2]

Unchanged: `ConcurrentProviderActivity`, no new spawn (`ps`, `pgrep`, `lsof`
are not added), `unsafe_code` forbidden, no new dependency. R-17's admissible
claim, if the owner approves `F_GETLK`, is "a lock on `<path>` is held by some
process at this instant", never "a mutation is in progress" and never "idle";
an `open` or `F_GETLK` failure is the unknown fallback; the detector is
asymmetric between providers by their own design (Homebrew: per-formula lock
files, 208 on the research machine; MacPorts: one registry lock taken only by
a `port` that can write the registry, **from `registry.tcl` 2.12.6, read 2026-10-05 (R-17 E3); no run was observed** [SG, consistency])
and so must be reviewed under INV-17 when built. Options are DP-030:
(a) stay owed, (b) one `libc` dependency and one `unsafe` call, (c) a
MacPorts-only `try_lock_shared` probe (safe there, MacPorts waits; unsafe for
Homebrew). **Recommendation (a)**, then (b) when the owner chooses it.

### Reachability — a type, plus a text test that is itself tested [SR H1, SG H2, M1, L4]

- **Type.** `run_marker::{read, write, clear}` take `&RunLock`, so code that
  touches the marker cannot compile without having taken the lock. (The
  dispatch function is outside this slice; a `&RunLock`/marker-guard parameter
  on it is proposed for the wiring, item 7, because `executor/` is a guarded
  review area.)
- **Text.** `crates/cli/tests/history_preconditions.rs` gains a pure
  `wiring_violations(flag, files) -> Vec<String>`: it strips `//` comment lines,
  string literals and `#[cfg(test)]` modules, requires **all** tokens in **one**
  file, uses qualified tokens (`history_store::probe(`, `run_lock::acquire(`,
  `run_marker::read(`, `run_marker::write(`, `history_store::write_record(`,
  `run_marker::clear(`) and the first **call** of `dispatch(` (not `fn
  dispatch(`), and orders `probe < acquire < marker read < marker write <
  dispatch < write_record < clear`; a missing or duplicated token is itself a
  violation. It is **unit-tested with `flag = true`** on a good stub and on one
  mutant per token (removed, reordered, in a comment, in a string, in
  `cfg(test)`, split across files), so the proof is a permanent test, not a
  scratch run. Each of the five public tripwires calls it with the real flag.
- **Detector tripwire.** With the flag true, `AnalysisSource::Live`'s answer must
  satisfy `was_checked()` — a label `Unknowable` does not — and its failure
  message names DP-030 and says that only the owner relaxes it and that an agent
  weakening it breaks the work order's no-weakened-gate rule [SG M2]. The label
  check is the strongest static form available before a detector exists; the
  application-level refusal (recording fake, call order, no dispatch after a
  failed record) is owed with the wiring.

## Commits

1. **docs** — this plan with both reviews folded in; **SE-015 and SE-092
   entries and DP-030** (the deviation is never in the tree unrecorded) [SG M2].
2. **domain** — `InterruptedRun` + stored shape; `ErrorId`s, exit codes,
   `en.toml` (incl. the generalised `HistoryUnusable`), schemas README, `06`;
   tests.
3. **persistence** — `write_new_atomic` `Durability::Required` (injectable
   directory sync) with its tests; `bounded_read::open_flags` `pub(crate)`.
4. **persistence** — `run_lock` and its tests, mutation-proved.
5. **persistence** — `run_marker`, `docs/schemas/run-marker.v1.md`, tests,
   mutation-proved.
6. **cli tests** — `wiring_violations` and the tripwires; the INV-15 test.
7. **docs** — 04 Concurrency and locking (status, limits), CHANGELOG.
8. **fixes** from the end-of-slice reviews, then "As built".

## Named tests

- **Lock:** a second acquire while held is `RunLockHeld` (exit 7); free again
  after drop; a lock whose holder is dropped without `unlock` is free (a
  `SIGKILL`ed child needs a spawn the guards forbid in adapters tests — stated,
  R-18 observed it); a symbolic reference at `run.lock`, including a dangling
  one, is refused and its target is never created; another owner, `0644`,
  not-a-file, `nlink 2` are `HISTORY_UNUSABLE`; **a `0400` file is opened,
  repaired to `0600` and locks**; the absent case is created `0600`, regular,
  `nlink 1`; the file persists after release and is never truncated; the path
  unlinked after the lock is refused by `verify_still_current`; the unreal-lock
  decision function; euid 0 refused (`invariant_06_…`); a second sequential run
  succeeds after the first's drop and clear (INV-16); reading the marker never
  creates `run.lock`.
- **Marker:** round trip; second write never replaces and names the first run,
  and never takes the lost-reply branch; absent reads `None` and creates
  nothing; symlink, wrong mode, foreign owner, oversized, unknown field,
  unknown schema, bad id (`..`, separator, empty), duplicate key, trailing
  data, BOM, NUL — each `HISTORY_UNUSABLE`; `clear` removes only its own run's
  marker, leaves another's, leaves an unusable one, and an absent one is
  `AlreadyAbsent`; a marker left beside a completed run's record still reads;
  a crash between the link and the temporary's removal leaves a readable
  marker beside a hidden dot-entry; a failing directory sync fails `write`
  (injected) while the record's best-effort behaviour is unchanged.
- **INV-15:** `invariant_15_a_replayed_migrate_creates_neither_the_lock_nor_the_marker`;
  `HistoryScope`'s existing `only_a_live_run_consults_history` is the scope
  decision it relies on [SG L5].
- **INV-17:** `invariant_17_the_lock_and_the_marker_name_no_provider` — a scan
  of both modules and of `crates/cli/src/executor/`; adapters may name
  providers by rule, so this is by choice, as the lock is MacPortsBrew-wide
  [SG L5].
- **Error ids:** exit codes 7 and 4 pinned; every new id has a message and a
  remediation; the remedy text names the marker file and directory separately.

## Decisions (Tier 3, for SE-015 / SE-092)

- **D-L1** Exclusive advisory `try_lock` on `run.lock`, held for the life of a
  mutating run. **D-L2** Persistent, empty, `0600` on the handle, created
  `O_EXCL`, opened `O_NOFOLLOW`, never unlinked or truncated, re-checked
  against the path after the lock and at the marker write. **D-L3** Taken by a
  mutating run only, after the gate and the probe, before the marker; the
  state-dependent gate is re-run under it (owed, with the wiring). **D-L4** The
  lock proves itself real by a second in-process open.
- **D-M1** One marker naming one run, durable, written before the first
  provider change, cleared on every ending that returns control, after the
  record. **D-M2** Unreadable or unusable lock or marker is `HISTORY_UNUSABLE`.
  **D-M3** By-hand remedy until `restore`. **D-M4** `clear` is the only delete.
- **D-D1** The detector is not built; the flip is tripwired; DP-030.
- **D-E1** `RunLockHeld` 7, `RunInterrupted` 4; the rest reuse existing ids.

## Review

- **spec-guardian (2026-10-05):** H1–H2, M1–M3, L1–L5; no Tier 1 or Tier 2
  violation, the detector decision within authority provided SE-015 records it.
  All folded in: H1 → marker timing and `a_run_that_dispatched_nothing_leaves_no_marker`;
  H2/M1 → dispatch in the order, qualified tokens; M2 → SE-015 in commit 1,
  `was_checked()` tripwire naming DP-030; M3 → remedy wording, snapshot honesty;
  L1–L5 → ids, open sequence, close-on-exec, test procedure, test list.
- **security-reviewer (2026-10-05):** H1, M1–M7, L1–L4; no step grows the spawn
  allowlist or needs `unsafe`. All folded in: H1 → the `&RunLock` type and the
  tested `wiring_violations`; M1/M2 → the open sequence; M3 → re-check after
  the lock; M4 → owed with the wiring and stated in SE-092; M5 → durable
  marker write; M6 → `&RunLock` on write/clear, `ClearOutcome`; M7 → the
  second-open check; L1 → path and remedy; L2 → relative names; L3 → pinned
  close-on-exec; L4 → `nlink`, grammar, `Exists`, ancestors. **Accepted, not
  fixed:** the check-to-unlink window in `clear` (same-uid, documented); the
  `euid` is the caller's, as the store's (CLI-derived once).

- **End-of-slice reviews (2026-10-05)** — `spec-guardian`, `security-reviewer`,
  `docs-auditor`, over `a365d93..HEAD`. No Tier 1 or Tier 2 finding; no step
  grew the spawn allowlist, used `unsafe`, added a dependency or touched a
  guarded path. Findings and their disposition are in SE-092's slice 2
  end-of-slice entry: the marker published-then-failed is taken back (D-M5),
  `clear` fails closed, error details name files not paths, the tripwires are
  stronger and say they are necessary, not sufficient, and eleven statements in
  eight documents were corrected. Accepted without a change: the caller's
  `euid`, the `0200` lock file's message, the generic remedy for a `0644` file.

## As built (2026-10-05)

**Commits** (`a365d93..HEAD`):

| | |
|---|---|
| `f87b1bd` | Part A — the lost-reply rule is a pure function and the publishing call is injectable; a crash after the link is tested. SE-092's overclaim corrected. |
| `ab50b3e` | Part B — R-17 (provider locks) and R-18 (advisory locks on the pinned toolchain). |
| `ea90722` | This plan, reviewed; SE-015 and DP-030 record the unbuilt detector. |
| `568658a` | Domain: `InterruptedRun`, `RUN_LOCK_HELD` (7), `RUN_INTERRUPTED` (4). |
| `b3dd97d` | `write_new_durable`. |
| `0f4143d` | `run_lock`. |
| `cddc515` | `run_marker`, `run-marker.v1.md`. |
| `44c3acb` | Tripwires and the INV-15 replay test. |
| `f755fb0` | 04, CHANGELOG. |
| `afcb550` | Review fixes. |
| (docs) | Review corrections, this section. |

**Differences from the plan.** (1) The `0277` umask test could not be written
cleanly (`unsafe` and a shell wrapper are both forbidden to a test) and stays
owed, DP-029. (2) `PublishError` was not split: a failed durable publish makes
the marker's writer take its own marker back, which is the same safety without
touching the enum's other users (D-M5). (3) `RunLock::acquire` takes the
`euid` from its caller, as the store does. (4) The detector was not built, as
planned, and the tripwire for it uses `was_checked()`.

**What stays owed.** The `0277` umask test (DP-029); detection of a concurrent
`brew`/`port` and the refusal it serves (DP-030, SE-015); and, with the
executor wiring and as preconditions of the flag-flip proposal: the lock and
marker calls in the shell in the planned order, the state-dependent gate re-run
under the lock, `verify_still_current` before each mutating dispatch, the
application-level refusal on an observed or unchecked provider, a stranded
marker surfaced in `doctor`, `history` and `restore` (which replace the by-hand
remedy), and shim-backed recognition (INV-16, from slice 1).

**For the owner.** DP-029 (umask test: leave owed, a reviewed exception in
`check-no-shell.sh`, or one `unsafe` call) and DP-030 (detector: leave owed, one
`libc` dependency and one `unsafe` call, or a one-provider probe) — both have a
recommendation and a reversible default taken. R-17's researcher took a
momentary shared `flock` on the real MacPorts registry lock and ran
`port version`, once, with nothing running; neither changes anything, and it is
disclosed here because the instruction was read-only observation.
