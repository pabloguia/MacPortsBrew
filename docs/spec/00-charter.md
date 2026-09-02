# 00 — Charter

**Status:** Tier 2. Changes require an explicit human decision.

## Identity

**Product name:** MacPortsBrew
**CLI command:** `macportsbrew`, with the short alias **`mpb`** installed alongside it
**Repository:** `macportsbrew`

The name pairs the two providers and yields the alias `mpb` — a deliberate nod to
*Música Popular Brasileira*. Two things follow that the project should stay aware of
rather than pretend away: leading with "MacPorts" can read to a newcomer as a
MacPorts-first tool, so the README and wiki must state the peer-provider position
explicitly and early; and `mpb` is already the executable name of MIT Photonic-Bands,
a scientific package a user may have installed by other means — neither provider
offered it under that name when checked (ADR-0003 records the observation). See
ADR-0003.

One line:

> Safely migrate and manage packages between Homebrew and MacPorts on macOS.

Longer:

> MacPortsBrew is an open-source package-manager orchestrator for macOS. It
> analyses what you have installed under Homebrew and MacPorts, maps equivalent
> packages between them, explains which provider is the better choice for each
> package on *your* machine, and executes migrations in either direction without
> ever removing something before its replacement is installed and verified.

Use this identity consistently across README, docs, wiki, CLI help, release
artifacts and repository metadata.

## Why it exists

Homebrew classifies Intel x86_64 macOS as **Tier 3 — not supported**, has
**stopped building new bottles for Intel systems**, and states it will **remove
the ability to run Homebrew on Intel systems in or after September 2027**
(source: <https://docs.brew.sh/Support-Tiers>, read 2026-09-02).

That turns a vague "degradation" story into a dated migration deadline for every
Intel Mac user with a Homebrew installation. MacPorts continues to support a
much wider range of macOS versions and both architectures.

MacPortsBrew exists to make that transition safe, reviewable and reversible —
and to remain useful afterwards, on any Mac, as a general orchestration layer.

**This dated fact is evidence, not architecture.** It lives in a versioned
policy data file with its source and read date. It is never hardcoded into
control flow, and it must be re-verified — Homebrew may extend, accelerate or
reverse the timeline.

## Who it is for

1. Intel Mac users with an existing Homebrew installation, facing the deadline above.
2. Users on any Mac who want Homebrew and MacPorts installed side by side without breaking either.
3. Users who want to move specific packages back to Homebrew (Apple Silicon, Tier 1, bottles available).
4. Users who simply want to know what is installed, by whom, and which binary actually runs.

## Long-term positioning

MacPortsBrew is **not** a one-way Intel escape hatch. It is:

> A safe orchestration, migration and coexistence layer for Homebrew and MacPorts on macOS.

Homebrew and MacPorts are **peer providers**. Neither is globally better.
Recommendations are a function of `machine × package × provider × current support state`.

The provider abstraction MUST NOT hardcode "two providers". A third provider
(pkgsrc, Nix, Fink) must be addable without redesigning the domain — but adding
one is explicitly out of scope for the initial releases.

## Non-goals

MacPortsBrew does **not**:

- become a package manager, build packages, or host binaries;
- manage language runtimes directly (it may *recommend* a runtime manager);
- manage Mac App Store apps or arbitrary `.app` bundles;
- consolidate or deduplicate the two providers' dependency trees;
- attempt to make one provider's libraries satisfy the other's dependencies;
- require Homebrew, MacPorts, network access, or an LLM in order to run;
- send telemetry.

## Discoverability

The repository, README and wiki are product surfaces. They must use the
terminology people actually search for — *Homebrew to MacPorts migration*,
*MacPorts to Homebrew migration*, *Homebrew Intel Mac support*, *Homebrew
bottles and source builds*, *Homebrew and MacPorts together* — in natural
sentences. Write for a human reader first. No keyword stuffing.

Suggested repository description:

> Safely migrate and manage packages between Homebrew and MacPorts on macOS. Intel and Apple Silicon.

Suggested topics: `homebrew`, `macports`, `macos`, `intel-mac`, `apple-silicon`,
`package-manager`, `migration`, `homebrew-migration`, `macports-migration`,
`cli`, `tui`, `developer-tools`. Confirm current GitHub topic conventions before finalising.

## Legal posture

MacPortsBrew is licensed under the **Apache License 2.0**
(see [`../adr/0001-licence-apache-2-0.md`](../adr/0001-licence-apache-2-0.md)).

Homebrew and MacPorts are third-party projects and their names are theirs.
MacPortsBrew is unaffiliated. README and docs MUST carry a short
non-affiliation notice. MacPortsBrew interacts with both projects only through
their public, supported command-line interfaces — it never vendors, forks,
patches or scrapes them, and never generates load on their infrastructure
beyond what a normal user's own commands would.
