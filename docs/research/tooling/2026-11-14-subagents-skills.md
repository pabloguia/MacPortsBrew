# Tooling recheck 2026-11-14 — subagents and skills

Read 2026-11-14, read-only, by a subagent; nothing was written. Previous note: `2026-11-03-subagents-skills.md`.

## Drift since 2026-11-03

- **No contract break.** Every field, value and tool-list form the repository uses is still documented and valid.
- No agent or skill file changed: last frontmatter commit is `753bdfb` (2026-10-04). `claude --version` is still `2.1.278`.
- Newly documented, none of it touching a field in use:
  - Subagent `autoCompactWindow` (100000 to 1000000, v2.1.296+), `omitClaudeMd` (v2.1.271+), `experimental.cacheTtl`, `initialPrompt`.
  - Skill `disallowed-tools`, `arguments`, `background`, `shell`, `metadata`, `compatibility`; the settings `skillListingMaxDescChars` and `skillOverrides` (`"name-only"`); `/skill-doctor`.
  - Fork mode is on by default from v2.1.302.
  - `claude --debug` flags unrecognised subagent fields (v2.1.296+).
- **Caveat on completeness.** Both pages (about 116k and 120k characters) were truncated at 100,000 characters by the fetch tool; the tail was read from offset 100000, the first 100,000 only through the tool's condensed answer. The 11-03 note read both in full. The "Available tools" background allowlist and the hook-trust wording were not re-read verbatim and are marked carried-over where they appear. **Next run: read both pages in full.**
- URLs: `https://code.claude.com/docs/en/sub-agents.md` and `https://code.claude.com/docs/en/skills.md`, fetched without redirect.

## 1. Fields the repository uses (read 2026-11-14)

Agents (`.claude/agents/*.md`, frontmatter lines 1 to 9) — sub-agents page, "Frontmatter reference":

| Field | Used in | Status |
|---|---|---|
| `name` | all 4 (line 2) | Documented; max 256 chars, no leading `-`, no `:`. Valid. |
| `description` | all 4 (line 3; 332 to 472 chars) | Required, documented. Valid. Combined budget 15,000 tokens; ours about 1,700 chars. |
| `tools` | all 4 (line 4), comma-separated | Comma string or YAML list both documented. Valid. `provider-researcher` also lists `TodoWrite`; carried over from 11-03 that all its tools are in the background allowlist. |
| `disallowedTools: Write, Edit, NotebookEdit` | 3 reviewers (line 5) | Documented; applied before `tools`. Valid. |
| `model: inherit` | all 4 | Documented. Valid. |
| `permissionMode: plan` | 3 reviewers (line 7) | Documented. Under `bypassPermissions`, `acceptEdits` or auto the parent's mode overrides; under `plan` the subagent stays in plan. Valid. |
| `color` | all 4 | Documented set. Valid. |

Skills (`.claude/skills/*/SKILL.md`) — skills page, "Frontmatter reference":

| Field | Status |
|---|---|
| `name`, `description` | Documented. `description` plus `when_to_use` capped at 1,536 characters in the listing; ours are 183 to 342. No `<` or `>`. Valid. |
| `argument-hint` | Documented; on 12 of 13 skills. Valid. |
| `allowed-tools` | Documented as a grant for the invoking turn only; does not restrict other tools. `${CLAUDE_PROJECT_DIR}` works in `allowed-tools` Bash rules. `WebFetch(domain:...)` in `add-mapping` line 5 not re-verified against the permissions page (out of group). |
| `license: Apache-2.0` | Accepted, not acted on. Valid. |

Longest `SKILL.md` is 88 lines, under the 500-line guidance.

## 2. Documented mechanisms the repository does not use

- **New: frontmatter `hooks` on the reviewers.** A `PreToolUse` hook with matcher `Bash` is the page's own example of enforcing read-only on a subagent that keeps `Bash`; it would close the gap where `disallowedTools` covers only the file tools. Caveat (carried over): project agent hooks are skipped until the folder is trusted, and a `-p` session does not count as trusted. The same mechanism could jail `provider-researcher`'s writes to `docs/research/`.
- **New: skill `hooks` frontmatter.** A rule that must hold every time belongs in a hook; a skill's `hooks` apply from invocation to session end. Relevant to `tooling-recheck`'s write jail.
- **Unchanged: `claude plugin validate`** on `.claude/agents` and `.claude/skills` as a `validate.sh` stage; not re-run (no files changed).
- **Unchanged: `maxTurns`, `skills:` preload, skill `context: fork`.** Not adopted; no repository text claims them.
- **New, low value: `skillOverrides`, `/skill-doctor`, `omitClaudeMd`.** Not needed at this size.

## 3. Guidance not followed

- **Deliberate non-follow (unchanged): `permissionMode: plan` on the reviewers** is overridden under `bypassPermissions`, `acceptEdits` or auto, so "read-only reviewer" holds mechanically only through `disallowedTools` plus the prompt, and `Bash` remains a write channel. `docs/spec/11-agentic-sdlc.md` ("read-only twice over") still cites the 2026-10-02 read. Fix: cite the current page and add the Bash caveat, or add the `PreToolUse` hook above.
- **Deliberate non-follow: `model: inherit` everywhere.** Matches the project rule; the per-invocation `model` takes precedence.

## Classification

| Item | Class |
|---|---|
| All used fields and values | Unchanged, valid |
| Reviewers' read-only claim (spec 11 wording) | Deliberate non-follow, wording stale |
| Subagent/skill `hooks`, `plugin validate`, `omitClaudeMd`, `autoCompactWindow`, `skillOverrides` | New mechanism, none adopted |

**Verdict:** no contract mismatch; no defect in `.claude/agents` or `.claude/skills`.
