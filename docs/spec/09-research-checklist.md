# 09 — Research checklist

**Status:** Tier 3. Blocking items must be answered before the architecture is
fixed. Record every answer in `docs/research/<item-id>-<slug>.md`, following
the template in `docs/research/README.md`, with the source URL, the command
run, and the date.

**Rule:** every factual claim about a provider is (a) observed on this machine
with a read-only command, (b) cited from official documentation with a read
date, or (c) implemented as runtime detection. There is no fourth option, and
"I recall that…" is not one of the three.

---

## Pre-seeded findings (verify; do not trust blindly)

These were gathered on **2026-09-02** during the pre-implementation review.
Re-check each — some are time-sensitive by nature.

### F-1 · Homebrew support tiers (blocking)
*Answered 2026-09-02 — `docs/research/F-1-homebrew-support-tiers.md`.*
Source: <https://docs.brew.sh/Support-Tiers>, `last_review_date` 2026-08-27,
read 2026-09-02.

- Tier 1: Apple Silicon, recent Apple-supported macOS, default prefix, bottles.
- Tier 2: prereleases, outdated Command Line Tools, non-default prefixes.
- Tier 3 (*not supported*): macOS versions out of CI/security updates,
  non-default prefixes, **and Intel x86_64 systems**.
- Unsupported: macOS 10.14 and earlier; multi-user setups.
- Homebrew **states** it has stopped building new bottles for Intel, and states
  it **will remove the ability to run Homebrew on Intel systems in or after
  September 2027**. The first is Homebrew's stated policy, and is not literally
  true of the artifacts: Intel bottles were still published in August 2026
  (`htop` on the 16th, `sqlite` on the 25th, `cmake` on the 26th). The accurate
  characterisation on 2026-09-02 is **frozen at `sonoma` and eroding** — of
  8586 core formulae, 7808 (90.9%) have a usable bottle on Intel macOS 26
  against 8420 (98.1%) on Apple Silicon, and only 79 carry an Intel `tahoe`
  bottle (F-1 E4, census method in R-2).

**Impact:** this is the product's central fact. It belongs in policy *data*
with this source and date (INV-20), not in code — quoted as Homebrew's policy,
with the measured coverage as the separate, re-runnable observation. Re-verify
before every release: the date may move in either direction, and the census is
a snapshot of a deteriorating situation that must be re-measured, not quoted.

### F-2 · `/usr/local` interferes with MacPorts builds (blocking)
*Answered 2026-09-02 — `docs/research/F-2-usr-local-interference.md`.*
Source: MacPorts Guide §3.1.12,
<https://guide.macports.org/chunked/using.html#using.port.install>, read
2026-09-02. **Not** the FAQ: `trac.macports.org` served a bot-mitigation
interstitial to every URL form tried on 2026-09-02, so the FAQ's current text
is not recorded anywhere in this repository.

The hazard is confirmed from the Guide's trace-mode text: it names "conflicts
caused by files installed by other ports or in common system locations, such as
`/usr/local`" and prescribes `port -t` — a filesystem sandbox — as the remedy.
The narrower claim that **the GNU autotools and gcc probe `/usr/local`** is
plausible but **uncited**: the Guide does not say it, and the FAQ that may have
said it could not be read. Treat it as unverified until `trac.macports.org` is
readable again. On Intel Macs, `/usr/local` is exactly where Homebrew lives.

The other direction exists and Homebrew mitigates it **in its compiler shim,
not in `doctor`**: `shims/super/cc` drops any `-I` or `-L` flag whose path lies
under `/opt/local` (also `/sw`, `/opt/X11`), always on, for every source build. (Corrected 2026-10-12: the shim filters only when argument refurbishing is on, which macOS builds do not enable by default; see the F-2 addendum and SE-098.)
`brew doctor` has **no** MacPorts, Fink or `/opt/local` check at any version
examined — none at HEAD, none at 1.9.0, 2.7.0 or 3.6.0 — so MacPortsBrew's
`doctor` must implement that check itself and cannot delegate it.

**Impact:** this is the mechanism by which accidental cross-provider
contamination actually happens, and it motivates INV-10 and the `doctor`
environment checks. Note that MacPorts already sanitises its build environment
to an allow-list, so the residual exposure is a filesystem search path rather
than an environment variable — which is why the remedy is trace mode.

### F-3 · Toolchain minimum macOS versions (blocking — affects the whole product)
Sources: <https://go.dev/wiki/MinimumRequirements> and
<https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html>, read 2026-09-02.

- Go 1.25 and 1.26 require **macOS 12+**; Go 1.27 and later require **macOS 13+**.
- Rust `x86_64-apple-darwin` supports **macOS 10.12+**; `aarch64-apple-darwin`
  supports macOS 11+.

**Impact — read this carefully.** The users this project exists to serve are on
Intel Macs that Apple and Homebrew have moved past. Many of those machines top
out below macOS 12. A current Go toolchain cannot produce a binary that runs on
them; Rust can. If Go is chosen, the product excludes part of its own target
audience — that may still be the right trade, but it must be a *stated,
justified* trade in the ADR, not an accident.

**RESOLVED 2026-09-02.** The owner set the floor at macOS 10.12 (Intel) /
11.0 (Apple Silicon), which excludes current Go toolchains and points to Rust.
See ADR-0002. What remains is verification, not decision — see that ADR's
"must still verify" list.

**Required of the language ADR:** state the minimum macOS version MacPortsBrew
will support, name the machines that implies, and show that the chosen toolchain
can actually build and run there. Verify the current numbers — they change with
every toolchain release.

---

## Blocking research

### R-1 · Homebrew read interfaces
*Answered 2026-09-02 — `docs/research/R-1-homebrew-read-interfaces.md`*, with
two sub-items UNRESOLVED (`brew tap-info --json`, blocked by this repository's
own hook; and whether "no bottle anywhere" is separable offline). Its first
finding is a standing warning: `brew info <unknown-name>` taps `homebrew/core`
(a 1.4 GB clone into the prefix) before reporting the miss, and the quieting
variables do not prevent it — never pass an unknown name to `brew info`.

Which commands give machine-readable, stable data for: installed packages;
user-requested (top-level) packages; dependencies; reverse dependencies;
formula and cask metadata; deprecation and disabled state; pinned state; taps;
prefix and configuration. Which produce JSON, and how stable is that JSON across
versions. Which commands trigger a network fetch or an auto-update as a side
effect, and how to suppress that. Confirm cask semantics and why casks are
outside migration scope.

### R-2 · Homebrew artifact availability
*Answered 2026-09-02 — `docs/research/R-2-homebrew-artifact-availability.md`.*
The reliable way to determine, without installing, whether *this* package on
*this* machine will use a prebuilt bottle or compile from source — including
when no bottle exists for the platform at all. Distinguish "no bottle
published", "bottle exists but not for this platform", "source build forced by
configuration". Determine what is answerable offline.

### R-3 · MacPorts read interfaces
*Answered 2026-09-02 — `docs/research/R-3-macports-read-interfaces.md`*, from
documentation and source; the parser-level items need a machine with MacPorts.
The equivalents: installed; **requested** (authoritative user-intent record);
dependencies; dependents; variants and their effect; inactive versions;
port metadata; deprecated/replaced ports; prefix and configuration.
Which output is stable enough to parse and what the parsing failure mode is.

### R-4 · MacPorts binary archive availability
*Answered 2026-09-02 — `docs/research/R-4-macports-binary-archives.md`.*
When does MacPorts install a prebuilt archive versus building locally.
Confirm the effect of non-default variants and of licence restrictions on
binary availability. This is what makes the availability model symmetric
(`02-domain-model.md`) rather than a Homebrew-only "bottle" concept.

### R-5 · Privilege requirements
*Answered 2026-09-02 — `docs/research/R-5-privilege-requirements.md`*; the
MacPorts half was cited, not observed, and **was re-checked on a MacPorts
machine on 2026-09-08** (E19-E32), which this line asked for and which the note
itself flagged as outstanding until then.
Exactly which operations of each provider require elevation, on each
architecture and prefix. Confirm that everything the foundation pass does
requires none.

### R-6 · Primary-provider mechanisms
*Answered 2026-09-02 — `docs/research/R-6-primary-provider-mechanisms.md`.*
MacPorts' own selection mechanism, Homebrew's linking behaviour, versioned
executable naming, and what each actually guarantees. Compare against a
MacPortsBrew-owned shim directory using the criteria in
`05-environment-and-primary-provider.md`. Recommend one; justify it.

### R-7 · Implementation language — **RESOLVED 2026-09-02: Rust**
Decided by the project owner; ADR-0002 is *accepted* and holds the
constraints, the evaluation and the list of what the foundation pass must
still verify. What remains here is **verification, not decision**: each item
on that list, mechanically checked, before the bootstrap
(`10-first-pass-work-order.md`, step 2). In particular, the TUI and terminal
crates are checked for licence compatibility, deployment floor and East Asian
double-width handling together — discovering any of the three after the UI is
written means rewriting it.

### R-8 · Stateful and service packages
*Answered 2026-09-02 — `docs/research/R-8-stateful-and-service-packages.md`.*
How each provider manages background services and where common stateful
packages keep their data. Enough to *detect* the classes in
`02-domain-model.md` and refuse to migrate them (INV-19) — not enough to migrate
them, which is a later pass.

### R-9 · Gatekeeper and release distribution
*Answered 2026-09-02 — `docs/research/R-9-gatekeeper-and-distribution.md`*.
Its UNRESOLVED experiment and its ad-hoc-signing recommendation are both
**superseded by ADR-0005** (2026-09-02): the experiment now documents the path
not taken rather than gating a release.
What happens to a downloaded, unsigned CLI binary on current macOS. Options and
costs for signing/notarisation versus ad-hoc signing with documented
instructions. Feeds `docs/decisions-pending.md`.

### R-10 · CI runners
*Answered 2026-09-02 — `docs/research/R-10-ci-runners.md`.*
Current availability of hosted Intel macOS runners on GitHub Actions, and the
fallback if there are none.

### R-11 · Apple signing and notarisation toolchain
*Partially answered 2026-09-06 — `docs/research/R-11-apple-signing-toolchain.md`.
Three of the five closed from official documentation; two remain open and need
something other than reading. Still blocks the implementation pass that writes
the release workflow, not ADR-0005, which is decided.*

**One premise below is wrong and is left in place so the correction is
visible.** `macos-15-intel` is not a free standard runner: GitHub's
`runner-images` README lists it as an alias of `macos-15-large`, and GitHub Docs
says larger runners "are always charged for, even when used by public
repositories". Bare `macos-15` is the free arm64 image. An Intel release job
costs money on a public repository, which is a cost DP-007 did not weigh.

**Answered:** a standalone binary **cannot be stapled at all** ("Although
tickets are created for standalone binaries, it's not currently possible to
staple tickets to them"), so the `.dmg` is the only staplable container that
delivers a single binary and is the *only* offline-verifiable path — the
opposite of what the question below supposes. And the `.pkg` is signed by a
**Developer ID Installer** identity, a different certificate from the Developer
ID Application identity that signs the Mach-O and the `.dmg`, so shipping all
three needs two.

**Still open:** whether the hardened runtime wants an entitlement for a tool
that spawns `brew` and `port` (needs one notarisation attempt), and what a
quarantined, notarised, unstapled binary does offline (needs a machine with the
quarantine attribute set and the network off).

The original five, as filed:
Five things the signing decision assumes and this repository has not verified:
**whether stapling a `.pkg` or `.dmg` covers a binary copied out of it or only
the container** — if only the container, the `.dmg` buys nothing the bare binary
does not and is dropped; whether `codesign`, `notarytool` and `stapler` are
present on the free `macos-15-intel` standard runner image and at what Xcode
floor, which R-10 did not establish; which certificate signs the `.pkg` —
R-9 C-1 lists an "installer certificate" among the acceptable kinds without
saying it signs the `.pkg`, so this was wrongly treated as answered; whether the
hardened runtime requires any entitlement for a tool that spawns `brew` and
`port` (expected: none, worth confirming rather than discovering at
notarisation time); and what a quarantined, notarised,
**unstapled** binary does on a machine with no network, which is the state of
exactly the user this project is written for.

Already answered by R-9 (2026-09-02) and **not** open, despite an earlier draft
of ADR-0005 listing them: that a Developer ID **Application** certificate signs
Mach-O files, disk images and command-line tools (C-3) — the `.pkg` is not in
that list and stays open above — whether the hardened runtime is required for a command-line
tool (C-1, C-3, C-4), and what container `notarytool` accepts for a standalone
binary (C-2).

### R-12 · Cask migratability
*Answered 2026-09-04 — `docs/research/R-12-cask-migratability.md`.*
Whether a Homebrew cask has a MacPorts destination at all, and whether a cask
can consume MacPorts components in place of the Homebrew formulae it declares.
Answers: sometimes a destination exists and it is never the same operation, so
INV-01's verification cannot mean for a cask what it means for a formula; and
0.32% of casks declare a formula dependency, which a MacPorts equivalent on
`PATH` can satisfy and the next `brew upgrade --cask` un-does. Raised by DP-012.

Found on the way, and larger than the question: Homebrew records
`installed_on_request` for casks in the on-disk `INSTALL_RECEIPT.json`, which
falsifies the absolute six places in this repository stated (F-28) and gives
DP-012 a third option the charter may forbid.

### R-13 · What revocation actually does to a published artifact
*Open. Raised 2026-09-04 by DP-009. Blocks nothing; it decides how much of
`SECURITY.md`'s withdrawal plan is load-bearing.*
ADR-0005 asserts that a revoked Developer ID certificate, or a revoked
notarisation ticket, stops **previously published** artifacts from launching,
and that the secure timestamp does not answer revocation the way it answers
expiry. Nothing in this tree cites that. Checked on 2026-09-04 and found not to
say it: Apple's Gatekeeper page
(<https://support.apple.com/guide/security/gatekeeper-and-runtime-protection-sec5599b66df/web>)
contains no occurrence of "revoke" or "revocation"; the notarisation page
(<https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>)
is JavaScript-rendered and yielded no text.

**Read rendered on 2026-09-06, and the page does say something.** The
JavaScript-rendering obstacle above is a tooling limit, not a fact about Apple:
the page reads fine in a browser, and anyone continuing R-13 or R-11 should
render rather than fetch. What it says, in full, on revocation:

> Notarization also protects your users if your Developer ID signing key is
> exposed. The notary service maintains an audit trail of the software
> distributed using your signing key. **If you discover unauthorized versions of
> your software, you can work with Apple to revoke the tickets associated with
> those versions.**

That is the first citation in this tree for ticket revocation existing at all,
and it establishes two things: revocation is of **tickets**, not only of
certificates, and it is **Apple-mediated** — "work with Apple" is not a button
the owner presses, so any runbook step that assumes same-day self-service
revocation is wrong about the mechanism.

Also relevant, from the same page: "When the user **first** installs or runs
your macOS software, the presence of a ticket (either online or attached to the
executable) tells Gatekeeper that Apple notarized the software." That is
suggestive of a first-launch evaluation but is prose about the happy path, not a
statement about re-evaluation, and it must not be read as one.

**Everything behavioural in the paragraph below remains open**, and none of it
looks documented: whether a stapled ticket keeps an artifact launching after
revocation, whether that differs offline, whether an already-launched binary is
re-evaluated, and the bare-Mach-O-from-a-shell case. These need observation, and
observing them needs a real certificate and a real revocation — which is to say
R-13's behavioural half may simply not be answerable before the first release.

What an answer needs: whether Gatekeeper consults revocation state for a
Developer ID signature and when — first launch only, or every launch; whether a
**stapled** ticket keeps an artifact launching after the ticket is revoked, and
whether it differs offline; whether an already-launched-once binary is
re-evaluated; and whether any of it differs for a bare Mach-O run from a shell,
which R-9 already found Apple does not document. The distinction that matters
for the plan is *published-and-already-downloaded* versus *downloaded after
revocation* — only the first makes the outbound channel urgent.

### R-14 · What actually reaches a user of a published binary
*Partially answered 2026-09-06 — `docs/research/R-14-what-reaches-a-user.md`.
Two of four closed; two need a remote rather than a reading. Raised 2026-09-04
by the security review of DP-009's plan.*

**The withdrawn claim was nearer right than its withdrawal.** A published
repository advisory *is* added to the GitHub Advisory Database — "GitHub will
review each published security advisory, add it to the GitHub Advisory
Database" — unconditionally, subject to review. The ecosystem-package condition
the review reasoned from applies to **forks** and to **Dependabot alerts**, not
to inclusion. The reach is still near zero, but for a different reason: nothing
can declare a dependency on a binary shipped through Releases, so there is
nothing for Dependabot to alert. An advisory is a public record, not a push
channel, and `SECURITY.md` should say that rather than either the original claim
or its blanket withdrawal.

**And a feature the plan was written without: immutable releases.** Assets
"cannot be modified or deleted", and deleting the release permits deleting the
tag but "you cannot reuse the same tag name" — not even in a same-named
successor repository. Withdrawing one bad artifact therefore means deleting the
whole release and permanently burning that version number. Attestations are
generated automatically. Whether the attestation survives deletion — which the
runbook's step 2 leans on — is still uncited. This is an owner decision, not a
release-workflow detail.

**Still open:** what a "Watch → Releases only" subscriber actually receives and
when, and whether `releases.atom` includes deleted releases. Both are minutes of
observation once a remote exists, and neither should be guessed at first.

The original four, as filed:
The withdrawal plan originally claimed a repository security advisory
"syndicates to the GitHub Advisory Database" and was therefore "the only
outbound channel with reach". That is uncited, and doubtful in this project's
shape: the global database is curated around packages in supported ecosystems,
and MacPortsBrew publishes a signed binary through Releases and no ecosystem
package. The claim is withdrawn until answered.

What an answer needs, each with a source and a read date: whether a repository
advisory for a project with no ecosystem package appears in the global database
at all; what a "Watch → Releases only" subscriber actually receives, and when;
whether `releases.atom` includes deleted or de-listed releases; and whether
deleting a release leaves the tag, the `SHA256SUMS` asset and the build
provenance attestations in place — the plan's step 2 depends on the last of
these and states it without a citation.

### R-15 · Whether a `brew.env` can undo the quieting environment
*Answered 2026-10-02 — `docs/research/R-15-homebrew-brew-env.md`.* Raised by
SE-092's end-of-pass amendment; blocks the mutating half of the executor
relying on that environment. Answers: yes — `brew` exports the `HOMEBREW_*`
lines of up to three `brew.env` files after the environment it is given, an
empty value deletes a key, and nothing switches the files off (Homebrew 7.0.4).
Also found: `HOMEBREW_NO_AUTO_UPDATE` does not keep a read offline when the API
cache is missing. What to do about it is DP-027. Left open: whether a
substituted API domain or curl configuration can get altered API data
accepted, and what a Ruby read does on a cold cache with no network at all
(it bears on "`analyze` works with no network").

### R-16 · Whether a MacPorts user configuration can outrank the invocation
*Answered 2026-10-03 — `docs/research/R-16-macports-user-configuration.md`.*
Raised 2026-10-02 by the review of R-15 (DP-027). Answers: yes, per key —
`mportinit` overlays the system `macports.conf`, then
`~/.macports/macports.conf` (read under the `HOME` the executor passes, only
if `~/.macports` is a directory), then `$PORTSRC`, last one wins; `user.conf`
is opened but accepts no key, because `user_options` is empty (MacPorts
2.12.6). No file can undo `-q`/`-N`, which are argv-only. A user file can
redirect what a read reports (`portdbpath`, `sources_conf`, `prefix`) and
steers nearly every part of an install (`binpath`, `pubkeys_conf`,
`sources_conf`, `buildfromsource`, …). No read was found to reach the network or
evaluate a Portfile. Pointing `HOME` at an empty directory suppresses the
user file at almost no cost, because `port` replaces `HOME` itself. The
system files are root-owned. What to do about it is DP-027. Left open: which
user's file an elevated `port` reads (depends on the sudo policy), and the
network finding was established from source, not from a traced run.

### R-17 · Where each provider holds a lock while it mutates, and whether a filesystem read can see it
*Answered 2026-10-05 — `docs/research/R-17-provider-mutation-locks.md`.*
Answers: both providers use held advisory `flock`s whose files are left behind
after release, so a file's presence proves nothing and a "stale lock" does not
exist (Homebrew 7.0.4 `var/homebrew/locks/<name>.<type>.lock`, `update`,
`vendor-install-<name>`; MacPorts 2.12.6 `<portdbpath>/registry/.registry.lock`,
taken only by a registry-writable `port`). Casks, Homebrew cleanup, and MacPorts
`sync`/`selfupdate`/`clean` take none. A filesystem-only detector can say "a
lock is held right now" (positive evidence) through `fcntl(F_GETLK)` on an
`O_RDONLY` fd, observed to see `flock` locks on macOS 26.7.1 without acquiring,
creating or altering anything; it cannot say "idle". The `flock(LOCK_SH|LOCK_NB)`
probe is unsafe against Homebrew, whose `EX|NB` taker aborts. Left open: no real
provider mutation was observed holding a lock; other macOS versions and network
volumes; the official documentation.

### R-18 · Whether advisory file locks are available on the pinned toolchain
*Answered 2026-10-05 — `docs/research/R-18-advisory-file-locks-on-the-pinned-toolchain.md`.*
Answers: yes — `File::lock`, `lock_shared`, `try_lock`, `try_lock_shared` and
`unlock` are stable since 1.89.0 and the pinned 1.98.0 compiles them;
`TryLockError` is `Error(io::Error) | WouldBlock`. On Apple targets std calls
`flock(2)`; contention is per open file description (two opens in one process
conflict), the lock is released when its holder dies (observed with SIGKILL),
and a lock file must never be unlinked (a recreated path is a second inode that
locks independently). Left open: behaviour on NFS, SMB and exFAT volumes
(`man flock` is silent and this machine has none), and whether std opens with
close-on-exec, which decides whether a spawned child can keep a lock alive.

### R-19 · What the release workflow can rely on: notarisation, reproducible builds, deployment target, packaging, runners
*Partially answered 2026-10-10 — `docs/research/R-19-release-toolchain.md`.*
Continues R-11; blocks the implementation pass that writes the release workflow.
Answers:
- **R-11 Q4 is closed.** The hardened runtime documents no exception for
  spawning a child, and an ad-hoc `runtime`-flagged binary spawned one normally.
- **R-11's "`macos-15-intel` is a paid larger runner" is wrong.** GitHub Docs
  lists `macos-15-intel` and `macos-26-intel` as free standard runners, and
  `-large` is the billed label of the same image.
- **Notarising a CLI** needs the hardened runtime, a secure timestamp
  (`timestamp.apple.com`, so network) and no `get-task-allow`. A bare Mach-O
  can be notarised as a ZIP but never stapled; only a UDIF `.dmg` or a signed
  flat `.pkg` can be.
- **Reproducibility.** Two release builds of one commit are byte-identical on
  one machine with the current profile (x86_64 and arm64, same `LC_UUID` and
  CDHash). Across machines, the Cargo registry paths must be remapped and one
  Xcode pinned.
- **The achievable "byte-identical" claim is payload identity within one
  build**: the `.pkg` and `.dmg` payloads hashed to the bare binary. Signed
  binaries and containers differ across rebuilds.
- **Deployment target.** The 1.98.0 defaults record 10.12 (x86_64) and 11.0
  (arm64), but no hosted Xcode's stated range includes 10.12.
- **Packaging.** Extract with documented `pkgutil --expand` plus `tar`. Use
  `-fs HFS+` for the x86_64 `.dmg`. GitHub's keychain recipe puts passwords in
  argv, against ADR-0005.

Left open:
- R-11 Q5 (offline, quarantined, unstapled CLI).
- Whether the nested ticket covers the bare binary.
- Developer ID CDHash stability.
- How a home-domain `.pkg` places `--install-location`.
- Feeding `security` passwords by stdin.

### R-20 · Which read-only provider read proves that a package is the active (linked) installation
*Answered 2026-10-18 — `docs/research/R-20-what-a-read-shows-as-active.md`.*
This is the prerequisite of pass 08 slice 6 part B, and the
`provider-researcher` note SE-094 owed for `PROVIDER_REPORTS_ACTIVE`.
Observed on macOS 26.7.1 x86_64, Homebrew 7.0.4, MacPorts 2.12.6.

Answers:
- **MacPorts.** `port -q installed`, bare or named, prints
  `  <name> @<version>_<revision><variants> (active)\n` for the active image.
  An inactive image prints the same row with no suffix, because 2.12.6 has
  no `(inactive)` marker (`port.tcl` `action_installed`).
- **What `-q` changes.** It drops only the header and the stale-ports
  warning, and the rows are byte-identical. `-v` would append text after
  `(active)`.
- **Named reads.** A named read matches exactly and ignores case
  (`xz` does not list `xz-bootstrap`; `XZ` prints `xz`). It exits 0 with
  empty stdout when nothing matches.
- **Homebrew formulae.** `linked_keg` in `brew info --json=v2 --installed`
  is the version of the keg `var/homebrew/linked/<name>` points to, or
  `null`. On 192 formulae every string equals an `installed[].version`.
- **`keg_only` does not predict `linked_keg`.** `openssl@3` is keg-only and
  linked; `openssl@4` and `pydantic` are not keg-only and are unlinked.
- **Casks.** They carry no linked or active field.
- **The current verify read.** `brew list --versions` shows no linkage.
- **Writes and `brew.env`.** No `brew.env` exists on this machine. On a warm
  API cache both Homebrew reads wrote only bootsnap compile-cache files.

Left open:
- The state right after a fresh install.
- An observed inactive or variant MacPorts row: the only ones are
  synthesised fixtures.
- Whether a configuration can turn on verbose for a `-q` read.
- Whether `linked_keg` can disagree with the `bin/` symlinks.
- Network traffic: not captured.

---

## Non-blocking research

- Homebrew and MacPorts release cadence and how often their output formats change.
- Existing tools in this space and what they get right or wrong.
- Runtime version managers worth naming, and their current standing.
- GitHub Wiki publishing workflows from a repository directory. *Partially
  answered 2026-09-02 (R-10, final section): a wiki is a Git repository at
  `<repo>.wiki.git`, filenames become page titles and nesting does not survive,
  so a wiki is a generated mirror or nothing. Whether `GITHUB_TOKEN` may push to
  it is UNRESOLVED — there is no `wiki` permission scope.*
- GitHub topic conventions. *Answered 2026-09-02 (R-10, final section): up to 20
  topics, lowercase letters, digits and hyphens only, 50 characters each.*
- Reasonable minimum macOS for MacPorts itself, to bound the support matrix.
  *Answered 2026-09-02 (R-4): MacPorts 2.12.6 ships package installers down to
  macOS 10.12 Sierra, coinciding with this project's Intel floor.*
