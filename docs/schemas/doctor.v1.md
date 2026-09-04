# `doctor` schema, version 1

Emitted by `macportsbrew doctor --json`, and written in redacted form by
`macportsbrew doctor --report <file>`. Derived from `DoctorReport` in
`crates/application/src/doctor.rs`. The compatibility rule is in
[`README.md`](README.md).

`doctor` reports structural facts about the machine — prefixes, `PATH` order,
leaked build variables, provider health, support state. It makes no
recommendation about any package; that is the policy engine's job, and it is in
[`analysis.v1.md`](analysis.v1.md).

## Root

| Field | Type | Notes |
|---|---|---|
| `schemaVersion` | integer | `1` |
| `toolVersion` | string | |
| `policyVersion` | string | |
| `findings` | array of `DoctorFinding` | Ordered worst-severity-last in human output; the array here is in check order |
| `redaction` | string | **`--report` only.** `GROUNDED`, `HEURISTIC` or `AS_CAPTURED` — see Redaction. Added in v1; additive, so no version bump |
| `machine` | `Machine` | **`--report` only.** As in `snapshot.v1.md` |
| `pathEntries` | integer | **`--report` only.** How many `PATH` entries were seen. The entries themselves are not emitted; only findings quote one, redacted |
| `providers` | object | **`--report` only.** Keyed by provider id: `presence`, `version`, `prefix`, `packages` |

## `DoctorFinding`

| Field | Type | Notes |
|---|---|---|
| `id` | string | Stable, lower-case. The list below is the current set |
| `severity` | string | `OK`, `WARNING`, `ERROR` |
| `reason` | `Reason` | `{ "id": "doctor.*", "params": { ... } }`. Carries no display text |
| `provider` | string, optional | The provider id the finding is about, when it is about one |

## Finding identifiers

| `id` | Typical severity | `reason.id` | Parameters |
|---|---|---|---|
| `provider_present` | `OK` | `doctor.provider_present` | `provider`, `version` |
| `provider_absent` | `OK` | `doctor.provider_absent` | `provider` |
| `provider_broken` | `ERROR` | `doctor.provider_broken` | `provider`, `reason` |
| `provider_architecture_mismatch` | `WARNING` | `doctor.provider_architecture_mismatch` | `provider`, `provider_arch`, `machine_arch` |
| `provider_support_degraded` | `WARNING` | `doctor.provider_support` | `provider`, `tier`, `source`, `readOn` |
| `provider_support_ending` | `WARNING` | `doctor.provider_support_ending` | `provider`, `endDate`, `source`, `readOn` |
| `prefix_not_on_path` | `WARNING` | `doctor.prefix_not_on_path` | `provider`, `prefix` |
| `path_order` | `OK` or `WARNING` | `doctor.path_order` | `order` |
| `path_duplicate` | `WARNING` | `doctor.path_duplicate` | `entry` |
| `path_missing` | `WARNING` | `doctor.path_missing` | `entry` |
| `path_relative` | `WARNING` | `doctor.path_relative` | `entry` |
| `path_world_writable` | `WARNING` | `doctor.path_world_writable` | `entry` |
| `path_group_writable` | `WARNING` | `doctor.path_group_writable` | `entry` |
| `path_owner_writable` | `WARNING` | `doctor.path_owner_writable` | `entry` |
| `path_world_writable_precedes_system` | `ERROR` | `doctor.path_world_writable_precedes_system` | `entry` |
| `path_relative_precedes_system` | `ERROR` | `doctor.path_relative_precedes_system` | `entry` |
| `path_finding_unknown` | `WARNING` | `doctor.path_finding_unknown` | `entry` |
| `path_has_no_system_directory` | `ERROR` | `doctor.path_has_no_system_directory` | — |
| `build_variable_names_prefix` | `WARNING` | `doctor.build_variable_names_prefix` | `variable`, `provider` |
| `running_translated` | `WARNING` | `doctor.running_translated` | — |
| `command_line_tools` | `OK`, `WARNING` or `ERROR` | `doctor.command_line_tools_present` / `_missing` / `_unknown` | — |
| `policy_stale` | `WARNING` | (policy staleness) | the read date and the threshold |

**An `ERROR` is a search path somebody other than this user decides.** A
world-writable directory ahead of the system ones means any account on the
machine chooses what `git` means; an entry that is not absolute means the
working directory does, and any repository you clone can carry one. Both produce
wrong answers, which is what this project reserves `ERROR` for.

**A directory you own, or one your admin group owns, is reported as a
`WARNING` wherever it sits.** It is worth knowing — anything running as you can
plant a binary there, which is how a `curl | sh` installer or a package
postinstall would do it — but it is not a broken machine: `~/bin` ahead of
`/usr/bin` is how developer machines are arranged, and `/usr/local/bin` is
`admin`-writable because that is how Homebrew installs itself, on the very
prefix `prefix_not_on_path` warns you about *not* having on your `PATH`. An
error every developer machine reports is an error nobody reads.

**One entry yields one writable finding.** A directory that is both
group-writable and owner-writable is one directory with one remedy; the
strongest writer is the one named.

**A dated provider policy is a `WARNING`, never an `ERROR`.** A support tier or
an announced end-of-support date is something the user should know about and
plan around; it does not mean their machine is broken. Both carry `source` and
`readOn` in their parameters so the reader can check the claim themselves —
that is INV-20 reaching the user interface.

## Redaction

`doctor --report` writes a file the user attaches to an issue. It **may**
contain versions, architecture, macOS version, provider prefixes, package names
and findings. It **must never** contain usernames, home paths, environment
variable values or tokens, or anything outside the two providers' scope. A
parameter that can carry a path is redacted: the home directory becomes `~`.

The file written by `--report` carries a **`redaction`** field, additive in v1,
saying which of three redactions produced it:

| Value | Meaning |
|---|---|
| `GROUNDED` | The home directory was known. Every occurrence of it, and of the user name, was replaced. |
| `HEURISTIC` | A live run whose home could not be resolved — `HOME` unset, or set to a relative path, which happens under `cron`, `launchd` and some CI. Paths shaped like a macOS home (`/Users/<name>`) were redacted, but a home somewhere else — a network mount, a relocated account — cannot be recognised and is still present. |
| `AS_CAPTURED` | The analysis came from a recording (`--replay`) or a snapshot file (`--from-snapshot`). Those were redacted when they were captured and are not redacted again, and neither carries a record of *how*. This is an unknown, not a guarantee: a snapshot captured with no resolvable home was redacted heuristically, and reading it back cannot discover that. |

The field exists because the two files are otherwise indistinguishable, and a
file that merely *looks* redacted is the one a user attaches without thinking.
Before it existed, a `--report` run with `HOME` unset published the real home
path — measured at 39 occurrences, username included. A reader who sees
`HEURISTIC` should skim the paths before sharing.

The user sees the file and chooses to share it. **MacPortsBrew never uploads
it, and makes no network call that reports anything about the user.**

## Worked example

`macportsbrew doctor --json` on an Intel Mac with Homebrew and no MacPorts:

```json
{
  "schemaVersion": 1,
  "toolVersion": "0.1.0",
  "policyVersion": "2026.09.02-1",
  "findings": [
    {
      "id": "path_duplicate",
      "severity": "WARNING",
      "reason": { "id": "doctor.path_duplicate", "params": { "entry": "~/some/dir/bin" } }
    },
    {
      "id": "provider_support_degraded",
      "severity": "WARNING",
      "reason": {
        "id": "doctor.provider_support",
        "params": {
          "provider": "homebrew",
          "tier": "Degraded",
          "source": "https://docs.brew.sh/Support-Tiers",
          "readOn": "2026-09-02"
        }
      },
      "provider": "homebrew"
    },
    {
      "id": "provider_support_ending",
      "severity": "WARNING",
      "reason": {
        "id": "doctor.provider_support_ending",
        "params": {
          "provider": "homebrew",
          "endDate": "2027-09-01",
          "source": "https://docs.brew.sh/Support-Tiers",
          "readOn": "2026-09-02"
        }
      },
      "provider": "homebrew"
    },
    {
      "id": "command_line_tools",
      "severity": "OK",
      "reason": { "id": "doctor.command_line_tools_present" }
    },
    {
      "id": "provider_absent",
      "severity": "OK",
      "reason": { "id": "doctor.provider_absent", "params": { "provider": "macports" } },
      "provider": "macports"
    },
    {
      "id": "provider_present",
      "severity": "OK",
      "reason": { "id": "doctor.provider_present", "params": { "provider": "homebrew", "version": "6.0.21" } },
      "provider": "homebrew"
    }
  ]
}
```

Note the `~` in the `path_duplicate` entry: that is the redaction, applied
before the value ever reaches the payload.

## In this build

`--deep` (inspecting binaries for cross-prefix links) is accepted and reports
that it is not part of this build. Every other check runs, and every one of
them works identically from `--from-snapshot` and under `--replay`, because
`doctor` reads the snapshot and the policy data and nothing else.
