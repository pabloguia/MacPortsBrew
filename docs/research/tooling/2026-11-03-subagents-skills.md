<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling recheck 2026-11-03 — subagents and skills

Read 2026-11-03, read-only, by a subagent; nothing was written to the
repository. Previous note: `2026-10-30-subagents-skills.md`.

## Sources

| Page | Result (2026-11-03) |
|---|---|
| <https://code.claude.com/docs/en/sub-agents.md> | HTTP 200, no redirect, 116,346 characters; read in full (the 10-30 gap above 100,000 characters is closed) |
| <https://code.claude.com/docs/en/skills.md> | HTTP 200, no redirect, 118,821 characters; read in full |

Machine observation: `claude --version` is `2.1.278 (Claude Code)`, unchanged;
`claude plugin validate .claude/agents` and `claude plugin validate
.claude/skills` both print "Validation passed". No agent or skill frontmatter
changed since 10-30 (`35bc7a5`, `d5bfca6` touch only spec 11 wording).

## Fields the repository uses

All still documented, all values valid (sub-agents "Supported frontmatter
fields", skills "Frontmatter reference"):

- Agents: opening `---` on line 1; `name` (at most 256 characters, no `:`, no
  leading `-`); `description` (required); `tools` as a comma-separated string
  (`provider-researcher`'s tools are all in the background allowlist);
  `disallowedTools: Write, Edit, NotebookEdit`; `model: inherit`;
  `permissionMode: plan` (ignored under `bypassPermissions`, `acceptEdits` or
  auto; holds under a `plan`, `default` or `dontAsk` main session); `color`.
- Skills: `name`, `description` (combined with `when_to_use`, truncated at
  1,536 characters; all well under), `argument-hint`, `allowed-tools`
  (per-turn: "The grant clears when you send your next message"), `license`
  (accepted, not acted on). The longest `SKILL.md` is 88 lines (under 500).
  `${CLAUDE_PROJECT_DIR}` in `allowed-tools` is substituted only on the user's
  machine; in a cloud or remote session it stays literal, so those `Bash(...)`
  grants would not match there.

## Changed since 2026-10-30

1. Skills' `disallowed-tools` is now documented as per-turn too. The 10-30 fix
   proposed for `tooling-recheck`'s write jail would hold only for the
   invoking turn; a skill `hooks` entry is the only skill-level mechanism that
   lasts the session.
2. Subagent frontmatter hooks need workspace trust (sub-agents, "Define hooks
   for subagents"): until the folder is trusted they are skipped silently, and
   "A `-p` session doesn't count as trusted." A frontmatter `PreToolUse` hook
   closing the reviewers' `Bash` gap would be inert in an unattended run; a
   project `settings.json` hook is the alternative.
3. New version gates (v2.1.304 and later) touch no field the repository uses.

## Mechanisms not used (carried over, not adopted)

Frontmatter `PreToolUse` hooks for the reviewers' `Bash` and for
`provider-researcher`'s writes (now with the trust caveat); `claude plugin
validate` as a `validate.sh` stage (whether CI has the CLI is open);
`maxTurns`; a `skills:` preload; skill `context: fork`.

## Still open

`docs/spec/11-agentic-sdlc.md` ("read-only twice over") still cites the
2026-10-02 read and omits the `plan`/`default`/`dontAsk` cases; the reviewers'
"Read-only reviewer" holds mechanically only for the file tools. Wording, not a
contract break.

**Verdict:** no contract break.
