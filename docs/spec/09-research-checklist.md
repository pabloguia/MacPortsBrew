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
Source: <https://docs.brew.sh/Support-Tiers>, read 2026-09-02.

- Tier 1: Apple Silicon, recent Apple-supported macOS, default prefix, bottles.
- Tier 2: prereleases, outdated Command Line Tools, non-default prefixes.
- Tier 3 (*not supported*): macOS versions out of CI/security updates,
  non-default prefixes, **and Intel x86_64 systems**.
- Unsupported: macOS 10.14 and earlier; multi-user setups.
- Stated: Homebrew **has stopped building new bottles for Intel**, and **will
  remove the ability to run Homebrew on Intel systems in or after
  September 2027**.

**Impact:** this is the product's central fact. It belongs in policy *data*
with this source and date (INV-20), not in code. Re-verify before every release
— the date may move in either direction.

### F-2 · `/usr/local` interferes with MacPorts builds (blocking)
Source: MacPorts FAQ, <https://trac.macports.org/wiki/FAQ>, read 2026-09-02.

`/usr/local` can interfere with MacPorts even though MacPorts installs to
`/opt/local`: the GNU autotools and gcc probe `/usr/local` for headers,
libraries and binaries. On Intel Macs, `/usr/local` is exactly where Homebrew
lives.

**Impact:** this is the mechanism by which accidental cross-provider
contamination actually happens, and it justifies INV-10 (sanitised environment
for provider invocations) and the `doctor` environment checks. Confirm the
current guidance and find the equivalent hazard in the other direction.

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
*Partially answered 2026-09-02* — `docs/research/R-1-homebrew-read-interfaces.md`
records one finding: `brew info <unknown-name>` taps `homebrew/core` (a 1.4 GB
clone into the prefix) before reporting the miss, and the project's quieting
variables do not prevent it. The survey below remains open.

Which commands give machine-readable, stable data for: installed packages;
user-requested (top-level) packages; dependencies; reverse dependencies;
formula and cask metadata; deprecation and disabled state; pinned state; taps;
prefix and configuration. Which produce JSON, and how stable is that JSON across
versions. Which commands trigger a network fetch or an auto-update as a side
effect, and how to suppress that. Confirm cask semantics and why casks are
outside migration scope.

### R-2 · Homebrew artifact availability
The reliable way to determine, without installing, whether *this* package on
*this* machine will use a prebuilt bottle or compile from source — including
when no bottle exists for the platform at all. Distinguish "no bottle
published", "bottle exists but not for this platform", "source build forced by
configuration". Determine what is answerable offline.

### R-3 · MacPorts read interfaces
The equivalents: installed; **requested** (authoritative user-intent record);
dependencies; dependents; variants and their effect; inactive versions;
port metadata; deprecated/replaced ports; prefix and configuration.
Which output is stable enough to parse and what the parsing failure mode is.

### R-4 · MacPorts binary archive availability
When does MacPorts install a prebuilt archive versus building locally.
Confirm the effect of non-default variants and of licence restrictions on
binary availability. This is what makes the availability model symmetric
(`02-domain-model.md`) rather than a Homebrew-only "bottle" concept.

### R-5 · Privilege requirements
Exactly which operations of each provider require elevation, on each
architecture and prefix. Confirm that everything the foundation pass does
requires none.

### R-6 · Primary-provider mechanisms
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
How each provider manages background services and where common stateful
packages keep their data. Enough to *detect* the classes in
`02-domain-model.md` and refuse to migrate them (INV-19) — not enough to migrate
them, which is a later pass.

### R-9 · Gatekeeper and release distribution
What happens to a downloaded, unsigned CLI binary on current macOS. Options and
costs for signing/notarisation versus ad-hoc signing with documented
instructions. Feeds `docs/decisions-pending.md`.

### R-10 · CI runners
Current availability of hosted Intel macOS runners on GitHub Actions, and the
fallback if there are none.

---

## Non-blocking research

- Homebrew and MacPorts release cadence and how often their output formats change.
- Existing tools in this space and what they get right or wrong.
- Runtime version managers worth naming, and their current standing.
- GitHub Wiki publishing workflows from a repository directory.
- GitHub topic conventions.
- Reasonable minimum macOS for MacPorts itself, to bound the support matrix.
