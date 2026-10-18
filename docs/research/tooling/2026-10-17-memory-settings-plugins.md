<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification — memory, settings, plugins (2026-10-17)

Group: memory and settings (`memory`, `settings`, `best-practices`, `plugins`).
Read on 2026-10-17 from `https://code.claude.com/docs/en/<page>.md`. The
`settings` page no longer holds the key definitions; they moved to
`settings-reference`, which was read as well, with `permissions` and `hooks`
for the keys the repository uses. Returned by a read-only subagent; the runner
wrote this file. Verdict: **drift, 5 items**, none a contract break.

## Drift

1. `CLAUDE.md` says a shell or MCP reader loads nothing, so a file must be
   opened with Read. `memory` says a path-scoped rule also loads when Claude
   views a matching file with a Bash command that counts as a read (a single-file
   `cat` or `head`). Which commands count is on a page not fetched
   (`tools-reference`). MCP readers are not mentioned: unconfirmed, not
   contradicted. Wording only; the instruction "open it with Read" stays safe.
2. The `Stop` gate "will refuse to let the turn end" (`CLAUDE.md`, `.claude/README.md`).
   `hooks` documents a cap of eight consecutive continuations, reset each time
   Claude calls a tool, raised by `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`.
   `stop-gate.sh` already names the variable; the prose lags it.
3. The bare-command duplicates in `settings.json` (`Bash(sudo)`, `Bash(doas)`,
   `Bash(su)`, `Bash(chsh)`, `Bash(launchctl)`, `Bash(brew sh)`, `Bash(brew up)`)
   are redundant: `permissions` says a trailing space-star also matches the bare
   command. Harmless; the rationale in `docs/spec/11-agentic-sdlc.md` is stale.
4. `docs/spec/11-agentic-sdlc.md` lists `git push` in its denied table, but
   `settings.json` has no `git push` rule; a hook enforces it (`CLAUDE.md`). The
   documented form is `permissions.ask: ["Bash(git push *)"]`, which misses
   `git -C . push`, so the hook stays as the backstop.
5. `Edit(./rust-toolchain.toml)` and `Edit(./docs/spec/01-invariants.md)` are
   relative to the current directory; a session started in a subdirectory leaves
   them unanchored. `.claude/README.md` says to start at the root (stated, not
   enforced); `guard-protected-paths.sh` is the backstop.

## Still documented and valid

`@AGENTS.md` import; `autoMemoryEnabled: false`; `permissions.disableBypassPermissionsMode`;
`allow`/`deny`/`ask` order; `//`, `~/` and `./` anchors; `WebFetch(domain:…)` and
`Bash(curl *)`; `$schema`; hook events `SessionStart`, `UserPromptSubmit`,
`PreToolUse`, `PostToolUse`, `Stop`; the matcher `Edit|Write|NotebookEdit`;
timeouts; `${CLAUDE_PROJECT_DIR}`; `disableAllHooks`; `defaultMode` unset;
`sandbox.*` keys; `enabledPlugins` and `extraKnownMarketplaces`; the 200-line
target (`CLAUDE.md` 123, `AGENTS.md` 199); `CLAUDE.local.md` ignored; plan mode
via `--permission-mode plan`.

## Documented mechanisms the repository does not use

The sandbox (with `sandbox.failIfUnavailable: true`), an ask rule for
`git push`, `defaultMode: "plan"` (valid in project settings), the
`InstructionsLoaded` hook, `"disableAllHooks": false` in project settings,
`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`, block-level HTML comments in `CLAUDE.md`
(stripped before injection), and the hook `if` field.

## Guidance not followed

`AGENTS.md` is one line under the 200-line target and holds derivable content
(crate tables, concept lists); the maintainer history in `CLAUDE.md` would cost
nothing in an HTML comment. `AGENTS.md` is shared with other tools, a recorded
trade-off.

## Not verified

Sandboxing, sub-agents (covered by the sibling note), skills (same), overview,
github-actions, errors pages; plugin names `security-guidance` and
`rust-analyzer-lsp`; the slash form `/plugin marketplace add`; the installed
Claude Code version against v2.1.283.
