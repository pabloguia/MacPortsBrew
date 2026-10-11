<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 09 — staged guarded files

These are the guarded writes of pass 09
([plan](../09-release-engineering-plan.md)), staged here because the
SE-093 escape was not armed when they were drafted (work order §3). Nothing in
this directory is executed by any gate or workflow: `.github/workflows/` and
`scripts/` are the only places those run from.

| Staged | Applies to | How |
|---|---|---|
| [`workflows/release.yml`](workflows/release.yml) | `.github/workflows/release.yml` | copy verbatim |
| "The script", below | `scripts/build-release.sh` | write verbatim, `chmod 0755`; SHA-256 below |
| "The workflow check", below | `scripts/check-release-workflow.sh` | write verbatim, `chmod 0755`; SHA-256 below |

Both scripts are fenced blocks in this file rather than files under
`09-staged/scripts/` because the path guard matches `*/scripts/*.sh` anywhere in
the tree and refused that write. Each was checked from a byte-identical copy in
the session scratchpad: `build-release.sh`'s first version was dry-run (plan,
fact 15), and the current revision (`c8d3b0f5…`) was dry-run unsigned for
`x86_64-apple-darwin` on 2026-10-11 (plan, fact 17) — exit 0, every assertion
passing; the arm64 build and the signed path are not exercised.
`check-release-workflow.sh` was run over a copy of `.github/workflows/` plus
the staged `release.yml` (passes) and over 24 constructed violations (each
refused).

## Applying them — one armed message, guarded writes first, hook tests last

The owner sends one message containing `MPB_ALLOW_GUARD_EDIT=1` and asking for
commit 3 and commit 4 of the plan. In that message the agent:

1. writes `scripts/build-release.sh` from the block below and checks
   `shasum -a 256 scripts/build-release.sh` against
   `c8d3b0f510fe82687643c6d5b7649aa590fbcadde73dd15af39b8a3b4fc59e67`;
   `chmod 0755`;
2. edits `scripts/run-fixture-matrix.sh`:
   `BIN="${MPB_FIXTURE_BIN:-target/debug/macportsbrew}"` (one line);
3. adds `actions/upload-artifact`, `actions/download-artifact` and
   `actions/attest-build-provenance` to `ALLOWED_REPOS` in
   `scripts/check-action-pins.sh`;
4. writes `scripts/check-no-secrets.sh` and its `validate.sh` stage (plan,
   "The secret-material check");
5. writes `scripts/check-release-workflow.sh` from the block below, checks
   `shasum -a 256` against
   `ab444b43555bf565123a89288aa35df0f8184a46a50f79e55fd67c378431d7c3`,
   `chmod 0755`, and adds its `validate.sh --quick` stage (plan, D-T2) —
   updating the stage list in `validate.sh`'s header, and nowhere else, once
   for both new stages;
6. adds the gate tests to `scripts/test-gates.sh` (plan, "Named tests"),
   including `build_release_never_executes_the_input_on_the_sign_path`, the
   `--expect-unsigned-sha256` refusals, and the 24 constructed
   `check-release-workflow` cases ("The workflow check", below);
7. copies `workflows/release.yml` to `.github/workflows/release.yml`;
8. runs `.claude/hooks/test-hooks.sh`, then `scripts/validate.sh`, then
   `actionlint .github/workflows/release.yml` if present;
9. records every guarded file it wrote in the end-of-pass report (the
   disclosed-escape rule).

The release workflow's notes step reads README.md's
`<!-- release-notes:begin -->` / `<!-- release-notes:end -->` section and fails
without it, so commit 6's README Install section must land before the first
dispatch run (plan, "Release notes and install documentation").

Commit 2 (`--version` metadata, `crates/cli`) is not guarded and lands before
these, so the script's `--version` check is satisfied when it first runs in CI.

## Owner checklist for the first real release

Nothing here is done by an agent: each item creates or handles a credential, or
spends or commits something of yours.

**The repository (DP-007, DP-031).** Create the public GitHub repository and
push. The workspace `repository` URL says `macportsbrew/macportsbrew`; whether
that organisation is available or yours is DP-031, and if it is not, the
workspace field, README links and D-S3's identifier change first. In Settings →
General, enable **immutable releases** (08 MUST).

**The protected tag.** The workflow that runs is the one at the tagged commit,
so whoever can create a `v*` tag chooses the code that receives the signing
secrets; the ruleset is the barrier. Settings → Rules → Rulesets → new tag
ruleset targeting `v*` (08 MUST, ADR-0005):

- **Enforcement status: Active** (not Evaluate, not Disabled).
- Restrict creation, update and deletion.
- **Bypass list: you only.** No GitHub App — explicitly not the Claude GitHub
  App, whose pushes trigger workflows (`claude.yml` runs with `contents:
  write`) — no deploy key, and no repository-role bypass if anyone else is ever
  an administrator.

The `guard` job additionally refuses a tag whose commit is not an ancestor of
`main`; that is defence in depth, not a substitute for the ruleset.

**The environment.** Settings → Environments → `release-signing`:

- Deployment branches and tags: **selected tags only**, rule `v*`.
- **Required reviewers: you.** Every signing run waits for your approval.
- **"Allow administrators to bypass configured protection rules": off.**
- No other workflow may name this environment (08 MUST);
  `scripts/check-release-workflow.sh` fails `validate.sh` if one does.

**Secrets in that environment (names only; never as repository secrets):**

| Name | What it holds |
|---|---|
| `MPB_APP_CERT_B64` | the Developer ID **Application** certificate and private key, exported as a `.p12`, base64-encoded |
| `MPB_APP_CERT_PASSWORD` | that `.p12`'s export password |
| `MPB_INSTALLER_CERT_B64` | the Developer ID **Installer** certificate and private key, `.p12`, base64 |
| `MPB_INSTALLER_CERT_PASSWORD` | its export password |
| `MPB_NOTARY_KEY` | the App Store Connect API private key (the downloaded `.p8` file's contents) |
| `MPB_NOTARY_KEY_ID` | that key's Key ID |
| `MPB_NOTARY_ISSUER` | the Issuer ID (team keys; leave unset for an individual key) |

**Variables in that environment (not secret; they appear on every signature):**
`MPB_TEAM_ID`, `MPB_APP_IDENTITY` (`Developer ID Application: <name> (<team>)`),
`MPB_INSTALLER_IDENTITY` (`Developer ID Installer: <name> (<team>)`).

Setting them without the value passing through a command line or the clipboard:
`base64 -i <file>.p12 | gh secret set MPB_APP_CERT_B64 --env release-signing`,
`gh secret set MPB_NOTARY_KEY --env release-signing < <key>.p8`, and
`gh secret set MPB_APP_CERT_PASSWORD --env release-signing` with no value, which
prompts. Delete the exported `.p12` and the `.p8` afterwards, or keep them only
where you keep other private keys.

**The certificates.** In the Apple Developer account (Certificates, Identifiers
& Profiles): one **Developer ID Application** and one **Developer ID
Installer** certificate (DP-019: the `.pkg` needs the second). Export each from
Keychain Access with its private key as a `.p12` with a strong password.

**The notary key.** App Store Connect → Users and Access → Integrations → Team
Keys: a key with the Developer role (ADR-0005 prefers it over an Apple ID and
app-specific password). The `.p8` downloads once.

**Decisions to make before the first tag** (plan, Decisions;
[`decisions-pending.md`](../../decisions-pending.md)):

- **DP-031** — which GitHub account the repository lives under.
- **DP-035** (D-S3, D-P1) — the identifier `io.github.macportsbrew.macportsbrew`
  and the install directory `~/.macportsbrew/bin` are permanent once a release
  ships. Confirm them or name others.
- **DP-034** (D-S1) — per-job random keychain and re-wrap passwords reach
  `security`'s argv, which ADR-0005's text forbids; accept that on the
  ephemeral VM, or have `security -i` (stdin) researched first.
- **DP-033 ruled** (D-B2) — **the first tag is blocked until you rule it.** The
  charter (Tier 2, `00-charter.md` line 8) says `mpb` is "installed alongside"
  `macportsbrew`, and no release artifact installs it; shipping without your
  ruling would contradict the charter. No text has been changed meanwhile
  [SG2 B-H1].
- **DP-036** — whether DP-019's ".app" means the `.dmg`.
- **DP-031, then the sign job's repository condition** — once the repository
  is named, add `github.repository == '<owner>/<repo>'` to the `sign` job's
  `if:` (a workflow edit, guarded) [SG2 B-L5].

**Checks before the first tag.**

1. Run the workflow by hand (Actions → Release → Run workflow, `dry_run`
   ticked). It builds, packages and asserts both architectures unsigned and
   drafts nothing; its bundle is named `dry-run-unsigned-bundle` and its notes
   open with a DRY RUN banner. Read the `build` logs for `TOLERATED (dry run
   only)`: if the hosted runner stamps `com.apple.provenance`, the signed run
   will refuse (plan, D-P3), and that has to be solved before tagging.
2. **The `.pkg` check, which decides what the release notes may claim.** In a
   **Standard (non-administrator) user account** — a disposable one or a VM,
   not your own — on the oldest macOS you can reach (10.12 if possible, and at
   least one of 10.12–10.15 and one of 11+): `installer -dominfo -pkg <file>`
   (read-only), then double-click the package. Do it with the dry-run `.pkg`
   now, and with the **signed** `.pkg` from the first draft before publishing.
   Record, in R-19, with the date and macOS version:
   - where it landed (expected `~/.macportsbrew/bin/macportsbrew`, not
     `/.macportsbrew`);
   - whether it asked for an administrator password (expected: no);
   - `stat -f '%Su %Sp' ~/.macportsbrew ~/.macportsbrew/bin
     ~/.macportsbrew/bin/macportsbrew` — required: owned by that user, `0755`
     directories and executable, nothing group- or world-writable (08);
   - where the receipt went (`pkgutil --pkgs` and `pkgutil --pkg-info
     io.github.macportsbrew.macportsbrew`, run as that user);
   - that no `PATH` or shell file changed.

   Until this is recorded, README's Install section (and so the notes) states
   the "no administrator password, in your home folder" behaviour as expected,
   not as fact (plan, [SR M4]/[SG M4]). If it fails, the `.pkg` does not ship
   until it is fixed.
3. **The credential import, observed once on a runner [SR2 B-L5].** The sign
   step re-wraps each `.p12` with `/usr/bin/openssl` (LibreSSL) reading the
   OpenSSL-3 export through two process substitutions, then imports it with
   `security import` and sets the partition list. On this machine LibreSSL
   decrypted an OpenSSL-3.6 PBES2 `.p12` and the re-wrap round-tripped, but
   only with a self-signed throwaway key and no keychain; the whole sequence
   has never run on a hosted runner. Observe it once with a **throwaway**
   self-signed `.p12` (never the Developer ID one) — for example in a private
   scratch repository's workflow running the same step text — before the first
   tag. It fails closed: an unreadable `.p12` or a failed import stops the
   step under `set -euo pipefail` and nothing is signed.
4. **Research the release depends on [SG2 B-L3]**, each recorded with a date
   (an R-19 addendum) before the first tag:
   - GitHub's artifact-attestation documentation for `attest-build-provenance`
     (`subject-checksums`, `subject-path`) and `gh attestation verify`
     (`--repo`, `--signer-workflow`, `--source-ref`) — the notes and the
     publishing check above rely on them (SE-014's condition);
   - `gh`'s minimum macOS, from its own release notes (the notes say "probably
     macOS 12");
   - `security -i` reading commands from stdin — only if you choose DP-034 (b);
   - what a quarantined bare executable does on first run with no network
     (R-11 Q3) — the notes say it is not established; the research decides
     whether they can say more;
   - whether 10.12's `shasum` accepts `--ignore-missing` — the notes give a
     fallback either way.
   None of these changes a byte of any artifact; each changes what the notes
   may claim.

**The tag command** (from a clean `main` at the commit to release, after the
version in `Cargo.toml` and the CHANGELOG say `0.1.0`):

```
git tag -a v0.1.0 -m "MacPortsBrew 0.1.0"
git push origin v0.1.0
```

The workflow drafts the release. A draft is mutable — any `contents: write`
token can replace its assets — so before publishing:

1. Download every asset of the draft into an empty directory.
2. Verify each one, the signing record included, against **the release
   workflow at that tag**, not merely the repository:

   ```
   gh attestation verify <file> \
     --repo <owner>/<repo> \
     --signer-workflow <owner>/<repo>/.github/workflows/release.yml \
     --source-ref refs/tags/vX.Y.Z
   ```

   (`gh` probably needs macOS 12 or later — plan, [SG M6]; run it on any
   machine.)
3. Compare the downloaded files with `shasum -a 256 -c SHA256SUMS`, and the
   `SHA256SUMS` file with the one printed in the `assemble` job's step summary.
4. Read the notes and the signing record — eight files: three per
   architecture, `SHA256SUMS`, `SIGNING-RECORD.txt`.

Then publish it by hand. Publishing makes it immutable; a mistake afterwards
burns the version number (08).

**Runner cost (DP-019).** Standard GitHub-hosted runners are free and unlimited
on public repositories (GitHub Docs, read 2026-10-10, R-19 f3). Each release
uses four macOS jobs on `macos-26-intel` and `macos-26` (two build, two sign)
and three `ubuntu-latest` jobs, none on a `-large` or `-xlarge` label, so a
release costs nothing while the repository is public. DP-019's ruling assumed
the Intel runner was billed; R-19 overturned that, so the only recurring
release cost left is the Apple Developer Program membership you already hold.
What macOS minutes would cost on a private repository was not researched.

## The script

`scripts/build-release.sh`, SHA-256
`c8d3b0f510fe82687643c6d5b7649aa590fbcadde73dd15af39b8a3b4fc59e67`, 680
lines, shellcheck 0.11.0 clean, `bash -n` clean; revised after two rounds of
review and dry-run unsigned for `x86_64-apple-darwin` on 2026-10-11 at
`39b33e5` (exit 0; plan, fact 17). The first version, `dfaf2625…6fd6f8bf0`, is
the one fact 15 ran; the first revision was `7fd52400…`.

```bash
#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Builds, optionally signs and notarises, and packages one architecture of a
# MacPortsBrew release (docs/spec/08-distribution-and-project.md, Distribution;
# ADR-0005; the plan is docs/passes/09-release-engineering-plan.md).
#
#   scripts/build-release.sh <target-triple> <out-dir> [--release-input]
#                            [--sign --expect-unsigned-sha256 HEX]
#                            [--from-binary PATH]
#
#   <target-triple>   x86_64-apple-darwin (floor macOS 10.12) or
#                     aarch64-apple-darwin (floor macOS 11.0)
#   <out-dir>         created if absent; must be empty
#   --release-input   this unsigned, freshly built executable is the input to a
#                     signed release: the tree must be clean and --version must
#                     carry the build metadata. The release workflow's build
#                     job passes it on a tag push; the signing job refuses a
#                     record that lacks it. Not combinable with --sign or
#                     --from-binary.
#   --sign            Developer ID signing, notarisation and stapling. Without
#                     it the executable is ad-hoc signed and nothing is
#                     notarised: the artifacts are a dry run, never published.
#   --expect-unsigned-sha256 HEX
#                     required with --sign, refused without: the SHA-256 the
#                     build job recorded as unsigned_build_sha256. The input
#                     is compared with it right after it is copied in, before
#                     the keychain, the key file or any identity is consulted.
#   --from-binary P   package an already-built, unsigned executable instead of
#                     running cargo. The release workflow builds in a job that
#                     holds no secret and signs in a separate job that runs no
#                     dependency build script (plan, D-W2). The input is
#                     asserted statically exactly as a fresh build is, and is
#                     NEVER executed ("Executing the build", below).
#
# A relative <out-dir> or --from-binary path is resolved against the directory
# the script was invoked from, before it changes to the repository root.
#
# What it produces in <out-dir>, for <arch> in x86_64 | arm64:
#
#   macportsbrew-<version>-<arch>        the bare executable
#   macportsbrew-<version>-<arch>.pkg    installs ONLY `macportsbrew`, into
#                                        ~/.macportsbrew/bin, current-user
#                                        domain, no PATH edit, no script
#   macportsbrew-<version>-<arch>.dmg    UDZO, HFS+, holding the executable
#   SHA256SUMS.<arch>                    the three, in `shasum -a 256` format
#   macportsbrew-<version>-<arch>.release-record.txt
#                                        what was built and, when signed, what
#                                        signed it (08: team identifier,
#                                        certificate fingerprint, notarisation
#                                        submission per artifact)
#
# What it asserts, and refuses to finish without:
#
#   - the recorded minimum macOS equals the floor, read twice (otool, vtool),
#     and the executable is thin for the one architecture;
#   - it links only system libraries and carries no host path;
#   - the signature on the final executable: identifier, hardened runtime, and
#     with --sign a Developer ID Application authority, the team, a secure
#     timestamp;
#   - the payload extracted from the .pkg (pkgutil --expand, then tar) and from
#     the .dmg (read-only, unbrowsed attach) hashes to the bare executable AND
#     the payload file list and modes are exactly the expected ones (R-19 found
#     an AppleDouble `._` entry a hash comparison alone does not see);
#   - with --sign: notarisation Accepted for the ZIP of the bare executable,
#     the .pkg and the .dmg, and a valid staple on the two containers; on the
#     final stapled .dmg a Developer ID Application authority, the team and a
#     timestamp; pkgutil reporting the .pkg notarised; Gatekeeper (spctl)
#     accepting both containers as notarised Developer ID. The checksums are
#     taken after stapling, which changes a container's bytes.
#
# Environment read with --sign (names only; no value is ever printed, and no
# credential is ever put in argv):
#
#   MPB_SIGN_KEYCHAIN       path to a temporary keychain holding both identities
#   MPB_APP_IDENTITY        "Developer ID Application: <name> (<team>)"
#   MPB_INSTALLER_IDENTITY  "Developer ID Installer: <name> (<team>)"
#   MPB_TEAM_ID             the team identifier both must carry
#   MPB_NOTARY_KEY_PATH     App Store Connect API private key file, mode 0600
#   MPB_NOTARY_KEY_ID       its key identifier
#   MPB_NOTARY_ISSUER       its issuer (team keys only; omit for individual keys)
#
# What it never does: invoke either package manager, request administrator
# rights, install anything, edit PATH or any shell configuration, write outside
# <out-dir> and one mktemp directory it removes on exit, or publish anything.

set -euo pipefail

usage() {
  echo "usage: $0 <x86_64-apple-darwin|aarch64-apple-darwin> <out-dir> [--release-input] [--sign --expect-unsigned-sha256 HEX] [--from-binary PATH]" >&2
  exit 2
}
die()  { printf 'build-release: FAIL: %s\n' "$*" >&2; exit 1; }
note() { printf 'build-release: %s\n' "$*"; }

[ "$#" -ge 2 ] || usage
target="$1"
out_arg="$2"
shift 2
sign=0
release_input=0
from_binary=""
expect_unsigned=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --sign) sign=1 ;;
    --release-input) release_input=1 ;;
    --from-binary)
      [ "$#" -ge 2 ] || usage
      from_binary="$2"
      shift
      ;;
    --expect-unsigned-sha256)
      [ "$#" -ge 2 ] || usage
      expect_unsigned="$2"
      shift
      ;;
    *) usage ;;
  esac
  shift
done
if [ "$release_input" = 1 ] && { [ "$sign" = 1 ] || [ -n "$from_binary" ]; }; then
  usage
fi
# --sign and --expect-unsigned-sha256 come together or not at all.
if [ "$sign" = 1 ]; then
  printf '%s' "$expect_unsigned" | grep -Eqx '[0-9a-f]{64}' \
    || die "--sign needs --expect-unsigned-sha256 <64 lowercase hex>, the build job's unsigned_build_sha256"
elif [ -n "$expect_unsigned" ]; then
  usage
fi

# Relative paths mean relative to where the caller stands, not to the
# repository root this script changes into below.
invoked_from="$(pwd)"
case "$out_arg" in /*) ;; *) out_arg="$invoked_from/$out_arg" ;; esac
if [ -n "$from_binary" ]; then
  case "$from_binary" in /*) ;; *) from_binary="$invoked_from/$from_binary" ;; esac
fi

case "$target" in
  x86_64-apple-darwin)  floor="10.12"; arch="x86_64" ;;
  aarch64-apple-darwin) floor="11.0";  arch="arm64" ;;
  *) die "unsupported target '$target'" ;;
esac

# The identifier is forever: codesign's designated requirement and the macOS
# Installer's upgrade matching both key on it (R-19 finding 10; pkgbuild(1)).
# Derived from the workspace `repository` URL; owner decision D-S3 in the plan.
readonly IDENTIFIER="io.github.macportsbrew.macportsbrew"
# Interpreted relative to the user's home directory, because the distribution
# below enables only the current-user-home domain (plan, D-P1). R-19 lists the
# exact interpretation as UNRESOLVED; the plan carries the check that settles it.
readonly INSTALL_LOCATION="/.macportsbrew"

need=(otool vtool lipo codesign pkgbuild productbuild pkgutil lsbom hdiutil
      shasum tar xmllint git strings ditto)
if [ -z "$from_binary" ]; then need+=(cargo rustc); fi
if [ "$sign" = 1 ]; then need+=(xcrun security plutil spctl); fi
for tool in "${need[@]}"; do
  command -v "$tool" >/dev/null 2>&1 || die "required tool not found: $tool"
done

# The repository this script lives in; a staged copy outside it resolves the
# repository from the working directory instead.
root="$( (cd "$(dirname "${BASH_SOURCE[0]}")" && git rev-parse --show-toplevel 2>/dev/null) \
         || git rev-parse --show-toplevel)" || die "cannot find the repository root"
cd "$root"

version="$(sed -n '/^\[workspace\.package\]/,/^\[/ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' Cargo.toml | head -1)"
policy_version="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' data/policy/policy.toml | head -1)"
[ -n "$version" ] || die "no [workspace.package] version in Cargo.toml"
[ -n "$policy_version" ] || die "no version in data/policy/policy.toml"
commit="$(git rev-parse HEAD)"
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  [ "$sign" = 0 ] || die "tracked files are modified; a signed release is built from a clean commit"
  [ "$release_input" = 0 ] || die "tracked files are modified; a release input is built from a clean commit"
  note "WARNING: tracked files are modified; this unsigned build is not the commit it names"
  commit="$commit-modified"
fi

mkdir -p "$out_arg"
out="$(cd "$out_arg" && pwd)"
[ -z "$(ls -A "$out")" ] || die "$out is not empty"

base="macportsbrew-$version-$arch"
work="$(mktemp -d "${TMPDIR:-/tmp}/mpb-release.XXXXXX")"
mnt=""
cleanup() {
  if [ -n "$mnt" ]; then
    hdiutil detach -quiet "$mnt" 2>/dev/null || hdiutil detach -force -quiet "$mnt" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

if [ "$sign" = 1 ]; then
  for var in MPB_SIGN_KEYCHAIN MPB_APP_IDENTITY MPB_INSTALLER_IDENTITY MPB_TEAM_ID \
             MPB_NOTARY_KEY_PATH MPB_NOTARY_KEY_ID; do
    [ -n "${!var:-}" ] || die "--sign needs $var in the environment"
  done
  # Names only here. The keychain, the key file and the identities are not
  # touched until the input has been bound to the expected bytes (step 1).
fi

# --- 1. Build -------------------------------------------------------------------

if [ -z "$from_binary" ]; then
  if [ -n "${RUSTFLAGS:-}${CARGO_ENCODED_RUSTFLAGS:-}" ]; then
    die "RUSTFLAGS or CARGO_ENCODED_RUSTFLAGS is set; the release flags are this script's alone"
  fi
  cargo_home="${CARGO_HOME:-$HOME/.cargo}"
  # Host paths out of the executable (R-19 findings 7-8). The unit separator
  # lets a path contain a space, which RUSTFLAGS cannot carry.
  encoded="$(printf '%s\037%s' \
    "--remap-path-prefix=$cargo_home/registry/src=/cargo-registry" \
    "--remap-path-prefix=$root=/macportsbrew")"
  build_log="$work/cargo-build.log"
  note "building $target with MACOSX_DEPLOYMENT_TARGET=$floor (this invocation only)"
  # One cargo invocation per architecture, the deployment target set on that
  # invocation and nowhere else (R-19 finding 12).
  if ! MACOSX_DEPLOYMENT_TARGET="$floor" CARGO_ENCODED_RUSTFLAGS="$encoded" \
       MPB_BUILD_TARGET="$target" MPB_BUILD_COMMIT="$commit" \
       cargo build --release --locked -p macportsbrew --bin macportsbrew --target "$target" \
       >"$build_log" 2>&1; then
    tail -40 "$build_log" >&2
    die "cargo build failed"
  fi
  if grep -qiE 'built for newer|was built for|deployment target' "$build_log"; then
    grep -iE 'built for newer|was built for|deployment target' "$build_log" >&2
    die "the linker or rustc warned about the deployment target"
  fi
  built="${CARGO_TARGET_DIR:-$root/target}/$target/release/macportsbrew"
else
  built="$from_binary"
fi
[ -f "$built" ] || die "no executable at $built"

bin="$work/macportsbrew"
ditto --norsrc --noextattr --noqtn --noacl "$built" "$bin"
chmod 0755 "$bin"
unsigned_sha="$(shasum -a 256 "$bin" | awk '{print $1}')"

# The binding: what is about to be signed is exactly what the build job built,
# executed and asserted. Checked on the copy this script will sign, before the
# keychain, the key file or any identity is consulted.
if [ "$sign" = 1 ]; then
  [ "$unsigned_sha" = "$expect_unsigned" ] \
    || die "the input hashes to $unsigned_sha, not the expected unsigned build $expect_unsigned"
  note "input bound to the expected unsigned build ($unsigned_sha)"
  [ -f "$MPB_SIGN_KEYCHAIN" ] || die "MPB_SIGN_KEYCHAIN is not a file"
  [ -f "$MPB_NOTARY_KEY_PATH" ] || die "MPB_NOTARY_KEY_PATH is not a file"
  [ "$(stat -f '%Lp' "$MPB_NOTARY_KEY_PATH")" = "600" ] || die "the notary key file must be mode 0600"
  [ "$(stat -f '%u' "$MPB_NOTARY_KEY_PATH")" = "$(id -u)" ] || die "the notary key file must be owned by the signing user"
  security find-identity -v -p codesigning "$MPB_SIGN_KEYCHAIN" | grep -qF "\"$MPB_APP_IDENTITY\"" \
    || die "the identity named in MPB_APP_IDENTITY is not valid in MPB_SIGN_KEYCHAIN"
  # An Installer identity is not a code-signing one, so no -p codesigning here.
  security find-identity -v "$MPB_SIGN_KEYCHAIN" | grep -qF "\"$MPB_INSTALLER_IDENTITY\"" \
    || die "the identity named in MPB_INSTALLER_IDENTITY is not valid in MPB_SIGN_KEYCHAIN"
fi

# --- 2. What the executable records ------------------------------------------------

recorded_min_otool() {
  otool -l "$1" | awk '
    /cmd LC_VERSION_MIN_MACOSX/ { want=1 }
    /cmd LC_BUILD_VERSION/      { want=1 }
    want && /^ *version /       { print $2; exit }
    want && /^ *minos /         { print $2; exit }'
}
recorded_min_vtool() {
  vtool -show-build "$1" | awk '
    /LC_VERSION_MIN_MACOSX/ { want=1 }
    /LC_BUILD_VERSION/      { want=1 }
    want && /^ *version /   { print $2; exit }
    want && /^ *minos /     { print $2; exit }'
}

assert_binary() { # <file> <label>
  local f="$1" label="$2" archs by_otool by_vtool dylib host_hits
  archs="$(lipo -archs "$f")"
  [ "$archs" = "$arch" ] || die "$label: architectures '$archs', expected exactly '$arch'"
  by_otool="$(recorded_min_otool "$f")"
  by_vtool="$(recorded_min_vtool "$f")"
  [ "$by_otool" = "$floor" ] || die "$label: otool records minimum macOS '$by_otool', expected $floor"
  [ "$by_vtool" = "$floor" ] || die "$label: vtool records minimum macOS '$by_vtool', expected $floor"
  while IFS= read -r dylib; do
    case "$dylib" in
      /usr/lib/*|/System/Library/*) ;;
      *) die "$label: links a non-system library: $dylib" ;;
    esac
  done < <(otool -L "$f" | awk 'NR > 1 { print $1 }')
  host_hits="$(strings -a "$f" | grep -cF -e "$HOME/" -e "$root/" || true)"
  [ "$host_hits" = 0 ] || die "$label: $host_hits strings carry a host path"
  note "$label: $archs, minimum macOS $by_otool (otool) = $by_vtool (vtool), system libraries only, no host path"
}

assert_binary "$bin" "unsigned build"

# EXECUTING THE BUILD, AND WHERE IT IS NEVER DONE
# -----------------------------------------------
# A compromised dependency lives in the executable as well as in build
# scripts, and runs on `-V` (argument parsing, static initialisers). So the
# executable is run only on a fresh cargo output in an unsigned run - the
# build job, which holds no secret. With --sign or --from-binary it is never
# executed: the signing job has an unlocked keychain and the notary key on
# disk. The bytes it signs are bound to the bytes the build job ran by
# unsigned_build_sha256, which the workflow compares before the keychain
# exists (plan, D-W2). The gate test `build_release_never_executes_the_input_
# on_the_sign_path` holds this block to that (plan, Named tests).
version_check="not-executed"
if [ "$sign" = 1 ] || [ -n "$from_binary" ]; then
  note "not executing the input (--sign or --from-binary); the build job checked --version on these bytes"
elif [ "$(uname -m)" != "$arch" ]; then
  [ "$release_input" = 0 ] || die "--release-input needs a host that can execute the $arch executable"
  note "host is $(uname -m); not executing the $arch executable"
else
  short="$("$bin" -V)"
  [ "$short" = "macportsbrew $version" ] || die "-V printed '$short', expected 'macportsbrew $version'"
  long="$("$bin" --version)"
  if [ "$(printf '%s\n' "$long" | wc -l | tr -d ' ')" -gt 1 ]; then
    printf '%s\n' "$long" | grep -qxF "policy $policy_version" || die "--version does not name policy $policy_version"
    printf '%s\n' "$long" | grep -qxF "target $target"         || die "--version does not name target $target"
    printf '%s\n' "$long" | grep -qxF "commit $commit"         || die "--version does not name commit $commit"
    note "--version names version, policy, target and commit"
    version_check="full"
  elif [ "$release_input" = 1 ]; then
    die "--version carries no build metadata; a release input requires it (plan, commit 2)"
  else
    note "NOTE: --version carries no build metadata yet (plan, commit 2); accepted for an unsigned dry run only"
    version_check="short-only"
  fi
fi

# --- 3. Sign the executable, once, before any container ----------------------------

if [ "$sign" = 1 ]; then
  note "signing with the Developer ID Application identity, hardened runtime, secure timestamp"
  codesign --sign "$MPB_APP_IDENTITY" --keychain "$MPB_SIGN_KEYCHAIN" \
    --identifier "$IDENTIFIER" --options runtime --timestamp --force "$bin"
else
  note "NOT SIGNED WITH DEVELOPER ID: ad-hoc signature only; nothing here may be published"
  codesign --sign - --identifier "$IDENTIFIER" --options runtime --timestamp=none --force "$bin"
fi

codesign --verify --strict --verbose=2 "$bin"
sig="$(codesign -dvvv "$bin" 2>&1)"
# Logged on purpose: it discloses only what every published artifact carries
# (ADR-0005).
printf '%s\n' "$sig"
printf '%s\n' "$sig" | grep -qxF "Identifier=$IDENTIFIER" || die "signature identifier is not $IDENTIFIER"
printf '%s\n' "$sig" | grep -E '^CodeDirectory ' | grep -q 'runtime' || die "the hardened runtime flag is not set"
cdhash="$(printf '%s\n' "$sig" | sed -n 's/^CDHash=//p' | head -1)"
team="not signed"
cert_sha="not signed"
if [ "$sign" = 1 ]; then
  printf '%s\n' "$sig" | grep -q '^Authority=Developer ID Application: ' || die "authority is not Developer ID Application"
  printf '%s\n' "$sig" | grep -qxF "TeamIdentifier=$MPB_TEAM_ID" || die "team identifier is not the expected team"
  printf '%s\n' "$sig" | grep -q '^Timestamp=' || die "no secure timestamp in the signature"
  team="$MPB_TEAM_ID"
  mkdir "$work/certs"
  (cd "$work/certs" && codesign -d --extract-certificates "$bin" >/dev/null 2>&1)
  [ -f "$work/certs/codesign0" ] || die "could not extract the signing certificate"
  cert_sha="$(shasum -a 256 "$work/certs/codesign0" | awk '{print $1}')"
else
  printf '%s\n' "$sig" | grep -qx 'Signature=adhoc' || die "expected an ad-hoc signature in a dry run"
fi
assert_binary "$bin" "signed executable"

# The same three facts, asserted on any final signed artifact (08: Developer ID
# authority on the *final* artifact, not merely that a signature exists). Used
# for the stapled .dmg below; the executable is checked inline above.
assert_developer_id() { # <file> <label>
  local s
  s="$(codesign -dvvv "$1" 2>&1)"
  printf '%s\n' "$s"
  printf '%s\n' "$s" | grep -q '^Authority=Developer ID Application: ' || die "$2: authority is not Developer ID Application"
  printf '%s\n' "$s" | grep -qxF "TeamIdentifier=$MPB_TEAM_ID" || die "$2: team identifier is not the expected team"
  printf '%s\n' "$s" | grep -q '^Timestamp=' || die "$2: no secure timestamp in the signature"
}
signed_sha="$(shasum -a 256 "$bin" | awk '{print $1}')"

ditto --norsrc --noextattr --noqtn --noacl "$bin" "$out/$base"
chmod 0755 "$out/$base"

# --- 4. The .pkg: one file, one directory, the current user's home ----------------

pkgroot="$work/pkgroot"
mkdir -p "$pkgroot/bin" "$work/components"
chmod 0755 "$pkgroot" "$pkgroot/bin"
ditto --norsrc --noextattr --noqtn --noacl "$bin" "$pkgroot/bin/macportsbrew"
chmod 0755 "$pkgroot/bin/macportsbrew"
# The component stays unsigned; the product archive carries the signature
# (productbuild(1), R-19 finding 14).
pkgbuild --root "$pkgroot" --identifier "$IDENTIFIER" --version "$version" \
  --install-location "$INSTALL_LOCATION" --ownership recommended --quiet \
  "$work/components/macportsbrew.pkg"

dist="$work/distribution.xml"
cat >"$dist" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>MacPortsBrew $version ($arch)</title>
    <options customize="never" require-scripts="false" hostArchitectures="$arch"/>
    <domains enable_anywhere="false" enable_currentUserHome="true" enable_localSystem="false"/>
    <allowed-os-versions>
        <os-version min="$floor"/>
    </allowed-os-versions>
    <choices-outline>
        <line choice="macportsbrew"/>
    </choices-outline>
    <choice id="macportsbrew" visible="false" title="macportsbrew">
        <pkg-ref id="$IDENTIFIER"/>
    </choice>
    <pkg-ref id="$IDENTIFIER" version="$version" onConclusion="none">macportsbrew.pkg</pkg-ref>
</installer-gui-script>
EOF
xmllint --noout "$dist"

if [ "$sign" = 1 ]; then
  productbuild --distribution "$dist" --package-path "$work/components" \
    --sign "$MPB_INSTALLER_IDENTITY" --keychain "$MPB_SIGN_KEYCHAIN" --timestamp \
    "$out/$base.pkg"
else
  productbuild --distribution "$dist" --package-path "$work/components" "$out/$base.pkg"
fi

# --- 5. The .dmg: HFS+, because the x86_64 floor is 10.12 (R-19 finding 16) ----------

dmgroot="$work/dmgroot"
mkdir "$dmgroot"
chmod 0755 "$dmgroot"
ditto --norsrc --noextattr --noqtn --noacl "$bin" "$dmgroot/macportsbrew"
chmod 0755 "$dmgroot/macportsbrew"
hdiutil create -quiet -volname "MacPortsBrew $version $arch" -srcfolder "$dmgroot" \
  -fs HFS+ -format UDZO -ov "$out/$base.dmg"
if [ "$sign" = 1 ]; then
  codesign --sign "$MPB_APP_IDENTITY" --keychain "$MPB_SIGN_KEYCHAIN" \
    --identifier "$IDENTIFIER.dmg" --timestamp "$out/$base.dmg"
  codesign --verify --strict --verbose=2 "$out/$base.dmg"
fi

# --- 6. Payload assertions -----------------------------------------------------------

expect_list() { # <label> <expected, newline-separated> <actual, newline-separated>
  if [ "$2" != "$3" ]; then
    printf 'expected:\n%s\nactual:\n%s\n' "$2" "$3" >&2
    die "$1 differs from what was put in"
  fi
}

# THE ONE TOLERANCE, AND WHY IT IS NARROW
# ---------------------------------------
# On a machine that stamps every new file with `com.apple.provenance` - which
# cannot be removed (R-19 E11) - pkgbuild archives that attribute as an
# AppleDouble `._<name>` entry beside each file and directory. It changes no
# executable byte but it adds payload entries, and the Installer would restore
# the attribute. An unsigned dry run tolerates such an entry only when its
# bytes decode to exactly that: the AppleDouble layout copyfile writes, an
# all-zero Finder info, an empty resource fork and ONE attribute named
# `com.apple.provenance`. Anything else fails. A signed release tolerates
# nothing (plan, D-P3).
readonly ZERO32="0000000000000000000000000000000000000000000000000000000000000000"
PROVENANCE_HEX="$(printf 'com.apple.provenance' | od -An -tx1 -v | tr -d ' \n')00"
readonly PROVENANCE_HEX
appledouble_provenance_only() { # <payload archive> <member>
  local hex voff vlen
  hex="$(tar -xOf "$1" "$2" | od -An -tx1 -v | tr -d ' \n')"
  [ "${#hex}" -ge 304 ]  || return 1                    # long enough for every field read below
  [ "${#hex}" -le 8192 ] || return 1                    # at most 4096 bytes in all
  [ "${hex:0:16}"   = "0005160700020000" ] || return 1  # AppleDouble, version 2
  [ "${hex:48:4}"   = "0002" ]             || return 1  # two entries
  [ "${hex:52:24}"  = "000000090000003200000071" ] || return 1  # Finder info + attributes
  [ "${hex:76:8}"   = "00000002" ]         || return 1  # resource fork entry
  [ "${hex:92:8}"   = "00000000" ]         || return 1  # ... of length zero
  [ "${hex:100:64}" = "$ZERO32" ]          || return 1  # Finder info all zero
  [ "${hex:168:8}"  = "41545452" ]         || return 1  # "ATTR"
  [ "${hex:236:4}"  = "0001" ]             || return 1  # exactly one attribute
  [ "${hex:260:2}"  = "15" ]               || return 1  # name length 21
  [ "${hex:262:42}" = "$PROVENANCE_HEX" ]  || return 1  # com.apple.provenance
  # The attribute's value: offset and length (bytes 120-127) stay inside the
  # file, and the value is small (a provenance value is a few bytes).
  voff=$((16#${hex:240:8}))
  vlen=$((16#${hex:248:8}))
  [ "$vlen" -le 64 ] || return 1
  [ $(( (voff + vlen) * 2 )) -le "${#hex}" ] || return 1
}

tolerated=""
check_pkg_payload_list() { # <pass> <payload archive>
  local pass="$1" payload="$2" entry expected actual kept=""
  expected="$(printf '%s\n' . ./bin ./bin/macportsbrew)"
  actual="$(tar -tf "$payload" | sort)"
  tolerated=""
  while IFS= read -r entry; do
    if printf '%s\n' "$expected" | grep -qxF -- "$entry"; then
      kept="$kept$entry"$'\n'
      continue
    fi
    case "$entry" in
      ./._bin|./bin/._macportsbrew) ;;
      *) printf 'expected:\n%s\nactual:\n%s\n' "$expected" "$actual" >&2
         die "pkg ($pass): unexpected payload entry $entry" ;;
    esac
    appledouble_provenance_only "$payload" "$entry" \
      || die "pkg ($pass): $entry carries more than com.apple.provenance"
    [ "$sign" = 0 ] || die "pkg ($pass): $entry - a signed release carries no AppleDouble entry (plan, D-P3)"
    tolerated="$tolerated$entry"$'\n'
  done <<<"$actual"
  expect_list "pkg ($pass) payload" "$expected" "${kept%$'\n'}"
  if [ -n "$tolerated" ]; then
    note "TOLERATED (dry run only): provenance-only AppleDouble entries in the pkg payload: $(printf '%s' "$tolerated" | tr '\n' ' ')"
  fi
}

assert_payloads() { # <pass label>
  local pass="$1" x p list modes sha attach_out attrs
  # .pkg - the documented route: pkgutil --expand, then tar (R-19 finding 15).
  x="$work/pkg-$pass"
  pkgutil --expand "$out/$base.pkg" "$x"
  list="$(cd "$x" && find . -mindepth 1 -maxdepth 1 | sort)"
  expect_list "pkg ($pass) archive" "$(printf '%s\n' ./Distribution ./macportsbrew.pkg)" "$list"
  grep -q 'enable_currentUserHome="true"' "$x/Distribution" || die "pkg ($pass): current-user domain not enabled"
  grep -q 'enable_localSystem="false"' "$x/Distribution"    || die "pkg ($pass): system domain not disabled"
  grep -q 'enable_anywhere="false"' "$x/Distribution"       || die "pkg ($pass): install-anywhere not disabled"
  grep -qF "install-location=\"$INSTALL_LOCATION\"" "$x/macportsbrew.pkg/PackageInfo" \
    || die "pkg ($pass): install location is not $INSTALL_LOCATION"
  if [ -e "$x/macportsbrew.pkg/Scripts" ] || grep -qE '<(pre|post)install' "$x/macportsbrew.pkg/PackageInfo"; then
    die "pkg ($pass): the package carries scripts"
  fi
  check_pkg_payload_list "$pass" "$x/macportsbrew.pkg/Payload"
  modes="$(lsbom -p fm "$x/macportsbrew.pkg/Bom" | sort)"
  if [ -n "$tolerated" ]; then
    # Exact path match on the first field, never a substring.
    modes="$(printf '%s\n' "$modes" | awk -F '\t' '$1 != "./._bin" && $1 != "./bin/._macportsbrew"')"
  fi
  expect_list "pkg ($pass) modes" "$(printf '%s\t%s\n' . 40755 ./bin 40755 ./bin/macportsbrew 100755)" "$modes"
  p="$work/pkg-$pass-payload"
  mkdir "$p"
  tar -xf "$x/macportsbrew.pkg/Payload" -C "$p"
  sha="$(shasum -a 256 "$p/bin/macportsbrew" | awk '{print $1}')"
  [ "$sha" = "$signed_sha" ] || die "pkg ($pass): payload hashes to $sha, the bare executable to $signed_sha"
  note "pkg ($pass): payload = bare executable ($sha); list and modes exact; current-user domain only; no scripts"

  # .dmg - read-only, unbrowsed, at a private mount point (R-19 E11).
  mnt="$work/mnt-$pass"
  mkdir "$mnt"
  attach_out="$(hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$mnt" "$out/$base.dmg")"
  printf '%s\n' "$attach_out" | grep -q 'Apple_HFS' || die "dmg ($pass): not an HFS+ volume"
  list="$(cd "$mnt" && find . -mindepth 1 | sort)"
  expect_list "dmg ($pass) volume" "./macportsbrew" "$list"
  sha="$(shasum -a 256 "$mnt/macportsbrew" | awk '{print $1}')"
  # HFS+ keeps attributes natively, so the AppleDouble question becomes: which
  # attributes does the copy on the volume carry? Same tolerance as the pkg.
  attrs="$(xattr "$mnt/macportsbrew" | sort | tr '\n' ' ')"
  case "$attrs" in
    "") ;;
    "com.apple.provenance ")
      [ "$sign" = 0 ] || die "dmg ($pass): the executable carries com.apple.provenance (plan, D-P3)"
      note "TOLERATED (dry run only): the dmg's executable carries com.apple.provenance" ;;
    *) die "dmg ($pass): the executable carries extended attributes: $attrs" ;;
  esac
  hdiutil detach -quiet "$mnt"
  mnt=""
  [ "$sha" = "$signed_sha" ] || die "dmg ($pass): payload hashes to $sha, the bare executable to $signed_sha"
  hdiutil imageinfo "$out/$base.dmg" | grep -qx 'Format: UDZO' || die "dmg ($pass): not UDZO"
  note "dmg ($pass): payload = bare executable ($sha); volume holds only macportsbrew; HFS+, UDZO"
}

assert_payloads "built"

# --- 7. Notarise and staple (signed releases only) -----------------------------------

notary_record="$work/notary.txt"
: >"$notary_record"
if [ "$sign" = 1 ]; then
  notarise() { # <file> <label>
    local f="$1" label="$2" json status id
    json="$work/notary-$label.json"
    note "notarising $label"
    if ! xcrun notarytool submit "$f" --key "$MPB_NOTARY_KEY_PATH" --key-id "$MPB_NOTARY_KEY_ID" \
         ${MPB_NOTARY_ISSUER:+"--issuer" "$MPB_NOTARY_ISSUER"} \
         --wait --timeout 30m --output-format json >"$json"; then
      note "notarytool exited non-zero for $label"
    fi
    status="$(plutil -extract status raw -o - "$json" 2>/dev/null || echo unknown)"
    id="$(plutil -extract id raw -o - "$json" 2>/dev/null || echo unknown)"
    printf 'notary_submission_%s=%s %s\n' "$label" "$id" "$status" >>"$notary_record"
    if [ "$status" != "Accepted" ]; then
      if [ "$id" != "unknown" ]; then
        xcrun notarytool log "$id" --key "$MPB_NOTARY_KEY_PATH" --key-id "$MPB_NOTARY_KEY_ID" \
          ${MPB_NOTARY_ISSUER:+"--issuer" "$MPB_NOTARY_ISSUER"} >&2 || true
      fi
      die "notarisation of $label: $status (submission $id)"
    fi
    note "notarisation of $label: Accepted (submission $id)"
  }
  # The bare executable cannot be stapled; its ticket is found online. It is
  # submitted as a ZIP until the first release shows the nested ticket covers
  # it (R-19 finding 6).
  ditto -c -k --keepParent "$out/$base" "$work/$base.zip"
  notarise "$work/$base.zip" "bare"
  notarise "$out/$base.pkg" "pkg"
  notarise "$out/$base.dmg" "dmg"
  for f in "$out/$base.pkg" "$out/$base.dmg"; do
    xcrun stapler staple "$f"
    xcrun stapler validate "$f"
  done
  pkgsig="$(pkgutil --check-signature "$out/$base.pkg")"
  printf '%s\n' "$pkgsig"
  printf '%s\n' "$pkgsig" | grep -qF "Developer ID Installer: " || die "pkg: not signed by a Developer ID Installer identity"
  printf '%s\n' "$pkgsig" | grep -qF "($MPB_TEAM_ID)" || die "pkg: not signed by the expected team"
  # pkgutil's notarisation line and spctl's source line are expected wording,
  # not yet observed on a Developer ID artifact here; the first tag run
  # confirms them, and a mismatch fails closed (plan, Could not verify).
  printf '%s\n' "$pkgsig" | grep -qF "trusted by the Apple notary service" \
    || die "pkg: pkgutil does not report the package as notarised"
  codesign --verify --strict --verbose=2 "$out/$base.dmg"
  # Developer ID authority, team and timestamp on the FINAL .dmg (08).
  assert_developer_id "$out/$base.dmg" "dmg (stapled)"
  # What Gatekeeper itself says about each container.
  gk="$(spctl -a -vv -t install "$out/$base.pkg" 2>&1)" || { printf '%s\n' "$gk" >&2; die "pkg: Gatekeeper rejects it"; }
  printf '%s\n' "$gk"
  printf '%s\n' "$gk" | grep -qF "source=Notarized Developer ID" || die "pkg: Gatekeeper does not see a notarised Developer ID package"
  gk="$(spctl -a -vv -t open --context context:primary-signature "$out/$base.dmg" 2>&1)" \
    || { printf '%s\n' "$gk" >&2; die "dmg: Gatekeeper rejects it"; }
  printf '%s\n' "$gk"
  printf '%s\n' "$gk" | grep -qF "source=Notarized Developer ID" || die "dmg: Gatekeeper does not see a notarised Developer ID image"
  # Stapling rewrote both containers; assert again on the final bytes.
  assert_payloads "stapled"
else
  note "NOT NOTARISED: dry run"
fi

# --- 8. Checksums and the release record ---------------------------------------------

(cd "$out" && shasum -a 256 "$base" "$base.pkg" "$base.dmg") >"$out/SHA256SUMS.$arch"
(cd "$out" && shasum -a 256 -c "SHA256SUMS.$arch" >/dev/null) || die "SHA256SUMS.$arch does not verify"
[ "$(shasum -a 256 "$out/$base" | awk '{print $1}')" = "$signed_sha" ] || die "the bare artifact is not the signed executable"

record="$out/$base.release-record.txt"
{
  echo "tool_version=$version"
  echo "policy_version=$policy_version"
  echo "license=Apache-2.0"
  echo "target=$target"
  echo "deployment_floor=$floor"
  echo "commit=$commit"
  if [ "$sign" = 1 ]; then echo "signed=developer-id"; else echo "signed=ad-hoc-dry-run"; fi
  if [ "$release_input" = 1 ]; then echo "release_input=yes"; else echo "release_input=no"; fi
  echo "version_check=$version_check"
  # Which machine the host_* lines below describe: the one that compiled, or
  # the one that only signed a prebuilt input (whose toolchain is the build
  # job's, published separately as build_* lines in SIGNING-RECORD.txt).
  if [ -z "$from_binary" ]; then echo "host_role=build"; else echo "host_role=sign"; fi
  echo "identifier=$IDENTIFIER"
  echo "pkg_install_location=~$INSTALL_LOCATION/bin (current-user-home domain)"
  echo "unsigned_build_sha256=$unsigned_sha"
  echo "executable_sha256=$signed_sha"
  echo "executable_cdhash=$cdhash"
  echo "team_identifier=$team"
  echo "certificate_sha256=$cert_sha"
  cat "$notary_record"
  echo "host_macos=$(sw_vers -productVersion)"
  echo "developer_dir=$(xcode-select -p 2>/dev/null || echo unknown)"
  echo "xcode=$( (xcodebuild -version 2>/dev/null || echo unknown) | tr '\n' ' ')"
  echo "linker=$(ld -v 2>&1 | head -1)"
  if [ -z "$from_binary" ]; then echo "rustc=$(rustc -V)"; else echo "rustc=prebuilt input"; fi
} >"$record"

note "summary for $target"
printf '  %-44s %s\n' "artifact" "sha256"
while read -r sum name; do printf '  %-44s %s\n' "$name" "$sum"; done <"$out/SHA256SUMS.$arch"
printf '  %-44s %s\n' "(unsigned build output)" "$unsigned_sha"
printf '  %-44s %s\n' "(executable CDHash)" "$cdhash"
if [ "$sign" = 1 ]; then
  note "signed, notarised and stapled; payloads asserted after stapling"
else
  note "DRY RUN: ad-hoc signed, not notarised; payloads asserted; do not publish"
fi
```

## The workflow check

`scripts/check-release-workflow.sh`, SHA-256
`ab444b43555bf565123a89288aa35df0f8184a46a50f79e55fd67c378431d7c3`, 147
lines, shellcheck 0.11.0 clean. A `validate.sh --quick` stage (plan, D-T2;
[SR M1], [SG L3]; strengthened for [SG2 B-M1], [SR2 B-L2]). Observed
2026-10-11 from the scratchpad copy: it accepts a copy of `.github/workflows/`
plus the staged `release.yml`, and refuses each of 24 constructed violations,
one or more per rule — the environment in another workflow, a second selection
line, the mapping form; `secrets.MPB_*` in another workflow and in a composite
action; `secrets.` outside the signing job; `secrets[…]` and
`toJSON(secrets)`; `cargo`, `rustup`, `validate.sh` and `run-fixture-matrix`
in the signing job; `set -e -x`, `set -eux`, `set -o xtrace`, `SHELLOPTS`,
`BASH_XTRACEFD`, `bash -x`; an extra trigger, a `v*`-less tag filter, a
`branches` filter beside the tags, `workflow_run`, `pull_request_target`,
`ACTIONS_STEP_DEBUG`. The cases become `scripts/test-gates.sh` entries in the
armed batch.

```bash
#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# The release-workflow MUSTs of docs/spec/08-distribution-and-project.md and
# ADR-0005 that are properties of workflow text, asserted rather than written
# down (pass 09, D-T2). Over every workflow in .github/workflows/ and every
# composite action under .github/actions/:
#
#   1. no `pull_request_target` or `workflow_run` trigger;
#   2. no ACTIONS_STEP_DEBUG or ACTIONS_RUNNER_DEBUG;
#   3. no `secrets[...]` and no `toJSON(secrets)` - both expose every secret;
#   4. `release-signing` and `secrets.MPB_*` appear in release.yml only.
#
# And over release.yml itself:
#
#   5. its `on:` is exactly `push` with `tags: ["v*"]`, and `workflow_dispatch`;
#   6. exactly one line selects the environment, and it is
#      `environment: release-signing` (a `name: release-signing` line would be
#      a second selection);
#   7. the job holding that environment runs no cargo, rustup, Rust toolchain
#      action, validate.sh or run-fixture-matrix - nothing that compiles or
#      executes the project next to a credential;
#   8. `secrets.` appears inside that job only;
#   9. no xtrace anywhere: no `set` whose short-option clusters contain x
#      (`set -x`, `set -eux`, `set -e -x`), no `xtrace` (`set -o xtrace`,
#      `shopt`), no SHELLOPTS, no BASH_XTRACEFD, no `bash -x`, no `shell:`
#      with -x. "None in a step that sees a credential" is guaranteed by
#      "none at all".
#
# Comment lines are ignored, so a file may name what it forbids. Run from
# anywhere inside the repository. MPB_WORKFLOW_DIR and MPB_ACTIONS_DIR override
# the two directories so that scripts/test-gates.sh can run it over
# constructed fixtures.
#
#   scripts/check-release-workflow.sh

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
dir="${MPB_WORKFLOW_DIR:-.github/workflows}"
actions="${MPB_ACTIONS_DIR:-.github/actions}"
release="$dir/release.yml"
failures=0
fail() { printf 'check-release-workflow: FAIL: %s\n' "$*" >&2; failures=$((failures + 1)); }
hits_of() { # <body> <extended regex> [-i]; prints matching lines, fails if none
  printf '%s\n' "$1" | grep -E ${3:+"$3"} -- "$2"
}

# A file, comment lines removed, as "<file>:<line>:<text>".
uncommented() { # <file>
  awk -v f="$1" '!/^[[:space:]]*#/ { print f ":" NR ":" $0 }' "$1"
}

files=()
if [ -d "$dir" ]; then
  for f in "$dir"/*.yml "$dir"/*.yaml; do [ -f "$f" ] && files+=("$f"); done
fi
if [ -d "$actions" ]; then
  while IFS= read -r f; do files+=("$f"); done \
    < <(find "$actions" -type f \( -name 'action.yml' -o -name 'action.yaml' \) | sort)
fi
[ "${#files[@]}" -gt 0 ] || { echo "check-release-workflow: no workflow or action; nothing to check"; exit 0; }

for wf in "${files[@]}"; do
  body="$(uncommented "$wf")"
  if hits="$(hits_of "$body" '(^|[^A-Za-z0-9_])(pull_request_target|workflow_run)([^A-Za-z0-9_]|$)')"; then
    fail "forbidden trigger (08, ADR-0005):"$'\n'"$hits"
  fi
  if hits="$(hits_of "$body" 'ACTIONS_(STEP|RUNNER)_DEBUG')"; then
    fail "debug logging is forbidden (ADR-0005):"$'\n'"$hits"
  fi
  if hits="$(hits_of "$body" 'secrets[[:space:]]*\[|tojson\([[:space:]]*secrets' -i)"; then
    fail "an expression that exposes every secret:"$'\n'"$hits"
  fi
  if [ "$wf" != "$release" ]; then
    if hits="$(hits_of "$body" 'release-signing')"; then
      fail "only release.yml may name the release-signing environment (08):"$'\n'"$hits"
    fi
    if hits="$(hits_of "$body" 'secrets\.MPB_')"; then
      fail "only release.yml may read a secrets.MPB_* value (ADR-0005):"$'\n'"$hits"
    fi
  fi
done

if [ -f "$release" ]; then
  body="$(uncommented "$release")"

  # 5. The triggers, exactly.
  on_block="$(awk '/^[[:space:]]*#/ { next } /^[^[:space:]]/ { inon = ($0 ~ /^on:[[:space:]]*$/); next } inon && NF { print }' "$release")"
  [ "$(grep -c '^on:[[:space:]]*$' "$release" || true)" = 1 ] || fail "release.yml must have exactly one top-level 'on:' mapping"
  on_keys="$(printf '%s\n' "$on_block" | sed -n -E 's/^  ([A-Za-z_]+):.*/\1/p' | sort | tr '\n' ' ')"
  [ "$on_keys" = "push workflow_dispatch " ] || fail "release.yml's triggers are '$on_keys'; they must be exactly push and workflow_dispatch"
  push_block="$(printf '%s\n' "$on_block" | awk '/^  [A-Za-z_]/ { inpush = ($0 ~ /^  push:[[:space:]]*$/); next } inpush { print }')"
  if ! printf '%s\n' "$push_block" | grep -Eqx '    tags:[[:space:]]*\[[[:space:]]*"v\*"[[:space:]]*\][[:space:]]*' \
     || [ "$(printf '%s\n' "$push_block" | grep -c .)" != 1 ]; then
    fail "release.yml's push trigger must be exactly: tags: [\"v*\"]"
  fi

  # 6. One environment line. Any line that could select the environment -
  #    `environment: release-signing`, or `name: release-signing` under an
  #    environment mapping - counts; prose in an error message does not.
  mentions="$(printf '%s\n' "$body" | grep -cE '(environment|name):.*release-signing' || true)"
  envline="$(printf '%s\n' "$body" | grep -E '^[^:]*:[0-9]+:[[:space:]]*environment:[[:space:]]*release-signing[[:space:]]*$' || true)"
  if [ "$mentions" != 1 ] || [ -z "$envline" ]; then
    fail "release.yml must select release-signing exactly once, as 'environment: release-signing' (found $mentions)"
  else
    # 7, 8. The signing job: from its two-space job key to the next one.
    envno="$(printf '%s' "$envline" | sed -n -E 's/^[^:]*:([0-9]+):.*/\1/p')"
    job="$(awk -v n="$envno" '
      { line[NR] = $0 }
      /^jobs:[[:space:]]*$/ { injobs = 1; next }
      injobs && /^[^[:space:]#]/ { injobs = 0; if (start && !stop) stop = NR }
      injobs && /^  [A-Za-z0-9_-]+:[[:space:]]*$/ {
        if (NR <= n) start = NR; else if (start && !stop) stop = NR
      }
      END {
        if (!start) exit
        if (!stop) stop = NR + 1
        for (i = start; i < stop; i++) print i ":" line[i]
      }' "$release")"
    if [ -z "$job" ]; then
      fail "could not find the job that holds environment: release-signing"
    else
      jobtext="$(printf '%s\n' "$job" | grep -v '^[0-9]*:[[:space:]]*#' || true)"
      if hits="$(hits_of "$jobtext" '(^|[^A-Za-z0-9_-])(cargo|rustup|rust-toolchain|validate\.sh|run-fixture-matrix)([^A-Za-z0-9_-]|$)')"; then
        fail "the signing job compiles or runs the project (plan, D-W2):"$'\n'"$hits"
      fi
      first="$(printf '%s\n' "$job" | head -1 | cut -d: -f1)"
      last="$(printf '%s\n' "$job" | tail -1 | cut -d: -f1)"
      if hits="$(printf '%s\n' "$body" | awk -F: -v a="$first" -v b="$last" '/secrets\./ && ($2 < a || $2 > b)')" && [ -n "$hits" ]; then
        fail "secrets. outside the signing job:"$'\n'"$hits"
      fi
    fi
  fi

  # 9. No xtrace, in any spelling.
  xre='(^|[^A-Za-z0-9_])set([[:space:]]+[-+][A-Za-z]+)*[[:space:]]+[-+][A-Za-z]*x|xtrace|SHELLOPTS|BASH_XTRACEFD|bash[[:space:]]+-[A-Za-z]*x|shell:.*[[:space:]]-[A-Za-z]*x'
  if hits="$(hits_of "$body" "$xre")"; then
    fail "xtrace in the release workflow (ADR-0005: it exposes argv):"$'\n'"$hits"
  fi
fi

if [ "$failures" -gt 0 ]; then
  echo "check-release-workflow: $failures rule(s) broken" >&2
  exit 1
fi
echo "check-release-workflow: release workflow rules hold"
```
