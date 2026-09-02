---
name: capture-fixture
description: Capture a real provider recording into fixtures/recordings/ with scripts/capture-fixture.sh, then replay and assert against it. Use before writing any adapter test, whenever a provider's output format may have changed, and whenever a test would otherwise need a live Homebrew or MacPorts installation.
argument-hint: "[fixture name, e.g. homebrew-only-intel-macos26]"
allowed-tools: Read, Grep, Glob, Edit, Bash(${CLAUDE_PROJECT_DIR}/scripts/capture-fixture.sh*), Bash(${CLAUDE_PROJECT_DIR}/scripts/run-fixture-matrix.sh*), Bash(cargo test:*), Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(${CLAUDE_PROJECT_DIR}/scripts/validate.sh*), Bash(${CLAUDE_PROJECT_DIR}/target/debug/macportsbrew*), Bash(jq *)
license: Apache-2.0
---

# Capture a fixture

Fixture: $ARGUMENTS

This procedure and the "Capturing a fixture" section of `CONTRIBUTING.md` are
the same steps. If you change one, change the other.

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

## What the matrix needs

`docs/spec/07-quality-and-testing.md` names the coverage the fixture set owes:
Homebrew only; MacPorts only; both; neither; Intel; Apple Silicon; an older
macOS; a current macOS; prebuilt artifact available; source build required;
exact mapping; ambiguous mapping; no mapping; casks present; a third-party tap;
a stateful package; a service package; a reverse-dependency blocker; a
deprecated formula; a broken or partial provider install. Before capturing,
check what `fixtures/recordings/` already covers and capture the gap.

## Refusals

- **Never run a mutating provider subcommand to produce a fixture.** The
  capture script's allow-list is read-only, and the repository's hooks deny the
  mutating forms. If a fixture would need one, it is not a fixture this project
  can have.
- **Never edit a recording to make a test pass.** A recording is evidence. If
  the parser disagrees with it, the parser is what changes.

## Verification commands

```sh
scripts/capture-fixture.sh <fixture-name>
jq '.provenance' fixtures/recordings/<fixture-name>.json
target/debug/macportsbrew --replay fixtures/recordings/<fixture-name>.json status
target/debug/macportsbrew --replay fixtures/recordings/<fixture-name>.json analyze
scripts/run-fixture-matrix.sh --update   # then read the diff
scripts/run-fixture-matrix.sh
cargo test --workspace
```
