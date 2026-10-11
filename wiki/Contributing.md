# Contributing

The full guide is [`../CONTRIBUTING.md`](../CONTRIBUTING.md) — prerequisites,
build, test, validation, the architecture overview, licensing, and six
step-by-step recipes. This page is the short version and the orientation.

## Two things to know first

**Homebrew and MacPorts are peers.** Neither is globally better, and no
contribution may encode that one is. The two migration directions share one
planner and one policy engine, and a test checks that exchanging the provider
identifiers in the input exchanges them in the output and changes nothing else.

**Every claim about provider behaviour carries its source and the date you read
it**, or is phrased conditionally, or is runtime capability detection. There is
no fourth option, and "I remember that…" is not one of the three. Dated facts
go in `data/policy/*.toml` with `source` and `read_on`, never into control flow.

## Getting set up

You need `rustup` (the toolchain is pinned and installs itself), plus `jq`,
`python3` and `shellcheck`.

```sh
cargo build
cargo test --workspace
scripts/validate.sh
```

`scripts/validate.sh` is the one implementation of "valid" — format, clippy,
layering, SPDX headers, dependency licences, i18n parity, and the test suite.
CI is configured to run it and will be the authority; the same script is what
you run locally. No CI job has run yet — the repository has no remote — so for
now your local run is the authority, which is a reason to run the full one
rather than `--quick`.

**You do not need Homebrew or MacPorts installed to contribute.** Tests run
against captured fixtures and never touch a live installation. That is a rule,
not a convenience: a test that mutates a developer's real package manager is a
build failure.

## The six recipes

Each exists twice, written from the same source: as a section of
[`../CONTRIBUTING.md`](../CONTRIBUTING.md) for a person, and as a skill under
`.claude/skills/` for a coding agent. They are the same steps, so a human and an
agent do the same thing.

| Recipe | When |
|---|---|
| **Adding a package mapping** | The heuristics miss an equivalence, and you can cite the destination name from the destination provider's own index |
| **Adding a policy rule** | A recommendation should change for a reason the existing rules do not express |
| **Adding a locale** | Bringing user-facing text to `pt-BR`, `es`, `ja` or `zh-CN` |
| **Adding a doctor check** | A machine condition should be reported before it causes a failure |
| **Capturing a fixture** | Before writing any adapter test, or when a provider's output may have changed |
| **Adding a provider** | Integrating a third package manager. Out of scope for the initial releases, but the abstraction must hold |

## Good first contributions

- **A mapping with a citation.** The smallest useful change, and the one with
  the clearest acceptance criterion: the destination name read from the
  destination provider's index, on a date you record.
- **A fixture from a machine nobody has.** The fixture matrix wants Apple
  Silicon, older macOS versions, casks, third-party taps, a broken provider
  install. If you have one of those, a recording from it is worth more than most
  code.
- **A documentation correction with a source.** Provider facts go out of date by
  construction. A correction that names what the source says now, and when you
  read it, can be applied immediately.
- **A translation.** The i18n architecture and the complete English catalogue
  are in place; the translations are not.

## What makes a change complete

- It is implemented, and tests exist and pass.
- `scripts/validate.sh` passes.
- Every invariant it touches still has a passing test named after it —
  `grep -rn "fn invariant_" crates/*/tests crates/*/src` finds them. A change
  that touches an invariant and cannot point at its test is incomplete.
- i18n keys are consistent, in both directions: a missing key and an orphaned
  key both fail.
- Documentation and these wiki pages are accurate for any user-visible change.
- A material specification change is recorded in `docs/spec-evolution.md` **in
  the same commit**, not reconstructed afterwards.

Commits are Conventional Commits, one coherent change each, in English.

## Licensing

Apache-2.0. Under §5, contributions are licensed to the project under the same
terms by default: **inbound equals outbound, and no contributor licence
agreement is required.** You do not sign anything.

Every source file carries `SPDX-License-Identifier: Apache-2.0` as its first
comment line. There is deliberately no per-file copyright line and no `NOTICE`
file.

Dependency licences are enforced mechanically. GPLv2-only is forbidden — it is
incompatible with Apache-2.0 — and a dependency that raises the macOS
deployment floor is rejected the same way.

## Reporting rather than fixing

Perfectly good contributions. The templates in
[`../.github/ISSUE_TEMPLATE`](../.github/ISSUE_TEMPLATE) cover bugs, incorrect
recommendations, mapping problems, provider compatibility, features and
documentation. Each asks for the machine facts that make a report actionable —
`macportsbrew doctor --report` collects them, without home paths, user names
or environment values; read it before you attach it.

For a security problem, do not open an issue: see
[`../SECURITY.md`](../SECURITY.md).

## For coding agents

[`../AGENTS.md`](../AGENTS.md) is the orientation document, and it is
tool-neutral. `CLAUDE.md` adds only the Claude Code specifics.

The short version: read the invariants and `docs/spec-evolution.md` before
significant work; you may improve the specification below Tier 2 but never
silently; and the one unrecoverable error here is not being wrong, it is being
wrong invisibly.

## See also

- [Architecture](Architecture.md) — how the pieces fit.
- [`../docs/spec/README.md`](../docs/spec/README.md) — the specification.
- [`../docs/schemas/README.md`](../docs/schemas/README.md) — the JSON contract
  and its compatibility rule.
