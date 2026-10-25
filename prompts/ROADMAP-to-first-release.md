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
2b. ~~Follow-ups from the 2a review~~ **Written 2026-10-24** (armed message; pull
   request #13): the sign-path test stubs `uname` and opens with a canary that must
   see the input run when the guard is removed, so it is a real test on arm64 too; the
   text check covers every spelling of running the input and any copy of its bytes;
   `check-no-secrets.sh` reads ignored names, the P-256/P-384 PKCS#8 and SEC1 key forms,
   clears `GIT_CONFIG_*`, and labels untracked and ignored hits; `test-gates.sh` clears
   the git environment and runs no hook on its throwaway commits.
2c. ~~Follow-ups from the 2b review~~ **Written 2026-10-24** (armed message; pull
   request #13): the sign-path text check is an allowlist (a quoting-aware scanner;
   a command naming the input under its known names may start only with a listed
   tool; exactly two runs, in the guard's `else`), 22 escaping spellings each
   refused; `check-no-secrets.sh` walks ignored files except
   the build directories, so a key inside an ignored directory is refused, and its
   name-rule advice no longer leads to a second refusal; `test-gates.sh` reads no
   developer git configuration.
2d. ~~Follow-ups from the 2c review~~ **Written 2026-10-24, committed with 2e and 2f.** The sign-path scanner reads `build-release.sh` as one stream
   (quotes, `$(`, backticks, `${...}`, `$((...))`, process substitution and
   here-documents carried across lines, if/fi depth for the guard's `else`); every
   command's first word is on an allowlist or is one of the script's functions;
   commands naming the input, a copy, a positional parameter inside a function, or a
   name assigned one of those, start only with tools that run nothing; `tar`,
   `xcrun`, `find`, `command`, `trap` held to safe forms; no `PATH`/`DYLD_` assignment.
   53 spellings refused, the review's 18 among them. `check-no-secrets.sh
   --build-dirs` walks `target/`, `dist/`, `.token-pilot/` for the name rules;
   `validate.sh` passes it outside `--quick` (the commit gate and CI). The gate
   tests isolate `XDG_CONFIG_HOME` and the git template directory.
2e. ~~Make the sign-path guarantee dynamic~~ **Written 2026-10-24, committed with 2f**
   (its review found two Highs, answered by DP-053 (c) and 2f). `test-gates.sh` runs the whole signed path of
   `build-release.sh`, and the `--from-binary` dry path, to exit 0 under a second
   set of stubs (the signing, packaging, notarisation and Gatekeeper tools answer
   what the script asserts and run nothing; containers are cpio archives); the
   input is the canary. Two canaries show the harness sees a run: the guard
   removed, and a run added after notarisation. The 2d review's seven spellings,
   each put at the end of the signed path, are all caught by the run; the text
   check, now a stated backstop for the lines the signed path does not take,
   catches five. `build-release.sh` gains `readonly sign from_binary` after option
   parsing, and a `finished` sentinel: bash 3.2, macOS's, was observed to exit 0
   from a fatal shell error (an unbound variable, a readonly assignment, `${x:?}`)
   when an EXIT trap is set, so a run that stopped short looked like a success.
2f. ~~The 2e review's findings~~ **Written 2026-10-24** (armed message; pull request
   #13; DP-053 ruled (c): the gate is for the accidental regression). The
   whole-path run now looks like the signing job:
   - real Mach-O inputs for both targets, built by `cc`, that leave the marker if run;
   - real `lipo`/`otool`/`vtool`/`strings`;
   - `GITHUB_ACTIONS`, `CI`, `RUNNER_TEMP` and `MPB_NOTARY_ISSUER` set;
   - a non-empty key and keychain.

   The run is in its own process group, and a member that outlives it fails the
   case. Any stub call in a form the stub does not know fails it too. A refused
   notarisation has its own case. The canaries cover the guard removed (dry and
   signed), a late run, and a background run. The text check:
   - taints through `$(...)`;
   - refuses awk `system(`/`getline`/`|`, `git -c`, `cargo --config` and a lone `&`;
   - requires `finished=1` to be the last line of code.

   Both reviews' twelve spellings are caught by the run and by the text check.
   Not done: the codesign stub writing `codesign0` elsewhere (L3), since the real
   tool writes into its working directory and the script requires that file there.
   Still open (DP-053 (b)): before anyone else can change `build-release.sh`, a
   signing job that cannot execute the input.
2g. ~~The 2f review's findings~~ **Written 2026-10-24** (armed message; pull request #13).
   - **M1:** a substitution whose last stage prints a digest or a field (`shasum`,
     `awk`, `wc`, `lipo`, `otool`, `vtool`, `stat`, `plutil`, `grep -c`) yields a
     value derived from the input, not the input. The guidance says such a value is
     fixed by how it is derived, never by widening INPUT.
   - **L1:** the awk rule refuses a lone `|`, not `||`.
   - **L2:** `security` is a strict stub, and `hdiutil create`/`attach` refuse a
     missing folder or mount point.
   - **L3:** the guard-removed canaries run for each target the host can execute,
     and are skipped by name otherwise.
   Also recorded, outside DP-053 (c): `printf … >>.git/config` setting
   `core.fsmonitor`, followed by any git call, belongs to the structural defence.
2h. ~~The 2g review's findings~~ **Written 2026-10-24** (armed message; pull request #13;
   all in the text check, `scripts/test-gates.sh`). L-C stays open: the x86_64-on-arm64
   canary is first exercised by the arm64 CI runners.
   - **M-A:** variables a tool reads as a program to run are not refused. Examples
     are `export GIT_PAGER="$bin"; git log` and `export RUSTC_WRAPPER="$bin"`. This
     was already at HEAD, and the stubbed `git`/`cargo` do not honour them. The
     DIGEST exemption widens it: `awk`, `stat`, `otool` and `wc` can print a name,
     not a digest. Fix:
     - extend the refused assignments to `GIT_.*`, `RUSTC.*`, `CARGO_.*`, `CC`,
       `CXX`, `LD`, `AR`, `PAGER`, `EDITOR`, `VISUAL`, `SHELL`, `.*_ASKPASS`,
       `TAR_.*`, `XCODE.*`;
     - narrow DIGEST to `shasum`, `lipo -archs`, `wc <`, and `awk` after one of those.
   - **M-B:** a double-quoted awk program hides `system(` and `|`. Refuse any
     double-quoted awk program; values go in with `-v`.
   - **L-A:** `lastw` is the substitution's last segment, not its last pipeline
     stage, and the `grep -c` test matches inside a pattern.
   - **L-B:** if `arch` fails for the host's own architecture, the canaries skip
     vacuously. Make that a FAIL.
   - **L-C:** the x86_64-on-arm64 canary has never run; the first arm64 CI run
     (`macos-26`, `macos-15`) is its first test.
   - **L-D:** `>|` on an awk command reads as a lone `|`.
2i. ~~The 2h review's findings~~ **Written 2026-10-24** (applied by the owner from a
   command the session prepared; all in `scripts/test-gates.sh`). Done: M1, M2 in
   part (a self-test over a copy with one extra run), L4, L5 in part. Accepted, not
   done: L3, the rest of L5 (fail-closed friction), and a committed table of
   evasion spellings (DP-053 (c): that belongs to the structural defence).
   - **M1:** the text check passes when awk itself fails. A syntax error exits 2 with
     empty output, which reads as "holds", and this was true before 2h too. Fix:
     fail on a non-zero awk status, and run the check once over a known-bad copy,
     requiring at least one violation.
   - **M2:** none of the text check's rules is tested in the suite; the 53 mutants
     and 17 probes lived in the session's scratchpad. Fix: commit a probe table
     (R/A plus a line) run through the same insertion loop, with a sink in OK but
     not in INPUT (`git tag -m "$q" x`) for every accepted line. `printf` sinks pass
     whatever the exemption does.
   - **L3:** `shasum` output carries the file name, and so does `grep -H` or a
     multi-file `grep -c`. Count `shasum` as a digest only on standard input, or
     allow only `awk '{print $1}'` after it; for `grep -c`, refuse `-H` and more than
     one file.
   - **L4:** add `CODESIGN_ALLOCATE`, `SDKROOT` and `TOOLCHAINS` to the refused
     assignments.
   - **L5:** fix these honest-edit false positives:
     - `wc -c < f` with a space;
     - `wc … | tr -d ' '` not exempt;
     - the message for `GIT_*` hardening variables;
     - `awk -- '…'`;
     - `grep -e x -c`.
2j. ~~**Small, one armed message (`scripts/test-gates.sh`), the 2i review and a flake**~~
   **Written 2026-10-25** (SE-106; the report's follow-up 2j). The three Lows are done
   with a probe each; the flake's cause is **not proved**: a canary that sees no run
   now prints the script's output and runs the input directly, and a preflight runs
   each input once before the canaries. The next occurrence will say what happened.
   As planned:
   - make the self-test copy with `mktemp`;
   - require the self-test's expected refusal text;
   - fail with "marker not found" when no line was inserted;
   - print the last lines of `build-release.sh`'s output when a canary does not see
     a run (it failed intermittently once, cause unknown), and, if the cause is
     first-execution assessment, run each input once before the cases.
3. ~~**Small, unguarded, any time:** reword the top-level `--help` line for
   `history` ("Reads only") and its pt-BR twin; add an i18n-parity check.~~
   **Done 2026-10-25** (pull request PR_PENDING). The line now says `history` reads
   MacPortsBrew's own state directory and only `history acknowledge <id> --yes`
   writes it. There was no pt-BR twin: `--help` is clap's compile-time English and
   does not go through the catalogue (the help-text exemption in
   `docs/spec-evolution.md`), so the parity is enforced as "one text whatever the
   language" by `help_is_one_text_whatever_the_language`, with the wording pinned by
   `the_history_help_line_names_the_one_subcommand_that_writes`
   (`crates/cli/tests/locale_pt_br.rs`). No `validate.sh` stage was added.
4. **Optional, as one armed message:** the `.claude/` fixes in
   `docs/research/tooling/2026-10-22-*.md`.
5. **Owner, after 2 (2 written 2026-10-23), the tag after #13 merges:** certificates, the `release-signing` environment, the
   `v*` tag ruleset, immutable releases, the dispatch dry run, the Standard-account
   `.pkg` check, the Gatekeeper protocol, DP-036, the tag.
6. **Release 0.2.0 track (pass 08 slices 6–9):** unchanged; DP-046 to DP-051 are
   still open.
