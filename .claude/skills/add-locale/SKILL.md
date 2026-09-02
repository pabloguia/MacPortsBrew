---
name: add-locale
description: Add a translation catalogue under locales/ and wire it into the i18n layer. Use when adding pt-BR, es, ja or zh-CN, when a locale exists but is not rendered, and whenever user-facing text needs to reach a language other than the English source catalogue.
argument-hint: "[BCP-47 tag, e.g. pt-BR]"
allowed-tools: Read, Grep, Glob, Write, Edit, Bash(cargo test:*), Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(${CLAUDE_PROJECT_DIR}/scripts/validate.sh*), Bash(${CLAUDE_PROJECT_DIR}/scripts/check-i18n-parity.sh), Bash(${CLAUDE_PROJECT_DIR}/target/debug/macportsbrew*)
license: Apache-2.0
---

# Add a locale

Locale: $ARGUMENTS

This procedure and the "Adding a locale" section of `CONTRIBUTING.md` are the
same steps. If you change one, change the other.

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

## Refusals

- **Do not machine-translate an error remediation string and move on.** These
  are the sentences a user reads when something has gone wrong with their
  package manager; a wrong one is worse than an English one.
- **Do not add a key that English does not have.** Parity is enforced in both
  directions: a missing key and an orphaned key both fail the build. If the
  locale needs a distinction English does not make, the English catalogue gets
  the new key first.

## Verification commands

```sh
scripts/check-i18n-parity.sh
cargo test -p macportsbrew
scripts/validate.sh
target/debug/macportsbrew --language qps analyze
target/debug/macportsbrew --language <tag> status
```
