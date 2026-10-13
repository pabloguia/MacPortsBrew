# R-19 — What the release workflow can rely on: notarisation, reproducible builds, deployment target, packaging, runners

- **Status:** partially answered — every question is answered by citation or
  observation except four residuals that need a Developer ID identity, an
  offline quarantined machine, or a real install (see "Cannot determine").
  Two R-11 questions close here, one R-11 "correction" is itself overturned,
  and R-11's last behavioural question stays `UNRESOLVED`.
- **Answered on:** 2026-10-10
- **Machine:** macOS 26.7.1 (25G241), x86_64; Xcode 26.5 (17F42) selected at
  `/Applications/Xcode.app/Contents/Developer`, SDK 26.5, Command Line Tools
  26.6.0.0.1781586589, `ld` PROJECT:ld-1267; rustc 1.98.0 (88d9e12ae
  2026-08-18), cargo 1.98.0 (797e8a9bc 2026-08-05), both targets installed.
  Repository HEAD `8f60433`. Neither Homebrew nor MacPorts was invoked.
- **Blocking:** the implementation pass that writes the release workflow
  (ADR-0005; `docs/spec/08-distribution-and-project.md`, Distribution), and the
  byte-identity requirement in both.

**About the identifier.** Filed as R-19 in `docs/spec/09-research-checklist.md`.
It continues R-11 (`R-11-apple-signing-toolchain.md`), whose two open questions
are listed first, below.

## Question

R-11 left open, as filed 2026-09-06:

- **R-11 Q4** — does the hardened runtime need an entitlement for a tool that
  spawns `brew` and `port`? `UNRESOLVED` there.
- **R-11 Q5** — what does a quarantined, notarised, **unstapled** binary do
  offline, and is a CLI launched from a shell subject to the same Gatekeeper
  evaluation? `UNRESOLVED` there.

This note was asked to close those and to answer five further questions the
release workflow depends on:

1. Stapling on a bare Mach-O vs `.pkg` vs `.dmg`; Gatekeeper on each when
   quarantined; whether hardened runtime and `--timestamp` are required to
   notarise a CLI; whether a bare Mach-O can be notarised by ZIP upload and what
   that buys.
2. What two builds of one commit need to be byte-identical, and which
   "byte-identical" claim survives signing.
3. How to pin `MACOSX_DEPLOYMENT_TARGET` per architecture (10.12 x86_64,
   11.0 arm64) with the pinned toolchain, and whether that toolchain still
   supports 10.12.
4. `pkgbuild`/`productbuild` for a CLI installed to one stated directory with
   no `PATH` edit; extracting the payload of a `.pkg` and a `.dmg` to assert
   hashes; whether a `.dmg` can be built deterministically.
5. Current GitHub runner labels for both architectures, their Xcode versions
   against the deployment targets, and secret handling for `codesign` and
   `notarytool`.

## Evidence

All web sources read 2026-10-10. Apple's documentation pages render with
JavaScript, so they were read through the DocC JSON endpoint behind each page
(`https://developer.apple.com/tutorials/data/documentation/<path>.json`). That
endpoint holds the same text as the rendered page. A `curl` of the HTML URL
returns an empty shell (R-11 already recorded this).

### E1 — Apple: notarisation prerequisites

*Notarizing macOS software before distribution*,
<https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>:

> Enable the Hardened Runtime capability for your app and command line targets

> Include a secure timestamp with your code-signing signature.

> Don't include the com.apple.security.get-task-allow entitlement with the value set to any variation of true.

> Link against the macOS 10.9 or later SDK

> Beginning in macOS 10.14.5, software signed with a new Developer ID certificate and all new or updated kernel extensions must be notarized to run. Beginning in macOS 10.15, all software built after June 1, 2019, and distributed with Developer ID must be notarized.

> When the user first installs or runs your macOS software, the presence of a ticket (either online or attached to the executable) tells Gatekeeper that Apple notarized the software.

*Resolving common notarization issues*,
<https://developer.apple.com/documentation/security/resolving-common-notarization-issues>:

> If you don't enable the hardened runtime, notarization fails and reports an issue with the following message: `The executable does not have the hardened runtime enabled.`

> If you use a custom export process, notarization might fail with the following message: `The signature does not include a secure timestamp.`

> Generating a secure timestamp requires internet access. macOS accepts only one secure timestamp server, namely `timestamp.apple.com` […]

> When code signing items like Mach-O files, disk images, bundles, apps, command line tools, photos, and so on, sign with a Developer ID Application certificate. Sign installer packages with a Developer ID Installer certificate.

The same page says `codesign -dvv` shows a `Timestamp` line when a secure
timestamp is present, and `Signed Time` when it is not.

### E2 — Apple: what can be uploaded and what can be stapled

*Customizing the notarization workflow*,
<https://developer.apple.com/documentation/security/customizing-the-notarization-workflow>:

> The notary service accepts disk images (UDIF format), signed flat installer packages, and ZIP archives. It processes nested containers as well, like packages inside a disk image.

> While you can notarize a ZIP archive, you can't staple to it directly. […] Although tickets are created for standalone binaries, it's not currently possible to staple tickets to them.

> The notary service generates a ticket for the top-level file that you specify, as well as for each nested file.

> After notarization completes, the next time any user attempts to run your app on macOS 10.14 or later, Gatekeeper finds the ticket online. This includes users who downloaded your app before notarization.

> This ensures that Gatekeeper can find the ticket even when a network connection isn't available.

`stapler(1)`, the installed copy
`/Applications/Xcode.app/Contents/Developer/usr/share/man/man1/stapler.1`
(Xcode 26.5), read 2026-10-10:

> A ticket contains a list of the code signatures for executables within a supported file format.

> Stapling does not invalidate the code signature and must be run after an executable or archive has been code-signed and notarized with Developer ID. Code-signing a supported file format invalidates any stapled tickets, so stapler staple must be run again if this occurs.

> stapler requires internet access to retrieve tickets when stapling or validating.

> stapler works only with UDIF disk images, signed "flat" installer packages, and certain code-signed executable bundles such as ".app". Passing an unsigned "flat" installer package or an unsigned executable bundle in path to stapler is considered an error.

`xcrun stapler --help` says the same: "Supported file formats are: UDIF disk
images, code-signed executable bundles, and signed "flat" installer packages."

### E3 — Apple: Gatekeeper and the hardened runtime

*Gatekeeper and runtime protection*, Apple Platform Security,
<https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web>:

> When a user downloads and opens an app, a plug-in, or an installer package from outside the App Store

> Gatekeeper also requests user approval before opening downloaded software for the first time

> all software in macOS is checked for known malicious content the first time it's opened, regardless of how it arrived on the Mac.

The page does not mention quarantine, tickets, offline behaviour, Terminal or
command-line tools, or disk images.

*Hardened Runtime*,
<https://developer.apple.com/documentation/security/hardened-runtime>. The
overview reads:

> The Hardened Runtime, along with System Integrity Protection (SIP), protects the runtime integrity of your software by preventing certain classes of exploits, like code injection, dynamically linked library (DLL) hijacking, and process memory space tampering.

The complete list of runtime exceptions is: allow JIT, allow unsigned
executable memory, allow `DYLD` environment variables, disable library
validation, disable executable memory protection, and debugging tool. The
resource-access list is: audio input, camera, location, address book,
calendars, photos library, and Apple Events. **None of these concerns creating
a child process or `exec`.**

`codesign(1)`, `/usr/share/man/man1/codesign.1`, read 2026-10-10. On the
`runtime` flag:

> On macOS versions >= 10.14.0, opts signed processes into a hardened runtime environment which includes runtime code signing enforcement, library validation, hard, kill, and debugging restrictions. […] Note: macOS versions older than 10.14.0 ignore the presence of this flag in the code signature.

On `--timestamp`:

> requests that a timestamp authority server be contacted to authenticate the time of signing. […] If the timestamp authority service cannot be contacted over the Internet, or it malfunctions or refuses service, the signing operation will fail.

On `-i`:

> If this option is omitted, the identifier is derived from either the Info.plist (if present), or the filename of the executable being signed

### E4 — Observed: the hardened runtime and `posix_spawn` (2026-10-10)

The test program is in scratch, `hr/spawn.c`: `posix_spawn("/bin/echo",
{"child-ran"})` followed by `waitpid`.

```
$ clang -O1 -o spawn spawn.c && codesign -s - -o runtime --force spawn && codesign -dv spawn 2>&1 | grep -E "flags|Signature"; ./spawn; echo exit=$?
CodeDirectory v=20500 size=303 flags=0x10002(adhoc,runtime) hashes=3+2 location=embedded
Signature=adhoc
child-ran
child exit 0
exit=0
```

**Not observed:** Gatekeeper's assessment of any artifact. A repository hook
refused the `spctl`/`syspolicy_check` man-page lookup as touching system
security settings, and this note does not route around a hook. No Developer ID
identity exists here, so nothing was signed with one or notarised.

### E5 — Observed: two release builds of the host target (2026-10-10)

Both builds ran from the repository at HEAD `8f60433`, with `--locked
--offline`. Only the target directory differed; both were under the session
scratch directory. No tracked file was changed. The repository's
`[profile.release]` is `strip = true`, `lto = "thin"`, `codegen-units = 1`.
`RUSTFLAGS` and `MACOSX_DEPLOYMENT_TARGET` were unset.

```
$ cargo build --release --locked --offline --target x86_64-apple-darwin --target-dir <scratch>/tA   # 23:42 UTC, 6m 46s
$ cargo build --release --locked --offline --target x86_64-apple-darwin --target-dir <scratch>/tB   # started after A, 5m 42s
$ shasum -a 256 …/release/macportsbrew …/release/mpb ; otool -l …/macportsbrew | grep uuid
f9e87a707318b8d55171ffab62e7c8c5040dbd11bd5380d59e5e383cc0780c38  tA/x86_64-apple-darwin/release/macportsbrew
a5e65b0eba106fe763b9e2e6cccd18064a923f127df43b5fc35823623e4765bf  tA/x86_64-apple-darwin/release/mpb
    uuid 0BC157AB-A3EA-31CE-8CD0-AA4D949069E0
f9e87a707318b8d55171ffab62e7c8c5040dbd11bd5380d59e5e383cc0780c38  tB/x86_64-apple-darwin/release/macportsbrew
a5e65b0eba106fe763b9e2e6cccd18064a923f127df43b5fc35823623e4765bf  tB/x86_64-apple-darwin/release/mpb
    uuid 0BC157AB-A3EA-31CE-8CD0-AA4D949069E0
$ cmp tA/…/macportsbrew tB/…/macportsbrew && echo IDENTICAL
IDENTICAL
```

The same x86_64 binary was then inspected:

```
$ otool -l …/macportsbrew | grep -E "^ +cmd " | sort | uniq -c
   1 LC_DATA_IN_CODE  1 LC_DYLD_INFO_ONLY  1 LC_DYSYMTAB  1 LC_FUNCTION_STARTS
   1 LC_LOAD_DYLIB    1 LC_LOAD_DYLINKER   1 LC_MAIN      4 LC_SEGMENT_64
   1 LC_SOURCE_VERSION 1 LC_SYMTAB         1 LC_UUID      1 LC_VERSION_MIN_MACOSX
$ otool -l …/macportsbrew | grep -A4 LC_VERSION_MIN_MACOSX
      cmd LC_VERSION_MIN_MACOSX
  cmdsize 16
  version 10.12
      sdk 26.5
$ codesign -dv …/macportsbrew          # (no output: x86_64 is not linker-signed)
$ strings -a …/macportsbrew | grep -c "/Users/<user>"
49
$ strings -a …/macportsbrew | grep "/Users/<user>" | sed -E 's#(/Users/<user>/[^/]+/[^/]+/[^/]+).*#\1#' | sort | uniq -c
  49 /Users/<user>/.cargo/registry/src
$ strings -a …/macportsbrew | grep -c "scratchpad\|/tA/"
0
$ nm -ap …/macportsbrew | grep -c " OSO "
0
$ find crates -name build.rs            # (none in the workspace)
$ ls <scratch>/tA/x86_64-apple-darwin/release/build | (strip hash) | sort -u
serde  serde_core  serde_json  zmij
```

The only warning in the build log is Cargo's manifest warning that `main.rs`
belongs to two `bin` targets. The linker printed no deployment-target warning.

### E6 — Observed: two aarch64 release builds with an explicit target and a path remap (2026-10-10)

```
$ MACOSX_DEPLOYMENT_TARGET=11.0 RUSTFLAGS="--remap-path-prefix=$HOME/.cargo/registry/src=/cargo-registry" \
    cargo build --release --locked --offline --target aarch64-apple-darwin --target-dir <scratch>/tC   # then tD, identically
$ shasum -a 256 tC/aarch64-apple-darwin/release/macportsbrew tC/aarch64-apple-darwin/release/mpb
fe268f64819de317c5f66d59787b921eff65f7ee32b8234a02c7ef4b8c61b61d  tC/aarch64-apple-darwin/release/macportsbrew
53b74e1ddc57f096557579b6e9c192492e29c02d447e39c51fda0b76a832aa7b  tC/aarch64-apple-darwin/release/mpb
fe268f64819de317c5f66d59787b921eff65f7ee32b8234a02c7ef4b8c61b61d  tD/aarch64-apple-darwin/release/macportsbrew
53b74e1ddc57f096557579b6e9c192492e29c02d447e39c51fda0b76a832aa7b  tD/aarch64-apple-darwin/release/mpb
$ cmp tC/…/macportsbrew tD/…/macportsbrew && echo IDENTICAL
IDENTICAL
$ codesign -dvvv tC/…/macportsbrew | grep CDHash= ; (same for tD)
CDHash=f9e51ce3f8578048cb521652fdc7761644858bf9
CDHash=f9e51ce3f8578048cb521652fdc7761644858bf9
$ otool -l tC/…/macportsbrew
      cmd LC_UUID           uuid 89B99963-FD7F-362D-BB76-53D2928D7425
      cmd LC_BUILD_VERSION  platform 1  minos 11.0  sdk 26.5  ntools 1
      cmd LC_CODE_SIGNATURE dataoff 3306768  datasize 26000
$ codesign -dv tC/…/macportsbrew
Identifier=macportsbrew-de1bcd7c9999f074
CodeDirectory v=20400 size=25976 flags=0x20002(adhoc,linker-signed) hashes=808+0 location=embedded
Signature=adhoc
TeamIdentifier=not set
$ codesign --verify --strict -v tC/…/macportsbrew
…: valid on disk
…: satisfies its Designated Requirement
$ strings -a tC/…/macportsbrew | grep -c "/Users/<user>"      → 0
$ strings -a tC/…/macportsbrew | grep -c "/cargo-registry/"  → 49
```

### E7 — Observed: signing determinism with an ad-hoc identity (2026-10-10)

```
$ for n in 1 2; do cp tA/…/macportsbrew s$n; codesign -s - -i org.example.r19 -o runtime --timestamp=none s$n; done
$ cp s1 r1 && codesign --remove-signature r1
$ shasum -a 256 s1 s2 r1 tA/…/macportsbrew
4401e37423c73d61794084f778b61b750f5b8e688117e39c8fe23bcb590c5c22  s1
4401e37423c73d61794084f778b61b750f5b8e688117e39c8fe23bcb590c5c22  s2
dd55abf582ff677ef7340ebc21c7c0a66c54fe7b964f09ea9c0f26368de27011  r1
f9e87a707318b8d55171ffab62e7c8c5040dbd11bd5380d59e5e383cc0780c38  tA/x86_64-apple-darwin/release/macportsbrew
$ codesign -dvvv s1 | grep CDHash= ; codesign -dvvv s2 | grep CDHash=
CDHash=f99beef0ed1e6d480905bc16149dc6209796868c
CDHash=f99beef0ed1e6d480905bc16149dc6209796868c
```

### E8 — `ld(1)`, rustc and Cargo on reproducibility

`ld(1)` (installed with Xcode 26.5, ld-1267), read 2026-10-10:

> `-random_uuid` […] By default the linker generates the UUID of the output file based on a hash of the output file's content. […] Using a hash based UUID is important for reproducible builds

> `-no_uuid` Do not generate an LC_UUID load command in the output file. Be warned that binaries without UUIDs may cause the debugger and crash reporting tools to be unable to track and inspect the binary.

> `-oso_prefix prefix-path` When generating the debug map, the linker will remove the specified prefix-path from the path in OSO symbols. This can be used so to help build servers generate identical binaries.

> `-reproducible` By default output content will be deterministic, but small changes in input files such as a compilation time might affect certain data structures in the linked binary.

> `-adhoc_codesign` […] The default for Apple Silicon binaries is to be ad-hoc codesigned.

From the rustc book, *Remap source paths*,
<https://doc.rust-lang.org/stable/rustc/remap-source-paths.html>. On scope:
"`rustc` supports remapping source paths prefixes **as a best effort** in all
compiler generated output". On scopes, `macro` covers "the expansion of
`std::file!()` macro. This is where paths in embedded panic messages come from".
On the linker:

> on Apple platforms, linkers generate [OSO entries] […] which are not remapped by the compiler and need to be manually remapped with `-oso_prefix`.

> generated code introduced by Cargo's build script may still contain un-remapped paths.

`rustc --help -v` on 1.98.0 lists `--remap-path-prefix` and
`--remap-path-scope`. `rustc -C help` lists `strip=val` as `none`
(default), `debuginfo` or `symbols`.

Cargo, *Profiles*, <https://doc.rust-lang.org/cargo/reference/profiles.html>,
on `strip`: "`strip = true` is equivalent to `strip = "symbols"`". On
`codegen-units`: "The default is 256 for incremental builds, and 16 for
non-incremental builds."

Cargo, *Environment variables*,
<https://doc.rust-lang.org/cargo/reference/environment-variables.html>: no
mention of `SOURCE_DATE_EPOCH` or `MACOSX_DEPLOYMENT_TARGET`.

### E9 — Deployment target: rustc and Xcode

The pinned toolchain, observed:

```
$ rustc --print deployment-target --target x86_64-apple-darwin
MACOSX_DEPLOYMENT_TARGET=10.12
$ rustc --print deployment-target --target aarch64-apple-darwin
MACOSX_DEPLOYMENT_TARGET=11.0
```

The rustc book, *Platform support* (stable),
<https://doc.rust-lang.org/stable/rustc/platform-support.html>, places
`x86_64-apple-darwin` in "Tier 2 with Host Tools" as "64-bit macOS (10.12+,
Sierra+)", and `aarch64-apple-darwin` in "Tier 1 with Host Tools" as "ARM64
macOS (11.0+, Big Sur+)". The *apple-darwin* page,
<https://doc.rust-lang.org/stable/rustc/platform-support/apple-darwin.html>,
says:

> The minimum supported version is macOS 10.12 Sierra on x86, and macOS 11.0 Big Sur on ARM64.

> `rustc` respects the common environment variables used by Xcode to do so, in this case `MACOSX_DEPLOYMENT_TARGET`.

The stable book shows no version number. It describes current stable, which may
be newer than 1.98.0. The observed defaults above are the 1.98.0 fact.

Apple, *Xcode support*, <https://developer.apple.com/support/xcode/>,
"Deployment Targets" column, macOS line:

| Xcode | macOS deployment targets |
|---|---|
| 16, 16.1, 16.2, 16.3, 16.4 | macOS 10.13–15 |
| 26, 26.0.1, 26.1, 26.1.1 | macOS 11-26 / 11-26.1 |
| 26.2, 26.3 | macOS 11-26.2 |
| 26.4.1 | macOS 11-26.4 |
| 26.5, 26.6 | macOS 11-26.5 |

The page defines the column as: "The OS range supported by this version of
Xcode for uploading apps to App Store Connect."

### E10 — Packaging tools: man pages (installed copies, read 2026-10-10)

`pkgbuild(1)`:

- `--root root-path`: "Package the entire contents of the directory tree".
- `--install-location`: "Specify the default install location for the
  contents of the package. […] whether or not the default install location is
  actually used by the macOS Installer depends on the distribution file you
  deploy with the package."
- `--identifier` and `--version`: the macOS Installer recognises an upgrade
  only when the identifiers match.
- `--ownership`: "By default […] the recommended UID and GID will be applied
  […] files installed in the user home directory will be owned by that user."
- `--sign`, `--keychain`, `--timestamp`, `--timestamp=none`.
- `--filter`: excludes paths in the root that match.

`productbuild(1)`:

- On product signing: "The signature can optionally include a trusted
  timestamp. This is enabled by default when signing with a Developer ID
  identity".
- "Note that component packages do not need to be signed (e.g. with
  pkgbuild(1)) before adding them to a signed product archive."
- The pre-install requirements property list has a `home` key: "the product
  can be installed under the user's home directory […] (Home directory
  installation is disabled by default.)"
- The `arch` key: "On Apple Silicon, the macOS Installer will evaluate the
  product's distribution under Rosetta 2 unless the arch key includes the arm64
  architecture specifier."

Apple, *Distribution XML Reference*, `domains` element,
<https://developer.apple.com/library/archive/documentation/DeveloperTools/Reference/DistributionDefinitionRef/Chapters/Distribution_XML_Ref.html>
(Documentation Archive, "Updated: 2012-12-13"):

> If this element is not present, `enable_anywhere` is `true`, `enable_currentUserHome` is `false`, and `enable_localSystem` is `true`.

> A home directory installation is done as the current user (not as `root`), and it cannot write outside of the home directory.

`installer(8)`: `-dominfo` "Displays a list of domains into which the software
package can be installed. For example: LocalSystem or CurrentUserHomeDirectory."

`pkgutil(1)` documents `--expand pkg-path dir-path`, `--payload-files` and
`--check-signature`. **Neither `pkgutil(1)` nor `pkgutil --help` mentions
`--expand-full`.**

`hdiutil(1)`:

- `attach`: `-readonly` ("force the resulting device to be read-only"),
  `-nobrowse` ("render any volumes invisible in applications such as the macOS
  Finder") and `-mountpoint path`.
- `create -srcfolder`: "copies file-by-file the contents of source into image,
  creating a fresh […] filesystem".
- `-fs`: "The default file system is APFS."
- `UDZO - UDIF zlib-compressed image`.
- A search for "deterministic" or "reproduc" finds nothing.

### E11 — Observed: building and extracting a `.pkg` and a `.dmg` (2026-10-10)

Everything below was unsigned, built from the E5 binary (`root/macportsbrew`
is a copy of it), and stayed in scratch. Nothing was installed. The
`--install-location` is a dummy.

```
$ for n in 1 2; do pkgbuild --root root --identifier org.example.mpb-r19 --version 0.0.0 \
    --install-location /tmp/r19-never-installed --quiet out/p$n.pkg; sleep 2; done
$ shasum -a 256 out/p1.pkg out/p2.pkg
1775bd16097a98385093490f9029475d55c4f47fb9a51a6d81f628a24db9b803  out/p1.pkg
aedd5c51d21153ed34537fb286c000d80435b7158f3b539b97f16469044eb94c  out/p2.pkg
$ xar --dump-toc=toc1.xml -f out/p1.pkg; xar --dump-toc=toc2.xml -f out/p2.pkg; diff toc1.xml toc2.xml
<   <creation-time>2026-10-10T23:56:26</creation-time>   >   …23:56:29
<    <inode>310589276</inode>                            >   310589377
<    <atime|mtime|ctime>2026-10-10T23:56:25Z            >   …23:56:28Z
$ pkgutil --expand-full out/p1.pkg x1 && find x1 -type f | sort
x1/Bom  x1/PackageInfo  x1/Payload/macportsbrew
$ shasum -a 256 x1/Payload/macportsbrew root/macportsbrew
f9e87a707318b8d55171ffab62e7c8c5040dbd11bd5380d59e5e383cc0780c38  x1/Payload/macportsbrew
f9e87a707318b8d55171ffab62e7c8c5040dbd11bd5380d59e5e383cc0780c38  root/macportsbrew
$ pkgutil --expand out/p1.pkg x2 && file x2/Payload
x2/Payload: gzip compressed data, from Unix, original size modulo 2^32 3858944
$ mkdir x2p && tar -xf x2/Payload -C x2p && shasum -a 256 x2p/macportsbrew
f9e87a707318b8d55171ffab62e7c8c5040dbd11bd5380d59e5e383cc0780c38  x2p/macportsbrew
$ pkgutil --payload-files out/p1.pkg
.
./macportsbrew
./._macportsbrew
$ xattr -l root/macportsbrew
com.apple.provenance:
$ xattr -d com.apple.provenance root2/macportsbrew; xattr -l root2/macportsbrew   # rc=0, attribute still present
com.apple.provenance:
$ COPYFILE_DISABLE=1 pkgbuild --root root2 … out/p4.pkg && pkgutil --payload-files out/p4.pkg
.  ./macportsbrew  ./._macportsbrew
$ pkgbuild --root root2 --filter '/\._' --filter '\.DS_Store$' … out/p5.pkg && pkgutil --payload-files out/p5.pkg
.  ./macportsbrew  ./._macportsbrew
```

```
$ for n in 1 2; do hdiutil create -quiet -volname MacPortsBrew -srcfolder dm/src -fs HFS+ -format UDZO -ov out/d$n.dmg; sleep 2; done
$ shasum -a 256 out/d1.dmg out/d2.dmg
7e3f93c4f396ca65482218ed074e25c1ec8f99aee42d08783e2d8dbbfe3cab2b  out/d1.dmg
3305bfed68871ee30dc7a153c2644ccae3b16641bb07fe2ae88ba71ec46306d8  out/d2.dmg
$ hdiutil imageinfo out/d1.dmg | grep -E "^Format:|Format Description"
Format Description: UDIF read-only compressed (zlib)
Format: UDZO
$ hdiutil attach -nobrowse -readonly -noautoopen -mountpoint <scratch>/pk/mnt out/d1.dmg | tail -1
/dev/disk8s1   Apple_HFS   <scratch>/pk/mnt
$ shasum -a 256 mnt/macportsbrew
f9e87a707318b8d55171ffab62e7c8c5040dbd11bd5380d59e5e383cc0780c38  mnt/macportsbrew
$ hdiutil detach -quiet <scratch>/pk/mnt; echo detach=$?
detach=0
```

### E12 — GitHub: runner labels, images and secrets

`actions/runner-images` `README.md`, `main`,
<https://raw.githubusercontent.com/actions/runner-images/main/README.md>:

| Image | Arch | Labels |
|---|---|---|
| macOS 26 | x64 | `macos-latest-large`, `macos-26-intel`, `macos-26-large` |
| macOS 26 Arm64 | arm64 | `macos-latest`, `macos-26` or `macos-26-xlarge` |
| macOS 15 | x64 | `macos-15-large`, or `macos-15-intel` |
| macOS 15 Arm64 | arm64 | `macos-15`, or `macos-15-xlarge` |
| macOS 14 (deprecated) | x64 | `macos-14-large` |
| macOS 14 Arm64 (deprecated) | arm64 | `macos-14` or `macos-14-xlarge` |

The README also says: "The `-xlarge` and `-large` suffixes are unique to macOS
images and are only available for GitHub Actions." It does not say which labels
are billed.

GitHub Docs, *GitHub-hosted runners reference*,
<https://docs.github.com/en/actions/reference/runners/github-hosted-runners>,
standard runners for **public repositories**:

- Intel: 4 CPU, 14 GB RAM, 14 GB storage. Labels `macos-15-intel` and
  `macos-26-intel`.
- arm64: 3 CPU (M1), 7 GB RAM, 14 GB storage. Labels `macos-latest`,
  `macos-14`, `macos-15`, `macos-26`, and `xcode-27` (marked "(Public
  preview)").

The same page: "Use of the standard GitHub-hosted runners is free and unlimited
on public repositories."

Image readmes on `main` (each lists Rust 1.98.0, Cargo 1.98.0, Rustup 1.29.0
where shown):

| Image readme | OS / image version | Default Xcode | Other Xcodes |
|---|---|---|---|
| `macos-15-Readme.md` (x64) | 15.7.9 (24G830) / 20260824.0482.1 | 16.4 (16F6) | 16.0–16.3, 26.0.1, 26.1.1, 26.2, 26.3 |
| `macos-15-arm64-Readme.md` | 15.7.9 (24G830) / 20260907.0337.1 | 16.4 (16F6) | same set |
| `macos-26-Readme.md` (x64) | 26.6.1 (25G76) / 20260824.0517.1 | 26.6 (17F113) | 26.0.1, 26.1.1, 26.2, 26.3, 26.4.1, 26.5 |
| `macos-26-arm64-Readme.md` | 26.6.2 (25G83) / 20260907.0351.1 | 26.6 (17F113) | same set |

Command Line Tools: 16.4.0.0.1.1747106510 on the macOS 15 images and
26.6.0.0.1781586589 on the macOS 26 images.

GitHub Docs, *Signing Xcode applications*,
<https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications>,
gives this recipe:

```
security create-keychain -p "$KEYCHAIN_PASSWORD" $KEYCHAIN_PATH
security set-keychain-settings -lut 21600 $KEYCHAIN_PATH
security unlock-keychain -p "$KEYCHAIN_PASSWORD" $KEYCHAIN_PATH
security import $CERTIFICATE_PATH -P "$P12_PASSWORD" -A -t cert -f pkcs12 -k $KEYCHAIN_PATH
security set-key-partition-list -S apple-tool:,apple: -k "$KEYCHAIN_PASSWORD" $KEYCHAIN_PATH
security list-keychain -d user -s $KEYCHAIN_PATH
```

The certificate is stored base64-encoded as a secret and decoded to a file
under `$RUNNER_TEMP`. Cleanup is `security delete-keychain …` under
`if: ${{ always() }}`.

`security(1)` (installed), `create-keychain`:

> If neither -P or -p password are specified, the user is prompted for a password on the command line. Use of the -p option is insecure.

`import -P passphrase`: "Specify the unwrapping passphrase immediately. The
default is to obtain a secure passphrase via GUI."

`xcrun notarytool --version`: `1.1.2 (41)`. `xcrun notarytool help submit`:

```
-k, --key <key>         App Store Connect API key. File system path to the private key.
-d, --key-id <key-id>   App Store Connect API Key ID. […]
-i, --issuer <issuer>   App Store Connect API Issuer ID, UUID format. Required for Team API Keys. Do not provide for Individual API Keys.
--keychain <keychain>   Pass the path to a keychain file to use for reading the keychain item.
--wait/--no-wait        Wait until processing is complete. […]
-f, --output-format <output-format>   […] ["normal", "json", "plist"]
```

`notarytool help store-credentials` accepts `--key/--key-id/--issuer` and
`--keychain <path>`, and validates the credentials by default.

## Findings

### R-11's open questions

1. **R-11 Q4 is answered: no entitlement is needed to spawn `brew` and `port`.**
   Apple's documented exception list for the hardened runtime has no entry for
   creating a child process or for `exec` (E3). Each exception relaxes a
   protection on the signed process's own memory, loading or debugging. Here, a
   process signed with the `runtime` flag spawned and reaped a child normally
   (E4). The residual is narrow: that run was ad-hoc signed, not Developer ID
   signed and notarised. The first release's notarised binary running
   `macportsbrew doctor` confirms it at no extra cost. Two related facts from
   E3: the hardened runtime is **ignored below macOS 10.14.0**, so it changes
   nothing for 10.12–10.13; and the executor's sanitised environment (INV-10)
   already drops `DYLD_*`, which the runtime would refuse without its
   exception anyway.

2. **R-11 Q5 stays `UNRESOLVED` and narrows to one undocumented case.** The
   documented part, all cited:
   - With no stapled ticket, "Gatekeeper finds the ticket online" on 10.14 and
     later (E2).
   - Stapling exists so "Gatekeeper can find the ticket even when a network
     connection isn't available" (E2).
   - Gatekeeper asks for approval "before opening downloaded software for the
     first time" (E3).

   Apple's security guide names apps, plug-ins and installer packages. It does
   not name command-line tools or Terminal (E3). So neither "a quarantined
   notarised bare binary run from a shell offline" nor "a binary copied out of
   a stapled, mounted `.dmg`" has a documented outcome. Observation was not
   possible here: there is no Developer ID artifact, and the repository's hooks
   forbid touching Gatekeeper configuration.

3. **R-11's correction of its own Q2 premise is itself wrong.**
   - GitHub Docs lists `macos-15-intel` and `macos-26-intel` as **standard**
     runners for public repositories (E12), and standard runners are "free and
     unlimited on public repositories".
   - The runner-images README lists `-intel` beside `-large` as labels of the
     same x64 image. R-11 read that as "an alias of `macos-15-large`, therefore
     a larger runner".
   - The billing tier follows the label, not the image. The `-large` label is
     the larger, billed runner; the `-intel` label is the standard 4-CPU
     runner.

   So `07-quality-and-testing.md` (CI) and `repository-validation.yml` were
   right and R-11 plus its checklist entry are wrong. An Intel release job on
   `macos-15-intel` or `macos-26-intel` costs nothing on a public repository.

### Q1 — Stapling, Gatekeeper, notarisation of a CLI

4. **Where a ticket can be stapled:**
   - `.dmg` (UDIF): yes.
   - Signed flat `.pkg`: yes. Stapling an unsigned one is "considered an
     error".
   - Bare Mach-O: no.
   - ZIP: no.

   Sources: `stapler(1)`, `stapler --help`, *Customizing the notarization
   workflow* (E2). Stapling needs network, and re-signing a container
   invalidates its staple (E2). The order is therefore fixed: sign the inner
   Mach-O, build and sign the container, notarise, staple. **Every container
   checksum is computed after stapling**, because stapling changes the
   container's bytes. (That last step is an inference from "attaches"; the
   release job's own `SHA256SUMS` generation settles it.)

5. **Hardened runtime and a secure timestamp are both required to notarise a
   command-line tool, and so is the absence of `get-task-allow`.** All three
   are cited, with the exact failure messages (E1). Apple names "command line
   targets" explicitly. `--timestamp` contacts `timestamp.apple.com` and fails
   the signing if it cannot (E1, E3), so the signing step needs network.
   `productbuild` timestamps Developer ID signatures by default; `pkgbuild` and
   `codesign` need the flag (E10, E3). The SDK must be 10.9 or later; the
   builds here record SDK 26.5 (E5, E6).

6. **A bare Mach-O can be notarised as a ZIP, and that buys only the online
   lookup.** The notary accepts ZIP archives and "tickets are created for
   standalone binaries" (E2), but they cannot be stapled. What it buys is that
   a quarantined bare binary passes Gatekeeper's online check on 10.14+,
   including for users who downloaded it before notarisation completed (E2).
   **A separate ZIP submission may be redundant.**
   - Tickets are generated "for each nested file" (E2).
   - A ticket "contains a list of the code signatures for executables"
     (`stapler(1)`, E2).
   - The bare binary is byte-identical to the Mach-O nested in the `.dmg` and
     `.pkg` (08's MUST).

   So the ticket issued for the nested copy should be found online for the bare
   one. That is an **inference** from the keying, not a citation. The first
   release can check it with `notarytool log <id>`, which lists the per-file
   ticket contents. Until it is checked, submit the ZIP as well; it costs one
   submission.

### Q2 — Reproducible builds

7. **On one machine, the pinned toolchain and the current release profile
   already produce byte-identical unsigned binaries, with no extra flag.**
   - Two x86_64 release builds with different target directories, about 6
     minutes apart, gave identical SHA-256 for both `macportsbrew` and `mpb`,
     and an identical `LC_UUID` (E5).
   - Two aarch64 builds produced the same result (E6).
   - The output did not vary with build time. `SOURCE_DATE_EPOCH` is not
     needed for the Mach-O: neither Cargo's environment-variable reference nor
     the rustc book mentions it (E8). It matters only to container tooling that
     honours it, and `pkgbuild`/`hdiutil` document no such variable (E10).

8. **The cross-machine conditions are what a CI-built release must hold.** In
   order of what E5/E6 showed:
   - **Host paths from the Cargo registry leak into the binary.** 49 strings
     under `$HOME/.cargo/registry/src` were found in the x86_64 binary: the
     panic locations from `file!()` (E5, E8). A runner's `HOME` is not the
     developer's, so without a remap a local rebuild cannot match the release.
     `--remap-path-prefix=$CARGO_HOME/registry/src=/cargo-registry` removed all
     of them (E6: 0 host paths, 49 remapped).
   - Workspace sources already appear as relative paths: 0 occurrences of the
     repository path (E5).
   - The standard library's paths are already `/rustc/<commit>` and fixed by
     the pin.
   - Rust's remap does not reach linker-generated OSO paths (E8). The release
     profile emits none: `debug = false`, `strip = "symbols"`, and the `OSO`
     count is 0 (E5). If debug info is ever enabled, `-C link-arg=-oso_prefix`
     becomes necessary.
   - **The linker and SDK are inputs.** The binary records `sdk 26.5` and an
     `ld` tool version (E5, E6). Byte identity across machines therefore needs
     the same Xcode selected (`DEVELOPER_DIR` or `xcode-select`). The hosted
     images carry several Xcodes, and the default differs per image (E12).
   - `LC_UUID` is a hash of the output's content (E8 `ld(1)`). It is
     reproducible as it stands. Do **not** use `-no_uuid` (it breaks crash
     tooling) or `-random_uuid` (it breaks reproducibility).
   - `codegen-units = 1` and `lto = "thin"` were in effect. The default (16)
     was not tested, so nothing is claimed about it.
   - Four dependencies run build scripts (`serde`, `serde_core`, `serde_json`,
     `zmij`). rustc warns that build-script output may escape the remap (E8).
     E6 found no host path, so none escaped in this build.

9. **Which "byte-identical" claim is achievable — precisely:**
   - **(a) The three artifacts of one build embed byte-identical executables:
     achievable and assertable.** The `.pkg` payload extracted two ways and the
     `.dmg` payload extracted by read-only attach both hash to the bare
     binary's SHA-256 (E11). This is the claim 08 and ADR-0005 make. The
     signing step must run once, on the bare binary, **before** packaging, and
     the signed file must then be copied into both containers.
   - **(b) An unsigned binary rebuilt from the same commit: achievable**
     under finding 8's conditions. Observed on one machine; cross-machine is
     conditional and unobserved.
   - **(c) A signed binary rebuilt and re-signed: not byte-identical.**
     - `--timestamp` asks a timestamp authority "to authenticate the time of
       signing" (E3), so each signing embeds a different timestamp token.
     - The control is ad-hoc signing with `--timestamp=none`. It *is*
       byte-deterministic and gives the same CDHash (E7), so the variable is
       the timestamp and the CMS signature, not the code-directory hashing.
     - **Inference, to be confirmed on the first two signed builds:** the
       CDHash of two Developer ID signings of identical input, with the same
       `-i` and `-o runtime`, is identical.
     - Stripping a signature does not restore the original bytes:
       `--remove-signature` gave a third hash (E7). "Remove the signature and
       compare with an unsigned rebuild" is not a usable check. Compare the
       CDHash instead, or rebuild unsigned and compare with an unsigned build
       kept from the release job.
   - **(d) Containers rebuilt: not byte-identical**, before any signing. Two
     `pkgbuild` runs differ in the xar table of contents: creation time,
     inodes, file times. Two `hdiutil create` runs differ too (E11). No
     documented option fixes either (E10). Signing and stapling add more
     variation.

   The honest public statement is: *the payload of every artifact is the
   published bare binary, byte for byte; the unsigned binary is reproducible
   from source with the stated toolchain, Xcode and flags; signatures and
   containers are not.*

10. **The `arm64` linker signature survives Cargo's strip, and its identifier
    is not the one to ship.** After `strip = "symbols"` the aarch64 binary
    reports "valid on disk" with `flags=(adhoc,linker-signed)` (E6). So
    rustc's link-time strip does not leave a broken signature: R-9's hazard
    applies to a *post-link* strip, not to this profile. The linker-signed
    identifier is `macportsbrew-de1bcd7c9999f074`, which is name plus rustc
    metadata hash. Without `-i`, `codesign` derives the identifier from the
    filename (E3). The release job should pass `-i` explicitly, so the
    designated requirement does not depend on a filename. A linker signature
    is replaced without `--force` (`codesign(1)`, `linker-signed`). Also:
    `macportsbrew` and `mpb` are **different binaries** with different hashes
    (E5, E6), even though both build from one `main.rs`. Every checksum and
    payload assertion is per file.

### Q3 — Deployment target

11. **The pinned toolchain supports 10.12, and the floors hold by default.**
    rustc 1.98.0 defaults to 10.12 for x86_64 and 11.0 for aarch64 (E9). The
    current stable platform-support pages state the same minimums (E9). The
    built binaries record `LC_VERSION_MIN_MACOSX version 10.12` for x86_64
    (`ld` emits the older load command for this target) and
    `LC_BUILD_VERSION minos 11.0` for aarch64 (E5, E6). These are exactly the
    load commands `scripts/check-deployment-target.sh` reads.

12. **How to pin per architecture.** rustc reads one environment variable,
    `MACOSX_DEPLOYMENT_TARGET` (E9). One value applies to every target in one
    `cargo` invocation, and Cargo's environment reference does not mention
    the variable (E8). So:
    - **One `cargo build --target T` per architecture, with
      `MACOSX_DEPLOYMENT_TARGET` set for that invocation.** E6 did this with
      11.0. Equivalently, leave it unset and rely on the pinned defaults, as
      the existing check does.
    - Assert the result on the artifact either way, as
      `check-deployment-target.sh` already does.

    Two cautions:
    - A setting left in the job environment and applied to both targets in one
      invocation either raises the x86_64 floor (11.0) or is clamped for
      aarch64. The script header records the clamp (an explicit 10.11 is
      clamped to 10.12).
    - A toolchain bump can move the default. The pin comment in
      `rust-toolchain.toml` already says this.

13. **No hosted Xcode officially supports a 10.12 deployment target, yet the
    linker produces one without complaint.** Apple's table gives Xcode 16.x
    "macOS 10.13–15" and Xcode 26.x "macOS 11-…" (E9). That column is defined
    for App Store Connect uploads, which this project does not do.
    - Every Xcode on the hosted images is 16.0–26.6 (E12). Locally, Xcode
      26.5's `ld` (ld-1267) linked a binary recording 10.12 and printed no
      deployment-target warning (E5).
    - So the x86_64 10.12 floor is held by rustc and `ld`'s behaviour, **not
      by any Apple support statement**. That reinforces 07's "best-effort" for
      10.12–10.15.
    - The artifact check is the only guard against a future `ld` raising or
      refusing the floor. The arm64 floor, 11.0, is inside every Xcode 26
      range.
    - Whether a binary with these load commands actually runs on 10.12
      remains untestable on hosted CI (07 already says so).

### Q4 — Packaging

14. **`.pkg` for one CLI in one stated directory, user domain, no `PATH`
    edit.** The shape the man pages and the Distribution reference support
    (E10):
    1. `pkgbuild --root <dir holding only the signed binary> --identifier
       <stable id> --version <semver> --install-location <stated dir>
       --ownership recommended` → component package. It can stay unsigned
       ("component packages do not need to be signed").
    2. A distribution file with `<domains enable_anywhere="false"
       enable_currentUserHome="true" enable_localSystem="false"/>`, and
       `arch`/host requirements including `arm64`. Without that, Apple
       Silicon evaluates the distribution under Rosetta 2.
    3. `productbuild --distribution <file> --package-path <dir> --sign
       "Developer ID Installer: …" [--keychain <temp keychain>]
       --timestamp` → the signed product archive that is notarised and
       stapled.

    A home-domain install "is done as the current user (not as `root`), and
    it cannot write outside of the home directory" (E10). That is what makes
    ADR-0005's "user-domain install requiring no administrator password"
    expressible. **No `postinstall` script** is needed: nothing edits `PATH`,
    and the documentation carries the one line. **How `--install-location`
    is interpreted in the current-user-home domain** (prefixed with the home
    directory, or used verbatim) is not stated by the sources read, and it is
    the thing that decides the "one stated directory". It is `UNRESOLVED`
    (see "Cannot determine").

15. **Payload extraction for the hash assertion.**
    - `.pkg`: the **documented** route is `pkgutil --expand <pkg> <dir>`, then
      `tar -xf <dir>/<component>/Payload` (or `Payload` for a component
      package). Observed: the payload is gzip-compressed and its file hashes
      to the bare binary (E11). `pkgutil --expand-full` also works on macOS
      26.7.1 (E11) but appears in neither `pkgutil(1)` nor `--help` (E10). A
      CI assertion should not rest on an undocumented flag. If `pkgbuild
      --compression`/`--min-os-version` is ever set, the payload format may
      change and the `tar` step must be re-observed.
    - `.dmg`: `hdiutil attach -nobrowse -readonly -noautoopen -mountpoint
      <tmp> <dmg>`, hash, then `hdiutil detach <tmp>` (E11, observed;
      options from E10).
    - The assertion should also check the **file list**, not only the hash
      (`pkgutil --payload-files`, `ls` on the mount). On this machine every
      scratch file carries a `com.apple.provenance` attribute that could not
      be removed, and `pkgbuild` turned it into a `._macportsbrew`
      AppleDouble entry in the payload. Neither `COPYFILE_DISABLE=1` nor
      `--filter` removed it (E11). It does not change the binary's bytes, but
      it does add an entry, and on install it restores an attribute. Whether
      a hosted runner sets the attribute is a runtime fact: the first release
      job should record `pkgutil --payload-files`.

16. **A `.dmg` cannot be built deterministically with `hdiutil`, and it does
    not need to be.** Two runs from identical input differ (E11), and
    `hdiutil(1)` offers no reproducibility option (E10). The claim 08
    requires is payload identity (finding 9a), which holds. **Filesystem
    choice is load-bearing for the x86_64 image:** `hdiutil`'s default is
    APFS (E10). The x86_64 `.dmg` serves a 10.12 floor, so it should be
    created with `-fs HFS+` (as in E11). That sidesteps the question of
    whether 10.12 mounts an APFS image, which this note did not cite. The
    format must be UDZO (R-11 Q3, Apple; E11 confirms `-format UDZO` produces
    it).

### Q5 — GitHub Actions

17. **Labels, read 2026-10-10.**
    - **Intel, standard and free on public repositories:** `macos-15-intel`
      (macOS 15.7.9) and `macos-26-intel` (macOS 26.6.1).
    - **arm64, standard:** `macos-15`, `macos-26`, `macos-latest` (=26),
      `macos-14` (deprecated). `xcode-27` is in public preview.
    - **Billed, and forbidden by 07:** `-large`, `-xlarge`,
      `macos-latest-large`.

    Default Xcode is 16.4 on the macOS 15 images and 26.6 on the macOS 26
    images. Both carry 26.0.1–26.3, and macOS 26 also carries 26.4.1, 26.5 and
    26.6. Rust 1.98.0 is preinstalled on all of them (E12). All of these Xcodes
    hold 11.0 for arm64 officially and 10.12 for x86_64 only in practice
    (finding 13). Because the release must name one Xcode for reproducibility
    (finding 8), **select it explicitly**: the same version is present on both
    a 15 and a 26 image (26.0.1–26.3), so one Xcode can build both
    architectures.

18. **Secret handling.**
    - **codesign.** GitHub's documented recipe (E12) is a temporary keychain
      under `$RUNNER_TEMP`. It decodes the `.p12` from a base64 secret to a
      file, imports it with `-A`, sets the partition list for `apple-tool:`
      and `apple:`, puts the keychain on the user search list, and deletes it
      under `if: always()`. Pass `--keychain` to
      `codesign`/`productbuild`/`pkgbuild` so the identity cannot come from
      elsewhere (E3, E10). **That recipe puts both passwords in `argv`**
      (`create-keychain -p`, `unlock-keychain -p`, `import -P`,
      `set-key-partition-list -k`), and `security(1)` itself calls `-p`
      "insecure" (E12). ADR-0005 requires credentials never to reach `argv`.
      The alternatives `security(1)` offers are a command-line prompt
      (`create-keychain` with neither `-p` nor `-P`) and a GUI prompt
      (`import` without `-P`). Whether either can be fed from stdin on a
      non-interactive runner is not established. This is a genuine conflict
      between GitHub's official recipe and ADR-0005, and it is the main agent's
      call (see "Impact"). On an ephemeral hosted VM the `ps` exposure is
      confined to the job itself. The real exposure is a `set -x` or a
      debug log, which ADR-0005 already forbids.
    - **notarytool.** Use the App Store Connect API key. `--key` takes a
      **file path** to the private key, while `--key-id` and `--issuer` are
      identifiers rather than secrets. Write the `.p8` from a secret into
      `$RUNNER_TEMP` with `0600` permissions, pass the path, and delete it in
      an `always()` step. Alternatively, `store-credentials --keychain <temp
      keychain>` and `submit -p <profile> --keychain <temp keychain>` keep it
      inside the temporary keychain (E12). `--wait` with `-f json` gives a
      machine-readable submission id for 08's per-release record.

## Cannot determine

- **R-11 Q5, the behavioural half.** What a quarantined, notarised, unstapled
  bare binary does when executed from a shell with no network, and whether a
  binary copied out of a mounted, stapled `.dmg` carries quarantine and passes
  offline. It needs a Developer ID artifact, a quarantined download and a
  machine with networking off. Observation is the only route; Apple does not
  document the CLI case (E3). Note also that this repository's hooks forbid
  reading or changing Gatekeeper settings, so the experiment belongs on a
  machine outside the agent's reach. **UNRESOLVED.**
- **The notarised-and-spawning confirmation of finding 1.** The first notarised
  binary running `doctor` confirms it. Expected to pass.
- **Whether the nested ticket covers the bare binary online (finding 6), and
  whether two Developer ID signings share a CDHash (finding 9c).** Both are
  settled by the first release: run `notarytool log` on the `.dmg` submission
  and `codesign -dvvv` on two signings. **UNRESOLVED until then.**
- **How a current-user-home `.pkg` interprets `--install-location`**, and
  whether the macOS 10.12 Installer honours `enable_currentUserHome` for a
  signed product archive the same way current macOS does. It needs an install,
  which this note may not perform. The safe experiment is
  `installer -pkg … -target CurrentUserHomeDirectory` in a disposable user
  account or VM. Before that, `installer -dominfo -pkg <built pkg>` (read-only)
  shows which domains the built package offers. **UNRESOLVED.**
- **Whether `security` can take the keychain and `.p12` passwords from
  stdin** on a non-interactive runner (finding 18). It needs one CI run; a
  local test would create a keychain, which this note avoided. **UNRESOLVED.**
- **Cross-machine byte identity** of the unsigned binary (finding 8) was not
  observed. Only one machine was available. It needs one build on a hosted
  runner with the same Xcode and the remap, compared against a local build.
- Whether macOS 10.12 mounts an APFS disk image. Not cited; avoided by
  `-fs HFS+` (finding 16).
- Whether `com.apple.provenance` is present on files a hosted runner builds
  (finding 15).

## Impact on the specification

- **Contradicts R-11 and its checklist entry** (`docs/spec/09-research-checklist.md`,
  R-11: "`macos-15-intel` is not a free standard runner … An Intel release job
  costs money on a public repository"). GitHub Docs lists `macos-15-intel` and
  `macos-26-intel` as free standard runners (finding 3). **Confirms**
  `07-quality-and-testing.md` (CI, "hosted Intel macOS runners exist as
  `macos-15-intel` and `macos-26-intel` — standard runners, free") and
  `.github/workflows/repository-validation.yml`. ADR-0005's compatibility line
  "buildable natively on the free `macos-15-intel` standard runner" also
  stands. R-11 §Q2 and "What this changes" item 3 should be corrected through
  `docs/spec-evolution.md`. This note does not edit them.
- **Closes R-11 Q4** (finding 1). The checklist's "Still open" for R-11 drops to
  Q5 alone.
- **Sharpens 08 / ADR-0005 "byte-identical executables"** (finding 9). The
  requirement as written is achievable: one build, three containers, payload
  equals the bare binary. Nothing in 08 should be read as "a rebuild reproduces
  the signed artifact"; that is false. The release workflow must order these
  steps:
  1. sign the bare binary;
  2. copy the signed binary into the containers;
  3. sign the containers;
  4. notarise;
  5. staple;
  6. compute `SHA256SUMS`;
  7. assert the payloads.

  If the project wants to promise source reproducibility, that is a new,
  narrower claim about the **unsigned** binary, conditional on finding 8's
  pins (the Xcode version and the registry remap). It would need a spec
  entry. It is not implied by the current text.
- **Bears on ADR-0005's credential rule** (finding 18). GitHub's official
  keychain recipe passes passwords in `argv`, and `security(1)` calls that
  insecure. The ADR's "never in `argv`" needs either a stdin-fed `security`
  invocation proven on CI, or an explicit, recorded acceptance of the argv
  exposure on an ephemeral hosted VM. That decision belongs to the main agent
  or the owner.
- **Adds to the release workflow's assertions:**
  - Assert the payload **file list**, not only the hash; the AppleDouble entry
    is the example (finding 15).
  - Use documented `pkgutil --expand` plus `tar`, not `--expand-full`.
  - Build the x86_64 `.dmg` with `-fs HFS+` (finding 16).
  - Pass an explicit `codesign -i` (finding 10).
  - Select one Xcode explicitly (finding 8).
  - Never put `MACOSX_DEPLOYMENT_TARGET` in a job-wide `env:` that covers both
    targets in one invocation (finding 12).
- **Reinforces 07's best-effort status for 10.12–10.15** with a new reason:
  10.12 lies outside every hosted Xcode's stated deployment range (finding 13).
  `check-deployment-target.sh` is the only guard. That is consistent with
  ADR-0002, not a contradiction.
- **ADR-0005 "`.pkg` MUST … be a user-domain install requiring no
  administrator password"** is expressible (`enable_currentUserHome`, finding
  14). Which concrete directory it lands in is still open.

## Re-check when

- **The pinned toolchain moves past 1.98.0**: re-run
  `rustc --print deployment-target` for both targets and the two-build
  identity test (E5).
- **The selected Xcode or `ld` changes**: the 10.12 link (finding 13), the
  `LC_VERSION_MIN_MACOSX` form, and the binary's recorded SDK all depend on it.
- **runner-images changes labels or retires an image.** These are dated facts
  in the INV-20 sense. GitHub's earlier statement that x86_64 macOS ends with
  the macOS 15 image's retirement (recorded in 07) did not appear on either
  page read today. Re-read both pages before relying on `macos-*-intel` past
  2027.
- **Apple changes what `stapler` accepts** (a staplable bare Mach-O would
  change findings 4 and 6), **the notarisation prerequisites**, or the
  hardened-runtime exception list.
- **The release profile changes** (`strip`, `debug`, `codegen-units`, `lto`),
  or a dependency with a build script is added: findings 7–8 must be
  re-observed.
- **`pkgbuild` gains `--compression`/`--min-os-version` in the release job**:
  re-observe the payload format for the `tar` extraction (finding 15).
