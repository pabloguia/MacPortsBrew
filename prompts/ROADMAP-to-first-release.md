<!-- SPDX-License-Identifier: Apache-2.0 -->

# Roadmap to a first distribution — the owner's run sheet

Written 2026-10-10, at the end of pass 08 slice 2; **revised 2026-10-22**, after
pass 14 merged and part A of pass 15 began. It orders the remaining work so that a user can **install and
use** a MacPortsBrew release as early as possible, and says, for each work
order, which model the owner should start the session with.

> **Why the model is named here and nowhere else.** SE-017 keeps work orders
> role-only: "the person starting a session decides who runs it." This file is
> that person's sheet, not a work order, so it may name models — and it is
> dated because model names date. The criterion is the one `CLAUDE.md` gives for
> subagents: **the judgement a task needs times the cost of being wrong.**
> Anything that can change a user's machine, weaken a gate or ship a supply-chain
> artifact is high on both. Re-read this table when a newer model family ships.

## Status on 2026-10-22

**Done:** passes 00–07, 09 (release machinery, staged), 10 (release candidate),
12, 13 and 14 (read-only improvements and maintenance; pass 14 merged as PR #11 on 2026-10-21), and pass 08 slices 1–5. The read-only build
is complete and reviewed. The mutating loop is wired behind a test-only
allowance; a run takes its snapshot and records itself; `history`, `restore
--plan` and the stranded-run `doctor` finding exist. CI ("Repository validation") has passed on `main` and on every topic branch of passes 13–14; the latest `main` run is 2026-10-21 (run 39578935218 at `ad5a95d`). On 2026-10-22 the merged topic branches were deleted and only `main` remained.

**Not done:**

- **v0.1.0 is not tagged.** Pass 09's guarded files are still staged, not
  installed (DP-033, DP-034, DP-035 and DP-045 were ruled 2026-10-23, so part B
  of pass 15 can be armed); the certificates and repository settings are the
  owner's.
- **Mutations stay off.** Eight tests fail the day the switch is turned on
  (`docs/passes/08-pre-flip-design-notes.md` has the table). Beyond them,
  every real plan halts at `VERIFY_DESTINATION` — four of its six checks
  cannot be evaluated by any build (DP-051) — and then at `SET_PRIMARY` and
  `VERIFY_RESOLUTION`, so a run can only end `PARTIAL` and nothing can be
  removed. Slices 6–9 close what they can, in that order; **DP-051 decides
  whether v0.2.0 can complete a run at all**.

## The strategy (unchanged): two releases, two tracks

1. **v0.1.0 — the read-only release** (track **R**): it forces the whole
   release pipeline to exist and be proven before a build that can change a
   machine travels through it.
2. **v0.2.0 — the first mutating release** (track **M**), only after the owner
   rules on the flip proposal. Its likely shape, if the owner takes the
   reversible defaults of DP-046..DP-048 and DP-051 (b): no elevated step,
   primary report-only, source always retained, four destination checks
   reported as not checked — which means MacPorts→Homebrew formulae only, and
   nothing for the Intel Homebrew users the project started for. That is a real
   product decision, with an INV-17 and charter question in it; read those
   entries.

## The sequence from here

| # | Work order | Track | Can start | Model | Why that model |
|---|---|---|---|---|---|
| 1 | [`14-read-only-maintenance.md`](14-read-only-maintenance.md) | R | **done** (merged 2026-10-21) | **Sonnet**, reviewers on **Opus** | Research, data, docs, additive reads and tests; the executor, planner and allowlist are out of bounds. Item B (`node@20`, whose disable date the 2026-10-12 read gives as 2026-10-28) is time-sensitive. |
| 2 | [`15-first-tag-readiness.md`](15-first-tag-readiness.md) | R | **part A in progress** on `pass/15-first-tag-readiness` (2026-10-22); part B after DP-033/034/035/045 | **Sonnet** for part A; **Opus** preferred for part B, or Sonnet with `security-reviewer` on Opus over every guarded file | Part A is reconciliation and docs truth. Part B writes the release gates and workflow in one armed message: a weakened gate ships to every user. |
| — | **Owner: certificates, environment, ruleset, dry run, tag v0.1.0** | R | after 15 | — | Only the owner handles credentials and tags. |
| 3 | [`08-slice-6-verification-before-removal.md`](08-slice-6-verification-before-removal.md) | M | after 14's R-20 note | **Opus** recommended; Sonnet only under the rules below | Planner, gate, verification and executor (INV-01): by this file's own criterion, the stronger model. The design is written; the risk is a fail-closed rule subtly wrong. |
| 4 | [`08-slice-7-plan-tie-and-consent.md`](08-slice-7-plan-tie-and-consent.md) | M | after 6 merges; part A after DP-049 | **Opus** recommended; Sonnet only under the rules below | Consent wiring and the plan tie in the executor; INV-02, INV-06. |
| 5 | [`08-slice-8-primary-and-resolution.md`](08-slice-8-primary-and-resolution.md) | M | after 7 merges (stop if DP-047 is ruled (a)) | **Opus** recommended; Sonnet only under the rules below | Planner, gate and executor; changes what a run can complete. |
| 6 | [`08-slice-9-detector-and-flip-proposal.md`](08-slice-9-detector-and-flip-proposal.md) | M | after 8 merges and DP-030/028 | **Opus** | Implements the detector ruling and writes the proposal the owner flips the switch on; judgement-heavy, irreversible in effect. |
| 7 | [`11-first-mutating-release.md`](11-first-mutating-release.md) | M→R | after the flip | **Opus** for the go/no-go; **Sonnet** may draft the acceptance protocol and docs | Acceptance on a disposable machine and the release decision. |

Rows 1 and 2 both edit `CHANGELOG.md`, `README.md` and `docs/spec-evolution.md`, so run them one after the other; rows 3–6
must run one at a time, because they all change the executor.

### Rules when a mutating-track slice runs on Sonnet

Each M work order already requires them; they are repeated here because they
are what makes the cheaper model acceptable at all. They lower the cost of being
wrong; they do not change that slices 6–8 are judgement-heavy.

1. **Plan mode first**, and the plan reviewed by `spec-guardian` and
   `security-reviewer` **on Opus** (the Agent tool's `model` parameter) before
   any code.
2. **The same two reviewers on Opus after every part's commit**, not only at the
   end; a finding is resolved before the next part starts.
3. **The mutation proof is not optional**: a subagent in an isolated worktree
   applies the mutants the work order lists, and the session reads every
   survivor.
4. **Stop conditions are honoured**: when a work order says "stop" or "write a
   DP", the session does that rather than finding a way round.
5. The owner reads the pull request's diff of `crates/*/src/executor*`,
   `execution_gate.rs`, `planner.rs` and `verification.rs` before merging.

**Mechanical helpers** (fixture regeneration, a typo sweep, a read-only
`doctor` check) can go to Sonnet or Haiku subagents from inside any session.

## What only the owner can do

| Item | Blocks | Notes |
|---|---|---|
| ~~Rule DP-033, DP-034, DP-035, DP-045~~ **Ruled 2026-10-23**: DP-033 (b) the `.pkg`/`.dmg` carry `mpb`; DP-034 (b) first, (a) if it fails; DP-035 `io.github.pabloguia.macportsbrew` + `~/.macportsbrew/bin`; DP-045 (a). **DP-036**: confirm (default holds) | 15 B, the tag | **DP-034 closed 2026-10-23: `security -i` held on all four runners** (R-19 addendum); the signing step takes that recipe in part B. |
| Send the armed message for work order 15 part B (`MPB_ALLOW_GUARD_EDIT=1` at the start, in your own words) | 15 B | It covers that one message (SE-093). |
| Apple Developer Program; **Developer ID Application** and **Installer** certificates; an App Store Connect API key; the `release-signing` environment with its 7 secrets and 3 variables; the `v*` tag ruleset; immutable releases | the tag | Checklist in `docs/passes/09-staged/README.md`; never in a file, never in a session. |
| The dispatch dry run, the Standard-account `.pkg` check, the Gatekeeper protocol | the tag | `docs/passes/10-read-only-release-candidate-report.md`. |
| Rule **DP-046** (elevated steps), **DP-047** (primary mechanism), **DP-048** (removal window), **DP-049** (`sha2` and `--plan-digest`), **DP-050** (source builds), **DP-051** (the four unevaluable destination checks) | DP-049: slice 7 part A; DP-047: slice 8; DP-046, DP-048 and DP-051: slice 9 and the flip; DP-050: nothing (slice 6 reads it) | DP-046 and DP-047 contain an INV-17 and charter question that only you can answer. DP-049 has no default for its part A; DP-051 has none at all. |
| Decide DP-019's remaining sub-question: Intel artifacts built per commit or per release | 15 (the release workflow) | Default: per release. Cost of the Intel runner. |
| Rule **DP-030** (detector) and **DP-028** (INV-10 wording); DP-029 when convenient | slice 9 | Recommendation for DP-030: (a) now, (b) at the flip. |
| Arm work order 14 item J (skills wording, generator) if wanted | 14 J | Optional. |
| A second captured machine (arm64), and a disposable macOS VM per architecture | slice 8 (optional), pass 11 | Fixtures are captured, never invented; acceptance is a human-run protocol. |

## How to start each session

```text
Você vai executar <work order>. A ordem de trabalho completa está em
prompts/<file>.md — leia-a inteira antes de qualquer coisa, depois os
documentos da seção de leitura dela, na ordem. Trabalhe de forma autônoma até a
Definition of Done e, no fim, me dê um resumo: commits, decisões, o que ficou
devido e o que precisa de mim.
```

Set `.claude/current-pass` to the work order's file name without `.md`
(for example `14-read-only-maintenance`) before the session; the
session-start hook announces `prompts/<that>.md` as the work order. When a
session runs on Sonnet, add one more sentence to the prompt above, asking it to
run `spec-guardian` and `security-reviewer` with `model: opus` on the plan and
after every part.

## Remaining work after the rulings of 2026-10-23

In order. Nothing here needs another decision.

1. ~~DP-034 research~~ **Done 2026-10-23** (R-19 addendum; `security -i` held on four
   runners; pull request #14 closed unmerged). *Yours:* delete the remote branch
   `research/dp034-security-stdin`.
2. ~~Pass 15 part B~~ **Written 2026-10-23, pending the merge of #13** (pull request #13, commits `fbc270b^..c40a920`;
   `docs/passes/15-first-tag-readiness-report.md`). The release machinery is installed,
   `mpb` is a byte-identical copy in the `.pkg`, the `.dmg` and as a bare file, and the
   signing step uses `security -i`. *Yours:* review and merge #13.
2a. ~~Follow-ups from part B's reviews~~ **Written 2026-10-23** (armed message; pull
   request #13): the sign-path and hash-binding gate tests, `check-no-secrets.sh` over
   untracked files and two EC key forms with the git environment cleared, the
   `RUNNER_TEMP` assertion and the `mpb` fields in `SIGNING-RECORD.txt` (DP-052 (a)),
   the matrix banner and the "never run" comments.
2b. **Follow-ups from the 2a review, one armed message, before the tag** (none High):
   - **the sign-path test is vacuous on arm64** (two of the four CI runners): a removed
     guard falls to the "host is not x86_64" branch and nothing runs; add a `uname` stub
     and a canary case that deletes the guard and must see the input executed;
   - widen the text check (any variable in command position, other copies of the bytes),
     and let the `codesign` stub succeed so the run reaches the signing half;
   - `check-no-secrets.sh`: name rules over ignored files too (`git add -f <key>`), the
     OpenSSL P-256 (`MIGH…`) and P-384 PKCS#8 forms, `GIT_CONFIG_*` cleared, and untracked
     hits labelled as such;
   - `test-gates.sh`: clear the git environment and disable hooks on its throwaway commits.
3. **Small, unguarded, any time:** reword the top-level `--help` line for
   `history` ("Reads only") and its pt-BR twin; add an i18n-parity check.
4. **Optional, as one armed message:** the `.claude/` fixes in
   `docs/research/tooling/2026-10-22-*.md`.
5. **Owner, after 2 (2 written 2026-10-23), the tag after #13 merges:** certificates, the `release-signing` environment, the
   `v*` tag ruleset, immutable releases, the dispatch dry run, the Standard-account
   `.pkg` check, the Gatekeeper protocol, DP-036, the tag.
6. **Release 0.2.0 track (pass 08 slices 6–9):** unchanged; DP-046 to DP-051 are
   still open.
