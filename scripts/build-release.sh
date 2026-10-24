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
#   mpb-<version>-<arch>                 the same bytes under the short name
#   macportsbrew-<version>-<arch>.pkg    installs ONLY `macportsbrew` and
#                                        `mpb`, into ~/.macportsbrew/bin,
#                                        current-user domain, no PATH edit,
#                                        no script
#   macportsbrew-<version>-<arch>.dmg    UDZO, HFS+, holding the same two
#   SHA256SUMS.<arch>                    the four, in `shasum -a 256` format
#
# `mpb` is not a second build: it is a byte-identical copy of the one signed
# executable (DP-033 (b), ruled 2026-10-23; the same main.rs as a second
# [[bin]]). A copy is what lets every assertion below say "the same bytes", and
# it carries the same signature and CDHash, so the executable's notarisation
# ticket is expected to be its ticket too - first shown by the first signed
# release, as the ZIP question in step 7 is. The .pkg cannot run a conflict
# guard (it has no scripts, by design); the guard is PATH order (ADR-0003,
# ADR-0005).
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
#     the .dmg (read-only, unbrowsed attach) hashes to the bare executable, for
#     BOTH names, AND the bare `mpb` does too, AND
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
# Owner decision D-S3, ruled 2026-10-23 (DP-035): io.github.pabloguia.macportsbrew,
# following the repository's account (github.com/pabloguia/MacPortsBrew, DP-031).
# Permanent once a release is published.
readonly IDENTIFIER="io.github.pabloguia.macportsbrew"
# Interpreted relative to the user's home directory, because the distribution
# below enables only the current-user-home domain (plan, D-P1). R-19 lists the
# exact interpretation as UNRESOLVED; the plan carries the check that settles it.
readonly INSTALL_LOCATION="/.macportsbrew"
# The names every container carries, in the order a sorted listing shows them.
# Every list and every hash below is derived from this one line; the gate tests
# pin it (scripts/test-gates.sh).
PAYLOAD=(macportsbrew mpb)
readonly PAYLOAD

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
mpb_base="mpb-$version-$arch"
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
  # `cargo tree` below answers for Cargo features. The test-only allowance is
  # also reachable as a bare cfg, which a compiler wrapper can pass without
  # cargo seeing it, so a wrapper is refused outright: in the environment, and
  # in the two cargo configuration files this build would read.
  for var in RUSTC RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER CARGO_BUILD_RUSTC \
             CARGO_BUILD_RUSTC_WRAPPER CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER; do
    [ -z "${!var:-}" ] || die "$var is set; a release is compiled by the toolchain's own rustc, with no wrapper"
  done
  for cfg in "$root/.cargo/config" "$root/.cargo/config.toml" "$cargo_home/config" "$cargo_home/config.toml"; do
    if [ -f "$cfg" ] && grep -Eq 'rustc-wrapper|rustc-workspace-wrapper|^[[:space:]]*rustc[[:space:]]*=' "$cfg"; then
      die "$cfg names a compiler or a compiler wrapper; a release is built without one"
    fi
  done
  # Host paths out of the executable (R-19 findings 7-8). The unit separator
  # lets a path contain a space, which RUSTFLAGS cannot carry.
  encoded="$(printf '%s\037%s' \
    "--remap-path-prefix=$cargo_home/registry/src=/cargo-registry" \
    "--remap-path-prefix=$root=/macportsbrew")"
  # No feature flag reaches this build: the argument loop above refuses every
  # option it does not name (`--features`, `--all-features`, `-F`), and cargo
  # reads Cargo features from nowhere else. That is a rule about this script; what is
  # about to be compiled is a fact about the dependency graph, so ask cargo for
  # it. `test-support` (the application crate) compiles in the test-only
  # mutation allowance and must never be in a release (pass 08 slice 4 review
  # L1; tripwire the_switch_needs_release_builds_free_of_the_test_support_feature).
  # Normal and build edges only: a dev-dependency is not part of this build
  # (resolver 2). `{f}` lists each package's enabled features, which also
  # shows a feature set on the root, and which `-e features` does not.
  # Fail closed: a cargo that cannot answer is not an answer of "clean".
  if ! graph="$(cargo tree --locked -p macportsbrew --target "$target" \
                  --prefix none -e normal,build -f '{p} {f}' 2>&1)"; then
    printf '%s\n' "$graph" | tail -20 >&2
    die "cargo tree failed; cannot show that the build is free of the test-support feature"
  fi
  # A here-string, not a pipeline: under pipefail, `grep -q` that stops reading
  # at its first match can kill a still-writing printf, the pipeline then returns
  # 141, and the `if` reads that as "no match" (reproduced above 64 KB of graph).
  hits="$(grep -E '(^|[ ,])test-support([ ,]|$)' <<<"$graph" || true)"
  if [ -n "$hits" ]; then
    printf '%s\n' "$hits" >&2
    die "the test-support feature resolves into this build; a release is built without it"
  fi
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
ditto --norsrc --noextattr --noqtn --noacl "$bin" "$out/$mpb_base"
chmod 0755 "$out/$mpb_base"

# Every copy of the program in <out-dir> or a container: <dir> <label>, then
# for each name in PAYLOAD the file <dir>/<name> must exist and hash to the
# signed executable. A missing name is a failure, not a skipped one.
assert_same_bytes() {
  local dir="$1" label="$2" name sha
  for name in "${PAYLOAD[@]}"; do
    [ -f "$dir/$name" ] || die "$label: no $name"
    sha="$(shasum -a 256 "$dir/$name" | awk '{print $1}')"
    [ "$sha" = "$signed_sha" ] || die "$label: $name hashes to $sha, the bare executable to $signed_sha"
  done
}
# The short name carries the same signature, so the same CDHash, as the bare
# executable: its notarisation ticket is expected to be the executable's (step 7).
mpb_cdhash="$(codesign -dvvv "$out/$mpb_base" 2>&1 | sed -n 's/^CDHash=//p' | head -1)"
[ "$mpb_cdhash" = "$cdhash" ] || die "$mpb_base: CDHash $mpb_cdhash, the executable's is $cdhash"

# --- 4. The .pkg: two names, one directory, the current user's home ---------------

pkgroot="$work/pkgroot"
mkdir -p "$pkgroot/bin" "$work/components"
chmod 0755 "$pkgroot" "$pkgroot/bin"
for name in "${PAYLOAD[@]}"; do
  ditto --norsrc --noextattr --noqtn --noacl "$bin" "$pkgroot/bin/$name"
  chmod 0755 "$pkgroot/bin/$name"
done
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
for name in "${PAYLOAD[@]}"; do
  ditto --norsrc --noextattr --noqtn --noacl "$bin" "$dmgroot/$name"
  chmod 0755 "$dmgroot/$name"
done
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
  expected="$(printf '%s\n' . ./bin; printf './bin/%s\n' "${PAYLOAD[@]}")"
  actual="$(tar -tf "$payload" | sort)"
  tolerated=""
  while IFS= read -r entry; do
    if printf '%s\n' "$expected" | grep -qxF -- "$entry"; then
      kept="$kept$entry"$'\n'
      continue
    fi
    case "$entry" in
      ./._bin|./bin/._macportsbrew|./bin/._mpb) ;;
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
    modes="$(printf '%s\n' "$modes" | awk -F '\t' '$1 != "./._bin" && $1 != "./bin/._macportsbrew" && $1 != "./bin/._mpb"')"
  fi
  expect_list "pkg ($pass) modes" \
    "$(printf '%s\t%s\n' . 40755 ./bin 40755; printf './bin/%s\t100755\n' "${PAYLOAD[@]}")" "$modes"
  p="$work/pkg-$pass-payload"
  mkdir "$p"
  tar -xf "$x/macportsbrew.pkg/Payload" -C "$p"
  assert_same_bytes "$p/bin" "pkg ($pass) payload"
  note "pkg ($pass): payload = bare executable ($signed_sha) under ${PAYLOAD[*]}; list and modes exact; current-user domain only; no scripts"

  # .dmg - read-only, unbrowsed, at a private mount point (R-19 E11).
  mnt="$work/mnt-$pass"
  mkdir "$mnt"
  attach_out="$(hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$mnt" "$out/$base.dmg")"
  printf '%s\n' "$attach_out" | grep -q 'Apple_HFS' || die "dmg ($pass): not an HFS+ volume"
  list="$(cd "$mnt" && find . -mindepth 1 | sort)"
  expect_list "dmg ($pass) volume" "$(printf './%s\n' "${PAYLOAD[@]}")" "$list"
  # HFS+ keeps attributes natively, so the AppleDouble question becomes: which
  # attributes does each copy on the volume carry? Same tolerance as the pkg.
  for name in "${PAYLOAD[@]}"; do
    attrs="$(xattr "$mnt/$name" | sort | tr '\n' ' ')"
    case "$attrs" in
      "") ;;
      "com.apple.provenance ")
        [ "$sign" = 0 ] || die "dmg ($pass): $name carries com.apple.provenance (plan, D-P3)"
        note "TOLERATED (dry run only): the dmg's $name carries com.apple.provenance" ;;
      *) die "dmg ($pass): $name carries extended attributes: $attrs" ;;
    esac
  done
  assert_same_bytes "$mnt" "dmg ($pass) volume"
  hdiutil detach -quiet "$mnt"
  mnt=""
  hdiutil imageinfo "$out/$base.dmg" | grep -qx 'Format: UDZO' || die "dmg ($pass): not UDZO"
  note "dmg ($pass): payload = bare executable ($signed_sha); volume holds only ${PAYLOAD[*]}; HFS+, UDZO"
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
  # it (R-19 finding 6). The bare `mpb` is not submitted: it has this one's
  # CDHash (asserted above), and a ticket is issued for a CDHash, not a name -
  # expected, not yet observed: the first signed release is where it is shown.
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

(cd "$out" && shasum -a 256 "$base" "$mpb_base" "$base.pkg" "$base.dmg") >"$out/SHA256SUMS.$arch"
(cd "$out" && shasum -a 256 -c "SHA256SUMS.$arch" >/dev/null) || die "SHA256SUMS.$arch does not verify"
[ "$(wc -l <"$out/SHA256SUMS.$arch" | tr -d ' ')" = 4 ] || die "SHA256SUMS.$arch does not list exactly four artifacts"
[ "$(shasum -a 256 "$out/$base" | awk '{print $1}')" = "$signed_sha" ] || die "the bare artifact is not the signed executable"
[ "$(shasum -a 256 "$out/$mpb_base" | awk '{print $1}')" = "$signed_sha" ] || die "the bare mpb is not the signed executable"

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
  echo "payload_names=${PAYLOAD[*]}"
  echo "mpb_sha256=$(shasum -a 256 "$out/$mpb_base" | awk '{print $1}')"
  echo "mpb_cdhash=$mpb_cdhash"
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
