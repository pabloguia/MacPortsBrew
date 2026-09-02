---
name: preflight
description: Run the MacPortsBrew definition-of-done checks before committing or before reporting a pass complete. Verifies links, config validity, the no-mutation boundary, invariant test coverage and specification currency, and reports what it could not check.
allowed-tools: Read, Grep, Glob, Bash(bash -n *), Bash(shellcheck *), Bash(python3 -m json.tool *), Bash(git status*), Bash(git diff*), Bash(grep *), Bash(cargo fmt*), Bash(cargo clippy*), Bash(cargo test*), Bash(cargo tree*), Bash(otool -l *), Bash(vtool -show *), Bash(${CLAUDE_PROJECT_DIR}/.claude/hooks/stop-gate.sh), Bash(${CLAUDE_PROJECT_DIR}/.claude/hooks/test-hooks.sh)
license: Apache-2.0
---

# Preflight

Demonstrate, do not assert. Every line below is either checked or reported as
unchecked. "Probably fine" is not a result.

## 1. The mechanical gate

Run `${CLAUDE_PROJECT_DIR}/.claude/hooks/stop-gate.sh` and read its output. It
covers internal link resolution, JSON and shell validity, the grep-checkable
no-mutation boundary, and specification-currency warnings. It is the same gate
that runs when your turn ends, so running it now costs nothing and saves a
round trip.

If anything under `.claude/hooks/` or `.claude/settings.json` changed, also run
`${CLAUDE_PROJECT_DIR}/.claude/hooks/test-hooks.sh`: it feeds every hook its
fixture inputs and asserts the decisions, including the fail-closed paths.

## 2. What the gate cannot check - do these by hand

**Invariant coverage.** For every invariant in `docs/spec/01-invariants.md` the
change touches, point at the test named after it. Not "there are tests" - the
specific test, by name. If it does not exist, the change is incomplete.

**Undeclared deviation.** Diff your change against `docs/spec/`. Anything
material and unrecorded goes into `docs/spec-evolution.md` now, with evidence,
using the `/spec-evolution` skill. This is the single failure mode the project
treats as unrecoverable.

**Provider claims.** Every factual statement you added about Homebrew or
MacPorts carries its command or its URL and a date, or is phrased
conditionally, or is runtime detection. Grep your diff for provider names and
check each hit.

**Symmetry.** Does the change give one migration direction a code path the
other lacks? INV-17 makes that a design defect, not a detail.

**Deployment floor.** If you added or updated a dependency, it clears three
gates, not one: an Apache-2.0-compatible licence (ADR-0001), a deployment floor
no higher than 10.12 / 11.0 (ADR-0002), and an `unsafe` and `build.rs` surface
proportionate to a binary that performs privileged operations. Check the built
artifact's recorded minimum OS version rather than the build configuration — the
linker is what decides, and it can raise the floor silently.

**Determinism.** Anything you added to the analysis path that reads a clock,
the environment, a hash order or an unsorted map iteration breaks INV-15.
`analyze --from-snapshot` must be byte-identical across runs.

## 3. Delegate the reviews you should not do yourself

Run these in parallel and read what comes back before you report:

- `spec-guardian` on the diff - invariants, tiers, undeclared deviation.
- `security-reviewer` if the change touches the process executor, path
  resolution, elevation, the state directory, provider output parsing,
  packaging or dependencies.
- `docs-auditor` if anything user-visible or documented changed.

You are not the right reviewer of your own change, and a fresh context finds
what a long one has stopped seeing.

## 4. Report

State what passed, what failed, what you fixed, and - explicitly - what you did
not check and why. A preflight that reports no scope is worth nothing.
