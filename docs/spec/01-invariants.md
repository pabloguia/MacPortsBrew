# 01 — Invariants

**Status:** Tier 1. These may not be changed by an agent. If one appears wrong
or unimplementable, stop and raise it with the human.

Each invariant MUST have at least one test named after it (in Rust, a test
function whose name begins `invariant_07_`, or the exact convention the
foundation pass fixes). A pull request that cannot point at
the test defending an invariant it touches is incomplete.

## Safety

**INV-01 — Destination before source.**
A package is never removed from its source provider until the destination
package is installed *and* verified per the verification contract in
`04-migration-and-safety.md`.

**INV-02 — Every destructive action comes from a reviewed plan.**
The executor executes a plan object that a human has seen. It never derives
destructive actions at execution time. Anything not in the plan does not happen.

**INV-03 — Failure preserves the prior working state.**
When any step fails, the system is left at least as usable as before, whenever
technically possible. Partial success is reported as partial success, never as
success.

**INV-04 — Reverse dependencies are checked within the same provider.**
Before removing a package, its reverse dependencies *inside that provider* are
resolved. If removal would break them, it is blocked by default and explained.

**INV-05 — Installing a package manager requires explicit consent.**
Homebrew and MacPorts are never installed, upgraded or reconfigured implicitly.

**INV-06 — Least privilege.**
The process never runs wholly as root. Discovery, inventory, mapping, analysis,
planning and dry-run MUST work with no elevation at all. Elevation is requested
per-operation, is enumerated in the plan, and is shown to the user before it happens.

## Provider isolation

**INV-07 — No intentional cross-provider dependency.**
MacPortsBrew never configures, links or records a Homebrew package as depending
on a MacPorts-managed artifact, or vice versa.

**INV-08 — One provider never satisfies another's dependency.**
The presence of `openssl3` in MacPorts does not satisfy Homebrew's `openssl@3`
requirement, in any calculation, ever.

**INV-09 — Duplication is correct.**
The same upstream software may exist under both providers with its own
dependency tree. MacPortsBrew may report this; it never tries to consolidate it.

**INV-10 — Provider invocations run in a sanitised environment.**
Every `brew` and `port` invocation is made with a controlled environment that
does not leak the other provider's prefix into `PATH`, `CPATH`,
`LIBRARY_PATH`, `PKG_CONFIG_PATH`, `LDFLAGS`, `CPPFLAGS` or equivalents.
(Rationale: build systems — notably the GNU autotools and gcc — probe
`/usr/local` for headers, libraries and binaries, which is exactly where
Homebrew lives on Intel. See F-2 in `09-research-checklist.md`.)

## Ownership and scope

**INV-11 — Primary-provider selection does not transfer ownership.**
Choosing which `git` runs does not change which provider owns `git`'s dependencies.

**INV-12 — Package-level choice is not global priority.**
Selecting a package's primary provider never reorders global resolution for
other packages. Global provider priority is a separate, explicitly confirmed operation.

**INV-13 — Normal mode is about user intent.**
Default views and destructive workflows operate on top-level / user-requested
packages. Transitive dependencies are hidden, are never preselected for
migration, and are reachable only through an explicitly entered advanced mode.

## Determinism

**INV-14 — No LLM at runtime.**
No shipped code path consults a language model. Recommendations are pure
functions of (machine snapshot, provider metadata, policy version).

**INV-15 — Reproducible recommendations.**
Given the same machine snapshot, the same provider metadata and the same policy
version, the output is byte-identical. This is enforced by
`analyze --from-snapshot`, not by assertion.

**INV-16 — Idempotence.**
Re-running a successful operation changes nothing further and reports so. No
duplicated shell blocks, no duplicated `PATH` entries, no duplicated shims.

## Product

**INV-17 — Migration is bidirectional and symmetric.**
Homebrew→MacPorts and MacPorts→Homebrew use the same domain model, the same
planner and the same executor. Neither direction is a special case in code.

**INV-18 — Coexistence is a supported end state.**
Keeping both providers is a first-class outcome, not an intermediate step
toward a migration.

**INV-19 — Stateful packages are never migrated blind.**
A package that owns persistent data outside its own prefix (databases, service
state, managed launch agents) is never auto-migrated. It is classified for
manual review and requires an explicit, documented data-and-service plan.
*(Rationale: reinstalling `postgresql` under another provider does not move its
data directory, and the two providers manage background services differently.)*

**INV-20 — Facts about providers are dated data, not code.**
Support tiers, deadlines, prefixes and known-package knowledge live in
versioned data files carrying a source and a read date, and are re-checkable.
Where behaviour can be detected at runtime, detection wins over stored knowledge.
