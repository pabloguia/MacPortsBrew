# MacPortsBrew — Repository Readiness Pass (00-REPO-READINESS-PASS)

> Ran 2026-09-02; report in `../docs/passes/00-REPO-READINESS-PASS-report.md`.
> Paths below describe the tree as it was when the pass started (for example
> `docs/agentic-sdlc/`, which the pass removed, and `00-FOUNDATION-PASS`,
> which it renumbered to `01`).

The repository currently holds only specification and agent configuration —
no implementation code. This pass runs once, before the Foundation Pass, to
deeply review and evolve that specification, the agentic-SDLC tooling
(subagents, skills, hooks, settings) and the repository's own directory
structure, and to leave the repository ready for the Foundation Pass to
begin. It produces no application code.

---

## 1. Read this first

In this order, before doing anything else:

1. `AGENTS.md` — orientation and the working protocol.
2. `docs/spec/00-charter.md` — what MacPortsBrew is, who it is for, what it is not.
3. `docs/spec/01-invariants.md` — Tier 1. Non-negotiable.
4. `docs/spec-evolution.md` — what has already been argued, so you do not
   reopen it.
5. `docs/spec/11-agentic-sdlc.md` — how work is shaped here today.
6. `prompts/README.md` and `prompts/TEMPLATE-PASS.md` — the pass convention
   this file already follows.
7. `.claude/README.md`, `.claude/settings.json`, every file in
   `.claude/agents/` and `.claude/skills/`.

Read the rest of `docs/spec/*.md`, `docs/adr/*`, `docs/decisions-pending.md`
and `docs/agentic-sdlc/*` on demand while you work. Each file stands alone;
do not load the whole specification up front.

## 2. Authority and precedence

| Tier | Source | Changeable? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | No. If one seems wrong, **stop and ask the human.** |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. |
| 3 | **MUST** in `docs/spec/02..11` | Only with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Change freely; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

You are expected to improve tiers 3–5 where evidence or sound engineering
justifies it. Never silently. Use `/spec-evolution` when you do.

You additionally have the project owner's standing, explicit permission for
this pass to edit, restructure, consolidate or replace **any** document,
agent, skill, hook, configuration file or directory layout in this
repository — including `docs/spec/*`, `AGENTS.md`, `CLAUDE.md`,
`docs/adr/*`, `docs/spec-evolution.md`, `docs/decisions-pending.md`,
`prompts/*`, `.claude/agents/*`, `.claude/skills/*`, `.claude/hooks/*`,
`.claude/settings.json`, and the `docs/` tree itself. Do not stop to have a
plan approved before acting — decide, record the decision, and proceed. Stop
only for the cases in §4's last bullet and §6.

## 3. Hard boundary for this pass

**This pass ships no implementation code and resolves no reserved decision.**

At the end of this pass: no `src/` (or equivalent implementation-language
tree) exists — this repository still contains specification, documentation
and agent tooling only. No item in the *Open* section of
`docs/decisions-pending.md` has been resolved without the human. No `git
init` and no `git push` has run — version control starts only once the
Foundation Pass has produced the first version of code, per
`docs/spec/10-first-pass-work-order.md`'s Step 0; this pass works directly
on the filesystem, uncommitted. No mutating `brew` or `port` command has run
against this machine, and no elevation was requested — both already
enforced by `.claude/hooks/guard-provider-mutation.sh` and the permission
denials in `.claude/settings.json`; keep them that way.

## 4. Working protocol

- **Verify, do not assume.** Every factual claim about Homebrew, MacPorts or
  Claude Code itself must be (a) observed on this machine with a read-only
  command, (b) cited from official documentation with the date you read it,
  or (c) expressed as runtime capability detection. "I recall that…" is not
  a fourth option. `docs/spec/11-agentic-sdlc.md` already asks for the
  Claude Code documentation to be re-verified every pass (`memory`,
  `sub-agents`, `skills`, `hooks`, `permissions`, `plugins`,
  `best-practices`) — actually fetch and read it during this pass rather
  than relying on what that file currently says.
- **This repository is not yet under version control, and this pass does
  not change that.** `git init` happens once the Foundation Pass has
  produced the first version of code, not before — that is a deliberate
  decision by the project owner, and it matches
  `docs/spec/10-first-pass-work-order.md`'s Step 0, which already assigns
  git initialization to the Foundation Pass. Work directly on the
  filesystem; there is no commit history to keep coherent in this pass —
  completeness is judged by §6, not by commits. `git push` stays denied
  regardless — DP-007 is still open.
- **Never touch the developer's real package managers.** Reading is
  allowed; writing is a build failure regardless of pass.
- **Use the reviewers.** `spec-guardian`, `security-reviewer`,
  `docs-auditor` and `provider-researcher` all apply within this pass, not
  only to future ones. Run independent research or review items in parallel
  subagents rather than doing them inline in the main session — this is the
  same discipline you are formalizing in §5's delegation-by-default policy,
  applied to your own work here.
- **When blocked on a human decision** — anything in
  `docs/decisions-pending.md`'s scope, or a genuinely wrong-looking Tier 1
  invariant, or an external credential/account you don't have — use
  `/decision-pending` or stop and ask directly. Otherwise keep going.

## 5. Order of work

1. **Re-verify the tooling facts.** Fetch the current Claude Code
   documentation pages `docs/spec/11-agentic-sdlc.md` names, and note any
   drift before changing anything that depends on them — changing subagents,
   skills, hooks or settings against stale assumptions would compound the
   problem this step exists to prevent.
2. **Deep specification review.** Read every file in `docs/spec/`,
   `docs/adr/`, `docs/agentic-sdlc/`, `docs/decisions-pending.md` and
   `docs/spec-evolution.md`. Find and resolve inconsistencies, gaps,
   contradictions and duplicated content. A known concrete case to resolve:
   `docs/agentic-sdlc/claude-code-setup.md` appears to overlap with
   `docs/spec/11-agentic-sdlc.md` — establish a single source of truth for
   that material and remove or fold in the other.
3. **Documentation optimization — read this step's rationale carefully
   before touching `docs/spec-evolution.md` or the ADRs.**
   `docs/spec/11-agentic-sdlc.md` currently treats `docs/spec-evolution.md`
   as an append-only log where no material change is ever removed, on the
   principle that silent deviation is the one unrecoverable error in this
   project. That principle does not change. What changes is *how much
   history is worth carrying* while the project is still pre-code: you may
   consolidate `docs/spec-evolution.md` and the ADRs down to the *current*
   rationale per topic — merging redundant entries, dropping what was
   superseded or reverted — as long as you record this policy shift itself
   exactly once, explicitly, in `docs/spec-evolution.md` or a new ADR,
   before applying it. After that point the convention is "consolidate, not
   accumulate" — but a Tier 1–3 change still may never land without a
   traceable record of what changed and why.

   **This does not apply to the product's own history feature.**
   `docs/spec/04-migration-and-safety.md` (snapshot/restore, the History
   section) and `docs/spec/06-cli-and-ux.md` (the `macportsbrew history`
   command) specify how MacPortsBrew records and reverts *migrations* for
   its end users. That is product behavior, not documentation housekeeping,
   and must be preserved and, where it has a real gap, strengthened —
   specifically check that "undo a migration" is a clearly discoverable,
   honestly-scoped user story (no promise of transactional semantics the
   underlying providers don't offer — that constraint already exists and
   must not be weakened).
4. **Agentic-SDLC tooling review.** Evaluate `.claude/agents/*`,
   `.claude/skills/*`, `.claude/hooks/*` and `.claude/settings.json` against
   the re-verified Claude Code documentation from step 1 and against
   general agentic-SDLC and software-engineering best practices. Propose and
   make concrete improvements; add a new subagent, skill or hook only where
   a real, current gap justifies it — `docs/spec/11-agentic-sdlc.md`
   already forbids writing a skill for a procedure that doesn't exist in
   code yet, and that rule still applies.
5. **Formalize sub-agent delegation as the project's default.** Write into
   `docs/spec/11-agentic-sdlc.md` (and reflect briefly in `CLAUDE.md`) an
   explicit policy: read-only research and exploration are delegated to
   subagents by default rather than performed inline; independent review
   dimensions run in parallel; the main session is reserved for synthesis,
   decisions and edits that need full repository context; task "weight"
   (not a hardcoded model name — `SE-017` already forbids that) decides
   whether a task warrants a stronger or a lighter model.
6. **Repository readiness.** Install and configure whatever skills,
   plugins, subagents or supporting tooling the Foundation Pass will
   actually need, within the no-speculative-tooling rule from step 4.
7. **Renumber the pass sequence if warranted.** Given this file's own
   `00-REPO-READINESS-PASS` id sits before the existing `00-FOUNDATION-PASS`,
   decide whether to renumber the sequence (e.g. Foundation → 01,
   Architecture Review → 02, Focused Implementation → 03) for a clean
   ordering, or leave the ids as they are. Either way, update
   `prompts/README.md`'s table and every cross-reference to match your
   choice, and record which you picked and why.
8. **Preflight and close.** Run `/preflight`; resolve or explicitly accept
   in writing what it flags. Set `.claude/current-pass` to the id of the
   pass that should run next (Foundation, under whatever id you settled on
   in step 7). Leave the repository uncommitted and without a `.git`
   directory — that is correct at this point, not an oversight.

## 6. Definition of done

Demonstrate each; do not assert it.

- [ ] No `.git` directory exists yet — version control is still deliberately
      deferred to the Foundation Pass.
- [ ] `docs/spec/*`, `AGENTS.md`, `CLAUDE.md`, `docs/adr/*`,
      `docs/decisions-pending.md` and `docs/spec-evolution.md` are mutually
      consistent, with no duplicated content — the
      `docs/agentic-sdlc/claude-code-setup.md` vs.
      `docs/spec/11-agentic-sdlc.md` overlap named in §5 step 2 is resolved.
- [ ] `.claude/agents/*`, `.claude/skills/*`, `.claude/hooks/*` and
      `.claude/settings.json` have been reviewed against currently-fetched
      Claude Code documentation, with any drift recorded.
- [ ] The sub-agent-delegation-by-default policy is written into
      `docs/spec/11-agentic-sdlc.md` and summarized in `CLAUDE.md`.
- [ ] The migration history/undo user story in
      `docs/spec/04-migration-and-safety.md` and
      `docs/spec/06-cli-and-ux.md` has been checked and, where a real gap
      existed, closed — without promising transactional semantics.
- [ ] The documentation-consolidation policy change (§5 step 3) is recorded
      exactly once, explicitly, before being applied.
- [ ] Any newly installed skill, plugin, subagent or tool is documented and
      justified by a concrete, current need — nothing speculative.
- [ ] `prompts/README.md` and `.claude/current-pass` reflect the final pass
      sequence and numbering decision from §5 step 7.
- [ ] No `src/` or equivalent implementation-code tree exists.
- [ ] No item in `docs/decisions-pending.md`'s *Open* section was resolved
      without the human.
- [ ] `/preflight` passes, or the report names exactly what it could not
      check and why.
- [ ] The reviewers relevant to any change made (`spec-guardian`,
      `security-reviewer`, `docs-auditor`) have run, and their findings are
      resolved or explicitly accepted in writing.

## 7. End-of-pass report

Write it with `/pass-report` to
`docs/passes/00-REPO-READINESS-PASS-report.md`. It indexes what changed —
structurally, what was consolidated or removed, the documentation-policy
decision from §5 step 3, the pass-numbering decision from §5 step 7, and any
open recommendation left for the human before the Foundation Pass starts.
Every material deviation it names must already exist in
`docs/spec-evolution.md`; the report indexes, it does not restate.
