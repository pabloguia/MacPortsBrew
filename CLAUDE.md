# MacPortsBrew — Claude Code

@AGENTS.md is the orientation document for every agent working here. Read it
first; this file adds only what is specific to Claude Code.

## Start of a session

`.claude/hooks/session-start.sh` injects the active pass, the hard boundary and
the open decisions. Then read, in order:

1. `docs/spec/00-charter.md` — what this is and who it is for.
2. `docs/spec/01-invariants.md` — Tier 1. Not changeable by you.
3. `docs/spec-evolution.md` — the current rationale per topic, so you do not
   reopen settled ground.
4. `docs/spec/11-agentic-sdlc.md` — how work is shaped here. Once.
5. `prompts/<active pass>.md` — the work order.

Read the rest of `docs/spec/` on demand. Each file stands alone. Do not load
the whole specification up front — that is what the tiering and the per-file
split are for.

## How work is shaped here

**Re-verify the tooling first.** `/tooling-recheck` at the start of every pass.
Claude Code's field names, hook contracts and rule syntax are dated facts, and
the configuration in `.claude/` is only as good as the last check.

**Plan before you build.** Start non-trivial work in plan mode
(`claude --permission-mode plan`, or Shift+Tab). Explore, produce a plan,
have it reviewed, then implement. `docs/spec/11-agentic-sdlc.md` describes the
pass protocol this fits into.

**Delegate by default.** Read-only research and exploration go to subagents;
independent items run in parallel in one message; the main session is for
synthesis, decisions and the edits that need the whole repository in view.
Pick the model by task weight at invocation — the judgement needed times the
cost of being wrong — never by a model name in a file. Read what a subagent
returns and decide; do not relay it. `.claude/agents/` holds four —
`provider-researcher`, `spec-guardian`, `security-reviewer`, `docs-auditor` —
described in `docs/spec/11-agentic-sdlc.md` (Subagents). You are not the
right reviewer of your own change: run the relevant ones before you report
anything complete, and resolve or accept their findings in writing.

**Use the skills.** Procedural: `/tooling-recheck`, `/spec-evolution`, `/adr`,
`/research-note`, `/decision-pending`, `/preflight`, `/pass-report`. Contributor
recipes, each written from the same source as its `CONTRIBUTING.md` section so
a human and an agent follow identical steps: `/add-mapping`,
`/add-policy-rule`, `/add-locale`, `/add-doctor-check`, `/capture-fixture`,
`/add-provider`. They encode procedures this project cares about getting right
consistently, and you invoke them yourself — a pass runs autonomously.

**Path-scoped rules.** `.claude/rules/*.md` carry `paths:` frontmatter and load
the relevant specification chapter when the Read, Edit or Write tool opens a file
under `crates/adapters/**`, `crates/policy/**`, `crates/domain/**`,
`data/policy/**`, `locales/**` or the executor (`crates/cli/src/executor/**`,
`crates/application/src/executor*`, `execution_gate.rs`, `verification.rs`) —
a shell or MCP reader loads nothing, so open a file with Read before changing
it. Do not restate their content in this file; extend the rule.

**Verify before you claim.** `/preflight` runs the definition-of-done checks.
The `Stop` hook runs the mechanical part of it whether you ask or not, and will
refuse to let the turn end while an objective check fails. That is deliberate:
"give the agent a way to check its work" beats "tell the agent to be careful".

**Observe the machine without touching it.** `--replay
fixtures/recordings/<name>.json` runs the whole binary against a captured
recording, and `--from-snapshot <file>` analyses a captured snapshot. Both are
how you exercise a code path on a machine you do not have — and neither
consults the live filesystem, which
`invariant_15_a_replayed_reader_never_consults_the_live_filesystem` asserts.
`scripts/capture-fixture.sh` produces a recording; fixtures are captured, never
invented.

## Boundaries the tooling enforces

`.claude/settings.json` denies, and `.claude/hooks/` blocks a second time:

- **All elevation.** Nothing here needs `sudo` (INV-06).
- **Every mutating `brew` and `port` subcommand.** Read-only ones are allowed
  and are how provider behaviour is meant to be observed. Tests use captured
  fixtures, never a live install.
- **Writes into either provider's prefix, into shell configuration, and into
  launch machinery.**
- `git push` of anything but a topic branch to `origin` — force and delete
  forms, `--mirror`/`--all`, a push aimed at `main` or `master`, a URL or path
  destination, or an `origin` that is not this project's repository (SE-099).
  An ordinary push of a topic branch is allowed. The guard reads the whole
  command line, so run `git push` on its own: no `cd` prefix, no chaining.
- `gh pr merge --admin`, every `gh api` call that writes, and `--tags` or
  `--follow-tags` on a push (SE-108).

## Publishing and releases (SE-108)

An interactive session carries a change all the way: push the topic branch,
open the pull request, and merge it (`gh pr merge <n> --merge`) once
`gh pr checks <n> --required` shows every required check green. `enforce_admins`
is off and the session holds the owner's credential, so branch protection does
not stop it; the hook does. It may also tag a version the specification and
`CHANGELOG.md` already describe, at a commit on `origin/main`, from its topic
branch, one tag per push: `git tag -a vX.Y.Z -m "..." origin/main`, then
`git push origin vX.Y.Z`. What stays the owner's: approving the
`release-signing` environment, the certificates and their passwords, every
repository or GitHub setting, and any force, delete or rewrite. An unattended
run does none of this (`docs/spec/11-agentic-sdlc.md`).

If a hook blocks something you believe is legitimate, that is a question for
the human, not an obstacle to route around. Rewording a command to slip past a
guard is a specification violation in itself. If the block is a test that
names a mutating command as data, put the fixture in a file — that is what
`.claude/hooks/test-hooks.sh` does.

## Commits

The convention is in `AGENTS.md`. Never commit `.claude/settings.local.json`
or `CLAUDE.local.md`; both are git-ignored for a reason.

## The toolchain, as the tooling sees it

`rustup default|override|toolchain` and `cargo install` are denied because
the pinned toolchain holds the macOS floor (ADR-0002 has the reasoning). `cargo
fmt`, `clippy`, `test`, `build` and `tree` are allowed.

The gates do not implement their own checks: they run `scripts/validate.sh`,
which is the one implementation of "valid" and the same one CI runs. The `Stop`
gate runs `--quick` on every turn end; `commit-gate.sh` runs the default mode
(the test suite and the fixture matrix) before every `git commit` that touches
code, and refuses the commit while it fails (SE-093). `MPB_GATE_FULL=1` makes
the `Stop` gate run the tests too. **Do not add a check anywhere else.**

**The stage list is in `validate.sh`'s own header and nowhere else.** It used to
be written here and in `CONTRIBUTING.md` as well, and both copies drifted by five
stages — including four of the five added in one day. Three spellings of one
thing is two spellings that will be wrong, which is the argument
`check-action-pins.sh` already makes about the toolchain pin.

## The two invariants a Claude session trips over most

INV-17 and INV-20. Read them in `docs/spec/01-invariants.md` before touching
a planner, an adapter or anything that mentions a support tier; the wording
there is the only copy.
