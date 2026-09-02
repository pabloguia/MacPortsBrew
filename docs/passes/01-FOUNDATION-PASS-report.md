# 01-FOUNDATION-PASS — report

Written 2026-09-02 with `/pass-report`, after `/preflight`. On the final tree
the `Stop` gate passed in full (`MPB_GATE_FULL=1`: links, configuration
validity, the no-mutation grep over `crates/*/src`, and `scripts/validate.sh`
with the whole test suite), `.claude/hooks/test-hooks.sh` passed 70 of 70,
`scripts/run-fixture-matrix.sh` passed 157 checks over 6 fixtures, and
`scripts/check-deployment-target.sh` passed on the release builds for both
architectures. The gate was never skipped and no turn ended under the block
cap. Two things this report has to say first:

- **The pass did not run in plan mode.** The work order's sequence (steps 0
  to 21) served as the plan, and `spec-guardian` reviewed the *result* of
  each layer rather than a plan for it. The reviews found real defects
  (listed under Reviewers), which argues the protocol works either way, but
  it is a deviation from "plan, have it reviewed, then implement" and is
  named here so the next pass does not read it as precedent.
- **MacPorts was never observed.** This machine has no MacPorts. Every
  MacPorts fact is cited from official documentation with its date; the
  MacPorts adapter's parse shapes are marked `UNVERIFIED (R-3)` in code and
  its fixtures are *synthesised* with that provenance. The first task on a
  machine with MacPorts is to capture a real fixture and confront the parser
  with it.

## What shipped

A read-only vertical slice a user can run today, as `macportsbrew` or `mpb`:

```
macportsbrew status                machine + provider dashboard
macportsbrew analyze [pkg...]      inventory, classes, mapping, availability, recommendation, reasons
macportsbrew list [--provider p]   one row per package across providers
macportsbrew explain <pkg>         the facts, the rules that fired and did not, the mapping evidence
macportsbrew plan <pkg> --to <p>   a MigrationPlan as data: every command, its elevation, every risk
macportsbrew doctor [--report f]   structural checks; a redacted diagnostic file
macportsbrew snapshot [--out f]    both providers' state, schema-versioned
```

Every command has `--json` with `schemaVersion: 1` (`docs/schemas/`),
`--language` (English plus the `qps` pseudolocale), `--from-snapshot <file>`,
`--advanced` (INV-13), and, hidden, `--replay <recording>` and `--today`.
Exit codes follow `06-cli-and-ux.md`: `plan` exits 4 when it stops at a
manual-review step, `doctor` exits 5 on a broken provider, `--from-snapshot`
of an incompatible schema exits 8, usage errors exit 2.

On this machine (Intel, macOS 26.6.2, Homebrew 6.0.21, MacPorts absent) the
slice reports 189 Homebrew packages of which 56 are top-level, classes the
casks as applications out of scope, marks the 18 formulae without a bottle
for this platform as source builds, surfaces Homebrew's announced end of
Intel support with its source and read date, and answers `plan <pkg> --to
macports` with a plan that stops at manual review because MacPorts is not
installed and no mapping can be confirmed offline.

The binary contains no code path that installs, removes, links, elevates,
writes a shell file, creates a symbolic link or controls a service. Measured
on the final tree with the gate's own patterns: 0 mutating-verb string
literals in `crates/*/src`, 0 elevation, service-control or symbolic-link
tokens in non-comment code, 0 `std::process::Command` sites outside
`crates/adapters/src/process/`. A deliberately broken change (a type error
in the domain crate) was blocked by the gate with exit 2 and the tree
restored; the transcript is in this pass's session, and
`.claude/hooks/test-hooks.sh` reproduces the boundary cases on every run.

## Research findings

Every blocking item is answered in `docs/research/`, sourced and dated
2026-09-02; `docs/spec/09-research-checklist.md` carries the statuses.

| Item | Status | Note | What changed because of it |
|---|---|---|---|
| F-1 Homebrew support tiers | re-verified | `F-1-homebrew-support-tiers.md` | Tier text unchanged; "stopped building Intel bottles" is stated policy, not a literal fact: bottles are frozen at `sonoma` and eroding, 7808 of 8586 formulae pour on Intel macOS 26 against 8420 on Apple Silicon. SE-003. |
| F-2 `/usr/local` interference | partially answered | `F-2-usr-local-interference.md` | Confirmed from the Guide's trace-mode text; `brew doctor` has no MacPorts check at any version; Homebrew mitigates in its compiler shim. SE-005. |
| R-1 Homebrew read interfaces | answered | `R-1-homebrew-read-interfaces.md` | `installed_on_request` is the only intent record and `installed_as_dependency` does not exist in 6.0.21; `HOMEBREW_NO_AUTO_UPDATE` is what makes reads offline; `brew deps` and `brew uses` have no JSON. SE-035. |
| R-2 Homebrew artifact availability | answered | `R-2-homebrew-artifact-availability.md` | The cached index carries only this machine's bottle tag (`tahoe`), so "none published" and "none for this platform" are indistinguishable offline; the tag rule is data. SE-004. |
| R-3 MacPorts read interfaces | answered from documentation | `R-3-macports-read-interfaces.md` | No JSON mode; `--index` field flags are the parse target; several installed rows per name; `requested` and `leaves` pseudo-portnames are the intent record. SE-035. |
| R-4 MacPorts binary archives | answered from documentation | `R-4-macports-binary-archives.md` | Availability is an exact archive-name match; `BINARY_AVAILABLE` is never assertable offline; five distinct source-build causes; installers down to macOS 10.12. SE-004, SE-022. |
| R-5 Privilege requirements | answered | `R-5-privilege-requirements.md` | Nothing in this pass elevates; elevation varies per package (casks with a `sudo:` stanza), not only per provider. SE-016. |
| R-6 Primary-provider mechanisms | answered | `R-6-primary-provider-mechanisms.md` | Neither native mechanism can express a cross-provider choice; the shim is the last of four layers and must never live in `/usr/local/bin`. SE-036. |
| R-7 Language | resolved before the pass | ADR-0002 | Verification list demonstrated; see CI. |
| R-8 Stateful and service packages | answered | `R-8-stateful-and-service-packages.md` | `service != null`, not "has a service key"; a package carries a class *set*; curated data seeds 23 stateful families. SE-006. |
| R-9 Gatekeeper | answered, one item unresolved | `R-9-gatekeeper-and-distribution.md` | Quarantine, not the missing signature, is the blocker; bare binaries cannot be stapled; the behaviour of a quarantined unsigned binary run from a shell on macOS 26 is `UNRESOLVED`. SE-014; DP-004 recommendation. |
| R-10 CI runners | answered | `R-10-ci-runners.md` | `macos-15-intel` and `macos-26-intel` exist and are free on public repositories; macOS 10.12 to 10.15 have no hosted path. SE-037. |

Unresolved, and what each blocks: the quarantined-binary experiment (R-9)
blocks nothing now and reopens DP-004 if it shows a hard kill; the MacPorts
output bytes (R-3) block a trustworthy MacPorts contract test; the Homebrew
internal cache's `service_run_args` schema (R-8) blocks offline service
detection when the JSON is unavailable; whether `GITHUB_TOKEN` can push to
the wiki (R-10) blocks wiki automation.

Tooling was re-verified at the start of the pass (`/tooling-recheck`, three
parallel groups, `docs/research/tooling/2026-09-02-*-foundation.md`). One
defect: the `@claude` workflow had granted `cargo build` and `cargo test` to
an unattended pull-request run through the settings file; fixed. Plan mode
is confirmed not to constrain `Bash`. Recorded in SE-025.

## Decisions

Made in this pass, each with its record:

- **One crate per layer** (`crates/domain`, `policy`, `application`,
  `adapters`, `cli`), so the layering rule is a Cargo dependency edge that
  `scripts/check-layering.sh` asserts from `cargo metadata`, and
  `std::process::Command` is textually confined to `adapters/process/`.
- **Capture, then analyse.** `analyze()` is a pure function of a snapshot;
  the live path captures first. `--from-snapshot` is therefore the same
  code, which is how INV-15 is proven (`scripts/run-fixture-matrix.sh`,
  golden files under `fixtures/golden/`). SE-042 records the two readings
  this forced.
- **Outcomes parameterised by provider id** rather than the provider-named
  vocabulary in `03-policy`; JSON carries `kind` plus ids. SE-004.
- **Provider verbs and elevation as data** (`data/policy/provider-commands.toml`),
  never as string literals in code, so the no-mutation grep stays
  meaningful. SE-020, SE-040.
- **Dependency gate** (ADR-0004): Unicode-3.0 allowed; nine build scripts
  read and allow-listed by name; a new one fails the gate.
- **Fixture provenance** has three values: `captured` (this machine),
  `synthesised` (MacPorts, from documentation), `derived` (edited captures,
  every edit named in the note). `fixtures/recordings/README.md` maps the
  six fixtures to the twenty criteria of `07-quality-and-testing.md`.
- **Recheck recommendations not adopted, with reasons** (SE-025):
  `permissions.disableAutoMode`, a `SubagentStop` gate, and the sandbox
  block, which is written out in `.claude/README.md` for the next session
  to try against the build.
- **Two unrequested artefacts removed.** Two subagents produced
  Codex-oriented copies of the Claude configuration (`.codex/`, `.agents/`)
  that nothing in the work order asked for; both were deleted before commit.

Reserved for the human (`docs/decisions-pending.md`), unchanged in status:
DP-004 release signing (now carrying R-9's recommendation of ad-hoc signing
plus documented quarantine handling; reversible default kept), DP-006
copyright holder (no copyright line anywhere), DP-007 hosting (no remote;
`git push` denied; the CI workflows are written and dormant).

For the human, wording rather than a decision: INV-10's parenthetical
rationale describes `/usr/local` contamination as an environment-variable
problem; R-4 finds MacPorts already sanitises its build environment and the
hazard is a filesystem search-path one. The invariant stands; its rationale
text is Tier 1 and was not edited.

ADRs: ADR-0004 written; ADR-0002's verification list annotated with what is
now demonstrated and its runner bullet answered by R-10.

## Specification changes

All in `docs/spec-evolution.md`; this section indexes.

Made: SE-003, SE-004, SE-005, SE-006, SE-011, SE-014, SE-016, SE-022 and
SE-025 extended; SE-035 (installed rows; ownership mechanisms named),
SE-036 (package primary resolved), SE-037 (CI runners), SE-038 (mapping
confidence `NONE` is not a review item), SE-039 (availability `UNKNOWN`
reviewed only for a reachable destination that does not already hold the
package), SE-040 (deadlines are data, the horizon is code), SE-041
(coexistence decided inside the rule), SE-042 (which side drives a unit;
snapshot analysis as of its capture date), SE-043 (a withdrawn source
package is a review item) added.

Deferred, each with its interface or plan type already in place: `doctor
--deep`; the executor and everything after `PLAN` in the state machine
(`StepKind` and `VerificationCheck` exist; nothing runs them); `history`,
`restore`, `env`, `primary`, `provider-priority`, `providers`
(installation); the cache with TTL (there is no cache: every command
captures fresh, and `--refresh`, `--no-network`, `--dry-run`, `--yes` are
accepted and inert, which the help text says); translations beyond English
(the catalogue, parity check and pseudolocale exist); localised `--help`
text (SE-011); the wiki publishing automation (needs a credential decision,
R-10); the OS-level sandbox (evaluated, block written, not enabled).

Rejected: an engine-level guard that rewrote outcomes after the rules
(SE-041); a provider-id tie-break for the primary (SE-041) and for the
unit's driving side (SE-042); firing manual review on every package with no
counterpart (SE-038) or on every offline analysis (SE-039); pinning GitHub
Actions to commit SHAs from inside a session (no honest network path;
SE-025, unchanged); a `libc` dependency for process-group kill (deferred,
documented in the executor).

## What it does not do

- Executes nothing. `plan` renders; `migrate` does not exist. The
  verification contract exists as `VerificationCheck` values in a plan and
  is not performed.
- Does not consult either provider's full catalogue. Mapping toward a
  destination sees the curated table and the destination's *installed*
  packages; an equivalent that is not installed and not curated is
  reported as `NONE`, not guessed.
- Never asserts `BINARY_AVAILABLE` for MacPorts (R-4: online only). For
  Homebrew it reads the bottle tags the cached index carries for this
  machine.
- Detects MacPorts services by the presence of the provider's launch-daemon
  directory; under replay that directory does not exist, so no fixture
  exercises a MacPorts `SERVICE` class.
- Has no cache, no lock, no history, no state beyond the snapshots the user
  asks for.
- Computes a unit held by both providers from the copy that `PATH` resolves,
  else from the first provider id; in `--advanced` mode revealing dependency
  units can move a row between units, so the top-level count differs by one
  on the both-providers fixture (63 versus 62). Known, not fixed.
- Replays of a recording (`--replay`) are byte-identical except
  `snapshotProvenance.capturedAt`, which the matrix script normalises;
  `--from-snapshot` is byte-identical without normalisation.

## Tests and coverage

456 tests pass across the workspace (domain 17, policy 100, application 28,
adapters 190, cli 121), plus 157 matrix checks and 70 hook cases.

Invariant tests, named after the invariants (`grep -r 'fn invariant_'`):
every one of INV-01 to INV-20 has at least one, in `crates/policy/tests/`,
`crates/adapters/tests/`, `crates/application/tests/` and
`crates/cli/tests/end_to_end.rs` (INV-07 and INV-08 share one at the
application layer, `invariant_07_08_...`, and INV-08 has its own in policy).
INV-11 and INV-12 are type-level tests and say so.

Coverage floors: `scripts/check-coverage.sh` implements the per-layer floors
of `07-quality-and-testing.md` (line coverage; branch coverage needs a
nightly toolchain and is not enforced) and CI runs it. **It did not run in
this pass**: `cargo-llvm-cov` is not on this machine and cannot be added
from an agent session. The floors are therefore asserted by CI once a
remote exists, and unverified until then.

Contract tests: Homebrew against the captured fixture (38 tests, every
assertion on real output); MacPorts against synthesised fixtures (64 tests,
every parse shape marked unverified). Golden files: six, regenerated
deliberately twice in this pass (SE-039 and SE-043 changed outcomes), each
regeneration diffed semantically before acceptance.

## CI

`.github/workflows/repository-validation.yml`: an ubuntu job (hooks,
gate, licence text, generated support matrix, cargo-deny) and a macOS matrix
of `macos-26`, `macos-26-intel`, `macos-15`, `macos-15-intel`, each running
`scripts/validate.sh --full` (release builds for both targets, the
deployment-target assertion on every artifact and every rlib object), the
hook tests and the fixture matrix; a coverage job on `macos-26-intel`. The
support matrix (`docs/support-matrix.md`) is generated from the workflow's
labels and checked in CI.

**CI has not run.** The repository has no remote (DP-007). Every job was
written from R-10's verified labels and exercised locally where the host
allows: the full validation, the deployment-target check and the fixture
matrix passed on this Intel macOS 26 machine; the arm64 artifact was
cross-built and asserted here, not executed. What CI would add that this
pass could not: native arm64 execution, macOS 15 on either architecture,
the coverage floors, cargo-deny's advisory check, and the hook tests on a
second macOS. macOS 10.12 to 10.15 and 11 to 14 are best-effort in the
generated matrix and nothing claims otherwise.

## Reviewers

All three ran on the pass's work; every finding is resolved or accepted
below.

- **spec-guardian** on the policy crate: two violations (an engine guard
  outside the rule set; reason ids without catalogue keys), six missing
  invariant tests, four undeclared readings. All resolved: SE-038 to SE-041,
  nineteen policy invariant tests, catalogue coverage asserted both ways.
- **spec-guardian** on domain, application and CLI: plan commands carried
  the descriptive environment info rather than the sanitised invocation
  environment (INV-10) — resolved with `invariant_10_...` at the application
  layer; the primary step's elevation was unenumerated — resolved; the
  `--json` suppression of two stderr notices — resolved; missing `docs/schemas/`
  — written; six invariants without named tests — written; the
  capture-date reading — recorded as SE-042.
- **security-reviewer**: thirteen findings. Must-fix (home paths and user
  name in live snapshots; the PATH-shadowing audit never wired into doctor;
  truncated provider output not failing) and should-fix (bounded reader
  collection on timeout, non-absolute program refused, replay prefix
  overrides validated and replay-only, runtime argv allow-lists in both
  adapters, `author_association` gate on the `@claude` workflow,
  interrupted reads retried, `provides()` validating its own path, input
  bounds aligned) all resolved. Accepted as informational: the predictable
  temporary-file name in `write_atomic` (denial of service only; `O_EXCL`
  and `rename` make a write-through impossible), the gate not scanning
  `crates/*/tests` (not shipped; the gate's message now says so), and the
  process-group kill that needs `libc` or an equivalent (deferred).
  Pinning Actions to SHAs stays deferred (SE-025).
- **docs-auditor**: ten corrections (sample counts, the bare command's
  behaviour, support wording, the pseudolocale, hidden flags, the peer
  clause in first paragraphs, an R-10 citation, the `--help` exemption now
  in SE-011); clean on links, copyright, support-matrix honesty, i18n
  parity and recipe parity.

## Known limitations and risks discovered

- **MacPorts is entirely documentation-derived.** The adapter, its fixtures
  and its contract tests may be wrong in ways only a real installation can
  show. This is the largest risk in the pass and the first thing to attack.
- **The Homebrew fixture is one machine.** Intel, macOS 26, one user's
  installation. Derived fixtures reshape it; nothing was captured on Apple
  Silicon or on macOS 15.
- **Coverage floors are unverified locally.** See Tests.
- **The engine's `today` matters.** A live analysis uses the real date; a
  snapshot analysis uses the capture date. The deadline rule's weight and
  the staleness notice change with it, by design.
- **`analyze --replay` re-captures**, so the executor's timing tests are the
  only place wall-clock enters the suite; two assertions were loosened to
  20 seconds after a flake under a parallel workspace run.
- **The one place a provider id decides anything**: the driving side of a
  unit held by both providers when no executable resolves (SE-042).
- **`--advanced` shifts one unit** on the both-providers fixture (see What
  it does not do).
- **Assumptions to attack in the architecture review**: that a package unit
  keyed by the source provider's spelling is the right display key; that
  curated-first mapping with installed-only discovery is enough until the
  catalogue pass; that `MANUAL_REVIEW` dominating by outcome (not priority
  number) is the right composition rule; that the class-set precedence
  order is right; that `ProviderInventory` as one value per provider scales
  to a large MacPorts installation (8192-row bound in the parser).

## Recommended next tasks

Architecture-review pass (reviewing, not coding):

1. Challenge the unit model (`analysis.rs`: `source_for`, the merge of
   rows, the `--advanced` count shift) against `02-domain-model.md`
   "Inventory".
2. Challenge the mapping evidence weights and the `CANDIDATE_THRESHOLD`;
   decide whether the destination catalogue (Homebrew's API cache, the
   MacPorts PortIndex) belongs in the next implementation pass.
3. Review the policy composition rule and the fourteen rules' priorities
   against the starting policy in `03`.
4. Decide the sandbox question with `scripts/validate.sh --full` as the
   test (`.claude/README.md`).

Focused implementation passes, each independently verifiable:

1. **Capture a real MacPorts fixture** on a machine with MacPorts
   (`scripts/capture-fixture.sh`), run the contract tests against it, fix
   the parser, and mark the `UNVERIFIED (R-3)` comments resolved. Success:
   a `captured` MacPorts fixture in `fixtures/recordings/` and green
   contract tests against it.
2. **Capture on Apple Silicon** and on macOS 15; retire the derived
   arm64 fixture or keep it beside the captured one.
3. **Destination catalogues offline**: read Homebrew's internal API cache
   and the MacPorts PortIndex through the adapters so mapping sees more than
   installed packages (R-1, R-3). Success: `explain` shows catalogue-backed
   candidates for a package with no installed counterpart.
4. **The executor and `migrate`**, behind INV-01/02: the state machine
   after `PLAN`, the verification contract, history and `restore --plan`.
   Requires the security reviewer on every step and a process-group kill
   decision (an ADR if `libc` is taken).
5. **Locales**: `pt-BR`, `es`, `ja`, `zh-CN` catalogues, the locale
   detection that renders them, and localised help (SE-011).
6. **Coverage in CI**: once the remote exists (DP-007), run the workflow,
   read the coverage floors, and record the real numbers here.
