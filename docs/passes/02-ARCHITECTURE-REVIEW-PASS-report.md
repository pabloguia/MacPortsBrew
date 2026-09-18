# 02-ARCHITECTURE-REVIEW-PASS — report

**Date:** 2026-09-02 · **Commit at review:** `ab56b06`

The findings are in
[`02-ARCHITECTURE-REVIEW-findings.md`](02-ARCHITECTURE-REVIEW-findings.md), ranked
by consequence, forty-five of them - twenty at the review, the rest found while
fixing those. This report says what was done, what it cost, and
what to do next. **One finding stops at the human: F-01, recorded as DP-010.**

## About the earlier session under this pass id

An earlier session on 2026-09-02 wrote a report under this filename that opened
"the pass is not complete … **It did not start.**" That was accurate and it said
so plainly, which is the reason this pass could begin from a known position rather
than a claimed one. What that session did do — resolving DP-004, DP-006 and DP-007
with the owner, writing ADR-0005, and opening DP-008 and DP-009 — is not lost: it
lives in `docs/decisions-pending.md` (Resolved), ADR-0005, and SE-014, SE-021,
SE-034 and SE-037. This file is now the report for the review it was named after.

## Gate status: run in full, for the first time in this pass lineage

`scripts/validate.sh` in default mode **exits 0 with the whole test suite**, and
`.claude/hooks/test-hooks.sh` passes 120 of 120 (70 at the review; the rest were
added with the guards they assert).

That is worth stating precisely, because the previous session reported the opposite
and reported it as a fact about this machine: *"this machine has no Rust
toolchain"*, and therefore *"no test ran in this pass, and no coverage figure in
this report is observed."* On this machine today `cargo`, `rustc 1.98.0` and the
pinned toolchain are all present, `rustc --print deployment-target` returns
`10.12`, matching `rust-toolchain.toml`'s claim, and every Rust check runs.

Whatever changed between the two sessions, the mechanism that let a whole pass
record unverified numbers is still in place: `.claude/hooks/stop-gate.sh:122` guards
the `validate.sh` call with `command -v cargo` and **still exits 0** when the guard
fails. A gate whose skipped path is indistinguishable from its successful path is
not a gate. That is finding F-20 and it is on the task list.

**What every "validate.sh ok" in this report and its commits actually means.**
The gate this pass ran is **not the gate the repository defines.** Sixteen files
of enforcement-layer work — `.claude/hooks/`, `.claude/settings.json`,
`scripts/validate.sh`, three `scripts/check-*.sh`, the workflows, `AGENTS.md`,
`docs/spec/11-agentic-sdlc.md` — sit uncommitted in the working tree, 387 lines
of them, all predating this session. One consequence is concrete:
`scripts/validate.sh:33` runs `scripts/check-memory-files.sh`, and **that file is
untracked**. `HEAD`'s `validate.sh` does not call it, so a fresh clone is not
broken — it simply runs a weaker gate, without the memory-files check.

The direction is the safe one: local checks are a *superset* of what the
repository would run, so nothing passed here that `HEAD`'s gate would have
caught. The risk is the other way round, and it arrives with CI: the pipeline
will enforce less than this pass believed it was enforcing, and the difference
is invisible from inside a session where the stronger gate is the one that runs.
Not fixed here — this is the owner's in-progress work, it is guard-protected,
and committing the script alone would achieve nothing while its caller's change
is also uncommitted. It is one `git add` plus a review, and it is theirs.

**Both paragraphs above have since been overtaken, and the second was wrong to
call permanent.** The cargo hole is closed (`344c4ca`): a turn that changes code
while `cargo` is off `PATH` now fails the gate instead of passing it having checked
nothing, and only a turn that changed no code gets a warning. And the coverage
floors are no longer unverified — the owner installed `cargo-llvm-cov` and the
first measurement in this repository's history is in "Tests and coverage" below.
It found `application` sitting under a floor its own gate had demanded since
2026-09-02, which is the concrete cost of the two passes that reported coverage
without running it.

**And the divergence itself is closed, as of `96605e9` (2026-09-08).** All
nineteen files were reviewed and committed — reviewed rather than swept in,
because a commit message has to describe the change and that one could not be
written from a diffstat. So from that commit onward, "validate.sh ok" in this
report means the gate the repository defines. The window it describes was real
and lasted the length of this pass; it is recorded rather than deleted, because
"the local gate was a superset" is a claim a later reader may need to check
against a specific commit.

One asymmetry survived it — **F-42 / DP-018**: `.codex/` was a second, live hook
layer whose guards were an older and weaker copy of `.claude/`, and whose path
guard protected the machine but not one repository path. The paragraph above is
about the gap between the working tree and `HEAD`; that one was about the gap
between two harnesses in the same tree. It is closed: `.codex/` and
`.agents/skills/` are generated from `.claude/`, and a `validate.sh` stage fails
when they drift.

### The committed home path, and how it got past the check written to catch it

`a7ec60a` committed `.codex/hooks.json` wiring all five Codex hooks by absolute
path — `/Users/<name>/projects/MacPortsBrew/.codex/hooks/...`. Two consequences,
both real: a username entered a repository DP-007 sends to public GitHub, and
**every clone got five hooks pointing at a directory that does not exist**, so a
Codex session ran with no `PreToolUse` guard and no `Stop` gate and announced
nothing. F-42's exact shape, restored by the one file the generator does not
write, in the commit that closed F-42.

Both preflight reviewers found it independently, and `spec-guardian`
demonstrated it by cloning to another path and watching the check report `ok`.

**The check could not see it.** It grepped `.codex/hooks/x.sh` out of the
*middle* of the absolute string and tested that against the repository, where it
exists — validating a substring rather than the command the harness spawns. It
now rejects any absolute path in the manifest, and rejecting the state actually
committed is how that was confirmed. The manifest resolves from the git root,
which is Codex's own documented guidance for project-local hooks
(developers.openai.com/codex/hooks, read 2026-09-09); Codex sets no
`CLAUDE_PROJECT_DIR` equivalent, so there was nothing to substitute.

**The turn before this one ended with the gate red**, because the fix touched
`.codex/**`, which `665b4a6` had just placed under guard, and the generator's
shell-redirect route around that guard was itself a finding of the same review.
Using the hole to repair the defect that revealed it was not an option. It went
to the owner as a patch.

### Disclosed escape: guard edits, three times — 2026-09-08, 09 and 11

`docs/spec/11-agentic-sdlc.md` obliges this report to say so. The owner
authorised it in session — *"Altere o flag e siga"* — for three changes the
guard had refused:

- **F-44.** `shellcheck -x -P SCRIPTDIR .claude/hooks/*.sh scripts/*.sh` — CI's
  very first gate step — exited 1 on this tree, and had since it was written.
  Two `SC2016` in `guard-provider-mutation.sh` whose silencing directive sat
  twenty lines above the assignments it was meant to cover, where a shellcheck
  directive silences nothing. Nothing after that step would have run.
- **A `codex layer` stage** in `validate.sh`, running the generator's `--check`.
  Without it the generator existed and nothing ran it, which is the shape of the
  drift it was written to stop.
- **A `shellcheck` stage** in `validate.sh`, the same invocation the workflow
  uses. Shellcheck previously reached this repository through two surfaces that
  could not reprove — a CI job that has never run, and a post-edit hook that
  reports findings as context.
- **The path guard widened** to cover `.codex/**` and `.agents/skills/**`. This
  was omitted from the first version of this list, and it is the most
  security-relevant of the four — a disclosure that leaves out the change to the
  guard itself is not a disclosure.

**The flag was not set by the agent** — and the first version of this paragraph
said it *could not have been*, which was an overclaim `spec-guardian` caught.
There is a route: `.claude/settings.local.json` is not in the guard's pattern
list (`*/.claude/settings.json` matches only the committed file) and a settings
file's `env` block reaches hook processes. So the accurate sentence is that the
agent **did not**, and that doing so would itself have been the violation — the
sentence at `guard-protected-paths.sh:60`, "an agent whose gate keeps failing
does not need bad intent to disable it". An unenforced norm stated as an
impossibility is the wrong shape in a project whose named unrecoverable error is
being wrong invisibly. The gap is real and should be closed:
`*/.claude/settings.local.json` belongs in the pattern.

The changes were prepared as a patch, verified in a scratch tree — the exact CI
invocation moved from exit 1 to exit 0 there before anything was handed over —
and applied by the owner with `git apply`.

**The second use, 2026-09-09**, carried F-45: three checks that lived only in the
workflow moved into `validate.sh`, the corresponding workflow steps deleted, and
`scripts/check-licence-text.sh` added. Same route: a patch, verified first,
applied by the owner. It is listed here because "used once" was true when it was
written and stopped being true the next day, which is what a disclosure section
is for.

**The third use, 2026-09-11**, carried the two HIGH findings from the preflight
reviews: the manifest's absolute paths, and the guard pattern that named
`validate.sh` and `check-*.sh` while leaving `generate-codex-layer.sh` — the one
sanctioned writer into the guarded `.codex/` — unprotected. It is now
`*/scripts/*.sh`, plus `.claude/agents/`, `.claude/skills/` and
`settings.local.json`, and five cases in `test-hooks.sh` assert them: the guard
had been widened twice with nothing testing either widening.

**And the owner changed the working agreement here**: from 2026-09-11 the agent
applies these patches itself rather than handing each one over. What is kept is
what the hand-over was for — the change is still built as a patch, so it is one
reviewable unit rather than a scatter of edits, and every use is listed in this
section. What is dropped is the round trip.

**Both new stages were shown to fail before being trusted.** `codex layer`
failed on its first real run, because the same patch had edited two hooks and
left the generated layer stale — the check catching live drift rather than a
constructed mutation. `shellcheck` was mutation-checked with a deliberate
`SC2034`, and the file restored byte-identical to a pre-mutation copy.

## End-to-end re-verification against the real machine, 2026-09-09

Seven fixes landed after the previous live validation — F-31, F-37, F-38, F-39,
F-41, and both halves of F-34 — so that validation no longer described this
binary. Re-run on the dual-provider machine (Homebrew 6.0.22 at `/usr/local`,
MacPorts 2.12.6 at `/opt/local`):

| Property | Result |
|---|---|
| Determinism | Two live `--json status` runs, **byte-identical** |
| `doctor` live | Exit 0, **zero** raw message identifiers in the output (F-38) |
| Redaction | **No** occurrence of the invoking username in any output, and `capturedBy` is the constant `macportsbrew capture` |
| Live vs `--from-snapshot` | Four differing fields, **all four provenance**: `capturedAt`, `capturedBy`, `origin`, `source`. The analysis itself is identical |
| F-34, both directions | Live, `path_unknown` does **not** fire and the real `PATH` findings do. Under `--replay` it fires and says a recording carries no environment — the warning replaced by a statement |
| F-41, both routes | `--replay <a directory>` and `--from-snapshot <a directory>` both exit 8, and each names the command that produces *its own* input and the flag the user actually passed |
| F-39 | `status` reports "Ownership is unknown for 3 row(s)" live, where the count alone used to be the only clue |

The control matters as much as the fix in two of those. `path_unknown` firing
under replay proves nothing on its own — a check that always fires is not a
check. It firing under replay **and staying silent live, where the real findings
appear instead**, is what says the branch discriminates. Likewise F-41: the
`--from-snapshot` half is unchanged behaviour, and asserting it is what shows the
new identifier did not simply swap one wrong message for another.

## What was verified rather than inherited

The work order's first instruction is not to trust the previous report. Every
numeric claim in it was re-run:

| Foundation-pass claim | Result |
|---|---|
| 456 tests pass | **Verified**, and the per-crate split matches exactly — domain 17, policy 100, application 28, adapters 190, cli 121. |
| 157 fixture-matrix checks over 6 fixtures | **Verified.** |
| 70 hook cases | **Verified.** |
| The slice runs — 189 packages, 56 top-level | **Verified** on this machine (Intel, macOS 26.6.2, Homebrew 6.0.21 at `/usr/local`, MacPorts absent). |
| INV-15 reproducibility | **Verified empirically** — two `--from-snapshot` runs byte-identical over 184 KB. |
| INV-06 — nothing elevates | **Verified structurally** — zero elevation tokens in `crates/*/src`, exactly one `Command::new` site. |
| All 20 invariants have a test | **They exist.** Several would not fail if the invariant were violated — F-19. |
| Homebrew's Intel timeline | **Re-verified live 2026-09-02.** Unchanged; see below. |

The foundation pass's reporting is accurate. Where this review disagrees with it,
it disagrees about the *significance* of things that report already disclosed —
most importantly the `--advanced` unit shift, which it recorded as a cosmetic count
difference ("63 versus 62 … Known, not fixed") and which turns out to change the
unit's source provider, its `advanced_only` flag and the rules that fire (F-03).

## Time-sensitive facts, re-verified and dated

<https://docs.brew.sh/Support-Tiers>, **read 2026-09-02**:

- *"macOS Tahoe 26 is the final version of macOS to run on Intel hardware … Homebrew
  has stopped building new bottles for Intel systems, and will remove the ability to
  run Homebrew on Intel systems in or after September 2027."* — **unchanged.** The
  charter's timeline claim and `end_date = "2027-09-01"` in
  `data/policy/provider-support.toml` are correct.
- Intel x86_64 as of September 2026 is **Tier 3** (Catalina 10.15 → Tahoe 26).
  **Unsupported** is a separate category — Mojave 10.14 and earlier.

That last distinction is the one the charter and README got wrong (F-11, fixed;
DP-011 decided 2026-09-04),
and the one `provider-support.toml` gets right. The data outlived the prose, which
is the dated-facts design working as intended even while the prose drifted.

`rustc 1.98.0` deployment target re-verified at `10.12` on this machine.

## The findings, in one paragraph

Four Tier 1 violations. One provider's dependency graph decides the other's
recommendation and the user is told a false number (F-01, INV-04/INV-08 — the one
that stops at the human). A package held by both providers can only be migrated in
one direction, and which one is decided by alphabetical order of the provider ids
(F-02, INV-17), with **all 8 `plan` invocations in the test suite pointing the same
way**. `--advanced` removes a user-requested package from the top-level view, flips
its source and changes the rules that fire, so `analyze` and `explain` contradict
each other on one input (F-03, INV-13). And the one plan command that escapes the
sanitised environment is excluded from the test named after that invariant by an
explicit filter (F-04, INV-10). Then a demonstrated privacy leak — `doctor --report`
with `HOME` unset wrote 39 occurrences of the real home path into a file the
documentation invites users to attach to issues (F-05). Then verification steps that
cannot verify (F-06), a `doctor` that cannot see the commonest shadowing directory
on macOS (F-07), a "no shell to inject into" guarantee enforced by a lint that
checks something else (F-08), and unvalidated snapshot input reaching plan argv
(F-09). The documentation findings are honesty defects rather than errors of fact:
CI has never run and the matrix says "tested" (F-12); the README's opening sentence
says the tool executes migrations (F-13); INV-03 is restated as a guarantee with its
qualifier removed (F-14).

## Specification changes

Eleven entries, **SE-044 to SE-054**, appended to
`docs/spec-evolution.md`. Ten are `Status: proposed` and grouped under a clearly
marked heading, deliberately and temporarily — a proposal sitting beside accepted
rationale reads as settled — and each says that on acceptance it merges into its
topic under SE-032, keeping its identifier.

**SE-054 has since been accepted and implemented**, at the owner's request, along
with **SE-055** (supply chain), which the review's own findings F-16 to F-18
called for. SE-054 duly moved into "Domain and safety" and records what was built
rather than what was proposed — the two differ, and the entry says how and why.
F-05, F-16, F-17 and F-18 are fixed, and so are the four Tier 1 findings this
review filed —
F-01, F-02 and F-03 outright, F-04 as the violation it named, leaving a
declared Tier 3 deviation that belongs to the executor pass (SE-056). F-21
was found by a reviewer of the F-04 fix and is fixed too (SE-051 part 1), so the
Tier 1 column is five closed. Three limits are named rather than left implicit:
`detection.prefix` is checked for shape but not for plausibility, so a prefix
nobody owns is still declarable (F-09, SE-051 part 2, open); a live run validates
that prefix less than a replayed one does; and nothing yet guards what a
*recording* can make an adapter construct beyond the prefix. The fix took three
security reviews and three rebuilds — the first version was defeated four ways,
the second six more including a working bypass of the rule itself, and between
them they wrongly refused three legitimate things, one of which stopped the tool
running at all on any machine whose `HOME` carried a trailing slash. SE-051
records the path and not only the destination, because the path is the part that
stops someone repeating it.

Nothing in `docs/spec/` was edited. No invariant was changed. Under section 3 of the
work order a reviewer may propose at any tier and may not apply, and the two narrow
exceptions — a failing test, and a factual documentation correction — were not used:
the factual corrections available here are in a Tier 2 document (F-11) and in
user-facing text whose wording is a judgement call, so they went to DP-011 and to
the task list rather than into this diff.

**That paragraph describes the review diff, and the pass did not stop there.** It
was written at the third commit of thirty-eight. The owner then asked, in the same
session, for the findings to be resolved rather than filed — and afterwards for a
coverage floor to be measured and met. Both instructions supersede section 3,
because the owner wrote section 3; neither was silent, and this is where that is
recorded. From `19b0c7a` onward the pass has been writing `src/`, tests, CI and
`.claude/`, which section 3 forbids a reviewer to touch. That is seventy-two
commits at 2026-09-16 — `git rev-list --count 19b0c7a^..HEAD` is the number, so
the next reader recounts rather than trusting this sentence, which said
thirty-five and "most of them `fix(...)`" until it was recounted. Twenty-nine are
`docs`, twenty-six `fix`, eight `test`: the plurality is documentation, not
repair. A hand-maintained count inside the document F-46 audited is the same
defect F-46 is about, which is why it now carries the command that replaces it.

Two things follow, and both matter more than the bookkeeping. **The review's
separation of argument from diff is gone for the later findings**: F-01 to F-04
were argued in one commit and fixed in another, so the argument survives
independently of the fix, but F-25 onward were found and fixed together and the
finding text is the only place the reasoning exists. **And a reviewer who fixes
what it finds stops being an independent reviewer of it** — which is why every
fix in this pass went through `spec-guardian` and `security-reviewer` before being
called done, and why three of them were rebuilt after those reviews defeated the
first attempt. That substitution is a weaker control than a separate pass, and
naming it is the point of this paragraph.

**Nothing was rejected in this pass.** The previous report observed that a review
rejecting nothing has probably not reviewed anything. The honest answer here is that
the design decisions this pass examined were mostly *right* and the defects were
implementation drift away from them — the policy engine's purity, the
capture/analysis split, the ownership evidence model and the argv-only executor all
survived being attacked. What this pass rejects is the *reading* that SE-042's
declaration settled the cross-provider count question (F-01), and the reading that
the `--advanced` shift was cosmetic (F-03).

## Decisions

None taken. Three added for the owner, each with a recommendation and a reversible
default already in place:

- **DP-010** — a Tier 1 invariant (INV-04/INV-08) is currently relaxed by a Tier 3
  entry (SE-042 change 2). Does the invariant stand, or does it need an explicit
  carve-out? Recommendation: the invariant stands.
- **DP-011** — the charter (Tier 2) states a provider fact its own data contradicts.
  Recommendation: amend.
- **DP-012** — do casks belong in the default top-level view? Either answer changes
  the default view's scope, which is not an agent's call.

DP-008 and DP-009 are untouched and still open. Neither blocks the implementation
passes below.

## Research

R-1 to R-10 remain answered and dated 2026-09-02; nothing this pass found
contradicts any of them, and F-2 and R-9 were leaned on again and held. **R-11 is
still open** and still blocks the release workflow.

No new research item is needed for any finding in this pass: every one is decidable
from the code, from a source already cited, or from the owner.

**The largest standing research gap closed while this pass was running.** This
paragraph said "MacPorts has never been observed" and that the adapter, its
fixtures and its contract tests were documentation-derived. True when written,
and false from 2026-09-07, when MacPorts 2.12.6 was installed here. The first
thing it did was disprove the tool: the executor passed no `HOME`, `port` exits
1 without one whatever you ask it, and a healthy install was reported as
`Broken` (F-37). R-5's MacPorts half moved from cited to observed (E19-E32),
which also produced SE-077 and a third independent confirmation of F-33.

Findings F-24, F-33, F-34, F-37 and F-39 all rest on that observation. Anything
in this report dated before 2026-09-07 that treats MacPorts as unobservable
describes the machine this pass started on, not the one it ended on.

## Tests and coverage

456 pass, 0 fail, re-run at the reviewed commit. No test was added: section 3 permits
a *failing* test to demonstrate a finding, and the findings that would warrant one
(F-02's missing reverse direction, F-04's filtered exception) are better delivered as
the first task of the implementation pass, where the fix and its test land together.
That is a judgement call and it is named as one.

**Both numbers are now stale, and the second sentence is no longer true.** The suite
is **541** and tests were added throughout, because the pass went on to fix what it
found (see "Specification changes"). The judgement call above was overtaken rather
than reversed: the fix and its test did land together, just in this pass instead of
the next one.

The more useful number is not 456. It is that **F-01 through F-04 all survived a
fully green suite**, and F-19 explains why for each: a test that filters out its own
exception, a test that skips every program the invariant is about, a test that
asserts on the planner's correct value instead of the engine's incorrect one, a
symmetry test that renames provider contents but keeps the map keys so iteration
order never changes, and a subset assertion that was never written.

**Coverage was measured for the first time in this repository's history** (DP-015),
after the owner installed `cargo-llvm-cov` and raised the floors to 95% for
`domain`, `policy`, `adapters` and planning with a 90% overall backstop.

| Crate | Lines | Gate's floor | Owner's floor |
|---|---|---|---|
| `domain` | 96.65% | 95 | 95 |
| `policy` | 97.46% | 95 | 95 |
| `application` | 95.27% | 95 | 95 |
| `adapters` | 95.29% | 80 | 95 |
| `cli` | 88.82% | none | none |

Overall **94.86%** (11300/11912), against the owner's 90.

Three things this measurement produced that no amount of reading would have.
`application` was at **89.47%** — below the floor its own gate already demanded —
and nobody knew, because `scripts/check-coverage.sh` has existed since 2026-09-02
and no CI job has ever run it (F-12). `planner.rs`, the file `AGENTS.md` calls
dangerous to change because step ordering *is* INV-01, had **zero** unit tests.
And four argv guards in that file turn out to be unreachable through `build_plan`,
which halts at `ManualReview` with no commands before the command builder runs.

Two limits stand. **Branch coverage is still unmeasured** — `--branch` needs
nightly and the pin is stable (ADR-0002), so every percentage here is lines only.
And **`check-coverage.sh` counts test code that lives in `src/` in its own
denominator**: adding in-module tests to `provider_commands.rs` moved
`application`'s total from 1851 to 1902 lines, diluting the very number it was
raising.

**Corrected 2026-09-17: the last sentence of this section said the owner's new
floors "are not written into the gate or the specification yet". They are, in
both** — `check-coverage.sh` carries 95/95/95/95 with a 90 backstop, and
`07-quality-and-testing.md`'s table carries the same with the `adapters` raise
explained in its own row. The sentence was true when written and nobody came
back to it. Same defect as the two it was describing, one level up.

**And the denominator was measured rather than fixed.** `cargo-llvm-cov` turns
out to be installed on this machine, so the deferral reason — the agent cannot
iterate on a locally-unavailable tool — no longer holds. A direct transcription
of LLVM's own `LineCoverageStats` was written and checked against the number
`llvm-cov` reports for the same file: **it disagrees on 59 of 75 files, by 449
lines**, always low, and macro expansion is not the cause (this workspace has
none). So the gate was left alone, because replacing a denominator that is too
kind by an unknown amount with one that is wrong by a known 4.4% is not an
improvement, and F-40's own objection — a gate is the wrong place for an
unvalidated reimplementation — now has a measurement behind it. Moving the
in-`src/` test modules to `crates/*/tests/` is the recommendation.

## Guarded-path edits made under the disclosed escape, 2026-09-17

`docs/spec/11-agentic-sdlc.md` obliges this section whenever the enforcement
layer is edited from inside a session. The session was started with
`MPB_ALLOW_GUARD_EDIT=1` in the environment, deliberately, by the owner.

**What was edited, and why:**

| Path | Why |
|---|---|
| `.claude/hooks/arm-guard-edit.sh` *(new)* | Arms the escape from the human's own message instead of the environment (SE-081) |
| `.claude/hooks/guard-protected-paths.sh` | Honours that marker, binds it to the session that armed it, refuses the marker and log unconditionally, and logs every write the escape permits |
| `.claude/settings.json` | Registers the new `UserPromptSubmit` hook |
| `.claude/hooks/test-hooks.sh` | Tests for the above; made hermetic; three assertions moved out of subshells that discarded their failures |
| `scripts/run-fixture-matrix.sh` | DP-016(c): `expect_plan` tells a refusal from a plan that halts at manual review |
| `scripts/check-links.sh` | F-47: the fence tracker, and a header that describes what the gate now enforces |
| `scripts/test-gates.sh` *(new)* | The two gates' own constructed-input tests |
| `scripts/validate.sh` | Runs them; stage count fourteen → fifteen |
| `.claude/skills/add-mapping/`, `.claude/skills/add-doctor-check/` | Both recipes, each edited together with its `CONTRIBUTING.md` half |
| `.codex/**`, `.agents/skills/**` | Regenerated from `.claude/` by `scripts/generate-codex-layer.sh` (DP-018) |

**The escape now records itself.** Every write it permits is appended to
`.claude/.guard-edit-log` with the time, the path and what armed it. That is new
in this change, and it exists because a disclosure obligation discharged from
memory is the agent's recollection wearing a record's clothes.

**Two things about that log a reader should not be misled by.** It is
git-ignored session state, not an artefact. And on its first run it recorded
`stop-gate.sh`, `claude.yml` and `spec-guardian.md` — none of which were
touched: those were `test-hooks.sh` feeding protected paths to the guard, under
a session whose environment armed the escape. The suite now runs those cases
against a scratch project root, so it cannot write into the real log again.
Evidence a gate fabricates is worse than evidence it does not keep.

**A note for whoever runs the next pass, because it cost a red gate here.**
`.agents/skills/tooling-recheck/` was recreated by a concurrent session in this
repository six times over this pass, the last of them *between* a green
`validate` and the commit that followed it — so the gate was briefly red for a
reason that had nothing to do with the change being committed, and nothing in
the working tree said which session had done it. `scripts/generate-codex-layer.sh`
removes it in seconds; the generator declines to produce that one skill on
purpose, because its subject is Claude Code and it means nothing in the Codex
harness (the generator says so at its own line 184). Worth knowing before
debugging it as a defect in the change under test.

**The escape armed itself once, from a subagent's report, and that is the most
useful thing that happened here.** This section first claimed that only the
human's message could arm the prompt route, because tool results and subagent
output "never reach this event at all". A `spec-guardian` hand-back quoting the
token — while reviewing this very mechanism — was relayed through the prompt
channel and armed the guard on its first live firing. Nobody typed it. The
marker stores an excerpt of what armed it, the excerpt read
`<agent-message from=…>`, and that is how it was caught within the minute: the
detection property this change was built around, working on its own author.
The token must now sit on a line starting at column zero, which removes that
route because the harness indents relayed output — **a filter resting on a
formatting convention, not a boundary**, and written up as one.

**One limitation, stated because the alternative is a reader over-trusting it.**
The environment variable cannot be forged from inside a session; the prompt-armed
marker can, by any session with a shell, because the path guard is `PreToolUse`
on `Edit|Write|NotebookEdit` and never sees `Bash`. SE-081 states this in full.
The marker is detection, not prevention.

### What `spec-guardian` found, and what happened to each

No Tier 1 violation and no charter change. Thirteen items; the ones that changed
something:

| # | Finding | Resolution |
|---|---|---|
| 1 | **The weak half of the escape can manufacture the strong half.** `settings.local.json` sits *inside* the escape-guarded `case`, so an armed session can write an `env` block that arms every later session by the route the documentation calls prevention — in a git-ignored file no diff shows. Reproduced | **Raised as DP-020 and since resolved**, on the owner's ruling, option (c): `settings.local.json` moved to the unconditional deny list, `settings.json` left escape-guarded because a change to it is committed and reviewable. The deciding difference is visibility, not the write. Closes the accidental and Edit/Write routes and nothing else — `Bash` never reaches this guard |
| 2 | The INV-19 refusal's only test is not named `invariant_19_*`, so `grep -rn "fn invariant_"` never surfaces it — and all four existing INV-19 tests pass with the new refusal deleted | **Fixed.** Renamed `invariant_19_a_source_that_cannot_answer_the_service_question_refuses_the_plan` |
| 3 | `expect_plan` accepted any outcome from any fixture, so the only MacPorts-source plan in the matrix became a refusal with nothing pinning it, and a broadened refusal would leave the matrix green | **Fixed.** Each fixture now declares what it owes, derived from the source provider. 193 → 198 checks |
| 4 | "Strictly more than the old check asserted" is false — the old check required every plan invocation to emit a parseable document | **Fixed** both in the script header and the findings entry; finding 3 restores the property, so the corrected sentence is now true |
| 5 | The F-47 index row still read "Open … the fix is written", while the body below it said fixed | **Fixed** |
| 6 | The marker's third line is described as "free text"; the guard reads it into `ARMED_BY` and writes it to the log | **Fixed** |
| 7 | The new Stop-gate stage names no line in `07-quality-and-testing.md`, which `11-agentic-sdlc.md` makes a MUST | **Fixed.** It maps to "Validation is mechanical, not remembered" |
| 8 | SE-081 does not cross-reference SE-072 item 4, which owned this topic (SE-032) | **Fixed**, in both directions |
| 10 | An *absent* `SERVICE_METADATA` is not `Unknown`, so a snapshot from a build predating the capability loads at the same schema version and leaves INV-19 silently unevaluated | **Recorded, not closed.** Closing it changes the snapshot contract, not this refusal |
| 13 | `.gitignore` named the log exactly, so a variant spelling would be commit-visible | **Fixed** — `*` added, after exactly that happened with `.guard-edit-log.polluted` |

Items 9, 11 and 12 needed nothing: 9 confirmed the exit-code contract is real and
documented in three agreeing places, 11 was already fixed by the time it was
read, and 12 was an unreproduced failure from a run concurrent with edits — the
clean rerun is 153/153.

### What `security-reviewer` found in it, and what happened to each

The escape was reviewed before it was committed, which is the only reason most
of this list is in the past tense. Nine findings; seven were real and are fixed,
two were already true.

| # | Finding | Resolution |
|---|---|---|
| 1 | The disclosure log could be silenced without denying anything — `2>/dev/null \|\| true` swallowed a failed append, and `rm -f log && mkdir log` is one `Bash` command | **Fixed.** A permitted write that cannot be recorded is now denied. Detection is the whole of what the prompt route rests on, so a swallowed failure bought a *silent* enforcement-layer write |
| 2 | The expiry was not fail-closed: `mpb_require_json_parser` exited before the disarm, so a message arriving with no `jq`/`python3` left the previous marker armed for the rest of the session. Nothing cleared it at session end either, and `--resume` keeps the `session_id` | **Fixed.** The delete is the first statement in the hook; `session-start.sh` deletes the marker too |
| 3 | Four comments claimed more than the code did — including "no state that outlives the thing that justified it" and "the two settings files … are both refused by the path guard" (refused to three *tools*; `Bash` can append to `~/.claude/settings.json` and arm every future session) | **Fixed**, in the hook and in SE-081. The ninth false statement about what the code prevents in this pass, and the first caught before shipping |
| 4 | `test-gates.sh` wrote to a fixed `/tmp` name and followed a symlink planted there — and this change had just wired it into `--quick`, so it ran at every turn end | **Fixed.** `mkdtemp`, like every other temporary path here |
| 5 | The log had no field escaping: a path may contain a newline, `*` spans it, so a crafted path could close its own record and open a fabricated one | **Fixed.** Both fields squashed to one line |
| 6 | Every permitted write was logged twice, because `check` runs on the path and on its canonical form | **Fixed** — found independently while reading the log, before the review returned |
| 7 | `[ -f "$MARKER" ]` follows a symlink, so pointing the marker at a file the Write tool accepts turns "Bash per arming" into "Bash once". Marker written world-readable with 100 characters of the prompt | **Fixed.** Symlinked marker refused; `umask 077` |
| 8 | `test-gates.sh` lifted a span of `check-links.sh` that included top-level `glob` code, so anything later added in that gap would execute inside a `--quick` stage | **Fixed.** The lift is now two `def` spans with checked anchors |
| 9 | The `Edit\|Write\|NotebookEdit` matcher means any future file-writing tool bypasses the guard entirely; `.codex/` has no `UserPromptSubmit` wiring, so a Codex session never disarms a Claude marker | **Accepted, not fixed.** Both predate this change. The second is safe only because of the `session_id` binding, which makes that binding load-bearing rather than defence in depth — now said in the guard's own comment |

Five of the seven fixes have a new case in `test-hooks.sh`: the parser-less
disarm, the symlinked marker, the unrecordable write, one-write-one-line, and a
newline in the path.

**And one edit made outside the tools, disclosed here for the same reason.**
Midway through restructuring `guard-protected-paths.sh` a half-applied edit left
the file with a syntax error, so the hook failed closed on every subsequent
`Edit` — including the one that would repair it. The repair was made with
`Bash`. The authorisation was present and explicit the whole time, and the guard
would have allowed the write had it been able to run; but it is exactly the
route SE-081 names as the marker's weakness, so it is recorded rather than
passed over.

## CI

Unchanged and still unrun — there is no remote. Beyond the two items the previous
pass flagged, this review found:

- **`cargo-deny` never checks advisories.** The workflow runs
  `check licenses bans sources`; `deny.toml` declares `[advisories] yanked = "deny"`,
  which is therefore dead configuration that reads as enforced (F-16).
- **Twelve unpinned `uses:` references, four of them mutable refs**, not the one
  previously reported — including `taiki-e/install-action@cargo-llvm-cov`, which also
  downloads a binary into the job (F-17).
- **`claude-code-review.yml` cannot post the comments it is configured to make** —
  it declares `pull-requests: read` while granting an inline-comment tool — and has
  no `author_association` guard, unlike `claude.yml`, which has a correct one (F-18).
  The fork-secret concern is real but milder than recorded: the trigger is
  `pull_request`, not `pull_request_target`, so GitHub withholds secrets from fork
  runs.

The two axes of ADR-0004 that *are* enforced were verified and are genuinely good:
`check-licences.sh` fails on an unlisted licence and on any unreviewed `build.rs`,
and `check-deployment-target.sh` reads the load commands off the built binaries and
every object inside every rlib rather than trusting the build.

## Risks and limitations of this review

- **MacPorts was not observed** for the first five days of this pass, and was
  from 2026-09-07 — see "Research" above. Findings raised before that date rest
  on synthesised fixtures and say so; F-24, F-33, F-34, F-37 and F-39 rest on the
  live install. This entry read "MacPorts was not observed" flatly until
  2026-09-11, contradicting the same document 170 lines later. A reader who stops
  at "Risks and limitations" — which is where a careful reader stops — took away
  the opposite of the truth.
- **No Apple Silicon machine.** The both-providers fixture is arm64 and derived; the
  planner findings were reproduced under `--replay`, not natively.
- **Coverage unmeasured**, so "which safety paths are tested" was answered by reading
  the tests rather than by instrumentation. F-19 is a judgement about test quality,
  not a coverage report.
- **The reviewers were used as instruments and their findings were re-verified, not
  relayed.** Every claim reproduced above was re-run by the main session: the
  cross-provider count, the direction asymmetry, the `--advanced` flip, the empty
  environment on `SET_PRIMARY`, the test filter, the redaction leak, the missing
  advisories check, and the action inventory. Two subagent claims were checked and
  *not* carried forward as findings, because my own probe showed my reading had been
  wrong rather than the code: ownership is correctly recorded per row with an
  evidence field, and `NO_ACTION_ALREADY_THERE` behaves correctly despite its name.
- **Two guard false-positives were hit**, both on read-only work — one on a `plan`
  invocation whose *package name* matched a tool name, one on a grep written to
  verify INV-06, blocked by the INV-06 guard. Neither was reworded to evade the
  guard; the documented remedy was used both times. This is the third consecutive
  pass to report this, which is the pattern that eventually produces a workaround
  that is not honest about itself.

## Recommended next tasks, in priority order

**Rewritten 2026-09-08**, replacing a 2026-09-06 list whose every mechanical item
had shipped. The original review's list is preserved below because it is the
record of what the review recommended; a stale to-do list is worse than none, and
this section has now been the stale thing twice.

### 1. Owner, decisions — two open

- **DP-016** (F-29) — what an *unknown* service status should do to
  `MANUAL_REVIEW`. Trades INV-19 against whether `--replay` can still show a real
  plan. Four options, recommendation (c). Nothing executes plans in this build,
  so it becomes live with the executor.
- **DP-017** (F-31) — three of this repository's own guards make one security
  property untestable. Four options; **(d)** is the cheapest and was added after
  the entry was first written — a script that re-derives the two Darwin
  open-flag constants from the SDK header instead of trusting a dated comment.

**DP-018 was resolved on 2026-09-08**: `.claude/` is the source and `.codex/` is
generated from it. This list said "five open" and named it among them until
2026-09-11, and carried a live instruction — *do not commit `.agents/` or
`.codex/` before DP-018 is settled* — against something already done in
`a7ec60a`. Both are removed. The two items that were listed here as decisions
but have no DP number are in section 2, which is where implementation-blocked
work belongs.

Not decisions, but the owner's to schedule: **immutable releases** (R-14) trade
supply-chain integrity against withdrawal manoeuvrability, and a withdrawal
permanently burns the version number; **the Intel runner is billed even on a
public repository** and **the `.pkg` needs a second certificate** (R-11), which
gives DP-008 a cost it was decided without.

### 2. Implementation — what is genuinely left

- **F-40** — the coverage denominator counts test code living in `src/`. Measured
  2026-09-07: **34% of the instrumented surface is test code**, unevenly (16.4%
  in `application`, 44.5% in `policy`), so correcting it reorders which crate is
  nearest its floor rather than shifting all six numbers together. Two honest
  routes — faithful segment post-processing with its own tests, or relocating the
  in-`src/` test modules, which this project has done once already and which
  makes the question disappear. Not a single change either way, and it re-tunes
  the floors when it lands.
- **F-33's data half** — a per-provider success semantic beside the verb in
  `provider-commands.toml`. The obligation is now item 4 of `plan.v1.md`'s
  executor-debt list; the schema change belongs to the executor.
- **F-35** — verify elevation detected rather than stored. Needs a `Capability`
  variant, an adapter method and a policy-data fallback. Executor-shaped.
- **F-04's residual** — the self-invocation's relative program and empty
  environment. A declared Tier 3 deviation (SE-056); the fix is "resolve this
  process's own binary", which is the executor's design.
- **F-43's first item** — **done 2026-09-16**, and this entry was wrong about it
  twice. It said the planner "cannot see which route produced its input" (it can,
  through `Provenance.captured_by`), and then that both planner guards were
  unreachable second lines (`planner.rs:619` is the designed gate on a `~`-rooted
  prefix, and a machine with Homebrew under `$HOME` reaches it). Both corrections
  came from reviewers. `ErrorId::InputUnusable` names no route; the residue is
  F-48.
- **F-29's implementation**, once DP-016 is ruled. The tri-state field itself
  needs no ruling — `AGENTS.md` already requires it.
- **F-23's routing** — clap's `about` through the catalogue. A CLI-pass design
  question, because `--help` renders before `--language` is parsed.
- **F-22** — nothing to do until casks are plannable, at which point the verb
  must become kind-aware in the same change. Three tests fail if it is not.

### 3. Blocked on a machine or a remote, not on effort

**MacPorts is no longer among them.** It was installed on 2026-09-07, and the
first thing it did was disprove the tool: the executor passed no `HOME`, `port`
exits 1 without one whatever you ask it, and a healthy 2.12.6 install was
reported as `Broken` (F-37). R-5's MacPorts half is now observed rather than
cited (E19–E32), which also produced SE-077 and a third independent confirmation
of F-33.

Still waiting: R-11's hardened-runtime and offline-quarantine questions (one
notarisation attempt, one offline machine); R-13's behavioural half, which needs
a real certificate and a real revocation and so may not be answerable before the
first release; R-14's Q2 and Q3, minutes once a remote exists; an Apple Silicon
capture; and **CI, which has still never run** — every result in this pass is a
local result.

Two smaller observations are blocked by this repository's own deny rules rather
than by a machine: a read-only `port select --list` and a read-only
`launchctl print`. Both are recorded in R-5 E32 rather than reworded around.

<details>
<summary>The original list, as the review filed it</summary>

## The review's original list, superseded

**Owner, before anything else.** DP-010 — the Tier 1 ruling. Every other planner
task is cheaper once it is answered, and the fix for F-01 depends on it. DP-011 and
DP-012 can follow at leisure; neither blocks work.

**Implementation pass 1 — the planner and the analysis merge.** These four are one
change and should be done together, because they are the same defect seen from four
angles: F-02 (`--to` names the destination; the guard tests the destination's
contents), F-03 (decide unit identity before the visibility filter; stop hardcoding
`advanced: true`), F-01 (per-provider reverse-dependency count, per DP-010), F-04
(remove the filter from the INV-10 test and give `SET_PRIMARY` a sanitised
environment and an absolute program). Ship with the mirrored `--to homebrew` tests
that do not currently exist, and with the one-line subset assertion in the INV-13
test that would have caught 63→62.

**Implementation pass 2 — the things that are cheap and fail today.** `advisories`
added to the `cargo-deny` command (F-16). Every `uses:` pinned to a 40-hex SHA plus
`scripts/check-action-pins.sh` in `validate.sh` (F-17). `claude-code-review.yml`
given write permission or its `--comment` removed, and an author guard (F-18).
`stop-gate.sh` made to fail loudly when it cannot run the Rust checks (F-20). The
owner-writable finding kind in `path_audit.rs`, without which `doctor` cannot honour
ADR-0005 (F-07). Redaction that says when it could not run (F-05).

**Implementation pass 3 — before the executor, not during it.** Snapshot input
validated at deserialisation and `PlanCommand` given a fallible constructor (F-09).
`PackageAbsent` added and `VERIFY_REMOVAL` fixed; `VERIFY_DESTINATION` made
sensitive to pre-existing state; `check_ordering` made pairwise (F-06). A
single-instance lock, or at minimum a failing test marking the MUST (F-20). The
no-shell guarantee given both a type and a check, and an honest sentence (F-08).

**Documentation pass.** F-12 (the matrix states configuration until a green run),
F-13 (the README's opening sentence), F-14 (INV-03's qualifier restored, "guarantees"
dropped), F-15 (`restore`'s two missing limits), plus the four undated wiki claims
and the wiki's `../docs/...` links, which break the day the wiki is published.

**Then the foundation pass's own list, unchanged and still right.** Capture a real
MacPorts fixture; capture on Apple Silicon; destination catalogues offline; the
executor; locales; coverage in CI once a remote exists.

</details>

## What is right about the foundation

The next pass needs to know what it must not break, and this list is not a
courtesy — each item was checked, several by trying to break it.

**The layering is real.** Exactly one `std::process::Command` in the workspace, with
`env_clear`; the layering check asserts dependency edges from `cargo metadata`
rather than from a diagram; zero elevation tokens in shipped source; no
`HashMap`/`HashSet` in domain, policy or application, so determinism is structural
rather than tested for.

**The policy engine is genuinely pure.** An impurity sweep over `crates/policy/src`
returned only the domain type `CommandLineTools` — a false positive on the word. No
I/O, no clock, no environment; `today` is injected as a fact. That is what makes
INV-14 and INV-15 hold, and it is why the policy layer is the one layer where the
symmetry testing is exemplary.

**INV-15 is proven rather than asserted.** Two `--from-snapshot` runs byte-identical
across 184 KB, and the fixture matrix re-proves it over six fixtures on every run.
Designing capture and analysis as separate functions so that `--from-snapshot` and
`--replay` exercise the *same* code path is the best structural decision in the
project, and it is what made most of this review possible on a machine with one
provider.

**Truncation is treated as corruption.** The reasoning at `process/mod.rs:182-190` —
that a truncated line-oriented inventory parses *perfectly* into a silently short
inventory — is the best piece of security reasoning in the codebase, and it closes a
whole class of data-loss bug.

**Ownership comes from the providers' own records.** `installed_on_request` and the
`requested` pseudo-portname, with per-row evidence and an authoritative flag;
`leaves` used only for orphan candidates and never substituted for intent. There is
no graph inference anywhere a provider has a record — F-20's cask case is the
no-record case, and even it is flagged non-authoritative rather than guessed
silently.

**The state directory is the best-defended area in the codebase.** `O_EXCL` at
creation rather than `chmod` afterwards, `sync_all` then `rename`, symlink and
non-directory refusal, ownership checked via `symlink_metadata` before permissions
are touched, and a uid read without `libc`. The boundary that is *not* guaranteed is
documented rather than glossed.

**Elevation fails safe** — anything not literally `"none"` or `"required"` becomes
`Unknown`, and `Unknown` counts as elevated. F-10 is the cost of that default being
reached by omission, not an argument against the default.

**"No telemetry" is verifiable**, not promised: no networking primitive anywhere in
`crates/*/src`.

**The peer position is kept in the documentation even where the code does not keep
it.** The two migration pages are within ten percent of each other by length and
structurally parallel — the measurement and its date are in the findings
register's closing section, cited rather than repeated here, because maintaining
it twice is what let both copies go stale together (F-46). Nine sections each,
structurally parallel; the peer clause is in the opening paragraph of both `README.md`
and `wiki/Home.md`; and `wiki/MacPorts-to-Homebrew-Migration.md:10-11` invites a bug
report if the direction ever reads as an afterthought. That page is right and F-02 is
the bug it asked for.

**`explain` is the right product.** Facts, the rules that fired with their reasons,
and the rules that did not. For a tool whose value is judgement, exposing the
judgement is correct — and it is what made F-01 visible, because the false
`provider: "macports"` label was printed rather than hidden.

**Zero broken internal links** — the figure and what it counts are in the findings
register's closing section, cited rather than repeated here, because this is the
copy that went stale and was then corrected wrongly (F-46) — and the tiering held under
pressure in the previous session: three Tier 1 edits, all three stopped at the human.
This review adds a fourth test of the same mechanism (DP-010) and the mechanism is
why F-01 is a question rather than a commit.
