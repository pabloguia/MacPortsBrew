<!-- SPDX-License-Identifier: Apache-2.0 -->

# Roadmap to a first distribution — the owner's run sheet

Written 2026-10-10, at the end of pass 08 slice 2. It orders the remaining work
so that a user can **install and use** a MacPortsBrew release as early as
possible, and says, for each work order, which model the owner should start the
session with.

> **Why the model is named here and nowhere else.** SE-017 keeps work orders
> role-only: "the person starting a session decides who runs it." This file is
> that person's sheet, not a work order, so it may name models — and it is
> dated because model names date. The criterion is the one `CLAUDE.md` gives for
> subagents: **the judgement a task needs times the cost of being wrong.**
> Anything that can change a user's machine, weaken a gate or ship a supply-chain
> artifact is high on both, so it gets the stronger model. Re-read this table
> when a newer model family ships.

## Status on 2026-10-11

Rows 1–3 are **done**, and an exhaustive pre-release review followed them
(SE-094; `docs/passes/10-read-only-release-candidate-report.md` has the owner's
checklist for tagging v0.1.0). Row 3a is new: read-only improvements sized for
Sonnet, which can run while the owner works through the publication checklist.
The mutating track (rows 4–7) still needs the stronger model.

## Where the project was (2026-10-10)

- **Shippable today, if packaged:** a read-only tool — `status`, `analyze`,
  `list`, `explain`, `plan`, `doctor`, `snapshot`, and `migrate` that shows a
  plan and refuses at the execution gate. Both providers, both directions,
  fixtures for seven machines, coverage floors met.
- **Not shippable:** nothing changes a machine (`THIS_BUILD_DISPATCHES_MUTATIONS =
  false`); `history`, `restore` and `primary` do not exist; there is **no
  release machinery** at all — no build-and-package script, no signing or
  notarisation workflow, no tag, no git remote.
- Pass 08 (`08-the-mutating-executor.md`) has items 1–2 done (history store;
  lock and marker — the detector is not built, DP-030) and items 3–7 open.

## The strategy: two releases, two tracks

1. **v0.1.0 — the read-only release.** Analysis, explanation and plans a user
   can follow by hand. It is honest, useful on its own, and — more importantly —
   it forces the whole release pipeline (build, sign, notarise, package,
   checksum, publish, Gatekeeper story) to exist and be proven *before* a build
   that can change a machine travels through it. Track **R**.
2. **v0.2.0 — the first mutating release**, `--retain-source` by default, only
   after the owner rules on the flip proposal. Track **M**, then back through
   the pipeline Track R built.

The tracks touch different code, so they can alternate; run them in the order
below unless an owner prerequisite blocks one.

## The sequence

| # | Work order | Track | Model | Why that model |
|---|---|---|---|---|
| 1 ✓ | [`09-release-engineering.md`](09-release-engineering.md) | R | **Opus** | Supply chain, signing, guarded scripts and workflows, byte-identical artifacts: a mistake ships to every user and is hard to recall. |
| 2 ✓ | [`08-slice-3-config-pin-mirror.md`](08-slice-3-config-pin-mirror.md) | M | **Opus** | The spawn-site pin and the configuration re-read sit on the executor's safety boundary (INV-10, SE-050, DP-027). |
| 3 ✓ | [`10-read-only-release-candidate.md`](10-read-only-release-candidate.md) | R | **Sonnet** | Mostly docs truth, UX copy, smoke tests on fixtures and a release checklist; the reviewers (run on Opus) catch what it misses. |
| 3a | [`12-read-only-improvements.md`](12-read-only-improvements.md) | R | **Sonnet**, reviewers on **Opus** | A pt-BR catalogue, the owed dual-Homebrew `doctor` finding, a marker for defaulted ownership, cited curated mappings and two test fixes: presentation, data and additive reads, with the executor, planner and process allowlist out of bounds. |
| 3b | [`13-cross-provider-advice.md`](13-cross-provider-advice.md) | R | **Sonnet**, reviewers on **Opus** | DP-041 (b): a dated data file and a provider-neutral `doctor` rule so the MacPorts-on-`PATH` wording says which direction is riskier. Policy data, one finding, no filesystem read, executor/planner/process allowlist out of bounds. |
| — | **Owner: tag and publish v0.1.0** | R | — | Only the owner pushes, tags and holds the certificates. |
| 4 | [`08-slice-4-wiring-and-consent.md`](08-slice-4-wiring-and-consent.md) | M | **Opus** | Wires lock, marker, history and re-check into the run loop and adds elevation consent (INV-01, 02, 03, 06). The most dangerous code in the repository. |
| 5 | [`08-slice-5-history-and-restore.md`](08-slice-5-history-and-restore.md) | M | **Opus** | `restore` is a migration in reverse and is held to INV-01/03; `history` and the `doctor` additions are lighter but ride in the same slice. |
| 6 | [`08-slice-6-detector-and-flip-proposal.md`](08-slice-6-detector-and-flip-proposal.md) | M | **Opus** | Implements the owner's DP-030 ruling and writes the proposal the owner flips the switch on; judgement-heavy, irreversible in effect. |
| 7 | [`11-first-mutating-release.md`](11-first-mutating-release.md) | M→R | **Opus** for the go/no-go; **Sonnet** may draft the acceptance protocol and docs | Acceptance on a disposable machine and the release decision. |

**Reviewers.** Every work order ends with `spec-guardian`, `security-reviewer`
and `docs-auditor`. When a session runs on Sonnet, start the reviewers on Opus
(the Agent tool's `model` parameter) for tracks M and for anything under
`crates/adapters/src/process/`, the executor, the gate or release signing.

**Mechanical helpers** (fixture regeneration, a typo sweep, a `doctor` check
that only reads) can go to Sonnet or Haiku subagents from inside any session.

## What only the owner can do — start these now

They block the work orders named, and none of them can be done by an agent.

| Item | Blocks | Notes |
|---|---|---|
| Create the GitHub repository and add the remote (DP-007 decided public GitHub) | 9 (CI on a real runner), publish | `git push` is denied in sessions until then. |
| Apple Developer Program membership; **Developer ID Application** and **Developer ID Installer** certificates; an App Store Connect API key for `notarytool` | 9 (signing job), publish | Stored as repository secrets; never in a file, never in a session (prohibited for an agent to handle). |
| Decide DP-019's remaining sub-question: Intel artifacts built per commit or per release | 9 | Cost of the Intel runner. |
| Arm the guarded-path escape for the session that edits `scripts/*.sh` and `.github/workflows/` | 9 | `SE-093`: the agent stages, you arm, it writes guarded files first and runs hook tests last. |
| Rule on **DP-030** (detector) | 6 | The flip is blocked by a test until it is ruled. Recommendation (a) now, (b) at the flip. |
| Rule on **DP-028** (INV-10 wording) and **DP-029** (umask test) | 6 (DP-028 at the latest) | Neither blocks v0.1.0. |
| Remove the stray untracked `.agents/skills/tooling-recheck/` | 9's validate run | Generated layer; the path guard refuses an agent's edit. |
| A disposable macOS VM per architecture (or two spare Macs) for acceptance | 7 | INV-05 forbids tests on a live install; acceptance is a human-run protocol. |

## How to start each session

```text
Você vai executar <work order>. A ordem de trabalho completa está em
prompts/<file>.md — leia-a inteira antes de qualquer coisa, depois os
documentos da seção 1 dela, na ordem. Trabalhe de forma autônoma até a
Definition of Done e, no fim, me dê um resumo: commits, decisões, o que ficou
devido e o que precisa de mim.
```

Set `.claude/current-pass` to the work order's file name without `.md`
(for example `12-read-only-improvements`) before the session; the
session-start hook announces `prompts/<that>.md` as the work order.
