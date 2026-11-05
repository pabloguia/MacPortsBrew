# R-27 — What `NO_CROSS_PROVIDER_LINKAGE` can read, examine and honestly claim, and whether it needs `otool` or an in-process Mach-O reader

- **Status:** partially answered (sweep, formats, cache and comparison answered; the items under "Cannot determine" are UNRESOLVED)
- **Answered on:** 2026-11-04
- **Machine:** macOS 26.7.2 (25G332), x86_64, Homebrew 7.0.11-143-g874c46a @ `/usr/local` (`/usr/local/Homebrew` HEAD `874c46a98f1832f5c872c94f7d5a33503022c154`), MacPorts 2.12.6 @ `/opt/local`; Xcode 26.5 (17F42) selected (`xcode-select -p` = `/Applications/Xcode.app/Contents/Developer`), Command Line Tools 26.6 also installed, `otool` cctools-1040 / LLVM 21.0.0; repository at `9b8d68d`
- **Blocking:** work order 16 part H item 1 (`NO_CROSS_PROVIDER_LINKAGE`, verification check 5 in `docs/spec/04-migration-and-safety.md`), and the owner's choice between a staged `otool` program entry and an ADR-0004 decision (R-21 Impact)

Listed in `docs/spec/09-research-checklist.md` under R-25 to R-28; work order 16 part H item 1 asks for one note per check.

## Question

R-21 refused `brew linkage` and MacPorts' verbs and found that `otool -L`/`-l`
answers the question but is not a permitted program. The allowlist is frozen
(work order 16 §3 items 3 and 5). So:

1. What does a sweep of every Mach-O of every installed port and formula find, and at what cost?
2. Which files must the check examine, and what can it honestly claim (INV-07, INV-08, "new" linkage in spec 04)?
3. What are `otool`'s exact formats and exit codes?
4. What does the `/usr/bin/otool` shim need, what can it do on a machine with no developer directory, and is R-21's cache question settled?
5. In-process reader or program entry?

## How the sweep ran

Scripts and raw outputs lived in the session scratch directory, not the repository.

- **File lists.** MacPorts: `port -q echo active` (18 ports; `port -q installed` showed no inactive image), then `port -q contents <port>` per port. Homebrew: a read-only directory walk of `/usr/local/Cellar/<rack>/<version>/`, every keg (194 racks, 207 kegs), keeping symlinks and not directories. `brew` was not run per formula. Checked for one keg: `brew list --formula pkgconf` and the walk gave the same 35 paths, `diff` empty.
- **Quieting.** `brew` calls ran with `HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_FROM_API=1 HOMEBREW_NO_ENV_HINTS=1`. `port` is quieted by `-q`.
- **Scope.** "In scope" means `bin/`, `sbin/`, `lib/` of the prefix or keg, as asked. Everything else is counted as "outside" and read in-process too.
- **Classification.** By the file's first bytes, not its name or mode. Every Mach-O was parsed by a 90-line Python prototype (fat, thin 32/64, either byte order, load commands, bounds).
- **`otool`.** `/usr/bin/otool -L <path>` and `-l <path>`, plus `-arch all -L` for fat files, one spawn at a time, with exactly the sanitised environment `sanitised_environment` builds for an empty spec (`PATH=/usr/bin:/bin:/usr/sbin:/sbin LANG=C LC_ALL=C TERM=dumb NO_COLOR=1`, no `HOME`, no `TMPDIR`), `cwd=/`, stdin null, as `crates/adapters/src/process/system.rs` lines 252-272 spawn.
- **Crafted specimens.** Byte-patched copies of real files and small assembled files in the scratch directory. They are data only, read by `otool` and the prototype and never executed. Every observation from one is marked **crafted**. None is a fixture.

## Evidence

All commands run 2026-11-04 as uid 501, unelevated.

### E-1 · Side effects of this research (read this first)

**A Command Line Tools install dialog was raised on this machine by one of these tests.**

```
## SDKROOT bogus rc=72 s=1.394 db_changed=True ('03:02:48', 12902, '247d98e393e3') ('03:02:49', 13245, '33068ef5dc2c')
  err: otool: error: Failed to open property list '/nonexistent/SDKSettings.plist' | otool: error: Failed to determine realpath of '/nonexistent' (errno=No such file or directory) | otool: error: sh -c '/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild -sdk /nonexistent -find otool 2> /dev/null' failed with exit code 16384: (null) (errno=No such file or directory) | xcode-select: Failed to lo
$ ps -o pid,ppid,user,lstart,etime,command -p 73462
73462     1 pirado Wed Nov  4 03:02:50 2026       00:15 /System/Library/CoreServices/Install Command Line Developer Tools.app/Contents/MacOS/Install Command Line Developer Tools
```

The test was `env -i` plus the sanitised variables plus `SDKROOT=/nonexistent`, stdin null, `cwd=/`, run from a script. The `err:` line is cut at 400 characters by that script, and the test was deliberately not repeated. The process started about a second after the test, with no other cause known. That is attribution by timing and by the stderr text, not proof. It was still running when this note was written; nothing was terminated, because this research is read-only, and nothing is installed unless someone clicks Install.

Also written, outside both providers: `xcrun_db` in the user's temp directory (E-12), and this research's own files in the session scratch directory.

`find` over `/usr/local/var/homebrew`, `/opt/local/var/macports/registry` (depth 2) and `~/Library/Caches/Homebrew` (depth 1) newer than a marker printed nothing.

### E-2 · The sweep's counts (`files.jsonl`, 173,209 records, 212 packages)

| | Homebrew in scope | MacPorts in scope | Homebrew outside | MacPorts outside |
|---|---|---|---|---|
| Mach-O, thin | 1695 | 56 | 460 | 5 |
| Mach-O, fat | 8 | 0 | 0 | 0 |
| Fat wrapper around `ar` archives | 15 | 0 | 0 | 0 |
| `ar` archive | 956 | 16 | 8 | 0 |
| Script (`#!`) | 197 | 25 | 890 | 147 |
| Other non-Mach-O | 3615 | 9 | 131914 | 5842 |
| Symlink | 761 | 59 | 25567 | 964 |

```
filetype: homebrew bin EXECUTE 1077, sbin EXECUTE 26, lib DYLIB 324, lib BUNDLE 177, lib OBJECT 95, lib EXECUTE 4,
          other BUNDLE 246, other EXECUTE 119, other DYLIB 90, other OBJECT 5
          macports bin EXECUTE 40, lib DYLIB 16, other EXECUTE 4, other DYLIB 1
bits Counter({64: 2240})      cpu {'0x1000007': 2231, '0x100000c': 9}
exec bit on Mach-O: True 1535, False 689
distinct inodes of Mach-O in scope: 1755
macports outside: /opt/local/libexec/gettext/{cldr-plurals,hostname,urlget}, /opt/local/libexec/xz-bootstrap/bin/xz, …/xz-bootstrap/lib/liblzma.5.dylib
```

All 8 fat files are LLVM's `libclang_rt.*_osx_dynamic.dylib` (x86_64, x86_64h, arm64). There is no 32-bit slice on this machine.

### E-3 · Cross-provider references: none in a load command

Over all 2224 Mach-O (in scope and outside), every slice, from the prototype, with "other prefix" = `/opt/local` for a Homebrew file and `/usr/local` or `/opt/homebrew` for a MacPorts file:

```
D none of ['link', 'rpath', 'id', 'dyld_env', 'dylinker', 'resolved_cross', 'rpath_rel_cross']
```

That is: no linking command names the other prefix, no `LC_RPATH` does, no `LC_ID_DYLIB`, no `@rpath`/`@loader_path` entry resolves into it, no `@loader_path` rpath normalises into it, no `LC_DYLD_ENVIRONMENT` exists at all, and no `LC_LOAD_DYLINKER` other than `/usr/lib/dyld`. Symlinks: 27,351, none resolving into the other prefix, none dangling.

```
kinds homebrew LC_LOAD_DYLIB 7992, LC_LOAD_WEAK_DYLIB 20, LC_REEXPORT_DYLIB 1; macports LC_LOAD_DYLIB 284
(LC_LAZY_LOAD_DYLIB and LC_LOAD_UPWARD_DYLIB: none on this machine)
in scope  homebrew: abs:own 3035, abs:system 3299, @rpath 420, @loader_path 13
          macports: abs:own 134, abs:system 127
outside   homebrew: abs:own 97, abs:system 974, @rpath 175
          macports: abs:own 12, abs:system 11
@executable_path as a direct entry: 0.  Relative names: 0.
```

### E-4 · `@rpath` and friends, resolved from the file's own `LC_RPATH` alone

```
homebrew in  @loader_path  RESOLVED on disk -> homebrew 13
homebrew in  @rpath        RESOLVED on disk -> homebrew 412
homebrew in  @rpath        no candidate exists on disk   8
homebrew out @rpath        RESOLVED on disk -> homebrew 175
rpath entries (homebrew): @loader_path 725, absolute 493, @executable_path 6.   macports: 0 entries, 0 @rpath names
@rpath name in a non-executable with no own LC_RPATH: 0
@executable_path rpath, by file type: DYLIB 6 (imath, openexr)
```

The 8 are all `swiftformat`: names like `@rpath/libswiftCore.dylib`, rpaths `['/usr/lib/swift', '@loader_path', '…/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx']`. `man dyld` lines 237-246 (read 2026-11-04):

> "The run path stack is built from the LC_RPATH load commands in the dependency chain that lead to the current dylib load."

### E-5 · Non-Mach-O: scripts and bytes

```
in-scope shebangs  homebrew: system 184, own 12, elsewhere 1 (httpd.exp '#! .');  macports: system 25
of those, via env: homebrew python3 15, python 10, perl 9, php 4, bash 1, ruby 1, sh 1; macports python3 1, sh 1   (43 files)
whole file lists:  env shebangs homebrew 425 files in 32 packages, macports 3 files in 2 packages
                   shebang interpreter under the other prefix: 0
packages with no Mach-O at all: homebrew 12 (certifi, cask, pipx, phpbrew, cloc, autoconf, nvm, …), macports 2 (automake, autoconf)
```

The other prefix's bytes appearing anywhere in a file, whatever it is:

```
homebrew in scope: Mach-O 4 (cmake cpack, libLLVM.23.1.dylib, mise 2026.8.12, mise 2026.9.0), archive 1 (libLLVMOrcTargetProcess.a)
macports in scope: Mach-O 3 (htop, libltdl.7.dylib, pkg-config), script 3 (gettextize, glibtool, ncurses6-config), archive 1 (libltdl.a)
$ strings -a /opt/local/bin/htop | grep /usr/local         → /usr/local/bin/  /usr/local/lib/  /usr/local/sbin/
$ strings -a /opt/local/lib/libltdl.7.dylib | grep /usr/local → /usr/local/lib:/lib:/usr/lib
$ strings -a /usr/local/Cellar/mise/2026.9.0/bin/mise | grep /opt/local → "/opt/local/libexec/openssl3",  "/opt/local",
$ grep -n /usr/local /opt/local/bin/glibtool → 308:sys_lib_dlsearch_path_spec="/usr/local/lib /lib /usr/lib"
```

### E-6 · Shape of packages against "executables only"

```
homebrew 194 packages: exe in bin/sbin 116, Mach-O but no exe in bin/sbin 66 (python@3.13, sdl3, libssh, freetype, readline, …), no Mach-O 12
macports  18 packages: exe in bin/sbin 11, Mach-O but no exe in bin/sbin 5 (libtool, libtextstyle, xz-bootstrap, oniguruma6, gettext-tools-libs), no Mach-O 2
not reachable by load commands from the package's own executables:
    homebrew BUNDLE 423, DYLIB 253, OBJECT 100; macports DYLIB 9
closure from a package's bin/sbin executables, following load commands across packages (127 packages):
    median 4 files, 1 other package; 68 packages reach another; top netpbm 340 files/6 packages, llvm 142/2, coreutils 103/1, graphviz 80/32, php@8.4 58/38
```

`xz-bootstrap` has no Mach-O in `bin/sbin/lib` of the prefix; both of its live under `libexec/`.

### E-7 · Cost (sequential, machine otherwise uncontrolled)

```
files 2239 elapsed 187.9
spawns 4478  median 0.0406  p95 0.0488  max 0.361  sum 184.9 s
per-flag median: -L 0.0405 s, 263 bytes (max 16500); -l 0.0407 s, 5024 bytes (max 229676)
per package, -L + -l over its in-scope Mach-O (185 packages): median 0.175 s, p95 2.468 s, max 27.34 s
  netpbm 334 files 27.34 s; llvm 276 files 22.44 s; httpd 124 9.69 s; coreutils 102 8.41 s; graphviz 40 3.42 s; krb5 40 3.22 s
macports whole: 5.16 s; largest gettext, 17 files
port -q contents: median 0.196 s, max 0.243 s
Python prototype, parse step only, whole file already in memory: 0.107 s over 2224 files
largest Mach-O: semgrep-core 200,249,728 bytes; otool's slowest single spawn 0.361 s
max sizeofcmds 5296 bytes (libavdevice), max ncmds 70
```

`DEFAULT_MAX_OUTPUT_BYTES` is 64 MiB and `DEFAULT_TIMEOUT` 120 s (`process/mod.rs` lines 29, 33). A file list read costs what R-21 measured.

### E-8 · `otool -L` formats

```
$ otool -L /opt/local/lib/liblzma.5.dylib                     (rc 0, thin: first line "<path>:")
/opt/local/lib/liblzma.5.dylib:
	/opt/local/lib/liblzma.5.dylib (compatibility version 14.0.0, current version 14.3.0)
	/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1356.0.0)
$ otool -D /opt/local/lib/liblzma.5.dylib
/opt/local/lib/liblzma.5.dylib:
/opt/local/lib/liblzma.5.dylib

$ otool -L <fat, x86_64 x86_64h arm64>                        (host slice only; first line "<path>:", no architecture)
…/libclang_rt.tsan_osx_dynamic.dylib:
	/usr/local/opt/llvm/lib/clang/23/lib/darwin/libclang_rt.tsan_osx_dynamic.dylib (compatibility version 0.0.0, current version 0.0.0)
	/usr/lib/libc++.1.dylib (…)
$ otool -arch all -L <same>                                   (each slice under its own heading)
…/libclang_rt.tsan_osx_dynamic.dylib (architecture x86_64):
	…
…/libclang_rt.tsan_osx_dynamic.dylib (architecture x86_64h):
	…
…/libclang_rt.tsan_osx_dynamic.dylib (architecture arm64):
	…
$ otool -arch all -L /opt/local/bin/xz                        (thin: "<path>:", same as without -arch)

$ otool -L /opt/local/lib/liblzma.a                           (archive; a heading per member, no tab lines)
Archive : /opt/local/lib/liblzma.a
/opt/local/lib/liblzma.a(liblzma_la-tuklib_physmem.o):
```

Over the sweep: head lines `path:` 2224, `Archive :` 15, member 504, stderr empty on all 4501 spawns, and every tab line of all 2239 `-L` outputs matches

```
^\t(.+) \(compatibility version (\d+\.\d+\.\d+), current version (\d+\.\d+\.\d+)(?:, (weak|reexport|lazy|upward))?\)$
```

Suffixes, one specimen per kind (**crafted** for `lazy`, `upward`; `weak` 16 lines and `reexport` 1 line are real in the sweep):

```
	…/CoreServices (compatibility version 1.0.0, current version 1226.0.0, weak)
	…/CoreServices (compatibility version 1.0.0, current version 1226.0.0, reexport)
	…/CoreServices (compatibility version 1.0.0, current version 1226.0.0, lazy)
	…/CoreServices (compatibility version 1.0.0, current version 1226.0.0, upward)
```

Source: `print_libraries` in `otool/ofile_print.c`, <https://raw.githubusercontent.com/apple-oss-distributions/cctools/main/otool/ofile_print.c>, read 2026-11-04 (first 100,000 characters): the five linking commands fall through to the `LC_ID_DYLIB` case, skipped when `just_id` is set, and it appends `", weak"`, `", reexport"`, `", upward"`, `", lazy"`.

**The self-id line.** Real sweep: 423 thin Mach-O carry `LC_ID_DYLIB`; in all 423 the first `-L` line equals it. **Crafted** (`LC_ID_DYLIB` and the first `LC_LOAD_DYLIB` swapped in a copy of `liblzma`):

```
$ otool -L o_id_after
<S>/o_id_after:
	/opt/local/lib/liblzma.5.dylib (compatibility version 14.0.0, current version 14.3.0)
	/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1356.0.0)
$ otool -D o_id_after
<S>/o_id_after:
/usr/lib/libSystem.B.dylib
```

`-L` prints the id in load-command order, unmarked. `man otool` lines 44-46: "as well as the shared library ID if the file is a shared library". Homebrew's id also need not be the file's path: the `tsan` id above is `/usr/local/opt/llvm/…`.

### E-9 · `otool -l` formats

```
$ otool -arch all -l <fat>
1:   …tsan_osx_dynamic.dylib (architecture x86_64):
2:   Load command 0
366: …tsan_osx_dynamic.dylib (architecture x86_64h):
731: …tsan_osx_dynamic.dylib (architecture arm64):
$ otool -arch all -l /opt/local/lib/liblzma.5.dylib
/opt/local/lib/liblzma.5.dylib:
Load command 0
      cmd LC_SEGMENT_64

          cmd LC_ID_DYLIB
      cmdsize 56
         name /opt/local/lib/liblzma.5.dylib (offset 24)
   time stamp 1 Wed Dec 31 21:00:01 1969
      current version 14.3.0
compatibility version 14.0.0

          cmd LC_LOAD_DYLIB
      cmdsize 56
         name @rpath/libSvtAv1Enc.4.dylib (offset 24)

         path @loader_path/../lib (offset 12)          (under cmd LC_RPATH, as in R-21's E-15)
```

The `LC_RPATH` and `LC_ID_DYLIB` block shapes held across the sweep: a regular expression over `cmd …`, `cmdsize N`, `path|name X (offset N)` matched the prototype on all 2216 thin files (E-15).

Named per kind (**crafted** `LC_LAZY_LOAD_DYLIB`, `LC_LOAD_UPWARD_DYLIB`; the others real): `cmd LC_LOAD_WEAK_DYLIB`, `cmd LC_REEXPORT_DYLIB`, `cmd LC_LAZY_LOAD_DYLIB`, `cmd LC_LOAD_UPWARD_DYLIB`. An unknown command number (**crafted**, `0x7f`) prints as `cmd ?(0x0000007f) Unknown load command` plus a hex dump, and simply vanishes from `-L`.

Command numbers in the sweep: `0x2 0x5 0xb 0xc 0xd 0xe 0x19 0x1b 0x1d 0x24 0x26 0x29 0x2a 0x2e 0x32 0x80000018 0x8000001c 0x8000001f 0x80000022 0x80000028 0x80000033 0x80000034`. Values from `include/mach-o/loader.h`, <https://raw.githubusercontent.com/apple-oss-distributions/cctools/main/include/mach-o/loader.h>, read 2026-11-04: `LC_LOAD_DYLIB 0xc`, `LC_ID_DYLIB 0xd`, `LC_LOAD_WEAK_DYLIB (0x18 | LC_REQ_DYLD)`, `LC_RPATH (0x1c | LC_REQ_DYLD)`, `LC_REEXPORT_DYLIB (0x1f | LC_REQ_DYLD)`, `LC_LAZY_LOAD_DYLIB 0x20`, `LC_LOAD_UPWARD_DYLIB (0x23 | LC_REQ_DYLD)`, `LC_DYLD_ENVIRONMENT 0x27`, `LC_LOAD_DYLINKER 0xe`.

### E-10 · Exit codes: not an object, malformed, hidden slices

```
$ otool -L /usr/local/Cellar/p7zip/17.06/bin/7z   → stdout "<path>: is not an object file", rc=0
$ otool -l <same>; otool -arch all -L <same>      → same line, rc=0
x_text, x_garbage, m_empty, m_trunc4 (crafted)    → same line, rc=0
missing file     rc=1  stderr "error: …/otool-classic: can't open file: <S>/__missing__ (No such file or directory)"
directory        rc=1  stderr "error: …/otool-classic: can't map file: <S>/d_directory (Invalid argument)"
```

**Crafted** malformed files; every one of the first four exits **0**:

| File | `otool -L` stdout (rc=0 unless stated) | Prototype |
|---|---|---|
| `cmdsize` 0 at command 3 | `load command 3 size zero (can't advance to other load commands)`; the later entries are not listed | refused |
| `ncmds` 100000 | the real entries, then `load command 21 size zero …` | refused |
| `sizeofcmds` 0x7fffffff | the real entries, then `Inconsistent sizeofcmds` | refused |
| name offset out of range | `\tBad offset (2147418112) for name of LC_LOAD_DYLIB command 14` (tab-prefixed); `-l`: `name ?(bad offset 2147418112)` | refused |
| truncated to 200 bytes | `load command 1 extends past end of load commands` | refused |
| fat table beyond file | rc=1, stderr `fat file: … truncated or malformed (fat_arch structs would extend past the end of the file)` | refused |
| fat slice beyond file | `/usr/bin/otool` rc=1, stderr `slice size too large, exceeds file size` + `fatal error … in …/otool-classic`; `otool-classic` directly rc=**-6** (SIGABRT) | refused |
| `0xcafebabe` Java class | rc=1, `truncated or malformed`  | "not Mach-O" |

A cross name (**crafted**, a copy of MacPorts' `xz` with its `liblzma` name replaced):

```
$ otool -L n_cross → <S>/n_cross:   	/usr/local/lib/liblzma.5.dylib (compatibility version 14.0.0, current version 14.3.0)   rc=0
```

Fat hiding (**crafted**: x86_64 slice clean, arm64 slice names `/opt/local/lib/libX.dylib`):

```
$ otool -L f_x64clean_armcross           → the x86_64 list only; /opt/local absent; rc=0
$ otool -arch all -L f_x64clean_armcross → both slices; rc=0
$ otool -arch x86_64 -L f_armonly_cross  → rc=1 "does not contain…"
$ otool -L f_armonly_cross               → heading "<S>/f_armonly_cross (architecture arm64):", all entries; rc=0
```

`man otool` lines 171-177: "The arch_type can be "all" to operate on all architectures in the file. The default is to display only the host architecture, if the file contains it;". Man page dated July 31, 2020, title `OTOOL-CLASSIC(1)`.

Name forgery and encoding (**crafted**):

```
name "/x\n\t/usr/lib/ok (c)":   -L → "\t/x" then "\t/usr/lib/ok (c) (compatibility version …)"
                                -l → "         name /x" then "\t/usr/lib/ok (c) (offset 24)"
name "/opt/a b (compat)/c.dylib": -L → printed whole, the regex's greedy (.+) keeps it
name with bytes ff fe:          stdout carries the raw bytes
```

`process/system.rs` line 157 converts stdout with `String::from_utf8_lossy`.

32-bit thin (**crafted** `i386`, `LC_LOAD_DYLIB` `/opt/local/lib/x32.dylib`): `otool -L`, `-l` and the prototype all read it. Thin arm64 on this x86_64 host (**crafted**): all read it.

### E-11 · Which tool prints what

```
$ ls -l /usr/bin/otool /usr/bin/xcrun
-rwxr-xr-x  78 root  wheel  118640 Oct 22 07:38 /usr/bin/otool
-rwxr-xr-x   1 root  wheel  117104 Oct 22 07:38 /usr/bin/xcrun
$ xcrun --find otool
/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/otool
$ ls -l …/XcodeDefault.xctoolchain/usr/bin | grep -i otool
-rwxr-xr-x  1 root  wheel     138208 Apr 30  2026 llvm-otool
lrwxr-xr-x  1 root  wheel         10 Mar 28  2026 otool -> llvm-otool
-rwxr-xr-x  1 root  wheel     980224 Apr 30  2026 otool-classic
$ otool --version       (on stderr)
llvm-otool(1): Apple Inc. version cctools-1040
otool(1): Apple Inc. version cctools-1040
disassembler: LLVM version 21.0.0
$ strings -a …/llvm-otool | grep -iE 'otool-classic|objdump' → otool-classic, llvm-objdump, objdump, …
```

Every error `/usr/bin/otool -L` printed names `otool-classic` (E-10). `otool -L` and `otool-classic -L` printed the same stdout for each specimen in E-10, compared by eye on output cut at about 500 characters, not byte for byte; stderr differed on the abort. The Command Line Tools copy (`DEVELOPER_DIR=/Library/Developer/CommandLineTools`) reports the same version and printed `xz` the same. `/usr/bin/python3` is on the same shim inode count (78) and `xcrun --find python3` resolves into Xcode.

### E-12 · The shim, the environment, the cache

Each line: a 1.2 s pause, then the invocation, comparing `xcrun_db` mtime, size and sha256 before and after. `xcrun_db` is `$(getconf DARWIN_USER_TEMP_DIR)xcrun_db`, mode `-rw-------`, owner `pirado`.

```
sanitised, no HOME (x3)           rc=0 s=0.050 0.034 0.042  db_changed=False
sanitised + HOME (x2)             rc=0 s=0.034 0.041        db_changed=False
sanitised + TMPDIR=scratch        rc=0 s=0.042              db_changed=False   (scratch dir stays empty)
xcrun_nocache=1                   rc=0 s=4.980              db_changed=True    11592 → 12130 bytes
xcrun_verbose=1, xcrun_log=1      rc=0 s=0.043 0.040        db_changed=False
DEVELOPER_DIR=CommandLineTools    rc=0 s=0.078              db_changed=True    12130 → 12609   (a key not seen before)
DEVELOPER_DIR=missing             rc=1 s=0.006  "xcrun: error: missing DEVELOPER_DIR path: …"
DEVELOPER_DIR=empty dir           rc=1 s=0.006  "xcrun: error: invalid DEVELOPER_DIR path (…), missing xcrun at: …/usr/bin/xcrun"
PATH leads with a planted otool   rc=0 s=0.036              real otool's output; PLANTED marker absent
PATH planted + DEVELOPER_DIR=empty rc=1                     the same xcrun error; planted not run
TOOLCHAINS=com.example.none       rc=0 s=1.216              db_changed=True    12609 → 12902
SDKROOT=/nonexistent              rc=72 s=1.394             db_changed=True    12902 → 13245   and E-1's dialog
```

```
$ env -i PATH=… LANG=C LC_ALL=C TMPDIR=<scratch>/ xcrun_verbose=1 /usr/bin/otool -L /opt/local/lib/liblzma.5.dylib
otool: note: PATH = '/usr/bin:/bin:/usr/sbin:/sbin'
otool: note: SDKROOT = '/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk'
otool: note: TOOLCHAINS = ''
otool: note: DEVELOPER_DIR = '/Applications/Xcode.app/Contents/Developer'
otool: note: xcrun_db = '/var/folders/0j/s5tdr1vd2bzbb07xskbs9dw40000gn/T/xcrun_db'
otool: note: database key is: otool|/Applications/…/MacOSX.sdk||/Applications/Xcode.app/Contents/Developer|
otool: note: lookup resolved in '/var/folders/…/T/xcrun_db' : '/Applications/…/XcodeDefault.xctoolchain/usr/bin/otool'
```

Size history of the file outside controlled pairs: 11278 (R-21, 2026-10-28), 9944 at 02:53:55, 10768 at 03:01:56, 11592 at 03:02:01, 2425 at 03:07:36. The shim-based `/usr/bin/python3` running these scripts, `man`, and any other process of this uid also use the file, so these are not attributed. The drop to 2425 is unexplained.

`port` itself, incidentally: with no `HOME`, `port -q contents pkgconfig` printed `child 'child process exited abnormally'` and a `cut` usage error, and listed nothing. With `HOME` set it works. `crates/adapters/src/macports/mod.rs` line 1131 takes a `home`.

### E-13 · Documentation read 2026-11-04

- `man xcode-select` (local): "`--install` Opens a user interface dialog to request automatic installation of the command line developer tools." "`-p`, `--print-path` Prints the path to the currently selected developer directory. This is useful for inspection, but scripts and other tools should use xcrun(1) to locate tool inside the active developer directory."
- `man xcrun` (local, January 14, 2025): cache options `--no-cache` "In effect, causes the cache entry to be refreshed", `--kill-cache`; "`DEVELOPER_DIR` Overrides the active developer directory"; `SDKROOT`, `TOOLCHAINS`, `xcrun_nocache`, `xcrun_verbose`, `xcrun_log`; "When xcrun has taken the place of another tool, the arguments are those of the tool replaced, and the various xcrun options can't be used." The page does not state a write policy for the cache, nor mention a dialog.
- Apple, "Installing the command-line tools", <https://developer.apple.com/documentation/xcode/installing-the-command-line-tools>: "Note: On a fresh install of macOS, invoking any command in Xcode or the command-line tools package (such as `git`) from the command line prompts you to download and install the Command Line Tools for Xcode package." and "In the system dialog that appears, click Install and then agree to the Command Line Tools License Agreement."
- Apple, TN2339, <https://developer.apple.com/library/archive/technotes/tn2339/_index.html> (updated 2017-06-19): "macOS 10.9 and later includes shims or wrapper executables. These shims, installed in `/usr/bin`, can map any tool included in `/usr/bin` to the corresponding one inside Xcode." It does not mention the prompt.

### E-14 · The project's reads, 2026-11-04

- `environment.rs` lines 34-60: `PATH` from `leading_path` plus `SYSTEM_PATH`, `PINNED_ENVIRONMENT` (`LANG`, `LC_ALL`, `TERM`, `NO_COLOR`), `HOME` only if given, `extra` minus reserved and build flags.
- `crates/domain/src/snapshot.rs` line 626, `is_build_flag`: `grep` for `"SDKROOT"` and `"DEVELOPER_DIR"` across `crates/domain/src/*.rs` finds neither, so `extra` could carry them. Today's `extra` is whatever a caller passes; an `otool` spec would pass none.
- `crates/domain/src/snapshot.rs`, `plan.rs`, `verification.rs`: `grep -E "linkage|load_command|install_name|rpath"` finds only `plan.rs` line 361, the doc comment "No new cross-provider linkage was introduced." So the snapshot holds no linkage facts.
- `system.rs` lines 252-272: `env_clear`, `envs(spec)`, stdin null, `current_dir("/")`.
- `mod.rs`: `PERMITTED_PROGRAMS` six names; `is_permitted_program` a leaf name check, plus "from a `bin` directory" for anything with a read list; `SYSTEM_PROBE_INVOCATIONS` absolute path plus whole fixed argv; `ReplayRunner` answers by program name and argv (`recording.rs` line 6-8).
- `filesystem.rs` lines 50-56: `FilesystemScope::{ThisMachine, Elsewhere}`; `Elsewhere` means "Do not read it."
- `RecordingProvenance` (`recording.rs` lines 19-36): `macos`, `architecture`, `provider_versions`, `captured_at`, `captured_by`, `origin`, `note`. No tool version field.
- `docs/passes/16-staged/P/README.md` line 40: "**Not included:** `otool` (part H stages it after its own research)".

### E-15 · The prototype against `otool`

```
reader vs -L disagree 0   (2216 thin Mach-O: same multiset of names, id included)
reader vs -l rpath/id disagree 0   (same 2216, same order)
fat compared 8  -arch all vs reader disagree 0
```

The prototype is 90 lines of Python, including the fat table and bounds checks. E-10's refusals are its output.

## Findings

**Question 1 — what the sweep finds**

1. **This machine has no cross-provider load command in either direction** (E-3): 2224 Mach-O files of 212 packages, every slice, every linking kind, plus ids, rpaths, `LC_DYLD_ENVIRONMENT` and symlinks. So the sweep cannot show a true positive. Detection is shown only on **crafted** specimens (E-10, `n_cross` in the method), never on a real contaminated build (Cannot determine).
2. **Counts.** In `bin/sbin/lib`: 1759 Mach-O (1751 thin, 8 fat), and 222 scripts (E-2). Outside: 465 Mach-O.
3. **`@rpath` needs no guess on this machine in nearly all cases, but the right question is not "which file loads".** From the file's own `LC_RPATH`: 412 of 420 in-scope and 175 of 175 outside resolved to a file on disk; all 13 `@loader_path` entries resolved; there are no direct `@executable_path` entries (E-4). The other 8 are `swiftformat`'s, whose lead rpath is `/usr/lib/swift`, with no file on disk there. The check does not need to know which candidate loads. It needs to know whether **any** candidate lies under the other prefix, and that is lexical: expand each own `LC_RPATH`, substitute `@loader_path`, normalise, test. Conclusion, not observation: none of the 8 is an unknown for this question.
4. **Two residual unknowns stay.** An `@executable_path` rpath in a non-executable depends on who loads it (6 dylibs here: `imath`, `openexr`). And `man dyld` says the stack comes from the whole chain (E-4), so a dylib's `@rpath` name can also be satisfied by a loader's rpath. The check should scan rpath entries of every file it examines, which catches an offending entry in a loader in the same package, and it must report `@executable_path`-in-non-executable as unchecked, not passed.
5. **No `LC_RPATH` points at the other provider** (E-3).
6. **Cost is small for a per-package check, once per file** (E-7): about 0.04 s per spawn; 0.175 s median per package for two flags, 27 s worst (`netpbm`, 334 files). With one flag (finding 20) roughly half. A package's own cost is dominated by its Mach-O count, not by size: a 200 MB file took under 0.4 s.

**Question 2 — what to examine and what to claim**

7. **Every Mach-O of type EXECUTE, DYLIB or BUNDLE in the package's whole file list, selected by magic. Not "executables only", and not only `bin/sbin/lib`.** Evidence (E-6, E-2): 71 of 212 packages have Mach-O but no executable in `bin/sbin`, so "executables only" would be vacuous for them; 676 Homebrew and 9 MacPorts dylibs and bundles are unreachable by load commands from their package's own executables; 465 Mach-O sit outside `bin/sbin/lib` (`xz-bootstrap`'s only binaries are under `libexec/`); 689 lack the execute bit, so neither mode nor directory selects them. `MH_OBJECT` files and `ar` archives have no dynamic link commands and are skipped with a count, not reported as passed.
8. **The file list source is the one R-21 established**: `port -q contents`, `brew list --formula`. Both need the P/02 reads. The list is provider metadata becoming a path, so work order §3 item 10 applies: each entry must be under the detected prefix/keg, must be opened without following out of it, and the check handles a symlink by testing its target.
9. **"New" cannot be honestly claimed as written.** Spec 04 check 5 says "No *new* cross-provider linkage was introduced". The snapshot before a run carries no linkage facts (E-14), so nothing can compare before and after. Conclusion, for the main agent:
   - For a package with `PACKAGE_NEWLY_PRESENT`, every file is new to this run, so "present" and "new" coincide.
   - For a package the destination already held (F-06), a link found now is **not** new, and was not caused by this run. It is still decisive for a removal: if the destination links into the source's prefix, removing the source breaks the destination.
   - So the claim that is true in both cases is: *"at verification time, the destination package's examined files carry no load command, rpath or symlink naming the source provider's detected prefix"*. A pre-existing link should fail the check for removal purposes and be worded as pre-existing, not as introduced. That is a change of statement, reported under Impact.
10. **The check is evidence about INV-07, not the invariant.** INV-07 binds what MacPortsBrew configures, links or records; INV-10's sanitised environment is what upholds it during a provider call. A link in a package the provider built under the user's own shell is a fact about the machine, not a MacPortsBrew violation. INV-08 is about calculations and is not read by this check at all.
11. **A binary built against a MacPorts library by a user's environment (F-2) is detectable if the link is dynamic**, because the install name recorded is the absolute path, or via an rpath. Shown on crafted names only. **Not** detectable: a static link to the other provider's `.a`; headers or configuration taken from it; a binary outside the package's file list, including one the user built themselves.
12. **Non-Mach-O.** A shebang read is cheap and symmetric: 0 interpreters under the other prefix in the whole sweep (E-5). But `#!/usr/bin/env python3` (43 in-scope files, 428 overall, in 34 packages) resolves through `PATH` at run time, so it is **indeterminate**, not clean. 14 packages have no Mach-O at all. Script *bodies* are not checked: a byte search is noise (`glibtool`'s dlsearch default, `ncurses6-config`'s skip list; E-5).
13. **What the check cannot see, to be stated in its report:** transitive dependencies unless it walks them; `dlopen` and any path computed at run time (E-5's `mise`, `cmake`, `libLLVM` hold such strings, and whether they are ever opened cannot be read from the file); `DYLD_*` set by the user at run time; static linking; interpreter module paths (`PYTHONPATH`, `PERL5LIB`); configuration files naming the other prefix; a non-Mach-O executable's behaviour.
14. **Transitive linkage is a design choice with a measured cost.** A walk over load commands within the destination is cheap and bounded on this machine: median 4 files, worst 340 (E-6). 68 of 127 packages reach another package. A removal of the source is only safe if the walk is clean, since a dependency's own link breaks the destination as surely; a pre-existing contaminated dependency is exactly case 9's second bullet. Conclusion: walk the destination provider's own dependencies at file level. Stop at any file outside the destination's prefix, which is either the system or the finding.
15. **The H.3 rule makes "indeterminate" expensive.** H item 3 says any check that cannot be evaluated leaves the package unchecked and its source never removed. If "no Mach-O" and "`env` shebang" both count as unevaluable, 14 + up to 34 packages here can never lose their source. That needs a decision on whether "no load commands exist to examine" is a pass with a stated limit, rather than unchecked. **Reported, not decided here.**

**Question 3 — formats** (evidence E-8 to E-11)

16. **Exit status does not mean a file was examined.** A non-object file and every crafted load-command fault exit **0**, with the diagnostic on **stdout** (E-10). A parser reading only tab lines would turn a truncated listing into a clean pass. Any otool parser must refuse every line outside its grammar, including `is not an object file`, and classify those by rule; never `rc == 0` alone. A missing file, a directory and a bad fat table are rc 1 on stderr.
17. **`-L` on a fat file shows only the host slice, so `-L` alone hides a cross link in another slice** (E-10, crafted; `man otool` confirms). **R-21's staged shape `-L <path>` / `-l <path>` is insufficient; `-arch all` is required.** With it, thin output is unchanged and fat output has per-slice headings (E-8, E-9).
18. **The self-id line must not be identified by position.** It is first in all 423 real dylibs, but it is printed in load-command order, unmarked (E-8, crafted). `-L` alone cannot tell it from a link in a crafted or unusual file. `-l` names it `LC_ID_DYLIB`, and `-D` prints only it. On this machine an id can also legitimately differ from the file's own path (`/usr/local/opt/llvm/…`).
19. **Variants.** `/usr/bin/otool` → xcrun → `llvm-otool`, which hands these flags to `otool-classic` (E-11). Both print all five linking kinds; `-L` drops unknown commands, `-l` shows them as `?(0x…)`.
20. **`-arch all -l <path>` alone carries everything** (E-9): the id labelled, all linking kinds labelled, `LC_RPATH`, `LC_DYLD_ENVIRONMENT`, and unknown commands, in one spawn. Its grammar is much wider than `-L`'s (5 KB median, 230 KB max) but only the `cmd` / `name` / `path` lines matter. Forged lines from an install name containing a newline are visible to a strict parser (E-10): a `name` line not ending in ` (offset N)` is refused.

**Question 4 — the shim**

21. **The sanitised environment leaves everything `otool` needs** (E-12): no `HOME`, no `TMPDIR`, no `DEVELOPER_DIR`, `cwd=/`, stdin null. `xcrun_verbose` shows it finding `DEVELOPER_DIR` and `SDKROOT` itself. `/var/db/xcode_select_link` points at the selected Xcode; that xcrun reads that link was not traced.
22. **R-21's open cache question is settled: a lookup hit writes nothing; a miss rewrites `xcrun_db`** (E-12). A miss was observed with a new key (`DEVELOPER_DIR`, `TOOLCHAINS`) and with `xcrun_nocache=1`. The file is in the user's temp directory, is not under either provider, and is **not** moved by `TMPDIR`. So running `otool` is not write-free. R-21 saw no write because it hit. Whether that write disqualifies it is a question of precedent for `security-reviewer`: the checklist's R-20 entry records bootsnap compile-cache writes from the Homebrew reads, and R-21 refused `brew doctor` and `brew linkage` for writes into the provider's repository or its cache.
23. **A cold lookup is slow and starts other processes.** About 5 s with `xcrun_nocache=1`, and the failure text in E-1 shows xcrun running `sh -c '…/xcodebuild -sdk … -find otool …'`. So on a miss, a permitted single-program read can run a shell and `xcodebuild`. ADR-0006's timeout kills one process, and `scripts/check-no-shell.sh`, a check of this repository's source, has no view of what a permitted program starts.
24. **The install dialog can be raised, from a process with a cleared environment, no TTY and null stdin** (E-1): attributed to `SDKROOT=/nonexistent`, with Xcode selected and `xcode-select -p` succeeding. The sanitised environment does not set `SDKROOT`, so this exact path is not reachable unless `extra` carries it. Apple's documentation says a command run on a fresh machine prompts (E-13). So R-21's gate is **necessary but not sufficient**: `xcode-select -p` succeeding does not prove a lookup cannot fall through to the dialog, and a background read raising a system install prompt runs against INV-05's spirit (Homebrew and MacPorts are what INV-05 names; the developer tools are not).
25. **`DEVELOPER_DIR` pointing nowhere fails cleanly** (rc 1, no dialog in E-12), and a planted `otool` first on `PATH` was not run while the toolchain holds one. An `otool` spec must still have an empty `leading_path`, because a provider's `bin` could hold a tool of that name. What xcrun does when the selected directory lacks the tool was not tested, since that risks a second dialog.

**Question 5 — reader or program entry: the comparison**

| | `otool` program entry | In-process, hand-written reader |
|---|---|---|
| **New spawned program** | Yes: the seventh, first with a caller-supplied path, first that can start `sh` and `xcodebuild` (finding 23) | None |
| **Frozen files** | `PERMITTED_PROGRAMS` plus a new shape; staged | None. Workspace lint and ADR-0004 untouched if no crate is added |
| **Dependency** | Developer tools on the machine, at run time | None |
| **Code** | Output parser with a strict grammar, plus classifying the diagnostic lines in E-10, plus the gate | Fat table, thin 32/64, both byte orders, load-command walk, string bounds. 90 lines of Python here; a Rust size is an estimate, on the order of 200-300 lines with error types (not measured) |
| **Correctness risk** | Apple maintains the parse; our parser of its text is the risk, with wording not documented as stable | Ours entirely; agreed with `otool` on 2224 of 2224 here (E-15), and `dyld` is the true reference, which neither tests |
| **Malformed files** | rc 0 with partial output (E-10); the C tool aborted on one crafted file | Refuses by construction (E-10) |
| **Slices** | Requires `-arch all` | All, always |
| **Failure modes** | No developer dir, dialog, cold 5 s lookup, licence state not observed | I/O errors only |
| **Cost** | About 40 ms per file | Parse is negligible; reads need only header and `sizeofcmds` (5.3 KB max here), not the 200 MB file |
| **Replay (INV-15)** | Free: `ReplayRunner` answers by argv, stdout carried verbatim, the real parser exercised end to end. 5 KB/file with `-l` means a large recording for a package such as `netpbm` (about 1.7 MB; estimate from E-7's median) | Not free. Under `FilesystemScope::Elsewhere` a read says nothing, so `--replay` answers *unevaluable* unless a recording gains file facts. The parser itself is a pure function over bytes and can be tested on captured header bytes (§3 item 9) |
| **Is it a provider fact?** | No. The content is a fact about installed files, reported by an Apple tool at version cctools-1040, and `RecordingProvenance` has no field for that version. Its output format is a dated fact outside both providers | Bytes on disk, no tool involved |
| **Security** | Argv and a detected path, no injection; but `/usr/bin/otool`'s real target is decided by a user-writable cache file (E-12; a same-uid attacker already wins anyway), and C parses hostile bytes in a child process, which isolates a crash | Safe Rust parsing untrusted bytes in our own process: a bug is a refusal or a panic, not memory corruption, given no `unsafe`; arithmetic must be checked, reads bounded, and the name resolved once, the rule `read_bounded` states |
| **Works with no Xcode/CLT** | No (unevaluable, or a prompt) | Yes |

**A dependency (`object`, `goblin`) was not evaluated**: neither is in the local registry, and R-22-style `cargo metadata` was not run. The parse is small enough that a dependency's build-script and licence review is unlikely to be repaid (UNRESOLVED; conclusion only).

26. **Recommendation: the in-process, hand-written reader in `crates/adapters/`, with no `unsafe`, as a pure function over bytes behind one bounded file read.** The main reasons are findings 16, 17, 22, 23 and 24 together. `otool` looks simple, but its exit status does not mean what a check needs, its default hides slices, it writes a cache, it can start a shell and `xcodebuild`, and it can raise a system dialog. Each of these needs guarding in code, and the last cannot be guarded fully. It would also make the check unevaluable on a machine with no developer tools, so a source would never be removed there. The reader needs no owner action to stage and adds no spawn. The honest cost: `--replay` does not exercise a live answer without a recording extension, the parser is ours to keep correct, and it needs fuzzing and captured-byte fixtures. Under replay the check should answer unevaluable (`FilesystemScope::Elsewhere`), which the existing cask receipt read already does, unless the owner wants file facts added to the recording format.

27. **Rule for an unknown command, if the reader is chosen:** a command number with `LC_REQ_DYLD` set that the reader does not know makes that file unchecked. dyld refusing such a binary is a lead from the header's naming, and was **not** verified here. The sweep saw only `0x80000018 1c 1f 22 28 33 34` (E-9).

**The exact allowlist shape, if the owner chooses `otool` anyway**

28. One program, one form, in a table of its own, because neither `SYSTEM_PROBE_INVOCATIONS` (fixed argv) nor `PERMITTED_SUBCOMMANDS` (provider read list; a "bin directory under a detected prefix" rule that does not describe `/usr/bin`) fits:

    ```
    program   exactly /usr/bin/otool   (the path, not the leaf; never resolved from PATH)
    argv      exactly 4 elements:  "-arch"  "all"  "-l"  <path>
    <path>    one absolute path, no "..", no NUL, within MAX_PATH_LEN,
              under a detected provider prefix (for Homebrew the keg under its cellar),
              an entry of that package's own file list (§3 item 10)
    env       sanitised, empty leading_path, no extra: never SDKROOT, DEVELOPER_DIR, TOOLCHAINS, xcrun_*
    gate      /usr/bin/xcode-select -p exits 0 (already permitted), else unevaluable; not sufficient (finding 24)
    parse     any line outside the grammar makes the file unevaluable; rc 0 proves nothing (finding 16)
    ```

    This replaces R-21's `-L <abs path>` and `-l <abs path>` row. `may_spawn` sees only program and argv, so "under a detected prefix" is checked by the adapter or needs the prefix table; a static test cannot hold it alone.

## Cannot determine

- **UNRESOLVED: a real contaminated binary.** No file here is one. All detection evidence is on crafted names. Needs a machine holding one, or a disposable prefix where a build may run; nothing may be built here against either provider.
- **UNRESOLVED: `/usr/bin/otool` with no developer directory and no tools.** Not observable here. Apple's documentation (E-13) says a fresh install prompts, for commands "from the command line", and does not cover a background process. E-1 suggests no TTY is needed. Needs a clean VM. Runtime detection is a partial answer only (finding 24). This test must never be repeated on a working machine.
- **UNRESOLVED: exactly what triggered the dialog.** Attribution is by timing. The stderr tail after `xcode-select: Failed to lo` was cut and not re-captured.
- **UNRESOLVED: `xcrun`'s cache write and eviction policy.** No documentation found. The write on a miss is observed; the shrink to 2425 bytes is not explained.
- **UNRESOLVED: Xcode licence not accepted, and the state after a macOS upgrade with stale tools.** Not observable here. Whether `/usr/bin/otool` fails or prompts is unknown. Treat a nonzero exit as unevaluable.
- **Apple Silicon.** Default slice selection, arm64e, and whether a MacPorts file naming `/usr/local` is cross-provider when Homebrew is at `/opt/homebrew` (the check should use *detected* prefixes). Runtime detection; needs an arm64 machine to observe.
- **`dyld`'s rpath stack order, and whether it refuses unknown `LC_REQ_DYLD` commands.** Read only as far as `man dyld`; dyld's source was not read.
- **Whether a 32-bit or big-endian slice can occur in practice.** None here. Reader handles them; `otool` read a crafted one.
- **Crate alternatives.** Not evaluated (above).
- **Inactive MacPorts images.** None here, as in R-21; the sweep used active ports only.
- **Casks.** Out of scope: R-21's question about `brew list --cask` remains, so a cask's file list is unknown and the check is unevaluable for it.

## Impact on the specification

Nothing here was edited. For the main agent to weigh:

- **`docs/spec/04-migration-and-safety.md`, Verification contract check 5: "No *new* cross-provider linkage was introduced".** Not honestly checkable as worded, because no baseline exists (finding 9). Suggested direction: the check reports what is present at verification time, distinguishes `PACKAGE_NEWLY_PRESENT` from a pre-existing package, and a link in either blocks removal. This is a Tier 3 wording change with a `docs/spec-evolution.md` entry.
- **Spec 04 "`doctor --deep` … inspects the dynamic-library dependencies"** can reuse the same reader, with the same limits (finding 13). No contradiction.
- **R-21 finding 14 and its staging row** are corrected here: `-arch all` is required, `-l` alone suffices, "not an object file" is rc 0 (R-21 recorded the line and not the code), `xcode-select -p` is not a sufficient gate, and the cache question is closed: a miss writes. R-21 should get a forward pointer.
- **R-21's sentence that the shim "consults `xcrun`'s cache"** stands; add that it may write it and may run `sh` and `xcodebuild`.
- **Work order 16 part H item 3** interacts with findings 12 and 15: as written, every script-only package stays unchecked for ever. Needs a ruling on what counts as genuinely unevaluable, and it shapes S's tripwire (part H item 5).
- **INV-05** names Homebrew and MacPorts, not the developer tools, so a raised prompt does not breach its letter. It is reported because the project's stance on implicit installs reaches the same place, not as a contradiction.
- **Owner trade-off (R-21 Impact):** this note recommends the reader, so nothing for `otool` needs staging under `docs/passes/16-staged/`. If the owner rules otherwise, finding 28 is the shape.
- No Tier 1 statement was found to be contradicted.

## Re-check when

- Xcode or the Command Line Tools change `otool` (here cctools-1040, LLVM 21.0.0), especially if `llvm-otool` stops handing `-L`/`-l` to `otool-classic`, since all format findings rest on that.
- macOS changes the shim, `xcrun`'s cache location, or its prompt behaviour.
- `include/mach-o/loader.h` gains a linking load command, or a new `LC_REQ_DYLD` command appears in a sweep.
- Apple Silicon or a machine with a real cross-provider link becomes available: re-run the sweep there, as both would answer open items.
- The set of installed packages changes materially. The counts here describe this machine on this date, not a population.
- Homebrew moves past 7.0.11-143-g874c46a or MacPorts past 2.12.6, particularly if either changes rpath or install-name handling, or `brew list --formula` / `port -q contents` output.
- Before any `otool` staging is applied: re-run E-10 and E-12 (never the `SDKROOT` case of E-1).

## The Rust reader against `otool` (2026-11-04)

This is the agreement of `crates/adapters/src/macho.rs` itself, which is what part H's plan asks for. E-15's 2224 of 2224 was the Python prototype's. The harness lived in the session scratch directory and is not in the repository. Same machine and Homebrew as above; `/Library/Developer/CommandLineTools/usr/bin/otool` (cctools-1040) run directly, `SDKROOT` and `DEVELOPER_DIR` unset, stdin null, `cwd=/`. `xcrun_db`'s mtime and size were the same before and after. MacPorts' files were not in this run.

**The reader alone, every regular file under `/usr/local/Cellar`, no spawn (139,759 files):**

```
thin refs: Execute 1226, Dylib 406, Bundle 423     fat refs 8     fat of archives 15
skipped: Object 100, Archive 964
script: absolute 622, env 422, relative 43         not an object 135524
Malformed(FatTable) 3, TooLarge(Slices) 3          largest window total for one file 7016 bytes
```

The counts match E-2 (2155 thin including objects, 8 fat, 15 fat around archives, 964 archives). No real Mach-O answered an error, an unknown file type or an unknown `LC_REQ_DYLD` command, so the stricter refusals here than in the prototype (`sizeofcmds` must equal the commands walked, a name offset must be past the fixed structure, a name must be non-empty and NUL-terminated) cost nothing on this machine.

**All 6 errors are Java `.class` files**, whose version reads as `nfat`: the 3 `Malformed` answers and the 3 `TooLarge`, in `gettext` and `nmap`. That is the plan's rule working as written: a `0xcafebabe` that is not a fat table is not "not Mach-O". Its cost is that those two packages answer `Unchecked` for a file no loader reads. Reported, not decided here.

**Against `otool`, 200 files:** every fat file with references (8), 5 fat wrappers around archives, 5 archives, 5 objects, 5 scripts, 5 other non-objects, and 167 thin files at an even stride over the sorted list.

```
-arch all -l agrees 175 of 175     per slice, in order: each linking kind, LC_LOAD_DYLINKER, LC_RPATH; LC_ID_DYLIB excluded
-arch all -L agrees 175 of 175     per slice, as a multiset, adding back the id -l names
slices compared 191, of which 61 carry an id; load references 786; rpaths 144
archives: 5 of 5 no tab lines      objects and fat wrappers: 10 of 10 no tab lines
scripts and non-objects: 10 of 10 "is not an object file"
disagreements 0                    otool nonzero exits 0
```

The 200 are a sample, not the sweep, and `otool` is the reference here as E-15 says, not `dyld`. No file here holds a cross-provider reference, a `LC_LAZY_LOAD_DYLIB`, a `LC_LOAD_UPWARD_DYLIB` or a `LC_DYLD_ENVIRONMENT` (E-3), so those kinds and detection itself are shown only on specimens built in `crates/adapters/tests/macho.rs`.

## Ruling on Java classes (2026-11-04, work order 16 H-2)

The 6 files above stay `Unchecked`. A `0xcafebabe` that does not fit a fat table answers `MALFORMED_FILE`, because a crafted fat header must not hide as a Java class; `gettext` and `nmap` therefore answer unchecked on this check and keep their sources. **An accepted cost, in the safe direction**, and one that also keeps a probe from running for them (no file may be malformed). `crates/adapters/src/destination_reads.rs`, `a_cafebabe_that_does_not_fit_is_malformed_and_never_not_macho`.

The reader's first live run on 2026-11-04 (session scratch, read-only, no package's program started) found the same in MacPorts' `gettext`, and both `gettext` packages also carry an `env` shebang on their executable lists, so `gettext` answers unchecked here on either provider, and its probe is not reached. The same run: `xz`, `htop`, `m4` and `pkgconf` read clean on each provider that has them; `openssl@4`'s `openssl` answered `openssl@3`, and `npm` left its keg (`ENTRY_LEAVES_OWNER`, the name since H-2 review round 1), as R-26 predicted.

## Accepted limits of the reader (2026-11-04, work order 16 H-2, review round 1)

**A relocated cellar answers `UNCHECKED`, and that is accepted.** A listed or reached file whose canonical path is outside the destination and in no known prefix answers `FILE_NOT_EXAMINED`, never `NO_LOAD_COMMANDS`, since nothing was read. A Homebrew whose cellar lives elsewhere by a link therefore keeps its source. Round 2 widened this to any real file a load command names in no known prefix outside the system's own roots, which also covers another provider's relocated store, and round 3 to a real interpreter or path-shaped shebang word outside those roots and `SYSTEM_PATH`. It costs a removal, not a claim.

**The limit of this claim, for part L.** A static read of what this reader found, on this macOS (x86_64, 26.7.2, one machine). The age threshold at which dyld stops applying its default fallback is known only as `man dyld`'s "Fall 2023 or later", and the reader cannot tell a binary's age, so it asks of all of them. **No chain beyond the walk:** an `@rpath` name is tried against every rpath this walk saw, and never against those of a loader the walk did not reach, so no whole-machine loader chain is read. Results on the 10.12 floor were not observed.

**The system's own roots (work order 16 H-2 round 4, an allow-list).** `/usr/lib`, `/System/Library/Frameworks` and `/System/Library/PrivateFrameworks`, tested on where a path really is, through its deepest existing ancestor. `man dyld` (read 2026-11-04) writes dylibs by full paths under `/usr/lib`, keeps `/System/Library/Frameworks` on its default fallback, and says an image may come from the shared cache. Round 3's list was the other way round, `/System/Library` with exclusions, and a firmlink a later macOS adds would have reopened a false `HOLDS`. What it found stays true, on this machine, and is why:

- `/usr/share/firmlinks` lists seven under `/System/Library`, each the Data volume: `Caches`, `Assets`, `PreinstalledAssets`, `AssetsV2`, `PreinstalledAssetsV2`, `Speech` and `CoreServices/CoreTypes.bundle/Contents/Library`.
- `AssetsV2/locks` and `AssetsV2/downloadDir` are `drwxrwxrwx`, and `Caches/MobileActivation/data_ark.plist` is `-rw-rw-rw-`.
- `/System/Volumes/Data` is the whole writable volume, `canonicalize` keeps that spelling, dyld honours it, and everything reports one `st_dev`.
- `/usr/lib/cron` is a link to `/private/var/at`, which is why an absent candidate's text does not decide. `realpath` returns the stored name, for a case or a Unicode fold, so a fold of a root is that root (observed 2026-11-04).
- `WebKit`, `JavaScriptCore` and others under `/System/Library/Frameworks`, and `SafariCore` and others under `PrivateFrameworks`, are links into `/System/Volumes/Preboot/Cryptexes/OS`, so they do not qualify. **No installed package here reaches one as a real file:** the measurement below has none, and a package that does answers `FILE_NOT_EXAMINED`.

**`@rpath`, by dyld's semantics (round 4).** `man dyld` keeps a run path list "built from the LC_RPATH load commands", so a name is tried against every rpath the walk saw (finding 4 already said an entry can serve another file). Security proved a real load through the executable's rpath while `libD`'s only rpath was `/usr/lib/swift`, answering `HOLDS`. A name none of whose candidates exists is `RPATH_UNRESOLVED`, except that an absent candidate under the roots answers it, since a system library may be in the shared cache; a candidate that exists elsewhere has already been measured. **Observed 2026-11-04:** `/usr/lib/swift/libswiftCore.dylib` is not on disk, and `dlopen` of it succeeds from a Python process. That the file comes from the cache is still inferred. A rpath entry that is a real directory in no known prefix, outside the roots, is `FILE_NOT_EXAMINED`.

**dyld's default fallback (round 4), `FALLBACK_UNDER_OTHER_PREFIX`.** An earlier statement in this work that there is no fallback was wrong, and is withdrawn: `man dyld` (read 2026-11-04) says "for new binaries (Fall 2023 or later) there is no default", and for older ones `/usr/local/lib:/usr/lib` and `/Library/Frameworks:/System/Library/Frameworks`. This is the project's founding direction, a MacPorts destination with Homebrew at `/usr/local`. A binary marked as older was measured in review to try `/usr/local/lib/<leaf>` and then `/usr/lib/<leaf>` after a name that resolves to nothing, or an `@rpath` answered only by an absent system candidate. So those names also look there, and a file of that name under another prefix is `UNCHECKED`, never `FAILS`, since the reader cannot tell which loads.

**A relocated cellar answers `UNCHECKED`, and that is accepted.** A listed or reached file whose canonical path is outside the destination and in no known prefix answers `FILE_NOT_EXAMINED`, never `NO_LOAD_COMMANDS`, since nothing was read. A Homebrew whose cellar lives elsewhere by a link therefore keeps its source. Rounds 2 to 4 widened this to any real file a load command names, any real rpath directory, and any real interpreter or path in a `#!` line outside the roots and `SYSTEM_PATH`. It costs a removal, not a claim.

**What that costs, measured 2026-11-04, after round 4.** Read-only, over this machine's installed packages through the product's own reads and judges, no package's program run. One row per first cause the judge names:

| Linkage answer | Homebrew (194) | MacPorts (18) |
|---|---|---|
| `HOLDS` | 163 | 16 |
| `INTERPRETER_RESOLVED_AT_RUN_TIME` | 10 | 1 |
| `WALK_OVER_BOUND` | 9 | 0 |
| `FILE_LIST_NOT_KNOWN` | 4 | 0 |
| `FILE_NOT_EXAMINED` | 3 | 0 |
| `RPATH_UNRESOLVED` | 3 | 0 |
| `MALFORMED_FILE` | 2 | 1 |
| `FALLBACK_UNDER_OTHER_PREFIX` | 0 | 0 |
| `FAILS` | 0 | 0 |

The fallback turned nothing unchecked here. **The real-rpath rule costs 3 packages, each because of a developer tool on this machine:** `swiftformat` and `xcodegen` carry an rpath into the Xcode toolchain at `/Applications/Xcode.app/.../swift-5.5/macosx`, which exists here, and `phpbrew`'s interpreter, `php@8.4`, carries `/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk/usr/lib`. A machine with no Xcode or Command Line Tools would answer differently for them. `imath`, `openexr` and `jpeg-xl` answer `RPATH_UNRESOLVED` by an older rule: `libImath` and `openexr`'s libraries carry `@executable_path/../lib` in a dylib. `jpeg-xl`'s own files carry no such rpath in what was searched, and its executables set the flag only when walked, which is consistent with reaching those libraries and was not traced further. Integrity on the same packages: 190 and 18 hold, and 4 Homebrew `FILE_LIST_NOT_KNOWN`, the same four.

**Script bodies are unread, and that is accepted scope.** A shebang's interpreter, and any `/...` inside a word on its line, are classified (`#!/usr/bin/env /opt/homebrew/bin/python3`). What the script then runs, loads or names is not read: nothing short of running it would answer, and the claim is about references in load commands and shebangs, as the basis `REFERENCES_EXAMINED` says.

**The window between `canonicalize` and `open` is accepted.** The walk classifies by what resolved and then opens the name, so something swapped between the two is read under the first answer. The directories differ in who could do that, as read on 2026-11-04: `/opt/local` is `root:wheel` 755, so only root can swap a file under the MacPorts prefix, and `/usr/local/Cellar` is `pirado:admin` 775, so the owner's account and any process running as it, or as a member of `admin`, can under Homebrew's. Anyone able to race it there could as easily have written the file before the check, so closing the window would not remove the exposure, and the reader claims nothing about a machine under attack. A handle-only walk would not change what a later `exec` loads.
