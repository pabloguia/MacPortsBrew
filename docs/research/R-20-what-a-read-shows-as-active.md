# R-20 — Which read-only provider read proves that a package is the active (linked) installation, as rendered on this machine?

- **Status:** answered for what this machine holds; partially answered for the
  states it does not hold (an inactive MacPorts image, a MacPorts variant, the
  state right after a fresh install). See "Cannot determine".
- **Answered on:** 2026-10-18
- **Machine:** macOS 26.7.1 (25G241), x86_64. Homebrew 7.0.4 @ `/usr/local`
  (repository `/usr/local/Homebrew`, HEAD `edb70f031e4170c780799633a1226ff73e1077f4`,
  committed 2026-09-17; `homebrew/core` git revision `e045d7385fd`, last commit
  2026-09-02), 192 formulae and 9 casks installed, all from `homebrew/core` and
  the official cask tap, no third-party tap (`ls /usr/local/Homebrew/Library/Taps`
  lists only `homebrew`, which holds only `homebrew-core`, 2026-10-18; the
  listing verb itself was not run, because this repository's guard treats it
  as mutating). MacPorts 2.12.6 @ `/opt/local`,
  18 ports installed.
- **Blocking:** pass 08 slice 6 part B (`docs/passes/08-pre-flip-design-notes.md`,
  "Part B — `PROVIDER_REPORTS_ACTIVE` evaluable on `VERIFY_DESTINATION`"), whose
  prerequisite this note is. It is also the "`provider-researcher` note owed for
  both" in SE-094's `PROVIDER_REPORTS_ACTIVE` paragraph
  (`docs/spec-evolution.md`), and it bears on
  `docs/spec/04-migration-and-safety.md` destination check 1 ("reports the
  package as installed and active").
- **Nothing was mutated.** No install, uninstall, link, unlink, activate,
  deactivate, update, selfupdate, sync, upgrade or cleanup was run. The
  Homebrew reads did write to the user's cache, as E9 records.
- **Scratch:** every captured output lives in the session scratchpad
  (`.../scratchpad/r20/`), outside the repository.

## Question

`PROVIDER_REPORTS_ACTIVE` is unevaluable today (SE-094, pass 08 slice 4, D11)
because no read a plan carries shows "active". Part B proposes two success
semantics over two reads that are already on the allowlist: bare
`port -q installed` and `brew info --json=v2 --installed`. Before code is
written, this note asks what those reads actually print on this machine:

1. MacPorts: the byte-level rendering of `(active)`, whether any `(inactive)`
   marker exists, the separators and whitespace, what `-q` changes, and how a
   name that is a prefix of another looks.
2. Homebrew: the type and value of `linked_keg`, `keg_only` and
   `installed[].version` for a linked non-keg-only formula, a keg-only formula
   and an unlinked formula, whether `linked_keg` always equals an
   `installed[].version`, and what a cask entry carries.
3. Whether the Homebrew read, run as the project runs it, writes to the cache,
   and whether a `brew.env` is present (SE-016 item 3, R-15).
4. What the official documentation says `(active)` and `linked_keg` mean.

## Evidence

All times are 2026-10-18, local time (UTC-3).

### E0 — The environment every provider command ran in

The project's process executor builds an environment from nothing (R-15 E1).
These commands reproduce it with `env -i`. The invoking shell had
`HOMEBREW_NO_AUTO_UPDATE`, `HOMEBREW_NO_ANALYTICS` and `HOMEBREW_NO_ENV_HINTS`
set, and `env -i` drops them. `XDG_CONFIG_HOME` was unset.

MacPorts (quieted by argv, `-q`):
```
env -i PATH=/opt/local/bin:/usr/bin:/bin:/usr/sbin:/sbin LANG=C LC_ALL=C TERM=dumb NO_COLOR=1 HOME="$HOME" /opt/local/bin/port ...
```
Homebrew (quieted by environment; the six keys are `QUIETING_KEYS`,
`crates/adapters/src/homebrew/mod.rs`):
```
env -i PATH=/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin LANG=C LC_ALL=C TERM=dumb NO_COLOR=1 HOME="$HOME" \
  HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1 \
  HOMEBREW_NO_INSTALL_CLEANUP=1 HOMEBREW_NO_COLOR=1 HOMEBREW_NO_EMOJI=1 /usr/local/bin/brew ...
```

### E1 — `brew.env` candidates and the API cache before any `brew` ran (observed 22:40)

R-15 finding 1 lists the locations. With `XDG_CONFIG_HOME` unset, the user file
is `~/.homebrew/brew.env`. The `/opt/homebrew` path was added because it is
the other default prefix.
```
$ ls -la /etc/homebrew/brew.env /usr/local/etc/homebrew/brew.env /opt/homebrew/etc/homebrew/brew.env ~/.homebrew/brew.env
ls: /etc/homebrew/brew.env: No such file or directory
ls: /usr/local/etc/homebrew/brew.env: No such file or directory
ls: /opt/homebrew/etc/homebrew/brew.env: No such file or directory
ls: ~/.homebrew/brew.env: No such file or directory
```
The cache (`find ~/Library/Caches/Homebrew -maxdepth 3 -exec ls -ldT {} +`,
trimmed). It held 718 files and 37,616 KiB (`du -sk`):
```
drwxr-xr-x@ 4 ... Oct 18 21:53:28 2026 ~/Library/Caches/Homebrew
-rw-r--r--@ 1 ... 15568216 Oct 18 21:53:33 2026 .../api/internal/packages.tahoe.jws.json
-rw-r--r--@ 1 ... 14307794 Oct 18 21:53:35 2026 .../api/internal/packages.tahoe.jws.json.payload
-rw-r--r--@ 1 ...   454887 Oct 18 21:53:37 2026 .../api/internal/packages.tahoe.jws.json.payload.index
-rw-r--r--@ 1 ...   350163 Oct 18 21:53:37 2026 .../api/internal/executables.txt
drwxr-xr-x@ 3 ... Oct 18 21:53:21 2026 .../bootsnap
```
The API cache was present and non-empty, written at 21:53, before this note's
first command (22:40). What wrote it is not known (see "Cannot determine").
R-15 E4 gives the condition for a Ruby read to skip the download:
`skip_download?` returns `true` for a present, non-empty target when
`stale_seconds` is `nil`, which is the case under `HOMEBREW_NO_AUTO_UPDATE`
without `HOMEBREW_FORCE_API_AUTO_UPDATE`.

### E2 — Provider versions (observed)

```
$ port version            # E0 environment, no -q
Warning: port definitions are more than two weeks old, consider updating them by running 'port selfupdate'.
Version: 2.12.6
$ brew --version          # E0 environment
Homebrew 7.0.4
Homebrew/homebrew-core (git revision e045d7385fd; last commit 2026-09-02)
$ git -C /usr/local/Homebrew describe --tags
7.0.4
```

### E3 — `port -q installed` and `port installed`, byte level (observed 22:41)

Both exit 0. `port -q installed`: 18 lines on stdout, 0 bytes on stderr.
`port installed`: 19 lines on stdout, and one line on stderr (the "port
definitions are more than two weeks old" warning from E2). Every row of
`port -q installed` ends in `(active)`; `grep -vc '(active)'` gives `0`.
`cat -vet`, where `$` marks the end of a line:
```
  autoconf @2.73_0 (active)$
  automake @1.18.1_0 (active)$
  gettext @1.0_0 (active)$
  gettext-runtime @1.0_0 (active)$
  gettext-tools-libs @1.0_0 (active)$
  gperf @3.3_0 (active)$
  htop @3.5.2_0 (active)$
  jq @1.8.2_0 (active)$
  libiconv @1.19_0 (active)$
  libtextstyle @1.0_0 (active)$
  libtool @2.6.2_0 (active)$
  m4 @1.4.21_0 (active)$
  ncurses @6.6_0 (active)$
  oniguruma6 @6.9.10_0 (active)$
  pkgconfig @0.29.2_0 (active)$
  tree @2.3.2_0 (active)$
  xz @5.8.3_0 (active)$
  xz-bootstrap @5.8.3_1 (active)$
```
`od -c` of the first 120 bytes and the last 40 bytes:
```
0000000            a   u   t   o   c   o   n   f       @   2   .   7   3
0000020    _   0       (   a   c   t   i   v   e   )  \n           a   u
...
0000000    c   t   i   v   e   )  \n           x   z   -   b   o   o   t
0000020    s   t   r   a   p       @   5   .   8   .   3   _   1       (
0000040    a   c   t   i   v   e   )  \n
```
`port installed` (without `-q`) starts with the header
`The following ports are currently installed:\n`. After that header its body
matches `port -q installed` byte for byte:
`diff <(tail -n +2 port-installed.out) port-q-installed.out` reports no
difference.

### E4 — Named and pseudo-port reads (observed 22:41–22:42)

`port -q <args>`, E0 environment. Each command's stdout is shown under
`cat -vet`. Every command exits 0 with an empty stderr.
```
installed gettext          ->   gettext @1.0_0 (active)$
installed xz               ->   xz @5.8.3_0 (active)$
installed jq               ->   jq @1.8.2_0 (active)$
installed gettext xz       ->   gettext @1.0_0 (active)$
                                xz @5.8.3_0 (active)$
installed cowsay           -> (empty)
installed no-such-port-zz  -> (empty)
installed inactive         -> (empty)
echo inactive              -> (empty)
installed active           -> the same 18 rows as E3
installed XZ               ->   xz @5.8.3_0 (active)$
installed xz*              ->   xz @5.8.3_0 (active)$
                                xz-bootstrap @5.8.3_1 (active)$
installed name:xz          ->   xz @5.8.3_0 (active)$
                                xz-bootstrap @5.8.3_1 (active)$
```
The same arguments without `-q` (each exits 0, and each prints the stale
warning on stderr):
```
installed gettext          -> The following ports are currently installed:$
                                gettext @1.0_0 (active)$
installed cowsay           -> None of the specified ports are installed.$
installed no-such-port-zz  -> None of the specified ports are installed.$
installed inactive         -> None of the specified ports are installed.$
```

### E5 — MacPorts source on this machine, and the same files at the `v2.12.6` tag

`/opt/local/bin/port` is the installed `tclsh8.6` script
(`-r-xr-xr-x root wheel 210516 Aug 25 11:44`), read 2026-10-18,
`proc action_installed`. The relevant lines, verbatim:
```
    if { [llength $ilist] > 0 } {
        ui_notice "The following ports are currently installed:"
        foreach i [portlist_sortregrefs $ilist] {
            set extra ""
            if {[macports::ui_isset ports_verbose]} {
                ... append extra " requested_variants='$rvariants'"
                ... append extra " platform='$os_platform $os_major'"
                ... append extra " archs='$archs'"
                ... append extra " date='[clock format $date -format "%Y-%m-%dT%H:%M:%S%z"]'"
            }
            set activestr ""
            if {[$i state] eq "installed"} {
                set activestr " (active)"
            }
            puts "  [$i name] @[$i version]_[$i revision][$i variants]${activestr}${extra}"
        }
    } elseif { $restrictedList } {
        ui_notice "None of the specified ports are installed."
    } else {
        ui_notice "No ports are installed."
    }
```
Also cited from the tag:
<https://raw.githubusercontent.com/macports/macports-base/v2.12.6/src/port/port.tcl>,
read 2026-10-18. The fetch returned the same `activestr` and `puts` lines and
the same three `ui_notice` lines, and it reported that the literal
`(inactive)` does not appear in the part it read (characters 100,000–200,000
of 210,483; `action_installed` lies inside that range). The `port` script on
this machine was searched for the literal `(active)`:
`grep -n "(active)\|currently installed\|None of the specified ports are installed" /opt/local/bin/port`
matches lines 185, 189, 1115, 2342, 3119, 3143 and 3148. That search could not
find an `(inactive)` rendering by construction, so it was repeated for that
literal (2026-10-18, after review): `grep -n -F '(inactive)' /opt/local/bin/port`
matches only line 1116, `^(inactive)(@.*)?$`, the pseudo-port parser that turns
the word `inactive` on a command line into a set of ports — input, not output.
What the output can say is fixed by `action_installed` itself: lines 3141–3145
set `activestr` to `""` and then to `" (active)"` when the image is active, and
print `  [$i name] @[$i version]_[$i revision][$i variants]${activestr}${extra}`.

Registry state transitions, `/opt/local/libexec/macports/lib/registry2.0/portimage.tcl`
(installed copy, lines 892–895, 1068–1070, 1118). The same three matches were
returned from
<https://raw.githubusercontent.com/macports/macports-base/v2.12.6/src/registry2.0/portimage.tcl>,
read 2026-10-18:
```
                # Recording that the port has been activated should be done
                # here so that this information cannot be inconsistent with the
                # state of the files on disk.
                $port state installed
...
                # Update the port's state in the same transaction as the file
                # delete operations.
                $port state imaged
...
        $regref state imaged          (a newly registered image)
```

### E6 — MacPorts documentation (read 2026-10-18)

- `port-installed(1)`, the 2.12.6 copy on this machine
  (`/opt/local/share/man/man1/port-installed.1.gz`, footer
  `MacPorts 2.12.6`), rendered with `mandoc`:
  > port installed can be used in two separate ways: Either without arguments,
  > in which case it will print a list of all installed ports (both active and
  > inactive), or with arguments, which will return a list of installed ports
  > that match the argument expression.
  >
  > The output of port installed contains the name, version and variants of
  > the installed ports and whether the port is active or inactive.

  > -v  Print the platform at install time (i.e., your macOS version) and the
  > architecture(s) of the installed port.
  > -q  Do not print the header line. This is useful when parsing the output
  > of port installed in scripts for further processing.

  > PORT IMAGES ... Only one version can be active at a time. On activation,
  > files will be copied to their intended place in prefix.
- `port-activate(1)`, same source: "port activate will set the status of an
  previously installed version of a port to active ... If any version is active
  at the time of using port activate, it will be deactivated first."
- The MacPorts Guide, "Port Images",
  <https://guide.macports.org/chunked/internals.images.html>, read 2026-10-18:
  "a MacPorts port by default is not installed into its final or “activated”
  location", it is "only made available to other ports and end-users after an
  activation phase", and "Deactivating a port only removes the files from their
  activated locations (usually under `${prefix}`)". The page, as fetched, does
  not state how many versions may be active at once.

### E7 — `brew info --json=v2 --installed` (observed 22:42:44)

Exit 0. Stdout is 880,989 bytes and stderr is empty. A second run (E9) produced
byte-identical stdout (`cmp` reports no difference). The document was analysed
with a throwaway `python3 -I` script in the scratchpad. Its output, trimmed:
```
top keys: ['casks', 'formulae']
formulae 192 casks 9
type(linked_keg),type(keg_only): {('str', 'bool'): 172, ('NoneType', 'bool'): 20}
('linked',   'keg_only',     'lk_in_installed=True', 'n_installed=2') 1
    ('openssl@3', '3.6.4', ['3.6.3', '3.6.4'], ...)
('linked',   'not_keg_only', 'lk_in_installed=True', 'n_installed=1') 163
    ('actionlint', '1.7.12', ['1.7.12'], ...)
    ('argon2', '20190702_1', ['20190702_1'], ...)      # versions.stable is '20190702', revision 1
('linked',   'not_keg_only', 'lk_in_installed=True', 'n_installed=2') 8
    ('cryptography', '50.0.1', ['50.0.0', '50.0.1'], ...)
('unlinked', 'keg_only',     'lk_in_installed=None', 'n_installed=1') 16
    ('apr', None, ['1.7.6'], ...)   ('curl', None, ['8.21.0'], ...)
('unlinked', 'keg_only',     'lk_in_installed=None', 'n_installed=2') 2
    ('libffi', None, ['3.7.1', '3.8.0'], ...)   ('php@8.4', None, ['8.4.24', '8.4.25'], ...)
('unlinked', 'not_keg_only', 'lk_in_installed=None', 'n_installed=2') 2
    ('openssl@4', None, ['4.0.1', '4.0.2'], ...)   ('pydantic', None, ['2.13.4', '2.13.5'], ...)
linked_keg not equal to any installed version: []
installed[] keys: ['built_as_bottle', 'installed_on_request', 'poured_from_bottle', 'runtime_dependencies', 'time', 'used_options', 'version']
formula keys mentioning link/keg/active/pin: ['keg_only', 'keg_only_reason', 'link_overwrite', 'linked_keg', 'outdated', 'pinned']
cask keys mentioning link/activ/install: ['installed', 'installed_time']
names that are prefixes of other names: [('apr','apr-util'), ('gd','gdk-pixbuf'), ('gh','ghidra'), ('git','git-lfs'), ('git','gitleaks'), ('libev','libevent'), ('libssh','libssh2'), ('openjdk','openjdk@21'), ('tree','tree-sitter'), ('uv','uvwasi')]
full_name differs from name: []
```
`linked_keg` and `keg_only` are present on all 192 formula entries; the
analysis found no entry missing either key.

The five representative entries, verbatim from the document (JSON values):
```
gh        {"linked_keg": "2.98.0", "keg_only": false, "keg_only_reason": null,
           "installed[].version": ["2.98.0"], "installed_on_request": [true]}
openssl@3 {"linked_keg": "3.6.4", "keg_only": true,
           "keg_only_reason": {"reason": ":versioned_formula", "explanation": ""},
           "installed[].version": ["3.6.3", "3.6.4"], "installed_on_request": [false, false]}
openssl@4 {"linked_keg": null, "keg_only": false, "keg_only_reason": null,
           "installed[].version": ["4.0.1", "4.0.2"], "installed_on_request": [false, false]}
pydantic  {"linked_keg": null, "keg_only": false, "keg_only_reason": null,
           "installed[].version": ["2.13.4", "2.13.5"], "installed_on_request": [false, false]}
curl      {"linked_keg": null, "keg_only": true,
           "keg_only_reason": {"reason": ":provided_by_macos", "explanation": ""},
           "installed[].version": ["8.21.0"], "installed_on_request": [true]}
```
The cask entries, as `(token, type(installed), installed == version)`:
```
[('adoptopenjdk','str',True), ('copilot-cli','str',False), ('gcloud-cli','str',False),
 ('gimp','str',False), ('meld','str',True), ('oracle-jdk','str',False),
 ('visualdiffer','str',False), ('wailbrew','str',False), ('wireshark-chmodbpf','str',False)]
```
No cask key contains `link`, `activ` or `keg`. A cask's `installed` is a
version string, for example `"installed": "1.0.70"` against
`"version": "1.0.98"` for `copilot-cli`.

### E8 — The on-disk records behind E7 (observed 22:45, `ls -l` / `readlink`, no writes)

```
var/homebrew/linked/gh         -> ../../../Cellar/gh/2.98.0          opt/gh        -> ../Cellar/gh/2.98.0
var/homebrew/linked/openssl@3  -> ../../../Cellar/openssl@3/3.6.4    opt/openssl@3 -> ../Cellar/openssl@3/3.6.4
var/homebrew/linked/openssl@4: No such file or directory              opt/openssl@4 -> ../Cellar/openssl@4/4.0.2
var/homebrew/linked/pydantic:  No such file or directory              opt/pydantic  -> ../Cellar/pydantic/2.13.5
var/homebrew/linked/curl:      No such file or directory              opt/curl      -> ../Cellar/curl/8.21.0
var/homebrew/linked/libffi:    No such file or directory              opt/libffi    -> ../Cellar/libffi/3.8.0
bin/openssl -> ../Cellar/openssl@3/3.6.4/bin/openssl
bin/gh      -> ../Cellar/gh/2.98.0/bin/gh
bin/curl: No such file or directory
$ ls /usr/local/var/homebrew/linked | wc -l
     172
```
All paths are relative to `/usr/local`. The `var/homebrew/linked/openssl@3`
symlink has mtime 2026-09-01 20:02. `Cellar/openssl@4/4.0.2` has mtime
2026-09-01 19:53. `Cellar/pydantic/2.13.5` and `opt/pydantic` both have mtime
2026-09-01 21:43. `pydantic` 2.13.5's `INSTALL_RECEIPT.json` has
`installed_on_request: false` and `poured_from_bottle: false`.

Homebrew 7.0.4 source on this machine (`Library/Homebrew/formula.rb`, read
2026-10-18). The same text was returned from
<https://raw.githubusercontent.com/Homebrew/brew/7.0.4/Library/Homebrew/formula.rb>,
read 2026-10-18:
```ruby
  # The link status symlink directory for this {Formula}.
  # You probably want {#opt_prefix} instead.
  def linked_keg
    linked_keg = possible_names.map { |name| HOMEBREW_LINKED_KEGS/name }
                               .find(&:directory?)
    linked_keg || (HOMEBREW_LINKED_KEGS/name)
  end

  def linked? = linked_keg.exist?
  def optlinked? = opt_prefix.symlink?

  # {PkgVersion} of the linked keg for the formula.
  def linked_version
    return unless linked?
    Keg.for(linked_keg).version
  end
  ...
      "keg_only"                        => keg_only?,          # line 3115
      "keg_only_reason"                 => keg_only_reason&.to_hash,
      "linked_keg"                      => linked_version&.to_s, # line 3131
  ...
      return json_formula.dup.merge(
        hash.slice("name", "installed", "linked_keg", "pinned", "outdated"),   # to_hash_with_variations
      )
```
`Library/Homebrew/startup/config.rb` line 46:
`HOMEBREW_LINKED_KEGS = (HOMEBREW_PREFIX/"var/homebrew/linked").freeze`.

The adapter's inference that part B rejects is in
`crates/adapters/src/homebrew/json.rs` lines 421–426, read 2026-10-18:
```rust
    pub fn active_keg(&self) -> Option<&Keg> {
        match &self.linked_keg {
            Some(linked) => self.installed.iter().find(|k| &k.version == linked),
            None => self.installed.last(),
        }
    }
```

### E9 — What the Homebrew reads wrote to the cache (observed)

Method: `touch` a marker file in the scratchpad, wait one second, run the read,
then `find ~/Library/Caches/Homebrew -newer <marker>`, and diff a full
`ls -ldT` listing taken before and after. This method only reads the cache.

| Run (time) | Command | New or changed entries | Under `api/` |
|---|---|---|---|
| 1 (22:42:44) | `brew --version` then `brew info --json=v2 --installed` | 5 files, 21,832 bytes, all under `bootsnap/.../compile-cache-iseq/{33,36,72,ac,bf}/`, plus their 5 parent directories | 0 |
| 2 | `brew --version` alone | 0 | 0 |
| 3 | `brew info --json=v2 --installed` again | 0 | 0 |
| 4 (22:47:35) | `brew list --versions` for `gh`, `pydantic`, `openssl@3`, `openssl@4`, `curl` | 3 files, 34,056 bytes, under `bootsnap/.../compile-cache-iseq/{49,4c,53}/` | 0 |

`du -sk ~/Library/Caches/Homebrew` read 37,616 before run 1 and 37,728 after
it. Network traffic was not monitored.

### E10 — Homebrew's current verify read, for comparison (observed 22:47:35)

`brew list --versions <name>`, E0 environment. Every command exits 0 with an
empty stderr. Output under `cat -vet`:
```
gh        -> gh 2.98.0$
pydantic  -> pydantic 2.13.4 2.13.5$
openssl@3 -> openssl@3 3.6.4 3.6.3$
openssl@4 -> openssl@4 4.0.1 4.0.2$
curl      -> curl 8.21.0$
```

### E11 — Homebrew documentation (read 2026-10-18)

- "Querying `brew`", <https://docs.brew.sh/Querying-Brew>: "The schema itself is
  not currently documented outside of the code in formula.rb that generates
  it", and "fields may be added to the schema as needed without incrementing
  the schema". The page does not define `linked_keg`. It uses `linked_keg` in two
  recipes: "To find the names of linked keg-only formulae:"
  `brew info --json=v1 --installed | jq "map(select(.keg_only == true and .linked_keg != null) | .name)"`,
  and "To find the names of normal (not keg-only) formulae that are installed,
  but not linked to the shared directories:"
  `brew info --json=v1 --installed | jq "map(select(.keg_only == false and .linked_keg == null) | .name)"`.
- FAQ, <https://docs.brew.sh/FAQ>, "What does keg-only mean?": "It means the
  formula is installed only into the Cellar and is not linked into the default
  prefix. This means most tools will not find it. ... Or, you can link in the
  formula if you need to with `brew link <formula>`, though this can cause
  unexpected behaviour if you are shadowing macOS software."
- Manpage, <https://docs.brew.sh/Manpage> (first 100,000 of 134,331 characters
  read). `link`: "Symlink all of formula’s installed files or cask’s binaries,
  manpages and shell completions into Homebrew’s prefix. This is done
  automatically when you install formulae and casks but can be useful for
  manual installations." `link --force`: "Allow keg-only formulae to be
  linked." `unlink`: "Remove symlinks for formula or cask from Homebrew’s
  prefix." TERMINOLOGY: "a formula is keg-only if it is not symlinked into
  Homebrew’s prefix". `info --json`: "`v1` is valid for formula only. `v2` is
  valid for both formula and cask." `info --installed`: "If `--json` is passed,
  print JSON for installed formulae and, with `--json=v2`, installed casks."

### E12 — The repository's recordings (read 2026-10-18)

A `python3 -I` scan of every string in `fixtures/recordings/*.json` was run for
lines matching `^  \S+ @\S+`:
```
both-providers-arm64-macos15.json   rows 17  inactive ['  openssl3 @3.1.5_0']  variants ['  gnutls @3.8.4_0+universal (active)', '  wget @1.25.0_1+gnutls (active)']
both-providers-intel-macos26.json   rows 18  inactive []                       variants []
macports-only-intel-macos12.json    rows 13  inactive ['  openssl3 @3.1.5_0']  variants (same two)
```
`fixtures/recordings/README.md` classes `macports-only-intel-macos12.json` as
`synthesised` ("Written from documentation because no machine was available").
It classes `both-providers-arm64-macos15.json` as `derived`, taking its
MacPorts half from that synthesised fixture.

## Findings

### MacPorts

1. **`port -q installed` marks the active image, and only the active image.**
   One row is printed per registered image (E3, E5). The row is two spaces,
   the registry name, one space, `@`, the version, `_`, the revision, the
   variants appended with no separator, then ` (active)` (one space, the
   literal `(active)`) **if and only if** the registry state is `installed`.
   The row ends in `\n`, with no trailing whitespace (E3 `od -c`, `cat -vet`).
   The registry sets state `installed` in the same step that activates the
   files, and `imaged` when it deactivates them (E5, `portimage.tcl`). So the
   marker is the provider's own record of activation, not an inference.
2. **No `(inactive)` marker exists in 2.12.6.** An inactive image prints the
   same row without a suffix (E5: `action_installed` sets `activestr` to `""`
   or `" (active)"` only; the one `(inactive)` in the 2.12.6 script, line 1116,
   is the pseudo-port parser, not output). This is **read from the source, not
   observed**: this machine has no inactive image (finding 3). The man page's "whether the port is
   active or inactive" (E6) describes the same output. It does not describe a
   second marker. A reader that expects an `(inactive)` token would never see
   one.
3. **This machine has no inactive image.** All 18 rows carry `(active)` (E3),
   and `port -q installed inactive` and `port -q echo inactive` are both empty
   (E4). The inactive rows in `fixtures/recordings/` (`  openssl3 @3.1.5_0`)
   come from a synthesised fixture and a fixture derived from it (E12). Their
   shape agrees with the source in E5, but they are not an observation of the
   rendering.
4. **`-q` removes the header and the stale-ports warning, and changes no row.**
   Without `-q` stdout gains `The following ports are currently installed:`
   and stderr gains the warning. The rows are byte-identical (E3). For a name
   that matches nothing, `-q` turns `None of the specified ports are
   installed.` into empty stdout, and the exit status stays 0 (E4). This agrees
   with the `verify = "stdout_names_package"` rationale in
   `data/policy/provider-commands.toml`.
5. **`-v` would break a "last token is `(active)`" rule.** Under
   `ports_verbose` the `extra` string (`requested_variants=...`, `platform=...`,
   `archs=...`, `date=...`) is appended *after* `(active)` (E5). The project's
   argv never passes `-v`. Whether a configuration file can switch on verbose
   for a `-q` read was not determined.
6. **A named read is an exact match that ignores case, and prints the
   registry's spelling.** `installed xz` does not list `xz-bootstrap`, and
   `installed gettext` does not list `gettext-runtime` (E4). `installed XZ`
   prints `xz` (E4). A glob (`xz*`) or a `name:` expression (`name:xz`) widens
   the match to `xz-bootstrap` (E4). In the bare read, a name that is a prefix
   of another appears as its own row, and the two are told apart only by the
   whole first token (`xz` vs `xz-bootstrap`, E3).
7. **Token structure.** Port names and the `@version_revision+variants` field
   contain no spaces (E3, E5 `puts`, E12 variant rows). So a non-verbose row
   splits on whitespace into exactly three tokens if active
   (`name`, `@...`, `(active)`) and two if inactive. The two-token form comes
   from the source; it was not observed (finding 3).

### Homebrew

8. **`linked_keg` is Homebrew's own record of the linked keg.** It is the
   `PkgVersion` string of the keg that `<prefix>/var/homebrew/linked/<name>`
   resolves to, or `null` when that link does not exist (E8 source).
   `var/homebrew/linked` holds 172 entries, and the document holds 172 string
   `linked_keg` values (E7, E8). The documentation does not define the field.
   It says the schema is documented only in `formula.rb` (E11), so the source
   in E8 is the definition.
9. **Type and value.** On this machine `linked_keg` is a string or `null`,
   and is present on all 192 formulae. Every string equals one
   `installed[].version` exactly, including the `_<revision>` suffix: for
   `argon2` it is `20190702_1`, while `versions.stable` is `20190702` (E7).
   So the comparison must be against `installed[].version`, not
   `versions.stable`. (An earlier draft added that, with two kegs installed,
   it named the newer keg in all nine linked cases; E7's output as kept does
   not show that comparison, so the sentence is withdrawn.)
10. **`keg_only` does not predict `linked_keg`, in either direction.**
    - **Linked non-keg-only** (171): `gh`, `"2.98.0"`, `keg_only: false`.
      `bin/gh` points into that keg (E7, E8).
    - **Keg-only and linked** (1): `openssl@3`, `"3.6.4"`, `keg_only: true`
      (`:versioned_formula`). `bin/openssl` points into the 3.6.4 keg (E7, E8).
    - **Keg-only and unlinked** (18): for example `curl`, `null`, with no
      `bin/curl` (E7, E8).
    - **Not keg-only but unlinked** (2): `openssl@4` and `pydantic`, `null`,
      each with two kegs (E7, E8).

    The documentation treats both mixed cases as normal and gives a `jq`
    recipe for each (E11). `keg_only` is computed from the formula definition
    Homebrew loads now (`keg_only?`, E8). Whether that definition matched when
    `openssl@3` was linked is not known.
11. **`linked_keg` is not the opt link.** `opt/<name>` exists for unlinked
    and keg-only formulae as well. `opt/pydantic` points to 2.13.5 and
    `opt/openssl@4` to 4.0.2, while both have `linked_keg: null` (E8). The JSON
    does not report the opt link (E7: no `opt`-like key).
12. **The adapter's `active_keg` gives an "active" keg that Homebrew does not
    record as linked.** For `pydantic` and `openssl@4` it returns the last
    `installed[]` entry (2.13.5 and 4.0.2), because their `linked_keg` is
    `null` (E7, E8 `active_keg`). These are two concrete cases of the inference
    SE-094 and part B reject.
13. **A cask entry has nothing that means "linked" or "active".** No cask key
    contains `link`, `activ` or `keg`. `installed` is a version string and can
    differ from `version` (E7). The Manpage says `brew link` also symlinks a
    cask's binaries (E11), but the JSON reports no such state.
14. **The current verify read shows no linkage.** `brew list --versions`
    prints every installed keg in an order that is not linked-first and not
    sorted (`openssl@3 3.6.4 3.6.3`, `openssl@4 4.0.1 4.0.2`). It prints the
    same shape for linked, unlinked and keg-only formulae (E10). This confirms
    SE-094's "believed to show no linkage".
15. **Names.** Ten installed formula names are prefixes of other installed
    names, including `gh`/`ghidra` and `tree`/`tree-sitter` (E7). A match on
    `name` equality, not on a prefix, is what tells them apart. Here
    `full_name` equals `name` for every formula, because all of them are
    `homebrew/core` formulae (E7). `linked_keg` also follows a renamed
    formula's old names (`possible_names`, E8).

### The cache, the `brew.env`, and the network

16. **There is no `brew.env` in any location R-15 lists** (E1).
17. **The JSON read wrote to the user's cache.** On its first run in this
    session, `brew --version` followed by `brew info --json=v2 --installed`
    (run 1 in E9; the two were not separated, so the files cannot be pinned on
    the JSON read alone) wrote 5 bootsnap
    compile-cache files, 21,832 bytes (E9). An identical second run wrote
    nothing. The current verify read, `brew list --versions`, also wrote 3
    bootsnap files (34,056 bytes) on its first run (E9). Neither read changed
    anything under `api/` (E9). The reads did not refresh the API cache, which
    is consistent with R-15 E4's `skip_download?` rule for a present target.
    That the reads stayed offline is inferred from the unchanged `api/` files;
    the network was not monitored. Moving the verify read from
    `list --versions` to `info --json=v2 --installed` did not, here, add a kind
    of write that the current read does not already make. It does load every
    installed formula's definition, and its output is 880,989 bytes against a
    one-line answer. Both points belong to the security ruling part B
    requires. This note does not make that ruling.
18. **The MacPorts read wrote nothing.** No entry under `/opt/local/var/macports`
    (`find -maxdepth 4 -newer <marker>`) changed across a `port -q installed`
    run, and `~/.macports` does not exist (observed 2026-10-18 with the E9
    marker method, after the E3/E4 reads; the registry files `registry.db`,
    `-shm` and `-wal` are `root:wheel`, mode `rw-r--r--`, so uid 501 cannot
    write them).

### The answer to the question

19. **MacPorts:** `port -q installed`, bare or named, proves the active image.
    A row whose whole first token equals the port's registry name and whose
    last token is exactly `(active)` is the provider's record that the image is
    activated (findings 1, 6, 7). The absence of such a row means "not active
    or not installed". It never means "inactive" in a positive sense.
20. **Homebrew formulae:** `brew info --json=v2 --installed` proves the linked
    keg. A `formulae[]` entry whose `name` equals the package, with a string
    `linked_keg` equal to one of its `installed[].version`, is Homebrew's own
    record of the linked keg (findings 8, 9). Proving "linked" does not need
    `keg_only` and is not settled by it (finding 10).
21. **Homebrew casks, and keg-only formulae that are not linked:** no
    permitted read reports an active or linked state (findings 11, 13).
    `PROVIDER_REPORTS_ACTIVE` cannot be evaluated positively for them from
    these reads.

## Cannot determine

- **The state right after a fresh install.** No install was performed. The
  Manpage says linking "is done automatically when you install formulae and
  casks" (E11), and part B's prerequisite asks for `linked_keg` "after an
  install of a non-keg-only formula". That remains cited, not observed. It
  would be settled by a capture on a disposable machine or VM, before and after
  `brew install <non-keg-only formula>`, and likewise for a keg-only formula
  and for `port install` and `port activate`.
- **The inactive MacPorts row and a variant row, as rendered.** Only the
  source (E5) and a synthesised fixture (E12) show them. This needs a machine
  with two images of one port, or a port installed with a variant, captured
  with `scripts/capture-fixture.sh`.
- **Whether a MacPorts configuration can turn on `ports_verbose` for a `-q`
  read** and so append text after `(active)` (finding 5). Read the
  `ui_options` initialisation in `macports1.0/macports.tcl` and R-16's
  key list.
- **Whether `linked_keg` can disagree with the files in `<prefix>/bin`.** It
  is a record (`var/homebrew/linked/<name>`), not a check of the symlinks.
  For `gh` and `openssl@3` they agreed (E8). Other cases, such as a symlink
  deleted or overwritten by hand, were not tested, and testing them would need
  writes.
- **Why `openssl@4` and `pydantic` are unlinked, and why `openssl@3` is linked
  although keg-only** (`brew link --force`, a keg-only declaration added after
  linking, or a link conflict). The mtimes in E8 do not settle it, and nothing
  here depends on it.
- **The network.** No traffic was captured. "No download" is inferred from
  unchanged `api/` files (finding 17).
- **What wrote the API cache at 21:53 today**, before this note's first
  command (E1). It was not this note.
- **Third-party taps** (`name` vs `full_name` vs `possible_names`) and
  **casks' on-disk link state**. Neither is present or examined here.
- **Other machines.** arm64, other macOS versions, and Homebrew and MacPorts
  versions other than 7.0.4 and 2.12.6 were not observed.

## Impact on the specification

This note edits nothing else; its entry in `docs/spec/09-research-checklist.md`
was added alongside it.

- **`docs/passes/08-pre-flip-design-notes.md`, part B, prerequisite sentence:**
  "Homebrew's `linked_keg` ... `null` for keg-only and unlinked, absent for
  casks". The observations refine it. `null` is right for an unlinked formula,
  whether keg-only or not, but **a keg-only formula can be linked**
  (`openssl@3`, finding 10). For casks, no linked-like field exists at all
  (finding 13).
- **Part B, the `JsonReportsPackageLinked` predicate against its own prose.**
  This is a contradiction inside the design note, not inside `docs/spec/`. The
  predicate ("a string `linked_keg` equal to one of its `installed[].version`")
  is supported by findings 8, 9 and 20. The sentence "**A keg-only formula or a
  cask stays unverifiable and halts**" does not follow from it. The predicate
  would certify `openssl@3` (keg-only, linked). The main agent should decide
  between two options. (a) Keep the predicate and reword the prose to
  "unlinked (keg-only or not) or cask". (b) Make the predicate also require
  `keg_only == false`, which is stricter and would halt on a linked keg-only
  formula. Either choice belongs in the slice's SE entry. Test names that say
  "kegonly ... is not active" should say which one was chosen.
- **Part B, `StdoutMarksPackageActive`** ("first token is the name and last
  token is exactly `(active)`") is supported for the project's argv (`-q`, no
  `-v`) by findings 1, 6 and 7. Two qualifications. The comparison is against
  the registry's spelling, which a case-insensitive named read prints
  (finding 6). The rule silently fails closed if verbose output is ever on
  (finding 5), which is safe.
- **SE-094's `PROVIDER_REPORTS_ACTIVE` paragraph** (`docs/spec-evolution.md`)
  carries two "believed" clauses. "MacPorts' existing verify read, whose output
  is believed to mark the active version" is now **observed** (findings 1, 19).
  "Homebrew's, which is believed to show no linkage" is now **observed**
  (finding 14). If the main agent wants the evolution record to cite this
  note, an SE amendment is how to do it.
- **`docs/spec/04-migration-and-safety.md` destination check 1** ("reports the
  package as installed and active") has no Homebrew meaning for "active" in
  the specification. This note supplies one for formulae, "linked"
  (`linked_keg`), and finds none for casks. Writing that definition into 04 is
  a Tier 3 change for the main agent.
- **Part B's replay assertions over `fixtures/recordings/*.json`** would test
  the inactive-row case only against synthesised data (E12, finding 3). That
  is allowed by the fixtures README, but it should be recorded as
  documentation-derived, not observed.
- **SE-016 item 3, SE-050 item 6, R-15:** finding 17 adds that on a warm API
  cache both the current verify read and the proposed one write only bootsnap
  compile-cache files. It does not answer the tap-Ruby or `brew.env` concern.
  `security-reviewer` still has to rule.
- **`crates/adapters/src/homebrew/json.rs` `active_keg`:** findings 11 and 12
  give two concrete cases on this machine where it reports an "active" keg
  that Homebrew does not record as linked. No code is changed by this note.

## Re-check when

- MacPorts moves past 2.12.6. Re-read `action_installed` in `port.tcl` (the
  `activestr`, `extra` and `puts` lines) and `portimage.tcl`'s state
  transitions. A future `(inactive)` marker or a reordering of `extra` would
  change findings 2, 5 and 7.
- Homebrew moves past 7.0.4. Re-read `Formula#linked_keg`, `#linked_version`
  and `to_hash`'s `linked_keg` and `keg_only` lines, and the cask `to_h`. The
  schema is documented as subject to additive change "without incrementing the
  schema" (E11), so a new cask field meaning "linked" would change
  finding 13.
- `HOMEBREW_LINKED_KEGS` moves away from `var/homebrew/linked`.
- A machine with an inactive MacPorts image, a variant install or a third-party
  tap becomes available. Capture it and close the matching "Cannot determine"
  item.
