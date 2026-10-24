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
fact 15), and the revision before pass 15 (`c8d3b0f5…`) was dry-run unsigned for
`x86_64-apple-darwin` on 2026-10-11 (plan, fact 17) — exit 0, every assertion
passing; the arm64 build and the signed path are not exercised.
`check-release-workflow.sh` was run over a copy of `.github/workflows/` plus
the staged `release.yml` (passes) and over 24 constructed violations (each
refused).

## Addendum, 2026-10-22 (pass 15, part A) — what changed since this was staged

Written by pass 15 before the armed message. Read this first; where it and an
older paragraph below disagree, this wins. **What each hash covers:** every
SHA-256 in this file is of the block exactly as extracted — the lines between
its fences, each ending in a newline, nothing after the last one — which is how
`sed -n '<first>,<last>p'` yields it. (`release.yml` is a file, hashed whole.)

**Pins, verified against each action's own tag on 2026-10-22** (`gh api
repos/<owner>/<repo>/commits/<tag>`):

| Action | Staged | Now | Result |
|---|---|---|---|
| `actions/checkout` | `d23441a4…` (v6) | `3d3c42e5…` (v7.0.1) | **Changed** to `main`'s pin; v7.0.1 is the tag's commit and the latest release. |
| `dtolnay/rust-toolchain` | `6c977a6c…` (commit of 2026-08-05) | `683b588c…` (`master`, commit of 2026-10-16) | **Changed** to `main`'s pin; `master` still resolves to it. |
| `actions/upload-artifact` | `043fb46d…` (v7.0.1) | same | Verified. v7.0.2 exists; not taken (a pin moves by decision). |
| `actions/download-artifact` | `3e5f45b2…` (v8.0.1) | same | Verified. v8.0.2 exists; not taken. |
| `actions/attest-build-provenance` | `4d101475…` (v4.2.2) | same | Verified; v4.2.2 is the latest release. |

Two comments in `main`'s guarded workflows are out of date, for part B to fix:
`repository-validation.yml` pins `taiki-e/install-action` to `1168f681…` under
`# v2`, but that commit is the tag **v2.87.28** (`v2` now points at v2.87.30,
`a4d3f3f7…`); and the workflows annotate `dtolnay/rust-toolchain@683b588c…` as
`master, resolved 2026-09-02`, but that commit is dated 2026-10-16. The staged
`release.yml` does not use `taiki-e/install-action`; besides `actions/checkout`
and `dtolnay/rust-toolchain`, already in `ALLOWED_REPOS`, it uses only the three
actions step 3 adds. Separately, the generated `docs/support-matrix.md` still
says "No CI job has ever run"; its source is `scripts/support-matrix.sh` (guarded),
and CI has run: "Repository validation" passed on `main` at `ad5a95d` on
2026-10-21 (run 39578935218) on macos-26, macos-26-intel, macos-15 and
macos-15-intel. Part B should change the banner it generates.

**`release.yml` was changed** only in those two pins. Its SHA-256 is `4801f886c962f72d0174c79d35ede3fe0678f5b82351865cee796f987f16fecb`
(it had none recorded; the pre-pass file hashed `e8e090ee456fa135848e2a0f252f6ff467463b4cd8699032753a24ef96a0677c`). Its `sign` job
already carries `github.repository == 'pabloguia/MacPortsBrew'` (DP-031 is
resolved), so the checklist line that asked for it is gone.

**`build-release.sh` changed in three ways**, SHA-256 now `2b5fab09f93157330e44d6f39e30ea276c92747929f26ad5844e37c2a706cd8a`
(719 lines, shellcheck 0.11.0 clean, `bash -n` clean):

1. **The `test-support` check** (new, before the `cargo build`): it asks
   `cargo tree --locked -p macportsbrew --target "$target" --prefix none -e
   normal,build -f '{p} {f}'` and refuses when any package lists the feature, or
   when cargo cannot answer. The format string matters: `-e features` does not
   list a feature set on the root package, so grepping it passes a build that
   enables `test-support` directly; `{f}` does. Observed on this tree: the
   release graph lists no `test-support`; the same command over the dev edges
   does (`macportsbrew-application … test-support`); and over
   `macportsbrew-application --features test-support` it does. The match is on
   the whole word (`(^|[ ,])test-support([ ,]|$)`, which also covers cargo's `(*)`
   marker on a repeated package) and is made with a **here-string, not a
   pipeline**: the security review reproduced that under `set -o pipefail` a
   `grep -q` that stops at its first match can kill a still-writing `printf`, the
   pipeline returns 141, and an `if` reads that as "no match" — above about 64 KB
   of graph, where today's is under 3 KB. A gate case now puts the match on the
   first line of a 100 KB graph. The argument loop already refused every option
   it does not name (`--features`, `--all-features`, `-F`); cases pin that too.
2. **A compiler wrapper is refused** (environment: `RUSTC`, `RUSTC_WRAPPER`,
   `RUSTC_WORKSPACE_WRAPPER` and the three `CARGO_BUILD_*` forms; and a
   `rustc-wrapper`, `rustc-workspace-wrapper` or `rustc =` key in
   `<repo>/.cargo/config[.toml]` or `$CARGO_HOME/config[.toml]`), beside the
   existing `RUSTFLAGS` refusal. The test-only allowance is gated by
   `cfg(any(test, feature = "test-support"))`; a wrapper can pass the cfg
   without cargo's feature graph showing it.
3. **`IDENTIFIER` is now `io.github.pabloguia.macportsbrew`** (DP-035, ruled
   2026-10-23), and the comment above it says so; before, the value was
   `io.github.macportsbrew.macportsbrew` under a comment claiming it was
   "derived from the workspace `repository` URL", which it was not.

**`check-release-workflow.sh` gained rule 10**, SHA-256 now `544ec1e6e289176980b29bfc2098e77b73821333b932e8c6155b49c9329b5c23`
(179 lines, shellcheck clean): in `release.yml` (a) no non-comment line names
`cargo` at all, so a release is built only through `build-release.sh` and no
spelling of a cargo line matters (the first version matched `--features` /
`--all-features` / `-F` on lines naming cargo, and the security review found
real spellings it missed: `-rF`, a quoted `"--features=…"`, a YAML multi-line
scalar, the flag in a variable); (b) no non-comment line holds `--features` or
`--all-features`, quoted or not; (c) no `-F`, alone or in a cluster, quoted or
not, on a line that names `build-release.sh`, a backslash continuation counted as
one line. It reads `release.yml` only, so `validate.sh`'s own `cargo test
--all-features` is untouched.

**Gate tests** (two new sections at the end of this file): `275fe4f5b25f4695790edc8b2475bc4ccd791c338506ad629944acc88e4e866e` for the
`build-release.sh` cases (20) and `e1871acc98f7b3599b7734f2d17ad087a809bc907950fbed504cf50627b8e799` for the workflow-check cases
(44, reconstructed from the description of the original 24: 26 violations of
the original rules, rule 10's, and the ones that must pass). They run the real
scripts with a stand-in `cargo` that **records its arguments** — the cases assert
the exact `cargo tree` command line, because a narrowed query (`-f '{p}'`,
`--depth 0`, another `-p`) would otherwise blind the check while every case still
passed — and with constructed workflow trees. Each was proved by mutation on
2026-10-22: 16 mutants of `build-release.sh` (the refusal removed; the
pattern widened to a substring, or narrowed at either end or the `(*)` marker;
a failing `cargo tree` read as clean; the pipeline back; `-f`, `-e`, `-p`,
`--locked`, `--target` each narrowed or dropped; a wrapper variable dropped; the
config check removed; the flag loop accepting `--features`) and 12 of the
workflow check (each part of rule 10, rule 10 off, a comment line counted,
`rustup` dropped from rule 7, `xtrace` from rule 9) were each killed by at least
one case. The one mutant that first survived (comments counted in rule 10(c)) got
its own case.

**The tripwire** `the_switch_needs_release_builds_free_of_the_test_support_feature`
(`crates/cli/tests/history_preconditions.rs`) is replaced by a structural test,
text in the last section, SHA-256 `c22d4bea20fbc91aec335afa665be04d06804c0f5122922b15ac07500731cef9`. It now matches the **whole** `cargo
tree` command line as one needle (so inserting `--depth 0` or `--prune` anywhere
in it fails), the here-string match, both wrapper refusals, rule 10 in the
workflow check, and that `release.yml` names `cargo` nowhere in code. It cannot
land before the scripts exist, so it lands **in the same change** as step 1
below, which is why it is staged here rather than committed on the pass-15 topic
branch. Observed in a scratch copy of the tree with the staged scripts
installed: it passes, and it fails when the command line is narrowed, a flag is
inserted, or the wrapper loop is shortened.

**`validate.sh`'s stage list** (its header, nowhere else) goes from fifteen
stages to **seventeen** with `check-no-secrets.sh` and `check-release-workflow.sh`.
Both are `--quick` stages, so both run on the Stop gate.

**`prune-target.sh`** (SE-100) runs at the end of every `validate.sh` mode and
never fails the run. `release.yml`'s `build` job runs `scripts/validate.sh`
(line 185) and then `build-release.sh` on a fresh runner checkout: the prune
deletes only `*.o` files older than 30 minutes under `target/*/deps`, and only
warns, never runs `cargo clean`, when over the entry limit. The release build
writes under `target/<triple>/release`, so it is unaffected; and the
`test-support` build `cargo test --all-features` leaves in `target/debug` cannot
be reused by the `--target` release build (features are part of cargo's
fingerprint). Confirmed by reading the script; not exercised on a runner.

**Amended "Applying them", for the armed message:** step 1 and step 5 check the
SHA-256s above (not the older ones); step 3 adds the three actions to
`ALLOWED_REPOS`; step 6 appends the two gate-test blocks to `test-gates.sh` (put
`# shellcheck disable=SC2016` after the shebang header there, because the cases
hold single-quoted shell on purpose, then re-run `shellcheck -x`); the
`build-release.sh` cases print `SKIP` off Darwin, where the packaging tools do
not exist; and the Rust test replaces the tripwire in the same change.

**Rulings of 2026-10-23, and the work they add to part B** (the scripts below
are still the ones the earlier sections describe; these items are not staged
yet):

- **DP-033 (b): the `.pkg` and `.dmg` also carry `mpb`.** `build-release.sh`'s
  payload for the `.pkg` grows `./bin/mpb` beside `./bin/macportsbrew` (same
  mode, same bytes), the `.dmg` volume holds both, and the payload assertions
  (`check_pkg_payload_list`, the dmg listing, the release record, `SHA256SUMS`
  scope) grow an entry for each. The `mpb` file is a copy of the signed
  executable, or the build's second `[[bin]]` signed separately; **decide that
  with the first run**, because `mpb` is the same `main.rs` and a copy is
  byte-identical, which the payload assertion can then state. A `.pkg` has no
  scripts and cannot run the conflict guard, so the guard is `PATH` order:
  the documented line **appends** the directory, so an earlier `mpb` wins.
  ADR-0005, ADR-0003, spec 06 and the user text were amended on 2026-10-23
  (SE-104); the payload code, its tests and the gate cases are part B.
- **DP-034 (b), held (2026-10-23):** the signing step of `release.yml` is
  rewritten to the recipe at the end of the R-19 addendum of that date: one
  `security -i` per command, hex passwords, `|| exit 1` on each, and the comment
  that called the argv route a deviation removed. The staged `release.yml` above
  still has the argv form, so **its SHA-256 changes in part B**; the research did
  not show that the keychain signs, so the first dry run is where that is checked.
- **DP-035:** applied above.

**Not changed by this pass, and why:** the install directory and identifier
(DP-035), the `mpb` text (DP-033, default "change no text"), the keychain
password's route through `security`'s argv (DP-034), and the owner sections of
the checklist below.

## Installed, 2026-10-23 (pass 15, part B) — final SHA-256s and what changed

Every block above was extracted and checked against its SHA-256 before it was
written; all six matched. Installed **as staged** in commit `f4fc487`; the files
whose text then changed by a reviewed decision are marked. `security-reviewer`
(Opus) read every guarded file before it was committed; the second review's
two Mediums are accepted and recorded, not fixed (the pass report).

| File | Staged SHA-256 | Installed (final) SHA-256 | Why it differs |
|---|---|---|---|
| `scripts/build-release.sh` | `2b5fab09…` | `0f1b3f023c22ef729b6de8e9d7a27374a8ccf1ffc01c8b8f15c0b289a4b234b2` | DP-033 (b): `mpb` in the payload (commit `3ad50fa`) |
| `scripts/check-release-workflow.sh` | `544ec1e6…` | `544ec1e6e289176980b29bfc2098e77b73821333b932e8c6155b49c9329b5c23` | — |
| `.github/workflows/release.yml` | `4801f886…` | `75476befc9858fe5ec3bf53dffead8a39e1ddf6f3b5f83921b0fe6c0b328a9c3` | DP-034 (b): the `security -i` signing step (commit `33eb543`) |
| gate cases, `build-release.sh` block | `275fe4f5…` | appended verbatim to `scripts/test-gates.sh` | — |
| gate cases, workflow-check block | `e1871acc…` | appended verbatim to `scripts/test-gates.sh` | — |
| structural test | `c22d4bea…` | replaces the tripwire verbatim | — |
| `scripts/check-no-secrets.sh` | (not staged) | `8e6c7394ca7bdf6de883cc2b0f5689e1c200ef2ba83198b58f0f6371bb44e322` | written from the plan and the review (SE-102) |

**Follow-ups 2a, later on 2026-10-23** (one more armed message): `build-release.sh`
→ `5fcd3480a06d570bbfb6af1254acaed8f78f3e14555d7da2fdeab2c4097f3131` (comments only:
the notarisation of the bare `mpb` is "expected", not stated); `release.yml` →
`b528f9d1576d490189c86f7bd596c428865ebf78e2f02ffdf33f18c0f991296c` (`RUNNER_TEMP` must
hold no character `security -i` splits on; `SIGNING-RECORD.txt` carries
`payload_names`, `mpb_sha256`, `mpb_cdhash`); `check-no-secrets.sh` →
`663a207d722cf5b26988ff3c1b3ae3a22d8b021bb9a32cbc88ca612f2023c586` (untracked,
non-ignored files; two EC key forms; the git environment cleared). The plan's
`build_release_never_executes_the_input_on_the_sign_path` and binding refusals
now exist in `test-gates.sh`.

**`mpb`.** `PAYLOAD=(macportsbrew mpb)` drives every list and hash. The `.pkg`
installs `./bin/macportsbrew` and `./bin/mpb` (`0755`), the `.dmg` holds both,
and a bare `mpb-<version>-<arch>` is published beside the bare executable, so
`SHA256SUMS.<arch>` lists four files (asserted). That bare file goes beyond
DP-033 (b) as ruled; the owner kept it (**DP-052 (a)**, 2026-10-23), and ADR-0005's
Decision now says four artifacts. `mpb` is a byte-identical copy
of the signed executable rather than the second `[[bin]]` signed on its own: a
copy lets every assertion say "the same bytes", and it carries the same CDHash
(asserted), so the executable's notarisation ticket is **expected** to cover it
and it is not submitted separately. That is an inference, not an observation:
the bare executable itself is still submitted as a ZIP until the first release
shows how tickets are found (R-19 finding 6), and the first signed run is where
`mpb`'s coverage is first shown. The `._mpb` AppleDouble entry is tolerated in a dry run
only, exactly as `._macportsbrew` is. The release record gains `payload_names`,
`mpb_sha256` and `mpb_cdhash`. Unsigned dry run for `x86_64-apple-darwin`,
2026-10-23, from the revised copy: exit 0; the `.pkg` payload listed `.`,
`./bin`, `./bin/macportsbrew`, `./bin/mpb`; the `.dmg` held both; all four
`SHA256SUMS` entries verified (`607af296…` for both executables). arm64 and the
signed path are not exercised.

**Signing step.** As the R-19 addendum's recipe: `openssl rand -hex 24`, masked;
`sec_i() { printf '%s\n' "$1" | "$sec" -i; }`; one `sec_i … || exit 1` per
`create-keychain`, `set-keychain-settings`, `unlock-keychain`, `import`,
`set-key-partition-list`; the "deviation" comment removed. Whether the
stdin-built keychain signs is still first shown by the owner's dry run with the
real certificates.

**Not done here, accepted in writing** (SE-102, the pass report): the plan's
`build_release_never_executes_the_input_on_the_sign_path` and
`--expect-unsigned-sha256` refusal tests (the script's comment names the first;
it does not exist yet), and the review's Lows.

## Applying them — one armed message, guarded writes first, hook tests last

The owner sends one message containing `MPB_ALLOW_GUARD_EDIT=1` and asking for
commit 3 and commit 4 of the plan. In that message the agent:

1. writes `scripts/build-release.sh` from the block below and checks
   `shasum -a 256 scripts/build-release.sh` against
   `2b5fab09f93157330e44d6f39e30ea276c92747929f26ad5844e37c2a706cd8a`;
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
   `544ec1e6e289176980b29bfc2098e77b73821333b932e8c6155b49c9329b5c23`,
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

**The repository (DP-007, DP-031).** Done 2026-10-16: the repository is
`pabloguia/MacPortsBrew`, public (DP-031 resolved (b), a personal account), and
the workspace `repository` field says so. D-S3's identifier is
`io.github.pabloguia.macportsbrew` (DP-035, ruled 2026-10-23). Still to do in Settings → General:
enable **immutable releases** (08 MUST).

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

- **DP-035 ruled 2026-10-23** (D-S3, D-P1) — the identifier
  `io.github.pabloguia.macportsbrew` and the install directory
  `~/.macportsbrew/bin`; both are permanent once a release ships. Nothing left
  to decide.
- **DP-034 ruled 2026-10-23: (b) first** (D-S1) — `security -i` (stdin) is
  researched on a hosted runner with a throwaway keychain and no secrets; if it
  does not hold, (a): accept the two per-job random values in argv on the
  ephemeral VM. **Open until that research is written.**
- **DP-033 ruled 2026-10-23: (b)** (D-B2) — the `.pkg` and `.dmg` also carry
  `mpb`; the charter stands. The work this adds to part B is under "Rulings of
  2026-10-23" in the addendum.
- **DP-036** — whether DP-019's ".app" means the `.dmg`.

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
     io.github.pabloguia.macportsbrew`, run as that user);
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
`2b5fab09f93157330e44d6f39e30ea276c92747929f26ad5844e37c2a706cd8a`, 719
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
# Owner decision D-S3, ruled 2026-10-23 (DP-035): io.github.pabloguia.macportsbrew,
# following the repository's account (github.com/pabloguia/MacPortsBrew, DP-031).
# Permanent once a release is published.
readonly IDENTIFIER="io.github.pabloguia.macportsbrew"
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
`544ec1e6e289176980b29bfc2098e77b73821333b932e8c6155b49c9329b5c23`, 179
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
#      "none at all";
#  10. nothing in release.yml builds with a feature: (a) no non-comment line
#      names `cargo` at all, so every release build goes through
#      scripts/build-release.sh and whatever spelling a cargo line could take
#      is moot; (b) no non-comment line holds `--features` or `--all-features`,
#      quoted or not; (c) no `-F`, alone or inside a cluster of short flags
#      (`-rF`), quoted or not, on a line that names build-release.sh, a
#      backslash continuation counted as one line. A release is built without
#      `test-support`, which compiles the test-only mutation allowance in
#      (pass 08 slice 4 review L1). Only release.yml is read, so validate.sh's
#      own `cargo test --all-features` is not touched. scripts/build-release.sh
#      asks cargo the same question of the dependency graph.
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

  # 10. Nothing builds with a feature. (a) No cargo at all in the file's code:
  #     comment lines are not code, and the release is built by build-release.sh.
  if hits="$(hits_of "$body" '(^|[^A-Za-z0-9_-])cargo([^A-Za-z0-9_-]|$)')"; then
    fail "release.yml names cargo; a release is built only through scripts/build-release.sh (pass 08 slice 4 review L1):"$'\n'"$hits"
  fi
  #     (b) No long feature flag anywhere, in any quoting or in a variable.
  if hits="$(hits_of "$body" '(^|[^A-Za-z0-9_-])--(all-)?features([^A-Za-z0-9_-]|$)')"; then
    fail "a feature flag in the release workflow; test-support must not be in a release (pass 08 slice 4 review L1):"$'\n'"$hits"
  fi
  #     (c) No -F on a line that calls the release build. Continuations are
  #     joined first, so a flag on the next line of a command is on its line.
  joined="$(awk '
    /^[[:space:]]*#/ { next }
    { sub(/[[:space:]]+$/, "") }
    /\\$/ { sub(/\\$/, ""); buf = buf $0 " "; next }
    { print buf $0; buf = "" }
    END { if (buf != "") print buf }' "$release")"
  if hits="$(printf '%s\n' "$joined" | grep -E 'build-release\.sh' | grep -E "(^|[[:space:]\"'])-[A-Za-z]*F")"; then
    fail "a -F flag on a line that calls the release build (pass 08 slice 4 review L1):"$'\n'"$hits"
  fi
fi

if [ "$failures" -gt 0 ]; then
  echo "check-release-workflow: $failures rule(s) broken" >&2
  exit 1
fi
echo "check-release-workflow: release workflow rules hold"
```


## Gate tests for `build-release.sh`

SHA-256 `275fe4f5b25f4695790edc8b2475bc4ccd791c338506ad629944acc88e4e866e`. Appended to `scripts/test-gates.sh` before its closing
summary. Run against the staged script on 2026-10-22: 20 cases, 0 failures;
16 mutants of the script, each killed.

```bash
echo "== build-release.sh: a release is built free of the test-support feature (pass 15)"
# The script under test is the real one, run with a stand-in `cargo` that answers
# `tree` from a file and records every call WITH ITS ARGUMENTS, so each case shows
# which cargo commands the script reached and what it asked. The tripwire in
# crates/cli/tests/history_preconditions.rs asks that this check exist; these
# cases ask that it can say no, and that it asks the question it should.
BR="${MPB_GATES_BUILD_RELEASE:-scripts/build-release.sh}"
if [ "$(uname -s)" != Darwin ]; then
  note SKIP "build-release.sh test-support cases" "needs macOS's packaging tools"
elif [ ! -x "$BR" ]; then
  note FAIL "build-release.sh is missing or not executable" "$BR"; miss
else
  tg="$(mktemp -d "${TMPDIR:-/tmp}/mpb-gates.XXXXXX")"
  mkdir "$tg/bin" "$tg/home0" "$tg/home1"
  cat >"$tg/bin/cargo" <<'STUB'
#!/bin/sh
case "$1" in
  tree) echo "$*" >>"$STUB_LOG"
        if [ -n "${STUB_TREE_FAIL:-}" ]; then echo "error: stub tree failure" >&2; exit 101; fi
        cat "$STUB_TREE" ;;
  build) echo "$*" >>"$STUB_LOG"; echo "stub: no build here" >&2; exit 1 ;;
  *) echo "unexpected: $*" >>"$STUB_LOG"; exit 99 ;;
esac
STUB
  printf '#!/bin/sh\nexit 0\n' >"$tg/bin/rustc"
  chmod 0755 "$tg/bin/cargo" "$tg/bin/rustc"
  printf '[build]\nrustc-wrapper = "wrap"\n' >"$tg/home1/config.toml"

  # The exact question the check must ask: this package, locked, for the target,
  # over the edges a release build uses, listing each package's enabled features.
  TREE_CALL="tree --locked -p macportsbrew --target x86_64-apple-darwin --prefix none -e normal,build -f {p} {f}"
  n=0
  br_case() { # <label> <want> <tree text> <extra args...>   (ENVV, CASE_CARGO_HOME optional)
    local label="$1" want="$2" tree="$3" out rc log verdict first second pat; shift 3
    n=$((n + 1))
    printf '%s\n' "$tree" >"$tg/tree"
    : >"$tg/log"
    out="$(env PATH="$tg/bin:$PATH" STUB_LOG="$tg/log" STUB_TREE="$tg/tree" \
           STUB_TREE_FAIL="$([ "$want" = refused-tree-failed ] && echo 1)" \
           CARGO_HOME="${CASE_CARGO_HOME:-$tg/home0}" ${ENVV:+"$ENVV"} \
           "$BR" x86_64-apple-darwin "$tg/out$n" "$@" 2>&1)"
    rc=$?
    log="$(cat "$tg/log")"
    first="$(printf '%s\n' "$log" | sed -n 1p)"
    second="$(printf '%s\n' "$log" | sed -n 2p)"
    verdict=1
    case "$want" in
      refused-flag|refused-env|refused-config)
        # Refused before cargo is asked at all.
        if [ "$rc" -ne 0 ] && [ -z "$log" ]; then
          case "$want" in
            refused-flag)   pat='usage:' ;;
            refused-env)    pat='is set; a release is compiled' ;;
            refused-config) pat='names a compiler' ;;
          esac
          printf '%s' "$out" | grep -q "$pat" && verdict=0
        fi ;;
      refused)             [ "$rc" -ne 0 ] && [ "$first" = "$TREE_CALL" ] && [ -z "$second" ] \
                             && printf '%s' "$out" | grep -q 'test-support feature resolves' && verdict=0 ;;
      refused-tree-failed) [ "$rc" -ne 0 ] && [ "$first" = "$TREE_CALL" ] && [ -z "$second" ] \
                             && printf '%s' "$out" | grep -q 'cargo tree failed' && verdict=0 ;;
      reached-build)       [ "$rc" -ne 0 ] && [ "$first" = "$TREE_CALL" ] \
                             && case "$second" in "build "*) true ;; *) false ;; esac \
                             && printf '%s' "$out" | grep -q 'cargo build failed' && verdict=0 ;;
    esac
    if [ "$verdict" -eq 0 ]; then pass; note ok "$label"; else
      note FAIL "$label" "rc=$rc first='$first' second='$second'"; miss
    fi
    ENVV=""; CASE_CARGO_HOME=""
  }

  CLEAN='macportsbrew v0.1.0 (/r/crates/cli) default
macportsbrew-application v0.1.0 (/r/crates/application)
macportsbrew-domain v0.1.0 (/r/crates/domain) '
  br_case "a feature flag is refused before cargo is asked: --features"      refused-flag "$CLEAN" --features test-support
  br_case "a feature flag is refused before cargo is asked: --features="     refused-flag "$CLEAN" --features=test-support
  br_case "a feature flag is refused before cargo is asked: --all-features"  refused-flag "$CLEAN" --all-features
  br_case "a feature flag is refused before cargo is asked: -F"              refused-flag "$CLEAN" -F test-support
  br_case "a dependency with test-support is refused"        refused "macportsbrew v0.1.0 (/r/crates/cli) default
macportsbrew-application v0.1.0 (/r/crates/application) test-support"
  br_case "test-support set on the root package is refused"  refused "macportsbrew v0.1.0 (/r/crates/cli) test-support"
  br_case "test-support first in a feature list is refused"  refused "macportsbrew-application v0.1.0 (/r/a) test-support,other"
  br_case "test-support last in a feature list is refused"   refused "macportsbrew-application v0.1.0 (/r/a) default,test-support"
  br_case "test-support in the middle is refused"            refused "macportsbrew-application v0.1.0 (/r/a) a,test-support,b"
  br_case "a repeated package (marked (*)) is refused too"   refused "macportsbrew-application v0.1.0 (/r/a) default,test-support (*)"
  # Under pipefail, a `grep -q` pipeline stops reading at its first match and can
  # kill a still-writing printf (status 141 reads as "no match"). Above 64 KB of
  # graph that is what happened; the match is on the FIRST line here.
  BIG="$({ echo 'macportsbrew-application v0.1.0 (/r/a) test-support'; yes 'macportsbrew-domain v0.1.0 (/r/d) default' | head -4000; })"
  br_case "a graph over 64 KB with the feature first is refused" refused "$BIG"
  br_case "a cargo that cannot answer is not a clean answer" refused-tree-failed "$CLEAN"
  br_case "a clean graph goes on to the build, asking the exact question" reached-build "$CLEAN"
  # The pattern must not be wider than the feature name.
  br_case "a package named for it is not the feature"        reached-build "macportsbrew-test-support-helper v0.1.0 (/r/h) default"
  br_case "a longer feature name is not the feature"         reached-build "macportsbrew-application v0.1.0 (/r/a) test-supported,x-test-support"
  # The feature is also reachable as a bare cfg through a compiler wrapper, which
  # cargo tree cannot see: refused in the environment and in cargo configuration.
  ENVV="RUSTC_WRAPPER=wrap"           br_case "RUSTC_WRAPPER is refused"                refused-env "$CLEAN"
  ENVV="RUSTC_WORKSPACE_WRAPPER=wrap" br_case "RUSTC_WORKSPACE_WRAPPER is refused"      refused-env "$CLEAN"
  ENVV="CARGO_BUILD_RUSTC_WRAPPER=w"  br_case "CARGO_BUILD_RUSTC_WRAPPER is refused"    refused-env "$CLEAN"
  ENVV="RUSTC=other-rustc"            br_case "RUSTC is refused"                        refused-env "$CLEAN"
  CASE_CARGO_HOME="$tg/home1"         br_case "a rustc-wrapper in cargo's config is refused" refused-config "$CLEAN"
  rm -rf "$tg"
fi
```

## Gate tests for `check-release-workflow.sh`

SHA-256 `e1871acc98f7b3599b7734f2d17ad087a809bc907950fbed504cf50627b8e799`. Appended to `scripts/test-gates.sh` after the block above.
Run against the staged script and the staged `release.yml` on 2026-10-22: 44
cases, 0 failures; 12 mutants of the script, each killed.

```bash
echo "== check-release-workflow.sh: each rule can say no (pass 09 D-T2, pass 15 rule 10)"
# Constructed workflow trees, each a copy of the repository's own workflows plus
# the release workflow with ONE change, run through the real script with
# MPB_WORKFLOW_DIR / MPB_ACTIONS_DIR. The script passes the unchanged tree and
# must refuse every changed one; a rule that cannot say no is a comment.
CRW="${MPB_GATES_CHECK_RELEASE_WORKFLOW:-scripts/check-release-workflow.sh}"
REL="${MPB_GATES_RELEASE_YML:-.github/workflows/release.yml}"
if [ ! -x "$CRW" ]; then
  note FAIL "check-release-workflow.sh is missing or not executable" "$CRW"; miss
elif [ ! -f "$REL" ]; then
  note FAIL "release.yml is missing" "$REL"; miss
else
  wg="$(mktemp -d "${TMPDIR:-/tmp}/mpb-gates.XXXXXX")"
  wf_case() { # <label> <accept|reject> <target: release|other|action> <python: transforms the text `t`>
    local label="$1" want="$2" target="$3" py="$4" d rc
    d="$(mktemp -d "$wg/case.XXXXXX")"
    mkdir "$d/wf" "$d/act"
    for f in .github/workflows/*.yml; do
      [ "$(basename "$f")" = release.yml ] || cp "$f" "$d/wf/"
    done
    cp "$REL" "$d/wf/release.yml"
    printf 'name: composite\nruns:\n  using: composite\n  steps: []\n' >"$d/act/action.yml"
    case "$target" in
      release) f="$d/wf/release.yml" ;;
      other)   f=""; for c in "$d"/wf/*.yml; do [ "$(basename "$c")" = release.yml ] || { f="$c"; break; }; done ;;
      action)  f="$d/act/action.yml" ;;
    esac
    PYSRC="$py" python3 - "$f" <<'PYEOF' || { note FAIL "$label" "the construction did not apply"; miss; return; }
import os, sys
p = sys.argv[1]
t = open(p).read()
before = t
exec(os.environ["PYSRC"])
if t == before:
    sys.exit(1)
open(p, "w").write(t)
PYEOF
    MPB_WORKFLOW_DIR="$d/wf" MPB_ACTIONS_DIR="$d/act" "$CRW" >/dev/null 2>&1
    rc=$?
    if { [ "$want" = accept ] && [ "$rc" -eq 0 ]; } || { [ "$want" = reject ] && [ "$rc" -eq 1 ]; }; then
      pass; note ok "$label"
    else
      note FAIL "$label" "wanted $want, exit $rc"; miss
    fi
  }
  # After the signing job's environment line, so the text lands inside that job.
  in_sign() { printf 't = t.replace("    environment: release-signing\\n", "    environment: release-signing\\n%s\\n", 1)' "$1"; }
  # After the first `steps:` of the file, which is not the signing job's.
  in_first() { printf 't = t.replace("    steps:\\n", "    steps:\\n%s\\n", 1)' "$1"; }
  at_end() { printf 't = t + "\\n%s\\n"' "$1"; }

  wf_case "the workflows as they are"                         accept release 't = t + "\n# a comment may name --all-features and pull_request_target\n"'
  wf_case "release-signing named in another workflow"         reject other   "$(at_end '    environment: release-signing')"
  wf_case "a second environment selection line"               reject release "$(in_sign '    environment: release-signing')"
  wf_case "the environment as a mapping"                      reject release 't = t.replace("environment: release-signing", "environment:\n      name: release-signing", 1)'
  wf_case "secrets.MPB_* in another workflow"                 reject other   "$(at_end '      - run: echo ${{ secrets.MPB_TEAM_ID }}')"
  wf_case "secrets.MPB_* in a composite action"               reject action  "$(at_end '      - run: echo ${{ secrets.MPB_TEAM_ID }}')"
  wf_case "secrets. outside the signing job"                  reject release "$(in_first '      FOO: ${{ secrets.GITHUB_TOKEN }}')"
  wf_case "secrets[...] in another workflow"                  reject other   "$(at_end "      - run: echo \${{ secrets['X'] }}")"
  wf_case "toJSON(secrets)"                                   reject other   "$(at_end '      - run: echo ${{ toJSON(secrets) }}')"
  wf_case "cargo in the signing job"                          reject release "$(in_sign '      - run: cargo build')"
  wf_case "rustup in the signing job"                         reject release "$(in_sign '      - run: rustup show')"
  wf_case "a Rust toolchain action in the signing job"        reject release "$(in_sign '      - uses: dtolnay/rust-toolchain@x')"
  wf_case "validate.sh in the signing job"                    reject release "$(in_sign '      - run: scripts/validate.sh')"
  wf_case "run-fixture-matrix in the signing job"             reject release "$(in_sign '      - run: scripts/run-fixture-matrix.sh')"
  wf_case "set -x"                                            reject release "$(in_first '      - run: set -x')"
  wf_case "set -eux"                                          reject release "$(in_first '      - run: set -eux')"
  wf_case "set -e -x"                                         reject release "$(in_first '      - run: set -e -x')"
  wf_case "set -o xtrace"                                     reject release "$(in_first '      - run: set -o xtrace')"
  wf_case "SHELLOPTS"                                         reject release "$(in_first '      - run: SHELLOPTS=xtrace true')"
  wf_case "BASH_XTRACEFD"                                     reject release "$(in_first '      - run: BASH_XTRACEFD=3 true')"
  wf_case "bash -x"                                           reject release "$(in_first '      - run: bash -x script.sh')"
  wf_case "an extra trigger"                                  reject release 't = t.replace("  workflow_dispatch:", "  pull_request:\n  workflow_dispatch:", 1)'
  wf_case "a tag filter that is not v*"                       reject release 't = t.replace("tags: [\"v*\"]", "tags: [\"*\"]", 1)'
  wf_case "a branches filter beside the tags"                 reject release 't = t.replace("tags: [\"v*\"]", "tags: [\"v*\"]\n    branches: [main]", 1)'
  wf_case "workflow_run in another workflow"                  reject other   "$(at_end '  workflow_run:')"
  wf_case "pull_request_target in another workflow"           reject other   "$(at_end '  pull_request_target:')"
  wf_case "ACTIONS_STEP_DEBUG"                                reject release "$(in_first '      ACTIONS_STEP_DEBUG: true')"
  # Rule 10: nothing in the release workflow builds with a feature (pass 08 slice 4 review L1).
  wf_case "--features on the release build"                   reject release 't = t.replace("scripts/build-release.sh \"$TARGET\" \"$RUNNER_TEMP/unsigned/out\"\n", "scripts/build-release.sh \"$TARGET\" \"$RUNNER_TEMP/unsigned/out\" --features test-support\n", 1)'
  wf_case "-F on the release build"                           reject release 't = t.replace("scripts/build-release.sh \"$TARGET\" \"$RUNNER_TEMP/unsigned/out\"\n", "scripts/build-release.sh \"$TARGET\" \"$RUNNER_TEMP/unsigned/out\" -F test-support\n", 1)'
  wf_case "-rF in a short-flag cluster on the release build"  reject release 't = t.replace("scripts/build-release.sh \"$TARGET\" \"$RUNNER_TEMP/unsigned/out\"\n", "scripts/build-release.sh \"$TARGET\" \"$RUNNER_TEMP/unsigned/out\" -rF test-support\n", 1)'
  wf_case "a quoted \"-F\" on the release build"             reject release 't = t.replace("scripts/build-release.sh \"$TARGET\" \"$RUNNER_TEMP/unsigned/out\"\n", "scripts/build-release.sh \"$TARGET\" \"$RUNNER_TEMP/unsigned/out\" \"-F\" test-support\n", 1)'
  wf_case "-F on the continuation of build-release.sh"        reject release "$(in_first $'      - run: |\\n          scripts/build-release.sh t o \\\\\\n            -F x')"
  wf_case "a quoted --features="                              reject release "$(in_first '      - run: echo \"--features=test-support\"')"
  wf_case "a single-quoted --all-features"                    reject release "$(in_first "      - run: echo '--all-features'")"
  wf_case "the flag held in a variable"                       reject release "$(in_first '      - run: F=--features=x true')"
  wf_case "--all-features on a cargo line"                    reject release "$(in_first '      - run: cargo build --release --all-features')"
  wf_case "cargo named at all"                                reject release "$(in_first '      - run: cargo --version')"
  wf_case "cargo with a toolchain selector and a short subcommand" reject release "$(in_first '      - run: cargo +stable b')"
  wf_case "cargo rustc"                                       reject release "$(in_first '      - run: cargo rustc --release')"
  wf_case "a cargo action"                                    reject release "$(in_first '      - uses: some/cargo@x')"
  wf_case "a cargo line split across lines"                   reject release "$(in_first $'      - run: |\\n          cargo \\\\\\n            build')"
  wf_case "a flag of another tool on a line that is not a build" accept release "$(in_first '      - run: grep -F x file')"
  wf_case "a commented build line with a flag"                accept release 't = t + "\n    # cargo build --all-features\n"'
  wf_case "a commented call of build-release.sh with -F"      accept release 't = t + "\n    # scripts/build-release.sh t o -F x\n"'
  rm -rf "$wg"
fi
```

## The structural test that replaces the tripwire

SHA-256 `c22d4bea20fbc91aec335afa665be04d06804c0f5122922b15ac07500731cef9`. Replaces `the_switch_needs_release_builds_free_of_the_test_support_feature`
in `crates/cli/tests/history_preconditions.rs`, in the same change that installs
the scripts it reads.

```rust
/// Detection: a release is built without the `test-support` feature (slice 4
/// review L1), which compiles the test-only mutation allowance in. A positive
/// check, held whatever the switch is. It looks for the check's structure, not
/// its words: `build-release.sh` asks cargo what the release resolves, for the
/// target, over the edges a release build uses, with the exact command line
/// below, and refuses when `test-support` is among it, when cargo cannot answer,
/// and when a compiler wrapper could add the cfg unseen; `check-release-
/// workflow.sh` refuses cargo and a feature flag in the release workflow; and
/// the release workflow builds only through `build-release.sh`. The behaviour is
/// proved by constructed cases in `scripts/test-gates.sh`, which also pin the
/// arguments cargo is given; this test is that the check cannot be deleted or
/// narrowed without a test failing here.
#[test]
fn the_switch_needs_release_builds_free_of_the_test_support_feature() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |rel: &str| {
        std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    };
    // Whitespace-free, so a re-wrapped line does not hide the check.
    let squeeze = |text: &str| text.split_whitespace().collect::<String>();
    let script = squeeze(&read("scripts/build-release.sh"));
    let check = squeeze(&read("scripts/check-release-workflow.sh"));
    let workflow = read(".github/workflows/release.yml");

    for (what, needle) in [
        (
            "it asks cargo the whole question, and nothing narrower",
            r#"graph="$(cargotree--locked-pmacportsbrew--target"$target"\--prefixnone-enormal,build-f'{p}{f}'2>&1)""#,
        ),
        (
            "it matches the feature as a whole word, here-string not pipeline",
            r#"grep-E'(^|[,])test-support([,]|$)'<<<"$graph"||true"#,
        ),
        (
            "refusing when it resolves",
            "test-supportfeatureresolvesintothisbuild",
        ),
        (
            "refusing when cargo cannot answer",
            "cargotreefailed;cannotshowthatthebuildisfree",
        ),
        (
            "refusing a compiler wrapper in the environment",
            "forvarinRUSTCRUSTC_WRAPPERRUSTC_WORKSPACE_WRAPPER",
        ),
        (
            "refusing a compiler wrapper in cargo's configuration",
            "rustc-wrapper|rustc-workspace-wrapper",
        ),
    ] {
        assert!(
            script.contains(needle),
            "scripts/build-release.sh: {what}: `{needle}` is gone (slice 4 review L1)"
        );
    }
    for (what, needle) in [
        (
            "cargo anywhere in the release workflow",
            "release.ymlnamescargo",
        ),
        ("a long feature flag anywhere in it", "--(all-)?features"),
        ("-F on a call to the release build", "build-release"),
    ] {
        assert!(
            check.contains(needle),
            "scripts/check-release-workflow.sh no longer refuses {what}"
        );
    }
    let names_cargo = workflow.lines().any(|l| {
        let l = l.trim_start();
        !l.starts_with('#')
            && l.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
                .any(|w| w == "cargo")
    });
    assert!(
        !names_cargo && workflow.contains("scripts/build-release.sh"),
        "release.yml must build only through scripts/build-release.sh, so the check applies"
    );
}
```
