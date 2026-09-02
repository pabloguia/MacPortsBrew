#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# PreToolUse(Bash) guard.
#
# What it defends
#   INV-05  a package manager is never installed, upgraded or reconfigured
#           implicitly
#   INV-06  least privilege; nothing in this repository's agent workflow needs
#           root
#   07-quality-and-testing.md  "Tests never touch the developer's real Homebrew
#           or MacPorts installation. Any test that could is a build failure."
#   10-first-pass-work-order.md  the foundation pass ships no mutating code path
#
# Why it exists even though .claude/settings.json already has deny rules:
# Bash deny rules match command text, and the permissions documentation says
# plainly that argument-constraining Bash patterns are fragile. This hook adds
# a second, auditable layer that also catches absolute paths
# (/opt/homebrew/bin/brew install), env-prefixed forms, options before the
# subcommand, quoting, command substitution and common wrappers.
#
# What it is NOT: a sandbox. A determined agent can defeat any text matcher -
# variables, aliases, functions, encodings. It is a tripwire for the accident,
# not a cage for the adversary. The real boundary is the permission system plus
# a human reading the plan.

set -euo pipefail
# shellcheck source=lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

mpb_require_json_parser
INPUT="$(cat)"
COMMAND=""
mpb_field_or_fail_closed COMMAND "$INPUT" "tool_input.command"
[ -n "$COMMAND" ] || exit 0

# Normalise, over-matching on purpose (a false positive costs a reworded
# command; a false negative costs the developer's package manager):
#   - whitespace variants collapse to one space;
#   - quotes and backslashes are stripped, so 'brew' "install" and br\ew match;
#   - grouping and substitution characters become spaces, so $(brew install)
#     and (sudo ls) match;
#   - a leading path on an executable is stripped, so /opt/local/bin/port and
#     port match the same rule.
NORM="$(printf '%s' "$COMMAND" | tr '\n\t\r' '   ' | tr -s ' ')"
NORM="$(printf '%s' "$NORM" | sed -E "s/['\"\\\\]//g; s/[()\`{}!=]/ /g" | tr -s ' ')"
NORM="$(printf '%s' "$NORM" | sed -E 's#(^|[^[:alnum:]_./-])/[^ ]*/(brew|port|sudo|doas|su|launchctl|chsh|rustup|cargo|installer|softwareupdate|xcode-select|osascript)([[:space:]])#\1\2\3#g')"

deny() { mpb_deny "PreToolUse" "$1"; }
SEP='(^|[;&|]|[[:space:]])'
OPTS='(-[^[:space:]]+[[:space:]]+)*'

# --- elevation ------------------------------------------------------------
if printf '%s' "$NORM" | grep -Eq "${SEP}(sudo|doas|su)([[:space:]]|\$)" \
  || printf '%s' "$NORM" | grep -Eqi 'with administrator privileges'; then
  deny "Blocked by MacPortsBrew INV-06 (least privilege). No agent task in this repository requires elevation: discovery, inventory, mapping, analysis, planning and dry-run all work unprivileged, and the foundation pass ships no mutating code path at all. If you believe a task genuinely needs sudo, stop and record the question in docs/decisions-pending.md instead of running it."
fi

# --- Homebrew mutation ----------------------------------------------------
if printf '%s' "$NORM" | grep -Eq "${SEP}brew[[:space:]]+${OPTS}(install|reinstall|uninstall|remove|rm|upgrade|update|update-reset|link|unlink|pin|unpin|tap|untap|services|cleanup|autoremove|bundle|postinstall|migrate|fetch|edit|sh|ruby|irb|test|vendor-install|developer|analytics)([[:space:]]|\$)"; then
  deny "Blocked by MacPortsBrew INV-05 and by docs/spec/07-quality-and-testing.md. This command mutates the developer's real Homebrew installation. Read-only brew subcommands (list, leaves, deps, uses, info, config, outdated, --prefix) are allowed and are how provider behaviour is meant to be observed. To exercise a mutating path, capture a snapshot fixture instead - see docs/spec/07-quality-and-testing.md."
fi

# --- MacPorts mutation ----------------------------------------------------
if printf '%s' "$NORM" | grep -Eq "${SEP}port[[:space:]]+${OPTS}(install|uninstall|upgrade|activate|deactivate|select|selfupdate|sync|clean|reclaim|load|unload|setrequested|unsetrequested|destroot|build|fetch|configure|extract|patch|checksum|test|archive|rev-upgrade|dmg|pkg|mpkg|mirror|bump|migrate|edit)([[:space:]]|\$)"; then
  deny "Blocked by MacPortsBrew INV-05 and by docs/spec/07-quality-and-testing.md. This command mutates the developer's real MacPorts installation. Read-only port subcommands (installed, echo, deps, rdeps, dependents, info, variants, outdated, version) are allowed. Use captured snapshot fixtures for anything else."
fi

# --- system installers (INV-05 applies to the toolchain too) --------------
if printf '%s' "$NORM" | grep -Eq "${SEP}(xcode-select[[:space:]]+--install|softwareupdate[[:space:]]+${OPTS}(-i|--install|-ia|-iR)|installer[[:space:]]+-)"; then
  deny "Blocked by MacPortsBrew INV-05. Installing a toolchain, a system update or a package implicitly is the human's action, taken outside a session. Record what is needed in .claude/README.md (Prerequisites) instead."
fi

# --- toolchain integrity (ADR-0002) ---------------------------------------
# The macOS floor is a product commitment, and the toolchain is what keeps it.
# Rust has raised its own macOS floor before (see ADR-0002); an unpinned or
# silently switched toolchain drops the target audience in a routine upgrade.
if printf '%s' "$NORM" | grep -Eq "${SEP}rustup[[:space:]]+${OPTS}(default|override|toolchain|install|self)([[:space:]]|\$)"; then
  deny "Blocked by ADR-0002. The toolchain is pinned in rust-toolchain.toml because the macOS 10.12 / 11.0 floor depends on it, and a floor increase is a breaking change that requires an ADR - not a routine upgrade. Change the pin in the file, with the ADR, rather than switching the active toolchain."
fi

if printf '%s' "$NORM" | grep -Eq "${SEP}cargo[[:space:]]+(\+[^[:space:]]+[[:space:]]+)?${OPTS}(install|publish|yank)([[:space:]]|\$)"; then
  deny "Blocked: this mutates the developer's environment or the crates.io registry rather than this repository. 'cargo install' writes binaries into ~/.cargo/bin, which is the same class of action as 'brew install' and is the human's to take. Development tooling belongs in documented prerequisites."
fi

# --- shell configuration and launch machinery -----------------------------
# A dotfile path together with any writing verb - not only a redirect.
# shellcheck disable=SC2016  # the $HOME / $ZDOTDIR are literals to match, not to expand
DOTFILE='(~|\$HOME|\$ZDOTDIR|/Users/[^/ ]+|(^|[[:space:]]))/?\.(zshrc|zprofile|zshenv|zlogin|bashrc|bash_profile|bash_login|profile|config/fish/[^ ]*)([[:space:]]|$)'
WRITER='(>|(^|[;&|]|[[:space:]])(tee|sed -i|sed -E -i|cp|mv|ln|install|truncate|dd|perl -i)([[:space:]]|$))'
if printf '%s' "$NORM" | grep -Eq "$DOTFILE" && printf '%s' "$NORM" | grep -Eq "$WRITER"; then
  deny "Blocked by MacPortsBrew docs/spec/05-environment-and-primary-provider.md. Shell configuration is only ever written through the managed-block procedure (detect, locate, diff, preview, confirm, back up, write atomically, verify) and never by an agent writing to a dotfile. Reading one is fine; nothing in the foundation pass writes shell files at all."
fi

if printf '%s' "$NORM" | grep -Eq "${SEP}(launchctl|chsh|systemsetup|csrutil|spctl)([[:space:]]|\$)"; then
  deny "Blocked by MacPortsBrew INV-19 and the threat model in docs/spec/04-migration-and-safety.md. Background services, the login shell and system security settings are never touched as a side effect of any operation, and never by an agent working on this repository."
fi

exit 0
