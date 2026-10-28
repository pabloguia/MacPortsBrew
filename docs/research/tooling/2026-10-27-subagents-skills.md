<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification: subagents and skills (2026-10-27)

Date read: **2026-10-27**, read-only, `/tooling-recheck` at the start of work
order 16. Previous note: [`2026-10-26-subagents-skills.md`](2026-10-26-subagents-skills.md).

**Verdict:** no contract break. Every frontmatter field and value the repository
uses is still documented and valid; nothing it uses was renamed or deprecated.

## Sources

| Page | Result |
|---|---|
| https://code.claude.com/docs/en/sub-agents.md | fetched in two windows; the frontmatter and version tables came through a summarising fetch, so treat them as a summary |
| https://code.claude.com/docs/en/skills.md | fetched, same caveat |

`curl` was denied in this run, so the `.md` variants were read with WebFetch.
Neither page had moved. Permission and hook pages were not read here.

## 1. Fields the repository uses: still valid

Subagents (`.claude/agents/*.md`): `name` and `description` (the only required
fields); `tools` as a comma-separated string; `disallowedTools: Write, Edit,
NotebookEdit` (applied before `tools`; a `Bash(...)` specifier there removes the
whole tool); `model: inherit`; `permissionMode: plan` (valid values `default`,
`acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`, `plan`); `color` values
in the valid set. Unrecognised fields are ignored without an error.

Skills (`.claude/skills/*/SKILL.md`): `name`, `description`, `argument-hint`,
`allowed-tools`, `license` all documented; `allowed-tools` comma-separated is
valid; `${CLAUDE_PROJECT_DIR}` is substituted in `allowed-tools` Bash rules.
Outside Claude Code (upload, Skills API) `argument-hint` is a validation error;
irrelevant for in-repository use.

## 2. Changed or newly confirmed since 2026-10-26

- **SKILL.md under 500 lines** is now stated ("Length and truncation"). Repository skill lengths not measured here.
- **Description listing cap** confirmed at 1,536 characters per entry (`skillListingMaxDescChars`); listing budget 1% of context (`skillListingBudgetFraction`). The repository's descriptions are well under.
- Subagent descriptions together count toward a 15,000-token startup warning; four agents are far below.
- **Fork mode** is on by default from v2.1.302; while on, the main session cannot wait for a subagent in interactive sessions. `permissions.deny: ["Agent(fork)"]` or `CLAUDE_CODE_FORK_SUBAGENT=0` turns it off. Not relied on here.
- `Task` renamed `Agent` (v2.1.63), `Task(...)` still an alias; unused in these files.

## 3. Mechanisms not used that would enforce what is only stated

Carried from 2026-10-26, none adopted: a frontmatter `PreToolUse` hook to stop
Bash writes from the three read-only reviewers; a `Write|Edit` hook confining
`provider-researcher` to `docs/research/`; `maxTurns`; `skills:` preload on the
reviewers; `context: fork` with `agent:` for `research-note`; skill-level
`disallowed-tools`; `disable-model-invocation`; skill `hooks`.

## 4. Statements that are too strong (carried over)

- `permissionMode: plan` on the reviewers is not a read-only guarantee: the main session's mode can override it. Only `disallowedTools` is firm, and Bash stays.
- `allowed-tools` pre-approves; it does not restrict. Omitting `Write`/`Edit` from a skill does not forbid them, and path patterns in `tooling-recheck`'s `allowed-tools` are not a jail.

## Not verified this run

`TodoWrite` as a tool name; any permission or hook page; skill body line counts.
