<!-- SPDX-License-Identifier: Apache-2.0 -->

# Staged for the owner: the probe's admission and two script fixes (work order 16, part H)

**Status (2026-11-08).** Applied. PR #70 (`5aa536d`) staged them; on the owner's instruction a session
applied all three on `wo16/H-apply` and `scripts/validate.sh` passed. The patches stay here as the
record of what was reviewed.

| Patch | Files | What it does | Blocks |
|---|---|---|---|
| `01-curated-probe-admission.patch` | `crates/adapters/src/process/` (`mod.rs`, `system.rs`, `recording.rs`, new `probe.rs`) | The admission that would let a run start a package's own curated `--version` probe | H-3 |
| `02-capture-ownership-reads.patch` | `scripts/capture-fixture.sh`, `scripts/test-gates.sh` | A capture records the MacPorts ownership reads | Nothing |
| `03-release-workflow-pipe.patch` | `scripts/check-release-workflow.sh` | Ends a false CI failure | Nothing |

The builders' records: `01-NOTES.md` and `02-03-NOTES.md`.

## `01`: the probe's admission

**What H/01 carries.** The frozen process module only: the admission, `invariant_10_*`, the table
reader and the eleven `admit` refusals of `docs/passes/16-liveness-design.md`. The causes they name
are already reserved outside it, in `UncheckedCause`. **Applying it starts no program**, because
nothing outside the module calls it yet.

`process/` is frozen by the work order (§3 item 3) because it decides what may be started, and this is
growth in what may be started: a package's own program, run as the canonical path with the placed
name as `arg0`, exactly eight environment variables, no inherited `HOME`, a fresh `0700` scratch
directory under the state root, 30 seconds and 64 KiB, its output thrown away. There is still exactly
one `Command::new`. A replay or a recording refuses a probe.

Built in a scratch worktree on `54b56eb` plus `RunLock::path()`: build, `clippy -D warnings`, the
workspace tests (2044 passed), `scripts/validate.sh --quick`, and every mutant of rounds 2 and 3 (58, then 13 and 12 on rounds 3 and 4's lines; one equivalent survivor), all killed. The
workspace run used an equivalent `RunLock::path()`; with #70's exact hunk the adapters crate's 788
tests were run
(`01-NOTES.md`, "As built" lists every refusal and its cause).

**Only a compiled program is admitted:** a thin native 64-bit Mach-O executable for this build's
architecture, or a big-endian fat file every slice of which is a Mach-O executable and one of which
is for this build's architecture (the kernel, not this reader, picks the slice). Anything else is refused, a script with or without `#!`, an empty file, a dylib or
another architecture's program, because macOS runs a file it cannot execute with `/bin/sh`.

**The leftover sweep removes only what a probe made:** directories under `<state root>/probes`
named `probe-<digits>-<6 hex>`, owned by the user, mode `0700`. Anything else there stops the probe
and nothing is removed, so a state directory pointed somewhere unexpected costs a probe, never a file.

**What H-3 owes, in unfrozen code, after `01` is applied.** Only the wiring:

1. A probe method on `DestinationSite` taking `Option<&RunLock>`, defaulting to
   `Unchecked(NOT_OBSERVED)`; the read-only loop passes no lock.
2. `MachineDestinationSite` calls an inherent method of `DestinationSites` that calls
   `run_curated_probe(owner, &state, &lock, package, leaf, argv)`, with the provider taken from the
   adapter. The two adapters implement the crate-internal `ProbeOwner` trait the admission asks for
   the owner rule, and an agreement test reads the table through `process::curated_probe_rows()`.
3. `observed_by_this_build` turns true for `LIVENESS_PROBE` in that same change.
4. The cause `PROBE_TARGET_IS_ENV_SCRIPT` now covers every target that is not a compiled program
   for this build. Its text says so in both catalogues; whether the identifier is renamed before
   the first release is H-3's call.
5. A target the kernel kills at exec (a malformed `MH_EXECUTE` past its header) answers `FAILS`,
   the safe direction but a false report of the package; telling it apart is H-3's, and so is the
   stale test comment `01-NOTES.md` names.
6. `~/Library` is closed first: **this stays open.** `security-reviewer` has not accepted the search
   by name in `docs/research/R-28-liveness-probes.md`, "Re-qualification of rows 1-13".

**What the version pin costs.** After any upgrade of a package with a row, its probe answers
`PROBE_VERSION_NOT_QUALIFIED`, so the check is unchecked and the source is kept, until the row is
re-qualified. The plan does not warn about it beforehand.

## `02`: the ownership reads in a capture

It records `port -q provides <path>` for each executable the capture's `-q contents` lists, and
`port -q installed <name>` for each installed port, with the adapters' exact argv; `test-gates.sh`
gains drift cases that keep the two in step. **Apply both files together:** the new test cases fail
against the old capture script. Two limits, in `02-03-NOTES.md`: `--replay` does not reach these reads
today (the adapter answers `NOT_OBSERVED` under a replay), and a full capture on a machine with a
large output still fails on `jq`'s argument limit, the existing owner item.

## `03`: the release-workflow pipe

`scripts/check-release-workflow.sh` fed three early-exiting readers (`grep -q`, `grep -Eqx`,
`head -1`) through a pipe under `pipefail`. When the reader exits before the writer has finished, the writer gets
SIGPIPE and the check reports a false failure: every time locally once the input passes the 64 KiB
pipe buffer, intermittently below it, as on PR #69's `macos-26` job on 2026-11-04 with about 26 KB
(`printf: write error: Broken pipe`). They read from a here-string now. Reproduced: 300 of 300 false failures above about 71 KB with
the old form, none with the new. Other guarded scripts with the same shape are listed in
`02-03-NOTES.md`, not patched.

## What the owner does

From a clean checkout of `main` that contains pull request #70 (it carries `RunLock::path()`, which
`01` needs to compile), on a topic branch:

```bash
git fetch origin && git switch -c wo16/H-apply origin/main
```

```bash
git apply --check docs/passes/16-staged/H/01-curated-probe-admission.patch docs/passes/16-staged/H/02-capture-ownership-reads.patch docs/passes/16-staged/H/03-release-workflow-pipe.patch
```

```bash
git apply docs/passes/16-staged/H/01-curated-probe-admission.patch docs/passes/16-staged/H/02-capture-ownership-reads.patch docs/passes/16-staged/H/03-release-workflow-pipe.patch
```

```bash
scripts/validate.sh
```

The default mode runs the tests, the gate tests and shellcheck, which cover all three. If it passes,
stage the files (`git apply` leaves the new `process/probe.rs` untracked, and a commit with a
pathspec would leave it out) and commit:

```bash
git add crates/adapters/src/process scripts/capture-fixture.sh scripts/test-gates.sh scripts/check-release-workflow.sh
```

```bash
git commit -m "feat(process): the curated probe's admission, and two script fixes (part H)"
```

Then open the pull request as usual.

Each patch may also go alone; `02`'s two files go together. If a check fails, do not edit around it:
`git apply --reverse` what was applied and tell a session what failed.

To have a session apply them instead, send it a message containing exactly:

> MPB_ALLOW_GUARD_EDIT=1 Apply docs/passes/16-staged/H/01-curated-probe-admission.patch,
> docs/passes/16-staged/H/02-capture-ownership-reads.patch and
> docs/passes/16-staged/H/03-release-workflow-pipe.patch on a topic branch, run the checks their
> README lists, and open a pull request.

## Counted as applied

Each patch, when `git apply --reverse --check <patch>` succeeds on `origin/main`. The ledger records
the owner's commit. H-3 starts on `wo16/H-5-probe` only after `01` is applied, and its probe stays
off until `~/Library` is closed.

## Reviews

| Reviewer | Reviewed at | Verdict | Resolution |
|---|---|---|---|
| `security-reviewer`, round 1 | `01` built on `54b56eb` (sha256 `7997598b…`) | Pass with changes: M1 the sweep removed anything under `<state root>/probes`; M2 H-3 could not reach the module; L1-L5 | All fixed (`01-NOTES.md`, "Review round 1") |
| `spec-guardian`, round 1 | the same | Approve with changes: F1 reachability, F2-F7, R1-R4, spec 04 wording | All fixed |
| `security-reviewer`, round 2 | `b2a2351` | Pass with changes, nothing blocking: L1 a `#!` interpreter outside the prefixes was admitted unless named; L2 the euid oracle reads `TMPDIR`; L3 known prefixes omit a detected custom prefix; I1-I4 | L1 fixed. L2, L3, I1 open for H-3 and part L (SE-113); I2 commented; I3, I4 for H-3 |
| `spec-guardian`, round 2 | `b2a2351` | Approve with changes: F1 the owner's commit left the new `probe.rs` out; F2 the staging PR is not docs-only; F3, F4 undeclared deviations; F5 stale notes; F6 no `invariant_05_*` probe test; F7 status | All fixed: the commit stages first; SE-113 declares F2-F4; `invariant_05_a_probe_never_starts_a_provider_program` |
| `security-reviewer`, round 3 (confirmation) | `0264e35` | Not closed: without `#!`, macOS runs a non-Mach-O file with `/bin/sh`, so text, an empty or 1-byte file, or `#!` after a BOM or whitespace was admitted and run | Fixed: condition 11 admits only a Mach-O `MH_EXECUTE` for this build's architecture |
| `security-reviewer`, round 4 (confirmation) | `015be5e` | Not confirmed: a little-endian fat header, or a fat file whose slice the kernel prefers (x86_64h here, arm64 under Rosetta) is not an executable, was admitted and handed to `/bin/sh` | Fixed: big-endian fat only, every slice a native Mach-O executable, one for this build |
| `security-reviewer`, round 4 confirmation | `58a9e52` | Confirmed: no admitted file reaches `ENOEXEC`, so no shell runs. Notes: a stale test comment; a malformed `MH_EXECUTE` killed at exec answers `FAILS` | Recorded for H-3 (`01-NOTES.md`, "Round 4 confirmation"); the patch is left as built |
| `docs-auditor` | `0264e35` | 14 findings: the cause's display text became false; the SIGPIPE explanation; H.md "docs only"; undated claims; stale lists | Fixed; the cause's text is reworded in both catalogues, its identifier left for H-3 |
