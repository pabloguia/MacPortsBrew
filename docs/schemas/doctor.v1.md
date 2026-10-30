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
| `concurrentProviderActivity` | object | Always written by `doctor --json`, last (not in the `--report` file): whether a provider may have been changing the machine while it was read, never a check this build did not make — the one shape every document writes it in ([`migrate.v1.md`](migrate.v1.md#concurrentprovideractivity)). Added 2026-10-03; additive, so no version bump (SE-015) |

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
| `provider_installed_once` | `OK` | `doctor.provider_installed_once` | `provider` |
| `provider_installed_more_than_once` | `WARNING` | `doctor.provider_installed_more_than_once` | `provider`, `count`, `prefixes`, `running` |
| `provider_installations_unknown` | `OK` | `doctor.provider_installations_unknown` | `provider` |
| `provider_broken` | `ERROR` | `doctor.provider_broken` | `provider`, `reason` |
| `provider_architecture_mismatch` | `WARNING` | `doctor.provider_architecture_mismatch` | `provider`, `provider_arch`, `machine_arch` |
| `provider_support_degraded` | `WARNING` | `doctor.provider_support` | `provider`, `tier`, `source`, `readOn` |
| `provider_support_ending` | `WARNING` | `doctor.provider_support_ending` | `provider`, `endDate`, `source`, `readOn` |
| `prefix_not_on_path` | `WARNING` | `doctor.prefix_not_on_path` | `provider`, `prefix` |
| `provider_prefix_is_not_absolute` | `WARNING` | `doctor.provider_prefix_is_not_absolute` | `provider`, `prefix` |
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
| `path_unknown` | `WARNING` | `doctor.path_unknown` | — |
| `cross_provider_exposure` | `OK` or `WARNING` | `doctor.cross_provider_exposure` | `affected`, `intruder`, `intruder_prefix`, `defence` |
| `cross_provider_exposure_unadvised` | `OK` | `doctor.cross_provider_exposure_unadvised` | `affected`, `intruder` |
| `path_has_no_system_directory` | `ERROR` | `doctor.path_has_no_system_directory` | — |
| `build_variable_names_prefix` | `WARNING` | `doctor.build_variable_names_prefix` | `variable`, `provider` |
| `running_translated` | `WARNING` | `doctor.running_translated` | — |
| `command_line_tools` | `OK`, `WARNING` or `ERROR` | `doctor.command_line_tools_present` / `_missing` / `_unknown` | — |
| `policy_stale` | `WARNING` | (policy staleness) | the read date and the threshold |
| `provider_configuration_none` | `OK` | `doctor.provider_configuration_none` | `provider` |
| `provider_configuration_outranks` | `WARNING` | `doctor.provider_configuration_outranks` | `provider`, `file`, `keys` |
| `provider_configuration_analytics` | `WARNING` | `doctor.provider_configuration_analytics` | `provider`, `file`, `keys` (only the keys that govern the provider's analytics; DP-028) |
| `provider_reads_offline` | `OK` | `doctor.provider_reads_offline` | `provider` |
| `provider_read_may_reach_network` | `WARNING` | `doctor.provider_read_may_reach_network` | `provider` |
| `provider_network_use_unknown` | `WARNING` | `doctor.provider_network_use_unknown` | `provider` |
| `provider_configuration_unreadable` | `WARNING` | `doctor.provider_configuration_unreadable` | `provider`, `file` |
| `provider_configuration_unknown` | `WARNING` | `doctor.provider_configuration_unknown` | `provider` |

**Where a provider is installed is reported, per present provider** (since
2026-10-12, pass 12). A Mac migrated from another can keep a working copy of a
package manager under the old prefix, and detection that stops at the first
prefix never mentions it (`04-migration-and-safety.md`, "Cross-provider
contamination"). `provider_installed_more_than_once` is a `WARNING` naming every
place (`prefixes` is the places joined by `, `, in detection order, and `running` the detected prefix this tool runs - `?` when the snapshot has none - so the copy to keep is not a guess);
`provider_installed_once` is `OK`. **An unknown is never "one"**: a recording, a
snapshot taken before the field and a provider found at no candidate prefix all
say `provider_installations_unknown` at `OK`, because nothing is known to be
wrong. The finding is the same for every provider and names no path of its own;
the candidates are dated data, `data/policy/provider-prefixes.toml`.

**A provider's own configuration is reported, per present provider** (since
2026-10-04; DP-027, ruled 2026-10-03). Homebrew applies its `brew.env` files over the environment this tool runs it
with, and nothing in that environment can win (R-15, Homebrew 7.0.4, read
2026-10-02). MacPorts overlays a user `macports.conf` on its system file; it
cannot undo the `-q` this tool passes, but it can redirect what a read reports
and steer an install (R-16, MacPorts 2.12.6, read 2026-10-03). Which files are looked for and how a line is
read is dated data, `data/policy/provider-configuration.toml`. A file setting
any key the provider's reader accepts, or one that exists and cannot be read, or
whose key cannot be named, is a `WARNING` naming the
file and, when it was read, the key names (a key that cannot be named is
reported as unreadable) — never a value. Nothing found is an `OK`. An input that
does not record the files — a replay, or a snapshot taken before they were
recorded — is a `WARNING` that it is unknown, never the `OK` it could be mistaken
for.

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

## Cross-provider exposure

One `cross_provider_exposure` finding per **ordered pair** of present providers; its
`provider` is the **affected** one and `intruder` the other, so two providers give two
findings, one in each direction (DP-041, SE-098). It is `WARNING` only when `PATH` is
known and the intruder's directory is searched before the affected provider's, or the
intruder is on it and the affected provider's directory is not; otherwise `OK`, an unknown `PATH` (a
recording) included. `defence` is a stable identifier from a closed vocabulary that the
human output renders as a sentence: `opt_in_trace` (the affected provider offers a trace
mode that can prevent conflicts with other prefixes, but only when asked to) or `default_clean_path` (its own
build tooling constructs the build's `PATH`, so the user's is not an input by default).
The two directions carry **different** values because the data says so
(`data/policy/cross-provider-advice.toml`), not because any code does. An affected
provider the data does not describe gets `cross_provider_exposure_unadvised` instead: the
pair is reported and no direction-specific claim is made. The finding reports only;
nothing is changed.

A recording carries no `PATH`, so both findings below are `OK`:

<!-- sample: --replay fixtures/recordings/both-providers-arm64-macos15.json --json doctor at=/findings/5 -->
```json
{
  "id": "cross_provider_exposure",
  "severity": "OK",
  "reason": {
    "id": "doctor.cross_provider_exposure",
    "params": {
      "affected": "homebrew",
      "defence": "default_clean_path",
      "intruder": "macports",
      "intruder_prefix": "/opt/local"
    }
  },
  "provider": "homebrew"
}
```

<!-- sample: --replay fixtures/recordings/both-providers-arm64-macos15.json --json doctor at=/findings/6 -->
```json
{
  "id": "cross_provider_exposure",
  "severity": "OK",
  "reason": {
    "id": "doctor.cross_provider_exposure",
    "params": {
      "affected": "macports",
      "defence": "opt_in_trace",
      "intruder": "homebrew",
      "intruder_prefix": "/opt/homebrew"
    }
  },
  "provider": "macports"
}
```

## The `interrupted_run` finding

Added in pass 08 slice 5 (additive; no version bump). One finding with the id
`interrupted_run`, always present, built by the shell from a look at the state
directory that creates nothing and takes no lock (`doctor` itself stays a pure
function of the snapshot and the policy). Its `reason.id` says which of these it
found; `WARNING` is used for everything that needs a person's attention, never
`ERROR`, because the exit-5 code is for unhealthy providers and a stranded run
is not one (it blocks the next command that changes packages with exit 4).

| `reason.id` | Severity | Meaning |
|---|---|---|
| `doctor.interrupted_run_not_checked` | `OK` | A recording or a snapshot file, or the user could not be established. Under `--replay` and `--from-snapshot` this is always the answer, so the output stays a function of the input |
| `doctor.interrupted_run_none` | `OK` | No marker |
| `doctor.interrupted_run_in_progress` | `WARNING` | A marker and the run lock is held: a run is in progress. The look takes a shared lock for an instant, which can make a command starting at that instant exit 7 |
| `doctor.interrupted_run_possibly_in_progress` | `WARNING` | A marker, and the lock could not be tested |
| `doctor.interrupted_run_marker_unreadable` | `WARNING` | The marker cannot be read; nothing is known |
| `doctor.interrupted_run_recorded_not_confirmed` | `WARNING` | A marker nobody holds the lock for, and the run has a record that reads; `changed` lists the mutating steps it dispatched |
| `doctor.interrupted_run_recorded_unreadable` | `WARNING` | A marker, and a record at its id that cannot be read as one |
| `doctor.interrupted_run_started_not_recorded` | `WARNING` | A marker and no record: the marker and the snapshot it names are the only trace |

The parameters (`run_id`, `snapshot`, `changed`) are identifiers and file names,
never paths. Nothing is deleted by `doctor`; `history acknowledge` is the one
deliberate way to deal with a marker.

## Worked example

`macportsbrew doctor --json` on an Intel Mac with Homebrew and no MacPorts:

<!-- sample: --replay fixtures/recordings/homebrew-only-intel-macos26.json --json doctor -->
```json
{
  "schemaVersion": 1,
  "toolVersion": "0.1.0",
  "policyVersion": "2026.10.18-1",
  "findings": [
    {
      "id": "path_unknown",
      "severity": "WARNING",
      "reason": {
        "id": "doctor.path_unknown"
      }
    },
    {
      "id": "provider_configuration_unknown",
      "severity": "WARNING",
      "reason": {
        "id": "doctor.provider_configuration_unknown",
        "params": {
          "provider": "homebrew"
        }
      },
      "provider": "homebrew"
    },
    {
      "id": "provider_network_use_unknown",
      "severity": "WARNING",
      "reason": {
        "id": "doctor.provider_network_use_unknown",
        "params": {
          "provider": "homebrew"
        }
      },
      "provider": "homebrew"
    },
    {
      "id": "provider_support_degraded",
      "severity": "WARNING",
      "reason": {
        "id": "doctor.provider_support",
        "params": {
          "provider": "homebrew",
          "readOn": "2026-09-04",
          "source": "https://docs.brew.sh/Support-Tiers",
          "tier": "DEGRADED"
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
          "endDate": "2027-09-01",
          "provider": "homebrew",
          "readOn": "2026-09-04",
          "source": "https://docs.brew.sh/Support-Tiers"
        }
      },
      "provider": "homebrew"
    },
    {
      "id": "command_line_tools",
      "severity": "OK",
      "reason": {
        "id": "doctor.command_line_tools_present"
      }
    },
    {
      "id": "interrupted_run",
      "severity": "OK",
      "reason": {
        "id": "doctor.interrupted_run_not_checked"
      }
    },
    {
      "id": "provider_absent",
      "severity": "OK",
      "reason": {
        "id": "doctor.provider_absent",
        "params": {
          "provider": "macports"
        }
      },
      "provider": "macports"
    },
    {
      "id": "provider_installations_unknown",
      "severity": "OK",
      "reason": {
        "id": "doctor.provider_installations_unknown",
        "params": {
          "provider": "homebrew"
        }
      },
      "provider": "homebrew"
    },
    {
      "id": "provider_present",
      "severity": "OK",
      "reason": {
        "id": "doctor.provider_present",
        "params": {
          "provider": "homebrew",
          "version": "6.0.21"
        }
      },
      "provider": "homebrew"
    }
  ],
  "concurrentProviderActivity": {
    "activity": "UNKNOWABLE",
    "possiblyInconsistent": true
  }
}
```

Note the `~` in the `path_duplicate` entry: that is the redaction, applied
before the value ever reaches the payload.

## In this build

`--deep` (inspecting binaries for cross-prefix links) is accepted and reports
that it is not part of this build. Every other check runs, and every one of
them works identically from `--from-snapshot` and under `--replay`, because
`doctor` reads the snapshot and the policy data and nothing else.

## Not in this version

Nothing outstanding. The concurrency flag, listed here until 2026-10-03, is
now carried at the root of what `doctor --json` prints (SE-015). The file
`--report` writes does not carry it: that file is a redacted diagnostic about
the machine, written before and independently of the printed document.
