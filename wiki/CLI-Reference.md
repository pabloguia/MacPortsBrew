# CLI Reference

`macportsbrew` is the canonical command. **`mpb` is the same program under a
shorter name** — installed alongside, not a wrapper, and behaving identically.
Everything below works under either.

Every command on this page changes nothing in either package manager,
`migrate --yes` included: it is the one command that asks for execution, and
its gate refuses every plan this build produces. Two commands write a file you
asked for — `snapshot` and `doctor --report`. Given a path that already
exists, they replace it only when it is itself a MacPortsBrew document, and
refuse anything else. (Every command also creates and removes one empty file
in your temporary folder, to learn which user it runs as.)
`snapshot` without `--out` writes into MacPortsBrew's state directory,
`~/Library/Application Support/macportsbrew`.

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

The same table as `analyze` — recommendation column included — without the
reasons behind each recommendation that `analyze` prints after it.

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
| `--to <PROVIDER>` | Destination provider: `homebrew` or `macports`. Required; any other value is a usage error (exit 2) |
| `--retain-source` | Keep the source package after migration |

Prints the ordered steps, the risks, and where a human would decide. Exits **4**
when the plan contains a step needing human review — which is a normal outcome,
not an error, and the plan is printed anyway.

In 0.1.0 a package is matched to the destination only if the destination
already has an equivalent installed, or the small curated table names one.
Otherwise the plan stops at *Needs a human decision* with "no equivalent
found" — nothing was found, which does not mean no equivalent exists. See
[Package Mapping](Package-Mapping.md).

### `macportsbrew migrate <PACKAGE> --to <PROVIDER>`

Shows a migration plan; with `--yes`, puts it to the execution gate. **The gate
refuses every plan this build produces, so nothing is changed** — before any
provider command is run.

| Flag | Effect |
|---|---|
| `--to <PROVIDER>` | Destination provider: `homebrew` or `macports`. Required; any other value is a usage error (exit 2) |
| `--retain-source` | Keep the source package after migration |
| `--yes` | Put the plan to the execution gate instead of only showing it. Nothing in this build prompts |
| `--dry-run` | Show the plan and run nothing. Wins over `--yes` |

Without `--yes` it prints the plan as `plan` does. With `--yes` it prints the
plan and the gate's refusals, one line per reason the gate refuses, and exits
**4** — for every plan this build produces. `--yes` with `--from-snapshot`
exits **2** before any output: a snapshot file records nothing an execution
could check.

When the package is already where you asked for it and no other provider holds
it, `migrate` says there is nothing to move and exits **0**. On a machine where
the other provider was not detected, it says so in the same answer: a provider
installed somewhere detection does not look would read the same, so the answer
is as far as detection can see, and `--json` lists the provider in
`notDetected`. The `--json` contract is
[`../docs/schemas/migrate.v1.md`](../docs/schemas/migrate.v1.md).

### `macportsbrew doctor`

Structural checks: provider presence and health, prefixes, `PATH` order,
duplicate `PATH` entries, leaked build variables, a package manager installed
in more than one place (every standard prefix is searched; an unknown is
reported as unknown, never as one), Command Line Tools, support state with its
source and read date.

| Flag | Effect |
|---|---|
| `--report <FILE>` | Write a diagnostic file for a bug report, without your user name or home folder path. `PATH` entries a finding mentions appear with your home folder shown as `~`; other environment variables are named but their values are left out. It also lists every installed package. Replaces an existing file only if it is a MacPortsBrew document. Read it before you share it |
| `--deep` | Inspect binaries for cross-prefix links. **Not in this build** |

### `macportsbrew snapshot`

Captures both providers' state to a file.

| Flag | Effect |
|---|---|
| `--out <FILE>` | Where to write; defaults to the state directory, `~/Library/Application Support/macportsbrew`. Replaces an existing file only if it is a MacPortsBrew document |

The file leaves out your user name and home folder path, but it carries your
`PATH` (with your home folder shown as `~`) and the full list of installed
packages. Read it before you share it.

Read it back with `--from-snapshot`. The format is documented at
[`../docs/schemas/snapshot.v1.md`](../docs/schemas/snapshot.v1.md).

## Global flags

| Flag | Effect |
|---|---|
| `--json` | Machine-readable output with a versioned schema, **on stdout only** |
| `--language <TAG>` | Locale for human output. `en`, `pt-BR` (Brazilian Portuguese), or `qps` for the pseudolocale; a bare `pt` (or `pt-PT`, `pt_PT.UTF-8`) also gets `pt-BR`, the only Portuguese catalogue; an unknown tag is ignored and the environment is tried next, then English. Without the flag the language comes from `MACPORTSBREW_LANG`, then `LC_ALL`, then `LANG` (`LC_ALL=C` means English); `--json` is always English |
| `--no-color` | Plain output without colour |
| `--verbose` | More detail on stderr |
| `--advanced` | Include dependency-level packages, which are managed automatically by their provider |
| `--from-snapshot <FILE>` | Analyse a captured snapshot instead of this machine |
| `--no-network` | Accepted; this build makes no network call of its own, and the flag does not stop a Homebrew read fetching its catalogue on an empty cache |
| `--refresh` | Accepted; there is no cache in this build |
| `--dry-run` | Show what would be done and run nothing; wins over `--yes`. Every command but `migrate --yes` is a dry run in this build anyway |
| `--yes` | Confirm: `migrate` puts its plan to the execution gate instead of only showing it. The gate refuses every plan this build produces, so nothing runs; nothing in this build prompts |
| `--help`, `-h` | Help. `--help` is the long form, `-h` the summary |
| `--version`, `-V` | Version |

`--no-network` and `--refresh` exist so that a script written today keeps
working when the network and the cache arrive; they are inert, and the help
text says so. `--yes` and `--dry-run` are not inert: they decide whether
`migrate` only shows its plan or puts it to the execution gate — which, in this
build, refuses it.

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
`migrate --yes` exits 4 for every plan this build produces, because the
execution gate refuses each one.

## Not in this build

Specified and not yet written — these commands do not exist in 0.1.0, and
running one prints an "unrecognized subcommand" error:

```
macportsbrew                          the interactive overview (bare `macportsbrew` runs `status` in this build)
macportsbrew restore --run <id>       plan, review, confirm, execute
macportsbrew primary <pkg> <provider> package-level selection
macportsbrew provider-priority <p>    global priority
macportsbrew providers                detect or install a provider
macportsbrew env [--revert]           the managed shell block
macportsbrew history [show <id>]      runs, outcomes, restorability
macportsbrew cleanup
```

Also absent: translations other than `pt-BR` (`es`, `ja` and `zh-CN` are not
written), the cache, and `doctor --deep`.

## Examples

```sh
# What is on this machine
macportsbrew status

# Everything, with the reasoning summarised
macportsbrew analyze

# Just one package, with the full derivation
macportsbrew explain git

# What moving it would involve, in either direction (stops for a human
# decision unless the destination already has git installed)
macportsbrew plan git --to macports
macportsbrew plan git --to homebrew

# The same plan, put to the execution gate (refused in this build, exit 4)
macportsbrew migrate git --to macports --yes

# Machine-readable, for a script
macportsbrew analyze --json | jq '.summary'

# Capture now, analyse later or elsewhere (state.json must not exist yet)
macportsbrew snapshot --out state.json
macportsbrew analyze --from-snapshot state.json

# Check the layout in a locale you do not read
macportsbrew --language qps analyze
```
