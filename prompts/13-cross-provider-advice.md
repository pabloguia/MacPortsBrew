<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — Direction-specific cross-provider advice, as data (13)

`04-migration-and-safety.md` ("Cross-provider contamination") makes it a MUST
that `doctor` implements the MacPorts-on-`PATH` check itself, **and** that its
wording says the two directions are not equally risky: MacPorts' remedy for
`/usr/local` interference is opt-in trace mode (`port -t`), whereas Homebrew's
compiler shim drops `-I`/`-L` flags under `/opt/local` on every source build, so
the residual risk toward Homebrew sits in `PATH`-resolved tools (F-2). Today
`doctor` has only provider-neutral findings (`path_order`, `prefix_not_on_path`,
`build_variable_names_prefix`), and no sentence says which direction is riskier.
SE-095 listed the MUST as owed and filed DP-041; **the owner ruled DP-041 (b) on
2026-10-12**: carry the direction-specific advice as **dated data**, selected by
a **provider-neutral rule**, never as a branch on a provider name (INV-17,
INV-20).

This pass builds exactly that, and nothing that touches a machine. The
repository starts at the end of pass 12 (`0d524cc` or later): `THIS_BUILD_DISPATCHES_MUTATIONS`
is `false`, `doctor` knows both providers from the snapshot, and `PolicyData`
loads six data files with `source` and `read_on` on every row.

---

## 1. Read this first

In this order, before doing anything else:

1. `AGENTS.md` — orientation.
2. `docs/spec/00-charter.md` — especially the non-goals (no network required to
   run; the tool reports and never rewrites a binary).
3. `docs/spec/01-invariants.md` — Tier 1. INV-17 (no provider named above
   `crates/adapters/`; both directions are peers), INV-20 (dated facts are
   data with `source` and `read_on`), INV-15 (a replay or snapshot run never
   consults the live machine), INV-12 (nothing here sets a priority).
4. `docs/spec-evolution.md` — **SE-095** (where the MUST was recorded as owed),
   SE-094 (the staleness date is the newest effective date), SE-097 and DP-043
   (`index_as_of`), SE-011's addenda (the catalogue, `pt-BR` ships).
5. `docs/spec/11-agentic-sdlc.md` once, then
   `docs/spec/04-migration-and-safety.md` "Cross-provider contamination",
   `docs/spec/03-policy-and-recommendations.md` "Policy data", and
   `docs/research/F-2-usr-local-interference.md` **in full**: its findings 1, 4
   and 6, its "Cannot determine" list and its "Re-check when" list are this
   pass's starting facts and its first task.
6. `docs/decisions-pending.md`, DP-041 (Resolved) and DP-043.
7. `.claude/rules/policy-data.md`, `.claude/rules/adapters.md`,
   `.claude/rules/domain.md`, and the skills `/add-doctor-check` (step C),
   `/research-note` (step A), `/spec-evolution`, `/preflight`, `/pass-report`. They are
   the procedure; follow them rather than improvising one.

## 2. Authority and precedence

As `TEMPLATE-PASS.md` §2. Tier 1 and Tier 2 are not yours to change; if one
looks wrong, stop and write it in the report. The 03 and 04 MUSTs are Tier 3:
amending them is allowed with an SE entry, and this pass amends two sentences
(§5, D).

## 3. Hard boundary for this pass

- `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`. **No file changes** under
  `crates/cli/src/executor/`, `crates/application/src/executor*`,
  `crates/application/src/execution_gate.rs`, `crates/domain/src/verification.rs`,
  `crates/application/src/planner.rs` or `crates/adapters/src/process/`;
  `git diff --stat` on those paths is empty at the end. No new provider
  subcommand, no new system probe, no change to `provider-commands.toml`.
- **`doctor` reads no filesystem and spawns nothing for this finding.** It
  decides from the snapshot (both providers present, `environment.path`) and
  the policy data. Under `--replay` and `--from-snapshot` the answer therefore
  comes from the recording or snapshot (INV-15); an empty recorded `PATH` is
  "not known", reported once as today (`path_unknown`), never "not on `PATH`".
- **No provider name and no provider path in any Rust file above
  `crates/adapters/`** (INV-17), checked by `scripts/check-layering.sh` and
  `crates/cli/tests/layering.rs`. Provider ids appear only in the data file
  (as `provider = "..."`, the precedent every other data file follows) and in
  catalogue text. The rule is handed an id and looks it up; it never compares
  one to a literal.
- No new dependency (`Cargo.lock` unchanged), no `unsafe`, no network.
- No guarded path is edited: `.claude/hooks/`, `.claude/settings*.json`,
  `scripts/*.sh`, `.github/workflows/`, `.codex/`, `.agents/skills/`. If a hook
  blocks something that looks legitimate, stop and say so in the report; never
  reword a command to slip past a guard. (The i18n parity script is a guarded
  path: keys are handled with `# unreferenced:` markers, §5 C, not by editing it.)
- `--json` changes are **additive only**: `schemaVersion` unchanged, the new
  finding id documented in `docs/schemas/doctor.v1.md` and visible in a checked
  sample (§5 D).
- Fixtures are captured, never invented. A synthetic third provider in a *unit*
  test of the rule is allowed (neutral ids such as `alpha`/`beta`, built in the
  test); a recording is not edited.
- Nothing is pushed, tagged or published.

## 4. Working protocol

As `TEMPLATE-PASS.md` §4, and:

- **Plan before each step** (a few lines in the report draft); build; prove;
  review; record. **One Conventional commit per step** (§5 A to D); `cargo fmt
  --all` as its own command first.
- **Every new rule has a test, proved by mutation**: break the rule by hand,
  see the test fail, restore, and say which mutant was killed.
- **Every material decision lands in `docs/spec-evolution.md` in the same
  commit** (`/spec-evolution`, merged per topic, SE-032). The next free
  identifier is SE-098.
- **Reviewers on the strongest model**, through the Agent tool's `model`
  parameter: `spec-guardian` for every step, `docs-auditor` for A and D;
  `security-reviewer` is not needed unless a step starts reading the
  filesystem, which this pass must not do. Fix or answer every finding in
  writing.
- **Delegate read-only exploration and the research re-read** to subagents;
  keep the main session for decisions and edits.
- An owner decision (money, licence, audience, the process allowlist) goes to
  `docs/decisions-pending.md` with your recommendation (`/decision-pending`);
  take the reversible default and keep going.

### 4a. Running unattended, and the commit gate

If the owner leaves it running: never stop to ask, do not use `AskUserQuestion`,
work on `pass/13-cross-provider-advice` (from the current `HEAD`; if it exists,
switch to it), commit the work order first as `docs(prompts): pass 13 work order
for cross-provider advice`, and keep a "Progress" section in
`docs/passes/13-cross-provider-advice-report.md` (A to D as `todo` /
`done <commit>` / `blocked: <why>`); after a compaction read it and `git log -n
15 --oneline` before anything else. A failing gate is work; if the same failure
survives three honest attempts, revert that step's uncommitted files, mark it
`blocked` and move on. A blocked tool call is information, never retried
reworded. D always runs last so the report exists.

What pass 12 learned about the commit gate, so it costs nothing this time:

- `commit-gate.sh` runs the whole suite on any commit that touches `crates src
  tests fixtures locales data scripts .claude Cargo.*`, with a 1380 s deadline.
  On a quiet machine the suite takes about 15 minutes; a load spike makes the
  hook refuse the commit "did not finish". **Run `scripts/validate.sh` yourself
  first, in the background, and commit only after it passes**, then `git commit`
  once the machine is quiet. Check `uptime` before blaming the code. Never use
  `MPB_SKIP_COMMIT_GATE`, and never kill processes that are not yours.
- Do not run `git commit` for a long gate inside a call you cannot afford to
  lose: a session that ends mid-hook leaves nothing behind.
- Write the commit message **in the same command** as `git commit -F -` (a
  here-document); the scratch directory is cleaned between calls.
- `setsid` does not exist on macOS; use the Bash tool's background mode.

## 5. Order of work

Each step stands alone; if one blocks, record why and move to the next, except
that B depends on A and C on B. The order matters because the *facts* decide
the data and the data decides the code: an advice sentence written before it is
re-read from its source is a provider fact stated from memory, which is the
failure this project is designed against.

### The design, fixed in advance (change it only with a recorded reason)

A new data file **`data/policy/cross-provider-advice.toml`**, one `[[entry]]`
per provider whose builds and tools can be exposed to another provider's
prefix (the **affected** provider). Fields: `provider`, `build_defence`,
`source`, `read_on`, optional `index_as_of` (§5 B), `note`.

`build_defence` is a **closed vocabulary of stable identifiers**, one Rust enum
in `crates/policy`, never prose: `opt_in_trace` (the provider can sandbox a
build against other prefixes, but only when asked) and `default_clean_path` (the
provider's own build tooling drops other prefixes' search paths on every source
build). Adding a third mechanism later is a new variant, a catalogue entry and
a data row, and the compiler points at all three.

A **provider-neutral rule** in `crates/application/src/doctor.rs`: for every
ordered pair (**affected** A, **intruder** B), both `Present` in the snapshot,
emit one finding `cross_provider_exposure` with `provider = A`:

| Condition | Severity |
|---|---|
| `environment.path` is known and B's `bin` directory precedes A's, or A's is absent from it | `WARNING` |
| otherwise (including an unknown `PATH`) | `OK` |

Params: `affected`, `intruder`, `intruder_prefix` (B's detected prefix, as the
other findings already carry), `defence` (the **identifier**, rendered through
the catalogue by the existing identifier-family mechanism, so the sentence for
`opt_in_trace` differs from the one for `default_clean_path`). An affected
provider with no entry in the data gets `cross_provider_exposure_unadvised`
(`OK`): the pair is reported, no direction-specific claim is made - an unknown
is never a "safe".

This is the whole MUST: the check is `doctor`'s own, it never defers to the
provider, and the sentence says which direction is riskier because the data
differs per provider, not because any line of code does.

### A. Re-read the facts, dated (`/research-note`)

Re-read **from the source**, read-only, and record the date and the commit
or version of each, as an addendum to
`docs/research/F-2-usr-local-interference.md` (its own "Re-check when" asks for
exactly this whenever the contamination check is touched):

1. MacPorts Guide section on `port install` (trace mode, `-t`) and `port(1)`
   `-t`, at <https://guide.macports.org/>. The MacPorts FAQ was unreadable
   (Anubis); cite the Guide only, as F-2 does, and say so again if still true.
2. Homebrew's superenv compiler shim's `keep?` logic: read the **local**
   checkout read-only (`brew --repo`, `git -C … log -1 --format=%H%n%cI`, then
   the file under `Library/Homebrew/shims/`), and cite the upstream path and
   the commit it was at; the local checkout is only as new as its last update,
   so record that commit's date as the **index date** (§B `index_as_of`).
3. F-2's "Cannot determine" items, answered if the shim file answers them
   cheaply: does it also cover `-isystem`, `-F`/`-iframework`, `rpath`? Whether
   `brew`'s own subprocesses see `/opt/local` on `PATH` is **not** answerable
   from the shim; leave it unanswered and make sure no sentence claims it.
4. Whether MacPorts' default configuration enables trace mode (F-2 grepped,
   did not read exhaustively): read `macports.conf` in full, read-only, and say
   what it says.

Output: the addendum, and a two-line table of **what each sentence in C may
claim** (and what it may not). No catalogue text is written until this exists.
Commit: `docs(research): re-read F-2's contamination facts`.

### B. The data file and its loader

1. `data/policy/cross-provider-advice.toml` with one entry for each provider in
   `provider-prefixes.toml`, `build_defence` from A's findings, `source` precise
   enough to re-read, `read_on` the day of A, and `index_as_of` for a fact read
   from a local checkout. Header in the style of the other files, including the
   admission rule: **a row exists only for a fact read from the provider's own
   documentation or source on `read_on`**.
2. `crates/policy/src/data.rs`: a `cross_provider_advice` field in
   `PolicySources` (and `embedded_sources()`, `include_str!`), the entry type
   and the `BuildDefence` enum, validation in `load_from` (source mandatory,
   dates valid, `index_as_of` never after `read_on` - **reuse the check pass 12
   wrote for curated rows, do not write a second one**, one entry per provider,
   the provider id is a safe name), the provenance fed to `newest_provenance`
   so staleness measures the **effective** date (SE-094, DP-043), and an
   accessor `cross_provider_advice_for(&ProviderId) -> Option<BuildDefence>`.
   `PolicySources` is built literally in `embedded_sources()` (same file)
   and in `crates/policy/tests/invariants.rs`; fix both literals.
3. Bump `policy.toml`'s version by the rule (`<newest read_on>-<serial>`), then
   every place that states it (`grep` the old string; the pass-12 commit
   `0d524cc` shows the list: README sample, wiki, issue-template placeholders,
   schema samples, goldens via `scripts/run-fixture-matrix.sh --update` on a
   freshly built binary - and check that **only** the version moved in them).
4. Tests in `crates/policy`: the file loads; an entry with no source, a bad
   date, a duplicate provider or an `index_as_of` after `read_on` is refused;
   the accessor answers `None` for a provider with no entry (never a default);
   extend `invariant_20_provider_facts_are_dated_data` to the new rows. Mutants:
   drop the duplicate check; ignore `index_as_of`; make the accessor default.

Commit: `feat(policy): dated cross-provider advice data (pass 13, B)`.

### C. The neutral rule, the catalogue, and the proof it is neutral

1. `doctor.rs`: the rule as designed, in a helper beside `installation_finding`
   (copy its shape: a pure function of ids, data and the snapshot, returning
   `(&'static str, Severity, Reason)`). **No provider literal.**
2. Catalogue (`locales/en.toml` and `locales/pt-BR.toml`, both, parity is
   enforced): `doctor.cross_provider_exposure` (names affected, intruder, the
   intruder's prefix and carries `{defence}`), `doctor.cross_provider_exposure_unadvised`,
   and one sentence per `BuildDefence` under a `defence.*` family
   (`defence.opt_in_trace`, `defence.default_clean_path`). Each sentence claims
   only what A's table allows, says "report only - nothing was changed", names
   the provider-native remedy as a suggestion (the trace-mode flag, reinstalling
   from the owning provider) and never offers to run or rewrite anything (04's
   MUST NOT). Add `("defence", "defence")` to `FAMILIES` in
   `crates/cli/src/i18n/mod.rs` and extend `crates/cli/tests/reasons_catalogue.rs`
   so the `defence` parameter is held to that vocabulary. The `defence.*` keys are
   rendered through the family mechanism, so the parity script sees them as
   unreferenced: give each the `# unreferenced:` marker with the reason, as the
   file's own convention says; do **not** edit the script.
3. A test that every `BuildDefence` variant has a catalogue sentence in **both**
   languages (the compiler cannot see a data-driven key; this test is the net).
4. Tests in `crates/application/tests/doctor_findings.rs`, assertions on
   identifiers and severities, never on English:
   - the two directions get **different** `defence` for the same snapshot shape
     (the property the MUST names), with the real data;
   - **neutrality proof**: a synthetic snapshot with two neutral ids and a
     synthetic `PolicyData` (via `load_from`) gives the pair advice that follows
     the *data*; swap the two rows and the advice swaps. A third provider id with
     no row gets `_unadvised`;
   - `Warning` iff the intruder's `bin` precedes the affected one, `OK` when it
     follows, when it is the same position, and when `PATH` is empty (replay);
     one provider present gives no finding; an absent or broken provider gives
     none.
   Mutants: invert the precedence comparison; always `OK`; look the advice up by
   the wrong end of the pair; emit for a provider that is not `Present`.
5. `scripts/check-layering.sh` and `crates/cli/tests/layering.rs` pass: that is
   the INV-17 evidence. Do not weaken them.

Commit: `feat(application,cli): doctor says which direction is riskier (pass 13, C)`.

### D. Record (`/spec-evolution`, `/pass-report`)

- `docs/spec/04-migration-and-safety.md`: the MUST's sentence now says the
  wording comes from dated data keyed by the affected provider (amend, do not
  rewrite); `docs/spec/03-policy-and-recommendations.md`: add the new file to
  its list of policy data.
- `docs/spec-evolution.md`: **SE-098** for the design and its evidence, and an
  addendum to SE-095 ("the MUST is met, SE-098"); `docs/decisions-pending.md`:
  DP-041 gets "implemented" and the commit.
- `docs/schemas/doctor.v1.md`: the two new finding ids in the table, a prose
  paragraph, and **a checked sample of the finding**: a marker with
  `at=<pointer>` on `--replay fixtures/recordings/both-providers-arm64-macos15.json
  --json doctor` (both providers present, so the finding appears; the `PATH`
  is unknown there, so it is `OK`, which is the replay behaviour worth
  showing). Regenerate samples with a fresh binary and run
  `cargo test -p macportsbrew --test documented_samples`.
- `README.md`, `wiki/CLI-Reference.md` (`doctor`), `wiki/Troubleshooting.md`
  (a short "two package managers, one `PATH`" entry that says what the
  finding means and what to do - reorder `PATH` yourself, or run the
  provider's own repair; nothing is changed for you), `CHANGELOG.md`: if 0.1.0
  is still untagged, under `[0.1.0]` as a "Pass 13" subsection; if it has been
  tagged, under `[Unreleased]`.
- `docs/passes/13-cross-provider-advice-report.md` (`/pass-report`), with what
  is owed: the **non-empty `/usr/local`** check F-2 also proposed (a
  filesystem fact recorded at capture, `Unknowable` under replay, adapter work),
  `doctor --deep`, and anything A could not answer.
Commit: `docs: record pass 13`.

## 6. Definition of done

- [ ] A: the F-2 addendum exists, dated, with commits and versions; the table of
      what each sentence may claim exists; nothing claims what A left unanswered.
- [ ] B: the data file loads; every row has `source` and `read_on`; the new
      validations refuse bad rows (mutants killed); policy version bumped by
      the rule; goldens differ only in the version.
- [ ] C: the finding exists for **both** directions with different advice;
      the neutrality test passes with swapped data and a third id; `Warning`
      only when the intruder precedes; replay gives `OK` and one `path_unknown`;
      en and pt-BR parity green; no provider name above the adapters.
- [ ] D: SE-098 and the SE-095 addendum; DP-041 marked implemented; schema,
      sample, README, wiki, CHANGELOG, report written; owed items listed.
- [ ] §3 holds: `git diff --stat` on the protected paths is empty; `Cargo.lock`
      unchanged; `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`.
- [ ] `scripts/validate.sh` passes (the commit gate runs it on every commit);
      coverage floors hold or the report says they were not measured.
- [ ] Reviewers ran on the strongest model; every finding fixed or answered in
      writing.

## 7. Not in this pass, and why

- **The non-empty `/usr/local` check** F-2 proposed alongside this one: it is a
  filesystem fact, so it needs an adapter read recorded at capture and
  `Unknowable` under replay (the shape of pass 12's `installations`), and it
  decides nothing the advice needs. Owed, listed in the report.
- **`doctor --deep`** (dynamic-library links across prefixes): a SHOULD, opt-in,
  bounded and interruptible; its own pass.
- **A runtime provider capability for build isolation.** It would replace data
  by detection, but neither provider reports how it isolates builds through a
  public read command, so the facts are about the provider's software and
  belong in dated data. Revisit only if a provider ever reports it.
- **DP-042** (the catalogue sweep) and **the mutating track** (pass 08 slices
  4-6, pass 11): they touch the process allowlist and the executor.
- **Translating the new sentences natively.** They ship machine-drafted under
  DP-040 (a); native review is already owed.
