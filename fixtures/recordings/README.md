# Fixture recordings

Each file here is a [`Recording`](../../crates/adapters/src/process/recording.rs):
the read-only provider and system invocations an adapter makes, with the exact
bytes each one answered, plus provenance. `macportsbrew --replay <file>` answers
every invocation from the recording and nothing runs.

`docs/spec/07-quality-and-testing.md` sets two rules this directory exists to
keep. Fixtures are **captured, not invented**, and every fixture **carries its
provenance**: macOS version, architecture, provider versions, capture date and
capturing command. A fixture without provenance cannot be trusted the day a
provider's output format changes.

Three origins appear in `provenance.origin`, and the difference matters:

| `origin` | What it means |
|---|---|
| `captured` | Recorded from a live machine by `scripts/capture-fixture.sh`. Every byte was observed. |
| `synthesised` | Written from documentation because no machine was available. Its `note` cites the research note behind every shape. |
| `derived` | An **edited capture**. Its `note` names the base fixture and enumerates every edit made to it. |

A derived fixture is not a shortcut around the capture rule; it is a capture
plus a written diff. If the `note` does not say an edit was made, the bytes are
the base fixture's.

## The matrix

`docs/spec/07-quality-and-testing.md` lists the criteria the fixture matrix must
cover. Every column below is a criterion from that list; a ✓ means the criterion
is observable in that fixture's `analyze --json` output in **normal mode**
(no `--advanced`), which is what `fixtures/golden/` records and what
`scripts/run-fixture-matrix.sh` compares.

| Criterion | both-providers-arm64-macos15 | homebrew-only-intel-macos26 | homebrew-deprecated-and-tap-intel-macos26 | macports-only-intel-macos12 | neither-provider-intel-macos13 | macports-broken |
|---|:--:|:--:|:--:|:--:|:--:|:--:|
| Homebrew only | | ✓ | ✓ | | | |
| MacPorts only | | | | ✓ | | |
| Both | ✓ | | | | | |
| Neither | | | | | ✓ | |
| Intel | | ✓ | ✓ | ✓ | ✓ | ✓ |
| Apple Silicon | ✓ | | | | | |
| An older macOS | ✓ (15) | | | ✓ (12) | ✓ (13) | ✓ (12) |
| A current macOS | | ✓ (26) | ✓ (26) | | | |
| Prebuilt artifact available | ✓ | ✓ | ✓ | | | |
| Source build required | ✓ | ✓ | ✓ | ✓ | | |
| Exact mapping | ✓ | ✓ | ✓ | ✓ | | |
| Ambiguous mapping | ✓ | | | | | |
| No mapping | ✓ | ✓ | ✓ | ✓ | | |
| Casks present | ✓ | ✓ | ✓ | | | |
| A third-party tap | | | ✓ | | | |
| A stateful package | ✓ | | | ✓ | | |
| A service package | ✓ | ✓ | ✓ | | | |
| A reverse-dependency blocker | ✓ | ✓ | ✓ | | | |
| A deprecated formula | ✓ | ✓ | ✓ | ✓ | | |
| A broken/partial provider install | | | | | | ✓ |

Two criteria are covered from one side only, and neither gap is silent:

- **A service package** is detected from Homebrew's `has_service` field. The
  MacPorts side reads `${prefix}/etc/LaunchDaemons` on the real filesystem
  (`../../crates/adapters/src/macports/service.rs`), which does not exist under
  replay, so a replayed MacPorts scan is `Unreadable` and no port is ever
  classified `SERVICE`. `macports-only-intel-macos12` holds `nginx` and
  `postgresql17-server` precisely so that this becomes observable the day the
  scan can be replayed.
- **A strong (non-curated) mapping** and **coexistence** appear only in
  `both-providers-arm64-macos15`, because they need a package installed under
  both providers at once.

## The fixtures

### `homebrew-only-intel-macos26.json` — `captured`

Recorded by `scripts/capture-fixture.sh` on an Intel machine running macOS
26.6.2 with Homebrew 6.0.21 and no MacPorts. 180 formulae, 9 casks, one
deprecated formula, every `port` invocation `NOT_STARTED`. This is the only
file here whose bytes were all observed; the other Homebrew-side fixtures are
edits of it, and say so.

### `macports-only-intel-macos12.json` — `synthesised`

Written from `docs/research/R-3-macports-read-interfaces.md` because MacPorts is
installed on no machine available to this project. Its `note` cites the evidence
item behind every line shape. Intel, macOS 12.7.6, MacPorts 2.10.7, Homebrew
absent.

### `macports-broken.json` — `synthesised`

The same machine with `port version` exiting non-zero: the provider is present
but cannot answer. `doctor` exits 5 on it, and it is the only fixture where a
provider reports `BROKEN`.

### `both-providers-arm64-macos15.json` — `derived`

Base: `homebrew-only-intel-macos26.json` for the machine and Homebrew halves,
`macports-only-intel-macos12.json` for the MacPorts half. Edited to an Apple
Silicon machine on macOS 15.6.1 with both providers present. The full edit list
is in the file's `provenance.note`; in summary it moves the machine to arm64,
moves the Homebrew prefix to `/opt/homebrew`, rewrites the bottle tags so that
most formulae have an `arm64_*` variant the machine accepts and eight named ones
keep only their Intel release tag (so they need a source build), adds a
`postgresql@17` record shaped from the captured `httpd` record, and folds four
ports into the MacPorts half:

| Added port | Why it is there |
|---|---|
| `htop` | Shares a normalised name **and** a homepage with the installed Homebrew `htop`: a `STRONG` mapping, and the coexistence case (INV-18). |
| `python312` | The curated exact counterpart of the installed Homebrew `python@3.12`. |
| `nodejs22`, `nodejs24` | Both carry `nodejs.org`; neither normalises to `node`. Homebrew's `node` scores them equally, so the mapping is `AMBIGUOUS` and nothing is selected. |

### `neither-provider-intel-macos13.json` — `derived`

Base: `homebrew-only-intel-macos26.json`. The machine invocations answer for an
Intel Mac on macOS 13.7.6; every `brew` and every `port` invocation carries the
`NOT_STARTED` status the capture script records for an executable that is not
there. Neither provider is detected and there is nothing to analyse — which is a
result, not a failure, and the CLI still exits 0 with a schema-1 document.

### `homebrew-deprecated-and-tap-intel-macos26.json` — `derived`

Base: `homebrew-only-intel-macos26.json`. It exists because `jq` over the
capture's `brew info --json=v2 --installed` document finds one `deprecated: true`
formula, **no** `disabled: true` formula and **no** formula outside
`homebrew/core`. Three fields in that one document were edited — `bvi` becomes
disabled, `codeburn` moves to a non-core tap — and nothing else in the recording
was touched.

## Running the matrix

```sh
cargo build
scripts/run-fixture-matrix.sh            # assert
scripts/run-fixture-matrix.sh --update   # regenerate fixtures/golden/
```

The script runs `status`, `analyze`, `list`, `doctor`, `explain` and `plan`
against every fixture, asserts the exit codes, asserts each output is JSON at
`schemaVersion: 1`, compares the analysis against `fixtures/golden/`, and checks
that the snapshot round trip (`snapshot --out` then `analyze --from-snapshot`)
produces the same analysis. `crates/cli/tests/end_to_end.rs` asserts the same
matrix from inside `cargo test`.

One field is normalised before any comparison, by name and by itself:
`--replay` re-captures a snapshot on every run and a capture stamps itself with
the wall clock, so `.snapshotProvenance.capturedAt` differs between two runs of
the same command. It is the only such field. INV-15 itself is asserted
unnormalised on the `analyze --from-snapshot` path the invariant names.

## Adding a fixture

Capture, do not invent:

```sh
scripts/capture-fixture.sh <fixture-name>
scripts/run-fixture-matrix.sh --update
```

If you must derive one from an existing recording, set
`provenance.origin` to `derived` and write **every** edit into
`provenance.note`. An undocumented edit turns a fixture into an assertion about
a machine that does not exist.
