# R-25 — What `PROVIDER_INTEGRITY_CHECK` can read, what it finds on a real machine, and what it may claim

- **Status:** partially answered (the design is answered; the states this machine does not hold are under Cannot determine)
- **Answered on:** 2026-11-04
- **Machine:** macOS 26.7.2 (25G332), x86_64, Homebrew 7.0.11-143-g874c46a @ `/usr/local` (`/usr/local/Homebrew` HEAD `874c46a98f1832f5c872c94f7d5a33503022c154`), MacPorts 2.12.6 @ `/opt/local`; 194 formulae, 9 casks, 18 ports
- **Blocking:** work order 16 part H (`PROVIDER_INTEGRITY_CHECK`; `04-migration-and-safety.md`, Verification contract, check 2)

**Not the checklist's R-24.** `docs/spec/09-research-checklist.md` gives R-24 to a different question ("What a read can say about a package's executables before it is installed, and about an inactive or file-less port", raised by part F). This note does **not** answer that entry, apart from the two source readings marked in finding 16.

## Question

R-21 refused every provider verb for integrity and proposed a construction: the provider's own file list (`port -q contents <port>`, `brew list --formula <formula>`) with every listed path existing, plus the already-allowed state reads. Part H needs to know:

1. Over every installed package, is "every listed path exists" free of false failures? `lstat` or `stat`? Paths outside the prefix? Paths a user may delete?
2. How large and slow are the lists, against the process runner's limits?
3. Is the `runtime_dependencies` construction correct, is it part of the check, and what is its MacPorts peer?
4. What exactly may the check claim?
5. What are the parsing rules, and what must be checked before a provider's path is stat'd?

## Evidence

All run 2026-11-04 as uid 501, unelevated. Evidence only; conclusions are under Findings.

### E-1 · Source identity

```
$ git -C /usr/local/Homebrew rev-parse HEAD
874c46a98f1832f5c872c94f7d5a33503022c154
$ brew --version
Homebrew 7.0.11-143-g874c46a
$ git --no-optional-locks status --porcelain -- <cmd/list.rb formula.rb cli/named_args.rb keg.rb tab.rb formula_installer.rb system_command.rb formulary.rb dev-cmd/verify.rb>
(no output)
$ shasum -a 256 /opt/local/bin/port …/port1.0/portdestroot.tcl …/port1.0/portstartupitem.tcl
e04d53dc06ec84cd718cb40f6ba8867b2fa22c20ef9dbfa9c8203a5d6a915e49  /opt/local/bin/port        (same as R-21 E-1)
f4306484af1e3acab00e65925255359c6d4ad17b5310017bea321f82396714a0  …/portdestroot.tcl
cc4d2c99b1e303d4677f9d562f8c3a9f076bea2d8e28263f099b805f0ee592b3  …/portstartupitem.tcl
```

Homebrew has moved since R-21 (7.0.4, `edb70f03`). Homebrew line numbers below are at `874c46a`, so R-21's differ.

### E-2 · The sweep

A script run sequentially, stdin closed, with exactly these forms, the names taken from the provider's own inventory:

```
port -q installed
port -q contents <name>                                   (18 names)
port -q info --index --depends_lib --depends_run <name>   (18 names)
brew info --json=v2 --installed
brew list --formula <name>                                (194 names)
```

Environments, built from nothing with `env`-style replacement, mirroring the adapters' sanitised ones:

```
common:  LANG=C LC_ALL=C TERM=dumb NO_COLOR=1 HOME=<home>
port:    PATH=/opt/local/bin:/opt/local/sbin:/usr/bin:/bin:/usr/sbin:/sbin
brew:    PATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin
         HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1
         HOMEBREW_NO_INSTALL_CLEANUP=1 HOMEBREW_NO_COLOR=1 HOMEBREW_NO_EMOJI=1
         HOMEBREW_NO_INSTALL_FROM_API=1
```

The first six `HOMEBREW_` keys are `QUIETING_KEYS` (`crates/adapters/src/homebrew/mod.rs` line 164). `HOMEBREW_NO_INSTALL_FROM_API=1` is not in that list: it was added here as a guard, as in R-21. Each listed path was then probed in-process with `os.lstat`, plus `os.readlink` and `os.stat` for a symlink. Each listing also went through a Python mirror of `crates/adapters/src/listing.rs::parse` (not the Rust itself).

The sweep ran twice, because the first run's raw-file naming collided on dotted names (`python@3.12`, `python@3.13`) in the scratch script. Sizes and exit statuses were the same both times. The figures below are the second run, 05:59:34Z to 06:02:57Z.

### E-3 · Sweep totals

```
=== port ===
packages 18   nonzero exits {}   nonempty stderr {}
total lines 7123   total bytes 344796   total secs 3.13
largest ncurses 3907 lines, 161984 bytes, 0.206 s;  gettext 2398, 148439, 0.196;  xz 314, 12273, 0.173
duration min/median/max 0.17 0.17 0.21
smallest htop 3, gperf 4, tree 5, pkgconfig 10, jq 12
probe seconds 0.84
parser refusals {}
states {'file': 6100, 'symlink-ok:file': 1022, 'symlink-ok:dir': 1}
packages with duplicate lines 0     non-ASCII paths 0     paths with spaces 0

=== brew ===
packages 194  nonzero exits {}   nonempty stderr {}
total lines 148951   total bytes 14047094   total secs 192.06
largest firebase-cli 19560 lines, 2632593 bytes, 2.584 s;  go 15595, 1179473, 1.621;
        jupyterlab 12154, 1479676, 1.641;  llvm 9397, 682932, 1.178;  codeburn 9327, 1113822, 1.315
duration min/median/max 0.80 0.95 2.58
smallest ca-certificates 5, phpbrew 5, opentofu 7, osv-scanner 7, swiftformat 7
probe seconds 26.80
parser refusals {}
states {'file': 134040, 'symlink-ok:file': 14886, 'symlink-ok:dir': 25}
packages with duplicate lines 0
non-ASCII paths 5  (go: …/issue27836.dir/Þfoo.go, Þmain.go; jupyterlab, pipx, semgrep: …/libexec/bin/𝜋thon)
paths with spaces 105 (cmake 24, python@3.12 24, python@3.13 24, python@3.14 24, z3 4, ghidra 3, …)
```

No state `lstat-fail`, `symlink-dangling`, `dir` or `other` occurred on either side. Longest path: Homebrew 216 bytes (`firebase-cli`), MacPorts 80 (`gettext`).

### E-4 · Where the listed paths lie

```
port, by directory: /opt/local/share 6879, bin 84, lib 81, include 49, libexec 30.  Outside /opt/local: 0
port, by kind:  other 3424, share/doc 2409, share/man 1223, share/locale 52, share/info 15
brew, by kind:  other 113369, share/man 22051, share/doc 11109, etc 1213, __pycache__ 524,
                share/locale 361, .brew 194, share/info 123, var 7
brew: paths outside <cellar>/<name>/  0
brew: listings not in exactly one keg  0
brew: listing's keg differs from linked_keg  0   (13 formulae have more than one keg)
brew: listing without a top-level INSTALL_RECEIPT.json  0
```

### E-5 · The Homebrew list is a live walk

`Library/Homebrew/cmd/list.rb` lines 252-256 (`!$stdout.tty?` holds under the runner):

```ruby
kegs, casks = args.named.to_kegs_to_casks
if args.verbose? || !$stdout.tty?
  find_args = %w[-not -type d -not -name .DS_Store -print]
  system_command! "find", args: kegs.map(&:to_s) + find_args, print_stdout: true if kegs.present?
```

An independent walk of each listing's keg, against the captured listing:

```
$ /usr/bin/find <keg> -not -type d -not -name .DS_Store -print       (sets compared per formula)
compared 194 differ 0
```

What each keg holds about its own contents, read from disk:

```
INSTALL_RECEIPT.json keys (pkgconf 3.0.7): aliases arch built_as_bottle built_on changed_files compiler
  homebrew_version installed_on_request loaded_from_api loaded_from_internal_api poured_from_bottle
  runtime_dependencies source source_modified_time time unused_options used_options
changed_files, over the listed kegs: 142 kegs, 760 entries, 0 missing
sbom.spdx.json files[] length: {0: 79 kegs, 1: 108 kegs}; 7 kegs have no sbom
pkgconf files[0]: {'fileName': 'pkgconf-3.0.7.tar.xz', 'checksums': [{'algorithm': 'SHA256', …}]}
.brew/ (pkgconf): pkgconf.rb
```

`keg.rb` lines 250-264, `empty_installation?`: a keg is empty if its non-dot top level holds only metafiles, `.DS_Store` and `INSTALL_RECEIPT.json`. Observed entries other than `.brew/*`, the receipt and the SBOM: minimum 2 (`ca-certificates`, `phpbrew`), then 4.

### E-6 · Symlinks, by where they resolve

```
brew: in-keg 14892, prefix-outside-keg 19.   port: in prefix 1023.   Other provider's prefix: 0 on both.
```

The 19 (`<keg>` is the formula's keg):

```
<keg>/lib/python3.1{2,3,4}/site-packages/certifi/cacert.pem => /usr/local/etc/ca-certificates/cert.pem   (3)
<keg>/libexec/bin/{𝜋thon,python,python3,python3.14} => …/Cellar/python@3.14/3.14.7/…/bin/python3.14 via ../opt/python@3.14  (4 × 3 formulae = 12)
<keg>/etc/clang -> ../../../../etc/clang => /usr/local/etc/clang
<keg>/bin/{npm,npx} -> /usr/local/lib/node_modules/npm/bin/… (2)
<keg>/libexec/conf/sonar-scanner.properties -> ../../../../../etc/sonar-scanner.properties
```

Symlinks to a directory listed as one entry, not descended: Homebrew 25 (e.g. `llvm/23.1.0/etc/clang`), MacPorts 1 (`/opt/local/lib/terminfo`).

### E-7 · Limits in `crates/adapters/src/process/` (read at `9b8d68d`)

- `mod.rs` line 29: `DEFAULT_TIMEOUT = 120 s`. Line 33: `DEFAULT_MAX_OUTPUT_BYTES = 64 * 1024 * 1024`, "per stream" (line 649).
- `system.rs` lines 122-158, `read_bounded`: past the cap it keeps the first `cap` bytes, sets `truncated`, drains the rest, and returns `String::from_utf8_lossy`.
- `mod.rs` lines 917-947, `require_success`: exit success with `truncated` is `ProviderOutputChanged`; `TimedOut` is `ProviderTimeout`; `NotStarted` is `ProviderNotFound`; any other non-success is `ProviderUnhealthy`.
- `listing.rs` lines 199-215, `stdout_of`: `ProviderNotFound` propagates; every other `require_success` error becomes a per-package `UnknownReason::Read(<id>)`.
- `snapshot.rs` line 323: `MAX_PATH_LEN = 4096`. `listing::parse` does not apply it.

### E-8 · `runtime_dependencies`

`formula.rb` lines 3074-3091 (R-21 cited 3040-3056 at `edb70f03`):

```ruby
def missing_dependencies(hide: [])
  tab_deps = any_installed_keg&.runtime_dependencies
  return [] if tab_deps.blank?
  tab_deps.filter_map do |d|
    full_name = d["full_name"]
    next if full_name.blank?
    base_name = Utils.name_from_full_name(full_name)
    next if hide.exclude?(base_name) && ((HOMEBREW_CELLAR/base_name).directory? ||
                                         (HOMEBREW_PREFIX/"opt"/base_name).directory?)
    Dependency.new(full_name)
```

- `formula.rb` lines 2962-2979: `any_installed_keg` is the `opt` keg if optlinked, else the last of `installed_prefixes`.
- `formula.rb` lines 3215-3227: `to_hash`'s `installed` has one entry per keg, `runtime_dependencies` being that keg's `tab.runtime_dependencies`.
- `formula.rb` lines 2803-2808: `Formula.installed` skips a rack that fails `from_rack` (`rescue … []`).
- `keg.rb` lines 644-657, `optlink`: `opt/<alias>` and `opt/<oldname>` are symlinks to the keg.

Sweep over each formula's `linked_keg` entry, or every entry where none is linked:

```
dep edges 939   keys full_name 939, version, revision, pkg_version, declared_directly 939, bottle_rebuild 864
formulae with empty list 94   null []
no Cellar dir 0   no opt dir 0   dep not in the same inventory 0   empty rack 0
recorded pkg_version no longer an installed version: 212      e.g. aom -> libvmaf recorded 3.1.0, installed 3.2.0
all entries: bare names 974, tap-qualified 0
kegs of one formula with differing dependency lists: mise
inventory fields: oldnames present, aliases present
```

### E-9 · What MacPorts has for dependencies

`/opt/local/bin/port`:
- Line 2683 `action_dependents`: answers the reverse direction, through `[$regref dependents]` (line 2703). Not run.
- Lines 2788-2830 `action_deps`: `mportlookup`, then the index or the Portfile, not the registry. Not run.

Already-allowed form, sweep over 18 ports:

```
$ port -q info --index --depends_lib --depends_run xz
libiconv, gettext-runtime

$ port -q info --index --depends_lib --depends_run tree     (no dependencies: two empty lines)

edges 19   not installed []    stderr empty on all 18
```

Registry, as a research instrument only (`sqlite3` is not a permitted program):

```
$ /usr/bin/sqlite3 -readonly "file:/opt/local/var/macports/registry/registry.db?immutable=1" ".schema files" ".schema dependencies"
CREATE TABLE files (id INTEGER, path TEXT, actual_path TEXT, active INTEGER, binary BOOL, FOREIGN KEY(id) REFERENCES ports(id) ON DELETE CASCADE);
CREATE TABLE dependencies (id INTEGER, name TEXT, variants TEXT, FOREIGN KEY(id) REFERENCES ports(id) ON DELETE CASCADE);
$ … "select count(*), sum(active), sum(path<>actual_path), sum(actual_path is null) from files; select count(*) from dependencies; …"
7123|7123|0|0
19
(dependencies whose name is not an installed port: 0;  files not under /opt/local: 0)
```

### E-10 · Paths outside the prefix: MacPorts' own rule

- `macports.conf` lines 6, 30, 33: `prefix /opt/local`, `applications_dir /Applications/MacPorts`, `frameworks_dir /opt/local/Library/Frameworks`.
- `port1.0/portdestroot.tcl` line 392: allowed outside the prefix are `$applications_dir $frameworks_dir /Library/LaunchAgents /Library/LaunchDaemons /Library/StartupItems`. Lines 410-422: anything else is `ui_warn "violation by …"`, then "it will be an error in future releases!", with `# error "mtree violation!"` commented out. Lines 356 and 425: `destroot.violate_mtree yes` skips the check with a warning.
- `port1.0/portstartupitem.tcl` lines 273-281, `install_darwin_launchd`: as root with `install`, a plist symlink goes to `${destroot}/Library/${dstdir}`; otherwise to `${prefix}/etc/${dstdir}`.

### E-11 · Encoding

- `macports1.0/macports.tcl` line 1076: `encoding system utf-8`. Line 1717: `set env(LANG) en_US.UTF-8`.
- `system_command.rb` lines 271-272: `$stdout << Formatter.redact_secrets(line, @secrets)`. `grep -n "scrub\|force_encoding\|encode("` over it finds nothing.

```
$ env -i LANG=C LC_ALL=C TERM=dumb NO_COLOR=1 PATH=… /opt/local/libexec/macports/bin/tclsh8.6 enc.tcl | xxd
   (enc.tcl: puts [encoding system]; puts "Þ"; puts "\U0001D70B")
00000000: 7574 662d 380a c39e 0aef bfbd 0a         utf-8........
```

Raw formats, trailing `$` added by `sed`:

```
  /opt/local/bin/htop$
  /opt/local/share/icons/hicolor/scalable/apps/htop.svg$
  /opt/local/share/man/man1/htop.1.gz$
```

```
/usr/local/Cellar/ca-certificates/2026-08-13/INSTALL_RECEIPT.json
/usr/local/Cellar/ca-certificates/2026-08-13/.brew/ca-certificates.rb
/usr/local/Cellar/ca-certificates/2026-08-13/libexec/post-install
```

Failure output: R-21 E-4 (`port -q contents nosuchport-xyz`: `Error: port contents failed: Registry error: nosuchport-xyz not registered as installed.`, exit 1; same `port` sha256) and R-21 E-9 (`brew list --formula cowsay`: `Error: No such keg: /usr/local/Cellar/cowsay`, exit 1, Homebrew 7.0.4). Neither was re-run.

`port` lines 3333-3338, `action_contents`: with no files registered it runs `ui_notice "Port … does not have any files registered."` and prints no path. Line 3316 calls `registry_installed $portname $composite_version all yes`, and `registry_installed` (lines 159-163) takes `only_active` as its fourth argument.

### E-12 · `brew verify`

`Library/Homebrew/dev-cmd/verify.rb` lines 12-17: "Verify the build provenance of bottles using GitHub's attestation tools. This is done by first fetching the given bottles … depends on the GitHub CLI." Line 62: `bottle.fetch`. Lines 29-30: `--force`, "Remove a previously cached version and re-fetch." Named args are formulae. It is not in R-21's tables. Not run.

### E-13 · Nothing was written

After every command above, for `find … -newer <marker>` over `/usr/local/Homebrew/.git` (depth 1), `/usr/local/var/homebrew`, `/usr/local/Homebrew/Library/Taps` (depth 3), `~/Library/Caches/Homebrew` (depth 2), `/usr/local/Cellar` (depth 2) and `/opt/local/var/macports/registry` (depth 2): `0 0 0 0 0` lines. `registry.db` still `Sep  7 15:58`.

### E-14 · Recordings

```
$ grep -l -e '--formula' fixtures/recordings/*.json | wc -l    → 0
$ grep -l '"contents"' fixtures/recordings/*.json | wc -l      → 0
```

## Findings

**The central result**

1. **The construction is not symmetric, and R-21 finding 9's wording is wrong for Homebrew.** `port -q contents` prints a stored record: the registry's file map (R-21 E-3; 7123 rows, 7123 lines, E-9). `brew list --formula` prints a walk of the disk at the moment of asking (E-5, and 194 of 194 identical to an independent `find`). **So on Homebrew, "every listed path exists" cannot fail under `lstat`: a deleted file is simply not listed.** No per-file record exists to compare against: the receipt has no file list, the SBOM's `files[]` holds at most the source tarball, and `changed_files` is partial, covering only relocated files (760 of 148,951 here) (E-5). This conclusion comes from reading the source. No file was deleted to confirm it, because that would be a mutation.

**Question 1 — the sweep**

2. **Counts: 0 missing and 0 dangling of 156,074 listed paths, under both `lstat` and `stat`** (E-3).

   | | listed | `lstat` fails | symlinks | dangling (`stat` fails) |
   |---|---|---|---|---|
   | MacPorts, 18 ports | 7,123 | 0 | 1,023 | 0 |
   | Homebrew, 194 formulae | 148,951 | 0 | 14,911 | 0 |

3. **What that does and does not show.** The MacPorts row is real evidence of no false failure, over a small sample of plain command-line ports. The Homebrew `lstat` column is **not** evidence of anything (finding 1). Only the Homebrew dangling column can tell, over 14,911 symlinks.
4. **`stat` is the pass condition, and the two failures are reported separately.** Pass: `stat` succeeds for every listed path. `lstat` alone is the literal "record satisfied" on MacPorts, but on Homebrew it is vacuous, and a dangling link means a file that cannot be used. Named classes:
   - `MISSING`: `lstat` gives `ENOENT` or `ENOTDIR`.
   - `DANGLING`: `lstat` succeeds and `stat` gives `ENOENT`, `ENOTDIR` or `ELOOP`.
   - **Any other errno** (`EACCES`, `ENAMETOOLONG`, `EIO`) proves nothing about the file, so the check is unevaluable, never failed and never passed. None occurred here; it varies by machine, so it is runtime detection.

   `std::fs::symlink_metadata` and `std::fs::metadata` do this with no new dependency.
5. **The honest risk in `stat`** is the 19 symlinks leaving a keg (E-6). `llvm`'s `etc/clang` and `sonar-scanner`'s properties point into `<prefix>/etc`, and a user could delete those. Then `DANGLING` fires on a working package. That direction is safe: the source is kept and the path is named (INV-01, INV-03). It stays as a failure.
6. **No exclusions: not docs, not caches.** Excluding `share/doc`, `share/man` and `share/info` would blind the check to **51.2 % of MacPorts' entries** (3,647 of 7,123) and 22.3 % of Homebrew's (E-4). The exclusion is not needed either, since no false failure occurred. It would also change the claim into "the files we decided matter are present", which the provider never said. A false failure costs a kept source and a named path; a false pass can cost the only working copy. A user's deletion can only be in play for a destination that was already installed, not one this run has just installed.
7. **Paths outside the prefix: none on this machine, not to be stat'd, and not to be passed silently.** No installed port lists one (E-4), so nothing here can exercise it. They are legitimate on MacPorts: `/Applications/MacPorts`, `/Library/LaunchAgents`, `/Library/LaunchDaemons`, `/Library/StartupItems` are MacPorts' own allowed roots, and a port setting `destroot.violate_mtree` may go anywhere, with a warning (E-10). Such a path is therefore not a sign of tampering, only of a package this check does not cover. **Recommended rule:** a listing with any entry outside the detected prefix makes the check **unevaluable for that package** (a named reason such as `LISTS_OUTSIDE_PREFIX`), the rest being reported as usual, and its source is never removed (work order part H item 3). The cost, to be stated openly: MacPorts application bundles, and service ports whose plist was linked into `/Library`, end up unchecked. The later widening is reversible: `lstat` under a dated root list, after a captured recording exists and `applications_dir`, which is configurable, has a read.

**Question 2 — sizes**

8. **Nothing is close to the limits.** The largest lists are 2,632,593 bytes (`firebase-cli`, 3.9 % of the 64 MiB cap) and 161,984 bytes (`ncurses`, 0.24 %). The slowest calls are 2.58 s and 0.21 s against 120 s (E-3, E-7). The longest path is 216 bytes.
9. **A list over the cap fails closed already.** `read_bounded` truncates and flags, `require_success` gives `ProviderOutputChanged`, `stdout_of` gives a per-package `UnknownReason::Read(ProviderOutputChanged)`. `ProviderTimeout` takes the same route (E-7). The check must map all of these to **unevaluable**, never to failed: a truncated list proves nothing missing, and a truncated line must never reach `stat`.
10. **The stats themselves run in-process, outside the runner's timeout.** 26.80 s for 148,951 paths and 0.84 s for 7,123 (E-3, measured in Python, including first touch). Scaled by path count, the largest list is roughly 3.5 s. That is an estimate, not a measurement. The loop should test the cancellation token between paths.

**Question 3 — `runtime_dependencies`**

11. **"Each dependency has a directory in the Cellar" is close to `formula.rb` but not what it says, and should not be copied as it stands** (E-8):
    - Homebrew's rule is Cellar **or** `opt/<base>`. The `opt` clause covers aliases and old names (`keg.rb` 644-657), so Cellar alone would misreport a renamed dependency.
    - It reads `any_installed_keg`, not the linked keg. Kegs of one formula can differ (`mise`, E-8). **Read the `installed[]` entry whose `version` equals `linked_keg`**, which check 1 has already required.
    - `.directory?` passes for an empty rack, and that is weaker than installed.
    - Blank is treated as "no dependencies". **`null` must be unevaluable**; only `[]` is an honest "none" (94 here, 0 null).
    - **Versions must not be compared.** 212 of 939 edges record a version no longer installed on a healthy machine (E-8).
12. **Recommended form: compare against the same inventory, with no filesystem read.** A dependency is satisfied when its base name equals a formula's `name`, or is in its `aliases` or `oldnames`, in `brew info --json=v2 --installed`, with a non-empty `installed`. It constructs no path from metadata (§3 item 10), adds no read, and works under `--replay`. 939 of 939 edges pass here (E-8). A `full_name` must be split with Homebrew's rule (last `/` component). No tap-qualified name exists on this machine, so that case is unobserved.
13. **It should be part of the check for Homebrew.** By finding 1, this is the only provider-kept record the Homebrew check tests that can actually fail, alongside the dangling links.
14. **MacPorts has no peer read, so this is a declared asymmetry and not an INV-17 violation.** `port -q info --index --depends_lib --depends_run` is allowed and gave 19 of 19 here, matching the registry's 19 rows (E-9). But it is the index's current declaration, blind to the variants and version actually installed, so it is not a record of this installation: on another machine it can differ from what was installed. Do not use it. The real record is the registry's `dependencies` table (E-9), which only `sqlite3` or an equivalent can read: a new program or a new dependency, not justified by this. `port dependents` answers the reverse direction and cannot enumerate a dependency that is gone. INV-17 binds the domain model, planner and executor; **the one rule above the adapter is "each provider's readable record of this package is satisfied"**, and what that record is lives in the adapter, like the quieting asymmetry. On MacPorts a missing dependency's own files are not examined by this check. That is stated, not hidden.

**Question 4 — the claim**

15. **Wording.**

    **`PROVIDER_INTEGRITY_CHECK` may assert:**
    - MacPorts (basis: registry file map): "every file the MacPorts registry records for the active port is present under the detected prefix, and every recorded symlink resolves."
    - Homebrew (basis: keg walk and receipt): "the linked keg holds a receipt and more than its metadata, every symlink in it resolves, and every runtime dependency its receipt records is installed."
    - Both, in one user-facing sentence: "the provider's read of this package found nothing missing." Carry a provider-neutral `basis` field in `--json`, so the difference in strength is visible and not averaged away.

    **It may not assert:** that file contents are unmodified, since no checksum is read and the registry's `files` table has none (E-9, which settles R-21's open question for this schema); signature or provenance; that a binary runs (`LIVENESS_PROBE`); anything about linkage (`NO_CROSS_PROVIDER_LINKAGE`); that no extra files exist; that the package is untampered; **that a file absent from a Homebrew keg was detected (finding 1)**; or that the *provider* judged anything. "Integrity verified" and "the provider's check passed" are both false statements.

    **Spec 04's "where the provider offers one": the honest position is that neither provider offers one that is read-only** (R-21 findings 6-9). `brew verify` joins the refusals: it fetches bottles into the cache, needs the GitHub CLI and network, and attests the bottle rather than the installed keg (E-12). Read literally, the clause makes check 2 vacuously true on both sides, which is a silent pass. It must instead be read as a check **this tool constructs from the provider's records**. The identifier may stay, being stable; the sentence should not. A Homebrew formula passing with this weaker basis, rather than being reported unchecked forever, is a judgement that should be recorded: permanently unchecked would mean a Homebrew destination never releases its source, which is a one-direction special case against INV-17.

**Question 5 — parsing and path checks**

16. **Formats and signals** (E-3, E-7, E-11):

    | Signal | Answer |
    |---|---|
    | Exit 0, every line the indent (`"  "` or `""`) plus a plain absolute path | parse |
    | Exit 0, empty stdout | `ProviderOutputChanged` (`listing::parse`), unevaluable |
    | Exit 0, any other line (a stray warning on stdout, a header) | `ProviderOutputChanged`, unevaluable |
    | Non-UTF-8 byte (lossy conversion gives U+FFFD), control, format or separator character | `ProviderOutputChanged`, unevaluable for the whole list |
    | An entry that is the parent of another | `ProviderOutputChanged`, unevaluable (a symlink to a directory is one entry and passes, 26 here) |
    | Truncated at the cap | `ProviderOutputChanged`, unevaluable |
    | Non-zero exit (`Registry error`, `No such keg`, a failing `find` under `system_command!`) | `ProviderUnhealthy`, unevaluable |
    | Timeout | `ProviderTimeout`, unevaluable |
    | Stderr with exit 0 | ignored for the decision; none observed in 212 reads |

    **Only `stat` errnos and a missing dependency make the check fail.** Everything above is "could not tell". `listing::parse` already implements the parse rows and accepted all 212 listings in the mirror (E-3). Spaces need nothing special: lines split on `\n` alone and nothing is trimmed (105 such paths passed). UTF-8 non-ASCII passed on Homebrew under `LC_ALL=C` (5 paths). A newline in a file name splits one path into two: the second is refused unless it is absolute, and if absolute it still faces the location check below. Two source readings relevant to the checklist's other R-25: a file-less port prints no path (E-11), so it lands on the empty-stdout row, and `contents` selects an active entry first (E-11). Neither was observed.

17. **Checks before any `stat`, in order:**
    1. `FilesystemScope::Elsewhere` (`--replay`, `--from-snapshot`): no `stat` at all; the check is unevaluable (INV-15), as `VERIFY_RESOLUTION` already is (spec 04).
    2. `require_success`, then `listing::parse`.
    3. New: length at most `MAX_PATH_LEN`, else `ProviderOutputChanged`, because `parse` does not apply it (E-7).
    4. **Homebrew: every path `is_under(<cellar>/<name>/<linked_keg>)`, component-wise; otherwise `ProviderOutputChanged`.** `keg_outcome` already has this rule. A path outside means the listing is not of that keg, and nothing legitimate produces it (0 of 148,951; `find` is rooted at the keg).
    5. **MacPorts: `is_under(<detected prefix>)` decides whether it is stat'd.** Not under: no `stat`; finding 7. Because of the legitimate roots (E-10), this is not `ProviderOutputChanged`. At least one entry must be under the prefix, or `NOTHING_UNDER_PREFIX` as today.
    6. Homebrew only: the list contains `<keg>/INSTALL_RECEIPT.json` (194 of 194), and at least one entry other than `.brew/*`, the receipt and the SBOM (E-5), after Homebrew's own `empty_installation?`. Absence is a failure.

    The path then reaches `lstat` and `stat` and nothing else: never opened, never in an argv, displayed only through `escape_hostile`. An intermediate symlink is followed by the kernel, and that yields only an existence answer.

18. **Sequence.** The check reads only after check 1 has established "active" or "linked". `port -q contents` lists image files for an inactive entry (R-21 E-3), which are not expected on disk, and `brew list --formula` lists the `opt` keg (E-4: equal to `linked_keg` in every case here). Casks stay unevaluable: `brew list --cask` is not on the allowlist (R-21 finding 5), and a cask cannot pass check 1 anyway.

## Cannot determine

- **UNRESOLVED: any port listing outside its prefix.** None installed. Needs a machine with an application or service port, and a recording of it. Finding 7's rule fails closed meanwhile.
- **UNRESOLVED: a port that alters its own registered files after activation** (a font, MIME or module cache, say). Would be a false failure in the safe direction. Needs a wider port set. All 18 here are plain command-line ports.
- **UNRESOLVED: the state immediately after a fresh install.** Everything here is long-installed. Whether a `post_install` leaves a symlink dangling at the moment verification runs was not observable, and no install may run here. Part of the 19 in E-6 are the candidates. Needs R-1's disposable prefix.
- **UNRESOLVED: a real deletion from a keg or a registered path.** Finding 1 and the `MISSING` class rest on source and semantics. Exercising them would mutate a provider; a scratch directory exercises only the adapter's stat logic.
- **UNRESOLVED: a non-UTF-8 name, and any non-ASCII name through `port`.** None exist here. Bare `tclsh8.6` printed U+FFFD for a non-BMP escape (E-11), which is not the same as a registry value. Both outcomes refuse at the parser, unless Tcl emits different valid UTF-8, which would be `MISSING` (safe direction).
- **UNRESOLVED: `brew list --formula` under the adapter's exact environment**, without `HOMEBREW_NO_INSTALL_FROM_API=1`. The sweep added the guard so as not to risk a download (R-15). `to_rack` returns for an installed name before any loader (`formulary.rb` 1164-1165), so no difference is expected, but it was not observed. Settled by capturing the recording part H needs.
- **Deferred to runtime detection:** `EACCES` under an unprivileged user; list sizes on other machines; `applications_dir`.
- **Not observed:** tap-qualified or renamed dependencies; kegs built from source (whether every one here came from a bottle was not tabulated); ARM; Homebrew's `find` failing part way.

## Impact on the specification

- **`docs/spec/04-migration-and-safety.md`, Verification contract, check 2 ("where the provider offers one") should change.** Neither provider offers a read-only one (finding 15). Suggested: "This tool's check of the provider's own records against the disk, as defined per provider; not a verdict by the provider and not content integrity." This is a Tier 3 MUST, so it needs a `docs/spec-evolution.md` entry. **Contradiction to report.**
- **R-21 finding 9's "symmetric … the provider's own record … satisfied on disk" does not hold for Homebrew** (finding 1), and neither does the same phrase in part H's framing. R-21 is left unedited. Whoever writes the SE entry should cite this note and record the declared asymmetry (finding 14), including the judgement that a Homebrew formula passes on the weaker basis (finding 15). The main agent may want the owner's eye on that one.
- **R-21's "registry per-file checksums" open item is settled for MacPorts 2.12.6:** the `files` table has no checksum column (E-9).
- **No allowlist growth** (`PERMITTED_SUBCOMMANDS`, `provider-commands.toml`): both list reads and the inventory are already allowed. Part P's staged forms are sufficient.
- **Fixtures:** no file in `fixtures/recordings/*.json` holds either list read (E-14). Part H needs a capture. This machine can produce one for both providers (`/capture-fixture`), but not one with a missing path, or an outside-prefix path (finding 7), so those tests cannot be recording-driven; stat logic needs a scratch directory.
- **`listing.rs`'s "nothing is stat-ed or followed"** stays true if the stat logic lives in a separate function, which keeps the parser pure and reusable.
- **`--json` and the catalogue:** a neutral `basis` field and failure classes `MISSING`, `DANGLING`, a missing dependency, plus unevaluable reasons including `LISTS_OUTSIDE_PREFIX`, need stable identifiers and keys in both locales.

## Re-check when

- Homebrew moves past `874c46a`, particularly `cmd/list.rb` (if the `find` ever becomes a stored manifest, finding 1 changes and the Homebrew check should get stronger), `missing_dependencies`, `Keg#empty_installation?`, the tab's fields, and `dev-cmd/verify.rb`.
- MacPorts base moves past 2.12.6 (sha256s in E-1), particularly `action_contents`, the `files` and `dependencies` schema, the mtree allowed roots, and `install_darwin_launchd`.
- A port with an outside-prefix entry, or a non-ASCII file name, is installed on any machine that can capture.
- Before implementing part H: re-run the sweep's two list forms and confirm the HEAD and sha256s.
