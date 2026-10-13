# MacPortsBrew specification

Read in this order. Each file stands alone — load only what the work needs.

| File | What it settles |
|---|---|
| [00-charter.md](00-charter.md) | Identity, why the project exists, who for, non-goals, legal posture |
| [01-invariants.md](01-invariants.md) | The rules that may never be broken. **Read before any change.** |
| [02-domain-model.md](02-domain-model.md) | Provider contract, ownership, package classes, artifact availability, mapping, snapshots |
| [03-policy-and-recommendations.md](03-policy-and-recommendations.md) | The rule engine, its inputs, its outputs, its data |
| [04-migration-and-safety.md](04-migration-and-safety.md) | State machine, plans, the verification contract, failure semantics, privilege, process execution |
| [05-environment-and-primary-provider.md](05-environment-and-primary-provider.md) | Executable ownership, primary provider, shell configuration |
| [06-cli-and-ux.md](06-cli-and-ux.md) | Command surface, JSON and exit-code contract, caching, i18n, diagnostics |
| [07-quality-and-testing.md](07-quality-and-testing.md) | Layering, tests, fixtures, coverage policy, CI, definition of done |
| [08-distribution-and-project.md](08-distribution-and-project.md) | Releases, Gatekeeper, licence, docs strategy, wiki, contribution |
| [09-research-checklist.md](09-research-checklist.md) | What must be verified before deciding — with dated pre-seeded findings |
| [10-first-pass-work-order.md](10-first-pass-work-order.md) | What the foundation pass delivers and how it is accepted |
| [11-agentic-sdlc.md](11-agentic-sdlc.md) | How coding agents work on this repository: memory files, enforced boundaries, hooks, reviewers, delegation, passes, gates |

Related:

- [`../spec-evolution.md`](../spec-evolution.md) — every material change to this specification, with rationale. Read before re-litigating a decision.
- [`../decisions-pending.md`](../decisions-pending.md) — decisions reserved for the project owner.
- [`../research/`](../research/) — dated, sourced answers to the research checklist.
- [`../passes/`](../passes/) — end-of-pass reports.
- [`../../prompts/`](../../prompts/) — the work order for each pass.
- [`../../AGENTS.md`](../../AGENTS.md) — agent orientation; [`../../CLAUDE.md`](../../CLAUDE.md) is its thin Claude Code layer; [`../../.claude/README.md`](../../.claude/README.md) is the operator runbook for the agent configuration.
- [`../adr/`](../adr/) — architecture decision records:
  - [0001 — Licence: Apache-2.0](../adr/0001-licence-apache-2-0.md)
  - [0002 — Minimum macOS target and implementation language](../adr/0002-implementation-language.md)
  - [0003 — Product name and CLI alias](../adr/0003-name-and-cli-alias.md)
  - [0004 — Dependency gate](../adr/0004-dependency-gate.md)
  - [0005 — Release signing and distribution artifacts](../adr/0005-release-signing-and-artifacts.md)
  - [0006 — No `libc`, and the timeout kills one process rather than a group](../adr/0006-no-libc-and-the-timeout-kills-one-process.md)

## Authority

| Tier | Source | Changeable by an agent? |
|---|---|---|
| 1 | `01-invariants.md` | No. Stop and ask. |
| 2 | `00-charter.md` | Only with an explicit human decision. |
| 3 | **MUST** statements in `02..11` | Yes, with a `spec-evolution.md` entry. |
| 4 | **SHOULD** statements | Yes; log the material ones. |
| 5 | **HYPOTHESIS** statements | Expected to change. Log the outcome. |

Improving this specification is expected. Deviating from it silently is not.
