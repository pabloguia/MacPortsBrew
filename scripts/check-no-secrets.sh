#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# No signing credential is tracked in this repository. 08 and ADR-0005 put the
# release credentials in the `release-signing` environment and nowhere else;
# this is the check that a later change has not put one beside the code (pass
# 09 plan, "The secret-material check"). Over every tracked file, it refuses:
#
#   1. a name ending in .p12, .pfx, .p8, .cer, .mobileprovision,
#      .provisionprofile, .pem, .key, .ppk, .jks or .keystore, or holding
#      .keychain (a keychain and its -db), in any letter case;
#   2. a name holding an App Store Connect key's file name, AuthKey_ followed
#      by ten letters or digits and .p8, anywhere in the path, in any case, and
#      whatever follows it (a renamed `.p8.txt` is still the key);
#   3. any binary file, in the index or in the working tree. The repository
#      tracks none, and the credential that matters most - a Developer ID .p12
#      - is DER, which no text rule sees under another name. There is no
#      allow-list; the day a binary file is meant to be tracked, adding one is a
#      reviewed change to this file;
#   4. a PEM armour line for a private key or a certificate: five dashes,
#      BEGIN, an optional word (RSA, EC, ENCRYPTED, OPENSSH, TRUSTED...), then
#      PRIVATE KEY or CERTIFICATE;
#   5. base64 of DER: `MII` and sixty or more base64 characters, which is how
#      every DER SEQUENCE with a two-byte length - a PKCS#12, a PKCS#8 key, a
#      certificate - starts once encoded. The release secrets are such values
#      (`MPB_*_CERT_B64`), and pasting one into a file is the likely accident.
#      Two shorter EC keys are named too: the start of an unarmoured P-256
#      PKCS#8 body (an App Store Connect key without its armour) and of a SEC1
#      EC private key. A letter of each is in a group, `(B)` and `(E)`, so
#      that this file does not match itself;
#   6. a team-identifier-shaped value given to a team key: TEAM_ID,
#      TeamIdentifier or DEVELOPMENT_TEAM, then = or : (spaces and a quote of
#      either kind allowed), then ten capitals or digits; or `--team-id` and
#      such a value. `Developer ID Application: <name> (<team>)` is not
#      matched: the research notes quote other vendors' public signatures in
#      exactly that form.
#
# The plan named rules 1 (its first five types), 2, 4 and the first spelling of
# 6; the rest were added on review (pass 15). Every rule covers the files git
# tracks AND the untracked files it does not ignore: the commit gate runs
# before the command it guards, so in `git add <key> && git commit` the key is
# still untracked when it is checked. Rules 4-6 also read the index's copy of
# every tracked file, so a credential staged and then changed or deleted in the
# working copy is seen. An ignored file is not this check's business
# (.gitignore is).
#
# The git environment is cleared first: GIT_DIR, GIT_WORK_TREE or
# GIT_INDEX_FILE inherited from a caller would point the scan somewhere else.
#
# The patterns spell five dashes as `-{5}` so that this file does not match
# itself, and the gate tests (scripts/test-gates.sh) build every refused input
# at run time, so the repository never contains one. It prints where a match
# is - the path, and the line number for the content rules - and never the
# matching text: a gate's output is a log, and a log is not a place to repeat a
# secret.
#
# Unlike check-action-pins.sh, which walks the tree so that a source export is
# scanned, this one refuses a tree that is not a git work tree: "tracked" has
# no meaning there, and a gate that cannot scan fails closed.
#
#   scripts/check-no-secrets.sh
#   MPB_SECRETS_ROOT=<a git work tree> scripts/check-no-secrets.sh   (gate tests)

set -uo pipefail
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_OBJECT_DIRECTORY
if [ -n "${MPB_SECRETS_ROOT:-}" ]; then
  root="$MPB_SECRETS_ROOT"
  echo "no-secrets: scanning $root (MPB_SECRETS_ROOT)" >&2
else
  root="$(dirname "${BASH_SOURCE[0]}")/.."
fi
cd "$root" || { echo "no-secrets: cannot enter $root" >&2; exit 1; }

# Fail closed: a tree git cannot list is not a tree with no secret in it.
if [ "$(git rev-parse --is-inside-work-tree 2>/dev/null)" != "true" ]; then
  echo "no-secrets: $(pwd) is not a git work tree; refusing to pass on a scan of nothing" >&2
  exit 1
fi

bad=0
refuse() { echo "no-secrets: $1: $2" >&2; bad=1; }

if ! listing="$({ git ls-files -z && git ls-files -o --exclude-standard -z; } | tr '\0' '\n')"; then
  echo "no-secrets: git ls-files failed; refusing" >&2
  exit 1
fi
files="$(printf '%s' "$listing" | grep -c '' || true)"
if [ "$files" -eq 0 ]; then
  echo "no-secrets: git tracks no file here and sees none untracked; refusing to pass on a scan of nothing" >&2
  exit 1
fi

# One grep per rule over the whole list, not two processes per file: this is a
# --quick stage, and it runs on every turn end.
while IFS= read -r path; do
  [ -n "$path" ] && refuse "credential file type" "$path"
done < <(printf '%s\n' "$listing" \
           | grep -Ei '\.(p12|pfx|p8|cer|mobileprovision|provisionprofile|pem|key|ppk|jks|keystore)$|\.keychain' || true)
while IFS= read -r path; do
  [ -n "$path" ] && refuse "App Store Connect key file name" "$path"
done < <(printf '%s\n' "$listing" | grep -Ei 'AuthKey_[A-Z0-9]{10}\.p8' || true)

# `--eol` gives the index's (i/) and the working tree's (w/) view; `-text` is
# git's own verdict that the content is binary.
if ! eol="$({ git ls-files --eol -z && git ls-files -o --exclude-standard --eol -z; } | tr '\0' '\n')"; then
  echo "no-secrets: git ls-files --eol failed; refusing" >&2
  exit 1
fi
while IFS= read -r path; do
  [ -n "$path" ] && refuse "binary file" "$path"
done < <(printf '%s\n' "$eol" | awk -F '\t' '{ split($1, f, " +") } f[1] == "i/-text" || f[2] == "w/-text" { print $2 }')

# <rule> <extended regular expression>, over the working tree (tracked and
# untracked-not-ignored) and the index.
# `git grep` exits 1 for "no match" and above 1 for an error; an error refuses,
# it is not a clean answer. Only the path and the line number leave this
# function (a binary file is reported by name).
content_rule() {
  local rule="$1" pattern="$2" out rc where hit
  for where in worktree index; do
    if [ "$where" = index ]; then
      out="$(git grep --cached -n -E --no-color -e "$pattern" 2>&1)"
    else
      out="$(git grep --untracked -n -E --no-color -e "$pattern" 2>&1)"
    fi
    rc=$?
    case "$rc" in
      0) while IFS= read -r hit; do
           case "$hit" in
             "Binary file "*) refuse "$rule ($where)" "${hit#Binary file }" ;;
             *) refuse "$rule ($where)" "$(printf '%s' "$hit" | cut -d: -f1,2)" ;;
           esac
         done <<<"$out" ;;
      1) ;;
      *) echo "no-secrets: git grep failed (exit $rc) for the $rule rule ($where); refusing" >&2
         bad=1 ;;
    esac
  done
}
content_rule "PEM private key or certificate" '-{5}BEGIN ([A-Z0-9]+ )?(PRIVATE KEY|CERTIFICATE)'
content_rule "base64 of DER" 'MII[A-Za-z0-9+/]{60,}|MIGTAgEAMBMG(B)yqGSM49|MHcCAQE(E)[A-Za-z0-9+/]{40,}'
content_rule "team identifier value" \
  "(TEAM_ID|TeamIdentifier|DEVELOPMENT_TEAM)[[:space:]]*[=:][[:space:]]*[\"']?[A-Z0-9]{10}|--team-id[= ]+[\"']?[A-Z0-9]{10}"

if [ "$bad" -eq 0 ]; then
  echo "no-secrets: ok ($files files, tracked or untracked and not ignored)"
fi
exit "$bad"
