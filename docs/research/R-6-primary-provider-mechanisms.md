# R-6 — What mechanism should MacPortsBrew use to set a package's primary provider?

- **Status:** answered, with two named `UNRESOLVED` sub-questions (privilege
  requirement of `port select --set`; relink-on-upgrade behaviour of
  `brew upgrade` for an unlinked formula)
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (Darwin 25.6.0), x86_64 (Intel), Homebrew 6.0.21 @
  `/usr/local`, MacPorts **not installed** (`command -v port` → not found;
  `/opt/local` absent)
- **Blocking:** `05-environment-and-primary-provider.md` ("Implementing package
  primary" — currently a **HYPOTHESIS**), the plan types in
  `03-planning-and-mapping.md`, the `doctor` surface, and the executable
  ownership classification in `05`.

> **Machine note, added 2026-09-15.** This note states, correctly for its answer
> date, that MacPorts could not be observed here. **It can be since 2026-09-06**,
> when MacPorts 2.12.6 was installed at `/opt/local` (first ports 2026-09-07).
> Nothing below has been re-checked against it; the citations stand as citations.
> R-5 and R-5a were re-checked in place and show what that costs — R-5 gained
> fourteen evidence blocks and produced SE-077. Where this note names a "future
> attempt" that needed a MacPorts machine, that attempt is now available (F-46).

## Question

`05-environment-and-primary-provider.md` states a **HYPOTHESIS** — a
MacPortsBrew-owned shim directory early on `PATH` — and requires it be compared
against MacPorts' own selection mechanism, Homebrew's linking behaviour,
versioned executable names, and doing nothing. It also states a **MUST**:
"Prefer a provider-native mechanism over a MacPortsBrew-owned one wherever the
provider offers one that does the job."

So the question is not "which mechanism is nicest" but: **does either provider
offer a native mechanism that does *this* job** — making a chosen
implementation of one package resolve first, across two providers, without
becoming a global priority change (INV-12) and without transferring dependency
ownership (INV-11)?

---

## Evidence

### E0 — Session environment (quiet-provider settings actually in force)

```
$ env | grep -i '^HOMEBREW' | sort
HOMEBREW_NO_ANALYTICS=1
HOMEBREW_NO_AUTO_UPDATE=1
HOMEBREW_NO_ENV_HINTS=1
```

These come from the `env` block of the repository's `.claude/settings.json`
(`HOMEBREW_NO_AUTO_UPDATE=1`, `HOMEBREW_NO_ANALYTICS=1`,
`HOMEBREW_NO_ENV_HINTS=1`, plus `MACPORTSBREW_AGENT_SESSION=1` and
`CARGO_TERM_COLOR=never`). Every `brew` command in this note was run with them
set. `HOMEBREW_NO_INSTALL_FROM_API` was **not** set, i.e. the default API mode.
Per R-1, these three do **not** suppress the `homebrew/core` tap clone that
`brew info <unknown-name>` triggers; no command in this note passes an
unknown name.

```
$ date -u +%Y-%m-%dT%H:%M:%SZ
2026-09-02T12:40:09Z
$ brew --version
Homebrew 6.0.21
Homebrew/homebrew-core (git revision e045d7385fd; last commit 2026-09-02)
$ brew --prefix
/usr/local
$ command -v port || echo "port: not found"
port: not found
$ ls -d /opt/local 2>/dev/null || echo "/opt/local: absent"
/opt/local: absent
```

MacPorts is not installed on this machine. **Every MacPorts claim below is
category (2) — cited — or category (3) — deferred to runtime detection. None
is observed.**

### E1 — Homebrew: what `/usr/local/bin` actually contains

```
$ ls -l /usr/local/bin | head -20
total 51232
lrwxr-xr-x@  1 pirado  admin        43 Aug 17 07:36 2to3-3.12 -> ../Cellar/python@3.12/3.12.14/bin/2to3-3.12
lrwxr-xr-x@  1 pirado  admin        38 Aug 18 23:12 411toppm -> ../Cellar/netpbm/11.02.28/bin/411toppm
lrwxr-xr-x@  1 pirado  admin        28 Jan 10  2025 7z -> ../Cellar/p7zip/17.06/bin/7z
lrwxr-xr-x@  1 pirado  admin        29 Jan 10  2025 7za -> ../Cellar/p7zip/17.06/bin/7za
lrwxr-xr-x@  1 pirado  admin        29 Jan 10  2025 7zr -> ../Cellar/p7zip/17.06/bin/7zr
-rwxr-xr-x   1 root    wheel        80 Oct 17  2025 VBoxAudioTest
-rwxr-xr-x   1 root    wheel        80 Oct 17  2025 VBoxAutostart
-rwxr-xr-x   1 root    wheel        82 Oct 17  2025 VBoxBalloonCtrl
-rwxr-xr-x   1 root    wheel        80 Oct 17  2025 VBoxBugReport
-rwxr-xr-x   1 root    wheel        77 Oct 17  2025 VBoxDTrace
-rwxr-xr-x   1 root    wheel        79 Oct 17  2025 VBoxHeadless
-rwxr-xr-x   1 root    wheel        77 Oct 17  2025 VBoxManage
-rwxr-xr-x   1 root    wheel        79 Oct 17  2025 VBoxVRDP
-rwxr-xr-x   1 root    wheel        77 Oct 17  2025 VirtualBox
-rwxr-xr-x   1 root    wheel       115 Oct 17  2025 VirtualBoxVM
lrwxr-xr-x@  1 pirado  admin        29 Sep  1 20:03 ab -> ../Cellar/httpd/2.4.68/bin/ab
lrwxr-xr-x@  1 pirado  admin        42 Jun 30 19:20 actionlint -> ../Cellar/actionlint/1.7.12/bin/actionlint
lrwxr-xr-x@  1 pirado  admin        37 Aug  7 12:27 acyclic -> ../Cellar/graphviz/15.1.1/bin/acyclic
lrwxr-xr-x@  1 pirado  admin        36 Jul 31 13:00 adaparse -> ../Cellar/ada-url/4.0.0/bin/adaparse
```

Counted with a Python script over `os.listdir('/usr/local/bin')`:

```
total /usr/local/bin entries: 1086
symlinks -> ../Cellar:       1024
non-Cellar entries:            62
```

The 62 non-Cellar entries include VirtualBox binaries owned by `root:wheel`,
symlinks into `/Applications/Docker.app`, symlinks into `/usr/local/Caskroom`,
symlinks into `/usr/local/lib/node_modules` (npm globals: `pnpm`, `codex`,
`ast-index`, `context-mode`), `/usr/local/aws-cli/aws`,
`/usr/local/microsoft/powershell/7/pwsh`, and 36 regular files written by
`pip`-style installers (`ruff`, `pytest`, `mypy`, `uvicorn`, `websockets`,
`pyright`, …).

### E2 — Homebrew's own "what is not mine" command, and its blind spot

```
$ brew --prefix --unbrewed | wc -l
69643
$ brew --prefix --unbrewed | grep -c '^bin/'
47
```

Cross-checked against the directory listing:

```
kind x in_unbrewed: Counter({('symlink', False): 1048, ('file', True): 36, ('file', False): 1, ('dir', False): 1})
symlinks reported as unbrewed: []
regular files NOT reported as unbrewed: ['.keepme']
```

The 47 vs 36 difference is fully accounted for: of the 47 `bin/` paths, 36 are
top-level names and 11 are files *inside* `bin/warsaw/`, a foreign directory
(`bin/warsaw/core`, `bin/warsaw/libnspr4.dylib`, `bin/warsaw/uninstall.sh`, …).
No reported top-level name is absent from the directory listing.

Manpage text for the flag (`man brew`, Homebrew 6.0.21, read 2026-09-02):

```
   --prefix [--unbrewed] [--installed] [formula ...]
       Display Homebrew's install path. Default:

       o   macOS ARM: /opt/homebrew

       o   macOS Intel: /usr/local

       o   Linux: /home/linuxbrew/.linuxbrew

       If formula is provided, display the location where formula is or would be installed.

       --unbrewed
	      List files in Homebrew's prefix not installed by Homebrew.
```

### E3 — Homebrew glossary (`man brew`, Homebrew 6.0.21, read 2026-09-02)

```
       prefix path in which Homebrew is installed, e.g. /opt/homebrew or /home/linuxbrew/.linuxbrew

       keg    installation destination directory of a given formula version, e.g.
	      /opt/homebrew/Cellar/foo/0.1

       rack   directory containing one or more versioned kegs, e.g. /opt/homebrew/Cellar/foo

       keg-only
	      a formula is keg-only if it is not symlinked into Homebrew's prefix

       opt prefix
	      a symlink to the active version of a keg, e.g. /opt/homebrew/opt/foo
```

### E4 — `brew link` / `brew unlink` (`man brew`, Homebrew 6.0.21, read 2026-09-02)

```
   link, ln [options] installed_formula [...]
       Symlink all of formula's installed files into Homebrew's prefix. This is done automatically
       when you install formulae but can be useful for manual installations.

       --overwrite
	      Delete files that already exist in the prefix while linking.

       -n, --dry-run
	      List files which would be linked or deleted by brew link --overwrite without actually
	      linking or deleting any files.

       -f, --force
	      Allow keg-only formulae to be linked.

       --HEAD Link the HEAD version of the formula if it is installed.
```

```
   unlink [--dry-run] installed_formula [...]
       Remove symlinks for formula from Homebrew's prefix. This can be useful for temporarily
       disabling a formula: brew unlink formula && commands && brew link formula

       -n, --dry-run
	      List files which would be unlinked without actually unlinking or deleting any files.
```

Searching the same manpage for a `switch` subcommand:

```
$ grep -n -i 'switch' brewman.txt
764:	      Switch into the mode used when evaluating all formulae and casks, but only list
3031:       --cask switch.
```

Neither is a `brew switch` command. There is no `brew switch` in Homebrew 6.0.21.

### E5 — Homebrew link state as machine-readable data

`brew info --json=v2 --installed` (876,713 bytes, 180 formulae), fields
extracted with Python:

```
formulae in JSON: 180
keg_only count: 19
keg_only names: apr apr-util curl icu4c@78 krb5 libffi libpq llvm m4 ncurses net-snmp openjdk openjdk@21 openldap openssl@4 php@8.4 readline sqlite unzip
keg_only reasons: {':provided_by_macos': 11, ':shadowed_by_macos': 4, 'it conflicts with PostgreSQL': 1, ':versioned_formula': 3}
not keg_only and linked_keg null: ['pydantic']
installed_on_request True count: 47 of 180
```

Per-formula detail:

```
--- git ---
  keg_only: False | reason: {}
  linked_keg: 2.55.0
  versioned_formulae: []
  installed.version: 2.55.0 | installed_on_request: True | installed_as_dependency: None
--- curl ---
  keg_only: True | reason: {'reason': ':provided_by_macos', 'explanation': ''}
  linked_keg: None
  versioned_formulae: []
  installed.version: 8.21.0 | installed_on_request: True | installed_as_dependency: None
--- python@3.12 ---
  keg_only: False | reason: {}
  linked_keg: 3.12.14
  versioned_formulae: ['python@3.14', 'python@3.13', 'python@3.11', 'python@3.10', 'python@3.9']
  installed.version: 3.12.14 | installed_on_request: True | installed_as_dependency: None
--- openjdk@21 ---
  keg_only: True | reason: {'reason': ':versioned_formula', 'explanation': ''}
  linked_keg: None
  versioned_formulae: ['openjdk@25', 'openjdk@17', 'openjdk@11', 'openjdk@8']
  installed.version: 21.0.12.1 | installed_on_request: False | installed_as_dependency: None
--- llvm ---
  keg_only: True | reason: {'reason': ':provided_by_macos', 'explanation': ''}
  linked_keg: None
  versioned_formulae: ['llvm@22', 'llvm@21', ...]
  installed.version: 23.1.0 | installed_on_request: False | installed_as_dependency: None
```

Formulae with more than one keg installed, with the linked one:

```
[('cryptography', ['50.0.0', '50.0.1'], '50.0.1'), ('harfbuzz', ['14.3.1', '14.4.0'], '14.4.0'),
 ('libffi', ['3.7.1', '3.8.0'], None), ('mise', ['2026.8.12', '2026.9.0'], '2026.9.0'),
 ('openssl@3', ['3.6.3', '3.6.4'], '3.6.4'), ('openssl@4', ['4.0.1', '4.0.2'], None),
 ('pcre2', ['10.47_1', '10.48'], '10.48'), ('php@8.4', ['8.4.24', '8.4.25'], None),
 ('pipx', ['1.16.7', '1.17.1'], '1.17.1'), ('pydantic', ['2.13.4', '2.13.5'], None),
 ('simdjson', ['4.6.8', '4.6.9'], '4.6.9'), ('tree-sitter', ['0.26.13', '0.27.0'], '0.27.0')]
```

On-disk link registry (**not documented in `man brew`**; `grep -i
'var/homebrew\|linked_keg\|LINKED_KEGS'` on the manpage returns nothing):

```
$ ls -l /usr/local/var/homebrew/linked | head -5
total 0
lrwxr-xr-x@ 1 pirado  admin  33 Jun 30 19:20 actionlint -> ../../../Cellar/actionlint/1.7.12
lrwxr-xr-x@ 1 pirado  admin  29 Jul 31 13:00 ada-url -> ../../../Cellar/ada-url/4.0.0
lrwxr-xr-x@ 1 pirado  admin  26 May 25 19:36 aom -> ../../../Cellar/aom/3.14.1
lrwxr-xr-x@ 1 pirado  admin  33 Apr 27  2024 argon2 -> ../../../Cellar/argon2/20190702_1
$ ls -1 /usr/local/var/homebrew/linked | wc -l
     160
$ ls -ld /usr/local/var/homebrew/linked/pydantic
ls: /usr/local/var/homebrew/linked/pydantic: No such file or directory
$ ls -ld /usr/local/var/homebrew/linked/git
lrwxr-xr-x@ 1 pirado  admin  26 Jul  3 02:36 /usr/local/var/homebrew/linked/git -> ../../../Cellar/git/2.55.0
```

160 = 180 installed − 19 keg-only − 1 (`pydantic`, unlinked).

### E6 — `brew --prefix <formula>` and the `opt/` symlink are not link state

```
$ brew --prefix git
/usr/local/opt/git
$ brew --prefix curl          # keg-only, linked_keg = None
/usr/local/opt/curl
$ brew --prefix pydantic      # NOT keg-only, linked_keg = None (unlinked)
/usr/local/opt/pydantic
$ brew --prefix ripgrep       # not installed at all
/usr/local/opt/ripgrep
exit=0

$ ls -ld /usr/local/opt/git /usr/local/opt/curl /usr/local/opt/python@3.12 /usr/local/opt/pydantic
lrwxr-xr-x@ 1 pirado  admin  21 Jul  3 02:34 /usr/local/opt/curl -> ../Cellar/curl/8.21.0
lrwxr-xr-x@ 1 pirado  admin  20 Jul  3 02:36 /usr/local/opt/git -> ../Cellar/git/2.55.0
lrwxr-xr-x@ 1 pirado  admin  29 Aug 17 07:36 /usr/local/opt/python@3.12 -> ../Cellar/python@3.12/3.12.14
lrwxr-xr-x@ 1 pirado  admin  25 Sep  1 21:43 /usr/local/opt/pydantic -> ../Cellar/pydantic/2.13.5
$ ls -1 /usr/local/opt | wc -l
     223
$ ls -1 /usr/local/Cellar/pydantic
2.13.4
2.13.5
```

`/usr/local/opt/pydantic` points at `2.13.5` — the newer of two racked kegs —
even though the formula is unlinked. The `opt` symlink tracked the upgrade;
`../Cellar/pydantic/2.13.4` did not disappear.

### E7 — What a keg-only formula does to `which`

```
$ ls -l /usr/local/bin/curl
ls: /usr/local/bin/curl: No such file or directory
$ which -a curl
/usr/bin/curl
$ ls -l /usr/local/opt/curl/bin
total 648
-r-xr-xr-x@ 1 pirado  admin  308000 Jul  3 02:34 curl
-r-xr-xr-x@ 1 pirado  admin    6342 Jul  3 02:34 curl-config
-r-xr-xr-x@ 1 pirado  admin   11605 Jul  3 02:34 wcurl

$ which -a git
/usr/local/bin/git
/usr/bin/git
$ ls -l /usr/local/bin/git
lrwxr-xr-x@ 1 pirado  admin  28 Jul  3 02:36 /usr/local/bin/git -> ../Cellar/git/2.55.0/bin/git
```

Homebrew's `curl` 8.21.0 is installed and is **not** on `PATH`; the system
`curl` wins. Homebrew's `git` 2.55.0 shadows `/usr/bin/git` purely by PATH
order.

### E8 — Versioned executable names under Homebrew (observed)

Three `python@3.x` formulae are installed, none keg-only, **all three linked**:

```
$ ls -l /usr/local/bin | grep -E 'python|pip|idle|pydoc|wheel'
lrwxr-xr-x@ 1 pirado admin 40 Aug 17 07:35 python3        -> ../Cellar/python@3.14/3.14.7/bin/python3
lrwxr-xr-x@ 1 pirado admin 44 Aug 17 07:36 python3.12     -> ../Cellar/python@3.12/3.12.14/bin/python3.12
lrwxr-xr-x@ 1 pirado admin 44 Aug  7 12:27 python3.13     -> ../Cellar/python@3.13/3.13.15/bin/python3.13
lrwxr-xr-x@ 1 pirado admin 43 Aug 17 07:35 python3.14     -> ../Cellar/python@3.14/3.14.7/bin/python3.14
lrwxr-xr-x@ 1 pirado admin 37 Aug 17 07:35 pip3           -> ../Cellar/python@3.14/3.14.7/bin/pip3
lrwxr-xr-x@ 1 pirado admin 41 Aug 17 07:36 pip3.12        -> ../Cellar/python@3.12/3.12.14/bin/pip3.12
lrwxr-xr-x@ 1 pirado admin 41 Aug  7 12:27 pip3.13        -> ../Cellar/python@3.13/3.13.15/bin/pip3.13
lrwxr-xr-x@ 1 pirado admin 40 Aug 17 07:35 pip3.14        -> ../Cellar/python@3.14/3.14.7/bin/pip3.14
lrwxr-xr-x@ 1 pirado admin 38 Aug 17 07:35 idle3          -> ../Cellar/python@3.14/3.14.7/bin/idle3
lrwxr-xr-x@ 1 pirado admin 43 Aug 17 07:36 2to3-3.12      -> ../Cellar/python@3.12/3.12.14/bin/2to3-3.12
```

The suffixed names (`python3.12`, `pip3.13`, …) coexist. The **unsuffixed**
names (`python3`, `pip3`, `idle3`, `pydoc3`, `wheel3`, `python3-config`) are
all owned by `python@3.14` — the last formula to win the link.

### E9 — Homebrew versioned formulae (cited)

<https://docs.brew.sh/Versions>, read 2026-09-02:

> "[homebrew/core](https://github.com/homebrew/homebrew-core) supports multiple
> versions of formulae by using a special naming format such as `foo@1.2.rb`
> and a corresponding class name such as `FooAT12`."

> "Versioned formulae should only be linkable at the same time as their
> non-versioned counterpart if the upstream project provides support for it,
> e.g. by using suffixed binaries. If this is not possible, use
> `keg_only :versioned_formula` to allow users to have multiple versions
> installed at once."

> "Versioned formulae must be ABI-stable for the lifetime of the version
> branch. Updates to the versioned formula must not introduce ABI
> incompatibilities or otherwise require dependents to be revision bumped."

### E10 — Homebrew keg-only (cited)

<https://docs.brew.sh/FAQ> ("What does keg-only mean?"), read 2026-09-02:

> "It means the formula is installed only into the Cellar and is not linked
> into the default prefix. This means most tools will not find it."

> "You can modify a tool's build configuration to find keg-only dependencies."

> "Or, you can link in the formula if you need to with `brew link <formula>`,
> though this can cause unexpected behaviour if you are shadowing macOS
> software."

### E11 — Homebrew prefix and PATH (cited)

<https://docs.brew.sh/Installation>, read 2026-09-02:

> "The script installs Homebrew to its default prefix (`/opt/homebrew` for
> Apple Silicon, `/usr/local` for macOS Intel and `/home/linuxbrew/.linuxbrew`
> for Linux)"

> "When you install Homebrew, it prints some directions for updating your
> shell's config. If you don't follow those directions, Homebrew will not
> work."

Observed on this machine — `/usr/local/bin` is on the system default `PATH`
without any shell configuration, because it is the first line of `/etc/paths`:

```
$ cat /etc/paths
/usr/local/bin
/System/Cryptexes/App/usr/bin
/usr/bin
/bin
/usr/sbin
/sbin

$ echo "$PATH" | tr ':' '\n' | head -12
/Users/pirado/.local/share/sonarqube-cli/bin
/Users/pirado/.grok/bin
/Users/pirado/.bun/bin
/Users/pirado/bin
/Library/Java/JavaVirtualMachines/jdk-26.jdk/Contents/Home/bin
/Users/pirado/.local/bin
/usr/local/bin
/usr/local/sbin
/System/Cryptexes/App/usr/bin
/usr/bin
/bin
/usr/sbin
```

Six user-owned directories precede `/usr/local/bin` on this machine's `PATH`.

### E12 — MacPorts prefix and PATH (cited)

<https://guide.macports.org/chunked/installing.macports.html>, read 2026-09-02:

> users "do not wish to install it to `/opt/local/`, the default MacPorts
> location."

> "Only one copy of MacPorts may use the default prefix `/opt/local`, so for
> additional installations use the option `--prefix`."

<https://guide.macports.org/chunked/installing.shell.html>, read 2026-09-02:

> `export PATH=/opt/local/bin:/opt/local/sbin:$PATH`

> the MacPorts paths are placed at the front so that "the MacPorts binaries
> will take precedence over vendor-supplied binaries."

> `export MANPATH=/opt/local/share/man:$MANPATH`

> The postflight script may modify `.zprofile`, `.profile`, `.bash_login`,
> `.bash_profile`, `.tcshrc`, or `.cshrc`, depending on which ones exist in the
> home directory.

### E13 — MacPorts `port select`: the man page (cited)

`doc/port.1` in `macports/macports-base`, fetched via
`gh api repos/macports/macports-base/contents/doc/port.1`, read 2026-09-02
(roff source, lines 687–709, verbatim including escapes):

```
select
.RS 4
For a given group, selects a version to be the default by creating appropriate symbolic links\&. For instance, python might be linked to python2\&.6\&. Available select groups are installed as subdirectories of ${prefix}/etc/select/ and can be listed using
\fB\-\-summary\fR\&. To list the available versions in a group, use
\fB\-\-list\fR\&. To see which version is currently selected for a group, use
\fB\-\-show\fR\&. To change the selected version for a group, use
\fB\-\-set\fR\&.
.sp
For example:
.sp
.nf
port select \-\-summary
port select \-\-show python
port select \-\-list python
port select \-\-set python python34
.fi
.RE
```

`grep -n -i 'privileg\|elevat\|as root\|superuser'` over the whole of `port.1`
returns **nothing**. The only `sudo` occurrences are in the `sync`
(`sudo port -d sync`) and `selfupdate` (`sudo port selfupdate`) examples.

### E14 — MacPorts `port select`: the implementation (cited)

`proc mportselect` in `src/macports1.0/macports.tcl` of
`macports/macports-base`, fetched via `gh api`, read 2026-09-02. Verbatim
extract:

```tcl
# mportselect
#   * command: The only valid commands are list, set, show and summary
#   * group: This argument should correspond to a directory under
#            ${macports::prefix}/etc/select.
#   * version: This argument is only used by the 'set' command.
# On error mportselect returns with the code 'error'.
proc mportselect {command {group ""} {version {}}} {
    ui_debug "mportselect \[$command] \[$group] \[$version]"

    global macports::prefix
    set conf_path ${prefix}/etc/select/$group
    if {![file isdirectory $conf_path]} {
        return -code error "The specified group '$group' does not exist."
    }
```

The `set` branch, verbatim:

```tcl
        set {
            # Use ${conf_path}/$version to read in sources.
            if {$version eq "" || $version eq "base" || $version eq "current"
                    || [catch {set src_file [open "${conf_path}/$version"]} result]} {
                ui_debug "${result}: $::errorInfo"
                return -code error "The specified version '$version' is not valid."
            }
            set srcs [split [read -nonewline $src_file] \n]
            close $src_file

            # Use ${conf_path}/base to read in targets.
            if {[catch {set tgt_file [open ${conf_path}/base]} result]} {
                ui_debug "${result}: $::errorInfo"
                return -code error [concat "The configuration file" \
                                           "'${conf_path}/base' could not be" \
                                           "opened."]
            }
            set tgts [split [read -nonewline $tgt_file] \n]
            close $tgt_file

            # Iterate through the configuration files executing the specified
            # actions.
            set i 0
            foreach tgt $tgts {
                set src [lindex $srcs $i]

                switch -glob -- $src {
                    - {
                        # The source is unavailable for this file.
                        set tgt [file join $prefix $tgt]
                        file delete $tgt
                        ui_debug "rm -f $tgt"
                    }
                    /* {
                        # The source is an absolute path.
                        set tgt [file join $prefix $tgt]
                        file delete $tgt
                        file link -symbolic $tgt $src
                        ui_debug "ln -sf $src $tgt"
                    }
                    default {
                        # The source is a relative path.
                        set src [file join $prefix $src]
                        set tgt [file join $prefix $tgt]
                        file delete $tgt
                        file link -symbolic $tgt $src
                        ui_debug "ln -sf $src $tgt"
                    }
                }
                incr i
            }

            # Update the selected version.
            set selected_version ${conf_path}/current
            file delete $selected_version
            symlink $version $selected_version
            return
        }
        show {
            set selected_version ${conf_path}/current

            if {[catch {file type $selected_version} err]} {
                # this might be okay if nothing was selected yet,
                # just log the error for debugging purposes
                ui_debug "cannot determine selected version for $group: $err"
                return none
            } else {
                return [file readlink $selected_version]
            }
        }
```

The `list` branch excludes the two reserved names:

```tcl
            foreach v $versions {
                # Only the file name corresponds to the version name.
                set v [file tail $v]
                if {$v eq "base" || $v eq "current"} {
                    continue
                }
                lappend lversions [file tail $v]
            }
            return [lsort $lversions]
```

### E15 — MacPorts: which ports participate in a select group (cited)

`_resources/port1.0/group/select-1.0.tcl` in `macports/macports-ports`, read
2026-09-02, provides "the port selection mechanism exposed by the `port select`
command". It declares:

```tcl
options select.group select.file select.entries
default select.group {}
default select.file {}
default select.entries {}
```

and installs configuration into `${destroot}${prefix}/etc/select/$group/$name`
via `set selectFile ${destroot}${prefix}/etc/select/$group/$name`, with
`xinstall -m 0755 -d` for the directory and `xinstall -m 0644 $file $selectFile`
for the file. The portgroup itself creates **no** symlinks; that is
`mportselect`'s job (E14).

Participation is opt-in per Portfile. GitHub code search over
`macports/macports-ports` on 2026-09-02 reports:

```
total_count for "PortGroup select":  38
total_count for "select.group":     164
```

Sample hits: `python/py-hy/Portfile`, `python/py-mypy/Portfile`,
`java/maven3/Portfile`, `java/maven4/Portfile`, `sysutils/qt4_select/Portfile`.
**GitHub's code-search index is not guaranteed exhaustive; treat these counts
as a lower-bound order of magnitude, not a census.** The authoritative list on
any given machine is `port select --summary`, which is category (3) — runtime
detection.

### E16 — Which of these read commands work offline

Re-run with the network black-holed by pointing all proxy variables at a closed
port (`http_proxy=https_proxy=all_proxy=http://127.0.0.1:9`), with the quiet
Homebrew variables from E0 still set:

```
$ brew --prefix git
/usr/local/opt/git
exit=0
$ brew --prefix ripgrep          # not installed
/usr/local/opt/ripgrep
exit=0
$ brew list --formula | wc -l
     180
$ brew info --json=v2 --installed | wc -c
  876713
(stderr: empty)
```

Corroborating: the install receipt records the metadata source as a local cache
file.

```
$ python3 -c "import json;d=json.load(open('/usr/local/Cellar/git/2.55.0/INSTALL_RECEIPT.json'));..."
['aliases', 'arch', 'built_as_bottle', 'built_on', 'changed_files', 'compiler',
 'homebrew_version', 'installed_on_request', 'loaded_from_api',
 'loaded_from_internal_api', 'poured_from_bottle', 'runtime_dependencies',
 'source', 'source_modified_time', 'time', 'unused_options', 'used_options']
installed_on_request True
installed_as_dependency None
{'poured_from_bottle': True, 'installed_on_request': True, 'time': 1783056973,
 'source': {'spec': 'stable', 'versions': {...}, 'path':
 '/Users/pirado/Library/Caches/Homebrew/api/internal/packages.tahoe.jws.json',
 'tap_git_head': None, 'tap': 'homebrew/core'}}
```

---

## Findings

1. **Neither provider has a native mechanism that can express a cross-provider
   package primary.** Both are confined to their own prefix by construction.
   `mportselect` joins every symlink source and target with `${prefix}`
   (E14: `set src [file join $prefix $src]`, `set tgt [file join $prefix $tgt]`;
   the absolute-source branch is still written into a target under `$prefix`).
   `brew link` "Symlink[s] all of formula's installed files into Homebrew's
   prefix" (E4). Neither can point `git` at the other provider's binary. The
   **MUST** in `05` — "prefer a provider-native mechanism … wherever the
   provider offers one that does the job" — is therefore **satisfied
   vacuously for the cross-provider case**: no native mechanism does that job.

2. **`port select` is per-group, opt-in, and coarse.** It sets a whole
   *group* (`python`, `gcc`, `postgresql`) to a whole *version*, by replaying a
   pairwise list of target/source paths from
   `${prefix}/etc/select/<group>/base` and `${prefix}/etc/select/<group>/<version>`
   (E13, E14). A group exists only if some port installed it via the `select`
   portgroup (E15); on the order of a hundred-plus Portfiles do so out of a
   tree of tens of thousands. There is no group for `git`. It is **not** a
   global priority: it touches only the paths listed in that group's `base`
   file (INV-12 is respected by construction). It does **not** transfer
   dependency ownership: it only rewrites symlinks (INV-11 respected).

3. **`port select` has excellent native state and drift semantics.**
   `--set` is a pure overwrite (`file delete $tgt` then
   `file link -symbolic $tgt $src`), so it is idempotent (INV-16) by
   construction; the chosen version is recorded as the `current` symlink and
   read back verbatim by `--show`, which returns the string `none` when nothing
   is selected (E14). MacPortsBrew can therefore read the current selection
   without guessing. **However**, the `set` branch never checks that `$src`
   exists — uninstalling the selected port leaves the group's symlinks
   dangling and `current` still naming the departed version. Drift is
   detectable (compare `--show` against `--list` and stat each target) but is
   not self-healing.

4. **`port select --set` writes into the MacPorts prefix, which in a default
   installation means elevation.** Evidence: E14 shows writes to
   `${prefix}/bin`-style targets and to `${prefix}/etc/select/<group>/current`;
   E12 establishes `/opt/local` as the default prefix. I could **not** find an
   official statement that `port select --set` requires `sudo`: `port.1`
   contains no privilege language at all (E13). See *Cannot determine*. This
   matters because INV-06 says nothing MacPortsBrew does needs elevation —
   which means MacPortsBrew must **propose** a `sudo port select --set …`
   command for the user to run, not run it.

5. **`brew link` is not a primary-selection mechanism; it is an
   all-or-nothing keg-level operation.** It links *all* of a formula's files
   (E4) — binaries, man pages, headers, libs — so using `brew unlink git` to
   let a MacPorts `git` win also removes Homebrew's `git` man pages,
   `git-*` helper binaries and `libexec` links. And it is not per-executable:
   on this machine one formula (`git`) accounts for one entry in
   `/usr/local/bin` but `python@3.14` accounts for at least ten (E8).

6. **Homebrew's link state is machine-readable, and there are two sources.**
   The documented one is `linked_keg` in `brew info --json=v2` (E5): non-null
   for a linked formula (`git` → `"2.55.0"`), `null` for keg-only formulae and
   for genuinely unlinked ones. The undocumented one is the directory
   `${prefix}/var/homebrew/linked/<name>` → `../../../Cellar/<name>/<version>`
   (E5); it is not mentioned anywhere in `man brew`. The two agree exactly on
   this machine (160 entries = 180 − 19 keg-only − 1 unlinked). **Use
   `linked_keg`; treat the directory as an offline cross-check only.**

7. **`brew --prefix <formula>` tells you nothing about installation or link
   state.** It returned `/usr/local/opt/ripgrep` for a formula that is not
   installed, exit 0 (E6), which matches the manpage: "If formula is provided,
   display the location where formula is or would be installed" (E2). It also
   returned `/usr/local/opt/curl` for a keg-only unlinked formula and
   `/usr/local/opt/pydantic` for a non-keg-only unlinked one. It is a **path
   computation**, not a query. Any adapter that infers "installed" or "linked"
   from `brew --prefix <formula>` is wrong.

8. **The `opt` prefix is the stable path and it is the correct shim target.**
   The glossary calls it "a symlink to the active version of a keg" (E3), and
   `/usr/local/opt/pydantic → ../Cellar/pydantic/2.13.5` while
   `/usr/local/Cellar/pydantic/` still holds `2.13.4` (E6) — the `opt` symlink
   followed the upgrade. A shim pointing at
   `/usr/local/opt/<formula>/bin/<exe>` survives a Homebrew version bump; a
   shim pointing at `/usr/local/Cellar/<formula>/<version>/bin/<exe>` breaks on
   every upgrade. Note the "active" in the glossary means *active keg in the
   rack*, **not** *linked into `bin`* — `pydantic` has an `opt` symlink and no
   `bin` symlinks.

9. **Three distinct "not on PATH" states exist for a Homebrew formula, and
   they are not interchangeable.** (a) *keg-only by design* — 19 of 180 here,
   with a machine-readable reason (`:provided_by_macos` ×11,
   `:shadowed_by_macos` ×4, `:versioned_formula` ×3, and one free-text "it
   conflicts with PostgreSQL") (E5); `curl` is installed at 8.21.0 and
   `which -a curl` returns only `/usr/bin/curl` (E7). (b) *not keg-only but
   currently unlinked* — `pydantic` here (E5), which is a state a user or a
   failed operation produced, not a policy. (c) *not installed at all* — for
   which `brew --prefix` still answers (E6, finding 7). A `doctor` that
   reports these three the same way is wrong.

10. **Versioned executable naming is the mechanism Homebrew actually relies on,
    and it is a formula-authoring decision, not a user-facing one.**
    docs.brew.sh/Versions: "Versioned formulae should only be linkable at the
    same time as their non-versioned counterpart if the upstream project
    provides support for it, e.g. by using suffixed binaries. If this is not
    possible, use `keg_only :versioned_formula`" (E9). Observed both outcomes
    on one machine: `python@3.12`/`@3.13`/`@3.14` are all linked and coexist
    via suffixed names, while `openjdk@21` is `keg_only :versioned_formula`
    (E5, E8). Consequence: for suffix-capable packages the *versioned name* is
    already an unambiguous, zero-configuration primary selector
    (`python3.12`), and the only contested name is the unsuffixed one
    (`python3` → `python@3.14` here, by last-link-wins). MacPortsBrew should
    prefer *recommending the versioned name* over rearranging any symlink,
    whenever the versioned name exists. This is the exact analogue of what
    `port select` automates on the MacPorts side (E13: "For instance, python
    might be linked to python2.6").

11. **`brew switch` does not exist in Homebrew 6.0.21** (E4). Any design that
    assumed a Homebrew command for switching the active keg of a rack is
    obsolete; the only lever is `brew link`/`unlink`.

12. **PATH order between `/usr/local/bin` and `/opt/local/bin` is exactly the
    thing INV-12 forbids using as a package primary.** MacPorts' own installer
    *prepends* `/opt/local/bin:/opt/local/sbin` so that "the MacPorts binaries
    will take precedence over vendor-supplied binaries" (E12), and
    `/usr/local/bin` is the first line of macOS's `/etc/paths` (E11). On a
    machine with both providers, whichever prefix is earlier wins **for every
    contested name at once** — 1024 names on this machine from Homebrew alone
    (E1). Reordering those two entries is a *global provider priority* change
    and must be its own command with its own impact preview, as `05` already
    requires. It is not a candidate mechanism for package primary.

13. **A shim directory must not be placed inside either provider's prefix, and
    `/usr/local/bin` is provably not a safe place to put anything.** 62 of
    1086 entries in `/usr/local/bin` were not put there by Homebrew (E1):
    Docker Desktop, Caskroom wrappers, npm globals, AWS CLI, PowerShell,
    VirtualBox, and 36 pip-installed console scripts. Worse, Homebrew's own
    detector for foreign content, `brew --prefix --unbrewed`, **reports only
    regular files and never symlinks** (E2: 1048 symlinks, 0 reported;
    36 of 37 regular files reported, the exception being Homebrew's own
    `.keepme`). So Homebrew cannot see foreign symlinks in its own `bin`, and a
    MacPortsBrew symlink placed there would be invisible to `brew doctor`-class
    checks and indistinguishable from a Cask link. Combined with `brew link
    --overwrite` ("Delete files that already exist in the prefix while
    linking", E4), a shim in `/usr/local/bin` can be silently deleted by a
    routine `brew` operation.

14. **Path-prefix ownership matching is confirmed to be a bad heuristic**, as
    `05` already says: `/usr/local/bin/pnpm`, `/usr/local/bin/docker` and
    `/usr/local/bin/ruff` all sit under Homebrew's prefix and none is a
    Homebrew formula's file (E1). Ownership must come from resolving the
    symlink and asking the provider about the *resolved* path — for Homebrew,
    a resolved path under `${prefix}/Cellar/<name>/<version>/` names the
    formula directly, which is why the `../Cellar/…` link shape (1024 of 1086
    entries) is the reliable signal, not the containing directory.

15. **All the Homebrew reads needed for this analysis work offline** (E16):
    `brew --prefix`, `brew --prefix <formula>` (installed or not),
    `brew list --formula`, and `brew info --json=v2 --installed` all returned
    identical results with every proxy variable pointed at a dead port, with
    empty stderr. `analyze`'s primary-provider reporting is therefore
    offline-safe on the Homebrew side. The MacPorts equivalents
    (`port select --summary`, `--list`, `--show`) read only files under
    `${prefix}/etc/select` (E14) and cannot require the network, but this is
    **cited from source, not observed** — MacPorts is not installed here.

16. **Provider record of user intent, for the record.** Homebrew's own record
    of what the user asked for is `installed_on_request` — a key of
    `INSTALL_RECEIPT.json`, surfaced as `installed[].installed_on_request` in
    `brew info --json=v2` (47 of 180 true here, E5/E16). The sibling field
    `installed_as_dependency` is `None` in the JSON and **absent from the
    receipt's key list entirely**; do not read it. This is a *user-intent*
    record, not a top-level position inferred from the dependency graph, and
    it is the one to use. (Full treatment belongs to R-1/R-2; recorded here
    because it surfaced while extracting `linked_keg` from the same document.)

### The comparison `05` asks for

Criteria taken verbatim from `05-environment-and-primary-provider.md`
("Evaluate each option against…" and the **MUST** list).

| Criterion | `port select` | `brew link`/`unlink` (+ PATH order) | MacPortsBrew shim dir | Report only |
|---|---|---|---|---|
| Can express *cross-provider* primary | **No** (F1, F2) | **No** (F1); only via global PATH reorder, which is INV-12 territory (F12) | **Yes** | n/a |
| Granularity | per *group* (python, gcc…), opt-in, ~10² ports (F2) | per *formula*, all files at once (F5) | per *executable* | n/a |
| Survives a provider upgrade | Yes — config reinstalled by the port; `current` persists (F3) | Homebrew relinks the new keg on upgrade, overwriting a manual choice — **UNRESOLVED for the unlinked case** | Yes **if** it targets `opt/` (Homebrew) or `${prefix}/bin` (MacPorts), not the versioned keg (F8) | n/a |
| Survives uninstall of the target | No — dangling symlinks, `current` stale (F3) | Homebrew removes its own links on uninstall | No — needs `doctor` validation, which `05` already mandates | n/a |
| Visible to a confused user in 6 months | Moderate — `port select --show` exists but users forget the group name | Poor — a missing `/usr/local/bin/git` looks like a broken install | **Good** — one clearly named directory, one clearly named PATH entry in one managed block | Best (nothing changed) |
| Reversible in one command | Yes (`--set` to the other version) | Yes (`brew link <f>`), but needs elevation-free write to the prefix and loses nothing only if nothing else changed | Yes (delete the symlink / remove the directory / `env --revert`) | n/a |
| Idempotent (INV-16) | Yes, by construction — delete-then-link (F3) | Yes (`brew link` on a linked formula is a no-op) | Yes, by construction | Yes |
| Drift detection | Native and cheap: `--show` vs `--list` + stat targets (F3) | `linked_keg` in JSON, plus `${prefix}/var/homebrew/linked/` (F6) | Stat each symlink target; compare with the provider's current `opt`/`bin` path | Trivially |
| Effect on `which git` | None — no `git` select group exists | `brew unlink git` removes `/usr/local/bin/git`, so `/usr/bin/git` or `/opt/local/bin/git` wins (E7 shows the mechanism) | Shim dir first on PATH → `which git` shows the shim path | None |
| Breaks tools resolving by absolute path | Yes, if they hardcode `${prefix}/bin/python` — that path is what moves | Yes — `/usr/local/bin/<x>` disappears | **No** — provider paths untouched; only a new directory is added | No |
| Needs elevation | Yes in a default `/opt/local` install (F4, UNRESOLVED) | No (Homebrew prefix is user-writable here: entries are `pirado:admin`, E1) | **No** — `~/.local/share/…` | No |
| Shell rc interaction | None (MacPorts already owns its PATH entry, E12) | None for `link`; a PATH reorder means editing the user's own lines — forbidden by `05` ("Never reorder entries the user placed themselves") | One PATH prepend inside the managed block; no reordering of user lines | None |
| Per-package vs global | Per group (INV-12 ok) | Per formula (INV-12 ok); PATH reorder is **global** (INV-12 violation if used as primary) | Per executable (INV-12 ok) | n/a |
| Transfers dependency ownership | No (INV-11 ok) | No (INV-11 ok) | No (INV-11 ok) | No |
| Symmetric between directions (INV-17) | MacPorts only | Homebrew only | **Yes** — same mechanism both ways | Yes |
| Can be clobbered by the provider | No | n/a | Only if placed in a provider prefix — hence F13's placement rule | No |

### Recommendation

**A layered answer, not a single mechanism. Recommended, in priority order:**

1. **Default: report only.** `analyze` and `doctor` explain the current
   resolution — for each contested executable, the winning path, its owner
   classified per `05`, and the losers in PATH order — and change nothing.
   This is offline-safe on the Homebrew side (F15) and is the correct default
   because it is the only option with no failure mode.

2. **When the packages provide versioned names, recommend the versioned name.**
   `python3.12`, `gcc-14`. Both providers converge on this: Homebrew requires
   it before a versioned formula may be linked alongside its unversioned
   counterpart (F10), and `port select`'s stated purpose is automating the same
   choice (E13). This costs nothing, is invisible to no one, and cannot drift.
   Offer it before offering to change any symlink.

3. **For an intra-MacPorts version choice within an existing select group,
   delegate to `port select`.** This is `05`'s "prefer a provider-native
   mechanism" applied where it genuinely applies. MacPortsBrew **proposes** the
   exact command (`sudo port select --set python python312`), shows the impact
   by reading `${prefix}/etc/select/<group>/base`, and lets the user run it —
   because it needs elevation and INV-06 says MacPortsBrew does not elevate
   (F4). MacPortsBrew then re-reads `port select --show <group>` to verify.

4. **For a genuine cross-provider package primary, use the MacPortsBrew shim
   directory** — the **HYPOTHESIS** in `05` is confirmed as the right choice,
   because it is the only option that can express the requirement at all (F1)
   and the only one that is symmetric between the two migration directions
   (INV-17). Constraints established by this note:
   - Location: `~/.local/share/macportsbrew/bin` (or `$XDG_DATA_HOME`
     equivalent). **Never inside `/usr/local`, `/opt/homebrew` or `/opt/local`**
     — F13 shows both that the prefix is a contended namespace and that
     Homebrew cannot even see foreign symlinks there, while
     `brew link --overwrite` can delete them.
   - Target: the provider's **stable** path, never the versioned keg —
     `/usr/local/opt/<formula>/bin/<exe>` for Homebrew (F8),
     `${prefix}/bin/<exe>` for MacPorts.
   - PATH: exactly one prepend, inside the single managed block `05` already
     specifies, and only of the shim directory. Never reorder
     `/usr/local/bin` against `/opt/local/bin` to achieve a package primary —
     that is global priority (F12) and a separate command with a separate
     confirmation.
   - `doctor` validates every shim on every run: target exists, target still
     belongs to the provider MacPortsBrew recorded, and no shim is orphaned by
     an uninstall.

5. **Never** use `brew link --force` on a keg-only formula as a primary
   mechanism. docs.brew.sh warns it "can cause unexpected behaviour if you are
   shadowing macOS software" (E10), and 15 of the 19 keg-only formulae here are
   keg-only precisely because macOS provides or shadows them (E5). Report the
   keg-only status and the `opt` path; let the user decide.

**Why not the alternatives `05` listed:** per-shell aliases and functions were
not separately investigated in depth, but they fail two of `05`'s MUSTs on
their face — they are not visible to non-interactive shells or to tools that
`exec` a binary, and they cannot be validated against stale targets by
inspecting the filesystem. "Doing nothing and reporting" is not rejected; it is
recommendation 1, the default.

---

## Cannot determine

- **UNRESOLVED — does `port select --set` require `sudo`?** No official
  statement found: `port.1` contains no privilege language at all (E13), and
  the guide pages fetched do not cover `port select`. The source shows writes
  under `${prefix}` (E14) and the default prefix is `/opt/local` (E12), which
  in a stock installation is root-owned — but I have not *cited* the ownership,
  and MacPorts is not installed here to observe it. **A future attempt needs:**
  a machine with MacPorts installed, then `ls -ld /opt/local/bin
  /opt/local/etc/select` and `id -u`, plus `port select --set` run as an
  unprivileged user to capture the exact error. All read-only except the last,
  which is mutating and must not be run by an agent.
- **UNRESOLVED — what `brew upgrade` does to a formula the user deliberately
  unlinked.** Whether the new keg is relinked (destroying the choice) or the
  unlinked state is preserved is not stated in `man brew` and cannot be
  observed without running `brew upgrade`, which is mutating and forbidden
  here. **A future attempt needs:** the relevant `Formula`/`Keg` code in the
  `Homebrew/brew` repository (the `link` step of `FormulaInstaller`), which is
  a citable source and does not require running anything.
- **trac.macports.org could not be read.** Both `trac.macports.org/wiki/…`
  fetches returned an "Access Denied" interstitial (bot protection). Every
  MacPorts citation here therefore comes from `guide.macports.org` and the
  `macports/macports-base` and `macports/macports-ports` repositories, all of
  which are official per `docs/research/README.md`. **A future attempt needs:**
  a fetch path that trac does not block, or the same facts from the
  repositories.
- **The exhaustive list of MacPorts select groups** is not determinable
  offline or from GitHub search (E15). It is category (3): read
  `port select --summary` at runtime.
- **Whether `port select --show` is safe to call for a group that does not
  exist** — the source returns a Tcl error (E14, `"The specified group '$group'
  does not exist."`), so the adapter must treat a non-zero exit as "no group",
  not as a failure. Not observed.
- **Cask-provided binaries in `/usr/local/bin`** (gcloud, docker, meld, gimp
  here) are not covered by `--unbrewed` and were not chased down to a
  cask-ownership query. That belongs to R-1/R-2, not here.

---

## Impact on the specification

Nothing in `docs/spec/` is contradicted. Four statements should be **promoted
or sharpened**; all are Tier 3–5, so each needs a `spec-evolution.md` entry.
I have not edited `docs/spec/`.

1. **`05-environment-and-primary-provider.md`, "Implementing package primary"
   — the HYPOTHESIS can be resolved.** The shim directory is confirmed as the
   only mechanism that can express a cross-provider package primary (F1), and
   the research this section demanded is now done. Proposed replacement shape:
   the layered recommendation above (report-only default → versioned names →
   `port select` for intra-MacPorts groups → shim for cross-provider), plus the
   three hard constraints from F8 and F13 (target the `opt`/`bin` stable path;
   never place the shim inside a provider prefix; one PATH prepend in the
   managed block). Tier 5 → Tier 3/4.

2. **`05`, same section, "Prefer a provider-native mechanism… Owning less is
   better."** This MUST is correct and should stay, but as written it invites a
   future agent to conclude that `brew link` or `port select` must be used for
   package primary. It should be qualified: the preference applies **where a
   native mechanism can express the requirement**, and neither can express a
   cross-provider choice (F1). Recommend adding that sentence.

3. **`05`, "Executable ownership".** The MUST already says path-prefix matching
   is a fallback heuristic that must be marked as such; F14 is direct evidence
   (`/usr/local/bin/pnpm`, `/usr/local/bin/docker`, `/usr/local/bin/ruff`).
   Recommend citing this note, and adding that Homebrew's own
   `brew --prefix --unbrewed` is **not** a sufficient oracle because it reports
   regular files only and never symlinks (F13) — an agent could reasonably
   assume otherwise from its one-line manpage description.

4. **`05`, "Two different concepts".** F12 gives the concrete number that makes
   the distinction concrete: reordering `/usr/local/bin` against
   `/opt/local/bin` moves 1024 names at once on this machine. Worth citing in
   the impact-preview requirement, as evidence for why global priority needs a
   stronger confirmation.

Also worth recording, though it belongs to other items:

- **R-1 (`docs/research/R-1-homebrew-read-interfaces.md`)** should gain F15:
  `brew --prefix`, `brew --prefix <formula>`, `brew list --formula` and
  `brew info --json=v2 --installed` are all **offline-safe** with the three
  quiet variables set, verified by black-holing the proxies (E16). This is a
  direct partial answer to R-1's open question, obtained without triggering the
  tap clone R-1 warns about.
- **R-2/domain model**: `brew --prefix <formula>` must not be used as an
  installed-or-linked query (F7); `linked_keg` is the documented signal (F6);
  the three distinct not-on-PATH states (F9) need distinct representations in
  the domain model.

---

## Re-check when

- **Homebrew's major version changes** — `linked_keg`, `keg_only_reason` and
  the `--json=v2` shape are the load-bearing fields; `brew switch` was removed
  once and `--unbrewed`'s semantics are undocumented.
- **`${prefix}/var/homebrew/linked/` is undocumented** and may move without
  notice. If MacPortsBrew ever reads it, that read needs a version guard and a
  fallback to `linked_keg`.
- **MacPorts is installed on a test machine** — at that point every MacPorts
  claim in this note should be re-run as an observation, and the two
  `UNRESOLVED` items above closed. Until then this note's MacPorts half is
  cited-only.
- **`select-1.0.tcl` or `mportselect` changes** — both were read at
  `master` on 2026-09-02, not at a tag. Pin a commit if this note is ever
  used to justify parsing `${prefix}/etc/select/<group>/base` directly.
- **Not time-sensitive otherwise.** Nothing here depends on Homebrew's Intel
  support-tier deadline; the mechanisms compared are prefix-layout facts, not
  policy.
