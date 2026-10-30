<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification: memory, settings, best practices, plugins (2026-10-30)

Date read: **2026-10-30**. Read-only. Previous note: [`2026-10-27-memory-settings-plugins.md`](2026-10-27-memory-settings-plugins.md). This note lists only what changed since that one, plus the items still open.

**Verdict:** no contract break. Every field and value the repository uses that these four pages cover is still documented. Two statements in the previous note need correcting (the cloud-session scope and the 1M-context size limit). There is one new possible conflict, in `.claude/README.md` about `env` before trust. None of the repository files in scope have changed since 2026-10-27 (`git log --since=2026-10-27` shows only the research commit `6953863`).

## Sources

| Page | Result |
|---|---|
| https://code.claude.com/docs/en/memory | fetched 2026-10-30 (HTML variant, via the fetch-and-index tool) |
| https://code.claude.com/docs/en/settings | fetched 2026-10-30 (HTML variant). Per-key detail lives in `settings-reference`, which was **not fetched** |
| https://code.claude.com/docs/en/best-practices | fetched 2026-10-30 (HTML variant) |
| https://code.claude.com/docs/en/plugins | fetched 2026-10-30 (HTML variant, overview only) |

How the pages were read:
- I did not fetch the `.md` variants. The HTML variants succeeded on the first try, so no fallback was needed.
- I read the pages through targeted full-text search of the indexed copy, not end to end. A statement listed below as "not re-confirmed" means my search did not surface it. It does not mean the statement is gone from the page.
- A second raw fetch, to grep for the highest version number cited, was refused by the sandbox (HTTP 403) and then by the permission system. I did not work around either refusal, so I make no claim about the highest version a page cites.
- `hooks.md` text appeared in the shared index under the label `hooks-2026-10-30`, fetched the same day by another run. It is cited once below and labelled as such.

## 1. Still documented and valid (re-confirmed 2026-10-30)

- **`autoMemoryEnabled: false`** in project settings turns auto memory off for one project, and `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` also works (memory, 2026-10-30). Repository: `.claude/settings.json`, key `autoMemoryEnabled: false`. Valid.
- **`.claude/rules/*.md` with `paths:` frontmatter.** Files are found recursively. A rule without `paths` loads unconditionally. A path-scoped rule loads when Claude uses Read, Write or Edit on a matching file, and also when it views one with a Bash command that "counts as a read" (memory, 2026-10-30). Valid.
- **`@AGENTS.md` imported from `CLAUDE.md`.** Documented as the way to share one file across tools. Keeping the import "never makes Claude read `AGENTS.md` twice", whatever the **Project instructions** value (memory, 2026-10-30). Valid.
- **`$schema` pointing at `https://json.schemastore.org/claude-code-settings.json`.** Documented. The page adds that "the schema can lag behind the newest CLI releases", so a validation warning on a new key is not proof of an invalid configuration (settings, 2026-10-30). Valid.
- **Trust.** `deny` and `ask` rules apply at once. `permissions.allow`, `permissions.additionalDirectories`, `extraKnownMarketplaces` and most `env` values wait for folder trust (settings, 2026-10-30). Valid for the `deny`/`ask`/`allow` usage.
- **Block-level HTML comments** in `CLAUDE.md` are stripped before injection, comments inside code blocks are kept, and a file opened with the Read tool still shows them (memory, 2026-10-30). This is still unused here. Note that this note's SPDX header is itself such a comment.
- **`/doctor prompt-audit`** needs v2.1.283+ and covers `CLAUDE.md`, `CLAUDE.local.md`, `AGENTS.md`, rules, skills, commands, subagents and output styles. New detail: it runs through the bundled `/claude-api` skill and is unavailable while that skill is off through `skillOverrides` or `disableBundledSkills` (memory, 2026-10-30).
- **The `InstructionsLoaded` hook** logs which `CLAUDE.md` and rules files load, when, and why. It fires for an `AGENTS.md` reached through a `CLAUDE.md` import, which is this repository's case, and does not fire for one read directly through the setting (memory, 2026-10-30).
- **`includeGitInstructions` and `attribution`.** Documented as the fix when `CLAUDE.md` sets its own commit or pull-request rules (memory, 2026-10-30). Still unused.
- **Size target:** under 200 lines per `CLAUDE.md` (memory, 2026-10-30).

Not re-confirmed today, because search did not surface them:
- the `Stop` cap of eight consecutive blocks (`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`)
- the official marketplace being added on the first interactive session
- the settings-page sentence on what a leading `/` in a rule anchors to
- `disableBypassPermissionsMode`, which the settings overview does not mention (it is per-key material in `settings-reference`, not fetched)

## 2. New or changed since 2026-10-27

1. **Correction: cloud sessions** (settings, 2026-10-30). The previous note's line "Cloud sessions apply none of a repository's permission rules, hooks or `env`" is too broad. The page now distinguishes three cases:
   - **Cloud threads in a project:** none of `.claude/settings.json`'s permission rules, hooks or `env` apply.
   - **A session with one repository outside a project's threads:** reads the shared `.claude/settings.json`, because the file is part of the clone.
   - **A session with several repositories:** reads only `enabledPlugins` and `extraKnownMarketplaces`.

   `docs/spec/11-agentic-sdlc.md:150-151` states the several-repositories case correctly. It does not mention the project-thread case.
2. **Correction: on-demand size limit for a 1M context window** (memory, 2026-10-30). The limit is 200,000 characters on Claude Opus 4.6 and Sonnet 4.6, and 150,000 on later models. The previous note gave a flat 150,000. The limit is still 40,000 characters for a 200K window, and it still applies only from v2.1.309.
3. **A file over 4 MiB is skipped** (memory, 2026-10-30). The `/doctor` checkup proposes trims for a checked-in `CLAUDE.md`. Each `CLAUDE.md`, rules file and `@path` import counts as a separate file toward the length warnings.
4. **AGENTS.md availability detail** (memory, 2026-10-30):
   - Reading `AGENTS.md` directly needs v2.1.277+.
   - Before v2.1.281, Amazon Bedrock and telemetry-disabled sessions read `CLAUDE.md` only.
   - Disabling the built-in plugin through `/plugin` also turns `AGENTS.md` reading off.
   - Before v2.1.280, `/memory` did not list an `AGENTS.md` read directly.
   - **Project instructions** takes four values: `claude-md-or-agents-md` (the default), `claude-md-and-agents-md`, `claude-md` and `managed-only`. It is set through `pluginConfigs."cc-plugin-agents-md@builtin".options.instructionFiles`, and only in user, `--settings` or managed settings.

   This repository's import pattern works under every value except `managed-only`.
5. **The memory page now tells you to remove "a `SessionStart` hook that prints `AGENTS.md`"**, because it adds a second copy (memory, 2026-10-30). I checked `.claude/hooks/session-start.sh`: it only mentions that `CLAUDE.md` and `AGENTS.md` are "already loaded" (line 55). It does not print the file. No action needed.
6. **`settings.local.json` handling** (settings, 2026-10-30). The first time Claude Code writes the file, it adds `**/.claude/settings.local.json` to the user's global git excludes. While the file stays untracked, its `allow` rules skip the trust step. If the file is tracked, the trust step applies again. The repository already git-ignores the file, as `CLAUDE.md` says, so this is consistent.
7. **Best practices: outdated-advice table** (best-practices, 2026-10-30). The table now lists, as no-longer-needed advice:
   - "Use subagents to protect your context window": running out of context is rarely the problem now. Subagents are still recommended for parallel work, keeping noisy output out of the way, and independent review.
   - "Describe your architecture and file layout in `CLAUDE.md`": Claude reads the repository itself.
   - Piling on emphasis: it makes Claude over-apply the rule, and a rule that cannot bend belongs in a hook or a deny rule.
8. **Plugins: marketplace tiers** (plugins, 2026-10-30). Claude Code accepts the official names (`claude-plugins-official`, `claude-code-plugins`) and the community names (`claude-community`, among others) only for marketplaces sourced from `github.com/anthropics/`. Every other marketplace is third-party. A project-scope plugin is enabled through the committed `.claude/settings.json`, but each collaborator still installs it locally. Any plugin runs with user privileges.

## 3. Documented mechanisms the repository does not use that would enforce what it only states

- **`onFailure: "block"` on the guard and gate hooks.** It is still documented. The `hooks.md` index entry fetched 2026-10-30 (outside this note's four pages) repeats that exit 1 without valid JSON is non-blocking unless the hook is one "whose failure blocks the action". It also adds that a `PreToolUse` hook that times out does not block the call unless that same condition holds. This was open on 2026-10-26 and 2026-10-27 and is still open. It sits in a guarded path.
- **An `InstructionsLoaded` hook** would make `CLAUDE.md`'s path-scoped-rule promise observable (memory, 2026-10-30). It fires for this repository's imported `AGENTS.md`.
- **`includeGitInstructions: false` plus `attribution`.** `CLAUDE.md` and `AGENTS.md` define the commit convention. The built-in git instructions still compete with it, and the memory page names this pair as the fix (memory, 2026-10-30).
- **`/doctor prompt-audit`** as a `/tooling-recheck` step. It detects contradictions mechanically, so the repository would not rely on reviewers noticing them (memory, 2026-10-30).

## 4. Findings by file

Still open from 2026-10-27 (the files are unchanged):

1. **`.claude/settings.json:97-98`**: `Edit(./rust-toolchain.toml)` and `Edit(./docs/spec/01-invariants.md)` are current-directory-relative. The project-rooted form is `Edit(/…)`. The leading-slash semantics were not re-confirmed on the settings page today; `permissions.md` must be re-read before changing this. `guard-protected-paths.sh` backs the rule meanwhile. Guarded path.
2. **`CLAUDE.md:57`**, "a shell or MCP reader loads nothing": wrong for Bash. A Bash command that counts as a read also loads a path-scoped rule (memory, 2026-10-30). Fix: say "an MCP reader, or a Bash command that is not a recognised read, loads nothing". Owner's file.
3. **`docs/spec/11-agentic-sdlc.md:116`**, "a single leading slash anchors at the settings file": not re-confirmed today. The 2026-10-27 reading of the settings page said the anchor is the session's primary working directory. Carry it forward pending a `permissions.md` read.
4. **`docs/spec/11-agentic-sdlc.md:193`**, the unqualified "does not apply to what a subprocess does": unchanged. Not re-checked today.
5. **`.claude/README.md:50-52`**, "describes features up to v2.1.296": stale. Today's memory page cites v2.1.309 (memory, 2026-10-30). Fix: restate the date and version from a fresh `claude --version`, or drop the version-ceiling claim.
6. **`AGENTS.md`** (199 lines; the layering and path tables near the top). Best-practices, 2026-10-30, now lists architecture and file-layout description as outdated advice, because Claude reads the repository itself. Fix: keep `AGENTS.md` under 200 lines, and consider replacing derivable tables with pointers. Note that `AGENTS.md` doubles as human orientation and is tool-neutral by charter, so this is a SHOULD at most. Owner's call.
7. **`CLAUDE.md`**: many bold lead phrases. Best-practices, 2026-10-30: piled emphasis makes rules over-apply, and an unbendable rule belongs in a hook or a deny rule. Unchanged.

New:

8. **`.claude/README.md:74-77`**: says "hooks and the `env` block are used even in the two untrusted cases the permissions page lists… read 2026-10-22". The settings page, 2026-10-30, says "most `env` values apply only after each teammate trusts the folder". These may describe different cases: the README's two cases are "a parent folder trusted only" and `claude -p`, while the settings page describes an untrusted folder in general. They read as contradictory as written. Fix: re-read `permissions.md` ("project allow rules and workspace trust") and `settings-reference#env`, then either qualify the README with which `env` values wait, or confirm the exception still holds. This matters for `HOMEBREW_NO_AUTO_UPDATE` and the other `env` keys in `.claude/settings.json`.
9. **`docs/spec/11-agentic-sdlc.md:150-151`**: correct as far as it goes. Add the case "a cloud thread in a project applies no permission rule, hook or `env`" (settings, 2026-10-30). Only a single-repository session outside a project thread reads the shared file. This lands with an SE line when a part next edits that chapter.
10. **`docs/spec/11-agentic-sdlc.md:66-68`**: the AGENTS.md-availability rationale ("not at all on v2.1.276 or earlier, or in the first session after upgrading") is still correct but incomplete. Add, as optional precision: "and before v2.1.281 on Bedrock or with telemetry disabled, or when the built-in plugin is disabled" (memory, 2026-10-30). The repository's `@AGENTS.md` import makes it immune to all of these, which is the point of SE-024.
11. **`CLAUDE.md:33-40`**, "Delegate by default": consistent with the 2026-10-30 best-practices table. Subagents are still recommended for parallel work, noisy output and independent review, and those are the reasons the file gives. No change needed. If the file is ever trimmed, avoid citing context protection as a reason.
12. **Previous note `2026-10-27-memory-settings-plugins.md`**, section 1 (cloud) and section 2 (size limit): both superseded by items 1 and 2 of section 2 above. Per SE-032, the new note replaces them. The old note is a dated record and stays as it is.

## Recommendations (not adopted; `.claude/`, `CLAUDE.md` and `AGENTS.md` wait for the owner)

- Re-read `permissions.md` and `settings-reference` first. That settles findings 1, 3 and 8.
- Then apply findings 2 and 5, and `onFailure: "block"` after a `test-hooks.sh` run.
- Optionally add an `InstructionsLoaded` logging hook, and `includeGitInstructions: false` with `attribution`.
- Optionally add `/doctor prompt-audit` as a `/tooling-recheck` step.
- Findings 4, 9 and 10 are spec 11 wording and land with an SE line.
