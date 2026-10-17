<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 4 — wiring the run loop, and elevation consent: report

Work order: [`prompts/08-slice-4-wiring-and-consent.md`](../../prompts/08-slice-4-wiring-and-consent.md).
Branch `pass/08-slice-4-wiring-and-consent`, from `main` at `428df05`.
`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`. Run unattended overnight on
2026-10-17 at the owner's request ("evolua tudo que puder sem minha decisão").
Nothing is merged, tagged or released by this session; it opens a pull request.

## Progress

- Plan to `docs/passes/08-slice-4-plan.md`: done (revision 3)
- Plan reviewed by `spec-guardian` and `security-reviewer` before code: done,
  twice (§7 and §8 of the plan)
- Part A, the wiring behind the switch, with the behavioural run-order test:
  done (`crates/cli/tests/executor_wiring.rs`, both directions)
- Part B, elevation consent (gate still refusing): done, not wired
- Owed items from SE-092 done or carried: done or carried, each carried item
  behind a flip tripwire (plan D14, D15)
- Mutation proof of every rule and ordering edge: todo
- Docs (04, `plan.v1.md`, SE-092, CHANGELOG), final reviews, coverage, "As built": todo
