# F-1 — What are Homebrew's current support tiers, and does the Intel/Tier 3 finding still hold?

- **Status:** answered — with one material correction to the pre-seeded wording
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (build 25G83), x86_64, Homebrew 6.0.21 @ `/usr/local`,
  MacPorts not installed
- **Blocking:** `docs/spec/00-charter.md` (why the project exists);
  `docs/spec/01-invariants.md` INV-17 (peer providers) and INV-20 (policy data,
  not code); `docs/spec/09-research-checklist.md` F-1; whatever policy-data file
  the Foundation Pass creates for support tiers.

## Question

Re-verify, on 2026-09-02, the pre-seeded finding recorded in
`09-research-checklist.md` F-1: that Homebrew classifies Intel x86_64 macOS as
Tier 3, that it has stopped building new bottles for Intel, and that it will
remove the ability to run Homebrew on Intel in or after September 2027. Also
record the stated macOS version requirement. These are dated policy facts and
INV-20 requires them held as data with source and date.

## Evidence

### E1 — `docs/Support-Tiers.md`, read 2026-09-02

Rendered page: <https://docs.brew.sh/Support-Tiers>, read 2026-09-02.
Source of that page, read verbatim 2026-09-02:
<https://raw.githubusercontent.com/Homebrew/brew/master/docs/Support-Tiers.md>

Front matter, verbatim:

```
---
last_review_date: "2026-08-27"
---
```

Tier definitions, verbatim (opening sentence of each):

> ## Tier 1
>
> A Tier 1 configuration is considered fully supported. These configurations
> receive the highest level of CI coverage and are prioritised during issue
> review and formula development.

> ## Tier 2
>
> A Tier 2 configuration is not fully supported. These configurations are
> outside the scope of complete CI coverage and may not consistently function
> as expected.

> ## Tier 3
>
> A Tier 3 configuration is not supported. These configurations fall far
> outside Homebrew's testing infrastructure and may fail to function reliably,
> even if basic installation is possible.

Tier 1 macOS qualification list, verbatim:

> To qualify as Tier 1, a macOS configuration must meet all of the following:
>
> - On official Apple Silicon hardware (not a virtual machine)
> - Running the latest patch release of a macOS version supported by Apple for
>   that hardware and included in Homebrew's CI coverage (typically the latest
>   stable or prerelease version and the two preceding versions)
> - Installed in the default prefix: `/opt/homebrew`
> - Not building official packages from source (i.e. using bottles)
> - Installed on the Mac's internal storage (not external or removable drives)
> - Running with `sudo` access available
> - Xcode Command Line Tools installed and up to date

Tier 2 membership list, verbatim:

> Tier 2 configurations include:
>
> - macOS prerelease versions before they are promoted to Tier 1
> - macOS systems with outdated versions of Xcode Command Line Tools
> - Linux systems with `glibc` versions between 2.13 and 2.38 (Homebrew's own
>   `glibc` formula will be installed automatically)
> - Homebrew installed outside the default prefix, requiring source builds for
>   official packages (i.e. installing outside `/opt/homebrew`, `/usr/local` or
>   `/home/linuxbrew/.linuxbrew`)
> - Architectures not yet officially supported by Homebrew

Tier 3 consequences and membership, verbatim:

> The following conditions typically apply:
>
> - Homebrew may work, but with a poor and unstable experience
> - Migration to a Tier 1 or 2 configuration or to a non-Homebrew tool is
>   strongly recommended
> - Pull requests must meet a very high bar: they must resolve an issue (not
>   merely work around it) and must not introduce high ongoing maintenance cost
>   (e.g. patches must already be merged upstream)
> - Homebrew maintainers do not commit to fixing bugs affecting these systems
> - Functionality may regress intentionally if it benefits supported
>   configurations
> - Loud configuration warnings will be printed at runtime
> - CI coverage is unavailable; bottles will rarely be built or published
> - Issues affecting only these configurations may be closed without response
> - Support is limited to community replies via Homebrew's Discussions
>
> Tier 3 configurations include:
>
> - macOS versions no longer covered by CI and no longer receiving regular
>   Apple security updates
> - Systems that build official packages from source despite available bottles
> - Homebrew installed outside the default prefix (e.g. `/opt/homebrew`,
>   `/usr/local` or `/home/linuxbrew/.linuxbrew` used on mismatched
>   architectures)
> - Homebrew installations managed by Nix (e.g. nix-darwin or nix-homebrew)
> - Installing formulae using `--HEAD`
> - Installing deprecated or disabled formulae
> - Intel x86_64 systems running macOS

Unsupported, verbatim:

> An unsupported configuration is one in which:
>
> - Homebrew will not run without third-party patches or modifications
> - Migration to another tool is required (e.g. [MacPorts](https://www.macports.org),
>   [Tigerbrew](https://github.com/mistydemeo/tigerbrew) or a native Linux
>   package manager)
>
> Unsupported configurations include:
>
> - FreeBSD
> - macOS Mojave 10.14 and earlier
> - Multi-user Homebrew environments where multiple users share the same
>   installation
> - Beowulf clusters
> - Nokia 3210s
> - CPUs built inside of Minecraft
> - Toasters

"Future macOS support", verbatim:

> macOS Tahoe 26 is the final version of macOS to run on Intel hardware. In
> alignment with this change, Homebrew has stopped building new bottles for
> Intel systems, and will remove the ability to run Homebrew on Intel systems
> in or after September 2027.
>
> The following timeline outlines expected Tier classifications based on
> Apple's release cycle and Homebrew's CI coverage.
>
> - As of September 2026:
>
>   Apple Silicon:
>   - Tier 1: macOS Tahoe 26, Sequoia 15, Sonoma 14
>   - Tier 3: macOS Big Sur 11 through Ventura 13
>
>   Intel x86_64:
>   - Tier 3: macOS Catalina 10.15 through Tahoe 26
>   - Unsupported: macOS Mojave 10.14 and earlier
>
> - Expected in or after September 2026:
>
>   Apple Silicon:
>   - Tier 1: macOS 27, Tahoe 26, Sequoia 15
>   - Tier 3: macOS Big Sur 11 through Sonoma 14
>
>   Intel x86_64:
>   - Tier 3: macOS Big Sur 11 through Tahoe 26
>   - Unsupported: macOS Catalina 10.15 and earlier
>
> - Expected in or after September 2027:
>
>   Apple Silicon:
>   - Tier 1: macOS 28, 27, Tahoe 26
>   - Tier 3: macOS Monterey 12 through Sequoia 15
>   - Unsupported: macOS Big Sur 11
>
>   Intel x86_64:
>   - Unsupported: all macOS versions

Rosetta 2 subsection, verbatim:

> [Apple has also announced](https://developer.apple.com/documentation/apple-silicon/about-the-rosetta-translation-environment)
> that Rosetta 2 will remain available as a general-purpose compatibility tool
> through macOS 27, with only a narrower legacy-games-focused subset planned
> beyond that.
>
> This does not change the support tier of an otherwise supported Apple Silicon
> Mac, but it does shorten the expected support window for x86_64-only casks
> that rely on [`requires_rosetta`](https://docs.brew.sh/Cask-Cookbook#caveats-mini-dsl).

(The one substitution in the quotation above: Homebrew's source writes that
link as the repository-relative `Cask-Cookbook.md#caveats-mini-dsl`; it is
rewritten to its absolute `docs.brew.sh` form here so it does not read as a
link into this repository.)

Third-party taps, verbatim:

> Packages installed from third-party taps outside the Homebrew GitHub
> organization are unsupported by default.

### E2 — `docs/Installation.md`, read 2026-09-02

<https://docs.brew.sh/Installation> and
<https://raw.githubusercontent.com/Homebrew/brew/master/docs/Installation.md>,
both read 2026-09-02. Front matter: `last_review_date: "2026-08-27"`.

macOS requirements, verbatim:

> * An Apple Silicon CPU; using a 64-bit Intel CPU is a Tier 3 configuration
> * macOS Sonoma (14) (or higher) installed on officially supported hardware
> * Command Line Tools (CLT) for Xcode … or Xcode
> * The Bourne-again shell for installation (i.e. `bash`)

On the prefix, verbatim:

> The script installs Homebrew to its default prefix (`/opt/homebrew` for Apple
> Silicon, `/usr/local` for macOS Intel and `/home/linuxbrew/.linuxbrew` for
> Linux) so that you don't need *sudo* after Homebrew's initial installation
> when you `brew install`. This prefix is required for most bottles (binary
> packages) to be used.

### E3 — observed contradiction of "has stopped building new bottles for Intel"

Machine environment for every command below:
`HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1`.
Local `homebrew/core` checkout at
`/usr/local/Homebrew/Library/Taps/homebrew/homebrew-core`, HEAD
`e045d7385fd7ad8907621300224f8a860bdd7bbb`, last commit 2026-09-02 (from
`brew config`, run 2026-09-02 12:37 UTC).

```
$ cd /usr/local/Homebrew/Library/Taps/homebrew/homebrew-core
$ git log -1 --format=%H -- Formula/h/htop.rb
e3a519a091bf08853a390b99c37a8528ea561576
$ git log -1 --format='%ad %s' --date=short e3a519a091bf08853a390b99c37a8528ea561576
2026-08-16 htop: update 3.5.3 bottle.
$ git show --format='' -U0 e3a519a091bf08853a390b99c37a8528ea561576 -- Formula/h/htop.rb
-    sha256 cellar: :any, arm64_tahoe:   "fcd2302f01490680101ec96b7338230e8e74c2061cea9139d8b50731ab44c874"
-    sha256 cellar: :any, arm64_sequoia: "1571d6aa264fb71ec23282651d005d02ae2c58be25c77f54a177498ce4953814"
-    sha256 cellar: :any, arm64_sonoma:  "6ea6d70385c51d699068066f2652d12d4b6166541ce6c3accb680c3a0627d399"
-    sha256 cellar: :any, sonoma:        "8c6d5453410472b39b5ab5951e5fc3d0da789ba29f73aa785cd429626605fff9"
-    sha256 cellar: :any, arm64_linux:   "8e6cf12deb2ff22ac67dcea4bffb61fee5e2c310fd941f906e317e45ebd6454a"
-    sha256 cellar: :any, x86_64_linux:  "94b1fbc2ed2be30a24ff5c46e80cc0c8b164c176f0a38709b92443ddcf7549a4"
+    sha256 cellar: :any, arm64_tahoe:   "d57bf20d0cbade0647877bbc74d736e743e050d2c0296b0591c78aa2c70ca516"
+    sha256 cellar: :any, arm64_sequoia: "c9f94a86905f32eb73ae3cbfbeef9bb269cdd723f0843a1146b8fb103973421d"
+    sha256 cellar: :any, arm64_sonoma:  "76274649e2fe8e65c53c85e8e540aaf759facc6f1629b8f9ff41200f6a8b2b4a"
+    sha256 cellar: :any, sonoma:        "598e42a8354e720c30f70d5713d977cf88386379710e7876f9e733ea132c8f98"
+    sha256 cellar: :any, arm64_linux:   "5421cf57c0c0ae15422d78a55db4abf72569edad65ebae619e1d652f9e697b46"
+    sha256 cellar: :any, x86_64_linux:  "781f9ced3961360377b636cfaa939804b87356364fe928e374288d3e55b5faad"
```

`sonoma` (no `arm64_` prefix) is the Intel macOS 14 tag — see
`R-2-homebrew-artifact-availability.md` E4 for the naming rule from Homebrew's
own source. Its checksum changed in a commit dated 2026-08-16.

Two further Intel-tagged bottle updates, same method:

```
$ C=$(git log -1 --format=%H -- Formula/c/cmake.rb); git log -1 --format='%ad %s' --date=short $C
2026-08-26 cmake: update 4.4.3 bottle.
$ git show --format='' -U0 $C -- Formula/c/cmake.rb | grep '^+.*sha256'
+    sha256 cellar: :any_skip_relocation, arm64_tahoe:   "0a53d2312ce16641029e616591e15dd256258401b401f5663d9560fddf78fc25"
+    sha256 cellar: :any_skip_relocation, arm64_sequoia: "49b8b85daec411080b7cfaaaed84846d1e08c366ddb27b199e61ff21cf881646"
+    sha256 cellar: :any_skip_relocation, arm64_sonoma:  "d50416765d0be66fba64b845e9198c9185a9b26a4baea54a50996c3a68b56238"
+    sha256 cellar: :any_skip_relocation, tahoe:         "9e321dc7e2f284ece5f048949ad98caeabc9c5fd853c90a1d2c930a33e0bf44e"
+    sha256 cellar: :any_skip_relocation, sequoia:       "0422f40c02b98e088ffe0a8f37c6b7efb24ed4f8832cb9efe0e53f97225e586f"
+    sha256 cellar: :any_skip_relocation, sonoma:        "f4176e173808ea4a59acb6182ec5bbb0be6fe3e452dbdfd0f8303a6377b13fee"

$ C=$(git log -1 --format=%H -- Formula/s/sqlite.rb); git log -1 --format='%ad %s' --date=short $C
2026-08-25 sqlite: update 3.53.4 bottle.
$ git show --format='' -U0 $C -- Formula/s/sqlite.rb | grep '^+.*sha256'
+    sha256 cellar: :any, arm64_tahoe:   "d09c4852f75e19c18c3e347ce73c81b0b864792046f079d074205522c735d3fc"
+    sha256 cellar: :any, arm64_sequoia: "4eefe9a700a6de4c31d20a16a2e675b51a24c87638d79fbc2195b952e7cb8d6a"
+    sha256 cellar: :any, arm64_sonoma:  "45049bbdee244bc9d91e22c36629b5ba93b719b52c4d121b86392b3410be630f"
+    sha256 cellar: :any, tahoe:         "58c8931be9f84faf36cd62062f161dcc7c70a104761a4d0753ef4398dd9f627f"
+    sha256 cellar: :any, sequoia:       "f6752e93c0ddf55d10ae3aaea01d280be9632855649caa87daf1371fa6e26eb4"
+    sha256 cellar: :any, sonoma:        "32f6a117203a602fe3e06f524c19e431965ecc0bf9ede4c11ab443551558cfe9"
```

`tahoe` and `sequoia` (no `arm64_` prefix) are the Intel macOS 26 and macOS 15
tags. Both appear in bottle-update commits dated 2026-08-25 and 2026-08-26.

### E4 — Intel bottle coverage census across the whole core tap

Full method, agreeing counts and cross-check against Homebrew's own API cache
are in `R-2-homebrew-artifact-availability.md` E5/E6. Summary of the observed
per-tag counts over all 8586 formula files in the local `homebrew/core`
checkout at HEAD `e045d738` on 2026-09-02:

```
7305	x86_64_linux
7279	arm64_linux
7277	arm64_sonoma
7277	arm64_sequoia
7276	arm64_tahoe
6680	sonoma          <- Intel macOS 14
2004	ventura         <- Intel macOS 13
1123	all
  88	sequoia         <- Intel macOS 15
  79	tahoe           <- Intel macOS 26
```

Formulae with a usable bottle on this machine's tag (`tahoe`, Intel macOS 26,
counting the documented older-macOS fallback and `:all`): **7808 of 8586
(90.9%)**. On `arm64_tahoe`: **8420 of 8586 (98.1%)**.

## Findings

1. **The tier structure is unchanged in substance.** Three tiers plus
   "Unsupported"; the page's own `last_review_date` is 2026-08-27 (E1). The
   pre-seeded summary in `09-research-checklist.md` remains an accurate
   *summary* of the tier names and their meanings.

2. **"Intel x86_64 systems running macOS" is still listed under Tier 3
   verbatim** (E1). Confirmed.

3. **"will remove the ability to run Homebrew on Intel systems in or after
   September 2027" is still present verbatim, and the timeline still shows
   Intel x86_64 as "Unsupported: all macOS versions" expected in or after
   September 2027** (E1). Confirmed, unchanged.

4. **"Homebrew has stopped building new bottles for Intel systems" is Homebrew's
   stated policy, and as of 2026-09-02 it is not yet literally true of the
   artifacts in `homebrew/core`.** New Intel bottles were published in
   August 2026: `htop` 3.5.3 Intel `sonoma` on 2026-08-16, `sqlite` 3.53.4
   Intel `tahoe`/`sequoia`/`sonoma` on 2026-08-25, `cmake` 4.4.3 Intel
   `tahoe`/`sequoia`/`sonoma` on 2026-08-26 (E3). 6680 of 8586 formulae carry
   an Intel `sonoma` bottle at their current stable version (E4).

5. **The accurate characterisation of the Intel bottle supply on 2026-09-02 is
   "frozen at `sonoma` and eroding", not "stopped".** 6680 formulae have an
   Intel `sonoma` bottle against 7277 with `arm64_sonoma` — roughly 600
   formulae have already lost their Intel bottle. Only 79 formulae carry an
   Intel `tahoe` bottle (E4). Coverage on this machine is 90.9% against 98.1%
   on Apple Silicon (E4). `R-2` E7 records seven formulae on this machine whose
   older installed version was poured from a bottle and whose newer version was
   built from source — the erosion observed in a single Cellar.

6. **The stated minimum macOS is Sonoma (14) or higher, and the stated CPU
   requirement is Apple Silicon, with 64-bit Intel explicitly named a Tier 3
   configuration** (E2). This is a *stated requirement*, not an enforced floor:
   this machine runs Homebrew 6.0.21 on Intel macOS 26.6.2.

7. **Non-default prefix appears in both Tier 2 and Tier 3** (E1), with different
   qualifiers: Tier 2 for "installed outside the default prefix, requiring
   source builds"; Tier 3 for a default prefix "used on mismatched
   architectures". `/usr/local` is named as *a* default prefix in both the
   Tier 2 list and in `Installation.md` (E2), and is the correct default prefix
   for macOS Intel. This machine is therefore in the default prefix.

8. **Third-party taps are "unsupported by default"** (E1). Relevant to how
   MacPortsBrew reports confidence for packages outside `homebrew/core`.

9. **Rosetta 2 has its own, shorter clock**: general-purpose availability
   "through macOS 27" per Apple, cited by Homebrew (E1). This affects casks
   with `requires_rosetta`, not formulae, and does not change an Apple Silicon
   Mac's tier.

## Cannot determine

- Whether the 79 Intel `tahoe` and 88 Intel `sequoia` bottles are produced by a
  deliberately maintained subset of CI runners or are incidental. The observed
  membership is heavily weighted towards the build toolchain (`gcc`, `gcc@12`
  … `gcc@15`, `llvm@16`, `llvm@17`, `libgccjit`, `python@3.11`–`python@3.14`,
  `perl`, `autoconf`, `automake`, `m4`, `make`, `ninja`, `cmake`, `pkgconf`,
  `readline`, `sqlite`, `xz`, `zstd`, `openssl@3.5`), which *suggests* Homebrew
  keeps Intel bottles for what is needed to compile everything else from source
  on Intel — but that is an inference from the membership list, not a cited
  policy. Settling it needs a statement from Homebrew (an issue, a PR or a
  `homebrew/core` CI configuration file), which was not located today.
- Whether "in or after September 2027" will move. By construction this cannot
  be determined in advance; it is why this note is time-sensitive.

## Impact on the specification

- **Confirms** `docs/spec/00-charter.md` and `AGENTS.md`: Intel x86_64 macOS is
  Tier 3 and the September 2027 removal statement is current as of 2026-09-02.
  No change needed to the charter's framing.
- **Confirms INV-20**: this is dated policy data with a moving
  `last_review_date` (2026-08-27 today), and it must live in data with its
  source and read date, never in code.
- **Contradicts, in detail, the pre-seeded bullet in
  `docs/spec/09-research-checklist.md` F-1** that reads "Homebrew **has stopped
  building new bottles for Intel**". That sentence is a correct *quotation* of
  Homebrew's documentation but is a false statement about the artifacts as of
  2026-09-02 (Finding 4). The checklist presents it as fact, not as quotation.
  Recommended change: attribute it — "Homebrew *states* it has stopped building
  new bottles for Intel; observed 2026-09-02, Intel `sonoma` bottles are still
  being published and Intel coverage is 90.9% versus 98.1% on Apple Silicon
  (`R-2`)". This is a Tier 3/5 correction and needs a `docs/spec-evolution.md`
  entry. **Not edited from here.**
- **Bears on `docs/spec/02-domain-model.md`** (availability model): the product
  must not tell an Intel user "no bottle exists" when 90.9% of core still pours.
  The honest message is about *trend and erosion*, which is exactly what
  `R-2` gives a mechanism to compute per package.
- **New for the policy-data file**: the tier table has more Tier 3 triggers than
  Intel alone — `--HEAD` installs, deprecated/disabled formulae, source builds
  despite available bottles, Nix-managed installs, non-default prefixes. If
  MacPortsBrew reports "your Homebrew is Tier N", it must evaluate all of them,
  not just architecture.

## Re-check when

**Time-sensitive by nature. Re-verify before every release**, and specifically:

- whenever `last_review_date` in `docs/Support-Tiers.md` changes (2026-08-27
  today);
- before September 2026, September 2027 and any macOS 27 release, since the
  page's timeline is keyed to those dates;
- whenever the Intel bottle census in E4 is re-run and moves materially — the
  numbers, not the prose, are what the product should react to. Re-run the
  census with the script recorded in `R-2-homebrew-artifact-availability.md`.
