<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 10 — the read-only release candidate: report

Date: 2026-10-11. Work order: [`prompts/10-read-only-release-candidate.md`](../../prompts/10-read-only-release-candidate.md).
Commits: `9b41123` (truth sweep: code, catalogue, docs, walkthrough tests),
the `doctor --report` section-by-section test, and the end-of-pass commit that
adds this report and the staleness test. `THIS_BUILD_DISPATCHES_MUTATIONS` is
`false`; nothing under `crates/cli/src/executor/`, `crates/application/src/executor*`,
`execution_gate.rs`, `verification.rs` or `crates/adapters/src/process/` changed in
this pass; no `--json` schema changed (`schemaVersion` unchanged; the only
JSON-visible change is the classification fix below, which changes values, not
shapes).

## What changed for a user

**The truth sweep** (`docs-auditor`, run first and again at the end) found every
place a surface described a build this is not. Fixed:

- **Plans** no longer print `$ macportsbrew primary <pkg> <provider>` — a command
  that does not exist — but say how to make a copy the one that runs (`PATH`
  order or the full path). The snapshot and record steps no longer promise
  restore or history; the snapshot step tells the user to run
  `macportsbrew snapshot --out before.json` first. The plan header says the user
  runs the steps at their own risk and that `--retain-source` is the safe pattern.
  The plan type and its `--json` form are unchanged (rendering only).
- **`migrate --yes`** opens with "This version never changes your machine, so the
  plan was not run", lists the gate's reasons, then says what to do by hand.
- **`--help`**: an accurate tagline ("See what Homebrew and MacPorts have
  installed, compare them package by package, and plan a migration in either
  direction. Version 0.1.0 reads and plans; it changes nothing."), long help
  without internal jargon, no spec identifiers in flag help.
- **No invented address.** The stale-data notice, `doctor` and the status view
  no longer print `github.com/macportsbrew/macportsbrew` (DP-031 is undecided);
  the notice is said once, on stderr. The README's install lines use
  `<owner>/<repo>`, which the release job fills (staged `release.yml` updated
  and its notes assembly simulated: the three verification flags survive).
- **Empty and broken states**: neither provider, or a broken one, is named
  instead of a suggestion to use `--advanced`; `doctor` words an absent provider
  as absent; `list --provider <unknown>` is an error like `plan --to <unknown>`;
  `explain` renders an empty fact as `none`.
- **A wrong recommendation**: `ffmpeg` was classed a language runtime because it
  has versioned siblings, and told to use a runtime manager. `RUNTIME` now needs
  a language family named in `data/policy/runtime-managers.toml` (SE-006
  addendum); goldens regenerated.
- **Catalogue**: the duplicate `doctor.report_written` (the second, absolute
  copy won) is gone; plurals in the status summary and the unknown-ownership
  line; corrected reason texts (`source_withdrawn`, reverse dependencies,
  `destination_provider_absent`).
- **README, wiki, CHANGELOG, SECURITY** rewritten for a first public release:
  "a later release" instead of passes; restore/history/`primary` marked as not
  in 0.1.0; the network claim made true; CI described as configured, never as
  run; the MacPorts→Homebrew page gained its "by hand" section and a real plan
  from the live capture; every output sample refreshed from the binary (the three
  test-held ones by `documented_samples.rs`). The CHANGELOG opens with a
  user-facing "0.1.0 at a glance"; the pre-release log follows.

## Tests added

- `crates/cli/tests/walkthrough.rs`: every Getting-Started step (`status`,
  `analyze`, `list`, `doctor`, `explain`, `plan`) on every recording, pinning the
  exit code and the first line read; the unknown-provider error; the `none`
  fact; stale policy data warns once on stderr and never refuses. Every new
  rendering branch was mutated and the mutant killed.
- `crates/cli/tests/capture_redaction.rs`: `doctor --report`'s sections
  (machine, prefix, packages, findings) asserted one by one, non-vacuously.
- Updated: `migrate.rs` (the tool's own step is explained, not shown as `$`),
  `i18n.rs` and the catalogue test (placeholder checks moved off the URL).

## Accepted, not changed (with reasons)

- **Support tier shown for an absent provider** (`status`). It is true of the
  machine whether or not the provider is installed, and it is how the Intel
  timeline reaches a user who has not installed Homebrew yet (INV-20 data).
- **`doctor`'s `[OK]` for an absent provider** keeps its severity: adding a
  severity would change the `doctor` schema. The sentence now says "is not
  installed here", which `[OK]` reads correctly against.
- **The gate's per-step reasons stay visible** under `migrate --yes` rather than
  moving behind `--verbose`: they are the honest answer to "why not", and the
  do-it-yourself line now follows them.
- **`mpb` wording** is untouched everywhere: DP-033 (a charter question).
- **`Cargo.toml`'s `repository` field** still names `macportsbrew/macportsbrew`;
  it is package metadata, not output, and changes with DP-031.
- **MacPorts-sourced plans from a recording are refused** by design (DP-016(c));
  the live capture (`fixtures/snapshots/`) is the input that shows the plan.

## Release-candidate checklist (for the owner)

Before tagging `v0.1.0`:

1. **Rule the blockers**: DP-033 (`mpb`, charter line 8), DP-034 (keychain
   passwords in argv), DP-031 (repository account — then update `Cargo.toml`
   `repository` and add the sign job's base-repository condition), DP-035
   (identifier and install directory are permanent), DP-036.
2. **The armed write** (send a message starting `MPB_ALLOW_GUARD_EDIT=1`):
   `scripts/build-release.sh` and `scripts/check-release-workflow.sh` from
   [`09-staged/README.md`](09-staged/README.md) (verify the SHA-256s printed
   there), `.github/workflows/release.yml`, `check-action-pins.sh`'s
   `ALLOWED_REPOS`, the `validate.sh` stage list, and the hook/settings fixes
   listed in `docs/research/tooling/2026-10-10-hooks-permissions-ci.md`.
3. **Version**: `Cargo.toml` already says `0.1.0` and `macportsbrew --version`
   prints it with policy `2026.10.03-2`. Rename `## [Unreleased]` in
   `CHANGELOG.md` to `## [0.1.0] - <date>`.
4. **The three artifacts** (per architecture, x86_64 and arm64): the bare
   executable `macportsbrew-0.1.0-<arch>`; `macportsbrew-0.1.0-<arch>.pkg`,
   installing only `macportsbrew` into `~/.macportsbrew/bin` for the current
   user, no script, no `PATH` edit; `macportsbrew-0.1.0-<arch>.dmg`, HFS+ UDZO,
   holding the executable. All Developer ID signed, notarised (the `.pkg` and
   `.dmg` stapled), with `SHA256SUMS` and attestations.
5. **The checks in [`09-staged/README.md`](09-staged/README.md)'s owner list**,
   including the Standard-account `.pkg` install and the first CI run going
   green on both runners (no CI job has ever run in this repository).

**Manual Gatekeeper test protocol** (on a clean macOS user account, Standard,
not admin; once per artifact and architecture available to you):

1. Download each artifact from the **draft** release in Safari (so it carries
   the quarantine attribute); confirm with `xattr -p com.apple.quarantine <file>`.
2. Verify before opening: `shasum -a 256 -c SHA256SUMS --ignore-missing`, and
   `gh attestation verify <file> --repo <owner>/<repo> --signer-workflow
   <owner>/<repo>/.github/workflows/release.yml --source-ref refs/tags/v0.1.0`
   (on macOS 12 or later; `gh` may not run on older systems).
3. `.pkg`: double-click; expect no administrator prompt and no Gatekeeper
   warning; then `stat -f '%Su %Sp' ~/.macportsbrew ~/.macportsbrew/bin
   ~/.macportsbrew/bin/macportsbrew` (owner = you, `drwxr-xr-x` / `-rwxr-xr-x`)
   and `pkgutil --pkgs | grep macportsbrew`.
4. `.dmg`: double-click; it mounts read-only with one file; copy it out, run
   `./macportsbrew --version`; expect no Gatekeeper dialog.
5. Bare file: `chmod +x`, run `--version` from Terminal with the network on,
   then repeat on a fresh download with the network off and record what macOS
   does (unknown today, R-11 Q3).
6. Record each outcome (macOS version, architecture, dialog text if any) in
   `docs/research/` before publishing the draft.

## Definition of done

- [x] `docs-auditor` finds no surface that overstates the build (second run,
      after the fixes; see "Reviews").
- [x] Walkthrough CLI tests for every fixture, proved by mutation.
- [x] `doctor --report` redaction tested on every section.
- [x] §3 holds.
- [x] `validate.sh` (by the commit gate on every commit) and coverage, 2026-10-11:
      domain 97.37%, policy 97.60%, application 95.59%, adapters 95.25% (floors
      95), cli 92.08% (no floor), overall 95.39% (floor 90).
- [x] The release-candidate checklist is above.

## Reviews

The truth sweep was the first `docs-auditor` run. The end-of-pass reviews are
part of the exhaustive pre-release review that follows this pass; its findings
and their resolution are recorded in that review's report.
