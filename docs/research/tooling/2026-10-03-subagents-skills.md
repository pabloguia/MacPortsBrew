# Tooling recheck: subagents and skills, 2026-10-03

Read on: 2026-10-03

Sources, both fetched on the first try as the `.md` variant, no redirect, none
unfetched:

- <https://code.claude.com/docs/en/sub-agents.md>, read 2026-10-03
- <https://code.claude.com/docs/en/skills.md>, read 2026-10-03

Previous note: [`2026-10-02-subagents-skills.md`](2026-10-02-subagents-skills.md).

Files checked: `.claude/agents/{docs-auditor,provider-researcher,security-reviewer,spec-guardian}.md`
and `.claude/skills/*/SKILL.md` (13 skills). The `.claude/settings.json` allow
list was read only to judge whether a skill's grant is redundant.

**Method caveat.** `curl` is denied by the repository's permission rules, so
the pages were read through an indexed fetch (searched excerpts, close to
verbatim) and `WebFetch` (a model-extracted quotation). Field tables and the
`allowed-tools` and `permissionMode` passages were read in both and agreed.
The frontmatter was read directly from disk.

## Drift

**Repository drift since 2026-10-02: none.** No commit touched
`.claude/agents/` or `.claude/skills/`. Commit `5c40b11` (SE-093) made those
two directories editable by the path guard; it did not change their content.

**Documentation since 2026-10-02: additions only, none breaking.** "Not
recorded on 2026-10-02" means the previous note does not mention it; it does
not prove the page changed on that date.

Sub-agents page:

- `permissionMode` lists `auto` as a value (not recorded on 2026-10-02).
- A subagent that declares `bypassPermissions` keeps the main conversation's
  mode instead (v2.1.267+).
- Under auto mode, the classifier evaluates the subagent's tool calls and also
  reviews its work and final report before delivery.
- `model` accepts `sonnet`, `opus`, `haiku`, `fable`, a full model ID, or
  `inherit`.
- `tools`: a list that resolves to no tool makes the subagent fail to launch
  (v2.1.208+).
- `disallowedTools`: an entry with a specifier, such as `Bash(git push *)`,
  still removes the whole tool.
- A background subagent keeps only an enumerated set of built-in tools; other
  built-ins are removed silently.
- `name` containing `:` is not loaded (v2.1.218+).
- `color` and `experimental` are ignored in `--agents` JSON; this does not
  apply to files.

Skills page:

- New fields `metadata` and `compatibility` (Agent Skills spec), and
  `user-invocable` (not recorded on 2026-10-02).
- `allowed-tools` accepts a space- or comma-separated string, or a YAML list.
- **Workspace trust does not gate `allowed-tools`**: a project skill's grant
  applies even in a `-p` run in a folder never trusted. The page advises
  reviewing the `allowed-tools` of skills checked into a repository (F3).
- `allowManagedPermissionRulesOnly` makes Claude Code ignore project skills'
  `allowed-tools` (v2.1.282+).
- `${CLAUDE_PROJECT_DIR}` substitution in `allowed-tools` requires v2.1.196+.
- An unrecognised frontmatter field is ignored without error.
- Packaging a skill for claude.ai or the Skills API rejects any field outside
  `name, description, license, compatibility, metadata, allowed-tools`; the
  page's example names `argument-hint`. Relevant only if these skills are ever
  uploaded there.

## 1. Fields the repository uses, re-confirmed

### Subagents (sub-agents.md, read 2026-10-03)

| Field | Used in | Value | Documented now? | Valid? |
|---|---|---|---|---|
| `name` | all 4, line 2 | the four agent names | Yes, required; no `:` | Yes |
| `description` | all 4, line 3 | prose, 183 to 472 characters | Yes, required | Yes; all four follow the page's "use proactively" advice |
| `tools` | all 4, line 4 | `Read, Grep, Glob, Bash`; `provider-researcher` adds `WebFetch, WebSearch, Write, Edit, TodoWrite` | Yes | Yes; every name is in the background-retained list |
| `disallowedTools` | the three reviewers, line 5 | `Write, Edit, NotebookEdit` | Yes; applied before `tools` | Yes; redundant with `tools` (F4) |
| `model` | all 4 | `inherit` | Yes | Yes |
| `permissionMode` | the three reviewers, line 7 | `plan` | Yes | Valid; ignored under a `bypassPermissions`, `acceptEdits` or auto main session (unchanged; `docs/spec/11-agentic-sdlc.md` already says so) |
| `color` | all 4 | `green`, `cyan`, `red`, `orange` | Yes | Yes |

### Skills (skills.md, read 2026-10-03)

| Field | Used in | Documented now? | Valid? |
|---|---|---|---|
| `name` | all 13, equal to the directory name | Yes | Yes |
| `description` | all 13 | Yes; `description` plus `when_to_use` truncated at 1,536 characters | Yes; all well under the cap |
| `argument-hint` | 12 of 13 (not `preflight`) | Yes | Yes |
| `allowed-tools` | all 13 | Yes | Yes. The legacy `:*` suffix and the `WebFetch(domain:...)` and `Edit/Write(<glob>)` forms are not shown on this page; they belong to `/permissions` (hooks group) |
| `license` | all 13 | Yes; accepted, not acted on | Yes |

Every `SKILL.md` is under the page's 500-line guidance (49 to 88 lines).

### `allowed-tools` semantics, verified

The page states that `allowed-tools` grants permission for the listed tools
during the turn that invokes the skill, cleared at the next user message, and
that it does not restrict which tools are available: every tool remains
callable. The restricting field is `disallowed-tools`. The 2026-10-02 finding
stands.

**Wording check, deferred on 2026-10-02 and done now.** No `SKILL.md`, agent
definition, `CONTRIBUTING.md`, `AGENTS.md`, `CLAUDE.md`, `.claude/README.md` or
`docs/spec/11-agentic-sdlc.md` claims that `allowed-tools` restricts.
`capture-fixture/SKILL.md`'s "allow-list is read-only" refers to
`scripts/capture-fixture.sh`, not the frontmatter. The 2026-10-02 open item is
closed with no edit needed.

## 2. Documented mechanisms not used that would enforce what is only stated

- **A read-only agent type for `tooling-recheck`'s subagents.** The skill calls
  them read-only but names no agent type; a project agent with
  `tools: Read, Grep, Glob, WebFetch` would enforce it (F1).
- **Agent frontmatter `hooks` (`PreToolUse` on `Edit|Write`) for
  `provider-researcher`**, to enforce "writes only its research note".
  Optional, as on 2026-10-02.
- **Skill `disallowed-tools`.** No skill claims a restriction that needs it.
- **`disable-model-invocation`, `context: fork`.** Rejected on 2026-09-27; do
  not re-raise.
- **`maxTurns`, `skills`, `effort`, `isolation`, `memory`, `omitClaudeMd`,
  `background`.** Unused; nothing stated depends on them.

## 3. Findings

**F1. `tooling-recheck` states its subagents are read-only; nothing enforces
it.** `.claude/skills/tooling-recheck/SKILL.md`, section 1. The subagent that
produced this note had `Write` and `Edit` available. Documented fix: launch the
groups through an agent whose `tools` list excludes `Write`, `Edit` and
`NotebookEdit` (sub-agents.md, "Supported frontmatter fields"), or reword the
line so it states a discipline, not a property.

**F2. `preflight` pre-approves commands its procedure never runs, including one
that writes.** `.claude/skills/preflight/SKILL.md` grants `Bash(cargo fmt*)`,
which without `--check` rewrites files, plus `cargo clippy*`, `cargo test*` and
`cargo tree*`. The body runs only `stop-gate.sh` and `test-hooks.sh`. All four
grants duplicate `.claude/settings.json` allow rules, so the practical effect is
nil. The page advises granting what the skill needs. Fix: drop the four cargo
grants.

**F3. Checked-in skills' grants apply in untrusted `-p` runs.** All 13
`SKILL.md`. Seven grant `Bash(cargo test:*)`. *Reasoning, not from the page:*
that compiles and runs code from the checkout, so once the repository is public
(DP-007) a fork can pre-approve execution in a session that runs its skills.
The documented mitigation is user-side (`allowManagedPermissionRulesOnly`).
Recommendation: record that this is accepted with the public-repository
decision; no change to the skills.

**F4. `disallowedTools` on the reviewers is redundant with `tools`.** Harmless;
no fix. Repeats 2026-10-02 F3.

**Outside this group's files.** `.claude/README.md` lists 7 skills; 13 exist.
The six recipe skills are missing from its table.
