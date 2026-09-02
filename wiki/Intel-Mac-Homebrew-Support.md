# Intel Mac Homebrew Support

**Everything on this page is dated. Check the source before acting on it.**
Homebrew may extend, accelerate or reverse any of it, and a page like this one
read three years late is worse than no page at all.

## What Homebrew says

As of **2026-09-02**, reading <https://docs.brew.sh/Support-Tiers> (the page's
own `last_review_date` was 2026-08-27):

- **Intel x86_64 systems running macOS are listed under Tier 3 — not
  supported.**
- Homebrew states it **has stopped building new bottles for Intel systems**.
- Homebrew states it **will remove the ability to run Homebrew on Intel systems
  in or after September 2027**, and its timeline shows Intel x86_64 as
  "Unsupported: all macOS versions" expected at that point.
- The stated minimum is macOS Sonoma (14) or higher, and the stated CPU
  requirement is Apple Silicon
  (<https://docs.brew.sh/Installation>, read 2026-09-02).

Those are stated requirements, not enforced floors. The machine these examples
came from runs Homebrew 6.0.21 on Intel macOS 26.6.2 without complaint.

## What is actually true of the artifacts

Measured against `homebrew/core` on **2026-09-02**, "stopped building new
bottles for Intel" is Homebrew's stated policy and **not yet literally true**
([`../docs/research/F-1-homebrew-support-tiers.md`](../docs/research/F-1-homebrew-support-tiers.md)):

- New Intel bottles were published in August 2026 — `htop` 3.5.3 on 2026-08-16,
  `sqlite` 3.53.4 on 2026-08-25, `cmake` 4.4.3 on 2026-08-26.
- **6680 of 8586 formulae** carried an Intel `sonoma` bottle at their current
  stable version, against **7277 with `arm64_sonoma`**. Roughly 600 formulae
  have already lost their Intel bottle.
- Only **79 formulae** carried an Intel `tahoe` (macOS 26) bottle.

The accurate description on that date is **frozen at `sonoma` and eroding**,
not stopped. Intel bottle coverage on the example machine was 90.9%, against
98.1% on Apple Silicon.

Why "frozen at `sonoma`" still works on macOS 26: Homebrew has a documented
older-macOS fallback. A bottle built for an older macOS with the same
architecture is poured on a newer one, so an Intel `sonoma` bottle installs
fine on Intel macOS 26
([`../docs/research/R-2-homebrew-artifact-availability.md`](../docs/research/R-2-homebrew-artifact-availability.md),
2026-09-02).

## What this means for you, concretely

**Not "migrate everything now".** What actually degrades, in order:

1. **New formulae and new versions increasingly have no Intel bottle**, so they
   compile from source on your machine. That is slower and needs the Command
   Line Tools, but it works. See
   [Understanding Bottles and Source Builds](Understanding-Bottles-and-Source-Builds.md).
2. **Coverage erodes gradually.** Individual packages lose their Intel bottle
   at different times; there is no single day when everything changes.
3. **In or after September 2027, per Homebrew's stated plan, Homebrew stops
   running on Intel at all.** That is the date that matters, and it is the one
   most likely to move.

Nothing breaks on a schedule. What you have installed keeps working until you
change it.

## What MacPortsBrew does with this

**It treats the deadline as data, and it never hardcodes it.** The date lives in
`data/policy/provider-support.toml` with its source and read date, alongside
every other provider fact (INV-20). No `if` statement in the program contains
September 2027.

The policy rule that uses it — `SOURCE_SUPPORT_ENDING` — names no provider. It
fires when *whichever provider currently holds a package* has published an end
of support for this machine, that date is inside the horizon, **and** some
other provider offers a strong equivalent with a prebuilt artifact ready. Run
it on a machine where the other provider has the deadline and it points the
other way.

So the deadline alone never moves a package. It has to be paired with somewhere
better to go. You can see whether it fired for any package:

```sh
macportsbrew explain <package>
```

`doctor` reports the state directly, with the citation attached:

```
[Warning] homebrew support for this machine is Degraded
          (source: https://docs.brew.sh/Support-Tiers, read 2026-09-02).
[Warning] homebrew has announced the end of support for this machine in or
          after 2027-09-01 (source: https://docs.brew.sh/Support-Tiers,
          read 2026-09-02). This is dated data; re-check it before relying on it.
```

## What MacPorts offers instead

MacPorts continues to support both architectures and a much wider range of
macOS versions. MacPorts 2.12.6 ships package installers down to **macOS 10.12
Sierra**
([`../docs/research/R-4-macports-binary-archives.md`](../docs/research/R-4-macports-binary-archives.md),
read 2026-09-02), which is exactly MacPortsBrew's own Intel floor: below that,
neither MacPortsBrew nor the provider it would recommend has a supported path.

MacPorts is not automatically the better choice for a given package — it may
need to compile where Homebrew would pour, and it has its own coverage gaps.
That is what `analyze` is for.

## This is not an escape hatch

MacPortsBrew is not a one-way Intel evacuation tool. It is an orchestration and
coexistence layer, and
[MacPorts to Homebrew](MacPorts-to-Homebrew-Migration.md) is a first-class
direction with the same planner. If you are on Apple Silicon, none of this page
applies to you and Homebrew is very likely the better provider for most of what
you have.

## Re-check this page when

- Homebrew changes the Support Tiers page — its `last_review_date` is the thing
  to watch.
- September 2027 approaches, or moves.
- Intel bottle coverage changes materially in either direction.

If you find this page out of date, the
[documentation issue template](../.github/ISSUE_TEMPLATE/documentation.yml)
asks for the source and the date you read it, which is all a correction needs.
