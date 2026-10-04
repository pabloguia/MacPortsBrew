---
name: tooling-recheck
description: Re-verify the Claude Code facts this repository's agent configuration depends on against the current official documentation, in parallel subagents, and record any drift. Use at the start of every pass, before changing anything under .claude/ or .github/workflows/, and whenever a hook, permission rule, subagent or skill behaves unexpectedly.
argument-hint: "[optional: pages to limit to, e.g. hooks permissions]"
allowed-tools: Read, Grep, Glob, Agent, Edit(docs/research/tooling/**), Write(docs/research/tooling/**)
license: Apache-2.0
---

# Re-verify the tooling facts

Scope: $ARGUMENTS (empty means every page below)

`docs/spec/11-agentic-sdlc.md` ("Verified facts about the tooling") treats
Claude Code's field names, hook contracts and rule syntax as dated third-party
data, the way INV-20 treats provider facts. This is the procedure that keeps
that date current. It found four real defects the first time it ran on
configuration written the same day — do not skip it because the configuration
"has not changed"; the documentation is the thing that changes.

## 1. Delegate, in parallel

One subagent per group, all launched in one message, each told to return its
note as text and write nothing — the Agent call does not enforce that unless the
subagent type's `tools` list excludes `Write`, `Edit` and `NotebookEdit`, so the
runner checks `git status` afterwards. Each fetches
the current page at `https://code.claude.com/docs/en/<page>` (trying the
`.md` variant or locating the moved page when a URL fails, and saying so),
reads the repository files named, and reports drift with the page URL and
today's date on every fact. Never fill a gap from memory; an unfetchable page
is reported as unfetched.

| Group | Pages | Repository files to compare |
|---|---|---|
| Memory and settings | `memory`, `settings`, `best-practices`, `plugins` | `CLAUDE.md`, `AGENTS.md`, `.claude/settings.json`, `.claude/README.md`, `docs/spec/11-agentic-sdlc.md` |
| Subagents and skills | `sub-agents`, `skills` | `.claude/agents/*.md`, `.claude/skills/*/SKILL.md` |
| Hooks, permissions, CI | `hooks`, `permissions`, `headless`, `github-actions` | `.claude/hooks/*.sh`, `.claude/settings.json`, `.github/workflows/*.yml` |

Ask each for: every documented field, value or contract the repository uses
(is it still documented, is the value valid, does the script read and emit
what the contract says); every documented mechanism the repository does not
use but that would enforce something it currently only states; and any
guidance the memory files do not follow. Findings by file and line, with the
documented fix.

## 2. Read the reports and decide

- **A contract mismatch** (a field the docs no longer have, an output shape a
  hook emits that the docs no longer read, a rule that cannot match what it
  names) is a defect. Fix it, add a case to `.claude/hooks/test-hooks.sh`
  where a hook is involved, and run that script.
- **A new mechanism that enforces an existing rule** is adopted when it is
  reversible and its effect is understood; otherwise it goes into the pass
  report as a recommendation with the reason it was not adopted.
- **Guidance the project deliberately does not follow** is recorded as such,
  once, so the next run does not re-raise it.

## 3. Record

- Save each subagent's note as `docs/research/tooling/<YYYY-MM-DD>-<group>.md`.
  The table above is the one list of pages; add a page here, nowhere else.
- Update the read date at the top of `docs/spec/11-agentic-sdlc.md`
  ("Verified facts about the tooling") and in `.claude/README.md`.
- Material drift — anything that changed what the configuration enforces or
  what the specification requires — goes through `/spec-evolution`. Under the
  consolidation convention (SE-032), extend the existing tooling entry for the
  topic rather than adding a chronological one.
- The pass report names what was re-verified, what drifted, and what was left
  as a recommendation.
