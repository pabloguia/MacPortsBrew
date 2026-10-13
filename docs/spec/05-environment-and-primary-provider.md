# 05 — Environment and primary provider

**Status:** Tier 3 unless marked. Not implemented in the foundation pass beyond
detection and planning.

## Two different concepts

**Package primary** — which implementation of `git` runs when the user types
`git`. Scope: one package. Does not change dependency ownership (INV-11).

**Global provider priority** — which prefix wins in `PATH` generally.
Scope: everything. Changes the resolution of packages the user never mentioned.

**MUST.** These are separate commands, separate confirmations and separate
history entries (INV-12). Global priority requires an impact preview listing
the commands whose resolution would change, and a stronger confirmation.

## Executable ownership

**MUST.** For any executable on `PATH`, classify its owner:

`SYSTEM` · `HOMEBREW` · `MACPORTS` · `MACPORTSBREW_SHIM` · `EXTERNAL` · `UNKNOWN`

**MUST.** Determine ownership by asking the provider which package owns the
resolved path, following symlinks first. Path prefix matching is a fallback
heuristic and must be marked as such — users relocate prefixes.

**MUST.** Never shadow a macOS system tool without saying so explicitly and
requiring confirmation. Some system binaries are load-bearing for the OS.

## Implementing package primary

**Decided** by R-6 (2026-09-02) against the criteria this section previously
listed: **not one mechanism, but four layers, each used only where it is the
least invasive one that can express the request.** The implementation stays
deferred; the shape does not.

1. **Report only — the default.** `analyze` and `doctor` explain the current
   resolution for each contested executable (winner, its owner, the losers in
   `PATH` order) and change nothing. It is the only option with no failure
   mode, and the Homebrew reads it needs stay offline once Homebrew's API
   cache is present — with no cache, a Ruby read downloads it first, and a
   `brew.env` can outrank the quieting environment
   (`04-migration-and-safety.md`, Process execution; R-15).
2. **Recommend the versioned executable name where one exists** — `python3.12`,
   `gcc-14`. Both providers already converge on this: Homebrew requires
   suffixed binaries before a versioned formula may be linked alongside its
   unversioned counterpart, and automating the same choice is what `port select`
   exists for. It costs nothing and cannot drift. Offer it before offering to
   move any symlink.
3. **Delegate an intra-MacPorts version choice to `port select`.** This is the
   provider-native mechanism doing the job it was built for. MacPortsBrew
   *proposes* the exact command and shows its impact by reading the group's
   `base` file; it does not run it. The write goes into the MacPorts prefix and
   needs elevation, and keeping primary selection off the elevating path is what
   keeps `analyze`, `doctor` and dry-run unprivileged as INV-06 requires. It
   then re-reads the selection to verify.
4. **A MacPortsBrew-owned shim directory for a genuine cross-provider primary**
   — `~/.local/share/macportsbrew/bin` or the platform-appropriate equivalent,
   containing symlinks to the chosen implementation, with exactly one `PATH`
   prepend inside the managed block below. This is the only option that can
   express a cross-provider choice at all, and the only one symmetric between
   the two migration directions (INV-17).

**MUST.** The shim directory is never placed inside a provider's prefix, and in
particular never in `/usr/local/bin`. Homebrew cannot see foreign symlinks in
its own prefix — `brew --prefix --unbrewed` reports regular files only — and
`brew link --overwrite` deletes files that already exist there, so a shim put in
that directory is both invisible to the provider's own checks and deletable by a
routine `brew` operation (R-6, 2026-09-02).

**MUST.** A shim targets the provider's **stable** path, never the versioned
keg: `${prefix}/opt/<formula>/bin/<exe>` for Homebrew — the `opt` symlink
follows an upgrade, the `Cellar` path does not — and `${prefix}/bin/<exe>` for
MacPorts (R-6, 2026-09-02).

**MUST NOT.** Never reorder `/usr/local/bin` against `/opt/local/bin` to achieve
a package primary. That moves every contested name at once — 1024 of them from
Homebrew alone on the machine measured — and is therefore global provider
priority, a separate command with its own impact preview and confirmation
(INV-12, R-6 2026-09-02).

**MUST**, whichever layer is used: deterministic, inspectable, reversible,
idempotent, self-repairing, unmistakably owned by MacPortsBrew, and validated
against stale targets on every run (`doctor` reports broken or orphaned entries).
Neither provider's mechanism self-heals: `port select` never checks that its
target exists, so uninstalling a selected port leaves dangling symlinks and a
stale recorded selection (R-6, 2026-09-02).

**MUST.** Prefer a provider-native mechanism over a MacPortsBrew-owned one
wherever the provider offers one **that can express the requirement**. Owning
less is better. For a cross-provider primary no native mechanism qualifies:
MacPorts' `mportselect` joins both link source and target with `${prefix}`, and
`brew link` symlinks into Homebrew's prefix, so neither can point a name at the
other provider's binary (R-6, 2026-09-02). Layer 4 is not a preference for
owning more; it is the absence of an alternative.

## Shell configuration

**MUST.** Detect shell (zsh, bash, fish) and the correct file for the shell's
login/interactive mode rather than assuming. Support at minimum zsh and bash;
fish where practical.

**MUST.** All MacPortsBrew-owned configuration lives in a single delimited block:

```
# >>> MacPortsBrew managed environment >>>
# managed by macportsbrew <version> — do not edit between these markers
...
# <<< MacPortsBrew managed environment <<<
```

**MUST.** The write procedure is: detect shell → locate file → find existing
managed block → compute the new block → **diff and preview** → confirm →
back up the original → write atomically → verify by re-reading → report.
Content outside the markers is never touched. Re-running produces no change
(INV-16). `macportsbrew env --revert` removes the block and restores.

**MUST NOT.** Never append blindly. Never duplicate a `PATH` entry. Never
reorder entries the user placed themselves. Never modify a shell file during
analysis or dry-run.

## Environment variables

**MUST.** Distinguish variables MacPortsBrew *manages* (its own shim directory)
from those it merely *reports* (`PATH`, `MANPATH`, `PKG_CONFIG_PATH`,
`CPATH`, `LIBRARY_PATH`, compiler flag variables). Managing a build-flag
variable globally is how cross-provider contamination gets created — do not.
Report it; explain it; leave it to the user (INV-10).

## Installing a provider

Deferred to a later pass; the contract is fixed now.

**MUST**, for either provider: detect macOS version and architecture; determine
whether that provider genuinely supports this machine — and say so honestly even
when the answer is "installable, but not recommended here"; identify
prerequisites (Command Line Tools, disk, permissions); use the provider's own
official installation method rather than a reimplementation; explain exactly
what will happen, including what will be elevated; obtain explicit consent
(INV-05); install; verify the binary, version, prefix and environment
integration; report.

**MUST NOT.** Never install, upgrade or reconfigure a provider as a side effect
of another operation. Never recommend a provider on a platform where it is
unsupported without saying so plainly. An advanced override may exist where the
installation is technically feasible — it is never the default path.
