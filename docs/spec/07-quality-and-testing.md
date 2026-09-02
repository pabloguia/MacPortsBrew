# 07 — Quality, testing and CI

**Status:** Tier 3 unless marked.

## Architecture for agents

MacPortsBrew will be evolved largely by coding agents. That is a design
constraint, not a footnote. This section draws the consequence for the
architecture; [`11-agentic-sdlc.md`](11-agentic-sdlc.md) draws it for the
process, and specifies what must be mechanically enforced rather than asked
for.

**MUST.** Small cohesive modules; explicit interfaces at every layer boundary;
low coupling; no global mutable state; no `utils` grab-bag; no monolithic CLI
file; no shell invocation outside the central process executor; documented
invariants next to the code that upholds them.

**Suggested layering** (HYPOTHESIS — adapt to the language's idioms, keep the
boundaries):

```
domain/      pure types and rules. No I/O. No provider names in signatures.
policy/      rule engine + versioned policy data.
application/ discovery, inventory, mapping, assessment, planning, verification, doctor.
adapters/    homebrew, macports, process, filesystem, shell, system, persistence.
ui/          cli, tui, rendering, i18n.
```

**MUST.** `domain` and `policy` have zero dependencies on `adapters`.
This is enforced by an automated check in CI, not by convention.

## Testing

**MUST.** Tests never touch the developer's real Homebrew or MacPorts
installation. Any test that could is a build failure.

**MUST.** Layered strategy:

- **Invariant tests** — one per invariant in `01-invariants.md`, named after it.
  These are the real gate. A change that breaks one is rejected regardless of
  coverage numbers.
- **Domain and policy unit tests** — pure, fast, exhaustive on branches.
- **Adapter contract tests** — run against **recorded real provider output**.
- **Golden/reproducibility tests** — `analyze --from-snapshot` against committed
  snapshots produces byte-identical output (INV-15).
- **End-to-end tests** — the CLI over fixture machines, asserting on `--json`.

## Fixtures

**MUST.** Fixtures are *captured*, not invented. A documented capture script
records real `brew` and `port` output plus machine facts into a snapshot.

**MUST.** Every fixture carries provenance: macOS version, architecture,
provider versions, capture date, capturing command. A fixture without
provenance cannot be trusted when a provider's output format changes.

**MUST.** Adapter parsers assert on the *shape* of provider output and fail
loudly with a "provider output changed" error rather than silently mis-parsing.
Silent misparse of a package list is a data-loss-class bug.

**MUST.** The fixture matrix covers at least: Homebrew only; MacPorts only;
both; neither; Intel; Apple Silicon; an older macOS; a current macOS; prebuilt
artifact available; source build required; exact mapping; ambiguous mapping;
no mapping; casks present; a third-party tap; a stateful package; a service
package; a reverse-dependency blocker; a deprecated formula; a broken/partial
provider install.

## Coverage

**MUST NOT.** No single blanket percentage across the repository — it produces
test padding in adapters and UI while the parts that matter stay thin.

**MUST.** Differentiated floors, enforced in CI:

| Layer | Floor | Rationale |
|---|---|---|
| `domain`, `policy` | 95% line **and** branch | Pure, cheap to test, where correctness lives |
| planning, verification, safety logic | 95% | This is the code that can destroy a machine |
| `adapters` | 80%, plus a contract test per parsed command | Value is in real fixtures, not line count |
| `ui` | reported, no floor | Cover behaviour, not rendering |

**SHOULD.** Spot-check the domain and policy layers with mutation testing when
the ecosystem offers a usable tool. A surviving mutant in a safety path is a
finding worth a ticket.

## CI

**MUST.** GitHub Actions validating: formatting, linting, static analysis,
layering rules, i18n key parity, unit and contract tests, coverage floors,
schema-version consistency, documentation link check, and a build for **both**
`x86_64` and `arm64` macOS.

**MUST.** A **deployment-target check** in CI, asserted on the built artifacts
rather than on the build configuration. It reads the minimum OS version recorded
in each binary and fails when it is above 10.12 for `x86_64` or 11.0 for
`arm64`. ADR-0002 makes a floor regression a build-breaking class of bug; a
configuration that *says* 10.12 while the linker recorded something higher is
exactly the failure this catches, and it is invisible to review.

**MUST.** The dependency gate has three axes, not two: licence (below),
deployment floor (above), and the `unsafe` and build-script surface a crate
brings into a binary that performs privileged operations. A crate that clears
licence and floor is not automatically acceptable; adding one with a
non-trivial `build.rs` or a large `unsafe` surface needs an ADR.

**MUST.** A dependency-licence check in CI. It fails the build on any
dependency whose licence is outside the allowlist in
[`08-distribution-and-project.md`](08-distribution-and-project.md) — GPLv2-only
in particular, which is incompatible with the project's Apache-2.0 licence. It
also fails on any source file missing its `SPDX-License-Identifier` header.
Licence compatibility is a build-breaking property, not a release-day surprise.

**MUST.** Hosted Intel macOS runner availability is uncertain and must be
verified, not assumed. If unavailable, cross-compile and validate behaviour
against Intel fixtures — and say so honestly in the docs rather than claiming
tested support. Publish a support matrix distinguishing **tested**,
**best-effort** and **unsupported** OS/architecture combinations, and keep it
generated from what CI actually runs.

**SHOULD.** A scheduled job that re-runs adapter contract tests against the
latest provider releases, so provider output drift is discovered by CI rather
than by a user.

## Validation is mechanical, not remembered

**MUST.** Every check in the definition of done below is runnable by a command,
and the command is what decides. A validation that exists only as a line in a
checklist is a validation that will be reported as passing without having run.

**MUST.** The same checks are reachable three ways, from the same scripts: in
CI, where they are authoritative; locally, as a single command a contributor can
run before pushing; and as an agent's end-of-turn gate
([`11-agentic-sdlc.md`](11-agentic-sdlc.md)), so an agent is *prevented* from
asserting a passing build rather than merely asked not to. Three entry points,
one implementation — a gate that drifts from CI is worse than no gate, because
it is trusted.

## Definition of done

A change is done when: it is implemented; tests exist and pass; full validation
passes; the invariants it touches still have passing named tests; i18n keys are
consistent; documentation and wiki are accurate for any user-visible change;
no cross-provider contamination was introduced; and `docs/spec-evolution.md`
records any material specification change.

For a change made by an agent, [`11-agentic-sdlc.md`](11-agentic-sdlc.md) adds
the requirements that the gate passed, the reviewers ran, and every new provider
claim carries its source and date.
