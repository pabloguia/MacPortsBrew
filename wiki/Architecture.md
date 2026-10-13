# Architecture

A short tour of how MacPortsBrew is put together, for anyone who wants to know
why it behaves the way it does. The authoritative version is
[`../docs/spec/README.md`](../docs/spec/README.md); this page is the map.

## One crate per layer

The dependency edges between crates *are* the layering rule. `domain` cannot
import an adapter without someone editing a `Cargo.toml`, and
`scripts/check-layering.sh` asserts the edges from `cargo metadata` on every
build.

```
cli          presentation: i18n catalogue, renderer, command surface
 |
application  capture, analysis, classification, mapping, planning, doctor
 |     \
policy  \    the rule engine and its versioned data
 |       \
domain    adapters   the only layer that touches the machine
```

| Crate | Depends on | Holds |
|---|---|---|
| `domain` | `serde` only | Provider-neutral types and pure rules. No I/O, no clock, no environment, and **no provider name in any signature** |
| `policy` | `domain` | The rule engine. `recommend(facts, data)` is a pure function |
| `application` | `domain`, `policy` | Everything above the machine and below the screen. I/O only through traits the domain declares |
| `adapters` | `domain` | Homebrew, MacPorts, the process executor, persistence, machine inspection. **Provider names appear here and nowhere above** |
| `cli` | all | Every sentence a user reads |

## Capture and analyse are separate

This is the shape that makes the rest work.

**Capture** talks to providers through the domain's read contract and produces
a `Snapshot`. **Analysis** is a pure function of a snapshot plus the policy
data.

Three things follow, and each of them is a test rather than a claim:

- `analyze --from-snapshot` runs the *same function* as a live analysis, so
  reproducibility is proven rather than asserted.
- The same snapshot always produces the same answer: every map is ordered, every
  array with a domain order is sorted, no rule reads a clock, and the current
  date is an input.
- A recorded `ProcessRunner` can substitute for the real one, so the whole
  program runs against a captured recording with `--replay` and every layer
  above is unchanged.

## The executor is the only place a subprocess is spawned

`crates/adapters/src/process/` constructs `std::process::Command`, and nothing
else in the workspace may. That is checked textually, not trusted.

Inside it, two rules:

- **Argv arrays only.** No shell, no string interpolation, ever. Package names
  and paths are arguments, never fragments of a command line. Command injection
  is structurally impossible rather than defended against.
- **A sanitised environment.** No inherited variables; the provider-quieting
  variables set. Homebrew is quieted by environment, MacPorts by argv — the
  mechanisms are genuinely different and the code does not pretend otherwise —
  and both live in one place.

## The policy engine is rules, not conditionals

Named rules, each in its own file with its own tests. A rule declares
its identifier, the policy version it arrived in, **its source** — a
specification section, an invariant or a research note — and when that
reasoning was last reviewed. A rule with no source is an opinion.

Rules are evaluated in identifier order, resolved by priority, and ties break by
identifier, so the answer never depends on declaration order. **Nothing outranks
`MANUAL_REVIEW`**, and that is enforced by outcome rather than by priority
number.

Two properties hold across every rule and are checked across all of them at
once:

- **No rule may encode "provider X is better".** Provider identifiers are values
  read out of the facts and passed through; none is written down anywhere in the
  crate.
- **A rule reads facts and data and nothing else.** It cannot act, consult a
  clock, or reach for anything outside its two arguments.

`macportsbrew explain` is the engine's own output, not a reconstruction: what
fired, what did not, and the facts that were read.

## Provider facts are data, not code

Everything MacPortsBrew believes about Homebrew or MacPorts lives in
`data/policy/*.toml` with the source it was read from and the date it was read.
Support tiers, end-of-support dates, bottle tags, elevation requirements,
curated mappings, runtime families, stateful package families.

No deadline and no provider verb is written into Rust. Correcting a provider
fact is a data edit with a citation, not a code change — which is what makes the
correction cheap enough to actually happen.

## Nothing is displayed from the lower layers

The domain, the policy engine and the adapters emit **stable identifiers** — an
`ErrorId`, a `Reason` with named parameters — and the `cli` crate turns them
into sentences in the user's locale. English is the source catalogue, compiled
into the binary so the tool can explain a failure before it has read anything
from disk.

Column layout measures **display width**, not bytes or characters, because
Japanese and Chinese text is double-width and `char`-count arithmetic produces
ragged tables for half the target locales.

## Testing

Tests run against **captured fixtures**, never a live installation. A fixture is
a recording of real read-only provider invocations with their exact output,
carrying provenance — macOS version, architecture, provider versions, capture
date, capturing command — and redacted so no home path or user name survives.

Fixtures are captured, never invented. A recording that disagrees with the
parser means the parser is wrong.

Adapter parsers assert on the **shape** of provider output and fail loudly with
a "provider output changed" error rather than silently mis-parsing. A silent
misparse of a package list is a data-loss-class bug.

## The invariants and their tests

Twenty invariants in
[`../docs/spec/01-invariants.md`](../docs/spec/01-invariants.md), each with at
least one test named after it:

```sh
grep -rn "fn invariant_" crates/*/tests crates/*/src
```

The naming is deliberate. A change that touches an invariant and cannot point
at the test defending it is incomplete.

## This build

Read-only. No mutation, no elevation, no network call of its own (Homebrew,
asked to read, may fetch its catalogue on an empty cache — see the FAQ). A gate script
greps the source on every change for a mutating provider subcommand literal or
an elevation token and fails the build if it finds one — so "this build cannot
mutate anything" is enforced rather than promised.

## See also

- [Contributing](Contributing.md) — how to change any of this.
- [`../docs/schemas/README.md`](../docs/schemas/README.md) — the JSON contract.
- [`../docs/spec/README.md`](../docs/spec/README.md) — the specification, in
  reading order.
