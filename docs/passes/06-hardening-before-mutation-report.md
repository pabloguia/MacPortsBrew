<!-- SPDX-License-Identifier: Apache-2.0 -->

# 06-HARDENING-BEFORE-MUTATION — pass report

**Date:** 2026-10-02 to 2026-10-03 · **Work order:**
[`../../prompts/06-hardening-before-mutation.md`](../../prompts/06-hardening-before-mutation.md) ·
**Branch:** `pass/02-architecture-review` · **Commits:** `02034fd`..`4bcba71`,
seven, on base `18b623a`

**Preflight, 2026-10-03.** `.claude/hooks/stop-gate.sh` exits 0 and
`scripts/validate.sh` in default mode passes on the final tree: every stage,
803 tests, 0 failed, and the fixture matrix's 198 checks over 7 fixtures.
`scripts/validate.sh --full` (the coverage floors) was **not** run; see Tests.
Section 3 of the work order is demonstrated, not asserted: `git diff 18b623a
HEAD -- Cargo.lock .claude scripts .codex .agents .github` is empty (`Cargo.lock`
sha1 `48666da4…dce` before and after); `crates/domain/src/plan.rs`, the
execution gate and both gate test files have no diff in the range;
`THIS_BUILD_DISPATCHES_MUTATIONS` is `false`; `PERMITTED_PROGRAMS` is
byte-identical; `PERMITTED_SUBCOMMANDS` only narrows; DP-025 and DP-026 are
untouched. `MPB_SKIP_STOP_GATE` was never used. **The `Stop` gate blocked four
turn ends during the pass**, none with a defect in committed code: once on a
test an implementer was mid-way through writing, twice while the catalogue and
its sample were being edited out of step, and once because the disk filled
(scratch builds for mutation testing; cleared, and every affected run
repeated). Each
time the work was finished and the gate re-run green before any commit.
`.agents/skills/tooling-recheck/` did not reappear.

**Two protocol breaches, both by the research subagent, both disclosed in
[R-15](../research/R-15-homebrew-brew-env.md) E6/E7 and left for the owner.**
One `brew config` was run with the real `HOME` before the note moved to a
temporary one: Homebrew downloaded its catalogue and wrote about 30 MB under
`~/Library/Caches/Homebrew` (files left in place). And one run blanked
`HOMEBREW_NO_ANALYTICS` against the real Homebrew repository, whose
`homebrew.analyticsdisabled` is unset, so it **probably sent one Homebrew
analytics event** (inferred from source; no traffic was captured). The work
order says writing is a failure; that is what these were.

## What shipped

Nothing a user runs changed what it does to the machine: this build still
dispatches no mutating step. What changed is what it refuses and what it says.

- **A provider read can carry only the options this tool asks for**
  (`83f364d`). Each read subcommand has its own exact list of long options;
  `brew info --github`, another value of a listed option, an abbreviation a
  provider would resolve and a bare `--` are refused at construction and at the
  spawn site, for both providers.
- **Hostile text has one definition** (`5433a71`). Control, Unicode format and
  line-separator characters are classified once in `domain::text`; a snapshot
  refuses controls and keeps the rest, and every human rendering escapes all
  three. Review found, and this commit closed, a **terminal-escape injection
  through a provider id in a crafted snapshot** that `analyze` and `list`
  printed raw.
- **`plan` and `analyze` say whether the input may be inconsistent**
  (`c1c84e1`), as `migrate` already did: `concurrentProviderActivity` in
  `analysis.v1` (so `status --json` and `list --json`) and `plan.v1`, and a
  closing sentence in the human output.
- **A weak mapping is no longer "only a None match"** (`1f01347`): "No
  equivalent under macports reaches confidence level Strong: the best is at
  level None."
- **The documentation stops promising what Homebrew does not do** (`f400ca8`,
  `4bcba71`): README, SECURITY, wiki, CLI help, CHANGELOG and the
  specification now say the tool makes no network call of its own, and that a
  Homebrew read it runs fetches the catalogue on an empty cache and can be
  un-quieted by a `brew.env`.

## Research findings

| Item | Status | Note |
|---|---|---|
| R-15 · Whether a `brew.env` can undo the quieting environment | **Answered** 2026-10-02 | [`R-15-homebrew-brew-env.md`](../research/R-15-homebrew-brew-env.md). Yes: up to three files are exported after the environment `brew` is given, an empty value deletes a key, nothing switches them off (Homebrew 7.0.4). Also: `HOMEBREW_NO_AUTO_UPDATE` does not keep a Ruby read offline on an empty cache. |
| R-15 residuals | Unresolved | Whether a substituted API domain or curl configuration can get altered catalogue data accepted; what a Ruby read does on an empty cache with no network (bears on "`analyze` works with no network"). |
| R-16 · Whether a MacPorts user configuration can outrank the invocation | **Opened**, unresolved | `09-research-checklist.md`. Blocks DP-027's mechanism being one provider capability rather than a Homebrew special case. |

The tooling recheck re-read two of three groups
([`tooling/2026-10-02-*.md`](../research/tooling/)): no contract drift; one
nuance recorded (SE-027). **The hooks, permissions and CI group was not
re-read**: the auto-mode classifier refused that subagent as data exfiltration,
and the refusal was not routed around. Its facts stay dated 2026-09-27.

## Decisions

Made here:

- **S2's option lists are a permission list, not provider data** — what this
  tool asks for carries no `source`/`read_on` (SE-050).
- **The snapshot refuses controls only; the renderer escapes everything**
  (SE-092 amendment (a)). Refusing format characters would refuse correct
  machines (ZWJ, ZWNJ, soft hyphen in directory names). Reported variable
  *values* are kept and unbounded beyond the document's own limit — two earlier
  versions that refused or bounded them were reversed on review for the same
  reason.
- **`plan --json` gains the flag through a CLI document, not `MigrationPlan`**
  (SE-015), which the gate and run loop share.
- **The confidence parameters carry identifiers; Debug is not a wire format**
  (SE-064). `plan --json` reason params change spelling `None` → `NONE`,
  recorded as a fix.
- **No environment mechanism for `brew.env` was built in S2** — nothing a
  variable can set outranks a file.

Reserved for the owner, with the reversible default taken:

- **DP-027 (new)** — how to answer a `brew.env` that outranks the quieting
  environment. Default: disclose, change no behaviour; no mutating step relies
  on that environment (none is dispatched). It names two questions above the
  specification: the charter's "no telemetry"/"no network access" non-goals
  (Tier 2) and INV-10's controlled environment (Tier 1).
- **DP-025, DP-026** — untouched, defaults in force.

No ADR was written or changed.

## Specification changes

All recorded in [`../spec-evolution.md`](../spec-evolution.md):

- **SE-050** — the per-subcommand long-option allowlist; closes the long-option
  half of S4 "Not closed" (1); "permitted, not required" recorded.
- **SE-092** end-of-pass amendment — (a) closed (one hostile-text definition,
  the provider-id and `reported`-name checks, the JSON risk stated); (c)
  answered (R-15).
- **SE-016** item 3 — the Homebrew half of quieting is a request, not a
  guarantee; the cold-cache fetch is a **gap, not accepted** (SE-050 item 6's
  premise corrected); the public surfaces qualified; F-21's `=0` observation put
  in doubt.
- **SE-015** — `plan` and `analyze` carry the flag; `explain` and `doctor` do
  not yet, recorded.
- **SE-064** — the confidence parameters and the sentence; the `{class}` and
  `{state}` siblings recorded, not fixed.
- **SE-027** — `permissionMode` is ignored under some parent modes.
- **SE-051** — pointer to the `=0` doubt and the `brew.env` limit on the
  recorded environment.
- Tier 3 MUSTs amended in place: `04` (Process execution, Threat model), `05`,
  `06` (both network MUSTs, now qualified rather than relaxed).

**Rejected, so they are not reopened:** refusing Unicode format characters at
capture (refuses correct machines); refusing or bounding `reported` values
(same); adding the flag to `MigrationPlan` (it is the gate's contract, and
`migrate` would carry it twice); setting `XDG_CONFIG_HOME` or `HOME` to an empty
directory to suppress the user's `brew.env` (moves the user's tap-trust store,
leaves two of three files open — DP-027 (a)); the first two wordings of the
mapping sentence ("only a None match"; "has confidence None", which reads like
a null and asserts a mapping that does not exist).

## What it does not do

- Dispatch any mutating step; the gate refuses every plan this build produces.
- Pin a plan read's **positional** arguments at the spawn site — only the gate
  does (SE-050).
- Require a listed option to be present: `brew info <name>` without
  `--json=v2` passes the spawn site; nothing builds it (SE-050).
- Escape `--json` beyond JSON's rules (except `migrate`'s `gateRefusals`); a
  consumer that renders display fields must escape format characters itself.
- Read every snapshot display field in the load check (SE-092 lists the rest;
  all are escaped when shown).
- Detect a `brew.env`, or keep a Homebrew read offline on an empty cache.
- Carry the concurrency flag in `explain` or `doctor`.

## Tests and coverage

803 tests pass under `validate.sh`. New tests by slice are named in their SE
entries; every new rule was proved by mutation in a scratch copy with its own
target directory, restored by rewrite and `touch`, and compared with `diff -r`
(identical in every file the slice touched). One mutation run of S3 hit the
full disk and was discarded and re-run.

Invariants this pass touched, by test:

- **INV-05** — `invariant_05_a_provider_is_never_updated_or_upgraded_from_the_spawn_site` (unchanged, green).
- **INV-10** — `invariant_10_every_program_an_adapter_names_is_on_the_allowlist`, `invariant_10_a_plan_read_never_adopts_the_environment_it_carries` (unchanged, green). The `brew.env` gap cannot be tested from fixtures; it is DP-027's.
- **INV-14** — the four `invariant_14_*` scans cover the new domain modules.
- **INV-15** — new: `invariant_15_a_replayed_plan_or_analysis_cannot_observe_this_hosts_processes`, `invariant_15_a_plan_or_analysis_from_a_snapshot_file_is_a_function_of_the_file`, `invariant_15_a_hostile_reported_name_is_refused_identically_by_capture_and_by_load`, `…_at_capture_and_at_load`; `invariant_15_reproducible_recommendations_end_to_end` green.
- **INV-17** — the twelve `invariant_17_*` tests green; S2 is one table read by one function.
- **INV-20** — `invariant_20_no_provider_deadline_is_written_into_source` green (U+2028/9 written in decimal).
- **INV-02 / §3** — `every_plan_this_planner_builds_is_refused_for_execution_in_this_build` and its fixture counterpart, unmodified and green.

**Coverage floors were not measured**: `validate.sh --full` was not run, so the
95% floor for planning, verification and safety logic is asserted by nothing in
this report.

## CI

Not run: the repository has no remote (DP-007; `git push` is denied). Everything
above is the local gate on one Intel Mac (macOS 26). CI's architecture matrix
has not seen this range.

## Known limitations and risks discovered

- **A `brew.env` governs `brew`, not this tool**, and the charter and INV-10
  questions it raises are the owner's (DP-027).
- **On an empty Homebrew cache, analysis writes ~30 MB and needs the network**;
  untested with no network (R-15 residual). `--no-network` does not prevent it.
- **The `{class}` and `{state}` reason parameters are still Debug-spelled** in
  the planner: `plan` on a cask prints "classed as GuiApp" (SE-064).
- **`ConcurrentProviderActivity` has no fallback for an unknown value**, so the
  detector that adds one owes it (SE-015).
- **`error.provider_not_found`** tells the user to run `macportsbrew
  providers`, which is not a command: `locales/en.toml` line 49, and no such
  subcommand in `crates/cli/src/app/mod.rs` (checked 2026-10-03). Pre-existing.
- **The top-level error path does not redact the home** in `--verbose` detail
  (security review of S3, out of its scope).
- Pass-05 limitations **closed here**: the read allowlist accepting any long
  option (its long-option half); `is_safe_provider_text` and `escape_controls`
  disagreeing; the "None match" wording.
- Assumptions to attack: that a Unicode 16.0 Cf table is the right boundary
  for "hide" (default-ignorable characters outside Cf are not covered, stated);
  that a reason parameter's spelling is not a schema change.

## Recommended next tasks

1. **The owner's guarded edits** — still owed from pass 05 and grown:
   `.claude/rules/executor.md` and its `CLAUDE.md` entry; `migrate` in
   `run-fixture-matrix.sh`; `.claude/rules/adapters.md`'s "does the offline work"
   sentence (SE-016 item 3); `.claude/README.md`'s verification date and its
   language-server citation (tooling note); a wording check of the skills'
   `allowed-tools`, which pre-approve rather than restrict (SE-027). *Owner.*
2. **Re-run the hooks, permissions and CI tooling group** with the owner present
   or with the classifier's permission. *Maintenance.*
3. **Rule on DP-027, DP-025 and DP-026.** *Owner.* DP-026 and DP-027 block the
   mutating pass.
4. **R-16**, MacPorts user configuration. *Research, `provider-researcher`.*
5. **R-15's residuals**: the empty-cache, no-network case; whether altered
   catalogue data can be accepted. *Research.*
6. **The `{class}` and `{state}` parameters** through `labels`. *Maintenance.*
7. **The concurrency flag in `explain` and `doctor`.** *Focused implementation.*
8. **Coverage measured** with `validate.sh --full`. *Maintenance.*
9. **The mutating half of the executor** — obligations 8 and 12, history store,
   lock and detector, `--retain-source` re-run, a MacPorts-to-Homebrew
   recording that yields a plan, a required-options entry for any new read
   path, and the positional half of the spawn-site pin. *Focused
   implementation, its own reviewed pass*, after 1–4.
