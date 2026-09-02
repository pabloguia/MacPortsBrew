# ADR-0005 — Release signing and distribution artifacts

- **Status:** accepted
- **Date:** 2026-09-02
- **Pass:** 02-ARCHITECTURE-REVIEW-PASS (owner decision)
- **Resolves:** DP-004

## Context

`08-distribution-and-project.md` requires a self-contained binary with no
runtime dependencies, published through GitHub Releases for `x86_64` and
`arm64` with checksums, and requires that the installation documentation lead
with that path. The constraint behind it is that MacPortsBrew must install for
someone whose package manager is already broken.

**Known, with sources.** R-9 (2026-09-02) established four things that bound
this decision: the blocker is the **quarantine attribute**, not the absence of
a signature; quarantine is applied by the *downloading application* through
Launch Services, so a browser sets it and `curl` does not; a **bare Mach-O
cannot be notarisation-stapled**; and `arm64` binaries are ad-hoc signed by
`ld` by default, which makes a post-link `strip` or `lipo` that invalidates the
signature a real failure mode. R-9 also mapped the Gatekeeper regime across the
supported range: no notarisation regime below 10.14.5, Developer ID required
from 10.14.5 and 10.15, and no Control-click override from macOS 15 onward.
R-10 (2026-09-02) established that standard GitHub-hosted runners are free and
unlimited on public repositories and that larger runners never are.

**What changed the calculus.** The recommendation recorded under DP-004 argued
for ad-hoc signing on four grounds. Two are now void: the owner already holds a
paid Apple Developer account, so the 99 USD/year is not a new recurring cost,
and the legal identity it is tied to is the owner's own name — which DP-006
independently resolved on the same day as the copyright holder. The signature
and the copyright line will name the same person, so signing discloses nothing
the repository does not already disclose. The third ground — that a bare binary
cannot be stapled — is not answered by the account; it is answered by shipping
containers that *can* be stapled alongside the bare binary. The fourth, that
`arm64` ad-hoc signing is free, remains true and is now superseded rather than
contradicted.

**What is not known.** Three things, gathered as **R-11**, blocking the
implementation pass that writes the release workflow rather than this decision:

1. **Whether stapling a container covers a binary copied out of it, or only the
   container itself.** This is the crux for both the `.pkg` and the `.dmg`: if
   stapling covers only the container, they buy offline verification of the
   *download* and not of the *installed tool*, and the `.dmg` in particular
   should be dropped rather than maintained.
2. Whether `codesign`, `notarytool` and `stapler` are present on the free
   `macos-15-intel` standard runner image, and at what Xcode floor. R-10
   established the image's existence and its preinstalled Homebrew; it did not
   establish the signing toolchain, and this ADR's compatibility argument
   assumes it.
3. What a quarantined, notarised, **unstapled** binary does on a machine with
   no network — a plausible state for exactly the user this chapter is written
   for, and the case where "signing removes the first-run obstacle" is least
   likely to hold.

Two things this ADR does **not** treat as unknown, because R-9 already sourced
them from Apple on 2026-09-02 and an earlier draft of this record wrongly listed
them as open: a **Developer ID Application** certificate signs Mach-O files,
disk images and command-line tools (R-9 C-3, quoting Apple); and the **hardened
runtime is required** for notarisation, with Apple naming command-line targets
explicitly (R-9 C-1, C-3, C-4). `notarytool` accepts a ZIP for a standalone
binary, with the ticket issued but not attachable (R-9 C-2).

A third was claimed as settled here and is not: **which certificate signs the
`.pkg`**. R-9's C-1 lists an "installer certificate" among the acceptable kinds
without saying it signs the `.pkg`, so treating that as cited was an inference
dressed as a citation — the failure this project is built to avoid. It is back
in R-11. R-9's own UNRESOLVED
experiment — a browser-downloaded, quarantined, *unsigned* binary run from a
shell on macOS 26 — survives as documentation of the path not taken.

## Options considered

**A — Bare binary, ad-hoc signed.** The reversible default held until today.
Costs nothing and forecloses nothing, and the honest quarantine documentation
it requires has independent value. Made moot by the account, not wrong.

**B — Bare binary only, Developer ID signed and notarised.** Removes the
first-run obstacle for browser downloads on 10.14.5+. Cannot be stapled, so
verification requires network on first run. Cheap: one signing step, no new
artifact. Forecloses offline verification entirely.

**C — Bare binary plus `.pkg` plus `.dmg`, all signed and notarised, the two
containers stapled.** Keeps the bare binary as the leading path the
specification mandates, and adds two artifacts that *can* carry a stapled
ticket. Costs a larger release job — three artifacts across two architectures,
six plus checksums — and a wider failure surface. Makes offline install
possible; makes the release pipeline something that must be tested.

**D — `.pkg` only.** Smallest matrix, best Gatekeeper story. Forecloses the
constraint the chapter is built on: a `.pkg` is an Apple Installer artifact and
the install is not a file copy the user can perform by hand when everything
else on the machine is broken.

## Decision

**Sign every release artifact with Developer ID, notarise it, and publish three
artifacts per architecture: the bare binary, a `.pkg` and a `.dmg`. The bare
binary remains the path the documentation leads with; the containers exist to
carry a stapled notarisation ticket. Sign with a secure timestamp.**

Chosen by the project owner, 2026-09-02.

## Consequences

**Positive.** A browser-downloaded artifact stops being a first-run obstacle on
every macOS version that has a notarisation regime. The `.pkg` is *intended* to
give an install path verifiable without network — stated as an intention rather
than a property, because R-11 finding 1 is exactly the question of whether a
stapled container's ticket reaches the tool once installed. The signature names
a real, accountable identity, consistent with the copyright line rather than in
tension with it.

**Requirement — the three artifacts embed byte-identical executables.** One
build produces one signed Mach-O per architecture and both containers embed that
exact file; CI asserts that the payload extracted from each container hashes to
the published bare-binary checksum. Without it a user can verify the bare binary
and install a `.pkg` whose payload that checksum never covered, and a pipeline
compromise gets somewhere to hide.

**Negative — the release pipeline now holds secrets, in a public repository.**
This is the material cost and it is a design constraint, not a configuration
detail. GitHub states that *"with the exception of `GITHUB_TOKEN`, secrets are not
passed to the runner when a workflow is triggered from a forked repository"*
(GitHub Docs, "Using secrets in GitHub Actions",
<https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-secrets>,
read 2026-09-02). That is what stops a stranger's pull request from printing a
signing key. Therefore: **the signing and notarisation job runs only on tag
pushes in the base repository, never in the pull-request build path, and
ordinary CI must pass with no secrets at all** so that forks' pull requests are
not broken by their absence.

**That rule is not self-enforcing, and stating it as a property of the signing
workflow would be a mistake.** Repository secrets are readable by *any*
workflow in the repository that names them, under any trigger, once it is on
the default branch — so "signing runs only on tag pushes" describes where
signing happens and says nothing about where the key can be read. The
enforcement MUST therefore be structural: the signing credentials live in a
GitHub **Environment** protected by a deployment tag rule limited to `v*`, no
workflow other than the release workflow may reference that environment, and a
protected-tag ruleset keeps a non-owner from creating or moving a release tag.
`pull_request_target` and `workflow_run` are forbidden without a named ADR,
since both run in base-repository context.

**The credentials that must never reach a public log** are more than the
obvious one: the Developer ID private key and its import password, the
temporary keychain password, the notarisation credential — for which the App
Store Connect API key is preferred over an Apple ID plus app-specific password,
precisely because it keeps the owner's e-mail out of the job — and the Apple ID
itself, which is personal data and is not masked unless separately registered.
Every credential reaches its tool by file or stdin, never in `argv`, where `ps`
and any `set -x` expose it; log masking replaces exact registered values only
and is not a control to lean on; `ACTIONS_STEP_DEBUG` is forbidden on the
release workflow. `codesign -dvvv` output is the opposite case and should be
logged: it discloses the Common Name and Team ID, which every published
artifact carries anyway.

**Third-party actions are the release job's real dependency graph.** Any action
the signing job runs executes arbitrary code in the process holding the key, so
every `uses:` in a workflow that can reach the signing environment MUST be
pinned to a full 40-hex commit SHA, and the check belongs in
`scripts/validate.sh` rather than in a reviewer's discipline.

**Consequence accepted rather than deferred — provenance attestation becomes a
MUST.** Putting a signing key in the pipeline is what made the published
checksums insufficient as a trust anchor: they sit on the same release page as
the artifacts, so whatever can replace one can replace the other.
`08-distribution-and-project.md` promotes attestation from SHOULD to MUST in
this same change, because the risk and the mitigation belong to one decision.

**Negative — certificate lifetime becomes an operational dependency, and
expiry is the benign half.** Signing with a secure timestamp is what keeps
already-published signatures verifying after the certificate *expires*; it is
not optional here, and it is the reason a lapsed account does not retroactively
break past releases. **Revocation is the other half and the timestamp does not
answer it**: if the certificate is revoked, or Apple revokes the notarisation
tickets, previously published artifacts stop launching — on the machines of
precisely the users this chapter is written for, whose package manager is
already broken. This decision therefore owes an incident-response plan: who
revokes, how a compromised release is withdrawn, how users are told, and what
the next release looks like on a revoked identity. That plan is not in this
record and is a task, not a solved problem.

**Negative — the R-9 `strip`/`lipo` failure mode gets worse, not better.** With
ad-hoc signatures the damage was a missing property; with Developer ID it is an
invalid signature on a published artifact. CI must assert the signature on the
*final* artifact, after every post-link step, and must assert the authority is
Developer ID rather than merely that a signature exists.

**Finding — the `.pkg` install location is not a free choice.** The obvious
target, `/usr/local/bin`, **is Homebrew's own prefix on Intel**. A tool built to
detect and prevent accidental cross-provider interference must not install
itself into a provider's prefix, and the repository's own guards deny writes
there. Further, a `.pkg` writing to a system location requires an administrator
password, which sits badly beside a project whose proposition is that discovery,
analysis and planning need no elevation at all (INV-06 governs the running
process, not an installer, so this is a coherence problem rather than a
violation — but it is the kind of incoherence users notice). **The `.pkg` MUST install to a MacPortsBrew-owned location outside every
provider prefix — `/usr/local` and `/opt/homebrew` for Homebrew, `/opt/local`
for MacPorts — and MUST be a user-domain install requiring no administrator
password.** Both are MUSTs; an earlier draft made the second a SHOULD, which
would have let a later pass ship a system-domain `.pkg` and call it compliant.
The same exclusion binds the `.dmg`'s copy target and the `cp` line in the
documentation. Four consequences follow and are not optional. The installer
performs **no `PATH` edit and no shell-configuration write** — that is the
write class this project forbids elsewhere, and SE-036 reserves it for a single
managed block that is *proposed* rather than performed. The install directory is
user-owned, `0755`, not group-writable, and is not the state directory. Because
a user-writable directory ahead of system paths on `PATH` is verbatim what
`doctor` is required to report, `doctor` **must report MacPortsBrew's own
directory too** rather than special-case it — a special case in a shadowing
check is a hole in the shadowing check. That is a requirement, not a
description: `crates/adapters/src/system/path_audit.rs` today emits only
`Duplicate`, `Relative`, `Missing`, `WorldWritable` and `GroupWritable`, and a
user-owned `0755` directory matches none of them, so the finding kind this
paragraph depends on does not exist yet and is a task. And the `.pkg` does **not** create the `mpb` alias,
because ADR-0003 requires installation to detect an existing `mpb` and decline
to shadow it, and a non-interactive installer cannot make that decision.

**Expensive to change later.** Dropping Developer ID after publishing signed
releases invalidates trust users have already extended, whereas adding it
invalidated nothing. That asymmetry was the reason the earlier recommendation
said adding later was safe; it now applies in the other direction and should be
read as a commitment.

## Compatibility impact

- **macOS 10.12–10.14.4:** no notarisation regime (R-9). The signature is inert
  and harmless; nothing regresses for the oldest supported systems.
- **macOS 10.14.5 and 10.15+:** Developer ID required for quarantined
  artifacts. This is the range the decision pays for.
- **macOS 15 and later:** no Control-click override (R-9), so an unsigned
  artifact would have been at its worst here.
- **`arm64`:** `ld`'s ad-hoc signature is replaced by the Developer ID
  signature. The post-link invalidation hazard is unchanged in kind and larger
  in consequence.
- **`x86_64`:** unchanged, and buildable natively on the free
  `macos-15-intel` standard runner (R-10).
- **Deployment floor (ADR-0002) and licence (ADR-0001):** cleared for *crates*,
  and the scope of that clearance matters. This decision adds no crate, links no
  library and changes no toolchain; signing and notarisation are post-link steps,
  so nothing here can raise the `x86_64-apple-darwin` 10.12 / 11.0 floor and
  there is no new third-party licence to check against Apache-2.0. It does not
  follow that the decision adds no dependencies: the release job will consume
  third-party GitHub Actions, which ADR-0004's gate does not cover and which no
  existing check inspects. Those are governed by the SHA-pinning requirement
  above, and extending ADR-0004 to reach them is an open task. The premise that
  Apple's signing tooling is present on the CI image is **assumed, not
  verified** — R-11 finding 2.
- **Provider versions:** none. Nothing here invokes `brew` or `port`.

## Revisit when

- **R-11 finding 1 shows a stapled container does not cover an extracted
  binary.**
  Then the `.dmg` is buying nothing the bare binary does not already offer and
  should be dropped rather than carried.
- Apple changes what `notarytool` accepts or what can be stapled.
- The owner lets the Developer Program membership lapse — at which point the
  secure timestamp is what decides whether published releases keep verifying,
  and the next release reverts to option A rather than shipping unsigned.
