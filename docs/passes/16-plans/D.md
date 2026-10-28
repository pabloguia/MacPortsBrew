<!-- SPDX-License-Identifier: Apache-2.0 -->

# Work order 16, part D — what defines a service (DP-032)

Plan written 2026-10-28, before any code. Reviewed by `spec-guardian` and
`security-reviewer`; their review is the approval (work order 16 §4).
Evidence: `docs/research/R-8-stateful-and-service-packages.md`, "Addendum
2026-10-28".

**Status: held at this plan on DP-054** (`docs/decisions-pending.md`). Every
option there rewrites a different set of tests and samples, so no code lands
until it is ruled. Everything below except §6 is common to all three options.

## The ruling

DP-032: (c), then (b); the answer is the **union**, stated once in the domain.
A package defines a service if it **declares** one or one was
**materialised** for it. Each adapter supplies its two halves. An unknown answer
refuses the plan (INV-19).

## What the research changed

1. The installed Portfile is two levels down:
   `<prefix>/var/macports/registry/portfiles/<name>-<version>_<revision>/<sha256>-<size>/Portfile`,
   one regular file, and the registry's `ports.portfile` column holds the
   `<sha256>-<size>` leaf (R-8 addendum, Q1).
2. **No PortGroup adds a startup item** (all 105 in the local tree, Q3). A stored
   PortGroup that mentions `startupitem` is still unknown, so that a future one is
   not read as "none".
3. A declared item need not be created: `dbus` declares `type launchd` with
   `create no` and MacPorts creates nothing, yet `port load` acts on it (Q2).
   Declared is the union's first half for exactly this case.
4. Some ports write a launchd plist from Tcl with no `startupitem.*` option
   (`dnsmasq`, `grafana`, Q6). Only the materialised half catches them, which is
   why the union is the answer and the declared half alone is not.
5. Homebrew: the declaration is `service` non-null in `--json=v2` (not "has
   `run`"). The keg plist is written at install from the formula as it was then,
   so it can outlive the declaration. Its default name is `sh.brew.<name>` since
   Homebrew 6.0.22, `homebrew.mxcl.<name>` before, and `name macos:` overrides
   both (Q5). DP-032's wording names only `homebrew.mxcl.*`; the intent is "the
   plist Homebrew materialised", recorded in SE-109.

## Design

1. **Domain (`crates/domain/src/package.rs`).** A tri-state `Answer` (`YES`,
   `NO`, `UNKNOWN`) and `ServiceDefinition { declared: Answer, materialised:
   Answer }`, with one rule, `defines_service()`: `YES` if either half is `YES`,
   `NO` if both are `NO`, else `UNKNOWN`. No provider name.
   `PackageMetadata` gains `service: Option<ServiceDefinition>` (additive,
   `serde(default)`); `defines_service: bool` stays, written as
   `defines_service() == YES` so older readers keep reading it, and **validated on
   load** to agree with `service` when both are present. A document without
   `service` reads as today: the bool plus the provider's `ServiceMetadata`
   capability.
2. **Classification and the planner.** `classify` adds `SERVICE` on `YES`. The
   planner refuses (`SafetyUnverifiable`, the DP-016 (c) error) when the source
   row's answer is `UNKNOWN`, in addition to the capability check that stays for
   old documents. Asked of the source row, as today.
3. **MacPorts, materialised half.** The existing `service::scan`, unchanged in
   what it accepts: `Scanned` gives `YES`/`NO` per port, `Unreadable` gives
   `UNKNOWN` for every port. The research's false unknown (a `<plist>` symlink
   beside the item directory, `startupitem.install no`) stays unknown: safe, and
   widening the scan is not this part.
4. **MacPorts, declared half: a bounded Portfile reader**
   (`crates/adapters/src/macports/portfile.rs`), only where the filesystem is this
   machine:
   - **Path rule (§3 item 10).** Built from the registry's own name, version and
     revision, each checked as a safe single leaf (`names::is_safe_name`, and the
     version and revision against `[A-Za-z0-9._+~-]`, no `..`). The hash leaf is
     found by listing `<name>-<version>_<revision>/` and must be exactly one entry
     matching `^[0-9a-f]{64}-[0-9]+$`. Every component is checked with
     `symlink_metadata` (a real directory, then a real regular file, never a
     symbolic reference followed), the file's size must equal the leaf's size
     suffix and be at most 256 KiB, and it is read once with that bound.
   - **Recognised forms**, on top-level lines (no leading whitespace) with a
     literal value on the same line: `startupitem.create yes|true|1|no|false|0`,
     `startupitem.type launchd|none`, `startupitem.custom_file <literal>`,
     `startupitems name <literal> ...`. `YES` when the MacPorts creation condition
     holds on the recognised values, or the port declares a `launchd` type; `NO`
     when no `startupitem` token appears anywhere and no stored PortGroup mentions
     one.
   - **Anything else is `UNKNOWN`:** an indented or block-scoped declaration
     (`variant`, `subport`, `if`, `platform`, `proc`, a phase), a continuation
     line, a `${...}` or `[...]` value, the `-append`/`-delete` forms, any other
     type, a stored PortGroup mentioning `startupitem`, invalid UTF-8, an
     unreadable file. One unknown line makes the whole answer unknown.
   - The reader is a pure function of the bytes (unit-tested on in-code text);
     only the path walk touches the disk.
5. **Homebrew.** Declared: `service` non-null (already read; `null` is `NO`, a
   missing key is a parse error today and stays one). Materialised: in each
   installed version's keg, a regular file named `homebrew.mxcl.<name>.plist`,
   `sh.brew.<name>.plist`, or the declared `name`'s plist, checked with
   `symlink_metadata`; never through `opt/` (a symlink). Read only where the
   filesystem is this machine. The keg path is built from the cellar the adapter
   detected and the name and version `brew` reported, each checked as a leaf.
6. **Where the filesystem is not this machine** (`--replay`, `--from-snapshot`
   of a replay): **DP-054.** Option (a): both materialised halves and the
   MacPorts declared half are `UNKNOWN`, and the plan is refused. Option (b):
   capture records each read beside the invocations, and replay answers from
   the recording or `UNKNOWN`. Option (c): as (a) for MacPorts; for Homebrew a
   `null` declaration answers `NO` with a named plan caveat. INV-15 holds in
   every option: no read of this host's disk.
7. **Capability.** `ServiceMetadata` is `Available` only when every installed
   row's answer is known; otherwise `Unknown`, as today.
8. **Docs.** `snapshot.v1.md` (`service`), `02-domain-model.md` (the rule, once),
   `04-migration-and-safety.md` (INV-19's input), SE-109 (D bullet, the
   `homebrew.mxcl.*` wording), R-8 E-7's misquoted default corrected.

## Tests

- Domain: the union's truth table, all nine cells; `defines_service` written
  from the rule; a snapshot whose bool disagrees with `service` refused on load;
  a document without `service` loads.
- `invariant_19_an_unknown_service_answer_refuses_the_plan`, in both directions
  (INV-17).
- Portfile reader: each recognised form, each unrecognised form, and the real
  Portfile excerpts the addendum cites (nginx, redis, dbus, mysql8, logrotate),
  as in-code text — read from the local ports tree's text, cited, not invented.
- Path walk, on a scratch tree: a symlinked leaf, a symlinked hash directory, two
  hash directories, a size mismatch, an oversized file, a hostile version
  (`../x`), each `UNKNOWN`.
- Homebrew materialised half on a scratch keg: each of the three names; a
  symlinked plist is `UNKNOWN`; a formula with `null` declaration and a keg plist
  is `YES` (the stale case).
- `invariant_15_a_replayed_reader_never_consults_the_live_filesystem` extended:
  a scratch tree holding a declaring Portfile and a keg plist, read with
  `Elsewhere`, answers per §6, never `YES` or `NO` from the disk.
- Fixture: no port here declares or creates an item (observed 2026-10-28), so
  the end-to-end case is an **owner item**: capture a recording on a machine
  with `nginx` or `dbus` installed from MacPorts.

## Mutants

1. The union as `AND`. 2. `UNKNOWN` read as `NO` in the planner. 3. The reader
   accepts an indented declaration. 4. The reader ignores a stored PortGroup.
5. The path walk follows a symlink. 6. Homebrew's materialised half skips
   `sh.brew.*`. 7. An `Elsewhere` read touching the disk. 8. The load check on the
   two copies removed.

## What this part does not do

No executor, gate or verification change; `THIS_BUILD_DISPATCHES_MUTATIONS`
stays `false`. No new process allowlist entry: every read here is a file read
or `port -q provides`, already allowed.

## Reviews

| Reviewer | Commit reviewed | Findings | Resolution |
|---|---|---|---|
