#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Keeps target/ from growing until the machine slows to a halt (SE-100).
# Housekeeping, not a check: it never fails validation, and validate.sh runs
# it last, after every build a mode makes and after the verdict is decided.
#
# Why it exists (2026-10-17): Cargo never removes an artifact when a crate
# rebuilds under a new hash, and under macOS's default `split-debuginfo =
# "unpacked"` each build also left one object file per codegen unit. A night
# of rebuilds left 605,518 entries in target/debug/deps, and launching a test
# binary from that directory blocked in exec for 20-100 s - something on the
# exec path walks the executable's directory - so validation took 25 minutes.
# Cargo.toml now sets `packed`, which stops the object files; stale binaries,
# their .dSYM bundles and libraries still accumulate, about 180 entries per
# full rebuild against 626 after a clean one.
#
#   scripts/prune-target.sh          what validate.sh runs: safe alongside
#                                    another build in the same checkout
#   scripts/prune-target.sh --clean  also `cargo clean` when over either limit;
#                                    run by hand, never from a gate
#
# Every `deps` directory under ./target (the host profile and any `--target`
# triple) gets:
#   1. Object files older than 30 minutes deleted. Under `packed` rustc writes
#      them during a build and removes them once linked, so a fresh one may be
#      another build's work in flight; an old one is from an `unpacked` build
#      and is debug info for a binary a rebuild has already replaced.
#   2. Its entries counted, against MPB_TARGET_PRUNE_LIMIT (default 1000; a
#      full packed build leaves about 626, and each rebuild adds about 180).
# Then target/ as a whole is measured against MPB_TARGET_SIZE_LIMIT_MB
# (default 5120 MiB, 5 GiB). incremental/ caches and stale deps binaries grow
# without bound: on 2026-10-25 target/ held 7.6 GiB, 3.3 GiB of it incremental/,
# with debug/deps at 932 entries, far under the entry limit of the time.
# Over either limit the default mode says so and how to clean; it does not
# clean, because a gate has a deadline and may share the checkout with another
# build (security review of 2026-10-17, M1 and M2). `--clean` runs
# `cargo clean --target-dir target`: the directory this script measured,
# whatever CARGO_TARGET_DIR says.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || { echo "prune-target: skipped (cannot enter the repository)"; exit 0; }

clean=0
case "${1:-}" in
  --clean) clean=1 ;;
  '') ;;
  *) echo "prune-target: skipped (unknown argument; use --clean or nothing)"; exit 0 ;;
esac

# Ten digits or more is refused: past the shell's integer range `[ -gt ]`
# errors out and the comparison would silently be false.
limit="${MPB_TARGET_PRUNE_LIMIT:-1000}"
case "$limit" in
  ''|*[!0-9]*|??????????*) echo "prune-target: skipped (MPB_TARGET_PRUNE_LIMIT is not a number of at most 9 digits)"; exit 0 ;;
esac

size_limit_mb="${MPB_TARGET_SIZE_LIMIT_MB:-5120}"
case "$size_limit_mb" in
  ''|*[!0-9]*|??????????*) echo "prune-target: skipped (MPB_TARGET_SIZE_LIMIT_MB is not a number of at most 9 digits)"; exit 0 ;;
esac

if [ ! -d target ] || [ -L target ]; then
  echo "prune-target: skipped (no target/ directory of its own)"; exit 0
fi

over=""
while IFS= read -r -d '' deps; do
  removed=$(find "$deps" -maxdepth 1 -type f -name '*.o' -mmin +30 -print -delete | wc -l | tr -d ' ')
  entries=$(find "$deps" -mindepth 1 -maxdepth 1 | wc -l | tr -d ' ')
  echo "prune-target: $deps: $entries entries, $removed stale object files removed"
  if [ "$entries" -gt "$limit" ]; then
    over="$deps"
  fi
done < <(find target -mindepth 2 -maxdepth 3 -type d -name deps -print0)

reason=""
if [ -n "$over" ]; then
  reason="$over is over $limit entries, and test binaries launched from it slow down as it grows"
fi

# -x keeps du on target/'s own volume; -k makes the unit the same on BSD and GNU du.
size_kb=$(du -skx target 2>/dev/null | cut -f1)
case "$size_kb" in
  ''|*[!0-9]*) echo "prune-target: size of target/ not measured (du failed)" ;;
  *)
    size_mb=$((size_kb / 1024))
    echo "prune-target: target: ${size_mb} MiB (limit ${size_limit_mb} MiB)"
    if [ "$size_mb" -gt "$size_limit_mb" ]; then
      reason="${reason:+$reason; }target/ is ${size_mb} MiB, over its ${size_limit_mb} MiB limit"
    fi
    ;;
esac

if [ -n "$reason" ]; then
  if [ "$clean" = 1 ]; then
    echo "prune-target: $reason; running cargo clean (the next build is cold)"
    cargo clean --target-dir target || echo "prune-target: cargo clean failed; target/ is left as it was"
  else
    echo "prune-target: WARNING $reason."
    echo "prune-target: when no build is running, run: scripts/prune-target.sh --clean"
  fi
fi
exit 0
