# Verification: Claude Code hooks, permissions and CI against current docs

Read date for every documentation fact below: **2026-09-02**. Repository files
read read-only; nothing in the repository was modified.

Pages fetched (all resolved, none 404'd, none redirected):

| Page | URL | Status |
|---|---|---|
| Hooks reference | https://code.claude.com/docs/en/hooks (and `/hooks.md`) | fetched |
| Hooks guide ("Automate actions with hooks") | https://code.claude.com/docs/en/hooks-guide | fetched |
| Permissions ("Configure permissions") | https://code.claude.com/docs/en/permissions | fetched (no redirect to `/iam`) |
| Headless ("Run Claude Code programmatically") | https://code.claude.com/docs/en/headless | fetched |
| GitHub Actions | https://code.claude.com/docs/en/github-actions | fetched |
| Settings reference (for `permissions.*` keys) | https://code.claude.com/docs/en/settings-reference | fetched |

Caveat on method: the hooks reference is ~310 KB; the WebFetch summariser
truncates it, so the Stop / SessionStart / decision-control facts were taken
from the full page indexed via `ctx_fetch_and_index` and quoted from the
indexed chunks. Facts marked "(not verified)" are ones I could not locate
verbatim and are therefore *not* asserted.

---

## 1. Hooks

### 1.1 Documented event list (hooks reference, read 2026-09-02)

SessionStart, Setup, UserPromptSubmit, UserPromptExpansion, PreToolUse,
PermissionRequest, PermissionDenied, PostToolUse, PostToolUseFailure,
PostToolBatch, Notification, MessageDisplay, SubagentStart, SubagentStop,
TaskCreated, TaskCompleted, Stop, StopFailure, TeammateIdle,
InstructionsLoaded, ConfigChange, CwdChanged, DirectoryAdded, FileChanged,
WorktreeCreate, WorktreeRemove, PreCompact, PostCompact, PreModelSwitch,
PostModelSwitch, Elicitation, ElicitationResult, SessionEnd.

### 1.2 stdin JSON

Common input fields (hooks reference, "Common input fields" table, verbatim
names): `session_id`, `prompt_id`, `transcript_path`, `cwd`, `permission_mode`
(`"default"|"plan"|"acceptEdits"|"auto"|"dontAsk"|"bypassPermissions"`;
"Not all events receive this field"), `effort` (`{level}`), `hook_event_name`;
plus `agent_id`, `agent_type` inside a subagent.

PreToolUse (hooks-guide "Hook input", verbatim example):
```json
{ "session_id": "abc123", "cwd": "/Users/sarah/myproject",
  "hook_event_name": "PreToolUse", "tool_name": "Bash",
  "tool_input": { "command": "npm test" } }
```
Reference adds `tool_use_id`. For Bash, `tool_input.command` holds the shell
command. For Edit/Write the primary field is `file_path`; for NotebookEdit it
is `notebook_path` (permissions page, "Match by input parameter":
"`command` for Bash and PowerShell, `file_path` for Read, Edit, and Write,
`path` for Grep and Glob, `notebook_path` for NotebookEdit, and `url` for
WebFetch").

PostToolUse: same plus `tool_response`, `tool_use_id`, `duration_ms`.

Stop (hooks reference, "Stop input", verbatim excerpt):
```json
{ "session_id": "abc123", "transcript_path": "...", "cwd": "/Users/...",
  "permission_mode": "default", "hook_event_name": "Stop",
  "stop_hook_active": true,
  "last_assistant_message": "I've completed the refactoring. Here's a summary...",
  "background_tasks": [ ... ], "session_crons": [ ... ] }
```
`stop_hook_active` IS documented (reference Stop input, SubagentStop input, and
hooks-guide "Stop hook hits the block cap": "Claude Code overrides a Stop hook
after it blocks eight times in a row without progress. Your hook script needs
to check whether it already triggered a continuation. Parse the
`stop_hook_active` field from the JSON input and exit early if it's `true`",
cap adjustable with `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`).

SessionStart: hooks-guide says "`SessionStart` hooks get a `source` of
`startup`, `resume`, `clear`, `compact`, or `fork`". (A WebFetch summary also
reported a `session_start_reason` field; not verified verbatim, so not
asserted. The repository's script reads neither, so this does not matter.)

### 1.3 Exit-code contract (hooks reference "Exit code 2 behavior per event", verbatim rows)

| Event | Can block? | Exit 2 |
|---|---|---|
| PreToolUse | Yes | Blocks the tool call |
| PostToolUse | No | Shows stderr to Claude; the tool already ran |
| Stop | Yes | Prevents Claude from stopping, continues the conversation |
| SessionStart | No | Exit code is ignored; hook can run side effects |
| SubagentStop | Yes | Prevents the subagent from stopping |
| UserPromptSubmit | Yes | Blocks prompt processing and erases the prompt |

Exit 0 (reference "Exit code 0"): "For most events, Claude Code writes stdout
to the debug log and doesn't show it in the transcript. The exceptions are
`UserPromptSubmit`, `UserPromptExpansion`, `SessionStart`, and
`PostModelSwitch`, where Claude Code adds plain-text stdout as context". Stdout
that starts with `{` and ends with `}` is parsed as JSON. Hooks-guide: "For a
`PreToolUse` hook this [exit 0] doesn't approve the tool call: the normal
permission flow still applies."

Other non-zero exit (hooks-guide): if stdout is a schema-valid JSON object the
exit code is ignored and the JSON decides; otherwise a non-blocking error
("Failed with non-blocking status code: <first stderr line>").

Stderr on exit 0: reference (SessionStart section) — "Stderr from a
SessionStart hook that exits 0 is informational only". No event is documented
as delivering exit-0 stderr to Claude.

### 1.4 JSON stdout contract

- PreToolUse (reference + guide, verbatim):
  `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"..."}}`
  values `allow|deny|ask` (+ `defer` in `-p` mode); `updatedInput`,
  `additionalContext` also under `hookSpecificOutput`. Reference note:
  "PreToolUse previously used top-level `decision` and `reason` fields, but
  these are deprecated for this event... Other events like PostToolUse and
  Stop continue to use top-level `decision` and `reason` as their current
  format."
- Stop / SubagentStop / PostToolUse / UserPromptSubmit / PreCompact /
  ConfigChange: top-level `decision: "block"` + `reason` ("Required when
  `decision` is `"block"`"). Stop and SubagentStop also accept
  `hookSpecificOutput.additionalContext` ("Non-error feedback for Claude. The
  conversation continues... shown in the transcript as hook feedback rather
  than a hook error"; same loop protections). "A hook that blocks by exiting 2
  routes the same way as `reason`".
- `additionalContext`: "Return `additionalContext` inside `hookSpecificOutput`
  alongside the event name" (reference); hooks-guide: "if you place it at the
  top level of the JSON, Claude Code silently ignores it". Applies to
  SessionStart, SubagentStart, UserPromptSubmit, PreToolUse, PostToolUse,
  PostToolUseFailure, PostToolBatch, Stop, SubagentStop, PostModelSwitch.
- Universal fields: `continue`, `stopReason`, `suppressOutput`,
  `systemMessage` (reference "JSON output"; exact table not quoted here).
- Whether `hookEventName` is mandatory is not stated; every documented example
  includes it.

### 1.5 Configuration fields (reference "Common fields", verbatim)

`type` (`"command"|"http"|"mcp_tool"|"prompt"|"agent"`), `if` (permission-rule
filter, tool events only), `timeout` ("Seconds before canceling... Defaults:
600 for `command`, `http`, and `mcp_tool`; 30 for `prompt`; 60 for `agent`"),
`statusMessage` ("Custom spinner message displayed while the hook runs"),
`once` (skill frontmatter only). Command hooks add `command`, `args`, `async`,
`asyncRewake`, `shell`. `${CLAUDE_PROJECT_DIR}` is documented ("the project
root where the session started"; kept unchanged when entering a worktree),
also `${CLAUDE_PLUGIN_ROOT}`, `${CLAUDE_PLUGIN_DATA}`. `CLAUDE_ENV_FILE` is
documented for SessionStart/CwdChanged. `disableAllHooks` is documented.

Matchers: SessionStart `startup`, `resume`, `clear`, `compact`, `fork`;
PreCompact/PostCompact `manual`, `auto`; tool events match tool name as regex
(`Edit|Write`, `mcp__.*`); "UserPromptSubmit, PostToolBatch, Stop,
TeammateIdle, ... no matcher support, always fires"; "If you add a `matcher`
field to an event without matcher support, it is silently ignored."

Prompt-based hooks exist (`"type": "prompt"`, `prompt` with `$ARGUMENTS`,
optional `model`, default timeout 30 s, returns `ok`/`reason` JSON) and
agent-based hooks exist (`"type": "agent"`, default timeout 60 s, up to 50
tool turns). Hooks-guide sections "Prompt-based hooks", "Agent-based hooks",
"HTTP hooks".

Hooks and permission modes (hooks-guide): "`PreToolUse` hooks fire before any
permission-mode check, in every permission mode, including `dontAsk`. A hook
that returns `permissionDecision: "deny"` blocks the tool even in
`bypassPermissions` mode or with `--dangerously-skip-permissions`."

### 1.6 Comparison with the repository hooks

`.claude/settings.json` (hooks block, lines 141-205)

- L144 `"matcher": "startup|resume|clear|compact"` — valid, but the documented
  fifth value `fork` is missing, so a forked session gets no context
  injection. Fix: `"startup|resume|clear|compact|fork"` (or omit the matcher).
- L194 `"matcher": ""` on Stop — Stop has no matcher support; silently
  ignored. Harmless; remove for accuracy.
- L148/161/172/185/198 `${CLAUDE_PROJECT_DIR}/.claude/hooks/x.sh` unquoted in
  shell form. Documented examples write `"\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/x.sh"`
  or use exec form (`"args": []`). Breaks only if the checkout path contains
  spaces. Low.
- `timeout` and `statusMessage` fields: documented; values are within
  defaults (Stop 600 == default).

`.claude/hooks/lib.sh`

- L53-68 `mpb_deny`: emits `hookSpecificOutput.{hookEventName,permissionDecision:"deny",permissionDecisionReason}` and exits 0 — matches the current (non-deprecated) PreToolUse contract exactly.
- L70-82 `mpb_context`: nests `additionalContext` under `hookSpecificOutput` — matches the documented shape (top-level would be ignored).
- L44-51 `mpb_require_json_parser`: exit 2 with stderr — on PreToolUse this blocks; correct for the fail-closed intent.
- L18-42 `mpb_json_field`: swallows jq/python parse failures into an empty string (`|| printf ''`, `sys.exit(0)` on JSON error). See the guard finding below.

`.claude/hooks/guard-provider-mutation.sh` (PreToolUse, Bash)

- L30-32 reads `tool_input.command`; correct field. But L32 `[ -n "$COMMAND" ] || exit 0` means a *malformed* stdin (parser present, JSON unparseable) yields an empty command and the guard exits 0 = fails **open**, contradicting the header comment (lib.sh L8-10) and `11-agentic-sdlc.md` L113-115 ("A guard hook that cannot read its input fails closed"). Fix: have `mpb_json_field` distinguish "parse failed" (return non-zero) from "field absent", and exit 2 on parse failure.
- Exit 0 with no output on the allow path: correct ("doesn't approve... normal permission flow still applies").
- Deny path uses exit 0 + JSON, not exit 2: matches the guide's "Choose one approach per hook". Consistent.
- Hook fires even in `bypassPermissions`: documented, so the second layer the spec wants is real.
- Optional: the documented `if` field (`"if": "Bash(*)"`-style) is not needed; matcher already `Bash`.

`.claude/hooks/guard-protected-paths.sh` (PreToolUse, Edit|Write|NotebookEdit)

- L17-18 reads `tool_input.file_path` then `tool_input.notebook_path` — matches the documented primary fields for Edit/Write and NotebookEdit.
- L19 `[ -n "$FILE" ] || exit 0` — same fail-open-on-parse-failure as above.
- L21-28 path classes match the deny intent; note it matches literal paths only (no symlink resolution). Permission rules do check symlink targets for deny; the hook does not. Acceptable as a second layer.

`.claude/hooks/post-edit-checks.sh` (PostToolUse, Edit|Write)

- L70-73 writes findings to **stderr and exits 0**. Per the docs, exit-0 stderr is informational/debug only and exit-0 stdout for PostToolUse goes to the debug log, so the shellcheck/JSON/link/SPDX notes never reach Claude. Documented ways to deliver non-blocking feedback on PostToolUse: (a) `{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"..."}}` on exit 0 (shown "next to the tool result"), or (b) exit 2 ("Shows stderr to Claude; the tool already ran" — does not block, but is labelled a hook error). Recommended fix: emit `additionalContext` via a `mpb_context "PostToolUse" "$notes"` call instead of stderr. This is the most consequential mismatch: the spec's "cheap per-file checks" (11-agentic-sdlc.md L110) currently produce output nobody sees.
- L16 `mpb_have jq || mpb_have python3 || exit 0` — fails open by design (non-guard); acceptable and consistent with "Never blocks".
- L43 `grep -nP` is unsupported by BSD grep on macOS; the `||` fallback masks the error. Cosmetic.

`.claude/hooks/session-start.sh` (SessionStart)

- L49 `mpb_context "SessionStart" "$text"` — correct nested `additionalContext` shape; exit 0. SessionStart exit codes are ignored, so nothing here can block; consistent.
- Does not read stdin (`source` unused) — fine.

`.claude/hooks/stop-gate.sh` (Stop)

- Never reads stdin, so it never checks `stop_hook_active` (L21-26). The hooks-guide says the script "needs to check" it; the built-in cap (8 consecutive blocks) is the only loop guard. For a definition-of-done gate this is arguably intentional (the gate must re-run after each fix), but it is undocumented in the script and in `11-agentic-sdlc.md`. Fix: either read `stop_hook_active` and downgrade to `hookSpecificOutput.additionalContext` on the second pass, or document the reliance on the 8-block cap / `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`.
- L102-107 blocks with exit 2 + stderr — documented and routed to Claude as the reason to continue. Correct.
- L97-100 warnings go to stderr on exit 0 — same visibility problem as post-edit-checks: warnings are not delivered to Claude. Documented alternative: `hookSpecificOutput.additionalContext` on Stop ("Stop hook feedback", keeps conversation going) — note that this *continues* the turn, so a warning-only path that must not block should use `systemMessage` or simply be accepted as debug-only. Decide deliberately.
- L24 `cd "$ROOT" || exit 0` fails open if the root is missing. Low.
- Not reading stdin is not documented as an error; harmless.

`docs/spec/11-agentic-sdlc.md`

- L101-103: "Anthropic's own permissions documentation states that Bash patterns constraining arguments are fragile" — still true verbatim (permissions page Warning: "Bash permission patterns that try to constrain command arguments are fragile").
- L108 obligation for PreToolUse(Bash) matches the hook.
- L110 "Never blocks" for PostToolUse — consistent with docs (PostToolUse cannot block), but see the visibility finding.
- L113-115 fail-closed MUST is violated by the parse-failure path (above).

---

## 2. Permissions

### 2.1 Documented rule syntax (permissions page, read 2026-09-02, verbatim where quoted)

- `Bash(cmd *)` is the primary form. "The `:*` suffix is an equivalent way to
  write a trailing wildcard, so `Bash(ls:*)` matches the same commands as
  `Bash(ls *)`... The `:*` form is only recognized at the end of a pattern."
  Both are current; the dialog writes the space form.
- "A `*` at the end, with a space before it, also matches the bare command.
  `Bash(ls *)` matches `ls`". "The space before a trailing `*` is part of the
  rule. `Bash(ls *)` requires a space after `ls`, so `lsof` doesn't match.
  `Bash(ls*)` has no space, so it matches `lsof` too."
- "A `*` can go anywhere in the rule"; a rule with no `*` is an exact match.
- Compound commands: "The recognized command separators are `&&`, `||`, `;`,
  `|`, `|&`, `&`, and newlines. A rule must match each subcommand
  independently." Wrappers stripped before matching: `timeout`, `time`,
  `nice`, `nohup`, `stdbuf`, `command`, `builtin`, `noglob`, bare `xargs`.
  "A deny or ask rule matches past any leading assignment, so `Bash(rm *)` in
  deny still matches `FOO=bar rm -rf tmp/`." Not stripped: `arch`, `env`,
  `npx`, `direnv exec`, etc.
- Fragility warning: still present (quoted above), recommending deny rules on
  `curl`/`wget` + `WebFetch(domain:...)` + PreToolUse hooks.
- Redirections: "Claude Code checks the target of an output redirection...
  as a file write. The check covers your `Edit` allow and deny rules".
- Edit/Read paths (gitignore syntax): `//path` = absolute from filesystem
  root; `~/path` = home; `/path` = "Path relative to the settings source"
  (project settings: `<primary working directory>/path`); `path`/`./path` =
  relative to cwd. Warning: "A pattern like `/Users/alice/file` isn't an
  absolute path. The single leading slash anchors at the settings source, not
  the filesystem root. Use `//Users/alice/file` for absolute paths." `**`
  crosses directories; `*` single segment. Edit deny covers NotebookEdit;
  rules on `Write(...)`/`NotebookEdit(...)` are accepted but never consulted.
- `WebFetch(domain:example.com)`; `domain:*.example.com` = subdomains only.
- `disableBypassPermissionsMode`: the only documented value is `"disable"`
  ("set `permissions.disableBypassPermissionsMode` or
  `permissions.disableAutoMode` to `"disable"`"; settings reference lists the
  key under `permissions`).
- Precedence: deny at any scope beats allow at any scope; managed highest.

### 2.2 Rule-by-rule check of `.claude/settings.json`

deny (L4-67):
- L5 `Bash(sudo *)` — matches `sudo ls` and bare `sudo`. Correct.
- L6 `Bash(sudo)` — redundant with L5 (trailing ` *` already matches the bare
  command). Harmless.
- L7-9 `doas *`, `chsh *`, `launchctl *` — correct.
- L10-15, 17-27 `brew <sub> *` — correct; each also matches the bare
  subcommand.
- L16 `Bash(brew update*)` — matches `brew update`, `brew update --force`,
  and also `brew update-reset`, `brew updater`. Over-broad is acceptable in a
  deny list; `Bash(brew update *)` would be the idiomatic form and still
  matches bare `brew update`. Same for L34 `port selfupdate*`, L35 `port
  sync*`, L37 `port reclaim*`. Not wrong; style.
- L28-44 `port <sub> *` — correct. Note the hook additionally denies
  `configure|extract|patch` and `brew migrate|edit`, which are not in this
  list; add for parity if desired.
- L45-47 `curl *`, `wget *`, `nc *` — exactly the documented pattern.
- L48 `Bash(git push *)` — matches the docs' own example, including bare
  `git push`.
- L49-55 `Edit(~/...)` — `~/` form is correct.
- **L56 `Edit(/opt/local/**)`, L57 `Edit(/usr/local/**)`, L58
  `Edit(/opt/homebrew/**)`, L59 `Edit(/Library/**)` — WRONG.** A single
  leading `/` in project settings anchors at the repository root, so these
  deny `<repo>/opt/local/**`, `<repo>/usr/local/**`, `<repo>/opt/homebrew/**`
  and `<repo>/Library/**`, not the real prefixes. Fix: `Edit(//opt/local/**)`,
  `Edit(//usr/local/**)`, `Edit(//opt/homebrew/**)`, `Edit(//Library/**)`.
  Until fixed, only `guard-protected-paths.sh` protects the prefixes, and
  Bash redirections into them (`echo x > /usr/local/foo`) are not caught by
  the rule check (the hook does not inspect redirections either, except the
  dotfile pattern at guard-provider-mutation.sh L69).
- L60 `Edit(~/Library/LaunchAgents/**)` — correct.
- L61-67 cargo/rustup — correct.

ask (L69-72): `Bash(git commit *)` correct; `Bash(git init*)` matches `git
init`, `git init --bare`, and any `git init…`; fine.

allow (L73-129):
- Space-less trailing `*` on L74-76, 79, 83, 85, 86, 94, 95, 98, 99, 110-116,
  120, 122, 123: each also matches longer program/subcommand names. Most are
  harmless (`brew leaves*`, `cargo fmt*`), but:
  - **L98 `Bash(arch*)`** matches `arch -x86_64 brew install foo` (rule text
    is the whole command; `arch` is not a stripped wrapper, so the
    `brew install *` deny does not see it) and also `archive…`. Fix:
    `Bash(arch)` (exact; `arch` takes no read-only arguments worth allowing).
    The PreToolUse hook does catch `… brew install` after `arch`, so the
    second layer holds today.
  - L95 `Bash(sw_vers*)`, L85 `port version*` etc.: prefer `Bash(sw_vers *)`
    form for consistency; no functional risk found.
- L78 `Bash(brew list)` redundant with L77 `Bash(brew list *)`.
- L102-109, 128 `WebFetch(domain:...)` — correct syntax.
- L127 `Bash(file *)` — allowed; docs note `file -m/-f` still prompts. Fine.
- L130 `"disableBypassPermissionsMode": "disable"` — correct value and key
  location.

Compound commands: because allow rules must match every subcommand and deny
rules match past leading assignments, `brew list && brew install x` is not
allowed by L77 and is denied by L10. No gap found beyond the `arch` and `//`
findings.

---

## 3. Headless / SDK (headless page, read 2026-09-02)

- Non-interactive: `claude -p "<prompt>"` (`--print`). Exit 0 on success,
  non-zero on failure. Piped stdin capped at 10 MB.
- `--bare`: "recommended mode for scripted and SDK calls, and will become the
  default for `-p`"; skips hooks, skills, plugins, MCP, CLAUDE.md. Without it,
  "a `-p` session runs the hooks in a project's `.claude/settings.json`... even
  in a folder you've never trusted".
- `--allowedTools "Bash(git diff *),Read"` uses permission-rule syntax; "The
  trailing ` *` enables prefix matching... The space before `*` is important:
  without it, `Bash(git diff*)` would also match `git diff-index`."
- `--permission-mode`: `auto`, `dontAsk` ("denies anything not in your
  `permissions.allow` rules or the read-only command set, which is useful for
  locked-down CI runs"), `acceptEdits`; `-p` starts in Manual (`default`)
  otherwise. `--dangerously-skip-permissions` not covered on this page.
- `--output-format text|json|stream-json`; `--json-schema` with `json`;
  `--max-turns`, `--append-system-prompt`, `--settings <file-or-json>`,
  `--mcp-config`, `--continue`, `--resume <id>`.
- Relevance to this repo: the GitHub Action runs `claude -p` on the runner
  with the project's `.claude/settings.json` hooks active (claude.yml passes
  `settings:` explicitly, and hooks run under `-p` regardless), so the Stop
  gate and both guards execute in CI. `dontAsk` is the documented mode for a
  locked-down CI run; claude.yml does not set one (the action's default
  applies).

---

## 4. GitHub Actions (github-actions page, read 2026-09-02)

Current action: `anthropics/claude-code-action@v1`. Documented inputs:
`prompt`, `claude_args`, `anthropic_api_key`, `claude_code_oauth_token`,
`github_token`, `plugin_marketplaces`, `plugins`, `settings` ("Claude Code
settings, as a JSON string or a path to a settings JSON file"),
`trigger_phrase`, `use_bedrock`, `use_vertex`, `use_foundry`; also
`allowed_bots`, `allowed_non_write_users`, `anthropic_federation_rule_id`,
`anthropic_organization_id`, `anthropic_service_account_id`,
`anthropic_workspace_id`. Full list deferred to the action's `docs/usage.md`.
`track_progress`, `use_sticky_comment`, `use_commit_signing`,
`additional_permissions`, `path_to_claude_code_executable` are NOT on this
page (not verified either way).

Deprecated (page "Upgrade from beta"): `@beta` tag; `mode` input (removed —
mode is detected from presence of `prompt`); `direct_prompt` → `prompt`;
`max_turns`, `model` → `claude_args`; `custom_instructions` →
`--append-system-prompt` in `claude_args`. `allowed_tools`/`disallowed_tools`
inputs are not mentioned; `--allowedTools` goes in `claude_args`.

Required workflow permissions (interactive example): `contents: write`,
`pull-requests: write`, `issues: write`, `id-token: write` ("required for the
Claude Code GitHub Action's default GitHub App authentication"), `actions:
read`. Review example: `contents: read`, `pull-requests: read`, `issues:
read`, `id-token: write`.

### 4.1 `.github/workflows/claude.yml`

- L11-14 triggers, L18 `if: contains(... '@claude')`, L21-26 permissions,
  L28-30 `actions/checkout@v6` + `fetch-depth: 1`, L31-33 `@v1` +
  `anthropic_api_key` — identical to the documented interactive example.
- L38 `settings: .claude/settings.json` — documented input, path form.
- L39 `claude_args: --max-turns 25` — documented.
- L20 `timeout-minutes: 30` — matches the docs' cost advice.
- No deprecated input used.
- Spec gap (`11-agentic-sdlc.md` L239-241 "Any CI job granting an agent tool
  access declares its allowed tools explicitly"): no `--allowedTools` is
  passed; the action's own default tool set plus `permissions.allow` from the
  settings file apply. Fix: add `--allowedTools "..."` (and optionally
  `--permission-mode dontAsk`) to `claude_args`. `contents: write` is what the
  docs require for interactive mode; the "no write to the default branch"
  MUST NOT (L234) is therefore a branch-protection matter, not a workflow one —
  worth a comment.

### 4.2 `.github/workflows/claude-code-review.yml`

- L11-13, L19-23, L25-34 — identical to the documented "Run a skill" review
  example (`plugin_marketplaces`, `plugins: code-review@claude-code-plugins`,
  `prompt: "/code-review:code-review --comment ..."`, `claude_args:
  '--allowedTools "mcp__github_inline_comment__create_inline_comment"'`).
  The docs explicitly say to keep that `claude_args` line. Read-only
  permissions satisfy the spec. No deprecated input. Note from the docs: on
  public repositories fork PRs get no secrets, so the review runs only for
  same-repo branches.

### 4.3 `.github/workflows/repository-validation.yml`

Plain shell; assessed for soundness:
- L25-26 `permissions: contents: read` — least privilege, good.
- L35 `sudo apt-get install shellcheck jq` — runs on the runner, outside any
  agent session; the repo's `sudo` deny is a Claude Code permission rule and
  does not apply here. `jq` is installed but `stop-gate.sh` uses `python3`
  (present on ubuntu-latest). Fine.
- L38 `shellcheck -x .claude/hooks/*.sh` — includes `lib.sh`; `-x` follows the
  `# shellcheck source=lib.sh` directives. Sound.
- L41-43 runs `stop-gate.sh` with `CLAUDE_PROJECT_DIR=${{ github.workspace }}`;
  the script tolerates missing `Cargo.toml` and missing git state. Sound.
  When `Cargo.toml` lands, `rust-toolchain.toml` will make `cargo` install the
  pinned toolchain on the runner (rustup is preinstalled); acceptable.
- L45-49 `sha256sum --check --status` with the two-space `hash  LICENSE`
  format — correct.
- Minor: `on: push` + `pull_request` double-runs on PR branches; no
  `timeout-minutes`. Cosmetic.

---

## 5. Consolidated mismatch list (file:line → documented fix)

1. `.claude/settings.json:56-59` — `Edit(/opt/local/**)`, `Edit(/usr/local/**)`,
   `Edit(/opt/homebrew/**)`, `Edit(/Library/**)` anchor at the repo root, not
   `/`. Use `//opt/local/**`, `//usr/local/**`, `//opt/homebrew/**`,
   `//Library/**`.
2. `.claude/hooks/post-edit-checks.sh:70-73` — findings written to stderr on
   exit 0 are not delivered to Claude. Emit
   `hookSpecificOutput.additionalContext` (`hookEventName: "PostToolUse"`).
3. `.claude/hooks/stop-gate.sh:21-26` — never reads `stop_hook_active`; relies
   silently on the 8-block cap. Read it and decide (downgrade to
   `additionalContext`, or document the reliance / set
   `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`). Also L97-100 warnings on stderr/exit 0
   are not delivered.
4. `.claude/hooks/lib.sh:18-42` + `guard-provider-mutation.sh:32` +
   `guard-protected-paths.sh:19` — unparseable stdin yields empty field →
   exit 0 → fails open, against `11-agentic-sdlc.md:113-115`. Make
   `mpb_json_field` return non-zero on parse failure and exit 2 there.
5. `.claude/settings.json:144` — SessionStart matcher lacks `fork`.
6. `.claude/settings.json:98` — `Bash(arch*)` allows `arch -x86_64 <anything>`
   and `archive…`; use `Bash(arch)`.
7. `.claude/settings.json:194` — `"matcher": ""` on Stop is ignored; remove.
8. `.claude/settings.json:6,78` — `Bash(sudo)`, `Bash(brew list)` redundant
   with the ` *` forms. Cosmetic.
9. `.claude/settings.json` hook `command` values — unquoted
   `${CLAUDE_PROJECT_DIR}`; docs quote it or use exec form. Low.
10. `.github/workflows/claude.yml:39` — no explicit `--allowedTools`, which
    `11-agentic-sdlc.md:239-241` requires; add to `claude_args`.
11. `.claude/settings.json:16,34,35,37,71,74-76,79,83,85,86,94,95,99,110-116,120,122,123`
    — space-less trailing `*` also matches longer command names; not wrong
    for deny, over-broad for allow; prefer `cmd *`. Style.

No page failed to fetch. No documented input used by the workflows is
deprecated.
