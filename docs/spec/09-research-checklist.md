# 09 — Research checklist

**Status:** Tier 3. Blocking items must be answered before the architecture is
fixed. Record every answer in `docs/research/<item-id>-<slug>.md`, following
the template in `docs/research/README.md`, with the source URL, the command
run, and the date.

**Rule:** every factual claim about a provider is (a) observed on this machine
with a read-only command, (b) cited from official documentation with a read
date, or (c) implemented as runtime detection. There is no fourth option, and
"I recall that…" is not one of the three.

---

## Pre-seeded findings (verify; do not trust blindly)

These were gathered on **2026-09-02** during the pre-implementation review.
Re-check each — some are time-sensitive by nature.

### F-1 · Homebrew support tiers (blocking)
*Answered 2026-09-02 — `docs/research/F-1-homebrew-support-tiers.md`.*
Source: <https://docs.brew.sh/Support-Tiers>, `last_review_date` 2026-08-27,
read 2026-09-02.

- Tier 1: Apple Silicon, recent Apple-supported macOS, default prefix, bottles.
- Tier 2: prereleases, outdated Command Line Tools, non-default prefixes.
- Tier 3 (*not supported*): macOS versions out of CI/security updates,
  non-default prefixes, **and Intel x86_64 systems**.
- Unsupported: macOS 10.14 and earlier; multi-user setups.
- Homebrew **states** it has stopped building new bottles for Intel, and states
  it **will remove the ability to run Homebrew on Intel systems in or after
  September 2027**. The first is Homebrew's stated policy, and is not literally
  true of the artifacts: Intel bottles were still published in August 2026
  (`htop` on the 16th, `sqlite` on the 25th, `cmake` on the 26th). The accurate
  characterisation on 2026-09-02 is **frozen at `sonoma` and eroding** — of
  8586 core formulae, 7808 (90.9%) have a usable bottle on Intel macOS 26
  against 8420 (98.1%) on Apple Silicon, and only 79 carry an Intel `tahoe`
  bottle (F-1 E4, census method in R-2).

**Impact:** this is the product's central fact. It belongs in policy *data*
with this source and date (INV-20), not in code — quoted as Homebrew's policy,
with the measured coverage as the separate, re-runnable observation. Re-verify
before every release: the date may move in either direction, and the census is
a snapshot of a deteriorating situation that must be re-measured, not quoted.

### F-2 · `/usr/local` interferes with MacPorts builds (blocking)
*Answered 2026-09-02 — `docs/research/F-2-usr-local-interference.md`.*
Source: MacPorts Guide §3.1.12,
<https://guide.macports.org/chunked/using.html#using.port.install>, read
2026-09-02. **Not** the FAQ: `trac.macports.org` served a bot-mitigation
interstitial to every URL form tried on 2026-09-02, so the FAQ's current text
is not recorded anywhere in this repository.

The hazard is confirmed from the Guide's trace-mode text: it names "conflicts
caused by files installed by other ports or in common system locations, such as
`/usr/local`" and prescribes `port -t` — a filesystem sandbox — as the remedy.
The narrower claim that **the GNU autotools and gcc probe `/usr/local`** is
plausible but **uncited**: the Guide does not say it, and the FAQ that may have
said it could not be read. Treat it as unverified until `trac.macports.org` is
readable again. On Intel Macs, `/usr/local` is exactly where Homebrew lives.

The other direction exists and Homebrew mitigates it **in its compiler shim,
not in `doctor`**: `shims/super/cc` drops any `-I` or `-L` flag whose path lies
under `/opt/local` (also `/sw`, `/opt/X11`), always on, for every source build.
`brew doctor` has **no** MacPorts, Fink or `/opt/local` check at any version
examined — none at HEAD, none at 1.9.0, 2.7.0 or 3.6.0 — so MacPortsBrew's
`doctor` must implement that check itself and cannot delegate it.

**Impact:** this is the mechanism by which accidental cross-provider
contamination actually happens, and it motivates INV-10 and the `doctor`
environment checks. Note that MacPorts already sanitises its build environment
to an allow-list, so the residual exposure is a filesystem search path rather
than an environment variable — which is why the remedy is trace mode.

### F-3 · Toolchain minimum macOS versions (blocking — affects the whole product)
Sources: <https://go.dev/wiki/MinimumRequirements> and
<https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html>, read 2026-09-02.

- Go 1.25 and 1.26 require **macOS 12+**; Go 1.27 and later require **macOS 13+**.
- Rust `x86_64-apple-darwin` supports **macOS 10.12+**; `aarch64-apple-darwin`
  supports macOS 11+.

**Impact — read this carefully.** The users this project exists to serve are on
Intel Macs that Apple and Homebrew have moved past. Many of those machines top
out below macOS 12. A current Go toolchain cannot produce a binary that runs on
them; Rust can. If Go is chosen, the product excludes part of its own target
audience — that may still be the right trade, but it must be a *stated,
justified* trade in the ADR, not an accident.

**RESOLVED 2026-09-02.** The owner set the floor at macOS 10.12 (Intel) /
11.0 (Apple Silicon), which excludes current Go toolchains and points to Rust.
See ADR-0002. What remains is verification, not decision — see that ADR's
"must still verify" list.

**Required of the language ADR:** state the minimum macOS version MacPortsBrew
will support, name the machines that implies, and show that the chosen toolchain
can actually build and run there. Verify the current numbers — they change with
every toolchain release.

---

## Blocking research

### R-1 · Homebrew read interfaces
*Answered 2026-09-02 — `docs/research/R-1-homebrew-read-interfaces.md`*, with
two sub-items UNRESOLVED (`brew tap-info --json`, blocked by this repository's
own hook; and whether "no bottle anywhere" is separable offline). Its first
finding is a standing warning: `brew info <unknown-name>` taps `homebrew/core`
(a 1.4 GB clone into the prefix) before reporting the miss, and the quieting
variables do not prevent it — never pass an unknown name to `brew info`.

Which commands give machine-readable, stable data for: installed packages;
user-requested (top-level) packages; dependencies; reverse dependencies;
formula and cask metadata; deprecation and disabled state; pinned state; taps;
prefix and configuration. Which produce JSON, and how stable is that JSON across
versions. Which commands trigger a network fetch or an auto-update as a side
effect, and how to suppress that. Confirm cask semantics and why casks are
outside migration scope.

### R-2 · Homebrew artifact availability
*Answered 2026-09-02 — `docs/research/R-2-homebrew-artifact-availability.md`.*
The reliable way to determine, without installing, whether *this* package on
*this* machine will use a prebuilt bottle or compile from source — including
when no bottle exists for the platform at all. Distinguish "no bottle
published", "bottle exists but not for this platform", "source build forced by
configuration". Determine what is answerable offline.

### R-3 · MacPorts read interfaces
*Answered 2026-09-02 — `docs/research/R-3-macports-read-interfaces.md`*, from
documentation and source; the parser-level items need a machine with MacPorts.
The equivalents: installed; **requested** (authoritative user-intent record);
dependencies; dependents; variants and their effect; inactive versions;
port metadata; deprecated/replaced ports; prefix and configuration.
Which output is stable enough to parse and what the parsing failure mode is.

### R-4 · MacPorts binary archive availability
*Answered 2026-09-02 — `docs/research/R-4-macports-binary-archives.md`.*
When does MacPorts install a prebuilt archive versus building locally.
Confirm the effect of non-default variants and of licence restrictions on
binary availability. This is what makes the availability model symmetric
(`02-domain-model.md`) rather than a Homebrew-only "bottle" concept.

### R-5 · Privilege requirements
*Answered 2026-09-02 — `docs/research/R-5-privilege-requirements.md`*; the
MacPorts half is cited, not observed, and is re-checked on a MacPorts machine.
Exactly which operations of each provider require elevation, on each
architecture and prefix. Confirm that everything the foundation pass does
requires none.

### R-6 · Primary-provider mechanisms
*Answered 2026-09-02 — `docs/research/R-6-primary-provider-mechanisms.md`.*
MacPorts' own selection mechanism, Homebrew's linking behaviour, versioned
executable naming, and what each actually guarantees. Compare against a
MacPortsBrew-owned shim directory using the criteria in
`05-environment-and-primary-provider.md`. Recommend one; justify it.

### R-7 · Implementation language — **RESOLVED 2026-09-02: Rust**
Decided by the project owner; ADR-0002 is *accepted* and holds the
constraints, the evaluation and the list of what the foundation pass must
still verify. What remains here is **verification, not decision**: each item
on that list, mechanically checked, before the bootstrap
(`10-first-pass-work-order.md`, step 2). In particular, the TUI and terminal
crates are checked for licence compatibility, deployment floor and East Asian
double-width handling together — discovering any of the three after the UI is
written means rewriting it.

### R-8 · Stateful and service packages
*Answered 2026-09-02 — `docs/research/R-8-stateful-and-service-packages.md`.*
How each provider manages background services and where common stateful
packages keep their data. Enough to *detect* the classes in
`02-domain-model.md` and refuse to migrate them (INV-19) — not enough to migrate
them, which is a later pass.

### R-9 · Gatekeeper and release distribution
*Answered 2026-09-02 — `docs/research/R-9-gatekeeper-and-distribution.md`*.
Its UNRESOLVED experiment and its ad-hoc-signing recommendation are both
**superseded by ADR-0005** (2026-09-02): the experiment now documents the path
not taken rather than gating a release.
What happens to a downloaded, unsigned CLI binary on current macOS. Options and
costs for signing/notarisation versus ad-hoc signing with documented
instructions. Feeds `docs/decisions-pending.md`.

### R-10 · CI runners
*Answered 2026-09-02 — `docs/research/R-10-ci-runners.md`.*
Current availability of hosted Intel macOS runners on GitHub Actions, and the
fallback if there are none.

### R-11 · Apple signing and notarisation toolchain
*Partially answered 2026-09-06 — `docs/research/R-11-apple-signing-toolchain.md`.
Three of the five closed from official documentation; two remain open and need
something other than reading. Still blocks the implementation pass that writes
the release workflow, not ADR-0005, which is decided.*

**One premise below is wrong and is left in place so the correction is
visible.** `macos-15-intel` is not a free standard runner: GitHub's
`runner-images` README lists it as an alias of `macos-15-large`, and GitHub Docs
says larger runners "are always charged for, even when used by public
repositories". Bare `macos-15` is the free arm64 image. An Intel release job
costs money on a public repository, which is a cost DP-007 did not weigh.

**Answered:** a standalone binary **cannot be stapled at all** ("Although
tickets are created for standalone binaries, it's not currently possible to
staple tickets to them"), so the `.dmg` is the only staplable container that
delivers a single binary and is the *only* offline-verifiable path — the
opposite of what the question below supposes. And the `.pkg` is signed by a
**Developer ID Installer** identity, a different certificate from the Developer
ID Application identity that signs the Mach-O and the `.dmg`, so shipping all
three needs two.

**Still open:** whether the hardened runtime wants an entitlement for a tool
that spawns `brew` and `port` (needs one notarisation attempt), and what a
quarantined, notarised, unstapled binary does offline (needs a machine with the
quarantine attribute set and the network off).

The original five, as filed:
Five things the signing decision assumes and this repository has not verified:
**whether stapling a `.pkg` or `.dmg` covers a binary copied out of it or only
the container** — if only the container, the `.dmg` buys nothing the bare binary
does not and is dropped; whether `codesign`, `notarytool` and `stapler` are
present on the free `macos-15-intel` standard runner image and at what Xcode
floor, which R-10 did not establish; which certificate signs the `.pkg` —
R-9 C-1 lists an "installer certificate" among the acceptable kinds without
saying it signs the `.pkg`, so this was wrongly treated as answered; whether the
hardened runtime requires any entitlement for a tool that spawns `brew` and
`port` (expected: none, worth confirming rather than discovering at
notarisation time); and what a quarantined, notarised,
**unstapled** binary does on a machine with no network, which is the state of
exactly the user this project is written for.

Already answered by R-9 (2026-09-02) and **not** open, despite an earlier draft
of ADR-0005 listing them: that a Developer ID **Application** certificate signs
Mach-O files, disk images and command-line tools (C-3) — the `.pkg` is not in
that list and stays open above — whether the hardened runtime is required for a command-line
tool (C-1, C-3, C-4), and what container `notarytool` accepts for a standalone
binary (C-2).

### R-12 · Cask migratability
*Answered 2026-09-04 — `docs/research/R-12-cask-migratability.md`.*
Whether a Homebrew cask has a MacPorts destination at all, and whether a cask
can consume MacPorts components in place of the Homebrew formulae it declares.
Answers: sometimes a destination exists and it is never the same operation, so
INV-01's verification cannot mean for a cask what it means for a formula; and
0.32% of casks declare a formula dependency, which a MacPorts equivalent on
`PATH` can satisfy and the next `brew upgrade --cask` un-does. Raised by DP-012.

Found on the way, and larger than the question: Homebrew records
`installed_on_request` for casks in the on-disk `INSTALL_RECEIPT.json`, which
falsifies the absolute six places in this repository stated (F-28) and gives
DP-012 a third option the charter may forbid.

### R-13 · What revocation actually does to a published artifact
*Open. Raised 2026-09-04 by DP-009. Blocks nothing; it decides how much of
`SECURITY.md`'s withdrawal plan is load-bearing.*
ADR-0005 asserts that a revoked Developer ID certificate, or a revoked
notarisation ticket, stops **previously published** artifacts from launching,
and that the secure timestamp does not answer revocation the way it answers
expiry. Nothing in this tree cites that. Checked on 2026-09-04 and found not to
say it: Apple's Gatekeeper page
(<https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web>)
contains no occurrence of "revoke" or "revocation"; the notarisation page
(<https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>)
is JavaScript-rendered and yielded no text.

What an answer needs: whether Gatekeeper consults revocation state for a
Developer ID signature and when — first launch only, or every launch; whether a
**stapled** ticket keeps an artifact launching after the ticket is revoked, and
whether it differs offline; whether an already-launched-once binary is
re-evaluated; and whether any of it differs for a bare Mach-O run from a shell,
which R-9 already found Apple does not document. The distinction that matters
for the plan is *published-and-already-downloaded* versus *downloaded after
revocation* — only the first makes the outbound channel urgent.

### R-14 · What actually reaches a user of a published binary
*Open. Raised 2026-09-04 by the security review of DP-009's plan. Decides how
much of `SECURITY.md`'s runbook is a real channel and how much is a gesture.*
The withdrawal plan originally claimed a repository security advisory
"syndicates to the GitHub Advisory Database" and was therefore "the only
outbound channel with reach". That is uncited, and doubtful in this project's
shape: the global database is curated around packages in supported ecosystems,
and MacPortsBrew publishes a signed binary through Releases and no ecosystem
package. The claim is withdrawn until answered.

What an answer needs, each with a source and a read date: whether a repository
advisory for a project with no ecosystem package appears in the global database
at all; what a "Watch → Releases only" subscriber actually receives, and when;
whether `releases.atom` includes deleted or de-listed releases; and whether
deleting a release leaves the tag, the `SHA256SUMS` asset and the build
provenance attestations in place — the plan's step 2 depends on the last of
these and states it without a citation.

---

## Non-blocking research

- Homebrew and MacPorts release cadence and how often their output formats change.
- Existing tools in this space and what they get right or wrong.
- Runtime version managers worth naming, and their current standing.
- GitHub Wiki publishing workflows from a repository directory. *Partially
  answered 2026-09-02 (R-10, final section): a wiki is a Git repository at
  `<repo>.wiki.git`, filenames become page titles and nesting does not survive,
  so a wiki is a generated mirror or nothing. Whether `GITHUB_TOKEN` may push to
  it is UNRESOLVED — there is no `wiki` permission scope.*
- GitHub topic conventions. *Answered 2026-09-02 (R-10, final section): up to 20
  topics, lowercase letters, digits and hyphens only, 50 characters each.*
- Reasonable minimum macOS for MacPorts itself, to bound the support matrix.
  *Answered 2026-09-02 (R-4): MacPorts 2.12.6 ships package installers down to
  macOS 10.12 Sierra, coinciding with this project's Intel floor.*
