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
the relevant specification chapter when you touch `crates/adapters/**`,
`crates/policy/**`, `crates/domain/**`, `data/policy/**` or `locales/**`. Do not
restate their content in this file; extend the rule.

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
- `git push` — the repository has no remote yet by the owner's decision
  (DP-007).

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

The `Stop` gate does not implement its own checks: it runs
`scripts/validate.sh`, which is the one implementation of "valid" and the same
one CI runs. `--quick` (format, clippy, layering, SPDX, dependency licences,
i18n parity) on every turn end; the full default mode, which adds the test
suite, when `crates/`, `tests/`, `fixtures/`, `locales/`, `data/` or
`Cargo.lock` changed. `MPB_GATE_FULL=1` forces the tests regardless. Do not add
a check anywhere else.

## The two invariants a Claude session trips over most

INV-17 and INV-20. Read them in `docs/spec/01-invariants.md` before touching
a planner, an adapter or anything that mentions a support tier; the wording
there is the only copy.
