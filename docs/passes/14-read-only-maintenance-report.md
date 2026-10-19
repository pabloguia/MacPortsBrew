<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 14 — read-only maintenance after pass 08 slice 5: report

Branch `pass/14-read-only-maintenance`, cut from `docs/handoff-after-slice-5`
(commit `127596d`, the planning commit that wrote this work order). Work order:
[`prompts/14-read-only-maintenance.md`](../../prompts/14-read-only-maintenance.md).
Run on 2026-10-18 by a session on the strongest model available (the owner had
asked for a cheaper one; a session cannot change its own model), with
`spec-guardian`, `security-reviewer` and `docs-auditor` run on the same model.

**Gates.** Every commit went through the commit gate (`scripts/validate.sh`,
default mode: tests and the fixture matrix) and passed. `/preflight` as a whole
and `scripts/check-coverage.sh` were **not** run in this session; CI is the
authority on coverage. `/tooling-recheck` was **skipped**: the last one is
dated 2026-10-17 (`docs/research/tooling/2026-10-17-*.md`), one day earlier.

## What shipped

Nothing new runs. `THIS_BUILD_DISPATCHES_MUTATIONS` is still `false`. For a
user, one thing changed: `status` (and `summary.ownershipUnknown` in the
analysis `--json` payloads) no longer counts a row that names a mapping target
nobody installed as a row whose provider "kept no record" — on the replayed
Homebrew-only fixture the line now says 9 rows, all casks, instead of 11.

## Items

| Item | Result | Commit | Where it is recorded |
|---|---|---|---|
| A. R-20, what a read shows as active | Done. Answered for this machine, partially for states it does not hold | `2226360` + review fixes | `docs/research/R-20-what-a-read-shows-as-active.md`; checklist R-20; SE-094 addendum |
| B. `node@20` and its disable date | Done, **no data change**: the curated row is still correct, and the disable takes effect on its date through Homebrew's own load-time check (read from source; the API path read, not observed). The policy version does not move | `b0208c5` + review fixes | R-2 finding 13 and its "Re-check when" item for 2026-10-28 |
| C. Schema truth | Half done: `summary.ownershipUnknown` documented. The checked `snapshot.v1.md` sample was **not** added (below) | `e3749cb` | `docs/schemas/analysis.v1.md`; SE-097 |
| D. The ownership-unknown count | Done: installed rows only, wording kept | `e3749cb` | SE-097 ("Closed 2026-10-18") |
| E. A seam for `make_durable` | Done in the adapters; one survivor left in `interrupted.rs` | `80edf39` | this report |
| F. Stale prose | Done: spec 11's push row, ADR-0005's owner-writable paragraph, the checklist's R-11 entry | `06e1507` + review fixes | SE-099 addendum, SE-037 note |
| G. `Recommendation` with no reasons | Done for reading | `1ff8519` | SE-094 ("Closed for reading") |
| H. R-18's close-on-exec question | Done by observation, not by citation (a deviation from the item's wording, which said to leave it open if the source was not available locally) | `378a6ad` + review fixes | R-18 E7; checklist R-18; SE-015 addendum |
| I. `doctor` and a non-empty `/usr/local` (optional) | Not done: it adds a filesystem read and two schema changes, left for a pass of its own | — | SE-098 "Owed", unchanged |
| J. Skills wording (owner-armed) | Not done: not armed. The exact edits are below | — | this report |

### Why the `snapshot.v1.md` sample was not added (item C, second half)

`crates/cli/tests/documented_samples.rs` runs each sample's invocation on every
test run. `snapshot` always writes a file — into the real state directory
without `--out`, and `--out` refuses an existing file — so a checked sample
would make the test suite write on every run. Under `--replay`,
`environment.installations` is `UNKNOWABLE` anyway, so the sample would show
little. It waits for the `snapshot` change below (finding 1).

### Item J, the exact edits for an armed message

1. `.claude/skills/add-locale/SKILL.md` step 5 says to wire the catalogue in
   `Catalogue::for_locale` and that it "returns English for every tag except the
   pseudolocale". Replace it with `CONTRIBUTING.md`'s step 5 (around line 279):
   `Messages::for_locale` in `crates/cli/src/i18n/mod.rs`, a constructor beside
   `Catalogue::portuguese_brazil` in `catalogue.rs`, `pt-BR` wired.
2. Six skills use `Bash(cargo test:*)` in `allowed-tools` (`add-doctor-check`,
   `add-locale`, `add-mapping`, `add-policy-rule`, `add-provider`,
   `capture-fixture`); the other wording drifts are in
   `docs/research/tooling/2026-10-17-subagents-skills.md`.
3. Then `scripts/generate-codex-layer.sh`, whose output (`.codex/`,
   `.agents/skills/`) is guarded.

## Research findings

- **R-20** (new): answered for this machine — MacPorts' `port -q installed`
  marks the active image with `(active)` (observed); an inactive image has
  nothing after its variants (read from `action_installed`, not observed: no
  inactive image here). Homebrew's `info --json=v2 --installed` carries
  `linked_keg`, which `keg_only` does not predict (`openssl@3` is keg-only and
  linked); casks carry no such field; `list --versions` shows no linkage.
  **Unresolved and blocking slice 6 part B:** the `security-reviewer` ruling on
  moving `info --json=v2 --installed` into the run loop (it loads every installed
  formula's Ruby; the tap-Ruby and `brew.env` concerns of SE-016 item 3, SE-050
  item 6 and R-15 are unanswered because this machine has no third-party tap and
  no `brew.env`).
- **R-18** (addendum E7): close-on-exec observed on the pinned toolchain for
  plain opens and for a `run_lock`-style open. Still unread: std's source at the
  tag.
- **R-2** (finding 13): a dated `disable!` is applied at load. Observed on the
  tap path (`HOMEBREW_NO_INSTALL_FROM_API=1`); the API path, which this tool's
  reads take, read from source only. Open: whether the API payload's
  `disable_args` carries the date; re-check on or after 2026-10-28.

**Network use in this pass.** The R-20 research fetched official pages and the
providers' sources at their tags (docs.brew.sh, guide.macports.org,
raw.githubusercontent.com), recorded with read dates in R-20. Nothing else used
the network; the R-18 addendum chose not to, which is why std's source was not
re-read.

## Decisions

Made in this pass, inside the tiers:

- The ownership-unknown count takes installed rows only and keeps its wording
  (SE-097). `schemaVersion` stays 1 because the field was undocumented and 0.1.0
  is untagged; after a release the same narrowing would need a bump.
- `node@20`: no data change and no policy version bump; the finding went to R-2.
- `make_durable`'s seam is private to its module, with in-module tests, rather
  than a new Cargo feature — a new test-support feature would be one more thing
  work order 15 must keep out of a release build.
- Item I deferred; the `snapshot.v1.md` sample not added.

Reserved for the owner: none new in this pass. No ADR was written; ADR-0005
gained a dated update.

## Specification changes

Indexed; each is in `docs/spec-evolution.md`:

- SE-097: the count closed, with the `schemaVersion` reasoning.
- SE-094: the `PROVIDER_REPORTS_ACTIVE` owed note answered by R-20 (a pointer);
  the `Recommendation` read rule closed.
- SE-099: spec 11's push row, now honest about being a hook-only tripwire.
- SE-037 and SE-015: the checklist's R-11 and R-18 entries.
- `docs/passes/08-pre-flip-design-notes.md`: Part B corrected by R-20 (keg-only
  does not mean unlinked; the inactive row is read, not observed).

Rejected: none.

## What it does not do

- It does not make `PROVIDER_REPORTS_ACTIVE` evaluable (slice 6).
- It does not close SE-097's other owed line: a `MAPPED` destination that is not
  installed has availability `UNKNOWN` and `disabled: false`, so after
  2026-10-28 a MacPorts `nodejs20` mapping to `node@20` will not be reported as
  pointing at a disabled formula. Safe (`UNKNOWN` never acts), still owed.
- It does not fix finding 1 below.

## Tests and coverage

New tests, each shown to fail with its change reverted:

- `a_mapped_row_that_is_not_installed_is_not_an_ownership_gap`
  (`crates/application/tests/summary_evidence.rs`).
- `a_recommendation_with_no_reasons_is_refused_when_read` and a round-trip
  control (`crates/domain/src/recommendation.rs`).
- Five `durable_tests` in `crates/adapters/src/persistence/history_store.rs`;
  the slice 5 survivors (comparison removed, directory sync dropped, sync
  skipped, second read replaced by the first) are now killed.

Still surviving: "clear the marker before `make_durable`" in
`crates/cli/src/app/interrupted.rs`, which needs a seam there.

No invariant gained or lost a test. Coverage was not measured in this session.

## CI

Not run by this session; the branch is pushed for CI to run on its pull request.

## Known limitations and risks discovered

1. **`snapshot` and `doctor --report` ignore `--dry-run`, and under `--replay`
   `snapshot` writes into the live state directory** (`security-reviewer`, M1;
   `crates/cli/src/app/mod.rs`, `context.rs`). The global `--dry-run` help says
   "Show what would be done and run nothing … Every command but `migrate --yes`
   is a dry run in this build anyway", which is false for both. It breaks no
   MUST (neither command is mutating or destructive in 06's sense) but breaks a
   user-facing promise and the idea that `--replay` leaves the machine alone.
   **This session tripped it:** running `--replay … snapshot` without `--out`
   (once with `--dry-run`) wrote
   `snapshot-20261019T015001Z.json` and `snapshot-20261019T015009Z.json` into
   `~/Library/Application Support/macportsbrew/snapshots/`. They hold a replayed
   fixture, not this machine; nothing names them, so they are inert. The owner
   may delete them; this session did not.
2. The curated `node@20` row's `source` says "local homebrew-core" without
   saying whether the API loader was off; a data correction would move the
   policy version (spec-guardian, finding 1).
3. R-20's inactive-row rule and several of its Homebrew states rest on one
   machine with no third-party tap.

## Recommended next tasks

- **A small focused pass (03 template) for finding 1:** `snapshot --dry-run` and
  `doctor --report --dry-run` write nothing and say what they would write;
  under `--replay` or `--from-snapshot`, `snapshot` needs `--out`; correct the
  `--dry-run` help; a test that the state directory is unchanged; an SE entry.
  Then add the checked `snapshot.v1.md` sample.
- **Pass 08 slice 6** can start Part A now; Part B waits for the
  `security-reviewer` ruling R-20 names.
- **Work order 15** after the owner's rulings on DP-033/034/035/045.
- Item I, and the `interrupted.rs` seam, in a later read-only pass.
