# Decisions pending a human

Decisions an agent must not make alone, because they trade away something the
project owner may not want traded. Each carries a recommendation and the
reversible default taken in the meantime, so work is never blocked.

Resolved decisions move to the bottom of this file with a pointer to where they
are recorded.

---

## Open

### DP-004 — Release signing
- **Options:** Developer ID signing plus notarisation (requires a paid Apple
  Developer account) versus ad-hoc signing with documented quarantine-clearing
  instructions. See SE-014 and research item R-9.
- **Note:** macOS 10.12 is old enough that Gatekeeper behaviour differs across
  the supported range; the research item should cover the whole range, not just
  current macOS.
- **Default meanwhile:** ad-hoc signing plus honest documentation; do not
  publish a public release until decided.

### DP-006 — Copyright holder
- **Why it is the owner's call:** it is a legal identity — an individual's full
  legal name or an entity — and it is awkward to change once contributions and
  releases carry it.
- **Where it appears:** README, release metadata, and any future `NOTICE` or
  `THIRD-PARTY-NOTICES` file. **Not** in source files: per ADR-0001 those carry
  an SPDX identifier only, so this decision blocks nothing in the code.
- **Default meanwhile:** omit the copyright line entirely rather than guess.
  The Apache-2.0 grant is effective without it.

### DP-007 — Repository hosting and visibility
- **Why it is the owner's call:** deciding when the work becomes public.
- **Context:** the folder is not yet a git repository, by the owner's choice
  (2026-09-02). A remote repository is also what would make the project
  reachable from devices other than the linked Mac — Cowork projects themselves
  are desktop-only and do not sync.
- **Note:** the repository name should be `macportsbrew` (ADR-0003).
- **Default meanwhile:** none. The foundation pass initialises git locally as
  its first step; pushing anywhere is a separate, explicit decision.

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
