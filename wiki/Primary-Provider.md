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
machine: a recording carries no `PATH`, `doctor` reads only the live one, and
it says so rather than guessing.

For one particular tool, ask your shell directly:

```sh
command -v git     # the copy that runs
which -a git       # every copy on PATH, in the order they are found
```

**A per-package resolution report is specified but not in 0.1.0.**
`snapshot <package>` records the raw resolution of each executable in its
`--json`, but the report is not built. The design is for
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
this layer is `doctor`'s `PATH`-order report plus `command -v` and `which -a`.
`snapshot <package>` records where each of a package's executables resolves,
and a migration plan reads it (below); the per-executable report of the
winner, its owner and the losers in `analyze` is designed and not yet built.

### 2. Recommend the versioned executable name

Where one exists — `python3.12`, `gcc-14` — using it is free and cannot drift.
Both providers already lean on this convention. Homebrew's documentation says a
versioned formula should be linkable alongside its unversioned counterpart only
if upstream supports it, "e.g. by using suffixed binaries"
(<https://docs.brew.sh/Versions>, read 2026-09-02); MacPorts' own selection
command exists to choose which version the unsuffixed name points to
(`port(1)`, read 2026-09-02; both in
[research note R-6](../docs/research/R-6-primary-provider-mechanisms.md)).

When primary selection is built, MacPortsBrew is designed to offer this before
it offers to move any symlink.

### 3. Delegate an intra-MacPorts version choice to MacPorts' own selection mechanism

For choosing between versions *within* MacPorts, the provider's own selection
command is the right tool, doing the job it was built for.

**When this layer is built, MacPortsBrew will propose the exact command and
show its impact. It will not run it.** The write goes into the MacPorts prefix
and needs elevation, and keeping primary selection off the elevating path is
what keeps analysis, `doctor` and dry-run unprivileged. Afterwards it will
re-read the selection to verify what happened.

### 4. A MacPortsBrew-owned shim directory

For a genuine cross-provider choice, a directory MacPortsBrew would own — the
specification sketches `~/.local/share/macportsbrew/bin`, but the location is
not decided; it is neither `~/.macportsbrew/bin`, where the installer puts the
program, nor the state directory — containing symlinks to the chosen
implementation, with **exactly one** `PATH` prepend inside a managed block in
your shell configuration. Not built in 0.1.0.

This is the only layer that can express a cross-provider primary at all, and the
only one symmetric between the two migration directions. It is also the most
invasive, which is why it is last.

## What MacPortsBrew will never do to your shell

- **Never edit a shell file outside a single, clearly delimited managed block**,
  added with your consent and removable with `macportsbrew env --revert`.
- **Never change `PATH` silently.**
- **Never shadow a macOS system tool without saying so explicitly and asking.**
  Some system binaries are load-bearing for the operating system.
- **Never shadow an existing `mpb`.** MIT Photonic-Bands ships an executable by
  that name. The signed installer cannot check for one, and no install route here
  does today, so the install directory is only ever appended to `PATH`, and an
  earlier `mpb` wins. A tool
  whose purpose is preventing accidental executable shadowing should not open by
  shadowing something.

## In this build

Layer 1 only, and only its `PATH`-order part, plus the decision a plan
records. `macportsbrew snapshot <package>` records the executables a package
ships and where each one resolves on your `PATH`. A migration plan then decides
its primary per executable from that snapshot alone, the same way in both
directions: it completes only where the destination's copy was already
observed winning, or where the package ships no executable, and it predicts no
win. Where only a shim (layer 4) could make the destination's copy run, it is
refused with `PRIMARY_NEEDS_SHIM` and the source is kept. A destination that is
not installed yet is refused too, in both directions, while DP-055 is open, so
for most migrations the primary half is refused. A plan records that, before
any source would be removed, a fresh search of `PATH` must find each executable
in the destination's own package, and it names the `command -v` check you can
run yourself. In this release the gate refuses every migration plan before its
first step, so these checks are what a plan records, not something that runs.
`macportsbrew primary`,
`macportsbrew provider-priority` and `macportsbrew env` are specified and not
yet written: they do not exist in 0.1.0. A plan's `SET_PRIMARY` step records
a decision and runs no command, and `macportsbrew primary` is not a command you
can type today. Nothing in version
0.1.0 writes to a shell file, creates a symlink, or changes what runs.

## See also

- [Using Homebrew and MacPorts Together](Using-Homebrew-and-MacPorts-Together.md)
  — the situation that makes this question interesting.
- [Troubleshooting](Troubleshooting.md) — "the wrong version runs".
