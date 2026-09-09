#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# The one implementation of "valid" (docs/spec/07-quality-and-testing.md,
# "Validation is mechanical, not remembered"). Three entry points call it:
# CI (authoritative), a contributor before pushing, and the agent's Stop
# gate. Do not add a check anywhere else.
#
#   scripts/validate.sh --quick   format, lint, layering, no-shell, links, SPDX,
#                                 action pins
#   scripts/validate.sh           --quick plus the test suite
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
fi
if [ "$mode" = "--full" ]; then
  step "deployment target"; run scripts/check-deployment-target.sh
fi

if [ "$fail" != 0 ]; then
  printf '\nvalidate: FAILED\n' >&2
  exit 1
fi
printf '\nvalidate: ok (%s)\n' "$mode"
