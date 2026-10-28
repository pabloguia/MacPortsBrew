# 06 — CLI, output contract and i18n

**Status:** Tier 3 unless marked.

## Surface

**MUST.** The canonical command is `macportsbrew`; **`mpb`** is installed
alongside it as an equal short alias, not a wrapper. Both resolve to the same
binary and behave identically. Help output, documentation and error messages use
`macportsbrew` as the canonical spelling and mention `mpb` once, near the top.

**MUST.** Installation never makes `mpb` shadow another `mpb` — MIT
Photonic-Bands ships one, installed from source or through other channels
(ADR-0003). The signed `.pkg` has no scripts by design (ADR-0005), so it cannot detect
one: the release artifacts carry both names and the guard is `PATH` order — the
documented line **appends** the install directory, never prepends it, so an
earlier `mpb` wins (DP-033, ruled 2026-10-23). An install route that can run
code (a future one; a cargo build runs none, and builds both binaries) MUST
detect an existing `mpb` on `PATH` and, if found, report the conflict and
install only `macportsbrew` unless the user explicitly asks otherwise. No route
in this repository performs that detection today. A tool whose whole purpose is preventing
accidental executable shadowing must not start by shadowing something (see
`05-environment-and-primary-provider.md`).

```
macportsbrew                      interactive overview (default)
macportsbrew status               machine + provider dashboard
macportsbrew analyze [pkg...]     inventory + mapping + assessment + recommendations
macportsbrew list                 packages (--provider, --advanced)
macportsbrew explain <pkg>        full recommendation derivation
macportsbrew plan <pkg> --to <provider> [--retain-source]   build a migration plan (no mutation)
macportsbrew migrate <pkg> --to <provider> [--retain-source] [--yes]
                                  show a plan; --yes executes it [this build: the gate refuses every plan]
macportsbrew primary <pkg> <provider>       package-level selection [later pass]
macportsbrew provider-priority <provider>   global priority         [later pass]
macportsbrew providers            detect / install a provider (consent required)
macportsbrew snapshot [--out <file>]        capture both providers' state
                                            (--out required under --replay
                                            and --from-snapshot)
macportsbrew restore --plan (--run <id> | --snapshot <file>)
                                  reviewable plan back to that state; the undo path
macportsbrew restore --run <id>   plan, review, confirm, execute   [later pass, with migrate]
macportsbrew env [--revert]       managed shell block: preview / remove   [later pass]
macportsbrew doctor [--deep] [--report]
macportsbrew history [show <id>] [--limit <n>]
                                  runs, newest first: outcome, restorable?, snapshot; `show` prints one run's record
                                  and its steps. Reads this machine's own state directory and writes nothing in it;
                                  never under --replay or --from-snapshot, and never as root
macportsbrew history acknowledge <id> [--yes]
                                  deal with the interrupted-run marker of that run; without --yes only says
                                  what it would do; with --yes writes the state directory and nothing else
macportsbrew cleanup              [later pass]
```

`history` is how a user finds the run they want to undo, and
`restore --run <id> --plan` is how they undo it. What that promises, what it
does not, what every mutating run prints when it finishes, and what `history`
shows per run are specified once, in `04-migration-and-safety.md` ("Undoing a
migration"; "History"); this chapter only renders them.

Global flags: `--json`, `--language <tag>`, `--dry-run`, `--yes`,
`--no-network`, `--refresh`, `--from-snapshot <file>`, `--verbose`, `--no-color`.

**MUST.** `--dry-run` is accepted by every mutating command and is the default
for anything destructive until explicitly confirmed.

## Output contract

**MUST.** Every read command supports `--json` with a **versioned schema**
(`schemaVersion` in the payload, documented under `docs/schemas/`, created by
the Foundation Pass). Human output
may change freely; JSON output is a compatibility surface and changes need a
schema version bump plus a `spec-evolution.md` entry.

**MUST.** JSON goes to stdout and nothing else does. Progress, warnings and
prompts go to stderr. Non-TTY output is plain, unstyled and stable.

**MUST.** Stable exit codes, documented and tested:

```
0 success · 1 generic failure · 2 usage error · 3 partial success
4 blocked by a safety check · 5 provider unavailable/unhealthy
6 user aborted · 7 lock held by another instance · 8 stale/unusable input
```

A restore that cannot proceed because the run's snapshot is missing or
unreadable exits 8; a restore plan that contains a `MANUAL_REVIEW` step exits
4 when executed, with the step named; a mutating command that refuses to start
because an interrupted run has not been looked at exits 4 and names the run
(`RUN_INTERRUPTED`); a mutating command that finds another MacPortsBrew run
holding the single-instance lock exits 7 (`RUN_LOCK_HELD`);
and `migrate` reports *nothing to move*, exit 0, when another provider was not
detected, naming it in the text and in `notDetected` (SE-092, DP-026). Another
provider that is broken refuses as `PROVIDER_UNHEALTHY` (exit 5), and one
missing from the input or with an unread inventory as `SAFETY_UNVERIFIABLE`
(exit 4); `PROVIDER_PRESENCE_UNPROVEN` is reserved and no longer raised.

**MUST.** An error taxonomy with stable identifiers, each carrying a
user-facing remediation string. `"error: exit status 1"` is never acceptable
user-facing output.

## Interactive experience

**SHOULD.** The interactive view is the primary experience and shows one row per
*package*, both providers side by side, the effective resolution, and the
recommendation. Dependency counts summarised, never enumerated (INV-13).
Recommended actions preselected; everything overridable.

**MUST.** Anything the TUI can do, flags can do too. No functionality is
reachable only through interactive keystrokes — that would make the tool
unscriptable and untestable.

## Caching and network

**MUST.** `analyze` works with no network, degrading clearly-marked network
dependent facts to `UNKNOWN` rather than failing or guessing. *Untested on a
machine whose Homebrew cache is empty, where Homebrew's own reads try to
download the catalogue first (R-15, an open residual).*

**MUST.** A cache with an explicit TTL, provenance (which command produced it,
when, against which provider version) and a `--refresh` bypass. Cached facts are
labelled as cached in `--json` output. The cache is never authoritative for a
mutating decision.

## Internationalisation

**MUST.** All user-facing text goes through the i18n layer. Domain, policy and
adapter code contain no display strings — they emit stable reason and error
identifiers that the UI renders.

**MUST.** English is the canonical source locale. Detect the system locale;
fall back to English; allow `--language` and an environment override.

**MUST.** Machine-readable output, logs, history and error identifiers are
**always English/stable identifiers**, never localised. Locale affects
presentation only.

**Scope for the foundation pass:** the i18n *architecture* plus a complete
English catalogue. Target locales are `en`, `pt-BR`, `es`, `ja`, `zh-CN`;
translations land in later passes (`pt-BR` has, 2026-10-12; `es`, `ja` and
`zh-CN` have not). Rationale in `docs/spec-evolution.md`, SE-011.

**MUST.** CI enforces key parity across catalogues and fails on a missing or
orphaned key. A pseudolocale (`--language qps`) that lengthens and accents every
string is available for layout testing.

**MUST.** Plural and gender handling uses a message format that supports the
target locales' rules; do not concatenate sentence fragments.

**MUST.** Table and column layout measures **display width**, not byte or rune
count — Japanese and Chinese text is double-width and will corrupt naïve column
alignment. This is tested with the pseudolocale and with CJK fixtures.

## Privacy and diagnostics

**MUST.** No telemetry, no network calls that report anything about the user.
This MUST is about what MacPortsBrew itself does, as the charter's non-goals
now say (DP-028, ruled 2026-10-27). Two things a provider does on its own
account are disclosed rather than prevented (R-15, 2026-10-02): a `brew.env`
in Homebrew's configuration can switch Homebrew's own analytics back on for
the reads this tool asks of it, and Homebrew downloads its catalogue on an
empty cache. `doctor` names a configuration key that governs a provider's
analytics (`provider_configuration_analytics`) and says, per provider, whether
a read may reach the network (`provider_read_may_reach_network`,
`provider_reads_offline`, `provider_network_use_unknown`); a plan's risks
carry the configuration, and a mutation under it is refused (DP-027). README
and wiki say so (SE-016 item 3).

**MUST.** `doctor --report` writes a redacted local diagnostic file the user can
attach to an issue: versions, architecture, macOS version, provider prefixes,
package names, findings. Never: usernames, home paths, environment variable
values, tokens, or anything outside the two providers' scope. The user sees the
file and chooses to share it; MacPortsBrew never uploads it.

## Advanced mode

**MUST.** Dependency-level functionality is reachable only through an explicit
advanced mode (`--advanced`, or an equivalent deliberate action), and entering
it displays a warning to the effect that these packages are managed
automatically by their provider and that removing or migrating them
individually can break packages that depend on them.

**Naming a package is such a deliberate action, for that package only**
(DP-037, ruled 2026-10-27). When `plan`, `migrate` or `restore` acts on a row
outside the default view — judged by the **source row** the plan would remove,
not by whether the unit is listed — and `--advanced` was not given, the
warning is shown, the plan records `enteredBy: NAMED` (`ADVANCED` with
`--advanced`, `DEFAULT_VIEW` otherwise), and `--json` carries
`warnings: ["advanced.warning"]`. The named package's own dependencies are
never added. The stronger confirmation below is defined with the confirmation
levels in work order 16 part K; until then no plan executes.

**MUST.** In advanced mode: reverse dependencies are still checked (INV-04);
dependencies are still never preselected; destructive actions require a stronger
confirmation than in normal mode; and the other provider is still never treated
as satisfying a dependency (INV-08).

## Project language

**MUST.** Everything internal is written in English: source, identifiers,
comments, test names, log messages, configuration keys, error identifiers,
commit messages, ADRs, specification, `AGENTS.md`. Only text presented to the
user is translated, and only through the i18n layer.
