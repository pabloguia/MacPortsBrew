<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling recheck 2026-11-14 — hooks, permissions, headless, GitHub Actions

Read 2026-11-14 by a read-only subagent. No hook script, `test-hooks.sh` or mutating command was run; statements about script behaviour come from reading the code. Previous note: `2026-11-03-hooks-permissions-ci.md`. Shell `curl` was not available to the subagent; pages were read through a fetch-and-index tool, targeted queries and WebFetch (first 100,000 characters). The full `hooks` page and the `NotebookEdit` input table were not read.

**Verdict: no contract break in the scripts or `settings.json`. One docs change reshapes a carried finding; one open repository defect is unchanged.**

## Drift since 2026-11-03

1. **Monitor: the gap is now version-dependent.** [H §Match Monitor commands]: a `PreToolUse` hook with a `Bash` matcher also runs on Monitor shell commands, and `allow`/`updatedInput` have no effect on them, from v2.1.314. The guards only deny, so `Bash` suffices on v2.1.314+. The local CLI is **2.1.278**, so the gap still exists here. The 11-03 `Bash|Monitor|PowerShell` recommendation is superseded by "record a minimum CLI version".
2. **`--bare`.** [HL]: hooks do not run in bare mode when passed with `--settings`, and bare "will become the default for `-p`". The workflows pass their hooks through the action's `settings` input — a forward risk.
3. **Wildcards.** [P §Wildcard patterns]: a trailing ` *` also matches the bare command; the space before `*` is part of the rule; `:*` is equivalent.
4. **Repository since 11-03:** `stop-gate.sh` gained the SE-113 `probe.rs`/`probe/tests.rs` exemptions; `test-hooks.sh` grew (not read). `settings.json` and the four workflows: no diff since `35bc7a5`. Emitted forms re-grepped: `lib.sh:95` (`permissionDecision: "deny"`), `:118` (`additionalContext`), `:142` (Stop block form).
5. **Unchanged and open:** the Linux "Repository gate" step cannot fail; `onFailure: "block"` absent; `display_report: "true"`.

## Sources (read 2026-11-14)

[H] `https://code.claude.com/docs/en/hooks.md` (highest version seen v2.1.314, a lower bound); [P] `permissions.md`; [HL] `headless.md` (whole); [GA] `github-actions.md` (whole); [TR] `tools-reference.md` (indexed, not read for Monitor/PowerShell). Not fetched: `cli-reference`, `env-vars`, `settings-reference`, `permission-modes`, `sandboxing`, the action's `action.yml`.

## 1. Contracts used — valid

| Script | Reads | Emits | Status |
|---|---|---|---|
| `session-start.sh` | no stdin | `additionalContext` | valid; no matcher is valid |
| `arm-guard-edit.sh` | `prompt`, `session_id` | stdout / `additionalContext` | valid; `<pasted_content id="…">` markers re-confirmed |
| `guard-provider-mutation.sh` | `tool_input.command` | `permissionDecision: "deny"` | valid; no deprecated top-level `decision` |
| `commit-gate.sh` | `tool_input.command` | deny / `additionalContext` | valid; timeout 1500 s explicit |
| `guard-protected-paths.sh` | `tool_input.file_path`, `notebook_path`, `session_id` | deny | valid; `notebook_path` carried from 11-03 |
| `post-edit-checks.sh` | `tool_input.file_path` | `additionalContext` | valid |
| `stop-gate.sh` | `stop_hook_active` | `{decision:"block",reason}` / `systemMessage` | valid; cap 8, `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` |

Exit codes: 0 applies JSON; 2 blocks; any other non-zero is non-blocking except under `onFailure: "block"` (v2.1.295+, no effect on `Stop`). A timed-out PreToolUse hook does not block. Output caps: 10,000 characters, then a file plus a 2,000-character preview; `lib.sh:113-115` stays under it. A `-p` session runs the project's hooks with no trust dialog [HL].

`settings.json`: matchers `Bash` and `Edit|Write|NotebookEdit` valid; `Bash(prefix *)`, `WebFetch(domain:…)`, `Edit(./…)` valid (`./` row carried); `disableBypassPermissionsMode: "disable"` valid.

CI: `--permission-mode dontAsk`, `--allowedTools`, `--max-turns` documented; action inputs `anthropic_api_key`, `github_token`, `settings`, `prompt`, `claude_args` documented; `display_report` still not on [GA]'s table; `id-token: write` omitted consistently; SHA pin deliberate.

## 2. Findings

1. **Unchanged, highest value: the Linux "Repository gate" step cannot fail** (`repository-validation.yml:61-66`): `stop-gate.sh` exits 0 with `{decision:"block"}`, so the step stays green; the macOS job's `validate.sh --full` still catches it. Fix (guarded, owner): read the decision with `jq -e '.decision == "block"'` and exit 1, plus a `test-hooks.sh` case.
2. **Changed: Bash guards and Monitor.** `Bash` suffices to deny on v2.1.314+; nothing records a minimum CLI. Fix: record v2.1.314 as the minimum in `.claude/README.md` or spec 11 and have `/tooling-recheck` compare `claude --version`.
3. **Unchanged: `display_report: "true"`** (`claude.yml:381`, `claude-code-review.yml:319`) against the action's own warning; set `"false"` or record the accepted risk.
4. **Strengthened: `--bare`.** Add to the pin-bump checklist: a dispatched run must show the hooks fire.
5. **Carried, open:** `onFailure: "block"` on the three guards (only with a minimum-CLI record; local CLI 2.1.278); `stop-gate.sh` has no watchdog; non-human prompts and pasted text vs. `arm-guard-edit.sh`; worktree reads use `CLAUDE_PROJECT_DIR` not `cwd`; no-space allow wildcards; `Read(./.git/**)` does not cover `grep -r`; header claims about the Bash route; mods bound the deny and hook layers absent managed settings; no `concurrency:`; stale `lib.sh:111` comment.

## 3. Mechanisms not used

`ConfigChange` (could block settings changes); `FileChanged` (detects, cannot block); the `if` field (not for the guards, which read the whole line on purpose, SE-099); `Edit` deny rules on the escape's files; the sandbox; `permissions.disableAutoMode`; `--permission-prompts none`; exec form.

## 4. Open recommendations

| Recommendation | Status |
|---|---|
| `Bash|Monitor|PowerShell` matcher | Superseded: record a minimum CLI of v2.1.314 |
| `onFailure: "block"` on the three guards | Stands, with the CLI-version caveat |
| `display_report: "false"` | Stands |
| CI "Repository gate" able to fail | Stands, highest value |
| `--bare` check on every pin bump | Stronger |
