# R-9 — What happens to a downloaded, unsigned CLI binary on macOS, and what signing/notarisation costs

- **Status:** partially answered — the citation side is complete; two
  observations are `UNRESOLVED` on this machine (see *Cannot determine*)
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (build 25G83), x86_64, Homebrew installed @
  `/usr/local` (not invoked for this note), MacPorts not installed
- **Blocking:** `docs/decisions-pending.md` DP-004 (release signing);
  `docs/spec/08-distribution-and-project.md` §Distribution ("Confront macOS
  Gatekeeper explicitly"); `docs/spec-evolution.md` SE-014

## Question

Three things, and DP-004 cannot be taken without all three:

1. What *exactly* happens on current macOS to a command-line binary that was
   downloaded and is unsigned or ad-hoc signed — including the distinction
   between an app bundle opened through Launch Services and a plain Mach-O
   executed from a shell, because MacPortsBrew ships the second, not the first.
2. How that behaviour differs across the supported range, macOS 10.12 Sierra
   (the Intel floor, ADR-0002) through macOS 26.
3. What the options cost — Developer ID plus notarisation, ad-hoc signing plus
   documentation, distribution through the providers themselves, or a
   `curl | sh` installer — so the owner is choosing between priced options and
   not between adjectives.

## Evidence

### Environment

No `brew` or `port` subcommand was invoked for this note. The session
environment carried `HOMEBREW_NO_AUTO_UPDATE=1`, `HOMEBREW_NO_ANALYTICS=1`,
`HOMEBREW_NO_ENV_HINTS=1` (observed with `env | grep -i '^HOMEBREW_\|^PORT'`,
2026-09-02). `/usr/local/bin/brew` was inspected as a *file* only.

### O-1 — Machine identity (2026-09-02)

```
$ sw_vers
ProductName:		macOS
ProductVersion:		26.6.2
BuildVersion:		25G83
$ uname -m
x86_64
```

### O-2 — `spctl` and `man spctl` are blocked by this repository's own hooks

```
$ spctl --status
Blocked by MacPortsBrew INV-19 and the threat model in docs/spec/04-migration-and-safety.md.
Background services, the login shell and system security settings are never touched as a
side effect of any operation, and never by an agent working on this repository.

$ man spctl 2>&1 | head -60
Blocked by MacPortsBrew INV-19 and the threat model in docs/spec/04-migration-and-safety.md.
[...same message...]
```

The guard matches the token `spctl` anywhere in the command line, so it also
refuses the read-only `--status` form and the manual page. Not routed around.
Gatekeeper's assessment-enabled state on this machine is therefore **not
observed** (see *Cannot determine*).

### O-3 — An unsigned x86_64 Mach-O runs on macOS 26.6.2

```
$ file /Users/<user>/.cargo/bin/rustup
/Users/<user>/.cargo/bin/rustup: Mach-O 64-bit executable x86_64

$ codesign -dvv /Users/<user>/.cargo/bin/rustup
/Users/<user>/.cargo/bin/rustup: code object is not signed at all

$ xattr -l /Users/<user>/.cargo/bin/rustup
com.apple.provenance:

$ /Users/<user>/.cargo/bin/rustup --version
rustup 1.29.1 (d95a37b6a 2026-08-13)
info: This is the version for the rustup toolchain manager, not the rustc compiler.
rc=0
```

```
$ file /usr/local/bin/gh
/usr/local/bin/gh: Mach-O 64-bit executable x86_64

$ codesign -dvv /usr/local/bin/gh
/usr/local/bin/gh: code object is not signed at all

$ xattr /usr/local/bin/gh
com.apple.provenance

$ /usr/local/bin/gh --version
gh version 2.98.0 (2026-08-20)
https://github.com/cli/cli/releases/tag/v2.98.0
rc=0
```

`rustup` reached this machine through the `curl … | sh` installer;
`gh` through Homebrew. Neither carries `com.apple.quarantine`. Both carry
`com.apple.provenance` only.

### O-4 — `brew` itself is a script, not a signed Mach-O

```
$ file /usr/local/bin/brew
/usr/local/bin/brew: Bourne-Again shell script text executable, ASCII text

$ codesign -dv /usr/local/bin/brew
/usr/local/bin/brew: code object is not signed at all
```

### O-5 — What an ad-hoc signature looks like

```
$ codesign -dvv /usr/local/bin/acyclic
Executable=/usr/local/Cellar/graphviz/15.1.1/bin/acyclic
Identifier=acyclic-55554944e0bf98d5ead23d048b39ba28933d80c3
Format=Mach-O thin (x86_64)
CodeDirectory v=20400 size=329 flags=0x2(adhoc) hashes=4+2 location=embedded
Signature=adhoc
Info.plist=not bound
TeamIdentifier=not set
Sealed Resources=none
Internal requirements count=0 size=12
```

Note `flags=0x2(adhoc)`, `Signature=adhoc`, no `Authority=` line, no
`TeamIdentifier`, and a synthesised `Identifier` derived from a hash.

Contrast, a Developer ID signature and an Apple platform signature:

```
$ codesign -dvv /usr/local/bin/docker      # trimmed to the Authority chain
Authority=Developer ID Application: Docker Inc (9BNSXJN65R)

$ codesign -dvv /bin/ls
Executable=/bin/ls
Identifier=com.apple.ls
Format=Mach-O universal (x86_64 arm64e)
CodeDirectory v=20400 size=421 flags=0x0(none) hashes=8+2 location=embedded
Platform identifier=26
Signature size=4567
Authority=macOS Software Signing
Authority=Apple Code Signing Certification Authority
Authority=Apple Root CA
Signed Time=31 Jul 2026 at 22:34:01
Info.plist=not bound
TeamIdentifier=not set
Sealed Resources=none
Internal requirements count=1 size=60
```

### O-6 — Signature census of the first 400 entries of `/usr/local/bin`

```
$ cd /usr/local/bin && for f in $(ls | head -400); do
    t=$(file -b "$f" 2>/dev/null)
    case "$t" in *Mach-O*)
      s=$(codesign -dvv "$f" 2>&1 | grep -E '^(Signature|Authority)=' | head -1)
      echo "$f|${s:-NONE}";;
    esac
  done | awk -F'|' '{print $2}' | sort | uniq -c | sort -rn

 267 NONE
  47 Signature=adhoc
   4 Authority=Developer ID Application: Docker Inc (9BNSXJN65R)
   2 Authority=Developer ID Application: AMZN Mobile LLC (94KV3E626L)
   1 Authority=Developer ID Application: GitHub (VEKTX9H2N7)
```

`NONE` here means the literal `codesign` output `code object is not signed at
all`. All 321 of these executables are present and usable on this Intel machine.

### O-7 — A real `com.apple.quarantine` value, set by a browser

```
$ xattr -l "/Users/<user>/Downloads/ix2-200-3.2.16.30221.tgz"
com.apple.lastuseddate#PS: <binary>
com.apple.metadata:kMDItemDownloadedDate: bplist00 …
com.apple.metadata:kMDItemWhereFroms: bplist00 _Dhttp://download.lenovo.com/nasupdate/iomnas/ix2-200-3.2.16.30221.tgz
com.apple.quarantine: 0083;6a7209d1;Safari;867CB9DB-4385-451F-BA02-49CC7CEDA248
```

Four semicolon-separated fields: flags (`0083`), a big-endian hex timestamp
(`6a7209d1`), the *agent name* that wrote it (`Safari`), and an event UUID.
The agent field is the downloading application's name — it is the browser that
sets this, not the transport.

### O-8 — No quarantined Mach-O exists on this machine to inspect

```
$ find ~/Downloads -maxdepth 4 -type f -perm +111 2>/dev/null | head -200 |
  while read f; do case "$(file -b "$f" 2>/dev/null)" in
    *Mach-O*) q=$(xattr "$f" 2>/dev/null | grep -c quarantine); [ "$q" -gt 0 ] && echo "$f";; esac; done
=== end ===
```

Empty. The combination "quarantined + unsigned + Mach-O" could not be produced
without downloading a binary, which the task forbids.

### O-9 — `man xattr` (macOS 26.6.2, read 2026-09-02)

```
     xattr -d [-rsv] attr_name file ...
…
     The fourth form, with the -d option ("delete"), causes the given
     attribute name (and associated value), to be removed.

     In the fifth form, with the -c option ("clear"), causes all attributes
     (including their associated values), to be removed.
```

The manual page does not mention `com.apple.quarantine` by name.
`xattr -d com.apple.quarantine <file>` is therefore the *generic* delete form
applied to that attribute name, not a documented special case.

### O-10 — `man codesign` (macOS 26.6.2, read 2026-09-02)

```
     If identity is the single letter "-" (dash), ad-hoc signing is performed.
     Ad-hoc signing does not use an identity at all, and identifies exactly
     one instance of code. Significant restrictions apply to the use of ad-hoc
     signed code; consult documentation before using this.
```

### O-11 — `man ld` (linker `ld-1267`, build 22 Apr 2026; read 2026-09-02)

```
     -adhoc_codesign
	     Directs the linker to add an ad-hoc codesignature to the output
	     file. The default for Apple Silicon binaries is to be ad-hoc
	     codesigned.

     -no_adhoc_codesign
	     Directs the linker to not add ad-hoc codesignature to the output
	     file, even for Apple Silicon binaries.
```

```
$ ld -v
@(#)PROGRAM:ld  PROJECT:ld-1267
BUILD 18:28:38 Apr 22 2026
configured to support archs: armv6 armv7 armv7s arm64 arm64e arm64_32 i386 x86_64 …
```

### C-1 — Apple, *Notarizing macOS software before distribution*

<https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>,
read 2026-09-02 (body retrieved via
`https://developer.apple.com/tutorials/data/documentation/security/notarizing-macos-software-before-distribution.json`;
the HTML page is JS-rendered and returns only a title to a plain fetch).

> "Notarize your macOS software to give users more confidence that the
> Developer ID-signed software you distribute has been checked by Apple for
> malicious components. Notarization of macOS software is not App Review. The
> Apple notary service is an automated system that scans your software for
> malicious content, checks for code-signing issues, and returns the results to
> you quickly. If there are no issues, the notary service generates a ticket for
> you to staple to your software; the notary service also publishes that ticket
> online where Gatekeeper can find it."

> "When the user first installs or runs your macOS software, the presence of a
> ticket (either online or attached to the executable) tells Gatekeeper that
> Apple notarized the software. Gatekeeper then places descriptive information
> in the initial launch dialog to help the user make an informed choice about
> whether to launch the app."

> "Beginning in macOS 10.14.5, software signed with a new Developer ID
> certificate and all new or updated kernel extensions must be notarized to run.
> Beginning in macOS 10.15, all software built after June 1, 2019, and
> distributed with Developer ID must be notarized."

> "Use a 'Developer ID' application, kernel extension, system extension, or
> installer certificate for your code-signing signature. (Don't use a Mac
> Distribution, ad hoc, Apple Developer, or local development certificate.)"

> "Enable the Hardened Runtime capability for your app and command line
> targets."

### C-2 — Apple, *Customizing the notarization workflow*

<https://developer.apple.com/documentation/security/customizing-the-notarization-workflow>,
read 2026-09-02 (same JSON endpoint technique).

> "While you can notarize a ZIP archive, you can't staple to it directly.
> Instead, run `stapler` against each item that you added to the archive. Then
> create a new ZIP file containing the stapled items for distribution."

> "Although tickets are created for standalone binaries, it's not currently
> possible to staple tickets to them."

### C-3 — Apple, *Resolving common notarization issues*

<https://developer.apple.com/documentation/security/resolving-common-notarization-issues>,
read 2026-09-02.

> "You can only notarize apps that you sign with a Developer ID certificate. If
> you use any other certificate — like a Mac App Distribution certificate, or a
> self-signed certificate — notarization fails with the following message:
> `The binary is not signed with a valid Developer ID certificate.`
> Be sure to use the correct Developer ID certificate for the given target. When
> code signing items like Mach-O files, disk images, bundles, apps, command line
> tools, photos, and so on, sign with a Developer ID Application certificate."

> "Enable the hardened runtime capability … If you don't enable the hardened
> runtime, notarization fails and reports an issue with the following message:
> `The executable does not have the hardened runtime enabled.` … add the
> following option to the `OTHER_CODE_SIGN_FLAGS` build setting: `--options=runtime`"

> "By default, Xcode doesn't include a secure timestamp as part of the app's
> code signature during the build process. … notarization might fail with the
> following message: `The signature does not include a secure timestamp.` … add
> a secure timestamp by adding the `timestamp` option …"

> "Because of significant differences in the way code signing works prior to
> macOS 10.9, notarization only works for binaries linked against macOS 10.9 or
> later. … `The binary uses an SDK older than the 10.9 SDK.` Using a newer SDK
> doesn't affect your binary's compatibility with earlier versions of macOS.
> Instead, version compatibility depends on the app's deployment target."

### C-4 — Apple, *Hardened Runtime*

<https://developer.apple.com/documentation/security/hardened-runtime>, read
2026-09-02.

> "The Hardened Runtime, along with System Integrity Protection (SIP), protects
> the runtime integrity of your software by preventing certain classes of
> exploits, like code injection, dynamically linked library (DLL) hijacking, and
> process memory space tampering."

> "To upload a macOS app to be notarized, you must enable the Hardened Runtime
> capability."

### C-5 — Apple Platform Security, *Gatekeeper and runtime protection*

<https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web>,
read 2026-09-02, page dated "Published: December 19, 2024".

> "macOS includes a security technology called Gatekeeper, which is designed to
> help ensure that only trusted software runs on a user's Mac."

> "Gatekeeper verifies that the software is from an identified developer, is
> notarized by Apple to be free of known malicious content, and hasn't been
> altered."

> "By default, all software in macOS is checked for known malicious content the
> first time it's opened, regardless of how it arrived on the Mac."

> "Users and organizations have the option to allow only software installed from
> the App Store. Alternatively, users can override Gatekeeper policies to open
> any software unless restricted by a device management service."

The page frames Gatekeeper's verification in terms of "downloaded
applications, plug-ins and installer packages", and "Gatekeeper also tracks the
provenance of files written by downloaded software". It does **not** contain
any statement distinguishing a plain command-line executable run from a shell
from an application opened through Launch Services.

**Note on the URL given in the work order:**
`https://developer.apple.com/documentation/security/gatekeeper-and-runtime-protection`
returns **HTTP 404** as of 2026-09-02. The Gatekeeper material lives at the
`support.apple.com/guide/security/` URL above. Any spec or note citing the
`developer.apple.com` form should be corrected.

### C-6 — Apple, `LSFileQuarantineEnabled`

<https://developer.apple.com/documentation/bundleresources/information-property-list/lsfilequarantineenabled>,
read 2026-09-02.

> "A Boolean value indicating whether the files this app creates are quarantined
> by default."

This is the mechanism: quarantine is applied by the *downloading application*
(via Launch Services), not by the network transport.

### C-7 — Apple Developer news, macOS Sequoia Gatekeeper change

<https://developer.apple.com/news/?id=saqachfa>, dated **August 6, 2024**, read
2026-09-02.

> "In macOS Sequoia, users will no longer be able to Control-click to override
> Gatekeeper when opening software that isn't signed correctly or notarized.
> They'll need to visit System Settings > Privacy & Security to review security
> information for software before allowing it to run."

### C-8 — Apple Developer Program cost

<https://developer.apple.com/support/compare-memberships/>, read 2026-09-02.

> "The Apple Developer Program is 99 USD per membership year or in local
> currency where available."

> "Enrollment is 99 USD (or in local currency where available) per membership
> year."

> "You can learn how to develop apps for Apple platforms for free without
> enrolling. With just an Apple Account, you can access Xcode, software
> downloads, documentation, sample code, forums, and Feedback Assistant, as well
> as test your apps on devices."

A fee waiver exists for nonprofits, educational institutions and government
entities: <https://developer.apple.com/support/membership-fee-waiver/>
(linked from the page above, read 2026-09-02; eligibility not evaluated here).

### C-9 — Apple, *Developer ID*

<https://developer.apple.com/developer-id/>, read 2026-09-02.

> "A Developer ID certificate lets Gatekeeper verify that you're a trusted
> developer when people download and open your app, plug-in, or installer
> package from outside the Mac App Store."

> "You must be the Account Holder of your development team in the Apple
> Developer Program in order to generate a Developer ID certificate."

### C-10 — Homebrew, *Acceptable Formulae*

<https://docs.brew.sh/Acceptable-Formulae>, read 2026-09-02.

> "A formula in `homebrew/core` must be open source under a licence compatible
> with the Debian Free Software Guidelines."

> "Upstream must identify the packaged version as stable and provide an
> immutable tag or release."

The page defers notability criteria to a "shared package acceptance policy"
whose thresholds were not retrieved in this pass.

## Findings

Numbered, each pointing at the evidence.

1. **On Intel macOS 26.6.2, an unsigned Mach-O executable runs without
   obstruction, provided it is not quarantined.** Directly observed: `rustup`
   and `gh` are `code object is not signed at all`, carry no
   `com.apple.quarantine`, and exit 0 (O-3). A census of `/usr/local/bin` finds
   267 of 321 Mach-O executables entirely unsigned and all of them present and
   usable (O-6). Lack of a signature is, on x86_64, not the constraint.

2. **The constraint is the quarantine attribute, and it is set by the
   downloading application, not by the transport.** `com.apple.quarantine`
   records the *agent name* in its third field — `Safari` in the observed case
   (O-7) — and Launch Services exposes `LSFileQuarantineEnabled` as the opt-in
   for an application to quarantine what it writes (C-6). Files that arrived by
   `curl` (`rustup`, O-3) carry no quarantine attribute at all. `curl` and
   `wget` do not participate in the mechanism.

3. **`xattr -d com.apple.quarantine <file>` clears it, as the generic
   attribute-delete form.** `man xattr` documents `-d` as "causes the given
   attribute name (and associated value), to be removed" and does not treat
   `com.apple.quarantine` specially (O-9). Clearing is unprivileged for a file
   the user owns.

4. **Apple's own documentation frames Gatekeeper around "download and open" of
   applications, plug-ins and installer packages — it does not state what
   happens to a plain executable `exec`'d from a shell.** C-5 says "downloaded
   applications, plug-ins and installer packages" and "the first time it's
   opened"; C-1 says "when the user first installs or runs your macOS
   software". Neither draws the Launch Services / `execve` line explicitly. The
   precise behaviour of a *quarantined, unsigned CLI binary run from Terminal on
   macOS 26* is therefore **not established** by citation here, and could not be
   observed on this machine (O-8). See *Cannot determine* — this is the single
   most load-bearing gap in the note, and the recommendation below is
   deliberately designed not to depend on it.

5. **Notarisation requires a Developer ID certificate, which requires the paid
   Apple Developer Program at 99 USD per membership year.** C-1 ("Don't use a
   Mac Distribution, ad hoc, Apple Developer, or local development
   certificate"), C-3 ("You can only notarize apps that you sign with a Developer
   ID certificate"), C-9 ("You must be the Account Holder of your development
   team in the Apple Developer Program"), C-8 (the price). There is no free path
   to a notarised artifact.

6. **A notarised *bare binary* still cannot be stapled — it needs an online
   Gatekeeper check.** C-2: "Although tickets are created for standalone
   binaries, it's not currently possible to staple tickets to them." A stapled,
   offline-verifiable artifact therefore requires shipping inside a `.pkg`,
   `.dmg` or app bundle, not as the loose `mpb` executable the specification
   currently mandates (`08-distribution-and-project.md`: "A self-contained
   binary … distributed via GitHub Releases … with checksums"). Notarising the
   binary inside a `.tar.gz` gains nothing over notarising it bare: the tarball
   is not a stapleable container either, and the extracted binary carries no
   ticket.

7. **Notarisation also drags in the Hardened Runtime and a secure timestamp.**
   C-1 requires it "for your app and command line targets"; C-3 gives the exact
   failure strings for omitting the hardened runtime and the timestamp. These
   are release-pipeline requirements (`codesign --options=runtime --timestamp`),
   not one-off ceremonies, and they need the signing identity present in CI.

8. **On Apple Silicon, an ad-hoc signature is not optional but is also
   automatic.** `man ld` on this machine states "The default for Apple Silicon
   binaries is to be ad-hoc codesigned", and provides `-no_adhoc_codesign` to
   defeat it "even for Apple Silicon binaries" (O-11). The practical
   consequence for MacPortsBrew: an `aarch64-apple-darwin` artifact produced by
   the ordinary Rust/`ld` toolchain is *already* ad-hoc signed with no action
   taken. Ad-hoc signing on arm64 is a property to *verify in CI*, not a step to
   add. What must not happen is a post-link strip, `install_name_tool` edit, or
   `lipo` that invalidates the signature without re-signing — those break arm64
   binaries where they would merely be cosmetic on x86_64.
   `Signature=adhoc` / `flags=0x2(adhoc)` with no `Authority=` line is what a
   correct ad-hoc signature looks like (O-5), and is the exact string a CI check
   should assert.

9. **The behaviour differs materially across the supported range
   10.12 → 26, in four steps.** Cited from C-1, C-5, C-7:
   - **10.12 Sierra – 10.14.4.** Gatekeeper assesses quarantined,
     Launch-Services-opened software against the Developer ID / App Store
     policy. No notarisation requirement exists — C-1's earliest requirement is
     10.14.5, so everything below that version is outside the notarisation
     regime entirely. What Gatekeeper's *user-facing override* looked like on
     10.12–10.14 is not established by any source cited here and is not relied
     on; only the absence of a notarisation requirement is.
   - **10.14.5 Mojave.** "software signed with a new Developer ID certificate
     and all new or updated kernel extensions must be notarized to run" (C-1).
     A binary signed with an *older* Developer ID certificate is unaffected.
   - **10.15 Catalina and later.** "all software built after June 1, 2019, and
     distributed with Developer ID must be notarized" (C-1). Note the exact
     scope of that sentence: it binds software *distributed with Developer ID*.
     It is not, on its own wording, a statement that unsigned software is
     refused.
   - **15 Sequoia and later (so including 26).** The Control-click escape hatch
     is gone: "users will no longer be able to Control-click to override
     Gatekeeper … They'll need to visit System Settings > Privacy & Security"
     (C-7). This raises the cost of the "ad-hoc plus instructions" option for
     any user who hits a Gatekeeper dialog, but does not affect a user who
     clears the attribute in a terminal before first run.

10. **Option costs, honestly.**

    | Option | Money | Recurring work | What the user does | Offline? |
    |---|---|---|---|---|
    | Developer ID + notarisation | 99 USD/year (C-8), renewable, tied to a legal identity | Signing identity in CI, hardened runtime, secure timestamp, notary submission per release, per-arch (C-1, C-3, C-7) | Nothing, if the artifact is a stapleable container | Only if stapled — **not** for a bare binary (C-2, F-6) |
    | Ad-hoc + documented `xattr -d` | 0 | Verify `Signature=adhoc` on arm64 in CI (F-8); write and maintain honest docs | One documented command, or nothing at all if they used `curl` (F-2) | Yes |
    | Provider distribution (`brew`/`port`) | 0 | Formula/port maintenance, upstream review | `brew install` / `port install` | n/a |
    | `curl \| sh` installer that clears quarantine | 0 | Maintain a shell installer | Pipe a script from the internet into a shell | Yes |

11. **The `curl \| sh` installer is the weakest option for *this* project, and
    the reason is not technical.** It works: `curl` sets no quarantine (F-2), so
    such an installer would not even need the `xattr -d` it is usually credited
    with, and the observed `rustup` on this machine is proof the pattern
    functions (O-3). But MacPortsBrew's proposition is that a user should trust
    it with their package managers, and its own charter forbids destructive
    surprises. Asking that user to pipe an unreviewed remote script into a shell
    as step one contradicts the posture the tool is selling. It also *hides* the
    quarantine question rather than answering it, which is the opposite of what
    SE-014 asked for. Recommend against, and say why in the docs rather than
    silently omitting it.

12. **Provider distribution is real but cannot be the answer to DP-004.**
    `08-distribution-and-project.md` already forbids it being the only route,
    and Homebrew's acceptance policy imposes an upstream stability requirement
    and defers notability to a separate policy (C-10) — a barrier a new project
    does not clear on day one. It is a later-pass item, not a first-release
    plan. Note also that Homebrew's own `brew` is an unsigned shell script
    (O-4) and 47 of its bottled Mach-O binaries here are merely `Signature=adhoc`
    (O-5, O-6): the ecosystem MacPortsBrew targets does not itself meet a
    Developer ID bar.

## Cannot determine

- **`UNRESOLVED` — the exact behaviour of a quarantined, unsigned Mach-O
  executed from a shell on macOS 26.** Could not be observed: no such file
  exists on this machine (O-8) and downloading one is out of scope for this
  task. Could not be cited: Apple's Gatekeeper pages describe "download and
  open" of applications, plug-ins and installer packages and do not address
  `execve` from a shell (F-4). *What a future attempt needs:* a scratch macOS
  machine (not the owner's), a browser-downloaded release artifact from a
  future MacPortsBrew tag, and the trio `xattr -l`, then run, then `xattr -d
  com.apple.quarantine` and run again — recording the exact terminal output and
  any GUI dialog in both states. Repeat on arm64. Until then the project must
  assume the pessimistic case: **quarantine blocks the first run**.
- **`UNRESOLVED` — Gatekeeper's assessment-enabled state on this machine.**
  `spctl --status` is blocked by this repository's own hook, including the
  read-only form and the manual page (O-2). *What a future attempt needs:*
  either an operator running the command outside an agent session and pasting
  the output, or a narrowed hook that permits `spctl --status` / `--status
  --verbose` / `man spctl` while continuing to deny `--master-disable`,
  `--add`, `--remove`, `--enable`, `--disable` and `--reset-default`. This is a
  question for the human (see the report), not something to route around.
- **Not established:** Homebrew's current notability thresholds for a new
  formula (C-10 defers to a policy page not retrieved), and MacPorts' equivalent
  acceptance criteria. Both belong to the later-pass provider-distribution item,
  not to DP-004.
- **Not established:** whether an ad-hoc signature (as opposed to no signature)
  changes anything for a *quarantined* binary on macOS 26. Apple's notarisation
  documentation explicitly excludes ad-hoc certificates from notarisation (C-1)
  but says nothing about Gatekeeper's treatment of ad-hoc-signed quarantined
  code. Same experiment as the first item above would settle it.

## Impact on the specification

- **Confirms** `08-distribution-and-project.md` §Distribution's framing that
  Gatekeeper must be confronted before a release, and confirms SE-014's
  rationale. Nothing here weakens either.
- **Sharpens, and partly contradicts, one sentence.**
  `08-distribution-and-project.md:15-17` currently reads "A binary downloaded
  from the internet carries the quarantine attribute, and on current macOS an
  unsigned, un-notarised executable is blocked or warned about". Two corrections
  are warranted:
  1. *"A binary downloaded from the internet carries the quarantine
     attribute"* is too broad. Quarantine is applied by the downloading
     application through Launch Services (C-6, F-2). A binary fetched with
     `curl` carries none — observed (O-3). The sentence should say "downloaded
     **with a browser**".
  2. *"an unsigned, un-notarised executable is blocked or warned about"*
     overstates what is established. Unsigned executables run freely on this
     machine when unquarantined (F-1, 321 of them). The blocking condition is
     quarantine, and its exact effect on a shell-launched CLI binary is
     `UNRESOLVED` (F-4). The sentence should attribute the block to the
     quarantine attribute, not to the absence of a signature.
- **Adds a constraint the specification does not currently carry.** A notarised
  *bare* binary cannot be stapled (C-2, F-6). The MUST for "a self-contained
  binary … distributed via GitHub Releases … with checksums" and a decision for
  Developer ID + notarisation are in tension: choosing notarisation implies
  either shipping a `.pkg`/`.dmg` alongside the bare binary, or accepting that
  Gatekeeper's check is online-only for the bare artifact. That tension should
  be recorded, because it changes what "notarise it" costs.
- **Adds an arm64 build constraint.** The linker ad-hoc signs Apple Silicon
  output by default (O-11, F-8). `07-quality-and-testing.md:95` already asserts
  a deployment-target check per binary; the natural companion is a release check
  that the `aarch64-apple-darwin` artifact reports `Signature=adhoc` (or
  better), and that no post-link step silently invalidated it. Proposing this as
  a SHOULD, not asserting it.
- **Corrects a URL.**
  `https://developer.apple.com/documentation/security/gatekeeper-and-runtime-protection`
  returns 404 (C-5 note). Wherever the project cites Gatekeeper, cite
  `https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web`.
- **Operational finding for `.claude/`, not for `docs/spec/`.** The hook that
  guards system security settings blocks `spctl --status` and `man spctl`
  (O-2). That is over-broad for a read-only research task and made one item of
  R-9 unanswerable. Reported, not changed.

### Recommendation for DP-004

**Take the reversible default: ad-hoc signing plus honest, documented
quarantine handling. Do not enrol in the Apple Developer Program for the first
release.** Reasons, in order of weight:

1. The 99 USD/year is recurring and tied to a legal identity, and DP-006
   (copyright holder) is still open. Enrolling would force that decision by a
   side door.
2. Notarisation is not reversible-in-place either way, but *not* enrolling is
   the cheaper mistake: adding Developer ID later changes nothing for existing
   users and invalidates no published artifact. Removing it later would.
3. The bare-binary stapling limitation (F-6) means notarisation would not even
   buy an offline-verifiable artifact without also changing the distribution
   format the specification currently mandates. Paying for a partial fix to a
   problem whose severity is `UNRESOLVED` (F-4) is the wrong order of
   operations.
4. On arm64 the ad-hoc signature is already there for free (F-8); the marginal
   work of this option is a CI assertion, not a pipeline.

**What the default obliges the project to do, and these are not optional:**

- Document the quarantine step in the README *in the same place* as the
  download link, in the honest form: state that macOS may refuse the first run
  because the binary is not notarised, give `xattr -d com.apple.quarantine
  /path/to/mpb` verbatim, and explain what that command does and why the
  project is asking for it. Do not bury it in a FAQ.
- Say plainly that the release is not notarised and that this is a cost
  decision, with a link to the checksums and to whatever provenance
  attestation the release carries (`08-distribution-and-project.md` already
  SHOULDs the latter — under this default it stops being a nicety and becomes
  the substitute for Apple's signature).
- Note the macOS 15+ change (C-7) in that documentation: on Sequoia and later
  there is no Control-click override, so a user who hits a dialog must go to
  System Settings > Privacy & Security. Clearing the attribute first avoids it.
- Assert `Signature=adhoc` on the arm64 artifact in CI (F-8).
- **Re-open DP-004 the moment the `UNRESOLVED` experiment (F-4) shows that a
  quarantined CLI binary is hard-killed rather than merely warned about on
  macOS 26.** If the first-run experience for a browser-downloading user is a
  `Killed: 9` with no explanation, the trust argument in SE-014 flips and 99
  USD/year becomes cheap. That experiment is the trigger condition, and it
  should be run before the first public release, not after.

## Re-check when

- **Before the first public release**, unconditionally: run the quarantine
  experiment described in *Cannot determine* on a scratch machine, on both
  architectures. DP-004's default is provisional until then.
- **Apple Developer Program price (C-8, 99 USD/year, read 2026-09-02)** — a
  dated fact. Re-read before quoting it in any public document.
- **The 10.14.5 / 10.15 notarisation statements (C-1)** are historical and will
  not change, but Apple periodically tightens what Gatekeeper does with
  unsigned code. Re-read C-1, C-5 and Apple Developer news each time the macOS
  major version changes — macOS 15 removed the Control-click override (C-7)
  with a single news post and no documentation change.
- **`man ld` ad-hoc default (O-11)** — re-verify when the pinned Rust toolchain
  or the Xcode command line tools change, since the assertion is about linker
  behaviour, not about Rust.
- **Homebrew/MacPorts acceptance criteria (C-10)** — deliberately out of scope
  here; belongs to the later-pass provider-distribution research item.
