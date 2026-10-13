<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 09 — Release engineering: plan

Work order: [`../../prompts/09-release-engineering.md`](../../prompts/09-release-engineering.md).
Drafted 2026-10-10. To be reviewed by `security-reviewer` (supply chain,
signing, secrets) and `spec-guardian` **before any guarded write**; the finding
each later change answers is cited in brackets (`SG`, `SR`). Sources:
[R-19](../research/R-19-release-toolchain.md) (2026-10-10; cited as R-19
f*N* for finding *N*, E*N* for evidence),
[R-11](../research/R-11-apple-signing-toolchain.md),
[ADR-0005](../adr/0005-release-signing-and-artifacts.md),
[`08-distribution-and-project.md`](../spec/08-distribution-and-project.md)
(Distribution), [`07-quality-and-testing.md`](../spec/07-quality-and-testing.md),
DP-004, DP-008, DP-019.

**Revised 2026-10-10 after both reviews**; every change cites its finding as
`[SR Hn/Mn/Ln]` or `[SG Hn/Mn/Ln]`, and "Revision after review" at the end lists
each finding with its disposition.

**Start SHA [SG M8]:** this pass starts at
`ea46a1f6dbe6327ecf13ca4d714e598ba9420ac7` (`ea46a1f`), and the dry-run hashes
in fact 15 are for that commit and the first script version (`dfaf2625…`).
Commits made by other sessions after it are **excluded** from the work order's
§3 boundary check (`git diff <start>..HEAD --stat -- crates/*/src`): at revision
time that is `ffdd3b3` (pass 08 slice 3, `crates/adapters/`), and any further
pass 08 slice 3 commit that lands before this pass's commits. The check is run
over this pass's own commits, listed by hash in the pass report, not over the
range.

**The guarded files are staged, not written.** The escape (SE-093) was not
armed, so the release script and workflow are in
[`09-staged/`](09-staged/README.md) for the owner to apply in one armed message.
The release script could not be staged at `09-staged/scripts/build-release.sh`
as the work order's caller asked: the path guard matches `*/scripts/*.sh`
anywhere in the tree and refused the write. That refusal is a question for the
owner, not something to route around; the script is therefore carried as a
fenced block inside [`09-staged/README.md`](09-staged/README.md) ("The
script") — documentation no gate executes — and was dry-run from a copy in
the session scratchpad, outside the repository. The armed message writes it to
`scripts/build-release.sh` verbatim.

## Facts that shape it

1. **Notarising a command-line tool needs the hardened runtime, a secure
   timestamp and no `get-task-allow`**, with Apple naming "command line
   targets" (R-19 E1, f5). `--timestamp` contacts `timestamp.apple.com` and
   fails without network, so signing needs network (R-19 E3).
2. **No entitlement is needed to spawn `brew` and `port`** under the hardened
   runtime: no runtime exception concerns `exec`, and an ad-hoc `runtime`
   signed process spawned and reaped a child (R-19 f1, E3, E4). The hardened
   runtime is ignored below macOS 10.14.0.
3. **Stapling:** a `.dmg` (UDIF) and a *signed* flat `.pkg` take a ticket; a
   bare Mach-O and a ZIP do not (R-19 f4, E2). Re-signing a container voids its
   staple, so the order is fixed: sign the executable → copy it into both
   containers → sign the containers → notarise → staple → assert → checksum.
   Checksums come **after** stapling because stapling rewrites the container.
4. **A bare executable is notarised by ZIP upload and gains only the online
   lookup** (R-19 f6). Whether the ticket of the copy nested in the `.dmg`
   already covers it is an inference until `notarytool log` on the first
   release shows it; until then the ZIP is submitted too (one submission).
5. **Which "byte-identical" holds** (R-19 f9):
   - (a) the three artifacts of **one** build embed the same executable —
     achievable and asserted (E11);
   - (b) an **unsigned** rebuild of one commit is byte-identical on one machine
     with the pinned toolchain and the current release profile, no extra flag
     needed (E5, E6); across machines it is conditional on the same Xcode, the
     registry path remap and the same build-metadata environment, and is
     **unobserved**;
   - (c) a **signed** rebuild is not byte-identical — every secure timestamp
     differs; ad-hoc `--timestamp=none` signing is deterministic (E7);
     `--remove-signature` does not restore the unsigned bytes (E7);
   - (d) containers are never byte-identical across runs (`pkgbuild` xar TOC
     times and inodes; `hdiutil`), and no documented option fixes either (E10,
     E11).
6. **Deployment target:** rustc reads one `MACOSX_DEPLOYMENT_TARGET`, so one
   value per `cargo` invocation; the pinned 1.98.0 defaults are 10.12 / 11.0
   (R-19 f11, f12, E9). The binaries record `LC_VERSION_MIN_MACOSX 10.12`
   (x86_64) and `LC_BUILD_VERSION minos 11.0` (aarch64). **10.12 lies outside
   every hosted Xcode's stated deployment range** (16.x: 10.13+, 26.x: 11+);
   `ld` links it without a warning, so the artifact assertion is the only guard
   (R-19 f13).
7. **`.pkg` shape:** an unsigned component (`pkgbuild`) inside a product archive
   signed with **Developer ID Installer** (`productbuild --sign`); a
   distribution with `enable_currentUserHome="true"` and the system domain off
   makes the "no administrator password" MUST expressible; no `postinstall` is
   needed because nothing edits `PATH` (R-19 f14, E1, E10). **How Installer
   interprets `--install-location` in the current-user-home domain is
   UNRESOLVED** (R-19, "Cannot determine"); this session could not run even
   the read-only `installer -dominfo` (a settings deny rule, `Bash(installer *)`).
8. **Payload extraction:** `pkgutil --expand` then `tar` is the documented
   route (`--expand-full` is undocumented); `.dmg` by `hdiutil attach
   -nobrowse -readonly -noautoopen -mountpoint`. **Assert the file list, not
   only the hash:** on a machine that stamps `com.apple.provenance`, `pkgbuild`
   adds `._` AppleDouble entries that neither `COPYFILE_DISABLE` nor `--filter`
   removes (R-19 f15, E11). Re-observed by this dry run (fact 15).
9. **`.dmg`:** not deterministic and need not be; `hdiutil`'s default file
   system is APFS, so the x86_64 image is made `-fs HFS+` for the 10.12 floor;
   format UDZO (R-19 f16).
10. **Runners:** `macos-26-intel` / `macos-15-intel` (x86_64) and `macos-26` /
    `macos-15` (arm64) are **standard, free on public repositories**; `-large`
    and `-xlarge` are billed and forbidden by 07 (R-19 f3, f17, E12). This
    **overturns R-11's and DP-019's premise that the Intel runner is billed**.
    The macOS 26 images carry Xcode 26.5, the version on the research machine,
    and Rust 1.98.0.
11. **Secrets (R-19 f18):** GitHub's documented keychain recipe puts the
    keychain password and the `.p12` password in `argv`, which ADR-0005
    forbids ("every credential reaches its tool by file or stdin"). `notarytool`
    takes the App Store Connect key as a **file path** (`--key`); key ID and
    issuer are identifiers.
12. **What 08 and ADR-0005 bind on the workflow:** signing only on tag pushes
    in the base repository; credentials in a protected environment with a `v*`
    tag rule that **no other workflow references**; a protected-tag ruleset;
    no `pull_request_target`, `workflow_run` or `ACTIONS_STEP_DEBUG`; every
    `uses:` pinned to a 40-hex SHA with a version comment; Developer ID
    authority asserted on the **final** artifact; **a build provenance
    attestation per artifact, one `SHA256SUMS` itself attested, and a
    verification snippet naming the repository**; a per-release record of team
    identifier, certificate fingerprint and notarisation submission per
    artifact, in the notes and a sidecar beside `SHA256SUMS`; **immutable
    releases**; semantic versioning; tool and policy versions recorded; the
    SPDX identifier in release artifact metadata.
13. **What the tree has today:** `scripts/check-action-pins.sh` allow-lists
    five action repositories, none of `actions/upload-artifact`,
    `actions/download-artifact`, `actions/attest-build-provenance`.
    `scripts/run-fixture-matrix.sh` hard-codes `target/debug/macportsbrew`.
    `macportsbrew --version` prints `macportsbrew 0.1.0` from clap's `version`
    attribute. The workspace `repository` is
    `https://github.com/macportsbrew/macportsbrew`. The state directory is
    `~/Library/Application Support/macportsbrew`; SE-036's shim directory is
    `~/.local/share/macportsbrew/bin` (05).
14. **Action versions, resolved 2026-10-10 read-only from each repository's
    own tag ref** (`gh api repos/<r>/git/ref/tags/<t>`, all lightweight tags,
    so the ref object is the commit): `actions/upload-artifact` v7.0.1 →
    `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a` (released 2026-04-10; v7.0.2
    is three days old and was passed over); `actions/download-artifact` v8.0.1
    → `3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c` (2026-03-11; v8.0.2 likewise
    passed over); `actions/attest-build-provenance` v4.2.2 →
    `4d101475d8b20a2381f78447822ac1eab6504dd8` (2026-08-06), a composite
    action whose one nested step is itself SHA-pinned
    (`actions/attest@508db95d…` v4.2.1). `actions/checkout` and
    `dtolnay/rust-toolchain` reuse the SHAs in `repository-validation.yml`.
15. **The dry run (2026-10-10, this machine: macOS 26.7.1 x86_64, Xcode 26.5,
    rustc 1.98.0, ld-1267).** `build-release.sh x86_64-apple-darwin <scratch>`
    unsigned, `CARGO_TARGET_DIR` and `TMPDIR` in the scratchpad, no tracked file
    changed:
    - **First run failed its own assertion, as designed:** the `.pkg` payload
      listed `./._bin` and `./bin/._macportsbrew` beside the three expected
      entries — the provenance AppleDouble R-19 predicted. Each decodes to an
      AppleDouble v2 with zero Finder info, an empty resource fork and one
      attribute, `com.apple.provenance`; the BOM lists them too. The script now
      decodes them and tolerates exactly that, **in an unsigned run only**
      (D-P3). The `.dmg` copy carries the same attribute natively (HFS+).
    - **Two clean runs** of a local clone of HEAD `ea46a1f` (no tracked change;
      separate `CARGO_TARGET_DIR`s A and B, run one after the other, same clone
      path), both exit 0 with every assertion passing:

      | | run A | run B |
      |---|---|---|
      | unsigned build output | `cd4d4653c9a9836c9a91631ccb5d28f4c9b6cde98330b1f5c30f0577b4c2deb0` | identical |
      | `macportsbrew-0.1.0-x86_64` (ad-hoc, runtime, `-i`) | `05f05623e039c5c265ad99bebb7e7ca14d2fa6ee6218a78321110120470e2ead` | identical |
      | executable CDHash | `059a7db16e461ad76f27d657d4271b816a486043` | identical |
      | `.pkg` | `1f08f8ea975064017ca3fc31071f664420e7ae64f1111b7f61f45a9ebbf33e4b` | `2daf31837518adc2748dd798592f0146a779b7b7933a93977d765f3c642557d5` |
      | `.dmg` | `0ded9daa327e9beb4da36b4c4fb96d476d701ecb8747ebb21f32555e17a43f56` | `b30d6c8721a1b10eefc93a7912164c580b6028ba8d89453544b6792441da02f6` |
      | payload in `.pkg` and `.dmg` | = the bare executable | = the bare executable |

      Confirms fact 5 on this machine: (a) asserted; (b) the unsigned output
      and the ad-hoc-signed executable rebuild byte-identically with the
      script's flags (same machine, same clone path; a different path was not
      tried); (d) both containers differ between runs while their payloads do
      not. Recorded: minimum macOS 10.12 by otool and vtool; only system
      libraries; no host path; `-V` = `macportsbrew 0.1.0`; `--version` has no
      metadata yet (commit 2); `codesign -dvvv`:
      `Identifier=io.github.macportsbrew.macportsbrew`,
      `flags=0x10002(adhoc,runtime)`, `Signature=adhoc`; the `.pkg`
      `PackageInfo` says `install-location="/.macportsbrew"`, `auth="root"`,
      `postinstall-action="none"`, five payload files (three plus the two
      tolerated entries); the `.dmg` volume holds only `macportsbrew`, HFS+,
      UDZO. Logs: `scratchpad/release-dryrun/clean-{A,B}.log`.
    - **`auth="root"` in `PackageInfo`** is `pkgbuild`'s default for a
      component. Whether the current-user-home domain overrides it (it should:
      R-19 E10, "done as the current user (not as root)") is part of the same
      Installer check as fact 7.
16. **A concurrent session** was editing tracked files under
    `crates/adapters/` during this work, so the first two runs built a
    modified tree and recorded `commit=<sha>-modified`. The clean runs above
    built a local clone of HEAD `ea46a1f` in the scratchpad. That session has
    since committed `ffdd3b3`; see "Start SHA" above [SG M8].
17. **The revised script, dry-run (2026-10-11) [SR2 B-L4].** Fact 15 ran the
    first version (`dfaf2625…`); the revision after the first review was
    `7fd52400…`. The revision after the second (`c8d3b0f5…`, 680 lines,
    shellcheck 0.11.0 and `bash -n` clean; argument refusals run: exit 2 for
    `--release-input` with `--sign` or `--from-binary` and for
    `--expect-unsigned-sha256` without `--sign`, exit 1 for `--sign` without or
    with a malformed hash) was dry-run by the coordinator on 2026-10-11 from
    the repository root at `39b33e5`, with one tracked document modified (so
    the record says `commit=39b33e5…-modified`), unsigned,
    `x86_64-apple-darwin`: **exit 0**.

    | output | SHA-256 |
    |---|---|
    | unsigned build output | `3e6a58c9c9f132fad6a3cc5299a6685f77aef2496f33651714d6f11fceb4d763` |
    | bare (ad-hoc, runtime, `-i`) | `70b9f8609a0b172585be9f5447a9169fe4a802f72017eebf327ff57bc4cbfce5` |
    | `.pkg` | `5038f85fee267487f2b6aba45b89e95064f1c83e1e92bff3cb900d205cd379e4` |
    | `.dmg` | `02229c95ed6a3191060bdf979e49325cbf024871c896ec920b355d0b6cf58ab8` |

    Minimum macOS 10.12 by otool and by vtool; payload list and modes exact;
    the provenance AppleDouble entries tolerated (dry run only); the `.dmg`
    HFS+, UDZO. Refusals observed on the real tree: `--sign` without
    `--expect-unsigned-sha256` ("FAIL: --sign needs --expect-unsigned-sha256
    …"), and `--release-input` on a dirty tree ("FAIL: tracked files are
    modified; a release input is built from a clean commit"). **Not
    exercised:** the arm64 build and the signed path.
18. **`notarytool submit --timeout`**: `xcrun notarytool submit --help` on this
    machine (Xcode 26.5, 2026-10-10) documents `--timeout <duration>` as the
    limit for `--wait`, with `m` and `h` suffixes [SR L4].

## Design

### `scripts/build-release.sh` — the one implementation of a release artifact

`build-release.sh <target-triple> <out-dir> [--release-input] [--sign]
[--from-binary PATH]`. One invocation per architecture. A relative `<out-dir>`
or `--from-binary` path is resolved against the invocation directory, before
the script changes to the repository root [SR L6]. `--release-input` (build
job, tag push) requires a clean tree and full `--version` metadata; it is a
usage error together with `--sign` or `--from-binary`. `--sign` requires
`--expect-unsigned-sha256 <hex>` (refused without `--sign`), and the script
compares the input with it right after copying it in, before the keychain,
the key file or any identity is consulted [SR2 B-L1, SG2 B-L2].

**Where the checks live, stated honestly [SR2 B-L1].** Every assertion about
an artifact lives in this script (CLAUDE.md: one implementation of "valid").
The workflow is not check-free, and does not pretend to be. It holds the checks
that concern the *run* rather than the artifact, and that cannot run inside
the script: `guard`'s dispatch, semver-tag and ancestor-of-`main` refusals;
the tag-equals-workspace-version check; the secret-presence check; the
pre-credential hash binding in `sign`, which duplicates the script's own
binding on purpose, because the credential step creates the keychain before it
calls the script; `assemble`'s per-arch `SHA256SUMS` verification and its
record re-check; and the release-notes snippet check. The textual rules about
the workflow itself are enforced by `check-release-workflow.sh` (D-T2). Bash 3.2-compatible, `set -euo pipefail`, argv
only, no `eval`, every expansion quoted; shellcheck-clean (0.11.0). It never
names either package manager in a command, never requests administrator
rights, writes only `<out-dir>` (which must be empty) and one `mktemp -d`
directory removed by an `EXIT` trap that first detaches any image.

1. **Floor and build.** `x86_64-apple-darwin` → 10.12, `aarch64-apple-darwin`
   → 11.0. `cargo build --release --locked -p macportsbrew --bin macportsbrew
   --target T` with `MACOSX_DEPLOYMENT_TARGET`, `CARGO_ENCODED_RUSTFLAGS`,
   `MPB_BUILD_TARGET` and `MPB_BUILD_COMMIT` set **on that invocation only**
   (fact 6). Refuses an inherited `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS`
   (they would change the bytes silently). Rust flags: `--remap-path-prefix`
   for `$CARGO_HOME/registry/src` → `/cargo-registry` and the repository root
   → `/macportsbrew`, passed unit-separator-encoded so a path with a space
   survives (fact 5b). Fails on any `built for newer` / deployment-target
   warning in the build log, as `check-deployment-target.sh` does.
2. **The executable, asserted** (`assert_binary`, run on the unsigned build
   and again on the signed one): `lipo -archs` is exactly the one
   architecture; the recorded minimum equals the floor **exactly**, read twice,
   by `otool -l` and by `vtool -show-build`; `otool -L` lists only `/usr/lib/`
   and `/System/Library/` (no runtime dependency, 08); `strings -a` finds no
   `$HOME/` or repository path. **The executable is executed only in an
   unsigned run of a fresh cargo build** (the build job, which holds no
   secret), when the host can execute it: `-V` prints `macportsbrew
   <version>`, and `--version`'s metadata lines name the policy version, the
   target and the commit (required with `--release-input`; tolerated as absent
   in a plain unsigned run until commit 2 lands). **With `--sign` or
   `--from-binary` it is never executed** [SR H1, SG H1]: a compromised
   dependency runs on `-V` too (argument parsing, static initialisers), and
   the signing job has an unlocked keychain and the notary key on disk. The
   record says which happened: `version_check=full|short-only|not-executed`.
3. **Signing, once, before any container** (fact 3). With `--sign`:
   `codesign --sign "$MPB_APP_IDENTITY" --keychain "$MPB_SIGN_KEYCHAIN"
   --identifier io.github.macportsbrew.macportsbrew --options runtime
   --timestamp --force`. Without: the same command ad-hoc (`--sign -
   --timestamp=none`), so a dry run exercises the identifier, the runtime flag
   and the verification path (D-S4). Then `codesign --verify --strict`, and
   `codesign -dvvv` is **logged** (ADR-0005: it discloses only what the
   artifact carries) and asserted: the identifier, `runtime` in the
   CodeDirectory flags, and with `--sign` `Authority=Developer ID
   Application: `, `TeamIdentifier=$MPB_TEAM_ID`, a `Timestamp=` line; the
   leaf certificate is extracted (`codesign -d --extract-certificates`) and its
   SHA-256 recorded (08's certificate fingerprint).
4. **The `.pkg`** (D-P1, D-P2). Staging root `bin/macportsbrew`, root and
   `bin` `0755`, the executable `0755`, copied with `ditto --norsrc
   --noextattr --noqtn --noacl`. `pkgbuild --root <root> --identifier
   <id> --version <semver> --install-location /.macportsbrew --ownership
   recommended` (component unsigned). A hand-written distribution:
   `<domains enable_anywhere="false" enable_currentUserHome="true"
   enable_localSystem="false"/>`, `hostArchitectures="<arch>"` (so an arm64
   package is not evaluated under Rosetta, R-19 f14), `<os-version
   min="<floor>"/>`, one invisible choice, `customize="never"`, no scripts;
   `xmllint --noout`. `productbuild --distribution … --package-path …
   [--sign "$MPB_INSTALLER_IDENTITY" --keychain … --timestamp]`. **Only
   `macportsbrew` is installed** — never `mpb` (ADR-0005; D-B2, which departs
   from 06 and is the owner's, DP-033 [SG H2]) — and nothing edits `PATH`.
5. **The `.dmg`** (D-D1): a folder holding only `macportsbrew`, `hdiutil
   create -volname "MacPortsBrew <version> <arch>" -srcfolder … -fs HFS+
   -format UDZO`; with `--sign`, `codesign` with the Application identity and
   `--timestamp`, then `--verify --strict`.
6. **Payload assertions** (`assert_payloads`, after building and, when
   signed, again after stapling). `.pkg`: `pkgutil --expand` → exactly
   `Distribution` and `macportsbrew.pkg`; the **extracted** Distribution
   enables the current-user domain and disables the other two; `PackageInfo`
   names `install-location="/.macportsbrew"` and no scripts; `tar -tf
   Payload` is exactly `. ./bin ./bin/macportsbrew`, modulo D-P3; `lsbom -p
   fm Bom` gives modes `40755 40755 100755`; `tar -xf` and the extracted file
   hashes to the bare executable. `.dmg`: read-only attach, the volume is
   `Apple_HFS`, `find` lists exactly `./macportsbrew`, its hash equals the
   bare executable's, its extended attributes are none (modulo D-P3), then
   detach; `hdiutil imageinfo` says `Format: UDZO`.
7. **Notarisation and stapling, `--sign` only** (D-N1): `ditto -c -k
   --keepParent` the bare executable into a ZIP; `xcrun notarytool submit
   <file> --key "$MPB_NOTARY_KEY_PATH" --key-id … [--issuer …] --wait
   --output-format json` for the ZIP, the `.pkg` and the `.dmg`; status and
   submission id read with `plutil -extract` (no `jq` on the signing host);
   `--timeout 30m` bounds each wait (fact 18) [SR L4]; anything but `Accepted`
   prints `notarytool log <id>` and fails. Then `stapler staple` and `stapler
   validate` on both containers, and on the **final** bytes [SR M5, SG H3]:
   `pkgutil --check-signature` asserts `Developer ID Installer: `, the team
   and the notarisation line (`trusted by the Apple notary service`);
   `codesign -dvvv` on the stapled `.dmg` asserts `Authority=Developer ID
   Application: `, `TeamIdentifier=$MPB_TEAM_ID` and `Timestamp=`
   (`assert_developer_id`, the executable's three checks as a function);
   `spctl -a -vv -t install` on the `.pkg` and `spctl -a -vv -t open --context
   context:primary-signature` on the `.dmg` must accept with `source=Notarized
   Developer ID`; and the payload assertions run again on the stapled bytes.
   The pkgutil and spctl wordings are expected, not observed (no Developer ID
   artifact exists here, and `spctl` is denied in this session); a mismatch
   fails closed, and the first tag run confirms or corrects them.
8. **Outputs:** `macportsbrew-<version>-<arch>{,.pkg,.dmg}`,
   `SHA256SUMS.<arch>` (verified with `shasum -c` before exit), and
   `macportsbrew-<version>-<arch>.release-record.txt`: tool and policy
   version, `license=Apache-2.0`, target, floor, commit, signed mode,
   `release_input=yes|no`, `version_check=…`, `host_role=build|sign` (which
   machine the `host_*` lines describe), identifier, install location, the
   unsigned build's SHA-256, the executable's SHA-256 and CDHash, team
   identifier, certificate SHA-256, one `notary_submission_<bare|pkg|dmg>=<id>
   <status>` line each, host macOS, developer directory, Xcode, linker and
   rustc versions (`rustc=prebuilt input` on the sign host).
9. **`--from-binary PATH`** packages an already-built executable instead of
   running cargo, asserting it **statically** exactly as above and never
   executing it. It exists for D-W2.
10. **Environment with `--sign`** (names only): `MPB_SIGN_KEYCHAIN`,
    `MPB_APP_IDENTITY`, `MPB_INSTALLER_IDENTITY`, `MPB_TEAM_ID`,
    `MPB_NOTARY_KEY_PATH` (must be a regular file, mode `0600`, owned by the
    caller), `MPB_NOTARY_KEY_ID`, optional `MPB_NOTARY_ISSUER`. Missing any is
    a failure, never a silent downgrade to unsigned; both identities are
    checked first — the Application one with `security find-identity -v -p
    codesigning <keychain>`, the Installer one with `security find-identity -v
    <keychain>` (not a code-signing identity) [SR L6]. No value is printed; the
    only argv-borne values are identity *names* and paths.
11. **Robustness [SR L6]:** the BOM filter for the tolerated entries matches
    the first field exactly (`awk`), not by substring; the AppleDouble decoder
    bounds the member to 152–4096 bytes and the attribute value to at most 64
    bytes inside the member.

### The install directory — `~/.macportsbrew/bin` (D-P1, owner: DP-035)

**This is the owner's [SG M7].** Once users have added the `PATH` line and
Installer receipts exist, moving the directory strands them, so it is as hard
to change after the first release as the identifier (D-S3); both are routed
together in DP-035, with the value below as the reversible default.

08 and ADR-0005 bound the choice: MacPortsBrew-owned; outside `/usr/local`,
`/opt/homebrew` and `/opt/local`; a current-user install with no
administrator password; user-owned `0755`; **not the state directory**; the
same target for the `.dmg` copy and the documented `cp` line.
`~/.macportsbrew/bin` meets each: it is in the home domain, owned by nothing
else, distinct from the state directory
(`~/Library/Application Support/macportsbrew`) and from SE-036's shim
directory (`~/.local/share/macportsbrew/bin`), whose job — symlinks for a
chosen primary — must not be confused with the tool itself. It has no space,
so the one documented line needs no quoting subtleties. `~/.local/bin` was
rejected: it is shared by many tools, so it is not "MacPortsBrew-owned", and
`doctor` reporting it would be reporting someone else's directory.

**The documented line appends:** `export PATH="$PATH:$HOME/.macportsbrew/bin"`,
added to `~/.zprofile` (or `~/.zshrc`) for zsh, the default shell since macOS
10.15, or `~/.bash_profile` for bash, the default on 10.12–10.14 — the notes
name the files rather than saying "your shell profile" [SG L7].
Appending means no existing command is shadowed by anything in that
directory, which is the shadowing `doctor` exists to report; ADR-0005 still
requires `doctor` to report the directory rather than special-case it, and
that finding kind is still owed (`path_audit.rs`, ADR-0005).

### Credentials, and the one deviation from ADR-0005 (D-S1, owner: DP-034)

**This is a deviation from ADR-0005's text, not a reading of it [SG M1].**
ADR-0005 names "the temporary keychain password" among the credentials that
must never reach a public log, and says: "Every credential reaches its tool by
file or stdin, never in `argv`". The staged step passes a temporary keychain
password and a re-wrap password in `security`'s argv. The owner rules in
DP-034 between (a) accepting that on the ephemeral VM and amending ADR-0005,
(b) `security -i` reading its commands from stdin — which may satisfy the ADR as
written and is **unverified**, needing a dated observation on a throwaway
keychain in a disposable account (an R-19 addendum) — and (c) another route
that research finds. The staged default is (a), reversible because nothing is
signed before the ruling. `security-reviewer` accepts (a) provided nothing else
runs on the signing VM, which the H1 fix now ensures [SR L3].

The sign step creates a keychain under `$RUNNER_TEMP` with `umask 077`. The two
argv values are `openssl rand -base64 32` generated inside the step, registered
with `::add-mask::`, never stored, unset as soon as their last consumer has run,
and worthless once the hosted VM is destroyed. The step resets `PATH` to
`/usr/bin:/bin:/usr/sbin:/sbin` and names its tools by absolute path
(`/usr/bin/security`, `/usr/bin/openssl`, `/usr/bin/base64`, `/usr/bin/tr`,
`/usr/bin/xargs`, `/bin/rm`, `/bin/chmod`), so nothing in the image's
user-writable prefixes is resolved while a secret is in the environment, and
`build-release.sh` inherits that `PATH` [SR L3]. The owner's secrets never
reach argv:

- **The `.p12` password** reaches only `/usr/bin/openssl pkcs12 -passin
  env:<NAME>`. The certificate and key are re-wrapped under the random
  password through two process substitutions, so no unencrypted key touches
  disk, and only the re-wrapped file is imported (`security import … -P
  <random>`). Observed 2026-10-10 on this machine with a throwaway
  self-signed key (no keychain created): LibreSSL 3.3.6 decrypts a PBES2
  (AES-256, PBKDF2) `.p12` written by OpenSSL 3.6 with `-passin env:`, and the
  `-in <(…) -inkey <(…)` re-wrap round-trips; a single piped stdin does not
  (the export reads its input twice).
- **The notary key** is written with `printf` (a builtin, no argv) to
  `notary-key.p8`, `chmod 600`, and passed by path.
- Each secret variable is `unset` right after its consumer: the Application
  `.p12` pair after its import, the Installer pair after its import, the
  notary key after it is written [SR L3]. The cleanup step deletes the
  keychain and the key under `if: always()`.

### `.github/workflows/release.yml`

Triggers: `push` of `v*` tags, and `workflow_dispatch` with a boolean
`dry_run` (default `true`). A dispatch is **always** a dry run: the `guard` job
refuses `dry_run=false`, because ADR-0005 allows signing on tag pushes only.
A pushed tag must be a semantic version, and the `build` job checks it names
the workspace version. Top-level `permissions: {}`; `concurrency` per ref, no
cancellation; `DEVELOPER_DIR` spelled once, in the top-level `env`, with its
source (R-19 E12, 2026-10-10) [SG L4]; `timeout-minutes` on every job — 5, 90,
120, 15, 15 — so a stuck notarisation cannot hold the credential VM for the
six-hour default [SR L4].

| Job | Runs on | Permissions | Does |
|---|---|---|---|
| `guard` | ubuntu-latest | `contents: read` | refuses a non-dry dispatch and a non-semver tag; on a tag, checks out with full history and **refuses a tag whose commit is not an ancestor of `origin/main`** [SR M2]; warns loudly on a dry run |
| `build` ×2 | `macos-26-intel`, `macos-26` | `contents: read` | named Xcode check, `scripts/validate.sh` (tests + fixture matrix), `build-release.sh` **unsigned** (with `--release-input` on a tag: clean tree, full `--version` metadata — the only place the built program is executed), the fixture matrix against the release executable (`MPB_FIXTURE_BIN`), uploads the raw build output and the unsigned artifacts |
| `sign` ×2 | same | `contents: read`, environment `release-signing` | tag push only; downloads the raw executable; **before any credential exists**, compares `shasum -a 256` of `in/raw/macportsbrew` with the build record's `unsigned_build_sha256` and requires `release_input=yes`, `version_check=full` and `commit=$GITHUB_SHA` in that record [SR H1, SG H1, SR L1]; then **one step** sees the secrets (D-S1) and runs `build-release.sh --sign --expect-unsigned-sha256 <that value> --from-binary`, which binds the input again itself and never executes it [SR2 B-L1]; cleanup under `always()`; uploads the signed artifacts. If any secret is absent it fails with `::error::` naming the missing names: a tag never produces an unsigned release |
| `assemble` | ubuntu-latest | `contents: read` | verifies each `SHA256SUMS.<arch>`, re-checks `unsigned_build_sha256` and `commit` across the two records (a consistency check against accidental drift, not the binding, which is in `sign` [SR L1]), concatenates `SHA256SUMS`, writes `SIGNING-RECORD.txt` with the signing fields **and the build job's toolchain as `build_*` lines** [SR L5, SG M5], generates `RELEASE-NOTES.md` from README.md's marked Install section [SG M2], uploads the bundle — named `release-bundle` on a tag and `dry-run-unsigned-bundle` on a dispatch, whose notes open with a DRY RUN banner [SR L2] |
| `draft` | ubuntu-latest | `contents: write`, `id-token: write`, `attestations: write` | tag push only; `attest-build-provenance` over `subject-checksums: SHA256SUMS` (every artifact), over `SHA256SUMS` itself and over `SIGNING-RECORD.txt` [SR M3, SG L2]; `gh release create --draft --verify-tag` with the notes and every file. **Never publishes** |

The skip-loudly behaviour: a dispatch prints a `::warning title=Dry run::` in
`guard`, the script prints `NOT SIGNED WITH DEVELOPER ID` and `NOT
NOTARISED`, the bundle and its notes say DRY RUN, and nothing downstream of
`build` that could sign or publish runs.

**Why `build` and `sign` are separate jobs, and what the split rests on
(D-W2, restated [SR H1, SG H1]).** Four dependencies run build scripts (R-19
f8), and a build script is arbitrary code; so is any dependency code linked
into the executable, which runs whenever the program does. In one job either
could leave a process behind that outlives its step and reads the next step's
environment or the keychain. The split puts compilation **and every execution
of the built program** on the build VM, which holds no secret, and gives the
signing VM only `checkout`, `download-artifact`, Apple's tools and this
repository's two shell steps: `build-release.sh` with `--sign --from-binary`
asserts the input statically (`lipo`, `otool`, `vtool`, `strings`, `codesign`)
and never executes it. What binds the signed bytes to the bytes the build job
ran and checked is the hash comparison in `sign`, before the keychain is
created. That comparison guards against a corrupted or mismatched artifact;
it cannot guard against a compromised build VM, which produces both the
executable and its record — the defence there is the same as for any CI
build: pinned actions, a pinned toolchain, `--locked` dependencies and
ADR-0004's dependency gate.

**What CI asserts about byte identity** (fact 5): (a) on every run, by the
script, before and after stapling; the build-to-sign hand-off by `sign`'s hash
comparison. (b) is not asserted by CI: it is a documented, conditional property
of the unsigned executable, demonstrated on one machine (fact 15).
`SIGNING-RECORD.txt` publishes what an attempt needs — commit, target,
deployment floor, the unsigned build's SHA-256, and the **build** job's host
macOS, developer directory, Xcode, linker and rustc (`build_*` lines); the
flags are in the script at that commit [SR L5, SG M5]. (c) and (d) are never
claimed.

### `macportsbrew --version` (commit 2; `crates/cli`, not a guarded path)

`-V` stays `macportsbrew <version>` (scripts parse it). `--version` becomes
clap's `long_version`:

```
macportsbrew 0.1.0
policy 2026.10.03-1
target x86_64-apple-darwin
commit <40-hex>
```

- `policy` from the embedded policy data's `version()` at run time (the same
  value `doctor --report` prints), not duplicated at build time.
- `target` from `option_env!("MPB_BUILD_TARGET")`, falling back to the triple
  implied by `std::env::consts::ARCH` (`x86_64`/`aarch64` → `…-apple-darwin`),
  so a developer build still prints a truthful value.
- `commit` from `option_env!("MPB_BUILD_COMMIT")`, else `unknown (not a
  release build)`. No `build.rs` (none exists in the workspace, and adding one
  adds a build script); `option_env!` is tracked by rustc's env-dep info, so a
  changed value rebuilds.
- Built in `main.rs` as `Cli::command().long_version(build_info())` then
  `Cli::from_arg_matches`; the string is owned, so clap's `string` feature is
  enabled in the workspace `clap` entry (a feature, no new crate: `Cargo.lock`
  is expected unchanged and the commit asserts it with `git diff --exit-code
  Cargo.lock`; `cargo tree -e normal` unchanged).
- Behaviour otherwise unchanged; `THIS_BUILD_DISPATCHES_MUTATIONS` stays
  `false` (§3 of the work order). Switching to `from_arg_matches` must not
  change a usage error's exit code; one test asserts it [SG L8].
- **i18n [SG L6]:** `policy`, `target`, `commit` and `unknown (not a release
  build)` are user-visible text outside the catalogue, which 06 forbids ("All
  user-facing text goes through the i18n layer"); the existing clap help text
  has the same gap and no recorded exemption. The version block is a
  machine-read contract (scripts and the release job parse it), so it stays
  English and fixed, and an exemption is recorded (proposed SE text, item 4).

### Release notes and install documentation (commit 6)

**One source, generated, not copied [SG M2].** README.md's Install section
carries the release text between two marker lines, `<!-- release-notes:begin
-->` and `<!-- release-notes:end -->`. The `assemble` job extracts that
section, replaces `X.Y.Z` with the version, prefixes a DRY RUN banner on a
dispatch, and appends the signing record on a tag; it fails if the markers are
missing or duplicated, or if the section's verification snippet lacks any of
`--repo $REPO`, `--signer-workflow $REPO/.github/workflows/release.yml` and
`--source-ref refs/tags/v$VERSION`, each matched literally after the version
substitution [SR2 B-L3, SG2 B-L1]. The YAML holds no copy of the text.
`wiki/Getting-Started.md` **links** to README's Install section rather than
repeating it (SE-067; 08: "no uncontrolled duplication").

**Ordering [SG2 B-L1]: the README markers (commit 6) must land before the first
dispatch dry run.** The notes step fails without them, so a dry run before
commit 6 fails at `assemble` — loudly, after building and asserting both
architectures — and proves less than it should.

**`mpb` text: nothing changes until DP-033 is ruled [SG2 B-H1].** The charter
(`00-charter.md` line 8, Tier 2) says `mpb` is "installed alongside"
`macportsbrew`; changing that, 06's MUST, ADR-0003, README's Quick start
(~lines 129–132), `wiki/Getting-Started.md` (~lines 30–32) or the `--help`
`long_about` is a charter amendment and the owner's explicit decision. This
pass therefore makes **no** `mpb` text correction in commit 2 or commit 6, and
the first tag is blocked until DP-033 is ruled. Whatever wording the ruling
approves must distinguish **release artifacts** (which install `macportsbrew`
only, as staged) from **cargo builds** (`crates/cli/Cargo.toml` defines both
`[[bin]] macportsbrew` and `[[bin]] mpb`, so `cargo build` and `cargo install`
produce `mpb` today). The list of texts to change, for whoever applies the
ruling: the charter line; 06 (Surface); ADR-0003's decision; README Quick start;
`wiki/Getting-Started.md`; `crates/cli/src/app/mod.rs` `long_about`.

Commit 6 still corrects README's two stale statements unrelated to `mpb`: ".dmg
is provisional on that answer" (DP-008 made all three unconditional) and "R-11,
still open" (now narrowed to Q5).

**What the section says, beyond the 08 MUSTs already in it** (each Medium/Low
finding it answers):

- **Best-effort, in those words [SG M3]:** 10.12–10.15 "are best-effort", with
  the reason (built for and checked to declare that minimum, not tested every
  release; R-19 f13: the 10.12 floor is held by `ld`'s behaviour, not an Apple
  support statement).
- **The bare file [SG M2]:** rename to `macportsbrew`, make it executable, put
  it on `PATH`; one complete `curl` line does all three.
- **The `.dmg` [SG M2]:** Finder hides `~/.macportsbrew`, so the row gives the
  `mkdir` and the Shift-Command-G steps, or "a folder you choose".
- **The `.pkg` [SR M4, SG M4]:** "designed to … without asking for an
  administrator password" and "being confirmed on real installs before the
  first release" — conditional until the owner's Standard-account check
  (`09-staged/README.md`, checks before the first tag, step 2) is recorded in
  R-19, after which commit 7 (or the next release) states it plainly. Later,
  the observed ownership and modes become a script assertion on the expanded
  `PackageInfo` (owed; needs the observation first).
- **Shell files [SG L7]:** `~/.zprofile` / `~/.zshrc` for zsh, `~/.bash_profile`
  for bash.
- **`mpb` [SG H2, SG2 B-H1]:** the proposed text below carries a "The short
  name `mpb`" subsection and a "does not install `mpb`" clause in the `.pkg`
  row; both are **held out of commit 6 until DP-033 is ruled**, and rewritten
  to the ruling's wording then.
- **Gatekeeper [SG L7]:** a quarantined bare file asks Apple over the network
  on first run; what happens with no network **has not been established**
  (R-11 Q3), so an offline user is pointed at the `curl` line or the `.pkg`;
  and the ticket is **not** claimed to reach a copied-out executable (R-11 Q5).
- **Verification [SR M3, SG L2, SG M6]:** `gh attestation verify` with
  `--repo`, `--signer-workflow <owner>/<repo>/.github/workflows/release.yml`
  and `--source-ref refs/tags/vX.Y.Z`; `SIGNING-RECORD.txt` is attested too;
  `gh` probably needs macOS 12 (it is a Go program; ADR-0002 records Go 1.25+
  needs macOS 12 — inferred, not checked for `gh`), so the notes say to verify
  on any machine; `shasum -c --ignore-missing` on 10.12 is unverified, so the
  notes give the by-eye fallback.

**Proposed README Install text** (between the markers; commit 6 writes it and
adjusts headings to README's level; `macportsbrew/macportsbrew` follows
DP-031; the "The short name `mpb`" subsection and the `.pkg` row's last
sentence are **held until DP-033 is ruled** [SG2 B-H1]):

````markdown
<!-- release-notes:begin -->
For macOS 10.12 or later on Intel (`x86_64`) and macOS 11.0 or later on Apple
Silicon (`arm64`). **macOS 10.12–10.15 are best-effort:** each release is built
for them and checked to declare that minimum, but is not tested on them every
time, and Apple's current tools no longer state support for 10.12.

### Which file to download

Every file contains the **same program**, byte for byte: one build, one signed
executable per architecture, and the release job checked that the copy inside
the `.pkg` and the `.dmg` is identical to the bare file. Take `arm64` on Apple
Silicon and `x86_64` on Intel — `uname -m` in Terminal tells you which.

| File | What it is | What happens when you use it |
|---|---|---|
| `macportsbrew-X.Y.Z-<arch>` | The bare executable: one signed program file | Nothing happens until you place it. Rename it to `macportsbrew`, make it executable, and put it in a folder on your `PATH` — the Terminal line below does all three. Nothing else on your Mac changes. |
| `macportsbrew-X.Y.Z-<arch>.pkg` | A macOS installer package, for your user account only | Double-click it. It is designed to put `macportsbrew` in the hidden folder `.macportsbrew/bin` inside your home folder, without asking for an administrator password (being confirmed on real installs before the first release). It does not change your `PATH` or any shell file; see "Putting it on your `PATH`". It does not install the short name `mpb`. |
| `macportsbrew-X.Y.Z-<arch>.dmg` | A macOS disk image holding the executable | Double-click it and a window opens with one file in it. Drag it to a folder you choose. Finder hides folders whose names start with a dot, so to use `~/.macportsbrew/bin`, first run `mkdir -p ~/.macportsbrew/bin` in Terminal, then in Finder press Shift-Command-G, type `~/.macportsbrew/bin`, and drag the file into the window that opens. |

**Not sure? Take the `.pkg`.** **Live in a terminal? This one line downloads,
names, places and marks the program executable:**

```
mkdir -p ~/.macportsbrew/bin && curl -fL -o ~/.macportsbrew/bin/macportsbrew "https://github.com/macportsbrew/macportsbrew/releases/download/vX.Y.Z/macportsbrew-X.Y.Z-$(uname -m)" && chmod +x ~/.macportsbrew/bin/macportsbrew
```

### Putting it on your `PATH`

Add this line to `~/.zprofile` if you use zsh (the default since macOS 10.15),
or to `~/.bash_profile` if you use bash (the default on 10.12–10.14), then open
a new Terminal window:

```
export PATH="$PATH:$HOME/.macportsbrew/bin"
```

It adds the folder at the end, so it never hides a program you already have.

### The short name `mpb`

No release installs `mpb`: another program, MIT Photonic-Bands, uses that name.
If `command -v mpb` prints nothing, you may add it yourself with
`ln -s macportsbrew ~/.macportsbrew/bin/mpb`. If it prints a path, leave it.

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
gh attestation verify macportsbrew-X.Y.Z-<arch> --repo macportsbrew/macportsbrew --signer-workflow macportsbrew/macportsbrew/.github/workflows/release.yml --source-ref refs/tags/vX.Y.Z
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
````

### The secret-material check (commit 3)

`scripts/check-no-secrets.sh`, a `validate.sh --quick` stage with gate tests:
fails on any tracked file with extension `.p12`, `.pfx`, `.p8`, `.cer`,
`.mobileprovision` or `.keychain*`; on a PEM armour line for a private key or a
certificate (five dashes, `BEGIN`, then `PRIVATE KEY` with an optional
algorithm word, or `CERTIFICATE`; this plan does not spell the line out, so
that it cannot trip the check it describes) with no allow-list; on a file named
like an App Store Connect key (`AuthKey_[A-Z0-9]{10}\.p8`); and on a
team-identifier-shaped value assigned to a team key
(`(TEAM_ID|TeamIdentifier)[=:][[:space:]]*"?[A-Z0-9]{10}`). The workflow names
its secrets `MPB_*_CERT_B64`, writes `notary-key.p8`, and contains none of
these patterns. The gate tests build each pattern into a fixture file at
runtime so that this repository never contains one (CLAUDE.md, hooks).

### The release-workflow check (commit 3, armed; D-T2) [SR M1, SG L3]

The workflow MUSTs of 08 and ADR-0005 were only written down; nothing failed if
a later pass broke one. `scripts/check-release-workflow.sh`, a `validate.sh
--quick` stage, asserts over `.github/workflows/*.y{,a}ml` **and every
composite action under `.github/actions/`**, comment lines ignored: no
`pull_request_target` or `workflow_run` trigger; no `ACTIONS_STEP_DEBUG` or
`ACTIONS_RUNNER_DEBUG`; no `secrets[…]` and no `toJSON(secrets)`;
`release-signing` and `secrets.MPB_*` in `release.yml` only. Over
`release.yml` [SG2 B-M1, SR2 B-L2]: `on:` is exactly `push` with `tags:
["v*"]` and `workflow_dispatch`; exactly one line selects the environment, as
`environment: release-signing`; the job holding it contains no `cargo`,
`rustup`, Rust toolchain action, `validate.sh` or `run-fixture-matrix`;
`secrets.` appears inside that job only; and no xtrace in any spelling — a
`set` whose short-option clusters contain `x` (`set -x`, `set -eux`, `set -e
-x`), `xtrace`, `SHELLOPTS`, `BASH_XTRACEFD`, `bash -x`, a `shell:` with `-x`.
"No xtrace at all" is the rule that guarantees "none in a step that sees a
credential". `MPB_WORKFLOW_DIR` and `MPB_ACTIONS_DIR` let the gate tests run it
over constructed directories.
The full text is staged as an embedded block in
[`09-staged/README.md`](09-staged/README.md) ("The workflow check"), with its
SHA-256, so the armed write is mechanical. Observed 2026-10-11 from the
scratchpad copy (`ab444b43…`, 147 lines): shellcheck clean; it accepts the
current workflows plus the staged `release.yml` and refuses each of 24
constructed violations.

## Commits

1. **docs** — this plan, both reviews folded in; DP-031, DP-033, DP-034,
   DP-035 and DP-036 in `docs/decisions-pending.md` (written at revision);
   `docs/spec-evolution.md` entries from "Proposed spec-evolution text" below
   (merged by the main session onto the existing ADR-0005/DP-004 topic and
   06's surface topic, per SE-032); R-11 §Q2 and the checklist's R-11 line
   corrected by reference to R-19 f3; DP-019's runner cost marked void by R-19
   (the owner's ruling stands). **Owed research, before the workflow is relied
   on [SG M6]:** an R-19 addendum citing GitHub's artifact-attestation
   documentation with a read date (SE-014's condition) for
   `attest-build-provenance`'s `subject-checksums` and `subject-path`, and for
   `gh attestation verify --repo`, `--signer-workflow` and `--source-ref`; and
   `gh`'s minimum macOS, stated from its release notes rather than inferred
   from ADR-0002's Go fact. A second addendum, for DP-034 (b), observes
   `security -i` reading commands from stdin on a throwaway keychain in a
   disposable account. These, with the offline-Gatekeeper outcome (R-11 Q3)
   and `shasum --ignore-missing` on 10.12, are on the owner checklist's
   "checks before the first tag" wherever the notes depend on them
   [SG2 B-L3].
2. **cli** — `--version` metadata; clap `string` feature; tests (below);
   `Cargo.lock` unchanged. **No `long_about` `mpb` change** until DP-033 is
   ruled [SG2 B-H1].
3. **scripts (armed)** — guarded writes first: `scripts/build-release.sh`;
   `scripts/run-fixture-matrix.sh` gains `BIN="${MPB_FIXTURE_BIN:-target/debug/macportsbrew}"`
   (one line; the default unchanged); `scripts/check-action-pins.sh` adds the
   three action repositories to `ALLOWED_REPOS`; `scripts/check-no-secrets.sh`
   and `scripts/check-release-workflow.sh` with their `validate.sh` stages
   (header list updated there, and only there); `scripts/test-gates.sh` cases,
   including the sign-path no-execution test, the `--expect-unsigned-sha256`
   refusals and the 24 constructed `check-release-workflow` cases. Hook tests
   last.
4. **workflows (armed)** — `.github/workflows/release.yml` as staged, after
   commit 3's allow-list change (`check-action-pins.sh` scans `.github/` and
   would otherwise fail on the three new repositories). `actionlint` 2026-10-10
   on the staged file: clean. `scripts/support-matrix.sh` reads only
   `repository-validation.yml`, so the release workflow's labels do not enter
   the published matrix — correct, since it builds rather than tests.
5. **dry run** — done 2026-10-11: unsigned `x86_64-apple-darwin` of the
   revised script at `39b33e5`, exit 0 (fact 17); results in the pass report.
6. **docs** — README Install section with the release-notes markers (text
   above, minus the parts held for DP-033), `wiki/Getting-Started.md` linking
   to it, best-effort wording [SG M2, SG M3]; **no `mpb` text change** until
   DP-033 is ruled [SG2 B-H1]; 08 sharpened per D-R1 (no change to a MUST's
   force); CHANGELOG. **Lands before the first dispatch dry run** of the
   release workflow, whose notes step needs the markers [SG2 B-L1]; being
   unguarded, it may land before commits 3–4. **Written to the working tree
   2026-10-11** for review; README's text says 10.12–10.15 "and 11–14" are
   best-effort, matching its own "Supported machines" section, which the
   proposal above had omitted.
7. **fixes** from the end-of-pass reviews; `/pass-report`.

## Named tests

- **The script's own assertions** (fact 15 ran the first version): floor exact
  by otool and vtool; thin; system libraries only; no host path; `-V` /
  `--version` (fresh unsigned build only; full metadata with
  `--release-input`); signature identifier, runtime flag, Developer ID
  authority, team, timestamp; pkg archive list, domains, install location, no
  scripts, payload list, BOM modes (exact-match filter), payload hash; dmg
  HFS+, volume list, attributes, hash, UDZO; with `--sign` notarisation
  `Accepted` (bounded wait), staple valid, installer signature and
  notarisation line, Developer ID authority, team and timestamp on the stapled
  `.dmg`, Gatekeeper acceptance of both containers, payloads re-asserted after
  stapling; `SHA256SUMS.<arch>` verifies.
- **cli:** `short_version_is_name_and_semver` (`-V` → `macportsbrew
  <CARGO_PKG_VERSION>`); `long_version_names_policy_target_and_commit` (four
  lines, in order; policy equals the embedded version; target is an
  `-apple-darwin` triple); `a_developer_build_says_it_is_not_a_release_build`
  (commit line when `MPB_BUILD_COMMIT` was unset at compile time);
  `a_usage_error_keeps_its_exit_code` (an unknown flag exits with the same code
  as before the switch to `from_arg_matches`) [SG L8].
- **gate tests** (`scripts/test-gates.sh`, added with the armed write):
  - `check-no-secrets` rejects each constructed pattern and accepts the tree;
  - `check-action-pins` accepts the three new repositories pinned and still
    rejects them unpinned;
  - the release script contains no package-manager command and no
    privilege-escalation command word (a text check over
    `scripts/build-release.sh`, like the hooks' own fixtures-in-files rule);
  - `build_release_never_executes_the_input_on_the_sign_path` [SR H1, SG H1]:
    runs `build-release.sh` with `--sign --expect-unsigned-sha256 <the stub's
    own SHA-256> --from-binary <stub>` (and, a second case, `--from-binary`
    alone), where the stub is an executable shell script that writes a marker
    file when run. `PATH` is prefixed with stubs: `lipo`,
    `otool`, `vtool` and `strings` answer as a valid binary of the host
    architecture would, so the run gets past `assert_binary` to the execution
    block; `security` lists both identities; `codesign` fails, which ends the
    run just after that block; the `--sign` environment points at fixture
    files (a keychain placeholder, a `0600` key file). Asserts the marker was
    never written. Without the stubs the run would die at `lipo` before the
    block and prove nothing. It also asserts,
    textually, that every `"$bin"` invocation in the script sits inside the
    branch guarded by `[ "$sign" = 1 ] || [ -n "$from_binary" ]`'s `else`;
  - `check-release-workflow` accepts the tree and refuses, in constructed
    directories, the 24 cases listed in `09-staged/README.md` ("The workflow
    check"), at least one per rule: the environment elsewhere or selected
    twice; `secrets.MPB_*` in another workflow or a composite action;
    `secrets.` outside the signing job; `secrets[…]`, `toJSON(secrets)`;
    `cargo`, `rustup`, `validate.sh`, `run-fixture-matrix` in the signing job;
    each xtrace spelling; an extra trigger, a wrong tag filter, a `branches`
    filter, `workflow_run`, `pull_request_target`, `ACTIONS_STEP_DEBUG`
    [SR M1, SG L3, SG2 B-M1, SR2 B-L2]. All 24 observed 2026-10-11 from the
    scratchpad copy.
- **Invariants [SG L1]:** **no invariant in `docs/spec/01-invariants.md` is
  touched by this pass.** INV-05 is "Installing a package manager requires
  explicit consent" — this pass installs no package manager; INV-06 (least
  privilege) governs the running process, and ADR-0005 places the installer
  outside it. The properties this pass does assert — no package-manager
  command and no elevation in the release script, a current-user-only
  distribution — are release-engineering requirements of 08 and ADR-0005,
  checked by the gate test and the script above, not invariant tests. No
  planner, adapter, executor or policy file changes; `crates/cli` changes only
  the version string (the `mpb` help text waits on DP-033).
- **Binding refusals [SR2 B-L1, SG2 B-L2]**, gate tests with the armed write:
  `--sign` without `--expect-unsigned-sha256` fails (exit 1); a malformed hex
  fails; `--expect-unsigned-sha256` without `--sign` is a usage error (exit 2)
  — all three observed 2026-10-11 from the scratchpad copy; and, with the same
  PATH stubs as the no-execution test, a `--from-binary` input whose hash
  differs from the expected value fails before the stub `security` is called
  (the stub records any call).

## Decisions (Tier 3, for `docs/spec-evolution.md`)

- **D-B1** One `cargo build --target T` per architecture with
  `MACOSX_DEPLOYMENT_TARGET` on that invocation; asserted on the artifact,
  exactly equal to the floor, by `otool` and `vtool`.
- **D-B2** *(owner, Tier 2: DP-033 [SG H2, SG2 B-H1])* The staged release
  artifacts install `macportsbrew` only; `mpb` is not built for release and
  not installed by any artifact (ADR-0005). This contradicts the **charter**
  (`00-charter.md` line 8: "with the short alias **`mpb`** installed alongside
  it"), 06's MUST, ADR-0003 and the owner's DP-005 ruling, so it is a charter
  amendment needing the owner's explicit decision, not an agent's Tier 3 call.
  **Reversible default: change no text; the first tag is blocked until DP-033
  is ruled.** Any approved wording distinguishes release artifacts from cargo
  builds, which still produce `mpb` (`crates/cli/Cargo.toml`, `[[bin]] name =
  "mpb"`).
- **D-B3** Release flags: the two path remaps; Xcode 26.5 named by
  `DEVELOPER_DIR`, spelled once in the workflow's top-level `env` with its
  source [SG L4]; `--locked`; no `-no_uuid`, `-random_uuid` or post-link
  strip. The floors 10.12 / 11.0 are spelled in the script, in
  `check-deployment-target.sh` and in the `rust-toolchain.toml` comment — three
  copies; reading them from one place is owed, not done in this pass [SG L4].
- **D-R1** The byte-identity claim, stated precisely (fact 5): (a) is the 08
  MUST and is asserted; (b) is a documented, conditional property of the
  unsigned executable, not a CI assertion, and `SIGNING-RECORD.txt` publishes
  what an attempt needs — the unsigned build's SHA-256, the deployment floor
  and the **build** job's toolchain (`build_*` lines), not the signing VM's
  [SR L5, SG M5]; (c) and (d) are never claimed.
- **D-P1** *(owner: DP-035 [SG M7])* `.pkg` installs to `~/.macportsbrew/bin`
  (current-user-home domain, `--install-location /.macportsbrew`); the
  documented line appends to `PATH`. Permanent once users follow it, so routed
  with D-S3; reversible default the current value. "No administrator password,
  user-owned `0755`" is **unverified** until the owner's Standard-account check
  is recorded, and the documentation says so conditionally until then [SR M4,
  SG M4].
- **D-P2** Unsigned component inside a signed product archive; system and
  anywhere domains off; per-architecture `hostArchitectures`; `os-version
  min` = floor; no scripts.
- **D-P3** Payload list exact; the provenance AppleDouble entry (fact 15) is
  tolerated only in an unsigned run and only when its bytes decode to one
  `com.apple.provenance` attribute; a signed run tolerates none. If the first
  CI dry run shows the hosted runner stamps provenance too, the signed run will
  refuse, and that is the point at which a way to build the payload without it
  must be found — not a reason to relax the signed check.
- **D-D1** `.dmg`: one file, HFS+ for both architectures (one code path; 11.0
  mounts HFS+), UDZO, signed with the Application identity.
- **D-S1** *(owner: DP-034 [SG M1])* A **deviation** from ADR-0005's text
  ("the temporary keychain password … Every credential reaches its tool by
  file or stdin, never in `argv`"), not a reading of it: per-job random
  keychain and re-wrap passwords reach `security`'s argv, masked and unset in
  the one step; the owner's `.p12` passwords and notary key never do. Options
  (a) accept on the ephemeral VM and amend ADR-0005, (b) `security -i` from
  stdin (unverified; research owed), (c) another route. Staged default (a).
  The credential step resets `PATH`, uses absolute tool paths and unsets each
  secret after its consumer [SR L3].
- **D-S2** Hardened runtime, `--timestamp`, explicit `-i`; no entitlements
  (R-19 f1).
- **D-S3** *(owner: DP-035, after DP-031 [SG L5])* Code-signing and package
  identifier `io.github.macportsbrew.macportsbrew`, from the workspace
  `repository` URL. It is permanent once a release ships (designated
  requirement, Installer upgrade matching). It assumes a GitHub organisation
  `macportsbrew` that may not be available or the owner's — the repository's
  account is DP-031, decided first.
- **D-S4** An unsigned dry run ad-hoc signs with the same identifier and the
  runtime flag, so it exercises the signing and verification path.
- **D-N1** Notarise the ZIP of the bare executable, the `.pkg` and the `.dmg`,
  each wait bounded by `--timeout 30m` [SR L4]; staple the two containers;
  assert Developer ID authority, team and timestamp on the final `.dmg`,
  pkgutil's notarisation line on the `.pkg`, and Gatekeeper acceptance of both
  [SR M5, SG H3]; checksums after stapling.
- **D-W1** Workflow triggers and job split as in the table; a dispatch is
  always a dry run and its bundle says so; a tag must be on `main`; every job
  has a timeout; the release is drafted, never published.
- **D-W2** *(restated [SR H1, SG H1, SR L1])* Build and sign in separate jobs.
  Compilation and every execution of the built program happen on the build
  VM, which holds no secret; `build-release.sh` never executes its input with
  `--sign` or `--from-binary`, so the signing VM runs no code compiled from
  this repository or its dependencies — only Apple's tools, `checkout`,
  `download-artifact` and this repository's shell. The signing job binds its
  input to the build job's record by `unsigned_build_sha256` (plus
  `release_input=yes`, `version_check=full`, the commit) **before** creating
  the keychain, and `build-release.sh` binds it again itself
  (`--expect-unsigned-sha256`, required with `--sign`) before consulting the
  keychain or an identity [SR2 B-L1, SG2 B-L2]; `assemble`'s re-check is a consistency check against
  accidental drift, not the binding, and neither defends against a
  compromised build VM, which writes both the executable and its record.
- **D-W3** Three new action repositories, pinned as fact 14, allow-listed in
  `check-action-pins.sh`; versions at least six weeks old where a newer patch
  existed. ADR-0004's gate still does not cover actions (ADR-0005's open task).
- **D-V1** `--version` shape as designed; `-V` unchanged; the version block is
  a fixed English machine-read contract, exempt from the i18n MUST by a
  recorded SE exemption (proposed below) [SG L6].
- **D-T1** `check-no-secrets.sh` as designed.
- **D-T2** `check-release-workflow.sh` as designed (a `validate.sh --quick`
  stage with gate tests), turning 08's and ADR-0005's textual workflow MUSTs
  into a failing check [SR M1, SG L3].
- **D-A1** Verification is pinned to the release workflow
  (`--signer-workflow`, `--source-ref`), and `SIGNING-RECORD.txt` is attested
  beside the artifacts and `SHA256SUMS` [SR M3, SG L2]. Relying on the
  attestation features waits on the dated citation owed in commit 1 [SG M6].

## Could not verify here

- How Installer resolves `--install-location /.macportsbrew` in the
  current-user-home domain, and whether 10.12's Installer honours the domain
  for a signed product archive (R-19). `installer -dominfo` is denied in this
  session; the owner's check is in `09-staged/README.md`.
- The credential import on a hosted runner [SR2 B-L5]: `/usr/bin/openssl`
  (LibreSSL) re-wrapping an OpenSSL-3 `.p12` through two process
  substitutions, `security import` of the result, and the partition-list step.
  Only the LibreSSL decrypt and re-wrap were observed here, with a throwaway
  self-signed key and no keychain. The owner observes the whole sequence once
  with a throwaway `.p12` on a runner before the first tag
  (`09-staged/README.md`, checks before the first tag, step 3); it fails closed
  under `set -euo pipefail`, so nothing is signed if it does not work.
- The `sign` job's base-repository condition (`github.repository ==
  '<owner>/<repo>'`): owed once DP-031 names the repository [SG2 B-L5]; noted
  in the workflow beside the job's `if:` and in DP-031.
- Whether hosted runners stamp `com.apple.provenance` (D-P3); whether
  `/Applications/Xcode_26.5.app` is the exact path on both images (the build
  job fails loudly if not); whether `shellcheck` and `jq` are on the macOS
  images (the existing validation workflow already assumes so).
- Everything that needs a Developer ID identity: notarisation, stapling, the
  nested-ticket question (fact 4), two Developer ID signings sharing a CDHash.
- An arm64 dry run: this machine is x86_64 and cannot execute the arm64
  executable; the arm64 path was not built here.
- The fixture matrix against the *release* executable: it needs the one-line
  `MPB_FIXTURE_BIN` change to a guarded script, so it was not run; that debug
  and release builds produce the same golden output is expected, not observed.
- Unsigned reproducibility from a different checkout path or another machine
  (the root remap is meant to make the path irrelevant; only one path was
  tried).
- The staged workflow has never run: it is checked by `actionlint` only
  (1.7.12, 2026-10-10, clean after the revision).
- The revised script's arm64 build and its signed path (fact 17: only the
  unsigned x86_64 path was dry-run) [SR2 B-L4].
- The wording `pkgutil --check-signature` prints for a notarised package
  (`trusted by the Apple notary service`) and the `spctl` source line
  (`source=Notarized Developer ID`) — expected, not observed; `spctl` is denied
  in this session. A mismatch fails closed on the first tag run.
- That `security find-identity -v <keychain>` (no policy) lists a Developer ID
  Installer identity as valid.
- Whether the release `.pkg` installs with no administrator password, into the
  home folder, user-owned `0755` — the owner's Standard-account check [SR M4,
  SG M4].
- What a quarantined bare executable does on first run with no network (R-11
  Q3) [SG L7]; whether `shasum -c --ignore-missing` exists on 10.12's `shasum`
  [SG L7].
- GitHub's attestation features and `gh`'s minimum macOS, not yet cited with a
  date (owed, commit 1) [SG M6].
- `security -i` reading commands from stdin (DP-034 (b)).
- Whether a GitHub organisation `macportsbrew` is available (DP-031).

## Proposed spec-evolution text

For the main session to merge into `docs/spec-evolution.md` per SE-032 (onto
existing topics where one exists; identifiers left to the merge). Each item is
an entry's body.

1. **Release artifacts install `macportsbrew` only; cargo builds keep `mpb`;
   release users create the alias themselves** *(a charter amendment: written
   only if the owner rules DP-033 (a); onto the ADR-0003 / SE-023 topic)*
   [SG2 B-H1].
   - Original: the charter, `00-charter.md` line 8 (Tier 2): "**CLI command:**
     `macportsbrew`, with the short alias **`mpb`** installed alongside it";
     `06-cli-and-ux.md` (Surface, MUST): "`mpb` is installed alongside it as an
     equal short alias"; ADR-0003; DP-005.
   - Change (if ruled (a)): release artifacts — the bare file, the `.pkg`, the
     `.dmg` — install `macportsbrew` only. Cargo builds are unchanged:
     `crates/cli/Cargo.toml` keeps `[[bin]] name = "mpb"`, so `cargo build`
     and `cargo install` still produce both names. The release install
     documentation tells the user to create `mpb` by hand only if `command -v
     mpb` prints nothing. Proposed charter line, for the owner to approve or
     reword: "**CLI command:** `macportsbrew`; the short alias `mpb` is built
     alongside it from source, and release installs document how to add it."
     06's MUST reworded to: "`mpb` is an equal alias for the same binary. A
     cargo build produces both names. No release artifact installs `mpb`; the
     install documentation describes creating it and tells the user to check
     for an existing `mpb` first." ADR-0003's decision gains the same
     distinction; README, `wiki/Getting-Started.md` and the `--help`
     `long_about` follow it.
   - Rationale: the guard ADR-0003 requires cannot run inside a script-free
     package, and an unguarded alias would shadow MIT Photonic-Bands' `mpb` for
     a user whose `PATH` puts `~/.macportsbrew/bin` first — the shadowing this
     tool exists to report. Documenting the alias keeps the guard, performed by
     the user, at no cost to anyone who never types `mpb`.
   - Status: proposed; nothing is written, and the first tag is blocked, until
     the owner rules DP-033. Pass 09.

2. **The release signing step's temporary passwords** *(pending DP-034; onto
   the ADR-0005 / DP-004 topic)*.
   - Original: ADR-0005, "Every credential reaches its tool by file or stdin,
     never in `argv`", naming the temporary keychain password.
   - Change (if DP-034 (a)): two per-job random values — the temporary
     keychain password and a `.p12` re-wrap password — may reach `security`'s
     argv on the ephemeral hosted VM, registered with `::add-mask::`, generated
     and unset inside the one credential step, on a VM where nothing built from
     the repository is executed. The owner's `.p12` passwords and the App Store
     Connect key never reach argv. If (b) holds, no change is needed: `security
     -i` keeps both values out of argv and the ADR stands as written.
   - Rationale: GitHub's documented recipe puts both passwords in argv (R-19
     f18); the values are worthless after the VM is destroyed; the residual
     exposure is to processes on that VM, which the build/sign split empties.
   - Status: proposed; DP-034 decides it. Pass 09.

3. **The distribution, as pass 09 builds it** *(onto the ADR-0005 / DP-004 /
   SE-067 topic)*: one `cargo build --target T` per architecture with
   `MACOSX_DEPLOYMENT_TARGET` on that invocation, asserted on the artifact
   (D-B1); the byte-identity claim stated as fact 5 (D-R1: (a) asserted; (b)
   conditional, with the build toolchain published in `SIGNING-RECORD.txt`;
   (c), (d) never claimed); the `.pkg` shape and install directory (D-P1, D-P2,
   pending DP-035 for the directory), the `.dmg` shape (D-D1); hardened
   runtime, no entitlements (D-S2); the identifier (D-S3, pending DP-035); the
   notarisation set and final-artifact assertions (D-N1); the job split, with
   **the built program never executed on the signing VM and the hand-off bound
   by hash before the keychain exists** (D-W1, D-W2); three pinned action
   repositories (D-W3); verification pinned to the release workflow and the
   signing record attested (D-A1); `check-no-secrets.sh` (D-T1) and
   `check-release-workflow.sh`, which makes 08's textual workflow MUSTs a
   failing check (D-T2). Rationale and sources: this plan and R-19.

4. **The version block is exempt from the i18n MUST** *(onto 06's i18n
   topic)*.
   - Original: 06, "All user-facing text goes through the i18n layer".
   - Change: the output of `--version` / `-V` (`macportsbrew <semver>`, then
     `policy`, `target` and `commit` lines, and `unknown (not a release
     build)`) is a fixed English, machine-read contract — release scripts and
     the release job parse it — and is exempt. The exemption is that block
     only; clap's help text is a separate, still-open gap, recorded here rather
     than silently covered.
   - Rationale: translating a parsed contract would break its parsers; the
     words are identifiers more than prose.
   - Status: proposed. Pass 09.

5. **Best-effort is said in those words in the install text** *(onto 08's
   supported-machines topic; no change to a MUST's force)*: the README Install
   section and the release notes generated from it say 10.12–10.15 "are
   best-effort", with R-19 f13's reason (the 10.12 floor is held by `ld`, not
   by an Apple support statement).

## Review

`security-reviewer` and `spec-guardian` reviewed the draft on 2026-10-10:
SR — 1 High, 5 Medium, 6 Low; SG — 3 High, 8 Medium, 8 Low. Neither approved
it as staged. The revision below answers every finding; both should re-review
the revision before the armed write.

Second round, 2026-10-11, on the revision: `spec-guardian` B-H1, B-M1,
B-L1–B-L3, B-L5; `security-reviewer` B-L1–B-L5. Folded on 2026-10-11 and cited
as `[SG2 …]` / `[SR2 …]`; the last nine rows of the table below.

## Revision after review

| Finding | Disposition |
|---|---|
| [SR H1] / [SG H1] sign job executes the release binary with the keychain unlocked and the notary key on disk | **Fixed.** The script never executes `$bin` with `--sign` or `--from-binary`; only a fresh unsigned build runs it, and `--release-input` requires full metadata there. The sign job compares `shasum -a 256` of `in/raw/macportsbrew` with the build record's `unsigned_build_sha256` (and requires `release_input=yes`, `version_check=full`, the commit) before creating the keychain. Gate test `build_release_never_executes_the_input_on_the_sign_path` planned for the armed write. D-W2 restated. |
| [SR L1] assemble cross-check proves less than claimed | **Fixed.** Binding moved to `sign`, before credentials; `assemble`'s check reworded as a consistency re-check; D-W2 says neither defends against a compromised build VM. |
| [SG H2] D-B2 overrides 06, ADR-0003, DP-005 | **Routed to the owner: DP-033**; D-B2 marked *(owner)*; hand-alias instructions check `command -v mpb` first; SE text proposed (item 1). The first-round default and text corrections are **superseded by [SG2 B-H1]** below. |
| [SR M5] / [SG H3] Developer ID authority not asserted on the final `.dmg` | **Fixed.** `assert_developer_id` on the stapled `.dmg` (authority, team, timestamp); `spctl -t install` on the `.pkg`, `spctl -t open --context context:primary-signature` on the `.dmg`; pkgutil notarisation line asserted. Wordings expected, not observed; fail closed. |
| [SR M1] / [SG L3] workflow MUSTs unenforced | **Planned, text staged.** `check-release-workflow.sh` (D-T2), a `validate.sh --quick` stage with four gate-test cases, in the armed batch; full text and SHA-256 embedded in `09-staged/README.md`; run against the current workflows and three constructed violations. |
| [SR M2] tag, environment, draft protections under-specified | **Fixed.** Owner checklist: ruleset Active, bypass owner only (no GitHub Apps incl. Claude, no deploy keys); environment required reviewer = owner, admin bypass off; verify draft assets with `gh attestation verify --signer-workflow` and compare to `SHA256SUMS` before publishing. `guard` refuses a tag whose commit is not an ancestor of `main`. |
| [SR M3] / [SG L2] verification trusts every workflow; signing record unattested | **Fixed.** Snippet adds `--signer-workflow …/release.yml` and `--source-ref refs/tags/vX.Y.Z`; `assemble` fails if the README snippet omits them; `SIGNING-RECORD.txt` attested in `draft`. |
| [SR M4] / [SG M4] `.pkg` "no admin password, user-owned" unverified | **Owner check specified; text made conditional.** Standard non-admin account, signed and dry-run `.pkg`, oldest macOS; `stat -f '%Su %Sp'` of the directory, `bin`, binary; receipt location; requirement user-owned `0755`, not group-writable; recorded in R-19. The install text says "designed to … being confirmed" until then; a script assertion follows the observation. |
| [SG M1] D-S1 is a deviation, not a reading | **Routed to the owner: DP-034**, quoting ADR-0005; options (a) argv on the ephemeral VM, (b) `security -i` from stdin (unverified; research owed), (c) other. Plan and workflow comment reworded. SE text proposed (item 2). |
| [SR L3] credential step resolves tools from `PATH`; secrets inherited | **Fixed.** `PATH` reset to system directories; absolute tool paths; each secret unset right after its consumer; `::add-mask::` for both random passwords. |
| [SG M2] notes miss steps; "one source" is copies | **Fixed (plan + workflow); README markers land in commit 6.** Notes generated in `assemble` from README's marked section; YAML holds no copy; wiki links to README; text adds rename, `chmod +x`, one complete `curl` line, visible `.dmg` target or Finder steps. |
| [SG M3] best-effort not said | **Fixed in the proposed README text**; SE text proposed (item 5). |
| [SR L5] / [SG M5] signing record lacks build toolchain | **Fixed.** `SIGNING-RECORD.txt` gains `deployment_floor` and `build_*` lines (unsigned SHA-256, version check, host macOS, developer dir, Xcode, linker, rustc) from the build record; records carry `host_role`; D-R1 corrected. |
| [SG M6] attestation features uncited; `gh` likely needs macOS 12 | **Owed research** (R-19 addendum, commit 1) before the workflow is relied on; notes say `gh` probably needs macOS 12 and to verify on any machine. |
| [SG M7] D-P1 hard to change | **Routed to the owner with D-S3: DP-035**, default the current value. |
| [SG M8] start SHA; other-session commits | **Fixed.** Start SHA `ea46a1f` recorded; dry-run hashes attributed to it; `ffdd3b3` and further slice-3 commits excluded from the §3 check, which runs over this pass's listed commits. |
| [SR L2] dry-run bundle notes claim signed/notarised | **Fixed.** Dispatch bundle named `dry-run-unsigned-bundle`; its notes open with a DRY RUN banner. |
| [SR L4] no job timeouts; unbounded notarytool wait | **Fixed.** `timeout-minutes` on every job; `notarytool submit --timeout 30m` (documented by `notarytool submit --help`, observed 2026-10-10). |
| [SR L6] relative paths, Installer identity, BOM filter, AppleDouble bounds | **Fixed.** Paths resolved against the invocation directory; Installer identity checked with `find-identity -v` (policy-less form unverified on a runner); exact-match `awk` BOM filter; decoder bounds member size (152–4096 bytes) and attribute value (≤ 64 bytes, inside the member). |
| [SG L1] invariant labelling | **Fixed.** Named tests now say no invariant is touched, and why INV-05 and INV-06 are not the governing rules. |
| [SG L4] values spelled more than once | **Partly fixed.** `DEVELOPER_DIR` hoisted to top-level `env` with its source; the floors' three copies recorded as owed (D-B3), not unified in this pass. |
| [SG L5] DP-019's ".app"; GitHub org assumed | **Routed to the owner: DP-036** (".app" means `.dmg`?) and **DP-031** (repository account); D-S3 notes its dependency on DP-031. |
| [SG L6] `--version` strings outside i18n | **Exemption proposed** (SE item 4); D-V1 updated. |
| [SG L7] offline Gatekeeper unknown; `--ignore-missing` on 10.12; shell files | **Fixed in the proposed text:** the unknown offline outcome is stated with the `curl`/`.pkg` alternative; a by-eye fallback for `shasum`; `~/.zprofile`/`~/.zshrc` and `~/.bash_profile` named. |
| [SG L8] usage-error exit code after `from_arg_matches` | **Planned:** `a_usage_error_keeps_its_exit_code` (commit 2). |
| [SG2 B-H1] the charter (Tier 2, `00-charter.md` line 8) says `mpb` is installed alongside | **Routed to the owner as a charter amendment: DP-033 rewritten.** Reversible default now "change no text; the first tag is blocked until the ruling". All `mpb` text corrections removed from commits 2 and 6; the proposed README text's `mpb` parts held; SE item 1 rewritten to distinguish release artifacts from cargo builds (`[[bin]] name = "mpb"` in `crates/cli/Cargo.toml`) and to propose a charter line for the owner. "DP-033 ruled" is on the owner's before-first-tag list. |
| [SG2 B-M1] / [SR2 B-L2] strengthen the workflow check | **Done, staged.** `check-release-workflow.sh` (`ab444b43…`, 147 lines): one `environment: release-signing` selection, release.yml only; the signing job contains no `cargo`, `rustup`, toolchain action, `validate.sh`, `run-fixture-matrix`; `secrets.` only inside that job; xtrace in every listed spelling; `secrets[` and `toJSON(secrets` refused everywhere; `.github/actions/**` scanned; `on:` pinned to `push` (`tags: ["v*"]`) and `workflow_dispatch`. Shellcheck clean; accepts the tree; refuses all 24 constructed cases (observed 2026-10-11); re-embedded with the new SHA-256. |
| [SR2 B-L1] / [SG2 B-L2] move the binding into the script | **Done.** `--expect-unsigned-sha256 <hex>`, required with `--sign` and refused without it, compared right after the copy-in and before any keychain, key-file or identity check (those checks moved after it). The workflow passes the record's value and **keeps** its earlier check, because the credential step creates the keychain before calling the script; the plan's "no check of its own" principle is replaced by an honest list of the run-level checks the workflow holds. |
| [SR2 B-L3] / [SG2 B-L1] literal snippet check; markers before the first dry run | **Done.** `assemble` requires `--repo $REPO`, `--signer-workflow $REPO/.github/workflows/release.yml` and `--source-ref refs/tags/v$VERSION` literally; the plan and the staged README state that commit 6's markers land before the first dispatch dry run. |
| [SG2 B-L3] owed research on the owner checklist | **Done.** The attestation citation, `gh`'s minimum macOS, `security -i` (if DP-034 (b)), the offline Gatekeeper outcome and `shasum --ignore-missing` on 10.12 are step 4 of the checks before the first tag. |
| [SR2 B-L5] LibreSSL re-wrap of an OpenSSL-3 `.p12` unobserved on a runner | **Owner check added** (step 3, a throwaway `.p12` on a runner); stated to fail closed; listed under "Could not verify". |
| [SG2 B-L5] base-repository condition for `sign` | **Owed once DP-031 is ruled**, noted in the plan, beside the job's `if:` in the staged workflow, in DP-031, and on the owner's list. |
| [SR2 B-L4] the revised script is not dry-run | **Done 2026-10-11** by the coordinator: unsigned `x86_64-apple-darwin` at `39b33e5`, exit 0, hashes in fact 17; arm64 and the signed path remain unexercised ("Could not verify"). |
