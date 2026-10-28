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
path rule.

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
| No `startupitem` token anywhere, no top-level `source`, and 4b passes | `NO` |
| Anything else — `create no` alone, `type none` alone, a declaration only inside a block or a continuation, `${...}`/`[...]` values, the `-append`/`-delete` forms, a top-level `source` | `UNKNOWN` |

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

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
| security-reviewer | plan, `41c26c2` | approve with fixes: M1 stored PortGroups found by building a path from Portfile text; M2 an absent keg or `stat` error read as `NO`; M3 a stale custom-label plist missed; M4 check-then-open (F-30); M5 the label and cellar unchecked; L1–L7 | All folded in: §4a–4c, §5, §7; L6's recording constraints in Follow-up |
| spec-guardian | plan, `41c26c2` | **not approved**: V1 a document without `service` read from its bool; V2 the Portfile rule narrower than F-A2; V3 holding the code on DP-054 when (a)/(b) are already ruled (DP-016 (d), D.5); D1 DP-054 framing; D2 the PortGroup path; D3 the custom label; R1–R6; missing tests and mutants | All folded in: absence is unknown and the live snapshot is re-captured (§1); the decision table and its oracle (§4c); the code proceeds and DP-054 asks only about relaxing (§ruling); the tests and mutants above |
