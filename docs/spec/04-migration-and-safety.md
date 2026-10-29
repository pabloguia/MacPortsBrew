# 04 — Migration, verification and safety

**Status:** Tier 3 unless marked. Implemented as far as plan construction,
dry-run rendering and the refusal gate: `migrate --yes` reaches a refusal gate
that refuses every plan this build produces; the read-step loop behind it
exists, is tested against recordings, and reads nothing, because the gate
refuses first (SE-020, SE-092).

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
and mutates nothing — including no `sudo` prompt, and no file this tool would
write (an output file or anything in its state directory); a command that
would have written says where instead (SE-101).

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

**What check 1 reads, per provider** (work order 16 part S; SE-107; R-20). The
provider's own record of activation, from a read of its **whole** inventory that
names no package, one rule for both: exactly one record of the package is marked
active by the provider. MacPorts — exactly one image of the port marked
`(active)` by the registry (`port -q installed`); Homebrew formulae — the keg
Homebrew records as linked (`linked_keg` in `brew info --json=v2 --installed`),
equal to one of that formula's own installed versions. A cask has no such record
and cannot pass, and an unlinked formula (keg-only ones included) does not pass.
**Check 1 is satisfied only together with `PACKAGE_NEWLY_PRESENT` on the same
step**: "active" is read from a whole inventory and does not by itself prove that
this run installed the package. The reads and their semantics are built (work
order 16 part S-1); the commands-table cells that make the planner write them,
and so make the check evaluable, are the owner's to apply (S-2).

**MUST.** A failed verification means the source package is not touched (INV-01)
and the run reports failure with the specific check that failed.

**MUST NOT.** "The install command exited 0" is not verification.

**MUST.** A verification command is a read. The commands table refuses a
`verify`, `verify_absent` or `verify_active` verb that shares a non-flag word with the same entry's own
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

**MUST.** `SERVICE` is detected from the source row's service answer, the union
`02-domain-model.md` states once (DP-032). Where that answer is `UNKNOWN` — a
half could not be read, a Portfile form the bounded reader does not interpret,
a cask, a row from a document that predates the field, or any disk-backed half
under `--replay` — the planner refuses with `SafetyUnverifiable`, naming the
package and never a path. The re-plan under the run lock goes through the same
planner, so an answer that became unknown refuses the run.

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
gone, `history` shows its run as not restorable and says so. **The state
directory holds this machine's captures only**: a snapshot built from a
recording or a snapshot file goes only to a file the user names (SE-101).

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

*Declared deviation (pass 08 slice 5, Tier 3, SE-092):* `--snapshot <file>` is
not built; it exits 1 as not implemented. `--run <id>` is built as far as the
plan: `restore --plan --run <id>` shows it, and `restore --run <id>` puts it to
the execution gate, which refuses it.

**MUST.** A restore step whose artifact is no longer available on this
machine — one the destination records as unavailable, unsupported on this
platform or disabled, or a provider that no longer supports this macOS —
becomes a `MANUAL_REVIEW` step that the plan still shows, with the reason. A
step that would build from source is not this case: a source build is a cost
the plan names in its default view (`destination_source_build`, with what the
build needs), not an unavailable artifact (DP-050, ruled 2026-10-27). `history` then reports the run as
partially restorable, and executing the plan exits with the safety-check code.
This is the case the project exists for, and it is precisely where "undo"
must not read as more reversible than it is.

*Declared deviation (pass 08 slices 5 and 6, Tier 3, SE-092, SE-107):* **this
MUST is met only where the destination's own record says the artifact cannot
be had.** Since slice 6 the planner makes a plan `MANUAL_REVIEW`
(`destination_artifact_unavailable`) when the destination's row is
`UNAVAILABLE`, `UNSUPPORTED_PLATFORM` or `DISABLED` and does not already hold
the package, for `migrate` and a restore plan alike. What stays unmet, clause by clause:

- **No adapter reports `UNAVAILABLE` or `UNSUPPORTED_PLATFORM` yet**, and a
  disabled package was already refused through its withdrawal flag, so no plan
  built from a real snapshot reaches the new rule today.
- **A restore's destination row is normally `UNKNOWN`**: the destination's
  catalogue is not consulted offline, and an unknown is not raised as
  unavailable (it would stop every restore). So the restore case this MUST is
  written about is not caught.
- **"A provider that no longer supports this macOS"** is a provider-level
  support fact, not a package's availability, and the planner does not read it.
- **`history` does not report a run as partially restorable**: it is read-only
  and starts no provider, so it cannot know whether an artifact is still
  available. It reports what it can know — the snapshot, the ends of the
  commands, whether the record is confirmed — and a restore plan shows the
  planner's risks.

`the_switch_needs_manual_review_for_an_unavailable_artifact` keeps failing with
the switch on until these are met or the owner accepts them.

*A second, found building `restore` (same pass):* a restore is planned by the
same planner and needs the same mapping evidence as the migration did. When the
run removed its source, nothing installed there is left to show the mapping in
the reverse direction, so unless a curated row covers the package the plan
stops at `MANUAL_REVIEW` (`mapping_confidence_below_threshold`, exit 4). That is
the planner's rule working, not a restore special case; letting the run's record
attest the mapping would be a planner change, and is carried.

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
risky, and the wording says which. The advice is **dated data**, not a sentence in
code: `data/policy/cross-provider-advice.toml` records, for each provider that can be
exposed to another's prefix, which build defence its own tooling has, and `doctor`
selects the sentence by that row, never by a provider's name (INV-17, INV-20; DP-041,
SE-098). For MacPorts the documented remedy for `/usr/local` interference is opt-in
trace mode (`port -t`). For Homebrew the build's `PATH` is rebuilt from its own
directories, so a directory on the user's `PATH` is not an input to a formula build;
its compiler shim also drops `-I`/`-L` flags under `/opt/local`, but only when
argument refurbishing is on, which is **not** the default on macOS (F-2, re-read
2026-10-12; the 2026-09-02 reading that the shim filters "on every source build" was
wrong and is corrected there). What was read in source to run with the user's `PATH` is `env :std` formulae and
`post_install`; nothing was read about `brew`'s own non-build subprocesses, and the
sentence claims nothing about them.

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

**Status (2026-10-17): the lock and the marker exist and are wired behind a
mutation allowance no shipped build has; the detector does not.** Pass 08
slice 4 put them in the run loop in the order below, checked on every build by
the wiring tripwires and behaviourally against fakes
(`crates/cli/tests/executor_wiring.rs`); a shipped build takes neither, because
its gate refuses every plan with a mutating step. The lock is an exclusive advisory
`flock` on `run.lock` in the state root (`persistence::run_lock`), taken only by
a run that will dispatch a provider-mutating step — after the execution gate
and the history probe, before the interrupted-run marker — and held for the
life of the run, so a killed process releases it and there is no stale lock. A
held lock is exit 7. It is never waited for, never unlinked or truncated, and
`run.lock` is checked against its path after it is taken and again when the
marker is written; deleting it during a run is unsupported. Taking it also
proves it works, by a second open that must be refused: a volume where `flock`
does nothing is refused (`SAFETY_BLOCKED`), but **a volume shared between hosts
on which it is host-local is not detected**, and nothing here checks the state
root's ancestors (`state_dir.rs`). It is **not** the detector: it excludes
nothing about `brew` or `port`, which take their own locks (R-17), and it
protects against no other user.

The **interrupted-run marker** (`persistence::run_marker`,
`docs/schemas/run-marker.v2.md`) names the run, is written durably immediately
before the first mutating command — read steps may run before it, as they
change no provider state — and cleared once the run's record is published
durably (at once when nothing mutating was handed over), by that run only.
Under the lock it holds, a marker that is gone or another run's when it is
cleared is an anomaly the run reports (`HISTORY_UNUSABLE`), never a run done. A marker found by a
mutating command refuses it, exit 4, naming the run; analysis, plan and dry runs
never read it. `doctor` shows a stranded marker (`interrupted_run`) and says
which of three things it is - recorded and not confirmed, recorded and
unreadable, started and not recorded - in the same words `history show` and
`history acknowledge` use. The one deliberate way to remove a marker other than
the run's own `clear` is `macportsbrew history acknowledge <id> --yes`: it takes
the lock, refuses another run's marker, makes the record durable and reads it
back before it removes the marker, and keeps the marker for a record it cannot
read (pass 08 slice 5, SE-092; this changes the earlier rule that no path
deletes a marker but the run's own).

**The detector is not built, and the refusal it serves is owed (SE-015,
DP-030).** No filesystem read can tell that a provider is mutating without
`fcntl(F_GETLK)` (R-17), which needs `unsafe` or a new dependency, and the
alternatives disturb a provider or grow the process allowlist. A live run says
`NOT_CHECKED`; a test fails the build that turns mutations on while it does.
**Done with the wiring (slice 4):** under the lock the machine is captured
again and re-planned, and any difference from the validated plan refuses,
naming the fact that moved (SE-092, D6); the lock is confirmed before every
mutating command. **Done in slice 5:** a stranded marker in `doctor`. **Still owed,
each a precondition of the flag-flip proposal and each behind a tripwire:** the
application-level refusal on an observed or unchecked provider; and **a
residual the re-plan does not close** — between `VERIFY_DESTINATION` and
`REMOVE_SOURCE` a concurrent `brew` or `port` could remove the destination,
unseen while the detector does not exist (slice 4 review, M6a). The check
before the removal is now built behind the switch (work order 16 part S-1;
the paragraph below), and what remains after it is accepted (DP-048).

**The residual removal window is accepted and declared (DP-048, ruled
2026-10-27).** Once slice 6's re-read of the destination runs immediately
before `REMOVE_SOURCE`, what remains is the gap between that read and the
provider's own uninstall: the lock check and the source's configuration
re-read that follow it (M1), milliseconds, plus the provider's run time. Nothing a
reader can do closes it, so it is declared here, next to DP-030's, rather than
claimed closed. **Met (work order 16 part S-1):** the destination is read again,
with its `VERIFY_DESTINATION`'s own commands and after its configuration is
re-checked, immediately before each `REMOVE_SOURCE` command and before the lock
and source-configuration re-checks; a re-read that does not certify halts the
removal (`DESTINATION_NOT_REVERIFIED`). **Not yet met:** it will be bounded by making removal opt-in
(`--remove-source`, with a confirmation stronger than an install's) and by
dispatching `REMOVE_SOURCE` only when the re-read ran, DP-030's probe
answered, and all six destination checks were evaluated for that package. The
re-read exists (part S); the opt-in, the probe and the checks do not yet — work
order 16 parts K, G and H build them — and the switch stays off until they do.

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

**The consent path is built and not wired** (pass 08 slice 4, D10; SE-016).
Whether to ask is re-derived from the command table — never the plan's
`elevation` field, and for the package's kind — and `UNKNOWN` asks. Consent is per operation: one token per
step, minted only when the person types an exact phrase naming that step, never
cached and never inherited; `--yes` cannot stand for it. The question is asked
only when both input and the question's own stream are terminals, on standard
error, with every value escaped and nothing truncated. The execution gate still
refuses every elevated step, no elevated command is built, and a tripwire fails
the build that turns mutations on while nothing asks.

## Process execution

**MUST.** One centralised executor. Argv arrays only — no shell, no string
interpolation, ever. Package names and paths are arguments, never fragments of
a command line. A provider's subcommand, and every option before it, come from a
code-level allowlist checked at the spawn site; options after it come from a
per-subcommand code-level list in the same allowlist, each matched as the whole
argument, so an option not on that subcommand's list — an abbreviation, another
value, a bare `--` — is refused at the spawn site as an unlisted subcommand is.
The adapters' own argument-vector allowlists and, for a plan's verbs, dated
policy data whose loader refuses a verification verb that repeats the entry's
own add or remove verb, stay behind it. User input never becomes a flag (F-53,
SE-050).

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
by environment (`HOMEBREW_NO_AUTO_UPDATE` keeps a read from updating
Homebrew, alongside `HOMEBREW_NO_ANALYTICS` and `HOMEBREW_NO_ENV_HINTS` — R-1,
R-2, 2026-09-02; it does not keep a read offline on its own, as the next
paragraph says), while MacPorts is quieted by argv — `-q`, which implies `-N` — plus
never invoking `selfupdate` or `sync`; no `MACPORTS_*` variable serves the
purpose (R-3, 2026-09-02). Both live in one place.

**What the Homebrew half does not do, stated rather than relied on (R-15,
2026-10-02).** `HOMEBREW_NO_AUTO_UPDATE` keeps a read off the network only
while Homebrew's API cache is present: with no cache, every Ruby read
(`config`, `info`, `list`) downloads it first and writes it under the user's
cache (~30 MB at 7.0.4). **So the side-effect-free MUST above is not met on a
machine whose Homebrew cache is cold**, and that is recorded as an open gap,
not accepted (SE-016 item 3): what a cold cache does with no network at all is
not yet established. And the environment is not the
last word: `brew` reads a `brew.env` from `/etc/homebrew/`, from the prefix's
`etc/homebrew/` and from the user's configuration directory, and exports its
`HOMEBREW_*` lines *after* the environment it was started with — a file
outranks every quieting variable, and an empty value deletes one. No
variable switches the files off. So the quieting environment is what this
tool asks for, not what `brew` is guaranteed to run with. MacPorts has a
related property: a user `~/.macports/macports.conf` overlays its system file
key by key. It cannot undo `-q`/`-N`, which are argv-only, but it can redirect
what a read reports and steer an install (R-16). **MUST** (DP-027, ruled 2026-10-03): capture
records, for each present provider, which of those files exist and which
keys they set — every key the provider's own reader would accept, by name only,
never a value; the file list and the line syntax are dated data — reads report
it (`doctor`, a plan's risks), and the execution gate refuses every mutating
step against a provider whose configuration sets a key, cannot be read, or was
not recorded by the input. Detection reads before the
provider does, so a mutating executor must re-read immediately before each step
(SE-016); `application::recheck_provider_configuration` is that check, and its
token is handed to the step it guards. Two residuals stay, owed with the consent
path (pass 08 slice 4): the window between the re-read and the provider's own
read includes, for an elevated step, the time the user spends at the password
prompt; and an elevated MacPorts step may run `port` with a different home from
the invoking user's, so the user `macports.conf` it reads may not be the one
re-read.

## History

**MUST.** Append-only, human-readable, machine-parsable. This is the one list
of what a run records; `06-cli-and-ux.md` renders it. Per run: id, timestamp,
tool version, policy version, package, direction, versions, chosen action,
whether it overrode a recommendation, retain-source, primary selection,
environment changes, the plan executed, verification results, outcome
(success / partial / failure), the snapshot path, and whether the run is
restorable (from its snapshot, partially, or not, with the reason).
**Restorability is derived when the record is read, not stored in it** (history
schema 2, pass 08 slice 5): it depends on a snapshot file that can disappear
after the record was written, so the record cannot state it. `history` derives
it, in order, from: whether any mutating step was dispatched (none: nothing to
restore); whether the record is confirmed (the interrupted-run marker does not
name it); whether the record names a snapshot and that snapshot is present,
readable and the one this run took; and whether every dispatched mutating
command ended in a known exit 0 (otherwise partially). The best answer is
"from the snapshot", never "fully": a reinstall plan is not version-exact. Never records
secrets or environment variable values.

**MUST.** The record type gives a run that dispatched no mutating step the
status `NO_CHANGE` (nothing to restore), whatever its verification reads
showed: it is never a partial migration, and it takes no lock and leaves no
interrupted-run marker. Whether such a run is written to history at all is
`plan.v1.md` obligation 12's call — today, only a mutating run is. "Dispatched"
fails closed: a halt that cannot be placed before or after a step's dispatch
counts as one. Whether a mutation was dispatched is derived from the steps; the
status from the steps and the run's ending, checked against each other, so a
supplied ending the steps do not bear out is never success (SE-092).

**Status (2026-10-18): the record type and the store exist, and the run loop
takes its snapshot first and writes a record durably on every ending of a run
that handed anything mutating over — behind a mutation allowance no shipped
build has (pass 08 slices 4 and 5), so no shipped build writes a record.** `RunRecord` (`crates/domain/src/history.rs`) holds
every field above, built by `application::history::run_record` from the plan
and the run loop's per-step records, deriving the outcome itself. A
verification read enters it as `ReadEvidence` only — result, line count,
truncation flag — so it has no field a provider read could occupy (`plan.v1.md`
obligation 3). Its start time is supplied by the caller, from the one
wall-clock read (`invariant_14_the_wall_clock_is_read_in_exactly_one_place`).
The **plan executed** is recorded as its steps — index, kind, provider, whether
each was dispatched and its result — not as the plan document or its argv: a
command's environment holds variable values and its program can hold a home
path. **Environment changes** is a list of a type with no values in this build,
so it is empty by construction.

**MUST.** *A record never certifies its own write* (pass 08 slice 5, D3). The
run's `SNAPSHOT_BEFORE` is stored `SNAPSHOT_TAKEN` (the snapshot was written
durably, from the capture taken under the run lock, and read back, before the
marker and before the first change) and its `RECORD_HISTORY` is stored
`RECORD_PENDING_READ_BACK`. The record is **confirmed** only when the write was
durable and a read-back of the published file under the lock equals what was
written; only then is the interrupted-run marker cleared. So a record is
confirmed by the **absence of a marker naming it**, and one the marker still
names is published but not confirmed — never offered for restore, never taken as
proof that a re-run is done.

**MUST.** The store (`persistence::history_store`, shape in
`docs/schemas/history.v2.md`) is one file per mutating run,
`history/run-<id>.json`, published by a link that can never replace a record.
Reading creates nothing, and a record is never trusted: `RunRecord::from_stored`
derives every derived field again and refuses one that disagrees (only a halt
with no evidence keeps the record's own word on whether it was dispatched), and anything
the writer would not have left — a stranger entry, a symbolic reference, group
or other access, another owner — is `HISTORY_UNUSABLE` (exit 8), never "no
record". History is neither read nor written as root, nor under `--replay` or
`--from-snapshot` (INV-15). The **snapshot path** is the snapshot's file name
only, inside the same state directory: a run is restorable only from the state
directory it was recorded in, and moving `MACPORTSBREW_STATE_DIR` leaves its
history behind. Before a mutating step is dispatched, the store's ability to
publish is proved by a probe; a run never finds out after a change that it
cannot record it. (The probe and the write call are both wired, in that order,
and the wiring tripwires check it on every build — `plan.v1.md` obligation 12.)
A record not known to be durable keeps the marker, never exits 0, is never
deleted and is never offered for restore.

**MUST.** A `--retain-source` re-run is recognised as done only when the latest
record for the same package, source, destination and destination package is the
only one at its stamp, is **confirmed** (the marker, read at planning and again
under the lock, does not name it and was readable), is a retain-source `SUCCESS`, no record is stamped after
the current run, and a fresh capture shows the destination holding the package
active and the executable resolving to that record's primary — a record alone is
never proof of the machine's present state. Recognition replaces only a plan
with no `MANUAL_REVIEW` step. **A shim resolution is not recognised** — no
capture can yet say which provider a shim points at — so INV-16 for a re-run is
not closed until shims record their target (SE-092).

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
  either position; a `--long` option after the subcommand must be, exactly and
  as the whole argument, one on that subcommand's own list — the options this
  tool's reads pass and no others, so `brew info --github` (which opens a
  browser: R-15, E4 and finding 4, Homebrew 7.0.4, read 2026-10-02),
  `--json=v1` beside the listed `--json=v2`, an abbreviation such as
  `port info --nam` that the provider would resolve (`port` resolves a long
  option by unambiguous prefix in `cmd_option_matches`, as it resolves an
  action in `find_action`: `/opt/local/bin/port`, MacPorts 2.12.6, read
  2026-09-25, the option path 2026-10-03; for `brew`, Ruby's option parser may
  resolve an unambiguous prefix too), and a bare `--` are all refused. The list is a tool permission list, not a provider fact: it says
  what this tool asks for and carries no date. **Positional arguments are pinned
  by count and by a per-provider class** (pass 08 slice 3): each subcommand
  takes none, at most one name, exactly one of a fixed set (`port echo`), one or
  more names (`port info`, which must also carry `--index` - without it `info`
  evaluates each Portfile), or one or more absolute paths with no `..`
  (`port provides`). A `port` name starts with a letter or digit, carries no
  `@` and is none of the pseudo-ports or the `and`/`or`/`not` operators, whatever
  its case — `installed@1` and `+universal` are refused, because `port` reads
  the first as a set and falls back to the working directory's Portfile on the
  second; a `brew` name starts with a letter or digit and does not end in `.rb`,
  `.json` or `.tar.gz`, which Homebrew's loaders would read as a file. Which
  package a plan's read names is still the execution gate's
  `COMMAND_IS_NOT_THE_PLANNERS_READ`, because only the plan knows it. No name
  class admits `/` or `:` — for `port` either one makes a list element a
  Portfile it opens and evaluates, and Homebrew's own refusal of a path or URL
  can be turned off by a `brew.env` — and an empty argument vector — `port`'s
  interactive mode — is refused. The
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

  **A plan names a program and an environment, and neither runs (DP-023).**
  `migrate` re-plans in process and never deserialises a plan (SE-092), so
  this is not a defence against an edited plan file; it is defence in depth
  against a planner defect, or a doctored snapshot or recording, putting a
  program, an environment or a verb into a plan that the executor would then
  run. A plan's provider command reaches a spawn through
  `registry::plan_read_spec`, which takes **argv alone** from it. The program is
  re-derived from a `DetectedPrefix` and the adapter's executable literal, and
  the environment from the adapter's own construction for that prefix and the
  home the adapters were built with (none under replay). `DetectedPrefix` has
  private fields and one constructor, `registry::detected_prefix`, which
  resolves the prefix the way the adapters' own is resolved — probing the
  documented candidates on a live run, the override only under replay, and no
  fallback when nothing was detected — so neither the prefix nor the home can
  be a value taken from a plan or a snapshot. A `DetectedPrefix` records
  whether it was resolved through a replay runner, every spec built from such
  a prefix is marked replay-only, and the live runner refuses a marked spec
  before anything is spawned; and a plan read runs through
  `registry::PlanReadSites`, which is bound to the runner detection used, so a
  recording's prefix is answered by the recording. `RegistryContext.home` is
  still a public field the caller fills: `migrate`'s one call path hands the
  run loop the registry context the capture's adapters were built from
  (`Context::execution_registry`), and the loop builds the sites from it
  with `registry::plan_read_sites`, so the adapters and the sites share one
  runner, one set of prefix sources and one home (SE-050, SE-092). The prefix
  is re-detected, not reused from capture; one that changed in between is
  refused as `ProgramDisagrees`. The plan's `environment`
  is not a parameter anywhere on that path, so a plan carrying
  `DYLD_INSERT_LIBRARIES`, a build flag or a different `PATH` changes nothing
  that is spawned (INV-10). A plan whose `program` disagrees with the derived
  one is refused rather than corrected, because running a binary other than
  the one the human reviewed is the INV-02 failure re-derivation would
  otherwise hide. The argv is checked against the read allowlist above **when
  the spec is constructed**, with the shipped rule and not the test build's
  widened one: a replay runner asks no allowlist, so a check left to the spawn
  would let a recorded test run a plan read the binary refuses. A `TOOL`
  command and an unsafe prefix are refused the same way. The run loop
  (`crates/cli/src/executor/`, decisions in `application::executor`) takes
  the registry context, not read sites, and builds the sites only after the
  execution gate passed the plan, so a plan read is built only for a plan
  whose argv the gate pinned to the planner's own read; it builds no spec
  itself, and a test holds the command-line crate to naming neither the
  spec type nor `plan_read_spec`, and `CommandSpec::provider` to the two
  provider adapters and one spawn-site test; and it renders a read site's
  refusal and the gate's refusals, which quote plan text, with the home
  redacted and control, format and line-separator characters escaped.
  `migrate --yes` calls the loop (SE-092), and the gate refuses every plan
  this build produces before any read site is built, so no plan read is
  spawned yet. SE-050 records the design and its
  mutation evidence.
- **Executable shadowing / PATH spoofing** — resolve provider executables from
  their detected prefix, not from `PATH`, when correctness matters. Report
  writable directories that precede system paths on `PATH`.
- **Symlink attacks and unsafe temp files** — create temporary files with
  restrictive permissions in a private directory; never follow a symlink when
  writing; write atomically.
- **Malicious or malformed provider metadata** — treat all provider output as
  untrusted input. Bound sizes, validate shapes, never `eval`, never let
  metadata become a path or a flag. Text that reaches a reader is held to one
  classification (`domain::text`): controls, Unicode format characters, and
  the line and paragraph separators. A snapshot refuses controls in every
  display field `check_names` reads and in reported variable names (the
  fields it does not read are listed in SE-092 and are escaped when shown), refuses a provider id that is
  not an identifier or does not match its detection, and keeps format
  characters and reported values as recorded — the same way on capture and on
  load. Every human rendering escapes all three classes, at the catalogue's
  parameter substitution and wherever snapshot text is printed directly, so a
  kept character cannot reorder text or break a line; default-ignorable
  characters outside Cf are not covered. `--json` is not escaped beyond JSON's own rules, except `migrate`'s
  `gateRefusals`, which are cleaned like the human view (`migrate.v1.md`): a
  consumer that renders it must escape them itself (SE-092, end-of-pass amendment (a)).
- **Privilege boundaries** — elevation is per-operation, enumerated, visible,
  never cached, never inherited by unrelated steps.
- **State and configuration integrity** — MacPortsBrew's own state directory is
  user-owned with restrictive permissions; corrupt state fails closed rather
  than producing a plan.
- **Supply chain of MacPortsBrew itself** — pinned dependencies, checksummed and
  ideally attested release artifacts.
