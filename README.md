# MacPortsBrew

[![License](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![SPDX](https://img.shields.io/badge/SPDX-Apache--2.0-informational.svg)](LICENSE)

> See what Homebrew and MacPorts have installed, compare them package by
> package, and plan a migration in either direction. Version 0.1.0 reads and
> plans; it changes nothing.

MacPortsBrew is an open-source package-manager orchestrator for macOS. It
analyses what you have installed under Homebrew and MacPorts, maps equivalent
packages between them, explains which provider is the better choice for each
package on *your* machine, and builds a reviewable migration plan in either
direction — one that never removes anything before its replacement is installed
and verified. The two providers are peers: neither is globally better, and
neither direction is a special case.

**This version plans; it does not execute.** `migrate` shows a plan, and with
`--yes` reaches a refusal gate that refuses every plan this build produces;
nothing in the binary runs a mutating provider subcommand. See
[What this build does *not* do](#what-this-build-does-not-do).

## Homebrew and MacPorts are peers here

Neither provider is globally better, and MacPortsBrew does not have a favourite.
A recommendation is a function of `machine × package × provider × current
support state`, and the same planner, the same policy engine and the same
executor serve **Homebrew to MacPorts** and **MacPorts to Homebrew**. Migrating
back is not an afterthought bolted on to a one-way tool; it is the same code
path with the two provider identifiers exchanged, and a test asserts exactly
that (`invariant_17_swapping_the_providers_swaps_the_analysis`).

Running both side by side is also a supported end state, not a failure to
choose. See [Using Homebrew and MacPorts Together](wiki/Using-Homebrew-and-MacPorts-Together.md).

## Why it exists

Homebrew lists "Intel x86_64 systems running macOS" under **Tier 3**, and states
it will **remove the ability to run Homebrew on Intel systems in or after
September 2027** (source: <https://docs.brew.sh/Support-Tiers>, read
2026-09-04). Tier 3 is *not supported* in Homebrew's words, and it is **not**
Homebrew's *Unsupported* category — that one means Homebrew "will not run
without third-party patches or modifications", and the macOS versions in it are
Mojave 10.14 and earlier. So on 2026-09-04 an Intel Mac above Mojave ran
Homebrew: what it had was a deadline and eroding coverage, which is what
`macportsbrew status` reports as `Degraded`. Which versions sit in which
category is a timeline on that page and it moves — the row after the current one
expects Catalina 10.15 to become Unsupported — so check the source rather than
this paragraph. Homebrew also states it has stopped building new bottles for Intel
systems; measured against the actual `homebrew/core` artifacts on 2026-09-02,
the more accurate description is *frozen and eroding* rather than *stopped* —
6680 of 8586 formulae still carried an Intel `sonoma` bottle at their current
stable version, against 7277 with `arm64_sonoma`
([`docs/research/F-1-homebrew-support-tiers.md`](docs/research/F-1-homebrew-support-tiers.md),
2026-09-02).

That turns a vague story about degradation into a dated deadline for anyone
with an Intel Mac and a Homebrew installation. MacPorts continues to support a
wider range of macOS versions and both architectures; MacPorts 2.12.6 ships
package installers down to macOS 10.12 Sierra
([`docs/research/R-4-macports-binary-archives.md`](docs/research/R-4-macports-binary-archives.md),
read 2026-09-02).

**These are dated facts, and MacPortsBrew treats them as data.** They live in
versioned policy files with their source and read date, never in control flow,
and they must be re-verified — Homebrew may extend, accelerate or reverse the
timeline. If you are reading this long after 2026, check the source before
acting on it.

MacPortsBrew is not an Intel escape hatch. It is a safe orchestration,
migration and coexistence layer that stays useful afterwards, on any Mac.

## What this build does *not* do

This is version 0.1.0. It reads, explains and plans; it does not install,
remove, switch which copy of a tool runs, keep history or restore. The binary
says so in its own `--help`:

- **It executes nothing.** `plan` builds and prints a migration plan, and so
  does `migrate`; `migrate --yes` puts the plan to an execution gate that
  refuses every plan this build produces, so it exits 4 for every plan it
  builds and runs nothing. A package already where you asked exits 0 as
  *nothing to move*; on a machine where the other provider was not detected
  it says so, because a provider installed where detection does not look
  would read the same, and `--json` names it in `notDetected`. The planner's
  own refusals keep their exit codes. No code path in the binary runs a
  mutating provider subcommand, and a check on every change looks for one.
  You can follow a plan by hand, at your own risk; take
  `macportsbrew snapshot --out before.json` first.
- **It never asks for elevation.** Analysis, inventory, mapping, planning and
  `doctor` are all unprivileged by design.
- **Not yet implemented:** executing a migration, `restore`, `primary`,
  `provider-priority`, `providers`, `env`, `history`, `cleanup`, the
  interactive TUI, `doctor --deep`, caching, and translations (English ships,
  plus the `qps` pseudolocale for layout testing).
  `--no-network` and `--refresh` are accepted and do nothing in this build.
  `--yes` makes `migrate` put its plan to the execution gate, which refuses it
  (exit 4); adding `--dry-run` shows the plan without putting it to the gate.
  `--no-network` does not stop a Homebrew read fetching its catalogue on an
  empty cache (see [Privacy](#privacy)).

## Quick start

MacPortsBrew builds from source with a pinned Rust toolchain.
[`rust-toolchain.toml`](rust-toolchain.toml) pins 1.98.0 and both Apple
targets; `rustup` installs it automatically on the first build, and the pin is
what holds the macOS deployment floor
([ADR-0002](docs/adr/0002-implementation-language.md)).

```sh
git clone <repository-url> macportsbrew
cd macportsbrew
cargo build --release
```

The binary is at `target/release/macportsbrew`. No command below changes
anything in either package manager (`snapshot --out` writes only the file you
name):

```sh
macportsbrew status              # machine and provider dashboard
macportsbrew analyze             # inventory, mapping, assessment, recommendations
macportsbrew list                # one row per package across providers
macportsbrew explain git         # the full derivation of one recommendation
macportsbrew plan git --to macports    # a migration plan; nothing is executed
macportsbrew doctor              # prefixes, PATH order, leaked build variables
macportsbrew snapshot            # capture both providers' state to a file
```

Add `--json` to any of them for a versioned, machine-readable payload on stdout
(see [`docs/schemas/`](docs/schemas/README.md)).

**`mpb` is the same program under a shorter name**, installed alongside
`macportsbrew` rather than wrapping it. Installation refuses to shadow an
existing `mpb` on your `PATH` silently — MIT Photonic-Bands ships an executable
by that name — and installs only `macportsbrew` if it finds one
([ADR-0003](docs/adr/0003-name-and-cli-alias.md)).

### What it looks like

<!-- sample: --replay fixtures/recordings/homebrew-only-intel-macos26.json status -->
```
$ macportsbrew status
Machine
  macOS 26.6.2, x86_64, Native

Providers
Provider  Presence  Version  Prefix      Support    Packages
homebrew  Present   6.0.21   /usr/local  Degraded   189 packages (47 top-level)
macports  Absent    -        -           Supported  0 packages (0 top-level)
56 packages in the default view; 15 need manual review. Policy 2026.10.03-2.
Ownership is unknown for 10 rows: the provider kept no record, or this analysis could not read one.
```

## Install

**The plan is a self-contained binary from GitHub Releases**, for `x86_64` and
`arm64`, with checksums, so MacPortsBrew can be installed by someone whose
Homebrew installation is already broken or who has no package manager at all.
`brew install` and `port install` may follow later as conveniences; neither
will ever be the only route.

**This build has no published release.** The repository will be published
on GitHub, but it is not there yet, so the only way to get MacPortsBrew today
is to build it from source as above.

What follows, between the two markers, is also the text of every release's
notes: the release workflow copies it, filling in the version for `X.Y.Z` and
the repository's real owner and name for `<owner>/<repo>`, so the two cannot
drift apart. Until the first release exists, read both as placeholders; the
real values appear in the release notes.

<!-- release-notes:begin -->
<!-- In README.md the version and repository in this section are placeholders; the release workflow fills them in when it copies the section into a release's notes. -->
For macOS 10.12 or later on Intel (`x86_64`) and macOS 11.0 or later on Apple
Silicon (`arm64`). **macOS 10.12–10.15 are best-effort**, and so are 11–14:
each release is built for them and checked to declare that minimum, but only
macOS 15 and 26 are covered by the CI configuration (see `docs/support-matrix.md` in the
repository), and Apple's current tools no longer state support for 10.12.

### Which file to download

Every file contains the **same program**, byte for byte: one build, one signed
executable per architecture, and the release job checks that the copy inside
the `.pkg` and the `.dmg` is identical to the bare file. Take `arm64` on Apple
Silicon and `x86_64` on Intel — `uname -m` in Terminal tells you which.

| File | What it is | What happens when you use it |
|---|---|---|
| `macportsbrew-X.Y.Z-<arch>` | The bare executable: one signed program file | Nothing happens until you place it. Rename it to `macportsbrew`, make it executable, and put it in a folder on your `PATH` — the Terminal line below does all three. Nothing else on your Mac changes. |
| `macportsbrew-X.Y.Z-<arch>.pkg` | A macOS installer package, for your user account only | Double-click it. It is designed to put `macportsbrew` in the hidden folder `.macportsbrew/bin` inside your home folder, without asking for an administrator password (being confirmed on real installs before the first release). It does not change your `PATH` or any shell file; see "Putting it on your `PATH`". |
| `macportsbrew-X.Y.Z-<arch>.dmg` | A macOS disk image holding the executable | Double-click it and a window opens with one file in it. Drag it to a folder you choose. Finder hides folders whose names start with a dot, so to use `~/.macportsbrew/bin`, first run `mkdir -p ~/.macportsbrew/bin` in Terminal, then in Finder press Shift-Command-G, type `~/.macportsbrew/bin`, and drag the file into the window that opens. |

**Not sure? Take the `.pkg`.** **Live in a terminal? This one line downloads,
names, places and marks the program executable:**

```
mkdir -p ~/.macportsbrew/bin && curl -fL -o ~/.macportsbrew/bin/macportsbrew "https://github.com/<owner>/<repo>/releases/download/vX.Y.Z/macportsbrew-X.Y.Z-$(uname -m)" && chmod +x ~/.macportsbrew/bin/macportsbrew
```

### Putting it on your `PATH`

Add this line to `~/.zprofile` if you use zsh (the default since macOS 10.15),
or to `~/.bash_profile` if you use bash (the default on 10.12–10.14), then open
a new Terminal window:

```
export PATH="$PATH:$HOME/.macportsbrew/bin"
```

It adds the folder at the end, so it never hides a program you already have.

### The first time you run it

macOS marks files a browser downloads, so it can check them before they first
run. These files are signed with a Developer ID and notarised by Apple. The
`.pkg` and the `.dmg` carry Apple's approval inside them. The bare file cannot,
so the first time a browser-downloaded bare file runs, your Mac asks Apple over
the network; what happens if there is no network at that moment has not been
established. If you may be offline, use the Terminal line above (`curl` does not
mark the file) or the `.pkg`. Whether the approval inside the `.dmg` also covers
a copy dragged out of it is not yet established either.

### Checking what you downloaded

```
gh attestation verify macportsbrew-X.Y.Z-<arch> --repo <owner>/<repo> --signer-workflow <owner>/<repo>/.github/workflows/release.yml --source-ref refs/tags/vX.Y.Z
shasum -a 256 -c SHA256SUMS --ignore-missing
```

`gh` (GitHub's command-line tool) probably needs macOS 12 or later; on an older
Mac, run the first line on any other computer against the same file. If your
`shasum` refuses `--ignore-missing`, compare `shasum -a 256 <file>` with that
file's line in `SHA256SUMS`. The attestation and Apple's notarisation establish
where a file came from; `SHA256SUMS` only shows the download was not damaged.
`SIGNING-RECORD.txt` records the team identifier, the certificate fingerprint,
the notarisation submission of every file, and the toolchain that built it.
<!-- release-notes:end -->

The release machinery that produces these files is written and reviewed but
has not yet produced a release
([how a release is built](docs/passes/09-release-engineering-report.md)).

### Gatekeeper, honestly

When releases do exist, macOS will treat them like any other downloaded
executable, and it is worth saying plainly how that works rather than leaving
you to discover it:

The blocker is the **quarantine attribute, not the absence of a signature**. On
Intel macOS 26.6.2, unsigned Mach-O executables run without obstruction as long
as they are not quarantined — 267 of 321 executables in one `/usr/local/bin`
are entirely unsigned and all of them work. Quarantine is applied by the
*downloading application* through Launch Services, so a browser sets it and
`curl` does not, and `xattr -d com.apple.quarantine <file>` clears it without
elevation for a file you own. What a *quarantined, unsigned* binary does when
run from a shell on macOS 26 is **not established**: Apple documents Gatekeeper
around opening applications, plug-ins and installer packages, not around
`execve`, and no quarantined Mach-O existed on the machine used to check. The
project assumes the pessimistic case and will document the exact command you
may need. One further fact shaped the answer: a bare binary cannot be
notarisation-stapled, so a `.pkg` and a `.dmg` are published beside it to carry
a stapled ticket. All of this is sourced and dated in
[`docs/research/R-9-gatekeeper-and-distribution.md`](docs/research/R-9-gatekeeper-and-distribution.md)
(2026-09-02, citing Apple's
<https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web>).

Releases will be **Developer ID signed and notarised**
([why](docs/adr/0005-release-signing-and-artifacts.md)). The bare binary is
signed and notarised but not stapled, so verifying it on first run contacts
Apple. The `.pkg` and `.dmg` carry a stapled ticket, which is *intended* to
make them verifiable offline. Whether a stapled ticket reaches a binary copied
out of a container is still being established; it decides what may be
*claimed* for the containers, not whether they ship — all three are published.
When releases exist, each artifact will carry a build provenance
attestation alongside its checksum; the attestation and the notarisation are the
intended trust anchors, and the checksum file guards against transport
corruption and nothing more. Nothing about your machine is sent to this project.

MacPortsBrew will not ship a `curl | sh` installer. It would work — `curl` sets
no quarantine — but a tool asking you to trust it with your package managers
should not open by asking you to pipe an unreviewed remote script into a shell.

## Supported machines

The published [support matrix](docs/support-matrix.md) distinguishes what CI is
configured to exercise from what is merely believed to work, and it is generated
from the CI configuration rather than written by hand — which also means it
states configuration and not results. **No CI job has ever run**: this
repository has no remote yet, so nothing has executed the workflow, and the
matrix says so at the top. macOS 15 and 26 are **configured to be tested**
natively on both architectures; macOS 11 through 14 on both architectures, and
10.12 through 10.15 on Intel, are **best-effort** — the
deployment floor is met and asserted on every artifact, and nothing executes
there, because GitHub keeps hosted images only for the latest two stable
macOS versions (`docs/research/R-10-ci-runners.md`, read 2026-09-02).
Verification on real hardware is welcome.

The floor is macOS 10.12 Sierra on Intel and 11.0 Big Sur on Apple Silicon.
Below 10.12, neither MacPortsBrew nor the provider it would recommend has a
supported path.

## Documentation

| Where | What |
|---|---|
| [`wiki/`](wiki/Home.md) | User documentation: guides, migration walkthroughs, troubleshooting, FAQ, CLI reference |
| [`docs/spec/`](docs/spec/README.md) | The specification, in reading order |
| [`docs/schemas/`](docs/schemas/README.md) | The versioned `--json` schemas |
| [`docs/support-matrix.md`](docs/support-matrix.md) | Tested versus best-effort, generated from the CI configuration |
| [`docs/adr/`](docs/adr/0001-licence-apache-2-0.md) | Architecture decision records |
| [`docs/research/`](docs/research/README.md) | Dated, sourced answers about provider behaviour |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Build, test, architecture, and the contributor recipes |
| [`SECURITY.md`](SECURITY.md) | Threat model summary and how to report a vulnerability |
| [`AGENTS.md`](AGENTS.md) | Orientation for coding agents |

## Privacy

No telemetry. No network calls that report anything about you. MacPortsBrew
makes no network calls of its own. A Homebrew read on an empty cache downloads
Homebrew's catalogue; without a network that read fails with a timeout error
rather than a guess. The reads it asks Homebrew for are run
with Homebrew's update, analytics and hint switches off, but Homebrew still
downloads its package catalogue when its own cache is empty, and a `brew.env`
file in Homebrew's configuration can switch its analytics and network fetches
back on for those reads. `doctor` tells you when such a file sets anything —
it names the file and the settings, never their values — and the same goes for
a MacPorts `~/.macports/macports.conf`, which can redirect where MacPorts reads
its registry and ports from. A migration step against a provider configured
that way is refused — this build refuses every step regardless, and this is the
refusal that remains once migration lands (observed on Homebrew 7.0.4,
2026-10-02, [R-15](docs/research/R-15-homebrew-brew-env.md), and MacPorts
2.12.6, 2026-10-03, [R-16](docs/research/R-16-macports-user-configuration.md)).
Whether that is enough to meet the no-network and no-telemetry promises is
still an open question for the project. `doctor --report` writes a diagnostic
file locally, without home paths, user names or environment values (tested
section by section); read it before you share it. MacPortsBrew never uploads
it.

## Licence

Copyright 2026 Pablo Guia.

Apache License 2.0. The full text is in [`LICENSE`](LICENSE), and every source
file carries `SPDX-License-Identifier: Apache-2.0`. Contributions are licensed
inbound under the same terms as outbound, so **no contributor licence agreement
is required** — see [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Not affiliated

MacPortsBrew is an independent project. It is **not affiliated with, endorsed
by, or sponsored by** the Homebrew project or the MacPorts Project. Those names
belong to their respective projects and are used here only to refer to them.
MacPortsBrew interacts with both solely through their public, supported
command-line interfaces: it never vendors, forks, patches or scrapes them, and
never generates load on their infrastructure beyond what your own commands
would.
