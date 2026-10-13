# Tooling recheck: subagents and skills, 2026-10-10

Read on: 2026-10-10

Sources (both fetched first try as the `.md` variant, no redirect, none unfetched):

- <https://code.claude.com/docs/en/sub-agents.md>, read 2026-10-10
- <https://code.claude.com/docs/en/skills.md>, read 2026-10-10

Previous note: [`2026-10-03-subagents-skills.md`](2026-10-03-subagents-skills.md).

Files checked: `.claude/agents/{docs-auditor,provider-researcher,security-reviewer,spec-guardian}.md`
and `.claude/skills/*/SKILL.md` (13 skills, 49–88 lines each, under the 500-line guidance).

**Method caveat.** Field tables were read by line match on the raw `.md` pages plus
indexed excerpts. Not re-read this time: "Available tools" (`Agent(type)`
syntax), the background-retained tool list, hooks-in-subagents. Those
2026-10-03 facts are carried over, not re-verified.

## 1. Drift since 2026-10-03

### Repository

One commit touched this group after the previous note (`753bdfb`, 2026-10-04):

- Previous **F1** addressed in wording: `.claude/skills/tooling-recheck/SKILL.md:23`
  now says the Agent call does not enforce read-only.
- Previous **F2** resolved: `preflight`'s `allowed-tools` grants no `cargo` pattern.
- `.claude/README.md:24` now lists all 13 skills.

### Documentation (additions only, none breaking)

Sub-agents page:

- `permissionMode` values: `default`, `acceptEdits`, `auto`, `dontAsk`,
  `bypassPermissions`, `plan`, and `manual` (new alias for `default`). The main
  conversation's `bypassPermissions`, `acceptEdits` or auto mode still overrides it.
- New `omitClaudeMd` (v2.1.271+): launches without user/project/local CLAUDE.md.
- `name` at most 256 characters, no `:`; filename need not match.
- `maxTurns` returns output marked partial (v2.1.246+), resumable.
- `model` accepts `sonnet`, `opus`, `haiku`, `fable`, a full ID, or `inherit`.
- `disallowedTools` with a specifier still removes the whole tool.
- `color`: `red, blue, green, yellow, purple, orange, pink, cyan`.
- Other documented, unused fields: `hooks`, `memory`, `background`, `effort`,
  `isolation`, `initialPrompt`, `skills`, `mcpServers`.

Skills page:

- New or newly noted fields: `when_to_use` (joined to `description`, truncated at
  1,536 characters in the listing), `arguments`, `disallowed-tools`,
  `user-invocable`, `background` (with `context: fork`, v2.1.218+), `paths`,
  `shell`, `metadata`, `compatibility`, per-skill `model` and `effort`. `license`
  is accepted, not acted on.
- `disable-model-invocation: true` also blocks preloading into subagents and
  scheduled-task runs.
- `allowed-tools` unchanged: grants for the invoking turn only, does not restrict
  the tool set, not gated by workspace trust; `allowManagedPermissionRulesOnly`
  makes Claude Code ignore project skills' `allowed-tools` (v2.1.282+).
- Listing budget is 1% of the context window; tunable by
  `skillListingBudgetFraction`, `SLASH_COMMAND_TOOL_CHAR_BUDGET`, `skillOverrides`.
- New: a project skill named `verify` or `simplify` runs before each commit
  (v2.1.286+). No project skill has either name.
- Linked worktrees search `.claude/skills` only up to the worktree root; v2.1.277+
  falls back to the main checkout's.

## 2. Fields the repository uses, re-confirmed

| Field | Where | Valid? |
|---|---|---|
| `name`, `description` | all agents and skills | Yes |
| `tools` | all 4 agents | Yes |
| `disallowedTools` | the 3 reviewers | Yes; redundant with `tools` (F3) |
| `model: inherit` | all 4 agents | Yes |
| `permissionMode: plan` | the 3 reviewers | Valid; overridden by an auto or bypass main session (F4) |
| `color` | all 4 agents | Yes |
| `argument-hint`, `allowed-tools`, `license` | skills | Yes |

## 3. Documented mechanisms not used

- A read-only research agent type for `tooling-recheck`'s subagents (F1).
- Subagent `hooks` to confine `provider-researcher` to `docs/research/`. Optional.
- `omitClaudeMd`: would drop the invariants pointers reviewers inherit; do not adopt without a reason.
- `maxTurns`, `effort`, `isolation`, `memory`, `skills`, `background`: nothing stated depends on them.

## 4. Findings

- **F1 (carried).** `tooling-recheck`'s subagents are read-only by instruction, not
  construction. Fix when wanted: a `docs-fetcher` agent with `tools: Read, Grep, Glob,
  WebFetch, WebSearch` and `disallowedTools: Write, Edit, NotebookEdit`, named by the skill.
- **F2 (carried).** Checked-in skills' `allowed-tools` grant `cargo` and the built
  binary, ungated by workspace trust; relevant once the repository is public.
  Mitigation is user-side (`allowManagedPermissionRulesOnly`). Accepted with DP-007.
- **F3 (carried, harmless).** Reviewers' `disallowedTools` duplicate `tools`; kept as defence in depth.
- **F4 (info).** `permissionMode: plan` is overridden by an auto or bypass main
  session; the reviewers' real read-only guarantee is `tools`/`disallowedTools`.

## 5. Not verified this time

`SubagentStart`/`SubagentStop` names, `Agent(type)` syntax, plugin-subagent
restrictions, fork mode — all unused by this repository's agents and skills.
