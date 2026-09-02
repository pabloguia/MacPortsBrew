#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Dependency gate, three axes (docs/spec/07-quality-and-testing.md):
#   1. licence - every crate in the graph is under an allow-listed licence
#      (08-distribution-and-project.md); GPLv2-only in particular is a build
#      failure. Anything outside the list needs an ADR before it is added.
#   2. build-script and unsafe surface - a crate with a build.rs is accepted
#      only when it is named in the allow-list below, each with the ADR that
#      reviewed it. `unsafe_code = "forbid"` covers the workspace's own code;
#      dependencies' unsafe surface is reviewed in the same ADR.
#   3. deployment floor - checked on the built artifact by
#      check-deployment-target.sh, not here.
# The same licence policy is in deny.toml for cargo-deny in CI; this script
# needs only `cargo metadata` so it runs anywhere the toolchain does.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo metadata --format-version 1 --locked | python3 -c '
import json, re, sys
meta = json.load(sys.stdin)
workspace = set(meta["workspace_members"])
allowed = {"Apache-2.0", "MIT", "BSD-2-Clause", "BSD-3-Clause", "ISC",
           "Unlicense", "Zlib", "MPL-2.0",
           # ADR-0004: Unicode-3.0 (unicode-ident), permissive, reviewed.
           "Unicode-3.0"}
# Crates whose build.rs was reviewed; ADR-0004 holds the review.
build_scripts_allowed = set()
bad = 0
for p in meta["packages"]:
    if p["id"] in workspace:
        continue
    lic = p.get("license") or ""
    if not lic:
        print("licence: %s %s declares no licence" % (p["name"], p["version"]))
        bad += 1
        continue
    # SPDX expression: accept if every OR-alternative group has one allowed
    # licence, i.e. treat "A OR B" as satisfiable and "A AND B" as both needed.
    def ok(expr):
        expr = expr.replace("/", " OR ")
        return any(all(t.strip(" ()") in allowed for t in alt.split(" AND "))
                   for alt in expr.split(" OR "))
    if not ok(lic):
        print("licence: %s %s is under \"%s\", outside the allow-list (08-distribution-and-project.md)" % (p["name"], p["version"], lic))
        bad += 1
    has_build = any(t["kind"] == ["custom-build"] for t in p["targets"])
    if has_build and p["name"] not in build_scripts_allowed:
        print("licence: %s %s carries a build.rs that no ADR has reviewed (07-quality-and-testing.md, dependency gate)" % (p["name"], p["version"]))
        bad += 1
if bad:
    sys.exit(1)
print("licence: ok")
'
