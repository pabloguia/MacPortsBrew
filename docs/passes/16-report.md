<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16 — report

The end-of-pass report of `prompts/16-complete-mutating-product.md`, written one
section per part as each part merges (§7). The live state is the ledger,
`docs/passes/16-progress.md`; each part's plan, reviews and "As built" are in
`docs/passes/16-plans/<part>.md`.

**A gap, recorded.** Parts A to E merged without their section here; this file
was first written with part F (2026-11-03). Their summaries below are taken
from the ledger and their plans, not written at their merge.

## Parts merged before this file existed

| Part | Merged | Where the detail is |
|---|---|---|
| A — documentation rulings | `439814c` (PR #26) | SE-109; two read-only `doctor` disclosures that DP-028's charter wording required |
| B — creation-mode seam | `d4d4737` (PR #27) | SE-109; the `0277` read-back half stays owed |
| C — advanced-mode entry | `0564fc3` (PR #28) | `16-plans/C.md`; its confirmation half lands in K |
| D — what defines a service | `cba6fbb` (PR #40) | `16-plans/D.md`; the startup-item recording is an owner item |
| P — owner items staged once | applied `12220fe`, `4a64aa0` | `docs/passes/16-staged/P/`; R-21, R-22, R-23 |
| M — Claude workflows | applied `22b7fdf` | `docs/passes/16-staged/M/`; the reviewer is deferred by the owner |
| S — slice 6 parts B and C | `8e7e01b` (PR #44) | `16-plans/S.md`; SE-107 |
| E — slice 7 and the dependency ADR | `cc324f9` (PR #51) | `16-plans/E.md`; ADR-0007, SE-110, SE-111 |

## F — slice 8, the executable list from provider metadata (DP-047 (b))

**Pull requests.** F-0 plan and spec `caf8799` (PR #55); F-1 staging `a163d5d`
(PR #56), applied by the owner as `1c74723` (PR #59); F-2a reads `111ab0e`
(PR #57); F-2b capture `6ee88ee` (PR #58); F-3 primary `bbfd890` (PR #61);
F-4 documentation (this file's pull request).

**What a user gets.** `snapshot <package>...` records each named package's
executables from its provider's own file list, and where each resolves on
`PATH`. A migration plan decides its primary per executable from the snapshot
alone, by one rule for both providers (INV-17): it completes only where the
destination's copy was observed winning, predicts no win, and refuses with
`PRIMARY_NEEDS_SHIM` where only a shim could help; while DP-055 is open, a
destination not installed yet is refused too, so most migrations' primary half
is refused. `VERIFY_RESOLUTION` must be
recorded `RESOLVED` before any `REMOVE_SOURCE` (INV-01). History moves to
`history.v3`. `rg` is decided by its own name rather than refused for not
matching `ripgrep` (DP-047 (b) reverses slice 8's name rule; unit-tested, and
the end-to-end test waits on the recapture). The switch stays off; the gate still refuses every plan.

**Deviations.** All in SE-112 and its F-3 addendum, and in F.md's "As built"
sections (F-2a deviations 1–6, F-2b, F-3 deviations 1–13), including
`ErrorId::PrimaryNeedsShim` defined and reserved for part I, and DP-055 (an
uninstalled destination's executables are unknown before install; refused in
both directions by default).

**Reviews.** spec-guardian and security-reviewer on the plan and on every code
pull request; docs-auditor on F-0, F-2b, F-3's fix round and F-4; each finding
resolved in F.md or in F-4's pull request; independent mutation
proofs per pull request (F-3: 62 mutants, 50 killed, the 10 survivors each later
killed by a named test, 2 argued equivalent).

**Not done, and why.**

- **The recording recapture** and what waits on it (deviations 9 and 10, the
  contract tests after `F/05`, the provenance in `fixtures/recordings/README.md`,
  and the ignored
  `migrate_reads_the_list_it_decides_on_and_a_replay_refuses_primary_resolution_unknown`).
  `scripts/capture-fixture.sh` aborts on macOS `ARG_MAX`; the script is guarded,
  so its fix is an owner item (the ledger's "Needs the owner"). F therefore ends
  `merged (owner item open)`.
- **The commit gate** skipped prefixed and worktree commits (`cf80b88` landed
  unvalidated, fixed later); the fix is staged as `F/07`/`F/08` for the owner.
- Handed on: H (owner queries, per-executable liveness), I (the shim offer and
  the shim target rule of §3 item 10), L (the new tripwire rows).

## G — the lock probe (DP-030 (b))

Built: the provider lock probe with the workspace's one `unsafe` (ADR-0007), its data,
its domain answers with a closed cause, and the refusal under the run lock and before
every mutating command. **Reachable today:** read-only commands show the sample. **Not
reachable:** the refusal, behind the switch, which stays off. Tripwires: the lock's
place in the shell (`[locks]`), and `mutations_cannot_dispatch_while_a_live_run_still_says_not_checked`,
now a check on every build. Reviewed by `spec-guardian` and `security-reviewer` at each
step; G-2's mutation proof left one accepted survivor (`G.md`). **Declared, not closed:**
04's false negatives; Apple Silicon and a lock a real provider holds are unverified (R-17,
owner items). The deviation from the schema compatibility rule, and from the plan's
order, are in SE-015.
