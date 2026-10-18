<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification — subagents and skills (2026-10-17)

Group: `sub-agents` and `skills`, read on 2026-10-17 from
`https://code.claude.com/docs/en/sub-agents.md` and `/skills.md` (both fetched,
read through a summarising fetch, about 110k characters each). Returned by a
read-only subagent; the runner wrote this file. Verdict: **drift, 5 items**, all
wording or unverified specifiers, none a contract break.

## Still documented and valid

Agent frontmatter `name`, `description`, `tools` (comma string),
`disallowedTools`, `model: inherit`, `permissionMode: plan`, `color`; `TodoWrite`
as a tool name. Skill frontmatter `name`, `description`, `argument-hint`,
`allowed-tools` (comma string), `license` (accepted, not acted on);
`${CLAUDE_PROJECT_DIR}` in `Bash(...)` rules; `$ARGUMENTS`. Descriptions are
183–342 characters (cap 1,536); the longest body is 88 lines (guidance 500).

## Drift

1. Seven skills spell `Bash(cargo test:*)`, the colon-star form. The page shows
   only `Bash(pattern)` with a trailing `*`, e.g. `Bash(python3 *)`, and does
   not say `:*` is equivalent. Fix per page: `Bash(cargo test *)`. Skills are
   under `.claude/`; the path guard refuses a hand edit, so this is the owner's.
2. Unverified: `Edit(docs/research/tooling/**)` and `Write(...)` in
   `tooling-recheck/SKILL.md`, and `WebFetch(domain:...)` in `add-mapping`. The
   skills page documents only Bash patterns; validity depends on `permissions`
   (the sibling note read it). `allowed-tools` pre-approves for the turn and is
   not a restriction.
3. Unverified: `Agent` in `tooling-recheck/SKILL.md` `allowed-tools`.
4. `research-note/SKILL.md` says delegating to `provider-researcher` keeps the
   read-only discipline "in a place where it is stated rather than remembered".
   The agent holds Write and Edit; its limit to its own note is prose.
   `docs/spec/11-agentic-sdlc.md` already admits it. The skill wording over-claims.
5. The reviewers' "Read-only reviewer" claim holds for the file tools only:
   Bash is in `tools`, and `permissionMode` is ignored when the main session runs
   in `bypassPermissions`, `acceptEdits` or `auto`. `docs/spec/11-agentic-sdlc.md`
   records both caveats; the frontmatter wording is what is stale.

## Documented mechanisms not used

A `PreToolUse` hook in an agent's frontmatter (confine `provider-researcher` to
`docs/research/`; block writing Bash for the reviewers); `context: fork` with
`agent:` on `research-note`; `permissions.deny` Bash rules for subagent writes;
skill `disallowed-tools`; `maxTurns`. `disable-model-invocation` is deliberately
unused (a recorded decision).

## Guidance not followed

Agent descriptions run three to four sentences where the page says keep them
brief (minor). `tooling-recheck` itself notes the Agent call does not enforce
"write nothing"; the remedy is a `tools` list without Write/Edit/NotebookEdit,
which this run used by choosing the read-only `Explore` type.
