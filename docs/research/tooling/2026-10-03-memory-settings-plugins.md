# Tooling recheck: memory, settings, best practices, plugins, 2026-10-03

Read on: 2026-10-03

Group "Memory and settings" of `/tooling-recheck`. Previous note:
[`2026-10-02-memory-settings-plugins.md`](2026-10-02-memory-settings-plugins.md).
Installed CLI: `claude --version` returns **2.1.278** (run 2026-10-03),
unchanged since 2026-09-27.

| Page | URL | Read |
|---|---|---|
| Memory | <https://code.claude.com/docs/en/memory.md> | 2026-10-03 |
| Settings | <https://code.claude.com/docs/en/settings.md> | 2026-10-03 |
| Best practices | <https://code.claude.com/docs/en/best-practices.md> | 2026-10-03 |
| Plugins | <https://code.claude.com/docs/en/plugins.md> | 2026-10-03 |
| Settings reference (outside the group; read to check four keys) | <https://code.claude.com/docs/en/settings-reference.md> | 2026-10-03 |

All four `.md` variants returned HTTP 200 on the first try, with no redirect.
Best practices now links to the plugins page as `/docs/en/plugins/overview`;
the next run should try `/plugins/overview.md` if `/plugins.md` fails. `curl`
is denied in this repository, so pages were read through a sandboxed fetch.

## What changed since 2026-10-02

Nothing that invalidates a field or value the repository uses. Each item below
appears in today's pages and is not in the 2026-10-02 note; whether each is new
to the docs since then is unknown.

1. **Settings are read from the session's primary working directory.** Claude
   Code reads the shared `.claude/settings.json` from that directory, so a file
   committed at the repository root needs the session started there (Settings,
   "Where Claude Code keeps the local file in a git repository"). After `/cd`,
   both project files are read from the new directory (v2.1.246+). The local
   file is read at the repository root.
2. **Cloud sessions with several repositories read almost nothing from
   `.claude/settings.json`**: only `enabledPlugins` and
   `extraKnownMarketplaces` — not permission rules, hooks, `env` or other keys
   (Settings, "Settings in cloud sessions"). The 2026-09-02 note quoted the
   unqualified sentence; the qualifier is drift since then.
3. **Path-scoped rules trigger on the Read, Write or Edit tool only**, not on
   every tool use (Memory, "Path-specific rules"). The 2026-09-02 note quoted
   "when Claude reads files matching the pattern".
4. **The Project instructions setting** (`claude-md-or-agents-md`, the default;
   `claude-md-and-agents-md`; `claude-md`; `managed-only`) is set through
   `pluginConfigs."agents-md@builtin"` and is ignored in project and local
   settings. `InstructionsLoaded` hooks do not fire for an `AGENTS.md` read
   through this setting; they do for one a `CLAUDE.md` imports (Memory,
   "AGENTS.md").
5. **Combined-length warning**: also raised when files each within the limit
   add up past a combined limit at session start; the limit is not stated
   (Memory, "My CLAUDE.md is too large").
6. **Best practices names "Manual mode"** (`"manual"` is an alias for
   `"default"` since v2.1.200) and documents `/goal`. The plugins page shows a
   context-cost estimate for official-marketplace plugins and three marketplace
   tiers.
7. **Unchanged and re-confirmed:** auto mode is the built-in starting mode from
   v2.1.283; `/doctor prompt-audit` needs v2.1.283; the `/doctor` trim check
   needs v2.1.206; imports go four hops deep; reading `AGENTS.md` directly needs
   v2.1.277.

## 1. Fields, values and contracts the repository uses

| Repository use | Where | Status, read 2026-10-03 |
|---|---|---|
| `"$schema"` (schemastore URL) | `.claude/settings.json` | Documented, same URL; the schema can lag the newest CLI |
| `permissions.disableBypassPermissionsMode: "disable"` | `.claude/settings.json`; `11-agentic-sdlc.md` | Valid in any file; also makes a subagent's `permissionMode: bypassPermissions` ignored |
| `autoMemoryEnabled: false` | `.claude/settings.json`; `11-agentic-sdlc.md` | Valid. `CLAUDE_CODE_DISABLE_AUTO_MEMORY` overrides it for a session in either direction (not mentioned in the repository) |
| `env` block | `.claude/settings.json` | Valid. Most `env` values wait for workspace trust; `deny` and `ask` apply at once |
| `hooks` wired from `.claude/settings.json` | | Reloaded on edit; contracts are the hooks group's |
| Not setting `permissions.defaultMode` | `11-agentic-sdlc.md`; `.claude/README.md` | Documented choice; `auto` and `bypassPermissions` do not take effect from project settings, `plan` and `default` do |
| `CLAUDE.md` imports `@AGENTS.md` | `CLAUDE.md:3` | Documented and endorsed; with a `CLAUDE.md` present, the default reads it instead of `AGENTS.md`, so the import is what loads `AGENTS.md` |
| Under 200 lines each | `CLAUDE.md` 118, `AGENTS.md` 199 | Wording unchanged; combined-limit warning exists (item 5) |
| `.claude/rules/*.md` with `paths:` | five files | Syntax valid; trigger is the Read, Write or Edit tool only (item 3) |
| `settings.local.json`, `CLAUDE.local.md` git-ignored | `CLAUDE.md`; `.gitignore` | Valid; neither tracked |
| `claude-plugins-official` added on first interactive start | `.claude/README.md` | Confirmed; plugin ids not on this page and not verified |
| `claude --permission-mode plan`, Shift+Tab | `CLAUDE.md`; `.claude/README.md` | Confirmed (Best practices) |
| `/context` to confirm memory files loaded | `.claude/README.md` | Confirmed; `/status` "Setting sources" is the documented check that settings loaded |

Outside this group: `11-agentic-sdlc.md` says a single leading slash anchors at
the settings file; today's settings page says a `/`-prefixed rule in the local
file anchors at the session's primary working directory (citing
`/permissions`). The repository uses `//`, so its rules are unaffected; the
sentence needs the permissions page.

## 2. Documented mechanisms the repository does not use

1. **Start-directory precondition.** A session started in a subdirectory reads
   no committed deny rules and runs no hooks — guards, commit gate and `Stop`
   gate included — silently. The `SessionStart` hook cannot detect it, because
   it is the thing not loaded. The remedies are documentation and launching from
   the root.
2. **`InstructionsLoaded` hook** to log which memory and rules files load;
   recommended by Memory. Today the check is manual (`/context`).
3. **`permissions.defaultMode: "plan"`** would enforce "plan mode is the
   recommended start". From v2.1.283 sessions start in auto mode without it.
   `disableAutoMode: "disable"` is the stricter option, pending since
   2026-09-27. An owner's call.
4. **Block-level HTML comments in `CLAUDE.md`** are stripped before injection —
   the documented place for maintainer notes.
5. **`/doctor prompt-audit`** (v2.1.283; not on 2.1.278), the **`/doctor` trim
   check** and the **`/status` length warnings**, unadopted.
6. **`/status` "Setting sources"**, the counterpart of the `/context` check.

## 3. Guidance the memory files and docs do not follow

| # | Where | Finding | Documented fix |
|---|---|---|---|
| G1 | `docs/spec/11-agentic-sdlc.md` | "Claude Code reads `CLAUDE.md`, not `AGENTS.md`" is no longer general: from v2.1.277 it reads `AGENTS.md` when no `CLAUDE.md` exists. The conclusion (keep the import) holds | Reword (Memory, "AGENTS.md") |
| G2 | `docs/spec/11-agentic-sdlc.md`; `.claude/README.md` | The "property of the harness" statement omits two conditions where the guards are absent: a session started outside the repository root, and a multi-repository cloud session | State both (Settings) |
| G3 | `CLAUDE.md`; `AGENTS.md` | "load … when you touch" a subtree: rules fire only on the Read, Write or Edit tool; reading through a shell or an MCP reader loads nothing | Say so (Memory, "Path-specific rules") |
| G4 | `AGENTS.md` (199 lines) | One line under target; the combined limit is not checked | Move subtree-only content to rules |
| G5 | `AGENTS.md` crate and path tables | Directory layouts are what the trim check targets | Keep the layering rule, not the layout |
| G6 | `CLAUDE.md` stage-list history | Maintainer history in loaded context | HTML comment |
| G7 | `.claude/README.md` header | Says verified 2026-09-02 only | Point to the latest note per group |
| G8 | `.claude/README.md` plugin section | "verified … on 2026-09-27" | 2.1.278, 2026-10-03 |
| G9 | `.claude/README.md` | Owed since 2026-10-02: `env` and `allow` wait for workspace trust, `deny` and `ask` do not | One sentence |
| G10 | `.claude/README.md` verification | Omits `/status` and the start-directory precondition | Add both |
| G11 | `docs/spec/11-agentic-sdlc.md` | Does not mention `CLAUDE_CODE_DISABLE_AUTO_MEMORY` | One clause |
| G12 | `.gitignore` comment | "loaded after CLAUDE.md"; the page says `CLAUDE.local.md` loads alongside it | Reword |
| G13 | `docs/spec/11-agentic-sdlc.md` dates | Can move to 2026-10-03 | Add this note |

Nothing in `CLAUDE.md`, `AGENTS.md` or `.claude/settings.json` uses a field or
value these pages no longer document.
