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

# Every way this hook can fail to validate refuses the commit. A failed `cd`
# under `set -e` used to exit 1 and a missing `validate.sh` exit 0, and Claude
# Code proceeds on both - "exit code 1 [is] a non-blocking error" (hooks page,
# read 2026-10-03) - so the commit landed unvalidated (tooling recheck D2).
ROOT="${CLAUDE_PROJECT_DIR:-$(pwd)}"
if ! cd "$ROOT" 2>/dev/null; then
  mpb_deny "PreToolUse" "Commit refused: the commit gate could not enter the project directory, so it cannot validate this commit."
fi
if [ ! -x scripts/validate.sh ]; then
  mpb_deny "PreToolUse" "Commit refused: scripts/validate.sh is missing or not executable, so this commit cannot be validated. Restore it, or set MPB_SKIP_COMMIT_GATE=1 and disclose it in the pass report."
fi
PATH="$HOME/.cargo/bin:$PATH"

mode=--quick
if [ -n "$(git status --porcelain -- crates src tests fixtures locales data scripts .claude Cargo.lock Cargo.toml 2>/dev/null | head -1)" ]; then
  mode=default
fi

# A PreToolUse hook that reaches its own timeout does not block the tool call
# (same page), so a validation slower than the 1500 s the settings give this
# hook would let the commit through (D2b). The run is bounded below that, its
# whole process group killed at the deadline, and the deadline refuses. Perl
# is the watchdog because it ships with macOS and every CI image, and
# coreutils' `timeout` does not.
#
# `|| rc=$?` on the same line, not `rc=$?` after it: under `set -e` a failing
# command substitution would end the script there with exit 1 - which lets the
# commit through, the D2 defect again (it did, in the first draft of this fix).
DEADLINE="${MPB_COMMIT_GATE_DEADLINE:-1380}"
rc=0
out="$(perl -e '
  my $limit = shift;
  my $pid = fork();
  die "fork: $!" unless defined $pid;
  if ($pid == 0) { setpgrp(0, 0); exec @ARGV or die "exec: $!"; }
  $SIG{ALRM} = sub { kill "TERM", -$pid; sleep 2; kill "KILL", -$pid; exit 124 };
  alarm $limit;
  waitpid($pid, 0);
  exit($? >> 8);
' "$DEADLINE" scripts/validate.sh "$mode" 2>&1)" || rc=$?
if [ "$rc" -eq 124 ]; then
  mpb_deny "PreToolUse" "Commit refused: scripts/validate.sh $mode did not finish within ${DEADLINE}s, and a hook that runs out of time lets the commit through. Run it by hand to see why it is slow, then commit again."
fi
if [ "$rc" -ne 0 ]; then
  tail_out="$(printf '%s\n' "$out" | grep -vE '^\s*(Compiling|Checking|Finished|Running|Blocking)' | tail -40 | sed "s#${HOME}#~#g")"
  mpb_deny "PreToolUse" "Commit refused: scripts/validate.sh $mode fails, and every commit here must pass it. Fix the failure, then commit again.
$tail_out"
fi
exit 0
