<!-- GENERATED FILE - do not edit. Generated from .claude/skills/adr/SKILL.md
     by scripts/generate-codex-layer.sh; `.claude/` is the source (DP-018). -->
---
name: adr
description: Write a new architecture decision record in docs/adr/ using the project template. Use when choosing between technologies or designs, when a decision forecloses an option, when a decision will be questioned later, or when docs/spec-evolution.md needs somewhere to put the reasoning behind a structural change.
argument-hint: "[decision title]"
allowed-tools: Read, Glob, Write, Edit
license: Apache-2.0
---

# Write an ADR

Decision to record: $ARGUMENTS

1. Read `docs/adr/0000-template.md` and cover every section it names — a
   section may be folded into another when the decision is small, but its
   question must be answered somewhere in the record. Read `0001`, `0002` and
   `0003` first: they set the standard for depth and for how an owner
   decision is recorded (the evaluation kept as written, the decision stated
   separately).
2. Take the next number. Filename: `NNNN-kebab-case-title.md`.
3. Status starts `proposed`. It becomes `accepted` when the human confirms, or
   when the decision is yours to make under the tier rules. Say which.

## What a good ADR here does

- **Context states what is known with sources and dates, and what is not
  known.** Separate the two. A confident-sounding context built on recollection
  is the failure mode this project is designed against.
- **Options considered are real options.** For each: what it is, what it costs,
  what it makes easy, what it makes hard, and *what it forecloses*. An ADR with
  one serious option and two straw men is a decision already made, written up.
- **The decision is one sentence you could put in a commit message.**
- **Consequences include the negative ones**, and name what becomes expensive to
  change later. If nothing became more expensive, you probably did not decide
  anything.
- **Compatibility impact** is answered concretely for macOS versions,
  architectures and provider versions - never left blank.
- **Revisit when** names a *condition*, not a date. "When Homebrew publishes a
  revised Intel timeline" is a condition. "In six months" is a reminder.

## Cross-references, both directions

- Add a `docs/spec-evolution.md` entry if the ADR changes the specification, and
  link the two.
- Add the ADR to the list in `docs/spec/README.md`.
- If it resolves a `DP-NNN`, move that item to Resolved in
  `docs/decisions-pending.md` and link it.
- If it supersedes an earlier ADR, mark the earlier one
  `superseded by ADR-NNNN` - do not delete it.

## The two constraints every ADR in this repository must clear

Any decision touching dependencies or the toolchain states explicitly that it
holds under both:

- **macOS 10.12 x86_64 / 11.0 arm64** (ADR-0002). A dependency that raises the
  deployment floor is rejected.
- **Apache-2.0 compatibility** (ADR-0001). GPLv2-only is forbidden outright.

Saying "this is fine" is not clearing them. Name the specific versions and
licences you checked, and the date.
