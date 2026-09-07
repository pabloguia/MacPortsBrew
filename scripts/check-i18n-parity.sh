#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# i18n key parity (docs/spec/06-cli-and-ux.md), in all three directions:
#
#   1. every catalogue under locales/ has exactly the keys of the English
#      source catalogue - no missing key, no extra one;
#   2. every message identifier the code references exists in English;
#   3. every English key is referenced from code, or says in a comment above
#      itself why it is not.
#
# The third was missing, and this header used to claim it: "fails on an
# orphaned key" meant a key a *translation* carries and English does not. A key
# English carries that nothing renders was invisible - which is how four
# deliberate `support.*` labels sat unused while the column printed a
# lower-cased Rust identifier (F-20).
#
# Deleting a deliberate string is the owner's call and not a gate's, so an
# unreferenced key is allowed when the line above it begins `# unreferenced:`
# and says why. A stale marker fails too: a key that *is* referenced must not
# claim otherwise.
#
# The pseudolocale is generated at runtime from English and has no file.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

[ -f locales/en.toml ] || { echo "i18n: locales/en.toml not found"; exit 1; }

python3 - <<'PY'
import re, sys, glob, os
def keys(path):
    ks = set()
    for line in open(path, encoding="utf-8"):
        m = re.match(r'^\s*"?([A-Za-z0-9_.-]+)"?\s*=', line)
        if m: ks.add(m.group(1))
    return ks
def production(src):
    """Everything above the trailing `#[cfg(test)] mod tests`.

    Splitting on the first `#[cfg(test)]` is wrong: in
    `adapters/src/process/mod.rs` that attribute sits on a `const` three
    hundred lines above the test module, and everything below it - production
    code in the only subprocess site in the repository - was being discarded.
    Only the attribute that opens a test *module* ends the file.

    Doc comments go too, and that is the second half of the same rule. A `///`
    line is documentation, not a render site: `cli.about` satisfied direction
    three for months on an `assert!` inside a `///` example and on prose that
    merely named the key, while nothing in the program rendered it. A gate that
    counts a comment as a call site cannot see the drift it exists to catch
    (F-32). Stripping whole lines is deliberately crude and correct here - a
    `///` never carries code the program runs.
    """
    m = re.search(r"^#\[cfg\(test\)\]\s*\nmod tests\b", src, re.M)
    src = src[: m.start()] if m else src
    return "\n".join(
        l for l in src.splitlines() if not l.lstrip().startswith(("///", "//!"))
    )
def excused(path):
    """Keys whose comment block explains why nothing renders them.

    The marker applies to the key it sits *immediately* above, allowing
    continuation lines. Anything else - a blank line, a section header, an
    unrelated comment - clears it. Without that, a marker at the end of one
    section excused the first key of the next, straight across a `# ---` header.
    """
    out, marked = set(), False
    for line in open(path, encoding="utf-8"):
        m = re.match(r'^\s*"?([A-Za-z0-9_.-]+)"?\s*=', line)
        if m:
            if marked: out.add(m.group(1))
            marked = False
        elif line.startswith("# unreferenced:"):
            marked = True
        elif not (marked and line.startswith("#")):
            marked = False
    return out
en = keys("locales/en.toml")
bad = 0
for f in sorted(glob.glob("locales/*.toml")):
    if f.endswith("/en.toml"): continue
    k = keys(f)
    for m in sorted(en - k): print(f"i18n: {f} is missing key {m}"); bad += 1
    for o in sorted(k - en): print(f"i18n: {f} has orphaned key {o}"); bad += 1
# Message ids referenced from code: msg!("id") / Message::new("id") / "msg.id" literals in ui.
used = set()
for root, _, files in os.walk("crates"):
    for fn in files:
        if fn.endswith(".rs"):
            src = open(os.path.join(root, fn), encoding="utf-8").read()
            if root.endswith("/tests") or "/tests/" in root or fn.endswith("_test.rs"):
                continue
            src = production(src)
            used |= set(re.findall(r'\bmsg!\(\s*"([A-Za-z0-9_.-]+)"', src))
            used |= set(re.findall(r'MessageId::new\(\s*"([A-Za-z0-9_.-]+)"', src))
            used |= set(re.findall(r'render_id\(\s*"([A-Za-z0-9_.-]+)"', src))
            used |= set(re.findall(r'\.render\(\s*"([A-Za-z0-9_.-]+)"', src))
            used |= set(re.findall(r'label\(ctx,\s*"([A-Za-z0-9_.-]+)"', src))
            used |= set(re.findall(r'Reason::new\(\s*"([A-Za-z0-9_.-]+)"', src))
for u in sorted(used - en):
    print(f"i18n: code references message {u} which locales/en.toml lacks"); bad += 1
# Direction three. `used` above collects the six call shapes; a message id is
# also written as a plain literal in a `match` arm - `ErrorId::message_id` is
# thirteen of them - so any literal shaped like an identifier counts as a use.
# The same character set `keys()` accepts, or a key it allows and this rejects
# would be a permanent false orphan.
ID = re.compile(r'"([A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+)"')
literals = set()
for root, _, files in os.walk("crates"):
    for fn in files:
        if not fn.endswith(".rs"): continue
        if root.endswith("/tests") or "/tests/" in root or fn.endswith("_test.rs"): continue
        literals |= set(ID.findall(production(
            open(os.path.join(root, fn), encoding="utf-8").read())))
excuses = excused("locales/en.toml")
for o in sorted(en - literals - excuses):
    print(f"i18n: locales/en.toml has {o} and nothing renders it; wire it, delete it, "
          f"or write `# unreferenced: <why>` above it"); bad += 1
for stale in sorted(excuses & literals):
    print(f"i18n: {stale} is marked unreferenced and is referenced"); bad += 1
if bad: sys.exit(1)
print(f"i18n: ok ({len(en)} keys, {len(literals & en)} rendered, {len(excuses)} unreferenced with a stated reason)")
PY
