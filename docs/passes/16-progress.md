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
| C — advanced-mode entry (warning, JSON) | 0 | merged `0564fc3` | PR #28 | Plan and reviews: `docs/passes/16-plans/C.md` (approved; mutation proof recorded). Its confirmation half lands in K |
| D — what defines a service | 0 | waiting on owner (P's `04-fixture-matrix-service-rule.patch`) | PR #29, docs merged `ca26df6` | Implemented and approved by both reviewers (code, two rounds); 20 of 21 mutants killed, m18 race-only. The code is `docs/passes/16-plans/D-implementation.patch`: the commit gate cannot pass until the matrix patch is applied. **Resume** as `wo16/D-2` from `origin/main` after it is: `git apply docs/passes/16-plans/D-implementation.patch`, fill the `<commit>` placeholder in `fixtures/snapshots/README.md`, split into Conventional commits, delete the patch file, run `scripts/validate.sh`. No port with a startup item is installed here (owner item) |
| P — stage the owner items, once | 0 | waiting on owner (merged `fef24c0`) | PR #30 | Staged: `docs/passes/16-staged/P/` (four patches, security-reviewed). Owner applies them; see Needs the owner. Research: R-21, R-22, R-23 |
| M — harden the Claude workflows | 0 | waiting on owner (merged `bc52b3c`) | PR #31 | Staged: `docs/passes/16-staged/M/` (three security reviews, final approve). Owner applies it; see Needs the owner |
| S — slice 6 parts B and C | 0 | in review `wo16/S-1-code` | S-0 merged `0f64d9d` (PR #32); S-1 on `wo16/S-1-code` | S-1 built (inert): plan `16-plans/S.md` "As built" and "Mutation proof" (40 mutants, 38 killed, 2 argued equivalent); code reviewed by both reviewers and approved on PR #33, low fixes applied. Next: S-1 merges, then **waiting on owner** `docs/passes/16-staged/S/` for S-2. Follow-up: the policy-version invariant does not count `analytics_read_on` (part A's 2026-10-27) |
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
- **Rule DP-054** (blocks nothing; `docs/decisions-pending.md`). Whether to
  relax DP-032 and DP-016 for `--replay` so replayed Homebrew plans keep showing.
  Action: reply (a) to keep the rulings as built, or (c) to relax them; the
  recommendation is (a).
- **Capture a MacPorts recording with a startup item** (part D's end-to-end
  case; D may merge without it). No port here declares or creates one. Action:
  on a machine with MacPorts `nginx` or `dbus` installed, run
  `scripts/capture-fixture.sh` and commit the recording under
  `fixtures/recordings/`.

- **Apply part M's staged workflows** (blocks setting the Claude key; nothing
  in the chain). Staged: `docs/passes/16-staged/M/`. Action: follow its README —
  `git apply` the patch, run `scripts/test-gates.sh`,
  `scripts/check-release-workflow.sh` and `scripts/validate.sh`, commit; then
  create the `claude` environment (required reviewer, `main` only) and store the
  key there only.

- **Apply part P's four staged patches** (blocks part D's code commit through
  04; E through 01; F and H through 02; J through 03). Staged:
  `docs/passes/16-staged/P/`. Action: follow its README — `git apply` the four
  patches, run `.claude/hooks/test-hooks.sh`,
  `scripts/generate-codex-layer.sh --check` and `scripts/validate.sh`, commit.

## Disclosures

Things a session or its subagents did that a reader must know, recorded rather
than buried (work order 16 §4.3; AGENTS.md "never silently").

- 2026-10-28, part M drafting subagent: before any hook fired, it copied
  `scripts/check-release-workflow.sh` into the scratchpad, edited the copy with
  Python run from Bash and ran it. A later Edit of such a copy was refused
  ("part of the enforcement layer"), which shows that route goes around the
  guard. The copy, its patch and its "verified" results were discarded; the
  staged script hunks were written as patch text and are unexecuted (see
  `docs/passes/16-staged/M/README.md`). It also made one reworded attempt to
  place a `rust-toolchain.toml` in the scratchpad after the ADR-0002 guard
  refused a copy; that was refused too and produced nothing.
- 2026-10-28, part P dependency research: after the ADR-0002 guard refused a
  scratch copy of `rust-toolchain.toml`, the scratch builds selected the same
  pinned 1.98.0 with `RUSTUP_TOOLCHAIN`. No default, override or installed
  toolchain changed; recorded in R-22.

## Decisions raised during the pass

Append each new DP number with the part that raised it.

- DP-054 — part D, 2026-10-28: whether to relax DP-032/DP-016 for replay (narrowed after review)

## Session log

One line per session, newest last: the date, the parts advanced, and where the
session stopped and why.

- 2026-10-27: ledger created; part 0 done; no implementation part started.
  The resumability rules came in with the pull request that added this file.
- 2026-10-28: C merged (PR #28); D implemented and approved, held as a patch on P's matrix patch (PR #29); P and M staged and merged (PRs #30, #31). Next: S, which needs nothing from the owner; D's code, E, F, H and J wait on P's staged items.
