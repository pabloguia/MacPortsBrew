---
paths:
  - "locales/**"
---

# Locales — parity in both directions

Read `docs/spec/06-cli-and-ux.md` ("Internationalisation") before editing.

## English is the source; every other catalogue has exactly its keys

`locales/en.toml` is canonical and is compiled into the binary, so the tool can
explain a failure before it has read anything from disk. Every other catalogue
is a translation of **exactly** its keys.

`scripts/check-i18n-parity.sh` fails on a **missing** key and on an **orphaned**
one. Both directions. Adding a key to a translation that English does not have
breaks the build just as surely as omitting one — if a locale needs a
distinction English does not make, the English catalogue gets the new key first.

The parity script also checks the other edge: every message identifier the code
references must exist in English. A `Reason` or an `ErrorId` without a template
is a build failure, not a runtime surprise.

## Keys and placeholders are contract; only values are translated

Keys are dotted, lower-case and stable. Renaming one is a compatibility change.
Placeholder names — `{provider}`, `{count}`, `{readOn}`, `{source}` — are part
of the key's contract and are **never** translated. A literal brace is `{{` or
`}}`.

## Never assemble a sentence from fragments

Use the ICU-style plural select:

```
{count, plural, =0 {none at all} one {# package} other {# packages}}
```

`#` renders the value; an `=N` arm matches that exact value and wins over the
category arms. The `one`/`other` split comes from the locale's plural rule:
Japanese and Chinese have no singular and write a single `other` arm; English
and Spanish take the singular only at 1; Brazilian Portuguese takes it at 0 and
1, so "0 pacote" is correct and "0 pacotes" is not. If a locale's rule is not in
`Locale::plural_category`, add it there with a comment saying what the rule is.

Concatenating a stem and an "s" is wrong in most of the target locales and
unfixable in some.

## Never localise machine-readable output

`--json` payloads, log lines, history records, error identifiers, reason
identifiers, rule identifiers and provider identifiers are **always English and
stable**, regardless of `--language`. The locale affects presentation only.

## Layout measures display width

Japanese and Chinese are double-width. Column arithmetic done in `char` counts
produces ragged tables for half the target locales, so everything goes through
`crates/cli/src/render/width.rs`. Check your work with the pseudolocale, which
lengthens and accents every string:

```sh
macportsbrew --language qps analyze
```

A layout that only fits English is visibly wrong there, and a string that
escaped the catalogue is visibly bare. The pseudolocale is generated from
English at run time and has no file of its own — do not create one.

## Error remediation strings are the ones that matter

These are the sentences a user reads when something has gone wrong with their
package manager. A wrong translation is worse than an English one. Do not
machine-translate them and move on.

## Adding a locale

Follow the `add-locale` skill, or the matching section of `CONTRIBUTING.md`;
they are the same steps. Note that `Messages::for_locale` in
`crates/cli/src/i18n/mod.rs` wires `pt-BR` (through `Catalogue::portuguese_brazil`,
the translation laid over English) and the pseudolocale, and returns English for
every other tag, so a translation file on its own is not yet rendered — wiring
the loader is part of the change.
