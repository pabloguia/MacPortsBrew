#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# What this gate enforces, all four of them:
#
#   1. Every relative markdown link resolves to a file that exists **in this
#      repository**, spelled as the filesystem spells it. A target that is
#      absolute or climbs out with `../` is refused from the text alone rather
#      than tested against the host, and a target that resolves only because
#      this filesystem is case-insensitive is refused too - both used to pass,
#      and both are broken for every reader of the rendered page (F-47).
#   2. Every `#` fragment resolves to a heading in the file it names, where a
#      heading inside a block quote is a heading: GFM renders `> # Foo` as one
#      and github.com anchors it identically (F-47).
#   3. No file has two headings that slug to one anchor (F-46), and no file
#      leaves a top-level code fence open at end of file (F-47) - the second
#      because the heading scan cannot tell code from prose past an
#      unterminated fence, so an answer from there is not an answer.
#   4. Nothing here consults the machine to decide what a link *means*. Where a
#      target points is decided lexically; the filesystem is asked only whether
#      what it names is there, and how it is spelled.
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
    def unquote(line):
        """A line with its block quote markers removed, and how many there were.

        `> # Foo` is an ATX heading inside a block quote: GFM 0.29-gfm's own
        Example 206 renders it `<blockquote><h1>Foo</h1>`, and github.com gives
        it the same permalink as a top-level heading - `href="#foo"`, the bare
        slug, observed on a rendered README 2026-09-20. A scan testing the raw
        line saw none of them, so eight headings in `docs/research/` were
        anchors this gate did not know existed: harmless for links, silence in
        the duplicate check, which is the direction that ships a collision.

        CommonMark's marker is `>` with a following space **or** `>` alone, and
        it nests, so both forms are consumed until the line stops being quoted.
        """
        depth = 0
        while True:
            quoted = re.match(r" {0,3}> ?", line)
            if not quoted:
                return depth, line
            depth += 1
            line = line[quoted.end():]

    out, fence = [], None  # fence: (character, length, quote depth) while open
    opened_at = 0
    for line_no, line in enumerate(open(path), 1):
        depth, content = unquote(line)
        # A fence is tracked at the quote depth it was opened at, for two
        # reasons that pull in opposite directions.
        #
        # Stripping the marker and keeping one fence for the whole file would be
        # worse than not stripping at all: a ``` line *inside* a plain fence
        # would close it, and this corpus is markdown about markdown. There are
        # six fences inside block quotes in `docs/research/` and, the other way,
        # a quoted marker under an unquoted fence is ordinary quoted output.
        #
        # But matching the depth *symmetrically* was also wrong, and shipped
        # here for exactly as long as it took a reviewer to implement it rather
        # than read it. CommonMark closes a fence when its containing block
        # ends, so an ordinary document - a quoted fence, then a blank line -
        # has a fence no depth-matched closer ever reaches. The end-of-file
        # error then fires on valid markdown and takes every heading after it.
        # Hence: a drop in quote depth closes the fence.
        if fence is not None and depth < fence[2]:
            fence = None
        stripped = content.lstrip()
        marker = stripped[:1]
        if marker in ("`", "~"):
            indent = len(content) - len(stripped)
            run = len(stripped) - len(stripped.lstrip(marker))
            info = stripped[run:].strip()
            if fence is None:
                if indent <= 3 and run >= 3 and marker not in info:
                    fence, opened_at = (marker, run, depth), line_no
                    continue
            elif ((marker, depth) == (fence[0], fence[2])
                  and run >= fence[1] and indent <= 3 and not info):
                fence = None
                continue
            # Anything else on a marker line is content. Falling through here
            # rather than toggling is the whole of the fix.
        if fence is not None or not content.startswith("#"):
            continue
        text = content.lstrip("#")
        # A heading has a space after its hashes; `#!/usr/bin/env` and `#include`
        # do not.
        if text.strip() and text[:1] in (" ", "\t"):
            out.append(slug(text))
    # Only a fence still open at depth 0 is an error. One opened inside a block
    # quote is closed by end of file the same way it is closed by the block
    # quote ending, so calling it unterminated would reject a valid document -
    # the failure the depth-drop rule above exists to prevent, arriving one line
    # later.
    if fence is not None and fence[2] == 0:
        return out, (
            "%s:%d: unterminated %s fence; past this line the heading scan "
            "cannot tell code from prose, so its answer is not an answer"
            % (path, opened_at, fence[0] * fence[1])
        )
    return out, None


def link_target(base, path):
    """Where a relative link points **inside this repository**.

    Returns `(resolved, refusal)`, exactly one of which is None.

    What this replaces answered about the *host*. `os.path.join` discards
    everything to its left when its right operand is absolute, so a target of
    `/etc/hosts` resolved to `/etc/hosts`, which exists on every machine this
    gate runs on: the link passed here and 404s on GitHub, where a leading `/`
    is site-root-relative and names no file in the repository. A target climbing
    out with `../` was resolved above the repository root and tested there. Both
    verdicts were facts about the machine, so CI and a laptop could disagree
    about one document - the property `--replay` exists to deny the readers
    (INV-15), in a gate.

    Decided from the text alone: this function never asks the filesystem
    anything. The empty path stays with the caller on purpose - a bare
    `#fragment` link has no path, and resolving it here would produce a
    directory, which is not a key in the heading map, so every in-page anchor in
    the repository would stop being checked with nothing said.
    """
    if os.path.isabs(path):
        return None, "absolute link target names no file in this repository"
    resolved = os.path.normpath(os.path.join(base, path))
    if resolved == ".." or resolved.startswith(".." + os.sep):
        return None, "link target climbs out of the repository"
    return resolved, None


_entries = {}


def spelled_as_on_disk(path):
    """True when every component of `path` matches its directory entry exactly.

    `os.path.exists` is case-insensitive on the default macOS filesystem, and
    github.com is not. Without this, `readme.MD` resolves here and 404s for
    every reader - and because the wrongly-cased name is not a key in the
    heading map either, the fragment check is skipped too, so a link with a
    wrong-case path *and* a nonexistent anchor passes clean. Same class as the
    hole above: a verdict that is a fact about the machine.
    """
    here = ""
    for part in path.split(os.sep):
        if part in ("", "."):
            continue
        listing = _entries.get(here)
        if listing is None:
            try:
                listing = set(os.listdir(here or "."))
            except OSError:
                return False
            _entries[here] = listing
        if part not in listing:
            return False
        here = os.path.join(here, part)
    return True


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
            if path:
                resolved, refusal = link_target(base, path)
                if refusal:
                    broken.append(f"{f}:{line_no}: {refusal}: {target}")
                    continue
                if not os.path.exists(resolved):
                    broken.append(f"{f}:{line_no}: no such file: {target}")
                    continue
                if not spelled_as_on_disk(resolved):
                    broken.append(
                        f"{f}:{line_no}: resolves only on a case-insensitive "
                        f"filesystem, so it is broken for every reader: {target}"
                    )
                    continue
            else:
                # A bare `#fragment`: the target is this file. Deliberately not
                # routed through `link_target` - see its docstring.
                resolved = os.path.normpath(f)
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
            if not path:
                continue
            # The same resolution as the loop above, through the same function.
            # Spelling `os.path.join` a second time here is how one file comes
            # to hold two rules: this line had no absolute-path guard, so
            # `join("wiki", "/x")` discarded the base exactly as the other site
            # did. A refusal is already a failure there; here it only has to not
            # be counted as a wiki escape.
            resolved, refusal = link_target("wiki", path)
            if refusal is None and not resolved.startswith("wiki" + os.sep):
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
