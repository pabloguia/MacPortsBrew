# R-26 — What `EXECUTABLES_OWNED_BY_DESTINATION` can read on each provider, and what it can and cannot pin

- **Status:** partially answered (answered for what this machine holds; an inactive MacPorts image, a `select`-managed link, a dangling registered link and a keg-only formula with a linked keg were not available and rest on source; see "Cannot determine")
- **Answered on:** 2026-11-04
- **Machine:** macOS 26.7.2 (25G332), x86_64, Homebrew 7.0.11-143-g874c46a98f @ `/usr/local` (`/usr/local/Homebrew` HEAD `874c46a98f1832f5c872c94f7d5a33503022c154`, committed 2026-11-01; `homebrew/core` revision `e045d7385fd`), MacPorts 2.12.6 @ `/opt/local`; 194 formulae, 18 ports (all active). Repository at `9b8d68d`.
- **Blocking:** work order 16 part H item 1 (`EXECUTABLES_OWNED_BY_DESTINATION`, `docs/spec/04-migration-and-safety.md` Verification contract check 3), §3 item 10 (the shim target's owner, part I), SE-107's L2

## Question

Check 3 says the expected executables exist at paths owned by the destination,
"confirmed by asking the provider which package owns the file, not by
string-matching the path". R-21 findings 10-11 name the reads. The implementation
needs to know, on both providers (INV-17):

1. What `port -q provides <prefix>/bin/<leaf>` answers for each kind of entry, how
   "not owned" differs from "another port", its cost, whether it is per image or
   active only, and whether it names a version.
2. How Homebrew's `Keg.for` rule applies to `<prefix>/bin/<leaf>`, keg-only formulae,
   several versions and `opt/` links, and whether the keg version is the `brew info`
   linked version. So: can the check pin "the version this run installed" (L2), or
   only name a package?
3. Which path the check asks about, and how that differs from `RESOLVES_TO`.
4. Whether this is one rule with two reads, and what it answers under `--replay`.
5. Metadata hygiene under §3 item 10.

## Evidence

All commands run 2026-11-04 as uid 501, unelevated, with only read-only subcommands.
Every `brew` call ran with
`HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_FROM_API=1 HOMEBREW_NO_ENV_HINTS=1`
exported. `port` is quieted by argv (`-q`). Only names from `brew info --json=v2
--installed` were passed to `brew`. Paths in one's home directory are written `~`.

### E-1 · Source identity

```
$ git -C /usr/local/Homebrew rev-parse HEAD
874c46a98f1832f5c872c94f7d5a33503022c154
$ git -C /usr/local/Homebrew --no-optional-locks status --porcelain -- Library/Homebrew/{keg.rb,formula.rb,utils/path.rb,cli/named_args.rb,formula_installer.rb,cmd/list.rb}
(no output; exit 0)
$ shasum -a 256 /opt/local/bin/port …/macports1.0/macports.tcl …/registry2.0/receipt_sqlite.tcl
e04d53dc06ec84cd718cb40f6ba8867b2fa22c20ef9dbfa9c8203a5d6a915e49  /opt/local/bin/port
aa1d25941f3eec0e35a4584471ea7a94471bcdd9a7550978e694fa9e528b2c32  /opt/local/libexec/macports/lib/macports1.0/macports.tcl
858dca027c7dfd8cacc4879a692ebfb929ad677f8393e7f61499fc39e49820bc  /opt/local/libexec/macports/lib/registry2.0/receipt_sqlite.tcl
```

Homebrew moved from R-21's 7.0.4 (`edb70f03`), so every Homebrew line below was
re-read at this HEAD. `port` and `macports.tcl` have R-21's hashes.

### E-2 · `port provides` source

`/opt/local/bin/port` `action_provides`, lines 2180-2215:

```
set file [file normalize $filename]
if {[file exists $file] || ![catch {file type $file}]} {
    if {![file isdirectory $file] || [file type $file] eq "link"} {
        set port [registry::file_registered $file]
        if { $port != 0 } {
            if {![macports::ui_isset ports_quiet]} { puts -nonewline "$file is provided by: " }
            puts $port
        } else { if {![macports::ui_isset ports_quiet]} { puts "$file is not provided by a MacPorts port." } }
    } else { if {![macports::ui_isset ports_quiet]} { puts "$file is a directory." } }
} else { if {![macports::ui_isset ports_quiet]} { puts "$file does not exist." } }
…
return 0
```

`registry2.0/receipt_sqlite.tcl` lines 137-146: `set port [registry::entry owner $file $cs]`,
then `[$port name]`, else `0`. `registry2.0/registry.tcl` lines 238-248 pass `cs`
from `fs_case_sensitive`, "assuming worst case (case-insensitive.)" on failure.

Cited, `src/cregistry/entry.c`, `reg_entry_owner`, tag v2.12.6, read 2026-11-04
(<https://raw.githubusercontent.com/macports/macports-base/v2.12.6/src/cregistry/entry.c>; the
installed copy is a compiled `registry.dylib`, so this is the tag matching
`port -q version`, read through a fetch tool that quotes extracts, not the whole file):

```
"SELECT id FROM registry.files %s WHERE (actual_path = ? %s) AND active"
```

with `COLLATE NOCASE` where `cs` is false. No row gives `*entry = NULL` and success.
`reg_entry_imagefiles` is `SELECT path FROM registry.files WHERE id=? ORDER BY path`, with no `active`.

### E-3 · `port -q provides`, one path, every case

```
$ port -q provides /opt/local/bin/pkg-config          [pkgconfig]  exit=0     (owned file)
$ port -q provides /opt/local/bin/xz                  [xz]         exit=0     (owned file)
$ port -q provides /opt/local/bin/lzcat               [xz]         exit=0     (registered symlink -> xz)
$ port -q provides /opt/local/bin/XZ                  [xz]         exit=0     (case variant; no such entry)
$ port -q provides /opt/local/bin/../bin/xz           [xz]         exit=0
$ port -q provides /opt/local/man/man1/xz.1.gz        [xz]         exit=0     (via the directory link /opt/local/man -> share/man)
$ port -q provides /opt/local/lib/liblzma.5.dylib     [xz]         exit=0
$ port -q provides /opt/local/bin/port                []           exit=0     (exists; MacPorts base, no port)
$ port -q provides /opt/local/bin/no-such-leaf-r25    []           exit=0     (missing)
$ port -q provides /opt/local/bin                     []           exit=0     (directory)
$ port -q provides /opt/local/man                     []           exit=0     (directory link, unregistered)
$ port -q provides /usr/bin/true                      []           exit=0     (outside the prefix)
$ port -q provides /usr/local/bin/brew                []           exit=0     (the other provider's)
$ port -q provides bin/xz                             []           exit=0     (relative)
$ port -q provides '/opt/local/bin/g['                []           exit=0     (a real leaf class, absent here)
```

`[…]` is stdout plus stderr verbatim. Without `-q`, for contrast:

```
$ port provides /opt/local/bin/xz /opt/local/bin/no-such-leaf-r25 /opt/local/bin /usr/bin/true
Warning: port definitions are more than two weeks old, consider updating them by running 'port selfupdate'.
/opt/local/bin/xz is provided by: xz
/opt/local/bin/no-such-leaf-r25 does not exist.
/opt/local/bin is a directory.
/usr/bin/true is not provided by a MacPorts port.
exit=0
```

```
$ /usr/bin/sqlite3 'file:/opt/local/var/macports/registry/registry.db?immutable=1' 'select count(*), sum(active) from files;' 'select state, count(*) from ports group by state;'
7123|7123
installed|18
$ … "select p.name,p.version,p.revision,p.variants,p.state,f.path,f.actual_path,f.active from files f join ports p using(id) where f.actual_path='/opt/local/bin/xz';"
xz|5.8.3|0||installed|/opt/local/bin/xz|/opt/local/bin/xz|1
$ port -q installed xz
  xz @5.8.3_0 (active)
exit=0
$ printf 'puts [file normalize /opt/local/bin/lzcat]\nputs [file normalize /opt/local/man/man1/xz.1.gz]\n' | /opt/local/bin/port-tclsh
/opt/local/bin/lzcat
/opt/local/share/man/man1/xz.1.gz
$ … "select p.name, f.actual_path, f.active from files f join ports p using(id) where f.actual_path='/opt/local/bin/lzcat';"
xz|/opt/local/bin/lzcat|1
```

### E-4 · The sweep (scripted; each `port` call separately timed)

Method: for every name from `port -q echo active`, `port -q contents <name>`, keeping
direct children of `/opt/local/{bin,sbin}`; union with a `scandir` of both directories;
one `port -q provides <path>` per path; and where `realpath` differs,
`port -q provides <realpath>`. For every formula from `brew info --json=v2 --installed`,
`brew list --formula <name>`, keeping `<cellar>/<rack>/<version>/{bin,sbin}/<leaf>`; a
`scandir` of `/usr/local/{bin,sbin}`; and this function over both, which is `Keg.for` (E-5):

```python
def keg_for(path):
    if not os.path.exists(path): return ("ENOENT", None, None)
    q = Path(os.path.realpath(path))
    while str(q) != "/":
        if str(q.parent.parent) == real_cellar: return ("KEG", q.parent.name, q.name)
        q = q.parent
    return ("NOT_A_KEG", None, None)
```

MacPorts results:

```
rows 91  listed by a port 84  on disk 91
provides t n=91 min=0.162 med=0.180 p95=0.366 max=0.735 (seconds)
exit codes {0: 91}  stderr non-empty 0
output lines {1: 84, 0: 7}
listed path answered by the listing port: 84, disagreeing: 0
symlinks 20  dangling 0  provides(link)==provides(realpath): 20 of 20
on disk, not listed, answer empty: daemondo, port, port-tclsh, portf (link -> port), portindex, portmirror, sbin/.turd_MacPorts
listed entries in sbin: none   a leaf in both bin and sbin: none
real path outside /opt/local: none
$ ls -la /opt/local/etc/select
ls: /opt/local/etc/select: No such file or directory
```

Homebrew results over `/usr/local/{bin,sbin}`:

```
entries 1158  links 1119
Keg.for: KEG 1089, NOT_A_KEG 69
KEG rows: keg==linked_keg 1089, keg!=linked_keg 0, keg not in installed[] 0, rack not a formula name 0
NOT_A_KEG rows: regular file 39, link into Caskroom 8, brew itself 1, other link 19, link into a keg whose real path leaves it 2
```

Homebrew results per formula, from `brew list --formula <name>` (194 calls, exit 0 for all,
median 1.05 s, max 3.19 s):

```
formulae with a bin/sbin entry 146   entries 1488 (all executable; 37 in sbin)   entries that are symlinks 236
list path's own Keg.for == (this formula, list version): 1486   other: 2 (node's npm, npx)
not keg-only: prefix entry's Keg.for == (this formula, linked_keg): 1089   other: 2 (npm, npx)   absent: 0
keg-only: 19 formulae, 17 with entries; prefix entry absent 396, present 1 (openssl@4's openssl, Keg.for = openssl@3 3.6.4)
keg_only true with linked_keg set: none
keg_only false with linked_keg null: pydantic (no bin/sbin entry)
linked_keg with a _N revision suffix: 12 (argon2 20190702_1, ffmpeg 9.0.1_1, …)
<prefix>/opt/<name>, var/homebrew/linked/<name>, linked_keg disagreeing: none
```

The 13 formulae holding two kegs (`on disk`, `list` = version in `brew list --formula`):

```
name          on disk                   list       linked_keg  opt        keg_only
harfbuzz      14.3.1 14.4.0             14.4.0     14.4.0      14.4.0     false
mise          2026.8.12 2026.9.0        2026.9.0   2026.9.0    2026.9.0   false
openssl@3     3.6.3 3.6.4               3.6.4      3.6.4       3.6.4      false
openssl@4     4.0.1 4.0.2               4.0.2      null        4.0.2      true
pcre2         10.47_1 10.48             10.48      10.48       10.48      false
php@8.4       8.4.24 8.4.25             8.4.25     null        8.4.25     true
pipx          1.16.7 1.17.1             1.17.1     1.17.1      1.17.1     false
pkgconf       3.0.6 3.0.7               3.0.7      3.0.7       3.0.7      false
```
(cryptography, libffi, pydantic, simdjson, tree-sitter hold two kegs with no bin/sbin entry; libffi and pydantic have `linked_keg` null, opt at the newer.)

### E-5 · Homebrew source at this HEAD (`Library/Homebrew/`)

`keg.rb` 120-133, `Keg.for`:

```ruby
def self.for(path)
  original_path = path
  raise Errno::ENOENT, original_path.to_s unless original_path.exist?
  real_cellar = HOMEBREW_CELLAR.realpath
  if (path = original_path.realpath)
    until path.root?
      return Keg.new(HOMEBREW_CELLAR/path.relative_path_from(real_cellar)) if path.parent.parent == real_cellar
      path = path.parent.realpath # realpath() prevents root? failing
    end
  end
  raise NotAKegError, "#{original_path} is not inside a keg"
end
```

- `keg.rb` 210-217, `Keg#initialize`: resolves one hop if the path starts with `#{HOMEBREW_PREFIX}/opt/`; refuses unless `path.parent.parent.realpath == HOMEBREW_CELLAR.realpath`; `@name = path.parent.basename`.
- `keg.rb` 466-468: `def version = PkgVersion.parse(path.basename.to_s)`. `pkg_version.rb`: `REGEX = /\A(.+?)(?:_(\d+))?\z/`; `to_str` appends `_N` only if the revision is positive.
- `formula.rb` 1011-1015 `linked_keg` is `HOMEBREW_LINKED_KEGS/name` over `possible_names`; 1104 `linked? = linked_keg.exist?`; 1112-1116 `linked_version` is `Keg.for(linked_keg).version`; 3182 `"linked_keg" => linked_version&.to_s`. `startup/config.rb` 46: `HOMEBREW_LINKED_KEGS = HOMEBREW_PREFIX/"var/homebrew/linked"`.
- A second rule, which decides what `brew unlink` removes. `keg.rb` 369-380, `Keg#unlink`: for `src` in the keg, `dst = HOMEBREW_PREFIX + relative_path_from(src, path)`, then `next if src != resolved_path(dst)`, the comment reading "check whether the file to be unlinked is from the current keg first". `utils/path.rb` 84-86: `resolved_path(path) = path.symlink? ? path.dirname.join(path.readlink) : path`, one hop.
- `cli/named_args.rb` 566-577, `resolve_default_keg`: `opt/<rack>` first, then `var/homebrew/linked/<rack>`, then the only keg. So `brew list --formula` prefers `opt`.
- `formula_installer.rb` 129: `link_keg ||= !formula.keg_only? || auto_link_versioned_keg_only?`; 1984-1995: a requested, versioned keg-only formula with no installed version and no conflicting relative is linked.

### E-6 · Individual Homebrew cases

```
$ ls -l /usr/local/bin/openssl
/usr/local/bin/openssl -> ../Cellar/openssl@3/3.6.4/bin/openssl
$ realpath /usr/local/bin/openssl /usr/local/opt/openssl@4/bin/openssl
/usr/local/Cellar/openssl@3/3.6.4/bin/openssl
/usr/local/Cellar/openssl@4/4.0.2/bin/openssl
$ ls -l /usr/local/opt/openssl@4 /usr/local/var/homebrew/linked/openssl@4
ls: /usr/local/var/homebrew/linked/openssl@4: No such file or directory
/usr/local/opt/openssl@4 -> ../Cellar/openssl@4/4.0.2
$ ls -l /usr/local/opt/pcre2 /usr/local/var/homebrew/linked/pcre2
/usr/local/opt/pcre2 -> ../Cellar/pcre2/10.48
/usr/local/var/homebrew/linked/pcre2 -> ../../../Cellar/pcre2/10.48
$ ls -l /usr/local/bin/npm /usr/local/Cellar/node/26.7.0/bin/npm
/usr/local/bin/npm -> ../Cellar/node/26.7.0/bin/npm
/usr/local/Cellar/node/26.7.0/bin/npm -> /usr/local/lib/node_modules/npm/bin/npm-cli.js
$ realpath /usr/local/bin/npm
/usr/local/lib/node_modules/npm/bin/npm-cli.js
$ brew list --versions openssl@4
openssl@4 4.0.1 4.0.2
$ brew info --json=v2 --installed  (name, keg_only, linked_keg, installed[].version)
node False 26.7.0 ['26.7.0']
openssl@3 False 3.6.4 ['3.6.3', '3.6.4']
openssl@4 True None ['4.0.1', '4.0.2']
pcre2 False 10.48 ['10.47_1', '10.48']
pydantic False None ['2.13.4', '2.13.5']
$ realpath /opt/local/bin/XZ
/opt/local/bin/xz
$ time brew --cellar
/usr/local/Cellar
… 0.069 total
```

One-hop rule over all 1119 links, `normpath(join(dirname(p), readlink(p)))`: into the Cellar
1091, each one a path some `brew list --formula` printed; elsewhere 28. Of the 1091, `Keg.for`
disagrees on exactly `npm` and `npx`. No link has `Keg.for` = KEG with its first hop outside the Cellar.

### E-7 · `port select`'s links, source only

`macports.tcl`, `proc mportselect` from line 5962: `conf_path` is `${prefix}/etc/select/$group`;
the `set` branch does `file delete $tgt` and `file link -symbolic $tgt $src` (lines 6036-6038,
6044-6046) with no `registry::` call in the procedure. No group exists on this machine (E-4).

### E-8 · This repository, at `9b8d68d`

- `crates/domain/src/provider.rs:393` `fn resolution(&self, executable: &Path) -> Result<Resolution, DomainError>`.
- `crates/adapters/src/macports/mod.rs:973-1004` canonicalises under `FilesystemScope::ThisMachine`, then `provides(target)` on the **canonical** path.
- `macports/mod.rs:573-584`, `provides`: returns `None` for a safety refusal, a runner error, a non-zero exit and an empty answer alike.
- `macports/parse.rs:386-403`, `parse_provides`: first line that is a safe name or carries `is provided by:`.
- `crates/adapters/src/homebrew/mod.rs:1181-1201` canonicalises under `ThisMachine`; `authoritative: canonical.is_some() && …`. `:1418-1445` `owner_of`: for `root` in **Cellar and Caskroom**, the first component after `root` is the package; no version component is read.
- `crates/adapters/src/filesystem.rs:50`, `FilesystemScope::{ThisMachine, Elsewhere}`.
- `crates/application/src/executor.rs:1137` `judge_resolution`: `NotObserved` is `Unevaluable::ResolutionNotObserved`. `:1442-1480` and `destination_before` at `:1495`: `ProviderReportsActive`/`PackageNewlyPresent` judged from `destination_before`.
- `crates/adapters/src/homebrew/mod.rs:694-718`: a snapshot `InstalledPackage` per keg, with `version`, `active`, `install_path`.
- `docs/spec-evolution.md` SE-107: "under `--replay` a certifying read halts before it is made (C1)"; L2: "'some version active' is not 'the version this run installed'; the plan carries no version."
- `docs/passes/16-plans/F.md` Design §4: `VERIFY_DESTINATION` carries `ExecutablesOwnedByDestination { executables }`, leaf names only.

### E-9 · Nothing was written

```
$ stat -f '%Sm %z %N' /opt/local/var/macports/registry/registry.db*   (before and after the sweep: no diff)
$ find /usr/local/var/homebrew /usr/local/Cellar /usr/local/opt /opt/local/var/macports/registry -maxdepth 2 -newer <marker>
(no output)
$ find ~/Library/Caches/Homebrew -maxdepth 1 -newer <marker>
(no output)
$ git -C /usr/local/Homebrew rev-parse HEAD     874c46a98f… (unchanged)
```

Depth-limited, as in R-21 E-16. The direct `sqlite3` reads used `immutable=1`; they are
this note's observation, not a read the product should make.

## Findings

**1 · MacPorts**

1. **The registry answers for the entry as registered, symlink or not.** A registered
   link has its own row, and `file normalize` keeps a final link while resolving
   intermediate ones, so the link itself is what is looked up (E-3 `lzcat`, its row and
   `port-tclsh`), and a path through a directory link still answers (E-3 `/opt/local/man/…`).
   20 of 20 links answered the same port as their real path (E-4), so no link here
   tells the two apart by answer. All 84 listed entries answered
   their listing port (E-4).
2. **A `select`-managed link is expected to answer empty, exit 0.** This is inference
   from source: `mportselect` creates it with no registry call (E-7), and the query matches
   `actual_path` only (E-2). **Not observed**: no group exists here. The nearest observed
   analogue is `portf`, an unregistered link in `bin` that answers empty even though
   its target `port` is also unowned. The current `resolution` would differ, since it asks
   about the canonical path (E-8) and so names the selected port.
3. **Exit code never distinguishes anything: 0 in all 91 sweep calls and all 15 probes.**
   Under `-q` the output has two shapes:
   - one line, a port name: that port's active image owns the path;
   - empty: not owned, missing, directory, directory link, relative, or outside the prefix (E-2, E-3).

   **"Another port" is a non-empty answer that differs from the expected name.** "Not
   owned" is an empty answer. The empty answer cannot say why; `-q` is what removes the
   reason (E-3, no `-q`). A check that needs "missing" versus "unowned" gets it from a
   filesystem read, not from `port`. A non-zero exit means the read failed and is neither
   of the two; which failures reach it was not observed.
4. **Active only.** `AND active` in the query (E-2, cited). An inactive image's files
   have `active = 0`, so no row matches and the answer is empty, per source. Not observed:
   7123 of 7123 rows are active here (E-3).
5. **It names no version.** `[$port name]` (E-2). The version comes from a second read:
   the `(active)` row of `port -q installed <name>`, `@5.8.3_0` (E-3). Because the
   owner query is active only, the two compose: the owner is the active image, and the
   active row is its version, within the window between the two reads.
6. **Cost: median 0.180 s per call, p95 0.366 s, max 0.735 s** (n=91). `xz` with 23
   executables is about 4 s. One call per path, as R-21 finding 10 requires. Sequential.
7. **Case-insensitive.** `/opt/local/bin/XZ` answers `xz` on this filesystem (E-3), through
   `COLLATE NOCASE` (E-2). Asking about a validated name from the list avoids this
   mattering, and the check must compare leaf names exactly and never take case
   from the answer.

**2 · Homebrew**

8. **`Keg.for` by source (E-5):** existence (ENOENT otherwise) → `realpath` of the path →
   walk up until `parent.parent == <cellar>.realpath` → name is the rack's basename →
   version is the keg directory's basename (`PkgVersion.parse`). Otherwise `NotAKegError`.
   It is not a prefix match. It needs the cellar real path, and the real path of each entry.
9. **The keg version is the version `brew info` reports as linked, in every case that
   can be tested here.** 1089 of 1089 entries; 12 formulae here have a `linked_keg` with a `_N`
   suffix (E-4). By source they are the same computation: `linked_keg` is `Keg.for(<linked>).version`
   (E-5). A keg directory literally named `<v>_0` would parse to a `to_s` of `<v>`, so
   string-comparing it to the directory name would fail, closed. None exists here.
10. **Multiple versions:** 13 formulae hold two kegs (E-4). The prefix link, `opt`, and
    `linked_keg` all named the same keg in all of them that are linked. So the
    keg from `Keg.for` is the disambiguator, and a name check alone would pass while
    a stale keg's link remained; a version comparison is possible.
11. **`opt/` differs from `linked` for keg-only formulae.** `opt` is present for all 194,
    `var/homebrew/linked/<name>` only when linked (E-4, E-6). Where both exist they agreed.
12. **Keg-only: the executable is not at `<prefix>/bin`.** 396 of 397 entries are simply
    absent there. **The 397th is the case that matters: `openssl@4` ships `bin/openssl`,
    and `/usr/local/bin/openssl` exists and belongs to `openssl@3`** (E-6). A check of
    "the leaf exists under the prefix" would pass for the wrong package. By the rule, it
    answers `openssl@3`: "another package".
13. **What "expected" means for a keg-only formula, compared with `linked_keg`.** Branch
    on `linked_keg`, never on `keg_only`: `pydantic` is not keg-only and unlinked, and
    source says a keg-only versioned formula can be auto-linked (E-4, E-5).
    - `linked_keg` set: expected is `<prefix>/{bin,sbin}/<leaf>`.
    - `linked_keg` null: **check 1 already fails** ("an unlinked formula (keg-only ones
      included) does not pass"), and F refuses `PRIMARY_NEEDS_SHIM` at planning. So check 3
      should not be reached with a pass available. It must report not placed, not
      evaluate some other path to a pass.
    - Part I's shim target (§3 item 10) is the one consumer of the other form:
      `<prefix>/opt/<name>/{bin,sbin}/<leaf>`, which `Keg#initialize` itself treats as a
      route to a keg (E-5). There the owning version is `opt`'s, and there is no `linked_keg` to
      compare with, so the only version evidence is the `opt` link (E-4: no disagreement
      with `brew list --formula`, which prefers `opt`).
14. **`Keg.for` has a false negative on a real formula's own executables.**
    `/usr/local/bin/npm` is a link Homebrew made to `Cellar/node/26.7.0/bin/npm`, which
    is itself a link out of the keg into `/usr/local/lib/node_modules/npm` (E-6). `Keg.for`
    says `NotAKegError`. `Keg#unlink`'s one-hop rule says the link is the keg's (E-5). So
    **Homebrew has two rules that disagree** on 2 of 1091 links here, and R-21 E-14
    described only one. Choosing:
    - One hop alone would accept `Cellar/x/1/bin/foo -> /anywhere`, as the second hop
      is what the probe would run; it cannot stand alone.
    - Realpath alone is what `resolution` already answers: `npm` is `External` today (E-8),
      so `VERIFY_RESOLUTION` would already halt it.
    - **Recommendation: require the realpath rule (Keg.for) and report the
      one-hop-only case with its own reason**, not a generic "not owned". The source is kept
      (INV-03), and that loses only automation for that package. Allowing the second hop
      for one curated package would be a Tier 4 decision on evidence, not a default.
15. **`owner_of` is not yet Keg.for** (E-8). It reads no version; and it also accepts the
    Caskroom, returning the cask's token as `package`, so a formula and a cask of the same
    name would look alike. No such collision was tested. Both matter to H (finding 19,
    "Impact").

**L2 · Which version**

16. **Neither provider attributes a version to a run, and the plan carries none (SE-107
    L2). The check can name a package and a version; it cannot prove causation.**
    - Homebrew: the owner read yields `(name, version)` in one step (findings 8, 9).
    - MacPorts: `provides` yields the name; the version needs a second read (finding 5).

    So both providers can pin **"the version the provider holds active, and it owns this
    path"**, and the rule pins the version with one read on Homebrew, two on MacPorts.
17. **"The version this run installed" is derivable where the plan installs, from the paired
    snapshot, on both providers, and only as an inference.** A step carrying
    `PACKAGE_NEWLY_PRESENT` means the snapshot held no such package (E-8; `Before::Absent`),
    so the run-time set of versions must be exactly one element `{v}`, the active record
    must be `v`, and the owner's version must be `v`. More than one version is a halt: it
    is exactly "some version active". What this establishes is "the only version present,
    and it is the one that owns the executables", not that this run's process wrote it: a
    concurrent install of the same package is indistinguishable (finding 23). For a
    package already held, the planner writes no `PACKAGE_NEWLY_PRESENT` and this run
    installs nothing (F-06), so no version to pin arises. Comparing the active version to the
    snapshot's is the available guard there.

    This does not need a version in the plan. It would need the version rows the check
    reads to be compared in each adapter's own shape (Homebrew's `version` carries `_N`;
    `port -q installed` renders `@v_r+variants`), never across providers.

**3 · Which path; location against resolution**

18. **The check asks about the entry the provider placed: `<detected prefix>/{bin,sbin}/<leaf>`,
    on both, and never the list path.**
    - MacPorts: the two are the same path (84 of 84, E-4), and only the registered path
      can be asked.
    - Homebrew: the list path is `<cellar>/<rack>/<version>/bin/<leaf>`, which `brew list
      --formula` prints for an unlinked keg too. Asking about it proves "the keg ships
      it", and passes for `openssl@4` while `<prefix>/bin/openssl` is `openssl@3`'s (E-4, E-6).
      `DESTINATION_LACKS_EXECUTABLE` already answers "ships it" at planning.
    - The plan carries leaves, not directories (E-8), so the check has to find the directory.
      A leaf in both `bin` and `sbin` of one package did not occur (1488 + 84 entries, E-4).
      Rule: `bin`, then `sbin`, the first entry that exists on disk is the one asked about.
19. **Location is not resolution; do not merge them, but share the primitive.**
    - `RESOLVES_TO` on `VERIFY_RESOLUTION` asks: *searching `PATH`, which file wins for this
      name, and whose is it?* It consults `PATH`, follows to the final file, and asks the
      owner of the final file (E-8). It runs after `SET_PRIMARY`.
    - `EXECUTABLES_OWNED_BY_DESTINATION` on `VERIFY_DESTINATION` asks: *does the file the
      destination placed exist and belong to the destination package?* No `PATH` is
      consulted. It holds when another provider wins `PATH`, and it runs before `SET_PRIMARY`.
    - The owner lookup is the shared part. `ProviderRead::resolution(<path>)` already takes a
      path, not a name (E-8), so a second owner mechanism is not needed, but it needs three
      additions: a version, the entry-versus-final distinction (finding 20), and
      a failed read kept apart from "not owned" (`provides` conflates them, E-8; AGENTS.md:
      an unknown is never a silent no). Whichever method ends up exposing these, `judge_*`
      for the two checks stay separate, as they answer different questions.

**4 · INV-17 and replay**

20. **One rule, two reads, and the asymmetry stated plainly.** The rule is: *the entry
    `<prefix>/{bin,sbin}/<leaf>` exists, the provider's record names the plan's package as its
    owner, and the file it finally resolves to is the same package's.*

    | | MacPorts | Homebrew |
    |---|---|---|
    | Whose record | the registry, a database of files per port | the Cellar, a directory layout per keg (no "read" of any provider process except `brew --cellar`) |
    | Is it a question to the provider? | **yes**, `port -q provides` | **no**, the adapter applies Homebrew's rule (R-21 finding 11; E-5) |
    | Owner of the entry | `provides(entry)` | first hop in `<cellar>/<name>/<version>/` |
    | Owner of the final file | `provides(realpath)` when the entry is a link | `Keg.for`: realpath in the same keg |
    | Version | second read, the `(active)` row | in the same answer, the keg directory |
    | Existence | a filesystem read: `provides` cannot be used, as empty means several things and a dangling registered link passes `file type` (E-2) | a filesystem read (`Keg.for` raises ENOENT) |
    | Cost | ~0.18 s per path, ×2 for links | one `brew --cellar` (0.07 s) plus in-process `realpath` |
    | Agreed on this machine | 20 of 20 links: entry owner = final owner | 1089 of 1091: first-hop owner = final owner |

    **MacPorts is an actual owner query; Homebrew's is not, and no wording makes it one.**
    What is symmetric is that each answer is the provider's own record of ownership, that
    each needs a filesystem read for existence and the final path, and that the outcome is
    judged alike. Only the read differs, and it stays in the adapter. Neither direction is a
    special case above the adapter (INV-17).
21. **Under `--replay` the answer is unevaluable on both, never a pass, and for the same reason.**
    - Homebrew: there is no invocation to replay except `brew --cellar`. Under
      `FilesystemScope::Elsewhere` the adapter does not read this host's disk (E-8;
      `authoritative` is already false there). An answer computed from the path as written
      is a string match, the one thing check 3 forbids.
    - MacPorts: `port -q provides` could be replayed from a recording. That alone does not
      make it evaluable, since existence and the final path are filesystem reads (finding 20),
      and SE-107 C1 already halts a certifying read before it is made under `--replay`.

    So `judge_*` gets a not-observed answer from both adapters and halts as an `Unevaluable`,
    the shape `ResolutionNotObserved` already has (E-8). The Homebrew rule itself can
    still be tested purely: a function of `(real path, real cellar)` strings, plus a
    `ThisMachine` scratch-tree contract test as `filesystem.rs` describes. A recording's
    `brew list --formula` / `port -q contents` output is replayable evidence that the
    package *ships* a leaf, not that it owns a placed path; it does not answer this check.
    A note on the citation: INV-15's text (`01-invariants.md` line 87-90) is about byte-identical
    output for one snapshot; "a replayed reader never consults the live filesystem" is the
    test `invariant_15_a_replayed_reader_never_consults_the_live_filesystem` and
    `FilesystemScope`'s rule, derived from it.

**5 · Metadata hygiene (§3 item 10)**

22. **Names and paths.**
    - **Leaf class:** `ExecutableName::parse` (`crates/domain/src/executable.rs:89`, F-2a): one
      component, no `/`, not `.`/`..`, no leading `-`, no hostile-class character. Over all
      1572 listed entries here, the only leaf outside `[A-Za-z0-9._+@-]` was `g[`,
      which the class admits and which needs nothing special as an argv
      element (E-3, sweep). A test should keep `g[` and `clang++` valid.
    - **Path construction:** detected prefix + literal `bin`|`sbin` + validated leaf,
      with the prefix `brew --prefix` or the configured detection and no provider text. Neither
      `port`'s nor `brew`'s path text, nor `readlink` output, ever becomes an argument
      except the `realpath` result passed back into `provides`, and that only after
      `is_safe_path` and a component-wise check that it is under the canonicalised prefix.
      Comparison is by component, not by string prefix (`/opt/local2` is not under `/opt/local`).
    - **Answers are compared, not used.** The port name, rack and keg component are compared to
      the plan's package and never joined to a path.
    - **Parsing:** for this check, exactly zero or one line. `parse_provides` accepts the
      first safe line of any output (E-8), which is laxer than what is needed; a second line,
      or a line that isn't a safe name, should be `ProviderOutputChanged`. An adapter
      already binds `-q provides` with one path by call; the spawn site still allows several (R-21 finding 10).
    - **Symlinks:** follow only to read (`canonicalize`, no open); a final path outside the
      detected prefix is not owned. A dangling entry fails the existence read on both.
      Under the same uid nothing here executes or opens the file; the liveness probe does.
23. **TOCTOU: the check is evidence of a moment, and its wording must say that.** Between the
    read and any later use, the entry can change: Homebrew's links here are owned by this account
    (`ls -l`: `pirado  admin`), so the same uid can replace them, and MacPorts' can be changed by whoever may write its prefix. Nothing here
    can close that window; `canonicalize` then `provides` has its own small gap, and the
    MacPorts name and version are two reads. Suggested wording: **"owned by `<package>`
    when read at `<time>`"**, never "is owned". Consequences for the design: check 3 is not a
    security boundary against a local process of the same uid; the removal re-read
    (SE-107 C1, `reverify_for`) should include check 3, so the last evidence precedes
    `REMOVE_SOURCE` directly; and the probe must run the resolved path and report what it ran.
    On a read changing between calls (an empty answer after a non-empty one), the outcome is a halt.

## Cannot determine

- **`port -q provides` on a `select`-managed link.** Inference: empty, exit 0 (finding 2, E-7).
  Needs a machine with a populated `${prefix}/etc/select/<group>` and a selected version. No
  installs here to create one.
- **On an inactive image's file.** Cited: empty, by `AND active` (E-2). Needs a port with two
  images; R-20 and R-21 have the same gap. Whichever is active still answers for a shared path.
- **On a dangling registered link.** By source, `file type` succeeds, so the registry would
  answer the port (E-2). None here (0 dangling, E-4). The design takes no risk on it: existence
  is read separately (finding 20).
- **Which failures make `port -q provides` exit non-zero.** A locked or unreadable registry was
  not observed (exit 0 in all calls). The adapter should treat non-zero as a failed read.
  It takes a registry fault, which cannot be arranged read-only.
- **A keg-only versioned formula that auto-linked** (E-5 lines 129, 1984-1995). None here; the
  rule branches on `linked_keg`, so it treats one as linked. No observation of its JSON.
- **A renamed formula whose rack still has the old name.** `linked_keg` uses `possible_names`
  (E-5), and 10+ formulae here carry `oldnames`, but every rack here matched its name (E-4).
  Comparing the rack to the plan's package would refuse such a keg, which is the safe direction.
- **A prefix or Cellar that is itself a symlink.** Not on this machine. `Keg.for` handles it with
  the cellar's realpath (E-5); `file normalize` would resolve parents (E-2). The MacPorts adapter's
  `target.starts_with(&self.prefix)` compares a canonical path to an uncanonicalised prefix (E-8);
  Homebrew's canonicalises both. Worth a test in H.
- **Apple Silicon.** No machine here. The rule depends on `brew --cellar`, not `/usr/local`, so
  it is runtime detection, and this note's counts are one machine's.

## Impact on the specification

- **`docs/spec/04-migration-and-safety.md` line 109 ("replacing that rule with an owner query
  is part H's") cannot be done, and `docs/passes/16-plans/F.md` line 197 says the same.** No Homebrew
  read answers ownership at this HEAD either (R-21 E-14 stands; `Keg.for` is the rule). The sentence
  should say H keeps the rule, read in the adapter. Check 3's own "by asking the provider which
  package owns the file" (line 55) is likewise literally true for MacPorts only, and
  the existing MUST at lines 106-109 already says so for `VERIFY_RESOLUTION`; check 3 should
  carry the same sentence. Material Tier 3 wording: an SE entry is the main agent's call.
- **Check 3 needs a third clause:** the entry *and* its final file. "The path's real location
  inside a keg" (04, line 108) matches `Keg.for`, and has the `npm` false negative (finding 14).
  State that as the accepted cost, or decide otherwise. It already costs at `VERIFY_RESOLUTION` today.
- **L2 (SE-107):** answered as findings 16-17. No plan field is needed; the pin comes from the
  paired snapshot and the provider's rows, and states what it establishes.
- **R-21 finding 11 is incomplete rather than wrong:** `Keg#unlink` is a second Homebrew rule
  (finding 14). R-21's Re-check also names Homebrew past 7.0.4, which has now happened; only
  `keg.rb`'s rule was re-read here.
- **`fixtures/recordings/macports-only-intel-macos12.json`'s provenance** calls `-q provides <path>`'s
  rendering UNVERIFIED. E-3 and E-4 verify the bare-name form on MacPorts 2.12.6. The fixture stays
  synthesised; only that clause could be updated.
- **Code the implementation has to change, not in the specification:** `provides` conflates a
  failed read with "not owned"; `parse_provides` is lax; `owner_of` reads no version and includes the
  Caskroom (E-8; findings 15, 19, 22). The allowlist needs **no growth**: `port -q provides`,
  `port -q installed`, `brew --cellar`, `brew info --json=v2 --installed` are all permitted.
  The MacPorts tail rule still allows several paths; `ARGV_PROVIDES` narrowing was already handed on
  (R-21 finding 10, F.md `SG2-N1`).
- **A check on a package with no provider-owned executable is out of this note:** `NoExecutables`
  has an empty list and F's gate holds it.
- No Tier 1 statement is contradicted.

## Re-check when

- Homebrew moves past `874c46a9`, particularly `keg.rb` (`Keg.for`, `unlink`, `initialize`),
  `formula.rb` `linked_keg`/`linked_version`, `pkg_version.rb`, `cli/named_args.rb`
  `resolve_default_keg`, or adds any "which formula owns this path" command, which would
  change finding 20's table and supersede line 109 of spec 04 for real.
- MacPorts base moves past 2.12.6, particularly `action_provides`, `file_registered`,
  `reg_entry_owner`'s `AND active`, or `mportselect` beginning to register its links.
- `node` stops linking `npm` out of its keg, or a further formula class starts (finding 14).
- A machine with an inactive image, a `select` group or an auto-linked keg-only formula
  becomes available: settle those three "Cannot determine" items by running E-3's forms.
- Part I implements the shim target: re-read finding 13 against what it builds.
