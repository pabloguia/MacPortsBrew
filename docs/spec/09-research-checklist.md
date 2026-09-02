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
*Answered 2026-09-02 — `docs/research/R-9-gatekeeper-and-distribution.md`*, with
one UNRESOLVED experiment named there and required before the first release.
What happens to a downloaded, unsigned CLI binary on current macOS. Options and
costs for signing/notarisation versus ad-hoc signing with documented
instructions. Feeds `docs/decisions-pending.md`.

### R-10 · CI runners
*Answered 2026-09-02 — `docs/research/R-10-ci-runners.md`.*
Current availability of hosted Intel macOS runners on GitHub Actions, and the
fallback if there are none.

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
