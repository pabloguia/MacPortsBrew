<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification: hooks, permissions, headless, GitHub Actions (2026-11-03)

Date read: **2026-11-03**, by a read-only subagent. No hook script, `test-hooks.sh` or mutating command was run, so every statement about script behaviour comes from reading the code. The previous note is `2026-10-30-hooks-permissions-ci.md`. "Changed" means a difference from that note's claims or the files it described. It is not a page diff.

**Verdict: no contract break.**

Every script still reads documented input fields and emits a documented form. Every field, value, flag and input the repository uses is still documented.

Three findings are new and matter more than the carried list:
1. **The Linux "Repository gate" CI step cannot fail.** `stop-gate.sh` has exited 0 on a block since 2026-10-18. This is a repository defect, not a docs change, and the 10-30 note missed it (§3 item 1).
2. **The Bash guards do not see `Monitor` commands.** Bash allow rules do apply to `Monitor`, so an allowed `git commit` could skip the commit gate (§3 item 2).
3. **`display_report: "true"` goes against the pinned action's warning** about untrusted input (§3 item 3).

## What changed since 2026-10-30

- **Repository:** nothing under `.claude/hooks/`, `.claude/settings.json` or `.github/workflows/` changed after `35bc7a5`. `git diff 35bc7a5 HEAD` over `.claude` and `.github/workflows` shows only `.claude/README.md` (`d5bfca6`, a header note). Line numbers from the previous note still hold. HEAD was on `wo16/G-2-probe`.
- **10-30 §3 item 3 (mods): partly done.** `docs/spec/11-agentic-sdlc.md:46-55` now records the mod caveat and the unfiltered cross-session envelope as waiting for the owner.
- **10-30 "unverified" items settled:** `--tools`, `--setting-sources`, `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB` and `display_report` (§1, CI). `autoMemoryEnabled` was confirmed by the sibling note `2026-11-03-memory-settings-plugins.md`.
- **Corrections to 10-30:** its §4 item 6 treated `--bare` only as not needed. [HL] states it will become the `-p` default (§3 item 5). Its §1 CI list accepted `display_report` as unverified, and the input's warning is now read (§3 item 3).

## Sources (fetched 2026-11-03)

| Tag | URL | Result |
|---|---|---|
| [H] | https://code.claude.com/docs/en/hooks.md | HTTP 200; 286,092 chars; highest version cited v2.1.309 |
| [P] | https://code.claude.com/docs/en/permissions.md | HTTP 200; highest version cited v2.1.314 |
| [HL] | https://code.claude.com/docs/en/headless.md | HTTP 200 |
| [GA] | https://code.claude.com/docs/en/github-actions.md | HTTP 200 |

None of the four pages moved. The non-`.md` URLs were tried first and also resolved. The fetch tool cut `/hooks` off at 100,000 characters, so [H] was read in full from the `.md` variant.

Fetched beyond the four, to settle flags the repository uses:

| Tag | URL | Result |
|---|---|---|
| [CLI] | https://code.claude.com/docs/en/cli-reference.md | HTTP 200 |
| [ENV] | https://code.claude.com/docs/en/env-vars.md | HTTP 200 |
| [TR] | https://code.claude.com/docs/en/tools-reference.md | HTTP 200 |
| [PM] | https://code.claude.com/docs/en/permission-modes.md | HTTP 200 |
| [SR] | https://code.claude.com/docs/en/settings-reference.md | HTTP 200 |
| [XS] | https://code.claude.com/docs/en/cross-session-messaging.md | HTTP 200 |
| [ACT] | https://raw.githubusercontent.com/anthropics/claude-code-action/fa2b2666b747000bf42767d1f332065b375e3c8f/action.yml, plus `bun.lock` at the same SHA | HTTP 200 |
| [GHAPI] | https://api.github.com/repos/anthropics/claude-code-action/compare/fa2b2666b747000bf42767d1f332065b375e3c8f...4845bacbb38b3c4b15d948f035349786f678e068 | HTTP 200 |

Every citation below is `[tag §section]`, meaning that row's URL as read on **2026-11-03**.

**Not fetched, so unverified here:** `sandboxing`, `plugins/mods/*`, `worktrees`, `terminal-config`, and the action's `docs/usage.md` and `docs/security.md`. The `sandbox` settings keys are not stated below for that reason.

## 1. Contracts the repository uses: still documented, still met

### Script by script

| Script | Reads | Emits | Documented at |
|---|---|---|---|
| `session-start.sh` | no stdin | `additionalContext` (`:57`) | [H §Decision control; §Add context] |
| `arm-guard-edit.sh` | `prompt`, `session_id` | plain stdout (`:154`, `:161`) or `additionalContext` (`:209`, `:234`, `:266`) | [H §UserPromptSubmit input; §UserPromptSubmit decision control] |
| `guard-provider-mutation.sh` | `tool_input.command` | `permissionDecision: "deny"` | [H §PreToolUse decision control] |
| `commit-gate.sh` | `tool_input.command` | deny, or `additionalContext` (`:37`, now in the PreToolUse field table) | [H §PreToolUse decision control] |
| `guard-protected-paths.sh` | `tool_input.file_path`, `tool_input.notebook_path`, `session_id` | deny | [P §Tool-specific rules] names `notebook_path`; [H] still has no NotebookEdit input table |
| `post-edit-checks.sh` | `tool_input.file_path` | `additionalContext` | [H §Add context] |
| `stop-gate.sh` | `stop_hook_active` | `{decision:"block",reason}` or `systemMessage` | [H §Stop decision control] |

Each code path prints at most one JSON object, and diagnostics go to stderr. That meets "parsed as JSON only if it is a single JSON object" [H §Exit code output].

### Semantics the scripts rely on, all unchanged

- **Exit codes:** exit 2 blocks on PreToolUse, and any other non-zero exit does not block [H §Exit code output].
- **PreToolUse timeout:** a timed-out hook does not block without `onFailure: "block"` [H §Timeouts]. `commit-gate.sh:69-83` rests on this.
- **Stop cap:** the 8-consecutive-continuation cap still resets on a tool call, and `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` still raises it. `additionalContext` on Stop continues the turn [H §Stop input; §Stop decision control]. That supports `lib.sh:106-108`.
- **Context cap:** 10,000 characters. Over the cap, Claude Code writes a file and passes a 2,000-character preview. `reason` is not among the capped strings [H §JSON output]. `lib.sh:113-115` stays under the cap.
- **SessionStart sources:** `startup|resume|clear|compact|fork` [H §Matcher patterns]. `settings.json:203-214` has no matcher, which is correct for the marker delete.
- **Trust:** a `-p` session treats the folder as trusted for hooks [H §Workspace trust].

### Configuration values in `settings.json`

- **Hook fields:** `matcher`, `timeout`, `statusMessage` and shell-form `"${CLAUDE_PROJECT_DIR}"` in double quotes are documented [H §Hook handler fields; §Reference scripts by path].
- **Matchers:** `Bash` and `Edit|Write|NotebookEdit` take the exact-match path [H §Matcher patterns].
- **Rule forms:** `Bash(prefix *)`, `//abs`, `~/`, `./`, `WebFetch(domain:…)` and an `ask` rule are valid [P §Wildcard patterns; §How path patterns match].
- **Bypass switch:** `disableBypassPermissionsMode: "disable"` is valid [P §Permission modes].

### CI flags and inputs, now all read at source

- **`--setting-sources user`:** reads neither the project's settings files nor its `.mcp.json` [CLI; P §What runs before you trust a folder].
- **`--permission-mode dontAsk`:** "Locked-down CI and scripts" [HL §Auto-approve tools; PM].
- **`--tools "Read,Grep,Glob,Bash"`:** documented. The default set omits Glob and Grep on Linux, so naming them is required [CLI].
- **`--allowedTools`, `--disallowedTools`:** a bare name removes the tool, and a scoped rule denies "as written" [CLI]. `--max-turns` is documented [CLI; GA §Pass CLI arguments].
- **Action inputs:** `anthropic_api_key`, `github_token`, `settings` (a path is accepted), `prompt` and `claude_args` are documented [GA §Action parameters].
- **`id-token: write` omitted:** consistent. [GA] ties it to default App authentication and to federation, and neither is used.
- **`display_report`:** real, at `action.yml:152` of the pin [ACT], and still not on [GA]'s table of common inputs. See §3 item 3.
- **`permissions.blockReadsOutsideWorkingDirectories`:** valid, requires v2.1.257 [SR].
- **`CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`:** documented, leaves GitHub tokens in place, and the action sets it only for `allowed_non_write_users` [ENV; ACT `action.yml:301`]. The workflow comments are accurate.

## 2. Resolved since 2026-10-30

Only the spec 11 record of the mod caveat (see above). No guarded-path finding was applied.

## 3. Findings

### 1. New, highest value: the Linux "Repository gate" step cannot fail

- **Where:** `.github/workflows/repository-validation.yml:61-66` runs `.claude/hooks/stop-gate.sh` as a plain step.
- **Why it cannot fail:** the documented Stop form is exit 0 plus `{decision:"block"}` [H §Stop decision control]. Since `7aa07db` (2026-10-18) the script uses that form at `stop-gate.sh:44-49` and `:269-284`. Its only `exit` statements are `exit 0` (`:48`, `:57`, `:283`, `:294`), and `:38` turns off `set -e`. A failing gate therefore prints JSON and the step stays green.
- **How it was missed:** `docs/spec-evolution.md:3115` records the older exit-2 form. `7aa07db` changed the script and did not touch this workflow. `test-hooks.sh:225` asserts the JSON form, which is right for the hook and does not cover the CI consumer.
- **What is still caught:** the macOS matrix runs `scripts/validate.sh --full` directly (`repository-validation.yml:115`), and that exits non-zero. These stop-gate checks fail on no CI runner unless something else repeats them:
  - links (`:68-75`);
  - JSON parse and `bash -n` (`:77-84`);
  - the no-mutation boundary scan (`:86` onward);
  - Linux `validate.sh` under `MPB_GATE_FULL=1`.

  Whether `validate.sh` duplicates any of these was not checked. Whether this job is a required check was not checked either.
- **Fix (guarded path, the owner's):** keep the hook contract and make the step read it:
  `out="$(.claude/hooks/stop-gate.sh </dev/null)"; printf '%s\n' "$out"; if printf '%s' "$out" | jq -e '.decision == "block"' >/dev/null 2>&1; then exit 1; fi`
  Add a `test-hooks.sh` case that runs this wrapper on a broken tree.

### 2. New: the Bash guards do not see `Monitor` commands

- **Docs:** "When Monitor runs a command, it uses the same permission rules as Bash, so `allow` and `deny` patterns you have set for Bash apply here too" [TR §Monitor tool]. [H §PreToolUse] lists Monitor as a tool of its own ("Bash, PowerShell, Monitor, Edit, Write, and NotebookEdit calls"). A matcher of letters only is an exact match [H §Matcher patterns].
- **Effect:** `settings.json:229` (`"matcher": "Bash"`) runs neither `guard-provider-mutation.sh` nor `commit-gate.sh` on a Monitor call.
- **Sharpest case (inference from those three statements, not tested):** `Bash(git commit *)` at `settings.json:104` would approve `git commit` through Monitor with no commit gate. The push, merge and `gh api` refusals are also hook-only (`guard-provider-mutation.sh:140-141`), so through Monitor they fall back to a prompt.
- **CI is not affected:** `--tools` excludes Monitor.
- **PowerShell:** opt-in on macOS, with `tool_input.command` the same as Bash [TR §PowerShell tool].
- **Fix:** `"matcher": "Bash|Monitor|PowerShell"`, the shape [H §Hook lifecycle] shows for `Bash|PowerShell`. A WebSocket Monitor call has `ws` in place of `command` [TR §Monitor tool], and both guards exit 0 at `:28`/`:35` on an empty command.

### 3. New: `display_report: "true"` against the input's own warning

- **Docs:** [ACT `action.yml:152`], default `"false"`: "WARNING: This outputs Claude-authored content in the GitHub Step Summary. This should only be used in cases where the action is used solely with trusted input."
- **Repository:** `claude.yml:381` and `claude-code-review.yml:319` set it to `"true"`, while both prompts declare `work/` and `pr.diff` to be untrusted data.
- **Mitigating:** the owner gates dispatch, and the token is read-only.
- **Fix:** set `"false"` and read the run log, or record the accepted risk beside DP-039.

### 4. New, low: `Read(./.git/**)` does not cover `grep -r`

- **Docs:** Read deny rules "don't apply to a command that reads files without naming them, such as `grep -r pattern .`" [P §Read and Edit]. `grep` is in the read-only set, which runs without a rule, and "to require a prompt for one of these commands, add an `ask` or `deny` rule" [P §Read-only commands].
- **Repository:** `claude.yml:270-272` and `:425-427` present the deny as protecting the token the action writes into `./.git/config`. `contents: read` remains the hard bound, as the comment says.
- **Fix:** add `Bash(grep *)` to `--disallowedTools` at `claude.yml:438` and `claude-code-review.yml:346` (the Grep tool stays). Qualify the comment. [P] names the sandbox for OS-level enforcement.

### 5. New, forward risk: `--bare` will become the `-p` default

- **Docs:** "`--bare` is the recommended mode for scripted and SDK calls, and will become the default for `-p` in a future release." Bare mode does not read a hook in `~/.claude`, and settings come through `--settings` [HL §Start faster with bare mode].
- **Repository:** both workflows keep their deny rules and hooks through the action's `settings` input, which they document as written to user settings (`claude.yml:318-324`). If the default flips, those could silently stop applying.
- **Bounded today:** the pin locks `@anthropic-ai/claude-agent-sdk@0.3.259` [ACT `bun.lock`]. Treating that as CLI v2.1.259 is an inference. The pin is 56 commits behind the `v1` tag, which moved on 2026-11-03 [GHAPI].
- **Fix:** add to the pin-bump checklist that a run must confirm the denies and hooks still load, and pass `--settings` explicitly if they do not.
- **Side effect of the same pin:** version-gated features are absent in CI if the inference holds, including `onFailure` (v2.1.295) and the `tee` target check (v2.1.269) [H §Block the action when a hook fails; P §Redirections].

### 6. New, wording: two header claims about the Bash route overstate it

- **Docs:** redirect targets and `tee` destinations are checked "as if Claude wrote or read that file directly", covering `Edit` deny rules and protected paths, and "a target that starts with `~`" needs approval [P §Redirections]. Protected-path writes prompt in `default` and `acceptEdits` [PM §Protected paths]. Arbitrary subprocesses stay outside this [P §Read and Edit].
- **Affected:**
  - `arm-guard-edit.sh:59-61` (`cat >> ~/.claude/settings.json`);
  - `arm-guard-edit.sh:65-67` ("one redirect writes the marker");
  - `guard-provider-mutation.sh:108-109` ("a deny rule on a path does not reach a subprocess", true only of unrecognized programs).
- **Fix:** qualify all three as "a shell redirect is checked; a script that opens the file itself is not". The protected-path list was not read completely, so whether the marker is on it is unverified. See §4 item 1.

### 7. New, low: `stop-gate.sh` has no watchdog

`stop-gate.sh:216` runs `validate.sh` unbounded under a 600 s timeout (`settings.json:276`). A timed-out hook "renders no decision" [H §Timeouts], and `onFailure` has no effect on Stop [H §Block the action when a hook fails]. The turn would end unchecked and silently. `commit-gate.sh:69-80` already solved this for commits. Fix: the same bounded run, blocking at the deadline.

### 8. Carried, still open

Each was re-read against today's pages.

| Item | Where | Documented fact | Fix |
|---|---|---|---|
| Non-human prompts arm-filter gap | `arm-guard-edit.sh:201-219` | Scheduled tasks, background subagents and other sessions fire the event [H §UserPromptSubmit]; no field separates them. [XS] does not show the envelope, so `<cross-session-message` is known only from this session's tool description. `test-hooks.sh` has no case | Add the envelope to the case list |
| Pasted text arms the escape | `arm-guard-edit.sh:223-231` | `<pasted_content id="…">` markers: "account for those lines" [H §UserPromptSubmit input] | Skip lines between the markers |
| `onFailure: "block"` absent | `settings.json:233,239,250` | A crash or missing script otherwise lets the call through [H §Block the action when a hook fails] | Add it to the three guards, not to `arm-guard-edit.sh` (there a failure blocks the prompt) |
| Worktree reads the wrong tree | `guard-provider-mutation.sh:153,157`; `stop-gate.sh:40`; `commit-gate.sh:45` | `CLAUDE_PROJECT_DIR` stays put, `cwd` follows Claude [H §Reference scripts by path] | Take repository state from `cwd` |
| No-space allow wildcards | `settings.json:105,106,113` (`git branch*` matches `git branch -D`), `114-117`, `136-138`, `140`, `144`, `146-147`, `155-156`, `160`, `170-176`, `180`, `182-183` | "The space before a trailing `*` is part of the rule" [P §Wildcard patterns] | `Bash(cmd *)` |
| `./` deny rules | `settings.json:97-98` | `./path` is relative to the current directory, `/path` to the settings source [P §How path patterns match] | `/` in project scope; CI's user scope anchors elsewhere, so keep `./` or use `//` there |
| Mods bound the deny and hook layers | `settings.json:4-99`, all guards, CI on an API key | Applies absent managed settings or a Team login [P §Precedence among rules, hooks, and mods] | Recorded in spec 11; the mods pages are still unfetched |
| No `concurrency:` | `claude.yml`, `claude-code-review.yml` | "Use GitHub's concurrency controls" [GA §Manage costs] | Add it; low value |
| `settings.local.json` | line 21 `Bash(rm -rf *)`; line 22 `Write(…)` | `Write(path)` rules never match, per 10-30 [P]; not re-read today beyond the line check | The operator's file |
| Re-entry blocks again | `stop-gate.sh:277-282` | "Check this value … to avoid blocking on a condition that will never resolve" [H §Stop input] | Deliberate; recorded once |
| Stale comment | `lib.sh:111` | Over-limit behaviour [H §JSON output] | Update the date and behaviour |

## 4. Documented mechanisms not used that would enforce a stated rule

1. **`Edit` deny rules on the escape's files, used as a Bash barrier.** `Edit(./.claude/.guard-edit-marker)` and `Edit(./.claude/.guard-edit-log)` would refuse a redirect or `tee` onto them [P §Redirections]. Today only `guard-protected-paths.sh` denies these paths, and it never sees Bash. This does not stop a subprocess. Hooks writing the marker are not tool calls, so they should be unaffected (inference). New to this note.
2. **The sandbox.** "For filesystem and network enforcement that doesn't depend on the command text, use sandboxing" [P §Bash rule limits]. It "applies to Bash, PowerShell, and Monitor" [P §Sandboxing]. `settings.json:44-46` is the case [P] warns about: a `curl` deny does not match by path or inside `sh -c`, "so pair it with the sandbox network allowlist when the restriction must hold". `guard-provider-mutation.sh:22-25` already says a text matcher is not a boundary. New to this note; the `sandboxing` page was not fetched, so its settings are not stated.
3. **`onFailure: "block"`.** See §3 item 8. The sibling 11-03 note reports that the best-practices page now recommends it.
4. **`ConfigChange`.** It can block a settings change "from taking effect" [H §ConfigChange], and would close the settings half of DP-020. Whether it fires on a Bash-redirect write is not stated.
5. **`FileChanged`.** A watcher that fires "no matter what changed the file", including Bash, but it cannot block. It registers "a literal filename in the working directory" [H §FileChanged], so a `.claude/…` subpath is not stated to work. It would detect the marker forgery that `arm-guard-edit.sh:65-71` admits.
6. **`permissions.disableAutoMode: "disable"`.** Absent beside `settings.json:192` and in the derive steps at `claude.yml:338-343` and `claude-code-review.yml:281-286` [P §Permission modes].
7. **`--permission-prompts none`** (v2.1.259). It tells Claude not to retry a denied request [HL §Turn off permission prompts]. Optional beside `dontAsk`, and it would save turns.
8. **`permissions.blockWritesOutsideWorkingDirectories`** (v2.1.314). A trade-off against worktree use, per the sibling note; the owner's.
9. **Exec form** (`command` plus `args`). "Prefer exec form" [H §Reference scripts by path]. A preference, not a defect.

## Recommendations (not adopted; `.claude/` and `.github/workflows/` wait for the owner)

In order of value:
1. §3 item 1: make the CI gate step able to fail, and add a test.
2. §3 item 2: widen the Bash matcher.
3. §3 item 8: the two prompt-channel filters, and `onFailure: "block"`.
4. §3 item 3: decide on `display_report`.
5. §4 items 1–2: the deny rules on the escape's files, then evaluate the sandbox.
6. §3 item 5: add the `--bare` check to the pin-bump checklist.
7. §3 items 4, 6, 7, and the remaining carried items.
