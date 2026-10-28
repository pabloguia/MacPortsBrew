# R-23 — What part J's elevation needs from the process allowlist and the stop gate

- **Status:** partially answered (the gate change is settled; the allowlist entry waits for J's design)
- **Answered on:** 2026-10-28
- **Machine:** macOS 26.7.1, x86_64
- **Blocking:** work order 16 part P item 3, part J

In this note "the elevation program" is the one DP-046 (b) names; the word is
kept out of the text, as `.claude/hooks/test-hooks.sh` keeps it out of its own.
The program was **not run** in any form, and no policy file was read. Its manual
pages were rendered from their files with `mandoc -T ascii` (version 1.9.17p2,
dated 7 June 2025).

## Findings

### F-1. The binary

`/usr/bin/<program>`, mode `-r-s--x--x`, owner `root:wheel` (observed by listing
`/usr/bin`). The manual names the policy file under `/private/etc/`.

### F-2. Today's allowlist cannot express it, and the entry belongs to J's design

`crates/adapters/src/process/mod.rs`:

- `PERMITTED_PROGRAMS` holds leaf names (lines 73-82), and
  `is_permitted_program` admits any absolute, `..`-free path with a listed leaf
  and no read list (lines 542-565): a bare leaf entry would admit
  `/tmp/x/<program>`.
- `is_permitted_invocation` refuses a listed program that is neither a provider
  with a read list nor a system probe (lines 460-488), so a leaf alone can never
  run, and `every_permitted_program_is_a_probe_or_has_a_read_list` would fail.
- `PERMITTED_SUBCOMMANDS` holds reads only (lines 98-100): no mutating
  subcommand is listed, so there is nothing yet for a wrapper to wrap.

So the entry is not a one-line addition: it is a third category of program (a
wrapper with one exact flag prefix, pinned to a path taken from `data/policy/`,
wrapping only a permitted **mutating** provider subcommand — MacPorts'
`install`/`uninstall`, the only `required` cells in
`data/policy/provider-commands.toml` lines 250-253). That shape is J's design,
which work order 16 J.2 puts before any code. **P stages only the gate change;
J stages the allowlist entry and the mutating list with its design**, as the
work order allows ("a later part that finds a need P missed stages it the same
way").

### F-3. The flags J's design must weigh

From the manual (`mandoc` rendering, line numbers in the scratch rendering):

- `--` ends the program's own options, so the provider's argv is never parsed
  as its options.
- `-k`, with a command, ignores the cached credential and does not update it:
  it satisfies "no credential cached across runs, and none left behind".
- `-K` removes every cached credential for the user and cannot take a command:
  not wanted.
- `-n` never prompts. The current runner gives the child a null stdin and its
  own process group (`crates/adapters/src/process/system.rs` lines 257 and 290),
  so without a terminal-attached mode a prompt cannot reach the user; J chooses
  between `-k -n --` (works only under a no-password policy) and `-k --` with a
  terminal-attached runner mode for the elevated spec only.
- Refused by construction with an exact prefix: `-E`/`--preserve-env` (an INV-10
  breach), `-s`/`-i` (a shell), `-S` (the tool handling a password),
  `VAR=value` arguments, and the other options.
- The child's environment is decided by the policy's `env_reset`/`env_keep`, not
  by what the tool passes: J item 4's INV-10 test reads the received
  environment.
- Detecting the policy has no read-only route here: the file is root-only, and
  listing the policy runs the program. J's research note settles this.

### F-4. The stop gate

`.claude/hooks/stop-gate.sh` lines 123-127 refuse the program's name, with five
other elevation, service-control and symlink tokens, in non-comment code under
every `src/` and `build.rs` (lines 111-115); `data/policy/` is not scanned.
Taking the path from `data/policy/` keeps the full path out of source, but the
allowlist entry is still a literal in `process/mod.rs` — deliberately, so that
nothing outside the source decides which program runs (`mod.rs` lines 54-72).
So the gate must admit the name **only under `crates/adapters/src/process/`**,
decided by path, with every other token still refused everywhere. That change
and its `test-hooks.sh` cases are P's staged item; the `.codex/` mirror is
regenerated with it (`scripts/generate-codex-layer.sh`).

### F-5. What J also changes, unguarded

- `invariant_06_reading_the_machine_needs_no_elevation`
  (`crates/adapters/tests/invariants.rs` lines 562-579) needs the same
  `process/` exemption.
- `crates/cli/tests/migrate.rs` lines 245-251 and 577-579 assume the name never
  appears in output; rendering an elevated step's exact argv changes that.
- `.claude/hooks/guard-provider-mutation.sh` still refuses the name on any
  command line; commits and searches keep it as data (§3 item 11).
- A recording of an elevated run cannot be made here (§3 items 2 and 9); it is an
  owner item for a disposable machine.
