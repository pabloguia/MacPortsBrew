#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# PostToolUse(Edit|Write). Cheap, per-file, non-blocking.
#
# Every check here is optional-by-tool-presence and skips silently when the
# tool is absent, so the hook works today (Markdown and shell only) and keeps
# working once the implementation language is confirmed. Add the language's
# formatter and linter to the dispatch table below; do not add anything slow -
# the whole-repository gate belongs in stop-gate.sh.

set -uo pipefail
# shellcheck source=lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

mpb_have jq || mpb_have python3 || exit 0
INPUT="$(cat)"
FILE="$(mpb_json_field "$INPUT" "tool_input.file_path")"
[ -n "$FILE" ] && [ -f "$FILE" ] || exit 0

notes=()

case "$FILE" in
  *.sh)
    mpb_have shfmt      && shfmt -w -i 2 -ci "$FILE" >/dev/null 2>&1
    if mpb_have shellcheck; then
      # Output captured in a variable: no predictable file in a shared /tmp.
      if ! sc_out="$(shellcheck -x -P SCRIPTDIR "$FILE" 2>&1)"; then
        notes+=("shellcheck findings in $FILE:"$'\n'"$sc_out")
      fi
    fi
    ;;
  *.rs)
    # Format only. clippy is a whole-crate operation and belongs in the gate,
    # not in a per-file hook that runs after every edit.
    mpb_have rustfmt && rustfmt --edition 2021 "$FILE" >/dev/null 2>&1
    ;;
  *.json)
    if mpb_have python3 && ! python3 -m json.tool "$FILE" >/dev/null 2>&1; then
      notes+=("$FILE is not valid JSON.")
    fi
    ;;
  *.md)
    # Trailing whitespace and tabs make diffs noisy in a docs-heavy repository.
    if grep -nP '[ \t]+$' "$FILE" >/dev/null 2>&1 || grep -n '[ 	]$' "$FILE" >/dev/null 2>&1; then
      notes+=("$FILE has trailing whitespace on at least one line.")
    fi
    # Every internal link must resolve. A broken link in the spec is a real
    # defect: agents navigate this repository by following them.
    while IFS= read -r target; do
      case "$target" in
        http*|"#"*|mailto:*) continue ;;
      esac
      resolved="$(cd "$(dirname "$FILE")" && printf '%s' "${target%%#*}")"
      [ -n "$resolved" ] || continue
      if [ ! -e "$(dirname "$FILE")/${target%%#*}" ]; then
        notes+=("$FILE links to ${target%%#*}, which does not exist.")
      fi
    done < <(grep -oE '\]\([^)]+\)' "$FILE" 2>/dev/null | sed -E 's/^\]\(//; s/\)$//' || true)
    ;;
esac

# SPDX header check, per ADR-0001. Applies to source and script files only.
case "$FILE" in
  *.sh|*.rs|*.py|*.toml)
    if [ "$(basename "$FILE")" != "Cargo.toml" ] && ! head -5 "$FILE" | grep -q 'SPDX-License-Identifier: Apache-2.0'; then
      notes+=("$FILE is missing its 'SPDX-License-Identifier: Apache-2.0' header (ADR-0001; CI fails on this).")
    fi
    ;;
esac

# Exit-0 stderr from a PostToolUse hook is debug output nobody reads. The
# documented way to put non-blocking findings in front of the agent is
# additionalContext on stdout (hooks reference, read 2026-09-02).
if [ ${#notes[@]} -gt 0 ]; then
  mpb_context "PostToolUse" "MacPortsBrew post-edit checks:"$'\n'"$(printf '%s\n' "${notes[@]}")"
fi
exit 0
