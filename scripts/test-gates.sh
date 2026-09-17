#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# The gates' own tests.
#
#   scripts/test-gates.sh
#
# A gate is only ever exercised against a tree it passes, which tells you it
# said yes and nothing about whether it can say no. Both gates tested here were
# written, shipped, and found broken afterwards by someone constructing an input
# they should have rejected - F-46's duplicate-anchor check by F-47, and
# `expect_json`'s treatment of exit 4 by DP-016(c). So each case below is a
# constructed input with a required verdict, and half of them require a
# rejection.
#
# The functions are LIFTED FROM THE GATE, not copied into this file. A copy
# would pass forever while the gate drifted away from it, which is the same
# two-spellings defect `validate.sh`'s header records about the stage list.
# Extraction is anchored on text that must exist; if an anchor disappears this
# fails rather than silently testing nothing.
#
# What it does not cover: `check-links.sh`'s link resolution and
# `run-fixture-matrix.sh`'s golden comparison, both of which need a tree and a
# built binary. This covers the two parsers - the parts that answer a question
# about text and can be wrong quietly.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

failures=0
checks=0
note() { printf '  %-5s %-52s %s\n' "$1" "$2" "${3:-}"; }
miss() { failures=$((failures + 1)); }
pass() { checks=$((checks + 1)); }

echo "== check-links.sh: headings_of, fence tracking (F-47)"
if python3 - <<'PY'; then pass; else miss; fi
import sys

src = open("scripts/check-links.sh").read()

# The two `def`s and nothing between them. A single span from `def slug(` to the
# duplicate-check comment also swallows the top-level `files = sorted(glob(...))`
# that sits between them, which would then run here - inside a `--quick` stage,
# at every turn end and in CI - and anything later added in that gap would run
# too, with nothing in a `check-links.sh` diff to suggest it. Anchors are
# checked rather than assumed: if one moves, this fails loudly instead of
# testing nothing.
ANCHORS = [("def slug(", "files = sorted("), ("def headings_of(", "# Collected as a list first")]
body = ""
for start, end in ANCHORS:
    if start not in src or end not in src:
        print(f"  ERROR anchor moved in check-links.sh: {start!r} .. {end!r}", file=sys.stderr)
        raise SystemExit(1)
    body += src[src.index(start):src.index(end)] + "\n"
ns = {}
exec(compile("import re\n" + body, "<lifted from check-links.sh>", "exec"), ns)
headings_of = ns["headings_of"]

B = "`" * 3
T = "~" * 3

# (name, document, expected slugs, expects an unterminated-fence error)
CASES = [
    ("plain headings",
     "# One\n\n## Two\n", ["one", "two"], False),
    ("heading inside a backtick fence is code",
     f"# One\n\n{B}\n# Not A Heading\n{B}\n\n## Two\n", ["one", "two"], False),
    ("heading inside a tilde fence is code",
     f"# One\n\n{T}\n# Not A Heading\n{T}\n\n## Two\n", ["one", "two"], False),
    ("info string on the opener",
     f"# One\n\n{B}python\n# comment\n{B}\n\n## Two\n", ["one", "two"], False),
    # The five that desynchronised a single boolean.
    ("backtick line INSIDE a tilde block",
     f"# One\n\n{T}\n{B}\n# Not A Heading\n{T}\n\n## Two\n", ["one", "two"], False),
    ("```python line INSIDE a backtick block",
     f"# One\n\n{B}\n{B}python\n# Not A Heading\n{B}\n\n## Two\n", ["one", "two"], False),
    ("marker indented four columns is content",
     f"# One\n\n    {B}\n    code\n\n## Two\n", ["one", "two"], False),
    ("inline span of three backticks is not an opener",
     f"# One\n\n{B}code{B} is inline\n\n## Two\n", ["one", "two"], False),
    ("unterminated fence at end of file",
     f"# One\n\n{B}\ncode forever\n", ["one"], True),
    # Length and character rules.
    ("a longer opener is not closed by a shorter run",
     "# One\n\n````\n```\nstill code\n````\n\n## Two\n", ["one", "two"], False),
    ("a shorter opener IS closed by a longer run",
     "# One\n\n```\ncode\n`````\n\n## Two\n", ["one", "two"], False),
    ("a tilde fence is not closed by backticks",
     f"# One\n\n{T}\n{B}\n# Not A Heading\n{T}\n\n## Two\n", ["one", "two"], False),
    ("a close carries no info string",
     f"# One\n\n{B}\n{B} trailing\n# Not A Heading\n{B}\n\n## Two\n", ["one", "two"], False),
    ("managed-block shell comments are not headings",
     f"# One\n\n{B}sh\n# >>> MacPortsBrew managed environment >>>\n"
     f"# <<< MacPortsBrew managed environment <<<\n{B}\n\n## Two\n",
     ["one", "two"], False),
]

# A private directory, not a fixed name in a world-writable one. `/tmp` is
# sticky but that only stops deleting someone else's file, not creating a name
# nobody has taken; `open(path, "w")` follows a symlink planted there and
# truncates its target, once per case, as whoever runs `validate.sh`. Wiring
# this into `--quick` made it run at every turn end and in CI with a predictable
# name, which is the difference between a latent hole and an open one. Every
# other temporary path in `scripts/` and `.claude/hooks/` already uses `mktemp`.
import tempfile, os
workdir = tempfile.mkdtemp(prefix="mpb-gate-")

bad = 0
for name, doc, want_slugs, want_error in CASES:
    path = os.path.join(workdir, "case.md")
    open(path, "w").write(doc)
    slugs, err = headings_of(path)
    ok = slugs == want_slugs and bool(err) == want_error
    verdict = "OK" if ok else "MISS"
    detail = "" if ok else f"got {slugs} err={bool(err)}, wanted {want_slugs} err={want_error}"
    print(f"  {verdict:<5} {name:<52} {detail}")
    if not ok:
        bad = 1
import shutil
shutil.rmtree(workdir, ignore_errors=True)
print(f"  {len(CASES)} constructed documents, {sum(1 for c in CASES if c[3])} requiring a rejection")
raise SystemExit(bad)
PY

echo
echo "== run-fixture-matrix.sh: expect_plan, exit 4 is two outcomes (DP-016)"
if ! grep -q '^expect_plan() {' scripts/run-fixture-matrix.sh; then
  echo "  ERROR expect_plan not found in run-fixture-matrix.sh" >&2
  miss
else
  # The gate's own function, with its two reporters stubbed so a verdict can be
  # read off. `failures` is what `fail` increments inside the lifted code.
  eval "$(awk '/^expect_plan\(\) \{/,/^\}/' scripts/run-fixture-matrix.sh)"
  T="$(mktemp -d)"
  trap 'rm -rf "$T"' EXIT

  NO_STOP='{"schemaVersion":1,"steps":[{"kind":"ADD_DESTINATION"}]}'
  STOP='{"schemaVersion":1,"steps":[{"kind":"ADD_DESTINATION"},{"kind":"MANUAL_REVIEW"}]}'

  case_is() { # <want: accept|reject> <name> <stdout> <stderr> <rc> [plan|refusal]
    local want="$1" name="$2"
    printf '%s' "$3" >"$T/out"
    printf '%s' "$4" >"$T/err"
    local saved=$failures
    failures=0
    LAST=""
    # The fifth argument is what the fixture owes. These cases exercise the
    # shape checks, so each declares the shape it is feeding in; the two cases
    # that cross the declaration are the ones asserting it bites.
    expect_plan "$name" "$T/out" "$T/err" "$5" "${6:-plan}"
    local got=accept
    [ "$failures" -ne 0 ] && got=reject
    failures=$saved
    if [ "$got" = "$want" ]; then
      note OK "$name" "$LAST"
      pass
    else
      note MISS "$name" "wanted $want, got $got"
      miss
    fi
  }
  # `fail` and `ok` are the gate's own reporters, stubbed here so a verdict can
  # be read off. They are called from the lifted `expect_plan`, which reaches
  # this shell through `eval` - so shellcheck cannot see the call sites and
  # reports them unused. The directive is on the line above each function
  # deliberately: a misplaced one left this repository's CI first step exiting 1
  # unnoticed for weeks (F-44).
  # shellcheck disable=SC2329
  fail() { LAST="$*"; failures=$((failures + 1)); }
  # shellcheck disable=SC2329
  ok() { :; }

  case_is accept "exit 0, plan, no stop"                "$NO_STOP" ""      0
  case_is accept "exit 4, plan that stops"              "$STOP"    ""      4
  case_is accept "exit 4, refusal (DP-016 shape)"       ""         "why"   4 refusal
  # The per-fixture declaration, in both directions. Without it a broadened
  # refusal turns every fixture into a refusal and the matrix stays green.
  case_is reject "a refusal where a plan was owed"      ""         "why"   4 plan
  case_is reject "a plan where a refusal was owed"      "$STOP"    ""      4 refusal
  case_is reject "exit 4, plan with NO stop"            "$NO_STOP" ""      4
  case_is reject "exit 0, plan WITH a stop"             "$STOP"    ""      0
  case_is reject "exit 4, stdout present but not JSON"  "garbage"  "x"     4
  case_is reject "exit 0, nothing written"              ""         ""      0 refusal
  case_is reject "exit 4, refusal that says nothing"    ""         ""      4 refusal
  case_is reject "an exit code that means neither"      "$STOP"    ""      7
  case_is reject "plan written, stderr not empty"       "$NO_STOP" "leak"  0
  case_is reject "schemaVersion is not 1"               '{"schemaVersion":2,"steps":[]}' "" 0
fi

echo
if [ "$failures" -eq 0 ]; then
  echo "test-gates: every constructed case produced the required verdict"
  exit 0
fi
echo "test-gates: $failures case(s) did not" >&2
exit 1
