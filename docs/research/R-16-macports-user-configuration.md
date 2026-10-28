# R-16 — Can a MacPorts user configuration outrank the invocation the process executor makes?

- **Status:** answered (residuals for the elevated path, see "Cannot determine")
- **Answered on:** 2026-10-03
- **Machine:** macOS 26.7.1 (25G241), x86_64, MacPorts 2.12.6 @ `/opt/local`.
  Homebrew was not touched by this note.
- **Source read:** `/opt/local/libexec/macports/lib/macports1.0/macports.tcl`
  (sha256 `aa1d25941f3eec0e35a4584471ea7a94471bcdd9a7550978e694fa9e528b2c32`)
  and `/opt/local/bin/port` (sha256
  `e04d53dc06ec84cd718cb40f6ba8867b2fa22c20ef9dbfa9c8203a5d6a915e49`), both
  as installed by MacPorts 2.12.6, read 2026-10-03. Line numbers below refer
  to these exact files.
- **Blocking:** DP-027's mechanism, framed as one provider capability
  ("provider configuration can outrank the invocation", INV-17) rather than a
  Homebrew special case. Bears on INV-10 ("or equivalents") for the mutating
  pass, and on the MacPorts adapter's claim that `-q` alone quiets `port`
  (`crates/adapters/src/macports/mod.rs` lines 49-56 and 268-272).

**About the identifier.** Filed as R-16 in `docs/spec/09-research-checklist.md`,
raised by the review of R-15 (`R-15-homebrew-brew-env.md`), which is the
Homebrew sibling of this note.

## Question

The process executor gives `port` an environment built from nothing (INV-10)
and quiets it by argv (`-q`, which implies `-N`). `port` also reads
configuration files, one of them under the user's `HOME`. Which files, in what
order, and does the `HOME` the executor passes lead `port` to the user's? Which
keys may those files set, and of those, which (a) undo `-q`/`-N` or otherwise
change output the adapter parses, (b) make one of the adapter's reads reach the
network, (c) make a read evaluate a Portfile or Tcl the tool did not choose, or
(d) steer a mutating install? Can anything switch the files off or relocate
them? Who can write them?

## Evidence

### E1 — What the executor passes `port`, and which reads it runs (repository source, read 2026-10-03, HEAD `5c40b11`)

- `crates/adapters/src/macports/mod.rs` lines 103-115 and 138-155: the
  complete argv set is `version`; `-q installed`; `-q echo requested`;
  `-q echo leaves`; `-q echo active`; `-q info --index --line --name` followed
  by exactly one of the `INFO_FIELDS` tails (`--version --revision
  --replaced_by`, `--homepage --license`, `--description`, `--depends_lib`,
  `--depends_run`, `--depends_build`) and validated port names; and
  `-q provides <path>`. **`version` is the one read without `-q`.**
- Same file, lines 889-907, `environment_spec`: `leading_path` is
  `<prefix>/bin`, `<prefix>/sbin`; `extra` is empty; `home` is passed through.
  `crates/adapters/src/process/environment.rs` line 41 appends `SYSTEM_PATH`
  (`/usr/bin`, `/bin`, `/usr/sbin`, `/sbin`, `crates/domain/src/snapshot.rs`
  line 141) and line 47 adds `PINNED_ENVIRONMENT` (`LANG=C`, `LC_ALL=C`,
  `TERM=dumb`, `NO_COLOR=1`, `snapshot.rs` line 146). Nothing else; in
  particular **no `PORTSRC`**.
- The value of `HOME`: `crates/adapters/src/system/machine.rs` line 125
  (`std::env::var("HOME")` through `usable_home`, line 136), handed to the
  registry by `crates/cli/src/app/context.rs` line 157 (`home: if replay {
  None } else { home.clone() }`). On a live run, `port` receives **the user's
  real `HOME`**. `mod.rs` lines 274-281 record why `HOME` cannot simply be
  dropped (F-37: without it `mportinit` falls back to `dscl`, see E2).

### E2 — Which files `mportinit` reads, and in what order (`macports.tcl`, MacPorts 2.12.6)

The compiled-in locations, `macports_autoconf.tcl` lines 40 and 43:

```
40    variable macports_conf_path "/opt/local/etc/macports"
43    variable macports_user_dir "~/.macports"
```

Resolving the user directory, `macports.tcl` lines 1124-1141, verbatim:

```
1126    if {[info exists env(HOME)]} {
1127        set user_home $env(HOME)
1130        set macports_user_dir [file normalize $macports::autoconf::macports_user_dir]
1131    } elseif {[info exists env(SUDO_USER)] && $os_platform eq "darwin"} {
1132        set user_home [exec -ignorestderr dscl -q . -read /Users/$env(SUDO_USER) NFSHomeDirectory | cut -d ' ' -f 2]
1133        set macports_user_dir [file join $user_home [string range $macports::autoconf::macports_user_dir 2 end]]
1134    } elseif {[exec id -u] != 0 && $os_platform eq "darwin"} {
1135        set user_home [exec -ignorestderr dscl -q . -read /Users/[exec -ignorestderr id -un 2> /dev/null] NFSHomeDirectory | cut -d ' ' -f 2]
1136        set macports_user_dir [file join $user_home [string range $macports::autoconf::macports_user_dir 2 end]]
1137    } else {
1139        set macports_user_dir /dev/null/NO_HOME_DIR
1140        set user_home /dev/null/NO_HOME_DIR
```

The configuration search list and its parser, lines 1155-1199:

```
1156    set conf_files [list]
1157    lappend conf_files ${macports_conf_path}/macports.conf
1158    if {[file isdirectory $macports_user_dir]} {
1159        lappend conf_files ${macports_user_dir}/macports.conf
1160    }
1161    if {[info exists env(PORTSRC)]} {
1162        set PORTSRC $env(PORTSRC)
1163        lappend conf_files $PORTSRC
1164    }
1167    set conf_option_re {^(\w+)([ \t]+(.*))?$}
1168    foreach file $conf_files {
1169        if {[file exists $file]} {
...
1182                if {[dict exists $bootstrap_options $option]} {
1183                    global macports::$option
1185                    if {[dict exists $bootstrap_options $option is_path]} {
1186                        if {[catch {set $option [realpath $val]}]} {
1187                            set $option [file normalize $val]
...
1191                    } else {
1192                        set $option $val
```

The per-user `user.conf`, lines 1201-1214, and the list it is filtered by,
line 76:

```
76      variable user_options {}
...
1202    set per_user ${macports_user_dir}/user.conf
1203    if {[file exists $per_user]} {
...
1206            if {[regexp $conf_option_re $line match option ignore val] == 1} {
1207                if {$option in $user_options} {
1208                    global macports::$option
1209                    set $option $val
```

```
$ grep -rn "macports::user_options\|lappend user_options\|user_options " \
    /opt/local/libexec/macports/lib/macports1.0/ /opt/local/bin/port
libexec/macports/lib/macports1.0/macports.tcl:76:    variable user_options {}
libexec/macports/lib/macports1.0/macports.tcl:969:        macports::user_options \
```

(Line 969 is a `global` declaration inside `mportinit`. The `user_options`
in `port1.0/portutil.tcl` and `portinstall.tcl` is a different variable, a
per-port array in the Portfile interpreter.)

Then, in order: `sources_conf` is opened (line 1223; error if unset, line
1216), `variants_conf` if set and present (lines 1280-1302; a variant already
in `variations` is kept, line 1289, and `variations` is the command line's
`global_variations`, lines 1060-1064 and `bin/port` line 5654),
`archive_sites_conf` (default `${macports_conf_path}/archive_sites.conf`,
lines 1306-1309, read lazily at lines 7204-7235), and `pubkeys_conf` (default
`${macports_conf_path}/pubkeys.conf`, lines 1312-1328, every non-comment line
appended to `archivefetch_pubkeys`).

After the files are read, `port` replaces `HOME` for the rest of the run,
line 1386: `set env(HOME) [file join $portdbpath home]`. `user_home` keeps the
original and is used by `copy_xcode_plist` (lines 1990-1992, called only when
`getuid == 0`, line 1799) and exported to Portfile interpreters
(`portinterp_options`, line 79).

### E3 — Which keys a `macports.conf` may set (`macports.tcl` lines 53-73)

```
53    foreach opt [list binpath auto_path clonebin_path extra_env portdbformat \
54        portarchivetype portimage_mode hfscompression portautoclean \
55        porttrace portverbose keeplogs destroot_umask release_urls release_version_urls \
56        rsync_server rsync_options rsync_dir \
57        startupitem_autostart startupitem_type startupitem_install \
58        place_worksymlink xcodeversion xcodebuildcmd xcodecltversion xcode_license_unaccepted \
59        configureccache ccache_size configuredistcc configurepipe buildnicevalue buildmakejobs \
60        universal_archs build_arch macosx_sdk_version macosx_deployment_target \
61        macportsuser proxy_override_env proxy_http proxy_https proxy_ftp proxy_rsync proxy_skip \
62        master_site_local patch_site_local archive_site_local fetch_credentials fetch_threads \
63        buildfromsource revupgrade_autorun revupgrade_mode revupgrade_check_id_loadcmds \
64        host_blacklist preferred_hosts sandbox_enable sandbox_network delete_la_files cxx_stdlib \
65        default_compilers pkg_post_unarchive_deletions ui_interactive] {
...
69    foreach opt [list applications_dir archive_sites_conf ccache_dir developer_dir \
70                      frameworks_dir packagemaker_path portdbpath prefix pubkeys_conf \
71                      sources_conf variants_conf] {
72        dict set bootstrap_options $opt is_path 1
```

How the ones that matter here are consumed (all `macports.tcl`):

- `portdbpath`: created if missing (lines 1336-1340); the registry is opened
  at `${portdbpath}/registry/registry.db` (`registry.path`, line 1387; lines
  1929-1931); source trees live under `${portdbpath}/sources` (`getsourcepath`,
  lines 3268-3279); `HOME` becomes `${portdbpath}/home` (line 1386).
- `prefix`: required (line 1330); `portsharepath` derives from it (line 1522).
- `binpath`: `set env(PATH) $binpath` (lines 1527-1531); otherwise
  `${prefix}/bin:${prefix}/sbin:/bin:/sbin:/usr/bin:/usr/sbin`.
  `findBinary` falls back to a `PATH` search when the configure-time location
  is not executable (lines 510-531).
- `extra_env`: names added to the keep-list before `port` deletes every other
  variable from its environment (lines 1719-1762). It keeps a variable; it
  cannot create one.
- `ui_interactive`: only a false value acts — it *sets*
  `ports_noninteractive` (lines 1465-1473).
- `portverbose`: sets `macports::portverbose`, which `-v` overrides (lines
  1454-1463); it is read in `fetch_port` (line 2337) and line 3901, not by the
  ui channels.
- `buildfromsource`: `never` → `ports_binary_only`, `always` →
  `ports_source_only`, unless the command line set either (lines 1424-1437).
- `proxy_*` and `proxy_override_env`: set `http_proxy`, `HTTPS_PROXY`,
  `FTP_PROXY`, `RSYNC_PROXY`, `NO_PROXY` (lines 1834-1894);
  `get_systemconfiguration_proxies` (line 1838) is the system's local proxy
  configuration.
- `host_blacklist`, `preferred_hosts`: used in site sorting (lines 7021-7029).
- `pubkeys_conf`: `verify_ports_signature` trusts every `.pub`/`.pem` listed
  there, alongside the compiled-in keys (lines 3564-3574).
- `sandbox_enable`, `sandbox_network`: `port1.0/portsandbox.tcl` line 39
  (`portsandbox_active` = supported && `sandbox_enable`) and lines 151-155
  (`sandbox_network` adds `(deny network*)` outside fetch/mirror).
- `master_site_local`, `patch_site_local`, `archive_site_local`: exported as
  environment variables unless already present (lines 1808-1816).

### E4 — How `-q` and `-N` are set, and how the ui channels use them (`/opt/local/bin/port`, `macports.tcl`)

`bin/port` lines 4413-4427 (argv parsing, before `mportinit`):

```
4413                    q {
4414                        set ui_options(ports_quiet) yes
4415                        # quiet implies noninteractive
4416                        set ui_options(ports_noninteractive) yes
4417                        # quiet implies no warning for outdated PortIndex
4418                        set ui_options(ports_no_old_index_warning) 1
...
4424                    N {
4426                        set ui_options(ports_noninteractive) yes
```

`bin/port` lines 5622-5630 install the question handlers only when stdin and
stdout are terminals and neither `ports_quiet` nor `ports_noninteractive` is
`yes`; line 5654 then calls `mportinit ui_options global_options
global_variations`. `macports.tcl` lines 458-480 (`ui_channels_default`):
`debug` → stderr only with `ports_debug`; `info` → stdout only with
`ports_verbose`; `notice` → nothing when `ports_quiet` is set; `warn`/`error`
→ stderr. `ports_quiet`, `ports_noninteractive`, `ports_verbose` and
`ports_debug` are not `bootstrap_options` (E3). The stale-index warning is
skipped when `ports_no_old_index_warning` is set (lines 1918-1926).

### E5 — The read actions' data paths (`/opt/local/bin/port`, `macports.tcl`)

- `installed`, `echo active`: `get_installed_ports` → `registry::entry imaged`
  (`bin/port` lines 518-547).
- `echo requested`, `echo leaves`: `get_ports_with_prop requested 1` and
  `registry::entry imaged` plus `dependents` (lines 691-711).
- `info --index`: `action_info` sets `index_only` (lines 1695-1698), resolves
  the name with `mportlookup` (line 1705) and skips `mportopen` (lines
  1716-1731, the only Portfile open in the action). `mportlookup` (lines
  4162-4209) seeks into `[macports::getindex $source]` for each entry of
  `sources` and reads the record as data (`lindex`, `read`, `dict get`).
  `getindex` (lines 3346-3353) is `<tree>/PortIndex` for a `file://` source
  and `${portdbpath}/sources/…/PortIndex` otherwise. If `PortIndex.quick` is
  missing, `_mports_load_quickindex` writes it next to the index (lines
  4286-4289, `mports_generate_quickindex`, line 4334 opens it `w`).
- `provides`: `registry::file_registered $file` (`bin/port` line 2192).
- `~/.macports` is created only by the interactive shell, when readline is in
  use and a line was entered (`bin/port` lines 4738-4746); `history` lives
  there (line 4765).

### E6 — The documentation

- `port(1)`, installed copy `/opt/local/share/man/man1/port.1.gz` (generated
  from the macports-base repository, MacPorts 2.12.6), read 2026-10-03,
  section FILES:
  > `${prefix}/etc/macports/macports.conf` Global configuration file for the MacPorts system.
  > `${prefix}/etc/macports/sources.conf` Global listing of the ports trees used by MacPorts. This file also enables rsync synchronization.
  > `${prefix}/etc/macports/variants.conf` Global variants used when a port is installed.
  > `~/.macports/macports.conf` User configuration file for the MacPorts system. It overrides the global *macports.conf(5)* file.

  Same page, options: "`-q` Quiet mode, suppress informational messages to a
  minimum, implies `-N`"; "`-N` Non-interactive mode, interactive questions
  are not asked". `PORTSRC` and `user.conf` are not mentioned.
- `macports.conf(5)`, installed copy
  `/opt/local/share/man/man5/macports.conf.5.gz`, read 2026-10-03, FILES:
  > `~/.macports/macports.conf` User-specific configuration override. This file, if found, will be used instead of the default file at `${prefix}/etc/macports/macports.conf`.
- <https://guide.macports.org/chunked/internals.configuration-files.html>,
  read 2026-10-03: "macports.conf is the configuration file used to bootstrap
  the MacPorts system. This file is read by the port command and determines
  how it behaves." "All MacPorts configurations files are located in
  `${prefix}/etc/macports`". Key descriptions quoted there: `binpath` "Sets
  env(PATH), the directory search path for locating system executables
  (rsync, tar, etc.) during port installation."; `extra_env` "Extra
  environment variables to keep."; `buildfromsource` "Controls whether ports
  are built from source or downloaded as pre-built archives.";
  `host_blacklist` "Space-separated list of download hosts that should not be
  used."; `preferred_hosts` "Space-separated list of download hosts that
  should be used preferentially." The page says nothing about a per-user
  file, `PORTSRC`, `user.conf` or precedence (the fetch tool reported none of
  them present).
- The shipped `macports.conf` comments, read 2026-10-03:
  `/opt/local/etc/macports/macports.conf` line 17-18 (`binpath` "…for users
  only and is unsupported."), line 202 (`PORTSRC` in the list of kept
  variables), lines 206-211 (`extra_env`), 262-263 (`ui_interactive`, "Whether
  the user interface should ask interactive questions").

### E7 — The files on this machine (observed 2026-10-03, read-only)

```
$ ls -ld /opt/local/etc/macports /opt/local/etc/macports/* /opt/local/var/macports /opt/local/var/macports/sources
drwxr-xr-x  12 root  wheel    384 Sep  6 21:53 /opt/local/etc/macports
-rw-r--r--   1 root  wheel   2549 Sep  6 21:53 /opt/local/etc/macports/archive_sites.conf
-rw-r--r--   1 root  wheel  11722 Sep  6 21:53 /opt/local/etc/macports/macports.conf
-rw-r--r--   1 root  wheel    515 Sep  6 21:53 /opt/local/etc/macports/pubkeys.conf
-rw-r--r--   1 root  wheel   1184 Sep  6 21:53 /opt/local/etc/macports/sources.conf
-rw-r--r--   1 root  wheel    389 Sep  6 21:53 /opt/local/etc/macports/variants.conf
(and the read-only *.default copies, root:wheel -r--r--r--)
drwxr-xr-x@ 12 root  wheel    384 Sep  7 14:33 /opt/local/var/macports
drwxr-xr-x   4 root  wheel    128 Sep  6 21:53 /opt/local/var/macports/sources
$ ls -ld /opt/local /opt/local/etc /opt/local/var/macports/registry /opt/local/var/macports/home
drwxr-xr-x  11 root      wheel     352 Sep  6 21:53 /opt/local
drwxr-xr-x   3 root      wheel      96 Aug 25 11:56 /opt/local/etc
drwxr-xr-x   4 macports  macports  128 Sep  6 21:53 /opt/local/var/macports/home
drwxr-xr-x   9 root      wheel     288 Sep  7 14:33 /opt/local/var/macports/registry
$ ls -ld "$HOME/.macports"
ls: <home>/.macports: No such file or directory
$ env | grep -c '^PORTSRC='
0
$ grep -v '^\s*#' /opt/local/etc/macports/macports.conf | grep -v '^\s*$'
prefix              	/opt/local
portdbpath          	/opt/local/var/macports
applications_dir    	/Applications/MacPorts
frameworks_dir      	/opt/local/Library/Frameworks
sources_conf        	/opt/local/etc/macports/sources.conf
archive_sites_conf      /opt/local/etc/macports/archive_sites.conf
pubkeys_conf            /opt/local/etc/macports/pubkeys.conf
variants_conf       	/opt/local/etc/macports/variants.conf
$ grep -v '^\s*#' /opt/local/etc/macports/sources.conf | grep -v '^\s*$'
rsync://rsync.macports.org/macports/release/tarballs/ports.tar.gz [default]
$ grep -v '^\s*#' /opt/local/etc/macports/variants.conf | grep -v '^\s*$'
(nothing)
$ grep -v '^\s*#' /opt/local/etc/macports/pubkeys.conf
/opt/local/share/macports/macports-pubkey.pem
```

Every key the system file leaves commented — among them `binpath`,
`extra_env`, `buildfromsource`, `host_blacklist`, `preferred_hosts`,
`proxy_*`, `ui_interactive`, `sandbox_enable` — is therefore free for a later
file to set without anything to overwrite.

### E8 — Observed: two reads under an empty temporary HOME (2026-10-03, 23:00-23:01 -03)

Every command in this note that ran `port` is listed here. `$T` is
`<scratch>/r16.d2Ra`, created by `mktemp -d` in the session scratch directory;
`$T/home` was created empty. Environment shape as the executor builds it
(E1), with `HOME=$T/home`; stdin `/dev/null`. **The real `HOME` was never
passed to `port`.**

```
E=(/usr/bin/env -i PATH=/opt/local/bin:/opt/local/sbin:/usr/bin:/bin:/usr/sbin:/sbin \
   LANG=C LC_ALL=C TERM=dumb NO_COLOR=1 HOME="$T/home")
```

| # | argv | exit | stdout (trimmed) | stderr |
|---|---|---|---|---|
| 1 | `version` | 0 | `Version: 2.12.6` | 108 bytes (the same warning as run 3) |
| 2 | `"-q installed"` (one argument: zsh did not word-split a variable) | 1 | — | `Usage: port [-bcdfknNopqRstuvy] …` — `parse_options` rejected it before `mportinit` (`bin/port` lines 5609-5613) |
| 3 | `version` | 0 | `Version: 2.12.6` | `Warning: port definitions are more than two weeks old, consider updating them by running 'port selfupdate'.` |
| 4 | `-q installed` | 0 | 18 lines, first `  autoconf @2.73_0 (active)` | empty |

After the runs: `find "$T/home"` printed only `$T/home` (nothing created), and
`ls -ld "$HOME/.macports"` still reported "No such file or directory".

One further pure-Tcl check, which loads no MacPorts code (MacPorts' own
interpreter, `tclsh_path` at `macports_autoconf.tcl` line 60):

```
$ echo 'puts [file normalize ~/.macports]; puts [file isdirectory [file normalize ~/.macports]]; puts [info patchlevel]' \
    | /usr/bin/env -i HOME=<scratch-parent>/x /opt/local/libexec/macports/bin/tclsh8.6
<scratch-parent>/x/.macports
0
8.6.17
$ (same, HOME=/var/empty)
/private/var/empty/.macports
0
8.6.17
$ ls -ld /var/empty
drwxr-xr-x  2 root  sys  64 Sep 24 23:06 /var/empty
```

Nothing else was run. No file was written outside `$T` and this note (and the
checklist entry). No configuration file was created, so the precedence
findings below rest on source (E2, E3), not on an observed override.

## Findings

1. **Files, order and precedence.** `mportinit` reads, in this order (E2):
   1. `${macports_conf_path}/macports.conf` — `/opt/local/etc/macports/macports.conf`
      (compiled in at configure time);
   2. `~/.macports/macports.conf` — only if `~/.macports` **is a directory**
      (line 1158);
   3. the file named by `$PORTSRC`, if that variable is set (lines 1161-1164);
   4. `~/.macports/user.conf`, filtered by `user_options`;
   5. then the files the merged result names: `sources_conf`, `variants_conf`,
      `archive_sites_conf` (lazily), `pubkeys_conf`.

   Files 1-3 are parsed by one loop with `set $option $val` and no
   is-it-already-set test (lines 1168-1199), so **for every key, the last file
   that sets it wins: `PORTSRC` > user `macports.conf` > system
   `macports.conf`**, key by key. It is an overlay, not a replacement: a key
   the user file does not mention keeps the system value. `macports.conf(5)`
   says the user file "will be used instead of the default file" (E6); the
   2.12.6 source does not do that, and `port(1)`'s "overrides" is the accurate
   description.

2. **`user.conf` is inert in 2.12.6.** It is opened (lines 1202-1203) but a
   line is applied only when its key is in `macports::user_options`, which is
   initialised to the empty list (line 76) and never appended to (E2 grep).
   No key can be set through it. (A future version that fills
   `user_options` changes this; see "Re-check when".)

3. **The user file follows the `HOME` the executor passes.** `HOME` is set,
   so the first branch applies (line 1126) and `~/.macports` is expanded by
   Tcl against that `HOME` (E8, the `tclsh` check). With the executor's
   current behaviour (E1), **a live run reads the real user's
   `~/.macports/macports.conf`** whenever that directory exists. Leaving
   `HOME` unset does not avoid it: as a non-root user, the third branch asks
   `dscl` for the account's real home directory (line 1135), and as root with
   `SUDO_USER` set, the second branch asks for the *invoking* user's (line
   1132).

4. **Nothing switches the user file off, but `HOME` relocates it, cheaply.**
   No variable or flag disables the search (lines 1156-1164 test only
   `isdirectory` and `PORTSRC`'s presence); `port(1)` documents none (E6).
   Pointing `HOME` at a directory with no `.macports` subdirectory — an
   empty directory MacPortsBrew owns, or `/var/empty` (root-owned, empty on
   this machine, E8) — makes line 1158 false, so the user file and
   `user.conf` are not read. This is cheaper than Homebrew's equivalent (R-15
   finding 6 b), because `port` discards `HOME` itself after reading the
   configuration (line 1386 sets it to `${portdbpath}/home`); the original
   survives only as `user_home`, used to copy the user's Xcode preferences
   when running as root (lines 1799, 1990-1992) and exported to Portfile
   interpreters (line 79). For the reads in E1 neither use is reached.
   Observed: under an empty `HOME`, `port version` and `port -q installed`
   behave normally and create nothing in it (E8).

5. **`PORTSRC` is a lever the executor has and does not use.** It is read
   *after* the user file (line 1163), so a `PORTSRC` the executor sets wins
   over the user file for every key that `PORTSRC` file sets. Pointing it at
   the system `macports.conf` itself re-applies the system's keys last —
   MacPorts' analogue of `HOMEBREW_SYSTEM_ENV_TAKES_PRIORITY`. It does not
   help for keys the system file leaves unset (E7: among them `binpath`,
   `extra_env`, `buildfromsource`, `host_blacklist`, `preferred_hosts`, the
   proxies, `ui_interactive`, `sandbox_enable`), because a file cannot unset a
   key. This is established from source only;
   it was not exercised.

6. **(a) `-q`/`-N` cannot be undone by any configuration file.** `-q` sets
   `ports_quiet`, `ports_noninteractive` and `ports_no_old_index_warning` in
   `ui_options` from argv, before `mportinit` (E4). None of those is a
   `bootstrap_option` (E3), so no file can set or clear them; the ui channels
   read only `ui_options` (lines 458-480). `ui_interactive` can only add
   non-interactivity (lines 1465-1473). `portverbose` does not reach the ui
   channels (E3). **The adapter's claim that MacPorts is quieted by argv
   holds against configuration.** What a file *can* change in parsed output
   is the data, not the formatting:
   - `portdbpath` relocates the registry, so `-q installed`, `-q echo
     requested|leaves|active` and `-q provides` answer from
     `${portdbpath}/registry/registry.db` — any SQLite file the user's
     configuration names (E3, E5).
   - `sources_conf` chooses the ports trees, so `-q info --index` reads the
     `PortIndex` of whichever tree the user names, including a `file://` tree
     the user owns (E5).
   - `prefix` changes `portsharepath` and the default `PATH` `port` uses
     (lines 1522, 1527-1531); a value with no `share/macports` stops every
     read (line 1523-1525).
   - Invalid values add `Warning:` lines on stderr (e.g. lines 1231, 1274,
     1293, 1435, 1486). Stdout parsing is not affected by those.
   - Independently of configuration: **`version` is run without `-q`**, so the
     stale-index warning reaches its stderr (E8 run 3, observed); which index
     is checked depends on `sources_conf` (lines 1918-1926).

7. **(b) No key was found that makes one of the adapter's reads reach the
   network.** The read actions read the registry and a local `PortIndex`
   (E5). The network-shaped keys — `proxy_*`, `host_blacklist`,
   `preferred_hosts`, `fetch_*`, `archive_sites_conf`, `rsync_*`,
   `release_urls` — are consumed by fetch, sync and selfupdate code, which
   the read actions do not call in the paths read. `get_systemconfiguration_proxies`
   (line 1838) reads local system configuration. This is a reading of the
   action code paths in E5, not a traced run with the network disabled; see
   "Cannot determine".

8. **(c) No read evaluates a Portfile or Tcl the tool did not choose.**
   `info --index` explicitly skips `mportopen` (E5); `mportlookup` reads the
   `PortIndex` record as a Tcl list/dict value without evaluating it. The
   configuration files themselves are parsed by a regex into variables, never
   `source`d (E2). What a user configuration *can* do to a read is make
   `port` read data, and in two cases **write**, somewhere the tool did not
   choose: with a user-owned `portdbpath`, `mportinit` creates the directory
   (lines 1336-1340), may set its hidden flag (lines 1741-1747), and writes
   caches under it (lines 1361-1371, `mportshutdown`, line 1950); with a
   user-owned source tree lacking `PortIndex.quick`, `info --index` writes
   that file (lines 4286-4289). Under the system configuration on this
   machine these locations are root-owned (E7), so a user-run read writes
   nothing there.

9. **(d) For a mutating install, a user `macports.conf` steers nearly
   everything.** From E3: `binpath` sets the `PATH` builds run with (it can
   put `/usr/local/bin` — Homebrew's Intel prefix — first, which is exactly
   what INV-10 "or equivalents" forbids the tool from causing);
   `extra_env` widens what survives `port`'s own environment clean-up (only
   for variables the caller passed, which the executor does not);
   `buildfromsource` forces source or binary-only; `variants_conf` adds
   global variants (a non-default variant is one of the three states the
   research rules distinguish: "a source build was forced by configuration
   or by a non-default variant"); `sources_conf` chooses which Portfiles —
   Tcl, run by an elevated `port` — are evaluated; `archive_sites_conf`,
   `host_blacklist`, `preferred_hosts` and `*_site_local` choose where
   archives and distfiles come from; **`pubkeys_conf` chooses which public keys
   verify binary archives** (lines 3564-3574), alongside the compiled-in
   directory; `sandbox_enable` can turn the build sandbox off;
   `startupitem_autostart`/`startupitem_install` decide whether a service is
   installed and loaded; `macportsuser`, `prefix`, `portdbpath`,
   `applications_dir`, `frameworks_dir`, `developer_dir`, `build_arch`,
   `universal_archs`, `macosx_deployment_target`, `cxx_stdlib`,
   `default_compilers`, `configureccache`/`configuredistcc` change what is
   built and where it goes. **Which user's file an elevated `port` reads
   depends on how elevation passes `HOME` and `SUDO_USER`** (finding 3:
   `HOME` if set, else `SUDO_USER`'s home via `dscl`) — runtime detection,
   see "Cannot determine".

10. **Who can write the files.** The system `macports.conf`, `sources.conf`,
    `variants.conf`, `pubkeys.conf`, `archive_sites.conf`, their directory and
    `/opt/local/etc` are `root:wheel`, not group- or world-writable (E7). The
    user cannot change them without elevation — unlike Homebrew's prefix file
    on this machine (R-15 E5). `~/.macports/macports.conf` is under the
    user's own `HOME` and needs no elevation (it does not exist here, E7).
    `PORTSRC` is set by whoever builds the environment, which for this tool is
    the executor.

## Cannot determine

- **Which `macports.conf` an elevated `port install` reads.** It depends on
  whether the elevation mechanism preserves `HOME`, resets it to root's, or
  removes it while setting `SUDO_USER` (finding 3, lines 1126-1136). That is
  the machine's `sudoers` policy, which this note could not read without
  elevation. Runtime capability detection: the mutating executor knows the
  environment it hands the elevated process and can pass `HOME` explicitly.
  **UNRESOLVED** for this machine.
- **That no read reaches the network under any configuration** was
  established by reading the action code paths, not by running the reads with
  a hostile configuration and the network denied. A run with a temporary
  `HOME` containing `.macports/macports.conf` (setting e.g. `proxy_http` to an
  unroutable address) and a traffic capture would settle it. Not done,
  because the work order allowed only an empty temporary `HOME`.
- **The user-file and `PORTSRC` precedence was not exercised.** Settling it
  empirically needs a temporary `HOME` with a `.macports/macports.conf` that
  sets a harmless observable key; `portdbpath` would make `mportinit` write,
  so a key whose effect shows on stdout without a write needs choosing first.
- Whether the `macports.conf(5)` wording ("used instead of") reflects an
  older behaviour or is simply inaccurate was not traced in the repository
  history.

## Impact on the specification

- **Confirms** the MacPorts adapter's statement that quieting is by argv
  (`crates/adapters/src/macports/mod.rs` lines 49-56; `.claude/rules/adapters.md`
  "MacPorts is quieted by argv"): no configuration file can undo `-q`/`-N`
  (finding 6). The Homebrew asymmetry R-15 found does not exist in that form
  for MacPorts.
- **Qualifies** the module comment "Every one of these reads local state - the
  registry, the PortIndex, `macports.conf`" (`mod.rs` lines 49-50): *which*
  registry and *which* PortIndex is decided by up to three configuration
  files, one of them user-writable (finding 6). A snapshot's
  `environment_info` already notes that `portdbpath` may move the registry
  (`mod.rs` lines 850-853); the effective configuration is not recorded.
- **Bears on INV-10 for the mutating pass:** a user `macports.conf` can set
  `binpath` and `extra_env`, the "equivalents" INV-10 names (finding 9). The
  executor's sanitised environment is not the environment an elevated build
  runs in unless the user file is suppressed or detected.
- **For DP-027:** the "provider configuration can outrank the invocation"
  capability should describe MacPorts as: *outranks — yes, per key, through
  the user `macports.conf` (and `PORTSRC`, which the executor does not set);
  can undo quieting — no; can redirect what a read reports — yes
  (`portdbpath`, `sources_conf`, `prefix`); can reach the network or evaluate
  code on a read — not found; can steer a mutation — yes, broadly (finding 9);
  suppressible by the invocation — the user file yes, by `HOME` at near-zero
  cost (finding 4); the system file no, but it is root-owned (finding 10).*
  The DP-027 option set maps across: (a) is "set `HOME` to an empty
  directory" for MacPorts, without Homebrew's trust-store cost; (b)/(c) are
  "read `/opt/local/etc/macports/macports.conf` and `~/.macports/macports.conf`
  with the line regex `^(\w+)([ \t]+(.*))?$` and continuation rule of lines
  1167-1196"; and MacPorts adds a fifth lever, `PORTSRC`, with the limit in
  finding 5.
- **Corrects** the R-16 checklist wording and DP-027 item (2), which say
  `port` "reads `~/.macports/macports.conf` and `user.conf`": it reads the
  former, and opens the latter but accepts no key from it in 2.12.6
  (finding 2).
- **Documentation discrepancy, upstream:** `macports.conf(5)` describes the
  user file as a replacement; the source overlays it (finding 1).

## Re-check when

- MacPorts moves past 2.12.6, especially if `mportinit`'s configuration block
  (`macports.tcl` lines 1124-1214), `bootstrap_options` (lines 53-73),
  `user_options` (line 76) or `bin/port`'s `-q`/`-N` handling (lines
  4413-4427) changes. A non-empty `user_options` makes `user.conf` live.
- The executor's MacPorts environment changes: how `HOME` is chosen, or the
  addition of `PORTSRC`.
- The adapter's argv set changes (E1) — in particular any read that drops
  `--index` from `info` (which would make `info` open and evaluate Portfiles)
  or adds an action that fetches.
- A mutating MacPorts pass is designed: the elevated `HOME`/`SUDO_USER`
  question in "Cannot determine" must be answered on the target machine first.

## Addendum, 2026-10-27: no analytics key (work order 16 part A, DP-028)

DP-028's charter wording promises that MacPortsBrew discloses a provider
configuration that enables the provider's analytics. For MacPorts the question
is whether `macports.conf` accepts any such key.

- **Observed, read-only, 2026-10-27, MacPorts 2.12.6:**
  `grep -rn -i "stats_participate\|stats_url" /opt/local/libexec/macports/lib`
  and `grep -n -i stats .../macports1.0/macports.tcl` both print nothing, and
  the installed `/opt/local/etc/macports/macports.conf` names no `stats` key.
- **So:** the MacPorts entry in `data/policy/provider-configuration.toml`
  carries no `analytics_keys`, and `doctor` has no analytics disclosure to make
  for MacPorts on this version.
- **Not established:** whether a statistics port (installed separately)
  reads a key of its own from elsewhere. Nothing MacPortsBrew runs invokes one.
- **Re-check when** the MacPorts base version changes.
