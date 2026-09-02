# AGENTS.md — orientation for coding agents

MacPortsBrew is evolved largely by coding agents. That is a design constraint,
not a footnote, and this file is the entry point.

**Read this first, then `docs/spec/00-charter.md` and
`docs/spec/01-invariants.md`, then `docs/spec-evolution.md`, then
`docs/spec/11-agentic-sdlc.md` once, then the work order for the pass you are
running (`prompts/`).** The evolution file records
what was already argued and why, so you do not spend a pass re-litigating
settled ground.

> **State of this file.** The repository currently contains its specification
> and no implementation, so this file orients you on the specification and
> the working protocol. `docs/spec/08-distribution-and-project.md` ("AGENTS.md
> and CLAUDE.md") lists what it must additionally explain once code exists;
> expanding it is a Foundation Pass deliverable, not an afterthought.

## What MacPortsBrew is

An open-source package-manager orchestrator for macOS. It analyses what is
installed under Homebrew and under MacPorts, maps equivalent packages between
them, explains which provider is the better choice for each package *on this
machine*, and migrates in either direction without ever removing something
before its replacement is installed and verified.

Homebrew and MacPorts are **peer providers**. Neither is globally better. The
project exists because Homebrew classifies Intel x86_64 as Tier 3 and has
stated it will remove Intel support in or after September 2027
(<https://docs.brew.sh/Support-Tiers>, read 2026-09-02; held as dated policy
data, INV-20) — but it is not an Intel escape hatch, and MacPorts-to-Homebrew
is a first-class direction with the same planner and the same executor
(INV-17).

Full identity, audience and non-goals: `docs/spec/00-charter.md`.

## The authority tiers

The specification is tiered. When two statements conflict, the higher tier wins.

| Tier | Source | May an agent change it? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | **No.** If one looks wrong, stop and ask the human. |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. |
| 3 | **MUST** in `docs/spec/02..11` | Yes, with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Yes; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

You are expected to improve tiers 3–5 when evidence or sound engineering
justifies it. You may never do so **silently**. That is the one unrecoverable
error in this repository: not being wrong, but being wrong invisibly.

## The working protocol

**Verify, do not assume.** Every factual claim about Homebrew or MacPorts is
(a) observed on this machine with a read-only command, (b) cited from official
documentation with the date you read it, or (c) expressed as runtime capability
detection. There is no fourth option, and "I recall that…" is not one of the
three. Notes go in `docs/research/`.

**Research first, build second.** Complete the blocking items in
`docs/spec/09-research-checklist.md` before the architecture is fixed. Run
independent items in parallel subagents so the main context stays clean.

**Never touch the developer's real package managers.** Reading the live system
is allowed and encouraged. Writing to it is a build failure. Tests run against
captured fixtures, never against a live install.

**Least privilege.** Nothing an agent does in this repository needs elevation.
Discovery, inventory, mapping, analysis, planning and dry-run all work
unprivileged, by design (INV-06).

**Commit incrementally.** Conventional commits, one coherent change each. A
reviewer must be able to read the history and follow the reasoning.

**Record, do not accumulate.** `docs/spec-evolution.md` and `docs/adr/` hold
the current rationale per topic (SE-032). A material change to Tiers 3–5
still lands with its entry in the same change; extend the topic's entry where
one exists, and drop what your change supersedes.

**When blocked on a decision that is the human's** — a licence, a minimum OS,
anything that trades away part of the target audience or spends their money —
write the options and your recommendation into `docs/decisions-pending.md`,
take the *reversible* default, and keep going.

## The invariants

`docs/spec/01-invariants.md` holds twenty short statements, and that file is
the only copy: this document does not paraphrase them, because a paraphrase
read more often than the original is the copy that wins. Read them before any
change; every one of them has, or will have, a test named after it.

## Where things are

| Path | What it holds |
|---|---|
| `docs/spec/` | The specification. `README.md` there is the reading order. |
| `docs/spec/11-agentic-sdlc.md` | How agents work on this repository. Read it once. |
| `docs/spec-evolution.md` | Every material specification change, with evidence. |
| `docs/decisions-pending.md` | Decisions reserved for the project owner. |
| `docs/adr/` | Architecture decision records. |
| `docs/research/` | Dated, sourced answers to the research checklist. |
| `docs/passes/` | End-of-pass reports. Each indexes; `spec-evolution.md` records. |
| `prompts/` | Work orders, one per pass. `README.md` there is the sequence. |
| `.claude/` | Claude Code configuration: permissions, hooks, subagents, skills. `README.md` there is the operator runbook. |
| `.github/workflows/` | Repository validation (runs the same gate as the `Stop` hook) and the dormant Claude Code workflows. |

## Project language

Everything internal is English: source, identifiers, comments, test names, log
messages, configuration keys, error identifiers, commit messages, ADRs,
specification, this file. Only text presented to a user is translated, and only
through the i18n layer (`docs/spec/06-cli-and-ux.md`).
