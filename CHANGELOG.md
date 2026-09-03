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

The read-only slice. Nothing here executes anything, elevates anything, or
writes into either provider's prefix.

**Policy version: `2026.09.02-1`.** The provider facts it carries were read on
2026-09-02 and are dated in `data/policy/*.toml`; the staleness threshold is
180 days, after which the CLI says the data may be out of date without refusing
to answer.

### Added

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
  [`docs/schemas/`](docs/schemas/README.md), all at `schemaVersion` 1.
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

### Fixed

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
- **An ambiguous package name is refused rather than guessed.** `python312` is
  what MacPorts calls the counterpart of three separate Homebrew packages;
  planning by that name silently picked the first.
- **Reverse dependencies are counted inside one provider.** The engine received
  the maximum across providers and reported it under the source's name, telling
  the user that three MacPorts ports depended on a package when none did and the
  three were Homebrew's. The count is per-provider (SE-045, DP-010).

### Security

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

### Not yet implemented

Named here because a changelog that lists only what exists invites the reader
to assume the rest does: `migrate`, `restore --run`, `primary`,
`provider-priority`, `providers`, `env`, `history`, `cleanup`, the interactive
TUI, `doctor --deep`, the cache and its TTL, and translations for `pt-BR`,
`es`, `ja` and `zh-CN` — the i18n architecture and the complete English
catalogue are in place, the translations are not.

### Known limitations

- `--no-network`, `--refresh`, `--dry-run` and `--yes` are accepted and are
  no-ops in a build that never uses the network, never caches, is always a dry
  run and never prompts. They exist so scripts written now keep working.
- `Catalogue::for_locale` returns the English catalogue for every tag except
  the pseudolocale, so `--language pt-BR` selects the locale's plural rules and
  layout while the text stays English.
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
  [ADR-0005](docs/adr/0005-release-signing-and-artifacts.md)) — and the
  repository will be public on GitHub
  ([DP-007](docs/decisions-pending.md)), but the remote does not exist yet.
