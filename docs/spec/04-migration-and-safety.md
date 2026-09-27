# 04 — Migration, verification and safety

**Status:** Tier 3 unless marked. Not implemented in the foundation pass beyond
plan construction and dry-run rendering.

## State machine

```
DISCOVER -> ASSESS -> MAP -> DEPENDENCY CHECK -> REVERSE DEPENDENCY CHECK
-> PLAN -> HUMAN REVIEW
-> INSTALL DESTINATION -> VERIFY DESTINATION
-> ACTIVATE (primary) -> VERIFY RESOLUTION
-> [optional] REMOVE SOURCE -> VERIFY REMOVAL
-> [optional] CLEANUP REVIEW
-> RECORD HISTORY
```

**MUST.** Each transition is explicit, logged, and resumable-or-abortable.
A run that dies mid-flight leaves a recoverable record; the next run detects
it and refuses to start another mutating run until the user has looked
(safety-check exit code, naming the run). The recovery path is the ordinary
one: `history show <id>` to see what completed,
then `restore --run <id> --plan` to get back, or a fresh plan to go forward.
There is no separate resume command to learn.

## The plan is the contract

**MUST.** A `MigrationPlan` is serialisable, diffable, reviewable and
re-executable. It contains: package, source, destination, retain-source
decision, primary-provider decision, the ordered steps, every command to be run
with its full argv, every operation requiring elevation, verification steps,
environment changes, cleanup policy, detected risks, and the rollback story.

**MUST.** The executor executes plan steps and nothing else (INV-02). If a step
fails, it does not improvise.

**MUST.** `--dry-run` produces the plan and every command that *would* run,
and mutates nothing — including no `sudo` prompt.

## Verification contract

The original brief said "verify" without defining it. It is defined here.

**MUST.** Destination verification succeeds only when all of these hold:

1. The destination provider reports the package as installed and active.
2. The provider reports the package's own integrity check as passing, where the
   provider offers one.
3. The expected executables exist at paths *owned by the destination provider's
   prefix*, confirmed by asking the provider which package owns the file — not
   by string-matching the path.
4. A liveness probe runs each primary executable with a safe, non-mutating flag
   (default `--version`; overridable per package in curated data; skippable with
   a recorded reason for packages with no such flag) and it exits successfully.
5. No new cross-provider linkage was introduced (see contamination check below).

**MUST.** A failed verification means the source package is not touched (INV-01)
and the run reports failure with the specific check that failed.

**MUST NOT.** "The install command exited 0" is not verification.

**MUST.** A verification command is a read. The commands table refuses a
`verify` or `verify_absent` verb that shares a non-flag word with the same entry's own
add or remove verb, and the spawn site refuses any provider subcommand not on
its read list (F-53, SE-050). The second half holds while that list contains only reads;
SE-050 records what changes when an executor adds mutating subcommands to it.

### Verifying a removal

**MUST.** Removal verification establishes **absence**, and absence cannot be
established by asking a provider about the package that is absent: observed
2026-09-19, both providers answer a removed package, a name that was never a
package, and a provider that could not run with the same empty output. It is
therefore established from the provider's **whole inventory**, and only when
that inventory is known to be complete.

**MUST NOT.** An empty, truncated or nonzero-exit answer is never absence.

The conditions, the observation behind them and the obligation never to record
that inventory are stated **once**, in
[`../schemas/plan.v1.md`](../schemas/plan.v1.md)'s `success` field and its
executor-debt item 3. This section points at that copy rather than repeating it:
this contract was written in four places on one day — a schema document, a Rust
doc comment and two data-file comments — and this chapter, which INV-01 names as
*the* verification contract, was silent. Three spellings of one thing is two
spellings that will be wrong.

## Reverse dependencies

**MUST.** Before any removal, resolve reverse dependencies within the source
provider and map them up to the top-level packages the user recognises. Present
those names, not the transitive middle. Block by default. Any override is an
explicit, separate, advanced-mode confirmation and is recorded.

## Stateful packages and services (INV-19)

**MUST.** Before planning a migration for a `STATEFUL` or `SERVICE` package:

- detect data directories and provider-managed launch agents/daemons;
- refuse to plan an automatic migration;
- emit a `MANUAL_REVIEW` outcome that names what would be left behind, what
  would need to be stopped, and what would need to be migrated by hand;
- where a documented, tested playbook exists for a specific package, link it.

**MUST NOT.** Never stop, start, unload or relocate a background service as an
implicit side effect of a package migration.

## Rollback and snapshots

**MUST.** Do not promise transaction semantics the underlying package managers
do not provide. State plainly, in docs and in the UI, what can and cannot be
undone.

**MUST.** The realistic safety net is snapshot-and-restore.
`macportsbrew snapshot` captures a `Snapshot` as defined in
`02-domain-model.md` — one artifact, which includes both providers' requested
packages and the managed environment state — and `macportsbrew restore --plan`
produces a *reviewable reinstall plan* from one. Restore never runs unattended
and never promises version-exact reconstruction.

**MUST.** A snapshot is taken automatically, without asking, before the first
mutating step of any run, and its path is printed. Automatic snapshots live in
the tool's state directory and are never pruned automatically; when one is
gone, `history` shows its run as not restorable and says so.

### Undoing a migration

This is the user story a person reaches for after a migration went wrong or
was simply regretted, and it must be discoverable without reading this file.

**MUST.** Every mutating run records, in history, the snapshot it took first,
the plan it executed and the outcome. `macportsbrew history` lists runs with
those three things; `macportsbrew restore --run <id> --plan` builds a
reviewable plan that returns the packages that run touched to their state
before it. Every mutating run ends by printing that exact command for its own
id, whatever its outcome — a partial success is precisely when it is needed.

**MUST.** A restore plan is an ordinary `MigrationPlan`. It goes through the
same human review (INV-02), the same executor and the same ordering as any
migration: reinstall what was removed and verify it per the contract above
before removing what was added (INV-01), within a single provider's reverse
dependencies (INV-04), never touching a `STATEFUL` or `SERVICE` package's data
or services (INV-19). Undo is a migration in the other direction, not a
special case (INV-17). Executing it is `restore --run <id>` without `--plan`,
which plans, shows the plan, confirms and executes — the same path `migrate`
takes, and available in the same pass as the executor. A run made with
`--retain-source` is the cheap case: nothing is reinstalled — the plan
verifies the retained source package per the verification contract, restores
it as the primary if the run had changed that, and only then removes the
destination package. The order is the same as for any migration (INV-01);
what is cheap is the first step, not its absence.

**MUST.** Scope. `--run <id>` touches only the packages that run changed, and
reverts, through the procedures in `05-environment-and-primary-provider.md`,
the primary selection and the managed shell block *if that run changed them*.
`--snapshot <file>` is additive: it reinstalls what the snapshot has and the
machine lacks, and lists — without acting on — what the machine has and the
snapshot lacks. Removing those is a separate, advanced-mode confirmation
(`06-cli-and-ux.md`), because a whole-machine reconcile is a far more
dangerous plan than a run-scoped one.

**MUST.** A restore step whose artifact is no longer available on this
machine — the Homebrew formula an Intel Mac can no longer bottle, or a
provider that no longer supports this macOS — becomes a `MANUAL_REVIEW` step
that the plan still shows, with the reason. `history` then reports the run as
partially restorable, and executing the plan exits with the safety-check code.
This is the case the project exists for, and it is precisely where "undo"
must not read as more reversible than it is.

**MUST.** The command is named `restore`, not `undo`, and the documentation
says why: what it produces is a reinstall plan, and what it does not promise
must be as visible as what it does. It does not promise version-exact
reconstruction — the provider installs what it offers now, and the plan shows
where that differs from the snapshot; it does not restore data owned by
`STATEFUL` packages; it does not reverse changes made outside MacPortsBrew
between the run and the restore; and a run whose snapshot is missing or
unreadable is listed as not restorable, with the reason, rather than
restored from a guess.

## Failure semantics

| Failure point | Result |
|---|---|
| Destination install fails | Source untouched. Report and stop. |
| Destination verification fails | Source untouched. Report which check failed. |
| Activation fails | Restore previous resolution where safe. Source untouched. |
| Source removal fails after a good destination | Destination usable. Report **partial success**. Leave recovery instructions. |
| Cleanup fails | Never affects migration status. Report separately. |

**MUST.** Partial success is its own exit code and its own history status.

## Cleanup

**MUST.** Cleanup is a separate, opt-in operation, never a side effect of
migration. Candidates come from the owning provider's own mechanism — never from
MacPortsBrew guessing at a dependency graph. Candidates are shown before removal.

## Cross-provider contamination

**MUST.** `doctor` performs a cheap structural check by default: prefixes, PATH
order, environment variables that leak one prefix into the other's builds
(INV-10), a dual-Homebrew-prefix check on Apple Silicon (Homebrew's default
prefix is `/opt/homebrew` on Apple Silicon and `/usr/local` on Intel —
<https://docs.brew.sh/Installation>, read 2026-09-02 — so a migrated machine
can carry both), and architecture mismatches.

**MUST.** `doctor` implements the MacPorts-on-`PATH` check itself and never
defers to the provider. `brew doctor` has no MacPorts, Fink or `/opt/local`
check at any version examined — none in `diagnostic.rb` at HEAD or at tags
1.9.0, 2.7.0 or 3.6.0 (F-2, 2026-09-02). The two directions are also not equally
risky, and the wording says which: MacPorts' documented remedy for
`/usr/local` interference is opt-in trace mode (`port -t`), whereas Homebrew's
compiler shim already drops `-I`/`-L` flags under `/opt/local` on every source
build by default, so the residual risk toward Homebrew is in `PATH`-resolved
*tools*, not in headers and libraries (F-2, 2026-09-02).

**SHOULD.** `doctor --deep` inspects the dynamic-library dependencies of
installed binaries for links crossing prefixes. Expensive; opt-in; bounded;
interruptible.

**MUST NOT.** Never rewrite, re-link or patch a binary. Report, explain the
affected package, and recommend the provider-native repair (reinstall from the
owning provider).

## Concurrency and locking

**MUST.** A single-instance lock prevents two mutating MacPortsBrew runs.
Detect a concurrently running `brew` or `port` before mutating and refuse
rather than interleave. Read-only analysis may proceed, flagged as possibly
inconsistent.

## Privilege

**MUST.** The design assumes MacPorts operations generally require elevation
and Homebrew's normally do not; R-5 confirms that in outline and sharpens it in
three places, which is why the plan asks the adapter **per step** rather than
per provider.

Elevation varies by *package*: a Homebrew cask whose `pkg` artifact or
`installer script:` carries `sudo: true` elevates on an otherwise unprivileged
provider, and `brew services` at system level elevates even to *list*, so an
unprivileged services inventory is necessarily incomplete and must say so.

Elevation also varies by *prefix*, and on MacPorts that is the whole mechanism.
Observed 2026-09-08 in the shipped 2.12.6 Tcl (R-5 E29): all four of its
elevation guards are conditional, and `portinstall.tcl:100` reads
`if {![file writable $prefix_frozen] || ([getuid] == 0 && [geteuid] != 0)}`
before `elevateToRoot`. MacPorts elevates because `/opt/local` is `root:wheel`,
not because it is MacPorts — and `data/policy/provider-prefixes.toml` already
contemplates a `--prefix` install, where it would not. So "generally requires
elevation" is a dated fact about the default installation, scoped to a provider
*prefix*, and never a property to compile in. The removal verb additionally
skips elevation for a dry run, which is our own "analysis and dry-run never
elevate" rule, enforced by the provider rather than by us. Therefore:
the plan lists exactly which steps elevate and why; elevation
is requested per operation at the moment it is needed; the credential is never
cached by MacPortsBrew; analysis and dry-run never elevate; the tool is never
installed setuid and never asks to be run under `sudo` wholesale.

**An `UNKNOWN` elevation is described as unknown and treated as elevating.**
Those are different questions and the answers differ. A plan that tells you a
read needs administrator rights teaches you to approve prompts without reading
them, which is the habit this invariant exists to prevent — so the *description*
says what is known. The *consent path* has no such latitude: an elevation nobody
has established is not an elevation that will not happen, so an executor asks
before any step whose elevation is not `NONE`. `elevatedSteps` is that
conservative set and stays it; `MigrationPlan::steps_requiring_elevation` is the
narrower one, and exists so a *report* can make the narrower claim truthfully.
Recorded because the split reads as a relaxation to anyone who meets only half
of it (SE-059, finding F-10).

## Process execution

**MUST.** One centralised executor. Argv arrays only — no shell, no string
interpolation, ever. Package names and paths are arguments, never fragments of
a command line. A provider's subcommand, and every option before it, come from a
code-level allowlist checked at the spawn site; options after it come from the
adapters' own argument-vector allowlists or, for a plan's verbs, from dated
policy data whose loader refuses a verification verb that repeats the entry's own
add or remove verb; any other subcommand is refused at the spawn site. User
input never becomes a flag (F-53, SE-050).

**MUST.** A child never inherits this process's working directory; it always
starts in `/`, which is root-owned and holds no Portfile, because a provider can
resolve a read against the directory it was started in (`port`'s `current`
pseudo-port; SE-050). A command spec carries no directory, so nothing can name
another one.

**MUST.** Every invocation records argv, sanitised environment, exit status,
stdout, stderr and duration, and supports timeout and cancellation.

**MUST.** Provider invocations set the environment *and flags* that keep them
quiet and side-effect-free during analysis — no auto-update, no interactive
prompts, no pager, stable locale for parsing. The mechanism is not the same on
both sides and the specification does not pretend otherwise: Homebrew is quieted
by environment (`HOMEBREW_NO_AUTO_UPDATE` is the one that does the offline work,
alongside `HOMEBREW_NO_ANALYTICS` and `HOMEBREW_NO_ENV_HINTS` — R-1, R-2,
2026-09-02), while MacPorts is quieted by argv — `-q`, which implies `-N` — plus
never invoking `selfupdate` or `sync`; no `MACPORTS_*` variable serves the
purpose (R-3, 2026-09-02). Both live in one place.

## History

**MUST.** Append-only, human-readable, machine-parsable. This is the one list
of what a run records; `06-cli-and-ux.md` renders it. Per run: id, timestamp,
tool version, policy version, package, direction, versions, chosen action,
whether it overrode a recommendation, retain-source, primary selection,
environment changes, the plan executed, verification results, outcome
(success / partial / failure), the snapshot path, and whether the run is
restorable (fully, partially, or not, with the reason). Never records secrets
or environment variable values.

## Threat model

Package management is privileged infrastructure work. Address these concretely;
do not gold-plate beyond them.

- **Command injection** — no shell *command line* is ever constructed. That is
  the claim, and it is narrower than the one this file used to make, because two
  reviews measured the wider ones and found them false (F-08).

  A shell does run. `/usr/local/bin/brew` begins `#!/bin/bash -pu` (read on this
  machine, 2026-09-04) and `port` is a Tcl program, so every provider read this
  tool performs starts an interpreter by way of the kernel's shebang handling.
  What does not happen is a *string* being assembled and handed to one:
  arguments are argv elements, they arrive as `"$@"`, and nothing re-splits
  them. Two properties of that bash invocation are load-bearing and worth
  naming rather than relying on quietly: `-p` is privileged mode, in which
  `$ENV` and `$BASH_ENV` are not processed, shell functions are not inherited
  from the environment, and `SHELLOPTS` is ignored (`bash(1)`, read 2026-09-04);
  and the environment is allowlist-built by `sanitised_environment`, so those
  variables are not present to be read in the first place.

  **The control on *what* runs is a run-time allowlist at the one spawn site.**
  `SystemRunner::run` refuses any program that is not, by the name at the end of
  its path, one of the six this tool has business running — `brew`, `port`,
  `sw_vers`, `uname`, `sysctl`, `xcode-select` — absolute, free of `..`, and for
  the two providers directly inside a `bin` directory. It is there, on the value
  about to reach the kernel, because everything upstream of it was defeated in
  review: public fields let a spec be rewritten after construction, a
  module-level `OnceLock` manufactures the `&'static str` the constructors ask
  for with no `Box::leak` and nothing to grep, and a *denylist* of interpreters
  had never heard of `tclsh`, which is what `port` is written in.

  A program name is half the question, because every mutation this tool must
  never perform is `brew` or `port` with different arguments. So for the two
  providers the same check has a second dimension, `PERMITTED_SUBCOMMANDS`:
  after a listed leading option (`-q` for `port`, none for `brew`), the next
  argument must be, exactly, one of the read subcommands the adapters run; no
  argument may be `port`'s action separator `;`; nothing after the subcommand
  may be a single-dash option, since `port -F <file>` runs a command file from
  either position; nothing after a subcommand that takes packages may contain
  `/` or `:` — for `port` either one makes a list element a Portfile it opens
  and evaluates, and Homebrew's own refusal of a path or URL can be turned off
  by a `brew.env` — so only `port provides`, which takes raw strings, accepts a
  path; and an empty argument vector — `port`'s interactive mode — is refused. The
  `port` behaviours were read in its source on this machine, MacPorts 2.12.6,
  2026-09-25 and 2026-09-26, and are cited where the list states them. The
  adapters' own argument-vector allowlists stay, tied to these lists in both
  directions by a test; this is what holds for a command that did not come
  from an adapter, such as a plan's verbs read out of policy data (F-53,
  SE-050). It is a list of *reads*: the pass that adds an executor extends it
  deliberately, and at that point it can no longer tell a read step from a
  write step on its own — SE-050 records that as open.

  Three further controls stand behind it, each holding something the allowlist
  does not. `check-layering.sh` keeps `std::process::Command` inside the
  executor — **where**. `CommandSpec`'s fields and its `new` are private to that
  module, so a spec cannot be built or rewritten from outside it, which is also
  what keeps INV-10's sanitised environment from being amended after the fact.
  And `check-no-shell.sh` asks the allowlist's question textually, so a program
  literal that is not one of the six fails in review rather than at run time.

  Note what is *not* claimed. The prefix a provider's executable is resolved
  from is a run-time value, not a literal: the leaf is fixed, the directory is
  detected. What holds there is separate and specific — a prefix is validated
  where a snapshot enters, a `prefix_override` is honoured only under replay,
  and `--from-snapshot` returns before any adapter is built, so a
  recording-derived or snapshot-derived prefix never reaches a live spawn.
- **Executable shadowing / PATH spoofing** — resolve provider executables from
  their detected prefix, not from `PATH`, when correctness matters. Report
  writable directories that precede system paths on `PATH`.
- **Symlink attacks and unsafe temp files** — create temporary files with
  restrictive permissions in a private directory; never follow a symlink when
  writing; write atomically.
- **Malicious or malformed provider metadata** — treat all provider output as
  untrusted input. Bound sizes, validate shapes, never `eval`, never let
  metadata become a path or a flag.
- **Privilege boundaries** — elevation is per-operation, enumerated, visible,
  never cached, never inherited by unrelated steps.
- **State and configuration integrity** — MacPortsBrew's own state directory is
  user-owned with restrictive permissions; corrupt state fails closed rather
  than producing a plan.
- **Supply chain of MacPortsBrew itself** — pinned dependencies, checksummed and
  ideally attested release artifacts.
