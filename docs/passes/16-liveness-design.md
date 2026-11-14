<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16 part H — how `LIVENESS_PROBE` runs a program off the allowlist

Written 2026-11-04, at `9b8d68d`. The design note `prompts/16-complete-mutating-product.md` part H item 2 requires
before any code. `security-reviewer` passes it first (Reviews, at the end). The
facts are [R-28](../research/R-28-liveness-probes.md); the ruling is DP-051.

**The problem.** `crates/adapters/src/process/` has one spawn site, and it starts
six programs named by literals. A probe runs an installed package's own
executable. No literal can name it, and the list must not grow to admit it.

## What R-28 changes about the ruling's reading

1. **A flag is not safe by default, and exit 0 does not show it was.**
   `httpd --version` started a server and exited 0: status, pipes, scratch and
   group all looked clean. **The curated table is the only protection against
   that class**, so a row is the security decision and is held to what was seen.
2. **"A scratch working directory" is not enough.** Programs write under `HOME`
   and `TMPDIR` (`gh`, `go`, `links`, `mise`, `npm`). One fresh directory per
   probe serves as all three.
3. **Nothing here detects a daemon.** Its parent exits 0 and it leaves its own
   group, so no timeout fires and `libc` would not reach it. **A passing probe
   does not exclude a started server**, and the docs say so. The controls are the
   observed-only rows, the exact version, the closed flag set and the stateful
   exclusion. ADR-0006's "nothing it spawns is expected to fork" gets an amendment.

## The admission

**The rule stays one rule at one site.** `PERMITTED_PROGRAMS`,
`PERMITTED_SUBCOMMANDS`, `SYSTEM_PROBES` and `is_permitted_invocation` are
untouched, and still refuse every package executable. A separate question sits at
the same site:

```rust
// process/mod.rs
pub(in crate::process) fn may_spawn(spec: &CommandSpec) -> bool {
    match &spec.admission {
        Admission::Allowlist => /* today's rule, with PERMITTED_IN_TESTS under cfg(test) */,
        // Never consults PERMITTED_IN_TESTS. Shape only: the site cannot prove
        // prefix membership, which `admit` established.
        Admission::CuratedProbe { .. } => is_admitted_probe_shape(spec), // program and arg0's leaf
    }
}
```

`Admission` is a private field, and `Admission::CuratedProbe { scratch, arg0, target }`
carries the directory, the placed leaf and the recorded identity, so an allowlist spec
cannot carry any of them and the `port current`
protection stays structural (`system.rs:260-272`). Only
`CommandSpec::curated_probe(AdmittedProbe)` produces it. **A spec can never be
widened afterwards:** the site refuses a probe whose timeout or output cap exceeds
the constants (`with_timeout` is `pub`), or whose environment keys are not exactly
the set below.

**No capability crosses a crate.** `adapters` cannot depend on `policy` or
`application`, so a type the judge hands back would need a public constructor that
anyone could call. And **the probe is not a method of `ProviderRead`:** any holder of
that trait (`capture.rs`, a future `doctor`) could then start a third-party program with
one call, past the gate, the consent and the run lock.

*(Not as built: H-1 built this trait and H-3 removed it; see "As built" below.)*

```rust
// domain: a trait of its own, not a supertrait of ProviderRead
pub trait DestinationProbe {
    fn liveness_probe(&self, package: &PackageIdentity, leaf: &ExecutableName,
                      argv: &[String]) -> ProbeObservation;
}
```

**Amended (2026-11-05, work order 16 H-2 round 6; read by `security-reviewer` 2026-11-08 at `b2a2351`, round 2 of the staged patch, no objection).** The trait above goes when H-3 lands, because its one implementor could never name the lock. The probe becomes `answer_probe(&self, leaf, argv, lock: Option<&RunLock>)` on `DestinationSite` in `crates/cli/src/executor/`, defaulting to `NOT_OBSERVED`, and an inherent method on `DestinationSites` taking `&RunLock`. It stays off `ProviderRead`, for the reason above.

It is obtained only from `registry::destination_sites(ctx, &StateDir, &RunLock)`, which
is added to the banned list of `crates/cli/tests/executor.rs` (`.claude/rules/executor.md`
rule 1: read sites are built after the gate passes). The read-only loop passes no such
site, and answers `NotObserved` (`analysis_and_dry_run_never_reach_a_probe`). Requiring
`&RunLock` gives the state root and makes the leftover sweep safe under that lock.

The adapter re-runs its own owner rule immediately before spawning, reads the table
itself (`process/probe.rs`, `include_str!` and a strict parser, as `prefixes.rs` does;
an agreement test against the `policy` loader lives in `crates/cli/tests`, through a
parameterless read-only accessor, which is not the seam forbidden below), and takes
prefix, Cellar and scope from itself, never from a caller. **`AdmittedProbe`** is
`pub(crate)`, private-fielded, with no `Clone`, `Default` or serde. `CommandSpec` derives
`Clone`, so the spec is built and run inside `process/probe.rs`, and `curated_probe` is
`pub(in crate::process)`. The application judge produces the *answer* from the
observation; it never produces the permission.

**`AdmittedProbe::admit` refuses unless all of these hold.** Each refusal is a named
`ProbeObservation::NotRun(cause)`, which answers `Unchecked`, never a spawn.

**The causes (reserved in the enum, work order 16 H-2 round 7; answered by nothing yet).** By condition: 1 `NOT_OBSERVED`; 2 `PROBE_AS_ROOT`; 3 `PROBE_LEAF_REFUSED`; 4 `NO_CURATED_PROBE` with no row, `PROBE_ARGV_NOT_THE_ROWS` otherwise; 5 `PROBE_ARGV_NOT_PERMITTED`; 6 `PROBE_VERSION_NOT_QUALIFIED`; 7 `PROBE_ENTRY_NOT_THE_PACKAGES`; 8 `PROBE_TARGET_NOT_THE_PACKAGES`; 9 `PROBE_TARGET_MODE_REFUSED`; 10 `PROBE_PROGRAM_EXCLUDED`; 11 `PROBE_TARGET_IS_ENV_SCRIPT`. The spawn failing is `PROBE_NOT_STARTED`, and the table below's unnamed `Unchecked` rows, a failed wait and a read error on a pipe, are `PROBE_END_NOT_READ`.

| # | Condition | Why |
|---|---|---|
| 1 | The filesystem scope is `ThisMachine` and the runner is the live one | INV-15 |
| 2 | `euid != 0` | INV-06, as `run_lock.rs` and `history_store.rs` refuse root |
| 3 | `leaf` passed `ExecutableName::parse` | §3 item 10 |
| 4 | A row exists for exactly `(provider, package, leaf)`, and the plan's `argv` equals it | The row is the decision; the plan cannot widen it |
| 5 | `argv` is exactly one element of the closed set `--version`, `-version`, `-V`, `-v`, checked in the loader, here and at the site | `argv = []` is the server-start case. `--init`, `--daemon` and `-D` would pass a character class. Growing the set is a reviewed code change |
| 6 | The installed version, from the owner rule's own answer, **equals the row's `observed_version` exactly**, revision included | A row observed at 3.5.3 says nothing about 4.0. The safe direction; DP-057 asks whether to widen it |
| 7 | The placed entry `<prefix>/<bin or sbin>/<leaf>`, built from typed parts, is owned by this package, by the owner rule, re-run now | §3 item 10 |
| 8 | The canonical path is the destination package's by the owner rule: inside the keg; and where the rule is a query, it is asked of the placed path **and** the canonical path, and both answer this package (`lzcat` is a link to `xz`; the same argv form). Not merely under the prefix, which admits `Caskroom`, `var/`, `Homebrew/` | §3 item 10; a lenient reading would admit a link into another port |
| 9 | On one handle or one `lstat` of the canonical target: a regular file; no setuid or setgid; owned by this uid or root; no group or world write | INV-06. The mode bit is the only check possible, since `invariant_06_*` refuses the elevation program's name in source |
| 10 | Neither the placed nor the canonical leaf is a provider program, a system probe, a shell or an interpreter | Defence in depth, and `bin/foo -> libexec/python3` |
| 11 | The target's first bytes are not an `env` shebang | The table excludes them when written; a later version can become one |

**A package just shown to link into the other provider is not run, and the adapter cannot enforce that:** it does not hold the other checks' answers, and a flag passed in would be decorative. It is a pure function in `application::executor`, `next_destination_question(plan, position, answers) -> Question`; the shell may call the probe only with what `Question::Probe { leaf, argv }` hands it. Tested there, with the mutant "a probe after a `Fails`".


**What runs:** the **canonical path, with `arg0` set to the placed leaf**
(`CommandExt::arg0`). That is "the resolved executable" of §3 item 10, and closes a
re-pointing of the link between `admit` and spawn, while a program dispatching on
`argv[0]` (`lzcat`, a link to `xz`) still does what its name says. Rows are qualified
in the same form. **The remaining race is declared:** macOS has no `fexecve`, so the
target file itself could be replaced. `(dev, ino, size, mtime, ctime, mode)` from `lstat` is recorded at `admit`
and re-read immediately before spawn (in `system.rs`) and after exit (in `probe.rs`); any difference is
`Unchecked (PROBE_TARGET_CHANGED)`, never `Holds`.

**A shim is never the program,** and the shim directory is not on the probe's `PATH`.

## How it runs

| Aspect | Value | Source |
|---|---|---|
| Environment | `env_clear`, then exactly `PATH`, `LANG=C`, `LC_ALL=C`, `TERM=dumb`, `NO_COLOR=1`, `HOME`, `TMPDIR`, `PYTHONDONTWRITEBYTECODE=1`, and `CFFIXED_USER_HOME` if qualification confirms it | R-28 findings 8, 13 |
| `PATH` | The destination's `bin`, then `SYSTEM_PATH`; never the other provider's | INV-10, observed: MacPorts' `spit` passes only with Homebrew's `bin` leaked |
| `HOME`, `TMPDIR`, working directory | One fresh directory per probe | R-28 findings 9-11 |
| The directory | `<state root>/probes/<random>`. `probes` joins `StateDir::directories()`, so it has the existing ownership and mode checks and a planted link refuses. `mkdir` `0700`, failing if it exists. Admission refuses a state root under a provider's prefix (`STATE_DIR_VARIABLE` can point there). An RAII guard removes it with `std`'s `remove_dir_all`; a removal failure does not change the answer; the next run sweeps leftovers | A crash or surviving daemon must not leave hostile content for ever Both refusals, the state root and the `mkdir`, answer `PROBE_NOT_STARTED`. |
| stdin | `Stdio::null()` | Already the runner's |
| Timeout | 30 s, a constant the site enforces. ADR-0006's one-process kill: a wrapper's child survives it, accepted, and stated in the ADR's amendment | R-28 finding 19, a HYPOTHESIS from one machine, and why a timeout is not `Fails` |
| Output | Read and bound at 64 KiB, then dropped: never parsed, stored, recorded or shown | "Only the exit status counts". **Departs from R-28 finding 22**, which keeps the dyld line: a later part may show it, redacted, once it has a reason |
| Elevation | None: condition 2, and no elevated form exists | INV-06 |

**No record, either way.** `ReplayRunner::run` answers by leaf and argv, so a recording
holding `htop --version` would otherwise answer one: it refuses
`Admission::CuratedProbe`. `SystemRunner::recording()`'s closure would write probe
output and the scratch path into a fixture: it refuses too. A cause is an identifier;
`NotStarted`'s text carries the program path, which can be under a home-based prefix,
and reaches neither `--json` nor history.

## What the answer is

An exhaustive match on `ExitStatus`, with no wildcard and not `success()` plus a flag.

| Observation | Answer |
|---|---|
| Exit 0, pipes closed, target unchanged | `Holds` |
| Exit non-zero, or a real signal | `Fails`: the step halts, the source is kept |
| The timeout | `Unchecked (PROBE_TIMED_OUT)`: 30 s is a hypothesis, and "could not tell" is never `Fails` |
| `Cancelled` (kept for exhaustiveness; no token is wired, so unreachable today, and `PROBE_END_NOT_READ` if it were reached); `Signalled { signal: None }` (the wait failed) | `Unchecked` with its cause |
| Exit 0, drain gave up (a pipe held) | `Unchecked (PROBE_LEFT_A_PROCESS)`. **Tightens "only the exit status counts"**, recorded in the SE entry: something outlived the probe, so the row is wrong |
| Exit 0, over the output cap only | `Unchecked (PROBE_OUTPUT_OVER_CAP)`. **`system.rs` does not tell these apart today:** the readers report one `truncated`, so they gain a three-way ending (complete, over cap, gave up), with allowlist behaviour unchanged |
| A read error on a pipe | `Unchecked` |
| No row; a skip in the plan | `Unchecked (NO_CURATED_PROBE)` |
| An `admit` refusal, or the spawn failed | `Unchecked` with its named cause |
| `--replay`, `--from-snapshot` | `Unchecked (NOT_OBSERVED)` |

## The curated table

`data/policy/liveness-probes.toml`, keyed `(provider, package, executable)`, since a
leaf is not a program: `pkg-config` is pkgconf 3.0.7 under one provider and 0.29.2
under the other. Fields: `argv`, `observed_version`, `source`, `read_on`. Never a path
or a program. **Observed rows only:** `cited` is dropped, because a citation establishes
none of what qualifies a row (`httpd`'s docs document `-v` and are silent on
`--version`), so R-28's rows 14-15 (`rg`) do not land and `rg` stays unchecked with its
source kept. Part of the policy version: plan bytes change (INV-15).

**The loader fails closed:** at most one row per key; `argv` in the closed set; a
package any `stateful-packages.toml` family lists, **under any provider**, is a load
error (`httpd` lists only one).

**What qualifies a row,** in R-28's four rounds, each row in the form it will run: the
canonical path with `arg0`, a scratch path of the state directory's real shape (with
its space, and long); exit 0 under the destination's `PATH` and under `SYSTEM_PATH`;
no process left; nothing written outside scratch, **`~/Library` searched** (R-28 did
not; CoreFoundation may resolve home without `HOME`); no network; no `/usr/bin` shim
reached. Not an `env` script, an interpreter, a daemon or a wrapper passing `argv` on.
R-28's rows 1-13 are re-qualified this way before the file lands; a row that fails
does not land.

## What does not change

No new `Command::new` site. No shell. `the_shipped_binary_may_spawn_exactly_six_programs`
stays true of the list, gaining a comment and a spec sentence on what it does not cover.

## What must be tested

As unit tests in `process/` under `cfg(test)`: **no `pub` seam that accepts a table or
a prefix may exist for `crates/*/tests`, because that seam ships.** Each of the eleven
`admit` refusals, by a test that would spawn if it were dropped; the site refusing a
right-shaped spec with `Admission::Allowlist`, a provider path with
`Admission::CuratedProbe`, a widened timeout, cap or environment; the exact environment;
the directory (exists before, gone after, `0700`, a planted file or link at the name or
at `probes` refuses); `arg0`; a target changed between `admit` and spawn; every row of
the answer table; replay and recording refusing; that output reaches no record; the
loader's rules. Named: `invariant_05_a_probe_never_starts_a_provider_program` (as built), `invariant_06_*`, `invariant_10_*`,
`invariant_15_a_replay_never_runs_a_probe_at_the_spawn_site` (as built), `invariant_19_a_stateful_package_is_no_row`,
`invariant_20_every_probe_row_is_dated`. Mutants: drop each condition; `Fails` as
`Holds`; a held pipe as `Holds`; the real `HOME`; the placed path for the canonical.

## Staging

`process/mod.rs` is a frozen path (§3 item 3), and this is growth in what may be
started. `16-staged/H/01-curated-probe-admission.patch` holds `mod.rs`, `system.rs`
(the `may_spawn` call, `arg0`, the working directory, the identity re-read before spawn,
the three-way reader ending, the recording refusal), `recording.rs`, `process/probe.rs`
and their tests. `state_dir.rs`'s `probes` directory and `registry::destination_sites`
land in H-2, since neither is frozen. **It is built after H-2 merges,**
in a scratch worktree on that `main`, because it needs H-1's causes, H-2's owner rule
and the data file: the owner is not handed code that has never compiled. Until it is
applied every probe answers `Unchecked (NOT_OBSERVED)`, which is safe: the source is
kept and the run says so.

## As built, staged (H/01, 2026-11-08)

`16-staged/H/01-curated-probe-admission.patch` as built, after its review rounds 1 to 4. Where this
section and an earlier one disagree, this one is what the patch does. The full table of
conditions, the post-admission checks, the site checks and the answer table are in
`16-staged/H/01-NOTES.md`, "As built"; what differs from the sections above is:

- **Who judges the answer.** `run_curated_probe` returns `CheckAnswer` itself, through an
  exhaustive table over the run's ending; there is no `ProbeObservation` and no application
  judge for the probe. The `DestinationProbe` trait above is not built: the call reaches the
  module through H-3's `DestinationSites` method, as the round-6 amendment says. (H-1 had
  built it in `domain` with a `ProbeObservation` return; **H-3 removed both**, since its one
  implementor became `DestinationSites::liveness_probe`, which takes the run lock a domain
  trait could never name. `docs/passes/16-plans/H.md`, "As built (H-3)".)
- **The owner rule is a crate-internal trait, `ProbeOwner`,** re-exported with
  `run_curated_probe` as `pub(crate)` from `process/mod.rs`. The admission asks it
  `entry_owner(package, leaf, placed)` on the path the admission built, and
  `target_is_packages(package, placed, canonical)`; the canonical path must also lie under
  `prefix()` by component, or `PROBE_TARGET_NOT_THE_PACKAGES`. H-3 implements it for both
  adapters.
- **Condition 1 also asks the runner.** `ProcessRunner::may_run_probes()`, default false, is
  true only of a `SystemRunner` that is not recording, and is asked before any write.
- **The euid is read inside `admit`** from `effective_uid()`; `None` is `PROBE_NOT_STARTED`.
- **The lock must be this root's:** `lock.path()` equals `<state root>/run.lock`, or
  `PROBE_NOT_STARTED`. `RunLock::path()` is added outside the frozen module, in the session's
  pull request, and must merge before H/01 is applied.
- **The leftover sweep removes only what a probe made:** entries under `probes` named
  `probe-<digits>-<6 lowercase hex>` that `lstat` shows as `0700` directories owned by the
  euid. Anything else there stops the probe with `PROBE_NOT_STARTED`, and nothing is removed.
  A state root pointed at a home directory therefore costs a probe, never a file.
- **Only a compiled program is admitted.** Condition 11 reads the head through the opened,
  identity-checked handle with `crate::macho` and admits only a thin native 64-bit `MH_EXECUTE` for this build's
  architecture, or a big-endian fat file every slice of which is one, one of them for this build; anything else is `PROBE_TARGET_IS_ENV_SCRIPT`. A `#!` deny list
  could not keep out a system program that prompts for a password (review round 2), and refusing
  `#!` alone was not enough, because macOS runs a file it cannot execute with `/bin/sh`: text, an
  empty file, `#!` after a BOM (review round 3), and a fat file's slice is chosen by the kernel,
  not by this reader (review round 4). The cause's text says this in both catalogues.
- **The site is `run_probe`, not `run`.** `ProcessRunner` gains `run_probe` and
  `may_run_probes`, a probe's spec is an opaque `ProbeRun` only the module can build, and no
  `Invocation` exists for a probe, so nothing can record it; `run` refuses a curated spec.
- **The stateful exclusion stays with the policy loader,** not in the adapter's reader:
  `check_liveness_probes` enforces it and `invariant_19_a_stateful_package_is_no_row` tests it
  (`crates/policy/src/data.rs`); H-3's agreement test through `curated_probe_rows()` keeps
  the two readers equal.
- **Condition 11's own failure** (open, `fstat` or reading the head) is
  `PROBE_TARGET_IS_ENV_SCRIPT`, not condition 9's cause; a handle whose identity differs from
  the `lstat` is `PROBE_TARGET_CHANGED`.
- Condition 7 is evaluated before condition 6; a changed target overrides `FAILS` as well as
  `HOLDS`; `CFFIXED_USER_HOME` is not set. Each item in this section is recorded in SE-113.

## Reviews

| Reviewer | Reviewed at | Verdict | Resolution |
|---|---|---|---|
| `security-reviewer` | uncommitted draft on `9b8d68d` | Pass with changes, L1-L14, S1-S3: `cited` runs an unseen program; `observed_version` never compared; empty `argv`; "enforced by type" false across crates; no route to the table; canonical path with `arg0`; missing conditions; the scratch parent; qualification environment | All in this revision |
| `spec-guardian` | the same draft | Approve with changes; B8, B9, D6 here | Placed-entry view overridden by L6's `arg0`, which keeps what it wanted; all else in this revision |
| `security-reviewer`, round 2 | `11d8d50` | Pass with changes, 1-8 before H-1: the probe on `ProviderRead` bypasses the gate; condition 12 cannot live in `admit`; condition 8 refuses MacPorts links; H/01 omits the re-check, and `system.rs` does not tell the endings apart | All in this revision |
| `spec-guardian`, round 2 | `11d8d50` | Approve with changes; condition 12 the same | The same |

