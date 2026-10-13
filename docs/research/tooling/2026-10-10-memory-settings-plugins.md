# Tooling recheck: memory, settings, best practices, plugins, 2026-10-10

Read on: 2026-10-10. Previous note: [`2026-10-03-memory-settings-plugins.md`](2026-10-03-memory-settings-plugins.md).
Installed CLI: `claude --version` → **2.1.278** (run 2026-10-10); the docs now
describe up to v2.1.296.

| Page | URL | Result |
|---|---|---|
| Memory | <https://code.claude.com/docs/en/memory.md> | HTTP 200 |
| Settings | <https://code.claude.com/docs/en/settings.md> | HTTP 200 |
| Best practices | <https://code.claude.com/docs/en/best-practices.md> | HTTP 200 |
| Plugins | <https://code.claude.com/docs/en/plugins.md> | HTTP 200 |

"New" means "not in the 2026-10-03 note"; whether it is new in the docs is unknown.

## What changed since 2026-10-03

No field or value the repository uses was removed or invalidated.

1. **The AGENTS.md plugin id was renamed**: `pluginConfigs."cc-plugin-agents-md@builtin"`
   with `options.instructionFiles`; before v2.1.285 it was `agents-md@builtin`
   (from v2.1.285 either is read). The repository sets neither.
2. **When `AGENTS.md` is not read**: the first session after upgrading from
   v2.1.276 or earlier reads `CLAUDE.md` only; also on v2.1.276 or earlier and with
   the built-in plugin disabled. Not an issue here: `CLAUDE.md` imports `@AGENTS.md`.
3. **Where `AGENTS.md` loads**: only when no `CLAUDE.md`, `.claude/CLAUDE.md` or
   `CLAUDE.local.md` exists in the working directory or above; rules do not count;
   a subdirectory's `AGENTS.md` loads on a Read there; `AGENTS.local.md`,
   `AGENTS.override.md` and `.agents/` are not read.
4. **Doc-side version drift**: mentions up to v2.1.296; `/doctor prompt-audit` and
   auto-mode-by-default (v2.1.283) are not available on the installed 2.1.278.

Re-confirmed: `@path` imports four hops deep; the over-200-lines and combined-limit
warnings; settings read from the primary working directory; path-scoped rules
trigger on the Read tool.

## 1. What the repository uses — all still valid

`$schema` URL; `permissions.disableBypassPermissionsMode: "disable"`;
`autoMemoryEnabled: false`; the `env` block (most `env` values wait for workspace
trust, `deny`/`ask` apply at once); the hook registrations; `defaultMode` unset;
`CLAUDE.md` importing `@AGENTS.md` (endorsed); files under 200 lines (`CLAUDE.md`
121, `AGENTS.md` 199); `.claude/rules/*.md` with `paths:` (the only field read);
`settings.local.json` and `CLAUDE.local.md` git-ignored;
`claude --permission-mode plan` / Shift+Tab; `/context`. **Unconfirmed:**
`.claude/README.md`'s claim that the official marketplace is registered on first
interactive start.

## 2. Mechanisms not used

1. `permissions.defaultMode: "plan"` (or `disableAutoMode`) — the owner's call,
   open since 2026-09-27; from v2.1.283 sessions start in auto mode without it.
2. `InstructionsLoaded` hook, to log which memory files and rules load.
3. Block-level HTML comments in `CLAUDE.md` (stripped before injection) for
   maintainer-only history.
4. `/doctor prompt-audit` (v2.1.283+).
5. `/status` "Setting sources" as the documented check that project settings loaded.

## 3. Guidance not followed

| # | Where | Finding | Disposition (2026-10-10) |
|---|---|---|---|
| G1 | `docs/spec/11-agentic-sdlc.md` | "reads `CLAUDE.md`, not `AGENTS.md`" is no longer general | **Fixed** |
| G4/G5 | `AGENTS.md`, `CLAUDE.md` | near the per-file target; layout tables are what the trim check cuts | Recommendation; not adopted — the tables are the orientation the project relies on |
| G6 | `CLAUDE.md` | stage-list history injected every session | Recommendation; not adopted |
| G7/G8 | `.claude/README.md` | verification dates | **Fixed** |
| G9 | `.claude/README.md` | `env`/`allow` wait for workspace trust, `deny`/`ask` do not | **Fixed** |
| G10 | `.claude/README.md` | mention `/status` "Setting sources" | **Fixed** |
| G11 | `docs/spec/11-agentic-sdlc.md` | `CLAUDE_CODE_DISABLE_AUTO_MEMORY` | **Fixed** |
| G12 | `.gitignore` comment | `CLAUDE.local.md` loads alongside, not after | Not changed (`.gitignore` comment, trivial) |
| G13 | `docs/spec/11-agentic-sdlc.md` | last-read dates | **Fixed** |
| G15 | `.claude/README.md` | official marketplace auto-registration unconfirmed | **Fixed** (says how to check) |
| G16 | `.claude/README.md` | `/plugin` shows context cost | Recommendation |

## Unverified

Individual plugin ids in `.claude/README.md`; the exact combined-length limit.
