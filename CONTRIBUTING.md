# Contributing to MacPortsBrew

Thank you for looking. This file is the human contributor's entry point;
[`AGENTS.md`](AGENTS.md) is the same orientation for coding agents, and every
recipe below also exists as a skill under
[`.claude/skills/`](.claude/README.md), written from this file so that a person
and an agent follow identical steps.

Two things to know before anything else.

**Homebrew and MacPorts are peers.** Neither is globally better, and no
contribution may encode that one is. Recommendations are a function of
`machine × package × provider × current support state`, and the two migration
directions share one planner and one policy engine (INV-17).

**Every claim about provider behaviour carries its source and the date you read
it, or is phrased conditionally, or is runtime capability detection.** There is
no fourth option, and "I remember that…" is not one of the three. Observations
go in [`docs/research/`](docs/research/README.md); dated facts go in
`data/policy/*.toml` with `source` and `read_on`, never into control flow
(INV-20).

## Prerequisites

- **`rustup`.** The toolchain is pinned in
  [`rust-toolchain.toml`](rust-toolchain.toml) — 1.98.0 with `rustfmt` and
  `clippy`, targeting both `x86_64-apple-darwin` and `aarch64-apple-darwin`.
  `rustup` installs it automatically on the first `cargo` invocation. **Do not
  switch toolchains.** The pin is what holds the macOS deployment floor of
  10.12 on Intel and 11.0 on Apple Silicon
  ([ADR-0002](docs/adr/0002-implementation-language.md)); raising it needs the
  deployment-target check to stay green and, if the floor moves, an ADR.
- **`jq` and `python3`** — required by `scripts/capture-fixture.sh` and by the
  validation scripts.
- **`shellcheck`** — for the hook and script checks.
- **`cargo-llvm-cov`** — for `scripts/check-coverage.sh`, which enforces the
  floors in [`docs/spec/07-quality-and-testing.md`](docs/spec/07-quality-and-testing.md).
  CI installs it from a pinned action; locally it is one command:

  ```sh
  cargo install cargo-llvm-cov --locked
  ```

  An agent session cannot run that — it writes into `~/.cargo/bin`, which is
  the developer's environment rather than this repository, and the guard says
  so. That is why the tool is listed here rather than installed on demand
  (DP-015).
- **macOS**, for anything that reads a real provider. The domain and policy
  crates are pure and build anywhere.

You do **not** need Homebrew or MacPorts installed to work on MacPortsBrew.
Tests run against captured fixtures and never touch a live installation.

## Build, test, format

```sh
cargo build                     # debug binary at target/debug/macportsbrew
cargo test --workspace          # the whole suite
cargo fmt --all                 # format
cargo clippy --all-targets --all-features --workspace -- -D warnings
```

## Validation

[`scripts/validate.sh`](scripts/validate.sh) is the one implementation of
"valid". CI is configured to run it and will be the authority; you run it before
opening a pull request; the agent `Stop` gate runs it at the end of every turn.
(No CI job has run yet — the repository has no remote — so today the local run
and the `Stop` gate are the whole of it.) Do not add a
check anywhere else.

| Invocation | What it runs |
|---|---|
| `scripts/validate.sh --quick` | every static check; the list lives in that script's header and is not repeated here, because this copy drifted by five stages once already |
| `scripts/validate.sh` | the above plus `cargo test --workspace` |
| `scripts/validate.sh --full` | the above plus the release build for both architectures with the deployment-target assertion (macOS only; needs `otool`) |

**A second harness is generated from the first.** `.codex/` and
`.agents/skills/` are produced from `.claude/` by
`scripts/generate-codex-layer.sh` (DP-018). Edit `.claude/`; a `validate.sh`
stage fails when the copies drift, and the path guard refuses a hand-edit of a
generated file. That is why an edit under `.agents/skills/` is blocked and the
message names the source.

## Architecture in one screen

One crate per layer, under `crates/`. The dependency edges between crates
*are* the layering rule — `domain` cannot import an adapter without someone
editing a `Cargo.toml` — and `scripts/check-layering.sh` asserts the edges from
`cargo metadata`.

| Crate | Depends on | Holds |
|---|---|---|
| `domain` | `serde` only | Provider-neutral types and pure rules. No I/O, no clock, no environment, no provider name in any signature. |
| `policy` | `domain` | The rule engine and the versioned data it reads. Pure: `recommend(facts, data)`. |
| `application` | `domain`, `policy` | Capture, analysis, classification, mapping, planning, `doctor`. I/O only through traits the domain declares. |
| `adapters` | `domain` | The only layer that touches the machine. Provider names appear here and nowhere above. |
| `cli` | all of the above | Presentation: the i18n catalogue, the renderer, the command surface. |

Two boundaries matter more than the rest:

- **The executor is the only place a subprocess is spawned.**
  `crates/adapters/src/process/` constructs `std::process::Command`, and
  `scripts/check-layering.sh` fails the build if anything else does. Argv
  arrays only — no shell, no string interpolation, ever. Package names and
  paths are arguments, never fragments of a command line.
- **Capture and analyze are separate.** `capture` talks to providers through
  the domain's read contract and produces a `Snapshot`; `analyze` is a pure
  function of a snapshot and the policy data. `analyze --from-snapshot`
  therefore runs the very same function as a live analysis, which is how
  reproducibility (INV-15) is proven rather than asserted. The same split is
  what makes `--replay` possible: a recorded `ProcessRunner` substitutes for
  the real one, and every layer above is unchanged.

## Licensing of contributions

MacPortsBrew is licensed under the **Apache License 2.0**. Under Apache-2.0
§5, any contribution you intentionally submit for inclusion is licensed to the
project under those same terms, unless you explicitly state otherwise.
Inbound equals outbound, and **no contributor licence agreement is required**.
You do not need to sign anything.

Every source file carries `SPDX-License-Identifier: Apache-2.0` as its first
comment line; `scripts/check-spdx.sh` fails the build on a file without one.
There is deliberately no per-file copyright line and no `NOTICE` file
([ADR-0001](docs/adr/0001-licence-apache-2-0.md)).

Dependency licences are enforced mechanically by `scripts/check-licences.sh`.
Allowed: Apache-2.0, MIT, BSD-2/3-Clause, ISC, Unlicense, Zlib, and MPL-2.0
used unmodified as a separate component. **GPLv2-only is forbidden** — it is
incompatible with Apache-2.0. Anything else needs an ADR before it is added. A
dependency that raises the macOS deployment floor is rejected the same way.

## Definition of done

A change is done when it is implemented; tests exist and pass;
`scripts/validate.sh` passes; every invariant the change touches still has a
passing test named after it (`invariant_NN_*`); i18n keys are consistent;
documentation and `wiki/` are accurate for any user-visible change; no
cross-provider contamination was introduced; and
[`docs/spec-evolution.md`](docs/spec-evolution.md) records any material
specification change, written when the change was made rather than
reconstructed afterwards.

Commits are [Conventional Commits](https://www.conventionalcommits.org), one
coherent change each, in English. A reviewer must be able to read the history
and follow the reasoning.

---

# Recipes

## Adding a package mapping

1. **Confirm the heuristics do not already reach it.** Run
   `macportsbrew explain <package>`; the "Mappings and their evidence" section
   prints the candidates and the evidence behind each. A curated row exists
   only for what the name-and-evidence heuristics of
   [`docs/spec/02-domain-model.md`](docs/spec/02-domain-model.md) demonstrably
   miss.
2. **Read the destination name from the destination provider's own index,
   today.** For MacPorts, `https://ports.macports.org/api/v1/ports/<name>/`;
   for Homebrew, the formula file in a `homebrew-core` checkout or
   `https://formulae.brew.sh/api/formula/<name>.json`. A row whose destination
   name was not read from the destination provider is not admissible — that is
   the standing admission rule for the file.

   **Two further admission rules, both mechanical, and both applying to the
   source name as much as to the destination** — `index_equivalences` builds the
   reverse index from the same rows, so every source name is a destination name
   read from the other side.

   - **The name must be one `is_safe_provider_name` accepts**: no leading dash,
     no path separator, nothing a provider would read as a flag or a path.
     `check_curated_names` refuses the whole file at load with
     `PolicyDataInvalid`, so a row that breaks this is a *build* defect failing
     every command, not a surprise inside one plan. It is refused there rather
     than in the planner because the planner's identical guard answered
     `InputUnusable`, which sends the user to re-capture a snapshot over a
     defect compiled into the binary (F-48).
   - **The name must not be a word the provider reads as a set of ports** rather
     than as one package — the `reserved_names` in
     `crates/application/src/provider_commands.rs`. Unlike the rule above this
     one is **not** checked at load: it refuses in the planner, with `Usage`,
     and only when somebody plans that particular package. It is therefore the
     one to check by hand while writing the row, because nothing will tell you
     until a user asks for that plan.
3. **Add one `[[entry]]` to `data/policy/curated-mappings.toml`** with every
   field: `source_provider`, `source_name`, `destination_provider`,
   `destination_name`, `confidence = "EXACT"` (a curated row is a human
   decision with a citation, which is the top of the scale), `source` —
   precise enough that someone can re-read exactly what you read — `read_on`
   in `YYYY-MM-DD`, and `rationale` explaining why a heuristic does not get
   here.
4. **Write it in one direction only.** A citation has a direction: you read the
   destination name from that provider on that date. The loader builds the
   reverse index from the same rows, so both directions are answerable and
   neither provider is a special case (INV-17). One source name may appear more
   than once — a package that one provider ships whole and the other splits is
   a one-to-many mapping, and collapsing it loses the half that carries the
   service or the data.
5. **Add a test** in `crates/policy/src/data.rs` if the row exercises something
   the existing rows do not: a one-to-many mapping, a versioned family, or a
   name the heuristics would otherwise mis-normalise. The cross-cutting
   property — that curated data answers in both directions — is already
   covered by `invariant_17_curated_data_answers_in_both_directions` in
   `crates/policy/tests/invariants.rs`.
6. **Verify:** `scripts/validate.sh`, then
   `macportsbrew explain <package>` and confirm the mapping now appears with
   your evidence.

## Adding a policy rule

1. **Read [`docs/spec/03-policy-and-recommendations.md`](docs/spec/03-policy-and-recommendations.md)
   and INV-14, INV-15, INV-17 and INV-19.** A rule is a function of
   `(facts, data)`: no I/O, no clock — the current date is `facts.today` — and
   no provider name written down anywhere.
2. **Create `crates/policy/src/rules/<snake_case_id>.rs`**, declare the module
   in `crates/policy/src/rules/mod.rs`, and add the rule to `all()`. That
   vector is sorted by identifier afterwards, so its position in the literal
   does not affect behaviour.
3. **Implement `Rule`.** `id()` is stable `SCREAMING_SNAKE_CASE` and appears in
   `explain` output and in every reason the rule emits, so changing one is a
   compatibility change. `since()` is the policy version it arrives in.
   `source()` names a specification section, an invariant or a research note —
   a rule with no source is an opinion. `reviewed_on()` is when you last
   checked the reasoning against that source. `evaluate()` returns `None` when
   the rule does not fire, which is a first-class answer that `explain` reports.
4. **Take `priority()` from a constant in `crates/policy/src/rules/mod.rs`,
   never a bare number:** `PRIORITY_MANUAL_REVIEW` (1000), `PRIORITY_RUNTIME`
   (950), `PRIORITY_DESTINATION_VETO` (900), `PRIORITY_SUPPORT_ENDING` (700),
   `PRIORITY_ARTIFACT_ADVANTAGE` (600), `PRIORITY_COEXISTENCE` (500),
   `PRIORITY_NOTHING_TO_DO` (200), `PRIORITY_FALLBACK` (100). Ties break by
   identifier, so the answer never depends on declaration order (INV-15).
   Nothing can outvote `MANUAL_REVIEW`, and the engine enforces that by
   outcome rather than by number (INV-19).
5. **Add the reason identifier in two places or the build fails:** the constant
   in `crates/policy/src/reasons.rs`, with its parameters listed in the doc
   comment, and the English text in `locales/en.toml`.
   `scripts/check-i18n-parity.sh` and `crates/cli/tests/reasons_catalogue.rs`
   assert that the catalogue covers everything the engine can emit. Adding an
   identifier is a compatibility change.
6. **Write the rule's tests in the same file:** the case that fires, the cases
   that do not, and the tie-break if the rule can collide with another.
7. **Check the cross-rule invariants still hold.**
   `cargo test -p macportsbrew-policy` runs
   `invariant_17_every_rule_individually_follows_the_rename`,
   `invariant_15_reproducible_recommendations` and
   `invariant_19_manual_review_cannot_be_outvoted` across every rule at once.
8. **Bump `version` in `data/policy/policy.toml`** if the rule changes an
   existing recommendation, and record the change in
   [`docs/spec-evolution.md`](docs/spec-evolution.md).
9. **Verify:** `scripts/validate.sh`, then `macportsbrew explain <package>` and
   confirm your rule appears under "Rules that fired" or "Rules considered that
   did not fire".

## Adding a locale

1. **Copy `locales/en.toml` to `locales/<tag>.toml`**, where `<tag>` is the
   BCP-47 tag — the targets are `pt-BR`, `es`, `ja` and `zh-CN`, listed in
   `crates/cli/src/i18n/locale.rs` as `KNOWN`. English is the canonical source
   locale; every other catalogue is a translation of exactly its keys.
2. **Translate the values only.** Keys, placeholder names (`{provider}`,
   `{count}`, `{readOn}`) and the identifiers inside them are part of the
   contract and are never translated.
3. **Use the ICU-style plural select for anything countable** rather than
   concatenating fragments:
   `{count, plural, =0 {none at all} one {# package} other {# packages}}`.
   `#` renders the value; an `=N` arm wins over the category arms. `one` and
   `other` come from the locale's plural rule — Japanese and Chinese write a
   single `other` arm; English and Spanish take the singular only at 1;
   Brazilian Portuguese takes it at 0 and 1. If your locale's rule is not
   already in `Locale::plural_category`, add it there with a comment saying
   what the rule is.
4. **Do not translate machine-readable output**, log lines, history records,
   error identifiers or reason identifiers. Locale affects presentation only;
   `--json` is always English and stable.
5. **Wire the catalogue up** in `Catalogue::for_locale`
   (`crates/cli/src/i18n/mod.rs`). In this build that function returns English
   for every tag except the pseudolocale, so a translation file on its own is
   not yet rendered — the loader is part of your change.
6. **Check parity and layout.** `scripts/check-i18n-parity.sh` fails on a
   missing or an orphaned key. Then run
   `macportsbrew --language qps analyze`: the pseudolocale lengthens and
   accents every string, so a layout that only fits English is visibly wrong.
   Japanese and Chinese are double-width, and all column arithmetic must go
   through `crates/cli/src/render/width.rs` — never `char` counts.
7. **Verify:** `scripts/validate.sh` and `cargo test -p macportsbrew`.

## Adding a doctor check

1. **Add the finding in `crates/application/src/doctor.rs`.** A
   `DoctorFinding` is `{ id, severity, reason, provider? }`: a stable
   lower-case `id`, a `Severity` of `OK`, `WARNING` or `ERROR`, and a `Reason`
   carrying an identifier plus named parameters.
2. **Name the reason `doctor.<something>`** and add its English text to
   `locales/en.toml`, with a `{placeholder}` for every parameter the finding
   sets. The finding carries no display text of its own.
3. **Read only the snapshot and the policy data.** `doctor` must produce the
   same findings from `--from-snapshot` and under `--replay` as it does live;
   a check that reaches for the live machine breaks that and cannot be tested
   from a fixture.
4. **Respect the redaction rules.** `doctor --report` may name versions,
   architecture, macOS version, provider prefixes, package names and findings.
   It must never emit usernames, home paths, environment variable values or
   tokens. A parameter that can carry a path is redacted the same way the rest
   of the report is — the home directory becomes `~`.
5. **Add a test in `crates/application/tests/doctor_findings.rs`** covering the
   finding present and the finding absent. Not "in the same file", which this
   step used to say: `doctor.rs` has no `#[cfg(test)]` module and never had one,
   and every doctor finding is tested there instead. A step that names a place
   that does not exist is one the next contributor has to work around and then
   guess at, which is how the two halves of a paired procedure start to drift.
6. **Verify:** `scripts/check-i18n-parity.sh`, then
   `macportsbrew --replay fixtures/recordings/<fixture>.json doctor` and the
   same with `--json`, on a fixture that exercises the check.

## Capturing a fixture

1. **Run `scripts/capture-fixture.sh <fixture-name>`.** It writes
   `fixtures/recordings/<fixture-name>.json` and needs `jq` and `python3`. Only
   allow-listed read-only commands run, in the same sanitised environment the
   executor builds — no inherited variables, the provider-quieting variables
   set.
2. **Fixtures are captured, never invented.** Do not hand-write provider
   output; a fixture's whole value is that it is real.
3. **Check the provenance the script records:** macOS version, architecture,
   provider versions, capture date and the capturing command. A fixture without
   provenance cannot be trusted when a provider changes its output format.
4. **Check the redaction before committing.** The script rewrites the home
   directory to `~` and the user name to `USER`. Read the file and confirm
   nothing personal survived — a fixture is committed and public.
5. **Replay it:**
   `macportsbrew --replay fixtures/recordings/<fixture-name>.json status` and
   the same with `analyze`. A replayed run never consults the live filesystem;
   `invariant_15_a_replayed_reader_never_consults_the_live_filesystem` asserts
   it.
6. **Assert against it in a contract test** —
   `crates/adapters/tests/homebrew_contract.rs` or `macports_contract.rs` —
   pulling the file in with `include_str!` and asserting on the *parsed shape*,
   never on formatted text. A parser that meets output it does not recognise
   must fail with `ErrorId::ProviderOutputChanged` rather than mis-parse
   silently.
7. **Add its golden file.** `scripts/run-fixture-matrix.sh` runs the whole read
   slice over every recording and compares the analysis byte for byte against
   `fixtures/golden/<fixture-name>.analyze.json`. Generate the new one with
   `scripts/run-fixture-matrix.sh --update`, then **read the diff before
   committing it** — `--update` will just as happily record a regression as a
   new fixture. Without `--update`, any difference is a failure.
8. **Verify:** `scripts/run-fixture-matrix.sh` and `cargo test --workspace`.

## Adding a provider

1. **Read [`docs/spec/02-domain-model.md`](docs/spec/02-domain-model.md)
   ("Provider contract") and
   [`docs/spec/04-migration-and-safety.md`](docs/spec/04-migration-and-safety.md)
   ("Process execution").** A provider is added by writing an adapter; nothing
   above the adapter layer changes.
2. **Create `crates/adapters/src/<provider>/`** implementing
   `macportsbrew_domain::ProviderRead` over a
   `macportsbrew_adapters::process::ProcessRunner`. The runner is the only
   place a subprocess is spawned, and `scripts/check-layering.sh` fails the
   build if the adapter constructs `std::process::Command` itself. That
   indirection is also what lets the same adapter code run against a live
   machine and against a recording.
3. **Declare the capabilities the provider actually has.**
   `CapabilityState::Unknown` is a first-class answer and must never be
   reported as `Available`; the analysis degrades and says so rather than
   guessing.
4. **Add one entry to `crates/adapters/src/registry.rs`:** the provider id in
   `known_provider_ids()`. That file is the only place above the adapter which
   knows a provider exists. Its **candidate prefixes go in
   `data/policy/provider-prefixes.toml`**, in detection order, with a `source`
   and a `read_on` like every other provider fact — INV-20 names prefixes, and
   the INV-20 test refuses one written into source (DP-014).
5. **Add the provider's data with `source` and `read_on` on every entry:** its
   verbs and per-operation elevation in `data/policy/provider-commands.toml`,
   and its support state per architecture and macOS range in
   `data/policy/provider-support.toml`. Both are data (INV-20); no verb and no
   deadline is written into Rust. **Four verbs, not three**: `add`, `remove`,
   `verify` — and `verify_absent`, which is how a removal is certified and whose
   argv must ask for the **whole inventory** with no package name. Observe what
   your provider answers when asked about a package that is *not* installed
   before you write that cell: on both shipped providers the single-name form
   answers "absent", "never existed" and "could not run" identically, which is
   why the inventory form exists. The loader refuses the semantic on a verb that
   takes a name, and `embedded_table_loads_with_provenance` refuses an entry
   that omits the verb.
6. **Capture a fixture** for the new provider (see *Capturing a fixture*) and
   add a contract test per parsed command in `crates/adapters/tests/`.
7. **Write no policy rule that names it.** Rules read facts, and provider
   identifiers flow through them as values (INV-17). If the new provider needs
   a rule, that rule must behave identically when the provider identifiers are
   exchanged, and
   `invariant_17_every_rule_individually_follows_the_rename` will check.
8. **Verify:** `scripts/validate.sh`, then
   `macportsbrew --replay fixtures/recordings/<new-fixture>.json status` and
   confirm the provider is detected and reported like the others.

---

## Reporting problems

Use the [issue templates](.github/ISSUE_TEMPLATE). Each asks for your macOS
version, architecture, provider versions, MacPortsBrew version and policy
version — `macportsbrew doctor --report` collects all of them into one redacted
file. Read it before you attach it. Never include secrets, tokens or anything
outside the two providers' scope.

Security problems do not go in an issue; see [`SECURITY.md`](SECURITY.md).
