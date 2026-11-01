<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part F — slice 8, with the executable list from provider metadata (DP-047 (b))

Plan written 2026-11-01, before any code, on `wo16/F-0-plan`, cut from
`origin/main` while part E-4 (PR #51) was in review. It was **held uncommitted
until E-4 merged** (`cc324f9`, PR #51), and is committed after it: F depends on E in the ledger's dependency column
(`docs/passes/16-progress.md` row F), and §4.1 starts a part only when its
dependencies are merged (DA-2, DA-18). **Revised 2026-11-01 after review round 1**
(`spec-guardian` SG-1..14, `security-reviewer` SR-1..16, both "approve with
changes"; see Reviews and "Changes from review round 1 (binding)"). Every
section that changed says so and names the finding. Their review is the approval
(work order 16 §4). **Revised again 2026-11-01 after review round 2**
(`spec-guardian` D1, Q3, N1–N7 — cited here as SG2-…; `security-reviewer` N1–N7
and the `F/05` additions — cited as SR2-…; both "approve with changes"; see
"Changes from review round 2 (binding)"). **Then `spec-guardian` and
`docs-auditor` reviewed the F-0 diff** (cited as SG3-n and DA-n; see "Changes
from the F-0 diff review"). Line numbers in code files were read before E-4
lands and are re-checked at F-2a.

## The ruling

- **DP-047**, resolved 2026-10-27: (b) now, (a) — layer 4, consented — in part I.
  "The executable list comes from provider metadata, so `rg` for `ripgrep` is
  planned rather than refused." INV-11 and INV-12 bind.
- **Work order 16 §F** (`prompts/16-complete-mutating-product.md` ≈471–475): run
  `prompts/08-slice-8-primary-and-resolution.md` with one change — the list from
  the reads P established — and apply §3 item 10's shim-name rule. §1 item 8
  reverses slice 8's "refuse an executable not named after its package", and
  moves the plan here and the report into `docs/passes/16-report.md`.
- **DP-051 (a)** (part H) evaluates the four destination checks; F does not. F
  only stops naming the package where the contract names executables.
- **Slice 8 §1's stop condition binds:** if `COMPLETED` needed the planner to
  stop emitting a check of 04's verification contract, F stops and writes a DP.
  Nothing below needs that.
- **New, not covered by DP-047 (SG-1):** what F does when the destination is
  not installed, so neither its placement on `PATH` nor whether it ships a given
  executable can be read. That is **DP-055** (`docs/decisions-pending.md`,
  Open), raised by round 1. Reversible default taken: **(a), refuse in both
  directions**; recommendation **now also (a)** (changed by SG2-D1: (b)'s own
  install can newly shadow names before anything verifies it — see DP-055).
  SE-112 records DP-055, not DP-047, as the authority for it.

What I depends on: I offers a shim exactly where F refuses with
`PRIMARY_NEEDS_SHIM`, for the executables F lists, through the step and consent
shapes F and E define. F's refusal carries the executable names and a reason per
executable, not a package-level yes/no.

## Where it lives today

- **`SET_PRIMARY`** `crates/application/src/planner.rs:944-973`: a `TOOL`
  command `macportsbrew primary <name> <to>` (no such subcommand in
  `crates/cli/src/app/mod.rs`) and `PROVIDER_REPORTS_ACTIVE` (`:971`), not
  evaluable on that step (`execution_gate.rs:531-544`). **`VERIFY_RESOLUTION`**
  `:974-980` carries no command and no check. `primary_after: request.to`
  `:1063` — the request, not an observation.
- **The contract names the package where it means executables:**
  `ExecutablesOwnedByDestination { executables: vec![dest_name] }` and
  `LivenessProbe { executable: dest_name }` (`planner.rs:926-936`); spec 04
  check 4 says "each primary executable" (`04-migration-and-safety.md` ≈52-58).
- **Gate.** `evaluable_in_this_build` `execution_gate.rs:531-544`;
  `CheckNotEvaluableInThisBuild` `:684-693`; `VerificationAssertsNothing`
  `:694-698` (a verification step with no command or no check);
  `CommandOnStepThePlannerWritesNoneOn` `:706-711` driven by
  `planner_writes_commands_on` `:923-936`, which today says `true` for
  `SetPrimary`; `CheckHasNoAnsweringRead` (`:302`, pushed ≈`:779`).
- **Ordering.** `MigrationPlan::check_ordering` `crates/domain/src/plan.rs:664-710`
  orders `REMOVE_SOURCE` after `ADD_DESTINATION` and `VERIFY_DESTINATION`
  (`:698-706`) and says nothing about `VERIFY_RESOLUTION` (SG-4).
- **Executor.** `SET_PRIMARY` is mutating (`plan.rs:56-59`), guarded at
  `executor.rs:885-891`; `VERIFY_RESOLUTION` halts `NoEvaluationForStepKind`
  (`:698-701`); `Disposition` `:88-128` has no `NothingToChange`; `outcome_of`
  `:1303` needs every step.
- **Domain.** `VerificationCheck` `plan.rs:341-402` (no `ResolvesTo`);
  `PlanRisk` `:420-429`. `PackageMetadata.executables: BTreeSet<String>`
  `crates/domain/src/package.rs:143-145`, skipped when empty and always empty:
  "none" and "never read" are one value — the defect DP-032 fixed for
  `defines_service` (`:131-142`). `notOnDefaultPathReason` (`:127-130`) is filled
  by the Homebrew adapter only (SG-1). `ProviderRead`
  `crates/domain/src/provider.rs:349-408` has no executables method.
- **Capture.** `crates/application/src/capture.rs:73-107` resolves every
  installed **package name**; `which` returning `None` is skipped (`:82-84`), so a
  name with no hit and a name never resolved leave the same trace (SG-2, SR-1);
  the hit is stored under the first provider that returns a resolution, owner or
  not (`:92`) — `m4` → `/usr/bin/m4`, `UNKNOWN`, sits under `homebrew` in the
  fixture. `which` (`adapters/src/system/machine.rs:530-564`) skips relative
  entries, uses `is_plain_executable_name` (`:580`), answers `None` under replay.
- **Resolution.** MacPorts' `resolution()` (`adapters/src/macports/mod.rs:896-960`)
  canonicalises only on this machine and asks `port -q provides <path>` — an
  allow-listed read through the runner, so "evaluated in process (no spawn)" in
  the design notes (`docs/passes/08-pre-flip-design-notes.md`, "Slice 8 —
  Design"; DA-17) is inexact: no *new* spawn site or program, but a read.
  Homebrew's `authoritative` is a Cellar-path rule (R-21 E-14, `Keg.for`);
  MacPorts' is the registry's answer (SG-9).
- **P/02, applied** (`12220fe`; `git apply --reverse --check
  docs/passes/16-staged/P/02-allowlist-f-h-reads.patch` succeeds here):
  `macports::ARGV_CONTENTS = ["-q","contents"]`,
  `homebrew::ARGV_LIST_FORMULA = ["list","--formula"]`, spawn-site shapes with
  `Positionals::ExactlyOneName`. Nothing calls them. Homebrew's `run`
  (`homebrew/mod.rs:312-318`) accepts only exact `ALLOWED_ARGV`. The name classes
  are `process::is_port_name` (`process/mod.rs:339`) and `is_brew_name` (`:355`),
  both `pub(crate)`, and `is_absolute_plain_path` (`:367`), **private** (corrected
  by SG2-N1; round 1 said all three were `pub(crate)`); MacPorts' prefix check is
  `strip_head` (`macports/mod.rs:188`).
- **R-21** (2026-10-28): `port -q contents <port>` — two-space-indented absolute
  paths, exit 1 `Registry error` when not installed, inactive ports unobserved;
  `brew list --formula <f>` — keg files in `find` order, `--formula`
  load-bearing, installed names only, 1.75 s per call; executables are direct
  entries of `bin/` and `sbin/`; Homebrew reachability depends on
  `keg_only`/`linked_keg`; casks not covered.
- **Fixtures.** No recording has either read; `scripts/capture-fixture.sh`
  records neither, and its `$port_names` expansion (≈line 137) is unquoted
  (SR-14). `fixtures/snapshots/both-providers-intel-macos26-live.json` (captured
  2026-10-28): `PATH` user dirs 0–5, `/usr/local/bin` 6 (again 8),
  `/usr/local/sbin` 7, `/usr/bin` 10, `/opt/local/bin` 25; Homebrew
  `environment` has `bin`, **no `sbin`**; 201 formulae, 18 ports, eight in both
  (`autoconf`, `gettext`, `htop`, `libtool`, `m4`, `ncurses`, `tree`, `xz`);
  Homebrew `m4` and `ncurses` are keg-only — `keg_only: true` in the recorded
  `info --json=v2 --installed` of `fixtures/recordings/both-providers-intel-macos26.json`
  (captured 2026-09-07; the snapshot's `notOnDefaultPathReason` carries the same
  fact) (DA-9).
  `fixtures/recordings/both-providers-arm64-macos15.json` is **derived**
  (`fixtures/recordings/README.md` ≈93): no second real `PATH` order exists.
- **Tests that delete the steps:** `crates/application/src/executor/tests.rs`
  `mutable()` at 674 and `completable_run()` at 1483 (corrected by DA-14).
- **What the build does today (SG3-3):** the gate refuses every planner-built
  plan before step 1 — `CheckNotEvaluableInThisBuild` for `SET_PRIMARY`'s
  `PROVIDER_REPORTS_ACTIVE` and for DP-051's four checks
  (`execution_gate.rs:684-693`). No real plan reaches `SET_PRIMARY`; only the
  narrowed test plans run that far.
- **History.** `HISTORY_SCHEMA_VERSION = 2` (`domain/src/history.rs:522`);
  recognition `application/src/history.rs:434-453` resolves the **package name**
  against `primary_after` (`:447`, `resolves_to` `:465`).

## Files: what F changes, and what it may not

§3 freezes `crates/adapters/src/process/mod.rs` and
`data/policy/provider-commands.toml` (item 3) and guards `scripts/*.sh`,
`.claude/hooks/`, `.claude/settings.json`, `.github/workflows/` (item 4). Slice 8
§2 adds `git diff --stat main -- crates/adapters/src/process/` empty, so
`process/recording.rs` is out of bounds too. **`execution_gate.rs` is not
frozen**; F edits it with both reviewers, and every fact it acts on travels in
the plan (`.claude/rules/executor.md`). F **calls** the `pub(crate)` name-class
functions in `process/` from the adapters (same crate) without editing them
(SR-7).

F changes: `domain/src/{package,plan,history,resolution,snapshot,error,machine}.rs`
and a new `domain/src/executable.rs`; `adapters/src/{homebrew,macports}/`,
`adapters/src/system/machine.rs` (SR-11), `registry.rs` if the read site needs
it; `application/src/{capture,planner,execution_gate,executor,history}.rs`;
`cli/src/executor/` (`ResolutionSite`), `cli/src/app/` (snapshot's positionals);
locales; schemas; spec 04/05/06; CHANGELOG; wiki. F stages, never edits:
`scripts/capture-fixture.sh` as `docs/passes/16-staged/F/05-capture-executables.patch`
(SG-7), and — only if R-24 says so — `docs/passes/16-staged/F/06-...` for
`process/mod.rs`.

## Design

### 0. Spec first (F-0) — *changed: SG-1, SG-4, SG-7, SG-8, SG-9, SR-2, SR-6*

SE-112, one entry, lands **in F-0** with the 04 and 05 edits (SG-7): "the first
release performs layer 1 inside the run; the executable list comes from
provider metadata". Addenda to SE-036 and SE-092 (S6 "Not closed (2)" and S8 D3
close for no-shim plans; open for shim plans until part I). It says:

- `SET_PRIMARY` stops carrying the `TOOL` command and `PROVIDER_REPORTS_ACTIVE`;
  the check is not dropped from the contract, it stays on `VERIFY_DESTINATION`
  (`planner.rs:922`).
- **04 check 3's "expected executables" are the source package's executable
  list** — the executables the user runs today (SG-4). A package whose list is
  `KNOWN{}` (empty, and only from a non-empty fully parsed listing, SR2-N1) has
  no primary to set: `NoExecutables`, carried on a command-less `SET_PRIMARY`
  completed with no dispatch, and no `VERIFY_RESOLUTION`, gate-checked (SR-2,
  changed by SG2-N2). Both readings go into the 04 edit.
- **DP-055 governs** the uninstalled-destination case (SG-1); SE-112 does not
  cite DP-047 for it. **Under DP-055 (a) the predicted-win branch (`d < w`,
  Design 4 step 6) is effectively unreachable** (SR2-N7; reasoning aligned with
  SE-112 by DA-10): an uninstalled destination is refused
  `PRIMARY_DESTINATION_NOT_INSTALLED` before its placement is ever compared, and
  an installed, linked destination whose directory comes first is already the
  winner. So `COMPLETED` comes only where the destination is already installed
  and wins (step 3), and part F does not migrate a primary to a destination it
  installs. A real plan's `COMPLETED` also waits on part H (DP-051) (SG3-3).
- **Spec 05's MUST "never shadow a macOS system tool without saying so"**: F
  proves a source-list executable never newly shadows anything (Design 4, step 5
  note) and declares the gap that remains — a destination executable *outside*
  the source's list — in 05 and SE-112 (SG-8). **The declared gap also names the
  install's own side effect (SG2-D1):** installing an uninstalled destination
  links every `bin`/`sbin` entry it ships at its own `PATH` position (Homebrew's
  at 6/7 here, ahead of `/usr/bin` at 10), so the install itself can newly
  shadow system tools and other packages' names — before `SET_PRIMARY`, and
  outside anything `VERIFY_RESOLUTION` checks (spec 05 MUST, INV-12, INV-03).
  That is why DP-055's recommendation is (a).
- **`authoritative` differs per provider** (SG-9): a registry answer for MacPorts
  (`port -q provides`), a Cellar path rule for Homebrew (no Homebrew read answers
  "which keg owns this path", R-21 E-14). F treats both as authoritative as
  today; making Homebrew's an owner query is handed to H
  (`EXECUTABLES_OWNED_BY_DESTINATION`).
- **Known limit** (SR-6): the decision uses the `PATH` captured by this process;
  the user's shells can differ, and their command hash can too. The plan carries
  the redacted `PATH` it used — so the plan the person confirms covers it: the
  shown plan, or under `--yes` its digest (E-4, SE-111; merged as `cc324f9`)
  (SG3-4) — and the refusal and preview
  text say "check `command -v <e>` in your shell; `hash -r` clears a stale hash"
  (SR-15).
- **Privacy (SR2-N6), chosen: state the limit in 06 "Privacy and
  diagnostics".** Redaction replaces the home directory and the account name
  only; every other `PATH` entry (`/pkg/env/global/bin`, a JetBrains or `nvm`
  directory) stays, as it already does in snapshots (`fixtures/snapshots/README.md`:
  "The `PATH` entries … are deliberately public"). The whole redacted `PATH` is
  kept rather than only the decision's entries because the rule reads every entry
  below `d` and `w` (relative entries included, SR-4), and a reader must be able to
  re-check it. Every user-supplied string the refusal or preview echoes (a package
  name, an executable) goes through `escape_hostile`.
- R-21 notes the authority for "a form that can write is refused": the
  `PERMITTED_SUBCOMMANDS` comment and AGENTS.md, not INV-05's letter.

### 1. The executable list (domain, F-2a) — *changed: SG-1, SR-2, SR-11*

New in `domain/src/executable.rs`:

```text
ExecutableName       a validated leaf
ExecutableList       tag "state": NOT_RECORDED | UNKNOWN { reason } | KNOWN { executables: BTreeSet<ExecutableName> }
Placement            tag "state": ON_PATH_DIR { dir } | NOT_LINKED { reason } | UNKNOWN { reason }
```

- **Name class (§3 item 10, SR-11), `ExecutableName::parse`:** non-empty, one
  path component, no `/`, not `.` or `..`, no leading `-`, at most 255 bytes,
  and **no character of any `HostileClass`** (control, format, line and
  paragraph separators — `domain::hostile_class`). Called by the adapter parsers,
  the planner and the gate; `machine.rs`'s `is_plain_executable_name` (`:580`)
  becomes a call to it, so `which` and the list share one class. Any listed entry
  that fails drops the package to `UNKNOWN { NAME_CLASS }`, never a list with the
  entry removed.
- **Placement is a neutral tri-state filled by each adapter for an installed
  package, from detection only (SG-1).** Homebrew: `ON_PATH_DIR` when
  `linked_keg` names the listed keg; `NOT_LINKED { keg_only reason }` when the
  formula is keg-only with no linked keg; otherwise `UNKNOWN`. MacPorts:
  `ON_PATH_DIR` when the port is active and its own `contents` listing places the
  executable directly under the detected `<prefix>/bin` or `/sbin` — observed,
  not inferred from a field's absence; otherwise `UNKNOWN`. An uninstalled package
  is `UNKNOWN { NOT_INSTALLED }` for **both** providers.
- **In a snapshot:** `PackageMetadata.executable_list: ExecutableList` and
  `PackageMetadata.placement: Placement`, both `#[serde(default)]` to
  `NOT_RECORDED`/`UNKNOWN`. Additive; `snapshot.v1` stays at 1 with a note,
  precedent `service` (`package.rs:138-142`). The old `executables` set is written
  from `KNOWN` for older readers and never read to decide. **Snapshot load fails
  closed** (SR-11): a key or entry that fails `ExecutableName::parse` is a load
  error, not a skipped row. **That tightens v1 for old documents (SG2-N6):** a
  v1 snapshot that loaded before can now be refused. `docs/schemas/snapshot.v1.md`
  gains a compatibility line saying so, and F-2b runs every file under
  `fixtures/snapshots/` and every snapshot the fixture matrix writes from
  `fixtures/recordings/` through the new loader
  (`every_committed_snapshot_loads_under_the_fail_closed_rule`).
- **Placement for an inactive MacPorts port** is `UNKNOWN { INACTIVE }`, and its
  list is `UNKNOWN { INACTIVE }` with no read (SR2-N5).
- **`ProviderRead::executables(&self, pkg) -> (ExecutableList, Placement)`**,
  default `(NOT_RECORDED, UNKNOWN)`.

### 2. The two reads and the argv tightening (adapters, F-2a) — *changed: SR-2, SR-7, SR-8, SR-10*

- **Argv, one rule per prefix (SR-7), its own commit.** Homebrew's `run`
  accepts `ALLOWED_ARGV` exactly, or `strip_head(argv, ARGV_LIST_FORMULA)`
  followed by exactly one element passing `crate::process::is_brew_name` — the
  spawn site's own function, not a copy. MacPorts binds `ARGV_CONTENTS` to
  exactly one `is_port_name`; `INFO_BASE` keeps its rule. **`ARGV_PROVIDES`'s
  tail rule is left exactly as it is today and is out of F (SG2-N1):**
  `is_absolute_plain_path` is private in the frozen `process/mod.rs`
  (`:367`, no `pub(crate)`), so calling it needs an edit F may not make; making it
  `pub(crate)` is a visibility-only owner patch, handed to later parts.
  `is_port_name` and `is_brew_name` are already `pub(crate)` (`:339`, `:355`).
  `strip_head` moves to an adapter-shared module (not `process/`).
- **Empty output (SR2-N1).** For both providers, exit 0 with empty stdout is
  `ProviderOutputChanged`: an installed package lists at least one file, and an
  empty answer is not evidence of "no executables". `NoExecutables` (`KNOWN{}`)
  comes **only** from a non-empty, fully parsed listing with zero direct
  `bin`/`sbin` children.
- **MacPorts read:** only for a port the inventory shows **active** (an inactive
  one is `UNKNOWN { INACTIVE }`, SR2-N5); name from the inventory. Parser (SR-2,
  SR-10): every line is two spaces then an absolute path, with no empty, `.` or
  `..` component, no trailing `/`, no NUL; anything else is
  `ProviderOutputChanged`. **A valid listing with no line under the detected
  `<prefix>/` is `UNKNOWN { NOTHING_UNDER_PREFIX }`** (SR2-N2: the round-1 claim
  that every port registers a receipt there was unverified and is withdrawn; R-24
  records what `port -q contents` prints for an installed port with no files).
  Executables: leaves whose parent equals
  `<prefix>/bin` or `<prefix>/sbin` **component-wise** (`Path::components`, never
  a string prefix); deeper paths ignored. Nothing is stat-ed or followed.
- **Homebrew read:** only for an installed **formula**; a cask answers
  `UNKNOWN { CASK_NOT_COVERED }` with no read. With several kegs, the listing is
  of the `opt`-linked one (R-21 E-9); the parser expects that keg's path — the one
  `linked_keg` names — and with several kegs and no `linked_keg` answers
  `UNKNOWN { AMBIGUOUS_KEG }` without a read (SR-10). Same line rules as MacPorts;
  executables are leaves whose parent is `<keg>/bin` or `<keg>/sbin`
  component-wise. The Cellar comes from detection (`brew --cellar`), not a
  literal (INV-20).
- **Failures (SR-8).** A provider that answered (non-zero exit, a `Registry error`,
  a timeout, a parser refusal) → `UNKNOWN { <ErrorId> }` for that package, capture
  continues. **Only an `ErrorId::Internal` returned by `self.runner.run(..)`
  while `self.runner.is_replay()`** (the recorded runner's "no recorded
  invocation", `process/recording.rs:137-138`) maps to a per-package `UNKNOWN`
  (SR2-N4). The adapter's **own** argv refusal (`run`'s allowlist check,
  `homebrew/mod.rs:313-318`) always propagates and aborts the capture, live or
  replayed: it is a defect, not an unknown. `ProviderNotFound` (the provider vanished mid-capture)
  propagates as today's detection error does.
- Lists longer than 4096 entries → `UNKNOWN { TOO_LARGE }`. Homebrew's
  `environment_info` gains `sbin` (`<prefix>/sbin`).

### 3. Capture: one resolution map, an explicit state per name (F-2b) — *changed: SG-2, SG-11, SR-1, SR-5, SR-12, SR-13*

**Which packages get the read** (cost: 1.75 s per formula, 201 formulae here):
**(B), the packages a command names** — `migrate <pkg>` (both sides of a 1→1
mapping) and `snapshot [<package>...]`; (A) every package (≈6 min) and (C)
MacPorts-all/Homebrew-named are rejected (cost; INV-17's spirit). Recorded in
SE-112. `snapshot <pkg>` (SG-11, SR-13): the user's string only **selects** an
inventory row; the argv carries the inventory's stored name; a name in no
inventory sends nothing and is reported; an argument starting `-` is never taken
as a package. It does **not** enter DP-037 advanced mode: advanced mode governs
default views and destructive workflows (INV-13), and `snapshot` is neither;
`migrate <pkg>` enters it as part C built. Test:
`analyze_and_doctor_goldens_are_byte_identical_without_named_packages`.

**One map, keyed by name, with a state (SG-2, SG-11, SR-1, SR-5):**

```text
EnvironmentSnapshot.resolutions: BTreeMap<ExecutableName, ResolutionState>
ResolutionState  tag "state": FOUND { resolution, pathIndex: Option<u32> } | NOT_ON_PATH
                 (a name absent from the map is NOT_OBSERVED)
```

- **Name class first (SR2-N5):** every name is parsed with
  `ExecutableName::parse` before `which` is called; one that fails is never
  resolved and stays `NOT_OBSERVED`, never `NOT_ON_PATH`.
- Live capture resolves every package name (as today) and every name in every
  `KNOWN` list. **Indices are computed on the unredacted `PATH`, before
  redaction** (SR2-N7). A `which` hit is `FOUND`, with the index of the first **absolute**
  `PATH` entry equal, component-wise, to the hit's parent (SR-5); a hit whose
  parent matches no entry, or matches after a relative or empty entry was skipped
  at a lower index, records `pathIndex: None`. `None` from `which` on a live
  machine is `NOT_ON_PATH`.
- **Replay records nothing for a name: `NOT_OBSERVED`**, never `NOT_ON_PATH`.
  `--from-snapshot` reads the file and re-resolves nothing.
- **Both maps are written (SG2-Q3, option (i)).** Dropping the per-provider map
  is not additive — an older reader would see no resolutions at all — so
  `ProviderSnapshot.resolutions` keeps being written **by today's rule**
  (`capture.rs:81-106`, unchanged), **derived and never read to decide**, and
  `environment.resolutions` is added beside it as an optional field. A new
  reader reads only `environment.resolutions` when it is present.
- **Fail-closed when they disagree (SG2-Q3):** on load, every `FOUND` in the
  environment map must agree (path and owner) with the provider-map entry today's
  rule would have written for that name, and every provider-map entry must have a
  `FOUND` counterpart; any disagreement is a load error
  (`a_snapshot_whose_two_resolution_maps_disagree_fails_to_load`). Hand-edited
  or mixed-build documents are refused rather than half-trusted.
- **Old snapshots** (no environment map) load the provider map into the new one
  with a deterministic precedence per name: an authoritative `Provider` owner
  first; then a non-authoritative `Provider`; then `UNKNOWN`; ties by
  `ProviderId` order. They carry no index, so they load as
  `FOUND { pathIndex: None }` — enough for history and steps 1–4, and
  `PRIMARY_UNPREDICTABLE` wherever step 6 needs an index.
- **`analysis.rs` moves to `resolution_of` in F-2b (SG2-Q3)**, with the analyze
  goldens byte-identical (the test above): analysis stops reading the provider
  map directly, so there is one accessor for every reader.
- **The `PATH` recorded** in the snapshot, the plan, history and run reports (never `doctor --report`, 06) goes
  through `redact_home` (SR-12); test
  `the_path_a_decision_used_is_home_redacted_in_plan_history_and_report`.
- `Snapshot::resolution_of(&ExecutableName) -> ResolutionState` is the only
  accessor the planner and history use.

**INV-15.** Under `--replay` every executable is `NOT_OBSERVED`, so the planner
refuses `PRIMARY_RESOLUTION_UNKNOWN`, deterministically, from the recording alone.
`invariant_15_a_replayed_reader_never_consults_the_live_filesystem` is extended
to the new reads; `invariant_15_a_replayed_capture_records_no_resolution_rather_than_none`
is new.

### 4. Planner, gate, executor (F-3) — *changed: SG-1, SG-2, SG-3, SG-4, SG-6, SG-8, SG-10, SR-2, SR-3, SR-4, SR-5, SR-6*

**Scope.** Only a 1→1 mapping is planned; 1→many and many→1 are refused
`PRIMARY_MAPPING_NOT_ONE_TO_ONE` (SG-4): which source executable answers which
destination package has no principled rule yet.

**The decision, per executable `e` in the source package's list**, from the
snapshot alone, using one `PATH` — the snapshot's — for both sides (SG-10):

1. List not `KNOWN` → `EXECUTABLES_UNKNOWN` (never the package name). `KNOWN{}`
   (only from a non-empty, fully parsed listing, SR2-N1) →
   `PrimaryDecision::NoExecutables` (SR-2): **a command-less `SET_PRIMARY`
   carrying `NO_EXECUTABLES`, completed with no dispatch, and no
   `VERIFY_RESOLUTION`** (changed by SG2-N2: round 1 said no `SET_PRIMARY` at all;
   keeping the step makes every plan carry exactly one decision, so
   `PrimaryDecisionMissing` has one meaning).
2. `resolution_of(e)` is `NOT_OBSERVED` → `PRIMARY_RESOLUTION_UNKNOWN` (SG-2).
3. `FOUND`, owner `Provider { destination, package: Some(p) }`, authoritative,
   and `p` is the plan's destination package → already wins, `NOTHING_TO_CHANGE`.
   Owned by the destination provider but another package, or `package: None` →
   `OWNED_BY_ANOTHER_PACKAGE` (SR-3).
4. `FOUND` with any non-provider owner → `NON_PROVIDER_ENTRY_WINS`; a
   provider owner that is not authoritative → **`WINNER_NOT_AUTHORITATIVE`**, a
   distinct reason (SG2-N4) so the message does not call a prefix-matched
   provider file a system tool. **Tier 4 choice (SG-8; the only Tier 4 left in
  this rule, SG3-2):** this
   refuses even though the user, today, runs the system tool and not the
   provider's copy; the alternative — leave such executables out of the primary
   set and report "stays the system's" — is recorded in SE-112 for a later part.
5. The destination's placement for `e`: `NOT_LINKED` → `PRIMARY_NEEDS_SHIM`;
   destination not installed → **DP-055**: under default (a),
   **`PRIMARY_DESTINATION_NOT_INSTALLED`** (its own id, DA-13); `UNKNOWN` for an
   installed destination → `PRIMARY_UNPREDICTABLE`. Likewise "does the
   destination ship `e`": when its list is `KNOWN` and lacks `e` →
   `DESTINATION_LACKS_EXECUTABLE`; when the destination is not installed →
   `PRIMARY_DESTINATION_NOT_INSTALLED`, **in both directions** (SG-1).
6. `ON_PATH_DIR { dir }`: `d` = index of the first absolute `PATH` entry equal,
   component-wise, to the destination's detected `bin` or `sbin` for `e` (SG-10;
   never a literal). If any relative or empty `PATH` entry sits at an index below
   `d` → `PRIMARY_UNPREDICTABLE` (SR-4). `d` absent → `PRIMARY_NEEDS_SHIM`.
   Winner `FOUND` with `pathIndex = w`: `w` missing → `PRIMARY_UNPREDICTABLE`
   (SR-5); `d < w` → predicted win, `NOTHING_TO_CHANGE`; `d > w` →
   `PRIMARY_NEEDS_SHIM`; `d == w` → `SAME_DIRECTORY`. `NOT_ON_PATH` → predicted
   win (the only "no hit" that counts, SG-2).

**Index comparison anchored on the captured winner** — a **MUST** (Tier 3) in
05, with its SE-112 reference (changed by SG3-2: round 1's "Tier 4" for the rule
itself is withdrawn), under SG-10's four conditions (one `PATH`; `d` from detection; relative entries
handled, SR-4; a different run-time `PATH` stated in the run record). It is not a
comparison of the two providers' `bin` indices: the winner is read.

**No new shadowing from the source's list (SG-8).** A predicted win only
displaces the current winner, which step 4 has already required to be a
provider; a system copy at an index below `w` would itself be the winner. So a
source-list executable never newly shadows a system tool. Where the destination
is **installed**, its own `KNOWN` list is checked too: an executable it ships
outside the source's list whose winner today is non-provider is listed in the
plan as a non-blocking risk `system_tool_keeps_name`; one the destination already
wins needs no new statement (the install predates the plan). Where it is **not
installed**, its extra executables are unknown — declared in 05 and SE-112 as the
remaining gap.

**All or nothing per package.** One executable refused refuses the plan; the
refusal lists every executable with its reason. Part of 05's MUST (SG3-2).

**Plan shape.** `SET_PRIMARY` keeps its kind, carries **no command**, and gains
`primary: PrimaryDecision` (additive in `plan.v1`; absent reads as `UNKNOWN`,
refused): `NOTHING_TO_CHANGE { executables, pathUsed }` |
`NO_EXECUTABLES` (on a command-less `SET_PRIMARY`, SG2-N2) |
`REFUSED { reasons: BTreeMap<ExecutableName, Reason>, pathUsed }`.
`pathUsed` is the redacted `PATH` (SR-6). `VERIFY_RESOLUTION` carries one
`ResolvesTo { executable, provider, package }` per executable (SR-3).
`VERIFY_DESTINATION` carries `ExecutablesOwnedByDestination { executables }` and
one `LivenessProbe` per executable; H evaluates them. `primary_after` is the
destination only when every executable is `NOTHING_TO_CHANGE`. The plan the
person confirms covers all of it: the shown plan, or under `--yes` its digest
(E-4, SE-111; merged as `cc324f9`) (SG3-4).

**Ordering (SG-4).** `check_ordering` (`plan.rs:698-706`) also requires, when
both are present, `VERIFY_RESOLUTION` before `REMOVE_SOURCE`, and at most one
`VERIFY_RESOLUTION`. Once H evaluates check 3, a destination missing an
executable fails earlier, at `VERIFY_DESTINATION`.

**Gate** (reads only the plan):
- `planner_writes_commands_on(SetPrimary)` → `false` (SG-3), so any
  `SET_PRIMARY` command is `CommandOnStepThePlannerWritesNoneOn`.
- `PrimaryNeedsShim { step, executables }`, `PrimaryRefused { step, reasons }`,
  `PrimaryDecisionMissing`, and **`ExecutableListsDisagree`** (SG-3): the
  `ResolvesTo` executables must equal the `PrimaryDecision` executables and the
  `ExecutablesOwnedByDestination` list, and **the `LivenessProbe` set plus the
  recorded skips (spec 04 check 4: "skippable with a recorded reason") must equal
  the list** (SG2-N3); every `ResolvesTo` must name the plan's destination
  provider and destination package. **Every plan with a mutating step carries
  exactly one command-less `SET_PRIMARY` with a decision, else
  `PrimaryDecisionMissing`; `NO_EXECUTABLES` comes with no `VERIFY_RESOLUTION`
  and an empty list everywhere; `NOTHING_TO_CHANGE` with exactly one
  `VERIFY_RESOLUTION`** (SR-2, changed by SG2-N2). Next ordinals in `key`;
  a **new `ErrorId::PrimaryNeedsShim`**, exit 4, added to `error.rs`'s contract
  (`:142-146`), to `docs/schemas/README.md`'s exit-code table and to
  `migrate.v1.md` (SG3-9). They fire regardless
  of allowance and before any step. Remediation names layers 1 and 2, says a
  `PATH` reorder is global priority (INV-12), a separate confirmed operation this
  release does not offer, and names `command -v`/`hash -r`.
- `evaluable_in_this_build`: `ResolvesTo` → `VerifyResolution` only.
- **One narrow change to `VerificationAssertsNothing` (SG-6):** not refused iff
  kind is `VerifyResolution`, `commands` is empty, `verification` is non-empty,
  and **every** check is `ResolvesTo`. `CheckHasNoAnsweringRead` is untouched:
  `answers` keeps `ResolvesTo` → `false`, and the rule already skips a step with
  no commands and every kind but `VERIFY_DESTINATION`/`VERIFY_REMOVAL`
  (`execution_gate.rs:766-774`, read for this revision), so it never sees
  `VERIFY_RESOLUTION`; `check_has_no_answering_read_is_unchanged` pins that.
- Every executable name in the plan re-checked with `ExecutableName::parse`.

**Executor.**
- `Disposition::NothingToChange`, only for a command-less `SET_PRIMARY` with
  `NOTHING_TO_CHANGE` or `NO_EXECUTABLES` (SG2-N2). The guard (`executor.rs:885-891`) is unchanged; nothing is
  dispatched. `outcome_of` counts it as done.
- `ResolutionSite` beside `ReadSite` in `cli/src/executor/`: per executable, a
  fresh `machine.which` and the owning adapter's `resolution()` through the
  registry context; no spec built in the shell, no new program. It re-applies the
  relative-entry rule to the run-time `PATH` (SR-4) and, if that `PATH` differs
  from the plan's `pathUsed`, says so in the run record (SG-10). **The run-time
  `PATH` is redacted with the same `redact_home` before it is compared with
  `pathUsed`** (SG2-N5), so a home directory never makes two equal `PATH`s
  differ; indices are still taken on the unredacted one.
  `application::executor` decides: pass only on owner `Provider { destination,
  Some(package) }` with `package` = the step's, **and** authoritative; else halt
  `ResolutionMismatch { executable, observed }`. Under replay or
  `--from-snapshot`: `Unevaluable`, never a pass (INV-15). `which` tests the mode
  bits, not `access(X_OK)`; noted in the code (SR-15).

### 5. History (F-3) — *changed: SG-12*

`history.v3` (approved, SG-12): `primaryAfter` is what was observed to resolve;
`primaryExecutables` sorted. `docs/schemas/history.v3.md` ships with F-3, which
emits it (SG-7). A v2 record still reads; recognition answers
`PrimaryNotConfirmed` for it, never `Recognised`. Recognition
(`application/src/history.rs:434-453`) checks every `primaryExecutables` entry
through `resolution_of`; a fresh snapshot whose list is not `KNOWN` answers
`NotRecognised::ExecutablesUnknown`. Closes SE-092 S6 "Not closed (2)" for
no-shim plans only. Restore has nothing to revert: F changes no primary.

### 6. INV-17

Same types, reads, planner rule, refusals and DP-055 default for both providers.
- **The combined outcome on the captured machine, both directions (SG3-11,
  DA-12)** (`fixtures/snapshots/both-providers-intel-macos26-live.json`,
  2026-10-28; expected from the captured resolutions, confirmed by the
  recapture): toward Homebrew, `htop`, which Homebrew already holds and wins,
  completes with nothing to change; toward MacPorts the same package is
  `PRIMARY_NEEDS_SHIM` (`/usr/local/bin` 6 before `/opt/local/bin` 25); `m4` is
  `NON_PROVIDER_ENTRY_WINS` both ways (`/usr/bin` 10 wins); and any destination
  not yet installed is `PRIMARY_DESTINATION_NOT_INSTALLED` both ways. The
  `PATH`-order asymmetry is DP-047's, a ruling **confirmed by the owner**,
  answered by part I; the uninstalled-destination refusal is DP-055's, open.
  SE-112 records both that way. A decision-rule
test exercises both directions without a permuted `PATH` (Tests).

## R-24 (F-0 if answerable read-only, else F-1)

Stub entry: `docs/spec/09-research-checklist.md`, "R-24" (open, part F; SG3-10,
DA-8); DP-055 cites it by that path.

(1) Does any allow-listed read, or one P could stage read-only, give `keg_only`
for a formula that is **not installed** (candidate `brew info --json=v2
--formula <name>`; R-21 traced only the installed branch)? And the MacPorts
counterpart: does any read-only form give an uninstalled port's file list
(`port contents` needs an installed port)? (2) What `port -q contents` prints for
an inactive port, and (3, added by SR2-N2) what it prints for an installed port
that registers no files — the case that decides whether
`UNKNOWN { NOTHING_UNDER_PREFIX }` or `ProviderOutputChanged` is the honest
answer for an empty listing. A positive answer to (1) on both sides is what would
let DP-055 be reconsidered; one side only would be asymmetric and is not taken
without the owner. If a read-only form exists it is staged as `F/06`.

## Tests — *changed: SG-2, SG-3, SG-4, SG-5, SG-6, SG-12, SG-14, SR-2..SR-13, SR-16; round 2: SG2-Q3, SG2-N2..N7, SR2-N1, N2, N4, N5*

**Where constructed rows live (SG2-N7).** The decision-rule units build package
rows in test code over the captured snapshot's `PATH`; they are never written to
a file, and nothing they build goes under `fixtures/` (§3 item 9).

**Unchanged on purpose:** `every_plan_this_planner_builds_is_refused_for_execution_in_this_build`
(`application/tests/execution_gate.rs:877`) and its fixture counterpart.

Before the recapture (constructed plans and snapshots; R-21 transcripts as the
observed shape for parser units; the existing live snapshot):

- `executable_name_refuses_a_slash_a_dot_dot_a_leading_dash_and_every_hostile_class`;
  `which_and_the_list_share_one_name_class` (SR-11)
- `one_bad_entry_makes_the_whole_list_unknown`;
  `an_empty_known_list_and_not_recorded_are_different_answers`
- `a_snapshot_with_a_bad_executable_name_fails_to_load` (SR-11)
- Parser: `a_contents_line_without_the_indent_is_provider_output_changed`,
  `a_listing_with_nothing_under_the_prefix_is_unknown_nothing_under_prefix`
  (SR-2, changed by SR2-N2),
  `macports_exit_0_with_empty_stdout_is_provider_output_changed` and
  `homebrew_exit_0_with_empty_stdout_is_provider_output_changed` (SR2-N1),
  `no_executables_only_from_a_non_empty_listing_with_no_bin_children` (SR2-N1),
  `a_path_with_an_empty_dot_or_dot_dot_component_or_a_trailing_slash_is_refused`,
  `the_parent_is_compared_by_component` (SR-10),
  `a_keg_path_outside_the_linked_keg_is_provider_output_changed`,
  `several_kegs_without_a_linked_keg_is_ambiguous_without_a_read`,
  `only_direct_entries_of_bin_and_sbin_are_executables` (both adapters)
- Argv (SR-7): `homebrew_run_admits_list_formula_with_exactly_one_name`,
  `macports_contents_is_bound_to_exactly_one_port_name`, and
  `macports_provides_tail_is_unchanged` (SG2-N1)
- `a_cask_answers_unknown_without_a_read`;
  `an_inactive_port_is_unknown_inactive_with_no_read` (SR2-N5)
- `a_name_failing_the_class_is_never_resolved_and_stays_not_observed` (SR2-N5)
- `a_replayed_missing_invocation_is_unknown_and_the_capture_completes`;
  `a_live_allowlist_refusal_is_a_capture_error` and its replay twin
  `a_replayed_allowlist_refusal_is_a_capture_error` (SR-8, SR2-N4: the adapter's
  own argv refusal aborts under replay too)
- `a_snapshot_whose_two_resolution_maps_disagree_fails_to_load`;
  `the_provider_map_is_still_written_by_todays_rule` (SG2-Q3)
- `every_committed_snapshot_loads_under_the_fail_closed_rule` (SG2-N6)
- `a_non_authoritative_provider_winner_is_winner_not_authoritative` (SG2-N4)
- `the_run_time_path_is_redacted_before_it_is_compared` (SG2-N5);
  `indices_are_taken_before_redaction` (SR2-N7)
- `liveness_probes_plus_recorded_skips_equal_the_list` (SG2-N3)
- `echoed_names_go_through_escape_hostile` (SR2-N6)
- `placement_is_filled_from_detection_for_both_providers` (SG-1)
- `invariant_20_bin_sbin_and_cellar_come_from_detection_and_the_keg_from_linked_keg` (SG-14)
- `invariant_15_a_replayed_reader_never_consults_the_live_filesystem` (extended);
  `invariant_15_a_replayed_capture_records_no_resolution_rather_than_none` (SG-2);
  `invariant_15_a_replayed_plan_is_refused_primary_resolution_unknown`
- `an_old_snapshot_loads_its_resolutions_by_the_stated_precedence` (SG-2)
- `analyze_and_doctor_goldens_are_byte_identical_without_named_packages` (SG-11);
  `snapshot_sends_the_inventory_name_and_nothing_for_an_unknown_one` (SR-13)
- `the_path_a_decision_used_is_home_redacted_in_plan_history_and_report` (SR-12)
- Decision rule, unit, constructed snapshot rows over the **captured** `PATH`:
  `a_relative_entry_below_the_destination_is_unpredictable` (SR-4),
  `a_missing_path_index_is_unpredictable` (SR-5),
  `owned_by_another_package_is_refused` (SR-3),
  `an_uninstalled_destination_is_refused_not_installed_in_both_directions`
  (DA-13),
  `a_keg_only_destination_needs_a_shim` (SG-5: **unit-only** unless the
  recapture captures a case, see below),
  `invariant_17_the_decision_rule_is_the_same_for_both_directions` (SG-14: the
  same rule over each provider's row of one snapshot, no permuted `PATH`)
- `nothing_to_change_is_carried_only_on_set_primary`;
  `no_executables_plans_a_command_less_set_primary_and_no_verify_resolution`
  (SR-2, renamed by SG2-N2)
- `verify_resolution_passes_only_on_the_destination_package_and_authoritative`
  (SR-3), `verify_resolution_halts_when_the_source_still_wins`,
  `verify_resolution_under_replay_is_unevaluable`
- Gate: `the_gate_refuses_primary_needs_shim_before_any_step`,
  `the_gate_refuses_a_plan_without_a_primary_decision`,
  `the_gate_refuses_any_command_on_set_primary` (SG-3),
  `the_gate_refuses_disagreeing_executable_lists` (SG-3),
  `the_gate_refuses_resolves_to_naming_another_provider_or_package` (SG-3),
  `the_gate_exempts_only_a_command_less_all_resolves_to_verify_resolution` and
  `check_has_no_answering_read_is_unchanged` (SG-6); `every_gate_refusal` lists
  the new variants
- `invariant_01_remove_source_after_verify_resolution` (SG-4, `check_ordering`)
- `invariant_03_a_missing_destination_executable_ends_partial_with_the_source_kept`
  — the plan **includes a `REMOVE_SOURCE`** step (under the test allowance) and the
  test asserts it never ran (SG-4)
- `invariant_11_setting_the_primary_transfers_no_ownership`
  (`application/tests/invariants.rs:809`) and
  `invariant_11_primary_selection_does_not_change_ownership`
  (`policy/tests/invariants.rs:1032`), updated for the decision shape (SG-14)
- `invariant_12_setting_a_primary_never_reorders_path`
- `no_run_writes_outside_the_state_directory_or_the_providers`, with **both
  `HOME` and `TMPDIR`** redirected (SR-16)

After the live recapture (Design 3 (B), names: the eight shared packages,
`pkgconfig`, `pkgconf`, `automake`):

- `a_plan_whose_destination_already_wins_sets_no_primary` (htop MacPorts→Homebrew)
- `invariant_17_a_plan_needing_a_shim_is_refused_before_any_install` (htop
  Homebrew→MacPorts)
- `a_name_a_system_entry_shadows_is_refused` (m4: `/usr/bin` 10 wins — step 4,
  `NON_PROVIDER_ENTRY_WINS`, in both directions; SG-5 notes m4 cannot reach the
  keg-only branch)
- Keg-only, captured if it exists (SG-5): `ncurses` MacPorts→Homebrew is expected
  to refuse with **per-executable** reasons — system-won ones
  `NON_PROVIDER_ENTRY_WINS`, and any that only MacPorts provides today (expected:
  `ncursesw6-config`) `PRIMARY_NEEDS_SHIM` from Homebrew's `NOT_LINKED`. The test
  is written only if the capture shows such an executable; otherwise the branch
  stays unit-only and the report says so. No snapshot is constructed to reach it.
- `an_executable_not_named_after_its_package_is_planned` — asserts what the
  capture shows (expected `libtool`'s `glibtool`/`glibtoolize`, `xz`'s `lzcat`)
- `invariant_01_a_real_retain_source_plan_completes_once_its_checks_are_evaluable`
  — no step removed; until H it is asserted to halt at `VERIFY_DESTINATION` and
  nowhere later
- `primary_after_is_what_resolved`
- History (SG-12): `invariant_16_a_rerun_after_a_no_shim_primary_is_recognised`
  (renamed from the design notes' `a_rerun_…`);
  `invariant_16_a_resolution_that_is_not_the_records_primary_is_not_recognised`
  (`application/tests/history_recognition.rs:453`) and
  `invariant_16_a_retained_run_whose_primary_is_the_shim_is_not_recognised_and_says_why`
  (`:485`) change to the executable list; new
  `invariant_16_a_v2_record_is_never_recognised` and
  `invariant_16_an_unknown_fresh_list_is_not_recognised`

After `F/05` is applied and the recording recaptured (SG-13):
`homebrew_contract.rs` / `macports_contract.rs` cases parse the recorded output,
and `the_recorded_listing_parses_to_the_snapshot_lists` asserts the same lists as
the live snapshot captured on the same day; the fixture matrix goldens are updated
with the diff read.

Then cut the narrowing: `mutable()` and `completable_run()`
(`crates/application/src/executor/tests.rs`, 674 and 1483; DA-14) stop deleting `SET_PRIMARY` and
`VERIFY_RESOLUTION`; they still strip the `VERIFY_DESTINATION` checks DP-051
leaves unevaluable, with a comment naming part H.

## Mutants (isolated worktree, by a subagent; removed afterwards) — *changed: SG-2, SG-3, SG-6, SR-2, SR-4, SR-7*

Slice 8 §3's:
1. A shadowing non-provider entry ignored (step 4 removed).
2. Relative `PATH` entries counted in `d` or `w`; and a relative entry below `d`
   not making the decision unpredictable (SR-4), in the planner and separately in
   `ResolutionSite`.
3. The `authoritative` test dropped — planner, and separately the executor.
4. The provider comparison negated (`d > w` predicts a win).
5. `NothingToChange` carried on every kind.
6. The replay case answering a pass.

The name class and the reads:
7. `ExecutableName::parse` admits `/`; admits `..`; admits a leading `-`; admits
   a format character.
8. A bad entry dropped instead of the list becoming unknown.
9. An unknown list treated as empty `KNOWN`; then as the package name.
10. A failed read mapped to empty `KNOWN`; a live `Internal` mapped to `UNKNOWN`;
    the adapter's own argv refusal mapped to `UNKNOWN` under replay (SR2-N4).
11. Homebrew `run` admitting zero names, two names, or `list` without
    `--formula`; `ARGV_CONTENTS` accepting zero or two names (SR-7;
    `ARGV_PROVIDES` is out of F, SG2-N1).
12. A path under `bin/x/` counted; the parent compared by string.
13. A non-empty listing with nothing under the prefix producing `KNOWN{}` instead
    of `UNKNOWN { NOTHING_UNDER_PREFIX }` (SR-2, SR2-N2).
13b. **Exit 0 with empty stdout producing `KNOWN{}`** — the vacuous
    `NoExecutables` (SR2-N1), per adapter.
13c. An inactive port read, or answered `KNOWN` (SR2-N5); a name failing the class
    recorded as `NOT_ON_PATH` (SR2-N5).

Capture and decision:
14. **Absent treated as no hit**: `NOT_OBSERVED` read as `NOT_ON_PATH` (SG-2).
15. A missing `pathIndex` treated as index 0.
16. `package: None` accepted as the destination package (SR-3).
17. All-or-nothing turned into any-of.
18. `NoExecutables` with a `VERIFY_RESOLUTION`, with a command on its
    `SET_PRIMARY`, or with no `SET_PRIMARY` at all, passing the gate (changed by
    SG2-N2: the command-less `SET_PRIMARY` is required, not forbidden);
    `KNOWN{}` producing `NOTHING_TO_CHANGE` with no check — the vacuous pass
    (SR-2).
18b. The two resolution maps disagreeing and the snapshot loading (SG2-Q3).
18c. A non-authoritative provider winner reported `NON_PROVIDER_ENTRY_WINS`
    or, worse, passing (SG2-N4).
18d. The run-time `PATH` compared unredacted against `pathUsed` (SG2-N5).

Gate (SG-3, SG-6):
19. `planner_writes_commands_on(SetPrimary)` left `true`.
20. `ExecutableListsDisagree` skipping each of its comparisons in turn (the
    `LivenessProbe` set compared without its recorded skips, SG2-N3), and the
    provider/package check on `ResolvesTo`.
21. The exemption admitting `ResolvesTo` on `VERIFY_DESTINATION`; `ResolvesTo`
    plus a command; mixed checks.
22. `PrimaryNeedsShim` refused only under an allowance.
23. `check_ordering` accepting `REMOVE_SOURCE` before `VERIFY_RESOLUTION`.
24. History recognition checking only the first executable.

## Needs the owner — *changed: SG-1, SG-7, SG-13; round 2: SG2-D1, SR2 `F/05` (b), (h)–(k)*

1. **Rule DP-055** (uninstalled destination). Default (a) is built and is now
   also the recommendation (SG2-D1). If the owner rules (b), it is one planner
   rule, the tests already written for the `PARTIAL` path, and the post-install
   shadowing halt DP-055 describes.
2. **Apply `docs/passes/16-staged/F/05-capture-executables.patch`** (SG-7). It
   must meet SR-14 and round 2:
   - (a) every name validated against the adapter classes before use;
   - (b) **changed by SR2:** a bash array built from the validated names and
     expanded as `"${names[@]}"`, one quoted `record` per name; `set -f` against
     globbing; today's unquoted `$port_names` (≈line 137) replaced by the array;
   - (c) children run with working directory `/`;
   - (d) an empty name never sent;
   - (e) formulae only from `formulae[]` of the recorded
     `info --json=v2 --installed`, ports from `-q echo active`;
   - (f) the diff checked for `redact`'s `$USER_NAME` collisions;
   - (g) a `scripts/test-gates.sh` case tying the script's argvs to the spawn-site
     allowlist (staged with it);
   - (h) **names come only from stdout already captured by `record`**, which
     keeps the raw stdout before redaction; no provider invocation runs outside
     `record`'s `env -i` — this also fixes today's direct calls at ≈line 130
     (`-q installed`) and ≈143-144 (`--version`, `version`);
   - (i) **the recorded argv is byte-identical to the adapter's** — the
     `name` field and element order — with a check that every invocation the
     adapter would make for the recorded inventory is present in the recording;
   - (j) `$out` resolved to an absolute path before any child runs in `/`, or the
     children run in a subshell;
   - (k) a per-invocation timeout, or the README states that none exists (the
     script has none today).

   Counted as applied only when `git apply --reverse --check` succeeds on
   `origin/main` (SG-13). Not blocking F-2/F-3.
3. **`F/06`, only if R-24 finds read-only forms.**
4. **An arm64 machine with both providers** (a second, real `PATH` order); the
   existing arm64 recording is derived.

**Not owner items: the recaptures.** The live snapshot is written by the binary's
own `snapshot` command, running allow-listed reads only, as part D did. Once
`F/05` is applied, the session runs `scripts/capture-fixture.sh` (the
`capture-fixture` skill permits it; the guard is on editing), checks redaction in
the diff, and updates provenance in `fixtures/recordings/README.md` (SG-13). A
refusal of either run becomes the owner item, never a reworded command.

## Handed to later parts

- **H:** `ExecutablesOwnedByDestination` and per-executable `LivenessProbe` carry
  the real list; Homebrew's `authoritative` becomes an owner query there (SG-9);
  `port -q provides` stays one path per call.
- **I:** `PrimaryDecision::Refused { NEEDS_SHIM }` with its executables is the
  offer's input; `MacportsbrewShim`, `PRIMARY_IS_SHIM`, SE-092 S8 D3 and "Not
  closed (2)" for shim plans. **The shim's target must exist under the
  destination's detected prefix and be owned by the destination** (§3 item 10) —
  the backstop against a forged executable name, e.g. one carrying a newline,
  that slipped past the name class (SR-10 residual, round 2).
- **Owner, later (SG2-N1):** a visibility-only patch making
  `process::is_absolute_plain_path` `pub(crate)`, so `ARGV_PROVIDES`'s tail rule
  in the MacPorts adapter can call the spawn site's own function. Not staged by F.
- **Later (unscheduled):** the SG-8 alternative (system-won executables left out
  of the primary set); 1→many and many→1 primaries; a loser list so spec 05's
  shadowing statement covers a destination's extra executables.
- **L:** the tripwire table gains "`SET_PRIMARY` with a command or no decision",
  `ExecutableListsDisagree` and the gate exemption.

## What this part does not do

No shim, no `PATH` or shell write, no `PATH` reorder (INV-12), no `port select`,
no cask executable list, no evaluation of H's four checks, no new program or
spawn site, no edit under `crates/adapters/src/process/` or
`data/policy/provider-commands.toml`, no change to
`THIS_BUILD_DISPATCHES_MUTATIONS`.

## Phasing as pull requests — *changed: SG-7*

| PR | Branch | Contents | Waits on |
|---|---|---|---|
| F-0 | `wo16/F-0-plan` | this plan; **SE-112, the 04/05 spec edits, and the 06 edits (the `snapshot` positionals, the `PATH` privacy limit, `snapshot <package>`), each marked "part F, not built"** (moved into F-0 by SG3-7); DP-055; the R-24 stub in `docs/spec/09-research-checklist.md` (SG3-10) | reviewers; **E-4 (PR #51) merged** before it is committed (SG3-1) |
| F-1 | `wo16/F-1-stage` | `docs/passes/16-staged/F/` (`05-capture-executables.patch`, `06-…` if any), README; docs-only | F-0 |
| F-2a | `wo16/F-2a-reads` | `ExecutableName`, `ExecutableList`, `Placement`; both parsers; **the argv tightening as its own commit** (SR-7); `machine.rs` name class | F-0 |
| F-2b | `wo16/F-2b-capture` | capture (B), `ResolutionState` map beside the provider map, the fail-closed load rule, **`analysis.rs` on `resolution_of` with goldens byte-identical** (SG2-Q3), Homebrew `sbin`, snapshot schema note and compatibility line (SG2-N6); **the live recapture as its last commit** | F-2a |
| F-3 | `wo16/F-3-primary` | planner decision, `PrimaryDecision`, `ResolvesTo`, `check_ordering`, gate, `NothingToChange`, `ResolutionSite`; **`history.v3` schema doc, the `plan.v1` notes (the additive `primary` decision and the refusal-reason tightening), `migrate.v1.md` (the new refusal values and exit-4 `PRIMARY_NEEDS_SHIM`), `docs/schemas/README.md`'s exit-code table, and the locale entries it emits** (SG3-9); the post-recapture tests; the narrowing cut | F-2b |
| F-4 | `wo16/F-4-docs` | CHANGELOG, wiki, "As built", report section; after `F/05`: the recording recapture, provenance and contract tests | F-3; `F/05` for the contract tests |

Each code PR: both reviewers on the plan delta and every commit; mutation proof —
F-2a: 7–13c; F-2b: 14–15, 18b; F-3: 1–6, 16–18a, 18c–24.

## Changes from review round 1 (binding)

Each item below overrides any text of the first draft it contradicts.

1. **SG-1 → DP-055, neutral placement.** Placement is `ON_PATH_DIR | NOT_LINKED |
   UNKNOWN`, filled from detection by each adapter; absence of a field never
   means "on `PATH`". Unknown placement and unknown "ships `e`" are one case,
   treated alike in both directions, owned by DP-055 (default (a), recommended
   (b)); SE-112 cites DP-055, not DP-047.
2. **SG-2 / SR-1 / SG-11 → one map with a state.** `FOUND { pathIndex } |
   NOT_ON_PATH | NOT_OBSERVED`; only `NOT_ON_PATH` predicts a win; replay records
   nothing; old snapshots load by the stated precedence.
3. **SG-3 / SR-9 → no command on `SET_PRIMARY`, lists agree.**
   `planner_writes_commands_on(SetPrimary) = false`; `ExecutableListsDisagree`.
4. **SG-4 → ordering, 1→1 only.** `VERIFY_RESOLUTION` before `REMOVE_SOURCE`;
   the source's list is check 3's expected set, written into 04 and SE-112;
   non-1→1 refused.
5. **SG-5 → keg-only is unit-only** unless the recapture captures a case
   (`ncurses` expected); no constructed snapshot.
6. **SG-6 → the exemption narrowed** to `VerificationAssertsNothing` alone;
   `CheckHasNoAnsweringRead` untouched.
7. **SG-7 → PR split** F-0 / F-1 / F-2a / F-2b / F-3 / F-4; staged path
   `docs/passes/16-staged/F/05-…`.
8. **SG-8 → shadowing** proved impossible for source-list executables; the
   installed destination's extra executables checked; the uninstalled gap declared;
   refusal on a system-won executable recorded as Tier 4 (the rule itself is a
   MUST, SG3-2).
9. **SG-9 → `authoritative`'s two meanings** recorded in SE-112; Homebrew's owner
   query handed to H.
10. **SG-10 → the index rule's four conditions** are requirements.
11. **SG-11 / SR-13 → `snapshot <pkg>`** selects only, no advanced mode, goldens
    byte-identical.
12. **SG-12 → `history.v3`**, tests named and renamed `invariant_16_*`.
13. **SG-13 → recapture conditions:** applied-check on `origin/main`, redaction
    diff, README provenance, the recording-equals-snapshot contract test.
14. **SG-14 → INV-11, INV-17, INV-20 tests** named; the build-refusal test
    unchanged.
15. **SR-2 → `NoExecutables`** and the prefix-less listing refusal.
16. **SR-3 → `ResolvesTo` carries the package**; `OWNED_BY_ANOTHER_PACKAGE`.
17. **SR-4 / SR-5 → relative entries and the recorded index** make a decision
    unpredictable rather than guessed; component-wise comparison.
18. **SR-6 / SR-12 / SR-15 → the `PATH` used** is in the plan (digest-bound),
    home-redacted, and the user is told to check `command -v` and `hash -r`.
19. **SR-7 → argv tail rules** via the spawn site's own functions, own commit.
20. **SR-8 → `Internal`** is per-package unknown only under replay; live aborts.
21. **SR-10 / SR-11 → parser and name-class hardening**; snapshot load fails
    closed.
22. **SR-14 → `F/05` requirements (a)–(g).** **SR-16 → `HOME` and `TMPDIR`**
    redirected in the write-boundary test.

## Changes from review round 2 (binding)

Each item overrides any earlier text it contradicts, round 1's binding list
included.

1. **SG2-D1 / SR2 (= SG N3) → DP-055 recommends (a).** (b)'s cost now includes the
   install's own side effect: linking every `bin`/`sbin` entry of an uninstalled
   destination at its `PATH` position (Homebrew's at 6/7, ahead of `/usr/bin` at
   10) can newly shadow system tools and other packages' names before anything
   verifies it, and a later halt cannot undo it (spec 05 MUST, INV-12, INV-03).
   Recommendation and default are both (a); (b), if ruled, carries a post-install
   halt before `SET_PRIMARY`. SE-112's declared gap names the side effect.
2. **SG2-Q3 → both maps written.** `ProviderSnapshot.resolutions` by today's rule,
   derived and never read to decide; `environment.resolutions` optional beside
   it; disagreement fails the load; `analysis.rs` on `resolution_of` in F-2b with
   goldens byte-identical.
3. **SG2-N1 → `ARGV_PROVIDES` out of F.** Its tail rule stays as it is;
   `is_absolute_plain_path` is private in the frozen file; the visibility patch is
   handed to a later owner patch. `ARGV_CONTENTS` and `ARGV_LIST_FORMULA` use the
   already `pub(crate)` `is_port_name` and `is_brew_name`.
4. **SG2-N2 → `NO_EXECUTABLES` keeps a command-less `SET_PRIMARY`**, completed with
   no dispatch (`NothingToChange` disposition), and no `VERIFY_RESOLUTION`;
   `PrimaryDecisionMissing`, the consistency rule and mutant 18 agree.
5. **SG2-N3 → liveness:** the `LivenessProbe` set plus recorded skips equals the
   list.
6. **SG2-N4 → `WINNER_NOT_AUTHORITATIVE`**, distinct from
   `NON_PROVIDER_ENTRY_WINS`.
7. **SG2-N5 → the run-time `PATH` is redacted the same way** before comparison
   with `pathUsed`.
8. **SG2-N6 → fail-closed loading is a v1 tightening:** compatibility line in
   `snapshot.v1.md`; every committed and matrix-written snapshot run through it.
9. **SG2-N7 → constructed rows stay in test code**, never a file under
   `fixtures/`.
10. **SR2-N1 → empty stdout is `ProviderOutputChanged`** for both providers;
    `NoExecutables` only from a non-empty, fully parsed listing with no direct
    `bin`/`sbin` child; mutant 13b and one parser test per adapter.
11. **SR2-N2 → the receipt claim is withdrawn;** a valid listing wholly outside
    the prefix is `UNKNOWN { NOTHING_UNDER_PREFIX }`; R-24 (3) observes an
    installed port with no files.
12. **SR2-N4 → only the runner's `Internal` under replay** becomes `UNKNOWN`; the
    adapter's own argv refusal always propagates; the live test has a replay twin.
13. **SR2-N5 → inactive is `UNKNOWN { INACTIVE }`;** the name class runs before
    `which`, and a failing name stays `NOT_OBSERVED`.
14. **SR2-N6 → the privacy limit is stated in 06** (the whole redacted `PATH` is
    kept, because the rule reads every entry below `d` and `w`); echoed strings go
    through `escape_hostile`.
15. **SR2-N7 → SE-112 says** the predicted-win branch is effectively unreachable
    under DP-055 (a), so `COMPLETED` comes only through step 3; indices are taken
    before redaction.
16. **SR-10 residual → handed to I:** the shim target exists under the
    destination's prefix and is owned by it.
17. **SR2 `F/05` → requirements (b) revised, (h)–(k) added.**

## Changes from the F-0 diff review (binding)

`spec-guardian` (SG3-n) and `docs-auditor` (DA-n) reviewed the F-0 diff (this plan,
SE-112, the 04/05/06 edits, DP-055, the ledger). Each item overrides earlier text.

1. **SG3-1, DA-2, DA-18 → held until E-4 merges.** F-0 is cut from `origin/main`
   and stays uncommitted until PR #51 merges (§4.1); the ledger's F row is `todo`
   with that note, E's row is `in review PR #51` with P/01 applied as `4a64aa0`
   (PR #50), and this header no longer claims a `c974d33` base.
2. **SG3-2, DA-3, DA-4, DA-6 → 05 states the full step list 1–6 as a MUST**
   (Tier 3, SE-112), naming `NOT_OBSERVED` and `NOT_ON_PATH` apart and every
   reason; "Tier 4" survives only for refusing a system-won executable; 05 says
   `COMPLETED` happens only where the destination is already installed and wins.
3. **SG3-3, DA-7 → the current build said accurately:** the gate refuses every
   planner-built plan before step 1 (`CheckNotEvaluableInThisBuild`); SE-112's
   declared items add that a real `COMPLETED` also waits on part H (DP-051); the
   06 edits are marked "part F, not built".
4. **SG3-4 → the digest is conditional:** "the plan the person confirms covers
   it: the shown plan, or under `--yes` its digest (E-4, SE-111)", once E-4 is
   merged.
5. **SG3-5, SG3-6, DA-1, DA-5, DA-16 → 04:** under DP-055 (a) a missing
   destination executable is refused before any install
   (`DESTINATION_LACKS_EXECUTABLE`, `PRIMARY_DESTINATION_NOT_INSTALLED`);
   `VERIFY_RESOLUTION` and H's check 3 are the backstop; an unknown list is
   `EXECUTABLES_UNKNOWN`; Homebrew's Cellar-rule caveat sits beside the MUST;
   `VERIFY_RESOLUTION` spelled one way.
6. **SG3-7 → the 06 edits stay in F-0**, marked not built; the Phasing F-0 row
   says so.
7. **SG3-8 → SE-112's format:** the declared items and INV-17 are Rationale
   sub-bullets, the fields in the declared order.
8. **SG3-9, DA-15 → compatibility named:** `plan.v1`'s tightening, the new
   `ErrorId::PrimaryNeedsShim` (exit 4) for `docs/schemas/README.md` and
   `migrate.v1.md`, listed in the F-3 row.
9. **SG3-10, DA-8 → R-24 stub** in `docs/spec/09-research-checklist.md`; DP-055
   cites it.
10. **SG3-11, DA-12 → INV-17:** one bullet with the combined outcome for both
    directions; DP-047 "confirmed by the owner"; DP-055 gains the MacPorts
    side-effect example and the fixture and date; "the minority of real
    migrations" replaced by its argument.
11. **DA-9** keg-only cited to the recording; **DA-10** the unreachability
    reasoning matches SE-112; **DA-13** `PRIMARY_DESTINATION_NOT_INSTALLED` is its
    own id; **DA-14** `crates/application/src/executor/tests.rs`, 674 and 1483;
    **DA-17** the design notes' path.

## As built (F-2a)

Branch `wo16/F-2a-reads`, cut from `origin/main` at `caf8799`. Three code
commits: the domain types and the shared name class; **the argv tightening on
its own** (SR-7); the two reads. `git diff --stat origin/main --
crates/adapters/src/process/` is empty, and nothing under
`data/policy/provider-commands.toml`, `scripts/`, `.claude/` or
`.github/workflows/` changed. No `brew` or `port` command was run.

- **Domain** (`crates/domain/src/executable.rs`): `ExecutableName::parse` is §1's
  class, and deserialising goes through it, so a document with a bad name fails
  to load. `ExecutableList::from_leaves` is all or nothing (`NAME_CLASS`,
  `TOO_LARGE` above 4096). `ProviderRead::executables` defaults to
  `(NOT_RECORDED, UNKNOWN { NOT_RECORDED })`. `machine.rs`'s
  `is_plain_executable_name` calls `ExecutableName::parse`; `which` is now
  stricter than before by the hostile classes and the 255-byte bound.
- **Argv** (SR-7): Homebrew's `run` admits `ALLOWED_ARGV` exactly, or
  `ARGV_LIST_FORMULA` and exactly one `process::is_brew_name`; MacPorts binds
  `ARGV_CONTENTS` to exactly one `process::is_port_name`; `ARGV_PROVIDES` is
  unchanged (SG2-N1). `strip_head` moved to `crates/adapters/src/argv.rs`.
- **Reads**: one parser for both providers, `crates/adapters/src/listing.rs`
  (line rules, component-wise parents, the SR-8/SR2-N4 failure mapping), so the
  INV-17 shape is one rule. The adapter's allowlist check runs before the runner
  is called, so its own refusal can never be mistaken for the runner's
  replayed `Internal`.

Deviations from the plan, and why:

1. **`executables` returns `Result<(ExecutableList, Placement), DomainError>`**,
   not the bare pair §1 writes. §2's failure rules need an error path: the
   adapter's own argv refusal and `ProviderNotFound` must abort the capture, and
   a bare pair cannot carry that.
2. **`Placement::ON_PATH_DIR` carries `dirs: BTreeMap<ExecutableName, String>`**,
   one directory per executable, not one `dir`. A package can ship into both
   `bin` and `sbin`, and Design 4 step 6 needs "the destination's detected `bin`
   or `sbin` **for `e`**"; a single `dir` cannot say which. A leaf found
   directly under both is `UNKNOWN { AMBIGUOUS_DIRECTORY }` for the placement
   while the list keeps the name.
3. **Two reasons not in the plan**: `AMBIGUOUS_DIRECTORY` (above) and
   `NO_LINKED_KEG`, the placement of an installed Homebrew formula that is
   neither keg-only nor linked (§1's "otherwise `UNKNOWN`" needed a reason).
   `UNKNOWN`'s `reason` serialises as either one of these identifiers or an
   `ErrorId`, untagged; `no_reason_collides_with_an_error_id` holds the two
   spellings apart.
4. **Directories in a listing are skipped**: an entry that is itself the parent
   of another listed entry is not an executable. `brew list` already omits
   directories (`find -not -type d`, R-21 E-8); `port -q contents` was not
   observed to list one, and this costs nothing if it never does.
5. **A Homebrew `linked_keg` naming no installed keg** is
   `UNKNOWN { PROVIDER_OUTPUT_CHANGED }` with no read; the keg path is built
   only from the validated installed versions, never from `linked_keg` itself.
6. **Placed in F-2b, per the phasing table and its test list**: the snapshot
   fields (`PackageMetadata.executable_list` / `placement`), the fail-closed
   load rule and `a_snapshot_with_a_bad_executable_name_fails_to_load`, and
   Homebrew `environment_info`'s `sbin` (the F-2b row names "Homebrew `sbin`";
   adding it changes snapshot output, which F-2b's goldens own). The Homebrew
   placement already uses the detected prefix's `sbin` directly. Nothing calls
   `executables` yet, so no snapshot, golden or user-visible output changes in
   F-2a, and F.md places the CHANGELOG line in F-4.

Tests added (all against captured fixtures plus, where a read is needed, one
listing invocation appended **in memory** to
`both-providers-intel-macos26.json`, shaped after R-21 E-4/E-9 - nothing under
`fixtures/` is new or changed):

- `crates/domain/src/executable.rs`:
  `executable_name_refuses_a_slash_a_dot_dot_a_leading_dash_and_every_hostile_class`,
  `one_bad_entry_makes_the_whole_list_unknown`,
  `an_empty_known_list_and_not_recorded_are_different_answers`, plus
  `a_bad_name_fails_to_deserialise_rather_than_loading`,
  `more_than_the_cap_is_too_large`, `the_serialised_shapes_are_tagged_by_state`,
  `no_reason_collides_with_an_error_id`.
- `crates/adapters/src/system/machine.rs`: `which_and_the_list_share_one_name_class`.
- `crates/adapters/src/argv.rs`, `listing.rs`: the head match;
  `a_path_with_an_empty_dot_or_dot_dot_component_or_a_trailing_slash_is_refused`,
  `the_parent_is_compared_by_component`,
  `only_direct_entries_of_bin_and_sbin_are_executables`, the indent, the
  failure mapping.
- `crates/adapters/src/{homebrew,macports}/mod.rs`:
  `homebrew_run_admits_list_formula_with_exactly_one_name`,
  `macports_contents_is_bound_to_exactly_one_port_name`,
  `macports_provides_tail_is_unchanged`,
  `an_inactive_port_is_unknown_inactive_with_no_read`, and
  `a_live_allowlist_refusal_is_a_capture_error` with its replay twin
  `a_replayed_allowlist_refusal_is_a_capture_error`, in **both** adapters.
- `crates/adapters/tests/executable_list.rs`:
  `placement_is_filled_from_detection_for_both_providers`,
  `invariant_20_bin_sbin_and_cellar_come_from_detection_and_the_keg_from_linked_keg`,
  `a_keg_path_outside_the_linked_keg_is_provider_output_changed`,
  `several_kegs_without_a_linked_keg_is_ambiguous_without_a_read`,
  `a_cask_answers_unknown_without_a_read`,
  `homebrew_exit_0_with_empty_stdout_is_provider_output_changed`,
  `macports_exit_0_with_empty_stdout_is_provider_output_changed`,
  `no_executables_only_from_a_non_empty_listing_with_no_bin_children` (both),
  `a_listing_with_nothing_under_the_prefix_is_unknown_nothing_under_prefix`,
  `a_contents_line_without_the_indent_is_provider_output_changed`,
  `only_direct_entries_of_bin_and_sbin_are_executables_for_both_adapters`,
  `a_provider_that_answered_badly_is_unknown_for_that_package_and_says_why`,
  `a_replayed_missing_invocation_is_unknown_and_the_capture_completes` (at the
  adapter: the next package is still answered; the capture loop itself is F-2b).

Left to F-2b, as their names say they test capture or loading:
`a_name_failing_the_class_is_never_resolved_and_stays_not_observed` and
`a_snapshot_with_a_bad_executable_name_fails_to_load`.

**Mutation proof, 7–13c** (run in this worktree by a script that applied each
mutant to the committed sources, ran `cargo test -p macportsbrew-domain` and/or
`-p macportsbrew-adapters`, and restored the file; not yet repeated by a
subagent in a separate worktree as §"Mutants" asks). Nineteen mutants, all
killed: 7 (`parse` admitting `/`, `..`, a leading `-`, a format character); 8
(a bad entry dropped); 9 (an unknown list made empty `KNOWN`; its "then as the
package name" half is a planner mutant, F-3); 10 (a failed read made empty
`KNOWN`; a live `Internal` made `UNKNOWN`; the adapter's own argv refusal made
`UNKNOWN` under replay, in each adapter); 11 (Homebrew admitting zero or two
names, `list` without `--formula`; `contents` admitting any tail); 12 (a path
under `bin/x/` counted; the parent compared by string); 13 (nothing under the
prefix made `KNOWN{}`); 13b (empty stdout accepted); 13c (an inactive port
read; answered `KNOWN`). 13c's second half, a failing name recorded
`NOT_ON_PATH`, is a capture mutant and moves to F-2b.

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
| `spec-guardian` | uncommitted draft on `c974d33` | Approve with changes. 1 HIGH placement inferred per provider (needs a DP); 2 HIGH "no hit" vs "never resolved", one environment map; 3 HIGH `SET_PRIMARY` commands and list consistency; 4 MED-HIGH ordering after `VERIFY_RESOLUTION`, check 3's reading, 1→1; 5 MED m4 cannot test keg-only; 6 MED narrower exemption; 7 MED PR split; 8 MED system-tool shadowing; 9 MED two meanings of `authoritative`; 10 index rule approved with conditions; 11 `snapshot <pkg>` and advanced mode, goldens; 12 `history.v3` approved, tests to name; 13 recapture conditions; 14 INV-11/17/20 tests, unchanged build-refusal test | All resolved in this revision: 1 → DP-055 and `Placement` (Design 1, 4); 2 → Design 3; 3 → Design 4 gate; 4 → Design 0, 4; 5 → Tests (unit-only unless captured); 6 → Design 4 gate; 7 → Phasing; 8 → Design 0, 4; 9 → Design 0, handed to H; 10 → Design 4; 11 → Design 3; 12 → Design 5, Tests; 13 → Needs the owner, Tests; 14 → Tests. Second round to review this revision before F-2 |
| `security-reviewer` | uncommitted draft on `c974d33` | Approve with changes; 1–4 required in the plan before F-2. 1 HIGH absent resolution predicts a win (= SG-2); 2 MED-HIGH vacuous pass on an empty or prefix-less listing; 3 MED owner must be the destination package; 4 MED relative entry below `d`; 5 MED record the index, compare by components; 6 MED `PATH` in the plan; 7 MED tail rules via spawn-site functions; 8 MED-LOW `Internal` live vs replay; 9 (= SG-3); 10 LOW parser line rules, kegs; 11 LOW `HostileClass`, shared class, fail-closed load; 12 LOW redaction; 13 LOW `snapshot <pkg>` argv; 14 INFO `F/05` (a)–(g); 15 INFO `hash -r`, mode bits; 16 `TMPDIR` | All resolved in this revision: 1 → Design 3 (with SG-2); 2 → Design 0, 2, 4, mutants 13, 18; 3 → Design 4 step 3 and `ResolvesTo`; 4 → Design 4 step 6 and `ResolutionSite`, mutant 2; 5 → Design 3, 4; 6 → Design 0, 4; 7 → Design 2, mutant 11; 8 → Design 2; 9 → Design 4 gate; 10 → Design 2; 11 → Design 1; 12 → Design 3; 13 → Design 3; 14 → Needs the owner 2; 15 → Design 0, 4; 16 → Tests. Second round to review this revision before F-2 |
| `spec-guardian`, round 2 | revised uncommitted draft on `c974d33` | Approve with changes. D1 MED (b)'s install can newly shadow names; Q3 dropping the provider map is not additive; N1 HIGH `is_absolute_plain_path` is private in the frozen file; N2 MED keep a command-less `SET_PRIMARY` for `NO_EXECUTABLES`; N3 liveness set plus skips; N4 distinct non-authoritative reason; N5 redact the run-time `PATH` before comparing; N6 fail-closed load tightens v1; N7 constructed rows stay in code | All resolved: D1 → DP-055 now recommends (a), Design 0's gap, Needs the owner 1; Q3 → Design 3 option (i), F-2b; N1 → Design 2, Handed to later parts; N2 → Design 0, 4, gate, mutant 18; N3 → gate, mutant 20; N4 → Design 4 step 4, mutant 18c; N5 → Executor, mutant 18d; N6 → Design 1, Tests; N7 → Tests preamble. See "Changes from review round 2 (binding)" |
| `security-reviewer`, round 2 | revised uncommitted draft on `c974d33` | Approve with changes. N1 MED-HIGH empty stdout must not yield `NoExecutables`; N2 receipt claim unverified; N3 (= SG2-D1); N4 only the runner's `Internal` under replay; N5 inactive ports, name class before `which`; N6 `pathUsed` privacy limit, `escape_hostile`; N7 predicted-win unreachable under (a), indices before redaction; SR-10 residual for I; `F/05` (b) revised, (h)–(k) | All resolved: N1 → Design 2, Tests, mutant 13b; N2 → Design 2, R-24 (3), mutant 13; N3 → DP-055; N4 → Design 2, Tests (replay twin), mutant 10; N5 → Design 1, 2, 3, mutant 13c; N6 → Design 0, F-2b; N7 → Design 0, 3; SR-10 → Handed to I; `F/05` → Needs the owner 2 |
| `spec-guardian`, F-0 diff | uncommitted F-0 on `wo16/F-0-plan` (from `origin/main`) | Approve with changes, 11 findings: held until E-4; 05 step list and tier; current build; digest conditional; 04 refusals before install and the backstop; 06 placement; SE-112 format; compatibility; R-24 stub; INV-17 wording | All resolved: see "Changes from the F-0 diff review (binding)" items 1–10 |
| `docs-auditor`, F-0 diff | uncommitted F-0 on `wo16/F-0-plan` (from `origin/main`) | 18 findings: ledger and header (2, 18); 05 (3, 4, 6); current build (7); 04 (1, 5, 16); R-24 (8); keg-only citation (9); unreachability wording (10); INV-17 (12); own id for not installed (13); test path (14); compatibility (15); design-notes path (17) | All resolved: same section, items 1–11 |
