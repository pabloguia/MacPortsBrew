#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
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

open_decisions="$(awk '/^## Open/,/^## Resolved/' docs/decisions-pending.md 2>/dev/null \
  | grep -oE '^### DP-[0-9]+ — [^—]*' | sed 's/^### /  - /' || true)"

# Only what no file the agent will open already says (11-agentic-sdlc.md):
# which pass, which decisions are open, and where this pass's boundary is.
text="MacPortsBrew session context (injected by .claude/hooks/session-start.sh)

Active pass: ${pass}
  Work order: prompts/${pass}.md, when that file exists. Its section 3 is the
  hard boundary for this pass; the guards in .claude/hooks/ hold regardless of
  pass. If the pass is 'unknown', ask which pass this is before doing anything.

Decisions still reserved for the human (docs/decisions-pending.md):
${open_decisions:-  (none parsed - read the file)}

Reading order and working protocol: CLAUDE.md and AGENTS.md, already loaded."

mpb_context "SessionStart" "$text"
exit 0
