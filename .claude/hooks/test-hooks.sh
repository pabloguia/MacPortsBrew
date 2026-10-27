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

# The suite asserts what the guards decide, which means it must not inherit the
# things that change their decision. Both escapes are exported deliberately, in
# a subshell, by the two cases that test them; inherited from the session they
# would turn a dozen `deny` assertions green while asserting nothing - and they
# would do it precisely during a session editing the hooks, which is the only
# time anybody runs this file with the escape armed. Found when the prompt-armed
# marker was added and this suite was run under a session that had the variable
# set, where the twelve enforcement-layer cases below all passed the wrong way.
unset MPB_ALLOW_GUARD_EDIT MPB_UNATTENDED

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
assert "traversal into a prefix is denied"    "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/../../../../../../../../../../../../opt/local/etc/x")"
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
# This file is the one CI check that cannot become a `validate.sh` stage,
# and the line below is why: it runs the stop gate, and the stop gate runs
# `validate.sh`. A stage here would recurse until something gave out. So it
# stays a workflow step and a thing you run by hand, and this comment exists
# so the next person to notice the asymmetry does not have to rediscover the
# reason (F-45).
assert "stop gate passes on a clean tree"     stop-gate.sh exit0 '{"stop_hook_active":false}'

# The branches SE-063 added, none of which the seventy cases above reach. Each
# is about the *state of a repository*, so each gets one: the live tree is dirty
# whenever anyone is working in it, which is precisely when the gate runs.
scratch_repo() {
  local dir; dir="$(mktemp -d)"
  mkdir -p "$dir/scripts" "$dir/crates" "$dir/.claude"
  printf '[workspace]\nmembers = []\n' > "$dir/Cargo.toml"
  printf '#!/bin/sh\nexit 0\n' > "$dir/scripts/validate.sh"
  chmod +x "$dir/scripts/validate.sh"
  git -C "$dir" init -q 2>/dev/null
  git -C "$dir" -c user.email=t@t -c user.name=t add -A >/dev/null 2>&1
  git -C "$dir" -c user.email=t@t -c user.name=t commit -qm base >/dev/null 2>&1
  # A second commit touching nothing the checks cover, so `HEAD@{1}` exists and
  # names a real previous position. With one commit it does not, and the gate
  # answers "unknown" - which it treats as "assume the worst", correctly, and
  # which is not the state these cases are about.
  printf 'notes\n' > "$dir/README.md"
  git -C "$dir" -c user.email=t@t -c user.name=t add -A >/dev/null 2>&1
  git -C "$dir" -c user.email=t@t -c user.name=t commit -qm notes >/dev/null 2>&1
  printf '%s' "$dir"
}
gate_in() {  # gate_in <dir> [env assignments...]
  local dir="$1"; shift
  printf '{"stop_hook_active":false}' \
    | env CLAUDE_PROJECT_DIR="$dir" "$@" "$HERE/stop-gate.sh" 2>&1
}
gate_rc_matches() {  # <rc> <want: 0|block> <output>
  if [ "$2" = "block" ]; then
    [ "$1" -eq 0 ] && printf '%s' "$3" | grep -q '"decision": "block"'
  else
    [ "$1" -eq "$2" ] && ! printf '%s' "$3" | grep -q '"decision": "block"'
  fi
}
gate_case() {  # gate_case <label> <expected rc> <needle> <dir> [env...]
  local label="$1" want="$2" needle="$3"; shift 3
  local out rc
  out="$(gate_in "$@")"; rc=$?
  # A block is exit 0 with a decision object on stdout (the documented form);
  # `want=block` asks for that, `want=0` for a pass or a warning.
  if gate_rc_matches "$rc" "$want" "$out" && printf '%s' "$out" | grep -q "$needle"; then
    pass=$((pass + 1))
  else
    failures+=("$label: expected rc=$want and \"$needle\", got rc=$rc: $(printf '%s' "$out" | tr '\n' ' ' | cut -c1-160)")
  fi
}

# `HOME` points at an empty directory because the gate prepends
# `$HOME/.cargo/bin` itself: a stripped `PATH` alone does not hide a toolchain.
nohome="$(mktemp -d)"

# 1. No toolchain, and this turn touched code. The checks did not run, which is
#    not the same as passing.
d="$(scratch_repo)"; : > "$d/crates/changed.rs"
gate_case "stop gate fails when code changed and cargo is absent" block \
  "cargo is not on PATH, and this turn" "$d" PATH=/usr/bin:/bin HOME="$nohome"
rm -rf "$d"

# 2. The same, after committing it. A committed change is still a change this
#    turn made, and reading only `git status` called that "nothing changed".
d="$(scratch_repo)"; : > "$d/crates/changed.rs"
git -C "$d" -c user.email=t@t -c user.name=t add -A >/dev/null 2>&1
git -C "$d" -c user.email=t@t -c user.name=t commit -qm work >/dev/null 2>&1
gate_case "stop gate sees a change committed mid-turn" block \
  "cargo is not on PATH, and this turn" "$d" PATH=/usr/bin:/bin HOME="$nohome"
rm -rf "$d"

# 3. No toolchain and nothing touched: a documentation turn is not held
#    hostage, and nothing may be reported as verified either.
d="$(scratch_repo)"
gate_case "stop gate warns when nothing changed and cargo is absent" 0 \
  "do not report anything as verified" "$d" PATH=/usr/bin:/bin HOME="$nohome"

# 4. And an explicit request for the tests is not answered with a warning.
gate_case "stop gate honours MPB_GATE_FULL when cargo is absent" block \
  "cargo is not on PATH" "$d" PATH=/usr/bin:/bin HOME="$nohome" MPB_GATE_FULL=1
rm -rf "$d"
rm -rf "$nohome"

# 5. The one implementation of "valid" is not runnable: fail rather than skip.
d="$(scratch_repo)"; chmod -x "$d/scripts/validate.sh"
gate_case "stop gate fails when validate.sh is not executable" block \
  "not executable" "$d"
rm -rf "$d"
broken="$(mktemp -d)"
mkdir -p "$broken/.claude" "$broken/docs"
printf 'see [missing](nowhere.md)\n' > "$broken/docs/broken.md"
out="$(printf '{"stop_hook_active":true}' | CLAUDE_PROJECT_DIR="$broken" MPB_SKIP_STOP_GATE=0 "$HERE/stop-gate.sh" 2>&1)"
rc=$?
if gate_rc_matches "$rc" block "$out" && printf '%s' "$out" | grep -q 'Broken link' && printf '%s' "$out" | grep -q 'repeated block'; then
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
boundary_case() {  # <label> <expected rc> <grep-needle-or-empty>
  local label="$1" want="$2" needle="$3" out rc
  out="$(printf '{}' | CLAUDE_PROJECT_DIR="$boundary" "$HERE/stop-gate.sh" 2>&1)"
  rc=$?
  if gate_rc_matches "$rc" "$want" "$out" && { [ -z "$needle" ] || printf '%s' "$out" | grep -q "$needle"; }; then
    pass=$((pass + 1))
  else
    failures+=("boundary: $label: rc=$rc: $(printf '%s' "$out" | tr '\n' ' ' | cut -c1-120)")
  fi
}
boundary="$(mktemp -d)"
mkdir -p "$boundary/src/application" "$boundary/src/tests" "$boundary/tests"
printf 'let step = "%s";\n' "$INST" > "$boundary/src/application/planner.rs"
boundary_case "literal in src/application/planner.rs blocks" block 'mutating subcommand literal'
printf '// the plan may say "%s" in a comment\n' "$INST" > "$boundary/src/application/planner.rs"
printf 'let s = "%s";\n' "$INST" > "$boundary/tests/fixture.rs"
boundary_case "comment and top-level tests/ do not block" 0 ''
printf 'let s = "%s";\n' "$INST" > "$boundary/src/tests/live.rs"
boundary_case "literal under src/tests/ still blocks" block 'mutating subcommand literal'
rm -f "$boundary/src/tests/live.rs"
printf 'let s = "brew %s jq";\n' "$INST" > "$boundary/src/application/exec.rs"
boundary_case "literal inside a longer string blocks" block 'mutating subcommand literal'
printf 'fn f() {}\n' > "$boundary/src/application/exec.rs"
printf 'fn main() { let s = "%s"; }\n' "$INST" > "$boundary/build.rs"
boundary_case "literal in build.rs blocks" block 'mutating subcommand literal'
rm -f "$boundary/build.rs"
printf 'fn f() { run("%s"); }\n' "$SUDO" > "$boundary/src/application/exec.rs"
boundary_case "elevation token in code blocks" block 'INV-06'
printf 'fn f() {}\n' > "$boundary/src/application/exec.rs"
# The workspace layout: one crate per layer under crates/<layer>/src.
mkdir -p "$boundary/crates/domain/src"
printf 'let s = "%s";\n' "$INST" > "$boundary/crates/domain/src/plan.rs"
boundary_case "literal under crates/*/src blocks" block 'mutating subcommand literal'
printf 'fn f() {}\n' > "$boundary/crates/domain/src/plan.rs"
printf 'fn main() { let s = "%s"; }\n' "$INST" > "$boundary/crates/domain/build.rs"
boundary_case "literal in a crate build.rs blocks" block 'mutating subcommand literal'
rm -rf "$boundary"

# --- cases added after the 2026-09-05 security review ------------------------
# Every one of these was empirically ALLOWED before the fix. They are here so
# the fix is verified on BSD tools too: this file runs on the macOS leg of
# repository-validation.yml, and the guards' regexes were written against GNU.

LN="l""n"
UP="u""p"
UNARCH="unarc""hive"
PUSH="pu""sh"

# Fail-closed on unreadable input. `jq` on empty stdin has no inputs to
# process: it exits 0 printing nothing, which read as "no command to check".
# The python3 branch already failed closed, so the guard's fail-closed
# property depended on which parser the machine happened to have.
assert "empty stdin fails closed"             "$G" closed ''
assert "whitespace-only stdin fails closed"   "$G" closed '   '
assert "top-level null fails closed"          "$G" closed 'null'
assert "top-level array fails closed"         "$G" closed '[]'
assert "empty stdin fails closed (paths)"     "$P" closed ''
assert "null fails closed (paths)"            "$P" closed 'null'

# Writes into a provider prefix from Bash. The path guard covers Edit and
# Write; Bash was the tool that could reach the artifact without spelling it.
assert "cp into a prefix is denied"           "$G" deny   "$(bash_input 'cp target/release/macportsbrew /opt/homebrew/bin/')"
assert "ln -sf into a prefix is denied"       "$G" deny   "$(bash_input 'ln -sf /tmp/x /opt/homebrew/bin/mpb')"
assert "install into a prefix is denied"      "$G" deny   "$(bash_input 'install -m 755 target/release/macportsbrew /usr/local/bin/mpb')"
assert "tee into a prefix is denied"          "$G" deny   "$(bash_input 'tee /opt/homebrew/etc/x.conf')"
assert "mkdir in a prefix is denied"          "$G" deny   "$(bash_input 'mkdir -p /opt/local/etc/mpb')"
assert "redirect into /etc/paths.d denied"    "$G" deny   "$(bash_input 'echo /opt/local/bin > /etc/paths.d/macports')"
# Reading out of a prefix stays allowed: it is how a fixture is captured.
assert "cp out of a prefix is allowed"        "$G" allow  "$(bash_input 'cp /opt/homebrew/etc/foo ./fixtures/x')"
assert "cat in a prefix is allowed"           "$G" allow  "$(bash_input 'cat /opt/local/etc/macports/macports.conf')"
assert "ls in a prefix is allowed"            "$G" allow  "$(bash_input 'ls /opt/homebrew/bin')"

# In-place editors: the tested spelling was the least likely one. BSD sed
# takes `-i ''` and `-i.bak`, and `perl -pi -e` is the commonest idiom there is.
assert "sed -i.bak on a dotfile is denied"    "$G" deny   "$(bash_input "sed -i.bak s/a/b/ ~/$ZRC")"
assert "perl -pi -e on a dotfile is denied"   "$G" deny   "$(bash_input "perl -pi -e s/a/b/ ~/$ZRC")"
assert "sed -i on a repo file is allowed"     "$G" allow  "$(bash_input 'sed -i.bak s/a/b/ docs/README.md')"

# Subcommand aliases the hand-written list missed. `brew ln` is `brew link`,
# `brew up` is `brew update`, and both are real spellings.
assert "brew $LN is denied"                   "$G" deny   "$(bash_input "brew $LN --overwrite python@3.12")"
assert "brew $UP is denied"                   "$G" deny   "$(bash_input "brew $UP")"
assert "port $UNARCH is denied"               "$G" deny   "$(bash_input "port $UNARCH jq")"
assert "rustup update is denied"              "$G" deny   "$(bash_input 'rustup update')"
assert "brew shellenv stays allowed"          "$G" allow  "$(bash_input 'brew shellenv')"

# The pin that holds the deployment floor. The rustup rule's own deny message
# points the agent at this file, which made the file the unguarded path.
assert "editing the pin from Bash is denied"  "$G" deny   "$(bash_input 'sed -i s/1.98.0/stable/ rust-toolchain.toml')"
assert "reading the pin is allowed"           "$G" allow  "$(bash_input 'cat rust-toolchain.toml')"
assert "editing the pin is denied"            "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/rust-toolchain.toml")"

# Publishing (DP-007, owner's ruling of 2026-10-16): an ordinary push of a topic
# branch to origin is allowed; whatever rewrites or removes what is published,
# aims at the default branch or names another destination is not. The guard reads
# the origin remote and the current branch of CLAUDE_PROJECT_DIR, so each case
# runs in a scratch repository.
push_repo() {  # push_repo <branch> <origin url>
  local d; d="$(mktemp -d)"
  git -C "$d" init -q 2>/dev/null
  git -C "$d" -c user.email=t@t -c user.name=t commit -q --allow-empty -m i 2>/dev/null
  git -C "$d" switch -q -c "$1" 2>/dev/null
  git -C "$d" remote add origin "$2"
  printf '%s' "$d"
}
OK_URL="git@github.com:pabloguia/MacPortsBrew.git"
d="$(push_repo topic "$OK_URL")"
push_assert() {  # push_assert <label> <allow|deny> <command>
  # A prefix assignment on a function call lasts for that call only, so the
  # guard sees the scratch repository and every later case sees the real one.
  CLAUDE_PROJECT_DIR="$d" assert "$1" "$G" "$2" "$(bash_input "$3")"
}
push_assert "git $PUSH of a topic branch is allowed"          allow "git $PUSH -u origin topic"
push_assert "git -C dir $PUSH to origin is allowed"           allow "git -C . $PUSH origin topic"
push_assert "git $PUSH --force is denied"                     deny  "git $PUSH --force origin topic"
push_assert "git $PUSH -f is denied"                          deny  "git $PUSH -f origin topic"
push_assert "git $PUSH --force-with-lease is denied"          deny  "git $PUSH --force-with-lease origin topic"
push_assert "git $PUSH --delete is denied"                    deny  "git $PUSH --delete origin topic"
push_assert "git $PUSH a deleting refspec is denied"          deny  "git $PUSH origin :topic"
push_assert "git $PUSH a forcing refspec is denied"           deny  "git $PUSH origin +topic"
push_assert "git $PUSH --mirror is denied"                    deny  "git $PUSH --mirror origin"
push_assert "git $PUSH --all is denied"                       deny  "git $PUSH --all origin"
push_assert "git $PUSH --no-verify is denied"                 deny  "git $PUSH --no-verify origin topic"
push_assert "git $PUSH to main is denied"                     deny  "git $PUSH origin main"
push_assert "git $PUSH HEAD:main is denied"                   deny  "git $PUSH origin HEAD:main"
push_assert "git $PUSH to master is denied"                   deny  "git $PUSH origin master"
push_assert "git $PUSH to a URL is denied"                    deny  "git $PUSH https://github.com/someone/else.git topic"
push_assert "git $PUSH to an ssh URL is denied"               deny  "git $PUSH git@github.com:someone/else.git topic"
push_assert "git $PUSH to a path is denied"                   deny  "git $PUSH /tmp/elsewhere topic"
# SE-108: the whole line is read, so none of the forms a push-segment reading
# let through (both reviewers, 2026-10-27) is allowed; and tags go one by one.
push_assert "cd /abs && git $PUSH is denied"                  deny  "cd /tmp/other && git $PUSH"
push_assert "a subshell cd and git $PUSH is denied"           deny  "(cd /tmp/x; git $PUSH origin topic)"
push_assert "xargs feeding main to git $PUSH is denied"       deny  "echo main | xargs git $PUSH origin"
push_assert "xargs feeding --force to git $PUSH is denied"    deny  "echo --force | xargs git $PUSH origin topic"
push_assert "a substituted --force on git $PUSH is denied"    deny  "git $PUSH \$(true; echo --force) origin topic"
push_assert "a URL set before git $PUSH is denied"            deny  "U=https://github.com/x/y; git $PUSH \$U topic"
push_assert "git $PUSH --tags is denied"                      deny  "git $PUSH origin --tags"
push_assert "git $PUSH --follow-tags is denied"               deny  "git $PUSH --follow-tags origin topic"
push_assert "git $PUSH of one release tag is allowed"         allow "git $PUSH origin v0.2.0"
rm -rf "$d"
d="$(push_repo main "$OK_URL")"
push_assert "git $PUSH from main is denied"                   deny  "git $PUSH origin"
rm -rf "$d"
d="$(push_repo topic "git@github.com:someone/else.git")"
push_assert "git $PUSH with a foreign origin is denied"       deny  "git $PUSH -u origin topic"
rm -rf "$d"
d="$(push_repo topic "https://github.com/pabloguia/MacPortsBrew.git")"
push_assert "git $PUSH with the https origin is allowed"      allow "git $PUSH -u origin topic"
rm -rf "$d"
unset d
assert "git commit naming push is allowed"    "$G" allow  "$(bash_input "git commit -m 'add $PUSH support'")"
assert "git log is allowed"                   "$G" allow  "$(bash_input 'git log --oneline')"

# Merging and releasing (SE-108): branch protection does not bind the owner's
# credential, so the hook keeps a merge behind the required checks.
MRG="pr merge"
assert "gh $MRG of a green PR is allowed"     "$G" allow  "$(bash_input "gh $MRG 22 --merge")"
assert "gh $MRG --admin is denied"            "$G" deny   "$(bash_input "gh $MRG 22 --merge --admin")"
assert "gh api read is allowed"               "$G" allow  "$(bash_input 'gh api repos/o/r/branches/main/protection')"
assert "gh api -X PUT is denied"              "$G" deny   "$(bash_input 'gh api -X PUT repos/o/r/pulls/1/merge')"
assert "gh api --method=DELETE is denied"     "$G" deny   "$(bash_input 'gh api --method=DELETE repos/o/r/branches/main/protection')"
assert "gh api with a field is denied"        "$G" deny   "$(bash_input 'gh api repos/o/r/rulesets -f name=x')"
assert "gh api with --input is denied"        "$G" deny   "$(bash_input 'gh api repos/o/r/environments/e --input body.json')"
MPB_UNATTENDED=1 assert "unattended gh $MRG is denied"     "$G" deny "$(bash_input "gh $MRG 22 --merge")"
MPB_UNATTENDED=1 assert "unattended gh release is denied"  "$G" deny "$(bash_input 'gh release create v0.2.0')"
d="$(push_repo topic "$OK_URL")"
MPB_UNATTENDED=1 push_assert "unattended git $PUSH of a tag is denied" deny "git $PUSH origin v0.2.0"
rm -rf "$d"
unset d

# Tier 1, and the control protecting itself.
assert "editing the invariants is denied"     "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/docs/spec/01-invariants.md")"
assert "editing a hook is denied"             "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/.claude/hooks/stop-gate.sh")"
# The patterns added when `.codex/` became generated, and the three the
# preflight reviews found missing. Without these, the guard widened with
# nothing asserting it - in the file whose whole purpose is "hooks decide
# what the specification says they decide".
assert "editing the codex layer is denied"    "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/.codex/hooks/stop-gate.sh")"
assert "editing a generated skill is denied"  "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/.agents/skills/adr/SKILL.md")"
assert "editing the generator is denied"      "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/scripts/generate-codex-layer.sh")"
# Guidance, not controls, since SE-093: a session may edit its subagents,
# skills and rules; the generated layers they feed stay guarded.
assert "editing a subagent is allowed"        "$P" allow  "$(edit_input "$CLAUDE_PROJECT_DIR/.claude/agents/spec-guardian.md")"
assert "editing a skill is allowed"           "$P" allow  "$(edit_input "$CLAUDE_PROJECT_DIR/.claude/skills/adr/SKILL.md")"
assert "editing a path rule is allowed"       "$P" allow  "$(edit_input "$CLAUDE_PROJECT_DIR/.claude/rules/adapters.md")"
assert "editing local settings is denied"     "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/.claude/settings.local.json")"
assert "editing settings.json is denied"      "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/.claude/settings.json")"
assert "editing validate.sh is denied"        "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/scripts/validate.sh")"
assert "editing a workflow is denied"         "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/.github/workflows/claude.yml")"
assert "editing a spec chapter is allowed"    "$P" allow  "$(edit_input "$CLAUDE_PROJECT_DIR/docs/spec/11-agentic-sdlc.md")"
assert "editing a crate source is allowed"    "$P" allow  "$(edit_input "$CLAUDE_PROJECT_DIR/crates/domain/src/lib.rs")"
# Exported and unset around the case rather than wrapped in `( ... )`. A
# subshell discards `failures`, so every assertion written inside one could
# report a problem to nobody: the three cases that used a subshell here - this
# one and the two unattended ones below - could not have failed this suite,
# which is the shape of defect this file exists to catch. Demonstrated with a
# three-line reproduction before it was changed.
# Pointed at a scratch project root, because an *allowed* enforcement-layer
# write is appended to `.claude/.guard-edit-log`, and a suite that writes into
# the real disclosure log makes the end-of-pass report cite its own test runs as
# though they were edits. Found by reading the log after the first run: it named
# `stop-gate.sh`, `claude.yml` and `spec-guardian.md`, none of which had been
# touched. Evidence a gate fabricates is worse than evidence it does not keep.
ESCAPE_SANDBOX="$(mktemp -d)"
mkdir -p "$ESCAPE_SANDBOX/.claude"   # the log lives here; without it the write is refused
REAL_DIR="$CLAUDE_PROJECT_DIR"
export CLAUDE_PROJECT_DIR="$ESCAPE_SANDBOX"
export MPB_ALLOW_GUARD_EDIT=1
assert "the disclosed escape opens it"        "$P" allow  "$(edit_input "$ESCAPE_SANDBOX/.claude/hooks/stop-gate.sh")"
unset MPB_ALLOW_GUARD_EDIT
export CLAUDE_PROJECT_DIR="$REAL_DIR"
rm -rf "$ESCAPE_SANDBOX"

# --- the prompt-armed escape ------------------------------------------------
#
# The same escape, armed from the human's message instead of the environment.
# The cases that matter are the ones where it must NOT arm: the token reaching
# the hook by any route other than the `prompt` field authorises nothing, and a
# marker left by another session authorises nothing here.
#
# These write and delete the real marker, so they run against a copy of the
# project root rather than the live one - a suite that disarmed the session
# running it would be a gate with a side effect on the thing it guards.
A=arm-guard-edit.sh
GUARD_TOKEN="MPB_ALLOW_GUARD""_EDIT=1"
prompt_input() { # <prompt-text> [session]
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import json,sys; print(json.dumps({"hook_event_name":"UserPromptSubmit","session_id":sys.argv[2],"cwd":".","prompt":sys.argv[1]}))' "$1" "${2:-S1}"
  else
    jq -cn --arg p "$1" --arg s "${2:-S1}" '{hook_event_name:"UserPromptSubmit",session_id:$s,cwd:".",prompt:$p}'
  fi
}
edit_session_input() { # <path> <session>
  printf '{"tool_name":"Edit","session_id":"%s","tool_input":{"file_path":"%s"}}' "$2" "$1"
}
REAL_PROJECT_DIR="$CLAUDE_PROJECT_DIR"
{
  SANDBOX="$(mktemp -d)"
  mkdir -p "$SANDBOX/.claude"
  export CLAUDE_PROJECT_DIR="$SANDBOX"
  MARK="$SANDBOX/.claude/.guard-edit-marker"
  LOG="$SANDBOX/.claude/.guard-edit-log"
  TARGET="$SANDBOX/.claude/hooks/stop-gate.sh"

  # Arming, and what it writes.
  assert "the token arms and says so"         "$A" context "$(prompt_input "$GUARD_TOKEN please fix the gate")"
  [ -f "$MARK" ] && pass=$((pass + 1)) || failures+=("the token writes a marker")
  [ "$(sed -n '1p' "$MARK")" = "S1" ] && pass=$((pass + 1)) || failures+=("the marker records the session")
  grep -q 'please fix the gate' "$MARK" && pass=$((pass + 1)) || failures+=("the marker records the prompt that armed it")

  # The guard honours it, for that session only, and records the write.
  assert "an armed prompt opens the layer"    "$P" allow  "$(edit_session_input "$TARGET" S1)"
  grep -q 'stop-gate.sh' "$LOG" && pass=$((pass + 1)) || failures+=("an allowed write is written to the disclosure log")
  grep -q 'please fix the gate' "$LOG" && pass=$((pass + 1)) || failures+=("the log names the prompt that authorised it")
  assert "another session is not armed"       "$P" deny   "$(edit_session_input "$TARGET" S2)"
  assert "a payload with no session is not"   "$P" deny   "$(edit_input "$TARGET")"

  # The marker and the log are refused even while armed: a session that could
  # edit them could authorise itself and then rewrite the account of it.
  assert "the marker itself is never writable" "$P" deny  "$(edit_session_input "$MARK" S1)"
  assert "the log itself is never writable"    "$P" deny  "$(edit_session_input "$LOG" S1)"

  # DP-020: the escape's weak half must not be able to manufacture its strong
  # half. `settings.local.json` carries an `env` block that reaches hook
  # processes, and it is git-ignored, so writing it under the marker would arm
  # every later session by the environment-variable route with nothing in any
  # diff to show for it. Refused whether or not the escape is armed.
  assert "local settings denied while armed"   "$P" deny \
    "$(edit_session_input "$SANDBOX/.claude/settings.local.json" S1)"
  # The other half of the same ruling, asserted rather than left to the reader:
  # `settings.json` stays writable under the escape, because a change to it is
  # committed and reviewable. The asymmetry *is* the decision.
  assert "committed settings stay openable"    "$P" allow \
    "$(edit_session_input "$SANDBOX/.claude/settings.json" S1)"

  # Disarming: the next message without the token clears it. This is the whole
  # of the expiry - no timer, rewritten by the event that defines the boundary.
  assert "a message without the token"        "$A" exit0  "$(prompt_input "carry on then")"
  [ ! -f "$MARK" ] && pass=$((pass + 1)) || failures+=("a message without the token disarms")
  assert "and the layer is closed again"      "$P" deny   "$(edit_session_input "$TARGET" S1)"

  # The token arriving by any route that is not the human's message. This is
  # the property the whole design rests on: the hook reads one field, and tool
  # results, file contents and subagent reports never reach this event at all.
  assert "the token in another field arms nothing" "$A" exit0 \
    "$(printf '{"hook_event_name":"UserPromptSubmit","session_id":"S1","cwd":"%s","prompt":"summarise the file"}' "$GUARD_TOKEN")"
  [ ! -f "$MARK" ] && pass=$((pass + 1)) || failures+=("the token outside the prompt field arms nothing")

  # THESE TWO ASSERTIONS USED TO SAY THE OPPOSITE, AND THE RULE THEY ASSERTED
  # WAS DISPROVED TWICE BY THIS ESCAPE ARMING ITSELF (F-52). The comment here
  # read "quoting it back is still the human saying it - there is no way to tell
  # them apart", and the case below asserted that a mid-line quotation arms.
  # Both times the escape armed on its own it was a reviewing subagent quoting
  # the specification that describes the escape, in exactly that shape.
  #
  # There is a way to tell them apart, and it is not perfect: the token must be
  # the FIRST THING ON A LINE. Every one of the token's occurrences in the
  # tracked corpus sits inside prose or backticks, `scripts/test-gates.sh`
  # asserts that no tracked line begins with it, and a quotation reproducing a
  # source line therefore cannot arm. What it does not cover is a quotation that
  # *reformats* - into a fenced block, a list item, a diff hunk - and the
  # git-ignored marker, whose excerpt now begins with the token by construction.
  # See the hook's header; DP-022 carries the two changes that would close it.
  assert "a mid-line quotation arms nothing"  "$A" context "$(prompt_input "the hook looks for $GUARD_TOKEN in the message")"
  [ ! -f "$MARK" ] && pass=$((pass + 1)) || failures+=("a token quoted mid-line arms nothing")

  # An INDENTED line carrying the token arms nothing - subsumed by the
  # line-start rule now, and kept because it is the shape the first live
  # self-arming took: a subagent hand-back reaches the session through the
  # prompt channel, and the harness indents relayed output.
  assert "an indented relay arms nothing"     "$A" exit0 \
    "$(prompt_input "[Subagent hand-back]
  The report below quotes $GUARD_TOKEN while describing it.")"
  [ ! -f "$MARK" ] && pass=$((pass + 1)) || failures+=("an indented relayed token arms nothing")
  assert "a tab-indented quote arms nothing"  "$A" exit0 "$(prompt_input "	quoting $GUARD_TOKEN back")"
  [ ! -f "$MARK" ] && pass=$((pass + 1)) || failures+=("a tab-indented token arms nothing")

  # A relay envelope arms nothing even when the token DOES open a line. The
  # second self-arming came in a `<task-notification>`, which did not indent, so
  # the line-start rule alone would not have stopped it. Naming envelopes is a
  # filter and the hook says so; these are the shapes that have actually fired.
  for envelope in "<task-notification><task-id>a507</task-id>" \
                  "<agent-message from=spec-guardian>" \
                  "<system-reminder>" \
                  "[SYSTEM NOTIFICATION - NOT USER INPUT]"; do
    assert "a relay envelope arms nothing"    "$A" exit0 \
      "$(prompt_input "$envelope
$GUARD_TOKEN is the disclosed escape")"
    [ ! -f "$MARK" ] && pass=$((pass + 1)) || failures+=("a relay envelope carrying the token arms nothing: $envelope")
  done

  # And the human's own sentence still arms, token opening the line, which is
  # how it is actually typed.
  assert "the human's own line still arms"    "$A" context \
    "$(prompt_input "$GUARD_TOKEN this session was opened deliberately, to edit the gates")"
  [ -f "$MARK" ] && pass=$((pass + 1)) || failures+=("a human line opening with the token arms")

  # Closed means "not armed", never "the owner's message is rejected": exit 2 on
  # UserPromptSubmit blocks the prompt itself (tooling recheck D3).
  assert "unreadable prompt input arms nothing and lets the prompt through" "$A" exit0 '{bad'
  [ ! -f "$MARK" ] && pass=$((pass + 1)) || failures+=("an unreadable prompt disarms rather than leaving it armed")

  # --- the five a security review found untested, each one a real defect ---

  # The disarm must happen before anything that can exit. With the parser check
  # ahead of it, a message arriving while jq and python3 were both unreachable
  # exited 2 and left the previous message's marker armed - for the rest of the
  # session, not just that message.
  assert "re-arming for the next case"        "$A" context "$(prompt_input "$GUARD_TOKEN again")"
  # A PATH with the ordinary tools on it and *no JSON parser* - which is the
  # real shape of this failure. Emptying PATH instead would remove `rm` as well
  # and prove only that a hook with no `rm` cannot delete anything.
  MINIBIN="$SANDBOX/minibin"
  mkdir -p "$MINIBIN"
  # `bash` is in the list because the shebang is `/usr/bin/env bash`, and `env`
  # resolves `bash` through PATH - without it the hook never starts, and the
  # marker survives for a reason that has nothing to do with the parser.
  for t in bash rm cat date sed tr cut mkdir; do
    p="$(command -v "$t")" && ln -sf "$p" "$MINIBIN/$t"
  done
  rc=0; PATH="$MINIBIN" "$HERE/$A" <<<"$(prompt_input "no token here")" >/dev/null 2>&1 || rc=$?
  [ ! -f "$MARK" ] && pass=$((pass + 1)) || failures+=("a hook that cannot run its parser still disarms")
  [ "$rc" -eq 0 ] && pass=$((pass + 1)) || failures+=("a hook that cannot run its parser does not reject the prompt (rc=$rc)")

  # A symlinked marker is not the marker. `[ -f ]` follows one, and pointing it
  # at a file the Write tool will happily write turns "needs Bash once per
  # arming" into "needs Bash once".
  assert "re-arming to test the symlink"      "$A" context "$(prompt_input "$GUARD_TOKEN again")"
  mv "$MARK" "$SANDBOX/elsewhere"
  ln -s "$SANDBOX/elsewhere" "$MARK"
  assert "a symlinked marker arms nothing"    "$P" deny   "$(edit_session_input "$TARGET" S1)"
  rm -f "$MARK"

  # A write the escape permits but cannot record must not happen. Detection is
  # the whole of what the prompt-armed route rests on, so a swallowed log
  # failure would buy a silent enforcement-layer write - worse than the deny it
  # replaced.
  assert "re-arming to test the log"          "$A" context "$(prompt_input "$GUARD_TOKEN again")"
  rm -f "$LOG"
  mkdir "$LOG"   # a directory cannot be appended to
  assert "an unrecordable write is refused"   "$P" deny   "$(edit_session_input "$TARGET" S1)"
  rmdir "$LOG"

  # One write, one line. `check` runs on the path as written and again on its
  # canonical form, and a log that counts one edit twice is one a report
  # overstates from.
  assert "the write is allowed again"         "$P" allow  "$(edit_session_input "$TARGET" S1)"
  [ "$(wc -l <"$LOG" | tr -d ' ')" = "1" ] && pass=$((pass + 1)) || failures+=("one permitted write writes exactly one log line")

  # A path may contain a tab or a newline, and `*` in the deny patterns spans
  # both - so an unescaped path can close its own record and open a complete,
  # plausible, fabricated one after it.
  # JSON-encoded rather than interpolated: a raw newline inside a JSON string is
  # not JSON, and the guard would fail closed on the payload instead of being
  # tested on the path.
  forged_path="$(printf '%s/scripts/a\n2020-01-01T00:00:00Z\t/forged/path.sh\tforged\nb.sh' "$SANDBOX")"
  if command -v python3 >/dev/null 2>&1; then
    forged_json="$(python3 -c 'import json,sys; print(json.dumps({"tool_name":"Edit","session_id":"S1","tool_input":{"file_path":sys.argv[1]}}))' "$forged_path")"
  else
    forged_json="$(jq -cn --arg p "$forged_path" '{tool_name:"Edit",session_id:"S1",tool_input:{file_path:$p}}')"
  fi
  assert "a newline in the path is squashed"  "$P" allow "$forged_json"
  [ "$(wc -l <"$LOG" | tr -d ' ')" = "2" ] && pass=$((pass + 1)) || failures+=("a path with a newline cannot forge a second log line")

  rm -rf "$SANDBOX"
}
export CLAUDE_PROJECT_DIR="$REAL_PROJECT_DIR"

# The unattended boundary: 11-agentic-sdlc.md reserves decisions-pending and
# ADRs for a human in the loop. The workflows set MPB_UNATTENDED=1.
assert "interactive may edit decisions"       "$P" allow  "$(edit_input "$CLAUDE_PROJECT_DIR/docs/decisions-pending.md")"
export MPB_UNATTENDED=1
assert "unattended may not edit decisions"    "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/docs/decisions-pending.md")"
assert "unattended may not edit an ADR"       "$P" deny   "$(edit_input "$CLAUDE_PROJECT_DIR/docs/adr/0002-implementation-language.md")"
unset MPB_UNATTENDED

# A user prefix and a case variant. ~/.homebrew is Homebrew's supported
# non-admin prefix; macOS's default volume is case-insensitive, so
# /opt/Homebrew and /opt/homebrew are one file that `case` saw as two.
assert "a user .homebrew prefix is denied"    "$P" deny   "$(edit_input "$HOME/.homebrew/bin/x")"
assert "a case-variant prefix is denied"      "$P" deny   "$(edit_input '/opt/Homebrew/etc/x')"

# --- the commit gate (SE-093; tooling recheck D2, D2b) -----------------------
# Every way it can fail to validate must refuse: Claude Code proceeds on an
# exit 1 and on a timed-out hook. Each case runs here, not in a subshell, so a
# failure reaches the report.
C=commit-gate.sh
COMMIT="git ""commit -m x"
d="$(scratch_repo)"
if out="$(printf '%s' "$(bash_input 'git status')" | CLAUDE_PROJECT_DIR="$d" "$HERE/$C" 2>&1)" && [ -z "$out" ]; then
  pass=$((pass + 1))
else
  failures+=("the commit gate ignores a command that is not a commit: $out")
fi
CLAUDE_PROJECT_DIR="$d" assert "a passing validator lets a commit through" "$C" allow "$(bash_input "$COMMIT")"
printf '#!/bin/sh\necho broken; exit 1\n' >"$d/scripts/validate.sh"
CLAUDE_PROJECT_DIR="$d" assert "a failing validator refuses the commit" "$C" deny "$(bash_input "$COMMIT")"
printf '#!/bin/sh\nsleep 30\n' >"$d/scripts/validate.sh"
CLAUDE_PROJECT_DIR="$d" MPB_COMMIT_GATE_DEADLINE=1 assert "a validator past the deadline refuses" "$C" deny "$(bash_input "$COMMIT")"
rm -f "$d/scripts/validate.sh"
CLAUDE_PROJECT_DIR="$d" assert "a missing validator refuses the commit" "$C" deny "$(bash_input "$COMMIT")"
CLAUDE_PROJECT_DIR="$d/does-not-exist" assert "an unreachable project refuses the commit" "$C" deny "$(bash_input "$COMMIT")"
rm -rf "$d"

# --- report -----------------------------------------------------------------
total=$((pass + ${#failures[@]}))
if [ ${#failures[@]} -gt 0 ]; then
  printf 'hook tests: %d/%d passed\n' "$pass" "$total"
  printf '  FAIL %s\n' "${failures[@]}"
  exit 1
fi
printf 'hook tests: %d/%d passed\n' "$pass" "$total"
exit 0
