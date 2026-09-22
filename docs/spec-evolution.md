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
- Date / Pass: 2026-09-02 / pre-implementation review; extended 2026-09-02 /
  01-FOUNDATION-PASS
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
- Extended 2026-09-02 (01-FOUNDATION-PASS), after F-1 and R-2: the "no new
  Intel bottles" half of the fact is now recorded **as Homebrew's stated
  policy, attributed**, not as a description of the artifacts — because it is
  not one. Intel bottles were still published in August 2026 (`htop` 16th,
  `sqlite` 25th, `cmake` 26th). `09-research-checklist.md` F-1 carries the
  attribution plus the measured census: of 8586 core formulae, 7808 (90.9%)
  have a usable bottle on Intel macOS 26 against 8420 (98.1%) on Apple Silicon,
  and only 79 carry an Intel `tahoe` bottle. The honest characterisation is
  **frozen at `sonoma` and eroding**, not "stopped".
- Rationale for the extension: the product must not tell an Intel user "no
  bottle exists" while 90.9% of core still pours — that is a claim they can
  falsify in one command, and the trust cost is the whole product. The real
  argument is the *gap and its trend*, which R-2 makes computable per package
  and re-runnable per release. Quoting a policy as an observation is also
  exactly the failure INV-20 exists to prevent, in the one fact the project
  cares most about.
- Evidence: <https://docs.brew.sh/Support-Tiers>, `last_review_date`
  2026-08-27, read 2026-09-02. Census and the commits contradicting the literal
  reading: `docs/research/F-1-homebrew-support-tiers.md` E3/E4, cross-checked
  against Homebrew's own API cache in
  `docs/research/R-2-homebrew-artifact-availability.md` E5/E8, both 2026-09-02.
- User impact: high. Compatibility impact: policy weighting on Intel machines.
  The census numbers are a snapshot of a deteriorating situation and are
  re-measured, never quoted forward.

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
- Date / Pass: 2026-09-02 / pre-implementation review; extended 2026-09-02 /
  00-REPO-READINESS-PASS and 2026-09-02 / 01-FOUNDATION-PASS
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
- Extended 2026-09-02 (01-FOUNDATION-PASS), on R-1, R-2, R-3 and R-4, in four
  places:
  1. `SOURCE_BUILD_REQUIRED` now carries a required **reason** from a closed set
     — `NO_ARTIFACT_FOR_PLATFORM`, `NON_DEFAULT_OPTIONS`,
     `LICENCE_NOT_REDISTRIBUTABLE`, `FORCED_BY_CONFIGURATION`. One state was
     conflating causes whose user advice and durability differ: two are the
     user's own reversible choice, one never resolves for that version, and one
     moves with provider policy.
  2. Two determinations are stated as **not offline-answerable**, degrading to
     `UNKNOWN` rather than to a negative: the "no artifact anywhere" versus "not
     for this platform" split under Homebrew's default API mode, where both are
     represented identically as an empty `bottle` object; and
     `BINARY_AVAILABLE` under MacPorts, which is never assertable offline at
     all, though several `SOURCE_BUILD_REQUIRED` reasons are.
  3. The MacPorts side of the symmetry stops being an assumption: the archive
     filename encodes the canonical variant set, so a non-default variant names
     a file that was never built, and only binary-distributable ports are staged
     for upload, decided by the port's licence *and* its whole dependency tree.
  4. The starting policy's MacPorts placeholder in
     `03-policy-and-recommendations.md` is replaced with the named support
     states — installer availability per macOS version (2.12.6: 10.12 upward)
     and builder availability per `(version, architecture)` pair (`x86_64`
     10.6–10.15, 11–15, 26; `arm64` 11 upward) — and `replaced_by` is named as
     the deprecation signal, distinct from `obsolete`.
  Also under this entry's symmetry topic: `03`'s outcome vocabulary now shows
  both forms — the provider-named spelling a user reads, and the implementation
  form `KEEP{provider}` / `MIGRATE_TO{provider}` / `KEEP_BOTH{primary}`. The
  vocabulary was the last place where the two directions read as two things
  rather than one value with a parameter, which is how a direction acquires a
  rule the other lacks (INV-17). Design judgement; no external fact involved.
- Evidence for the extension: `docs/research/R-2-...md` findings 5 and 11 and
  `docs/research/R-1-...md` finding 6 (the (a)/(b) collapse, and 857 of 8586
  formulae carrying no bottle for this tag);
  `docs/research/R-4-macports-binary-archives.md` findings 2–7 and 9–12
  (filename tuple, licence gating via `distributable_lib.tcl`, builder roster,
  the offline asymmetry, the 10.12 installer floor);
  `docs/research/R-3-macports-read-interfaces.md` finding 8 (`replaced_by`
  versus `obsolete`). All read or observed 2026-09-02.
- Architectural impact: the assessment type is provider-neutral, and the
  reason enum is part of it rather than of the adapters — the same four causes
  are expressible for both providers.
- Compatibility impact: an offline `analyze` reports strictly less about
  MacPorts availability than an online one, and says so; it never guesses
  upward.

## SE-005 — Cross-provider contamination has a named mechanism, and an invariant to prevent it
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review; extended 2026-09-02 /
  01-FOUNDATION-PASS
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
- Extended 2026-09-02 (01-FOUNDATION-PASS), after F-2, which could not read the
  FAQ this entry originally cited and re-sourced the fact:
  1. The hazard is now cited from the **MacPorts Guide's trace-mode text**
     (§3.1.12), which names "conflicts caused by files installed by other ports
     or in common system locations, such as `/usr/local`" and prescribes
     `port -t`. The narrower "autotools and gcc probe `/usr/local`" wording —
     this entry's original phrasing — is **plausible but uncited**, and
     `09-research-checklist.md` F-2 now says so in those words rather than
     repeating it as fact.
  2. The other direction is established and is **not symmetric in risk**.
     Homebrew mitigates in its compiler shim, always on for every source build:
     `shims/super/cc` drops any `-I`/`-L` flag under `/opt/local`. MacPorts'
     equivalent, trace mode, is opt-in. So the residual exposure toward Homebrew
     is `PATH`-resolved *tools*, not headers and libraries, and `doctor`'s
     wording says which direction is worse instead of presenting a false
     symmetry.
  3. **`brew doctor` has no MacPorts, Fink or `/opt/local` check at any version
     examined** — none at HEAD, none at tags 1.9.0, 2.7.0 or 3.6.0. A
     recollection that it warns about MacPorts on `PATH` is of something that is
     not there. `04-migration-and-safety.md` now requires MacPortsBrew's own
     `doctor` to implement that check and forbids delegating it.
- Evidence: MacPorts Guide §3.1.12,
  <https://guide.macports.org/chunked/using.html#using.port.install>, read
  2026-09-02, cross-checked against `macports/macports-guide`
  `guide/xml/using.xml`; Homebrew `Library/Homebrew/shims/super/cc` at
  `fd18738`, and `Library/Homebrew/diagnostic.rb` grepped at HEAD and at three
  tags — all 2026-09-02, recorded in
  `docs/research/F-2-usr-local-interference.md` E-3 to E-6. The FAQ itself was
  unreachable that day (Anubis bot mitigation on `trac.macports.org`), which is
  why the original citation is retired rather than re-confirmed.
- Architectural impact: the process executor owns environment construction;
  no ad-hoc invocations. `doctor` owns the cross-provider checks in both
  directions and delegates neither.

## SE-006 — Package classes; stateful and service packages are never auto-migrated
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review; extended 2026-09-02 /
  01-FOUNDATION-PASS
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
- Extended 2026-09-02 (01-FOUNDATION-PASS), after R-8: a package carries a
  **set** of classes, not one, with a fixed precedence —
  `STATEFUL` · `SERVICE` · `GUI_APP` · `THIRD_PARTY_SOURCED` >
  `PROVIDER_INTERNAL` > `RUNTIME` > `SIMPLE`. Default handling is the strictest
  any member class demands, and every member is displayed.
- Rationale for the extension: the original table read as one-of, and the
  counter-examples are ordinary packages, not corner cases. `php@8.4` is a
  `RUNTIME` (versioned language family, keg-only for that reason) *and* a
  `SERVICE` (php-fpm, with a plist in its keg); `postgresql@17` is `STATEFUL`
  *and* `SERVICE`. Under a one-of model, classifying `php@8.4` as `RUNTIME`
  suppresses the `MANUAL_REVIEW` its service block requires — the headline
  safety promise silently failing on a package a user is likely to have. The
  precedence is what makes INV-19 undefeatable by a later, gentler
  classification.
- Evidence: `docs/research/R-8-stateful-and-service-packages.md` findings 1,
  13 and 14, observed 2026-09-02 on this machine (`service != null` in
  `brew info --json=v2`, plist present in the keg, `keg_only_reason`
  `:versioned_formula`); the MacPorts half of finding 12 is cited from
  Portfiles.
- User impact: high — prevents a class of data loss.
- Architectural impact: classification is part of inventory, not of planning,
  and the class field is a set with a total order over its members rather than
  an enum.

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
- Date / Pass: 2026-09-02 / pre-implementation review; extended 2026-09-02 /
  01-FOUNDATION-PASS
- Original assumption: "never run entirely as root; elevate per operation".
- Change: the design assumes MacPorts operations generally require elevation
  and Homebrew's generally do not, with R-5 verifying the per-operation facts;
  the plan enumerates every elevating step; credentials are never cached;
  analysis and dry-run never elevate — including no `sudo` prompt.
- Rationale: the asymmetry between the two providers is a real UX and safety
  issue that symmetric language hid. "Dry run never prompts for a password" is
  a testable property users can rely on.
- Extended 2026-09-02 (01-FOUNDATION-PASS), on R-5 and R-3, in two places:
  1. **Elevation varies per package, not only per provider.** The old sentence
     read as though the answer followed from which provider a step used. It does
     not: a Homebrew cask whose `pkg` artifact or `installer script:` carries
     `sudo: true` elevates on the provider that "normally does not", and the
     Homebrew services subcommand at system level elevates even to *list*, so an
     unprivileged services inventory is necessarily incomplete and must say so.
     The plan therefore asks the adapter **per step**. Also recorded, because it
     is a hard provider-side error rather than an impoliteness: Homebrew refuses
     to run as root outside a short exemption list, so an executor that ever
     wraps a Homebrew invocation in elevation fails.
  2. **Quieting a provider is not only an environment concern.**
     `04-migration-and-safety.md`'s "Provider invocations set the environment"
     was Homebrew-shaped. Homebrew is quieted by environment
     (`HOMEBREW_NO_AUTO_UPDATE` does the offline work, with
     `HOMEBREW_NO_ANALYTICS` and `HOMEBREW_NO_ENV_HINTS`); MacPorts is quieted
     by argv — `-q`, which implies `-N` — plus never invoking the two
     network-touching update actions. No `MACPORTS_*` variable serves the
     purpose. The MUST now says "environment *and flags*" and names both
     mechanisms, so an adapter author does not go looking for a variable that
     does not exist.
- Evidence: `docs/research/R-5-privilege-requirements.md` findings 1, 5, 6 and
  14 with its per-operation table — observed for Homebrew as uid 501, with the
  MacPorts rows cited and flagged there as citation-only;
  `docs/research/R-3-macports-read-interfaces.md` finding 12;
  `docs/research/R-1-homebrew-read-interfaces.md` finding 2 and
  `docs/research/R-2-homebrew-artifact-availability.md` finding 11 for which
  Homebrew variable actually suppresses the network. All 2026-09-02.
- Architectural impact: elevation is a property the adapter answers per
  operation, so the plan's elevation list is derived, never inferred from the
  provider id.

## SE-035 — The read contract returns installed rows, and ownership names its mechanism
- Status: accepted
- Date / Pass: 2026-09-02 / 01-FOUNDATION-PASS
- Original assumption: `inventory()` returned "everything installed", read as
  one row per package name; and the ownership MUST said to "use the provider's
  own record of user intent" while leaving the mechanism to research.
- Change: two corrections to the provider read contract in
  `02-domain-model.md`.
  1. `inventory()` returns installed **rows** — `(name, version, revision,
     variant/option set, active?)` — with several rows per name permitted.
  2. Ownership names its mechanism per provider, and `ORPHAN_DEPENDENCY` is
     given one definition. Homebrew: `installed_on_request`, and nothing else —
     `installed_as_dependency` does not exist in Homebrew 6.0.21's JSON and is a
     legacy receipt field, the cask JSON carries no intent field so a cask's
     ownership is `UNKNOWN` (R-12 corrects the absolute: Homebrew records it on
     disk, outside any supported CLI), and `brew leaves` answers a different
     question.
     MacPorts: the registry `requested` flag through the
     `requested`/`unrequested`/`leaves`/`rleaves` pseudo-portnames, a stored
     flag rather than a graph inference, user-mutable and therefore recording
     intent *as last asserted*. `ORPHAN_DEPENDENCY` means unrequested with no
     dependents — the `leaves` definition; `rleaves` is a wider set that may be
     reported but never silently substituted.
- Rationale: both were modelling gaps that produce wrong answers rather than
  ugly ones. Several versions of a port installed at once with exactly one
  active is the *normal* state of a MacPorts machine — `port upgrade` leaves the
  old one behind by default — so a one-row-per-name inventory is wrong about
  what is installed and about disk usage on the provider this project exists to
  reach; Homebrew has the same shape in a rack of kegs, where reading the first
  entry of `installed[]` answers about the oldest one. And "use the provider's
  record of intent" without naming it lets two implementations disagree: on one
  machine the three candidate answers were 47 requested, 49 leaves and 44 both,
  and one of the plausible field names is a trap that silently returns nothing
  on current Homebrew.
- Evidence: `docs/research/R-3-macports-read-interfaces.md` findings 3, 4 and 5;
  `docs/research/R-1-homebrew-read-interfaces.md` findings 4, 5 and 7 —
  `installed_as_dependency` present in 70 of 192 receipts and none written by
  Homebrew 6.x, absent from `--json=v2` entirely; 12 formulae with more than one
  keg on the machine observed. Both 2026-09-02.
- User impact: inventory counts and "is this installed" answers are correct on a
  normal MacPorts machine; a cask is never reported as user-requested on
  evidence that does not exist.
- Architectural impact: `InstalledPackage` is a row type, and the aggregation to
  the per-name unit the user sees happens above the adapter, not inside it. The
  mechanism and its confidence are recorded with each classification.
- Compatibility impact: none — no released surface exists.

## SE-036 — Package primary resolved: four layers, and the shim is the last of them
- Status: accepted
- Date / Pass: 2026-09-02 / 01-FOUNDATION-PASS
- Original assumption: a **HYPOTHESIS** in
  `05-environment-and-primary-provider.md` — a MacPortsBrew-owned shim
  directory early on `PATH` — with five alternatives listed for research, and a
  flat MUST to "prefer a provider-native mechanism … wherever the provider
  offers one that does the job".
- Change: the hypothesis is resolved into a layered answer, each layer used only
  where it is the least invasive one that can express the request: **report
  only** by default; **recommend the versioned executable name** where one
  exists; **delegate an intra-MacPorts version choice to `port select`**,
  proposing the command rather than running it, because the write goes into the
  MacPorts prefix and keeping primary selection off the elevating path is what
  keeps analysis and dry-run unprivileged as INV-06 requires; and a
  **MacPortsBrew-owned shim directory** for a genuine cross-provider primary.
  Three hard constraints come with the shim: it is never placed inside a
  provider's prefix and in particular never in `/usr/local/bin`; it targets the
  provider's stable path (`${prefix}/opt/<formula>/bin/<exe>`,
  `${prefix}/bin/<exe>`) and never the versioned keg; and reordering
  `/usr/local/bin` against `/opt/local/bin` is forbidden as a primary mechanism.
  The "prefer a provider-native mechanism" MUST is qualified: the preference
  applies where a native mechanism **can express the requirement**. The
  implementation stays deferred.
- Rationale: neither provider's mechanism can express a cross-provider choice,
  by construction — `mportselect` joins both link source and target with
  `${prefix}`, and `brew link` symlinks into Homebrew's prefix — so the MUST was
  satisfiable only vacuously for the case that matters, while reading as an
  instruction to reach for `brew link` or `port select` anyway. Left unqualified
  it would have sent a future agent down a path that cannot work. The placement
  rule is not fastidiousness: Homebrew cannot see foreign symlinks in its own
  prefix (`--unbrewed` reports regular files only, 0 of 1048 symlinks) and
  `brew link --overwrite` deletes files already there, so a shim in
  `/usr/local/bin` would be invisible to the provider's checks and deletable by
  a routine operation. Layering also keeps the default at the only option with
  no failure mode, and puts the two zero-cost options ahead of the one that owns
  state.
- Evidence: `docs/research/R-6-primary-provider-mechanisms.md` findings 1–3, 5,
  8 and 10–14, with the criteria table `05` asked for; the `opt` symlink
  observed following an upgrade while the `Cellar` path did not; 62 of 1086
  entries in `/usr/local/bin` not placed there by Homebrew; 1024 names moved at
  once by a `PATH` reorder. All 2026-09-02.
- User impact: most users are advised, not modified — the two recommended
  layers change nothing on disk. A user who does take a cross-provider primary
  gets one clearly named directory and one clearly named `PATH` entry.
- Architectural impact: primary selection is a strategy with four
  implementations behind one interface, symmetric between directions (INV-17),
  and `doctor` validates shim targets on every run because neither provider's
  mechanism self-heals — `port select` never checks that its target exists.
- Compatibility impact: Tier 5 → Tier 3/4 for this section; nothing shipped
  depends on it yet.

---

## SE-054 — "No home directory" was two different situations, and treating them alike published a home path
- Status: accepted
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW, proposed and implemented
- Original assumption: `doctor --report` produces a file safe to attach to an
  issue, with home paths redacted. `redact_home(value, home: Option<&Path>)`
  returned the value unchanged when `home` was `None`.
- Change: `HomeRedaction { Known(PathBuf), Unresolved, Recorded }` replaces the
  `Option<&Path>` at every redaction site, threaded through `CaptureContext`,
  `SystemMachine` and the CLI `Context`. `Known` redacts the home and the user
  name as before. `Recorded` — a recording or a snapshot file — is the identity,
  as `None` used to be. `Unresolved` — a live run whose `HOME` is unset or
  relative — redacts by *shape*: `/Users/<name>` becomes `~` and the name is then
  redacted as a whole component, `Shared`, `Guest`, `Deleted Users` and
  `Localized` excepted. The diagnostic report gains a `redaction` field carrying
  `GROUNDED`, `HEURISTIC` or `AS_CAPTURED`.
- Rationale: `None` meant both "a replayed recording, redacted when it was
  captured" and "a live run whose home is unknown", and those need opposite
  treatment. Observed with `HOME` unset: 39 occurrences of the real home path,
  username and installed tooling included, in the file `06-cli-and-ux.md:134-137`
  invites the user to attach to an issue — routine under `cron`, `launchd` and
  some CI. The proposal offered "resolve it by a second mechanism **or** stamp
  the artifact"; what shipped is a third thing and both halves, and the reasons
  are worth keeping:
  - **A second mechanism was rejected.** Resolving the home without `HOME` means
    `getpwuid`, which needs `libc` and so an ADR under ADR-0004, or shelling out
    to `id -un`, which adds a probe to `SystemMachine` that every existing
    recording lacks — so every fixture would need recapturing and `--replay`
    would fail on all six until they were. The cost lands on the fixture corpus,
    which is the project's evidence base, to buy a case that is already handled
    conservatively.
  - **A stamp alone was not enough.** It tells the reader the file is unsafe
    without making it safer, and the reader who most needs the warning is the one
    who attaches the file without reading it.
  - **Three states, not a boolean.** A first attempt exposed `is_grounded() ->
    bool` and mapped `Recorded` to "grounded". That is false: a recording carries
    no record of *how* it was redacted, and a snapshot captured with no
    resolvable home was redacted heuristically — so `--from-snapshot ... --report`
    would have stamped a heuristically-redacted file as grounded, on exactly the
    path the leak came from. `AS_CAPTURED` names the unknown instead, per the
    domain rule that an unknown is never folded into its favourable neighbour.
- Evidence: reproduced and then re-measured to zero on this machine
  (`doctor --report` and `snapshot --out`, with and without `HOME`);
  `machine.rs:140-143`, `snapshot.rs`; 460 tests including
  `a_live_run_with_no_resolvable_home_still_redacts_by_shape`,
  `a_live_capture_whose_home_is_unresolvable_still_carries_no_home_path` and
  `invariant_15_a_replayed_or_snapshotted_run_never_redacts_against_this_host`;
  157 fixture-matrix checks unchanged. Finding F-05.
- User impact: the leak is closed for a home under `/Users`, and the file says
  which redaction produced it rather than looking identical either way.
- Architectural impact: the redaction site can now tell the two absences apart,
  which is what the defect was. `Context` keeps the real home separately from the
  redaction policy — the first locates the state directory, the second decides
  what may be written down, and conflating those was how one absent value came to
  answer two questions in the first place.
- Compatibility impact: the report gains a field. Additive, so `schemaVersion`
  stays 1 (`docs/schemas/README.md`); documented in `docs/schemas/doctor.v1.md`
  and noted in `CHANGELOG.md`, which that rule requires.
- **The order of audit and redaction is part of this entry, learned the hard
  way.** The first implementation redacted `PATH` and then handed the redacted
  entries to `path_audit::audit_path` together with the home. With the home
  unresolved there was nothing to expand `~` against, so every home-relative
  entry was classified `Relative` and never inspected — silently dropping the
  world- and group-writable checks, which are the executable-shadowing item in
  `04-migration-and-safety.md`, on exactly the runs this fix exists for, and
  reporting an absolute path as "not absolute" while doing it. Measured: the
  finding present with `HOME` set was absent with `HOME` unset. The audit now
  runs on the raw entries and each finding's `entry` is redacted afterwards, and
  `audit_path` no longer takes a home at all, so the round trip cannot return.
  That also closed a **pre-existing** bug of the same shape: a username outside
  the home (`/tmp/build/ada/bin`) was redacted to a `USER` path that does not
  exist, and stat'd, yielding a spurious `Missing` in place of a real finding.
- **Known residuals, deliberately retained.** `06-cli-and-ux.md:133-137` says
  *never* a home path or a user name. Two cases remain. A directory under
  `/Users` whose name contains a space — `/Users/Deleted Users/bob/...` — is left
  entirely alone, because the extracted name is the fragment `Deleted` and
  rewriting a fragment mangles the path while leaving the real name behind;
  macOS puts no live home there. And a live run whose home is **not** under
  `/Users` — a network mount, a relocated account, `/var/root` — is still
  published, because nothing in the process can recognise it as a home. The
  deviation is narrowed rather than closed, it is visible in the artifact as
  `HEURISTIC`, it is in `CHANGELOG.md` under Known limitations, and closing it
  needs the `libc`/`getpwuid` decision above. Recorded here rather than only in a
  code comment, because a knowingly-retained deviation from a MUST that lives in
  a doc comment is a deviation nobody will find.

## SE-045 — Reverse-dependency counts are per-provider; SE-042 change (2) is withdrawn
- Status: accepted
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW, ruled by the owner (DP-010)
- Original assumption: SE-042 change (2) — the engine receives the largest
  reverse-dependency count over every provider holding the package, each counted
  inside its own provider, "never summed across them".
- Change: `PackageFacts.reverse_dependency_count` is gone. The count lives in
  `PackageProviderFacts`, beside `availability`, one per provider, derived in
  `package_facts` where every other per-provider fact is derived.
  `MANUAL_REVIEW_REVERSE_DEPENDENCIES` reads the source side's own count, and
  `explain` lists one `package.<provider>.reverseDependencyCount` per provider
  instead of a single figure attributed to nothing.
- Rationale: INV-04 resolves reverse dependencies "inside that provider" and
  INV-08 forbids a cross-provider calculation "in any calculation, ever". A
  maximum across providers is such a calculation, and it reached the user as a
  false statement: for `gnutls` the emitted reason read `count: 3, provider:
  macports` when zero MacPorts ports depended on it and the three were
  Homebrew's. Measured across the six fixtures, three units carried an inflated
  count, in both directions — `python@3.12` was inflated *by* MacPorts — so it
  was not a bias toward one provider, it was wrong symmetrically.
- **Why the original was not carelessness, which matters for not reintroducing
  it.** The code carried a second reason SE-042 did not record: *"choosing one
  side by name would make the answer depend on provider ids (INV-17)"*. That is
  true, and `max` is commutative, so it preserved symmetry at the cost of
  INV-04's letter. It was a genuine collision between two Tier 1 invariants,
  resolved in a direction the authority table does not let an agent choose —
  hence DP-010 rather than a fix.
  The collision was an artifact of the source attribution being unstable
  (SE-047, SE-048). A map dissolves it: no side has to be chosen at
  fact-construction time at all, each rule reads the provider it is reasoning
  about, and both invariants hold at once.
- Evidence: observed on `both-providers-arm64-macos15`; after the change the
  rule fires on exactly the 139 units whose source side has dependents, with
  zero divergence in either direction. `invariant_04_reverse_dependencies_are_
  read_only_from_the_source_provider` gained the assertion that was missing —
  dependents under the *other* provider must not block a removal from this one —
  which is the assertion the defect needed and did not have.
- **The test's own rationale was the vulnerability.** That test argued the
  guarantee was type-level *because* the count was a single field on
  `PackageFacts` and `PackageProviderFacts` "carries no dependents field at
  all". The single scalar is precisely what let a cross-provider value be
  smuggled in, because one number cannot say which provider it came from. The
  guarantee is real now and it is the opposite shape.
- User impact: three units lose a reason they should never have carried, and the
  fixture's `manualReview` total goes from 23 to 22 — one unit changes outcome
  (`python@3.12`, to `USE_RUNTIME_MANAGER`); the other two remain under review
  for other, real reasons. An earlier draft of this entry said three units
  "stop being held at manual review", which overstated it. The reason text now
  names the provider the count came from. `plan gnutls --to macports` now says "3 packages under homebrew depend
  on this one" — which is true, and was previously attributed to MacPorts.
- Architectural impact: the destination's count is retained rather than
  discarded, so a future rule about what installing here changes for other
  packages has the fact available. It is a different rule and a different
  question from the removal guard.
- Compatibility impact: **breaking by the letter of the rule, and taken without
  a version bump, deliberately.** `06-cli-and-ux.md` makes `--json` a
  compatibility surface and `.claude/rules/domain.md` classes a removed field as
  breaking: `facts.package.reverseDependencyCount` is gone,
  `facts.package.perProvider.<id>.reverseDependencyCount` replaces it, and
  `derivation.factsUsed` now carries one entry per provider. `explain` shares the
  analysis schema version, so a bump would take `analysis`, `explain` and
  `doctor` to v2 together.
  That is not taken, because the contract protects *published* consumers and
  there are none: the repository has no remote, no release exists, and no payload
  carrying `schemaVersion: 1` has ever left it. `docs/schemas/explain.v1.md` is
  corrected in this change instead — the removed row deleted, the new one
  documented, the worked example fixed — so the document and the code agree.
  **This exemption expires at the first release.** After that, the same change
  costs a v2 across three documents, and that is the point of writing it down
  rather than letting the next reader infer that removals are free. Golden files
  regenerate.

## SE-047 — `--to` names the destination; a dual-held package is migratable in either direction
- Status: accepted
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW
- Original assumption: `build_plan` refuses when the unit's attributed `source`
  equals `request.to`.
- Change: `--to X` means X is the destination and the source is the other side
  that actually holds the package. The refusal is now "nothing but the
  destination holds it", which is the case where there is genuinely nothing to
  move. When the destination already holds the package, `ADD_DESTINATION` stays
  in the plan carrying **no command** and the reason
  `reason.already_under_destination`. The mapping threshold is waived in exactly
  one case: when the destination **is** the provider the unit is attributed to,
  because then the "counterpart" is the package itself and there is no
  equivalence to be unsure about.
- Rationale: for a package installed under both providers, which side counted as
  "source" fell back to provider-id order, so `htop --to macports` produced a
  nine-step plan and `htop --to homebrew` an exit-2 usage error, for one machine
  state. That is a special case in one direction, which INV-17 names as a defect,
  and it landed on the direction the charter goes out of its way to call
  first-class. Across the whole suite there were eight `plan` invocations and
  every one pointed the same way, so nothing exercised the mirror.
- **Why the step is kept rather than dropped.** Omitting `ADD_DESTINATION` when
  there is nothing to add was the first implementation and it was worse: a plan
  is a document a human reads before agreeing to it, so the absence of work is
  itself worth stating; every plan keeps one shape for `check_ordering` and for
  whatever executes it; and dropping the step silently removed the ordering
  assertion `invariant_01` depends on.
- **A narrower exemption than the first attempt, and why.** That attempt waived
  the threshold whenever the destination held *anything* installed under the
  mapped name. Since a destination row is inserted for any mapping with a
  selected candidate, a `WEAK` or `AMBIGUOUS` mapping whose candidate happened to
  be installed would have been promoted to `EXACT` and walked past
  `MANUAL_REVIEW_MAPPING_CONFIDENCE` — the guard that exists precisely for the
  guess that two differently-named packages are the same software. Caught on
  self-review before the change landed; the waiver now keys on
  `request.to == unit.source`, which is identity rather than equivalence.
- **Each side is addressed by its own spelling.** A unit is keyed by the name
  under the provider it is attributed to, so once `--to` may name that provider,
  the source is the other side — whose row is often called something else,
  `node` for `nodejs24`, `python312` for `python@3.13`. The removal command, its
  reason and the recorded `PackageIdentity` all used the unit key, which names a
  package the source side does not have — or, reachably, a *different* package it
  does, never analysed, mapped or counted for dependents. Found by
  `security-reviewer`; the destination's verification checks had the mirror of the
  same defect and now use the destination's name.
- **An ambiguous request is refused rather than guessed.** One provider's name
  can be the counterpart of several of another's: `python312` is what MacPorts
  calls the match for `python@3.12`, `python@3.13` *and* `python@3.14`. Looking
  the unit up by row name took the first match, so `plan python312 --to macports`
  planned `python@3.12` — a different package. It now reports the candidates and
  stops, which is what this project does with every other ambiguous match.
- **The planner analyses the whole machine.** It scoped the re-analysis to the
  requested name, and `only` does not merely hide rows — which unit a row lands
  in depends on what else was built, so scoping changed the unit and the planner
  reasoned about one the user's view does not contain.
- **A second defect fixed with it.** A unit's mappings point *away* from its own
  source, so when the destination was that source there was no mapping toward it
  to find — yielding no destination package and a `NONE` confidence that stopped
  the plan at manual review. The destination's own row already says what the
  package is called there.
- Evidence: `htop` now plans in both directions with mirrored step kinds;
  `gnutls --to macports` still stops at review and now says why — three Homebrew
  packages depend on it — while `--to homebrew` proceeds, because MacPorts has
  no dependents. The asymmetry that remains is a fact about the machine, not
  about a name. `invariant_17_a_dual_held_package_plans_in_either_direction`
  asserts both, and asserts mirrored plans once the one asymmetric fact in the
  fixture is removed.
- User impact: consolidating onto either provider is expressible, and a plan no
  longer opens by proposing an install that would do nothing.
- **Still emitted, and not fixed here.** The plan continues to warn that it
  cannot tell whether the destination would supply a prebuilt artifact, even for
  a package already built and installed there: the risk tests
  `!dest_row.availability.is_prebuilt()` and never consults the destination's
  presence. An earlier draft of this entry claimed otherwise. That is F-06 —
  "`VERIFY_DESTINATION` is satisfiable by pre-existing state", the planner never
  reading `unit.rows[to].presence` — which this change makes the *designed* shape
  for every consolidation rather than an accident, and which stays open.
- Architectural impact: `PlanRequest` still carries only a destination; the
  source is derived from what holds the package.
- Compatibility impact: an exit-2 usage error becomes a plan for some inputs.
  `locales/en.toml` gains one key.

## SE-048 — `--advanced` is a visibility switch and must not change the answer
- Status: accepted
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW
- Original assumption: advanced mode reveals dependency units; the foundation
  pass recorded the resulting one-unit difference as a count that "differs by
  one … Known, not fixed".
- Change: a unit's identity — driving side, `advanced_only`, the rows it
  contains — is decided from every provider that holds the package, and
  visibility is applied afterwards, to finished units. A unit top-level under
  *any* provider is top-level. The deferral in the merge is unconditional: when
  another provider's copy drives the unit, that provider's loop builds it and
  pulls this row in through its mappings.
- Rationale: the filter ran on rows on the way in, *before* the merge, so which
  provider built the unit depended on the mode — and with it the unit's source,
  its `advanced_only` flag and the rules that fired. `analyze` reported `gnutls`
  under MacPorts while `explain gnutls`, which forces the same path the planner
  uses, reported Homebrew. Two commands, one input, contradictory answers.
- **It was not a count artifact.** The unit's source, its `advanced_only` flag
  and the rules that fired all moved with the flag. The property that holds now
  is set-identity: the top-level set is **the same 63 units in both modes** on
  all six recordings, no unit changes source between them, and advanced mode
  adds 135 dependency units without removing anything.
- **A claim this entry made and had to withdraw.** It first cited `python@3.14`
  as a package "installed and top-level under MacPorts" that the default view
  had hidden. That was wrong twice over: `python@3.14` is a Homebrew formula,
  installed under Homebrew as a `DEPENDENCY`, and not installed under MacPorts
  at all. Its appearance in the default view was not this fix working — it was a
  defect this change introduced, and `security-reviewer` and `spec-guardian`
  both caught the sentence before it was committed. See the visibility-vote
  clause below. The episode is recorded rather than quietly deleted because an
  inference dressed as a measurement is the failure mode this project is built
  against, and it happened here in the entry arguing for the fix.
- **The visibility vote requires a strong mapping, not merely an installed row.**
  Deriving `advanced_only` from "top-level under any provider" let a row attached
  by a `PROBABLE` mapping vote — so a Homebrew *dependency* was promoted into the
  default view by a MacPorts row that is only probably the same software, and
  which three separate units claimed at once. A row votes only if it is the
  unit's own side or its mapping is `Strong` or better: the same threshold
  SE-047 refuses to let a guess waive in the planner, applied to the question of
  what the user sees.
- **A third defect surfaced by the same change.** `analysis.rs` set the unit's
  source to the *iterating* provider while `source_for` had chosen another, and
  deferred only when a unit already existed — otherwise falling through and
  building one labelled with the wrong side. Its own comment described the
  correct behaviour; the code did not implement it. The visibility filter had
  been hiding the second path, and
  `invariant_17_swapping_the_providers_swaps_the_analysis` failed the moment it
  stopped.
- **The deferral asks before it defers.** Making it unconditional was the first
  attempt and it dropped packages: mappings are scored per direction and are not
  symmetric, so when the side that drives the unit does not select the deferred
  row back, nobody builds it. An installed, user-requested port was then reported
  nowhere — neither unit nor row, in either mode — with `plan` stating it was not
  installed under any provider. Found by `spec-guardian`, reproduced, and now
  guarded by two tests: one that enters the branch with an asymmetric mapping and
  fails without the fix, and a completeness assertion over every installed name
  in every recording, which the suite had never had.
- Evidence: source is now identical across modes for every unit;
  `invariant_13_advanced_mode_only_ever_adds_to_the_top_level_view` asserts the
  top-level set is unchanged and that no unit changes source, over four
  fixtures. That one assertion is what the previous test lacked — it claimed
  only that advanced mode yields *more* units, which stayed true throughout.
  `every_installed_package_is_reported_somewhere` covers all six.
- User impact: a top-level package stops being hidden; two commands stop
  disagreeing.
- Architectural impact: `build_plan` still forces `advanced: true`, which is now
  harmless because it no longer changes what a unit is — naming a package is the
  explicit entry INV-13 requires.
- Compatibility impact: one golden file changes — one unit added, one source
  corrected, one outcome no longer falsely `MANUAL_REVIEW`.

## SE-051 — A snapshot is untrusted input, and so is anything that decides what a command runs in
- Status: accepted. All three parts are built. One entry rather than three,
  because it is one boundary (SE-032).
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW; implemented 2026-09-03, part 1
  (F-21) and then parts 2 and 3 (F-09)
- Original assumption: `--from-snapshot` accepts a file whose schema version
  matches, and an environment built by `sanitised_environment` is safe by
  construction.
- Change, in three parts:
  1. **Built.** `Snapshot::check_invocation_environments`, run inside `capture`
     and by `load_snapshot`, refuses an invocation environment no adapter could
     have produced: only the keys an adapter sets (`PATH`, an optional `HOME`,
     the four pinned variables, and that provider's own quieting variables),
     each present and equal to the value the adapter gives it — by name alone
     was not enough, since `HOMEBREW_NO_AUTO_UPDATE` is what makes a read-only
     command read-only and a snapshot could set it to `"0"` — `HOME` exactly
     `~`, and a `PATH`
     ending in the system directories with every leading entry under the prefix
     that provider was *detected* at; a provider the file calls present must
     declare one, and only an absent or broken provider falls back to the
     registry's list. A declared prefix must itself be a usable
     directory, and may not be one another provider is known to own or has
     claimed. The cross-provider comparisons fold case; the "under my own
     prefix" one is byte-exact, because the two directions need opposite folds -
     the loosest matcher is the safe one when every extra match is a refusal, the
     strictest when every extra match is an acceptance.
  2. **Built.** `Snapshot::check_names` refuses what a provider could not have
     reported, and the rule differs by *where the value goes* rather than by what
     type it has:
     - **A name becomes an argument**, so it takes the strict rule
       (`is_safe_provider_name`, the union of the two adapters' predicates): every
       installed, requested, dependency-edge, metadata, availability and
       resolution name, plus revisions.
     - **The prefix becomes a command's `program`**, so it takes the same shape
       rule the environment check applies — one definition, `prefix_shape_error`,
       because the two checks run on different sets of providers and a prefix that
       only one of them saw is how `relative/evil` reached an elevating step.
     - **Everything else reaches a reader**, so it is bounded and free of control
       characters and nothing more: reported versions, install paths, resolved
       paths, the machine's own `PATH` and its findings. Applying the strict rule
       to those refused real machines, twice.
     - **A variant is neither**, and gets its own predicate: MacPorts records a
       deselected variant with its `-` sign.
  3. **Built, and differently from the proposal.** The proposal was a fallible
     `PlanCommand` constructor refusing an argv element that is empty or begins
     with `-`. That is the wrong shape twice over. A verb may legitimately begin
     with a dash — `-q` is how MacPorts is quieted — so a finished argv cannot be
     judged as a whole; the guard sits in `planner::command`, where the package is
     still known to be a package. And "not a flag" is not the whole danger: `port`
     reads `installed`, `all`, `inactive` and nine others as *sets of ports*
     wherever a portname goes, and the set named `installed` is the whole
     installation. A provider-neutral predicate cannot know that, so the twelve
     are data with a source — the list `docs/research/R-3-macports-read-interfaces.md`
     verified against `port.1` on 2026-09-02 — and the guard reads them from the
     provider table.
- Rationale: `check_schema` compares an integer and nothing else, and
  `--from-snapshot` is global, so it reaches `plan`. Names from the file become argv
  elements and even executable names, with no `--` separator and without passing any
  adapter allow-list, because the allow-lists live in the adapters and the planner
  does not go through them. Sharing a snapshot for diagnosis is a workflow the
  documentation invites. Today the consequence is a misleading plan; when the
  executor runs `PlanCommand.argv` as the contract requires, it is flag injection.
- **The environment is the same boundary, and it has two sides, not one.** The
  obvious side is the deserialised field: `--from-snapshot` read
  `invocationEnvironment` verbatim, so a file could state what a `brew` or `port`
  command runs in (F-21). The side that was missed is that *constructed* does
  not mean *safe*: `sanitised_environment` leads the `PATH` with `<prefix>/bin`,
  and the prefix is whatever `brew --prefix` answered — from a recording under
  `--replay`, from the provider on a live run, untrusted either way. A review
  demonstrated it by patching one recorded line, and every layer above accepted
  the result. `sanitised_environment` cannot catch that; it never sees the other
  provider. So the check runs on both routes, and an earlier draft of this entry
  claiming `--replay` was unaffected was wrong.
- **What is asserted, arrived at over three security reviews.** The first
  implementation banned build-flag keys and the other provider's prefix, and was
  defeated four ways: `SUDO_ASKPASS` and `RUBYOPT` are neither and both name a
  program the provider will run; `/tmp/evil/bin` first on `PATH` is neither; and
  `/opt/../opt/homebrew/bin` slips a component-wise prefix test while the kernel
  resolves it where you would expect. Banning is an arms race against everything
  anyone will ever add to `brew`, so the second implementation asserted a shape
  instead — and **every remaining defeat was a place where the shape asserted was
  wider than the shape built**:
  - `/OPT/Local` compares unequal to `/opt/local` and opens the same directory.
    The boot volume is case-insensitive APFS, and `realpath` does not fold it, so
    this is the `..` problem again: the string and the kernel disagreeing.
  - A `HOME` of the snapshot's choosing hands back the entire refused key set.
    `bin/brew` reads `${HOME}/.homebrew/brew.env` and exports anything matching
    `^(HOMEBREW_|SUDO_ASKPASS=|(all|no|ftp|https?)_proxy=)` from it — read in
    Homebrew's own source on this machine, 2026-09-03. The legitimate values are
    absent or `~`, so that is now the rule.
  - `/usr/local` is a prefix the registry knows for Homebrew, and `/usr/local/bin`
    is commonly user- or group-writable where an Intel Homebrew created it — a
    stock macOS has no such directory — and `doctor` reports that configuration as
    an error in its own right. Containment now uses the prefix a provider was
    *detected* at; the registry's list anchors only the "may not claim" test,
    which is the job the comment always described.
  - A prefix shallow enough to contain other things switched the rule off
    altogether. Declaring `/opt` made the other provider's whole tree "mine";
    declaring `/` made everything mine, and `/tmp/evil/bin` went onto the `PATH`
    of a provider whose plan step elevates. This was a *working bypass of the rule
    this entry is about*, and it survived two rebuilds — which is why the declared
    prefix is now constrained in its own right (at least two components, no
    overlap with another provider's, never an ancestor of a system directory) and
    why every leading entry, not only the prefix, is tested against the other
    providers.
  - An environment with no pinned variables, or a `PATH` with no system
    directories, was accepted. Dropping `LANG=C` is a parser-divergence lever
    against a tool whose untrusted-output posture rests on
    `ErrorId::ProviderOutputChanged`; dropping `/usr/bin` changes which helper a
    provider finds. Both are now required, because the builder always emits them.
- **Refuse rather than rewrite.** Silently re-sanitising on load would make the
  analysis stop being a function of the snapshot, which is the property
  `--from-snapshot` exists to have, and it would hide that the file was tampered
  with. Refusing is what `load_snapshot` already promises — a snapshot that
  cannot be trusted produces no plan rather than a partial one.
- **What refusing costs, and the three legitimate things it wrongly refused.** A
  check like this fails by rejecting what is real, and this one did three times
  across three rounds. The third is the worst and was not a document at all: a
  `HOME` of `/Users/x/` — a trailing slash, which shells and `launchd` hand out
  freely — redacts to `~/` rather than `~`, so **every live run of every
  subcommand exited 8**, telling the user to take a snapshot that would fail the
  same way. `HOME` is now normalised where the environment is read, once, which
  fixes redaction everywhere rather than teaching each consumer the same trick,
  and `usable_home` has a test of its own because nothing else would catch its
  removal. The first two: `capture::broken()` writes no environment at all, so a snapshot taken on
  a machine whose provider had failed — precisely the machine someone captures
  one from to ask for help — was unreadable, and the error told them to take
  another. And home redaction rewrites a `PATH` to `~/homebrew/bin`, which the
  check called relative, so any Homebrew installed under `$HOME` produced a
  snapshot it could not read back. Both load now, and both are asserted.
- **Where the check runs.** Inside `capture`, and in `load_snapshot`. Not in the
  caller: `capture` is public, so a future subcommand or embedder that built a
  snapshot without running the rule would reopen this silently. The contract is a
  required field of `CaptureContext`, so one cannot be constructed without it —
  which is why four test call sites had to be given it, and that is the point.
- **Where the rule lives, and where the facts do.** The rule is in `domain`: pure,
  no I/O, no clock, no provider named. The facts it needs — which prefixes belong
  to which provider, which extra variables an adapter adds — come from
  `registry::environment_contracts`, in the layer that knows providers. Taking
  the prefixes from the registry and not only from the file is what closes the
  evasion above, and it is deliberately *not* used to widen where a provider may
  put its own `PATH`; taking the keys from each adapter's own list is what stops the
  builder and the reader drifting apart, the same consolidation SE-056 made for
  `is_build_flag`. `is_safe_path`, `SYSTEM_PATH` and the pinned variables moved to
  `domain` for the same reason, and `adapters` re-exports `is_safe_path` so its
  three existing callers are unchanged.
- Evidence: findings F-09 and F-21. (An earlier draft carried line references
  inherited from the proposal; they had drifted onto unrelated code, so they are
  dropped rather than re-guessed.) For part 1, every claim below was run rather than
  reasoned: every evasion named above, and the ancestor-prefix family a third
  review found after two rebuilds, reproduced against the real binary before it was
  believed and refused after; the case-insensitivity confirmed on this
  machine (`/USR/bin` resolves; an earlier draft added "and `realpath /OPT`
  returns `/OPT`", which is false — `/bin/realpath` does fold, and the
  observation came from Python's lexical `os.path.realpath`. The conclusion is
  unaffected, since nothing here calls `realpath`, but this bullet claims every
  statement in it was run) and the `HOME`
  mechanism read in Homebrew's own `bin/brew`; both false positives reproduced and
  fixed; a tampered recording refused on the `--replay` route; all six recordings
  still replay, and all six still capture through `snapshot --out` and load again —
  which `scripts/run-fixture-matrix.sh` has covered all along, at `:161-167`. An
  earlier draft of this entry said it covered "the replay half only", which was
  simply false, and is the fourth evidence claim of mine a reviewer caught in this
  pass. The
  tests are `invariant_10_a_snapshot_cannot_dictate_a_provider_command_environment`
  — seventeen cases run against both providers, each asserting the refusal names
  its own reason, plus ten that do not generalise and assert the exit code — and
  `invariant_10_a_recording_cannot_move_one_provider_onto_the_other`.
- **One door.** `Snapshot::check` runs the schema, the names and the
  environments, and both boundaries call it. Three checks reachable separately is
  how a boundary comes to apply two of them.
- **What the second guard is worth, said plainly.** Removing `check_names` from
  the door makes the F-09 test fail. Removing the *name* half of the guard in
  `planner::command` makes nothing fail, because the boundary applies the same
  predicate and gets there first — but the *reason* is narrower than it looks.
  A destination name can come from `mapping.selected`, which is sourced from
  `data/policy/curated-mappings.toml` and never passes `check_names` at all, so
  for that route the planner guard **was** the only check rather than the second
  one. Corrected 2026-09-16 (F-48): `check_curated_names` applies the same
  predicate in `PolicyData::load_from`, at the layer that owns the file, so the
  planner's name half is now the second line on that route too. The reason it had
  to move is the identifier, not the coverage — the planner answers
  `InputUnusable`, which tells a user to check their own capture, snapshot or
  recording for a defect that would be in the build's compiled-in data. The
  reserved-word half still exists only at the planner, since a pseudo-portname is
  a perfectly good name, and `every_curated_name_is_one_the_planner_would_accept`
  in `crates/application/tests/planner_refusals.rs` is what stops the curated
  table from producing one — the two lists are visible together in `build_plan`
  but are deliberately not compared per request. It is kept as defence in depth for a name reaching the planner by
  some other route, and because it fails closed where an empty destination name
  would become an empty argument — a case F-09 named and which a probe over every
  plan the suite builds never reached, the mapping-confidence block sending such
  a unit to manual review first.
- **What is still open, stated as limits rather than left implicit.**
  - `detection.prefix` is validated for shape — absolute or `~`-rooted, no `..`,
    bounded, at least two components, containing no system directory, not
    overlapping another provider's — but not for plausibility. A snapshot may
    still declare a prefix nobody owns, `/tmp/evil`, and legitimately lead its
    `PATH` with `/tmp/evil/bin`, producing a plan whose program is
    `/tmp/evil/bin/brew`. Closing that means deciding where a provider may be
    installed, which is a product question rather than a validation one, and is
    not decided here.
  - **The reserved-word list is applied by the planner, not at the boundary**, so
    a snapshot naming a port `installed` loads, is reported by every command, and
    can even be written back out; only a plan for it is refused. Moving the check
    to the boundary would need the domain to see provider-command data, which is
    a layer it cannot reach from `load_snapshot`. The schema document says which
    rule runs where rather than implying one place.
  - **The reserved-word list is MacPorts' only.** Homebrew's own vocabulary was
    not researched, so if `brew` reads some word as other than a formula name,
    nothing here catches it. The mechanism is in place — `reserved_names` per
    provider in the command table — and filling it is a research item.
  - **Several fields are unchecked because nothing renders them** — `provenance`,
    `metadata`, `environment.reported`, the per-provider `environment` map. Probed
    with escape codes and none reached a terminal, but that holds by where they
    are consumed rather than by a rule, so a new renderer could expose one.
  - **No fixture exercises a resolution.** All six recordings carry none, so the
    resolution arm of `check_names` runs only against hand-built snapshots in the
    test and on a live machine. That is the arm with the residual false-positive
    risk, which is why resolved paths are held to the printable rule and not the
    strict one.
  - The live route validates the incoming prefix *less* than the replayed one:
    `--replay` puts a recording's prefix through `is_safe_path`, while
    `brew --prefix` on a live run passes only an absoluteness test. The rule here
    closes the `..` case for both, but the asymmetry belongs in part 2.
  - A plan derived from a redacted snapshot is not runnable as it stands: its
    programs read `~/...`. Nothing executes plan commands yet, so this is a note
    for the executor pass rather than a defect.
- User impact: a malformed or hostile snapshot is rejected with a clear error.
- Architectural impact: validation moves to the boundary rather than living only in
  the adapters, and the boundary is now both routes rather than the loaded one.
- Compatibility impact: a snapshot stating an environment this tool would not have
  built stops loading. Snapshots written before `invocation_environment` existed
  carry none and are refused; they are pre-release scratch files.
  **The permitted key set is now part of the artifact contract without being part
  of its version.** Adding a quieting variable to an adapter makes every snapshot
  the new build writes unreadable by the old one, with a message that reads like
  tampering, and `.claude/rules/domain.md` does not count an added environment key
  as a `schemaVersion` change. Taken deliberately: "fails closed" is the stated
  posture and the alternative is warn-and-continue, which is how the boundary came
  to be unguarded in the first place. If the key set changes before the first
  release, the version should move with it.

## SE-057 — The reader-drain timeout was a bound on scheduling, not on a held pipe
- Status: accepted
- Date / Pass: 2026-09-03 / 02-ARCHITECTURE-REVIEW, implementation
- Original assumption: two seconds is long enough for a reader thread to hand
  back what it read, so a longer wait means a grandchild is holding the pipe.
- Change: `READER_DRAIN_TIMEOUT` in `adapters::process::system` goes from two
  seconds to six. The two tests that assert on captured `stdout` now assert
  `!inv.truncated` first.
- Rationale: the constant exists to bound the pathological case — a child exits
  and leaves a grandchild holding the write end, so a blocking read never
  returns. Two seconds does bound that. What it also did was fail on ordinary
  *scheduling*: with a dozen test binaries and a build competing for the
  machine, the reader for `/bin/echo` did not get its result onto the channel
  in time, and the invocation came back with an empty `stdout` marked
  truncated. This repository's own suite reproduced it at roughly one run in
  fifteen.
- **Nothing mis-parsed, and that is the part worth keeping.**
  `Invocation::require_success` turns truncation into `ProviderOutputChanged`
  before any adapter parses, precisely because a stream cut mid-way is a
  *plausible* one: a line-oriented inventory parses perfectly into an inventory
  silently missing packages. So the product behaviour was correct throughout —
  a loaded machine would have refused to analyse rather than analysed wrongly.
  What was wrong was the gate: `scripts/validate.sh` is the definition of
  "valid", and one that fails for a reason like this teaches people to re-run
  it, which is how a real failure gets waved through.
- **Why six and not more.** The two streams are drained one after the other, so
  the worst case is twice the constant, and
  `a_grandchild_holding_the_pipe_does_not_hang_the_runner` fixes twenty seconds
  as the ceiling for a held pipe. Ten seconds sat exactly on that bound and
  failed the test — which is how the doubling was found, and it is now written
  beside the constant. Six keeps the worst case at twelve. Draining
  concurrently would remove the doubling and allow a longer wait; that is a
  change to the one subprocess site and is not made here.
- Evidence: reproduced before the change at 1 in 12, 1 in 25 and 1 in 15 full
  runs of the adapters crate, and confirmed by making the assertion name its
  cause — the failure then reads "the reader thread did not drain in time"
  rather than reporting the output as wrong. After the change, 30 consecutive
  full runs with no failure, where roughly two would have been expected. Thirty
  runs is evidence, not proof.
- User impact: none in the normal case, where the channel already has the
  answer. A machine with a grandchild holding both pipes waits up to twelve
  seconds per invocation instead of four before recording a truncated read.
- Architectural impact: none. One constant and two assertions.
- Compatibility impact: none.

## SE-056 — A plan command says what it invokes, so the rule that governs it is not inferred from its name
- Status: accepted
- Date / Pass: 2026-09-03 / 02-ARCHITECTURE-REVIEW, implementation
- Original assumption: every `PlanCommand` is a provider invocation, and the one
  that is not — `SET_PRIMARY`, which names MacPortsBrew itself — is recognised by
  comparing `program` to the literal `"macportsbrew"`.
- Change: `PlanCommand` carries a `CommandTarget`, `PROVIDER` or `TOOL`.
  `invariant_10_plan_commands_carry_the_sanitised_invocation_environment` no
  longer filters anything out; it checks each command against the rule that
  governs it, and it now also asserts INV-10's actual sentence — that no environment
  value of any command contains another provider's prefix, in any variable — which it
  never did.
- Rationale: the string comparison lived inside the test named after the
  invariant, as `.filter(|c| c.program != "macportsbrew")`. It excluded the one
  command that INV-10 does not reach, so nobody saw that the command
  also carried no environment at all and a relative program the executor refuses
  by design. A gate that is green because its exception was written into it is
  worse than an absent gate, because it is cited as evidence — and this suite's
  green status is cited in the pass report, the README and the wiki.
  INV-10 is a rule about `brew` and `port` invocations, so not demanding a
  sanitised environment of a self-invocation is correct. What was wrong was
  deciding that by string match, invisibly, inside the test. A distinction the
  type makes cannot be filtered away by accident, and a rename cannot defeat it.
- **What the type settles, and what it does not.** It settles that this command
  is not a provider invocation and must not be resolved through `PATH` — the
  shadowing hazard `doctor` exists to warn about, previously implied by a bare
  name an executor would have had to search for. It does not settle whether an
  executor spawns it or performs the step in-process; the shim directory is
  MacPortsBrew's own (SE-036), so in-process is likely, and that belongs to the
  pass that writes the executor. The current shape — no environment, a readable
  name rather than a path — is now pinned by assertion rather than left
  unexamined, which is how it went unnoticed.
- **A declared deviation, named as one.** `docs/spec/04-migration-and-safety.md`
  ("The plan is the contract") states as **MUST** that a `MigrationPlan` is
  *re-executable* and carries "every command to be run with its full argv".
  While `program` for a `TOOL` command is a readable name rather than the
  program, one step of every plan that reaches the mutating path does not meet
  that MUST. Not of every plan: a plan halted at manual review carries no
  commands at all. This is a
  Tier 3 deviation and is recorded here as one rather than left as a matter of
  scope: the remedy is the executor's — most likely `current_exe()`, or
  performing the step in-process and emitting no command at all — and until it
  lands the `--json` contract says plainly which command this is, so a consumer
  is not misled about what it would run.
- **Where the shape is pinned, after review.** First written into the INV-10
  test itself, which repeated F-04's own mistake one step downstream: the
  remedy for the relative program is an absolute one, and asserting the
  relative form inside `invariant_10_*` would have made the fix fail a test
  whose name lends the old shape an authority INV-10 never gave it. The shape
  now lives in `the_self_invocation_has_the_shape_the_executor_pass_will_replace`,
  and the INV-10 test asserts only what INV-10 governs.
- **What the type does not enforce on its own**, also from review: nothing
  structural stops a provider command being labelled `TOOL` and skipping every
  provider check. What closes it is described three bullets below. The count is
  `== 1`, not `<= 1`: zero would make the whole arm dead code with the suite
  green, which is the shape of the defect being removed.
- **Three weaknesses in the leak assertion, corrected over two review rounds.**
  It checked two variables where INV-10 names six "or equivalents"; it compared
  against the literals `/opt/local` and `/opt/homebrew`, so a MacPorts install
  at another prefix would have made it vacuous while a real leak went unseen;
  and it matched by splitting on `:` and testing the start of each element,
  which finds `PATH`-shaped values and misses the form INV-10's own examples
  take — `LDFLAGS=-I/opt/local/include` begins with `-I`. It now derives the
  prefixes from the snapshot (`registry.rs` is meant to be the only place that
  knows where a provider lives), compares them by path component rather than by
  string, and asserts that no environment *value* of any command contains
  another provider's prefix, in any variable. The value test is a substring
  match and the ownership test is not, so they disagree in one shape: with
  providers at `/opt/local` and `/opt/local-arm`, a clean `PATH` belonging to
  the second contains the first as a substring and the check fails with no leak
  present. No pair of real prefixes does this today, and the failure is loud
  rather than silent, so it costs a future fixture author an afternoon rather
  than hiding anything. It is recorded rather than left to be discovered.
- **The build-flag list had begun to exist three times.** `process::environment`
  filters on it, `system::machine` kept a reporting superset in step with it by
  hand, and the INV-10 test named one of these variables and a `DYLD_` prefix —
  so a leak through `CXXFLAGS` passed the test written to catch it. This fix's
  own first draft made that a fourth copy, ten entries against the executor's
  fourteen, which is how the problem became visible. The list is
  now `domain::is_build_flag`, which is what the standing note in
  `system/machine.rs` asked for: it names no provider and does no I/O, so it is
  a domain fact. `system::machine::is_reported` now reads
  `name == "MANPATH" || macportsbrew_domain::is_build_flag(name)` instead of
  holding its own copy — fully qualified, because `process` keeps its
  `environment` module private and re-exports only `sanitised_environment` and
  `EnvironmentSpec`. The note there had asked for a `process` re-export and the
  deletion of the copy; the re-export is not what happened, the deletion is, and
  the shared definition sits a layer lower than the note imagined. So
  the superset keeps its one deliberate extra and cannot drift. Its test is a
  property now rather than a fourth hardcoded list: adding an entry in `domain`
  strengthens the executor's filter, `doctor`'s reporting and the INV-10 test at
  once, which it did not before — a probe adding one to the domain list left the
  whole suite green while the two lists silently disagreed.
- **How a `TOOL` command is told apart, after the second round.** First by
  asserting it runs out of no provider prefix — which repeated the same mistake
  again: the executor's likely fix is `current_exe()`, and a MacPortsBrew
  installed *by* Homebrew or MacPorts legitimately lives under that provider's
  prefix, so the correct future fix would have failed the INV-10 test. It is now
  told apart by what it runs: its program's file name is not one of the
  executables a provider is invoked through. Verified both ways — with the
  program moved to `/opt/homebrew/bin/macportsbrew`, `invariant_10_*` passes and
  the shape-pinning test fails, which is the division of labour intended.
  The set comes from `provider_commands::load()`, the table the planner itself
  builds commands from, after two attempts that did not work. Taking it from the
  plan's own commands left a reachable hole — a retain-source plan never invokes
  the source provider, so a `TOOL` command named `brew` passed. Taking the
  table's `executable` verbatim was worse: it stores `bin/brew` against a
  command's full path, so the comparison could never match at all and the
  assertion was dead while reading as if it worked. Both were found by probing
  and neither by reading.
- **What this fix does not reach.** The environments it inspects are ones the
  test wrote and the planner cloned, so it constrains the construction path and
  not the loading one. `--from-snapshot` takes `invocation_environment` verbatim
  from a file and nothing re-sanitises it. That is F-21, filed rather than fixed
  here. `--replay` is not affected: it drives the real adapters against recorded
  process output, so the environment is still constructed by
  `sanitised_environment`.
- **A second gap closed with it.** Every plan fixture had the destination already
  holding the package, so after SE-047 their `ADD_DESTINATION` carries no
  command — and the suite stopped seeing any command that installs software.
  `install_needed()` uses the curated `gnu-sed → gsed` mapping, which is `EXACT`
  and neither stateful nor a runtime family, so the plan completes and INV-10 is
  asserted against a real `port install`.
- Evidence: probes, each reverting one property, each run and each failing —
  the self-invocation mislabelled as a provider command; a provider command
  mislabelled as the tool, including with every other `TOOL` assertion disabled,
  so the name check is shown to catch it alone; a `TOOL` command named `brew` in
  a retain-source plan; the self-invocation deleted, which `<= 1` would have
  permitted; `CXXFLAGS=-I/opt/homebrew/include` and `LDFLAGS=-L/opt/local/lib`,
  both of which the earlier colon-splitting form passed; the other prefix
  appended to a provider command's `PATH`; and, in the other direction, the
  program moved to `/opt/homebrew/bin/macportsbrew`, which must *not* fail
  `invariant_10_*`. Both installing directions are exercised (INV-17), so the
  `brew` side is checked the same as the `port` side. Finding F-04, the last
  Tier 1 violation the architecture review left open.
- User impact: none directly; the plan gains a field that says what each command
  is.
- Architectural impact: `CommandTarget` is a domain type with a `Provider`
  default, so a deserialised v1 plan without the field reads as a provider
  command, which is what every v1 plan's commands were except one.
- Compatibility impact: additive to `plan.v1.md`, so `schemaVersion` stays 1 and
  the field is documented there; noted in `CHANGELOG.md`, which the
  backward-compatible rule requires. **The default has one wrong case, and it is
  this entry's own subject.** A plan written before this change carries no
  `target`, so its `SET_PRIMARY` command loads as `PROVIDER` — asserting the
  opposite of the fact the field exists to record. `PROVIDER` is still the right
  default, because it is correct for every other command in every such plan and
  the alternative is a third state that means "an old writer"; nothing consumes
  `target` yet, so nothing acts on the wrong value. It is recorded here rather
  than left for someone to find.
- **Not an exemption — a scope boundary, which is a different thing.** An
  earlier draft of this entry called `CommandTarget::Tool` "a declared exemption
  from INV-10's rule". That was wrong in a way worth recording, because it would
  have become precedent: INV-10 governs "every `brew` and `port` invocation",
  and a self-invocation is neither, so INV-10 never reached this command and
  nothing is being relaxed. The type records where the invariant's scope ends,
  in place of a string comparison that decided the same thing invisibly.
  Declaring an exemption from a Tier 1 invariant is not an agent's to make; the
  distinction is the difference between recording a boundary and quietly moving
  one.
- **Coverage, stated plainly.** What keeps a provider command from taking the
  `TOOL` label and skipping every provider check is a handful of assertions over
  four hand-built plans. That is thin for a boundary this load-bearing. Widening
  it means sweeping the helper over more plans, not adding a constraint to
  `MigrationPlan::check_ordering`, which is INV-01's home and should not grow an
  unrelated rule.

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
- Date / Pass (help text exemption added 2026-09-02 / 01-FOUNDATION-PASS): 2026-09-02 / pre-implementation review
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
- Foundation Pass addition: the command-line `--help` text is compile-time
  English produced by the argument parser and does not go through the
  catalogue. Localising help is deferred with the translations; the
  exemption is recorded here so `06-cli-and-ux.md`'s "all user-facing text
  goes through the i18n layer" is read with this one stated exception until
  the locale pass revisits it (evidence: `crates/cli/src/app/mod.rs`
  doc-comments; `docs-auditor` finding, 2026-09-02).

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

## SE-037 — CI runners: Intel is testable, the old range is not, and both facts have dates
- Status: accepted
- Date / Pass: 2026-09-02 / 01-FOUNDATION-PASS; extended 2026-09-02 /
  02-ARCHITECTURE-REVIEW-PASS
- Original assumption: `07-quality-and-testing.md` held open that "hosted Intel
  macOS runner availability is uncertain and must be verified, not assumed",
  with cross-compilation plus Intel fixtures as the contingency; ADR-0002 and
  SE-022 treated the whole question as unresolved and withheld the macOS
  build-and-test job on that basis.
- Change: the uncertainty is replaced by the answer and its expiry.
  `macos-15-intel` and `macos-26-intel` exist as *standard* runners, free on
  public repositories; `macos-latest` is `arm64`, so labels are pinned
  explicitly. Both architectures are therefore tested natively on macOS 15 and
  26, and cross-compilation is demoted to a redundancy check. macOS 10.12–10.15
  remains **best-effort**, now for a structural reason rather than an unknown
  one: GitHub maintains images only for the latest two stable macOS versions.
  Two dated policy facts are recorded as such under INV-20: GitHub's statement
  that x86_64 macOS ends after the macOS 15 image retires in Fall 2027 — in
  visible tension with `macos-26-intel` shipping after that statement, so Fall
  2027 is the *earliest* credible date and is re-checked, not planned against —
  and Apple's 2026-09-01 announcement that macOS 27 is the last release to
  support Rosetta, which is why nothing in CI may depend on it. Finally, a fact
  the specification did not carry at all: the runner images ship a live,
  preinstalled **Homebrew and no MacPorts**, so every macOS job sets the
  quieting variables and both adapters stay on captured fixtures.
- Rationale: an open "must be verified" in a MUST is an instruction to a future
  agent, not a property of the system, and it was blocking the one job that
  makes the Intel support claim honest. The preinstalled-Homebrew fact is the
  more subtle half: a runner's Homebrew will auto-update mid-job and make output
  non-reproducible, and the asymmetry — live Homebrew present, MacPorts absent —
  is exactly the pressure that would make one adapter tested against reality and
  the other against fixtures. Naming it keeps INV-17 from eroding by
  convenience: the live install is contamination to neutralise, not a resource
  to exploit.
- Evidence: `docs/research/R-10-ci-runners.md` findings 1–3 and 8–10, from the
  GitHub-hosted-runners reference table, `actions/runner-images`, and the
  `github.blog` changelogs of 2025-09-19 (macOS 13 retired, `-intel` suffix
  introduced), 2026-02-26 (macOS 26 GA including the Intel label) and the
  `macos-latest` migration; Apple's Rosetta announcement of 2026-09-01. All
  read 2026-09-02.
- User impact: the published support matrix can say **tested** for macOS 15 and
  26 on both architectures and mean it, and **best-effort** for 10.12–10.15
  with a reason rather than a shrug.
- Architectural impact: none on the product. On CI, the matrix is the source of
  the support matrix, which is generated from the job list rather than
  maintained beside it.
- Extended 2026-09-02 (02-ARCHITECTURE-REVIEW-PASS), by owner decision on
  DP-007: the repository will be **public** on GitHub, so "free on public
  repositories" stops being a condition this entry assumed and becomes a
  property of this project — the two-architecture macOS matrix, the most
  expensive runner SKU there is, costs nothing. Two constraints come with it and
  belong beside the runner labels rather than in a billing footnote: **larger
  (`-large`) runners are never free on public repositories and cannot draw on
  included minutes, so no job may request one**, and self-hosted runners are
  inappropriate on a public repository because a fork can execute arbitrary code
  on them. A third consequence is not about cost at all: GitHub does not
  pass secrets to a fork-triggered runner (GitHub Docs, "Using secrets in GitHub
  Actions", read 2026-09-02), which is what makes SE-014's signing job safe to
  run in the open and simultaneously dictates its shape.
- Compatibility impact: the Intel half of the project's reason to exist can be
  *tested* until at least Fall 2027; after that it can only be claimed, and the
  documentation will have to say so.

---

# Platform, toolchain, licence and distribution

## SE-014 — Gatekeeper confronted as a distribution requirement
- Status: accepted
- Date / Pass: 2026-09-02 / pre-implementation review; extended 2026-09-02 /
  01-FOUNDATION-PASS; resolved 2026-09-02 /
  02-ARCHITECTURE-REVIEW-PASS
- Original assumption: ship binaries and checksums via GitHub Releases.
- Change: signing/notarisation versus ad-hoc signing plus honest quarantine
  instructions is a required, recorded decision before the first release
  (DP-004, R-9).
- Rationale: the stated goal is that MacPortsBrew installs without a working
  package manager. A downloaded binary that macOS refuses to run defeats that
  goal at the first step, for a tool whose entire proposition is trustworthiness.
- Extended 2026-09-02 (01-FOUNDATION-PASS), after R-9, which corrected the
  mechanism `08-distribution-and-project.md` stated:
  1. The blocker is the **quarantine attribute**, not the absence of a
     signature. Unsigned Mach-O executables run unobstructed when unquarantined
     — 267 of 321 in one `/usr/local/bin` are unsigned and all of them work.
  2. Quarantine is set by the **downloading application** through Launch
     Services, so a browser sets it and `curl` does not. "A binary downloaded
     from the internet carries the quarantine attribute" was too broad.
  3. What a quarantined, *unsigned* binary does when run from a shell on
     macOS 26 is **UNRESOLVED**: Apple documents Gatekeeper around opening
     applications, plug-ins and installer packages, not around `execve`. The
     specification assumes the pessimistic case, and the experiment is a
     before-first-release obligation rather than an open question.
  4. A bare binary **cannot be notarisation-stapled**, so notarisation would not
     deliver an offline-verifiable artifact in the format this chapter mandates.
     That changes what "just notarise it" buys, and DP-004 now records it.
  5. `arm64` binaries are ad-hoc signed by `ld` by default, so ad-hoc signing
     there is a CI assertion rather than a pipeline step — and a post-link
     `strip` or `lipo` that invalidates the signature is a real failure mode.
- Rationale for the extension: the original sentence attributed the block to the
  wrong property. Left standing, it would have priced a recurring 99 USD/year
  subscription against a problem whose severity is still unmeasured, and bought
  a fix that does not apply to the artifact format the project ships.
- Resolved 2026-09-02 (02-ARCHITECTURE-REVIEW-PASS), by owner decision on
  DP-004, recorded in **ADR-0005**: Developer ID signing plus notarisation, with
  three artifacts per architecture — the bare binary, still the leading path,
  plus a `.pkg` and a `.dmg` that can carry a stapled ticket. What changed was
  not the analysis but one of its premises: the owner already holds a paid Apple
  Developer account, so the recurring cost this entry weighed does not exist, and
  the identity it is tied to is the name DP-006 resolved as copyright holder the
  same day. Extension point 4 above is answered by shipping containers that can
  be stapled, not by the account; extension point 5's `strip`/`lipo` hazard now
  invalidates a real signature rather than an ad-hoc one, which is why CI asserts
  the authority on the final artifact. The consequences are stated once in
  ADR-0005 and, where they bind, in `08-distribution-and-project.md` — not
  summarised again here, per SE-032.
- Extended again 2026-09-02 (02-ARCHITECTURE-REVIEW-PASS), by owner decision
  after security review: **build provenance attestation is promoted from SHOULD
  to MUST**, in the same change that created the risk it answers. Putting a
  signing key in the release pipeline is what made the published checksums
  insufficient as a trust anchor — they sit on the same release page as the
  artifacts, so whatever can replace one can replace the other, and the
  documented "verify the checksum" step would then verify a hash the attacker
  wrote. The requirement is in `08-distribution-and-project.md`. One caveat
  belongs here rather than there: the cost objection is *assumed* gone, not
  established — R-10 mentions `attestations` only as a `permissions:` scope, so
  cite GitHub's artifact-attestation documentation with a read date before the
  release workflow relies on it.
- Evidence: `docs/research/R-9-gatekeeper-and-distribution.md`, observations
  O-3, O-5, O-6, O-7 and O-11 on this machine, and citations C-1, C-2, C-5,
  C-6, C-7 and C-8, all 2026-09-02. Cite Gatekeeper as
  <https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web>;
  the `developer.apple.com/documentation/security/…` form returns 404.
- User impact: the first-run instructions describe a mechanism the user can
  verify, instead of one that would not survive contact with `curl`.

## SE-021 — Licence: Apache-2.0, with its consequences propagated
- Status: accepted
- Date / Pass: 2026-09-02 / owner decision (DP-003); extended 2026-09-02 /
  02-ARCHITECTURE-REVIEW-PASS
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
- Extended 2026-09-02 (02-ARCHITECTURE-REVIEW-PASS), by owner decision on
  DP-006: the copyright holder is **Pablo Guia**, an individual rather than an
  entity. It appears in `README.md` and release metadata, and still in no source
  file — ADR-0001's SPDX-only rule is unchanged. The string deliberately matches
  the name on the Apple Developer account, so that the Developer ID signature
  required by SE-014 and the copyright line name one person rather than two
  spellings; a `NOTICE` file remains unnecessary until something needs
  attributing.
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
- Compatibility impact: 10.12–10.15 are published as **best-effort**, and after
  R-10 (2026-09-02) that is a *policy consequence* rather than a gap in
  knowledge — GitHub keeps images only for the latest two stable macOS
  versions, so no hosted path to those releases exists or is coming. Stated
  honestly rather than claimed. Extended 2026-09-02 (01-FOUNDATION-PASS):
  macOS 15 and 26 are **tested** natively on both architectures, because hosted
  Intel runners do exist (SE-037); and MacPorts' own floor is now recorded
  beside the project's — MacPorts 2.12.6 ships package installers down to
  macOS 10.12 (R-4), coinciding exactly with the Intel floor, so below it
  neither this tool nor the provider it would recommend has a supported path.
  ADR-0002's "must still verify" list carries the same answer appended to the
  original question rather than replacing it.
- Formerly not done, now unblocked: the macOS build-and-test CI job. It was
  withheld because a workflow written against an unverified runner is a claim
  about CI that CI does not support; R-10 verified the runners, and SE-037
  records the matrix and its expiry.
- Still open inside the decision: which TUI and terminal crates. Licence
  compatibility, deployment floor and East Asian double-width handling are
  verified together before committing to one.

## SE-038 — "Mapping confidence below STRONG" excludes the absence of a candidate
- Status: accepted
- Date / Pass: 2026-09-02 / policy-layer implementation
- Original assumption: `03-policy-and-recommendations.md`, Starting policy,
  lists `MANUAL_REVIEW` for "mapping confidence below `STRONG`". Read
  literally that includes `NONE`, which is below `STRONG` on the scale in
  `02-domain-model.md`.
- Change: `MANUAL_REVIEW_MAPPING_CONFIDENCE` fires on `AMBIGUOUS` and
  `PROBABLE` only. `NONE` does not fire it, and neither does a candidate under
  a provider that is not present and healthy on this machine.
- Rationale: `NONE` is not weak evidence, it is the absence of a candidate.
  There is nothing for a person to adjudicate, and the honest answer — keep
  the package where it is — is what `KEEP_SOURCE_DEFAULT` already produces
  with a reason of its own. Firing manual review on it would mark every
  package with no counterpart as needing a human, which is most of a large
  installation, and an alert that fires on everything is an alert nobody
  reads. The presence half is the same argument: a shaky candidate under a
  provider the user has not installed is not a decision anyone can take.
- Evidence: `crates/policy/src/rules/manual_review_mapping_confidence.rs`
  tests `no_candidate_at_all_is_not_a_review_item` and
  `a_candidate_under_a_provider_that_is_not_here_is_not_a_review_item`; the
  `Confidence` scale and `Confidence::is_strong` in
  `crates/domain/src/mapping.rs`.
- User impact: fewer `MANUAL_REVIEW` rows. A package with no equivalent is
  reported as staying put, with a reason, rather than as needing attention.
- Architectural impact: none. The threshold stays `Confidence::is_strong` in
  the domain, so policy and mapping cannot drift apart on what "strong" means.
- Compatibility impact: none shipped yet.

## SE-039 — Availability `UNKNOWN` forces review only for a reachable destination
- Status: accepted
- Date / Pass: 2026-09-02 / policy-layer implementation
- Original assumption: `03-policy-and-recommendations.md` lists `MANUAL_REVIEW`
  for "availability `UNKNOWN`", unscoped.
- Change: `MANUAL_REVIEW_AVAILABILITY_UNKNOWN` fires when a destination that
  has a candidate, answers on this machine and does **not already hold the
  package** reports `UNKNOWN`. An `UNKNOWN` on the side already holding the
  package does not fire it, and neither does one under a provider with no
  candidate, one that is absent or unhealthy, or one under which the package
  is installed (the fixture matrix showed that without the last exclusion
  `KEEP_BOTH` could never win toward a provider whose archive availability
  is only knowable online, R-4).
- Rationale: the unscoped reading fires on almost every package. R-4 finding 9
  establishes that one provider's artifact availability is not determinable
  offline in the positive direction at all, and finding 10 that an offline
  analysis "must say `UNKNOWN` — never `BINARY_AVAILABLE` — for everything
  else". Every offline run would therefore mark the whole installation for
  human review. The decision an unknown actually blocks is *moving there*, so
  that is what it is scoped to. An unknown on the side already holding the
  package blocks nothing: the package is already installed.
- Evidence: `docs/research/R-4-macports-binary-archives.md` findings 9 and 10;
  tests `an_unknown_on_the_side_holding_the_package_is_not_this_rules_business`
  and `a_provider_that_is_not_on_this_machine_is_not_a_destination` in
  `crates/policy/src/rules/manual_review_availability_unknown.rs`.
- User impact: an offline `analyze` produces usable recommendations instead of
  a wall of `MANUAL_REVIEW`. The unknown is still reported as a fact per
  provider in `explain` and in the availability column.
- Architectural impact: introduces `Facts::mapped_destinations` as the single
  definition of "somewhere this package could be sent" — present, healthy, and
  reached by a mapping — which every rule that speaks about the other side now
  reads. That the definition sits in one place is what keeps the rules
  consistent with each other.
- Compatibility impact: none shipped yet.

## SE-040 — Provider deadlines and equivalences are data; the horizon is code
- Status: accepted
- Date / Pass: 2026-09-02 / policy-layer implementation
- Original assumption: `03-policy-and-recommendations.md` requires all provider
  support knowledge in a versioned data file (INV-20) and describes the
  approaching-deadline rule as using "a configurable horizon", without saying
  where the horizon itself lives.
- Change: every provider fact — support tier, stated end date, prebuilt-artifact
  policy, artifact-variant naming, the curated stateful families, the runtime
  managers and the curated equivalences — lives in `data/policy/*.toml`, each
  entry carrying `source` and `read_on`, both validated at load time so that an
  entry without provenance fails to load rather than failing a test. The
  horizon is a constant in code (`SUPPORT_END_HORIZON_DAYS`, two years),
  because it is a policy choice of this project's, not a fact about a provider.
  The rule's weight rises linearly across it and is a reason parameter rather
  than a priority.
- Rationale: INV-20's line is between what a provider published and what this
  project decided. The September 2027 date is the former and must be
  re-checkable without a release; how far ahead this tool starts warning is the
  latter and has no external source to cite. Making the weight a parameter
  rather than a priority keeps a deadline from becoming a different rule as it
  approaches, which would make two runs a month apart incomparable.
- Evidence: `data/policy/provider-support.toml` entry
  `hb-x86_64-tier3-with-stated-end` (source `https://docs.brew.sh/Support-Tiers`,
  read 2026-09-02, `end_date = "2027-09-01"` encoding "in or after September
  2027"); `docs/research/F-1-homebrew-support-tiers.md` findings 3 and 5;
  `PolicyData::load_from` rejects an entry with an empty `source` or an
  unparseable `read_on`; `invariant_20_no_provider_deadline_is_written_into_source`
  greps the crate's own sources for a deadline year and finds none.
- User impact: the staleness notice and the deadline both move by editing data.
- Architectural impact: the policy crate performs no run-time I/O — the data is
  `include_str!`-embedded — and no clock read: `today` is a field on `Facts`
  and date arithmetic takes both ends as arguments.
- Compatibility impact: `policy.toml` carries `version = "2026.09.02-1"`, which
  every `Recommendation` and `Derivation` reports.
- Specification edit: none owed. This is the existing MUSTs implemented, with
  the horizon's home recorded because the text did not say.

## SE-041 — Coexistence is decided inside the rule, never rescued after it
- Status: accepted
- Date / Pass: 2026-09-02 / policy-layer implementation
- Original assumption: `03-policy-and-recommendations.md` requires the engine to
  be a pure function and every recommendation to carry its reasons, and INV-18
  makes `KEEP_BOTH` a first-class end state. The first implementation added an
  engine-level guard that rewrote any winning outcome naming a provider whose
  support tier is `UNSUPPORTED` into "keep it where it is".
- Change: the guard is removed. Every rule that can name another provider
  checks the support tier itself — including both branches of
  `KEEP_BOTH_HEALTHY`, which previously checked it on the requested-coexistence
  path and not on the already-installed-under-both path — and the ranking that
  picks a primary scores an unsupported provider at zero so it can never be
  named. The engine now only chooses among rule verdicts and never edits one.
- Rationale: the guard was wrong twice over. It demoted a legitimate
  `KEEP_BOTH` for a package installed under two healthy providers whenever the
  unsupported side happened to rank highest, turning a first-class end state
  into an intermediate one — the exact thing INV-18 forbids. And it acted
  outside the rule set, so `explain` could not list it among the rules that
  fired: a recommendation could be changed by something no user could see. A
  rule with a missing condition is fixed by adding the condition, not by
  correcting its output afterwards.
- Evidence: `invariant_18_coexistence_is_a_first_class_outcome` in
  `crates/policy/tests/invariants.rs` asserts that no outcome the engine
  produces across the scenario matrix names an unsupported provider, and checks
  the specific shape the guard used to mishandle; `KEEP_BOTH_HEALTHY`'s tests
  `a_tie_leaves_the_side_already_holding_it_in_front` and
  `the_same_facts_with_the_sides_exchanged_swap_the_primary`. A related
  symmetry defect was found the same way: the primary was originally tied
  toward the lexicographically smaller provider id, which made one provider win
  every tie by alphabetical accident and failed
  `invariant_17_symmetric_rules`; the tie now goes to the side already holding
  the package, which is a fact rather than a name.
- User impact: a package present under both providers is reported as such,
  with a suggested primary, instead of occasionally being told to keep only
  one.
- Architectural impact: establishes that the engine composes rule verdicts and
  never overrides them, so everything a recommendation says is attributable to
  a rule that `explain` lists by id. The unattributed fallback reason the
  engine used to synthesise when no rule fired was removed for the same
  reason; `Recommendation::new`'s assertion plus the always-firing
  `KEEP_SOURCE_DEFAULT` rule is what guarantees a non-empty reason list.
- Compatibility impact: none shipped yet.
- Specification edit: none owed. INV-18 and the existing MUSTs already require
  this; the entry records that a first implementation got it wrong and how it
  was caught.

## SE-042 — The package unit across providers: which side drives it, and what an analysis of a snapshot is "today"
- Status: accepted
- Date / Pass: 2026-09-02 / 01-FOUNDATION-PASS
- Original assumption: `02-domain-model.md` makes the unit shown to the user
  "the package across providers" and `03-policy-and-recommendations.md` gives
  the engine one `source` provider per package, without saying which side is
  the source when the package is installed under both, nor which reverse
  dependencies count. `06-cli-and-ux.md` says `analyze --from-snapshot` must
  be byte-identical (INV-15) while the engine takes the current date as a
  fact.
- Change: (1) a unit held by two providers is driven by the provider that
  owns the executable `PATH` resolves for it; when no executable resolves,
  by the first provider id in sorted order, and that fallback is the one
  place an id order decides anything. (2) **Withdrawn 2026-09-02 by SE-045**, on the
  owner's ruling (DP-010). It read: the reverse-dependency count the engine sees
  is the largest count over every provider holding the package. A maximum across
  providers is a cross-provider calculation, which INV-04 and INV-08 forbid, and
  it reported a count under a provider it had not been counted in. The count is
  per-provider now; see SE-045, which also records why this was a collision
  between two Tier 1 invariants rather than an oversight.
  (3) An analysis of a snapshot file is computed as of the snapshot's capture
  date unless `--today` is given, so the output is a function of the file
  alone; a live analysis uses the real date.
- Rationale: (1) the executable that runs is the only machine fact that
  distinguishes the two copies; choosing by name would make one provider win
  every tie (INV-17), which `invariant_17_swapping_the_providers_swaps_the_analysis`
  caught on the first implementation. (2) Superseded with the change; the
  reasoning that a plan "may remove either copy" does not describe the code -
  a plan removes exactly one named side, and the planner already read that side
  correctly. See SE-045. (3) Without it the same
  snapshot would give different staleness notices and deadline weights on
  different days, and INV-15 would hold only by accident of the calendar.
- Evidence: `crates/application/tests/invariants.rs` (INV-17 test failing
  with `KeepBoth` vs `ManualReview` before the change, passing after);
  `crates/application/src/analysis.rs` (`source_for`, the reverse count);
  `crates/cli/src/app/context.rs` (`today` from `provenance.captured_at`).
- User impact: a package under both providers is judged from the copy the
  user actually runs; `analyze --from-snapshot` reproduces exactly.
- Architectural impact: the analysis stays a pure function of
  (snapshot, policy data, options); the date is an option.
- Compatibility impact: the `--json` payload carries `today` so a reader can
  tell which date an analysis was computed for.

## SE-043 — A package its own provider has withdrawn is a review item, not "nothing to do"
- Status: accepted
- Date / Pass: 2026-09-02 / 01-FOUNDATION-PASS
- Original assumption: `03-policy-and-recommendations.md` lists deprecated or
  disabled state *on the destination side* among the `MANUAL_REVIEW`
  triggers and says nothing about the source side; the first policy set
  therefore answered `NO_ACTION` for an installed formula its provider had
  disabled.
- Change: `MANUAL_REVIEW_SOURCE_WITHDRAWN` fires when the provider holding the
  package marks it deprecated, disabled or superseded, naming the state and
  the successor when one is published.
- Rationale: the package works today and will not be replaced; "nothing to
  do" hides the one fact the user needs to act on, and `03` makes
  `MANUAL_REVIEW` the default whenever availability is uncertain.
- Evidence: the derived fixture
  `fixtures/recordings/homebrew-deprecated-and-tap-intel-macos26.json` (a
  formula edited to `disabled: true`) produced `NO_ACTION` before the rule
  (`scripts/run-fixture-matrix.sh` golden diff, 2026-09-02);
  `crates/policy/src/rules/manual_review_source_withdrawn.rs` tests.
- User impact: one more `MANUAL_REVIEW` row, with a reason that names the
  state.
- Architectural impact: none; one more rule in `rules::all()`.
- Compatibility impact: a new reason id, `reason.source_withdrawn`.

---

## SE-055 — Every GitHub Action is pinned to a commit SHA, and the pin is checked
- Status: accepted
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW, implementation
- Original assumption: SE-025 rejected pinning Actions to commit SHAs from inside
  an agent session, on the ground that there was no honest network path by which
  a session could learn what a tag pointed at, and a guessed SHA is worse than an
  honest tag. Pinning stayed deferred across two passes.
- Change: every `uses:` in `.github/workflows/` names a 40-hex commit SHA with a
  trailing `# <version>` comment, and `scripts/check-action-pins.sh` — wired into
  `scripts/validate.sh`, so it runs in `--quick` and needs no toolchain — fails
  the build on a tag, a branch or a bare SHA with no readable comment. The
  `cargo-deny` invocation also gains `advisories`, which `deny.toml` had declared
  and the workflow had never run. `claude-code-review.yml` gains an
  `author_association` guard and the `pull-requests: write` its inline-comment
  tool needs.
- Rationale: the rejection's premise stopped being true. `api.github.com` answers
  what a tag or branch resolves to, which is the authoritative source rather than
  a guess, so the SHAs below were read rather than invented. The pins matter more
  than the usual secret argument suggests: `repository-validation.yml` carries no
  secrets and only `contents: read`, but it is the job that runs
  `check-deployment-target.sh`, so its integrity is load-bearing for a
  *correctness* claim, not only a confidentiality one. Four of the twelve refs
  were mutable by name — three `dtolnay/rust-toolchain@master` and
  `taiki-e/install-action@cargo-llvm-cov`, the latter a moving per-tool tag that
  also downloads a binary into the job, and neither previously reported. That
  one is pinned to the **generic `v2` commit with `tool: cargo-llvm-cov` named
  explicitly**, not to the per-tool tag's own commit: the tag resolves to a
  published variant whose `action.yml` carries the default, so pinning it by SHA
  does work, but it makes the tool a property of which commit was chosen rather
  than of anything readable in the workflow. The
  `advisories` omission meant `[advisories] yanked = "deny"` was dead
  configuration that read as enforced. The workflow permission was a plain
  defect: the job asked for `--comment` while holding `pull-requests: read`, so
  it could only ever run and produce nothing.
- Evidence: SHAs resolved from `api.github.com/repos/<owner>/<repo>/git/ref/...`
  on 2026-09-02, annotated tags dereferenced to their commit —
  `actions/checkout@v6` `d23441a4…af803`, `anthropics/claude-code-action@v1`
  `fa2b2666…3c8f`, `EmbarkStudios/cargo-deny-action@v2` `3c634983…5f25`,
  `dtolnay/rust-toolchain@master` `6c977a6c…8772`,
  `taiki-e/install-action@cargo-llvm-cov` `b1d53d4d…0f64`. The check was verified
  in both directions: it reports `ok (12 pinned)` on the pinned tree and fails
  with a named file and ref when one is reverted to `@v6`. Findings F-16, F-17,
  F-18.
- User impact: none directly. A contributor opening a fork pull request no longer
  gets a red check they cannot fix, because the review job no longer starts for
  them rather than starting and failing for want of a secret.
- Architectural impact: one more check in the one definition of valid. Updating an
  action is now a deliberate act with a diff, which is the point; Dependabot's
  `github-actions` ecosystem updates SHAs and preserves the comment.
- Compatibility impact: none. `SE-025`'s rejection of pinning is superseded by
  this entry and its reason is recorded here rather than deleted.
- **Who triages an advisory.** `check advisories` fetches the RustSec database at
  run time, with no `ignore` list and no pinned `db-urls`, so a new advisory
  against any locked crate — or a transient fetch failure — turns every push red
  with no change in this repository. That is the intended behaviour of advisory
  checking and not a defect, but it means the first red build of that kind is a
  triage task and not a regression: read the advisory, and either update the
  dependency through ADR-0004's gate or record an `ignore` with its reason and a
  date. Nobody had that written down before this entry.

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
  00-REPO-READINESS-PASS; re-verified 2026-09-02 / 01-FOUNDATION-PASS
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
  not done at the time: pinning the GitHub Actions to commit SHAs — the
  reasoning was that no session had a network path to read the SHAs honestly,
  and a guessed SHA is worse than an honest tag. **Superseded by SE-055**
  (2026-09-02): the premise was wrong rather than the conclusion, because
  `api.github.com` answers what a ref resolves to, and all twelve `uses:` are
  now pinned and checked. Two decisions
  about what the boundary does *not* do: `permissions.defaultMode` is deliberately unset, because plan mode
  gates on a human approving the plan and the owner runs passes
  autonomously; and Claude Code's sandbox — the documented OS-level layer
  that constrains subprocesses regardless of the agent's decision — is a
  SHOULD for the Foundation Pass to evaluate against the real build rather
  than enabled blind. `HOMEBREW_NO_INSTALL_FROM_API=0` was removed from the
  environment block: it set the documented default and explained nothing;
  R-1 records that the quieting variables in place do not stop `brew info`
  from tapping on an unknown name, and the suppressing setting is that
  research item's to find. Foundation Pass recheck (same day, second run):
  one contract defect — the `@claude` workflow passed `.claude/settings.json`
  as `settings:`, and the github-actions page documents a settings allow
  rule as an alternative grant to `--allowedTools`, so the run had been
  granted `cargo build`/`test` after all; it now passes `--disallowedTools`
  for every cargo verb that compiles or runs pull-request code. Settled: plan
  mode does not constrain `Bash`, so the `PreToolUse` guard is the boundary
  during a review. Adopted: `additionalContext` is truncated to the documented
  10,000-character cap. Recorded once, not adopted, with the reason:
  `permissions.disableAutoMode: "disable"` would change how the owner runs
  sessions (the Foundation Pass itself ran under auto mode) and is the
  operator's choice like `defaultMode`; a `SubagentStop` gate would block a
  research subagent on a build failure the main session is mid-way through
  fixing; the `sandbox` block stays a documented, evaluated recommendation
  (`.claude/README.md`) because it takes effect at session start and could
  not be exercised against the build from inside the pass that wrote it.
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
  `research/R-1-homebrew-read-interfaces.md`; Foundation Pass recheck notes
  `research/tooling/2026-09-02-*-foundation.md` (github-actions,
  permission-modes, hooks and sandboxing pages, read 2026-09-02).
- Compatibility impact: the hooks require `jq` or `python3` on a
  contributor's machine, and say so when neither is present.

## SE-026 — A definition-of-done gate that blocks, rather than a checklist that asks
- Status: accepted
- Date / Pass: 2026-09-02 / agentic SDLC review; corrected 2026-09-02 /
  00-REPO-READINESS-PASS; re-verified 2026-09-02 / 01-FOUNDATION-PASS
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

## SE-034 — Tier 1 wording corrections, made under explicit owner authorization
- Status: accepted
- Date / Pass: 2026-09-02 / 00-REPO-READINESS-PASS (owner authorization
  given in the session that ran the pass); extended 2026-09-02 /
  02-ARCHITECTURE-REVIEW-PASS
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
- Extended 2026-09-02 (02-ARCHITECTURE-REVIEW-PASS), a third correction on the
  same terms. INV-10's parenthetical rationale justified a sanitised environment
  by saying that build systems probe `/usr/local` for headers and libraries.
  That is true and it is not a reason for this rule: clearing environment
  variables does not stop a compiler from searching a path compiled into it, so
  the stated reason described a hazard the invariant does not address. F-2 found
  the filesystem half already covered by the providers themselves — MacPorts
  keeps an allowlist and offers opt-in trace mode, Homebrew's compiler shim
  always drops `-I` and `-L` flags pointing at the other prefix. The rationale
  now gives the reason that does hold: the developer's shell may export flags
  pointing at the other prefix, a provider invoked with those inherited behaves
  differently than it does on its own, and MacPortsBrew must not be the source
  of that distortion. **The invariant's requirement is untouched**; only its
  justification changed, and the owner authorized exactly this edit.
- Refined twice more the same day, on the same terms, and the second refinement
  is the interesting one. First: "reports **and builds** differently" overstated
  the claim, because F-2's E-4 records that MacPorts keeps only an allowlist of
  environment variables during its own builds and that allowlist contains none
  of the six INV-10 names. The word became **"behaves"**. Second, at the owner's
  prompting: the replacement rationale had grown to eight lines summarising F-2 —
  which provider mitigates the filesystem hazard, how, and how much — and every
  remaining accuracy complaint sat in that summary ("rather than" where F-2 says
  "not primarily"; "mitigates" for MacPorts' opt-in, default-off trace mode;
  "always-on" for a shim that is always on *for source builds*). **The rationale
  was cut to two sentences and the summary deleted rather than corrected.** An
  invariant states the rule and points at the evidence; restating a research
  note inside Tier 1 text creates a second copy that drifts from the sourced
  one, and it was already drifting. F-2 carries the nuance, with its dates. The
  result also matches the house style INV-19 already sets.
- Recorded here, all three times, because a Tier 1 edit made to *narrow* or
  *shorten* a claim is still a Tier 1 edit.
- Evidence: for the third correction and its two refinements, the owner's
  confirmation in the 02-ARCHITECTURE-REVIEW-PASS session of 2026-09-02, to be
  indexed in that pass's report. For the first two, the owner's confirmation in
  the readiness-pass session,
  2026-09-02, indexed in `passes/00-REPO-READINESS-PASS-report.md`; F-2 in
  `09-research-checklist.md` (MacPorts Guide §3.1.12,
  <https://guide.macports.org/chunked/using.html#using.port.install>, read
  2026-09-02 — F-2 records that
  `trac.macports.org` served a bot-mitigation interstitial to every URL form
  tried that day, so the FAQ is not the citable source it was taken for); ADR-0002 for the language.
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

---

# Raised by 02-ARCHITECTURE-REVIEW — accepted, awaiting filing under SE-032

**All 20 entries below are `Status: accepted`.** None is proposed.

The heading and the two counts said otherwise until 2026-09-05: it read
"not yet accepted", claimed "these six entries are `Status: proposed`", and
then cited "all eleven". Four mutually inconsistent statements about the same
block — and the harm ran the wrong way from the section's stated purpose. That
purpose was that "a proposal that sits beside accepted rationale reads as
settled"; what had happened was the inverse. Twenty settled decisions, four of
them owner Tier-2 rulings (SE-044/DP-011, SE-046/DP-012, SE-067/DP-008,
SE-071/DP-014), sat under a heading announcing they were not. An agent
following `CLAUDE.md`'s instruction to read this file "so you do not reopen
settled ground" would have reopened exactly that ground.

**Outstanding, and deliberately visible rather than silently done:** SE-032
requires each of these to merge into its topical section above, keeping its
identifier. That merge has not happened for any of the twenty. It is a
mechanical edit across a long file, so it belongs to a pass that can verify the
result rather than to the correction that found the discrepancy. Until it does,
read this section as part of the accepted record, not as a queue.

Evidence for all twenty is in
[`passes/02-ARCHITECTURE-REVIEW-findings.md`](passes/02-ARCHITECTURE-REVIEW-findings.md),
finding ids in each entry.

## SE-044 — Homebrew's "Tier 3" and "Unsupported" are different things, and the charter conflated them
- Status: accepted (Tier 2 — decided by the project owner, 2026-09-04, DP-011)
- Date / Pass: 2026-09-02, decided 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: `00-charter.md:37` and `README.md:31` — "Homebrew
  classifies Intel x86_64 macOS as **Tier 3 — not supported**".
- Change: `00-charter.md`, `README.md` and `wiki/Intel-Mac-Homebrew-Support.md`
  now name Tier 3 as Homebrew names it, quote what Homebrew's separate
  *Unsupported* category means, and state that Intel is not in it — Mojave 10.14
  and earlier is. All three keep the September 2027 date and its citation
  unchanged, and the charter adds the sentence the data always implied: this
  project's policy data says `DEGRADED` and `status` prints `degraded`, so the
  chapter says the same.
- Rationale: the source treats them as distinct categories. Intel on Catalina
  10.15 through Tahoe 26 is Tier 3 and Homebrew runs there today; "Unsupported" is
  Mojave 10.14 and earlier, defined as a configuration where Homebrew "will not run
  without third-party patches or modifications". The project's own data model gets
  this right — `provider-support.toml` maps Intel Tier 3 to `DEGRADED` and only
  `macos_max = "10.14"` to `UNSUPPORTED` — so the binary is more honest than the
  prose. The error runs in the direction that favours the tool, which is the
  direction that costs credibility when a reader checks the citation.
- **The recommendation this entry carried was itself wrong, in the same
  direction.** It said to write "Tier 3 — degraded, with bottle builds stopped
  and an announced end date". *Stopped* is the second half of the same defect:
  `README.md` had already corrected it to *frozen and eroding* after F-1 finding 4
  observed new Intel bottles published in August 2026, and
  `provider-support.toml` records `prebuilt_artifacts = true` with a note saying
  in terms that "stopped" is not the honest characterisation. Implementing the
  recommendation literally would have removed one overstatement by restoring
  another, in a Tier 2 document, under an owner's approval given for the
  substance. What landed is the substance — the prose agrees with the data — and
  not the words. Two entries proposing a correction is not two independent
  checks of it.
- Evidence: <https://docs.brew.sh/Support-Tiers>, re-read 2026-09-02: "A Tier 3
  configuration is not supported. These configurations fall far outside
  Homebrew's testing infrastructure"; "An unsupported configuration is one in
  which: Homebrew will not run without third-party patches or modifications".
  Both verbatim in `docs/research/F-1-homebrew-support-tiers.md` (E1). The
  timeline sentence and the September 2027 date are unchanged and remain
  correctly cited. Finding F-11.
- User impact: a reader on an Intel Mac is no longer told their installation is
  already unsupported when it is degraded.
- Architectural impact: none. Data already models it correctly.
- Compatibility impact: none.

## SE-046 — Ownership with no provider record is `UNKNOWN`, not an inferred `TOP_LEVEL`
- Status: accepted (DP-012, decided by the project owner 2026-09-05)
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW
- Original assumption: `AGENTS.md`, `.claude/rules/adapters.md` and SE-042 all state
  that casks carry no intent field, so ownership is `UNKNOWN` and is never inferred
  from a dependency graph.
- Change: either `classify_ownership` returns `Unknown` when there is no intent
  record, or the written rule is relaxed to "never an *unrecorded* graph inference"
  and the documentation stops promising `UNKNOWN`.
- Rationale: the code folds "no record" into `TopLevel` and `classify_ownership`
  cannot return `Unknown` at all. Nine casks on the reviewer's machine — 16% of 56
  top-level rows — enter the default destructive-workflow surface (INV-13) on
  `authoritative: false` evidence. The inference *is* recorded as non-authoritative,
  which is better engineering than silent guessing; the defect is that two
  documents and the code state different rules. Whichever way it goes, changing the
  default view's population is a scope decision.
- **Extended 2026-09-04 (R-12): the premise under both options was wrong.**
  Homebrew records `installed_on_request` for casks in the on-disk
  `INSTALL_RECEIPT.json` — 7 of the 9 here carry one, all `true`. Only the **cask
  JSON** has no such field, which is all R-1 §E-11/§E-12 ever established. The
  absolute had reached nine places including a Tier 3 MUST in
  `02-domain-model.md` and the `analysis.v1.md` contract; all nine now state the
  interface rather than the provider. **F-28 is the narrative** — how a sourced
  claim came to be wider than its source, and why no gate here could catch
  that — and this entry does not repeat it.
- **So there is a third option: read the receipt** — `true` gives `TOP_LEVEL`,
  `false` gives `DEPENDENCY`, absent gives `UNKNOWN`. It relaxes no document and
  guesses nothing, and it costs two rows here rather than nine. **It is blocked
  by INV-15, not by the charter.** The first version of this entry said the
  charter's "public, supported command-line interfaces" clause made it a Tier 2
  question; `spec-guardian` falsified that with shipped code. The full narrative
  is F-28; this entry is the pointer.
- Evidence: `crates/domain/src/ownership.rs:72-82`; observed distribution; F-20;
  F-28; `docs/research/R-12-cask-migratability.md`.
- User impact: unchanged in shape. The measured cost of option (a) is nine rows
  without the receipt and two with it.
- Architectural impact: `Ownership::Unknown` becomes reachable for installed rows.
  The third option adds a **fifth** unguarded live-filesystem read to the
  adapters — F-25's open class — so under `--replay` a cask's ownership would
  depend on the replaying developer's own Caskroom. That is the invariant
  problem, and F-25 has to land before option (c) can.
- **Built 2026-09-05, both halves.** The adapter reads the cask receipt
  (`cask_requested`), `classify_ownership` answers `Unknown` where no record
  exists, and the owner decided the half this entry left open: an `UNKNOWN` row
  is **shown** in the default view. So `Ownership::is_top_level` is now strictly
  `TOP_LEVEL` — "the provider's own record says the user asked for this", which
  is what `topLevelCount` reports — and `Ownership::shown_in_normal_mode` is the
  wider predicate behind `topLevelUnits`. One method had been doing both jobs
  because their answers coincided until `Unknown` became reachable.
- **Every way the receipt read can fail answers `None`.** Absent, oversized,
  symlinked, unreadable, not JSON, or a field that is not a boolean. `false`
  would say the user did not ask for the package, which makes a dependency out
  of a file the reader could not open. Four tests cover the seven cases and the
  `Elsewhere` scope, where the answer is `None` because the recording is the
  machine (INV-15).
- Measured: 27 ownership values across six golden fixtures move to `UNKNOWN`
  with mechanism `no_provider_record`; `topLevelCount` drops 56 → 47;
  `topLevelUnits` does not move, which is the owner's decision working — nothing
  left the default view.
- **The security review found one thing that mattered and it was mine.** The
  first version of `cask_requested` stat'd the path and then opened it again by
  name — so the size cap applied to an object that need not be the one read, a
  FIFO or `/dev/zero` swapped in between the two calls would hang or exhaust
  `analyze`, and a file that grew defeated the bound outright. This crate
  already contains the correct shape *and its reasoning*, in
  `persistence::snapshot_store::read_bounded`'s doc comment. The rule was
  written down three modules away and did not reach the second implementation,
  which is now F-30. Fixed: pre-check refuses a symlink or FIFO before the
  open, then the name is resolved once and `dev`/`ino`, `nlink`, the size and
  the read are all tested against the handle. Two more tests, and the three
  reversals of the owner's decision were probed and refused.
- **And two claims wider than their evidence.** The prose said the reader
  refuses "something pointing outside the prefix", which it never checked; and
  DP-012 and the CHANGELOG said "the schema already defined them differently",
  quoting "units shown in normal mode" — a Rust doc comment, not the schema,
  whose Notes cell for `topLevelUnits` was empty. The schema says it now.
- Compatibility impact: no `--json` field renamed, no value removed, and
  `UNKNOWN` was already a documented `ownership` value, so `schemaVersion` does
  not move. `topLevelCount` changes meaning for casks — narrower, and matching
  the definition it always carried. Golden files change.

## SE-049 — A verification check must be able to fail
- Status: accepted
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW
- Original assumption: the verification contract is expressed by the
  `VerificationCheck` values a plan carries.
- Change: add a `PackageAbsent` check and use it for `VERIFY_REMOVAL`; make
  `VERIFY_DESTINATION` record the destination's pre-state so "already there" is
  distinguishable from "this install succeeded"; make `check_ordering` pairwise
  rather than first-position.
- Rationale: `VERIFY_REMOVAL` currently carries `ProviderReportsActive`, documented
  as "reports the package installed and active" — as data the plan says to verify a
  removal by confirming the package is still present. `VERIFY_DESTINATION` is
  satisfiable by state that predates the plan, so a no-op or failed install clears
  the gate INV-01 depends on. `check_ordering` compares first occurrences, so a
  multi-package plan can interleave a removal before its own guard and still pass.
  None of this is reachable today because nothing executes and every plan holds one
  of each step — which is precisely why it should be fixed before the executor pass
  rather than during it.
- **`PACKAGE_NEWLY_PRESENT` is emitted conditionally, and its absence is the
  message.** Where the destination already holds the package the plan does not
  claim a proof it cannot give: it asserts presence and says, in the add step,
  "already installed; nothing to add". A weaker guard stated is worth more than a
  stronger one implied, and the executor pass can then decide what a no-op
  install is allowed to clear.
- **The pairwise ordering check is deferred, and the guard refuses one shape of
  the ambiguity rather than the ambiguity.** `check_ordering` compares positions;
  matching a removal to *its* guard needs the step to name the package it acts
  on. It now refuses any plan carrying more than one `ADD_DESTINATION`,
  `VERIFY_DESTINATION` or `REMOVE_SOURCE`, so a batch plan fails loudly at
  INV-01's enforcement point instead of passing quietly. **It does not close the
  question**: a plan with exactly one of each whose removal concerns a different
  package than the verification would still pass. None is expressible today —
  `selected` is a single candidate, `destination_package` a single value, `1→many`
  resolves to `MANUAL_REVIEW` — and no specification text commits to a batch
  plan.
  Adding `package: Option<String>` to `MigrationStep` was available and would
  have been additive under the same rule; the executor will need it regardless.
  Asserting the precondition is a reversible interim at a gate INV-01 rests on,
  which is the right instinct there, but it is an interim: the pairwise check
  stays owed, and the residual is recorded here rather than only in a pass plan
  while the finding reads "fixed".
- **The tolerant reader is prospective, and saying otherwise was wrong.**
  `VerificationCheck` is internally tagged, so a reader meeting `PACKAGE_ABSENT`
  without `#[serde(other)] Unknown` refuses the whole plan. An earlier draft of
  this entry said that is what makes the change additive under
  `docs/schemas/README.md` — it is not: that rule says a value may be added to an
  enumeration whose consumers **already** handle an unknown one, and this change
  adds both in the same commit, which protects nothing built before it. What
  actually makes no bump owed is narrower and was unstated: nothing in this
  workspace deserialises a `MigrationPlan`, so plan JSON is write-only output
  with no reader to break, and nothing has been released. The tolerant reader
  earns its place for the *next* value, which is reason enough to add it now.
- **And tolerance in this union needed a rule, which it did not have.** An
  unrecognised check was read and then nothing said what to do with it — so an
  executor could iterate the checks it knew, find them satisfied by state
  predating the plan, and clear INV-01's gate. That is F-06's own defect
  re-entering through the door opened to make F-06's fix additive.
  `VerificationCheck::is_satisfiable` is `false` for `Unknown` and
  `MigrationPlan::verification_is_performable` refuses a plan carrying one:
  tolerated on the way in, never satisfied. `.claude/rules/domain.md` already
  said it — *an unknown is never a yes* — and the precedent cited,
  `PathFindingKind::Unknown`, labels a `doctor` finding where the worst case is a
  mislabelled warning. This one is a gate.
- Evidence: `planner.rs:301-307`, `plan.rs:93-94`, `plan.rs:206-216`; finding F-06.
  Four tests: the removal step asserts absence and not presence; the destination
  step carries the newly-present check only where the destination was absent;
  the interleaved plan is refused while the ordinary one passes and the original
  inversion still fails; and an unrecognised check deserialises to `Unknown`.
  Note that the whole suite passed *before* any of these were written, with the
  two verification steps already changed: 494 by
  `cargo test --locked --all-features --workspace`, which is what
  `scripts/validate.sh` runs, against 498 after. (An earlier draft said 493, which
  is not a number either that command or the plain one produces — a wrong figure
  in the sentence whose whole point is evidence.) That is F-19's thesis exactly,
  and the reason these tests are the deliverable here rather than the code.
- User impact: none until execution exists.
- Architectural impact: `VerificationCheck` gains three variants, one of them the
  tolerant reader. INV-01's enforcement point states its own precondition rather
  than resting on it.
- **`PACKAGE_ABSENT` says the right thing and still cannot be performed.** The
  step carries no command, so an executor handed it must derive the query itself
  — and F-06's own words were "either silently invert it **or have nothing to
  run**". The first clause is closed here; the second is not, and the obvious
  implementation is a trap: "nonzero exit means absent" is verification by exit
  status with the sign flipped, and F-22 already records `brew list --versions`
  exiting 1 for a correctly installed cask. The executor pass owes this a
  source-side command and a defined success semantic. Recorded rather than
  implied, because the finding's row now reads "fixed".
- Compatibility impact: plan JSON gains two check values and a tolerant reader,
  so no `schemaVersion` bump — for the reasons above, which are not the ones the
  first draft gave. Noted in `CHANGELOG.md`, which `docs/schemas/README.md`
  requires for a backward-compatible schema change and which the first version of
  this change omitted. `plan.v1.md` documents all three values, the fail-closed
  rule and the `PACKAGE_NEWLY_PRESENT` semantic.

## SE-050 — The no-shell guarantee needs a check, or the claim needs correcting
- Status: accepted
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW; implemented 2026-09-04
- Original assumption: `04-migration-and-safety.md:257-258` — "There is no shell to
  inject into. Enforced by a lint rule, not by discipline."
- Change: a run-time allowlist at the one spawn site, with three narrower
  controls behind it.
  `SystemRunner::run` refuses any program whose leaf name is not one of
  `brew`, `port`, `sw_vers`, `uname`, `sysctl`, `xcode-select`, absolute, free of
  `..`, and — for the two providers — directly inside a `bin` directory.
  Behind it: `CommandSpec`'s fields and its `new` become private to the executor
  module, so a spec can be neither built nor rewritten from outside it;
  `check-layering.sh` continues to hold *where*; and `scripts/check-no-shell.sh`
  asks the allowlist's question textually so a bad literal fails in review.
  The sentence in `04-migration-and-safety.md` is rewritten to claim only that
  no shell *command line* is constructed.
- Rationale: the existing lint asserts *where* a subprocess is spawned, not *what*.
  The executor's only program check is `is_absolute()`, and a shell is absolute.
  What actually held the line was an allow-list in each of the two adapters —
  good, and checked at run time rather than under `debug_assert!` — but that is
  discipline, which the sentence says it is not relying on. A third adapter added by
  a later pass would pass clippy, the layering check and every test.
- **The first build of this was wrong, and a security review took it apart in
  one sitting. What follows is what it measured.** The shape it replaced was
  "the type stops a value becoming a program, the lint stops a literal being a
  shell". Both halves failed:
  - **`CommandSpec`'s fields were `pub`.** A private constructor controls
    construction, not assignment: `spec.program = <runtime value>` after the
    fact reaches the same place, and `cargo check`, `clippy -D warnings`,
    `check-layering.sh` and the new lint were all clean over it. The same line
    defeats **SE-051**, this pass's own INV-10 fix, by inserting
    `DYLD_INSERT_LIBRARIES` into `spec.environment` after `sanitised_environment`
    has run. That is a Tier 1 exposure that the entry as first written would have
    recorded as closed. The fields are private now.
  - **`&'static str` does not mean "written in this source".** It means "lives
    forever". A module-level `OnceLock<String>` initialised from
    `std::env::var` yields one, with no `leak`, no `unsafe`, and nothing for a
    grep to find; `LazyLock` is the same. `Box::leak` was measured too — it
    compiles clean. Enumerating the routes to `'static` is a losing game, which
    is the actual reason the control moved to run time.
  - **A denylist of interpreters cannot be completed.** The first version was
    missing `tclsh` — which `port` itself is written in, so it is the single
    likeliest interpreter for this tool to be tempted into naming — along with
    `xcrun`, `arch`, `awk`, `sed`, `find`, `expect`, `swift`, `login`, `lua`, and
    every versioned name the pattern did not anticipate (`ruby2.7`, `perl5.34`,
    `node22`; only Python was globbed). Three textual defeats besides:
    `concat!("/bin/b", "ash")`, `"/bin/\x73h"`, and a literal broken across
    lines past a line-oriented check.
  - **The `#[cfg(test)]` exemption asserted the wrong property.** It checked
    "at most one attribute" and relied on "test code is one trailing section",
    which are different claims. In the ordinary two-file layout —
    `#[cfg(test)] mod tests;` near the top, tests in a sibling file — the whole
    file below line three went unscanned, for exactly the two thousand-line
    adapters the check most exists to cover. Splitting those tests out is the
    next obvious refactor, so this was not a contrived case.
- **So the control is a run-time allowlist, and the rest is defence in depth.**
  It runs on the `PathBuf` about to reach the kernel, whatever produced it, so it
  survives public fields, `OnceLock`, escapes, `concat!` and any interpreter
  nobody thought of. There are exactly six programs this tool has business
  running; that list fits on one line, and a denylist of everything else does
  not. `every_program_an_adapter_names_is_on_the_allowlist` ties it to the
  constants the adapters actually use, so the two cannot drift apart silently.
  The lint keeps its place because a run-time check cannot fail at commit time,
  in front of a person; it now asks the same allowlist question rather than
  maintaining a list of bad names.
- **On the `leak` check that survives.** It is kept, and it is explicitly *not*
  a proof of anything — an earlier draft of this entry called `Box::leak` "the
  one way to manufacture a `&'static str` at runtime", which is the same species
  of over-claim F-08 is a finding about, made while correcting it. It is one way
  among several. It stays because it is the form that reads as ordinary code,
  and the bare-name variant is matched because `let f = Box::leak;` was measured
  walking past a version that required `leak(`. There is no legitimate use of it
  here; every occurrence of the word in the source is prose about *information*
  leaks.
- **Test code may name a shell, and must.**
  `a_grandchild_holding_the_pipe_does_not_hang_the_runner` spawns `/bin/sh -c` for
  the express purpose of proving the runner survives a process that outlives its
  parent; `is_system_path` and `which` are handed `/bin/sh` as data. A check that
  forbade the tests from naming the thing they test would be answered by rewording
  them, which is the failure mode this project treats as worse than the defect.
  So the allowlist is extended under `#[cfg(test)]` — extended, not bypassed, so
  the tests run the same code the shipped binary runs, with
  `the_shipped_binary_may_spawn_exactly_six_programs` asserting the production
  list is untouched by it. The lint's boundary is the `#[cfg(test)]` that opens
  an inline `mod tests {` running to the end of the file; anything else, the
  two-file split included, is scanned whole.
- **One thing this pass will not evade.** `invariant_06_reading_the_machine_needs_
  no_elevation` refused the elevating binary's name in a list of programs a test
  asserts are *rejected*. The invariant is right and the case was dropped: an
  allowlist refuses every name not on it, so that one needs no separate entry,
  and rewording it to slip past a guard would have been the wrong answer twice.
- Evidence: `check-layering.sh:58`, `process/system.rs:171`; finding F-08. The
  allowlist has three tests in `process/mod.rs` covering the six shipped
  programs, nine interpreters including `tclsh`, and five permitted names reached
  from a directory that is not permitted. The lint was then probed by replaying
  all fourteen defeats above and each of its own rules, every mutation reverted:
  fourteen of fourteen are refused and the clean tree exits 0, the one that
  escaped the first rewrite being the hex escape. There is no test harness for
  the `check-*.sh` scripts — none of the other seven has one — so those probes
  are evidence, not a regression test; the allowlist's are tests. The shebang and
  the meaning of `bash -p` were read on this machine on 2026-09-04 and are cited
  where they are stated.
- User impact: none directly.
- Architectural impact: adding a provider is now one adapter, one registry entry
  and a `&'static str`; there is no way for the new adapter to name its executable
  from configuration, which is the point.
- Compatibility impact: none.

## SE-058 — The user-facing documentation promised more than the design does
- Status: accepted
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: the wiki and README restate the invariants accurately, so
  reading either tells you what the tool guarantees.
- Change: three corrections, filed separately as F-13, F-14 and F-15 and recorded
  together because they are one defect wearing three faces — every one of them is
  a place where a qualifier the specification wrote down was dropped on the way to
  the reader.
  1. `README.md`'s lead said the tool "executes migrations in either direction".
     It now says it builds a reviewable plan, and the sentence that this build
     executes nothing moves *into* the lead instead of sitting fifty lines below
     the fold.
  2. INV-03 is "the system is left at least as usable as before, **whenever
     technically possible**". `wiki/Dependencies-and-Safety.md` headed the section
     "The safety guarantees" and dropped the qualifier; `wiki/FAQ.md` did the same;
     both migration pages restated it unqualified. All four carry it now, the
     heading is "What is guaranteed, and what is not", and each page says plainly
     that neither provider offers transactions.
  3. `wiki/Undoing-a-Migration.md` becomes `wiki/Restoring-After-a-Migration.md`,
     says why the command is `restore` and not `undo`, and carries all three limits
     `04-migration-and-safety.md:144-152` requires rather than one.
- Rationale: `04-migration-and-safety.md:83-84` is a **MUST** — "do not promise
  transaction semantics the underlying package managers do not provide" — and
  `:144-152` is another. Both were violated in the documentation while the code
  they describe was correct, which is the worse of the two ways round: the tool
  behaves honestly and the reader has been told to expect something else. A
  qualifier dropped in a wiki is not a smaller error than one dropped in a
  signature; it is the one the user actually reads.
- **On the missing limit that mattered most.** The restore page carried "cannot
  restore a version a provider no longer offers", which is the *rare* case. The
  common one — upstream has moved on, so a restore gives you something **newer**
  than the snapshot recorded — was not stated anywhere. A reader who understood
  the page would have expected the machine back as it was.
- **On renaming the page.** `08-distribution-and-project.md:188-192` is a SHOULD
  listing the wiki pages by name and asking that titles "use the words people
  search for"; the list is updated, and the word "undo" still closes the page's
  first sentence — *"In this build there is nothing to undo"* — so the search
  argument is not lost. There is no published
  URL to break: DP-007 is decided but the remote does not exist yet, which is the
  one moment renaming a wiki page costs nothing.
- **On the charter, which is Tier 2.** `00-charter.md:29` carries the identity
  sentence the README was quoting, including "executes migrations", and directs
  that it be used consistently across README, docs, wiki and CLI help. It is
  unchanged, and the README now deviates from its wording — as `wiki/Home.md`
  already did, in text F-13 cites approvingly. The charter states what
  MacPortsBrew *is*; a release page states what a release *does*, and at 0.1.0
  those differ. Recorded rather than taken quietly, because a Tier 2 document
  saying "use this consistently" is not something to route around silently. If the
  owner reads it as the wording rather than the substance, this is the entry to
  reverse.
- **Resolved 2026-09-04 (DP-013), and the charter now carries the rule itself.**
  The owner chose option (b): the identity describes the finished product, a
  release surface describes that release, and `00-charter.md` says so in the same
  paragraph that asks for consistency. It also names the failure mode the sentence
  exists to stop — an audit that finds the charter claiming more than the README
  has found the two doing their jobs, and should leave both alone. "Executes
  migrations" stays in the identity, because it is true of the project and not of
  this build, which is precisely the distinction the new sentence draws. On this
  entry's own topic — the identity lead — nothing in `README.md`, `wiki/` or the
  CLI help changed, because they were already right. The same commit does change
  `README.md` and `wiki/Intel-Mac-Homebrew-Support.md`, for DP-011 (SE-044).
- Evidence: `01-invariants.md:22-25`; `04-migration-and-safety.md:83-84, 144-152`;
  findings F-13, F-14, F-15. Audited after the fact, which found two things this
  entry had got wrong. The four new provider claims carried no source and no read
  date, violating `08-distribution-and-project.md:198-200` — another **MUST**,
  broken while fixing two others, and the claims being *true* and making the tool
  promise *less* is exactly why the instinct was to wave them through. MacPorts'
  half was already sourced in `R-5` and simply not cited; Homebrew's has no
  provenance anywhere in the tree and is phrased conditionally rather than
  asserted. And the explanation had been copied into four pages; it is in one now,
  with the others carrying the qualifier and a link, which is what the anchor was
  for. Three of those links then pointed at a bold paragraph rather than a
  heading — found by inspection, and the reason `scripts/check-links.sh` now
  exists. It gained a second failure class on 2026-09-15 (F-46): two headings
  that slug to one anchor now fail the build, because a link to a duplicated
  anchor resolves to whichever the renderer reaches first — silently, and
  differently between renderers — and the checker had been resolving anchors
  without ever counting them. Recognising a heading changed with it: a `#` inside
  a fenced block is no longer an anchor, and neither is one without a space after
  the hashes, so the map is strictly smaller and no link that failed before can
  pass now. The fence tracker was a single boolean that could desynchronise; it was
  replaced on 2026-09-17 by one tracking the opener's character and length, and
  an unterminated fence at end of file is now an error. The residual is recorded
  in F-47 rather than left to be discovered, and as of 2026-09-20 it is two link
  holes rather than a fence one. `08-distribution-and-project.md:246` had
  required link-checking all along and nothing did it. The charter question the audit said should have been
  filed rather than only noted is DP-013. No stale link to the renamed page remains in `wiki/`,
  `docs/`, `README.md`, `locales/` or `crates/`; neither the CLI help nor the
  catalogue uses the word "undo" at all, so the command surface needed no change.
- User impact: the documentation now states the limits the design always had. Some
  readers will find the tool promises less than they thought; that is the point.
- Architectural impact: none. No code changed.
- Compatibility impact: one wiki page is renamed. The wiki has never been
  published.

## SE-059 — A read is not summarised as needing administrator rights
- Status: accepted
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: `data/policy/provider-commands.toml`'s `_gui_app` keys
  exist, in the file's own words, "so the planner never reaches an elevation value
  by omission".
- Change: state the four missing cells — `verify` and `verify_gui_app`, per
  provider — from R-5a; make the loader refuse a table where any verb lacks an
  elevation for either package kind, or states one that is not `none`, `required`
  or `unknown`; and split the human summary so a step whose elevation is
  *unknown* is reported as unknown rather than as needing administrator rights.
  Policy version moves to `2026.09.04-1`.
- Rationale: `verify` reached `Elevation::Unknown` by omission in both entries
  while the comment said that could not happen, and the plan for
  `htop --to macports` ended *"Steps that would need administrator rights: 2, 3"*
  where step 3 is `port -q installed htop`, a read. Folding an unknown toward
  caution is right; **saying** it is a known requirement is not, and under INV-06
  the effect is to teach someone to approve a password prompt for a read. That is
  the habit the invariant exists to prevent, and it is trained by exactly this
  kind of sentence.
- **Homebrew's `verify` is `none` on evidence rather than inference:** `list` is
  absent from the exemption list in `check-run-command-as-root` in the shipped
  `brew.sh`, so running it as root *aborts* — Homebrew 6.0.21, `Homebrew/brew`
  HEAD `560147012b9678b42ef5e83b690f0895552d1366`, observed 2026-09-04.
- **MacPorts' `verify` was `unknown`, and is now `none` on an observation.**
  *Merged 2026-09-06 under SE-032; the paragraph this replaces is kept in
  summary because the reasoning is what makes the change legible.* This entry
  used to argue, at length and in the present tense, that `unknown` was "the
  answer, not a gap" — MacPorts was not installed on the machine, its
  documentation did not settle whether reading the registry elevates, the
  database file's mode was undocumented and umask-dependent, and "a confident
  `none` here would be a guess wearing a citation". Every one of those sentences
  was true when written and three of them are no longer.

  MacPorts 2.12.6 was installed on 2026-09-06 (macOS 26.6.2 build 25G83,
  `/opt/local`, zero ports installed), and R-5a's UNRESOLVED-1 named exactly
  what would settle it. As the non-root user, `port -q installed` exits 0 with
  no output, and `registry.db` is `-rw-r--r-- root wheel` — 0644. So the
  evidence that "pointed but did not state" pointed the right way, the cell is
  `none`, and the observation lives in the entry's `source` and `read_on`
  fields rather than in a comment, because INV-20 wants provenance in a field.
  Policy version moves to `2026.09.06-1`.

  Two things did **not** change and are the reason F-24 was split rather than
  closed. INV-20 still says runtime detection wins over stored knowledge, and no
  artefact implements detection for this cell — that obligation is now **F-35**,
  so it keeps an identifier that is actually open instead of living inside a
  section headed "closed". And the observation was of an *empty* registry: what
  the verify command prints for a port that **is** installed is still unknown,
  which is **F-33** — `port installed` exits 0 whether or not the port is there,
  so a `VERIFY_DESTINATION` cannot fail on exit status.

  One divergence recorded on the way: R-5a cites the registry directory as
  `root:admin 0755`; observed it is `root:wheel`. Mode matches, group does not.
- **The invariant test named in this entry was rewritten, and the first rewrite
  was wrong.** *2026-09-06.* Making the cell `none` removed the last reachable
  `Elevation::Unknown`, so `invariant_06_a_read_is_never_summarised_as_needing_administrator_rights`
  could no longer assert the rendering it was built around. The replacement
  claimed in its own comment to assert INV-06 "more strongly than before";
  `spec-guardian` disproved that by mutation — flipping Homebrew's `verify` to
  `required` makes the binary print "Steps that need administrator rights: 3, 6"
  with step 3 a read, and the whole suite stayed green, before *and* after the
  rewrite. The hole was pre-existing; documenting it as fixed was not.

  INV-06 is now asserted where it is a property of the plan rather than of a
  string: `invariant_06_a_step_whose_job_is_to_read_never_elevates` in
  `macportsbrew-application` walks both directions and refuses any
  `Elevation::Required` or `Unknown` on a command inside a verification step.
  The CLI test keeps the rendering half and pins the **exact** summary line,
  because "starts with the right words and contains a digit" passed the mutation
  unchanged. Both were confirmed red under it rather than reasoned about.
- **An `UNKNOWN` is described as unknown and *treated* as elevating.** Those are
  different questions and the split answers only the first. `elevated_steps` is
  unchanged — Required ∪ Unknown — and it is what an executor gates consent on,
  because an elevation nobody has established is not one that will not happen.
  `MigrationPlan::steps_requiring_elevation` is the narrower set and exists so a
  *report* can make the narrower claim truthfully. Written into
  `04-migration-and-safety.md`'s Privilege section as well as here, because a
  reader meeting only the split reads it as a relaxation.
- **The check as first written could not see the operation it was written for.**
  It iterated `[entry.verbs]`, and `set_primary` appears only in
  `[entry.elevation]` — with no `set_primary_gui_app` beside it, which is exactly
  the shape being forbidden. Nothing routes `set_primary` through `command()`
  today, so it was invisible rather than broken, but the guarantee claimed — that
  the next operation added cannot repeat F-10 — was false for one added to the
  elevation table. It keys off the union of both tables now, the missing cells are
  stated, and the test removes each of them in turn.
- **The finding's own symptom needed the second half.** Stating the value fixes
  Homebrew and does nothing for MacPorts, because `unknown` was rendered the same
  as `required`. `elevated_steps` stays as it is — every step whose elevation is
  not `None`, which is the conservative set and the right one for a
  machine-readable field, and a consumer reading it loses nothing. Only the human
  line changes, and it changes because it was making a claim the data does not
  support. No schema field moved; the per-command display and the JSON already
  distinguished the two.
- **The comment was the defect, not the missing keys.** A file that says "stated
  so the planner never reaches a value by omission" and then omits two values is
  worse than one that says nothing: it invites the next reader to trust the
  property rather than check it. The loader refuses an incomplete table now, so
  the next verb added cannot repeat it, and the same pass hardened `executable`
  while it was there — `Path::join` discards the base when its argument is
  absolute, so `/bin/sh` in that field would have been a program of the file's own
  choosing (F-20).
- Evidence: `planner.rs:598-606`; `docs/research/R-5a-verify-command-elevation.md`;
  finding F-10. `invariant_06_a_read_is_never_summarised_as_needing_administrator_rights`
  reproduces the verify step the finding observed, in both directions, and was
  checked by restoring each defect in turn — emptying the unknown branch and
  emptying the required branch each make it fail.
  It reproduces *one* of the two steps F-10 listed. The finding recorded
  "administrator rights: 2, 3"; step 2 is `AddDestination`, which on that fixture
  emits no command at all ("already installed; nothing to add") and therefore
  cannot have been in `elevated_steps` before this change either. An earlier draft
  of this entry said the test reproduces "the exact plan", which claimed more than
  the code does. The three loader tests each break the shipped table and
  assert the refusal, so the validation cannot pass vacuously. Only the policy
  version moved in the six golden analyses, which was inspected rather than
  assumed.
- User impact: a plan no longer tells you a read needs administrator rights. Where
  the answer is genuinely unknown it says so, in a separate line.
- Architectural impact: `provider-commands.toml` is now refused if incomplete,
  which makes the `add-provider` recipe's elevation step mandatory rather than
  conventional.
- Compatibility impact: `policyVersion` changes, and the six golden analyses with
  it. No `--json` field added, removed or renamed — but `plan.v1.md` described
  `elevatedSteps` as "every step that would need administrator rights", which is
  the F-10 sentence surviving verbatim in the compatibility surface: a consumer
  implementing that faithfully reproduces the finding. Corrected there, which is a
  clarification rather than a breaking change, and it had to land here rather than
  later — shipping a change that declares a claim false while leaving the
  authoritative statement of it standing is the worse half of both options. Both
  entries' `read_on` move to 2026-09-04 and their `source` gains the
  command-level citations, without which `2026.09.04-1` was a version no dated row
  in `data/policy/` accounted for.

## SE-060 — The Homebrew parser gains the bounds the MacPorts parser always had
- Status: accepted
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: both provider parsers apply the same defensive bounds to
  what a provider says.
- Change: the Homebrew parser bounds free text at `MAX_TEXT_LEN` (`desc`,
  `caveats`, `homepage`, `license`, `tap`, and the three reason fields) and
  refuses a document describing more than `MAX_ROWS` formulae or casks. Both
  numbers are the MacPorts parser's, and `MAX_TEXT_LEN` is now the domain's
  single constant rather than a copy in each adapter.
- Rationale: MacPorts capped output bytes, rows and free text; Homebrew capped
  the document at 64 MiB and the values that become *path components*, and
  nothing else. So a third-party tap could put tens of megabytes of `caveats`
  into a snapshot the user is then invited to share, and an arbitrary number of
  rows into one the analysis walks. Path traversal through a name was already
  closed on both sides by `checked_component`, which is the half that would have
  been a vulnerability; this is the resource half, and it is the half that reaches
  a user's file.
- **Truncation for text, refusal for rows, and the asymmetry is the point.** An
  over-long description is a nuisance, not evidence the output format changed;
  refusing a whole document over one would turn a cosmetic oddity into an
  unusable machine, and `bounded_text` on the MacPorts side has always truncated.
  A row count is different: a document claiming a hundred thousand formulae is
  not a machine this tool is being asked about, and quietly analysing the first
  8192 would answer a question nobody asked.
- Evidence: finding F-20. Three tests, each probed by removing the bound it
  asserts — both fail. The third is the boundary in the direction this pass has
  got wrong repeatedly: a document *at* the row limit still parses, because a cap
  that refuses the legitimate case is worse than no cap.
- User impact: none on a normal machine. A pathological document is refused or
  trimmed instead of being carried into a snapshot.
- Architectural impact: one definition of the text bound, in `domain`, where the
  snapshot boundary already enforced it.
- Compatibility impact: a document that previously parsed with an over-long field
  now parses with it trimmed; one with more than 8192 rows of either kind is
  refused with `ProviderOutputChanged`.

## SE-061 — The temporary file's name is unpredictable
- Status: accepted
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: `write_atomic`'s temporary sibling is safe because it is
  created with `O_CREAT | O_EXCL` at `0600` and never follows a symlink.
- Change: the name's variable component becomes a value from
  `std::collections::hash_map::RandomState`, which `std` seeds from the operating
  system, instead of a process-wide counter.
- Rationale: all three of those properties hold and none of them is about the
  *name*. `.{stem}.{pid}.{seq}.partial` is entirely predictable, and `create_new`
  is what turns a name an adversary has already taken into a failed write. In the
  state directory this costs nothing — `0700`, nobody else can create anything
  there — but `write_atomic` also serves `snapshot --out` and `doctor --report`,
  whose paths the user chooses and which are frequently in a shared directory.
  Denial of service only, and one line to remove.
- **Not a clock, and not a dependency.** A clock is guessable and this is not a
  file to introduce nondeterminism into; `rand` would be an ADR. `RandomState` is
  in `std`, is OS-seeded, and needs neither.
- Evidence: finding F-20. **The first version of the test asserted the wrong
  property** — that the 64 names differ, which the old counter satisfied too, so
  the test passed against the defect it was written for. It asserts *spread* now:
  a counter starting at zero leaves the top bits of its field empty for the life
  of the process, and the mutation to a counter fails it.
- User impact: none visible.
- Architectural impact: none.
- Compatibility impact: none — the name is never observed outside the write.

## SE-062 — Six invariant tests that would not have failed
- Status: accepted
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: an invariant with a test named after it is an invariant
  that is defended.
- Change: of the tests F-19 assessed, **four are strengthened** (INV-10,
  INV-07/08, INV-17's swap, INV-20), **one is new** (INV-11, which had no test
  where it is implemented), **one is confirmed already closed** by the F-03 fix
  (INV-13), and **one is not fixed** and is filed as F-25 (INV-15). An earlier
  draft of this line, and the commit subject that carries it, said "six
  strengthened", which the entry's own list contradicts.
  - **INV-10** — the test `continue`d past every program that was not one of the
    four macOS probes, which is to say past every `brew` and `port` invocation.
    INV-10 is about `brew` and `port`. It checks every invocation now, asserts
    the fixture contains at least one provider invocation so it cannot pass
    vacuously, and is renamed for what it covers rather than for the four probes
    it was written against.
  - **INV-07/08** — the comment said "facts fed to the engine count the source
    side only" and the assertion checked the planner's own value in one
    direction. The fixture makes the two directions genuinely differ, so both are
    asserted: moving to MacPorts raises no reverse-dependency risk, moving to
    Homebrew does, and a count that crossed providers would make them agree.
  - **INV-17 (swap)** — the test renamed provider *contents* and kept the map
    keys, so nothing about iteration order was exercised. It asserts the unit's
    `source` flips and that each row mirrors the other in shape — deliberately
    excluding the provider id *inside* an installed package, which is the one
    thing that must not mirror.
  - **INV-20** — scanned one crate of five for one of the four things the
    invariant names. It scans all five for two of them, deadlines and prefixes,
    and says plainly why the other two are not checkable this way: a support tier
    appears in source only as the neutral enumeration, and a package name is
    indistinguishable from any other identifier by grep. A check that looks
    complete is worse than one that states its limit.
  - **INV-11** — the only test named after it argues from the types in the policy
    crate, which is sound and is not where primary selection is implemented. A
    planner-side test now asserts that a plan carrying `SetPrimary` changes no
    dependency count, names no ownership in its JSON, and leaves the dependency
    with its own provider.
  - **INV-13** — F-19 asked for a subset assertion. It already exists:
    `invariant_13_advanced_mode_only_ever_adds_to_the_top_level_view` compares
    the top-level sets themselves across four fixtures and checks each unit's
    `source`, which is strictly more than the one line requested. A second copy
    was written and then removed; a pointer to the test that owns the property is
    what stays.
- **The widened INV-20 check found something on its first run**, which is the
  argument for widening it: `crates/adapters/src/macports/mod.rs` held
  `DEFAULT_PREFIX = "/opt/local"`, a second copy of what `registry.rs` is
  documented as the only place to know. It was unused outside tests, and the one
  test using it compared the copy to itself. Removed; both call sites go through
  the registry. The Homebrew adapter never had one, so this was a peer asymmetry
  as well.
- **INV-15 is not fixed here, and the attempt is worth recording.** F-19 says the
  replayed-reader test covers `which()` only, and that the other live-filesystem
  calls are unreached "by a coincidence of call order, not a guard" — which is
  right. Guarding them on `runner.is_replay()` was the obvious move and it is
  wrong: that flag answers "are subprocess results recorded", and the question
  here is "does this filesystem describe the machine being read". Those coincide
  under `--replay` and come apart in a contract test, which uses a recorded
  runner precisely so it does not spawn `brew` while genuinely testing against a
  scratch tree it created. Built that way, three contract tests failed — not
  because they were wrong, but because the guard was. The right signal is a
  constructor parameter, which is a design change in the layer `AGENTS.md` marks
  dangerous. Reverted, and filed as F-25 with the analysis.
- Evidence: finding F-19. Each strengthened test was probed by reintroducing the
  defect it now covers: a build flag injected into a `brew` invocation in the
  fixture fails INV-10; a prefix in `crates/application` and a deadline in
  `crates/domain` each fail INV-20, in crates the old check never read. The
  INV-17 and INV-11 additions were written against a passing tree, so what they
  buy is coverage of a shape nothing asserted rather than a caught defect —
  stated because "the test passes" is not evidence that it would have failed.
  **And the INV-11 test as first written could not have failed**, which is F-19's
  defect committed inside F-19's fix. Its one assertion about the invariant's
  actual statement sat inside `if let Some(dep) = ...find(|p| p.name == "dep")`,
  and `dep` is a dependency: absent from the default view, as a test two hundred
  lines away in the same file asserts. The block never ran. Beside it, a
  comparison of `analyze(snap)` with `analyze(snap)` restated INV-14's
  determinism. It asserts the *mechanism* now — selection goes through this
  tool's own directory, `CommandTarget::Tool`, at `Elevation::None`, which is
  precisely why no provider's record of what it owns can move — and a
  `SetPrimary` rewritten to `CommandTarget::Provider` fails it.
- User impact: none. No behaviour changed except the removal of an unused
  constant.
- Architectural impact: `registry.rs` is now genuinely the only place naming a
  provider prefix, which is **`AGENTS.md`'s provider-neutrality rule** — not
  INV-20's. An earlier draft of this line said INV-20 "already required" it,
  substituting one rule for the other and closing a finding on the wrong one.
  INV-20 says prefixes live in versioned *data files* with a source and a read
  date; `registry.rs` holds them in a `match` with the provenance in a doc
  comment, which is F-20's open bullet and is not closed here. The check exempts
  that one file and holds the rest of the workspace; **DP-014** puts the
  substantive question — moving them into `data/policy/` crosses a layer — to
  the owner.
- Compatibility impact: no external consumer exists yet. `DEFAULT_PREFIX` and
  `default_candidates` were `pub` items of the `adapters` crate, so this would be
  a breaking removal for one; there is none, and no release.

## SE-063 — The `Stop` gate fails where it used to pass having checked nothing
- Status: accepted
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: the `Stop` hook runs `validate.sh`, so a turn cannot end
  while an objective check fails.
- Change: when `cargo` is not on `PATH` and the turn changed code, tests,
  fixtures, locales or data, the gate **fails** rather than warning. It also
  fails when `scripts/validate.sh` is not executable, and warns when there is no
  `Cargo.toml` at all — a case the old shape skipped in silence.
- Rationale: the guard warned and exited 0, and the previous pass's report then
  states as fact that this machine has no toolchain and that no coverage figure
  was observed. Both were false: `cargo`, `rustc 1.98.0` and the pin were all
  present and `validate.sh` passed in full. That is the cost of a degradation
  that reads like a pass — not a missed check, but a *report* built on one.
  CLAUDE.md's own argument for this hook is that "give the agent a way to check
  its work" beats "tell the agent to be careful"; a way to check that can quietly
  not run is neither.
- **And the scoping did not fire in the case that occurs here.**
  `git status --porcelain` reports what is *uncommitted*, and this repository
  commits mid-turn constantly — the commits closing these findings are the
  evidence. Edit `crates/`, commit, end the turn: clean tree, "nothing changed",
  warn, exit 0. The same defect sat in the mode decision above it, silently
  downgrading the full suite to `--quick`. Both now ask whether the turn touched
  a watched path, which is uncommitted work *or* anything committed since the
  previous position of `HEAD`; with no reflog to consult they answer "unknown",
  which is treated as "assume the worst". `scripts` and `.claude` joined the
  pathspec — they are the gates themselves, which `AGENTS.md` lists first under
  "dangerous to change" — and `MPB_GATE_FULL=1` is honoured, because answering an
  explicit request for the tests with a warning is the same failure in miniature.
- **Scoped, so it is not a hostage.** A documentation turn on a machine without a
  toolchain is not blocked: the failure fires only when the working tree has
  changes under the paths the checks cover, which is the same set the gate
  already uses to decide between `--quick` and the full run. With no `git`
  either, it assumes the worst rather than the convenient.
- Evidence: finding F-20. Probed with `HOME` pointed at an empty directory so
  the hook's own `$HOME/.cargo/bin` prepend cannot find a toolchain: with a
  modified `crates/` file the gate exits 2 and names the reason, with a clean
  tree it exits 0 and warns. Removing `validate.sh`'s executable bit exits 2.
  Those were manual probes, and an earlier draft listed "hook tests 70/70"
  beside them as though the suite covered the change: 70/70 was 70/70 before it,
  and none of the seventy touched a branch this added. There are five cases now,
  each in a scratch repository of its own, because the live tree is dirty
  whenever anyone is working in it — which is exactly when the gate runs.
- User impact: none — this is agent tooling.
- Architectural impact: none.
- Compatibility impact: a turn that changes code on a machine with no toolchain
  can no longer end silently. That is the point.

## SE-064 — i18n parity checks the third direction, and two strings stop being built in code
- Status: accepted
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: `check-i18n-parity.sh` fails on an orphaned key, as its
  own header says.
- Change: the check gains the direction it lacked — an English key nothing
  renders — and the two strings that motivated it go through the catalogue: the
  support tier, which reached the user as `format!("{:?}", tier)` lower-cased,
  and `"{} ({} top-level)"`, which was a sentence composed in Rust.
- Rationale: "orphaned" in that header meant a key a *translation* carries and
  English does not. A key English carries that no code reads was invisible in
  both remaining directions — which is how four deliberate `support.*` labels sat
  unused while the column printed a Rust identifier no catalogue sees and no
  pseudolocale marks. `06-cli-and-ux.md:122-123` makes composing user-facing text
  in code a **MUST NOT**, and `"{} ({} top-level)"` is one: the plural of
  "package" is the catalogue's business, and not every language puts the
  qualifier in parentheses after the count.
- **A gate does not delete a product's prose.** Seventeen English keys are
  rendered by nothing, and they are not junk: three are written ahead of caching
  and network support, which `README.md` lists as unimplemented; nine are a
  `status` table shape the renderer superseded; two belong to a `doctor` heading
  and empty-state line it does not print; two are plural forms with no caller;
  and `cli.alias_note` is the alias note the clap literal should be carrying
  (F-23). An earlier draft of this sentence said "twelve" for the `status` group
  and accounted for thirteen of the seventeen. Deleting deliberate text is
  the owner's call. So each carries a `# unreferenced:` line saying why, the
  check refuses an unmarked one, and it also refuses a *stale* marker on a key
  that is rendered. The reason lives next to the string, where a translator sees
  it, and adding a key without wiring it now forces you to say why.
- **The rest of the `{:?}` bullet, closed here.** Five sites, not the three the
  finding listed — it missed both `Confidence` renderings. A machine's
  translation state, a plan step's kind, a verification check and a mapping's
  confidence all reached a person as the Rust identifier written in the source.
  Each now goes through an exhaustive `match` to a catalogue key, so adding a
  variant is a compile error rather than a silent `{:?}`; the two checks that
  carry data render it through the catalogue too, so a translation decides where
  the executable name sits in the sentence. Thirty keys.
  The test asserts the *property* — no CamelCase variant name survives into
  human output, across `plan`, `status` and `explain` — rather than a list of
  expected sentences, so it catches a sixth site nobody has written yet.
- **And the tier reaches the user from two more places, which the first version
  of this entry did not mention.** `doctor.rs` passed `format!("{:?}", tier)` —
  a display string built in the *application* layer — and `labels::tier` hands
  `DEGRADED` to rules whose sentences a person reads. `locales/en.toml:75-77` has
  claimed since it was written that a parameter carrying a stable domain
  identifier is "rendered through the matching key before substitution", and
  nothing did it. `Messages::render` does now, for `{tier}`, `{class}` and
  `{cause}`; a value naming no key passes through unchanged, because blanking one
  silently would be worse than showing an identifier.
- **The literal scan had to widen too.** Six call shapes were recognised;
  `ErrorId::message_id` writes thirteen ids as bare `match` arms, and every one
  of them looked orphaned under the new direction. Any string literal shaped like
  a message id counts as a use, which is both simpler and harder to slip past
  than a list of call shapes.
- **The gate cannot check that a reason is true, and the first batch proved
  it.** `count.packages` was marked "superseded at its one call site" — it never
  had one; the renderer built the string in Rust from the beginning. The check
  verifies a marker exists and is not stale, never that its sentence is honest,
  so a marker is prose and gets reviewed as prose. Corrected.
- Evidence: finding F-20. Probed four ways, each reverted: a new key with no
  reason fails; removing a marker from an existing one fails; a marker on a key
  that *is* rendered fails; and a marker at the end of one section no longer
  excuses the first key of the next, which it did until a review pointed at the
  `# --- header ---` lines between them. `status` now prints "Supported" where it printed
  "supported", and "190 packages (57 top-level)" from one key.
- User impact: the support column shows the catalogue's deliberate wording, and
  both strings pseudolocalise.
- Architectural impact: none.
- Compatibility impact: `--json` untouched; this is the human renderer.

## SE-065 — A rule named for what it is not
- Status: accepted
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: `NO_ACTION_ALREADY_THERE` describes the rule that fires
  when a package exists under only one provider.
- Change: the rule is `NO_ALTERNATIVE_PROVIDER_PRESENT`, matching the reason id
  it has always carried. File, struct and every reference move with it; the six
  golden analyses regenerate.
- Rationale: "already there" says the package has reached its destination. This
  rule fires when there is no other destination on the machine at all, and its
  reason has said `reason.no_alternative_provider_present` since it was written.
  `explain` exists so a person can see why a recommendation came out as it did,
  and a rule id the reader has to translate is the opposite of that (F-20).
- **Why this is safe to rename, checked rather than assumed.** A rule id is a
  tie-break in the engine's total order (INV-15), so a rename can move a rule
  past another. `PRIORITY_NOTHING_TO_DO` is this rule's alone — no other rule
  shares it — so no tie-break involves it, and evaluation order cannot change an
  outcome because a rule is a pure function of its two arguments. The six golden
  analyses confirm it: the only lines that moved are the 180 carrying the id.
- Evidence: finding F-20. `run-fixture-matrix.sh --update` produced a diff of
  exactly two kinds of line, inspected rather than assumed.
- User impact: `explain` names the rule for what it means. The rule id appears in
  `--json`.
- Architectural impact: none.
- Compatibility impact: a rule id in `explain --json` changes. Renaming a value
  is breaking under `docs/schemas/README.md`; there is no reader and no release,
  and `explain.v1.md` moves with it. Doing it before the first release is the
  cheapest this ever gets.

## SE-066 — Four things F-20 recorded without ceremony, and one deferred with reasons
- Status: accepted
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: the smaller findings are small enough to carry as a list.
- Change: four of them are closed.
  - **`resolution_for` let provider name decide a cross-provider answer.** It
    asks every provider who owns an executable, which is a machine-wide
    question, and returned the first hit in map order. Each provider answers for
    itself, so two can disagree, and `source_for` acts only on an answer that
    names an owner — so an `Unknown` from the alphabetically earlier provider
    discarded the answer that had one, and drove the unit from the wrong row.
    Answers are ranked now: a provider's own record, then a prefix heuristic
    marked as one, then a settled non-provider owner, then `Unknown`.
  - **A plan could carry commands no executor can run and pass every check.**
    `PlanCommand::placeholder` marks them, `MigrationPlan::steps_with_placeholders`
    reports them, the renderer says "illustration only", and `plan.v1.md` says an
    executor refuses such a plan.
  - **Four provider claims with no source and no read date**, in a page where
    everything else carries both. Two were observable here and are now observed
    — every mention of a bottle tag in `brew`'s manual page is a flag that
    *takes* one, and `HOMEBREW_BUILD_FROM_SOURCE` has zero occurrences in the
    shipped tree and zero in the manual page, Homebrew 6.0.21, read 2026-09-04.
    Two were already answered in `docs/research/` and simply not cited.
  - **A published wiki is a separate repository**, so every `../docs/...` link in
    `wiki/` breaks on publication. `wiki/README.md` listed three publishing
    constraints and not the one that will bite; it lists four now, and
    `check-links.sh` counts the escaping links on every run.
- **Promoting `Provider` alone was not the fix, and a review demonstrated that
  by running it rather than arguing.** `System`, `External` and
  `MacportsbrewShim` are named owners too and still lost to an `Unknown` from
  the earlier provider — MacPorts answers `System` for a `/usr/bin` path where
  Homebrew answers `Unknown`, so those two swapped answers when the providers
  swapped roles. Worse, `authoritative` was recorded and not read: a prefix
  heuristic marked `authoritative: false` beat a provider's own record on
  alphabetical order, and my change had newly promoted that heuristic above an
  `Unknown` it previously lost to. Four ranks now, and `authoritative` decides
  between two `Provider` answers. Within a rank the providers agree, or they
  contradict each other about a canonical path, which no ordering here settles.
  The test asserts **both orientations of every pair**; the single-orientation
  version passed with the defect restored, because `homebrew` sorts first.
- **`placeholder` failed open, against the precedent set three commits earlier.**
  `VerificationCheck::Unknown` makes an input a reader cannot parse *become* the
  refusing value; `skip_serializing_if` made the safe value the one never
  written, so absent meant runnable. It is always serialised now — a reader that
  looks never finds it missing — and `plan.v1.md` states the residual hazard in
  the same row, because a boolean has no shape that fails closed on a reader who
  never looks. Inverting its sense would make every real command carry a field
  saying so.
- **Two executor obligations existed only in doc comments**, and this change
  would have added a third. `plan.v1.md` lists all three under "What an executor
  inherits", so the executor pass gets a list rather than three comments to
  find.
- **Why the wiki links are counted and not failed.** They resolve today and the
  pages say to read them here. Rewriting them needs the repository's absolute
  URL, which does not exist (DP-007), so failing would block work that is
  correct. This is not the `Stop` gate's silent degradation in another costume:
  that hid a check that did not run, and this is a check that ran and found a
  future condition. The count is printed because a number nobody prints is a
  number that grows — and the first draft of `wiki/README.md` hardcoded "45",
  which was stale within the hour, so the tool owns the number now.
- **`explain` naming the fact that stopped each non-fired rule is deferred, and
  the reasoning is the deliverable.** The finding calls it a judgement call. The
  version it asks for changes `Rule::evaluate`'s contract — `None` would have to
  carry a reason — across every rule in the crate `AGENTS.md` flags for its total
  order, for a debugging aid. The cheap alternative, rendering a one-line
  description per rule from the catalogue, answers "what is this rule about" and
  not "why did it not fire", which is the question asked. Neither is built; half
  an answer dressed as the answer is worse than a recorded no.
- Evidence: finding F-20. Each change has a test, and each test was checked by
  restoring the defect: first-hit ordering fails
  `invariant_17_an_unknown_owner_does_not_outrank_a_named_one_by_provider_name`,
  and a hardcoded `placeholder = false` fails
  `a_command_no_executor_could_run_says_so` — which is unnumbered on purpose: it
  was `invariant_02_*` until a review pointed out that INV-02 is neither
  violated by deleting the marking nor defended by keeping it, and a name in the
  Tier-1 registry that defends nothing makes the registry unreliable. Two ranks
  were probed by inversion as well: a heuristic promoted above a provider's own
  record fails, and `System` demoted below `Unknown` fails. The placeholder path is
  unreachable through the CLI today, because a destination that is not installed
  halts at `MANUAL_REVIEW` first — which is a property of the caller, so the test
  builds the plan directly.
- User impact: an illustrative command is labelled as one. A machine where two
  providers both answered about the same executable is analysed from the answer
  that names an owner rather than from the alphabetically first.
- Architectural impact: none.
- Compatibility impact: `plan.v1.md` gains an optional boolean, absent meaning
  false, so a reader that ignores it behaves as before.

## SE-052 — The support matrix states configuration until a CI run exists
- Status: accepted
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW
- Original assumption: the generated matrix distinguishes tested from best-effort,
  and "tested" means a hosted runner runs the full suite natively.
- Change: until the first green CI run, the generator emits an unconditional line
  saying no CI job has ever executed and that every row describes configuration
  rather than evidence; "runs" becomes "is configured to run". Mirrored in
  `README.md` and in the `CHANGELOG.md` known-limitations list, which omits it.
- Rationale: the repository has no remote, so the workflow has never executed, and
  the matrix is generated from `runner:` labels — the generator parses nothing else.
  `08-distribution-and-project.md:236-237` names this failure mode in the project's
  own words. The best-effort rows are already handled correctly; it is the tested
  rows that claim an event that has not happened.
- **The "until" is detected, not remembered.** A hand-flipped constant would go
  stale the moment it mattered, so the generator asks `git remote`. That is a
  one-way inference and it is the honest one: with no remote, nothing can have
  run; with a remote, whether a run happened is not knowable from here, so the
  notice disappears while the "this is generated from labels" qualifier stays.
  The qualifier is the part that holds in both worlds, and it was the part
  missing.
- **The MUST itself said the thing the generator cannot do.**
  `08-distribution-and-project.md:229` required the matrix to distinguish "what
  CI actually exercises", which no parse of `runner:` labels can establish. It
  now requires the matrix to distinguish what CI is *configured* to exercise and
  to say which of the two it is reporting — a rule the generator can satisfy,
  rather than one it was quietly failing. The paragraph two lines below, "claiming
  tested support for an untested OS is the kind of dishonesty this project cannot
  afford", is unchanged and is the reason.
- Evidence: empty `git remote -v`; `scripts/support-matrix.sh`; finding F-12.
  Five surfaces carried the claim, not the three the finding named:
  `docs/support-matrix.md`, `README.md`, `CONTRIBUTING.md`, `wiki/Contributing.md`
  — and `.claude/agents/docs-auditor.md`, which told the auditor to check
  documentation against "what CI actually runs", so the reviewer most likely to
  catch this was briefed with the error.
- User impact: the matrix stops overstating what has been observed.
- Architectural impact: none — the change is in the generator, not the artifact.
- Compatibility impact: the checked-in matrix regenerates. The notice is removed
  by adding a remote, so the first push changes this file's output.

## SE-053 — `doctor` gains an owner-writable finding kind
- Status: accepted
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW; implemented 2026-09-03
- Original assumption: the path audit reports world-writable and group-writable
  directories, escalating world-writable ones that precede a system path.
- Change: add `PathFindingKind::OwnerWritable`, reported wherever it sits; emit a
  finding when no system directory appears on `PATH` at all; escalate to `Error`
  the two shapes that mean somebody other than this user decides what runs — a
  world-writable directory before the system ones, and an entry that is not
  absolute. Add `PathFindingKind::Unknown` so a kind stays additive.
- Rationale: the common shadowing directory on macOS is `~/bin` at `0755`, owned by
  the user — currently no finding at all, though it is verbatim the threat-model
  item at `04-migration-and-safety.md:260-261`. On Intel, `/usr/local/bin` is
  group-writable by `admin` and never escalated by position. ADR-0005:210-218
  requires `doctor` to report MacPortsBrew's own installed directory, which is
  user-owned `0755` — so the tool cannot honour a requirement its own release design
  places on it. And when `first_system` resolves to nothing, every escalation
  silently degrades to a warning.
- Evidence: `path_audit.rs:89-94`, `doctor.rs:202-228`, `snapshot.rs:69-91`; F-07.
- User impact: `doctor` detects the shadowing case the tool exists to detect.
- Architectural impact: `PathFindingKind` gains a variant; uid comparison without
  `libc` already exists in `state_dir.rs:164-185`.
- **The proposal said escalate *any* writable kind, and that was wrong.** Built
  that way, `doctor` exited 5 on this machine and would on every developer Mac:
  `/usr/local/bin` and `/usr/local/sbin` are `admin`-writable because that is how
  Homebrew installs itself, `~/bin`, `~/.local/bin` and `~/.bun/bin` are owned by
  the user because that is what those directories are for. It also contradicted
  the check next to it — `prefix_not_on_path` is a *warning for not having the
  provider prefix on `PATH`*, and this made having it there an error, so no
  configuration satisfied both. An error every machine reports is an error nobody
  reads, and this project's bar for `ERROR` is "broken in a way that will produce
  wrong answers".
  So `ERROR` is now the two shapes where the decider is not this user: a
  world-writable directory ahead of the system ones, and a non-absolute entry,
  whose meaning changes with the working directory. Owner- and group-writable are
  *reported* — which is the finding, since they produced nothing at all before —
  as warnings. This is a deviation from what this entry proposed, recorded rather
  than quietly taken.
- **Ownership is half the rule, and the half that makes it mean anything.**
  `OwnerWritable` is not "the owner-write bit is set" — nearly every directory
  has that, `/usr/bin` at `0755 root:wheel` included, and a check that fires on
  every entry says nothing about any of them. It is "writable by *this* user",
  which needs the process's own uid. Reading that from `$HOME` was the first
  attempt and was wrong in both directions, which a review demonstrated: with
  `HOME=/private/var` the tool reported `/usr/bin` as "belongs to you" and missed
  the one directory that really was plantable, and a `HOME` that cannot be
  stat'd switched the check off silently — one line in a shell profile away for
  exactly the adversary who put `~/bin` on the `PATH`. It is the owner of a file
  the process just created instead, which *is* the euid by construction and no
  variable can redirect; the technique is the one `persistence::state_dir`
  already uses. `std` exposes no `getuid`, and `libc` would be a dependency
  decision (ADR-0004), not something to take in passing.
- **The no-system-directory finding is a `DoctorFinding`, not a `PathFinding`.**
  It is a fact about the whole search path rather than about one entry, and
  `PathFinding` carries an entry and an index it would have to invent.
- **A trailing slash used to defeat the whole thing.** `/usr/bin/` is an ordinary
  entry that works, and the system directories were matched by string equality,
  so one character bypassed every escalation *and* fired the new
  no-system-directory error while all four were present and being searched — a
  quiet miss replaced by a loud wrong answer. Compared with trailing slashes
  trimmed now, with a test.
- **Adding a kind was a breaking change wearing an additive face.** A snapshot
  from the new build was refused outright by the old one — "unknown variant
  `OWNER_WRITABLE`", whole file, no analysis. `docs/schemas/README.md` allows an
  added enumeration value without a version bump only where consumers handle an
  unknown one, so `PathFindingKind::Unknown` is that consumer, and
  `snapshot.v1.md` now documents `pathFindings`, which it never did — which is
  why the compatibility question was never asked.
- **What the tests had encoded.** `group_writable_missing_relative_and_duplicate_are_warnings`
  put `/usr/local/bin` first and asserted a warning — the Intel Homebrew machine,
  and the defect itself written down as expected behaviour. The fixture now puts
  the writable entry after the system directories, and the escalation has a test
  of its own covering all three kinds. `a_path_with_no_system_directory_at_all_cannot_be_preceded`
  still passes unchanged: its concern was that "before nothing" must not read as
  "before position zero", which remains true, and the new finding has its own id.
- Evidence: run live on the machine this was found on. Four entries that produced
  no finding of any kind before — `~/bin`, `~/.local/bin`, `~/.bun/bin`,
  `~/.grok/bin` — are now reported; `/usr/local/bin` and `/usr/local/sbin` were
  group-writable warnings and, having both bits, were briefly reported twice each
  until one entry was made to yield one finding. `doctor` exits 0 on this machine,
  as it should. An earlier draft of this bullet said "six errors, none of which
  produced a finding of any kind before", which was false twice over and
  contradicted its own next clause. With `HOME` redirected to a root-owned
  directory the check now still names the real user-owned entry and does not name
  `/usr/bin`; with an empty `PATH` element ahead of the system directories the
  refusal is an error. Home paths reach the report as `~`.
- Compatibility impact: `--json` gains two finding kinds and three finding ids;
  more machines report findings, all but two of them warnings. Additive to
  `snapshot.v1.md` because an unknown kind is now tolerated — without that it
  would have been a version bump. `doctor.v1.md` listed two of the eleven `path_*`
  ids, one of which was `path_order`; it now lists all of them.

## SE-067 — All three release artifacts are unconditional, and each one says what it is
- Status: accepted (decided by the project owner, 2026-09-04, DP-008)
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: `08-distribution-and-project.md:18-20` — the `.dmg` is
  "provisional pending R-11": if a stapled container turns out not to cover a
  binary copied out of it, the `.dmg` "buys nothing the bare binary does not and
  is dropped rather than maintained". DP-008 asked whether the `.pkg`, resting on
  the same premise, was provisional too.
- Change: neither is. All three artifacts — bare binary, `.pkg`, `.dmg` — are
  published unconditionally per architecture and the choice is the user's.
  R-11 now decides what the documentation may **claim** for a stapled container,
  not whether one is published. A new MUST requires each artifact to carry a
  description wherever a user chooses between them, naming it in a technical
  reader's vocabulary and saying what happens in terms that assume no knowledge
  of signing, quarantine or `PATH` — and, because the `.pkg` is forbidden from
  editing `PATH`, requires the documentation to state the directory it installs
  into and the line that puts it there. `README.md` carries the three
  descriptions; `wiki/Getting-Started.md` links to them rather than repeating
  them.
- Rationale: the provisional clause weighed exactly one property, the
  notarisation ticket, and concluded that without it a container "buys nothing".
  That is false on its face: the three differ in how they install — a file to
  place by hand, an installer that places it, a window to drag it out of — and
  that difference survives any R-11 outcome. The original question was posed as
  "is the `.pkg` provisional too?", and answering it either way in the original
  frame would have kept the error; the owner's answer changes the frame.
- **What the descriptions had to be honest about.** The `.pkg` is a user-domain
  install requiring no administrator password, which is the reason to recommend
  it to an unsure reader — and it is forbidden from editing `PATH`, which means
  "it installs it for you" is not the whole truth. The description says both, and
  the new MUST makes the missing line an obligation on the release rather than a
  gap the reader discovers.
- Evidence: `08-distribution-and-project.md:63-71` (the `.pkg` install target,
  user-domain, no password, no `PATH` edit); ADR-0005, whose "Revisit when"
  bullet instructing that the `.dmg` be dropped is now marked superseded;
  R-9 C-2 for "Although tickets are created for standalone binaries, it's not
  currently possible to staple tickets to them" (Apple, read 2026-09-02); R-11
  for the open question.
- User impact: a reader choosing a download is told which one to take and why,
  instead of being shown three filenames.
- Architectural impact: none. Nothing is built yet; this binds the release pass.
- Compatibility impact: none. No release has been published.

## SE-068 — The release design had an inbound security channel and no outbound one
- Status: accepted (the project owner asked for the plan, 2026-09-04, DP-009)
- Date / Pass: 2026-09-04 / 02-ARCHITECTURE-REVIEW
- Original assumption: ADR-0005 named the gap and left it — "that plan is not in
  this record and is a task, not a solved problem" — and nothing scheduled it.
- Change: `SECURITY.md` gains "If a release must be withdrawn": when to revoke and
  when not to, who can, what must be recorded before the first release, a
  six-step runbook, what a user can actually be told, and what the next release
  looks like on a revoked identity. `08-distribution-and-project.md` gains one
  MUST — record the signing identity as CI observed it, per release — because the
  runbook's first question is unanswerable without it. R-13 is filed for the
  premise the whole thing rests on.
- Rationale: the asymmetry DP-009 identified was real and one-sided. Private
  vulnerability reporting is a hard prerequisite of publication; the path *out* —
  how a user learns a release was withdrawn, when the symptom is a binary that
  silently stops launching — had no channel, no gate and no owner. Writing it
  after the first release means writing it during an incident.
- **The premise is unverified, and the plan says so.** ADR-0005 asserts that
  revocation stops previously published artifacts from launching and that the
  secure timestamp does not answer it. Nothing in this tree cites that. Checked
  2026-09-04: Apple's Gatekeeper page contains no occurrence of "revoke" or
  "revocation", and the notarisation page is JavaScript-rendered and returned no
  text. The plan is therefore written to be cheap if the premise is false and
  sufficient if it is true, and R-13 owns the answer. Writing a confident runbook
  on an uncited vendor-behaviour claim would have been the same defect SE-044
  records two entries earlier.
- **The step that carries the plan is step 4, and it is not a technical one.**
  A user hit by revocation sees a macOS dialog that names no project. The
  advisory has to be titled in *their* words or the search that would find it
  returns nothing. Everything else in the runbook is process; that one decides
  whether any of it reaches a person.
- **What the plan refuses to buy.** No telemetry, no auto-update, no required
  network access — charter non-goals, and a notification channel is not worth
  trading one for. The consequence is stated rather than engineered around:
  every channel is pull, and someone who downloaded six months ago and never
  came back learns nothing until it stops working.
- **The security review of the plan found seven things, and the two that matter
  are both about order and reach.** The trigger said "revoke only if the private
  key may be in someone else's hands", which leaves the likelier case outside it:
  an attacker who can *use* the identity in CI never needs to read the key, and
  what they sign stays trusted until revocation. And the runbook revoked first —
  the global irreversible step before the cheap local one — which at 3am means
  every good release broken, the bad one still downloadable, and no replacement.
  Contain first, revoke last, with an exception for an identity being actively
  used. A third: step 2 deleted the release, and with it `SHA256SUMS`, which is
  the only public record of the digests the attestations are looked up by — the
  step justified by preserving evidence destroyed the lookup key. Digests are
  written down before anything is deleted.
- **And three claims about vendor behaviour asserted from memory, in the document
  whose one virtue is flagging exactly that.** "It syndicates to the GitHub
  Advisory Database" is uncited and doubtful for a project publishing no
  ecosystem package; withdrawn, and R-14 now owns it. "Deleting a release leaves
  the tag and the attestations" is the same class and is marked. And the
  fingerprint cannot go "in the build provenance attestation" —
  `actions/attest-build-provenance` emits a fixed SLSA predicate with no field
  for it, so the MUST names the release notes and a sidecar instead. The review
  also caught a miscitation of the charter: it lists **two** relevant non-goals,
  no telemetry and no required network, and *not* the absence of auto-update.
  "MacPortsBrew will never tell them" did not follow from what the charter says;
  the pull-only conclusion does, and is now derived from the two sentences that
  exist.
- Evidence: ADR-0005 (the gap, verbatim); R-9 C-9 for the Account Holder
  requirement, verified verbatim by the review; `docs/spec/00-charter.md`
  "Non-goals", which has two of the three claimed; the two Apple pages re-read
  2026-09-04 and found silent on revocation.
- User impact: none today — nothing is published. It binds the release pass.
- Architectural impact: none. One new release-pipeline MUST.
- Compatibility impact: none.

## SE-069 — A replayed reader needed a signal `is_replay()` could not carry
- Status: accepted
- Date / Pass: 2026-09-05 / 02-ARCHITECTURE-REVIEW
- Original assumption: INV-15 was upheld in the adapters because nothing reached
  the live filesystem under `--replay`.
- Change: `crates/adapters/src/filesystem.rs` introduces `FilesystemScope`,
  threaded through `HomebrewAdapter::new`, `MacPortsAdapter::new`, both
  `detect_prefix` functions, `macports::service::scan` and `RegistryContext`.
  Nine filesystem reads in the adapters now answer from recorded evidence or not
  at all. The CLI sets the scope beside every other `--replay` decision;
  `registry::effective_scope` refuses the one pair that cannot mean anything.
- Rationale: the property held only because `SystemMachine::which` returned
  `None` and nothing else supplied a path — a property of the *caller*. INV-15 is
  a property of the reader. F-25 recorded why the obvious guard is not one:
  `is_replay()` answers "are subprocess results recorded", and what a filesystem
  read raises is "does this filesystem describe the machine being described".
  The two coincide under `--replay` and come apart in a contract test, which
  replays subprocesses while genuinely reading a scratch tree it created.
- **What is deliberately not changed.** `registry::override_for` still gates
  `prefix_override` on `is_replay()`. Its question is whether a real executable
  could be spawned, which is the runner's; answering it with the new type would
  let a caller holding a live runner choose which binary runs, reopening what
  SE-051 closed. The comment there says so, because the inconsistency looks like
  an oversight and is not. `system::machine`'s `which` keeps the old signal too
  — safe, because it errs toward not reading — and moving it is the remaining
  half of F-25.
- **Where the line actually falls.** Only *following a symlink* needs this host's
  disk. A prefix, a Cellar, a Caskroom and a `port provides` answer all come from
  the recording, so classifying against them is pure computation over recorded
  evidence and stays correct. The first version of this change refused to
  classify at all under `Elsewhere`, and four correct MacPorts contract tests
  rejected it. What must not survive is the `authoritative` claim, which rested
  on having followed the link.
- **Every guard was probed by removal**, and the first run caught three of six.
  Each miss was a hole in the test, not the code: an assertion made vacuous by a
  scratch tree missing `bin/port`; a path never exercised; and a case that could
  not discriminate because macOS puts `temp_dir()` under `/var`, a symlink to
  `/private/var`, so the compared paths were never equal. That last one is worth
  remembering — a test can be defeated by the filesystem it is testing.
- Evidence: F-25; the `security-reviewer` pass on this change, which found the
  live-runner pair and the peer asymmetry; F-19, which named the calls.
- User impact: none today; the binary sets the scope the same way it always
  behaved. Under `--replay` a capability that was answered from the replaying
  host is now honestly `Unknown`.
- Architectural impact: a new public type in `adapters`, and a signature change
  across every adapter construction site. Nothing above `adapters` names it
  except the CLI, which owns the decision.
- Compatibility impact: none. `--json` payloads change only where a capability
  that was wrongly `Unavailable` under replay is now `Unknown`.

## SE-070 — A documented sample of the program's output is now the program's output
- Status: accepted
- Date / Pass: 2026-09-05 / 02-ARCHITECTURE-REVIEW
- Original assumption: the samples in `README.md` and `wiki/Getting-Started.md`
  show what the binary prints, kept true by whoever changes the renderer.
- Change: `crates/cli/tests/documented_samples.rs`. A sample is marked
  `<!-- sample: --replay <fixture> <command> -->` and the test replays exactly
  that, comparing stdout with the fenced block below the marker. Markers without
  `--replay` are refused; a leading `$ …` prompt line is ignored; a scan finding
  fewer than two samples fails.
- Rationale: three deliberate, correct changes — F-20's catalogue keys, the
  `count.packages_with_top_level` key, and the R-5a data read — each left a
  documented sample asserting the behaviour it replaced, and nothing in the
  toolchain could see it. `check-i18n-parity.sh` passes because every key is
  present and rendered, `check-links.sh` has nothing to resolve, and no test read
  a markdown file.
- **The fourth drift is the argument.** F-27 was filed, both blocks were fixed and
  committed, and the policy version then moved in the very next commit of the
  same session — leaving the samples stale again within the hour, while the
  person who had just written the finding was watching for exactly this. A rule
  that loses to its own author on the same day is not a rule, it is a wish. The
  project's own line, that giving the agent a way to check its work beats telling
  it to be careful, is what this entry is.
- **Probed, not assumed.** The three drifts that actually happened were replayed
  as mutations and each failed the test; so did deleting a marker, which the
  minimum-count floor catches. That floor exists because a scan that finds
  nothing satisfies every assertion after it, which is F-19's whole finding
  applied to a test written days later.
- Evidence: F-27; the commits `7f6827c` (blocks fixed) and `e502955` (policy
  version moved, blocks stale again).
- User impact: none directly. The first output a reader sees is now the output.
- Architectural impact: none. One test, no source change.
- Compatibility impact: none.

## SE-071 — Provider prefixes are dated data, and the INV-20 exemption is gone
- Status: accepted (decided by the project owner, 2026-09-05, DP-014)
- Date / Pass: 2026-09-05 / 02-ARCHITECTURE-REVIEW
- Original assumption: `registry.rs` held `/opt/homebrew`, `/usr/local` and
  `/opt/local` in a `match`, with the sources and read dates in a doc comment
  above it, and the INV-20 test carried an explicit exemption for that file
  whose comment said the exemption was not compliance.
- Change: `data/policy/provider-prefixes.toml` states the prefixes per provider
  in detection order, each entry with `source`, `read_on` and a note quoting
  what the source said. `crates/adapters/src/prefixes.rs` reads it and
  `registry::candidate_prefixes` is now one line. The exemption is removed and
  the INV-20 source scan holds every crate.
- Rationale: INV-20 names **prefixes** alongside support tiers and deadlines.
  The doc comment was provenance, and provenance is not what the invariant asks
  for: nothing held the comment to the values beside it, and the check that
  exists to catch a compiled-in provider fact was excusing the one file that had
  one.
- **Why the loader is in `adapters` and not `application`.** `data/policy/` is
  otherwise read by `application`, which sits above `adapters`; the Cargo edges
  are the layering rule and `scripts/check-layering.sh` asserts them. A second
  loader in `adapters`, reading its own file, keeps the edge and also keeps
  provider detection working without anything above this layer being present —
  which is what detection is for.
- **What this does not buy, said plainly.** A prefix is still not editable
  without a rebuild. No policy file in this project is; `include_str!` is how
  all of them arrive. DP-014's own rationale claimed "a data file is a one-line
  edit; a `match` is a release", and that half was wrong. What INV-20 asks is
  that a fact be stated as versioned data with its source and read date, where a
  person can re-check one against the other.
- **Probed.** Reintroducing `/opt/homebrew` into `registry.rs` was replayed as a
  mutation and the INV-20 scan refused it. Without that, removing an exemption
  only proves the check no longer skips a file, not that it would catch
  anything in it.
- Evidence: INV-20's text; DP-014; F-19, whose widening of this scan found a
  second copy of a prefix in the MacPorts adapter; `crates/policy/src/data.rs`
  for the embedding precedent.
- User impact: none. The same three paths, probed in the same order.
- Architectural impact: a second policy-data reader, in `adapters`. Adding a
  provider now touches a data file as well as the registry, which the
  `add-provider` recipe should say.
- Compatibility impact: none.

---

# Raised by the 2026-09-05 security and specification review

## SE-080 — Capture records that it redacted, instead of consumers guessing from the string

- Status: accepted
- Date / Pass: 2026-09-16 / 02-ARCHITECTURE-REVIEW
- Original assumption: a redacted path was assumed to be recognisable from its
  shape. It is not. `redact_known_home` produces **two** shapes — `~` for the
  home, and the bare login name replaced wherever it is a whole path component,
  so `/opt/ada/brew` is written `/opt/USER/brew` — and only the first is visible
  in the result.
- Change: `ProviderDetection` gains `prefix_redacted: bool`, `#[serde(default)]`,
  set by `capture` from whether `redact_option_path` changed the value. `doctor`
  keys its finding on that **or** the shape test, not on either alone.
- Reason: the owner's ruling, 2026-09-16, on F-49. The `~` shape fails closed —
  the planner refuses and the user gets an error — while the `USER` shape is
  absolute, is accepted, and puts a path that exists on no machine into a plan a
  human reads and copies commands from. A consumer cannot tell them apart, and
  the first version of `doctor`'s check tried to and was wrong.
- Evidence: reproduced both ways against the built binary. With the field set and
  an otherwise identical document, the finding fires; without it, the same prefix
  passes unreported.
- Compatibility, checked rather than assumed: no snapshot type carries
  `deny_unknown_fields`, so a new build reading an old document defaults to
  `false` and an old build reading a new one ignores the field and falls back to
  the shape test — the behaviour it already had. **`SNAPSHOT_SCHEMA_VERSION`
  stays 1**; a version that moves for a compatible addition trains readers to
  ignore it.
- Stated limit: `false` means "not known to have been redacted", never "not
  redacted". A `USER`-shaped prefix in a document written before this field is
  still missed, which is an absence in the file rather than a gap in the check.
  The test asserts that case rather than leaving it to be discovered.
- Consequence for DP-016: the same shape of answer. Once a recording carries what
  capture knew, an `Unknown` service status means "this recording predates the
  field" rather than "nothing here is classifiable", which is what lets (c)'s
  refusal narrow to exactly those recordings.

## SE-079 — Releases are immutable, and a withdrawal burns the version number

- Status: accepted
- Date / Pass: 2026-09-16 / 02-ARCHITECTURE-REVIEW
- Original assumption: `08-distribution-and-project.md` specified attestation,
  `SHA256SUMS` and a signing-identity record, and said nothing about whether a
  published release can be replaced. `SECURITY.md`'s withdrawal runbook was
  written against an ordinary release, whose documentation "gives the procedure
  and says nothing about what survives it" (R-14).
- Change: a MUST that releases are immutable. GitHub's immutable releases lock
  the tag to its commit, refuse modification or deletion of attached assets, and
  generate a release attestation automatically.
- Reason: the owner's ruling, 2026-09-16, on the trade R-14 established — supply
  chain integrity against withdrawal manoeuvrability. For a tool that rewrites
  somebody's package manager, proving that `v1.2.3` is always the same bytes is
  worth more than being able to substitute them.
- Evidence: R-14, which quotes the two constraints verbatim with their source and
  read date — a tag "cannot be changed, and cannot be deleted while the release
  exists", and if the release is deleted "you cannot reuse the same tag name";
  assets are "protected from modification or deletion".
- User impact: a withdrawn release is permanently a hole in the version sequence,
  and the fix ships as a new version rather than as a corrected one. Accepted
  deliberately: a burnt version number is cheap, and the property it buys is the
  one a user cannot check for themselves.
- Consequence for `SECURITY.md`: its withdrawal runbook is now written against
  this constraint rather than around it, which is the half DP-009 still owns.

## SE-078 — A second agent harness, generated from the first

- Status: accepted
- Date / Pass: 2026-09-11 / 02-ARCHITECTURE-REVIEW
- Original assumption: `docs/spec/11-agentic-sdlc.md` describes one harness.
  `.codex/` existed anyway — a live hook layer, wired by its own manifest — and
  no chapter mentioned it. The rationale for a running enforcement layer lived
  in a decisions-pending entry and a shell-script header.
- Change: `.claude/` is the source and `.codex/` plus `.agents/skills/` are
  generated from it by `scripts/generate-codex-layer.sh`, with `--check` as a
  `validate.sh` stage. Recorded here because `AGENTS.md` says this file holds
  the current rationale *per topic*, and a future pass reading chapter 11 would
  otherwise not learn the second tree exists.
- Rationale: DP-018, ruled by the owner. The alternative — both manifests
  pointing at one shared set of scripts — makes divergence structurally
  impossible but leaves a Codex manifest reaching into a directory named for a
  different tool. This repository is open source and the developer picks their
  harness, so somebody who picks Codex gets a complete, self-contained layer.
  Generation costs a script and a check; that is the price of the property.
- Evidence: F-42. `.codex/` had drifted to an older, weaker copy — its path
  guard protected the machine and no repository path, its stop gate carried two
  fail-opens `.claude/` had fixed, and `spec-guardian` was missing outright.
  Nothing weakened it; it was written once and `.claude/` moved on over four
  passes. The hook scripts contain nothing Codex-specific, so the transform is a
  copy plus a banner; `tools:`/`disallowedTools:` have no verified Codex
  equivalent and are dropped, which each generated agent states in its own
  header.
- User impact: none — no shipped behaviour changes. Architectural impact: the
  enforcement layer has one source and a gate stage that fails when the copy
  drifts. It caught real drift twice within a day of existing, and once caught
  the fix's own author: `a7ec60a` committed a manifest wired by absolute path,
  which is F-42 restored in the one file the generator does not write.

## SE-072 — The enforcement layer was enforced against the tool call's spelling, not against the artifact
- Status: accepted
- Date / Pass: 2026-09-05 / out-of-pass review (Cowork), applied the same day
- Original assumption: `.claude/settings.json` and the two `PreToolUse` guards
  enforce chapter 11's four stated purposes — no elevation, no mutating
  `brew`/`port`, no writes into a provider prefix or shell configuration, no
  silent toolchain change.
- Change: eleven defects fixed, each with a case in
  `.claude/hooks/test-hooks.sh` (44 new assertions, all of which failed before
  the fix). The load-bearing ones:

  1. **The parser failed open on unreadable input.** `jq` on empty or
     whitespace-only stdin has no inputs to process: it exits 0 printing
     nothing, which `mpb_json_field` returned as "the field is absent" and the
     guards read as "no command to check". The `python3` branch already failed
     closed, so the fail-closed property this chapter states as a MUST
     depended on which parser the machine happened to have — and `jq` is
     preferred, so the common case was the failing one. `null` and a top-level
     array had the same shape.
  2. **Writes into a provider prefix were enforced for `Edit` and `Write` only.**
     A deny rule on a path does not reach a subprocess, and the Bash guard had
     no prefix rule at all, so `cp target/release/macportsbrew
     /opt/homebrew/bin/` — the obvious way to try a fresh build against the
     real system — was allowed, as were `ln -sf`, `install -m 755`, `mkdir -p`,
     `tee` and a redirect into `/etc/paths.d`. Reading *out* of a prefix stays
     allowed: it is how a fixture is captured, so the rule matches a prefix in
     a destination position rather than anywhere in the command.
  3. **The toolchain pin had no guard at all.** Every defence was a matcher on
     the `rustup` command, and that rule's own deny message tells the agent to
     "change the pin in the file" instead. Editing `channel` makes the next
     `cargo` invocation fetch and use a different compiler, so the macOS floor
     ADR-0002 calls a product commitment moved with no denied command anywhere.
  4. **The control did not protect itself.** `.claude/hooks/`,
     `.claude/settings.json`, `scripts/validate.sh`, `scripts/check-*.sh` and
     `.github/workflows/` were all editable from inside the session they
     constrain. The gate can detect that its own path changed; it cannot
     prevent it. `MPB_ALLOW_GUARD_EDIT=1` is now the disclosed escape, on the
     same terms as `MPB_SKIP_STOP_GATE`. **Superseded by SE-081**, which adds a
     second way to arm it and changes the security property on that route from
     prevention to detection; read SE-081 for the current rationale, and this
     item only for what raised the escape in the first place.
  5. **In-place editors were matched on their least likely spelling.**
     `sed -i ''` was tested; `sed -i.bak` — the form chosen when a backup is
     wanted, i.e. the safety-minded instinct — and `perl -pi -e`, the commonest
     idiom there is, both walked past.
  6. **Subcommand aliases.** `brew ln` (that is `link`), `brew up` (`update`),
     `brew instal`, `port unarchive` and `rustup update` are all real
     spellings the hand-written alternation missed.
  7. **`crates/*/tests/` was exempt from every static scan, and `cargo test*`
     is allow-listed.** The one directory permitted to contain the word
     "install" was the one whose code runs against the developer's real
     machine. `check-no-shell.sh` now refuses a test that spawns a program
     named by a string literal: every legitimate end-to-end test spawns this
     crate's own binary through `env!("CARGO_BIN_EXE_…")`, never a literal.
  8. **`mpb_realpath` fell back to returning its input**, which is an identity
     function wearing a guard's name: on a jq-only machine with BSD `readlink`
     — this project's own target — a symlinked parent directory reached the
     caller unresolved.
  9. **The unattended boundary was a comment.** Chapter 11's MUST NOT about
     unattended runs resolving a `decisions-pending` item was asserted in
     `claude.yml` and enforced nowhere. Both workflows now set
     `MPB_UNATTENDED=1` and the path guard reads it; interactive sessions keep
     the documented `/decision-pending` flow.
  10. **`Bash(git push *)` needed an argument**, so a bare `git push` — the form
      that pushes to a tracked upstream, and the likeliest accident — matched
      nothing, while `CLAUDE.md` listed it among the boundaries the hooks block
      "a second time" and no hook matched it either. SE-025 applied exactly this
      reasoning to `su`; it was not carried across.
  11. **`Bash(brew sh*)` is a prefix glob**, so it also denied `brew shellenv` —
      read-only, and the documented way to discover the prefix. Deny cannot be
      overridden by allow, so it was permanently blocked.

  Also: `~/.homebrew` (Homebrew's supported non-admin prefix, present on this
  developer's machine) was not a prefix to the path guard, and `case` was
  case-sensitive on a case-insensitive volume; the Stop gate's `cd "$ROOT" ||
  exit 0` skipped every check silently on a bad `CLAUDE_PROJECT_DIR`; the
  `MPB_SKIP_STOP_GATE` escape left no trace at all; and the gate's failure
  message embedded absolute `/Users/<name>/…` paths that reach a committed pass
  report, which `06-cli-and-ux.md` forbids for `doctor --report`.

- Rationale: chapter 11's governing sentence is that anything an agent must not
  do is enforced by a mechanism rather than a sentence. These are the places
  where a mechanism existed and did not reach. The pattern across all eleven is
  one thing: the layer was written against *how a tool call is spelled* and the
  artifacts it protects are reachable by spellings nobody enumerated —
  `Bash` being the tool that reaches an artifact without naming it. Nothing here
  changes what the chapter promises; it closes the distance between the promise
  and the configuration.
- Evidence: every case was executed against the guards before and after, on the
  real repository, and each is now an assertion in `test-hooks.sh`, which runs
  on both the Linux and macOS legs of `repository-validation.yml`. Claude Code's
  permissions, hooks, skills and github-actions pages re-read 2026-09-05 to
  settle three syntax questions the review raised: `Bash(cargo test:*)` and
  `Bash(cargo test *)` are equivalent (the `:*` suffix is a documented trailing
  wildcard), `${CLAUDE_PROJECT_DIR}` does expand inside an `allowed-tools` Bash
  rule, and `id-token: write` is required by the action's default GitHub App
  authentication — so the review's proposal to remove it as unused is
  **rejected**, and the reason is now recorded in the workflow.
- User impact: none. The product is unchanged.
- Architectural impact: none on the product. Two new disclosed escape hatches
  (`MPB_ALLOW_GUARD_EDIT`, `MPB_UNATTENDED`) and two new checks
  (`scripts/check-memory-files.sh`; a toolchain-pin consistency check in
  `check-action-pins.sh`, because the pin was written in four places under a CI
  step named "pinned by rust-toolchain.toml").
- Cost accepted, and the one to watch: a session can no longer edit its own
  guards without the human starting it with `MPB_ALLOW_GUARD_EDIT=1`. That is
  friction on real maintenance, and it is deliberate — but it is also the third
  guard-friction item in a row (see the false positives reported by three
  consecutive passes), and the pattern that eventually produces a workaround
  that is not honest about itself. If it bites, the answer is a narrower rule
  discussed with the owner, not a reworded command.

## SE-073 — Four documentation claims that were not true, and one section that contradicted itself
- Status: accepted
- Date / Pass: 2026-09-05 / out-of-pass review (Cowork)
- Original assumption: the specification describes the configuration.
- Change:
  - The `Raised by 02-ARCHITECTURE-REVIEW` section header said "not yet
    accepted", claimed "these six entries are `Status: proposed`", and then
    cited "all eleven". It holds **twenty** entries and **every one is
    accepted**, four of them owner Tier-2 rulings. The harm ran opposite to the
    section's stated purpose: settled decisions read as unsettled, so an agent
    following `CLAUDE.md`'s instruction to read this file "so you do not reopen
    settled ground" would reopen exactly that ground. Header and counts
    corrected; the outstanding SE-032 merge of all twenty into their topical
    sections is named there as outstanding rather than done quietly.
  - Chapter 11 claimed `.claude/settings.json` denies "system security tools";
    it denied `launchctl` and `chsh` only, with `csrutil`, `spctl` and
    `systemsetup` living in the hook alone. Added.
  - Two rows of the deny→invariant table were justified by invariants that do
    not say what the row needs: prefix writes cited INV-07/INV-08, which are
    about one provider satisfying the other's dependency, and `chsh` cited
    INV-19, which is about stateful packages. `10-first-pass-work-order.md`
    step 21 makes "the deny rules still map to live invariants" an acceptance
    item, so a rule justified by the wrong invariant is a rule that gets deleted
    when that invariant is reworded. Both now cite the rule they actually defend.
  - `provider-researcher` was described as "read-only by construction" in
    chapter 11 and "never mutates anything" in its own brief, while holding
    `Write` and `Edit` — necessarily, since writing the note is its job. The
    grant is right; the words were not, in a project whose standard is that a
    claim about a mechanism must be true.
  - `AGENTS.md` was 202 lines against a MUST of "under 200" — trivial in
    consequence, and the point is that the number was stated to be checkable
    and nothing checked it. `scripts/check-memory-files.sh` now does, and also
    asserts that `CLAUDE.md` still imports `@AGENTS.md`, without which the
    tool-neutral document is authoritative in the specification and nowhere else.
  - `prompts/02-ARCHITECTURE-REVIEW-PASS.md` §4 was missing the
    `/tooling-recheck` bullet that the template and the other work orders
    carry, so the 02 pass ran without it and chapter 11's read date went stale.
    Both fixed.
- Rationale: each of these is small. Together they are the failure mode this
  repository is most exposed to — documentation that describes an intention as
  though it were a mechanism — and the one it can least afford, because the
  documents are how the next agent decides what it may rely on.
- Evidence: each claim checked against the file it describes; counts and line
  numbers taken by running the check, not by reading.
- User impact: none. Architectural impact: none.

## SE-074 — Coverage floors: `adapters` to 95%, a 90% backstop, and the MUST NOT narrowed

- Status: accepted
- Date / Pass: 2026-09-07 / 02-ARCHITECTURE-REVIEW
- Original assumption: `07-quality-and-testing.md` set `adapters` at 80% and
  forbade "a single blanket percentage across the repository", on the reasoning
  that a blanket number produces padding in adapters and UI while the parts that
  matter stay thin.
- Change: `adapters` rises to 95%; a **90% workspace backstop** is added; the
  MUST NOT is narrowed from "no blanket percentage" to "no blanket percentage
  *in place of* differentiated floors". Owner ruling of 2026-09-06, DP-015.
  Enforced in `scripts/check-coverage.sh`; the spec's table carries both.
- Rationale: the floors had never been measured. The first measurement this
  repository has taken put `adapters` at **95.29%** under an 80% floor, so the
  raise costs nothing and removes a floor nobody was near — the opposite of the
  padding hazard the MUST NOT was written against. The backstop sits *below*
  every named floor, so the only layer it can bite is `ui`, which has none by
  design. That is why narrowing the MUST NOT is not a reversal of it: the
  differentiated floors still do all the work, and the blanket number only stops
  the unnamed layer sliding. Measured: domain 96.65, policy 97.46, application
  95.27, adapters 95.29, cli 88.82, overall 94.86.
- **Two limits are written into the spec rather than left to be rediscovered.**
  Branch coverage is unenforced — `--branch` needs nightly and the pin is stable
  (ADR-0002) — so "95% line **and** branch" is half true. And the denominator
  counts test code living in `src/`: of the 213 instrumented lines in
  `crates/domain/src/plan.rs`, **91 are inside its `#[cfg(test)]` module**,
  measured from the coverage segments rather than inferred. Test code is
  near-fully covered, so every figure above is flattered by an unknown amount.
  Correcting it moves every number at once, which is why it is F-40 with its own
  measurement rather than a line in this change.
- Evidence: `scripts/check-coverage.sh` run against the recorded
  `target/llvm-cov.json`, printing the new `overall` row and exiting 0; the
  91-of-213 figure taken by counting instrumented line numbers at or beyond the
  test module's first line.
- User impact: none. Architectural impact: none; CI enforces one more number.

## SE-075 — The i18n gate counted a doc comment as a render site

- Status: accepted
- Date / Pass: 2026-09-07 / 02-ARCHITECTURE-REVIEW
- Original assumption: `check-i18n-parity.sh`'s third direction — every English
  key is referenced from code or says why not — was believed to be defeated by
  `#[cfg(test)]` modules inside `src/`, which the script cannot exclude the way
  it excludes `crates/*/tests/` (F-32, as first written).
- Change: `production()` now strips `///` and `//!` lines as well as the
  trailing test module. `cli.about` consequently needs, and gets, an
  `# unreferenced:` marker.
- Rationale: **the original finding was wrong and checking it is what found the
  real one.** `production()` already strips the trailing `#[cfg(test)] mod
  tests`, deliberately and with its reasoning recorded. What survived were two
  doc comments — an `assert!` inside a `///` example, and prose that merely
  named the key — so a key nothing renders satisfied the gate on documentation
  alone. A `///` line never carries code the program runs, which is why
  stripping whole lines is both crude and correct here.
- Evidence: simulated before shipping. With doc comments stripped, `cli.about`
  is the only newly-orphaned key and the gate demands a marker; with the marker,
  `i18n: ok (188 keys, 170 rendered, 18 unreferenced with a stated reason)`.
- User impact: none. Architectural impact: none.

## SE-077 — MacPorts elevates by prefix writability, not by being MacPorts

- Status: accepted
- Date / Pass: 2026-09-08 / 02-ARCHITECTURE-REVIEW
- Original assumption: `04-migration-and-safety.md` said the design "assumes
  MacPorts operations generally require elevation", sharpened in two places —
  by package, and by the `brew services` system-level case. Both sharpenings
  were about *what* is being operated on; the provider half was left as a
  property of the provider.
- Change: a third sharpening, by **prefix**. The MUST now states that MacPorts'
  elevation follows the writability of its prefix, that the default prefix is
  why the outline claim holds, and that a `--prefix` install is the case where
  it does not.
- Rationale: R-5's MacPorts half had been cited, never observed — the note said
  so itself, and `09-research-checklist.md` said it "is re-checked on a MacPorts
  machine". This became one on 2026-09-07. All four elevation guards in the
  shipped 2.12.6 Tcl are conditional; `portinstall.tcl:100` reads
  `if {![file writable $prefix_frozen] || ...}` before `elevateToRoot`. So the
  provider does not have a privilege property; its **installation** does. Same
  shape as the cask sharpening already in that paragraph, and the same shape as
  INV-20: a dated fact about a default, not architecture. The project's own
  `provider-prefixes.toml` already contemplates the case that disproves the
  general claim.
- Evidence: `docs/research/R-5-privilege-requirements.md` E19–E32, observed
  2026-09-08. The `elevateToRoot` condition and `/opt/local` being
  `root:wheel drwxr-xr-x` were re-read in the main session rather than taken
  from the research pass. Every read-only `port` subcommand the adapter uses ran
  as an unprivileged user and exited 0, so INV-06 holds on this side by
  observation now rather than by inference.
- User impact: none today — nothing mutates, and the default prefix behaves as
  the outline claim says. Architectural impact: the reason the plan asks **per
  step** now has three independent supports instead of two, and a future
  `--prefix` install becomes a case the specification has already conceded
  rather than one that contradicts it.

## SE-076 — A session-start hook that could not tell "nothing" from "I could not look"

- Status: accepted
- Date / Pass: 2026-09-07 / 02-ARCHITECTURE-REVIEW
- Original assumption: `session-start.sh` rendered an empty decision list as
  `(none parsed - read the file)`, on the assumption that empty meant the parse
  had failed.
- Change: the `## Open` section is read first. Missing section → say so; present
  and empty → `(none)`; otherwise the list.
- Rationale: empty has two causes and the message asserted the second. The
  branch had never fired in four passes because the file had never had an empty
  Open section; resolving DP-015 emptied it, and every session then opened by
  telling the agent its own decision list had failed to parse. Same shape as
  F-12 and as the stop gate's `cargo`-absent hole: **a path that did nothing was
  indistinguishable from a path that succeeded.**
- Evidence: all three states exercised — a live DP listed, `(none)` on an empty
  section, and the explicit message when the heading is absent.
- User impact: agents get a true statement at session start. Architectural
  impact: none.

## SE-081 — The guarded-path escape can be armed from a prompt, and that trades prevention for detection

- Status: accepted
- Date / Pass: 2026-09-17 / 02-ARCHITECTURE-REVIEW
- Supersedes: **SE-072 item 4**, which raised the escape and described it as the
  environment variable alone. That item now points here; per SE-032 this entry
  is the current rationale for the topic and SE-072's is kept only as the
  account of what raised it.
- Original assumption: the only way to open `guard-protected-paths.sh` was
  `MPB_ALLOW_GUARD_EDIT=1` in the environment, set before the session starts.
  `11-agentic-sdlc.md` describes it as a disclosed escape whose cost is an entry
  in the end-of-pass report.
- Change: a `UserPromptSubmit` hook, `.claude/hooks/arm-guard-edit.sh`,
  recognises the literal `MPB_ALLOW_GUARD_EDIT=1` in a user message and writes a
  marker naming the session, the time and an excerpt of that message. The guard
  honours the marker as well as the variable, and appends every write the escape
  lets through — by either route — to `.claude/.guard-edit-log`.
- Rationale, and the part that is a real loss: **the two routes are not equally
  strong, and the difference is the whole of this entry.** The environment
  variable cannot be forged *for a running process*: it is set before that
  process exists, and nothing inside a session can change its own environment.
  The marker is a file, and the session has a shell;
  `guard-protected-paths.sh` is `PreToolUse` on `Edit|Write|NotebookEdit` and
  never sees `Bash`.
- **A claim this entry made and a reviewer disproved, kept here because the
  correction is the useful part.** It first said the variable is beyond reach
  altogether, "because the two settings files whose `env` block would reach a
  hook are both in the guard's deny list". They were — to `Edit`, `Write` and
  `NotebookEdit`, and to nothing else, and only while the escape was *unarmed*.
  So an armed session could write an `env` block and grant itself the variable
  for every session afterwards: the weak half manufacturing the strong one.
  Ninth false statement about what the code prevents in this pass, and the first
  caught before it shipped.
- **Resolved as DP-020, on the owner's ruling, option (c).**
  `*/.claude/settings.local.json` is now in the guard's **unconditional** deny
  list, refused armed or not and by either arming route.
  `*/.claude/settings.json` stays escape-guarded, because a change to it is
  committed and reviewable and registering a hook is something a session
  legitimately does. The deciding difference is visibility, not the write: the
  git-ignored file is the one whose escalation would appear in no diff at all.
  `test-hooks.sh` asserts both halves, since the asymmetry *is* the ruling.
  This closes the accidental route and the `Edit|Write|NotebookEdit` route and
  nothing else — `Bash` never reaches this guard, so it writes either settings
  file exactly as it writes the marker. Naming the marker in the deny list stops the Edit and
  Write tools and nothing else, and no secret can fix this, because whatever
  would authenticate the marker has to be readable by the guard and is therefore
  readable by the agent. So: the variable is **prevention**, the marker is
  **detection**. The trade is worth making only because disclosure was always
  what this escape rested on — the variable never stopped a determined session
  either, it stopped an absent-minded one and made the deliberate act
  reportable. The marker keeps the second property, gives up part of the first,
  and adds something the variable never had: an author. A variable, by the time
  the guard reads it, is a property of a process with nothing to point at; a
  prompt has a writer, a time and a text, which is what lets the log say *which*
  instruction authorised a write.
- Scope, stated precisely because the obvious reading is wrong: the marker is
  rewritten on **every** user message — present with the token, deleted without
  one — so it expires at the human's next message with no timer to be wrong.
  That is not per-edit authorisation. One message can drive a long autonomous
  run and all of it is armed; the unit is one *work order*.
- That expiry needed two fixes a reviewer found, and neither was cosmetic. The
  delete is now the **first** statement in the hook: behind the parser check it
  was skipped whenever `jq` and `python3` were both unreachable, leaving the
  previous message's marker armed for the rest of the session — not a
  hypothetical in a tool whose own work can relink `jq`. And `session-start.sh`
  now deletes the marker, because nothing in this hook runs at session end and a
  resumed session keeps its `session_id`, so an armed marker would have been
  inherited by the run after it.
- The escape also fails closed on its own record now. A permitted write that
  cannot be appended to `.claude/.guard-edit-log` is **denied**: once detection
  is what the route rests on, a swallowed log failure buys a silent
  enforcement-layer write, which is worse than the refusal it replaced — and
  making the append fail takes one `Bash` command. Both log fields are squashed
  to a single line, because a path may contain a newline and `*` spans it, so an
  unescaped path could close its own record and open a fabricated one.
- Evidence: `test-hooks.sh` covers arming, disarming, the marker bound to its
  own session (another session's id is refused), the marker and log refused to
  Edit even while armed, an unreadable payload disarming rather than leaving the
  escape open, and the token present in a field other than `prompt` arming
  nothing.
- **The claim that last test was written to protect turned out to be false, and
  the escape's own first live firing is what proved it.** The entry said: the
  hook reads exactly one field, so "tool results, file contents and subagent
  reports never reach this event at all". The first half holds. The second does
  not — a subagent hand-back is delivered to the session *through the prompt
  channel*, and the first real arming of this escape was a `spec-guardian`
  report quoting the token while describing the mechanism. Nobody typed it that
  turn. It was caught inside a minute because the marker stores an excerpt of
  what armed it, and the excerpt read `<agent-message from=…>` — the detection
  property doing exactly the job it was added for, on its author.
- The mechanism is narrowed accordingly: the token now has to appear on a line
  beginning at column zero, and the harness indents every line of relayed model
  output. That is a **filter, not a boundary** — it rests on a formatting
  convention this repository does not own, and a relay that did not indent would
  arm. It removes the demonstrated route and nothing more. Tenth false statement
  about what the code prevents in this pass; the second caught before shipping,
  and the first caught by the mechanism it was written about.
- **That prediction came true on 2026-09-20, and the filter is now known not to
  hold.** A `docs-auditor` hand-back — commissioned to audit the disclosure of
  the *previous* unguarded write, and therefore quoting this entry's own MUST
  table, token and all — armed the marker. No human message was involved; the
  session had been resumed after compaction and the owner was asleep. The relay
  arrived in a `<task-notification>` envelope rather than the `<agent-message>`
  form the column-zero rule was written against, and something in it put the
  token at column zero. The stored excerpt again names the author, and again
  that is how it was caught: it reads `<task-notification> <task-id>…`, so the
  marker's own evidence says no human armed it. Twice now the escape has been
  armed by a reviewer quoting the specification that describes the escape.
- **What this settles.** The formatting convention is not a boundary and cannot
  be patched into one by naming a second envelope — the next relay shape would
  arm again. Either the prompt-armed marker is withdrawn in favour of the
  environment variable, which a session cannot forge, or arming must require
  something a relayed agent message structurally cannot carry. Until one of
  those lands, **an armed marker is evidence that something quoted the token,
  not that the owner authorised anything**, and a session finding itself armed
  without a human message should decline the window and say so. That is what
  happened here: the window was refused and no guarded path was written under
  it, which `.claude/.guard-edit-log` is the record of — it has no entry for
  that turn.
- **Narrowed again 2026-09-20, this time onto something this repository owns.**
  The token must be the **first thing on a line**, and `scripts/test-gates.sh`
  asserts as a corpus invariant that no line in the tracked tree begins with it:
  33 matching lines today, none of them opening with the token. **State exactly
  what that proves, because the first version of this entry did not.** It proves
  that no *tracked source line* begins with the token, and an edit introducing
  one fails the gate. It does **not** prove that no quotation of this repository
  can arm the escape, which is the broader sentence that was written here and in
  the hook, and which two reviewers disproved the same day by different routes:
  the git-ignored `.claude/.guard-edit-marker` now begins its third line with
  the token *by construction*, because the excerpt it stores is the opening of
  a prompt that had to begin with the token to arm; and a quotation that
  reformats — a fenced block, a list item, a table cell broken one token to a
  line, a diff hunk — begins a line with text that did not begin one in the
  source. The gate cannot see the first, and does not speak to the second. The
  assertion is also *detective* rather than preventive: it runs at `Stop` time,
  after the turn in which a prompt could have quoted the offending line. A prompt
  carrying a relay envelope is refused outright as well, and when the token is
  present but the prompt is refused the hook says so rather than going quiet, so
  a human whose own message was rejected is not left guessing. Eight constructed
  prompts hold the rule, and both mutations — the token matching anywhere on the
  line, and dropping the envelope refusal — fail cases.
- **Still a filter, and the gap is now the owner's rather than open.** Text from
  outside this corpus can put the token at the start of a line, and no readable
  secret stops a session that means to arm itself. The two routes that would be
  boundaries — the environment variable alone, or a per-session nonce — each
  spend how the owner works, so they are **DP-022** rather than chosen here.
- Found while writing it, and fixed in the same change: `test-hooks.sh`
  inherited `MPB_ALLOW_GUARD_EDIT` from the session running it, so under an
  armed session nine of its enforcement-layer `deny` assertions passed the wrong
  way — demonstrated, not inferred. Worse, the three cases that set an escape
  did so inside `( … )`, and a subshell discards the `failures` array, so those
  assertions could not have failed the suite at all. Both are now impossible:
  the suite unsets both variables at the top and sets them without a subshell.
- User impact: none; this is agent tooling. Architectural impact: the
  enforcement layer's escape is now detective rather than preventive on one of
  its two routes, and `11-agentic-sdlc.md`'s disclosure requirement carries
  correspondingly more weight.

## SE-082 — The enforcement layer is a property of the harness, not of the checkout

- Status: accepted
- Date / Pass: 2026-09-18 / 02-ARCHITECTURE-REVIEW
- Original assumption: `11-agentic-sdlc.md` described the guards, the escape and
  the `Stop` gate as things this repository has. It never said *where* they run,
  and DP-018 had already established there is more than one harness.
- Change: `11-agentic-sdlc.md` now states plainly that the guards are Claude Code
  hooks plus the generated `.codex/` layer, and names which surfaces read them —
  the terminal CLI, the IDE extensions and the desktop app's `Code` tab, which
  bundles Claude Code; and not the same app's ordinary chat surface, which is a
  different product and runs none of it.
- Rationale: the question came up because a session ended with the owner moving
  to the desktop app, and the honest answer at that moment was "I have not
  checked". Checking took one fetch and the answer is reassuring — but the
  reassuring half and the dangerous half live in the same application, one tab
  apart, and nothing in a checkout distinguishes them. An absent guard is silent,
  which is the shape of F-12 (a CI job that never ran) and F-42 (a second
  harness whose guards were weaker), and both of those cost a pass to find.
- Evidence: code.claude.com/docs/en/overview, read 2026-09-18 — "A standalone app
  for running Claude Code outside your IDE or terminal … The app includes Claude
  Code, so you don't need to install the CLI separately."
- Also corrected in the same row: the path list described the escape as covering
  `scripts/validate.sh` and `scripts/check-*.sh`, while the guard denies every
  `scripts/*.sh` plus `.claude/agents/`, `.claude/skills/`, `.codex/` and
  `.agents/skills/`. The narrow spelling is what left `generate-codex-layer.sh`
  unguarded once already (F-45).
- User impact: none; this is agent tooling. Architectural impact: none. It makes
  a precondition explicit that was being relied on without being stated.

## SE-083 — A verification step whose command cannot fail is an ordering invariant guarding nothing

- Status: accepted
- Date / Pass: 2026-09-18 / 03-UNKNOWN-IS-NEVER-A-YES
- Original assumption: `docs/schemas/plan.v1.md` left "how does an executor know
  this command succeeded" entirely to the executor, and its own executor-debt
  list (item 4) said the answer "belongs beside the verb in
  `provider-commands.toml`, which is a schema change and therefore the
  executor's, not this pass's".
- Change: `data/policy/provider-commands.toml` gains `[entry.success]`, one
  value per verb from a closed set of two — `exit_status` and
  `stdout_names_package`. The loader refuses an entry that omits a verb's value
  or names one it does not recognise, exactly as it already refuses a missing
  elevation. `PlanCommand` gains a `success` field carrying
  `SuccessSemantic::{ExitStatus, StdoutNamesPackage, Unknown}`, and
  `MigrationPlan::steps_with_unknown_success` reports the steps an executor must
  refuse. Executor-debt item 4 is rewritten: the data half is discharged and
  what remains is the executor reading the field.
- Rationale: `VERIFY_DESTINATION` is the step INV-01 puts in front of
  `REMOVE_SOURCE`, and on one of the two peer providers it cannot fail.
  Observed on this machine 2026-09-08 (MacPorts 2.12.6, R-5 E22): the MacPorts
  read verb exits **0** for an installed port, for a real port that is absent,
  and for a name that is not a port at all. An executor implementing
  `PROVIDER_REPORTS_ACTIVE` as "run it, succeed on exit 0" passes verification
  for a package that was never installed and then removes the source, while
  `check_ordering` holds perfectly. Homebrew's verify verb *does* exit 1 for an
  absent name — a **different** pair of reads, R-5a E1 on 2026-09-04 for the
  positive case and 2026-09-06 for the negative one, and the first version of
  this entry and of the data file's comment attributed both to R-5 E22 and to
  2026-09-08, which is a note containing no Homebrew observation at all
  (`spec-guardian`, 2026-09-18). The facts held; the citation did not, and a
  sourced claim wider than its source is what F-28 was. So the two providers
  cannot share one rule — which is why this is data with a source and a read
  date (INV-20) rather than a branch in Rust.
- Why it was taken now rather than left to the executor: the deferral was
  written when the alternative was inventing a shape against a hypothetical.
  It is not a hypothesis any more — the behaviour is observed, on both
  providers, in both directions — and the cost of carrying it is that the one
  open finding whose failure mode is silent data loss stays open through
  another pass. What genuinely belongs to the executor is unchanged: reading
  the field and acting on it.
- The shape, and the one place it departs from its neighbour: `Unknown` is the
  `#[serde(default)]` and the `#[serde(other)]` value, so a plan written before
  the field existed, or one naming a semantic a newer build added, refuses
  rather than falling back to exit status. `PlanCommand::placeholder`'s own
  documentation calls out that it could not do this — `false` is its permissive
  value and a boolean has no shape that fails closed on a reader who never
  looks. A tagged union does, and this one uses it.
- Not done in that pass, deliberately, and **two of the three have since been
  done**: `PACKAGE_ABSENT` carries a command as of 2026-09-19 (SE-088), and the
  human renderer shows the semantic as of the same day — that reasoning stopped
  holding once one verb carried two opposite meanings. **`[entry.verbs]` is
  still kind-blind (F-22), and that one stands.** Superseded in place rather
  than left standing, because SE-032 makes a topic's entry the *current*
  rationale and this is the entry a reader looking for `StdoutNamesPackage`
  lands on (`spec-guardian`, 2026-09-19).
- Evidence: `invariant_01_a_verification_command_states_how_it_could_fail`
  asserts the semantic per direction and that no step of a planned migration
  leaves it unstated; `every_verb_states_how_its_success_is_established` and
  `a_success_semantic_that_is_not_one_of_the_two_is_refused` probe the loader by
  deleting and by corrupting each cell; `the_shipped_table_states_the_semantics_
  that_were_observed` pins the two values the observation produced.

---

## SE-084 — In a document, a capability that is not recorded is not a capability nobody asked about

- Status: accepted
- Date / Pass: 2026-09-18 / 03-UNKNOWN-IS-NEVER-A-YES
- Extends: **SE-080**, which recorded that `SNAPSHOT_SCHEMA_VERSION` stays 1 for
  compatible additions and that once recordings carry service state the
  DP-016(c) refusal "stops triggering on its own". That remains true and is not
  what this entry changes.
- Original assumption: DP-016(c) closed the case where a provider cannot answer
  whether its packages define services — the planner refuses when the source's
  `SERVICE_METADATA` capability is `Unknown`. `ProviderCapabilities::is_unknown`
  answers `false` for an **absent** entry, deliberately: in memory a capability
  nobody reported was never asked about, and reading that as "undetectable"
  would refuse every plan built from a hand-made snapshot.
- Change, in two parts, and the second is the one that made the first correct:
  1. **The adapters now agree about the shape of "absent".** The Homebrew
     adapter wrote an empty capability table for a provider that is not
     installed; the MacPorts adapter wrote all nine as `Unknown`. Homebrew now
     does what MacPorts does, and MacPorts' hand-written list of nine is
     replaced by `Capability::all()`. This changes user-visible output: an
     analysis of a machine without Homebrew now reports its capabilities as
     nine `UNKNOWN`s rather than as nothing, which is the honest answer and the
     one the peer provider already gave. Three golden fixtures were regenerated
     for it.
  2. **`Snapshot::mark_unrecorded_capabilities_unknown`** records every
     capability a **loaded document** does not mention as `Unknown`, and
     `load_snapshot` calls it before any check runs. `Capability::all()` makes
     that total, with the drift guard `ErrorId::all()` arrived at after F-43:
     an exhaustive match in the same file, so a new variant is a compile error
     there, and a count constant that then fails until the list is extended.
- **Part 1 is here because two attempts at part 2 alone both failed, in
  opposite directions.** The first filled every table and the fixture matrix
  refused it — `neither-provider-intel-macos13`, *"snapshot round trip differs
  from the replay analysis"* — because inventing nine unknowns for a provider
  Homebrew's adapter had left empty made `--from-snapshot` disagree with a live
  run of the same machine (INV-15). The second skipped empty tables to satisfy
  the matrix, and `spec-guardian` demonstrated the cost: emptying the whole
  table, rather than deleting one key, then walked straight past the refusal
  this entry exists to install — exit 0 and a byte-identical plan, from a
  source provider still declaring itself `PRESENT` with a full inventory.
  **That is the outcome the next bullet rejects `#[serde(default)]` for**, and
  the second attempt shipped it while the entry argued against it. A snapshot
  file is untrusted input here (F-09, F-21), so a one-keystroke bypass of
  INV-19 is not a rough edge.
- **A third spelling, and a third producer, both found by `security-reviewer`
  on 2026-09-18 after the above was written.** The refusal keyed on
  `CapabilityState::Unknown` alone, so a document whose `SERVICE_METADATA`
  read `UNAVAILABLE` planned normally — reproduced at exit 0 with a
  `REMOVE_SOURCE` step that elevates. `Unavailable` is not an affirmative
  answer to "can this provider tell you whether a package defines a service":
  a provider that does not expose service metadata cannot tell you, exactly as
  one whose scan failed cannot, and only `Available` is. The predicate is now
  `ProviderCapabilities::cannot_answer`, which matches both and keeps the
  absent-is-false carve-out. It also closes an accident path rather than only
  a crafted one: Homebrew's parse-failure branch records `ServiceMetadata` as
  `Unavailable`, which is "I could not look" written as "detected absent".
  Separately, `capture::broken` was a **third** producer of the empty table
  this entry says cannot exist — reachable when a *present* Homebrew's output
  fails to parse — and now writes nine `Unknown`s like the two adapters.
- **Both were live in a tree that was green on every gate in the repository.**
  Recorded because the lesson is not "the reviewers are useful" but which
  class of defect the gates here cannot see: an enum variant that means the
  same thing as another and is matched in one place.
- **Both symptoms had one cause and it was in neither place.** A rule written
  at the load door to accommodate an adapter asymmetry can only choose which of
  the two defects to keep. With the peers agreeing, the rule is unconditional,
  both shapes refuse, and it is a no-op on every table this build writes. The
  asymmetry itself had no stated reason and contradicted `AGENTS.md`'s peer
  position; it was invisible until a document rule had to be written around it.
- Rationale, and it is a distinction rather than a reversal: the in-memory rule
  is right and stays. A document is in a different position. A capture asks
  every capability and writes every answer, so a document that omits one was
  written by a build that did not have the question — and the file does not say
  what answer it would have given. Absence there means the record does not say,
  which is what `Unknown` means. Before this, such a document carried the same
  `SNAPSHOT_SCHEMA_VERSION`, so it loaded, and INV-19's rule went silently
  unevaluated on a real plan: the exact outcome DP-016 exists to prevent, one
  route over. That was F-29's residual hole, recorded when DP-016(c) shipped.
- Why not the two routes the finding named. **Bumping `schemaVersion`** would
  refuse the document outright, including for read-only analysis, which
  `.claude/rules/adapters.md` says is wrong: "the analysis degrades and says
  so; it does not guess, and it does not fail either". Only the planning path
  refuses, and `analyze --from-snapshot` on the same document still succeeds
  and reports `UNKNOWN`.
  **`#[serde(default)]` on the field** is still rejected, but the reason this
  entry first gave has expired and is replaced rather than quietly kept: it
  said an optional `capabilities` would deserialise to an empty map and read
  as "never asked about", which was true of the bounded rule and is not true
  of the unconditional one — the empty map is now filled and refuses. The
  reason that survives is better, and `spec-guardian` demonstrated it while
  re-checking the rework: keeping the field **required** refuses the document
  at parse time, exit 8, *before* anything reads it, where defaulting it would
  refuse at the planner, exit 4, after the rest of the document had been
  trusted. A malformed document should not get as far as being analysed.
- Evidence, demonstrated rather than asserted:
  `invariant_19_a_document_that_does_not_record_the_service_question_refuses_
  the_plan` captures a real snapshot, sets the capability to a definite answer
  and plans successfully (the control, so the refusal cannot pass for the reason
  DP-016(c) already covered), then deletes the key and requires exit 4 with no
  document on stdout, then requires the analysis of the same file to succeed
  with `UNKNOWN`, and finally empties the whole table and requires **that** to
  refuse too — the arm the second attempt would have failed. Probed by removal:
  with the one call commented out the test fails with exit 0, a plan where a
  refusal is owed.

---

## SE-085 — The policy version was a rule with a rationale, a format and no gate

- Status: accepted
- Date / Pass: 2026-09-19 / 04-CLOSING-THE-REGISTER
- Original assumption: `data/policy/policy.toml`'s header defines `version` as
  `<newest read_on across the data set>-<serial within that day>` and states
  plainly why it matters — "the version exists so a recommendation can be traced
  to the data that produced it; anything that changes the data and not the
  version defeats that." `.claude/rules/policy-data.md` repeats the format, and
  every `Recommendation` and `--json` payload carries the value. Three artefacts
  state the rule. None checked it.
- What was found: on 2026-09-08 commit `33439cb` moved
  `provider-commands.toml`'s `read_on` forward for F-33 and left the manifest
  reading `2026.09.06-1`. For eleven days every recommendation this build
  produced carried a version naming a data set that was not the one that
  produced it — the precise failure the header describes, in the directory the
  header governs. `CHANGELOG.md` carried the same stale value and a read-date
  list missing 2026-09-08.
- Change: the manifest reads `2026.09.08-1`; the changelog and both documented
  `status` samples follow; seven golden fixtures were regenerated, and the diff
  is `policyVersion` and nothing else, which is how it was checked rather than
  hoped. `invariant_20_the_policy_version_names_the_newest_read_date_in_the_data`
  now asserts the date half on every run.
- Why the test is textual rather than through `PolicyData`: the loader knows
  four of these files. `provider-commands.toml` is read by the application layer
  and `provider-prefixes.toml` by the adapters, so a check written against the
  parsed type would have excluded exactly the file that drifted. It reads the
  directory the rule names, and fails if it finds fewer than four `.toml` files —
  a scan that finds nothing otherwise passes while proving nothing (F-19).
- What it does not check, said rather than implied: **the serial.** It counts
  edits within a day, and an edit that moves nothing else leaves no trace a test
  can count. The date half is the half that carries the traceability claim.
- **And this change did not bump the serial, under the rule it exists to
  enforce.** It edits `data/policy/provider-commands.toml` — comments only, no
  verb, elevation, success semantic or reserved name — and leaves the version at
  `-1`. `.claude/rules/policy-data.md` says "bump the serial for any edit that
  does not move a read date", unqualified. The reading taken is that a comment
  is not a fact and a recommendation computed before and after is traceable to
  the same data; the rule does not say that, and the new test cannot catch it
  either way. Stated here rather than left for a reader to notice, because this
  is the one change where "we edited a policy data file and did not bump" is
  exactly the sentence that ought to be visible (`spec-guardian`, 2026-09-19).
- Proved by mutation, not by going green: reverted to `2026.09.06-1`, the test
  fails naming `provider-commands.toml` as the file whose read date it missed.
- **And the bump made a second, older inconsistency visible, which is recorded
  here rather than in its own entry because it is the same topic.**
  `analysis.v1.md` defined `policyReadOn` as "the newest `read_on` across the
  policy data". It is not: `PolicyData` loads four of the directory's files, and
  `provider-commands.toml` — read by the application layer — and
  `provider-prefixes.toml` — read by the adapters (DP-014) — are not among them.
  So a payload now carries `policyVersion: 2026.09.19-1` beside
  `policyReadOn: 2026-09-04`, which reads as self-contradictory and is not. The
  gap was four days before this pass and is fifteen after it.
  **The definition is corrected, not the computation.** The narrower set drives
  the staleness notice, and the *older* date warns earlier — a data set is as
  fresh as its stalest half. Widening it would also make this crate read files
  it does not own, which is the layering those files were placed by. Found by
  `spec-guardian` after the change had committed; `analysis.v1.md` is the
  document the sample guard cannot check, because its block is abridged by
  design (F-50), which is why nothing caught it.

---
## SE-086 — A refusal that describes a symptom the tool itself caused

- Status: accepted
- Date / Pass: 2026-09-19 / 04-CLOSING-THE-REGISTER
- Evidence: `crates/application/src/planner.rs` (the refusal);
  `a_home_redacted_prefix_is_refused_by_the_planner_and_not_by_the_boundary`
  and `a_home_redacted_destination_is_refused_too_and_retain_source_does_not_rescue_it`
  in `crates/cli/tests/end_to_end.rs`;
  `is_safe_captured_path_admits_only_absolute_or_home_rooted` in
  `crates/domain/src/snapshot.rs`; `error.rs:95` for the exit-code table.
- Extends: **SE-080**, which added `ProviderDetection::prefix_redacted` so
  `doctor` could tell the two shapes of redaction apart. Nothing in SE-080's
  reasoning changes.
- Original assumption: F-48 recorded that a provider installed under `$HOME` can
  be analysed and never planned, and that the refusal "says their input is at
  fault when nothing is". The reversible half taken 2026-09-16 put the true
  statement in `doctor` and pointed the refusal's remediation at it. The refusal
  itself still said the prefix "names no directory this tool can resolve".
- Change: it now names the cause — capture removes the home from the prefix, so
  nothing downstream can resolve it back to a program, and analysis is
  unaffected — and adds that this is a limit of the build rather than a defect
  in anything the user supplied.
- **The first version of this change branched, and the branch was wrong.** It
  read `prefix_redacted || program.starts_with('~')` and kept a second arm for
  the other case, with a comment calling the pair load-bearing. `spec-guardian`
  showed the first term could be deleted with **every test in the workspace
  still passing**: a redacted *absolute* prefix — F-49's `/opt/USER/brew` shape —
  never reaches this guard at all, because the guard fires only on a
  non-absolute program. The pairing `doctor` needs is not the pairing this site
  needs, and shipping an untested term under a comment claiming otherwise is the
  F-19 class exactly, in a change that cites F-19 twice.
- So there is **one message and no branch**, and what keeps it honest is a test
  rather than dead code: `is_safe_captured_path` admits only `/`-, `~`- or
  `~/`-rooted prefixes, and `provider_commands` refuses an absolute
  `executable`, so a non-absolute program is `~`-rooted always. That premise is
  now pinned in the crate that owns it, and widening it fails a test that names
  this site.
- The test asserts the superseded message is **not** what answered, not only
  that the new one is. A refusal falling back would otherwise satisfy a positive
  assertion the moment the two strings shared a phrase.
- **The identifier is still wrong and deliberately not changed here.**
  `ErrorId::InputUnusable`'s catalogue string ends "check whatever produced it",
  so the envelope still names the user's input while the detail says the
  opposite. `CapabilityMissing` was proposed as "the same exit code 8"; it is
  **5**, and `06-cli-and-ux.md` makes exit codes a documented contract, so that
  is a contract change rather than a wording fix. Rewording the catalogue string
  is not available either — its two other callers are genuinely about the input.
  The detail therefore carries the correction, and the identifier question goes
  to the owner with DP-021.
- **Filed as DP-021, which is the part that was actually owed.** F-48's
  remaining half trades away an audience — users installing under `$HOME`
  because they have no administrator rights — or spends a Tier 3 MUST.
  `AGENTS.md` makes that the owner's, and it had been carried in the register as
  "a design question" for three days without being written where such questions
  live. The entry records a third option neither the finding nor the review had
  named: carry the prefix in the in-memory plan only, since the MUST governs
  what the tool *writes*. It also records that the MUST as written governs
  `doctor --report` alone, and that extending it to snapshots is a reading —
  which is the axis the whole recommendation turns on.
- **Both directions are now tested, and one was not.** `ADD_DESTINATION` builds
  from the destination and `REMOVE_SOURCE` from the source, so a home-installed
  provider is refused either way; DP-021 first claimed `--retain-source` exits 0,
  which holds only when it is the source. The code was symmetric throughout — the
  refusal names no provider — so the gap was in coverage, which is the half INV-17
  makes a habit of losing.

## SE-087 — A sample the binary has never been asked to produce is a claim, not a contract

- Status: accepted
- Date / Pass: 2026-09-19 / 04-CLOSING-THE-REGISTER
- Extends: **SE-063**, which built `documented_samples.rs` after F-27 found the
  same block wrong in `README.md` and `wiki/Getting-Started.md` for two commits.
  Its reasoning is unchanged; what changes is where it looks.
- Original assumption: `documented_files()` returned `README.md` and `wiki/*.md`.
  `docs/schemas/` was never in scope, and `plan.v1.md` — the document the
  executor pass is told to build against, whose own "What an executor inherits"
  list is the handover for five obligations — had never had a sample compared
  with the binary.
- Change: the scan reads `docs/schemas/*.md`, and the marker convention gains
  two optional tokens, each defaulting to what every existing marker already
  meant, so no marker in the tree changed:
  - `at=<RFC 6901 JSON Pointer>` resolves the block against a sub-value of the
    parsed stdout. A schema document shows a `PlanCommand`, not a plan.
  - `exit=<n>`, default `0`, replacing `status.success()`. Stricter for the
    existing samples, and the only way a document can honestly show a refusal —
    `plan.v1.md`'s whole-plan example exits **4**, so it was unreachable by a
    guard that asserted success.
- **What the first run found**, which is the argument for the change rather
  than a footnote to it. `doctor.v1.md`'s worked example was wrong in four ways,
  and the first draft of this entry got the story wrong — it said "fifteen lines
  shorter … missing findings", which is arithmetic about re-indentation and not
  a defect at all. `spec-guardian` re-derived it from the two documents. The
  block carried **six findings before and six after**; what differed was:
  - `"id": "path_duplicate"` where the binary emits `path_unknown` — the
    document asserted a **finding this build does not produce**, with a
    `params.entry` of `~/some/dir/bin` that exists nowhere;
  - `"tier": "Degraded"` where the binary writes `DEGRADED` — a documented enum
    value in the wrong case, in the document that *is* that enum's contract;
  - `"readOn": "2026-09-02"` on two findings where the data says `2026-09-04`;
  - `policyVersion` stale by two bumps.

  A fabricated finding id and a mis-cased enum are a worse and more useful story
  than "missing findings", and the correction is kept here rather than written
  over because the first version was produced by reading a line count instead of
  the two payloads.
- The two `PlanCommand` fragments were **correct in content** and folded by
  hand. They are rewritten as the binary serialises them: more verbose, and the
  sentence above them claims they are "as they actually serialise", which is now
  true. Byte comparison everywhere was chosen over a formatting-insensitive
  compare for JSON, because one rule that is occasionally ugly beats two rules
  that a reader has to know which of applies here.
- **Two blocks stay unchecked, and both now say so in their own text.**
  `analysis.v1.md` empties `packages` and writes a literal `"..."` path;
  `explain.v1.md` shows three fired rules of a roster of fourteen. Both declare
  the abridgement in prose, and no marker form makes an abridged block
  byte-equal to an output. Closing them needs either regeneration from a small
  recording or a key-structure-only comparison, and that is its own change.
  Naming them in the documents is what stops the widened scan from reading as
  complete coverage.
- Proved by mutation: a changed `success` inside an `at=` fragment, an `exit=0`
  on the plan that exits 4, and a pointer resolving to nothing each fail with
  the message a reader needs. `AT_LEAST` rises 2 → 6, which is the constant's
  stated purpose — left at 2 while four new samples existed, the whole directory
  could have left scope with the suite green.

---

## SE-088 — Absence cannot be established by asking about the thing that is absent

- Status: accepted
- Date / Pass: 2026-09-19 / 04-CLOSING-THE-REGISTER
- Evidence: observed on this machine 2026-09-19, MacPorts 2.12.6 (18 ports
  installed) and Homebrew 6.0.21 (201 formulae), recorded in
  `data/policy/provider-commands.toml`'s `source` and `read_on` for both
  entries; `invariant_01_a_verification_step_asserts_what_its_name_says` in
  `crates/application/tests/invariants.rs`.
- Extends: **SE-083**, which gave `VERIFY_DESTINATION` a success semantic
  because `port -q installed <name>` exits 0 for three different states. This is
  the mirror step, and the same observation reaches a different conclusion.
- Original assumption: `plan.v1.md`'s executor-debt item 3 said `PACKAGE_ABSENT`
  "needs a command and a success semantic", and warned that "nonzero exit means
  absent" is verification by exit status with the sign flipped. It named the
  wrong answer and left the right one open. `VERIFY_REMOVAL` has carried a
  verification check and **no command at all** since the planner was written —
  the only step in the plan asserting something an executor had no way to test.
- **The observation that decides the shape.** Asked about a **single name**,
  both providers answer an absent package, a name that was never a package, and
  a provider that could not run with the *same empty stdout*:

  | | installed | absent | not a package |
  |---|---|---|---|
  | `port -q installed <n>` | exit 0, names it | exit 0, empty | exit 0, empty |
  | `brew list --versions <n>` | exit 0, names it | exit 1, empty | exit 1, empty |

  So the obvious design — `StdoutNamesPackage` negated — reads "nothing came
  back" as "successfully removed", on the step that certifies a removal. That is
  the pass-03 defect exactly, one step further along: an answer carrying no
  information read as a yes, and this time on the far side of `REMOVE_SOURCE`
  where the package is already gone.
- Change: `VERIFY_REMOVAL` carries the provider's inventory verb **with no
  package argument** and `SuccessSemantic::StdoutOmitsPackage`. The answer is
  then a large positive output — 18 lines and 201 here.
  `names_no_package` in the planner is what keeps the package out of that argv;
  the name still travels in the plan, because the executor matches stdout
  against it, so every guard on it still applies.
- **The first version of this entry stated one condition and justified it with a
  false claim, and `security-reviewer` refused it.** It said "a provider that
  crashed, was killed or was truncated produces an empty stdout and is refused".
  Truncation does not produce an empty stdout: `read_bounded` returns the first
  `cap` bytes and sets a separate flag, and a killed process leaves whatever it
  flushed. A `brew list --versions` cut off at line 40 of 201 is **non-empty**,
  omits the name, and would have certified a removal it never saw — past
  `REMOVE_SOURCE`, where the package is already gone. The contract is now four
  conditions, checked in order: **exit 0, not truncated, non-empty, then the
  name test.** This is the second time in one night that an argument written
  down was taken for a guarantee, and both times what caught it was someone
  re-deriving the behaviour rather than reading the prose.
- **Two couplings the design rests on are now enforced rather than assumed**,
  also `security-reviewer`'s: the loader refuses `stdout_omits_package` on any
  verb the planner appends a package name to — `port installed <name>` without
  `-q` is the live counter-example, printing a non-empty sentence that does not
  contain the name, i.e. a certified removal of a port that is still there — and
  `embedded_table_loads_with_provenance` refuses an entry that omits the verb,
  which previously fell through to a literal `<verify_absent>` argv element with
  `placeholder: false`. `add-provider` now names four verbs instead of three.
- **A premise about casks was asserted and is now observed.**
  `verify_absent_gui_app = "none"` rested on "the inventory gives no name, so
  F-22 does not reach it". Had the bare inventory excluded casks, "the name is
  not in the list" would have been trivially true for every cask — a certified
  removal that never happened, in the cell claiming the hazard did not apply.
  Observed 2026-09-19, Homebrew 6.0.21 with nine casks installed: the bare
  `brew list --versions` lists casks **and** formulae, all nine appear, and it
  emits no `==>` section headers. So the absence verb is kind-agnostic where the
  presence verb is not.
- **The stdout is never recorded**, and that obligation is written now rather
  than when an executor exists: it is the machine's complete installed
  inventory, a strong fingerprint that routinely names internal tooling. What
  may be logged is the boolean result, the line count and the truncation flag.
- **Rejected: `port installed <name>` without `-q`.** It does print a real
  positive signal — its own not-installed sentence — and was observed doing so.
  It says the same thing for a name that is not a port, and matching a
  provider's prose is the mis-parse `.claude/rules/adapters.md` forbids: a
  parser meeting unrecognised output must fail rather than guess. A sentence is
  also free to change with a version or a locale in a way an inventory is not.
- **The cost, stated rather than discovered:** removing the last package a
  provider holds leaves an empty inventory, which this refuses. That is a false
  negative — "could not verify", never "verified" — and it is the direction this
  project chooses every time.
- What is left for the executor is **four stated conditions rather than a design
  question** — exit 0, not truncated, non-empty, then the name test — plus the
  never-log-stdout rule. This bullet said "one sentence" until 2026-09-19,
  contradicting this same entry thirty-five lines above, and it is the last
  substantive line here and so the one an executor pass would have acted on
  (`spec-guardian`). Item 3 joins item 4 as discharged **as data**, so two of
  `plan.v1.md`'s five inherited obligations are settled here — though only item
  4 is literally "read a field": item 3 is read the field, *then* honour four
  runtime conditions and a privacy rule, none of which is a field of the plan.
- Proved by mutation, twice: making the argv name the package fails the
  invariant test with the argv printed, and removing the command again fails it
  with "verifying a removal needs exactly one command".
- **Three more corrections from `spec-guardian`'s review, each recorded rather
  than folded in silently.** (1) The `human.rs` comment credited SE-084 and
  quoted a sentence that exists in no entry; the decision reversed is SE-083's,
  and SE-083's own "not done, deliberately" bullet is now superseded in place,
  since SE-032 makes a topic's entry the current rationale and that is the entry
  a reader looking for `StdoutNamesPackage` lands on. (2) The Homebrew `absent`
  column was dated 2026-09-19 while the observation beside it demonstrated only
  an installed formula and a name that never was one — the real-but-absent case
  was carried from the 2026-09-06 read. Re-observed the same day with `cowsay`:
  exit 1, no output. (3) The `plan.success_omits` catalogue string stated three
  of the four conditions and omitted exit 0, so the user-facing sentence was
  narrower than the contract.
- **The loader now depends on the planner**, which is worth saying because the
  guard's whole justification is that the two halves live in different files:
  `provider_commands::parse` calls `planner::names_no_package`. Same crate, no
  new layer edge, and `check-layering.sh` is unaffected — but the coupling the
  guard exists to enforce is now also the guard's own dependency.
- **The new command shape appears in no golden and no documented sample**, and
  that is the next thing to close. The seven goldens are `analyze` output;
  `plan.v1.md`'s whole-plan sample is all `MANUAL_REVIEW` and so carries no
  `VERIFY_REMOVAL`. By this project's own rule — a documented sample is the
  contract, prose is a claim — the shape is currently a claim plus one unit
  test.
- **`04-migration-and-safety.md` gains a removal-verification clause**, pointing
  at `plan.v1.md` rather than restating it. INV-01 names that chapter as *the*
  verification contract and it was silent on removal, while this contract had
  been written into four other places on one day.
- Policy version moves to `2026.09.19-1`, which is the read date of the new
  observation. Seven golden fixtures and six documented samples follow, and the
  guard SE-087 built is what found the two that had drifted.

---

## SE-089 — "The name appears in the output" was a substring rule guarding a removal

- Status: accepted
- Date / Pass: 2026-09-19 / 04-CLOSING-THE-REGISTER
- Evidence: measured on this machine 2026-09-19 — 201 installed Homebrew
  formulae, 18 of them a substring of another's name; inventory line shapes
  `node 26.7.0`, `cryptography 50.0.1 50.0.0`, `  autoconf @2.73_0 (active)`.
  Recorded in `SuccessSemantic`'s doc comments and `docs/schemas/plan.v1.md`.
- Extends: **SE-083**, which introduced `StdoutNamesPackage` for the right
  reason — exit status cannot answer on one provider — and defined the
  replacement loosely enough to reintroduce the same class of hole.
- Original assumption: "the package's own name in the command's standard
  output". Whether that meant a whole field or any substring was never stated,
  and the natural implementation is `stdout.contains(name)`.
- What it costs: `gh` is a substring of `ghidra`, `highway` and `libnghttp2`;
  `apr` of `apr-util`; `gd` of `gdk-pixbuf`. So a `VERIFY_DESTINATION` for `gh`
  whose install had **failed** is satisfied by `ghidra`'s line, and INV-01 puts
  that step in front of `REMOVE_SOURCE`. Fail-open, on the one gate the
  invariant exists to hold, reachable by 9% of the names on an ordinary machine.
- Change: both semantics now require the name to **equal the first
  whitespace-separated token** of a line, stated once and referenced from the
  other, and to be compared **literally** — never a substring, glob or regular
  expression. `is_safe_provider_name` admits `.` and `+`, and `lua@5.4` and
  `gtk+3` are real names, so a name compiled as a pattern either fails to
  compile or matches a different package.
- **Why a prefix test is not the fix**, since it is the first thing that comes
  to mind: `ghidra` *begins* with `gh`. And why it is "first token" rather than
  "the line is the name": MacPorts indents its inventory by two spaces and
  Homebrew appends versions, so neither equality nor prefix works on the whole
  line.
- What this does **not** do: no code implements either rule, because there is no
  executor. What shipped is the contract — which is the whole of the finding,
  since the defect was a specification that would have been implemented wrongly
  and defensibly by someone reading it carefully.
- Found while discharging executor-debt item 3 (SE-088) and independently by
  `security-reviewer` on the same diff. Recorded as **F-51**.

---

## SE-090 — INV-20's second sentence had been true of the documentation and of nothing else

- Status: accepted
- Date / Pass: 2026-09-19 / 04-CLOSING-THE-REGISTER
- Evidence: `Capability::UnprivilegedRead` in `crates/domain/src/provider.rs`;
  detection in both adapters' capability tables; the preference in
  `planner::command`; `invariant_20_a_detected_read_beats_a_stored_required`
  and `a_detected_read_never_relaxes_a_mutation` in `planner.rs`;
  `invariant_20_detection_relaxes_a_read_and_never_a_mutation` in
  `crates/application/tests/invariants.rs`. Seven golden fixtures gained the
  capability and nothing else.
- Extends: **SE-059**, which closed F-24's *value* — MacPorts' `verify`
  elevation became `none` on one machine's observation — and left the
  obligation open as F-35. Its reasoning is unchanged; what changes is that
  the obligation is now discharged.
- Original assumption: INV-20 ends "where behaviour can be detected at runtime,
  detection wins over stored knowledge". Four artefacts said so about this
  cell — the data file's own comment, SE-059, R-5a and
  `.claude/rules/policy-data.md` — and **no code implemented it**. Whether
  reading a provider needs administrator rights was a string in
  `provider-commands.toml`, true of the machine it was observed on. A registry
  is created with the installing user's umask, so a restrictive one makes the
  stored answer wrong and nothing noticed.
- Change: a tenth capability, detected **from a read the adapter already
  performs** — no new probe, nothing stat-ed, no permission inspected. The
  planner prefers it over the stored cell.
- **The strength of that evidence differs by provider, and saying "listing what
  is installed *is* the verify verb" was true of only one of them.** For
  MacPorts it is an identity: `inventory()` runs `port -q installed`, which is
  the `verify` argv character for character. For Homebrew it is an *inference* —
  the read observed is `brew info --json=v2 --installed`, the verb relaxed is
  `brew list --versions`. Same binary, same prefix, same user, so the inference
  is sound; it is still an inference and the entry claimed an identity.
- **Three narrowings, and they are the whole of why this is safe:**
  1. **Read operations only** (`READ_OPERATIONS`). Relaxing an elevation is
     safe exactly where INV-06 already requires `None`. Letting an observation
     about a *read* rewrite the elevation of `add` or `remove` would be this
     rule reaching past its evidence.
  2. **Never `Unavailable`.** A read that fails does not say *why* — a missing
     binary and a permission denial arrive identically — so this can confirm
     "no elevation needed" and can never assert the opposite. It cannot add a
     prompt nobody established.
  3. **`Unknown` falls back to the cell and does not fold to `Required`.**
     SE-059 recorded that rendering an unknown as "needs administrator rights"
     is itself the INV-06 defect: it trains a user to approve a password prompt
     for a read.
- **What this changes on any machine this project can test: nothing**, and that
  is said plainly rather than left for a reader to discover. The seven goldens
  moved by exactly the new capability key. **The first draft gave the wrong
  reason** — "both shipped cells already say `none`". There are **eight** read
  elevation cells, and MacPorts' two `_gui_app` ones say `unknown`
  (`provider-commands.toml:265-266`). Plans are identical because the GUI path
  halts at `MANUAL_REVIEW` before `command()`, not because the data agrees; and
  the cells that disagree are exactly the ones the first version of this branch
  would have overridden, keyed as it was on the un-suffixed operation name.
  What changes is that the answer is now *derived from the machine* wherever the
  machine can say, which is the sentence INV-20 has been making since the
  foundation pass.
- **Because the effect is invisible in the fixtures, the override is tested
  where it can be seen**: `command()` takes the commands table as an argument,
  so a unit test in `planner.rs` can force both read cells to `required` — which
  no real entry says — and assert that a detected capability beats them, that
  `Unknown` does not, that a `_gui_app` cell it never consulted survives, and
  that `add`/`remove` keep theirs. It is the third `fn invariant_*` living in
  `src/` — beside `invariant_01` in `domain/src/plan.rs` and `invariant_10` in
  `adapters/src/process/mod.rs` — which is **not** the same as the third test
  there: F-40's inventory counts 62 modules and 346 tests in `src/`, and this
  adds a 63rd module and two tests. The reason is the one F-40 itself names —
  a property not reachable without changing production visibility — and the
  cost is real, which is why the F-40 row records it. Shipping the branch
  untested was the alternative, and this pass has twice been shown what an
  untested branch under a confident comment is worth.
- Proved by mutation in both directions: deleting the preference fails the
  override test, and dropping the read-only narrowing fails the mutation test.
- **A consequence worth stating, because it changes what a data file is for:**
  on a healthy machine the four read elevation cells are now *dead data* — a
  live run never consults them. That is the intended direction
  (`.claude/rules/policy-data.md`: "this directory is the fallback and the
  explanation"), and it means a drifted read cell no longer shows up in any
  output. The fallback is still exercised by every `--replay` and
  `--from-snapshot` of a document written before this capability existed.
- **What is still open under F-35**, and it is the half that was always
  separable: per-cell provenance. Two cells now carry observations that the
  entry-level `source` and `read_on` can only describe in prose — MacPorts'
  `verify` mixes three read dates, Homebrew's cites a commit where the entry
  cites the FAQ. That is a schema change to the data file and is its own entry.

---
