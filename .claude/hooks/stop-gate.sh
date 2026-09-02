#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Stop. The definition-of-done gate.
#
# Anthropic's own guidance for agentic coding is "give Claude a way to check its
# work". Prose in a prompt is not a way; a hook that refuses to let the turn end
# is. Exit 2 blocks the stop and hands the reason back to the agent, which then
# fixes the problem instead of reporting success.
#
# Rules for what belongs here:
#   - Only objective, machine-checkable failures block. Judgement calls warn.
#   - It must run in seconds on this repository's current size.
#   - Every check maps to a line in docs/spec/07-quality-and-testing.md or
#     docs/spec/10-first-pass-work-order.md. If it maps to nothing, it does not
#     belong here.
#
# Escape hatch: MPB_SKIP_STOP_GATE=1 disables it. Using it is a decision to
# report unverified work, and the report must say so.
#
# Loop guard: Claude Code tells a Stop hook when it is re-entering after a
# block (stop_hook_active) and stops honouring the hook after a number of
# consecutive blocks (eight by default; CLAUDE_CODE_STOP_HOOK_BLOCK_CAP). This
# gate blocks again deliberately - the agent is expected to fix the failure,
# and the cap is the loop guard - but it says so in the message, because a
# turn that ends under the cap has ended with the gate failing.

set -uo pipefail
# shellcheck source=lib.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

ROOT="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
cd "$ROOT" || exit 0

[ "${MPB_SKIP_STOP_GATE:-0}" = "1" ] && exit 0

# The gate does not depend on its input, so unreadable input is not a reason
# to skip it; it only loses the re-entry flag.
INPUT="$(cat 2>/dev/null || true)"
REENTRY="$(mpb_json_field "$INPUT" "stop_hook_active" 2>/dev/null || true)"

fail=()
warn=()

# 1. Every internal Markdown link resolves. Agents navigate by these.
while IFS= read -r file; do
  while IFS= read -r raw; do
    target="${raw%%#*}"
    case "$target" in http*|mailto:*|"") continue ;; esac
    [ -e "$(dirname "$file")/$target" ] || fail+=("Broken link: $file -> $target")
  done < <(grep -oE '\]\([^)]+\)' "$file" 2>/dev/null | sed -E 's/^\]\(//; s/\)$//' || true)
done < <(find . -name '*.md' -not -path './.git/*' -not -path './target/*' 2>/dev/null)

# 2. Anything the agent configured must parse.
while IFS= read -r f; do
  python3 -m json.tool "$f" >/dev/null 2>&1 || fail+=("Invalid JSON: $f")
done < <(find .claude .github -name '*.json' 2>/dev/null)

while IFS= read -r f; do
  bash -n "$f" 2>/dev/null || fail+=("Shell syntax error: $f")
done < <(find .claude -name '*.sh' 2>/dev/null)

# 3. The foundation pass's hard boundary, grep-checkable by design
#    (docs/spec/10-first-pass-work-order.md: "grep finds no install/uninstall/
#    sudo/shell-write code path in the binary").
#    The exclusions apply to the matched line's *content*, never to the path:
#    a hit in src/application/planner.rs is still a hit, and so is one under
#    src/tests/ - Rust compiles any module into the binary regardless of the
#    directory's name, and integration tests live in the top-level tests/,
#    which is outside src/. build.rs is scanned too: `cargo build` runs it.
#    The foundation pass refines this against the real crate layout (step 21).
scan_paths=()
[ -d src ] && scan_paths+=(src)
[ -f build.rs ] && scan_paths+=(build.rs)
if [ ${#scan_paths[@]} -gt 0 ]; then
  mutating='"[^"]*\b(install|uninstall|reinstall|upgrade|link|unlink|selfupdate|activate|deactivate)\b[^"]*"'
  if grep -RInE "$mutating" "${scan_paths[@]}" 2>/dev/null \
     | awk -F: '{ line=$0; sub(/^[^:]*:[^:]*:/, "", line); if (line !~ /^[[:space:]]*(\/\/|#)/) print }' \
     | head -1 | grep -q .; then
    fail+=("A provider-mutating subcommand literal appears in src/ or build.rs. The foundation pass ships no mutating code path - see docs/spec/10-first-pass-work-order.md. If this is a plan type, model it as data rather than a command string; if it is a test, it belongs in the top-level tests/ directory.")
  fi
  if grep -RInE '\b(sudo|doas|launchctl|symlink|symlinkat|setuid|osascript|AuthorizationExecuteWithPrivileges)\b' "${scan_paths[@]}" 2>/dev/null \
     | awk -F: '{ line=$0; sub(/^[^:]*:[^:]*:/, "", line); if (line !~ /^[[:space:]]*(\/\/|#|\/\/\/|\/\/!)/) print }' \
     | head -1 | grep -q .; then
    fail+=("An elevation, service-control or symlink token appears in non-comment code under src/ or build.rs. INV-06 / INV-19: nothing in this pass elevates, controls services or creates symlinks.")
  fi
fi

# 4. Once there is a crate, the gate runs the real validation rather than only
#    the documentation checks. Formatting and lint always; the test suite when
#    code or tests changed, or when MPB_GATE_FULL=1. Anything slower than this
#    belongs in CI, which stays the authority
#    (docs/spec/07-quality-and-testing.md).
#    `--locked`: a gate must never rewrite Cargo.lock as a side effect
#    (supply chain, 04-migration-and-safety.md). Output is captured in
#    variables, never in a predictable file under a shared /tmp.
if [ -f Cargo.toml ] && command -v cargo >/dev/null 2>&1; then
  cargo fmt --all --check >/dev/null 2>&1 \
    || fail+=("cargo fmt --all --check fails. Run 'cargo fmt --all'.")

  if ! clippy_out="$(cargo clippy --locked --all-targets --all-features -- -D warnings 2>&1)"; then
    fail+=("cargo clippy fails with warnings denied:"$'\n'"$(printf '%s\n' "$clippy_out" | tail -40)")
  fi

  run_tests="${MPB_GATE_FULL:-0}"
  if [ "$run_tests" != "1" ] && command -v git >/dev/null 2>&1 && git rev-parse --git-dir >/dev/null 2>&1; then
    [ -n "$(git status --porcelain -- src tests 2>/dev/null | head -1)" ] && run_tests=1
  fi
  if [ "$run_tests" = "1" ]; then
    if ! test_out="$(cargo test --locked --all-features 2>&1)"; then
      fail+=("cargo test fails:"$'\n'"$(printf '%s\n' "$test_out" | tail -40)")
    fi
  fi
fi

# 5. Specification changes must be recorded, not just made. This warns rather
#    than blocks: only the agent can tell a typo from a material deviation.
if command -v git >/dev/null 2>&1 && git rev-parse --git-dir >/dev/null 2>&1; then
  changed_spec="$(git status --porcelain -- docs/spec prompts 2>/dev/null | head -1)"
  changed_evo="$(git status --porcelain -- docs/spec-evolution.md 2>/dev/null | head -1)"
  if [ -n "$changed_spec" ] && [ -z "$changed_evo" ]; then
    warn+=("docs/spec/ or prompts/ changed but docs/spec-evolution.md did not. If the change was material, record it now (see the entry format at the top of that file). If it was a typo, say so in your report.")
  fi
fi

if [ ${#fail[@]} -gt 0 ]; then
  printf 'MacPortsBrew stop gate - the turn cannot end with these unresolved:\n' >&2
  printf '  %s\n' "${fail[@]}" >&2
  if [ ${#warn[@]} -gt 0 ]; then
    printf 'Warnings:\n' >&2
    printf '  %s\n' "${warn[@]}" >&2
  fi
  printf 'Fix them, or explain in your report why each is acceptable and rerun with MPB_SKIP_STOP_GATE=1.\n' >&2
  if [ "$REENTRY" = "true" ]; then
    printf 'This is a repeated block. Claude Code ends the turn anyway after %s consecutive blocks; a turn that ends that way has ended with the gate failing, and the report must say so.\n' "${CLAUDE_CODE_STOP_HOOK_BLOCK_CAP:-8}" >&2
  fi
  exit 2
fi

# Warnings must not prolong the turn (additionalContext on Stop would), so
# they go to the human as a systemMessage and to the log on stderr.
if [ ${#warn[@]} -gt 0 ]; then
  printf 'MacPortsBrew stop gate - warnings:\n' >&2
  printf '  %s\n' "${warn[@]}" >&2
  mpb_message "MacPortsBrew stop gate warnings:"$'\n'"$(printf '  %s\n' "${warn[@]}")"
fi

exit 0
