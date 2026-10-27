<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification: memory, settings, best practices, plugins (2026-10-26)

Date read: **2026-10-26**, by a read-only subagent for `/tooling-recheck`, at the
start of pass 08 slice 6. Previous note: [`2026-10-22-memory-settings-plugins.md`](2026-10-22-memory-settings-plugins.md).

**Verdict:** no contract break. Two drift items of 2026-10-22 are closed by the
repository; three remain, all in guarded paths (the owner's, SE-027). No field the
repository uses has gone.

## Sources (fetched 2026-10-26, `.md` variants)

| Page | Result |
|---|---|
| `https://code.claude.com/docs/en/memory.md` | read in full |
| `https://code.claude.com/docs/en/settings.md` | read in full; still only "Settings files and precedence" — keys live in `settings-reference`, `permissions`, `hooks` |
| `https://code.claude.com/docs/en/best-practices.md` | read in full |
| `https://code.claude.com/docs/en/plugins.md` | read in full; now an overview, install and creation moved to `plugins/install` and `plugins/create` |
| `https://code.claude.com/docs/en/permissions.md` | read in full |
| `https://code.claude.com/docs/en/hooks.md` | first ~200k characters read; the last ~80k only searched |
| `https://code.claude.com/docs/en/settings-reference.md` | **partly read**: index and characters 100000–200000 |

Not fetched: `sandboxing`, `anthropic-marketplaces`, `plugins/install`,
`plugins/org`, `hooks-guide`, `env-vars`, `tools-reference`.

## 1. Fields, values and contracts the repository uses

Re-confirmed, no change since 2026-10-22:

- `permissions.disableBypassPermissionsMode: "disable"` (`.claude/settings.json`): valid; it also makes subagents ignore `permissionMode: bypassPermissions`.
- `autoMemoryEnabled: false`: boolean, default true; `CLAUDE_CODE_DISABLE_AUTO_MEMORY` overrides it for one session in either direction.
- The `env` block: none of its variables is in the ignored list (the list's tail was not read).
- `$schema`; hook events, matchers, `timeout` (seconds, default 600), `statusMessage`; the matcher `Edit|Write|NotebookEdit` is a valid exact-name list.
- The `Stop` cap: eight consecutive continuations, reset by a tool call, `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` — matches `.claude/README.md` and spec 11.
- `@AGENTS.md` in `CLAUDE.md`: documented; never reads `AGENTS.md` twice. Imports recurse at most four hops.
- `.claude/rules/*.md` with `paths:` only: `paths` is the only documented rule field.
- Deny > ask > allow; the `//path` and `~/` forms.

Closed since 2026-10-22 (fixed in the repository):

- `.claude/README.md` now says `allow` waits for folder trust while `deny`/`ask` apply at once — matches permissions.md, "What runs before you trust a folder".
- `.claude/README.md` now says the official marketplace is added on the first interactive session unless a managed policy blocks it — matches plugins.md.

Still open, guarded paths:

1. `.claude/settings.json`, `Edit(./rust-toolchain.toml)` and `Edit(./docs/spec/01-invariants.md)`: permissions.md says `./path` is relative to the current directory, and `/path` in project settings anchors at the project root. Documented fix: `Edit(/rust-toolchain.toml)`, `Edit(/docs/spec/01-invariants.md)`. `guard-protected-paths.sh` backs the rule up meanwhile.
2. `CLAUDE.md`, "a shell or MCP reader loads nothing": memory.md says a path-scoped rule also loads when Claude views a matching file with a Bash command that counts as a read, such as `cat` or `head` on a single file. Fix: "a shell reader other than a single-file read, or an MCP reader, loads nothing".
3. `docs/spec/11-agentic-sdlc.md`, "`Read`/`Edit` deny rules do not apply to what a subprocess does": permissions.md says they apply to the built-in tools, to file commands Claude Code recognises in Bash (`cat`, `head`, `tail`, `sed`, `tee`) and to `>`/`<` targets, but not to commands that read without naming files or to arbitrary subprocesses. Fix: that wording. (Spec 11 is not guarded; it is listed here because it changes what the specification says about enforcement and lands with an SE line.)

Verified today, unverifiable on 2026-10-22:

- PostToolUse stderr on exit 0 is never shown to Claude; exit 2 does not block and its stderr is shown; `additionalContext` sits beside the tool result. The repository's statement is correct.
- A multi-repository cloud session reads only `enabledPlugins` and `extraKnownMarketplaces` from each repository's settings, and those still do not load in a cloud session. Correct. New nuance: in a cloud thread inside a project, none of a repository's permission rules, hooks or `env` apply.

Still unverified: whether `CLAUDE_CODE_DISABLE_AUTO_MEMORY` disables a
subagent's `memory:`; the plugin names `security-guidance` and
`rust-analyzer-lsp`; the version claims in `.claude/README.md` (not
contradicted).

## 2. Mechanisms not used that would enforce what the repository only states

- **Hook `if` field** (PreToolUse and a few others; one permission rule): `commit-gate.sh` runs on every Bash call and filters inside the script; `"if": "Bash(git commit *)"` would start it only for commits. Test with `test-hooks.sh` first; compound commands are not documented. (carried)
- **`InstructionsLoaded` hook**: fires when a `CLAUDE.md` or a rule file loads (matcher values include `path_glob_match`). A logging hook would make "the rule loaded" observable. It does not fire for an `AGENTS.md` read through the setting, but does for one imported by `CLAUDE.md`, as here. New.
- **`permissions.disableAutoMode: "disable"`**: best-practices says that from v2.1.283 auto mode is the starting mode for interactive terminal and VS Code sessions. The repository leaves the mode to the operator (spec 11, `.claude/README.md`); this is the switch if an unattended auto run should be impossible. (carried; it matters more now that auto is the default start)
- **`permissions.blockReadsOutsideWorkingDirectories: true`** (v2.1.257+): would stop reads outside the repository. New.
- **Stop-hook `additionalContext`**: advisory, non-blocking feedback through the same cap. Optional. New.
- **Sandbox**: the block proposed in `.claude/README.md` uses valid keys; it lacks `failIfUnavailable` (without it, commands run unsandboxed when the sandbox cannot start). Not enabled; no change.
- **`/doctor` prompt audit**: proposes cuts for derivable content in `CLAUDE.md`, `AGENTS.md`, rules, skills and subagents. A candidate step for this skill. New.
- Tightening `Bash(git branch*)` (allows `-D`) and `Bash(cargo run *)`. (carried)

## 3. Guidance the memory files do not follow

- **Size and derivable content**: memory.md targets under 200 lines per file, and best-practices excludes what can be read from the code. `AGENTS.md` is 199 lines with derivable tables (crates, paths, domain concepts, the state machine). Unchanged.
- **Two reading orders**: `AGENTS.md` (invariants and evolution first) and `CLAUDE.md` (charter first) disagree. Unchanged.
- **Repeated lists** of subagents and skills in `CLAUDE.md`, `.claude/README.md` and spec 11. Unchanged.
- **Emphasis**: `CLAUDE.md` bolds a lead phrase on nearly every paragraph; best-practices says emphasis on many lines makes none stand out. New.
- **Reviewer scope**: best-practices advises telling a reviewer to flag only gaps that affect correctness or the stated requirements. `docs-auditor` lacks that sentence; `spec-guardian` and `security-reviewer` have it. New.
- `session-start.sh` output (2,774 bytes) is well under the 10,000-character cap.

## Recommendations (not adopted; `.claude/` and `CLAUDE.md` wait for the owner)

1. `settings.json`: the two `Edit(/…)` anchors.
2. `CLAUDE.md`: the single-file Bash read wording.
3. Spec 11: the Bash file-command wording (unguarded; deferred to a configuration pass so it lands with the `.claude/` changes it describes).
4. `"if": "Bash(git commit *)"` on `commit-gate.sh`, after a `test-hooks.sh` run.
5. Consider `disableAutoMode` and `blockReadsOutsideWorkingDirectories`.
