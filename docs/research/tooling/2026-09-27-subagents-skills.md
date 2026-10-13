# Tooling recheck — subagents and skills — 2026-09-27

Scope: `.claude/agents/*.md` and `.claude/skills/*/SKILL.md` against:
- https://code.claude.com/docs/en/sub-agents.md (fetched directly as `.md`, no redirect needed) — read 2026-09-27
- https://code.claude.com/docs/en/skills.md (fetched directly as `.md`, no redirect needed) — read 2026-09-27

Both pages fetched successfully on the first try; no page reported as unfetched.

Files inventoried:
- `.claude/agents/docs-auditor.md`, `.claude/agents/provider-researcher.md`,
  `.claude/agents/security-reviewer.md`, `.claude/agents/spec-guardian.md`
- `.claude/skills/{add-doctor-check,add-locale,add-mapping,add-policy-rule,
  add-provider,adr,capture-fixture,decision-pending,pass-report,preflight,
  research-note,spec-evolution,tooling-recheck}/SKILL.md`

---

## 1. Every documented field/value/contract the repo uses

### 1a. Subagents (`.claude/agents/*.md`) — per https://code.claude.com/docs/en/sub-agents.md, read 2026-09-27

| Field | Used in | Value(s) | Still documented? | Valid value? |
|---|---|---|---|---|
| `name` | all 4, line 2 | `docs-auditor`, `provider-researcher`, `security-reviewer`, `spec-guardian` | Yes — required field | Yes — no `:`, doesn't start with `-` |
| `description` | all 4, line 3 | prose | Yes — required field | Yes, well under the 15,000-token *combined* budget the page warns about |
| `tools` | all 4, line 4 | `Read, Grep, Glob, Bash` (docs-auditor, security-reviewer, spec-guardian); `Read, Grep, Glob, Bash, WebFetch, WebSearch, Write, Edit, TodoWrite` (provider-researcher) | Yes — "Tool Access" table | Yes — comma-separated string of exact tool names, one of the two documented formats. `WebFetch`/`WebSearch`/`TodoWrite` are not individually enumerated on the page's example row but the page says "Exact names: `Read`, `Grep`, `Glob`, `Bash`, `Edit`, `Write`, etc." — treating them as ordinary built-in tool names is consistent with that wording, but the page does not list them explicitly, so I can't cite a literal match beyond the ones it names |
| `disallowedTools` | docs-auditor:5, security-reviewer:5, spec-guardian:5 | `Write, Edit, NotebookEdit` | Yes — "denylist; removes tools from inherited or specified list" | Yes, valid format and names. **Note:** because `tools:` on these three already restricts to `Read, Grep, Glob, Bash`, none of `Write`/`Edit`/`NotebookEdit` were reachable anyway — the denylist is a no-op given the current allowlist. Not wrong, just redundant belt-and-braces |
| `model` | all 4, line 6 (or 5 for provider-researcher) | `inherit` | Yes — "Model Selection" table lists `inherit` as an allowed value, meaning "main conversation's model" | Yes |
| `permissionMode` | docs-auditor:7, security-reviewer:7, spec-guardian:7 | `plan` | Yes — allowed values `default, acceptEdits, auto, dontAsk, bypassPermissions, plan, manual` | Yes, `plan` is one of the seven documented values |
| `color` | all 4, line 8 (or 6/7) | `green`, `cyan`, `red`, `orange` | Yes — "Advanced Configuration" table lists `red, blue, green, yellow, purple, orange, pink, cyan` | Yes, all four values used are in that exact list |

`provider-researcher.md` correctly omits `permissionMode` (it needs `Write`/`Edit` to produce `docs/research/*.md` notes; the page documents no explicit rule that `plan` blocks those tools, but pairing `plan` with a writer role would be the kind of self-defeating combination the reviewer subagents deliberately avoid — consistent, not a gap).

### 1b. Skills (`.claude/skills/*/SKILL.md`) — per https://code.claude.com/docs/en/skills.md, read 2026-09-27

| Field | Used in | Value(s) | Still documented? | Valid value? |
|---|---|---|---|---|
| `name` | all 14 skills, line 2 | matches the directory name in every case (e.g. `tooling-recheck` in `.claude/skills/tooling-recheck/SKILL.md`) | Yes | Yes — redundant with the documented directory-name default, but not wrong |
| `description` | all 14, line 3 | prose, well under 1,536 chars in every file (longest is `tooling-recheck`'s at ~440 chars) | Yes — required-ish ("Recommended") field, 1,536-char combined cap with `when_to_use` | Yes |
| `argument-hint` | 12 of 14 (all except `preflight`) | e.g. `"[what the check should detect]"`, `"[BCP-47 tag, e.g. pt-BR]"` | Yes | Yes — free-text hint string |
| `allowed-tools` | all 14 | see file-by-file list below | Yes, for the plain tool names and the `Bash(<pattern>)` form | See finding below for two files |
| `license` | all 14, last line before `---` | `Apache-2.0` | Yes — "Agent Skills spec" field | Yes — valid SPDX-style identifier |

`allowed-tools` values, file by file:
- `add-doctor-check`, `add-policy-rule`: `Read, Grep, Glob, Edit`/`Write`, `Bash(cargo test:*)`, `Bash(cargo fmt:*)`, `Bash(cargo clippy:*)`, `Bash(${CLAUDE_PROJECT_DIR}/scripts/validate.sh*)`, `Bash(${CLAUDE_PROJECT_DIR}/scripts/check-i18n-parity.sh)`, `Bash(${CLAUDE_PROJECT_DIR}/target/debug/macportsbrew*)` — all `Bash(<git-style pattern>)`, matches the documented "With bash rules" form (`Bash(git add *)`) exactly, including the documented `${CLAUDE_PROJECT_DIR}` substitution.
- `add-locale`: same shape, `Write, Edit` added.
- `add-mapping`: same shape plus `WebFetch(domain:formulae.brew.sh)` etc. (five `WebFetch(domain:...)` entries). The skills page's `allowed-tools` section only illustrates `Bash(<pattern>)`; it does not show a `WebFetch(domain:...)` form. This is the same restricted-tool-specifier idea (a scoped variant of a tool name), and is plausible given the sub-agents page's own "Specifiers" line (`Bash(git push *)` or `Agent(worker, researcher)`), but **the skills page itself gives no worked example or explicit grammar for `WebFetch(domain:...)`** — flagged as unconfirmed by this page (see §2).
- `add-provider`: same `Bash(<pattern>)` shape plus `Bash(${CLAUDE_PROJECT_DIR}/scripts/check-layering.sh)`, `Bash(${CLAUDE_PROJECT_DIR}/scripts/capture-fixture.sh*)`.
- `adr`, `decision-pending`, `research-note`, `spec-evolution`: plain tool names only (`Read, Glob, Write, Edit` / `Read, Edit` / `Read, Grep, Glob, Write, Edit` / `Read, Grep, Glob, Edit`) — unambiguously valid.
- `capture-fixture`: `Bash(<pattern>)` shape plus `Bash(jq *)`.
- `pass-report`: `Read, Grep, Glob, Write, Edit`, `Bash(${CLAUDE_PROJECT_DIR}/.claude/hooks/stop-gate.sh)`, `Bash(git status*)`, `Bash(git log*)`, `Bash(grep *)`, `Bash(wc *)` — all valid `Bash(<pattern>)` form.
- `preflight`: no plain-tool prefix beyond `Read, Grep, Glob`; the rest are `Bash(<pattern>)` entries including `Bash(bash -n *)`, `Bash(shellcheck *)`, `Bash(python3 -m json.tool *)`, `Bash(otool -l *)`, `Bash(vtool -show *)` — all valid.
- `tooling-recheck`: `Read, Grep, Glob, Agent`, then **`Edit(docs/research/tooling/**)`** and **`Write(docs/research/tooling/**)`** (glob-scoped path restrictions on `Edit`/`Write`, not on `Bash`). **The skills page's `allowed-tools` section documents only the `Bash(<pattern>)` form for scoped tool grants; it gives no grammar for a path-scoped `Edit(<glob>)`/`Write(<glob>)` form.** This is the one field/value pair I cannot confirm against the fetched page (see §2 for the concrete finding and fix).

Fields the page documents that none of the 14 SKILL.md files use: `when_to_use`, `arguments`, `disable-model-invocation`, `user-invocable`, `disallowed-tools`, `model`, `effort`, `context`, `agent`, `background`, `hooks`, `paths`, `shell`, `metadata`, `compatibility` — see §2 for the two that matter.

---

## 2. Documented mechanisms the repo does not use that would enforce something it only states in prose

**(a) `disable-model-invocation` — not set on any of the six "contributor recipe" skills.**
`CLAUDE.md` ("How work is shaped here") says of `add-mapping`, `add-policy-rule`,
`add-locale`, `add-doctor-check`, `capture-fixture`, `add-provider`: "They encode
procedures this project cares about getting right consistently, and **you invoke
them yourself** — a pass runs autonomously." That is a claim about *how* these
skills get triggered (deliberate invocation, not the model's own judgement mid-pass).
The skills page documents exactly the field that would enforce this: `disable-model-invocation: true` — "Prevent Claude from automatically loading this skill." None of
`.claude/skills/{add-mapping,add-policy-rule,add-locale,add-doctor-check,
capture-fixture,add-provider}/SKILL.md` set it, so nothing stops the model from
auto-invoking one of these mid-pass on its own judgement, contrary to the stated
intent. Fix: add `disable-model-invocation: true` to each of the six recipe
`SKILL.md` files, right after `license:`.

**(b) `context: fork` / `agent:` — not used by `tooling-recheck`, which asks for it in prose instead.**
`.claude/skills/tooling-recheck/SKILL.md` §1 ("Delegate, in parallel") instructs:
"One read-only subagent per group, all launched in one message" — i.e., fan out to
three parallel subagents by prose instruction, relying on `Agent` being in
`allowed-tools`. The skills page documents `context: fork` ("runs in subagent") and
`agent:` ("subagent type to use with `context: fork`") as a declarative way to make
a skill's own execution happen inside a subagent, with `background:` to detach it.
On inspection this doesn't actually fit the need here: `context: fork` forks the
*whole skill invocation* into one subagent, not three independently-scoped ones, so
it is not a drop-in replacement for the current three-way manual fan-out. Recorded
here as considered-and-rejected rather than a defect — worth a one-line note in the
skill body (or `docs/spec-evolution.md`) so a future recheck doesn't re-raise it as
new information.

---

## 3. Guidance not followed

None found within the scope of these two pages. Specifically checked and clean:
- No subagent `name` contains `:` or starts with `-` (would be silently skipped
  with a debug-log error per "Files Claude Code Skips").
- No subagent frontmatter has a missing `description` alongside a present `name`
  (same skip condition).
- No skill or subagent file's opening line is anything other than `---` (would
  cause the whole file to be treated as documentation/content instead of
  frontmatter).
- No skill uses the reserved names `synced` or `anthropic-skills`.
- `argument-hint` is used on 12 of 14 skills; `preflight` (no arguments needed)
  and none of the four subagents (the field doesn't apply to subagents) correctly
  omit it — not a gap.
- Every skill's `license: Apache-2.0` is present, syntactically an Agent-Skills-spec
  field, though **note in passing** that these skills also use Claude-Code-only
  fields (`argument-hint`, `allowed-tools` with `Bash(...)`/path-scoped patterns) that
  the page says "cause upload/packaging errors" if ever pushed through the
  claude.ai skill-upload path — not a problem for a project-local dev skill, and
  nothing in `CLAUDE.md`/`AGENTS.md` claims these skills are meant to be portable
  to that spec, so not counted as guidance-not-followed.

---

## 4. Summary of findings by file and line, with fix

| File | Line | Finding | Fix |
|---|---|---|---|
| `.claude/skills/add-mapping/SKILL.md` | 5 | `WebFetch(domain:...)` scoped-tool form used five times; not shown or described anywhere on the fetched skills page (only `Bash(<pattern>)` is documented there) | No repository change needed on this recheck; flag as unconfirmed-by-this-page. Confirm against `/permissions` (out of this recheck's assigned scope — belongs to the "Hooks, permissions, CI" group) before relying on it further, and cite that page here once done |
| `.claude/skills/tooling-recheck/SKILL.md` | 5 | `Edit(docs/research/tooling/**)` / `Write(docs/research/tooling/**)` glob-scoped forms used; same gap — not documented on the skills page, only `Bash(<pattern>)` is | Same as above: confirm against `/permissions`, then either cite it here or replace with a `Bash`-only equivalent if the glob-scoped `Edit`/`Write` form turns out unsupported |
| `.claude/skills/add-mapping/SKILL.md`, `add-policy-rule/SKILL.md`, `add-locale/SKILL.md`, `add-doctor-check/SKILL.md`, `capture-fixture/SKILL.md`, `add-provider/SKILL.md` | frontmatter (line 5, before `license:`) | `CLAUDE.md`'s "you invoke them yourself" claim for these six skills is not enforced — `disable-model-invocation` is available and unused | Add `disable-model-invocation: true` to each file's frontmatter |
| `.claude/skills/tooling-recheck/SKILL.md` | body §1 | Manual three-way subagent fan-out in prose instead of the declarative `context: fork` / `agent:` mechanism | No fix — mechanism doesn't actually fit (forks one subagent, not three scoped ones); recorded so it isn't re-raised as new next time |
| `.claude/agents/docs-auditor.md`, `security-reviewer.md`, `spec-guardian.md` | line 5 (`disallowedTools`) | `disallowedTools: Write, Edit, NotebookEdit` is a no-op given `tools:` on line 4 already excludes them | No fix needed; harmless redundancy, not a defect |

No field, value, or contract used by `.claude/agents/*.md` or `.claude/skills/*/SKILL.md`
was found to be **un**documented or invalid outright, other than the two path-scoped
tool-grant forms noted above, whose status is "unconfirmed by the two pages read for
this recheck" rather than "wrong."

---

## Decision recorded by the main session, 2026-09-27

- **Not adopted, deliberately: `disable-model-invocation: true` on the six
  recipe skills.** `CLAUDE.md` says "you invoke them yourself — a pass runs
  autonomously": the skills exist for the agent to invoke. The field would
  forbid exactly that. Recorded once so the next recheck does not re-raise it.
- The `WebFetch(domain:...)` and `Edit(glob)` grants in `allowed-tools` were
  confirmed as valid rule syntax by the hooks/permissions group's read of
  `/permissions` the same day.
