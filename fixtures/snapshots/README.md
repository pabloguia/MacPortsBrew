# Fixture snapshots

Each file here is a snapshot the binary's own `snapshot` command wrote on a live
machine, the same document `snapshot --out` writes for a user, committed so the
tests can analyse and plan against it with `--from-snapshot`. Like the
recordings in [`../recordings/`](../recordings/README.md), these are **captured,
not invented**, and each one carries the provenance
`docs/spec/07-quality-and-testing.md` requires.

A snapshot exists here for one reason: a plan from a `--replay` is refused by
design wherever the source row's answer to the service question lives on the
recorded machine's disk (DP-016(c), DP-032). That was every MacPorts row from
the start (pass 08 slice 3, D-F1; SE-092); since work order 16 part D it is also
every Homebrew row that does not declare a service, because a recording cannot
show the keg. So the only input that can show a plan with commands, in either
direction, is a capture taken on the machine itself.

## both-providers-intel-macos26-live.json

| Provenance | Value |
|---|---|
| macOS | 26.7.1 |
| Architecture | x86_64 (native) |
| Provider versions | Homebrew 7.0.4; MacPorts 2.12.6 |
| Capture date | 2026-10-28T09:24:27Z |
| Capturing command | `macportsbrew snapshot --out <file>`, built from commit `<commit>` (the part D implementation commit; its working tree before commit, on top of `5f0db01`, branch `wo16/D-impl`) |
| `provenance.origin` | `captured` (unedited) |
| SHA-256 | `678e38f74bd558e7ebdc16e28b54b151be2ecc709473e01126e4aa312dfbf048` |

**Re-captured 2026-10-28 for part D.** The earlier capture (2026-10-11, commit
`655f2be`) predates the `service` record, and a row without one answers the
service question `UNKNOWN`, so nothing could be planned from it. A second
capture with the same build was byte-identical apart from `capturedAt`. It was
captured again after the implementation review narrowed the `source` rule to a
whole Tcl word; the only change besides `capturedAt` was `xz` and
`xz-bootstrap`, whose Portfiles mention `source` only inside a longer word, going
from `UNKNOWN` to `NO`. Each diff was checked for a home path and the account
name before commit; it carries neither. On that machine the reader answers `NO`
for 15 of 18 ports and `UNKNOWN` for three (`autoconf`, `ncurses`, `tree`),
whose installed Portfiles carry `source` as a word (prose, a URL path segment, a
file name); `YES` for the five formulae with a `service` and a keg plist; and
`UNKNOWN` for the nine casks.

Used by `invariant_17_a_macports_to_homebrew_plan_is_built_from_a_live_capture`
(`crates/cli/tests/migrate.rs`), which plans `htop` in both directions and
asserts the two plans mirror each other; and, since part D, by every test,
schema sample and wiki block that shows a plan with commands.

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
