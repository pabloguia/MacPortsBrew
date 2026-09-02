---
name: research-note
description: Answer a blocking item from docs/spec/09-research-checklist.md and write the dated, sourced note in docs/research/. Use before fixing any architecture that depends on how Homebrew or MacPorts actually behaves, and whenever a provider fact is about to be stated from memory.
argument-hint: "[R-n or F-n item id]"
allowed-tools: Read, Grep, Glob, Write, Edit
license: Apache-2.0
---

# Answer a research item

Item: $ARGUMENTS (an `R-n` or `F-n` id from `docs/spec/09-research-checklist.md`)

## Delegate the actual research

Run this through the `provider-researcher` subagent, one item per invocation,
in parallel where the items are independent. That keeps the main context clean
and keeps the read-only discipline in a place where it is stated rather than
remembered. Your job here is to frame the question, check the note that comes
back, and wire it into the specification.

## The three-source rule

Every claim in the note is (a) observed on this machine with a read-only
command, (b) cited from official documentation with the date read, or (c)
deferred to runtime capability detection. There is no fourth option. Reject a
note that violates this, however plausible its content.

## Check the note before accepting it

- Does every claim carry its command or its URL *and* a date?
- Are evidence and conclusion separated, so a later reader can keep one and
  discard the other?
- Are the states distinguished rather than collapsed: "no artifact published"
  vs "artifact exists but not for this platform" vs "source build forced by
  configuration or a non-default variant"?
- Does it say which facts need the network? `analyze` must work offline with
  clearly-marked `UNKNOWN` values, so this determines a real code path.
- Does it say what it could *not* determine? A note with no `UNRESOLVED`
  section on a genuinely hard item is usually a note that guessed.

## Then wire it in

1. Save as `docs/research/<item-id>-<slug>.md` per `docs/research/README.md`.
2. Update the item's status in `docs/spec/09-research-checklist.md`.
3. If the finding contradicts the specification, that is the point of the
   exercise: record it with the `/spec-evolution` skill and change the spec.
4. If the finding is a dated, changeable fact about a provider - a support tier,
   a deadline, a prefix - it belongs in versioned **policy data** with its
   source and read date, never in control flow (INV-20).
