# Tooling recheck — hooks, permissions and CI — 2026-09-27

Read-only recheck of `.claude/hooks/*.sh`, `.claude/settings.json` (hooks +
permissions blocks, rule syntax) and `.github/workflows/*.yml` against the
current Claude Code documentation. No repository file was edited.

## Fetch status

All four pages were fetched successfully via their `.md` variants, no
fallback needed:

| Page | URL fetched | Status |
|---|---|---|
| Hooks | https://code.claude.com/docs/en/hooks.md | SUCCESS (read 2026-09-27) |
| Permissions | https://code.claude.com/docs/en/permissions.md | SUCCESS (read 2026-09-27) |
| Headless | https://code.claude.com/docs/en/headless.md | SUCCESS (read 2026-09-27) |
| GitHub Actions | https://code.claude.com/docs/en/github-actions.md | SUCCESS (read 2026-09-27) |

Two related pages were **not** fetched (out of scope of the four requested
URLs, so nothing below asserts their content): `/docs/en/cli-reference.md`
(would carry `--input-format`, the full `--max-turns`/`--model`/`--add-dir`
flag list) and the single "JSON output fields available to all hooks" table
on the hooks page — the fetch got `systemMessage` and `terminalSequence`
verbatim but not `continue`/`stopReason`/`suppressOutput` individually. Where
this note relies on those three fields it says so and does not assert wording
it did not see.

---

## 1. Documented fields/values/contracts the repository uses — still valid?

### Hook events registered (`.claude/settings.json:182-252`)

`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop` — all
five are still current event names per
https://code.claude.com/docs/en/hooks.md (read 2026-09-27). The page now
documents a much larger set (30 events: `Setup`, `UserPromptExpansion`,
`PermissionRequest`, `PermissionDenied`, `PostToolUseFailure`,
`PostToolBatch`, `Notification`, `MessageDisplay`, `SubagentStart`,
`SubagentStop`, `TaskCreated`, `TaskCompleted`, `StopFailure`,
`TeammateIdle`, `InstructionsLoaded`, `ConfigChange`, `CwdChanged`,
`DirectoryAdded`, `FileChanged`, `WorktreeCreate`, `WorktreeRemove`,
`PreCompact`, `PostCompact`, `PreModelSwitch`, `PostModelSwitch`,
`Elicitation`, `ElicitationResult`, `SessionEnd`), so the five in use are a
small, still-valid subset — nothing the repo registered has been renamed or
removed. See §2 for events that would close a gap the repo's own comments
name.

**Hook definition schema.** Every entry uses `"type": "command"`,
`"command"`, `"timeout"` (seconds) and `"statusMessage"` — all four are
documented fields on the current schema (hooks.md; `"command"` is one of
five hook types now: `command`, `http`, `mcp_tool`, `prompt`, `agent`). Still
valid.

- `.claude/settings.json:209` — matcher `"Bash"` (bare tool name).
- `.claude/settings.json:220` — matcher `"Edit|Write|NotebookEdit"`.
- `.claude/settings.json:233` — matcher `"Edit|Write"`.

Matcher-against-tool-name is the documented default behaviour for most
events; the doc's only explicitly quoted matcher exception is `FileChanged`
(`|`-delimited literal filenames, not regex). The two multi-tool matchers
above are pipe-alternation over tool names, consistent with that model. The
fetch did not turn up a single sentence spelling out "matcher accepts
regex/alternation over the tool name" verbatim, so this is inference from the
documented default, not a literal quote — flagging rather than asserting.

**`${CLAUDE_PROJECT_DIR}` substitution** — used in every `"command"` value
(e.g. `.claude/settings.json:188`, `:200`, `:213`, `:224`, `:249`). Confirmed
current placeholder syntax.

### Stdin fields the scripts read

All confirmed as fields the current schema documents for the event in
question (hooks.md, read 2026-09-27):

| Field | Read at | Event | Still documented? |
|---|---|---|---|
| `session_id` | `.claude/hooks/arm-guard-edit.sh:231`, `.claude/hooks/guard-protected-paths.sh:55` | UserPromptSubmit, PreToolUse | Yes — common envelope field on every event |
| `prompt` | `.claude/hooks/arm-guard-edit.sh:143,148` | UserPromptSubmit | Yes |
| `tool_input.command` | `.claude/hooks/guard-provider-mutation.sh:34` | PreToolUse (Bash) | Yes |
| `tool_input.file_path` | `.claude/hooks/guard-protected-paths.sh:23`, `.claude/hooks/post-edit-checks.sh:18` | PreToolUse/PostToolUse (Edit/Write) | Yes |
| `tool_input.notebook_path` | `.claude/hooks/guard-protected-paths.sh:24` | PreToolUse (NotebookEdit) | Yes |
| `stop_hook_active` | `.claude/hooks/stop-gate.sh:54` | Stop | Yes — the fetched `SubagentStop` example shows this exact field name; the repo's own comment (`stop-gate.sh:21-26`) independently claims it for `Stop` too, which the fetch did not separately re-confirm verbatim for `Stop` specifically (only for `SubagentStop`). Treat the `Stop`-event claim as very likely but not independently re-verified this pass.

`cwd`, `transcript_path`, `hook_event_name` are read implicitly by nothing in
these scripts (none of the hooks branch on them), so there is nothing to
validate there beyond noting they remain part of the common envelope.

### Output JSON shapes emitted (`.claude/hooks/lib.sh`)

- `.claude/hooks/lib.sh:94-95` (`mpb_deny`) emits
  `{"hookSpecificOutput":{"hookEventName":...,"permissionDecision":"deny","permissionDecisionReason":...}}`.
  This is the **current, non-deprecated** PreToolUse schema. The doc says
  verbatim: *"PreToolUse previously used top-level `decision` and `reason`
  fields, but these are deprecated for this event. Use
  `hookSpecificOutput.permissionDecision` and
  `hookSpecificOutput.permissionDecisionReason` instead."* The repository
  already uses the new form everywhere — no drift, nothing to fix.
- `.claude/hooks/lib.sh:117-118` (`mpb_context`) emits
  `{"hookSpecificOutput":{"hookEventName":...,"additionalContext":...}}`,
  used for `SessionStart` (`.claude/hooks/session-start.sh:57`) and
  `PostToolUse` (`.claude/hooks/post-edit-checks.sh:76`). Both are events the
  doc documents as valid destinations for `additionalContext`. Still valid.
- `.claude/hooks/lib.sh:111-114` — the 9,000/10,000-character truncation
  matches the doc's stated cap verbatim: *"additionalContext is capped at
  10,000 characters."* Still valid, and the repo leaves a safety margin
  (9,000) rather than sitting on the exact limit.
- `.claude/hooks/lib.sh:133` (`mpb_message`) emits `{"systemMessage":...}`,
  used only for Stop-gate warnings (`.claude/hooks/stop-gate.sh:245`). The
  script's own header comment (`stop-gate.sh:241`: *"Warnings must not
  prolong the turn (additionalContext on Stop would)"*) states the reason:
  `additionalContext` is non-error feedback the model can act on and the
  conversation continues, which is the wrong shape for a Stop hook that is
  meant to let the turn end after only warning. `systemMessage` is the
  documented mechanism that surfaces to the human without continuing the
  turn. Consistent with the documented model; still valid usage.

### Exit codes

- Fail-closed guards exit **2** on unreadable input
  (`.claude/hooks/lib.sh:70`, `:85`; `.claude/hooks/arm-guard-edit.sh:151`).
  Matches the documented blocking contract: *"Exit 2 means a blocking
  error... even a JSON `permissionDecision` of `"allow"` can't override
  it."* Still valid for PreToolUse. For UserPromptSubmit specifically, the
  fetch confirmed the event can block (*"Can block the expansion"* is stated
  for the sibling `UserPromptExpansion`; `UserPromptSubmit`'s own row says
  only "before Claude processes it" with no explicit "can block" sentence
  captured) — the exit-2 contract itself is general across blockable events
  per the doc's exit-code section, so this is very likely correct but the
  fetch did not capture a UserPromptSubmit-specific "can block" sentence to
  quote. Flagging, not asserting a defect.
- `mpb_deny` exits **0** while printing a `permissionDecision: "deny"`
  object (`.claude/hooks/lib.sh:103`). Matches: *"With a parsed object that
  passes schema validation, for events that use the standard decision model,
  Claude Code ignores the exit code and the JSON alone decides the
  outcome."* Confirmed correct — this is not a bug, it is the documented
  pattern (the file's own comment at `lib.sh:90` already says so, and the
  doc bears it out).
- `.claude/hooks/stop-gate.sh:237` exits **2** to block Stop, with the
  concatenated `fail[]` messages on stderr. Matches verbatim: *"A hook that
  blocks by exiting 2 routes the same way as `reason`: Claude receives the
  stderr message as the explanation for why it should continue."* Confirmed
  valid.
- `.claude/hooks/stop-gate.sh:22-26`'s comment names
  `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` (default 8) as the loop-guard cap Claude
  Code itself enforces after repeated Stop blocks. **This specific env-var
  name and default were not confirmed in the pages fetched this pass** — the
  hooks.md fetch did not surface it. Not contradicted either; simply
  unverified. Flag for a follow-up fetch if this claim needs to be load-
  bearing (e.g. before relying on it in a spec change).

### Permission rule syntax (`.claude/settings.json:1-181`)

Verified against https://code.claude.com/docs/en/permissions.md (read
2026-09-27):

- Bash wildcard forms: both `Bash(cmd *)` (space + `*`, prefix-with-boundary)
  and `Bash(cmd*)` (no space, matches `lsof`-style substrings) appear
  deliberately for different entries — e.g. `Bash(chsh *)` **and**
  `Bash(chsh)` (`:88`, `:96`), `Bash(brew up)` **and** `Bash(brew up *)`
  (`:97-98`). Matches the documented wildcard table exactly; still valid.
- `Edit(//opt/local/**)`, `Edit(//usr/local/**)`, `Edit(//opt/homebrew/**)`,
  `Edit(//Library/**)` (`:55-58`) — double-leading-slash = "absolute path
  from filesystem root," per the documented pattern table. The repo
  correctly avoids the documented pitfall: *"A pattern like
  `/Users/alice/file` isn't an absolute path. The single leading slash
  anchors at the settings source... Use `//Users/alice/file` for absolute
  paths."* These rules use the double-slash form throughout — still valid
  and correctly anchored.
- `Edit(~/.zshrc)` etc. (`:48-54`, `:59`) — `~/path` home-directory form.
  Still valid.
- `Edit(./rust-toolchain.toml)` / `Edit(./docs/spec/01-invariants.md)`
  (`:106`, `:108`) — `./path` = relative to the settings source (project
  settings here, so `<primary working directory>/rust-toolchain.toml`).
  Still valid.
- `disableBypassPermissionsMode: "disable"` (`:172`) — exact documented key
  and value. Confirmed current.
- `WebFetch(domain:X)` allow entries (`:142-149`, `:168`) — bare-hostname
  form, matches documented exact-domain semantics (no wildcard subdomain
  needed for these single hosts). Still valid.
- Deny/ask/allow precedence: the repo's own comment in `claude.yml`
  ("deny beats allow at every scope") states exactly what the doc confirms
  verbatim: *"Rules are evaluated in order: deny, then ask, then allow. The
  first match in that order determines the outcome, and rule specificity
  doesn't change the order."* Still correct.

**Finding — two dead rules.** `.claude/settings.json:107` and `:109`:

```
106  "Edit(./rust-toolchain.toml)",
107  "Write(./rust-toolchain.toml)",
108  "Edit(./docs/spec/01-invariants.md)",
109  "Write(./docs/spec/01-invariants.md)"
```

The permissions page states plainly: *"Claude Code checks file perms only
against `Edit(path)`/`Read(path)` — NOT `Write`, `NotebookEdit`, `Glob`,
`MultiEdit` path rules (accepted but never consulted, with startup
warning)."* Under this documented behaviour, lines `107` and `109` — the
two `Write(...)` entries — are **inert**: they parse, but the permission
engine never consults a `Write(path)` rule, and Claude Code should print a
startup warning about each. This is not a live security hole today, because
`Edit(path)` rules "apply to all built-in file tools" (per the same page),
so the neighbouring `Edit(./rust-toolchain.toml)` / `Edit(./docs/spec/
01-invariants.md)` lines (`106`, `108`) already cover `Write` on the same
two paths. But the two `Write(...)` lines are dead configuration that
should be removed (or, if the intent was ever independent Write-only
coverage, that intent cannot be expressed this way at all under the current
docs). No other file in the deny list has a matching `Write(...)` twin
(`:48-59` protect shell configs and provider prefixes with `Edit(...)`
alone), which suggests these two were added later under a mistaken belief
that `Write(...)` does independent work.

**Same-finding corollary — `NotebookEdit` is not covered for these two
files.** The doc also says a `Read` deny rule cascades to `Edit`/`Write` on
the same path but explicitly **not** `NotebookEdit`; separately, `Edit(path)`
is what's actually consulted, and nothing in the doc says `Edit(path)`
cascades to `NotebookEdit` either. So `rust-toolchain.toml` and
`docs/spec/01-invariants.md` have no deny coverage against a `NotebookEdit`
tool call under the current permission model (low real-world risk, since
neither file is a notebook, but worth naming precisely rather than assuming
the `Edit(...)` line is a complete deny).

### `--disallowedTools` (used in `.github/workflows/claude.yml`)

Confirmed documented on the **permissions** page (not headless/github-
actions): *"a managed settings deny can't be overridden by `--allowedTools`,
and `--disallowedTools` can add restrictions beyond what managed settings
define."* `.github/workflows/claude.yml`'s `claude_args` block uses it with
the same `Bash(git diff *)`-style specifiers documented for `--allowedTools`
on the headless page (*"trailing ` *` = prefix match, e.g.
`"Bash(git diff *),Bash(git log *)"`"*). Both flags and both specifier forms
are still valid and match the workflow's usage exactly.

### GitHub Actions workflows

- Action identity: `anthropics/claude-code-action@<sha> # v1` in all three
  steps (`.github/workflows/claude.yml`, `.github/workflows/
  claude-code-review.yml`) — matches the documented repo
  (`anthropics/claude-code-action`) and current tag (`v1`, not the
  deprecated `@beta`). The repo is already past the beta→v1 migration: no
  `mode:` input, no `direct_prompt:` (uses `prompt:`), CLI options
  (`--max-turns`, `--allowedTools`, `--disallowedTools`) are inside
  `claude_args:` rather than top-level inputs — all four migration steps the
  doc lists are already done.
- `id-token: write` present in every job (`claude.yml`, `claude-code-
  review.yml`) — matches: *"Required by the action's default GitHub App
  authentication."* Still required, still present.
- `actions: read` present in `claude.yml` — matches the documented
  `actions: read` note (*"lets Claude read CI results on PRs"*).
- `settings: .claude/settings.json` passed as an Action input in both
  workflows — matches the documented `settings` input (*"Claude Code
  settings, as a JSON string or a path to a settings JSON file"*).
- `plugin_marketplaces` / `plugins` inputs in `claude-code-review.yml` —
  both are documented inputs, still valid names.
- `anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}` — matches the
  documented secret-to-input wiring exactly.

---

## 2. Documented mechanisms not used that would enforce something the repo only states

**a. `SessionEnd` — the strongest finding of this pass.**
`.claude/hooks/arm-guard-edit.sh:24-27` states, as the rationale for its own
design: *"the marker does not expire at session end: nothing here runs
then, a resumed session keeps its `session_id`, so `session-start.sh`
deletes the marker instead."* This is the repository's own comment
admitting there is no hook at session end, and working around the absence
by deferring cleanup to the next `SessionStart`. The current hooks page
documents `SessionEnd` — *"When a session terminates"* — which did not
exist, or was not in scope, when this design was written. Wiring
`rm -f "${CLAUDE_PROJECT_DIR}/.claude/.guard-edit-marker"` into a
`SessionEnd` hook would close the exact gap the comment names (a marker
surviving from an earlier run of a *resumed* session with the same
`session_id`, currently relying on `session-start.sh:24` to catch it on the
next start) at the boundary the comment says it should happen, rather than
one hook-firing later. This does not change the documented residual risk
that the agent can forge the marker via `Bash` (that limitation is
independent of when cleanup runs), but it would remove the specific "nothing
here runs then" gap the file's own header names.

**b. `PermissionDenied` (with `hookSpecificOutput.retry: true`)** — not
used. Applicable when auto permission mode denies a call; the repo does not
set `defaultMode: auto` anywhere in `.claude/settings.json`, so this is
lower-priority and may simply not apply to the repo's current permission
mode. Noting for completeness, not flagging as a gap.

**c. `PostToolUseFailure`** — not used. No hook reacts specifically to a
*failed* tool call (as opposed to `PostToolUse`, which only fires after
success per the table: *"After a tool call succeeds"*). `stop-gate.sh`
re-runs `scripts/validate.sh` wholesale at Stop time regardless, so this is
redundant rather than a hard gap, but it means a failed `cargo test` run
mid-turn produces no immediate structured signal — only the eventual Stop
gate re-verification.

**d. `WorktreeCreate` / `WorktreeRemove`** — not used. No repo policy
currently depends on worktree lifecycle, so this is informational only.

---

## 3. Guidance not followed

Nothing rose to the level of "the repo actively contradicts current
guidance." The one item close to that line is the dead `Write(...)` rule
pair in §1 above, which is better read as "syntax the docs say does nothing"
than "guidance ignored" — the protection those two lines were meant to add
is already provided by the neighbouring `Edit(...)` rules, so no boundary
this project depends on is actually open. Everything else checked
(precedence model, exit-code contracts, hook output schemas, Action
migration state, permission-rule anchoring) matches the current documentation.

---

## Summary of concrete, file:line findings

1. `.claude/settings.json:107` — `Write(./rust-toolchain.toml)` is inert
   under the current permission model (`Write(path)` rules are "accepted
   but never consulted, with startup warning" per permissions.md). Fix:
   remove the line; `Edit(./rust-toolchain.toml)` (`:106`) already covers
   both Edit and Write on that path.
2. `.claude/settings.json:109` — same finding, for
   `Write(./docs/spec/01-invariants.md)`; `:108`'s `Edit(...)` rule already
   covers it.
3. `.claude/hooks/arm-guard-edit.sh:24-27` — the comment's own stated gap
   ("nothing [runs] at session end") can be closed by registering a
   `SessionEnd` hook (documented, unused) that deletes
   `.claude/.guard-edit-marker` directly, instead of relying solely on the
   next `SessionStart` to catch a marker left by a resumed session.
4. `.claude/hooks/stop-gate.sh:22-26` — the claimed env var
   `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` (default 8) was not confirmed in the
   pages fetched this pass; unverified, not contradicted. Recommend a
   direct fetch of the relevant hooks.md anchor before treating it as fact
   in a future spec change.
5. `.claude/settings.json:106-109` (both files) — no deny coverage exists
   for `NotebookEdit` on `rust-toolchain.toml` or
   `docs/spec/01-invariants.md` under the current documented rule scope
   (`Read`'s cascade to Edit/Write explicitly excludes `NotebookEdit`, and
   nothing documents `Edit(path)` cascading to `NotebookEdit` either). Low
   practical risk (neither file is a notebook) but worth naming precisely.

No repository file was edited in the course of this recheck.

---

## Decisions recorded by the main session, 2026-09-27

Every fix below lands in a guarded path (`.claude/settings.json`,
`.claude/hooks/`), which this session may not edit unarmed. None is a live
hole, so each is a recommendation to the owner rather than a change:

- Remove the two inert `Write(...)` deny rules in `.claude/settings.json`; the
  neighbouring `Edit(...)` rules already cover the same paths.
- Consider a `SessionEnd` hook to clear the guard-edit marker, closing the gap
  `arm-guard-edit.sh`'s own header names.
- `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` (default 8) is no longer re-confirmable
  from the pages read today; re-confirm on `/hooks#stop-input` before a spec
  change leans on it.
