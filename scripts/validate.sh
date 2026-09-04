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
