#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# GENERATED FILE - do not edit.
#
# Generated from .claude/hooks/arm-guard-edit.sh by scripts/generate-codex-layer.sh.
# `.claude/` is the source of truth (DP-018); edit there and regenerate.
# A `validate.sh` stage fails when this file and its source disagree.
#
# UserPromptSubmit hook. Arms the guarded-path escape from the prompt itself.
#
# `guard-protected-paths.sh` refuses writes into the enforcement layer unless
# `MPB_ALLOW_GUARD_EDIT=1` is in the environment. That works, and it costs the
# human a restart with a variable set for every piece of guard work. This hook
# adds a second way to say the same thing: put the literal string
# `MPB_ALLOW_GUARD_EDIT=1` in a message, and the guard honours it for the work
# that message asks for.
#
# SCOPE: ONE MESSAGE, NOT ONE SESSION
# ----------------------------------
# This event fires on every user message with no matcher (hooks reference, read
# 2026-09-17), so every message rewrites the marker: present with the token,
# deleted without it. The armed window therefore ends at the human's next
# message, and it ends because the boundary is enforced by the same event that
# defines it rather than by an expiry timer that could be wrong.
#
# Two limits on that sentence, because the flat version of it was false when a
# reviewer checked. The window ends at the next message **provided this hook
# runs far enough to delete the marker** - which is why the delete is now the
# first statement in the file rather than the third. And the marker does not
# expire at session end: nothing here runs then, a resumed session keeps its
# `session_id`, so `session-start.sh` deletes the marker instead. Between the
# two, the state does not outlive what justified it; neither alone was enough.
#
# Say what that is and is not. It is NOT per-edit authorisation: one message can
# drive a long autonomous run, and all of it is armed. It is per *work order*.
# What the narrow scope actually buys is not a shorter window but an author: an
# environment variable, by the time the guard reads it, is a property of a
# process and there is nothing to point at. A prompt has a writer, a time and a
# text, which is what lets the guard log WHICH instruction authorised a write
# and lets the end-of-pass report point at something instead of at a memory.
#
# WHAT THIS LOSES, IN FULL
# ------------------------
# The agent cannot forge the environment variable *for this process*. It is set
# before the process starts, and nothing inside a session can reach back and
# change its own environment.
#
# It is NOT true that the variable is beyond reach altogether, and the first
# version of this comment said so. The two settings files whose `env` block
# would reach a hook process are refused by the path guard - to `Edit`, `Write`
# and `NotebookEdit`, and to nothing else. `Bash` does not reach that guard, so
# appending an `env` block to `~/.claude/settings.json` arms **every future
# session**, in the stronger of the two modes, from the weaker route. That gap
# is older than this hook; what is new is that this hook made the claim
# load-bearing, so it is stated rather than left flattering. The same pass
# report once claimed this immunity without checking it, and the guard already
# carries the correction.
#
# **The agent CAN forge this marker.** It is a file, and the agent has a shell.
# `guard-protected-paths.sh` is a PreToolUse hook on Edit|Write|NotebookEdit, so
# it does not see `Bash`, and one redirect writes the marker. The path is in the
# guard's unconditional deny list, which stops the Edit and Write tools and
# stops nothing else. There is no fix available here: verifying the marker means
# reading it, so any secret that would authenticate it has to sit somewhere the
# agent can read too.
#
# So state the change plainly rather than letting the next reader assume the
# stronger property:
#
#   the environment variable is PREVENTION - the agent cannot authorise itself;
#   this marker is DETECTION - the agent can authorise itself, and doing so
#   leaves a file it had to write, a log line the guard appends, and an
#   end-of-pass report whose claim the human can check against a prompt they
#   either typed or did not.
#
# That is a real reduction. It is worth taking only because the disclosure
# obligation is what this escape was always resting on: `MPB_ALLOW_GUARD_EDIT`
# never stopped a determined session either, it stopped an *absent-minded* one
# and made the deliberate act reportable. This keeps the second property and
# gives up part of the first. A human who wants prevention still has the
# variable, and it still works - the guard honours either.
#
# One more thing it loses, smaller and easier to trip over: an occurrence of the
# token the human pasted without meaning it - this file quoted back, a diff, a
# transcript - arms the guard exactly as a deliberate one does, whenever it lands
# unindented. There is no way to tell those apart, and pretending otherwise would
# be this guard overstating itself again. The excerpt in the marker and the log
# is there so the case is at least recognisable afterwards.
#
# AND THE ONE THIS HOOK GOT WRONG, KEPT HERE BECAUSE THE CORRECTION IS THE POINT
# -----------------------------------------------------------------------------
# The first version of this header claimed: "nothing but the human's own message
# can arm it. This hook reads exactly one field, `prompt`. Tool results, file
# contents, provider output and subagent reports never pass through this event
# at all." The first half is true and the second is false, and the escape's own
# first live firing is what proved it: a `spec-guardian` report quoting the
# token while describing this mechanism was relayed into the prompt channel and
# armed the guard. Nobody typed it that turn.
#
# So the accurate statement is narrower. This hook reads exactly one field; what
# it cannot do is assume that field only ever carries what the human wrote,
# because a subagent hand-back arrives through it too. The column-zero rule
# below removes that route by leaning on the harness indenting relayed output -
# a filter, not a boundary, and named as one where it is implemented.
#
# The reason this is written out rather than quietly patched: an agent reading
# this file needs to know that the prompt channel is not a human-only channel,
# and a register of "what the code prevents" is only worth keeping if the
# entries that turned out false stay in it.

# shellcheck source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

TOKEN='MPB_ALLOW_GUARD_EDIT=1'
# Resolved exactly as `guard-protected-paths.sh` resolves it. The two must name
# the same file or the writer and the reader disagree, and the failure is silent
# in the permissive direction for the reader.
PROJECT_ROOT="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
marker="$PROJECT_ROOT/.claude/.guard-edit-marker"

# THE DISARM IS THE FIRST THING THAT RUNS, before the parser check, before the
# read, before anything that can exit. The expiry is only as good as its
# unconditionality: with `mpb_require_json_parser` ahead of it, a message
# arriving while neither `jq` nor `python3` was on PATH exited 2 and left an
# earlier message's marker armed - and then kept it armed for every message
# afterwards, long after the parser came back. That is not a hypothetical in a
# package-manager orchestrator, where a message's own work can relink `jq`. The
# hook's 10s timeout in settings.json produces the same outcome.
rm -f "$marker"

mpb_require_json_parser
input="$(cat)"

# Fail closed, like every other guard here: an unreadable input is not "no
# token". The marker is already gone by this point, so the exit cannot leave one
# behind.
if ! prompt="$(mpb_json_field "$input" "prompt")"; then
  echo "MacPortsBrew hook: the prompt payload was not readable JSON, so this" >&2
  echo "hook cannot tell whether it carries the guard-edit token. Disarming." >&2
  exit 2
fi

# The token must appear on a line that starts at column zero.
#
# THIS RULE EXISTS BECAUSE THE FLAT VERSION WAS FALSE, AND WAS DISPROVED BY THIS
# HOOK'S FIRST LIVE FIRING. The header used to claim that nothing but the
# human's own message reaches this event, so a token in tool output, a file or a
# subagent's report armed nothing. A subagent hand-back is delivered to the
# session *through the prompt channel*: the first real arming of this escape was
# a `spec-guardian` report that quoted the token while describing it, and the
# marker recorded its own provenance - `<agent-message from=...>` - which is how
# it was caught within the minute.
#
# The harness indents every line of relayed model output, so requiring column
# zero removes that route. Say exactly what that is worth: it is a FILTER, not a
# boundary. It rests on a formatting convention of the harness rather than on
# anything this repository controls, and any relay that did not indent would
# arm. It narrows the accident; the log is still what catches the rest, and the
# excerpt it stores is there so a human can tell their own sentence from
# something quoted back at them.
armed=0
while IFS= read -r line; do
  case "$line" in
  " "* | "	"*) continue ;; # indented: relayed or quoted, not a line the human opened
  esac
  case "$line" in
  *"$TOKEN"*)
    armed=1
    break
    ;;
  esac
done <<<"$prompt"
[ "$armed" = "1" ] || exit 0

session="$(mpb_json_field "$input" "session_id")" || session=""
# An excerpt, not the message: enough to recognise which instruction armed the
# guard, and not a copy of whatever else the human happened to be saying.
excerpt="$(printf '%s' "$prompt" | tr '\n\r\t' '   ' | cut -c1-100)"

# Line 1 is the session, line 2 the time, line 3 the excerpt. All three are
# read by the guard: it puts lines 2 and 3 into `ARMED_BY` and writes them into
# the disclosure log, which is why it squashes tabs and newlines out of them
# first. "Free text" would be the wrong word for a field the guard parses and
# re-emits into its own evidence.
# Owner-only: this file holds a hundred characters of what the human typed, and
# the default umask here writes it world-readable. Local exposure only - it is
# git-ignored - but there is no reason for it to be readable by anything but the
# session that wrote it and the guard that reads it.
(
  umask 077
  {
    printf '%s\n' "${session:-unknown}"
    printf '%s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    printf '%s\n' "$excerpt"
  } >"$marker"
)

mpb_context "UserPromptSubmit" \
  "Guarded-path edits are armed for this message only: it carries ${TOKEN}. The
path guard will allow writes into the enforcement layer (.claude/hooks,
.claude/settings*.json, .claude/agents, .claude/skills, scripts/*.sh,
.github/workflows, .codex, .agents/skills) until the next user message, and will
append each one to .claude/.guard-edit-log. docs/spec/11-agentic-sdlc.md
requires the end-of-pass report to disclose what was edited under it and why."
