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
#    What is scanned is a path question and what is excluded is a content
#    question, and the two are separate.
#
#    Scanned: crates/*/src (and a flat src/, and every build.rs - `cargo build`
#    runs those). A hit anywhere under one of those is a hit, including under a
#    directory called tests/ inside src/: Rust compiles any module into the
#    binary regardless of the directory's name, so `crates/cli/src/tests/` is
#    shipped code and is scanned.
#
#    Not scanned: crates/*/tests, the integration-test directories. Cargo
#    builds those into separate test binaries that `cargo build` does not
#    produce and no release carries, so a mutating subcommand named there as
#    test data is not a mutating code path in the binary. That is the exemption
#    the message below points at, and it is why a fixture naming one belongs in
#    crates/*/tests or in a data file rather than in src/.
#
#    The exclusions - a match on a comment line - apply to the matched line's
#    *content*, never to its path: a hit in
#    crates/application/src/planner.rs is still a hit.
#    The foundation pass refines this against the real crate layout (step 21).
#    The workspace keeps one crate per layer under crates/<layer>/src; a
#    flat src/ is scanned too so the check does not depend on the layout.
scan_paths=()
[ -d src ] && scan_paths+=(src)
[ -f build.rs ] && scan_paths+=(build.rs)
for d in crates/*/src; do [ -d "$d" ] && scan_paths+=("$d"); done
for b in crates/*/build.rs; do [ -f "$b" ] && scan_paths+=("$b"); done
if [ ${#scan_paths[@]} -gt 0 ]; then
  mutating='"[^"]*\b(install|uninstall|reinstall|upgrade|link|unlink|selfupdate|activate|deactivate)\b[^"]*"'
  if grep -RInE "$mutating" "${scan_paths[@]}" 2>/dev/null \
     | awk -F: '{ line=$0; sub(/^[^:]*:[^:]*:/, "", line); if (line !~ /^[[:space:]]*(\/\/|#)/) print }' \
     | head -1 | grep -q .; then
    fail+=("A provider-mutating subcommand literal appears in src/ or build.rs. The foundation pass ships no mutating code path - see docs/spec/10-first-pass-work-order.md. If this is a plan type, model it as data rather than a command string; if it is a test, it belongs under crates/<crate>/tests/, which is not shipped and not scanned. And if you are relaxing this guard because the executor pass has begun: 04-migration-and-safety.md:201 is a MUST requiring a single-instance lock and a check for a concurrently running brew or port before anything mutates. Nothing implements it. This guard going away is the moment that MUST gets lost - see the obligations list in docs/schemas/plan.v1.md.")
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
#    The checks themselves live in scripts/validate.sh - one implementation,
#    three entry points (07-quality-and-testing.md). The gate only decides
#    how much of it to run: --quick (format, lint, layering, SPDX, licences,
#    i18n parity) on every turn end; the test suite too when code, tests,
#    fixtures or catalogues changed, or when MPB_GATE_FULL=1.
# Whether this turn touched anything the code checks cover.
#
# The working tree alone is not the answer: this repository commits mid-turn, so
# an edit under `crates/` followed by a commit leaves a clean tree, and a check
# keyed on `git status` reads that as "nothing changed". A committed change is
# still a change this turn made. So: uncommitted work *or* anything committed
# since the previous position of HEAD.
#
# `scripts` and `.claude` are in the pathspec because they are the gates
# themselves, which `AGENTS.md` lists first under "dangerous to change". An edit
# to `validate.sh` that runs no checks is the shape of defect this whole gate
# exists to refuse.
touched_paths() {
  local spec=(crates src tests fixtures locales data scripts .claude Cargo.lock Cargo.toml)
  command -v git >/dev/null 2>&1 || { echo unknown; return; }
  git rev-parse --git-dir >/dev/null 2>&1 || { echo unknown; return; }
  if [ -n "$(git status --porcelain -- "${spec[@]}" 2>/dev/null | head -1)" ]; then
    echo yes; return
  fi
  # `HEAD@{1}` is where HEAD was before the most recent move. Absent on a fresh
  # clone or a repository with no reflog, in which case say so rather than
  # guessing "no".
  if git rev-parse --verify --quiet 'HEAD@{1}' >/dev/null 2>&1; then
    if [ -n "$(git diff --name-only 'HEAD@{1}' HEAD -- "${spec[@]}" 2>/dev/null | head -1)" ]; then
      echo yes; return
    fi
    echo no; return
  fi
  echo unknown
}

if [ -f Cargo.toml ] && [ -x scripts/validate.sh ]; then
  PATH="$HOME/.cargo/bin:$PATH"
  touched="$(touched_paths)"
  if command -v cargo >/dev/null 2>&1; then
    run_tests="${MPB_GATE_FULL:-0}"
    [ "$run_tests" != "1" ] && [ "$touched" != "no" ] && run_tests=1
    mode=--quick
    [ "$run_tests" = "1" ] && mode=default
    if ! validate_out="$(scripts/validate.sh "$mode" 2>&1)"; then
      fail+=("scripts/validate.sh $mode fails:"$'\n'"$(printf '%s\n' "$validate_out" | grep -vE '^\s*(Compiling|Checking|Finished|Running|Blocking)' | tail -60)")
    fi
  else
    # A warning here, and the previous pass concluded from it that this machine
    # had no toolchain - which was false, and the report said so as fact. The
    # gate must not be able to pass while the checks it exists to run did not
    # run at all (F-20).
    #
    # Scoped to a change that needs them, so a documentation turn on a machine
    # without a toolchain is not held hostage to one. A change under `crates/`,
    # `tests/`, `fixtures/`, `locales/`, `data/` or `Cargo.lock` needs them, and
    # that is the same set the mode decision uses above.
    # `MPB_GATE_FULL=1` is an explicit request for the tests. Honouring it only
    # when they can run, and warning otherwise, would answer "run everything"
    # with silence.
    if [ "$touched" != "no" ] || [ "${MPB_GATE_FULL:-0}" = "1" ]; then
      fail+=("cargo is not on PATH, and this turn changed code, tests, fixtures, \
locales or data. The checks that would have run did not run - which is not the \
same as passing. Install the pinned toolchain, or say plainly in the turn's \
report that nothing was verified.")
    else
      warn+=("cargo is not on PATH; the code checks did not run. No code changed \
this turn, so nothing is blocked - but do not report anything as verified.")
    fi
  fi
else
  # The other half of the same defect: if this is not a Rust workspace, or
  # `validate.sh` is not executable, the block above is skipped entirely and the
  # gate exits 0 having checked nothing at all. Being in the wrong directory
  # looks exactly like a clean turn.
  if [ ! -f Cargo.toml ]; then
    warn+=("no Cargo.toml here; the stop gate ran no code checks. If you expected \
it to, the working directory is wrong.")
  elif [ ! -x scripts/validate.sh ]; then
    fail+=("scripts/validate.sh is not executable, so the one implementation of \
\"valid\" did not run. Restore its mode rather than working around it.")
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
