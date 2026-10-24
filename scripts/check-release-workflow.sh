#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# The release-workflow MUSTs of docs/spec/08-distribution-and-project.md and
# ADR-0005 that are properties of workflow text, asserted rather than written
# down (pass 09, D-T2). Over every workflow in .github/workflows/ and every
# composite action under .github/actions/:
#
#   1. no `pull_request_target` or `workflow_run` trigger;
#   2. no ACTIONS_STEP_DEBUG or ACTIONS_RUNNER_DEBUG;
#   3. no `secrets[...]` and no `toJSON(secrets)` - both expose every secret;
#   4. `release-signing` and `secrets.MPB_*` appear in release.yml only.
#
# And over release.yml itself:
#
#   5. its `on:` is exactly `push` with `tags: ["v*"]`, and `workflow_dispatch`;
#   6. exactly one line selects the environment, and it is
#      `environment: release-signing` (a `name: release-signing` line would be
#      a second selection);
#   7. the job holding that environment runs no cargo, rustup, Rust toolchain
#      action, validate.sh or run-fixture-matrix - nothing that compiles or
#      executes the project next to a credential;
#   8. `secrets.` appears inside that job only;
#   9. no xtrace anywhere: no `set` whose short-option clusters contain x
#      (`set -x`, `set -eux`, `set -e -x`), no `xtrace` (`set -o xtrace`,
#      `shopt`), no SHELLOPTS, no BASH_XTRACEFD, no `bash -x`, no `shell:`
#      with -x. "None in a step that sees a credential" is guaranteed by
#      "none at all";
#  10. nothing in release.yml builds with a feature: (a) no non-comment line
#      names `cargo` at all, so every release build goes through
#      scripts/build-release.sh and whatever spelling a cargo line could take
#      is moot; (b) no non-comment line holds `--features` or `--all-features`,
#      quoted or not; (c) no `-F`, alone or inside a cluster of short flags
#      (`-rF`), quoted or not, on a line that names build-release.sh, a
#      backslash continuation counted as one line. A release is built without
#      `test-support`, which compiles the test-only mutation allowance in
#      (pass 08 slice 4 review L1). Only release.yml is read, so validate.sh's
#      own `cargo test --all-features` is not touched. scripts/build-release.sh
#      asks cargo the same question of the dependency graph.
#
# Comment lines are ignored, so a file may name what it forbids. Run from
# anywhere inside the repository. MPB_WORKFLOW_DIR and MPB_ACTIONS_DIR override
# the two directories so that scripts/test-gates.sh can run it over
# constructed fixtures.
#
#   scripts/check-release-workflow.sh

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
dir="${MPB_WORKFLOW_DIR:-.github/workflows}"
actions="${MPB_ACTIONS_DIR:-.github/actions}"
release="$dir/release.yml"
failures=0
fail() { printf 'check-release-workflow: FAIL: %s\n' "$*" >&2; failures=$((failures + 1)); }
hits_of() { # <body> <extended regex> [-i]; prints matching lines, fails if none
  printf '%s\n' "$1" | grep -E ${3:+"$3"} -- "$2"
}

# A file, comment lines removed, as "<file>:<line>:<text>".
uncommented() { # <file>
  awk -v f="$1" '!/^[[:space:]]*#/ { print f ":" NR ":" $0 }' "$1"
}

files=()
if [ -d "$dir" ]; then
  for f in "$dir"/*.yml "$dir"/*.yaml; do [ -f "$f" ] && files+=("$f"); done
fi
if [ -d "$actions" ]; then
  while IFS= read -r f; do files+=("$f"); done \
    < <(find "$actions" -type f \( -name 'action.yml' -o -name 'action.yaml' \) | sort)
fi
[ "${#files[@]}" -gt 0 ] || { echo "check-release-workflow: no workflow or action; nothing to check"; exit 0; }

for wf in "${files[@]}"; do
  body="$(uncommented "$wf")"
  if hits="$(hits_of "$body" '(^|[^A-Za-z0-9_])(pull_request_target|workflow_run)([^A-Za-z0-9_]|$)')"; then
    fail "forbidden trigger (08, ADR-0005):"$'\n'"$hits"
  fi
  if hits="$(hits_of "$body" 'ACTIONS_(STEP|RUNNER)_DEBUG')"; then
    fail "debug logging is forbidden (ADR-0005):"$'\n'"$hits"
  fi
  if hits="$(hits_of "$body" 'secrets[[:space:]]*\[|tojson\([[:space:]]*secrets' -i)"; then
    fail "an expression that exposes every secret:"$'\n'"$hits"
  fi
  if [ "$wf" != "$release" ]; then
    if hits="$(hits_of "$body" 'release-signing')"; then
      fail "only release.yml may name the release-signing environment (08):"$'\n'"$hits"
    fi
    if hits="$(hits_of "$body" 'secrets\.MPB_')"; then
      fail "only release.yml may read a secrets.MPB_* value (ADR-0005):"$'\n'"$hits"
    fi
  fi
done

if [ -f "$release" ]; then
  body="$(uncommented "$release")"

  # 5. The triggers, exactly.
  on_block="$(awk '/^[[:space:]]*#/ { next } /^[^[:space:]]/ { inon = ($0 ~ /^on:[[:space:]]*$/); next } inon && NF { print }' "$release")"
  [ "$(grep -c '^on:[[:space:]]*$' "$release" || true)" = 1 ] || fail "release.yml must have exactly one top-level 'on:' mapping"
  on_keys="$(printf '%s\n' "$on_block" | sed -n -E 's/^  ([A-Za-z_]+):.*/\1/p' | sort | tr '\n' ' ')"
  [ "$on_keys" = "push workflow_dispatch " ] || fail "release.yml's triggers are '$on_keys'; they must be exactly push and workflow_dispatch"
  push_block="$(printf '%s\n' "$on_block" | awk '/^  [A-Za-z_]/ { inpush = ($0 ~ /^  push:[[:space:]]*$/); next } inpush { print }')"
  if ! printf '%s\n' "$push_block" | grep -Eqx '    tags:[[:space:]]*\[[[:space:]]*"v\*"[[:space:]]*\][[:space:]]*' \
     || [ "$(printf '%s\n' "$push_block" | grep -c .)" != 1 ]; then
    fail "release.yml's push trigger must be exactly: tags: [\"v*\"]"
  fi

  # 6. One environment line. Any line that could select the environment -
  #    `environment: release-signing`, or `name: release-signing` under an
  #    environment mapping - counts; prose in an error message does not.
  mentions="$(printf '%s\n' "$body" | grep -cE '(environment|name):.*release-signing' || true)"
  envline="$(printf '%s\n' "$body" | grep -E '^[^:]*:[0-9]+:[[:space:]]*environment:[[:space:]]*release-signing[[:space:]]*$' || true)"
  if [ "$mentions" != 1 ] || [ -z "$envline" ]; then
    fail "release.yml must select release-signing exactly once, as 'environment: release-signing' (found $mentions)"
  else
    # 7, 8. The signing job: from its two-space job key to the next one.
    envno="$(printf '%s' "$envline" | sed -n -E 's/^[^:]*:([0-9]+):.*/\1/p')"
    job="$(awk -v n="$envno" '
      { line[NR] = $0 }
      /^jobs:[[:space:]]*$/ { injobs = 1; next }
      injobs && /^[^[:space:]#]/ { injobs = 0; if (start && !stop) stop = NR }
      injobs && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ {
        if (NR <= n) start = NR; else if (start && !stop) stop = NR
      }
      END {
        if (!start) exit
        if (!stop) stop = NR + 1
        for (i = start; i < stop; i++) print i ":" line[i]
      }' "$release")"
    if [ -z "$job" ]; then
      fail "could not find the job that holds environment: release-signing"
    else
      jobtext="$(printf '%s\n' "$job" | grep -v '^[0-9]*:[[:space:]]*#' || true)"
      if hits="$(hits_of "$jobtext" '(^|[^A-Za-z0-9_-])(cargo|rustup|rust-toolchain|validate\.sh|run-fixture-matrix)([^A-Za-z0-9_-]|$)')"; then
        fail "the signing job compiles or runs the project (plan, D-W2):"$'\n'"$hits"
      fi
      first="$(printf '%s\n' "$job" | head -1 | cut -d: -f1)"
      last="$(printf '%s\n' "$job" | tail -1 | cut -d: -f1)"
      if hits="$(printf '%s\n' "$body" | awk -F: -v a="$first" -v b="$last" '/secrets\./ && ($2 < a || $2 > b)')" && [ -n "$hits" ]; then
        fail "secrets. outside the signing job:"$'\n'"$hits"
      fi
    fi
  fi

  # 9. No xtrace, in any spelling.
  xre='(^|[^A-Za-z0-9_])set([[:space:]]+[-+][A-Za-z]+)*[[:space:]]+[-+][A-Za-z]*x|xtrace|SHELLOPTS|BASH_XTRACEFD|bash[[:space:]]+-[A-Za-z]*x|shell:.*[[:space:]]-[A-Za-z]*x'
  if hits="$(hits_of "$body" "$xre")"; then
    fail "xtrace in the release workflow (ADR-0005: it exposes argv):"$'\n'"$hits"
  fi

  # 10. Nothing builds with a feature. (a) No cargo at all in the file's code:
  #     comment lines are not code, and the release is built by build-release.sh.
  if hits="$(hits_of "$body" '(^|[^A-Za-z0-9_-])cargo([^A-Za-z0-9_-]|$)')"; then
    fail "release.yml names cargo; a release is built only through scripts/build-release.sh (pass 08 slice 4 review L1):"$'\n'"$hits"
  fi
  #     (b) No long feature flag anywhere, in any quoting or in a variable.
  if hits="$(hits_of "$body" '(^|[^A-Za-z0-9_-])--(all-)?features([^A-Za-z0-9_-]|$)')"; then
    fail "a feature flag in the release workflow; test-support must not be in a release (pass 08 slice 4 review L1):"$'\n'"$hits"
  fi
  #     (c) No -F on a line that calls the release build. Continuations are
  #     joined first, so a flag on the next line of a command is on its line.
  joined="$(awk '
    /^[[:space:]]*#/ { next }
    { sub(/[[:space:]]+$/, "") }
    /\\$/ { sub(/\\$/, ""); buf = buf $0 " "; next }
    { print buf $0; buf = "" }
    END { if (buf != "") print buf }' "$release")"
  if hits="$(printf '%s\n' "$joined" | grep -E 'build-release\.sh' | grep -E "(^|[[:space:]\"'])-[A-Za-z]*F")"; then
    fail "a -F flag on a line that calls the release build (pass 08 slice 4 review L1):"$'\n'"$hits"
  fi
fi

if [ "$failures" -gt 0 ]; then
  echo "check-release-workflow: $failures rule(s) broken" >&2
  exit 1
fi
echo "check-release-workflow: release workflow rules hold"
