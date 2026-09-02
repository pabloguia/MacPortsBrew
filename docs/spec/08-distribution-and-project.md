# 08 — Distribution and open-source project

**Status:** Tier 3 unless marked.

## Distribution — the constraint that shapes everything

MacPortsBrew must be installable by someone whose Homebrew installation is
already broken, or who has no package manager at all. Therefore:

**MUST.** A self-contained binary with no runtime dependencies, distributed via
GitHub Releases for `x86_64` and `arm64`, with checksums. Installation
documentation leads with this path. `brew install` / `port install` may exist
later as conveniences; neither may ever be the only route.

**MUST.** Confront macOS Gatekeeper explicitly. A binary downloaded from the
internet carries the quarantine attribute, and on current macOS an unsigned,
un-notarised executable is blocked or warned about — R-9 verifies the exact
behaviour across the supported range — which is exactly the wrong first
impression for a tool that asks users to trust it with their package managers.
Decide and document one of: Developer ID signing plus notarisation (requires a
paid Apple account — a human decision, record it in
`docs/decisions-pending.md`); or ad-hoc signing plus explicit, honest
instructions for clearing quarantine. Do not ship a release without having made
this decision consciously.

**SHOULD.** Publish build provenance/attestation for release artifacts and a
verification snippet users can run.

**MUST.** Semantic versioning. Release artifacts record both the tool version
and the policy version. Changelog maintained.

## Licence

**Decided: Apache License 2.0.** Rationale, consequences and the deliberate
omission of a `NOTICE` file are in [`../adr/0001-licence-apache-2-0.md`](../adr/0001-licence-apache-2-0.md).

**MUST.** `LICENSE` at the repository root contains the canonical Apache-2.0
text, unmodified. The appendix boilerplate is left as-is — it is a template for
users of the licence, not a field to fill in.

**MUST.** Every source file carries an SPDX short-form identifier as its first
comment line:

```
SPDX-License-Identifier: Apache-2.0
```

No per-file copyright line — the identifier plus the root `LICENSE` is
sufficient, keeps headers from rotting, and avoids blocking on DP-006. A lint
rule enforces the identifier's presence; CI fails on a file without one.

**MUST.** The SPDX identifier `Apache-2.0` appears in package metadata, release
artifact metadata and a README badge.

**MUST NOT.** No `NOTICE` file while the project has nothing to attribute.
Apache-2.0 §4(d) makes a `NOTICE` binding on every downstream redistributor once
it exists, so it is added only when third-party code is actually vendored — and
then it is added in the same change that vendors the code.

**MUST.** `CONTRIBUTING.md` states the inbound-equals-outbound rule explicitly:
under Apache-2.0 §5, contributions are licensed to the project under Apache-2.0
by default, so **no contributor licence agreement is required**. Say this
plainly — contributors should not have to infer it.

**MUST.** Dependency licence policy, enforced mechanically (see
[`07-quality-and-testing.md`](07-quality-and-testing.md)):

- Allowed: Apache-2.0, MIT, BSD-2/3-Clause, ISC, Unlicense, Zlib, and
  MPL-2.0 where used unmodified as a separate component.
- **Forbidden: GPLv2-only.** It is incompatible with Apache-2.0. This is a real
  constraint on library selection, not a formality.
- Anything else requires a decision recorded in an ADR before it is added.

**MUST.** If third-party code is ever vendored, add a `THIRD-PARTY-NOTICES` file
listing each component, its licence and its source, plus the `NOTICE` file the
attribution requires.

## Repository files

`README.md` · `LICENSE` · `CONTRIBUTING.md` · `SECURITY.md` · `CHANGELOG.md` ·
`AGENTS.md` · `CLAUDE.md` · `.claude/` · `docs/` · `prompts/` · `wiki/` ·
issue templates · PR template · workflows.

**MUST NOT.** No boilerplate for appearance. Every file earns its place.

## AGENTS.md and CLAUDE.md

**MUST.** `AGENTS.md` is the canonical, tool-neutral orientation document for
coding agents.

**MUST.** `CLAUDE.md` is a thin Claude Code layer over it. What that layer may
and may not contain, and why the neutral file is the canonical one, is
specified once, in [`11-agentic-sdlc.md`](11-agentic-sdlc.md) (Memory files).

**MUST.** `AGENTS.md` is concise and high value. It explains: what MacPortsBrew is and why;
the layering and where things live; the provider-neutral design and the
isolation boundary; the domain concepts; how each adapter works and what it
must never do; the ownership and mapping models; the policy engine and how to
add a rule; the migration state machine; the primary-provider design; how to run
and extend the tests; how to add a mapping, a locale, a doctor check, a
provider; the invariants and where their tests live; the parts of the codebase
that are dangerous to change; and the specification-evolution process.

**MUST.** It tells future agents to read `docs/spec/01-invariants.md` and
`docs/spec-evolution.md` before significant work.

**MUST.** Every contributor recipe `AGENTS.md` and `CONTRIBUTING.md` describe —
adding a mapping, a policy rule, a locale, a doctor check, a fixture, a provider
— also exists as a skill under `.claude/skills/`, written from the same source
of truth, so a human contributor and an agent follow identical steps. A recipe
that exists in prose but not as a skill will be followed by people and improvised
by agents.

## Documentation strategy

**MUST.** Single-source with clear ownership, no uncontrolled duplication:

| Surface | Owns |
|---|---|
| `README.md` | What it is, why, quick start, install, one screenshot, links out |
| `wiki/` (in repo, published to the GitHub Wiki) | User documentation: guides, migration walkthroughs, troubleshooting, FAQ, CLI reference |
| `docs/` | Specification, ADRs, spec evolution, research findings, schemas |
| `AGENTS.md` | Agent orientation. `CLAUDE.md` adds only the Claude Code specifics |
| `prompts/` | Work orders, one per pass |

**MUST.** Wiki source lives in the repository under `wiki/` so it is
versioned, reviewable, agent-editable and link-checked, and is published to the
GitHub Wiki by automation. Research the current publishing mechanism; keep the
automation simple enough that a contributor can edit a page without learning it.

**SHOULD.** Wiki pages, at minimum: Home; Getting Started; Homebrew to MacPorts
Migration; MacPorts to Homebrew Migration; Using Homebrew and MacPorts Together;
Intel Mac Homebrew Support; Understanding Bottles and Source Builds; Package
Mapping; Dependencies and Safety; Undoing a Migration; Primary Provider;
Troubleshooting; CLI Reference; FAQ; Architecture; Contributing.

Page titles use the words people search for and describe what the page contains.
Clear beats clever. Content is written for a human; discoverability follows from
being genuinely useful and correctly titled.

**MUST.** Any documented claim about Homebrew or MacPorts behaviour carries its
source and the date it was checked, or is phrased conditionally. Time-sensitive
claims — especially the Intel timeline — are stated with their date so a reader
in 2028 knows what they are looking at.

**MUST.** CI checks internal documentation links and that every wiki page
referenced from navigation exists.

## Issues and contribution

**MUST.** A small set of issue templates: bug; incorrect recommendation; package
mapping problem; provider compatibility problem; feature request; documentation.
Each collects macOS version, architecture, provider versions, MacPortsBrew
version and policy version — ideally by asking for the `doctor --report` output.
Never request secrets.

**MUST.** `CONTRIBUTING.md` covers prerequisites, build, test, format, the
architecture overview, and the specific recipes: adding a mapping, adding a
policy rule, adding a locale, adding a doctor check, capturing a fixture.

## Compatibility posture

**Decided:** minimum supported macOS is **10.12 Sierra on Intel x86_64** and
**11.0 Big Sur on Apple Silicon** — the latter is not a choice, it is the first
macOS that ran on that hardware. Recorded in
[`../adr/0002-implementation-language.md`](../adr/0002-implementation-language.md).

**MUST.** This target is a hard constraint on the toolchain and on every
dependency, not an aspiration. A dependency that raises the deployment floor is
rejected the same way a GPLv2-only dependency is.

**MUST.** The published support matrix distinguishes what CI actually exercises
from what is merely believed to work. No hosted runner for macOS 10.12–10.15
is known (R-10 in `09-research-checklist.md` is the single place that fact is
established; until it is answered the versions are treated as untested), so
those versions are **best-effort** until someone verifies them on real
hardware, and the documentation says so in those words. Claiming tested
support for an untested OS is the kind of dishonesty this project cannot afford
— its users are on exactly those machines.

**MUST.** Prefer capability detection to macOS version comparisons. Where a
version check is unavoidable, isolate it, name it, and test both sides.
