# Specification evolution

The current rationale behind every material change to the specification:
what was originally assumed, what the specification now says instead, why,
the evidence, and the impact. Trivial implementation choices are not recorded.

**Convention — consolidate, not accumulate (SE-032).** This file is organised
by topic, not by date. When a change touches a topic that already has an
entry, the entry is extended and what it supersedes is dropped; identifiers
are stable, and a merged entry keeps one of the absorbed ids with
cross-references updated in the same change. A rejected change keeps its
entry, because a rejection is the current rationale for something's absence.
What never changes: a Tier 1–3 change lands with its entry, written when the
change is made. Silent deviation remains the one unrecoverable error.

**Entry format**

```
## SE-NNN — Title
- Status: accepted | experimental | deferred | rejected
- Date / Pass:
- Original assumption:
- Change:
- Rationale:
- Evidence:            (a URL with read date, a command, a checkable artifact — or
                        `design judgement` when no external fact is involved)
- User impact:
- Architectural impact:
- Compatibility impact:
```

---

# Product and scope

## SE-003 — Homebrew's Intel timeline is a dated fact, and sharper than the brief assumed
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: "Homebrew has progressively reduced official support for
  Intel macOS", stated qualitatively.
- Change: recorded as concrete, dated, sourced policy data: Intel x86_64 is
  Tier 3 (not supported); no new Intel bottles are being built; Homebrew states
  it will remove the ability to run on Intel systems in or after September 2027.
  Stored in versioned policy data with source and read date, never in code
  (INV-20).
- Rationale: a deadline changes the product from "quality-of-life migration
  helper" to "migration with a clock on it", which changes recommendation
  weighting, README framing and urgency. It is also exactly the kind of fact
  that will change again — so it must be data.
- Evidence: <https://docs.brew.sh/Support-Tiers>, read 2026-09-02.
- User impact: high. Compatibility impact: policy weighting on Intel machines.

## SE-012 — Casks and third-party taps explicitly out of migration scope
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: casks mentioned once as metadata to research.
- Change: `GUI_APP` and `THIRD_PARTY_SOURCED` classes; both reported in
  inventory and `doctor`, both excluded from migration planning.
- Rationale: MacPorts has no concept equivalent to a cask, and a package from
  a third-party tap has no reason to exist upstream. Any mapping produced for
  these would be invented, and inventing mappings is how a tool silently
  installs the wrong software.
- Evidence: `02-domain-model.md`, package classes; R-1 in the research
  checklist confirms cask semantics.

## SE-018 — Explicit non-goals
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: an extensive list of implementation prohibitions, but
  no product-scope boundary.
- Change: a non-goals section in the charter.
- Rationale: scope creep in a tool like this arrives as reasonable-sounding
  feature requests. Written non-goals let a maintainer decline in one line, and
  keep an agent from helpfully implementing a cask migrator.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-019 — Legal and etiquette posture
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: not addressed.
- Change: non-affiliation notice; interaction only through public supported
  interfaces; no vendoring, forking or scraping; no load on provider
  infrastructure beyond normal user behaviour.
- Rationale: the product name combines two third-party project names and the
  tool drives both. Cheap to state, awkward to retrofit.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-023 — Renamed to MacPortsBrew; CLI alias `mpb` with a conflict guard
- Status: accepted
- Date / Pass: 2026-09-02 / owner decision (DP-005)
- Original assumption: product name MacBrewPorts; CLI `macbrewports`; short
  alias an open question.
- Change: the product is **MacPortsBrew**, the canonical command is
  `macportsbrew`, and **`mpb`** ships as an equal short alias. Full reasoning
  in ADR-0003. Two consequences the specification carries: the README's
  opening and the wiki home page must state the peer-provider position
  explicitly, with the Homebrew-direction material as substantial as the
  MacPorts-direction material, because leading with "MacPorts" can read as
  MacPorts-first (INV-17, INV-18); and installation detects an existing `mpb`
  on `PATH` — MIT Photonic-Bands ships one; neither provider packaged it when
  checked, so it arrives by other means — and installs only `macportsbrew`
  rather than shadowing it.
- Rationale: owner decision; a tool whose purpose includes detecting
  accidental executable shadowing cannot begin by doing it.
- Evidence: ADR-0003, which records the provider observations with read
  dates (the earlier claim that both providers package `mpb` was wrong);
  `06-cli-and-ux.md` (Surface).
- Architectural impact: none. The conflict guard lives in installation and
  `doctor`.

---

# Domain and safety

## SE-004 — Artifact availability modelled symmetrically, not as a Homebrew "bottle" concept
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: detect Homebrew bottle vs source build; MacPorts
  implicitly treated as the binary-archive side.
- Change: one symmetric `ArtifactAvailability` model applied to both providers.
- Rationale: MacPorts also builds from source in real cases — non-default
  variants and licence-restricted ports are the cases the design assumes. A
  Homebrew-shaped model would produce recommendations that are wrong precisely
  when the user is on an old machine where a source build is painful.
- Evidence: R-4 in the research checklist, blocking and unanswered — it
  verifies the MacPorts cases; the model is symmetric by design (INV-17)
  whatever R-4 finds.
- Extended 2026-09-02 (00-REPO-READINESS-PASS), same principle applied to
  two places the baseline audit measured as asymmetric: the "MacPorts only"
  provider-presence row in `02-domain-model.md` is spelled out like the
  "Homebrew only" row — including the obligation to offer Homebrew
  installation with consent (INV-05), which the old row only implied by
  "the symmetric behaviour"; and the "Toward Homebrew" starting policy in
  `03-policy-and-recommendations.md` gains the analogue of the
  MacPorts-direction rules (a port marked replaced, obsolete or unbuildable
  on this macOS while Homebrew offers a supported equivalent), a hypothesis
  whose MacPorts states R-3 names. Evidence: the audit's counts — 58 words
  and two rules toward MacPorts against 26 and one toward Homebrew.
- Architectural impact: the assessment type is provider-neutral.

## SE-005 — Cross-provider contamination has a named mechanism, and an invariant to prevent it
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: "never intentionally create cross-provider dependencies",
  with detection deferred to a deep `doctor` scan.
- Change: INV-10 — all provider invocations run in a sanitised environment that
  does not leak the other prefix into build-visible variables. Detection
  remains; prevention is primary.
- Rationale: MacPorts' own FAQ notes that `/usr/local` interferes with builds
  because autotools and gcc probe it — and on Intel Macs `/usr/local` is
  Homebrew. Contamination is the default outcome of running `port` in a
  Homebrew user's shell; detecting it afterwards is much worse than not
  causing it.
- Evidence: <https://trac.macports.org/wiki/FAQ>, read 2026-09-02 (F-2).
- Architectural impact: the process executor owns environment construction;
  no ad-hoc invocations.

## SE-006 — Package classes; stateful and service packages are never auto-migrated
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: packages differ by ownership (top-level vs dependency)
  and by mapping confidence.
- Change: a package **class** dimension — `SIMPLE`, `STATEFUL`, `SERVICE`,
  `RUNTIME`, `GUI_APP`, `THIRD_PARTY_SOURCED`, `PROVIDER_INTERNAL` — and
  INV-19 forbidding automatic migration of stateful and service packages.
- Rationale: the migration model (install destination, verify, remove source)
  is correct for a CLI tool and silently destructive for a database.
  Reinstalling a database under another provider does not move its data
  directory, and the two providers manage background services differently.
  Without this, the headline safety promise is false for exactly the packages
  where failure hurts most.
- Evidence: R-8 in the research checklist, blocking and unanswered — it
  verifies the detection facts the classes rely on.
- User impact: high — prevents a class of data loss.
- Architectural impact: classification is part of inventory, not of planning.

## SE-007 — "Verify" given an operational definition
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: "remove the original only after verification", with
  verification undefined.
- Change: the five-check verification contract in `04-migration-and-safety.md`:
  provider-reported installation, provider integrity check, file ownership
  confirmed *by asking the provider* rather than by path matching, a
  non-mutating liveness probe, and a contamination check.
- Rationale: INV-01 is the project's most important promise and was
  unimplementable as written. An undefined verification degrades in practice to
  "the install command exited zero", which is not a safety property.
- Architectural impact: verification is a first-class application service with
  per-package overrides in curated data.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-008 — Snapshot and restore as the rollback mechanism, the determinism proof, and the undo story
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review; extended 2026-09-02 /
  00-REPO-READINESS-PASS
- Original assumption: rollback "where safe"; snapshots as an optional
  lightweight backup idea; `history` a bare command name with no user story
  for undoing a migration; two descriptions of what a snapshot contains.
- Change: `Snapshot` is one domain type (`02-domain-model.md`) with four jobs
  — automatic pre-mutation safety net, `analyze --from-snapshot`
  reproducibility harness (how INV-15 is *proven* rather than asserted), the
  source of all test fixtures, and the payload of the redacted diagnostic
  bundle. Extended: "undo a migration" is an explicit, discoverable user story
  (`04-migration-and-safety.md`, "Undoing a migration"; `06-cli-and-ux.md`).
  Every mutating run records its snapshot, plan and outcome in history and
  ends by printing `macportsbrew restore --run <id> --plan`; the restore plan
  is an ordinary `MigrationPlan` through the same review, executor and
  ordering (INV-01, INV-02, INV-04, INV-17, INV-19); it reverts the run's
  primary selection and managed shell block through the same procedures; a
  step whose artifact is no longer available on this machine becomes
  `MANUAL_REVIEW` and the run is reported partially restorable; `--snapshot`
  restores are additive unless advanced mode is entered; interrupted runs
  recover through `history show` and `restore`. The command is named
  `restore`, not `undo`, and the specification says what it does not promise:
  version-exact reconstruction, `STATEFUL` data, changes made outside the
  tool, or a run whose snapshot is missing. Automatic snapshots live in the
  state directory and are never pruned automatically; a missing one marks
  the run not restorable. A run made with `--retain-source` verifies the
  retained source before removing the destination, in the same order as any
  migration. Exit codes: a restore that cannot proceed for a missing
  snapshot exits 8 (stale input); a restore plan with a `MANUAL_REVIEW` step
  exits 4 (safety check) when executed.
- Rationale: package managers do not offer transactions, so honest rollback is
  "here is what you had, here is a reviewable plan to get back". Reusing the
  same artifact for testing and bug reports means the safety net is exercised
  constantly instead of only in an emergency. The undo story was the one
  place a user in trouble would look and find a command name without a
  path; naming it `undo` would have promised the transaction the providers
  do not offer, and the Intel case — the destination that no longer offers
  the package — is exactly the user this project exists for.
- Evidence: `04-migration-and-safety.md` (Rollback and snapshots; History);
  the readiness pass's baseline documentation audit, 2026-09-02, indexed in
  `passes/00-REPO-READINESS-PASS-report.md`, which found the user story
  undiscoverable and listed eight scope gaps, all closed.
- Architectural impact: significant and simplifying — one serialisation format
  serves four purposes; restore adds no executor path that migration does
  not already have.

## SE-015 — Concurrency, caching and offline behaviour
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: not addressed.
- Change: single-instance lock; detection of a concurrently running `brew` or
  `port` before mutation; a cache with TTL and provenance that is never
  authoritative for a mutating decision; `analyze` works offline with clearly
  marked `UNKNOWN` facts.
- Rationale: package managers are shared mutable state on the machine.
  Interleaving two writers is a corruption path, and an analysis that fails
  without network is useless to someone on a laptop with a broken Homebrew.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-016 — Privilege model made concrete
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: "never run entirely as root; elevate per operation".
- Change: the design assumes MacPorts operations generally require elevation
  and Homebrew's generally do not, with R-5 verifying the per-operation facts;
  the plan enumerates every elevating step; credentials are never cached;
  analysis and dry-run never elevate — including no `sudo` prompt.
- Rationale: the asymmetry between the two providers is a real UX and safety
  issue that symmetric language hid. "Dry run never prompts for a password" is
  a testable property users can rely on.
- Evidence: R-5 in the research checklist, blocking and unanswered.

---

# Policy, output and quality

## SE-009 — Machine-readable output contract, exit codes and error taxonomy
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: not addressed.
- Change: `--json` with a versioned schema on every read command; JSON alone on
  stdout; stable documented exit codes including a distinct partial-success
  code; a stable error-identifier taxonomy with remediation strings.
- Rationale: it makes the tool scriptable, makes end-to-end tests assert on
  structure instead of on rendered text, makes bug reports precise, and keeps
  localisation out of the machine surface.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-010 — Coverage policy differentiated by layer
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: ">= 90% overall where reasonable".
- Change: 95% line and branch for `domain`, `policy`, planning, verification and
  safety logic; 80% plus a contract test per parsed command for adapters;
  reported-only for UI. Invariant tests are the real gate.
- Rationale: a single repository-wide number is met most cheaply by testing the
  easiest code, which is the UI, while the safety logic stays thin. Layered
  floors put the effort where a bug destroys a machine.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-011 — i18n architecture first, five locales later
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: en, pt-BR, es, ja and zh-CN required in scope from the start.
- Change: the foundation pass ships the i18n layer, the canonical English
  catalogue, a pseudolocale and a CI key-parity check. Real translations land in
  later passes. Layout measures display width, since CJK text is double-width
  and breaks naïve column alignment.
- Rationale: five hand-maintained catalogues written before the vocabulary has
  stabilised go stale within a pass and then mislead users in their own
  language, which is worse than English. The architecture is the hard part and
  is delivered now; the catalogues are cheap once the keys settle.
- User impact: non-English users wait one or two passes.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-020 — Foundation pass forbidden from shipping any mutating code path
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: "do not rush destructive migration"; a read-only vertical
  slice recommended.
- Change: a hard boundary — no install, uninstall, `sudo`, shell write, symlink
  or service code exists in the binary the foundation pass produces, not even
  behind a flag. Types, plans and tests for them do.
- Rationale: "prefer" is not a constraint an agent can be held to, and
  half-finished destructive code is the single most dangerous artifact this
  project can produce. It is also `grep`-checkable, so it is an acceptance
  criterion rather than an intention, and the `Stop` gate checks it.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

---

# Platform, toolchain, licence and distribution

## SE-014 — Gatekeeper confronted as a distribution requirement
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: ship binaries and checksums via GitHub Releases.
- Change: signing/notarisation versus ad-hoc signing plus honest quarantine
  instructions is a required, recorded decision before the first release
  (DP-004, R-9).
- Rationale: the stated goal is that MacPortsBrew installs without a working
  package manager. A downloaded binary that macOS refuses to run defeats that
  goal at the first step, for a tool whose entire proposition is trustworthiness.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-021 — Licence: Apache-2.0, with its consequences propagated
- Status: accepted
- Date / Pass: 2026-09-02 / owner decision (DP-003)
- Original assumption: licence undecided; MIT and Apache-2.0 both candidates.
- Change: Apache License 2.0. `LICENSE` committed with the canonical text.
  Propagated rather than filed: SPDX short-form headers on every source file,
  enforced by lint; no `NOTICE` file until there is something to attribute;
  `CONTRIBUTING.md` states inbound=outbound with no CLA; a dependency-licence
  allowlist enforced in CI; GPLv2-only dependencies forbidden, which the
  language decision had to clear.
- Rationale: the substantive difference from MIT is the explicit patent grant
  and its litigation-termination clause, plus inbound=outbound contributions
  and the trademark reservation — relevant for a project whose name combines
  two third-party project names. Full reasoning in ADR-0001.
- Evidence: Apache License 2.0 text, §§3–6. `LICENSE` verified against two
  independent copies and the SPDX canonical body text; sha256
  `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`, checked
  in CI.
- Architectural impact: constrains dependency selection.

## SE-022 — macOS floor 10.12 / 11.0, which decided the implementation language: Rust
- Status: accepted
- Date / Pass: 2026-09-02 / owner decisions (DP-001, DP-002); absorbs SE-013
  and SE-031
- Original assumption: minimum macOS undecided; language open between Go, Rust
  and Python, to be chosen on general merits.
- Change: the floor is macOS **10.12 Sierra on Intel x86_64** and **11.0 on
  Apple Silicon**; the language is **Rust** (ADR-0002, accepted). Propagated
  rather than filed: `rust-toolchain.toml` pins the toolchain and a floor
  increase is a breaking change requiring an ADR; `.claude/settings.json` and
  the `PreToolUse` guard deny `rustup default|override|toolchain` and
  `cargo install`; CI asserts the deployment target on the built artifacts,
  not on the build configuration; the dependency gate has three axes —
  licence, deployment floor, and the `unsafe`/build-script surface a crate
  brings into a privileged binary; ADR-0002's verification list became
  acceptance criteria on the foundation pass. R-7 in the research checklist is
  verification, not decision.
- Rationale: a user on macOS 10.12 with a legacy Homebrew installation is the
  purest instance of the problem this project solves — Homebrew is
  unsupported at 10.14 and earlier, MacPorts still serves that machine. A
  language chosen on ergonomics alone would have excluded the core audience
  by accident: current Go requires macOS 12+ (13+ from Go 1.27), so reaching
  10.12 would mean an unmaintained toolchain, unacceptable for software that
  performs privileged package operations. Rust's `x86_64-apple-darwin`
  supports 10.12 and `aarch64-apple-darwin` supports 11.0 — the targets
  exactly. Choosing 10.12 was effectively choosing Rust.
- Evidence: <https://go.dev/wiki/MinimumRequirements> and
  <https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html>, read
  2026-09-02; <https://docs.brew.sh/Support-Tiers> for Homebrew's own floor;
  ADR-0002 for the full evaluation.
- User impact: the machines the product exists for are supported, and the
  mechanism that keeps them supported is enforced rather than intended.
- Architectural impact: the deployment target is a binding constraint on
  every dependency. The domain, policy and adapter boundaries were written
  language-neutral and remain so.
- Compatibility impact: 10.12–10.15 are published as **best-effort** because no
  hosted CI runner for them is known (R-10 open; treated as absent until
  answered). Stated honestly rather than claimed.
- Explicitly not done: the macOS build-and-test CI job. R-10 — whether hosted
  Intel macOS runners exist — is unresolved, and a workflow written against an
  unverified runner would be a claim about CI that CI does not support.
- Still open inside the decision: which TUI and terminal crates. Licence
  compatibility, deployment floor and East Asian double-width handling are
  verified together before committing to one.

---

# Process: the agentic SDLC

## SE-001 — Specification lives in versioned repository documents, not in a session prompt
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: the full specification is pasted into each agent session.
- Change: the specification lives in `docs/spec/`, tiered by authority; a work
  order in `prompts/` points at it.
- Rationale: a ~9,000-word prompt is re-pasted, drifts, cannot be diffed and
  cannot be reviewed. Committed specs are versionable, greppable, loadable
  file by file, and let an agent read only the part it needs.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-002 — Requirements tiered by authority
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review; scope corrected
  2026-09-02 / 00-REPO-READINESS-PASS
- Original assumption: every statement is equally binding, and the agent "may
  improve anything" as long as it logs the change. Later, the tier table said
  Tier 3 was "MUST in `02..08`, `11`" while `09` and `10` declared themselves
  Tier 3.
- Change: five tiers — invariants (unchangeable), charter (human decision),
  MUST, SHOULD, HYPOTHESIS — with a stated precedence order. Tier 3 is every
  MUST in `docs/spec/02` to `11`; the tier table says so in every copy.
- Rationale: uniform emphasis makes agents either literal-minded about
  incidentals or liberal with safety rules. Explicit tiers let an agent take
  real design latitude on the parts that deserve it while leaving the safety
  core alone. A table that omitted two chapters let their MUSTs be changed
  without an entry.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-017 — Passes are roles, never model names
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review
- Original assumption: the specification named specific models for specific
  passes.
- Change: passes are described by role — foundation, architecture review,
  focused implementation. The person starting a session decides who runs it.
  The same rule governs subagent files: `model: inherit`, with the choice made
  at invocation by task weight (SE-033).
- Rationale: model names date quickly and end up embedded in a repository that
  outlives them; the useful content was always the role.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-024 — Orientation documents: `AGENTS.md` canonical, `CLAUDE.md` a thin layer, one operator runbook
- Status: accepted
- Date / Pass: 2026-09-02 / agentic SDLC review; extended 2026-09-02 /
  00-REPO-READINESS-PASS
- Original assumption: `AGENTS.md` is the orientation document and Claude Code
  will read it. Later, a separate `docs/agentic-sdlc/claude-code-setup.md`
  restated large parts of `11-agentic-sdlc.md` and `.claude/README.md`, and
  `AGENTS.md` paraphrased eight invariants.
- Change: `AGENTS.md` remains canonical and tool-neutral; `CLAUDE.md` imports
  it with `@AGENTS.md` and adds only Claude Code specifics; both under 200
  lines; neither restates the specification — `AGENTS.md` points at the
  invariants instead of paraphrasing them. Extended: the operator runbook is
  `.claude/README.md` alone — prerequisites, plugins, workflow activation,
  running a pass, escape hatches — and `docs/agentic-sdlc/` was removed;
  `08-distribution-and-project.md` points at `11` for the memory-file rule
  instead of restating it. Claude Code's automatic per-repository memory is
  disabled in project settings (`autoMemoryEnabled: false`), and
  `CLAUDE.local.md` is git-ignored. Path-scoped rules (`.claude/rules/*.md`
  with `paths:` frontmatter) are a SHOULD for once `src/` exists: they are
  the documented way to load the relevant chapter when a path is touched,
  and are not written before the paths exist.
- Rationale: Claude Code reads `CLAUDE.md`, not `AGENTS.md`, so the
  specification's requirement was not going to take effect; making
  `CLAUDE.md` canonical would tie the project to one tool. Three files saying
  "this is not a sandbox" in three wordings, and a second copy of Tier 1 read
  more often than the original, are drift waiting to happen. Auto memory is
  an undated recollection carried between sessions outside the repository —
  the fourth source the working protocol forbids.
- Evidence: Claude Code `memory` documentation, read 2026-09-02: `CLAUDE.md`
  is read at project scope; `@path` imports resolve relative to the importing
  file; the page now documents the `@AGENTS.md` pattern; auto memory is on by
  default and `autoMemoryEnabled` is settable from project settings
  (`research/tooling/2026-09-02-memory-settings-plugins.md`). The readiness
  pass's baseline audit, indexed in `passes/00-REPO-READINESS-PASS-report.md`,
  listed the duplicated passages by line.
- Architectural impact: none.

## SE-025 — Agent boundaries enforced by configuration and hooks, re-verified against current documentation
- Status: accepted
- Date / Pass: 2026-09-02 / agentic SDLC review; corrected 2026-09-02 /
  00-REPO-READINESS-PASS
- Original assumption: the work order told the agent not to run mutating
  provider commands, not to elevate, and not to touch the developer's real
  package managers. Nothing checked.
- Change: `.claude/settings.json` denies all elevation, every mutating `brew`
  and `port` subcommand, writes into either provider's prefix, shell
  configuration and launch machinery, toolchain switching, raw network tools
  and `git push`; allows the read-only provider subcommands research depends
  on; sets `disableBypassPermissionsMode`; sets the environment that keeps
  provider invocations quiet. Two `PreToolUse` hooks add a second layer for
  absolute paths, environment prefixes and wrappers. Corrections from
  re-verification: the prefix deny rules are written `Edit(//opt/local/**)`
  because a single leading slash anchors at the settings file, so the
  original rules matched nothing; the guards fail closed on unreadable input,
  not only on a missing parser; the path-stripping normalisation covers
  `doas`, `chsh`, `rustup` and `cargo`; `Bash(arch*)` became `Bash(arch)`
  because `arch` is not a wrapper the permission system strips; the
  `SessionStart` hook matches every source including `fork` and injects only
  what no file the agent will open already says; the interactive CI workflow
  enumerates its tools; the no-mutation grep in the `Stop` gate no longer
  discards a hit because the file's *path* contains "plan" or "test".
  `.claude/hooks/test-hooks.sh` asserts every decision above from fixture
  inputs and runs in CI. From the security review of the same pass: the
  Bash guard strips quotes and backslashes and treats grouping and
  substitution characters as separators before matching, accepts options
  before a subcommand, and covers `su`, "with administrator privileges",
  `cargo +toolchain install`, `rustup install`, system installers
  (`xcode-select --install`, `softwareupdate -i`, `installer -pkg`) and any
  writing verb aimed at a dotfile, not only a redirect; the path guard
  canonicalises the target (symlinks, `..`, `//`) and refuses to write
  through a symlink; the fail-closed helper sets a variable in the caller's
  shell so its `exit 2` does not depend on the caller's `set -e`; no hook
  writes a predictable file under `/tmp`; the gate's cargo invocations use
  `--locked` and its no-mutation grep scans every path under `src/` and
  `build.rs` with a whole-string literal match; skills no longer pre-approve
  `find`; `shfmt` is allowed only in diff/list mode; `RUST_BACKTRACE` is not
  set (home paths in pasted panics); the `@claude` workflow neither builds
  nor tests the pull-request head and disables the `Stop` gate for that
  reason, and the validation workflow runs the gate in full. Deliberately
  not done: pinning the two GitHub Actions to commit SHAs — there is no
  network path in a session to read the SHAs honestly; it is the first
  step of activating the workflows (`.claude/README.md`). Two decisions
  about what the boundary does *not* do: `permissions.defaultMode` is deliberately unset, because plan mode
  gates on a human approving the plan and the owner runs passes
  autonomously; and Claude Code's sandbox — the documented OS-level layer
  that constrains subprocesses regardless of the agent's decision — is a
  SHOULD for the Foundation Pass to evaluate against the real build rather
  than enabled blind. `HOMEBREW_NO_INSTALL_FROM_API=0` was removed from the
  environment block: it set the documented default and explained nothing;
  R-1 records that the quieting variables in place do not stop `brew info`
  from tapping on an unknown name, and the suppressing setting is that
  research item's to find.
- Rationale: a safety rule that depends on good intentions is not a safety
  rule — the argument behind INV-02 and behind making the no-mutation
  boundary `grep`-checkable (SE-020), applied to the process that builds the
  product. The corrections are the argument for `/tooling-recheck`: four of
  them were in configuration written the same day against the same
  documentation, and none would have been noticed without a test that feeds
  the hooks the inputs the rules are supposed to catch. Raw network tools are
  denied because research fetches go through `WebFetch` on allow-listed
  provider domains, which is auditable and keeps the etiquette posture
  (SE-019) mechanical.
- Evidence: Claude Code `permissions`, `hooks` and `settings` documentation,
  read 2026-09-02 — rule anchoring, wrapper stripping, compound-command
  splitting, the `PreToolUse` decision contract, `defaultMode` values and
  scope, the `sandbox` keys — notes in
  `research/tooling/2026-09-02-hooks-permissions-ci.md` and
  `research/tooling/2026-09-02-memory-settings-plugins.md`; `test-hooks.sh`
  output, 34 of 34 cases, 2026-09-02;
  `research/R-1-homebrew-read-interfaces.md`.
- Compatibility impact: the hooks require `jq` or `python3` on a
  contributor's machine, and say so when neither is present.

## SE-026 — A definition-of-done gate that blocks, rather than a checklist that asks
- Status: accepted
- Date / Pass: 2026-09-02 / agentic SDLC review; corrected 2026-09-02 /
  00-REPO-READINESS-PASS
- Original assumption: the acceptance list in `10-first-pass-work-order.md` is
  read and honoured by the agent at the end of a pass.
- Change: `.claude/hooks/stop-gate.sh` runs on every turn end and refuses to
  let the turn finish while an objective check fails — link resolution,
  configuration validity, the `grep`-checkable no-mutation boundary, and the
  crate's format, lint and tests once it exists. Only machine-checkable
  failures block; judgement calls warn, and warnings reach the human as a
  system message because exit-0 stderr from a hook reaches nobody and
  `additionalContext` on `Stop` would prolong the turn. It has a documented
  escape hatch (`MPB_SKIP_STOP_GATE=1`, and Claude Code's `disableAllHooks`)
  whose use the report must disclose. Corrected: Claude Code stops honouring
  a `Stop` hook after eight consecutive blocks; the gate reads
  `stop_hook_active`, says when it is re-entering, and the specification
  states that a turn ending under the cap ended with the gate failing.
  `07-quality-and-testing.md` requires the same checks reachable from CI,
  locally and from this gate, out of one implementation.
- Rationale: "demonstrate, do not assert" was already the standard the work
  order set for the product. An agent reporting its own compliance is the
  same structure as an install command reporting its own success, which
  `04-migration-and-safety.md` refuses to accept as verification.
- Evidence: Claude Code `hooks` documentation, read 2026-09-02 — exit code 2
  on `Stop` prevents the turn ending; `stop_hook_active`;
  `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`; output-delivery rules per event
  (`research/tooling/2026-09-02-hooks-permissions-ci.md`).
- Risk accepted: a gate that is wrong gets disabled wholesale. Mitigated by
  keeping blocking checks objective and by the disclosed escape hatch.

## SE-027 — Read-only reviewers, a researcher, and procedural skills
- Status: accepted
- Date / Pass: 2026-09-02 / agentic SDLC review; extended 2026-09-02 /
  00-REPO-READINESS-PASS
- Original assumption: one agent does the work and reviews it, with an
  architecture-review pass as the only independent check.
- Change: four subagents — `provider-researcher` and the reviewers
  `spec-guardian`, `security-reviewer`, `docs-auditor`. Reviewers are
  read-only twice over: `disallowedTools` covering writes, and
  `permissionMode: plan`, the harness's read-only mode for the file tools.
  Whether it also closes the path through `Bash` is unverified — the
  security reviewer of the readiness pass, running under that setting,
  reported creating scratch files through `Bash` — so `11` names `Bash` as
  the documented gap until `/tooling-recheck` settles it. Skills:
  `/spec-evolution`, `/adr`, `/research-note`, `/decision-pending`,
  `/preflight`, `/pass-report`, and — added by the readiness pass —
  `/tooling-recheck`, which encodes the every-pass re-verification of the
  Claude Code facts as parallel subagent work. Skill bodies place their
  arguments with `$ARGUMENTS`. The SHOULD that side-effecting skills set
  `disable-model-invocation: true` is **reversed**: the procedural skills stay
  model-invocable because passes run autonomously and the agent is the one
  that must record the change or write the report when the need arises.
- Rationale: an independent context finds what a working one has stopped
  seeing, at the scale of a commit as well as a pass. Reviewers that cannot
  edit produce an argument instead of a diff, and the argument is the
  artifact. Skills exist because the recording procedures are exactly what
  gets done sloppily at the end of a long session.
- Evidence: Claude Code `sub-agents` and `skills` documentation, read
  2026-09-02 — the frontmatter contract, `permissionMode`, that
  `disable-model-invocation: true` leaves a skill reachable only by the user,
  and that an argument without a placeholder is appended as a trailing line
  (`research/tooling/2026-09-02-subagents-skills.md`).
- Deliberately deferred: the contributor recipes named in
  `08-distribution-and-project.md` are not written. A skill for a procedure
  that does not exist reads as authoritative and is wrong in ways nobody
  notices. They are a foundation-pass deliverable, written together with the
  `CONTRIBUTING.md` sections they mirror.

## SE-028 — The pass system, numbered in execution order
- Status: accepted
- Date / Pass: 2026-09-02 / agentic SDLC review; renumbered 2026-09-02 /
  00-REPO-READINESS-PASS
- Original assumption: `prompts/` holds one work order; later passes are
  described narratively in `10-first-pass-work-order.md`. Later, the
  readiness pass's own id `00` collided with the foundation pass's `00`, and
  work orders carried a second numbering (`P0`, `P1`, `P2-n`).
- Change: `prompts/README.md` states the convention and a seven-section shape;
  `prompts/TEMPLATE-PASS.md` is the template; every pass has a real work
  order that points at the specification's acceptance list rather than
  restating a subset of it; `.claude/current-pass` names the active one;
  `docs/passes/` holds the reports, which index while this file records.
  Renumbered: `00` repository readiness, `01` foundation, `02` architecture
  review, `03` focused implementation — the number is the order of execution
  and nothing else; the `P`-labels were dropped so there is one numbering;
  report files use the same ids. `10-first-pass-work-order.md` points at
  `prompts/` instead of restating the later passes.
- Rationale: the argument of SE-001 applies to every pass, not only the
  first: a pass with no written work order has no hard boundary. Two passes
  numbered `00` and two numbering systems for the same four passes is the
  kind of drift the convention exists to prevent, and cheapest to fix while
  nothing references the reports yet.
- Evidence: `prompts/00-REPO-READINESS-PASS.md` §5 step 7; a grep for every
  pass-id and `P`-label reference, 2026-09-02, all updated in the same
  change.

## SE-029 — The agentic SDLC is a specification chapter, and its tooling facts are dated data
- Status: accepted
- Date / Pass: 2026-09-02 / agentic SDLC review
- Original assumption: how agents work on the repository is implicit in
  `07-quality-and-testing.md`'s "architecture for agents" section.
- Change: `11-agentic-sdlc.md`, Tier 3, specifying the memory files, the
  permission boundary and what each denial defends, the obligation of each
  hook event, the required subagents and skills, delegation, the pass
  protocol, the CI posture, and an agent's definition of done. Claude Code
  field names, hook events and rule syntax carry a read date and are
  re-verified at the start of every pass — the treatment INV-20 gives
  provider facts. It states plainly that the configuration is not a sandbox:
  tripwires for the accident, not a cage for an adversary.
- Rationale: `07` drew the architectural consequence of "agents will maintain
  this"; nothing drew the process consequence, so the process lived in one
  prompt and disappeared when that prompt was superseded. Tier 3 means it can
  be improved with evidence like anything else. Treating tooling facts as
  stable is how a configuration silently stops enforcing anything —
  demonstrated by SE-025's corrections.
- Architectural impact: none on the product.
- Evidence: design judgement; no external fact involved. The rationale is the
  argument, and the entry stands or falls on it.

## SE-030 — Foundation pass starts with version control, and ends with the agent machinery caught up
- Status: accepted
- Date / Pass: 2026-09-02 / agentic SDLC review
- Original assumption: the foundation pass begins with research; git
  initialisation was mentioned only in DP-007.
- Change: `10-first-pass-work-order.md` gains step 0 — initialise git locally
  and commit the existing specification before changing anything — and step
  21, which requires the `Stop` gate, the skills and the deny rules to catch
  up with the code. The readiness pass worked uncommitted and left the
  repository without `.git`, by the owner's instruction: version control
  starts with the first code.
- Rationale: without step 0, a pass's first commit contains both the
  pre-existing specification and the agent's own changes, and the history
  stops being reviewable — which the working protocol requires it to be.
  Without step 21 the gate quietly stays behind the code it is meant to check.
- Evidence: DP-007; `prompts/00-REPO-READINESS-PASS.md` §3.

## SE-032 — Documentation history policy: consolidate, not accumulate
- Status: accepted
- Date / Pass: 2026-09-02 / 00-REPO-READINESS-PASS
- Original assumption: `11-agentic-sdlc.md` treated this file as an append-only
  log in which no material change is ever removed, so that silent deviation
  stays impossible.
- Change: this file and `docs/adr/` hold the **current rationale per topic**,
  not the chronology of how it was reached. Entries on one topic are merged;
  content a later decision superseded or reverted is dropped rather than
  annotated; a **rejected** change keeps its entry with `Status: rejected`.
  SE identifiers are stable — a merged entry keeps one of the absorbed ids and
  cross-references to the others are updated in the same change. What does
  not change: every Tier 1–3 change still lands with a traceable record of
  what changed and why, in the same change that makes it.
- Rationale: the principle behind the old rule — silent deviation is the one
  unrecoverable error — is about *traceability*, and a per-topic record of the
  current reasoning preserves it. What the append-only form added was
  chronology, which pre-code is worth little and costs a lot: every agent
  reads this file every pass, and it had grown to a third of the whole
  specification with entries that contradicted later ones (the language
  decision was told three times; a gate was announced and lifted the same
  day). Chronology that matters will be git history once the repository is
  under version control (01-FOUNDATION-PASS, step 0). Recorded here, once,
  before being applied — the condition the work order set.
- Evidence: `prompts/00-REPO-READINESS-PASS.md` §5 step 3 (the owner's
  instruction); `wc -c docs/spec-evolution.md` = 36,286 bytes against 271,586
  for the repository's entire documentation, measured 2026-09-02 before
  consolidation. Applied in the same pass: SE-013 and SE-031 absorbed into
  SE-022; the readiness pass's changes folded into SE-002, SE-008, SE-024,
  SE-025, SE-026, SE-027 and SE-028 rather than added as new entries.
- Architectural impact: none on the product. `11-agentic-sdlc.md` and the
  `/spec-evolution` skill describe the convention.

## SE-034 — Two Tier 1 wording corrections, made under explicit owner authorization
- Status: accepted
- Date / Pass: 2026-09-02 / 00-REPO-READINESS-PASS (owner authorization
  given in the session that ran the pass)
- Original assumption: `01-invariants.md` said "See `docs/research/`" as
  INV-10's evidence pointer and gave `TestInvariant07_...` "or the
  equivalent in the chosen language" as the test-naming example.
- Change: INV-10 points at F-2 in `09-research-checklist.md`, where the
  MacPorts FAQ evidence actually lives with its read date; the test-naming
  sentence names Rust's convention (a test function beginning
  `invariant_07_`) and leaves the exact form to the foundation pass. No
  invariant changed meaning.
- Rationale: Tier 1 is not changeable by an agent, so the readiness pass
  listed both items for the owner instead of fixing them. The owner
  authorized exactly these two edits. Recording them here keeps the rule
  intact: a Tier 1 edit is traceable to a human decision, never to an
  agent's judgement.
- Evidence: the owner's confirmation in the readiness-pass session,
  2026-09-02, indexed in `passes/00-REPO-READINESS-PASS-report.md`; F-2 in
  `09-research-checklist.md` (<https://trac.macports.org/wiki/FAQ>, read
  2026-09-02); ADR-0002 for the language.
- User impact: none.
- Architectural impact: none.
- Compatibility impact: none.

## SE-033 — Delegation to subagents is the default, and model choice is by task weight
- Status: accepted
- Date / Pass: 2026-09-02 / 00-REPO-READINESS-PASS
- Original assumption: subagents existed (SE-027) and the work orders said
  "run independent research items in parallel", but nothing said what the
  main session does itself and what it hands off, or how the model for a
  subagent is chosen.
- Change: `11-agentic-sdlc.md` ("Delegation by default"): read-only research
  and exploration are delegated — external documentation, research items,
  repository sweeps, audits, diff reviews; independent items run in parallel;
  the main session is for synthesis, decisions and the edits that need the
  whole repository in view; a subagent's report is read and decided on, not
  relayed. Model choice is by **task weight** — the judgement a task needs
  times the cost of its being wrong — decided at invocation using whatever
  aliases the tool offers, never written into a checked-in file (SE-017).
  Summarised in `CLAUDE.md`.
- Rationale: a long context stops seeing what it has read many times, and
  every byte a tool returns costs reasoning capacity for the rest of the
  session. A subagent starts with the project's orientation and none of the
  accumulated context, which is what makes it a good researcher and a good
  reviewer. Fixing the model per file would date the repository the way
  SE-017 refused to; task weight is the criterion that survives model churn.
- Evidence: Claude Code `sub-agents` documentation, read 2026-09-02 — own
  context window per subagent, background and concurrent execution, the
  `CLAUDE.md` hierarchy inherited (`research/tooling/2026-09-02-subagents-skills.md`). The readiness pass itself: three
  documentation verifications, one baseline audit and three reviews
  delegated in parallel, with the main session holding only what it edited.
- Architectural impact: none on the product.
