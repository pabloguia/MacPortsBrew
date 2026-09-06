# Decisions pending a human

Decisions an agent must not make alone, because they trade away something the
project owner may not want traded. Each carries a recommendation and the
reversible default taken in the meantime, so work is never blocked.

Resolved decisions move to the bottom of this file with a pointer to where they
are recorded.

---

## Open

### DP-016 — Under replay, no package is known to be a service, and `MANUAL_REVIEW` will not fire

- **The question:** F-29. `macports::service::scan` answers `Unreadable`
  wherever the running host is not the machine being described — which is every
  `--replay` run, by design (INV-15, F-25). `defines_service` renders that as
  `false`, and `classify.rs` reads `false` as *not a service*. So no
  `PackageClass::Service`, and **INV-19's `MANUAL_REVIEW` does not fire on a
  replayed service package.** The adapter already reports
  `Capability::ServiceMetadata: Unknown`, which is the correct answer, and a
  grep across `application`, `policy` and `cli` finds **no reader** — re-checked
  2026-09-06. An unknown nothing distinguishes from a no is an unknown in name
  only, which is the exact shape `.claude/rules/adapters.md` forbids in the
  sentence "an unknown is never a yes".
- **Why the field is wrong regardless of the ruling:** `AGENTS.md` states the
  principle already — "`ArtifactAvailability` (`UNKNOWN` is a first-class
  answer, never a silent 'no')" — and `defines_service: bool` cannot hold it.
  Making it a tri-state implements a rule this project already has and is not
  what this entry is asking. **The question is only what `Unknown` should
  *do*.**
- **Why it is yours:** it trades a safety property against the usefulness of the
  feature that exists to demonstrate safety. Every option below is defensible and
  none is reversible cheaply once `--replay` output is something people rely on.
- **Options:**
  (a) **`Unknown` forces `MANUAL_REVIEW`, like a known service.** Safe and
  simple. Cost: *every* replayed MacPorts plan becomes `MANUAL_REVIEW`, so
  `--replay` can no longer show what a real plan looks like — and `--replay` is
  how a contributor without MacPorts exercises the planner at all.
  (b) **`Unknown` does not force review, but the plan says the status is
  unknown and which packages it could not classify.** Keeps replay useful;
  leaves the INV-19 hole open, now labelled instead of silent.
  (c) **Refuse to build a migration plan at all from a recording whose service
  metadata is unknown**, while `analyze`, `status` and `explain` keep working.
  The hole cannot be reached because the path that needs the answer is closed.
  (d) **Record service status in the recording itself**, so a replay of a
  machine that *had* MacPorts carries real data and `Unknown` means only "this
  recording predates the field". Best long-term, largest change, and it makes
  every existing fixture stale.
- **Recommendation: (c), then (d).** (c) is the honest reading of INV-01's
  spirit — do not produce a plan you cannot verify the safety of — and it costs
  only the one command that actually depends on the missing fact, where (a)
  costs all of them and (b) keeps a Tier 1 invariant unenforced by design. (d)
  is where this should end up, and (c) does not block it: when recordings carry
  service data, the refusal stops triggering on its own.
- **Default meanwhile:** unchanged. `Unknown` reads as `false`, `MANUAL_REVIEW`
  does not fire under replay, and it is written down in four places — F-29,
  `filesystem.rs`'s type documentation, `ServiceScan::Unreadable`'s comment, and
  here. Nothing executes plans in this build, so the hole cannot yet cost
  anything; it becomes live with the executor.

---


---

## Resolved

### DP-015 — The coverage floors had never been measured — **RESOLVED 2026-09-06: option (a), with the floors raised**

**The owner's ruling, given in session:** install the tool — "pode instalar a
ferramenta, tem minha autorização" — and hold **90% overall**, with **95% for
`domain`, `policy`, `adapters` and planning**. That is option (a) plus a change
to the numbers themselves: `adapters` rises from 80 to 95, and a workspace-wide
backstop is added where none existed. `cli` is deliberately not given a floor;
it is covered by the 90%.

**Measured the same day, for the first time in this repository's history:**

| Crate | Lines | Old floor | New floor |
|---|---|---|---|
| `domain` | 96.65% | 95 | 95 |
| `policy` | 97.46% | 95 | 95 |
| `application` | 95.27% | 95 | 95 |
| `adapters` | 95.29% | 80 | 95 |
| `cli` | 88.82% | none | none |

Overall **94.86%** (11300/11912). Every floor the owner set is met, and
`adapters` cleared its raise with no work — it was already at 95.29% under an
80 floor, which is the pleasant half of never having measured.

The unpleasant half: **`application` was at 89.47%**, under a floor its own gate
had demanded since 2026-09-02, and nothing said so because no CI job has ever run
(F-12). Nineteen tests closed it. `planner.rs` had zero unit tests.

**What is not done, and why.** The new floors are not written into
`scripts/check-coverage.sh` or `docs/spec/07-quality-and-testing.md`. Both are
guard-protected, and the disclosed escape is the human setting
`MPB_ALLOW_GUARD_EDIT=1` at session start (`docs/spec/11-agentic-sdlc.md`).
Writing 95 into the specification while the gate still enforces 80 would be the
same defect this pass corrected three times over — documentation promising what
the code does not do — so it waits rather than being half-applied. **This is the
one outstanding owner action from DP-015**, and it is mechanical, not a decision.

Two limits carried forward from the entry below and still true. **Branch coverage
is unenforced**: `--branch` needs nightly, the pin is stable (ADR-0002), so the
spec's "95% line *and* branch" stays half-enforced and the numbers above are
lines only. And **`check-coverage.sh` counts test code living in `src/` in its own
denominator** — in-module tests moved `application`'s total from 1851 to 1902
lines, diluting the figure they were raising. Worth fixing in the same edit as
the floors.

<details>
<summary>The original entry, as filed</summary>

### DP-015 — The coverage floors have never been measured, and the toolchain policy is why
- **The question:** `07-quality-and-testing.md` sets coverage floors — 95% for
  `domain`, `policy` and planning, 80% for `adapters` plus a contract test per
  parsed command, `cli` reported. None has ever been measured. `cargo-llvm-cov`
  is not installed, and `cargo ins`​`tall` is denied by this repository's own
  tooling policy because the pinned toolchain holds the macOS floor (ADR-0002).
- **Correction, 2026-09-05: this entry understated what already exists.** It
  read as though nothing measured the floors at all. `scripts/check-coverage.sh`
  has existed since 2026-09-02, enforces `domain`/`policy`/`application` at 95%
  and `adapters` at 80%, and is wired into the `coverage` job of
  `.github/workflows/repository-validation.yml`, which installs the tool from a
  pinned action. Locally it exits 2 with "not available" by design. So the gate
  is built and connected; what has never happened is a **run** — F-12 records
  that no CI job has ever executed, because there is no remote. The floors are
  unmeasured for that reason, not for want of a gate, and the fix is smaller
  than this entry implied.
- **Also already recorded, and worth not re-discovering:** branch coverage is
  not enforced because `--branch` needs a nightly toolchain and the pin is
  stable (ADR-0002). The script says so in its header. The spec's
  "95% line **and** branch" is therefore half-enforced today.
- **Why it is yours:** every route out spends something. Relaxing the denial
  trades away part of what ADR-0002 bought; vendoring the tool adds a dependency
  to a project that checks every dependency's licence, deployment floor and
  `unsafe` surface; measuring only in CI leaves the floor unverifiable locally
  and waits on R-11's runner questions; and withdrawing the numbers is a real
  option that costs a stated quality bar.
- **Options:** (a) allow that one tool by name, as an exception with its own
  entry, and measure locally and in CI; (b) a CI-only step, accepting that a
  contributor cannot check the floor before pushing; (c) pin it as a workspace
  dev-dependency and take it through the dependency gate every other crate
  passes; (d) withdraw the numeric floors and state a weaker rule the project
  can actually hold.
- **Recommendation:** (a). It is the smallest exception that makes a stated
  requirement checkable; the tool is developer-only and never reaches a released
  artifact; and ADR-0002's concern is the *compiler* toolchain rather than a
  coverage harness. If you would rather not touch the denial at all, (c) is the
  honest second.
- **Default meanwhile:** the floors stand as written and are unverified. That is
  the state this entry exists to stop being invisible — nothing measures them, no
  gate fails on them, and no report has claimed otherwise. A gap being named,
  not a regression.

</details>

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

### DP-012 — Do casks belong in the default top-level view? — **RESOLVED 2026-09-05: yes, and labelled honestly**
Decided by the project owner, in two parts. The ownership *source* is option
(c): the adapter reads the cask's on-disk receipt, so a cask Homebrew recorded
as requested is `TOP_LEVEL` and one with no receipt is `UNKNOWN` — never the
inferred `TOP_LEVEL` the code produced on evidence it marked non-authoritative.
The *visibility* half, which this entry always reserved for the owner, is that
an `UNKNOWN` row **appears in the default view**. Recorded in **SE-046**, now
accepted.

**Why the visibility half needed deciding, with the number that made it
decidable.** Implementing (c) alone was measured first: under `--replay` the
recording carries no receipt, so every cask becomes `UNKNOWN`, and the default
view lost nine rows and manual review fell from 15 to 6. The recommendation this
entry carried had priced (c) at "two rows rather than nine" — true of a live
run, wrong of a replayed one, and the approval had been given for the smaller
number. Showing `UNKNOWN` keeps the population intact and changes only the
label: 56 units in the default view, of which nine now say "not known" instead
of a false "requested".

**What that separated.** `topLevelCount` and `topLevelUnits` had coincided by
accident. The schema defined the first — "those the provider's own record says
the user asked for" — and left the second's Notes cell **empty**; "units shown in
normal mode" was a Rust doc comment, and an earlier draft of this entry quoted it
as though the schema said it. The schema says it now. They diverge, 47 and 56,
each matching its own definition.

The research behind it, including both questions you asked about migrating
casks, is `docs/research/R-12-cask-migratability.md`; the absolute it falsified
is F-28.

### DP-014 — Provider prefixes were compiled in — **RESOLVED 2026-09-05: option (a)**
`data/policy/provider-prefixes.toml` holds the three prefixes with their sources
and read dates, and `crates/adapters/src/prefixes.rs` reads it — embedded with
`include_str!`, exactly as `crates/policy` embeds its own data. The layering
objection was real and is answered by the loader living in `adapters` rather
than reaching up into `application`. Recorded in **SE-071**.

**The INV-20 test's exemption is gone, which is the proof.** It used to excuse
`registry.rs` with a comment saying the exemption was not compliance; the check
now holds the whole workspace, and putting a prefix back into that file was
replayed as a mutation and refused.

**One line of this entry's own rationale was wrong.** It said "a data file is a
one-line edit with a new `read_on`; a `match` is a release". Both are a release:
every policy file in this project arrives through `include_str!`. What INV-20
asks for is that a fact be stated as versioned data beside the source it came
from and the date it was read, where a person can compare the two — not that it
be changeable without rebuilding, which nothing here does.

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
