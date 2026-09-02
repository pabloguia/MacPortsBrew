#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Deployment-target assertion on the built artifacts (ADR-0002;
# docs/spec/07-quality-and-testing.md). The configuration says 10.12 / 11.0;
# the linker is what decides, and it can raise the floor silently. So this
# reads what the binaries actually recorded:
#   x86_64  -> LC_VERSION_MIN_MACOSX version 10.12 (ld64 emits the older load
#              command for targets below 10.14), or LC_BUILD_VERSION minos.
#   aarch64 -> LC_BUILD_VERSION minos 11.0.
# It also scans every object inside every rlib the release build produced,
# so a transitive dependency compiled for a newer floor is caught even when
# the final link happens to record the lower value, and it fails on any
# linker warning about an object built for a newer macOS.
# Verified on 2026-09-02 with rustc 1.98.0: `rustc --print deployment-target`
# reports 10.12 / 11.0 by default, an explicit 10.11 is clamped to 10.12 with
# a warning, and an explicit 10.13 is recorded silently - the last is the
# failure this script exists to catch.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

if ! command -v otool >/dev/null 2>&1; then
  echo "deployment-target: otool not found; this check needs macOS Command Line Tools" >&2
  exit 2
fi

declare -A floor=([x86_64-apple-darwin]=10.12 [aarch64-apple-darwin]=11.0)
bad=0

recorded_min() {
  # Prints the minimum OS version recorded in a Mach-O file, whichever load
  # command carries it.
  otool -l "$1" 2>/dev/null | awk '
    /cmd LC_VERSION_MIN_MACOSX/ { want=1 }
    /cmd LC_BUILD_VERSION/      { want=1 }
    want && /^ *version /       { print $2; exit }
    want && /^ *minos /         { print $2; exit }
  '
}

vernum() { # 10.12 -> 10012, 11.0 -> 11000
  awk -F. '{ printf "%d%03d\n", $1, ($2 == "" ? 0 : $2) }' <<<"$1"
}

for target in "${!floor[@]}"; do
  echo "deployment-target: building --release --target $target"
  # Capture the linker's own warnings; "built for newer" is a failure.
  if ! out="$(cargo build --release --locked --target "$target" 2>&1)"; then
    printf '%s\n' "$out" | tail -20 >&2
    exit 1
  fi
  if printf '%s\n' "$out" | grep -qiE 'built for newer|was built for|deployment target'; then
    echo "deployment-target: the linker or rustc warned about the deployment target for $target:" >&2
    printf '%s\n' "$out" | grep -iE 'built for newer|was built for|deployment target' >&2
    bad=1
  fi
  want="${floor[$target]}"
  for bin in target/"$target"/release/macportsbrew target/"$target"/release/mpb; do
    got="$(recorded_min "$bin")"
    if [ -z "$got" ]; then
      echo "deployment-target: $bin records no minimum OS version" >&2; bad=1; continue
    fi
    if [ "$(vernum "$got")" -gt "$(vernum "$want")" ]; then
      echo "deployment-target: $bin records minimum macOS $got, above the floor $want (ADR-0002)" >&2; bad=1
    else
      echo "deployment-target: $bin records $got (floor $want) - ok"
    fi
  done
  # Every object in every rlib: a dependency compiled for a newer floor.
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/mpb-deploy.XXXXXX")"
  for rlib in target/"$target"/release/deps/*.rlib; do
    [ -f "$rlib" ] || continue
    while IFS= read -r obj; do
      ar -p "$rlib" "$obj" > "$tmp/obj.o" 2>/dev/null || continue
      got="$(recorded_min "$tmp/obj.o")"
      [ -n "$got" ] || continue
      if [ "$(vernum "$got")" -gt "$(vernum "$want")" ]; then
        echo "deployment-target: $(basename "$rlib") object $obj records $got, above the floor $want" >&2; bad=1
      fi
    done < <(ar -t "$rlib" 2>/dev/null | grep '\.o$' || true)
  done
  rm -rf "$tmp"
done

[ "$bad" = 0 ] && echo "deployment-target: ok"
exit "$bad"
