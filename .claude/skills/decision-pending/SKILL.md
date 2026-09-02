---
name: decision-pending
description: Record a decision that belongs to the human in docs/decisions-pending.md, or move a resolved one to the Resolved section. Use whenever you are blocked on a choice that trades away part of the target audience, spends the owner's money, commits their legal identity, or makes the project public.
argument-hint: "[what needs deciding]"
allowed-tools: Read, Edit
license: Apache-2.0
---

# Reserve a decision for the human

What needs deciding: $ARGUMENTS

## Is it theirs?

It is the human's decision when it:

- trades away part of the target audience (a minimum OS, a dropped architecture);
- costs money or commits a legal identity (an Apple Developer account, the
  copyright holder, the licence);
- makes something public or irreversible (pushing a repository, publishing a
  release, claiming a name);
- changes the charter - identity, audience, positioning, a non-goal;
- would require relaxing a Tier 1 invariant.

It is **yours** when it is a design or implementation trade-off inside the
tiers you are allowed to change. Do not park work on the human to avoid
deciding; that is how a pass produces a list of questions instead of a product.

## The entry, and the part people skip

Add to the **Open** section of `docs/decisions-pending.md`, matching the
existing entries' shape:

- `### DP-NNN — Title`
- **Options**, each with its real cost - not one option and two straw men.
- **Recommendation**, with the reasoning compressed to a few lines and a
  pointer to the ADR if there is one.
- **What it blocks**, honestly. Most decisions block less than they appear to.
- **Default meanwhile** - and this is the part that matters. Pick the
  *reversible* default, take it, and keep going. Work is never blocked on a
  pending decision unless the decision is genuinely load-bearing, in which case
  say `Default meanwhile: none` and say what stops.

## Resolving one

When the human decides: move the entry to **Resolved** with the date and the
decision, point at the ADR or the `docs/spec-evolution.md` entry that records
it, and then *propagate the consequences*. A resolved decision that is filed
rather than propagated is worse than an open one, because it looks handled.
SE-021 is the model: the licence choice was not just recorded, it produced SPDX
headers, a dependency allowlist, a CI check and a constraint on the language
ADR. Ask what your decision forces, and go and do it.
