# Decisions pending a human

Decisions an agent must not make alone, because they trade away something the
project owner may not want traded. Each carries a recommendation and the
reversible default taken in the meantime, so work is never blocked.

Resolved decisions move to the bottom of this file with a pointer to where they
are recorded.

---

## Open

### DP-014 — Provider prefixes are compiled in, and INV-20 names prefixes (Tier 1 compliance, and a layering question)
- **The question:** INV-20 says support tiers, deadlines, **prefixes** and
  known-package knowledge "live in versioned data files carrying a source and a
  read date, and are re-checkable". `crates/adapters/src/registry.rs:32-38`
  holds `/opt/homebrew`, `/usr/local` and `/opt/local` in a `match`, with the
  sources and read dates in a doc comment above it. Provenance is there; a data
  file is not, and re-checking still means re-releasing — which is the cost
  INV-20 exists to remove. Move them, or record why a doc comment satisfies
  INV-20 for prefixes?
- **Why it is not simply "move them":** `data/policy/` is loaded by the
  `application` crate, and `registry.rs` is in `adapters`, which sits *below*
  it — the Cargo edges are the layering rule and `scripts/check-layering.sh`
  asserts them. Moving the prefixes into `data/policy/provider-commands.toml`
  therefore needs one of: a second loader in `adapters`, a data file owned by
  `adapters`, or the prefixes being handed down from `application` at
  construction. Each is a real design choice about where "what a provider is"
  lives, and none is an obvious default.
- **Why it matters more than three strings:** the doc comment says the prefixes
  were read on 2026-09-02. Homebrew's Intel and Apple Silicon defaults have
  moved once in living memory. When they move again, a data file is a one-line
  edit with a new `read_on`; a `match` is a release. `prefix_override` mitigates
  it for anyone who knows to pass one.
- **Options:** (a) a small `data/policy/provider-prefixes.toml` read by a loader
  in `adapters`, keeping the layering intact and giving prefixes the same shape
  as every other dated fact; (b) `application` reads them and passes them to the
  registry, which makes `adapters` unable to detect anything on its own;
  (c) an explicit ruling that a doc comment carrying `source` and `read_on` next
  to a `match` satisfies INV-20 for this one category, recorded as an entry.
- **Recommendation:** (a). It is the smallest change that makes the invariant
  true rather than argued around, and it keeps detection where it belongs.
- **Default meanwhile:** unchanged, and the INV-20 test exempts `registry.rs`
  *explicitly*, with a comment saying the exemption is not compliance. Raised by
  `spec-guardian`, which found the first version of that comment citing
  `04-migration-and-safety.md` for a single-site rule that file does not contain
  — and SE-062 reporting the exemption as INV-20 compliance, which it is not.
  Recorded there and here rather than silently.

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

### DP-009 — May the first public release ship before the revocation plan exists? — **RESOLVED 2026-09-04: the plan exists**
The project owner asked for the plan rather than for a ruling, which is the
recommendation's path and removes the question. It is `SECURITY.md`, "If a
release must be withdrawn": when to revoke and — more usefully — when not to,
who can, what must be recorded before the first release, the runbook, what a
user can actually be told, and what the next release looks like on a revoked
identity. Recorded in **SE-068**.

Two things came out of writing it. The premise ADR-0005 rests on is **uncited**:
no page in this tree, and neither Apple page checked on 2026-09-04, says that
revocation stops previously published artifacts from launching. That is now
R-13, and the plan is written to hold either way. And the runbook's first
question — which artifacts carry the revoked identity — is unanswerable unless
the release records the signing identity CI observed, which is now a MUST in
`08-distribution-and-project.md`. Both were invisible while the plan was a
sentence in an ADR saying a plan was owed.

A security review of the plan then reordered it. Containment comes before
revocation — revoking first breaks the good releases while the bad one is still
downloadable — the trigger now covers an identity being *used* and not only a
key being read, and the digests are written down before anything is deleted,
because deleting a release deletes the `SHA256SUMS` the attestations are looked
up by. SE-068 carries the rest.

### DP-008 — Is the `.pkg` also provisional pending R-11? — **RESOLVED 2026-09-04: neither is**
Decided by the project owner: publish all three — bare binary, `.pkg`, `.dmg` —
unconditionally, and let the user choose. Recorded in **SE-067**;
`08-distribution-and-project.md` carries the decision and a new MUST that each
artifact be described where a user picks between them, and ADR-0005's "drop the
`.dmg`" bullet is marked superseded.

The question as filed had a false premise, and the answer that fixed it was not
one of the options offered. Both options — "mark both provisional" and "state
what the `.pkg` buys independently" — accepted the original clause's reasoning
that a container without a usable stapled ticket "buys nothing the bare binary
does not". It buys the installation experience, which no R-11 outcome touches.
What R-11 still decides is what the documentation may *claim*, and that is now
what the chapter says it decides.

### DP-011 — The charter's Intel tier vs. its own data — **RESOLVED 2026-09-04: amend**
Decided by the project owner. `00-charter.md`, `README.md` and
`wiki/Intel-Mac-Homebrew-Support.md` now name Tier 3 as Homebrew names it, quote
what Homebrew's separate *Unsupported* category means — "will not run without
third-party patches or modifications" — and state that it is Mojave 10.14 and
earlier, which Intel is not. Recorded in **SE-044**, now accepted. Finding F-11
is closed.

**The recommendation on file was itself wrong, in the same direction.** It
proposed the wording "Tier 3 — degraded, with bottle builds stopped", and
*stopped* is the claim `README.md` had already corrected to *frozen and eroding*
and that `provider-support.toml` records as `prebuilt_artifacts = true`.
Implemented literally, it would have removed one overstatement from a Tier 2
document by restoring another, under an approval given for the substance. The
substance landed and the words did not; SE-044 carries the reasoning. Two
entries proposing a correction is not two independent checks of it.

### DP-013 — Charter identity vs. what a release does — **RESOLVED 2026-09-04: option (b)**
Decided by the project owner. One sentence in `00-charter.md`, in the same
paragraph that asks for consistency: the identity describes the finished
product, a release surface describes that release, and where the two differ the
surface states what the build does. "Executes migrations" stays in the identity,
because it is true of the project and not of this build. Recorded in **SE-058**;
no other surface changed, because they were already right.

The sentence also names the failure mode it exists to stop. The risk DP-013
identified was never the drift itself — it was the next consistency audit
correctly finding the charter to be the outlier and "fixing" the README back to
a claim the binary cannot support.

### DP-010 — Reverse-dependency counts — **RESOLVED 2026-09-02: the invariant stands**
Decided by the project owner: INV-04 and INV-08 hold as written, and the count
moves into the per-provider map rather than a side being chosen. Recorded in
**SE-045**, which withdraws SE-042 change (2); implemented with SE-047 and
SE-048, because the three were one defect seen from three angles — the count was
collapsed to one number partly *because* the source attribution was unstable.

What this bought, measured: the engine no longer tells a user that three
MacPorts ports depend on a package when zero do; the rule fires on exactly the
139 units whose source side has dependents, with no divergence; and the
planner's own message names the provider the count came from. What it cost: the
shipped fixture's `manualReview` total goes from 23 to 22 — one unit changes
outcome, three lose a reason they should not have carried — and one golden file
changed.

The reasoning that made this the owner's call rather than an agent's is kept in
SE-045: the original `max` was not carelessness but a genuine collision between
INV-04 and INV-17, and a Tier 3 entry had been used to resolve it.


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
