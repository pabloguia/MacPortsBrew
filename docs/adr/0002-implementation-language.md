# ADR-0002 — Minimum macOS target and implementation language

- **Status:** accepted
- **Date:** 2026-09-02 (minimum macOS) · 2026-09-02 (language confirmed by the project owner)
- **Pass:** pre-implementation
- **Resolves:** DP-001 · DP-002
- **Sources:** the toolchain and provider facts here are F-1 and F-3 in
  `../spec/09-research-checklist.md` (Homebrew support tiers; Go and Rust
  minimum macOS versions), read 2026-09-02. The Swift, Zig and Python
  remarks in the evaluation table are the owner-accepted assessment of
  2026-09-02 and were not separately sourced; they bear on rejected
  options only.

## Part 1 — Minimum macOS target (decided)

**Decision: macOS 10.12 Sierra on Intel x86_64; macOS 11.0 Big Sur on Apple Silicon.**
Decided by the project owner.

The Apple Silicon figure is not a choice — 11.0 is the first macOS that ran on
that hardware. The real decision is the Intel floor, and 10.12 is a deliberate
commitment to the machines this project exists for.

Why it matters: Homebrew is *unsupported* on macOS 10.14 and earlier, and Tier 3
on Intel generally. A user on 10.12 with an existing Homebrew installation is
the purest instance of the problem MacPortsBrew solves — they cannot get help
from Homebrew, and MacPorts still supports their machine. Excluding them to gain
developer convenience would hollow out the product.

**This decision constrains everything downstream.** It is not a preference the
implementation may quietly relax: a dependency that raises the deployment floor
is rejected, exactly like a GPLv2-only dependency (ADR-0001).

## Part 2 — Implementation language (decided)

**Decision: Rust.** Confirmed by the project owner on 2026-09-02. The evaluation
below is the reasoning that produced the recommendation; it is kept as written
so a future reader can check the argument rather than take the conclusion.

### Constraints, in priority order

1. Produces a binary that runs on macOS 10.12 x86_64 and 11.0 arm64.
2. No runtime dependency to install — MacPortsBrew must install where a package
   manager is broken or absent.
3. Dependencies available under Apache-2.0-compatible licences; no GPLv2-only (ADR-0001).
4. Precise subprocess control: argv-only execution, environment construction,
   timeouts, cancellation, separate stdout/stderr capture.
5. A credible TUI library with correct East Asian double-width handling.
6. Strong support for the testing style this design leans on: pure domain
   functions, golden/snapshot comparison, exhaustive enum handling.
7. Legible to coding agents, since that is how the project will be maintained.

### Evaluation

| Option | Verdict on a 10.12 target |
|---|---|
| **Rust** | **Fits exactly.** `x86_64-apple-darwin` supports macOS 10.12 and `aarch64-apple-darwin` supports 11.0 — precisely the decided targets, with no workaround. Self-contained binaries. The mainstream TUI and terminal crates are pure Rust (no system library floor) and dual MIT/Apache-2.0 licensed, so constraints 3 and 5 are satisfied together. Exhaustive `match` on enums is a direct fit for the availability, ownership, class and confidence models, which are the parts that must never silently fall through. Snapshot-testing tooling matches the `analyze --from-snapshot` determinism harness. |
| **Go** | **Excluded by the decision.** Go 1.25/1.26 require macOS 12; 1.27+ require 13. Reaching 10.12 means an unmaintained toolchain years past end of support — an unacceptable trade for a tool that runs privileged package operations. Go would have been a strong choice at a macOS 12+ floor; that floor was rejected on purpose. |
| **Zig** | Can target old deployment versions and produces small binaries, but the language is pre-1.0 with breaking changes between releases, and the TUI ecosystem is thin. The instability is a poor match for a codebase meant to be evolved by agents over years. |
| **Swift** | Deployment below macOS 10.14.4 requires shipping the Swift runtime libraries with the binary, inflating size and complicating distribution. Cross-building Intel artifacts and the TUI ecosystem are both weaker than the alternatives. |
| **C / C++** | Reaches 10.12 trivially and is otherwise the wrong trade: manual memory safety in a tool that executes privileged operations, and far slower agentic iteration. |
| **Python (bundled)** | Fails constraint 2 in substance — a bundled interpreter is a large, fragile artifact, and bundling for a decade-old macOS is its own maintenance burden. Verify the current CPython floor before recording this as final, but it is not the recommendation regardless. |

### Why

**Rust.** The 10.12 decision does most of the work: it removes Go, and among
what remains Rust is the only option that hits the target natively while also
satisfying the licence, subprocess, TUI and testing constraints.

Put plainly: **choosing macOS 10.12 was effectively choosing Rust.**

### What the foundation pass must still verify

Accepting the language settles which toolchain, not whether it behaves. Each
item below is a build-breaking property, so each needs a mechanical check rather
than an inspection.

- Build and link a binary with `MACOSX_DEPLOYMENT_TARGET=10.12` for x86_64 and
  `11.0` for arm64, and confirm the linker does not silently raise the floor.
- Confirm no transitive dependency raises the deployment target — add a check,
  do not rely on inspection.
- Confirm the chosen TUI crate's double-width handling against CJK fixtures
  (`06-cli-and-ux.md`).
- Pin the toolchain version. Rust's macOS floor was itself raised once (10.7 to
  10.12, accepted in 2022) and can be raised again; an unpinned toolchain could
  drop the target audience in a routine upgrade. Treat a floor increase as a
  breaking change requiring an ADR.
- No hosted CI runner for macOS 10.12–10.15 is known (R-10, open; treated as absent until answered), so publish those as
  **best-effort** and seek a real-hardware verification path.
  **Answered 2026-09-02 by R-10** (`../research/R-10-ci-runners.md`): correct
  for 10.12–10.15, and now a policy consequence rather than a gap in knowledge —
  GitHub maintains images only for the latest two stable macOS versions, so no
  hosted path to those releases exists or is coming. The best-effort publication
  requirement stands unchanged. The broader worry does *not* hold: hosted Intel
  runners exist as `macos-15-intel` and `macos-26-intel`, free on public
  repositories, so both architectures are tested natively on macOS 15 and 26 and
  the floor assertion above runs on a real Intel host rather than only on a
  cross-compiled artifact. Those labels are dated policy data with a stated end
  no earlier than Fall 2027.

## Consequences

- The support matrix can honestly claim the machines the product is aimed at.
- Contributor onboarding is slower than Go's would have been. Accepted.
- Deployment-target regressions become a build-breaking class of bug, which
  means they need a mechanical check rather than review discipline. CI gains a
  deployment-target assertion on the built artifacts
  ([`../spec/07-quality-and-testing.md`](../spec/07-quality-and-testing.md)).
- The dependency allowlist now has three axes, not two: licence (ADR-0001),
  deployment floor, and — new with Rust — the `unsafe` and build-script surface a
  crate brings into a binary that performs privileged operations. A crate that
  clears the first two and pulls in an unaudited `build.rs` is not automatically
  acceptable.
- The toolchain is pinned in-repository (`rust-toolchain.toml`) and a toolchain
  floor increase is a breaking change requiring an ADR, not a routine upgrade.
  `.claude/settings.json` denies the commands that would silently change the
  active toolchain.

## Revisit when

The Intel installed base the project serves has genuinely moved on; or the
toolchain raises its floor above 10.12 and staying requires pinning an
unmaintained release — at which point the honest move is to raise the product's
floor deliberately, with an ADR, not to drift.
