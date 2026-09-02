# Verification: Claude Code hooks, permissions and CI against current docs (second run)

**This is the second `/tooling-recheck` run of the Hooks / permissions / CI group
on 2026-09-02.** The first run's note is
[2026-09-02-hooks-permissions-ci.md](2026-09-02-hooks-permissions-ci.md); its
eleven findings were acted on in the same day, and this run verifies that and
reports drift *since* it. Read date for every documentation fact below:
**2026-09-02**. Repository files read read-only; nothing in the repository was
modified except this note.

Pages fetched (all resolved, none 404'd, none redirected, none unfetched):

| Page | URL | Status |
|---|---|---|
| Hooks reference | <https://code.claude.com/docs/en/hooks> (and `/hooks.md`) | fetched |
| Hooks guide | <https://code.claude.com/docs/en/hooks-guide> | fetched |
| Permissions | <https://code.claude.com/docs/en/permissions> | fetched |
| Headless | <https://code.claude.com/docs/en/headless> | fetched |
| GitHub Actions | <https://code.claude.com/docs/en/github-actions> | fetched |
| Settings reference | <https://code.claude.com/docs/en/settings-reference> | fetched |
| Permission modes | <https://code.claude.com/docs/en/permission-modes> | fetched (followed from the permissions page to answer the plan-mode question) |
| Sandboxing | <https://code.claude.com/docs/en/sandboxing> | fetched (same reason, for the `sandbox` SHOULD) |

Method: the hooks reference is ~230 KB and the settings reference ~337 KB, so
both were indexed in full and quoted from the indexed chunks rather than from a
WebFetch summary. Anything I could not locate verbatim is marked
**(not located)** and is *not* asserted.

---

## 0. What the first run found, and where it stands

Every one of the eleven findings is either fixed in the tree or recorded in the
specification. Verified line by line; none is re-raised below.

| # | First-run finding | Status on 2026-09-02, second run |
|---|---|---|
| 1 | `Edit(/opt/local/**)` etc. anchored at the repo root | **Fixed.** `.claude/settings.json:55-58` now `Edit(//opt/local/**)`, `//usr/local/**`, `//opt/homebrew/**`, `//Library/**`. The reasoning is written into `docs/spec/11-agentic-sdlc.md` (permission-boundary table). |
| 2 | `post-edit-checks.sh` findings on exit-0 stderr reach nobody | **Fixed.** `post-edit-checks.sh:75-77` now calls `mpb_context "PostToolUse"`. |
| 3 | `stop-gate.sh` never read `stop_hook_active` | **Fixed.** `stop-gate.sh:39-40` reads it; `:134-136` names the re-entry and the cap. Warnings now go out as `systemMessage` (`:145`) instead of exit-0 stderr. |
| 4 | Unparseable stdin failed open | **Fixed.** `lib.sh:20-64` — `mpb_json_field` returns 3 on a parse failure and `mpb_field_or_fail_closed` exits the *caller* with 2. Both guards use it (`guard-provider-mutation.sh:34`, `guard-protected-paths.sh:23-24`), and `test-hooks.sh:116,130,138-164` asserts it. |
| 5 | SessionStart matcher lacked `fork` | **Fixed.** The matcher is gone entirely (`settings.json:162-172`), so the hook fires on every source. Confirmed correct: hooks reference, "If you add a `matcher` field to an event without matcher support, it is silently ignored", and the SessionStart matcher table lists `startup`, `resume`, `clear`, `compact`, `fork`. |
| 6 | `Bash(arch*)` allowed `arch -x86_64 <anything>` | **Fixed.** `settings.json:118` is `Bash(arch)`, an exact match. |
| 7 | `"matcher": ""` on Stop | **Fixed.** Removed. |
| 8 | Redundant `Bash(sudo)`, `Bash(brew list)` | Partly. `Bash(brew list)` is gone; `Bash(su *)`+`Bash(su)` at `:67-68` remain, harmless. Not re-raised. |
| 9 | Unquoted `${CLAUDE_PROJECT_DIR}` | **Fixed.** All five hook entries use `"\"${CLAUDE_PROJECT_DIR}\"/.claude/hooks/…"`. |
| 10 | `claude.yml` passed no `--allowedTools` | **Fixed**, but the fix has a new defect — see §5.1. |
| 11 | Space-less trailing `*` style | Still present in ~20 rules. Style, not a defect; not re-raised. |

---

## 1. Hooks — the contract, and what the repository emits

### 1.1 Configuration fields the repository uses (hooks reference, "Common fields")

- `type: "command"` — documented. Other values `http`, `mcp_tool`, `prompt`,
  `agent` (the last two carry an experimental warning for `agent`).
- `command` — documented.
- `timeout` — documented: "Seconds before canceling". The table names
  event-specific defaults; the excerpt located reads "…`PreModelSwitch`, and
  `PostModelSwitch`, and to 10 on `MessageDisplay`. `SessionEnd` hooks share a
  1.5-second budget; if your settings set a longer per-hook `timeout`, Claude
  Code raises the budget to match, up to 60 seconds". The repository's values
  (20, 15, 15, 120, 600) are all plain per-hook seconds on events with no
  special budget. Valid.
- `statusMessage` — documented verbatim: "Custom spinner message displayed
  while the hook runs". All five entries use it. Valid.
- `matcher` — `Bash` and `Edit|Write|NotebookEdit` are tool-name regexes;
  documented. SessionStart and Stop now carry none; correct.
- `${CLAUDE_PROJECT_DIR}` — documented verbatim: "the project root where the
  session started. Claude Code also sets this variable in the environment of
  stdio MCP servers and plugin LSP servers." Valid.

### 1.2 stdin JSON the repository reads

| Script | Field read | Documented? |
|---|---|---|
| `guard-provider-mutation.sh:34` | `tool_input.command` | Yes — the reference's own PreToolUse example script is `COMMAND=$(jq -r '.tool_input.command')`. |
| `guard-protected-paths.sh:23-24` | `tool_input.file_path`, then `tool_input.notebook_path` | Yes — permissions page, "Match by input parameter": `file_path` for Read/Edit/Write, `notebook_path` for NotebookEdit. |
| `post-edit-checks.sh:18` | `tool_input.file_path` | Yes. |
| `stop-gate.sh:40` | `stop_hook_active` | Yes — Stop input example shows `"stop_hook_active": true`. |
| `session-start.sh` | (reads nothing) | Fine. SessionStart input carries `source` and optionally `model`, `agent_type`, `session_title`; the script needs none. |

### 1.3 stdout JSON the repository emits

The reference now states this **explicitly**, where the first run could only
record that every example included it:

> `hookSpecificOutput` is a nested object for events that need richer control.
> **It requires a `hookEventName` field set to the event name.**

All three emitters comply: `mpb_deny "PreToolUse"` (both guards),
`mpb_context "PostToolUse"` / `mpb_context "SessionStart"`, and `mpb_message`
(top-level `systemMessage`, no `hookSpecificOutput`). **No defect.**

- **PreToolUse deny.** `lib.sh:77-90` emits
  `hookSpecificOutput.{hookEventName,permissionDecision:"deny",permissionDecisionReason}`
  and exits 0. Matches the current form exactly. The reference's note is
  unchanged: "PreToolUse previously used top-level `decision` and `reason`
  fields, but these are deprecated for this event… Other events like PostToolUse
  and Stop continue to use top-level `decision` and `reason` as their current
  format."
- **`additionalContext`.** `lib.sh:95-107` nests it under `hookSpecificOutput`.
  Documented placement, and the reference names where each event's reminder
  lands: PostToolUse "next to the tool result", SessionStart "at the start of
  the conversation, before the first prompt". Correct for both call sites.
- **`systemMessage`.** Universal-fields table, verbatim: "Warning message shown
  to the user. In Agent SDK and `--output-format stream-json` output, it can
  arrive as an `SDKInformationalMessage`." `stop-gate.sh:145` uses exactly this
  for warnings that must not prolong the turn, which is right: the Stop section
  says `additionalContext` "keeps the conversation going through the same loop
  protections as `decision: \"block\"`". **Caveat, honestly stated:** the
  universal-fields preamble says "Every event accepts them, but some events
  discard them or deliver `systemMessage` somewhere other than the transcript.
  Each event's section says so." I located explicit discard/redirect statements
  for `ConfigChange`, `FileChanged` and `DirectoryAdded`, and **no such
  statement in the Stop section** — so `systemMessage` on Stop behaves as the
  universal table describes. I could not locate a Stop-specific sentence
  confirming it positively **(not located)**; the absence of a discard note is
  the basis for the claim.
- **New limit worth knowing.** "Hook output strings, including
  `additionalContext`, `systemMessage`, and plain stdout, are capped at 10,000
  characters. Output that exceeds this limit is saved to a file and replaced
  with a preview and file path." Also: "Your hook's stdout must contain only
  the JSON object." Every repository hook redirects tool chatter to
  `/dev/null` or captures it in a variable, so stdout is JSON-only. See §6.2
  for the one place the cap could bite.

### 1.4 Exit codes

Unchanged and matched by the repository:

- Exit 2 blocks on PreToolUse and Stop; `stop-gate.sh:137` uses exit 2 + stderr,
  and the reference says "The blocking message is the reason from your JSON's
  blocking decision when it makes one, and your stderr text otherwise."
- The guards deny with exit 0 + JSON, never mixing. Matches "Choose one
  approach per hook".
- **Confirms the §0-#2 fix was necessary**, verbatim: "Stderr from a hook that
  exits 0 goes to the debug log only, never the transcript, and Claude never
  sees it… To surface a warning to Claude from a `PostToolUse` or
  `PostToolUseFailure` hook, exit 2 instead."
- New since the first run's wording: "A hook that exits 2 while printing JSON
  that fails JSON output schema validation still blocks: Claude Code uses
  stderr as the blocking reason and records the validation failure in the debug
  log. Before v2.1.214, Claude Code treated that combination as a non-blocking
  error and the action proceeded." No repository hook mixes the two.
- Hooks still fire under every permission mode, including `bypassPermissions`
  (hooks guide), so the second layer the specification claims is real.

### 1.5 Loop guard

`stop-gate.sh` blocking again after `stop_hook_active` is deliberate and is now
documented in the script (`:21-26`) and in `11-agentic-sdlc.md`. The cap
(`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`, default 8) is quoted correctly at
`stop-gate.sh:135`. Nothing to change.

---

## 2. Permissions

### 2.1 Rule syntax the repository relies on — all still valid

- `Bash(brew install *)` style. The matching table is unchanged: `Bash(ls *)`
  matches `ls -la` and bare `ls` but not `lsof`; `Bash(ls*)` also matches
  `lsof`. The repository's space-less deny forms (`Bash(brew update*)`,
  `Bash(port sync*)`, …) therefore over-match, which is harmless in a deny list.
- `Edit(//opt/local/**)`. Confirmed against the pattern table: `//path` =
  "Absolute path from filesystem root"; `/path` = "Path relative to the settings
  source", which in project settings is `<primary working directory>/path`.
  The current `//` form is correct.
- `Edit(~/.zshrc)`. `~/path` = "Path from home directory". Correct.
- `WebFetch(domain:example.com)`. Still valid, and there is a **new
  distinction** in the repository's favour: only the `domain:` form "adds its
  domain to the sandbox's allowed or denied domain list"; a bare `WebFetch`
  allow rule "Doesn't change which hosts sandboxed commands can reach." The
  eight `WebFetch(domain:…)` entries are therefore already the form that will
  carry over when the `sandbox` SHOULD is taken up (§4.1).
- `disableBypassPermissionsMode: "disable"`. Verbatim: "set
  `permissions.disableBypassPermissionsMode` or `permissions.disableAutoMode`
  to `\"disable\"`". `"disable"` is the only documented value, and the key sits
  under `permissions`. `settings.json:151` is correct.
- `autoMemoryEnabled`. Still a documented settings key — settings reference
  index row: "`autoMemoryEnabled` — Turn auto memory off or on". Boolean.
  `settings.json:153` is valid.
- `env`. Documented: "Set environment variables for every session and for the
  subprocesses Claude Code starts from it… object mapping variable names to
  string values." All five values in `settings.json:154-160` are strings.
  Valid. Timing note relevant to CI: "From project and local settings: after
  you trust the workspace, or at startup in `-p` mode, which never shows the
  trust dialog".
- Precedence, verbatim and unchanged: "If a tool is denied at any level, no
  other level can allow it… deny rules from any scope are evaluated before
  allow rules."
- The fragility warning `11-agentic-sdlc.md:130-132` cites is still present
  verbatim on the permissions page, so the two-layer argument still holds.

### 2.2 Redirections — the `//` fix bought more than it looks like

Verbatim: "Claude Code checks the target of an output redirection, such as `>`,
`>>`, or `2>`, as a file write. The check covers your `Edit` allow and deny
rules, protected paths, and the working directories… A target that starts with
`~` or contains a glob character needs approval."

The first run noted that, with the broken single-slash anchoring,
`echo x > /usr/local/foo` was caught by neither the rule nor the hook. With
`Edit(//usr/local/**)` in place the **rule now catches it**. Worth recording:
the `//` fix closed a Bash-redirection gap, not only an `Edit`-tool gap.

### 2.3 Plan mode and Bash-driven writes — asked for explicitly

The docs do **not** say plan mode blocks Bash-driven file writes. Exactly what
they say:

- Permissions page, mode table: `plan` — "Claude reads files and runs read-only
  shell commands to explore but doesn't edit your source files; with auto mode
  available, classifier-approved commands also run."
- Settings reference, `permissions.defaultMode`: `"plan"`: "Claude Code reads
  and plans but blocks edits until you approve a plan".
- Permissions page, read-only command set: "Without an ask rule, the built-in
  read-only commands still run without prompting, and **any other shell command
  goes through the regular permission flow while you are still planning**".
- Sandboxing page: "In plan mode, the rule isn't skipped: it prompts for
  sandboxed commands too, including read-only ones. Before v2.1.212, the skip
  applied in plan mode as well."
- Permission modes page: "In sessions with bypass permissions available, Claude
  Code also doesn't enforce plan mode's blocks. Claude is still instructed to
  plan without editing, but a file edit or shell command it attempts during
  planning runs without prompting."

**Conclusion, stated as the docs state it:** plan mode blocks the `Edit`/`Write`
*tools*. A file write performed *through Bash* is not categorically blocked —
it goes through the regular permission flow, meaning a prompt in Manual mode or
the classifier in auto mode. For this repository that matters twice over:

1. `guard-provider-mutation.sh`, not the mode, is what actually stops a
   Bash-driven dotfile or prefix write during planning. `CLAUDE.md`'s advice to
   start non-trivial work in plan mode is good practice but is not a boundary.
2. `disableBypassPermissionsMode: "disable"` is doing more work than its own
   sentence suggests: it is also what keeps a session in which "Claude Code
   doesn't enforce plan mode's blocks" from being available at all.

Neither is a defect. Both are worth a sentence in `11-agentic-sdlc.md` if that
chapter is next edited, because the current text implies plan mode is a
containment mechanism.

---

## 3. Headless

- `--allowedTools` is a **grant**, not a restriction: "Use `--allowedTools` to
  let Claude use certain tools without prompting."
- `--permission-mode dontAsk`: "Claude Code denies anything not in your
  `permissions.allow` rules or the read-only command set, which is useful for
  locked-down CI runs. `AskUserQuestion`, connector tools your organization set
  to `ask`, and MCP tools marked `requiresUserInteraction` are denied even when
  an allow rule matches."
- For `-p`, "the built-in starting permission mode is Manual on every plan".
- `--disallowedTools` exists as a CLI flag and, per the settings reference,
  "only restrict[s], so they can't widen what the managed rules grant."

These three facts together produce §5.1.

---

## 4. Documented mechanisms the repository does not use

### 4.1 Sandboxing — already a recorded SHOULD, so listed, not re-argued

`11-agentic-sdlc.md:118-123` (permission-boundary section) already defers the
`sandbox` block to the Foundation Pass, with the reason. **Deliberately not
followed; not re-raised.** What this run adds is the current key list, so the
Foundation Pass does not have to re-fetch, plus one fact that strengthens the
case:

> Filesystem restrictions in the sandbox combine the `sandbox.filesystem`
> settings with Read and Edit deny rules; both are merged into the final sandbox
> boundary. Network restrictions combine `WebFetch(domain:...)` permission rules
> with the sandbox's `allowedDomains` and `deniedDomains` lists.

That is, the existing `Edit(//opt/local/**)` / `Edit(//usr/local/**)` /
`Edit(//opt/homebrew/**)` denies and the eight `WebFetch(domain:…)` allows would
become OS-level Bash-subprocess boundaries **with no new rules written** — which
is precisely the gap `11-agentic-sdlc.md:131-132` names ("`Read`/`Edit` deny
rules do not apply to what a subprocess does").

Keys, all from the settings reference: `sandbox.enabled`,
`sandbox.filesystem` (object with `allowWrite`, `denyWrite`, `denyRead`,
`allowRead`, plus `allowManagedReadPathsOnly` and `disabled`),
`sandbox.network` (`allowedDomains`, `deniedDomains`,
`allowManagedDomainsOnly`, `httpProxyPort`), `sandbox.excludedCommands`
(entries use `Bash(...)` rule syntax), `sandbox.allowUnsandboxedCommands`,
`sandbox.allowAppleEvents` (default `false`; setting it `true` "removes
code-execution isolation" — leave it alone), `sandbox.bwrapPath` /
`sandbox.socatPath` (Linux/managed), and `autoAllowBashIfSandboxed`
(default `true`).

### 4.2 `permissions.disableAutoMode` — the strongest unadopted candidate

`11-agentic-sdlc.md:175-177` ("What this is not") states as a limitation that
"Claude Code's own auto mode runs classifier-approved commands without a human
in the loop". The permission-modes page adds the fact that makes this live:
"On Pro, Max, and Team plans, the built-in starting permission mode is auto
mode." So the limitation the chapter merely *states* is the default a session
starts in.

`permissions.disableAutoMode: "disable"` would enforce it, using the same key
family and the same value as the `disableBypassPermissionsMode` the project
already sets.

**Recommendation, not a defect, and deliberately not applied by me.** It trades
away real convenience, and `11-agentic-sdlc.md:105-107` says explicitly that
"the mode is the operator's choice per session". This is an owner-level call.
Mitigations the docs record, for whoever decides: "On entering auto mode, broad
allow rules that grant arbitrary code execution are dropped", "Explicit deny
rules are always respected", "Writes to protected paths route to the classifier
even when an allow rule matches", and content-scoped ask rules such as
`Bash(git push *)` "still force a prompt".

### 4.3 Hook events and fields not used

- **`SubagentStop`.** Real gap, worth considering. `Stop` does not fire for a
  subagent turn; `SubagentStop` is its own event, with `stop_hook_active`,
  `agent_id`, `agent_type` (the matcher value) and `agent_transcript_path`, and
  it can block. This repository delegates by default and one subagent
  (`provider-researcher`) has `Write`/`Edit`. Today a subagent's edits are
  covered by the two `PreToolUse` guards and `PostToolUse`, but never by the
  definition-of-done gate. A matcher-filtered `SubagentStop` running a subset
  of `stop-gate.sh` would close that. Reversible; recommended for the
  Foundation Pass rather than applied blind here, because it changes how every
  delegated task terminates.
- **`PermissionDenied` / `PermissionRequest`.** Tool events that could record
  every denial for the pass report, instead of the current situation where a
  guard's deny reason is visible only in the transcript.
- **The `if` field on a hook handler.** "`if` uses permission rule syntax to
  match against the tool name and arguments together, so `\"Bash(git *)\"` runs
  when any subcommand of the Bash input matches `git *` and `\"Edit(*.ts)\"`
  runs only for TypeScript files." Could narrow `post-edit-checks.sh` to the
  extensions it actually handles. Cosmetic; the script already dispatches on
  extension.
- **`Tool(param:value)` deny/ask rules**, e.g. `Bash(run_in_background:true)`,
  `Agent(model:opus)`. No current need.
- **`CLAUDE_ENV_FILE`** (SessionStart/CwdChanged) for persisting environment
  variables. Not needed; `settings.json`'s `env` covers it.
- **`disableAllHooks`** is documented and is already named in
  `11-agentic-sdlc.md` as an escape hatch that obliges disclosure. Correct.

---

## 5. Workflows against the GitHub Actions page

### 5.1 DEFECT — `.github/workflows/claude.yml:47-55`

The workflow states a safety property it does not have.

```yaml
47            settings: .claude/settings.json
...
50            # can read, edit and inspect git. It does NOT build or test: a
51            # pull-request head can carry a build.rs, a proc macro or a #[test]
52            # that runs with this job's secrets.
53            claude_args: >-
54              --max-turns 25
55              --allowedTools "Read,Grep,Glob,Edit,Write,Bash(git status *),Bash(git diff *),Bash(git log *),Bash(cargo fmt -- --check)"
```

The GitHub Actions page describes the two as **alternative grant mechanisms**,
not as a restriction plus a grant (verbatim): "Claude has no shell or GitHub API
access until you grant the tools the prompt needs, with `--allowedTools` in
`claude_args` **or** a `permissions.allow` rule in the `settings` input."

`.claude/settings.json`'s `permissions.allow` already contains
`Bash(cargo build*)` (`:131`), `Bash(cargo check*)` (`:130`),
`Bash(cargo clippy*)` (`:132`), `Bash(cargo test*)` (`:133`),
`Bash(cargo nextest *)` (`:138`) and `Bash(cargo metadata*)` (`:135`). Passing
that file as the `settings` input grants every one of them in this run. Each of
`cargo build`, `cargo check`, `cargo clippy`, `cargo test` compiles and executes
`build.rs` and proc macros from the pull-request head, and `cargo test` runs
`#[test]` bodies — the exact threat the comment at `:50-52` says the workflow
avoids. `MPB_SKIP_STOP_GATE: "1"` does not help: it disables the Stop gate, not
the allow rules.

`--allowedTools` cannot narrow this, because it only grants. The documented fix
uses deny, which "no other level can allow": either

- add to `claude_args` — `--disallowedTools "Bash(cargo build*),Bash(cargo check*),Bash(cargo clippy*),Bash(cargo test*),Bash(cargo nextest*),Bash(cargo run*),Bash(cargo bench*)"`
  (settings reference: `--disallowedTools` rules "only restrict, so they can't
  widen what the managed rules grant"); or
- point `settings:` at a CI-specific file that inherits the deny list and adds
  those entries, keeping `.claude/settings.json` unchanged for local work.

The second is the better shape for this repository — the constraint is then
readable in one file rather than inferred from the union of two — but the first
is a one-line change and is reversible. Either way the comment at `:48-52`
should say *which* mechanism enforces the claim.

### 5.2 `.github/workflows/claude.yml` — everything else matches

`:10-14` triggers, `:18` `if: contains(github.event.comment.body, '@claude')`,
`:21-26` permissions (`contents: write`, `pull-requests: write`,
`issues: write`, `id-token: write`, `actions: read`), `:35-37`
`actions/checkout@v6` with `fetch-depth: 1`, and `:38` `@v1` with
`anthropic_api_key` are identical to the page's interactive example, including
its annotations ("`id-token: write`: required for the Claude Code GitHub
Action's default GitHub App authentication"; "`actions: read`: lets Claude read
CI results on PRs"). `--max-turns` and `--allowedTools` are both on the page's
`claude_args` list, and `--allowed-tools` is noted as an accepted alias. No
deprecated input is used: `@beta`, `mode`, `direct_prompt`, `max_turns`,
`model` and `custom_instructions` all appear only in the "Upgrade from beta"
section, and none is present.

One honest note on the `settings` input: the page's inputs table now defers to
the action's own configuration reference ("For the full input list, see the
Claude Code GitHub Action's configuration reference",
<https://github.com/anthropics/claude-code-action/blob/main/docs/usage.md#inputs>).
I could not locate a verbatim per-input description of `settings` on this page
in this run **(not located)**; the input is however referred to by name in the
"Run on a schedule" text quoted in §5.1, so it is documented, and the first
run's record of it stands. Not a drift finding.

### 5.3 `.github/workflows/claude-code-review.yml` — no contradiction

`:28-34` still matches the page's "Run a skill" review example
(`plugin_marketplaces`, `plugins: code-review@claude-code-plugins`, a `/`-prefixed
`prompt`, and `claude_args: '--allowedTools "mcp__github_inline_comment__create_inline_comment"'`).
Read-only `contents`/`pull-requests`/`issues` plus `id-token: write` satisfies
`11-agentic-sdlc.md`'s "no unattended agent run holds push, merge or release".
Two facts from the page worth carrying into `.claude/README.md` when the
workflows are activated, neither a defect: Claude "skips draft and closed pull
requests, pull requests it judges not to need a review, such as automated or
trivial ones, and pull requests that already have a comment from Claude"; and
`github_token: ${{ secrets.GITHUB_TOKEN }}` should *not* be passed, because
"GitHub doesn't trigger workflows on commits made with the default
`GITHUB_TOKEN`" — the workflow correctly omits it.

### 5.4 `.github/workflows/repository-validation.yml` — outside the page's scope

Plain shell; the GitHub Actions page contradicts nothing in it. Re-checked for
soundness: `:25-26` `permissions: contents: read` is least privilege; `:35`
`sudo apt-get` runs on the runner, outside any agent session, so the
repository's `Bash(sudo *)` deny (a Claude Code permission rule) does not apply;
`:38` `shellcheck -x -P SCRIPTDIR .claude/hooks/*.sh` covers `lib.sh` and
resolves the `# shellcheck source=lib.sh` directives; `:40-43` runs
`test-hooks.sh` with `CLAUDE_PROJECT_DIR` set; `:45-50` runs the gate with
`MPB_GATE_FULL=1`; `:52-56` checks the LICENSE digest in the correct two-space
`sha256sum --check` format. Unchanged minor points from the first run, still
cosmetic: `on: push` plus `pull_request` double-runs on PR branches, and the
job has no `timeout-minutes`.

---

## 6. Smaller findings, none blocking

### 6.1 `lib.sh:23-25` — jq and python3 disagree on a `false` boolean

The jq branch computes `getpath(...) // ""`, and jq's `//` treats `false` as
empty, so `stop_hook_active: false` yields `""`. The python3 branch yields
`"false"`. The only consumer (`stop-gate.sh:134`) compares against `"true"`, so
both parsers produce the same behaviour today. Worth a comment in `lib.sh`
before a second caller reads a boolean field and gets a different answer
depending on which parser is installed. Not a documentation-contract issue.

### 6.2 `post-edit-checks.sh:28-30` — unbounded `additionalContext`

`sc_out` is passed through whole. The reference caps hook output strings at
10,000 characters, after which the content "is saved to a file and replaced
with a preview and file path". Not a failure, but `stop-gate.sh` already tails
clippy and test output to 40 lines (`:102`, `:111`) and this hook does not. A
`tail -40` here would keep the finding legible for the same reason.

### 6.3 `test-hooks.sh` — two small gaps

- `assert` (`:38`) captures stdout and stderr together (`2>&1`), so the
  `context` verdict (`:44`) would pass on a stderr line that happened to contain
  `additionalContext`. Separating the streams would make the assertion mean
  what its name says.
- The `systemMessage` warning path (`stop-gate.sh:142-146`, `mpb_message`) is
  never asserted, while the block path and the escape hatch both are. Since
  `11-agentic-sdlc.md:156-158` makes "warnings reach the human as a system
  message rather than prolonging the turn" a MUST, it should have a case.

### 6.4 Deny-list parity, carried over

`guard-provider-mutation.sh:65` additionally denies `port configure|extract|
patch|dmg|pkg|mpkg`, which have no counterpart in `settings.json`'s deny list.
The reverse no longer holds — `brew migrate` and `port edit` are now in both.
Harmless (the hook is the stricter layer, and the specification says the hook is
where enforcement that matters lives), but parity would make the two readable
against each other.

---

## 7. Consolidated list

**Defect (documented contract not met):**

1. `.github/workflows/claude.yml:47-55` — the `settings` input and
   `--allowedTools` are alternative grants, so `permissions.allow`'s
   `Bash(cargo build*)`, `cargo check*`, `cargo clippy*`, `cargo test*`,
   `cargo nextest *` all apply, contradicting the comment at `:50-52`. Fix with
   `--disallowedTools` in `claude_args`, or a CI-specific settings file whose
   `deny` list covers them. §5.1.

**Recommendations, not applied:**

2. `permissions.disableAutoMode: "disable"` would enforce what
   `11-agentic-sdlc.md:175-177` only states. Owner-level trade-off. §4.2.
3. A `SubagentStop` gate. The `Stop` gate does not cover subagent turns. §4.3.
4. `post-edit-checks.sh:28-30` — tail `sc_out` against the 10,000-char cap. §6.2.
5. `test-hooks.sh` — separate stdout from stderr in `assert`; add a
   `systemMessage` case. §6.3.
6. `lib.sh:23-25` — comment the jq/python `false` divergence. §6.1.
7. Deny-list parity between `settings.json` and the mutation guard. §6.4.
8. If `11-agentic-sdlc.md` is next edited: plan mode does not contain
   Bash-driven writes, and `disableBypassPermissionsMode` is part of why plan
   mode's blocks are enforced at all. §2.3.

**Deliberately not followed, recorded so the next run does not re-raise it:**

9. The `sandbox` block. Already a SHOULD deferred to the Foundation Pass in
   `11-agentic-sdlc.md`. §4.1 records the current key list and the fact that
   the existing `Edit(//…)` denies and `WebFetch(domain:…)` allows merge into
   the sandbox boundary for free.
10. Space-less trailing `*` in ~20 deny/allow rules (first run, finding 11) and
    the redundant `Bash(su *)` / `Bash(su)` pair. Style.

**Pages unfetched:** none.
