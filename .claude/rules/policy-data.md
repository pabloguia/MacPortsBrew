---
paths:
  - "data/policy/**"
---

# Policy data — every entry carries `source` and `read_on`

This directory is INV-20 made concrete: everything MacPortsBrew believes about
Homebrew or MacPorts lives here as **dated data**, never in control flow. Read
`docs/spec/03-policy-and-recommendations.md` ("Policy data") before editing.

## The rule, in one line

**No row exists without `source` and `read_on`.** Both are mandatory, on every
entry, in every file. A row without them is not a shortcut to be filled in
later; it is a claim from memory, which is the failure mode this whole project
is designed against.

- `source` — precise enough that someone else can re-read exactly what you
  read. A URL, a manual page plus the tool version, a file path plus a commit,
  an API endpoint. "Homebrew docs" is not a source.
- `read_on` — `YYYY-MM-DD`, the day *you* read it. Not the day the upstream page
  was written, and not the day the entry was drafted.

## Runtime detection beats this file

Wherever a fact can be detected at run time, detection wins and this directory
is the fallback and the explanation. Do not add a row that duplicates something
the adapter can observe; add one for what it cannot.

## What lives here

| File | Holds |
|---|---|
| `policy.toml` | The manifest: `version` (the policy version every recommendation carries) and `stale_after_days` |
| `provider-support.toml` | Support tier, published end date, prebuilt-artifact availability, per architecture and macOS range |
| `curated-mappings.toml` | Package equivalences the heuristics miss |
| `provider-commands.toml` | Provider verbs and per-operation elevation |
| `bottle-tags.toml` | Which prebuilt-artifact tags satisfy a machine |
| `runtime-managers.toml` | Runtime families and the managers to suggest |
| `stateful-packages.toml` | Families that own persistent data outside their prefix |

## Bumping the version

`version` is `<newest read_on across this directory>-<serial within that day>`.
Bump the serial for any edit that does not move a read date. Every
`Recommendation` and every `--json` payload carries it, so an answer is always
attributable to a specific set of facts.

Changing what the data says about a provider is a **policy** change and needs
the version bump; it does not need a tool version bump, because the tool
changing and what it believes changing are different events.

## Curated mappings have their own admission rule

**A row exists only when the destination name was read from the destination
provider's own index on `read_on`.** Candidates that could not be confirmed are
dropped, not guessed — `Confidence::None` is a first-class result that produces
`MANUAL_REVIEW` with the candidate list, which is the correct outcome for
something nobody has verified.

Write each row in **one direction**, because a citation has a direction. The
loader builds the reverse index from the same rows, so both directions are
answerable and neither provider is a special case (INV-17). One source name may
legitimately appear more than once: a package one provider ships whole and the
other splits is a one-to-many mapping, and collapsing it loses the half that
carries the service or the data.

Every curated row also carries a `rationale` saying why a heuristic does not get
there. That is what lets the row be retired when the heuristics improve, instead
of the table growing forever.

## Do not curate a large database

Curate only what the heuristics demonstrably miss. If you find yourself adding
rows in bulk, the heuristics are what should change.
