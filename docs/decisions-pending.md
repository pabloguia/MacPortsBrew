# Decisions pending a human

Decisions an agent must not make alone, because they trade away something the
project owner may not want traded. Each carries a recommendation and the
reversible default taken in the meantime, so work is never blocked.

Resolved decisions move to the bottom of this file with a pointer to where they
are recorded.

---

## Open

### DP-008 — Is the `.pkg` also provisional pending R-11?
- **The question:** `08-distribution-and-project.md` marks the `.dmg`
  **provisional pending R-11** and the `.pkg` unconditional — but the same
  premise carries both. R-11's first item is whether stapling a container covers
  a binary copied out of it. If it does not, the `.dmg` is dropped, and the
  `.pkg`'s stated reason for existing ("they can carry a stapled ticket") is
  falsified for the installed tool too.
- **Options:** mark both provisional, symmetric with the premise; **or** state in
  the chapter what the `.pkg` buys *independently* of stapling and keep it
  unconditional. There is a real independent benefit — Apple's Installer verifies
  the package's signature at install time, which is a different guarantee from a
  stapled ticket on an extracted binary — but it is not written down anywhere,
  and an unconditional MUST resting on an unstated benefit is the same defect in
  a different place.
- **Recommendation:** write the independent benefit into the chapter, then keep
  the `.pkg` unconditional. That is the answer that survives either outcome of
  R-11, and it makes the asymmetry deliberate instead of accidental.
- **Default meanwhile:** the chapter is left as written. This is safe rather than
  lucky: the only thing that could act on the `.pkg` MUST is the release
  workflow, and R-11 blocks that workflow anyway, so the question cannot be
  reached before it is answered.

### DP-009 — May the first public release ship before the revocation plan exists?
- **The question:** ADR-0005 states that a revoked certificate — or a revoked
  notarisation ticket — stops previously published artifacts from launching, on
  the machines of the users this project exists for, and that the secure
  timestamp does *not* answer revocation the way it answers expiry. It then says
  the incident-response plan "is not in this record and is a task, not a solved
  problem". Nobody has decided whether a release may precede that plan.
- **Note:** the asymmetry is worth seeing. The **inbound** channel is already a
  hard prerequisite of publication (`SECURITY.md`: private vulnerability
  reporting enabled before the repository is public). The **outbound** path —
  how a user learns a release was withdrawn, when the failure mode is a binary
  that silently stops launching — has no channel, no gate and no owner.
- **Recommendation:** write the plan first. It is four questions on one page —
  who revokes, how a release is withdrawn, how users are told, what the next
  release looks like on a revoked identity — and writing it after the first
  release means writing it during an incident.
- **Default meanwhile:** no public release. Nothing is published, so nothing is
  at risk while this is open.

---

## Resolved

### DP-002 — Implementation language — **RESOLVED 2026-09-02: Rust**
Decided by the project owner. Recorded in
[`adr/0002-implementation-language.md`](adr/0002-implementation-language.md)
(now *accepted*) and SE-022. The foundation pass is unblocked from step 3.

Consequences that now bind the project, propagated rather than filed:
`rust-toolchain.toml` pins the toolchain and a floor increase needs an ADR; CI
asserts the deployment target on the built artifacts rather than trusting the
build; the dependency gate checks licence, deployment floor **and** the `unsafe`
and build-script surface a crate brings in; and ADR-0002's verification list
becomes acceptance criteria on the first pass, not a reading exercise.

### DP-003 — Licence — **RESOLVED 2026-09-02: Apache License 2.0**
Decided by the project owner. Recorded in
[`adr/0001-licence-apache-2-0.md`](adr/0001-licence-apache-2-0.md) and SE-021.
`LICENSE` is committed. Consequences that now bind the project: SPDX headers on
every source file, no `NOTICE` file until something needs attributing,
inbound=outbound contributions with no CLA, and **no GPLv2-only dependencies**.

### DP-001 — Minimum supported macOS — **RESOLVED 2026-09-02: 10.12 Sierra (Intel) / 11.0 (Apple Silicon)**
Decided by the project owner. Recorded in
[`adr/0002-implementation-language.md`](adr/0002-implementation-language.md) and SE-022.
This is the constraint that decides DP-002 and binds every dependency: anything
that raises the deployment floor is rejected.

### DP-005 — CLI short alias — **RESOLVED 2026-09-02: `mpb`, with a conflict guard**
Decided by the project owner, together with the rename to **MacPortsBrew**.
Recorded in [`adr/0003-name-and-cli-alias.md`](adr/0003-name-and-cli-alias.md)
and SE-023. `mpb` collides with MIT Photonic-Bands (installed by other means — neither
provider packaged it when checked, ADR-0003), so installation detects an
existing `mpb` and declines to shadow it silently.

### DP-004 — Release signing — **RESOLVED 2026-09-02: Developer ID signing and notarisation, three artifacts**
Decided by the project owner, who already holds a paid Apple Developer account.
Recorded in [`adr/0005-release-signing-and-artifacts.md`](adr/0005-release-signing-and-artifacts.md)
and SE-014. Every release artifact is signed with Developer ID and notarised,
and three are published per architecture: the bare binary — still the path the
documentation leads with — plus a `.pkg` and a `.dmg`, which exist because they
can carry a stapled notarisation ticket and a bare Mach-O cannot (R-9).

The consequences — credential handling, why the tag-push rule needs a protected
environment rather than a convention, where the `.pkg` may install, and what
remains unverified — are in
[`adr/0005-release-signing-and-artifacts.md`](adr/0005-release-signing-and-artifacts.md).
The binding requirements are restated once, in
[`spec/08-distribution-and-project.md`](spec/08-distribution-and-project.md);
the specification change is SE-014. R-11 gathers what still blocks the
implementation pass, and the R-9 quarantine experiment survives as documentation
of the path not taken rather than as a decision gate.

### DP-006 — Copyright holder — **RESOLVED 2026-09-02: Pablo Guia**
Decided by the project owner. The string matches the name on the Apple
Developer account, so the Developer ID signature and the copyright line name
the same person rather than two spellings of one. Appears in `README.md`,
release metadata and any future `NOTICE`; **not** in source files, which carry
an SPDX identifier only (ADR-0001).

### DP-007 — Repository hosting and visibility — **RESOLVED 2026-09-02: public repository on GitHub**
Decided by the project owner: public, under a personal Pro account, named
`macportsbrew` (ADR-0003), with GitHub Pages for the project site and possibly
a custom domain later.

Consequences that now bind the project: standard GitHub-hosted runners are free
and unlimited on public repositories while the plan's included minutes are
private-repository only (R-10), so the two-architecture macOS matrix costs
nothing — but **`-large` runners are never free on public repositories and must
not be used**, and self-hosted runners do not belong on one, because a fork can
execute arbitrary code on them. That GitHub withholds secrets from
fork-triggered workflows is what makes ADR-0005's signing design safe to run in
the open, and equally what constrains its shape. Enabling Pages **is** the act of
publishing, not a step after it; if a custom domain is registered, use the
registrar's WHOIS privacy. `git push` stays denied by the hooks until the remote
exists.

**Before acting on this decision — settings no check in this repository can
verify.** Each is a one-way door in the sense that the damage happens in the
window before it is closed, so the order matters:

1. **Private vulnerability reporting enabled** (*Security* → *Report a
   vulnerability*) **before** the repository becomes public. A public repository
   shipping Developer ID-signed binaries with no private channel forces the first
   report of a signing-pipeline flaw onto a public tracker (`SECURITY.md`).
2. **A protected deployment environment** holding the signing credentials, with a
   deployment tag rule limited to `v*`, and no workflow other than the release
   workflow permitted to reference it. Repository secrets are readable by any
   workflow that names them, so this — not the convention that only one workflow
   asks — is what actually binds the key to a release (ADR-0005).
3. **A protected-tag ruleset**, so a non-owner cannot create or move a release
   tag. Without it, rule 2's `v*` condition is satisfied by anyone with write
   access.
4. **`dtolnay/rust-toolchain@master` pinned** before the repository is public. It
   is a mutable branch reference in the workflow that runs the gate, on a
   `pull_request` trigger; going public removes the mitigating fact it was
   reviewed under.
