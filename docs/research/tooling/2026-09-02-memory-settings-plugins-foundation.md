# Claude Code documentation drift check — memory, settings, plugins, best practices (second run, 2026-09-02)

**This is the second `/tooling-recheck` run for this group on 2026-09-02**, run
for the Foundation Pass, on top of `2026-09-02-memory-settings-plugins.md`
(same day, earlier run). This note reports drift **since that run**, not a
restatement of everything it already established. Where a fact is unchanged
it is noted briefly with a pointer back to the first note rather than
re-quoted in full.

Read date for every page below: **2026-09-02**.

| Page | URL | Status |
|---|---|---|
| Memory | https://code.claude.com/docs/en/memory | fetched |
| Best practices | https://code.claude.com/docs/en/best-practices | fetched |
| Plugins | https://code.claude.com/docs/en/plugins | fetched |
| Settings (concept) | https://code.claude.com/docs/en/settings | fetched, but has no field-level schema (see below) |
| Settings reference (schema) | https://code.claude.com/docs/en/settings-reference | fetched twice; the WebFetch summarizer truncated the page both times (noted below) |
| Permissions (rule syntax) | https://code.claude.com/docs/en/permissions | fetched in full, saved to disk and grepped directly for exact quotes |
| Sandboxing | https://code.claude.com/docs/en/sandboxing | fetched (outside the four assigned pages; needed because `settings-reference` would not yield the sandbox JSON shape, and the task explicitly asks for it since the Foundation Pass must evaluate enabling it) |

All four assigned pages fetched successfully (HTTP-level; no 404s). As in the
first run, `settings` is a concept page with no key-by-key schema, so
`settings-reference` and `permissions` were fetched again, plus `sandboxing`
this time. **`settings-reference` did not fully load through WebFetch's
summarization step twice** ("the content you've shared appears to be
truncated at the end") — every fact taken from it below is a paraphrase by
that summarizer, not a verbatim quote, and is flagged as such. The
`permissions` and `sandboxing` pages were saved to disk in full (658 and 700
lines) and grepped directly, so those facts below are verbatim.

Repository files re-read (read-only): `CLAUDE.md` (90 lines, was 94),
`AGENTS.md` (113 lines, was 120), `.claude/settings.json`, `.claude/README.md`,
`.gitignore`, `docs/spec/11-agentic-sdlc.md` sections "Verified facts about
the tooling", "Memory files", "The permission boundary".

---

## 1. Drift resolved since the first run today

Four of the eight items the first note flagged are **already fixed** in the
repository. Recorded here as confirmations, not new findings:

1. **Inert deny rules — fixed.** `.claude/settings.json` now reads
   `Edit(//opt/local/**)`, `Edit(//usr/local/**)`, `Edit(//opt/homebrew/**)`,
   `Edit(//Library/**)` — the double-leading-slash absolute form the
   permissions page requires ("A pattern like `/Users/alice/file` isn't an
   absolute path... Use `//Users/alice/file` for absolute paths",
   permissions page, read 2026-09-02). These four rules now anchor at the
   filesystem root and are live, not inert.

2. **Stop gate ceiling — fixed.** Both `docs/spec/11-agentic-sdlc.md`
   ("Claude Code additionally stops honouring a `Stop` hook after eight
   consecutive blocks... the gate tells the agent when it is re-entering, and
   a turn that ends under that cap has ended with the gate failing, which the
   report must also say") and `.claude/README.md` ("Claude Code also stops
   honouring a `Stop` hook after eight consecutive blocks and ends the turn
   anyway; a report written under that ceiling is a report written with the
   gate failing, and must say so") now state the cap. This matches the
   best-practices page verbatim: "Claude Code overrides the hook and ends the
   turn after 8 consecutive blocks" (best-practices, "Give Claude a way to
   verify its work", read 2026-09-02). Note: `11-agentic-sdlc.md` additionally
   names the env var `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP` — that name was not on
   any of the four pages this group covers; it is the Hooks/permissions/CI
   group's fact to verify.

3. **Auto mode acknowledged in the boundary description — fixed.**
   `11-agentic-sdlc.md` "What this is not" now reads: "...and Claude Code's
   own auto mode runs classifier-approved commands without a human in the
   loop." This matches the best-practices and permissions pages' description
   of auto mode as the default starting mode on paid plans and its effect on
   plan mode ("with auto mode available, classifier-approved commands also
   run", permissions page, `plan` mode row, read 2026-09-02). The first
   note's finding #3 ("the spec's plan-mode assumptions predate \[auto
   mode\]") is resolved.

4. **`CLAUDE.local.md` now git-ignored — fixed.** `.gitignore` now has an
   explicit entry: `CLAUDE.local.md` (with comment "Personal per-project
   notes for Claude Code (loaded after CLAUDE.md, never committed)."), and
   both `CLAUDE.md` ("Never commit `.claude/settings.local.json` or
   `CLAUDE.local.md`; both are git-ignored for a reason") and
   `.claude/README.md` ("`CLAUDE.local.md`, if you create one for personal
   notes, is git-ignored") document it. The first note's finding #7 is
   resolved.

**Not re-verified this run** (belongs to the other `/tooling-recheck`
groups, not this one): finding #4, skill frontmatter vs. `SHOULD
disable-model-invocation: true` — would require reading `.claude/skills/*/SKILL.md`,
which is the "Subagents and skills" group's file list.

---

## 2. "Fragile" citation — resolved the other way: the docs still say it

The first note reported that the permissions page "no longer" uses the word
"fragile" to describe argument-constraining Bash patterns, and recommended
softening `11-agentic-sdlc.md`'s citation. **That finding does not hold up on
a full read of the page.** The permissions page, section "Permission rule
syntax", read 2026-09-02, verbatim:

> "Bash permission patterns that try to constrain command arguments are
> fragile. For example, `Bash(curl http://github.com/ *)` intends to restrict
> curl to GitHub URLs, but won't match variations like: Options before URL...
> Different protocol... Redirects... Variables... Extra spaces..."

`11-agentic-sdlc.md`'s "Hooks" section says: "Enforcement that matters is
implemented as a hook, because permission rules match command text: the
permissions documentation says argument-constraining Bash patterns are
fragile". **This is accurate as written, verbatim, today.** No change needed.
(The first note's search evidently missed this passage — a WebFetch summary
is not a substitute for grepping the full page, which is why this run saved
the permissions and sandboxing pages to disk and grepped them directly rather
than trusting the summarizer.)

The rest of the mechanics `11-agentic-sdlc.md` and `.claude/README.md`
implicitly rely on are also confirmed verbatim on this read:

- Compound-command splitting: "The recognized command separators are `&&`,
  `||`, `;`, `|`, `|&`, `&`, and newlines. A rule must match each subcommand
  independently."
- Wrapper stripping: "Before matching Bash rules, Claude Code strips a fixed
  set of wrappers... The stripped wrappers are `timeout`, `time`, `nice`,
  `nohup`, and `stdbuf`, plus the shell builtins `command` and `builtin`, and
  zsh's `noglob`."
- The `guard-provider-mutation.sh` two-layer rationale: exec wrappers such as
  `watch`, `setsid`, `ionice`, `flock`, and `find -exec`/`find -delete` are
  **not** covered by a prefix allow/deny rule ("can't be auto-approved by a
  prefix rule... in Manual mode they always prompt... a `Bash(find *)` rule
  doesn't cover these forms"). Development-environment runners (`direnv exec`,
  `devbox run`, `mise exec`, `npx`, `docker exec`) are explicitly named as
  **not** in the stripped-wrapper list, so a rule like `Bash(devbox run *)`
  would authorize `devbox run rm -rf .` — a concrete argument for why the
  project relies on the hook layer rather than trying to enumerate every
  wrapper as a permission rule.

---

## 3. `permissions.disableAutoMode` — a new key, not in the first note, directly relevant to the spec's stated gap

The permissions page, section "Permission modes", read 2026-09-02, verbatim:

> "To prevent `bypassPermissions` or `auto` mode from being used, set
> `permissions.disableBypassPermissionsMode` or `permissions.disableAutoMode`
> to `"disable"` in any settings file. These are most useful in managed
> settings where they can't be overridden."

`.claude/settings.json` sets `disableBypassPermissionsMode` but has no
`disableAutoMode` key, and neither `11-agentic-sdlc.md` nor
`.claude/README.md` mentions it. This is directly on point for the gap the
spec itself now names (see item 1.3 above): "Claude Code's own auto mode runs
classifier-approved commands without a human in the loop" is stated as the
reason this configuration is "not a sandbox... not a cage for an adversary."
`permissions.disableAutoMode: "disable"` in `.claude/settings.json` would
close exactly that gap for every session that reads the committed file
(subject to the same lock/scope semantics as `disableBypassPermissionsMode`,
which the permissions page says "works from any scope" and is "typically
placed in managed settings... but works from any scope. A user can set it in
their own settings to lock themselves out of bypass mode.") **This is a
mechanism worth adopting**, reversible (a JSON key), and understood: it does
not touch the build, only the operator's own session at the point they'd
otherwise fall into `auto` mode.

Caveat found in the same read: `useAutoModeDuringPlan` (the key that governs
whether the classifier reviews Bash during plan mode) is on the exceptions
list for managed-precedence — Claude Code honors `false` from "any managed
source, `--settings`, `~/.claude/settings.json`, or
`.claude/settings.local.json`" but explicitly **not** from
`.claude/settings.json` — "a `false` in `.claude/settings.json` is ignored"
(settings-reference, exceptions table, paraphrased by the WebFetch
summarizer — not independently verified verbatim here, flagged accordingly).
If true, this means the project **cannot** force "plan mode is genuinely
read-only for every contributor" via the committed settings file; each
operator would have to set `useAutoModeDuringPlan: false` in their own user
or local settings, or the project would need managed settings, which it does
not have. Worth a line in `11-agentic-sdlc.md` or `.claude/README.md` if the
project ever wants to make that claim rather than leaving it to each
operator's personal configuration — but not a defect in what is currently
written, since nothing currently claims plan mode is enforced project-wide.

---

## 4. The sandbox: exact config shape, for the Foundation Pass evaluation

`11-agentic-sdlc.md` "The permission boundary" (SHOULD): "Claude Code's
sandbox (`sandbox` in `settings.json`) is the documented OS-level layer: it
constrains what a Bash subprocess can write and reach regardless of what the
agent decided. It is evaluated and, if it does not break the build, enabled
by the Foundation Pass once the build's filesystem and network needs are
known." `.claude/settings.json` currently has **no `sandbox` key** — correct
per the spec, since there is no build yet to evaluate it against. What
follows is the exact shape for when that evaluation happens, verbatim from
https://code.claude.com/docs/en/sandboxing, read 2026-09-02 (page fetched in
full and grepped from the saved file, not summarized).

**Top-level toggle and platform support.** "The sandbox is built into Claude
Code and runs on macOS, Linux, and WSL2. Native Windows is not supported. On
Windows, run Claude Code inside a WSL2 distribution." "On macOS, there is
nothing to install: sandboxing uses the built-in Seatbelt framework. On Linux
and WSL2, the sandbox relies on two packages." `sandbox.enabled` is a boolean,
off by default (settings-reference summary, unverified verbatim but
consistent with "By default, if the sandbox cannot start... Claude Code shows
a warning and runs commands without sandboxing", which presupposes `enabled`
is not on by default).

**JSON shape — two independent layers, filesystem and network:**

```json
{
  "sandbox": {
    "enabled": true,
    "failIfUnavailable": false,
    "allowUnsandboxedCommands": true,
    "autoAllowBashIfSandboxed": true,
    "excludedCommands": ["some-command"],
    "filesystem": {
      "disabled": false,
      "allowWrite": ["~/.kube", "/tmp/build"],
      "denyWrite": ["some/path"],
      "denyRead": ["~/"],
      "allowRead": ["~/projects"]
    },
    "network": {
      "allowedDomains": ["github.com", "*.npmjs.org"],
      "deniedDomains": []
    },
    "credentials": {
      "files": [
        { "path": "~/.aws/credentials", "mode": "deny" },
        { "path": "~/.ssh", "mode": "deny" }
      ],
      "envVars": [
        { "name": "GITHUB_TOKEN", "mode": "deny" }
      ]
    }
  }
}
```

(Assembled from four separate verbatim example blocks on the page; the shape
is real, the specific paths in this composite are illustrative.)

**What each part constrains, verbatim:**

- "Filesystem isolation controls which paths sandboxed commands can read and
  write, and network isolation controls which domains they can reach. With
  the filesystem layer off, sandboxed commands get unrestricted read and
  write access to the host filesystem, while their network egress stays
  confined to your allowed domains."
- Default write access, no config needed: "By default, sandboxed commands can
  write to the current working directory, the session temp directory, and any
  directories you've added... with `--add-dir`, `/add-dir`, or
  `permissions.additionalDirectories`."
- `sandbox.filesystem.allowWrite`: "these paths are enforced at the OS level,
  so all commands running inside the sandbox, including their child
  processes, respect them. This is the recommended approach when a tool needs
  write access to a specific location, rather than excluding the tool from
  the sandbox entirely with `excludedCommands`."
- Path prefix syntax **differs from permission-rule syntax**: sandbox
  filesystem paths use `/` for absolute, `~/` for home-relative, `./` or no
  prefix for project-root-relative (for project settings) or `~/.claude`-
  relative (for user settings) — "This syntax differs from Read and Edit
  permission rules, which use `//path` for absolute and `/path` for
  project-relative."
- `autoAllowBashIfSandboxed` "still defaults to `true`, so sandboxed commands
  keep running without prompts. Set it to `false` to prompt for sandboxed
  commands." — confirms the value the first note flagged as a behaviour
  change to understand before enabling: turning `sandbox.enabled` on alone
  removes the per-command Bash prompt by default.
- **Merging with the existing permission rules — this is the part most
  relevant to this repository's current `Edit(//opt/local/**)` etc. deny
  rules**: "Paths and domains from both sandbox settings and permission rules
  are merged into the final sandbox configuration." And explicitly: "`Edit`
  allow rules Grant write access to specific paths, the same way
  `sandbox.filesystem.allowWrite` does" / "`Read` and `Edit` deny rules Block
  access to specific files or directories" (table, "How sandboxing relates to
  permissions and permission modes" → "Permission rules"). In other words,
  enabling `sandbox.enabled: true` with **no other sandbox config** would
  automatically pull this repository's existing `Edit(//opt/local/**)`,
  `Edit(//usr/local/**)`, `Edit(//opt/homebrew/**)`, `Edit(//Library/**)`,
  `~/.zshrc`, `~/.zprofile`, etc. deny rules into the sandbox's filesystem
  deny list — which is precisely the OS-level enforcement
  `11-agentic-sdlc.md`'s Hooks section says is currently missing ("`Read`/`Edit`
  deny rules do not apply to what a subprocess does. Two layers,
  deliberately.").
- Protected paths the sandbox denies **regardless of config**, inside a
  writable directory: "the `.claude` settings files, the `.claude/skills`,
  `.claude/agents`, `.claude/commands`, and `.claude/hooks` directories,
  `.mcp.json`... shell startup files such as `.bashrc` and `.zshrc`,
  `.gitconfig`... and `hooks` and `config` inside `.git`." "There is no way to
  exempt one of these paths: an `allowWrite` entry or an `Edit` allow rule
  that covers the path doesn't lift the protection."
- `failIfUnavailable`: "By default, if the sandbox cannot start because
  dependencies are missing or the platform is unsupported, Claude Code shows
  a warning and runs commands without sandboxing. To make this a hard failure
  instead, set `sandbox.failIfUnavailable` to `true`. This is intended for
  managed deployments that require sandboxing as a security gate." — relevant
  if the project ever wants CI to fail rather than silently degrade when a
  runner lacks the sandbox dependencies.
- What it does **not** replace: "Sandboxing, permission rules, and permission
  modes are complementary layers." "`/sandbox` is not a permission mode.
  Permission modes decide whether a tool call runs and whether you are
  prompted first, while the sandbox restricts what a Bash command can access
  once it runs." This matches `11-agentic-sdlc.md`'s existing framing
  ("Neither is a sandbox") — no change needed there, the sandbox remains an
  addition, not a replacement, for the permission-rule and hook layers.

**Recommendation for the Foundation Pass, not adopted here (this run does
not touch `.claude/settings.json`):** once `rust-toolchain.toml` and a build
exist, `sandbox.enabled: true` plus `sandbox.filesystem.allowWrite` for
whatever `cargo build`/`cargo test` need outside the working directory (the
Cargo registry/cache under `~/.cargo`, most likely) is the concrete, reversible
change the SHOULD calls for. `autoAllowBashIfSandboxed` staying at its
default `true` is worth a deliberate decision either way, since it silently
removes the per-command Bash prompt.

---

## 5. Everything else — re-verified, unchanged from the first note

No drift found on a second read for: `CLAUDE.md` project scope and load
order; `@AGENTS.md` import syntax (still exactly the documented pattern,
still at `CLAUDE.md` line 3); 200-line-per-file guidance (both files now
slightly *shorter* — 90 and 113 lines — still well under 200, still both
comply); `autoMemoryEnabled: false` at the top level of `.claude/settings.json`
(confirmed: docs describe it as a project-settable top-level key, "Turn
auto memory off or on... Scope: Any file", and the repo sets it correctly);
`env`/`hooks`/`permissions.allow`/`deny`/`ask` shapes; the plugins page's
standalone-vs-plugin guidance and the fit argument for this repository
(project-specific customizations, nothing meant to be shared or reused
across projects, matches the "Standalone" row exactly — "Personal workflows,
project-specific customizations, quick experiments"). `.claude/README.md`'s
"Official plugins worth installing" section (`security-guidance`,
`rust-analyzer-lsp`, `commit-commands`) is unaffected by anything read this
run; the plugins page's LSP-plugin guidance for Rust ("For common languages
like TypeScript, Python, and Rust, install the pre-built LSP plugins from the
official marketplace") continues to support that recommendation as written.

---

## 6. Pages fetched beyond the assigned four, and why

The assigned group is `memory`, `settings`, `best-practices`, `plugins`. As
in the first run, `settings` alone has no field-level schema — every fact
about `disableBypassPermissionsMode`, `sandbox`, `env`, `hooks` types and
scopes lives on `settings-reference`, and the exact rule-matching semantics
live on `permissions`. This run additionally fetched `sandboxing` because the
task explicitly calls out "the sandbox setting (name, shape, what it
constrains) since the Foundation Pass must evaluate enabling it" and
`settings-reference` alone gives only field names and one-line descriptions,
not the JSON shape or the merge-with-permission-rules behaviour a real
evaluation needs. None of these three extra pages are outside the spirit of
the group — they are the pages `settings` itself links to for exactly this
information.

**Degraded source for this run:** `settings-reference` was fetched twice and
both times the WebFetch tool's internal summarization step reported the page
content as truncated before reaching the full reference table with Type and
Default columns. Every fact attributed to `settings-reference` above is
explicitly flagged as a paraphrase, not a verbatim quote, for that reason. A
future run should fetch it in smaller sections (e.g. by section anchor) or
via a tool that returns raw content rather than a model-summarized response,
if exact Type/Default values for keys like `sandbox.enabled`'s default or
`permissions.defaultMode`'s allowed-value list are needed with the same
verbatim confidence as the `permissions` and `sandboxing` facts above.
