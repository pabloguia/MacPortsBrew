<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — Read-only maintenance after pass 08 slice 5 (14)

Written 2026-10-18. Pass 08 slices 4 and 5 merged; v0.1.0 is not tagged yet
(work order 15 prepares it); the mutating track continues in slices 6–9. This
pass clears what the reports and `docs/spec-evolution.md` list as owed **in the
read-only build**, and writes the research note slice 6 cannot start without.
Like pass 12, it is sized for a session that is not the strongest model: every
item is research, data, documentation, an additive read or a test, and every
item has a reviewer that runs on a stronger model.

Nothing in this pass makes the tool change a machine, and nothing in it widens
what the tool may run.

---

## 1. Read this first

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/00-charter.md`,
   `docs/spec/01-invariants.md` — INV-15, INV-17 and INV-20 especially.
2. `docs/spec-evolution.md`: SE-094 ("Recorded, not changed"), SE-095 to SE-100.
   Do not reopen what they settled.
3. `docs/passes/12-read-only-improvements-report.md` and
   `docs/passes/08-slice-5-report.md`.
4. `docs/spec/11-agentic-sdlc.md` once, then the chapter each item names.
5. The skill for each item (`/research-note`, `/add-mapping`,
   `/add-doctor-check`, `/spec-evolution`, `/preflight`, `/pass-report`). They
   are the procedure.

## 2. Authority and precedence

As `TEMPLATE-PASS.md` §2.

## 3. Hard boundary for this pass

Pass 12 §3 ([`12-read-only-improvements.md`](12-read-only-improvements.md))
applies, except that the topic branch is pushed and a pull request opened (SE-099): the switch stays `false`; `git diff --stat main` is empty on
`crates/cli/src/executor/`, `crates/application/src/executor*`,
`execution_gate.rs`, `crates/domain/src/verification.rs`,
`crates/application/src/planner.rs` and `crates/adapters/src/process/`; no new
dependency, `unsafe` or network; no guarded path (`.claude/hooks/`,
`.claude/settings*.json`, `scripts/*.sh`, `.github/workflows/`, `.codex/`,
`.agents/skills/`); `--json` changes additive and documented; fixtures captured,
never invented; nothing pushed except the topic branch, nothing tagged.

Research items run **read-only** provider commands only (`brew info`, `brew
list`, `port installed`, `port info` and the like). A mutating command is a
failure even if a hook did not stop it.

Item J is the one exception, and only inside the owner's armed message (§5).

## 4. Working protocol

As pass 12 §4: plan each item in a few lines, build, prove (every new rule has a
test, proved by a killed mutant), review, record; one Conventional commit per
item with `cargo fmt --all` as its own command first; reviewers on the strongest
model available through the Agent tool's `model` parameter — `spec-guardian` for
every code item, `security-reviewer` for anything that reads the filesystem,
`docs-auditor` for every documentation item; research goes to
`provider-researcher`.

## 5. Order of work

Ordered by value and by what waits on it.

- **A. R-20 — what a provider's read shows as "active"** (`/research-note`,
  `provider-researcher`). Slice 6 cannot start its Part B without it, and it is
  SE-094's owed note. Observe on this machine and cite: MacPorts — the exact
  rendering and whitespace of `(active)` / `(inactive)` in `port -q installed`
  and `port -q installed <name>`; Homebrew — `linked_keg` in `brew info
  --json=v2 --installed` for a linked formula, a keg-only one and an unlinked
  one, and what a cask entry carries. Record the provider versions and read
  dates. Say plainly what remains unobserved (for example a fresh install, which
  this pass must not perform). Add R-20 to `docs/spec/09-research-checklist.md`
  and update SE-094's owed bullet to point at it.
- **B. `node@20` and its disable date** (`/add-mapping`). The curated row
  `node@20` ↔ `nodejs20` (`data/policy/curated-mappings.toml`, around lines
  207–216: Homebrew 7.0.4, homebrew-core `e045d7385fd`, index as of 2026-09-02,
  read 2026-10-12) records that Homebrew disables `node@20` on 2026-10-28.
  Re-read it against the **local** indexes (`brew update` and `port sync` are
  denied, and this pass has no network), keep `index_as_of` honest (DP-043),
  and decide what the row and the policy say about a formula that will be
  disabled. The disable itself becomes visible only after the owner updates the
  indexes; say so in the report rather than claiming it.
- **C. Schema truth.** Add `summary.ownershipUnknown` to the Summary table of
  `docs/schemas/analysis.v1.md` (it is emitted and undocumented; SE-097
  "Owed"). Add a checked JSON sample to `docs/schemas/snapshot.v1.md`, wired into
  `crates/cli/tests/documented_samples.rs`, so `environment.installations`
  appears in a checked golden (SE-095).
- **D. The "ownership unknown" count** (SE-097 "Owed"). `status` counts mapped
  rows that are not installed and says "the provider kept no record" of them.
  Recommended: exclude those rows from the count, since the sentence is about
  installed packages; if you find the wording is what should change instead,
  say why in SE-097. Update both locales, goldens, and the README/wiki samples.
- **E. A seam for `make_durable`** (slice 5 mutation-proof survivors).
  `make_durable` is in `crates/adapters/src/persistence/history_store.rs`
  (around line 250); its only product caller is
  `crates/cli/src/app/interrupted.rs` (around line 232). Model the seam on the
  `ReadBack` seam in `crates/cli/src/executor/mod.rs` (around line 414) —
  **read it, do not edit it**: that file is out of bounds (§3). Put the seam in
  `history_store.rs` or `interrupted.rs`, so a test can fail the second read,
  the equality check and each `sync_all`, and kill "clear the marker before
  `make_durable`". `security-reviewer` reviews it: the seam must not be
  reachable from product code.
- **F. Stale prose** (`docs-auditor` first, then fix): `docs/spec/11-agentic-sdlc.md`
  still says the remote does not exist and the first push is the owner's
  (SE-099 allowed topic pushes); ADR-0005 says the owner-writable `PATH`
  finding "does not exist yet" while `PathFindingKind::OwnerWritable` exists —
  verify, then reconcile; `docs/spec/09-research-checklist.md` R-11 has no
  pointer to R-19. Spec edits get their SE line. Wording you think `CLAUDE.md`
  needs (the Stop gate's cap, shell readers loading no rule) goes into the
  report as a proposal for the owner; do not edit `CLAUDE.md`.
- **G. `Recommendation` deserialises with no reasons** (SE-094). A `TryFrom`
  that refuses an empty `reasons` list, with its test.
- **H. R-18's open question about `O_CLOEXEC`**, answered from the Rust standard
  library's source at the pinned toolchain version — from the local toolchain's `rust-src` if it is installed, never from the network — cited with a read date; if it is not available locally, leave the question open and say so.
- **I. Optional, if time remains: `doctor` reports a non-empty `/usr/local`**
  (SE-098 "Owed", F-2), with `/add-doctor-check`: a filesystem read through the existing `FilesystemScope` — not a new process (if it needs one, it is a new system probe and the owner's; stop) — a new additive capture fact,
  `doctor.v1.md` and `snapshot.v1.md` updated, an INV-15 test that a replayed
  or snapshot run never looks at the live directory, both locales.
- **J. Only inside the owner's armed message.** The `add-locale` skill's step 5
  disagrees with `CONTRIBUTING.md` and the code (pass 12 report); six skills
  use the `Bash(cargo test:*)` form and other wording drifts listed in
  `docs/research/tooling/2026-10-17-subagents-skills.md`. Fix
  `.claude/skills/*` and `.claude/agents/*`, then run
  `scripts/generate-codex-layer.sh` — whose output is guarded, which is why this
  needs the owner's message starting `MPB_ALLOW_GUARD_EDIT=1`, covering that
  message only. Without it, list the exact edits in the report and stop.

## 6. Definition of done

- [ ] Each item done with its test (killed mutant named) or research note, or
      listed in the report as not done with the reason.
- [ ] §3 holds: the `git diff --stat` on the listed paths is empty, `Cargo.lock`
      is unchanged, no guarded path changed outside an armed message.
- [ ] `/preflight` passes, or the report names what was skipped and why.
- [ ] Every material change is in `docs/spec-evolution.md`; CHANGELOG
      `[Unreleased]` updated.
- [ ] Reviewers have run on the strongest model; findings resolved or accepted
      in writing.
- [ ] Topic branch pushed and a pull request opened; never merged by the agent.

## 7. End-of-pass report

`/pass-report` to `docs/passes/14-read-only-maintenance-report.md`. Include the
proposals for `CLAUDE.md` (item F) and, if item J was not armed, its exact edits.
