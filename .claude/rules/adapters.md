---
paths:
  - "crates/adapters/**"
---

# Adapters — the only layer that touches the machine

Read `docs/spec/02-domain-model.md` ("Provider contract", "Provider presence
states") and `docs/spec/04-migration-and-safety.md` ("Process execution",
"Threat model") before changing anything here. This file does not restate them;
it names the boundaries that a diff in this subtree can quietly cross.

## Never spawn a subprocess outside `process/`

`crates/adapters/src/process/` is the **one** place `std::process::Command` may
be constructed. `scripts/check-layering.sh` fails the build on a `Command::new`
anywhere else under `crates/*/src`; inside `process/` itself the rule in
`docs/spec/04-migration-and-safety.md` (one executor, so environment, argv
and timeout are decided once) is what forbids a second spawn site. An adapter
takes a `&dyn ProcessRunner` and asks it —
which is also what makes the same adapter code run against a live machine and
against a recording (`--replay`).

Inside the executor:

- **Argv arrays only.** No shell, no string interpolation, ever. Package names
  and paths are arguments, never fragments of a command line. This is how
  command injection is made structurally impossible rather than defended
  against, and it is enforced by a lint rule rather than by discipline.
- **Provider subcommands and flags come from a code-level allowlist.** User
  input never becomes a flag.
- **Every invocation records** argv, sanitised environment, exit status, stdout,
  stderr and duration, and supports timeout and cancellation.
- **The environment is sanitised, not inherited.** Homebrew is quieted by
  environment variables (`HOMEBREW_NO_AUTO_UPDATE` does the offline work,
  alongside `HOMEBREW_NO_ANALYTICS` and `HOMEBREW_NO_ENV_HINTS` — R-1 and R-2,
  2026-09-02); MacPorts is quieted by argv (`-q`, which implies `-N`) plus never
  invoking its update or sync verbs, because no `MACPORTS_*` variable serves the
  purpose (R-3, 2026-09-02). The mechanisms genuinely differ. Both live in one
  place, and neither is pretended to be the other.

## Treat all provider output as untrusted input

Bound sizes, validate shapes, never evaluate anything, and never let metadata
become a path or a flag. A parser that meets output it does not recognise
**fails loudly** with `ErrorId::ProviderOutputChanged` rather than mis-parsing.
A silent misparse of a package list is a data-loss-class bug, not a cosmetic
one.

## `UNKNOWN` is a first-class answer

`CapabilityState::Unknown`, `ArtifactAvailability::Unknown`,
`ProviderPresence::Broken`, `Ownership::Unknown` — none of these may be reported
as their favourable neighbour. An unknown is never a yes. The analysis degrades
and says so; it does not guess, and it does not fail either.

The cask JSON carries no user-intent field (R-1, 2026-09-02). Homebrew records
`installed_on_request` in the cask's on-disk `INSTALL_RECEIPT.json`, and
`cask_requested` reads it (R-12, DP-012). **Every way that read can fail answers
`None`** — absent, oversized, a symlink or a FIFO, hardlinked, swapped between
the check and the open, grown while being read, unreadable, not JSON, or a field
that is not a boolean — because `false` would mean "the user did not ask for
this" and make a dependency out of a file the reader could not open. The name is
resolved **once**: every test after the open is made against the handle, which
is the rule `persistence::snapshot_store::read_bounded` states and the first
version of this reader broke. `None` becomes
`Ownership::Unknown`, which is **never** an inferred `TOP_LEVEL` and never a
dependency-graph guess.

Reading a provider's on-disk state is not forbidden here — `service::scan` and
the offline-metadata probe already do it, and the charter's "public, supported
command-line interfaces" clause is about the upstream relationship rather than
local state (F-28). What binds is `FilesystemScope`: under `Elsewhere` the
answer is `None`, because the recording is the machine (INV-15).

## Resolve executables from the detected prefix, not from `PATH`

`PATH` is attacker-influenced and user-reordered. Where correctness matters, the
provider's executable comes from its detected prefix. Path-prefix matching for
*ownership* is a fallback heuristic and must be marked as such
(`Resolution.authoritative == false`) — users relocate prefixes.

## Never mutate, in any form

This layer reads. No install, uninstall, upgrade, link, activate, selfupdate or
sync verb appears in it, and the `Stop` gate greps for exactly those literals.
Nothing here elevates (INV-06), creates a symlink, or writes into a provider
prefix (INV-07, INV-08).

Tests in this crate run against `fixtures/recordings/*.json` and never against a
live installation (INV-05). Capture one with `scripts/capture-fixture.sh`;
fixtures are captured, never invented.

## Adding a provider

An adapter module plus one entry in `registry.rs` — that file is the only place
above the adapter that knows which providers exist. Follow the `add-provider`
skill, or the matching section of `CONTRIBUTING.md`; they are the same steps.
