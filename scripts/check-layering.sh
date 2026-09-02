#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Layering check (docs/spec/07-quality-and-testing.md): `domain` and `policy`
# depend on no adapter, and shell execution exists only in the central
# process executor (04-migration-and-safety.md, "Process execution").
#
# The crate graph is the first line of defence - a layer is a crate, so an
# illegal import does not compile without a Cargo.toml edit. This script is
# the second: it reads the real graph from `cargo metadata` and asserts the
# allowed edges, and it asserts textually that std::process::Command is
# constructed in exactly one module.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo metadata --format-version 1 --no-deps --locked | python3 -c '
import json, sys
meta = json.load(sys.stdin)
pkgs = {p["name"]: p for p in meta["packages"]}
# name -> set of workspace crates it may depend on
allowed = {
    "macportsbrew-domain":      set(),
    "macportsbrew-policy":      {"macportsbrew-domain"},
    "macportsbrew-application": {"macportsbrew-domain", "macportsbrew-policy"},
    "macportsbrew-adapters":    {"macportsbrew-domain"},
    "macportsbrew":             {"macportsbrew-domain", "macportsbrew-policy",
                                 "macportsbrew-application", "macportsbrew-adapters"},
}
workspace = set(pkgs)
bad = 0
for name, p in pkgs.items():
    deps = {d["name"] for d in p["dependencies"] if d["name"] in workspace}
    if name not in allowed:
        print(f"layering: unknown workspace crate {name}; add it to scripts/check-layering.sh")
        bad += 1
        continue
    illegal = deps - allowed[name]
    for d in sorted(illegal):
        print(f"layering: {name} depends on {d}, which the layering forbids")
        bad += 1
sys.exit(1 if bad else 0)
'

# std::process::Command may be constructed only in the executor module.
#
# One exception, and it is narrow: a crate's `tests/` directory. The rule this
# enforces is about the *product* - 04-migration-and-safety.md places every
# provider invocation behind one executor so that the environment, the argv and
# the timeout are decided in exactly one place. An end-to-end test spawns the
# built binary itself, which 07-quality-and-testing.md requires ("End-to-end
# tests - the CLI over fixture machines, asserting on --json") and which no
# executor can do, because the executor is inside the thing being spawned.
# The exception is `crates/<crate>/tests/` only: `src/` stays guarded in full,
# unit tests included, so a `#[cfg(test)]` module still cannot reach for a
# subprocess instead of the ReplayRunner.
executor='crates/adapters/src/process/'
hits="$(grep -RIn --include='*.rs' -E '(std::process::Command|process::Command::new|Command::new\()' crates 2>/dev/null \
  | grep -v "^${executor}" \
  | grep -vE '^crates/[^/]+/tests/' \
  | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|///|//!)' || true)"
if [ -n "$hits" ]; then
  echo "layering: std::process::Command constructed outside ${executor}:" >&2
  printf '%s\n' "$hits" >&2
  exit 1
fi

# No provider name in a domain signature (02-domain-model.md, "Shape").
# Identifiers only: comments and doc strings may explain what Homebrew does.
hits="$(grep -RIn --include='*.rs' -iE '\b(fn|struct|enum|trait|type|mod)[[:space:]]+[A-Za-z_]*(homebrew|macports|brew)[A-Za-z_]*' crates/domain/src crates/policy/src 2>/dev/null || true)"
if [ -n "$hits" ]; then
  echo "layering: a provider name appears in a domain or policy identifier:" >&2
  printf '%s\n' "$hits" >&2
  exit 1
fi

echo "layering: ok"
