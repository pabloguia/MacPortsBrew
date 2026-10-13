#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# The memory files stay short.
#
# docs/spec/11-agentic-sdlc.md states this as a MUST with a hard number -
# "under 200 lines each" - specifically so it is checkable, in the chapter that
# argues for checkability. Nothing checked it, and AGENTS.md had been two lines
# over since the Foundation Pass expanded it.
#
# The number is not sacred. What the rule is about is that adherence to a long
# instruction file falls off, and that this repository's real knowledge lives in
# docs/spec/, loadable a chapter at a time. So if 200 is wrong, raise it in the
# chapter with a reason - do not raise it here.

set -uo pipefail
status=0
limit=200

for f in AGENTS.md CLAUDE.md; do
  if [ ! -f "$f" ]; then
    echo "memory-files: $f is missing; docs/spec/08-distribution-and-project.md requires both." >&2
    status=1
    continue
  fi
  n="$(wc -l < "$f" | tr -d ' ')"
  if [ "$n" -ge "$limit" ]; then
    echo "memory-files: $f is $n lines; the limit is under $limit." >&2
    echo "  Content that has grown into a procedure belongs in a .claude/skills/" >&2
    echo "  entry; a fact about the domain belongs in docs/spec/." >&2
    status=1
  fi
done

# CLAUDE.md is a thin layer over AGENTS.md and must actually import it, or the
# tool-neutral document is authoritative in the specification and nowhere else
# (SE-024). Claude Code reads CLAUDE.md; it does not read AGENTS.md.
if [ -f CLAUDE.md ] && ! grep -q '@AGENTS\.md' CLAUDE.md; then
  echo "memory-files: CLAUDE.md does not import @AGENTS.md, so the canonical" >&2
  echo "  orientation document is not loaded in a Claude Code session (SE-024)." >&2
  status=1
fi

[ "$status" = 0 ] && echo "memory-files: ok (AGENTS.md $(wc -l < AGENTS.md | tr -d ' '), CLAUDE.md $(wc -l < CLAUDE.md | tr -d ' ') lines, import present)"
exit "$status"
