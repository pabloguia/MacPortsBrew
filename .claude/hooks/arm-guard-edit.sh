#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
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
# defines it rather than by an expiry timer that could be wrong. (session-start.sh
# also deletes the marker on every SessionStart, which fires on `clear` and
# `compact` as well as startup and resume: a compaction in the middle of an
# armed message ends the window early. That fails closed - not armed - and the
# human re-arms by saying the token again.)
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
# version of this comment said so. A settings file's `env` block reaches hook
# processes, so writing one is how a session grants itself the variable - for
# every session after it.
#
# Half of that is now closed and half is not, and the split is deliberate
# (DP-020). `settings.local.json` is refused unconditionally, armed or not,
# because it is git-ignored: the escalation through it would appear in no diff
# at all. `settings.json` stays writable under the escape, because a change to
# it is committed and reviewable, and registering a hook is a thing a session
# legitimately does.
#
# Neither closes the `Bash` route. This guard is PreToolUse on
# Edit|Write|NotebookEdit and never sees a shell, so `cat >> ~/.claude/
# settings.json` still arms every future session in the stronger mode from the
# weaker route. That gap is older than this hook; what this hook did was make
# the claim load-bearing, which is why it is stated rather than left flattering.
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
# because a subagent hand-back arrives through it too.
#
# The first narrowing - require the token at column zero, because the harness
# indents relayed output - was itself false, and was disproved on 2026-09-20 by
# a second hand-back arriving in an envelope that did not indent (F-52). What
# replaces it does not lean on the harness at all: the token must be the FIRST
# THING ON A LINE, and `test-gates.sh` asserts that no line in the tracked
# corpus begins with it. That makes quoting this repository's own text
# insufficient to arm the escape, and makes the assertion fail if a future edit
# would change that. It is still a filter rather than a boundary - see the long
# note at the rule itself for exactly which routes remain open, and DP-022 for
# the two that would close them and what each costs.
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

# Fail closed - but here "closed" is "not armed", which the `rm` above already
# made true. This hook used to exit 2 on an unreadable input or a missing JSON
# parser, and on UserPromptSubmit exit 2 "blocks the prompt, so it never
# reaches Claude" (hooks page, read 2026-10-03): with neither `jq` nor `python3`
# on PATH, every message the owner typed was rejected, and the disarm needed no
# block to hold (tooling recheck D3). So it says why nothing was armed, as
# plain stdout - context for the session - and lets the prompt through.
if ! mpb_have jq && ! mpb_have python3; then
  echo "MacPortsBrew hook: neither jq nor python3 is available, so this message"
  echo "cannot be checked for the guard-edit token. Nothing was armed."
  exit 0
fi
input="$(cat)"

if ! prompt="$(mpb_json_field "$input" "prompt")"; then
  echo "MacPortsBrew hook: the prompt payload was not readable JSON, so whether"
  echo "it carries the guard-edit token cannot be told. Nothing was armed."
  exit 0
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
# THE COLUMN-ZERO RULE WAS ALSO FALSE, AND WAS DISPROVED THE SAME WAY - BY THIS
# HOOK FIRING (F-52, 2026-09-20). It rested on "the harness indents every line
# of relayed model output", which this repository does not own and which a
# second relay shape did not honour: a `docs-auditor` hand-back, commissioned to
# audit the disclosure of an earlier unguarded write and therefore quoting this
# escape's own MUST table, arrived in a `<task-notification>` envelope and armed
# the guard. No human message was in that turn. Again the excerpt is what caught
# it. Twice now this escape has been armed by a reviewer quoting the
# specification that describes the escape.
#
# So the rule is narrowed onto something this repository DOES own: the token
# must be the **first thing on a line**. Every one of the 33 occurrences in the
# tracked corpus is inside prose or backticks, none begins a line, and
# `test-gates.sh` asserts that as a corpus invariant - so no quotation of this
# repository's own text can arm the escape, and the assertion fails if a future
# edit introduces one. That is the difference from what it replaces: a
# convention with a gate behind it rather than a convention of the harness.
#
# Say what it is still not. It is not a boundary. Text that did not come from
# this repository - a paraphrase, a commit message, a web page - can still put
# the token at the start of a line, and nothing here can stop a session that
# means to arm itself, because whatever would authenticate the marker is
# readable by the agent (SE-081). The two routes that would be boundaries both
# spend the owner's workflow and are DP-022, filed rather than chosen here.
case "$prompt" in
*"<task-notification"* | *"<agent-message"* | *"<system-reminder"* | *"[SYSTEM NOTIFICATION"*)
  # A relay envelope. Naming envelopes is a filter and the header says so; this
  # one is here because it is the demonstrated vector, not because the list is
  # believed to be complete. Explained rather than silent: a human who pasted
  # one of these into their own message needs to know why nothing armed.
  case "$prompt" in
  *"$TOKEN"*)
    mpb_context "UserPromptSubmit" \
      "The guard-edit token appeared in a prompt that also carries a relay
envelope, so nothing was armed. A subagent report reaches this hook through the
same channel a human message does, and twice it has armed this escape by quoting
the specification that describes it (F-52). If this was your own message, resend
it without the quoted envelope."
    ;;
  esac
  exit 0
  ;;
esac

armed=0
mentioned=0
while IFS= read -r line; do
  case "$line" in
  "$TOKEN"*)
    armed=1
    break
    ;;
  *"$TOKEN"*) mentioned=1 ;;
  esac
done <<<"$prompt"
if [ "$armed" != "1" ]; then
  if [ "$mentioned" = "1" ]; then
    mpb_context "UserPromptSubmit" \
      "The guard-edit token appeared in this prompt but not at the start of a
line, so nothing was armed - that is the rule that stops a quotation of this
repository's own text from arming the escape (F-52). To arm it, begin a line
with the token."
  fi
  exit 0
fi

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
