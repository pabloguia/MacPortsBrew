# R-21 — Which provider subcommands are provably read-only for slice 8's executable list and the four destination checks

- **Status:** partially answered (F answered for formulae and ports; H answered except where marked UNRESOLVED)
- **Answered on:** 2026-10-28
- **Machine:** macOS 26.7.1, x86_64, Homebrew 7.0.4 @ `/usr/local` (`/usr/local/Homebrew` HEAD `edb70f031e4170c780799633a1226ff73e1077f4`, tag `7.0.4`), MacPorts 2.12.6 @ `/opt/local`; Xcode selected (`xcode-select -p` = `/Applications/Xcode.app/Contents/Developer`), git 2.55.0
- **Blocking:** work order 16 part P item 1 (the allowlist growth P stages), part F (`08-slice-8-primary-and-resolution.md`: plan `rg` for `ripgrep`), part H (`PROVIDER_INTEGRITY_CHECK`, `EXECUTABLES_OWNED_BY_DESTINATION`, `NO_CROSS_PROVIDER_LINKAGE`, `LIVENESS_PROBE`)

Numbered R-21: R-19 and R-20 exist, and `docs/passes/08-slice-6-plan.md` line 27 names R-21 as the next free identifier.

## Question

For **both** providers (INV-17), what is the minimal set of exact argument
vectors that are provably read-only and that

- **part F** needs: the executables a package installs, from provider metadata,
  so that a package whose executable is not its own name is planned rather than
  refused;
- **part H** needs: a provider integrity check, ownership of the destination's
  executables, absence of cross-provider linkage (INV-07, INV-08), and a liveness
  probe?

The process allowlist (`PERMITTED_PROGRAMS`, `PERMITTED_SUBCOMMANDS` in
`crates/adapters/src/process/mod.rs`, and `data/policy/provider-commands.toml`)
is frozen for pass 16; anything new is staged for the owner, so each candidate
needs source evidence of no write path, or the write path that disqualifies it.

## What is allowed today (read 2026-10-28, branch `wo16/D-service-definition` at `41c26c2`)

- `PERMITTED_PROGRAMS`: `brew`, `port`, and the four probes `sw_vers`, `uname`,
  `sysctl`, `xcode-select` (each pinned to fixed argv in
  `SYSTEM_PROBE_INVOCATIONS`). **`otool` is not permitted.**
- `brew`: `--version`, `--prefix`, `--cellar`, `--caskroom`, `--cache`, `config`,
  `info` (`--json=v2`, `--installed`, no positional), `list` (`--pinned`,
  `--versions`, at most one name).
- `port` (leading `-q` only, `;` refused): `version`, `installed` (at most one
  name), `echo requested|active|leaves`, `info --index --<field>…`, `provides`
  (absolute paths, no `..`).
- `provider-commands.toml` verbs: `verify`/`verify_absent` = `list --versions`
  and `-q installed`.

Prior notes this one builds on rather than repeats:
R-3 (`port -q provides <path>` is a registry read; `port -q contents <name>` is a
registry read, its summary table line 1046-1047), R-5 E-section near line 698
(`port -q contents htop` run unelevated, exit 0), R-1 Finding 1 (an unknown name
can make `brew` clone `homebrew/core`; R-1's UNRESOLVED item on
`HOMEBREW_NO_INSTALL_FROM_API` and the absent cask tap), F-2 E-6/E-11 (`brew
doctor` run once, and it has no other-provider check), R-12 lines 1168-1197
(`otool -L` used on cask payloads), R-20 (which read proves "active").

## Evidence

All commands run 2026-10-28 as uid 501, unelevated. Every `brew` call ran with
`HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_FROM_API=1 HOMEBREW_NO_ENV_HINTS=1`
exported. Only names already installed were passed to `brew` (R-1 Finding 1).

### E-1 · Source identity

```
$ shasum -a 256 /opt/local/bin/port …/macports1.0/macports.tcl …/macports1.0/diagnose.tcl
e04d53dc06ec84cd718cb40f6ba8867b2fa22c20ef9dbfa9c8203a5d6a915e49  /opt/local/bin/port
aa1d25941f3eec0e35a4584471ea7a94471bcdd9a7550978e694fa9e528b2c32  /opt/local/libexec/macports/lib/macports1.0/macports.tcl
d812b9107972817158858bd727a8ae136bb072e53b8e6c6eb7929685b455ec38  /opt/local/libexec/macports/lib/macports1.0/diagnose.tcl
$ git -C /usr/local/Homebrew rev-parse HEAD
edb70f031e4170c780799633a1226ff73e1077f4
$ git -C /usr/local/Homebrew --no-optional-locks status --porcelain -- <the 14 Homebrew files cited below>
(no output; exit 0)
```

So every Homebrew line cited below is the file as committed at that HEAD. (`--no-optional-locks`
is used deliberately; see E-12.)

### E-2 · MacPorts action table (`/opt/local/bin/port`)

Lines 4114-4116 define `ACTION_ARGS_NONE 0`, `ACTION_ARGS_STRINGS 1`,
`ACTION_ARGS_PORTS 2`. The table entries:

```
4127:    provides    [list action_provides       [ACTION_ARGS_STRINGS]] \
4143:    rev-upgrade [list action_revupgrade     [ACTION_ARGS_NONE]] \
4145:    diagnose    [list action_diagnose       [ACTION_ARGS_NONE]] \
4156:    contents    [list action_contents       [ACTION_ARGS_PORTS]] \
4297:    rev-upgrade {id-loadcmd-check}
```

There is no `verify`, `check` or checksum action in the table (lines 4119-4180+).

### E-3 · `port contents` source (`/opt/local/bin/port` lines 3294-3338)

`action_contents` calls `require_portlist`, then for each port
`registry_installed $portname $composite_version …` (lines 3313-3321), then
`[$regref files]` if the entry's state is `installed`, else `[$regref imagefiles]`
(lines 3322-3326), and `puts` each path (lines 3327-3331). Optional
`global_options(ports_contents_size)` / `ports_contents_units` only change the
print format (lines 3299-3307). No `registry::write`, no file open for write, no
`mportopen`, no fetch in the procedure.

### E-4 · `port -q contents` observed

```
$ port -q contents pkgconfig
  /opt/local/bin/pkg-config          (exit 0; one path per line, two-space indent)
real 0.21
$ port -q contents xz | grep /bin/   → 23 paths, /opt/local/bin/lzcat … /opt/local/bin/xzmore
$ port -q contents gettext | grep -c /bin/
21
$ port contents pkgconfig            (no -q, first lines)
Warning: port definitions are more than two weeks old, consider updating …
Port pkgconfig @0.29.2_0 contains:
$ port -q contents nosuchport-xyz
Error: port contents failed: Registry error: nosuchport-xyz not registered as installed.
exit=1
```

`registry.db` mtime before and after: `Sep  7 15:58:03 2026` both times; `find
/opt/local/var/macports/registry -newer <marker>` printed nothing.

### E-5 · `port provides` (already allowed; R-3) observed with two paths

```
$ port -q provides /opt/local/bin/pkg-config /usr/local/bin/pkg-config
pkgconfig
exit=0
real 0.37
```

Source, lines 2188-2207: under `ports_quiet` the "is not provided by a MacPorts
port", "is a directory" and "does not exist" lines are all suppressed, and only a
providing port name is printed. The second path's absence from the output is
that suppression.

### E-6 · `port rev-upgrade` source

- `action_revupgrade` (`port` lines 2648-2660): refuses only if
  `revupgrade_mode eq "rebuild"` and the prefix is unwritable, then calls
  `macports::revupgrade $opts`.
- `revupgrade_mode` defaults to `rebuild` (`macports.tcl` lines 1683-1685); it is
  read from `macports.conf` (`/opt/local/etc/macports/macports.conf` line 253
  holds it commented out: `#revupgrade_mode     	rebuild`). No `port` option sets
  it (the only `rev-upgrade` option is `id-loadcmd-check`, `port` line 4297).
- `macports::revupgrade_scanandrebuild` calls, **before any scan and
  unconditionally**, `revupgrade_update_binary` and
  `revupgrade_update_cxx_stdlib` (`macports.tcl` lines 6432-6434). Both wrap work
  in `registry::write { … }` (lines 6304 and 6381), setting the `binary` flag on
  unflagged files and `cxx_stdlib` on entries that lack it.
- In `rebuild` mode, broken ports are rebuilt through
  `macports::upgrade $portname port:$portname {} $my_options depscache`
  (lines 6893-6905). Only in non-rebuild mode does it just print (lines 6764-6777).
- `--id-loadcmd-check` only adds a check (line 6484); it does not change the mode.
  `-y` (`ports_dryrun`) is consulted at line 6914, after the registry writes at
  6432-6434.

Not run on this machine (see Findings).

### E-7 · `port diagnose` source

`action_diagnose` (`port` lines 2550-2553) calls `macports::diagnose_main`. In
`diagnose.tcl`: `check_compilation_error_cache` (called at line 134) does `file
mkdir $builddir`, `open $filepath w`, runs `/usr/bin/cc`, then `file delete`
(lines 264-274); `check_path` (called at line 119) can append an `export PATH=…`
line to the user's shell profile after a yes/no question (lines 735-751). It is
installation-wide (`ACTION_ARGS_NONE`). Not run.

### E-8 · `brew list --formula <name>` source

- `cmd/list.rb` lines 23-24: `switch "--formula", "--formulae"` — "treat all named
  arguments as formulae". Line 90: `named_args [:installed_formula, :installed_cask]`.
- Lines 252-257: `kegs, casks = args.named.to_kegs_to_casks`; when stdout is not a
  TTY (always, under the process runner) it runs
  `system_command! "find", args: kegs.map(&:to_s) + %w[-not -type d -not -name .DS_Store -print]`.
  On a TTY, `PrettyListing` (lines 348-375) only reads directories and `puts`.
- `cli/named_args.rb` lines 287-305: `to_kegs_to_casks` →
  `to_formulae_and_casks(only:, method: :default_kegs)`; lines 357-358:
  `:default_kegs` → `resolve_default_keg`; lines 522-543 `resolve_kegs`:
  `Formulary.to_rack(name.downcase)` and the rack's subdirectories, raising
  `NoSuchKegError` if none; lines 567-578: prefer the `opt/` symlink, then the
  linked keg, then the single keg.
- Lines 372-373 and 379-380: with `only == :formula`, a `NoSuchKegError` is
  re-raised and a found keg returned **before** the cask branch (lines 384-452),
  so `--formula` never reaches `Cask::CaskLoader`. Without `--formula`, line 381
  calls `warn_if_cask_conflicts` (lines 629-641), which can load a cask.
- `brew.rb` line 201: `rescue RuntimeError, SystemCallError` prints the error; no
  missing-formula search on this path.
- `formulary.rb` lines 1151-1173, `Formulary.to_rack(ref)`: a `ref` containing `/`
  goes to `factory` (line 1157), and `is_brew_name` admits no `/`. If
  `HOMEBREW_CELLAR/<ref>` is a directory, that is returned (lines 1163-1164) with
  no loader involved. Otherwise
  `canonical_name(ref)` → `loader_for(ref).name` (lines 1168, 1172-1173), which
  this note did not trace.
- The existing verify read `list --versions <name>` takes a different branch:
  `cmd/list.rb` lines 189-191 call `filtered_list` (lines 297-313: `Formulary.to_rack`
  plus `rack.exist?`), and `list_casks` only when there are no names.

### E-9 · `brew list --formula` observed

```
$ brew list --formula pkgconf          (exit 0, 35 lines, real 1.75)
/usr/local/Cellar/pkgconf/3.0.7/bin/pkg-config
/usr/local/Cellar/pkgconf/3.0.7/bin/pkgconf   (also bin/pccritic, spdxtool, bomtool)
$ brew list --formula p7zip | grep bin → …/p7zip/17.06/bin/7za, 7zr, 7z
$ brew list --formula coreutils | grep -c /bin/
122
$ brew list --formula cowsay           (a real formula, not installed)
Error: No such keg: /usr/local/Cellar/cowsay
exit=1
$ brew list --formula --versions pkgconf
pkgconf 3.0.6 3.0.7
$ ls -l /usr/local/opt/pkgconf /usr/local/bin/pkg-config
/usr/local/opt/pkgconf -> ../Cellar/pkgconf/3.0.7
/usr/local/bin/pkg-config -> ../Cellar/pkgconf/3.0.7/bin/pkg-config
```

Output order is `find` order, not sorted. With two kegs installed, the listing
is of the `opt`-linked one (3.0.7), as E-8's `resolve_default_keg` says.

### E-10 · `brew info --json=v2 --installed` (already allowed) has no file list

```
$ brew info --json=v2 --installed | jq -c '.formulae[]|select(.name=="pkgconf")|keys'
["aliases",…,"keg_only","keg_only_reason",…,"linked_keg",…,"versions"]
$ … | .installed[0] | keys
["built_as_bottle","installed_on_request","poured_from_bottle","runtime_dependencies","time","used_options","version"]
$ … | {aliases, keg_only, linked_keg}
{"aliases":["pkg-config","pkgconfig"],"keg_only":false,"linked_keg":"3.0.7"}
```

The only occurrence of `pkg-config` in the whole document is the `aliases` array.

### E-11 · `brew linkage` source (`dev-cmd/linkage.rb`, `cache_store.rb`, `linkage_checker.rb`)

- `linkage.rb` line 34: the whole command runs inside `CacheStoreDatabase.use(:linkage)`.
  Lines 25-27 declare `--cached` ("Print the cached linkage values…"), but `run`
  (lines 33-59) never reads `args.cached?`.
- `linkage_checker.rb` lines 38-41 and 59: every `LinkageChecker.new` calls
  `check_dylibs`; lines 147-154 fetch the cached entry, and if it was empty, lines
  239-241 `store.update!(keg_files_dylibs:)`.
- `cache_store.rb` lines 71-76 (`set` marks dirty), 38-62 (`use` calls
  `write_if_dirty!` when the last user leaves), 106-111 (`cache_path.dirname.mkpath`
  and `cache_path.atomic_write(JSON.dump(@db))`), 174-176 (`HOMEBREW_CACHE/"linkage.json"`).
- `linkage_checker.rb` lines 206-209: a dylib whose path is not inside a keg
  (`Keg.for` raises `NotAKegError`) is added to `@system_dylibs`.

```
$ ls -l ~/Library/Caches/Homebrew/linkage.json
ls: …/Library/Caches/Homebrew/linkage.json: No such file or directory
```

Not run (see Findings).

### E-12 · `brew doctor` source

`diagnostic.rb` lines 1053-1077: `check_git_status` iterates
`HOMEBREW_REPOSITORY`, the core tap and the core cask tap, and in each runs
`git status --untracked-files=all --porcelain`. Neither `diagnostic.rb`,
`utils/git.rb`, `brew.sh` nor `bin/brew` contains `OPTIONAL_LOCKS` or
`no-optional-locks` (grep, no matches). git's own manual, observed locally:

```
$ git help -m status | col -b   (lines 542-551)
BACKGROUND REFRESH
       By default, git status will automatically refresh the index, updating
       the cached stat information from the working tree and writing out the
       result. …
```

`cmd/doctor.rb` takes only `--list-checks`, `--json`, `-D` and check names
(lines 24-33); it has no per-formula argument.

### E-13 · `brew audit`, `brew missing`, `brew which-formula` source

- `dev-cmd/audit.rb` line 124: `Utils::GemSetup.install_bundler_gems!(groups: gem_groups)`;
  line 44 `--online`, line 64 `--fix`.
- `cmd/missing.rb` line 22 `named_args [:formula, :cask]` (no `--formula`
  switch); line 32 `args.named.to_resolved_formulae_to_casks`; line 38
  `Homebrew::Missing.deps`. `formula.rb` lines 3040-3056 `missing_dependencies`:
  the keg tab's `runtime_dependencies`, each "missing" unless
  `HOMEBREW_CELLAR/<base>` or `HOMEBREW_PREFIX/opt/<base>` is a directory.
- `cmd/which-formula.sh` calls `ensure_executables_file` per argument;
  `utils/executables.sh` lines 43-45 `odie` when `HOMEBREW_NO_INSTALL_FROM_API`
  is set, and line 53 otherwise runs `"${HOMEBREW_BREW_FILE}" update --auto-update`
  when the cached `executables.txt` (`$HOMEBREW_CACHE/api/internal/executables.txt`,
  line 11) is missing or older than `HOMEBREW_API_AUTO_UPDATE_SECS` (default
  450, line 34).

None run.

### E-14 · Homebrew's own "which keg owns this path" rule

`keg.rb` lines 120-132, `Keg.for(path)`: take `path.realpath`, walk up until
`path.parent.parent == HOMEBREW_CELLAR.realpath`, else `NotAKegError`.

```
$ realpath /usr/local/bin/pkg-config
/usr/local/Cellar/pkgconf/3.0.7/bin/pkgconf
$ brew --cellar
/usr/local/Cellar
```

No Homebrew subcommand among `cmd/` or `dev-cmd/` answers "which installed
formula owns this path" (directory listing; `which-formula` answers a catalogue
question, E-13).

### E-15 · `otool`

```
$ ls -l /usr/bin/otool /usr/bin/vtool
-rwxr-xr-x  78 root  wheel  118640 Sep 24 23:06 /usr/bin/otool
-rwxr-xr-x  78 root  wheel  118640 Sep 24 23:06 /usr/bin/vtool
$ xcrun --find otool
/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/otool
$ otool -L /opt/local/bin/xz        (real 0.05)
	/opt/local/lib/liblzma.5.dylib (compatibility version 14.0.0, …)
$ otool -L /usr/local/Cellar/pkgconf/3.0.7/bin/pkgconf
	/usr/local/Cellar/pkgconf/3.0.7/lib/libpkgconf.8.dylib (…)
$ otool -L /usr/local/bin/SvtAv1EncApp
	@rpath/libSvtAv1Enc.4.dylib (compatibility version 4.0.0, current version 4.2.0)
$ otool -l /usr/local/bin/SvtAv1EncApp | grep -A2 LC_RPATH
         path @loader_path/../lib (offset 12)
$ otool -L /usr/local/Cellar/p7zip/17.06/bin/7z
/usr/local/Cellar/p7zip/17.06/bin/7z: is not an object file
$ ls -l $(getconf DARWIN_USER_TEMP_DIR)xcrun_db
-rw-------@ 1 pirado  staff  11278 Oct 27 21:58 …/T/xcrun_db     (unchanged after these calls)
```

`man xcrun` lines 48-53 and 158-163: xcrun keeps a lookup cache (`--no-cache`,
`--kill-cache`) and "has taken the place of another tool" when invoked under that
tool's name. The 78-link inode shared by `otool` and `vtool` is that shim.

### E-16 · Nothing was written

After every command above, `find … -newer <marker>` over
`/usr/local/Homebrew/.git` (depth 1), `/usr/local/var/homebrew` and
`/usr/local/Homebrew/Library/Taps` (depth 3), `~/Library/Caches/Homebrew` (depth 2),
the `pkgconf`, `p7zip`, `coreutils` racks and `/opt/local/var/macports/registry`
printed nothing.

## Findings

**Part F — the executable list**

1. **MacPorts: `port -q contents <port>` is read-only and gives the file list**
   (E-3 source: registry reads and `puts` only; E-4 observed, registry unchanged,
   0.21 s). Executables are the entries under the detected prefix's `bin/` and
   `sbin/` (E-4: `pkgconfig` → `/opt/local/bin/pkg-config`; `xz` → 23 names).
   No network: the procedure touches only the registry (E-3), consistent with R-3.
   Not yet allowed. A missing port is exit 1 with a `Registry error` line (E-4).
   The output's two-space indent must be stripped by the parser.
2. **Homebrew: `brew list --formula <formula>` is read-only and gives the keg's
   file list** (E-8: resolves the keg from the Cellar/`opt` link and runs
   `find … -print`; E-9 observed, nothing written, 1.75 s). **`--formula` is
   load-bearing:** without it a name can reach the cask loader (E-8, lines
   379-381 and 629-641), which under `HOMEBREW_NO_INSTALL_FROM_API=1` and an
   absent cask tap is R-1's unresolved tap-clone risk. With it, an **installed**
   name resolves from the Cellar alone (`to_rack` lines 1163-1164). A
   non-installed name goes through `loader_for` (E-8, not traced). Here, with
   the core tap present, that was a local `No such keg`, exit 1 (E-9). So
   the read must only be sent for a name the inventory already shows installed.
   For F and H it always is, because the check runs on the destination after it
   is verified present. No network on the installed-name path (E-8).
   Executables are entries under `<keg>/bin` and `<keg>/sbin`; whether they are
   reachable at `<prefix>/bin/<leaf>` depends on `keg_only` and `linked_keg`,
   which the already-allowed `brew info --json=v2 --installed` carries (E-10). Not
   yet allowed (`list` permits `--pinned` and `--versions` only).
3. **`brew info --json=v2` does not list executables** (E-10). Its `aliases` field
   happens to contain `pkg-config` for `pkgconf`; that is a formula alias, not an
   executable, and must not be used as one.
4. **`brew linkage` is not a file-list source** and is refused anyway (finding 9).
5. **INV-17 symmetry for F:** one read per provider, same shape (an installed
   package's registered or kegged files; the adapter filters `bin`/`sbin`). Casks
   are not covered: `brew list --cask` is outside the allowlist (F-22) and this
   note did not establish its read-only status (Cannot determine).

**Part H — `PROVIDER_INTEGRITY_CHECK`**

6. **`port rev-upgrade` can write, in every form, and is refused.** It writes the
   registry unconditionally before scanning (E-6, lines 6432-6434 → 6304, 6381),
   and in the default `rebuild` mode — which no command-line option can change —
   it rebuilds broken ports through `macports::upgrade` (lines 6893-6905).
   `--id-loadcmd-check` adds a check and does not prevent either; `-y` is
   consulted after the registry writes. It is also installation-wide
   (`ACTION_ARGS_NONE`), not a per-package check. Not run here because it can write.
7. **`port diagnose` is refused**: it creates and deletes files and runs a
   compiler (E-7, lines 264-274), can append to the user's shell profile after a
   prompt (lines 735-751), and is installation-wide.
8. **`brew doctor` is refused**: it runs `git status` in Homebrew's repository and
   taps without `--no-optional-locks`, and git documents that `status` writes the
   refreshed index back (E-12) — a write into the provider's prefix. It takes no
   formula argument, so it is not a per-package check either.
   **`brew audit` is refused**: it installs bundler gems into Homebrew (E-13,
   line 124) and lints formula source, which is not installation integrity.
   **`brew missing` is not needed**: what it computes — the keg tab's
   `runtime_dependencies` against Cellar/`opt` directories (E-13, `formula.rb`
   3040-3056) — is available from the already-allowed `brew info --json=v2
   --installed` (`installed[0].runtime_dependencies`, E-10) plus a directory
   stat in the adapter; and it has no `--formula` switch, so its positional can
   reach the cask loader (E-13 line 22, E-8).
9. **There is no provider subcommand for integrity that is read-only on either
   side.** The symmetric, read-only construction is: the file list from finding 1
   or 2, every listed path exists (a filesystem read in the adapter), plus the
   already-allowed state reads (`port -q installed <port>` shows `(active)`;
   `info --json=v2 --installed` shows `linked_keg`). That checks "the provider's
   own record of the destination's files is satisfied on disk", not content
   integrity. The MacPorts registry may hold per-file checksums, but no `port`
   action exposes them (E-2) — UNRESOLVED, and not needed for this construction.

**Part H — `EXECUTABLES_OWNED_BY_DESTINATION`**

10. **MacPorts: `port -q provides <absolute path>`, already allowed (R-3).** Under
    `-q` it prints only a providing port and nothing for an unowned, missing or
    directory path (E-5, source lines 2188-2207), so with several paths the
    output cannot be attributed. **Call it with exactly one path**; the current
    `AbsolutePaths` positional class admits one or more, which is permitted but
    not what the check should send.
11. **Homebrew: no subcommand; Homebrew's own rule is a filesystem one.**
    `Keg.for` (E-14) defines ownership as "the realpath lies in
    `<cellar>/<name>/<version>/`". The adapter can apply that rule with a
    `realpath` and the already-allowed `brew --cellar`; no allowlist growth.
    `brew which-formula` is refused: under the quiet environment it dies, and
    otherwise it runs `brew update --auto-update` and reads a network catalogue
    of what *could* be installed, not what owns a path (E-13). The asymmetry
    (MacPorts' registry vs Homebrew's Cellar layout) is each provider's own record
    and lives in the adapter.

**Part H — `NO_CROSS_PROVIDER_LINKAGE` (INV-07, INV-08)**

12. **`brew linkage` is refused, and could not answer the question if it were
    allowed.** It writes `$HOMEBREW_CACHE/linkage.json` the first time it checks a
    keg (E-11; the file does not exist here, so any run would create it), and
    `--cached` is declared but not honoured. Semantically, it files a dylib outside
    every keg — a MacPorts `/opt/local/lib/…` included — under *system* dylibs
    (E-11, lines 206-209), so a cross-provider link would be reported as a system
    link. Not run because it writes.
13. **MacPorts has no read-only linkage subcommand**: `rev-upgrade` is refused
    (finding 6), and it looks for broken links, not cross-provider ones.
14. **`otool -L` (and `-l` for `@rpath`) answers the question symmetrically and
    is read-only, but it is not a permitted program today** (E-15: it prints each
    load command's install name, e.g. `/opt/local/lib/liblzma.5.dylib` for
    MacPorts' `xz`; 0.05 s; no provider subcommand involved). Three properties the
    staging must carry:
    - `/usr/bin/otool` is an `xcrun` shim (E-15), so it depends on a selected
      developer directory and consults `xcrun`'s cache in the user's temp
      directory. The cache was not modified by these calls; whether a cache miss
      rewrites it was not determined. Gate it on the already-allowed
      `xcode-select -p` succeeding, and treat failure as *unevaluable*, never pass.
    - `@rpath/…` entries need `otool -l`'s `LC_RPATH` (E-15). An `@rpath`,
      `@loader_path` or `@executable_path` entry resolves relative to the binary,
      so a check that sees only `-L` must report it unchecked, not passed.
    - Non-Mach-O executables (Homebrew's `p7zip` `bin/7z` is a shell script, E-15)
      are "not an object file": unchecked, not passed.
    An in-process Mach-O reader avoids the new program entirely but is a new
    dependency or a hand-written parser (ADR-0004). That is the owner's trade-off,
    recorded under Impact.

**Part H — `LIVENESS_PROBE`**

15. **No provider subcommand.** It runs the destination's resolved executable,
    which is outside both `PERMITTED_PROGRAMS` and this note's question. Its
    admission mechanism is work order 16 part H item 2
    (`docs/passes/16-liveness-design.md`). Nothing to stage from this note.

## Allowlist growth to stage

| Program | Subcommand (exact argv) | Flags required | Positionals | Needed by | Already allowed? |
|---|---|---|---|---|---|
| `port` | `-q contents <port>` | leading `-q`; no long options (refuse `--size`, `--units`) | exactly one `is_port_name` | F (executable list); H `PROVIDER_INTEGRITY_CHECK` (file list) | No |
| `brew` | `list --formula <formula>` | `--formula` **required** whenever a name is given (E-8: keeps the cask loader out) | exactly one `is_brew_name` | F (executable list); H `PROVIDER_INTEGRITY_CHECK` (file list) | No (`list` permits `--pinned`, `--versions`) |
| `port` | `-q provides <abs path>` | leading `-q` | one absolute path per call (finding 10) | H `EXECUTABLES_OWNED_BY_DESTINATION` | Yes (R-3); one-path-per-call is a usage rule, not growth |
| `brew` | `info --json=v2 --installed` | as today | none | F (`keg_only`, `linked_keg`); H integrity (`runtime_dependencies`) | Yes |
| `brew` | `--cellar` | none | none | H `EXECUTABLES_OWNED_BY_DESTINATION` (Homebrew's `Keg.for` rule, applied in the adapter) | Yes |
| `otool` (new program, pinned `/usr/bin/otool`) | `-L <abs path>` and `-l <abs path>` | exactly `-L` or `-l` | exactly one absolute path, no `..`, under a detected provider prefix | H `NO_CROSS_PROVIDER_LINKAGE` (both providers) | No; gate on `xcode-select -p` succeeding |

`brew list --formula` collides with the current `list` entry's shape:
`ReadSubcommand` has no "required only when a positional is present" rule. The
staged change should either require `--formula` whenever `list` has a positional,
or add the form as its own shape. `--versions <name>` (the existing verify read)
also lacks `--formula`, but its branch does not reach the cask loader (E-8,
`cmd/list.rb` 189-191 and 297-313).

## Refused

| Form | Why |
|---|---|
| `port rev-upgrade` (any flags, including `--id-loadcmd-check`, `-y`) | Writes the registry before scanning (`macports.tcl` 6432-6434 → 6304, 6381); default `rebuild` mode, not overridable from argv, rebuilds via `macports::upgrade` (6893-6905); installation-wide |
| `port diagnose` | Creates files and compiles (`diagnose.tcl` 264-274); can append to the shell profile (735-751); installation-wide |
| `brew doctor` | Runs `git status` in Homebrew's repo and taps without `--no-optional-locks` (`diagnostic.rb` 1053-1077); git documents an index write-back; no per-formula argument |
| `brew audit` | Installs bundler gems (`dev-cmd/audit.rb` 124); `--online` network; lints formula source, not the installation |
| `brew missing [<name>]` | Not needed (its logic is `info --json=v2 --installed` + a stat, `formula.rb` 3040-3056); no `--formula` switch, so a name can reach the cask loader (`cmd/missing.rb` 22, R-1) |
| `brew linkage [<formula>]` (incl. `--cached`, `--test`) | Writes `$HOMEBREW_CACHE/linkage.json` (`cache_store.rb` 106-111 via `linkage_checker.rb` 239-241); `--cached` ignored; files non-keg dylibs, MacPorts ones included, as system dylibs (206-209) |
| `brew which-formula <exe>` | Dies under `HOMEBREW_NO_INSTALL_FROM_API`; otherwise runs `brew update --auto-update` (`utils/executables.sh` 43-53); catalogue, not ownership |
| `brew list <name>` without `--formula` | Can reach `warn_if_cask_conflicts` / the cask loader (`cli/named_args.rb` 379-381, 629-641): R-1's unresolved tap-clone risk |
| `brew info --json=v2 <name>` as an executable source | Read-only, but carries no file list; `aliases` is not an executable list (E-10) |

## Cannot determine

- **Casks.** Whether `brew list --cask <token>` is read-only under
  `HOMEBREW_NO_INSTALL_FROM_API=1` with no cask tap checked out. It needs
  R-1's disposable prefix. Until then a cask's executables are unknown, and F
  refuses as today.
- **Inactive MacPorts ports.** `action_contents` reads `imagefiles` for a
  non-`installed` entry (E-3). Every port here is active, so the output for an
  inactive port was not observed.
- **Whether an `xcrun` cache miss rewrites `xcrun_db`.** It was not rewritten
  here (E-15). Settling it needs a machine with a cold cache, or Apple's
  documentation of the cache's write policy. The write would land in the user's
  temp directory, not in either provider.
- **What `/usr/bin/otool` does on a machine without the Command Line Tools.**
  Not observable here (Xcode is selected). Runtime capability detection: gate on
  `xcode-select -p`.
- **What `Formulary.loader_for` does for a name that is not installed**, on a
  machine with no core tap checked out (`formulary.rb` line 1168). It is reached
  by `brew list --formula <name>`, and also by today's verify read
  `brew list --versions <name>` (E-8). That read does not reach the cask loader.
  Here, with the core tap present, it is a local exit 1 (E-9). Settling the
  general case needs R-1's disposable prefix.
- **Registry per-file checksums.** No `port` action exposes them (E-2). Content
  integrity, as opposed to presence, stays unchecked.

## Impact on the specification

- **The allowlist growth** above is what part P stages under
  `docs/passes/16-staged/P/`: two new provider read shapes (`port -q contents`,
  `brew list --formula`) and one new program (`otool`). The program is the
  material one. It is the first permitted program that takes a caller-supplied
  path, and the first that is an `xcrun` shim. `SYSTEM_PROBE_INVOCATIONS` pins
  probes to fixed argv, so `otool` fits neither that table nor
  `PERMITTED_SUBCOMMANDS` as they stand. It needs its own shape: an exact flag,
  one absolute path under a detected prefix. **Owner trade-off:** that program
  entry, versus an in-process Mach-O reader that spawns nothing but adds a
  dependency or a hand-written parser under ADR-0004.
- **The work order cites INV-05 for "a form that can write is refused".** INV-05's
  text (`01-invariants.md` line 31-32) covers "upgraded or reconfigured
  implicitly". That fits `rev-upgrade`'s rebuild and `audit`'s gem install
  directly. The index write of `brew doctor` and the cache write of
  `brew linkage` are refused by the `PERMITTED_SUBCOMMANDS` doc comment ("Every
  entry is a read") and the AGENTS.md protocol, not by INV-05's letter. That is
  not a contradiction, but whoever writes the SE entry should cite the right
  authority.
- **Slice 8's "a package whose executable is not its own name is refused"**
  (`prompts/08-slice-8-primary-and-resolution.md` line 73) can be lifted for
  formulae and ports once the two reads are applied. It stays for casks
  (Cannot determine).
- **`port -q provides` with several paths is ambiguous under `-q`** (finding 10).
  `Positionals::AbsolutePaths` admits one or more. Nothing in the specification
  is contradicted, but H's check must send one path per call. A test should hold
  that, or the shape should narrow to exactly one when the growth is staged.
- **`brew linkage` classifies foreign dylibs as system dylibs** (finding 12).
  Any future statement that Homebrew's linkage check detects a MacPorts link
  would be wrong.
- No statement in `docs/spec/` was found to be contradicted.

## Re-check when

- Homebrew moves past 7.0.4 / HEAD `edb70f03`, particularly `cmd/list.rb`,
  `cli/named_args.rb` (the `only == :formula` early return), `dev-cmd/linkage.rb`
  (whether `--cached` becomes honoured), `cache_store.rb`, and `diagnostic.rb`'s
  `git status` call.
- MacPorts base moves past 2.12.6 (sha256s in E-1), particularly
  `action_contents`, `action_provides`'s quiet output, and `revupgrade_scanandrebuild`'s
  unconditional registry updates.
- macOS or Xcode changes `/usr/bin/otool`'s shim behaviour, or the project adopts
  an in-process Mach-O reader (then the `otool` row is withdrawn).
- Before the owner applies the staged growth: re-run E-4 and E-9 and confirm the
  sha256s and HEAD.
