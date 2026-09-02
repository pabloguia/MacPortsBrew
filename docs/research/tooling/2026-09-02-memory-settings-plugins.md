# Claude Code documentation drift check — memory, settings, plugins, best practices

Read date for every page: **2026-09-02**. All four requested pages fetched
successfully (HTTP 200, no redirects). Two further pages were fetched because
the `settings` page is a concept page and the key-level schema lives elsewhere:

| Page | URL | Status |
|---|---|---|
| Memory | https://code.claude.com/docs/en/memory | fetched, read 2026-09-02 |
| Best practices | https://code.claude.com/docs/en/best-practices | fetched, read 2026-09-02 |
| Plugins | https://code.claude.com/docs/en/plugins | fetched, read 2026-09-02 |
| Settings (concept) | https://code.claude.com/docs/en/settings | fetched, read 2026-09-02 |
| Settings reference (schema) | https://code.claude.com/docs/en/settings-reference | fetched, read 2026-09-02 |
| Permissions (rule syntax) | https://code.claude.com/docs/en/permissions | fetched, read 2026-09-02 |

Not fetched (out of scope, so claims that depend on them are marked "not
verified here"): `hooks`, `sub-agents`, `skills`, `sandboxing`,
`permission-modes`, `github-actions`.

Repository files read (read-only): `CLAUDE.md` (94 lines), `AGENTS.md`
(120 lines), `.claude/settings.json`, `.claude/README.md`, `.gitignore`,
`.claude/skills/*/SKILL.md` frontmatter, `.claude/agents/*.md` frontmatter,
`docs/spec/11-agentic-sdlc.md` sections "Verified facts about the tooling",
"Memory files", "The permission boundary", "Hooks", "Subagents", "Skills",
"The pass protocol", "Continuous integration".

---

## 1. Memory

### 1.1 Is `CLAUDE.md` still the project-scope memory file? — Yes

- Project instructions live in `./CLAUDE.md` or `./.claude/CLAUDE.md`
  ("Team-shared instructions for the project", "Team members via source
  control"). Source: https://code.claude.com/docs/en/memory, table "Choose
  where to put CLAUDE.md files", read 2026-09-02.
- Load order: managed policy → `~/.claude/CLAUDE.md` → project → `CLAUDE.local.md`,
  concatenated, "instructions closer to where you launched Claude are read
  last". Same page, "How CLAUDE.md files load", read 2026-09-02.
- Delivery: "CLAUDE.md content is delivered as a user message after the system
  prompt, not as part of the system prompt itself." Same page, "Claude isn't
  following my CLAUDE.md", read 2026-09-02.
- "Project-root CLAUDE.md survives compaction: after `/compact`, Claude re-reads
  it from disk and re-injects it into the session." Same page, read 2026-09-02.
  (The repo's `SessionStart` hook also matches `compact`; the two are
  complementary, not redundant.)
- "Block-level HTML comments (`<!-- maintainer notes -->`) in CLAUDE.md files
  are stripped before the content is injected into Claude's context." Same
  page, read 2026-09-02. The repository does not use this; it is a free way to
  keep maintainer notes in `CLAUDE.md` without spending context.

**Repo status:** matches. `CLAUDE.md` is at the repo root; `.claude/CLAUDE.md`
does not exist (no duplication).

### 1.2 Does `@path` import still work, relative to the importing file? — Yes

Verbatim from https://code.claude.com/docs/en/memory, "Import additional
files", read 2026-09-02:

- "Both relative and absolute paths are allowed. Relative paths resolve relative
  to the file containing the import, not the working directory. Imported files
  can recursively import other files, with a maximum depth of four hops."
- "Import parsing skips Markdown code spans and fenced code blocks. To mention
  a path in your CLAUDE.md without importing it, wrap it in backticks".
- Imports may appear "anywhere in your CLAUDE.md" (example shows an import
  mid-sentence).
- External imports (path resolving outside the working directory) trigger a
  one-time approval dialog for project-level memory files.
- The docs now have a dedicated **"AGENTS.md"** subsection: "Claude Code reads
  `CLAUDE.md`, not `AGENTS.md`. If your repository already uses `AGENTS.md` for
  other coding agents, create a `CLAUDE.md` that imports it so both tools read
  the same instructions without duplicating them. You can also add
  Claude-specific instructions below the import." The example is literally
  `@AGENTS.md` followed by `## Claude Code`. A symlink is offered as the
  alternative "if you don't need to add Claude-specific content".

**Repo status:** `CLAUDE.md` line 3 reads `@AGENTS.md is the orientation
document ...` — an inline import outside backticks, which the docs confirm
resolves. `grep` found no other unintended `@` imports in `CLAUDE.md` or
`AGENTS.md` (every other path is in backticks). SE-024 and
`11-agentic-sdlc.md` "Memory files" describe exactly the pattern the official
page now documents — **no drift; the docs have moved toward the repo**.

One caveat the repo's rationale text should carry: "Splitting into `@path`
imports helps organization but doesn't reduce context, since imported files
load at launch." (memory page, "My CLAUDE.md is too large", read 2026-09-02.)
The effective launch payload is `CLAUDE.md` (94) + `AGENTS.md` (120) = **214
lines**, above the per-file target discussed below even though each file
individually is under it.

### 1.3 Size / line-length guidance

- "**Size**: target under 200 lines per CLAUDE.md file. Longer files consume
  more context and reduce adherence." (memory page, "Write effective
  instructions", read 2026-09-02.)
- "Claude Code loads a CLAUDE.md file of up to 4 MiB in full and skips a larger
  file. Shorter files produce better adherence." (memory page, "How it works",
  read 2026-09-02.)
- "The `/doctor` checkup proposes trims for a checked-in CLAUDE.md: it cuts
  content Claude can derive from the codebase, such as directory layouts,
  dependency lists, and architecture overviews, and keeps pitfalls, rationale,
  and conventions that differ from tool defaults. The trim check requires
  Claude Code v2.1.206 or later." (memory page, read 2026-09-02.)
- Best-practices page, "Write an effective CLAUDE.md", read 2026-09-02: "Keep it
  concise. For each line, ask: *'Would removing this cause Claude to make
  mistakes?'* If not, cut it. Bloated CLAUDE.md files cause Claude to ignore
  your actual instructions!" Exclude column includes "File-by-file descriptions
  of the codebase", "Long explanations or tutorials", "Anything Claude can
  figure out by reading code".
- No line-*length* (column width) guidance exists on either page; only
  line-*count*.

**Repo status:** `11-agentic-sdlc.md` "Memory files" already says "Target
under 200 lines each" — matches the documented per-file target. Both files
comply individually (94 and 120). Two content items are candidates under the
docs' own exclude column: `AGENTS.md` "Where things are" (a directory table,
which `/doctor` classifies as derivable) and `AGENTS.md` "The invariants, in
one breath" (a restatement of `docs/spec/01-invariants.md`, which the repo's
own MUST NOT — "Neither file restates the specification" — also argues
against). Reported as observations, not recommendations.

### 1.4 New memory mechanisms the repo does not use

All from https://code.claude.com/docs/en/memory, read 2026-09-02.

| Mechanism | What the docs say | Fit with stated repo needs |
|---|---|---|
| `.claude/rules/*.md` | "Rules without `paths` frontmatter are loaded at launch with the same priority as `.claude/CLAUDE.md`." Files discovered recursively; symlinks supported; user-level `~/.claude/rules/` loads before project rules. | Would let `CLAUDE.md` shed always-on rules into topic files without changing what loads. Does **not** reduce context (same as imports) unless path-scoped. |
| Path-scoped rules (`paths:` frontmatter, glob syntax, brace expansion, 1,000-pattern / 4 MiB budget) | "These conditional rules only apply when Claude is working with files matching the specified patterns." "Path-scoped rules trigger when Claude reads files matching the pattern, not on every tool use." Reload after compaction "as Claude reads files they apply to". | Direct fit for the repo's stated principle "Do not load the whole specification up front". Examples of rules that are currently always-on but only matter in a subtree: the Rust toolchain pin note (`rust-toolchain.toml`, `Cargo.toml`, `src/**/*.rs`), "Both directions are first class" (planner/executor sources once they exist), "Dated provider facts are data" (policy-data directory). The memory page itself says: "If an entry is a multi-step procedure or only matters for one part of the codebase, move it to a skill or a path-scoped rule instead." |
| `CLAUDE.local.md` | Personal per-project notes; "Add `CLAUDE.local.md` to your `.gitignore` so it isn't committed." Appended after `CLAUDE.md` at the same level. Per-worktree if gitignored. | Repo `.gitignore` ignores `.claude/settings.local.json` and `.claude/agent-memory/` but **not** `CLAUDE.local.md`. `.claude/README.md` routes personal preference to `settings.local.json` / `~/.claude/`, and does not mention `CLAUDE.local.md`. Gap: if anyone creates one it would be committable. |
| Auto memory | "Auto memory is on by default." Stored at `~/.claude/projects/<project>/memory/` (machine-local, shared across worktrees), `MEMORY.md` index loaded "first 200 lines or 25KB". Claude "skips anything your CLAUDE.md files already say". Disable per project with `"autoMemoryEnabled": false` in project settings, or `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`. "The main conversation's auto memory isn't loaded into subagents". | `11-agentic-sdlc.md` states `.claude/current-pass` "is the only piece of per-session state this repository keeps". That remains true of the *repository*, but Claude Code now keeps per-repository state outside it by default. Whether the project wants sessions to derive everything from versioned files (its stated preference: facts in `docs/`, dated and sourced) is a decision the spec does not currently record either way. `autoMemoryEnabled` is settable from `.claude/settings.json` ("Scope: Any file", settings-reference, read 2026-09-02). |
| `claudeMdExcludes` | Skip ancestor/other-team CLAUDE.md files by glob; "Arrays merge across layers". | Not relevant: no ancestor CLAUDE.md files apply to this repo. |
| `claudeMd` (managed) | Inject CLAUDE.md text from managed settings; managed-only. | Not relevant. |
| `/init` with `CLAUDE_CODE_NEW_INIT=1` | "also reads `AGENTS.md`" and incorporates it; `/import` "appends a one-time copy of instruction files such as `AGENTS.md` to the matching `CLAUDE.md` ... Requires Claude Code v2.1.213 or later." | The repo chose import-by-reference (SE-024), which avoids the duplication `/import` would create. No action implied; recorded so nobody runs `/import` here by accident. |
| `InstructionsLoaded` hook | "log exactly which instruction files are loaded, when they load, and why." | Would make `/preflight` able to assert "the memory files loaded" mechanically. Hook events page not fetched; existence taken from the memory page only. |
| `/context` | "Run `/context` and check the list under **Memory files** to verify your CLAUDE.md and CLAUDE.local.md files loaded." | Repo docs do not mention how to verify the import resolved. |

---

## 2. Settings

### 2.1 `permissions.disableBypassPermissionsMode` — exact value confirmed

From https://code.claude.com/docs/en/settings-reference, entry
`permissions.disableBypassPermissionsMode`, read 2026-09-02:

- "Prevent anyone from entering `bypassPermissions` mode. Claude Code then
  rejects the `--dangerously-skip-permissions` flag, and ignores an agent
  definition's `permissionMode: bypassPermissions`, so the subagent runs with
  the paren[t's mode]".
- "**Type**: the string `"disable"`" — the only documented value.
- "**Default**: unset".
- "**Per-session overrides**: this key takes precedence over
  `--dangerously-skip-permissions`, which Claude Code rejects while the key is
  set".
- "Before v2.1.223, Claude Code applied the frontmatter permission mode even
  with bypass disabled."
- Merge semantics: it is a "Lock" key — "Applies the strictest value any
  source sets" (settings-reference, precedence table, read 2026-09-02). So the
  project-file value cannot be loosened by a user or local file.

**Repo status:** `.claude/settings.json` sets `"disableBypassPermissionsMode":
"disable"` and `11-agentic-sdlc.md` requires exactly that. **No drift.**

### 2.2 `env`, `hooks`, `permissions.allow/deny/ask` — shape confirmed

- `env`: "**Type**: object mapping variable names to string values", "Scope:
  Any file". "A value here overwrites the same variable exported in your
  shell". "Values here are plain text in the settings file and reach every
  subprocess Claude Code starts." (settings-reference, `env`, read
  2026-09-02.) Repo's `env` block is this shape.
- `env` trust caveat: "`permissions.allow` rules,
  `permissions.additionalDirectories`, `extraKnownMarketplaces`, and most
  `env` values apply only after each teammate trusts the folder." (settings
  page, "The key waits for trust", read 2026-09-02.) So the provider-quieting
  variables (`HOMEBREW_NO_AUTO_UPDATE` etc.) and the read-only `allow` list are
  inert until the workspace-trust dialog is accepted; `deny` and `ask` rules
  are listed separately and are not gated the same way (sentence truncated in
  the fetched text; the permissions page confirms "deny rules from any scope
  are evaluated before allow rules", read 2026-09-02).
- `permissions.allow` / `deny` / `ask`: each "**Type**: array of permission
  rule strings", "Scope: Any file"; "The three rule arrays share one syntax".
  `--allowedTools` and `--disallowedTools` are the per-session equivalents.
  (settings-reference, read 2026-09-02.)
- `hooks`: listed in the settings-reference table as "Run your own commands as
  hooks at points in Claude Code's lifecycle". The plugins page shows the
  settings `hooks` object shape — `{"hooks": {"PostToolUse": [{"matcher":
  "Write|Edit", "hooks": [{"type": "command", "command": "..."}]}]}}` — and
  says of `hooks/hooks.json`: "Copy the `hooks` object from your
  `.claude/settings.json` ... since the format is the same." (plugins page,
  "Migrate hooks", read 2026-09-02.) The repo's `hooks` block has that shape.
  The `timeout` and `statusMessage` fields the repo uses were **not verified
  here** (the `hooks` reference page was not in scope).
- Hooks from a project settings file are subject to workspace trust:
  `autoMemoryDirectory` is honoured "under the same workspace trust rule as
  hooks in settings files" (memory page and settings-reference, read
  2026-09-02). The permissions page section "What runs before you trust a
  folder" exists but its body was not extracted.

### 2.3 Rule-syntax findings against the repo's actual rules

All from https://code.claude.com/docs/en/permissions, read 2026-09-02.

**(a) `Edit(/opt/local/**)`, `Edit(/usr/local/**)`, `Edit(/opt/homebrew/**)`,
`Edit(/Library/**)` are not absolute paths under the current syntax.** The
docs state: "A pattern like `/Users/alice/file` isn't an absolute path. The
single leading slash anchors at the settings source, not the filesystem root.
Use `//Users/alice/file` for absolute paths." And for a rule "defined in
Project settings at `.claude/settings.json`", `/path` "resolves to `<primary
working directory>/path`". So as written these four deny rules match
`<repo>/opt/local/**`, `<repo>/usr/local/**`, `<repo>/opt/homebrew/**` and
`<repo>/Library/**` — not the provider prefixes. The documented absolute form
is `Edit(//opt/local/**)` etc. The second layer
(`guard-protected-paths.sh`) still covers these paths, which is precisely the
"two layers" argument in `.claude/README.md`, but the first layer is currently
inert for those four entries. **This is the most concrete drift found.**

**(b) `~/`-prefixed rules are correct.** `Edit(~/.zshrc)`, `Edit(~/.config/fish/**)`,
`Edit(~/Library/LaunchAgents/**)` use the documented "`~/path` — Path from home
directory" form.

**(c) Redundant bare-command rules.** "A `*` at the end, with a space before
it, also matches the bare command. `Bash(ls *)` matches `ls`". Therefore
`Bash(sudo)` is subsumed by `Bash(sudo *)`, and `Bash(brew list)` by
`Bash(brew list *)`. Harmless, but the pairs can be collapsed.

**(d) No-space wildcards are deliberately broad.** "The space before a trailing
`*` is part of the rule. `Bash(ls *)` requires a space after `ls`, so `lsof`
doesn't match." The repo's `Bash(brew update*)`, `Bash(port selfupdate*)`,
`Bash(port sync*)`, `Bash(port reclaim*)`, `Bash(git init*)` therefore also
match e.g. `brew update-reset` and `port sync-something`. That is consistent
with the intent (deny/ask everything starting with those tokens) and is noted
only so it is understood to be intentional.

**(e) On "fragile".** `11-agentic-sdlc.md` says "Anthropic's own permissions
documentation states that Bash patterns constraining arguments are fragile".
The current page does **not** use the word "fragile"; what it now says is
mechanically stronger and still supports the two-layer design:
- "Bash rules match the whole command text, with `*` standing in for any
  text." "Before matching Bash rules, Claude Code strips a fixed set of
  wrappers" (so `env FOO=1 brew install` and similar are normalised — the
  fixed set was not extracted).
- Compound commands are split on `&&`, `||`, `;`, `|`, `|&`, `&`, newlines,
  and "A rule must match each subcommand independently."
- "Read and Edit deny rules apply to Claude's built-in file tools and to file
  commands Claude Code recognizes in Bash, such as `cat`, `head`, `tail`, and
  `sed`; they don't apply to arbitrary subprocesses, so for OS-level enforcement
  enable the sandbox." (settings-reference, `permissions.deny`, read
  2026-09-02.)
- Under auto mode: "A command that any other allow rule matches, such as
  `Bash(npm test)`, skips the classifier, and a destructive argument the
  rule's prefix didn't anticipate can get through unseen." (settings-reference,
  `autoMode.classifyAllShell`, read 2026-09-02.)
The spec's citation should be re-worded to what the page now says, or dated.

### 2.4 Documented settings the repo does not set — factual notes

| Key | What the docs say (settings-reference / settings / permissions pages, read 2026-09-02) | Relevance to the repo's stated needs |
|---|---|---|
| `permissions.defaultMode` | Values: `default` (alias `manual`), `acceptEdits`, `plan`, `auto`, `dontAsk`, `bypassPermissions`. "values `auto` and `bypassPermissions` don't take effect from project or local settings; set them in user or managed settings instead ... Before v2.1.257, `bypassPermissions` took effect from any file." Claude Code on the web "honors only `acceptEdits`, `plan`, `default`, and `auto` from this key". | `11-agentic-sdlc.md` says non-trivial work SHOULD start in plan mode and `CLAUDE.md` tells the agent to use `claude --permission-mode plan`. `"defaultMode": "plan"` (or `"default"`) is honoured from a project file; the repo currently relies on the human remembering the flag. |
| Auto mode (starting mode) | Best-practices page: "On Pro, Max, and Team plans, auto mode is the built-in starting permission mode for interactive terminal and VS Code sessions: a separate classifier model reviews most actions instead of you". | `11-agentic-sdlc.md` "What this is not": "The real boundary is the permission system plus a human reading a plan". Under the now-default auto mode the reviewer of most actions is a classifier, not the human, unless the session or project sets another mode. The spec does not currently mention auto mode. |
| `useAutoModeDuringPlan` | "when auto mode is available, the classifier reviews each shell command during planning instead of prompting you for it." Scope: "User, local, or managed. A repository can't turn it off for you." | Plan mode is no longer purely read-only when auto mode is available: "with auto mode available, classifier-approved commands also run" (permissions page, mode table). Cannot be set from `.claude/settings.json`. |
| `autoMode.classifyAllShell` | Suspends every shell allow rule so the classifier sees every command; "Scope: User or managed". | Prefix allow rules like `Bash(file *)`, `Bash(uname *)`, `Bash(brew list *)` bypass the classifier under auto mode. Not settable from the project file. |
| `sandbox` (`enabled`, `autoAllowBashIfSandboxed`, `excludedCommands`, `filesystem.allowWrite/denyWrite/denyRead`, `network.allowedDomains`) | "Scope: Any file". "Default: unset, so Claude Code runs commands without a sandbox." `filesystem.denyWrite` "Block sandboxed commands from writing to specific paths ... adds the paths from your `Edit(...)` deny permission rules." Permissions page: "Sandbox restrictions prevent Bash commands from reaching resources outside defined boundaries, even if a prompt injection bypasses Claude's decision-making". "When you enable sandboxing and leave `autoAllowBashIfSandboxed` at its default of `true`, sandboxed Bash commands run without prompting". | `.claude/README.md` says truthfully "Neither layer is a sandbox". The documented sandbox is the OS-level layer the README says the project lacks; it is settable from the project file. Note `autoAllowBashIfSandboxed` defaults to `true` once `enabled` is on, which changes prompting behaviour. `sandboxing` page not fetched, so platform support and protected-path details are not verified here. |
| `permissions.additionalDirectories` | "array of directory paths"; "Directories listed in `permissions.additionalDirectories` in a settings file grant file access only and don't load any of the configuration below." Trust-gated like `allow`. | Not needed today; everything is inside the repo. |
| `enabledPlugins` / `extraKnownMarketplaces` | See section 3. Project scope exists; "Enabling a plugin from an external source ... in a project's `.claude/settings.json` doesn't install it for other people." Trust-gated. | Repo uses neither. |
| `respectGitignore` | "Control whether the `@` file picker leaves out files that match `.gitignore` patterns." Boolean, default `true`, "Scope: Any file". | Cosmetic (interactive `@` picker only). No effect on hooks or tools. |
| `autoMemoryEnabled` / `autoMemoryDirectory` | See 1.4. `autoMemoryEnabled`: Boolean, default `true`, "Scope: Any file"; `CLAUDE_CODE_DISABLE_AUTO_MEMORY` env var "takes precedence". | Decision not recorded in spec. |
| `disableAllHooks` | "Turn off hooks, any custom status line, and any custom file suggestion command." Boolean, "Scope: Any file"; from a non-managed file it disables the non-managed hooks. | This is a built-in switch that disables the repo's guard and Stop hooks wholesale from `settings.local.json`. The spec's own escape hatch is `MPB_SKIP_STOP_GATE=1` with a reporting obligation; `disableAllHooks` has no such obligation attached. |
| `attribution` (`commit`, `pr`, `sessionUrl`) | Replaces deprecated `includeCoAuthoredBy`; commits get a `Co-Authored-By` trailer by default. | Repo commit rules ("Conventional commits") do not address the trailer either way. |
| `allowManagedPermissionRulesOnly` | Managed-only lock making managed settings the only source of rules. | Not applicable (no managed settings). |

### 2.5 Settings precedence and file roles — confirmed

- Order (highest first): managed → command-line `--settings`/flags → `.claude/settings.local.json` → `.claude/settings.json` → `~/.claude/settings.json`. (settings page, read 2026-09-02.)
- "Claude Code adds it [`settings.local.json`] to your global git excludes the first time it writes the file ... if you create the file by hand, add it to `.gitignore` yourself." Repo `.gitignore` already lists it. Matches `.claude/README.md`.
- Cloud sessions: "**Shared project settings** (`.claude/settings.json`): read, because the file is part of the clone." "User and project local settings ... not read." (settings page, read 2026-09-02.)

---

## 3. Plugins

All from https://code.claude.com/docs/en/plugins, read 2026-09-02, unless
noted.

### 3.1 What a plugin is

- "Plugins let you extend Claude Code with custom functionality that can be
  shared across projects and teams." A plugin is a "self-contained director[y]
  with skills, agents, hooks, or a `.claude-plugin/plugin.json` manifest".
- Layout (all at plugin root, never inside `.claude-plugin/`): `skills/`,
  `commands/` (legacy flat form), `agents/`, `hooks/hooks.json`, `.mcp.json`,
  `.lsp.json`, `monitors/monitors.json`, `bin/`, `settings.json` (only `agent`
  and `subagentStatusLine` keys honoured).
- Manifest fields: `name` ("Unique identifier and skill namespace"),
  `description`, optional `version` ("users only receive updates when you bump
  this field"), optional `author`.
- "Plugin skills are always namespaced (like `/my-first-plugin:hello`)".
- Local testing: `claude --plugin-dir ./my-plugin` (also `.zip`), `--plugin-url`,
  `/reload-plugins`, `claude plugin validate ./your-plugin`.
- "Develop a plugin in your skills directory": `claude plugin init my-tool`
  creates `~/.claude/skills/my-tool/` and it "loads as `my-tool@skills-dir` with
  no marketplace or install step".

### 3.2 `enabledPlugins` and marketplaces

- `enabledPlugins`: "Turn individual plugins on or off per scope"; IDs are
  `plugin@marketplace`. "Project settings take precedence over user settings,
  so setting a plugin to `false` in `~/.claude/settings.json` doesn't disable a
  plugin that the project's `.claude/settings.json` enables. To opt out of a
  project-enabled plugin on your machine, set it to `false` in
  `.claude/settings.local.json`." "Enabling a plugin from an external source
  such as a GitHub repository or npm package in a project's
  `.claude/settings.json` doesn't install it for other people ... reports the
  plugin as not installed until each user installs it themselves."
  (settings-reference, read 2026-09-02.)
- `extraKnownMarketplaces`: "Register additional plugin marketplaces by name,
  so that people who open the repository ... get the marketplace without adding
  it themselves." Honoured from a repository file "only after you accept the
  workspace trust dialog". Source types: `github`, `git`, `url`, `npm`, ...
  (settings-reference, read 2026-09-02.)
- Public marketplaces: `claude-plugins-official` (curated, registered
  automatically on first interactive launch) and `claude-community`
  (submissions via in-app forms). (plugins page, read 2026-09-02.)

### 3.3 Standalone `.claude/` versus plugin — what the docs actually say

| Approach | Skill names | Best for |
|---|---|---|
| Standalone (`.claude/` directory) | `/hello` | "Personal workflows, project-specific customizations, quick experiments" |
| Plugins | `/plugin-name:hello` | "Sharing with teammates, distributing to community, versioned releases, reusable across projects" |

"Start with standalone configuration in `.claude/` for quick iteration, then
convert to a plugin when you're ready to share."

Migration facts relevant to this repository's layout:
- Hooks move from `settings.json` to `hooks/hooks.json` ("the format is the
  same").
- "After migrating, remove the original files from `.claude/` to avoid
  duplicates. Project and user `.claude/agents/` definitions override
  same-named plugin agents, so the plugin version only takes effect once the
  originals are removed. Plugin skills are namespaced as
  `/plugin-name:skill-name`, so the original `/skill-name` and the plugin copy
  both remain available rather than one overriding the other."
- A plugin's `hooks.json` still runs inside the user's session; the permission
  `deny`/`allow`/`ask` rules and `disableBypassPermissionsMode` have no plugin
  equivalent on this page — they remain `settings.json` keys.

**Factual fit:** the repo's `.claude/agents`, `.claude/skills` and hooks are
described by `.claude/README.md` as "part of the project's safety story" and
by `11-agentic-sdlc.md` as project rules mapping to `docs/spec/`. Every skill
and agent is repository-specific (ADR template, `docs/spec-evolution.md`
format, INV numbers, pass protocol). That matches the docs' "project-specific
customizations" row for standalone use. Nothing on the plugins page says a
project-local configuration *should* become a plugin; the trigger the docs
name is "when you're ready to share" / "reusable across projects". The
permission boundary (deny rules, bypass lock) would in any case have to stay
in `settings.json`.

---

## 4. Best practices — followed / not followed

Source for every row: https://code.claude.com/docs/en/best-practices, read
2026-09-02.

### 4.1 Recommendations the repo already follows

| Docs recommendation (quoted) | Where the repo does it |
|---|---|
| "Give Claude a check it can run: tests, a build, a screenshot to compare." "As a deterministic gate: a Stop hook runs your check as a script and blocks the turn from ending until it passes." | `Stop` → `stop-gate.sh`; `/preflight`; `CLAUDE.md` "Verify before you claim". |
| "Have Claude show evidence rather than asserting success" | `11-agentic-sdlc.md` "Demonstrate, do not assert." |
| "Use plan mode to separate exploration from execution" ... "Planning is most useful when you're uncertain about the approach, when the change modifies multiple files ... If you could describe the diff in one sentence, skip the plan." | `CLAUDE.md` "Plan before you build. Start non-trivial work in plan mode"; spec SHOULD. Scope qualifier ("non-trivial") matches the docs' callout. |
| "Use hooks for actions that must happen every time with zero exceptions." "Unlike CLAUDE.md instructions which are advisory, hooks are deterministic" | Four hook events wired; `11-agentic-sdlc.md` "Enforcement that matters is implemented as a hook". |
| "Define specialized assistants in `.claude/agents/` ... with their own set of allowed tools" | Four subagents with `tools:` lists; reviewers additionally carry `disallowedTools`. |
| "Before treating a task as done, have a subagent review the diff in a fresh context and report gaps." "A fresh context improves code review since Claude won't be biased toward code it just wrote." | `CLAUDE.md` "You are not the right reviewer of your own change"; `11-agentic-sdlc.md` reviewers MUST run before end-of-pass. |
| "Create `SKILL.md` files in `.claude/skills/` to give Claude domain knowledge and reusable workflows." | Six skills. |
| "Permission allowlists: permit specific tools you know are safe" | Read-only `brew`/`port`/`cargo` allow list. |
| "Check CLAUDE.md into git so your team can contribute." | Checked in. |
| CLAUDE.md include column: "Bash commands Claude can't guess", "Architectural decisions specific to your project", "Developer environment quirks", "Common gotchas or non-obvious behaviors" | `CLAUDE.md` "The toolchain", "Two things that are easy to get wrong here". |
| "Use `--allowedTools` to scope permissions for batch operations ... which matters when you're running unattended." | `11-agentic-sdlc.md` CI: "Any CI job granting an agent tool access declares its allowed tools explicitly." |
| "The most useful specs are self-contained: they name the files and interfaces involved, state what is out of scope, and end with an end-to-end verification step" | `prompts/` work orders with scope, acceptance list, report (pass protocol). |
| "Use subagents to keep research out of it [the main context]" | `provider-researcher`, "Run independent items in parallel". |

### 4.2 Recommendations the repo does not currently follow (or does not mention)

| Docs recommendation (quoted) | Repo status |
|---|---|
| Stop-hook cap: "Claude Code overrides the hook and ends the turn after 8 consecutive blocks." | `11-agentic-sdlc.md` and `.claude/README.md` describe the gate as blocking "while an objective check fails" with no mention of the 8-block ceiling. The gate is therefore weaker than the spec states. |
| "`/goal` condition. A separate evaluator re-checks it after every turn and Claude keeps working until the goal resolves." (`/goal` "can't run while [`disableAllHooks`] is set, because it depends on hooks." — settings-reference) | Not mentioned anywhere in the repo. |
| "Use `disable-model-invocation: true` for workflows with side effects that you want to trigger manually." | Spec SHOULD says the same, but of six skills only `preflight` sets the field, and it sets it to `false`. `adr`, `spec-evolution`, `decision-pending`, `research-note`, `pass-report` all write files and omit the field. Internal inconsistency between spec and `.claude/skills/`. |
| "Customize compaction behavior in CLAUDE.md with instructions like `"When compacting, always preserve the full list of modified files and any test commands"`" | No compaction instruction in `CLAUDE.md`. (Mitigated in part by the `SessionStart` hook matching `compact` and by project-root CLAUDE.md re-injection.) |
| "Run `/context` to confirm Claude loaded the file." | Repo never tells the reader how to confirm the `@AGENTS.md` import resolved. |
| "For a checked-in CLAUDE.md, run `/doctor` and Claude proposes cuts for content it can derive from the codebase." Exclude: "File-by-file descriptions of the codebase", "Long explanations or tutorials". | `AGENTS.md` "Where things are" table and "The invariants, in one breath" are the derivable/duplicated kind the docs (and the repo's own MUST NOT) exclude. |
| "If Claude keeps skipping one instruction, add emphasis such as 'IMPORTANT' to that line alone. If you emphasize many lines, none of them stands out." | Not used; `CLAUDE.md` uses bold lead-ins on several paragraphs instead. Neutral observation. |
| "Worktrees: run separate CLI sessions in isolated git checkouts so edits don't collide" | Not mentioned; `.gitignore` and `.claude/README.md` do not discuss per-worktree `settings.local.json` / `CLAUDE.local.md` behaviour that the memory page describes. |
| "Tell the reviewer to flag only gaps that affect correctness or the stated requirements, and treat the rest as optional." "A reviewer prompted to find gaps will usually report some, even when the work is sound". | Reviewer agents' descriptions were read (frontmatter only); whether their bodies scope findings this way was not checked. |
| "With `claude -p "your prompt"` ... The run still creates a resumable session unless you pass `--no-session-persistence`." | CI section names `claude-code-action` (not on the fetched pages, so not verified here) and says nothing about `-p` / session persistence. |
| "auto mode is the built-in starting permission mode" on Pro/Max/Team; "Switch to Manual mode when you want to approve edits and commands yourself." | The spec's model of the boundary assumes a human prompt; see 2.4. |
| "Install a code intelligence plugin" for typed languages (Rust is one of the pre-built LSP plugins per the plugins page: "For common languages like TypeScript, Python, and Rust, install the pre-built LSP plugins from the official marketplace"). | Not mentioned. Factual note only. |

---

## 5. Summary of drift (by severity, as observed — not recommendations)

1. **Inert deny rules.** `Edit(/opt/local/**)`, `Edit(/usr/local/**)`,
   `Edit(/opt/homebrew/**)`, `Edit(/Library/**)` anchor at the project
   directory under the documented syntax; absolute form is `//path`.
   (permissions page, read 2026-09-02.) The hook layer still covers them.
2. **Stop gate ceiling.** Docs: the Stop hook is overridden "after 8
   consecutive blocks"; spec says it blocks while a check fails.
   (best-practices, read 2026-09-02.)
3. **Auto mode is now the default starting mode** on Pro/Max/Team, and plan
   mode runs classifier-approved commands; the spec's "human reading a plan"
   boundary description and its plan-mode assumptions predate this.
   `defaultMode` is settable from the project file except for `auto`/`bypass`.
   (best-practices, permissions, settings, settings-reference, read 2026-09-02.)
4. **Skill frontmatter vs spec SHOULD.** Five side-effect skills lack
   `disable-model-invocation: true`; `preflight` sets it to `false`.
5. **"Fragile" citation.** The permissions page no longer says that; it
   documents wrapper stripping, compound splitting, and that Read/Edit denies
   "don't apply to arbitrary subprocesses, so for OS-level enforcement enable
   the sandbox".
6. **Unrecorded per-repo state.** Auto memory is on by default and lives in
   `~/.claude/projects/<project>/memory/`; the spec says `current-pass` is the
   only per-session state. `autoMemoryEnabled` is project-settable.
7. **`CLAUDE.local.md` not in `.gitignore`**, and not mentioned by
   `.claude/README.md`.
8. **Confirmed unchanged:** `CLAUDE.md` project scope; `@AGENTS.md` import
   (now the documented pattern, verbatim); `disableBypassPermissionsMode:
   "disable"` is the exact and only value, a strictest-wins lock; `env`,
   `hooks`, `allow/deny/ask` shapes; 200-line per-file target; standalone
   `.claude/` is what the plugins page prescribes for project-specific
   configuration.
