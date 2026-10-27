<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — The complete mutating product (16)

On 2026-10-27 all thirteen open decisions were ruled (SE-109). The session made
the rulings under the owner's delegation, and the owner confirmed them in chat
the same day. The rulings are in `docs/decisions-pending.md`
(Resolved, DP-028 to DP-051), and they change what the first mutating release
is.

Before them, v0.2.0 moved MacPorts→Homebrew formulae only: no elevation, and
the source never removed. Now it is **symmetric**:

- both directions;
- elevation enumerated and consented per step;
- consented shims;
- removal opt-in, and only after the verification contract can be evaluated in
  full;
- the executed plan tied to the plan shown.

This work order carries the rulings out. It is long because it orders work that
spans many pull requests. **Each part is one pull request**, merged on green
required checks (SE-108). The read-only release (v0.1.0) is not this pass's
business; its tag is the owner's.

**The repository at the start:**

- `main` after pass 15, and pass 08 slice 6 **part A only** (PR #22). Slice 6
  parts B and C are not built.
- Mutations are wired behind `THIS_BUILD_DISPATCHES_MUTATIONS = false`.
- Slices 7, 8 and 9 have work orders, written for the narrower release.
- Nothing has ever been dispatched on a real machine.

---

## 1. Read this first

In this order:

1. `AGENTS.md`, then `CLAUDE.md`.
2. `docs/spec/00-charter.md` and `docs/spec/01-invariants.md`, especially
   INV-01, 02, 03, 04, 06, 10, 11, 12, 13, 15, 16, 17, 19 and 20.
3. `docs/decisions-pending.md`: the thirteen rulings dated 2026-10-27, read in
   full. **They are the requirements of this pass.** Where an older work order
   or design note disagrees with a ruling, the ruling wins, and the older text is
   the bug. Fix it in the part that meets it.
4. `docs/spec-evolution.md`: SE-109 first, then SE-092, SE-094, SE-107 and
   SE-108.
5. `docs/spec/11-agentic-sdlc.md`, once.
6. `docs/passes/08-pre-flip-design-notes.md`. Its table lists seven tripwires.
   It is followed by `this_build_does_not_dispatch_mutations`, the test the flip
   edits on purpose. Together they are the flip's checklist.
7. Per part, the specification chapter, the slice work order and the research
   notes the part names. Load nothing else up front.

## 2. Authority and precedence

| Tier | Source | Changeable? |
|---|---|---|
| 1 | `docs/spec/01-invariants.md` | No. If one seems wrong, **stop and ask the human.** |
| 2 | `docs/spec/00-charter.md` | Only with an explicit human decision. DP-028's non-goal wording becomes one when the owner confirms it (part 0). |
| 3 | **MUST** in `docs/spec/02..11` | Only with a `docs/spec-evolution.md` entry. |
| 4 | **SHOULD** | Change freely; record the material ones. |
| 5 | **HYPOTHESIS** | Expected to change. Record the outcome. |

## 3. Hard boundary for this pass

Each item is decided by a grep, a hook, a test or a diff. A part that cannot
keep one stops, writes a DP with `/decision-pending`, and the pass goes on with
the next part that does not depend on it.

1. **The switch stays off.** `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false` on
   `main` until the owner rules on part L's proposal. Decided by `grep`.
2. **Nothing mutating runs here.** No mutating `brew` or `port` subcommand runs
   on this machine, and no elevation program runs at all; the hooks refuse both.
   The elevated path (part J) is exercised only through the recorded runner and
   `--replay`. A test that must name the elevation program keeps the name in a
   fixture file, not on a command line, as `CLAUDE.md` says.
3. **The process allowlist is frozen, and no hook enforces it.**
   - Frozen paths: `crates/adapters/src/process/mod.rs` (`PERMITTED_PROGRAMS`,
     `PERMITTED_SUBCOMMANDS`) and `data/policy/provider-commands.toml`.
   - Any growth is staged under `docs/passes/16-staged/<part>/`, with a
     `README.md` saying what to apply and why, and the part stops.
   - Decided by `git diff main -- <those paths>` being empty in every part's pull
     request. `security-reviewer` checks it.
4. **Guarded files change only under the owner's armed message.** That covers
   `scripts/*.sh`, `.claude/hooks/`, `.claude/settings.json` and
   `.github/workflows/`, and the hook refuses everything else. A part that needs
   one stages its files in the same way and stops.
5. **`docs/spec/01-invariants.md` is not edited.** DP-028's INV-10 paragraph
   was applied by the owner (`cc844d1`, 2026-10-27).
6. **Dependencies.** A new dependency enters only through an ADR under
   ADR-0004. `libc`, or any crate that reaches it, and any `unsafe` enter only
   through the single ADR of part E. That ADR names its purposes, and
   ADR-0006's status names it as superseding ADR-0006 for those purposes only.
7. **No `v0.2.0` tag.** No change to repository settings, secrets or
   environments.
8. **Fixtures are captured, never invented.** A part that needs a recording no
   machine here can produce lists it under "needs the owner", and keeps its
   tests to recordings that exist.
9. **Provider metadata never becomes a path or a flag unchecked.** A name taken
   from metadata becomes a file name (a shim, a probe target, a Portfile path)
   only if both hold:
   - it is a single leaf: no `/`, no `..`, no leading `-`, nothing outside the
     identifier class;
   - the file it names exists under the destination's detected prefix and is
     owned by that destination.

   Decided by a test per site.

## 4. Working protocol

- **Re-verify the tooling first** (`/tooling-recheck`), once per session.
- **Verify, do not assume.** A provider fact is observed read-only, cited with a
  read date, or detected at runtime. Each new fact gets its note in
  `docs/research/` (`/research-note`), and its data in `data/policy/*.toml` with
  `source` and `read_on`.
- **Never touch the developer's real package managers.** Reading is allowed;
  writing is a failure.
- **Plan mode first** for every part that touches the planner, the gate, the
  executor, verification, the process executor, elevation or files outside the
  state directory: C, D, and S through J. `spec-guardian` and
  `security-reviewer` review the plan before any code, and again after each
  commit. Run them on the stronger model, and never name a model in a file
  (SE-017).
- **Mutation proof.** A part that adds a safety check lists its mutants in its
  plan. A subagent in an isolated worktree applies them, and the session reads
  every survivor.
- **Delegate by default.** Research and sweeps go to subagents, in parallel
  where independent; the main session decides and edits.
- **Commit incrementally.** Use Conventional Commits, one coherent change each.
  A material Tier 3–5 change lands with its SE entry in the same pull request.
- **Use the skills**: `/adr`, `/research-note`, `/capture-fixture`,
  `/add-doctor-check`, `/spec-evolution`, `/decision-pending`, `/preflight`,
  `/pass-report`.
- **Honour stop conditions.** "Stop" means stop. Write the DP, then continue
  elsewhere.
- **Before a session starts a part**, set `.claude/current-pass` to
  `16-complete-mutating-product`.

## 5. Order of work

**Part 0 comes first.** No part that implements a ruling merges until the owner
has confirmed the rulings. The confirmation is a comment or approval on the pull
request that records them, linked from SE-109. If the owner changes a ruling,
update `decisions-pending.md`, SE-109 and the affected part before starting it.

**Parts A, B, C, D and M** do not touch the executor's run path. They may run in
parallel sessions, each in its own worktree.

**Parts S, E, F, G, H, K, I, J and L** change what a run may do. They run **one
at a time, in that order**, each after the previous one merges, because each
later check must see the earlier code. The order is deliberate:

- **K before I and J.** Today removal is the default, and it is held back only
  because it elevates and elevation is refused. Flipping the default first means
  no `main` ever holds code in which a default removal can run elevated.
- **G and H before K.** Removal needs both, so they land first.

### 0 — The owner confirms the rulings

**Done 2026-10-27:** the owner confirmed all thirteen rulings in chat (SE-109),
including DP-028 (Tier 2 charter text) and DP-046 (the INV-17 and charter
reading).

### A — Propagate the documentation rulings (DP-028, DP-036, DP-048, DP-050)

1. **Charter.** Check first that both behaviours DP-028's charter wording
   promises exist: disclosure of a provider's own analytics, and saying when a
   provider read may reach the network. Then apply the wording exactly as ruled.
   If either behaviour is missing, build it here as a read-only disclosure (it
   may need `/add-doctor-check`), or stop with a DP.
2. **The four sentences that still call DP-028 open:**
   `docs/spec/06-cli-and-ux.md` (≈162), `README.md` (≈375), `wiki/FAQ.md` (≈70),
   and `CHANGELOG.md`'s unreleased section (≈837). Reword each to the ruling.
3. **Chapter 04.**
   - In the declared-deviation note slice 6 left (≈194–209), remove **only the
     source-build bullet**, now settled by DP-050. The other clauses stay
     declared.
   - Correct the "no longer available" example: the artifact is unavailable,
     unsupported or disabled, and a source build is a named cost.
   - Declare the residual removal window next to DP-030's.
4. **DP-036.** Remove pass 09's hedged reading of `.app` as the `.dmg`.
5. **Superseded work orders.** Put a one-line banner on `08-slice-7`, `08-slice-8`
   and `08-slice-9`: "Scope superseded by work order 16 (SE-109); run it through
   16's parts E, F, G and L." In `11-first-mutating-release.md` (≈58), replace
   "`--retain-source` is the default" with the ruling.
6. **Roadmap.** Rewrite `prompts/ROADMAP-to-first-release.md`: v0.2.0's shape, the
   sequence, the owner table and the status heading. Remove its interim banner.
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
   If no machine here has one, list it under "needs the owner".
3. Add attribution by `provides` first. Then add a **bounded** Portfile reader: a
   fixed set of recognised forms; anything else is unknown.
4. **One domain rule for both providers:** a package is a service if it is
   declared or materialised. Each adapter supplies its two halves. Unknown
   refuses the plan (INV-19).
5. **Replay.** Under `--replay` the reader answers from the recording or as
   unknown, and never from the live filesystem (INV-15). Extend
   `invariant_15_a_replayed_reader_never_consults_the_live_filesystem`.
6. **Review.** `security-reviewer` reviews the reader and the path rule of
   section 3 item 9.

### S — Finish slice 6: parts B and C

Run `08-slice-6-verification-before-removal.md` parts B and C:

- `PROVIDER_REPORTS_ACTIVE` evaluable;
- the destination re-read immediately before `REMOVE_SOURCE` is dispatched;
- the tripwire `the_switch_needs_the_destination_contract_evaluable` widened.

**Slice 6 part D is superseded** by DP-048 (c) and part K. Say so in an SE
entry, and do not build it.

### E — Slice 7, both routes of DP-049, and the dependency ADR

Run `08-slice-7-plan-tie-and-consent.md`, with these changes:

1. **One ADR, two purposes.** It adds `sha2` under ADR-0004, with
   `default-features = false` and the transitive set verified with
   `cargo metadata` per target. It also chooses part G's lock-probe mechanism:
   `libc` with one reviewed `unsafe` call in a single module, or a maintained safe
   wrapper. It supersedes ADR-0006 for those two purposes only. Update ADR-0006's
   status to name it. The dependency's `Cargo.toml` entry lands here; the probe's
   lands in G.
2. **Two routes to execute.** `migrate --yes --plan-digest <d>` for scripts, and
   an interactive confirmation of the same in-memory plan for a person at a
   terminal.

### F — Slice 8, as designed (DP-047 (b))

Run `08-slice-8-primary-and-resolution.md`, with one change: the executable list
comes from provider metadata where a provider exposes it, so `rg` for `ripgrep`
is planned rather than refused. That needs a research note if the read is new,
and the path rule of section 3 item 9.

### G — The lock probe (DP-030 (b))

Run `08-slice-9-detector-and-flip-proposal.md` **part A, option (b)**, using the
mechanism part E's ADR chose. It keeps that part's binding requirements:

- a provider-neutral detector trait in `domain`;
- lock locations held in `data/policy/`, never above `adapters/`;
- every `open` or `F_GETLK` error falls back to unknown;
- `Unknowable` under `--replay` (INV-15);
- a refusal on `Observed` and on the unknown fallback.

The lock locations come from R-17:

- Homebrew: every file under `<prefix>/var/homebrew/locks/`, including the
  suffix-less `update` and `vendor-install` locks;
- MacPorts: the registry lock under the **detected** `portdbpath`.

The claim is "a provider lock is held right now", never "idle". The refusal is a
named error with an exit code, added to `crates/domain/src/error.rs`'s contract
and the schema. Turn the `NOT_CHECKED` tripwire into a real answer, and keep its
failing test for a build that dispatches with no probe. The INV-17 review of the
MacPorts asymmetry goes in the SE entry.

### H — The four remaining destination checks (DP-051 (a))

The checks are `PROVIDER_INTEGRITY_CHECK`, `EXECUTABLES_OWNED_BY_DESTINATION`,
`NO_CROSS_PROVIDER_LINKAGE` and `LIVENESS_PROBE`.

1. **Research first.** One research note per check. A provider read that a check
   needs is staged (section 3 item 3) only after its note shows it read-only on
   both providers. A form that can write is refused, whatever it would check.
2. **`LIVENESS_PROBE`.** It runs the destination's resolved executable, which is
   outside the fixed allowlist.
   - **Design note first:** `docs/passes/16-liveness-design.md`. It says how the
     runner admits exactly that program without weakening the fixed allowlist or
     adding a second spawn site. `security-reviewer` passes it before any code.
   - **What may run.** Only a curated probe from a new dated data file. The
     entry holds argv only, never a program path; the program is the resolved
     executable under section 3 item 9.
   - **How it runs.** Sanitised environment (INV-10), no stdin, a scratch working
     directory, and ADR-0006's timeout. Only the exit status counts.
3. **Unchecked packages.** A package with no curated probe, or any check that
   cannot be evaluated for it, is reported unchecked, and its source is never
   removed.
4. **Run outcome.** Name the outcome a run ends with in that case. It is not
   `COMPLETED`. Give it a stable identifier in `--json` and a catalogue key in
   both locales.
5. **Tripwire.** The widened tripwire from part S must now fail only for a check
   that is genuinely unevaluable.

### K — Removal becomes opt-in (DP-048 (c), DP-037's confirmation, DP-050)

1. **Flip the default.**
   - Removal requires `--remove-source`. Change 06's synopsis.
   - Keep `--retain-source` as an accepted no-op for one release, with a
     deprecation notice, unless the owner rules otherwise.
   - Decide what `retainSource` means in `plan.v1` and `history.v*`: the same
     field with a new default, or a version bump. Write the decision into the
     schemas.
   - Adjust `restore`; SE-107 already notes that interaction.
   - Add an SE entry, and correct SE-109's compatibility line if the outcome
     differs.
2. **Define the two confirmation levels** in 06, with an SE entry. The level for
   normal-mode removal is stronger than an install's. The level for removal of a
   row entered through advanced mode (part C) is stronger still, so 06's "than in
   normal mode" stays true. Say how each level works under
   `--yes --plan-digest`, and whether a script can opt into each. Complete part
   C's invariant test.
3. **When `REMOVE_SOURCE` is dispatched.** Only when all of these hold: part S's
   re-read ran, part G's probe answered, and all six checks were evaluated for
   that package. Extend the INV-01, INV-03 and INV-04 tests.
4. **DP-050.**
   - Show the source-build risk in the default view, with what the build needs.
     If `destination_source_build` already does this, say so with the test that
     shows it.
   - If a filesystem read, with no new spawn, can detect that the Command Line
     Tools are absent: that absence makes a source-build step `MANUAL_REVIEW`,
     and `doctor` gets a finding for it (`/add-doctor-check`).
   - If it cannot be detected that way, write a research note that says why.

### I — Layer 4, consented (DP-047 (a))

Read `05-environment-and-primary-provider.md` and R-6 first.

1. **The offer.** When the destination does not win `PATH`, the plan offers one
   shim plus one managed `PATH` block for that executable.
2. **Without consent**, the gate refuses with `PRIMARY_NEEDS_SHIM` and names
   layers 1 and 2.
3. **Plan steps.** Both writes are plan steps with exact paths, covered by part
   E's digest and confirmation, with consent recorded per step (INV-02).
4. **What to build:**
   - a backup;
   - a preview;
   - `env --revert`;
   - restore support;
   - idempotence: running twice changes nothing (INV-16; extend its test);
   - a MacPortsBrew-owned shim directory, written with the durable write, that
     no symlink can lead out of;
   - shim names that follow section 3 item 9.
5. **Rules that bind.** INV-11 and INV-12. This is the first write outside the
   state directory, so `security-reviewer` on the plan is not optional. Tests
   run against a scratch home, never the real one.

### J — Elevation (DP-046 (b))

1. **Research note first:** what `sudo` passes on to the child on macOS
   (`env_reset`, `env_keep`, `secure_path`), and how its timestamp cache
   behaves.
2. **Then the design**, `docs/passes/16-elevation-design.md`, **then the ADR**.
   Both are reviewed before any code.
3. **The constraints are the ruling's:**
   - an absolute `sudo` path, argv only;
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
6. **Staging.** Adding `sudo` to the permitted programs is staged (section 3
   item 3), and the part stops until the owner applies it.
7. **INV-06.** Every new site answers an `invariant_06_*` test.

### L — The flip proposal, for the symmetric scope

Run `08-slice-9-detector-and-flip-proposal.md` part B, with both directions in
scope. The proposal accounts for:

- each of the seven tripwires and `this_build_does_not_dispatch_mutations`;
- every check added since;
- what remains for work order 11's acceptance in both directions, per
  architecture.

The proposal is the deliverable. The switch stays off.

### M — Harden the Claude workflows (DP-039 (b))

1. Stage hardened `claude.yml` and `claude-code-review.yml` under
   `docs/passes/16-staged/M/`, with a `README.md`, implementing the ruling's five
   points.
2. Add a `scripts/test-gates.sh` case if one can assert them. That case is
   staged too.
3. Stop. Installing them is the owner's armed message, and setting the key is
   the owner's.

## 6. Definition of done

Demonstrate each item; do not assert it.

- [x] Part 0 is confirmed (2026-10-27).
- [ ] Parts A–D, S, E–H, K, I and J are merged, each with
      green required checks. Part L's proposal is written. Part M is staged.
      Every stop is a DP, with the parts that continued past it.
- [ ] `grep -n THIS_BUILD_DISPATCHES_MUTATIONS` shows `false` on `main`.
- [ ] `git diff <pass start>..main -- crates/adapters/src/process/mod.rs
      data/policy/provider-commands.toml` is empty, or every hunk was applied by
      the owner from a staged README.
- [ ] Every invariant this pass touches points by name to its test, new ones
      included: 01, 02, 03, 04, 06, 10, 11, 12, 13, 15, 16, 17, 19, 20.
- [ ] Every new provider fact has a dated research note and a data entry.
- [ ] Part E's ADR exists, and ADR-0006's status names it and the purposes it
      covers.
- [ ] `/preflight` passes, or the report names what was skipped and why.
- [ ] `spec-guardian`, `security-reviewer` and `docs-auditor` findings are
      resolved or accepted in writing, per part.
- [ ] `docs/spec-evolution.md` records every material deviation, one entry per
      topic (SE-032).

### Needs the owner (collect here; never routed around)

- **Applying staged changes:**
  - the allowlist growth from parts H and J;
  - part M's workflows and test case;
  - any guarded script a part needs.
- **Captures no machine here can produce:**
  - a MacPorts port with a startup item;
  - an arm64 machine with both providers;
  - a machine with a shimmed `PATH`.
- **Release:** work order 11's acceptance in both directions on disposable
  machines, the ruling on the flip proposal, and the v0.2.0 tag.

## 7. End-of-pass report

Write it with `/pass-report` to `docs/passes/16-report.md`, one section per
part. Every material deviation it names must already be in
`docs/spec-evolution.md`.
