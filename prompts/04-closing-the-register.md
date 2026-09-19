# MacPortsBrew — Consolidation Pass: closing the register (04-CLOSING-THE-REGISTER)

Instantiated from `03-FOCUSED-IMPLEMENTATION-PASS.md` on 2026-09-19, and
deliberately **not** focused: where that template takes one capability, this
pass takes the register's residue. Written after the work began rather than
before it, which is backwards and is recorded as such — `spec-guardian` found
three `docs/spec-evolution.md` entries citing a pass identifier `prompts/` did
not define.

---

## 0. This pass

- **Capability:** every claim this repository makes about itself that no gate
  was checking is either checked or written down as unchecked. Not a feature.
- **Why now:** the 02 register had reached the shape where its *open* items were
  outnumbered by its *stale* ones, and the two are the same defect at different
  ages. `AGENTS.md` names being wrong invisibly as the one unrecoverable error
  here; a register that has drifted is that error about itself.
- **What made it a pass rather than a tidy-up:** the first thing checked —
  `data/policy/policy.toml`'s version against the newest `read_on` beside it —
  was wrong, had been for eleven days, and was governed by a rule stated in
  three places and enforced in none. The second — the documented samples under
  `docs/schemas/` — was wrong in a document that names a finding this build does
  not emit. Neither is a documentation problem.
- **Specification sections that govern it:** `docs/spec/01-invariants.md`
  (INV-17, INV-20), `docs/spec/06-cli-and-ux.md` (exit codes and the redaction
  MUST), `.claude/rules/policy-data.md`, `docs/schemas/*.md`.
- **Out of scope**, each named because it is adjacent:
  - **F-47's two residual holes** — absolute and `../`-escaping link targets,
    and blockquoted headings. Both need `scripts/check-links.sh`, which is
    guard-protected, and `MPB_ALLOW_GUARD_EDIT` is not set. `CLAUDE.md` makes
    that a question for the human, not an obstacle to route around.
  - **F-40's denominator** — the same, for `scripts/check-coverage.sh`. Its
    other route (moving in-`src/` test modules) is real work and is its own pass.
  - **F-23's routing** — needs a pre-pass over argv before `Cli::parse()`, and
    touches `scripts/check-i18n-parity.sh`'s exemption.
  - **F-04's residue** — deferred by design (SE-056) to the executor pass, and
    pinned by a test that says so.
  - **F-35, and executor-debt item 3.** Both are the successors this pass names.

## 1. Hard boundary

**Nothing that changes what the binary decides.** A refusal's wording, a gate's
scope, a data file's version and a document's accuracy are in scope. A rule, a
priority, a step order and an exit code are not — and the one place that bit was
`ErrorId::CapabilityMissing`, proposed in review as "the same exit code 8" and
in fact 5. It was not taken; it went to DP-021.

**No mutating code.** Unchanged from the foundation pass, and the `Stop` gate
still enforces it.

## 2. Working protocol

As `03-FOCUSED-IMPLEMENTATION-PASS.md`, with one addition this pass earned:
**prove every gate fails.** Each of the four gates touched or added here was run
against a constructed failure before being believed — the policy-version test
against a reverted manifest, and the sample guard against a changed value, a
wrong exit code, a pointer resolving to nothing, and an argv naming a write verb.
A gate that has only ever been observed passing has not been observed.

## 3. Definition of done

- [ ] Every open register row is closed, or says what it is waiting on.
- [ ] Every claim corrected is corrected in the *operative* copy, and the
      historical one is annotated rather than rewritten.
- [ ] Anything that belongs to the human is in `docs/decisions-pending.md` with
      options, a recommendation and a reversible default already taken.
- [ ] `/preflight` passes; `spec-guardian` and `security-reviewer` findings are
      resolved or accepted in writing.
- [ ] `docs/spec-evolution.md` carries an entry per topic, and each names what it
      does **not** do.

## 4. End-of-pass report

`/pass-report` to `docs/passes/04-closing-the-register-report.md`.
