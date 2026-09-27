# 11 — The agentic SDLC

**Status:** Tier 3 (MUST) unless marked.

`07-quality-and-testing.md` states that MacPortsBrew will be evolved largely by
coding agents and that this is a design constraint. That chapter draws the
consequence for the *architecture*. This one draws it for the *process*: what
the repository must provide so an agent can do correct work, and what must be
mechanically enforced rather than merely asked for.

The governing idea is one sentence:

> **Anything an agent must not do is enforced by a mechanism, not by a
> sentence.** A rule that lives only in prose is a rule that will be followed
> until the moment it is inconvenient.

This is the same principle the product applies to its users — destructive
actions come from a reviewed plan, not from good intentions — turned inward.

---

## Verified facts about the tooling

The project's own three-source rule applies to the tools it is built with.
Everything specified below was verified against Anthropic's Claude Code
documentation at `https://code.claude.com/docs/en/`, read **2026-09-02**
(re-verified the same day at the start of the Foundation Pass; the
permissions, hooks, skills and github-actions pages re-read **2026-09-05**
during the security review that produced SE-072), and every group re-read
**2026-09-27** (`docs/research/tooling/2026-09-27-*.md`). The
pages are the ones `/tooling-recheck` names — that skill's table is the one
list — and the notes from each run live in `../research/tooling/`.

**MUST.** These are dated facts about a third-party tool, and therefore data,
not architecture — the same treatment INV-20 gives Homebrew and MacPorts facts.
Field names, hook events and rule syntax change. At the start of every pass,
`/tooling-recheck` re-fetches those pages in parallel subagents and reports
drift against this chapter, `.claude/` and `.github/workflows/`; material
drift is recorded in `../spec-evolution.md` like any other change. The
Repository Readiness Pass found four defects this way in configuration that
had been written the same day (SE-025), which is the argument for doing it
every time.

---

## Memory files

**MUST.** `AGENTS.md` at the repository root is the canonical, tool-neutral
orientation document. Its required content is listed in
`08-distribution-and-project.md`.

**MUST.** `CLAUDE.md` at the repository root is a thin Claude Code layer: it
imports `AGENTS.md` with `@AGENTS.md` and adds only what is specific to Claude
Code — the session reading order, the subagents and skills available, the
enforced boundaries, the delegation policy, and the mistakes that are easy to
make here. Claude Code's memory documentation now describes exactly this
pattern for projects that keep `AGENTS.md`.

Rationale: Claude Code reads `CLAUDE.md`, not `AGENTS.md`. Making `CLAUDE.md`
canonical would tie a project meant to outlive any one tool to that tool;
making `AGENTS.md` canonical and importing it costs one line (SE-024).

**MUST.** Both stay short — under 200 lines each — and neither restates the
specification. Long instruction files reduce adherence, and an import does not
reduce what is loaded: both files load at every session start. Content that grows
into a *procedure* moves to a skill; content that is a *fact about the domain*
moves to the specification. A rule that exists in both places will drift, and
the copy in the memory file wins by proximity, which is the wrong copy to win.

**MUST.** Claude Code's automatic per-repository memory is disabled in
`.claude/settings.json` (`autoMemoryEnabled: false`). Everything a future
session needs to know about this project is a versioned file — the
specification, `../decisions-pending.md`, `../research/`, this chapter — with
a source and a date. An undated recollection carried between sessions in a
file outside the repository is precisely the fourth source the working
protocol forbids. Personal notes go in `CLAUDE.local.md`, which is git-ignored.

**SHOULD.** Once there is code, path-scoped rules (`.claude/rules/*.md` with
`paths:` frontmatter) are the documented way to load the relevant
specification chapter when a file under, say, `adapters/` is touched — the
same on-demand principle as the per-file specification split, applied by the
tool rather than by the agent remembering to read. Not written before the
paths exist.

---

## The permission boundary

**MUST.** `.claude/settings.json` is checked in and denies, at minimum:

| Denied | Defends |
|---|---|
| All elevation (`sudo`, `doas`) | INV-06. Nothing in this repository's agent workflow needs root. |
| Every mutating `brew` and `port` subcommand | INV-05 and `07-quality-and-testing.md`: tests never touch the developer's real installation. |
| Writes into `/usr/local`, `/opt/local`, `/opt/homebrew`, `~/.homebrew` | `04-migration-and-safety.md`, "never rewrite, re-link or patch a binary" — that is the rule this defends. (INV-07 and INV-08 are about one provider satisfying the other's dependency, which is a different claim; citing them here would leave the rule orphaned the day either is reworded.) Written as `Edit(//opt/local/**)`: a single leading slash anchors at the settings file, not the filesystem root. Enforced for `Bash` too — a deny rule on a path does not reach a subprocess, and `cp target/release/… /opt/homebrew/bin/` was the way in. |
| Writes to shell configuration files | `05-environment-and-primary-provider.md`: shell files are written only through the managed-block procedure. |
| `launchctl`, `chsh`, `csrutil`, `spctl`, `systemsetup` | INV-19 for launch machinery; `05-environment-and-primary-provider.md` for the login shell; the threat model in `04-migration-and-safety.md` for the security tools. Three different rules, not one. |
| `rustup default|override|toolchain`, `cargo install` | ADR-0002. The pinned toolchain is what holds the macOS floor. |
| `git push` | DP-007 is decided (public GitHub), but the remote does not exist yet, so the first push is the owner's act of publishing. Both a bare `git push` and `git push <args>`: a rule ending in ` *` needs the argument. |
| `rust-toolchain.toml`, `docs/spec/01-invariants.md` | ADR-0002 and Tier 1. The `rustup` rule tells the agent to change the pin in the file instead of the active toolchain, which made the file the unguarded path to the same outcome. |
| The enforcement layer itself — `.claude/hooks/`, `.claude/settings.json`, `.claude/agents/`, `.claude/skills/`, **every** `scripts/*.sh`, `.github/workflows/`, and the generated `.codex/` and `.agents/skills/` | A control that a session can edit is a control that session can switch off, and the gate can only detect that its own path changed, never prevent it. `MPB_ALLOW_GUARD_EDIT=1` is the disclosed escape, on the same terms as `MPB_SKIP_STOP_GATE`. It is armed two ways, and they differ: set in the environment before the session starts it cannot be forged from inside, while the same literal in a user message arms it for **that message only** through a `UserPromptSubmit` hook — which a session with a shell could write itself, making that route detective rather than preventive. **The message route is narrower than "the same literal" (2026-09-20, F-52).** A subagent hand-back reaches that hook through the same channel a human message does, and twice it armed the escape by quoting this very cell. The literal must now be the **first thing on a line**, and a prompt carrying a relay envelope arms nothing at all. SE-081 carries the rationale and what the rule still does not cover; DP-022 carries the two changes that would make it a boundary and what each costs the owner. Both append every write they permit to `.claude/.guard-edit-log`, and the report cites it (SE-081). |

**MUST** be stated wherever the enforcement layer is described: **it is not a
property of the repository, it is a property of the harness reading it.** The
guards are Claude Code hooks declared in `.claude/settings.json`, plus the
`.codex/` layer generated from them (DP-018). A surface that does not read those
files runs none of it — no path guard, no provider-mutation guard, no `Stop`
gate — and nothing in the checkout would say so.

Which surfaces do, as of 2026-09-18 (code.claude.com/docs/en/overview):

- **The terminal CLI** and **the IDE extensions**, which are Claude Code.
- **The desktop app's `Code` tab.** The app "includes Claude Code, so you don't
  need to install the CLI separately" — same hooks, same `CLAUDE.md`, same
  skills.
- **Not** the same app's ordinary chat surface, which is not Claude Code. Work
  done there reaches this repository with every guard absent, and the absence is
  silent, which is the same shape as F-12 and F-42: a path that enforced nothing
  was indistinguishable from a path that enforced everything.

This was asked and answered because a session ended with the owner moving to the
desktop app; the answer is reassuring and the question was not obviously so, and
an unwritten "it depends which window you are in" is exactly the kind of thing
this file exists to write down.

**MUST.** Read-only provider subcommands are explicitly *allowed*. Observing
the live system is how provider facts are meant to be established
(`09-research-checklist.md`); a configuration that made observation
inconvenient would push agents toward recollection, which is the failure this
project is designed against.

**MUST.** `permissions.disableBypassPermissionsMode` is set to `"disable"`. A
project whose central claim is that destructive operations must be explicit and
reviewable does not ship a configuration with a blanket escape hatch. The
project does not set `permissions.defaultMode`: the mode is the operator's
choice per session, and plan mode is the recommended start for non-trivial
work.

**MUST.** `settings.json` sets the environment that keeps provider invocations
quiet during analysis — no auto-update, no analytics, no interactive hints.
This is the same requirement `04-migration-and-safety.md` places on the product
itself, applied to the agent's own shell, and it is one place, not many.

**MUST NOT.** Personal preferences do not go in `settings.json`. They go in
`.claude/settings.local.json`, which is git-ignored.

**SHOULD.** Claude Code's sandbox (`sandbox` in `settings.json`) is the
documented OS-level layer: it constrains what a Bash subprocess can write and
reach regardless of what the agent decided. It is evaluated and, if it does
not break the build, enabled by the Foundation Pass once the build's
filesystem and network needs are known. Enabling it blind against an
unwritten build would be a claim about a mechanism nobody has exercised.

---

## Hooks

**MUST.** Enforcement that matters is implemented as a hook, because permission
rules match command text: the permissions documentation says argument-
constraining Bash patterns are fragile, and that `Read`/`Edit` deny rules do
not apply to what a subprocess does. Two layers, deliberately.

| Event | Obligation |
|---|---|
| `SessionStart` (every source) | Inject the active pass, the hard boundary and the open decisions. Nothing that could be read from a file the agent will open anyway. |
| `PreToolUse(Bash)` | Deny elevation, mutating provider subcommands, toolchain switching, shell-file writes and launch machinery — including absolute-path, environment-prefixed and wrapped forms the rule syntax does not catch. |
| `PreToolUse(Edit\|Write\|NotebookEdit)` | Deny writes outside the repository into provider prefixes, shell configuration and system paths. |
| `PostToolUse(Edit\|Write)` | Cheap per-file checks only: format, lint, JSON validity, link resolution, SPDX header. Never blocks; findings are delivered as `additionalContext`, because exit-0 stderr from this event reaches nobody. |
| `Stop` | The definition-of-done gate. Blocks the turn ending while an objective check fails. |

**MUST.** A guard hook that cannot read its input **fails closed** — both when
the JSON parser is missing and when the input does not parse. "The parser was
missing" and "the input was garbage" must never be indistinguishable from "the
command was safe" — the same rule `02-domain-model.md` applies to `UNKNOWN`
artifact availability.

**MUST.** The hooks have a test: `.claude/hooks/test-hooks.sh` feeds each hook
fixture inputs — the mutating commands as *data* — and asserts the decision,
including the fail-closed paths and that a deliberately broken tree blocks the
`Stop` gate. It runs in CI. A mechanism that is never exercised is a sentence
with a file extension.

**MUST.** Every check in the `Stop` gate maps to a line in
`07-quality-and-testing.md` or `10-first-pass-work-order.md`. A check that maps
to nothing does not belong there. Only objective, machine-checkable failures
block; judgement calls warn, and warnings reach the human as a system message
rather than prolonging the turn.

**MUST.** The gate has a documented escape hatch (`MPB_SKIP_STOP_GATE=1`), and
using it — or Claude Code's own `disableAllHooks` — obliges the end-of-pass
report to say so. A gate with no escape hatch gets disabled wholesale the first
time it is wrong. Claude Code additionally stops honouring a `Stop` hook after
eight consecutive blocks (`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`); the gate tells
the agent when it is re-entering, and a turn that ends under that cap has
ended with the gate failing, which the report must also say.

**SHOULD.** As the implementation lands, the gate grows to run the real
validation — format, lint, layering check, i18n key parity, invariant tests,
coverage floors — so that "tests pass" is something the agent is prevented from
asserting falsely rather than merely asked not to.

### What this is not

**MUST** be stated plainly wherever this configuration is documented: it is not
a sandbox. A determined agent can defeat any text matcher, and Claude Code's
own auto mode runs classifier-approved commands without a human in the loop.
These are tripwires for the accident, not a cage for an adversary. The real
boundary is the permission system plus a human reading a plan — which is
precisely what INV-02 says about the product. Claiming more would be the same
dishonesty the project refuses in its support matrix.

---

## Subagents

**MUST.** `.claude/agents/` provides, at minimum, a researcher and three
reviewers:

| Subagent | Why it is separate |
|---|---|
| `provider-researcher` | Read-only *toward the machine and the specification*: it observes with read-only commands and writes only its research note. It holds `Write` and `Edit` because writing that note is its job, so the property is a discipline stated in its brief, not one the frontmatter enforces — unlike the three reviewers below. One research item per invocation; independent items run in parallel without polluting the main context. |
| `spec-guardian` | Reviews a change against the invariants and the authority tiers. |
| `security-reviewer` | Reviews against the threat model in `04-migration-and-safety.md`. |
| `docs-auditor` | Links, dated claims, support-matrix honesty, peer-provider balance, i18n parity. |

**MUST.** The reviewers are read-only twice over: `disallowedTools` covering
`Write`, `Edit` and `NotebookEdit`, and `permissionMode: plan`, the
harness's read-only mode for the file tools. Plan mode does **not** stop a
`Bash` heredoc or `sed -i`: the permission-modes documentation (read
2026-09-02, Foundation Pass recheck) says plan mode blocks the file-editing
tools while "any other shell command goes through the regular permission
flow" — which the readiness pass had already observed when a reviewer
created files under its scratch directory through `Bash`. So `Bash` is the
documented gap, the `PreToolUse` guard is the boundary that holds during a
review, and the reviewers' prompts are what keep them from patching. A
reviewer that can fix what it finds stops reporting and starts patching, and
the finding disappears into a diff nobody reads.

**MUST.** `model` stays `inherit` in every agent file. Which model runs a
subagent is decided at invocation by task weight (below), never by a model
name in a checked-in file (SE-017).

**MUST.** Before an end-of-pass report, and before any commit touching the
domain model, the policy engine, the planner, verification, privilege or the
process executor, the relevant reviewers run and their findings are addressed
or explicitly accepted in writing.

Rationale: an agent is a poor reviewer of its own work, and a long context stops
seeing what it has already read many times. The reviewers exist for the same
reason the architecture-review pass exists, at a smaller scale.

## Delegation by default

**MUST.** Read-only research and exploration are delegated to subagents rather
than performed in the main session: fetching and reading external
documentation, answering a research item, sweeping the repository for
occurrences of something, auditing a set of files, reviewing a diff. The main
session is reserved for synthesis, decisions, and edits that need the whole
repository in view.

**MUST.** Independent items run in parallel: the blocking research items, the
three review dimensions, the documentation pages `/tooling-recheck` verifies.
Sequencing is for dependencies, not for tidiness.

**MUST.** A subagent returns a report; the main session reads it and decides.
Findings are resolved or accepted in writing by the main session, never
relayed onward as if they were its own conclusions.

**MUST.** Model choice is by **task weight**, decided when the subagent is
invoked: the judgement the task needs multiplied by the cost of it being
wrong. A link sweep, a single documentation page re-verified, a fixture
compared — light. A review against the invariants, a research item with
conflicting sources, a threat-model review — heavy, and gets the strongest
model available. The choice uses whatever aliases the tool offers at the time
and is not written into the repository.

**SHOULD.** What stays inline: reading a file the agent is about to edit,
making the edit, making a decision the specification reserves for the main
agent. Delegating those would move the context the decision needs out of the
place where the decision is made.

Rationale and evidence: the sub-agents documentation (read 2026-09-02) states
that each subagent runs in its own context window, that subagents run in the
background concurrently, and that they inherit the project's `CLAUDE.md`
hierarchy — so a delegated task starts with the same orientation and none of
the accumulated context. The Repository Readiness Pass ran three
documentation verifications, a baseline audit and three reviews this way; the
main session's context held the specification it was editing and nothing it
did not need to edit.

---

## Skills

**MUST.** `.claude/skills/` holds the procedures this project needs performed
identically every time. At minimum: recording a specification change; writing
an ADR; producing a research note; reserving a decision for the human;
re-verifying the tooling facts; running preflight; writing the end-of-pass
report.

**MUST.** The foundation pass adds the contributor recipes named in
`08-distribution-and-project.md` as skills once there is code for them to
operate on: adding a package mapping, adding a policy rule, adding a locale,
adding a doctor check, capturing a fixture, and adding a provider. Each skill
and the corresponding section of `CONTRIBUTING.md` are written together, from
the same source of truth, so a contributor and an agent follow the same steps.

**MUST NOT.** A skill is not written for a procedure that does not exist yet.
Speculative recipes for unwritten code are worse than none: they read as
authoritative and are wrong in ways nobody notices until someone follows them.

**MUST.** The procedural skills stay model-invocable. Passes run
autonomously, and the agent is the one that must record a specification
change, reserve a decision or write the report at the moment the need
arises; `disable-model-invocation: true` would leave those procedures
reachable only by a human typing the command. It is reserved for a skill
whose effect is external and irreversible — publishing, pushing — of which
there is none.

---

## The pass protocol

**MUST.** Work is organised into **passes**. A pass has a written work order in
`prompts/`, a defined scope, an acceptance list and an end-of-pass report.
Passes are described by role, never by model name (SE-017), and numbered in
the order they run.

| Id | Pass | Purpose |
|---|---|---|
| `00-REPO-READINESS-PASS` | Repository readiness | Ran once, before any code: specification and tooling review, documentation consolidation. |
| `01-FOUNDATION-PASS` | Foundation | Establish the domain, the read-only slice and the machinery. |
| `02-ARCHITECTURE-REVIEW-PASS` | Architecture review | An independent reviewer challenges what the previous pass built. Reviewing, not continuing to code. |
| `03-FOCUSED-IMPLEMENTATION-PASS` | Focused implementation | One independently verifiable capability; the file is copied per task. |

**MUST.** `prompts/README.md` states the convention and the template. Every
work order follows it: what to read and in what order, the authority tiers, the
hard boundary for that pass, the working protocol, the acceptance list, and the
report.

**MUST.** `.claude/current-pass` names the active pass in one line. It is what
the session-start hook announces, and it is the only piece of per-session state
this repository keeps.

**MUST.** Every pass ends with a report in `docs/passes/`, and every material
deviation the report names already exists in `../spec-evolution.md`. The report
indexes; the evolution file records.

**MUST.** `../spec-evolution.md` and `../adr/` hold the **current rationale
per topic**, not a chronology: entries on one topic are merged, superseded
content is dropped, identifiers are stable, and a rejected change keeps its
entry because a rejection is the current rationale for something's absence.
Consolidate, not accumulate (SE-032). What does not change: a Tier 1–3 change
never lands without a traceable record of what changed and why, written in
the same change.

**SHOULD.** Non-trivial work starts in plan mode: explore, plan, review the
plan, then implement. The specification's tiering exists precisely so a plan
can be checked against something before code is written.

---

## Continuous integration

**MUST.** The validation described in `07-quality-and-testing.md` runs in CI and
is the authority. A local gate is a fast feedback loop, never the source of
truth.

**SHOULD.** Claude Code runs in CI for review, not for authorship: pull-request
review and `@claude` mentions on issues and pull requests. Anthropic publishes
`anthropics/claude-code-action` for this.

**MUST.** An unattended run identifies itself: both Claude workflows set
`MPB_UNATTENDED=1`, and `guard-protected-paths.sh` reads it and refuses edits
to `decisions-pending.md` and `docs/adr/` under it. Interactive sessions keep
the documented `/decision-pending` flow. Until 2026-09-05 the rule below was
asserted in a workflow comment and enforced nowhere, which is the exact shape
of defect this chapter's governing sentence is about.

**MUST NOT.** No unattended agent run holds write access to the default branch,
merges a pull request, publishes a release, or resolves an item in
`decisions-pending.md`. Those are the human's, and an automation that could take
them would make the decisions-pending file decorative.

**MUST.** Any CI job granting an agent tool access declares its allowed tools
explicitly (`--allowedTools` in `claude_args`). The same reasoning as
`04-migration-and-safety.md`'s allowlist for provider subcommands: capability
is granted by enumeration, never by default.

---

## Definition of done, for an agent

A change is done when everything in `07-quality-and-testing.md`'s definition of
done holds, **and**:

- the `Stop` gate passes, or the report says which check was skipped and why;
- every invariant the change touches can be pointed at by name to a test that
  defends it;
- every material deviation is in `../spec-evolution.md`, written when the change
  was made, not reconstructed afterwards;
- the relevant reviewers have run and their findings are resolved or explicitly
  accepted;
- every new claim about Homebrew or MacPorts carries its command or its source
  and a date, or is phrased conditionally, or is runtime detection.

**MUST.** Demonstrate, do not assert. A report that says a check passed without
showing the check is not a report, and this applies to the agent's own work
exactly as `10-first-pass-work-order.md` applies it to the product.
