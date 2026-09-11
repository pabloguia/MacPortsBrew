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

  # The control protecting itself. An agent whose gate keeps failing does not
  # need bad intent to disable it, and the gate can only detect that its own
  # path changed - never prevent it. MPB_ALLOW_GUARD_EDIT=1 is the deliberate
  # escape, and using it obliges the end-of-pass report to say so, exactly like
  # MPB_SKIP_STOP_GATE.
  if [ "${MPB_ALLOW_GUARD_EDIT:-0}" != "1" ]; then
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
        mpb_deny "PreToolUse" "Blocked: $1 is part of the enforcement layer that is meant to constrain this session, so changing it from inside the session defeats it. If the change is intended, the human starts the session with MPB_ALLOW_GUARD_EDIT=1 and the end-of-pass report says so - the same disclosed-escape pattern as MPB_SKIP_STOP_GATE. See docs/spec/11-agentic-sdlc.md." ;;
    esac
  fi

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
