# Work orders

One file per pass. A work order is the session prompt: what to read, what
authority you have, what this pass must not do, and how it is accepted.

| File | Pass |
|---|---|
| [TEMPLATE-PASS.md](TEMPLATE-PASS.md) | The template. Copy it; do not improvise a new shape. |
| [00-REPO-READINESS-PASS.md](00-REPO-READINESS-PASS.md) | Ran once, before any code: deep specification and tooling review, documentation consolidation, repository readiness. Completed 2026-09-02; report in `../docs/passes/`. |
| [01-FOUNDATION-PASS.md](01-FOUNDATION-PASS.md) | Research, the domain, the read-only vertical slice. |
| [02-ARCHITECTURE-REVIEW-PASS.md](02-ARCHITECTURE-REVIEW-PASS.md) | An independent reviewer challenges the foundation. Reviewing, not coding. |
| [03-FOCUSED-IMPLEMENTATION-PASS.md](03-FOCUSED-IMPLEMENTATION-PASS.md) | One independently verifiable capability. Instantiated per task. |
| [03-unknown-is-never-a-yes.md](03-unknown-is-never-a-yes.md) | First instance of the template. An answer that carries no information never reaches a plan as a yes: a per-provider success semantic on every plan command (F-33), and a loaded snapshot that does not record the service question refuses to plan (F-29). Completed 2026-09-18. |
| [04-closing-the-register.md](04-closing-the-register.md) | Closes the architecture review's findings register. Report in `../docs/passes/`. |
| [05-the-executor.md](05-the-executor.md) | The executor's dry-run half: `migrate`, the execution gate, the read-step loop; nothing changes the machine. Completed 2026-10-01. |
| [06-hardening-before-mutation.md](06-hardening-before-mutation.md) | Closes what pass 05 found before any mutation may land: per-subcommand long-option allowlist, `brew.env` research, one hostile-text definition, the concurrency flag on `plan` and `analyze`, the "None match" wording. Completed 2026-10-03. |
| [07-owner-rulings-and-housekeeping.md](07-owner-rulings-and-housekeeping.md) | Implements the owner's rulings on DP-025, DP-026 and DP-027, answers R-16, and clears pass 06's maintenance list. Mutation stays off. Completed 2026-10-04. |
| [08-the-mutating-executor.md](08-the-mutating-executor.md) | What a mutating step needs before one may be dispatched: history store, lock and detector, configuration re-read, the positional pin, a mirror recording, elevation consent, and a proposal to the owner. Dispatch stays off. |
| [08-slice-2-lock-marker-detector.md](08-slice-2-lock-marker-detector.md) | Pass 08, slice 2: the run lock and the interrupted-run marker; the detector recorded as owed (DP-030). Completed 2026-10-05. |
| [08-slice-3-config-pin-mirror.md](08-slice-3-config-pin-mirror.md) | Pass 08, slice 3: configuration re-read, the positional spawn-site pin, a MacPorts-to-Homebrew recording that yields a plan. |
| [08-slice-4-wiring-and-consent.md](08-slice-4-wiring-and-consent.md) | Pass 08, slice 4: the run loop wired behind the switch, proved behaviourally; elevation consent. |
| [08-slice-5-history-and-restore.md](08-slice-5-history-and-restore.md) | Pass 08, slice 5: `history`, `restore` (plan only), the stranded marker in `doctor`. |
| [08-slice-6-detector-and-flip-proposal.md](08-slice-6-detector-and-flip-proposal.md) | Pass 08, slice 6: DP-030 as ruled, and the written proposal to turn mutations on. |
| [09-release-engineering.md](09-release-engineering.md) | Build, sign, notarise, package and checksum per architecture; the release workflow, proven unsigned. |
| [10-read-only-release-candidate.md](10-read-only-release-candidate.md) | v0.1.0: the read-only build made honest and worth installing. |
| [11-first-mutating-release.md](11-first-mutating-release.md) | v0.2.0: acceptance on disposable machines and the owner's go/no-go. |
| [12-read-only-improvements.md](12-read-only-improvements.md) | After v0.1.0: pt-BR, the dual-Homebrew `doctor` finding, defaulted-ownership evidence, curated mappings, two test fixes. |
| [ROADMAP-to-first-release.md](ROADMAP-to-first-release.md) | Not a work order: the owner's run sheet — order, owner prerequisites, and which model to start each session with (SE-017 keeps model names out of work orders). |

`.claude/current-pass` names the active one in a single line. The session-start
hook reads it and announces it; keep it accurate.

The number is the order of execution and nothing else: `00` ran once and will
not run again, `01` runs next, `02` reviews what `01` built, `03` is copied per
task after that. Pass reports use the same id (`../docs/passes/<id>-report.md`),
so a reader can pair every work order with what it produced.

## Conventions

**Passes are roles, never model names.** Model names date faster than the
repository and end up embedded in it (SE-017). A work order names the pass; the
person starting the session decides who runs it.

**A work order points, it does not restate.** The specification lives in
`../docs/spec/`, tiered and loadable file by file. A prompt that pastes it back
in is a copy that will drift, and drift in a safety specification is how a
promise quietly becomes false. If a work order and the specification disagree,
the specification wins and the work order is the bug.

**Every work order has these seven sections**, in this order:

1. **Read this first** — an ordered list, shortest sufficient set.
2. **Authority and precedence** — the tier table, unchanged between passes.
3. **Hard boundary** — what this pass must not do, stated so it is checkable.
4. **Working protocol** — the rules that apply while working.
5. **Order of work** — the sequence, where sequence matters.
6. **Definition of done** — demonstrable, not assertable.
7. **End-of-pass report** — what the handover must contain.

**The hard boundary is the part to get right.** "Prefer not to" is not a
constraint an agent can be held to. Write the boundary so a `grep`, a hook or a
test can decide whether it held. The foundation pass's boundary is the model:
*no mutating code path exists in the binary this pass produces* — checkable, and
therefore an acceptance criterion rather than an intention.

**Boundaries live in two places on purpose.** A work order states them so the
agent understands them; `.claude/settings.json` and `.claude/hooks/` enforce the
mechanical subset so understanding is not the only thing standing between the
project and a mistake. See `../docs/spec/11-agentic-sdlc.md`.

## Starting a pass

The operator's command block is in `../.claude/README.md` ("Running a
pass"); the normative statement is `../docs/spec/11-agentic-sdlc.md` ("The
pass protocol"). In short: set `.claude/current-pass`, start in plan mode,
`/tooling-recheck`, plan and have it reviewed, work incrementally with
research and review delegated, `/preflight`, `/pass-report`.

## Finishing one

A pass is finished when its report is written to `../docs/passes/` and every
material deviation it names already exists in `../docs/spec-evolution.md`.
Update `.claude/current-pass` to the next pass, or to `none`.
