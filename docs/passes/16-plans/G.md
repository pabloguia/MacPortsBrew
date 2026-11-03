<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part G — the provider lock probe (DP-030 (b))

Plan for `prompts/16-complete-mutating-product.md` §5 G, which runs
`prompts/08-slice-9-detector-and-flip-proposal.md` **part A, option (b)**,
with the mechanism ADR-0007 chose. Work order 16 §1 item 8 overrides the slice
file's plan-mode, merge, push, dependency and stop instructions. Reviewed by
`spec-guardian` and `security-reviewer` before any code (the Reviews table at
the end); their review is the approval (§4).

## The ruling and its binding requirements

DP-030 (b), ruled 2026-10-27: build the `F_GETLK` probe on each provider's locks
as R-17 locates them, with the locations in `data/policy/` (INV-20). The claim
is "a provider lock is held right now", never "idle"; a held lock refuses the
run with a named error. MacPorts' lock being taken only by a registry-writing
`port` is the provider's asymmetry, reviewed under INV-17.

Binding, from work order 16 §5 G and slice 9 part A:

1. A provider-neutral detector trait in `domain`.
2. Lock locations in `data/policy/`, never above `adapters/`.
3. Every error falls back to unknown.
4. `Unknowable` under `--replay` and `--from-snapshot` (INV-15).
5. A refusal on `Observed` and on the unknown fallback.
6. Opening: `O_RDONLY | O_NOFOLLOW | O_NONBLOCK`; `fstat` on the handle must
   say regular file; the directory listing is bounded; any error is unknown.
7. Mutants include: drop `O_NOFOLLOW`; drop the regular-file check; query with
   `F_RDLCK`; read `EINVAL` (any −1) as not held.
8. The refusal is a named error with an exit code, in `crates/domain/src/error.rs`'s
   contract and in the schema.
9. The `NOT_CHECKED` tripwire becomes a real answer, and a failing test is kept
   for a build that dispatches with no probe.
10. ADR-0007's obligations: `libc` direct in `crates/adapters` only; the exact
    query; the lint split (`deny` at the workspace, `#![forbid(unsafe_code)]`
    at the roots of `domain`, `policy`, `application`, `cli` `lib.rs` and
    `main.rs`); the one `#[allow(unsafe_code)]` in
    `crates/adapters/src/lock_probe/getlk.rs` with `// SAFETY:` and
    `#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]`;
    the comment- and string-stripped scan with its self-test; the
    root-attribute test from `cargo metadata`; SE-110 amended; ADR-0004's
    "`unsafe_code = "forbid"`" sentence corrected; a staged one-line fix of
    `scripts/check-licences.sh`'s comment (guarded); `security-reviewer` reads
    the `unsafe` line by line.
11. §3 item 10: a lock file is an entry of the lock directory itself, never
    followed.

## Where it lives today (code map, 2026-11-03)

- `crates/domain/src/concurrency.rs`: `ConcurrentProviderActivity`
  (`NotChecked`, `Unknowable`, `NoneObserved`, `Observed { providers }`),
  `possibly_inconsistent`, `was_checked`, `ConcurrencyFlag`.
- `crates/application/src/analysis.rs:55`:
  `AnalysisSource::concurrent_provider_activity` — `Live => NotChecked`,
  `Snapshot => Unknowable` (a replay is `Snapshot`).
- Consumers: `crates/cli/src/app/{mod,migrate,restore,human}.rs`; the gate and
  the executor do not read it.
- Tripwire: `crates/cli/tests/history_preconditions.rs:487`
  `mutations_cannot_dispatch_while_a_live_run_still_says_not_checked`.
- `crates/cli/tests/executor.rs:1772` `no_workspace_crate_depends_on_libc_directly`
  (ADR-0007 says G edits it on purpose).
- `Cargo.toml:38` `unsafe_code = "forbid"`; no crate root carries an attribute.
- Prefix: `registry::detected_prefix`; `portdbpath`:
  `configuration::macports_portdbpath` answers only for this machine and only
  the default `<prefix>/var/macports`.
- No lock data in `data/policy/`.
- R-17: Homebrew `<prefix>/var/homebrew/locks/` (E1, E2: 208 regular files
  here, the suffix-less `update` and `vendor-install-ruby` among them); MacPorts
  `${portdbpath}/registry/.registry.lock` (E3); observation C; the admissible
  claims (Recommendation table) and rules 1–5.

## Design

### 1. Domain (G-2)

- `ConcurrentProviderActivity` gains **`NotDetermined { providers }`**: a probe
  ran and could not answer for these providers (any open, `fstat`, listing or
  `F_GETLK` error; a listing over the bound; a provider whose lock location is
  not known, e.g. a non-default `portdbpath`). `possibly_inconsistent` true;
  `was_checked` false (only a definite answer is a check). Serialised
  `NOT_DETERMINED` with `providers` (additive in every `--json` document that
  carries the flag; schema notes updated).
- The doc of `NoneObserved` is narrowed to R-17's claim: "no held provider lock
  was observed at this instant" — never idle, never safe.
- `Observed { providers }` means "a lock on one of these providers' lock files
  was held at this instant".
- **The trait** (`crates/domain/src/concurrency.rs`):
  `pub trait ProviderLockProbe { fn sample(&self) -> ConcurrentProviderActivity; }`.
  Provider-neutral: it names no provider and no path. One sample is one
  instant (R-17 rule 4).
- `ErrorId::ProviderLockHeld` (`error.provider_lock_held`), **exit 7** — the
  code already means "lock held"; 06 and `docs/schemas/README.md` widen its
  row to "another MacPortsBrew run, or a provider's own lock". The unknown
  fallback (`NotDetermined`, `NotChecked`, `Unknowable`) refuses as
  `SAFETY_UNVERIFIABLE` (exit 4) with the reason
  `PROVIDER_LOCKS_NOT_DETERMINED`: nothing was seen held, so exit 7 would
  overclaim.

### 2. Data (G-2)

`data/policy/provider-locks.toml`, one `[[entry]]` per provider, read only by
`crates/adapters/src/registry.rs` (as `provider-prefixes.toml` is, DP-014):

- `homebrew`: `base = "prefix"`, `dir = "var/homebrew/locks"`, `files = "all"`
  (every directory entry, any suffix), `source` E1/E2, `read_on = 2026-10-05`.
- `macports`: `base = "portdbpath"`, `file = "registry/.registry.lock"`,
  `source` E3, `read_on`.
- Validation on load: relative, no `..`, no leading `/`, a closed `base` set;
  an unknown provider id refused. Policy version bumped; the data version test
  updated.
- A provider with no entry, or whose `base` cannot be resolved on this
  machine, is `NotDetermined` for that provider.

### 3. Adapters (G-2)

`crates/adapters/src/lock_probe/` — the only module that names `libc`:

- `getlk.rs`: `pub(super) fn held(fd: BorrowedFd<'_>) -> Result<bool, ()>`, the
  single `unsafe` call. `flock { l_type: F_WRLCK, l_whence: SEEK_SET, l_start:
  0, l_len: 0, .. }`, zeroed first; `fcntl(fd, F_GETLK, &mut fl)`; −1 →
  `Err(())` whatever `errno`; held iff `l_type != F_UNLCK`; `l_pid` never read.
  `// SAFETY:` states: the fd is open and borrowed for the call, the struct is
  initialised, the pointer is valid for the call, `F_GETLK` writes only the
  struct.
- `mod.rs`: `probe_file(dir: &Dir-like, name) -> Sample` opens with
  `OpenOptions::new().read(true).custom_flags(O_NOFOLLOW | O_NONBLOCK)` (std,
  no `unsafe`), `file.metadata()` (an `fstat` on the handle) must be a regular
  file, then `getlk::held`. Never creates, never writes, never unlinks
  (R-17 rules 2–3).
- Homebrew: `read_dir` of `<detected prefix>/var/homebrew/locks`, **bounded at
  4,096 entries** (R-17 E2 saw 208; over the bound → `NotDetermined`). A
  missing directory → no lock file exists → nothing held for Homebrew. Each
  entry is opened by joining its file name to the directory; the name is
  checked to be a single leaf (no `/`, not `.` or `..`) — §3 item 10. A
  symlink entry fails `O_NOFOLLOW` → `NotDetermined`.
- MacPorts: `${portdbpath}/registry/.registry.lock`; a missing file → nothing
  held; `portdbpath` unknown (non-default, or not this machine) →
  `NotDetermined`.
- `SystemLockProbe` implements the domain trait over the detected providers
  only. A provider that is not detected is not probed and not reported.
- **Replay and snapshot:** the registry hands back a probe that answers
  `Unknowable` whenever the runner is a replay or the filesystem scope is not
  this machine, without touching the filesystem (INV-15).
- `O_RDONLY` on a `0444` or `root:wheel` file works (R-17 finding 4); no
  elevation (INV-06).

### 4. Application and CLI (G-3)

- `AnalysisSource::concurrent_provider_activity` is replaced by taking the
  probe's sample for `Live`; `Snapshot` stays `Unknowable` and is never asked.
  `status`, `analyze`, `plan`, `migrate` and `restore` show the live sample
  (`concurrency.observed`, `concurrency.not_determined`, both locales; the
  existing `none_observed` text is reworded to R-17's claim).
- Read-only commands only flag it (04: analysis may proceed, possibly
  inconsistent).
- **The refusal** (`crates/application/src/executor.rs` decides,
  `crates/cli/src/executor/` performs, SE-092): after the run lock and the
  recapture, before the marker is written, the sample must be `NoneObserved`;
  `Observed` → `PROVIDER_LOCK_HELD` (exit 7), anything else →
  `PROVIDER_LOCKS_NOT_DETERMINED` (exit 4). And immediately before every
  provider-mutating command, beside the existing run-lock confirmation, a fresh
  sample; anything but `NoneObserved` halts the run before that command with
  the halt reason `PROVIDER_LOCK_HELD` or `PROVIDER_LOCKS_NOT_DETERMINED`
  (`migrate.v1.md`). `REMOVE_SOURCE` therefore dispatches only when the probe
  answered (K's precondition, 04).
- **Tripwires** (`history_preconditions.rs`):
  `mutations_cannot_dispatch_while_a_live_run_still_says_not_checked` is
  rewritten to the real answer: a textual check that the shell calls the
  probe once after the lock and once per mutating command, through one site,
  and a behavioural test against fakes that a dispatching loop with a probe
  answering `NotChecked`, `NotDetermined` or `Observed` dispatches nothing.
  **Kept failing for a build with no probe:** a test that, under
  `THIS_BUILD_DISPATCHES_MUTATIONS`, fails unless the production shell's probe
  is `SystemLockProbe` (not a stub returning `NoneObserved`).
- `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`.

### 5. The lint split and the scan (G-1)

Exactly ADR-0007's Decision:

- `Cargo.toml`: `unsafe_code = "deny"`; `#![forbid(unsafe_code)]` at
  `crates/{domain,policy,application,cli}/src/lib.rs` and
  `crates/cli/src/main.rs`.
- `crates/cli/tests/unsafe_scan.rs` (name to be fixed in G-1):
  - roots: every lib and bin target from `cargo metadata` except `adapters`
    carries the attribute; every crate's `Cargo.toml` keeps
    `[lints] workspace = true`;
  - scan: on comment- and string-stripped code of every `.rs` under `crates/`,
    `unsafe` and `unsafe_code` only in `crates/adapters/src/lock_probe/getlk.rs`;
    `libc` only under `crates/adapters/src/lock_probe/`;
  - the stripper's own tests (raw strings with hashes, nested block comments,
    `'"'`, byte strings, lifetimes) and a self-test planting `unsafe {}` and
    `#[allow(unsafe_code)]` elsewhere, expecting both caught.
- `no_workspace_crate_depends_on_libc_directly` edited on purpose: `libc`
  direct in `macportsbrew-adapters` only (G-2, when `libc` lands).
- G-1 lands with no `unsafe` yet: the scan admits only the not-yet-existing
  module, so G-1 proves the scan on the tree as it is.

### 6. Documentation

- R-17 addendum (`/research-note`): the `EINVAL` observation for a zero
  `l_type` (ADR-0007 asks G to confirm), made with the probe's own code under
  test against a scratch file, never a provider's file; and the Apple Silicon
  location as unverified (no arm64 machine here: owner item, already listed).
- 04 "Concurrency and locking": status rewritten (detector built behind the
  switch, its claims, the asymmetry). 06 and `docs/schemas/README.md`: exit 7's
  row. Schemas: the `NOT_DETERMINED` activity, the halt reasons, the refusal.
- SE-110 amended (what G built); SE-015's addendum; an INV-17 review paragraph:
  Homebrew's lock directory covers every `brew` command that locks, MacPorts'
  registry lock only registry-writing actions (`sync`, `selfupdate`, `clean`
  take none, R-17 :243-247), so a MacPorts `sync` is invisible — declared, not
  equalised by inventing a lock.
- ADR-0004's sentence; ADR-0007's status line notes G implemented it.
- Staged (`docs/passes/16-staged/G/`, §3 item 5): the one-line comment fix in
  `scripts/check-licences.sh:10-11` (`forbid` → `deny` at the workspace,
  `forbid` at the roots). Not blocking: the comment is wrong, the check is not.

## Phasing as pull requests

| PR | Branch | Contents |
|---|---|---|
| G-0 | `wo16/G-0-plan` | this plan; the staged `G/01` comment fix |
| G-1 | `wo16/G-1-lints` | the lint split, the root attributes, the scan and its self-tests |
| G-2 | `wo16/G-2-probe` | `libc` direct in adapters, `lock_probe/`, the data file, the domain variant and trait, the R-17 addendum |
| G-3 | `wo16/G-3-refusal` | analysis and rendering, the refusal and the per-command halt, the tripwires, schemas, 04/06, SE entries final, report section |

## Mutants (isolated worktree, by a subagent; removed afterwards)

1. `F_RDLCK` instead of `F_WRLCK` in the query.
2. A −1 return read as not held.
3. `l_type == F_WRLCK` instead of `!= F_UNLCK` as "held".
4. Drop `O_NOFOLLOW`.
5. Drop the regular-file check.
6. Drop the listing bound.
7. A listing error read as "no lock file".
8. A missing `portdbpath` read as "no lock file".
9. Replay probes the live filesystem (the registry's replay branch removed).
10. `NotDetermined` refuses as `Observed` / is let through.
11. The per-command re-sample removed.
12. `was_checked` true for `NotDetermined`.
13. The scan: drop the string stripping; drop nested-comment depth.
14. The root-attribute test skips `main.rs`.

Each killed or argued equivalent in writing.

## Needs the owner

- `G/01`, the `check-licences.sh` comment (does not block G).
- Apple Silicon and a real held lock: an arm64 machine with both providers
  (already listed) and, optionally, a recording taken while `brew` holds a
  lock. G's tests use a scratch file locked by the test itself (`flock` from
  std, `File::lock`, stable since 1.89) — never a provider's file.

## What this part does not do

No new program or spawn site (the probe is a file read, not a subprocess); no
edit under `crates/adapters/src/process/` or `provider-commands.toml`; no wait
or retry on a held lock; no "stale lock" notion; no unlink; no claim of idle;
no change to `THIS_BUILD_DISPATCHES_MUTATIONS`.

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
| `security-reviewer`, plan round 1 | `G.md` draft, 2026-11-03 | Approve with changes: M1 symlinked lock directory or ancestor followed; M2 scan admits several `unsafe`, `include!`, `#[path]`, cargo config; M3 `mem::zeroed` is a second `unsafe`, pointer argument and SAFETY text; L1 validate `l_type`; L2 the `ENOENT` race; L3 sample before and after the recapture; L4 construction surface and replay; L5 untrusted names; L6 refusal scope; info: exit codes confirmed, digest unaffected, self-inflicted halts, `deny_unknown_fields`, `HOMEBREW_LOCKS` override | All resolved, "Changes from review round 1" |
| `spec-guardian`, plan round 2 | `G.md` with items 1–18, 2026-11-03 | Approve with changes: every round-1 finding resolved; N1 the deviation from the README's compatibility rule must be explicit; N2, N3 mutants; N4, N5 notes | Item 19 |
| `security-reviewer`, plan round 2 | `G.md` with items 1–18, 2026-11-03 | Approve with changes: every round-1 finding resolved; N1 cross-process detection; N2 MacPorts strictness and `read_dir` item errors; N3–N6 notes | Item 20 |
| `spec-guardian`, plan round 1 | `G.md` draft, 2026-11-03 | Approve with changes: B1 the scan fails on the root attributes; B2 one `unsafe` only; B3 analysis stays pure; B4 the order before each command and the token name; B5 no `invariant_NN_` tests named; B6 the schema change is not additive by the README's rule; R1 combining answers; R2 the test seam and the kept-failing test; R3 device nodes; R4 R-17 rule 4; R5 04's MUST met partially; R6 `check-deployment-target.sh`; R7 exit codes recorded, and exit 3 mid-run; L1–L4 | All resolved, "Changes from review round 1" |
| `spec-guardian`, G-1 code | `wo16/G-1-lints`, uncommitted, 2026-11-03 | Approve with changes: 1 ADR-0004, ADR-0007 and SE-110 left false; 2 lint-lowering flags missed (`-W`, `--force-warn`, single-quoted TOML, other `*RUSTFLAGS`); 3 `include` under another name; 4 `#[path]` forms unread; 5 any `target` directory skipped; 6 roots by kind `lib`/`bin` only; 7 `build.rs` checked by target, "first inner attribute" stricter than item 4; 8 "exactly once" is G-2's | 1–7 fixed in G-1 (As built, G-1); 8 handed to G-2 |
| `security-reviewer`, G-1 code | `wo16/G-1-lints`, uncommitted, 2026-11-03 | Bypasses, verified by compiling: B1 raw C strings, B2 long `\u{..}` char literals desync the lexer (also a fake root `forbid`); B3 `include` renamed; B4 `#[path]` forms; B5 nested `target`, non-`.rs` path targets; B6 a macro at the site multiplies it; B7 a dependency's macro expands `unsafe` even under `forbid`, and 8 proc-macro crates are in the graph; B8 flags and wrappers; B9 targets outside `crates/`, other crate kinds | All fixed in G-1 with a planted self-test each, except B7's residue: admitted dependencies' macros, pinned and declared as a limit in SE-110 |

## Changes from review round 1 (binding; they override the Design above where they differ)

1. **The lock directory is never followed (SR M1, SG R3).** Before listing,
   `symlink_metadata` on the lock directory (and on MacPorts' `registry/`)
   must be a real directory; its `(st_dev, st_ino)` is recorded and checked
   again after the listing and every open; a mismatch is `NotDetermined`.
   Each opened file's `st_dev` must equal the directory's. "Missing" is only
   `NotFound` from `symlink_metadata` on the directory; `ENOTDIR`, `EACCES`,
   `ELOOP` and every other error are `NotDetermined`. Entries are pre-filtered
   with `DirEntry::file_type()` (no follow): anything not a regular file is
   `NotDetermined` without being opened, so a device node is never opened; the
   `fstat` on the handle stays as the check against a swap. `O_NOFOLLOW_ANY` is
   not used: macOS 11+ only, and its behaviour on 10.12–10.15 is unverified.
   The remaining window (an ancestor swapped and restored between the two
   identity checks) is declared in 04's threat model. A stray subdirectory or
   symlink in the lock directory refuses every run until removed; 04's status
   says so, and the refusal names the provider and the cause (SR L5), never the
   untrusted entry name unescaped. Mutant 15: the lock directory replaced by a
   symlink.
2. **The `ENOENT` race (SR L2).** An entry listed and then gone at its open is
   "no longer a lock file", not held, provided the directory's identity
   re-check passes; Homebrew's cleanup unlinks lock files (R-17 E1). Declared in
   04. Mutant 16: the identity re-check removed.
3. **One `unsafe` block, exactly (SR M3, SG B2, SG L4).** `libc::flock` is built
   by a complete struct literal in safe code, all five fields of libc 0.2.191's
   Apple definition (`l_start`, `l_len`, `l_pid`, `l_type`, `l_whence`), no
   `..`, no `mem::zeroed`. The argument is `&raw mut fl` typed
   `*mut libc::flock`. `#[allow(unsafe_code)]` sits on the item `held` only;
   no module-level `#![allow]`. `held` returns a named
   `LockState { Held, NotHeld, Unknown }`. After the call, `l_type` must be
   `F_RDLCK` or `F_WRLCK` (held) or `F_UNLCK` (not held); anything else is
   `Unknown` (SR L1; mutant 3 becomes "an unexpected `l_type` read as held").
   The SAFETY comment states: libc's `flock` is the kernel's `struct flock`
   for this target; the third variadic argument is the pointer `F_GETLK`
   reads; the kernel reads and writes that one struct and keeps no pointer
   past the return; `fl` is a local with no other borrow; the `BorrowedFd`
   keeps the fd open for the call. A comment notes that closing the probe's fd
   would drop POSIX record locks this process holds on that file, and it holds
   none.
4. **The scan, precisely (SG B1, SR M2).** Over comment- and string-stripped
   tokens of every `.rs` under `crates/`:
   - the keyword `unsafe` as a whole token (including `#[unsafe(..)]` and
     `unsafe extern`) appears exactly once, in `getlk.rs`;
   - `unsafe_code` inside `allow(..)`, `expect(..)`, `warn(..)` or
     `cfg_attr(.., allow(..))` appears exactly once, on `held` in `getlk.rs`;
   - `forbid(unsafe_code)` appears only as an inner attribute at the five roots;
   - identifiers such as `unsafe_op_in_unsafe_fn` do not match;
   - `libc` only under `crates/adapters/src/lock_probe/`;
   - no `include!` anywhere; no `build.rs` under `crates/adapters`; every
     `#[path = ..]` resolves to a file under `crates/` (today `migrate.rs:719`,
     `restore.rs:550`).
   The root test also asserts the workspace `unsafe_code` is exactly `"deny"`
   and that no `.cargo/config*` carries `-A` or `--cap-lints` rustflags. The
   self-test plants its samples as in-memory strings. ADR-0007's wording ("the
   identifier `unsafe_code` … only in that module") is corrected by an addendum
   in its status line and in SE-110's amendment, not silently.
5. **Analysis stays pure (SG B3).** The sample is taken by the shell after the
   capture (one instant, R-17 rule 4) through `&dyn ProviderLockProbe` and
   passed into analysis as an argument; `AnalysisSource`'s shape and
   serialisation are unchanged; `Snapshot` forces `Unknowable` whatever is
   passed. Affected and updated: `crates/application/tests/invariants.rs:800`,
   `crates/cli/tests/concurrency_flag.rs`, and the `explain`, `doctor`, `list`,
   `analysis`, `plan`, `migrate`, `restore` and `history-list` documents that
   carry the flag. `RunFacts::NotChecked` (`doctor.rs`, `interrupted.rs`) is a
   different type and is not touched.
6. **The order (SG B4, SR L3).** Under the run lock: sample → recapture and
   re-plan → sample; both must be `NoneObserved`. Before each
   provider-mutating command: (for `REMOVE_SOURCE`, the destination re-read) →
   configuration re-check → run-lock confirmation → **lock sample** → dispatch.
   The shell's call is `provider_lock_sample(` (not "probe", which
   `history_store::probe` holds), one site, with an order assertion in
   `wiring_violations`.
7. **Combining answers (SG R1, SR L6).** Over the providers the plan touches
   (with two providers today, both), `Observed` wins (exit 7) and the rendering
   still names any undetermined provider; otherwise any `NotDetermined` gives
   `PROVIDER_LOCKS_NOT_DETERMINED` (exit 4); `NoneObserved` only when every
   touched provider answered not held. A provider whose presence is `Broken`
   is `NotDetermined`, not left out (SG R5). Sets are `BTreeSet` (INV-15).
8. **Exit codes (SG R7).** Before anything mutates: `PROVIDER_LOCK_HELD`
   exit 7, `PROVIDER_LOCKS_NOT_DETERMINED` exit 4. A halt after a mutating step
   was performed ends `PARTIAL_SUCCESS`, exit 3 (INV-03, `migrate.v1.md`). The
   widening of exit 7 is recorded in SE-015, 06, `docs/schemas/README.md` and
   CHANGELOG. A halt report does not imply an external cause: a held lock may
   be a grandchild of our own timed-out provider command (ADR-0006) or an
   inherited fd (R-17 finding 3).
9. **R-17 rule 4 (SG R4).** The pre-check is what DP-030 ruled; rule 4's
   "attempt the provider's own lock-taking command" is how a mutating command
   behaves anyway. Homebrew's `OperationInProgressError` after a clean sample
   is a failed mutating command (`MUTATION_FAILED`, a halt), and a MacPorts
   command waiting for its lock ends at the process timeout (ADR-0006).
   Recorded in SE-015 as the reading of rule 4.
10. **04's MUST, met partially (SG R5).** 04 and SE-015 list the false
    negatives from R-17: Homebrew cask, download and cleanup operations and
    the instants between per-formula locks; MacPorts operations that take no
    registry lock (`sync`, `selfupdate`, `clean`); a provider at a prefix that
    was not detected. A non-default `portdbpath` refuses (conservative).
11. **The schema (SG B6).** No version bump: SE-015 records the evidence that
    no build ever wrote `NONE_OBSERVED` or `OBSERVED` (every live run said
    `NOT_CHECKED`), and `possiblyInconsistent` is carried beside the tag. The
    activity tables gain `NOT_DETERMINED` and the rule "a consumer treats an
    unrecognised `activity` as possibly inconsistent". The two tags' meanings
    are narrowed to R-17's claim. CHANGELOG notes it.
12. **Invariant tests (SG B5).** Named, in G-2 or G-3:
    - `invariant_15_a_replayed_or_snapshotted_run_never_samples_provider_locks`
      (an unreadable scratch lock directory; `Unknowable`, no access), and the
      existing `invariant_15_*` in `migrate.rs:492` and
      `concurrency_flag.rs:269` reworded to lock vocabulary;
    - `invariant_01_no_removal_dispatches_unless_the_lock_probe_answered_immediately_before_it`;
    - `invariant_17_the_lock_probe_names_no_provider` (textual over
      `lock_probe/`: it branches on the data's shape and `base`, never on a
      provider id) and a mirror-direction refusal test;
    - `invariant_06_the_lock_probe_needs_no_elevation` (a `0444` scratch file
      as the test user);
    - `invariant_05_the_lock_probe_creates_nothing` (a scratch directory and a
      missing `.registry.lock` unchanged after a sample);
    - `invariant_20_no_lock_location_is_written_into_source`.
13. **The test seam and the kept-failing tripwire (SG R2, SR L4).** Tests reach
    dispatch through a probe injected at the application seam, never a
    recording (a replay answers `Unknowable` and so halts; part J's plan says
    how its replayed elevated path meets that). `SystemLockProbe` is built
    only from a `DetectedPrefix` and `macports_portdbpath`; no public
    constructor takes a `PathBuf`; the scratch-file seam is `pub(crate)` /
    `cfg(test)`. `is_replay()` is asked once; "replay or not this machine" both
    answer `Unknowable`, each tested. The rewritten tripwire keeps "only the
    owner relaxes this" and adds a textual check:
    `ConcurrentProviderActivity::NoneObserved` is constructed outside tests
    only under `crates/adapters/src/lock_probe/`, and the production shell gets
    its probe only from `registry`. The rewrite is recorded in SE-015.
14. **ADR-0007's remaining obligation (SG R6).** G-2 re-runs
    `scripts/check-deployment-target.sh`.
15. **Records (SG L2).** No SE-113: behaviour in SE-015, mechanism in SE-110.
    The phasing table's G-0 row is "this plan; the staged `G/01`".
16. **Policy version (SG L1).** Bumped only if
    `invariant_20_the_policy_version_names_the_newest_read_date_in_the_data`
    requires it.
17. **The bound (SG L3).** 4,096 is a safety limit, not a provider fact; it
    stays in code with a comment saying so.
19. **Round 2 (spec-guardian).** N1: SE-015 says explicitly that item 11
    deviates from `docs/schemas/README.md`'s compatibility rule, citing
    `migrate.v1.md:71` and `analysis.v1.md:304` ("Never" written) and a textual
    test that no non-test path built `NONE_OBSERVED` or `OBSERVED` before G.
    N2: mutant 3b, an unexpected `l_type` read as not held. N3: mutant 17, the
    `DirEntry::file_type()` pre-filter removed; mutant 18, the per-open
    `st_dev` equality removed. N4: item 13's textual check binds part J — no
    replay probe may answer `NoneObserved`. N5: the refusal tells the user what
    to do (item 1).
20. **Round 2 (security-reviewer).** N1: G-2 proves detection from another
    process. If the textual `Command` check admits it in an integration test,
    a child process holds the `flock` while the probe samples; otherwise the
    R-17 addendum records, observed on this machine with the probe's own code,
    that a `flock` held through a second file description of the same process
    is reported, citing observation C for cross-process visibility, and a
    child-process test becomes an owner item. Item 12 lists whichever is
    built. N2: MacPorts gets the same strictness — `.registry.lock`
    `NotFound` is not held only when `registry/`'s identity check passes
    before and after the open; a missing `registry/` is `NotDetermined` (a
    working MacPorts always has it); an `Err` item while iterating `read_dir`
    is `NotDetermined`, never skipped (mutant 19). N3: a lock file created
    after the listing is not seen; listed with item 10's false negatives. N4:
    the root test asserts from `cargo metadata` that no workspace target is a
    `proc-macro` and that no `build.rs` exists anywhere in the workspace. N5:
    `validate.sh` is not changed by G (guarded); that CI sets no
    `RUSTFLAGS`/`CARGO_ENCODED_RUSTFLAGS` with `-A` or `--cap-lints` is
    checked by the root test reading the environment it runs in; a
    `validate.sh` stage, if G-1 finds the test alone insufficient, is staged
    then as `G/02`. N6: the
    `NOT_DETERMINED` JSON, halt reasons and history carry provider ids and
    cause tokens only, never a path.
18. **Data loading (SR info).** `deny_unknown_fields`; the location is an enum
    (`Directory { dir }` with every entry, or `File { file }`), not two optional
    keys. R-17's addendum records `HOMEBREW_LOCKS`' overridability as
    unverified.

## As built (G-1)

- `Cargo.toml` `unsafe_code = "deny"`; `#![forbid(unsafe_code)]` as the first
  inner attribute of the five roots. `crates/cli/tests/unsafe_scan.rs`, 13
  tests: the scan and one planted self-test per bypass.
- **Deviations from item 4, each stricter:** the `forbid` must be a root's
  *first* inner attribute; a file named `build.rs` anywhere under `crates/` is
  refused, compiled or not (item 20 N4), as well as a `custom-build` target;
  the bare `include` token is refused, not only `include!`; a `path` attribute
  must name a `.rs` file; an unclosed comment or literal is refused; at the
  site, no `macro_rules`/`macro` token and the one `allow` on a `fn` item;
  every target's source under `crates/`; the members' direct dependencies and
  the graph's proc-macro crates are pinned in the test; a `rustflags`, `rustc`
  or wrapper key in `.cargo/config*` and `RUSTC`, `RUSTC_WRAPPER`,
  `RUSTC_WORKSPACE_WRAPPER` are refused outright.
- **Limits, declared in SE-110:** Cargo configuration outside the repository
  is not read; what an admitted dependency's macro expands to is not seen.
- **Item 20 N5, decided:** the test alone is the check; no `validate.sh` stage
  is staged, since `validate.sh`'s default mode and CI both run it.
- **Mutation proof (isolated worktree, a subagent; removed afterwards):** 16
  mutants of the scan, including the plan's 13 (string stripping, nested
  comment depth) and 14 (`main.rs` dropped as a root, and its `forbid`
  deleted). 12 killed at first; the 4 survivors (the site's macro rule, the
  build-directory depth, the dependency pin's comparison, the exact `deny`)
  were each made testable and are now killed by
  `the_site_admits_one_allow_on_a_fn_and_no_macro`,
  `only_a_crates_own_build_directory_is_skipped` and
  `a_drift_in_the_pins_or_the_level_is_caught`, re-run by hand.
- **Handed to G-2:** "at most once" becomes "exactly once" when `getlk.rs`
  lands, the `allow` tied to `held`;
  `no_workspace_crate_depends_on_libc_directly` edited on purpose; the direct
  dependency pin gains `macportsbrew-adapters libc`.
