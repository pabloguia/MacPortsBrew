# AGENTS.md — orientation for coding agents

MacPortsBrew is evolved largely by coding agents. That is a design constraint,
not a footnote, and this file is the entry point.

**Read `docs/spec/01-invariants.md` and `docs/spec-evolution.md` before any
significant work.** The invariants are not changeable by you; the evolution file
records what was already argued, so you do not spend a pass re-litigating
settled ground. Then `docs/spec/00-charter.md`, `docs/spec/11-agentic-sdlc.md`
(once), and the work order for your pass in `prompts/`.

## What MacPortsBrew is

An open-source package-manager orchestrator for macOS. It analyses what is
installed under Homebrew and MacPorts, maps equivalent packages, explains which
provider is the better choice for each package *on this machine*, and migrates
in either direction without ever removing something before its replacement is
installed and verified. Homebrew and MacPorts are **peer providers**: neither is
globally better, and MacPorts-to-Homebrew is a first-class direction with the
same planner and the same executor (INV-17). Homebrew's Intel timeline is why
the project started and is not what it is; it is dated data (INV-20), never
architecture. Full identity and non-goals: `docs/spec/00-charter.md`.

## The authority tiers

When two statements conflict, the higher tier wins.

| Tier | Source | May an agent change it? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | **No.** If one looks wrong, stop and ask the human. |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. |
| 3 | **MUST** in `docs/spec/02..11` | Yes, with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Yes; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

You may improve tiers 3–5, never **silently**. That is the one unrecoverable
error here: not being wrong, but being wrong invisibly.

## The working protocol

**Verify, do not assume.** Every factual claim about a provider is (a) observed
on this machine with a read-only command, (b) cited from official documentation
with the date read, or (c) runtime capability detection. There is no fourth
option. Notes go in `docs/research/`; dated facts go in `data/policy/*.toml`
with `source` and `read_on`.

**Never touch the developer's real package managers.** Reading is encouraged;
writing is a build failure. Tests use captured fixtures (INV-05). Nothing here
needs elevation (INV-06). Commits are Conventional, one coherent change each.

**Record, do not accumulate.** `docs/spec-evolution.md` and `docs/adr/` hold the
current rationale *per topic* (SE-032): merge entries on one topic, drop
superseded content, keep identifiers stable, and keep a rejection's entry — a
rejection is the current rationale for something's absence. A material Tier 3–5
change lands with its entry in the same change.

**When blocked on a decision that is the human's** — a licence, a minimum OS,
anything that spends their money or trades away part of the audience — write the
options and your recommendation into `docs/decisions-pending.md`, take the
*reversible* default, and keep going.

## Layering, and where things live

One crate per layer under `crates/`. The Cargo dependency edges **are** the
layering rule; `scripts/check-layering.sh` asserts them from `cargo metadata`.

| Crate | Depends on | Holds |
|---|---|---|
| `domain` | `serde` only | Provider-neutral types and pure rules. No I/O, no clock, no environment, no provider name in any signature |
| `policy` | `domain` | The rule engine and its versioned data. `recommend(facts, data)` is pure |
| `application` | `domain`, `policy` | Capture, analysis, classification, mapping, planning, `doctor`. I/O only through domain traits |
| `adapters` | `domain` | The only layer that touches the machine. Provider names appear here and nowhere above |
| `cli` | all | Presentation: i18n catalogue, renderer, command surface |

| Path | What it holds |
|---|---|
| `docs/spec/` | The specification. Its `README.md` is the reading order |
| `docs/spec-evolution.md` · `docs/adr/` | Current rationale per topic |
| `docs/decisions-pending.md` · `docs/research/` | The owner's decisions; dated provider facts |
| `docs/schemas/` · `docs/passes/` · `prompts/` | `--json` contracts; pass reports; work orders |
| `data/policy/` | Provider facts as data, every entry with `source` and `read_on` |
| `locales/` · `fixtures/recordings/` | The English catalogue; captured provider recordings |
| `scripts/` · `.claude/` | `validate.sh` is the one "valid"; permissions, hooks, subagents, skills, and `rules/*.md`, which load the relevant specification chapter when you touch a subtree |

## Provider neutrality and the isolation boundary

**Nothing above `crates/adapters/` names a provider.** `registry.rs` is the only
place that knows which providers exist, and it reads where they install from
`data/policy/provider-prefixes.toml` rather than holding it (DP-014); everything
else works with `dyn ProviderRead`. Adding a provider is an adapter plus one
registry entry.

**Capture and analysis are separate.** Capture produces a `Snapshot` through the
domain's read contract; analysis is a pure function of a snapshot plus the
policy data. So `--from-snapshot` runs the same function as a live run, and a
recorded `ProcessRunner` (`--replay`) substitutes for the real one with every
layer above unchanged. That is how INV-15 is proven, not asserted.

## Domain concepts

`Ownership` (`TOP_LEVEL`, `DEPENDENCY`, `ORPHAN`, `UNKNOWN`) · `ClassSet` (a
package carries a *set*: `SIMPLE`, `STATEFUL`, `SERVICE`, `RUNTIME`, `GUI_APP`,
`THIRD_PARTY_SOURCED`, `PROVIDER_INTERNAL`; the strictest member decides) ·
`ArtifactAvailability` (`UNKNOWN` is a first-class answer, never a silent "no")
· `PackageMapping` with `Confidence` · `Resolution` (which executable actually
runs) · `SupportState` (always carrying a `DatedFact`) · `MigrationPlan`.

**Ownership model.** Use the provider's *own* record of intent, never a graph
inference: Homebrew's `installed_on_request`, MacPorts' registry `requested`
flag. `brew leaves` is a different set and is never substituted. The cask JSON
carries no intent field; Homebrew records it in the cask's on-disk receipt, and
the adapter reads that (R-12, DP-012) — no receipt means `UNKNOWN`, never an
inferred `TOP_LEVEL`. Ownership is a fact about a provider, not about the
machine, and an `UNKNOWN` row appears in the default view rather than being
hidden behind `--advanced`.

**Mapping model.** Never by name alone. `1→1`, `1→many`, `many→1` and *no
equivalent* are all first-class. Every candidate records evidence — a mapping
without it is a bug. Two sources: discovered from metadata, and a small curated
table for what heuristics miss. `AMBIGUOUS` and `NONE` never act automatically.

## Adapters: what each must never do

An adapter implements `ProviderRead` over a `process::ProcessRunner`. It must
**never** construct `std::process::Command` itself (the executor is the only
site, checked textually), never build a command by string interpolation, never
report `CapabilityState::Unknown` as `Available`, never read a clock or the
environment outside the executor's sanitised one, and never let provider
metadata become a path or a flag. A parser that meets unrecognised output fails
with `ErrorId::ProviderOutputChanged` rather than mis-parsing. Homebrew is
quieted by environment, MacPorts by argv; that asymmetry lives in one place.

## The policy engine

Named rules in `crates/policy/src/rules/`, one file each, evaluated in
identifier order, resolved by priority with ties broken by identifier (INV-15).
Every rule declares `id`, `since`, `source`, `reviewed_on`, `priority`,
`evaluate`. **No rule names a provider**, and no rule reads anything but its two
arguments. **Nothing outranks `MANUAL_REVIEW`**, enforced by outcome, not by
number (INV-19).

## The migration state machine

As implemented in `StepKind` (`crates/domain/src/plan.rs`):

```
SNAPSHOT_BEFORE -> ADD_DESTINATION -> VERIFY_DESTINATION -> SET_PRIMARY
-> VERIFY_RESOLUTION -> [REMOVE_SOURCE -> VERIFY_REMOVAL] -> [CLEANUP_REVIEW]
-> RECORD_HISTORY        (MANUAL_REVIEW may replace any step and halts)
```

`MigrationPlan::check_ordering` enforces that no `REMOVE_SOURCE` precedes the
`VERIFY_DESTINATION` guarding it (INV-01). The plan is the contract: the
executor runs plan steps and nothing else (INV-02).

## Primary provider (SE-036)

Four layers, each used only where it is the least invasive that expresses the
request: **report only** (the default, and all this build does); **recommend the
versioned executable name**; **delegate an intra-MacPorts version choice to the
provider's own selection mechanism**, proposing the command rather than running
it, because that write elevates and primary selection must stay off the
elevating path; and a **MacPortsBrew-owned shim directory** with exactly one
`PATH` prepend inside a managed block. Selecting a primary never transfers
ownership (INV-11) and is not a global priority (INV-12).

## Tests

`scripts/validate.sh` (`--quick` | default | `--full`) is the one definition of
valid; CI is the authority. Tests run against captured fixtures, never a live
installation. Coverage floors: `domain`/`policy`/planning 95%, `adapters` 80%
plus a contract test per parsed command, `cli` reported. Invariant tests are
named `invariant_NN_*` in `crates/*/tests/` (and, for the engine's own, in
`crates/policy/src/engine.rs`); find them with
`grep -rn "fn invariant_" crates/*/tests crates/*/src`. **A change that touches
an invariant and cannot point at its test is incomplete.**

## The six recipes

Each is a section of `CONTRIBUTING.md` **and** a skill under `.claude/skills/`,
written from the same source, so a human and an agent follow identical steps.
Invoke the skill: `add-mapping`, `add-policy-rule`, `add-locale`,
`add-doctor-check`, `capture-fixture`, `add-provider`.

## Dangerous to change

Touch these only deliberately, and run `security-reviewer` and `spec-guardian`
before reporting done. **`crates/adapters/src/process/`** — the only subprocess
site; argv only, no shell, sanitised environment, so command injection is
prevented structurally. **`crates/application/src/planner.rs`** — step ordering
*is* INV-01; reordering is a safety change, not a refactor.
**`crates/policy/src/rules/mod.rs`** — the priority constants and `all()` are a
total order the engine depends on. **`scripts/*.sh` and `.claude/hooks/*.sh`** —
the gates; weakening one is invisible until something it would have caught
ships. **`crates/domain/src/error.rs`** — exit codes are a documented contract.

## Project language

Everything internal is English: source, identifiers, comments, test names, log
messages, configuration keys, error identifiers, commit messages, ADRs,
specification, this file. Only text presented to a user is translated, and only
through the i18n layer.
