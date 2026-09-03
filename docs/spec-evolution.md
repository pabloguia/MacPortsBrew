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
     legacy receipt field, casks carry no intent field at all so a cask's
     ownership is `UNKNOWN`, and `brew leaves` answers a different question.
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

# Proposed by 02-ARCHITECTURE-REVIEW, not yet accepted

These seven entries are `Status: proposed`. They are grouped rather than filed
under their topics **deliberately and temporarily**: a proposal that sits beside
accepted rationale reads as settled, and these need triaging as a set. On
acceptance each merges into its topical section above under SE-032, keeping its
identifier. On rejection the entry stays, because a rejection is the current
rationale for something's absence.

Evidence for all eleven is in
[`passes/02-ARCHITECTURE-REVIEW-findings.md`](passes/02-ARCHITECTURE-REVIEW-findings.md),
finding ids in each entry.

## SE-044 — Homebrew's "Tier 3" and "Unsupported" are different things, and the charter conflates them
- Status: proposed (Tier 2 — needs the owner)
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW
- Original assumption: `00-charter.md:37` and `README.md:31` — "Homebrew
  classifies Intel x86_64 macOS as **Tier 3 — not supported**".
- Change: say "Tier 3 — degraded, with bottle builds stopped and an announced end
  date", and reserve "not supported" for what Homebrew labels Unsupported.
- Rationale: the source treats them as distinct categories. Intel on Catalina
  10.15 through Tahoe 26 is Tier 3 and Homebrew runs there today; "Unsupported" is
  Mojave 10.14 and earlier, defined as a configuration where Homebrew "will not run
  without third-party patches or modifications". The project's own data model gets
  this right — `provider-support.toml` maps Intel Tier 3 to `DEGRADED` and only
  `macos_max = "10.14"` to `UNSUPPORTED` — so the binary is more honest than the
  prose. The error runs in the direction that favours the tool, which is the
  direction that costs credibility when a reader checks the citation.
- Evidence: <https://docs.brew.sh/Support-Tiers>, re-read 2026-09-02. The timeline
  sentence and the September 2027 date are unchanged and remain correctly cited.
  Finding F-11.
- User impact: a reader on an Intel Mac is no longer told their installation is
  already unsupported when it is degraded.
- Architectural impact: none. Data already models it correctly.
- Compatibility impact: none.

## SE-046 — Ownership with no provider record is `UNKNOWN`, not an inferred `TOP_LEVEL`
- Status: proposed
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
- Evidence: `crates/domain/src/ownership.rs:72-82`; observed distribution; F-20.
- User impact: either nine casks leave the default top-level count, or the
  documentation stops claiming a guarantee the code does not provide.
- Architectural impact: `Ownership::Unknown` becomes reachable for installed rows.
- Compatibility impact: top-level counts and golden files change if the code moves.

## SE-049 — A verification check must be able to fail
- Status: proposed
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
- Evidence: `planner.rs:301-307`, `plan.rs:93-94`, `plan.rs:206-216`; finding F-06.
- User impact: none until execution exists.
- Architectural impact: `VerificationCheck` gains a variant; INV-01's enforcement
  point becomes pairwise.
- Compatibility impact: plan JSON gains a value; `schemaVersion` may need a bump.

## SE-050 — The no-shell guarantee needs a check, or the claim needs correcting
- Status: proposed
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW
- Original assumption: `04-migration-and-safety.md:257-258` — "There is no shell to
  inject into. Enforced by a lint rule, not by discipline."
- Change: either add a check that no `CommandSpec` program is a shell literal
  outside the executor's own tests — better, make `CommandSpec::program`
  constructible only from a detected prefix plus a relative executable, so the type
  carries the guarantee — or correct the sentence to say the control is a
  per-adapter run-time allow-list.
- Rationale: the existing lint asserts *where* a subprocess is spawned, not *what*.
  The executor's only program check is `is_absolute()`, and a shell is absolute.
  What actually holds the line today is an allow-list in each of the two adapters —
  good, and checked at run time rather than under `debug_assert!` — but that is
  discipline, which the sentence says it is not relying on. A third adapter added by
  a later pass would pass clippy, the layering check and every test.
- Evidence: `check-layering.sh:58`, `process/system.rs:171`; finding F-08.
- User impact: none directly.
- Architectural impact: a type-level guarantee is the stronger option and removes
  the need for the grep.
- Compatibility impact: none.

## SE-051 — A snapshot is untrusted input and is validated at deserialisation
- Status: proposed
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW
- Original assumption: `--from-snapshot` accepts a file whose schema version matches.
- Change: validate every name, version and path in a deserialised snapshot with the
  same predicates the adapters apply to provider output, failing with
  `SnapshotUnusable`; and give `PlanCommand` a fallible constructor that refuses an
  argv element that is empty or begins with `-` unless it came from the verb table.
- Rationale: `check_schema` compares an integer and nothing else, and
  `--from-snapshot` is global, so it reaches `plan`. Names from the file become argv
  elements and even executable names, with no `--` separator and without passing any
  adapter allow-list, because the allow-lists live in the adapters and the planner
  does not go through them. Sharing a snapshot for diagnosis is a workflow the
  documentation invites. Today the consequence is a misleading plan; when the
  executor runs `PlanCommand.argv` as the contract requires, it is flag injection.
- Evidence: `app/mod.rs:66-67`, `snapshot.rs:226-238`, `planner.rs:219, :251, :254,
  :377`; finding F-09.
- User impact: a malformed or hostile snapshot is rejected with a clear error.
- Architectural impact: validation moves to the boundary rather than living only in
  the adapters.
- Compatibility impact: snapshots containing names the predicates reject stop
  loading.

## SE-052 — The support matrix states configuration until a CI run exists
- Status: proposed
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
- Evidence: empty `git remote -v`; `scripts/support-matrix.sh`; finding F-12.
- User impact: the matrix stops overstating what has been observed.
- Architectural impact: none — the change is in the generator, not the artifact.
- Compatibility impact: the checked-in matrix regenerates.

## SE-053 — `doctor` gains an owner-writable finding kind
- Status: proposed
- Date / Pass: 2026-09-02 / 02-ARCHITECTURE-REVIEW
- Original assumption: the path audit reports world-writable and group-writable
  directories, escalating world-writable ones that precede a system path.
- Change: add `PathFindingKind::OwnerWritable`; escalate *any* writable kind to
  `Error` when it precedes a system path; emit a finding when no system directory
  appears on `PATH` at all.
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
- Compatibility impact: `--json` gains a finding kind; more machines report findings.
