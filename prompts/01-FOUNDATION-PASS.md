# MacPortsBrew — Foundation Pass (01-FOUNDATION-PASS)

You are the primary implementation agent for **MacPortsBrew**, an open-source
macOS package-manager orchestration tool. The repository currently contains its
specification, its agent configuration and no implementation. You are building
the foundation: the research that fixes the architecture, and a read-only
vertical slice a real user can run.

---

## 1. Read this first

In this order, before writing any code:

1. `AGENTS.md` — orientation and the working protocol.
2. `docs/spec/00-charter.md` — what MacPortsBrew is, who it is for, what it is not.
3. `docs/spec/01-invariants.md` — rules that may never be broken. Non-negotiable.
4. `docs/spec-evolution.md` — the changes already made to the original brief, and
   why. Read it so you do not spend this pass re-litigating settled ground.
5. `docs/spec/09-research-checklist.md` — what you MUST verify before deciding.
6. `docs/spec/10-first-pass-work-order.md` — the full scope and acceptance list.
7. `docs/spec/11-agentic-sdlc.md` — how work is shaped here: passes, reviewers,
   gates, and what is enforced rather than asked for.

Read the remaining `docs/spec/*.md` on demand. Each file stands alone; do not
load the whole specification into context up front.

## 2. Authority and precedence

The specification is tiered. When two statements conflict, the higher tier wins:

| Tier | Source | Changeable? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | No. If one seems wrong, **stop and ask the human.** |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. |
| 3 | Everything marked **MUST** in `docs/spec/02..11` | Only with a `docs/spec-evolution.md` entry. |
| 4 | Everything marked **SHOULD** | Change freely, log the material ones. |
| 5 | Everything marked **HYPOTHESIS** | Expected to change. Log the outcome. |

You are explicitly authorised — and expected — to improve, correct, simplify and
extend tiers 3–5 when evidence or sound engineering justifies it.
You may never do so **silently**: every material change goes into
`docs/spec-evolution.md` through the `/spec-evolution` skill, in the same change
that makes it. Not reconstructed at the end of the pass, when the evidence is
gone.

## 3. Hard boundary for this pass

**This pass ships no code that can mutate the system.**

No install, no uninstall, no `sudo`, no writes outside the repository and the
tool's own state directory, no shell-profile edits, no symlink creation, no
service control. Not behind a feature flag, not behind a prompt, not commented
as "wired up later". Planning and dry-run output are the deliverable; execution
is a later pass.

This is checkable, and it is checked: the `Stop` gate greps `src/` for
provider-mutating subcommand literals and for `sudo`. Keep it checkable. If you
find yourself writing an `uninstall()` body, stop: write the *plan type* and the
*test* instead, and leave the executor unimplemented behind an explicit
`ErrNotImplementedInThisPass` (or the chosen language's equivalent).

A second boundary, on your own working environment: **never run a mutating
`brew` or `port` command, and never elevate.** `.claude/settings.json` denies
these and `.claude/hooks/guard-provider-mutation.sh` blocks the forms the rule
syntax misses. Read-only provider commands are allowed and are how you are meant
to establish provider facts. If a guard blocks something you believe is
legitimate, that is a question for the human — rewording a command to slip past
it is itself a specification violation.

## 4. Working protocol

- **Verify, do not assume.** Every factual claim about Homebrew or MacPorts must
  be (a) observed on this machine with a read-only command, (b) cited from
  official documentation with the date you read it, or (c) expressed as runtime
  capability detection. Never a fourth option, and "I recall that…" is not one
  of the three.
- **Research first, build second.** Complete `09-research-checklist.md` and
  record the answers in `docs/research/` before the architecture is fixed. Run
  independent items in parallel `provider-researcher` subagents so the main
  context stays clean. `/research-note` frames one.
- **Plan before you build.** Start in plan mode. Produce a plan, have it
  reviewed by the human or by `spec-guardian`, then implement.
- **Use the reviewers.** `spec-guardian` on anything touching the domain model,
  the policy engine, the planner, verification, privilege or the process
  executor. `security-reviewer` on the executor, path resolution, elevation,
  state, provider-output parsing, packaging and dependencies. `docs-auditor`
  before you call the documentation done. You are not the right reviewer of your
  own change.
- **Commit incrementally.** Conventional commits, one coherent change each. A
  reviewer must be able to read the history and follow the reasoning. Do not
  deliver the whole pass as a single commit.
- **Small files, explicit interfaces.** See `docs/spec/07-quality-and-testing.md`.
- **Never touch the developer's real package managers.** Tests and development
  run against captured fixtures. Reading the live system is allowed; writing is
  a build failure.
- **When blocked on a decision that is the human's** — anything that trades away
  part of the target audience, spends their money, commits their legal identity
  or makes the project public — use `/decision-pending`: write the options and
  your recommendation into `docs/decisions-pending.md`, take the *reversible*
  default, and keep going.

## 5. Order of work

`docs/spec/10-first-pass-work-order.md` holds the full sequence, steps 0
to 21. Three things about it that are not obvious from the list:

**Step 0 — the repository is not yet under version control.** DP-007 records
that this was the owner's choice. Initialising git locally is this pass's first
action; pushing anywhere is a separate, explicit decision and `git push` is
denied. Commit the existing specification before you change anything, so the
pass's history starts from a known state.

**The language is decided; the toolchain is not verified.** Rust, ADR-0002,
accepted 2026-09-02. Nothing is gated on a human any more — but do not read
"decided" as "settled". That ADR carries a verification list, and every item on
it can invalidate a crate choice *after* you have built on it: the deployment
target the linker actually recorded on both architectures, a transitive
dependency quietly raising the floor, the TUI crate's East Asian double-width
handling. Work through that list before the bootstrap, with checks that run in
CI rather than with inspection. Discovering the double-width problem after the
UI is written means rewriting the UI.

**The order is causal, not bureaucratic.** The process executor comes before the
adapters because every provider fact you can trust flows through it, sanitised
environment and all (INV-10). The domain model comes before either adapter
because an adapter written first will shape the domain around whichever provider
was written first, and INV-17 forbids exactly that asymmetry. If you deviate
from the order, say which causal link you decided was not real.

## 6. Definition of done

`docs/spec/10-first-pass-work-order.md` holds the acceptance list, and it is
the only copy — this section does not restate it. All of it must be true, and
you must **demonstrate** each item, not assert it. In addition:

- [ ] `/tooling-recheck` ran at the start of the pass and its drift, if any,
      is recorded.
- [ ] `.claude/hooks/test-hooks.sh` passes, extended for any hook you changed.
- [ ] `/preflight` passes, and the reviewers' findings are resolved or accepted
      in writing.

The items below are the ones on that list most often reported without being
shown; they are repeated here only as a reminder to show them:

- [ ] Every item in ADR-0002's verification list is demonstrated by a check that
      runs in CI: deployment target asserted on the built artifacts for both
      architectures, no dependency raising the floor, CJK double-width verified,
      toolchain pinned in `rust-toolchain.toml`.
- [ ] The read-only vertical slice runs end to end on this machine and against fixtures.
- [ ] Every read command has a `--json` mode with a versioned schema.
- [ ] `analyze --from-snapshot <file>` reproduces byte-identical recommendations
      from a captured machine snapshot. This is how determinism is proven.
- [ ] Invariant tests exist and are named after the invariants they defend.
- [ ] CI is green: format, lint, static analysis, layering, i18n key parity,
      tests, coverage floors, build for both architectures.
- [ ] `AGENTS.md` is expanded to everything `08-distribution-and-project.md`
      requires of it, and `README.md`, the `wiki/` foundation and
      `docs/spec-evolution.md` are current.
- [ ] The contributor recipes in `08-distribution-and-project.md` exist as
      skills in `.claude/skills/` and as `CONTRIBUTING.md` sections written from
      the same source.
- [ ] The `Stop` gate has grown to run the real validation, not just the
      documentation checks it runs today.
- [ ] No mutating code path exists in the shipped binary — and `grep` says so.
- [ ] `/preflight` passes, and the reviewers' findings are resolved or accepted
      in writing.

## 7. End-of-pass report

Write it with `/pass-report` to `docs/passes/01-FOUNDATION-PASS-report.md`.
It covers: research findings with sources and dates; the language decision and
its rationale; specification changes made, deferred and **rejected**; what the
vertical slice does and does not do; test and coverage status per layer; CI
status and what it does *not* cover; known limitations; risks discovered; and
the recommended tasks for the architecture-review pass and the focused
implementation passes that follow.

Every material deviation named in that report must **already** exist in
`docs/spec-evolution.md`. The report indexes; the evolution file records.
