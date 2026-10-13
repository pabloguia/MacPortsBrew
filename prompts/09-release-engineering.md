<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — Release engineering (09)

MacPortsBrew has a tested read-only CLI and **no way to ship it**: no
build-and-package script, no signing or notarisation, no release workflow, no
checksum assertion, no tag. `docs/spec/08-distribution-and-project.md` and
ADR-0005 fix what a release is — per architecture, one signed Mach-O, published
bare, in a `.pkg` and in a `.dmg`, byte-identical in all three, with checksums,
for macOS 10.12+ on `x86_64` and 11.0+ on `arm64`. This pass builds that
machinery and proves it on an unsigned dry run, so that v0.1.0 (the read-only
release, pass 10) and every later release travel one reviewed path. It changes
nothing about what the binary does.

---

## 1. Read this first

1. `AGENTS.md`, `CLAUDE.md`; `docs/spec/00-charter.md`; `docs/spec/01-invariants.md`.
2. `docs/spec-evolution.md` — search it for SE entries on distribution,
   signing, DP-004, DP-008, DP-019, ADR-0005; do not reopen what they settle.
3. `docs/spec/08-distribution-and-project.md` — all of it; this pass implements
   its MUSTs.
4. `docs/adr/0005-release-signing-and-artifacts.md`,
   `docs/research/R-11-apple-signing-toolchain.md` (partially answered — its
   open questions are this pass's research), `docs/adr/0002-*` (the toolchain
   pin and the macOS floor).
5. `docs/spec/07-quality-and-testing.md` (deployment targets, CI matrix) and
   `.github/workflows/repository-validation.yml` (what CI already runs).
6. `docs/decisions-pending.md` — DP-004, DP-007, DP-008, DP-019 (Resolved);
   anything Open.
7. `prompts/ROADMAP-to-first-release.md` — the owner prerequisites table.

## 2. Authority and precedence

As `TEMPLATE-PASS.md` §2, unchanged.

## 3. Hard boundary for this pass

- **The product binary's behaviour does not change.** No file under
  `crates/*/src` changes except `--version`/build-metadata plumbing the plan
  names; `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`. Checked by
  `git diff <start>..HEAD --stat -- crates/*/src`.
- **No secret ever enters the repository, a log or a session.** Certificates,
  passwords and the notary API key exist only as GitHub Actions secrets the
  owner creates; workflows reference them by name; a test greps the tree for
  PEM headers, `p12`, `AuthKey_` and Apple team-id-shaped strings. The agent
  never types, reads or echoes a credential (prohibited action).
- **Nothing is published.** No tag pushed, no release created, no artifact
  uploaded; `git push` is denied anyway. The release job is proven on a
  `workflow_dispatch` dry run that signs nothing when the secrets are absent and
  says so.
- **No new runtime dependency** in the binary (`cargo tree -e normal` unchanged
  except build metadata); a new *build-time* tool (e.g. a `.dmg` builder) is an
  ADR under ADR-0001's gates, and the default is Apple's own `pkgbuild`,
  `productbuild`, `hdiutil`, `codesign`, `notarytool`, `stapler`.
- **Guarded paths only with the owner's armed escape** (SE-093): new files under
  `scripts/` and `.github/workflows/` are written in one armed message, guarded
  writes first, hook tests last. If the escape is not armed, **stage** the files
  under `docs/passes/09-staged/` and stop at the plan; do not route around the
  guard.
- `Cargo.lock` changes only if the plan names why.

## 4. Working protocol

As `TEMPLATE-PASS.md` §4, plus: plan mode first; the plan
(`docs/passes/09-release-engineering-plan.md`) is reviewed by
`security-reviewer` (supply chain, signing, secrets) and `spec-guardian` before
any guarded write. `/tooling-recheck` first — GitHub Actions runner labels,
`notarytool` flags and `macos-*` image contents are dated facts; pin every
action by SHA (`scripts/check-action-pins.sh` already enforces it).

## 5. Order of work

1. **Research before design** (`provider-researcher`, dated notes): close
   R-11's open questions — what `stapler` can and cannot attach to a bare
   Mach-O, a `.pkg` and a `.dmg`; Gatekeeper's behaviour on each when
   quarantined; whether `codesign --options runtime` plus `--timestamp` is
   required for notarisation of a CLI; minimum Xcode/SDK that still targets
   10.12 on `x86_64`; what a deterministic Rust build needs here
   (`--remap-path-prefix`, `SOURCE_DATE_EPOCH`, `-C strip`, `LC_UUID`
   stability). These decide whether "byte-identical" holds across rebuilds or
   only across the three containers of one build — **write down which**.
2. **The plan**, reviewed, then committed.
3. **`scripts/build-release.sh`** (guarded, armed): one invocation per
   architecture produces the Mach-O with the deployment target pinned, records
   its SHA-256, wraps it in `.pkg` (installs to one stated directory, **never
   edits `PATH`** — 08 forbids it) and `.dmg`, and **extracts the payload from
   both containers and asserts the hash equals the bare binary's**. Signing is a
   separate step that is skipped, loudly, when no identity is configured.
4. **`.github/workflows/release.yml`** (guarded, armed): on a tag (and on
   `workflow_dispatch` with `dry_run`), builds both architectures on native
   runners per DP-019, signs and notarises with the owner's secrets, staples
   what can be stapled, runs `scripts/validate.sh` and the fixture matrix
   against the *built* binary, generates `SHA256SUMS`, and drafts — never
   publishes — a release with the descriptions 08 requires. Least-privilege
   `permissions:`; secrets only in the signing step's environment.
5. **Release notes and install docs**: the three artifact descriptions 08
   mandates (technical name + what happens, no assumed knowledge of
   quarantine), the `.pkg`'s install directory and the one `PATH` line, the
   Gatekeeper section — in `README.md` and `wiki/Getting-Started.md`, from one
   source.
6. **Version and metadata**: `macportsbrew --version` prints tool version,
   policy version, target triple and commit; a test pins the shape.
7. **Dry run**: run `build-release.sh` locally unsigned for the host
   architecture into the scratchpad; record artifact hashes and the container
   payload assertion in the pass report.

## 6. Definition of done

- [ ] R-11's open questions answered or carried with a reason; ADR-0005 updated
      or confirmed in `docs/spec-evolution.md`.
- [ ] `build-release.sh` produces three artifacts per architecture whose payload
      hashes match, demonstrated by its own assertion on a local unsigned run.
- [ ] `release.yml` exists, pinned by SHA, and its dry-run path is described
      step by step in the report (it cannot run until the remote exists).
- [ ] No secret material anywhere in the tree (the grep test passes).
- [ ] §3 holds: product behaviour unchanged, nothing published.
- [ ] `/preflight`, `scripts/validate.sh`; reviewers run, findings resolved or
      accepted in writing.
- [ ] Owner checklist for the first real release written into the report:
      secrets to create (names only), runner cost, the exact tag command.

## 7. End-of-pass report

`/pass-report` to `docs/passes/09-release-engineering-report.md`.
