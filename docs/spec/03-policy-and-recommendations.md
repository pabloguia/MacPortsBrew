# 03 — Policy and recommendations

**Status:** Tier 3 unless marked.

## Shape

**MUST.** Recommendations come from an explicit rule engine, not nested
conditionals. Each rule is a named, independently testable unit:

```
Rule {
  id: "HOMEBREW_INTEL_EOL_APPROACHING"
  applies(facts) -> bool
  emits: Reason
  weight / priority
  since / source / reviewedOn
}
```

**MUST.** The engine is a pure function
`(facts, policyVersion) -> (Recommendation, [Reason])`.
No I/O, no clock reads, no environment access inside the engine. The current
date, if a rule needs it, is an input fact.

**MUST.** Policy is separate from planning, and planning is separate from
execution. A rule can never perform an action.

**MUST.** Every recommendation carries its reasons, and every reason is
renderable as a human sentence in every supported locale. A recommendation with
zero reasons is a bug.

## Outcomes

```
KEEP{provider}          rendered: KEEP_HOMEBREW | KEEP_MACPORTS
MIGRATE_TO{provider}    rendered: MIGRATE_TO_HOMEBREW | MIGRATE_TO_MACPORTS
KEEP_BOTH{primary}      rendered: KEEP_BOTH (with a primary)
USE_RUNTIME_MANAGER
MANUAL_REVIEW
NO_ACTION
```

**MUST.** The provider-named spellings on the right are the *rendered* forms,
kept because they are what a user reads. In the implementation the outcome is
parameterised by provider id — `KEEP{provider}`, `MIGRATE_TO{provider}`,
`KEEP_BOTH{primary}` — so the two directions are one value with a parameter,
not two variants that can drift apart or acquire rules the other lacks
(INV-17).

**MUST.** `MANUAL_REVIEW` is the default outcome whenever confidence,
availability or class is uncertain. Silence is never interpreted as approval.

## Facts the engine consumes

Machine: architecture; macOS version; whether the machine is running under
translation; Command Line Tools state.
Provider: presence, version, prefix, support tier for this machine, health.
Package: ownership, class, installed version, available version, artifact
availability per provider, deprecation/disabled state, variants/options,
reverse dependencies, mapping and confidence.
Context: current date, policy version and its age.

**MUST.** Nothing else. If a rule needs a new fact, the fact is added to the
input type — rules never reach out for data themselves.

## Policy data

**MUST.** All provider support knowledge lives in a versioned data file, not in
code (INV-20). Each entry carries `source`, `readOn`, and where relevant an
`effectiveFrom` / `effectiveUntil`. Example of the class of fact this holds:

> Homebrew support tier for `x86_64` macOS = tier 3 (not supported); no new
> Intel bottles; stated removal of Intel support in or after 2027-09.
> Source: https://docs.brew.sh/Support-Tiers — read 2026-09-02.

**MUST.** The build embeds a `policyVersion` and the data's newest `readOn`
date. When that date is older than a configured threshold, the CLI prints a
non-blocking staleness notice telling the user the policy data may be out of
date and how to update. Recommendations are still produced.

**MUST.** Runtime detection beats stored data wherever detection is possible.
Stored data is the fallback and the explanation, not the primary source of truth.

## Starting policy

**HYPOTHESIS — all of the below.** These encode the situation as understood on
2026-09-02 and are expected to be refined by real use. The "tier" vocabulary is
Homebrew's (F-1); MacPorts publishes no equivalent, so the MacPorts-side rules
use the two support states R-3 and R-4 established (2026-09-02): whether the
project ships an **installer** for this macOS version — MacPorts 2.12.6 offers
one for 10.12 upward — and whether a **build-farm builder** exists for this
`(macOS version, architecture)` pair — `x86_64` for 10.6–10.15, 11–15 and 26;
`arm64` for 11 upward. A builder is necessary, not sufficient: an archive may
still be absent for a given version. Per port, **deprecated** means the
Portfile names a successor in `replaced_by`; that is a separate state from
**obsolete**, which means installed but no longer present in any ports tree.

Toward MacPorts:
- Homebrew support for this machine is tier 3 / degraded, **and** the package
  has no compatible prebuilt artifact under Homebrew, **and** MacPorts offers a
  supported equivalent at `STRONG` confidence or better.
- Homebrew's stated Intel removal date is within a configurable horizon and the
  machine is Intel. Weight increases as the date approaches. The date is data.

Toward Homebrew:
- Homebrew is tier 1 for this machine, offers a prebuilt artifact, and the
  MacPorts equivalent requires a source build or is unmaintained for this macOS.
- The MacPorts port names a `replaced_by` successor, or is obsolete, or has no
  builder for this `(macOS version, architecture)` pair, and Homebrew offers a
  supported equivalent at `STRONG` confidence or better (R-3, R-4, 2026-09-02).

`KEEP_BOTH`:
- Both providers are healthy for the package and the user has asked for
  coexistence, or the package is a build-time dependency of things in both trees.

`USE_RUNTIME_MANAGER`:
- Package class is `RUNTIME`. Suggest, never act. Research which managers to
  name and keep the list as data with a review date — recommendations here age fast.

`MANUAL_REVIEW`:
- Class is `STATEFUL`, `SERVICE`, `GUI_APP` or `THIRD_PARTY_SOURCED`;
  a candidate exists under a reachable provider and its mapping confidence is
  below `STRONG` (SE-038); availability `UNKNOWN` for a destination the
  package could be moved to (SE-039); reverse-dependency
  blocker present; deprecated or disabled on the destination side.

**MUST NOT.** No rule may encode "provider X is better". Every rule is
conditioned on machine and package.

## Explainability

**MUST.** `explain <package>` prints the full derivation: the facts used, the
rules that fired, the rules that were considered and did not fire, the resulting
recommendation, and the policy version. This is the debugging interface for the
policy engine and it is also how users learn to trust it.

## Overrides

**MUST.** Every recommendation is overridable by the user, and the override is
recorded in the plan and in history with the recommendation it replaced.
Recommended actions may be preselected in interactive mode; nothing is ever
*executed* on the strength of a preselection alone.
