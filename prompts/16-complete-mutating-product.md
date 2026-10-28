<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — The complete mutating product (16)

On 2026-10-27 the session ruled all thirteen open decisions under the owner's
delegation, and the owner confirmed them in chat the same day (SE-109). The
rulings are in `docs/decisions-pending.md` (Resolved, DP-028 to DP-051), and
they change what the first mutating release is.

Before them, v0.2.0 moved MacPorts→Homebrew formulae only: no elevation, and
the source never removed. Now it is **symmetric**:

- both directions;
- elevation enumerated and consented per step;
- consented shims;
- removal opt-in, and only after the verification contract can be evaluated in
  full;
- the executed plan tied to the plan shown.

This work order carries the rulings out. It spans many pull requests and many
sessions. **Each part is one pull request**, merged on green required checks
(SE-108). The read-only release (v0.1.0) is not this pass's business; its tag is
the owner's.

**The repository at the start** (`fd443ef`, the merge of PR #24):

- `main` after pass 15, and pass 08 slice 6 **part A only** (PR #22). Slice 6
  parts B and C are not built.
- Mutations are wired behind `THIS_BUILD_DISPATCHES_MUTATIONS = false`.
- Slices 7, 8 and 9 have work orders, written for the narrower release (§1
  item 8 says which of their instructions this work order overrides).
- Nothing has ever been dispatched on a real machine.

**Where it stands now** is in `docs/passes/16-progress.md` (the ledger), not in
this file. Every session starts with section 4.1.

**Launching a session.**

- Use an **interactive** session in this repository, in **auto** permission mode.
  It must not be an unattended run: `MPB_UNATTENDED` unset, see §4.
- Paste the prompt below, the same text every time. It is the owner's paste text
  and is in the owner's language; everything else here is English, per
  `AGENTS.md`.

```text
Execute o work order prompts/16-complete-mutating-product.md. Leia-o inteiro,
depois siga a seção 4.1: leia docs/passes/16-progress.md, reconcilie com git e
GitHub, e continue de onde parou. Trabalhe de forma autônoma conforme a seção
4.3, inclusive merge de cada parte com os checks verdes, e só pare quando nada
mais puder avançar sem mim. No fim, me dê um resumo: o que avançou, os PRs, e
os itens de "Needs the owner" com a ação exata de cada um.
```

---

## 1. Read this first

In this order:

1. `AGENTS.md`, then `CLAUDE.md`.
2. `docs/spec/00-charter.md` and `docs/spec/01-invariants.md`, especially
   INV-01 to INV-08, 10 to 13, 15 to 20.
3. `docs/decisions-pending.md`: the thirteen rulings dated 2026-10-27, in full.
   **They are the requirements of this pass.** Where an older work order or
   design note disagrees with a ruling, the ruling wins and the older text is the
   bug. Fix it in the part that meets it.
4. `docs/spec-evolution.md`: SE-109 first, then SE-092, SE-094, SE-107 and
   SE-108.
5. `docs/spec/11-agentic-sdlc.md`, once.
6. `docs/passes/08-pre-flip-design-notes.md`. Its table lists seven tripwires.
   It is followed by `this_build_does_not_dispatch_mutations`, the test the flip
   edits on purpose. Together they are the flip's checklist.
7. Per part: the specification chapter, the slice work order and the research
   notes the part names. Load nothing else up front.
8. **Precedence over delegated work orders.** Parts S, E, F, G and L run slice
   work orders that inherit protocol from `08-the-mutating-executor.md` §3–4,
   slice 5 §2–3 and slice 2 §2. Where any of them conflicts with this work order,
   **this work order wins**:
   - They say plan mode. Here there is no interactive plan mode (§4).
   - They say "never merge" or "the owner merges". Here each part merges per §4.3.
   - They say "`git push` is denied". Here every topic branch is pushed (§4.2).
   - They say "no new dependency". Here dependencies follow §3 item 6.
   - They say "stop until DP-0xx is ruled", "stop if DP-047 is ruled (a)", or
     treat a ruled DP as pending. Every DP here is ruled, and the ruling applies.
     DP-047 is (b) now and (a) in part I, so F does not stop.
   - They refuse an executable not named after its package (slice 8). Part F
     reverses that.
   - Plans go to `docs/passes/16-plans/<part>.md`. For S, also append an
     "As built" section to the existing `docs/passes/08-slice-6-plan.md`.
   - Reports go into `docs/passes/16-report.md`.

## 2. Authority and precedence

| Tier | Source | Changeable? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | No. If one seems wrong, **stop and ask the human.** |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. DP-028's non-goal wording is one, confirmed 2026-10-27; part A applies it. |
| 3 | **MUST** in `docs/spec/02..11` | Only with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Change freely; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

## 3. Hard boundary for this pass

Each item is decided by a grep, a hook, a test or a diff. When a part cannot
keep one, that part stops. It records an owner item or a DP (`/decision-pending`)
in the ledger, and the session continues with the next part that can advance.

1. **The switch stays off.** `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false` on
   `main` until the owner rules on part L's proposal. Decided by `grep`.
2. **Nothing mutating runs here.** No mutating `brew` or `port` subcommand runs
   on this machine, and no elevation program runs at all; the hooks refuse both.
   The elevated path (part J) is exercised only through the recorded runner and
   `--replay`. A test that must name the elevation program keeps the name in a
   fixture file, not on a command line, as `CLAUDE.md` says.
3. **The process allowlist is frozen, and no hook enforces it.** The frozen paths
   are `crates/adapters/src/process/mod.rs` (`PERMITTED_PROGRAMS`,
   `PERMITTED_SUBCOMMANDS`) and `data/policy/provider-commands.toml`. Any growth
   goes through the staging procedure (item 5). Decided by
   `git diff fd443ef..HEAD -- <those paths>` containing only hunks the owner
   applied; `security-reviewer` checks it.
4. **Guarded files.** `scripts/*.sh`, `.claude/hooks/`, `.claude/settings.json`
   and `.github/workflows/` change only through the staging procedure. The hook
   refuses any edit to them without the owner's armed message.
5. **The staging procedure** is the single way a part asks the owner to change a
   frozen or guarded file:
   1. Write the change as a `git apply`-able patch under
      `docs/passes/16-staged/<part>/`, with a `README.md`. The README gives the
      exact owner action, the `MPB_ALLOW_GUARD_EDIT=1` sentence where the change
      is guarded, and why the change is needed.
   2. Merge the staging as a docs-only pull request, so the README is on `main`.
   3. Set the part to `waiting on owner`. Its notes say where to resume, for
      example "resume as `wo16/H-2`, from step 3".
   4. The item counts as applied iff
      `git apply --reverse --check docs/passes/16-staged/<part>/<x>.patch`
      succeeds on `origin/main`. Record the owner's commit in the ledger.
6. **Dependencies.** A new dependency enters only through an ADR under
   ADR-0004. `libc`, or any crate that reaches it, and any `unsafe` enter only
   through the single ADR of part E. That ADR names its purposes, and
   ADR-0006's status names it as superseding ADR-0006 for those purposes only.
7. **`docs/spec/01-invariants.md` is not edited.** DP-028's INV-10 paragraph is
   already applied, by the owner (`cc844d1`).
8. **No `v0.2.0` tag.** No change to repository settings, secrets or
   environments.
9. **Fixtures are captured, never invented.** A recording no machine here can
   produce becomes an owner item. Tests stay on recordings that exist.
10. **Provider metadata never becomes a path or a flag unchecked.** Each site
    defines its name class and tests it:
    - a shim's executable name: a single leaf, no `/`, no `..`, no leading `-`.
      The shim's *target* must exist under the destination's detected prefix
      and be owned by that destination;
    - a Portfile directory: `<name>-<version>_<revision>`, each field in its
      provider's character class, resolved under the detected registry path
      only;
    - a lock file: an entry of the lock directory itself, never followed (part
      G).
11. **Command text with guarded words is data.** The Bash guard refuses some
    words anywhere on a command line (the elevation programs, `port select`,
    `brew services` and others). So:
    - commit with `git commit -F <scratch file>`;
    - open pull requests with `--body-file`;
    - search for such words with `grep -f <pattern file>`.

    That keeps the words as data, the way `CLAUDE.md` prescribes for fixtures.
    It is not rewording a command, and it is never used to *run* a refused
    command.

## 4. Working protocol

- **This is an interactive session working autonomously. It is not an
  unattended run.** "Unattended" has a defined meaning in
  `11-agentic-sdlc.md`: `MPB_UNATTENDED=1`, under which merges, ADRs and DPs are
  refused. If `MPB_UNATTENDED` is `1`, do not start. Report why, and end.
- **Re-verify the tooling first** (`/tooling-recheck`), once per session.
- **Verify, do not assume.** A provider fact is observed read-only, cited with a
  read date, or detected at runtime. Each new fact gets its note in
  `docs/research/` (`/research-note`), and its data in `data/policy/*.toml` with
  `source` and `read_on`.
- **Never touch the developer's real package managers.** Reading is allowed;
  writing is a failure.
- **A reviewed plan before code.** This applies to every part that touches the
  planner, the gate, the executor, verification, the process executor,
  elevation, or files outside the state directory: C, D, S, E, F, G, H, K, I and
  J.
  - Write the plan to `docs/passes/16-plans/<part>.md`. The plan ends with a
    **Reviews** table: reviewer, commit reviewed, findings, resolution.
  - `spec-guardian` and `security-reviewer` review the plan before any code,
    and again after each commit. Run them on the stronger model, and never name
    a model in a file (SE-017).
  - Their review is the approval. Do **not** enter interactive plan mode, and do
    not wait for a human to approve a plan.
  - On resume, review only the commits the table does not list.
- **Mutation proof.** A part that adds a safety check lists its mutants in its
  plan. A subagent in an isolated worktree applies them, and the session reads
  every survivor. Remove that worktree afterwards.
- **Delegate by default.** Research and sweeps go to subagents, in parallel
  where independent; the main session decides and edits.
- **Commit incrementally.** Use Conventional Commits, one coherent change each.
  A material Tier 3–5 change lands with its SE entry in the same pull request.
- **Use the skills**: `/adr`, `/research-note`, `/capture-fixture`,
  `/add-doctor-check`, `/spec-evolution`, `/decision-pending`, `/preflight`,
  `/pass-report`.
- **A refusal is an owner item, never a puzzle.** This covers a permission
  prompt that cannot be answered, an auto-mode classifier refusal, and a hook
  refusal. Record it in the ledger with the exact command and what it was for.
  Never reword around it (`CLAUDE.md`). Then continue elsewhere.

### 4.1 Continuity: every session starts here, and any session can stop

**On start:**

1. **Mode.** If `MPB_UNATTENDED` is `1`, end (see §4). Make sure
   `.claude/current-pass` reads `16-complete-mutating-product`.
2. **Local leftovers.** Run `git status --porcelain`, `git stash list`,
   `git worktree list` and `git branch -vv`, and handle what they show:
   - A dirty tree on a `wo16/*` branch: commit it as `wip`, once the gates pass,
     and push it.
   - A dirty tree anywhere else: stash it as `wo16-orphan <date>` and log it.
   - Never discard anything.
   - Remove worktrees that mutation subagents left behind.
3. **Reconcile.** Run `git fetch`. List
   `git branch -a --list '*wo16/*'` and
   `gh pr list --state all --search "head:wo16/"`. The truth is, in this order:
   1. GitHub (a pull request's state and merge commit);
   2. the newest ledger on each open `wo16/*` branch
      (`git show origin/<branch>:docs/passes/16-progress.md`);
   3. `main`'s ledger.

   Correct the ledger where it lags. Check each "Needs the owner" line with the
   test of §3 item 5.4.
4. **Pick the work**, in this order:
   1. a part `in progress`: resume it on its branch, from its plan and its
      Reviews table;
   2. a part `in review`: finish it (§4.2);
   3. otherwise, the first part whose status is `todo` and whose **dependency
      column** is all `merged` (or `merged (owner item open)`). Prefer A, B, C,
      D, P and M, but a waiting or blocked part does not hold another part unless
      the dependency column says so;
   4. never work around a part that is `waiting on owner` or `blocked`.

### 4.2 Transitions and the ledger

- **Branches.** Name them `wo16/<part>-<slug>`. A follow-up after an owner
  action is `wo16/<part>-2-<slug>`.
- **On start.** At a part's first push, open a **draft** pull request, and set
  the ledger to `in progress` on that branch. A second commit records the pull
  request's number.
- **Push after every commit**, so a session that is cut off loses nothing.
- **Before merging.** Rebase on `origin/main`. On a ledger conflict, take
  `main`'s file and re-apply only this part's row and log line. Then mark the
  pull request ready.
- **Waiting for checks.** Use **one** background command that exits when no
  required check is pending. Do not start another part's branch while it runs.
- **Green.** `gh pr merge <n> --merge`, never `--admin`.
- **Red.** Fix it and push again. If it is red after three attempts, or red for
  a cause outside the part, the part is `blocked`, with an owner item.
- **Recording the merge.** Write `merged <sha>` (from
  `gh pr view <n> --json mergeCommit`) in the first commit of the next part's
  branch. If no part follows, open a ledger-only pull request
  (`docs(passes): ledger`), merge it on green, and do not list it as a part.
- **Status values:**
  - `todo`
  - `in progress <branch>`
  - `in review <PR>`
  - `merged <sha>`
  - `merged (owner item open) <sha>`
  - `waiting on owner <staged path>`
  - `blocked <DP or owner item>`
- **The session log** gets one line per session: date, the parts advanced, and
  where it stopped and why.
- **Context.** When a session nears its context limit, it finishes the current
  commit, updates the ledger, pushes and ends. It does not start a part it
  cannot carry to a pushed commit.

### 4.3 Autonomy: what this pass does without asking

The owner has authorised the following for this pass. Do it without asking:

- push `wo16/*` branches, open pull requests, and **merge each part once
  `gh pr checks <n> --required` is all green** (SE-108);
- fix a failing check on your own part;
- make every design choice the work order leaves to the session, and record it
  in the ADR or SE entry. That includes each ADR's option, the confirmation
  levels, schema versioning and the fate of `--retain-source`;
- start the next part right after a merge, in the same session, while context
  allows.

**Ask the human only when nothing else can advance.** Owner items (staged
patches, captures, a refusal, a DP that is genuinely theirs by
`/decision-pending`'s test) go into the ledger's "Needs the owner" and do **not**
end the session. The session ends only when:

- every remaining part is `merged`, `waiting on owner` or `blocked`; or
- a human must act before the hard boundary of §3 can hold.

Its final message lists every "Needs the owner" line with its exact action.

A trade-off inside Tiers 3–5 is **never** a reason to ask. Decide it, record it,
and continue.

## 5. Order of work

**Part 0** is done. The owner confirmed the rulings in chat on 2026-10-27, quoted
in SE-109.

**Parts A, B, C, D, P and M** do not touch the executor's run path. **Run one
session at a time**: the ledger assumes a single writer.

**Parts S, E, F, G, H, K, I, J and L** change what a run may do. They run **one
at a time, in that order**, each after the previous one merges. The reasons:

- **P before F.** P stages everything the chain needs from the owner, in one
  round trip.
- **G and H before K.** Removal needs both.
- **K before I and J.** Today removal is the default, and it is held back only
  because it elevates and elevation is refused. Flipping the default first means
  no `main` ever holds code in which a default removal can run elevated.

The ledger's dependency column is the binding form of this order.

### 0 — The owner confirms the rulings

**Done 2026-10-27**, in chat (SE-109). That includes DP-028 (Tier 2 charter text)
and DP-046 (the INV-17 and charter reading).

### A — Propagate the documentation rulings (DP-028, DP-036, DP-048, DP-050)

1. **Charter.** Check first that both behaviours DP-028's charter wording
   promises exist: disclosure of a provider's own analytics, and saying when a
   provider read may reach the network. Then apply the wording exactly as ruled.
   - If a behaviour is missing, build it as a read-only disclosure
     (`/add-doctor-check` may fit). If that turns out to be more than a
     disclosure, stop with a DP.
   - If the edit is refused, it is an owner item. Do not reword it.
2. **The four sentences that still call DP-028 open:**
   `docs/spec/06-cli-and-ux.md` (≈162), `README.md` (≈375), `wiki/FAQ.md` (≈71),
   and `CHANGELOG.md`'s unreleased section (≈839). Reword each to the ruling.
3. **Chapter 04.**
   - In the declared-deviation note slice 6 left (≈194–209), remove **only the
     source-build bullet**, now settled by DP-050. The other clauses stay
     declared.
   - Correct the "no longer available" example: the artifact is unavailable,
     unsupported or disabled, and a source build is a named cost.
   - Declare the residual removal window next to DP-030's.
4. **DP-036.** Replace the hedged reading of `.app` as the `.dmg` with the
   ruling, at three places:
   - `docs/decisions-pending.md`, DP-019 (≈"the owner's to confirm in DP-036");
   - `prompts/ROADMAP-to-first-release.md` (≈106);
   - `docs/spec-evolution.md` (≈4722).
5. **Superseded work orders.** Put this banner at the top of `08-slice-6`,
   `08-slice-7`, `08-slice-8`, `08-slice-9`, `08-slice-2` and
   `08-the-mutating-executor.md`: "Run through work order 16 (SE-109); its §1
   item 8 overrides this file's plan-mode, merge, push, dependency and stop
   instructions." In `11-first-mutating-release.md` (≈58), replace
   "`--retain-source` is the default" with the ruling.
6. **Roadmap.** Rewrite `prompts/ROADMAP-to-first-release.md`: v0.2.0's shape,
   the sequence, the owner table and the status heading. Remove its interim
   banner.
7. Run `docs-auditor` before merge.

### B — The creation-mode seam (DP-029)

- Make the durable write's creation mode a crate-private parameter. Production
  behaviour does not change.
- Test: create with `0o644` and assert the result is `0600`.
- Proof: removing the post-creation permission fix must fail that test.
- Record the `0277` read-back half as still owed.

### C — Naming a package enters advanced mode for that package (DP-037)

- **Rendered output.** Print `advanced.warning` (the key exists in both
  catalogues) when the named row is outside the default view.
- **New catalogue key(s).** Add the key for "how the row was entered" to `en`
  and `pt-BR`, and run the i18n parity check.
- **`--json`.** The output carries stable identifiers, never rendered text: for
  example `"enteredBy": "NAMED"` and `"warnings": ["advanced.warning"]`.
  Update the schema in `docs/schemas/`.
- **Transitive dependencies.** A named package's transitive dependencies are
  never added.
- **Test:**
  `invariant_13_a_named_dependency_warns_and_needs_the_stronger_confirmation`.
  Its confirmation half lands in part K, so mark that half pending, by name,
  until then.

### D — What defines a service (DP-032)

1. `/research-note`: an R-8 addendum. It covers:
   - where the installed Portfile lives, per version;
   - which PortGroups add startup items;
   - how `port -q provides` attributes a launchd item;
   - for Homebrew, what a materialised `homebrew.mxcl.*` plist looks like.
2. `/capture-fixture`: a recording with a MacPorts port that has a startup item.
   If no machine here has one, record it as an owner item. The part may then
   merge as `merged (owner item open)`, with tests on the recordings that exist.
3. Add attribution by `provides` first. Then add a **bounded** Portfile reader: a
   fixed set of recognised forms; anything else is unknown.
4. **One domain rule for both providers:** a package is a service if it is
   declared or materialised. Each adapter supplies its two halves. Unknown
   refuses the plan (INV-19).
5. **Replay.** Under `--replay` the reader answers from the recording or as
   unknown, and never from the live filesystem (INV-15). Extend
   `invariant_15_a_replayed_reader_never_consults_the_live_filesystem`.
6. **Review.** `security-reviewer` reviews the reader and the path rule of
   §3 item 10.

### P — Stage everything the chain needs from the owner, once

P exists so the owner acts once instead of five times. It is research plus
staging; no product code.

1. **Allowlist research.** Use `/research-note` to establish as read-only, on
   both providers, every subcommand that F (the executable list, e.g. a file
   list for a MacPorts port) and H (integrity, ownership, linkage) will need.
2. **Dependency research.** For E and G, run `cargo metadata` on the candidate
   dependency sets (`sha2` with `default-features = false`; `libc` or a safe
   wrapper). List every crate that ships a `build.rs`.
3. **Elevation research.** Note what J's elevation will need from the allowlist
   and from `stop-gate.sh`.
4. **Stage it all** through §3 item 5, in one directory with one README:
   - the allowlist growth;
   - the `build_scripts_allowed` entries in `scripts/check-licences.sh`;
   - J's allowlist entry for the elevation program;
   - the matching `stop-gate.sh` token-check change, which admits that program
     only inside `crates/adapters/src/process/`, with its
     `scripts/test-gates.sh` / `.claude/hooks/test-hooks.sh` cases.

A later part that finds a need P missed stages it the same way.

### M — Harden the Claude workflows (DP-039 (b))

1. Stage hardened `claude.yml` and `claude-code-review.yml` under
   `docs/passes/16-staged/M/` (§3 item 5), implementing the ruling's five points.
2. Add a `scripts/test-gates.sh` case if one can assert them. Stage that case
   too.
3. The part ends `waiting on owner`. Installing the files is the owner's armed
   message, and setting the key is the owner's.

### S — Finish slice 6: parts B and C

Run `08-slice-6-verification-before-removal.md` parts B and C:

- make `PROVIDER_REPORTS_ACTIVE` evaluable;
- re-read the destination immediately before `REMOVE_SOURCE` is dispatched;
- widen the tripwire `the_switch_needs_the_destination_contract_evaluable`.

**Slice 6 part D is superseded** by DP-048 (c) and part K. Say so in an SE
entry, and do not build it.

### E — Slice 7, both routes of DP-049, and the dependency ADR

Run `08-slice-7-plan-tie-and-consent.md`, with these changes:

1. **One ADR, two purposes.** It does two things:
   - It adds `sha2` under ADR-0004, with `default-features = false` and the
     transitive set verified per target.
   - It chooses G's lock-probe mechanism: `libc` with one reviewed `unsafe`
     call in a single module, or a maintained safe wrapper.

   It supersedes ADR-0006 for those two purposes only; update ADR-0006's status
   to name it. It lists every new crate with a `build.rs`, and P staged their
   `check-licences.sh` entries. If the probe uses `libc`, the ADR also records
   lowering `unsafe_code` from `forbid` to `deny`, with an SE entry, because
   `#[allow]` under `forbid` does not compile. The `sha2` entry in `Cargo.toml`
   lands here; the probe's lands in G.
2. **Two routes to execute.** `migrate --yes --plan-digest <d>` for scripts, and
   an interactive confirmation of the same in-memory plan for a person at a
   terminal.
3. **Determinism (INV-15).** The digest is computed over a canonical
   serialisation: no hash-map iteration order. Test it: the same snapshot gives
   the same digest across runs.

### F — Slice 8, as designed (DP-047 (b))

Run `08-slice-8-primary-and-resolution.md`, with one change: the executable list
comes from provider metadata through the reads P established, so `rg` for
`ripgrep` is planned rather than refused. Apply §3 item 10's shim-name rule.

### G — The lock probe (DP-030 (b))

Run `08-slice-9-detector-and-flip-proposal.md` **part A, option (b)**, using the
mechanism E's ADR chose. That part's binding requirements stay:

- a provider-neutral detector trait in `domain`;
- lock locations held in `data/policy/`, never above `adapters/`;
- every error falls back to unknown;
- `Unknowable` under `--replay` (INV-15);
- a refusal on `Observed` and on the unknown fallback.

**Locations, from R-17.** For Homebrew, every file under
`<prefix>/var/homebrew/locks/`, including the suffix-less `update` and
`vendor-install` locks. For MacPorts, the registry lock under the **detected**
`portdbpath`.

**Opening safely.** The directory belongs to the provider.

- Open with `O_RDONLY | O_NOFOLLOW | O_NONBLOCK`.
- Check with `fstat` on the handle that it is a regular file.
- Bound the directory listing.
- Any error is the unknown fallback.

Mutants to include: drop `O_NOFOLLOW`, and drop the regular-file check.

**The claim and the refusal.** The claim is "a provider lock is held right
now", never "idle". The refusal is a named error with an exit code, added to
`crates/domain/src/error.rs`'s contract and to the schema.

**Tripwires and review.** Turn the `NOT_CHECKED` tripwire into a real answer,
and keep its failing test for a build that dispatches with no probe. The INV-17
review of the MacPorts asymmetry goes in the SE entry.

### H — The four remaining destination checks (DP-051 (a))

The checks are `PROVIDER_INTEGRITY_CHECK`, `EXECUTABLES_OWNED_BY_DESTINATION`,
`NO_CROSS_PROVIDER_LINKAGE` (INV-07, INV-08) and `LIVENESS_PROBE`.

1. **Research first.** One research note per check. A read a check needs must be
   among those P established as read-only. A form that can write is refused,
   whatever it would check (INV-05).
2. **`LIVENESS_PROBE`.** It runs the destination's resolved executable, which is
   outside the fixed allowlist.
   - **Design note first:** `docs/passes/16-liveness-design.md`. It says how the
     runner admits exactly that program without weakening the fixed allowlist or
     adding a second spawn site. `security-reviewer` passes it before any code.
   - **What may run.** Only a curated probe from a new dated data file. The
     entry holds argv only, never a program path; the program is the resolved
     executable, under §3 item 10.
   - **How it runs.** Sanitised environment (INV-10), no stdin, a scratch
     working directory, and ADR-0006's timeout. Only the exit status counts.
3. **Unchecked packages.** A package with no curated probe, or any check that
   cannot be evaluated for it, is reported unchecked, and its source is never
   removed.
4. **Run outcome.** Name the outcome a run ends with in that case. It is not
   `COMPLETED`. Give it a stable identifier in `--json` and a catalogue key in
   both locales.
5. **Tripwire.** S's widened tripwire must now fail only for a check that is
   genuinely unevaluable.

### K — Removal becomes opt-in (DP-048 (c), DP-037's confirmation, DP-050)

1. **Flip the default.**
   - Removal requires `--remove-source`. Change 06's synopsis.
   - The session decides the fate of `--retain-source` (default proposal: an
     accepted no-op for one release, with a deprecation notice) and records it
     in an SE entry.
   - Decide what `retainSource` means in `plan.v1` and `history.v*`: the same
     field with a new default, or a version bump. Write it into the schemas.
   - Adjust `restore`; SE-107 already notes that interaction.
   - Coexistence becomes the default outcome (INV-18).
2. **Define the two confirmation levels** in 06, with an SE entry. The level for
   normal-mode removal is stronger than an install's. The level for removal of a
   row entered through advanced mode (part C) is stronger still, so 06's "than in
   normal mode" stays true. Say how each level works under
   `--yes --plan-digest`, and whether a script can opt into each. Complete C's
   invariant test.
3. **When `REMOVE_SOURCE` is dispatched.** Only when all of these hold: S's
   re-read ran, G's probe answered, and all six checks were evaluated for that
   package. Extend the INV-01, INV-03 and INV-04 tests.
4. **DP-050.**
   - Show the source-build risk in the default view, with what the build needs.
     If `destination_source_build` already does this, say so with the test that
     shows it.
   - Toolchain check. `xcode-select -p` is already a permitted system probe. The
     research note answers one question: does its answer, or a filesystem read of
     the Command Line Tools location, prove the tools are present? If either
     does, their absence makes a source-build step `MANUAL_REVIEW`, and `doctor`
     gets a finding for it (`/add-doctor-check`). If neither does, the note says
     why.

### I — Layer 4, consented (DP-047 (a))

Read `05-environment-and-primary-provider.md` and R-6 first.

1. **The offer.** When the destination does not win `PATH`, the plan offers one
   shim plus one managed `PATH` block for that executable.
2. **Without consent**, the gate refuses with `PRIMARY_NEEDS_SHIM` and names
   layers 1 and 2.
3. **Plan steps.** Both writes are plan steps with exact paths. Extend E's digest
   and confirmation to cover them, and record consent per step (INV-02).
4. **What to build:**
   - a backup;
   - a preview;
   - `env --revert`;
   - restore support;
   - idempotence: running twice changes nothing (INV-16; extend its test);
   - a MacPortsBrew-owned shim directory, written with the durable write, that
     no symlink can lead out of;
   - shim names that follow §3 item 10.
5. **Rules that bind.** INV-11 and INV-12, and INV-07: the shim directory must
   not make one provider's builds resolve the other's tools. This is the first
   write outside the state directory, so `security-reviewer` on the plan is not
   optional. Tests run against a scratch home, never the real one.

### J — Elevation (DP-046 (b))

1. **Research note first:** what `sudo` passes on to the child on macOS
   (`env_reset`, `env_keep`, `secure_path`), and how its timestamp cache
   behaves.
2. **Then the design**, `docs/passes/16-elevation-design.md`, **then the ADR**.
   Both are reviewed before any code.
3. **The constraints are the ruling's:**
   - an absolute `sudo` path, taken from `data/policy/`, so no other `src` line
     names the program;
   - argv only;
   - elevation only where the data cell says `required` or detection proves it;
   - every elevated step enumerated in the plan with its exact argv, and
     consented to per step;
   - no credential cached across runs, and none left behind;
   - the process never runs as root;
   - sudoers environment policy detected and disclosed, or refused.
4. **INV-10 test.** It checks the environment the elevated child **receives**,
   from the recording, not the one that was passed.
5. **Casks.** A cask whose elevation stays `unknown` is refused, with a named
   reason.
6. **Staging.** The allowlist entry and the `stop-gate.sh` change are P's staged
   package. If they are not applied yet, J ends `waiting on owner` at the first
   step that needs them.
7. **INV-06.** Every new site answers an `invariant_06_*` test.

### L — The flip proposal, for the symmetric scope

Run `08-slice-9-detector-and-flip-proposal.md` part B, with both directions in
scope. The proposal accounts for:

- each of the seven tripwires and `this_build_does_not_dispatch_mutations`;
- every check added since;
- what remains for work order 11's acceptance in both directions, per
  architecture;
- **the milestone DP-046 requires.** The non-elevating path (MacPorts→Homebrew
  formulae, source retained) is exercised first on the disposable machines, and
  then the rest.

The proposal is the deliverable. The switch stays off.

## 6. Definition of done

Demonstrate each item; do not assert it.

- [x] Part 0 is confirmed (2026-10-27).
- [ ] Every part is `merged`, or `waiting on owner` / `blocked` with its exact
      action, in `docs/passes/16-progress.md`. The ledger matches `main` and
      GitHub.
- [ ] `grep -n THIS_BUILD_DISPATCHES_MUTATIONS` shows `false` on `main`.
- [ ] `git diff fd443ef..main -- crates/adapters/src/process/mod.rs
      data/policy/provider-commands.toml` holds only hunks the owner applied
      from a staged patch.
- [ ] Every invariant this pass touches points by name to its test, new ones
      included: 01, 02, 03, 04, 05, 06, 07, 08, 10, 11, 12, 13, 15, 16, 17, 18,
      19 and 20.
- [ ] Every new provider fact has a dated research note and a data entry.
- [ ] E's ADR exists, and ADR-0006's status names it and the purposes it covers.
- [ ] `/preflight` passes, or the report names what was skipped and why.
- [ ] `spec-guardian`, `security-reviewer` and `docs-auditor` findings are
      resolved or accepted in writing, per part (the plans' Reviews tables).
- [ ] `docs/spec-evolution.md` records every material deviation, one entry per
      topic (SE-032).

### Needs the owner (what to expect; the ledger has the live list)

- **Launching** each session as the header says. A permission prompt or classifier
  refusal that blocks a push, a pull request or a merge is an owner item. One
  way to resolve it is adding allow rules for `git fetch`, `git push`,
  `git switch`, `git worktree`, and `gh pr list|view|checks|create|merge`.
- **P's staged package:**
  - the allowlist growth for F, H and J;
  - the `check-licences.sh` build-script entries;
  - the `stop-gate.sh` change;
  - any later additions.
- **M's workflows** and their test case.
- **Captures no machine here can produce:**
  - a MacPorts port with a startup item;
  - an arm64 machine with both providers;
  - a machine with a shimmed `PATH`.
- **Release:** work order 11's acceptance in both directions on disposable
  machines (the non-elevating milestone first), the ruling on the flip proposal,
  and the v0.2.0 tag.

## 7. End-of-pass report

Write it with `/pass-report` to `docs/passes/16-report.md`, adding one section
per part as each part merges. Every material deviation it names must already be
in `docs/spec-evolution.md`.
