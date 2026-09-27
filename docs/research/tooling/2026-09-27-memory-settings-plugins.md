# Tooling recheck — memory and settings — 2026-09-27

Group covered (per `.claude/skills/tooling-recheck/SKILL.md`, row "Memory and
settings"): pages `memory`, `settings`, `best-practices`, `plugins`; repo
files `CLAUDE.md`, `AGENTS.md`, `.claude/settings.json`, `.claude/README.md`,
`docs/spec/11-agentic-sdlc.md` ("Verified facts about the tooling").

Read-only task. No repository file was edited.

## Pages fetched

| Page | URL fetched | Result |
|---|---|---|
| Memory | `https://code.claude.com/docs/en/memory.md` | fetched in full (607 lines saved), read 2026-09-27 |
| Settings | `https://code.claude.com/docs/en/settings.md` | fetched in full (766 lines saved), read 2026-09-27 — this is the precedence/scope concept page; it has **no field-level schema** (that lives on `/settings-reference`, not in this task's page list; same gap the 2026-09-02 note recorded) |
| Best practices | `https://code.claude.com/docs/en/best-practices.md` | fetched in full, read 2026-09-27 |
| Plugins | `https://code.claude.com/docs/en/plugins.md` | fetched in full, read 2026-09-27 |

All four `.md` variants resolved on the first try; none was unfetched, no
fallback to a moved-page search was needed.

Repository files read: `CLAUDE.md` (118 lines), `AGENTS.md` (199 lines),
`.claude/settings.json`, `.claude/README.md`, `docs/spec/11-agentic-sdlc.md`
("Verified facts about the tooling", "Memory files", "The permission
boundary"), `.claude/agents/*.md` frontmatter, `.claude/skills/tooling-recheck/SKILL.md`,
`docs/research/tooling/2026-09-02-memory-settings-plugins.md` and its
`-foundation.md` follow-up (the last two runs of this exact group, for
regression comparison). `claude --version` was also run (read-only,
allowed): **2.1.278**.

---

## 1. Fields, values and contracts the repository uses — still documented, still valid

1. **`CLAUDE.md` importing `AGENTS.md` with `@AGENTS.md`** (`CLAUDE.md:3`,
   `"@AGENTS.md is the orientation document for every agent working here."`,
   token outside backticks/code fences so it is parsed as an import).
   Still exactly the documented mechanism. Memory page, "Import additional
   files", read 2026-09-27: "Both relative and absolute paths are
   allowed... Import parsing skips Markdown code spans and fenced code
   blocks." More directly, the page has grown a dedicated **"AGENTS.md"**
   section since 2026-09-02 whose subsection **"Remove an earlier AGENTS.md
   workaround"** names this exact setup and blesses it explicitly: *"A
   `CLAUDE.md` containing `@AGENTS.md`: you can leave it. Keeping the import
   never makes Claude read `AGENTS.md` twice, whichever **Project
   instructions** value you use."* (memory page, read 2026-09-27). No fix
   needed; this is now a first-class, named pattern rather than an inferred
   one.

2. **`autoMemoryEnabled: false`** (`.claude/settings.json`, top level).
   Still documented, still a project-settable key: *"To turn it off for a
   single project, set `autoMemoryEnabled` in that project's settings:
   `{"autoMemoryEnabled": false}`"* (memory page, "Enable or disable auto
   memory", read 2026-09-27). Matches verbatim. `11-agentic-sdlc.md`
   ("Memory files") cites the rationale correctly and is unaffected.

3. **Per-file 200-line target.** `docs/spec/11-agentic-sdlc.md` ("Memory
   files"): *"Both stay short — under 200 lines each"*. Memory page, "Write
   effective instructions": *"target under 200 lines per CLAUDE.md file"*.
   Both still say the same thing, read 2026-09-27. **But the repo is no
   longer comfortably inside it**: `CLAUDE.md` is 118 lines (was 90 at the
   2026-09-02 Foundation Pass re-check), `AGENTS.md` is **199 lines** (was
   120 at that same check) — a growth of 79 lines in `AGENTS.md` alone, and
   it is now one line under the documented target rather than 80 lines under
   it. See finding 3.1 below.

4. **`git commit *` / `git init*` under `permissions.ask`, checked-in
   `.claude/settings.json` as the shared-project scope.** Settings page,
   "Settings files and who they affect": *"Shared project | `.claude/settings.json`
   | Everyone working in the folder that contains it. In a git repository,
   commit it so teammates get it | Team permissions, hooks, plugins..."*
   (read 2026-09-27). Matches; `.claude/settings.json` is tracked in git
   (confirmed clean git status, no local-only copy shadowing it).

5. **`.claude/settings.local.json` git-ignored, personal-only.** Settings
   page: *"Project local | `.claude/settings.local.json` | You, in this one
   project only... Personal overrides for one project"* (read 2026-09-27).
   `.gitignore:2` lists `.claude/settings.local.json` explicitly; the file
   exists on disk (created 2026-09-26) and is correctly untracked. Matches
   `CLAUDE.md:92-93` and `.claude/README.md`'s statement that personal rules
   never belong in the shared file.

6. **`env` block inside a settings file is an ordinary settings key, not a
   separate precedence level, and lists merge across scopes for other keys
   like `permissions.allow`/`deny`/`ask`.** Settings page, "Settings
   precedence": *"An `env` block inside a settings file is an ordinary key
   and follows the levels above,"* and *"Lists merge instead of overriding...
   `permissions.allow`... Claude Code combines the lists instead of picking
   one."* (read 2026-09-27). `.claude/settings.json`'s `env` block
   (Homebrew/Cargo quieting vars) and its `permissions.deny`/`allow`/`ask`
   arrays are used exactly this way; no drift.

7. **Plugin marketplace naming and scopes.** `.claude/README.md`'s "Official
   plugins worth installing" section names `claude-plugins-official` as the
   official marketplace and describes project-scope adoption as requiring
   `enabledPlugins`/`extraKnownMarketplaces` in `settings.json`. Plugins
   page, read 2026-09-27, confirms both: *"marketplaces with one of
   Anthropic's official marketplace names, including `claude-plugins-official`..."*
   and *"Project scope: enabled for everyone who works in this repository,
   through the committed `.claude/settings.json`."* No drift. (The specific
   plugin IDs `security-guidance`, `rust-analyzer-lsp`, `commit-commands`
   are not verifiable from these four pages — that requires browsing the
   marketplace itself, which is out of this task's scope and was likewise
   not verified in the 2026-09-02 notes.)

8. **Best-practices' generic "install a code intelligence plugin" line**
   still supports `.claude/README.md`'s specific `rust-analyzer-lsp`
   recommendation: *"For common languages like TypeScript, Python, and Rust,
   install the pre-built LSP plugins from the official marketplace"*
   (plugins page, read 2026-09-27) and *"install a code intelligence
   plugin... after Rust code [exists]"* (`.claude/README.md`). Consistent.

9. **Subagent frontmatter shape** (`.claude/agents/*.md`: `name`,
   `description`, `tools`, `model: inherit`, plus `disallowedTools` and
   `permissionMode: plan` on the three reviewers). Best-practices page's
   worked example, read 2026-09-27, confirms `name`/`description`/`tools`/`model`
   as real frontmatter fields with a matching syntax
   (`tools: Read, Grep, Glob, Bash` / `model: opus`). `disallowedTools`,
   `permissionMode` and `color` are **not** shown or explained on any of
   these four pages — verifying those belongs to the "Subagents and skills"
   `/tooling-recheck` group (`sub-agents`, `skills` pages), not this one, so
   they are reported as **used-but-unverified-here**, not as a defect.

10. **`model: inherit`, "decided at invocation by task weight."**
    `11-agentic-sdlc.md` ("Subagents"): *"`model` stays `inherit` in every
    agent file. Which model runs a subagent is decided at invocation by task
    weight..."* Best-practices page, read 2026-09-27, "Run multiple Claude
    sessions": model choice by task is a live pattern the page recommends
    generally (Writer/Reviewer, fan-out), though it does not use the phrase
    "task weight" or discuss `model: inherit` specifically — that field's
    exact semantics are the "Subagents and skills" group's page
    (`/sub-agents`), out of scope here. No drift found in what these four
    pages do say.

---

## 2. Documented mechanisms the repo does not use, that would enforce something it only states

1. **`permissions.disableAutoMode`.** `.claude/settings.json` sets
   `disableBypassPermissionsMode: "disable"` but has no `disableAutoMode`
   key, and neither `11-agentic-sdlc.md` nor `.claude/README.md` mentions
   it. This is the exact gap the spec's own "What this is not" section
   names: *"Claude Code's own auto mode runs classifier-approved commands
   without a human in the loop."* **Not independently re-verified today** —
   the field lives on `/permissions`, which belongs to the "Hooks,
   permissions, CI" `/tooling-recheck` group and was not one of this task's
   four pages. It was confirmed verbatim on 2026-09-02
   (`docs/research/tooling/2026-09-02-memory-settings-plugins-foundation.md`,
   §3, quoting the permissions page: *"To prevent `bypassPermissions` or
   `auto` mode from being used, set `permissions.disableBypassPermissionsMode`
   or `permissions.disableAutoMode` to `"disable"` in any settings file."*).
   Carried forward as a still-open, still-unadopted recommendation; nothing
   in today's four pages contradicts it. **Fix, if adopted:** add
   `"disableAutoMode": "disable"` beside the existing
   `"disableBypassPermissionsMode": "disable"` in `.claude/settings.json`.

2. **`claudeMdExcludes`.** Not applicable yet (single-repo project, no
   monorepo ancestor `CLAUDE.md` files to exclude) — noted only because the
   settings page devotes a full subsection to it (read 2026-09-27); nothing
   for this repo to adopt.

3. **`sandbox` (OS-level filesystem/network isolation).** Already tracked as
   a `SHOULD` in `11-agentic-sdlc.md` and `.claude/README.md` ("The sandbox:
   evaluated, not enabled"), with the exact JSON shape recorded from the
   2026-09-02 Foundation Pass run against `/sandboxing` — a page outside
   this task's four and outside this group's assignment (it belongs to
   "Hooks, permissions, CI"). Not re-verified today; no drift expected from
   anything in `memory`/`settings`/`best-practices`/`plugins`, none of which
   mention the sandbox in enough depth to check it. Still unenabled in
   `.claude/settings.json` (no `sandbox` key), consistent with "not enabled
   until the build exists" — there is still no `rust-toolchain.toml` / build
   in this checkout, so this is not a defect.

4. **`/doctor`'s CLAUDE.md trim proposal.** Memory page, read 2026-09-27:
   *"The `/doctor` checkup proposes trims for a checked-in CLAUDE.md: it
   cuts content Claude can derive from the codebase... and keeps pitfalls,
   rationale, and conventions that differ from tool defaults."* The repo
   does not document running `/doctor` anywhere (`.claude/skills/`,
   `CLAUDE.md`, `.claude/README.md`). Given finding 3.1 below (`AGENTS.md`
   at 199/200 lines), this is a mechanism worth reaching for now rather than
   trimming by hand from scratch. Not adopted here (read-only task).

---

## 3. Guidance the memory files do not (or barely) follow

1. **`AGENTS.md` at 199 of a 200-line target — a near-miss, not a pass with
   margin.** `docs/spec/11-agentic-sdlc.md` ("Memory files") states the
   200-line ceiling as a MUST; `AGENTS.md` is currently 199 lines, up from
   120 at the last tooling recheck (2026-09-02 Foundation Pass run) — 79
   lines added since, with no line removed to compensate. One more
   paragraph and the file breaches its own documented target, and per the
   memory page ("Write effective instructions", read 2026-09-27): *"Longer
   files consume more context and reduce adherence."* **Fix:** run (or ask
   a session to run) `/doctor`'s trim proposal (finding 2.4 above) before
   the next material addition to `AGENTS.md`, or move a section such as
   "Where things live" (a directory table, which `/doctor`'s own exclude
   criteria — *"directory layouts... architecture overviews"* — targets by
   name) into a path-scoped rule or `docs/` reference instead of the memory
   file. The 2026-09-02 first-run note flagged the same directory-table
   section as a trim candidate under the docs' own exclude column; it has
   not been trimmed since, and the file has grown instead.

2. **The verified-Claude-Code-version line in `.claude/README.md` is now 20
   patch releases stale, and two of those releases changed exactly the
   behaviours this repository depends on.** `.claude/README.md`:
   *"**Claude Code** — 2.1.258 is what this configuration was verified with
   on 2026-09-02."* Installed today: **2.1.278** (`claude --version`, run
   read-only for this check). Two relevant version gates crossed between
   2.1.258 and 2.1.278, both stated on the memory page (read 2026-09-27):
   - **v2.1.277**: *"Reading `AGENTS.md` directly requires Claude Code
     v2.1.277 or later."* The installed version now supports native
     `AGENTS.md` reading — but the repo's `CLAUDE.md` still imports
     `AGENTS.md` rather than relying on that, which the docs explicitly
     endorse as harmless (finding 1.1). **No fix needed**, but worth stating
     in `.claude/README.md` so a reader does not wonder whether the import
     is now redundant or wrong — it is neither.
   - Best-practices page, read 2026-09-27: *"With Claude Code v2.1.283 or
     later, auto mode is the built-in starting permission mode for
     interactive terminal and VS Code sessions... On earlier versions, auto
     mode is the built-in starting permission mode only on Pro, Max, and
     Team plans."* Installed (2.1.278) is **below** 2.1.283, so the
     universal default has not yet taken effect here; whatever made this
     session start in auto mode did so under the older, plan-gated rule.
     This does not contradict `11-agentic-sdlc.md`'s "What this is not"
     section, which already names auto mode as a live gap, but it means
     that gap will widen automatically (auto mode becoming default for
     every session, not just paid plans) the next time Claude Code is
     upgraded past 2.1.283 — with no code or config change in this
     repository. **Fix:** re-run `/tooling-recheck`'s "Hooks, permissions,
     CI" group (which owns `/permissions`) once the installed CLI reaches
     2.1.283, specifically to decide whether `permissions.disableAutoMode`
     (finding 2.1) should be adopted before that default takes effect
     project-wide rather than after.
   - **Fix for this file specifically:** update the verified-version line in
     `.claude/README.md` to the version actually re-checked at the end of
     this pass, per the file's own stated convention ("re-verified at the
     start of the Foundation Pass the same day").

3. **The "eight consecutive `Stop`-hook blocks" cap is no longer stated,
   verbatim or otherwise, on the best-practices page — the source
   `docs/spec/11-agentic-sdlc.md` and `.claude/README.md` both cite for the
   number "eight."** 2026-09-02 note (`2026-09-02-memory-settings-plugins-foundation.md`,
   §1.2) quoted the best-practices page verbatim: *"Claude Code overrides
   the hook and ends the turn after 8 consecutive blocks."* Today's fetch of
   the same page (`https://code.claude.com/docs/en/best-practices.md`, read
   2026-09-27), section "Give Claude a way to verify its work", says only:
   *"a [Stop hook](https://code.claude.com/docs/en/hooks#stop) runs your check as a script and
   blocks the turn from ending until it passes. [Stop input](https://code.claude.com/docs/en/hooks#stop-input)
   covers the cap on consecutive blocks."* — the number itself has moved off
   this page and onto `/hooks#stop-input`, which is not one of this task's
   four pages (it belongs to the "Hooks, permissions, CI"
   `/tooling-recheck` group). This is not evidence the cap changed or that
   `docs/spec/11-agentic-sdlc.md`'s "eight" / `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`
   is wrong — only that **this specific page can no longer be cited as its
   source**, and the number cannot be re-confirmed within this task's scope.
   **Fix:** the next "Hooks, permissions, CI" `/tooling-recheck` run should
   fetch `/hooks` and either re-confirm "eight" verbatim (and update the
   citation in `11-agentic-sdlc.md`/`.claude/README.md` if they currently
   imply best-practices as the source) or record the new number if it
   changed. Flagging here rather than silently trusting the carried-forward
   figure is the point of re-running the check every pass.

4. **Nothing else** in `CLAUDE.md`, `AGENTS.md`, `.claude/settings.json`, or
   `.claude/README.md` was found to contradict `memory`, `settings`,
   `best-practices`, or `plugins` as read today. In particular: CLAUDE.md's
   delegation and skill-invocation guidance, the `.claude/README.md`
   "Prerequisites" and "Running a pass" sections, and the plugin
   recommendations were all re-checked and matched.

---

## Not verifiable within this task's four pages (named so no gap is silently filled)

- Hook JSON field names (`matcher`, `hooks[].type`, `command`, `timeout`,
  `statusMessage`) used throughout `.claude/settings.json`'s `hooks` block —
  documented on `/hooks`, assigned to the "Hooks, permissions, CI" group.
- `permissions.disableBypassPermissionsMode` and `permissions.disableAutoMode`
  exact semantics, and Bash/Edit permission-rule matching syntax
  (`//opt/local/**` double-slash absolute form, compound-command splitting,
  wrapper stripping) — documented on `/permissions`, same group.
- The `sandbox` JSON schema — documented on `/sandboxing`, same group.
- Subagent-specific frontmatter fields `disallowedTools`, `permissionMode`,
  `color`, and skill frontmatter fields (`argument-hint`, `allowed-tools`,
  `license`, `disable-model-invocation`) — documented on `/sub-agents` and
  `/skills`, assigned to the "Subagents and skills" group.
- GitHub Actions specifics (`/install-github-app`, workflow secrets,
  `anthropics/claude-code-action`) — documented on `/github-actions`,
  "Hooks, permissions, CI" group.

None of the above was asserted from memory anywhere in this note; each is
named as unfetched-for-this-task rather than silently assumed.

---

## Decisions recorded by the main session, 2026-09-27

- `.claude/README.md`'s verified version moves to 2.1.278 (`claude --version`).
- **`disableAutoMode` is the owner's**, not adopted here: it changes how the
  owner runs sessions — including the autonomous one that ran this recheck —
  and `.claude/settings.json` is guarded. Recommended to decide before the CLI
  passes 2.1.283, when auto mode becomes the default starting mode.
- `AGENTS.md` is at 199 of its 200-line MUST ceiling. Any line added there must
  be paid for by one removed.
