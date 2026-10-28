<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16 — progress ledger

This is the state of `prompts/16-complete-mutating-product.md`. It exists so
that a fresh session can continue where the last one stopped.

- **Read it first.** The work order's §4.1 says how to reconcile it.
- **Update it at every transition.** §4.2 says how.
- **When it disagrees with the outside world, the outside world wins.** The
  order is: GitHub, then the newest ledger on an open `wo16/*` branch, then this
  file on `main`.

Branches are named `wo16/<part>-<slug>`. The status values are listed in §4.2.

## Parts

| Part | Depends on | Status | Branch / PR / commit | Notes |
|---|---|---|---|---|
| 0 — owner confirms the rulings | — | merged `fd443ef` | PR #24 | Confirmed in chat 2026-10-27 |
| A — documentation rulings | 0 | merged `439814c` | PR #26 | Built two read-only `doctor` disclosures DP-028's charter wording needed (SE-109) |
| B — creation-mode seam | 0 | merged `d4d4737` | PR #27 | The `0277` read-back half stays owed (SE-109) |
| C — advanced-mode entry (warning, JSON) | 0 | in review PR #28 | PR #28 | Plan and reviews: `docs/passes/16-plans/C.md` (approved; mutation proof recorded). Its confirmation half lands in K |
| D — what defines a service | 0 | todo | | May merge with the capture owed |
| P — stage the owner items, once | 0 | todo | | Ends `waiting on owner` |
| M — harden the Claude workflows | 0 | todo | | Ends `waiting on owner` |
| S — slice 6 parts B and C | 0 | todo | | First of the chain |
| E — slice 7 + the dependency ADR | S, P | todo | | P's `check-licences.sh` entries must be applied before the dependency lands |
| F — slice 8 | E, P | todo | | P's allowlist growth must be applied |
| G — lock probe | F | todo | | |
| H — four destination checks | G, P | todo | | |
| K — removal opt-in | H, C, A | todo | | **Blocking:** C's confirmation half and the contract in `16-plans/C.md` §9 (absent `enteredBy` = strictest; level derived, never trusted; `UNKNOWN` ownership a deliberate choice) |
| I — layer 4, consented | K | todo | | |
| J — elevation | I, P | todo | | Needs P's elevation entries applied |
| L — flip proposal | J | todo | | The deliverable is a document |

"Depends on P" means that P's staged items for this part were applied, checked
by §3 item 5.4. It does not mean that P is merged.

## Needs the owner

Append items here as they appear. Each line gives:

- the item;
- the part it blocks;
- the staged path, if any;
- the exact action.

Remove a line when the owner has done it, citing the commit.

- **The hooks/permissions/CI group of `/tooling-recheck`** (blocks nothing in
  the chain; leaves `.claude/hooks/` contracts un-re-read since 2026-10-26).
  The auto-mode classifier refused the session's subagent launch for it on
  2026-10-27 ("Auto-Mode Bypass"). Action: run `/tooling-recheck` with scope
  "Hooks, permissions, CI" yourself, or allow that subagent launch, and save the
  note as `docs/research/tooling/2026-10-27-hooks-permissions-ci.md`.

## Decisions raised during the pass

Append each new DP number with the part that raised it.

- (none yet)

## Session log

One line per session, newest last: the date, the parts advanced, and where the
session stopped and why.

- 2026-10-27: ledger created; part 0 done; no implementation part started.
  The resumability rules came in with the pull request that added this file.
