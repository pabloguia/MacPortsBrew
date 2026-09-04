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

The cask JSON carries no user-intent field, so a cask's ownership is `UNKNOWN`
here and is **never** inferred from a dependency graph (R-1, 2026-09-02).
Homebrew *does* record `installed_on_request` for casks in the on-disk receipt
(R-12, 2026-09-04). That file is not a supported command-line interface, and the
charter's Legal posture confines this project to those, so reading it is DP-012
and the owner's — not something an adapter adds.

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
