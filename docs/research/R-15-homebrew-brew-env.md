# R-15 — Can a `brew.env` file undo the quieting environment the process executor gives `brew`?

- **Status:** answered (one residual left open, see "Cannot determine")
- **Answered on:** 2026-10-02
- **Machine:** macOS 26.7.1 (25G241), x86_64, Homebrew 7.0.4 @ `/usr/local`
  (repository `/usr/local/Homebrew`, HEAD `edb70f031e41`, committed
  2026-09-17), MacPorts 2.12.6 @ `/opt/local`
- **Blocking:** the mutating pass's reliance on the Homebrew quieting
  environment (SE-092 end-of-pass amendment, "Not closed" (c); pass-05 report,
  "Recommended next tasks" 5; work order `prompts/06-hardening-before-mutation.md`
  §5 item 1). Also bears on SE-050's argument-shape clause, on
  `docs/spec/04-migration-and-safety.md` ("Process execution": "`HOMEBREW_NO_AUTO_UPDATE`
  is the one that does the offline work") and on R-1 finding 2.

**About the identifier.** `docs/spec/09-research-checklist.md` has no item for
this question. Its items run to R-14, so this note takes the next free number,
R-15. It is not filed as `R-1a`, because R-1 asks what Homebrew's read
interfaces are, and this note asks whether the environment those reads run in
can be trusted. Whether the checklist gains an R-15 entry is for the main agent.

## Question

The process executor builds Homebrew's environment from nothing (INV-10). Can a
`brew.env` file override that environment, or add to it? There are three
candidate files: one in the user's real `HOME`, one in the Homebrew prefix and
one system-wide. Does the `HOME` the executor passes lead Homebrew to the user's
file? And does any variable that matters in practice become reachable this way
despite our environment: one that opens a browser, reaches the network,
auto-updates, or evaluates a formula from a path or URL? If it does, what is the
smallest change inside the process executor that would close it?

## Evidence

### E1 — What the executor sets (repository source, read 2026-10-02)

- `crates/adapters/src/homebrew/mod.rs` lines 121-128, `QUIETING_KEYS`:
  `HOMEBREW_NO_AUTO_UPDATE`, `HOMEBREW_NO_ANALYTICS`, `HOMEBREW_NO_ENV_HINTS`,
  `HOMEBREW_NO_INSTALL_CLEANUP`, `HOMEBREW_NO_COLOR`, `HOMEBREW_NO_EMOJI`, each
  set to `"1"` by `quiet_environment` (lines 137-147).
- `crates/adapters/src/process/environment.rs` lines 34-60,
  `sanitised_environment`: `PATH` = `<prefix>/bin` followed by `SYSTEM_PATH`
  (`/usr/bin:/bin:/usr/sbin:/sbin`, `crates/domain/src/snapshot.rs` line 141).
  `PINNED_ENVIRONMENT` is `LANG=C`, `LC_ALL=C`, `TERM=dumb`, `NO_COLOR=1`
  (`snapshot.rs` lines 146-151). `HOME` is set only when `spec.home` is
  `Some`. Nothing else is set, and in particular **not** `XDG_CONFIG_HOME`,
  `HOMEBREW_CACHE` or `USER`.
- The value of `HOME`: `crates/adapters/src/system/machine.rs` line 125 reads
  `std::env::var("HOME")`, which is passed through `usable_home` (lines
  136-139: trailing `/` trimmed, `/` rejected). `crates/cli/src/app/context.rs`
  line 157 hands it to the registry as `home: if replay { None } else {
  home.clone() }`. On a live run, then, `brew` receives **the user's real
  `HOME`**.
- The read argv the adapters use (`homebrew/mod.rs` lines 67-74, and
  `PERMITTED_SUBCOMMANDS` in `process/mod.rs` lines 103-128): `--version`,
  `--prefix`, `--cellar`, `--caskroom`, `--cache`, `config`, `info --json=v2
  --installed`, `list --pinned`, and the verification read `list --versions
  <names>`.

### E2 — Where `bin/brew` reads `brew.env`, and how (Homebrew 7.0.4 source on this machine, read 2026-10-02)

```
$ HOMEBREW_NO_AUTO_UPDATE=1 brew --version
Homebrew 7.0.4
$ HOMEBREW_NO_AUTO_UPDATE=1 brew --prefix
/usr/local
```

From `/usr/local/Homebrew/bin/brew`, quoted verbatim:

```
116 # These variables are exported in this file and are not allowed to be overridden by the user.
117 BIN_BREW_EXPORTED_VARS=(
118   HOMEBREW_BREW_FILE
119   HOMEBREW_PREFIX
120   HOMEBREW_REPOSITORY
121   HOMEBREW_LIBRARY
122   HOMEBREW_USER_CONFIG_HOME
123 )
...
128 export_homebrew_env_file() {
...
132   [[ -r "${env_file}" ]] || return 0
134   while read -r line
136     # only load variables defined in env_config.rb
137     [[ "${line}" =~ ^(HOMEBREW_|SUDO_ASKPASS=|(all|no|ftp|https?)_proxy=) ]] || continue
139     # forbid overriding variables that are set in this file
140     [[ "${line}" =~ ${BIN_BREW_EXPORTED_VARS_REGEX} ]] && continue
142     if [[ "${line}" == HOMEBREW_EXPERIMENTAL_RUST_FRONTEND=* ]]
...       echo "Warning: Ignoring HOMEBREW_EXPERIMENTAL_RUST_FRONTEND. This cannot be set in an env file." >&2
148     export "${line?}"
149   done <"${env_file}"
152 # First, load the system-wide configuration.
153 export_homebrew_env_file "/etc/homebrew/brew.env"
156 if [[ -n "${HOMEBREW_SYSTEM_ENV_TAKES_PRIORITY-}" ]]
158   SYSTEM_ENV_TAKES_PRIORITY="1"
161 # Next, load the prefix configuration
162 export_homebrew_env_file "${HOMEBREW_PREFIX}/etc/homebrew/brew.env"
165 if [[ -n "${XDG_CONFIG_HOME-}" ]]
167   HOMEBREW_USER_CONFIG_HOME="${XDG_CONFIG_HOME}/homebrew"
168 elif [[ -n "${HOMEBREW_XDG_CONFIG_HOME-}" ]]
170   HOMEBREW_USER_CONFIG_HOME="${HOMEBREW_XDG_CONFIG_HOME}/homebrew"
172   HOMEBREW_USER_CONFIG_HOME="${HOME}/.homebrew"
175 export_homebrew_env_file "${HOMEBREW_USER_CONFIG_HOME}/brew.env"
177 # If the system configuration takes priority, load it again to override any previous settings.
178 if [[ -n "${SYSTEM_ENV_TAKES_PRIORITY-}" ]]
180   export_homebrew_env_file "/etc/homebrew/brew.env"
```

Further down the same file:

```
202 # These variables are allowed to be set by the user as, e.g., `HOMEBREW_BROWSER`.
203 MANPAGE_VARS=( BROWSER BUNDLE_USER_CACHE DISPLAY EDITOR NO_COLOR )
...
213   [[ -z "${!VAR:-}" || "${!VAR:-}" = "0" ]] && continue
215   VAR_NEW="HOMEBREW_${VAR}"
217   [[ -n "${!VAR_NEW:-}" ]] && continue
218   export "${VAR_NEW}"="${!VAR}"
...
244   XDG_CACHE_HOME
245   XDG_CONFIG_HOME            (USED_BY_HOMEBREW_VARS: copied to HOMEBREW_XDG_CONFIG_HOME, line 258)
...
304 ENV_VAR_NAMES=(
306   HOME SHELL PATH TERM ... USER CI SSH_AUTH_SOCK SUDO_ASKPASS
307   http_proxy https_proxy ftp_proxy no_proxy all_proxy HTTPS_PROXY FTP_PROXY ALL_PROXY
310 for VAR in "${ENV_VAR_NAMES[@]}" "${!HOMEBREW_@}"
313   [[ -z "${!VAR:-}" ]] && continue
315   FILTERED_ENV+=("${VAR}=${!VAR}")
344 exec /usr/bin/env -i "${FILTERED_ENV[@]}" /bin/bash -p "${HOMEBREW_LIBRARY}/Homebrew/brew.sh" "$@"
```

No line of `bin/brew` consults a variable that disables the loading of
environment files. The only condition `export_homebrew_env_file` tests is
readability (line 132).

### E3 — The official documentation

- `docs/Manpage.md` in the Homebrew/brew repository at 7.0.4
  (`/usr/local/Homebrew/docs/Manpage.md` lines 4358-4378, read 2026-10-02;
  this file is the source of <https://docs.brew.sh/Manpage>):
  > `HOMEBREW_*` environment variables can also be set in Homebrew's environment files:
  > * `/etc/homebrew/brew.env` (system-wide)
  > * `${HOMEBREW_PREFIX}/etc/homebrew/brew.env` (prefix-specific)
  > * `$XDG_CONFIG_HOME/homebrew/brew.env` if `$XDG_CONFIG_HOME` is set or `~/.homebrew/brew.env` otherwise (user-specific)
  >
  > User-specific environment files take precedence over prefix-specific files and prefix-specific files take precedence over system-wide files (unless `$HOMEBREW_SYSTEM_ENV_TAKES_PRIORITY` is set, see below).
  >
  > Note that these files do not support shell variable expansion (e.g. `$HOME`) or command execution (e.g. `$(cat file)`).

  The same file, lines 5049-5053, under `HOMEBREW_SYSTEM_ENV_TAKES_PRIORITY`:
  "If set in Homebrew's system-wide environment file (`/etc/homebrew/brew.env`),
  the system-wide environment file will be loaded last to override any prefix
  or user settings." Lines 4757-4760, under `HOMEBREW_FORCE_API_AUTO_UPDATE`:
  "If set, update the Homebrew API formula or cask data even if
  `$HOMEBREW_NO_AUTO_UPDATE` is set." The Manpage says nothing about how a
  file value relates to a variable already set in the process environment.
  The live page <https://docs.brew.sh/Manpage> was fetched on 2026-10-02, but
  the tool truncated it before the ENVIRONMENT section, so the repository copy
  above is the one cited.
- <https://docs.brew.sh/Homebrew-for-Mac-Admins>, read 2026-10-02:
  "Homebrew reads environment files in this order: System
  `/etc/homebrew/brew.env`, Prefix `/opt/homebrew/etc/homebrew/brew.env`, User
  `~/.homebrew/brew.env`". "Later files override earlier files. Set
  `HOMEBREW_SYSTEM_ENV_TAKES_PRIORITY=1` in `/etc/homebrew/brew.env` to load that
  file again last, overriding conflicting prefix and user settings." "Write
  literal `NAME=value` lines, without `export` or surrounding shell quotes."
- The pull request that introduced the files is a lead, not a source:
  <https://github.com/Homebrew/brew/pull/15787>, merged 2023-08-01, read
  2026-10-02. Its description says the lines are filtered "to ensure we're only
  permitting setting `HOMEBREW_*` variables". It says nothing about precedence
  over the process environment.

### E4 — How Homebrew interprets the keys (Homebrew 7.0.4 source, read 2026-10-02)

- `Library/Homebrew/env_config.rb`: `HOMEBREW_NO_AUTO_UPDATE` (line 545),
  `HOMEBREW_NO_ANALYTICS` (531), `HOMEBREW_NO_ENV_HINTS` (570),
  `HOMEBREW_NO_INSTALL_CLEANUP` (605), `HOMEBREW_NO_COLOR` (561),
  `HOMEBREW_NO_EMOJI` (566) and `HOMEBREW_DEVELOPER` (302) are all `boolean:
  :set`. Line 920 makes such a key true whenever its value is
  `present?`, and `FALSY_VALUES = %w[false no off nil 0]` (line 865)
  applies only to `boolean: true` keys.
  `HOMEBREW_FORCE_API_AUTO_UPDATE` (414) is `boolean: true`.
  `HOMEBREW_FORBID_PACKAGES_FROM_PATHS` (406-412) defaults to
  `ENV["HOMEBREW_DEVELOPER"].blank?`. `HOMEBREW_NO_REQUIRE_TAP_TRUST` (632-638)
  is `boolean: :set, odeprecated: true`.
- `Library/Homebrew/utils/auto-update.sh` line 21:
  `[[ -z "${HOMEBREW_NO_AUTO_UPDATE}" ]] || return`. Lines 144-155: `brew update
  --auto-update` is armed only when `HOMEBREW_COMMAND` is one of `install
  outdated upgrade bundle release`, or `tap` with arguments. Lines 158-180 add
  `bump`, `bump-formula-pr`, `bump-cask-pr` and `bump-unversioned-casks`.
- `Library/Homebrew/brew.rb` lines 138-141: every internal Ruby command runs
  `Homebrew::API.fetch_api_files! if install_from_api`, where
  `install_from_api = !Homebrew::EnvConfig.no_install_from_api?`.
- `Library/Homebrew/api.rb` lines 189-217 (`fetch_api_files!`): `stale_seconds`
  is `nil` when `no_auto_update? && !force_api_auto_update?`. Otherwise it is
  `api_auto_update_secs` for an auto-update command, and
  `DEFAULT_API_STALE_SECONDS` (line 27, 7 days) for any other command. The
  download heading is `"Downloading Homebrew API data"`. Lines 33-39
  (`skip_download?`):
  ```
  return true if Homebrew.running_as_root_but_not_owned_by_root?
  return false if !target.exist? || target.empty?
  return true unless stale_seconds
  (Time.now - stale_seconds) < target.mtime
  ```
- `Library/Homebrew/utils/os.sh` line 55: on macOS,
  `HOMEBREW_DEFAULT_CACHE="${HOME}/Library/Caches/Homebrew"`. It is not
  derived from any `XDG_*` variable. `brew.sh` line 51:
  `HOMEBREW_CACHE="${HOMEBREW_CACHE:-${HOMEBREW_DEFAULT_CACHE}}"`.
- `Library/Homebrew/utils/analytics.rb` lines 108-109 and 187-191:
  `report_command_run` returns early when `no_analytics?` is true *or*
  `config_true?(:analyticsdisabled)`.
  `git -C /usr/local/Homebrew config --get homebrew.analyticsdisabled` →
  exit 1 (unset) on this machine.
- `Library/Homebrew/cmd/info.rb` lines 134-137: `Utils::Browser.open` is
  reached only under `args.github?`. Grepping for `Utils::Browser` /
  `exec_browser` finds callers only in `cmd/source.rb`, `info.rb`,
  `search.rb`, `docs.rb`, `home.rb`, `deps.rb` and developer commands.
- `Library/Homebrew/trust.rb` lines 27-43: the trust store lives at
  `$HOME/.homebrew/trust.json` when `HOMEBREW_USER_CONFIG_HOME` is
  `<user home>/.homebrew`, and at `$HOMEBREW_USER_CONFIG_HOME/trust.json`
  otherwise. `formulary.rb` line 127: every formula load calls
  `Homebrew::Trust.require_trusted_formula!`.

### E5 — The files on this machine (2026-10-02)

```
$ ls -la /usr/local/etc/homebrew/ /etc/homebrew/ ~/.homebrew/ ~/.config/homebrew
ls: /Users/pirado/.config/homebrew: No such file or directory
ls: /etc/homebrew/: No such file or directory
ls: /usr/local/etc/homebrew/: No such file or directory
/Users/pirado/.homebrew/:
-rw-------@ 1 pirado  staff     0 Sep  2 09:20 trust.json.lock
$ ls -ld /usr/local/etc /usr/local /etc
lrwxr-xr-x@  1 root    wheel   11 Sep 24 23:06 /etc -> private/etc
drwxr-xr-x  16 root    wheel  512 Sep 30 09:23 /usr/local
drwxrwxr-x@ 27 pirado  admin  864 Sep  1 20:19 /usr/local/etc
```

None of the three files exists here. The user owns and can write the
prefix's `etc`.

### E6 — Observed against a temporary HOME (2026-10-02, 23:08 -03)

Every run used the environment shape `quiet_environment` builds, with `HOME`
pointed at a throw-away directory. `$T` is
`/private/tmp/claude-501/-Users-pirado-projects-MacPortsBrew/d7cdfceb-8261-4040-b3b7-0317e77372cc/scratchpad/brewenv.HH8i`,
and `$H = $T/home`. The only files written were under `$T`. The only
subcommands run were `--cache`, `--prefix`, `config` and `list`.

```
/usr/bin/env -i PATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin LANG=C LC_ALL=C TERM=dumb NO_COLOR=1 \
  HOME="$H" HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_COLOR=1 HOMEBREW_NO_EMOJI=1 \
  HOMEBREW_NO_ENV_HINTS=1 HOMEBREW_NO_INSTALL_CLEANUP=1 [extra] /usr/local/bin/brew <argv>
```

`$H/Library/Caches/Homebrew/api` was seeded with a fresh copy (`cp -R`) of
the API data, so the Ruby reads had a present, non-empty cache. The mtime of
`packages.tahoe.jws.json`, the tripwire, read `23:08:32` before the first run
and after the last one.

| # | `$H/.homebrew/brew.env` contents | extra | argv | output (trimmed) |
|---|---|---|---|---|
| a | *(none)* | | `config` | `HOMEBREW_NO_ANALYTICS: set` / `NO_AUTO_UPDATE: set` / `NO_COLOR: set` / `NO_EMOJI: set` / `NO_ENV_HINTS: set` / `NO_INSTALL_CLEANUP: set` |
| b | `HOMEBREW_CACHE=$T/fromfile` | | `--cache` | `$T/fromfile` |
| c | `HOMEBREW_CACHE=$T/fromfile` | `HOMEBREW_CACHE=$T/fromenv` | `--cache` | `$T/fromfile` |
| d | `HOMEBREW_NO_AUTO_UPDATE=` `HOMEBREW_NO_ENV_HINTS=` `HOMEBREW_NO_ANALYTICS=` | | `config` | only `NO_COLOR: set`, `NO_EMOJI: set`, `NO_INSTALL_CLEANUP: set` remain |
| e | `HOMEBREW_NO_AUTO_UPDATE=0` `HOMEBREW_NO_ANALYTICS=false` | | `config` | all six still `set` |
| f | `HOMEBREW_NO_COLOR=` `HOMEBREW_NO_EMOJI=` `HOMEBREW_NO_INSTALL_CLEANUP=` | | `config` | `NO_ANALYTICS`, `NO_AUTO_UPDATE`, `NO_COLOR`, `NO_ENV_HINTS` `set`; `NO_EMOJI` and `NO_INSTALL_CLEANUP` gone |
| g | `HOMEBREW_FORCE_API_AUTO_UPDATE=1` `HOMEBREW_DEVELOPER=1` `HOMEBREW_API_DOMAIN=https://example.invalid/api` `HOMEBREW_BROWSER=/usr/bin/false` `HOMEBREW_CURLRC=/dev/null` `SUDO_ASKPASS=/usr/bin/false` `export HOMEBREW_NO_INSTALL_FROM_API=1` `HOMEBREW_PREFIX=/tmp/nope` `PATH=/tmp/nope` `HOMEBREW_EXPERIMENTAL_RUST_FRONTEND=1` | | `config` | stderr `Warning: Ignoring HOMEBREW_EXPERIMENTAL_RUST_FRONTEND. This cannot be set in an env file.`; `HOMEBREW_PREFIX: /usr/local`, `HOMEBREW_API_DOMAIN: https://example.invalid/api`, `HOMEBREW_BROWSER: /usr/bin/false`, `HOMEBREW_CURLRC: /dev/null`, `HOMEBREW_DEVELOPER: set`, `HOMEBREW_FORCE_API_AUTO_UPDATE: set`, our six `set`, `SUDO_ASKPASS: /usr/bin/false`; no `NO_INSTALL_FROM_API`; exit 0 |
| h | same as g | | `--prefix` | the same warning on stderr; `/usr/local`; exit 0 |
| i | `HOMEBREW_NO_REQUIRE_TAP_TRUST=1` | | `config`, then `list --versions` | `Warning: Calling HOMEBREW_NO_REQUIRE_TAP_TRUST is deprecated! ...`; `HOMEBREW_NO_REQUIRE_TAP_TRUST: set`; exit 0; `list` printed `actionlint 1.7.12` ... |
| j | `HOMEBREW_NO_INSTALL_FROM_API=1` | | `config` | `HOMEBREW_NO_INSTALL_FROM_API: set` |
| k | `HOMEBREW_CACHE="/tmp/q"` | | `--cache` | `"/tmp/q"` (the quotes are kept) |
| l | `HOMEBREW_DEVELOPER=1` `HOMEBREW_NO_REQUIRE_TAP_TRUST=1` | | `config` / `list --pinned` / `--prefix` | exit 1 / exit 1 / exit 0; stderr `Error: Calling HOMEBREW_NO_REQUIRE_TAP_TRUST is deprecated! ... This error was expected, as this is not a Tier 1 configuration` |

**Run d probably sent a Homebrew analytics event** (added after review,
2026-10-02). It blanked `HOMEBREW_NO_ANALYTICS` and ran `brew config`, a Ruby
command; `brew.rb` line 146 calls `report_command_run` for every Ruby command,
gated only by the variable and by `homebrew.analyticsdisabled` in the
repository's git configuration, which is unset on this machine
(`git config` exits 1), and the report goes out through a detached `curl`. A
temporary `HOME` changes none of that. No traffic was captured, so "sent" is
inferred from source, not observed; either way the run should not have
blanked the key against the real repository.

User-file location (the `HOME` file holds `HOMEBREW_CACHE=$T/fromhome`;
`$T/xdg/homebrew/brew.env` holds `HOMEBREW_CACHE=$T/fromxdg`; `$T/emptyxdg`
is an empty directory; argv `--cache`):

| extra | output |
|---|---|
| *(none)* | `$T/fromhome` |
| `XDG_CONFIG_HOME=$T/xdg` | `$T/fromxdg` |
| `XDG_CONFIG_HOME=$T/emptyxdg` | `$H/Library/Caches/Homebrew` (the default: no user file read) |
| `XDG_CONFIG_HOME=` (empty) | `$T/fromhome` |

### E7 — A breach of the research protocol: a write with the real HOME (2026-10-02, 23:06)

Before the temporary-HOME runs, this command was executed once, with the
user's real `HOME`, to see what `config` prints:

```
$ du -sh ~/Library/Caches/Homebrew/api
du: /Users/pirado/Library/Caches/Homebrew/api: No such file or directory
$ env -i PATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin LANG=C LC_ALL=C TERM=dumb NO_COLOR=1 HOME="$HOME" \
    HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1 HOMEBREW_NO_INSTALL_CLEANUP=1 \
    HOMEBREW_NO_COLOR=1 HOMEBREW_NO_EMOJI=1 /usr/local/bin/brew config
$ find ~/Library/Caches/Homebrew -maxdepth 3 -exec ls -ldT {} +
drwxr-xr-x  ... Oct  2 23:06:22 2026 /Users/pirado/Library/Caches/Homebrew
-rw-r--r--  ... 15473642 Oct  2 23:06:24 2026 .../api/internal/packages.tahoe.jws.json
-rw-r--r--  ... 14219630 Oct  2 23:06:24 2026 .../api/internal/packages.tahoe.jws.json.payload
-rw-r--r--  ...   453175 Oct  2 23:06:24 2026 .../api/internal/packages.tahoe.jws.json.payload.index
drwxr-xr-x  ... Oct  2 23:06:17 2026 .../bootsnap
$ du -sh ~/Library/Caches/Homebrew
 35M	/Users/pirado/Library/Caches/Homebrew
```

With all six quieting variables set, one `brew config` downloaded about 15 MB
of API data and wrote about 30 MB into the user's cache (the `.jws.json`, its
`.payload` and `.payload.index`), which had held no API data before the run.
**This broke the work order's protocol — reading is allowed, writing is a
failure — and is recorded as that, not as a side effect.** What existed
before is known only for `api/`: the pre-check looked there alone, so whether
`~/Library/Caches/Homebrew` itself or `bootsnap/` (mtime 23:06:17, inside the
run) predated the run is not established. The files were left in place for
the owner to decide on. It is the same mechanism the pass-05 report recorded
for `brew list --versions` on 2026-09-30, and it is the finding that
`HOMEBREW_NO_AUTO_UPDATE` does not keep a read offline on a cold cache.

## Findings

1. **Locations and order.** `bin/brew` reads up to four files in this order
   (E2, lines 153-181; documented in E3):
   1. `/etc/homebrew/brew.env`;
   2. `${HOMEBREW_PREFIX}/etc/homebrew/brew.env` (`/usr/local/etc/homebrew/brew.env`
      here);
   3. the user file: `$XDG_CONFIG_HOME/homebrew/brew.env` if
      `XDG_CONFIG_HOME` is non-empty, else `$HOMEBREW_XDG_CONFIG_HOME/homebrew/brew.env`
      if that is non-empty, else `$HOME/.homebrew/brew.env`. The middle
      branch is undocumented (E2 line 168), and an earlier file can set
      `HOMEBREW_XDG_CONFIG_HOME`, which moves the user file;
   4. `/etc/homebrew/brew.env` again, if `HOMEBREW_SYSTEM_ENV_TAKES_PRIORITY` is
      non-empty after step 1. Step 1's file can set it, and so can the
      process environment (E2 line 156 tests the variable, not where it came
      from).

   A file is skipped only when it is unreadable (E2 line 132). **The executor
   passes no `XDG_CONFIG_HOME` and no `HOMEBREW_XDG_CONFIG_HOME`, and it
   passes the user's real `HOME` (E1). On a live run, then, Homebrew reads
   `~/.homebrew/brew.env`, the prefix file and the system file.** The
   empty-string case falls back to `HOME` (E6, last table).

2. **Precedence: a file overrides the environment, with no exception available
   to us.** Each accepted line is passed to `export "${line?}"` with no
   is-it-already-set test (E2 line 148). Observed: a file value replaced a
   value set in the environment (E6 c). The only keys a file cannot set are
   `HOMEBREW_BREW_FILE`, `HOMEBREW_PREFIX`, `HOMEBREW_REPOSITORY`,
   `HOMEBREW_LIBRARY` and `HOMEBREW_USER_CONFIG_HOME` (E2 lines 117-123,
   observed in E6 g), plus `HOMEBREW_EXPERIMENTAL_RUST_FRONTEND` (line 142).
   A line must *begin* with `HOMEBREW_`, `SUDO_ASKPASS=` or a lowercase
   `*_proxy=`, so `export X=…` and `PATH=…` lines are ignored (E6 g). Quotes
   are kept literally (E6 k). The documentation describes precedence between
   files only. It does not say that a file beats the process environment
   (E3), so this finding rests on the source and the observation. Neither the
   source nor the Manpage has any variable that turns file loading off.
   `HOMEBREW_NO_ENV_FILTERING` does not appear in `bin/brew` or
   `env_config.rb`. `HOMEBREW_FORCE_BREW_WRAPPER` is "No longer used"
   (`env_config.rb` lines 433-437) and plays no part in file loading.

3. **Empty is unset; `0` is still set.** A line `HOMEBREW_NO_AUTO_UPDATE=`
   exports an empty value, and `bin/brew` line 313 drops empty values before
   it re-executes under `env -i`. The key is therefore **absent** in
   `brew.sh` and Ruby, and a file can delete `HOMEBREW_NO_AUTO_UPDATE`,
   `HOMEBREW_NO_ANALYTICS`, `HOMEBREW_NO_ENV_HINTS`, `HOMEBREW_NO_EMOJI` and
   `HOMEBREW_NO_INSTALL_CLEANUP` (E6 d, f). `HOMEBREW_NO_COLOR` survives,
   because `bin/brew` lines 203-218 re-derive it from our pinned `NO_COLOR=1`
   (E6 f), and `TERM=dumb` is not a `HOMEBREW_*` key. A value of `0` or
   `false` does **not** turn these keys off. All six are `boolean: :set`,
   and the shell gate is `-z` (E4, observed in E6 e).

4. **What becomes reachable, key by key, for the reads the adapters run.**
   - **Repository auto-update (`git` fetch, writes under the repository): not
     reachable from a read.** `brew update --auto-update` is armed only for
     `install`, `outdated`, `upgrade`, `bundle`, `release`, `tap <arg>` and
     `bump*` (E4). No subcommand in E1's list is among them, so blanking
     `HOMEBREW_NO_AUTO_UPDATE` or setting `HOMEBREW_AUTO_UPDATE_SECS` changes
     nothing for a read. **It is reachable for a mutating pass's `install` or
     `upgrade`.** A `brew.env` that blanks `HOMEBREW_NO_AUTO_UPDATE` turns
     these into "update first" (E4, `auto-update.sh` line 21), and
     `HOMEBREW_NO_INSTALL_FROM_API` shortens the interval to 300 s.
   - **API download (network, writes to the cache): reachable from every
     Ruby read.** `config`, `info` and `list` all call `fetch_api_files!`
     (E4, `brew.rb` line 141). A file can reach the network in four ways:
     blank `HOMEBREW_NO_AUTO_UPDATE` (stale after 7 days); set
     `HOMEBREW_FORCE_API_AUTO_UPDATE=1`, which is documented to override our
     variable (E3) and was observed set alongside it (E6 g); point
     `HOMEBREW_CACHE` at an empty directory, which downloads on every run
     because `skip_download?` returns `false` for a missing target *before* it
     looks at `stale_seconds` (E4); or point `HOMEBREW_API_DOMAIN` elsewhere
     (E6 g). **Independently of any `brew.env`, `HOMEBREW_NO_AUTO_UPDATE` does
     not keep a Ruby read offline when the API cache is absent or empty**: the
     same line in E4, observed in E7. R-1 finding 2 holds for a present cache
     only.
   - **What a network fetch runs under:** `HOMEBREW_CURLRC` (an arbitrary curl
     configuration file), the `*_proxy` variables and `HOMEBREW_API_DOMAIN` are
     all settable from a file and are not set by us (E2 line 137, E6 g).
     Whether a substituted API document would be accepted is **not
     established** here; see "Cannot determine".
   - **Analytics: reachable when the repository's own setting does not
     disable it.** Blanking `HOMEBREW_NO_ANALYTICS` leaves
     `report_command_run` gated only by `homebrew.analyticsdisabled` in the
     repository's git configuration (E4), which is unset on this machine. The
     answer depends on the machine.
   - **Opening a browser: not reachable.** `info` opens one only under
     `--github` (E4), and no read argv carries it. `HOMEBREW_BROWSER` chooses
     the browser and does not cause one to open.
   - **Evaluating a formula from a path or URL: reachable only through argv,
     which SE-050 already closes.** `HOMEBREW_DEVELOPER=1` from a file is
     honoured (E6 g), and it flips `HOMEBREW_FORBID_PACKAGES_FROM_PATHS` (E4).
     This confirms SE-050's reason for refusing `/` and `:` in Homebrew argv.
     It is not a new opening.
   - **Evaluating untrusted third-party tap code:** `HOMEBREW_NO_REQUIRE_TAP_TRUST=1`
     from a file is honoured, with a deprecation warning (E6 i). It switches
     off `require_trusted_formula!` (E4). `info --json=v2 --installed` loads
     every installed formula, so Ruby from untrusted taps becomes reachable
     from a read. Tap Ruby evaluated under the user's account is code
     execution, not a configuration change.
   - **Denial and output noise:** `HOMEBREW_DEVELOPER=1` together with a
     deprecated key makes every Ruby read exit 1 (E6 l). Any file key adds
     stderr lines (E6 g, h, i), and blanking `HOMEBREW_NO_ENV_HINTS` restores
     hints. Stdout parsing was unaffected in the cases observed (E6 h: `--prefix`
     printed `/usr/local`).
   - `SUDO_ASKPASS` reaches `brew.sh` (E6 g). The only `sudo` in a read is
     `/usr/bin/sudo --reset-timestamp` (`brew.sh` lines 640-653), which does
     not prompt, so `SUDO_ASKPASS` is not exercised by a read.

5. **Who can write the files.** On this machine the user owns
   `/usr/local/etc` (E5), so any process running as the user can create the
   prefix file. Nothing an environment can say suppresses that file or the
   system file. No mitigation applied inside the process executor can
   therefore close the prefix and system files. Only the user file is within
   its reach.

6. **Mitigation options inside the process executor, with their costs.**
   - **(a) Set `XDG_CONFIG_HOME` to a directory MacPortsBrew controls and
     keeps empty.** Observed to suppress the user file (E6, `emptyxdg`), and
     it leaves `HOME` alone, so the default cache stays at
     `~/Library/Caches/Homebrew` (E4, `os.sh` line 55). Costs, from source:
     it moves `HOMEBREW_USER_CONFIG_HOME` and with it the **trust store**
     (`trust.rb` lines 37-41), so taps the user has trusted explicitly look
     untrusted to our reads. It also arrives in Ruby as
     `HOMEBREW_XDG_CONFIG_HOME` (E2 line 258). It does nothing about the
     prefix and system files. How an untrusted-tap formula behaves under
     `info --installed` was not observed (there are no third-party taps on
     this machine).
   - **(b) Set `HOME` to a controlled empty directory.** Also suppresses the
     user file, but it moves the default cache. Every Ruby read would then
     download the API (E4 `skip_download?`, E7) unless `HOMEBREW_CACHE` is
     also set. That breaks the offline-metadata probe's meaning, and it moves
     the trust store too. It is strictly worse than (a).
   - **(c) Set a key "more strongly".** This is not available. A file is
     applied after the environment and overwrites it (finding 2).
   - **(d) Detect rather than prevent, which is runtime capability detection.**
     Before invoking `brew`, read the candidate files without writing:
     `/etc/homebrew/brew.env`, `<detected prefix>/etc/homebrew/brew.env`, and
     the user file resolved as `bin/brew` resolves it. The same order and the
     same line regex (E2 line 137) apply. Report or refuse when a line would
     blank or set a key of concern. At minimum that means any of our
     `QUIETING_KEYS` with an empty value, plus `HOMEBREW_FORCE_API_AUTO_UPDATE`,
     `HOMEBREW_CACHE`, `HOMEBREW_API_DOMAIN`, `HOMEBREW_ARTIFACT_DOMAIN`,
     `HOMEBREW_CURLRC`, `HOMEBREW_NO_REQUIRE_TAP_TRUST`, `HOMEBREW_DEVELOPER`,
     `HOMEBREW_XDG_CONFIG_HOME`, `HOMEBREW_SYSTEM_ENV_TAKES_PRIORITY`,
     `HOMEBREW_NO_INSTALL_FROM_API`, `SUDO_ASKPASS` and the proxies. This is
     the only option that covers all three files. It has an inherent
     check-then-use window, and the key list needs re-reading at each Homebrew
     version.
   - **(e) After-the-fact confirmation is not cheap.** `brew config` reports
     the effective keys (E6), but it is a Ruby read and has already run
     `fetch_api_files!` by the time it prints. The bash fast paths (`--prefix`,
     `--cache`, ...) print no quieting state. `--cache` does reveal an
     overridden `HOMEBREW_CACHE` (E6 b).

   The evidence supports (a) together with (d): (a) removes the
   user-controlled file at the cost of the trust-store relocation, and (d) is
   the only coverage for the prefix and system files. The choice belongs to
   the main agent.

## Cannot determine

- **Whether a substituted `HOMEBREW_API_DOMAIN` or a hostile `HOMEBREW_CURLRC`
  can make Homebrew accept altered API data.** The cache file is a JWS
  (`packages.tahoe.jws.json`), but the verification path was not read in this
  note. Settling it means reading `Library/Homebrew/api/internal.rb` and the
  JWS verification it calls, at the version in use. **UNRESOLVED.**
- **How an installed formula from an untrusted third-party tap behaves under
  `info --json=v2 --installed`** when the trust store is relocated (option a).
  This machine has only `homebrew/core` under `Library/Taps`. It needs a
  machine with an explicitly trusted third-party tap and a temporary-HOME
  reproduction.
- **The prefix and system files were not exercised empirically.** Writing to
  `/usr/local/etc/homebrew/` or `/etc/homebrew/` is outside what this note may
  do. Their behaviour is established from source (E2) and documentation (E3)
  only.
- **The trigger for analytics on a machine where
  `homebrew.analyticsdisabled` is unset** was established from source, and no
  network traffic was observed — including for run d of E6, which probably
  sent an event (see the note under E6's table).
- **What a Ruby read does on a cold cache with no network at all** — fails,
  waits, or answers from nothing. Not tested; it bears on
  `06-cli-and-ux.md`'s "`analyze` works with no network".

## Impact on the specification

- **Confirms** SE-050's reason for refusing `/` and `:` in Homebrew argv: a
  `brew.env` can set `HOMEBREW_DEVELOPER`, and doing so flips
  `HOMEBREW_FORBID_PACKAGES_FROM_PATHS` (finding 4). SE-050 cites `bin/brew`
  lines 128-180; at 7.0.4 the loader runs from 128 to 181.
- **Contradicts, in part,** `docs/spec/04-migration-and-safety.md`
  ("`HOMEBREW_NO_AUTO_UPDATE` is the one that does the offline work") and the
  doc comment on `quiet_environment` (`crates/adapters/src/homebrew/mod.rs`
  lines 132-133, "it is what makes a read offline and side-effect-free"). The
  claim fails in two ways. First, `HOMEBREW_NO_AUTO_UPDATE` does not prevent
  the first API download when the cache is absent or empty (E4
  `skip_download?`, E7). Second, a `brew.env` can delete the variable or
  outrank it (findings 2-4). R-1 finding 2 is correct only for a present
  cache.
- **Contradicts** the premise in INV-10's implementation that the environment
  `sanitised_environment` builds is the environment Homebrew runs under. For
  every `HOMEBREW_*` key, the effective environment is "ours, overridden by up
  to three files". A snapshot's recorded environment (SE-051) therefore does
  not describe what `brew` actually ran with if a `brew.env` was present.
- **For the mutating pass:** `install` and `upgrade` are auto-update commands
  (E4). A user `brew.env` that blanks `HOMEBREW_NO_AUTO_UPDATE` makes our
  `install` run `brew update` first. The mutating pass cannot rely on the
  quieting environment without option (a), (d) or both.
- **Possible tension with `docs/passes/02-ARCHITECTURE-REVIEW-findings.md`
  F-21**, which says "`HOMEBREW_NO_AUTO_UPDATE=0` turned a read-only command
  into one that writes". At 7.0.4, `=0` keeps auto-update off in both the
  shell gate and Ruby (finding 3, E6 e). That finding was reproduced at the
  time, possibly through a different mechanism or version. It is worth
  re-checking rather than citing.
- `docs/spec/09-research-checklist.md` has no R-15 entry. The main agent
  decides whether to add one.

## Re-check when

- Homebrew moves past 7.0.4, and especially when `bin/brew`'s
  `export_homebrew_env_file`, `BIN_BREW_EXPORTED_VARS`, `api.rb`
  `fetch_api_files!`/`skip_download?`, `utils/auto-update.sh` or `trust.rb`
  `trust_file` changes. A web search on 2026-10-02 also turned up Homebrew
  issue #22196, "Add an alternative to brew.env files for passing in
  additional env variables through to brew's filtered environment". Its
  status was not checked; it is a lead about a possible change to the
  mechanism.
- Any time the executor's Homebrew environment changes (`QUIETING_KEYS`,
  `PINNED_ENVIRONMENT`, how `HOME` is chosen, or the addition of
  `XDG_CONFIG_HOME`).
- `HOMEBREW_NO_REQUIRE_TAP_TRUST` is marked deprecated, so its removal changes
  finding 4's tap-trust bullet.
