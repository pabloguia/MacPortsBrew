# Getting Started

## Install

MacPortsBrew builds from source. There is no published release yet — the
repository will be public on GitHub
([`../docs/decisions-pending.md`](../docs/decisions-pending.md), DP-007,
decided 2026-09-02) but the remote does not exist yet — so this is currently
the only route.

When there is one, each release will publish three downloads per architecture —
a bare binary, a `.pkg` and a `.dmg` — carrying the same executable and
differing only in how you install it. Which one to take, and why, is in the
[README's install section](../README.md#three-downloads-and-how-to-choose)
(DP-008, decided 2026-09-04).

```sh
git clone <repository-url> macportsbrew
cd macportsbrew
cargo build --release
```

You need `rustup`. The toolchain is pinned in `rust-toolchain.toml` and
installs itself on the first build; do not switch toolchains, because the pin
is what holds the macOS floor of 10.12 on Intel and 11.0 on Apple Silicon.

The binary lands at `target/release/macportsbrew`. Put it somewhere on your
`PATH`, or run it in place.

**`mpb` is the same program under a shorter name.** It is installed alongside
`macportsbrew`, not as a wrapper. If you already have an `mpb` — MIT
Photonic-Bands ships one — installation will tell you and install only
`macportsbrew` rather than shadowing it. A tool whose purpose is preventing
accidental executable shadowing should not start by shadowing something.

You do **not** need Homebrew or MacPorts installed to run MacPortsBrew. It
reports what it finds, including "neither".

## Your first three commands

### `macportsbrew status`

The dashboard: your machine, and each provider's presence, version, prefix,
support state and package count.

```
Machine
  macOS 26.6.2, x86_64, Native

Providers
Provider  Presence  Version  Prefix      Support    Packages
homebrew  Present   6.0.21   /usr/local  Degraded   189 packages (56 top-level)
macports  Absent    -        -           Supported  0 packages (0 top-level)
56 top-level packages analysed; 15 need manual review. Policy 2026.09.04-1.
```

Three things to read here:

- **`degraded` is a support state, not a verdict on the software.** It means
  the provider itself has said this machine is not a first-class target. Run
  `macportsbrew doctor` and it will tell you which provider said what, with the
  source and the date it was read.
- **"56 top-level"** is the number of packages *you asked for*. The rest are
  dependencies that a provider installed on their behalf, and MacPortsBrew
  hides them unless you pass `--advanced` — see
  [Dependencies and Safety](Dependencies-and-Safety.md).
- **The policy version** is the identifier of the dated provider facts this
  answer was computed from. Every recommendation carries it.

### `macportsbrew analyze`

One row per package, with what each provider has, how it got there, whether an
install would be prebuilt or compiled, what kind of package it is, and the
recommendation.

```
Package     Provider  Version  Ownership  Availability  Class             Recommendation
coreutils   homebrew  9.11     Requested  Prebuilt      Simple            Needs a human decision
gemini-cli  homebrew  0.46.0   Requested  Deprecated    Simple            Nothing to do
jupyterlab  homebrew  4.6.3    Requested  Prebuilt      Background service Needs a human decision
python@3.12 homebrew  3.12.14  Requested  Prebuilt      Language runtime  Use a language runtime manager instead
```

"Needs a human decision" is a normal, common outcome. It is not an error, and
it is not the tool refusing to work: it is the tool declining to act
unattended on something where acting wrongly is expensive. The next command
tells you exactly why.

### `macportsbrew explain <package>`

The full derivation: the facts the engine read, the rules that fired, **the
rules that were considered and did not fire**, and the mapping evidence.

```
curl (held by homebrew)
  Recommendation: Needs a human decision
  Policy version: 2026.09.02-1

Facts the policy engine read
  ...
  package.reverseDependencyCount: 2
  provider.homebrew.support: DEGRADED
  provider.homebrew.supportEndDate: 2027-09-01
  ...

Rules that fired
  KEEP_SOURCE_DEFAULT -> Keep under homebrew
      Nothing argues for moving curl, so it stays under homebrew.
  MANUAL_REVIEW_REVERSE_DEPENDENCIES -> Needs a human decision
      2 packages under homebrew depend on this one, and removing or moving it
      on its own can break them.
  NO_ALTERNATIVE_PROVIDER_PRESENT -> Nothing to do
      homebrew is the only package manager on this machine carrying curl, so
      there is nowhere to move it to.

Rules considered that did not fire
  DESTINATION_UNSUPPORTED_HERE
  ...
```

Everything MacPortsBrew concludes is reproducible from that page. If you
disagree with an answer, this output names the rule to argue with.

## The rest of the surface

```sh
macportsbrew list                     # inventory only, no recommendations
macportsbrew list --provider macports # one provider
macportsbrew analyze --advanced       # include dependency-level packages
macportsbrew doctor                   # PATH order, prefixes, provider health
macportsbrew plan <pkg> --to macports # a plan; nothing is executed
macportsbrew snapshot --out state.json
macportsbrew analyze --from-snapshot state.json
```

Add `--json` to any of them for a versioned payload on stdout. See the
[CLI Reference](CLI-Reference.md) and
[`../docs/schemas/README.md`](../docs/schemas/README.md).

## What it will not do yet

`plan` builds and prints a plan. **Executing one is a later pass** — there is
no `migrate` command, and no code path in this binary runs a mutating provider
subcommand or asks for a password. Nothing on your machine changes when you run
any command on this page.

`--dry-run`, `--yes`, `--no-network` and `--refresh` are accepted and do
nothing, because this build is always a dry run, never prompts, never uses the
network and has no cache. They exist so a script written today keeps working.

## Where to go next

- Your Homebrew shows `degraded` and you are on Intel:
  [Intel Mac Homebrew Support](Intel-Mac-Homebrew-Support.md).
- You want to move something: pick your direction from
  [Homebrew to MacPorts](Homebrew-to-MacPorts-Migration.md) or
  [MacPorts to Homebrew](MacPorts-to-Homebrew-Migration.md).
- Something looks wrong: [Troubleshooting](Troubleshooting.md).
