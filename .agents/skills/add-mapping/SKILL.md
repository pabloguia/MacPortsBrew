<!-- GENERATED FILE - do not edit. Generated from .claude/skills/add-mapping/SKILL.md
     by scripts/generate-codex-layer.sh; `.claude/` is the source (DP-018). -->
---
name: add-mapping
description: Add a curated package equivalence to data/policy/curated-mappings.toml. Use when the mapping heuristics miss an equivalence between a Homebrew formula and a MacPorts port, when explain shows no candidate or a wrong one, and whenever a package name needs a human decision with a citation rather than a name match.
argument-hint: "[source package name]"
allowed-tools: Read, Grep, Glob, Edit, Bash(cargo test:*), Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(${CLAUDE_PROJECT_DIR}/scripts/validate.sh*), Bash(${CLAUDE_PROJECT_DIR}/target/debug/macportsbrew*), WebFetch(domain:formulae.brew.sh), WebFetch(domain:ports.macports.org), WebFetch(domain:docs.brew.sh), WebFetch(domain:trac.macports.org), WebFetch(domain:guide.macports.org)
license: Apache-2.0
---

# Add a package mapping

Package: $ARGUMENTS

This procedure and the "Adding a package mapping" section of
`CONTRIBUTING.md` are the same steps. If you change one, change the other.

1. **Confirm the heuristics do not already reach it.** Run
   `macportsbrew explain <package>`; the "Mappings and their evidence" section
   prints the candidates and the evidence behind each. A curated row exists
   only for what the name-and-evidence heuristics of
   `docs/spec/02-domain-model.md` demonstrably miss.
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

## Refusals

- **Do not guess a destination name.** If you cannot read it from the
  destination provider's index today, drop the candidate rather than write an
  unconfirmed row. `NONE` is a first-class mapping result and produces
  `MANUAL_REVIEW` with the candidate list, which is the correct outcome for
  something nobody has confirmed.
- **Do not build a large mapping database.** Curate only what the heuristics
  demonstrably miss, and record why each entry exists so it can be retired
  when the heuristics improve.

## Verification commands

```sh
scripts/validate.sh
target/debug/macportsbrew explain <package>
```
