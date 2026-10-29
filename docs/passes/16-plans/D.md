<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part D — what defines a service (DP-032)

Plan written 2026-10-28, before any code; revised the same day after both
reviews (see Reviews). Reviewed by `spec-guardian` and `security-reviewer`; their
review is the approval (work order 16 §4). Evidence:
`docs/research/R-8-stateful-and-service-packages.md`, "Addendum 2026-10-28"
(cited below as A-n and F-An).

## The ruling, and what is already ruled about replay

DP-032: (c), then (b); the answer is the **union**, stated once in the domain.
A package defines a service if it **declares** one or one was **materialised**
for it. Each adapter supplies its two halves. Unknown refuses the plan (INV-19).

Replay is already ruled, twice: DP-016 is (c) then (d) — refuse where the source
cannot answer, then record service status in the recording — and work order 16
D.5 says that under `--replay` the reader answers from the recording or as
unknown. So this part implements **unknown under `--replay`** now; answering
from the recording is DP-016 (d), an addition recorded as follow-up work.
DP-054 asks the owner only whether to relax DP-032 and DP-016 for replay
(its option (c)); nothing here waits on it.

The consequence, stated rather than hidden: a replayed plan whose source row is
not declared a service is refused for both providers (INV-17). DP-016 (c) set the
precedent — a plan is demonstrated from a **live snapshot**
(`--from-snapshot`, `crates/cli/tests/migrate.rs` ≈1342), not a replay. The
walkthrough, the `plan.v1`/`migrate.v1` samples, the CLI tests and the wiki page
that show a plan move to `fixtures/snapshots/both-providers-intel-macos26-live.json`,
**re-captured on this machine** with the new reader (a read-only capture). A
replayed `plan` still demonstrates the refusal and its reason.

## What the research established

1. Installed Portfile: `<portdbpath>/registry/portfiles/<name>-<version>_<revision>/<sha256>-<size>/Portfile`
   (A-1). Two installed entries differing only in variants may hold two hash
   directories (A-1, from source).
2. "Declares" (F-A2): an item whose `type` is not `none`, or whose `create` is
   true, or which has a `custom_file` — including `dbus`'s `type launchd` with
   `create no`, which MacPorts never materialises but `port load` acts on.
3. No PortGroup adds a startup item today (A-3).
4. Some ports write a launchd plist from Tcl with no `startupitem.*` option
   (A-6); only the materialised half catches them.
5. Homebrew declaration: `service` non-null. The keg plist is written at install
   from the formula as it was then and can outlive the declaration (A-5). Its
   default name is `sh.brew.<name>` since Homebrew 6.0.22, `homebrew.mxcl.<name>`
   before, and `name macos:` overrides both; a `homebrew.<name>.service` unit is
   written beside it. DP-032's wording names only `homebrew.mxcl.*`; the intent is
   "the plist Homebrew materialised", recorded in SE-109.

## Design

### 1. Domain (`crates/domain/src/package.rs`)

- `Answer` = `YES` | `NO` | `UNKNOWN`, serialised in capitals; a value from a
  newer build reads as `UNKNOWN` (`serde(other)`, the additive rule of
  `snapshot.v1.md`).
- `ServiceDefinition { declared: Answer, materialised: Answer }` and **one rule**,
  `defines_service()`: `YES` if either half is `YES`; `NO` only if both are `NO`;
  otherwise `UNKNOWN`. No provider name, no replay flag.
- `PackageMetadata` gains `service: Option<ServiceDefinition>` (additive,
  `serde(default)`). `defines_service: bool` stays for older readers and is
  **written from the rule**: `service.defines_service() == YES`.
- **Absence is unknown (V1).** A row without `service` answers `UNKNOWN`, whatever
  its bool says: the bool was one half only. So a document from before this field
  refuses every plan, and the committed live snapshot is re-captured.
- **Load checks**, at the single snapshot validation point
  (`Snapshot::check`, so no deserialisation site skips them): `defines_service`
  equals `service.defines_service() == YES` (`false` beside `UNKNOWN` is
  consistent); and if any row of a provider carries `service`, every row of that
  provider does (L5). `ServiceDefinition` holds enums only: nothing to escape.

### 2. Classification and the planner

- `classify` adds `SERVICE` on `YES` (INV-19's `MANUAL_REVIEW`).
- The planner asks the **source row**: `UNKNOWN` refuses with
  `SafetyUnverifiable` (DP-016 (c)'s error, message naming the package, never a
  path). This replaces the provider-capability check, which it subsumes.
- The executor's re-plan under the lock goes through the same planner, so an
  answer that became `UNKNOWN` refuses the run.
- The `ServiceMetadata` capability is reported `Available` only when every
  installed row of the provider is known, else `Unknown`. It is informational;
  no decision reads it.

### 3. MacPorts, materialised half

The existing `service::scan`, unchanged in what it accepts: `Scanned` gives
`YES`/`NO` per port, `Unreadable` gives `UNKNOWN` for every port. A plist symlink
beside the item directory (`startupitem.install no`, A-2) stays a known false
unknown: it fails closed.

### 4. MacPorts, declared half: a bounded Portfile reader

New module `crates/adapters/src/macports/portfile.rs`, two parts.

**4a. Locating the file (§3 item 10).** Only where the filesystem is this machine.

- `portdbpath`: the default `<prefix>/var/macports`; if `macports.conf` sets
  another value, the declared half is `UNKNOWN` for every port (L3).
- Per installed entry: name (`names::is_safe_name`), version and revision each
  `[A-Za-z0-9._+~-]`, not `.`/`..`, no leading `-`/`.`; a **missing revision is
  `UNKNOWN`**, never assumed `0` (L1). The composed leaf
  `<name>-<version>_<revision>` must be exactly one `Normal` path component.
- The version directory must be a real directory (`symlink_metadata`); absent is
  `UNKNOWN` (M2).
- Its listing is bounded (16 entries). Every entry is either a regular dot-file
  (skipped, as `service::scan` skips `.DS_Store`) or a real directory matching
  `^[0-9a-f]{64}-[0-9]{1,10}$` (size parsed with overflow check). **Exactly one**
  such directory, anything else `UNKNOWN` (L2; two directories is the variants
  case of A-1, a known false unknown).
- The Portfile is read with the project's one bounded reader,
  `persistence::bounded_read::read_bounded`, `OpenPolicy::RefuseSymlink`, cap
  256 KiB, with the `accept` closure `homebrew::cask_requested` uses: dev and
  inode equal to the `lstat` baseline, `nlink == 1`, regular file, and length equal
  to the leaf's size suffix (M4, R4). A read error, including invalid UTF-8, is
  `UNKNOWN`. Directories above the file are only `lstat`-checked: the residual
  `bounded_read.rs` already accepts under ADR-0006 (no `O_NOFOLLOW_ANY`/`openat`),
  accepted here as there, and untestable as a race, as there.
- Files are read one at a time and dropped; after 8 MiB in total the remaining
  ports are `UNKNOWN` (L7).

**4b. Stored PortGroups, without building a path from Portfile text (M1, D2).**
List `<portdbpath>/registry/portgroups/*/` (bounded, 256 entries; each entry a
real `<sha256>-<size>` directory, else `UNKNOWN`), and read every `*.tcl` in them
with `read_bounded` as above. If any stored group mentions `startupitem`
(case-insensitive, raw bytes), or any cannot be read, the declared half is
`UNKNOWN` for every port whose Portfile contains the token `PortGroup`. Today none
mentions it (A-3), so this costs nothing, catches nested groups, and needs no
path rule. *Implementation review (security-reviewer L3):* every top-level line
whose first word is `PortGroup` must also be exactly `PortGroup <name>
<version>`, both literal, with `<name>-<version>.tcl` among the stored names —
compared as a string, never joined into a path — or the declared half is
`UNKNOWN`; a group that is not stored is one 4b never read. *Code review
(security-reviewer R1):* any other line mentioning `portgroup`
(case-insensitive) that is not a `#` comment line — `x; PortGroup a 1.0`,
`if {$x} { PortGroup a 1.0 }` — is `UNKNOWN` too: it loads a group this check
cannot name.

**4c. Recognising the text: a pure function of the bytes**, unit-tested on
in-code text. Only lines with no leading whitespace and not starting with `#`
count as declarations; the token search runs on raw bytes, case-insensitive,
before decoding, and **counts comments** (a comment mentioning the token still
blocks `NO`).

| Portfile holds | Declared half |
|---|---|
| A top-level `startupitem.create yes\|true\|1` | `YES` |
| A top-level `startupitem.type <literal>` other than `none` | `YES` |
| A top-level `startupitem.custom_file <literal>` | `YES` |
| A top-level `startupitems` line whose first key is `name <literal>` | `YES` |
| No `startupitem` token anywhere, no `source` word anywhere, and 4b passes | `NO` |
| Anything else — `create no` alone, `type none` alone, a declaration only inside a block or a continuation, `${...}`/`[...]` values, the `-append`/`-delete` forms, the word `source` anywhere | `UNKNOWN` |

**`YES` wins (R1):** one recognised `YES` line makes the answer `YES` whatever
else the file holds. Over-approximating is the safe side — `YES` reaches
`MANUAL_REVIEW` — and real service ports (nginx's `${prefix}` executable,
postgresql's continuation lines) would otherwise be refusals. `create yes` with
`type none` is therefore `YES` (V2): MacPorts creates nothing, but by F-A2's
test it is not a declaration of nothing, and `YES` is the safe reading. A
recognised negative form never yields `NO` on its own (L4).

The oracle for the real excerpts the tests use, read from the local ports tree
and cited in A-6: `nginx` `YES`, `redis` `YES`, `dbus` `YES` (`type launchd`,
`create no`), `logrotate` `YES`, `mysql8` `UNKNOWN` (inside `subport`, variable
name), a port with no token (`jq`) `NO`.

**Threat model (L4).** The reader interprets Portfiles written in good faith. It
is not a security control against a hostile one (`startup\x69tem`, `eval`,
`[string cat ...]`): a hostile Portfile already ran as root at install.

**Combining entries (L1, R5).** Several installed entries of one port: any `YES`
gives `YES`, else any `UNKNOWN` gives `UNKNOWN`, else `NO`.

### 5. Homebrew

- **Declared:** `service` non-null is `YES`, `null` is `NO` (a missing key is a
  parse error today and stays one).
- **Materialised (M2, M3, D3, M5).** Only where the filesystem is this machine.
  The cellar from `brew --cellar` must pass `process::is_safe_path`; the formula's
  `name` (never `full_name`, which can hold `/`) and each installed version are
  leaf-checked, and the composed path must add exactly `Normal` components. Then,
  per installed version: `<cellar>`, `<cellar>/<name>` and
  `<cellar>/<name>/<version>` must each be a real directory, else `UNKNOWN`;
  the keg's top level is listed (bounded, 512 entries, else `UNKNOWN`):
  - `YES`: a regular file whose name ends in `.plist` (case-insensitive) —
    `sh.brew.<name>.plist`, `homebrew.mxcl.<name>.plist`, or an old custom label
    no longer in the JSON (A-5 observed no keg plist without a `service`) — or a
    `homebrew.<name>.service` unit;
  - `UNKNOWN`: a `*.plist` entry that is not a regular file (a symlink);
  - `NO`: none of these.
  The label from `service.name.macos` is never joined into a path. Versions
  combine as in §4.
- **Casks (R2).** A cask can carry a `launchctl` stanza, and nothing here reads
  it, so a cask row's answer is `UNKNOWN` — not today's hardcoded `false`. A plan
  whose source row is a cask is refused until a cask reader exists, recorded as
  follow-up.
- **INV-20 (R6).** The two default labels and the 6.0.22 change are dated
  provider facts: constants with source and sha256 in their doc comment, as
  `SERVICE_PREFIX` already is; the re-check triggers (a MacPorts minor version,
  the sha256 of Homebrew's `service.rb`) go into R-8.

### 6. Where the filesystem is not this machine

`--replay` and a snapshot of one: MacPorts declared and materialised halves and
the Homebrew materialised half are `UNKNOWN`; Homebrew's declared half is read
from the recorded JSON. Every reader checks `filesystem.is_this_machine()` before
touching the disk, as `service::scan` and `cask_requested` do (INV-15).

### 7. Paths in output (L6)

No keg, cellar or Portfile path in an unknown reason, an error detail or
`doctor --report`; reasons name the package and the half.

### 8. Docs

`snapshot.v1.md` (`service`, the load checks, absence is unknown);
`02-domain-model.md` (the rule, once); `04-migration-and-safety.md` (INV-19's
input); SE-109 (part D bullet: the union, absence is unknown, the
`homebrew.mxcl.*` wording, casks, replay); R-8 (E-7's misquoted default
corrected; re-check triggers); the walkthrough, schemas and wiki moved to the
live snapshot; `fixtures/snapshots/README.md` (the re-capture).

## Tests

Each invariant this touches points at a test:

- **INV-19**, `invariant_19_an_unknown_service_answer_refuses_the_plan`: an
  `UNKNOWN` source row is refused; a declared-only `YES` (the `dbus` shape) and a
  stale-keg `YES` each reach `MANUAL_REVIEW` through the planner.
- **INV-17**, `invariant_17_the_service_rule_is_the_same_in_both_directions`:
  the INV-19 cases above in both directions.
- **INV-15**, `invariant_15_a_replayed_reader_never_consults_the_live_filesystem`
  extended: a scratch tree holding a declaring Portfile, a stored PortGroup and a
  keg plist, read with `Elsewhere`, answers `UNKNOWN` for every disk-backed half.
- **INV-19 under the lock:** the counterpart of
  `invariant_19_a_service_that_appeared_under_the_lock_refuses_the_run`
  (`crates/application/src/executor/tests.rs`) for an answer that became
  `UNKNOWN`.
- Domain: the nine cells of the union; the bool written from the rule; a
  disagreeing bool refused on load; a provider with `service` on some rows only
  refused on load; a row without `service` answers `UNKNOWN`; an unknown variant
  from a newer build reads as `UNKNOWN`.
- Portfile recogniser: every row of the table in §4c, `create yes` with
  `type none`, a comment-only mention, the six real excerpts with their stated
  oracle.
- Path walk on scratch trees: absent version directory; a symlinked hash
  directory; two hash directories; a non-matching sibling; a size mismatch; an
  oversized file; a symlinked Portfile; a hostile version (`../x`); a missing
  revision; a non-default `portdbpath`; a stored PortGroup mentioning the token;
  an unreadable stored group — each `UNKNOWN`.
- Homebrew keg on scratch trees: each `YES` name, an old custom label, the
  `.service` unit, a symlinked plist (`UNKNOWN`), an absent keg (`UNKNOWN`), an
  unreadable keg (`UNKNOWN`), a hostile cellar (`UNKNOWN`), and `null` declaration
  with a keg plist (`YES`, the stale case).
- A cask row answers `UNKNOWN`, and a plan from it is refused.
- A document without `service` refuses a plan.
- **Fixture:** no port here declares or creates an item (A-7, observed
  2026-10-28), so an end-to-end case on a recorded startup-item port is an
  **owner item**; this part merges with it open, as D.2 allows.

## Mutants (run in an isolated worktree)

1. The union as `AND`. 2. `UNKNOWN` read as `NO` in the planner. 3. The
recogniser accepts an indented declaration. 4. The PortGroup rule (4b) removed.
5. `read_bounded`'s `accept` closure skipped. 6. An absent keg read as `NO`.
7. Only fixed plist names checked (the custom-label case). 8. An `Elsewhere`
reader touching the disk. 9. Either load check removed. 10. The bool written from
one half. 11. `ServiceMetadata` `Available` while a row is `UNKNOWN`. 12. The
refusal asked of the destination row. 13. An unknown `Answer` variant failing to
parse. 14. A missing revision read as `0`. 15. A row without `service` read from
its bool.

## Follow-up recorded, not done here

- DP-016 (d): record the reader's inputs (bounded Portfile bytes, the keg's
  top-level listing, relative to the prefix or cellar) in the recording, so
  replay runs the same pure function (L6).
- A cask reader for `launchctl`.
- Widening `service::scan` to the `startupitem.install no` symlink layout.

## What this part does not do

No executor, gate or verification change beyond the planner's refusal;
`THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`. No new process allowlist entry:
every read is a file read or the existing `port -q provides`. No new crate.

## Implementation notes from the second round (folded in, no re-review needed)

- `source` is searched anywhere in the raw bytes, like `startupitem`; any
  occurrence blocks `NO` (security-reviewer 1). *Implementation review
  (spec-guardian risk 4):* an occurrence is a whole Tcl word — not preceded or
  followed by a letter, digit, `_` or `-` — matched on raw bytes,
  case-insensitive, comments counted. `[source x]` and `source x` block `NO`;
  `sourceforge`, `sources` and `open-source` do not.
- §4b: an absent `registry/portgroups/` is `UNKNOWN` for ports that use a
  PortGroup; exceeding the 256-entry bound is `UNKNOWN`, never truncation; group
  reads count toward the 8 MiB budget (security-reviewer 2).
- `portdbpath`: a `macports.conf` that cannot be read or parsed is `UNKNOWN`;
  "default" means exactly `${prefix}/var/macports` or its expansion
  (security-reviewer 3). *Implementation review (security-reviewer L1):* only
  the expansion is accepted, compared as a path; the literal `${prefix}` form is
  not expanded and answers `UNKNOWN`.
- Homebrew: any regular top-level `*.service` file is `YES`, like `*.plist`
  (security-reviewer 4).
- Compatibility with a newer build: a document whose bool is `true` beside an
  answer this build reads as `UNKNOWN` is refused on load; that fail-closed break
  is stated in `snapshot.v1.md` (security-reviewer 5).
- The re-captured live snapshot's diff is checked for a home path or username
  before it is committed (security-reviewer 6).
- `snapshot.v1.md`'s capability-keyed refusal text is rewritten; the tests
  `invariant_19_a_source_that_cannot_answer_the_service_question_refuses_the_plan`
  and `invariant_19_a_document_that_does_not_record_the_service_question_refuses_the_plan`
  are kept and re-keyed on the row (spec-guardian 1).
- A cask source row is now refused where it reached `MANUAL_REVIEW` through
  `GUI_APP`; named in SE-109, tests updated (spec-guardian 2).
- An unindented declaration inside a block reads as top-level; under "YES wins"
  that only adds a false `YES`, the safe side, and is said in the threat-model
  paragraph so nobody "fixes" it toward `NO` (spec-guardian 3).

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
| security-reviewer | plan, `41c26c2` | approve with fixes: M1 stored PortGroups found by building a path from Portfile text; M2 an absent keg or `stat` error read as `NO`; M3 a stale custom-label plist missed; M4 check-then-open (F-30); M5 the label and cellar unchecked; L1–L7 | All folded in: §4a–4c, §5, §7; L6's recording constraints in Follow-up |
| spec-guardian | plan, `41c26c2` | **not approved**: V1 a document without `service` read from its bool; V2 the Portfile rule narrower than F-A2; V3 holding the code on DP-054 when (a)/(b) are already ruled (DP-016 (d), D.5); D1 DP-054 framing; D2 the PortGroup path; D3 the custom label; R1–R6; missing tests and mutants | All folded in: absence is unknown and the live snapshot is re-captured (§1); the decision table and its oracle (§4c); the code proceeds and DP-054 asks only about relaxing (§ruling); the tests and mutants above |
| security-reviewer | revised plan, `323cb62` | **approve**; six non-blocking notes | Folded into "Implementation notes from the second round" |
| spec-guardian | revised plan, `323cb62` | **approved**; three low notes | Folded into "Implementation notes from the second round" |

## As built

Implemented on `wo16/D-impl` (2026-10-28). Where the build differs from the
plan, or the plan left a choice open:

- **Not committed: `scripts/run-fixture-matrix.sh` blocks the gate.** The
  matrix derives the outcome a fixture owes from the source provider alone
  (`want="refusal"` only for MacPorts). Under this design a replayed Homebrew
  row that declares no service is refused, so the four Homebrew fixtures fail
  ("refused, and this fixture has the facts to be planned"; 4 failed, 178
  passed). The plan did not list the script, `scripts/*.sh` is outside this
  implementation's reach, and the guard hook refused an edit even to a scratch
  copy outside the repository. The change it needs — read `want` from the
  replayed snapshot's source row: a row with a `service` record owes a plan only
  when the union is `YES`; a row without one keeps today's expectation — is the
  owner's to stage. Every other check of `scripts/validate.sh` passes on the
  working tree; the code is held there and as a patch, uncommitted.
- **"Row" is the metadata entry.** `ProviderInventory.metadata` is keyed by
  package name, not by installed row, so the planner asks the source
  provider's entry for the source row's name, and a missing entry answers
  `UNKNOWN`. The all-or-none load check is over a provider's metadata entries.
- **`portdbpath`** (`configuration::macports_portdbpath`) reads the system
  `etc/macports/macports.conf` under the prefix (required) and the user files
  `provider-configuration.toml` names (absent is fine; no home to find them
  under is `UNKNOWN`). It answers the default only; nothing read becomes a path.
- **Recogniser details** left open by §4c: `create` accepts `yes`, `true`, `1`
  and `on`, case-insensitively (`string is true -strict`); `create`, `type` and
  `custom_file` count only as exactly two words with a literal value (no `$ [ ]
  { } " \ ;`); `startupitems name <literal>` may continue. A line continuing the
  one before (`\`) is never top-level.
- **Homebrew's materialised half opens no file**: directory listings and
  `lstat` only. The `ServiceMetadata` capability is now `UNKNOWN` on any
  Homebrew with a cask (casks answer `UNKNOWN`), and under every replay; four
  golden analyses changed in exactly that cell.
- **`error.safety_unverifiable`'s remediation** ("the fact is absent from the
  document itself") became false: a live capture can answer `UNKNOWN` (a
  Portfile mentioning `source`). Reworded in `en` and `pt-BR` to "absent from
  the input, or could not be read where it lives", the precedent SE-092 set for
  `PROVIDER_PRESENCE_UNPROVEN`; and "This is not something to fix on your
  machine" became "This does not necessarily mean anything is wrong on your
  machine" (spec-guardian risk 8), since an unknown can now come from this
  machine's own disk.
- **Documented samples may read a committed snapshot.**
  `crates/cli/tests/documented_samples.rs` accepted only `--replay
  fixtures/recordings/…`; it now also accepts `--from-snapshot
  fixtures/snapshots/….json`, still never a path elsewhere and never this
  machine. The `plan.v1.md`, `migrate.v1.md` and wiki plan samples read the live
  capture; `migrate.v1.md`'s executed sample now shows the gate refusing a
  replayed `MANUAL_REVIEW` plan (`jupyterlab`).
- **Binary-level execute-mode tests over a recording** now use a formula that
  declares a service (`emacs`, `jupyterlab`) and see `MANUAL_REVIEW` refusals;
  the plan with commands is refused before the gate and is asserted as such.
  `invariant_06_migrate_never_elevates` checks `ELEVATION_REQUIRED` in process
  on the live capture's plan. The rendered sentences for
  `MUTATION_NOT_IN_THIS_BUILD` and `ELEVATION_REQUIRED` are no longer reached
  through the binary. `run_report` renders the gate's refusals from the plan in
  front of it, not from `RunReport::gate_refusals`, so the run-report test
  never reached most of them;
  `every_gate_refusal_is_a_catalogue_sentence_in_every_shipped_locale`
  (`crates/cli/src/app/human.rs`) now renders one instance of every
  `ExecutionRefusal` variant through `gate_refusal_line` in `en` and `pt-BR`,
  from a list an exhaustive `match` keeps complete (spec-guardian MUST 1).
- **The gate over the live capture.** A replay now reaches only `MANUAL_REVIEW`
  plans, so `crates/cli/tests/execution_gate_fixtures.rs` also runs every
  `fixtures/snapshots/*.json`, asserts that some plan does not stop (so its
  `MUTATION_NOT_IN_THIS_BUILD` branch is exercised) and that both directions
  are planned (spec-guardian MUST 3).
- **Test helpers.** In-memory snapshots carry `service` (both halves from the
  old bool, `Answer: From<bool>`); `macports_contract`'s scratch prefixes lay
  down a registry Portfile that declares nothing for every recorded port and a
  scratch home, so those tests stay about the scan. Domain helpers added:
  `PackageMetadata::with_service` (writes the bool from the rule) and
  `service_answer`.
- **The live snapshot** was re-captured with the part D build, and again after
  the implementation review; see `fixtures/snapshots/README.md`.
- **Implementation review (spec-guardian, security-reviewer).** `source` is a
  whole Tcl word (risk 4; §4c and the second-round notes above amended);
  `keg()` decides only after counting its whole listing, so an over-long
  listing is `UNKNOWN` even with a plist in it (risk 5); `stored_groups` takes
  the `FilesystemScope` and answers unanswerable under `Elsewhere` (risk 6);
  `portdbpath` accepts only the expanded default, compared as a path (L1);
  `read_checked` requires the bytes read to equal the size checked and charges
  the budget by what was read (L2); and every `PortGroup <name> <version>` line
  must name a stored `<name>-<version>.tcl`, compared and never joined into a
  path (L3, §4b amended); any other non-comment line mentioning `portgroup` is
  `UNKNOWN` (code review R1; it changed no answer on the capturing machine).

## Mutation proof

Each mutant applied to the working tree, the named tests run, the file
restored (2026-10-28). All fifteen are killed (8, 9 and 11 split in two).
Mutants 3, 4, 5, 7, 14 and 15 were re-run after the final clippy edits to
`keg.rs`, `portfile.rs` and `package.rs`, and are still killed by the same tests.
After the implementation review, every mutant whose code it touched (3, 4, 5,
6, 7, 8a, 8b, 14) was re-run and is still killed, and mutants 16–20 were added
for the review's fixes, and mutant 21 for the code review's R1. All but one
are killed; **18 survives**, and is kept as
defence in depth: the bytes read differ from the length checked on the handle
only when another process rewrites the file between the check and the read,
which no single-threaded test can produce on demand.

| # | Mutant | Killed by |
|---|---|---|
| 1 | The union as `AND` | `package::tests::the_union_is_the_rule_in_all_nine_cells`, `the_bool_is_written_from_the_rule` |
| 2 | `UNKNOWN` read as `NO` in the planner | `invariant_19_an_unknown_service_answer_refuses_the_plan`, `invariant_19_a_source_that_cannot_answer_the_service_question_refuses_the_plan`, `invariant_17_the_service_rule_is_the_same_in_both_directions` |
| 3 | The recogniser accepts an indented declaration | `portfile::tests::every_row_of_the_decision_table`, `the_six_real_excerpts_answer_their_oracle` |
| 4 | The PortGroup rule (4b) removed | `portfile::tests::a_portgroup_is_unanswerable_only_when_the_stored_groups_are`, `a_stored_portgroup_mentioning_the_token_or_unreadable_blocks_no`, `every_portgroup_line_must_name_a_stored_group`, `more_stored_groups_than_the_bound_is_unknown_not_truncated` |
| 5 | `read_bounded`'s `accept` closure skipped (for the Portfile) | `portfile::tests::every_shape_the_walk_does_not_expect_is_unknown` (size mismatch, hard link) |
| 6 | An absent keg read as `NO` | `keg::tests::an_absent_or_unreadable_keg_is_unknown_and_not_no`, `versions_combine_yes_first_then_unknown` |
| 7 | Only fixed plist names checked | `keg::tests::every_name_homebrew_writes_and_an_old_custom_label_are_yes` |
| 8a | An `Elsewhere` keg reader touching the disk | `keg::tests::invariant_15_a_replayed_reader_never_consults_the_keg` |
| 8b | An `Elsewhere` Portfile reader touching the disk | `portfile::tests::a_non_default_portdbpath_or_another_machine_is_unknown_for_every_port` |
| 9a | The bool-agreement load check removed | `snapshot::tests::a_service_bool_that_disagrees_with_the_rule_is_refused_on_load` |
| 9b | The all-or-none load check removed | `snapshot::tests::a_provider_with_the_service_question_on_some_rows_only_is_refused_on_load` |
| 10 | The bool written from one half | `package::tests::the_bool_is_written_from_the_rule` |
| 11 | `ServiceMetadata` `AVAILABLE` while a row is `UNKNOWN` (MacPorts; Homebrew) | `a_declared_startup_item_with_nothing_materialised_is_a_service`; `capabilities_are_detected_from_the_document_and_never_assumed` |
| 12 | The refusal asked of the destination row | `invariant_19_a_source_that_cannot_answer_the_service_question_refuses_the_plan` and every `planner_refusals` test whose destination holds no row |
| 13 | An unknown `Answer` variant failing to parse | `package::tests::an_answer_from_a_newer_build_reads_as_unknown`, `snapshot::tests::a_service_bool_that_disagrees_with_the_rule_is_refused_on_load` |
| 14 | A missing revision read as `0` | `portfile::tests::every_shape_the_walk_does_not_expect_is_unknown` |
| 15 | A row without `service` read from its bool | `package::tests::a_row_without_the_service_question_answers_unknown_whatever_its_bool_says` |
| 16 | `keg()` answering `YES` at the first plist, before counting the listing | `keg::tests::a_launch_file_does_not_outrun_an_overflowing_listing` |
| 17 | `PortGroup` lines not checked against the stored names (L3) | `portfile::tests::every_portgroup_line_must_name_a_stored_group` |
| 18 | The bytes read not compared with the length checked (L2) | **survives** — see above |
| 19 | `source` matched as a substring rather than a word | `portfile::tests::a_whole_word_source_blocks_no_and_a_mention_inside_a_word_does_not`, `every_row_of_the_decision_table` |
| 20 | A literal `${prefix}/var/macports` accepted as the default (L1) | `configuration::tests::the_registry_is_found_only_at_the_default_portdbpath` |
| 21 | A non-comment line mentioning `portgroup` in another form skipped by `cover` (R1) | `portfile::tests::every_portgroup_line_must_name_a_stored_group` |
