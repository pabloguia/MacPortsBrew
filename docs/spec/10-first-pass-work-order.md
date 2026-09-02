# 10 — Foundation pass work order

**Status:** Tier 3.

## Objective

A **read-only vertical slice** that a real user can run today and get value
from, on a foundation that later passes can build on without rework.

Prefer small and correct over large and fragile. Feature count is not the goal.

## Hard boundary

No mutating code path ships in this pass — not behind a flag, not behind a
prompt, not commented as "wired up later". Install, uninstall, `sudo`, shell
file writes, symlink creation and service control do not exist in the binary
this pass produces. Their *types*, *plans* and *tests* do.

## Order of work

0. **Version control.** The repository is not yet under git, by the owner's
   choice (DP-007). Initialise it locally and commit the existing specification
   and agent configuration before changing anything, so this pass's history
   starts from a known state. Pushing to a remote is a separate, explicit
   decision and is denied by `.claude/settings.json`.
1. **Research.** Complete every blocking item in `09-research-checklist.md`.
   Write `docs/research/*.md` with sources and dates. Use `provider-researcher`
   subagents in parallel, one item each.
2. **Toolchain verification.** The language is decided — **Rust**, ADR-0002
   accepted 2026-09-02 — so this step is no longer a decision. Work through that
   ADR's verification list *before* the bootstrap, because each item can
   invalidate a crate choice: build and link for `x86_64-apple-darwin` with
   `MACOSX_DEPLOYMENT_TARGET=10.12` and `aarch64-apple-darwin` with `11.0` and
   confirm from the built artifact that the linker did not silently raise the
   floor; confirm no transitive dependency raises it, with a check rather than
   inspection; verify the candidate TUI crate's East Asian double-width handling
   against CJK fixtures; pin the toolchain in `rust-toolchain.toml`.
3. **Bootstrap.** Repository skeleton, tooling, formatter, linter, CI running
   green on an empty project before there is anything to break.
4. **Process executor.** Argv-only, sanitised environment, timeouts,
   cancellation, full recording. Everything else depends on it.
5. **System detection.** Architecture, macOS version, translation state,
   Command Line Tools.
6. **Domain model.** Provider-neutral types from `02-domain-model.md`.
   No provider names in domain signatures.
7. **Homebrew read adapter.** Detection, capabilities, inventory, requested,
   dependencies, reverse dependencies, artifact availability, metadata.
8. **MacPorts read adapter.** The same contract.
9. **Ownership and class classification.** Including casks, taps, runtimes,
   stateful and service detection.
10. **Snapshot.** Capture, serialise, load. `analyze --from-snapshot`.
11. **Mapping.** Discovery from metadata plus a minimal curated table.
    Confidence and evidence.
12. **Support assessment.** Symmetric artifact availability plus provider
    support state from versioned policy data.
13. **Policy engine.** Rules, reasons, `explain`.
14. **Planner.** Build and render `MigrationPlan` objects. Dry-run output.
    No executor.
15. **Doctor.** The cheap structural checks. `--report`.
16. **UI.** `status`, `analyze`, `list`, `explain`, `plan`, `doctor` — human
    and `--json`. Interactive view if it fits; the flag interface is mandatory,
    the TUI is not.
17. **i18n.** Layer plus the complete English catalogue, pseudolocale, CI parity check.
18. **Tests and fixtures.** Per `07-quality-and-testing.md`.
19. **Docs.** `AGENTS.md` expanded to everything `08-distribution-and-project.md`
    requires of it, `README.md`, `wiki/` foundation, `CONTRIBUTING.md`,
    `SECURITY.md`, issue and PR templates, `docs/spec-evolution.md` kept current.
20. **CI.** Everything from `07`, green.
21. **Agent machinery caught up with the code.** The `Stop` gate runs the real
    validation rather than only the documentation checks it starts with; the
    contributor recipes exist as skills; the deny rules in
    `.claude/settings.json` still map to live invariants. See
    [`11-agentic-sdlc.md`](11-agentic-sdlc.md).

## The slice

```
detect machine
  -> detect providers and capabilities
  -> inventory top-level packages in each
  -> classify ownership and package class
  -> map equivalents with confidence and evidence
  -> assess artifact availability and provider support per package
  -> recommend, with reasons
  -> render a readable report (and --json)
  -> produce a dry-run migration plan for a chosen package
```

## Acceptance

Demonstrate each; do not assert it.

- [ ] Every blocking research item answered in `docs/research/`, sourced and dated.
- [ ] Every item in ADR-0002's verification list is demonstrated, each by a
      check that runs in CI: deployment target asserted on the built artifacts
      for both architectures, no dependency raising the floor, CJK double-width
      verified, toolchain pinned.
- [ ] The slice runs end to end on this machine and produces a correct report.
- [ ] The slice runs end to end against every fixture in the matrix.
- [ ] `--json` on every read command, with a versioned documented schema.
- [ ] `analyze --from-snapshot` is byte-identical across runs (INV-15) and this
      is asserted by a test.
- [ ] One named test per invariant in `01-invariants.md`.
- [ ] Layering check passes: `domain` and `policy` import no adapter.
- [ ] Coverage floors met per layer.
- [ ] i18n key-parity check and pseudolocale layout test pass; CJK column
      alignment verified.
- [ ] Exit codes and the error taxonomy implemented and tested.
- [ ] `LICENSE` present and byte-identical to the canonical Apache-2.0 text.
- [ ] Every source file carries `SPDX-License-Identifier: Apache-2.0`, enforced by lint.
- [ ] Dependency-licence check green; no GPLv2-only dependency.
- [ ] CI green including a build for both architectures.
- [ ] `AGENTS.md` is accurate enough that a fresh agent can make a correct
      change without reading the whole codebase.
- [ ] `grep` finds no install/uninstall/sudo/shell-write code path in the binary.
- [ ] `docs/spec-evolution.md` records every material deviation.
- [ ] `docs/decisions-pending.md` lists every decision left to the human, each
      with options, a recommendation and the reversible default taken meanwhile.
- [ ] `.claude/hooks/stop-gate.sh` runs the real validation suite, and a
      deliberately broken change is demonstrated to be blocked by it.
- [ ] The contributor recipes named in `08-distribution-and-project.md` exist
      both as `.claude/skills/` entries and as `CONTRIBUTING.md` sections.
- [ ] `spec-guardian`, `security-reviewer` and `docs-auditor` have run on the
      pass's work and their findings are resolved or accepted in writing.

## Explicitly deferred

Executor and real migration; source removal; cleanup; primary-provider
implementation; shell configuration writes; provider installation; global
provider priority; `doctor --deep` contamination scanning; translations beyond
English; release signing and publishing; Homebrew formula and MacPorts port for
MacPortsBrew itself.

Each deferred item MUST have its interface and its plan type designed now, so
adding it later is filling in an implementation rather than reshaping the domain.

## Later passes

The architecture-review and focused-implementation passes have their own work
orders; `../../prompts/README.md` lists the sequence. This chapter does not
restate them.

## Priorities when something has to give

Read these as tie-breakers, top to bottom:

1. A correct domain model over feature count.
2. Safe inventory over destructive capability.
3. Bidirectional and symmetric design over a one-way implementation.
4. Safe coexistence over forced migration.
5. Provider isolation over storage efficiency.
6. Top-level packages over dependency manipulation.
7. Machine-readable provider interfaces over text scraping.
8. Dry-run over automatic execution.
9. Explainable policy over cleverness.
10. Deterministic logic over anything adaptive.
11. Testable adapters over concise shell.
12. Safe failure over fast failure.
13. Maintainability over micro-optimisation.
14. Useful documentation over documentation volume.
15. Genuine discoverability over keyword density.
