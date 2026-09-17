#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# PreToolUse(Edit|Write|NotebookEdit) guard.
#
# Denies writes into the paths that matter to *this* project: the two
# providers' prefixes, shell configuration, and system and launch machinery.
# Claude Code already protects .git and .claude. It is a deny list, not an
# allow list: writes elsewhere still go through the permission prompt.
#
# The path is canonicalised before matching - symlinks resolved, `..`
# collapsed, `//` normalised - so the guard matches the target, not the
# spelling (threat model: never follow a symlink when writing). A file that
# is itself a symlink is refused outright: the tool would write through it.

set -euo pipefail
# shellcheck source=lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

mpb_require_json_parser
INPUT="$(cat)"
FILE=""
mpb_field_or_fail_closed FILE "$INPUT" "tool_input.file_path"
[ -n "$FILE" ] || mpb_field_or_fail_closed FILE "$INPUT" "tool_input.notebook_path"
[ -n "$FILE" ] || exit 0

# The two ways the enforcement-layer escape can be open, and they are not
# equally strong. `MPB_ALLOW_GUARD_EDIT=1` in the environment is set before this
# process exists and cannot be reached from inside the session: prevention. The
# marker is a file written by `arm-guard-edit.sh` from the human's own message,
# and a session with a shell can write it too: detection, not prevention. The
# full statement of what that gives up is in `arm-guard-edit.sh`'s header, and
# it belongs there rather than here because the hook that creates the weakness
# is the honest place to describe it.
#
# The marker is bound to the session that armed it, so one left by a *different*
# session arms nothing. Say the limit with it: a resumed session keeps its
# `session_id`, so a marker left on disk by an earlier run of the same session
# does still arm it - which is why `session-start.sh` removes the marker, rather
# than this comparison being the only thing standing between the two runs.
#
# A symlink here is refused outright. `[ -f ]` follows one, and the marker's own
# place in the unconditional deny list below only stops Edit and Write from
# touching *that path*: pointed at a file those tools will happily write, the
# marker becomes forgeable without Bash, which is a weaker requirement than the
# one this escape's limitations are written around.
PROJECT_ROOT="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
MARKER="$PROJECT_ROOT/.claude/.guard-edit-marker"
GUARD_EDIT_LOG="$PROJECT_ROOT/.claude/.guard-edit-log"
ARMED_BY=""
if [ "${MPB_ALLOW_GUARD_EDIT:-0}" = "1" ]; then
  ARMED_BY="the MPB_ALLOW_GUARD_EDIT=1 environment variable"
elif [ -f "$MARKER" ] && [ ! -L "$MARKER" ]; then
  SESSION=""
  mpb_field_or_fail_closed SESSION "$INPUT" "session_id"
  MARKER_SESSION="$(sed -n '1p' "$MARKER")"
  if [ -n "$SESSION" ] && [ "$MARKER_SESSION" = "$SESSION" ]; then
    ARMED_BY="a prompt in this session at $(sed -n '2p' "$MARKER") reading: $(sed -n '3p' "$MARKER")"
  fi
fi

# Every write that the escape lets through is appended here, because the
# obligation this escape carries is disclosure and a report that cites nothing
# is the agent's memory wearing a record's clothes. Both routes are logged: the
# environment variable has the same reporting duty and never had a trail.
# `check` runs twice, on the path as written and on its canonical form, so this
# guards against logging one write as two. A disclosure log that double-counts
# is one a report overstates from, which is the failure it exists to prevent.
DISCLOSED=0
disclose() {
  if [ "$DISCLOSED" = "1" ]; then
    return 0
  fi
  DISCLOSED=1
  # Both fields are squashed to one line. The path is attacker-shaped - macOS
  # allows a tab or a newline in a filename, and `*` in the deny patterns spans
  # both - so an unescaped path can close its own record and open a complete,
  # plausible, fabricated one after it. `ARMED_BY` carries marker lines 2 and 3,
  # which an agent that wrote the marker chose. A record that can be forged by
  # the thing it records is not a record.
  local when path armed
  when="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
  path="$(printf '%s' "$1" | tr '\n\r\t' '   ')"
  armed="$(printf '%s' "$ARMED_BY" | tr '\n\r\t' '   ')"
  # Fail CLOSED when the write cannot be recorded. Once a session can arm this
  # escape from a prompt, the log is not bookkeeping beside the control - it is
  # the control, because detection is all that is left (see arm-guard-edit.sh).
  # An agent with Bash can make this append fail in a second - replace the log
  # with a directory, a symlink to /dev/null, or a read-only file - and a
  # swallowed failure would then buy a *silent* enforcement-layer write, which
  # is strictly worse than the deny it replaced. Same rule `mpb_json_field` and
  # `mpb_realpath` already apply: a guard that cannot tell cannot permit.
  # The subshell is load-bearing: a failed `>>` is reported by the *shell*, not
  # by `printf`, so `printf ... 2>/dev/null` does not suppress it and the
  # message lands on this hook's stderr - next to the JSON decision, which the
  # caller then cannot parse. Redirecting the whole compound command catches the
  # shell's own diagnostic too.
  if ! (printf '%s\t%s\t%s\n' "$when" "$path" "$armed" >>"$GUARD_EDIT_LOG") 2>/dev/null; then
    mpb_deny "PreToolUse" "Blocked: this write into the enforcement layer is permitted by the disclosed escape, but it could not be appended to $GUARD_EDIT_LOG, so it would happen with no record. docs/spec/11-agentic-sdlc.md requires the end-of-pass report to account for these writes, and an unrecordable one cannot be accounted for. Check that the path is a writable regular file, then retry."
  fi
}

if [ -L "$FILE" ]; then
  mpb_deny "PreToolUse" "Blocked: $FILE is a symbolic link, and a write would follow it to $(mpb_realpath "$FILE"). The threat model in docs/spec/04-migration-and-safety.md says never write through a symlink; write to the real path if that is what you mean."
fi

REAL=""
mpb_realpath_or_fail_closed REAL "$FILE"

# macOS's default volume is case-insensitive, so /opt/Homebrew/etc/x and
# /opt/homebrew/etc/x are the same file while `case` distinguishes them.
shopt -s nocasematch

check() {
  case "$1" in
    /opt/local/*|/usr/local/*|/opt/homebrew/*|*/.homebrew/*|*/.linuxbrew/*)
      mpb_deny "PreToolUse" "Blocked: $1 is inside a package manager's prefix. MacPortsBrew never rewrites, re-links or patches anything a provider owns - docs/spec/04-migration-and-safety.md, 'Cross-provider contamination'. Report the finding and recommend the provider-native repair instead." ;;
    */.zshrc|*/.zprofile|*/.zshenv|*/.zlogin|*/.bashrc|*/.bash_profile|*/.bash_login|*/.profile|*/.config/fish/*|.zshrc|.zprofile|.zshenv|.zlogin|.bashrc|.bash_profile|.bash_login|.profile)
      mpb_deny "PreToolUse" "Blocked: $1 is a shell configuration file. See docs/spec/05-environment-and-primary-provider.md - shell files are only ever written through the managed-block procedure, never by an agent editing a dotfile." ;;
    */Library/LaunchAgents/*|*/Library/LaunchDaemons/*|/Library/*|/System/*|/private/etc/*|/etc/*)
      mpb_deny "PreToolUse" "Blocked: $1 is system or launch machinery. INV-19 and the threat model in docs/spec/04-migration-and-safety.md: services and system configuration are never touched as a side effect." ;;

    # The pin that holds the deployment floor. The rustup rule in
    # guard-provider-mutation.sh tells the agent to change the file instead of
    # the active toolchain, which made the file the unguarded path to the same
    # outcome.
    */rust-toolchain.toml|rust-toolchain.toml)
      mpb_deny "PreToolUse" "Blocked by ADR-0002. rust-toolchain.toml holds the pin that keeps the macOS 10.12 / 11.0 deployment floor - a product commitment, not a build detail. Raising it is a breaking change needing an ADR and an owner decision, so it goes through docs/adr/ and a human. Reading the file is fine." ;;

    # Tier 1. docs/spec/01-invariants.md says plainly that an agent may not
    # change it; until now nothing said so except the file itself.
    */docs/spec/01-invariants.md)
      mpb_deny "PreToolUse" "Blocked: docs/spec/01-invariants.md is Tier 1 and may not be changed by an agent - it says so in its own first lines. If an invariant looks wrong or unimplementable, stop and raise it with the owner; an owner-authorised change is made outside a session and recorded in docs/spec-evolution.md." ;;
  esac

  # The marker and its log, refused whatever is armed - including by a session
  # the marker itself armed, which would otherwise be a session writing its own
  # next authorisation. Outside the escape for the same reason the generator is:
  # the hook that legitimately writes these uses a shell redirect, and this is a
  # PreToolUse hook on Edit|Write|NotebookEdit, so refusing here costs the
  # writer nothing. It also stops nothing else - `Bash` does not reach this
  # hook, which is the limitation `arm-guard-edit.sh` states in full.
  case "$1" in
    */.claude/.guard-edit-marker|*/.claude/.guard-edit-log)
      mpb_deny "PreToolUse" "Blocked: $1 is the guarded-path escape's own marker or its disclosure log, written by .claude/hooks/arm-guard-edit.sh from the human's message and read by this guard. Editing it would be a session authorising its own writes, or rewriting the record of having done so. This refusal covers the Edit, Write and NotebookEdit tools and nothing else - it is not a claim that the file cannot be reached; see arm-guard-edit.sh for what that does and does not buy. Arm the escape by putting MPB_ALLOW_GUARD_EDIT=1 in a message, or start the session with it set in the environment." ;;
  esac

  # The control protecting itself. An agent whose gate keeps failing does not
  # need bad intent to disable it, and the gate can only detect that its own
  # path changed - never prevent it. The escape is deliberate and is disclosed:
  # using it obliges the end-of-pass report to say so, exactly like
  # MPB_SKIP_STOP_GATE. It is armed either by `MPB_ALLOW_GUARD_EDIT=1` in the
  # environment or by that same literal in the human's message, which
  # `arm-guard-edit.sh` turns into a marker naming the prompt responsible.
  #
  # The pattern list is matched once and the escape is tested inside it, rather
  # than the list appearing under both an armed and an unarmed branch. Two
  # copies of a deny list is the shape that let `generate-codex-layer.sh` go
  # unguarded while two of its neighbours were named (F-45), and a copy that
  # drifts here would log a write it did not permit, or permit one it did not
  # log.
  case "$1" in
      # `.codex/**` is here because it is generated from `.claude/` (DP-018)
      # and a hand-edit there is a divergence that the generator will silently
      # overwrite. The generator itself is unaffected: it writes through a
      # shell redirect, and this hook is PreToolUse on Edit|Write|NotebookEdit.
      # `.agents/skills/**` is generated the same way, for the same reason.
      # `*/scripts/*.sh` rather than `validate.sh` plus `check-*.sh`: every
      # script in that directory is a gate now, and naming two of them left
      # `generate-codex-layer.sh` unguarded - the one sanctioned writer into
      # `.codex/`, which *is* guarded. Editing the transform and then running
      # it wrote arbitrary content into a tree a Codex session executes, and
      # `--check` called the result correct, with no denied tool call
      # anywhere in the sequence. `support-matrix.sh` and
      # `run-fixture-matrix.sh` were unguarded for the same reason and became
      # validate.sh stages in F-45.
      #
      # `.claude/agents/` and `.claude/skills/` are the generator's other
      # inputs. Instruction text rather than code, but they reach a generated
      # tree that is itself guarded, and an input weaker than its output is
      # the asymmetry backwards.
      #
      # `settings.local.json` is here because the pass report claimed the
      # agent *could not* set `MPB_ALLOW_GUARD_EDIT` itself. It could: a
      # settings file's `env` block reaches hook processes and only the
      # committed `settings.json` was matched. The report is corrected and so
      # is the pattern.
    */.claude/hooks/*|*/.claude/settings.json|*/.claude/settings.local.json|*/.claude/agents/*|*/.claude/skills/*|*/scripts/*.sh|*/.github/workflows/*|*/.codex/*|*/.agents/skills/*)
      if [ -z "$ARMED_BY" ]; then
        mpb_deny "PreToolUse" "Blocked: $1 is part of the enforcement layer that is meant to constrain this session, so changing it from inside the session defeats it. If the change is intended, the human authorises it - either by putting MPB_ALLOW_GUARD_EDIT=1 in a message, which arms it for that message alone, or by starting the session with it set in the environment - and the end-of-pass report says what was edited and why, the same disclosed-escape pattern as MPB_SKIP_STOP_GATE. See docs/spec/11-agentic-sdlc.md."
      fi
      # Reaching here means the escape was armed - `mpb_deny` exits - so this
      # write is one the end-of-pass report owes an account of. Recording it is
      # not optional bookkeeping: disclosure is the whole of what this escape
      # rests on now that a session can arm it from a prompt.
      disclose "$1"
      ;;
  esac

  # docs/spec/11-agentic-sdlc.md: no UNATTENDED run resolves an item in
  # decisions-pending.md. Interactive sessions still add and resolve items
  # through the /decision-pending skill, which is the documented flow; the CI
  # workflows set MPB_UNATTENDED=1.
  if [ "${MPB_UNATTENDED:-0}" = "1" ]; then
    case "$1" in
      */docs/decisions-pending.md|*/docs/adr/*)
        mpb_deny "PreToolUse" "Blocked: this is an unattended run (MPB_UNATTENDED=1), and docs/spec/11-agentic-sdlc.md reserves decisions-pending items and ADRs for a human in the loop. Report what you would have written instead." ;;
    esac
  fi
}

check "$FILE"
check "$REAL"

exit 0
