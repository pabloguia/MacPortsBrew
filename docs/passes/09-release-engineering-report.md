<!-- SPDX-License-Identifier: Apache-2.0 -->

# 09 — Release engineering: pass report

Work order: [`../../prompts/09-release-engineering.md`](../../prompts/09-release-engineering.md).
Plan, with every decision argued and every review finding dispositioned:
[`09-release-engineering-plan.md`](09-release-engineering-plan.md). Written
2026-10-11.

**Status: the release machinery is written, reviewed twice and staged; nothing
is released and nothing in `.github/workflows/` or `scripts/` was written.** The
SE-093 escape was not armed during the pass, so the guarded files wait in
[`09-staged/`](09-staged/README.md) for the owner's armed write, and the first
tag waits on five owner decisions. `/preflight` was not run by the agent that
wrote this report (its documentation work is uncommitted working-tree edits for
the coordinator to review); `scripts/check-links.sh` was run and passes.

## What was built

A user still cannot download MacPortsBrew. What exists now is everything
needed to make the first release a mechanical act:

- **`scripts/build-release.sh`** (staged): one invocation per architecture
  builds with the deployment target set on that `cargo` invocation only, signs
  the executable once (Developer ID, hardened runtime, secure timestamp — or
  ad-hoc in a dry run), builds a current-user-domain `.pkg` and an HFS+ UDZO
  `.dmg` around that one executable, notarises the bare file (as a ZIP) and both
  containers, staples the containers, and refuses to finish unless every
  assertion holds: the recorded minimum macOS equals the floor by `otool` and by
  `vtool`; only system libraries; no host path; the payload extracted from each
  container hashes to the bare executable **and** the payload file list and
  modes are exact; Developer ID authority, team and timestamp on the executable
  and on the stapled `.dmg`; pkgutil's notarisation line; Gatekeeper accepting
  both containers. It never executes its input when signing, and binds that
  input to the build job's unsigned bytes by hash before touching the keychain.
- **`.github/workflows/release.yml`** (staged): a tag push builds both
  architectures in a job with no secret (the only place the built program is
  executed), signs in a separate job that alone holds the `release-signing`
  environment, assembles `SHA256SUMS` and a signing record carrying the build
  toolchain, attests every artifact plus `SHA256SUMS` and the signing record,
  and **drafts** a release — it never publishes. A dispatch is always an
  unsigned dry run, and says so in its bundle and notes.
- **`scripts/check-release-workflow.sh`** (staged): a `validate.sh --quick`
  stage that turns 08's and ADR-0005's textual workflow rules into a failing
  check — triggers pinned, the environment selected once and only in
  `release.yml`, no compilation in the signing job, `secrets.` only there, no
  xtrace in any spelling, no `secrets[…]` / `toJSON(secrets)`, composite
  actions scanned.
- **`--version`** names the policy data, the target and the commit (commit
  `39b33e5`, landed by the coordinator; SE-011 addendum).
- **Documentation** (working tree, for review): README's Install section is now
  the single source of the release notes, between
  `<!-- release-notes:begin -->` and `<!-- release-notes:end -->`; it says
  macOS 10.12–10.15 are best-effort in those words, gives one complete `curl`
  line, names the shell files, states the unverified `.pkg` behaviour as
  "designed to", and pins verification to the release workflow and tag.
  `wiki/Getting-Started.md` links to it. R-11 and DP-019 carry the Intel-runner
  correction; `docs/spec-evolution.md` records the distribution (SE-067
  addendum) and the best-effort wording and runner correction (SE-037
  addendum).

## What was verified, and how

- **Dry runs, unsigned, `x86_64-apple-darwin`.** The first version
  (`dfaf2625…`) on 2026-10-10 against a clean clone of `ea46a1f`, twice: the
  unsigned output and the ad-hoc-signed executable rebuilt byte-identically,
  the containers did not, their payloads did (plan, fact 15). Its first run
  failed its own assertion on `com.apple.provenance` AppleDouble entries, as
  R-19 predicted; those are tolerated in an unsigned run only (D-P3). The
  current version (`c8d3b0f5…`) on 2026-10-11 at `39b33e5` (one tracked
  document modified, so the record says `-modified`): exit 0; minimum macOS
  10.12 by both tools; payload list and modes exact; `.dmg` HFS+, UDZO.

  | 2026-10-11 output | SHA-256 |
  |---|---|
  | unsigned build output | `3e6a58c9c9f132fad6a3cc5299a6685f77aef2496f33651714d6f11fceb4d763` |
  | bare executable (ad-hoc) | `70b9f8609a0b172585be9f5447a9169fe4a802f72017eebf327ff57bc4cbfce5` |
  | `.pkg` | `5038f85fee267487f2b6aba45b89e95064f1c83e1e92bff3cb900d205cd379e4` |
  | `.dmg` | `02229c95ed6a3191060bdf979e49325cbf024871c896ec920b355d0b6cf58ab8` |

  Refusals observed on the real tree: `--sign` without
  `--expect-unsigned-sha256`; `--release-input` on a dirty tree. Argument
  refusals observed from the scratchpad copy: `--release-input` with `--sign`
  or `--from-binary`, a malformed hash, the hash without `--sign`.
- **The workflow check** accepts the current workflows plus the staged
  `release.yml` and refuses each of 24 constructed violations, at least one per
  rule (2026-10-11).
- **Static checks:** both scripts shellcheck 0.11.0 clean, `build-release.sh`
  `bash -n` clean; the staged workflow actionlint 1.7.12 clean.
- **Not exercised:** the arm64 build (this machine is x86_64), the signed path
  (no Developer ID identity exists here), the workflow itself (it has never
  run), the `.pkg` under Installer.

## Staged for the owner's armed write

[`09-staged/README.md`](09-staged/README.md) holds the files, the exact apply
sequence (guarded writes first, hook tests last) and the SHA-256 each written
file must match:

| Staged | Applies to | SHA-256 |
|---|---|---|
| "The script" (embedded) | `scripts/build-release.sh` | `c8d3b0f510fe82687643c6d5b7649aa590fbcadde73dd15af39b8a3b4fc59e67` (680 lines) |
| "The workflow check" (embedded) | `scripts/check-release-workflow.sh` | `ab444b43555bf565123a89288aa35df0f8184a46a50f79e55fd67c378431d7c3` (147 lines) |
| `workflows/release.yml` | `.github/workflows/release.yml` | copied verbatim |

The same armed message also makes three one-line or small edits described
there: `MPB_FIXTURE_BIN` in `scripts/run-fixture-matrix.sh`, three action
repositories in `scripts/check-action-pins.sh`, `scripts/check-no-secrets.sh`
and both new `validate.sh` stages, and the gate tests (the sign-path
no-execution test, the binding refusals, the 24 workflow-check cases). The
scripts are embedded in a Markdown file because the path guard refuses
`*/scripts/*.sh` anywhere in the tree, including under `09-staged/`.

## What the owner must do before the first tag

The full checklist — repository, ruleset (Active, bypass owner only, no GitHub
Apps), environment (required reviewer, no administrator bypass), secrets and
variables, certificates, notary key, draft verification before publishing, the
tag command — is in [`09-staged/README.md`](09-staged/README.md), "Owner
checklist for the first real release". The items that block the first tag:

1. **Rule the open decisions** (below), DP-033 first among them.
2. **Run the dispatch dry run** after README's markers are committed, and check
   whether the hosted runner stamps `com.apple.provenance` (if it does, the
   signed run refuses, by design).
3. **The `.pkg` check** in a Standard (non-administrator) account on the oldest
   macOS available: where it lands, no password prompt, `stat -f '%Su %Sp'` of
   the directory, `bin` and binary (user-owned, `0755`, not group-writable),
   the receipt, no `PATH` change — recorded in R-19.
4. **Observe the credential import once** with a throwaway `.p12` on a runner
   (LibreSSL re-wrap of an OpenSSL-3 export, `security import`, the partition
   list). It fails closed.
5. **The owed research** (an R-19 addendum, dated): GitHub's attestation
   features and flags, `gh`'s minimum macOS, `security -i` if DP-034 (b), the
   offline Gatekeeper outcome (R-11 Q3), `shasum --ignore-missing` on 10.12.

## Decisions pending

| DP | Question | Default meanwhile |
|---|---|---|
| [DP-031](../decisions-pending.md) | Which GitHub account the repository lives under; then the `sign` job's `github.repository ==` condition | the workspace value |
| [DP-033](../decisions-pending.md) | No release artifact installs `mpb`, against the **charter** (Tier 2), 06, ADR-0003 and DP-005 | change no text; **first tag blocked** |
| [DP-034](../decisions-pending.md) | The temporary keychain and re-wrap passwords in `security`'s argv, against ADR-0005's text | as staged (argv, masked, ephemeral VM) |
| [DP-035](../decisions-pending.md) | The identifier `io.github.macportsbrew.macportsbrew` and the install directory `~/.macportsbrew/bin`, both permanent after a release | as staged |
| [DP-036](../decisions-pending.md) | Whether DP-019's ".app" means the `.dmg` | yes |

DP-019's runner-cost premise is corrected in place; its ruling stands.

## Specification changes

- `docs/spec-evolution.md`: SE-067 addendum (the distribution as built:
  D-B1, D-B3, D-R1, D-P1, D-P2, D-D1, D-S2, D-S3, D-N1, D-W1–D-W3, D-A1, D-T1,
  D-T2, and the one-source notes); SE-037 addendum (the Intel runner is free;
  best-effort said in those words at download time); SE-011 addendum (the
  `--version` block, merged by the coordinator).
- **Not written, deliberately:** the plan's proposed SE items 1 (`mpb`) and 2
  (the keychain password) wait on DP-033 and DP-034. Nothing in 06, ADR-0003,
  ADR-0005 or the charter was changed. `08-distribution-and-project.md` needed
  no edit: its ".dmg provisional" text was already in the past tense and it
  does not say "R-11 still open".
- No invariant is touched (plan, "Named tests"); no planner, adapter, executor
  or policy file changed.

## Reviews

| Round | Reviewer | Findings | Disposition |
|---|---|---|---|
| 1, 2026-10-10 | `security-reviewer` | 1 High (the signing job executed the built program with the keychain unlocked), 5 Medium, 6 Low | all fixed or routed; none open |
| 1, 2026-10-10 | `spec-guardian` | 3 High (the same High; D-B2 overriding 06; Developer ID authority not asserted on the final `.dmg`), 8 Medium, 8 Low | all fixed, routed to the owner, or recorded as owed |
| 2, 2026-10-11 | `spec-guardian` | B-H1 (`mpb` is a charter question), B-M1, B-L1–B-L3, B-L5 | DP-033 rewritten with a blocking default; the workflow check strengthened; the rest fixed or owed |
| 2, 2026-10-11 | `security-reviewer` | B-L1–B-L5 | the binding moved into the script; literal snippet check; the LibreSSL observation added to the owner's checks; the dry run done |

Every finding has a row in the plan's "Revision after review" table. The
end-of-pass reviews of the documentation edits in this report's commit are
still to run (`docs-auditor`, `spec-guardian`).

## Boundary

This pass's start SHA is `ea46a1f`. Commits by other sessions after it
(`ffdd3b3` and the pass 08 slice 3 commits) are excluded from the work order's
§3 boundary check, which runs over this pass's own commits: `39b33e5`
(`--version`), and the documentation commit the coordinator makes from this
working tree. `crates/*/src` changes in this pass: `crates/cli` only, the
version string; `Cargo.lock` unchanged.

## What is owed

- The armed write and its gate tests (above).
- The `doctor` finding kind ADR-0005 requires for `~/.macportsbrew/bin`
  (`path_audit.rs`), not done in this pass.
- A script assertion on the expanded `PackageInfo` once the `.pkg` behaviour is
  observed.
- One source for the deployment floors (spelled in the script,
  `check-deployment-target.sh` and the `rust-toolchain.toml` comment).
- `09-research-checklist.md`'s R-11 entry still quotes the billed-runner
  claim; that file's R-19 entry already says it is wrong. Marking the R-11
  entry superseded was outside this pass's writable files; R-11 itself and
  SE-037 carry the correction.
- After the first tag: confirm the pkgutil and spctl wordings the script
  expects, the nested-ticket question (R-19 f6), and the arm64 path.
