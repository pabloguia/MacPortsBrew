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
artifacts and repository metadata. **The identity describes the finished
product; a release surface describes that release** — README, wiki, CLI help,
release artifacts and repository metadata, and not the specification, which
answers to the charter like everything else here. Where the two differ, as they
do while a build plans migrations and has no `migrate` command, the surface
states what the build does, plainly and in its lead, rather than quietly
narrowing the sentence above. That is a deliberate exception to "consistently" and not drift:
an audit that finds the charter claiming more than the README has found the two
doing their jobs, and should leave both alone.

## Why it exists

Homebrew lists "Intel x86_64 systems running macOS" under **Tier 3**, which its
support page defines as *not supported*: these configurations "fall far outside
Homebrew's testing infrastructure and may fail to function reliably, even if
basic installation is possible". Homebrew further states it has stopped building
new bottles for Intel, and that it will **remove the ability to run Homebrew on
Intel systems in or after September 2027**
(source: <https://docs.brew.sh/Support-Tiers>, read 2026-09-02).

**Which macOS versions that covers is a timeline, the timeline is a forecast,
and the forecast is keyed to Apple's releases rather than to the calendar.** The
page says so — "expected Tier classifications based on Apple's release cycle and
Homebrew's CI coverage". Its row "As of September 2026" puts Intel Tier 3 at
Catalina 10.15 through Tahoe 26; the next row, "Expected in or after September
2026", would move Catalina into Unsupported, and is keyed to a macOS the current
row does not yet list; the row after that expects Intel to be Unsupported at
every version. Quote a row and say which, never the range on its own. And where
the timeline and the page's own *Unsupported* list could be read against each
other, the list governs: it is the definition, the timeline is a plan (F-26,
re-read 2026-09-04).

**Tier 3 is degradation with a deadline, not a machine Homebrew refuses to run
on**, and the distinction is not pedantry: it separates a reader whose
installation has already stopped working from one working against an announced
date. Homebrew keeps a second and harsher
category named *Unsupported* — a configuration in which "Homebrew will not run
without third-party patches or modifications" — and Intel is not in it on the
current row; the macOS versions in that category are Mojave 10.14 and earlier.
Nor is the bottle freeze yet total:
measured against `homebrew/core` on 2026-09-02, *frozen at `sonoma` and eroding*
describes the artifacts and *stopped* does not
([`../research/F-1-homebrew-support-tiers.md`](../research/F-1-homebrew-support-tiers.md)).
This project's own policy data says `DEGRADED`, and `status` prints `Degraded`
on an Intel machine; this chapter now says the same thing, because a reason for
existing that overstates itself is weaker than the true one and fails at the
first click on the citation. Note the vocabularies collide: *not supported* is
Homebrew's gloss on Tier 3, while this tool reserves its own "Not supported" for
Homebrew's Unsupported category. Where both appear, say whose word it is.

That is still a dated migration deadline for every Intel Mac user with a
Homebrew installation. MacPorts covers a much wider range of macOS versions and
both architectures: build farms for `x86_64` back to macOS 10.6 and `arm64` from
11, with MacPorts 2.12.6 package installers published down to 10.12 Sierra
([`../research/R-4-macports-binary-archives.md`](../research/R-4-macports-binary-archives.md),
read 2026-09-02). A builder existing is necessary and not sufficient; whether a
given package has a published archive is detected, never assumed.

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
