# 02-ARCHITECTURE-REVIEW — findings

**Date:** 2026-09-02 · **Reviewed at commit:** `ab56b06` · Work order:
[`../../prompts/02-ARCHITECTURE-REVIEW-PASS.md`](../../prompts/02-ARCHITECTURE-REVIEW-PASS.md)

Findings are ranked by consequence, not by count. Each states what was checked,
names the invariant or specification line it violates — or says explicitly that it
is a judgement call — and gives a concrete failure scenario. Proposed specification
changes are SE-044 to SE-054 in [`../spec-evolution.md`](../spec-evolution.md), so a
later pass can accept or reject each individually. The pass report is
[`02-ARCHITECTURE-REVIEW-PASS-report.md`](02-ARCHITECTURE-REVIEW-PASS-report.md).

**One finding stops at the human and is not mine to resolve: F-01.**

## Status since the review

Four findings were fixed immediately after the review, at the owner's request,
and this file is left as the record of what was found rather than rewritten:

| Finding | State |
|---|---|
| **F-05** — home path published when `HOME` is unset | **Fixed.** SE-054, now `accepted`. `HomeRedaction` distinguishes the two absences; measured back to zero on `doctor --report` and `snapshot --out`. One residual is retained and recorded in SE-054: a home outside `/Users` is still not recognised. |
| **F-16** — `cargo-deny` never checked advisories | **Fixed.** SE-055. |
| **F-17** — twelve unpinned actions, four mutable | **Fixed.** SE-055; `scripts/check-action-pins.sh` in `validate.sh`, verified to fail on an unpinned ref. |
| **F-18** — review workflow could not post, and had no author guard | **Fixed.** SE-055. |

Everything else stands as written, including all four Tier 1 violations.

## What was verified, and on what

The previous session reported that this machine had no Rust toolchain and skipped
every Rust check on that basis. **It has one**: `rustc 1.98.0`, matching the pin,
deployment target `10.12`. Everything marked *observed* below was run here — Intel
`x86_64`, macOS 26.6.2, Homebrew 6.0.21 at `/usr/local`, MacPorts absent.

| Foundation-pass claim | Result |
|---|---|
| 456 tests pass | **Verified.** Per-crate split matches exactly (domain 17, policy 100, application 28, adapters 190, cli 121). |
| 157 fixture-matrix checks over 6 fixtures | **Verified.** |
| 70 hook cases | **Verified.** 70/70. |
| `scripts/validate.sh` passes | **Verified.** Exit 0, full mode, tests included. |
| The slice runs — 189 packages, 56 top-level | **Verified** on this machine. |
| INV-15 reproducibility | **Verified empirically** — two `--from-snapshot` runs byte-identical over 184 KB. |
| INV-06 — nothing elevates | **Verified structurally** — zero elevation tokens in `crates/*/src`; exactly one `Command::new` site. |
| All 20 invariants have a test | **Verified** they exist. Several would not fail if the invariant were violated — see F-19. |
| Coverage floors | **Still unverified.** `cargo-llvm-cov` absent. Honestly reported before, still true. |
| Homebrew Intel timeline | **Re-verified live 2026-09-02** — see F-10. |

---

# Tier 1 violations

## F-01 — One provider's dependency graph decides the other provider's recommendation, and the user is told a false number

**Severity: highest. INV-04 and INV-08. This one stops at the human.**

`crates/application/src/analysis.rs:167-180` takes the **maximum** reverse-dependency
count across providers:

```rust
let mut reverse = p.inventory.reverse_dependencies_of(&name).len();
for m in &mappings { ... reverse = reverse.max(dp.inventory.reverse_dependencies_of(&sel.package.name).len()); }
```

INV-08 is absolute: *"The presence of `openssl3` in MacPorts does not satisfy
Homebrew's `openssl@3` requirement, **in any calculation, ever**."* INV-04 says
reverse dependencies are resolved *"inside that provider"*. The rule that consumes
the number says so too, in its own doc comment
(`crates/policy/src/rules/manual_review_reverse_dependencies.rs:16-20`): *"a
cross-provider count would not merely be unhelpful, it would be a statement the
domain forbids."*

**Observed** on `fixtures/recordings/both-providers-arm64-macos15.json`, package
`gnutls`:

```
rows.macports.reverseDependencyCount = 0
rows.homebrew.reverseDependencyCount = 3
unit source                          = macports

reason emitted: {"id":"reason.reverse_dependencies_present",
                 "params":{"count":"3","provider":"macports"},
                 "rule":"MANUAL_REVIEW_REVERSE_DEPENDENCIES"}
```

The user is told **three MacPorts ports depend on `gnutls`**. Zero do. The three are
Homebrew's, relabelled with the other provider's name. The rule then flips the
outcome to `MANUAL_REVIEW` on the strength of a graph that belongs to a provider the
plan is not touching.

The planner does **not** do this — `planner.rs:152` reads
`source_row.reverse_dependency_count`, the correct per-provider value. So for one
package on one run the engine sees 3 and the planner sees 0.

**In fairness on direction of harm:** `max` only ever over-reports, so it cannot
*miss* a blocking condition. The safety error is conservative. The defects are that
it states something false to the user, and that it breaches a Tier 1 invariant whose
wording admits no exception.

**Why this stops at the human.** It is declared — `docs/spec-evolution.md` SE-042,
change (2). That does not resolve it: SE-042 is a Tier 3–5 mechanism being used to
relax a Tier 1 invariant, which the authority table does not permit. SE-042's stated
rationale ("removing either copy is what a plan may do, so the guard must see the
worse side") does not describe the code either — a plan removes exactly one named
side, and the planner already reads that side correctly. **A reviewer may propose a
Tier 1 change and may not make one.** Recorded as DP-010.

**Remedy if the owner rules the invariant stands:** feed the engine the source
provider's own count, or split `reverse_dependency_count` into the per-provider map
in `PackageFacts.per_provider`, where availability already lives correctly
(`facts.rs:63-123`). SE-042(2) is then withdrawn.

---

## F-02 — A package held by both providers can only be migrated one way, and the direction is chosen by alphabetical order

**Severity: high. INV-17 — "Neither direction is a special case in code."**

`planner.rs:78-84` refuses when the unit's attributed `source` equals the requested
destination. There is no `--from` flag. For a package installed under **both**
providers, which side counts as "source" is decided by `source_for`
(`analysis.rs:396-422`) falling back to the provider being iterated — and iteration
is over a `BTreeMap`, so `"homebrew" < "macports"` decides it.

**Observed**, on packages that are `INSTALLED` and `TOP_LEVEL` under both providers:

```
htop   --to macports  -> exit 0, a full 9-step plan
htop   --to homebrew  -> exit 2, "htop is already held by homebrew"
gnutls --to homebrew  -> exit 2, "gnutls is already held by homebrew"
```

A user consolidating onto MacPorts has a command. A user consolidating onto
Homebrew — dropping the MacPorts copy of a package both providers hold — has none.
That is a special case in one direction, which is exactly what INV-17 forbids, and
it lands on the direction the charter goes out of its way to call first-class.

**Test evidence.** Across the whole suite there are **8 `plan` invocations and every
one is `--to macports`**. MacPorts→Homebrew planning has never been executed by a
test. The asymmetry is in the planner, which is precisely where INV-17 lives. (The
adapter layer is fine — 39 Homebrew tests against 62 MacPorts tests — and the policy
layer is genuinely symmetric; see F-19.)

**A second defect in the same guard.** It tests `source == request.to` rather than
*does the destination already hold this package*. So `plan htop --to macports`
produces a plan whose step 2 is `AddDestination — Add htop under macports` for a
package MacPorts already has installed and active, and whose step 6 removes the
Homebrew copy for real. The plan also warns that it could not determine whether
MacPorts would supply `htop` prebuilt — about a package already built and installed
there.

**Remedy.** `--to X` must mean "X is the destination, the other side is the source",
not "reject if the inferred source happens to be X"; and the guard must test the
destination's actual contents. Plus a mirrored test for every existing
`--to macports` case.

---

## F-03 — `--advanced` changes the answer, not just the view

**Severity: high. INV-13, and it makes two commands disagree about one input.**

`--advanced` is specified as additive revelation: transitive dependencies are hidden
by default and *"reachable only through an explicitly entered advanced mode"*. In
`analysis.rs:102-105` the non-top-level row is skipped **before** the merge at
`:129-146`, so which provider builds the unit depends on the mode.

**Observed**, same fixture, package `gnutls` (TOP_LEVEL under MacPorts by the
authoritative `requested` record; DEPENDENCY under Homebrew):

| | `analyze` | `analyze --advanced` |
|---|---|---|
| `summary.topLevelUnits` | **63** | **62** |
| `gnutls` source | `macports` | `homebrew` |
| `gnutls` `advancedOnly` | `false` | `true` |
| reasons emitted | 4, incl. `reason.destination_offers_prebuilt` | 3 |

Three separate problems:

1. **Advanced mode *removes* a user-requested package from the top-level set** and
   reclassifies it as never-preselect-for-migration. INV-13 says advanced mode
   reveals; here it hides.
2. **The recommendation itself changes with a view flag.** A visibility switch is
   recomputing policy.
3. **`build_plan` hardcodes `advanced: true`** (`planner.rs:59-63`) regardless of the
   user's mode, so the planner always reasons in the mode the user may not be in.
   `analyze` says `gnutls` is held by MacPorts; `explain gnutls` — which forces the
   same path — reports `package.source: homebrew`. Two commands, one input,
   contradictory answers.

The foundation report recorded this as a count that "differs by one … Known, not
fixed." It is not a count artifact: the unit's source, its `advancedOnly` flag and
its rule set all change, and the planner consumes all three.

**Remedy.** Decide the unit's driving side across all providers holding the name,
derive `advanced_only` from the unit (top-level in *any* provider ⇒ top-level),
*then* apply the visibility filter.

---

## F-04 — The one plan command that escapes INV-10, and the invariant's own test filters it out

**Severity: high — not for the leak, for what the test says about the suite.**

Every provider command in a plan carries a constructed environment. One does not.
`planner.rs:264-272` emits the `SET_PRIMARY` step with `environment: BTreeMap::new()`.

**Observed** in the emitted plan for `htop --to macports`:

```
ADD_DESTINATION    program=/opt/local/bin/port     env=[LANG, LC_ALL, NO_COLOR]
VERIFY_DESTINATION program=/opt/local/bin/port     env=[LANG, LC_ALL, NO_COLOR]
SET_PRIMARY        program=macportsbrew            env=[]
REMOVE_SOURCE      program=/opt/homebrew/bin/brew  env=[HOMEBREW_NO_ANALYTICS, ...]
```

The test named after the invariant,
`invariant_10_plan_commands_carry_the_sanitised_invocation_environment`
(`crates/application/tests/invariants.rs:361`), reads at line 367:

```rust
.filter(|c| c.program != "macportsbrew")
```

**The one command that fails the invariant is excluded by the test that defends it.**
That is the finding. A gate that is green because its exception was written into it
is worse than an absent gate, because it is cited as evidence — and this suite's
green status is cited in the foundation report, in `README.md` and in
`wiki/Architecture.md:129`.

Two further defects in the same three lines: `"macportsbrew"` is a **relative**
program name, which `process/system.rs:171-183` refuses by design, so **step 4 of
every migration plan is un-runnable by construction**; and a relative name resolves
through `PATH`, which is the shadowing hazard this tool exists to warn about.

**The adapter layer is clean on INV-10** — all three production `CommandSpec` sites
route through `sanitised_environment`, `Command::new` appears once with
`.env_clear()`, and environment reads occur only in `system/machine.rs`.

---

# Security and safety

## F-05 — `doctor --report` leaks the user's home path when `HOME` is unset

**Severity: high. Demonstrated, reproducible, affects users today.**

`06-cli-and-ux.md:134-137` describes this file as something a user will *"attach to
an issue"*; `wiki/Troubleshooting.md:23` repeats it. `redact_home`
(`crates/domain/src/snapshot.rs:132-146`) is well built — whole-component matching,
so `/Users/adamant` survives a user named `ada`. But
`crates/adapters/src/system/machine.rs:140-143` yields `home: None` when `HOME` is
unset or relative, and `redact_home` with `None` is **the identity function**.

**Observed on this machine:**

```
doctor --report r1.json            ->  0 occurrences of /Users/pirado
env -u HOME  doctor --report r2.json -> 39 occurrences
```

Leaked values include the username and full paths of installed tooling.

**Failure scenario.** A user runs `doctor --report` from `cron`, `launchd`, a CI
step, or any shell where `HOME` is not exported — all routine — and attaches the
file to a public issue exactly as the documentation invites. The output looks
identical to a redacted one; nothing warns them.

**Remedy.** Resolve the home directory by a second mechanism, or stamp the artifact
`"redacted": false`. Note `--replay` sets `home: None` deliberately and correctly
(`app/context.rs:94`) — the two cases being indistinguishable at the redaction site
is the underlying defect.

---

## F-06 — Verification steps that cannot verify what they are named for

**Severity: high for the executor pass. INV-01 depends on these.**

Two of the verification steps in a plan assert something other than what their name
says. Because the plan is the contract (INV-02) and the executor runs plan steps and
nothing else, these are load-bearing.

- **`VERIFY_REMOVAL` asserts the package is still installed.** `planner.rs:301-307`
  gives it `VerificationCheck::ProviderReportsActive`, documented at `plan.rs:93-94`
  as *"The destination provider reports the package installed and active."* There is
  no `PackageAbsent` variant in the enum. As data, the plan currently says: verify
  the removal by confirming the package is still there. Whatever executor lands will
  either silently invert it or have nothing to run.
- **`VERIFY_DESTINATION` is satisfiable by pre-existing state.** For a package
  already installed at the destination (F-02), all five contract checks
  (`04-migration-and-safety.md:44-55`) are true *before* the plan runs. The contract
  correctly forbids treating exit 0 as verification — but nothing distinguishes
  "this install succeeded" from "it was already there", so a failed or no-op install
  still clears the gate INV-01 depends on. The planner never reads
  `unit.rows[to].presence`.

**Also here, and cheap to fix:** `check_ordering` (`crates/domain/src/plan.rs:206-216`)
uses `.position()` — the *first* occurrence of each `StepKind`. `AGENTS.md` says it
enforces that no `REMOVE_SOURCE` precedes the `VERIFY_DESTINATION` **guarding it**.
A multi-package plan `[Snapshot, Add(A), Verify(A), Remove(A), Remove(B), Add(B),
Verify(B)]` passes. It holds today only because `build_plan` emits one of each. This
is the INV-01 enforcement point and it should be pairwise before any batch plan
lands.

---

## F-07 — `doctor` cannot see the most common executable-shadowing directory on macOS

**Severity: high. This is the tool's own value proposition.**

`crates/adapters/src/system/path_audit.rs:89-94` tests only world-writable
(`0o002`) and group-writable (`0o020`). There is no owner-writable test, and
`PathFindingKind` (`domain/src/snapshot.rs:69-91`) has no variant for one.
`application/src/doctor.rs:214-228` escalates to `Error` only for
`WorldWritable if precedes_system`.

1. **`~/bin` at `0755`, owned by the user, ahead of `/usr/bin`, produces no finding
   at all** — the standard shape of the attack (a `curl | sh` installer, an `npm`
   postinstall, a compromised dev dependency dropping `~/bin/git`). This is verbatim
   the threat-model item at `04-migration-and-safety.md:260-261`.
2. **`/usr/local/bin` on Intel is `drwxrwxr-x root:admin`** — every admin account is
   in `admin`. Ahead of `/usr/bin` it yields a warning, never escalated by position.
   This machine is exactly that configuration.
3. **ADR-0005:210-218 makes it a requirement**: the `.pkg` installs MacPortsBrew's
   own directory user-owned `0755`, and `doctor` *"must report MacPortsBrew's own
   directory too rather than special-case it"*. With no owner-writable kind it
   structurally cannot. The day the `.pkg` ships, the tool creates a shadowing
   directory it is contractually required to warn about.
4. **Not previously reported:** `doctor.rs:202-204` derives `first_system` from
   `/usr/bin`, `/bin`, `/usr/sbin`, `/sbin`. If none is on `PATH`, `precedes_system`
   is permanently `false` and even the world-writable escalation degrades to a
   warning. A `PATH` with no system directory is itself a finding, and is not one.

---

## F-08 — "There is no shell to inject into … enforced by a lint rule" is not enforced

**Severity: high as a claim; low as a live exposure.**

`04-migration-and-safety.md:257-258` states: *"Command injection — mitigated
structurally by argv-only execution. There is no shell to inject into. Enforced by a
lint rule, not by discipline."*

The lint (`scripts/check-layering.sh:58`) greps for `Command::new` outside
`crates/adapters/src/process/`. That enforces **where** a subprocess is spawned, not
**what**. The only program check in the executor is `spec.program.is_absolute()`
(`process/system.rs:171`) — and `/bin/sh` is absolute.

What actually prevents shell construction today is a run-time argv allow-list in each
of the two adapters (`homebrew/mod.rs:249`, `macports/mod.rs:258`) — good, and
correctly enforced at run time rather than by `debug_assert!`. But that is
per-adapter *discipline*, which is what the sentence says it is not relying on.
`SystemMachine::probe` (`system/machine.rs:221-229`) builds a `CommandSpec` from an
arbitrary program with no allow-list; its callers pass hardcoded constants, so it is
correct today, and the asymmetry is the finding.

**Failure scenario.** A later `add-provider` pass writes an adapter that shells out
to parse an awkward format. Clippy, the layering check and every test pass. The
threat model's "structurally impossible" is now false and nothing failed.

---

## F-09 — `--from-snapshot` is an unvalidated source of plan argv

**Severity: high once the executor lands; medium today.**

`--from-snapshot` is `global = true` (`cli/src/app/mod.rs:66-67`), so it applies to
`plan`. `check_schema` (`domain/src/snapshot.rs:226-238`) compares an integer version
and nothing else. No name, version or path from the file passes the `is_safe_name` /
`checked_name` predicates the adapters apply to provider output. Those names then
flow into plan commands: `planner.rs:377` pushes the package name into argv with no
`--` separator; `:251` and `:254` turn a package name into an **executable name**;
`:219` yields an **empty argv element** when the mapping is missing.

**Failure scenario.** "Here is a snapshot from my machine, what should I migrate?"
The file declares a package named `--force`, or `-o /etc/anything`, or
`../../../bin/sh`. Today the plan only renders it. When the executor runs
`PlanCommand.argv` as `04-migration-and-safety.md:34` says it must, it is a
flag-injection primitive that never passed an adapter allow-list — because the
allow-lists live in the adapters and the planner does not go through them.

**Violates** the adapter rule in `AGENTS.md` — *"never let provider metadata become a
path or a flag"* — at a boundary the adapters do not guard.

---

## F-10 — A read-only command is enumerated as elevating

**Severity: medium. INV-06 erosion by habituation.**

`data/policy/provider-commands.toml` defines elevation for `add`, `remove`,
`*_gui_app` and `set_primary` — but not for `verify`. `planner.rs:382-390` falls
through to `Elevation::Unknown`, and `:329-337` counts anything not `None` as
elevated.

**Observed:** the plan for `htop --to macports` ends *"Steps that would need
administrator rights: 2, 3"*, where step 3 is `port -q installed htop` — a read.

The fold is toward the safe side, and that is the right default. But the file's own
comment says the `_gui_app` keys exist *"so the planner never reaches an elevation
value by omission"*, and `verify` reaches it by omission. Under INV-06 the effect is
to train a user to accept an elevation prompt for a read, which is the habit the
invariant exists to prevent.

---

# Honesty and documentation

## F-11 — The charter and README misstate Homebrew's own tier vocabulary, in the project's favour

**Severity: medium-high. Tier 2 document; the project's central justification.**

**Re-verified live 2026-09-02** at <https://docs.brew.sh/Support-Tiers>:

- *"macOS Tahoe 26 is the final version of macOS to run on Intel hardware. In
  alignment with this change, Homebrew has stopped building new bottles for Intel
  systems, and will remove the ability to run Homebrew on Intel systems in or after
  September 2027."* — **unchanged.** The timeline claim and
  `end_date = "2027-09-01"` in `data/policy/provider-support.toml` are correct.
- As of September 2026, Intel x86_64 is **Tier 3** (macOS Catalina 10.15 through
  Tahoe 26). **Unsupported** is a *separate* category — Mojave 10.14 and earlier —
  defined as a configuration where *"Homebrew will not run without third-party
  patches or modifications"*.

`docs/spec/00-charter.md:37` and `README.md:31` both say Homebrew classifies Intel
x86_64 as **"Tier 3 — not supported"**, conflating the two. Homebrew emphatically
does run on Intel Tier 3 today.

**The data model gets this right** — `provider-support.toml` maps Intel Tier 3 to
`DEGRADED` and only `macos_max = "10.14"` to `UNSUPPORTED`. The binary is more honest
than the prose: `status` on this machine prints Homebrew support as `degraded`.

**Failure scenario.** A reader on an Intel Mac reads the project's own README,
concludes Homebrew is already unsupported for them, and migrates under an urgency the
cited source does not support. For a migration tool the error runs in the
self-serving direction, which is the direction that costs credibility when someone
checks. The charter is Tier 2, so this is proposed (SE-044), not applied.

---

## F-12 — CI has never run, and the documentation reads as though it has

**Severity: medium-high.**

`git remote -v` is empty; the workflow has never executed. Yet
`docs/support-matrix.md:22-23` defines **tested** as *"a hosted runner … **runs** the
full suite natively"*, and `README.md:181-184` says the matrix *"distinguishes what CI
**actually exercises** from what is merely believed to work"*. The matrix is generated
from `runner:` labels — `scripts/support-matrix.sh` parses nothing else — so every row
states intent, not evidence. `CONTRIBUTING.md:54` and `wiki/Contributing.md:32` add
*"CI runs it and is the authority"*.

This is the failure mode `08-distribution-and-project.md:236-237` names in its own
words: *"Claiming tested support for an untested OS is the kind of dishonesty this
project cannot afford."* The best-effort rows are handled correctly; the **tested**
rows claim an event that has not happened.

**Remedy in the generator, not the generated file:** emit an unconditional line
stating no CI job has ever run, until the first green one; soften "runs" to "is
configured to run"; mirror it in `README.md` and in `CHANGELOG.md`'s known
limitations, which omits it.

---

## F-13 — The README's opening paragraph says the tool executes migrations

**Severity: medium-high.**

`README.md:11`: *"…and **executes migrations in either direction** without ever
removing something before its replacement is installed and verified."*
`README.md:63`: *"**It executes nothing.** … there is no `migrate` command."*

The lead paragraph is what gets quoted, indexed and skimmed; the correction is fifty
lines below the fold. Every other landing surface handles it correctly —
`wiki/Home.md:5-10` says "builds a reviewable plan to move it", and both migration
pages open with an explicit note that execution is a later pass.

---

## F-14 — INV-03 is restated to users as a guarantee, with its qualifier removed

**Severity: medium-high. Violates an explicit MUST.**

`01-invariants.md:22-25`: *"left at least as usable as before, **whenever technically
possible**."* `wiki/Dependencies-and-Safety.md:132` heads the section **"The safety
guarantees"** and line 141 drops the qualifier; `wiki/FAQ.md:22-29` does the same.

`04-migration-and-safety.md:83-84` is a MUST: *"Do not promise transaction semantics
the underlying package managers do not provide."* Neither `brew` nor `port` offers
them — a `port uninstall` that dies after deactivation, or a `brew link` that fails
partway, leaves a state MacPortsBrew cannot rewind.

---

## F-15 — `restore` is documented as "undo" and omits two of the three limits the spec mandates

**Severity: medium-high. Violates an explicit MUST.**

`04-migration-and-safety.md:144-152` requires the name `restore` *"not `undo`, and
the documentation says why"*, no promise of version-exact reconstruction, and no
reversal of out-of-band changes. `wiki/Undoing-a-Migration.md` is titled **"Undoing a
Migration"**, calls it *"The undo path"* (line 49), never explains the naming, and
carries only the narrow limit (*"cannot restore a version a provider no longer
offers"*). Missing: that a successful restore can return a **newer** version than the
snapshot recorded, and that changes made by hand are not reversed. Line 68 also
overstates scope — the spec limits primary-selection restoration to *"if that run
changed them"*.

**Judgement call, in the design's favour:** `restore --plan` as *described* — ordered
steps, full argv, per-step elevation — would be followable by a person;
`cli/src/app/human.rs:371-376` already renders that shape. The promise is
deliverable; the documentation of its limits is not yet honest.

---

# Supply chain and configuration

## F-16 — `cargo-deny` never checks advisories, while `deny.toml` declares an advisories policy

**Severity: medium.**

`.github/workflows/repository-validation.yml:75` runs
`command: check licenses bans sources`. **`advisories` is absent.** `deny.toml:16-17`
declares `[advisories] yanked = "deny"` — dead configuration that reads as enforced.
A RUSTSEC advisory against `serde_json`, `toml` or `clap`, or a yanked crate in
`Cargo.lock`, ships green.

**Credit, and verified:** the other two axes of ADR-0004 are genuinely enforced.
`scripts/check-licences.sh:20-56` reads `cargo metadata --locked` and fails on an
unlisted licence **and** on any crate carrying an unreviewed `build.rs`;
`scripts/check-deployment-target.sh` reads `LC_VERSION_MIN_MACOSX` /
`LC_BUILD_VERSION` off the built binaries *and every object inside every rlib*, so a
dependency that raises the macOS floor is caught on the artifact rather than on the
promise.

---

## F-17 — Every GitHub Action is unpinned; four are mutable refs, not one

**Severity: medium. Blast radius bounded, but the job is load-bearing.**

Twelve `uses:` references, **none** pinned to a SHA:

| Ref | Sites | Shape |
|---|---|---|
| `dtolnay/rust-toolchain@master` | `repository-validation.yml:51, :91, :121` | mutable **branch** |
| `taiki-e/install-action@cargo-llvm-cov` | `:125` | mutable ref — **not previously flagged**, and it downloads a binary into the job |
| `actions/checkout@v6` | 5 | movable major tag |
| `anthropics/claude-code-action@v1` | 2 | movable major tag |
| `EmbarkStudios/cargo-deny-action@v2` | 1 | movable major tag |

`repository-validation.yml` has `permissions: contents: read` and no secrets, so the
realistic loss is build/cache poisoning and a read-scoped token, not a release —
which is why this is not ranked higher. But `check-deployment-target.sh` runs in that
job, so the job's integrity is load-bearing for a correctness claim. DP-007's
pre-publication checklist carries the `@master` item; it should carry all four, and
`scripts/validate.sh` should gain a pin check so it is enforced rather than
remembered.

---

## F-18 — `claude-code-review.yml` cannot post the comments it is configured to make, and has no author guard

**Severity: medium. Two defects in one file.**

1. **It will silently never work.** The job declares `permissions: pull-requests: read`
   (line 21) while granting an inline-comment tool and asking for `--comment`
   (line 33). It cannot post. Every PR gets a job that fails or does nothing, and
   because nobody expects output, nobody notices the review is not happening.
2. **No `author_association` guard**, unlike `claude.yml`, which has a correct one at
   lines 28-32 and explains its reasoning well at 19-29.

**Mitigation the previous pass did not record, which lowers the severity:** the
trigger is `pull_request`, not `pull_request_target`, and GitHub withholds repository
secrets from fork-triggered `pull_request` runs. This is *not* the classic fork
secret-exfiltration hole. The residual is a prompt-injection surface bounded by
read-only permissions, plus the contributor-experience problem previously identified:
a fork PR gets a red check the contributor cannot fix.

---

## F-19 — Several invariant tests would not fail if their invariant were violated

**Severity: medium, and it is the reason F-01 to F-04 survived a green suite.**

The work order asked which invariant tests would actually fail. Assessed individually:

| Test | Verdict |
|---|---|
| `invariant_10_plan_commands_carry_the_sanitised_invocation_environment` | **Excludes its own exception** (F-04), and checks only that `PATH` exists and no build-flag *key* is present — never that the other provider's prefix is absent from `PATH`, which is what INV-10 actually says. |
| `invariant_10_reading_the_machine_leaks_no_build_flag_into_a_probe` | **Cannot fail for the invariant's subject.** It `continue`s past every program that is not `sw_vers`/`uname`/`sysctl`/`xcode-select` — i.e. it skips every `brew` and `port` invocation. INV-10 is about `brew` and `port`. |
| `invariant_07_08_dependencies_never_cross_providers` | **Actively misleading.** Its comment says facts count the source side only, then asserts on `plan.risks` — the planner's value, which is correct — never on the engine's `reverse_dependency_count`, which is the one that crosses. Passes while F-01 is live. |
| `invariant_17_swapping_the_providers_swaps_the_analysis` | **Blind to the real asymmetry.** It renames provider *contents* but keeps the map keys, so iteration order never changes. Adding one assertion on the swapped `source` fails today. |
| `invariant_13_dependency_units_appear_only_with_advanced` | Its strongest claim is `advanced.len() > normal.len()`. It never asserts that normal-mode top-level units are a **subset** of advanced-mode ones. One line would have caught 63→62 (F-03). |
| `invariant_20_no_provider_deadline_is_written_into_source` | Scans **only `crates/policy/src`**, only for the literals `2027`–`2030`. INV-20 names four categories — tiers, deadlines, **prefixes**, package knowledge — across five crates. |
| `invariant_11` | "Type-level" is a narrow excuse. Sound for the policy crate, but INV-11 is about primary-provider selection, implemented in the **planner** (`StepKind::SetPrimary`, `primary_after`), which the test never touches. |
| `invariant_15_analysis_is_a_pure_function_of_the_snapshot` | Two calls, same process — near-vacuous alone. Redeemed entirely by the end-to-end version. |
| `invariant_15_a_replayed_reader_never_consults_the_live_filesystem` | Covers `which()` only. `canonicalize` at `homebrew/mod.rs:767, :885, :897`, `macports/mod.rs:730` and `read_dir` at `macports/service.rs:64` are unreached under replay by a coincidence of call order, not a guard. |
| `invariant_17_symmetric_rules`, `invariant_17_every_rule_individually_follows_the_rename` | **The strongest tests in the repository** — rule-by-rule rename over neutral ids. They are also why F-02 and F-03 survived: they prove the *policy* layer is symmetric, which is not where the asymmetry is. |
| `invariant_01`, `invariant_06`, `invariant_08` (policy), `invariant_12`, `invariant_14`, `invariant_16`, `invariant_18`, `invariant_19` | Sound. `invariant_16_idempotent_read_commands` (asserts the state dir stays empty) is particularly good. |

---

## F-20 — Smaller findings, recorded without ceremony

- **No single-instance lock, no stub, no failing test.** `04-migration-and-safety.md:201`
  is a MUST. Nothing mutates yet, which is exactly how a MUST gets lost between
  passes — every other deferred capability at least has a type in place.
- **The Homebrew parser lacks bounds the MacPorts parser has.** MacPorts caps output
  bytes, rows (8192), name length and free text; Homebrew has **no row cap** and
  **unbounded free text** (`desc`, `caveats`, `homepage`, `license`, `tap` —
  `json.rs:527, :539, :626, :631`). A hostile third-party tap can put 60 MiB of
  `caveats` into a snapshot the user is invited to share. *Closed and worth
  recording:* everything that becomes a **path component** is validated on both sides
  (`checked_component`, `json.rs:62-86`), so path traversal through a package name is
  not reachable.
- **Casks are `TOP_LEVEL` by graph inference; the model says `UNKNOWN`.**
  `domain/src/ownership.rs:72-82` folds "no intent record" into `TopLevel`, and
  `classify_ownership` **cannot return `Unknown`**. `.claude/rules/adapters.md`,
  `AGENTS.md` and SE-042 all say casks are `UNKNOWN` and *never* graph-inferred.
  Observed: 9 casks on this machine (16% of 56 top-level) enter the default
  destructive-workflow surface on `authoritative: false` evidence. The inference is
  at least *recorded* as non-authoritative, which is good engineering — the defect is
  that documentation and code state different rules. See SE-046; scope change makes
  it the owner's call.
- **Domain enums rendered to users through `{:?}`.** `cli/src/app/human.rs:139`
  (support tier), `:108` (translation), `:353` (`step.kind`), `:379` (a verification
  struct's `Debug`). `locales/en.toml:143-146` already defines the four `support.*`
  keys — all unreferenced — so an `UNSUPPORTED` tier prints `unsupported` instead of
  the catalogue's deliberate "Not supported", and the column never pseudolocalises.
  `wiki/Architecture.md:101` claims *"Nothing is displayed from the lower layers"*.
  Plus **18 orphaned English keys** and `human.rs:140` composing
  `"{} ({} top-level)"` in code, which `06-cli-and-ux.md:121-122` forbids as a MUST.
  `check-i18n-parity.sh` passes because it checks two of three directions, never
  `en.toml → code`, though its header claims it fails on an orphaned key.
- **The `Stop` gate degrades silently, and it misled the previous pass.**
  `.claude/hooks/stop-gate.sh:122` guards `validate.sh` with `command -v cargo` and
  still exits 0 when absent. The previous report states as fact that this machine has
  no toolchain and that no coverage figure was observed; today `cargo`, `rustc 1.98.0`
  and the pin are all present and `validate.sh` passes in full. A gate whose silent
  path and successful path are indistinguishable is not a gate.
- **Hardcoded prefixes in `registry.rs:31-36`** — `/opt/homebrew`, `/usr/local`,
  `/opt/local` in a `match`, with source and read date in a doc comment rather than in
  `data/policy/*.toml`. INV-20 names *prefixes* explicitly. Mitigated by
  `prefix_override`, and no current test could catch it.
- **`provider-commands.toml`'s `executable` can escape its prefix.** `planner.rs:372`
  does `prefix.join(&e.executable)`, and `join` with an absolute argument discards the
  base. `load()` validates only that `source` and `read_on` are non-empty. The file is
  `include_str!`-embedded so it is not runtime-attacker-controlled — but it is exactly
  the file where a plausible edit would read as a fix, and three lines of validation
  turn a code-review question into a test.
- **Plan placeholders are indistinguishable from executable commands.**
  `planner.rs:268` emits a relative program; `:371` and `:376` emit `<prefix>` and
  `<operation>` strings. `check_ordering` never looks at commands, so a plan can
  serialise, diff, review and pass every check while containing commands no executor
  can run.
- **Predictable temp-file name on user-supplied paths.** `atomic.rs:109-118` is
  `.{stem}.{pid}.{seq}.partial`; fine inside the `0700` state directory, but
  `write_atomic` also serves `snapshot --out` and `doctor --report`. Denial of service
  only — the code correctly refuses to delete a file that is not ours — and a
  randomised suffix removes it.
- **Timeout kills the child, not the process group.** Documented honestly at
  `system.rs:198-209` and mitigated by `READER_DRAIN_TIMEOUT`. Acceptable, but it is a
  dependency decision (`libc`) and belongs in an ADR rather than a code comment.
- **`resolution_for` reads across providers.** `analysis.rs:383-389` scans every
  provider's resolutions and returns the first hit in map order. Safe today only
  because capture files each executable under exactly one provider — and `source_for`
  consumes the result.
- **`NO_ACTION_ALREADY_THERE` is misnamed** — its reason id is
  `NO_ALTERNATIVE_PROVIDER_PRESENT`, which is what it means. Behaviour correct, name
  misleading in `explain`, whose job is clarity.
- **`explain` lists non-fired rules by identifier only**, with no indication of which
  fact stopped each. Useful as far as it goes; a user debugging an unexpected
  recommendation still has to read the rule source. Judgement call.
- **Four undated provider claims**, all in
  `wiki/Understanding-Bottles-and-Source-Builds.md:72, 76, 83, 101`, including the
  strong negative *"`HOMEBREW_BUILD_FROM_SOURCE` does not exist"*. Everything else in
  the wiki carries source and read date.
- **Wiki `../docs/...` links will break on publication** — a GitHub wiki is a separate
  repository. `wiki/README.md:52-61` lists three publishing constraints and not this
  one, which is the one that will bite.
- **Two guard false-positives hit during this review.** A `plan` invocation was blocked
  because a *MacPorts package name* matched a tool name, and a **read-only grep written
  to verify INV-06** was blocked by the INV-06 guard. Neither was reworded to evade the
  guard; the documented remedy (pattern in a file) was used both times. This is the
  third consecutive pass to report guard false-positives on read-only or documentation
  work — the pattern that eventually produces a dishonest workaround.

---

# What is right about the foundation

The next pass needs to know what it must not break. Each of these was checked, not
assumed.

**The layering is real, not aspirational.** Exactly one `std::process::Command` site
in the whole workspace (`process/system.rs:185`), with `.env_clear()`;
`check-layering.sh` asserts the dependency edges from `cargo metadata` rather than
from a diagram; zero elevation tokens in shipped source. No `HashMap`/`HashSet` in
domain, policy or application, so determinism is structural.

**The policy engine is genuinely pure.** No I/O, no clock, no environment anywhere in
`crates/policy/src` — an impurity sweep returned only the domain type
`CommandLineTools`, a false positive on the word. `today` is injected as a fact. That
is what makes INV-14 and INV-15 hold.

**INV-15 is proven, not claimed.** Two `--from-snapshot` runs byte-identical across
184 KB; the fixture matrix re-proves it over six fixtures and 157 checks on every
run. Designing capture and analysis as separate functions so `--from-snapshot` and
`--replay` exercise the *same* code path is the best structural decision in the
project.

**Truncation is treated as corruption.** `Invocation::require_success`
(`process/mod.rs:191-204`) turns a truncated read into `ProviderOutputChanged`, and
the reasoning at `:182-190` — that a truncated line-oriented inventory parses
*perfectly* into a silently short inventory — is the best piece of security reasoning
in the codebase. That class of bug is closed.

**Ownership is sourced from the providers' own records.** `installed_on_request` and
the `requested` pseudo-portname, with authoritative evidence recorded per row; `leaves`
is used only for orphan candidates and never substituted for intent. There is **no
graph inference anywhere the provider has a record** — the cask case (F-20) is the
no-record case only, and even it is flagged non-authoritative.

**The state directory is the best-defended area.** `write_atomic` opens with
`create_new` + `mode(0o600)` — `O_EXCL` at creation, not `chmod` afterwards — then
`sync_all` and `rename`, which does not follow a final symlink. `state_dir.rs` refuses
a symlink, refuses a non-directory, and checks ownership via `symlink_metadata` before
touching permissions. `effective_uid` reads the uid without `libc`. The boundary that
is *not* guaranteed is documented rather than glossed.

**Elevation fails safe.** `planner.rs:382-389` maps anything not literally `"none"` or
`"required"` to `Elevation::Unknown`, and `:334` counts `Unknown` as elevated. Unknown
is treated as dangerous — the correct default, and the opposite of the usual mistake.
(F-10 is the cost of that default being reached by omission, not an argument against
it.)

**"No telemetry" is verifiable.** No `reqwest`, `ureq`, `std::net`, `TcpStream`,
`UdpSocket` or `curl` anywhere in `crates/*/src`. The one URL-building site
(`macports/availability.rs:36-38`) gates on `is_safe_name` and returns a `String`
nothing fetches.

**Peer-provider balance holds, measured.** The two migration pages run 1065 and 944
words, nine sections each, structurally parallel, each with exactly one section the
other lacks. The peer position appears in the opening paragraph of `README.md` and
`wiki/Home.md` and again under its own heading.
`wiki/MacPorts-to-Homebrew-Migration.md:10-11` explicitly invites a bug report if the
direction ever reads as an afterthought. The documentation keeps a promise the
planner (F-02) does not.

**The policy layer's symmetry testing is exemplary** — `invariant_17_symmetric_rules`
renames providers to neutral `alpha`/`beta` ids and re-runs every rule individually.
That is the right technique; it should be applied one layer up.

**`explain` is the right product.** Facts, the rules that fired with reasons, and the
rules that did not. For a tool whose value is judgement, exposing the judgement is
correct, and it works today on a real machine.

**Zero broken internal links across 105 markdown files**; 20 of 20 invariants carry a
test named after them. The discipline is being kept, not merely described.

**The dated-research design paid a return.** F-2 and R-9 were written by earlier
passes for other purposes and survived being leaned on by decisions they were not
written for. The `source` + `read_on` requirement on every `data/policy/*.toml` entry
is what let this review re-verify the Intel timeline in one fetch and find the
discrepancy in the prose rather than in the data.
