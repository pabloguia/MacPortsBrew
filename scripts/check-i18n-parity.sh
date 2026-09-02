#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# i18n key parity (docs/spec/06-cli-and-ux.md): every catalogue under
# locales/ has exactly the keys of the English source catalogue, and every
# message identifier the code references exists in English. Fails on a
# missing or an orphaned key. The pseudolocale is generated at runtime from
# English and has no file.

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
            used |= set(re.findall(r'\bmsg!\(\s*"([A-Za-z0-9_.-]+)"', src))
            used |= set(re.findall(r'MessageId::new\(\s*"([A-Za-z0-9_.-]+)"', src))
for u in sorted(used - en):
    print(f"i18n: code references message {u} which locales/en.toml lacks"); bad += 1
if bad: sys.exit(1)
print(f"i18n: ok ({len(en)} keys, {len(used)} referenced from code)")
PY
