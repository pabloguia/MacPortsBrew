#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# LICENSE is the canonical Apache-2.0 body, byte for byte.
#
# SE-021 recorded that decision; ADR-0001 and `check-spdx.sh` cover the *headers*
# in every file, and nothing covered the licence text itself except a step in one
# CI workflow. `CLAUDE.md` is explicit that `scripts/validate.sh` is "the one
# implementation of valid" and "do not add a check anywhere else", and that step
# was somewhere else — on a job that has never run (F-12), carrying its own copy
# of the digest. Two spellings of a constant is one spelling that will be wrong,
# which is the argument `check-action-pins.sh` already makes about the toolchain
# pin (F-45).
#
# The digest lives here now, once. The workflow reaches it by running
# `validate.sh`, the same way it reaches every other check.

set -euo pipefail

ROOT="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
cd "$ROOT"

# sha256 of the Apache License 2.0 body as published by apache.org, recorded
# 2026-09-02 (SE-021).
EXPECTED="cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30"

# `sha256sum` is GNU coreutils and absent from a stock macOS; `shasum` ships with
# macOS and with most Linux distributions. The CI step this replaces used
# `sha256sum` only, which is why it could never have run in the macOS job.
if command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 LICENSE | cut -d' ' -f1)"
elif command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum LICENSE | cut -d' ' -f1)"
else
  echo "licence text: neither shasum nor sha256sum is available; cannot verify LICENSE." >&2
  echo "  Install one rather than skipping: a check that cannot run is not a check." >&2
  exit 2
fi

if [ "$actual" != "$EXPECTED" ]; then
  echo "licence text: LICENSE does not match the canonical Apache-2.0 body." >&2
  echo "  expected $EXPECTED" >&2
  echo "  actual   $actual" >&2
  echo "  SE-021 records that this project ships the licence verbatim. If the" >&2
  echo "  change is deliberate, it is a licensing decision (DP-003) and belongs" >&2
  echo "  with the owner, not in this digest." >&2
  exit 1
fi

echo "licence text: ok"
