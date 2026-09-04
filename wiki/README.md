# `wiki/` — the user documentation

These pages are the user-facing documentation. They live in the repository so
they are versioned, reviewable, agent-editable and link-checked like everything
else, rather than drifting in a separate place nobody diffs.

- **`Home.md` is the landing page.** That name is fixed by GitHub.
- **`_Sidebar.md` is the navigation.** Every page it names must exist; the
  repository's link check fails otherwise.
- **`docs/` stays canonical for the specification, the ADRs, the research notes
  and the schemas.** A wiki page explains, links and shows; it does not become a
  second source of truth for a fact `docs/` already owns. Where a page needs a
  provider fact, it cites the research note and repeats the source and the read
  date rather than restating the conclusion from memory.

## Rules for a page here

1. **Write for a human first.** Titles say what the page contains, in the words
   a person would search for. Clear beats clever, and there is no keyword
   stuffing.
2. **Every claim about Homebrew or MacPorts carries its source and the date it
   was read**, or is phrased conditionally. Time-sensitive claims — the Intel
   timeline above all — are stated with their date, so a reader in 2028 knows
   what they are looking at.
3. **Both providers are peers.** A page about one direction has a counterpart
   about the other, and neither is written as the default.
4. **Do not describe behaviour this build does not have.** Where something is
   planned, the page says so and says which pass it belongs to.

## Publishing

**Nothing publishes these pages automatically, and no automation ships.** This
is a deliberate gap, not an oversight.

A GitHub wiki is itself a Git repository, cloned as
`https://github.com/OWNER/REPO.wiki.git`, and only pushes to its default branch
go live. Publishing is therefore mechanically simple: check out the main
repository, check out the wiki repository, copy this directory over, commit,
push. What makes it unresolved is the credential — **there is no `wiki`
permission scope for `GITHUB_TOKEN`**, and whether `contents: write` suffices
was not established, so any workflow would need a personal access token or a
deploy key. That is a secret in CI, which is a decision for the repository
owner and not something a pass can take on their behalf
([`../docs/research/R-10-ci-runners.md`](../docs/research/R-10-ci-runners.md),
read 2026-09-02). The repository will be public on GitHub
([`../docs/decisions-pending.md`](../docs/decisions-pending.md), DP-007,
decided 2026-09-02) but the remote does not exist yet, so the question is not
yet live.

Four constraints will shape whatever is eventually written. The first three
come from the same note as the paragraph above (`R-10`, read 2026-09-02); the
fourth was found by review and is dated where it is stated:

- **The filename becomes the page title** and the extension selects the
  renderer. The names here are already the titles that should appear.
- **Directory nesting does not survive** as hierarchy, and the characters
  `\ / : * ? " < > |` cannot appear in a page title. This directory is
  deliberately flat for that reason.
- **A wiki is a separate repository, so no link may leave it.** Every
  `../docs/...` and `../README.md` in these pages resolves when read here and
  breaks the moment they are published. `scripts/check-links.sh` counts them on
  every run — the count is deliberately not repeated here, because a number in
  prose beside a script that computes it is a number that goes stale, and this
  one went stale within the hour of being written. Rewriting them needs the
  repository's absolute URL, which does not exist yet (DP-007), so this is a
  publication step and not a cleanup: **before publishing, every one of them
  becomes an absolute link or the text moves into the wiki.** It is listed here
  because it is the constraint that will actually bite, and it was the one this
  file did not mention.
- **Wikis are only indexed by search engines** for repositories with 500 or
  more stars *and* public editing disabled. If discoverability turns out to
  matter, GitHub Pages is the better target and the wiki is contributor-facing
  material. Either way, the source stays here.

Until a decision is made, read these pages in the repository. Every internal
link resolves as a relative path, so they work as files.
