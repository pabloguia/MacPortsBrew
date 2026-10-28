<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — The mutating executor (08-THE-MUTATING-EXECUTOR)

> **Run through work order 16 (SE-109); its §1 item 8 overrides this file's
> plan-mode, merge, push, dependency and stop instructions.**
> (`prompts/16-complete-mutating-product.md`)

The dry-run half of the executor shipped in pass 05, was hardened in pass 06,
and pass 07 settled the owner's decisions that blocked the rest (DP-026, DP-027;
report `../docs/passes/07-owner-rulings-and-housekeeping-report.md`). This pass
builds what a mutating step needs *before* one may be dispatched, and ends with
a reviewed decision on whether any dispatch lands in this build. It is a
safety change throughout: INV-01, INV-02, INV-03, INV-06, INV-16 and INV-19 are
all in play.

---

## 1. Read this first

1. `AGENTS.md`; `docs/spec/00-charter.md`; `docs/spec/01-invariants.md`.
2. `docs/spec-evolution.md` — SE-092 (the executor, its addenda and "Owed by
   the mutating pass"), SE-016 (DP-027 and its "Not closed"), SE-050, SE-015.
3. `docs/schemas/plan.v1.md` — "What an executor inherits", all twelve
   obligations, and the `provider_configuration` risk.
4. `docs/spec/04-migration-and-safety.md` — Process execution, Privilege,
   Concurrency and locking, History, the threat model.
5. `docs/spec/11-agentic-sdlc.md`, then this pass's plan once written.

## 2. Authority and precedence

| Tier | Source | Changeable? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | No. If one seems wrong, **stop and ask the human.** |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. |
| 3 | **MUST** in `docs/spec/02..11` | Only with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Change freely; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

## 3. Hard boundary for this pass

- **`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`** until the owner, in a
  message, rules on a written proposal to change it. Everything else in this
  pass is built and tested with it `false`.
- **Tests never touch a live installation** (INV-05): every mutating path is
  exercised through a recorded or synthesised `ProcessRunner`.
- **The spawn-site allowlist only grows by an owner-approved, security-reviewed
  change**, one entry per verb, each with a two-way test.
- **No elevation is performed by this tool's code** without the per-operation
  consent INV-06 requires; until that consent path exists the gate keeps
  refusing elevated steps.
- **No new dependency** without ADR-0001/ADR-0002's three gates.

## 4. Working protocol

As pass 07. Plan mode first, the plan reviewed by `spec-guardian` and
`security-reviewer` before any code. Mutation-test every new rule. Make guarded
writes first and run the hook tests last when the owner arms the escape
(SE-093).

## 5. Order of work

1. **The history store** (SE-092 S8): records written atomically to the state
   directory, read back, and confirmed against a fresh capture — so a
   `--retain-source` re-run is recognised as done (INV-16).
2. **A single-instance lock and a concurrent-provider detector** (SE-015,
   `04` Concurrency): research first (`provider-researcher`: where each provider
   holds a lock, whether a stale one is distinguishable); the detector adds the
   `ConcurrentProviderActivity` unknown-value fallback.
3. **Re-read provider configuration immediately before each mutating step**
   (SE-016, the check-then-use window), refusing if it changed.
4. **The positional half of the spawn-site pin** (SE-050).
5. **A MacPorts-to-Homebrew recording that yields a plan** (INV-17, captured,
   never invented).
6. **The elevation consent path** (INV-06): per operation, enumerated, shown.
7. **A written proposal** for flipping `THIS_BUILD_DISPATCHES_MUTATIONS`, with
   `--retain-source` as the default for the first release (superseded: DP-048
   (c), ruled 2026-10-27 — the source is retained by default and removal is
   opt-in with `--remove-source`; work order 16 part L writes the proposal), into
   `docs/decisions-pending.md` for the owner.

## 6. Definition of done

- [ ] Each item above shipped with named tests proved by mutation, or carried
      in writing.
- [ ] Section 3 holds; the gate tests are unweakened.
- [ ] `/preflight`, `scripts/check-coverage.sh` and the reviewers run; findings
      resolved or accepted in writing.
- [ ] Every material change is in `docs/spec-evolution.md`.

## 7. End-of-pass report

`/pass-report` to `docs/passes/08-the-mutating-executor-report.md`.
