# R-10 — Are there hosted Intel x86_64 macOS runners on GitHub Actions today, and what is the fallback if not?

- **Status:** answered (main question); one sub-item `UNRESOLVED` (see *Cannot determine*)
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (build 25G83), x86_64, Homebrew 6.0.21 @ `/usr/local`,
  MacPorts not installed. No Rust toolchain installed (see E-14).
- **Blocking:** `docs/spec/07-quality-and-testing.md` (CI: "a build for **both**
  `x86_64` and `arm64` macOS"; the deployment-target assertion on built
  artifacts; "Hosted Intel macOS runner availability is uncertain and must be
  verified, not assumed"; the tested/best-effort/unsupported support matrix);
  `docs/adr/0002-implementation-language.md` ("What the foundation pass must
  still verify", which records *"No hosted CI runner for macOS 10.12–10.15 is
  known (R-10, open; treated as absent until answered)"*); the still-unwritten
  macOS build-and-test job in `.github/workflows/`.

> **Machine note, added 2026-09-15.** This note states, correctly for its answer
> date, that MacPorts could not be observed here. **It can be since 2026-09-06**,
> when MacPorts 2.12.6 was installed at `/opt/local` (first ports 2026-09-07).
> Nothing below has been re-checked against it; the citations stand as citations.
> R-5 and R-5a were re-checked in place and show what that costs — R-5 gained
> fourteen evidence blocks and produced SE-077. Where this note names a "future
> attempt" that needed a MacPorts machine, that attempt is now available (F-46).

## Question

Two things, and they are not the same question:

1. Does GitHub Actions offer **any** hosted Intel x86_64 macOS runner today —
   label, macOS version, size class, cost — so that the `x86_64` half of the
   mandated two-architecture build can be *built and tested natively* rather
   than only cross-compiled?
2. If not, or in addition, which fallbacks are real: cross-compiling
   `x86_64-apple-darwin` from an arm64 runner and asserting the deployment
   target on the artifact; running Intel binaries under Rosetta 2 on arm64
   runners; a self-hosted runner on the owner's Intel Mac.

It blocks the CI workflow because `07-quality-and-testing.md` forbids claiming
tested support that CI does not actually exercise, and `.github/workflows/`
carries a comment saying the macOS job "lands with the research".

Two non-blocking GitHub questions from the same checklist — wiki publishing
from a repository directory, and topic conventions — are answered in the final
section.

## Evidence

Provider quiet-environment note: **R-10 required no `brew` or `port`
invocation at all.** Nothing in this note is derived from provider output on
this machine, so no auto-update suppression was needed here. The variables that
*do* apply, established in `R-1-homebrew-read-interfaces.md` and carried into
the recommended workflow below, are `HOMEBREW_NO_AUTO_UPDATE=1`,
`HOMEBREW_NO_ANALYTICS=1`, `HOMEBREW_NO_ENV_HINTS=1`, with
`HOMEBREW_NO_INSTALL_FROM_API` left unset. They matter for CI because Homebrew
is preinstalled on every GitHub macOS image (E-12).

All `gh api` calls below are read-only HTTP GETs against public GitHub
endpoints. No repository state was modified.

### E-1 — This machine (observed 2026-09-02)

```
$ sw_vers
ProductName:		macOS
ProductVersion:		26.6.2
BuildVersion:		25G83
$ uname -m
x86_64
$ arch
i386
```

### E-2 — The deployment-target assertion works on a real Mach-O (observed 2026-09-02)

```
$ vtool -show-build /bin/ls
/bin/ls (architecture x86_64):
Load command 9
      cmd LC_BUILD_VERSION
  cmdsize 32
 platform MACOS
    minos 10.14
      sdk 26.6.1
   ntools 1
     tool LD
  version 1267.0
/bin/ls (architecture arm64e):
Load command 11
      cmd LC_BUILD_VERSION
  cmdsize 32
 platform MACOS
    minos 11.0
      sdk 26.6.1
   ntools 1
     tool LD
  version 1267.0
```

```
$ otool -l /bin/ls | grep -A5 LC_BUILD_VERSION
      cmd LC_BUILD_VERSION
  cmdsize 32
 platform 1
    minos 10.14
      sdk 26.6.1
   ntools 1
     tool 3
```

### E-3 — Apple's own documentation on this machine (observed 2026-09-02)

```
$ man vtool
VTOOL(1)                     General Commands Manual                    VTOOL(1)

NAME
     vtool - Mach-O version number utility
...
     The vtool utility displays and edits build and source version numbers
     embedded in the Mach-O(5) file format. These version numbers are stored
     within the Mach-O load commands, as described in the <mach-o/loader.h>
     header file and in the VERSION LOAD COMMANDS section below. When editing
     files, a new out_file must be specified using the -output flag; vtool will
     only ever write to a single output file, and input files are never modified
     in place.
...
     Show          Show options include -show, -show-build, -show-source, and
                   -show-space.
```

```
$ man arch
ARCH(1)                      General Commands Manual                     ARCH(1)

NAME
     arch - print architecture type or run selected architecture of a universal
     binary
...
     The other use of the arch command is to run a selected architecture of a
     universal binary.  A universal binary contains code that can run on
     different architectures.  By default, the operating system will select the
     architecture that most closely matches the processor type.
...
     The arch_name argument must be one of the currently supported
     architectures:

           i386     32-bit intel

           x86_64   64-bit intel

           x86_64h  64-bit intel (haswell)
```

`$ xcodebuild -version` → `Xcode 26.5` / `Build version 17F42`.

`$ arch -x86_64 uname -m` → `x86_64`. **This is not evidence about Rosetta**:
this host is Intel, so `arch -x86_64` selects the native slice. It is recorded
only so a later reader does not mistake it for a Rosetta observation.

### E-4 — GitHub-hosted runners reference table

Source: <https://docs.github.com/en/actions/reference/runners/github-hosted-runners>,
read 2026-09-02. (The older URL
`…/using-github-hosted-runners/using-github-hosted-runners/about-github-hosted-runners`
no longer carries the hardware table.)

Standard runners, **public repositories** (rows containing macOS, quoted):

| Virtual machine | Processor (CPU) | Memory (RAM) | Storage (SSD) | Architecture | Workflow label |
|---|---|---|---|---|---|
| macOS | 4 | 14 GB | 14 GB | Intel | `macos-15-intel`, `macos-26-intel` |
| macOS | 3 (M1) | 7 GB | 14 GB | arm64 | `macos-latest`, `macos-14`, `macos-15`, `macos-26`, `xcode-27` (Public preview) |

The **private repositories** table on the same page carries identical macOS
rows (same CPU/RAM/storage/labels).

Quoted notes from the same page:

> "Use of the standard GitHub-hosted runners is free and unlimited on public
> repositories."

> "All actions provided by GitHub are compatible with arm64 GitHub-hosted
> runners. However, community actions may not be compatible with arm64 and need
> to be manually installed at runtime."

> "The arm64 macOS runners do not have a static UUID/UDID assigned to them
> because Apple does not support this feature. However, Intel MacOS runners are
> assigned a static UDID."

### E-5 — `actions/runner-images` README, "Available images"

Source: <https://raw.githubusercontent.com/actions/runner-images/main/README.md>,
read 2026-09-02. Rows whose image name contains macOS or Xcode. Each source row
carries an inline shields.io badge image and an `Endpoint Badge` image; those
have been elided and replaced by the marker `<BADGE:name -> issue N>`, which
records the badge's *label* and the issue it links to, because that label is
what carries the deprecation state. Everything else is verbatim:

```
| Image | Architecture | YAML Label | Included Software |
| Xcode 27 <BADGE:preview -> issue 14404>          | arm64 | `xcode-27` or `xcode-27-xlarge` | [xcode-27] |
| macOS 26                                         | x64   | `macos-latest-large`, `macos-26-intel`, `macos-26-large` | [macOS-26] |
| macOS 26 Arm64                                   | arm64 | `macos-latest`, `macos-26` or `macos-26-xlarge` | [macOS-26-arm64] |
| macOS 15                                         | x64   | `macos-15-large`, or `macos-15-intel` | [macOS-15] |
| macOS 15 Arm64                                   | arm64 | `macos-15`, or `macos-15-xlarge` | [macOS-15-arm64] |
| macOS 14 <BADGE:deprecated -> issue 13518>       | x64   | `macos-14-large` | [macOS-14] |
| macOS 14 Arm64 <BADGE:deprecated -> issue 13518> | arm64 | `macos-14` or `macos-14-xlarge` | [macOS-14-arm64] |
```

There is **no `macos-13` row of any kind** in this table.

### E-6 — Changelog: macOS 13 (the last "plain" Intel label) retired

Source:
<https://github.blog/changelog/2025-09-19-github-actions-macos-13-runner-image-is-closing-down/>,
published 2025-09-19, read 2026-09-02.

- Labels retired: `macos-13`, `macos-13-large`, `macos-13-xlarge`.
- Full retirement date: **2025-12-04**; brownouts 2025-11-04/05, 11-11/12,
  11-18/19, 11-25/26.
- Migration guidance names arm64 `macos-15` / `macos-14`, **and introduces a new
  Intel label**: `macos-15-intel`, alongside `macos-14-large` / `macos-15-large`.
- Quoted: *"GitHub will no longer support this architecture on macOS after the
  macOS 15 runner image is retired in Fall 2027."*
- No pricing statement in this entry.

### E-7 — Changelog: macOS 26 GA, including an Intel label

Source:
<https://github.blog/changelog/2026-02-26-macos-26-is-now-generally-available-for-github-hosted-runners/>,
published 2026-02-26, read 2026-09-02.

Labels announced: `macos-26` (standard arm64), **`macos-26-intel` (standard
x64)**, `macos-26-large` (large x64), `macos-26-xlarge` (xlarge arm64).
Quoted: *"macOS 26 runners run natively on Apple Silicon (arm64) and also
support Intel (x64)."* No pricing or deprecation statement in this entry.

### E-8 — Changelog: `macos-latest` now points at macOS 26

Source:
<https://github.blog/changelog/2026-05-14-github-actions-upcoming-image-migrations/>,
published 2026-05-14, read 2026-09-02. Quoted: *"The `macos-latest` image label
migration will begin June 15 and take 30 days to complete. The `macos-latest`
image label will point to the macOS 26 image instead of macOS 15."* and *"To
continue using macOS 15, you can target the `macos-15` label directly."*

### E-9 — macOS 14 deprecation window

Source: `actions/runner-images` issue 13518, created 2026-01-11, state `open`,
<https://github.com/actions/runner-images/issues/13518>, read 2026-09-02 via
`gh api repos/actions/runner-images/issues/13518`. Quoted:

> "**Deprecation will begin on July 6th, 2026** and the **images will be fully
> unsupported by November 2nd, 2026** for GitHub Actions and Azure DevOps."

Affected: `macos-14`, `macos-14-large`, `macos-14-xlarge` (macOS 14 and macOS
14 Arm64). Brownouts run across October 2026. Mitigation: move to
`macos-latest` / `macos-15` / `macos-26`.

### E-10 — Billing: free minutes and the macOS rate

Source: <https://docs.github.com/en/billing/concepts/product-billing/github-actions>
and <https://docs.github.com/en/billing/reference/actions-minute-multipliers>,
both read 2026-09-02.

Included minutes per month, **private repositories only**: GitHub Free 2,000;
GitHub Pro 3,000; GitHub Team 3,000; GitHub Enterprise Cloud 50,000.

Quoted: *"The use of standard GitHub-hosted runners is free"* on public
repositories; *"Larger runners are always charged for, even when used by public
repositories."*; *"The larger runners are not free for public repositories."*;
*"Included minutes cannot be used for larger runners."*

Standard-runner per-minute rates, verbatim table from the reference page:

| Operating system | Billing SKU | Per-minute rate (USD) |
|---|---|---|
| Linux 1-core (x64) | `actions_linux_slim` | $0.002 |
| Linux 2-core (x64) | `actions_linux` | $0.006 |
| Linux 2-core (arm64) | `actions_linux_arm` | $0.005 |
| Windows 2-core (x64) | `actions_windows` | $0.010 |
| Windows 2-core (arm64) | `actions_windows_arm` | $0.010 |
| macOS 3-core or 4-core (M1 or Intel) | `actions_macos` | $0.062 |

Larger macOS runners: `macos_l` (macOS 12-core) $0.077/min; `macos_xl`
(macOS 5-core, M2 Pro) $0.102/min.

The word **"multiplier" does not appear on that page** — GitHub now expresses
this as a per-minute price, not as a per-OS multiplier applied to a Linux
baseline.

### E-11 — Rosetta 2 on GitHub's arm64 macOS images

Source: the `actions/runner-images` repository itself, read 2026-09-02 via
`gh api -X GET search/code -f q='Rosetta repo:actions/runner-images'` →
6 hits: `images/macos/scripts/build/install-rosetta.sh`,
`images/macos/templates/macOS-26.arm64.anka.pkr.hcl`,
`images/macos/templates/xcode-27.arm64.anka.pkr.hcl`,
`images/macos/templates/macOS-15.arm64.anka.pkr.hcl`,
`images/macos/templates/macOS-14.arm64.anka.pkr.hcl`,
`images/macos/scripts/tests/Rosetta.Tests.ps1`.

`images/macos/scripts/build/install-rosetta.sh`, verbatim:

```bash
#!/bin/bash -e -o pipefail
################################################################################
##  File:  install-rosetta.sh
##  Desc:  Install Rosetta
################################################################################

echo "Installing Rosetta"
/usr/sbin/softwareupdate --install-rosetta --agree-to-license
```

`images/macos/templates/macOS-26.arm64.anka.pkr.hcl`, lines 167–172, verbatim:

```hcl
    scripts         = [
      "${path.root}/../scripts/build/install-xcode-clt.sh",
      "${path.root}/../scripts/build/install-homebrew.sh",
      "${path.root}/../scripts/build/install-rosetta.sh"
    ]
  }
```

`images/macos/scripts/tests/Rosetta.Tests.ps1`, verbatim:

```powershell
Import-Module "$PSScriptRoot/../helpers/Common.Helpers.psm1"

$os = Get-OSVersion

Describe "Rosetta" -Skip:(-not $os.IsArm64) {
    It "Rosetta is available" {
        $commandResult = Get-CommandResult "/usr/bin/pgrep oahd"
        $commandResult.Output | Should -Match "\d+"
        $commandResult.ExitCode | Should -Be 0
    }
}
```

Note: none of the four macOS image *manifests* (`macos-15-arm64-Readme.md`,
`macos-26-arm64-Readme.md`, …) mentions Rosetta in its installed-software list.
The build template and the image test are the evidence; the published manifest
is silent.

### E-12 — What the macOS images actually contain

Source: `images/macos/*-Readme.md` in `actions/runner-images`, read 2026-09-02
via `gh api repos/actions/runner-images/contents/images/macos/<file>`:

| Manifest | OS Version | Image Version | Homebrew | occurrences of "macports" |
|---|---|---|---|---|
| `macos-26-Readme.md` (x64) | macOS 26.6.1 (25G76) | 20260824.0517.1 | Homebrew 6.0.18 | 0 |
| `macos-26-arm64-Readme.md` | macOS 26.5.2 (25F84) | 20260728.0273.1 | Homebrew 6.0.13 | 0 |
| `macos-15-Readme.md` (x64) | macOS 15.7.9 (24G830) | 20260824.0482.1 | Homebrew 6.0.18 | 0 |
| `macos-15-arm64-Readme.md` | macOS 15.7.7 (24G720) | 20260727.0256.1 | Homebrew 6.0.12 | 0 |

### E-13 — Apple: Rosetta has an announced end

Source: <https://developer.apple.com/news/?id=w5ngl9k2>, "Upcoming changes to
Rosetta support for Intel-based macOS apps", published **2026-09-01**, read
2026-09-02. Verbatim:

> "In 2020, we introduced Apple silicon for Mac computers, providing Rosetta as
> a translation layer during the transition period. As announced at WWDC last
> year, we're now concluding this transition."
>
> - "macOS 26.4 or later: Users may receive a system notification when
>   launching apps that rely on Rosetta, alerting them to update to an Apple
>   silicon native version."
> - "macOS 27: Final release to support Rosetta — Intel-only apps will no
>   longer run on Mac computers with Apple silicon after this update."
>
> "Please note that Rosetta functionality for older, unmaintained gaming titles
> that rely on Intel-based frameworks will continue to be supported."

### E-14 — Rust targets and deployment target

Source: <https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html>
and <https://doc.rust-lang.org/rustc/platform-support.html>, read 2026-09-02.
(The Rust project's own documentation — the toolchain analogue of "official",
not one of the provider sources named in `README.md`.)

- `aarch64-apple-darwin` — **Tier 1**, "macOS on ARM64 (M1-family or later
  Apple Silicon CPUs)", minimum macOS 11.0 Big Sur.
- `x86_64-apple-darwin` — **Tier 2 (with host tools)**, "macOS on 64-bit x86",
  minimum macOS 10.12 Sierra.
- Verbatim: *"The minimum supported version can be raised per-binary by
  changing the deployment target. `rustc` respects the common environment
  variable `MACOSX_DEPLOYMENT_TARGET` used by Xcode for this purpose"*, and the
  current default is printable with `rustc --print=deployment-target`.
- Verbatim: *"Cross-compilation of these targets is supported using Clang, but
  may require Xcode or the macOS SDK (`MacOSX.sdk`) to be available to compile
  C code and link."* `SDKROOT` selects the SDK, otherwise inferred as
  `xcrun --sdk macosx --show-sdk-path`.

Observed on this machine 2026-09-02: `which rustup cargo rustc` → all "not
found". So no claim in this note about rustup/cargo behaviour is observed; the
Rust items are cited only.

### E-15 — Self-hosted runners and public repositories

Source: GitHub docs, self-hosted runner access management
(<https://docs.github.com/en/actions/how-tos/manage-runners/self-hosted-runners/manage-access>),
read 2026-09-02. Verbatim:

> "We recommend that you only use self-hosted runners with private
> repositories. This is because forks of your public repository can potentially
> run dangerous code on your self-hosted runner machine by creating a pull
> request that executes the code in a workflow."

### E-16 — GitHub wikis (non-blocking item)

Sources, both read 2026-09-02:
<https://docs.github.com/en/communities/documenting-your-project-with-wikis/about-wikis>
and
<https://docs.github.com/en/communities/documenting-your-project-with-wikis/adding-or-editing-wiki-pages>.

> "Wikis are part of Git repositories, so you can make changes locally and push
> them to your repository using a Git workflow."

```shell
$ git clone https://github.com/YOUR-USERNAME/YOUR-REPOSITORY.wiki.git
```

> "only changes pushed to the default branch will be made live and available to
> your readers."

> "the filename determines the title of your wiki page, and the file extension
> determines how your wiki content is rendered" — Markdown (`*.md`,
> `*.markdown`) and Textile (`*.textile`) among the supported formats.

> "Don't use the following characters in your wiki page's titles: `\ / : * ? " < > |`"

> "By default, only people with write access to your repository can make
> changes to wikis"; a public repository may be configured to allow everyone on
> GitHub.com to contribute.

> "If you create a wiki in a public repository, the wiki is available to the
> public."

> "Search engines will only index wikis with 500 or more stars that you
> configure to prevent public editing."

Workflow permissions reference
(<https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#permissions>,
read 2026-09-02) lists the available `permissions:` scopes as `actions`,
`artifact-metadata`, `attestations`, `checks`, `code-quality`, `contents`,
`deployments`, `discussions`, `id-token`, `issues`, `packages`, `pages`,
`pull-requests`, `security-events`, `statuses`, `vulnerability-alerts`.
**There is no `wiki` scope.**

### E-17 — GitHub topics (non-blocking item)

Source:
<https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/classifying-your-repository-with-topics>,
read 2026-09-02. Verbatim guidance:

> "Add no more than 20 topics."
> "Use lowercase letters, numbers, and hyphens."
> "Use 50 characters or less."

Topics should reflect "the repository's intended purpose, subject area,
community, or language". GitHub analyses public repositories and suggests
topics that maintainers accept or reject. Featured topics are curated at
`github.com/topics/` via the `github/explore` repository.

## Findings

1. **Yes — hosted Intel x86_64 macOS runners exist today, as *standard* runners,
   free on public repositories.** Two labels: **`macos-15-intel`** (macOS 15,
   image 20260824.0482.1, macOS 15.7.9) and **`macos-26-intel`** (macOS 26,
   image 20260824.0517.1, macOS 26.6.1). Both are 4-CPU / 14 GB RAM / 14 GB SSD
   Intel machines and appear in the *standard* runners table for both public and
   private repositories (E-4, E-5, E-7). ADR-0002's parenthetical
   *"No hosted CI runner … is known (R-10, open; treated as absent until
   answered)"* was correct as a statement about macOS 10.12–10.15 (finding 8)
   but the broader worry — no Intel runner at all — is resolved in the
   affirmative.

2. **This is a *new* mechanism, not the old one.** The historical Intel path was
   `macos-13` (retired 2025-12-04, E-6) and the `-large` size class. The current
   path is a dedicated `-intel` **suffix on the standard size class**, introduced
   in the same changelog that retired `macos-13` (E-6) and extended to macOS 26
   at GA (E-7). Distinguishing these matters for cost (finding 4): `-intel` is
   standard and free on public repos; `-large` is a larger runner and is
   *never* free.

3. **`macos-latest` is arm64.** Since the migration that began 2026-06-15 it
   points at macOS 26 arm64 (E-8, E-4). `macos-latest-large` is, confusingly,
   the **x64** macOS 26 image (E-5). Pin explicit labels; do not use
   `macos-latest` in this project's workflows.

4. **Cost.** Standard runners of any OS are *"free and unlimited on public
   repositories"* (E-4, E-10) — so a public MacPortsBrew repository pays nothing
   for `macos-26-intel`, `macos-26` and `macos-15-intel` jobs. For a **private**
   repository, macOS standard minutes are billed at **$0.062/min** against
   monthly included minutes of 2,000 (Free) / 3,000 (Pro, Team) / 50,000 (GHEC)
   (E-10). Derived, not quoted: $0.062 ÷ $0.006 (Linux 2-core x64) ≈ **10.3×**,
   which is the historical "macOS 10× multiplier" in its current form; GitHub no
   longer publishes a multiplier, only the rate (E-10). Larger runners
   (`macos-*-large`, `macos-*-xlarge`, $0.077–$0.102/min) are charged even on
   public repositories and cannot draw on included minutes — **do not use them**.
   This makes the choice between public and private repository (DP-007's
   neighbourhood) a real cost decision: on a public repo the macOS matrix is
   free; on a private GitHub Free repo, 2,000 included minutes buy roughly 2,000
   macOS runner-minutes' worth of quota consumption before $0.062/min applies.

5. **Rosetta 2 is present on GitHub's arm64 macOS images, but it is a shrinking
   asset.** `install-rosetta.sh` runs in the macOS 14/15/26 arm64 and Xcode 27
   arm64 Packer templates, and an image test asserts `pgrep oahd` succeeds on
   arm64 images (E-11). So `arch -x86_64 ./target/x86_64-apple-darwin/…` would
   work on `macos-26`. However, Apple announced on **2026-09-01** that
   **macOS 27 is the final release to support Rosetta** for ordinary apps
   (E-13). A CI design that depends on Rosetta acquires an expiry roughly a year
   out. Rosetta is therefore recorded as **available but not to be depended
   upon**; it is a nice-to-have cross-check, not the Intel test strategy.

6. **The deployment-target assertion is straightforward and can be written
   today.** `vtool -show-build` prints `platform`/`minos`/`sdk` **per
   architecture slice** on this machine's system binaries (E-2, E-3), and
   `otool -l` prints the same `LC_BUILD_VERSION` fields. `rustc` honours
   `MACOSX_DEPLOYMENT_TARGET` and prints its current default via
   `rustc --print=deployment-target` (E-14). The CI assertion is: build with
   `MACOSX_DEPLOYMENT_TARGET=10.12` (x86_64) / `11.0` (arm64), then parse
   `vtool -show-build <artifact>` and fail if `minos` exceeds the floor. Prefer
   `vtool` over `otool -l`: it labels the architecture slice in its output and
   prints `platform MACOS` symbolically rather than as `platform 1`.

7. **Cross-compilation remains the right way to *build* both slices, and native
   Intel runners the right way to *test* the Intel slice.** `x86_64-apple-darwin`
   is **Tier 2 with host tools** (not Tier 1 — `aarch64-apple-darwin` is Tier 1),
   minimum macOS 10.12; cross-compilation between the Darwin targets is
   supported via Clang with the macOS SDK available, which every GitHub macOS
   image has (E-14). Because a *native* Intel standard runner exists and is free
   on public repos (finding 1), there is no reason to test x86_64 only by
   cross-compilation plus emulation. Build native on each; cross-compile only as
   a redundancy check that the cross path does not rot.

8. **macOS 10.12–10.15 has no hosted path, and none is coming.** The oldest
   hosted macOS today is **macOS 15** (`macos-15`, `macos-15-intel`); macOS 14
   is deprecated with full unsupport on **2026-11-02** (E-9), and GitHub states
   it maintains "the latest two stable versions of any given OS version" (E-9).
   ADR-0002's "no hosted CI runner for macOS 10.12–10.15 is known" **stands, and
   should be restated as a policy consequence rather than a gap in knowledge**:
   GitHub's N-1 image policy structurally excludes it. Those combinations must be
   published as **best-effort**, never as **tested**.

9. **Intel hosted runners have a stated end date, and the two statements about
   it are in tension.** The 2025-09-19 changelog says *"GitHub will no longer
   support this architecture on macOS after the macOS 15 runner image is retired
   in Fall 2027"* (E-6), yet `macos-26-intel` shipped GA on 2026-02-26 (E-7),
   which is a macOS 26 Intel image outliving that sentence's premise. Treat
   **Fall 2027** as the earliest credible date for losing hosted Intel and
   **re-check** (see below). This is dated policy data in the sense of INV-20 and
   should be held the same way the Homebrew Tier 3 / September 2027 statement is.

10. **Homebrew is preinstalled on every GitHub macOS image; MacPorts is on
    none** (E-12). Two consequences. (a) Any CI job that shells out near
    Homebrew must carry the quiet environment (`HOMEBREW_NO_AUTO_UPDATE=1`,
    `HOMEBREW_NO_ANALYTICS=1`, `HOMEBREW_NO_ENV_HINTS=1`) or a runner's
    preinstalled Homebrew will auto-update mid-job and make output
    non-reproducible. (b) A MacPorts adapter can **not** be contract-tested
    against a live MacPorts on a hosted runner without installing MacPorts,
    which `07-quality-and-testing.md` forbids ("Tests run against captured
    fixtures, never against a live install"). The asymmetry is not a reason to
    treat MacPorts as second-class (INV-17): **both** adapters are tested from
    captured fixtures, and the presence of Homebrew on the runner is treated as
    contamination to be neutralised, not as a convenience to exploit.

11. **A self-hosted runner on the owner's Intel Mac is technically available and
    should not be used for a public repository.** This machine is Intel on
    macOS 26.6.2 (E-1) and would be a valid self-hosted Intel runner. GitHub's
    own guidance is explicit that forks of a public repository can run dangerous
    code on a self-hosted runner (E-15). Combined with the project rule that
    agents never touch the developer's real package managers, a self-hosted
    runner on *this* machine is the wrong instrument twice over: it exposes the
    machine to fork PRs, and it puts CI on a host with a live Homebrew at
    `/usr/local`. Keep it as a documented emergency option only, gated to
    non-fork events, and prefer hosted `macos-*-intel` for as long as it exists.

### Recommended CI matrix for `.github/workflows/`

Written as the shape of the job, not as final YAML. All labels are pinned; none
is `macos-latest` (finding 3). Every macOS job sets
`HOMEBREW_NO_AUTO_UPDATE=1`, `HOMEBREW_NO_ANALYTICS=1`,
`HOMEBREW_NO_ENV_HINTS=1` at the job level (finding 10a).

| Job | `runs-on` | Arch | What it asserts | Cost on a public repo |
|---|---|---|---|---|
| `validate` (exists today) | `ubuntu-latest` | x64 | Docs links, hook validity, repository gate, LICENSE hash | free |
| `macos-arm64` | `macos-26` | arm64 | fmt, clippy, layering, i18n parity, unit + contract tests, coverage floors; `cargo build --release --target aarch64-apple-darwin` with `MACOSX_DEPLOYMENT_TARGET=11.0`; **`vtool -show-build` ⇒ `minos` ≤ 11.0** | free (standard) |
| `macos-intel` | `macos-26-intel` | x86_64 | Same test suite natively on Intel; `cargo build --release --target x86_64-apple-darwin` with `MACOSX_DEPLOYMENT_TARGET=10.12`; **`vtool -show-build` ⇒ `minos` ≤ 10.12** | free (standard) |
| `macos-intel-oldest` | `macos-15-intel` | x86_64 | Same as above on the oldest hosted macOS, so "tested down to macOS 15 on Intel" is a claim CI backs | free (standard) |
| `macos-arm64-oldest` | `macos-15` | arm64 | Same on the oldest hosted arm64 | free (standard) |
| `cross-check` (in `macos-arm64`, extra step) | `macos-26` | — | `rustup target add x86_64-apple-darwin`; cross-build; assert `minos` ≤ 10.12 on the cross artifact; optionally run the test binary under `arch -x86_64` as a Rosetta smoke check that is allowed to be skipped when `pgrep oahd` fails | free |
| `support-matrix` | `ubuntu-latest` | — | Generates the tested/best-effort/unsupported table **from the job list above**, and fails if the committed table disagrees | free |

Deliberately excluded: `macos-14*` (unsupported from 2026-11-02, E-9);
`macos-*-large` / `-xlarge` (charged even on public repos, finding 4);
`macos-latest` (moving target, finding 3); any self-hosted runner (finding 11).

Support-matrix rows this justifies:

- **tested**: macOS 15 and macOS 26, on both `x86_64` and `arm64`.
- **best-effort**: macOS 11.0–14 on both architectures (deployment floor met and
  cross-checked mechanically, but no CI executes there); macOS 10.12–10.15 on
  `x86_64` (floor met, no hosted path, finding 8).
- **unsupported**: macOS below 10.12 on `x86_64`; below 11.0 on `arm64`.

## Cannot determine

- **Whether `GITHUB_TOKEN` can push to `<repo>.wiki.git`.** There is no `wiki`
  permission scope (E-16), and the docs do not state whether `contents: write`
  extends to the wiki repository. `UNRESOLVED`. What would settle it: GitHub
  documentation that names the wiki under automatic token authentication, or an
  observed push from an Actions job — which this project cannot observe until it
  has a remote (DP-007). Until then, assume a PAT or deploy key is required and
  treat wiki automation as blocked on that decision.
- **The exact retirement date of hosted Intel macOS.** "Fall 2027" is stated for
  the macOS 15 image; `macos-26-intel`'s existence makes the scope of that
  sentence unclear (finding 9). No GitHub document read on 2026-09-02 gives a
  retirement date for `macos-26-intel`.
- **Whether `macos-15-intel` will outlive macOS 15's own N-1 rotation.** By the
  policy in E-9, macOS 15 becomes the "N-1" image once macOS 27 ships; its
  deprecation issue does not exist yet.
- **Runner behaviour under load** — queue times for `-intel` labels, and whether
  Rosetta remains installed on macOS 27 arm64 images once Apple removes it for
  ordinary apps (E-13). Both are observable only by running CI.
- **Anything about `rustup`/`cargo` observed on this machine**: no Rust
  toolchain is installed here (E-14), so all Rust claims in this note are
  citations, not observations.

## Impact on the specification

Nothing in `docs/spec/` is *contradicted*, but two statements are now weaker
than the evidence allows and one is stronger:

1. `docs/spec/07-quality-and-testing.md`, CI section: *"Hosted Intel macOS
   runner availability is uncertain and must be verified, not assumed. If
   unavailable, cross-compile and validate behaviour against Intel fixtures"* —
   **verified: available** (finding 1). The MUST is satisfied and the
   contingency clause is not triggered. The sentence should be updated to record
   the answer and its expiry (Fall 2027 at the earliest, finding 9) rather than
   left as an open uncertainty. This is a `docs/spec-evolution.md` candidate.
2. `docs/adr/0002-implementation-language.md`, "What the foundation pass must
   still verify": *"No hosted CI runner for macOS 10.12–10.15 is known (R-10,
   open; treated as absent until answered)"* — **confirmed and now closed**
   (finding 8), and the reason is GitHub's N-1 image policy, not an absence of
   information. The ADR item can be marked answered by this note; the
   best-effort publication requirement it imposes stands unchanged.
3. `.github/workflows/repository-validation.yml`'s header comment states the
   macOS build-and-test job "lands with the research". That condition is now
   met; the matrix above is the input. No workflow edit is made from this note.
4. New, not currently in `docs/spec/`: **the runner images ship Homebrew and not
   MacPorts** (finding 10). `07-quality-and-testing.md`'s fixture rule already
   covers the correct behaviour, but the *reason it must be enforced in CI* —
   a live Homebrew is present on every macOS runner and will auto-update — is
   not written down anywhere and is exactly the kind of cross-provider
   contamination the definition of done asks about. Worth a sentence.
5. Dated policy data in the INV-20 sense, to be held as such: Apple's
   2026-09-01 Rosetta end-of-life (E-13) and GitHub's "no x86_64 macOS after
   macOS 15 retires in Fall 2027" (E-6). Both bear on how long the Intel half of
   this project's reason to exist can be *tested* rather than merely claimed.

## Re-check when

- **Hard trigger — before Fall 2027**, and immediately if a `github.blog`
  changelog announces a retirement date for `macos-26-intel` or for x86_64 macOS
  runners generally. Findings 1, 2 and 9 expire then and the whole matrix
  changes.
- **2026-11-02**: `macos-14*` becomes fully unsupported (E-9). Confirms the
  matrix's exclusion; no action expected.
- **When macOS 27 ships (autumn 2026 per E-13)**: check whether a `macos-27` /
  `macos-27-intel` pair appears, whether `macos-15` enters deprecation, and
  whether the arm64 images still install Rosetta.
- **If this project moves to a private repository**: finding 4's cost analysis
  changes from "free" to $0.062/min against a 2,000–3,000 minute allowance.
- Any time the `actions/runner-images` README's macOS rows change shape, or the
  GitHub-hosted-runners reference table moves URL again (it moved between the
  URL named in the task and the one cited in E-4).

## Non-blocking items answered here

### GitHub Wiki publishing workflows from a repository directory

A GitHub wiki **is a Git repository**, cloned as
`https://github.com/OWNER/REPO.wiki.git`; only pushes to its default branch go
live (E-16). So "publishing `docs/wiki/` from the main repository" is
mechanically: check out the main repo, check out the wiki repo, copy the
directory over, commit, push. There is no first-party GitHub Action for it, and
**there is no `wiki` permission scope** for `GITHUB_TOKEN` (E-16) — whether
`contents: write` suffices is `UNRESOLVED` above, so plan on a PAT or deploy
key.

Constraints that shape the source directory: the **filename becomes the page
title** and the extension selects the renderer (`*.md` → Markdown); the
characters `\ / : * ? " < > |` must not appear in page titles; `Home.md` is the
landing page. Two consequences for this project: (a) wiki page names cannot
carry the `docs/spec/NN-topic.md` numbering scheme unchanged if a colon is ever
used, and directory nesting does not survive as hierarchy — a generator must
flatten and rename deliberately; (b) wikis are only indexed by search engines
for repositories with 500+ stars *and* public editing disabled (E-16), so if
discoverability matters, GitHub Pages is the better target and the wiki is for
contributor-facing material. Default recommendation: keep `docs/` canonical in
the repository, and treat any wiki as a generated mirror published by an
explicitly-triggered workflow, never hand-edited — otherwise the wiki becomes a
second source of truth, which is the failure mode `docs/spec/` is organised to
avoid.

### GitHub topic conventions

Up to **20 topics**, **lowercase letters, numbers and hyphens only**, **50
characters or less** each, describing purpose, subject area, community or
language (E-17). GitHub suggests topics for public repositories, which
maintainers accept or reject; featured topics are curated via `github/explore`.
Applied to this project, a set that satisfies the rules and matches how the
audience actually searches: `macos`, `homebrew`, `macports`, `package-manager`,
`rust`, `cli`, `tui`, `migration`, `apple-silicon`, `intel`, `x86-64`,
`developer-tools`. Note `x86-64` rather than `x86_64` — underscores are outside
the allowed character set.
