#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# What this gate enforces, all three of them:
#
#   1. Every relative markdown link resolves to a file that exists.
#   2. Every `#` fragment resolves to a heading in the file it names.
#   3. No file has two headings that slug to one anchor (F-46), and no file
#      leaves a code fence open at end of file (F-47) - the second because the
#      heading scan cannot tell code from prose past an unterminated fence, so
#      an answer from there is not an answer.
#
# This list was one behaviour out of date for the whole of F-46's life: the
# duplicate check existed only in an inline comment. The rule this repository
# already applies to `validate.sh`'s stage list applies here - the header is
# where a reader learns what a gate does, so it is the copy that must not drift.
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

    Returns `(slugs, unterminated)`, where `unterminated` is a message or None.

    A single boolean that either marker flips was wrong in five ways, and the
    corpus this scans is markdown about markdown, so each is an ordinary future
    edit rather than an adversarial one (F-47). The opener is therefore tracked
    as its *character and length*, and closed only by the same character, at
    least as long, with an empty info string:

      - a ``` line inside a ~~~ block is content, not a close;
      - a ```python line inside a ``` block is content - a close carries no
        info string;
      - a marker indented four or more columns is an indented code block,
        where the marker is content;
      - a line opening with an inline span of three or more backticks is not
        an opener, because its info string contains the fence character.

    The fifth is why the return type changed. **An unterminated fence at end of
    file is an error**, which is stricter than CommonMark - it deliberately
    turns every desynchronisation above from "silently stopped reading" into a
    visible failure, the same rule the adapters follow where a parser meeting
    unrecognised output fails rather than mis-parses. It matters because the two
    consumers fail in opposite directions and one is silent: a dropped heading
    makes a *link* fail loudly and safely, while it makes the *duplicate* check
    pass quietly, shipping the collision the check exists to catch.

    Rejecting an info string containing the fence character is CommonMark's
    rule for backticks; applying it to tildes too is stricter than the spec.
    That is the safe direction only because of the check above - a tilde opener
    wrongly refused leaves the scan inverted, and it is the end-of-file error
    that then reports it rather than a silent wrong answer.
    """
    out, fence = [], None  # fence: (character, length) while one is open
    opened_at = 0
    for line_no, line in enumerate(open(path), 1):
        stripped = line.lstrip()
        marker = stripped[:1]
        if marker in ("`", "~"):
            indent = len(line) - len(stripped)
            run = len(stripped) - len(stripped.lstrip(marker))
            info = stripped[run:].strip()
            if fence is None:
                if indent <= 3 and run >= 3 and marker not in info:
                    fence, opened_at = (marker, run), line_no
                    continue
            elif marker == fence[0] and run >= fence[1] and indent <= 3 and not info:
                fence = None
                continue
            # Anything else on a marker line is content. Falling through here
            # rather than toggling is the whole of the fix.
        if fence is not None or not line.startswith("#"):
            continue
        text = line.lstrip("#")
        # A heading has a space after its hashes; `#!/usr/bin/env` and `#include`
        # do not.
        if text.strip() and text[:1] in (" ", "\t"):
            out.append(slug(text))
    if fence is not None:
        return out, (
            "%s:%d: unterminated %s fence; past this line the heading scan "
            "cannot tell code from prose, so its answer is not an answer"
            % (path, opened_at, fence[0] * fence[1])
        )
    return out, None


# Collected as a list first, because two headings that slug the same are the
# defect below and a set would hide it.
scanned = {os.path.normpath(f): headings_of(f) for f in files}
heading_list = {f: hs for f, (hs, _) in scanned.items()}
headings = {f: set(hs) for f, hs in heading_list.items()}

# An unterminated fence is a failure rather than a warning: it does not make
# this file's answer approximate, it makes it unusable, and the duplicate check
# below would go quiet rather than wrong (F-47).
unterminated = [msg for _, (_, msg) in sorted(scanned.items()) if msg]

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

broken = unterminated + duplicates
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
    print("links: %d problem(s):" % len(broken), file=sys.stderr)
    for b in broken:
        print("  " + b, file=sys.stderr)
    raise SystemExit(1)
note = ""
if escaping:
    note = (f"; {len(escaping)} wiki link(s) point outside wiki/ and will break "
            f"when the wiki is published (F-20, DP-007)")
print(f"links: {len(files)} files, every relative link and anchor resolves{note}")
PY
