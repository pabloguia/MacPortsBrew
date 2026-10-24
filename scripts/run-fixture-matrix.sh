#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Runs the whole read slice over every recorded fixture and compares the
# analysis against a committed golden file.
#
#   scripts/run-fixture-matrix.sh [--update]
#
# This is the mechanical form of two things docs/spec/07-quality-and-testing.md
# asks for: the fixture matrix (every fixture is exercised by every read
# command, with its exit code asserted) and the golden/reproducibility tests
# (INV-15: the same snapshot, metadata and policy version produce byte-identical
# output). `--update` regenerates fixtures/golden/; without it, a difference is
# a failure.
#
# What it writes: fixtures/golden/ (only with --update) and one mktemp -d
# directory, which is removed on exit. MACPORTSBREW_STATE_DIR points into that
# directory so no command can reach the developer's real state directory.
#
# Needs jq and a built binary (cargo build).
#
# THE ONE NORMALISATION, AND WHY IT IS THERE
# ------------------------------------------
# `--replay` re-captures a snapshot on every run, and a capture stamps itself
# with the wall clock (crates/adapters/src/persistence/time.rs is the only
# clock read in the workspace). So `analyze --replay` differs between two runs
# in exactly one field, `.snapshotProvenance.capturedAt`, and in no other.
# Comparing raw bytes would therefore fail on the clock rather than on a
# behaviour change. Both the golden comparison and the snapshot round-trip
# below replace that one field with a fixed placeholder, by name, with jq — a
# whole-document normaliser would be able to hide a real difference, and this
# cannot. Everything else is compared byte for byte.
# INV-15 itself is asserted unnormalised, on the `--from-snapshot` path it
# names: see `reproducible` below.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

# The release workflow runs the matrix against the release executable. An
# override is never silent: a shell that exported it once would otherwise have
# the matrix pass against some other binary (pass 15 review A1; validate.sh
# also unsets it).
BIN="${MPB_FIXTURE_BIN:-target/debug/macportsbrew}"
if [ -n "${MPB_FIXTURE_BIN:-}" ]; then
  echo "run-fixture-matrix: testing $BIN (MPB_FIXTURE_BIN), not the debug build" >&2
fi
GOLDEN="fixtures/golden"
TODAY="2026-09-02"
update=0
[ "${1:-}" = "--update" ] && update=1

command -v jq >/dev/null || { echo "run-fixture-matrix: jq is required" >&2; exit 2; }
[ -x "$BIN" ] || { echo "run-fixture-matrix: $BIN is not built (cargo build)" >&2; exit 2; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export MACPORTSBREW_STATE_DIR="$tmp/state"

failures=0
checks=0
fail() { echo "  FAIL: $*" >&2; failures=$((failures + 1)); }
ok() { checks=$((checks + 1)); }

# Runs the binary, captures stdout, stderr and the exit code separately.
run() { # <stdout-file> <args...>
  local out="$1"; shift
  set +e
  "$BIN" "$@" >"$out" 2>"$tmp/stderr"
  local rc=$?
  set -e
  return $rc
}

# Asserts the exit code, that stdout parses as JSON, and that it carries the
# schema version the CLI contract promises.
#
# For `plan`, use `expect_plan` instead: two of its three outcomes share exit
# code 4 and only one of them writes a document.
expect_json() { # <label> <expected-codes-space-separated> <out-file> <rc>
  local label="$1" want="$2" out="$3" rc="$4"
  case " $want " in
    *" $rc "*) ok ;;
    *) fail "$label: exit $rc, expected one of [$want]"; return ;;
  esac
  if ! jq -e . "$out" >/dev/null 2>&1; then
    fail "$label: stdout is not JSON"
    return
  fi
  ok
  if [ "$(jq -r '.schemaVersion // empty' "$out")" != "1" ]; then
    fail "$label: schemaVersion is not 1"
    return
  fi
  ok
}

# `plan` settles in one of three states, and the exit code does not separate
# them because two of them are 4:
#
#   exit 0, document on stdout   a plan with no manual-review stop
#   exit 4, document on stdout   a plan that stops at manual review
#   exit 4, stdout empty         a refusal: no plan was built at all
#
# The first two are `Ok` from `run_inner`. The third is an `Err`, and `run`
# (crates/cli/src/app/mod.rs) renders every `Err` to *stderr* as text - there
# is no JSON error envelope anywhere in this program, under `--json` or not.
# So "exit 4" alone cannot tell a built plan from a refused one, and
# `expect_json` reported the refusal DP-016(c) added as "stdout is not JSON":
# a true sentence about the wrong thing, on a run that behaved correctly.
#
# What separates them here is not a tolerance but the planner's own rule,
# `Ok(if plan.requires_manual_review() { 4 } else { 0 })`, where
# `requires_manual_review` is "any step is `MANUAL_REVIEW`". The exit code and
# the steps array are therefore one fact stated twice, and this asserts they
# agree *in both directions* - 4 without a stop, and 0 with one, are both
# failures.
#
# Not "strictly more than the old check", which this comment claimed until a
# reviewer measured it. `expect_json "0 4"` required *every* plan invocation to
# emit a parseable document, and simply allowing an empty stdout at exit 4 drops
# that. It is restored by the caller naming which outcome each fixture owes, so
# a document is still required everywhere one is possible; the assertions above
# are what is added on top. A non-empty stdout that does not parse stays a
# failure either way.
expect_plan() { # <label> <out-file> <err-file> <rc> <want: plan|refusal>
  local label="$1" out="$2" err="$3" rc="$4" want="$5" stops
  case "$rc" in
    0 | 4) ok ;;
    *)
      fail "$label: exit $rc, expected 0 (plan), or 4 (plan stopping at manual review, or refusal)"
      return
      ;;
  esac

  # Which of the two the *caller* says this fixture must produce. Without it
  # `expect_plan` accepted any outcome from any fixture, and the old
  # `expect_json "0 4"` had asserted something this did not: that every plan
  # invocation emits a parseable document. A refusal broadened by a regression -
  # keyed on the destination instead of the source, say - would then turn every
  # fixture into a refusal and leave the matrix green.
  if [ "$want" = "refusal" ] && [ -s "$out" ]; then
    fail "$label: a plan was built, and this fixture cannot answer the service question so it must be refused"
    return
  fi
  if [ "$want" = "plan" ] && [ ! -s "$out" ]; then
    fail "$label: refused, and this fixture has the facts to be planned"
    return
  fi
  ok

  if [ ! -s "$out" ]; then
    # No document: a refusal. Only exit 4 may produce one - an exit 0 that
    # wrote nothing is a command that silently did nothing.
    if [ "$rc" != "4" ]; then
      fail "$label: exit $rc wrote no plan; only a refusal (exit 4) may write none"
      return
    fi
    ok
    # A refusal that says nothing leaves the user with an exit code and no
    # reason, which is the failure mode 06-cli-and-ux.md exists to prevent.
    if [ ! -s "$err" ]; then
      fail "$label: refused with exit 4 and wrote nothing to stderr"
      return
    fi
    ok
    return
  fi

  # A document: it must be JSON, carry the schema version, and its stops must
  # agree with the exit code.
  if ! jq -e . "$out" >/dev/null 2>&1; then
    fail "$label: stdout is not JSON"
    return
  fi
  ok
  if [ "$(jq -r '.schemaVersion // empty' "$out")" != "1" ]; then
    fail "$label: schemaVersion is not 1"
    return
  fi
  ok
  stops="$(jq '[.steps[]? | select(.kind == "MANUAL_REVIEW")] | length' "$out")"
  if [ "$rc" = "4" ] && [ "$stops" = "0" ]; then
    fail "$label: exit 4 on a plan with no MANUAL_REVIEW step"
    return
  fi
  if [ "$rc" = "0" ] && [ "$stops" != "0" ]; then
    fail "$label: exit 0 on a plan with $stops MANUAL_REVIEW step(s)"
    return
  fi
  ok
  # A built plan goes to stdout, so nothing is left to say on stderr.
  if [ -s "$err" ]; then
    fail "$label: a plan was written and stderr is not empty: $(head -c 200 "$err")"
    return
  fi
  ok
}

# Replaces the single wall-clock field, by name. Fails loudly if the document
# is not an object, rather than emitting something that would compare equal.
normalise() { # <in> <out>
  jq -S 'if type != "object" then error("not an object") else
           if has("snapshotProvenance") and (.snapshotProvenance | type) == "object"
           then .snapshotProvenance.capturedAt = "<captured-at>" else . end
         end' "$1" >"$2"
}

# For the snapshot round trip: drop both fields the prompt allows to differ,
# so what remains is the analysis itself.
strip_source() { # <in> <out>
  jq -S 'del(.snapshotProvenance) | .source = "<source>"' "$1" >"$2"
}

mkdir -p "$GOLDEN"

for fixture in fixtures/recordings/*.json; do
  name="$(basename "$fixture" .json)"
  echo "== $name"

  # doctor reports an ERROR finding, and so exit 5, on a fixture whose
  # provider is present but unhealthy. Every other fixture must be 0.
  doctor_codes="0"
  case "$name" in *broken*) doctor_codes="0 5" ;; esac

  for cmd in status analyze list; do
    run "$tmp/$cmd.json" --replay "$fixture" --json "$cmd" --today "$TODAY" && rc=0 || rc=$?
    expect_json "$cmd" "0" "$tmp/$cmd.json" "$rc"
    if [ -s "$tmp/stderr" ]; then
      fail "$cmd: stderr is not empty under --json: $(head -c 200 "$tmp/stderr")"
    else
      ok
    fi
  done

  run "$tmp/doctor.json" --replay "$fixture" --json doctor --today "$TODAY" && rc=0 || rc=$?
  expect_json "doctor" "$doctor_codes" "$tmp/doctor.json" "$rc"

  # explain and plan need a package; the empty fixtures legitimately have none.
  pkg="$(jq -r '[.packages[] | select(.advancedOnly == false)][0].name // empty' "$tmp/analyze.json")"
  if [ -n "$pkg" ]; then
    src="$(jq -r --arg p "$pkg" '[.packages[] | select(.name == $p)][0].source' "$tmp/analyze.json")"
    dest="macports"; [ "$src" = "macports" ] && dest="homebrew"

    run "$tmp/explain.json" --replay "$fixture" --json explain "$pkg" --today "$TODAY" && rc=0 || rc=$?
    expect_json "explain $pkg" "0" "$tmp/explain.json" "$rc"

    # Which outcome this fixture owes, derived from the source rather than from
    # the fixture's name so it stays true as fixtures are added. A MacPorts
    # source cannot answer whether its packages define services under `--replay`
    # - the scan will not read a filesystem that is not the recorded machine's
    # (F-25, F-29) - so DP-016(c) refuses rather than let INV-19's rule be
    # silently unevaluated. Every other source has the fact and gets a plan.
    want="plan"
    [ "$src" = "macports" ] && want="refusal"

    # Three correct outcomes, two of which are exit 4; `expect_plan` tells them
    # apart and holds the fixture to the one it owes.
    run "$tmp/plan.json" --replay "$fixture" --json plan "$pkg" --to "$dest" --today "$TODAY" && rc=0 || rc=$?
    expect_plan "plan $pkg -> $dest" "$tmp/plan.json" "$tmp/stderr" "$rc" "$want"
  else
    echo "  (no package unit: explain and plan skipped)"
  fi

  # The golden analysis.
  golden="$GOLDEN/$name.analyze.json"
  normalise "$tmp/analyze.json" "$tmp/analyze.norm.json"
  if [ "$update" = "1" ]; then
    cp "$tmp/analyze.norm.json" "$golden"
    echo "  updated $golden"
  elif [ ! -f "$golden" ]; then
    fail "golden: $golden is missing (run with --update)"
  elif cmp -s "$tmp/analyze.norm.json" "$golden"; then
    ok
  else
    fail "golden: $golden differs from this run"
    diff <(head -c 4000 "$golden") <(head -c 4000 "$tmp/analyze.norm.json") | head -20 >&2 || true
  fi

  # The snapshot round trip: capture through the same replay, then analyse the
  # file. The analysis must be the same one, modulo the two fields that say
  # where it came from.
  run "$tmp/snap.stdout" --replay "$fixture" snapshot --out "$tmp/snapshot.json" --today "$TODAY" && rc=0 || rc=$?
  if [ "$rc" != "0" ] || [ ! -s "$tmp/snapshot.json" ]; then
    fail "snapshot: exit $rc, no file written"
  else
    ok
    run "$tmp/from-snapshot.json" --from-snapshot "$tmp/snapshot.json" --json analyze --today "$TODAY" && rc=0 || rc=$?
    expect_json "analyze --from-snapshot" "0" "$tmp/from-snapshot.json" "$rc"
    strip_source "$tmp/analyze.json" "$tmp/replay.stripped.json"
    strip_source "$tmp/from-snapshot.json" "$tmp/snapshot.stripped.json"
    if cmp -s "$tmp/replay.stripped.json" "$tmp/snapshot.stripped.json"; then
      ok
    else
      fail "snapshot round trip differs from the replay analysis"
      diff "$tmp/replay.stripped.json" "$tmp/snapshot.stripped.json" | head -20 >&2 || true
    fi

    # INV-15, unnormalised: the same snapshot file analysed twice is byte
    # for byte the same document.
    run "$tmp/repro-1.json" --from-snapshot "$tmp/snapshot.json" --json analyze --today "$TODAY" && rc=0 || rc=$?
    run "$tmp/repro-2.json" --from-snapshot "$tmp/snapshot.json" --json analyze --today "$TODAY" && rc=0 || rc=$?
    if cmp -s "$tmp/repro-1.json" "$tmp/repro-2.json"; then ok; else
      fail "INV-15: two --from-snapshot runs are not byte-identical"
    fi
  fi
done

# Nothing may have leaked into a state directory: every command above is a
# read command (INV-16), and the two that write were given an explicit path.
if [ -d "$MACPORTSBREW_STATE_DIR" ] && [ -n "$(find "$MACPORTSBREW_STATE_DIR" -type f -print -quit)" ]; then
  fail "a read command wrote into MACPORTSBREW_STATE_DIR"
else
  ok
fi

echo
if [ "$failures" -eq 0 ]; then
  echo "run-fixture-matrix: $checks checks passed over $(find fixtures/recordings -name '*.json' | wc -l | tr -d ' ') fixtures"
  exit 0
fi
echo "run-fixture-matrix: $failures failed, $checks passed" >&2
exit 1
