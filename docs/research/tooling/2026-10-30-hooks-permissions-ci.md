<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification: hooks, permissions, headless, GitHub Actions (2026-10-30)

Date read: **2026-10-30**, by a read-only subagent. Nothing was run against `.claude/` or the workflows, and `test-hooks.sh` was not run. The previous note is [`2026-10-26-hooks-permissions-ci.md`](2026-10-26-hooks-permissions-ci.md). Here, "changed" means a difference between that note's claims (or the files it described) and today's pages and files. It is not a page diff.

**Verdict:** no contract break. Every hook still reads documented input fields, and each one prints exactly one JSON object whenever it uses the JSON form. Since 2026-10-26 the repository has changed in four commits:
- `00e42e5`: push and merge refusals in `guard-provider-mutation.sh`.
- `22b7fdf`: both Claude workflows rewritten (DP-039 (b)).
- `12220fe`: `stop-gate.sh` elevation-token scan.
- `test-hooks.sh`: new cases.

The workflow rewrite closes five items from the previous note. The docs closed one more (the `find` item). Two items are new and both concern the guard-edit escape's prompt channel (§3 items 1–2). One new documented mechanism, mods, weakens the deny and hook layers in some session setups (§3 item 3).

## Sources (fetched 2026-10-30, read with targeted extraction, none moved)

| Tag | URL (fetched as the `.md` variant directly, as in the previous note; the non-`.md` URLs were not tried) |
|---|---|
| [H] | https://code.claude.com/docs/en/hooks.md (283 KB) |
| [P] | https://code.claude.com/docs/en/permissions.md |
| [HL] | https://code.claude.com/docs/en/headless.md |
| [GA] | https://code.claude.com/docs/en/github-actions.md |

All four pages returned HTTP 200. Every fact below is cited as `[tag §section]`, read 2026-10-30.

**Not fetched, so unverified here:** `cli-reference`, `settings-reference`, `env-vars`, `permission-modes`, `terminal-config`, `worktrees`, `plugins/mods/*`, and the claude-code-action repository at the pinned SHA.

The workflows' comments cite these pages as read 2026-10-28. Specifically unverified today:
- the `--tools` and `--setting-sources` flag definitions themselves;
- `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`;
- `autoMemoryEnabled`;
- the action's `display_report` input, which is not on [GA]'s input table.

## 1. Contracts the repository uses — still documented, still met

### PreToolUse deny
Still documented: `hookSpecificOutput.{hookEventName, permissionDecision, permissionDecisionReason}` with exit 0. `permissionDecision` now lists `allow`/`deny`/`ask`/`defer` [H §JSON output].

The repository meets it in `lib.sh:91-104` (`mpb_deny`), used by:
- `guard-provider-mutation.sh:49`
- `guard-protected-paths.sh:99-238`
- `commit-gate.sh:47,50,82,86`

Exit 2 is still a blocking error on PreToolUse, and exit 1 without JSON is still non-blocking [H §Exit code output]. `lib.sh:66-87` uses exit 2 only for unreadable input or a missing parser.

### Input fields
`tool_input.command`, `tool_input.file_path`, `session_id`, `prompt`, `stop_hook_active` and `cwd` are documented [H §Common input fields, §UserPromptSubmit input, §Stop input].

`tool_input.notebook_path` is named only by [P §Tool-specific rules] as NotebookEdit's primary field. [H] still has no NotebookEdit input table. It is read at `guard-protected-paths.sh:24`.

New common fields that no script reads, with no impact: `prompt_id`, `scratchpad_dir`, and UserPromptSubmit's `session_title` [H].

### `additionalContext`, `systemMessage` and plain stdout (changed)
The cap is still 10,000 characters per string. Over the cap, Claude Code **now saves the text to a file in the session directory and passes a path plus the first 2,000 characters**. It does not truncate, and the cap has no setting [H §JSON output].

`lib.sh:111-115` truncates to the last 9,000 characters, so the cap is never reached, and the tail it keeps is the right one for linter output. Two fixes to `lib.sh:111`: its comment's read date (2026-09-02) is stale, and it does not record the new over-limit behaviour.

A `<system-reminder>` tag inside these strings is now escaped to `&lt;` [H]. No script emits one.

`mpb_message` (`lib.sh:130-140`, systemMessage) and the Stop `reason` are not truncated. [H] does not list `reason` among the capped strings.

### Stop
- `{decision:"block",reason}` with exit 0 still applies.
- The 8-consecutive-continuation cap still resets on a tool call, and `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` still raises it [H §Stop decision control].
- Exit 2 with no stdout and a missing-file stderr is still non-blocking on Stop [H §Exit code output].

`stop-gate.sh:47,282` and `lib.sh:146-156` use the JSON form.

[H] now documents `additionalContext` on Stop as an alternative that continues the turn under the same loop protections. That confirms `lib.sh:106-108`'s reason for using `systemMessage` for warnings.

### UserPromptSubmit
- Exit 2 blocks the prompt.
- The default timeout is now stated as 30 s [H §timeout].

`settings.json:221` sets 10 explicitly.

Plain stdout is injected as a system reminder [H §UserPromptSubmit decision control]. That makes `arm-guard-edit.sh:153-164`'s plain-text "nothing armed" messages valid context.

### SessionStart
Sources are `startup|resume|clear|compact|fork`; `fork` is new [H §Matcher patterns]. `session-start.sh:24` has no matcher, so it deletes the marker on every source, `fork` included. That is correct.

### Exit 1, timeouts, and failure
A timed-out PreToolUse hook does not block unless `onFailure: "block"` (v2.1.295+) [H §Timeouts]. `commit-gate.sh:69-83` rests on this (watchdog at 1,380 s, below the 1,500 s timeout), and it still holds.

### Configuration fields
`matcher`, `timeout`, `statusMessage`, shell-form `command` and `"${CLAUDE_PROJECT_DIR}"` in double quotes are all documented. [H §Security best practices] says: "In shell form, wrap it in double quotes."

### Permission rule forms in `settings.json`
- `Bash(prefix *)`, `Edit(//abs/**)`, `Edit(~/…)` and `Edit(./…)` are valid.
- `./path` means "relative to current directory", while `/path` anchors at the settings source [P §How path patterns match, §Where a pattern anchors].
- `WebFetch(domain:…)`, the `ask` rule, and `disableBypassPermissionsMode: "disable"` [P §Permission modes] are valid.

### CI flags and inputs
Documented:
- `--permission-mode dontAsk` [HL §Auto-approve tools; P §Permission modes]
- `--allowedTools` [HL]
- `--max-turns` [GA §Cost]
- `--setting-sources user` ("reads neither the project's settings files nor its `.mcp.json`") [P §What runs before you trust a folder]
- `permissions.defaultMode` and `permissions.blockReadsOutsideWorkingDirectories` as setting names [P]
- `anthropic_api_key`, `github_token` ("When omitted … authenticates as the Claude GitHub App"), `prompt` (a non-empty value means automation mode), `settings` (a path is accepted) and `claude_args` [GA §Action parameters, §Interactive and automation modes]

Omitting `id-token: write` is consistent: [GA] ties it to App authentication and to federation, and neither is used.

`-p` hooks run without a trust step, which keeps the base checkout's hooks active in CI [H §Workspace trust].

## 2. Resolved since 2026-10-26

| Previous item | Resolution | Where |
|---|---|---|
| §2.5 `dontAsk` in CI | `--permission-mode dontAsk` plus `defaultMode: "dontAsk"` in the derived settings | `claude.yml:340,434`; `claude-code-review.yml:283,342` |
| §3.5 (part) no `--max-turns` on review | `--max-turns 30` | `claude-code-review.yml:343` |
| §3.6 review `pull-requests: write` | job is `contents: read` only | `claude-code-review.yml:182-183` |
| §3.8 comments overstate allow rules | allow and ask rules are deleted from the derived settings, and the comment says why [P §What runs before you trust a folder] | `claude.yml:327-343` |
| §3.11 unpinned `plugin_marketplaces` | plugin and marketplace removed | `claude-code-review.yml:28-33` |
| §3.4 (part) `Bash(find *)` allows `-delete`/`-exec` | [P §Exec wrappers and `find` actions] now says a prefix rule "can't auto-approve" `find` with `-exec`, `-delete`, `-fprint` or `-files0-from`; in Manual mode they prompt | docs change; nothing to fix |

## 3. Guidance not followed / findings

### 1. New: documented non-human sources of UserPromptSubmit
[H §UserPromptSubmit] now lists three: a session-scoped scheduled task or `/loop` iteration, a background subagent reporting back, and a message from another session.

`arm-guard-edit.sh:201-219` filters `<task-notification`, `<agent-message`, `<system-reminder` and `[SYSTEM NOTIFICATION` only. In this session's own tooling (observed in the SendMessage tool description, not on a docs page), a cross-session message arrives as `<cross-session-message from=…>`, which the list does not name. A scheduled-task prompt carries no envelope at all.

[H] documents no input field that tells these sources apart from a typed prompt.

Fix: add the cross-session envelope to the case list. Record in DP-022 that the documented list of non-human sources now exists and has no distinguishing field.

### 2. Carried, still open: pasted text arms the escape
[H §UserPromptSubmit input] still documents pasted text arriving between `<pasted_content id="…">` and `</pasted_content id="…">` lines ("account for those lines if your hook parses the prompt"). The loop at `arm-guard-edit.sh:223-231` still treats a column-zero token inside such a block as arming.

Fix: skip lines between the markers.

### 3. New: mods can override deny rules and hooks
[P §Extend permissions with hooks] says that with neither managed settings nor a Team/Enterprise `/login` as the credential (API key or `CLAUDE_CODE_OAUTH_TOKEN` included), a mod that handles `tool.check` "can approve a call that a deny rule refuses or a `PreToolUse` hook blocked". In `dontAsk` mode it can approve "a call that nothing but the mode would deny" [P; HL §Auto-approve tools].

That bounds what `settings.json:4-99` and the three guards can claim locally. It also bounds the CI runs, which use an API key. Whether the pinned action or the runner installs any mod is unverified.

No repository-level setting to refuse mods appears on [P]. Fix: state this in `11-agentic-sdlc.md`'s enforcement list (the boundary holds absent mods, or under managed settings or a Team login), and record it as a dated fact. The mods pages were not fetched.

### 4. Carried: hooks in a worktree read the wrong tree
[H §Reference scripts by path] says `${CLAUDE_PROJECT_DIR}` stays at the start root, while `cwd` "follows Claude". No script reads `cwd`. Affected:
- `guard-provider-mutation.sh:153,157` (push remote and branch)
- `stop-gate.sh:40`
- `commit-gate.sh:45`

Fix: take repository state from `cwd`; keep `CLAUDE_PROJECT_DIR` for the hooks and the marker.

### 5. Carried: no-space trailing wildcards
"The space before a trailing `*` is part of the rule" [P §Wildcard patterns]. Allow rules without the space in `settings.json`:
- lines 105, 106, 113 (`git status*`, `git diff*`, `git branch*`, which matches `git branch -D`)
- lines 114-117 (scripts)
- lines 136-138, 140, 144, 146-147, 155-156, 160 (brew/port/system reads)
- lines 170-176, 180, 182-183 (cargo, rustc, rustup)

Fix: `Bash(cmd *)`. The no-space deny rules over-match, which is the safe direction.

### 6. New, minor: `./` rules are cwd-relative
`Edit(./rust-toolchain.toml)` and `Edit(./docs/spec/01-invariants.md)` (`settings.json:97-98`) resolve "relative to current directory" [P]. A `/rust-toolchain.toml` rule would anchor at the primary working directory in project settings, which is more robust if the cwd moves. Whether the cwd follows a Bash `cd` for rule matching is not stated; that part is inference.

In CI's derived **user** settings, a `/` rule would anchor at `~/.claude` instead [P §Where a pattern anchors]. So any rewrite must keep `./` there or use `//` absolute paths.

`guard-protected-paths.sh:128,133` covers both files regardless.

### 7. Carried: `onFailure: "block"` not used
It is documented (v2.1.295+). With it, a PreToolUse failure "blocks the tool call", and a missing or crashing script counts as a failure [H §Block the action when a hook fails]. It is absent at `settings.json:233,239,250`.

It has no effect on Stop. Do not put it on `arm-guard-edit.sh` (UserPromptSubmit): there a failure blocks the prompt.

Unverified: the Claude Code version bundled by the pinned action.

### 8. Carried: no `concurrency:` in either Claude workflow
[GA §Cost] says: "Use GitHub's concurrency controls to limit parallel runs." `claude.yml` and `claude-code-review.yml` have none; `release.yml:65` does. Low value, because the environment reviewer serialises approvals.

### 9. Carried: `.claude/settings.local.json` (the operator's file, git-ignored)
- `Write(//private/tmp/claude-501/**)` (line 22) never matches [P §`Write(path)` … rules never match]; use `Edit(...)`.
- `Bash(rm -rf *)` (line 21) is still allowed.

### 10. Carried, a deliberate difference
`stop-gate.sh:277-282` blocks again when `stop_hook_active` is true and relies on the cap. [H] says to check the flag "to avoid blocking on a condition that will never resolve". Recorded once.

## 4. Documented mechanisms not used that would enforce a stated rule

1. **`onFailure: "block"`** on the three guards. See §3 item 7.
2. **`ConfigChange`** (sources `user_settings`, `project_settings`, `local_settings`, `policy_settings`, and the new `skills`). It can block a change "from taking effect" [H §ConfigChange, §Exit code 2 behavior]. It would close the settings half of DP-020 (a session editing its own controls). The open question, still untested, is whether it fires on a Bash-redirect write.
3. **`FileChanged`** (new to this note). It is watcher-based and "runs the hook no matter what changed the file", Bash included, but it cannot block [H §FileChanged]. It is a detection layer for the gap `arm-guard-edit.sh:65-71` admits: a marker forged through Bash. Caveat: the matcher registers "a literal filename in the working directory", and whether a subdirectory path such as `.claude/.guard-edit-marker` is accepted is not stated.
4. **`permissions.disableAutoMode: "disable"`** [P §Permission modes], beside `settings.json:192`. Missing both locally and in the CI derive step (`claude.yml:338-343`).
5. **Exec form** (`command` plus `args`). It is preferred for path placeholders and needs no quoting [H §Exec form and shell form]. A preference, not a defect.
6. **`--bare`** [HL §Start faster with bare mode]. Not needed while `--setting-sources user` plus the `work/` scrub hold, because `--bare` would also drop the base checkout's hooks, which CI keeps on purpose.

## Recommendations (not adopted; `.claude/` and `.github/workflows/` wait for the owner)

In order of value:
1. §3 items 1–2: the prompt-channel filters, which are a security property of the escape.
2. §3 item 3: record the mod caveat in the enforcement claims.
3. §3 item 7: `onFailure: "block"`.
4. §3 item 4: read `cwd` in a worktree.
5. §3 item 5: spaces in the allow wildcards.
6. §4 items 2–4.
7. §3 items 6 and 8.
8. `lib.sh:111` comment date.
