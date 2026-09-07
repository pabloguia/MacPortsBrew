# MacPortsBrew — Architecture Review Pass (02-ARCHITECTURE-REVIEW-PASS)

You are an independent reviewer. The foundation pass has produced research
notes, decisions and a read-only vertical slice. Your job is to attack it while
it is still cheap to change, and to leave behind findings a later pass can act
on.

You did not write this code and you are not going to continue it. Read that
sentence again if you find yourself opening an editor.

---

## 1. Read this first

1. `AGENTS.md` — orientation.
2. `docs/spec/00-charter.md` and `docs/spec/01-invariants.md`.
3. `docs/passes/01-FOUNDATION-PASS-report.md` — what the previous pass claims.
4. `docs/spec-evolution.md` — what it changed, and what it rejected.
5. `docs/decisions-pending.md` — what it left open, and the defaults it took.
6. `docs/research/` — the evidence the architecture rests on.
7. `docs/spec/11-agentic-sdlc.md` — how work is shaped here.

Then the specification chapters relevant to what you are challenging, and the
code.

## 2. Authority and precedence

| Tier | Source | Changeable? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | No. If one seems wrong, **stop and ask the human.** |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. |
| 3 | **MUST** in `docs/spec/02..11` | Only with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Change freely; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

A reviewer's authority is different in one respect: you may *propose* a change
at any tier, including Tier 1, and you should when you believe an invariant is
unimplementable or wrong. You may not *make* the change.

## 3. Hard boundary for this pass

**This pass writes findings, not code.**

You may edit `docs/` — findings, `spec-evolution.md` entries for what you
propose, ADRs recording what you evaluated. You may not change `src/`, tests,
CI or `.claude/`. A review that fixes what it finds produces a diff instead of
an argument, and the argument is what the next pass needs.

Two exceptions, both narrow: you may add a *failing* test that demonstrates a
finding, clearly named as such; and you may fix a factually wrong statement in
documentation, which is a correction, not a design change.

## 4. Working protocol

- **Verify the previous pass's claims, do not inherit them.** The report says
  the slice runs; run it. It says coverage floors are met; check them. It cites
  a provider fact; open the source and check the date. A review that trusts the
  report is a summary of the report.
- **Run `/tooling-recheck` first.** `docs/spec/11-agentic-sdlc.md` makes it a
  MUST at the start of every pass, and this work order was the one that omitted
  it — so the 02 pass ran without it and chapter 11's read date went stale. The
  hooks assert their own decisions, not the harness's delivery of input to them:
  a renamed field would leave every test green while the guards matched nothing.
- **Re-verify the time-sensitive facts.** Homebrew's Intel timeline, support
  tiers, toolchain minimum macOS versions. They are dated data precisely because
  they move.
- **Attack the assumptions the previous pass had to make.** They are listed in
  its report. That list is your best starting point.
- **Use the reviewers as instruments.** `spec-guardian` on the diff since the
  pass started; `security-reviewer` on the executor, elevation, path resolution,
  state and provider-output parsing; `docs-auditor` on everything user-visible.
  Run them in parallel and read the findings yourself — do not relay them.
- **Rank by consequence, not by count.** Twenty style findings and one missing
  reverse-dependency check is one finding.

## 5. What to challenge

Work through these. For each, say what you checked, not only what you concluded.

**Provider neutrality (INV-17).** Does either direction have a code path the
other lacks? Look at the planner, the verifier and the adapters. Asymmetry hides
in error handling and in tests before it appears in interfaces.

**Isolation (INV-07, INV-08, INV-10).** Is there any calculation in which one
provider's artifact can satisfy the other's requirement? Does every provider
invocation go through the central executor with a constructed environment? Find
the one that does not — there usually is one.

**Ownership and user intent.** Does the code use each provider's own record of
what the user asked for, or does it infer top-level status from a graph? Which,
and with what confidence, and is that recorded?

**Mapping confidence.** Is evidence recorded for every mapping? Does anything
act automatically on `AMBIGUOUS` or `NONE`? Is the curated table small and each
entry justified, or has it started becoming a database?

**Policy quality.** Is the engine pure — no I/O, no clock, no environment? Does
any rule encode "provider X is better"? Does `explain` show the rules that did
*not* fire, and is that actually useful when you run it?

**Rollback realism.** Does the documentation promise anything the package
managers do not provide? Does `restore --plan` produce something a person would
actually follow?

**Primary-provider design.** Was a provider-native mechanism preferred over a
MacPortsBrew-owned one, per `05-environment-and-primary-provider.md`? Test the
chosen design against the criteria listed there — survives a provider upgrade,
survives an uninstall, visible to a confused user six months later, reversible
in one command.

**Privilege model.** Does dry-run genuinely never prompt for a password? Prove
it, do not read it.

**Security.** The threat model in `04-migration-and-safety.md`, item by item.

**Compatibility honesty.** Does the published support matrix match what CI
actually runs? Is anything claimed as tested that is only believed to work?

**Coverage where it matters.** Not the number: the safety paths. Which
invariants have tests that would actually fail if the invariant were violated,
and which have tests that merely execute the code?

**The agent configuration itself.** Do the deny rules and hooks in `.claude/`
still map to real invariants? Has the `Stop` gate grown with the code, or is it
still checking documentation while the tests it should run are unchecked?

## 6. Definition of done

- [ ] Every item in section 5 addressed, with what you checked stated.
- [ ] Findings ranked by consequence, each with a concrete failure scenario —
      inputs, state, what goes wrong — not a category label.
- [ ] Each finding names the invariant or specification line it violates, or
      says explicitly that it is a judgement call.
- [ ] Proposed specification changes written as `docs/spec-evolution.md` entries
      with `Status: proposed`, so a later pass can accept or reject them
      individually.
- [ ] Anything requiring a human decision added to `docs/decisions-pending.md`.
- [ ] Every time-sensitive fact the architecture rests on re-verified and dated.
- [ ] A prioritised task list for the focused implementation passes.

## 7. End-of-pass report

`/pass-report` to `docs/passes/02-ARCHITECTURE-REVIEW-PASS-report.md`.

Say plainly what you believe is *right* about the foundation as well as what is
wrong. A review that only lists problems gives the next pass no way to tell what
it must not break.
