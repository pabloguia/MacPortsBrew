<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling recheck 2026-11-03 — memory, settings and plugins

Read 2026-11-03, read-only, by a subagent; nothing was written to the
repository. Previous note: `2026-10-30-memory-settings-plugins.md`. Pages were
read through full-text search, so "not surfaced" means not found by search.

## Sources

| Page | Result (2026-11-03) |
|---|---|
| <https://code.claude.com/docs/en/memory.md> | HTTP 200; highest version cited v2.1.309 |
| <https://code.claude.com/docs/en/settings.md> | HTTP 200; highest version cited v2.1.314 |
| <https://code.claude.com/docs/en/best-practices.md> | HTTP 200 |
| <https://code.claude.com/docs/en/plugins.md> | HTTP 200 |
| `settings-reference.md`, `permissions.md` | HTTP 200; fetched outside the four pages to settle 10-30's open items |

In-scope repository changes since 10-30: `d5bfca6` only (header text in
`.claude/README.md` and spec 11 flagging the 10-30 items).

## Fields the repository uses: still documented and valid

`$schema`; `permissions.deny`/`ask`/`allow` (deny and ask apply at once, allow
waits for folder trust; the `//` absolute form is the documented one);
`permissions.disableBypassPermissionsMode: "disable"` (re-confirmed, and it also
ignores a subagent's `bypassPermissions`); `autoMemoryEnabled: false`; `env`
(none of the five variables is in the ignored list); the `@AGENTS.md` import
(never read twice); `.claude/rules/*.md` with `paths:` (a recognised single-file
Bash read such as `cat` or `head` also loads a rule); the git-ignored
`settings.local.json` and `CLAUDE.local.md`; size targets (`CLAUDE.md` 142
lines, `AGENTS.md` 199). The `Stop` block cap is re-confirmed: after eight
blocks in a row with no tool call between them, Claude Code overrides the next
(`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` raises it).

## Changed since 2026-10-30

1. New key `permissions.blockWritesOutsideWorkingDirectories` (v2.1.314+):
   Write, Edit and NotebookEdit refuse paths outside the working directories in
   every mode. It does not limit shell, MCP or hooks, and it removes
   `EnterWorktree`/`ExitWorktree`, which sessions here use.
2. A `CLAUDE.local.md` in the working directory stops Claude reading
   `AGENTS.md` directly under the default value; the `@AGENTS.md` import makes
   the repository immune.
3. `/doctor`'s checkup now cuts derivable content (layouts, dependency lists,
   architecture overviews) from instruction files.
4. Settled: a `/path` pattern in project settings anchors at the primary working
   directory; `./path` is relative to the current directory; the `env` block and
   hooks are used in both untrusted cases (a trusted parent only; `-p` or the
   SDK). 10-30 item 8 is closed as consistent.

## Mechanisms not used that would enforce what is only stated

- `onFailure: "block"` on the guard and gate hooks, now recommended on the
  best-practices page itself. Guarded path: the owner's change.
- `blockWritesOutsideWorkingDirectories`: a trade-off against worktree use;
  the owner's.
- An `InstructionsLoaded` hook; `includeGitInstructions: false` with
  `attribution` (which also drops the git status snapshot); `/doctor
  prompt-audit` as a recheck step. Unchanged, unused.

## Findings still open (wording, guarded or the owner's)

1. `.claude/settings.json:97-98`: `Edit(./rust-toolchain.toml)` and
   `Edit(./docs/spec/01-invariants.md)` are current-directory relative; the
   documented fix is the `/` form. `guard-protected-paths.sh` covers both
   meanwhile. Guarded.
2. `CLAUDE.md:57`: "a shell or MCP reader loads nothing" is wrong for a
   recognised single-file Bash read.
3. `docs/spec/11-agentic-sdlc.md:124`: a single leading slash anchors at the
   settings *source* (for project settings, the primary working directory), not
   the settings file.
4. Spec 11 lines 199-201: qualify as "arbitrary subprocesses", as line 124 does.
5. `.claude/README.md:53-55`: "features up to v2.1.296" is stale (the settings
   page cites v2.1.314).
6. `AGENTS.md`'s layout tables run against best-practices' advice; a SHOULD at
   most, the owner's call.
7. `CLAUDE.md`'s many bold lead phrases (best-practices: a rule that cannot
   bend belongs in a hook).
8. Spec 11 lines 158-159: a cloud thread applies none of `.claude/settings.json`'s
   rules, hooks or `env`.
9. Spec 11 lines 84-86: optional additions to the `AGENTS.md`-availability
   rationale.
10. New: `CLAUDE.md:61-62` says the `Stop` hook "will refuse to let the turn
    end"; the eight-block cap applies (spec 11 and `.claude/README.md` already
    say so).

**Verdict:** no contract break.
