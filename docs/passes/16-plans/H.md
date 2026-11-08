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
link whose realpath leaves the keg (`ENTRY_LEAVES_OWNER`, on both providers as built: `npm`, `npx`, 2 of 1091; an accepted
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
  `DestinationProbe` is a separate trait, from `registry::destination_sites` only (the design, since amended: "What H-3 owes").
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
answered `Holds`, when linkage answered `Holds` and no check answered `Fails`. Any `Fails` stops what follows.

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
version (the newest `read_on`), by `invariant_20_*`; not in the staleness date, as built (H-2 item 15).

**The tripwire (item 5).** `the_switch_needs_the_destination_contract_evaluable` must not go quiet because a site that observes nothing made the
checks "evaluable". So a closed `observed_by_this_build(check)` flips per check only when its real observer lands (H-2 for three, H-3 for liveness), and
the test asserts `!DISPATCHES || all four observed`, over a fixture **with executables**, so it sees `LivenessProbe`. The `assert_eq!(not_evaluable,
written)` becomes that per-check statement, and the `read_only` helper (`crates/cli/tests/executor.rs:414-427`), which narrows by evaluability, narrows by observability
instead, so it does not silently widen. `a_check_with_no_observation_is_unchecked_never_holds` stands beside it. **With H/01 unapplied this keeps failing
under the switch**, which is exactly what it is for.

## As built (H-1)

Deviations from the text above, each reviewed below:

1. **The `read_only` helper does not narrow by observability.** The contract gate refuses a `VERIFY_DESTINATION` missing one of the four, so no narrowed plan could pass it. Tests keep the whole contract and supply a test site answering `Holds`; a run through the gate with `NothingObserved` ends `COMPLETED_WITH_UNCHECKED`. What "does not silently widen" asked for is carried by the tripwire and `a_check_with_no_observation_is_unchecked_never_holds`.
2. **The argv comparison has no table yet.** `curated_probe_argv` answers `None` in a shipped build, so the gate refuses every `LIVENESS_PROBE` (`PROBE_IS_NOT_THE_TABLES`) and the planner writes only skips, until H-2 brings `liveness-probes.toml`. A test build, and only one (the `test-support` feature, which `build-release.sh` refuses), has one row, for the name `mpb-fixture-probe`, no package's: that is how `crates/cli/tests/executor_wiring.rs` reaches a removal through the gate.
3. **`MutatingRun` gained `resolutions`**, `Unresolved` in the product. A plan reaching `VERIFIED` must have an executable list, whose decision needs a `VERIFY_RESOLUTION`, which the mutating loop could not answer. No behaviour of a shipped build moves.
4. **The causes:** every `CheckFailure` the plan names exists now; `UncheckedCause` holds only what H-1 can produce. H-2 adds its own to `migrate.v2` and `history.v4` with no version bump, since nothing has been released under either; `migrate.v2` says so. After a release a new cause is a new version.
5. **The tripwire was renamed** `the_switch_needs_the_destination_contract_observed`, and the contract also requires `PROVIDER_REPORTS_ACTIVE`, and `PACKAGE_NEWLY_PRESENT` when the plan adds the package (both reviews of H-1: otherwise H-1 opened a shape with no read-answered check).
6. **Left for H-2, on purpose:** the note in `.claude/rules/executor.md` that the gate reads compiled data, since that data lands there; and `MutatingRun.destination` as a pre-built reference, which H-2 must make a closure called after the gate (executor rule 1) when a real site lands.
7. **A real consolidation cannot reach `COMPLETED_WITH_UNCHECKED` yet:** its command-less `ADD_DESTINATION` is `NOT_DISPATCHED_MUTATING`, so the run ends `PARTIAL_SUCCESS` with the unchecked list. As before H-1, when it could not reach `COMPLETED` either. Part K's to settle.
8. **`invariant_15_*` moves to H-2.** Under `--replay` a `VERIFY_DESTINATION` halts before any read (SE-107 C1), and `--yes` with `--from-snapshot` is a usage error, so no run in H-1 reaches the site. `a_check_with_no_observation_is_unchecked_never_holds` carries the substance, under its own name.
9. **Left as it is:** `.claude/hooks/stop-gate.sh:129` and its generated copy still point at `plan.v1.md`, a guarded file, and the frozen `crates/adapters/src/process/mod.rs` does too. `plan.v1.md` still exists and holds the same obligations.

## As built (H-2)

Deviations from the text above:

1. **The closure takes the lock and keeps nothing of it.** `run_with_mutations(cleared, sites, destination, io, home)`, `destination: FnOnce(&RunLock) -> D`, replaces `MutatingRun.destination`; `registry::destination_sites` takes `&RunLock` as proof and does not hold it, since a site that borrowed it could not be returned by a closure a test also implements. `run_mutating` is the only caller in the product, so the constructor stays in the shell, and the word scan bans it elsewhere.
2. **R-28's row 8 did not land**, so the table is 12 rows: 6 and 6, not the 6 and 7 INV-17 above expects. See R-28's last section.
3. **`MAX_WALK_FILES` counts every regular file of the list, as written**, so a package with over 4096 regular files answers `WALK_OVER_BOUND`. R-27's 340 was a closure of Mach-O files. Measured 2026-11-04 (R-27's last section): `WALK_OVER_BOUND` answers for 9 of 194 Homebrew packages and none of MacPorts' 18. The number stays.
4. **`basis` is `checkBases` on the receipt and in history**, a sorted set for the run, not a field per check. `Holds` carries no payload, and a per-check field would have reached 54 matches for a statement one set makes. `history.v4` is unreleased, so no version moved. `from_stored` refuses bases under another version, out of order, or with no `VERIFY_DESTINATION` that was reached. **An allow-list, `Halted` included (rounds 2, 3):** the first rule asked for a verified step, so a run halting on `Fails` after another check held wrote a record its own reader refused, which left the marker behind. The bases are true of the checks that did hold. `Verified`, `VerifiedWithUnchecked` and `Halted` qualify; `NotReached`, `NotPerformedInThisBuild` and `NotDispatchedMutating` answered nothing and do not.
5. **A relative shebang on the executable list counts as an `env` one**: `#! .` names nothing that can be tested, so it is `INTERPRETER_RESOLVED_AT_RUN_TIME` and not a `Holds`.
6. **A relative install name is `RPATH_UNRESOLVED`**, the loader's search, not a reference to nothing. Any `MachoError` is `MALFORMED_FILE`, the over-a-cap ones and Java classes included, so each also stops the probe; the gloss says "could not be read as an object". **Any linkage answer but `Holds` stops the probe (round 2):** whatever linkage could not tell may be what the program loads. The probe's entry names linkage's cause when ownership held, and `OWNERSHIP_NOT_TOLD` when it did not. `MALFORMED_FILE` and `WALK_OVER_BOUND` stay first in the judge, so neither is hidden.
7. **(Reversed by item 46.)** **An `@rpath` none of whose candidates exists is `RPATH_UNRESOLVED`, however a candidate is spelled** (round 2, which reverses round 1's exception for a destination-spelled candidate: a file that is not there tells nothing about what loads). One candidate that exists answers the name, since R-27 finding 3 asks whether any lies under the other prefix, not which loads. **An absent candidate under the system's roots also answers (round 3):** its dylibs live in the shared cache, which is `swiftformat`. An absent candidate elsewhere does not. An absolute reference into the destination that cannot be resolved is `WALK_OVER_BOUND`, including a weak one.
8. **Ownership on an unknown MacPorts name:** `provides` names no version, so the version is the `(active)` row's, and with no active row the judge answers `ACTIVE_RECORD_DIFFERS`. A MacPorts link is asked for the entry and then the final file. **INV-17:** a link leaving the prefix answers `ENTRY_LEAVES_OWNER`, the one provider-neutral cause Homebrew's leaving a keg answers (was `OWNER_LEAVES_KEG`). An empty `provides` means several things, so it is `Unchecked(OWNER_NOT_NAMED)`, the `select` case included (R-26 finding 2, source only); a non-empty other owner stays `Fails`.
9. **The adapter states the comparable version, and application appends nothing.** The first build appended `_<revision>` above the adapter, and it was not the safe direction this item claimed: measured on this machine, 19 of 194 formulae got a false `OWNER_VERSION_DIFFERS` (`git`, `coreutils`; `gts` became `0.7.6_3_4`). `OwnershipFacts::Owner.held` now carries the adapter's own `comparable_version` of the row: Homebrew's row version as it stands, which already names the keg directory, and MacPorts' `version_revision`. Nothing above the adapters says keg or formula. A destination held only inactive has no version to pin: `Unchecked(HELD_VERSION_NOT_TOLD)`. The planner counts that row as held, so it writes no `PACKAGE_NEWLY_PRESENT`, and a link already there is `LINK_PRESENT_BEFORE_THE_RUN`.
10. **`runtime_dependencies` is now optional in the parser**: `null` or absent parsed before as a document error or as `[]`, and now reaches the check as `DEPENDENCIES_NOT_TOLD`. The rest of the parse is unchanged. `oldnames` is parsed for the first time.
11. **`DestinationProbe` and `ProbeObservation` land with `NotRun` only.** The rest of the observation table is H/01's. The read-only loop keeps `NothingObserved`, so a dry run walks nothing.
12. **`~/Library` was searched by name, not whole:** a full walk did not finish in over an hour. R-28's last section says what was left out, which the design note's "`~/Library` searched" asks more of. **Rounds:** R-28's sweep had cold, warm, `syspath` and `ro`. The re-qualification dropped the cold/warm split and added the sandbox. Rows 11-13, added after the sweep, had never had an `ro` round; **round 3 ran it on all twelve landed rows** (scratch `chmod 555`, plain and under the sandbox), and all twelve passed. "What H-3 owes" below has what must close `~/Library` before any probe runs.
13. **The filesystem tests are in `crates/adapters/tests/destination_reads.rs`,** so the module is `pub` and exposes what they need: the stop gate's token scan covers non-comment code under `src/`, and they plant links.
14. **What this slice cannot show:** no run reaches the machine site in this build, since the gate refuses every plan, and no recording holds these reads. The judges, the walk, the parsers and each adapter's reads over a fake runner and a scratch prefix (`crates/adapters/tests/destination_adapters.rs`) are tested, on synthesised answers in the shapes R-25 and R-26 quote; the site's sequence on a live machine is not. `invariant_10_*` and the eleven `admit` refusals are H/01's.
15. **Probe rows do not move the staleness date,** against this plan's INV-20 line: they stay in the policy version and are excluded from `policyReadOn`, since a qualification of 2026-11-04 says nothing about how fresh the provider facts are, and it had moved every golden's date to hide 2026-09-17. The rows' `source` and `read_on` are still validated.
16. **One allowlist grew:** `-q installed <name>` in `macports/mod.rs`'s `is_allowed_argv`, the owner's version, a read the verify step already makes. `process/` and `provider-commands.toml` have no hunk.
17. **Reference spellings (review round 1).** A reference, rpath, interpreter, `LC_DYLD_ENVIRONMENT` path or link target is classified by its spelling, by its canonical path, and by `(dev, ino)` of each existing ancestor against each prefix, in the adapter; `PrefixTable` stays pure text. A firmlink (`/System/Volumes/Data/usr/local/...`), a Unicode fold (`/uſr/local/...`) and a plain link under `/private/tmp` no longer hide a prefix. The firmlink and fold tests skip on a volume without them.
18. **(Reversed by item 34.)** **`FILE_NOT_EXAMINED`:** a listed or reached file outside the destination and in no known prefix, or not a regular file, never gives `NO_LOAD_COMMANDS`. A relocated cellar therefore answers `Unchecked`, accepted in R-27. **Round 2 widens "reached":** any real file a load command names, however spelled, in no known prefix and outside the system's own roots, since another provider's relocated store looks exactly like this. **Round 3 corrects the roots** to `/usr/lib` and `/System/Library`: `/System` covered `/System/Volumes/Data`, the whole writable volume, and both reviewers proved a false `Holds` through that spelling. The firmlinks under `/System/Library` and `Templates` are excluded, by text in either case and by inode for any other fold; what was found is in R-27. **Round 3 extends the rule to shebangs:** a real interpreter or path-shaped word in no known prefix, outside `SYSTEM_PATH` and the roots, is `FILE_NOT_EXAMINED`, since `#!/outside/interp` was neither read nor flagged. The measured cost is in R-27's last section.
19. **Work bound.** Candidates are deduped before `canonicalize`, the queue by inode, and `MAX_WALK_REFERENCES` (65536) counts operations, setting `walk_over_bound`, so a quadratic `LC_RPATH` × `@rpath` or overlapping fat slices cost a stated bound. **A joined path of `PATH_MAX` or more is unresolved before any filesystem call or cache entry** (round 2): 256 KB of rpath was a `stat` per ancestor. **Every ancestor of a resolved path is charged to the bound (round 3):** a real tree 440 levels deep otherwise cost about 43 minutes at it.
20. **`..` against links:** the raw join is canonicalised with `..` intact, and if it disagrees with the collapsed text the answer is `SPELLINGS_DISAGREE` (precedent: `is_safe_path`). A link is judged by its whole resolution as well as its first hop, without opening it. A revisited inode upgrades to on-list, so an executable-directory link or hardlink to an `env` script counts; `bin -> libexec/bin` is compared by canonical directory (round 2). **`@loader_path//x` is `<dir>/x`, as dyld reads it:** round 1 read `/x`. All leading slashes go, for each token and in the `@rpath` join, so `//../../..` is measured where it leads; a longer word such as `@loader_pathology` is no token and says nothing. **An absent file through a link below a prefix** is measured through the canonical form of its deepest existing ancestor.
21. **`canonicalize` errors:** only `ENOENT`, `ENOTDIR` and `ELOOP` are no entry; any other is `STAT_FAILED`, on both providers.
22. **Deleted, not recorded:** `skipped_files` and `env_shebang_elsewhere` had no reader, and a count nobody reads is a claim nobody checks. The plan put them in the basis; the basis is a set of neutral statements and has no place for them.
23. **(Reversed by item 37.)** **Interpreter arguments:** any `/...` inside a word on a shebang line is classified (`-S/x`, `-P/x:/y`, `PATH=/x`, quoted forms), through `macho::shebang_arguments`. Only the interpreter is read, and an absolute program `env` is told to run: a word such as `-P<destination>/bin` is classified and never read (round 3). A `#!` line with no line end in the head, on a file longer than it, is refused: on Darwin a line with no newline in its first 512 bytes does not exec, and NUL ends a line where CR does not (observed with `/bin/echo` only, 2026-11-04). Script bodies are unread, accepted in R-27.
24. **Names in the dependency calculation stay as they are:** the installed names include `aliases` and `oldnames`, because R-25 finding 12 requires them: a renamed dependency is recorded under its old name. A `runtime_dependencies` entry that is tap-qualified holds `/`, which `checked_name` refuses for the whole document, so such a machine's inventory fails closed before this check; a parser limit older than this part, and its base-name branch in `KegFacts::of` is unreachable today. Flagged, not changed here.
25. **Accepted in writing:** the window between `canonicalize` and `open` (R-27's last section). No provider validation in the policy loader for `liveness-probes.toml`: policy cannot name a provider, and `registry` is what knows them.
26. **Tests that hold the guards:** `invariant_15_the_adapters_own_guard_stops_a_read_of_this_disk` gives each adapter an answering runner and a real scratch tree, so a removed guard yields facts rather than `NOT_OBSERVED`. **Round 2 replaces round 1's "shadowed" acceptance:** `DestinationSites::is_live`, `observes` and `linkage_prefixes` expose the registry's guard, and `invariant_15_each_half_of_the_sites_guard_stops_it_looking` asserts each half directly on any host, so dropping `!is_replay`, or the guard, fails. **Round 3:** the table is built inside `linkage_prefixes`, so no unguarded route to it exists. A provider no prefix was detected for is `invariant_15_a_provider_not_detected_observes_nothing`.
27. **`ENTRY_OF_NO_PACKAGE`, new (round 2):** a Homebrew file in `bin` that no keg owns was named `ANOTHER_PACKAGES_ENTRY`, which it is not. The words say "no package this provider's rule can name" (round 3), because a cask's `bin` link lands here too.
28. **`PROBE_VERSION_NOT_QUALIFIED` exists and nothing answers it yet.** It is in `migrate.v2` while that is unreleased, so H-3 does not need a new version for it.
29. **`stat_counts` and `examine` take `&CheckedList`,** whose fields are private and which only `checked_list` builds, so "the list before the `stat`" is the compiler's and no longer a convention.
30. **The liveness loader refuses an empty `provider` or `package`.** That is presence, not naming a provider; which ones exist stays the registry's.
31. **The `MachineDestinationSite` test is in `crates/cli/tests/executor.rs`, not `crates/cli/src/executor/`,** because the source scans there refuse a provider's name, `map_plan` and another use of the constructor word; the site gained `MachineDestinationSite::new`, which needs `DestinationSites`, which needs the lock.
32. **The `analysis_and_dry_run_never_reach_a_probe` scan already counts by `words`,** the same split the word scan at `executor.rs:900` uses, so a reflowed second caller is one more word (`15c`).
33. **Accepted, no change (round 3):** the Unicode test's `eprintln!` skip on a case-sensitive volume, and that only the firmlink test pins `(dev, ino)` of a prefix. `invariant_07_an_unsealed_directory_is_not_the_systems_under_any_spelling` and its skips are gone: the allow-list makes it moot.
34. **Roots are an allow-list (round 4):** what `man dyld` names, `/usr/lib`, `/System/Library/Frameworks` and `/System/Library/PrivateFrameworks`. The exclusion list is gone, since a future firmlink would have reopened a false `Holds`. `is_system` tests where the path really is, through the deepest existing ancestor, so `/usr/lib/cron -> /private/var/at` does not qualify. `realpath` returns the stored name (observed 2026-11-04), so a fold of a root is the root and a fold of anything else is that thing.
35. **(Reversed by item 39.)** **`@rpath` by dyld's semantics (round 4):** a name is tried against every rpath the walk has seen, to a fixed point under the work bound, because security proved a real load through the executable's rpath while the library's only rpath was `/usr/lib/swift`, answering `Holds`. A rpath that is a real directory in no known prefix and outside the roots is `FILE_NOT_EXAMINED`. An absent candidate under the roots answers the name, whatever exists elsewhere: `settle_rpath_names` starts from `in_system`. **The walk's rpaths only:** no whole-machine loader chain, said in R-27.
36. **dyld's default fallback (round 4), `FALLBACK_UNDER_OTHER_PREFIX`:** when a name resolves to nothing, or an `@rpath` has only an absent system candidate, the reader looks where `man dyld` says older binaries look, and a file of that name under another prefix is `Unchecked`, never `Fails`, since which loads cannot be told. This is the project's founding direction, a MacPorts destination with Homebrew at `/usr/local`. Measured in R-27: none here, and three packages `FILE_NOT_EXAMINED` by item 35's rule. The locations are in the new `data/policy/dyld-fallbacks.toml`, since `invariant_20_no_provider_deadline_is_written_into_source` refuses the literal in source, rightly. `PINNED_DIGEST` moved with the policy serial.
37. **A shebang's CR is part of the path (round 4):** the kernel ran the other provider's program through an `x\r` directory, and a CRLF script names a missing interpreter. **Any word naming a regular file in the destination is read,** and a directory says nothing. **`arch`, `nice`, `time`, `sandbox-exec` and `xcrun` read as `env`,** in `macho.rs`'s list, because each runs what it is told; none of them appears in this machine's `#!` lines. **Item 42 reverses this list.**
38. **`when read: ` with an empty list** is omitted, by its own catalogue key. **`WalkWork` reports bytes,** so `overlapping_fat_slices_cost_one_read_of_the_range` holds the range dedupe again.
39. **(Reversed by item 46.)** **The run path list, per file (round 5):** a name found only through a sibling's rpath skipped `RPATH_UNRESOLVED` and the fallback, and a real binary loaded the other provider's file. The union still classifies, joins the walk and answers `RPATH_UNRESOLVED`; `RpathName` gains `missed_by_its_own_file`, set for any occurrence whose own file's rpaths do not find it and never merged away, and every such name looks at the fallback. No load graph. The over-approximation and its cost are in R-27 and `migrate.v2`, with `man dyld`'s whole sentence.
40. **`DYLD_ENVIRONMENT_NOT_MODELLED` (round 5):** any `LC_DYLD_ENVIRONMENT` answers `Unchecked`, because a real binary loaded from an outside directory this way. Its paths are still classified. Judged after the fallback and before `RPATH_UNRESOLVED`.
41. **A fallback location is measured as `reached` measures (round 5):** the destination's own file joins the walk, so the fallback is part of the fixed point; a real file nobody can place is `FILE_NOT_EXAMINED`; another prefix, detected or a candidate, is the doubt.
42. **The wrapper list is gone (round 5), reversing item 37's list:** on any interpreter, a word that is no flag and holds no `/` is found by `PATH` and makes the script indeterminate on the list. `macho.rs` keeps `env` alone, since `env -Spython3` has no such word. Measured in R-27: 8 of 1,165 `#!` lines carry such a word, none on an executable list seen.
43. **`dyld-fallbacks.toml`'s loader (round 5):** `deny_unknown_fields`, `source` and `read_on` required and checked as `prefixes.rs` does, and an empty list refused, since it would silently stop the look. Policy data `2026.11.04-3`, for the comments here and in `liveness-probes.toml`.
44. **Text (round 5):** `FILE_NOT_EXAMINED` says "something it lists, loads or searches" in both catalogues and the domain, since it covers a real rpath directory and a word on a `#!` line too. `reason.verify_destination` says "(this version runs none)". `AT_LEAST` is the real 15. `plan.v2` and `04` say what would be observed past the gate this build does not reach.
45. **Accepted, no change (round 5):** a real framework under the Preboot Cryptexes answers `FILE_NOT_EXAMINED`; the fixed point's cost; a script body's own lookups, already declared unread; `SYSTEM_ROOTS` and the data file stating overlapping facts. All in R-27.
46. **What dyld would refuse answers nothing (round 6):** a candidate counted as found if it existed, so `fallback()` was skipped, and real binaries with plain text, an empty file, an arm64-only dylib on this x86_64 host, or an executable at the named path loaded the other provider's file. `read_file` records the inodes where a read showed an `MH_DYLIB` slice for this build's `cputype`, with no build-version or version-min mark of another platform (`SliceRefs::is_dylib_for_this_build`), and every load waits in `Walk::pending` until its candidates have been read. `missed_by_its_own_file` is gone, as what it said is now each pending load's own list. `RPATH_UNRESOLVED` asks the same question of the union. **A real file under the system's own roots counted, unread,** until item 51. **A candidate in no known prefix answers no name,** reversing round 2's test wording; `FILE_NOT_EXAMINED` is judged first, so the answer does not change. `LC_LOAD_DYLINKER` is not held to this, being no dylib. Accepted in R-27: a file dyld refuses for another reason, and Rosetta's architecture.
47. **A relative `#!` word (round 6):** a word that is no flag and does not start with `/` is indeterminate on the list (`#!/usr/bin/nohup bin/x` ran `bin/x`). The `/...` tail of such a word is no longer classified, and a flag's value counted only as `-X/...` until item 50. Extends item 42.
48. **A word is measured whole (round 6):** from where its first absolute path starts, as well as by pieces, since `sh` gets the whole word (`/x\r/prog`, `/a:b/prog`) and a CRLF line's word must still be read short of its CR.
49. **The loader, tighter (round 6):** `read_on` must be shaped `YYYY-MM-DD`, not just ten characters with two dashes, and every location must be absolute.
50. **A flag's attached value starts at its first `/` (round 7):** `#!/usr/bin/env -iS<other>/bin/p`, off the list, really ran and read `Holds`, a regression from item 47. Flags cannot be told from a relative value, so `-cbin/x`'s tail is measured for nothing: part L's open list.
51. **A real file under the roots answers a name only if its head shows a dylib (round 7, closes item 46's exception):** a load naming a non-dylib under `/usr/lib` read `Holds` while dyld loaded the other provider's. `can_be_taken` reads the head once, charging bytes and flagging nothing. The cheap read was not awkward: a system dylib in the shared cache has no file, so it never reaches this. `is_system` there became a test of the canonical path's text, so a repeated deep name costs no `canonicalize` (about 9 minutes at the bound, as reviewed). **A zippered dylib is taken:** the first run of this read refused `/usr/lib/libgmalloc.dylib`, which carries `LC_BUILD_VERSION` for macOS and for Mac Catalyst, so a macOS mark beside another platform's counts as macOS (`otool`, 2026-11-05).
52. **The platform rule is macOS plus Mac Catalyst only (final delta):** a dylib marked both iOS and macOS counted as taken while dyld refused it. A Catalyst mark alone answers no name. `window` can still set `unreadable_file` or `walk_over_bound` on the head read of item 51, which can only add `Unchecked`.
52. **Pinned (round 7):** `LC_LOAD_DYLINKER` is not held to the dylib rule, and `LC_VERSION_MIN_IPHONEOS`, `TVOS` and `WATCHOS` answer no name where `MACOSX` does. Each loader slice's own `cputype` was not small, and stays on the open list.
53. **The causes the admission adds are reserved (round 7),** as item 28 did for `PROBE_VERSION_NOT_QUALIFIED`: 15, in the enum, `unchecked_key`, both catalogues and `migrate.v2`, each answered by nothing yet, so the owner's patch stays in the frozen module. The design note names no refusal, so the names are this round's, and it now maps them to its conditions. 39 causes in all.
54. **A fallback location that cannot be looked at proves nothing (fifth proof, a defect unmutated):** `fallback()` read any `canonicalize` error as nothing there; only `is_gone` does now, and anything else is `UNREADABLE_FILE`.
55. **A dynamic linker never goes on to the fallback (fifth proof):** `if dylib`, in place of a guard that sent an absent one there, since dyld's fallback is not what starts it. Item 52's pin covers both.
56. **A flag's value is taken only from the start of a word (fifth proof):** `#!/bin/sh x=-I<other>/lib/f` is not measured. `sh` takes that word as a relative file name, and on the list it already answers as found at run time, so this narrower reading has no effect off the list, and a test pins it.

## The claim and the exit

This check is a bounded static read and cannot prove what dyld loads. It answers `Holds` for what this reader found on this macOS, on this architecture. **Part L's proposal must carry that sentence and R-27's limits.** H-2 is inert: `THIS_BUILD_DISPATCHES_MUTATIONS` is false and the gate refuses every plan.

**The reader's completeness was not the condition for merging,** six rounds each having found new proved false `Holds`. Merging is safe because the build is inert. Accepting this limit for removals is the owner's at part L, and is not decided here.

**Open for part L:**

- A bare `MH_DYLIB` with no commands counts as a dylib.
- `read_system_file` opens with a plain `File::open`, with no `O_NOFOLLOW` or `O_NONBLOCK`: the path is canonical and under roots only root can write, and no non-regular file was found under them to depth 5 (2026-11-05).
- The tool's architecture against the loader's, and Rosetta in both directions: an arm64 tool on x86_64-only packages answers `RPATH_UNRESOLVED`.
- Memory at the byte bound, about 1.6 GB extrapolated.
- `#!/bin/sh -cbin/x`, a flag with an attached relative value.
- One machine, x86_64, macOS 26.7.2 only; a tap-qualified `runtime_dependencies` entry failing the inventory.
- Any Swift-built formula, and rpaths into Xcode or the Command Line Tools, answering differently per machine.
- The first-run assertion, which has no actor while the gate refuses every plan: that the table holds a `Destination` class and that a listed file is not `FILE_NOT_EXAMINED` through an empty table (`h2`). R-27's counts are one machine's and are not what it asserts.

## Slices

| Slice | Branch | What | Depends on |
|---|---|---|---|
| H-0 | `wo16/H-1-checks` | Docs only: the four notes, the design, this plan, DP-056, DP-057, the SE entry, and the rewording of 04 (the checks, the failure table, partial success, marked not built), 05's ownership MUST, `analysis.v1` and `plan.v1`'s check table by a forward note, ADR-0006's amendment. 06 stays honest once 04 defines the kind of partial success, so it is not edited | Reviews |
| H-1 | `wo16/H-2-answer` | `CheckAnswer`, the dispositions, the outcome, `plan.v2`, `migrate.v2`, `restore.v2`, `history.v4`, the gate, the removal rule, the tripwire, with a site that observes nothing, so all four answer `Unchecked`. **Words that claim what SE-113 retracts:** the catalogue's `verification.*`, `migrate.check.*`, `reason.verify_destination` in both locales; the two wiki migration pages; 05's "`CheckNotEvaluableInThisBuild`" (the refusal becomes `MutationNotInThisBuild`), 04's "one probe per name" and "stay unevaluable", `plan.v1` obligation 6 | H-0 |
| H-2 | `wo16/H-3-reads` | Integrity, ownership, `macho.rs`, linkage, `StateDir`'s `probes`, `registry::destination_sites`, the data file (rows re-qualified first), the planner's rows | H-1 |
| H-stage | `wo16/H-4-stage` | Docs only, **after H-2 merges**: `16-staged/H/01` built and tested in a scratch worktree on that `main`; `H/02`, the capture of the ownership reads (`port -q provides`; `port -q installed <name>`), since `capture-fixture.sh` has none and `test-gates.sh` pins its forms; the rewording that takes effect with H/01 of 04's "A command spec carries no directory", "Every invocation records" and "one of the six". **The part is then `waiting on owner`** | H-2 |
| H-3 | `wo16/H-5-probe` | Wires the admitted probe in, flips `observed_by_this_build` for liveness | §3 item 5.4 on H/01 |

### What H-3 owes (not built now)

**H/01 carries the table reader, the eleven conditions and the named `NotRun` causes** (`docs/passes/16-liveness-design.md:76-108`): the adapter reads the table itself, takes prefix, Cellar and scope from itself, and `admit` refuses on conditions 4 (row), 6 (exact `observed_version`), 7 and 8 (the owner rule re-run now). Their causes, `PROBE_TARGET_CHANGED` among them, are reserved in the enum, answered by nothing yet, so H/01 is the frozen module only. H-3 owes only the wiring. (Round 5's advice here, that a row lookup belonged in `application`, is retracted by `spec-guardian`.)

- **How the lock reaches a probe (round 4 corrects round 3, which did not type-check):** `D: DestinationSite` lives in `crates/cli/src/executor/`, which can name `RunLock`, so the probe is a method of that trait, `answer_probe(&self, leaf, argv, lock: Option<&RunLock>)`, defaulting to `NOT_OBSERVED`, with `MachineDestinationSite` calling an inherent method on `DestinationSites`. That calls `verify_still_current` before the admission. **The provider comes from the adapter:** condition 4's row key is the adapter's own id, and a passed provider may select the adapter only.
- **The two `ProbeOwner` implementations (H/01's review, 2026-11-08):** the admission asks a crate-internal `ProbeOwner` trait for conditions 4 and 6-8, so those conditions are only as good as the implementations H-3 writes. `prefix()` and `runner()` return the adapter's own detected fields, never a parameter; `provider()` is the adapter's `PROVIDER_ID`; each adapter gets condition 7 and 8 tests against a real scratch tree (a link into another package, the `lzcat` to `xz` case, a target under `Caskroom`, under `var/`, under `Homebrew/`). Both implementations go under AGENTS.md "Dangerous to change" and `.claude/rules/`, reviewed as if frozen. A test proves the production runner reaches `SystemRunner::run_probe`, since a wrapper that forgets to forward it answers `NOT_OBSERVED` for ever; a recording runner never reaches a destination site.
- **The read-only loop reaches the same code:** `destination_answers` (`crates/cli/src/executor/mod.rs:443`) runs from it too, which is why the lock is an `Option`, and read-only keeps `NOT_OBSERVED`.
- **`observed_by_this_build` flips** for `LIVENESS_PROBE`, in the change that lands this wiring.
- **The tripwire extends:** `lock: None` starts nothing on `MachineDestinationSite`, with a panicking runner, and a scan of `answer_probe`'s callers.
- **`~/Library` must be closed before any probe runs:** by a search by time over the whole of it, or by `security-reviewer` accepting the search by name explicitly. R-28's last section lists what the search by name left out, and the sandbox round shows only that a denied write did not stop the program.

For the H/01 staged README, as it stands. **The first block is what the patch carries, the second is not in the patch, and the README must not tell the owner otherwise:**

> **What H/01 carries.** The frozen process module only: the admission, `invariant_10_*`, the table reader and the eleven `admit` refusals of `docs/passes/16-liveness-design.md`. The causes they name are already reserved outside it. Applying it starts no program, because nothing unfrozen calls it yet.

> **What H-3 owes, in unfrozen code, after H/01 is applied.** Only the wiring.
>
> (1) **The lock, concretely.** `run_with_mutations` is generic over `D: DestinationSite`, and that trait lives in `crates/cli/src/executor/`, which can name `RunLock`. So the probe is a method of that trait, `answer_probe(&self, leaf, argv, lock: Option<&RunLock>)`, defaulting to `Unchecked(NOT_OBSERVED)`, and the loop calls it for `Question::Probe`. `destination_answers` (`crates/cli/src/executor/mod.rs:443`) runs from the read-only loop too, which passes no lock and keeps `NOT_OBSERVED`. `DestinationSite::answer` keeps the other three questions.
>
> (2) `MachineDestinationSite` implements it by an inherent method on `DestinationSites` taking `&RunLock`, which calls `verify_still_current` before the admission, per call. The provider comes from the adapter: condition 4's row key is the adapter's own id, and a passed provider may select the adapter only. The tripwire extends: `lock: None` starts nothing on `MachineDestinationSite`, with a panicking runner, and a scan of `answer_probe`'s callers.
>
> **The fate of what is there.** Domain's `DestinationProbe` trait goes, since its one implementor becomes that inherent method and it could never name the lock; the design note is amended to say so, **which `security-reviewer` has yet to read.** `ProbeObservation` moves with it or stays as that method's return type. `invariant_15_a_replay_never_runs_a_probe` moves to the inherent method, holding a real `RunLock`, and keeps asserting `NotRun(NOT_OBSERVED)` under a replay with a runner that panics if asked.
>
> (3) `observed_by_this_build` turns true for `LIVENESS_PROBE`.
>
> (4) `~/Library` is closed before any probe runs, by one of two routes: a search by time over the whole of it, which the 2026-11-04 walk did not finish in over an hour, or `security-reviewer` accepting in writing the search by name in R-28's last section, which left out `Containers`, `Group Containers`, `Mobile Documents`, `CloudStorage`, `Developer` and most of `Caches`. The sandbox round would have denied such a write, and the program exited 0; whether one was attempted was not recorded. **This stays open:** `security-reviewer` has not accepted the search by name.
>
> **What the version pin costs.** After any upgrade of a package with a row, its run answers `PROBE_VERSION_NOT_QUALIFIED` for that probe, so the check is unchecked and **its source is kept**, until the row is re-qualified. Staleness does not warn about old rows: they are excluded from `policyReadOn`. **The plan does not warn either:** the planner finds a row by `(provider, package, executable)` with no version, so the plan still shows `LIVENESS_PROBE` with its `argv`, and the mismatch appears only in the run's result.
>
> **Exit condition.** `observed_by_this_build` turns true for `LIVENESS_PROBE` only in the change that lands (1), (2) and (3) together, with H/01 applied and (4) already closed. Until then `the_switch_needs_the_destination_contract_observed` names the probe, and the switch stays off.

**No frozen hunk in H-1 or H-2:** every read named is already allowed in `data/policy/provider-commands.toml`. H-2 grows one in-code allowlist, `macports/mod.rs`'s `is_allowed_argv`, by `-q installed <name>`, the read the verify step already makes ("As built (H-2)" item 16). `git diff fd443ef..HEAD` on the frozen paths is run on each PR.

**Part K does not start while H-3 waits.** K's item 3 needs all six checks, and "merged (owner item open)" would stretch F's precedent, whose open item was a test capture, not a check of the contract. So when the staging merges, the session ends with the "Needs the owner" line (§4.3), unless the owner explicitly lets K go first, in which case K's tests inject the site's answers and the tripwire stays failing.

**What is testable without a recording.** The judges, over facts. The filesystem half, in a scratch directory. The parsers, on the texts R-25 and R-26 observed. **Whole-binary recordings are owner-blocked:** no recording holds these reads, and `capture-fixture.sh` aborts on `ARG_MAX` after `brew list --formula cmake`, an open owner item. Under `--replay` a `VERIFY_DESTINATION` halts before any read (SE-107 C1, `plan.v1` obligation 6), so the whole binary ends halted, and `NOT_OBSERVED` is tested at the site and judge layers. **No recording of a contaminated binary can exist:** detection is shown on bytes built in the test, and said so.

## Mutants

Each `Fails` read as `Holds`; each `Unchecked` read as `Holds`; the new variant counted as verified; a skip as `Holds`; an `env` shebang as `Holds`; `lstat` for `stat`; dropping `is_under`; the list path for the placed entry; `LC_ID_DYLIB` counted; only the first fat slice; the destination's own prefix counted; case-sensitive "under"; an unnormalised `..`; a dropped walk; `destination_verified_before` accepting the new variant; `COMPLETED_WITH_UNCHECKED` as `SUCCESS` or exit 0; a halt outranked by it; the gate accepting a missing check; `destination_still_verified` accepting the new variant; `next_action` dispatching `VERIFY_REMOVAL` after a pass-over; a probe from a read-only path; a probe before ownership or after a `Fails`.

**The first proof, at `701f0fd`, before round 1's tests: 120 mutants, 74 killed, 46 survived.** Equivalent, with nothing needed: `18c` to `18g` (a field defaulted to empty is refused by the loader's own checks), `19a2`, `20b`, `13e`. The rest were closed by rounds 1 and 2.

**The second proof, at `b70f512`: 85 mutants, 78 killed, 7 survived.** `18a` and `18b` are equivalent: a defaulted field is refused by the loader's empty-key check, proved by the pair with `18p`. `b03b`, `b03c`, `b10b`, `b10c` and `b12c` are closed by the tests folded into `crates/adapters/tests/destination_reads.rs`: `the_path_limit_is_exact`, `invariant_07_a_reference_spelled_under_a_prefix_counts_though_it_leads_out`, `an_env_script_listed_through_a_link_to_an_executable_directory_is_on_the_list`, `a_prefix_that_is_a_link_to_another_still_counts_by_its_canonical_text`, and the control `a_link_outside_every_prefix_into_another_counts_by_where_it_leads`.

**The third proof, at `56742e2`: 60 mutants, 48 killed, 1 refused by the compiler, 11 survived.** What closed each:

- `b4`: `a_real_directory_in_a_word_outside_every_prefix_is_not_examined`.
- `c4`: `check_bases_read_back_on_a_destination_verified_with_unchecked`.
- `e2`: `an_absent_paths_failed_ancestors_are_charged_too`.
- `f4`: its proof asserted that an argument of `env`'s program is not read. Round 4 reads any word naming a regular destination file, since the reader cannot tell the argument from the program, so the stricter answer is held by the `argument` case of `a_shebang_word_naming_a_regular_file_of_the_destination_is_read_and_a_directory_is_not`, with `env -i`, `env FOO=1` and `env -u X`.
- `g2b`, `g3b`: `a_nul_ends_the_line_for_the_refusal`, `a_cr_does_not_end_the_line_for_the_refusal`.
- `k4`: the cause list of `every_linkage_answer_but_holds_stops_the_probe_and_is_the_reason_given`.
- `a4`, `d3`: moot, the allow-list removed the exclusions they mutated.
- `h2`: no hermetic test (`linkage()` passing an empty table degrades `LINKS_ANOTHER_PROVIDER` to `FILE_NOT_EXAMINED`, and is never a false `Holds`): "What H-3 owes" has the machine round assert that the table holds a `Destination` class and that a listed file is not `FILE_NOT_EXAMINED` through an empty table.
- `i2`: equivalent, a compile-time guarantee.

**The fourth proof, at `dd1d62a`: 89 mutants, 62 killed, 27 survived.** It also proved two defects unmutated, item 48. What closed each, in `crates/adapters/tests/destination_reads.rs` unless named:

- `p2b`: `a_name_and_rpath_a_candidate_brings_get_their_own_round`.
- `p4c`: `candidates_that_cannot_be_spelled_are_charged_to_the_bound`.
- `p5b`: `a_taken_name_does_not_fall_back_though_a_system_candidate_is_absent`, rewritten after item 46.
- `f7b`: `the_fallback_is_the_reason_given_over_an_unresolved_name`, in `destination_judges.rs`.
- `f9a`: `a_fallback_link_under_another_prefix_counts_by_its_spelling`. `f9c`: `a_fallback_under_a_prefix_known_by_a_link_counts_by_its_identity`.
- `f10a`: `a_fallback_under_an_undetected_prefix_is_a_doubt`. `f10c`: `the_destinations_own_file_at_a_fallback_location_is_no_doubt`. `f11`: `a_fallback_location_is_charged_to_the_bound`.
- `t1`, `t2`, `t4`: `the_fallback_file_carries_its_provenance_and_refuses_what_stops_the_look`, in `destination_reads.rs`'s own tests, which round 5's loader made the seam for.
- `cr3`: `a_word_ended_by_a_carriage_return_is_still_read`.
- `s6-*`: `env_with_flags_on_the_list_is_indeterminate`. `s8`: `an_absolute_word_of_env_is_read_as_the_interpreter`. `s9`: `an_absolute_interpreter_that_is_a_directory_is_not_examined`. `s12`: `every_path_of_an_assignment_is_a_reference`. `s13`: `a_quoted_word_names_the_file_inside_the_quotes`.
- `o2`: `the_bytes_read_are_counted`.
- `c1`, `c2`: `an_empty_list_renders_its_own_line`, in `human.rs`, replacing a `source.contains` that matched itself.
- `p4d`, `f9b`, `f12`: equivalent.

`security-reviewer`'s `fixed-point-off`, a fallback file reached through an `@rpath` name joining the walk, is a case of `a_fallback_location_is_measured_by_its_class`.

**The fifth proof, at `c6e95b3`: 75 mutants, 67 killed, 8 survived.** Part A, all the fourth proof's gaps, has no survivors. It also proved item 54's defect unmutated. What closed each:

- `r1`: the `@rpath` half of `a_real_file_under_the_systems_roots_answers_a_name_only_if_it_is_a_dylib`. The proof's sketch asserted the rule item 51 changed.
- `e4`: the cause list of `every_linkage_answer_but_holds_stops_the_probe_and_is_the_reason_given`.
- `e6`: `the_environment_is_the_reason_given_over_an_unresolved_name`, in `destination_judges.rs`. `e3` and `e3b`, tried with it: `a_found_link_is_a_failure_though_the_environment_is_set`.
- `b4`: `the_systems_own_file_at_a_fallback_location_is_no_file_unexamined`.
- `w5`: `a_dynamic_linker_is_not_held_to_the_dylib_rule`. `w6`: moot, item 55 removes the guard, and that test pins it.
- `h6`: `a_flag_value_is_taken_only_from_the_start_of_a_word`.
- `a5b`: `2026-11-0` and `2026-11-041` in the loader's unit test.

## Invariant tests, by name

`invariant_01_*` an unchecked destination is never removed, both directions; `invariant_03_*` the status mapping; `invariant_07_*`, `invariant_08_*` linkage; `invariant_10_*` the probe environment; `invariant_15_*` replay and from-snapshot; `invariant_16_*` the retained run; `invariant_17_*` one judge, both directions; `invariant_19_*` a stateful row is a load error; `invariant_20_*` every row dated.

## INV-17

One `CheckAnswer` model and one judge per check, in both directions. Declared asymmetries, each the provider's own: Homebrew's list is generated and MacPorts' is a record; Homebrew's owner is a rule and MacPorts' a query; only Homebrew has a dependency read to check; `LISTS_OUTSIDE_PREFIX` arises only under MacPorts. `ENTRY_LEAVES_OWNER` is one provider-neutral cause on both. **A path rule against an empty query:** a Homebrew file no keg owns is `Fails(ENTRY_OF_NO_PACKAGE)`, because Homebrew's rule on the path is a complete answer, and MacPorts' empty `provides` is `Unchecked(OWNER_NOT_NAMED)`, because that query's silence means several things. The initial table is 6 Homebrew and 7 MacPorts rows; coverage differs as data, not as a code path (R-28 finding 25). The asymmetries above do have an effect, and each is reported rather than averaged away.

## Reviews

| Reviewer | Reviewed at | Verdict | Resolution |
|---|---|---|---|
| `spec-guardian` | uncommitted draft on `9b8d68d` | Approve with changes, B1-B11, D1-D8, R1-R7: tripwire would go quiet; `Verified` wildcards fail open; `history.v4`, status, INV-16; breaking schema unbumped; no skip shape, `CHECK_HAS_NO_ANSWERING_READ`; `env` as `Holds`; weak basis; `cited`; "by type"; staging too early; K must wait; transitive walk; wall clock | All in this revision |
| `security-reviewer` | the same draft | Changes required, P1-P9, S1-S3: the same two fail-opens by another route; no-executable package vacuously passes; symlinks and the walk; the reader's specification; one disposition per step; the passed-over removal through the rest of the plan; H/01 incomplete and too early | All in this revision |
| `spec-guardian`, round 2 | `11d8d50` | Approve with changes, 13: SE-113's format and counts; 05 and `plan.v1` still say what SE-113 retracts; unbuilt behaviour unmarked; `restore.v1` missed; the catalogue and wiki; passed-over steps unnamed | All in `H-0` |
| `security-reviewer`, round 2 | `11d8d50` | Pass with changes, 1-8: the walk's false `Holds` through a directory link; the re-read cannot use the four-answer judge; the skip and `is_site_answered`; no token exists, so numbers are the bound | All in `H-0` |
| `docs-auditor` | `f32b359` | 14 findings: the dialog overstated where the owner rules; `[]` read as a failure; a stale part F citation; spec 04 keeping a phrase R-25 rejects; undated claims | All in `e6a28ec` |
| `spec-guardian`, H-1 | `d5d5d62` | Changes required: a hand-edited `COMPLETED` over something unchecked read as `SUCCESS`; the site asked after a read that failed; the re-read vacuous on an empty list; named invariant tests missing; schema documents wrong; words still retracted; H-1 opened a shape with no read-answered check | All folded; deviation 5, 6 and 7 above |
| `security-reviewer`, H-1 | `d5d5d62` | Changes required: the same `SUCCESS` proved by running it; a record stating a removal after something unchecked; the contract requiring no read-answered check; an unknown ownership worded as a negative (`OWNERSHIP_NOT_HELD`, now `OWNERSHIP_NOT_TOLD`) | All folded |
| Mutation proof, H-1 | `d5d5d62` | 31 of 37 killed. Survivors M9, M23, M25, M29, M34, M36 | A test added for each; re-proved below |
| `security-reviewer`, H-1 round 2 | `deb73d1` | Four low: a second removal, a handed-over removal with no end, a removal check with no removal behind it, the ending rule one way only; the comment overstating. No record this build's loop can write is wrongly refused | Folded; the comment now says the rule catches a contradiction and authenticates nothing |
| Mutation proof, H-1 round 2 | `deb73d1` | 13 of 16 killed. M29 survived because an integration test's build has no row (`cfg(test)` is the unit test's); M34, N5; N1 near-equivalent | Each got a test: the row at unit level; a control for the empty list; a run with nothing kept; a count of site calls on the re-read |
| `docs-auditor`, H-1 | `deb73d1` | 14: "all four answer `NOT_OBSERVED`" untrue of the probe and of any run; 04 contradicting itself on what is built; v4's stale origin; links to superseded documents; the wiki's miscount | Folded. Not done: a sample marker on the MacPorts page's transcript, whose input is not a fixture |
| `spec-guardian`, H-1 round 2 | `deb73d1` | Small changes: a second `REMOVE_SOURCE`; the status guard and the re-read's guard untested; `invariant_15_*` naming a test that does not exercise it; leftovers | Folded; deviations 8 and 9. `crates/cli/tests/executor.rs:931` matching any ancestor named `src`, an older test, is left |
| Mutation proof, H-1 round 3 | `0526230` | 5 of 9 killed. A, C, E and G survived: each hand-built case was refused by an earlier rule, and two wrote `unchecked` beside `result` rather than inside it, so `deny_unknown_fields` refused them | `each_rule_about_a_removal_answers_for_itself`, with a control; the field path corrected, and A, E and G re-run against the fix: killed |
| `spec-guardian`, H-2 round 1 | `701f0fd` | Changes required: false `Holds` (a file outside every prefix); false `Fails` on 19 of 194; a masked malformed file; invariant tests; the two risks and declarations | Folded into `ca9fa5c` |
| `security-reviewer`, H-2 round 1 | `701f0fd` | Changes required: H-1, H-2 (files dropped silently into `NO_LOAD_COMMANDS`), H-3, M-1 to M-4, L-1 to L-9 | Folded into `ca9fa5c` |
| `spec-guardian`, H-2 round 2 | `ca9fa5c` | Changes required: a history record refused; INV-15 registry guard mutants; the probe stop too narrow; "nothing was dropped"; a Homebrew case misnamed | Folded into `b70f512` |
| `security-reviewer`, H-2 round 2 | `ca9fa5c` | Changes required: R2-1 to R2-11, among them R2-4 (the probe gate) | Folded into `b70f512` |
| `spec-guardian`, H-2 round 3 | `b70f512` | Changes required: `/System` (high); H-3 gaps; the `from_stored` allow-list; the INV-15 route | Folded into `56742e2` |
| `security-reviewer`, H-2 round 3 | `b70f512` | Changes required: R3-1 (`/System`, its only high) to R3-9, among them R3-7 (`examine` taking an unchecked list) | Folded into `56742e2` |
| `spec-guardian`, H-2 round 4 | `56742e2` | Changes required: a CR false `Holds`; a weakened test; the lock route; the deny-list. The claim's limits were the coordinator's ruling, not this review's | Folded into `dd1d62a` |
| `security-reviewer`, H-2 round 4 | `56742e2` | Changes required: R4-1, R4-2, R4-3 (CR, high), R4-4 to R4-6 | Folded into `dd1d62a` |
| Mutation proof, H-2 | `701f0fd` | 120 mutants, 74 killed, 46 survived | "Mutants" above |
| Mutation proof, H-2 | `b70f512` | 85 mutants, 78 killed, 7 survived | "Mutants" above |
| Mutation proof, H-2 | `56742e2` | 60 mutants, 48 killed, 1 refused by the compiler, 11 survived | "Mutants" above |
| Mutation proof, H-2 | `dd1d62a` | 89 mutants, 62 killed, 27 survived; two defects unmutated | "Mutants" above |
| Mutation proof, H-2 | `c6e95b3` | 75 mutants, 67 killed, 8 survived; one defect unmutated | Closed by the tests in "Mutants" above |
| `docs-auditor`, H-2 | `56742e2` | Findings folded in round 4 | Folded into `dd1d62a` |
| `security-reviewer`, H-2 round 5 | `dd1d62a` | Changes required, all proved: R5-1 to R5-4 | Folded into `355fcfd` |
| `security-reviewer`, H-2 round 6 | `355fcfd` | Changes required: R6-1, A to D | Folded into `c6e95b3` |
| `security-reviewer`, H-2 round 7 | `c6e95b3` | No (a); nothing reachable by accident on a real package; small code, and the open list | Accepted to land with the open list; folded in `3fce170` |
| `spec-guardian`, H-2 round 5 | `dd1d62a` | Changes required: SE-113's count; this table's placeholder; `FILE_NOT_EXAMINED` narrower than it covers; `destination_answers` from the read-only loop. Its advice on the row lookup was retracted in round 6 | Folded into `355fcfd` and `c6e95b3` |
| `spec-guardian`, H-2 round 7 | `c6e95b3` | No invariant violation; must-fix text | Accepted to land with the open list; folded in `3fce170` |
| `docs-auditor`, H-2 | `dd1d62a` | A stale "not checked" sentence; `plan.v2` and `04` stating observation past a gate not reached; "none exists yet"; the peer page's list; `AT_LEAST`; R-27's duplicate and an unobserved "would answer" | Folded into `355fcfd` |
| `docs-auditor`, H-2 | `c6e95b3` | `04`, `05`, `plan.v2` on what is observed; R-27 and R-28 dates and rows that did not land; the quoted "no equivalent" text; `migrate.v2` and the domain's comments; a wiki sentence per provider | Folded in `3fce170` |
| `spec-guardian`, H-2 final delta | `3fce170` | Must-fixes done; no invariant violation, nothing untrue; merge from its side | Non-blocking items folded in the next commit |
| `security-reviewer`, H-2 final delta | `3fce170` | No (a). (b): a dylib marked both iOS and macOS counted as taken; `read_system_file` has no `O_NONBLOCK`; two refusals in the design note unnamed; the mapping of the 15 causes holds what it passed | The platform rule is macOS plus Catalyst only, tested; the file open is on the open list; the two refusals are named in the note. The amendment is read and accepted (round 7); the `~/Library` search is not |

