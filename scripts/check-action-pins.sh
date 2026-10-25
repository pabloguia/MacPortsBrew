#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Every `uses:` in a GitHub Actions workflow names a 40-hex commit SHA, never a
# tag and never a branch (docs/spec/08-distribution-and-project.md; SE-055).
#
# A tag and a branch are both mutable: whoever controls the action's repository
# can change what a pinned name resolves to, and the change lands in the next CI
# run with no diff in this repository. That matters here because the validation
# workflow is what asserts the deployment floor on the built artifacts, so the
# job's integrity is load-bearing for a correctness claim, not only for secrets.
#
# The readable version belongs in a trailing comment, which this check requires
# so that a SHA is never the only thing a reader has to go on:
#
#     uses: actions/checkout@d23441a4...af803 # v6
#
# Local actions (`uses: ./...`) and container actions (`uses: docker://...`) are
# not pinned by SHA and are skipped.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# Every YAML under .github, not a glob list: workflows, and composite actions,
# which carry `uses:` steps of their own and would otherwise be an unchecked way
# back in. Walking the tree rather than asking git also means a source export
# with no `.git` is scanned rather than silently skipped.

# The actions this project intends to run. A SHA alone says nothing about *what*
# is pinned: GitHub forks share an object store, so a commit pushed to any fork
# of `actions/checkout` is fetchable under that fork's name and executes, while
# the trailing comment still reads `# v6`. This does not bind a SHA to a version
# — that needs a checked-in registry re-verified against the API, which is a
# larger piece of work and is recorded as such — but it does stop an unfamiliar
# repository arriving unremarked.
ALLOWED_REPOS='
actions/attest-build-provenance
actions/checkout
actions/download-artifact
actions/upload-artifact
anthropics/claude-code-action
dtolnay/rust-toolchain
EmbarkStudios/cargo-deny-action
taiki-e/install-action
'

bad=0
found=0

while IFS= read -r file; do
  [ -f "$file" ] || continue
  while IFS= read -r line; do
    # `uses:` value, with any leading dash and indentation stripped.
    ref="${line#*uses}"
    ref="${ref#"${ref%%[![:space:]]*}"}"
    ref="${ref#:}"
    ref="${ref#"${ref%%[![:space:]]*}"}"
    case "$ref" in
      ./*|.\ *|docker://*) continue ;;
    esac
    found=$((found + 1))
    # Everything before a trailing comment is the ref itself.
    pin="${ref%%#*}"
    pin="${pin%"${pin##*[![:space:]]}"}"
    after="${pin##*@}"
    if [ "$pin" = "$after" ]; then
      echo "action-pins: no @ref at all: $file: $pin"
      bad=1
      continue
    fi
    if ! printf '%s' "$after" | grep -Eq '^[0-9a-f]{40}$'; then
      echo "action-pins: not a 40-hex commit SHA: $file: $pin"
      bad=1
      continue
    fi
    repo="${pin%@*}"
    if ! printf '%s' "$ALLOWED_REPOS" | grep -qx -- "$repo"; then
      echo "action-pins: not an allow-listed action repository: $file: $repo"
      echo "action-pins:   add it to ALLOWED_REPOS in $0 if it is intended"
      bad=1
      continue
    fi
    case "$ref" in
      *"#"*) ;;
      *)
        echo "action-pins: pinned but unreadable, add a '# <version>' comment: $file: $pin"
        bad=1
        ;;
    esac
    # Whitespace before the colon is valid YAML and GitHub honours it, so the
    # match must tolerate it or the check waves the spelling through in silence.
  done < <(grep -hE '(^|[[:space:]{,])uses[[:space:]]*:' "$file" || true)
done < <(find .github -type f \( -name '*.yml' -o -name '*.yaml' \) 2>/dev/null | sort)

# Fail closed. Finding nothing is not the same as finding nothing wrong: a
# source export with no `.git`, a moved directory or a broken glob would all
# have printed `ok (0 pinned)` and exited 0, and this gate now runs on every
# turn end, so a silent skip would stay invisible for a long time.
if [ "$found" -eq 0 ]; then
  echo "action-pins: no 'uses:' found under .github - refusing to pass on an empty scan"
  bad=1
fi

# The pinned toolchain is written in two kinds of file, and one of them is
# under a step named "pinned by rust-toolchain.toml" - which is exactly the
# shape of duplication 11-agentic-sdlc.md warns about: "a rule that exists in
# both places will drift". Bump the file and CI silently tests a different
# compiler than the pin ADR-0002 says holds the macOS floor.
pinned="$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml | head -1)"
if [ -z "$pinned" ]; then
  echo "action-pins: no channel found in rust-toolchain.toml" >&2
  bad=1
else
  while IFS= read -r line; do
    [ -z "$line" ] && continue
    file="${line%%:*}"
    value="$(printf '%s' "$line" | sed -E 's/.*toolchain:[[:space:]]*//; s/[[:space:]]*$//')"
    if [ "$value" != "$pinned" ]; then
      echo "action-pins: $file pins toolchain '$value' but rust-toolchain.toml says '$pinned'." >&2
      echo "  ADR-0002 makes the toolchain the mechanism that holds the macOS floor;" >&2
      echo "  two spellings of it is one spelling that will be wrong." >&2
      bad=1
    fi
  done < <(grep -rn '^[[:space:]]*toolchain:' .github/workflows/ 2>/dev/null || true)
fi

if [ "$bad" = 0 ]; then
  echo "action-pins: ok ($found pinned, toolchain $pinned)"
fi
exit "$bad"
