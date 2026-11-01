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
`~/Library/Application Support/macportsbrew` — and only for this machine:
under `--replay` or `--from-snapshot` it needs `--out` (exit 2 otherwise).
With `--dry-run`, neither command writes its file or any state; each says
where it would have written.

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
| `--plan-digest <DIGEST>` | With `--yes`: go on to the execution gate only if `<DIGEST>` is the digest of the plan about to run, as `migrate` printed it. Any other digest stops before the gate (exit **4**, nothing on stdout); a malformed one, or one without `--yes`, is a usage error (exit **2**) |

Without `--yes`, or with `--dry-run`, it prints the plan as `plan` does,
followed by the plan's digest (`sha256:` and 64 lowercase hexadecimal digits;
`planDigest` in `--json`). A
script that wants to run exactly the plan it showed passes that digest back with
`--yes --plan-digest`; a plan that changed in between is refused. The digest is
valid only for the same version of MacPortsBrew, and someone holding it can
test a guess at what the plan contains, so leave it out of anything you
publish. With `--yes` it prints the
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
reported as unknown, never as one), for each pair of providers, in each direction, how
the affected one's own tooling guards its builds (a warning only when the other is
searched first on a known `PATH`; see [Troubleshooting](Troubleshooting.md)), Command Line Tools,
support state with its source and read date.

| Flag | Effect |
|---|---|
| `--report <FILE>` | Write a diagnostic file for a bug report, without your user name or home folder path. `PATH` entries a finding mentions appear with your home folder shown as `~`; other environment variables are named but their values are left out. It also lists every installed package. Replaces an existing file only if it is a MacPortsBrew document. Read it before you share it |
| `--deep` | Inspect binaries for cross-prefix links. **Not in this build** |

### `macportsbrew history`

Lists the runs this MacPortsBrew has made, newest first, and `history show <ID>`
shows one: when it started, the package and direction, how it ended, its steps,
the snapshot it took before the first change, and whether it can be put back.
It reads the state directory and nothing else, and it creates nothing.

| Flag | Effect |
|---|---|
| `--limit <N>` | How many runs to list, 1 to 200. Default 20 |

- **Restore** says one of: nothing to restore (the run changed nothing); a plan
  can be built from the snapshot; partly (a command that changes packages did
  not finish or did not exit 0, so the machine's state after it is not known);
  or no, with the reason (no snapshot was taken, it is gone, it cannot be read
  or it is not this run's, or the record is not confirmed). It never says
  "fully": a reinstall plan is not version-exact.
- **A run in the list shows its snapshot as present from the file's details
  only.** `history show` reads the snapshot and checks that it is the one the
  run took.
- A run started in the same second as another is listed after it or before it
  by an arbitrary but fixed order, not by time.
- `history` refuses under `--from-snapshot` and under a replay, and when run as
  root: the history is this machine's own, now.
- A run id that is not shaped like one is a usage error (exit 2); one that is
  well formed but has no record is exit 8, as is a store or record that cannot
  be read. A record that cannot be read is never skipped.

`--json` documents: [`history-list.v1.md`](../docs/schemas/history-list.v1.md),
[`history-show.v1.md`](../docs/schemas/history-show.v1.md).

#### `macportsbrew history acknowledge <ID>`

Deals with the interrupted-run marker a run left. `doctor` reports an
interrupted run and says which of three things it is: its record exists but is
not confirmed; its record cannot be read; or it began and left no record (the
provider may be half-changed, and the marker with the snapshot it names is the
only trace).

| Flag | Effect |
|---|---|
| `--yes` | Do it. Without it, or with `--dry-run`, the command says what it knows and what it would do and changes nothing |

With `--yes` it takes the run lock (another run holding it: exit 7), refuses a
marker that names a different run (exit 4), makes the run's record durable and
checks it reads back, and only then removes the marker. A record it cannot read
keeps the marker (exit 8). It deletes no record and no snapshot. With no marker
there is nothing to acknowledge (exit 0). `--json`:
[`history-acknowledge.v1.md`](../docs/schemas/history-acknowledge.v1.md).

### `macportsbrew restore --plan --run <ID>`

Builds the plan that puts back what one recorded run changed, from the run's
record and the snapshot it took before its first change. It is an ordinary
migration plan, built by the same planner in the other direction; nothing is
executed in this build.

| Flag | Effect |
|---|---|
| `--plan` | Show the plan and run nothing. Without it the plan is put to the execution gate, which refuses it (exit 4) |
| `--run <ID>` | The run, by the id `history` lists |
| `--snapshot <FILE>` | The additive whole-machine reconcile. **Not in this build** (exit 1) |

- The plan reinstalls the package under the provider the run moved it from,
  verifies it, and removes the package the run added only if the record shows
  the add finished with exit status 0 and the snapshot shows the destination
  did not already hold it.
- Notes before the plan say what it cannot: it is not version-exact, why a
  package is kept, and that a record is not confirmed.
- A record the interrupted-run marker still names is planned for review and is
  never offered for execution.
- A snapshot that is missing, unreadable or not the run's is exit 8. A machine
  already as the run found it is "nothing to restore", exit 0. A plan that
  stops at manual review exits 4, as `plan` does.
- Refused under `--from-snapshot` and a replay, and when run as root.

`--json`: [`restore.v1.md`](../docs/schemas/restore.v1.md).

### `macportsbrew snapshot`

Captures both providers' state to a file.

| Flag | Effect |
|---|---|
| `--out <FILE>` | Where to write; defaults to the state directory, `~/Library/Application Support/macportsbrew`, for a capture of this machine only (required under `--replay` and `--from-snapshot`). Replaces an existing file only if it is a MacPortsBrew document |

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
| `--dry-run` | Show what would be done and change nothing: no file is written either (`snapshot` and `doctor --report` say where they would have written); wins over `--yes`. Nothing in this build changes a provider anyway |
| `--yes` | Confirm: `migrate` puts its plan to the execution gate instead of only showing it. The gate refuses every plan this build produces, so nothing runs; nothing in this build prompts |
| `--help`, `-h` | Help. `--help` is the long form, `-h` the summary |
| `--version`, `-V` | Version. `--version` also prints the policy data version, the target, the commit and, for a build CI numbered, `build <n>`; `-V` prints only the version |

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

Executing a restore (`restore --run <id>` without `--plan`) is built as far as
the execution gate, which refuses it; nothing runs. These commands are specified
and not yet written — they do not exist in 0.1.0, and running one prints an
"unrecognized subcommand" error:

```
macportsbrew                          the interactive overview (bare `macportsbrew` runs `status` in this build)
macportsbrew primary <pkg> <provider> package-level selection
macportsbrew provider-priority <p>    global priority
macportsbrew providers                detect or install a provider
macportsbrew env [--revert]           the managed shell block
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
