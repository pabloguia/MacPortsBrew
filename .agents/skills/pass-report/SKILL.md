<!-- GENERATED FILE - do not edit. Generated from .claude/skills/pass-report/SKILL.md
     by scripts/generate-codex-layer.sh; `.claude/` is the source (DP-018). -->
---
name: pass-report
description: Write the end-of-pass report required by every MacPortsBrew work order. Use at the end of a foundation, architecture-review or focused implementation pass, after preflight has passed.
argument-hint: "[pass id, e.g. 01-FOUNDATION-PASS]"
allowed-tools: Read, Grep, Glob, Write, Edit, Bash(${CLAUDE_PROJECT_DIR}/.codex/hooks/stop-gate.sh), Bash(git status*), Bash(git log*), Bash(grep *), Bash(wc *)
license: Apache-2.0
---

# End-of-pass report

Pass: $ARGUMENTS

Write it to `docs/passes/<pass-id>-report.md`. It is the handover to the next
agent and to a maintainer reading the repository in a year. Write it for them,
not as a summary of your session.

## Run `/preflight` first

A report written before the gate is a claim, not a result. If you are reporting
with the gate failing or skipped, say so at the top, in the first paragraph,
with the reason.

## Sections

**What shipped.** What a user can now do that they could not before. If the
answer is "nothing runs yet", say that plainly.

**Research findings.** Each blocking item from
`docs/spec/09-research-checklist.md`: answered, partially answered or
unresolved, with the note that holds it. List what remains unresolved and what
it blocks.

**Decisions.** Decisions you made and why. Decisions you reserved for the human,
with the reversible default you took meanwhile. Every ADR written or changed.

**Specification changes.** Made, deferred and *rejected* - rejections are the
most useful section for the next agent, because they stop a settled question
being reopened. Each one must already exist in `docs/spec-evolution.md`; this
section indexes them, it does not replace them.

**What it does not do.** The boundary of the slice. Be specific enough that
nobody has to discover a gap by hitting it.

**Tests and coverage.** Per layer, against the floors in
`docs/spec/07-quality-and-testing.md`. Name the invariants that now have tests
and the ones that do not.

**CI.** Green or not, on what, for which architectures, and what is not covered.
If a support claim is not backed by something CI runs, say so here so the
support matrix stays honest.

**Known limitations and risks discovered.** Including the assumptions this pass
made that a reviewer should attack.

**Recommended next tasks.** Concrete and independently verifiable. Say which
pass type each belongs to.

## The rule about this report

Every material deviation it names must **already** be in
`docs/spec-evolution.md`. The report indexes; the evolution file records. If you
find yourself explaining a deviation here for the first time, stop and go write
the entry.
