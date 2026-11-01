# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Two version numbers matter and they move independently:

- the **tool version**, which follows semantic versioning; and
- the **policy version**, the identifier stamped on every recommendation and
  carried in every `--json` payload. Its format is
  `<newest read_on across data/policy/>-<serial within that day>`
  (`data/policy/policy.toml`). Dated facts about Homebrew and MacPorts live in
  that directory, so a change to what the tool believes about a provider moves
  the policy version without necessarily moving the tool version.

The `--json` output is a compatibility surface: each payload carries a
`schemaVersion`, and changing a schema incompatibly requires a version bump and
an entry in [`docs/spec-evolution.md`](docs/spec-evolution.md). Human-readable
output may change freely.

## [Unreleased]

These changes came after the 0.1.0 release candidate and are on `main`; the
0.1.0 tag is cut from `main`, so they ship in 0.1.0 and the 0.1.0 section below
describes the ones a user sees. (DP-045, ruled
2026-10-23: they ship in 0.1.0.)

### Changed after the release candidate

- `migrate --json` now writes `planDigest`, and `migrate` (rendered) prints
  it after the plan: `sha256:` and 64 lowercase hexadecimal digits, taken over
  the plan itself with the build's version in the input. `migrate --yes
  --plan-digest <d>` runs only the plan whose digest is `<d>`, so a script
  runs exactly the plan it was shown; any other digest stops before the gate
  with `PLAN_NOT_SHOWN` (exit 4) and nothing on stdout, in every build. A
  malformed digest, or `--plan-digest` without `--yes`, is a usage error
  (exit 2). The digest fingerprints unredacted data: remove it before
  publishing a document. New dependency: `sha2` (RustCrypto, `MIT OR
  Apache-2.0`), which brings seven new small crates (`cfg-if` was already
  present), `libc` among them on Apple Silicon only (ADR-0007, SE-110,
  SE-111).
- `migrate --yes`, once a build dispatches mutations: the plan the gate cleared
  is shown on standard error and runs only after a person types
  `run plan <code>`; `--yes` never answers that question. New error
  identifiers `USER_ABORTED` (exit 6) and `PLAN_NOT_SHOWN` (exit 4) (SE-111).
- `migrate --yes`, once a build also admits elevation (part J): an operation
  that needs administrator rights is asked about after the plan is confirmed,
  under the run lock and before anything is written; declining ends the run
  with `USER_ABORTED` (exit 6) and nothing changed, and the machine is read
  again after the answer. Not reachable in this build: the gate refuses every
  elevated step (SE-111).
- `migrate --json`: a configuration re-check now records the program it was
  made for, and a step it would be spent on that names another program, or no
  program, halts with the precondition `WRONG_PROGRAM_TOKEN`. The elevation
  consent prompt (not yet reachable) asks for `allow step N <code>`, with a
  code drawn fresh for each question (SE-111).
- `migrate --json`: a destination check (`VERIFY_DESTINATION`) is now read only
  after its provider's configuration was read again and re-checked; a
  configuration that cannot be read again, or has changed, halts the step before
  the read (`PRECONDITION_FAILED`, `CONFIGURATION_UNREADABLE` or
  `CONFIGURATION_DRIFT`). Under `--replay` the configuration cannot be read
  again, so such a step halts. The gate also refuses a check none of its step's
  commands can answer (`CHECK_HAS_NO_ANSWERING_READ`). The gate still refuses
  every plan this build makes, so no run reaches these steps yet. Behind the
  switch, which stays off: the destination is read again immediately before each
  removal command (`DESTINATION_NOT_REVERIFIED`). Every `VERIFY_DESTINATION` in
  `plan --json` now lists a second command, the read that shows the package
  active (`brew info --json=v2 --installed`, `port -q installed`), and the
  check it answers, `PROVIDER_REPORTS_ACTIVE`, is evaluated from that read, in
  both directions. Without `PACKAGE_NEWLY_PRESENT` on the same step it certifies
  only a package the destination already held before the run; otherwise the
  step halts `UNEVALUABLE` (`ACTIVE_ALONE_AFTER_AN_INSTALL`, or
  `PRESENCE_BEFORE_UNKNOWN` when that cannot be told). A Homebrew cask, which
  records no linked keg, never passes it (work order 16 part S; SE-107). The policy data version is
  `2026.10.18-1`.
- `doctor`: says, per provider, whether the reads this tool asks it for may
  reach the network on the provider's own account (its local metadata index is
  missing), are answered offline, or cannot be told from this input; and names
  a configuration setting that decides whether the provider sends its own
  analytics (`brew.env`'s `HOMEBREW_NO_ANALYTICS`; MacPorts 2.12.6 has none,
  observed 2026-10-27, R-16 addendum). The
  charter's no-network and no-telemetry non-goals now say exactly this
  (DP-028).
- `plan`, `migrate` and `restore --plan`: a destination that records the
  package as not available, not supported on this platform, or disabled now
  makes the plan a manual review (`destination_artifact_unavailable`, exit 4),
  in either direction. No provider reports the first two states today, so no
  real plan changes yet; a package that only builds from source, or whose
  availability is unknown, is still planned (DP-050). See SE-107.
- `--help`: the top-level line for `history` no longer says it "Reads only". It
  reads MacPortsBrew's own state directory, and `history acknowledge <id> --yes`
  writes there (it makes a run's record durable and removes the interrupted-run
  marker). Behaviour is unchanged; only the help was wrong.
- `--dry-run` now writes nothing: `snapshot` and `doctor --report` say where
  they would have written instead of writing. Under `--replay` or
  `--from-snapshot`, `snapshot` needs `--out` (exit 2), so another machine's
  capture is not filed in the state directory by default.
- `status`'s ownership line, and `summary.ownershipUnknown` in the analysis
  `--json` payloads: the count of rows whose ownership is unknown now takes
  installed rows only (the value is narrower; `schemaVersion` is unchanged). A row that names a mapping
  target nobody installed is no longer counted as a provider that "kept no
  record". The field is now documented in `docs/schemas/analysis.v1.md`.
- `migrate --json`: the execution gate now also refuses `PROVIDER_REPORTS_ACTIVE`
  as a check this build cannot evaluate. It used to be answered by a read that
  shows a package is installed, which is not the same as active. Every plan was
  already refused for execution; the refusal list gains one entry.

- `restore --run <id>`, with or without `--plan`: a run this machine never
  recorded is now answered `RUN_NOT_FOUND` (exit 8) from the state directory
  before any provider is asked. It used to capture the machine first: on an empty
  Homebrew cache that made Homebrew download its catalogue into
  `$HOME/Library/Caches/Homebrew` (731 files where it was found) only to say there
  was no such run. Nothing else about the command changes; `history show` never
  did this.

### Added after the release candidate

- `macportsbrew history acknowledge <id>`: deals with the interrupted-run
  marker a run left. Without `--yes` it says what it knows and changes nothing;
  with it, it makes the run's record durable, checks it reads back, and removes
  the marker (a record it cannot read keeps the marker). `doctor` now reports an
  interrupted run (the `interrupted_run` finding, always present; `OK` when
  there is none or when it was not checked), and every run that handed a
  mutating step over prints its snapshot and the command that plans the way
  back. The by-hand "remove interrupted-run.json" remedy is gone from the
  messages. `--json`: `history-acknowledge.v1`.
- `macportsbrew restore --plan --run <id>`: the plan that puts back what a
  recorded run changed, built by the same planner in the other direction from
  the run's record and the snapshot it took first, shown with notes about what
  it cannot say. Nothing is executed: `restore --run <id>` without `--plan`
  puts a recorded run's plan to the execution gate, which refuses it (exit 4)
  (with no record of that run it answers `RUN_NOT_FOUND`, exit 8, first), and
  `restore --snapshot` is not built (exit 1). `--json`: `restore.v1`.
- `macportsbrew history` and `history show <id>`: the runs this MacPortsBrew has
  made, newest first, and one run's steps, snapshot and whether it can be put
  back (restorability is derived when read, never stored). Read-only, creates
  nothing, refused under `--from-snapshot`, under a replay and as root. A new
  error identifier, `RUN_NOT_FOUND` (exit 8), for a well-formed run id with no
  record. `--json`: `history-list.v1` and `history-show.v1`.
- `migrate --json`, additive and reachable only under the mutation switch that
  no build turns on: the optional `historyError` and `markerLeft` execution
  fields, the `PERFORMED` step disposition with `ends`, and the halt reasons
  `MUTATION_FAILED`, `MUTATION_REFUSED`, `MUTATING_STEP_CARRIES_VERIFICATION`,
  `PRECONDITION_FAILED` and `STATE_CHANGED`; `history.v1` gains `PERFORMED`.
  Consumers that match these values exhaustively should accept them.
- `migrate --json`, additive and reachable only under the same switch: the
  optional `runId`, `snapshotFile` and `recordReadBack` execution fields, the
  `SNAPSHOT_TAKEN` and `RECORD_PENDING_READ_BACK` step dispositions, the
  `SNAPSHOT_NOT_TAKEN` halt reason, and the `RECORD_NOT_CONFIRMED` reason in
  `retainedRunNotRecognised`.
- A run's snapshot and record are now performed by the mutating loop (still
  behind that switch): the snapshot is written first, from the capture taken
  under the lock, and read back before anything changes; the run's record is
  stored pending and is confirmed by a read-back. History schema 2
  ([`docs/schemas/history.v2.md`](docs/schemas/history.v2.md)) and marker
  schema 2 ([`docs/schemas/run-marker.v2.md`](docs/schemas/run-marker.v2.md))
  replace version 1 of each, which no build ever wrote.

### Under the hood, after the release candidate

- The mutating run loop is wired behind a switch this build does not turn on:
  run lock, interrupted-run marker, a fresh capture and re-plan under the lock,
  a configuration re-check before each change, and a durable history record on
  every ending. The elevation consent prompt is built and not wired. Nothing a
  user can run changes a machine.

## [0.1.0] - not yet tagged

_The owner adds the date when the release is tagged._

### MacPortsBrew 0.1.0 at a glance

The first public release. MacPortsBrew looks at what Homebrew and MacPorts
have installed on your Mac, compares them package by package, explains which
provider suits each package on this machine and why, and builds a migration
plan in either direction — Homebrew to MacPorts or MacPorts to Homebrew, with
the same planner and the same safety rules.

**What it does:** `status`, `analyze`, `list`, `explain`, `plan`, `migrate`
(shows the plan), `doctor` (prefixes, `PATH` order, leaked build variables,
provider health, a package manager installed in more than one place, which
provider is exposed to the other, an interrupted-run marker, and dated support
facts), `snapshot`, and `history`, `history show`, `history acknowledge`
and `restore --plan --run`, which only read, or change nothing, on a machine that
has recorded no run. Every command has a
versioned `--json` form. `doctor --report` writes a diagnostic file for issue
reports, without your user name or home folder path. `PATH` entries a finding
mentions appear with your home folder shown as `~`; other environment variables
are named but their values are left out. It also lists every installed package.
Read it before you share it. `doctor --report` and `snapshot --out` replace an
existing file only when it is itself a MacPortsBrew document. `--dry-run` writes
no file at all: `snapshot` and `doctor --report` say where they would have
written.

**What it does not do:** it changes nothing in either package manager. It does
not install, remove or switch which copy of a tool runs, and it neither runs a
migration nor restores one. The history commands read a record that only an
executing release writes, so on every machine running 0.1.0 they say there is
nothing yet: `history` answers "No runs are recorded" (exit 0); `history show`
and `restore --plan --run` answer `RUN_NOT_FOUND` (exit 8) for any well-formed
run id; `history acknowledge` says no interrupted run is recorded (exit 0); and
`doctor`'s `interrupted_run` finding is `OK`. `restore --run` without `--plan`
answers `RUN_NOT_FOUND` (exit 8) the same way; the execution gate's refusal
(exit 4) applies only to a recorded run, so no 0.1.0 user meets it. And
`restore --snapshot` is not built (exit 1).
`migrate --yes` puts a plan to an execution gate that refuses every plan this
version produces (exit 4). `migrate` prints the plan's digest, and
`migrate --yes --plan-digest <digest>` puts the plan to the gate only if it is
still the one that digest names; a plan that changed is refused with exit 4 and
nothing on stdout. A plan lists the commands a migration would run, in
order, with how to check each one, so you can carry it out yourself at your own
risk — take `macportsbrew snapshot --out before.json` first. It never asks for
an administrator password and makes no network calls of its own (a Homebrew
read on an empty cache downloads Homebrew's catalogue). It refuses to run as
root. Every command a plan shows is quoted so it can be pasted into a shell as
written. A misspelt `--to`, `--provider` or `--today` is an error naming the
valid values, not an empty answer.

**When it cannot tell, it says so.** A package is called an orphan only when
the provider could say what depends on it; MacPorts deprecation status, which
0.1.0 does not read, is shown as unknown rather than as "not deprecated".

**Mapping looks only at what is already installed.** In 0.1.0 a package is
matched to the other provider only if that provider already has an equivalent
installed, or the curated table (26 rows; 19 of them for versioned runtimes, GNU
tools and libraries whose names differ) names one. For anything else the plan
stops at *Needs a human decision* with "no equivalent found" — nothing was
found, which does not mean no equivalent exists. Searching the other provider's
full catalogue is planned for a later release.

**Machines:** macOS 10.12 or later on Intel and 11.0 or later on Apple
Silicon. macOS 15 and 26 are covered by the CI configuration; 10.12–14 are
best-effort — built for and checked to declare that minimum, not exercised.
See [`docs/support-matrix.md`](docs/support-matrix.md).

**Languages:** English, and Brazilian Portuguese (`--language pt-BR`, or
`MACPORTSBREW_LANG`, `LC_ALL`, `LANG` in that order). The Portuguese was drafted
by a model and reviewed by others, not by a native speaker; `--json`, error
identifiers, flags and command names stay English.

**Policy data:** version `2026.10.18-1`. Dated provider facts — Homebrew's
support tiers and Intel timeline among them — carry their source and the date
they were read, and the tool says when the newest of them (a row read from a local index counts
at that index's date) is more than 180 days old.

### Development history (pre-release)

The detailed record of how 0.1.0 was built, kept for contributors. It uses the
project's internal vocabulary (passes, slices, decision and specification
identifiers); the summary above is the user-facing account.

**Policy version: `2026.10.18-1`.** The provider facts it carries were read on
2026-09-02, 2026-09-04, 2026-09-06, 2026-09-08, 2026-09-19, 2026-10-02,
2026-10-03, 2026-10-12 and 2026-10-18 and are dated in `data/policy/*.toml`; the staleness threshold is
180 days, after which the CLI says the data may be out of date without refusing
to answer.

#### Pre-release review (SE-094)

- **Fixed:** `ORPHAN` without readable reverse dependencies (now `DEPENDENCY`);
  intent read from the active installation; MacPorts
  `DEPRECATION_STATE` is `UNKNOWN`; an unfollowable MacPorts link is unresolved;
  MacPorts tuples whose variant ends in `_<digits>` parse; `.DS_Store` in a
  launchd location is not a startup item; root refusal at startup; shell-quoted
  plan commands; usage errors for unknown `--to`/`--provider` and malformed
  `--today`; `snapshot --out`/`doctor --report` replace only their own
  documents; standard output flushed before exit; crates `publish = false`.
- **Recorded as owed** (the gate half of `PROVIDER_REPORTS_ACTIVE` was done after
  the release candidate; see [Unreleased]): `PROVIDER_REPORTS_ACTIVE` asserting
  "active", a plan's rollback field (see SE-094). The dual-Homebrew `doctor` check it listed is done
  in pass 12, below.

#### Pass 12: read-only improvements after the release-candidate review

All read-only: nothing here makes the tool change a machine, and nothing widens
what it may run.

- **A Brazilian Portuguese translation.** `--language pt-BR` (or
  `MACPORTSBREW_LANG`, `LC_ALL`, `LANG`, in that order) shows the tool in
  Portuguese, falling back to English for anything missing. `--json`, error
  identifiers, flags and command names stay English. It was drafted by a model
  and reviewed by others, not by a native speaker, so the wording of the error and
  `doctor` sentences may need correcting; reports are welcome (DP-040). `es`, `ja`
  and `zh-CN` are not written yet.
- **`doctor` reports a package manager installed in more than one place.** On an
  Apple Silicon Mac migrated from an Intel one, a working Homebrew can survive
  under the old prefix; every standard prefix is now searched, and `doctor` names
  the places and which copy this tool runs (a warning, never a change). When the
  answer is not known - a recording, or a snapshot taken before this existed - it
  says so instead of saying "one" - and so it does for a provider found at a prefix
  none of the standard ones names. The snapshot carries the answer as
  `environment.installations` (additive; `schemaVersion` is unchanged).
- **`explain` says when an ownership answer was a default.** When a provider could
  not say what depends on a package nobody asked for, it is shown as a dependency,
  never an orphan; `explain` and `--json` (`ownershipSplitDefaulted`) now say that
  was a default and not a finding.
- **Nineteen more curated package mappings**, in both directions, for versioned
  runtimes (`python@3.13`, `node@24`, `php@8.3`, `ruby@3.3`, PostgreSQL and MySQL
  server/client pairs, ...), `g`-prefixed GNU tools and `lib`-prefixed libraries.
  Each row was observed on 2026-10-12 in both providers' own local indexes, which
  were themselves as old as 2026-09-02 (Homebrew's formula checkout) and
  2026-09-06 (MacPorts' ports tree); each row says so, and none is proof against a
  fresh sync.
- **`--version` carries the CI build number when it has one.** A fifth line
  `build <n>` appears only for a build compiled with `MPB_BUILD_NUMBER` set to a
  positive decimal integer (the CI run's sequential number, never repeated; it is
  not part of the semantic version). A malformed value stops the build at compile
  time. A developer build prints the four lines as before. The release workflow
  and script are not in the repository yet (staged for the owner), so nothing sets it yet.

**Changed**

- **Policy data is version `2026.10.12-4`.** A curated row may carry `index_as_of`,
  the date of the index it was read from, and the "may be out of date" notice
  measures from that date, so rows read from an older local index do not make
  older facts look newer (DP-043).
- The layering test that keeps the command-line crate free of provider names now
  also refuses a path into a provider's adapter module, and the network-isolation
  test is named for what it checks.

**Known, not changed** (the `status` count below was changed after the release
candidate; see [Unreleased])

- `status` counts a mapped counterpart that is not installed among the rows whose
  ownership is unknown, and its sentence says the provider kept no record.
- The two owed items of the pre-release review that this does not close: a plan's
  rollback field, and `PROVIDER_REPORTS_ACTIVE` asserting "active" (SE-094).
- `doctor --deep` remains owed (the `PATH` wording DP-041 asked for is pass 13, below).

#### Pass 13: which direction of cross-provider contamination is the riskier one

- **`doctor` says how each provider's own tooling guards its builds against the
  other's prefix.** For every ordered pair of present providers there is one
  `cross_provider_exposure` finding (`--json`: `doctor.cross_provider_exposure`, with
  `affected`, `intruder`, `intruder_prefix` and `defence`). It is a warning only when
  `PATH` is known and the other provider's directory is searched first, or yours is not
  on it while theirs is; a recording carries no `PATH`, so it is `OK` there. The sentence differs by
  direction, because the advice is dated data
  (`data/policy/cross-provider-advice.toml`), not code: MacPorts' own guide suggests trying
  trace mode (`port -t`) for conflicts with files in places such as `/usr/local`, and
  trace mode is off unless asked for (MacPorts 2.12.6, read 2026-10-12); Homebrew 7.0.4 builds a formula
  with a `PATH` it constructs itself, so a directory on yours is not an input, unless a
  formula asks for yours (`env :std`) or the step runs outside a build (`post_install`).
  A provider the data does not describe is reported as such, never as safe. Report only:
  nothing is changed.
- **A correction to an earlier reading.** The first research note said Homebrew's
  compiler shim drops `-I`/`-L` flags under `/opt/local` on every source build. Re-reading
  Homebrew 7.0.4 showed it does so only when argument refurbishing is on, which is not the
  default on macOS; the note, the specification and the data now say what the source
  shows (SE-098).
- **Policy data is version `2026.10.12-4`**, and `policyReadOn` is 2026-09-17, the date
  of the Homebrew checkout the new rows were read from.
- Still owed: a check of what `/usr/local` holds, `doctor --deep`, and what `brew`'s own
  non-build subprocesses do with `/opt/local` on `PATH`.

#### Added

- **`macportsbrew --version` names what ran.** Four lines: the version, the
  policy data version, the target triple and the commit (`unknown (not a release
  build)` for a developer build). `-V` still prints `macportsbrew <version>`.
- **MacPorts can be a plan's source on an ordinary machine, and a service it
  cannot attribute is never read as "no service" (pass 08, slice 3).** The
  MacPorts service scan used to answer "unknown" whenever no port had ever
  installed a launchd item — the usual state — so every MacPorts→Homebrew plan
  was refused on a live machine; and it skipped any item not named
  `org.macports.<port>` (`dbus`'s), reporting a service port as having none, so
  INV-19 did not force its manual review. It now reads both launchd locations,
  attributes each item to the port MacPorts' own registry says installed it
  (`port provides`), not to the port its name resembles, answers "none" only
  under a real MacPorts prefix with every port active, and answers "unknown" —
  refusing the plan — for anything it cannot attribute. One scan per capture
  feeds both the capability and every package's answer. MacPorts 2.12.6,
  read 2026-10-10 (`docs/research/R-8-stateful-and-service-packages.md`,
  addendum).
- **Provider arguments are pinned by count and class at the spawn site.** A
  MacPorts pseudo-port (`installed`, `all`, even `installed@1`), a bare variant
  or version, the `and`/`or`/`not` operators, `port info` without `--index`
  and a Homebrew name ending in `.rb` are refused before any provider runs; the
  planner's own reserved-name guard, which `installed@1` used to pass, is fixed
  (MacPorts 2.12.6 and Homebrew 7.0.4, read 2026-10-10; spec 04, Process
  execution).
- **The provider configuration is re-checked before each mutating step**, as a
  function the executor shell must call and hand to the step it guards. Not yet
  wired (slice 4); a build that turns mutations on without it fails its tests.
- **The single-instance run lock and the interrupted-run marker (pass 08,
  slice 2).** An exclusive advisory lock on `run.lock` in the state directory
  (a second run is `RUN_LOCK_HELD`, exit 7; a killed holder frees it), and an
  `interrupted-run.json` naming a run that began changing packages and was
  never accounted for (the next mutating command refuses, `RUN_INTERRUPTED`,
  exit 4, naming the run). Shape in `docs/schemas/run-marker.v1.md`. **Nothing
  in this build takes the lock or writes the marker**, because nothing
  dispatches a mutation; tests fail the build that turns mutations on without
  wiring both, in order. **The detector for a concurrently running `brew` or
  `port` is not built** (R-17, DP-030): every live run is still flagged
  possibly inconsistent, and a test fails the same build while it says so.
  `HISTORY_UNUSABLE`'s message now covers the lock and the marker too.
- **The migration history store (pass 08).** One file per mutating run in the
  state directory's `history/`, published so that no record is ever replaced,
  and read back without trusting it: a record that disagrees with itself, or
  anything the store would not have written, is the new `HISTORY_UNUSABLE`
  (exit 8) rather than "no record" (`docs/schemas/history.v1.md`). Nothing in
  this build writes a record, because nothing dispatches a mutation.
- **`migrate --retain-source` consults this machine's history; recognising a
  re-run is not yet reachable.** On a live run only, when this machine's
  history and a fresh capture agree, it would answer `retainedRunDone` (exit 0)
  instead of planning again; when history has something to say that is not
  enough, the plan carries `retainedRunNotRecognised` with the reason. Additive to
  `migrate.v1`. Until shims record which provider they point at, a real re-run
  is still planned again, with `PRIMARY_IS_SHIM`.

- **A provider's own configuration is detected and reported (DP-027).** Capture
  records which Homebrew `brew.env` files and which MacPorts user
  `macports.conf` exist and which settings they set — names only,
  never values — from dated data (`data/policy/provider-configuration.toml`).
  `doctor` reports it per provider, a plan carries a non-blocking
  `provider_configuration` risk, and the execution gate refuses any step that
  would change such a provider (`PROVIDER_CONFIGURATION`). New optional
  `configuration` field in `snapshot.v1`; a replay records `UNKNOWABLE`.
- **`explain --json` and `doctor --json` carry `concurrentProviderActivity`**,
  and their human output ends with the same sentence as `plan` and `analyze`.
- **`migrate`'s `nothingToMove` carries `notDetected`** — the other providers
  that were not detected; the answer is *nothing to move*, exit 0, where it
  used to refuse (DP-026).

- **`migrate <pkg> --to <provider> [--retain-source] [--yes] [--dry-run]`.**
  It shows the plan `plan` would, and with `--yes` (without `--dry-run`) puts it
  to the execution gate, which refuses every plan this build produces: exit 4
  for every plan it builds, the refusals in the output, nothing run. A package
  already at its destination is reported as *nothing to move*, exit 0, unless
  something its mappings name may be the package under another name
  (`SAFETY_BLOCKED`, exit 4). Where another provider was not detected, the
  answer says so and `nothingToMove.notDetected` names it, because a provider
  installed where detection does not look would read the same (DP-026). The
  planner's own refusals keep their codes.
  `--yes` with `--from-snapshot` is a usage error. New `--json` schema:
  [`migrate.v1.md`](docs/schemas/migrate.v1.md).
  Every `migrate` output that shows a plan or *nothing to move* ends by saying
  whether a package manager may have been changing the machine while it was read (`concurrentProviderActivity`): a live
  run says it was not checked, and a run from `--replay` or `--from-snapshot`
  says it cannot be known. Both are flagged as possibly inconsistent; this build
  has no check that could say otherwise.
- **`plan` and `analyze` carry the concurrency flag too.** `analysis.v1` gains
  a root field `concurrentProviderActivity` — so `analyze --json`,
  `status --json` and `list --json` all carry it — and `plan --json` gains the
  same field at its root, written after `dryRunOnly` so every earlier byte
  keeps its place. Both are additive, `schemaVersion` stays 1, and a consumer
  reads the field's absence (an older build's document) as possibly
  inconsistent. The object is the one `migrate` writes, from the same source,
  through one shared type. The human output of `plan` and `analyze` now ends
  with the sentence `migrate`'s does; `status` and `list` carry it only in
  `--json`.
  `migrate`'s embedded `plan` does not gain the field: it is written once, at
  `migrate`'s root.
- **`doctor` reports a provider whose recorded prefix is not absolute**
  (`provider_prefix_is_not_absolute`, `WARNING`). A provider installed under the
  user's home is captured as `~/...`, which `planner::command` refuses, so
  analysis is correct and no step that would run that provider can be built. The
  finding says so in the diagnosis instead of leaving the user to meet it as a
  refusal that blames their input. `ErrorId::InputUnusable` (exit 8) is the
  identifier those refusals now carry, replacing a snapshot-specific remediation
  that was wrong on every route but `--from-snapshot`.

- **Domain layer** (`crates/domain`): provider-neutral types for packages,
  ownership, package classes, artifact availability, mappings, resolutions,
  support state, snapshots, migration plans and the error taxonomy. No I/O, no
  clock, no provider name in any signature.
- **Policy engine** (`crates/policy`): named, individually testable rules
  evaluated in identifier order with priority-based resolution and identifier
  tie-breaks. `MANUAL_REVIEW` cannot be outvoted. No rule names a
  provider; everything a rule knows about one comes from `data/policy/*.toml`
  with a source and a read date.
- **Application layer** (`crates/application`): capture, analysis,
  classification, mapping, planning, explanation and `doctor`. Analysis is a
  pure function of a snapshot and the policy data, so `--from-snapshot` runs
  the same code as a live run.
- **Adapters** (`crates/adapters`): read-only Homebrew and MacPorts adapters
  over a single process executor, a provider registry, snapshot persistence and
  machine/`PATH` inspection. The only layer that touches the machine.
- **CLI** (`crates/cli`): `status`, `analyze`, `list`, `explain`, `plan`,
  `doctor` and `snapshot`, each with `--json`; the English message catalogue,
  the pseudolocale (`--language qps`), and a display-width-aware table
  renderer.
- **Commands and aliases**: `macportsbrew` and `mpb` from one binary.
- **Versioned JSON schemas** documented in
  [`docs/schemas/`](docs/schemas/README.md): every `--json` payload at
  `schemaVersion` 1, the history record and run-marker files at 2.
- **Fixture replay and the golden matrix**: `--replay <recording>` (a hidden
  testing flag, like `--today`) runs the whole program against a captured
  recording; `scripts/capture-fixture.sh`
  produces one; `scripts/run-fixture-matrix.sh` runs the read slice over every
  recording and compares the analysis byte for byte against
  `fixtures/golden/`, with `--update` to regenerate. The shipped recordings
  cover Homebrew only, MacPorts only, both, neither, a broken provider, and a
  deprecated formula with a third-party tap, across Intel and Apple Silicon and
  across older and current macOS.
- **Policy data** in `data/policy/`: curated mappings, provider support states,
  bottle tags, provider commands and elevation, runtime managers and stateful
  package families — every entry with its `source` and `read_on`.
- **Validation**: `scripts/validate.sh` in three modes, plus layering, SPDX,
  dependency-licence, i18n-parity, coverage and deployment-target checks.
- **Documentation**: `README.md`, `CONTRIBUTING.md` with six contributor
  recipes, `SECURITY.md`, the `wiki/` page set, `docs/schemas/`, issue and pull
  request templates, and six contributor skills under `.claude/skills/`
  written from the same source as the recipes.

#### Fixed

- A plan on a cask said "classed as GuiApp", and a withdrawal sentence printed
  `DISABLED`: the `{class}` and `{state}` parameters now carry one identifier
  vocabulary from every emitter and render through their labels.
- The top-level error message and its `--verbose` detail redact the home
  directory, like every other output a person may paste into a bug report.
- `error.provider_not_found` no longer points at `macportsbrew providers`, a
  command this build does not have; it points at `doctor`.
- The executor's reader-drain timeout was two seconds, which bounded a held
  pipe but also failed on ordinary scheduling: under load a reader thread could
  miss the window and the invocation came back with empty output marked
  truncated. Nothing mis-parsed — truncation is refused before any adapter
  parses — but the test gate failed for it at roughly one run in fifteen. Now
  six seconds, which keeps the worst case (two streams, drained in sequence)
  under the twenty the pathological-case test fixes. SE-057.

- **A package held by both providers can be migrated in either direction.**
  `--to` was compared against the unit's *inferred* source, which falls back to
  provider-id order, so one direction produced a plan and the mirror an error
  for the same machine. `--to` names the destination now and the other holder is
  the source (SE-047). Where the two directions still differ it is because of a
  fact — dependents on one side — and the plan says which side.
- **A plan no longer proposes installing what the destination already has.** The
  step remains, saying so, and carries no command. It still warns that it cannot
  tell whether the destination would supply a prebuilt artifact — that is a
  separate open finding about verification reading pre-existing state.
- **`--advanced` no longer changes the answer.** It decided which provider built
  a unit, and with it the unit's source, its visibility flag and the rules that
  fired — so `analyze` and `explain` could disagree about which provider held a
  package. Unit identity is now decided before visibility is applied, and the
  top-level set is the same in both modes (SE-048).
- **A removal names the package as the provider being removed from calls it.**
  A unit is keyed by one provider's spelling; the other side often uses another
  (`node` for `nodejs24`). Now that either side can be the source, addressing the
  removal by the unit key could name a package that side does not have — or a
  different one it does.
- **A plan command says whether it invokes a provider or this tool.** The one
  command that is not a provider invocation was recognised by comparing its
  program to a literal — inside the test named after the invariant, so
  nobody saw it carried no environment and a name that cannot be resolved
  safely. `PlanCommand` gains a `target` field (`PROVIDER` or `TOOL`; additive,
  so `schemaVersion` stays 1 — [`docs/schemas/plan.v1.md`](docs/schemas/plan.v1.md)).
- **An ambiguous package name is refused rather than guessed.** `python312` is
  what MacPorts calls the counterpart of three separate Homebrew packages;
  planning by that name silently picked the first.
- **Reverse dependencies are counted inside one provider.** The engine received
  the maximum across providers and reported it under the source's name, telling
  the user that three MacPorts ports depended on a package when none did and the
  three were Homebrew's. The count is per-provider (SE-045, DP-010).
- **A weak mapping is no longer called "a None match".** The sentence used the
  confidence label as an adjective, and the planner filled it from Rust's
  `Debug` spelling (`None`, `Strong`) while the policy rule raising the same
  reason passed identifiers (`NONE`, `STRONG`). Both now take the parameters
  from one helper, the renderer resolves them through the `confidence.*` labels,
  and the sentence reads "No equivalent under macports reaches confidence level
  Strong: the best is at level None." In `plan --json` the planner's
  `confidence` and `threshold` parameters change from `None`/`Strong` to
  `NONE`/`STRONG`; `schemaVersion` stays 1 (SE-064).

#### Documentation
- **The release machinery is written, reviewed and staged — not shipped**
  (pass 09). A release script that builds, signs, notarises, staples and
  packages one architecture and asserts every artifact (deployment floor,
  system libraries only, Developer ID authority on the final artifacts,
  payload identity across the bare file, `.pkg` and `.dmg`), a release workflow
  that drafts but never publishes, and a check that enforces the workflow's
  security rules are staged in `docs/passes/09-staged/` for the owner to apply;
  no workflow runs them yet and no release exists. The README's Install section
  now carries the text every release's notes will be generated from, says
  macOS 10.12–10.15 are best-effort in those words, and no longer calls the
  `.dmg` provisional. R-11's claim that the Intel runner is billed is corrected:
  it is free on a public repository (R-19). Five decisions are open for the
  owner (DP-031, DP-033–DP-036); the first tag waits on them. See
  [`docs/passes/09-release-engineering-report.md`](docs/passes/09-release-engineering-report.md).
- **Four claims about Homebrew and MacPorts carried no source and no read date**,
  on a page where everything else carried both — including the strong negative
  "`HOMEBREW_BUILD_FROM_SOURCE` does not exist". Two were checked here on
  2026-09-04 (it has zero occurrences in the shipped tree and zero in the manual
  page; and every mention of a bottle tag in that manual page is a flag that
  *takes* one, never a subcommand that prints it). Two were already answered in
  `docs/research/` and simply not cited. Also: a published GitHub wiki is a
  separate repository, so every `../docs/...` link in `wiki/` breaks on
  publication — the constraint `wiki/README.md` did not list, and the one that
  will bite. SE-066, finding F-20.
- **The documentation said more than the design does, in three places, and each
  was a qualifier the specification had written down and the reader never saw.**
  The README's lead said the tool "executes migrations"; it builds a reviewable
  plan, and this build executes nothing — which the lead now says, instead of
  the correction sitting fifty lines below the fold. INV-03 guarantees the
  machine is left at least as usable as before *whenever technically possible*,
  and four wiki pages dropped that qualifier while heading the section "The
  safety guarantees"; all four carry it now and say plainly that neither `brew`
  nor `port` offers transactions, so a provider's own half-finished write is not
  something any orchestrator above it can rewind. And the restore page —
  formerly "Undoing a Migration" — now says why the command is `restore` and not
  `undo`, and carries all three limits the specification mandates rather than
  one. The limit that was missing and matters most is the ordinary one: upstream
  moves on, so a restore usually gives you a **newer** version than the snapshot
  recorded, not the version you had. SE-058, findings F-13, F-14, F-15.

#### Changed
- **A cask's ownership comes from the provider's own record, and an unknown one
  is shown rather than hidden.** Homebrew's cask JSON carries no
  `installed_on_request`; it writes one into the cask's on-disk receipt, and the
  adapter reads it (DP-012, R-12). A cask with no receipt is `UNKNOWN` — never
  the inferred `TOP_LEVEL` the code used to produce on evidence it marked
  non-authoritative. `--replay` answers `UNKNOWN` for every cask, because a
  recording carries no receipt.

  Two counts that used to coincide now differ. `topLevelCount`'s schema
  definition always implied it; `topLevelUnits` had an empty Notes cell and is
  documented now. **`topLevelCount`** is "those the provider's own
  record says the user asked for" and no longer counts a cask without a receipt
  — it drops from 56 to 47 on the shipped fixture. **`topLevelUnits`** is "units
  shown in normal mode" and is unchanged, because an `UNKNOWN` row appears in
  the default view: a package whose ownership is unknown is not a package known
  to be a dependency, and hiding it would make normal mode quietest about the
  rows nobody has established anything about. No `--json` field is renamed, no
  value is removed, and `UNKNOWN` was already a documented `ownership` value —
  so `schemaVersion` does not move. What changes is which rows carry it.

  The human summary line reads "N packages in the default view" where it said
  "N top-level packages analysed", because the population it counts is now
  wider than that phrase.
- **A command in a plan that no executor could run now says so.** A plan built
  for a provider that was not detected has no prefix to resolve, so its commands
  read `<macports prefix>/bin/macports` and `<verify>` — illustrations that
  looked exactly like the real thing, in a document that serialised, diffed and
  passed every check. They are marked now, in the human output and in `--json`.
  Nothing executes in this build; `plan.v1.md` records refusing such a plan as
  an obligation the executor pass inherits, alongside two others.
  SE-066, finding F-20.
- **A machine where both providers answered about the same executable is read
  from the answer that names an owner**, not from whichever provider sorts first
  alphabetically. Nothing observed does this — a capture files each executable
  under one provider — but the answer should not have depended on the providers'
  names. SE-066, finding F-20.
- **A policy rule is named for what it means.** `NO_ACTION_ALREADY_THERE` fires
  when a package exists under only one provider — that is, when there is nowhere
  to move it to, not when it has arrived somewhere. Its reason has said
  `reason.no_alternative_provider_present` since it was written; the rule is
  called that now, and `explain` stops asking the reader to translate. The rule
  id appears in `explain --json`, so this is a breaking rename, done before the
  first release because that is the cheapest it ever gets. SE-065, finding F-20.
- **The support column showed a Rust identifier, and the package count was a
  sentence built in code.** `locales/en.toml` has said "Not supported" for an
  unsupported tier since it was written; the column printed `unsupported`,
  lower-cased from the enum, which no catalogue sees and no pseudolocale marks.
  Beside it, `"{} ({} top-level)"` was composed in Rust — the plural of "package"
  is the catalogue's business, and not every language puts that qualifier in
  parentheses. Both go through the catalogue now — and so does everything else
  that was reaching you as a Rust identifier: a plan step reads "Take a
  snapshot" rather than `SnapshotBefore`, a verification check "the provider
  reports it installed and active" rather than `ProviderReportsActive`, and a
  mapping's confidence and the machine's translation state likewise. The check
  that should have caught them was missing its third direction: it verified that every key the
  code references exists, and never that every key that exists is referenced.
  The same defect had two more sites, both fixed with it: `doctor` built the
  same `{:?}` string in the application layer, and the rules hand a stable
  identifier like `DEGRADED` into sentences a person reads. The catalogue has
  said since it was written that such a parameter is rendered through its own
  key before substitution; now it is. SE-064, finding F-20.
- **Six invariant tests that would not have failed if their invariant were
  violated.** The one for the sanitised environment skipped every `brew` and
  `port` invocation — which is what that invariant is about — and checked only
  the four macOS probes. The one for cross-provider dependency counts asserted
  one direction, against the planner's own value, under a comment describing
  something else. The one that swaps the two providers renamed their *contents*
  and kept the map keys, so iteration order was never exercised. The one for
  dated provider facts read one crate of five for one of the four things the
  invariant names — and on being widened to the rest, immediately found a second
  copy of a provider prefix in the MacPorts adapter, unused outside a test that
  compared it to itself. The one for primary-provider selection argues from types
  in the crate where selection is not implemented. None of this changes
  behaviour; it changes what a green suite means. SE-062, finding F-19.
- **Two verification steps in a plan said something other than their name, and
  now say what they mean.** `VERIFY_REMOVAL` carried "the provider reports the
  package installed and active" — as data, *verify the removal by confirming the
  package is still there*. It carries `PACKAGE_ABSENT`. And every check in the
  contract was satisfiable by state that predated the plan, so for a package the
  destination already held, a failed or no-op install cleared the gate that keeps
  anything from being removed before its replacement is verified; where the
  destination was absent, the step now also asserts `PACKAGE_NEWLY_PRESENT`,
  which a no-op cannot satisfy. Nothing executes in this build, so nothing acted
  on either — but the plan is the contract, and it has to be right before there
  is an executor to read it.
  Plan JSON gains two `check` values and a reader that tolerates a third it does
  not recognise, so `schemaVersion` stays 1 — see `docs/schemas/plan.v1.md`,
  which also records the rule that makes that tolerance safe: an unrecognised
  check can never count as satisfied. SE-049, finding F-06.

#### Security
- **A read can no longer carry an option this tool never asks for.** The
  spawn-site read allowlist accepted any `--long` option after a permitted
  subcommand, so `brew info --github` — which opens a browser — passed it, and
  only the execution gate stood between a plan and that. Each read subcommand
  now carries the exact long options it may take, matched as whole elements:
  `brew info --github`, another value of a listed option (`--json=v1` beside
  `--json=v2`), an abbreviation a provider would resolve (`port info --nam`)
  and a bare `--` are refused at construction (`plan_read_spec`) and at the
  spawn site, for both providers, by one function. Positional arguments are
  still pinned only by the gate. SE-050.
- **What a snapshot says can no longer reorder what you are shown or break
  its lines.** One definition of hostile text now serves both ends: controls,
  Unicode format characters (the right-to-left override, zero-width characters,
  tag characters) and the line and paragraph separators. Every human rendering
  escapes all three wherever a value is substituted into a sentence, and in the
  tables, plans and `explain` output that print snapshot text directly. The
  snapshot check still refuses only controls — format characters occur in real
  directory names — and now also reads the names of the reported build
  variables, which `doctor` prints back, and every provider identifier, which
  must be an identifier and must match the detection filed under it. A crafted
  snapshot with a provider id like `homebrew` followed by a terminal escape
  reached the `status` and `analyze` tables before. Reported *values* are kept
  as recorded: a tab in `CPPFLAGS` is a correct machine. Default-ignorable
  characters outside the format category (Hangul fillers, variation selectors)
  are not covered. `--json` output is unchanged. **Compatibility:** a v1
  snapshot with a control character in a reported variable name, or a provider
  id that is not an identifier or does not match its detection — neither of
  which a capture writes — loaded before and is now refused. SE-092.
- **The temporary file a write goes through no longer has a guessable name.**
  It was `.{name}.{pid}.{counter}.partial`, and the write refuses rather than
  being redirected if that name is taken — so anyone able to create files in the
  directory could make every `snapshot --out` and `doctor --report` fail. The
  variable part is OS-seeded randomness now. Inside the state directory this
  never mattered; those two commands write where you tell them to. SE-061,
  finding F-20.
- **A hostile tap can no longer put tens of megabytes into your snapshot.** The
  MacPorts parser has always capped output bytes, row counts and free text; the
  Homebrew one capped the document at 64 MiB and the values that become path
  components, and nothing else — so `caveats` and `desc` were unbounded and there
  was no limit on how many formulae one document could describe. Both are bounded
  now, at the numbers MacPorts already used, and free text is trimmed rather than
  refused because an over-long description is a nuisance and not evidence that
  the output format changed. Path traversal through a package name was already
  closed on both sides. SE-060, finding F-20.
- **A plan no longer tells you a read needs administrator rights.** The plan for
  `htop --to macports` ended *"Steps that would need administrator rights: 2,
  3"*, and step 3 was `port -q installed htop` — a read. It said so because
  `verify` had no elevation stated in `data/policy/provider-commands.toml`, and a
  missing value folds to "unknown", which the summary rendered as "would need".
  Folding toward caution is right; saying it is a known requirement is not, and
  training someone to approve a password prompt for a read is exactly the habit
  INV-06 exists to prevent. The four missing values are stated now — Homebrew's
  `verify` is `none` on evidence, MacPorts' is `unknown` because its
  documentation does not settle it and this machine has no MacPorts to observe —
  and a step whose elevation is genuinely unknown is reported as unknown, on its
  own line. The loader refuses a command table that leaves any verb's elevation
  unstated, which is what that file's own comment had claimed all along.
  SE-059, finding F-10.
- **This tool spawns six programs, and refuses everything else at the moment of
  spawning.** `brew`, `port`, `sw_vers`, `uname`, `sysctl`, `xcode-select` — each
  absolute, free of `..`, and for the two providers only from a `bin` directory
  under a detected prefix. The threat model had said command injection was
  "structurally impossible … enforced by a lint rule"; the lint enforced *where*
  a subprocess may be spawned, and the executor's only check on the program was
  that it is absolute, which `/bin/sh` is.
  A first attempt put the guarantee in the type instead, and a security review
  took it apart: `CommandSpec`'s fields were public, so `spec.program = <anything>`
  after construction reached the same place — and the same line would have undone
  this release's own fix for the environment, by inserting `DYLD_INSERT_LIBRARIES`
  after sanitisation. The fields are private now, and the check that matters runs
  on the value about to reach the kernel, where no amount of cleverness upstream
  can route around it.
  Also corrected: a claim in `04-migration-and-safety.md` that no shell interprets
  any of this. One does — `brew` is a bash script and `port` is a Tcl program.
  What is true, and what is now written, is that no shell *command line* is ever
  constructed: arguments are argv elements and nothing re-splits them.
  SE-050, finding F-08.
- **`doctor` now sees the commonest executable-shadowing directory on macOS.**
  A directory you own at `0755` — `~/bin`, `~/.local/bin`, whatever a
  `curl | sh` installer created — ahead of `/usr/bin` produced no finding at
  all, though it is verbatim the threat-model case: anything running as you can
  plant a `git` there. It is reported now, as a warning — a directory you own
  ahead of `/usr/bin` is how developer machines are arranged, not a broken one.
  `ERROR` is reserved for the shapes where somebody else decides what runs: a
  world-writable directory before the system ones, and an entry that is not
  absolute, whose meaning changes with the working directory. A `PATH` with no
  system directory on it is a finding rather than a silent degradation of every
  escalation, and `/usr/bin/` with a trailing slash is recognised as one.
  SE-053, finding F-07.
- **A snapshot can no longer put a flag where a package name belongs.** The
  adapters validate every name a provider reports; `--from-snapshot` read the same
  fields out of a file that never met one, and the planner turns them into argv
  elements and executable names without passing any allow-list. A package called
  `--force`, or `-o/etc/anything`, or `../../../bin/sh` rendered into a plan, and
  the plan is what an executor will run. A snapshot is now refused unless what it
  states is what a provider could have said, with the rule chosen by where the
  value goes: a name becomes an argument and is held to the adapters' own
  predicate; the prefix becomes a command's program and is held to the prefix
  rule; everything else reaches a reader and is only bounded and free of control
  characters. Twelve words `port` reads as *sets of ports* — removing the set
  named `installed` is how you remove the whole installation — are refused from
  data. SE-051 parts 2 and 3, finding F-09.
- **A file can no longer decide what a provider command runs in.** Two routes led
  there. `--from-snapshot` read `invocationEnvironment` verbatim, so a snapshot
  could put the other provider's prefix on a `port` command's `PATH`, or a
  `SUDO_ASKPASS` beside it on the step that elevates. And a *constructed*
  environment was no safer: the `PATH` leads with `<prefix>/bin`, and the prefix is
  what `brew --prefix` answered — from a recording under `--replay`, from the
  provider on a live run. A snapshot is now refused, on either route, unless its
  environment has the shape an adapter builds: only the variables an adapter emits,
  the pinned ones unchanged, and every `PATH` entry a system directory or one under
  a prefix that provider owns — and no provider may claim a prefix another owns.
  Refused, not quietly repaired, so the analysis stays a function of the snapshot.
  What is asserted is equality with what an adapter builds — the same variables,
  the same fixed values, a `PATH` of that provider's own directories followed by
  the system ones — because two security reviews showed that anything looser has
  a way through: `SUDO_ASKPASS`, a case-folded `/OPT/Local` on a case-insensitive
  volume, and a `HOME` of the file's choosing, which makes `brew` load
  `$HOME/.homebrew/brew.env` and re-export the very keys being refused. A snapshot
  written before `invocationEnvironment` existed carries none, and no longer loads
  unless every provider in it was absent or broken — which is the same exemption
  that keeps a snapshot from a machine with a failed provider readable.
  SE-051 part 1, finding F-21.

- **`doctor --report` and `snapshot --out` no longer publish the home path when
  `HOME` is unset.** Redaction treated "no home directory" as "nothing to
  redact", which is right for a replayed recording and wrong for a live run
  whose `HOME` is unset or relative — routine under `cron`, `launchd` and some
  CI. Measured before the fix: 39 occurrences of the real home path, username
  included, in a file the documentation invites the user to attach to an issue.
  The two cases are now distinct states rather than one absent value, and the
  report says which redaction produced it in a new `redaction` field
  (`GROUNDED`, `HEURISTIC` or `AS_CAPTURED`; additive, so `schemaVersion` stays 1 —
  [`docs/schemas/doctor.v1.md`](docs/schemas/doctor.v1.md)).
- **`doctor` no longer stops reporting writable `PATH` directories when `HOME`
  is unset.** The audit ran on redacted entries, so a `~` it could not expand
  became a false "not absolute" and skipped the world- and group-writable
  checks — the executable-shadowing item in the threat model — on the same runs
  the redaction fix targets. The audit now runs on raw entries and each finding
  is redacted afterwards. This also fixes a pre-existing case: a username
  appearing outside the home turned a real directory into a path that does not
  exist, reported as missing instead of as writable.
- **Every GitHub Action is pinned to a commit SHA**, checked by
  `scripts/check-action-pins.sh` in `scripts/validate.sh`. Four of the twelve
  references were mutable by name, including one that downloads a binary into
  the job. `cargo-deny` now runs `advisories`, which `deny.toml` had declared
  and the workflow had never invoked, so a RUSTSEC advisory or a yanked crate
  could ship green. See SE-055.

#### Not yet implemented

Named here because a changelog that lists only what exists invites the reader
to assume the rest does: a published release (the release machinery is staged,
not applied), executing a migration (`migrate --yes` stops at the
gate), executing a restore, `restore --snapshot`, `primary`,
`provider-priority`, `providers`, `env`, `cleanup`, the interactive
TUI, `doctor --deep`, the cache and its TTL, and translations for `es`, `ja`
and `zh-CN` — the i18n architecture, the English catalogue and the `pt-BR`
translation are in place; the others are not.

#### Known limitations

- **Mapping looks only at what is already installed.** In 0.1.0 a package is
  matched to the other provider only if that provider already has an
  equivalent installed, or the small curated table names one. For anything
  else the plan stops at *Needs a human decision* with "no equivalent found" —
  nothing was found, which does not mean no equivalent exists. Searching the
  other provider's full catalogue is planned for a later release.
- **CI has run, and the matrix still states configuration.** When this version was
  first prepared no hosted runner had executed the workflow in
  `.github/workflows/`; since then "Repository validation" has passed on `main`
  (run 39578935218 at `ad5a95d`, 2026-10-21, green on macos-26, macos-26-intel,
  macos-15 and macos-15-intel). The original reasoning stands for the matrix
  file: The support
  matrix is generated from that workflow's `runner:` labels and from nothing
  else, so every row in it — including the **tested** ones — states what CI is
  configured to do, not what any run reported. The matrix says so at the top
  until there is a green run. SE-052, finding F-12.

- `--no-network` and `--refresh` are accepted and do nothing in this build.
  `--yes` makes `migrate` put its plan to the execution gate, which refuses it
  (exit 4); `--dry-run` with it shows the plan without putting it to the gate.
  `--no-network` does not stop a Homebrew read fetching its catalogue
  on an empty cache (next item).
- **A Homebrew read is not as quiet as the environment this tool gives it.**
  On an empty Homebrew API cache, every Ruby read this tool runs (`config`,
  `info`, `list`) downloads the package catalogue — about 15 MB, writing about
  30 MB under your cache directory — and `--no-network` does not prevent it. A
  `brew.env` in `/etc/homebrew/`, `<prefix>/etc/homebrew/` or `~/.homebrew/`
  outranks the quieting environment, `HOMEBREW_NO_ANALYTICS` included, so it
  can switch analytics and network fetches back on for those reads. MacPortsBrew
  itself makes no network call of its own. Observed on Homebrew 7.0.4,
  2026-10-02 ([R-15](docs/research/R-15-homebrew-brew-env.md)). `doctor` now
  reports such a file and a mutation against it is refused (DP-027). The
  charter's non-goals now say what holds: MacPortsBrew sends nothing and makes
  no network call of its own, and `doctor` discloses a provider configuration
  that may enable the provider's analytics and whether a provider read may
  reach the network ([DP-028](docs/decisions-pending.md), ruled 2026-10-27).
- `Messages::for_locale` wires `pt-BR` (the translation laid over English) and the
  pseudolocale; `es`, `ja` and `zh-CN` still select their plural rules and layout
  while the text stays English.
- Branch-coverage floors are not enforced: `cargo-llvm-cov --branch` needs a
  nightly toolchain and the pinned toolchain is stable
  ([ADR-0002](docs/adr/0002-implementation-language.md)).
- When a live run cannot resolve the home directory, redaction falls back to
  recognising the *shape* of a macOS home (`/Users/<name>`). A home somewhere
  else — a network mount, a relocated account — is not recognised, and the
  diagnostic file says `"redaction": "HEURISTIC"` rather than pretending
  otherwise. Skim the paths before sharing such a file.
- There is no published release yet. Release signing is decided — Developer ID
  plus notarisation, with a bare binary, a `.pkg` and a `.dmg`
  ([DP-004](docs/decisions-pending.md),
  [ADR-0005](docs/adr/0005-release-signing-and-artifacts.md)), and the
  repository is `github.com/pabloguia/MacPortsBrew`
  ([DP-007](docs/decisions-pending.md), DP-031).
