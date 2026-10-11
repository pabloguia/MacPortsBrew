<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 3 — configuration re-read, the positional pin, the mirror plan: plan

Work order: [`../../prompts/08-slice-3-config-pin-mirror.md`](../../prompts/08-slice-3-config-pin-mirror.md).
Drafted 2026-10-10. Reviewed by `spec-guardian` and `security-reviewer` before
any code; findings cited `[SG Hn]` / `[SR Mn]`.

## Facts that shape it

1. **The configuration read already exists and is filesystem-only.**
   `adapters::configuration::read(provider, prefix, home, FilesystemScope)`
   (`crates/adapters/src/configuration.rs:143`) stats and bounded-reads the
   files `data/policy/provider-configuration.toml` names (Homebrew's three
   `brew.env`, MacPorts' user `macports.conf`), records every key by name and no
   value, and answers `Unknowable` off this machine. The planner turns
   `outranks()` into the `PROVIDER_CONFIGURATION_RISK` risk
   (`planner.rs:754-810`); the gate refuses every mutating step against that
   provider from the plan's risks (`execution_gate.rs:572-593`). Nothing re-reads
   between the capture and a step.
2. **The spawn site pins programs, subcommands and long options, and positionals
   only by shape** (`process/mod.rs:289-312`: no `/` or `:` outside a raw-string
   subcommand). SE-050: "which package a plan's read names is still pinned only
   by the gate". The spawn site cannot know the plan's package; it *can* know how
   many positionals each read takes and what they may be.
3. **The argument shapes this build runs** (adapters' `ALLOWED_ARGV`,
   `provider-commands.toml` verbs): Homebrew — none of its reads take a
   positional except `list --versions <name>` (verify) and its bare inventory
   form (verify-absent); MacPorts — `version` and `installed` take none (and
   `installed <name>` is the verify read), `echo` takes exactly one of
   `requested`, `active`, `leaves`, `info` takes one or more port names after
   its field options, `provides` takes one or more raw paths.
4. **MacPorts→Homebrew is unplannable on a live machine that runs no MacPorts
   service.** `macports::service::scan` (`crates/adapters/src/macports/service.rs:65`)
   answers `Unreadable` when `<prefix>/etc/LaunchDaemons` does not exist — the
   ordinary state of an install where no port has a startup item, including this
   machine (`ls /opt/local/etc` → `macports` only, 2026-10-10). `ServiceMetadata`
   is then `Unknown`, and DP-016(c) (`planner.rs:559-590`) refuses every plan
   with a MacPorts source. Observed: a live `snapshot --out` then
   `--from-snapshot … plan jq --to homebrew` exits 4 with "does not record whether
   macports packages define services". **This is an INV-17 asymmetry in the
   product**, not only a fixture gap.
5. **The scan also misses a service.** MacPorts 2.12.6
   `port1.0/portstartupitem.tcl` (sha256
   `cc4d2c99b1e303d4677f9d562f8c3a9f076bea2d8e28263f099b805f0ee592b3`, read
   2026-10-10): the item directory is `${prefix}/etc/${startupitem.location}/${uniquename}`
   (line 290), `startupitem.location` defaults to `LaunchDaemons` (line 108) and is
   a per-port option whose other value is `LaunchAgents` (lines 546-560); the
   directory is created in the destroot only when a startup item is created
   (`startupitem_create`, line 513; `startupitem.create`/`type none` create
   nothing, lines 130-133). So a port whose item lives in `LaunchAgents` is
   reported "defines no service" today, and INV-19 does not force its
   `MANUAL_REVIEW`.
6. Under `--replay` and `--from-snapshot` of a recording, the filesystem is
   `Elsewhere` and the scan rightly answers `Unreadable` (INV-15). A **live
   snapshot** is a capture of this machine and records the scan's answer, so a
   snapshot taken by the binary's own `snapshot` command is a captured fixture
   (never invented) on which the mirror plan can be built.

## Design

### Part A — the configuration re-read

- **Adapter** (`crates/adapters/src/registry.rs`): `reread_configuration(provider,
  prefix, home) -> ProviderConfiguration`, calling `configuration::read` with
  `FilesystemScope::ThisMachine`. Filesystem only; no process, no new spawn.
- **Application** (`crates/application/src/executor.rs`, pure):
  `recheck_provider_configuration(captured: &ProviderConfiguration, fresh:
  &ProviderConfiguration) -> Result<(), ConfigurationDrift>`. Passes only when
  `fresh.outranks() == Some(false)` **and** `fresh` equals `captured` (same files,
  scopes, readability, key names). `ConfigurationDrift` names the provider-neutral
  reason (`Outranks`, `Unknown`, `ChangedSinceCapture`); refusal is
  `ErrorId::SafetyBlocked` (a safety check stopped it, nothing changed).
- **Not wired**: the switch is off. `history_preconditions.rs`'s
  `wiring_violations` gains the token `recheck_provider_configuration(` and the
  rule **every** `dispatch(` call is preceded, since the previous dispatch call
  (or since the marker write for the first), by a re-check call — so a loop that
  re-checks once before entering it fails. Mutant tests as in slice 2.
- INV-15: the application function takes values, so replay can never reach a
  filesystem through it; the adapter takes no `FilesystemScope` parameter that a
  replay could pass `Elsewhere` through — a replay never calls it, which the
  existing `HistoryScope`-style gate in the shell enforces when wired (stated).

### Part B — the positional half of the spawn-site pin

`ReadSubcommand` gains `positionals: Positionals`, one of:

| Variant | Meaning | Used by |
|---|---|---|
| `None` | no positional | `brew --version`, `--prefix`, `--cellar`, `--caskroom`, `--cache`, `config`, `info`; `port version` |
| `AtMostOneName` | zero or one safe provider name | `brew list` (inventory / verify); `port installed` (inventory / verify) |
| `OneOf(&[...])` | exactly one, from a fixed set | `port echo` → `requested`, `active`, `leaves` |
| `Names` | one or more safe provider names | `port info` |
| `RawStrings` | one or more raw strings (replaces `raw_strings: true`) | `port provides` |

"Safe provider name" is `domain::is_safe_provider_name` — the same predicate the
planner and the adapters already use, so the spawn site accepts no name the rest
of the system would not. A positional is any element after the subcommand that
does not start with `-`. Order is not pinned (options and names may interleave as
the adapters write them) but the count and each element's class are. Separators,
single-dash options and unlisted `--long` options stay refused as now.

Two-way tests per subcommand: every argv the adapters and `provider-commands.toml`
produce passes (the existing `every_read_this_build_passes` helper), and each of
too many, too few, an unsafe name, an unlisted `echo` word and a positional on a
`None` subcommand is refused. **No subcommand, option or program is added**: the
allowlist does not grow. SE-050 is amended: the spawn site now pins arity and
class; *which* package a plan names remains the gate's (`COMMAND_IS_NOT_THE_PLANNERS_READ`),
because only the plan knows it.

### Part C — the service scan, and the mirror plan

- **Fix the scan** (`macports/service.rs`): scan both `etc/LaunchDaemons` and
  `etc/LaunchAgents`. A location directory that does not exist (`NotFound`) while
  `<prefix>/etc` is a readable directory means no installed port created an item
  there → that location contributes nothing, and the scan is **known**. Any other
  error (permission, not a directory, `<prefix>/etc` missing or unreadable) stays
  `Unreadable`. The name-match caveat (R-8 Finding 11) is unchanged.
  `startupitem.type none` and `startupitem_type none` in `macports.conf` create
  no item, so such a port is reported as defining none; the user `macports.conf`
  case is already a `PROVIDER_CONFIGURATION_RISK` (DP-027), and the system file is
  not read — recorded as a residual in R-8's addendum.
- **The mirror fixture**: a live snapshot of this machine
  (`macportsbrew snapshot --out`), redaction checked, committed as
  `fixtures/snapshots/both-providers-intel-macos26-live.json` with a provenance
  note in a new `fixtures/snapshots/README.md` (captured by the binary, the
  command, the date, the versions). Tests:
  `invariant_17_a_macports_to_homebrew_plan_is_built_from_a_live_capture` (CLI,
  `--from-snapshot … migrate jq --to homebrew` builds a full plan whose step
  kinds mirror the Homebrew→MacPorts plan's) and the CLI half of
  `invariant_17_retained_run_recognition_is_direction_neutral`.
- **What stays as it is**: `--replay` with a MacPorts source is still refused by
  DP-016(c), by design (INV-15). `scripts/run-fixture-matrix.sh` (guarded) does
  not learn about snapshots; the CLI tests carry them.

## Commits

1. **docs** — this plan, reviewed; R-8 addendum (facts 4–5); SE-050 and SE-092 notes.
2. **adapters** — the service scan reads both locations and knows an absent one.
3. **adapters** — the positional pin.
4. **adapters + application** — the configuration re-read and its comparison.
5. **cli tests** — the re-check tripwire; the mirror snapshot fixture and the
   INV-17 tests.
6. **docs** — 04 (Process execution, Concurrency status), CHANGELOG; "As built".

## Named tests

- **Scan:** absent `LaunchDaemons` under a readable `etc` → known, empty; absent
  `etc` → unreadable; a permission error → unreadable; an item under
  `LaunchAgents` is found; a directory without its plist is not a service; off
  this machine → unreadable (existing); `invariant_19_a_launch_agent_service_forces_manual_review`
  (application, through a snapshot whose scan found it).
- **Pin:** per `Positionals` variant, accepted and each refused shape;
  `every_read_this_build_passes` unchanged; the allowlist-size test unchanged.
- **Re-check:** each `ConfigurationDrift` reason; equal-and-clean passes;
  `Unknowable`/`NotRecorded` refused; mutation-proved.
- **Tripwire:** a re-check missing before the second dispatch is a violation.
- **INV-17:** the two tests above.

## Decisions (for SE-050 / SE-092 / R-8)

- **D-C1** an absent StartupItem location under a readable `<prefix>/etc` is a
  known "none there"; both locations are scanned.
- **D-P1** the spawn site pins positional arity and class per subcommand; the
  package identity stays the gate's.
- **D-R1** a mutating step requires the provider's configuration, re-read just
  before it, to be equal to the capture and to outrank nothing.
- **D-F1** a live snapshot taken by the binary is a captured fixture.

## Review

- **spec-guardian (2026-10-10):** H1, M1–M5, L1–L3; no Tier 1/2 violation.
- **security-reviewer (2026-10-10):** H1–H2, M1–M3, L1–L4; no allowlist growth.

Both found the original Part C unsound and the original Part B's class too wide.
The revision below **supersedes** the Design and Decisions above where they differ.

## Revision after review (2026-10-10) — this is what is built

### C′ — the service scan, made fail-closed [SG H1, M1; SR H1]

The scan is fail-open **today**, not only after the planned change: an item
directory not named `org.macports.<port>` — `dbus`'s `org.freedesktop.dbus-system`
in `LaunchDaemons` and `org.freedesktop.dbus-session` in `LaunchAgents`
(`devel/dbus/Portfile:54-66`), and 85 Portfiles in the local tree that rename
the item, its plist or declare several — is skipped, so a service port is
reported as defining none and INV-19 does not force its review; per-entry errors
are dropped, a non-UTF-8 name is skipped, and more than `MAX_ROWS` entries
truncate silently. The rebuilt scan answers **known** only when all of these hold,
and `Unreadable` otherwise:

1. `<prefix>/etc/macports` is a directory and `<prefix>/etc` is a readable
   directory, by `symlink_metadata` [SR H1, SG L1];
2. each of `etc/LaunchDaemons` and `etc/LaunchAgents` is either absent by
   `symlink_metadata` (`NotFound`) or a real, readable directory — a symbolic
   reference or anything else is `Unreadable` [SG L1];
3. every entry of each is a directory named `org.macports.<name>` whose `<name>`
   is an **installed** port and which holds `org.macports.<name>.plist`; **any
   other entry** (another uniquename, a stray plist, a directory without its
   plist), any entry error, any non-UTF-8 name, or more than `MAX_ROWS` entries
   makes the whole scan `Unreadable` — never "not a service" [SR H1];
4. **every installed port is active** — deactivation deletes the item directory
   (`registry2.0/portimage.tcl` `_deactivate_files`, ~975), so with an inactive
   port the absence proves nothing about it [SG H1]. Decided in the adapter, which
   holds both the inventory and the scan.

Locations other than the two launchd reads are not scanned: `install_darwin_launchd`
links the item into `/Library/<location>`, and launchd loads only those two —
recorded as a residual. **Meaning recorded, not settled** [SG M1]: this answers
"a launchd item MacPorts *materialised*", while Homebrew's `has_service` answers
"the formula *declares* a service"; a port built with `startupitem.type none` (or
a system `macports.conf` `startupitem_type none`) materialises none. INV-19 speaks
of "managed launch agents", which the materialised reading matches; whether the
two providers must use one meaning is **DP-032** for the owner (recommendation:
read the installed Portfile under `var/macports/registry/portfiles/`, which also
covers inactive ports — owed with an R-8 addendum). Attributing items by
`port -q provides <plist>` [SR H1 fix 2] is the better answer and is recorded as
owed: it adds a capture-time invocation, which this slice does not.

Tests: absent locations under a real prefix → known empty; absent `etc/macports`
→ unreadable; a symbolic reference at a location → unreadable; a
`org.freedesktop.dbus-system`-shaped item → unreadable
(`invariant_19_a_service_under_a_custom_uniquename_is_never_reported_serviceless`);
an item for a port that is not installed → unreadable; a directory without its
plist → unreadable; an inactive installed port → `ServiceMetadata` unknown
(`invariant_19_an_inactive_service_port_is_not_reported_as_serviceless`); a
`LaunchAgents` item → found. All built in temporary trees or in code — no
invented fixture [SG L2]. DP-016's entry gets a note that the refusal now stops
on live captures with a sound answer, and why [SG L3].

### B′ — a per-provider positional class [SG M3; SR H2, L1, L2]

| Provider | Name class at the spawn site |
|---|---|
| `port` | `macports::names::is_safe_name` (no `@`), first character ASCII alphanumeric, and not a reserved pseudo-port — the `reserved_names` of `provider-commands.toml` plus `and`, `or`, `not` — compared case-insensitively with any `@…` suffix stripped |
| `brew` | `is_safe_provider_name`, first character ASCII alphanumeric, and no `.rb`, `.json` or `.tar.gz` suffix (Homebrew's loaders accept such a relative file; today only the child's working directory `/` stops it) |
| `port provides` | `AbsolutePaths`: absolute, no `..` component, no NUL — replaces `RawStrings` |

The spawn-site reserved list is a tool refusal list (no `read_on`), tied by a test
to `provider-commands.toml`'s `reserved_names`, to which `and`, `or`, `not` are
added. **The planner's guard** (`planner.rs:1160-1164`) strips an `@…` suffix
before comparing, so `installed@1` is refused there too — an existing defect,
fixed here with its test. Refused-shape tests include `all`, `current`,
`installed@1`, `+universal`, `@1.0`, `not`, `and`, `jq@1.8` (port), `foo.rb` (brew).
SE-050 is amended to say "arity and a per-provider name class".

### A′ — the re-check [SG M2, M4; SR M1–M3, L3]

- `reread_configuration(provider, prefix, home, FilesystemScope)` — `Elsewhere`
  answers `Unknowable` [SG M4]; `home` is the captured home the adapters were
  built with, never an environment read.
- `recheck_provider_configuration(provider, captured, fresh) -> Result<ConfigurationRechecked, ConfigurationDrift>`
  passes only when **both** `captured.outranks()` and `fresh.outranks()` are
  `Some(false)` — equality is dropped: the capture's labels are home-redacted and
  the fresh read's are not, so equality would always fail under a home-rooted
  prefix, and once both outrank nothing it adds only "the same empty files" [SR M1].
- `ConfigurationRechecked { provider }` is neither `Clone` nor `Copy`; the wiring
  (slice 4) makes the mutating dispatch take it by value and check its provider
  against the step's — the per-step, per-provider enforcement the textual
  tripwire cannot give [SG M2]. The plan's sentence about loops was wrong and is
  withdrawn. The tripwire asserts that the re-check result is bound or
  `?`-propagated (`let _ =` and a bare statement are violations) [SR L3].
- The window between the re-read and the provider's own read is inherent, and for
  an elevated step will include the password prompt; for an elevated MacPorts step
  the home `port` resolves may differ from the invoking user's — both written into
  D-R1 and 04, and the elevated case owed with the consent path (slice 4) [SR M2, M3].

### C″ — the mirror fixture [SG M5; SR L4]

Captured **after** C′ lands, by the binary's own `snapshot`; committed under
`fixtures/snapshots/` with a README giving the five provenance fields of 07,
`origin: captured`, its sha256, and that its `PATH` and inventory are deliberately
public (the committed recordings already carry the same). A test over `fixtures/**`
fails on `/Users/<name>`, the current user name, a `reported` environment value,
or a configuration key list — in `crates/cli/tests/`, since `validate.sh` is
guarded. SE-092 records D-F1 against 07's "committed snapshots" and the DP-016(c)
reason.

### Decisions, revised

- **D-C1′** the MacPorts service scan is known only under the four conditions of
  C′; any unattributable item is unknown. Materialised-versus-declared is DP-032.
- **D-P1′** per-provider positional classes at the spawn site; reserved
  pseudo-ports refused there and, with `@` stripped, in the planner.
- **D-R1′** a mutating step requires both the captured and the freshly re-read
  configuration to outrank nothing; the re-check yields a linear token the wiring
  must consume per step and provider.
- **D-F1** a live snapshot taken by the binary is a captured fixture, privacy-checked by a test.

## As built (2026-10-11)

Commits `ffdd3b3`, `4871a19`, `143ce68`, `20f4c40`, `655f2be`, and the two
end-of-slice commits that follow them (review fixes; the mirror fixture).
`THIS_BUILD_DISPATCHES_MUTATIONS` is still `false`; no dependency, no new spawn
site, no guarded path touched; every new rule mutation-tested, survivors killed
or argued equivalent in the commit messages.

- **C′ — the service scan.** As planned, then strengthened after the
  end-of-slice reviews: each item is attributed to the port `port -q provides`
  names for its plist, whatever the item is called (security review M1: a
  subport may write its parent-named item; `dbus` writes `org.freedesktop.*`),
  and one scan per capture feeds both the `ServiceMetadata` capability and every
  package's `defines_service` (security review and spec-guardian, M2). The
  attribution makes `dbus` attributable instead of unknown, which removes the
  residual INV-17 asymmetry spec-guardian recorded (L7). R-8 has an addendum.
- **B′ — positionals.** As planned; `port info` must also carry `--index`
  (security review L3: without it `action_info` evaluates each Portfile).
- **A′ — the re-check.** `recheck_provider_configuration(provider, snapshot,
  fresh)` looks the captured reading up itself (security review L2). The
  re-read is `adapters::configuration::read` with `FilesystemScope::ThisMachine`,
  which already has the signature the plan proposed for a new
  `reread_configuration`, so none was added; slice 4 must pass it the captured
  `home`, never one read from the environment. The tripwire requires the bound
  token to be an argument of the dispatch it guards (security review L1). The
  two elevated-step residuals are in 04 and the SE-016 addendum (spec-guardian
  M2), and slice 4's work order carries them.
- **C″ — the mirror fixture.** Planned with `jq`; built with `htop`, because a
  snapshot records only installed packages and a MacPorts-only package maps to
  Homebrew at confidence `None` (SE-092 addendum). The test compares kinds,
  providers and each step's checks. `fixture_privacy.rs` fails on non-UTF-8 files
  as well. The CLI half of the retained-run recognition test is owed in slice 4.
- **Policy data** `2026.10.03-2`: `reserved_names` gained `and`/`or`/`not`
  without moving a read date, so the serial moved (docs-auditor M2).
