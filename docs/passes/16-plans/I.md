<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part I — layer 4, consented (DP-047 (a))

Plan written 2026-11-21, at `9354db3` (the merge of part K's last slice, PR #86),
for `prompts/16-complete-mutating-product.md` §5 I, before any code. Part I is
**the first write outside the state directory**, so `security-reviewer` on this
plan is not optional (§5 I item 5); `spec-guardian` reviews it too. Their review
is the approval (§4). Every citation below was read at `9354db3`.

**The switch stays off** (§3 item 1). Part I builds the offer, the steps, the
consent, the writers, `env`, restore support and `doctor`'s validation. In this
build the execution gate still refuses every plan before its first step
(`MutationNotInThisBuild`, `crates/domain/src/plan.rs:614`). The writers are
exercised only against a scratch home, by tests (§5 I item 5).

## The ruling, and what binds

DP-047, ruled 2026-10-27 and confirmed by the owner (`docs/decisions-pending.md:476`):
(b) now, which part F built, then (a) before v0.2.0, **only with consent**. A shim
is never written implicitly. When the destination does not win `PATH`, the plan
offers layer 4 for that executable: one shim in the MacPortsBrew-owned directory
and one managed `PATH` block. The offer comes with a backup, a preview,
`env --revert`, restore support and idempotence per INV-16. Without consent, the
gate refuses with `PRIMARY_NEEDS_SHIM` and names layers 1 and 2.

Binding, in order of authority:

- **INV-02.** Both writes are plan steps with exact paths. Nothing not in the plan
  is written.
- **INV-07.** The shim directory must not make one provider's builds resolve the
  other's tools. See D11. This is the hardest constraint of the part.
- **INV-11.** A shim transfers no ownership.
- **INV-12.** A shim changes the resolution of the consented names and of no
  other name. Global priority is not offered.
- **INV-16.** Running twice changes nothing: no duplicated block, `PATH` entry
  or shim.
- **INV-03, INV-06, INV-15 and INV-17** hold as everywhere.
- **Spec 05.** These are Tier 3 unless marked:
  - layer 4 is the shim directory, never inside a provider prefix (`:61-73`);
  - a shim targets the provider's stable path (`:75-78`);
  - layer 4 is deterministic, inspectable, reversible, idempotent,
    self-repairing, owned and validated on every run (`:86-91`);
  - the shell-configuration MUSTs: detect, one delimited block, the write
    procedure, `env --revert`, never append blindly, never duplicate, never
    reorder (`:188-209`);
  - managed versus reported variables (`:213-217`).
- **§3 item 10.** A shim's executable name is a single leaf: no `/`, no `..`, no
  leading `-`. Its target must exist under the destination's detected prefix and
  be owned by that destination.

## Where it lives today

The code map was taken at `9354db3`; every claim was re-read.

- **The input.** `PrimaryDecision::Refused` carries `PrimaryReason::PrimaryNeedsShim`
  (`crates/domain/src/primary.rs:44`, `:109`). The decision is computed by
  `application::primary::decide` (`crates/application/src/primary.rs:59`). It is
  refused at `:195` (not linked), `:207` (no `PATH` entry) and `:222` (`d > w`).
  `PrimaryDecision::needing_a_shim` (`domain/src/primary.rs:173`) is documented as
  "the input a later part's shim offer starts from". F.md `:789-793` hands it to
  this part.
- **The refusal.** `ExecutionRefusal::PrimaryNeedsShim` is raised in
  `execution_gate.rs:1237` (`primary_refusals`) and declared at `:318`. Its doc
  comment reserves `ErrorId::PrimaryNeedsShim` (`domain/src/error.rs:127`, exit 4)
  for "the part that offers a consented shim". That identifier is defined and
  never raised today (F.md:1635).
- **The steps.** `StepKind` (`domain/src/plan.rs:29-50`) has no write kind. A
  `MigrationStep` (`:465`) carries `commands: Vec<PlanCommand>` and
  `primary: Option<PrimaryDecision>`. `CommandTarget::Tool` (`:321-338`) is
  documented for the primary's shim directory, but since F-3 no step carries a
  `Tool` command.
  - The planner builds `SET_PRIMARY` with no command (`planner.rs:1047-1059`) and
    one `VERIFY_RESOLUTION` per executable list (`:1060-1080`).
  - The executor never dispatches `SET_PRIMARY` (`executor.rs:1073`, `guard`
    `:1270-1285`).
- **Consent.** Per-step consent exists only for elevated provider commands:
  `ConsentGiven{step,program,argv}` (`application/src/consent.rs:119`, not
  `Clone`). The plan tie is `ShownPlan`, `admits` and the digests
  (`application/src/plan_tie.rs:41`, `:66`, `:203-374`). There is no consent for
  a file write.
- **Durable writes.** These are in `crates/adapters/src/persistence/atomic.rs`:
  - `write_atomic` (`:49`) refuses a symlink target;
  - `write_new_durable` (`:158`) publishes by `hard_link`;
  - `write_new_with_mode` (`:201`) is the DP-029 creation-mode seam.

  Every writer targets the state directory (`state_dir.rs:35`). None writes
  elsewhere in the home.
- **Not present.** There is no `env` subcommand (`cli/src/app/mod.rs:102`), no
  managed block and no shell detection. Restore (`application/src/restore.rs`)
  has no primary or shim handling (F.md:517).
- **History.** `NotRecognised::PrimaryIsShim` (`application/src/history.rs:427`)
  marks a re-run whose winner is a shim as never recognised. 04 `:753-755`
  declares "INV-16 for a re-run is not closed until shims record their target"
  (SE-092 S6 and S8 D3, open for shim plans until this part).
- **The environment.** The adapter reads environment variables in one place,
  `adapters/src/system/machine.rs:115-126`, which is where `PATH` and `HOME` are
  read.
- **The tooling.**
  - `stop-gate.sh:211` fails a turn on the word `symlink` in any non-comment code
    under `src/`, process executor included: "nothing in this pass … creates
    symlinks". A shim is a symlink (spec 05 `:63`). See D14.
  - The Edit guards deny any path ending in a shell file name, scratch paths
    included (`guard-protected-paths.sh:118-119`). Test fixtures for shell files
    are therefore created at test time under a temporary home, never committed
    under such a name.

## Decisions the work order leaves to the session

§4.3 lets the session make these choices. Each is recorded in SE-115 (new, this
part) when its slice lands.

### D1 — The offer is opt-in, per run: `--allow-shim`

`plan` and `migrate` gain `--allow-shim`. Without it, nothing changes from today,
except that the refusal is now raised as `ErrorId::PrimaryNeedsShim` (exit 4). Its
text names layer 1 ("the destination's own `bin` already winning, by your own
`PATH` order") and layer 2 (the versioned name, where the destination ships one).
It says that reordering `PATH` is global priority (INV-12), which is not offered.

With `--allow-shim`, a `PRIMARY_NEEDS_SHIM` executable becomes a planned shim,
provided D4–D6 admit it. Every other refusal reason is unchanged, so a shim is
never planned over `NON_PROVIDER_ENTRY_WINS`, `WINNER_NOT_AUTHORITATIVE` or
`OWNED_BY_ANOTHER_PACKAGE`.

As a consequence, a shim never displaces a system tool, because step 4 has
already refused a non-provider winner. Spec 05's shadowing MUST (`:30-31`) is
therefore met by construction. A test pins it (`invariant_12_*` below).

The flag is a request, not consent. Consent is given per write step at
confirmation (D3). `PrimaryDecision` gains `ShimPlanned{executables, path_used}`.
Its executables carry the shim name and target, and a test holds it disjoint from
`Refused`.

### D2 — Two step kinds: `WRITE_SHIM` and `WRITE_PATH_BLOCK`

Two step kinds are added, and the state machine becomes:

```
… SET_PRIMARY -> [WRITE_SHIM ...] -> [WRITE_PATH_BLOCK] -> VERIFY_RESOLUTION -> [REMOVE_SOURCE …]
```

- **`WRITE_SHIM`**, one per shimmed executable, carries a `FileWrite::Shim` with:
  - the shim's exact path (home-redacted in the plan, as `PATH` is, F-3);
  - the link's target;
  - the slot's state as planned: `Absent`, or `RecordedShim{target}`, a symlink
    this tool's history recorded.
- **`WRITE_PATH_BLOCK`** appears at most once per plan. It carries a
  `FileWrite::Block` with:
  - the shell file's exact path;
  - the file's state as planned: `Absent`, or `Present{sha256, mode}`;
  - the block before, if any, and the block after;
  - the unified diff the preview shows.

  The step is planned only when the block must change. An identical block means
  no step, and the plan says why (INV-16).
- **Not `PlanCommand`.** A write has no program and no argv, so dressing it as
  one would put a fake program through checks that read `program`.
  `CommandTarget::Tool` is removed: it was reserved for this, and this design
  does not use it. A test pins that no variant documents an unbuilt use.
- **`is_mutating`** is true for both (`plan.rs:59`, exhaustive).
- **`check_ordering`** (`plan.rs:801`) gains three rules:
  - both kinds come after the `VERIFY_DESTINATION` that guards the destination;
  - both come before `VERIFY_RESOLUTION`;
  - every `WRITE_SHIM` comes before the `WRITE_PATH_BLOCK`, so the block never
    points `PATH` at a directory missing a planned entry.

  INV-01's rule is unchanged: no `REMOVE_SOURCE` before its `VERIFY_RESOLUTION`.
- **Schema.** The new kinds and the `write` field mean `plan.v3` and `migrate.v3`.
  A v2 reader that matches kinds exhaustively would misread v3, so this is a
  version, not an additive change. Recorded in SE-115.

### D3 — Consent per write step, recorded per step

Each write step needs its own consent, the analogue of elevation's
`ConsentGiven` (J will build both on one shape).

- **Interactive.** After Q0 (and Q1 or Q2 where a removal asks), one question per
  write step shows its exact path, its target or diff, and how it is undone. A
  decline exits 6 before anything runs (`invariant_02_a_declined_*`).
- **Scripts.** `--yes --plan-digest` is consent for every write step. The digest
  is SHA-256 over the plan's JSON (`plan_tie.rs:219`), so it covers each step's
  exact path, target, before-state and after-block. No new digest is needed. A
  test pins that changing any of those fields changes the digest.
- **The witness.** `WriteConsented{step}` is minted only by those two routes. It
  is not `Clone`, `Copy`, `Default` or `Deserialize`, and it is consumed by value
  at dispatch, as K-6's `RemovalCleared` is. The run loop hands a write to the
  writer only with the witness for that step. Part K's forgery check
  (`witness_forgery_violations`) is extended to it.
- **History.** Each write step's record names its consent route (`prompt` or
  `digest`).
- **The confirmation levels are unchanged.** `ConfirmationLevel` orders removal
  strength (`plan.rs:525`). A write is a different axis, and folding it in would
  make a shim plan look like a removal.

### D4 — The shim directory

- **Location.** `$XDG_DATA_HOME/macportsbrew/bin` when `XDG_DATA_HOME` is
  absolute and under the home; otherwise `~/.local/share/macportsbrew/bin`
  (SE-036, R-6). `XDG_DATA_HOME` is read beside `HOME` in
  `adapters/src/system/machine.rs` and passed down as data.
- **Never inside a provider prefix.** Each detected prefix
  (`data/policy/provider-prefixes.toml`) is compared by path components after the
  existing parents are resolved. A test runs this against both prefixes and
  against `/usr/local/bin` itself (spec 05 `:68-73`).
- **No symlink can lead out.** Walking from the home down, every component that
  exists must be a real directory owned by the user and not group- or
  world-writable (`symlink_metadata`, never followed). Any other component
  refuses with `SHIM_DIRECTORY_UNSAFE`, naming the component. Missing components
  are created `0700` with the state directory's `create_private_directory`
  discipline (`state_dir.rs:198`). A `~/.local` that is a symlink, which
  dotfile managers do, is refused, not followed. This is a Tier 4 choice:
  refusing costs those users layer 4, and following would let the link choose
  where the tool writes.
- **What belongs there.** Nothing but shims this tool's history recorded.
  `doctor` reports any other entry (D10). A write never replaces one.

### D5 — A shim: its name, its target, its write

- **The name class** (§3 item 10). The name comes from the source package's file
  list, as F-3 takes it (spec 05 `:111-115`). The class is:
  - non-empty, at most 255 bytes;
  - one leaf: no `/`, no NUL;
  - not `.` or `..`, no leading `-`;
  - UTF-8 that the plan's JSON round-trips.

  Each rule has its own test and its own mutant.
- **The target** is the destination's **stable** path (spec 05 `:75-78`). The
  adapter for each provider derives it from that provider's detected prefix, in
  `registry`, so nothing above `adapters` names a provider:
  - Homebrew: `${prefix}/opt/<formula>/bin/<exe>`;
  - MacPorts: `${prefix}/bin/<exe>`.
- **Checked twice:** at planning, and again by the writer under the run lock
  immediately before the link is made.
  1. The target exists.
  2. With its parents resolved, it lies under the detected prefix, compared by
     path components.
  3. It is owned by the destination package: its real location is in the
     destination's file list (R-21's rule for Homebrew: the real location in a
     keg of its Cellar).

  A failed check at write time halts the run (INV-03) with
  `SHIM_TARGET_NOT_DESTINATIONS`, and nothing is written.
- **The write.**
  1. Create the link under a fresh temporary name in the shim directory
     (`create_new`-equivalent: fail if it exists).
  2. Rename it over the slot.
  3. Sync the directory.

  The slot must be absent, or a symlink whose target equals the planned
  `RecordedShim{target}`. Any other content refuses with `SHIM_SLOT_OCCUPIED`,
  and the run halts before the rename.
  The link is created only in `crates/adapters/src/shim/` (D14). It is never
  followed, and its target is never written through.
- **Re-check after the write.** `read_link` must equal the planned target.

### D6 — The managed `PATH` block

- **The shell** is the base name of `$SHELL` (read beside `HOME`, passed as data):
  zsh, bash or fish. Any other shell is refused with `SHELL_NOT_SUPPORTED`,
  naming layers 1 and 2. Which file each shell reads at login, and whether
  `ZDOTDIR` applies, is R-31's answer (I-1). The plan fixes only that the answer
  is a fact, cited, never assumed (spec 05 `:188-190`).
- **The block** is exactly the marker lines of spec 05 `:195-198`, the version
  line, and one guarded prepend:

  ```
  case ":$PATH:" in *':<dir>:'*) ;; *) export PATH='<dir>'":$PATH" ;; esac
  ```

  The guard means re-sourcing never duplicates the entry (INV-16, spec 05
  `:207`). The directory is single-quoted, so an `XDG_DATA_HOME` containing a
  space or a shell metacharacter cannot become code. Fish gets its own form if
  R-31 keeps fish.
- **Finding the block.** Zero or one pair of markers, in order, is accepted.
  Anything else (two pairs, one marker alone, reversed markers) refuses with
  `MANAGED_BLOCK_MALFORMED`, and nothing is written. Content outside the markers
  is never touched; a test compares it byte for byte.
- **The procedure** follows spec 05 `:201-204`, in this order:
  1. detect;
  2. locate;
  3. find the block;
  4. compute;
  5. diff and preview, in the plan;
  6. confirm (D3);
  7. back up;
  8. write atomically;
  9. verify by re-reading;
  10. report.
- **The backup** is the original bytes, written durably into the state directory
  (`backups/<run-id>/<leaf>`, `write_new_durable`, mode `0600`) before the shell
  file is touched. Its name and digest go into the history record.
- **The write.** Under the lock, the file is re-read. Its digest must equal the
  plan's `Present{sha256}`, or the file must still be absent. A mismatch halts
  with `SHELL_FILE_CHANGED`, and nothing is written.
  - `write_atomic` is used, preserving the existing mode. A new file is created
    `0600` through the creation-mode seam.
  - A symlinked shell file refuses with `SHELL_FILE_IS_SYMLINK`, at planning and
    at write. That is the existing `write_atomic` refusal (`atomic.rs:49`), made
    a named reason. This is a Tier 4 choice and costs dotfile-manager users
    layer 4; writing through the link would edit a file the plan did not name.
- **Only the shim directory is managed** (spec 05 `:213-217`). No other variable
  is written, ever.

### D7 — `VERIFY_RESOLUTION` with a shim

The run's own `PATH` does not contain the shim directory until a new shell reads
the block. So the check evaluates the **managed `PATH`**: the shim directory
followed by the recorded `PATH` with that directory removed, exactly what the
block's line computes. It passes when:

- the name's first hit is the shim;
- `read_link` of the shim equals the planned target; and
- that target is still the destination package's (D5's third check).

The report says that open shells keep their old `PATH` until restarted, to check
`command -v <name>` in a new shell, and that `hash -r` clears a stale hash
(spec 05 `:170-173`). In this build the resolution site is `Unresolved`
(`cli/src/executor/mod.rs:810`), and nothing changes about that.

### D8 — History, recognition, and SE-092's open items

- **`history.v5`** records each write step:
  - its kind and exact path;
  - for a shim, its target;
  - for the block, the before-digest, the backup's name and digest, and the
    after-digest;
  - its consent route.
- **Recognition** of a re-run whose winner is a shim (closes SE-092 S6 (2) and
  S8 D3 for shim plans, and 04 `:753-755`). The re-run is recognised when the
  latest qualifying record (04's existing conditions) recorded that shim. It also
  needs a fresh capture to show the shim present, `read_link` equal to the
  recorded target, the target still the destination's, and the block present and
  equal to the recorded after-digest.
- `NotRecognised::PrimaryIsShim` narrows to "a shim this tool did not record, or
  whose target differs". INV-16's re-run tests are extended to the shim case.

### D9 — `env`, `env --revert`, and restore

- **`macportsbrew env`** is read-only. It shows the shell file it would use, the
  block if present, and each shim with its target and D10's validation. It
  writes nothing, ever, including during analysis or dry-run (spec 05
  `:207-209`).
- **`macportsbrew env --revert`** builds a plan that removes the block and every
  shim this tool's history recorded, and nothing else. That plan is shown,
  confirmed (D3) and refused by the execution gate in this build, like any
  other. It is a plan, not a shortcut around the gate.
  - **The block.** It is removed only if the file's bytes outside the markers are
    unchanged since the recorded write. In that case the result equals the
    recorded backup, which is checked against the backup's digest.
  - **The shell file changed outside the markers.** Only the block is removed,
    and the plan says that the backup is kept, at its name, for the user's own
    comparison. A backup is never restored over later edits.
- **Restore of a run.** `restore` (`application/src/restore.rs`) gains the
  reverse of the run's writes:
  - it removes the shims that run created, only if each slot still holds the
    recorded link;
  - it removes the block only if no other recorded shim remains and the bytes
    outside the markers still match.

  A shim the run replaced (`RecordedShim`) is reverted to its recorded earlier
  target, not deleted.

### D10 — `doctor` validates every shim on every run

This is spec 05's MUST (`:86-91`; R-6 recommendation 4). `doctor` reports, and
changes nothing:

- each shim whose target is missing;
- each shim whose target is no longer the recorded provider's;
- each entry in the shim directory that history did not record;
- each recorded shim that is missing;
- a block that is malformed, or that names a directory other than the shim
  directory;
- a shim directory present but not on the managed `PATH`.

It is built with `/add-doctor-check`.

### D11 — INV-07: the shim directory and the providers' builds

**The fact that makes this hard** (`docs/research/F-2-usr-local-interference.md`
E-9). Homebrew rebuilds `PATH` for a superenv build, so the user's `PATH` does
not reach it. The user's `PATH` **does** reach two kinds of Homebrew-run code:

- a formula that declares `env :std` (`std.rb:29` prepends `ORIGINAL_PATHS`,
  taken from the user's `PATH`);
- `post_install`, which runs with `PATH.new(ORIGINAL_PATHS)` (`formula.rb`
  `:1729-1738`), including after a bottle is poured.

A shim toward MacPorts, first on the user's `PATH`, can therefore make those runs
resolve a MacPorts tool. MacPortsBrew would have configured that, and INV-07 says
it never does. Whether MacPorts' build or activation phases inherit the user's
`PATH` is not established here.

**I-1 answers it before any code (R-30, `provider-researcher`).** R-30 establishes,
for each provider, every phase that runs with the user's `PATH` or a part of it:
build, `post_install`, activation, and `port`'s own `binpath`. It records how each
was observed or cited.

**What follows from each answer is fixed now, so it is not argued later:**

- **No phase of the other provider inherits the user's `PATH`.** D1–D10 stand.
  `invariant_07_*` pins the build environment's `PATH` from a recording, where
  one exists.
- **A phase does inherit it**, as F-2 already shows for Homebrew. The shim
  directory is then a cross-provider configuration for that provider's
  inheriting phases, and no session decides whether to accept that.
  - A DP (`/decision-pending`) records the options:
    - (a) refuse a shim whose name the inheriting provider could reach, which is
      most names;
    - (b) disclose it and accept, as the user's own `PATH` already reaches the
      other provider's `bin` and the shim changes only precedence;
    - (c) a shim form that defers to the inheriting provider during its own
      phases, which needs a detectable marker and is no longer a symlink;
    - (d) offer layer 4 only toward the provider whose phases do not inherit.
  - Part I takes the reversible default: build D1–D10 with the planner refusing
    every shim offer as `PRIMARY_NEEDS_SHIM` exactly as today, behind one
    constant named after the DP. The owner's ruling then flips one value, not a
    design.
  - `spec-guardian` is asked whether (b) is compatible with INV-07's text. Its
    view is advisory; INV-07 is not edited (§3 item 7).

### D12 — INV-12: only the consented names move

The shim directory holds only names consented to, in this run or an earlier
recorded one. The plan's impact preview lists every name whose resolution changes
when the directory is prepended: the full directory listing after the write,
against the recorded `PATH`. That list must equal the shimmed names. The test
builds a scratch `PATH` with several contested names and asserts that no other
name's first hit changes (`invariant_12_a_shim_moves_only_the_consented_names`).

### D13 — INV-17

One type, one planner rule and one writer for both directions. The target comes
from the destination adapter's stable-path rule, so the code has no direction
branch. A test runs the same offer toward Homebrew and toward MacPorts on the
captured two-provider snapshot (`both-providers-intel-macos26-live.json`). There,
`htop` toward MacPorts is today's `PRIMARY_NEEDS_SHIM` (SE-112), and the test
asserts the same shape of steps both ways.

### D14 — The `symlink` token: a staged gate change

`stop-gate.sh:211` refuses `\bsymlink\b` in all non-comment `src/` code, and its
message says nothing in this pass creates symlinks. Layer 4 is a symlink by spec
(05 `:63`), so part I needs a narrow exemption, staged for the owner (§3 items 4
and 5):

- **The patch** is `docs/passes/16-staged/I/01-shim-symlink-token.patch`. It
  admits the token only under `crates/adapters/src/shim/` (a path prefix with its
  trailing slash), the same shape the gate already uses for the elevation
  program under `process/`. Every other `src/` path keeps refusing it. The
  gate's message changes to name the one admitted site.
- `test-hooks.sh` gains a case for each side of the exemption. The patch is
  `security-reviewer`ed before it is merged as staging.
- **Ordering.** Only I-6 needs the patch. I-2..I-5 do not create a link and
  proceed meanwhile. If the patch is not applied when I-6 starts, part I becomes
  `waiting on owner docs/passes/16-staged/I/`, resuming as `wo16/I-6-…`
  (§3 item 5.3).
- **Rejected:** a wrapper-script shim (an executable file that `exec`s the
  target), which avoids the token. It deviates from spec 05's "symlinks", it
  makes the target a string inside a file rather than a link `doctor` can read,
  and choosing it to avoid a guard is the rewording `CLAUDE.md` forbids.

## Research, before code (I-1)

- **R-30 — what reaches each provider's build and post-install phases from the
  user's `PATH`.** D11. `provider-researcher`, read-only commands and official
  sources. It extends F-2 E-9 to `post_install`, `env :std`, MacPorts phases and
  `binpath`.
- **R-31 — which file each supported shell reads, on macOS, for a login and for
  an interactive shell.** D6. It covers:
  - zsh with and without `ZDOTDIR`;
  - bash's `.bash_profile`, `.bash_login` and `.profile` order;
  - fish's `conf.d`;
  - Terminal.app and iTerm opening login shells;
  - what each provider's installer writes there (R-6 E12 quotes MacPorts'
    postflight list).

  Cited from the shells' own documentation, with the dates read.

Both are docs only, with checklist entries in `docs/spec/09-research-checklist.md`.

## Slices

| Slice | Branch | What | Depends on |
|---|---|---|---|
| I-0 | `wo16/I-0-plan` | This plan and its reviews; the ledger (K merged, I in progress). Docs only | K merged |
| I-1 | `wo16/I-1-research` | R-30 and R-31, their checklist entries; D11's branch taken and recorded (a DP if a phase inherits `PATH`) | I-0 |
| I-stage | `wo16/I-stage-symlink` | D14's patch, its `README.md`, `security-reviewer`; merged as staging. The ledger notes the owner item | I-0 |
| I-2 | `wo16/I-2-steps` | Domain: `FileWrite`, `WRITE_SHIM`, `WRITE_PATH_BLOCK`, `ShimPlanned`, the name class, `check_ordering`'s three rules, `CommandTarget::Tool` removed, `plan.v3` and `migrate.v3` schemas; `ErrorId` and catalogue keys for D4–D6's refusals (`en`, `pt-BR`) | I-1 |
| I-3 | `wo16/I-3-offer` | Application: `--allow-shim`, the planner's offer (D1, D12, D13), `PRIMARY_NEEDS_SHIM` raised with layers 1 and 2, the impact preview, the gate's refusals for a write step missing its fields; behind D11's constant if a DP is open | I-2 |
| I-4 | `wo16/I-4-block` | Adapters: shell detection from data, R-31's file, block find, compute and diff, backup, the atomic write with the digest re-check (D6). No link created. Tests on a scratch home only | I-3 |
| I-5 | `wo16/I-5-consent` | D3: the per-step questions, `WriteConsented`, the run loop's hand-over, history's consent route; part K's forgery check extended | I-4 |
| I-6 | `wo16/I-6-shim` | D4, D5: the shim directory and the link writer in `crates/adapters/src/shim/`. **Needs I-stage applied** | I-5, I-stage applied |
| I-7 | `wo16/I-7-verify-history` | D7, D8: `VERIFY_RESOLUTION` on the managed `PATH`, `history.v5`, recognition; 04 `:753-755` closed for shims; SE-092's items closed | I-6 |
| I-8 | `wo16/I-8-env-restore-doctor` | D9, D10: `env`, `env --revert`, restore's reverse writes, `doctor`'s checks; 06's synopsis (`:55`, "later pass" removed) | I-7 |
| I-9 | `wo16/I-9-close` | A positive tripwire `the_switch_needs_consented_writes` (every write reaches its writer only with `WriteConsented`, and only from a plan step); ledger, `16-report.md`'s I section, CHANGELOG, wiki `Primary-Provider.md`; `docs-auditor` | I-8 |

Each code slice gets `spec-guardian` and `security-reviewer` on its diff and a
mutation proof, as part K's did, recorded in the Reviews table below.

## Tests, per invariant, by name

Every test runs against a temporary home and a temporary state directory, never
the real ones.

- **INV-01.**
  - `invariant_01_no_write_step_precedes_the_verify_destination_guarding_it`
  - `invariant_01_no_remove_source_precedes_a_shims_verify_resolution`
- **INV-02.**
  - `invariant_02_every_shim_and_block_write_is_a_plan_step_with_its_exact_path`
  - `invariant_02_no_write_is_handed_over_without_its_own_consent`
  - `invariant_02_changing_a_writes_path_or_content_changes_the_plan_digest`
  - `invariant_02_a_declined_write_question_exits_6_before_anything_runs`
- **INV-03.**
  - `invariant_03_a_shell_file_changed_since_the_plan_is_left_as_it_was`
  - `invariant_03_a_failed_block_write_leaves_the_original_and_its_backup`
  - `invariant_03_an_occupied_slot_is_never_replaced`
- **INV-06.** `invariant_06_no_write_of_layer_4_leaves_the_home` (no elevation;
  every path is under the home or the state directory).
- **INV-07.** `invariant_07_*`, as D11's branch decides.
- **INV-11.** `invariant_11_a_shim_transfers_no_ownership` (the destination's
  dependents and the source's ownership records are unchanged).
- **INV-12.**
  - `invariant_12_a_shim_moves_only_the_consented_names`
  - `invariant_12_no_shim_is_offered_over_a_non_provider_winner`
- **INV-15.** `invariant_15_a_replayed_plan_never_reads_or_writes_the_home`
- **INV-16.**
  - `invariant_16_a_second_run_writes_nothing`
  - `invariant_16_the_block_never_duplicates_its_path_entry`
  - `invariant_16_a_rerun_after_a_recorded_shim_is_recognised`
- **INV-17.** `invariant_17_the_shim_offer_is_direction_neutral`
- **§3 item 10.** One test per name-class rule, and
  `a_shim_target_outside_the_destinations_prefix_is_refused`.

## Mutants each slice must kill

- **I-2.**
  - Each name-class rule dropped, one mutant per rule.
  - `check_ordering`'s three rules, each dropped.
  - `is_mutating` false for either kind.
- **I-3.**
  - The offer made without `--allow-shim`.
  - A shim planned over `NON_PROVIDER_ENTRY_WINS`.
  - The impact preview listing only the new names.
  - D11's constant ignored.
- **I-4.**
  - The digest re-check removed.
  - The `case` guard removed.
  - A second marker pair accepted.
  - Content outside the markers re-wrapped.
  - The backup written after the shell file.
  - A symlinked shell file followed.
  - The directory unquoted.
- **I-5.**
  - The witness skipped at hand-over.
  - Consent for step *n* covering step *m*.
  - `--yes` without `--plan-digest` admitted.
- **I-6.**
  - The prefix-inside check removed.
  - The target-under-prefix check removed.
  - The ownership check removed.
  - An occupied slot replaced.
  - A symlinked `~/.local` followed.
  - The post-write `read_link` check removed.
- **I-7.**
  - `VERIFY_RESOLUTION` read against the run's own `PATH`.
  - A re-run recognised with a different target.
- **I-8.**
  - `--revert` removing an unrecorded shim.
  - A backup restored over later edits.
  - `env` writing anything.
  - `doctor` missing an orphan.

## Security notes for the reviewers

- **The first writes into the home outside the state directory.** The two paths
  are the shim directory and one shell file, both named in the plan, both
  re-checked under the lock, neither ever followed through a symlink.
- **Injection.**
  - No provider metadata becomes a path unchecked: the name class and the
    target's prefix and ownership checks (§3 item 10).
  - No value becomes shell code: the block's one variable part is the shim
    directory, single-quoted. A directory containing a single quote is refused,
    and the test feeds it one.
- **Time of check to time of use.** The writer re-checks at write time:
  - the slot's state;
  - the target's existence, prefix and owner;
  - the shell file's digest.

  A change between planning and writing halts the run; nothing is overwritten.
- **Backups** sit in the state directory, `0600`, durable, never served for
  restore over later edits.
- **No elevation**, no new program, no allowlist change (§3 item 3). `env` is
  read-only. `--revert` is a plan behind the same gate.
- **The guard change (D14)** admits one token at one path prefix. Its patch is
  reviewed on its own.

## What is not claimed

- **That layer 4 works on a real machine.** Nothing here writes outside a scratch
  home, the switch is off, and no fixture of a shimmed machine can be captured
  without running the writer. The flip proposal (part L) inherits the first real
  exercise.
- **Global priority.** It is still not offered, and nothing here moves toward it.
- **Shells other than those R-31 admits.**
- **INV-07 for every inheriting phase**, until D11's branch is taken on R-30's
  evidence.

## Needs the owner (expected)

- **D14's staged patch**, `docs/passes/16-staged/I/01-…`, applied with
  `MPB_ALLOW_GUARD_EDIT=1`. It blocks I-6.
- **The DP from D11**, if R-30 shows an inheriting phase. It blocks offering a
  shim at all (the constant), not building one.

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
