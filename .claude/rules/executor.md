---
paths:
  - "crates/cli/src/executor/**"
  - "crates/application/src/executor.rs"
  - "crates/application/src/executor/**"
  - "crates/application/src/execution_gate.rs"
  - "crates/domain/src/verification.rs"
---

# The executor — the gate, the guard, and nothing it does not have to do

Read `docs/spec/04-migration-and-safety.md` ("Process execution", "Threat
model", "Concurrency and locking") and `docs/schemas/plan.v1.md` ("What an
executor inherits" — the twelve obligations) before changing anything here, and
SE-092 in `docs/spec-evolution.md` for why the executor is split the way it is.
This file names the boundaries a diff in this subtree can quietly cross; it does
not restate the chapters.

## Two halves, and which one decides

`crates/application/src/executor.rs` decides — every choice is a pure function
of the plan, the snapshot it was built from and the reads obtained.
`crates/cli/src/executor/` is the I/O shell: it obtains reads and hands them
over, and the only things it still decides are sequencing (SE-092). A decision
that drifts into the shell is a decision no unit test sees.

## The rules the module comments state, and the tests that hold them

1. **The gate first, structurally.** `run` takes the registry context, not read
   sites, consults `execution_refusals` and builds sites only when the gate
   passed the plan. A word scan keeps the read-site constructors in the shell.
2. **The guard before everything else about a step.** `next_action` never
   answers `Dispatch` for a mutating step, for a read whose elevation is not
   `NONE`, or for any step of a plan carrying `MANUAL_REVIEW`. The shell never
   reads `THIS_BUILD_DISPATCHES_MUTATIONS` or the plan's own claim about it.
3. **No command spec is built here.** A read goes through the adapters' read
   sites, which re-derive program and environment from the detected prefix and
   check argv against the shipped allowlist (DP-023, SE-050).
4. **Success is judged, never inferred from an exit status** — against the
   snapshot the plan arrived paired with (`PlannedRun`).
5. **Nothing a read printed is kept raw**; kept texts are home-redacted and
   escaped.
6. **No provider is named** here, in code or in prose (INV-17).
7. **No mutating verb, elevation, link or launch token as a literal** (the
   `Stop` gate's scans).

## The gate reads only the plan

`execution_refusals` is pure and reads nothing but the plan and the compiled
command table. A fact the gate must act on travels *in* the plan: a provider's
own configuration that outranks the invocation is a `provider_configuration`
risk the planner writes, and the gate refuses every mutating step against that
provider from it (DP-027 (b)). Adding an input to the gate's signature is a
design change for SE-092, not a refactor.

`ExecutionRefusal::key` is an exhaustive ordering; a new variant takes the next
ordinal, and the gate's tests in `crates/application/tests/execution_gate.rs`
match every variant so a new one fails to compile there until it is classified.

## Dangerous to change

`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false` until a pass whose work order
says otherwise, and
`every_plan_this_planner_builds_is_refused_for_execution_in_this_build` and its
fixture counterpart are never weakened to make a change pass. Run
`security-reviewer` and `spec-guardian` before reporting a change here complete.
