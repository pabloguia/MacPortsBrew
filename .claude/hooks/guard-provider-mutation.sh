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
if printf '%s' "$NORM" | grep -Eq "${SEP}brew[[:space:]]+${OPTS}(install|instal|reinstall|uninstall|uninstal|remove|rm|upgrade|up|update|update-reset|link|ln|unlink|pin|unpin|tap|tap-new|untap|services|cleanup|autoremove|bundle|postinstall|migrate|fetch|edit|sh|ruby|irb|test|vendor-install|developer|analytics)([[:space:]]|\$)"; then
  deny "Blocked by MacPortsBrew INV-05 and by docs/spec/07-quality-and-testing.md. This command mutates the developer's real Homebrew installation. Read-only brew subcommands (list, leaves, deps, uses, info, config, outdated, --prefix) are allowed and are how provider behaviour is meant to be observed. To exercise a mutating path, capture a snapshot fixture instead - see docs/spec/07-quality-and-testing.md."
fi

# --- MacPorts mutation ----------------------------------------------------
if printf '%s' "$NORM" | grep -Eq "${SEP}port[[:space:]]+${OPTS}(install|uninstall|upgrade|activate|deactivate|select|selfupdate|sync|clean|reclaim|load|unload|setrequested|unsetrequested|destroot|build|fetch|configure|extract|patch|checksum|test|archive|unarchive|rev-upgrade|dmg|pkg|mpkg|mirror|bump|migrate|edit)([[:space:]]|\$)"; then
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
if printf '%s' "$NORM" | grep -Eq "${SEP}rustup[[:space:]]+${OPTS}(default|override|toolchain|install|update|self)([[:space:]]|\$)"; then
  deny "Blocked by ADR-0002. The toolchain is pinned in rust-toolchain.toml because the macOS 10.12 / 11.0 floor depends on it, and a floor increase is a breaking change that requires an ADR - not a routine upgrade. Change the pin in the file, with the ADR, rather than switching the active toolchain."
fi

if printf '%s' "$NORM" | grep -Eq "${SEP}cargo[[:space:]]+(\+[^[:space:]]+[[:space:]]+)?${OPTS}(install|publish|yank)([[:space:]]|\$)"; then
  deny "Blocked: this mutates the developer's environment or the crates.io registry rather than this repository. 'cargo install' writes binaries into ~/.cargo/bin, which is the same class of action as 'brew install' and is the human's to take. Development tooling belongs in documented prerequisites."
fi

# --- writing verbs --------------------------------------------------------
# Used by the dotfile and the provider-prefix rules below. In-place editors are
# matched on the flag's real spellings, not on one of them: BSD sed takes
# `sed -i ''` and `sed -i.bak` (the form an agent reaches for when it wants a
# backup - the safety-minded instinct), and `perl -pi -e` is the most common
# in-place idiom there is. Matching only `sed -i ` tested the least likely form.
WRITER='(>|(^|[;&|]|[[:space:]])((tee|cp|mv|ln|install|ditto|rsync|truncate|dd|mkdir|chmod|chown|touch|unlink|shred)([[:space:]]|$)|sed([[:space:]]+-[^i[:space:]][^[:space:]]*)*[[:space:]]+-i|perl[[:space:]]+-[a-zA-Z]*i[a-zA-Z]*|(python3?|ruby)[[:space:]]+-[ce]|awk[[:space:]]+-i))'

# --- shell configuration and launch machinery -----------------------------
# A dotfile path together with any writing verb - not only a redirect.
#
# The directive must sit immediately above the assignment. It used to sit
# twenty lines up, above a comment block, where it silenced nothing and CI's
# very first gate step exited 1 on two SC2016 findings nobody had seen,
# because CI has never run and validate.sh has no shellcheck stage (F-44).
# shellcheck disable=SC2016  # $HOME / $ZDOTDIR are literals to match, not to expand
DOTFILE='(~|\$HOME|\$ZDOTDIR|/Users/[^/ ]+|(^|[[:space:]]))/?\.(zshrc|zprofile|zshenv|zlogin|bashrc|bash_profile|bash_login|profile|config/fish/[^ ]*)([[:space:]]|$)'
if printf '%s' "$NORM" | grep -Eq "$DOTFILE" && printf '%s' "$NORM" | grep -Eq "$WRITER"; then
  deny "Blocked by MacPortsBrew docs/spec/05-environment-and-primary-provider.md. Shell configuration is only ever written through the managed-block procedure (detect, locate, diff, preview, confirm, back up, write atomically, verify) and never by an agent writing to a dotfile. Reading one is fine; nothing in the foundation pass writes shell files at all."
fi

# --- writes into a provider prefix ----------------------------------------
# guard-protected-paths.sh covers this for Edit and Write. A deny rule on a
# path does not reach a subprocess, so Bash was the tool that could reach the
# artifact without spelling it: `cp target/release/macportsbrew
# /opt/homebrew/bin/` is the obvious way to "try the binary against the real
# system" after a build, and nothing stopped it.
#
# The prefix must be in a DESTINATION position - the tail of the command, or a
# redirect target. Reading out of a prefix is legitimate and common (capturing
# a fixture), so `cp /opt/homebrew/etc/foo ./fixtures/` must stay allowed.
# shellcheck disable=SC2016  # $HOME is a literal to match, not to expand
PREFIX='(/opt/homebrew|/opt/local|/usr/local|/etc/paths\.d|(~|\$HOME|/Users/[^/ ]+)/\.(home|linux)brew)'
if printf '%s' "$NORM" | grep -Eq ">>?[[:space:]]*${PREFIX}" \
  || printf '%s' "$NORM" | grep -Eq "${SEP}(tee|cp|mv|ln|install|ditto|rsync|mkdir|chmod|chown|touch)[[:space:]]([^;&|]*[[:space:]])?${PREFIX}[^[:space:];&|]*[[:space:]]*([;&|]|\$)"; then
  deny "Blocked: this writes into a package manager's prefix or into /etc/paths.d. MacPortsBrew never writes into a provider prefix - INV-07, INV-08, and the 'never rewrite, re-link or patch a binary' rule in docs/spec/04-migration-and-safety.md. Build artifacts stay in target/; to exercise the binary, run it from there. Reading out of a prefix is allowed and is how fixtures are captured."
fi

# --- the toolchain pin itself (ADR-0002) ----------------------------------
# The rustup rule above guards the command that switches toolchains, and its
# own deny message says to change the pin in the file instead. That made the
# file the unguarded path to the same outcome: editing `channel` makes the next
# cargo invocation download and use a different compiler, and the macOS floor
# moves with no denied command anywhere.
if printf '%s' "$NORM" | grep -Eq 'rust-toolchain\.toml' && printf '%s' "$NORM" | grep -Eq "$WRITER"; then
  deny "Blocked by ADR-0002. rust-toolchain.toml holds the pin that keeps the macOS 10.12 / 11.0 deployment floor, which is a product commitment, not a build detail. Raising it is a breaking change that needs an ADR and an owner decision - so the change goes through docs/adr/ and a human, not through this command. Reading the file is fine."
fi

# --- publishing ------------------------------------------------------------
# DP-007 is decided (public GitHub) and the repository exists, so an ordinary
# push of a topic branch to `origin` is an agent's step, authorised by the owner
# on 2026-10-16. What stays the owner's: anything that rewrites or removes what
# is already published, anything aimed at the default branch, and any push that
# names a destination other than the `origin` remote of this project's own
# repository. The settings.json deny rules for pushing were removed with this
# change; this hook is now the only layer, so it fails closed.
if printf '%s' "$NORM" | grep -Eq "${SEP}git([[:space:]]+-[^[:space:]]+([[:space:]]+[^-][^[:space:]]*)?)*[[:space:]]+push([[:space:]]|\$)"; then
  if printf '%s' "$NORM" | grep -Eq '[[:space:]](-f|--force[a-z-]*|--delete|-d|--mirror|--all|--prune|--no-verify|--receive-pack[a-z-]*|--exec)([[:space:]]|$)' \
    || printf '%s' "$NORM" | grep -Eq 'push([[:space:]]+[^[:space:]]+)*[[:space:]]+(\+|:)[^[:space:]]'; then
    deny "Blocked: a force, a delete, a mirror or all-branches push, or a refspec starting with + or :, rewrites or removes what is already published. That stays the owner's act. An ordinary push of a topic branch to origin is allowed."
  fi
  if printf '%s' "$NORM" | grep -Eq '(^|[[:space:]:/])(main|master)([[:space:]]|$)|HEAD:|refs/heads/(main|master)'; then
    deny "Blocked: a push aimed at main or master. The default branch changes through a pull request, not a push from this session."
  fi
  if printf '%s' "$NORM" | grep -Eqi 'github\.com|git@|https?://|ssh://|file://|[[:space:]]/[^[:space:]]'; then
    deny "Blocked: a push that names a URL or a path. Only the origin remote is allowed, and only when it is this project's repository."
  fi
  PUSH_URL="$(git -C "${CLAUDE_PROJECT_DIR:-.}" remote get-url origin 2>/dev/null || true)"
  if ! printf '%s' "$PUSH_URL" | grep -Eq '^(git@github\.com:|https://github\.com/)pabloguia/MacPortsBrew(\.git)?$'; then
    deny "Blocked: the origin remote is not pabloguia/MacPortsBrew on GitHub (it is '${PUSH_URL:-unset}'), so this session does not push. Fix the remote, or push by hand."
  fi
  PUSH_BRANCH="$(git -C "${CLAUDE_PROJECT_DIR:-.}" rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
  case "$PUSH_BRANCH" in
    main|master|HEAD|"")
      deny "Blocked: the current branch is '${PUSH_BRANCH:-unknown}'. A push from main, master or a detached HEAD is not made from this session; switch to a topic branch." ;;
  esac
fi

if printf '%s' "$NORM" | grep -Eq "${SEP}(launchctl|chsh|systemsetup|csrutil|spctl)([[:space:]]|\$)"; then
  deny "Blocked by MacPortsBrew INV-19 and the threat model in docs/spec/04-migration-and-safety.md. Background services, the login shell and system security settings are never touched as a side effect of any operation, and never by an agent working on this repository."
fi

exit 0
