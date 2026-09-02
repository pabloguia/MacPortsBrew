# R-1 — Homebrew read interfaces

- **Status:** partially answered — one side-effect finding recorded; the
  interface survey itself is the Foundation Pass's to complete
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (build 25G83), x86_64, Homebrew 6.0.21 @
  `/usr/local`, MacPorts not installed
- **Blocking:** `04-migration-and-safety.md` (Process execution: "provider
  invocations set the environment that keeps them quiet and side-effect-free
  during analysis"); `02-domain-model.md` (artifact availability "must not
  trigger a provider auto-update"); the read-only allowlist in
  `.claude/settings.json`.

## Question

Which Homebrew read commands trigger a network fetch, an auto-update or
another side effect on the installation, and how is that suppressed? This
note records one observation made incidentally during the Repository
Readiness Pass; the full survey (installed, requested, dependencies, reverse
dependencies, metadata, deprecation state, pins, taps, prefix, JSON stability)
remains open.

## Evidence

Command, run 2026-09-02 from a Claude Code session with the checked-in
environment (`HOMEBREW_NO_AUTO_UPDATE=1`, `HOMEBREW_NO_ANALYTICS=1`,
`HOMEBREW_NO_ENV_HINTS=1`; `HOMEBREW_NO_INSTALL_FROM_API` unset, i.e. the
default API mode), for a formula name that does not exist:

```
$ brew info mpb
==> Tapping homebrew/core
Cloning into '/usr/local/Homebrew/Library/Taps/homebrew/homebrew-core'...
Updating files: 100% (9129/9129), done.
Tapped 5 commands and 8586 formulae (9,141 files, 1.4GB).
Error: No available formula with the name "mpb". Did you mean mpv, mpd, mpc or mob?
==> Searching for a previously deleted formula (in the last month)...
```

Wall-clock: the command exceeded a 120-second timeout and finished in the
background. Disk: a 1.4 GB clone of `homebrew/core` now exists under the
Homebrew prefix that was not there before.

## Findings

1. `brew info <name>` for a name the API does not know **taps `homebrew/core`**
   — a network clone of the full formula repository into the provider's
   prefix — before reporting the miss, in order to search for a recently
   deleted formula. A command classified as read-only wrote 1.4 GB into the
   developer's Homebrew installation and took minutes.
2. The environment variables the project sets to keep Homebrew quiet do not
   suppress this. Which variable or flag does (if any — candidates to test:
   `HOMEBREW_NO_INSTALL_FROM_API`, `HOMEBREW_NO_AUTO_UPDATE`, a `--json`
   query against the API only) is **not determined here**; the Foundation
   Pass must establish it before the Homebrew adapter's `describe(pkg)` or
   `artifactAvailability(pkg)` is allowed to run on an unknown name.
3. For the domain: "package not found" is a legitimate answer the adapter
   must return without side effects. An adapter that reaches it the way
   `brew info` does violates the analysis-must-not-mutate rule.

## Cannot determine

- The suppressing variable or flag, and whether the tap clone also happens
  for other read subcommands (`brew deps`, `brew uses`) on unknown names.
- Whether this behaviour is specific to Homebrew 6.x or older.

## Impact on the specification

- Confirms `04-migration-and-safety.md` (Process execution) and
  `02-domain-model.md` (artifact availability): the quiet environment is a
  real requirement and is not yet fully known.
- The read-only allowlist in `.claude/settings.json` is correct in intent
  (`brew info` does not mutate installed packages) but an agent should treat
  `brew info <unknown>` as slow and disk-consuming until the suppressing
  setting is known. No specification change; the fact belongs in this note
  and, once complete, in the adapter's environment construction.

## Re-check when

Homebrew's major version changes, or the Foundation Pass completes the R-1
survey — this note is then folded into the full answer.
