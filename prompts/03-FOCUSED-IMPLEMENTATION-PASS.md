# MacPortsBrew — Focused Implementation Pass (03-FOCUSED-IMPLEMENTATION-PASS, instantiated per task)

A focused pass delivers **one independently verifiable capability**. Copy this
file to `prompts/03-<slug>.md`, fill in section 0, set `.claude/current-pass`,
and run it.

If the task cannot be stated in one sentence and verified without running the
rest of the product, it is not a focused pass — split it.

---

## 0. This pass

- **Capability:** <one sentence>
- **Why now:** <what it unblocks, or which finding it closes>
- **Specification sections that govern it:** <files>
- **Invariants it touches:** <INV-nn, …>
- **Out of scope:** <the adjacent things you will be tempted to fix>

Candidate tasks, from `docs/spec/10-first-pass-work-order.md`: artifact
availability detection; reverse-dependency protection; one policy rule; the
executor for one migration step; primary-provider management; one doctor check;
one locale; release packaging; one wiki page; coverage in one layer.

---

## 1. Read this first

1. `AGENTS.md`.
2. `docs/spec/01-invariants.md` — specifically the invariants named in section 0.
3. The specification sections named in section 0.
4. `docs/spec-evolution.md` — filtered to entries touching this area.
5. `docs/passes/02-ARCHITECTURE-REVIEW-PASS-report.md` — the findings, if this
   pass closes one.

Nothing else, unless the work leads you there. A focused pass that reads the
whole specification has already lost the benefit of being focused.

## 2. Authority and precedence

| Tier | Source | Changeable? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | No. Stop and ask. |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. |
| 3 | **MUST** in `docs/spec/02..11` | Only with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Change freely; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

## 3. Hard boundary for this pass

**Scope.** Only the capability in section 0. Adjacent defects you find go into
findings or issues, not into this diff. A focused pass that grows is no longer
independently verifiable, which was the whole point of it.

**Mutation.** If the capability is *not* an executor step, the no-mutating-code
boundary from the foundation pass still holds and the `Stop` gate still enforces
it. If it *is* an executor step, say so explicitly in section 0, and then:

- it is reachable only through a plan a human has seen (INV-02);
- it never removes before the destination is installed and verified (INV-01);
- it has a `--dry-run` path that mutates nothing and prompts for nothing;
- failure leaves the prior working state (INV-03) and reports partial success as
  partial success;
- every elevating operation is enumerated in the plan before it happens (INV-06);
- its invariant tests exist **before** it is wired into a command.

## 4. Working protocol

- **Write the test first.** For a focused pass this is not a preference. The
  capability is defined by what verifies it, and a test written afterwards tends
  to describe what the code does rather than what the specification requires.
- **Verify, do not assume.** Provider facts: observed with a read-only command,
  cited with a read date, or runtime detection.
- **Fixtures, never the live system.** If you need a case the fixture matrix
  does not cover, capture a fixture with full provenance and add it to the
  matrix.
- **Use the reviewers**, scoped to the diff.
- **Commit incrementally**, conventional commits.

## 5. Order of work

1. Read the specification sections and the named invariants.
2. Write the failing test — invariant test first if one applies.
3. Implement the smallest thing that passes it.
4. Extend to the fixture matrix rows that apply.
5. Wire into the CLI and `--json`, with the schema version bumped if the payload
   changed.
6. Documentation: i18n keys, wiki page, `CONTRIBUTING.md` recipe if the
   capability is extensible.
7. `/preflight`, reviewers, report.

## 6. Definition of done

- [ ] The capability works end to end and is demonstrated, not asserted.
- [ ] Every invariant named in section 0 has a passing test named after it.
- [ ] Fixture matrix rows that apply are covered; new fixtures carry provenance.
- [ ] `--json` schema updated and versioned if the payload changed.
- [ ] Coverage floor for the layer met.
- [ ] i18n keys added to the English catalogue; parity check passes.
- [ ] Documentation updated for anything user-visible.
- [ ] `docs/spec-evolution.md` records any material deviation.
- [ ] `/preflight` passes; reviewers' findings resolved or accepted in writing.
- [ ] Nothing outside section 0's scope changed.

## 7. End-of-pass report

`/pass-report` to `docs/passes/03-<slug>-report.md`. Short is correct here.
Say what now works, what it does not cover, what you found and did not fix, and
what the next pass in this area should do.
