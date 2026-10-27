<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification: hooks, permissions, headless, GitHub Actions (2026-10-26)

Date read: **2026-10-26**, by a read-only subagent for `/tooling-recheck`, at the
start of pass 08 slice 6. `test-hooks.sh` was not run (nothing under `.claude/`
changed). Previous note: [`2026-10-22-hooks-permissions-ci.md`](2026-10-22-hooks-permissions-ci.md).
"Changed" below means a difference between that note's claims and today's pages,
not a page diff.

**Verdict:** no contract break. Every script reads the documented input fields and
emits one JSON object on each JSON path. Two new findings are about enforcement
rather than contract (§3, items 1 and 9).

## Sources (fetched 2026-10-26, read in full, none moved)

`https://code.claude.com/docs/en/hooks.md`, `/permissions.md`, `/headless.md`,
`/github-actions.md`. Not fetched, so anything depending on them is unverified:
`sandboxing`, `settings-reference`, `cli-reference`, `terminal-config`,
`worktrees`, `permission-modes`, and the claude-code-action repository's docs.

## 1. Contracts the repository uses

- **PreToolUse deny**: `hookSpecificOutput.permissionDecision: "deny"` with a reason, exit 0 — `lib.sh` `mpb_deny`, used by the three guards. Exit 2 with stderr is used only for unreadable input, which blocks as documented.
- **Input fields**: `tool_input.command`, `tool_input.file_path`, `session_id`, `prompt`, `stop_hook_active` — all documented. `tool_input.notebook_path`: named by permissions.md as NotebookEdit's field; hooks.md still has no NotebookEdit table (was unverified, now partly verified).
- **`additionalContext`**: documented for SessionStart, UserPromptSubmit, PreToolUse, PostToolUse; capped at 10,000 characters per string. `lib.sh` truncates at 9,000.
- **Stop**: `{decision:"block",reason}` with exit 0; the eight-continuation cap resets on a tool call; `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`. Exit 2 on Stop with a bare stderr is a non-blocking error, which is why `stop-gate.sh` uses the JSON form. `systemMessage` is a universal field.
- **Exit 1, timeouts**: exit 1 without JSON is non-blocking; a timed-out PreToolUse hook does not block (unless `onFailure: "block"`). `commit-gate.sh`'s watchdog rests on this, and it holds. Default timeout 600 s (30 on UserPromptSubmit); every value in `settings.json` is explicit.
- **Matchers** `Bash`, `Edit|Write|NotebookEdit`, `Edit|Write`: valid. `${CLAUDE_PROJECT_DIR}` in double quotes in shell form: compliant.
- **Permission rule forms** in `settings.json` (`Bash(prefix *)`, `Edit(//abs/**)`, `Edit(~/…)`, `WebFetch(domain:…)`, the `ask` rule, `disableBypassPermissionsMode`): valid. `settings.json` uses `Edit(...)` only, the right tool name for path rules.
- **Workflow inputs** (`anthropic_api_key`, `prompt`, `settings`, `claude_args`, `plugin_marketplaces`, `plugins`), `--max-turns`, `--allowedTools` including the inline-comment tool, `id-token: write`, `actions: read`: documented. `--disallowedTools` now verified (a denial at any level cannot be overridden).

**Fail-open residue.** Scripts that source `lib.sh` run under `set -euo pipefail`.
An unhandled non-zero exit, a missing or non-executable script, is exit 1, which
does not block. The guards fail closed only on their explicit exit-2 paths; the
comments that say they fail closed overstate it. Fix: §2 item 1.

## 2. Mechanisms not used that would enforce a stated rule

1. **`onFailure: "block"`** (v2.1.295+, `command` and `http` hooks) on the Bash group (`guard-provider-mutation.sh`, `commit-gate.sh`) and on `guard-protected-paths.sh`. It covers a failing, missing or timed-out guard. No effect on Stop; on UserPromptSubmit it would block the prompt, so not on `arm-guard-edit.sh`. Unverified: the Claude Code version the pinned action bundles. (carried, semantics now fully documented)
2. **Sandbox**, for filesystem and network enforcement that does not depend on command text; permissions.md lists `/usr/bin/curl`, `sh -c 'curl …'`, `git -C . push` and `/bin/rm` as forms a Bash rule does not stop. Key names unverified (page not fetched). (carried)
3. **`ConfigChange` hook**: fires on `project_settings` and `local_settings` changes during a session and can block. It would close the settings half of the gap the guard scripts admit (a session changing its own controls, DP-020); it does not cover `.claude/hooks/*.sh` or the marker. Whether it fires for a change made by a Bash redirect was not tested. New.
4. **`permissions.disableAutoMode`** beside `disableBypassPermissionsMode`. (carried)
5. **`--permission-mode dontAsk`** (or `--permission-prompts none`, v2.1.259+) in both Claude workflows' `claude_args`; headless.md calls `dontAsk` useful for locked-down CI. `claude -p` starts in Manual mode from v2.1.302; the bundled version is unverified. (carried)
6. **Exec form** (`command` plus `args`), which hooks.md prefers for any hook that references a path placeholder. The repository uses the documented shell-form fallback: a preference, not a defect. New.
7. **`if` field** on the commit gate: best-effort for `$()` and compound forms, so not recommended for a guard. (carried)
8. PreToolUse `permissionDecision: "ask"` for ambiguous cases. Optional.

## 3. Guidance not followed

1. **Hooks in a worktree** (new). hooks.md: with a worktree, `${CLAUDE_PROJECT_DIR}` stays at the session-start root while the input's `cwd` follows Claude. No script reads `cwd`. `stop-gate.sh` and `commit-gate.sh` validate, and `guard-provider-mutation.sh` reads the branch (`git -C "${CLAUDE_PROJECT_DIR:-.}" rev-parse --abbrev-ref HEAD`), from the main checkout, so a worktree session would be validated, and its push checked, against the wrong tree. Fix: read `cwd` for repository state; keep `CLAUDE_PROJECT_DIR` for the hooks and the marker.
2. **`Write(...)` path rules never match** (new; permissions.md). `.claude/settings.local.json` (git-ignored) has one; its `Edit(...)` sibling carries the effect. `settings.json` is clean.
3. **No-space trailing wildcards** in allow rules: the space before a trailing `*` is part of the rule, so `Bash(git branch*)` matches `git branch -D` and `Bash(git diff*)` matches `git diff-index` (headless.md's own example). About thirty allow rules in `settings.json` (`git status*`, `git diff*`, `git branch*`, the `scripts/*.sh*` entries, several `brew`/`port`/`cargo`/`rustup` reads). Fix: `Bash(cmd *)`. No-space deny rules over-match, the safe direction.
4. **Broad allows**: `Bash(find *)` in `settings.json` also allows `-delete` and `-exec` (an inference from the permissions table). `.claude/settings.local.json` allows `Bash(rm -rf *)`, and no deny rule or guard matches `rm`. That file is the operator's, not the repository's.
5. **CI cost controls**: `claude-code-review.yml` has no `--max-turns` (it has `timeout-minutes: 20`); neither workflow has a `concurrency:` group.
6. **Review workflow `pull-requests: write`**: the docs' review example uses `read`; the file's own comment says to test `read` first.
7. **Stop-hook loop guidance**: `stop-gate.sh` reads `stop_hook_active` and blocks again by design, relying on the cap. A deliberate difference, recorded once.
8. **Workflow comments overstate allow rules**: under `claude -p` in a never-trusted folder, `permissions.allow` in `.claude/settings.json` is not used (hooks and deny rules are). `claude.yml` and `claude-code-review.yml` say the checked-in permissions apply. Whether the action's `settings:` input changes that is unverified.
9. **Pasted text can arm the guard-edit escape** (new). hooks.md: in sessions that mark pasted text, it arrives between `<pasted_content id="…">` and `</pasted_content id="…">` lines in the UserPromptSubmit `prompt`. `arm-guard-edit.sh` filters only task-notification, agent-message, system-reminder and system-notification lines, so the arming token at column zero inside pasted text still arms the escape. Fix: ignore the lines between the markers.
10. Stale read dates in comments of `claude.yml`, `lib.sh`, `post-edit-checks.sh`, `arm-guard-edit.sh`, `commit-gate.sh`; the content they cite still holds.
11. `plugin_marketplaces` has no documented `#ref` pinning syntax; the pin stays unverified, as `claude-code-review.yml` says.

## Recommendations (not adopted; `.claude/` and `.github/workflows/` wait for the owner)

In order of value: §3 item 9 (the pasted-text arming gap, a security property of
the escape); §2 item 1 (`onFailure: "block"` on the three guards); §3 item 1 (read
`cwd` in a worktree); §3 item 3 (spaces in the allow wildcards); §2 items 3–5;
§3 items 5, 6 and 8.
