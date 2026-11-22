<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part I — layer 4, consented (DP-047 (a))

Plan written 2026-11-21, at `9354db3` (the merge of part K's last slice, PR #86),
for `prompts/16-complete-mutating-product.md` §5 I, before any code. Part I is
**the first write outside the state directory**, so `security-reviewer` on this
plan is not optional (§5 I item 5); `spec-guardian` reviews it too. Their review
is the approval (§4). Every citation was read at `9354db3`. The first draft
(`42780c5`) was revised after both reviews; each finding's resolution is in the
Reviews table, and the sections it changed carry *(rev. SG-n / SR-x)*.

**The switch stays off** (§3 item 1). Part I builds:

- the offer, the steps and the consent;
- the writers;
- `env`, restore support and `doctor`'s validation.

In this build the execution gate still refuses every plan before its first step:
the constant `THIS_BUILD_DISPATCHES_MUTATIONS` (`crates/domain/src/plan.rs:614`)
is `false`, and the gate refuses with `MutationNotInThisBuild`. The writers are
exercised only against a scratch home, by tests (§5 I item 5).

## The ruling, and what binds

DP-047, ruled 2026-10-27 and confirmed by the owner (`docs/decisions-pending.md:476`):
(b) now, which part F built, then (a) before v0.2.0, **only with consent**.

- A shim is never written implicitly. When the destination does not win `PATH`,
  the plan offers layer 4 for that executable: one shim in the MacPortsBrew-owned
  directory and one managed `PATH` block.
- The offer comes with a backup, a preview, `env --revert`, restore support, and
  idempotence per INV-16.
- Without consent, the gate refuses with `PRIMARY_NEEDS_SHIM` and names layers 1
  and 2.

Binding, in order of authority:

- **INV-01, 02 and 03.**
  - Both writes are plan steps with exact paths. So is every directory they
    create.
  - Nothing not in the plan is written. That includes the rollback.
  - A failure leaves the prior working state recoverable by the tool's own
    means.
- **INV-07.** The shim directory must not make one provider's builds resolve the
  other's tools. See D11. This is the hardest constraint of the part.
- **INV-11.** A shim transfers no ownership.
- **INV-12.** A shim changes the resolution of the consented names, and of no
  other name, under any spelling the filesystem treats as the same. Global
  priority is not offered.
- **INV-16.** Running twice changes nothing: no duplicated block, `PATH` entry
  or shim. The re-run's plan says "nothing to change".
- **INV-06, INV-15 and INV-17** hold as everywhere.
- **Spec 05.** These are Tier 3 unless marked:
  - layer 4 is the shim directory, never inside a provider prefix (`:61-73`);
  - a shim targets the provider's stable path (`:75-78`);
  - layer 4 is deterministic, inspectable, reversible, idempotent,
    self-repairing, owned and validated on every run (`:86-91`);
  - the shell-configuration MUSTs (`:188-209`);
  - managed versus reported variables (`:213-217`).
- **§3 item 10.** A shim's executable name is a single leaf: no `/`, no `..`, no
  leading `-`. Its target must exist under the destination's detected prefix and
  be owned by that destination.

## Where it lives today

- **The input.** `PrimaryDecision::Refused` carries `PrimaryReason::PrimaryNeedsShim`
  (`crates/domain/src/primary.rs:44`, `:109`), computed by
  `application::primary::decide` (`crates/application/src/primary.rs:59`):
  - refused at `:195` (not linked), `:207` (no `PATH` entry) and `:222` (`d > w`);
  - `PrimaryDecision::needing_a_shim` (`domain/src/primary.rs:173`) is "the input
    a later part's shim offer starts from";
  - F.md `:789-793` hands it to this part.
- **A shim winner today.** `decide` treats a winner owned by
  `ExecutableOwner::MacportsbrewShim` as `NonProviderEntryWins`
  (`application/src/primary.rs:163-167`). Nothing in `crates/adapters/` assigns
  that owner. Capture follows symlinks first (spec 05 `:24-26`), so a shim would
  be classified by its target. D1a changes both.
- **The refusal.** `ExecutionRefusal::PrimaryNeedsShim` is raised in
  `execution_gate.rs:1237` (`primary_refusals`, which checks that four lists are
  equal, `:1233-1237`) and declared at `:318`.
  `ErrorId::PrimaryNeedsShim` (`domain/src/error.rs:127`, exit 4) is reserved
  for "the part that offers a consented shim": defined and never raised
  (F.md:1635).
- **The steps.**
  - `StepKind` (`domain/src/plan.rs:29-50`) has no write kind.
  - `MigrationStep` (`:465`) carries `commands` and `primary`.
  - `CommandTarget::Tool` (`:321-338`) is documented for the shim directory and
    unused since F-3.
  - `check_ordering` (`:801-871`) tests positions only when a step is present.
  - The planner builds `SET_PRIMARY` with no command (`planner.rs:1047-1059`)
    and one `VERIFY_RESOLUTION` per executable list (`:1060-1080`).
  - The executor never dispatches `SET_PRIMARY` (`executor.rs:1073`, `guard`
    `:1270-1285`).
- **Consent.**
  - Per-step consent exists only for elevated provider commands:
    `ConsentGiven{step,program,argv}` (`application/src/consent.rs:119`, not
    `Clone`).
  - The plan tie is `ShownPlan`, `admits` and the digests
    (`application/src/plan_tie.rs:41`, `:66`, `:203-374`).
  - SE-114 gives removal its own domain-separated `RemovalDigest`, so that a
    script echoing `.planDigest` can never remove anything.
- **Durable writes** (`crates/adapters/src/persistence/atomic.rs`):
  - `write_atomic` (`:49`) refuses a symlink target and always publishes `0600`
    (`:38`, `:74`). It preserves no mode, owner, xattr or hard link.
  - `write_new_durable` (`:158`) publishes by `hard_link`, never replacing.
  - `write_new_with_mode` (`:201`) is a private test seam.
  - Every writer targets the state directory (`state_dir.rs:35`).
- **The provider environment.** The tool's own provider invocations get a
  constructed `PATH` (`adapters/src/process/environment.rs:34-46`, `:88-94`).
- **Not present.**
  - There is no `env` subcommand (`cli/src/app/mod.rs:102`), no managed block,
    no shell detection.
  - Restore (`application/src/restore.rs`) has no primary or shim handling
    (F.md:517).
  - The snapshot (`docs/schemas/snapshot.v1.md`) holds none of layer 4's facts.
- **History.**
  - `NotRecognised::PrimaryIsShim` (`application/src/history.rs:427`).
  - 04 `:753-755` declares that INV-16 for a re-run is not closed until shims
    record their target (SE-092 S6 and S8 D3).
- **The environment.** `std::env::var` is read only in
  `adapters/src/system/machine.rs` (`:115-126`, `:194-196`, `:206`).
- **The tooling.**
  - `stop-gate.sh:211` fails a turn on `\bsymlink\b` in non-comment `src/` code.
    One regex shares the token with `doas`, `launchctl`, `setuid` and
    `osascript`. D14 covers this.
  - The Edit guards deny any path ending in a shell file name, scratch paths
    included (`guard-protected-paths.sh:118-119`). So test files for shell
    configuration are created at test time under a temporary home, never
    committed under such a name.

## Decisions the work order leaves to the session

§4.3 lets the session make these choices. SE-115 (new, this part) records each
one when its slice lands, with the Tier 3 deviations listed in D15.

### D1 — The offer is opt-in, per run: `--allow-shim`

`plan` and `migrate` gain `--allow-shim`.

- **Without it.** Behaviour is today's, except that the refusal is raised as
  `ErrorId::PrimaryNeedsShim` (exit 4). Its text names:
  - layer 1, the destination's own `bin` already winning by the user's own `PATH`
    order;
  - layer 2, the versioned name, where the destination ships one;
  - that reordering `PATH` is global priority (INV-12), which is not offered.
- **With it.** A `PRIMARY_NEEDS_SHIM` executable becomes a planned shim if D4–D6
  admit it. Every other refusal reason stands, so a shim is never planned over
  `NON_PROVIDER_ENTRY_WINS`, `WINNER_NOT_AUTHORITATIVE` or
  `OWNED_BY_ANOTHER_PACKAGE`.
- **The flag is a request, not consent.** Consent is given per write step
  (D3).
- **The decision type.** `PrimaryDecision` gains `ShimPlanned{executables,
  path_used}`, held disjoint from `Refused` by a test.
- **Tests.** The planner's offer is reached through an allowance parameter, as
  the gate's is, not through D11's constant. That keeps D1–D10 testable while
  the constant is off *(rev. SG-17)*.

### D1a — A shim winner is recognised in capture and decided in `decide` *(rev. SG-1)*

- **Capture learns the shim directory** from the same machine read that gives
  `HOME` and `XDG_DATA_HOME` (D4). For each contested name, before following any
  symlink, it checks whether the resolved entry's parent is the shim directory,
  compared by `(st_dev, st_ino)`.
  - If it is, the owner is `MacportsbrewShim` and capture records `read_link`.
  - Only then does capture follow the link, and it records the target's owner as
    the shim's target owner.
  - This orders 05 `:24-26`'s "following symlinks first" after a shim check: a
    Tier 3 change, recorded in SE-115.
- **`decide` gains a step before step 4.** The winner is a shim in the shim
  directory:
  - target equal to the destination's stable path, and still the destination
    package's → `NothingToChange` (INV-16);
  - any other target → a planned **replacement**: a `WRITE_SHIM` of
    `Replace{from}`, offered under `--allow-shim` like a new shim;
  - target missing or not a provider's → also a replacement, and `doctor`
    reports the stale target until a plan repairs it.

  That is spec 05's "self-repairing": repair is offered as a consented plan,
  never done silently. A deviation from "self" in the strict sense, recorded in
  SE-115.
- **The way back.** A shim toward MacPorts, then a migration back to Homebrew,
  is a planned replacement or a removal. The removal applies when the Homebrew
  `bin` already wins without the shim, which `decide` evaluates on `PATH` with
  the shim directory removed.
- **Tests:**
  - `invariant_16_a_shim_winner_pointing_at_the_destination_is_nothing_to_change`
  - `invariant_17_a_shimmed_primary_can_be_moved_back`

### D2 — Two step kinds, three operations

Two step kinds are added, `WRITE_SHIM` and `WRITE_PATH_BLOCK`. Each carries a
`FileWrite` with an operation: `Create`, `Replace{from}` or `Remove{expect}`
*(rev. SG-4 (d))*.

```
migrate:  … VERIFY_DESTINATION -> SET_PRIMARY -> [WRITE_SHIM ...] -> [WRITE_PATH_BLOCK]
          -> VERIFY_RESOLUTION -> [REMOVE_SOURCE …] -> RECORD_HISTORY
revert:   SNAPSHOT_BEFORE -> [WRITE_PATH_BLOCK Remove] -> [WRITE_SHIM Remove ...] -> RECORD_HISTORY
```

**`WRITE_SHIM`**, one per name, carries:

- the shim's exact path;
- for `Create` and `Replace`, the target;
- the slot's planned state: `Absent`, or `RecordedShim{target}`;
- the directories it creates (D4, *rev. SR-H4 / SG-12*).

**`WRITE_PATH_BLOCK`**, at most one per plan, carries:

- the shell file's exact path;
- its planned state: `Absent`, or `Present{sha256, mode, nlink, owner, dev,
  ino}`;
- the block before and after;
- the preview diff, with **zero context lines**, so it contains only lines this
  tool owns *(rev. SR-M4 / SG-2)*.

No byte of the file outside the markers enters the plan, the snapshot, `--json`
or history: only the digest. A test feeds a file holding a token-like line and
asserts it appears nowhere.

**Rollback is plan data** *(rev. SG-3)*. Every `Create` and `Replace` carries its
reverse:

- `Remove{expect: the written link}`;
- the restore of `from`;
- the block's removal.

The reverse is shown, digested and consented with the write. When a later step of
the same primary fails, for example `VERIFY_RESOLUTION`, the executor runs the
reverses of this run's writes in reverse order. They come from the plan, never
derived (INV-02), and run with D5 and D6's checks. If a reverse cannot run, the
run ends `PARTIAL` with that write recorded and restorable.

**Not `PlanCommand`.** A write has no program and no argv. `CommandTarget::Tool`
is removed: it was reserved for this, and this design does not use it. That is a
breaking change for `plan.v3`, recorded in D15.

**`is_mutating`** is true for both kinds (`plan.rs:59`, exhaustive).

**`check_ordering`** gains these rules, each over **every** occurrence, never
`.position()`'s first one (F-06's trap) *(rev. SG-4 (a)(b))*:

1. A `Create` or `Replace` write requires a `VERIFY_DESTINATION` for the same
   destination **present** and before it, and a `VERIFY_RESOLUTION` **present**
   and after it.
2. Every `WRITE_SHIM` with `Create` or `Replace` precedes the `WRITE_PATH_BLOCK`
   with `Create` or `Replace`.
3. In a revert, the `WRITE_PATH_BLOCK` `Remove` precedes every `WRITE_SHIM`
   `Remove`, so `PATH` never points at a half-emptied directory.
4. A plan never mixes `Remove` with `Create` or `Replace` for the same path.
5. No `REMOVE_SOURCE` precedes the `VERIFY_RESOLUTION` (INV-01, unchanged).

**Five lists are equal** *(rev. SG-4 (c))*. The gate's `primary_refusals` check
(`execution_gate.rs:1233-1237`) grows from four lists to five, adding the set of
`WRITE_SHIM` names equal to `ShimPlanned.executables` and to
`VERIFY_RESOLUTION`'s list.

**Schemas:** `plan.v3` and `migrate.v3` (D15).

### D3 — Consent per write step, and a write digest

Each write step needs its own consent, the analogue of elevation's
`ConsentGiven`. J will build both on one shape.

- **Interactive.** After Q0, and Q1 or Q2 where a removal asks, the tool asks one
  question per write step. Each shows:
  - its exact path, escaped *(rev. SG-18 / SR-M3)*;
  - its target or zero-context diff;
  - its reverse.

  A decline exits 6 before anything runs.
- **Scripts: a domain-separated write digest** *(rev. SG-5)*. It mirrors SE-114's
  removal digest, so that a script written for installs can never write into a
  shell file.
  - `writeDigest` is SHA-256 under the domain `macportsbrew-write-digest/1` over
    the plan's JSON. It is required beside `--plan-digest` whenever a write step
    is present.
  - A test pins that it never equals the plan digest.
  - Another test pins that changing any write field changes it: path, target,
    slot state, directories, before-state, block, reverse.
- **The digested plan carries an undisplayed identity of the home** *(rev. SR-M2
  / SG-19)*: the SHA-256 of the absolute, unredacted home path and its
  `(st_dev, st_ino)`. A run under another `HOME` therefore fails `admits`. The
  writer also refuses when the run's home is not the plan's.
- **The witness.** `WriteConsented{plan_digest, step}` is bound to the plan as
  well as the step *(rev. SR-L6)*.
  - It is minted only by those two routes.
  - It is not `Clone`, `Copy`, `Default` or `Deserialize`.
  - It is consumed by value at dispatch.
  - The run loop hands a write to the writer only with the witness for that step
    of that plan.
  - Part K's `witness_forgery_violations` is extended to it.
- **History** records each write step's consent route (`prompt` or `digest`).
- **The confirmation levels are unchanged.** `ConfirmationLevel` (`plan.rs:525`)
  orders removal strength. A write is a different axis.
- **K-6's `invariant_01_only_add_and_remove_are_ever_dispatched`** is renamed
  `invariant_01_only_add_remove_and_layer_4_writes_are_ever_dispatched`. The
  closed set gains exactly the two kinds; that is stated, not weakened silently
  *(rev. SG-5)*.

### D4 — The shim directory

- **Location.** `$XDG_DATA_HOME/macportsbrew/bin` is used when `XDG_DATA_HOME`
  is absolute, contains no `.` or `..` component, and lies under the home
  *(rev. SR-M6 / SG-16)*. Otherwise the directory is
  `~/.local/share/macportsbrew/bin` (SE-036, R-6).
  - `XDG_DATA_HOME` is read beside `HOME` in `adapters/src/system/machine.rs`
    and passed down as data.
  - The directory's path must contain no `'`, `:`, newline, NUL or control
    character *(rev. SR-M8 / SG-6)*: a `:` would split the `PATH` entry.
- **Never equal to, inside or above a provider prefix** *(rev. SR-M7)*. The
  prefixes compared are the union of:
  - those in `data/policy/provider-prefixes.toml`;
  - the prefixes actually detected, which may be custom and may be under the
    home;
  - `/usr/local`.

  Comparison uses H's inode- and firmlink-aware comparison (`invariant_07_a_firmlink_spelling_…`,
  `…_unicode_fold_…`), by `(st_dev, st_ino)` ancestry, never component strings
  *(rev. SR-H2 / SG-16)*. A test covers both prefixes, a custom one under the
  home, and `/usr/local/bin` itself (spec 05 `:68-73`).
- **No symlink can lead out** *(rev. SR-M6)*.
  - The home must be a real directory, owned by `effective_uid()`
    (`machine.rs:163`, not whatever `$HOME` claims), and not group- or
    world-writable.
  - From the home down, every component that exists must meet the same test
    (`symlink_metadata`, never followed).
  - Anything else refuses with `SHIM_DIRECTORY_UNSAFE`, naming the component.
  - The writer refuses euid 0 itself (`run_lock.rs:147` is not relied on).
  - A `~/.local` that is a symlink, as dotfile managers make, is refused, not
    followed. This is a Tier 4 choice.
  - **ACLs that grant write are not detected.** No std interface reads them;
    reading them would need `libc` (outside part E's ADR purposes) or a new
    program (§3 item 3). That is declared, not claimed.
- **Creating a missing component is a write in the plan** *(rev. SR-H4 /
  SG-12)*. The first `WRITE_SHIM` that needs the directory lists each directory
  it creates, digested.
  - They are created only at dispatch, never in `plan`, analysis or dry-run.
  - Each is `0700`, with `create_private_directory`'s discipline
    (`state_dir.rs:198`).
  - History records them, and a revert removes each only when empty.
- **What belongs there.** Nothing but shims this tool's history recorded. A write
  never replaces anything else (D5).

### D5 — A shim: its name, its target, its write

- **The name class**, an allowlist *(rev. SR-M3 / SG-6)*:
  - bytes `[A-Za-z0-9._+-]`, 1–255 of them;
  - no leading `.` (the writers' temporaries are dot-names that listings
    ignore) and no leading `-`;
  - not `.` or `..`.

  This rejects `/`, NUL, whitespace, newline, control characters, ESC and every
  ANSI sequence. A name outside it refuses with `SHIM_NAME_NOT_ADMITTED`, naming
  it escaped. Each rule has its own test and its own mutant. The Homebrew
  `<formula>` component of the target is checked by the same class, since it is
  also metadata.
- **Names the shell does not resolve by `PATH`** *(rev. SR Info)*. Builtins and
  reserved words of the detected shell (`cd`, `[`, `time`, …) are refused with
  `SHIM_NAME_IS_SHELL_BUILTIN`. The list comes from R-31, as dated data in
  `data/policy/`. Aliases and functions cannot be seen, so D7's report says so.
- **Case and normalisation** *(rev. SR-H2)*.
  - Two planned names equal after case folding and Unicode NFD refuse together,
    with `SHIM_NAMES_COLLIDE`, before anything is written.
  - A planned name whose folded form matches another name on any `PATH` entry
    that is not the same executable refuses with `SHIM_NAME_SHADOWS_ANOTHER`.
    That is the system `HEAD` against a `head` shim.
- **The target** is the destination's stable path (spec 05 `:75-78`). The
  provider's adapter derives it from its detected prefix in `registry`, so
  nothing above `adapters` names a provider:
  - Homebrew: `${prefix}/opt/<formula>/bin/<exe>`, or `/sbin/` for an `sbin`
    executable;
  - MacPorts: `${prefix}/bin/<exe>`, or `${prefix}/sbin/<exe>` *(rev. SG-15)*.

  It must be absolute, with no `.` or `..` component *(rev. SR-M3)*.
- **Checked at planning, from the snapshot, and again under the run lock** just
  before the link is made:
  1. The target exists.
  2. With its parents resolved, it lies under the detected prefix by `(st_dev,
     st_ino)` ancestry.
  3. It is the destination package's: its real location is in the
     destination's file list (R-21's rule for Homebrew).

  A failure halts the run with `SHIM_TARGET_NOT_DESTINATIONS`. A MacPorts name
  that `port select` manages can fail check 3, because its link points into the
  select group. The text then names layer 3 *(rev. SG-15)*. Each check has its
  own test *(rev. SG-6)*.
- **The write** *(rev. SR-M5 / SG-6)*:
  - For `Absent`, the link is created directly at the slot. The call fails with
    `EEXIST` if anything is there, so nothing is ever replaced.
  - For `Replace{from}`, a link is created at a fresh temporary name. The slot is
    re-read and must still be a symlink whose `read_link` equals `from`. The link
    is then renamed over the slot. The window between that re-read and the
    rename is the residual, stated in 04's style.

  Any other slot content refuses with `SHIM_SLOT_OCCUPIED`.
- **Re-check after the write.** `read_link` must equal the planned target. The
  link is never followed for writing.
- **Where the link is created.** Only in `crates/adapters/src/shim/` (D14).

### D6 — The managed `PATH` block

- **The shell.** zsh and bash only. Fish is refused with `SHELL_NOT_SUPPORTED`:
  spec 05 asks for fish "where practical", and an owned `conf.d` file would be a
  second form of the block. That is declared in D15.
  - The shell is the base name of `$SHELL`, read beside `HOME`. `$SHELL` can
    differ from the account's login shell, and reading that needs `getpwuid`
    (`libc`) or a new program. So the plan names the file it will write in the
    question, and the user sees it before consenting. That is declared, not
    claimed *(rev. SR-L3)*.
  - Other shells refuse with `SHELL_NOT_SUPPORTED`, naming layers 1 and 2.
- **The file is R-31's answer.** R-31 says which file each shell reads at login
  and in an interactive shell, with and without `ZDOTDIR`. The writer never
  creates a file whose existence changes which other startup file the shell
  reads *(rev. SR-H1)*.
  - Bash reads only the first of `.bash_profile`, `.bash_login` and `.profile`
    that exists, so a new `.bash_profile` would silence `.profile`. If the file
    the shell reads today exists, the block goes there.
  - If no candidate exists, the file the shell would read first is created.
  - Otherwise the writer refuses with `SHELL_FILE_WOULD_SHADOW`.
  - Test: `invariant_03_creating_the_shell_file_never_shadows_another_startup_file`.
  - A `ZDOTDIR` outside the home refuses (INV-06 below) *(rev. SG-7)*.
- **macOS `path_helper`** (`/etc/zprofile`) reorders `PATH` for login shells. R-31
  establishes where the block must sit to come first after it, and D7 labels the
  result a prediction *(rev. SR-H1 / SG-11)*.
- **The block** is the marker lines of spec 05 `:195-198`, the version line, and
  one guarded prepend:

  ```
  case ":$PATH:" in *':<dir>:'*) ;; *) export PATH='<dir>'":$PATH" ;; esac
  ```

  - The guard means re-sourcing never duplicates the entry (INV-16).
  - The directory is single-quoted and D4 already refuses `'`, `:`, newline, NUL
    and control characters, so it cannot become code. The test feeds each.
  - **The version line is not compared** *(rev. SG-14)*. Two blocks are equal
    when they are equal without it, and D8's after-digest is computed without
    it, so an upgrade does not rewrite the block.
- **Finding the block** *(rev. SR-M8)*:
  - A marker is a whole line with exact bytes.
  - Any other line containing the marker text, two pairs, one marker alone, or
    reversed markers refuses with `MANAGED_BLOCK_MALFORMED`.
  - CRLF line ends are handled explicitly and kept as found.
  - The read is bounded, at 1 MiB, refused above it.
  - Content outside the markers is never touched, compared byte for byte, bytes
    as bytes with no lossy decoding.
  - A file not ending in a newline gets one, as part of the block. The test
    covers that.
- **The procedure** is spec 05 `:201-204`, in this order: detect, locate, find,
  compute, diff and preview, confirm (D3), back up, write, re-read, report.
- **The backup** *(rev. SR-L2 / SG-20)*:
  - the original bytes, written by `write_new_durable` (`0600`) to
    `backups/<run-id>/shell-file` in the state directory, before the shell file
    is touched;
  - a non-dot name, with its mapping, name and digest in history;
  - kept while the run's record is restorable, and removed by the restore or
    revert that consumes it;
  - never shown by `doctor --report`.
- **The writer** is a dedicated one, not `write_atomic` *(rev. SR-M1 / SG-7)*.
  - **It refuses:**
    - a non-regular file (a FIFO would hang the read);
    - an owner other than the effective uid;
    - `nlink != 1`, since the rename would break a hard-linked dotfile;
    - `uchg` or `schg` set (`std::os::macos::fs::MetadataExt::st_flags`);
    - a symlink: `SHELL_FILE_IS_SYMLINK`, at planning and at write. This is a
      Tier 4 choice that costs dotfile-manager users layer 4.
  - **The same walk as D4** applies to every parent of the shell file, so a
    symlinked `ZDOTDIR` is refused like `~/.local`.
  - **Under the lock:**
    1. One bounded read gives the bytes, the digest, the backup and the computed
       block.
    2. The digest and `(dev, ino, size, mtime)` must match the plan's
       `Present`, or the file must still be absent. A mismatch halts with
       `SHELL_FILE_CHANGED`, and nothing is written.
    3. The new bytes go to a temporary sibling, which is `fchmod`-ed to the
       original mode.
    4. `lstat` is re-checked immediately before the rename.
    5. The rename, then a directory sync.
  - **A new file** is published with `write_new_durable`'s no-replace link, at
    `0600`.
  - **What is lost.** ACLs and xattrs do not survive the rename and cannot be
    read without `libc`. The question says the file is replaced. The window
    after the last `lstat`, against an editor that does not take this tool's
    lock, is the stated residual.
- **Only the shim directory is managed** (spec 05 `:213-217`).

### D7 — `VERIFY_RESOLUTION` with a shim

The run's own `PATH` does not contain the shim directory until a new shell reads
the block. The check therefore evaluates the **managed `PATH`**: the shim
directory first, then the recorded `PATH` with that directory removed. It passes
when:

- the name's first hit is the shim;
- `read_link` equals the planned target;
- the target is still the destination package's.

**It is labelled a prediction** *(rev. SR-H1 / SG-11)*. The user's startup files,
later prepends, `path_helper`, aliases, functions and builtins can each change
what a shell runs. The report says so, as spec 05 `:168-173` does. It tells the
user to check `command -v <name>` in a new shell, and that `hash -r` clears a
stale hash.

In this build the resolution site is `Unresolved` (`cli/src/executor/mod.rs:810`),
and nothing changes about that.

### D8 — History, the intent record, and recognition

- **The intent record comes before every write** *(rev. SG-3)*. The run marker
  (`run_marker.rs`, `run-marker.v3`) gains each write step's pending intent:
  - path;
  - target or block digest;
  - backup name;
  - directories to create.

  It is written durably before the write and cleared with the run. A run killed
  between a link and its record therefore leaves an intent that `restore` of the
  interrupted run consumes. Test:
  `invariant_03_a_run_killed_after_the_link_is_restorable`.
- **`history.v5`** records each write step:
  - its kind, operation and exact path, home-redacted like the plan, and error
    text redacted the same way *(rev. SR-M4)*;
  - for a shim, its target;
  - for the block, the before-digest, the backup's name and digest, and the
    after-digest without the version line;
  - the directories created;
  - the consent route.
- **Recognition** of a re-run whose winner is a shim closes SE-092 S6 (2) and S8
  D3 for shim plans, and 04 `:753-755`. A re-run is recognised when:
  - the latest qualifying record recorded that shim;
  - a fresh capture shows it present, with `read_link` equal to the recorded
    target, still the destination's;
  - the block is present and equal to the recorded after-digest.

  D1a's `NothingToChange` makes the same answer reachable without history
  *(rev. SG-1)*.
- **`NotRecognised::PrimaryIsShim`** narrows to "a shim this tool did not record,
  or whose target differs".

### D9 — `env`, `env --revert`, and restore

- **`macportsbrew env` writes nothing, ever.** It shows the shell file it would
  use, the block if present, and each shim with its target and D10's
  validation, all escaped. `env --json` is `env.v1`.
- **`macportsbrew env --revert`** builds a revert plan (D2), which is shown,
  consented (D3) and refused by the execution gate in this build, like any
  other plan.
  - It removes the block and each recorded shim **whose slot still holds the
    recorded link**: `symlink_metadata` shows a symlink whose `read_link` equals
    the recorded target *(rev. SR-H3 (1))*.
  - Anything else at a recorded name is left and reported, never removed.
  - Recorded directories are removed only when empty, deepest first
    *(rev. SR-H3 (3))*.
  - **The block, with the file unchanged outside the markers.** The result
    equals the recorded backup, checked by digest.
  - **The block, with the file changed outside the markers.** Only the block is
    removed. The plan says the backup is kept, at its name, for the user's own
    comparison, and is never restored over later edits.
  - The diff shows any user edits **inside** the markers, which removal would
    lose *(rev. SR-L7)*.
  - Removing the block where the file changed outside it is a reading of 05
    `:205` ("removes the block and restores"), recorded in D15.
- **Restore of a run** (`application/src/restore.rs`) gains the reverse of the
  run's writes, from history or from D8's intent record. The rules are the same
  as revert's.
  - A shim the run replaced is put back to its recorded earlier target only if
    D5's three target checks pass on that target today. Otherwise it is
    removed, and the plan says so. State never becomes a link target unchecked
    *(rev. SR-H3 (2))*.

### D10 — `doctor` validates every shim

This is spec 05's MUST (`:86-91`; R-6 recommendation 4), met in `doctor`. A run
also re-checks every shim it plans or writes. "Validated on every run" is read
as both of these, recorded in D15 *(rev. SG-10)*.

`doctor` reports, and changes nothing:

- each shim whose target is missing, or no longer the recorded provider's;
- each unrecorded entry in the shim directory. An entry named after a
  security-sensitive tool (`ssh`, `git`, the elevation programs, kept as data) is
  reported at a higher severity *(rev. SR-L4)*;
- each recorded shim that is missing;
- a block that is malformed, or names another directory;
- a shim directory present but not on the managed `PATH`;
- leftover temporaries from a killed run, both the shell file's sibling and a
  dot-temp link *(rev. SR-L5)*;
- the count of backups kept.

It keeps the existing writable-`PATH`-entry warning for the shim directory
(`doctor.rs:457-500`) honestly, rather than exempting it *(rev. SR-L4)*. It is
built with `/add-doctor-check`.

### D11 — INV-07: the shim directory and the providers' phases

**The fact that makes this hard** (`docs/research/F-2-usr-local-interference.md`
E-9). Homebrew rebuilds `PATH` for a superenv build, but the user's `PATH`
reaches two kinds of Homebrew-run code:

- a formula that declares `env :std` (`std.rb:29`, `ORIGINAL_PATHS`);
- `post_install` (`formula.rb:1729-1738`), including after a bottle is poured.

A shim toward MacPorts, first on the user's `PATH`, can make those runs resolve
a MacPorts tool. A shimmed `pkg-config`, a `*-config` or a compiler would make an
`env :std` build link MacPorts libraries. MacPortsBrew would have configured
that, and INV-07 says it never does.

**Always true, and tested unconditionally** *(rev. SG-9 / SR Info)*. The tool's
own provider invocations never see the shim directory, because they get a
constructed `PATH` (`environment.rs:34-46`).
`invariant_07_the_shim_directory_never_enters_a_provider_invocations_path`
puts the shim directory first in the user's `PATH` and asserts the constructed
one. No fixture is needed.

**I-1 answers the rest before any code (R-30, `provider-researcher`).** R-30
establishes, for each provider, every phase that runs with any part of the user's
`PATH`, and **the composed order** *(rev. SG-9)*:

- build, `env :std`, `post_install`, activation;
- `brew`'s own non-build subprocesses: F-2's row 501, "not investigated". A
  shimmed `git` reaches `brew update`;
- `port`'s `binpath`;
- the elevated MacPorts route, through `sudo`'s `env_reset` and `secure_path`.

For each, R-30 records whether the provider's own directories come ahead of the
inherited part.

**What follows from each answer is fixed now:**

- **No phase of the other provider inherits a `PATH` in which the shim
  directory could come first.** D1–D10 stand, with the unconditional test above.
- **A phase does inherit it**, as F-2 shows for Homebrew. The shim directory is
  then a cross-provider configuration for that phase. A DP (`/decision-pending`)
  records it.
  - It is **the owner's alone**: option (b) needs an interpretation of a Tier 1
    invariant, not a Tier 3–5 trade-off. `spec-guardian`'s advisory view
    (Reviews) is that (b) is not compatible with INV-07's text except where the
    shim changes nothing.
  - The options:
    - (a) refuse a shim whose name the inheriting provider's phases could reach;
    - (c) a shim form that defers to the inheriting provider during its own
      phases, which needs a detectable marker, is no longer a symlink, and
      reopens D14 and 05 `:63`;
    - (d) offer layer 4 only toward the provider whose phases do not inherit,
      decided from dated per-provider data (INV-20) with no direction branch,
      which is how it stays within INV-17;
    - (b) disclose and accept, listed only as the owner's possible reading of
      INV-07.
  - Part I takes the reversible default. It builds D1–D10, and the planner
    refuses every shim offer as `PRIMARY_NEEDS_SHIM`, exactly as today, behind
    one constant named after the DP. Tests reach the offer through D1's
    allowance parameter. The owner's ruling flips one value, not a design.
  - INV-07 is not edited (§3 item 7). A deny-list of dangerous names would be
    mitigation, not compliance, and is not offered as the answer.

### D12 — INV-12: only the consented names move

The impact preview lists every name whose resolution changes when the block's
directory is prepended. It is computed from the shim directory as it will be
after this run's writes, against the recorded `PATH`, with names compared
case-folded and NFD-normalised (D5).

That list must equal this run's names **plus** the recorded shims already there
*(rev. SG-11)*. Writing a block that was deleted, for example, re-activates names
consented in earlier runs. Every name is shown and digested.

Tests:

- `invariant_12_a_shim_moves_only_the_consented_names` builds a scratch `PATH`
  with several contested names, a case-variant system name, and a recorded
  earlier shim, and asserts that no other name's first hit changes;
- `invariant_12_no_shim_is_offered_over_a_non_provider_winner`.

### D13 — INV-17

There is one type, one planner rule and one writer for both directions. The
target comes from the destination adapter's stable-path rule, so the code has
no direction branch.

On `both-providers-intel-macos26-live.json`, `htop` toward MacPorts is today's
`PRIMARY_NEEDS_SHIM` (SE-112), while toward Homebrew it is `NothingToChange`
(`crates/application/tests/primary_decision.rs:165-177`). So that snapshot
cannot show the same steps both ways *(rev. SG-8)*.

- **I-3 searches the captured recordings** for a case that is `PRIMARY_NEEDS_SHIM`
  toward Homebrew. A keg-only formula is `NotLinked` and reaches step 5. Note
  that `m4` and `ncurses` lose to `/usr/bin` first.
- **If none exists**, the test edits the snapshot's `PATH` and says so, as
  `htop_both_ways` already does. An owner capture item asks for a machine where
  MacPorts' `bin` comes first (§3 item 9).

The test is `invariant_17_the_shim_offer_is_direction_neutral`. D1a's way back
is `invariant_17_a_shimmed_primary_can_be_moved_back`.

### D14 — The `symlink` token: a staged gate change

`stop-gate.sh:211` refuses `\bsymlink\b` in all non-comment `src/` code. Layer 4
is a symlink by spec (05 `:63`). Part I therefore stages a narrow exemption for
the owner (§3 items 4 and 5): `docs/passes/16-staged/I/01-shim-symlink-token.patch`,
with its `README.md`.

**Token-specific** *(rev. SG-13 / SR-L1)*. `symlink` is split out of the shared
regex, and only it is admitted. It is admitted only in one file,
`crates/adapters/src/shim/link.rs`, and only at a pinned number of hits, the way
the probe exemption is pinned. Everything else stays refused, there and
everywhere:

- `symlinkat`, `doas`, `launchctl`, `setuid` and `osascript`;
- `soft_link`, the deprecated alias the regex misses today, which the same patch
  adds.

**Complete:**

- the generated `.codex/hooks/` copy, or the drift stage fails, as P/03 did;
- cases in both `scripts/test-gates.sh` and `.claude/hooks/test-hooks.sh`, for
  each side of the exemption, and that `setuid`, `launchctl` and `osascript`
  still fail in `link.rs`;
- `.claude/rules/adapters.md`'s "nothing here … creates a symlink", amended
  through the same staging if its path is guarded.

`security-reviewer` reviews the patch before it merges as staging.

**Ordering.** Only I-6 needs the patch applied. If it is not applied when I-6
starts, part I becomes `waiting on owner docs/passes/16-staged/I/`, resuming as
`wo16/I-6-…` (§3 item 5.3).

**Rejected:** a wrapper-script shim. It deviates from spec 05's "symlinks", it
hides the target from `read_link`, and choosing it to avoid a guard is the
rewording `CLAUDE.md` forbids. D11 (c) would reopen this, and says so.

### D15 — What SE-115 must state *(rev. SG-10)*

- **Tier 3 readings:**
  - 05 `:205`, revert removes only the block when the file changed (D9);
  - 05 `:88`, "validated on every run" as `doctor` plus the run's own re-checks
    (D10);
  - 05 `:24-26`, the shim check before following symlinks (D1a);
  - "self-repairing" as a consented repair plan (D1a);
  - fish not offered (D6);
  - 05 `:201-209`, the guard removes every occurrence of the shim directory
    and prepends it once, which can move an occurrence the user placed
    themselves (A14);
  - the bounded text scan of the home's `.zshenv` for `ZDOTDIR`, a new read of
    a user file that refuses on sight without parsing (A15);
  - the placement choice and the declared interactive non-login gap (A16);
  - zsh module builtins admitted and reported, not refused (A17).
- **The state machine.** AGENTS.md's diagram gains the two kinds and the revert
  form. 04 `:10-17` already places them under "ACTIVATE (primary)", so 04 is
  not violated, and SE-115 says so.
- **Breaking.** `CommandTarget::Tool` / `TOOL` removed in `plan.v3`.
- **Schemas:**
  - `plan.v3`, `migrate.v3`;
  - `history.v5`, `run-marker.v3`;
  - `snapshot.v2` (D16);
  - `restore.v3` (the reverse writes);
  - `doctor.v1`'s additions, or `doctor.v2` if they are not additive;
  - `env.v1`.
- **The new `ErrorId`s** and their exit codes, in 06's table.
- **The declared residuals:**
  - ACLs and xattrs;
  - `$SHELL` versus the login shell;
  - the check-to-use windows the run lock does not close (an editor, a dotfile
    manager, `brew` moving `opt`) *(rev. SG-20)*;
  - aliases and functions.

### D16 — Capture: what planning reads, and replay *(rev. SG-2)*

Planning decides only from the snapshot (INV-15). `snapshot.v2` gains a
`layer4` section, captured under `FilesystemScope` only:

- `SHELL`, `XDG_DATA_HOME` and `ZDOTDIR` as read;
- the home's identity `(dev, ino)` and hashed path;
- the shell file's candidates and, for each, presence, kind, mode, owner,
  `nlink`, flags, size and sha256 — **no bytes outside the markers**;
- the block's own bytes;
- each path component's kind, owner and mode;
- the shim directory's listing, each entry's kind and `read_link`;
- each planned target's existence, real location and owner.

Under `Elsewhere`, as in a replay, the section is absent. An absent fact is the
named refusal `LAYER4_FACTS_UNKNOWN`, never an inferred `Absent` slot or file.
`--from-snapshot` and `--replay` therefore give byte-identical plans without
touching the home. Test: `invariant_15_a_replayed_plan_never_reads_or_writes_the_home`.

## Amendments after the second review (`8ffe78b`)

Each item **overrides** the text of the decision it names. A slice implements the
decision as amended here.

**A1 — Only a recorded entry is this tool's** (D1a, D2, D4, D5; SR-N1).

- Capture labels a winner `MacportsbrewShim` only when it meets all of:
  - it is a symlink;
  - its path and its `read_link` are named by a qualifying history record.
- Any other entry in the shim directory is **foreign**: a link nobody recorded, a
  regular file, anything else. It is:
  - refused as a non-provider winner;
  - never offered for `Replace`;
  - reported by `doctor`.
- An unrecorded link that happens to point at the destination is still foreign
  to `decide`. `doctor` shows it as unrecorded.
- Mutant: an unrecorded entry offered for `Replace`.

**A2 — The home identity is digest input, never a field** (D3, D16; SR-N2,
SG-N2).

- **The digest.** The identity, `(st_dev, st_ino)` of the home plus its
  absolute path, is fed into `writeDigest`'s and the plan digest's computation
  beside the plan bytes. It is never serialised: not into `--json`, the plan,
  history, a snapshot or `doctor --report`. "Undisplayed" means exactly that.
- **`snapshot.v2`.** Neither the hashed path nor the inode is kept. `XDG_DATA_HOME`,
  `ZDOTDIR` and the block's bytes are home-redacted the way `PATH` is.
  `capture-fixture.sh` keeps that redaction.
- **Writers.** The writers check the run's home against the identity the digest
  was computed with.
- **Test.** `no_document_contains_the_home_path_or_an_unkeyed_hash_of_it`.

**A3 — Reverses are data inside the forward step** (D2, D3; SG-N1, SR-N3, SG-N7).

- **Scope.** A reverse is not a step. `check_ordering` does not see it, and
  rule 1 does not apply to it. D5 and D6's checks do apply, at the moment it
  runs.
- **Consent.** The forward step's `WriteConsented` is consumed at its dispatch
  together with a `ReverseCovered{plan_digest, step}`. That token is minted from
  the same consent, is not `Clone`, and is held only by the run loop until the
  run ends. A reverse runs only with it.
- **The reverses:**
  - a block `Replace` restores the previous block;
  - a block `Create` on a file that was absent removes the file only if its bytes
    equal exactly what was written, including the newline D6 adds; otherwise it
    removes only the block;
  - a shim `Replace` restores `from`, after D5's three checks on `from`;
  - a shim `Create` removes the link only while `read_link` equals what was
    written.
- **Order.** Reverses run in reverse order of the writes, so the block comes
  before the shims, mirroring rule 3.
- **Semantics.** This changes 04 `:37-38`'s failure semantics: the executor runs
  planned reverses instead of only halting. It is a Tier 3 change, added to D15.
- **Mutants:**
  - a reverse run without its token;
  - a `Create` reverse deleting a file whose bytes changed.

**A4 — Restore's ordering rule** (D2, D9; SG-N1).

- **The rule.** A restore plan's `WRITE_SHIM Replace` back to an earlier provider
  needs no `VERIFY_DESTINATION`. In its place, D5's three target checks run at
  planning and again at write, and a `VERIFY_RESOLUTION` must be present after it.
- **Placement.** `check_ordering` gains this as rule 6, keyed on the plan's kind
  (`restore`), so a `migrate` plan cannot use it.
- **Mutant:** rule 6 admitted for a `migrate` plan.

**A5 — The way back inside `migrate`** (D1a, D2; SG-N4).

- **The rule.** A `WRITE_SHIM Remove` in a `migrate` plan requires a
  `VERIFY_RESOLUTION` after it, rule 7. That check is evaluated on the managed
  `PATH` without the removed name.
- **The block.** It stays while any other recorded shim remains. It is removed,
  as a `WRITE_PATH_BLOCK Remove` before the shim removals, only when none
  remains.

**A6 — `LAYER4_FACTS_UNKNOWN` has a scope** (D16, INV-15; SG-N3, SR-N4).

- **When it refuses.** It refuses an executable only if that executable would
  otherwise be offered a shim or recognised as one. Every other plan is
  unchanged. Existing `--replay` and `--from-snapshot` cases keep their outputs,
  which the test suite pins.
- **Old snapshots.** `snapshot.v1` still loads, and reads as "no `layer4`
  facts".
- **A replayed shim winner.** Under replay, a winner in a directory capture
  could not check is `Unknown`, never classified by its target's owner. So a
  replayed shimmed machine cannot read as `NothingToChange` through step 3.
- **New facts for D5 and D12.** `layer4` gains, for each `PATH` entry, the folded
  name set (names only) and each name's resolved `(dev, ino)`. "The same
  executable" means the same resolved `(dev, ino)`. These facts are absent under
  `Elsewhere`, and their absence is `LAYER4_FACTS_UNKNOWN`.
- **Reading at capture.** Capture reads the shell-file candidates with the
  bounded, no-follow reader and its non-regular refusal (SR-N6), so a FIFO cannot
  hang `plan`.

**A7 — `SHIM_NAME_SHADOWS_ANOTHER`, worded narrowly** (D5; SG-N5). It refuses
when a **different spelling** folds to the planned name and does not resolve to
the same executable. The planned name's own byte-equal entries, the source's copy
among them, never refuse.

**A8 — The formula component has its own class** (D5; SR-N5). It uses Homebrew's
formula-name characters plus `@`: `python@3.12` is admitted. `/`, `..`, a
leading `-`, NUL, whitespace and control characters stay refused.

**A9 — D13's fallback** (SG-N6). If I-3 finds no captured case, the test edits
the Homebrew row's **placement** to `NotLinked`, the keg-only shape, and declares
that edit. `PATH` is never permuted, which `primary_decision.rs:196-199` rules out
(SG-14).

**A10 — `--write-digest`, named** (D3, D15, 06; SG-N8).

- **The flag.** `--write-digest <hex>` is required beside `--yes --plan-digest`
  whenever a write step is present. It is named in 06's synopsis.
- **With `--removal-digest`.** It is independent of `--removal-digest`: a plan
  with both a removal and a write needs both.
- **`ADVANCED_REMOVAL`.** A plan at that level keeps no script route, with or
  without writes (`NO_SCRIPT_ROUTE`, SE-114).

**A11 — `effective_uid()` is `None`** (D4; SG-N9). The writer refuses with
`SHIM_DIRECTORY_UNSAFE`, and capture records the uid as unknown. Mutant: `None`
treated as the home's owner.

**A12 — Intents are maybe-written** (D8; SG-N10, SR-N6).

- **A shim.** Restore from an intent acts only when the slot matches the
  intent's after-state. A slot still matching the before-state, or holding no
  link at all, is a no-op.
- **The block.** The file may match the before-digest, which is a no-op, or the
  after-digest, which is reversed. Anything else is reported, not touched.
- **Lifetime.** An intent lives until the history record is durable, as the run
  marker does.
- **Which directory.** Revert and restore use the shim directory history
  recorded, never one recomputed from today's `XDG_DATA_HOME`.
- **Tests:** both halves, for both kinds.

**A13 — Mirrors** (D14; SG-13). If `.claude/rules/adapters.md` is amended,
any generated `.codex` mirror of `.claude/rules/` is regenerated in the same
staged patch.

## Amendments after R-31 (I-1)

R-31 (`docs/research/R-31-macos-shell-startup.md`, 2026-11-22) answers D5 and
D6's questions. Four findings change the plan; the rest confirm it. A18
records R-30's answer to D11.

**A14 — The block's guard tests *first*, not *present*** (D6, D7; R-31). In a
nested login shell `path_helper` rebuilds `PATH` with the system entries ahead of
everything the parent shell had, so the shim directory is present but behind
`/usr/local/bin`, and D6's `case` skips it. The guarded line becomes: if the
directory is already the first entry, nothing; otherwise remove every occurrence
of it and prepend it once. `brew shellenv` also tests *first*, but re-prepends without removing, at the cost of a duplicate (R-31 finding 7); this form adds the removal 05 asks for. It
never duplicates an entry and touches only the entry MacPortsBrew owns, so spec
05's "never duplicate" and "never reorder entries the user placed" hold.
Re-running changes nothing (INV-16). The exact text is fixed in I-4 with a test
that runs the block under `/bin/zsh` and `/bin/bash` with a scratch `HOME`, after
a nested `path_helper`, and asserts the directory is first exactly once. The
block's form is a Tier 3 reading of 05 `:201-209`; SE-115 names it.

**A15 — An unexported `ZDOTDIR`** (D6; R-31). A `ZDOTDIR` assigned inside the
home's `.zshenv` without `export` is invisible to the tool's environment, which
would then write a `.zprofile` in the home that shell never reads. Capture scans
that `.zshenv` as bounded text (the 1 MiB bound of D6, never sourced, never
shown) for the token `ZDOTDIR`. If it appears and the environment carries no
`ZDOTDIR`, the writer refuses with `SHELL_FILE_UNDETERMINED`. A scan cannot
evaluate the shell, so this is a refusal on sight, not a parse; it is declared,
with its test and mutant.

**A16 — Placement, settled** (D6, D7; R-31). zsh: the end of
`${ZDOTDIR:-$HOME}/.zprofile`, after `/etc/zprofile`'s `path_helper` and after any
`brew shellenv` line the file holds. bash: the end of the first existing of
`.bash_profile`, `.bash_login`, `.profile`. `.zshenv` and `.bashrc` are never
chosen. Creating a zsh file shadows nothing (each is read independently), so
`invariant_03_creating_the_shell_file_never_shadows_another_startup_file` has a
bash case that refuses and a zsh case that creates. An interactive non-login
shell (a bash started from a login zsh, say) does not read the block; D7's report
says so.

**A17 — Module builtins are reported, not refused** (D5; R-31). zsh's
`module_builtins` (`mv`, `rm`, `ln`, …) become builtins only when the user's own
configuration loads `zsh/files` or another module, which the tool cannot see.
Refusing them would forbid shims for ordinary tools. D5 refuses `builtins` and
`reserved_words`; a `module_builtins` name is admitted, and D7's report names the
module that would take it over.

**A18 — D11's branch, taken** (R-30, 2026-11-22). Homebrew's `post_install`,
`post_install_steps` and cask `install_steps` run with exactly the user's `PATH`,
and `env :std` builds with it ahead of Homebrew's prefix; MacPorts replaces the
caller's `PATH` with `binpath` before every phase. So the second branch applies
for Homebrew and the first for MacPorts, and DP-058 records it for the owner. The
default meanwhile is the one D11 fixed: `LAYER4_SHIM_OFFER_ADMITTED = false`, a
domain constant named after DP-058, read only by the planner's offer; every shim
offer stays `PRIMARY_NEEDS_SHIM`, and tests reach it through D1's allowance. The
recommendation is DP-058 (d). One correction: D11's "a shimmed `git` reaches
`brew update`" is wrong by default, since `brew update` runs an absolute `git`
(R-30 finding 6); a shimmed `git` is reached by a `post_install` step that runs
`git` by name. F-2's E-9 citation of `formula.rb:1729-1738` is stale (now
`:1785-1787`, R-30); F-2 itself is not edited.

**A19 — from the I-1 reviews** (2026-11-22). The slice that loads
`shell-builtins.toml` (D5) rejects a table missing `source` or a valid
`read_on`, with its own test. The consumer matches a table by `binary` and
`version`, not only by `$SHELL`'s base name: a provider-installed bash 5.x
whose base name is `bash` gets an undetermined list (reported, the shim refused
as undetermined), never 3.2.57's. A15's `SHELL_FILE_UNDETERMINED` gets a stable
`ErrorId` and a catalogue message (en, pt-BR) in I-4, like D6's other refusals.
R-31's iTerm2 and tmux login-shell claims are paraphrase and general knowledge;
I-4 quotes or observes them before its nested-login-shell test relies on them.

## Research, before code (I-1)

- **R-30 — what reaches each provider's phases from the user's `PATH`, and in
  what order.** D11. `provider-researcher`, read-only commands and official
  sources.
- **R-31 — macOS shell startup.** D5, D6. `provider-researcher`, with the shells'
  own documentation and the dates read. It covers:
  - which file zsh and bash read for login and for interactive shells;
  - `ZDOTDIR`;
  - bash's first-of-three rule;
  - Terminal.app and iTerm opening login shells;
  - `path_helper` and where a block must sit to come first after it;
  - what each provider's installer writes there (R-6 E12);
  - each shell's builtins and reserved words, as dated data.

Both are docs only, with checklist entries in `docs/spec/09-research-checklist.md`.

## Slices

| Slice | Branch | What | Depends on |
|---|---|---|---|
| I-0 | `wo16/I-0-plan` | This plan and its reviews; the ledger (K merged, I in progress). Docs only | K merged |
| I-1 | `wo16/I-1-research` | R-30 and R-31, their checklist entries; D11's branch taken and recorded (a DP if a phase inherits `PATH`); R-31's builtin lists into `data/policy/` | I-0 |
| I-stage | `wo16/I-stage-symlink` | D14's patch, `README.md`, `security-reviewer`; merged as staging. The ledger notes the owner item | I-0 |
| I-2 | `wo16/I-2-steps` | Domain: `FileWrite` with its three operations and reverses, the two kinds, `ShimPlanned`, the name class, `check_ordering`'s five rules, `CommandTarget::Tool` removed; `plan.v3`, `migrate.v3`; the new `ErrorId`s and catalogue keys (`en`, `pt-BR`) | I-1 |
| I-3 | `wo16/I-3-capture-offer` | D16's capture and `snapshot.v2`; D1a in capture and `decide`; `--allow-shim`, the offer (D1, D12, D13), `PRIMARY_NEEDS_SHIM` raised with layers 1 and 2, the impact preview, the gate's five-list check; D11's constant | I-2 |
| I-4 | `wo16/I-4-block` | D6's dedicated writer, block find, compute and diff, the backup, the refusals; tests on a scratch home only. No link created | I-3 |
| I-5 | `wo16/I-5-consent` | D3: the per-step questions, `writeDigest`, the home identity, `WriteConsented`, the run loop's hand-over, history's consent route; K's forgery check extended; K-6's test renamed | I-4 |
| I-6 | `wo16/I-6-shim` | D4, D5: the directory walk and creation, the link writer in `crates/adapters/src/shim/link.rs`. **Needs I-stage applied** | I-5, I-stage applied |
| I-7 | `wo16/I-7-verify-history` | D2's rollback, D7, D8: the managed `PATH` check, the intent record (`run-marker.v3`), `history.v5`, recognition; 04 `:753-755` and SE-092's items closed | I-6 |
| I-8 | `wo16/I-8-env-restore-doctor` | D9, D10: `env` and `env.v1`, `env --revert`, restore's reverse writes (`restore.v3`), `doctor`; 06's synopsis (`:55`, "later pass" removed) | I-7 |
| I-9 | `wo16/I-9-close` | A positive tripwire, `the_switch_needs_consented_writes` (every write reaches its writer only with `WriteConsented`, only from a plan step, only in `shim/` or the block writer); SE-115 completed (D15); ledger, `16-report.md`'s I section, CHANGELOG, wiki `Primary-Provider.md`; `docs-auditor` | I-8 |

Each code slice gets `spec-guardian` and `security-reviewer` on its diff and a
mutation proof, as part K's did, recorded in the Reviews table below.

## Tests, per invariant, by name

Every test runs against a temporary home and a temporary state directory, never
the real ones.

- **INV-01:**
  - `invariant_01_no_write_step_precedes_the_verify_destination_guarding_it`
  - `invariant_01_a_write_without_a_verify_resolution_after_it_is_refused`
  - `invariant_01_no_remove_source_precedes_a_shims_verify_resolution`
  - `invariant_01_only_add_remove_and_layer_4_writes_are_ever_dispatched`
- **INV-02:**
  - `invariant_02_every_shim_block_and_directory_write_is_a_plan_step`
  - `invariant_02_no_write_is_handed_over_without_its_own_consent`
  - `invariant_02_changing_any_write_field_changes_the_write_digest`
  - `invariant_02_the_plan_digest_never_equals_the_write_digest`
  - `invariant_02_a_plan_shown_under_another_home_is_not_admitted`
  - `invariant_02_a_declined_write_question_exits_6_before_anything_runs`
  - `invariant_02_nothing_is_created_during_plan_or_dry_run`
- **INV-03:**
  - `invariant_03_a_shell_file_changed_since_the_plan_is_left_as_it_was`
  - `invariant_03_a_failed_block_write_leaves_the_original_and_its_backup`
  - `invariant_03_an_occupied_slot_is_never_replaced`
  - `invariant_03_creating_the_shell_file_never_shadows_another_startup_file`
  - `invariant_03_a_failed_verify_runs_the_planned_reverses`
  - `invariant_03_a_run_killed_after_the_link_is_restorable`
- **INV-06:** `invariant_06_no_write_of_layer_4_leaves_the_home`. No elevation;
  every path is under the home or the state directory, and a `ZDOTDIR` outside
  the home is refused.
- **INV-07:**
  - `invariant_07_the_shim_directory_never_enters_a_provider_invocations_path`,
    unconditionally;
  - more as D11's branch decides.
- **INV-11:** `invariant_11_a_shim_transfers_no_ownership`.
- **INV-12:**
  - `invariant_12_a_shim_moves_only_the_consented_names`
  - `invariant_12_no_shim_is_offered_over_a_non_provider_winner`
  - `invariant_12_a_case_variant_of_another_name_is_refused`
- **INV-15:** `invariant_15_a_replayed_plan_never_reads_or_writes_the_home`.
- **INV-16:**
  - `invariant_16_a_second_run_writes_nothing`
  - `invariant_16_a_shim_winner_pointing_at_the_destination_is_nothing_to_change`
  - `invariant_16_the_block_never_duplicates_its_path_entry`
  - `invariant_16_an_upgrade_does_not_rewrite_the_block`
  - `invariant_16_a_rerun_after_a_recorded_shim_is_recognised`
- **INV-17:**
  - `invariant_17_the_shim_offer_is_direction_neutral`
  - `invariant_17_a_shimmed_primary_can_be_moved_back`
- **§3 item 10 and D5.** One test per name-class rule, one per target check, and:
  - `a_shim_target_outside_the_destinations_prefix_is_refused`
  - `what_a_write_question_shows_of_a_provider_value_is_escaped`
  - `no_byte_outside_the_markers_reaches_the_plan_or_history`

## Mutants each slice must kill

- **I-2.**
  - Each name-class rule dropped.
  - Each of `check_ordering`'s five rules dropped, and each changed to
    first-occurrence.
  - The presence requirement dropped.
  - `is_mutating` false for either kind.
  - A reverse omitted from the plan.
- **I-3.**
  - The shim winner read as `NonProviderEntryWins` again.
  - The shim check after the symlink follow.
  - The offer made without the allowance.
  - A shim planned over `NON_PROVIDER_ENTRY_WINS`.
  - The impact preview without the recorded shims.
  - Names compared by bytes.
  - D11's constant ignored.
  - An absent fact read as `Absent`.
  - The five-list check back to four.
- **I-4.**
  - The digest or `lstat` re-check removed.
  - The `case` guard removed.
  - A second marker pair accepted, or a non-whole-line marker accepted.
  - Content outside the markers re-wrapped.
  - The backup written after the shell file.
  - A symlinked shell file followed.
  - `nlink > 1` accepted.
  - The mode not restored.
  - A shadowing startup file created.
  - Context lines in the diff.
- **I-5.**
  - The witness skipped at hand-over.
  - Consent for step *n* covering step *m*, or another plan's step *n*.
  - `--yes` without `--write-digest` admitted.
  - The home identity dropped from the digest.
- **I-6.**
  - Each prefix check removed.
  - The target-under-prefix, ownership or existence check removed.
  - An `Absent` slot written by rename.
  - An occupied slot replaced.
  - A symlinked `~/.local` followed.
  - The home's owner taken from `$HOME`.
  - The post-write `read_link` check removed.
- **I-7.**
  - `VERIFY_RESOLUTION` read against the run's own `PATH`.
  - The reverses not run on failure.
  - The intent written after the link.
  - A re-run recognised with a different target.
- **I-8.**
  - `--revert` removing an unrecorded or changed slot.
  - A backup restored over later edits.
  - A replaced shim restored without the target checks.
  - A non-empty directory removed.
  - `env` writing anything.
  - `doctor` missing an orphan or a leftover temporary.

## Security notes for the reviewers

- **The first writes into the home outside the state directory.** These are the
  shim directory, its parents where missing, and one shell file. Every one is
  named in the plan, re-checked under the lock, and never followed through a
  symlink.
- **Injection.**
  - No provider metadata becomes a path unchecked: the allowlisted name class,
    the target's prefix, ownership and existence checks, and the formula
    component checked too.
  - No value becomes shell code: the directory is single-quoted and refused if
    it holds `'`, `:`, newline, NUL or a control character.
  - Every displayed name and path is escaped.
- **Time of check to time of use.** The writer re-checks under the lock:
  - the slot's state;
  - the target's existence, prefix and owner;
  - the shell file's digest and `(dev, ino, size, mtime)`;
  - the home's identity.

  An `Absent` slot is published without replace. The remaining windows are
  stated (D15).
- **Secrets.** No shell-file byte outside the markers enters any document. The
  backups are `0600`, durable, kept while restorable, and never shown.
- **No elevation**, no new program, no allowlist change (§3 item 3). `env` is
  read-only. `--revert` and restore are plans behind the same gate.
- **The guard change (D14)** admits one token in one file at a pinned count. Its
  patch is reviewed on its own.

## What is not claimed

- **That layer 4 works on a real machine.** Nothing here writes outside a scratch
  home, and the switch is off. The flip proposal (part L) inherits the first real
  exercise.
- **Global priority.** It is still not offered.
- **Fish, and shells R-31 does not admit.**
- **INV-07 for every inheriting phase**, until D11's branch is taken on R-30's
  evidence.
- **That a shell will run the shim.** D7 is a prediction.
- **ACL, xattr and login-shell checks** (D15).

## Needs the owner (expected)

- **D14's staged patch**, `docs/passes/16-staged/I/01-…`, applied with
  `MPB_ALLOW_GUARD_EDIT=1`. It blocks I-6.
- **The DP from D11**, if R-30 shows an inheriting phase. It blocks offering a
  shim at all (the constant), not building one.
- **A capture** from a machine where MacPorts' `bin` precedes Homebrew's, if I-3
  finds no captured `PRIMARY_NEEDS_SHIM` toward Homebrew (D13).

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
| security-reviewer | `42780c5` | **approve with changes.** H1 creating a shell file can shadow another startup file (bash's first-of-three), `path_helper`; H2 case- and normalisation-insensitive APFS breaks INV-12 and plans colliding names; H3 revert removes a slot that no longer holds the recorded link, restore links an unchecked recorded target, created directories never removed; H4 directory creation not in the plan. M1 `write_atomic` preserves no mode, owner, xattr or hard link, FIFO and flags; M2 the digest covers redacted paths, so another `HOME` admits; M3 the name class admits control characters, ESC, whitespace, a leading dot; the formula component and target unchecked; M4 the diff's context leaks rc secrets, history and errors unredacted; M5 rename into an `Absent` slot replaces a late arrival; M6 the walk skips `HOME` itself, `XDG_DATA_HOME` with `..`, the shell file's parents, ACLs, euid 0; M7 prefixes: the union and both directions; M8 whole-line markers, CRLF, bounded read, `:` and control characters in the directory. L1 token-specific, `soft_link`; L2 dot-name backups, retention; L3 `$SHELL` not the login shell; L4 keep `doctor`'s writable-entry warning, sensitive names; L5 leftover temporaries; L6 bind the witness to the plan; L7 edits inside the markers lost on revert. Info: the tool's own pinned `PATH`; builtins, aliases; fish | All folded: H1 D6 and R-31; H2 D4, D5, D12; H3 D9; H4 D4 and D2; M1 D6's dedicated writer (ACLs and xattrs declared, D15); M2 D3's home identity; M3 D5's allowlist; M4 D2 zero-context, D8 redaction, D6 backups never shown; M5 D5; M6 D4, D6; M7 D4; M8 D6, D4. L1 D14; L2 D6; **L3 declared, not built** (`getpwuid` needs `libc`; the question names the file); L4, L5 D10; L6 D3; L7 D9. Info: D11's unconditional test, D5's builtins, fish refused (D6) |
| spec-guardian | `42780c5` | **approve with changes.** 1 `decide` reads a shim winner as `NON_PROVIDER_ENTRY_WINS`, so INV-16 holds only through history, `RecordedShim` is unreachable, no way back, no repair; capture never assigns `MacportsbrewShim`. 2 planning's facts are not in the snapshot (INV-15); privacy of the shell file. 3 no rollback story, no intent before a write (a crash leaves an unrecorded shim). 4 `check_ordering` must require presence, every occurrence, five lists equal; revert has no step form. 5 the plan digest consenting to shell writes is in tension with SE-114's removal digest; K-6's test name. 6 rename into an `Absent` slot; the name class misses a leading dot, control characters, `:` in the directory; per-check tests. 7 `write_atomic` cited wrongly; the creation-mode seam is test-only; the shell file's parents; `ZDOTDIR` outside the home. 8 the INV-17 test is infeasible on the cited snapshot; option (d) needs dated data. 9 an unconditional INV-07 test; widen R-30. 10 the Tier 3 deviations SE-115 must name. 11 the impact preview without recorded shims; placement and `path_helper`. 12 created directories as writes. 13 the patch token-specific and complete (`.codex`, both test scripts, `rules/adapters.md`). 14 the version line and INV-16. 15 `sbin` targets; `port select` names. 16 inode- and firmlink-aware comparison. 17 tests cannot flip a constant. 18 escaping. 19 the run's home equals the plan's. 20 the windows the lock does not close; backups' retention. **D11 advisory:** (b) not compatible with INV-07's text except where the shim changes nothing; the DP is the owner's alone. Citations accurate except `plan.rs:614` (the constant) and `write_atomic`'s mode | All folded: 1 D1a, D8; 2 D16, D2; 3 D2's rollback, D8's intent record; 4 D2's five rules, the revert form, the five-list check; 5 D3's `writeDigest`, the rename; 6 D5; 7 D6; 8 D13 and an owner capture item; 9 D11, R-30; 10 D15; 11 D12, D7; 12 D4; 13 D14; 14 D6; 15 D5; 16 D4; 17 D1; 18 D3, D5; 19 D3; 20 D15, D6. The advisory taken: the DP states that (b) is an interpretation of a Tier 1 invariant, the owner's alone. Citations corrected |
| security-reviewer (re-review) | `8ffe78b` | **approve with changes.** Every earlier finding resolved or declared (L3, ACLs). New: N1 any entry in the shim directory read as this tool's, so a foreign link is `Replace`d; N2 the home identity's hash and `snapshot.v2` leak the user name, and `XDG_DATA_HOME`, `ZDOTDIR` and the block are unredacted; N3 reverse semantics, order, the way back's ordering; N4 D12 and D5 need `PATH` listings the snapshot lacks; N5 the class refuses every `@` formula; N6 a FIFO at capture, maybe-written intents, the recorded directory | Folded: N1 A1; N2 A2; N3 A3, A5; N4 A6; N5 A8; N6 A6, A12 |
| spec-guardian (re-review) | `8ffe78b` | **approve with changes (minor).** All 20 addressed; 2, 3, 4 and 8 with residuals. New: N1 rule 1 against restore's `Replace` and the rollback's consent; N2 the home identity is a privacy leak; N3 `LAYER4_FACTS_UNKNOWN` would refuse every replay; N4 the way-back `Remove` unordered in `migrate`; N5 `SHIM_NAME_SHADOWS_ANOTHER` matches the shimmed name itself; N6 D13's fallback misdescribed; N7 rollback changes 04's failure semantics; N8 `--write-digest` unnamed; N9 `effective_uid()` can be `None`; N10 restore from an intent, both halves | Folded: N1 A3, A4; N2 A2; N3 A6; N4 A5; N5 A7; N6 A9; N7 A3 (and D15); N8 A10; N9 A11; N10 A12. Not re-reviewed a third time: each amendment narrows or names, none adds a write path; each slice's own review reads D-sections and amendments together |
