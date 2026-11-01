# Troubleshooting

## Start here

```sh
macportsbrew doctor
```

It checks the things that are wrong most often, and each finding carries its
reason — and, for anything derived from provider policy, the source and the
date that policy was read. Replayed from the committed recording of an Intel
Mac with Homebrew and no MacPorts
(`macportsbrew --replay fixtures/recordings/homebrew-only-intel-macos26.json doctor`),
it prints:

```
[Warning] This analysis carries no PATH, so nothing can be said about which provider's executables win. A recording does not capture the environment; run `doctor` live for the real answer.
[Warning] homebrew: this input does not record the configuration files of homebrew - it is a recording, or a snapshot taken before they were recorded - so whether one overrides how this tool runs homebrew is unknown.
[Warning] homebrew support for this machine is Degraded (source: https://docs.brew.sh/Support-Tiers, read 2026-09-04).
[Warning] homebrew has announced the end of support for this machine in or after 2027-09-01 (source: https://docs.brew.sh/Support-Tiers, read 2026-09-04). This is dated data; re-check it before relying on it.
[OK] Command Line Tools are installed.
[OK] macports is not installed here (it was not found where it is documented to live).
[OK] homebrew 6.0.21 is present.

Deep inspection of binaries for cross-prefix links (--deep) is not part of this build.

Possibly inconsistent: this was read from a recording or a snapshot file, which cannot show whether a package manager was changing the machine it came from while it was read.
```

The first two warnings are there because a recording carries no `PATH` and no
provider configuration files. Run on your own machine, `doctor` reads both and
reports, for example, a provider prefix that is not on `PATH`.

For an issue report, `macportsbrew doctor --report <new-file>` writes the same
information to a file, without your user name or home folder path. `PATH`
entries a finding mentions appear with your home folder shown as `~`; other
environment variables are named but their values are left out. It also lists
every installed package. It replaces an existing file only if that file is a MacPortsBrew document. Read
it before you share it; MacPortsBrew never uploads it.

## The wrong version of a tool runs

Both providers have it, and `PATH` order decides. `macportsbrew doctor`, run
live, shows the order in which your `PATH` reaches each provider's prefix and
flags duplicate entries. For one tool, ask the shell:

```sh
command -v <tool>   # the copy that runs
which -a <tool>     # every copy on PATH, in order
```

A per-package report of what runs and which provider owns it is specified for
`analyze` but is not in 0.1.0.

See [Primary Provider](Primary-Provider.md) for the four ways to change it,
least invasive first. This build reports; it does not change what runs.

## `doctor` says one provider is exposed to the other

With both Homebrew and MacPorts present, `doctor` reports each direction separately,
because they are not equally risky and the two sentences differ. It is a warning only
when your `PATH` is known and the *other* provider's directory is searched first (or
yours is not on it); a recording carries no `PATH`, so nothing can be judged there and `doctor` says so once
(`path_unknown`).

- **MacPorts' side** (MacPorts 2.12.6, read 2026-10-12). Its own guide says files in common
  system locations such as `/usr/local` can conflict with an installation and suggests
  trying trace mode (`port -t`) when one fails. Trace mode is off unless you ask for it. The guide names `/usr/local`; it says nothing
  about other prefixes such as `/opt/homebrew`.
- **Homebrew's side** (Homebrew 7.0.4, read 2026-10-12). In a formula build Homebrew rebuilds the
  build's `PATH` from its own directories, so a directory on yours is not an input to
  that build. That does not cover a formula that asks for your `PATH`, post-install
  steps, or anything run outside a build.
- **What it does not tell you.** Which copy of a tool runs when you type its name still
  follows your `PATH` order, and that order is yours to arrange. Nothing was looked up
  about what `/usr/local` actually holds on your machine. The finding reports only;
  nothing was changed. The facts are dated data in `data/policy/cross-provider-advice.toml`
  with their sources.

## A package compiles instead of installing

There is no prebuilt artifact for your machine, or non-default options were
requested, or a licence restriction applies. `macportsbrew analyze` names which:

```
Package             Provider    Version      Ownership  Availability                                                Class               Recommendation
…
gh                  homebrew    2.98.0       Requested  Builds from source (no prebuilt artifact for this machine)  Simple              Nothing to do
…
```

(One row of `analyze` replayed from the same recording; `…` marks rows left
out.)

This is normal and increasingly common on Intel. See
[Understanding Bottles and Source Builds](Understanding-Bottles-and-Source-Builds.md),
and [Intel Mac Homebrew Support](Intel-Mac-Homebrew-Support.md) for why.

Check the Command Line Tools are installed — `doctor` says. Homebrew lists the Command Line Tools (or Xcode) among its requirements
(<https://docs.brew.sh/Installation>, read 2026-09-02), and a MacPorts source
build compiles with the same Apple tools. A missing installation may not be
obvious until a build fails.

## "Needs a human decision" on everything

That is usually correct rather than broken. Run:

```sh
macportsbrew explain <package>
```

The most common causes, in rough order of frequency:

- **Reverse dependencies.** Other packages under the same provider depend on
  this one.
- **The package holds data or runs as a service.** Never migrated
  automatically, in either direction.
- **The mapping confidence is below Strong.** Acting needs Strong or better.
- **Availability is unknown.** Something could not be determined offline, and
  an unknown is not a yes.
- **The destination provider is not installed.** This build never installs a
  package manager; install it yourself first.
- **The destination has no equivalent installed.** In 0.1.0 a package is
  matched only against what the other provider already has installed, or the
  small curated table. "No equivalent found" means nothing was found, not that
  no equivalent exists; see [Package Mapping](Package-Mapping.md).

## A package I have is not listed

Three likely reasons:

1. **It is a dependency**, and normal mode hides those. Try
   `macportsbrew analyze --advanced`.
2. **It is a GUI app or cask.** Casks are reported and labelled, and excluded
   from migration planning: MacPorts has no cask format, and a port with the
   same name is often a different version, vendor or install location
   ([research note R-12](../docs/research/R-12-cask-migratability.md), cited
   2026-09-04).
3. **It was not installed by either provider.** MacPortsBrew inventories
   Homebrew and MacPorts and nothing else.

## A provider is installed but reported absent

Detection looks in the standard prefixes: `/opt/homebrew` and `/usr/local` for
Homebrew, `/opt/local` for MacPorts (<https://docs.brew.sh/Installation>,
<https://guide.macports.org>, both read 2026-09-02). Both Homebrew candidates
are probed on every machine, so an Intel Homebrew under Rosetta on an Apple
Silicon Mac is found rather than missed. If a provider is installed in more
than one of those places, `doctor` says so and names where, and which copy
this tool runs: reported only, never changed.

If you have relocated a prefix, detection may miss it. That is worth a
[provider compatibility report](../.github/ISSUE_TEMPLATE/provider-compatibility.yml)
— include the actual prefix.

If the provider is present but not working, `doctor` reports it as broken with
the reason, and the analysis **degrades rather than failing**: the other
provider is still analysed.

## An error I do not understand

Every error carries a stable identifier and a remediation string. The exit code
tells you the category:

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | generic failure |
| 2 | usage error |
| 3 | partial success |
| 4 | blocked by a safety check |
| 5 | provider unavailable or unhealthy |
| 6 | user aborted |
| 7 | lock held by another instance |
| 8 | stale or unusable input |

**Exit 4 from `plan` is normal**, not a fault: it means the plan contains a step
where a human has to decide. The plan is printed anyway, because it is the
useful output.

**Exit 4 from `migrate --yes` is expected for every plan in this build.** The
plan is put to the execution gate, which refuses every plan this build produces
and lists why; nothing runs and nothing changes.

**Exit 4 with nothing on stdout from `migrate --yes --plan-digest`** means the
plan built now is not the one that digest names (`PLAN_NOT_SHOWN`): something
changed since you ran `migrate`. Run `migrate` again, review the plan, and pass
the digest it prints now.

`"error: exit status 1"` is never acceptable output from this tool. If you see
something like it, that is a bug worth reporting.

## Something looks wrong in the numbers

Snapshot it and attach the snapshot:

```sh
macportsbrew snapshot --out state.json   # a new file; an existing one is refused
macportsbrew analyze --from-snapshot state.json
```

Because analysis is a pure function of the snapshot, that file lets anyone
reproduce your exact result. It is written without your user name or home
folder path, but it carries your `PATH` (with your home folder shown as `~`)
and the full list of installed packages; read it before you share it.

## Both providers seem to be interfering with each other

Look for build variables. If `CPPFLAGS`, `LDFLAGS`, `CFLAGS` or
`PKG_CONFIG_PATH` name one provider's prefix, packages the other provider
compiles can pick up the wrong headers and libraries — and the result installs
cleanly and breaks later. `doctor` names any such variable. MacPortsBrew never
manages them for you.

See
[Using Homebrew and MacPorts Together](Using-Homebrew-and-MacPorts-Together.md).

## `doctor` warns about a provider's configuration

Homebrew reads `brew.env` files (`/etc/homebrew/`, its prefix's
`etc/homebrew/`, `~/.homebrew/`) and applies them over the settings this tool
runs it with; MacPorts reads `~/.macports/macports.conf` over its system file.
`doctor` names any such file that sets something — the file and the setting
names, never their values — and a plan carries it as a risk.

- **"sets …"**: the setting is yours to keep. Reads still run, but may behave
  differently from what this tool asks; a migration step against that provider
  will be refused while the file sets it. Remove or move the setting if you
  want this tool to migrate packages of that provider.
- **"exists but could not be read"**: check the file's permissions, or whether
  it is a link to something that is not a regular file.
- **"this input does not record …"**: the output came from a recording or an
  older snapshot. Run `macportsbrew doctor` live to read the files.

## Reporting a problem

Pick the matching template in
[`../.github/ISSUE_TEMPLATE`](../.github/ISSUE_TEMPLATE). Each asks for your
macOS version, architecture, provider versions, MacPortsBrew version and policy
version — `doctor --report` collects all of them. **Never include secrets or
tokens.** For a security problem, do not open an issue; see
[`../SECURITY.md`](../SECURITY.md).
