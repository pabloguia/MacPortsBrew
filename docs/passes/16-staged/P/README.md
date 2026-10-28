<!-- SPDX-License-Identifier: Apache-2.0 -->

# Staged for the owner: what parts D to J need, once (work order 16, part P)

Four patches, each against `main` at `0564fc3`, each applicable on its own and
all together. They change files the session may not change: the licence gate,
the frozen process allowlist (§3 item 3), the stop gate and its tests, and the
fixture matrix. The research behind them is R-21, R-22 and R-23.

| Patch | Files | Needed by | Executed by the session? |
|---|---|---|---|
| `01-licences-libc.patch` | `scripts/check-licences.sh` | E (`sha2`), G | No — written as patch text; the unpatched script passes |
| `02-allowlist-f-h-reads.patch` | `crates/adapters/src/process/mod.rs`, `macports/mod.rs`, `homebrew/mod.rs` | F, H | **Yes**, in a scratch copy: fmt, clippy `-D warnings`, the adapters' tests, `validate.sh --quick` |
| `03-stop-gate-elevation-token.patch` | `.claude/hooks/stop-gate.sh`, `test-hooks.sh`, and their `.codex/` mirrors | J; and a fix to the gate itself | No — written as patch text; checked with `bash -n` and an equivalent harness |
| `04-fixture-matrix-service-rule.patch` | `scripts/run-fixture-matrix.sh` | D | No — written as patch text; its rule checked against snapshots from both builds |

The session may not edit even a scratch copy of a guarded file, so three of the
four were written directly as patch text. That is why running the checks is the
owner's first step.

## Why each is needed

1. **`libc` in `build_scripts_allowed`.** `sha2` 0.11, which part E's digest
   needs, reaches `libc` through `cpufeatures` on Apple Silicon, and
   `check-licences.sh` reads `cargo metadata` unfiltered, so `libc`'s build
   script is seen on every machine (R-22 F-1). Its build script runs
   `$RUSTC --version`, emits `cfg` flags and writes no file. Part E's ADR is
   where the dependency itself is reviewed; this only lets the gate accept it,
   and it does nothing until `sha2` lands (no `libc` is in `Cargo.lock` today).
   ADR-0006 says "no `libc`" until part E's ADR supersedes it (§3 item 6), so
   apply this one with or after that ADR if you prefer the order to be
   literal.
2. **Two reads for the executable list and the integrity file list** (R-21):
   `port -q contents <exactly one port>` and `brew list --formula <exactly one
   formula>`. A new `Positionals::ExactlyOneName` exists because `port contents`
   with no name evaluates the Portfile in the working directory.
   `brew list` now has three exact shapes, and a bare `list <name>` — sent by
   nothing in this build — is refused, which is a tightening. The adapters hold
   the two argv prefixes so the both-direction tests pass; part F is their
   consumer. **Not included:** `otool` (part H stages it after its own research)
   and the elevation entry with the mutating subcommand list (part J stages them
   with its design, which the work order puts before any code; R-23 F-2).
3. **The stop gate admits the elevation program's name only under
   `crates/adapters/src/process/`**, decided by the file's path, with every other
   elevation, service-control and symlink token still refused everywhere,
   `process/` included (R-23 F-4). Six `test-hooks.sh` cases; the `.codex/`
   mirror hunks are the generator's output for the same bodies.
   **It also fixes a defect in the gate as it stands:** under `pipefail`, the
   existing scans' `| head -1 | grep -q .` passed silently when grep met an
   unreadable file or a stage took SIGPIPE on a large output, so a hit could get
   through. All three scans now capture grep's output whole and fail on a read
   error; four `chmod 000` cases cover it (skipped when the tests run as root).
4. **The fixture matrix reads what a replayed plan owes from the snapshot.**
   With part D, a replayed row that does not declare a service is unknown and
   refused (DP-032, DP-016 (c)); the matrix still expects a plan, so part D's
   code cannot pass the commit gate until this lands. The rule is
   build-tolerant: a row with no `service` record (a build before part D) keeps
   today's expectation, so this can be applied first. A row whose answer is not
   `YES` **owes** a refusal: asserted, not tolerated. A `YES` row owes a plan
   that stops at manual review (INV-19), and a `NO` under `--replay` fails,
   because it can only mean the live disk was consulted (INV-15). The branch for
   a build before part D is removed once part D is on `main` (a part D
   follow-up).

## What the owner does

From a clean checkout of `main`, on a topic branch:

```bash
git apply docs/passes/16-staged/P/01-licences-libc.patch docs/passes/16-staged/P/02-allowlist-f-h-reads.patch docs/passes/16-staged/P/03-stop-gate-elevation-token.patch docs/passes/16-staged/P/04-fixture-matrix-service-rule.patch
```

```bash
.claude/hooks/test-hooks.sh
```

```bash
scripts/generate-codex-layer.sh --check
```

(If that reports drift, run `scripts/generate-codex-layer.sh` to regenerate the
mirror rather than hand-editing `.codex/`.)

```bash
scripts/validate.sh
```

If all pass, commit and open the pull request as usual. If one fails, do not
edit around it: revert that patch with `git apply --reverse <patch>` and tell a
session what failed. Each patch can be applied, and reverted, on its own.

To have a session apply them instead, send it a message containing exactly:

> MPB_ALLOW_GUARD_EDIT=1 Apply the four patches in docs/passes/16-staged/P/ on a
> topic branch, run the checks its README lists, and open a pull request.

## Reviews

`security-reviewer`, three rounds: 01 and 02 approved as built; 03 and 04
approved after their fixes (the gate's `pipefail` defect, the misleading
message, the `review` outcome, `NO` under replay, the stale snapshot). Residuals
it records: a symbolic link or `#[path]` can make a file outside `process/` read
as inside it for 03's path rule (the allowlist at the spawn site stays the real
control); `brew list` with no name stays admitted (harmless); `build_scripts_allowed`
matches by name, so a later `libc` version's build script is not re-read.

## Counted as applied

Each patch, when `git apply --reverse --check <patch>` succeeds on
`origin/main`. The ledger records the owner's commit.
