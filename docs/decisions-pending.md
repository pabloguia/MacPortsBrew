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

### DP-010 — A Tier 1 invariant is currently relaxed by a Tier 3 entry. Which gives way?
- **The question:** `crates/application/src/analysis.rs:167-180` feeds the policy
  engine the **maximum** reverse-dependency count across providers. INV-04 says
  reverse dependencies are resolved "inside that provider"; INV-08 says one
  provider never satisfies another's requirement "in any calculation, ever". This
  is such a calculation. It is *declared* — SE-042 change (2) — but a Tier 3–5
  entry cannot relax a Tier 1 invariant, and the authority table does not offer a
  route by which an agent could have approved it. So the question is the owner's:
  does the invariant stand, or does INV-04/INV-08 need an explicit carve-out?
- **What it costs today:** the user is told something false. For `gnutls` on the
  shipped both-providers fixture the emitted reason reads
  `count: "3", provider: "macports"` when **zero** MacPorts ports depend on it —
  the three are Homebrew's, relabelled. The outcome flips to `MANUAL_REVIEW` on the
  strength of a graph the plan does not touch. The planner does not do this; it
  reads the correct per-provider value, so engine and planner disagree on one run.
- **In the current behaviour's favour:** `max` only ever over-reports, so it cannot
  *miss* a blocking condition. The safety error is conservative. If the carve-out
  is chosen, that is the argument for it.
- **Options:** (a) the invariant stands — the engine receives the source provider's
  own count, or `reverse_dependency_count` moves into the per-provider map where
  availability already lives correctly; SE-042(2) is withdrawn. (b) INV-04 and
  INV-08 gain an explicit, narrow exception for the *advisory* reverse-dependency
  signal, written into Tier 1 by the owner, and the reason text is fixed regardless
  so it names the provider the count actually came from.
- **Recommendation:** (a). The conservative direction of the error is real but it
  is not what INV-08 says, and the false provider label is a defect under either
  option. If the "worse side" signal is wanted, it can be expressed as a distinct,
  honestly-named fact rather than by overwriting a per-provider count.
- **Default meanwhile:** unchanged. Nothing executes, so the cost is a wrong number
  and an over-triggered review, not an unsafe action. Recorded as SE-045
  (`Status: proposed`) and finding F-01.

### DP-011 — The charter states a provider fact its own data contradicts (Tier 2)
- **The question:** `docs/spec/00-charter.md:37` says Homebrew classifies Intel
  x86_64 macOS as "**Tier 3 — not supported**". Re-verified 2026-09-02, the source
  treats Tier 3 and Unsupported as distinct: Intel on Catalina 10.15 through Tahoe
  26 is **Tier 3**, and "Unsupported" — defined as a configuration where Homebrew
  "will not run without third-party patches" — is Mojave 10.14 and earlier. The
  charter is Tier 2, so an agent may not change it.
- **Why it matters more than a wording nit:** this sentence is the project's stated
  reason for existing, it is copied into `README.md:31`, and the error runs in the
  direction that favours the tool. The repository's own data model already gets it
  right (`provider-support.toml` maps Intel Tier 3 to `DEGRADED`), so the binary is
  more honest than the prose — `status` prints `degraded` on an Intel machine.
- **Options:** amend the charter to "Tier 3 — degraded, with bottle builds stopped
  and an announced end date", and `README.md` with it; **or** keep the wording and
  record why the conflation is deliberate.
- **Recommendation:** amend. The timeline claim and the September 2027 date are
  correct and re-verified; only the tier vocabulary is wrong, and it is the part a
  sceptical reader can check in one click.
- **Default meanwhile:** unchanged, because it is Tier 2. Recorded as SE-044
  (`Status: proposed`) and finding F-11.

### DP-012 — Do casks belong in the default top-level view?
- **The question:** `AGENTS.md`, `.claude/rules/adapters.md` and SE-042 all state
  that a cask carries no user-intent field, so its ownership is `UNKNOWN` and is
  **never** inferred from a dependency graph. `crates/domain/src/ownership.rs:72-82`
  instead folds "no intent record" into `TOP_LEVEL`, and `classify_ownership`
  cannot return `Unknown` at all. On the reviewer's machine that is 9 casks — 16%
  of 56 top-level rows — inside the population INV-13 reserves for default views
  and destructive workflows, on evidence the code itself marks
  `authoritative: false`.
- **Why it is the owner's:** either answer changes what the default view contains,
  which is a scope decision rather than a bug fix. Making it `UNKNOWN` removes nine
  rows from the headline count on a typical machine; leaving it means the
  documentation stops promising `UNKNOWN`.
- **In the code's favour:** the inference is *recorded* as non-authoritative rather
  than silently guessed, which is better than what the absolute rule would suggest
  the alternative is.
- **Options:** (a) `None ⇒ Ownership::Unknown`, and decide separately whether
  `UNKNOWN` rows are shown in normal mode; (b) relax the written rule to "never an
  *unrecorded* graph inference" and keep the current behaviour.
- **Recommendation:** (b) for the rule, (a) for the reachability of `Unknown` —
  that is, keep casks visible, stop claiming they are `UNKNOWN`, and make
  `Ownership::Unknown` reachable for the cases that genuinely are. Casks are
  already classified out of scope for migration, so their presence in the count is
  informative rather than dangerous.
- **Default meanwhile:** unchanged. Recorded as SE-046 (`Status: proposed`) and
  finding F-20.


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
