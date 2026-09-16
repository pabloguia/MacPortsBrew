#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Every relative markdown link, and every anchor, resolves.
#
# `08-distribution-and-project.md:183-184` is a MUST: wiki source is
# link-checked. Nothing checked it. A pass that renamed one wiki page and its
# six inbound links validated that by reading carefully, which worked, and then
# in the same change introduced three anchors pointing at a bold paragraph
# rather than a heading - which reading carefully did not catch. That is the
# whole argument for a gate: the diligent path and the wrong path look identical
# from the inside.
#
# Scope: relative links between markdown files in this repository, and `#`
# fragments within them. External URLs are not fetched - a gate that needs the
# network is a gate that fails on a train.
#
# It also counts, without failing on, the links in `wiki/` that point outside
# it. Those resolve here and break on publication, because a GitHub wiki is a
# separate repository; rewriting them needs an absolute URL and the repository
# has no remote yet (DP-007). Failing would block work that is correct today, so
# the number is reported instead - and a number nobody prints is a number that
# grows.
set -euo pipefail
cd "$(dirname "$0")/.."

python3 - "$@" <<'PY'
import glob, os, re, sys

def slug(heading: str) -> str:
    """GitHub's heading slug: strip formatting and punctuation, spaces to hyphens."""
    text = re.sub(r"`|\*|_", "", heading.strip())
    text = re.sub(r"[^\w\s-]", "", text).strip().lower()
    return re.sub(r"\s+", "-", text)

files = sorted(
    set(glob.glob("wiki/*.md"))
    | set(glob.glob("docs/**/*.md", recursive=True))
    | {"README.md", "CONTRIBUTING.md", "CHANGELOG.md", "AGENTS.md", "CLAUDE.md"}
)
files = [f for f in files if os.path.exists(f)]

def headings_of(path):
    """Heading slugs, in order, skipping fenced code.

    Fence-awareness is not decoration. `05-environment-and-primary-provider.md`
    documents a managed block whose markers are shell comments -
    `# >>> MacPortsBrew managed environment >>>` and its closing twin - and a
    scan that only tests `startswith("#")` reads those as two headings with one
    slug. The duplicate check below called that a collision on its first run.
    The extra anchors were harmless while this map was only ever asked "does
    this fragment exist"; they stop being harmless the moment anything counts.
    """
    out, fenced = [], False
    for line in open(path):
        if line.lstrip().startswith(("```", "~~~")):
            fenced = not fenced
            continue
        if fenced or not line.startswith("#"):
            continue
        text = line.lstrip("#")
        # A heading has a space after its hashes; `#!/usr/bin/env` and `#include`
        # do not.
        if text.strip() and text[:1] in (" ", "\t"):
            out.append(slug(text))
    return out


# Collected as a list first, because two headings that slug the same are the
# defect below and a set would hide it.
heading_list = {os.path.normpath(f): headings_of(f) for f in files}
headings = {f: set(hs) for f, hs in heading_list.items()}

# Two headings with the same slug give one anchor, and a link to it resolves to
# whichever the renderer reaches first - silently, and differently between
# renderers. This checker resolved anchors and never counted them, so a report
# carried `## Recommended next tasks` twice (live and archived) and
# `decisions-pending.md` carried `### DP-015` twice, each for months. A link to
# the archive got the live list instead, or the reverse (F-46).
duplicates = []
for f, hs in sorted(heading_list.items()):
    seen = set()
    for h in hs:
        if h in seen:
            duplicates.append("%s: two headings share the anchor #%s" % (f, h))
        seen.add(h)

broken = list(duplicates)
for f in files:
    base = os.path.dirname(f)
    body = open(f).read()
    for line_no, line in enumerate(body.splitlines(), 1):
        for _text, target in re.findall(r"\[([^\]]*)\]\(([^)\s]+)\)", line):
            if target.startswith(("http://", "https://", "mailto:")):
                continue
            path, _, fragment = target.partition("#")
            resolved = os.path.normpath(os.path.join(base, path)) if path else os.path.normpath(f)
            if path and not os.path.exists(resolved):
                broken.append(f"{f}:{line_no}: no such file: {target}")
                continue
            if fragment and resolved in headings and fragment not in headings[resolved]:
                broken.append(f"{f}:{line_no}: no such heading: {target}")

# Links that leave `wiki/`. They work in this repository and will not work in a
# published wiki, which is a separate repository (F-20).
escaping = []
for f in files:
    if not f.startswith("wiki/"):
        continue
    for line_no, line in enumerate(open(f).read().splitlines(), 1):
        for _text, target in re.findall(r"\[([^\]]*)\]\(([^)\s]+)\)", line):
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            path = target.partition("#")[0]
            if path and not os.path.normpath(os.path.join("wiki", path)).startswith("wiki/"):
                escaping.append(f"{f}:{line_no}: {target}")

if broken:
    print("links: %d broken reference(s):" % len(broken), file=sys.stderr)
    for b in broken:
        print("  " + b, file=sys.stderr)
    raise SystemExit(1)
note = ""
if escaping:
    note = (f"; {len(escaping)} wiki link(s) point outside wiki/ and will break "
            f"when the wiki is published (F-20, DP-007)")
print(f"links: {len(files)} files, every relative link and anchor resolves{note}")
PY
