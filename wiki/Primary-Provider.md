# Primary Provider

When both providers have the same tool, only one of them actually runs. This
page is about which, and how much MacPortsBrew is willing to do about it.

## Two different questions

They get confused constantly, so they are named separately here:

- **Ownership** — which provider installed a package and is responsible for
  upgrading and removing it. A fact, per provider.
- **Primary** — which copy your shell resolves when you type the command. A
  consequence of `PATH` order and of what each provider linked.

**Choosing a primary never transfers ownership.** If Homebrew
installed `git` and MacPorts installed `git`, and you make MacPorts' the one
that runs, Homebrew still owns its copy and will still upgrade it. Nothing
about ownership changed.

**A package-level choice is not a global priority.** Making MacPorts'
`git` primary says nothing about `curl`.

## What runs today

```sh
macportsbrew doctor
```

shows the order in which your `PATH` reaches each provider's prefix, a prefix
that is not on `PATH` at all, and duplicate `PATH` entries. Run it on the live
machine: a recording or a snapshot does not carry your `PATH`, and `doctor`
says so rather than guessing.

For one particular tool, ask your shell directly:

```sh
command -v git     # the copy that runs
which -a git       # every copy on PATH, in the order they are found
```

**Per-package resolution is specified but not in 0.1.0.** The design is for
`analyze` to report, for each package, the executable `PATH` resolves to, the
canonical path after following symlinks, and which provider owns it — asking
the provider which package owns the resolved path, with prefix matching as a
marked fallback, and an owner of `UNKNOWN` rather than a guess when nothing
claims it. This build's `analyze` does not show that.

## The four layers, in order of invasiveness

The design is deliberately not one mechanism. Each layer is used only where it
is the least invasive one that can express what you asked for
([`../docs/research/R-6-primary-provider-mechanisms.md`](../docs/research/R-6-primary-provider-mechanisms.md),
2026-09-02).

### 1. Report only — the default, and what this build does

Report what wins and change nothing. It is the only option with no failure
mode, and it is often all you need: once you can see that `/usr/local/bin/git`
is winning over `/opt/local/bin/git`, you know what to do about it. In 0.1.0
this layer is `doctor`'s `PATH`-order report plus `command -v` and `which -a`;
the per-executable report of the winner, its owner and the losers is designed
and not yet built.

### 2. Recommend the versioned executable name

Where one exists — `python3.12`, `gcc-14` — using it is free and cannot drift.
Both providers already converge on this convention: Homebrew requires suffixed
binaries before a versioned formula may be linked alongside its unversioned
counterpart, and automating the same choice is what MacPorts' own selection
mechanism exists for.

MacPortsBrew offers this before it offers to move any symlink.

### 3. Delegate an intra-MacPorts version choice to MacPorts' own selection mechanism

For choosing between versions *within* MacPorts, the provider's own selection
command is the right tool, doing the job it was built for.

**MacPortsBrew proposes the exact command and shows its impact. It does not run
it.** The write goes into the MacPorts prefix and needs elevation, and keeping
primary selection off the elevating path is what keeps analysis, `doctor` and
dry-run unprivileged. Afterwards it re-reads the selection to
verify what happened.

### 4. A MacPortsBrew-owned shim directory

For a genuine cross-provider choice, a directory MacPortsBrew owns —
`~/.local/share/macportsbrew/bin` or the platform equivalent — containing
symlinks to the chosen implementation, with **exactly one** `PATH` prepend
inside a managed block in your shell configuration.

This is the only layer that can express a cross-provider primary at all, and the
only one symmetric between the two migration directions. It is also the most
invasive, which is why it is last.

## What MacPortsBrew will never do to your shell

- **Never edit a shell file outside a single, clearly delimited managed block**,
  added with your consent and removable with `macportsbrew env --revert`.
- **Never change `PATH` silently.**
- **Never shadow a macOS system tool without saying so explicitly and asking.**
  Some system binaries are load-bearing for the operating system.
- **Never shadow an existing `mpb`.** Installation checks for one — MIT
  Photonic-Bands ships an executable by that name — and installs only
  `macportsbrew` if it finds one. A tool whose purpose is preventing accidental
  executable shadowing should not open by shadowing something.

## In this build

Layer 1 only, and only its `PATH`-order part. `macportsbrew primary`,
`macportsbrew provider-priority` and `macportsbrew env` are specified and not
yet written: they do not exist in 0.1.0, and a plan that mentions `primary`
describes a step a later release will run, not a command you can type today. Nothing in version
0.1.0 writes to a shell file, creates a symlink, or changes what runs.

## See also

- [Using Homebrew and MacPorts Together](Using-Homebrew-and-MacPorts-Together.md)
  — the situation that makes this question interesting.
- [Troubleshooting](Troubleshooting.md) — "the wrong version runs".
