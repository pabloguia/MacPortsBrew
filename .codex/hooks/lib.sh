#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# GENERATED FILE - do not edit.
#
# Generated from .claude/hooks/lib.sh by scripts/generate-codex-layer.sh.
# `.claude/` is the source of truth (DP-018); edit there and regenerate.
# A `validate.sh` stage fails when this file and its source disagree.
#
# Shared helpers for MacPortsBrew Claude Code hooks.
#
# Hooks receive one JSON object on stdin. We need exactly one thing from it:
# a named string field. We refuse to parse JSON with sed. Either `jq` or
# `python3` must be present; a guard hook that cannot read its input fails
# closed, because "the parser was missing" must never read as "the command was
# safe" (the same rule the tool itself applies to provider output).

set -euo pipefail

mpb_have() { command -v "$1" >/dev/null 2>&1; }

# mpb_json_field <json-text> <dotted.path>
# Prints the string value, or the empty string when the field is absent.
# Returns 3 when the input is not valid JSON, so a caller can tell "no such
# field" from "could not read the input" - a guard must block on the latter.
mpb_json_field() {
  local json="$1" path="$2" out
  # Empty or whitespace-only input is unreadable, not "a document with no such
  # field". `jq` has no inputs to process there: it exits 0 and prints nothing,
  # which a caller would read as "no command to check" and therefore allow.
  # The same holds for a top-level `null`, which parses but carries no field.
  # Both must fail closed, or the guard's fail-closed property depends on which
  # parser the machine happens to have.
  case "$json" in
    *[![:space:]]*) : ;;
    *) return 3 ;;
  esac
  if mpb_have jq; then
    out="$(printf '%s' "$json" | jq -r --arg p "$path" '
      if type != "object" then error("tool input is not a JSON object") else
        getpath($p | split(".")) // "" | if type == "string" then . else tojson end
      end
    ' 2>/dev/null)" || return 3
    printf '%s' "$out"
  elif mpb_have python3; then
    out="$(printf '%s' "$json" | python3 -c '
import json, sys
try:
    doc = json.load(sys.stdin)
except Exception:
    sys.exit(3)
if not isinstance(doc, dict):
    sys.exit(3)
node = doc
for part in sys.argv[1].split("."):
    if isinstance(node, dict) and part in node:
        node = node[part]
    else:
        sys.exit(0)
sys.stdout.write(node if isinstance(node, str) else json.dumps(node))
' "$path" 2>/dev/null)" || return 3
    printf '%s' "$out"
  else
    return 1
  fi
}

# mpb_field_or_fail_closed <variable-name> <json-text> <dotted.path>
# Sets the named variable in the caller's shell. An unreadable input exits the
# caller with 2 - not a subshell - so the contract does not depend on the
# caller having `set -e`. (A `$(...)` form would exit only the subshell.)
mpb_field_or_fail_closed() {
  local __mpb_value
  if [ "$#" -ne 3 ]; then
    echo "MacPortsBrew hook: mpb_field_or_fail_closed needs <var> <json> <path>. Failing closed." >&2
    exit 2
  fi
  if ! __mpb_value="$(mpb_json_field "$2" "$3")"; then
    echo "MacPortsBrew hook: the tool input was not readable JSON, so this guard" >&2
    echo "cannot tell whether the call is safe. Failing closed." >&2
    exit 2
  fi
  printf -v "$1" '%s' "$__mpb_value"
}

mpb_require_json_parser() {
  if ! mpb_have jq && ! mpb_have python3; then
    echo "MacPortsBrew hook: neither jq nor python3 is available, so this guard" >&2
    echo "cannot inspect the tool call. Failing closed. Install jq (port install jq" >&2
    echo "or brew install jq) and retry." >&2
    exit 2
  fi
}

# Emit a PreToolUse deny decision and exit non-blocking-zero.
# Exit 0 with a decision object is the documented way to deny with a reason.
mpb_deny() {
  local event="$1" reason="$2"
  if mpb_have jq; then
    jq -n --arg e "$event" --arg r "$reason" \
      '{hookSpecificOutput:{hookEventName:$e,permissionDecision:"deny",permissionDecisionReason:$r}}'
  else
    python3 -c '
import json, sys
print(json.dumps({"hookSpecificOutput":{"hookEventName":sys.argv[1],
  "permissionDecision":"deny","permissionDecisionReason":sys.argv[2]}}))
' "$event" "$reason"
  fi
  exit 0
}

# Emit additionalContext for the agent. Documented for SessionStart and
# PostToolUse (shown next to the tool result); on Stop it would continue the
# turn, so Stop uses mpb_message instead.
mpb_context() {
  local event="$1" text="$2"
  # additionalContext is capped at 10,000 characters (hooks reference, read
  # 2026-09-02); keep the tail, which is where a linter's summary line is.
  if [ "${#text}" -gt 9000 ]; then
    text="[truncated to the last 9000 characters]"$'\n'"${text: -9000}"
  fi
  if mpb_have jq; then
    jq -n --arg e "$event" --arg t "$text" \
      '{hookSpecificOutput:{hookEventName:$e,additionalContext:$t}}'
  else
    python3 -c '
import json, sys
print(json.dumps({"hookSpecificOutput":{"hookEventName":sys.argv[1],
  "additionalContext":sys.argv[2]}}))
' "$event" "$text"
  fi
}

# Emit a systemMessage: shown to the human as a warning, does not change the
# hook decision. Used for Stop-gate warnings, which must not prolong the turn.
mpb_message() {
  local text="$1"
  if mpb_have jq; then
    jq -n --arg t "$text" '{systemMessage:$t}'
  else
    python3 -c '
import json, sys
print(json.dumps({"systemMessage":sys.argv[1]}))
' "$text"
  fi
}

# Block the stop with the documented form: exit 0 and a decision object on
# stdout. The exit-2-and-stderr form is read differently when the stderr text
# looks like a missing-file report, and compiler output often does (tooling
# recheck 2026-10-17, item 6). The caller exits 0 after calling this.
mpb_block() {
  local text="$1"
  if mpb_have jq; then
    jq -n --arg r "$text" '{decision:"block",reason:$r}'
  else
    python3 -c '
import json, sys
print(json.dumps({"decision":"block","reason":sys.argv[1]}))
' "$text"
  fi
}

# mpb_realpath <path>: canonical absolute path with symlinks resolved, so a
# guard matches the target and not the spelling.
#
# Returns non-zero and prints nothing when it cannot canonicalise. It used to
# fall back to returning the input unchanged, which is an identity function
# wearing a guard's name: on a machine with jq but no python3 and BSD readlink
# (older macOS - exactly this project's target), a symlinked parent directory
# would then reach the caller un-resolved and evade a prefix check that only
# tests the leaf. A guard that cannot resolve the target cannot say the target
# is safe, which is the same rule mpb_json_field applies to an unreadable input.
mpb_realpath() {
  if mpb_have python3; then
    python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$1" 2>/dev/null && return 0
  fi
  if readlink -f / >/dev/null 2>&1; then
    readlink -f "$1" 2>/dev/null && return 0
  fi
  return 1
}

# mpb_realpath_or_fail_closed <variable-name> <path>
# Sets the named variable in the caller's shell; exits the caller with 2 when
# the path cannot be canonicalised.
mpb_realpath_or_fail_closed() {
  local __mpb_real
  if ! __mpb_real="$(mpb_realpath "$2")"; then
    echo "MacPortsBrew hook: could not canonicalise '$2', so this guard cannot" >&2
    echo "tell what the write would reach. Failing closed. (Install python3, or" >&2
    echo "use a shell whose readlink supports -f.)" >&2
    exit 2
  fi
  printf -v "$1" '%s' "$__mpb_real"
}
