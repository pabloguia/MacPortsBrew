<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification: memory, settings, best practices, plugins (2026-10-27)

Date read: **2026-10-27**, read-only, `/tooling-recheck` at the start of work
order 16. Previous note: [`2026-10-26-memory-settings-plugins.md`](2026-10-26-memory-settings-plugins.md).

**Verdict:** no contract break. Nothing the repository uses has gone. The three
items still open from 2026-10-26 are unchanged and sit in guarded paths.

## Sources (all `.md` variants, read in full)

| Page | Result |
|---|---|
| https://code.claude.com/docs/en/memory.md | fetched |
| https://code.claude.com/docs/en/settings.md | fetched; keys live in `settings-reference.md` |
| https://code.claude.com/docs/en/best-practices.md | fetched |
| https://code.claude.com/docs/en/plugins.md | fetched (overview only) |

Not re-fetched, so nothing here rests on them: `permissions.md`, `hooks.md`,
`settings-reference.md`, `plugins/install.md`, `plugins/manifest-reference.md`.

## 1. Still documented and valid

- `autoMemoryEnabled: false`; `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` also disables it (memory.md).
- `.claude/rules/*.md`: `paths` is the only field read; list or comma-separated; rules load on Read, Write or Edit of a match (memory.md).
- `@AGENTS.md` in `CLAUDE.md`: documented; `AGENTS.md` is then read only through the import; import depth at most four hops.
- `deny`/`ask` apply at once, `allow` waits for folder trust; precedence managed > command line > local > shared project > user; lists merge (settings.md).
- Cloud sessions apply none of a repository's permission rules, hooks or `env`; matches the repository's statements.
- `Stop` cap of eight consecutive blocks, `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` (best-practices.md).
- Official marketplace added on the first interactive session unless managed policy blocks it (plugins.md).

## 2. New or changed since 2026-10-26

- **Local settings file location** (settings.md, since v2.1.211): at the git repository root, or the main checkout's root in a worktree. A rule starting with `/` anchors at the session's primary working directory.
- **On-demand size limit** (memory.md, v2.1.309): subdirectory `CLAUDE.md` files and path-scoped rules are truncated at 40,000 characters (200K context) or 150,000 (1M). The rules here are short; not measured.
- **Total-size warnings** (memory.md): a startup and `/status` warning when one file passes the recommended length or the files together pass a combined limit.
- **`/doctor prompt-audit`** (memory.md, v2.1.283+): audits `CLAUDE.md`, `AGENTS.md`, rules, skills, subagents and output styles for stale or contradictory instructions.
- **`AGENTS.md` setting ID** `cc-plugin-agents-md@builtin` (pre-v2.1.285: `agents-md@builtin`), honoured only in user, `--settings` and managed settings. Not set here; fine.
- `.claude/README.md`'s "documentation describes up to v2.1.296" is stale: today's pages cite v2.1.309.

## 3. Mechanisms not used that would enforce what is only stated

- `onFailure: "block"` on the guard and gate hooks (best-practices.md, linking `hooks.md#block-the-action-when-a-hook-fails`). Already recorded in spec 11; still open and guarded. Field placement needs a `hooks.md` read.
- An `InstructionsLoaded` hook would make "the rule loaded" observable (memory.md).
- `includeGitInstructions: false` would stop the built-in commit instructions competing with the project's. Optional.
- Block-level HTML comments are stripped before injection (memory.md); maintainer notes could cost no context.
- Carried from 2026-10-26, not re-read today: the hook `if` field, `disableAutoMode`, `blockReadsOutsideWorkingDirectories`, sandbox `failIfUnavailable`, tightening `Bash(git branch*)` and `Bash(cargo run *)`.

## 4. Findings by file

Still open from 2026-10-26:

1. `.claude/settings.json`: `Edit(./rust-toolchain.toml)` and `Edit(./docs/spec/01-invariants.md)` are relative to the current directory; the documented project-rooted form is `Edit(/…)`. `guard-protected-paths.sh` backs the rule meanwhile. Guarded.
2. `CLAUDE.md`, "a shell or MCP reader loads nothing": memory.md says a single-file Bash read (`cat`, `head`) also loads a path-scoped rule. Owner's file.
3. `docs/spec/11-agentic-sdlc.md` "Read/Edit deny rules do not apply to what a subprocess does" is unqualified where an earlier line of the same chapter states the nuance correctly.

New:

4. `docs/spec/11-agentic-sdlc.md`: "a single leading slash anchors at the settings file" — settings.md now says the session's primary working directory.
5. `.claude/README.md`: the "up to v2.1.296" claim (section 2).
6. `AGENTS.md` (199 lines) is one line inside memory.md's 200-line target; best-practices.md names derivable content (layout tables) as the thing to cut.
7. `CLAUDE.md` carries many bold lead phrases; best-practices.md warns that piled emphasis makes rules over-apply and that an unbendable rule belongs in a hook.

## Recommendations (not adopted; `.claude/` and `CLAUDE.md` wait for the owner)

Items 1, 2 and 5 above; `onFailure: "block"` after a `hooks.md` read and a
`test-hooks.sh` run; `/doctor prompt-audit` as an optional `/tooling-recheck`
step. Items 3 and 4 are spec 11 wording and land with an SE line when a part
next edits that chapter.
