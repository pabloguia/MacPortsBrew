# Security

MacPortsBrew orchestrates package managers, which is privileged infrastructure
work even when MacPortsBrew itself never elevates. Security is therefore a
design property here, not a policy document.

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

There is no security contact address yet, because the remote does not exist
yet. The repository *will* be published publicly on GitHub
([DP-007](docs/decisions-pending.md), decided 2026-09-02), and the route is
decided with it: a **private GitHub security advisory** — the *Security* tab,
*Report a vulnerability* — private to the maintainers until published.

**Private vulnerability reporting must be enabled before the repository is made
public, not after.** A public repository that ships Developer ID-signed binaries
([ADR-0005](docs/adr/0005-release-signing-and-artifacts.md)) with no private
channel forces the first report of a signing-pipeline flaw onto a public
tracker. This file names that channel as the only one the moment it exists.

Until then, if you have found something and are reading this, raise it with the
person who gave you the repository, privately, and describe: what you did, what
happened, what you expected, and the machine you saw it on. `macportsbrew
doctor --report` collects the machine facts without home paths, user names or
environment values (tested section by section) — read it before you send it.

Please do not include credentials, tokens or anything from outside the two
providers' scope in a report. MacPortsBrew never needs them and cannot use
them.

## What is in scope

Anything that could cause MacPortsBrew to damage a working machine, to remove
something before its replacement is verified, to run a command that was not in
a reviewed plan, to elevate where it said it would not, or to mis-report what
is installed. A silent misparse of a package list is a data-loss-class bug and
is treated as a security problem, not a cosmetic one.

Also in scope: anything in the release pipeline that would let a user run a
binary that is not the one the repository built.

## What is out of scope

Vulnerabilities in Homebrew or MacPorts themselves — report those to those
projects. MacPortsBrew is unaffiliated with either and interacts with both only
through their public command-line interfaces.

## Threat model summary

The full model, with the mitigations and the reasoning, is in
[`docs/spec/04-migration-and-safety.md`](docs/spec/04-migration-and-safety.md)
("Threat model"). In short:

| Threat | How it is addressed |
|---|---|
| **Command injection** | Structurally impossible: argv arrays only, no shell, no string interpolation. Package names and paths are arguments, never fragments of a command line. `scripts/check-layering.sh` asserts that `std::process::Command` is constructed only inside `crates/adapters/src/process/`. |
| **Executable shadowing and `PATH` spoofing** | Provider executables are resolved from their detected prefix, not from `PATH`, wherever correctness matters. `doctor` reports writable directories that precede system paths, and duplicate `PATH` entries. |
| **Symlink attacks and unsafe temporary files** | Temporary files are created with restrictive permissions in a private directory; writes never follow a symlink; writes are atomic. |
| **Malicious or malformed provider metadata** | All provider output is untrusted input: sizes are bounded, shapes are validated, nothing is evaluated, and metadata never becomes a path or a flag. A parser that meets output it does not recognise fails with `ErrorId::ProviderOutputChanged` rather than mis-parsing. |
| **Privilege boundaries** | Elevation is per-operation, enumerated, visible, never cached and never inherited by an unrelated step. Discovery, inventory, mapping, analysis, planning and dry-run are unprivileged by design (INV-06), and this build has no code path that elevates at all. |
| **State and configuration integrity** | MacPortsBrew's own state directory is user-owned with restrictive permissions. Corrupt state fails closed — no plan is produced — rather than producing a plan from a damaged record. |
| **Supply chain** | Dependencies are pinned and their licences checked mechanically. Release artifacts carry checksums, and build provenance is a stated goal. |

## If a release must be withdrawn

Everything above is the **inbound** half of this file: how a problem reaches the
maintainers. This is the outbound half — what happens when a published release
has to be pulled back, and how anyone finds out. It exists because
[ADR-0005](docs/adr/0005-release-signing-and-artifacts.md) records revocation as
the one failure this project's release design does not absorb, and because an
incident is a bad time to start writing an incident plan.

**The premise, and its status.** ADR-0005 states that a revoked certificate — or
a revoked notarisation ticket — stops previously published artifacts from
launching, and that the secure timestamp, which does answer certificate
*expiry*, does not answer revocation. That is the working assumption here and it
is **not verified**. Neither Apple page this project cites mentions revocation at
all: <https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web>
was re-read on 2026-09-04 and contains no occurrence of the word, and
<https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>
renders through JavaScript and returned no text to a fetch. R-13 owns closing
it. The plan below is written to be cheap if the premise turns out to be false
and sufficient if it turns out to be true, which is the only shape worth writing
before the answer is in.

### Revoking is not the first move

Revocation is itself destructive: it invalidates every artifact ever signed with
that identity, including all the good ones. Under stress the reflex is to
revoke, and usually a new release is the right answer instead.

**Revoke if the identity may have been *used* by anyone else, or if the key may
have been read.** Using it is the wider and the likelier case: the key lives in a
CI environment (ADR-0005), and a malicious third-party action, a moved `v*` tag
or a stolen notarisation credential lets an attacker sign and notarise without
ever seeing the key. An artifact signed that way is valid, notarised, and
indistinguishable from a real one on every user's machine until the identity is
revoked.

| What happened | What it needs |
|---|---|
| A bug in a published release, even a data-loss one | A new release and an advisory. Not revocation. |
| A vulnerability in MacPortsBrew's own code | A new release and an advisory. Not revocation. |
| A release built from a commit that should not have shipped | Withdraw the release. Not revocation. |
| The signing key, or the environment holding it, may have been read by anyone else | **Revoke**, then everything below. |
| The release environment or the notarisation credential may have been **used** by anyone else | **Revoke**, then everything below. The key never left; that does not help. |
| Apple revokes it without being asked | Everything below, minus the revocation step. |

### Who

The Account Holder of the Apple Developer team — one person, the project owner
(DP-006). Apple requires it: *"You must be the Account Holder of your
development team in the Apple Developer Program in order to generate a Developer
ID certificate"* (<https://developer.apple.com/developer-id/>, read 2026-09-02;
`docs/research/R-9-gatekeeper-and-distribution.md` C-9). There is no second
person, and this file says so rather than implying a rota that does not exist.
Anyone else who finds evidence of key compromise reports it through the private
channel above and cannot act on it.

### What has to be recorded before the first release

Two steps below are impossible to perform after the fact unless something was
written down before. Both are release-pipeline requirements, not habits:

1. **Which certificate signed which release.** The blast radius of a revocation
   is "every artifact signed with this identity", and answering that a year
   later means knowing, per release, the signing identity's team identifier and
   certificate fingerprint. CI already asserts Developer ID authority on the
   final artifact ([`docs/spec/08-distribution-and-project.md`](docs/spec/08-distribution-and-project.md));
   recording *what it saw* — in the release notes and in the build provenance
   attestation — costs one line and cannot be reconstructed later.
2. **Where the key lives and who can reach it.** In the protected deployment
   environment named by ADR-0005, not in anyone's memory.

Private vulnerability reporting must already be enabled, because it is a
publication prerequisite for the inbound channel (DP-007) and step 3 below uses
the same mechanism.

### The runbook

**Contain before you revoke.** Revocation is global and irreversible and it
breaks the *good* releases too — the ones a user would otherwise fall back to.
Doing it first leaves every working install broken, the bad artifact still
downloadable, and no replacement yet. The one exception is an identity being
actively used to sign artifacts in the wild: then revoke first, immediately, and
come back to step 1.

1. **Write down the digests before anything is deleted.** Every withdrawn
   artifact's SHA-256, copied out of `SHA256SUMS`. This is the step whose cost is
   invisible and whose omission is unrecoverable: deleting a release deletes its
   assets *and* that file, the build provenance attestations are looked up by
   subject digest, and without the digests no user can be told whether the copy
   on their disk is the bad one.
2. **Withdraw the artifacts.** Delete the GitHub release, or mark it a
   pre-release with the reason in its body. Leave the tag and the attestations:
   they record what was built from which commit. Say plainly what that is worth
   — an attestation distinguishes a compromised artifact from a legitimate one
   only if CI itself was sound, which is exactly what the second and third rows
   of the trigger table put in doubt.
3. **Publish a GitHub Security Advisory**, from the same Security tab that
   receives reports, carrying the digests from step 1. Whether a repository
   advisory for a project that publishes no packaged ecosystem artifact reaches
   the GitHub Advisory Database is **not established here** (R-14); write it as
   though its readers are the people already watching this repository, because
   those are the ones it demonstrably reaches.
4. **Put the symptom in the advisory's title and first line, in the words the
   user will use.** This is the step that decides whether the plan reaches
   anyone, and
   the one an incident makes easiest to skip. A user hit by this does not
   experience a revocation. They experience macOS refusing to open something,
   with a dialog that names no project and explains nothing, and they paste that
   dialog into a search engine. An advisory titled in this project's internal
   vocabulary is invisible to that search.
5. **Say what to do**, in the same advisory: whether to delete the artifact,
   whether a fixed release exists, and if it does not, that one is coming and
   how they will know.
6. **Record it** in `CHANGELOG.md` and in the next release's notes, so the
   history does not live only in an advisory a reader must already know to look
   for.
7. **Revoke**, if the trigger is met and it was not already done. Last, because
   it is the step that cannot be taken back.

### How a user actually finds out, honestly

**Nothing in this design tells them.** Two charter non-goals bind it
([`docs/spec/00-charter.md`](docs/spec/00-charter.md), "Non-goals"): MacPortsBrew
does not "send telemetry", and does not "require Homebrew, MacPorts, network
access, or an LLM in order to run". Neither is being traded away for a
notification channel. Having no auto-update is *not* a charter non-goal — it is
a design choice, recorded here as one; an opt-in, off-by-default version check
would violate neither sentence on its face and would still owe its own argument,
because it reports an address and a version to somebody.

So every channel above is *pull*: the advisory, the repository, the releases
page, the project site, and whatever GitHub offers a watcher. Someone who
downloaded a binary six months ago and never came back learns nothing until it
stops working.

That is the honest state of it, and it is why step 4 is worth more than the rest
of the runbook combined.

### What this plan does not cover

Named so a maintainer knows what they are improvising rather than discovering
it mid-incident. None of these is filled here, and each is a smaller piece of
work than it looks:

- **A compromised dependency** rather than a compromised key — an advisory
  against a crate already inside a shipped binary. The dependency gate is a
  routine check, not a runbook.
- **Maintainer account takeover on GitHub.** Everything above assumes GitHub is
  trustworthy and the account is the owner's. An attacker holding it can
  publish releases, publish advisories, move tags and edit this file. Nothing
  here detects that or routes around it.
- **The owner being unreachable.** Revocation has a bus factor of one, stated
  plainly under "Who", and there is no dead-man path.
- **A non-signing secret alone** — a notarisation credential with no
  certificate access, for instance.
- **The links that break afterwards.** Withdrawing a release 404s every install
  command in the README, the wiki and anything a reader copied.
- **Forensic retention.** Actions logs expire and deleted release assets do not
  come back; nothing says what to keep or for how long.
- **Disclosure timing.** The inbound half of this file has no embargo policy, no
  response window, and no guidance on requesting a CVE or GHSA identifier.

### The next release, on a revoked identity

On a fresh certificate from the same team, sign, notarise and publish as usual.
Nothing self-heals: every user re-downloads, and the advisory says so plainly.

If the identity is gone rather than reissued — the account lapsed, or Apple
withdrew it — the next release reverts to **option A** of ADR-0005: a bare
binary, ad-hoc signed, carrying the quarantine documentation this project
already has because that was the reversible default before DP-004. It does not
ship unsigned and unexplained. That path costs the Gatekeeper story and keeps
the tool installable, which is the right way round for a tool whose users'
package manager is already broken.

## The safety invariants a security reviewer should read first

They are in [`docs/spec/01-invariants.md`](docs/spec/01-invariants.md), which
is the only copy. The load-bearing ones for security are INV-01 (destination
before source), INV-02 (every destructive action comes from a reviewed plan),
INV-03 (failure preserves the prior working state), INV-06 (least privilege)
and INV-19 (stateful packages are never migrated blind). Each has at least one
test named after it, findable with
`grep -rn "fn invariant_" crates/*/tests crates/*/src`.

## What this build can do

Version 0.1.0 is read-only. It inventories, analyses, maps, explains and plans;
it executes nothing, elevates nothing, writes nothing into either provider's
prefix, and makes no network call of its own — Homebrew, asked to read, may
fetch its catalogue on an empty cache, and a `brew.env` can switch its analytics
and network back on (DP-027, R-15). A gate script greps the source on every
change for a mutating provider subcommand literal or an elevation token and
fails the build if it finds one. That materially reduces the current attack
surface, and it will grow when the executor lands.
