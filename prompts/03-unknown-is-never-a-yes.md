# MacPortsBrew — Focused Implementation Pass: an unknown is never a yes (03-UNKNOWN-IS-NEVER-A-YES)

Instantiated from `03-FOCUSED-IMPLEMENTATION-PASS.md` on 2026-09-18. That file
stays the template; this one is the work order.

---

## 0. This pass

- **Capability:** An answer that carries no information never reaches a plan as
  a yes — every provider command in a plan states how success is established,
  and a loaded snapshot that does not record whether packages define services
  refuses to plan instead of planning as though they do not.
- **Why now:** it closes the two open findings whose failure mode is a safety
  rule that does not fire while everything looks green. **F-33's data half**:
  `port -q installed <name>` exits 0 for an installed port, an absent one and a
  name that is not a port at all, so a `VERIFY_DESTINATION` toward MacPorts
  cannot fail — and INV-01 puts exactly that step in front of `REMOVE_SOURCE`.
  **F-29's residual hole**: an *absent* `SERVICE_METADATA` entry is not
  `Unknown`, so a document written by a build predating the capability loads
  under the same `SNAPSHOT_SCHEMA_VERSION` and the DP-016(c) refusal does not
  fire. Both are the same defect — a missing answer read as a definite one —
  which is why they are one pass and not two.
- **Specification sections that govern it:**
  `docs/spec/04-migration-and-safety.md` (Verification contract; Stateful
  packages and services), `docs/spec/02-domain-model.md`,
  `docs/schemas/plan.v1.md`, `docs/schemas/snapshot.v1.md`,
  `.claude/rules/adapters.md` ("`UNKNOWN` is a first-class answer").
- **Invariants it touches:** INV-01 (destination before source — the
  verification that guards `REMOVE_SOURCE` must be able to fail), INV-19
  (stateful packages are never migrated blind), INV-20 (provider facts are
  dated data), INV-15 (a replayed reader never consults the live filesystem —
  must not regress).
- **Out of scope**, and each is named because it is adjacent enough to be
  tempting:
  - **`plan.v1.md` executor-debt item 3** — `PACKAGE_ABSENT` needs a command as
    well as a semantic. This pass gives the semantic a home; the missing command
    on `VERIFY_REMOVAL` is a separate change.
  - **F-35** — verify elevation detected rather than stored. Same chapter, same
    table, different mechanism: it needs a `Capability` variant and an adapter
    that probes the machine, which is a behavioural feature and not a contract.
  - **F-22** — the kind-aware verb for casks. Untouched; its three tests must
    still pass.
  - **F-40, F-47, F-48** — unrelated subsystems.

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
