# CLI Reference

`macportsbrew` is the canonical command. **`mpb` is the same program under a
shorter name** — installed alongside, not a wrapper, and behaving identically.
Everything below works under either.

Every command on this page is read-only. Nothing here changes your machine.

## Commands

### `macportsbrew status`

The machine and provider dashboard: architecture, macOS version, translation
state, and each provider's presence, version, prefix, support state and package
count. Ends with the policy version the answer was computed under.

### `macportsbrew analyze [PACKAGES]...`

Inventory, mapping, assessment and recommendations — one row per package across
providers. With package names, restricts to those.

Columns: package, provider, version, ownership, availability, class,
recommendation. Rows marked `*` are mapping candidates not installed under that
provider.

### `macportsbrew list`

The inventory without the recommendation machinery.

| Flag | Effect |
|---|---|
| `--provider <PROVIDER>` | Only packages held by this provider |

### `macportsbrew explain <PACKAGE>`

The full derivation of one recommendation: the facts the engine read, the rules
that fired with their outcomes and reasons, the rules that were considered and
**did not** fire, and the mapping evidence. The package name may be in either
provider's spelling.

This is the command to run when you disagree with an answer. It names the rule.

### `macportsbrew plan <PACKAGE> --to <PROVIDER>`

Builds a migration plan. **Nothing is executed in this build.**

| Flag | Effect |
|---|---|
| `--to <PROVIDER>` | Destination provider id. Required |
| `--retain-source` | Keep the source package after migration |

Prints the ordered steps, the risks, and where a human would decide. Exits **4**
when the plan contains a step needing human review — which is a normal outcome,
not an error, and the plan is printed anyway.

### `macportsbrew doctor`

Structural checks: provider presence and health, prefixes, `PATH` order,
duplicate `PATH` entries, leaked build variables, Command Line Tools, support
state with its source and read date.

| Flag | Effect |
|---|---|
| `--report <FILE>` | Write a redacted diagnostic file for a bug report |
| `--deep` | Inspect binaries for cross-prefix links. **Not in this build** |

### `macportsbrew snapshot`

Captures both providers' state to a file.

| Flag | Effect |
|---|---|
| `--out <FILE>` | Where to write; defaults to the state directory |

Read it back with `--from-snapshot`. The format is documented at
[`../docs/schemas/snapshot.v1.md`](../docs/schemas/snapshot.v1.md).

## Global flags

| Flag | Effect |
|---|---|
| `--json` | Machine-readable output with a versioned schema, **on stdout only** |
| `--language <TAG>` | Locale for human output. `en`, or `qps` for the pseudolocale; an unknown tag falls back to English silently |
| `--no-color` | Plain output without colour |
| `--verbose` | More detail on stderr |
| `--advanced` | Include dependency-level packages, which are managed automatically by their provider (INV-13) |
| `--from-snapshot <FILE>` | Analyse a captured snapshot instead of this machine |
| `--no-network` | Accepted; analysis never uses the network in this build |
| `--refresh` | Accepted; there is no cache in this build |
| `--dry-run` | Accepted; everything in this build is a dry run |
| `--yes` | Accepted; nothing in this build prompts |
| `--help`, `-h` | Help. `--help` is the long form, `-h` the summary |
| `--version`, `-V` | Version |

The last four exist so that a script written today keeps working when the
mutating commands arrive. They are not silently ignored — they are honestly
inert, and the help text says so.

`--from-snapshot` computes the analysis **as of the snapshot's capture date**,
so a support deadline is weighed the way it was when the snapshot was taken.

## Output rules

**JSON goes to stdout and nothing else does.** Progress, warnings and prompts
go to stderr, so `macportsbrew analyze --json | jq` always works.

Non-TTY output is plain, unstyled and stable.

**`--json` is a compatibility surface.** Every payload carries a
`schemaVersion`; human output may change freely, JSON may not. The schemas are
in [`../docs/schemas/README.md`](../docs/schemas/README.md).

**Machine-readable output is never localised.** Identifiers, reason ids, rule
ids and error ids are stable English regardless of `--language`; the locale
affects human presentation only.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | generic failure |
| 2 | usage error |
| 3 | partial success |
| 4 | blocked by a safety check |
| 5 | provider unavailable or unhealthy |
| 6 | user aborted |
| 7 | lock held by another instance |
| 8 | stale or unusable input |

Codes 3, 6 and 7 belong to execution and are not reachable in this build.

## Not in this build

Specified and not yet written:

```
macportsbrew                          the interactive overview (bare `macportsbrew` runs `status` in this build)
macportsbrew migrate ...              execute a plan
macportsbrew restore --run <id>       plan, review, confirm, execute
macportsbrew primary <pkg> <provider> package-level selection
macportsbrew provider-priority <p>    global priority
macportsbrew providers                detect or install a provider
macportsbrew env [--revert]           the managed shell block
macportsbrew history [show <id>]      runs, outcomes, restorability
macportsbrew cleanup
```

Also absent: translations beyond English (the architecture and the full English
catalogue are in place), the cache, and `doctor --deep`.

## Examples

```sh
# What is on this machine
macportsbrew status

# Everything, with the reasoning summarised
macportsbrew analyze

# Just one package, with the full derivation
macportsbrew explain git

# What moving it would involve, in either direction
macportsbrew plan git --to macports
macportsbrew plan git --to homebrew

# Machine-readable, for a script
macportsbrew analyze --json | jq '.summary'

# Capture now, analyse later or elsewhere
macportsbrew snapshot --out state.json
macportsbrew analyze --from-snapshot state.json

# Check the layout in a locale you do not read
macportsbrew --language qps analyze
```
