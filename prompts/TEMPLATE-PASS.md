# MacPortsBrew — <Pass name> (<id>)

<One paragraph: what this pass is for, and what state the repository is in when
it starts. Written for someone who has read nothing else.>

---

## 1. Read this first

In this order, before doing anything else:

1. `AGENTS.md` — orientation.
2. `docs/spec/00-charter.md` — what MacPortsBrew is, who for, what it is not.
3. `docs/spec/01-invariants.md` — Tier 1. Non-negotiable.
4. `docs/spec-evolution.md` — what has already been argued, so you do not
   reopen it.
5. `docs/spec/11-agentic-sdlc.md` — how work is shaped here.
6. <the specification files this pass actually needs>

Read the rest on demand. Each file stands alone; do not load the specification
up front.

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

## 3. Hard boundary for this pass

<State it so a grep, a hook or a test can decide whether it held. Not "prefer
not to". Name what does not exist in the artifact this pass produces, and how
that is checked.>

## 4. Working protocol

- **Verify, do not assume.** Observed with a read-only command, cited with a
  read date, or runtime detection. No fourth option.
- **Never touch the developer's real package managers.** Reading is allowed;
  writing is a failure. Hooks block the obvious forms; the rule holds anyway.
- **Re-verify the tooling first.** `/tooling-recheck` at the start of the
  pass; record drift.
- **Delegate by default.** Research, sweeps and reviews go to subagents, in
  parallel where independent; the main session synthesises and edits.
- **Use the reviewers.** `spec-guardian`, `security-reviewer`, `docs-auditor`.
  You are not the right reviewer of your own change.
- **Commit incrementally.** Conventional commits, one coherent change each.
- **When blocked on a human decision**, use `/decision-pending`: options,
  recommendation, reversible default. Then keep going.

## 5. Order of work

<Numbered, where sequence matters. Say why it matters — an agent that
understands the ordering will recover from a surprise; one following a list
will not.>

## 6. Definition of done

Demonstrate each; do not assert it.

- [ ] <checkable item>
- [ ] `/preflight` passes, or the report names what was skipped and why.
- [ ] Every invariant this pass touched can be pointed at by name to its test.
- [ ] `docs/spec-evolution.md` records every material deviation.
- [ ] The reviewers have run and their findings are resolved or accepted in
      writing.

## 7. End-of-pass report

Write it with `/pass-report` to `docs/passes/<id>-report.md`. Every material
deviation it names must already be in `docs/spec-evolution.md`.
