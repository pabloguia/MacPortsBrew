<!-- SPDX-License-Identifier: Apache-2.0 -->

# Roadmap to a first distribution — the owner's run sheet

Written 2026-10-10, at the end of pass 08 slice 2; **revised 2026-10-22**, after
pass 14 merged and part A of pass 15 began. It orders the remaining work so that a user can **install and
use** a MacPortsBrew release as early as possible, and says, for each work
order, which model the owner should start the session with.

> **Why the model is named here and nowhere else.** SE-017 keeps work orders
> role-only: "the person starting a session decides who runs it." This file is
> that person's sheet, not a work order, so it may name models — and it is
> dated because model names date. The criterion is the one `CLAUDE.md` gives for
> subagents: **the judgement a task needs times the cost of being wrong.**
> Anything that can change a user's machine, weaken a gate or ship a supply-chain
> artifact is high on both. Re-read this table when a newer model family ships.

## Status on 2026-10-27

**Done:** passes 00–07, 09 (release machinery), 10 (release candidate), 12, 13,
14 and 15 (merged as PR #13 on 2026-10-25), pass 08 slices 1–5 and slice 6
part A (PR #22). The read-only build is complete and reviewed. The mutating
loop is wired behind `THIS_BUILD_DISPATCHES_MUTATIONS = false`; a run takes its
snapshot and records itself; `history`, `restore --plan` and the stranded-run
`doctor` finding exist. Nothing has been dispatched on a real machine.

**On 2026-10-27 (SE-109)** all thirteen open decisions (DP-028 to DP-051) were
ruled under the owner's delegation and confirmed by the owner in chat the same
day. Their rulings are in `docs/decisions-pending.md` (Resolved).

**Not done:**

- **v0.1.0 is not tagged.** What remains is the owner's: the certificates, the
  repository settings, the dry run and the tag (see the owner table below).
- **Mutations stay off.** The rest of the mutating product is work order 16,
  [`16-complete-mutating-product.md`](16-complete-mutating-product.md). Its
  ledger, `docs/passes/16-progress.md`, is the live status; this sheet does not
  repeat it.

## The strategy: two releases, two tracks

1. **v0.1.0 — the read-only release** (track **R**): it forces the whole
   release pipeline to exist and be proven before a build that can change a
   machine travels through it.
2. **v0.2.0 — the first mutating release** (track **M**), only after the owner
   rules on the flip proposal (work order 16 part L). Since the rulings of
   2026-10-27 it is **symmetric**:
   - both directions, Homebrew→MacPorts and MacPorts→Homebrew (INV-17);
   - elevation enumerated in the plan and consented to per step (DP-046 (b));
   - a consented shim plus one managed `PATH` block when the destination does
     not win `PATH` (DP-047, (b) first, then (a));
   - the source **retained by default**; removal opt-in with
     `--remove-source`, a stronger confirmation, and only after the
     destination re-read, the lock probe and all six destination checks
     (DP-048 (c), DP-030 (b), DP-051 (a));
   - the executed plan tied to the plan shown, by `--plan-digest` for scripts
     and an interactive confirmation for a person (DP-049).

   The non-elevating path (MacPorts→Homebrew formulae, source retained) is
   exercised first on the disposable machines, then the rest (DP-046's
   milestone).

## The sequence from here

| # | Work order | Track | Can start | Model | Why that model |
|---|---|---|---|---|---|
| — | **Owner: certificates, environment, ruleset, dry run, tag v0.1.0** | R | now | — | Only the owner handles credentials and approves signing; a session may push the tag (SE-108). |
| 1 | [`16-complete-mutating-product.md`](16-complete-mutating-product.md) | M | now; one session at a time | **Opus** for parts S, E, F, G, H, K, I, J and L; **Sonnet** may run A, B, C, D, P and M, with the reviewers on the stronger model | Parts S onward change what a run may do: the planner, the gate, the executor, verification, elevation and writes outside the state directory. Parts A–M are docs, staging, read-only disclosure and additive tests. |
| 2 | [`11-first-mutating-release.md`](11-first-mutating-release.md) | M→R | after the owner's flip ruling | **Opus** for the go/no-go; **Sonnet** may draft the acceptance protocol and docs | Acceptance on a disposable machine per architecture, both directions, the non-elevating milestone first. |

Work order 16 folds in pass 08 slice 6 parts B and C and slices 7–9; those
files carry a banner saying so, and its §1 item 8 says which of their
instructions it overrides (plan mode, merging, pushing, dependencies, stops).

### Rules for a mutating-track part

Work order 16 §4 is the binding form; these are why the rules exist.

1. **A reviewed plan before code**, in `docs/passes/16-plans/<part>.md`.
   `spec-guardian` and `security-reviewer` review it, on the stronger model,
   before any code and after every commit. Their review is the approval; there
   is no interactive plan mode.
2. **The mutation proof is not optional**: a subagent in an isolated worktree
   applies the mutants the plan lists, and the session reads every survivor.
3. **A refusal is an owner item**, never a puzzle: a hook, a permission prompt
   or a classifier refusal goes into the ledger's "Needs the owner".
4. **Each part merges on green required checks** (SE-108); the switch stays
   off on `main` until the owner rules on part L.

**Mechanical helpers** (fixture regeneration, a typo sweep, a read-only
`doctor` check) can go to Sonnet or Haiku subagents from inside any session.

## What only the owner can do

| Item | Blocks | Notes |
|---|---|---|
| Apple Developer Program; **Developer ID Application** and **Installer** certificates; an App Store Connect API key; the `release-signing` environment with its 7 secrets and 3 variables; the `v*` tag ruleset; immutable releases | the v0.1.0 tag | Checklist in `docs/passes/09-staged/README.md`; never in a file, never in a session. |
| The dispatch dry run, the Standard-account `.pkg` check, the Gatekeeper protocol | the v0.1.0 tag | `docs/passes/10-read-only-release-candidate-report.md`. DP-036 is ruled: the `.app` of DP-019 is the `.dmg`. |
| Launch each work order 16 session with its fixed prompt, interactive, in auto mode | every part of 16 | A refusal that blocks a push, a pull request or a merge is an owner item. |
| Apply work order 16's staged packages (`docs/passes/16-staged/`) with the armed message each README gives | parts E, F, H, J and M | The allowlist, `check-licences.sh`, `stop-gate.sh` and the Claude workflows are guarded or frozen. |
| Captures no machine here can produce: a MacPorts port with a startup item, an arm64 machine with both providers, a machine with a shimmed `PATH` | parts D, F, I (tests stay on what exists meanwhile) | Fixtures are captured, never invented. |
| Rule on the flip proposal (part L); run work order 11's acceptance on disposable machines; tag v0.2.0 | v0.2.0 | Acceptance is a human-run protocol. |
| Decide DP-019's remaining sub-question: Intel artifacts built per commit or per release | the release workflow | Default: per release. |

## How to start each session

Work order 16 has its own fixed prompt, in its header; use that, the same text
every time. For any other work order:

```text
Você vai executar <work order>. A ordem de trabalho completa está em
prompts/<file>.md — leia-a inteira antes de qualquer coisa, depois os
documentos da seção de leitura dela, na ordem. Trabalhe de forma autônoma até a
Definition of Done e, no fim, me dê um resumo: commits, decisões, o que ficou
devido e o que precisa de mim.
```

Set `.claude/current-pass` to the work order's file name without `.md`
(for example `16-complete-mutating-product`) before the session; the
session-start hook announces `prompts/<that>.md` as the work order.

## History

The release-machinery follow-ups 2a–2j of pass 15 part B, the DP-034 research
and the `history` help line are done and merged (PRs #13 and #16);
`docs/passes/15-first-tag-readiness-report.md` and `docs/spec-evolution.md`
(SE-106) hold their detail. The optional `.claude/` fixes from
`docs/research/tooling/2026-10-22-*.md` and later re-checks remain the owner's,
as one armed message, if wanted.
