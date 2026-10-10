<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — The first mutating release (11)

The owner has ruled on pass 08's flip proposal and set
`THIS_BUILD_DISPATCHES_MUTATIONS` (or has not — then this pass does not start).
This pass turns that build into v0.2.0: it runs the acceptance protocol on
disposable machines, makes every user surface say what the tool now changes,
and hands the owner a go/no-go. Tests in the repository still never touch a live
installation (INV-05); acceptance on a real machine is a **human-run protocol**
the agent writes, and whose results the owner reports back.

---

## 1. Read this first

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/00-charter.md`, `docs/spec/01-invariants.md`.
2. The flip proposal and its ruling in `docs/decisions-pending.md`;
   `docs/passes/08-the-mutating-executor-report.md`.
3. `docs/spec/04-migration-and-safety.md` (whole), `06-cli-and-ux.md`,
   `08-distribution-and-project.md`.
4. `docs/passes/09-release-engineering-report.md`,
   `10-read-only-release-candidate-report.md`.

## 2. Authority and precedence

As `TEMPLATE-PASS.md` §2.

## 3. Hard boundary for this pass

- The enabled step kinds are exactly those the owner's ruling names; a test pins
  them. Nothing in this pass widens them.
- No test, script or session command mutates a real Homebrew or MacPorts; the
  acceptance protocol is executed by the owner on a disposable VM and only its
  results enter the repository.
- Nothing is tagged or published by the agent.

## 4. Working protocol

As `TEMPLATE-PASS.md` §4.

## 5. Order of work

1. **Acceptance protocol** (`docs/passes/11-acceptance-protocol.md`): for each
   architecture and direction, a fresh VM state, the exact commands, what to
   capture (`doctor --report`, `history --json`, `snapshot`), the expected exit
   codes, and an abort-and-restore step for each failure. Cover: a simple
   formula/port, a package with dependents, a `STATEFUL`/`SERVICE` package (must
   stop at `MANUAL_REVIEW`, INV-19), a killed run (marker left, next run exit 4,
   `restore` path), two concurrent runs (exit 7), a provider mutating
   concurrently (as the detector ruling allows), a `brew.env`/`macports.conf`
   that outranks the invocation (refusal).
2. **Owner runs it**; the agent turns each reported failure into a fixture
   captured from the VM (`scripts/capture-fixture.sh`) and a failing test before
   any fix.
3. **User surfaces**: README, wiki, `--help`, release notes, CHANGELOG `[0.2.0]`
   say what is changed, what is never changed, how to undo, and that
   `--retain-source` is the default.
4. **Go/no-go memo** for the owner in the report: what passed, what failed, what
   is accepted, the recall plan.

## 6. Definition of done

- [ ] Protocol written, executed by the owner, every failure fixed with a
      captured fixture and test or accepted in writing.
- [ ] §3 holds; reviewers run on the full range; `/preflight`, `validate.sh`,
      coverage.
- [ ] Go/no-go memo delivered.

## 7. End-of-pass report

`/pass-report` to `docs/passes/11-first-mutating-release-report.md`.
