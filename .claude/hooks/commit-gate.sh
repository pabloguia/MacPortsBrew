#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# PreToolUse(Bash): the definition-of-done gate, moved to where it binds.
#
# Every commit in this repository must have `scripts/validate.sh` passing
# (AGENTS.md). Until 2026-10-03 the `Stop` gate ran the full suite at every
# turn end that touched code, which held a turn hostage to work a subagent was
# still doing; and a commit made mid-turn was only checked after it landed.
# This hook runs the check before `git commit` and refuses the commit while it
# fails. The `Stop` gate runs `--quick` only (SE-093).
#
# Mode: the default (tests and the fixture matrix) when the change being
# committed - staged or in the working tree, since `git commit -a` exists -
# touches anything the tests cover; `--quick` for a documentation-only commit.
#
# Disclosed escape, on the same terms as MPB_SKIP_STOP_GATE: set
# MPB_SKIP_COMMIT_GATE=1 in the environment, and say so in the pass report.

set -euo pipefail

. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

mpb_require_json_parser
INPUT="$(cat)"
COMMAND=""
mpb_field_or_fail_closed COMMAND "$INPUT" "tool_input.command"
[ -n "$COMMAND" ] || exit 0

NORM="$(printf '%s' "$COMMAND" | tr '\n\t\r' '   ' | tr -s ' ')"
# `git`, any global options (`-C <dir>`, `-c <k=v>`, `--no-pager`, ...), then
# the `commit` subcommand. `git log --grep commit` does not match.
GIT_COMMIT='(^|[;&|(])[[:space:]]*git([[:space:]]+(-[Cc][[:space:]]+[^[:space:]]+|-[^[:space:]]+))*[[:space:]]+commit([[:space:]]|$)'
printf '%s' "$NORM" | grep -Eq "$GIT_COMMIT" || exit 0

if [ "${MPB_SKIP_COMMIT_GATE:-0}" = "1" ]; then
  mpb_context "PreToolUse" "MPB_SKIP_COMMIT_GATE=1 is set: this commit was not validated. The end-of-pass report must say so."
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-$(pwd)}"
[ -x scripts/validate.sh ] || exit 0
PATH="$HOME/.cargo/bin:$PATH"

mode=--quick
if [ -n "$(git status --porcelain -- crates src tests fixtures locales data scripts .claude Cargo.lock Cargo.toml 2>/dev/null | head -1)" ]; then
  mode=default
fi

if ! out="$(scripts/validate.sh "$mode" 2>&1)"; then
  tail_out="$(printf '%s\n' "$out" | grep -vE '^\s*(Compiling|Checking|Finished|Running|Blocking)' | tail -40 | sed "s#${HOME}#~#g")"
  mpb_deny "PreToolUse" "Commit refused: scripts/validate.sh $mode fails, and every commit here must pass it. Fix the failure, then commit again.
$tail_out"
fi
exit 0
