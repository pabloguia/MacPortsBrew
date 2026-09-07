# 02-ARCHITECTURE-REVIEW-PASS — report

**Date:** 2026-09-02 · **Commit at review:** `ab56b06`

The findings are in
[`02-ARCHITECTURE-REVIEW-findings.md`](02-ARCHITECTURE-REVIEW-findings.md), ranked
by consequence, twenty of them. This report says what was done, what it cost, and
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
`.claude/hooks/test-hooks.sh` passes 70 of 70.

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

That last distinction is the one the charter and README get wrong (F-11, DP-011),
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
`.claude/`, which section 3 forbids a reviewer to touch. The count is
thirty-five commits, most of them `fix(...)`.

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
from the code, from a source already cited, or from the owner. The largest standing
research gap is unchanged and is not a finding of this review — **MacPorts has never
been observed.** The adapter, its fixtures and its contract tests are
documentation-derived. This review inherits that limitation completely: every
MacPorts-side behaviour asserted here was read from synthesised fixtures, and F-02
in particular (the one-direction planner) would be sharper evidence still on a
machine that actually has both providers.

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
raising. The owner's new floors are not written into the gate or the
specification yet; both files are guard-protected and that needs
`MPB_ALLOW_GUARD_EDIT=1`.

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

- **MacPorts was not observed**, as above. Half of every symmetry finding rests on
  synthesised fixtures.
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

**Rewritten 2026-09-06.** The original list is preserved below because it is the
record of what the review recommended; almost all of it has since shipped, and a
stale to-do list is worse than none. What follows is what is actually left.

### 1. Owner, mechanical — one session-start flag unblocks four things

Start a session with `MPB_ALLOW_GUARD_EDIT=1` (the disclosed escape,
`11-agentic-sdlc.md`) and these land together, because they are all the
enforcement layer:

- **The DP-015 coverage floors** — `adapters` 80→95 and a 90% overall backstop,
  into `scripts/check-coverage.sh` and `07-quality-and-testing.md`. Measured and
  met already; only the gate and the specification do not say so yet.
- **The test-lines-in-`src/` denominator** in the same script, which dilutes the
  number it is checking.
- **F-32** — the i18n gate counting a test fixture as a rendered string.
- **`session-start.sh`** rendering an empty decision list as a parse failure.

The last two are the same bug as F-12 and as the `cargo`-absent stop gate, which
is why they belong in one change with the rule written once: **a gate must be
able to fail.**

### 2. Owner, decisions — four, three of them new tonight

- **DP-016** (F-29) — what an *unknown* service status should do to
  `MANUAL_REVIEW`. Trades INV-19 against whether `--replay` can still show a real
  plan. Four options, recommendation (c).
- **Immutable releases** (R-14) — strictly good for supply-chain integrity,
  strictly bad for withdrawal manoeuvrability, and a withdrawal permanently burns
  the version number. Belongs in ADR-0005 or its own entry.
- **The Intel runner costs money** (R-11) — `macos-15-intel` is an alias of
  `macos-15-large`, and larger runners are billed even on public repositories.
  Either pay, cross-compile from the free arm64 standard runner, or ship arm64
  first and say so.
- **A second certificate** (R-11) — the `.pkg` needs Developer ID *Installer*,
  distinct from the Developer ID *Application* identity that signs the Mach-O and
  the `.dmg`. Re-read DP-008 with that cost attached.

### 3. Owner, housekeeping

Sixteen files of enforcement-layer work sit uncommitted, including
`scripts/check-memory-files.sh`, which `validate.sh:33` calls and git does not
have. See "Gate status" above for what that means for every result in this pass.
`.agents/` and `.codex/` are untracked and un-ignored.

### 4. Implementation, in dependency order

- **F-31** — the receipt reader's FIFO guard cannot prevent the hang it exists
  to prevent. Needs `O_NOFOLLOW | O_NONBLOCK`, which needs `libc` in `adapters`,
  which is an ADR-0001/ADR-0002 decision first.
- **F-29's implementation**, once DP-016 is ruled. The tri-state field itself
  needs no ruling — `AGENTS.md` already requires it.
- **F-23's routing** — clap's `about` through the catalogue. A CLI-pass design
  question, because `--help` renders before `--language` is parsed.
- **F-22** — nothing to do until casks are made plannable, at which point the
  verb must become kind-aware in the same change. Three tests fail if it is not.

### 5. Blocked on a machine or a remote, not on effort

**MacPorts has still never been observed** — `port` is not on this machine and
`/opt/local` does not exist, so F-24 and every MacPorts-side assertion in this
repository remain documentation-derived. Also waiting: R-11's hardened-runtime
and offline-quarantine questions (one notarisation attempt, one offline machine),
R-13's behavioural half (needs a real certificate and a real revocation, so
possibly not answerable before the first release), R-14's Q2 and Q3 (minutes,
once a remote exists), an Apple Silicon capture, and coverage in CI.

<details>
<summary>The original list, as the review filed it</summary>

## Recommended next tasks, in priority order

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
it.** The two migration pages are 1065 and 944 words, nine sections each,
structurally parallel; the peer clause is in the opening paragraph of both `README.md`
and `wiki/Home.md`; and `wiki/MacPorts-to-Homebrew-Migration.md:10-11` invites a bug
report if the direction ever reads as an afterthought. That page is right and F-02 is
the bug it asked for.

**`explain` is the right product.** Facts, the rules that fired with their reasons,
and the rules that did not. For a tool whose value is judgement, exposing the
judgement is correct — and it is what made F-01 visible, because the false
`provider: "macports"` label was printed rather than hidden.

**Zero broken internal links across 105 markdown files**, and the tiering held under
pressure in the previous session: three Tier 1 edits, all three stopped at the human.
This review adds a fourth test of the same mechanism (DP-010) and the mechanism is
why F-01 is a question rather than a commit.
