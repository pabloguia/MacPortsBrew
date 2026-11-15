<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling recheck 2026-11-14 — memory and settings

Read 2026-11-14, read-only, by a subagent; nothing was written. Previous note: `2026-11-03-memory-settings-plugins.md`. HEAD `3277c4d`; no in-scope file (`CLAUDE.md`, `AGENTS.md`, `.claude/settings.json`, `.claude/README.md`, spec 11) changed since that note (last: `d5bfca6`). Installed Claude Code: `2.1.278`.

## Drift since 2026-11-03

- **No contract break.** Every field the repository uses is still documented and valid.
- **Page layout changed.** `settings.md` is now short; the key entries moved to `settings-reference.md` (457,636 characters, read in slices at 0, 100k, 200k, 300k; the slice from 400k was not read). Path anchoring and trust moved to `permissions.md`. Other pages were read through WebFetch's condensed answers, with `grep` over saved copies as spot checks.
- **Version ceiling moved:** `settings.md` cites v2.1.319 (`permissions.denyInsteadOfPrompting`). `.claude/README.md:55` ("up to v2.1.296") is stale.
- **New mechanisms:** `/doctor prompt-audit` (v2.1.283+, not available on 2.1.278); a named Project instructions setting; a combined-size startup warning; a per-file on-demand limit (40,000 characters on a 200K window, v2.1.309+), which applies to path-scoped rules (largest here: `adapters.md`, 5,818); `permissions.denyInsteadOfPrompting` (v2.1.319, user scope only); `attribution: false` (v2.1.281+).
- **Unchanged:** `blockWritesOutsideWorkingDirectories` (v2.1.314); the eight-block `Stop` cap; `/path` anchors at the primary working directory for project settings.
- **Not re-confirmed:** that `disableBypassPermissionsMode` also ignores a subagent's `bypassPermissions` (carried from 11-03). The README's plugin names are not on `plugins.md`.

## Sources (read 2026-11-14)

`https://code.claude.com/docs/en/memory.md`, `settings.md`, `settings-reference.md`, `permissions.md`, `best-practices.md`, `plugins.md` — all 200.

## (1) Facts the repository uses — all unchanged and valid

`$schema` URL (the schema can lag); `permissions.deny/ask/allow` (deny and ask apply at once, allow waits for trust); `//` absolute paths (`settings.json:54-57`); `Edit(~/...)`; Read/Edit deny covering `cat`, `head`, `tail`, `sed`, `tee` and redirects but not arbitrary subprocesses; `permissions.disableBypassPermissionsMode: "disable"`; `autoMemoryEnabled: false`; the five `env` variables (none in the ignored list); `.claude/settings.json` read from the primary working directory; `settings.local.json` git-ignored; cloud sessions in a several-repository setup read only `enabledPlugins` and `extraKnownMarketplaces`; the `@AGENTS.md` import (documented pattern, never read twice); `CLAUDE.md` wins over `AGENTS.md`; `.claude/rules/*.md` `paths:` as a YAML list; the 200-line target (`CLAUDE.md` 142, `AGENTS.md` 199); `permissions.defaultMode` unset; project-scope plugins through `enabledPlugins`.

## (2) Mechanisms not used that would enforce what is only stated

1. **`onFailure: "block"`** on the guard hooks — best-practices now recommends it. Guarded; owner.
2. **`permissions.blockWritesOutsideWorkingDirectories`** — trade-off against worktree use; no action.
3. **`permissions.denyInsteadOfPrompting`** — user scope only; not a repository setting.
4. **`InstructionsLoaded` hook** — would show that the six rules fire.
5. **`/doctor prompt-audit`** — a candidate `/tooling-recheck` step once the CLI is upgraded.
6. `claudeMdExcludes`, `autoMemoryDirectory`, `--setting-sources`, `sandbox` — not needed or unchanged.

## (3) Guidance not followed — deliberate, unchanged

`AGENTS.md`'s layout tables (they encode rules); bold lead-ins in `CLAUDE.md`; no `model:` in subagent files; reviewers keep `Bash`; `AGENTS.md` at 199 lines is a watch item.

## Findings by file and line

| # | File and line | Finding | Fix | Class |
|---|---|---|---|---|
| 1 | `.claude/settings.json:97-98` | `Edit(./...)` is current-directory relative | `Edit(/rust-toolchain.toml)`, `Edit(/docs/spec/01-invariants.md)` | robustness; guarded, owner |
| 2 | `CLAUDE.md:57` | "a shell or MCP reader loads nothing" is wrong for a single-file `cat` or `head` | qualify | wording |
| 3 | `CLAUDE.md:61-62` | omits the eight-block `Stop` cap | add the cap and `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` | wording |
| 4 | `docs/spec/11-agentic-sdlc.md:126` | "a single leading slash anchors at the settings file" | anchors at the settings source: for project settings, the primary working directory | wording |
| 5 | `docs/spec/11-agentic-sdlc.md:87` | AGENTS.md reading: the v2.1.281 and v2.1.280 qualifiers are missing | add; the import makes the repository immune | incomplete |
| 6 | `.claude/README.md:53-55` | version ceiling v2.1.296 | v2.1.319, read 2026-11-14 | stale |
| 7 | `.claude/README.md:9-14` | "most recently on 2026-11-03" | 2026-11-14 | stale |
| 8 | `docs/spec/11-agentic-sdlc.md:22-60` | re-read chain ends 2026-11-03 | add 2026-11-14 | stale |
| 9 | `docs/spec/11-agentic-sdlc.md:156-160` | a cloud thread in a project applies none of the rules, hooks or `env` — not stated | add | incomplete |
| 10 | `.claude/settings.json:232-253` | no `onFailure: "block"` | add | new mechanism; guarded, owner |

**Verdict:** no contract break. The open items are wording and robustness; the guarded ones are the owner's.
