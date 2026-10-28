# Decisions pending a human

Decisions an agent must not make alone, because they trade away something the
project owner may not want traded. Each carries a recommendation and the
reversible default taken in the meantime, so work is never blocked.

Resolved decisions move to the bottom of this file with a pointer to where they
are recorded.

---

## Open

### DP-054 — Relax DP-032 and DP-016 for `--replay`, so a replayed Homebrew plan can go ahead while the keg cannot be read?

Raised 2026-10-28, work order 16 part D; narrowed the same day after
`spec-guardian`'s review. Plan: `docs/passes/16-plans/D.md`. Evidence: R-8,
"Addendum 2026-10-28".

- **What is already ruled, and is being implemented.** DP-032: a package is a
  service if it is declared or materialised, and unknown refuses the plan.
  DP-016 (c) then (d): refuse where the source cannot answer, then record service
  status in the recording. Work order 16 D.5: under `--replay` the reader answers
  from the recording or as unknown. Part D implements that: under `--replay` the
  disk-backed halves are unknown, so a replayed plan whose source row does not
  declare a service is refused, **for both providers**. Plans are demonstrated
  from a live snapshot instead (`--from-snapshot`), the precedent DP-016 (c) set
  for MacPorts; the walkthrough, samples and wiki move to it. DP-016 (d) is the
  later addition that lets replay answer.
- **Why the materialised half matters for Homebrew.** The keg plist is written
  at install from the formula as it was then; `brew info --json=v2` reports the
  formula as it is now. A formula that later dropped its `service` block keeps the
  plist (R-8 addendum, A-5).
- **The open question.** Whether, under `--replay` only, a Homebrew formula whose
  current declaration is `null` should count as not a service, with a named plan
  caveat, so replayed Homebrew plans keep showing.
  - **(a) No.** Keep the rulings as they are. This is what part D builds.
  - **(c) Yes.** Cost: it reverses DP-032 ("unknown refuses") and DP-016 (c) for
    replay; it adds a second, replay-dependent rule beside the one DP-032 stated
    once in the domain, so either the domain learns about replay or an adapter
    reports an unknown half as `NO`, which `.claude/rules/adapters.md` forbids; and
    it is a special case for one provider, an INV-17 shape. `security-reviewer`
    recommends against it.
- **Recommendation: (a).** Nothing waits on this: part D proceeds on (a), and (c)
  would be an addition on top.

---

## Resolved

### DP-028 — Whether INV-10 and the charter's non-goals cover what a provider reads from its own configuration — **RESOLVED 2026-10-27: (c), with the exact wording below**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (c). Say precisely what holds, so neither tier reads as a promise about a
provider's own configuration. (b) would refuse users for a configuration they set
on purpose and adds no safety to a read; (a) leaves the promise to be inferred
from silence. The INV-10 paragraph was applied **by the owner**, in
commit `cc844d1` on 2026-10-27 (the invariants file is guarded, and says so); the charter wording is
applied by work order 16 part A on 2026-10-27, after part A checked both
behaviours it promises and built the two `doctor` disclosures that were missing
(SE-109, "Propagation, work order 16 part A"):

- **INV-10, a second paragraph:** "This covers the environment MacPortsBrew
  passes. Configuration a provider reads on its own (`brew.env`, a user
  `macports.conf`) is not MacPortsBrew's to set: it is detected and disclosed
  with every read it may affect, and a mutating step refuses while such a key
  is set (DP-027)."
- **Charter non-goals:** "require Homebrew, MacPorts, network access, or an LLM
  in order to run (a provider's own read may reach the network on its own
  account, and MacPortsBrew says when it can);" and "send telemetry
  (MacPortsBrew sends nothing; if a provider's own configuration enables its
  analytics, MacPortsBrew discloses that)."

Propagation: work order 16, part A (charter, Tier 2, ruled here) and the owner's
INV-10 edit (listed there under "needs the owner").

Raised 2026-10-03 by `spec-guardian`'s review of how the agent recorded
DP-027's ruling. DP-027 put two questions to the owner that sit above the
specification, and the ruling of 2026-10-03 adopted the *mechanism*
((b) for mutation, (c) for reads) without answering them:

1. **INV-10 (Tier 1)** says provider invocations run "in a controlled
   environment, or equivalents". R-15 shows the environment `brew` runs with
   is this tool's, overridden by up to three `brew.env` files; R-16 shows the
   same for `port` through a user `macports.conf`. Is INV-10 met by what this
   tool sets — so the ruling's detection is extra care — or must it also cover
   what the provider reads, so that a read under (c) still falls short of it?
2. **The charter's non-goals (Tier 2)** say MacPortsBrew does not "require …
   network access" and does not "send telemetry". This tool sends nothing, but
   a Homebrew read it runs fetches the catalogue on an empty cache, and reports
   Homebrew's own analytics when a `brew.env` switches them back on. Are those
   consistent with the two sentences?

- **(a) INV-10 is met by what this tool sets; the non-goals are about this
  tool's own behaviour.** Detection and disclosure (DP-027 (c)) is the honest
  addition; no Tier 1 or Tier 2 text changes. Cost: a read can run under a
  configuration this tool did not choose, which a user may read as "controlled"
  not meaning what it says.
- **(b) INV-10 covers what the provider reads.** Then reads must also refuse
  (or relocate, DP-027 (a)) when a key is set, and the invariant's
  wording says so. Cost: refuses users who configured those keys on purpose,
  and an owner's edit to Tier 1.
- **(c) Amend the wording** of INV-10 and the non-goals to say what this tool
  sets and what it discloses, so neither reads as a promise about the
  provider's own configuration. Cost: an owner's edit to Tiers 1 and 2.

**Recommendation:** (c) — say precisely what holds, rather than leave a reader
to infer (a) from silence. **What it blocks:** nothing in this build; the
mutating pass already refuses under DP-027 (b). **Reversible default taken:**
(a), with the reading stated here rather than written into either tier.

### DP-029 — How a test may run code under a different umask — **RESOLVED 2026-10-27: (d), a creation-mode seam, new option**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** **(d)**, an option the review added. None of (a)–(c): (a) leaves the one
line that matters unguarded, and (b) and (c) each weaken a guard the owner put in
on purpose. Instead the durable-write path takes its creation mode from a
crate-private parameter (production passes `0o600`, unchanged); a test creates the
record with a deliberately wide mode (`0o644`) and asserts it ends `0600`. That
kills the removal of the post-creation permission fix without a umask, `unsafe`
or a shell. The `0277` read-back half stays owed and held by reading, as
recorded; revisit only if a second umask-dependent rule appears. Propagation:
work order 16, part B — done 2026-10-27 (`write_new_with_mode`,
`a_record_created_wide_or_narrow_still_lands_0600`, at `0o644` and `0o400`; the mutant that drops the `fchmod` is
killed by it alone).

Raised 2026-10-05, pass 08 slice 2, Part A. SE-092 owes a test that a history
record lands `0600` and reads back under a `0277` umask. The umask is
process-wide, so the test needs a child process under it. Setting a umask takes
`unsafe` (`unsafe_code = "forbid"` in the workspace) or a shell wrapper, and
`scripts/check-no-shell.sh` refuses any test that names a program by a string
literal — deliberately, because such a test can reach a developer's real
provider (INV-05). Both guards are the owner's; this session does not edit or
route around either.

- **(a) Leave it owed and hold the rule by reading.** The code sets the mode on
  the open handle after creation. Cost: removing that line survives every test.
- **(b) A reviewed exception in `check-no-shell.sh`** for one fixed helper (a
  literal path to the system shell, used only to set a umask and exec this
  crate's own test binary). Cost: an edit to a guarded script, and a hole the
  guard's author closed on purpose.
- **(c) `unsafe` for one `umask` call in a single test crate.** Cost: lifts
  `forbid` for that crate; the call is one line and has no memory-safety
  content, but the workspace lint is Tier-3 policy that would need an entry.

**Recommendation:** (a) for now — the property is one line and is read in
review; take (b) only if a second umask-dependent rule appears. **What it
blocks:** nothing. **Reversible default taken:** (a).

### DP-030 — How, if at all, MacPortsBrew detects a `brew` or `port` that is mutating — **RESOLVED 2026-10-27: (b), built before the flip proposal**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (b). A mutating release that cannot see a provider mutating at the
same moment is not complete; "possibly inconsistent" on every run is not an
answer a user can act on. Build the `F_GETLK` probe on each provider's locks
as R-17 locates them (Homebrew: every file under `<prefix>/var/homebrew/locks/`,
including the suffix-less `update` and `vendor-install` locks; MacPorts: the
registry lock under the detected `portdbpath`), with the locations held in
`data/policy/` rather than in code (INV-20). One ADR chooses between
`libc` with one reviewed `unsafe` call in a single module and a maintained safe
wrapper whose transitive set is verified with `cargo metadata`; that ADR
supersedes ADR-0006 **for this purpose only**, and is shared with DP-049's if
`sha2` reaches `libc` too. The claim is "a provider lock is held right now",
never "idle"; a held lock refuses the run with a named error. MacPorts' lock
being taken only by a registry-writing `port` (R-17) is the provider's asymmetry,
documented and reviewed under INV-17. (c) stays rejected. Propagation: work
order 16, part G.

Raised 2026-10-05, pass 08 slice 2, from R-17 and R-18. 04's MUST asks for
detection of a concurrently running `brew` or `port` before mutating, and a
refusal. Observation (R-17): both providers hold a kernel `flock` for the
duration of a mutation, so a file's presence proves nothing and a stale lock
does not exist; the only probe that acquires nothing is `fcntl(F_GETLK)`. It is
unreachable from `std`; the workspace forbids `unsafe`; `libc` would be a new
dependency; a process listing grows the spawn allowlist. Nothing in this slice
does any of those, so **the detector is not built and the refusal is owed**.

- **(a) Stay owed.** Every live run is flagged "possibly inconsistent" and no
  mutation is dispatched in this build anyway. A test fails the build that sets
  `THIS_BUILD_DISPATCHES_MUTATIONS` while a live run says `NOT_CHECKED`, so the
  flip proposal must come back to this decision. Cost: nothing now; before the
  flip, the owner decides.
- **(b) One `libc` dependency and one reviewed `unsafe` call** for `F_GETLK` on
  each provider's lock file (Homebrew's `locks/*.lock`, MacPorts'
  `.registry.lock`). Honest claim: "a lock is held right now", never "idle".
  Cost: a new dependency (an ADR), lifting `forbid` for one module, and an
  asymmetry between providers that is theirs (MacPorts' lock is taken only by a
  `port` that can write the registry), to be reviewed under INV-17.
- **(c) A MacPorts-only `try_lock_shared` probe.** Safe there (MacPorts waits);
  unsafe for Homebrew (it aborts), so it covers one provider. Cost: violates
  the peer-provider rule in spirit; rejected unless (b) is refused.

**Recommendation:** (a) now, (b) at the flip proposal if the owner accepts the
dependency. **What it blocks:** the flag-flip proposal, by test, not by
convention. **Reversible default taken:** (a).

### DP-032 — What "defines a service" means for MacPorts, given that Homebrew's answer is a declaration — **RESOLVED 2026-10-27: (c) then (b); the answer is the union**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (c) as the first step, then (b), and the answer is the **union**, stated
once in the domain for both providers (each adapter supplies its halves; for
Homebrew, the formula's `service` block and a materialised `homebrew.mxcl.*`
plist). For MacPorts: a port defines a service if its installed Portfile declares a startup item
(directly or through a PortGroup that adds one) **or** MacPorts materialised a
launchd item that `port -q provides` attributes to it. A Portfile the bounded
reader cannot interpret makes the answer unknown, and the plan is refused, so
the fail-closed property of today's scan is kept. Requires an R-8 addendum and a
captured fixture. Propagation: work order 16, part D.

Raised 2026-10-10, pass 08 slice 3, by `spec-guardian` (M1). INV-19 forces
`MANUAL_REVIEW` for a package that owns service state ("managed launch agents").
Homebrew answers from the formula's `service` block — what the package
**declares**. MacPorts has no such field in `port info` (R-8); the adapter answers
from the launchd items MacPorts **materialised** under the prefix. A port built
with `startupitem.type none`, or under a system `macports.conf`
`startupitem_type none`, materialises none, so the two providers can answer
differently for the same software (`nginx`), which bears on INV-17.

- **(a) Keep the materialised meaning for MacPorts**, documented. Matches
  INV-19's "managed launch agents" wording. Cost: a port the user runs from their
  own plist is not reviewed.
- **(b) Read the declaration**: the installed Portfile MacPorts keeps per version
  under `<prefix>/var/macports/registry/portfiles/<name>-<version>_<revision>/`,
  including PortGroups that add startup items. Covers inactive ports too. Cost: a
  bounded textual read of Tcl, an R-8 addendum, and a new class of parser.
- **(c) Attribute materialised items with `port -q provides`** (already on the
  read allowlist) instead of by directory name. Cost: one capture-time invocation;
  still the materialised meaning.

**Recommendation:** (b), with (c) as the step before it. **What it blocks:**
nothing in v0.1.0 — the slice-3 scan is fail-closed (any item it cannot attribute
makes the answer unknown, and the plan is refused). **Reversible default taken:**
(a), fail-closed.

### DP-036 — DP-019's ruling names an `.app`; is that the `.dmg`? — **RESOLVED 2026-10-27: (a), the `.dmg`**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (a). The `.app` in DP-019's ruling is the `.dmg`; nothing changes.
No application bundle is planned.

Raised 2026-10-10, pass 09, by `spec-guardian` (L5). DP-019's ruling reads "The
`.app`, the `.pkg` and the Intel build all ship". No `.app` exists in any
design: DP-008, ADR-0005 and 08 name a bare executable, a `.pkg` and a `.dmg`.
Pass 09 reads ".app" as the `.dmg`, and says so rather than silently
reinterpreting it.

- **(a) Yes, the `.dmg`.** Nothing changes.
- **(b) An application bundle was meant.** A command-line tool in an `.app` is
  a different artifact (bundle layout, Info.plist, a different Gatekeeper path)
  and would be its own pass.

**Recommendation:** (a). **What it blocks:** nothing before the first tag's
artifact list is final. **Reversible default taken:** (a).

### DP-037 — Is naming a package the "explicitly entered advanced mode" INV-13 requires? — **RESOLVED 2026-10-27: (b), with the warning and the stronger confirmation**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (b), both halves. Naming a package outside the default view is the
explicit entry into advanced mode **for that package only**: `advanced.warning`
is printed (and carried in `--json`), the plan records how the row was entered,
and any destructive step for it (`REMOVE_SOURCE`) needs the stronger confirmation
`06-cli-and-ux.md` requires (it does not yet define one: work order 16 part K
defines both levels, normal and advanced, and how each works under `--yes`). A named package's own transitive dependencies are
never added implicitly. Test:
`invariant_13_a_named_dependency_warns_and_needs_the_stronger_confirmation`.
Propagation: work order 16, part C.

Raised 2026-10-11 by `spec-guardian` (pre-release review, H1). INV-13 (Tier 1)
says transitive dependencies are "reachable only through an explicitly entered
advanced mode", and `06-cli-and-ux.md` says entering it displays a warning and
that destructive actions there need a stronger confirmation. `plan` and
`migrate` analyse with `advanced: true` for any package named on the command
line (SE-048: "naming a package is the explicit entry INV-13 requires"). That
is an agent's reading of a Tier 1 sentence, never ruled by the owner. So
`plan openssl@3 --to macports` on a dependency row builds a plan with no
warning; nothing executes in this build, but the mutating pass would inherit
the reading.

- **(a) Refuse, or force `MANUAL_REVIEW`, for a source row the normal view hides
  unless `--advanced` was given**, with `invariant_13_a_dependency_is_not_planned_without_advanced`.
- **(b) Rule that naming a package is the explicit entry**, and meet 06's
  warning on that path (print `advanced.warning` when the named package is
  outside the default view) and its stronger confirmation when execution lands,
  or amend 06 with an SE entry.

**Recommendation:** (b) with the warning: naming a package is a deliberate act,
and a refusal would make the planner useless for the dependency a user is
actually asking about; the warning keeps the act informed. **What it blocks:**
the mutating pass (slice 4), not the read-only 0.1.0. **Reversible default
taken:** no change in behaviour; this entry is the record.

### DP-039 — `claude.yml` can run a pull request's own hooks with secrets — **RESOLVED 2026-10-27: (b), hardened before any automatic trigger returns**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (b). Until the hardening lands, both workflows stay manual-only
(SE-099) and no key is set. The hardening: refuse when the pull request's head
repository is not the base; load settings and hooks from a trusted base-branch
checkout, never from the head; accept a trigger only from `OWNER`/`MEMBER`; no
`Write`/`Edit` and only an allowlisted `Bash`; the key in an environment with a
required reviewer. `.github/workflows/` is guarded, so the session stages the
files and the owner sends the armed message. Propagation: work order 16, part M.

Raised 2026-10-11 by `security-reviewer` (pre-release review, H1).
`.github/workflows/claude.yml` runs on `issue_comment` with the API key and a
`contents: write` token, and Claude Code loads `.claude/settings.json` and its
hooks from the checked-out tree. A fork pull request that changes them, plus an
`@claude` comment by any collaborator, would run that code with the secrets.
The workflow is dormant until the repository is public and the secret is set.

Options: **(a)** before publishing, disable the workflow (remove the secret or
the file) until it is hardened; **(b)** harden it: refuse when the pull
request's head repository is not the base, read settings from a trusted
base-branch checkout, narrow the trigger to `OWNER`, and drop `Write`/`Edit`;
**(c)** keep it as is.

**Recommendation:** (a) now, (b) when it is wanted. `.github/workflows/` is
guarded, so either needs the owner's armed write. **What it blocks:** setting
the repository's `ANTHROPIC_API_KEY` secret. **Reversible default taken:** the
file is unchanged; this entry is the warning.

**Update 2026-10-17 (owner's instruction, SE-099):** both Claude workflows
(`claude.yml` and `claude-code-review.yml`) are now **manual only**
(`workflow_dispatch`, with the task or the pull request number as an input). This
is a step toward (a), not (b): the file is kept, nothing starts it by itself, and
the exposure above is reachable only by someone with write access who runs it on
purpose and holds the key. The repository holds no key, so neither can run today
(the review check had failed on every pull request for that reason). **Still
open, and now narrower:** the hardening of (b) before any automatic trigger is put
back, or before the key is set and the workflow is run against a ref someone else
controls. To re-enable the old triggers, restore `issue_comment` and
`pull_request_review_comment` (and `pull_request`) and the author-association
conditions that the workflows' own comments describe, and set the secret.

### DP-046 — May the first mutating release refuse every elevated step? — **RESOLVED 2026-10-27: (b): the first mutating release runs both directions**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** **(b), and the first mutating release waits for it.** INV-17 says
neither direction is a special case, the charter says MacPortsBrew is not a
one-way escape hatch, and (a) would ship nothing runnable to the Intel Homebrew
users the project started for. The evidence argument behind (a) is kept another
way: v0.2.0 is not tagged until the acceptance protocol (work order 11) has
passed **in both directions** on disposable machines per architecture, and the
non-elevating path (MacPorts→Homebrew formulae, with the source retained,
because removing a MacPorts port elevates) is exercised there first, as a
milestone, not as a release. The elevated spawn mechanism is constrained: an
absolute `sudo` path and argv only; elevation only where the data cell says
`required` or detection proves it; every elevated step enumerated in the plan
with its exact argv and consented to per step; no credential cached across runs
or left behind; the process itself never runs as root (INV-06); the environment the
elevated child actually receives checked against INV-10. What `sudo` passes on is
decided by the user's sudoers policy, so a dated research note comes first, and
that policy is detected and disclosed, or refused, like DP-028's provider
configuration. A cask whose elevation stays `unknown` after detection is
refused with a named reason (INV-20). Design and ADR first, reviewed by
`security-reviewer` and `spec-guardian` before any code. Propagation: work order
16, part J.

Raised 2026-10-18 (design notes for slice 7, `docs/passes/08-pre-flip-design-notes.md`).
Elevation consent is built (slice 4) and will be wired (slice 7), but no
elevated spawn mechanism exists: the gate refuses every elevated step
(`crates/application/src/execution_gate.rs`) and the guard halts on
`ElevationNotNone` (`crates/application/src/executor.rs`). The elevation cells
in `data/policy/provider-commands.toml` mark MacPorts' `add` and `remove` as
`required` (a static data cell; detection never relaxes it for a mutating
operation, `executor.rs` ≈671–673, so even a user-owned MacPorts prefix is
refused), and every cask's `add_gui_app` as `unknown`, which is refused too. So
a release that keeps that refusal can migrate **MacPorts→Homebrew, formulae
only**, and never asks for consent in a shipped build.

- **(a) Yes: the first release refuses elevated steps.** v0.2.0 moves
  MacPorts→Homebrew formulae, retaining the source (removal also elevates —
  DP-048), further narrowed by slice 6's rule that a keg-only formula cannot be
  verified active; a Homebrew→MacPorts plan and every cask are refused at the
  gate with a named reason. Cost: one direction of a peer pair (INV-17) is
  unavailable in that release. The planner and executor are the same code in
  both directions; the refusal is a release policy (no elevation yet) applied to
  stored provider knowledge, not to a detected fact (INV-20 prefers detection
  where it can). **Owner question:** does that satisfy INV-17, and the charter's
  statement that MacPortsBrew is not a one-way escape hatch? That is a reading of
  a Tier 1 invariant and of the charter, the owner's. It also means the audience
  the project started for — Intel Homebrew users moving to MacPorts — gets
  nothing it can run from v0.2.0, and the README must say so plainly.
- **(b) No: an elevated spawn mechanism lands first** (a `sudo` argv and a new
  permitted program, a security decision under SE-016 and INV-06). Cost: the
  largest new attack surface in the project, before any mutation has ever run
  on a real machine.
- **(c) Postpone the first mutating release** until both directions can run.

**Recommendation:** (a), with (b) as its own later pass after v0.2.0 has run on
disposable machines. Moving the riskier half second is how the executor gets
real-world evidence before it elevates. Ruling (a) includes ruling the INV-17
reading above; `spec-guardian` reviews the argument when the proposal is
written, but cannot settle it.
**What it blocks:** the flip proposal (work order 08 slice 9).
**Reversible default taken:** (a); nothing elevates in this build.

### DP-047 — How the first mutating release sets a primary (`SET_PRIMARY`, `VERIFY_RESOLUTION`) — **RESOLVED 2026-10-27: (b) now, (a) as a consented layer before v0.2.0**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (b) now (slice 8 as designed), then (a) before v0.2.0, **only with
consent**. Without layer 4, Homebrew→MacPorts is refused on every machine where
Homebrew's `bin` comes first, which is DP-046's asymmetry again. A shim is never
written implicitly: when the destination does not win `PATH`, the plan offers
layer 4 for that executable (one shim in the MacPortsBrew-owned directory, one
managed `PATH` block, a backup, a preview, `env --revert`, restore support,
idempotent per INV-16); without consent the gate refuses with
`PRIMARY_NEEDS_SHIM` and names layers 1 and 2. The executable list comes from
provider metadata, so `rg` for `ripgrep` is planned rather than refused.
INV-11 and INV-12 bind. Propagation: work order 16, parts F and I.

Raised 2026-10-18 (design notes for slice 8). Every planned run today ends
`PARTIAL`: `SET_PRIMARY` carries `macportsbrew primary <name> <to>`, a command
that does not exist, and `VERIFY_RESOLUTION` asserts nothing, so the executor
halts on both (`crates/application/src/planner.rs`, `executor.rs`). Spec 05 has
four layers; layer 4 (the shim directory and a managed `PATH` block) is the only
one that writes outside the providers.

- **(a) Build layer 4 now.** Cost: the first write into a user's home outside
  the state directory, a shell-configuration write with backup, preview and
  `env --revert`, restore support, and shim recognition with no machine to
  capture a fixture from.
- **(b) Report only, and refuse what needs a shim.** The planner reads the
  captured `PATH` order: when the destination's `bin` already wins,
  `SET_PRIMARY` has nothing to change and `VERIFY_RESOLUTION` checks a fresh
  resolution in process (no spawn); when it would need a shim, the gate refuses
  before anything is installed (`PRIMARY_NEEDS_SHIM`, exit 4), naming layers 1
  and 2. Cost: on a machine where Homebrew's `bin` precedes MacPorts' (as on
  the one captured machine with both, `both-providers-intel-macos26`, `PATH`
  indices 6 and 25; MacPorts' installer prepends `/opt/local/bin` too, so the
  order varies), Homebrew→MacPorts is refused — the same kind of asymmetry as
  DP-046, here from a detected machine fact (the captured resolution). With
  DP-046 (a) and DP-048 (c), v0.2.0 only moves towards Homebrew; the INV-17 and
  charter question in DP-046 applies here too, and is the owner's.
- **(c) Report only, and proceed.** Install, then end `PARTIAL` with
  `PRIMARY_NOT_SET`. Cost: every such run is partial by design; `COMPLETED`
  never means what the user asked for.

**Recommendation:** (b). It writes nothing outside the providers, makes
`COMPLETED` mean exactly "the destination now runs", and layer 4 later only
widens what is accepted. A package whose executable is not its own name (`rg`
for `ripgrep`) is refused under (b) until provider metadata supplies the
executable list — the slice's own design decision, recorded in SE.
**What it blocks:** slice 8 and therefore the flip. **Reversible default taken:**
(b) is what slice 8 builds unless the owner rules otherwise.

### DP-048 — The window between the last destination check and the provider's uninstall — **RESOLVED 2026-10-27: (c), with removal opt-in in v0.2.0**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (c), and removal ships in v0.2.0 **as an opt-in**. Today removal is
the default (`06-cli-and-ux.md` gives `[--retain-source]` as the opt-out); this
ruling flips it: the source is retained by default, and `--remove-source` is
chosen per run with a confirmation stronger than an install's. Removal is
dispatched only when slice 6's re-read of the destination immediately before
`REMOVE_SOURCE` exists, DP-030's probe exists, and all six destination checks
were evaluated for that package. The residual window between
the last re-check and the provider's uninstall is accepted and declared in 04
next to DP-030's. The flip changes 06's synopsis, `retainSource` in the
plan and history schemas, and `restore`; work order 16 part K records it. Propagation: work order 16, part K.

Raised 2026-10-18 (design notes for slice 6). Slice 6 adds a re-verification of
the destination immediately before `REMOVE_SOURCE` is dispatched, after the lock
and configuration re-checks. What remains is the gap between that read and the
provider's own uninstall — milliseconds plus the provider's run time. Only a
provider lock closes it (R-17, DP-030).

- **(a) Accept the residual**, declared in 04 next to DP-030's.
- **(b) The first release never removes:** `--retain-source` is forced and
  `REMOVE_SOURCE` is refused at the gate, so the window cannot open; removal
  becomes a later release's decision.
- **(c) Both:** build the re-check now (slice 6), ship v0.2.0 with removal
  refused, and accept the residual when removal is enabled.

**Recommendation:** (c). The re-check is cheap and must exist before removal is
ever on; a first release that cannot remove anything cannot break INV-01 in
practice. **What it blocks:** the flip proposal. **Reversible default taken:**
(c) — slice 6 builds the re-check, and the proposal's defaults refuse
`REMOVE_SOURCE`.

### DP-049 — Tying the executed plan to the plan shown: a `sha2` dependency and `--plan-digest` — **RESOLVED 2026-10-27: (a) and (b), both routes in v0.2.0**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (a) **and** (b), both in v0.2.0: `migrate --yes --plan-digest <d>`
for scripts, and an interactive confirmation of the same in-memory plan for a
person at a terminal. `sha2` enters through an ADR under ADR-0004
(`default-features = false`, transitive set verified with `cargo metadata`); if
it reaches `libc`, the ADR is the one DP-030 writes and supersedes ADR-0006 for
those two purposes only. (c) stays rejected. Propagation: work order 16, part E
(slice 7).

Raised 2026-10-18 (design notes for slice 7). `plan.v1.md` obligation 8 allows
"under `--yes` a digest of a previously rendered plan that must match". Today
`migrate --yes` runs without anyone seeing the plan first
(`crates/cli/src/app/migrate.rs`). The recommended design hashes the canonical
in-memory `MigrationPlan` with SHA-256, prints it as `planDigest`, and requires
`migrate --yes --plan-digest <d>`. `Cargo.lock` has no hash crate; `sha2`
(RustCrypto; licence, maintenance and transitive crates to be verified in the
ADR with `cargo metadata`) would be a new dependency, which ADR-0004's gate admits
only through an ADR.

**Why this is the owner's and not an ADR alone:** under ADR-0004 a dependency is
normally the agent's decision through an ADR. This one also changes the command
a user types (`--plan-digest`), and its transitive set may include `libc` (an
unverified lead: `cpufeatures` on aarch64 Apple targets), which accepted
ADR-0006 rejects — superseding an accepted ADR in the safety path is the owner's.

- **(a) `sha2` plus `--plan-digest`.** Cost: one dependency and whatever it pulls
  in (possibly reopening ADR-0006); scripts run `migrate` twice (show, then
  execute with the digest).
- **(b) No dependency: an interactive confirmation on a terminal only** (the
  same in-memory object shown, then confirmed). Cost: `--yes` and scripts cannot
  execute at all until (a) exists.
- **(c) A hand-written or non-cryptographic hash.** Rejected: cryptography in
  the safety path, or collisions an attacker controlling an inventory could
  craft.

**Recommendation:** (a), with (b) added later as a second route for a person at a
terminal. **What it blocks:** slice 7 part A, and therefore the flip.
**Reversible default taken:** none for part A — slice 7 writes the ADR and
builds parts B–D, and stops before adding the dependency until the owner rules.

### DP-050 — Does a destination with no bottle for this machine become `MANUAL_REVIEW`? — **RESOLVED 2026-10-27: (a), plus a toolchain check**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (a). A source build is a cost the plan names, not an unavailable
artifact; 04's example is corrected. The plan shows the source-build risk in the
default view with what the build needs. If research shows the absence of the
build toolchain (the Command Line Tools; to be confirmed in a dated note) can be detected by a filesystem read,
with no new spawn, its absence makes that step `MANUAL_REVIEW`; otherwise the
risk text says so. Propagation: work order 16, parts A and K.

Raised 2026-10-18 (design notes for slice 6). 04 (Undoing a migration) says a
step whose artifact is no longer available becomes `MANUAL_REVIEW`. Slice 6
makes the planner raise one for `Unavailable`, `UnsupportedPlatform` and
`Disabled`. 04's own example — "an Intel Mac can no longer bottle" — is
`SourceBuildRequired{NoArtifactForThisPlatform}` in the Homebrew adapter, which
that rule does not block.

- **(a) Keep source builds non-blocking** (today's behaviour): the plan carries
  the existing `destination_source_build` risk. Cost: 04's example is met only
  as a risk, not as a stop.
- **(b) Block `NoArtifactForThisPlatform` too.** Cost: every migration towards
  Homebrew on a Tier-3 Intel machine for a formula without an Intel bottle stops
  at `MANUAL_REVIEW` — the audience this project started for.
- **(c) Block it only for a restore.** Rejected: 04 says a restore is not a
  special case (INV-17 in spirit).

**Recommendation:** (a); after the ruling, correct 04's example. A source build
is a cost the plan names, not an unavailable artifact. **What it blocks:**
nothing. **Reversible default taken:** (a), with a declared-deviation note in 04
pointing here (slice 6); 04's MUST text is not rewritten before the ruling.

### DP-051 — Four checks of the destination contract no build can evaluate — **RESOLVED 2026-10-27: (a) before v0.2.0, with (b)'s honesty per package**

**Ruling (2026-10-27, made by the session under the owner's delegation, SE-109; confirmed by the owner in chat the same day):** (a), all four checks built before v0.2.0, because DP-048 ships
removal (`PROVIDER_REPORTS_ACTIVE`, the fifth, is slice 6 part B's). Each lands with its research note. A read added to the allowlist must
be shown read-only by that note and passed by `security-reviewer`; a form that
can write is refused, whatever it would check. `LIVENESS_PROBE` runs the
destination's resolved executable only with a curated probe from dated data
(`source`, `read_on`): argv only, no shell, sanitised environment, no stdin, a
scratch working directory, and the timeout of ADR-0006; only the exit status
counts. A package with no curated probe, or any check that cannot be evaluated
for it, is reported unchecked: its source is never removed, and the run ends with
a named outcome that is not plain `COMPLETED`. Propagation: work order 16, part H.

Raised 2026-10-18 by `spec-guardian`, reviewing the handoff after slice 5. The
planner puts six checks on `VERIFY_DESTINATION` (`crates/application/src/planner.rs`
≈861–875), as 04's verification contract requires (≈47–58). The gate can
evaluate only `PACKAGE_NEWLY_PRESENT` (`execution_gate.rs` ≈509–521), and slice 6
adds `PROVIDER_REPORTS_ACTIVE`. The other four halt every real run
(`executor.rs` ≈605–611; `plan.v1.md` obligation 6; SE-094):
`PROVIDER_INTEGRITY_CHECK`, `EXECUTABLES_OWNED_BY_DESTINATION`, `LIVENESS_PROBE`
and `NO_CROSS_PROVIDER_LINKAGE`. `LIVENESS_PROBE` runs the installed program,
which is outside `PERMITTED_PROGRAMS`; an integrity, ownership or linkage read
probably grows the process allowlist. Both are the owner's.

- **(a) Build all four** in a new slice, each with its research note and the
  allowlist growth the owner approves per read. Cost: the largest remaining
  work before the flip, and running an arbitrary installed program (with what
  arguments, under what timeout) is a new risk class.
- **(b) A declared deviation for a release that never removes:** with
  `REMOVE_SOURCE` refused by release policy (DP-048, slice 6 part D), the
  verification contract guards no removal; `VERIFY_DESTINATION` certifies the
  install with the two evaluable checks, the four others are reported as not
  checked in the run's record, and the run ends `COMPLETED_WITH_UNCHECKED` (or
  stays `PARTIAL`) — never plain `COMPLETED`. The four become required, by test,
  the moment removal is enabled. Cost: a deviation from a MUST in 04, declared
  in SE and in the README, and a new outcome to name.
- **(c) Postpone the flip** until (a).

**Recommendation:** (b) for v0.2.0, (a) before any release that removes. The
contract exists to protect a removal; a release that cannot remove can say
honestly what it did not check. **What it blocks:** the flip, and slice 8's
`COMPLETED`. **Reversible default taken:** none — the planner keeps emitting all
six checks, the tripwire (widened in slice 6) keeps failing, and no slice may
make a run complete by dropping a check.

### DP-053 — What edit of `build-release.sh` must the sign-path gate catch: an accident, or a hostile one? — **RESOLVED 2026-10-24: (c)**

**Ruling (owner, 2026-10-24):** (c). The gate's job is the accidental regression now; a structural defence against a hostile edit comes before anyone else can change `build-release.sh`. Follow-up 2f carries the cheap hardening; the structural defence is open work, not yet scheduled.


Raised 2026-10-24 by `security-reviewer`, reviewing pass 15 follow-up 2e. The gate
`build_release_never_executes_the_input_on_the_sign_path` (`scripts/test-gates.sh`)
now runs the whole signed path under stubs, with the input as the canary, and keeps
a text scanner as a backstop. The review built three edits that run the input on
the signing runner and pass both:

- a run conditioned on CI's environment (`if [ -s "$MPB_NOTARY_KEY_PATH" ]`, or
  `GITHUB_ACTIONS`): the harness's key file is empty, so the branch never runs there;
- a run delayed and detached (`(sleep 20; "$input") &` through `awk system()`),
  which happens after the harness has looked;
- `xcrun "$bin"`, which the real `xcrun` runs, while the stub refused it quietly.
  The text check catches this one.

A test can make each of these harder: an environment that looks like CI, a
process-group check, stubs that fail loudly on any form they do not know, taint
through `$(...)`, and refusing `system(` in awk programs. No test of a shell script
can be complete against an edit *written to evade it*. The author of that edit can
read the test.

- **(a) The gate's job is the accidental regression.** That means an honest edit
  that adds `"$bin" --version` to a diagnostic or a new branch, or moves the run.
  Hostile edits are held where they can be: by review of every change to
  `build-release.sh` and `release.yml`, by the `v*` ruleset and the
  `release-signing` environment (item 5), and by the signing job never running a
  script from an unreviewed ref. The cheap hardening above goes in as follow-up
  2f, and the gate's claims say "accidental".
- **(b) The gate must also resist a hostile edit.** This needs a structural
  change, not a test. One option is a signing job that cannot execute the input:
  it holds the bytes only as an opaque file passed to `codesign`/`notarytool`, for
  example in a sandbox profile that denies `process-exec` on that path, or on a
  separate runner. Cost: a redesign of the signing job, which needs research
  (sandbox-exec is deprecated) and the owner's runner budget.
- **(c) Both:** (a) now, (b) before a second maintainer or an outside contribution
  can change `build-release.sh`.

**Recommendation:** (c). Today the only author of `build-release.sh` is the owner
or a session the owner arms and reviews, so (a) is enough to tag. (b) becomes
necessary the day someone else can land a change there.

**What it blocks:** nothing before the tag under (a). The 2d and 2e work stays
uncommitted until the owner rules, because its last review found Highs.

**Reversible default taken:** none committed. The 2d and 2e changes sit in the
working tree of `pass/15-first-tag-readiness`, and their docs say what the gate
does not catch.

### DP-052 — Does the release publish a bare `mpb` file beside the bare `macportsbrew`? — **RESOLVED 2026-10-23: (a) keep it**

- **Ruled 2026-10-23 by the owner: (a).** Four files per architecture. ADR-0005's
  Decision is amended to four artifacts; spec 08 and SE-104 say so. Carrying
  `payload_names`, `mpb_sha256` and `mpb_cdhash` into `SIGNING-RECORD.txt` is a
  guarded `release.yml` edit and lands with follow-ups 2a
  (`prompts/ROADMAP-to-first-release.md`), before the tag.

- **Context (2026-10-23, pass 15 part B).** DP-033 (b) says "the bare-file route
  still installs one name". The owner's armed message for part B asked that the
  payload assertions, "SHA256SUMS" included, gain the second file; `SHA256SUMS`
  lists published files, so part B read that as a published bare
  `mpb-<version>-<arch>` and built it (commit `3ad50fa`: four files per
  architecture, the bare `mpb` byte-identical, its CDHash asserted equal to the
  executable's, not notarised separately). `spec-guardian` found that this goes
  beyond DP-033 (b) and beyond ADR-0005's Decision ("publish three artifacts").
- **(a) Keep it.** Four files per architecture; ADR-0005's Decision line is amended
  to four; spec 08 drops "DP-052, open". Cost: a fifth upload per architecture that
  a user could make with `cp`, an unobserved notarisation inference (that the
  executable's ticket covers a file with its CDHash), and `SIGNING-RECORD.txt` must
  carry `mpb_sha256`/`mpb_cdhash` (follow-up 2a).
- **(b) Drop it.** `build-release.sh` publishes three files again (the `.pkg` and
  `.dmg` still carry both names); the README row goes, and the documented `cp` line
  stays the bare route's way to the short name. Cost: one armed message (guarded
  file) and the gate cases' needles change.
- **Recommendation: (b)**, the ruling as written: the bare route already gets the
  short name with one documented `cp`, and every published file is one more thing
  to notarise, attest and revoke.
- **Reversible default taken:** the build as it is (a), because it is committed and
  undoing it needs a guarded write; nothing is published before the owner tags, and
  the tag is the owner's. **What it blocks:** the tag.

---

### DP-034 — The temporary keychain password reaches `security`'s argv, which ADR-0005's text forbids — **RESOLVED 2026-10-23: (b), `security -i` holds on all four runners**

- **Ruled 2026-10-23 by the owner:** (b) first, (a) if it failed. **It held**: one
  run of the throwaway research workflow on macos-26, macos-26-intel, macos-15 and
  macos-15-intel (R-19 addendum of 2026-10-23, run 40033361119, pull request #14
  closed unmerged). The passwords do not reach argv, nothing is echoed or written,
  and ADR-0005 needs no exception. **Two things the signing step must do because of
  it:** one `security -i` per command (a pipe of several reports only the last
  command's exit status), and hex passwords (single quotes are not honoured).
  **Owed, part B:** the signing step in `release.yml` takes the recipe in the
  addendum, and its comment about "a deviation awaiting a ruling" goes. **Not
  shown:** that the stdin-built keychain signs (the control failed to sign too;
  the first dry run with the real certificates settles it).
- The original entry follows.

- **Ruled 2026-10-23 by the owner:** follow the recommendation. First research whether `security -i` (commands on stdin) works for `create-keychain`, `unlock-keychain`, `import` and `set-key-partition-list`, does not echo them and keeps no history, on a hosted macOS runner with a throwaway keychain and **no secrets** (macos-15 and macos-26). If it holds, the signing step uses it and ADR-0005 needs no exception. If not, (a): the two per-job random values in argv on the ephemeral VM, masked, ADR-0005 amended.
- **Needs from the owner:** an armed message to write the research workflow into `.github/workflows/`, and permission to run it once on a `pull_request` of a topic branch (the pass-15 work order forbids running workflows). The workflow is deleted and the pull request closed unmerged afterwards. **Closes** with an R-19 addendum holding the dated observation.
- The original entry follows.

Raised 2026-10-10, pass 09, by `spec-guardian` (M1) and `security-reviewer`
(L3). ADR-0005 lists "the temporary keychain password" among the credentials
that must never reach a public log, and says: "Every credential reaches its
tool by file or stdin, never in `argv`". The staged signing step passes a
per-job random keychain password to `security create-keychain -p`,
`unlock-keychain -p` and `set-key-partition-list -k`, and a per-job random
re-wrap password to `security import -P`. Your `.p12` passwords and notary key
never reach argv. This is a **deviation from the ADR's text**, not a reading of
it.

- **(a) Accept argv for the two per-job random values on the ephemeral hosted
  VM**, masked with `::add-mask::`, generated and unset inside the one step,
  worthless once the VM is destroyed; amend ADR-0005 to say so. Cost: a value
  visible to `ps` on that VM for the step's life. `security-reviewer` accepts
  this provided nothing else runs there — which the pass-09 fix (the signing job
  never executes the built program) now ensures.
- **(b) `security -i`, reading its commands from stdin**, so neither password
  appears in any process's argv. **Unverified:** whether interactive mode
  accepts these subcommands from a pipe, echoes them, or records history needs a
  dated observation on a throwaway keychain in a disposable account or VM (not
  this machine, where a keychain write is out of bounds). Cost: one research
  addendum; a less common recipe.
- **(c) Another route** found by that research (for example a tool reading the
  passphrase from a file descriptor). Cost: unknown until looked for.

**Recommendation:** (b) investigated first, as an R-19 addendum; (a) if it does
not hold. **What it blocks:** the first tag. **Reversible default taken:** (a)
as staged, with the deviation stated in the workflow; nothing is signed before
you rule.



### DP-045 — Do pass 08 slices 4 and 5 ship in 0.1.0? — **RESOLVED 2026-10-23: (a) they ship in 0.1.0, described honestly**

- **Ruled 2026-10-23 by the owner: (a).** `history`, `history show`,
  `history acknowledge`, `restore --plan --run` and `doctor`'s `interrupted_run`
  finding ship in 0.1.0. **Propagated:** the `DP-045 pending` markers are gone from
  `CHANGELOG.md` and `README.md`; their text stands. **Done 2026-10-25:** the
  top-level `--help` called `history` "Reads only" while `history acknowledge --yes`
  can write; it now names `acknowledge --yes` as the one writer. It was a clap doc
  comment in `crates/cli/src/app/mod.rs`, not a catalogue string, and had no pt-BR
  twin (`--help` is English by a stated exemption); roadmap item 3 has the tests.
- The original entry follows.

Raised 2026-10-18, handoff after slice 5. `main` now carries `history`,
`history show`, `history acknowledge`, `restore --plan --run`, `restore --run`
(refused) and `doctor`'s `interrupted_run` finding. Nothing hides them in
`--help` (`crates/cli/src/app/mod.rs`). With mutations off, `history` is always
empty, `restore --plan --run <id>` answers `RUN_NOT_FOUND` (exit 8) and the
finding is always OK. `CHANGELOG.md` (the 0.1.0 section, around line 108) still
says 0.1.0 does not keep history or restore, while `README.md` (around line 87)
documents `history` and `restore --plan`. DP-044 is the precedent for pass 12.

- **(a) Ship them in 0.1.0, described honestly.** The CHANGELOG folds them in,
  saying what each prints on a machine where no run was ever recorded. Cost: a
  surface that can only say "nothing yet" until 0.2.0, and the docs-truth work
  of saying so everywhere.
- **(b) Tag an earlier commit** (before PR #8). Cost: the tag misses every fix
  made since, including the security and tooling fixes after slice 5, and the
  release pipeline is then proven on a commit that is not `main`.
- **(c) Hide them from `--help` in 0.1.0.** Cost: code that exists but is
  undocumented, which is how a surface drifts; a hidden command is still
  reachable.

**Recommendation:** (a). The commands are read-only, already reviewed, and their
"nothing yet" answer is true. **What it blocks:** the CHANGELOG and README text
of the first tag (work order 15). **Reversible default taken:** (a) in the text
staged by work order 15; nothing is tagged before the owner rules.

---

### DP-033 — No release artifact installs `mpb`, and the charter says it is installed alongside `macportsbrew` — a charter amendment — **RESOLVED 2026-10-23: (b) the .pkg and .dmg also carry mpb**

- **Ruled 2026-10-23 by the owner: (b).** The charter stands; the `.pkg` and the
  `.dmg` carry `mpb` beside `macportsbrew`. Recorded in `docs/spec-evolution.md`
  SE-104. **Propagated:** spec 06's MUST (the guard for an installer that runs no
  code is `PATH` order, and the documented line appends), ADR-0003 and ADR-0005
  (amended, the latter reversed on the alias), the README, `wiki/Getting-Started.md`
  and `wiki/Primary-Provider.md`; spec 08's installer MUST was missed and amended
  in pass 15 part B (2026-10-23). The bare `mpb` the build also publishes is not
  part of this ruling: DP-052. While doing it, one thing was found that the old
  text hid: **no install route in this repository detects an existing `mpb`**
  (a cargo build produces both binaries unconditionally); the amended texts say so
  rather than promise it. **Owed, part B of pass 15:** the payload and its
  assertions in `build-release.sh`, the gate cases, the release record. **Owed,
  not scheduled:** a real detection for the routes that can run code (06's MUST
  now names it for a future route), and the `--help` `long_about` check.
- The original entry follows.

Raised 2026-10-10, pass 09, by `spec-guardian` (H2); raised to a charter
question 2026-10-11 by `spec-guardian`'s second review (B-H1).

**This is a Tier 2 question and needs your explicit decision.** The charter,
`docs/spec/00-charter.md` line 8, states the project's identity: "**CLI
command:** `macportsbrew`, with the short alias **`mpb`** installed alongside
it". `06-cli-and-ux.md` (Surface, MUST) says the same — "`mpb` is installed
alongside it as an equal short alias" — as do ADR-0003 and your DP-005 ruling,
with a conflict guard because MIT Photonic-Bands ships an `mpb`.

Pass 09's release design (D-B2) installs only `macportsbrew`: the bare file is
one executable; the `.pkg` carries no scripts, so it cannot run 06's "detect an
existing `mpb` and decline" guard at install time; and ADR-0005 already says
the `.pkg` does not create the alias. A **cargo build is different**:
`crates/cli/Cargo.toml` defines two `[[bin]]` targets, `macportsbrew` and
`mpb`, from one `main.rs`, so `cargo build` and `cargo install` do produce an
`mpb` today; the release script builds `--bin macportsbrew` only. Any wording
you approve must distinguish release artifacts from cargo builds.

- **(a) Release artifacts install `macportsbrew` only; cargo builds keep
  both.** Amend the charter line (for example: "`macportsbrew`; the short alias
  `mpb` is built by cargo and documented for release installs"), 06's MUST,
  ADR-0003, README, `wiki/Getting-Started.md` and the `--help` `long_about`.
  The install documentation tells a release user to add the alias by hand,
  after `command -v mpb` prints nothing (ADR-0003's guard, done by the user).
  Cost: for release users, `mpb` is documented rather than installed.
- **(b) The `.pkg` and `.dmg` also carry `mpb`.** The charter stands. Cost: the
  `.pkg` cannot run the conflict guard (no scripts, by design), so the guard
  rests on `PATH` order alone (an earlier `mpb` wins because the documented
  line appends); ADR-0005 is amended; the payload assertions grow an entry;
  the bare-file route still installs one name.
- **(c) `macportsbrew` installs its own alias on first run**, after the guard.
  The charter stands. Cost: a write the tool makes outside its state directory,
  which no current specification permits; a new feature, its own pass.

**Recommendation:** (a). **What it blocks:** the first tag, and every text
change about `mpb` (README, wiki, `long_about`, 06, ADR-0003, the charter).
**Reversible default taken:** **change no text, and block the first tag until
you rule.** The staged release artifacts install `macportsbrew` only, which is
what all three options produce for the bare file; nothing is published before
the ruling, so no published claim contradicts the charter meanwhile.

### DP-035 — Two names that cannot change after the first release: the identifier and the install directory — **RESOLVED 2026-10-23: identifier io.github.pabloguia.macportsbrew; directory ~/.macportsbrew/bin**

- **Ruled 2026-10-23 by the owner:** the recommendations. Identifier
  `io.github.pabloguia.macportsbrew` (option (b), following DP-031); install
  directory `~/.macportsbrew/bin` (option (a)). **Propagated:** the staged
  `build-release.sh` (new SHA-256 in the staged README) and the owner checklist;
  both are permanent once a release is published. The older pass 09 plan and
  report keep the earlier string as history.
- The original entry follows.

Raised 2026-10-10, pass 09, by `spec-guardian` (M7, L5); plan decisions D-S3
and D-P1.

1. **The code-signing and package identifier**, staged as
   `io.github.macportsbrew.macportsbrew` (derived from the workspace
   `repository` URL). `codesign`'s designated requirement and the macOS
   Installer's upgrade matching both key on it, so changing it after a release
   breaks upgrades and identity continuity.
   - (a) as staged; (b) `io.github.<account>.macportsbrew`, following DP-031;
     (c) a reverse-DNS name under a domain you control.
2. **The install directory**, staged as `~/.macportsbrew/bin` (the `.pkg`'s
   current-user-home target and the documented `cp` line). Once users have added
   the `PATH` line and Installer receipts exist, moving it strands them. It is a
   new top-level dot-directory in your users' homes, beside the existing state
   directory (`~/Library/Application Support/macportsbrew`) and SE-036's shim
   directory (`~/.local/share/macportsbrew/bin`).
   - (a) as staged; (b) `~/.local/share/macportsbrew/tool/bin` — keeps one
     MacPortsBrew tree under XDG, at the cost of a longer path and sitting
     beside the shim directory it must not be confused with; (c)
     `~/Library/Application Support/macportsbrew/bin` — rejected by the plan:
     it is the state directory (08 forbids) and has a space.

**Recommendation:** identifier — follow DP-031, which was resolved (b) on
2026-10-16, so `io.github.pabloguia.macportsbrew` (or a domain you control);
directory — (a). **What it blocks:** the first tag only.
**Reversible default taken:** both values as staged; neither is permanent until
a release is published.


### DP-031 — Which GitHub account the public repository lives under — **RESOLVED 2026-10-16: (b) the personal account**

Raised 2026-10-10, pass 09, by `spec-guardian` (L5). DP-007 decided "public
repository on GitHub" but not whose. The workspace `repository` field is
`https://github.com/macportsbrew/macportsbrew`; the release identifier (D-S3,
DP-035) is derived from it; the staged owner checklist tells you to create
`macportsbrew/macportsbrew`. Whether a GitHub organisation named `macportsbrew`
is available, or already yours, was **not checked** (it needs your account).

- **(a) An organisation `macportsbrew`**, if available. The URL matches the
  workspace field and the identifier needs no change. Cost: an organisation to
  administer (rulesets, environment, members).
- **(b) Your personal account**, `<you>/macportsbrew`. Cost: the workspace
  `repository`, README links and every `--signer-workflow` snippet change, and
  DP-035's identifier should not then claim `io.github.macportsbrew`.
- **(c) Another organisation name.** Same cost as (b).

**Recommendation:** (a) if the name is free; decide it before DP-035, because
the identifier follows it. **What it blocks:** creating the remote and the first
tag. **Reversible default taken:** the workspace value, unchanged; nothing is
published before you create the repository.

**Owed once this is ruled** (2026-10-11, `spec-guardian` second review, B-L5):
the release workflow's `sign` job gains `github.repository == '<owner>/<repo>'`
in its `if:`, so a fork's copy of the workflow cannot reach the signing
environment even by misconfiguration. It is a guarded workflow edit, made in
the same change as the workspace `repository` field if that changes.

- **Ruled 2026-10-16 by the owner, in a session:** (b). The repository is
  `pabloguia/MacPortsBrew`, created public with `gh repo create` the same day.
- **Propagated 2026-10-17:** the workspace `repository` field (`Cargo.toml`) now
  names it; the staged release workflow's `sign` job carries
  `github.repository == 'pabloguia/MacPortsBrew'` (the owed item above), in
  `docs/passes/09-staged/workflows/release.yml`, which is staged material and not
  yet a workflow; `docs/passes/09-staged/README.md` says the repository exists.
  No README link named the old URL. **Not propagated, because it is still yours:**
  DP-035's identifier, which is staged as `io.github.macportsbrew.macportsbrew`
  and by this ruling should become `io.github.pabloguia.macportsbrew` or a name
  under a domain you control.

### DP-038 — Personal data in the history and in the fixtures, before the first push — **RESOLVED 2026-10-16: (a) published as is**

Raised 2026-10-11 by `security-reviewer` (pre-release review, M2) and
`docs-auditor` (M2). HEAD is clean of the maintainer's account name, but:

- **Git history**: about 500 lines across eleven commits carry home-directory
  paths and the machine's layout (tool directories, a LaunchAgents listing, a
  download path). No secrets were found in history or at HEAD.
- **Author identity**: every commit is authored with a personal e-mail address.
- **Fixtures**: the committed recordings and the live snapshot carry this
  machine's `PATH` entries (with the home shown as `~`) and its full package
  list. The fixture README says this is deliberate; it is the owner's call to
  make knowingly.

Options: **(a)** publish as is; **(b)** before the first push, squash or
`git filter-repo` the history and rewrite the author e-mail to a GitHub
`noreply` address; **(c)** (b) plus re-deriving the fixtures with a generic
`PATH` (a *derived* fixture, its edits listed in the note).

**Recommendation:** (b) at least, because it cannot be undone after
publication; (c) if the tool list is felt to be personal. **What it blocks:**
the first push. **Reversible default taken:** nothing rewritten (rewriting
history is the owner's act).

- **Ruled 2026-10-16 by the owner, in a session:** (a). Asked before the first
  push whether to publish the history as it stands or to review it first, the
  owner chose to publish it as is, accepting what the entry above lists (home
  paths in about eleven commits, the personal author e-mail, the fixtures'
  `PATH` entries and package list). The history was pushed to the public
  repository the same day, so (b) and (c) are no longer available without
  rewriting a published history. No secret was found at the time of the review.

### DP-040 — Who reviews the Brazilian Portuguese catalogue, and whether it ships as is — **RESOLVED 2026-10-12: (a) ships as is, native review to follow**

- **Ruled 2026-10-12 by the owner:** option (a). `locales/pt-BR.toml` ships in 0.1.0 with
  every key translated, `error.*` and `doctor.*` included. A native speaker's review of the
  error and `doctor` sentences is still owed; it is tracked in the CHANGELOG ("reports are
  welcome") and `wiki/Contributing.md`. If a review finds a wrong remediation sentence, the
  per-key fallback means deleting that entry in `locales/pt-BR.toml` restores the English one
  with no code change. The deviation from the `add-locale` skill's refusal is accepted.

### DP-041 — The MacPorts-on-PATH wording `04` asks for, given INV-17 — **RESOLVED 2026-10-12: (b) direction-specific advice as dated data; IMPLEMENTED 2026-10-12 (pass 13, SE-098)**

- **Ruled 2026-10-12 by the owner:** option (b). The advice that the two directions are not
  equally risky (`04-migration-and-safety.md`, "Cross-provider contamination", F-2) is carried
  as **dated data** - each row with `source` and `read_on` - which a provider-neutral `doctor`
  rule selects by the affected provider's row, never by its name (INV-17, INV-20).
- **Implemented** in pass 13: `data/policy/cross-provider-advice.toml`, the `doctor` finding
  `cross_provider_exposure` (and `_unadvised`), catalogue text in both languages, tests with a
  mutant per branch, and the research note re-read (F-2 addendum, 2026-10-12). One thing the
  re-read changed: Homebrew's compiler shim does **not** drop `/opt/local` flags on every build
  (it does so only when refurbishing is on, which macOS does not turn on by default), so the
  Homebrew side of the advice is that the build's `PATH` is rebuilt, not that flags are
  filtered; `04` and F-2 were corrected. The ruling's substance (advice as data, neutral rule)
  is unchanged. What remains owed is listed in SE-098.

### DP-042 — Searching the other provider's whole catalogue for a match — **RULED 2026-10-12: (c) a sweep; not implemented, needs the owner's allowlist change**

- **Ruled 2026-10-12 by the owner:** option (c), searching the other provider's catalogue
  broadly rather than one package on demand. **This differs from the recommendation** ((b), a
  per-package lookup), and the reasons it was recommended against are carried here so the
  design meets them: a sweep reads thousands of packages, so it must be bounded, interruptible
  and cached; Homebrew's `info --json=v2 --eval-all` on an empty cache can download the formula
  API (so it is not offline, and offline behaviour must be defined: say so, never "no match");
  MacPorts' `port search` reads the local ports tree, which is only as new as its last sync
  (R-4), so results carry that date. **Not implemented**: it needs new provider subcommands in
  the process allowlist (`crates/adapters/src/process/`, `provider-commands.toml`) and new
  plan-read sites, which §3 of the pass-12 work order freezes (the process allowlist) and which belong to
  the track that reviews the process boundary with `security-reviewer`. **Binding, not a
  preference:** the charter's non-goal (`docs/spec/00-charter.md`) is that the tool must not
  require network access to run, so the sweep must work offline or say it cannot. Mapping stays "installed packages and
  the curated table" in 0.1.0; the plan still says "no equivalent found" - nothing was found,
  which does not mean none exists.

### DP-043 — A separate "as of" date for rows read from a local index — **RESOLVED 2026-10-12: (b), implemented**

- **Ruled 2026-10-12 by the owner:** option (b), done the same day. A curated row may carry
  `index_as_of` (`YYYY-MM-DD`, never later than `read_on`); the loader refuses a bad one, and
  staleness is measured from it. The 19 rows of 2026-10-12 carry `2026-09-02`, the older of the
  two index dates (homebrew-core 2026-09-02, MacPorts ports tree 2026-09-06), so `policyReadOn`
  is back to 2026-09-04 in every payload and the older facts no longer look newer. Policy
  version `2026.10.12-2` (the serial moves because no read date did). Evidence:
  `a_curated_row_ages_from_its_index_not_from_the_day_it_was_read` (two mutants killed: the
  field ignored, the not-after-`read_on` check removed); the goldens differ from the previous
  ones only in the version and `policyReadOn`.

### DP-044 — Do the pass-12 changes ship in 0.1.0? — **RESOLVED 2026-10-12: yes, in 0.1.0**

- **Ruled 2026-10-12 by the owner:** the pass-12 changes are part of the current
  version, 0.1.0, which has not been given to anyone. Nothing was published, so
  there is no reviewed-commit release to keep apart. `CHANGELOG.md` folds them into
  `[0.1.0]` (an empty `[Unreleased]` stays above it), and the pass-10 checklist's
  version step carries a note that the policy version is now `2026.10.12-1`.
- **Also ruled:** once the repository is on GitHub, CI will number each build with a
  sequential integer that never repeats. The binary side is done: `--version` prints
  `build <n>` when compiled with `MPB_BUILD_NUMBER` set (a positive decimal integer;
  anything else stops the build). **Owed, and the owner's to arm** because the files
  are not in place yet (staged in `docs/passes/09-staged/`, guarded paths): the release script and workflow must pass it, e.g.
  `MPB_BUILD_NUMBER="${{ github.run_number }}"` next to `MPB_BUILD_COMMIT`, and the
  signing record should carry it. `run_number` is sequential per workflow file and
  does not change across re-runs of one run; renaming the workflow file restarts it,
  so a counter that must survive a rename wants its own source.
- **To evolve when the repository goes to GitHub** (noted at the owner's request, 2026-10-12):
  1. pass `MPB_BUILD_NUMBER` in the release workflow (neither it nor `build-release.sh` is in the repository yet: both are
     staged in `docs/passes/09-staged/`, and putting them in `.github/workflows/` and
     `scripts/` is an armed write) and put it in the signing record;
  2. decide where the counter comes from (`github.run_number`, or its own source if the
     workflow file may be renamed) and say so in the release notes;
  3. at tag time: put the date on `## [0.1.0] - not yet tagged` in `CHANGELOG.md` and delete
     the line under it (an empty `[Unreleased]` is already above);
  4. before the first push: DP-031 (the account), DP-038 (personal data in the history and the
     fixtures) and DP-039 (`claude.yml`), then the settings listed under DP-007.

### DP-027 — A `brew.env` outranks the environment `brew` is given, and both ways to answer it spend something of the user's — **RESOLVED 2026-10-03: (b) for mutating steps, (c) for reads**

- **Ruled 2026-10-03 by the owner**, adopting the recommendation below:
  **(b) detect and refuse** before a mutating step, **(c) detect and report**
  on reads, and **not (a)**. To be implemented in pass 07
  (`../prompts/07-owner-rulings-and-housekeeping.md`, order of work item 6),
  recorded in SE-016.
- **Implemented 2026-10-04, wider than the recommendation's wording, and the
  owner may narrow it.** The recommendation named "a reviewed list" of keys;
  review showed a list lets keys R-15 itself flags (`SUDO_ASKPASS`,
  `HOMEBREW_XDG_CONFIG_HOME`, …) pass as "nothing found", so every key a
  provider's reader accepts now counts, and an unrecorded configuration is
  refused as an unknown. Its cost: any setting in a `brew.env` (a GitHub token,
  say) refuses a mutation against Homebrew, with `doctor` naming it. Narrowing
  means naming keys that do not outrank anything. SE-016 holds the detail.
- **The two questions above the specification are not settled by this
  ruling.** They are DP-028 (ruled 2026-10-27, SE-109), with the agent's recommendation.
- **Obligations carried:** the survey of keys that steer a mutating install
  (owed by the mutating pass); R-16 for MacPorts, answered 2026-10-03
  (`research/R-16-macports-user-configuration.md` — the user `macports.conf`
  overlays the system file key by key; `user.conf` is opened but accepts no
  key in 2.12.6); key lists and file locations as dated data (INV-20).

**As raised:**

Raised 2026-10-02 by the research item of pass 06
([`research/R-15-homebrew-brew-env.md`](research/R-15-homebrew-brew-env.md)).
`brew` reads up to three `brew.env` files — `/etc/homebrew/`, the prefix's
`etc/homebrew/`, and the user's (`$XDG_CONFIG_HOME/homebrew/` or
`~/.homebrew/`) — and `export`s every `HOMEBREW_*` line **after** the
environment it was started with, so a file beats what the process executor
sets; an empty value deletes a key (Homebrew 7.0.4 source, observed against a
temporary `HOME`, 2026-10-02). No variable switches the files off, and the
prefix file was user-writable on the Intel machine observed. For the reads this build
performs, a file can make them fetch API data (SE-050 item 6 accepted this on a
premise R-15 refutes; SE-016 item 3 records it as an open gap), switch analytics back on, and — through the
deprecated `HOMEBREW_NO_REQUIRE_TAP_TRUST` — load untrusted tap Ruby during
`info --installed`. For a mutating pass it is sharper: `install` and
`upgrade` run `brew update` first once a file blanks `HOMEBREW_NO_AUTO_UPDATE`.
Nothing an executor sets can win, so the answer is detection, relocation or
acceptance, and each costs the user something.

- **(a) Point `XDG_CONFIG_HOME` at an empty directory MacPortsBrew owns.**
  Suppresses the user file (observed). Moves Homebrew's tap trust store with
  it, so taps the user explicitly trusted read as untrusted to this tool's
  invocations; needs a state directory this build does not have; does nothing
  about the prefix and system files.
- **(b) Detect and refuse.** Read the three files read-only, in `bin/brew`'s
  order and with its line filter, before a Homebrew invocation (capture records
  it as a provider fact; the gate refuses to run a mutating step against it),
  when a line blanks a quieting key or sets one of a reviewed list
  (`HOMEBREW_FORCE_API_AUTO_UPDATE`, `HOMEBREW_CACHE`, `HOMEBREW_API_DOMAIN`,
  `HOMEBREW_CURLRC`, `HOMEBREW_NO_REQUIRE_TAP_TRUST`, `HOMEBREW_DEVELOPER`,
  the proxies, …). Covers all three files. Refuses users who configured those
  keys on purpose; the list is a denylist that must be re-read at each Homebrew
  version, with a check-then-use window.
- **(c) Detect and report, never refuse.** As (b), surfaced by `doctor` and in
  the plan's risks. Costs nobody a migration; protects nobody either.
- **(d) Accept, and say so.** Document that a `brew.env` governs `brew` and
  this tool does not override it. Its cost is a public one: a file that blanks
  `HOMEBREW_NO_ANALYTICS` turns Homebrew's own analytics back on for this
  tool's reads, where `06-cli-and-ux.md` ("no telemetry") and the README
  promise none. The argument that this is the user asking Homebrew, not this
  tool reporting, is yours to accept or reject.

**Whatever is chosen, three things are owed with it.** (1) The survey above is
of *reads* and of auto-update; the keys that steer a mutating install or build
— `HOMEBREW_CC`, `HOMEBREW_SVN`, `HOMEBREW_BOTTLE_DOMAIN`,
`HOMEBREW_ARTIFACT_DOMAIN`, `HOMEBREW_TEMP`, `HOMEBREW_MAKE_JOBS`, and any the
next version adds — bear on INV-10 ("or equivalents") and are not yet
surveyed (spec-guardian, 2026-10-02). (2) The question is not Homebrew's
alone: `port` reads `~/.macports/macports.conf` and `user.conf` under the
same real `HOME` (`macports.tcl` lines 1158 and 1202, MacPorts 2.12.6, cited
by spec-guardian 2026-10-02, not yet a research note — R-16). The mechanism is
framed as a provider capability, "provider configuration can outrank the
invocation", so neither direction becomes a special case (INV-17). (3) The
file locations and any key list are dated provider facts: they live in
`data/policy/*.toml` with `source` and `read_on`, never in a Rust constant
(INV-20).

**Two questions in this are above the specification, and only yours
(spec-guardian, end of pass 06).** The charter's non-goals say MacPortsBrew
does not "require … network access" and does not "send telemetry"
(`docs/spec/00-charter.md`, Non-goals): this tool sends nothing, but whether a
Homebrew read it runs fetching the catalogue on an empty cache, or reporting
Homebrew's own analytics because a `brew.env` switched them back on, is
consistent with those sentences is a charter reading, Tier 2. And INV-10 says
provider invocations run "in a controlled environment": R-15 shows the
environment `brew` runs with is ours overridden by up to three files, so
whether INV-10 is met by what this tool sets or must also cover what `brew`
reads is a Tier 1 question. The specification records both gaps and changes
neither sentence; the reversible default below keeps the build's behaviour
while you decide.

**Recommendation:** (b) for mutating steps, (c) for reads, and not (a) — (a)
silently changes how the user's own trust decisions apply, which is the kind
of side effect this tool exists not to have, and it still leaves two of three
files open. **What it blocks:** the mutating half of the executor relying on
the quieting environment; nothing in this build, which dispatches no mutation.
**Reversible default taken:** (d) for reads — behaviour unchanged, and the
specification now says the environment does not bind `brew` against a
`brew.env` (SE-016) — and nothing for mutation, because the gate refuses every
mutating step this build produces.

### DP-026 — What `migrate` answers on a machine where the other provider was not detected — **RESOLVED 2026-10-03: (b), exit 0 with a qualified answer**

- **Ruled 2026-10-03 by the owner**, adopting the recommendation below:
  **(b)**. `migrate` answers *nothing to move*, exit 0, on a package already at
  its destination when the other provider was not detected, and `migrate.v1`'s
  `nothingToMove` carries the qualification so a script can tell the two cases
  apart. Implemented in pass 07; recorded in SE-092. This also settles the
  INV-16 re-run question the paragraph below raised for the mutating pass.

**As raised:**

Raised 2026-10-01 by the independent review of the `migrate` slice (SE-092, S6
addendum). INV-16 says migrating a package already at its destination changes
nothing and says so. `migrate` answers *nothing to move*, exit 0, only when
every other provider was detected and read; on a machine with one provider —
a common one — a package that provider holds is refused instead
(`PROVIDER_PRESENCE_UNPROVEN`, exit 4), because detection looks only where a
provider is documented to live and one installed elsewhere would read the same.
Both answers change nothing. Which one a person and a script should get is an
audience trade-off, so it is the owner's.

- **(a) Keep the refusal, exit 4** (current). Never claims more than was
  established; a script on a one-provider machine sees "blocked" for something
  already done.
- **(b) Exit 0 with a qualified answer** — "nothing to move here; \<provider\>
  was not detected" — keeping the qualification in the text and in `--json`.
  Idempotent for scripts; the claim is only as strong as detection.
- **(c) A distinct exit code** for "done as far as can be seen". Precise, and a
  change to the documented exit-code contract of `06-cli-and-ux.md`.

**Recommendation:** (b), with the qualification a field of `nothingToMove` in
`migrate.v1` so a script can tell the two apart without parsing text.
**Reversible default taken:** (a) — behaviour unchanged.

**(a) cannot outlive the mutating pass without an INV-16 answer.** Once
`migrate --yes` can complete a migration, the source provider can be
uninstalled afterwards; a re-run on that machine then finds one provider and the
package at the destination, and under (a) refuses with exit 4 rather than
reporting done — the re-run INV-16 requires to change nothing and say so would
say "blocked" instead. So the mutating pass must either settle this decision
(for (b) or (c)) or give (a) a way to recognise its own completed run, such as
the history store's record confirmed against a fresh capture (SE-092, S8 "Owed
by the mutating pass"), before it ships.

### DP-025 — The charter's example of a surface that differs from the identity is now stale — **RESOLVED 2026-10-03: (a), reword the example**

- **Ruled 2026-10-03 by the owner**, adopting the recommendation below:
  **(a)**. `00-charter.md`'s example now reads "while a build plans migrations
  and executes none of them". Edited in pass 07 as the owner's explicit Tier 2
  decision; recorded in SE-092.

**As raised:**

Raised 2026-10-01 by `spec-guardian`'s review of the `migrate` slice (SE-092,
S6 addendum). `00-charter.md` gives, as the example of a release surface
differing from the identity, "while a build plans migrations and has no
`migrate` command". This build has a `migrate` command: it shows a plan, and
`migrate --yes` reaches a refusal gate that refuses every plan the build
produces. The rule the sentence states still holds — the surfaces say plainly
that the build executes nothing — only its example is out of date, and the
charter is Tier 2, which an agent does not reword.

- **(a) Reword the example** to "while a build plans migrations and executes
  none of them". Keeps the rule and its point; one clause.
- **(b) Drop the example**, leaving the rule unqualified.
- **(c) Leave it** until the mutating pass, when the clause stops being true in
  a second way and both can be fixed at once.

**Recommendation:** (a). **Reversible default taken:** (c) — the charter is
untouched, and `README.md`, the wiki and `plan.v1.md` were rewritten to state
what this build does.


### DP-024 — Where the executor lives, given that a new crate means editing two guarded scripts — **RESOLVED 2026-09-27: a module of the command-line crate**

- **Ruled 2026-09-27 by the owner**, who authorised the recommendations of the
  executor plan.
- **Options that were put:** (A) a new `crates/executor/`, as the work order
  proposed — which needs `scripts/check-layering.sh` (two lines) and
  `scripts/check-coverage.sh` (one key) edited, both guarded; (B) a module in
  the command-line crate, needing no guarded edit; (C) `application` with I/O
  through a trait, rejected because it contradicts INV-02's test and the work
  order; (D) `adapters`, rejected because that layer only reads.
- **Decision: (B)**, built so that moving to (A) is a directory move plus two
  manifest lines once the owner arms the guard for the two scripts. Every
  decision the executor makes is a pure function in `application`; the module
  holds only the I/O shell. The rationale and what (B) strains are in SE-092.

### DP-023 — How the executor turns a plan command into something it may run — **RESOLVED 2026-09-27: re-derive inside `process/`**

- **Ruled 2026-09-27 by the owner**, on the question `prompts/05-the-executor.md`
  §5 left open as that pass's item 1: `CommandSpec::provider` takes the
  executable as `&'static str` on purpose, and `PlanCommand.program` is a
  runtime `String`, so no legal path existed from a plan to a spawn.
- **Options that were put:** re-derive the program inside `process/` from the
  detected prefix and a literal, taking only the argv from the plan; add a
  constructor in `process/` that accepts a plan command (a change to a module
  `AGENTS.md` calls dangerous, which must never take `PlanCommand.environment`
  verbatim, INV-10); or leave it for a dedicated pass.
- **Decision: re-derive.** Nothing outside the source chooses which binary runs,
  which keeps the guarantee `CommandSpec::provider` exists for. The plan
  supplies argv only, and that argv still passes the spawn-site allowlist
  (SE-050: read subcommands, argument shape). A plan whose `program` disagrees
  with the re-derived one is a refusal cause, not a correction. The cost the
  work order named stands and is accepted: the executor runs a command it
  reconstructed rather than the exact document the human reviewed, so the
  refusal on disagreement is what keeps INV-02 honest.
- **Recorded in:** `prompts/05-the-executor.md` §5, which now states the ruling.
  The executor pass is not started by this entry. It answers §5's question
  only; §6's item 1 — whether `migrate` re-plans in process or deserialises a
  plan — is a separate design decision, taken in the executor pass itself.

### DP-022 — The guarded-path escape is a filter, and both ways to make it a boundary spend your workflow — **RESOLVED 2026-09-27: keep the filter**

- **Ruled 2026-09-27 by the owner: keep the filter.** Neither boundary is
  taken; prompt-arming stays, narrowed as SE-081 and F-52 left it. The residual
  is therefore accepted rather than open: the escape is a filter against the
  demonstrated accident — a subagent's report quoting the token — and not a
  boundary against a session that means to arm it. F-52 is closed on this
  ruling. The hook headers that point here (`arm-guard-edit.sh`,
  `test-hooks.sh`) still read "filed rather than chosen"; they are guarded
  paths and are corrected the next time the owner arms the guard for them.
- **Filed 2026-09-20, from F-52, which is the second time this escape armed
  itself.** Both times a reviewing subagent quoted the specification that
  describes the escape, and both times the hand-back reached the hook through
  the same prompt channel a human message uses. The first narrowing (token at
  column zero, because "the harness indents relayed output") rested on a
  convention this repository does not own, and a second relay shape did not
  honour it.
- **What landed instead, and what it is worth.** The token must now be the
  **first thing on a line**, and `test-gates.sh` asserts that no line in the
  tracked corpus begins with it — 33 matching lines today, none opening with the
  token. That narrows the accident onto something the repository controls, and
  reviewers established the same day that it is **narrower than it first read
  here**: the assertion covers *tracked source lines*, so it cannot see the
  git-ignored marker — whose third line now begins with the token by
  construction — and it says nothing about a quotation that *reformats* into a
  fenced block, a list item or a diff hunk. It is also detective rather than
  preventive, running at `Stop` time. So the filter is real and it is
  **still a filter**: text that did not come from
  here — a paraphrase, a commit message, a page on the web — can put the token
  at the start of a line, and nothing readable can stop a session that means to
  arm itself (SE-081).
- **The two routes that would be boundaries.**
  1. **Withdraw prompt-arming; the environment variable only.** A session
     cannot forge its own environment, so this is an actual boundary. The cost
     is the workflow you used twice on 2026-09-19 and 2026-09-20: no arming
     mid-session, every guarded edit needs a session started with the variable
     set.
  2. **A per-session nonce.** `session-start.sh` generates a random value and
     shows it to you; arming requires the token with that value. A subagent
     quoting documentation can never produce it, because it is in no document.
     It does **not** stop a determined session — the agent can read the nonce —
     so it closes the demonstrated accident and not the deliberate act. The cost
     is that you copy a value instead of typing a fixed string.
- **Why it is yours:** both spend how you work, and the reversible default is
  the one that does not. `AGENTS.md` reserves that class of trade for you.
- **The reversible default, taken:** the filter above, with its limits written
  where the rule is implemented rather than in a summary of it.

### DP-019 — DP-008 was decided without its cost, and the cost is a second certificate plus a billed runner — **RESOLVED 2026-09-27: option (a), all three artifacts**

- **Ruled 2026-09-27 by the owner: option (a).** The `.app`, the `.pkg` and
  the Intel build all ship, and the second certificate and the Intel runner are
  costs chosen rather than inherited. DP-008 stands as decided. Still open
  inside it, and a scheduling question rather than an audience one: whether
  Intel builds run per commit or per release. **Default meanwhile: per
  release**, which is the cheaper of the two and reversible. Nothing is billed
  until a remote exists (DP-007).
- **Premise corrected 2026-10-11 (pass 09): the Intel runner is not billed.**
  R-19 finding 3 (GitHub Docs and `actions/runner-images`, read 2026-10-10)
  shows that `macos-15-intel` and `macos-26-intel` are **standard** runners,
  free and unlimited on public repositories; the billing tier follows the
  label, and only `-large` / `-xlarge` are billed (R-19 finding 17). R-11 read
  `-intel` as an alias of the billed `-large` runner, and this entry inherited
  that. **The ruling stands** — all three artifacts and the Intel build ship;
  only one of the two costs it accepted turns out to be zero. The second
  certificate (Developer ID Installer) is still a real requirement. The pass-09
  release workflow builds and signs on `macos-26-intel` and `macos-26`, so the
  per-commit / per-release question no longer has a cost attached. Separately,
  the ruling's ".app" is the `.dmg` (no `.app` exists in any design; DP-036
  ruled (a) on 2026-10-27, and no application bundle is planned).
- **Reopened 2026-09-16 on the owner's ruling**, after R-11 established two
  numbers DP-008 did not have when it chose to publish all three artifacts —
  bare binary, `.pkg` and `.dmg` — unconditionally.
- **The two costs.** The `.pkg` needs a **second certificate**: signing an
  installer package uses a Developer ID Installer certificate, which is not the
  Developer ID Application certificate the binary and the `.dmg` use. And the
  **Intel runner is billed even on a public repository**, so the macOS Intel half
  of the release matrix has a recurring cost that the free-for-public-repos
  assumption did not cover.
- **Why it is yours:** it spends money, and it was decided once already on the
  strength of not knowing that. `AGENTS.md` names "anything that spends their
  money" as the owner's.
- **What it does *not* reopen.** SE-067's reasoning stands on its own: a `.pkg`
  buys the installation experience, which no R-11 outcome touches. The question
  is not whether the `.pkg` is worth shipping but whether it is worth *these two
  costs*, which is a different question and was never asked.
- **Options:**
  (a) **Keep all three.** Pay for the second certificate and the Intel runner.
  Costs money on a schedule; keeps the audience DP-008 chose.
  (b) **Drop the `.pkg`.** Removes the second certificate entirely. Costs the
  installation experience for users who want a double-click install, which is
  precisely what SE-067 said the `.pkg` was for.
  (c) **Keep the `.pkg`, drop the Intel runner** and publish Intel builds only
  when someone asks. Costs the Intel timeline promise the charter leans on
  (INV-20), and Homebrew's Intel Tier 3 status is the project's founding
  argument — this is the one that trades away audience.
  (d) **Defer.** No remote exists, so nothing is published and nothing is billed
  yet. Costs nothing today and costs a reversal later if the answer is (b).
- **Recommendation: (a), decided before the first release rather than at it.**
  The project's founding argument is about Intel users; (c) trades away exactly
  the people it was built for, and (b) removes the artifact SE-067 justified on
  its own merits. What changes is that the cost is now known and chosen rather
  than inherited. If the recurring runner cost is the sticking point, the
  narrower question is whether Intel builds must be *per-commit* or can be
  *per-release*, which is a scheduling answer rather than an audience one.
- **What it blocks:** nothing today. There is no remote (DP-007) and no release.
- **Default meanwhile:** DP-008 stands — all three artifacts — because it is
  what is documented and because reversing it costs nothing while nothing ships.

### DP-021 — A provider installed under the home directory can be analysed and can never be planned, and closing that costs either a privacy rule or an audience — **RESOLVED 2026-09-27: option (b) now, (c) as the eventual fix**

- **Ruled 2026-09-27 by the owner: (b) now, (c) as the shape of the eventual
  fix.** (b) is complete — `doctor` warns first, the refusal names the cause and
  points at `doctor`, and both directions are tested — so a home-installed
  provider is analysed and never planned, and says so honestly. (c), carrying
  the un-redacted prefix only in the in-memory plan and never in a written
  document, is the agreed design for whoever makes such a machine plannable;
  it needs capture to keep the removed value, which it does not today. (a) is
  rejected: it spends the privacy MUST at three exits instead of one. The
  smaller question — the error identifier — was not ruled on; **default
  meanwhile: accept the mismatch**, the reversible option, with the detail
  sentence carrying the correction. F-48 is closed on this ruling.
- **Raised 2026-09-19**, filing the design question F-48 has carried open since
  2026-09-16. F-48's other half — the curated table judged by a guard that
  blamed the user — was fixed that day; this half was left as "a design
  question" and was never written down as a decision, which is the gap this
  entry closes. `AGENTS.md` names "anything that trades away part of the
  audience" as the owner's, and both routes out do exactly that or spend a
  privacy rule to avoid it.
- **What is true today.** Capture redacts a prefix under `$HOME` to `~/homebrew`
  on **every** route, live capture included (`capture.rs:102-104`, before
  `Snapshot::check`). The domain boundary passes that shape deliberately
  (`snapshot.rs:243-265`), and `planner::command` then refuses it, because
  `Command::new` resolves a relative string against the working directory —
  the difference between running the provider and running whatever is under the
  directory the user happened to be in (`planner.rs:674-682`, exit 8).
- **Who it strands, and in which direction — both of them.** `ADD_DESTINATION`
  and `VERIFY_DESTINATION` build their commands from the **destination** and
  `REMOVE_SOURCE` from the **source**, so a home-installed provider is refused
  whether it is where the package is going or where it is coming from.
  `analyze` exits 0 and is correct; every `plan` touching that provider exits 8.
  **This entry first said "`plan --retain-source` exits 0", which is true only
  when the home-installed provider is the source** — `--retain-source` drops
  `REMOVE_SOURCE` and `VERIFY_REMOVAL` and nothing else, so it cannot rescue a
  home-installed *destination*. Found 2026-09-19 by `spec-guardian`, which also
  noted that the regression test covered only the away direction: one direction
  covered and one not, on a path both reach, is the INV-17 shape this project
  names as a trap. `a_home_redacted_destination_is_refused_too_and_retain_source_does_not_rescue_it`
  now covers the other, and the code was symmetric all along — the refusal names
  no provider. What was missing was the test, which is the harder half to see.
  The audience is
  specifically users who installed Homebrew under `$HOME` because they have no
  administrator rights — managed and corporate machines — which is a population
  that overlaps the Intel users INV-20 and the charter are written for.
- **Options:**
  (a) **The plan carries the un-redacted prefix, and redaction becomes a
  display-time concern.** What the user actually wants. It collides with the
  Tier 3 **MUST** at `06-cli-and-ux.md:133` — read that line before pricing
  this option, because **it is narrower than this entry first made it sound**.
  In full it governs one artefact: "`doctor --report` **writes a redacted local
  diagnostic file** … Never: usernames, home paths…". Extending it to snapshots
  and plans is a reading the codebase already takes by analogy
  (`capture.rs:96-104` argues it), and it is a reading rather than the text.
  That matters here more than anywhere, because option (c) below turns on
  exactly this axis, and the extrapolation is what prices (a) above it. Also,
  if the prefix were re-supplied from this host at plan time, it collides with
  with INV-15 — `invariant_15_a_replayed_or_snapshotted_run_never_redacts_against_this_host`
  forbids exactly that. So it is not a small change: it touches the snapshot
  schema, `capture.rs`, `planner.rs` and the INV-15 tests, and it needs a
  spec-evolution entry arguing why a home path in a plan document is acceptable
  where the spec says it never is.
  (b) **The refusal says what is true, and `doctor` says it first.** Smaller and
  honest. Costs the audience above permanently, or until (a) is taken.
  (c) **Carry the prefix only in the in-memory plan and never in a written
  document.** Narrower than (a): `plan --json` and any snapshot still redact,
  so the MUST is kept where it is about *output*. It needs the un-redacted value
  to survive capture, which today it does not — `prefix_redacted` records only
  that redaction happened, not what was removed.
- **Recommendation: (b) now, (c) as the shape of the eventual fix.** (b) is
  already three-quarters taken and costs nothing to complete. (c) is the version
  of (a) that does not spend the privacy MUST, because that MUST is about what
  the tool *writes*, and a plan step that runs on this machine is not a document
  until it is serialised. What makes (a) tempting and wrong is that it reads as
  "just stop redacting"; what it actually is, is moving a rule that currently
  holds at capture to somewhere it has to be re-enforced at three separate exits.
- **What it blocks:** nothing. The tool analyses these machines correctly today.
- **What has already landed, and needs no ruling.** These are (b)'s completion,
  all reversible, and none forecloses (a) or (c). Listed separately from the
  question so a reader can tell what is awaiting you from what is done:
  1. `doctor` warns before the user reaches a plan (2026-09-16).
  2. The refusal's remediation points at `doctor` (2026-09-16).
  3. The refusal itself names the cause instead of describing the symptom
     (2026-09-19), and a test covers the destination direction as well as the
     source one.
- **What is awaiting you:** only the choice between (a), (b) and (c) — plus one
  smaller question it makes sense to rule on at the same time.
- **The smaller question: the error identifier.** The refusal still answers
  `ErrorId::InputUnusable`, whose catalogue string ends "if the input came from
  a snapshot file or a recording, check whatever produced it" — so the envelope
  still names the user's input as the suspect while the detail says the opposite.
  `spec-guardian` proposed `CapabilityMissing` on the grounds that it is "the
  same exit code 8"; **it is not — it is 5**, and `06-cli-and-ux.md` makes exit
  codes a documented contract, so that swap is a contract change and not a
  wording fix. Rewording `error.input_unusable` is not available either: its two
  other callers are genuinely about the input. The options are to accept the
  mismatch, to move this path to exit 5, or to add an identifier for "this build
  cannot express a step for this machine", which is a category the table does not
  currently have. Meanwhile the detail carries the correction in the sentence the
  user actually reads, which is the reversible half.

### DP-020 — The weak half of the guard-edit escape can manufacture the strong half, in a file no diff shows — **RESOLVED 2026-09-17: option (c), deny the git-ignored half and leave the committed one openable**

- **Raised 2026-09-17 by `spec-guardian`, reviewing SE-081, and reproduced.**
- **The question:** should `*/.claude/settings.json` and
  `*/.claude/settings.local.json` move into `guard-protected-paths.sh`'s
  **unconditional** deny list, beside the escape's own marker and log?
- **Why it is not already obvious.** Today both sit *inside* the escape-guarded
  `case`, so a session that is armed may write them. A settings file's `env`
  block reaches hook processes. So a session armed by the route SE-081 calls
  detection — a prompt, which an agent can forge with one `Bash` redirect — can
  write `"env": {"MPB_ALLOW_GUARD_EDIT": "1"}` into `.claude/settings.local.json`
  and arm **every session after it** by the route SE-081 calls prevention: no
  marker, no prompt, no author, and log lines that read "the
  MPB_ALLOW_GUARD_EDIT=1 environment variable". `settings.local.json` is
  git-ignored, so none of it appears in a diff. Verified against the current
  hook: a `Write` to that path under an armed marker returns no denial.
- **Why it is yours.** Both routes out cost something real, and the cost is the
  kind this project makes the owner pay rather than an agent.
  - **Deny unconditionally.** Closes it. But `settings.json` is how this
    session's own hook was registered, and an agent could then never change
    either file through `Edit` — every settings change becomes a `Bash` write
    (unguarded, so the gap merely moves) or a task for you outside a session.
  - **Leave it, documented.** No friction, and the escalation stays reachable.
    It is at least *recorded* now, in SE-081 and here, and the disclosure log
    catches the write that starts it — provided anyone reads the log.
  - **Deny `settings.local.json` only.** It is git-ignored personal state with
    no legitimate in-session need, and it is the file the escalation actually
    wants. `settings.json` stays editable. Narrower, and it leaves the same hole
    one file over.
- **Recommendation: the third option.** What makes this dangerous is not the
  write, it is the *invisibility*. A change to `settings.json` is committable
  and shows up in review — it is a change that ought to be reviewed, and it
  still can be. A change to `settings.local.json` appears nowhere. Denying the
  git-ignored one removes the invisibility without removing the legitimate
  capability, and registering a hook — the thing option (a) would cost — stays
  possible and stays visible. Stated plainly so it is not over-read: this closes
  the accidental route and the Edit/Write route, and closes nothing else.
  `Bash` writes either file regardless, since it never reaches this guard, and
  no option here changes that.
- **The default taken while this was open, and it was the recommendation's
  opposite:** nothing was changed in the deny list, because documenting first
  and tightening later is reversible, while tightening first and discovering an
  agent cannot register a hook is a session you have to rescue. Reproduced
  twice, once by `spec-guardian` and once directly: under an armed marker a
  `Write` to either settings file returned no denial, while the marker itself
  was refused.
- **A cheaper mitigation worth ruling on at the same time:** the `Bash` guard
  has no path rule for settings files at all (checked: 105 deny rules, none
  naming `.claude`). A `Bash` deny would not close the `Edit` route above, and
  the reverse is also true — they are two separate holes into one file.

- **Resolved 2026-09-17 on the owner's ruling**, as recommended.
  `*/.claude/settings.local.json` moved into `guard-protected-paths.sh`'s
  unconditional deny list and out of the escape-guarded one, so it is refused
  whether the escape is armed or not, and by either arming route.
  `*/.claude/settings.json` stays in the escape-guarded list: a change to it is
  committed and reviewable, and registering a hook is something a session
  legitimately does. The asymmetry is the ruling, so `test-hooks.sh` asserts
  both halves rather than only the denial.
- **What it does not do, recorded so the next reader does not over-read it.**
  It closes the accidental route and the `Edit|Write|NotebookEdit` route.
  `Bash` never reaches this guard, so it writes either settings file exactly as
  it can write the marker; no arrangement of this deny list changes that. The
  claim in `arm-guard-edit.sh` is rewritten to say which half is closed.

### DP-016 — Under replay, no package is known to be a service, and `MANUAL_REVIEW` will not fire — **RESOLVED 2026-09-16: option (c), refuse the plan rather than weaken the rule**

- **Note, 2026-10-10 (pass 08 slice 3).** The refusal still applies to every
  replay. On a **live** capture it now stops when MacPorts' service scan gives a
  sound known answer — which the ruling anticipated ("stops triggering on its own
  once real service data exists"). Before, the scan answered unknown on any
  machine with no `<prefix>/etc/LaunchDaemons`, the ordinary state, so MacPorts
  could not be a plan's source live either. The scan is now fail-closed in the
  other direction too: any launchd item it cannot attribute, an inactive port, or
  an unusual tree makes the answer unknown (`docs/passes/08-slice-3-plan.md`, C′).
  What "defines a service" means for MacPorts is DP-032.

- **The question:** F-29. `macports::service::scan` answers `Unreadable`
  wherever the running host is not the machine being described — which is every
  `--replay` run, by design (INV-15, F-25). `defines_service` renders that as
  `false`, and `classify.rs` reads `false` as *not a service*. So no
  `PackageClass::Service`, and **INV-19's `MANUAL_REVIEW` does not fire on a
  replayed service package.** The adapter already reports
  `Capability::ServiceMetadata: Unknown`, which is the correct answer, and a
  grep across `application`, `policy` and `cli` finds **no reader** — re-checked
  2026-09-06. An unknown nothing distinguishes from a no is an unknown in name
  only, which is the exact shape `.claude/rules/adapters.md` forbids in the
  sentence "an unknown is never a yes".
- **Why the field is wrong regardless of the ruling:** `AGENTS.md` states the
  principle already — "`ArtifactAvailability` (`UNKNOWN` is a first-class
  answer, never a silent 'no')" — and `defines_service: bool` cannot hold it.
  Making it a tri-state implements a rule this project already has and is not
  what this entry is asking. **The question is only what `Unknown` should
  *do*.**
- **Why it is yours:** it trades a safety property against the usefulness of the
  feature that exists to demonstrate safety. Every option below is defensible and
  none is reversible cheaply once `--replay` output is something people rely on.
- **Options:**
  (a) **`Unknown` forces `MANUAL_REVIEW`, like a known service.** Safe and
  simple. Cost: *every* replayed MacPorts plan becomes `MANUAL_REVIEW`, so
  `--replay` can no longer show what a real plan looks like — and `--replay` is
  how a contributor without MacPorts exercises the planner at all.
  (b) **`Unknown` does not force review, but the plan says the status is
  unknown and which packages it could not classify.** Keeps replay useful;
  leaves the INV-19 hole open, now labelled instead of silent.
  (c) **Refuse to build a migration plan at all from a recording whose service
  metadata is unknown**, while `analyze`, `status` and `explain` keep working.
  The hole cannot be reached because the path that needs the answer is closed.
  (d) **Record service status in the recording itself**, so a replay of a
  machine that *had* MacPorts carries real data and `Unknown` means only "this
  recording predates the field". Best long-term, largest change, and it makes
  every existing fixture stale.
- **Recommendation: (c), then (d).** (c) is the honest reading of INV-01's
  spirit — do not produce a plan you cannot verify the safety of — and it costs
  only the one command that actually depends on the missing fact, where (a)
  costs all of them and (b) keeps a Tier 1 invariant unenforced by design. (d)
  is where this should end up, and (c) does not block it: when recordings carry
  service data, the refusal stops triggering on its own.
- **Default meanwhile:** unchanged. `Unknown` reads as `false`, `MANUAL_REVIEW`
  does not fire under replay, and it is written down in four places — F-29,
  `filesystem.rs`'s type documentation, `ServiceScan::Unreadable`'s comment, and
  here. Nothing executes plans in this build, so the hole cannot yet cost
  anything; it becomes live with the executor.

---


---

**The owner's ruling, given in session:** option (c) — refuse to build a
migration plan from a recording whose service metadata is unknown, while
`analyze`, `status` and `explain` keep working.

**What it forces.** The hole INV-19 leaves under replay is closed by closing the
one path that needs the missing fact, rather than by making every replayed
MacPorts plan a `MANUAL_REVIEW` — which (a) would have done, and which would have
cost `--replay` the thing it exists to demonstrate. (d) stays the long-term
answer and is now unblocked by F-49's ruling: once a recording carries service
status, `Unknown` means "this recording predates the field" and the refusal
narrows to those.

**Propagated:** the refusal, its error identifier and its test.

### DP-017 — Three of this repository's own guards make one security property untestable — **RESOLVED 2026-09-16: option (d), assert the constants**

- **The question:** F-31 hardens the cask-receipt open with
  `O_NOFOLLOW | O_NONBLOCK`. The behavioural test for that — plant a symlink,
  watch the strict open fail — cannot be written anywhere in this repository,
  and the reason is a collision between three guards that are each individually
  right.
- **The collision, verified against each script rather than argued** (a first
  version of this list named the wrong script for one of them, which is why
  each now cites a line):
  1. `stop-gate.sh` refuses the token `symlink` in non-comment code anywhere
     under `crates/*/src`. `#[cfg(test)]` is caught **by the path glob**, not by
     a decision: the hook's stated reason (`stop-gate.sh:83-87`) is that Rust
     compiles a `src/tests/` *directory* into the shipped binary, which is not
     true of `cfg(test)` and which the hook never mentions. Its message says a
     fixture like this "belongs under `crates/<crate>/tests/`".
     It also fires on an **assertion message** — English prose that happens to
     sit on a code line — a false positive already met once while writing this.
     Rewording that message was legitimate (it names bits now, which is what it
     asserts); rewording a *call* to dodge the same grep would not be, and the
     two look identical from outside the diff. That ambiguity is itself an
     argument for scoping the guard rather than living beside it. The pattern
     recurred twice more the same day: the path guard refused a shell command
     for quoting the pinned-toolchain filename in prose, and refused another for
     naming a provider verb inside a fixture. Each was met with the documented
     remedy — put the content in a file — but three false positives in one
     session is a rate, not an accident.
  2. It cannot go there: `read_bounded` and `OpenPolicy` are `pub(crate)`, and
     reaching the flags through the public `cask_requested` proves nothing —
     the `symlink_metadata` pre-check refuses the symlink first, with the flags
     or without them. A test that passes either way is worse than none.
  3. The FIFO half — the one this finding is *named* after — needs `mkfifo`,
     for which `std` has no equivalent (`UnixListener::bind` makes a socket
     node, whose `open` returns `ENXIO` at once and so tests nothing about
     blocking). Spawning it is caught by `check-no-shell.sh:150` — but that
     matches `Command::new("` **in its literal form only**, and the script's
     own comment concedes as much ("a literal in program position is the
     signal"). A `const` holding the path walks past it. So this door is
     alarmed, not locked, and the alarm is the point: routing around it is the
     violation, not the grep's incompleteness. `libc` is refused by ADR-0006
     and, more strongly, by `unsafe_code = "forbid"` in the workspace manifest.
     A **safe** wrapper crate (`nix::unistd::mkfifo`) evades both and is named
     here only to be rejected: a dependency acquired to test one flag is worse
     than every option below.
- **What ships in the meantime:** the flag *set* is asserted
  (`open_flags(RefuseSymlink) == 0x0104`), which catches the specific
  regression the security review named — dropping the `O_NONBLOCK` term while
  every other test stays green. What is **not** asserted is that the kernel
  honours it. The code says so in the test itself rather than implying
  coverage it does not have.
- **Why it is yours:** every route out weakens a gate or widens an API, and
  `AGENTS.md` is explicit that a hook blocking something legitimate "is a
  question for the human, not an obstacle to route around".
- **Options:**
  (a) **Leave it.** The residual is one untested flag on a low-severity,
  high-precondition path, disclosed in three places. Costs nothing; the flag
  can regress silently in its *effect*, though not in its presence.
  (b) **Narrow the `stop-gate.sh` scan** to exempt `#[cfg(test)]` blocks, or to
  exempt `std::os::unix::fs::symlink` specifically. INV-06/INV-19 are about
  *creating* symlinks in shipped code; a test creating one in a temp directory
  is not that. Cost: the guard's current strength is that it is not
  path-scoped, and every exemption is a thing the next reader must re-derive.
  (c) **Make `bounded_read` `pub`** so the test can live in
  `crates/adapters/tests/`. Cost: widening a security primitive's API surface
  for a test, which is the tail wagging the dog.
  (d) **Assert the constants instead of the behaviour.** A
  `scripts/check-open-flags.sh` that re-derives `0x0100` and `0x0004` from
  `$(xcrun --show-sdk-path)/usr/include/sys/fcntl.h` and compares them with the
  literals in `bounded_read.rs`, skipping where `xcrun` is absent. No gate
  weakened, no API widened, no dependency, no FIFO. It does **not** test that
  the kernel honours the flags — but it closes the wrong-integer failure mode
  entirely, which under (a) rests on a dated comment and one manual check. This
  repository's stated preference is "give the agent a way to check its work"
  over "tell the agent to be careful", and this is that. Raised by the second
  security review; it is not implemented here because adding a stage to
  `validate.sh` is a change to the one definition of valid.
- **Recommendation: (d) now, (b) if the behavioural test is wanted.** (d) is
  cheap, additive and closes the failure mode most likely to actually happen —
  a constant going stale. It does not buy the behavioural test; only (b) does. It is the only one that
  buys the test, and the exemption can be written as a named allowance with its
  reason, which is what the other guards in this repository already do. Both
  edit `scripts/` or `.claude/hooks/`, and this session does not edit gates
  without you.

**The owner's ruling, given in session:** option (d) — `scripts/check-open-flags.sh`,
re-deriving the constants from the SDK header.

**What it does not buy, stated because the entry above argues it.** It does not
test that the kernel honours the flags; only (b) would, and (b) was not taken.
What it closes entirely is the wrong-integer failure mode, which until now rested
on a dated comment and one manual check — and which is the one that actually
happens, because a constant goes stale without anything moving.

**Blocked on the guard.** The script and its `validate.sh` stage are both under
`scripts/`, which the path guard protects; it lands in a session started with
`MPB_ALLOW_GUARD_EDIT=1`, and that use is disclosed in the pass report.

### DP-018 — A second agent harness ran under a weaker copy of the enforcement layer — **RESOLVED 2026-09-08: `.claude/` is the source, `.codex/` is generated from it**

**The owner's ruling, given in session:** *"o principal é o .claude, gerar o
.codex a partir dele. como é opensource, o dev escolhe"* — option (a), not the
(b) this entry recommended.

**The reason is better than the recommendation it overrode.** (b) would have
pointed both manifests at one shared set of scripts, which makes divergence
structurally impossible but leaves a `.codex/hooks.json` reaching into a
directory named for a different tool. This repository is open source and the
developer picks their harness; somebody who picks Codex should get a complete,
self-contained layer. Generation costs a script and a check; that is the price
of the property.

**Implemented:** `scripts/generate-codex-layer.sh`, with `--check`.

What generating turned up, none of which was visible from the outside:

- **The hook scripts contain nothing Codex-specific.** Not one occurrence of the
  string, checked. `mpb_deny` and `mpb_context` were byte-identical in both
  trees. So `.codex/hooks/` was never an adaptation — it was a **snapshot of
  `.claude/hooks/` that stopped being updated**, and every "Codex-only" line was
  an older version of a Claude line: the `cd || exit 0` fail-open, the silent
  `MPB_SKIP_STOP_GATE`, the identity-function `mpb_realpath`, the conflated
  empty-decisions branch.
- **`spec-guardian` was missing entirely** from `.codex/agents/` — the one
  subagent that checks invariants. Three of four, and the absent one was that.
- **`tools:` and `disallowedTools:` do not survive the translation.** They are
  what make all four reviewers read-only under Claude Code, and the Codex TOML
  format in this repository carries no equivalent. Codex agents *do* appear to
  take a `sandbox` setting, but the only source found for that was secondary, so
  **it is not written**: a security-relevant key from an unverified reading is
  exactly what `AGENTS.md` forbids. The generated files say so in their own
  header, and confirming it is a `tooling-recheck` for Codex — which does not
  exist, see below.
- **`.agents/skills/` was a blind find-and-replace of the product name**, and it
  shows: `.Codex/` with a capital C in three files — a directory that does not
  exist and only appears to work because macOS is case-insensitive by default —
  and `https://code.Codex.com/docs/`, a domain that does not resolve. Renaming a
  proper noun cannot tell a path from a prose word from a hostname. The
  generator substitutes **paths only**, and names `settings.json` explicitly
  because the two harnesses do not spell their configuration the same way.
- **One skill is deliberately not generated.** `tooling-recheck`'s subject *is*
  Claude Code's field names and hook contracts, verified against Claude's
  documentation. A Codex equivalent has to be written against Codex's
  documentation; deriving it is precisely how that broken URL got there.

**Two things this ruling still needs, and both are blocked on
`MPB_ALLOW_GUARD_EDIT=1`:**

1. **The check is not wired into `validate.sh`.** Until it is, the generator
   exists and nothing runs it — which is the same shape as the drift it was
   written to stop. One line: `step "codex layer"; run
   scripts/generate-codex-layer.sh --check`.
2. **`.claude/`'s path guard still does not cover `.codex/`.** The generator
   itself demonstrated the gap: it wrote the whole tree with no guard objecting.
   With `--check` wired in, a hand-edit is caught rather than prevented, which is
   adequate — but the asymmetry should be closed on purpose rather than left as
   an accident.


### DP-015 — The coverage floors had never been measured — **RESOLVED 2026-09-06: option (a), with the floors raised**

**The owner's ruling, given in session:** install the tool — "pode instalar a
ferramenta, tem minha autorização" — and hold **90% overall**, with **95% for
`domain`, `policy`, `adapters` and planning**. That is option (a) plus a change
to the numbers themselves: `adapters` rises from 80 to 95, and a workspace-wide
backstop is added where none existed. `cli` is deliberately not given a floor;
it is covered by the 90%.

**Measured the same day, for the first time in this repository's history:**

| Crate | Lines | Old floor | New floor |
|---|---|---|---|
| `domain` | 96.65% | 95 | 95 |
| `policy` | 97.46% | 95 | 95 |
| `application` | 95.27% | 95 | 95 |
| `adapters` | 95.29% | 80 | 95 |
| `cli` | 88.82% | none | none |

Overall **94.86%** (11300/11912). Every floor the owner set is met, and
`adapters` cleared its raise with no work — it was already at 95.29% under an
80 floor, which is the pleasant half of never having measured.

The unpleasant half: **`application` was at 89.47%**, under a floor its own gate
had demanded since 2026-09-02, and nothing said so because no CI job has ever run
(F-12). Nineteen tests closed it. `planner.rs` had zero unit tests.

**What is not done, and why.** The new floors are not written into
`scripts/check-coverage.sh` or `docs/spec/07-quality-and-testing.md`. Both are
guard-protected, and the disclosed escape is the human setting
`MPB_ALLOW_GUARD_EDIT=1` at session start (`docs/spec/11-agentic-sdlc.md`).
Writing 95 into the specification while the gate still enforces 80 would be the
same defect this pass corrected three times over — documentation promising what
the code does not do — so it waits rather than being half-applied. **This is the
one outstanding owner action from DP-015**, and it is mechanical, not a decision.

> **Superseded 2026-09-17.** Both were written, and this paragraph outlived
> them: `check-coverage.sh` carries 95/95/95/95 with a 90 backstop, and
> `07-quality-and-testing.md`'s table carries the same, with the `adapters`
> raise explained in its own row. The outstanding owner action it names is
> **closed**. Left in place rather than deleted because the paragraph below it
> still stands, and because a struck-out claim is easier to trust than a missing
> one.
>
> The *other* half of it did not close. The denominator still counts test code
> in `src/`, and correcting it is now measured rather than estimated: a direct
> transcription of `llvm-cov`'s own line algorithm disagrees with `llvm-cov` on
> 59 of 75 files, by 449 lines, so post-processing the segments is not the small
> job F-40 assumed. Moving the in-`src/` test modules to `crates/*/tests/` is
> the recommendation. It stays yours because it lowers every percentage at once,
> by different amounts per crate, against floors you set.
>
> **Measured 2026-09-20, and the projection is worse-founded than it looked.**
> The attempt above was tried again from scratch and failed harder: a
> reconstruction from the JSON export's segments disagrees with llvm-cov's own
> per-file summary on **72 of 76** files. Then the same question was put to
> llvm-cov a second way — `cargo llvm-cov report --lcov`, its own line writer,
> no transcription at all — and **that disagrees with its own JSON summary on 60
> of 76 files**, always lower. So the obstacle is not that the algorithm is hard
> to copy; it is that **llvm-cov's two exports do not agree with each other about
> what a line is**, and `check-coverage.sh`'s denominator is defined by one of
> them. There is no per-region exclusion switch, and `#[cfg(test)]` code shares a
> file with the code it tests, so `--ignore-filename-regex` cannot reach it.
> **The new percentages cannot be computed from the existing data by any route
> tried.** Getting a real number means doing the move and re-measuring.
>
> What *is* exact, because it needs no coverage algorithm — a syntactic census of
> `#[cfg(test)]` modules in `crates/*/src/`, 2026-09-20:
>
> | Crate | Modules | `#[test]` fns | Test lines | `src/` lines | Share |
> |---|---:|---:|---:|---:|---:|
> | `domain` | 11 | 35 | 816 | 4267 | 19.1% |
> | `policy` | 19 | 83 | 1572 | 4507 | 34.9% |
> | `application` | 5 | 27 | 728 | 3754 | 19.4% |
> | `adapters` | 20 | 134 | 3113 | 9153 | 34.0% |
> | `cli` | 8 | 71 | 817 | 3272 | 25.0% |
> | **Total** | **63** | **350** | **7046** | **24953** | **28.2%** |
>
> These are source lines, not instrumented lines, so they do not give the new
> percentages — but they size the decision and show its shape. **The dilution is
> not uniform:** `policy` and `adapters` carry about a third of their `src/` as
> test code, `domain` and `application` about a fifth. Those two crates would
> move furthest, and both sit on a 95 floor. That is the "different amounts per
> crate" above, with numbers on it.
>
> (The earlier F-40 inventory counted 62 modules and 346 tests; this census
> counts 63 and 350. The method is stated here so the difference is checkable
> rather than mysterious: a scan for `#[cfg(test)]` followed by the next `mod`,
> brace-matched to its close.)

Two limits carried forward from the entry below and still true. **Branch coverage
is unenforced**: `--branch` needs nightly, the pin is stable (ADR-0002), so the
spec's "95% line *and* branch" stays half-enforced and the numbers above are
lines only. And **`check-coverage.sh` counts test code living in `src/` in its own
denominator** — in-module tests moved `application`'s total from 1851 to 1902
lines, diluting the figure they were raising. Worth fixing in the same edit as
the floors.

<details>
<summary>The original entry, as filed</summary>

### DP-015 as it was raised — The coverage floors have never been measured, and the toolchain policy is why
- **The question:** `07-quality-and-testing.md` sets coverage floors — 95% for
  `domain`, `policy` and planning, 80% for `adapters` plus a contract test per
  parsed command, `cli` reported. None has ever been measured. `cargo-llvm-cov`
  is not installed, and `cargo ins`​`tall` is denied by this repository's own
  tooling policy because the pinned toolchain holds the macOS floor (ADR-0002).
- **Correction, 2026-09-05: this entry understated what already exists.** It
  read as though nothing measured the floors at all. `scripts/check-coverage.sh`
  has existed since 2026-09-02, enforces `domain`/`policy`/`application` at 95%
  and `adapters` at 80%, and is wired into the `coverage` job of
  `.github/workflows/repository-validation.yml`, which installs the tool from a
  pinned action. Locally it exits 2 with "not available" by design. So the gate
  is built and connected; what has never happened is a **run** — F-12 records
  that no CI job has ever executed, because there is no remote. The floors are
  unmeasured for that reason, not for want of a gate, and the fix is smaller
  than this entry implied.
- **Also already recorded, and worth not re-discovering:** branch coverage is
  not enforced because `--branch` needs a nightly toolchain and the pin is
  stable (ADR-0002). The script says so in its header. The spec's
  "95% line **and** branch" is therefore half-enforced today.
- **Why it is yours:** every route out spends something. Relaxing the denial
  trades away part of what ADR-0002 bought; vendoring the tool adds a dependency
  to a project that checks every dependency's licence, deployment floor and
  `unsafe` surface; measuring only in CI leaves the floor unverifiable locally
  and waits on R-11's runner questions; and withdrawing the numbers is a real
  option that costs a stated quality bar.
- **Options:** (a) allow that one tool by name, as an exception with its own
  entry, and measure locally and in CI; (b) a CI-only step, accepting that a
  contributor cannot check the floor before pushing; (c) pin it as a workspace
  dev-dependency and take it through the dependency gate every other crate
  passes; (d) withdraw the numeric floors and state a weaker rule the project
  can actually hold.
- **Recommendation:** (a). It is the smallest exception that makes a stated
  requirement checkable; the tool is developer-only and never reaches a released
  artifact; and ADR-0002's concern is the *compiler* toolchain rather than a
  coverage harness. If you would rather not touch the denial at all, (c) is the
  honest second.
- **Default meanwhile:** the floors stand as written and are unverified. That is
  the state this entry exists to stop being invisible — nothing measures them, no
  gate fails on them, and no report has claimed otherwise. A gap being named,
  not a regression.

</details>

### DP-009 — May the first public release ship before the revocation plan exists? — **RESOLVED 2026-09-04: the plan exists**
The project owner asked for the plan rather than for a ruling, which is the
recommendation's path and removes the question. It is `SECURITY.md`, "If a
release must be withdrawn": when to revoke and — more usefully — when not to,
who can, what must be recorded before the first release, the runbook, what a
user can actually be told, and what the next release looks like on a revoked
identity. Recorded in **SE-068**.

Two things came out of writing it. The premise ADR-0005 rests on is **uncited**:
no page in this tree, and neither Apple page checked on 2026-09-04, says that
revocation stops previously published artifacts from launching. That is now
R-13, and the plan is written to hold either way. And the runbook's first
question — which artifacts carry the revoked identity — is unanswerable unless
the release records the signing identity CI observed, which is now a MUST in
`08-distribution-and-project.md`. Both were invisible while the plan was a
sentence in an ADR saying a plan was owed.

A security review of the plan then reordered it. Containment comes before
revocation — revoking first breaks the good releases while the bad one is still
downloadable — the trigger now covers an identity being *used* and not only a
key being read, and the digests are written down before anything is deleted,
because deleting a release deletes the `SHA256SUMS` the attestations are looked
up by. SE-068 carries the rest.

### DP-008 — Is the `.pkg` also provisional pending R-11? — **RESOLVED 2026-09-04: neither is**
Decided by the project owner: publish all three — bare binary, `.pkg`, `.dmg` —
unconditionally, and let the user choose. Recorded in **SE-067**;
`08-distribution-and-project.md` carries the decision and a new MUST that each
artifact be described where a user picks between them, and ADR-0005's "drop the
`.dmg`" bullet is marked superseded.

The question as filed had a false premise, and the answer that fixed it was not
one of the options offered. Both options — "mark both provisional" and "state
what the `.pkg` buys independently" — accepted the original clause's reasoning
that a container without a usable stapled ticket "buys nothing the bare binary
does not". It buys the installation experience, which no R-11 outcome touches.
What R-11 still decides is what the documentation may *claim*, and that is now
what the chapter says it decides.

### DP-012 — Do casks belong in the default top-level view? — **RESOLVED 2026-09-05: yes, and labelled honestly**
Decided by the project owner, in two parts. The ownership *source* is option
(c): the adapter reads the cask's on-disk receipt, so a cask Homebrew recorded
as requested is `TOP_LEVEL` and one with no receipt is `UNKNOWN` — never the
inferred `TOP_LEVEL` the code produced on evidence it marked non-authoritative.
The *visibility* half, which this entry always reserved for the owner, is that
an `UNKNOWN` row **appears in the default view**. Recorded in **SE-046**, now
accepted.

**Why the visibility half needed deciding, with the number that made it
decidable.** Implementing (c) alone was measured first: under `--replay` the
recording carries no receipt, so every cask becomes `UNKNOWN`, and the default
view lost nine rows and manual review fell from 15 to 6. The recommendation this
entry carried had priced (c) at "two rows rather than nine" — true of a live
run, wrong of a replayed one, and the approval had been given for the smaller
number. Showing `UNKNOWN` keeps the population intact and changes only the
label: 56 units in the default view, of which nine now say "not known" instead
of a false "requested".

**What that separated.** `topLevelCount` and `topLevelUnits` had coincided by
accident. The schema defined the first — "those the provider's own record says
the user asked for" — and left the second's Notes cell **empty**; "units shown in
normal mode" was a Rust doc comment, and an earlier draft of this entry quoted it
as though the schema said it. The schema says it now. They diverge, 47 and 56,
each matching its own definition.

The research behind it, including both questions you asked about migrating
casks, is `docs/research/R-12-cask-migratability.md`; the absolute it falsified
is F-28.

### DP-014 — Provider prefixes were compiled in — **RESOLVED 2026-09-05: option (a)**
`data/policy/provider-prefixes.toml` holds the three prefixes with their sources
and read dates, and `crates/adapters/src/prefixes.rs` reads it — embedded with
`include_str!`, exactly as `crates/policy` embeds its own data. The layering
objection was real and is answered by the loader living in `adapters` rather
than reaching up into `application`. Recorded in **SE-071**.

**The INV-20 test's exemption is gone, which is the proof.** It used to excuse
`registry.rs` with a comment saying the exemption was not compliance; the check
now holds the whole workspace, and putting a prefix back into that file was
replayed as a mutation and refused.

**One line of this entry's own rationale was wrong.** It said "a data file is a
one-line edit with a new `read_on`; a `match` is a release". Both are a release:
every policy file in this project arrives through `include_str!`. What INV-20
asks for is that a fact be stated as versioned data beside the source it came
from and the date it was read, where a person can compare the two — not that it
be changeable without rebuilding, which nothing here does.

### DP-011 — The charter's Intel tier vs. its own data — **RESOLVED 2026-09-04: amend**
Decided by the project owner. `00-charter.md`, `README.md` and
`wiki/Intel-Mac-Homebrew-Support.md` now name Tier 3 as Homebrew names it, quote
what Homebrew's separate *Unsupported* category means — "will not run without
third-party patches or modifications" — and state that it is Mojave 10.14 and
earlier, which Intel is not. Recorded in **SE-044**, now accepted. Finding F-11
is closed.

**The recommendation on file was itself wrong, in the same direction.** It
proposed the wording "Tier 3 — degraded, with bottle builds stopped", and
*stopped* is the claim `README.md` had already corrected to *frozen and eroding*
and that `provider-support.toml` records as `prebuilt_artifacts = true`.
Implemented literally, it would have removed one overstatement from a Tier 2
document by restoring another, under an approval given for the substance. The
substance landed and the words did not; SE-044 carries the reasoning. Two
entries proposing a correction is not two independent checks of it.

### DP-013 — Charter identity vs. what a release does — **RESOLVED 2026-09-04: option (b)**
Decided by the project owner. One sentence in `00-charter.md`, in the same
paragraph that asks for consistency: the identity describes the finished
product, a release surface describes that release, and where the two differ the
surface states what the build does. "Executes migrations" stays in the identity,
because it is true of the project and not of this build. Recorded in **SE-058**;
no other surface changed, because they were already right.

The sentence also names the failure mode it exists to stop. The risk DP-013
identified was never the drift itself — it was the next consistency audit
correctly finding the charter to be the outlier and "fixing" the README back to
a claim the binary cannot support.

### DP-010 — Reverse-dependency counts — **RESOLVED 2026-09-02: the invariant stands**
Decided by the project owner: INV-04 and INV-08 hold as written, and the count
moves into the per-provider map rather than a side being chosen. Recorded in
**SE-045**, which withdraws SE-042 change (2); implemented with SE-047 and
SE-048, because the three were one defect seen from three angles — the count was
collapsed to one number partly *because* the source attribution was unstable.

What this bought, measured: the engine no longer tells a user that three
MacPorts ports depend on a package when zero do; the rule fires on exactly the
139 units whose source side has dependents, with no divergence; and the
planner's own message names the provider the count came from. What it cost: the
shipped fixture's `manualReview` total goes from 23 to 22 — one unit changes
outcome, three lose a reason they should not have carried — and one golden file
changed.

The reasoning that made this the owner's call rather than an agent's is kept in
SE-045: the original `max` was not carelessness but a genuine collision between
INV-04 and INV-17, and a Tier 3 entry had been used to resolve it.


### DP-002 — Implementation language — **RESOLVED 2026-09-02: Rust**
Decided by the project owner. Recorded in
[`adr/0002-implementation-language.md`](adr/0002-implementation-language.md)
(now *accepted*) and SE-022. The foundation pass is unblocked from step 3.

Consequences that now bind the project, propagated rather than filed:
`rust-toolchain.toml` pins the toolchain and a floor increase needs an ADR; CI
asserts the deployment target on the built artifacts rather than trusting the
build; the dependency gate checks licence, deployment floor **and** the `unsafe`
and build-script surface a crate brings in; and ADR-0002's verification list
becomes acceptance criteria on the first pass, not a reading exercise.

### DP-003 — Licence — **RESOLVED 2026-09-02: Apache License 2.0**
Decided by the project owner. Recorded in
[`adr/0001-licence-apache-2-0.md`](adr/0001-licence-apache-2-0.md) and SE-021.
`LICENSE` is committed. Consequences that now bind the project: SPDX headers on
every source file, no `NOTICE` file until something needs attributing,
inbound=outbound contributions with no CLA, and **no GPLv2-only dependencies**.

### DP-001 — Minimum supported macOS — **RESOLVED 2026-09-02: 10.12 Sierra (Intel) / 11.0 (Apple Silicon)**
Decided by the project owner. Recorded in
[`adr/0002-implementation-language.md`](adr/0002-implementation-language.md) and SE-022.
This is the constraint that decides DP-002 and binds every dependency: anything
that raises the deployment floor is rejected.

### DP-005 — CLI short alias — **RESOLVED 2026-09-02: `mpb`, with a conflict guard**
Decided by the project owner, together with the rename to **MacPortsBrew**.
Recorded in [`adr/0003-name-and-cli-alias.md`](adr/0003-name-and-cli-alias.md)
and SE-023. `mpb` collides with MIT Photonic-Bands (installed by other means — neither
provider packaged it when checked, ADR-0003), so installation detects an
existing `mpb` and declines to shadow it silently.

### DP-004 — Release signing — **RESOLVED 2026-09-02: Developer ID signing and notarisation, three artifacts**
Decided by the project owner, who already holds a paid Apple Developer account.
Recorded in [`adr/0005-release-signing-and-artifacts.md`](adr/0005-release-signing-and-artifacts.md)
and SE-014. Every release artifact is signed with Developer ID and notarised,
and three are published per architecture: the bare binary — still the path the
documentation leads with — plus a `.pkg` and a `.dmg`, which exist because they
can carry a stapled notarisation ticket and a bare Mach-O cannot (R-9).

The consequences — credential handling, why the tag-push rule needs a protected
environment rather than a convention, where the `.pkg` may install, and what
remains unverified — are in
[`adr/0005-release-signing-and-artifacts.md`](adr/0005-release-signing-and-artifacts.md).
The binding requirements are restated once, in
[`spec/08-distribution-and-project.md`](spec/08-distribution-and-project.md);
the specification change is SE-014. R-11 gathers what still blocks the
implementation pass, and the R-9 quarantine experiment survives as documentation
of the path not taken rather than as a decision gate.

### DP-006 — Copyright holder — **RESOLVED 2026-09-02: Pablo Guia**
Decided by the project owner. The string matches the name on the Apple
Developer account, so the Developer ID signature and the copyright line name
the same person rather than two spellings of one. Appears in `README.md`,
release metadata and any future `NOTICE`; **not** in source files, which carry
an SPDX identifier only (ADR-0001).

### DP-007 — Repository hosting and visibility — **RESOLVED 2026-09-02: public repository on GitHub**
Decided by the project owner: public, under a personal Pro account, named
`macportsbrew` (ADR-0003), with GitHub Pages for the project site and possibly
a custom domain later.

Consequences that now bind the project: standard GitHub-hosted runners are free
and unlimited on public repositories while the plan's included minutes are
private-repository only (R-10), so the two-architecture macOS matrix costs
nothing — but **`-large` runners are never free on public repositories and must
not be used**, and self-hosted runners do not belong on one, because a fork can
execute arbitrary code on them. That GitHub withholds secrets from
fork-triggered workflows is what makes ADR-0005's signing design safe to run in
the open, and equally what constrains its shape. Enabling Pages **is** the act of
publishing, not a step after it; if a custom domain is registered, use the
registrar's WHOIS privacy. `git push` stays denied by the hooks until the remote
exists.

**Before acting on this decision — settings no check in this repository can
verify.** Each is a one-way door in the sense that the damage happens in the
window before it is closed, so the order matters:

1. **Private vulnerability reporting enabled** (*Security* → *Report a
   vulnerability*) **before** the repository becomes public. A public repository
   shipping Developer ID-signed binaries with no private channel forces the first
   report of a signing-pipeline flaw onto a public tracker (`SECURITY.md`).
2. **A protected deployment environment** holding the signing credentials, with a
   deployment tag rule limited to `v*`, and no workflow other than the release
   workflow permitted to reference it. Repository secrets are readable by any
   workflow that names them, so this — not the convention that only one workflow
   asks — is what actually binds the key to a release (ADR-0005).
3. **A protected-tag ruleset**, so a non-owner cannot create or move a release
   tag. Without it, rule 2's `v*` condition is satisfied by anyone with write
   access.
4. **`dtolnay/rust-toolchain@master` pinned** before the repository is public. It
   is a mutable branch reference in the workflow that runs the gate, on a
   `pull_request` trigger; going public removes the mitigating fact it was
   reviewed under.
