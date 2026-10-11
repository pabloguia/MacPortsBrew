# Fixture snapshots

Each file here is a snapshot the binary's own `snapshot` command wrote on a live
machine, the same document `snapshot --out` writes for a user, committed so the
tests can analyse and plan against it with `--from-snapshot`. Like the
recordings in [`../recordings/`](../recordings/README.md), these are **captured,
not invented**, and each one carries the provenance
`docs/spec/07-quality-and-testing.md` requires.

A snapshot exists here for one reason: a `--replay` with MacPorts as a plan's
source is refused by design (DP-016(c): a recording cannot answer what is on the
recording machine's disk), so the only input that can show a MacPorts→Homebrew
plan end to end is a capture taken on the machine itself (pass 08 slice 3,
D-F1; SE-092).

## both-providers-intel-macos26-live.json

| Provenance | Value |
|---|---|
| macOS | 26.7.1 |
| Architecture | x86_64 (native) |
| Provider versions | Homebrew 7.0.4; MacPorts 2.12.6 |
| Capture date | 2026-10-11T04:09:13Z |
| Capturing command | `macportsbrew snapshot --out <file>`, built from commit `655f2be` |
| `provenance.origin` | `captured` (unedited) |
| SHA-256 | `31a5c163b9d38aebe2f179f27ac320931f80f26d7e1edb1d2fe4fb00256a3f36` |

Used by `invariant_17_a_macports_to_homebrew_plan_is_built_from_a_live_capture`
(`crates/cli/tests/migrate.rs`), which plans `htop` in both directions and
asserts the two plans mirror each other.

## What a snapshot here may and may not carry

The capture redacts the home directory to `~`, records which provider
configuration files exist without any key's value, and leaves the environment's
`reported` values out. `crates/cli/tests/fixture_privacy.rs` fails if any file
under `fixtures/` carries a home path with an account name, the name of the
account running the tests, a `reported` value or a configuration key list.

The `PATH` entries and the package inventory are deliberately public: they are
what the analysis reads, and the committed recordings captured on the same
machine already carry them.

To replace a snapshot, capture a new one with the binary, check it with the
privacy test, and update the table above — never edit the bytes by hand.
