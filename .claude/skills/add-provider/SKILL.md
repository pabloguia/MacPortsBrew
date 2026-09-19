---
name: add-provider
description: Add a third package-manager provider as an adapter under crates/adapters/src/. Use when integrating pkgsrc, Nix, Fink or any other macOS package manager, and whenever a change would otherwise hardcode the assumption that exactly two providers exist.
argument-hint: "[provider id, e.g. pkgsrc]"
allowed-tools: Read, Grep, Glob, Write, Edit, Bash(cargo test:*), Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(${CLAUDE_PROJECT_DIR}/scripts/validate.sh*), Bash(${CLAUDE_PROJECT_DIR}/scripts/check-layering.sh), Bash(${CLAUDE_PROJECT_DIR}/scripts/capture-fixture.sh*), Bash(${CLAUDE_PROJECT_DIR}/target/debug/macportsbrew*)
license: Apache-2.0
---

# Add a provider

Provider: $ARGUMENTS

This procedure and the "Adding a provider" section of `CONTRIBUTING.md` are
the same steps. If you change one, change the other.

1. **Read `docs/spec/02-domain-model.md` ("Provider contract") and
   `docs/spec/04-migration-and-safety.md` ("Process execution").** A provider
   is added by writing an adapter; nothing above the adapter layer changes.
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
6. **Capture a fixture** for the new provider (see the `capture-fixture` skill)
   and add a contract test per parsed command in `crates/adapters/tests/`.
7. **Write no policy rule that names it.** Rules read facts, and provider
   identifiers flow through them as values (INV-17). If the new provider needs
   a rule, that rule must behave identically when the provider identifiers are
   exchanged, and
   `invariant_17_every_rule_individually_follows_the_rename` will check.
8. **Verify:** `scripts/validate.sh`, then
   `macportsbrew --replay fixtures/recordings/<new-fixture>.json status` and
   confirm the provider is detected and reported like the others.

## Symmetry is the acceptance test

INV-17 says migration is bidirectional and symmetric, and adding a third
provider is where an asymmetry would show. After the adapter exists, check
that nothing above `crates/adapters/` mentions the new id: `grep -rn "<id>"
crates/domain crates/policy crates/application crates/cli` should return
nothing but generic plumbing. A hit is a design defect, not a detail.

## Refusals

- **Adding a provider is out of scope for the initial releases** (the charter's
  non-goals). The abstraction must permit it; shipping one is a separate
  decision. If you are here to make the abstraction hold, do that and stop.
- **Never let one provider satisfy another's dependency** (INV-08), and never
  create an intentional cross-provider dependency (INV-07). A third provider
  multiplies the ways to get that wrong.

## Verification commands

```sh
scripts/check-layering.sh
cargo test --workspace
scripts/validate.sh
target/debug/macportsbrew --replay fixtures/recordings/<new-fixture>.json status
grep -rn "<provider-id>" crates/domain crates/policy crates/application crates/cli
```
