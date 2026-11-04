<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part H — the four remaining destination checks (DP-051 (a))

Plan written 2026-11-04, at `9b8d68d`, for `prompts/16-complete-mutating-product.md` §5 H. Reviewed by
`spec-guardian` and `security-reviewer` before any code; their review is the
approval (§4). Facts: R-25 (integrity), R-26 (ownership), R-27 (linkage), R-28
(liveness). The admission of a probe is its own note,
[`16-liveness-design.md`](../16-liveness-design.md).

## The shape of the answer

Today a check on `VERIFY_DESTINATION` either passes or halts. DP-051's "reported
unchecked" needs a third answer, and the whole part follows from that:

```text
CheckAnswer = Holds | Fails(named cause) | Unchecked(named cause)
```

- **`Fails` halts the step**, as a failed read does today. The source is kept.
- **`Unchecked` does not halt.** It is recorded per check and per executable. The
  source is **never removed**, and the run ends `COMPLETED_WITH_UNCHECKED`.
- "Could not tell" is never `Fails` and never `Holds`.

**A check with `Unchecked` is a different disposition, not a field on `Verified`.**
Every `matches!(.., Disposition::Verified { .. })` would otherwise keep compiling and
fail open: `destination_verified_before` (`executor.rs:1028`), `:1359`, `:1656`, and
the source is removed. So:

- On `VERIFY_DESTINATION`, `Verified` is written only by the one judge, and requires all four `Holds`. `judge_reads` keeps `Verified` for `VERIFY_REMOVAL` only, and `certify_destination` stops using its terminal arm. Enforced by the test and the mutant, since `Disposition` is a public enum.

- `Disposition::VerifiedWithUnchecked { evidence, unchecked }` is the new one, from one
  judge whose input requires all four answers. **One disposition per step, written after
  every judge has answered**, so nothing records `Verified` from the two read-answered
  checks alone (`certify` returns it at `:1481` today).
- Those wildcards become explicit matches, and every other non-verified record still
  halts `UnexecutedMutation`.

## The four checks

Each row is one rule for both providers (INV-17), with its two reads and the
asymmetry declared.

### `PROVIDER_INTEGRITY_CHECK` (R-25)

**Neither provider offers a read-only integrity check.** R-21 refused every verb;
R-25 adds `brew verify` (fetches, needs the network). So the check is ours, and
spec 04's "the provider reports ... where the provider offers one" is reworded.

| | MacPorts | Homebrew |
|---|---|---|
| Read | `port -q contents <port>`: a stored registry record | `brew list --formula <f>`: **`find` on the keg, at call time** |
| Rule | every listed path `stat`s (missing and dangling are named separately) | the same, plus each `runtime_dependencies` name of the `linked_keg` entry is in the inventory, plus a receipt in the keg |
| Peer of the dependency rule | none: `info --index` is the current index, not the record. A declared asymmetry | |

**R-25's main result, stated in the output:** on Homebrew "every listed path exists"
cannot fail under `lstat`, because the list is the disk. **Absence of a file from a
Homebrew keg is not detected**, and the SE entry says that sentence. `--json` carries a
neutral `basis`, with a catalogue key per basis and no provider name above `adapters`;
a test asserts the Homebrew basis never renders a completeness claim. A Homebrew formula
passes on this weaker basis rather than staying unchecked for ever, which would make
MacPorts-to-Homebrew never remove (an INV-17 defect in effect). **The owner confirms this
reading: DP-057**, and part L names it. The check never asserts content integrity,
provenance, or that the package runs.

**The Homebrew rule, whole (R-25):** the `installed[]` entry whose version equals
`linked_keg`; a receipt in the keg and more than its metadata (at least one real entry);
each `runtime_dependencies` name matches the inventory by `name`, `aliases` or `oldnames`
(no `stat`; versions not compared, 212 of 939 healthy edges differ). `null` dependencies are `Unchecked`; `[]` is an honest none (94 here). An empty file list is `ProviderOutputChanged`.

`Fails`: a missing or dangling path (`stat`, with the two named apart); a missing
dependency; a keg with no receipt or only metadata. `Unchecked`: any other `stat` errno, a
failed read, a list over the cap (a per-package `ProviderOutputChanged` maps here), a listed
path outside the prefix (`LISTS_OUTSIDE_PREFIX`, not `stat`ed; application and service ports
stay unchecked), `NOT_OBSERVED`. Exclusions: none, since excluding docs would hide 51.2% of
MacPorts' entries. Nothing is `stat`ed
before `FilesystemScope`, the parser, a length bound, and component-wise `is_under`.

### `EXECUTABLES_OWNED_BY_DESTINATION` (R-26)

**The question is about the placed entry `<prefix>/{bin,sbin}/<leaf>`,** never the
keg's list path: `openssl@4` ships `bin/openssl` while `/usr/local/bin/openssl`
belongs to `openssl@3`, so the list path would pass the wrong package.

| | MacPorts | Homebrew |
|---|---|---|
| Owner | `port -q provides <one path>` (active only, case-insensitive: compare the answer) | The realpath lies in `<cellar>/<name>/<version>/` (`Keg.for`), an adapter rule |
| Version | The `(active)` row of `port -q installed <name>` | The keg's version component |

Both pin "the active version `v` owns this path". **SE-107's L2, as far as it can be
closed:** on a step with `PACKAGE_NEWLY_PRESENT`, the paired snapshot must show exactly one
version present, active, equal to the owner's; on a package the destination already held,
the active version must equal the snapshot's. **An inference, stated as one:** no provider
ties a version to a run.

`Fails`: no entry; another package's; more than one version after a fresh install; the
active record not `v`; an owner version other than `v`. `Unchecked`: a failed read, a one-hop
link whose realpath leaves the keg (`OWNER_LEAVES_KEG`: `npm`, `npx`, 2 of 1091; an accepted
cost, the source is kept), `NOT_OBSERVED`. `provides` currently conflates a failed read with
"not owned", `parse_provides` is lax, and `owner_of` reads no version and accepts the
Caskroom: all fixed on this path, without changing `RESOLVES_TO`'s judge. Answers are compared,
never joined into paths. Results are worded "owned by X when read at T". Spec
04's "replacing that rule with an owner query is part H's" is withdrawn, and check 3's
and 05's "asking the provider" get the sentence `VERIFY_RESOLUTION` already has: no
Homebrew read answers ownership.

### `NO_CROSS_PROVIDER_LINKAGE` (R-27, INV-07, INV-08)

**An in-process reader, not `otool`.** A decision within this part's authority (no
dependency, no `unsafe`, no spawn, no allowlist growth, so §3 items 3, 5 and 6 are not
touched), recorded in the SE entry and as DP-056, which blocks nothing, because R-21 named it
the owner's trade-off. R-27 observed what R-21 could not: `otool` exits 0 on a non-object
file, `-L` shows only the host slice (a crafted cross link in the other was hidden), it writes `xcrun_db` on a cache miss, and, in one test with a deliberately bogus `SDKROOT` (attributed by timing and stderr, not proof, and not reachable under the sanitised environment unless `extra` carries it), an install dialog and a spawned `sh` and `xcodebuild`, with `xcode-select -p` succeeding. "Reversible"
holds only while the switch is off: a false `Holds` once removal ships is not, so part L names
the reader and its limits.

**`crates/adapters/src/macho.rs`:**

- Two pure stages, no `unsafe`, no dependency: the fat table to byte ranges, then one window per
  slice. A 200 MB file is never read whole. `#![deny(clippy::indexing_slicing,
  clippy::arithmetic_side_effects, clippy::unwrap_used, clippy::expect_used)]`: a panic
  mid-migration strands a run. Checked `offset + size`; `cmdsize >= 8`; strings cut at the first NUL
  inside `cmdsize`; bytes compared, never `from_utf8_lossy`.
- Caps on `nfat`, `ncmds`, `sizeofcmds` (5.3 KB seen; 1 MiB) and bytes per file. Over any is
  `Unchecked`.
- Every slice; `LC_LOAD_DYLIB`, weak, reexport, lazy, upward, `LC_LOAD_DYLINKER`, `LC_RPATH`,
  `LC_DYLD_ENVIRONMENT`; `LC_ID_DYLIB` excluded by kind, never position. An unknown `LC_REQ_DYLD`
  command, or an unknown file type, is `Unchecked`. `MH_OBJECT` and `ar` are skipped with a count.
  `0xcafebabe` whose table does not fit is `Unchecked`, never "not Mach-O": a crafted fat header must
  not hide as a Java class.
- **Opening (§3 item 10):** one resolution, `O_NOFOLLOW | O_NONBLOCK`, regular-file on the handle, the `crates/adapters/src/persistence/snapshot_store.rs:366-374` rule; a listed FIFO must not block.
- **Tests:** the truncation test, plus a deterministic mutate-every-byte loop over a small specimen (no
  dependency; R-27 finding 26 asks for fuzzing); a one-off comparison of the Rust against `otool` on
  this machine's files, recorded, since 2224 of 2224 was the Python prototype's.

**What the check examines:** every `MH_EXECUTE`, `MH_DYLIB`, `MH_BUNDLE` in the package's whole file
list, by magic (executables alone would be vacuous for 71 packages); symlink entries of its own list by `lstat`, testing the target lexically and never following it to
read; **and a bounded file-level walk through the destination's own files**, since "removal is only safe
if the walk is clean" (R-27 finding 14: A links a destination dependency that links the source).

**A load-command target is resolved to its canonical path, and "what counts" applies to that path as well
as the lexical one:** `O_NOFOLLOW` guards only the last component, so a directory link inside the destination
into the other prefix would otherwise be examined as the destination's own, and answer a false `Holds`. Only
the canonical path is opened, as above; `@loader_path` expands from the loader's canonical path. Test: that
link; mutant: lexical test only.

**Bounds:** `MAX_WALK_FILES = 4096` and `MAX_WALK_BYTES = 256 MiB` (R-27's worst closure is 340 files), counting
the own list's magic reads too, with a visited set on `(dev, ino)`. **No wall clock** (AGENTS.md; an adapter reads
none), and no cancellation token is wired to adapters, so the numbers are the bound. Over either, or a load command
reaching a destination file not examined, is `Unchecked`.

**What counts.** "Under" is component-wise, after normalising `..` and `@loader_path`, and
case-insensitive (`/opt/local/bin/XZ` answers `xz`, R-26; an upper-case prefix is inferred, not observed), with nested prefixes by longest match. "Another
provider's":

- a prefix a provider was **detected at**, other than the destination's: `Fails`;
- a candidate in `provider-prefixes.toml` where no provider was detected: `Unchecked
  (UNDER_UNDETECTED_PREFIX)`, because `/usr/local/lib/x.dylib` hand-built on a machine with no Homebrew
  is not Homebrew's.

An absolute shebang counts the same way.

**The claim is reworded.** "No *new* linkage" cannot be claimed: the snapshot holds no baseline. It is
"**no reference to another provider's prefix present at verification**". `Fails` halts in both cases, with
a distinct cause when the package was held before this run (`LINK_PRESENT_BEFORE_THE_RUN`, worded as such:
R-27 finding 9), because a source is not removed under it and a primary is not moved to it. The check is
evidence about INV-07, not the invariant, and no check reads one provider's artifact as satisfying the
other's (INV-08).

`Unchecked` otherwise: a malformed Mach-O, an unreadable file, an `@executable_path` rpath that cannot be
resolved lexically, an `@rpath` name with no candidate from the file's own `LC_RPATH` (zero candidates must not read
as clean), `NOT_OBSERVED`.

**Part H item 3 for scripts (DP-057, default the conservative one):**

- No loadable file and no `env` shebang on the package's executable list: `Holds`, `basis:
  NO_LOAD_COMMANDS`, a declared reading of DP-051's sentence: nothing was left unevaluated.
- An `env` shebang on the executable list: `Unchecked (INTERPRETER_RESOLVED_AT_RUN_TIME)`. R-27 finding 12
  calls it indeterminate, and the probe table excludes those scripts anyway, so a `Holds` here would buy
  nothing and cost honesty. One outside the list is counted in the `basis`.

**What it cannot see, in the docs:** `dlopen`, static links, a user-built binary outside the file list.

### `LIVENESS_PROBE` (R-28)

[`16-liveness-design.md`](../16-liveness-design.md). The plan's check becomes
`LivenessProbe { executable, argv }` from the row. **A skip is a plan shape of its own:**
a `VerificationCheck::LivenessProbeSkipped { executable: Option<...>, cause }` variant, so the compiler forces
it through `evaluable_in_this_build`, `answers` and `CheckKind`: one per listed executable with no row, and one with
no executable, valid only when the list is empty or unknown, so a no-executable package is never vacuously passing:
DP-051 says no curated probe is unchecked. A skip answers `Unchecked` and never `Holds`.
`liveness_probes_plus_recorded_skips_equal_the_list` becomes true for the first time (its comment says "this
build's plan shape records no skip"). Spec 04's "default `--version`", "safe" and "skippable" go. The plan
warns at plan time when a skip is known, since a removal passed over must not surprise anyone.

## Where it lives

- **`domain`:** `CheckAnswer`, the causes, `ProbeObservation`, the facts each judge takes.
  `ProviderRead` gains `destination_facts(...)`, default `NotObserved`, which starts no program.
  `DestinationProbe` is a separate trait, from `registry::destination_sites` only (the design).
- **`application`:** the pure judges, one per check, over facts; they never branch on a provider id, by test
  (INV-17). `certify` becomes the one step that runs reads, then judges, then writes the one disposition.
  `next_destination_question(plan, position, answers)` is the pure sequencing: the shell asks only what it hands over.
- **`adapters`:** the reads, `macho.rs`; `process/probe.rs` (staged).
- **`cli/executor`:** a `DestinationSite`, as `ResolutionSite` is: sequencing only.
- **`policy`:** loads `liveness-probes.toml` for the planner.

**The gate.** It refuses a `VERIFY_DESTINATION` missing any of the four, where "missing" for liveness is
the list's set ≠ the probes' and skips' set, a duplicate refused (`DESTINATION_CONTRACT_INCOMPLETE`: S's review, the plan-only analogue of V2), and one
whose probe `argv` differs from the row (with the SE-092 note in `.claude/rules/executor.md`: the gate now reads
one thing besides the plan, compiled data). **`CHECK_HAS_NO_ANSWERING_READ`** (`execution_gate.rs:833`) and
`NoPassingRead` (`executor.rs:1438`) would fire on all four: a closed `is_site_answered(check)`, true of exactly
the four and the skip on `VERIFY_DESTINATION`, leaves them out, and the two read-answered checks stay under the rule, as today.
`RESOLVES_TO` escapes by step kind, which does not apply. `CHECK_NOT_EVALUABLE_IN_THIS_BUILD` stays for the rest.

**The order inside a step.** The reads; ownership; integrity and linkage; liveness only for a leaf ownership
answered `Holds`, when no check answered `Fails` and no file was malformed. Any `Fails` stops what follows.

**The removal.** `destination_verified_before` false halts today (`executor.rs:958`); a positive match on
`VerifiedWithUnchecked` gives a pass-over, `Disposition::SourceKeptUnchecked`, and nothing else does. After it, on a
positive match of the preceding record, `VERIFY_REMOVAL` and `CLEANUP_REVIEW` record the same disposition (`PACKAGE_ABSENT`
would fail on a source still there), `history.v4` validates it per step kind, and `outcome_of` accepts only that exact
shape, so `NotPerformedInThisBuild` cannot produce `COMPLETED_WITH_UNCHECKED`. `resolution_held_before` is read as it is today, so a resolution that did not hold still halts. **S's re-read before a removal has its own judge,** not the four-answer one: the reads plus a fresh check 3 per
executable, through the site, `Ok` only if every read passes and every answer is `Holds`; `Unchecked` or `Fails` gives
`DestinationNotReverified`, a halt, with nothing dispatched (`invariant_01_the_reread_uses_the_verify_steps_own_commands`
stands for the reads). **Checks 2, 4 and 5 are not re-run: that window is declared.** `check_ordering` is untouched. **`SET_PRIMARY`** dispatches nothing in this build (the gate refuses all but two
decisions); part I inherits the rule: ownership `Unchecked` means no shim.

**The outcome.** `RunOutcome::CompletedWithUnchecked { unchecked, source_kept }`; `--json` id
`COMPLETED_WITH_UNCHECKED`; catalogue keys in `en` and `pt-BR`. In `outcome_of`, **a halt or a step not reached outranks it**,
and the unchecked list survives on `PARTIAL_SUCCESS`. **Exit code 3:** a kind of partial success (06), so a script testing for
0 does not read it as plain, and no new `ErrorId`. With at most 13 rows nearly every default run exits 3 once K makes coexistence the
default: correct under the ruling, said in the docs and in part L.

**History.** `StoredEnding` is `deny_unknown_fields`, and `history.v3.md` says this case is not additive: a new ending and a new step
result are **`history.v4`**. `RunStatus::of` maps `COMPLETED_WITH_UNCHECKED` to `PARTIAL` when a mutation was dispatched and `NO_CHANGE`
otherwise, **never `SUCCESS`** (spec 04, "Partial success is its own exit code"; `invariant_03_*`). `recognise_retained_run` still refuses anything but `SUCCESS`, on
purpose: a re-run re-verifies an unchecked destination rather than recognising it as done (`invariant_16_*`).

**Schemas.** `flag` to `argv` is a rename and a type change, and the skip is new: **`plan.v2`**, a bumped `PLAN_SCHEMA_VERSION`, and a plan
carrying `flag` fails closed at the existing `SCHEMA_VERSION_NOT_THIS_BUILDS`. The new outcome and disposition: **`migrate.v2`**, since that enum
has no `UNKNOWN`. **`restore.v2`**, because `restore.v1` embeds "the `plan.v1` plan, unchanged" and its `outcome` is "exactly
as `migrate.v1`"; `history-show` is corrected in the same change (it already says `history.v2`, stale since F-3). The
schema-version test constructs the number, since no plan-file input exists. **INV-20:** the table takes part in the policy
version (the newest `read_on`) and the staleness notice, by `invariant_20_*`.

**The tripwire (item 5).** `the_switch_needs_the_destination_contract_evaluable` must not go quiet because a site that observes nothing made the
checks "evaluable". So a closed `observed_by_this_build(check)` flips per check only when its real observer lands (H-2 for three, H-3 for liveness), and
the test asserts `!DISPATCHES || all four observed`, over a fixture **with executables**, so it sees `LivenessProbe`. The `assert_eq!(not_evaluable,
written)` becomes that per-check statement, and the `read_only` helper (`crates/cli/tests/executor.rs:414-427`), which narrows by evaluability, narrows by observability
instead, so it does not silently widen. `a_check_with_no_observation_is_unchecked_never_holds` stands beside it. **With H/01 unapplied this keeps failing
under the switch**, which is exactly what it is for.

## Slices

| Slice | Branch | What | Depends on |
|---|---|---|---|
| H-0 | `wo16/H-1-checks` | Docs only: the four notes, the design, this plan, DP-056, DP-057, the SE entry, and the rewording of 04 (the checks, the failure table, partial success, marked not built), 05's ownership MUST, `analysis.v1` and `plan.v1`'s check table by a forward note, ADR-0006's amendment. 06 stays honest once 04 defines the kind of partial success, so it is not edited | Reviews |
| H-1 | `wo16/H-2-answer` | `CheckAnswer`, the dispositions, the outcome, `plan.v2`, `migrate.v2`, `restore.v2`, `history.v4`, the gate, the removal rule, the tripwire, with a site that observes nothing, so all four answer `Unchecked`. **Words that claim what SE-113 retracts:** the catalogue's `verification.*`, `migrate.check.*`, `reason.verify_destination` in both locales; the two wiki migration pages; 05's "`CheckNotEvaluableInThisBuild`" (the refusal becomes `MutationNotInThisBuild`), 04's "one probe per name" and "stay unevaluable", `plan.v1` obligation 6 | H-0 |
| H-2 | `wo16/H-3-reads` | Integrity, ownership, `macho.rs`, linkage, `StateDir`'s `probes`, `registry::destination_sites`, the data file (rows re-qualified first), the planner's rows | H-1 |
| H-stage | `wo16/H-4-stage` | Docs only, **after H-2 merges**: `16-staged/H/01` built and tested in a scratch worktree on that `main`; `H/02`, the capture of the ownership reads (`port -q provides`; `port -q installed <name>`), since `capture-fixture.sh` has none and `test-gates.sh` pins its forms; the rewording that takes effect with H/01 of 04's "A command spec carries no directory", "Every invocation records" and "one of the six". **The part is then `waiting on owner`** | H-2 |
| H-3 | `wo16/H-5-probe` | Wires the admitted probe in, flips `observed_by_this_build` for liveness | §3 item 5.4 on H/01 |

**No frozen hunk in H-1 or H-2:** every read named is already allowed, and H-2 adds no argv form. `git diff fd443ef..HEAD` on the frozen paths is run on each PR.

**Part K does not start while H-3 waits.** K's item 3 needs all six checks, and "merged (owner item open)" would stretch F's precedent, whose open item was a test capture, not a check of the contract. So when the staging merges, the session ends with the "Needs the owner" line (§4.3), unless the owner explicitly lets K go first, in which case K's tests inject the site's answers and the tripwire stays failing.

**What is testable without a recording.** The judges, over facts. The filesystem half, in a scratch directory. The parsers, on the texts R-25 and R-26 observed. **Whole-binary recordings are owner-blocked:** no recording holds these reads, and `capture-fixture.sh` aborts on `ARG_MAX` after `brew list --formula cmake`, an open owner item. Under `--replay` a `VERIFY_DESTINATION` halts before any read (SE-107 C1, `plan.v1` obligation 6), so the whole binary ends halted, and `NOT_OBSERVED` is tested at the site and judge layers. **No recording of a contaminated binary can exist:** detection is shown on bytes built in the test, and said so.

## Mutants

Each `Fails` read as `Holds`; each `Unchecked` read as `Holds`; the new variant counted as verified; a skip as `Holds`; an `env` shebang as `Holds`; `lstat` for `stat`; dropping `is_under`; the list path for the placed entry; `LC_ID_DYLIB` counted; only the first fat slice; the destination's own prefix counted; case-sensitive "under"; an unnormalised `..`; a dropped walk; `destination_verified_before` accepting the new variant; `COMPLETED_WITH_UNCHECKED` as `SUCCESS` or exit 0; a halt outranked by it; the gate accepting a missing check; `destination_still_verified` accepting the new variant; `next_action` dispatching `VERIFY_REMOVAL` after a pass-over; a probe from a read-only path; a probe before ownership or after a `Fails`.

## Invariant tests, by name

`invariant_01_*` an unchecked destination is never removed, both directions; `invariant_03_*` the status mapping; `invariant_07_*`, `invariant_08_*` linkage; `invariant_10_*` the probe environment; `invariant_15_*` replay and from-snapshot; `invariant_16_*` the retained run; `invariant_17_*` one judge, both directions; `invariant_19_*` a stateful row is a load error; `invariant_20_*` every row dated.

## INV-17

One `CheckAnswer` model and one judge per check, in both directions. Declared asymmetries, each the provider's own: Homebrew's list is generated and MacPorts' is a record; Homebrew's owner is a rule and MacPorts' a query; only Homebrew has a dependency read to check; `LISTS_OUTSIDE_PREFIX` arises only under MacPorts and `OWNER_LEAVES_KEG` only under Homebrew. The initial table is 6 Homebrew and 7 MacPorts rows; coverage differs as data, not as a code path (R-28 finding 25). The asymmetries above do have an effect, and each is reported rather than averaged away.

## Reviews

| Reviewer | Reviewed at | Verdict | Resolution |
|---|---|---|---|
| `spec-guardian` | uncommitted draft on `9b8d68d` | Approve with changes, B1-B11, D1-D8, R1-R7: tripwire would go quiet; `Verified` wildcards fail open; `history.v4`, status, INV-16; breaking schema unbumped; no skip shape, `CHECK_HAS_NO_ANSWERING_READ`; `env` as `Holds`; weak basis; `cited`; "by type"; staging too early; K must wait; transitive walk; wall clock | All in this revision |
| `security-reviewer` | the same draft | Changes required, P1-P9, S1-S3: the same two fail-opens by another route; no-executable package vacuously passes; symlinks and the walk; the reader's specification; one disposition per step; the passed-over removal through the rest of the plan; H/01 incomplete and too early | All in this revision |
| `spec-guardian`, round 2 | `11d8d50` | Approve with changes, 13: SE-113's format and counts; 05 and `plan.v1` still say what SE-113 retracts; unbuilt behaviour unmarked; `restore.v1` missed; the catalogue and wiki; passed-over steps unnamed | All in `H-0` |
| `security-reviewer`, round 2 | `11d8d50` | Pass with changes, 1-8: the walk's false `Holds` through a directory link; the re-read cannot use the four-answer judge; the skip and `is_site_answered`; no token exists, so numbers are the bound | All in `H-0` |
| `docs-auditor` | `f32b359` | 14 findings: the dialog overstated where the owner rules; `[]` read as a failure; a stale part F citation; spec 04 keeping a phrase R-25 rejects; undated claims | All in `e6a28ec` |
