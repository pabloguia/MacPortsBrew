#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# The one implementation of "valid" (docs/spec/07-quality-and-testing.md,
# "Validation is mechanical, not remembered"). Three entry points call it:
# CI (authoritative), a contributor before pushing, and the agent's Stop
# gate. Do not add a check anywhere else.
#
#   scripts/validate.sh --quick   seventeen stages: cargo fmt, cargo clippy,
#                                 layering, no-shell, links, SPDX, dependency
#                                 licences, i18n parity, action pins, memory
#                                 files, licence text, support matrix, codex
#                                 layer, no secrets, release workflow,
#                                 the shellcheck lint, gate tests
#
#                                 This list drifted from the code, and so did the
#                                 copies in CLAUDE.md and CONTRIBUTING.md - four
#                                 spellings of one thing, three of them wrong by
#                                 five stages. Those two now point here instead
#                                 of repeating it; this one is beside the code it
#                                 describes and is the only place to update.
#   scripts/validate.sh           --quick plus cargo test and the fixture matrix
#   scripts/validate.sh --full    everything, plus the release build for both
#                                 architectures with the deployment-target
#                                 assertion (macOS only; needs otool)
#
#   Every mode ends with scripts/prune-target.sh, which trims target/ and
#   never fails the run: housekeeping, not one of the stages above.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

mode="${1:-default}"
# The overrides the gates honour for their own tests and for the release
# workflow never steer this run: it validates this tree and the binary it
# builds (pass 15 review A1, L3).
unset MPB_FIXTURE_BIN MPB_SECRETS_ROOT
fail=0
step() { printf '\n== %s\n' "$1"; }
run()  { if ! "$@"; then fail=1; fi; }

step "cargo fmt";    run cargo fmt --all --check
step "cargo clippy"; run cargo clippy --locked --all-targets --all-features --workspace -- -D warnings
step "layering";     run scripts/check-layering.sh
step "no-shell";     run scripts/check-no-shell.sh
step "links";        run scripts/check-links.sh
step "spdx";         run scripts/check-spdx.sh
step "licences";     run scripts/check-licences.sh
step "i18n";         run scripts/check-i18n-parity.sh
step "action pins";  run scripts/check-action-pins.sh
step "memory files"; run scripts/check-memory-files.sh
# These two ran only in the workflow, which had never run then (F-12; CI first
# ran on 2026-10-21). CLAUDE.md
# says this file is "the one implementation of valid" and to "not add a check
# anywhere else"; they were somewhere else, and the licence one carried its
# own copy of the digest (F-45).
step "licence text"; run scripts/check-licence-text.sh
step "support matrix"; run scripts/support-matrix.sh --check
# `.codex/` and `.agents/skills/` are generated from `.claude/` (DP-018).
# Without this stage the generator exists and nothing runs it, which is the
# shape of the drift it was written to stop (F-42).
step "codex layer";  run scripts/generate-codex-layer.sh --check
# No signing credential is tracked (08, ADR-0005; pass 09 plan, "The
# secret-material check"). Static and about a second, so --quick.
step "no secrets";   run scripts/check-no-secrets.sh
# The release workflow's textual MUSTs (08, ADR-0005; pass 09 plan, D-T2),
# including that a release builds only through build-release.sh, with no
# feature flag (SE-102).
step "release workflow"; run scripts/check-release-workflow.sh
# Shellcheck ran only in CI and in the post-edit hook, which reports rather
# than fails - so a directive on the wrong line left the workflow's first
# step exiting 1, unseen, because CI had never run (F-44, F-12). Same
# invocation as the workflow, so the two cannot disagree.
step "shellcheck";   run shellcheck -x -P SCRIPTDIR .claude/hooks/*.sh scripts/*.sh

# The gates' own constructed-input tests. Two of the parsers here were shipped,
# passed against the tree, and found broken afterwards by someone building an
# input they should have rejected (F-47, DP-016). Static in cost - no cargo, no
# binary - so it belongs in --quick, where a gate that stops being able to say
# no is caught on the turn that breaks it rather than in a later review.
#
# Maps to 07-quality-and-testing.md, "Validation is mechanical, not remembered".
# 11-agentic-sdlc.md makes it a MUST that every check in the Stop gate names a
# line it enforces, and a gate nothing exercises is remembered validation
# wearing a mechanical gate's name.
step "gate tests";   run scripts/test-gates.sh

if [ "$mode" != "--quick" ]; then
  step "cargo test"; run cargo test --locked --all-features --workspace
  # After `cargo test`, deliberately: it needs target/debug/macportsbrew and
  # exits 2 without it. Nine seconds, so it stays out of --quick (F-45).
  step "fixture matrix"; run scripts/run-fixture-matrix.sh
fi
if [ "$mode" = "--full" ]; then
  step "deployment target"; run scripts/check-deployment-target.sh
fi

# Housekeeping, not a check (SE-100): after every build this mode made and
# after the verdict is printed, never through `run`, so it cannot change the
# verdict. Without it target/deps grew to 605,518 entries and every test
# binary stalled in exec (2026-10-17). It never runs `cargo clean`.
prune() { step "prune target"; scripts/prune-target.sh || true; }

if [ "$fail" != 0 ]; then
  printf '\nvalidate: FAILED\n' >&2
  prune
  exit 1
fi
printf '\nvalidate: ok (%s)\n' "$mode"
prune
