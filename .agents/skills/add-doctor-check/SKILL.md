<!-- GENERATED FILE - do not edit. Generated from .claude/skills/add-doctor-check/SKILL.md
     by scripts/generate-codex-layer.sh; `.claude/` is the source (DP-018). -->
---
name: add-doctor-check
description: Add a structural check to macportsbrew doctor in crates/application/src/doctor.rs. Use when a machine condition should be reported before it causes a failure — PATH order, prefix problems, leaked build variables, provider health — and whenever a support state or environment fact deserves a warning the user can act on.
argument-hint: "[what the check should detect]"
allowed-tools: Read, Grep, Glob, Edit, Bash(cargo test:*), Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(${CLAUDE_PROJECT_DIR}/scripts/validate.sh*), Bash(${CLAUDE_PROJECT_DIR}/scripts/check-i18n-parity.sh), Bash(${CLAUDE_PROJECT_DIR}/target/debug/macportsbrew*)
license: Apache-2.0
---

# Add a doctor check

Check: $ARGUMENTS

This procedure and the "Adding a doctor check" section of `CONTRIBUTING.md`
are the same steps. If you change one, change the other.

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

## Refusals

- **A finding is not a recommendation.** `doctor` reports structural facts
  about the machine. If the output is "you should move this package", it
  belongs in the policy engine as a rule, not here.
- **Severity is not decoration.** `ERROR` means the user's setup is broken in a
  way that will produce wrong answers. A dated provider policy — a support
  tier, an announced end date — is a `WARNING` carrying its source and read
  date, never an `ERROR`.

## Verification commands

```sh
scripts/check-i18n-parity.sh
cargo test -p macportsbrew-application
target/debug/macportsbrew --replay fixtures/recordings/<fixture>.json doctor
target/debug/macportsbrew --replay fixtures/recordings/<fixture>.json doctor --json
scripts/validate.sh
```
