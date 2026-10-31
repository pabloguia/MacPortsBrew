<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part E — slice 7: the configuration token names its program, a code in the consent phrase, the executed plan tied to the one shown (both routes), consent wired; and the dependency ADR

Plan written 2026-10-30, before any code. `spec-guardian` and
`security-reviewer` review it before any code and again after each commit; their
review is the approval (work order 16 §4). **`THIS_BUILD_DISPATCHES_MUTATIONS`
stays `false` throughout** (§3 item 1). No mutating `brew` or `port` command
runs anywhere, and no elevation program runs at all; every test runs on in-code
plans, fakes, or `fixtures/recordings/*.json` (§3 items 2 and 9).

## The ruling

Work order 16, part E: run `prompts/08-slice-7-plan-tie-and-consent.md` with
§1 item 8's overrides and part E's three changes.

- **One ADR, two purposes** (§5 E.1): `sha2` under ADR-0004, and the mechanism
  part G's lock probe uses. It supersedes ADR-0006 for those two purposes only.
- **Both routes of DP-049** (ruled (a) and (b), 2026-10-27, SE-109):
  `migrate --yes --plan-digest <d>` for scripts, and an interactive
  confirmation of the same in-memory plan for a person at a terminal.
- **Determinism (INV-15):** the digest is over a canonical serialisation with
  no hash-map iteration order, and the same snapshot gives the same digest
  across runs.
- **The slice's four parts, in its order:** D (SR 9), C (SR 10), A (the tie),
  B (consent wired). §10 explains why A is split in two and its second half
  lands last.

What this part does *not* rule on: DP-046 (b)'s elevated spawn (part J) and
DP-037's stronger confirmation levels (part K). Both build on what lands here.

## Where it lives today

Every line below was re-read on `8e7e01b`. Where the design notes
(`docs/passes/08-pre-flip-design-notes.md`, "Slice 7") cite a line that has
moved, the current one is given and the note is the stale text.

- **The configuration token holds only the provider.** `ConfigurationRechecked
  { provider }` (`crates/application/src/configuration_recheck.rs:66–69`);
  `recheck_provider_configuration(provider, snapshot, fresh)` (:86–108). Its
  three shell call sites each re-read for a program but forget it: the
  destination re-check before a removal, re-read for the verify step's first
  program (`crates/cli/src/executor/mod.rs:609–626`); the per-command re-check
  before dispatch, re-read for `command.program` (:651–664); the certifying
  read's re-check (`certify`, :801–808). `mutation_dispatch` compares only the
  provider (:844–846; the design notes say ≈743). `certify_destination` and
  `destination_still_verified` likewise compare only the provider
  (`crates/application/src/executor.rs:939–944`, :971–973).
- **The consent phrase is fixed and buffered input can answer it.**
  `confirmation_phrase` is `allow step N` (`crates/application/src/consent.rs:102–104`);
  `confirm(request, typed)` (:111–121); `cli::consent::ask` reads with
  `take(MAX_LINE_BYTES).read_line` on the caller's `BufRead` (`crates/cli/src/consent.rs:123`),
  so a line typed before the question is read as its answer (SR 10).
- **Consent is built and has no product caller.** `cli::consent::ask`
  (`crates/cli/src/consent.rs:65–139`) and `elevation_requests`
  (`crates/application/src/consent.rs:130–181`); the tripwire
  `the_switch_needs_consent_wired` (`crates/cli/tests/history_preconditions.rs:1166`)
  counts callers of `consent::ask(`.
- **Elevated dispatch is refused twice.** The gate: `ElevationRequired` /
  `ElevationUnknown` from the plan's own sets (`crates/application/src/execution_gate.rs:809–816`)
  and the re-derived pin under an allowance (:879); the design notes'
  ≈727–734, ≈797 are stale. The guard: `mutating_action` halts
  `ElevationNotNone` on a re-derived or declared elevation that is not `NONE`
  (`crates/application/src/executor.rs:770–775`), and a read step likewise
  (:707–714); the notes' ≈613–619, 667–676 are stale. So **no cleared run has
  an elevation request in this build**, and the prompt is unreachable in a
  shipped build until part J (DP-046 (b)).
- **The run loop takes no shown plan.** `run_with_mutations`
  (`crates/cli/src/executor/mod.rs:487–760` approximately): allowance, live
  check (:499), `history_store::probe` (:509), lock (:511), recapture and
  `plan_still_holds` (:513–528, which is plan **equality**,
  `crates/application/src/executor.rs:1162–1173`), marker read (:530–536),
  loop, marker written before the first mutating command (:580–593).
  `MutatingRun` (:433–454) has no `shown` and no `consent`. `Recapture` is a
  boxed `FnOnce` (:420).
- **`migrate` executes before it renders.** In `migrate_with`
  (`crates/cli/src/app/migrate.rs:361–559`) the plan is built (:375–407, the
  history-consulted second plan for `--retain-source` at :385–405; the notes'
  ≈380–401), then executed (:494–520; the notes' ≈483–527), then written
  (:521–552). `execute_mutating` (:211–282) builds the `MutatingRun`
  (:265–279). The module comment admits the gap (:36–38).
- **No `ErrorId` for exit 6.** `ErrorId` (`crates/domain/src/error.rs:17–125`)
  and `exit_code` (:130–152) have 0–5, 7, 8; `06-cli-and-ux.md:83` reserves
  6 for "user aborted".
- **The tripwires this part closes.**
  `the_switch_needs_the_executed_plan_tied_to_the_one_shown`
  (`crates/cli/tests/history_preconditions.rs:1245`, unconditional);
  `the_switch_needs_consent_wired` (:1166); and the obligation-8 test in
  `crates/cli/tests/migrate.rs:1023`
  (`invariant_02_migrate_executes_a_plan_built_in_process_only_while_nothing_destructive_is_dispatched`,
  which fails on any `migration_start(` in `migrate.rs` whatever the tie; the
  notes' ≈957 is stale). `wiring_violations` is
  `crates/cli/tests/history_preconditions.rs:229`.
- **The witness idiom.** `not_clone_nor_default` is a private module of
  `execution_gate.rs` (:427–446). It is **repeated** in the new file, not
  imported, so `execution_gate.rs` does not change at all.
- **The dependency state.** `Cargo.toml:35` `unsafe_code = "forbid"`;
  `[workspace.dependencies]` has no hash crate and `serde_json` carries
  `preserve_order` (bears on §3); no `libc` in `Cargo.lock`. P/01
  (`docs/passes/16-staged/P/01-licences-libc.patch`, `"libc"` into
  `build_scripts_allowed`) is **not applied**: `git apply --reverse --check`
  fails on this branch; P/02–P/04 are applied.

**Where the design notes and the code disagree, this plan follows the code and
the rulings.** Besides the stale lines above: (i) the notes have
`ShownPlan` hold a digest; this plan has it hold the plan itself (§3), which
makes the interactive route independent of the dependency; (ii) the notes put
the token program check only in `mutation_dispatch`; the two destination sites
take the same token and get the same check (§1); (iii) R-22 F-2 recommends
`libc` for constants only, with no `unsafe`; that covers G's open and `fstat`
but not the held-lock test R-17 requires, which needs `fcntl(F_GETLK)` (§0).

## Design

### 0. The ADR: ADR-0007, `sha2` and the lock probe's mechanism

`docs/adr/0007-sha2-and-the-lock-probe-mechanism.md`, written with `/adr`,
**Status: proposed** in E-0, **accepted** in E-4 (the change that adds the
dependency), when ADR-0006's status line becomes "accepted; superseded **for
the `sha2` dependency and the lock probe only** by ADR-0007". Every other part of
ADR-0006 stands: the timeout kills one process, and nothing in
`crates/adapters/src/process/` uses `libc` (§0.3).

**0.1 `sha2`.** `sha2 = { version = "0.11.1", default-features = false }` in
`[workspace.dependencies]`, used by `crates/application` only
(`check-layering.sh` restricts workspace edges, not external crates;
`crates/domain` stays `serde`-only). `Sha256::digest` needs no feature
(R-22 F-1). The transitive set, from R-22 F-1 (2026-10-28) and re-measured with
`cargo tree -e normal,build --target <t>` in E-4's commit:

| Crate | Version | Licence | `build.rs` | Targets |
|---|---|---|---|---|
| sha2 | 0.11.1 | MIT OR Apache-2.0 | no | both |
| digest | 0.11.3 | MIT OR Apache-2.0 | no | both |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 | no | both |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 | no | both |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 | no | both |
| typenum | 1.20.1 | MIT OR Apache-2.0 | no | both |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | no | both |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | no | both |
| libc | 0.2.191 | MIT OR Apache-2.0 | **yes** | built for `aarch64-apple-darwin` only (through `cpufeatures`); present in the unfiltered metadata for every target |

**Every new crate with a `build.rs`: `libc` alone.** Its script (0.2.191, read
for R-22 F-4) runs `$RUSTC --version`, emits `rustc-cfg`/`rustc-check-cfg`,
spawns another program only on FreeBSD and Emscripten targets, and writes no
file — ADR-0004's probe idiom. ADR-0007 extends ADR-0004's table with that row
(ADR-0004 Decision 3). `check-licences.sh` reads `cargo metadata` unfiltered,
so the entry is needed on every host: that is P/01, staged and not applied.
Licences are all within the list; `deny.toml` bans nothing that matters here
(`multiple-versions = "warn"` only). The deployment floor is checked by
`check-deployment-target.sh` on E-4's release build (`cpufeatures` on Apple
Silicon calls `sysctlbyname` through `libc`).

**Why a cryptographic hash and this crate.** DP-049 (c) is rejected: a
hand-written hash is cryptography in the safety path, a non-cryptographic one
admits collisions an attacker who controls an inventory could craft. `sha2` is
RustCrypto's, the reference implementation in the ecosystem, and its
`default-features = false` set is nine small crates with one build script.

**0.2 The probe's mechanism: `libc`, one reviewed `unsafe` call in one
module.** The choice the work order offers is `libc` with one `unsafe` call, or
a maintained safe wrapper. R-17's rule 1 decides what the probe must call: the
held-lock test is `fcntl(F_GETLK)` on an `O_RDONLY` descriptor, never
`flock(LOCK_SH|LOCK_NB)`, because a shared hold refuses a Homebrew
`EX|NB` taker, which aborts the user's `brew install`
(`docs/research/R-17-provider-mutation-locks.md`, findings 4 and 5, E4
observation B). `std` exposes `File::try_lock_shared` (the `flock` shape R-17
rejects) and no `F_GETLK`. So R-22 F-2's conclusion — `libc` for three
constants, no `unsafe` — is correct for the open (`custom_flags`) and the
regular-file check (`File::metadata` is `fstat` on the handle) and **does not
cover the held test**. One `libc` function call is unavoidable unless a wrapper
makes it.

The options, measured:

- **`libc`, one `unsafe` call.** `libc` is already in the graph through `sha2`
  (and admitted by P/01's one entry). G adds it as a direct dependency of
  `crates/adapters` (on `x86_64` it is then built where `sha2` alone does not
  build it). Our `unsafe` is one `fcntl(fd, F_GETLK, &mut flock)` on a
  zero-initialised stack `libc::flock`, the descriptor borrowed from a live
  `File` for the call's duration, the return checked. No new crate, no new
  build script, no new staged patch.
- **`rustix` 1.1.6, `process::fcntl_getlk` (safe).** Verified in the local
  registry 2026-10-30: `rustix-1.1.6/src/process/fcntl_getlk.rs:21`, compiled
  for Apple targets (`src/process/mod.rs` cfg list excludes only other OSes);
  `default-features = false, features = ["process", "std"]` pulls `rustix`,
  `errno`, `bitflags`, `libc` on both targets (`cargo tree --offline` in a
  scratch crate, 2026-10-30). Adds three crates, one of them with a build
  script that compiles probe snippets with `$RUSTC` (R-22 F-4), so a second
  `build_scripts_allowed` entry and a second owner patch; the `unsafe` it
  holds is far larger than ours and outside our review.
- **`nix` 0.31.3, `fcntl(fd, FcntlArg::F_GETLK(&mut libc::flock))` (safe).**
  Verified in the local registry: `nix-0.31.3/src/fcntl.rs:752, 862, 881`. Adds
  `nix` (build script), `cfg_aliases`, `bitflags`, plus `libc`; same staging
  cost as `rustix`.

*(The lint and scan bullets below, and "no new staged patch", are superseded by
R10 and ADR-0007's Decision: four crate roots keep `forbid`, and the scan is
code-aware over every `.rs` under `crates/`.)*

**Chosen: `libc`, one `unsafe` call.** It adds nothing to the graph beyond what
`sha2` already brings and P/01 already admits; the whole of the new `unsafe`
surface is a few lines this project reviews line by line, instead of a crate's
worth it trusts; and it needs no further owner action. The cost is the lint:
`#[allow(unsafe_code)]` does not compile under `forbid`, so **G lowers
`unsafe_code` from `forbid` to `deny`** in `Cargo.toml`, in the same change as
the probe, and keeps the single site structural:

- the call lives in one module G names (proposed:
  `crates/adapters/src/lock_probe/getlk.rs`), carrying the module's only
  `#[allow(unsafe_code)]`, a `// SAFETY:` comment, and
  `#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]`;
- a word-scan test (G) fails if `unsafe` or `allow(unsafe_code)` appears in any
  other file under `crates/*/src`, and if `libc::` appears outside
  `crates/adapters/src/lock_probe/` — which keeps ADR-0006's group-kill
  decision intact: `process/` still never signals a group;
- `security-reviewer` reads the call line by line (slice 7 §3's rule for
  "`unsafe`-adjacent text").

**What E itself lands of this.** The ADR and its SE entry (§9). The `sha2`
entry in `Cargo.toml` (E-4). **Not** the `libc` entry, the lint change or the
probe: those are G's. E-4 adds a test, `no_workspace_crate_depends_on_libc_directly`
(reads each `crates/*/Cargo.toml`), which G edits on purpose to admit
`crates/adapters` alone; until then `libc` can only be transitive. The
`check-licences.sh` header comment that says `unsafe_code = "forbid"`
(`scripts/check-licences.sh:10`) becomes stale in G; G stages its one-line fix
(§3 item 5).

**Revisit when** a probe needs more than one `libc` function, or `std`
stabilises an `F_GETLK` wrapper (then the `unsafe` is removed and `forbid`
restored), or `sha2`'s major version changes its graph.

### 1. Part D — SR 9: the configuration token carries its program

- `ConfigurationRechecked { provider, program: String }`, with
  `program(&self) -> &str`. Still neither `Clone` nor `Copy`.
- `recheck_provider_configuration(provider, program, snapshot, fresh)`: the
  `program` is the one the caller re-read for (the `Reread` seam's second
  argument, which `registry::reread_configuration` already refuses unless the
  re-detected prefix derives it). The function stores it; it compares nothing
  new.
- **Three consumers compare it, each against the commands it is spent on:**
  - `mutation_dispatch` (`crates/cli/src/executor/mod.rs:837`): after the
    provider check, `checked.program() != command.program` →
    `MutationResult::PreconditionFailed(Precondition::WrongProgramToken)`, and
    nothing is handed to the site;
  - `certify_destination` (`crates/application/src/executor.rs:924`): every
    command of the `VERIFY_DESTINATION` step must name the token's program,
    else `Halted { PreconditionFailed { command: i, why: WrongProgramToken } }`;
  - `destination_still_verified` (:964): the same over the verify step's
    commands, `Err(Precondition::WrongProgramToken)`.
- `Precondition::WrongProgramToken` (`WRONG_PROGRAM_TOKEN`), the next variant
  after `DestinationNotReverified`; its key
  `migrate.precondition.wrong_program_token` in both locales, beside
  `wrong_provider_token` (`locales/en.toml:417`, `locales/pt-BR.toml:294`);
  `migrate.v1.md`'s open set of `why` values (:136) gains it.
- `MutationSite`'s doc (`crates/cli/src/executor/mod.rs:320–341`) gains: the
  site compares `checked.program()` with the program it derived itself, and
  refuses on a difference. `NotInThisBuild` refuses everything anyway.

The shell changes are argument plumbing: the three calls already have the
program in hand (:609, :651, :801).

### 2. Part C — SR 10: a fresh code in the consent phrase

- **The phrase:** `allow step N <code>`, `<code>` six lowercase hexadecimal
  characters, ASCII, never translated. One code **per request**.
- **The code type:** `application::consent::ConsentCode`, private field, built
  only from three bytes (`ConsentCode::from_bytes([u8; 3])`), rendered as hex.
  It cannot be empty or malformed, which kills "a failing nonce falls back to
  an empty code" by construction; `confirmation_phrase(request, &code)` and
  `confirm(request, &code, typed)` take it. `confirm` still removes only one
  trailing `\n` or `\r\n` and compares exactly.
- **Where the bytes come from:** `/dev/urandom`, read with `std::fs` (three
  bytes, `read_exact`), in a new `crates/adapters/src/entropy.rs`
  (`fresh_bytes::<3>() -> Option<[u8; 3]>`). The adapters are the layer that
  touches the machine; this is a file read, not a spawn, so no spawn site is
  added. Any error is `None`. Rejected: `RandomState`, which the run id and
  temp names use (`persistence/history_store.rs:52–56`) — its seeding is a
  `std` implementation detail, not a documented randomness source; and
  `tcflush` (still races a paste after the flush, and needs `libc`).
- **The seam:** `ConsentStreams` gains `code: &'a mut dyn FnMut() -> Option<[u8; 3]>`.
  `ask` calls it once per request **before writing that request's question**;
  *(superseded by R5: `None` → `CodeUnavailable`)* `None` → `Refused(Declined)` with nothing further read and the
  `consent.code_unavailable` line written. The product closure (part B) is
  `adapters::entropy::fresh_bytes`.
- **Why it closes SR 10:** a line typed before the question cannot contain a
  code nobody had seen; it is read, does not match, and the answer is
  `Declined` — the run halts (B), it does not keep reading for a better line.
- New keys, en and pt-BR: `consent.code_unavailable`; `consent.type_to_allow`
  keeps its `{phrase}` parameter, whose value now carries the code. The code is
  shown through `escape_hostile` like every other value (it is hex, but the
  rule is uniform).

### 3. Part A — the executed plan tied to the one shown

New `crates/application/src/plan_tie.rs`.

- **`ShownPlan`** — a witness: a private `plan: MigrationPlan` field, neither
  `Clone` nor `Default` (the `not_clone_nor_default` idiom repeated in this
  file, `const _` checks as in `execution_gate.rs:440–445`), `#[must_use]`.
  `ShownPlan::covers(&self, plan: &MigrationPlan) -> bool` is **value equality
  with the plan that was shown**. Holding the plan rather than its digest
  means the check inside the process needs no hash at all; the digest exists
  only to carry "what was shown" across a process boundary.
- **Two minting functions, both in this file, and no other constructor:**
  - `confirm_shown(run: &PlannedRun, code: &ConsentCode, typed: &str) -> Option<ShownPlan>`
    — the interactive route (DP-049 (b)). The phrase is `run plan <code>`,
    exact, with a fresh code (§2's type and source); on a match it holds
    `run.plan().clone()`.
  - `tie(run: &PlannedRun, echoed: &PlanDigest) -> Result<ShownPlan, PlanNotShown>`
    — the script route (DP-049 (a)), E-4: `PlanDigest::of(run.plan()) == *echoed`.
- **`PlanDigest`** (E-4): `[u8; 32]`, written `sha256:<64 lowercase hex>`,
  parsed strictly (`PlanDigest::parse`: exactly that prefix and 64 characters
  of `[0-9a-f]`; uppercase, whitespace, a shorter or longer value are malformed).
  Compared as bytes; it is not a secret, so no constant-time compare is
  claimed or needed.
- **What is digested** (E-4): `SHA-256( "macportsbrew-plan-digest/1 " ‖
  <tool version> ‖ "\n" ‖ serde_json::to_vec(&plan) )`, over the **in-memory**
  `MigrationPlan`, never the redacted document (redaction is lossy: two plans
  could share a document). The prefix is domain separation plus the build's
  version, compile-time (`env!("CARGO_PKG_VERSION")`, no environment read): a
  digest is valid only for the build that printed it, and a mismatch across
  versions fails closed. Security question for the reviewer: the digest is of
  an unredacted plan, so a published digest lets someone confirm a guessed home
  path; accepted as negligible next to the alternative.

**Determinism (INV-15).** Canonical here means this build's serialisation of
the struct, and it is canonical because nothing in it depends on iteration
order or the clock:

- `serde_json::to_vec` on the struct, **never through `serde_json::Value`**:
  struct fields serialise in declaration order; `PlanCommand::environment` is a
  `BTreeMap` (`crates/domain/src/plan.rs:274`), sorted; every collection in
  `MigrationPlan` (:482–) is a `Vec` whose order is the planner's. The
  workspace's `preserve_order` feature (insertion-ordered maps) does not reach a
  struct serialised directly, and is one more reason not to go through `Value`.
- `crates/domain/src/` holds no `HashMap` or `HashSet` and the plan no float
  (checked 2026-10-30 by grep; pinned by a test, Tests).
- The plan carries no timestamp: `run_stamp` and `captured_at` are not in it.
  `dry_run_only` follows the build constant, which is "same build" by
  definition.
- **Print the digest from `migrate`, never from `plan`:** with
  `--retain-source`, `migrate` re-plans against history (`migrate.rs:385–405`)
  and `plan` does not, so only `migrate`'s plan is the one `--yes` executes.

**How `migrate` uses it** (`crates/cli/src/app/migrate.rs`; *where the
interactive plan is minted is superseded by R1, the stdout and ids by R2*):

- `Render` mode (E-4): the document gains a root `planDigest` (additive;
  `migrate.v1.md`), and the human output a line naming it.
- `--plan-digest <d>` (E-4) is a `migrate` flag. Malformed is `USAGE` (exit 2)
  in every mode, before anything is printed. Without `--yes` it is a usage error
  ("only with `--yes`"); with `--yes --dry-run` it is parsed and otherwise
  unused (`--dry-run` wins, as today).
- `Execute` mode with `--plan-digest`: `tie` right after the plan is built
  (after the `--retain-source` re-plan, :405), **whatever the allowance**, so
  the tie is testable in this build. A mismatch is `SAFETY_BLOCKED` (exit 4),
  "the plan changed since you saw it; nothing was changed", with stdout empty
  (the file's rule: a refusal leaves stdout empty, :335–336) — distinguishable
  from a matching digest, which goes on to the gate and exits 4 **with** the
  document.
- `Execute` mode without `--plan-digest`, **only when an allowance exists**
  (never in this build): if standard input and standard error are both
  terminals, the interactive route — the plan rendered with `human::plan` on
  standard error (stdout keeps the document), the digest shown once it exists
  (E-4), then the `run plan <code>` question through a new
  `cli::consent::confirm_plan(&PlannedRun, ConsentStreams, &Messages) ->
  Result<ShownPlan, ConsentRefusal>`; `Declined` → `USER_ABORTED` (exit 6),
  nothing probed or locked. If either is not a terminal: `USAGE` (exit 2),
  "`--yes` needs a terminal to confirm the plan, or `--plan-digest`" (in E-2
  without the second clause). No allowance (this build): the read-only loop
  exactly as today; nobody is asked anything.
- `execute_mutating` takes the `ShownPlan` and puts it in
  `MutatingRun { shown, .. }`.

**In the run loop.** `run_with_mutations` checks
`io.shown.covers(plan)` **after the allowance and live checks and before
`history_store::probe(`**: a refused tie takes no lock, writes nothing, and
returns `SAFETY_BLOCKED`. Under the lock `plan_still_holds` is plan equality
(`executor.rs:1168`), so the plan that runs equals the plan shown, not only the
plan validated. A future `restore --run` inherits the field.

### 4. Part B — consent wired

- **The decision is the application's** (SE-092): new
  `application::consent::consent_for(run: &PlannedRun, ask: impl FnOnce(&[ElevationRequest]) -> ConsentDecision)
  -> Result<ConsentTokens, ConsentHalt>`:
  `elevation_requests(plan, run.before())`; `Err(PlanDisagrees)` →
  `ConsentHalt::PlanDisagrees` without asking; no requests → empty tokens
  without asking; otherwise `ask`, and `Given` must hold exactly one token per
  request, in order (anything else is `ConsentHalt::Declined`); `Refused(Declined)`
  → `Declined`; `Refused(NotATerminal)` → `Unseen`. `ConsentHalt::error()` maps
  `Declined` → `ErrorId::UserAborted` (exit 6), `Unseen` and `PlanDisagrees` →
  `SafetyBlocked` (exit 4). `ConsentTokens` holds the `ConsentGiven`s keyed by
  step; `take(step) -> Option<ConsentGiven>`; not `Clone`.
- **The seam:** `MutatingRun.consent: &'a dyn Fn(&[ElevationRequest]) -> ConsentDecision`.
  The product closure, in `migrate.rs`, calls `consent::ask` with locked
  standard input, standard error, `std::io::IsTerminal` for both, the
  context's messages and home redaction, and `adapters::entropy::fresh_bytes`
  as the code source.
- **The order in `run_with_mutations`:** shown tie, probe, lock, recapture and
  `plan_still_holds` (`Replanned`), marker read (`MarkerRead`), then
  `consent_for(planned, io.consent)?` (`Boundary::ConsentAsked`, observed only
  when a question was put), then — **only when a question was put** — the
  recapture and `plan_still_holds` again (`Boundary::ReplannedAfterConsent`),
  then the loop. On any refusal: no snapshot, no marker, no dispatch, no record;
  the run returns the error before anything was written, like the
  interrupted-run refusal (:530–535).
- **Re-run the recapture after a slow answer? Yes, when a question was put.**
  A person may leave the prompt for minutes; the lock excludes another
  MacPortsBrew run, not the user in another terminal, and a dependent that
  appears meanwhile (INV-04) is exactly what `plan_still_holds` catches. The
  cost is one capture, only on the elevated path. `Recapture` becomes
  `&'a dyn Fn() -> Result<(Snapshot, MigrationPlan), DomainError>` (the
  product closure already captures by reference). The re-run's result replaces
  `now` (the snapshot `TakeSnapshot` writes). The per-command lock check,
  configuration re-check and removal re-read stay at dispatch, unchanged.
  Unreachable in this build; part J makes it reachable and adds the end-to-end
  test.
- **Token plumbing only.** `mutation_dispatch` gains `consent:
  Option<ConsentGiven>` and hands it to `MutationSite::perform(provider,
  command, checked, consent, lock)`; the shell takes it with
  `tokens.take(step.index)` for the step's first command. **The guard keeps
  halting every elevated command with `ElevationNotNone`**
  (`executor.rs:770–775`): `next_action_with` does **not** gain a consents
  parameter, so no token can reach the guard's decision in this slice.
  `execution_gate.rs` is not touched; its elevation refusals and
  `every_plan_this_planner_builds_is_refused_for_execution_in_this_build`
  (`crates/application/tests/execution_gate.rs:877`) stay as they are.
- **Declared gap:** the prompt is unreachable in a shipped build until
  DP-046 (b) (part J): the gate refuses every elevated step, so a cleared run
  has no request. The report says so.

### 5. `ErrorId::UserAborted`

`USER_ABORTED`, exit 6, `error.user_aborted` in both locales;
`exit_codes_follow_the_contract` (`crates/domain/src/error.rs:250`) gains the
row; every schema listing exit codes (`migrate.v1.md` "Exit codes", :179, and
`docs/schemas/README.md`'s table, R12)
gains 6. First used by the interactive route (E-2), reused by B (E-3).

### 6. Tripwires, as positive structural checks

- **`the_switch_needs_the_executed_plan_tied_to_the_one_shown`** (E-2) becomes
  positive, held whatever the switch: in the file that takes the lock,
  `io.shown.covers(` precedes `history_store::probe(`; `migrate.rs`'s
  `MutatingRun {` literal names `shown:`; the token `ShownPlan {` appears
  under `crates/*/src` only in `application/src/plan_tie.rs`; and `migrate.rs`
  calls `confirm_plan(` (E-2) and `tie(` (E-4). Until E-4 it keeps one
  failing-on-flip assertion naming DP-049 (a), because the ruling requires both
  routes in v0.2.0; E-4 removes it.
- **The `migrate.rs` obligation-8 test** (`crates/cli/tests/migrate.rs:1023`)
  is rewritten (E-2) as
  `invariant_02_migrate_runs_only_a_plan_tied_to_the_one_shown`: the
  `MutatingRun` built in `migrate.rs` takes `shown` from a `plan_tie` minting
  function's result, and `execute_mutating` cannot be reached without one (its
  signature takes `ShownPlan` by value).
- **`the_switch_needs_consent_wired`** (E-3) becomes positive:
  `wiring_violations`' order gains the recapture call, `consent_for(` and
  `run_marker::write(` — `consent_for(` after the first recapture and
  `plan_still_holds(`, before `run_marker::write(`, and its result handled with
  `?` (a `let _ =` or a bare statement is a violation); the second recapture
  after it; and `migrate.rs`'s product closure calls `consent::ask(`. The
  caller count it has today stays as a sub-check.
- `the_switch_needs_manual_review_for_an_unavailable_artifact`,
  `the_switch_needs_the_destination_contract_evaluable`,
  `the_switch_needs_removal_opt_in`,
  `mutations_cannot_dispatch_while_a_live_run_still_says_not_checked` and
  `this_build_does_not_dispatch_mutations` are not touched.

### 7. Boundaries (each shown by a command or a test in the report)

- `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false` (`grep`).
- `git diff fd443ef..HEAD -- crates/adapters/src/process/mod.rs data/policy/provider-commands.toml`
  holds only the owner's earlier hunks; this part adds none.
- `git diff main -- crates/application/src/execution_gate.rs` is empty for
  every E pull request; the gate's tests pass unchanged.
- No new spawn site (the textual `std::process::Command` check), no new
  permitted program, no elevation program named outside a fixture file.
- `migrate.v1` changes are additive: `planDigest` at the root,
  `WRONG_PROGRAM_TOKEN` in an open set, exit 6; checked in a golden under
  `fixtures/golden/` and in `documented_samples.rs`; `schemaVersion` stays 1.
- No guarded file is edited. P/01 is the owner's (§10); G's
  `check-licences.sh` comment fix is G's to stage.
- `docs/spec/01-invariants.md` is not edited.

### 8. Docs, in the same changes

- `docs/schemas/plan.v1.md`: obligation 8 (:327–336) "built: both routes" (E-2
  for the interactive, E-4 for the digest); obligation 9 (:337–353) "wired"
  (E-3), with the gap named; obligation 10 (:355–383) the token's program and
  the `MutationSite` comparison (E-1).
- `docs/schemas/migrate.v1.md`: `planDigest`, `WRONG_PROGRAM_TOKEN`, exit 6
  and `USER_ABORTED`, the two execute routes; samples regenerated from the
  program's output.
- `docs/spec/04-migration-and-safety.md` "Privilege" (:393–): "built and not
  wired" becomes "wired; unreachable in a shipped build until part J"; the
  coded phrase.
- `docs/spec/06-cli-and-ux.md`: the `migrate` line (:32) gains `--plan-digest`
  (a `migrate` flag, not a global one: R4); exit 6 (:83) gets its cases (a declined plan or
  elevation question).
- `CHANGELOG.md` (Unreleased), `wiki/CLI-Reference.md`,
  `wiki/Dependencies-and-Safety.md` (`sha2`, and that `libc` is transitive),
  `locales/en.toml` and `locales/pt-BR.toml`, the module comments of
  `migrate.rs` (:36–38), `cli/src/consent.rs` (:5–8) and
  `application/src/consent.rs` (:5–9).
- `docs/adr/0007-*.md` (E-0, accepted in E-4); ADR-0006's status (E-4).
- `docs/passes/08-pre-flip-design-notes.md`: the slice 7 row of the tripwire
  table marked done, and the stale lines corrected where they are cited.
- `docs/passes/16-progress.md`: the E row and Needs the owner.

### 9. The record

- **SE-111** (re-checked at commit): slice 7 —
  the token's program (SR 9), the coded phrase (SR 10), the tie (both routes;
  `ShownPlan` holds the plan, the digest crosses processes), consent wired with
  the recapture re-run, `USER_ABORTED`; cross-references SE-092 (the shell
  sequences, the application decides) and SE-016 (the check-then-use window;
  consent per operation). Lands with E-1 and is completed by each later PR
  (SE-032: one topic, one entry).
- **SE-110**: ADR-0007 — `sha2` admitted, `libc` transitive through it, G's
  probe uses `libc` with one `unsafe` call, and **`unsafe_code` is lowered from
  `forbid` to `deny` by G** with the single-site scan; R-22 F-2's "no `unsafe`"
  recommendation corrected (it covers the open and `fstat`, not `F_GETLK`).
  Written in E-0 as proposed; final in E-4; G amends it when the lint changes.

### 10. Phasing as pull requests

P/01 is not applied, so a commit adding `sha2` to `Cargo.toml` fails
`check-licences.sh` (`libc`'s build script is unadmitted), and the commit gate
refuses it. Everything else is independent of the dependency, so the slice's
order D, C, A, B becomes **D, C, A-interactive, B, A-digest**: the interactive
route needs no hash (§3), and only the digest route waits. The reviewers still
see the consent wiring (B) in the executor alone.

| Step | Branch / PR | Contents | Gate |
|---|---|---|---|
| E-0 | `wo16/E-plan`, docs only | this plan; `docs/adr/0007-sha2-and-the-lock-probe-mechanism.md` as **Proposed**; SE-110 as proposed; the ledger | both reviewers approve the plan |
| E-1 | `wo16/E-1-tokens` | commit 1, part D (§1); commit 2, part C (§2, with `adapters::entropy`); SE-111 started; obligation 10, locales | reviewers per commit |
| E-2 | `wo16/E-2-shown` | `plan_tie.rs` with `ShownPlan`, `confirm_shown`, `covers`; `cli::consent::confirm_plan`; `MutatingRun.shown` and the check before the probe; `UserAborted`; the tripwire and obligation-8 test rewritten (§6); obligation 8 (interactive) | — |
| E-3 | `wo16/E-3-consent` | part B (§4): `consent_for`, the seam, the order, the second recapture, the token plumbing, the guard pin; `the_switch_needs_consent_wired` positive; obligation 9; 04 "Privilege" | — |
| E-4 | `wo16/E-4-digest` | **waits on the owner applying P/01.** `sha2` in `Cargo.toml`/`Cargo.lock`; `PlanDigest`, `tie`, `--plan-digest`, `planDigest`; the digest on the interactive prompt; the DP-049 (a) assertion removed; ADR-0007 accepted, ADR-0006's status; SE-111 and SE-110 final; `no_workspace_crate_depends_on_libc_directly`; 06, migrate.v1, CHANGELOG, wiki | `git apply --reverse --check docs/passes/16-staged/P/01-licences-libc.patch` succeeds on `origin/main` before the branch is cut |

- **What waits, exactly:** E-4 only. E-1 to E-3 merge on green checks as they
  come. The ledger sets E to `waiting on owner` after E-3, "resume as
  `wo16/E-4-digest` from §10's E-4 row".
- **The owner action:** after E-0 has merged (so ADR-0007 is on `main`, which
  P's README asks for), on a topic branch from `main`:
  `git apply docs/passes/16-staged/P/01-licences-libc.patch`, then
  `scripts/validate.sh`, commit, open the pull request. The unpatched graph has
  no `libc`, so the patched script passes before `sha2` lands. Counted as
  applied when the reverse check above succeeds on `origin/main`.
- **The digest's design and its tripwire do not wait:** §3 is reviewed now,
  and E-2 keeps the DP-049 (a) assertion failing on a flip until E-4.

## Tests

Every invariant test is named `invariant_NN_*`. In-code plans and fakes, or
recordings.

**Part D**
- `a_recheck_records_the_program_it_was_made_for` (application unit).
- `a_token_for_another_program_is_refused_and_nothing_is_handed_over` (shell
  unit: a recording fake site sees no call; `WRONG_PROGRAM_TOKEN`).
- `certify_destination_refuses_a_token_for_another_program`, including a
  two-command step whose second command names another program.
- `invariant_01_a_destination_token_for_another_program_is_refused`
  (`destination_still_verified`, before `REMOVE_SOURCE`).
- `executor_wiring.rs`: each re-check is made for the program of the command
  it is spent on (the fake `Reread` records its argument; the fake site the
  token's program).

**Part C**
- `a_line_typed_before_the_question_is_never_consent` (input holds
  `allow step 3\n` and the right code is drawn after: `Declined`).
- `each_question_has_its_own_code` (two requests, the seam called twice, each
  phrase carries its own; the first request's code does not answer the second).
- `without_a_code_nothing_is_asked` (seam `None`: `Declined`, zero bytes read,
  no question written).
- `a_code_is_six_lowercase_hex_and_nothing_else` (`ConsentCode`);
  `confirm_requires_the_code_and_the_step` (wrong code, no code, code for
  another step, upper case).
- The four existing tests in `crates/cli/src/consent.rs` updated to the coded
  phrase.
- `entropy_reads_three_bytes_or_answers_none` (adapters; a path seam to a
  missing file gives `None`).

**Part A (E-2, interactive)**
- `a_shown_plan_covers_only_the_plan_shown` (one changed field fails `covers`).
- `confirm_shown_mints_only_on_the_exact_phrase_with_its_code`.
- `invariant_02_a_plan_other_than_the_one_shown_dispatches_nothing_and_takes_no_lock`
  (shell: a `ShownPlan` for plan P and a cleared run of plan P′; the observer
  sees no `Probed`, the state directory is untouched).
- `invariant_02_a_declined_plan_question_exits_6_before_anything_runs` and
  `the_plan_question_is_asked_only_at_two_terminals` (`confirm_plan` with fake
  streams: not a terminal → nothing read, nothing written).
- `the_plan_is_shown_on_standard_error_before_it_is_confirmed` (order in the
  prompt stream: plan text, then the question).
- The two §6 structural tests; `invariant_02_migrate_runs_only_a_plan_tied_to_the_one_shown`.

**Part A (E-4, digest)**
- `plan_digest_is_stable_and_covers_every_field` (a table of single-field
  edits, one per `MigrationPlan` field and one per `MigrationStep` and
  `PlanCommand` field, each changes the digest; the same plan built twice gives
  the same digest).
- `plan_digest_is_pinned_for_a_fixed_plan` (a literal `sha256:…` for an in-code
  plan: a `serde_json` or `sha2` bump that changes the bytes fails here, not in
  the field).
- `invariant_15_the_same_recording_gives_the_same_plan_digest` (every recording
  that yields a plan, `migrate --replay … --json` run twice as two processes;
  the `planDigest`s equal).
- `the_plan_holds_no_unordered_collection` (word scan of `crates/domain/src/`:
  no `HashMap`, `HashSet`, `f32`, `f64`).
- `the_digest_is_of_the_plan_not_of_the_redacted_document` (a plan under a
  home: the digest differs from the SHA-256 of the document's `plan` value).
- `a_shown_plan_cannot_be_minted_from_another_digest` (`tie` with a digest of
  P′).
- `invariant_02_a_digest_of_another_plan_dispatches_nothing_and_takes_no_lock`
  (binary, `--replay`: stdout empty, exit 4, the message; versus a matching
  digest: the document and the gate's refusals).
- `the_digest_migrate_shows_is_the_one_yes_checks` (render then
  `--yes --plan-digest`, two processes, `--retain-source` included).
- `a_malformed_plan_digest_is_a_usage_error` (prefix, length, case,
  whitespace; exit 2, stdout empty) and
  `plan_digest_needs_yes`.
- `yes_requires_a_well_formed_plan_digest_when_mutations_dispatch` and
  `yes_without_a_digest_asks_at_a_terminal_and_refuses_elsewhere` (switch
  `true`, isolated worktree, subagent; never committed).
- `no_workspace_crate_depends_on_libc_directly`.

**Part B**
- `consent_for_asks_only_when_there_is_something_to_ask` (no requests: the
  closure is never called — it panics if it is).
- `consent_for_never_asks_about_a_plan_that_disagrees_with_the_table`
  (`PlanDisagrees`, closure never called).
- `consent_for_maps_each_refusal_to_its_exit` (`Declined` → 6, `Unseen` → 4,
  `PlanDisagrees` → 4) and `consent_for_refuses_a_token_count_that_is_not_one_per_request`.
- These run on an uncleared `PlannedRun` whose plan elevates (a
  MacPorts-side removal or add, whose cell is `required`): the application
  decision is testable today even though no cleared run reaches it.
- `invariant_06_the_guard_halts_an_elevated_command_whatever_the_tokens`
  (`next_action_with` on an elevated mutating step under an allowance:
  `ElevationNotNone`; plus a scan that its signature names no `ConsentGiven`).
- `invariant_06_a_declined_or_unseen_question_halts_before_the_marker` —
  application half here (`consent_for` returns the halt); shell half
  structural (§6's order and `?` rule). The end-to-end shell case needs a
  cleared elevated run, which only part J's gate produces; declared, and handed
  to J.
- `a_consent_token_is_carried_to_the_site` (shell unit: a token obtained
  through `consent::ask` with fake streams, handed through `mutation_dispatch`,
  received by a recording fake site; `None` when none was given).
- `executor_wiring.rs` order test: `Replanned` → `MarkerRead` → (no
  `ConsentAsked` for a run with no requests) → `SnapshotWritten` →
  `MarkerWritten` → …, unchanged for every cleared run.
- `the_switch_needs_consent_wired` passes with the switch `true` (isolated
  worktree, subagent).
- i18n parity and `hostile_text.rs` for every new key, both locales.

## Mutants (isolated worktree, by a subagent; removed afterwards)

The slice work order's list (§3 "How to work") is the minimum; each of its
items is marked (S). Each must be killed; a survivor is argued in writing.

1. (S) `mutation_dispatch` drops the program comparison.
2. `certify_destination` drops the program comparison.
3. `destination_still_verified` drops the program comparison.
4. A program comparison that checks command 0 only.
5. `recheck_provider_configuration` stores an empty or constant program.
6. The shell re-checks with another command's program (the verify program for
   the source's command).
7. (S) The code dropped from the phrase.
8. (S) One code reused across requests (the seam called once per `ask`).
9. (S) A failing nonce falls back to an empty code (kept as a mutant though
   `ConsentCode` makes it unrepresentable; expected killed at compile time,
   recorded as such).
10. `ask` reads the answer before drawing the code, or draws it and does not
    put it in the phrase.
11. `confirm` accepts a prefix of the phrase or ignores case.
12. (S) `tie` always `Ok` (E-4).
13. (S) The digest over a subset of fields (one field skipped in the
    serialisation, e.g. `#[serde(skip)]` on `steps[].commands[].argv`) (E-4).
14. (S) The digest over the redacted document (E-4).
15. The digest without the domain-separation prefix or version (killed by the
    pinned literal).
16. `PlanDigest::parse` accepts upper case, whitespace or a short value.
17. `covers` always true.
18. `confirm_shown` mints on any line, or without the code.
19. (S) The tie check moved after the lock (after `history_store::probe(` or
    `run_lock::acquire(`).
20. The tie check removed from `run_with_mutations` (`migrate.rs` still ties):
    killed by the shell test of a mismatched `ShownPlan`.
21. `migrate` skips `tie` when `--plan-digest` is given but no allowance exists
    (E-4: killed by the replay mismatch test).
22. The interactive question asked when only one stream is a terminal.
23. (S) The guard lets an elevated command through when a token is present
    (a `consents` parameter added to `next_action_with` and consulted).
24. (S) A declined answer that still writes the marker (`consent_for` result
    ignored, or the call moved after `run_marker::write(`).
25. `consent_for` asks when there are no requests, or asks on `PlanDisagrees`.
26. `Declined` mapped to exit 4 or `Unseen` to exit 6.
27. The second recapture removed, or run whatever was asked.
28. The token not handed to the site (`None` always).
29. `ConsentTokens::take` returns a token for another step.

## Needs the owner

1. **Apply P/01** (blocks E-4 only). After E-0 has merged: on a topic branch
   from `main`, `git apply docs/passes/16-staged/P/01-licences-libc.patch`,
   `scripts/validate.sh`, commit, open the pull request. (Or send a session the
   armed sentence in `docs/passes/16-staged/P/README.md`, applying 01 alone.)
   Applied iff `git apply --reverse --check` of the patch succeeds on
   `origin/main`; the ledger records the commit.

Nothing else in E needs the owner: no allowlist growth, no capture, no guarded
file.

## Handed to later parts

- **G:** the probe as ADR-0007 §0.2 decides — `libc` as a direct dependency of
  `crates/adapters` only, the one `unsafe` call in one module, `unsafe_code`
  lowered to `deny` in the same change with SE-110 amended, the single-site
  scan, `no_workspace_crate_depends_on_libc_directly` edited to admit the
  adapters, and the staged one-line fix of `scripts/check-licences.sh:10`'s
  `forbid` comment.
- **J:** relaxing the guard for a covering token
  (`invariant_06_only_a_covering_token_lets_an_elevated_command_through`); the
  end-to-end shell test of a declined or unseen elevation question; the second
  recapture exercised; DP-046's sudoers disclosure before the question.
- **K:** DP-037's stronger confirmation builds on `confirm_plan` (the named-entry
  notice already precedes it); removal opt-in.
- **L:** the flip proposal lists that both routes are built and the elevation
  prompt is reachable only after J.

## What this part does not do

No change to `THIS_BUILD_DISPATCHES_MUTATIONS`, `process/mod.rs`,
`provider-commands.toml` or `execution_gate.rs`; no new spawn site and no new
permitted program; no elevation and no elevated command spec; the guard is not
relaxed; no lock probe, no `libc` direct dependency, no lint change (G); no
guarded file; no stronger confirmation levels (K); `01-invariants.md` not
edited.

## Changes from review round 1 (binding)

Both reviewers approved the plan with changes on `e6556e2`. Each change below
overrides the text above where they differ; the body is left as written so the
review can be read against it. Identifiers: `SG` spec-guardian, `SR`
security-reviewer.

**R1 — Where the interactive plan is minted (SG M1, SR L4).** The interactive
`ShownPlan` is minted only **after** the live-scope check (`migrate.rs:221`) and
after `clear(planned, Some(allowance))` succeeds (:228–233): `confirm_shown` and
`cli::consent::confirm_plan` take the `GateClear`, and are called inside
`execute_mutating` after the gate. A plan the gate or the live check refuses is
never asked about; test
`invariant_02_a_plan_the_gate_refuses_is_never_asked_about`. The digest route's
`tie` stays "whatever the allowance" (testable in this build).
`confirm_plan` renders the plan from the value it mints from, so what is shown
and what is minted cannot be two variables (SG L5).

**R2 — Stdout and error ids on a refusal before execution (SG M2, SR L8).**
Stdout is empty on each, and each has its own error id, so a script tells them
from the planner's own exit 4 by the id, never by stdout:
- digest mismatch → new `ErrorId::PlanNotShown` (`PLAN_NOT_SHOWN`), exit 4;
- plan question declined → `USER_ABORTED`, exit 6 (the plan was shown on
  standard error);
- `--yes` without a terminal and without `--plan-digest` → `USAGE`, exit 2.

`migrate.rs`'s module rule 5 (:32–34), `docs/schemas/README.md:95–97` and
`migrate.v1.md` "Exit codes" are amended in the same change (E-2 for the
interactive cases, E-4 for the mismatch), and SE-111 records it.

**R3 — When consent is asked (SG M3).** 04 "Privilege" says elevation is
requested per operation at the moment it is needed. The reading recorded in
SE-111 and written into 04 (E-3): the **question** is asked once per elevated
operation, up front under the lock and before anything is written, so a refusal
leaves nothing half-done; the **credential** is requested by part J's spawn at
the moment that operation runs, and a token covers exactly one operation.

**R4 — What `--yes` means (SG M4).** `--yes` means "execute"; it never means
"do not ask", and it never answers a question (`application/src/consent.rs:22`,
04 :437). Recorded in SE-111; `plan.v1.md` obligation 8 (:330–332) and 06's
`migrate` line are reworded in E-2. `--plan-digest` is a `migrate` flag, not a
global one (SG L4): §8's "global flags (:61)" is wrong.

**R5 — Exit 6 only for a person's refusal (SG L1, SG L2, SR L6).**
- No code available → `ConsentRefusal::CodeUnavailable`, mapped to
  `SAFETY_BLOCKED` (exit 4, fail closed); nothing asked, nothing read.
- `consent_for` maps every `ConsentDecision` arm by name: `Given` checked
  token by token — each token's `step()` and `covers(step, program, argv)` must
  match its request, in order, one per request; any other shape is `Internal`;
  `Refused(Declined)` → `UserAborted` (6); `Refused(NotATerminal)`,
  `Refused(NoRequests)`, `Refused(PlanDisagrees)` and `CodeUnavailable` →
  `SafetyBlocked` (4). `NoRequests` is never read as empty tokens. Test
  `consent_for_maps_every_decision_by_name`.

**R6 — The token's program is checked, not only carried (SR L1; round 2 SG N1).** The
`executor_wiring.rs` test that the fake `Reread` records its argument and the
token carries that same program is mandatory. `recheck_provider_configuration`
is never called with an empty program: the **shell** checks first, and a
verify step with no commands (`mod.rs:609`/:801 fall back to `""` today)
answers `WrongProgramToken` without a re-check; the destination sites likewise
answer `WrongProgramToken` for a step with no command. E-1 first checks that no
existing test depends on how a `VERIFY_DESTINATION` with no commands behaves. If E-1 finds the seam's
result type can carry the program it was re-read for, the token is built from it.

**R7 — The entropy source (SR L2).** `adapters::entropy::fresh_bytes` opens
`/dev/urandom` read-only, checks with `fstat` on the handle that it is a
character device (anything else is `None`), and `read_exact`s three bytes, never
`fs::read`. The path seam is private to the module's tests. A structural test
pins that `migrate.rs` passes `adapters::entropy::fresh_bytes` as the code
source with no fallback (SG L5). `ask` draws the first request's code before it
writes the heading (`cli/src/consent.rs:79`), so `without_a_code_nothing_is_asked`
holds. SE-111 records why 24 bits is enough: a pre-typed line matches with
probability 2⁻²⁴ and a miss halts the run, so there is no retry; a program
driving a pseudo-terminal can read and answer the code, which is the user's own
automation, outside SR 10.

**R8 — The digest's disclosure (SR L3).** `migrate.v1.md` says `planDigest` is
a fingerprint of unredacted data and is to be removed before publishing a
document; a test pins that `doctor --report` never contains it; ADR-0007's
"Revisit when" gains a keyed digest. "Valid only for the build that printed it"
reads "only for builds with the same version string and plan serialisation".

**R9 — Who may mint (SR L5, SG L5).** The `confirm(` caller scan
(`crates/cli/tests/executor.rs:1385`) gains `confirm_shown(` (callers:
`cli::consent` only) and `tie(` (callers: `migrate.rs` only). `ShownPlan`
derives `Debug` only (a scan fails on any other derive, `Deserialize` in
particular); `PlanDigest` has no public byte constructor (only `of` and `parse`).

**R10 — Dependencies and the lint (SR M1, SR M2, SR M3, SG M6, SG L3, SR L7).**
ADR-0007 is revised: the workspace table goes to `deny`, the four crates other
than `adapters` restore `#![forbid(unsafe_code)]` at their roots — for `cli`
both `src/lib.rs` and `src/main.rs`, the binaries' root (round 2 SG N2) —
(compiler enforced, no guarded file), and the scan is code-aware over every `.rs` under
`crates/`, matching the keyword `unsafe`, the identifier `unsafe_code`, and the
identifier `libc`; a leaf `-sys` crate is considered and rejected; the
`F_GETLK` query is specified (`F_WRLCK`, held iff `l_type != F_UNLCK`, −1 is
unknown) with G's two extra mutants; the dependencies' `unsafe` surface is
reviewed; ADR-0004's table carries the `libc` row; "first build-script
dependency" is corrected; nine crates. E-4 runs
`scripts/check-deployment-target.sh` for both targets and records both minimum
OS versions. SE-110 is reformatted and filed under "Platform, toolchain,
licence and distribution". This supersedes §0.2's lint bullets.

**R11 — Tests renamed and added (SG L6, SG L7).**
- `certify_destination_refuses_a_token_for_another_program` →
  `invariant_01_certify_destination_refuses_a_token_for_another_program`;
  `the_plan_is_shown_on_standard_error_before_it_is_confirmed` →
  `invariant_02_the_plan_is_shown_on_standard_error_before_it_is_confirmed`.
- INV-17: `consent_for`'s tests include an elevated step in the other direction
  (a cask whose cell is `unknown`); `invariant_17_the_tie_and_the_program_token_behave_alike_in_both_directions`
  runs the tie and the program-token checks over planner-built plans from
  `fixtures/`, both directions, and no honest `VERIFY_DESTINATION` trips
  `WRONG_PROGRAM_TOKEN`.
- INV-15: the digest test also runs over `fixtures/snapshots/` through
  `--from-snapshot`; `invariant_15_the_consent_code_never_enters_a_document_or_a_record`.
- `plan_digest_is_stable_and_covers_every_field` destructures each struct
  without `..`.
- Tripwires: the consent checks carry a named tag asserted by `violations_with`'s
  caller; a self-test `the_consent_tripwire_fires_on_its_mutants` (after
  `run_marker::write(`, `let _ =`, the second recapture missing). The textual
  "`io.shown.covers(` precedes the probe" check passes `let _ =`; that mutant
  is killed by the behavioural `invariant_02_a_plan_other_than_the_one_shown_…`
  test, which is why the textual check is enough. In
  `08-pre-flip-design-notes.md`'s table the consent row is marked done in E-3,
  the tie row "interactive built; digest owed" until E-4.

**R12 — Docs (SG L4).** `docs/schemas/README.md`'s exit-code table row 6 gains
`USER_ABORTED` (E-2); its 4 row names `PLAN_NOT_SHOWN` (E-4).

**R13 — Mutants added (SR question 7).**
30. The plan question's code reused for the first elevation question.
31. `covers` comparing a subset of fields.
32. The second recapture's result discarded (the pre-consent capture written).
33. The interactive confirmation minted from the pre-history plan
    (`migrate.rs:375`, not after :405) — needs a `--retain-source` case.
34. The product code source returning a constant.
35. `consent_for` mapping `NoRequests` to empty tokens.
36. The program comparison in `mutation_dispatch` moved after `perform`.
37. The interactive question asked before the gate (R1).

**R15 — For G (round 2 SR N1, N2).** The root-attribute test lists every lib
and bin target from `cargo metadata`, not a fixed file list. The scan's comment
and string stripper is a real lexer or carries its own tests for raw strings
(`r#"…"#`), nested block comments and a `'"'` literal, plus a self-test that a
planted `unsafe {}` and a planted `#[allow(dead_code, unsafe_code)]` are caught.

**R14 — G's dependency on E (SG L8).** G waits on P/01 and on E-4 (ADR-0007
accepted). The ledger says so.

## As built (E-1)

- **SR 9** (`9c9dbbd` and its review fixes). The plan's
  `certify_destination_refuses_a_token_for_another_program` is named
  `invariant_01_certify_destination_refuses_a_token_for_another_program`
  (R11). The shell unit test uses the product site `NotInThisBuild` instead
  of a recording fake: the word scan
  `there_is_exactly_one_mutation_site_and_it_refuses` allows one
  `MutationSite` implementation under `crates/*/src`, and `Refused` versus
  `PreconditionFailed` shows whether the site was handed the command.
- **The `Reread` seam** (R6's last sentence): it could return the program with
  the configuration, but the token is built from the same local variable the
  shell passes to `reread`, `recheck_provider_configuration` stays a public
  pure function, and SE-092 keeps the shell to sequencing; recorded in SE-111.
- **The shell's empty-program guards** (all three paths). On the two verify
  paths they sit behind the gate (a verify step's commands must be the table's
  operations) and `plan_still_holds`, so no wiring test reaches them; the
  destination sites refuse the same step themselves, tested.
- **Mutant 3** (`destination_still_verified` drops the comparison): its early
  check is killed by the cleared-step case (`DestinationNotReverified`
  without it); the match arm mapping the inner `WrongProgramToken` is
  equivalent while the early check stands, and is kept as defence in depth,
  as the `WrongProviderToken` arm is.
- **SR 10.** `ConsentRefusal::CodeUnavailable` (R5) is added in E-1; its exit
  mapping lands with `consent_for` in E-3.
- **The mutating path's empty-program guard** (SR review L3) is defence in
  depth too: no plan the gate passes has a mutating command with no program,
  and `registry::reread_configuration` refuses `""` anyway; no test reaches it.
- **Owed to E-3** (SR review of `013025d`, I1): the structural pin that
  `migrate.rs` passes `adapters::entropy::fresh_bytes` with no fallback, a
  caller scan that refuses `ConsentCode::from_bytes(` outside the consent
  module and its tests, and a check that `--replay` and `--from-snapshot`
  never reach `ask`.

### Mutation proof (E-1, at `013025d`, isolated worktree, removed)

Fourteen mutants, each against `cargo test --workspace --no-fail-fast`: twelve
killed, two survived and are argued here.

| Mutant | Result | Killed by |
|---|---|---|
| 1 dispatch drops the program check | killed | `a_token_for_another_program_is_refused_and_nothing_is_handed_over` |
| 2 `certify_destination` drops it | killed | `invariant_01_certify_destination_refuses_a_token_for_another_program` |
| 3a `destination_still_verified`'s early check removed | killed | `invariant_01_a_destination_token_for_another_program_is_refused` |
| 3b its `WrongProgramToken` arm removed | survived | equivalent while the early check stands (As built) |
| 4 command 0 only | killed | `invariant_01_certify_destination_refuses_a_token_for_another_program` |
| 5 an empty program stored | killed | the dispatch test and ten `executor.rs` tests |
| 6 another program re-checked for the command | killed | `each_re_check_is_made_for_the_program_it_is_spent_on` and eleven others |
| 6b the mutating path's empty-program guard removed | survived | unreachable: a plan whose command names no program differs from the plan re-built under the lock (`plan_still_holds`), and `registry::reread_configuration` refuses `""`; defence in depth (As built) |
| 7 code dropped from the phrase | killed | six consent tests |
| 8 one code reused | killed | `each_question_has_its_own_code` |
| 9 a failed draw falls back to a fixed code | killed | `without_a_code_nothing_is_asked` |
| 10 heading before the first draw | killed | `without_a_code_nothing_is_asked` |
| 11 case-insensitive confirm | killed | two consent tests |
| 12 no character-device check | killed | `entropy_reads_three_bytes_or_answers_none` |

The order of write, draw and read, and a draw failing mid-run, were added after
the run on the reviewers' advice
(`each_code_is_drawn_before_its_question_and_shown_before_the_answer_is_read`,
`a_code_that_fails_mid_run_gives_no_partial_consent`).

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
| spec-guardian | `e6556e2` | approve with changes: M1–M6, L1–L8 | R1–R6, R9–R12, R14 |
| security-reviewer | `e6556e2` | approve with changes: M1–M3, L1–L8 | R1, R2, R5–R10, R13 |
| spec-guardian | `7313df2` | approve; N1–N4 (low) | N1 in R6, N2 in R10, N3 ADR-0004 sentence qualified, N4 superseded markers in the body |
| security-reviewer | `7313df2` | approve; N1–N2 (low, for G) | N1 in R10 and R15, N2 in R15 |
| docs-auditor | `3efe24c` | 4 medium, 5 low (dates, an unsourced `EINVAL`, ADR-0004's count, tense, status wording, stale lines) | all fixed in the next commit |
| security-reviewer | `9c9dbbd` | approve with changes: M1–M2, L1–L3, info | M1 cleared-step case; M2 argued (As built); L1 SE-111; L2 both directions; L3 guard on the mutating path |
| spec-guardian | `9c9dbbd` | approve with changes: F1–F8 | F1 argued as M2; F2 and F3 cleared-step case and As built; F4 both directions; F5 SE-111; F6 Evidence; F7, F8 As built |
| security-reviewer | `013025d` (and `e1d6aae`) | approve: L1–L2, I1–I3 | L1, L2 tests added; I1 owed to E-3 (As built); I2 accepted (root needed to plant a link at the device); I3 As built |
| spec-guardian | `013025d` (and `e1d6aae`) | approve with changes: D1–D4, R1–R3 | D1 SE-111 future tense and As built (owed to E-3, named `invariant_06_*` there); D2 phrase escaped; D3 impact lines; D4 As built; R1 `/dev/null` case; R2 E-3; R3 accepted |
