<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification — hooks, permissions, headless, GitHub Actions (2026-10-22)

Group: `hooks`, `permissions`, `headless`, `github-actions`, read on 2026-10-22
from `https://code.claude.com/docs/en/<page>.md` (all four fetched, no moved
page). Not fetched: the sandboxing page, the CLI reference and the settings
reference; what depends on them is marked UNVERIFIED. Returned by a read-only
subagent; the runner wrote this file. Verdict: **no contract break**; one
mechanism that turns a fail-open hook into a fail-closed one, and several
over-matching allow rules.

## Still documented and valid

Events `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`;
matchers `Bash`, `Edit|Write|NotebookEdit`; `type`, `command`, `timeout`
(seconds), `statusMessage`; the `"${CLAUDE_PROJECT_DIR}"` placeholder, double
quoted; `PreToolUse` `hookSpecificOutput.permissionDecision: "deny"` with exit 0,
and exit 2 plus stderr to block; `additionalContext` (10,000-character cap, a
file and a 2,000-character preview beyond it); `systemMessage`; Stop
`{decision:"block",reason}`, `stop_hook_active`, the eight-block cap; the
`prompt`, `session_id`, `tool_input.command` and `tool_input.file_path` fields;
the permission rule forms and deny → ask → allow order; the workflow inputs
`anthropic_api_key`, `prompt`, `settings`, `claude_args`, `plugin_marketplaces`,
`plugins`, `--max-turns`, `--allowedTools`. Unverified: `tool_input.notebook_path`,
`--disallowedTools`.

## Mechanisms not adopted (recommendations)

a. **`onFailure: "block"`** on `command` hooks (Claude Code v2.1.295+; the
   version the Action bundles is UNVERIFIED). Without it a missing,
   non-executable or crashing guard, a timeout, or any exit other than 0 or 2
   lets the call proceed; `lib.sh`'s `set -euo pipefail` makes an unhandled
   failure exit 1, which is fail-open. Fix: add it to the three `PreToolUse`
   entries. Not for `UserPromptSubmit` (a failure would block the owner's
   prompt) and no effect on `Stop`.
b. **The sandbox**: Bash and Read/Edit deny rules do not bind subprocesses,
   `sh -c` or the same program by path. `guard-provider-mutation.sh` is, by its
   own header, a text-matching tripwire; sandbox write-denies for the two
   provider prefixes, `/usr/local` and shell dotfiles, and a network allowlist,
   would enforce what is now stated. Sandbox keys UNVERIFIED (page not fetched).
c. `permissions.disableAutoMode: "disable"` beside the bypass disable.
d. The hook `if` field (`Bash(git commit *)`) on the commit gate; best-effort for
   `$()` and unknown shapes, so never on a guard.
e. `--permission-prompts none` on both workflows (low value).
f. **The `<pasted_content id="…">` delimiter** at the head of a pasted prompt:
   `arm-guard-edit.sh` filters only task-notification, agent-message,
   system-reminder and `[SYSTEM NOTIFICATION` lines, so a token at column zero
   inside pasted text still arms the escape. Fix: ignore lines between the
   markers.

## Guidance not followed

- **Wildcard placement.** The text before the first `*` is matched as written, so
  `Bash(ls*)` matches `lsof` and `cargo test*` matches a `cargo-test-foo`
  subcommand; `cmd *` also matches the bare `cmd`. No-space allow rules:
  `git status*`, `git diff*`, `git branch*` (also `-D`), the `scripts/*.sh*`
  entries, the `brew --prefix*` family, the `port` read family, `sw_vers*`, the
  `cargo fmt|check|build|clippy|test|metadata*` family. The no-space deny rules
  over-match, which is the safe direction.
- **CI cost controls.** `claude-code-review.yml` has no `--max-turns` and no
  `concurrency:` block; `claude.yml` has no `concurrency:` block (it has
  `--max-turns 25` and `timeout-minutes`).
- **Stop-hook loop guidance.** `stop-gate.sh` reads `stop_hook_active` but blocks
  again and relies on the eight-block cap: deliberate (its header), different
  from the documented advice.
- **Review workflow permission.** The documentation's example uses
  `pull-requests: read`; the repository's `write` rests on an admitted
  unreproduced observation (its own comment says to test `read`).
- **Stale read dates in comments**: `claude.yml` lines 9, 57, 103; `lib.sh:111`;
  `post-edit-checks.sh:76`; `arm-guard-edit.sh` lines 20, 149. The cited content
  still holds as of 2026-10-22.
- **Allow rules in CI.** Under `claude -p` in a never-trusted folder project
  `permissions.allow` is not used; the workflows' comments say the checked-in
  permissions apply, true only because of the `settings:` input. A comment fix.
