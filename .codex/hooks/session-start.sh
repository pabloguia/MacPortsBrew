#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# GENERATED FILE - do not edit.
#
# Generated from .claude/hooks/session-start.sh by scripts/generate-codex-layer.sh.
# `.claude/` is the source of truth (DP-018); edit there and regenerate.
# A `validate.sh` stage fails when this file and its source disagree.
#
# SessionStart. Injects the small amount of state that is genuinely per-session
# and cannot be inferred from a file the agent has not opened yet: which pass is
# running, what is still undecided, and the one boundary that must not be
# crossed. Everything else stays in the specification, where it can be read on
# demand. Keep this output short - it is paid for on every session.

set -euo pipefail
# shellcheck source=lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

ROOT="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
cd "$ROOT"

pass="unknown"
[ -f .claude/current-pass ] && pass="$(tr -d '[:space:]' < .claude/current-pass)"

# "Nothing to report" and "I could not look" must not be the same message.
# Until DP-015 was resolved this file had never had an empty Open section, so
# the fallback below had never rendered - and when it finally did, it told the
# agent its own decision list had failed to parse. Same shape as a gate that
# cannot fail (F-12, and the stop gate's cargo-absent hole).
open_section="$(awk '/^## Open/,/^## Resolved/' docs/decisions-pending.md 2>/dev/null || true)"
if [ -z "$open_section" ]; then
  open_decisions="  (docs/decisions-pending.md has no '## Open' section - read the file)"
else
  open_decisions="$(printf '%s\n' "$open_section" \
    | grep -oE '^### DP-[0-9]+ — [^—]*' | sed 's/^### /  - /' || true)"
  [ -z "$open_decisions" ] && open_decisions="  (none)"
fi

# Only what no file the agent will open already says (11-agentic-sdlc.md):
# which pass, which decisions are open, and where this pass's boundary is.
text="MacPortsBrew session context (injected by .claude/hooks/session-start.sh)

Active pass: ${pass}
  Work order: prompts/${pass}.md, when that file exists. Its section 3 is the
  hard boundary for this pass; the guards in .claude/hooks/ hold regardless of
  pass. If the pass is 'unknown', ask which pass this is before doing anything.

Decisions still reserved for the human (docs/decisions-pending.md):
${open_decisions}

Reading order and working protocol: CLAUDE.md and AGENTS.md, already loaded."

mpb_context "SessionStart" "$text"
exit 0
