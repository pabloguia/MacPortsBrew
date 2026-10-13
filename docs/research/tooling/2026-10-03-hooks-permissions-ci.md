# Tooling recheck: hooks, permissions and CI, 2026-10-03

Read on: 2026-10-03

Sources, all four fetched on the first try as the `.md` variant, no redirect:

- <https://code.claude.com/docs/en/hooks.md>, read 2026-10-03
- <https://code.claude.com/docs/en/permissions.md>, read 2026-10-03
- <https://code.claude.com/docs/en/headless.md>, read 2026-10-03
- <https://code.claude.com/docs/en/github-actions.md>, read 2026-10-03

Not fetched, so nothing below asserts their content: the action's
`docs/usage.md` (its full input reference) and `/docs/en/permission-modes.md`
(the protected-paths table).

Previous note: [`2026-09-27-hooks-permissions-ci.md`](2026-09-27-hooks-permissions-ci.md).

**Method.** Each page downloaded as raw markdown and read by section, so the
quotations are the pages' own text. Repository files read at `5c40b11`:
`.claude/settings.json`, `.claude/hooks/{lib,stop-gate,commit-gate,
session-start,arm-guard-edit,guard-protected-paths,guard-provider-mutation,
post-edit-checks}.sh`, `.github/workflows/{claude,claude-code-review,
repository-validation}.yml`.

## What changed since 2026-09-27

In the documentation:

- The Stop loop cap is documented verbatim: an 8-consecutive-continuation
  cap, raised with `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`. `stop-gate.sh`'s claim is
  correct; the 09-27 open item is resolved.
- The 10,000-character cap covers `additionalContext`, `systemMessage`,
  `initialUserMessage` and plain stdout per field; overflow is saved to a file
  with a 2,000-character preview rather than truncated.
- `Edit` deny rules now reach Bash file commands Claude Code recognises (`cat`,
  `head`, `tail`, `sed`, `tee`) and redirect targets. NotebookEdit is
  explicitly covered by an `Edit` deny rule (resolves 09-27 item 5).
- Deny rules match when either the requested path or its resolution matches;
  Edit and Write refuse to write through a symlink themselves.
- **PreToolUse timeouts fail open**: a timed-out command hook does not block
  the tool call.
- New handler fields `if` (one permission rule as a pre-filter), `args` (exec
  form), `asyncRewake`.
- UserPromptSubmit also fires for scheduled tasks and `/loop`,
  background-subagent report-backs, and cross-session messages.
- SessionStart has a `fork` source; `SessionEnd` has a 1.5 s budget.
- A `tool.check` mod can override a hook block and, outside managed or
  Team/Enterprise settings, a deny rule.
- Project `permissions.allow` rules need workspace trust; a `-p` or SDK run
  never applies them from the checkout. Hooks in that file run regardless.
- headless.md: `--bare` will become the default for `-p` in a future release,
  and `--bare` skips hook auto-discovery.
- github-actions.md: the action rejects a triggering user without write access;
  the review example uses `pull-requests: read`.
- The event set is the same 33 names as on 09-27.

In the repository since 2026-09-27: `bb7fe14` and `5c40b11` (SE-093) added
`commit-gate.sh`, moved the Stop gate to `--quick`, widened the allow list and
unguarded `.claude/agents/` and `.claude/skills/`. Neither 09-27
recommendation (drop the inert `Write(...)` rules, add `SessionEnd`) was
applied.

## 1. What the repository uses, re-confirmed

- **Events and matchers.** `SessionStart`, `UserPromptSubmit`, `PreToolUse`,
  `PostToolUse`, `Stop` current. `"Bash"`, `"Edit|Write|NotebookEdit"`,
  `"Edit|Write"` are exact-match matchers. Matching hooks run in parallel and
  `deny` wins over `defer`, `ask`, `allow`.
- **Stdin fields** `session_id`, `prompt`, `tool_input.command`,
  `tool_input.file_path`, `tool_input.notebook_path`, `stop_hook_active`: all
  documented. Pasted text now arrives wrapped in `<pasted_content id="…">`
  lines.
- **Output shapes.** `mpb_deny` emits the current
  `hookSpecificOutput.permissionDecision` schema; `mpb_context` emits
  `additionalContext`, valid for all four events it is used on; `mpb_message`
  emits `systemMessage`. Every hook keeps stdout to one JSON object. The
  9,000-character truncation in `lib.sh` stays under the cap; its cited read
  date (2026-09-02) is stale, and `mpb_message` does not truncate.
- **Exit codes.** Stop's exit 2 with stderr, and the guards' fail-closed exit
  2, are as documented. Exit 1 is a non-blocking error (D2).
- **Permission rule syntax.** Wildcard pairs, `//`, `~/`, `./` anchoring,
  deny-ask-allow precedence, `disableBypassPermissionsMode`, `WebFetch(domain:)`
  all as documented. The documentation's own example (`Bash(git push *)` does
  not stop `git -C . push`) is what `guard-provider-mutation.sh` exists for.
- **Workflows.** The action is pinned by SHA; `prompt`, `claude_args`,
  `settings`, `anthropic_api_key`, `plugin_marketplaces`, `plugins`,
  `id-token: write` and `actions: read` are documented. The `settings` input
  grants allow rules; a deny at any level cannot be re-allowed.

## 2. Mechanisms not used that would enforce what is only stated

1. **`ConfigChange`** on `local_settings|project_settings` can stop a
   mid-session settings change (a Bash write of an `env` grant or
   `disableAllHooks` to `settings.local.json`) taking effect in the running
   session. It surfaces no message, so it should log.
2. **`SessionEnd`** to delete the guard-edit marker at session end rather than
   at the next start (recommended on 09-27).
3. **Stop input `background_tasks`** lets the gate tell "done" from "paused for
   a subagent".
4. **`"if": "Bash(git *)"`** on `commit-gate.sh`, keeping the script's own
   regex as the real test.
5. **`disableAutoMode`** and **`dontAsk`** in CI exist; enabling either is the
   owner's call.

## 3. Defects and recommendations, by file

**Defects (the repository does something the docs say behaves differently):**

- **D1** `.claude/settings.json`: `Write(./rust-toolchain.toml)` and
  `Write(./docs/spec/01-invariants.md)` are accepted but never consulted, with
  a startup warning (permissions.md). The `Edit(...)` rules beside them cover
  the paths. Fix: delete both. Open since 09-27.
- **D2** `.claude/hooks/commit-gate.sh`: a failed `cd` exits 1 and a missing
  `validate.sh` exits 0; both let the commit through unvalidated, contrary to
  the header. Fix: deny on both, as `stop-gate.sh` does.
- **D2b** the commit gate's 1500 s hook timeout fails open. Fix: bound
  `validate.sh` inside the script below the timeout and deny on expiry.
- **D3** `.claude/hooks/arm-guard-edit.sh`: exit 2 on UserPromptSubmit rejects
  the human's whole prompt; with no JSON parser every prompt is rejected. The
  marker is already deleted, so blocking adds nothing. Fix: exit 0 with
  context explaining the disarm.
- **D4** `.github/workflows/claude.yml` sets `MPB_SKIP_STOP_GATE` but not
  `MPB_SKIP_COMMIT_GATE`. Hooks run under `-p` and the `settings` input allows
  `git commit`, so a commit made from Bash would run `cargo test` on the PR
  head with `ANTHROPIC_API_KEY` in the environment — the exposure the
  workflow's own comments say it avoids. Fix: set `MPB_SKIP_COMMIT_GATE: "1"`.

**Recommendations:**

- `session-start.sh` deletes the marker on `compact` too, so an auto-compaction
  disarms an armed message (errs safe); read `source` or document it.
- `arm-guard-edit.sh`'s relay filter should name the cross-session envelope
  (its tag is not documented; unverified) and treat `<pasted_content>` text as
  quoted. Its armed message lists `.claude/agents` and `.claude/skills` as
  guarded; they are not since SE-093.
- Stale comments: `lib.sh`'s read date; `guard-provider-mutation.sh`'s "a deny
  rule on a path does not reach a subprocess" (now true only for `cp`, `mv`,
  `ln` and arbitrary subprocesses); `claude.yml`'s collaborator caveat (the
  action checks write access itself).
- `Bash(git diff*)`, `Bash(git branch*)`, `Bash(git status*)` lack the space
  before `*`.
- `claude-code-review.yml` could use `pull-requests: read`, per the documented
  example; its claim that `read` produced nothing cannot have been observed
  while the workflows are dormant.
- Pin the plugin marketplace once `usage.md` is read.
- Watch `--bare` becoming the `-p` default: every hook would stop in CI
  silently, including the `MPB_UNATTENDED` refusals.
- State that an installed mod can override deny rules and hooks.

Every fix above is in a guarded path.
