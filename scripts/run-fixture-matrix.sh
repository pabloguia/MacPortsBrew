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

BIN="target/debug/macportsbrew"
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

    # 4 is the safety-check code: a plan that stops at manual review
    # (06-cli-and-ux.md). Both answers are correct; anything else is not.
    run "$tmp/plan.json" --replay "$fixture" --json plan "$pkg" --to "$dest" --today "$TODAY" && rc=0 || rc=$?
    expect_json "plan $pkg -> $dest" "0 4" "$tmp/plan.json" "$rc"
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
