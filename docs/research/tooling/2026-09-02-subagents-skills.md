# Verification: Claude Code subagent and skill documentation vs. this repository

Read date for every documentation fact below: **2026-09-02**.

Sources fetched (all via WebFetch; `curl` is denied by project policy so HTTP
status codes could not be observed directly):

- S1 `https://code.claude.com/docs/en/sub-agents` (read 2026-09-02)
- S2 `https://code.claude.com/docs/en/skills` (read 2026-09-02)
- S3 `https://code.claude.com/docs/en/slash-commands` (read 2026-09-02) — the
  fetched body was **byte-identical** to S2 (verified with `cmp` on the two
  persisted outputs), and `https://code.claude.com/docs/llms.txt` (read
  2026-09-02) lists **no** `/docs/en/slash-commands` page. Conclusion: the
  page no longer exists as a separate document; it resolves to the skills page.
- S4 `https://code.claude.com/docs/en/commands` (read 2026-09-02) — built-in
  commands reference; says only "To add your own commands, see skills".
- S5 `https://code.claude.com/docs/llms.txt` (read 2026-09-02) — page index.

Repository files read (read-only): `.claude/agents/{docs-auditor,
provider-researcher, security-reviewer, spec-guardian}.md`,
`.claude/skills/{adr, decision-pending, pass-report, preflight, research-note,
spec-evolution}/SKILL.md`, `docs/spec/11-agentic-sdlc.md` lines 142–192
(Subagents, Skills), `docs/spec-evolution.md` SE-017. `.claude/commands/`
does not exist in the repository.

---

## 1. Subagent frontmatter (S1, read 2026-09-02)

### Documented fields

| Field | Required | Documented meaning |
|---|---|---|
| `name` | Yes | lowercase + hyphens; hooks receive it as `agent_type`; filename need not match; `:` reserved for plugin scope |
| `description` | Yes | "When Claude should delegate to this subagent" |
| `tools` | No | allowlist; "Inherits every tool available to subagents if omitted" |
| `disallowedTools` | No | denylist, "removed from inherited or specified list"; if both set, `disallowedTools` applied first, then `tools` resolved against the remainder; a tool in both is removed |
| `model` | No | `sonnet`, `opus`, `haiku`, `fable`, a full model ID, or `inherit` |
| `permissionMode` | No | `default`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`, `plan`, or `manual` (alias of `default`) |
| `maxTurns` | No | turn cap; output returned "marked as partial" at the limit; resumable |
| `skills` | No | skills preloaded (full content injected) at startup |
| `mcpServers` | No | MCP servers available to this subagent |
| `hooks` | No | lifecycle hooks scoped to the subagent |
| `memory` | No | `user` (`~/.claude/agent-memory/<name>/`), `project` (`.claude/agent-memory/<name>/`, shareable via VCS), `local` (`.claude/agent-memory-local/<name>/`, not checked in) |
| `background` | No | `true` keeps the subagent in the background |
| `effort` | No | `low`, `medium`, `high`, `xhigh`, `max`; default inherits session |
| `isolation` | No | `worktree` runs in a temporary git worktree |
| `color` | No | exactly one of `red`, `blue`, `green`, `yellow`, `purple`, `orange`, `pink`, `cyan` |
| `initialPrompt` | No | first user turn when run as main agent via `--agent` |
| `experimental` | No | map; `cacheTtl: 5m|1h` |

Also documented: `tools`/`disallowedTools` accept `mcp__<server>` and
`mcp__<server>__*` patterns; `disallowedTools: mcp__*` removes every MCP tool;
`Agent(agent_type)` syntax restricts spawnable subagent types.

### Fields the four repository agents use, checked

All four agents use exactly: `name`, `description`, `tools`, `model: inherit`,
`color`; the three reviewers add `disallowedTools: Write, Edit, NotebookEdit`.

| Agent | `tools` | `disallowedTools` | `model` | `color` | Verdict |
|---|---|---|---|---|---|
| docs-auditor | Read, Grep, Glob, Bash | Write, Edit, NotebookEdit | inherit | green | all documented, all values valid |
| provider-researcher | Read, Grep, Glob, Bash, WebFetch, WebSearch, Write, Edit, TodoWrite | (none) | inherit | cyan | all documented, all values valid |
| security-reviewer | Read, Grep, Glob, Bash | Write, Edit, NotebookEdit | inherit | red | all documented, all values valid |
| spec-guardian | Read, Grep, Glob, Bash | Write, Edit, NotebookEdit | inherit | orange | all documented, all values valid |

Specific confirmations (S1, read 2026-09-02):

- `model: inherit` is valid: "**inherit**: use the same model as the main
  conversation".
- `disallowedTools` is still a documented field.
- `color` values are constrained to the eight listed above; `green`, `cyan`,
  `red`, `orange` are all in the list.
- Comma-separated `tools:` matches the documented example
  (`tools: Read, Grep, Glob, Bash`).

Observations on the repository's combination:

- The reviewers' `disallowedTools: Write, Edit, NotebookEdit` is **redundant
  but harmless** given `tools: Read, Grep, Glob, Bash`: the docs say `tools`
  is an allowlist and, when both are set, the denylist is applied first and
  the allowlist resolved against the remainder. Keeping both is a legitimate
  belt-and-braces choice and `docs/spec/11-agentic-sdlc.md` "Subagents" MUST
  explicitly requires `disallowedTools` on the reviewers, so no change is
  forced. Note the allowlist form is the stricter one: a bare
  `disallowedTools`-only agent "keeps Bash, MCP tools, and the rest of its
  pool".
- None of the four agents lists `Skill`, so none can invoke a project skill
  (see §2). The reviewers' bodies do not ask them to, so this is consistent.
  `provider-researcher` writes research notes itself rather than calling
  `/research-note`; that is consistent with its tool list.
- `Write, Edit` stay reachable through `Bash` for all four agents (heredocs,
  `sed -i`). The reviewers are "read-only" only by convention plus the
  project's hooks; the docs offer `permissionMode: plan` ("Plan mode
  (read-only exploration)") as the harness-level way to say so.

### Documented fields that would help these agents

- `permissionMode: plan` — read-only exploration enforced by the permission
  system rather than by tool omission. Fits the three reviewers exactly.
- `maxTurns` — bounds a runaway review; output is returned "marked as
  partial" and can be resumed.
- `memory: project` — cross-session learning stored in
  `.claude/agent-memory/<name>/` and "shareable via version control". Useful
  for `docs-auditor` (previously-flagged links/claims) and `spec-guardian`
  (recurring deviation patterns). Must be weighed against INV-14/15-style
  determinism expectations for reviewers; it does not affect shipped code.
- `skills:` — preload e.g. `spec-evolution` into `spec-guardian` so it knows
  the entry format it is checking against (note: preloading is blocked for
  skills with `disable-model-invocation: true`).
- `background: true` — for `provider-researcher`, which the spec already
  says should run "in parallel without polluting the main context".
- `effort` — per-agent effort override (`low`…`max`).
- `hooks` — per-agent lifecycle hooks, e.g. a `PreToolUse` guard scoped to
  the researcher only.

## 2. Invocation, parallelism, skills, isolation, model choice (S1, read 2026-09-02)

- **Invocation**: automatic delegation from the `description`; `@`-mention
  from the typeahead (guaranteed); naming it in natural language (Claude
  decides); `--agent <name>` makes it the main-session agent. Programmatic
  invocation is the `Agent` tool.
- **Parallel / background**: "Subagents can run in the foreground or the
  background". Foreground blocks the main conversation; background runs
  concurrently and permission prompts are surfaced in the main session
  naming the subagent (since v2.1.186; before that, background tool calls
  needing permission were auto-denied). Default concurrent limit is 20
  (`CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS`). **Background tool filter**: a
  background subagent keeps every MCP tool "but only these built-in tools:
  Read, Grep, Glob, Bash, PowerShell, Edit, Write, NotebookEdit, WebFetch,
  WebSearch, TodoWrite, Skill, ToolSearch, EnterWorktree, ExitWorktree,
  Monitor, TaskStop, SendMessage, and Artifact". Everything the four repo
  agents list is inside that set, so they behave identically in either mode.
- **Nesting**: "By default, a subagent can spawn subagents of its own, up to
  three layers below the main conversation" (`CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH`).
- **Skills**: "without it [the `skills` field], the subagent can still
  discover and invoke project, user, and plugin skills through the Skill tool
  during execution. To prevent a subagent from invoking skills entirely, omit
  `Skill` from the `tools` list or add it to `disallowedTools`." All four repo
  agents set a `tools` allowlist without `Skill`, so they **cannot** invoke
  skills. Preloading via `skills:` injects the full content at startup.
- **Context isolation**: "Each subagent runs in its own context window with a
  custom system prompt, specific tool access, and independent permissions."
  A non-fork subagent's initial context is: its own system prompt (not the
  Claude Code system prompt) plus environment details; the delegation task
  message; **every level of the CLAUDE.md hierarchy** the main conversation
  loads; and a git-status snapshot. "Explore and Plan are the only subagents
  that omit CLAUDE.md and git status." So the four custom agents *do* receive
  CLAUDE.md (and therefore the `@AGENTS.md` import) at startup.
- **Resume**: an existing subagent can be continued via `SendMessage` with
  its ID/name instead of starting over.
- **Model selection guidance, quoted**: "The `model` field controls which
  model the subagent uses: Model alias: use one of the available aliases:
  `sonnet`, `opus`, `haiku`, or `fable`; Full model ID: use a full model ID
  such as `claude-opus-5` or `claude-sonnet-5`. Accepts the same values as
  the `--model` flag; inherit: use the same model as the main conversation."
  Resolution order: per-invocation `model` parameter → frontmatter `model`
  (where `inherit` selects the main model) → `CLAUDE_CODE_SUBAGENT_MODEL` env
  var → main conversation's model. The only "lighter model" guidance found:
  "Set `CLAUDE_CODE_SUBAGENT_MODEL` environment variable to run subagents on
  a faster, cheaper model like Haiku." (S1, read 2026-09-02). Note the env
  var "doesn't change the model the built-in Explore and Plan subagents run
  on".
- **SE-017 fit**: SE-017 replaced model-name choreography with role-based
  passes ("The session prompt for a given pass names whichever agent is
  running it"). The docs offer only aliases, full IDs, or `inherit`; there is
  no role-based value. `model: inherit` is therefore the only frontmatter
  value compatible with SE-017, and it is what all four agents use. If a
  cheaper model is ever wanted for e.g. `docs-auditor`, the SE-017-compatible
  mechanism is the `CLAUDE_CODE_SUBAGENT_MODEL` environment variable set by
  the human at session start, not a name in the file. (Aliases like `haiku`
  are still names; they would violate SE-017's intent even though the docs
  permit them.)

## 3. Skill frontmatter (S2, read 2026-09-02)

### Documented fields

"All fields are optional. Only `description` is recommended." Frontmatter is
read only when the opening `---` is the file's first line. Booleans accept
`yes/no/on/off/1/0/true/false` (since v2.1.218).

| Field | Meaning |
|---|---|
| `name` | display name; **the command name comes from the directory name** for personal/project skills, from `name` only in plugin skills |
| `description` | what + when; combined `description` + `when_to_use` truncated at 1,536 chars in the listing |
| `when_to_use` | extra trigger phrases, appended to description in the listing |
| `argument-hint` | autocomplete hint, e.g. `[issue-number]` |
| `arguments` | named positional args for `$name` substitution (space-separated string or YAML list) |
| `disable-model-invocation` | `true` = only the user can invoke; also blocks preloading into subagents and scheduled-task use; default `false` |
| `user-invocable` | `false` = only Claude can invoke; hidden from `/` menu; default `true` |
| `allowed-tools` | tools pre-approved **for the turn that invokes the skill**; grant clears on the next user message; does *not* restrict the tool pool; space- or comma-separated or YAML list; `Bash(pattern)` rules allowed; `${CLAUDE_SKILL_DIR}` / `${CLAUDE_PROJECT_DIR}` substituted inside Bash rules |
| `disallowed-tools` | tools removed from the pool while the skill is active; clears on next message |
| `model` | same values as `/model`, or `inherit`; applies for the rest of the turn |
| `effort` | `low`…`max` |
| `context` | `fork` runs the skill in a forked subagent (skill body becomes the prompt; no conversation history) |
| `agent` | subagent type for `context: fork` (`Explore`, `Plan`, `general-purpose`, or any `.claude/agents/` name; default `general-purpose`) |
| `background` | only with `context: fork`; default `true` (runs in background, since v2.1.218); `false` waits |
| `hooks` | hooks registered on invocation, kept for the session |
| `paths` | glob patterns limiting automatic activation |
| `shell` | `bash` (default) or `powershell` for `` !`cmd` `` blocks |
| `metadata` | free-form map, ignored by Claude Code |
| `license` | Agent Skills spec field; "Claude Code accepts the field but doesn't act on it" |
| `compatibility` | Agent Skills spec field, string ≤ 500 chars; accepted, not acted on |

Portability note (S2): outside Claude Code (claude.ai uploads, Skills API,
`package_skill.py`) only `name`, `description`, `license`, `compatibility`,
`metadata`, `allowed-tools` are allowed; any other key (e.g. `argument-hint`)
fails with a hard error. Irrelevant for this repo unless skills are ever
uploaded.

### Size / format guidance (S2, read 2026-09-02)

- No numeric SKILL.md line limit is documented. The guidance is qualitative:
  "Keep the body itself concise. Once a skill loads, its content stays in
  context across turns, so every line is a recurring token cost. State what
  to do rather than narrating how or why, and apply the same conciseness test
  you would for CLAUDE.md content."
- Lifecycle: invoked skill content "enters the conversation as a single
  message and stays there across later turns"; not re-read on later turns.
  After auto-compaction, "the first 5,000 tokens of each" invoked skill are
  re-attached within a combined 25,000-token budget.
- Progressive disclosure via supporting files: "Skills can include multiple
  files in their directory. This keeps `SKILL.md` focused on the essentials
  while letting Claude access detailed reference material only when needed"
  (`reference.md`, `examples.md`, `scripts/`). Reference them from SKILL.md
  so Claude knows when to load each.
- Listing budget: names+descriptions listing capped at 1% of the context
  window (`skillListingBudgetFraction`); per-entry cap 1,536 chars
  (`skillListingMaxDescChars`). "Put the key use case first".
- Validation: `claude plugin validate .claude/skills` (v2.1.233+) finds
  SKILL.md files whose frontmatter does not parse.

### The six repository skills, checked

All six use `name`, `description`, `allowed-tools`, `license: Apache-2.0`;
five use `argument-hint`; `preflight` uses `disable-model-invocation: false`
instead of `argument-hint`. Every field used is documented and every value is
syntactically valid (comma-separated `allowed-tools` with `Bash(...)` patterns
and `${CLAUDE_PROJECT_DIR}` substitution are all documented forms). Each skill
directory contains only `SKILL.md` (no supporting files).

| Skill | lines | fields used | valid | notes |
|---|---|---|---|---|
| adr | 55 | name, description, argument-hint, allowed-tools (Read, Glob, Write, Edit), license | yes | side-effecting (writes docs/adr/) but Claude may auto-invoke |
| decision-pending | 50 | name, description, argument-hint, allowed-tools (Read, Edit), license | yes | side-effecting; Claude may auto-invoke |
| pass-report | 61 | name, description, argument-hint, allowed-tools (Read, Grep, Glob, Bash, Write, Edit), license | yes | side-effecting; **unrestricted `Bash` pre-approved for the turn** |
| preflight | 68 | name, description, disable-model-invocation: false, allowed-tools (Read, Grep, Glob, 14× `Bash(pattern)`), license | yes | `disable-model-invocation: false` is the documented default, i.e. a no-op; no `argument-hint` (takes no args, fine) |
| research-note | 47 | name, description, argument-hint, allowed-tools (Read, Grep, Glob, Write, Edit), license | yes | side-effecting; Claude may auto-invoke |
| spec-evolution | 57 | name, description, argument-hint, allowed-tools (Read, Grep, Glob, Edit), license | yes | side-effecting; Claude may auto-invoke |

Findings:

1. **No skill body contains `$ARGUMENTS`, `$N` or `$ARGUMENTS[N]`** (grep
   confirmed), although five declare an `argument-hint`. Per S2 this still
   works: "If you invoke a skill with arguments but no placeholder in the
   skill's content receives one, Claude Code appends `ARGUMENTS: <your input>`
   to the end of the skill content so Claude still sees what you typed." So
   `/adr Choose Rust` works, but the argument lands at the end as a bare
   `ARGUMENTS:` line rather than where the instructions would use it. Adding
   an explicit `$ARGUMENTS` (or `arguments: [title]` + `$title`) placeholder
   is the documented, clearer form.
2. **`disable-model-invocation` is absent from the five side-effecting
   skills.** `docs/spec/11-agentic-sdlc.md` (Skills, SHOULD): "A skill with
   side effects sets `disable-model-invocation: true` so it is triggered
   deliberately rather than inferred." The docs describe the same field for
   "workflows with side effects or that you want to control timing". `adr`,
   `decision-pending`, `pass-report`, `research-note`, `spec-evolution` all
   write repository files and none sets it. This is drift between the spec's
   own SHOULD and the skills — either set the field or record the deliberate
   exception in `docs/spec-evolution.md`. Trade-off to note: setting it also
   removes the description from Claude's context and blocks preloading into
   subagents (`skills:` field), so a `spec-guardian` that preloads
   `spec-evolution` would need that skill to stay model-invocable.
3. `preflight`'s `disable-model-invocation: false` is the default value; it
   is valid but carries no information. If the intent is "Claude may run
   preflight on its own", that is already the default.
4. `pass-report` pre-approves bare `Bash` for the invoking turn. The docs
   warn: "A skill can grant itself broad tool access, so review the
   `allowed-tools` of skills checked into a repository before you run Claude
   Code there." Project deny rules still apply (allowed-tools does not
   override deny), but a scoped set like `preflight`'s is the more honest
   form.
5. `license: Apache-2.0` is accepted and ignored. Fine; consistent with the
   Agent Skills spec. (Whether Apache-2.0 is the project licence is a
   DP-question, not a docs one.)
6. `name` matches the directory name in all six, so the command names are
   unaffected by the field (command name comes from the directory).
7. `when_to_use` (new field) is available if descriptions need trigger
   phrases without lengthening `description`.
8. `allowed-tools` semantics changed vs. older mental models: it is a
   **per-turn pre-approval that clears on the next user message**, not a
   restriction. The repo's skills use it correctly as pre-approval.

### `$ARGUMENTS` substitution (S2, read 2026-09-02)

`$ARGUMENTS` = full argument string as typed; `$ARGUMENTS[N]` / `$N` =
0-based positional (shell-style quoting; missing index stays literal);
`$name` via `arguments:` frontmatter (missing → empty string);
`${CLAUDE_SESSION_ID}`, `${CLAUDE_EFFORT}`, `${CLAUDE_SKILL_DIR}`,
`${CLAUDE_PROJECT_DIR}` (v2.1.196+), `${CLAUDE_PLUGIN_ROOT}`,
`${CLAUDE_PLUGIN_DATA}` (plugin skills only). Escape a literal with `\$1`.
Stacking `/a /b 123` loads both and passes `123` to each (v2.1.199+).
`` !`command` `` runs shell before the content is sent (dynamic context).

## 4. Slash commands vs skills (S2, S4, S5, read 2026-09-02)

- Skills in `.claude/skills/<dir>/SKILL.md` **appear as `/<dir>`**: "invoke
  one directly with `/skill-name`"; command name = directory name.
- `.claude/commands/` **still works but is not a separate mechanism**:
  "Custom commands have been merged into skills. A file at
  `.claude/commands/deploy.md` and a skill at `.claude/skills/deploy/SKILL.md`
  both create `/deploy` and work the same way. Your existing
  `.claude/commands/` files keep working." Precedence: "if a skill and a
  command share the same name, the skill takes precedence."
- The standalone `/docs/en/slash-commands` page is gone (S3 resolves to S2;
  S5 does not list it). Built-in commands live at `/docs/en/commands` (S4).
- This repository has no `.claude/commands/`, which matches the current
  recommendation.

## 5. Drift against `docs/spec/11-agentic-sdlc.md` (lines 142–192)

- Subagents MUST "hold `disallowedTools` covering `Write`, `Edit` and
  `NotebookEdit`" — satisfied, and still a documented field. The spec could
  note that the `tools` allowlist already excludes them and that
  `permissionMode: plan` is the harness-level read-only switch.
- Spec rationale "independent items run in parallel without polluting the
  main context" — supported by the docs (background subagents, 20-concurrent
  default, isolated context). Not expressed in frontmatter; could be
  `background: true` on `provider-researcher`.
- Skills SHOULD set `disable-model-invocation: true` when side-effecting —
  **not satisfied** by five of six skills (see §3 finding 2).
- The spec's statement that skills are "procedures … performed identically
  every time" is compatible with the documented lifecycle, with one caveat
  worth recording: invoked skill content persists across turns and is
  truncated to 5,000 tokens each after compaction, so long procedures should
  be re-invoked after compaction (docs: "re-invoke it after compaction to
  restore the full content").

## 6. Recommended concrete changes (for the main agent to decide)

Subagents (`.claude/agents/`):

1. Reviewers (`spec-guardian`, `security-reviewer`, `docs-auditor`): add
   `permissionMode: plan` to make read-only a harness property, not a
   convention. Keep `tools` and `disallowedTools` as they are (spec MUST).
2. Optionally add `maxTurns` to the three reviewers (a review is bounded
   work; partial output is returned and resumable).
3. `provider-researcher`: consider `background: true` to encode the spec's
   parallel-research intent; consider `maxTurns`.
4. Keep `model: inherit` on all four (only SE-017-compatible value). If a
   cheaper reviewer model is wanted, use `CLAUDE_CODE_SUBAGENT_MODEL` at
   session level, never a name in the file.
5. Do **not** add `Skill` to reviewer tool lists (they must not act, only
   report). If `spec-guardian` should know the spec-evolution entry format,
   use `skills: [spec-evolution]` — which requires that skill to remain
   model-invocable (see next section).

Skills (`.claude/skills/`):

6. Add `$ARGUMENTS` (or `arguments:` + named placeholder) to the bodies of
   `adr`, `decision-pending`, `pass-report`, `research-note`,
   `spec-evolution` at the point where the title/id is used.
7. Decide per skill on `disable-model-invocation: true` to honour the spec's
   SHOULD: strongest case for `pass-report` and `adr`; `spec-evolution` and
   `research-note` may deliberately stay auto-invocable because the project
   *wants* the agent to reach for them, in which case record the exception
   in `docs/spec-evolution.md` rather than leave the SHOULD silently unmet.
8. Remove the no-op `disable-model-invocation: false` from `preflight`, or
   keep it with a comment-free intent (it is harmless).
9. Narrow `pass-report`'s bare `Bash` pre-approval to the patterns it needs
   (git status/diff/log, cargo test), mirroring `preflight`.
10. No `.claude/commands/` migration needed; none exists.
