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

REAL="$(mpb_realpath "$FILE")"

check() {
  case "$1" in
    /opt/local/*|/usr/local/*|/opt/homebrew/*)
      mpb_deny "PreToolUse" "Blocked: $1 is inside a package manager's prefix. MacPortsBrew never writes into a provider prefix - INV-07, INV-08 and the 'never rewrite, re-link or patch a binary' rule in docs/spec/04-migration-and-safety.md. Report the finding instead." ;;
    */.zshrc|*/.zprofile|*/.zshenv|*/.zlogin|*/.bashrc|*/.bash_profile|*/.bash_login|*/.profile|*/.config/fish/*|.zshrc|.zprofile|.zshenv|.zlogin|.bashrc|.bash_profile|.bash_login|.profile)
      mpb_deny "PreToolUse" "Blocked: $1 is a shell configuration file. See docs/spec/05-environment-and-primary-provider.md - shell files are only ever written through the managed-block procedure, and not at all in the foundation pass." ;;
    */Library/LaunchAgents/*|*/Library/LaunchDaemons/*|/Library/*|/System/*|/private/etc/*|/etc/*)
      mpb_deny "PreToolUse" "Blocked: $1 is system or launch machinery. INV-19: services are never started, stopped or relocated as a side effect." ;;
  esac
}

check "$FILE"
check "$REAL"

exit 0
