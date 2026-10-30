<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification: subagents and skills (2026-10-30)

Date read: **2026-10-30**. Read-only research. Previous note: [`2026-10-27-subagents-skills.md`](2026-10-27-subagents-skills.md).

**Verdict:** no contract break. Every frontmatter field and value the repository uses is still documented and valid. `claude plugin validate` passes on both directories. Two things are new or newly stated and matter here. First, the sub-agents page now spells out what happens to `permissionMode: plan` under each main-session mode. Second, the skills page now says an `allowed-tools` grant lasts only for the invoking turn. Neither breaks a file. Both sharpen the statements in `docs/spec/11-agentic-sdlc.md` §Subagents. Nothing was renamed or deprecated.

## Sources

| Page | Result (2026-10-30) |
|---|---|
| https://code.claude.com/docs/en/sub-agents | Fetched and indexed (110 KB). The `.md` variant was read with WebFetch at offset 0 (chars 0–100,000). **Chars 100,000–116,070 were not read.** |
| https://code.claude.com/docs/en/skills | Fetched and indexed (115 KB). The `.md` variant was read with WebFetch at offsets 0 and 100,000, so the whole page was covered. |

`curl` was denied by the project hook (`Bash(curl *)`), so the pages were read with WebFetch and the context-mode fetcher. WebFetch summarises through a small model. Tables were requested verbatim, but treat them as close quotations, not byte-exact. Neither page moved. Hook, permission and settings pages were not read in this run.

**Machine observation (2026-10-30):**
- `claude --version` reports `2.1.278 (Claude Code)`.
- `claude plugin validate .claude/agents` passes ("Validation passed").
- `claude plugin validate .claude/skills` passes ("Validation passed").

## 1. Fields the repository uses: still documented, still valid

### Subagents

Source: https://code.claude.com/docs/en/sub-agents, "Supported frontmatter fields", read 2026-10-30.

| File:line | Field = value | Status |
|---|---|---|
| all four `.claude/agents/*.md`:2–3 | `name`, `description` | Documented as the only required fields. `name` must be 256 characters or fewer, with no `:` and no leading `-`. All four names comply. The longest description is 472 characters. |
| `docs-auditor.md:4`, `security-reviewer.md:4`, `spec-guardian.md:4` | `tools: Read, Grep, Glob, Bash` | Valid as a comma-separated string. |
| `provider-researcher.md:4` | `tools: … Write, Edit, TodoWrite` | Valid. **Newly confirmed:** `TodoWrite` is a resolvable tool name. It appears in the documented background-subagent built-in allowlist (Read, Grep, Glob, LSP, Bash, PowerShell, Edit, Write, NotebookEdit, WebFetch, WebSearch, TodoWrite, Skill, …). This closes the 2026-10-27 "not verified" item. Every tool this agent lists survives the background filter. |
| `docs-auditor.md:5`, `security-reviewer.md:5`, `spec-guardian.md:5` | `disallowedTools: Write, Edit, NotebookEdit` | Valid. It is applied before `tools`. A specifier entry still removes the whole tool. |
| all four:`model` line (6 or 5) | `model: inherit` | Valid. The documented values are `sonnet`, `opus`, `haiku`, `fable`, a full model ID, or `inherit`. |
| `docs-auditor.md:7`, `security-reviewer.md:7`, `spec-guardian.md:7` | `permissionMode: plan` | Valid. The value set is `default`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`, `plan`, plus **`manual` as an alias for `default`**. The alias is not in the 2026-10-27 note. See §2 for the behaviour. |
| `docs-auditor.md:8` (green), `security-reviewer.md:8` (red), `spec-guardian.md:8` (orange), `provider-researcher.md:6` (cyan) | `color` | Valid. The set is `red`, `blue`, `green`, `yellow`, `purple`, `orange`, `pink`, `cyan`. |
| all four:1 | opening `---` on line 1 | Required. A file whose `---` is not on its first line is treated as documentation and skipped silently ("Subagent files Claude Code skips"). All four comply. |

### Skills

Source: https://code.claude.com/docs/en/skills, "Frontmatter reference", read 2026-10-30.

| File:line | Field | Status |
|---|---|---|
| every `SKILL.md`:2–3 | `name`, `description` | Documented. Every field is optional; `description` is "Recommended". The combined `description` + `when_to_use` text is capped at 1,536 characters (configurable via `skillListingMaxDescChars`). The repository's descriptions are well under the cap. |
| every `SKILL.md` except `preflight`:4 | `argument-hint` | Documented. Outside Claude Code it is still a hard error. The documented upload allowlist is `name`, `description`, `license`, `compatibility`, `metadata`, `allowed-tools`. It is irrelevant here (in-repository use only). |
| every `SKILL.md`:5 (`preflight`:4) | `allowed-tools`, comma-separated, with `Bash(...)`, `WebFetch(domain:...)`, `Edit(path/**)` and `Write(path/**)` rules | Documented. Space- or comma-separated strings and YAML lists are accepted. `${CLAUDE_PROJECT_DIR}` is still substituted in `allowed-tools` Bash rules and in the skill body. **Newly stated:** "The grant clears when you send your next message." See §2. |
| every `SKILL.md`:6 (`preflight`:5) | `license: Apache-2.0` | Documented. "Claude Code accepts the field but doesn't act on it." Part of the Agent Skills spec. |
| every `SKILL.md` | line count | Measured this run. The longest is `add-mapping/SKILL.md` at 88 lines, against the documented "under 500 lines". |

## 2. Changed or newly stated since 2026-10-27

All of the following are from the 2026-10-30 reads.

1. **`permissionMode` resolution is now documented case by case** (sub-agents, "Permission modes"):
   - If the main session is in `bypassPermissions`, `acceptEdits` or auto, the subagent runs in that mode and the field is ignored. This was already known; the auto-mode classifier also reviews the subagent's final report.
   - **New:** if the main session is in `plan`, the subagent stays in plan "whichever `permissionMode` you set" (except agent-team teammates).
   - **New:** if the main session is in `default` or `dontAsk`, the value you set applies, except `bypassPermissions` (v2.1.267+) and `auto` when auto mode is unavailable.
   - **Consequence:** the reviewers' `plan` takes effect only when the main session is in `default`, `dontAsk` or `plan`. This very run's main session was in auto mode, so the reviewers' `plan` was ignored.
2. **`allowed-tools` is per-turn** (skills, frontmatter and "Pre-approve tools for a skill"). The grant "clears when you send your next message". It "does not restrict which tools are available". This is consistent with the earlier "pre-approves, does not restrict", and narrower in time.
3. **Skill `hooks` outlive the skill.** They are registered at invocation and keep running "for the rest of the session". A `once` option exists. The skills page now recommends this for "a rule that must hold every time" (Troubleshooting, "Claude stops following a skill").
4. **Skill frontmatter rows present on 2026-10-30.** The 2026-10-27 note did not enumerate these, so when they were added is undetermined: `when_to_use`, `arguments`, `disallowed-tools`, `user-invocable`, `model`, `effort`, `context`, `agent`, `background` (with `context: fork`, default `true`), `hooks`, `paths`, `shell`, `metadata`, `compatibility`.
5. **Subagent frontmatter rows present on 2026-10-30** that the previous note did not list:
   - `autoCompactWindow` (v2.1.296+)
   - `omitClaudeMd` (v2.1.271+)
   - `experimental.cacheTtl` (v2.1.248+)
   - `background`, `effort`, `isolation: worktree`, `initialPrompt`, `memory`, `mcpServers`, `skills`
   - `maxTurns`, which now returns partial output that can be resumed (v2.1.246+)
6. **Background is the default** for subagents spawned from the main conversation. Background subagents get a reduced built-in tool set. "The same definition can resolve to different tools in the foreground and the background." The repository's agents use only allowlisted tools, so nothing is lost.
7. **Diagnostics now documented:**
   - `claude plugin validate <dir>` checks that frontmatter parses (v2.1.233+). It does not flag a parsed file that lacks `name`.
   - `claude --debug` logs "does not recognize or apply" for misspelled fields (v2.1.296+). **This machine runs v2.1.278, so this log line is not available here.**
8. **Unchanged since 2026-10-27:**
   - Fork mode is on by default from v2.1.302, and in an interactive terminal from v2.1.232.
   - `Task` was renamed `Agent` in v2.1.63, and `Task(...)` still works as an alias.
   - Unrecognised fields are ignored without an error.
   - Plugin subagents ignore `hooks`, `mcpServers` and `permissionMode`. This does not apply here; these are project agents.
   - Listing budget is 1% of the context window (`skillListingBudgetFraction`).
   - The 1,536-character cap.
   - New on the page but not relevant here: a v2.1.304 resume fix.

## 3. Documented mechanisms not used that would enforce what is only stated

| Stated (file:line) | Documented enforcement (sub-agents / skills, read 2026-10-30) | Status |
|---|---|---|
| Reviewers are "Read-only reviewer" (`docs-auditor.md:3`, `security-reviewer.md:3`, `spec-guardian.md:3`). Spec 11:259–266 names `Bash` as the gap. | Frontmatter `hooks: PreToolUse` with `matcher: "Bash"`, scoped to the subagent (sub-agents, "Define hooks for subagents"). Or a project `SubagentStart`/`SubagentStop` hook with the matcher anchored on the agent name. | Not adopted (carried over). |
| `provider-researcher` "writes only its research note" (`provider-researcher.md:3`, body :43–44) | Frontmatter `hooks: PreToolUse` with `matcher: "Write\|Edit"` that rejects paths outside `docs/research/`. | Not adopted (carried over). `Write`/`Edit` are unrestricted in its `tools` (:4). |
| Frontmatter is "valid" (`validate.sh` has no frontmatter stage; `grep frontmatter scripts/validate.sh` finds nothing) | `claude plugin validate .claude/agents` and `claude plugin validate .claude/skills` (v2.1.233+). Both pass today. | **New option.** It needs the `claude` CLI where `validate.sh` runs. If CI lacks it, it can only be a local or `/preflight` step. |
| Bounded review effort | `maxTurns` | Not adopted (carried over). |
| Reviewers apply the spec chapter | `skills:` preload | Not adopted (carried over). |
| `research-note` is a self-contained task | `context: fork` + `agent:` | Not adopted (carried over). |
| `tooling-recheck` writes only under `docs/research/tooling/**` (its `allowed-tools`, `tooling-recheck/SKILL.md:5`) | `disallowed-tools` (removes tools while the skill is active) or a skill `hooks: PreToolUse`. `allowed-tools` path patterns only pre-approve. | Not adopted (carried over). |
| Procedural rules that "must hold every time" in skills | Skill `hooks` (§2.3) | Not adopted. The page now recommends them explicitly. |

`disable-model-invocation` stays deliberately unused. Spec 11:349–355 records why, and that rationale is consistent with the page.

## 4. Guidance not followed, or statements too strong

- **`docs/spec/11-agentic-sdlc.md:259–261`** says the reviewers are read-only "twice over" via `disallowedTools` and `permissionMode: plan`. Two points from the 2026-10-30 sub-agents page:
  - The plan half applies only in `default`, `dontAsk` or `plan` main sessions. In autonomous runs (auto, `acceptEdits`, bypass) only `disallowedTools` plus the `tools` list hold. The spec text at :266–268 already concedes the auto/acceptEdits/bypass case, but cites "sub-agents documentation, read 2026-10-02".
  - **Documented fix:** cite the 2026-10-30 read and add the newly documented `plan`-main-session case. Optionally name the frontmatter `PreToolUse` hook as the mechanism that would close the `Bash` gap.
- **Reviewer descriptions** (`docs-auditor.md:3`, `security-reviewer.md:3`, `spec-guardian.md:3`) say "Read-only reviewer". That is a stated property. Mechanically it holds only for the file tools; `Bash` remains (carried over).
- **`tooling-recheck/SKILL.md:5`** uses `Edit(docs/research/tooling/**)` and `Write(docs/research/tooling/**)`, which read like a jail. The page states that `allowed-tools` "does not restrict" and that the grant clears at the next message. **Documented fix:** `disallowed-tools` or a skill `hooks` entry if a restriction is intended (carried over, now with the per-turn wording).
- **Previous note §2 bullet 1** listed skill lengths as "not measured". They are now measured; see §1.

## Not verified this run

- Chars 100,000–116,070 of the sub-agents page.
- Hook, permission and settings pages, including the exact `hooks` schema for `once` and the "limits on skill hooks".
- Whether the CI runner has the `claude` CLI for `claude plugin validate`.
- That frontmatter `hooks` take effect on this install (v2.1.278). No version gate was stated for them in the parts read.
