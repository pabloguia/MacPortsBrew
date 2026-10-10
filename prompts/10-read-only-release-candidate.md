<!-- SPDX-License-Identifier: Apache-2.0 -->

# MacPortsBrew — The read-only release candidate (10)

The release machinery exists (pass 09). This pass makes the **read-only build**
worth installing and honest about itself, and produces v0.1.0's release
candidate: a version a stranger can download, run on a machine with a working,
broken or absent Homebrew or MacPorts, understand, and get a correct plan they
can follow by hand. Nothing in this pass makes the tool change a machine.

---

## 1. Read this first

1. `AGENTS.md`, `CLAUDE.md`, `docs/spec/00-charter.md` (especially "release
   surfaces state what the build actually does" and the non-goals),
   `docs/spec/01-invariants.md` (INV-20: dated facts are data, never
   architecture).
2. `docs/spec/06-cli-and-ux.md` — the output contract, exit codes, i18n.
3. `docs/spec/08-distribution-and-project.md` — what release notes and install
   docs must say.
4. `docs/passes/09-release-engineering-report.md` — how a release is built.
5. `README.md`, `wiki/` (18 pages), `CHANGELOG.md`, `locales/en.toml`.

## 2. Authority and precedence

As `TEMPLATE-PASS.md` §2.

## 3. Hard boundary for this pass

- `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`; no file under
  `crates/cli/src/executor/`, `crates/application/src/executor*`,
  `execution_gate.rs`, `verification.rs` or `crates/adapters/src/process/`
  changes (`git diff --stat` on those paths is empty).
- No `--json` schema changes incompatibly (`schemaVersion` unchanged; any
  additive field is in `docs/schemas/*.v1.md` and a checked sample).
- Nothing is tagged or published; the owner does that.

## 4. Working protocol

As `TEMPLATE-PASS.md` §4. `docs-auditor` is the principal reviewer here; run it
early and at the end.

## 5. Order of work

1. **Truth sweep.** Every user-facing surface — `--help`, `README.md`, `wiki/`,
   release-notes draft, error remediations — says what v0.1.0 does and does not
   do, in the user's words: it reads, explains and plans; it does not install,
   remove, switch or restore. No surface implies otherwise (e.g. `migrate --yes`
   explains the refusal and what to do by hand). `docs-auditor` checks it.
2. **A user's first ten minutes.** Walk `wiki/Getting-Started.md` against every
   fixture in `fixtures/recordings/` via `--replay` and fix what confuses:
   empty states, a machine with neither provider, a broken provider, an
   Intel-timeline warning (INV-20: from `data/policy/`, dated). Add a CLI test
   per walkthrough step that pins the exit code and the first line of output.
3. **Following a plan by hand.** For a read-only release the plan *is* the
   product: make `plan` and `migrate` (without `--yes`) render each step as the
   command a user would type, its verification, and the `MANUAL_REVIEW` reasons,
   with a clear warning that the user runs it at their own risk and that
   `--retain-source` is the safe pattern. This is presentation in `crates/cli/`
   only; the plan type does not change.
4. **`doctor --report`** is what users attach to issues: confirm it redacts the
   home directory and user name everywhere (a test exists — extend it to every
   section), and write `.github/ISSUE_TEMPLATE` text **staged** under
   `docs/passes/10-staged/` if `.github/` is guarded.
5. **Policy freshness.** The policy data's staleness threshold is 180 days;
   decide with `/spec-evolution` what the release does when data is stale on the
   user's machine (it already says so; confirm it never refuses).
6. **Release candidate checklist** in the report: version bump to `0.1.0` in
   `Cargo.toml` and `--version`, `CHANGELOG.md` `[Unreleased]` → `[0.1.0]` with
   the date left for the owner, policy version stated, the three artifact
   descriptions, a manual Gatekeeper test protocol for the owner (download from
   the draft release on a clean user account, quarantined, each artifact).

## 6. Definition of done

- [ ] `docs-auditor` finds no surface that overstates the build.
- [ ] Walkthrough CLI tests exist for every fixture, proved by mutation of the
      rendering they pin.
- [ ] `doctor --report` redaction tested on every section.
- [ ] §3 holds.
- [ ] `/preflight`, `scripts/validate.sh`, `scripts/check-coverage.sh`; all three
      reviewers run, findings resolved or accepted in writing.
- [ ] The release-candidate checklist for the owner is in the report.

## 7. End-of-pass report

`/pass-report` to `docs/passes/10-read-only-release-candidate-report.md`.
