<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 4 — wiring the run loop, and elevation consent

Work order for one session. Parent: [`08-the-mutating-executor.md`](08-the-mutating-executor.md),
item 6, plus the wiring every earlier slice owed. Its §3 binds here unchanged;
slice 2's §3 "How this repository bites" applies verbatim. **This is the most
safety-critical slice of the project**: it decides, in code, the order in which
a mutating run probes, locks, marks, dispatches, records and clears.

## 0. Read first, in this order

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/01-invariants.md` — INV-01, 02, 03, 06,
   15, 16, 19.
2. `docs/spec/04-migration-and-safety.md`: State machine, The plan is the
   contract, Verification contract, Concurrency and locking (Status), History,
   Privilege, Threat model.
3. `docs/spec-evolution.md`: SE-092 — the slice 2 entries' **"Owed with the
   wiring"** lists are this slice's backlog; SE-015; SE-016; SE-050.
4. `docs/passes/08-slice-2-plan.md` and `08-slice-3-plan.md` ("As built").
5. Code: `crates/cli/src/executor/mod.rs`, `crates/application/src/executor.rs`,
   `execution_gate.rs`, `crates/domain/src/verification.rs`,
   `crates/adapters/src/persistence/{history_store,run_lock,run_marker}.rs`,
   `crates/cli/tests/history_preconditions.rs` (the tripwires this slice
   satisfies).

## 1. What this slice delivers

### Part A — the wiring, behind the switch

Wire the executor shell so that, **when** `THIS_BUILD_DISPATCHES_MUTATIONS` is
true, a mutating run does exactly: gate → `history_store::probe` →
`run_lock::acquire` → re-run the state-dependent part of the gate **under the
lock** (a fresh capture; refuse if the plan no longer holds) → `run_marker::read`
(refuse, exit 4, naming the run) → `run_marker::write` immediately before the
first step that can change a provider → per mutating step: `verify_still_current`,
the configuration re-read (slice 3), dispatch, verify → `history_store::write_record`
on every ending → `run_marker::clear` (surface `NotOurs`) → release. Snapshot
and record steps leave `NotPerformedInThisBuild` here.

Because the switch stays off, prove the wiring **behaviourally**: an
application-level run with a recording fake `ProcessRunner` and a scratch state
directory, driven through a test-only entry that forces the mutating path on
*inside the test binary only* (never a feature flag in the shipped binary),
asserting call order, that nothing dispatches after a failed probe, lock, marker
write or record, and that every ending clears or keeps the marker as SE-092
D-M1 says. Declare in SE-092 how the order rule treats read steps before the
first mutating step (slice 2 left that open). `wiring_violations` must pass with
the flag forced true in a scratch copy — record that run.

### Part B — elevation consent (INV-06)

Per operation, enumerated, shown, confirmed; nothing elevated without it; the
process is never wholly root. Today the gate refuses `ElevationRequired` and
`ElevationUnknown`; build the consent path (types, rendering through the i18n
catalogue, a non-interactive refusal when stdin is not a TTY, `--yes` never
implying elevation consent) and keep the gate refusing until the consent is
wired. No code in this slice runs an elevated command; the guards block the
word anyway. MacPorts' version selection stays "proposed, never run" (SE-036).

## 2. Hard boundary

Slice 2 §2, plus: the switch stays `false` in every shipped build (a test pins
the constant); the test-only forcing entry is `#[cfg(test)]` or lives under
`crates/*/tests/` and is unreachable from `main`; no elevated command is
executed or constructed in shipped source.

## 3. Part C — plan, build, prove, review, record

Plan to `docs/passes/08-slice-4-plan.md`; `spec-guardian` and
`security-reviewer` review it **before code**. Mutation-prove every rule and
every ordering edge (swap two calls; drop one; fail each in turn). Docs in the
same changes (04, `plan.v1.md` obligations, SE-092, CHANGELOG). End with all
three reviewers, coverage, `validate.sh`, "As built".

## 4. Definition of done

- [ ] The behavioural run-order test exists and kills every ordering mutant.
- [ ] Every "Owed with the wiring" item in SE-092 is done or carried with a
      reason.
- [ ] Elevation consent exists, is tested, and the gate still refuses elevated
      steps until it is wired to dispatch.
- [ ] The mutating dispatch takes the `ConfigurationRechecked` token by value
      and refuses one whose `provider()` is not the step's; the re-read passes
      the captured `home`, never one read from the environment; the two
      elevated-step residuals in 04 (Provider configuration) are closed or
      carried with a reason.
- [ ] The CLI half of `invariant_17_retained_run_recognition_is_direction_neutral`
      exists (SE-092, the mirror-fixture addendum).
- [ ] `PROVIDER_REPORTS_ACTIVE` asserts what its name says: the destination is
      the provider's *active* installation, not merely installed (SE-094; today
      it is evaluated from an "installed" read, which is harmless only while
      the gate refuses every plan).
- [ ] §2 holds; no gate test weakened; reviewers run; "As built" written.
