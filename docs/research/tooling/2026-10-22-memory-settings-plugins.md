<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification — memory, settings, best practices, plugins (2026-10-22)

Group: `memory`, `settings`, `best-practices`, `plugins`, read on 2026-10-22 from
`https://code.claude.com/docs/en/<page>.md` (all four fetched, HTTP 200).
`/settings` is now only "Settings files and precedence"; the keys moved to
`/settings-reference.md`, `/permissions.md` and `/hooks.md`, also fetched.
Not fetched: `sandboxing.md`, `anthropic-marketplaces.md`, `hooks-guide.md`,
`env-vars.md`. Returned by a read-only subagent; the runner wrote this file.
Verdict: **drift, 5 items, none a contract break.**

## Still documented and valid

`$schema`; `permissions.disableBypassPermissionsMode: "disable"` (also makes
subagents ignore `permissionMode: bypassPermissions`); `autoMemoryEnabled`; the
`env` block (none of the repo's variables is in the ignored list); deny → ask →
allow order, first match wins; the `//path` and `~/` forms; hook events,
matchers, `timeout` and `statusMessage`; the Stop hook's eight-block cap and
`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`; `disableAllHooks`; `sandbox.enabled`,
`sandbox.filesystem.allowWrite`, `sandbox.network.allowedDomains`; the
`@AGENTS.md` import (native AGENTS.md reading needs v2.1.277+, and only
`CLAUDE.md` is read when both exist); `.claude/rules/*.md` with `paths:` only.

## Drift

1. `settings.json:97-98` — the two `Edit(./…)` rules for the toolchain pin and
   `docs/spec/01-invariants.md`: `./` anchors at the current directory, not the
   project root. Documented fix: the `/path` form (`Edit(/docs/spec/01-invariants.md)`).
   The hook backs the rule up.
2. `.claude/README.md:73-74` says `env` waits for trust; the permissions page
   lists hooks and `env` as "used" in the two untrusted cases it covers. Only
   `permissions.allow` waits.
3. `CLAUDE.md:57-58` "a shell or MCP reader loads nothing": a single-file read
   by a shell command that counts as a read (`cat`, `head`) also loads
   path-scoped rules. MCP readers still load nothing.
4. `docs/spec/11-agentic-sdlc.md:107` "a deny rule on a path does not reach a
   subprocess": Read/Edit deny rules now also cover recognised Bash file
   commands (`cat`, `head`, `tail`, `sed`, `tee`) and `>` / `<`; still not
   arbitrary subprocesses.
5. `.claude/README.md:82-85`: Claude Code adds the official marketplace on the
   first interactive session unless a managed policy blocks it.

## Unverifiable from these pages

PostToolUse stderr on exit 0 (spec 11:191); the multi-repository cloud-session
plugin rule (spec 11:141-142); `CLAUDE_CODE_DISABLE_AUTO_MEMORY` also disabling a
subagent's `memory:` (spec 11:83); the plugin repository and names in
`.claude/README.md:85-100`; the version claims at `.claude/README.md:50-52`.

## Mechanisms not adopted (recommendations)

Hook `if` field on the commit gate (`Bash(git commit *)`); a Stop-hook handler
checking for review evidence (weigh against the eight-block cap);
`permissions.disableAutoMode`; `defaultMode: "plan"` (spec 11:158 leaves the mode
to the operator, deliberately); tightening `git branch*` (allows `-D`) and
`cargo run *`.

## Guidance the memory files do not follow

`AGENTS.md` is 199 lines against a target under 200, and holds derivable
content (crate and path tables, domain concepts) the docs say to move out;
`AGENTS.md:6-10` and `CLAUDE.md:11-16` give the reading order in different
sequences (charter first vs invariants first); the skills and subagent lists
are repeated across three files.
