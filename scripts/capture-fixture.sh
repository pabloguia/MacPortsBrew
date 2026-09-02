#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Captures a fixture: the read-only provider and system commands the
# adapters use, recorded verbatim into the Recording format that
# crates/adapters/src/process/recording.rs replays
# (docs/spec/07-quality-and-testing.md, "Fixtures": captured, never
# invented, with provenance).
#
#   scripts/capture-fixture.sh <fixture-name> [output-path]
#
# Only allow-listed read-only commands run (the same list the adapters use;
# keep the two in step). The environment is the sanitised one the executor
# builds: no inherited variables, the provider quieting variables set.
# Output is redacted: the home directory becomes `~` and the user name is
# never written. Needs jq.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

name="${1:?fixture name}"
out="${2:-fixtures/recordings/${name}.json}"
{ command -v jq >/dev/null && command -v python3 >/dev/null; } || { echo "capture-fixture: jq and python3 are required" >&2; exit 2; }

HOME_DIR="${HOME:-}"
USER_NAME="$(id -un)"
redact() {
  # Home path and user name never enter a fixture.
  local s="$1"
  [ -n "$HOME_DIR" ] && s="${s//$HOME_DIR/~}"
  s="${s//$USER_NAME/USER}"
  printf '%s' "$s"
}

# The sanitised environment (mirror of adapters/process/environment.rs).
mk_env() { # <leading-path-or-empty> <extra key=value...>
  local lead="$1"; shift
  local path="/usr/bin:/bin:/usr/sbin:/sbin"
  [ -n "$lead" ] && path="$lead:$path"
  local json
  json="$(jq -n --arg p "$path" --arg h "$HOME_DIR" \
    '{PATH:$p, LANG:"C", LC_ALL:"C", TERM:"dumb", NO_COLOR:"1", HOME:$h}')"
  for kv in "$@"; do
    json="$(jq --arg k "${kv%%=*}" --arg v "${kv#*=}" '. + {($k): $v}' <<<"$json")"
  done
  printf '%s' "$json"
}

invocations='[]'
record() { # <env-json> <program-path> <args...>
  local envj="$1" prog="$2"; shift 2
  local name; name="$(basename "$prog")"
  local start end ms stdout stderr status
  start=$(python3 -c 'import time; print(int(time.time() * 1000))')
  if [ ! -x "$prog" ]; then
    status='{"status":"NOT_STARTED","error":"No such file or directory (os error 2)"}'
    stdout=""; stderr=""
  else
    local so se envpairs=()
    so="$(mktemp)"; se="$(mktemp)"
    while IFS= read -r kv; do envpairs+=("$kv"); done < <(jq -r 'to_entries[] | "\(.key)=\(.value)"' <<<"$envj")
    set +e
    env -i "${envpairs[@]}" "$prog" "$@" >"$so" 2>"$se" </dev/null
    rc=$?
    set -e
    stdout="$(cat "$so")"; stderr="$(cat "$se")"
    rm -f "$so" "$se"
    status="$(jq -n --argjson c "$rc" '{status:"EXITED",code:$c}')"
  fi
  end=$(python3 -c 'import time; print(int(time.time() * 1000))')
  ms=$(( end - start ))
  local argv; argv="$(printf -- '%s\n' "$@" | jq -R . | jq -s .)"
  local envr; envr="$(redact "$envj")"
  for check in "argv:$argv" "environment:$envr" "status:$status"; do
    jq -e . <<<"${check#*:}" >/dev/null 2>&1 || { echo "capture-fixture: internal error, ${check%%:*} is not JSON: ${check#*:}" >&2; exit 3; }
  done
  invocations="$(jq --arg n "$name" --argjson a "$argv" --argjson e "$envr" \
    --argjson s "$status" --arg o "$(redact "$stdout")" --arg r "$(redact "$stderr")" \
    --argjson ms "$ms" \
    '. + [{program:$n, argv:$a, environment:$e, status:$s, stdout:$o, stderr:$r, truncated:false, durationMs:$ms}]' \
    <<<"$invocations")"
  echo "capture-fixture: $name $* -> $(jq -r '.status' <<<"$status")" >&2
}

sys_env="$(mk_env "")"
record "$sys_env" /usr/bin/sw_vers -productVersion
record "$sys_env" /usr/bin/sw_vers -buildVersion
record "$sys_env" /usr/bin/uname -m
record "$sys_env" /usr/sbin/sysctl -n hw.machine
record "$sys_env" /usr/sbin/sysctl -n hw.optional.arm64
record "$sys_env" /usr/sbin/sysctl -n sysctl.proc_translated
record "$sys_env" /usr/bin/xcode-select -p

# Homebrew: the executable is resolved from the candidate prefixes, never
# from PATH (threat model: executable shadowing).
brew_bin=""
for p in /opt/homebrew/bin/brew /usr/local/bin/brew; do [ -x "$p" ] && { brew_bin="$p"; break; }; done
brew_env="$(mk_env "$(dirname "${brew_bin:-/usr/local/bin/brew}")" \
  HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1 \
  HOMEBREW_NO_INSTALL_CLEANUP=1 HOMEBREW_NO_COLOR=1 HOMEBREW_NO_EMOJI=1)"
brew_prog="${brew_bin:-/usr/local/bin/brew}"
record "$brew_env" "$brew_prog" --version
record "$brew_env" "$brew_prog" --prefix
record "$brew_env" "$brew_prog" --cellar
record "$brew_env" "$brew_prog" --caskroom
record "$brew_env" "$brew_prog" --cache
record "$brew_env" "$brew_prog" config
record "$brew_env" "$brew_prog" info --json=v2 --installed
record "$brew_env" "$brew_prog" list --pinned

# MacPorts: likewise from its prefix.
port_bin=""
[ -x /opt/local/bin/port ] && port_bin=/opt/local/bin/port
port_env="$(mk_env "/opt/local/bin:/opt/local/sbin")"
port_prog="${port_bin:-/opt/local/bin/port}"
record "$port_env" "$port_prog" version
record "$port_env" "$port_prog" -q installed
record "$port_env" "$port_prog" -q echo requested
record "$port_env" "$port_prog" -q echo active
record "$port_env" "$port_prog" -q echo leaves
record "$port_env" "$port_prog" dir

# Port metadata: one PortIndex field group per invocation, over every
# installed name. `port info --line` documents its separator only as "Fields
# are separated by spaces" and the one official example shows a field that
# itself contains a space (R-3 E-8), so each invocation below carries at most
# one variable-width field and that field is last. The names come from the
# registry, which is why this runs after `-q installed`.
port_names=""
[ -n "$port_bin" ] && port_names="$("$port_bin" -q installed 2>/dev/null \
  | sed -E 's/^[[:space:]]*//; s/[[:space:]].*$//' | sort -u | tr '\n' ' ')"
if [ -n "$port_names" ]; then
  for fields in "--version --revision --replaced_by" "--homepage --license" \
                "--description" "--depends_lib" "--depends_run" "--depends_build"; do
    # Word splitting is intended: port names match [A-Za-z0-9_.+-] only.
    # shellcheck disable=SC2086
    record "$port_env" "$port_prog" -q info --index --line --name $fields $port_names
  done
fi

macos="$(sw_vers -productVersion)"; arch="$(uname -m)"
versions='[]'
[ -n "$brew_bin" ] && versions="$(jq --arg v "homebrew $("$brew_bin" --version 2>/dev/null | head -1)" '. + [$v]' <<<"$versions")"
[ -n "$port_bin" ] && versions="$(jq --arg v "macports $("$port_bin" version 2>/dev/null | head -1)" '. + [$v]' <<<"$versions")"
now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
mkdir -p "$(dirname "$out")"
jq -n --arg macos "$macos" --arg arch "$arch" --argjson pv "$versions" --arg now "$now" \
  --arg by "scripts/capture-fixture.sh $name" --argjson inv "$invocations" \
  '{schemaVersion:1, provenance:{macos:$macos, architecture:$arch, providerVersions:$pv,
    capturedAt:$now, capturedBy:$by, origin:"captured", note:null}, invocations:$inv}' > "$out"
echo "capture-fixture: wrote $out ($(wc -c < "$out" | tr -d ' ') bytes, $(jq '.invocations|length' "$out") invocations)"
