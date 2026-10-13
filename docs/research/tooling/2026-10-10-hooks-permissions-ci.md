# Tooling recheck: hooks, permissions and CI, 2026-10-10

Read on: 2026-10-10. Previous note: [`2026-10-03-hooks-permissions-ci.md`](2026-10-03-hooks-permissions-ci.md).
Repository read at `8f60433`; installed CLI `claude 2.1.278`.

Sources, all fetched as the `.md` variant, read 2026-10-10:
<https://code.claude.com/docs/en/hooks.md>, <https://code.claude.com/docs/en/permissions.md>,
<https://code.claude.com/docs/en/headless.md>, <https://code.claude.com/docs/en/github-actions.md>.
Not fetched: the action's `docs/usage.md`, `permission-modes.md`, `env-vars.md`.

"New" means "documented, and not recorded in the 2026-10-03 note".

## 1. Documentation, new to the note

1. **`onFailure: "block"`** on command and HTTP hooks (v2.1.295+; default
   `"continue"`): a hook that cannot start, exits other than 0/2, prints invalid
   JSON or times out then acts as exit 2. No effect on `Stop`, `SubagentStop`,
   `TaskCompleted`, `TeammateIdle` or async hooks.
2. **`if`** holds one permission rule, is best-effort (the hook runs anyway when the
   command cannot be parsed); since v2.1.214 `Edit(src/**)` matches only the
   working-directory `src`.
3. **Exec form (`args`)** recommended whenever a command references a path placeholder.
4. **UserPromptSubmit**: a timed-out hook's output is discarded and the prompt still
   reaches Claude; new `hookSpecificOutput.suppressOriginalPrompt`; default timeout 30 s.
5. **Stop exception**: exit 2 with empty stdout and stderr saying a file is missing
   (e.g. `No such file or directory`) is treated as non-blocking.
6. **SessionEnd** budget raisable per hook up to 60 s.
7. **ConfigChange** matchers; a block has no effect for `policy_settings`.
8. 33 events, including `Setup`, `PreModelSwitch`/`PostModelSwitch`, `MessageDisplay`,
   `PostToolBatch`, `UserPromptExpansion`.
9. **headless**: `--permission-prompts none`; a run that sets no permission mode
   takes the built-in starting mode, "which can be `auto`"; `dontAsk` for locked-down CI;
   `--bare` will become the default for `-p`.
10. **github-actions**: triggering-user checks (write access, human actor unless in
    `allowed_bots`); the review example uses `pull-requests: read`; no documented ref
    pin for `plugin_marketplaces`.

Re-confirmed unchanged: the Stop 8-block cap; PreToolUse timeouts fail open;
`Write(...)`/`NotebookEdit(...)`/`Glob(...)` path rules are never consulted; a
`Read` deny also blocks Edit/Write; wrapper stripping list; space before `*` matters.

## 2. Status of the 2026-10-03 defects

D1, D2, D2b, D3, D4 are **closed**. Still open: `settings.json` `Bash(git status*)`,
`Bash(git diff*)`, `Bash(git branch*)` lack the space; `session-start.sh` disarms on
`compact` (errs safe); `claude.yml`'s collaborator caveat partly stale;
`claude-code-review.yml` uses `pull-requests: write` where the docs example uses
`read`; the review workflow's plugin marketplace is unpinned.

## 3. New findings

| # | Where | Finding | Fix | Disposition (2026-10-10) |
|---|---|---|---|---|
| N1 | `settings.json` PreToolUse guards; `guard-provider-mutation.sh`, `guard-protected-paths.sh` | an unforeseen non-zero exit (not 2), a killed or non-executable script, or a timeout lets the call through | `trap 'exit 2' ERR` in the guards now; `"onFailure": "block"` once the CLI is ≥ 2.1.295 | **Staged** for the owner-armed guarded batch (pass 09) |
| N2 | `arm-guard-edit.sh` (UserPromptSubmit) | a timeout silently drops the armed state | none — blocking would block the human's prompt | Accepted, noted |
| N3 | `stop-gate.sh` | exit 2 with stderr only; a relayed stage line saying a file is missing would make the block non-blocking | also print a JSON `decision: "block"` on stdout; add a `test-hooks.sh` case | **Staged** for the armed batch |
| N4 | `claude.yml`, `claude-code-review.yml` | no `--permission-mode` | `--permission-mode dontAsk` | **Staged** for the armed batch (owner's call; recommended) |
| N5 | `settings.json` commit-gate handler | optional `"if": "Bash(git *)"` pre-filter | — | Recommendation |
| N6 | `settings.json` hook commands | shell form with hand quoting | exec form with `args` | Recommendation |
| N7 | workflow header comments | stale read dates | refresh when next edited | **Staged** with N4 |

Every fix above except the workflow edits sits in a guarded path.

## 4. Mechanisms not used

`onFailure: "block"` (N1); `--permission-mode dontAsk` (N4); `ConfigChange` logging;
`SessionEnd` to delete the guard-edit marker; Stop's `background_tasks`; `Setup`.
