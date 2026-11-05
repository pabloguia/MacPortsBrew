# R-28 — Is a version flag safe to run, and what do `LIVENESS_PROBE`'s environment, timeout and curated data have to be?

- **Status:** partially answered (questions 1-5 answered from observation and citation; `gpg`, `rg`, the unset-`HOME` case, stdin reads, network use and arm64 are UNRESOLVED, see Cannot determine)
- **Answered on:** 2026-11-04
- **Machine:** macOS 26.7.2 (25G332), x86_64, Homebrew 7.0.11-143-g874c46a @ `/usr/local` (`/usr/local/Homebrew` HEAD `874c46a98f1832f5c872c94f7d5a33503022c154`; homebrew-core `e045d7385fd`, last commit 2026-09-02), MacPorts 2.12.6 @ `/opt/local`; `xcode-select -p` = `/Applications/Xcode.app/Contents/Developer`, `/Library/Developer/CommandLineTools` present
- **Blocking:** work order 16 part H items 1-2 (`docs/passes/16-liveness-design.md`, the curated data file, the unchecked outcome); DP-051's ruling; 04's verification contract check 4

## Side effects of this research on the machine (read this first)

The sweep was within the permitted form (a version flag, scratch `HOME`/`TMPDIR`/cwd, null stdin), and it still changed this machine. These are recorded because they are the finding (E-3, E-4), and so that the owner can act on what is left:

1. **`httpd --version` started a real Apache server** from `/usr/local/opt/httpd/bin/httpd`, listening on `*:8080`, from 02:57:40 to 03:00:10 on 2026-11-04 (2 min 30 s). I stopped that one process with `kill -TERM 49260`, after checking its command line and that it was not in the process list taken before the run. That `kill` is the one action in this note outside "read-only".
2. **Left in the Homebrew prefix, not removed:** `/usr/local/var/log/httpd/error_log` (3 lines, this session's only) and `/usr/local/var/log/httpd/access_log` (0 bytes). `httpd` removed its own `httpd.pid` on shutdown. They are left because the project does not write into a provider's prefix; deleting them is the owner's call.
3. **Left in the real home, not removed:** 11 crash reports in `~/Library/Logs/DiagnosticReports/` (`emacs-31.1-2026-11-04-*.ips`, `semgrep-core-2026-11-04-*.ips`), written by the OS for the crashing runs in E-3.
4. **Network, inferred:** in each of its 4 runs, starting from an empty scratch, `mise --version` reported `mise version 2026.11.0 available`, a version not on this machine. Nothing here captured the traffic itself (E-3).
5. **Unattributed:** a zero-length `/private/tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress` was created at 03:02:51, after the last sweep program ran (see Cannot determine).

## Question

DP-051's ruling: `LIVENESS_PROBE` runs the destination's resolved executable only with a curated probe from a new dated data file (`source`, `read_on`). The entry holds argv only, never a program path. It runs in a sanitised environment (INV-10), with no stdin, a scratch working directory and ADR-0006's timeout; only the exit status counts. A package with no curated probe is reported unchecked. 04 says "default `--version`", and the ruling removes that default.

1. Is a version flag actually safe, and what can it do?
2. What must the environment be (variables, `HOME`, `TMPDIR`, `DYLD_*`, `PATH`, a script-based executable behind a shim, INV-10)?
3. What timeout, what does ADR-0006's one-process kill leave when a probe forks, and where does output go?
4. What makes a good curated entry, what can `source` honestly be, what is the file's shape, and what is the initial table (at most 15)?
5. What does a probe not prove?

## Evidence

Machine commands were run on 2026-11-04 as uid 501, unelevated. Every `brew` call had `HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_FROM_API=1 HOMEBREW_NO_ENV_HINTS=1` exported; `port` was only called with `-q` and read verbs. No probe invoked `brew` or `port`. Official pages were read on 2026-11-04 through the fetch tool, which returns quotes from the page rather than the raw page.

### E-1 · What the code does today (branch `wo16/H-1-checks`, `9b8d68d`)

- `crates/adapters/src/process/system.rs`, `SystemRunner::run`: `.env_clear().envs(&spec.environment).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())`; `command.current_dir("/")` with the comment "There is deliberately no per-spec override - `CommandSpec` has no directory field"; `command.process_group(0)`; on timeout `child.kill()`, one process.
- Same file: `READER_DRAIN_TIMEOUT` = 6 s per stream, so 12 s in the worst case. `read_bounded` keeps draining after the cap, "Keep draining so the child does not block on a full pipe, but discard the excess". `collect` returns `truncated = true` when a reader did not finish. The test `a_grandchild_holding_the_pipe_does_not_hang_the_runner` asserts exit 0, `truncated`, and `require_success` erroring.
- `crates/adapters/src/process/mod.rs`: `DEFAULT_TIMEOUT` = 120 s, `DEFAULT_MAX_OUTPUT_BYTES` = 64 MiB; both are per-spec fields, and `with_timeout` exists.
- `crates/adapters/src/process/environment.rs` and `crates/domain/src/snapshot.rs`: `SYSTEM_PATH` = `/usr/bin`, `/bin`, `/usr/sbin`, `/sbin`; `PINNED_ENVIRONMENT` = `LANG=C`, `LC_ALL=C`, `TERM=dumb`, `NO_COLOR=1`; `EnvironmentSpec { leading_path, home, extra }`; `is_build_flag` drops `CPATH`, `LIBRARY_PATH`, the other build flags and `key.starts_with("DYLD_")`.
- `crates/application/src/planner.rs` ≈974: `VerificationCheck::LivenessProbe { executable: e.to_string(), flag: "--version".into() }`. `crates/domain/src/plan.rs` ≈355: field `flag`, doc comment "The non-mutating flag (`--version` by default)".
- `docs/adr/0006-…md`: "this build performs only reads, so nothing it spawns is expected to fork". The ADR holds the kill and drain mechanism; the timeout's value is only in `mod.rs`.
- Every program in all 7 `fixtures/recordings/*.json` is one of `brew`, `port`, `sw_vers`, `sysctl`, `uname`, `xcode-select`. No recording holds a probe, or an executable-list read (`port -q contents`, `brew list --formula`).

### E-2 · The sweep

Inventory and selection:

```
$ brew list --formula | wc -l   → 194        $ brew list --cask | wc -l → 9
$ port -q installed | wc -l     → 18
$ which gpg gpg2 php            → gpg not found / gpg2 not found / php not found
$ ls -l /opt/local/bin/rg /usr/local/bin/rg  → No such file or directory (both)
```

60 formulae have `<prefix>/opt/<name>/bin/<name>`. 10 were left out to stay under 60, because a runtime class like them was already sampled: `actionlint brotli git-lfs gitleaks jasper lz4 mpg123 osv-scanner swiftformat x265`. Sampled: 50 formulae and 8 ports (`autoconf automake gettext gperf htop jq tree xz`), 58 executables. The port `m4`'s executable is `gm4`, covered in E-7.

Harness (`python3 -I sweep.py <scratch>`, written to mirror `SystemRunner`):

```python
env={'PATH':path,'LANG':'C','LC_ALL':'C','TERM':'dumb','NO_COLOR':'1','HOME':d,'TMPDIR':d}   # d: fresh mkdtemp per run
subprocess.Popen([prog,'--version'],env=env,cwd=d,stdin=subprocess.DEVNULL,
                 stdout=subprocess.PIPE,stderr=subprocess.PIPE,process_group=0)
# threads drain both pipes, join(6) each; 30 s timeout, then ch.kill() (one process);
# ps -axo pid=,ppid=,pgid=,command= before and after; walk of d afterwards
```

Rounds (232 runs): **cold** and **warm** with `PATH=<provider>/bin:/usr/bin:/bin:/usr/sbin:/sbin`; **syspath** with `PATH=/usr/bin:/bin:/usr/sbin:/sbin`; **ro** with the scratch `chmod 555`. For `semgrep` and `pipx`, `PYTHONDONTWRITEBYTECODE=1` was added first, because of this:

```
$ find /usr/local/Cellar/semgrep/1.174.0/libexec -maxdepth 8 -type f -name '*.py' | wc -l   → 1761    (*.pyc, -maxdepth 9 → 0)
$ find /usr/local/Cellar/pipx/1.17.1/libexec     -maxdepth 8 -type f -name '*.py' | wc -l   → 141     (*.pyc, -maxdepth 9 → 0)
```

Result across the rounds: 0 timeouts; 0 runs with a held pipe; largest output of a run 4061 bytes (`phpbrew`). Exit codes were the same in all four rounds for every executable except `cask` (below).

### E-3 · Cold `--version`: every case that is not plain

Python's `-6` is SIGABRT; the runner would report `Signalled`.

| Executable | Exit | Cold s | Output, verbatim and trimmed | Written to scratch |
|---|---|---|---|---|
| `httpd` (hb) | **0** | 10.766 | stdout and stderr empty; later rounds stdout `httpd (pid 49260) already running` | nothing; see E-4 |
| `emacs` (hb) | **-6** | 0.018 | `dyld[47852]: Library not loaded: /usr/local/opt/tree-sitter/lib/libtree-sitter.0.26.dylib` | nothing |
| `semgrep` (hb) | **-6** | 4.307 | `dyld[50735]: Library not loaded: …libtree-sitter.0.26.dylib  Referenced from: … /usr/local/Cellar/semgrep/1.174.0/libexec/lib/python3.14/site-packages/semgrep/bin/semgrep-core` | nothing |
| `cask` (hb) | **134**; **127** on syspath | 1.406 | the same dyld line for `emacs-31.1`; on syspath `/usr/local/opt/cask/bin/cask: line 56: emacs: command not found` | nothing |
| `argon2` (hb) | 1 | 0.47 | `Error: no password read` | nothing |
| `bvi` (hb) | 1 | 0.503 | `Input is not from a terminal` | nothing |
| `ffmpeg` (hb) | 8 | 0.161 | stderr: the version banner (882 bytes), stdout empty | nothing |
| `go` (hb) | 2 | 0.072 | `flag provided but not defined: -version` + usage | 9 entries: `Library/Application Support/go/telemetry/local/go@go1.27.3-go1.27.3-darwin-amd64-2026-11-04.v1.count`, `upload.token`, `weekends`, … |
| `links` (hb) | 3 | 0.51 | `Unknown option --version` | `.links/cookies.txt` |
| `lua` (hb) | 1 | 0.924 | `/usr/local/opt/lua/bin/lua: unrecognized option '--version'` | nothing |
| `p11-kit` (hb) | 2 | 1.47 | `p11-kit: unknown global option: --version` | nothing |
| `tmux` (hb) | 1 | 1.971 | `tmux: unknown option -- -` | nothing |
| `unzip` (hb) | 10 | 0.408 | `caution:  both -n and -o specified; ignoring -o` + usage | nothing |
| `gh` (hb) | 0 | 0.083 | `gh version 2.98.0 (2026-08-20)` | `.local/state/gh/device-id` |
| `mise` (hb) | 0 | 0.541 | `2026.9.0 macos-x64 (2026-09-01)`; stderr `mise WARN  mise version 2026.11.0 available` | 9 entries: `.local/share/mise/migrations/…`, `Library/Caches/mise/latest-version` |

Plain exit 0, nothing in scratch, no new process (cold seconds):

- Homebrew: `autoconf` .431, `cloc` .325, `cloudflared` .166, `cmake` .241, `codeburn` 1.261, `curl` .152, `dav1d` .442 (version on stderr), `espeak-ng` .044, `fribidi` .443, `gettext` .026, `git` .024, `htop` .463, `just` .034, `lame` .488, `m4` .459, `maturin` 1.008, `nmap` .585, `node` .076, `nvtop` .458, `pandoc` 2.194, `phpbrew` 2.155 (empty first line, 2024 bytes of PHP deprecation on stderr), `pipx` 1.13, `pkgconf` .025, `shellcheck` .057, `sonar-scanner` .898 (JVM; `INFO  Scanner configuration file: …`), `swiftlint` .125, `tree` .026, `uv` .052, `watch` .408, `wget` .923, `x264` .477, `xcodegen` .025, `xz` .419, `z3` 1.144, `zstd` .432.
- MacPorts: `autoconf` .089, `automake` .139, `gettext` .443, `gperf` .028, `htop` .422, `jq` .048, `tree` .44, `xz` .044.

Totals: 58 executables, 46 exit 0, 12 non-zero. Warm-round maximum 1.101 s (`codeburn`). Six slowest cold: `httpd` 10.766, `semgrep` 4.307, `pandoc` 2.194, `phpbrew` 2.155, `tmux` 1.971, `p11-kit` 1.47.

**ro round:** same exit codes. `links` took 3.022 s and printed `Unable to find or create links config directory. Please check, that you have $HOME variable set correctly…`. `mise` printed `[WARN] migrate: failed create_dir_all: ~/.local/share/mise/migrations` and exited 0. `gh` and `go` exited as before.

**Provider state behind the crashes** (no change made by this research):

```
$ brew list --versions emacs semgrep tree-sitter cask
cask 0.9.1 / emacs 31.1 / semgrep 1.174.0 / tree-sitter 0.27.0 0.26.13
$ ls /usr/local/opt/tree-sitter/lib → libtree-sitter.0.27.dylib libtree-sitter.0.dylib libtree-sitter.a libtree-sitter.dylib pkgconfig
```

### E-4 · `httpd --version`, in full

```
sweep record (cold): code 0, secs 10.766, held [], OUT '', ERR '', wrote nothing in scratch
  new process, ppid 1, its own group: [49260, 49260, '/usr/local/opt/httpd/bin/httpd --version']
$ ps -axo pid,ppid,pgid,user,lstart,command | grep -E "[h]ttpd"
49260     1 49260 pirado  Wed Nov  4 02:57:40 2026  /usr/local/opt/httpd/bin/httpd --version
49262 49260 49260 …  (and 49263, 49264, 49265, 49266)
$ lsof -nP -iTCP -sTCP:LISTEN | grep -i httpd
httpd 49260 pirado 4u IPv6 … TCP *:8080 (LISTEN)        (and the five children)
$ cat /usr/local/var/log/httpd/error_log
[Wed Nov 04 02:57:40.683704 2026] [mpm_prefork:notice] [pid 49260] AH00163: Apache/2.4.68 (Unix) configured -- resuming normal operations
[Wed Nov 04 02:57:40.684663 2026] [core:notice] [pid 49260] AH00094: Command line: '/usr/local/opt/httpd/bin/httpd'
$ ls -la /usr/local/var/run/httpd → httpd.pid (6 bytes, Nov 4 02:57)
$ grep -n "Listen\|ErrorLog" /usr/local/etc/httpd/httpd.conf → 52:Listen 8080   299:ErrorLog "/usr/local/var/log/httpd/error_log"
after kill -TERM 49260: no httpd process, nothing on 8080, httpd.pid gone, error_log third line:
[Wed Nov 04 03:00:10.797856 2026] [mpm_prefork:notice] [pid 49260] AH00169: caught SIGTERM, shutting down
```

The same executable with its documented flag, under the same environment:

```
httpd -v  → exit 0, 0.028 s, `Server version: Apache/2.4.68 (Unix)`, no new process
```

Sources:

- `apr_getopt` (Apache's own repository, `apache/apr` `misc/unix/getopt.c`, https://raw.githubusercontent.com/apache/apr/trunk/misc/unix/getopt.c, read 2026-11-04): `if (os->place[1] && *++os->place == '-') { /* found "--" */ ++os->ind; os->place = EMSG; *optch = os->opt; return (APR_EOF);`
- Apache httpd documentation (https://httpd.apache.org/docs/2.4/programs/httpd.html, read 2026-11-04): "It is designed to be run as a standalone daemon process"; "`-v` Print the version of httpd, and then exit."; "`-DFOREGROUND` (prevent the parent from calling `setsid()` et al)".

### E-5 · Documented flags where `--version` did not hold

Same environment; no new process in any run. `tmux -V` is in its usage line (E-3) and `httpd -v` in its man page and documentation (E-4). The other three flags were not checked against documentation: the runs below are the only evidence for them.

```
tmux -V        → 0, `tmux 3.7c`
lua -v         → 0, `Lua 5.5.1  Copyright (C) 1994-2026 Lua.org, PUC-Rio`
ffmpeg -version→ 0, `ffmpeg version 9.0.1 …`
links -version → 0, `Links 2.30`                    (creates `.links` in HOME)
unzip -v       → 0, `UnZip 6.00 of 20 April 2009, by Debian. Original by Info-ZIP.`
```

### E-6 · Script-based executables, `PATH`, and the other provider

Shebangs of every regular executable under each `bin`, read and not run:

```
/usr/local/bin total 1147, non-script 916; /usr/bin/env forms: python3 7 (+1 `#! /usr/bin/env python3` spit), node 6, perl 6 (+1), python 2,
  bash 1 (cask), sh 1 (+1), ruby 1; the rest are /bin/sh, /bin/bash, /usr/bin/perl*, or absolute prefix paths such as
  23 `#!/usr/local/opt/python@3.14/bin/python3.14`, 17 `#!/usr/local/Cellar/jupyterlab/4.6.3/libexec/bin/python`, 3 `#!/usr/local/opt/node/bin/node`, 1 `#!/usr/local/opt/php@8.4/bin/php`
/opt/local/bin total 90, non-script 50; /usr/bin/env forms: `#! /usr/bin/env sh` 1 (glibtoolize), `#! /usr/bin/env python3` 1 (spit);
  the rest /bin/sh, /usr/bin/perl, 4 `#!/opt/local/libexec/macports/bin/tclsh8.6`
```

Homebrew wrappers, read: `cloc` = `PERL5LIB="…" exec "…/libexec/bin/cloc" "$@"`; `sonar-scanner` = `SONAR_SCANNER_HOME="…" JAVA_HOME="${JAVA_HOME:-/usr/local/opt/openjdk/libexec/openjdk.jdk/Contents/Home}" exec "…" "$@"`. `cask` runs `"$EMACS" -Q --script "$SRCDIR/cask-cli.el" -- $subcommand "$@"` with no `exec`.

What each `PATH` resolves (`shutil.which`, no process started):

```
hb+sys {python3:/usr/local/bin/python3, perl:/usr/bin/perl, sh:/bin/sh, node:/usr/local/bin/node, ruby:/usr/bin/ruby, php:None, m4:/usr/bin/m4}
mp+sys {python3:/usr/bin/python3,       perl:/usr/bin/perl, sh:/bin/sh, node:None,                ruby:/usr/bin/ruby, php:None, m4:/usr/bin/m4}
sys    {python3:/usr/bin/python3,       perl:/usr/bin/perl, sh:/bin/sh, node:None,                ruby:/usr/bin/ruby, php:None, m4:/usr/bin/m4}
```

`--version` under three `PATH`s, `PYTHONDONTWRITEBYTECODE=1` on the Python ones. "own" = own prefix's `bin` + system, "sys" = system only, "other" = the other provider's `bin` + system:

| Executable (shebang) | own | sys | other | Note |
|---|---|---|---|---|
| hb `glibtoolize` (`env sh`) | 0 | 0 | 0 | `glibtoolize (GNU libtool) 2.6.2` |
| mp `glibtoolize` (`env sh`) | 0 | 0 | 0 | same |
| **mp `spit`** (`env python3`) | **1** | **1** | **0** | own, sys: `import requests  ModuleNotFoundError…`; other: exit 0, no output |
| hb `spit` (`env python3`) | 0 | 1 | 1 | same `requests` traceback where it fails |
| hb `glib-mkenums` (`env python3`) | 0 | 0 | 0 | `glib-mkenums version 2.88.3` |
| hb `gdbus-codegen` (`env python3`) | 2 | 2 | 2 | argparse usage: no such flag |
| hb `npm` (`env node`) | 0 | 127 | 127 | own: `11.19.0`, 72 entries in scratch under `node-compile-cache/v26.7.0-x64-…`; others: `env: node: No such file or directory` |

Names, flags and keg-only state:

```
/opt/local/bin/gm4 --version                 → 0, `m4 (GNU M4) 1.4.21`
/opt/local/bin/pkg-config --version          → 0, `0.29.2`
/usr/local/opt/pkgconf/bin/pkg-config --version → 0, `3.0.7`
/opt/local/bin/glibtool --version            → 0, `glibtool (GNU libtool) 2.6.2`
/usr/local/bin/pkg-config -> ../Cellar/pkgconf/3.0.7/bin/pkg-config
/usr/local/bin/m4 → No such file or directory
```

Dyld and SIP:

- `man dyld` (`/usr/share/man/man1/dyld.1`, on this machine, 2026-11-04): "DYLD_FALLBACK_LIBRARY_PATH … If a dylib is not found at its install path, dyld uses this as a list of directories to search for the dylib. For new binaries (Fall 2023 or later) there is no default. For older binaries, there is a default fallback search path of: /usr/local/lib:/usr/lib."
- Apple, System Integrity Protection Guide, Runtime Protections (https://developer.apple.com/library/archive/documentation/Security/Conceptual/System_Integrity_Protection_Guide/RuntimeProtections/RuntimeProtections.html, read 2026-11-04): "Any dynamic linker (`dyld`) environment variables, such as `DYLD_LIBRARY_PATH`, are purged when launching protected processes."

### E-7 · Writes outside the scratch directories

```
$ find /usr/local /opt/local -xdev -maxdepth 7 -type f -newer <marker before the sweep>
/usr/local/var/log/httpd/error_log  /usr/local/var/log/httpd/access_log  /usr/local/var/run/httpd/httpd.pid
  (a later pass at -maxdepth 8 also listed /usr/local/etc/warsaw/local.data, a third-party service's file)
$ find <same, -maxdepth 8> -newer <marker before the E-5/E-6 runs> → nothing
$ find ~/Library/Logs/DiagnosticReports -maxdepth 1 -type f -newer <marker> | wc -l → 11
$ ls -t ~/Library/Logs/DiagnosticReports → semgrep-core-2026-11-04-*.ips, emacs-31.1-2026-11-04-*.ips (11 in total; 8 + 3, matching 4 × 2 crashes + 3 from cask's emacs)
$ find ~ -maxdepth 3 -type f -newer <marker> (excluding Library, projects, .claude) → only ~/.claude-mem/*, another tool's own
$ ls -ld ~/.links ~/.local/state/gh ~/.local/share/mise → ~/.links absent; the other two dated Jun 18 and Sep 3
```

During the sweep, `ps` also showed new `ReportCrash agent`, `/usr/libexec/ReportMemoryException` and `mdworker_shared` processes with parent 1, in their own groups.

### E-8 · Risk classes from documentation

- **Homebrew on `--version` as evidence** (Formula Cookbook, https://docs.brew.sh/Formula-Cookbook, read 2026-11-04): "(despite their widespread use) `foo --version` and `foo --help` are bad tests"; "We want tests that don't require any user input and test the basic functionality of the application."
- **Wrappers** (same page): "For binaries that require setting one or more environment variables to function properly, use" `write_env_script` or `env_script_all_files`; "To surface one or more binaries buried in `libexec` or a macOS `.app` package, use" `write_exec_script` or `write_jar_script`.
- **A home directory on start: `gpg`.** Not installed here, so not observed. GnuPG manual (https://www.gnupg.org/documentation/manuals/gnupg/GPG-Configuration.html, read 2026-11-04): if the default `~/.gnupg` does not exist, "GnuPG creates this directory and a common.conf file with "use-keyboxd"." GnuPG source (GnuPG's GitHub mirror, `g10/gpg.c`, https://raw.githubusercontent.com/gpg/gnupg/master/g10/gpg.c, read 2026-11-04), on `opt.no_homedir_creation = 1` in the first option pre-parse: "Set here here because the homedir would otherwise be created before main option parsing starts."; and "Note that keydb_add_resource may create a new homedir." The same source uses `ARGPARSE_FLAG_NOVERSION` in the pre-parse and `ARGPARSE_FLAG_USERVERS` in the main parse. The read did not show the creating call or the `--version` path side by side.
- **Interpreters** (Python, https://docs.python.org/3/using/cmdline.html, read 2026-11-04): "Execute the Python code contained in *script*, which must be a filesystem path…"; `-V`/`--version` "Print the Python version number and exit."; `PYTHONDONTWRITEBYTECODE` "If this is set to a non-empty string, Python won't try to write `.pyc` files on the import of source modules."
- **Reads stdin** (`argon2`, https://raw.githubusercontent.com/P-H-C/phc-winner-argon2/master/README.md, read 2026-11-04): salt "The salt to use, at least 8 characters."; "Password is read from stdin."
- **`HOME` unset** (Python, https://docs.python.org/3/library/os.path.html, read 2026-11-04): "On Unix, an initial `~` is replaced by the environment variable `HOME` if it is set; otherwise the current user's home directory is looked up in the password directory through the built-in module pwd."
- **Bootstrap on load, from source on this machine:** `/usr/local/Cellar/cask/0.9.1/cask-cli.el` line 37 requires `cask-bootstrap`; `cask-bootstrap.el` at load runs `(package-refresh-contents)` and `(package-install pkg)` from `https://elpa.gnu.org/packages/` and `https://melpa.org/packages/` into a directory under the user's Emacs directory, unless its dependencies are already there. `cask --version` never got that far here, because `emacs` crashed (E-3).
- **ripgrep:** `template.rg.1` (https://raw.githubusercontent.com/BurntSushi/ripgrep/master/crates/core/flags/doc/template.rg.1, read 2026-11-04) synopsis `rg [OPTIONS] --version`; README (https://raw.githubusercontent.com/BurntSushi/ripgrep/master/README.md, same date) "The binary name for ripgrep is `rg`." Homebrew's formula on this machine, `Formula/r/ripgrep.rb`: `system bin/"rg", "Hello World!", testpath`, version URL `15.2.0`. MacPorts' Portfile on this machine, `textproc/ripgrep/Portfile` lines 46-47: `xinstall -m 0755 ${ripgrep_bin} ${destroot}${prefix}/bin/` and `ln -s rg ${destroot}${prefix}/bin/ripgrep`.
- **Not read:** the GNU Coding Standards' `--version` section returned HTTP 429 on four attempts on 2026-11-04, so nothing here rests on it.

### E-9 · What the recordings and tests exercise

```
both-providers-intel-macos26 (captured): hb htop tree xz wget m4 pkgconf gettext; mp htop tree xz jq m4 pkgconfig gettext
both-providers-arm64-macos15 (derived):  hb htop tree xz wget m4 pkgconf gettext; mp htop wget nginx
homebrew-only-intel-macos26 (captured), homebrew-deprecated-and-tap-…: hb htop tree xz wget m4 pkgconf gettext
macports-only-intel-macos12 (synthesised): mp wget nginx
```

(Restricted to those candidate names.) `crates/application/tests/primary_decision.rs` uses `htop`; `crates/adapters/tests/executable_list.rs` uses `ripgrep`; work order 16 part F names `rg`. `data/policy/stateful-packages.toml` line 128: `family = "httpd"`, `names = { homebrew = ["httpd"] }`; line 115: `family = "nginx"`. The candidate rows' linkage on this machine: `/usr/local/bin/{htop,tree,xz,wget,gettext,pkg-config}` are symlinks into the Cellar; `m4` is not linked.

## Findings

### 1. A version flag is not safe by default, and exit 0 does not say it was benign

1. **`httpd --version` starts the server and exits 0** (E-4): a daemon on `*:8080`, with a pid file and logs in the provider's prefix, and no output. `apr_getopt` reads any `--word` as the end of options, so the flag is discarded and httpd does its default thing. This is the strongest evidence for the ruling's removal of the default: under 04's "default `--version`", the check would have passed and left a listening server behind. The exit status, the runner's `truncated` flag, the scratch directory and the process group all looked clean.
2. **Three more behaviours among the 58:** `argon2` takes `--version` as a salt and reads a password from stdin (E-3, E-8); `unzip` reads it as a cluster of short flags and warns about `-n`/`-o` (E-3); `cask` passes it to Emacs and, by its source, can reach the network and install packages on load (E-8; not reached here).
3. **12 of 58 exit non-zero (E-3).** Three are real breakage: `emacs`, `semgrep`, `cask` (a dyld failure on `libtree-sitter.0.26.dylib`) although `brew list --versions` reports all three installed. So the probe found something the provider's own record did not, which is its value. Nine are not breakage, just programs that do not answer `--version` here: `argon2 bvi ffmpeg go links lua p11-kit tmux unzip`. `tmux`, `lua`, `ffmpeg`, `links` and `unzip` answer another flag (E-5), as does `httpd` (E-4). `go` refused the flag and was not tried further, because `go version` is a subcommand.
4. **Harmless-looking writes are common enough to design for.** `gh` writes a device id, `go` a telemetry counter even when it refuses the flag, `links` a cookie file, `mise` migrations and a cache, `npm` 72 compile-cache entries (E-3, E-6). All went to scratch only because `HOME` and `TMPDIR` pointed there; the real `~/.links` stayed absent (E-7).
5. **Writes that `HOME` does not catch:** `httpd`'s logs (absolute, from its config); crash reports (the OS, to the real home, E-7); and Python bytecode: `semgrep` and `pipx` have 1761 and 141 sources and no `.pyc` in their kegs (E-2), so by Python's documented behaviour a run without `PYTHONDONTWRITEBYTECODE` would write into the prefix. That was deliberately not run. `mise` also used the network from a version flag (E-3).
6. **Stdin:** with `/dev/null` a read gets end-of-file and leaves no trace, so which programs read it is not observable under the permitted form. `argon2` is the one that says so, and `bvi` tests for a terminal. Null stdin held: no program hung.
7. **No GUI process appeared in the `ps` diffs and nothing hung** in 232 + 32 runs. Absence is bounded by this sample. `gpg` is not installed, so its case stays cited, not observed. The documentation and source point towards it creating a home directory on start, which a scratch `HOME` would contain. See Cannot determine.

### 2. The environment

8. **The set that sufficed:** `PATH`, `LANG=C`, `LC_ALL=C`, `TERM=dumb`, `NO_COLOR=1`, `HOME`, `TMPDIR` (E-2). No executable failed for lack of another variable. All the failures have other named causes.
9. **`HOME` must be set, to the fresh scratch directory, and must not be left unset or real.** Set to scratch, it took every write in Finding 4. Left unset, a program may fall back to the password database (documented for Python, E-8), i.e. to the real home, which is why it was not tried. "A scratch working directory" alone therefore is not enough: the ruling's sentence needs a scratch `HOME`. `EnvironmentSpec.home`'s comment ("The user's home directory") does not describe this use.
10. **`HOME` need not be writable to print a version** (the `ro` round: same exit codes), but a writable one is quieter and quicker (`links` lost 3 s). Recommended: one fresh writable directory per probe, as `HOME`, `TMPDIR` and cwd, removed afterwards, with removal failure not affecting the check. Nothing in this sample needed them apart.
11. **`TMPDIR` should be the same scratch.** `npm`'s 72 entries landed there (they cannot be told from `HOME` in this setup, which uses one directory). What an unset `TMPDIR` does was not run.
12. **`DYLD_*` is already handled:** `env_clear` plus the `DYLD_` filter (E-1), and SIP purges them for protected programs in any case (E-6). What remains is dyld's *default*: older binaries fall back to `/usr/local/lib`, which on Intel is Homebrew's (E-6). No variable set by the runner removes that default; setting one would change the environment that the probe is meant to test. That is for `NO_CROSS_PROVIDER_LINKAGE`, and for the docs (Finding 27).
13. **Add `PYTHONDONTWRITEBYTECODE=1`** to the probe environment (provider-neutral, and `extra` admits it), so a Python executable's probe does not write bytecode into the destination's prefix. Finding 5's reasoning; both runs with it wrote nothing in either prefix (E-7).
14. **`PATH` = the destination's `bin`, then `SYSTEM_PATH`, nothing else.** The destination comes first because `env` scripts need it: `npm` 127 and Homebrew's `spit` 1 without it (E-6). The named sweep alone would have hidden this, as only `cask` changed on syspath.
15. **The other provider's `bin` must stay off, and this is observed, not theoretical:** MacPorts' `spit` **fails with its own `PATH` and passes when Homebrew's `bin` is on `PATH`**, because `env python3` then reaches Homebrew's Python (E-6). A leaked `PATH` would have certified a MacPorts program by a Homebrew runtime, the failure INV-10 is about. Under a clean `PATH`, `spit` fails and that is an honest answer: it is unchecked or failed, and its source stays. No row for it.
16. **`env` scripts also find the system's interpreter.** `glib-mkenums` passes under all three `PATH`s, via `/usr/bin/python3` when no provider Python is reached (E-6). So an `env` probe's exit 0 does not prove the destination's own interpreter works. The table holds no `env` script for this reason.
17. **A shim.** None exists on this machine, so this is reasoning from E-6 and not an observation (Cannot determine). A shim's target is resolved absolutely (§3 item 10), so it never needs `PATH` to reach the target; the target's `env` shebang, however, resolves through whatever `PATH` the probe carries. So: **the probe runs the resolved target, never through the shim, and the shim directory is never on the probe's `PATH`** (else `env python3` could resolve to a shim for the other provider). Whether a shim *itself* works belongs to `VERIFY_RESOLUTION`.
18. **The working directory needs the runner to change.** `current_dir("/")` with no per-spec field (E-1) is deliberate, and the ruling wants a scratch directory. In this sample cwd, `HOME` and `TMPDIR` were one directory, so a write relative to cwd cannot be told from the others; every path listed in E-3 looks `HOME`-derived. If the design adds a spec field, it reopens that comment's reasoning about `port`'s `current` pseudo-port, so the field should be reachable only by a probe.

### 3. Timeout, forks, output

19. **Observed maximum 10.766 s** (`httpd`, until its parent exited), next 4.307 s (`semgrep`, crashing), and 2.194 s for the slowest benign program (`pandoc`), all cold. The warm maximum is 1.101 s. 120 s (`DEFAULT_TIMEOUT`) is far above any of it; `with_timeout` exists. **Recommended: 30 s per probe** (HYPOTHESIS, from one x86_64 machine; no arm64 data), so the worst case for one probe is 30 + 12 s of drain. Nothing here timed out, so the kill path was not exercised by real programs, only by the existing tests (E-1).
20. **What a kill cannot reach, by case:**
    - *A daemon (`httpd`, E-4):* the parent exits 0, so **no timeout fires at all**. The daemon has its own group (`pgid 49260`), so **group kill would not have reached it either, and `libc` would not change this**. It did not keep the pipes (the reads reached end-of-file), so `truncated` stayed false. The runner has no signal that it happened. Only the curated list stops this: nothing with a daemon default gets a row.
    - *A wrapper without `exec` (`cask`, E-6):* the kill takes `bash`; its `emacs` child survives. Group kill would fix this one, since the child stays in the group. The `exec` wrappers read (`cloc`, `sonar-scanner`) do not have this shape. Not observed through a timeout.
    - *A grandchild holding the pipe:* the existing test's case (E-1). Exit status arrives, reads give up after 12 s, `truncated` is true. No real program in this sample did it (0 held).
21. **`require_success` is the wrong call for a probe.** It refuses `truncated`, whereas the ruling counts the exit status. The design note has to choose what exit 0 plus a held pipe means. Recommended: not a pass. A process left holding the pipe means that the program did not simply print and exit, and the curated row is wrong. But `truncated` currently conflates "cap exceeded" with "drain gave up", so that needs separating.
22. **Output: read and bound, do not discard.**
    - A full pipe is not a risk with the current reader, since it keeps draining past the cap (E-1).
    - Discarding (`Stdio::null`) would save the 12 s wait, but needs a per-spec branch in the one spawn site, and loses the only text that explains a failure: the dyld line naming the missing library (E-3).
    - The largest real output is 4061 bytes, so a cap of 64 KiB is ample against today's 64 MiB default.
    - The decision reads only the status. Output is never parsed; it is kept only as a bounded diagnostic and is treated as hostile on display.

    Checking output would not rescue exit status: `httpd` cold prints nothing, `httpd` when running prints a plausible line and exits 0, and `dav1d` prints to stderr.

### 4. The curated data

23. **What makes a good entry:**
    - the executable has a flag its own documentation names, or that was observed here;
    - the observed run exits 0 in the E-2 environment under the destination's `PATH` and under `SYSTEM_PATH` alone, leaves no process, and writes nothing outside scratch;
    - it is a compiled program or an absolute-shebang script, not an `env` script (Finding 16);
    - it is not a service or stateful package (`data/policy/stateful-packages.toml`; `httpd` and `nginx` are already there, E-9), and not one that exercises a network path;
    - it is exercised by a captured recording where possible (E-9).

    An interpreter, a daemon, or a wrapper that passes argv on is not a first candidate. Keg-only formulae do not get rows, since 04 check 1 already fails them (`m4` here).
24. **`source` can honestly be one of two things, and the file should say which:** an observation on a machine with a date, or official documentation with a date. A row of the second kind (`rg`) runs on a user's machine a program this project has not itself seen run. Citing a flag also doesn't show the flag is benign; `httpd`'s own docs say `-v`, but nothing says what `--version` does. Recommended: allow `cited`, because the alternative is that part F's named example is reported unchecked until an owner runs it; make it visible in the field; and treat replacing it with `observed` as an owner item (Cannot determine).
25. **Proposed shape: `data/policy/liveness-probes.toml`.**

    ```toml
    [[probe]]
    provider = "macports"        # the destination; same vocabulary as curated-mappings.toml
    package = "m4"               # the destination package
    executable = "gm4"           # a leaf, ExecutableName::parse
    argv = ["--version"]         # arguments only; never a program, never a path
    evidence = "observed"        # observed | cited
    observed_version = "1.4.21"  # required when observed; what the evidence is about
    source = "…"
    read_on = "2026-11-04"
    ```

    - **Keyed per provider, package and executable leaf, not package-neutral:** the same leaf is not the same program. `pkg-config` is pkgconf 3.0.7 under Homebrew and pkg-config 0.29.2 under MacPorts (E-6). The evidence belongs to the build (`htop` 3.5.3 and 3.5.2). Names diverge too (`m4`/`gm4`; `pkgconf`/`pkgconfig`).
    - **Loader, failing closed as F's does (SR-11):** `executable` passes `ExecutableName::parse`; each `argv` element matches a closed class (a leading `-` or `--`, then letters, digits, `-`; no `=`, no `/`, no positional), so a row can't name a file or subcommand; at most one row per key; a row whose package matches `stateful-packages.toml` is a load error. This refuses `go version` (a positional), which stays unchecked. That is the safe direction, and the class can grow by entry.
    - **Program path:** never in the row. It is resolved at run time under §3 item 10, only after `EXECUTABLES_OWNED_BY_DESTINATION` has passed for that leaf.
    - **The plan:** `LivenessProbe { flag: "--version" }` is hard-coded in the planner (E-1). It has to carry the row's `argv` (the plan is the contract, INV-02), and an executable with no row becomes a recorded skip (F's `liveness_probes_plus_recorded_skips_equal_the_list`), not a probe.

    **INV-20 reading.** A flag is a dated fact about a package version, so it lives in data with `source` and `read_on`, and the row says which version. What can be detected at run time stays detected: the path, the owner, the active state. A row that goes stale fails in the safe direction for a flag that has gone away (non-zero, source retained). It does not for a future version that turned `--version` into a server start; only re-checking can catch that, so `read_on` should drive a staleness notice as DP-043 does for mappings. **INV-17 reading.** One schema; `provider` names the destination, with no source or direction field, so both migration directions read the same rows through the same code. Coverage may be asymmetric as data (below: `wget` has only a Homebrew row, `jq` only a MacPorts one, because that is what is installed here); that gives an honest unchecked on the other side, not a code path. A test should show the same row is consulted for each direction.

26. **Initial table (15 rows).** All `argv = ["--version"]`, all read 2026-11-04. "Observed" means exit 0 in all four sweep rounds with nothing written in scratch and no new process (E-3), or the E-6 run for the name-mismatched ones.

| # | provider | package | executable | evidence | `source` (what was seen) | In a captured recording (E-9) |
|---|---|---|---|---|---|---|
| 1 | homebrew | htop | htop | observed | `htop 3.5.3-3.5.3` | yes |
| 2 | macports | htop | htop | observed | `htop 3.5.2-3.5.2` | yes |
| 3 | homebrew | tree | tree | observed | `tree v2.3.2 (c) 1996 - 2026…` | yes |
| 4 | macports | tree | tree | observed | same text | yes |
| 5 | homebrew | xz | xz | observed | `xz (XZ Utils) 5.8.3` | yes |
| 6 | macports | xz | xz | observed | same text | yes |
| 7 | homebrew | gettext | gettext | observed | `gettext (GNU gettext-runtime) 1.0` | yes |
| 8 | macports | gettext | gettext | observed | same text | yes, and did not land: see the last section |
| 9 | macports | jq | jq | observed | `jq-1.8.2` | yes |
| 10 | homebrew | wget | wget | observed | `GNU Wget 1.25.0 built on darwin23.6.0.` | yes |
| 11 | macports | m4 | gm4 | observed (E-6: one run) | `m4 (GNU M4) 1.4.21` | yes |
| 12 | homebrew | pkgconf | pkg-config | observed (E-6: one run) | `3.0.7` | yes |
| 13 | macports | pkgconfig | pkg-config | observed (E-6: one run) | `0.29.2` | yes |
| 14 | homebrew | ripgrep | rg | **cited** | `template.rg.1` synopsis `rg [OPTIONS] --version`; `Formula/r/ripgrep.rb` for the leaf | no |
| 15 | macports | ripgrep | rg | **cited** | the same synopsis; `textproc/ripgrep/Portfile` lines 46-47 for the leaf | no |

Notes: rows 11-13 want the same four-round treatment before the file lands. MacPorts' `ripgrep` also installs the symlink `bin/ripgrep -> rg` (E-8); with F's one-probe-per-executable, a MacPorts source's `ripgrep` leaf has no row here and is an honest recorded skip. `httpd` deliberately has no row, though `httpd -v` behaved.

### 5. What a probe does not prove (for the docs)

27. A passing probe proves only that this file, in this environment, with this flag, loaded and exited 0 once. It does **not** prove:
    - **that the program does its job.** Homebrew's own guide calls `foo --version` a bad test (E-8);
    - **that no other program, library or plugin it needs works:** only what is loaded before the flag is handled. `--version` handled before a lazy load will pass;
    - **that its version is the expected one, or the same as the source's:** output is not read;
    - **that it works for this user:** scratch `HOME` means their configuration was never read, and clean `PATH` means their shell's resolution was never tried (`VERIFY_RESOLUTION` covers which file runs, not whether it works under their config);
    - **that its own runtime answered:** an `env` script can pass on the system's interpreter (`glib-mkenums`, E-6);
    - **that it uses no other provider's library:** older binaries' dyld fallback includes `/usr/local/lib` (E-6). That is the linkage check's job;
    - **that it had no side effects:** exit 0 fits a server starting (`httpd`), a network call (`mise`) or a file written (`gh`). A crashing probe also leaves a crash report in the real home, outside the sandboxed `HOME` (E-7);
    - **that data or services migrated.**

    A failing probe doesn't prove a broken program either, when the row's flag is wrong (nine cases, Finding 3), although a dyld abort does. And a package with no row was not probed: unchecked is not passed, and not failed.

## Cannot determine

- **`gpg`: UNRESOLVED as observation.** Not installed by either provider here. The documents and source suggest that a home directory is created at start (E-8), but this was not shown for `--version`. Settle by observing `gpg --version` in the E-2 environment on a machine that has it, and listing the scratch. Curated mappings hold `gnupg`, so this will come up.
- **`rg`: UNRESOLVED as observation.** Not installed. Rows 14-15 are cited. Settle on a machine with `ripgrep` from each provider; an owner item, since nothing here may be installed.
- **`HOME` unset, `TMPDIR` unset:** not run, outside this note's permission, and an unset `HOME` could reach the real home. The conclusion rests on Python's documentation and is not generalised by observation.
- **Which programs read stdin:** not observable with `/dev/null`. Settling it needs a syscall trace (root) or a stdin other than the ruling's.
- **Network use by each program:** only `mise`'s is visible, because it says so. Nothing else was instrumented. So "no network" is not claimed for any row. `cask`'s network path was read in source, not reached.
- **Whether a Python probe without `PYTHONDONTWRITEBYTECODE` actually writes into a keg:** deliberately not run (a write into a prefix). Inferred from Python's documentation and the `.pyc` counts.
- **A real shim:** none exists here (04 ≈687 says a capture can't yet recognise one). Finding 17 is reasoning, to be checked when part I builds them.
- **A probe timing out on a real program:** none did; so what a real kill leaves is from source and the existing test, not an observation.
- **Scope of write detection:** `find` was bounded (depth 7-8) and covered the prefixes and parts of the home; `ps` diffs were taken once per run. A write elsewhere, or a process that started and ended between snapshots, would not show.
- **arm64 and other macOS versions:** this is one Intel machine. On arm64, `/usr/local/lib` is not Homebrew's, so Finding 12 reads differently. Durations will differ.
- **`/private/tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress`:** zero length, owner `pirado`, created 03:02:51, after this note's last program run ended. No Homebrew file under `Library/Homebrew/os` or `extend` names it. Other sessions were active on this machine. Not attributed. If `/usr/bin/python3` under a `PATH` run created it, that would be another side effect; a future run should look at it before and after the system-`python3` cases. The owner may want to remove it, since (per a lead, not a source) its presence is used to make the system's updater offer the Command Line Tools.
- **Other programs using `apr_getopt` with a default action** (`ab`, `htpasswd`, `rotatelogs`): not run. The class is why the file must stay an allowlist.

## Impact on the specification

Line numbers of `docs/spec/` below are at `9b8d68d`.

- **`docs/spec/04-migration-and-safety.md`, verification contract check 4, "safe, non-mutating flag (default `--version`; … skippable with a recorded reason…)": contradicted by E-4**, and replaced by DP-051's ruling; `httpd --version` is the evidence to cite. The text should also stop calling it "safe": the curated row, not the flag, is what is being trusted. `docs/spec-evolution.md` entry needed (main agent's decision).
- **`crates/application/src/planner.rs` ≈974 and `crates/domain/src/plan.rs` ≈355** hard-code and document the default (`flag: "--version"`); the plan needs the row's `argv`, and a schema decision in `docs/schemas/plan.v1.md`. Not a spec statement, but it is the same contradiction in code.
- **DP-051's "the timeout of ADR-0006":** ADR-0006 states the kill and drain, not a value; the value is `DEFAULT_TIMEOUT`. The design note should name the probe's own.
- **ADR-0006, "nothing it spawns is expected to fork", and its Revisit clause:** no longer true of a probe. Note, though, that group kill is *not* the cure for the worst case here (Finding 20). That should be written down before anyone reaches for `libc` on its account.
- **DP-051's "a scratch working directory":** incomplete as written, since the writes went to `HOME` and `TMPDIR`. The ruling needs a scratch `HOME` and `TMPDIR` too (Findings 9-11). This also collides with `system.rs`'s deliberate `current_dir("/")` (Finding 18).
- **INV-10's wording** covers "every `brew` and `port` invocation", and a probe is neither. DP-051 applies the same discipline to it; E-6's `spit` shows why it should. Invariant text is the owner's; reported only.
- **`EnvironmentSpec.home`'s doc comment** ("The user's home directory") doesn't describe a probe, and a probe must not get that value.
- **Recordings:** none holds a probe today (E-1), so H's tests need one, and work order 16 §3 item 9 applies: it has to be captured. Such a capture must run only probes that have a row: this note's own side effects show that a version flag is not read-only by nature.
- **Confirms:** R-21 Finding 15 (no provider subcommand is involved), DP-051's "only the exit status" as a decision rule, though Finding 21 needs settling, and its unchecked-by-default rule, which is the actual safety property.

## Re-check when

- A row's package changes major version, or `read_on` is older than the staleness notice's window: re-run its probe under the E-2 environment.
- Homebrew ships `httpd` other than 2.4.68, or `apr_getopt`'s handling of `--` changes (E-4).
- `ripgrep` or `gnupg` is installed on a machine available to this project: replace the `cited` rows, and settle `gpg`.
- Part I builds shims (Finding 17); an arm64 machine is available (Findings 12, 19).
- `man dyld`'s fallback text changes (E-6): Intel `/usr/local/lib` is why it matters.

## Re-qualification of rows 1-13 (2026-11-04, work order 16 H-2)

Each row in the form it will run: the canonical path with `arg0` set to the placed leaf; a scratch path with a space, serving as `HOME`, `TMPDIR` and working directory; the design note's environment. Three rounds, plus the `ro` round, against the sweep's four: **the cold/warm split was dropped and the sandbox added.** Rows 11-13, added after the sweep with "want the same four-round treatment", had never had an `ro` round; it was first run here. The rounds: the destination's `bin` plus `SYSTEM_PATH`; `SYSTEM_PATH` alone; and under `sandbox-exec` denying the network, every exec but the program itself, and every write outside scratch (there `arg0` is the path). The harness stayed in the session scratch directory. No other program was run, and never `httpd`.

| Rows | Result |
|---|---|
| 1-7, 9-13 | Exit 0 in all three rounds; no process left; nothing in scratch; nothing on stderr. All compiled programs and regular files, no setuid or setgid. So no network and no `/usr/bin` shim was **needed**, which is what the sandbox can show |
| The 12 landed rows, `ro` (work order 16 H-2 round 3) | Scratch `chmod 555` as `HOME`, `TMPDIR` and working directory, plain with the destination's `bin` on `PATH` and under the same sandbox with `SYSTEM_PATH`. Exit 0 in both, nothing on stderr, no process left, the version line printed. All twelve pass, `gettext` (Homebrew) and rows 11-13 included |
| 8 (`macports`, `gettext`, `gettext`) | **Does not land.** The program behaves, but `port -q provides` answers `gettext-runtime` for the placed file, so the key is wrong and check 3 would fail it. A row keyed `gettext-runtime` would need its own decision; not invented here |

`observed_version` is in the owner rule's shape: `pkgconf` holds two kegs here (3.0.6, 3.0.7) and 3.0.7 is linked. `~/Library`, **searched by name and not exhaustively:** nothing naming a row's program was newer than the run's marker, to depth 8 everywhere but `Containers`, `Group Containers`, `Mobile Documents`, `CloudStorage` and `Developer` (not searched), and to depth 3 of `Caches`. A walk of the whole of it did not finish in over an hour and was stopped, so a write under another name, or in those directories, is not excluded; the sandbox round would have denied such a write, and the program exited 0; whether one was attempted was not recorded. No crash report named one, and `xcrun_db` and the Command Line Tools marker were unchanged. **The same limits as above:** one Intel machine, and a sample. Row `gettext` (Homebrew) lands, but its keg holds Java classes (R-27, Ruling on Java classes), so the probe is not reached for it.

Rows 14-15 (`rg`) did not land and cited rows were rejected (SE-113). The `evidence` field did not land. Rows are excluded from `policyReadOn` (DP-057), so Finding 25's staleness sentence is superseded.
