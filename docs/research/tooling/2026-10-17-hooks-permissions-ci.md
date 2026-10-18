<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification — hooks, permissions, CI (2026-10-17)

Group: `hooks`, `permissions`, `headless`, `github-actions`, read on 2026-10-17
from `https://code.claude.com/docs/en/<page>.md` (all four fetched; none
moved). Returned by a read-only subagent, which also sourced `lib.sh` in a
shell once to check item 5; the runner wrote this file. Verdict: **drift, 13
items**. Nothing was changed by this run: every file the items touch is a
guarded path (`.claude/hooks/`, `.claude/settings.json`, `.github/workflows/`)
or the owner's to rule on, so they are recommendations for the owner.

Not fetched, so unverified: sandboxing, settings-reference, cli-reference,
tools-reference; the `code-review` plugin's own `allowed-tools`; the action's
source (whether `plugin_marketplaces` accepts a `#ref`); GitHub's rule on who
can run `workflow_dispatch`.

## Still documented and used correctly

Hook inputs `tool_input.command`, `tool_input.file_path`, `session_id`,
`prompt`, `stop_hook_active` (`notebook_path` is documented on `permissions`
only); outputs `hookSpecificOutput.permissionDecision` and `additionalContext`,
`systemMessage`; exit 2 blocks `PreToolUse`, `UserPromptSubmit` and `Stop`; the
eight-continuation `Stop` cap and its override `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`
(`stop-gate.sh` names it); `timeout` in seconds; matcher forms; permission
rule forms (`Bash(x *)`, `//abs`, `~/`, `./`, `WebFetch(domain:)`); deny before
ask before allow; the workflow inputs `anthropic_api_key`, `settings`,
`plugin_marketplaces`, `plugins`, `prompt`, `claude_args`; project hooks run
under `-p` even in an untrusted folder.

## Drift — fix is the owner's (guarded paths)

1. `claude-code-review.yml:76-78` says `--allowedTools` "grants exactly one
   tool", but tools are also granted by `permissions.allow` in the `settings`
   input, and this job passes `.claude/settings.json` (`git commit`, `cargo *`,
   `grep`, `find`, WebFetch domains). Add `--disallowedTools` as `claude.yml`
   does, or drop `settings:` from this job.
2. `claude-code-review.yml` sets only `MPB_UNATTENDED`; `claude.yml` also sets
   `MPB_SKIP_STOP_GATE` and `MPB_SKIP_COMMIT_GATE`. Hooks run under `-p`, the
   Stop gate runs `validate.sh --quick` with the API key in the environment.
   Mirror `claude.yml` (partly the subagent's inference).
3. `claude.yml:6` says "no `prompt` input", but line 87 passes `prompt`
   (automation mode; results go to the run log).
4. `claude-code-review.yml:10-15` asserts `pull-requests: write` is needed; the
   only documented review workflow uses `read`. Unproven in a comment: cite or
   test.
5. **`lib.sh:12` runs `set -euo pipefail`; `stop-gate.sh:28` and
   `post-edit-checks.sh:12` deliberately use `set -uo pipefail` and then source
   it, which turns errexit ON** (confirmed in a shell). An unexpected failing
   command in `stop-gate.sh` then exits 1, which `hooks` documents as
   non-blocking: the turn ends unblocked. Put `set +e` after the `. lib.sh` line
   in those two scripts, or move `set -e` out of `lib.sh`.
6. `stop-gate.sh` writes `validate.sh` output to stderr on exit 2. `hooks` says
   a Stop hook that exits 2 with nothing on stdout whose stderr says a file is
   missing is treated as a non-blocking error; compiler output often contains
   that phrase. Exit 0 with `{"decision":"block","reason":"..."}` on stdout is
   the documented form.
7. `settings.json` bare duplicates (`Bash(su)`, `Bash(brew sh)`, `Bash(chsh)`,
   `Bash(launchctl)`, `Bash(sudo)`, `Bash(doas)`, `Bash(brew up)`) are redundant
   with their `X *` twins; harmless.
8. `arm-guard-edit.sh` says the armed window lasts "until the next user
   message", but `session-start.sh` deletes the marker on every SessionStart,
   which also fires on `clear` and `compact`. Match `startup|resume` or correct
   the sentence.
9. `settings.json:104-105` anchor `Edit(./rust-toolchain.toml)` and
   `Edit(./docs/spec/01-invariants.md)` to the current directory; `/path`
   anchors at the project root. The hook catches both anyway.

## Documented mechanisms the repository does not use

`onFailure:"block"` on hooks (fail closed; needs a recent Claude Code, version in
use unverified); an explicit permission mode in the workflows (`dontAsk`;
`disableAutoMode`); the OS sandbox for what deny rules and hooks cannot reach
(`curl` by absolute path, `sh -c`); `--bare`, `--setting-sources user` or
`disableAllHooks` for secret-holding unattended runs (a trade-off: they also
disable the repo's own guards); `--max-budget-usd`.

## Guidance not followed

`hooks` prefers exec form (`command` plus `args`) for hooks that reference a
path placeholder; the repo uses the documented fallback, shell form with
double-quoted `${CLAUDE_PROJECT_DIR}`. A preference, not a bug.

## Ungrounded claims (not counted)

`workflow_dispatch` "only write access can start it" (`claude.yml`,
`claude-code-review.yml`) rests on GitHub behaviour not fetched; plugin pinning
by `#ref` stays "not verified"; the hook matcher covers `Bash` only while the
docs advise `Bash|PowerShell` (whether that tool exists on macOS is on an
unfetched page).
