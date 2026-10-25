# ADR-0003 — Product name and CLI alias

- **Status:** accepted
- **Date:** 2026-09-02
- **Pass:** pre-implementation (owner decision)
- **Resolves:** DP-005 · supersedes the working name used before this date

## Decision

Product name: **MacPortsBrew**. Canonical command: `macportsbrew`.
Short alias: **`mpb`**, installed alongside the canonical command.

Chosen by the project owner. The alias is a deliberate nod to *MPB — Música
Popular Brasileira*.

## Consequences

**Positive.** A memorable three-letter alias for a command users will type
often, with a story behind it. The name still names both providers, which keeps
the product legible to someone who has never heard of it.

**To manage: ordering implies precedence.** Leading with "MacPorts" can read to
a newcomer as a MacPorts-first tool, which is at odds with INV-17 and INV-18 —
the two providers are peers and migration is bidirectional. This is a
documentation obligation, not a naming mistake: the README's first paragraph and
the wiki home page MUST state the peer-provider position explicitly, and the
Homebrew-direction material must be as substantial as the MacPorts-direction
material rather than an afterthought.

**To manage: `mpb` is already taken.** MIT Photonic-Bands ships an `mpb`
executable. Neither provider packages it under that name as of 2026-09-02 —
`brew info mpb` on Homebrew 6.0.21 returned "No available formula with the
name \"mpb\"", and a port-name search for `mpb` on <https://ports.macports.org>
returned zero ports, both read 2026-09-02 — so the collision comes from
installations made by other means (from source, or through a scientific
software channel). The overlap with this project's audience is small, but the
collision is real and the consequence of ignoring it would be absurd: a tool
built to detect and prevent accidental executable shadowing would begin by
shadowing a scientific package. The guard is a `PATH` check, not a provider
lookup, so it holds whichever way `mpb` arrived.

Therefore, per `06-cli-and-ux.md`: where installation can run code it detects an
existing `mpb` on `PATH`, and when it finds one it installs only `macportsbrew`,
reports the conflict, and lets the user decide. `doctor` reports the situation
too. *(Amended 2026-10-23, DP-033 ruled (b): the signed `.pkg` runs no code, so
it cannot make that decision at install time. The release artifacts carry both
names; the guard there is that the documented `PATH` line appends the directory,
so an earlier `mpb` wins. No install route in this repository runs the detection
described above today: a cargo build produces both binaries unconditionally, and
the bare file is copied by the user.)*

Note also that MacPorts' own build automation tool is named `mpbb` — close
enough to be worth knowing about when writing documentation, though not a
conflict.

## Rejected alternative

Claiming `mpb` unconditionally. Rejected because it contradicts the product's
own safety position, for a saving of a few keystrokes in a rare conflict case.

## Revisit when

Telemetry-free evidence suggests the conflict never occurs in practice — at
which point the guard can be simplified, though there is little reason to.
