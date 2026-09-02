#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Coverage floors per layer (docs/spec/07-quality-and-testing.md):
#   domain, policy      95% lines
#   application         95% lines (planning, verification, safety logic)
#   adapters            80% lines
#   cli (ui)            reported, no floor
# Branch coverage: cargo-llvm-cov's --branch needs a nightly toolchain, and
# the pinned toolchain is stable (ADR-0002), so branch floors are not
# enforced yet; this is recorded as a limitation in the pass report.
# Needs cargo-llvm-cov, present in CI. In an agent session the tool cannot
# be added to the developer's toolchain, so locally this reports "not
# available" instead of a number.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

if ! cargo llvm-cov --version >/dev/null 2>&1; then
  echo "coverage: cargo-llvm-cov is not available; floors not checked here (CI checks them)" >&2
  exit 2
fi

cargo llvm-cov --workspace --locked --all-features --json --output-path target/llvm-cov.json >/dev/null
python3 - <<'PY'
import json, sys
data = json.load(open("target/llvm-cov.json"))
floors = {"domain": 95.0, "policy": 95.0, "application": 95.0, "adapters": 80.0, "cli": None}
totals = {k: [0, 0] for k in floors}
for f in data["data"][0]["files"]:
    name = f["filename"]
    for layer in floors:
        if f"/crates/{layer}/src/" in name:
            s = f["summary"]["lines"]
            totals[layer][0] += s["covered"]; totals[layer][1] += s["count"]
bad = 0
for layer, (cov, cnt) in totals.items():
    pct = 100.0 * cov / cnt if cnt else 0.0
    floor = floors[layer]
    verdict = "reported" if floor is None else ("ok" if pct >= floor else "BELOW FLOOR")
    print(f"coverage: {layer:12s} {pct:6.2f}% lines ({cov}/{cnt})  floor {floor if floor is not None else '-'}  {verdict}")
    if floor is not None and pct < floor: bad = 1
sys.exit(bad)
PY
