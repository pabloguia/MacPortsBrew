# ADR-0007 — `sha2` for the plan digest, and `libc` with one reviewed `unsafe` call for the lock probe

- **Status:** accepted, 2026-11-01, in work order 16 part E's step E-4, the
  change that adds `sha2` to `Cargo.toml`. The choice is the session's to make
  under work order 16 §4.3; the plan's reviewers approved it
  (`docs/passes/16-plans/E.md`, Reviews), and the owner applied P/01 first
  (`4a64aa0`, PR #50).
- **Date:** 2026-10-30
- **Pass:** 16-complete-mutating-product, part E
- **Supersedes:** ADR-0006, **for the `sha2` dependency and the lock probe
  only**. Every other part of ADR-0006 stands: the timeout kills one process,
  and nothing in `crates/adapters/src/process/` uses `libc`. ADR-0006's status
  line says so.

## Context

Two rulings of 2026-10-27 (SE-109) need code that the workspace cannot write
with `std` alone.

**DP-049: the executed plan is tied to the one shown.** The script route,
`migrate --yes --plan-digest <d>`, carries "what was shown" across a process
boundary as a digest. DP-049 (c), a hand-written hash, was rejected: it puts
cryptography in the safety path, and a non-cryptographic hash admits collisions
someone who controls an inventory could craft. `std` has no cryptographic hash.

**DP-030 (b): part G probes the providers' own locks before a mutation.** What
the probe must call is fixed by R-17 (`docs/research/R-17-provider-mutation-locks.md`,
findings 4 and 5; E4 observations B and C; 2026-10-05): the held-lock test
is `fcntl(F_GETLK)` on an `O_RDONLY` descriptor (observation C). A shared
`flock(LOCK_SH|LOCK_NB)` is refused (observation B) because the probe's own hold makes a concurrent Homebrew
`LOCK_EX|LOCK_NB` taker fail, which aborts the user's `brew install`. `std`
offers `File::try_lock_shared` (the `flock` shape R-17 rejects) and no
`F_GETLK`.

**What is measured** (R-22, `docs/research/R-22-dependency-sets-for-e-and-g.md`,
2026-10-28, macOS 26.7.1 x86_64, toolchain 1.98.0, scratch crates; and two
registry reads on 2026-10-30, recorded in the plan §0.2):

- `sha2 = { version = "0.11.1", default-features = false }` brings `digest`,
  `block-buffer`, `crypto-common`, `hybrid-array`, `typenum`, `cpufeatures`,
  `cfg-if` and `libc`. All are `MIT OR Apache-2.0`. `libc` is built only for
  `aarch64-apple-darwin` (through `cpufeatures`), but it is in the unfiltered
  `cargo metadata` that `scripts/check-licences.sh` reads on every host.
  `Sha256::digest` needs no feature (R-22 F-1).
- Only `libc` among those ships a `build.rs`. Its script (0.2.191) runs
  `$RUSTC --version`, emits `rustc-cfg`/`rustc-check-cfg`, spawns a program only
  on FreeBSD and Emscripten targets, and writes no file — the idiom ADR-0004
  already accepted (R-22 F-4).
- The open G needs (`O_RDONLY | O_NOFOLLOW | O_NONBLOCK`) and the regular-file
  check (`File::metadata` is `fstat` on the handle) are safe code with `libc`
  supplying constants only (R-22 F-2). **R-22 F-2's conclusion that G needs no
  `unsafe` does not cover the held-lock test**, which R-17 requires and which is
  a `libc` function call.
- The safe wrappers: `rustix` 1.1.6 exposes `process::fcntl_getlk`
  (`rustix-1.1.6/src/process/fcntl_getlk.rs:21`, compiled for Apple targets);
  `nix` 0.31.3 exposes `fcntl(fd, FcntlArg::F_GETLK(..))`
  (`nix-0.31.3/src/fcntl.rs:752, 862, 881`). Each adds its own crate with a
  build script, plus `bitflags` and one or two more, and still pulls `libc`
  (R-22 F-3; registry reads 2026-10-30).

**Measured again in E-4** (2026-11-01, `cargo tree -e normal,build --target <t>`
in the workspace): the set is as above, with `cfg-if` resolved at 1.0.4, the
version already in the lock file; `libc` 0.2.191 appears under `cpufeatures`
for `aarch64-apple-darwin` only. `scripts/check-deployment-target.sh` on the
release build: `x86_64-apple-darwin` records 10.12 and `aarch64-apple-darwin`
11.0 for both binaries — the floors hold.

## Options considered

### For the digest

**`sha2`, `default-features = false`.** RustCrypto's SHA-256, the reference
implementation in the ecosystem; nine small crates including `sha2`, and one
build script.
Makes the script route possible and a pinned digest testable. Costs one
dependency set under ADR-0004 and `libc` in the graph. Forecloses nothing: the
digest's format carries a version prefix, so another hash later is a new
prefix.

**A hand-written SHA-256.** No dependency. Cryptography written and maintained
here, in the safety path; DP-049 (c) rejected it.

**No digest; the interactive route only.** No dependency. Fails DP-049 (a),
which is part of the ruling for v0.2.0.

### For the lock probe

**`libc`, one reviewed `unsafe` call in one module.** `libc` is already in the
graph through `sha2` and admitted by P/01's single `build_scripts_allowed`
entry. G adds it as a direct dependency of `crates/adapters` only. The `unsafe`
is one `fcntl(fd, F_GETLK, &mut query)` on a stack `libc::flock` the module
fills itself, the descriptor borrowed from a live `File` for the call, the
return value checked (the query's exact form is in the Decision). Adds no crate, no build script and no owner patch. Costs
the lint: `#[allow(unsafe_code)]` does not compile under `forbid`, so the
workspace table's `unsafe_code` is lowered to `deny` and the four crates that
need no `unsafe` restore `forbid` at their roots (Decision). Makes the whole new `unsafe` surface a few
lines this project reads line by line. Makes harder: every later `unsafe` is a
`deny` away instead of a `forbid` away, which the single-site scan below
compensates for.

**`rustix` 1.1.6, `process::fcntl_getlk` (safe).** No `unsafe` in our code,
`forbid` kept. Adds `rustix` (a build script that compiles probe snippets with
`$RUSTC`), `errno`, `bitflags`; a second `build_scripts_allowed` entry and a
second owner patch. The `unsafe` it holds is far larger than ours and outside
our review.

**`nix` 0.31.3, `fcntl` with `F_GETLK` (safe).** As `rustix`: `nix` (build
script), `cfg_aliases`, `bitflags`, plus `libc`; the same staging cost.

**A leaf crate for the one call** (for example `crates/lock-probe-sys`, its
own `[lints]` table, `adapters` depending on it). Every layer crate, `adapters`
included, keeps `forbid` by the compiler. Costs a sixth crate for one call and
a new edge in `scripts/check-layering.sh`, a guarded file, so one more owner
patch; and the crate would be one `adapters` alone may use, which the layering
script would have to learn. Rejected: the crate-root `forbid` below gives the
same compiler guarantee to the four crates above the machine boundary, and
`adapters` is already the one layer allowed to touch the machine, under the
scan.

**`flock(LOCK_SH|LOCK_NB)` through `std`.** No dependency, no `unsafe`.
Rejected by R-17: the probe would break the user's concurrent Homebrew run.

## Decision

Add `sha2` 0.11.1 with `default-features = false` for the plan digest, and let
part G call `fcntl(F_GETLK)` through `libc` in one reviewed `unsafe` call in a
single module, lowering the workspace's `unsafe_code` from `forbid` to `deny`
in that change and restoring `forbid` at the roots of every crate but
`adapters`.

**The query, exactly** (R-17 observation C): `l_type = F_WRLCK`,
`l_whence = SEEK_SET`, `l_start = 0`, `l_len = 0`. The lock is held if and only
if the returned `l_type != F_UNLCK`. `l_pid` is never relied on (it is −1 for a
`flock` holder). A return of −1, whatever `errno`, is unknown. `F_RDLCK` is
wrong: it misses a shared holder (observation C). A zero `l_type` is not one of
the lock types (`F_RDLCK`, `F_UNLCK`, `F_WRLCK`), so the call is expected to
fail, presumably with `EINVAL`; G confirms it by observation and records it in
R-17. Whatever it returns, −1 is unknown.

**The single site, by the compiler where it can be.**

- `Cargo.toml`'s `[workspace.lints.rust] unsafe_code = "deny"`; the crate
  roots of `domain`, `policy`, `application` and `cli` (`cli`'s `lib.rs` and
  `main.rs`) carry `#![forbid(unsafe_code)]`, so an `allow` there does not compile. A test
  asserts those four attributes and that every crate's `Cargo.toml` keeps
  `[lints] workspace = true`.
- In `adapters`, the call lives in one module (proposed
  `crates/adapters/src/lock_probe/getlk.rs`) with the crate's only
  `#[allow(unsafe_code)]`, a `// SAFETY:` comment, and
  `#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]`.
- A scan over every `.rs` file under `crates/` (sources, tests, build scripts),
  on comment- and string-stripped code: the keyword `unsafe` and the identifier
  `unsafe_code` appear only in that module; the identifier `libc` (which catches
  `use libc as c` and `extern crate libc`) only under
  `crates/adapters/src/lock_probe/`. That keeps ADR-0006's process decision
  intact: `process/` never signals a group. The stripper is a real lexer or
  carries its own tests (raw strings, nested block comments, a `'"'` literal),
  and a self-test plants an `unsafe {}` and a lint-list `allow` and expects both
  caught; the root-attribute test reads every lib and bin target from
  `cargo metadata`.

## Consequences

- **What lands where.** E-4 adds `sha2` to `[workspace.dependencies]`, used by
  `crates/application` only (`crates/domain` stays `serde`-only), and a test,
  `no_workspace_crate_depends_on_libc_directly`, that G edits on purpose to
  admit `crates/adapters` alone. Until G, `libc` is only transitive.
- **G's obligations.** `libc` direct in `crates/adapters` only; the query, the
  lint split, the crate-root attributes, and the scan exactly as the Decision
  states; `security-reviewer` reads the call line by line; mutants include a
  query with `F_RDLCK` and `EINVAL` read as not held; SE-110 amended in the
  same change; ADR-0004's sentence "The workspace's own code is
  `unsafe_code = "forbid"`" corrected; and a staged one-line fix of
  `scripts/check-licences.sh`'s comment that says `forbid` (a guarded file).
  G also waits on this record being accepted (E-4) and on P/01.
- **Owner action.** P/01 (`docs/passes/16-staged/P/01-licences-libc.patch`)
  admits `libc`'s build script. Until the owner applies it, a commit adding
  `sha2` fails `check-licences.sh`, so E-4 waits; nothing else in E does.
- **Negative.** The workspace's own code gains its first `unsafe` (in G), and
  `adapters` loses the compiler's `forbid`. A future `unsafe` needs only a reviewer's attention to
  slip in under `deny`; the scan is what makes that a test failure rather than
  a review question.
- **Expensive later.** Removing `sha2` changes the digest format, so a digest
  printed by one build is never accepted by another anyway (the version is in
  the digest's prefix), and the cost is a code change only. Restoring `forbid`
  needs either `std` gaining `F_GETLK` or a wrapper crate, each a change to
  this record.

## The `unsafe` surface of the new dependencies

ADR-0004's gate reviews the dependencies' `unsafe` in the record that admits
them (`scripts/check-licences.sh`'s header). Counts of the word `unsafe` in each
crate's source (registry, 2026-10-30): `sha2` 0.11.1, 39 — the SIMD and
`aarch64` SHA intrinsics, selected at run time through `cpufeatures`;
`hybrid-array` 0.4.15, 43 — array reinterpretation behind safe constructors;
`block-buffer` 0.12.1, 22 — buffer handling; `cpufeatures` 0.3.1, 11 — the
`sysctlbyname` call on Apple arm64; `libc` 0.2.191 — foreign-function
declarations only, no bodies. `digest`, `crypto-common`, `typenum` and `cfg-if`
are reviewed as RustCrypto/ecosystem staples with no platform calls on our
path. All of it is RustCrypto's or the Rust project's maintained code; none of it
runs in a process this tool spawns, and none touches the filesystem.

## Compatibility impact

- **macOS 10.12 x86_64 / 11.0 arm64 (ADR-0002).** `sha2`'s set uses no
  platform API on x86_64; on arm64, `cpufeatures` reads CPU features through
  `sysctlbyname`, which macOS has offered since well before 11.0 (confirmed:
  E-4's floor check records 11.0 on arm64 and 10.12 on x86_64, 2026-11-01). `fcntl(F_GETLK)` is POSIX and
  present on both floors; the same query serves MacPorts' registry lock
  (R-17 finding 2) and Homebrew's lock files. This machine is x86_64 and `libc` builds only for
  arm64 until G, so E-4 ran `scripts/check-deployment-target.sh` (both
  targets: 10.12 and 11.0, unchanged); G runs it again.
  A raised floor rejects this record rather than the floor.
- **Licences (ADR-0001).** Every crate added is `MIT OR Apache-2.0` (R-22 F-1,
  2026-10-28); none is GPL. `deny.toml` bans none of them.
- **Architectures.** `libc` is compiled for arm64 only until G makes it a
  direct dependency, after which it is compiled for both.
- **Provider versions.** The probe's lock locations are data, read from
  `data/policy/` (G); this record decides only the mechanism.

## Revisit when

- the probe needs a second `libc` function, or another module wants `unsafe`;
- `std` stabilises an `F_GETLK` wrapper — then the `unsafe` is removed and
  `forbid` restored;
- `sha2`'s major version changes its dependency graph, or a crate in it starts
  shipping a build script.
- a published `planDigest` turns out to matter as a fingerprint of unredacted
  data (a guessed home path can be confirmed against it): then a keyed digest,
  at the cost of the two-process replay determinism test.
