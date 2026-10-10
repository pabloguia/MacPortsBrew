<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 08, slice 3 — configuration re-read, the positional pin, the mirror recording

Work order for one session. Parent: [`08-the-mutating-executor.md`](08-the-mutating-executor.md),
items 3, 4 and 5; its §3 (hard boundary) binds here unchanged. Slice 2's work
order ([`08-slice-2-lock-marker-detector.md`](08-slice-2-lock-marker-detector.md))
is the model of shape and its §3 "How this repository bites" applies verbatim —
read it.

## 0. Read first, in this order

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/01-invariants.md` (INV-10, INV-15,
   INV-17 especially).
2. `docs/spec/04-migration-and-safety.md`: "Process execution", "Threat model",
   "Cross-provider contamination".
3. `docs/spec-evolution.md`: SE-016 (the check-then-use window), SE-050 (the
   spawn-site pin and its "positional half"), SE-092 (the slice 2 end-of-slice
   entry, for what the wiring owes), the DP-027 ruling and R-15/R-16.
4. `docs/passes/08-slice-2-plan.md` — the shape of a plan and of "As built".
5. Code: `crates/adapters/src/process/` (the only spawn site — dangerous to
   change), `crates/application/src/execution_gate.rs` (`ProviderConfiguration`
   refusal), the capture of provider configuration from pass 07,
   `fixtures/recordings/README.md` and `scripts/capture-fixture.sh`.

## 1. What this slice delivers

### Part A — the configuration re-read (item 3)

DP-027 (ruled): a mutating step refuses when a provider configuration that can
outrank the invocation is present (`brew.env`, a user `macports.conf` key).
Today that is decided **from the plan's capture**. Build the re-read: an
adapter-level function that reads the same configuration facts again, from the
filesystem only (no new process), and an application-level comparison that
refuses a step when they differ from the capture or now outrank the invocation.
It is **not wired** (dispatch is off); a tripwire in
`crates/cli/tests/history_preconditions.rs` (extend `wiring_violations`) fails
the build that flips the switch without calling it before each mutating
dispatch. Under `--replay`/`--from-snapshot` it is never called (INV-15).

### Part B — the positional half of the spawn-site pin (item 4)

SE-050: the long options are pinned per subcommand at the spawn site; the
positional arguments are "still pinned only by the gate". Pin them at the spawn
site too, inside `process/`, so a plan that passed the gate still cannot make
the executor run an argument shape it was not written for. Two-way tests per
verb (accepted shape, each refused shape), proved by mutation. **No new verb**
enters the allowlist; the allowlist does not grow.

### Part C — a MacPorts-to-Homebrew recording that yields a plan (item 5)

INV-17 needs the reverse direction exercised end to end. Capture, with
`scripts/capture-fixture.sh` (read-only; never invent a fixture), a recording
on which `migrate <pkg> --to homebrew` builds a full plan; add it to the fixture
matrix; add the CLI half of `invariant_17_retained_run_recognition_is_direction_neutral`
that slice 1 left waiting. If this machine cannot yield one, write down exactly
what state it would need and leave it owed — do not edit a recording by hand.

## 2. Hard boundary

As slice 2 §2: `THIS_BUILD_DISPATCHES_MUTATIONS` false; no live provider is
mutated; no elevation; no new dependency; `Cargo.lock` unchanged; guarded paths
untouched; the spawn allowlist does not grow; no gate test weakened.

## 3. Part D — plan, build, prove, review, record

Plan to `docs/passes/08-slice-3-plan.md`, reviewed by `spec-guardian` and
`security-reviewer` in parallel **before code**, findings cited `[SG Hn]`/`[SR Mn]`.
One Conventional Commit per step; every new rule unit-tested and proved by
mutation; invariant tests named `invariant_NN_*`; 04, SE-016, SE-050, SE-092 and
CHANGELOG in the same changes. End with the three reviewers on the full range,
`scripts/check-coverage.sh`, `scripts/validate.sh`, and an "As built" section.

## 4. Definition of done

- [ ] Parts A, B, C shipped with named tests proved by mutation, or carried in
      writing with the reason.
- [ ] Tripwires extended; §2 holds.
- [ ] Reviewers run; every finding fixed or accepted in writing.
- [ ] "As built" appended to the plan: commits, decisions, owed, for the owner.
