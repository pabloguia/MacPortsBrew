<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification: subagents and skills (2026-10-26)

Date read: **2026-10-26**, by a read-only subagent for `/tooling-recheck`, at the
start of pass 08 slice 6. Previous note: [`2026-10-22-subagents-skills.md`](2026-10-22-subagents-skills.md).

**Verdict:** no contract break. Every frontmatter field the repository uses is
still documented and valid.

## Sources

| Page | Result |
|---|---|
| `https://code.claude.com/docs/en/sub-agents.md` | fetched in full; claims below come from searches over the indexed copy, not an end-to-end reading |
| `https://code.claude.com/docs/en/skills.md` | the same |
| `permissions.md`, `permission-modes.md`, `hooks.md` | not fetched for this group; permission-rule claims are unverified here |

## 1. Fields and values the repository uses

Subagents (`.claude/agents/*.md`):

- `name`, `description`: valid and required; `name` is at most 256 characters, no `:`.
- `tools`: a valid allowlist. `TodoWrite` is confirmed as a built-in tool name (open in the 2026-10-22 note).
- `disallowedTools: Write, Edit, NotebookEdit` on the three reviewers: a valid denylist; an entry removes the whole tool.
- `model: inherit`: valid. Aliases `sonnet`, `opus`, `haiku`, `fable`, or a full model ID. Resolution order: the invocation's parameter, then the frontmatter, then `CLAUDE_CODE_SUBAGENT_MODEL`, then the main model.
- `color`: valid values (red, blue, green, yellow, purple, orange, pink, cyan).
- `permissionMode: plan`: a valid value (`default`/`manual`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`, `plan`), but see 3a.

Skills (`.claude/skills/*/SKILL.md`):

- `name`, `description`, `argument-hint`, `allowed-tools` (comma-separated), `license`: valid. `license` is accepted and not acted on. `argument-hint` is rejected by the claude.ai and API skill upload paths, so these skills cannot be uploaded there as they are (irrelevant to in-repository use).
- No description reaches the 1,536-character listing cut (they run 184–473).

## 2. Mechanisms not used that would enforce a stated rule

Carried from 2026-10-22, still valid and unadopted:

- **a. Bash writes from the three read-only reviewers.** `disallowedTools` denies `Write`/`Edit`, but `Bash` remains, and a `Bash(...)` specifier does not narrow a denied tool. Documented fixes: a frontmatter `PreToolUse` hook on `Bash` with a validating script (sub-agents.md, "Database query validator"), dropping `Bash`, or deny rules in `permissions.deny`.
- **b. `provider-researcher` writes anywhere.** It has `Write` and `Edit` with no path limit; a frontmatter `PreToolUse` hook would confine it to `docs/research/`.
- **c. `context: fork` plus `agent: <name>`** in skill frontmatter would make `research-note` run through `provider-researcher` rather than asking the model to. The fork starts without the conversation, so the skill body must stand alone. A design call.
- **d. `maxTurns`** is set on none of the four agents.
- **e. `disable-model-invocation: true`** on skills with side effects. A trade-off: their descriptions rely on the model invoking them.

New this run:

- **f. Skill-level `disallowed-tools`** removes tools while the skill is active. `allowed-tools` only pre-approves, so `preflight`, `spec-evolution` and `decision-pending`, which omit `Write`/`Edit`, are not restricted by that omission.
- `claude --debug` reports unknown subagent fields (v2.1.296+), and `claude plugin validate` checks a directory of agents (v2.1.233+); `/skill-doctor` reports per-skill context cost (v2.1.252+). Cheap checks a future run can use.

## 3. Guidance not followed, or a statement too strong

- **a. `permissionMode: plan` is not a read-only guarantee.** sub-agents.md, "Permission modes": when the main conversation runs in `bypassPermissions`, `acceptEdits` or auto mode, the subagent runs in that mode and the field is ignored. `disableBypassPermissionsMode` covers the bypass case only. Only `disallowedTools` is firm; the reviewers' Bash remains (2a). This session ran in auto mode; every reviewer write was checked by `git status`.
- **b. `allowed-tools` is a grant, not a restriction.** skills.md: it permits tools without prompting; `disallowed-tools` or deny rules restrict. A path pattern in `tooling-recheck`'s `allowed-tools` (`Edit(docs/research/tooling/**)`) is not a jail. The skill's prose about a subagent type's `tools` list is a different mechanism and is correct.
- **c.** `Agent(type)` lists inside a subagent definition are ignored (they apply only to `claude --agent`). Not used here.

The previous note's "skill body under 500 lines" guidance was not found in the
indexed page this time; treat it as unverified.

## Recommendations (not adopted; `.claude/` waits for the owner)

1. A `PreToolUse` Bash hook (or Bash deny rules) on the three reviewers (2a), and a path hook on `provider-researcher` (2b).
2. Reword `.claude/README.md` and spec 11 wherever `permissionMode: plan` or `allowed-tools` is described as enforcing read-only behaviour (3a, 3b).
3. `disallowed-tools` on the skills that must not write (2f).
