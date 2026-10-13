# MacPortsBrew — The executor, dry-run half (05-THE-EXECUTOR)

The repository can look at a machine, explain what it finds, and build a
migration plan. It cannot carry one out: there is no executor, and `enum Command`
offers `Status`, `Analyze`, `List`, `Explain`, `Plan`, `Doctor` and `Snapshot`.
The migration state machine exists as a serialisable `MigrationPlan` with
`check_ordering` enforcing INV-01, and `plan.v1.md` records five obligations an
executor inherits, two discharged as *data*. This pass writes the half of
`macportsbrew migrate` that cannot damage anything. The half that installs and
removes is a second pass, because the first time this project writes to a
machine deserves its own review.

**This work order was reviewed before implementation and rewritten against the
findings.** Four of its factual claims were wrong and are recorded in section 9,
because a pass that hides its own first draft teaches nothing.

---

## 1. Read this first

1. `AGENTS.md`, then `docs/spec/00-charter.md`, then
   `docs/spec/01-invariants.md`, then `docs/spec-evolution.md`, then
   `docs/spec/11-agentic-sdlc.md`.
2. `docs/schemas/plan.v1.md` — **"What an executor inherits"** is the debt list
   this pass exists to pay. Read it before designing anything.
3. `docs/spec/04-migration-and-safety.md` — process execution, the threat model,
   the concurrency MUST at `:221`.
4. `docs/spec/06-cli-and-ux.md` — the command surface and the `--dry-run` MUST.
5. `.claude/rules/adapters.md` — in particular "Never mutate, in any form",
   which is why section 5 does not put the executor there.

## 2. Authority and precedence

| Tier | Source | Changeable? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | No. If one seems wrong, **stop and ask the human.** |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. |
| 3 | **MUST** in `docs/spec/02..11` | Only with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Change freely; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

## 3. Hard boundary for this pass

**No mutating provider command is ever spawned.** Not behind a flag, not behind
a confirmation, not in a test.

**What actually holds that boundary, as opposed to what looks like it does.**
The first draft cited an "`adapters` no-mutation grep in `validate.sh`". There
is none. The mutating-verb grep lives at `.claude/hooks/stop-gate.sh:108`, scans
every `crates/*/src`, and matches **string literals** — while the verbs are
*data*, in `data/policy/provider-commands.toml` (`add = ["install"]`,
`remove = ["uninstall"]`). A run loop that hands every `PlanCommand` to a runner
contains no literal and the grep stays silent. So:

1. **The dispatch guard is the boundary.** `StepKind::is_mutating()`
   (`crates/domain/src/plan.rs:47`) already exists. The loop must refuse, by
   explicit branch, to hand any step it returns `true` for to a runner. The
   test constructs a plan containing `ADD_DESTINATION` and `REMOVE_SOURCE`,
   runs it against a recording runner, and asserts that runner received **zero**
   invocations for those steps.
2. `--dry-run` and `--yes` are **already** global flags
   (`crates/cli/src/app/mod.rs:58-63`) and `06-cli-and-ux.md:46` documents both,
   so a test asserting `migrate` "has no `--yes`" cannot be written. The honest
   check is reachability: `migrate --yes` against a recording produces no
   mutating invocation.
3. The executor's tests run against `fixtures/recordings/` through a recorded
   runner, never a live installation (INV-05). This protects the suite, not the
   shipped binary — say so rather than counting it twice.

**`dry_run_only` is not a boundary today.** It is written once
(`planner.rs:599`) and read for no decision anywhere. The first draft called it
"stronger than the greps"; that is backwards, for the reason `plan.v1.md:184`
already gives about `placeholder` — a boolean has no shape that fails closed on
a reader who never looks, and one JSON edit flips it. Making it *do* something
is work this pass owes, not a fact it may lean on.

**And the specification currently says nothing executes at all.**
`plan.v1.md:12`, `crates/domain/src/plan.rs:434` and `locales/en.toml:277` each
state it without qualification. The engineering position that the executor may
run *read-only* steps is sound — INV-02 says it runs plan steps and nothing
else, not that it may run none — but those three sentences become false the
moment it does, and must be revised **in the same change**, with a
`docs/spec-evolution.md` entry. `plan.v1.md:381-388` justifies staying at schema
v1 on the grounds that nothing deserialises a `MigrationPlan`; if `migrate`
does, that justification is gone and the version question reopens.

## 4. Working protocol

- **Verify, do not assume.** Observed, cited with a read date, or runtime
  detection. No fourth option. Four claims in this file's first draft failed it.
- **Never touch the developer's real package managers.**
- **Use the reviewers before implementing, not after.** This work order was
  rewritten by review; the code will be too.
- **Commit incrementally**, Conventional, one coherent change each.
- **When blocked on a human decision**, use `/decision-pending`.

## 5. Where the executor lives, and what it is allowed to hand a runner

**Not `crates/application/src/`.** INV-02's test
(`crates/application/tests/invariants.rs:704-722`) reads every file there and
fails on the *text* `ProcessRunner` or `std::process`, and `check-layering.sh:25`
pins the crate to `{domain, policy}`.

**Not `crates/adapters/` either.** `.claude/rules/adapters.md:83` says "This
layer **reads**", ties that to the Stop gate, and is a path-scoped rule loaded on
every touch of the subtree. Putting the executor there buys the dry-run half and
bills the mutating half for rewriting a safety rule. Worse for INV-17:
`check-layering.sh:70` scans only `domain` and `policy` for provider names, so
adapters is the one crate where a provider-specific arm in the run loop passes
every script.

**A new `crates/executor/`.** `check-layering.sh:34-37` fails the build on an
undeclared workspace crate, so it cannot be added silently; declaring
`{domain, adapters}` leaves every existing edge untouched, and
`check-no-shell.sh`, the layering `Command::new` grep and the Stop gate's scan
all cover `crates/*/src` automatically. Cost, stated: a row in AGENTS.md's
table, a `.claude/rules/executor.md`, one entry in `check-layering.sh`, and a
`spec-evolution.md` entry for the layering change.

**The blocker that placement does not solve.** `CommandSpec`'s fields are
`pub(in crate::process)`; `CommandSpec::provider` takes
`executable: &'static str` **on purpose** — "nothing a provider said, nothing in
a snapshot and nothing off the machine can decide which program runs"; and
`check-no-shell.sh` forbids `Box::leak` and `CommandSpec::new` outside
`process/`. `PlanCommand.program` is a runtime `String`. **So there is no legal
way today, in any crate, to build a `CommandSpec` from a `PlanCommand.**` The
pass must choose, with `security-reviewer` on the choice:

- **Re-derive** the program inside `process/` from the detected prefix and a
  literal, using the plan only for argv — preserving the guarantee, but running
  a command the executor reconstructed rather than the one the human reviewed,
  which is INV-02's subject. A plan whose `program` disagrees then becomes a
  refusal cause.
- **Add a constructor** in `process/` taking a plan command — a change to the
  module AGENTS.md calls "Dangerous to change", which must not accept
  `PlanCommand.environment` verbatim. `CommandSpec::provider` takes the
  environment unchecked today because every caller is an adapter that built the
  map itself; an executor reading a document moves INV-10's guarantee into the
  document.

**Ruled by the owner on 2026-09-27: re-derive (DP-023).** The program is
re-derived inside `process/` from the detected prefix and a literal; the plan
supplies argv only, and that argv still passes the spawn-site allowlist
(SE-050). A plan whose `program` disagrees with the re-derived one is a
refusal cause. This answers section 5's question only; section 6's item 1 —
where `migrate` gets its plan — is a separate decision.

## 6. Order of work

1. **Where `migrate` gets its plan.** Re-planned in process, or deserialised?
   It decides whether a `MigrationPlan` is deserialised for the first time in
   this workspace, whether `PlanCommand.environment` becomes untrusted input,
   and whether `dry_run_only` is a hardcoded `true` or a document field. Items 2
   and 4 cannot be written correctly before it.
2. **The refusal gate**, pure, in `application`. `check_ordering` (INV-01),
   `requires_manual_review` (INV-19), `steps_with_placeholders` (obligation 1),
   `verification_is_performable` (obligation 2), `steps_with_unknown_success`
   (obligation 4's *first* clause only), `steps_with_unknown_elevation` and
   `steps_requiring_elevation` (INV-06). **Refusing on elevation must not
   refuse rendering** — every MacPorts `add`/`remove`/`select` is `required`, and
   `04-migration-and-safety.md:37` requires `--dry-run` to *produce* every
   command that would run. Distinguish "refuse to execute" from "refuse to
   render" or `migrate --dry-run` is useless on one provider.
3. **Success evaluation.** Obligation 3's four conditions need a **fifth input
   the first draft missed**: the package name, which is on neither `Invocation`
   nor `MigrationStep`, and which name differs by step — `plan.package` for
   `STDOUT_OMITS_PACKAGE` on `VERIFY_REMOVAL`, `plan.destination_package` for
   `STDOUT_NAMES_PACKAGE` on `VERIFY_DESTINATION`. Getting it backwards tests one
   provider's name against the other's inventory, which is an INV-07/INV-08
   coupling bug; the signature must make the mix-up unrepresentable.
   `destination_package` is `Option`, and `None` on a `STDOUT_NAMES_PACKAGE`
   check is a refusal cause belonging in item 2, not a fallback. Note also that
   the two arms differ: `plan.rs:99-101` makes empty stdout fail
   `STDOUT_NAMES_PACKAGE` "however the process exited", so exit status is not a
   precondition there the way it is for the `OMITS` arm. Whether a *truncated*
   stdout containing the name satisfies `STDOUT_NAMES_PACKAGE` is specified
   nowhere; close it here.
4. **The run loop**, with the dispatch guard of section 3 as its first branch.
   `Invocation::require_success` is for **transport** failure only, never step
   success: `04-migration-and-safety.md:60` is a MUST NOT that "the install
   command exited 0" is not verification.
5. **The read-only concurrency flag**, not the lock. `04:221` conditions the
   lock on mutation ("Detect a concurrently running `brew` or `port` **before
   mutating**"), so in a pass with no mutation its refusal path is unreachable
   and untestable. What belongs here is the clause the same MUST ends with —
   read-only analysis proceeds "flagged as possibly inconsistent" — which
   `plan.v1.md:168` records as still owed (F-20). The lock lands with the
   mutation it guards. Note that detection needs a mechanism
   `PERMITTED_PROGRAMS` does not list, and a lockfile read collides with INV-15
   under `--replay`.
6. **History**, and the privacy rule: obligation 3 forbids recording the
   inventory stdout — it fingerprints the machine. Record the boolean, the line
   count and the truncation flag.
7. **The CLI surface** and `migrate.v1.md`, written in the same change.

## 7. Invariants this pass engages

The first draft named INV-01, 02, 05, 06, 07, 08, 15. A run loop also engages:

| Invariant | Why | Test |
|---|---|---|
| INV-03 | `04:184` is a MUST: partial success is its own exit code and history status | **write one** |
| INV-10 | first code to take an environment *from a document* into a real spawn | **write one** |
| INV-14 | history records timestamps; the clock is read in one place | exists — point at it |
| INV-16 | re-running a completed migration changes nothing and says so | **write one** |
| INV-17 | a run loop that special-cases a provider; see section 5 on why no script would catch it | **write one** |
| INV-19 | `MANUAL_REVIEW` halting the executor is new behaviour | **write one** |

INV-04 is correctly absent: reverse dependencies bind the removal path.

**Closed 2026-09-25 (F-53, SE-050)** — kept here as the record of what this
review found. **And one hole in the existing code, found by this review.** Nothing checks that
a verb appearing under `verify` or `verify_absent` is not also an `add` or
`remove` verb. `verify = ["install"]` loads clean, produces a
`VERIFY_DESTINATION` carrying an install command, and passes every predicate in
item 2. Under a loop that runs read-only steps, it spawns. The loader needs that
rule, and the runner's permitted-invocation check needs a subcommand dimension
beside `PERMITTED_PROGRAMS`. **This is the most important single check in the
pass.**

## 8. Definition of done

- [ ] Every one of `plan.v1.md`'s five obligations is implemented and pointed at
      its test, or restated there as still owed with the reason.
- [ ] A test per refusal, and a mutation proving each can fire.
- [ ] Obligation 3's conditions each have a case that fails when that condition
      alone is removed, and one for the source/destination name mix-up.
- [ ] The dispatch guard test of section 3 exists and the recording shows zero
      mutating invocations.
- [ ] The `verify`-is-not-a-mutating-verb loader rule exists, with a fixture
      that fails without it.
- [ ] `--dry-run` renders every command that would run, including the elevated
      ones, and mutates nothing — `04:37` also requires no `sudo` prompt.
- [ ] `spec-evolution.md` records: the layering change, the revision of
      `plan.v1.md:12` / `plan.rs:434` / `locales/en.toml:277`, and any new
      `ErrorId` (each needs a `message_id` in `locales/en.toml` and a row in
      `06-cli-and-ux.md`'s exit-code table).
- [ ] `.claude/rules/executor.md` exists and AGENTS.md's crate table has its row.
- [ ] `security-reviewer` and `spec-guardian` have run and their findings are
      resolved or accepted in writing.

## 9. What this work order got wrong before it was implemented

Recorded because the alternative is a pass that looks like it planned well.

1. "the CLI offers `analyze`, `doctor`, `explain` and `plan` and nothing else" —
   there are seven commands. A grep that matched four was read as a census.
2. "the `adapters` no-mutation grep already in `validate.sh`" — no such grep
   exists; the real one is a hook, scans all crates, and is blind to verbs that
   arrive as data.
3. "`migrate` having no `--yes`" — `--yes` is already a documented global flag.
4. "the boundary already exists as data, and that is stronger than the greps" —
   `dry_run_only` is read for no decision anywhere, and a deserialisable boolean
   is weaker than a compile-time absence.

Two further errors were structural rather than factual: obligation 3 was said to
need nothing the `Invocation` does not carry (it needs the package name), and
the executor was placed in a crate whose path-scoped rule says it only reads.
