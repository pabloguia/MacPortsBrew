# 02-ARCHITECTURE-REVIEW-PASS — report

**Date:** 2026-09-02 · **Commit:** `b6111bf`

## Read this first: the pass is not complete

This report exists because a Tier 1 edit needs a traceable artifact, not because
the architecture review finished. **It did not start.** Section 5 of the work
order — provider neutrality under INV-17, isolation under INV-07/08/10, the
planner and verifier symmetry check — is untouched. What this session did was
record three owner decisions that arrived mid-pass, and propagate them.

Six of the seven items in the work order's definition of done are unmet. The one
met is "anything requiring a human decision added to
`docs/decisions-pending.md`" — and it was met by *emptying* that file's Open
section rather than by adding to it.

**Do not read this as a clean architecture review.** The next session should run
the pass as written; nothing here substitutes for it.

## Gate status: passed on what it could check, skipped the rest

`.claude/hooks/stop-gate.sh` exits 0. That covers internal link resolution, JSON
and shell validity, the grep-checkable no-mutation boundary, and
specification-currency warnings.

It **skipped** every Rust check. `stop-gate.sh:122` guards the `validate.sh` call
with `command -v cargo`, and this machine has no Rust toolchain — already
recorded in R-10, E-14. Run directly, `scripts/validate.sh --quick` reports
`cargo: command not found` for format, clippy, layering and licences; the two
checks that do not need cargo pass — SPDX ok, i18n ok at 154 keys with 106
referenced. **No test ran in this pass, and no coverage figure in this report is
observed.** The change is documentation-only, so it has no way to break a Rust
check — but "cannot have broken it" is not "verified", and that distinction is
the whole point of having a gate.

## What shipped

Nothing a user can run. No code changed; `crates/`, `scripts/`, `.github/` and
`.claude/` are untouched, the last three by the pass boundary in section 3.

What changed is that three questions blocking the project's public existence are
answered, and `docs/decisions-pending.md` now opens with **Open: None**.

## Decisions

Three, all the owner's, all made in session on 2026-09-02.

**DP-004 — release signing.** Developer ID signing plus notarisation, with a
secure timestamp, and three artifacts per architecture: the bare binary, still
the path the documentation leads with, plus a `.pkg` and a `.dmg` that can carry
a stapled ticket a bare Mach-O cannot (R-9). The earlier recommendation argued
for ad-hoc signing on four grounds; two were voided by a fact the analysis did
not have — the owner already holds a paid Apple Developer account, so the
recurring cost does not exist and the legal identity it binds is the one DP-006
resolved as copyright holder the same day. Recorded in **ADR-0005** (new) and
SE-014.

**DP-006 — copyright holder.** Pablo Guia, an individual. The string matches the
Apple Developer account deliberately, so the Developer ID signature and the
copyright line name one person rather than two spellings of one. Source files
unchanged: ADR-0001's SPDX-only rule stands. Recorded in SE-021.

**DP-007 — hosting and visibility.** Public repository on GitHub, personal Pro
account, named `macportsbrew` (ADR-0003), with Pages for the project site.
Recorded in SE-037.

**One Tier 1 edit, authorised explicitly, made twice.** INV-10's parenthetical
rationale justified a sanitised environment by saying build systems probe
`/usr/local` — true, and not a reason for this rule, since clearing environment
variables does not stop a compiler searching a path compiled into it. It was
first replaced with the reason that does hold, then **shortened** at the owner's
prompting from eight lines to two, deleting a summary of F-2 rather than
correcting it. Both edits are in SE-034, which now also records why: a second
copy of a research note inside Tier 1 text drifts from the sourced one, and this
one already had — every remaining accuracy complaint sat inside the summary, not
in the rule. The invariant's requirement is byte-identical throughout.

## Specification changes

All in `docs/spec-evolution.md`; this section indexes them.

Extended rather than multiplied, SE-032 applied: **SE-014** (signing resolved;
attestation promoted SHOULD → MUST), **SE-021** (copyright holder), **SE-034**
(third Tier 1 correction and its two refinements), **SE-037** (public repository
confirmed, with the `-large` prohibition). No new identifier was created for any
of them, and no entry restates what ADR-0005 says.

Normative text changed in `08-distribution-and-project.md` (the signing decision
as six MUSTs, attestation promoted), `07-quality-and-testing.md` (`-large` and
self-hosted runners prohibited) and `09-research-checklist.md` (R-11 opened).

**Nothing was rejected in this pass**, which is itself a signal: a review pass
that rejects nothing has probably not reviewed anything.

## Research

R-1 through R-10 are answered and dated 2026-09-02. **R-11 is open** and blocks
the implementation pass that writes the release workflow: whether stapling a
container covers a binary copied out of it — that answer decides whether the
`.dmg` is kept or dropped, since `08` marks it provisional on exactly this;
whether the Apple signing toolchain is present on the free `macos-15-intel`
runner, which ADR-0005's compatibility argument assumes; which certificate signs
the `.pkg`; whether the hardened runtime needs any entitlement for a tool that
spawns other package managers; and what a quarantined, notarised, unstapled
binary does with no network.

R-9's own unresolved experiment survives as documentation of the path not taken.
It no longer gates a release.

## What this pass does not do

It does not review the architecture. It does not touch code, tests, CI or
`.claude/`. It does not verify a single claim about Apple's toolchain by
observation — no signing, notarisation or stapling was performed, or could be. It
does not establish that the release pipeline it specifies is buildable.

## Tests and coverage

Unmeasured, for the reason in the gate section. The invariant this change
touches, INV-10, keeps both its tests —
`invariant_10_reading_the_machine_leaks_no_build_flag_into_a_probe`
(`crates/adapters/tests/invariants.rs`) and
`invariant_10_plan_commands_carry_the_sanitised_invocation_environment`
(`crates/application/tests/invariants.rs`) — and neither needed to change,
because only the rationale moved. `spec-guardian` confirmed the requirement text
is byte-identical across all three edits.

## CI

Unchanged and unrun. The support matrix is untouched and still honest.

Two CI facts this pass found and could not fix, the pass boundary forbidding it:

- **`dtolnay/rust-toolchain@master` is a mutable branch reference** in
  `repository-validation.yml`, on a `pull_request` trigger. Whoever controls that
  repository executes arbitrary code in every CI job. This is live today and gets
  worse the day DP-007 is acted on.
- **`claude-code-review.yml` runs on `pull_request` and consumes a secret.** On a
  public repository GitHub withholds secrets from fork-triggered workflows, so
  the first outside contributor's pull request gets a red check they cannot fix —
  the exact outcome the new MUST in `08` exists to prevent, violated by the
  repository's own CI on day one.

## Risks and limitations discovered

**The gate degrades silently.** `stop-gate.sh` skips every Rust check when
`cargo` is absent and still exits 0. On a machine without the toolchain — this
one — the definition-of-done gate reports success while checking a fraction of
what it names. It should fail loudly, or state what it skipped.

**A guard false-positives on documentation, twice.** Editing
`08-distribution-and-project.md` was blocked because the replacement prose quoted
a mutating provider command as an example. Writing *this report* was blocked for
the same reason, by the paragraph describing the first block. Nothing was
reworded to evade the guard either time — the first edit was re-anchored on text
that did not need the string, the second was written through a file rather than a
shell command line, which is the remedy `CLAUDE.md` names. But a guard that
blocks writing *about* a command is a guard that will eventually be worked around
instead of fixed, and the workaround will not always be honest.

**A hook now emits an ambiguous signal.** `session-start.sh` scrapes `### DP-`
headings from the Open section. With that section empty it prints
`(none parsed - read the file)` — byte-identical to what it prints when the parse
breaks. Found by `spec-guardian`.

**Four obligations have no register.** `decisions-pending.md` is for decisions,
and these are not: the revocation incident-response plan ADR-0005 declares owed
and does not write; extending ADR-0004's dependency gate to reach GitHub Actions;
the `scripts/validate.sh` check for action pinning; and enabling private
vulnerability reporting *before* publication, which is a one-way door. They live
only in prose today.

**The assumption a reviewer should attack.** That recording a decision well
substitutes for exercising it. Every control in ADR-0005 is written; none is
enforced by anything that runs. Three review rounds found, in the text this
session produced, an inference dressed as a citation, a chapter mandating two
contradictory CI assertions, a claim in the present tense about a `doctor` check
that does not exist, and six requirements dropped by a consolidation intended to
reduce risk. Each was caught by a reviewer, not by the author — and none would
have been caught by a machine.

## Recommended next tasks

**Architecture review pass — run 02 as written.** Section 5 is untouched. This is
the top of the list; everything below assumes an architecture nobody has
reviewed.

**Implementation pass, in priority order.** Pin every `uses:` to a 40-hex SHA and
add `scripts/check-action-pins.sh` to `validate.sh` — this one fails today and is
testable now. Guard `claude-code-review.yml` against fork pull requests. Make
`stop-gate.sh` fail loudly when it cannot run the Rust checks. Add the
user-writable-and-precedes-system-paths finding kind to
`crates/adapters/src/system/path_audit.rs`, without which `doctor` cannot honour
the requirement ADR-0005 places on it. Then the release workflow, which R-11
blocks.

**Research pass.** R-11, all five items.

**Owner, before publication.** Enable private vulnerability reporting; create the
protected deployment environment with its `v*` tag rule, and a protected-tag
ruleset. No check in this repository can verify any of the three.

**Owner, decisions.** Whether the `.pkg` is also provisional pending R-11 — the
same premise carries it and only the `.dmg` is marked — and whether the first
public release may ship before the revocation plan exists.

## What is right about the foundation

The next pass needs to know what not to break.

**The tiering works under pressure.** Three Tier 1 edits happened in this
session; all three stopped at the human, none was made on an agent's judgement,
and all three are recorded with their authorisation. The mechanism did what it
was designed to do on the first occasion that tested it.

**Reviewers earn their cost.** Three rounds found real defects in text that read
fine, including two miscitations — the failure mode this project is explicitly
built against — in work produced by an agent that had just finished writing about
the importance of citations. The read-only-reviewer design is the strongest thing
here.

**"Record, do not accumulate" is the right rule, and it is not free.** SE-032
prevented four new identifiers. The consolidation that followed the same rule
dropped six requirements and had to be caught and reversed. The rule is correct;
applying it to normative text needs a diff of requirements, not a reading.

**F-2 and R-9 held up.** Both were written by earlier passes and both survived
being leaned on hard by a decision they were not written for. The dated-research
discipline paid a return this session.
