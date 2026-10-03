# Tooling recheck: subagents and skills, 2026-10-02

Read on: 2026-10-02

Sources, both fetched on the first try as the `.md` variant, no redirect, none
unfetched:

- <https://code.claude.com/docs/en/sub-agents.md>, read 2026-10-02
- <https://code.claude.com/docs/en/skills.md>, read 2026-10-02

Previous note: [`2026-09-27-subagents-skills.md`](2026-09-27-subagents-skills.md).

Files checked: `.claude/agents/{docs-auditor,provider-researcher,security-reviewer,spec-guardian}.md`
and `.claude/skills/*/SKILL.md` (13 skills; the previous note said 14 but
listed 13 names, and 13 is the count on disk).

**Method caveat.** The fetch tool returns a model-summarised extraction of each
page, not its verbatim text; the claims about the pages below are as that
extraction reported them. The frontmatter was read directly from disk.

## Drift

**Repository drift: none.** No commit touched `.claude/agents` or
`.claude/skills` since 2026-09-27, and every frontmatter field and value the
repository uses is still documented and valid.

**Documentation drift since 2026-09-27: additions only, none breaking.**

- Subagent `permissionMode` lists `manual` as an alias for `default`, and
  `dontAsk`. **New nuance:** `permissionMode` is ignored when the main
  conversation is in `bypassPermissions`, `acceptEdits` or auto mode (F1).
- Skill `allowed-tools` pre-approves tools for the turn and does not restrict
  them; the grant clears at the next user message. A skill `disallowed-tools`
  field now exists (F2).
- Newly listed subagent fields: `maxTurns`, `omitClaudeMd`, `skills`, `memory`,
  `initialPrompt`, `experimental.cacheTtl`. Newly listed skill fields:
  `when_to_use`, `arguments`, `disallowed-tools`, `effort`, `background`,
  `paths`, `shell`.
- Skill booleans accept `yes/no/on/off/1/0`; relevant only if a field is added.
- `description` plus `when_to_use` is capped at 1,536 characters in the
  listing; the longest description here is about 356.
- `disable-model-invocation: true` also prevents preloading a skill into
  subagents and running it in scheduled tasks — which reinforces the
  2026-09-27 decision not to set it.

## 1. Fields the repository uses, re-confirmed

### Subagents (sub-agents.md, read 2026-10-02)

| Field | Used in | Value | Documented now? | Valid? |
|---|---|---|---|---|
| `name` | all 4, line 2 | the four agent names | Yes, required; must not start with `-` or contain `:` | Yes |
| `description` | all 4, line 3 | prose | Yes, required; combined descriptions over 15,000 tokens warn | Yes |
| `tools` | all 4, line 4 | `Read, Grep, Glob, Bash`; `provider-researcher` adds `WebFetch, WebSearch, Write, Edit, TodoWrite` | Yes, an allowlist | Yes; the three extra names are listed among the tools retained in background subagents, which closes the 2026-09-27 caveat |
| `disallowedTools` | the three reviewers, line 5 | `Write, Edit, NotebookEdit` | Yes; applied first | Yes, redundant with `tools` (F3) |
| `model` | all 4 | `inherit` | Yes | Yes; resolution order is the per-invocation parameter, the frontmatter, `CLAUDE_CODE_SUBAGENT_MODEL`, then the main model |
| `permissionMode` | the three reviewers, line 7 | `plan` | Yes | Valid; see F1 |
| `color` | all 4 | `green`, `cyan`, `red`, `orange` | Yes | Yes |

### Skills (skills.md, read 2026-10-02)

| Field | Used in | Documented now? | Valid? |
|---|---|---|---|
| `name` | all 13, equal to the directory name | Yes; defaults to the directory name | Yes |
| `description` | all 13 | Yes; 1,536-character cap with `when_to_use` | Yes; 197 to 356 characters, above the ~150 the page calls optimal for matching, not a defect |
| `argument-hint` | 12 of 13 (not `preflight`) | Yes | Yes |
| `allowed-tools` | all 13 | Yes; plain names, `Bash(pattern)`, globs | Yes; see F2. The `WebFetch(domain:...)` and `Edit/Write(<glob>)` forms are not shown on this page; the 2026-09-27 note confirmed them on `/permissions`, which this group did not re-read |
| `license` | all 13 | Yes; accepted, not enforced | Yes |

`${CLAUDE_PROJECT_DIR}` in `allowed-tools` is still documented. Every
`SKILL.md` is under 500 lines (49 to 88). No reserved names are used.

## 2. Documented mechanisms not used that would enforce what is only stated

- **`disable-model-invocation` on the recipe skills.** Not adopted; the
  2026-09-27 decision stands and the new preloading consequence strengthens
  it. Do not re-raise.
- **Skill `disallowed-tools`.** Could forbid `Write` in a read-only skill such
  as `preflight`. No current claim requires it; optional.
- **Agent frontmatter `hooks` (`PreToolUse`).** Could enforce "the
  researcher writes only `docs/research/`", today stated in prose. Project
  agents' frontmatter hooks need workspace trust, and `Stop` becomes
  `SubagentStop`. Optional.
- **`maxTurns`, `skills`, `effort`, `isolation`, `memory`.** Unused, and no
  stated claim needs them.
- **`context: fork` for `tooling-recheck`.** Still forks one subagent, not
  three; considered and rejected on 2026-09-27.

## 3. Findings

**F1. `permissionMode: plan` is not read-only enforcement for the reviewers.**
`.claude/agents/{docs-auditor,security-reviewer,spec-guardian}.md:7`. The page
says the field is ignored under a `bypassPermissions`, `acceptEdits` or auto
main session. `Write`, `Edit` and `NotebookEdit` are still unavailable to the
reviewers (their `tools` list excludes them), but `Bash` remains and can write
through a redirect. `docs/spec/11-agentic-sdlc.md` already names `Bash` as the
documented gap and the `PreToolUse` guard as the boundary; what is new is that
`plan` may not apply at all. Documented options: rely on `permissions.deny`
plus the hooks (the current enforcement), a frontmatter `PreToolUse` hook, or
dropping `Bash` from the reviewers. **Action taken:** the chapter's sentence
is corrected to say so; nothing under `.claude/` changes.

**F2. A skill's `allowed-tools` pre-approves; it does not restrict.** All 13
`SKILL.md` files. Any prose implying, for example, that `tooling-recheck` can
only write under `docs/research/tooling/` overstates the field. Restricting
mechanisms are the skill's `disallowed-tools` and a subagent's `tools` and
`disallowedTools`. **Action:** a wording check of the recipe documentation, not
performed in this pass (`.claude/` is outside its boundary).

**F3. `disallowedTools` on the reviewers is redundant.** Line 5 of each
reviewer. Harmless belt-and-braces; no fix.
