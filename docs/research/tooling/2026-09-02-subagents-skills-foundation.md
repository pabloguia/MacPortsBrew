# Verification: Claude Code subagent and skill documentation vs. this repository (second run, 2026-09-02)

**This is the second `/tooling-recheck` run for the "Subagents and skills"
group dated 2026-09-02.** The first run produced
`docs/research/tooling/2026-09-02-subagents-skills.md`. This note reports only
the delta since that run, plus a re-confirmation of the one open question
(`permissionMode: plan` vs. `Bash`-driven writes) with a tighter quote. It does
not repeat material the first note already established correctly and that
this run found unchanged.

Read date for every documentation fact below: **2026-09-02**.

Sources fetched today (WebFetch; both succeeded, nothing unfetched):

- S1 `https://code.claude.com/docs/en/sub-agents` — fetched twice: once for
  the general frontmatter/invocation summary, once narrowly for every
  sentence containing "permissionMode", "plan mode" or "read-only
  exploration", to get a complete, non-paraphrased quote.
- S2 `https://code.claude.com/docs/en/skills` — fetched once; output
  (96.7 KB) persisted to disk and grepped locally for the frontmatter
  reference table, `disable-model-invocation`, `allowed-tools`,
  `$ARGUMENTS`, and related sections rather than re-reading the whole page
  into context.

Repository files re-read: `.claude/agents/{docs-auditor, provider-researcher,
security-reviewer, spec-guardian}.md` (all four, full content); all seven
`.claude/skills/*/SKILL.md` frontmatter blocks and their `$ARGUMENTS` usage
(`adr`, `decision-pending`, `pass-report`, `preflight`, `research-note`,
`spec-evolution`, `tooling-recheck` — the last did not exist at the previous
run, or was not in scope of it); `docs/spec/11-agentic-sdlc.md` lines 185–219
("Subagents"); `docs/spec-evolution.md` (grep for SE-027, SE-029, SE-033).

---

## 1. The open question: does `permissionMode: plan` stop a `Bash` heredoc or `sed -i`?

`docs/spec/11-agentic-sdlc.md` ("Subagents", MUST on the reviewers) says
verbatim:

> Whether plan mode also stops a `Bash` heredoc or `sed -i` is **not
> verified**: the security review of the readiness pass observed a reviewer
> creating files under its scratch directory through `Bash` while running
> under that setting. Until `/tooling-recheck` settles it, `Bash` is the
> documented gap and the reviewers' prompts are what keep them from
> patching.

**Answer as of 2026-09-02: the docs still do not settle it, and the observed
behaviour is consistent with the docs' own silence.**

The entire documented content of `permissionMode: plan`, quoted verbatim from
S1's `permissionMode` table:

> | `plan` | Plan mode (read-only exploration) |

That six-word gloss is the complete definition in the frontmatter reference
table — there is no linked "how plan mode enforces read-only" section from
that row. A second, narrower fetch of S1 (asking specifically for every
sentence mentioning "permissionMode", "plan mode" or "read-only
exploration", plus the full permission-mode table) surfaced exactly two
other mentions on the whole page, neither of which addresses `Bash`:

> "A research agent used during [plan mode]... to gather context before
> presenting a plan." (describing the built-in `Plan` subagent)

> "`ExitPlanMode`, unless the subagent's `permissionMode` is `plan`" (listing
> `ExitPlanMode` as a tool available only to plan-mode subagents)

Nothing on S1 says plan mode inspects `Bash` command content, denies
filesystem-mutating shell invocations, or otherwise restricts what a `Bash`
call can do. The full `permissionMode` table (S1, verbatim) confirms the
other six modes are similarly one-line glosses (`default`: "prompts for
permission"; `bypassPermissions`: "Skip permission prompts"; etc.) — plan
mode is not an outlier in being under-specified, it is simply the one this
project depends on for a load-bearing guarantee.

**Conclusion: `docs/spec/11-agentic-sdlc.md`'s "not verified" / "documented
gap" language remains accurate today and should not be softened.** The
project's own observation (a reviewer created scratch files via `Bash` under
`permissionMode: plan`) is the only evidence either way, and it points toward
plan mode gating the `Edit`/`Write`/`NotebookEdit` *tools* (and `ExitPlanMode`
availability) rather than gating `Bash` by content inspection — but that
remains an inference from one observation, not a documented contract. The
repository's mitigating control (`disallowedTools: Write, Edit, NotebookEdit`
on all three reviewers, which is a real, documented, enforced restriction
independent of plan mode) is therefore still the load-bearing protection, and
the spec's framing — "the reviewers' prompts are what keep them from
patching" against `Bash` specifically — is the correct level of caution to
keep in the specification. No change recommended to that paragraph.

## 2. Subagent frontmatter — unchanged fields, one material change since the last run

Every field listed in the previous note (`name`, `description`, `tools`,
`disallowedTools`, `model`, `permissionMode`, `maxTurns`, `skills`,
`mcpServers`, `hooks`, `memory`, `background`, `effort`, `isolation`,
`color`, `initialPrompt`, `experimental`) is still documented at S1 today,
with the same valid values (confirmed again via a fresh fetch rather than
trusted from the prior note). `model: inherit` — "inherit: use the same
model as the main conversation" — is still present and still the only
SE-017/SE-033-compatible value.

**Material change since the prior run: all four repository agents now carry
`permissionMode: plan`.** The prior note's table showed no `permissionMode`
column populated for any of the four agents and recommended adding it to the
three reviewers as recommendation #1. Reading the files today:

| Agent | `permissionMode` | Matches prior recommendation? |
|---|---|---|
| docs-auditor | `plan` | yes (was recommended) |
| security-reviewer | `plan` | yes (was recommended) |
| spec-guardian | `plan` | yes (was recommended) |
| provider-researcher | (absent) | correct — this agent writes research notes, so plan mode would be wrong for it |

This is recommendation #1 from the prior note, adopted exactly as
recommended, and it is documented in `docs/spec-evolution.md` SE-027
("read-only twice over"). No drift: field name, value, and applicability all
still match S1.

**Still not adopted (recommendations #2, #3, #5 from the prior note — not
defects, just unexercised, documented mechanisms):** none of the four agents
sets `maxTurns`, `background`, `memory`, `skills`, `mcpServers`, `hooks`,
`isolation`, or `effort`. All eight remain valid, documented fields per S1
today. Restating only the two with the clearest fit, since the prior note
already made the case in full:

- `background: true` on `provider-researcher` would make the spec's own
  claim — "independent items run in parallel without polluting the main
  context" (`docs/spec/11-agentic-sdlc.md`, Subagents table) — a harness
  property instead of a convention the invoking agent has to remember.
- `maxTurns` on the three reviewers would bound a runaway review; S1 still
  documents the output as "returned... marked as partial" and resumable at
  the limit, so this is a strict downside-free addition when adopted.

Neither is a documented gap or a spec violation; they are optional
mechanisms the repository has chosen not to use yet, exactly as the prior
note characterized them.

## 3. Skill frontmatter and invocation — three prior recommendations adopted, one deliberately reversed

S2's frontmatter reference table was re-fetched and re-grepped in full
today; every field the prior note documented (`name`, `description`,
`when_to_use`, `argument-hint`, `arguments`, `disable-model-invocation`,
`user-invocable`, `allowed-tools`, `disallowed-tools`, `model`, `effort`,
`context`, `agent`, `background`, `hooks`, `paths`, `shell`, `metadata`,
`license`, `compatibility`) is still documented with the same semantics,
including the still-accurate portability note that only `name`,
`description`, `license`, `compatibility`, `metadata`, `allowed-tools`
survive outside Claude Code (claude.ai uploads, Skills API,
`package_skill.py`).

Checking the seven skills against the prior note's six numbered findings and
ten recommendations:

**Recommendation #6 (add `$ARGUMENTS` or a named placeholder) — adopted in
all six skills that had it as an open recommendation, plus the new seventh
skill:**

| Skill | `$ARGUMENTS` in body? |
|---|---|
| adr | yes — `Decision to record: $ARGUMENTS` |
| decision-pending | yes — `What needs deciding: $ARGUMENTS` |
| pass-report | yes — `Pass: $ARGUMENTS` |
| research-note | yes — `Item: $ARGUMENTS (an ...)` |
| spec-evolution | yes — `Change to record: $ARGUMENTS` |
| tooling-recheck | yes — `Scope: $ARGUMENTS (empty means every page below)` |
| preflight | n/a — takes no arguments, correctly has no `argument-hint` and no `$ARGUMENTS` |

Previously the prior note's finding #1 was "no skill body contains
`$ARGUMENTS`, `$N` or `$ARGUMENTS[N]`" despite five declaring an
`argument-hint`. That is now false for all six that declare one; drift is
resolved, not introduced.

**Recommendation #9 (narrow `pass-report`'s bare `Bash` pre-approval) —
adopted.** Prior note flagged `pass-report`'s `allowed-tools` as
pre-approving unrestricted `Bash` for the invoking turn. Today's
`allowed-tools` line is:

```
allowed-tools: Read, Grep, Glob, Write, Edit, Bash(${CLAUDE_PROJECT_DIR}/.claude/hooks/stop-gate.sh), Bash(git status*), Bash(git log*), Bash(grep *), Bash(wc *)
```

Bare `Bash` is gone; every `Bash` entry is now a scoped `Bash(pattern)` rule,
matching `preflight`'s style as the prior note recommended. All patterns are
documented, valid `Bash(...)` rule syntax per S2 (space- or comma-separated,
`${CLAUDE_PROJECT_DIR}` substitution documented and used correctly).

**Recommendation #8 (remove the no-op `disable-model-invocation: false` from
`preflight`) — adopted.** `preflight`'s frontmatter today has no
`disable-model-invocation` key at all (confirmed by reading the raw file);
the prior note's observation that the explicit `false` "is valid but carries
no information" is now moot because the key is simply absent, which is
equivalent and cleaner.

**Recommendation #7 (side-effecting skills SHOULD set
`disable-model-invocation: true`) — reversed, not adopted, and this is
correct process, not drift.** None of the seven skills sets
`disable-model-invocation` today (confirmed: `grep -rn
disable-model-invocation .claude/skills/*/SKILL.md` returns nothing). The
prior note flagged this as unresolved drift against the SHOULD in
`docs/spec/11-agentic-sdlc.md`. `docs/spec-evolution.md` SE-027 has since
recorded the resolution explicitly: "The SHOULD that side-effecting skills
set `disable-model-invocation: true` is **reversed**: the procedural skills
stay [model-invocable]," citing this exact tooling note
(`research/tooling/2026-09-02-subagents-skills.md`) as evidence and reasoning
that `disable-model-invocation: true` would also block preloading into
subagents (documented at S2: "Also prevents the skill from being preloaded
into subagents") — which the project wants for at least `spec-evolution`.
**This is the SE-032-consolidated topic entry working as designed**: a
material spec change, recorded once, with the tooling note as its evidence.
No further action; nothing to re-flag.

## 4. Mechanisms the repository does not use but that S2 documents and would fit

Two additions to the prior note's list, found on today's closer read of the
frontmatter table:

- **`when_to_use`** (documented, "Additional context for when Claude should
  invoke the skill, such as trigger phrases or example requests. Appended to
  `description`... and counts toward the 1,536-character cap.") — none of
  the seven skills use it. Given the project's own descriptions already
  read as dense trigger phrases ("Use whenever you are blocked on a choice
  that trades away part of the target audience..."), this is a genuine
  option rather than a gap: splitting the "what" into `description` and the
  "when" into `when_to_use` could make the listing easier to scan, but there
  is no functional difference today, so this stays a style note, not a
  recommendation to act on.
- **`paths`** (documented, "Glob patterns that limit when this skill is
  activated... Claude loads the skill automatically only when working with
  files matching the patterns") — not used by any of the seven. Not clearly
  applicable: none of these seven skills is file-type-scoped (they are
  procedure-scoped: write an ADR, run preflight, re-check tooling), so
  omitting `paths` looks correct rather than incomplete.

Everything else worth adopting was already named in the prior note
(`maxTurns`, `background: true`, `memory`, `skills:` preload, `context: fork`
for a skill that should run in isolation) and remains unadopted and
undecided — restated in §2 above only where it intersects the subagent
files; not re-argued in full here since the prior note's reasoning is
unchanged.

## 5. Pages unfetched

None. Both assigned pages (`sub-agents`, `skills`) were fetched successfully
today, the second `sub-agents` fetch specifically to extract a complete,
non-paraphrased quote for §1.
