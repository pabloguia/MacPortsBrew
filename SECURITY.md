# Security

MacPortsBrew orchestrates package managers, which is privileged infrastructure
work even when MacPortsBrew itself never elevates. Security is therefore a
design property here, not a policy document.

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

There is no security contact address yet, because the remote does not exist
yet. The repository *will* be published publicly on GitHub
([DP-007](docs/decisions-pending.md), decided 2026-09-02), and the route is
decided with it: a **private GitHub security advisory** — the *Security* tab,
*Report a vulnerability* — private to the maintainers until published.

**Private vulnerability reporting must be enabled before the repository is made
public, not after.** A public repository that ships Developer ID-signed binaries
([ADR-0005](docs/adr/0005-release-signing-and-artifacts.md)) with no private
channel forces the first report of a signing-pipeline flaw onto a public
tracker. This file names that channel as the only one the moment it exists.

Until then, if you have found something and are reading this, raise it with the
person who gave you the repository, privately, and describe: what you did, what
happened, what you expected, and the machine you saw it on. `macportsbrew
doctor --report` collects the machine facts in a redacted form — read it before
you send it.

Please do not include credentials, tokens or anything from outside the two
providers' scope in a report. MacPortsBrew never needs them and cannot use
them.

## What is in scope

Anything that could cause MacPortsBrew to damage a working machine, to remove
something before its replacement is verified, to run a command that was not in
a reviewed plan, to elevate where it said it would not, or to mis-report what
is installed. A silent misparse of a package list is a data-loss-class bug and
is treated as a security problem, not a cosmetic one.

Also in scope: anything in the release pipeline that would let a user run a
binary that is not the one the repository built.

## What is out of scope

Vulnerabilities in Homebrew or MacPorts themselves — report those to those
projects. MacPortsBrew is unaffiliated with either and interacts with both only
through their public command-line interfaces.

## Threat model summary

The full model, with the mitigations and the reasoning, is in
[`docs/spec/04-migration-and-safety.md`](docs/spec/04-migration-and-safety.md)
("Threat model"). In short:

| Threat | How it is addressed |
|---|---|
| **Command injection** | Structurally impossible: argv arrays only, no shell, no string interpolation. Package names and paths are arguments, never fragments of a command line. `scripts/check-layering.sh` asserts that `std::process::Command` is constructed only inside `crates/adapters/src/process/`. |
| **Executable shadowing and `PATH` spoofing** | Provider executables are resolved from their detected prefix, not from `PATH`, wherever correctness matters. `doctor` reports writable directories that precede system paths, and duplicate `PATH` entries. |
| **Symlink attacks and unsafe temporary files** | Temporary files are created with restrictive permissions in a private directory; writes never follow a symlink; writes are atomic. |
| **Malicious or malformed provider metadata** | All provider output is untrusted input: sizes are bounded, shapes are validated, nothing is evaluated, and metadata never becomes a path or a flag. A parser that meets output it does not recognise fails with `ErrorId::ProviderOutputChanged` rather than mis-parsing. |
| **Privilege boundaries** | Elevation is per-operation, enumerated, visible, never cached and never inherited by an unrelated step. Discovery, inventory, mapping, analysis, planning and dry-run are unprivileged by design (INV-06), and this build has no code path that elevates at all. |
| **State and configuration integrity** | MacPortsBrew's own state directory is user-owned with restrictive permissions. Corrupt state fails closed — no plan is produced — rather than producing a plan from a damaged record. |
| **Supply chain** | Dependencies are pinned and their licences checked mechanically. Release artifacts carry checksums, and build provenance is a stated goal. |

## The safety invariants a security reviewer should read first

They are in [`docs/spec/01-invariants.md`](docs/spec/01-invariants.md), which
is the only copy. The load-bearing ones for security are INV-01 (destination
before source), INV-02 (every destructive action comes from a reviewed plan),
INV-03 (failure preserves the prior working state), INV-06 (least privilege)
and INV-19 (stateful packages are never migrated blind). Each has at least one
test named after it, findable with
`grep -rn "fn invariant_" crates/*/tests crates/*/src`.

## What this build can do

Version 0.1.0 is read-only. It inventories, analyses, maps, explains and plans;
it executes nothing, elevates nothing, writes nothing into either provider's
prefix, and makes no network calls. A gate script greps the source on every
change for a mutating provider subcommand literal or an elevation token and
fails the build if it finds one. That materially reduces the current attack
surface, and it will grow when the executor lands.
