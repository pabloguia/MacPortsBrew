<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — Read-only improvements after 0.1.0 (12)

v0.1.0's release candidate exists (pass 10) and passed an exhaustive
pre-release review (SE-094). This pass improves the **read-only** build in the
places that review and the specification name as owed, and that can be done
without touching the code that could ever change a machine. It is sized for a
session that is not the strongest model available: every item is presentation,
data, an additive read, or a test, and every item has a reviewer that runs on
a stronger model (§4).

Nothing in this pass makes the tool change a machine, and nothing in it
widens what the tool may run.

---

## 1. Read this first

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/00-charter.md`,
   `docs/spec/01-invariants.md` — especially INV-15 (a replayed or snapshot
   run never consults the live machine), INV-17 (no provider named above
   `crates/adapters/`; both directions are peers) and INV-20 (dated facts are
   data with `source` and `read_on`).
2. `docs/spec-evolution.md`, **SE-094** in full: what was fixed before 0.1.0
   and, under "Recorded, not changed", what is owed. Do not reopen what it
   settled.
3. `docs/passes/10-read-only-release-candidate-report.md`.
4. `docs/spec/11-agentic-sdlc.md` once, then the chapter each item names.
5. The skill for each item (`/add-locale`, `/add-doctor-check`,
   `/add-mapping`, `/spec-evolution`, `/preflight`, `/pass-report`). They are
   the procedure; follow them rather than improvising one.

## 2. Authority and precedence

As `TEMPLATE-PASS.md` §2. Tier 1 and Tier 2 are not yours to change; if one
looks wrong, stop and write it in the report.

## 3. Hard boundary for this pass

- `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`. **No file changes** under
  `crates/cli/src/executor/`, `crates/application/src/executor*`,
  `crates/application/src/execution_gate.rs`, `crates/domain/src/verification.rs`,
  `crates/application/src/planner.rs` (step ordering is INV-01) or
  `crates/adapters/src/process/` — `git diff --stat` on those paths is empty
  at the end. The process allowlist does not grow: a new provider subcommand
  or a new system probe is the owner's decision (§7).
- No new dependency (`Cargo.lock` unchanged), no `unsafe`, no network.
- No guarded path is edited: `.claude/hooks/`, `.claude/settings*.json`,
  `scripts/*.sh`, `.github/workflows/`, `.codex/`, `.agents/skills/`. If a
  hook blocks something that looks legitimate, stop and say so in the report;
  never reword a command to slip past a guard.
- `--json` changes are **additive only**: `schemaVersion` unchanged, every new
  field documented in `docs/schemas/*.v1.md` and visible in a checked golden.
- Fixtures are captured, never invented. Where a test would need a machine
  this repository has no recording of, test the adapter through its existing
  seams (temporary directories, the `FilesystemScope` and runner fakes the
  crate already uses) and record the missing recording as owed.
- Nothing is pushed, tagged or published.

## 4. Working protocol

As `TEMPLATE-PASS.md` §4, and:

- **Plan before each item** (the plan can be a few lines in the report's
  draft); build; prove; review; record.
- **One Conventional commit per item**, `cargo fmt --all` as its own command
  before committing; the commit gate runs `scripts/validate.sh`. Commits end
  with the `Co-Authored-By` line the session gives you.
- **Every new rule has a test, and the test is proved by mutation**: break
  the rule by hand, see the test fail, restore. Say in the commit or report
  which mutant was killed.
- **Every material decision is recorded in `docs/spec-evolution.md` in the
  same change** (`/spec-evolution`; merge into the topic's entry, SE-032).
- **Reviewers run on the strongest model**, through the Agent tool's `model`
  parameter: `spec-guardian` for every item, `security-reviewer` for item B
  and anything that reads the filesystem, `docs-auditor` for A, D and F. You
  are not the right reviewer of your own change. Fix or answer every finding
  in writing.
- **Delegate read-only exploration** to subagents; keep the main session for
  decisions and edits.
- An owner decision (money, licence, audience, or growing the process
  allowlist) goes to `docs/decisions-pending.md` with your recommendation
  (`/decision-pending`); take the reversible default and keep going.

## 4a. Running unattended

The owner starts this pass and leaves it running overnight. Nobody answers a
question until morning, so:

- **Never stop to ask.** Do not use `AskUserQuestion`, do not end a turn with a
  question, and do not wait for approval of a plan: write the plan into the
  report draft and proceed. A choice that is the owner's goes to
  `docs/decisions-pending.md` with the reversible default taken (§4); a
  blocked item is recorded and skipped (§5). The only reason to end the session
  early is that §3 cannot be held.
- **Work on a branch.** Before the first edit, `git switch -c
  pass/12-read-only-improvements` from the current `HEAD` (if it exists, switch
  to it and continue from its last commit). If `git status` shows this work
  order, `prompts/README.md`, `prompts/ROADMAP-to-first-release.md` or
  `.claude/current-pass` uncommitted, commit exactly those four first as
  `docs(prompts): pass 12 work order for read-only improvements` — the commit
  gate may take several minutes; let it finish. Commit after every item so a crash
  or a compaction loses at most one item.
- **Keep a progress log that survives compaction.** Create
  `docs/passes/12-read-only-improvements-report.md` first, with a "Progress"
  section listing A–F as `todo` / `done <commit>` / `blocked: <why>`, and update
  it in each item's commit. After any compaction or restart, read that section
  and `git log --oneline -15` before doing anything else, and resume at the
  first item not marked done.
- **A failing gate is work, not a stop.** The `Stop` hook refuses to end a turn
  while `validate.sh --quick` fails, and the commit gate refuses a commit while
  the default mode fails. Diagnose and fix (`superpowers:systematic-debugging`
  if it helps). If the same failure survives three honest attempts, revert that
  item's uncommitted changes (`git restore`/`git checkout` on its files only),
  mark it `blocked` with the failure output, and move on.
- **A blocked tool call is information.** A hook or permission denial means
  the boundary did its job: never retry it reworded. Record it and move on.
- **Budget.** Prefer A, B, C, E, D, F order if time looks short: D is the most
  open-ended; cap it at roughly twenty well-cited rows. F always runs last,
  even if earlier items are blocked, so the morning report exists.

## 5. Order of work

Each item stands alone; if one blocks, record why and move to the next.

**A. A Brazilian Portuguese catalogue** (`/add-locale`, `06-cli-and-ux.md`).
`locales/pt-BR.toml` covering every key in `locales/en.toml`, wired so that
`--language pt-BR`, `MACPORTSBREW_LANG`, `LC_ALL` and `LANG` select it in the
documented order and anything missing falls back to English. Only text shown
to a user is translated: identifiers, `--json` output, error ids, flags and
command names stay English. Keep placeholders and plural forms exact; the i18n
test and `validate.sh`'s i18n stage must pass. The tone is the README's: plain,
no jargon, second person.

**B. The dual-Homebrew check `doctor` owes** (`/add-doctor-check`;
`04-migration-and-safety.md`, "Cross-provider contamination": "a
dual-Homebrew-prefix check on Apple Silicon"; SE-094 records it missing).
Detection today takes the first candidate prefix that holds the provider's
executable and says nothing about a second. Make the adapter report every
candidate prefix (from `data/policy/provider-prefixes.toml`, never a path in
code) that holds the executable, carry that in the snapshot as an additive
field, and add a `doctor` finding when a provider has more than one
installation on this machine. The finding and the code above the adapter
**name no provider and no path**: "a package manager is installed in two
places" is generic, and on an Apple Silicon Mac it is exactly the case 04
describes. Under replay and `--from-snapshot` the answer comes from the
recording or snapshot, never from the live disk (INV-15); a recording that
predates the field answers "not known", not "one". While here, confirm each
other MUST in that section ("architecture mismatches" included) has a check
and a test, and list any that does not.

**C. Say when an ownership answer was a default** (`02-domain-model.md`:
"Record which mechanism produced each classification, and its confidence";
SE-094 owes it). When reverse dependencies could not be read, a package
nobody asked for is `DEPENDENCY` by default, but its evidence still reads as
the provider's intent record alone. Add to the ownership evidence an
additive marker that the dependency/orphan split was not answered, render it
in `explain` (human and `--json`), document it in the schema, regenerate the
affected goldens with `scripts/run-fixture-matrix.sh --update` on a freshly
built binary, and check that nothing else changes in them.

**D. More curated mappings** (`/add-mapping`). 0.1.0 matches a package only
when an equivalent is already installed or the curated table names one, so
the table is the cheapest way to make plans useful. Add rows for widely used
packages whose names differ between the two providers (language runtimes and
their versioned names, `gnu-*` / `g*` tools, libraries with `lib` prefixes,
and the like), in both directions. **The admission rule in the file's header
binds**: a row exists only when the destination name was read from that
provider's own index on `read_on`, with the source cited — observe with
read-only commands (`brew info --json=v2 <name>`, `port info <name>`) or the
providers' official package pages, and drop anything you could not confirm.
Bump the policy data version as the skill says. Aim for quality over count;
twenty well-cited rows beat a hundred guessed ones.

**E. Two tests that claim more than they check** (SE-094 re-review).
`crates/cli/tests/layering.rs`: (1) the provider-name scan stops at the first
`#[cfg(test)]` and matches only quoted literals — make it skip only the test
modules themselves, and also refuse a path into a provider's adapter module
(`adapters::<provider>`), proving each by a planted violation in a temporary
copy or a unit test of the scanner; (2) rename
`invariant_14_nothing_in_the_build_can_open_a_network_connection` to what it
checks (no network crate in the lock file, no socket type in the source) and
keep it an `invariant_14_*` test.

**F. Record and report.** `CHANGELOG.md` gets an `[Unreleased]` section above
`0.1.0`'s with the user-facing changes; `README.md` and the wiki mention
`pt-BR` and the new `doctor` finding where they list languages and checks;
SE entries for B and C (and A if the i18n layer changed); the pass report in
`docs/passes/12-read-only-improvements-report.md` (`/pass-report`).

## 6. Definition of done

- [ ] A: every `en` key has a `pt-BR` translation; locale selection and
      fallback tested; i18n stage green.
- [ ] B: the finding exists, names no provider above the adapter, answers from
      the recording under replay, and is tested for one, two and "not known";
      the remaining 04 "Cross-provider contamination" MUSTs are confirmed or
      listed as owed.
- [ ] C: the marker is in the evidence, the schema, `explain` and the goldens;
      only the expected golden lines changed.
- [ ] D: every new row is cited and dated; the policy version is bumped.
- [ ] E: both test changes made, each proved by a killed mutant.
- [ ] §3 holds (`git diff --stat` on the protected paths is empty;
      `Cargo.lock` unchanged).
- [ ] `scripts/validate.sh` passes (the commit gate runs it on every commit),
      coverage floors hold.
- [ ] Reviewers ran on the strongest model; every finding fixed or answered.
- [ ] Report written, with what is owed and what needs the owner.

## 7. Not in this pass, and why

- **Searching the other provider's whole catalogue for a match.** It needs
  provider reads the process allowlist does not permit today, and growing the
  allowlist is the owner's decision. File it with `/decision-pending` (which
  reads, their cost, offline behaviour, your recommendation) and stop there.
- **Telling a refused spawn, a missing program and a fork failure apart**
  (`NotStarted`, SE-094): a change in `crates/adapters/src/process/`, held for
  the mutating track.
- **Pass 08 slices 4–6 and pass 11** (wiring, consent, history, restore, the
  detector, the flip): the mutating track, run on the strongest model.
- **A macOS 10.12–11 recording** for architecture detection: needs that
  machine; record it as owed if it is still missing.
