<!-- SPDX-License-Identifier: Apache-2.0 -->

# 03-UNKNOWN-IS-NEVER-A-YES — pass report

**Date:** 2026-09-18 · **Work order:**
[`../../prompts/03-unknown-is-never-a-yes.md`](../../prompts/03-unknown-is-never-a-yes.md)
· **Branch:** `pass/02-architecture-review`, on top of `d16a783`

The first instance of the focused-implementation template. One capability, two
findings, and they are one defect wearing two faces: **an answer that carries
no information was being read as a definite one.**

## What now works

**A plan says how each of its commands could fail.** Every `PlanCommand`
carries `success`: `EXIT_STATUS`, `STDOUT_NAMES_PACKAGE`, or `UNKNOWN`. The
value comes from `[entry.success]` in `data/policy/provider-commands.toml`,
which is dated, sourced data like every other provider fact (INV-20), and the
loader refuses an entry whose verb omits it or misspells it.

This exists because of one observation. On this machine, 2026-09-08, MacPorts
2.12.6: the read verb exits **0** for an installed port, for a real port that
is absent, and for a name that is not a port at all. `VERIFY_DESTINATION` is
the step INV-01 puts in front of `REMOVE_SOURCE`, so an executor reading "exit
0" as "verified" would pass verification for a package that was never
installed and then remove the source — while `check_ordering` held perfectly.
Homebrew's verify verb *does* exit 1 for an absent name, which is why this is
per-provider data and not one rule.

**A snapshot document that does not record a capability can no longer be
planned from as though it had answered.** `load_snapshot` records every
capability the document does not mention as `UNKNOWN` before any check runs,
so the DP-016(c) refusal fires on the one route that bypassed it. Read-only
analysis of the same document still works and reports `UNKNOWN`.

**And the two peer providers now agree about the shape of "absent".** The
Homebrew adapter wrote an empty capability table for a provider that is not
installed; the MacPorts adapter wrote all nine as `UNKNOWN`. Homebrew now does
what MacPorts does. This was not planned work — see below — and it changes
user-visible output: an analysis of a machine without Homebrew reports nine
`UNKNOWN`s rather than nothing, which is the honest answer and the one the
peer already gave. Three golden fixtures were regenerated for it. Read-only analysis of the same
## The distinction that is the whole of the second half

`ProviderCapabilities::is_unknown` answers `false` for an **absent** entry, on
purpose: in memory, a capability nobody reported was never asked about, and
reading that as "undetectable" would refuse every plan built from a hand-made
snapshot. That rule is untouched and its test still passes.

A document is in a different position. A capture asks every capability and
writes every answer, so a document that omits one was written by a build that
did not have the question — and the file does not say what answer it would have
given. Absence there means the record does not say, which is what `UNKNOWN`
means. The fix is at the load door, not in the predicate, and the two rules now
coexist because they answer different questions.

## What this pass got wrong — twice — and what caught each one

**Attempt one broke INV-15, and the fixture matrix refused it.** It filled
every provider's capability table. `neither-provider-intel-macos13` failed with
*"snapshot round trip differs from the replay analysis"*: inventing nine
unknowns for a provider whose table had been written empty made
`--from-snapshot` disagree with a live analysis of the same machine — the
property INV-15 exists to hold and the matrix exists to prove.

**Attempt two satisfied the matrix by skipping empty tables, and shipped a
bypass of the refusal this pass exists to install.** `spec-guardian`
demonstrated it against the binary: delete the `SERVICE_METADATA` key and the
plan is refused; empty the object *containing* it and you get exit 0 and a
byte-identical plan, from a source provider still declaring itself `PRESENT`
with a full inventory. INV-19's rule silently does not fire. A snapshot file is
untrusted input in this project's own threat model (F-09 and F-21 each cost
three security reviews on that premise), so a one-keystroke bypass is the whole
finding.

**Worse than the bug: SE-084 had already rejected `#[serde(default)]` in
writing, for producing exactly that behaviour — and attempt two implemented it
anyway.** The entry argued against a consequence in one paragraph and shipped
it in the next. That is the failure mode this repository treats as
unrecoverable: not being wrong, but being wrong while the document says
otherwise.

**Both symptoms had one cause, and it was in neither place I was looking.** The
Homebrew adapter wrote an empty capability table for an absent provider where
the MacPorts adapter wrote nine unknowns. The peers disagreed about the shape
of "absent", with no stated reason and against `AGENTS.md`'s peer position, and
it was invisible until a rule about documents had to be written around it. A
rule written at the load door to accommodate that could only choose which of
the two defects to keep. Fix the adapter and the rule is unconditional again,
both shapes refuse, and the INV-15 property holds because the rule is a no-op
on every table this build writes.

**Attempt three had a third hole, and `security-reviewer` found it.** The
refusal matched `CapabilityState::Unknown` and nothing else, so a document
whose capability read `UNAVAILABLE` planned normally — exit 0, a plan carrying
`REMOVE_SOURCE` with `elevation: REQUIRED`. I reproduced it before fixing it.
`Unavailable` is the other way a record says "I cannot establish this", and the
predicate now names the question it is actually asking:
`ProviderCapabilities::cannot_answer`. The same review found a **third**
producer of the empty table — `capture::broken`, reachable when a present
Homebrew's output fails to parse — which the comment I had just written said
could not exist.

So three attempts, three holes, each a different shape of the same mistake:
**closing a route instead of naming the question.** The question is "does the
record affirmatively say this provider can answer", and only the third version
asks it.

**The thing worth generalising:** a gate caught the first mistake in an hour
and an adversarial reviewer caught the second, and neither was found by me
reading my own change. The second would have shipped — it was green on every
gate in the repository.

## Three residuals the reviewers left, and what happened to each

All three were additive and none blocked a commit; all three are done, because
two of them were "the fix has no test", which `AGENTS.md` calls incomplete.

- **`is_unknown` is gone, not deprecated.** After the planner moved to
  `cannot_answer` it had zero callers. Two near-identical public predicates,
  where picking the wrong one is a silent data-loss-class defect, is the exact
  shape that produced the bug — leaving the loser in place for a future caller
  to pick would have preserved it. The reasoning in its doc comment that is
  still true moved to the survivor.
- **`capture::broken()` now has a test**, and getting one was a signature
  change: it took `&dyn ProviderRead` and used only `provider.id()`, so a test
  needed a stub of the whole trait. It takes a `&ProviderId` now and the test
  is four lines. This is the one producer the fixture matrix provably cannot
  reach — no recording makes `snapshot()` return `Err`, and `macports-broken`
  reaches `Broken` through `detect()` instead — so it was fixed blind until
  this.
- **`cannot_answer` is asserted at its definition site**, covering both
  spellings and the absent-is-false carve-out. That carve-out is what the
  whole document rule rests on and it was asserted only from two crates away.

## Accepted rather than changed

**An embedder can still hand `build_plan` a snapshot with the capability
absent, and get a plan.** `build_plan` is re-exported, `ProviderSnapshot`'s
fields are `pub`, and `Snapshot` is constructible by hand — so the in-memory
carve-out is reachable from outside the binary even though every route inside
it is closed (`load_snapshot` is the only deserialisation site in the tree, and
all three capture producers write full tables; `security-reviewer` traced both).
`capture.rs` made `CaptureContext` un-constructible for precisely this class of
reason, and the same argument was not applied here. It is accepted rather than
changed because the carve-out is deliberate and the alternative — refusing a
hand-built snapshot — breaks the in-memory case it exists for. Recorded so it
is a choice rather than an oversight.

**Homebrew's parse-failure branch writes `Unavailable` where it means
`Unknown`.** A document this build cannot parse is "I could not look", not
"detected absent", and the branch labels seven capabilities the second way.
Nothing depends on the distinction now that the safety predicate treats both
alike, and it is inert until a `doctor` line or a `has()` consumer surfaces it.
Not changed here because `Unavailable` carries the error id as its reason and
`Unknown` carries nothing, so the honest relabelling loses information a user
would want — which is a design question about a third state, not a typo.

## Rejections — what was considered and not done

The findings named two ways to close the second half. **Both were rejected, and
each would have been worse than the hole.**

- **`#[serde(default)]` on the capability field.** Rejected, and the reason
  changed under the rework, which is worth saying rather than hiding. The
  original reason — an optional field means a missing table deserialises to an
  empty map and reads as "never asked about" — was true of the bounded rule
  and is false of the unconditional one, where an empty map is filled and
  refuses. The reason that survives, demonstrated by `spec-guardian`: keeping
  the field required refuses a malformed document at **parse** time, exit 8,
  before anything reads it, where defaulting would refuse at the planner, exit
  4, after the rest of the document had been trusted. **The original rejection
  was also violated in substance by this pass's own second attempt**, recorded
  above rather than tidied away: an argument written down is not a guarantee,
  and what caught it was someone re-deriving the behaviour from the binary.
- **Bumping `SNAPSHOT_SCHEMA_VERSION`.** It refuses the document outright,
  including for read-only analysis. `.claude/rules/adapters.md` says an unknown
  must not cause that: "the analysis degrades and says so; it does not guess,
  and it does not fail either."

Also rejected: **rendering the success semantic in the human plan.** It would
need an i18n key and a column in a view nobody executes from; `--json` is the
executor's contract and is where the field belongs today.

And **the deferral itself.** `plan.v1.md` said the success semantic "is a
schema change and therefore the executor's, not this pass's". That was written
when the alternative was designing a shape against a hypothesis. It stopped
being one on 2026-09-08, when the third observation confirmed all three cases
produce exit 0 — and the cost of carrying it another pass was that the only
open finding whose failure mode is silent data loss stayed open. What genuinely
belongs to the executor is unchanged and is still item 4 of that list: read the
field, refuse what it cannot perform.

## What this does not cover

- **`PACKAGE_ABSENT` still carries no command** (`plan.v1.md` executor-debt
  item 3). This pass gave the semantic a home; the missing command on
  `VERIFY_REMOVAL` is a separate change and is still owed.
- **No executor reads the field**, because there is no executor. What shipped
  is the contract and the data behind it. `security-reviewer` suggested
  surfacing `stepsWithUnknownSuccess` at the root of the plan document, so a
  plan no executor may run says so in the artefact. **Deliberately not done**:
  it would be the only one of the three executor-refusal conditions with a
  root-level field — `placeholder` and an unrecognised `check` have none — and
  three conditions that an executor must treat identically should acquire
  their surface together, in the pass that writes the executor. Recorded so
  that decision is visible rather than an omission.
- **`[entry.verbs]` is still kind-blind** (F-22). Untouched deliberately; its
  three tests still pass.
- **F-35, F-40, F-47, F-48** — out of scope by the work order, and unchanged.
- **No fixture exercises a provider whose output this build cannot parse.**
  `capture::broken()` now writes nine `UNKNOWN`s like everything else, so the
  INV-15 property holds there — but it holds untested, because the matrix has
  no recording that reaches it. `spec-guardian` reached it by corrupting a
  real recording's `brew info` stdout, which is how such a fixture would be
  derived. Left for the pass that wants it: it is the only presence shape the
  matrix does not cover, and the safety consequence today is nil, since a
  broken provider carries an empty inventory and can never be a plan source.

## Provenance, corrected

`spec-guardian` also found that the Homebrew `[entry.success]` cells cited a
research note containing no Homebrew observation (R-5 E22 is `port` only) and a
date belonging to neither half of the claim. The facts held; the citation did
not, which is F-28's shape. The positive case is R-5a E1, 2026-09-04; the
negative case was observed 2026-09-06 and lives in the F-33 section of the
findings register rather than in a research note, which the comment now says
outright. The entry's `source` and `read_on` were updated with it — the file's
own convention is that `read_on` is the newest read in the entry, and adding
cells without moving it is how a dated fact stops being dated.

## What was found and not fixed

**F-50 — the drift guard F-27 built does not cover the documents an executor
builds against.** `crates/cli/tests/documented_samples.rs` scans `README.md`
and `wiki/*.md` and nothing else, so no gate has ever compared a
`docs/schemas/*.md` sample against what the binary prints. Two of
`plan.v1.md`'s samples were wrong when this pass touched them: one carried an
elevation that F-24's observation had changed a fortnight earlier, and neither
carried `placeholder`, which the row above them calls always present. **Both
samples are corrected here; the guard is not**, because widening a gate's scope
is its own change and would have grown a focused pass past the point of being
independently verifiable. It is recorded in the 02 report's implementation
list so the next pass inherits it.

## Tests, and which of them discriminate

A test that only goes green has shown it can say yes. These were probed:

- `invariant_19_a_document_that_does_not_record_the_service_question_refuses_the_plan`
  (`crates/cli/tests/end_to_end.rs`) — a real capture, the key deleted, the real
  binary planning from it. It carries a **control**: the same document with the
  capability set to a definite answer must plan successfully, so the refusal
  cannot pass for the reason DP-016(c) already covered. **Probed by removal:**
  with the one normalisation call commented out it fails with exit 0 — a plan
  built where a refusal is owed. It also asserts that analysis of the same file
  still succeeds and reports `UNKNOWN`, and finally that **emptying the whole
  table refuses too** — the arm attempt two would have failed, added because a
  reviewer had to find that case rather than a test.
- `invariant_01_a_verification_command_states_how_it_could_fail`
  (`crates/application/tests/invariants.rs`) — asserts the semantic **per
  direction**, because the two providers genuinely differ and a blanket rule is
  the mistake being closed. Asserting Homebrew's `EXIT_STATUS` is what keeps the
  MacPorts cell a fact rather than a workaround.
- `every_verb_states_how_its_success_is_established` and
  `a_success_semantic_that_is_not_one_of_the_two_is_refused`
  (`crates/application/src/provider_commands.rs`) — the loader probed by
  deleting each cell and by corrupting one.
- `the_shipped_table_states_the_semantics_that_were_observed` — pins the two
  values the observation produced, so a later "correction" to exit status fails
  and says why.
- `a_command_that_does_not_say_how_it_succeeds_is_unknown_not_exit_status`
  (`crates/domain/src/plan.rs`) — asserted through serde, because the hazard is
  a *document*, and covers both the absent field and a semantic a newer build
  might add.
- `capability_all_is_the_whole_enum_and_not_a_stale_copy_of_it` and
  `capability_all_has_no_duplicates` (`crates/domain/src/provider.rs`) — the
  drift guard `ErrorId::all()` arrived at after F-43, applied to the new
  `Capability::all()`.

## Specification changes

**SE-083** — the success semantic, and why the deferral was taken early.
**SE-084** — an unrecorded capability in a document is an unknown one, and why
neither route the finding named was used. Both in
[`../spec-evolution.md`](../spec-evolution.md).

`schemaVersion` stays 1 in both schemas. Per `.claude/rules/domain.md` the
breaking operations are renaming a field, changing a type, removing an enum
value and changing a union's tag; adding a field is none of them, and SE-080
already records that the version stays for compatible additions. Nothing that
parsed before fails to parse now.

## Next in this area

1. **F-35**, the obvious successor: verify elevation detected rather than
   stored (INV-20). `Capability` now has an `all()` with a drift guard, which is
   the primitive a new variant needs.
2. **Executor-debt item 3** — a command and a semantic for `PACKAGE_ABSENT`,
   which is the mirror of what shipped here.
3. **F-50**, cheap and in the right place while the schema documents are fresh
   in mind.
