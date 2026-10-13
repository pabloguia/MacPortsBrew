# Tooling recheck: memory, settings, best practices, plugins, 2026-10-02

Read on: 2026-10-02

Group "Memory and settings" of `/tooling-recheck`. Previous note:
[`2026-09-27-memory-settings-plugins.md`](2026-09-27-memory-settings-plugins.md).
Installed CLI: `claude --version` returns **2.1.278** (run 2026-10-02),
unchanged since 2026-09-27.

| Page | URL | Read |
|---|---|---|
| Memory | <https://code.claude.com/docs/en/memory.md> | 2026-10-02 |
| Settings | <https://code.claude.com/docs/en/settings.md> | 2026-10-02 |
| Best practices | <https://code.claude.com/docs/en/best-practices.md> | 2026-10-02 |
| Plugins | <https://code.claude.com/docs/en/plugins.md> | 2026-10-02 |

All four `.md` variants resolved on the first try. Field-level settings keys
live on `/settings-reference`, which is not in this group's list (the same gap
as earlier notes).

## Drift

Two small drifts; neither breaks a field, value or contract the repository
uses.

1. **The plugins page no longer carries the language-server sentence.** The
   2026-09-27 note quoted it recommending pre-built LSP plugins for Rust.
   On 2026-10-02 the recommendation is on the best-practices page ("If you
   work with a typed language, install a code intelligence plugin"), linking to
   `/docs/en/plugins/code-intelligence`. The substance still supports
   `.claude/README.md`'s `rust-analyzer-lsp` recommendation; the citation
   target moved. The plugin id itself was not verified here.
2. **New version-gated facts on the memory page**: `/doctor prompt-audit`
   (v2.1.283 or later), the `/doctor` trim check (v2.1.206 or later), an
   import depth of four hops, and a fuller `AGENTS.md` section gated on
   v2.1.277 and v2.1.281. None contradicts the repository.

## 1. Facts the repository depends on, re-confirmed

| Repository use | Status | Evidence (read 2026-10-02) |
|---|---|---|
| `CLAUDE.md` imports `@AGENTS.md` | Documented and endorsed | Memory: keeping the import "never makes Claude read AGENTS.md twice"; it is the documented fallback for sessions that cannot read `AGENTS.md` directly (before v2.1.281, some sessions read only `CLAUDE.md` files) |
| `autoMemoryEnabled: false` | Documented, valid | Memory, "Enable or disable auto memory" |
| `.claude/settings.json` shared and committed; `.claude/settings.local.json` personal | Confirmed | Settings, "Settings files and who they affect" |
| `permissions` arrays and the `env` block | Confirmed; lists merge across scopes | Settings, "Settings precedence" |
| `$schema` URL | Confirmed; the schema "can lag behind the newest CLI releases" | Settings |
| Memory files under 200 lines | Unchanged wording | Memory, "Write effective instructions". `CLAUDE.md` 118 lines, `AGENTS.md` 199 |
| Path-scoped rules with `paths:` | Confirmed | Memory, "Organize rules with `.claude/rules/`" |
| Plugin project scope; `claude-plugins-official` | Confirmed | Plugins |
| A `Stop` hook as a deterministic gate | Confirmed | Best practices |

## 2. Documented mechanisms the repository does not use

1. **`/doctor prompt-audit`** — audits memory files, rules, skills and agents
   for stale or contradictory content; needs v2.1.283, so unavailable on
   2.1.278. Worth naming in a procedural skill once the CLI reaches it.
2. **The `/doctor` trim proposal** (v2.1.206, available): cuts content Claude
   can derive from the codebase. Unadopted, as on 2026-09-27.
3. **Startup length warnings** in `/status`; this repository never records
   them. Recording them at pass start would be cheap.
4. **`permissions.disableAutoMode`** is not on these four pages (its page is
   `/permissions`). Best practices, read 2026-10-02: from v2.1.283 auto mode is
   the built-in starting mode for interactive terminal and VS Code sessions.
   Whether to set it remains the owner's decision (2026-09-27); the time
   pressure is unchanged.
5. **`claudeMdExcludes`**: not applicable.
6. **`CLAUDE.local.md` counts as a `CLAUDE.md`** for the "read `AGENTS.md`
   only when there is no `CLAUDE.md`" default; this repository is unaffected
   because it imports `AGENTS.md` explicitly.

## 3. Guidance not followed, or facts to refresh

1. **`AGENTS.md` is one line under the 200-line target, and an import does
   not reduce what loads.** Every session loads 317 lines before any rule. The
   page's remedy is moving subtree-specific content into path-scoped rules.
   The 2026-09-27 decision — any line added to `AGENTS.md` is paid for by one
   removed — still stands.
2. **`.claude/README.md` cites the plugins page for the language-server
   recommendation**; the citation should move to the code-intelligence page.
   Owed to the owner (`.claude/` is a guarded path).
3. **Verification dates** in `docs/spec/11-agentic-sdlc.md` and
   `.claude/README.md` can move to 2026-10-02 for this group's pages; the
   version stays 2.1.278.
4. **The number of consecutive `Stop` blocks** is not on best practices; it
   is at `/docs/en/hooks#stop-input`, which the hooks group re-confirms. That
   group was not run on 2026-10-02 (see the pass report).
5. **New: most `env` values, and `permissions.allow`, apply only after the
   folder is trusted; `deny` and `ask` apply at once** (Settings, "Project
   settings" troubleshooting). So the Homebrew quieting variables in
   `.claude/settings.json` do not take effect before trust, while the deny
   rules that protect INV-05 and INV-06 do. Nothing in the repository records
   this ordering; one sentence in `.claude/README.md` would, and is owed to the
   owner.

Nothing in `CLAUDE.md`, `AGENTS.md`, `.claude/settings.json`,
`.claude/README.md` or `docs/spec/11-agentic-sdlc.md` was found to contradict
these four pages.
