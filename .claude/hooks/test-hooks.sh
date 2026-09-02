#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Exercises every hook with fixture inputs and asserts the decision.
#
# Why this exists: docs/spec/11-agentic-sdlc.md says a boundary is enforced
# by a mechanism, not a sentence - and a mechanism that is never exercised is a
# sentence with a file extension. Run it after any change under .claude/hooks/.
# CI runs it too (.github/workflows/repository-validation.yml).
#
# The fixtures below name mutating commands as *data*. Nothing here runs them:
# each case pipes a JSON document into a hook and reads the hook's decision.
# Tokens the guards look for are assembled at runtime from fragments, so a
# grep of this file for a subcommand finds nothing and the Bash guard does not
# fire on the command that runs this script.

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export CLAUDE_PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$HERE/../.." && pwd)}"

pass=0
failures=()

json_ok() {
  if command -v python3 >/dev/null 2>&1; then
    printf '%s' "$1" | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null
  elif command -v jq >/dev/null 2>&1; then
    printf '%s' "$1" | jq -e . >/dev/null 2>&1
  else
    return 0
  fi
}

# assert <label> <hook> <expected: allow|deny|closed|context|exit0|exit2> <json>
assert() {
  local label="$1" hook="$2" expect="$3" json="$4" out rc verdict
  out="$(printf '%s' "$json" | "$HERE/$hook" 2>&1)"
  rc=$?
  case "$expect" in
    allow) [ "$rc" -eq 0 ] && ! printf '%s' "$out" | grep -Eq '"permissionDecision": *"deny"' && verdict=ok ;;
    deny) [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -Eq '"permissionDecision": *"deny"' && json_ok "$out" && verdict=ok ;;
    closed) [ "$rc" -eq 2 ] && verdict=ok ;;
    context) [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q '"additionalContext"' && json_ok "$out" && verdict=ok ;;
    exit0) [ "$rc" -eq 0 ] && verdict=ok ;;
    exit2) [ "$rc" -eq 2 ] && verdict=ok ;;
  esac
  if [ "${verdict:-}" = ok ]; then
    pass=$((pass + 1))
  else
    failures+=("$label: expected $expect, got rc=$rc: $(printf '%s' "$out" | tr '\n' ' ' | cut -c1-160)")
  fi
}

# JSON-encode a command string so quotes and backslashes in fixtures survive.
bash_input() {
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import json,sys; print(json.dumps({"tool_name":"Bash","tool_input":{"command":sys.argv[1]}}))' "$1"
  else
    jq -cn --arg c "$1" '{tool_name:"Bash",tool_input:{command:$c}}'
  fi
}
edit_input() { printf '{"tool_name":"Edit","tool_input":{"file_path":"%s"}}' "$1"; }

G=guard-provider-mutation.sh
P=guard-protected-paths.sh

# Fragments, joined at runtime (see the header).
INST="in""stall"
SUDO="su""do"
UPG="up""grade"
ZRC=".zs""hrc"

# --- provider mutation guard: the accident-shaped forms ---------------------
assert "brew list is allowed"                 "$G" allow  "$(bash_input 'brew list')"
assert "brew --prefix is allowed"             "$G" allow  "$(bash_input 'brew --prefix')"
assert "brew info is allowed"                 "$G" allow  "$(bash_input 'brew info jq')"
assert "port installed is allowed"            "$G" allow  "$(bash_input 'port installed')"
assert "cargo build is allowed"               "$G" allow  "$(bash_input 'cargo build')"
assert "cargo +toolchain build is allowed"    "$G" allow  "$(bash_input 'cargo +stable build')"
assert "reading a dotfile is allowed"         "$G" allow  "$(bash_input "cat ~/$ZRC")"
assert "grep in a dotfile is allowed"         "$G" allow  "$(bash_input "grep PATH $ZRC")"
# shellcheck disable=SC2016  # the fixture is literal command text
assert "command substitution alone is fine"   "$G" allow  "$(bash_input 'echo $(date)')"
assert "empty input is a no-op"               "$G" allow  '{"tool_input":{}}'

assert "brew $INST is denied"                 "$G" deny   "$(bash_input "brew $INST jq")"
assert "brew with option before verb"         "$G" deny   "$(bash_input "brew -v $INST jq")"
assert "absolute-path brew is denied"         "$G" deny   "$(bash_input "/opt/homebrew/bin/brew $INST jq")"
assert "env-prefixed port with flag denied"   "$G" deny   "$(bash_input "env FOO=1 port -N $INST jq")"
assert "port $UPG outdated is denied"         "$G" deny   "$(bash_input "port $UPG outdated")"
assert "pipe into brew is denied"             "$G" deny   "$(bash_input "echo y | brew $INST jq")"
assert "command substitution is denied"       "$G" deny   "$(bash_input "echo \$(brew $INST jq)")"
assert "quoted subcommand is denied"          "$G" deny   "$(bash_input "brew '$INST' jq")"
assert "sh -c wrapper is denied"              "$G" deny   "$(bash_input "sh -c 'brew $INST jq'")"
assert "backslash-split verb is denied"       "$G" deny   "$(bash_input "br\\ew $INST jq")"
assert "$SUDO is denied"                      "$G" deny   "$(bash_input "$SUDO ls")"
assert "$SUDO in a subshell is denied"        "$G" deny   "$(bash_input "($SUDO ls)")"
assert "su is denied"                         "$G" deny   "$(bash_input 'su root -c ls')"
assert "doas via path is denied"              "$G" deny   "$(bash_input '/usr/bin/doas ls')"
assert "osascript admin privileges denied"    "$G" deny   "$(bash_input 'osascript -e "do shell script \"ls\" with administrator privileges"')"
assert "rustup default via path is denied"    "$G" deny   "$(bash_input '/usr/local/bin/rustup default stable')"
assert "rustup $INST is denied"               "$G" deny   "$(bash_input "rustup $INST stable")"
assert "cargo $INST is denied"                "$G" deny   "$(bash_input "cargo $INST ripgrep")"
assert "cargo +toolchain $INST is denied"     "$G" deny   "$(bash_input "cargo +nightly $INST ripgrep")"
assert "xcode-select --$INST is denied"       "$G" deny   "$(bash_input "xcode-select --$INST")"
assert "softwareupdate -i is denied"          "$G" deny   "$(bash_input 'softwareupdate -i -a')"
assert "installer -pkg is denied"             "$G" deny   "$(bash_input 'installer -pkg x.pkg -target /')"
assert "append to dotfile is denied"          "$G" deny   "$(bash_input "echo x >> ~/$ZRC")"
assert "append with braces is denied"         "$G" deny   "$(bash_input "echo x >> \${HOME}/$ZRC")"
assert "tee into dotfile is denied"           "$G" deny   "$(bash_input "echo x | tee -a ~/$ZRC")"
assert "sed -i on dotfile is denied"          "$G" deny   "$(bash_input "sed -i '' 's/a/b/' ~/$ZRC")"
assert "cp over dotfile is denied"            "$G" deny   "$(bash_input "cp x ~/.zprofile")"
assert "relative dotfile write is denied"     "$G" deny   "$(bash_input "cd ~ && echo x >> $ZRC")"
assert "launchctl is denied"                  "$G" deny   "$(bash_input 'launchctl list')"
assert "unreadable input fails closed"        "$G" closed 'not json at all'

# --- protected paths guard --------------------------------------------------
assert "repository file is allowed"           "$P" allow  "$(edit_input "$CLAUDE_PROJECT_DIR/docs/x.md")"
assert "/usr/local is denied"                 "$P" deny   "$(edit_input '/usr/local/bin/x')"
assert "/opt/local is denied"                 "$P" deny   "$(edit_input '/opt/local/etc/x')"
assert "/opt/homebrew is denied"              "$P" deny   "$(edit_input '/opt/homebrew/x')"
assert "double-slash prefix is denied"        "$P" deny   "$(edit_input '//opt/local/etc/x')"
assert "traversal into a prefix is denied"    "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/../../../../opt/local/etc/x")"
assert "shell rc is denied"                   "$P" deny   "$(edit_input "$HOME/$ZRC")"
assert "relative shell rc is denied"          "$P" deny   "$(edit_input "$ZRC")"
assert "fish config is denied"                "$P" deny   "$(edit_input "$HOME/.config/fish/config.fish")"
assert "LaunchAgents is denied"               "$P" deny   "$(edit_input "$HOME/Library/LaunchAgents/x.plist")"
assert "notebook in repository is allowed"    "$P" allow  '{"tool_input":{"notebook_path":"n.ipynb"}}'
assert "unreadable input fails closed"        "$P" closed '{bad'
lnk="$(mktemp -d)"
ln -s /opt/local/etc/x "$lnk/inner.toml"
assert "symlink into a prefix is denied"      "$P" deny   "$(edit_input "$lnk/inner.toml")"
ln -s "$lnk" "$lnk/self"
assert "symlink to a safe target is denied"   "$P" deny   "$(edit_input "$lnk/self")"
rm -rf "$lnk"

# --- the fail-closed helper does not depend on the caller's set -e ---------
probe="$(mktemp)"
cat > "$probe" <<EOF
#!/usr/bin/env bash
set +e
. "$HERE/lib.sh"
set +e
V=""
mpb_field_or_fail_closed V 'not json' "tool_input.command"
echo "reached-after-helper"
exit 0
EOF
out="$(bash "$probe" 2>&1)"
rc=$?
if [ "$rc" -eq 2 ] && ! printf '%s' "$out" | grep -q 'reached-after-helper'; then pass=$((pass + 1)); else failures+=("helper must exit the caller under set +e: rc=$rc: $(printf '%s' "$out" | tr '\n' ' ' | cut -c1-120)"); fi
cat > "$probe" <<EOF
#!/usr/bin/env bash
set +e
. "$HERE/lib.sh"
set +e
V=""
mpb_field_or_fail_closed V '{"tool_input":{"command":"ls"}}' "tool_input.command"
[ "\$V" = "ls" ] && echo "value-set"
EOF
out="$(bash "$probe" 2>&1)"
if printf '%s' "$out" | grep -q 'value-set'; then pass=$((pass + 1)); else failures+=("helper must set the variable in the caller's shell: $out"); fi
rm -f "$probe"

# --- session start ----------------------------------------------------------
assert "session start injects context"        session-start.sh context '{"source":"startup"}'

# --- post-edit checks -------------------------------------------------------
tmp="$(mktemp -d)"
printf '{"broken' > "$tmp/bad.json"
assert "invalid JSON is reported as context"  post-edit-checks.sh context "$(printf '{"tool_input":{"file_path":"%s"}}' "$tmp/bad.json")"
printf '# ok\n' > "$tmp/ok.md"
assert "clean Markdown produces no output"    post-edit-checks.sh exit0   "$(printf '{"tool_input":{"file_path":"%s"}}' "$tmp/ok.md")"
rm -rf "$tmp"

# --- stop gate --------------------------------------------------------------
# On this repository the gate must pass; a deliberately broken tree must block.
assert "stop gate passes on a clean tree"     stop-gate.sh exit0 '{"stop_hook_active":false}'
broken="$(mktemp -d)"
mkdir -p "$broken/.claude" "$broken/docs"
printf 'see [missing](nowhere.md)\n' > "$broken/docs/broken.md"
out="$(printf '{"stop_hook_active":true}' | CLAUDE_PROJECT_DIR="$broken" MPB_SKIP_STOP_GATE=0 "$HERE/stop-gate.sh" 2>&1)"
rc=$?
if [ "$rc" -eq 2 ] && printf '%s' "$out" | grep -q 'Broken link' && printf '%s' "$out" | grep -q 'repeated block'; then
  pass=$((pass + 1))
else
  failures+=("stop gate blocks a broken tree and names re-entry: rc=$rc: $(printf '%s' "$out" | tr '\n' ' ' | cut -c1-160)")
fi
out="$(printf '{}' | CLAUDE_PROJECT_DIR="$broken" MPB_SKIP_STOP_GATE=1 "$HERE/stop-gate.sh" 2>&1)"
rc=$?
if [ "$rc" -eq 0 ]; then pass=$((pass + 1)); else failures+=("escape hatch disables the gate: rc=$rc"); fi
rm -rf "$broken"

# The no-mutation boundary: a mutating literal in src/ blocks wherever it is,
# including src/tests/ (Rust compiles any module in), embedded in a longer
# string, and in build.rs; a comment does not; the top-level tests/ is not
# scanned.
gate_case() {  # <label> <expected rc> <grep-needle-or-empty>
  local label="$1" want="$2" needle="$3" out rc
  out="$(printf '{}' | CLAUDE_PROJECT_DIR="$boundary" "$HERE/stop-gate.sh" 2>&1)"
  rc=$?
  if [ "$rc" -eq "$want" ] && { [ -z "$needle" ] || printf '%s' "$out" | grep -q "$needle"; }; then
    pass=$((pass + 1))
  else
    failures+=("boundary: $label: rc=$rc: $(printf '%s' "$out" | tr '\n' ' ' | cut -c1-120)")
  fi
}
boundary="$(mktemp -d)"
mkdir -p "$boundary/src/application" "$boundary/src/tests" "$boundary/tests"
printf 'let step = "%s";\n' "$INST" > "$boundary/src/application/planner.rs"
gate_case "literal in src/application/planner.rs blocks" 2 'mutating subcommand literal'
printf '// the plan may say "%s" in a comment\n' "$INST" > "$boundary/src/application/planner.rs"
printf 'let s = "%s";\n' "$INST" > "$boundary/tests/fixture.rs"
gate_case "comment and top-level tests/ do not block" 0 ''
printf 'let s = "%s";\n' "$INST" > "$boundary/src/tests/live.rs"
gate_case "literal under src/tests/ still blocks" 2 'mutating subcommand literal'
rm -f "$boundary/src/tests/live.rs"
printf 'let s = "brew %s jq";\n' "$INST" > "$boundary/src/application/exec.rs"
gate_case "literal inside a longer string blocks" 2 'mutating subcommand literal'
printf 'fn f() {}\n' > "$boundary/src/application/exec.rs"
printf 'fn main() { let s = "%s"; }\n' "$INST" > "$boundary/build.rs"
gate_case "literal in build.rs blocks" 2 'mutating subcommand literal'
rm -f "$boundary/build.rs"
printf 'fn f() { run("%s"); }\n' "$SUDO" > "$boundary/src/application/exec.rs"
gate_case "elevation token in code blocks" 2 'INV-06'
printf 'fn f() {}\n' > "$boundary/src/application/exec.rs"
# The workspace layout: one crate per layer under crates/<layer>/src.
mkdir -p "$boundary/crates/domain/src"
printf 'let s = "%s";\n' "$INST" > "$boundary/crates/domain/src/plan.rs"
gate_case "literal under crates/*/src blocks" 2 'mutating subcommand literal'
printf 'fn f() {}\n' > "$boundary/crates/domain/src/plan.rs"
printf 'fn main() { let s = "%s"; }\n' "$INST" > "$boundary/crates/domain/build.rs"
gate_case "literal in a crate build.rs blocks" 2 'mutating subcommand literal'
rm -rf "$boundary"

# --- report -----------------------------------------------------------------
total=$((pass + ${#failures[@]}))
if [ ${#failures[@]} -gt 0 ]; then
  printf 'hook tests: %d/%d passed\n' "$pass" "$total"
  printf '  FAIL %s\n' "${failures[@]}"
  exit 1
fi
printf 'hook tests: %d/%d passed\n' "$pass" "$total"
exit 0
