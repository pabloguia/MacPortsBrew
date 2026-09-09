<!-- GENERATED FILE - do not edit. Generated from .claude/skills/spec-evolution/SKILL.md
     by scripts/generate-codex-layer.sh; `.claude/` is the source (DP-018). -->
---
name: spec-evolution
description: Record a material change to the MacPortsBrew specification in docs/spec-evolution.md, in the project's entry format. Use whenever you change, correct, simplify, extend or reject anything in docs/spec/ above the trivial-implementation-choice line, and before finishing any pass that made such a change.
argument-hint: "[short title of the change]"
allowed-tools: Read, Grep, Glob, Edit
license: Apache-2.0
---

# Record a specification change

Change to record: $ARGUMENTS

The project's one unrecoverable error is deviating from the specification
silently. Improving it is expected; hiding the improvement is not. This is how
the improvement gets written down.

## First: is it material?

Record it if any of these is true. Otherwise do not - a log of trivia makes the
file useless.

- It changes what a **MUST** statement requires, or promotes/demotes a
  requirement between tiers.
- It changes a type, a boundary, a contract or a default that other parts of
  the specification depend on.
- It changes what a user sees, what a machine-readable surface emits, or what
  the tool refuses to do.
- It resolves, contradicts or invalidates a **HYPOTHESIS**.
- It rejects something the specification asks for. A rejection is a material
  change and gets an entry with `Status: rejected`.

Tier 1 (`01-invariants.md`) is not changeable this way at all. If an invariant
looks wrong, stop and raise it with the human.
Tier 2 (`00-charter.md`) needs an explicit human decision first.

## Then

The file is organised by topic and holds the current rationale, not a
chronology — consolidate, not accumulate (SE-032).

1. Find the topic. If an entry already covers it, **extend that entry**: add
   the pass and date to `Date / Pass`, fold the new change into `Change` and
   `Rationale`, and drop whatever the new change supersedes. Keep the id.
   Only a genuinely new topic gets the next free `SE-NNN`, placed under the
   matching heading.
2. Use the format declared at the top of that file. Every field, in that
   order. An entry missing `Evidence` is not an entry.
3. Write `Evidence` as evidence, not as reasoning: a URL with the date you read
   it, a command and its output, a test that fails, a measurement. "It seemed
   cleaner" is a rationale; put it under `Rationale` and find real evidence or
   mark the entry `experimental`.
4. Say what the change costs, not only what it buys. `User impact`,
   `Architectural impact` and `Compatibility impact` are where a future reader
   decides whether to trust the entry.
5. Make the corresponding edit in `docs/spec/`. The entry and the edit belong in
   the same change.
6. If the change resolves an item in `docs/decisions-pending.md`, move it to the
   Resolved section and point at the entry.
7. If it is architectural, it probably also wants an ADR - see the `/adr` skill.
   The ADR holds the reasoning; the SE entry holds the diff to the specification.

## Anti-patterns to avoid

Writing the entry after the fact, from memory, at the end of a long pass. By
then the evidence is gone and the entry becomes a summary of what you decided
rather than a record of why. Write it when you make the change.

Adding a new entry for a topic that has one, so the file tells the story
twice. Merging an entry whose id something else references without updating
the reference in the same change.
