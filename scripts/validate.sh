#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# The one implementation of "valid" (docs/spec/07-quality-and-testing.md,
# "Validation is mechanical, not remembered"). Three entry points call it:
# CI (authoritative), a contributor before pushing, and the agent's Stop
# gate. Do not add a check anywhere else.
#
#   scripts/validate.sh --quick   fourteen stages: cargo fmt, cargo clippy,
#                                 layering, no-shell, links, SPDX, dependency
#                                 licences, i18n parity, action pins, memory
#                                 files, licence text, support matrix, codex
#                                 layer, shellcheck
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

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

mode="${1:-default}"
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
# These two ran only in the workflow, which has never run (F-12). CLAUDE.md
# says this file is "the one implementation of valid" and to "not add a check
# anywhere else"; they were somewhere else, and the licence one carried its
# own copy of the digest (F-45).
step "licence text"; run scripts/check-licence-text.sh
step "support matrix"; run scripts/support-matrix.sh --check
# `.codex/` and `.agents/skills/` are generated from `.claude/` (DP-018).
# Without this stage the generator exists and nothing runs it, which is the
# shape of the drift it was written to stop (F-42).
step "codex layer";  run scripts/generate-codex-layer.sh --check
# Shellcheck ran only in CI and in the post-edit hook, which reports rather
# than fails - so a directive on the wrong line left the workflow's first
# step exiting 1, unseen, because CI has never run (F-44, F-12). Same
# invocation as the workflow, so the two cannot disagree.
step "shellcheck";   run shellcheck -x -P SCRIPTDIR .claude/hooks/*.sh scripts/*.sh

if [ "$mode" != "--quick" ]; then
  step "cargo test"; run cargo test --locked --all-features --workspace
  # After `cargo test`, deliberately: it needs target/debug/macportsbrew and
  # exits 2 without it. Nine seconds, so it stays out of --quick (F-45).
  step "fixture matrix"; run scripts/run-fixture-matrix.sh
fi
if [ "$mode" = "--full" ]; then
  step "deployment target"; run scripts/check-deployment-target.sh
fi

if [ "$fail" != 0 ]; then
  printf '\nvalidate: FAILED\n' >&2
  exit 1
fi
printf '\nvalidate: ok (%s)\n' "$mode"
