# R-3 — MacPorts read interfaces: what can be read, from where, and how stably

- **Status:** partially answered — every documented interface is cited; every
  claim about *exact output bytes*, *exit codes in context*, and *network
  behaviour* is marked for observation on a machine that has MacPorts
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (build 25G83), x86_64, Homebrew 6.0.21 @
  `/usr/local`, **MacPorts not installed** (see Evidence E-0). Nothing in this
  note is an observation of MacPorts; everything is category (b) citation or
  category (c) runtime detection.
- **Blocking:** `02-domain-model.md` (the read contract: `inventory()`,
  `requested()`, `dependencies()`, `reverseDependencies()`, `describe()`,
  `resolution()`, `environmentInfo()`; Ownership "use the provider's own record
  of user intent"); `04-migration-and-safety.md` (§ Process execution, quiet and
  side-effect-free provider invocations); `06-cli-and-ux.md` (`--json` must be
  built by MacPortsBrew, not passed through); the read-only allowlist in
  `.claude/settings.json`.

> **Machine note, added 2026-09-15.** This note states, correctly for its answer
> date, that MacPorts could not be observed here. **It can be since 2026-09-06**,
> when MacPorts 2.12.6 was installed at `/opt/local` (first ports 2026-09-07).
> Nothing below has been re-checked against it; the citations stand as citations.
> R-5 and R-5a were re-checked in place and show what that costs — R-5 gained
> fourteen evidence blocks and produced SE-077. Where this note names a "future
> attempt" that needed a MacPorts machine, that attempt is now available (F-46).

## Question

For MacPorts, what is the read-only query surface the adapter may rely on:
installed ports; the authoritative record of **user intent**; dependencies;
dependents; variants; active vs inactive; port metadata; deprecated/replaced
ports; prefix and configuration. Which of those read the local **PortIndex**,
which read the **registry**, and which need the **network** — because
`analyze` must work offline. Is there a machine-readable (JSON) mode? What is
the parsing failure mode, and what happens on an unknown port name?

## Evidence

### E-0 — Local machine state (observed, 2026-09-02)

```
$ sw_vers
ProductName:		macOS
ProductVersion:		26.6.2
BuildVersion:		25G83
$ uname -m
x86_64
$ command -v port || echo "port: not found on PATH"
port: not found on PATH
$ ls -d /opt/local
ls: /opt/local: No such file or directory
```

Environment in force for this session (relevant to E-14):

```
$ env | grep -iE 'HOMEBREW|MACPORTS'
HOMEBREW_NO_ANALYTICS=1
HOMEBREW_NO_AUTO_UPDATE=1
HOMEBREW_NO_ENV_HINTS=1
MACPORTSBREW_AGENT_SESSION=1
```

No `MACPORTS_*` variable is set, and none was needed: no `port` command was
run, because there is none to run.

**Method note, and a caveat on the quotations below.** `curl` is denied by this
repository's permission configuration, so the pages were retrieved with the
session's documentation-fetch tool, which renders a page to markdown and
extracts the requested passages. Passages below are reproduced as that tool
returned them. Two independent extraction passes over
`https://man.macports.org/port.1.html` disagreed about whether an
`EXIT STATUS` heading exists (E-13); where such a disagreement arose it is
recorded rather than resolved by preference. Anything whose exact bytes matter
for a parser is marked **needs observation** in "Cannot determine". One page
(`https://guide.macports.org/chunked/using.html`) was retrieved in full before
the deny rule bit, and E-1..E-6 are quoted from that local copy directly.

### E-1 — `port installed`: format, header, `-q`, `-v`

Source: MacPorts Guide, "Using MacPorts", § 3.1.17 `port installed`,
<https://guide.macports.org/chunked/using.html#using.port.installed>, read
2026-09-02.

> The `installed` action displays the installed versions and variants of the
> specified ports, or if no ports are specified, all installed ports. It also
> displays whether a port is "active", i.e., whether the files belonging to
> this port are currently present on disk or inactive, i.e., stashed away in a
> compressed tarball.

```
$ port installed

The following ports are currently installed:
  a52dec @0.7.4_0 (active)
  adns @1.4_0 (active)
  apache2 @2.2.27_0+preforkmpm (active)
  …
  youtube-dl @2014.07.25.1_0+python27 (active)
  zlib @1.2.8_0 (active)
```

> Use `-v` to also display the platform and CPU architecture(s) for which the
> ports were built, and any variants which were explicitly negated.

```
$ port -v installed libsdl

The following ports are currently installed:
  libsdl @1.2.15_3-x11 (active) platform='darwin 13' archs='x86_64'
```

Source: `macports-base`, `doc/port-installed.1.txt`,
<https://raw.githubusercontent.com/macports/macports-base/master/doc/port-installed.1.txt>,
read 2026-09-02:

> `port installed` can be used in two separate ways: Either without arguments,
> in which case it will print a list of all installed ports (both active and
> inactive), or with arguments, which will return a list of installed ports
> that match the argument expression.
>
> The output of `port installed` contains the name, version and variants of the
> installed ports and whether the port is active or inactive. See the PORT
> IMAGES section for an explanation of the active/inactive state.
>
> `-v`::
>     Print the platform at install time (i.e., your macOS version) and the
>     architecture(s) of the installed port.
>
> `-q`::
>     Do not print the header line. This is useful when parsing the output of
>     `port installed` in scripts for further processing.

SYNOPSIS from the same file:

```
*port* [*-vq*] *installed*
     [['portname' | 'pseudo-portname' | 'port-expressions' | 'port-url']]
```

### E-2 — Active vs inactive is an image, not a deletion

Source: `macports-base`, `doc/archives.txt` (included into
`port-installed(1)` as the PORT IMAGES section),
<https://raw.githubusercontent.com/macports/macports-base/master/doc/archives.txt>,
read 2026-09-02:

> PORT IMAGES
> -----------
>
> MacPorts has the ability to install multiple versions or different variant
> selections of the same port. The files will be stored in an intermediate
> location as 'archives', which allows to switch between the versions. Only one
> version can be active at a time. On activation, files will be copied to their
> intended place in 'prefix'.

Source: MacPorts Guide § 3.1.19 `port upgrade` (same page as E-1), read
2026-09-02:

> `upgrade` does not uninstall the old version of a port. Instead, it
> deactivates it, i.e., it stashes the files belonging to the older version
> away in a tarball.

### E-3 — `requested` is a recorded flag, set at install time

Source: MacPorts Guide § 3.1.12 `port install`, `--unrequested` switch (same
page as E-1), read 2026-09-02:

> `--unrequested`
> By default, each port you install using the install explicitly (contrary to
> ports installed as a dependency of a different port) is marked as
> "requested". If you want MacPorts to treat a port you installed manually as
> if it was automatically installed as a dependency (e.g., if a dependency
> failed to build and you re-tried installing the dependency only), pass this
> flag.

Source: MacPorts Guide § 3.1.15 `port uninstall` (same page), read 2026-09-02:

> Ports that were manually installed (i.e., are marked as "requested") or have
> other dependents will not be removed. You can manually uninstall the unneeded
> ports later using the `leaves` pseudo-port […]

Source: `port(1)`, <https://man.macports.org/port.1.html>, read 2026-09-02 —
the flag is also user-mutable after the fact:

> **setrequested** — "Mark portname as requested."
> **unsetrequested** — "Mark portname as unrequested."

(Both are mutating actions. They are named here only so the adapter knows the
flag can change without a reinstall; MacPortsBrew never runs them.)

### E-4 — Pseudo-portnames (the full documented list)

Source: `port(1)`, <https://man.macports.org/port.1.html>, read 2026-09-02;
cross-checked against the asciidoc source
<https://raw.githubusercontent.com/macports/macports-base/master/doc/port.1.txt>,
read 2026-09-02. Both passes returned the same twelve entries:

> The *pseudo-portnames* are:
>
> - *all*: all the ports in each ports tree listed in *sources.conf*
> - *current*: the port in the current working directory
> - *active*: set of installed and active ports
> - *inactive*: set of installed but inactive ports
> - *installed*: set of all installed ports
> - *uninstalled*: ports in the ports tree(s) that are not installed
> - *outdated*: installed ports that are out of date with respect to their current version/revision in the ports tree(s)
> - *obsolete*: set of ports that are installed but no longer exist in any port tree
> - *requested*: installed ports that were explicitly asked for
> - *unrequested*: installed ports that were installed only to satisfy dependencies
> - *leaves*: installed ports that are unrequested and have no dependents
> - *rleaves*: installed ports that are unrequested and that no requested ports depend on

Note: `actinact` was **not** returned by either pass and is not treated here as
documented.

Pseudo-portname **selectors** (same sources, read 2026-09-02):

> - *name*, *version*, *revision*, *epoch*, *variant* or *variants*,
>   *category* or *categories*, *maintainer* or *maintainers*,
>   *platform* or *platforms*, *description*, *long_description*, *homepage*,
>   *license*, *portdir*

and, from the asciidoc source, the relationship selectors:

> Other pseudo-portname selectors match ports which have a particular
> relationship to another port. These will match ports that are direct or
> recursive dependencies or dependents of the given portname:
>
> - *depof*, *rdepof*, *rdepends*, *dependentof*, *rdependentof*

### E-5 — `port echo`: expansion, and its behaviour on an unknown name

Source: `macports-base`, `doc/port-echo.1.txt`,
<https://raw.githubusercontent.com/macports/macports-base/master/doc/port-echo.1.txt>,
read 2026-09-02:

> `port echo` expands its argument list according to MacPorts' rules and prints
> a list of ports that match the expression given as argument. It can be useful
> to see what a pseudo-portname or a pseudo-portname selector (see man:port[1])
> expands to.
>
> If `port echo` doesn't recognize an argument as port, it will still print it
> as-is. You cannot use `port echo` to check whether a port exists:
>
> ----
> $ port echo nonexistentport
> nonexistentport
> ----
>
> EXAMPLES
> --------
> Common use cases are:
>
> ----
> port echo depends:zlib
> port echo inactive
> port echo obsolete
> port echo requested
> port echo leaves
> port echo category:^mail$
> ----

`port(1)` (<https://man.macports.org/port.1.html>, read 2026-09-02) adds:

> **echo** — Writes to stdout the arguments passed to *port*. This follows the
> expansion of *pseudo-portnames*, portname glob patterns, *pseudo-portname
> selectors* and the evaluation of *port-expressions*. **echo** may be used to
> determine the exact set of ports to which a given string of arguments will
> expand, without performing any further operations on them.

### E-6 — `deps` / `rdeps` and `--index`

Source: MacPorts Guide § 3.1.9 and § 3.1.10 (same page as E-1), read
2026-09-02:

```
$ port deps apache2
Full Name: apache2 @2.2.27_0+preforkmpm
Library Dependencies: apr, apr-util, expat, openssl, pcre, perl5, zlib
```

> Note that the list of dependencies might depend on the variants you chose.
> For example, choosing the `+openldap` variant of `apache2` adds a dependency
> on `openldap`:
>
> ```
> $ port deps apache2 +openldap
> Full Name: apache2 @2.2.27_0+openldap+preforkmpm
> Library Dependencies: apr, apr-util, expat, openssl, pcre, perl5, zlib, openldap
> ```
>
> `deps` accepts three switches:
>
> `--index`
> Do not read the Portfile to determine dependencies. Instead, rely on the
> information cached in the port index. Note that (despite specifying them),
> this option will ignore any effects of variants. It is, however, much faster.
>
> `--no-build`
> Exclude dependencies only required at build time, i.e., fetch, extract,
> patch, and build dependencies.
>
> `--no-test`
> Exclude test dependencies.

```
$ port rdeps gettext
The following ports are dependencies of gettext @0.22.5_0:
  ncurses
  gettext-tools-libs
    libiconv
      gperf
    libtextstyle
    gettext-runtime
```

> `rdeps` accepts four switches: `--full` […] `--index` […] `--no-build` […]
> `--no-test`

`port(1)` (read 2026-09-02) states the implementation relationship:

> **deps** — Lists the other ports that are required to build and run portname.
> This is simply an alias for "info --pretty --fullname --depends".

### E-7 — `dependents` / `rdependents` read the installed-ports database

Source: `macports-base`, `doc/port-dependents.1.txt`,
<https://raw.githubusercontent.com/macports/macports-base/master/doc/port-dependents.1.txt>,
read 2026-09-02:

> `port dependents` reports which ports depend upon a given (installed) port, if
> any. Note that *dependents* does not work for ports that are not installed on
> your system. **It uses the database of installed ports that MacPorts keeps
> internally, which records the dependency relations at installation time.**
> `port dependents` prints this information.
>
> For this reason, `port dependents` does not work for ports that are not
> installed on your system. If you want to find out which ports depend on a port
> you haven't installed, you can use the *depends:* pseudo-portname selector:
>
> ----
> port echo depends:<portname>
> ----
>
> This command will, however, not cover dependencies that are only present in
> non-default variants.
>
> `port rdependents` will print a tree of installed ports that depend on the
> given port.

Guide § 3.1.20 (same page as E-1) shows the line format:

```
$ port dependents openssl
apache2 depends on openssl
curl depends on openssl
…
```

### E-8 — `port info`: the machine-readable path, and every field flag

Source: `macports-base`, `doc/port-info.1.txt`,
<https://raw.githubusercontent.com/macports/macports-base/master/doc/port-info.1.txt>,
read 2026-09-02. Reproduced in full for the parts that matter:

```
SYNOPSIS
--------
*port* [*-q*] [*-D* 'portdir'] *info*
     [--categories|--category] [--conflicts] [--depends] [--depends_fetch]
     [--depends_extract] [--depends_build] [--depends_lib] [--depends_run]
     [--description] [--epoch] [--fullname] [--heading] [--homepage] [--index]
     [--license] [--line] [--long_description] [--maintainer|--maintainers]
     [--name] [--patchfiles] [--platform|--platforms] [--portdir] [--pretty]
     [--replaced_by] [--revision] [--subports] [--variant|--variants]
     [--version]
     [['portname' | 'pseudo-portname' | 'port-expressions' | 'port-url']]
```

> `port info` prints information about the given ports. Specifying at least one
> of the options limits the output to the corresponding field. If no fields are
> specified, a useful default set consisting of name, epoch, version, revision,
> categories, replaced_by, variants, description, homepage, dependencies,
> platforms, license, and maintainers is shown.
>
> The following options do not select fields for the output but change how the
> information is obtained or formatted:
>
> `--index`::
>     Do not read the 'Portfile', but rely solely on the port index
>     information. Note that this option will prevent the information reported
>     from reflecting the effects of any variants specified.
>
> `--line`::
>     Print a single line for each port. Fields are separated by spaces.
>
> `--pretty`::
>     Format the output in a convenient, human-readable fashion. This is the
>     default when no options are specified.
>
> […]
> `--depends`, `--depends_fetch`, `--depends_extract`, `--depends_build`,
> `--depends_lib`, `--depends_run`::
>     List the specified dependencies of a port. `--depends` is a shorthand
>     option for all other `--depends_*` options.
>
> `--fullname`::
>     Print name and the full MacPorts version tuple, e.g., ``yubico-pam @2.16''.
>
> `--platform`, `--platforms`::
>     List the platforms supported by a port. This field exists for historical
>     reasons only. In modern MacPorts, this is always 'darwin', i.e., macOS.
>
> `--portdir`::
>     Print the path to a port's directory relative to the port tree root.
>
> `--replaced_by`::
>     List the name of the port that replaces a port, if any.
>
> `--subports`::
>     Print a list of subports defined by this port's 'Portfile', i.e., ports
>     that are defined in the same 'Portfile' because they share common parts.
>
> `--variant`, `--variants`::
>     List the variants defined by a port by name.
>
> `-q`::
>     Do not print the field description when using `--pretty` output.

`port(1)` (read 2026-09-02) adds:

> **info** — Displays meta-information available for *portname*. […] Recognized
> field names are those from the PortIndex, see "port help info" for a complete
> list. If no specific fields are specified, a useful default collection of
> fields will be displayed. If the global option **-q** is in effect, the
> meta-info fields will not be labeled. If the option **--line** is provided,
> all such data will be consolidated into a single line per port, suitable for
> processing in a pipe of commands. […] If the option **--index** is provided,
> the information will be pulled from the PortIndex rather than from the
> Portfile. In this case variant information, such as dependencies, will not
> affect the output.

Guide § 3.1.8 shows the default (`--pretty`) rendering:

```
$ port info yubico-pam

yubico-pam @2.16 (security)
Variants:             universal

Description:          The Yubico PAM module provides an easy way to integrate the YubiKey into your
                      …
Homepage:             https://github.com/Yubico/yubico-pam

Build Dependencies:   pkgconfig, autoconf, automake, libtool
Library Dependencies: ykpers, yubico-c-client
Platforms:            darwin
License:              BSD
Maintainers:          cal@macports.org
```

**Separator hazard, cited not inferred.** `--line` says only "Fields are
separated by spaces." The Guide's `port search --line` example (§ 3.1.7, same
page as E-1) shows a field that itself contains a space:

```
$ port search --name --line --regex '^php\d*$'
php     5.5       lang www    PHP: Hypertext Preprocessor
```

Here `lang www` is a two-token categories field. That is `search`, not `info`,
but it is the only `--line` output the official documentation shows, and it is
enough to establish that "separated by spaces" does not imply
"unambiguously splittable on spaces".

### E-9 — `port variants`, and the meaning of the `+`/`-`/`[]`/`()` markers

Source: MacPorts Guide § 3.1.11 (same page as E-1), read 2026-09-02:

```
$ port variants apache2 +universal

apache2 has the variants:
   eventmpm: Use event MPM (experimental)
     * conflicts with preforkmpm workermpm
   openldap: Enable LDAP support through OpenLDAP
[+]preforkmpm: Use prefork MPM
     * conflicts with eventmpm workermpm
  +universal: Build for multiple architectures
   workermpm: Use worker MPM
     * conflicts with eventmpm preforkmpm
```

> This output lists all variants followed by their description. If a variant
> depends on or conflicts with other variants, a line detailing that follows. A
> variant name prefixed by `+` indicates that it has been enabled (on the
> command line), while a prefix `-` indicates that it has been disabled. When
> bracketed, a prefix `+` means that the variant is enabled by default. Any
> `[]` are derived from the Portfile. While `()` are derived from the
> variants.conf.

`port(1)`: "**variants** — Lists the variants available for *portname*."

### E-10 — Deprecation / replacement: `replaced_by`, `obsolete`

Source: `doc/port-info.1.txt` (E-8): `--replaced_by` — "List the name of the
port that replaces a port, if any." It is also in the default field set.

Source: MacPorts Guide § 3.1.19 `upgrade --no-replace` (same page as E-1),
read 2026-09-02:

> `--no-replace`
> Do not automatically install replacement ports for a port that you have
> installed, but was replaced with a different one.

Source: `port(1)` pseudo-portnames (E-4): *obsolete* — "set of ports that are
installed but no longer exist in any port tree". This is a **different** state
from `replaced_by`: `obsolete` means "gone from the tree", `replaced_by` means
"the tree names a successor".

### E-11 — Prefix, paths and configuration

Source: `port(1)` FILES section, <https://man.macports.org/port.1.html>, and
`doc/port.1.txt`, both read 2026-09-02 (the two passes agreed verbatim):

> `${prefix}/etc/macports/macports.conf`::
>     Global configuration file for the MacPorts system.
>
> `${prefix}/etc/macports/sources.conf`::
>     Global listing of the ports trees used by MacPorts. This file also enables
>     rsync synchronization.
>
> `${prefix}/etc/macports/variants.conf`::
>     Global variants used when a port is installed.
>
> `~/.macports/macports.conf`::
>     User configuration file for the MacPorts system. It overrides the global
>     'macports.conf(5)' file.

Source: MacPorts Guide, "MacPorts Configuration Files",
<https://guide.macports.org/chunked/internals.configuration-files.html>, read
2026-09-02:

> **prefix**: "Sets the directory where ports are installed. Any path may be
> used but those with spaces and/or non-ASCII characters should be avoided
> because it can break some ports. Default: `/opt/local`"
>
> **applications_dir**: "Directory in which ports will install native macOS
> application bundles. Default: `/Applications/MacPorts`"
>
> **portdbpath**: "Directory where MacPorts keeps working data such as
> downloaded sources, installed port receipts, and the main registry.
> Default: `${prefix}/var/macports`"
>
> **buildfromsource**: "Controls whether ports are built from source or
> downloaded as pre-built archives. Setting to 'always' will never use
> archives, 'never' will always try to use an archive and fail if one is not
> available. 'ifneeded' will try to fetch an archive and fall back to building
> from source if that isn't possible. Default: `ifneeded`"
>
> variants.conf: "This optional file specifies any variants you'd like to be
> invoked globally. If a variant specified in this file is not supported by a
> given Portfile, the variant is simply ignored. Default: none"
>
> sources.conf: "This file enables rsync synchronization of the default ports
> tree with the MacPorts rsync server when either of the commands **port
> selfupdate** or **port sync** are run. Default:
> `rsync://rsync.macports.org/macports/release/tarballs/ports.tar [default]`"

Path-printing actions — `macports-base`, `doc/port-dir.1.txt`,
<https://raw.githubusercontent.com/macports/macports-base/master/doc/port-dir.1.txt>,
read 2026-09-02:

> `port dir` prints the directory that contains the 'Portfile' for the given
> port expression.
>
> `port file` prints the path of the 'Portfile' for the given port expression.
>
> `port location` only works for installed ports and prints the location of the
> archive MacPorts internally uses to be able to deactivate and activate a
> port.
>
> `port work` prints the path to the so-called 'work' directory, i.e., the
> directory used by MacPorts to build a port. No output is generated if no
> build directory exists, e.g., because there is no unfinished build in
> progress for the given port.

`port prefix` — `macports-base`, `doc/port-prefix.1.txt`,
<https://raw.githubusercontent.com/macports/macports-base/master/doc/port-prefix.1.txt>,
read 2026-09-02:

> NAME
> ----
> port-prefix - Print the prefix for which MacPorts was configured
>
> DESCRIPTION
> -----------
> This command prints the installation prefix of your MacPorts installation,
> i.e., the directory under which MacPorts and all ports are installed. The
> prefix is set at configure time and defaults to '/opt/local'.
>
> AUTHORS
> -------
>  (C) 2026 The MacPorts Project

**But**: a targeted pass over the *published* `port(1)`
(<https://man.macports.org/port.1.html>, read 2026-09-02) returned
"ABSENT: prefix" for the `prefix` action, while returning text for `version`,
`help`, `setrequested`, `unsetrequested`, `reclaim`, `rdependents` and
`location`. `port prefix` is therefore present in `master` (copyright 2026) and
not in the currently published manual.

Version detection — `port(1)`, read 2026-09-02:

> **version** — "Display the release number of the installed MacPorts
> infrastructure."

Guide § 3.1.2 shows a base version in the current documentation examples:
"MacPorts base version 2.12.6 installed".

### E-12 — Registry and PortIndex locations

Source: MacPorts Guide, "The MacPorts Registry",
<https://guide.macports.org/chunked/internals.registry.html>, read 2026-09-02:

> "The SQLite registry used by default is located at `${portdbpath}/registry`,
> which by default would be `${prefix}/var/macports/registry`."
>
> "All data is stored in a single file named `registry.db`, although the
> additional directory `portfiles` is used temporarily for extracting stored
> Portfiles from the registry."
>
> "The legacy flat file registry files are contained in `${portdbpath}/receipts`,
> which by default is location `${prefix}/var/macports/receipts`."
>
> "If found, these will be automatically converted to the new SQLite registry."

So the default registry path is `/opt/local/var/macports/registry/registry.db`,
by composition of the cited `prefix` default (E-11) and the cited
`${portdbpath}/registry/registry.db` (this evidence).

PortIndex — Source: MacPorts Guide, "Local Portfile Repositories",
<https://guide.macports.org/chunked/development.local-repositories.html>, read
2026-09-02: a ports tree is a directory named in `sources.conf`, and

> "use **portindex** in the local repository's directory to create or update the
> index of the ports in your local repository"

The index file is named `PortIndex` and lives in the ports-tree directory. The
*absolute* path of the default tree's `PortIndex` is **not** stated verbatim by
any page read here; it is composed from `portdbpath` + the `sources.conf` entry
and is therefore a runtime-detection item, not a citation.

### E-13 — Exit status, and unknown port names

Source: `doc/port.1.txt`,
<https://raw.githubusercontent.com/macports/macports-base/master/doc/port.1.txt>,
read 2026-09-02, and one of two passes over
<https://man.macports.org/port.1.html>, read 2026-09-02 — both returned:

> "The *port* utility exits 0 on success, and >0 if an error occurs."

A second pass over the rendered man page reported "The page contains no
'EXIT STATUS' section." The two extraction passes disagree about the *heading*;
they do not disagree about the *sentence*, which only one pass was asked to
find. Treated here as: the sentence is documented in the asciidoc source of the
manual; the presence of a distinctly titled section in the rendered page is
**needs observation**.

Unknown port name — Source: `macports-base`, `src/port/port.tcl`,
<https://raw.githubusercontent.com/macports/macports-base/master/src/port/port.tcl>,
read 2026-09-02, in `proc action_info`:

```tcl
        if {$porturl eq "" || $index_only} {
        # Verify the portname, getting portinfo to map to a porturl
            if {[catch {mportlookup $portname} result]} {
                ui_debug "$::errorInfo"
                break_softcontinue "lookup of portname $portname failed: $result" 1 status
            }
            if {[llength $result] < 2} {
                break_softcontinue "Port $portname not found" 1 status
            }
```

and in `proc action_deps`:

```tcl
            if {[llength $result] < 2} {
                break_softcontinue "Portdir $portdir not found" 1 status
            }
```

No page or source line read here describes `port info` fetching, syncing or
otherwise touching the network when a name is unknown.

### E-14 — Keeping `port` quiet; network-touching actions

Source: `port(1)`, <https://man.macports.org/port.1.html>, read 2026-09-02:

> **-q** — Quiet mode, suppress informational messages to a minimum, implies -N
>
> **-N** — Non-interactive mode, interactive questions are not asked
>
> **-v** — Verbose mode, generates verbose messages
>
> **-d** — Debug mode, generate debugging messages, implies -v
>
> **-p** — Despite any errors encountered, proceed to process multiple ports and commands.
>
> **-D** *portdir*|*portname* — Specify a directory to which the port command
> should change before processing any actions. […]
>
> **-F** *cmdfile* — Read and process the *file* of commands specified by the
> argument. If the argument is *-*, then read commands from stdin. […]

The only sentence on that page containing "interactive" is the `-N` one; a
targeted pass found **no** sentence containing "reminder".

Documented network actions — `port(1)`, read 2026-09-02:

> **selfupdate** — Updates the MacPorts system, ports tree(s) and base tools if
> needed, from the MacPorts rsync server, installing the newest infrastructure
> available.
>
> **sync** — Performs a sync operation only on the ports tree of a MacPorts
> installation, pulling in the latest revision available of the Portfiles from
> the MacPorts rsync server.

Guide § 3.1.2 (same page as E-1): "The `selfupdate` action should be used
regularly to update the **local ports tree** with the global MacPorts ports
repository". Guide § 3.1.18: "Note that you will only get new versions by
updating your ports tree using `selfupdate` (or `sync`)." Guide § 3.1.21:
`livecheck` "checks to see if the application corresponding to a given port has
been updated at the developer's download site".

A reminder mechanism does exist — Guide § 3.1.5, read 2026-09-02:

> `reclaim` accepts switches to configure automatic reminders. If passed, the
> reclaim process will not be run.
> `--enable-reminders` — Enable regular reminders to run `port reclaim`.
> `--disable-reminders` — Disable regular reminders to run `port reclaim`.

What triggers a reminder, on which actions, and what it prints, is not stated
on that page.

### E-15 — Machine-readable output: is there JSON?

Source: `port(1)`, <https://man.macports.org/port.1.html>, read 2026-09-02.
Asked to quote every occurrence of "json"/"JSON" on the page, the extraction
returned exactly:

> NO OCCURRENCE OF JSON.

No `--json` appears in the `port-info(1)`, `port-installed(1)`,
`port-echo(1)`, `port-dependents(1)` or `port-dir(1)` synopses reproduced above
(E-1, E-5, E-7, E-8, E-11).

There **is** JSON on the MacPorts *website*, which is a different thing.
`https://ports.macports.org/api/` returns HTTP 404; `https://ports.macports.org/api/v1/`
(read 2026-09-02) returns a Django REST Framework router root with
`Content-Type: application/json`, `Allow: GET, HEAD, OPTIONS`, listing the
endpoints: `search`, `autocomplete/port`, `ports`, `category`,
`autocomplete/category`, `builders`, `builds`, `files`,
`autocomplete/maintainer`, `maintainer`, `autocomplete/variant`.

### E-16 — Binary archives (touches R-4; recorded here because it is a read concern)

Source: MacPorts Guide, "Port Binaries",
<https://guide.macports.org/chunked/using.binaries.html>, read 2026-09-02:

> "Binary archives can only be used on a target system running MacPorts. They
> allow MacPorts utilities to skip the build (which is usually the phase that
> takes longest) and begin installation after the destroot phase."
>
> "MacPorts runs a buildbot infrastructure that creates prebuilt binary packages
> for all ports in MacPorts for the default installation prefix."
>
> "If a port builds successfully and its license and those of its dependencies
> allow binary redistribution, the archives are uploaded to
> `packages.macports.org` and will be automatically used by MacPorts during
> installation."

The page does not state how non-default variants affect archive availability.
The `buildfromsource` key (E-11) is the configuration lever
(`always` / `never` / `ifneeded`, default `ifneeded`).

## Findings

1. **`port` has no JSON or other structured output mode.** (E-15.) Every read
   is line-oriented text that the adapter must parse. `06-cli-and-ux.md`'s
   `--json` is MacPortsBrew's own surface and, for MacPorts, must be
   constructed from parsed text — there is nothing to pass through. This is
   asymmetric with Homebrew (`brew info --json`) and the adapter design must
   not assume symmetry of *mechanism*, only of *contract*.

2. **The machine-readable path is `port info` with explicit field flags, one
   field per invocation.** (E-8.) `--line` collapses a port to one line but its
   only documented separator is "spaces", and the one official `--line` example
   shows a field containing a space. Therefore: request exactly one field per
   invocation (`port -q info --index --license <name>`), or accept that
   multi-field `--line` parsing is heuristic. `-q` additionally suppresses
   field labels. The full field vocabulary is the PortIndex's, enumerated in
   E-8.

3. **`requested` is the provider's own record of user intent, and it is a
   stored flag, not an inference.** (E-3, E-4.) MacPorts marks a port
   "requested" when the user names it in an `install`, and `--unrequested`
   exists precisely to suppress that marking. `02-domain-model.md`'s "use the
   provider's own record of user intent where it exists" is satisfiable
   directly:
   - `TOP_LEVEL` ← pseudo-portname `requested`
   - `DEPENDENCY` ← pseudo-portname `unrequested`
   - `ORPHAN_DEPENDENCY` ← pseudo-portname `leaves` (unrequested **and** no
     dependents) or `rleaves` (unrequested and no *requested* port depends on
     it, transitively)

   The mechanism to name in the confidence record is "MacPorts registry
   `requested` flag, read via the `requested`/`unrequested`/`leaves`
   pseudo-portnames". It is **not** a dependency-graph inference, and it must
   not be replaced by one. Caveat: the flag is user-mutable via
   `setrequested`/`unsetrequested` (E-3), so it records intent *as last
   asserted*, not *as originally typed*.

4. **`leaves` vs `rleaves` are two different orphan definitions and the
   difference is load-bearing.** (E-4.) `leaves` = unrequested with no
   dependents at all. `rleaves` = unrequested and not reachable from any
   requested port. A port can have a dependent that is itself an orphan; it is
   in `rleaves` but not `leaves`. Pick one deliberately and say which in the
   report. Recommendation: `rleaves` for "what could be removed", `leaves` for
   "what is removable right now without cascading".

5. **Active vs inactive is a first-class installed state, not a Homebrew
   concept with a rename.** (E-1, E-2.) Multiple versions/variant-sets of one
   port can be installed simultaneously; exactly one is active; inactive ones
   are archives on disk. `inventory()` for MacPorts must therefore return a
   *set* of installed instances per port name with an activation flag, not one
   row per name. `port upgrade` leaves the old version installed-but-inactive
   by default. Any disk-usage or "is X installed" logic that assumes one row
   per name will be wrong on a normal MacPorts machine.

6. **The installed-line format is documented and stable in shape:**
   `name @version_revision+variants (active)` with a header line that `-q`
   removes, and with `-v` appending `platform='…' archs='…'` and explicitly
   negated variants (`-x11` in the cited example). (E-1.) Parse with a
   tolerant regex anchored on `@` and `(active)`/`(inactive)`; the exact
   whitespace is **needs observation**.

7. **Dependency queries have three distinct sources and the adapter must choose
   consciously.** (E-6, E-7.)
   - `port deps` / `port rdeps` **without** `--index` evaluate the Portfile,
     honour variants, and are slow.
   - `port deps --index` / `port rdeps --index` read the PortIndex, are "much
     faster", and **ignore variant effects**.
   - `port dependents` / `port rdependents` read the registry's recorded
     dependency relations *from installation time* and only work for installed
     ports.

   For a whole-machine `analyze`, `--index` is the right default (bounded cost)
   with the variant caveat surfaced in the report; for a specific migration
   plan, the Portfile-evaluating form with the machine's actual variants is the
   correct one. `port echo depends:<name>` is the only way to ask about a port
   that is *not* installed, and it misses non-default-variant dependencies
   (E-7).

8. **Deprecation has two independent states in MacPorts.** (E-10.)
   `replaced_by` (the tree names a successor; readable per port via
   `port info --replaced_by`, and in the default field set) and `obsolete`
   (installed but no longer in any tree; readable as a set via
   `port echo obsolete`). These are not the same and must not collapse into one
   `deprecated` boolean in the domain model.

9. **Prefix and configuration are runtime detection, with a documented default
   and documented keys.** (E-11.) Default prefix `/opt/local`; config at
   `${prefix}/etc/macports/macports.conf` with a user override at
   `~/.macports/macports.conf`; keys `prefix`, `applications_dir`,
   `portdbpath`, `buildfromsource`; sibling files `sources.conf` (ports trees)
   and `variants.conf` (globally applied variants). **`variants.conf` matters
   for R-4 and for migration fidelity**: it silently changes what "default
   variants" means on this machine, and the `()` markers in `port variants`
   output (E-9) are how it becomes visible.

10. **`port prefix` exists in `master` but is absent from the published
    `port(1)`.** (E-11.) This is exactly the case `02-domain-model.md` means by
    "capabilities are *detected*, not assumed from a version string": the
    adapter should attempt `port prefix`, fall back to deriving the prefix from
    the resolved path of the `port` executable (`…/bin/port` →
    `dirname(dirname(realpath(port)))`), and then confirm against
    `macports.conf`.

11. **Offline vs network.** The only actions documented to reach the network
    are `selfupdate`, `sync` (both mutating; never run) and `livecheck` (E-14).
    Every action in the proposed adapter set below is documented as reading
    local state: the registry, the local ports tree, the PortIndex, or
    `macports.conf`. **No source read here positively asserts that
    `port info` never touches the network** — the claim is "not documented to,
    and the tree is explicitly local". Given R-1's finding that `brew info` on
    an unknown name silently cloned 1.4 GB, this must be *observed*, not
    assumed, before `analyze` is declared offline-safe (see "Cannot determine").

12. **Quieting MacPorts is done with argv flags, not environment variables.**
    (E-14.) `-q` (implies `-N`) plus never invoking `selfupdate`/`sync` is the
    documented equivalent of Homebrew's `HOMEBREW_NO_AUTO_UPDATE`. No
    `MACPORTS_*` environment variable was found in the pages read. A reminder
    mechanism for `port reclaim` exists and is configurable, but what triggers
    it and what it prints is undocumented on the page read — `-q -N` is the
    defensive posture.

13. **Unknown port name: documented failure, no documented side effect.**
    (E-13.) `port info <unknown>` reports `Port <name> not found` and sets a
    failure status; `port` "exits 0 on success, and >0 if an error occurs". By
    contrast `port echo <unknown>` prints the argument back unchanged and is
    explicitly *not* an existence test. So: **use `info` to test existence,
    never `echo`.**

14. **`ports.macports.org` has a JSON HTTP API** (E-15) with `ports`, `search`,
    `files`, `builds`, `maintainer`, `variant` and `category` endpoints. It is
    a *network* source and therefore cannot be part of `analyze`. It is a
    candidate for the mapping/enrichment layer only, behind an explicit
    online-only flag, and its stability is not stated on the root document.

## Cannot determine

Every item here needs a machine with MacPorts installed. The commands are
read-only.

- **UNRESOLVED — exact bytes of `port -q installed`.** Leading whitespace,
  whether `-q` also removes the two-space indent, how an inactive port renders,
  how epoch appears in the version tuple, and how multiple installed versions
  of one name are ordered. Needs: `port -q installed` and
  `port -q -v installed` on a machine with at least one inactive port and one
  port with an epoch.
- **UNRESOLVED — `port info --line` field separation.** Whether multi-valued
  fields are comma-joined and whether adjacent fields are ever ambiguous.
  Needs: `port -q info --index --line --name --version --categories --variants zlib`
  and the same for a port with several categories and maintainers.
- **UNRESOLVED — offline behaviour of every read action.** Needs each proposed
  argv form run with the network disabled (or under a network monitor),
  especially `port info <unknown-name>`, which is the exact shape that cost
  1.4 GB under Homebrew (R-1). Until then, `analyze`'s offline guarantee for
  MacPorts is asserted from documentation, not verified.
- **UNRESOLVED — exit codes per action in context.** Documented as `0` / `>0`
  (E-13); the specific code for "port not found", the effect of `-p`, and the
  code when *some* of several named ports fail are not documented. Needs:
  `port -q info nonexistent-xyz; echo $?` and the multi-port and `-p` variants.
- **UNRESOLVED — whether `port info` writes anything.** MacPorts evaluates
  Portfiles in Tcl; whether a non-`--index` `info` can touch `${portdbpath}`
  (e.g. a cache) is not documented on the pages read. Needs: `port info <name>`
  with `${prefix}/var/macports` under a before/after checksum, plus the same
  with `--index`.
- **UNRESOLVED — the `reclaim` reminder trigger.** Which actions can emit it,
  what it prints, and whether `-q -N` fully suppresses it. Needs: an
  installation that has not run `reclaim` recently.
- **UNRESOLVED — absolute `PortIndex` path for the default tree.** Composed,
  not cited (E-12). Needs: `port dir zlib` and the contents of
  `${prefix}/etc/macports/sources.conf`.
- **UNRESOLVED — `port prefix` availability by version.** Present in `master`
  doc (2026), absent from the published `port(1)` (E-11). Needs: `port version`
  and `port prefix; echo $?` on a released MacPorts.
- **UNRESOLVED — whether `actinact` is a real pseudo-portname.** Neither
  extraction pass returned it (E-4). Needs: `port help` / the released man page
  on a MacPorts machine.
- **PARTIAL — verbatim fidelity.** All quotations were obtained through a
  documentation-fetch tool that renders pages to markdown; two passes over the
  same man page disagreed about one section heading (E-13). A future attempt
  with direct retrieval (or `man port` on a MacPorts machine) should re-confirm
  E-4, E-8 and E-13 character for character before any parser is written
  against them.

## Impact on the specification

Nothing here **contradicts** the specification. Four items refine it, and one
is a wording problem worth an entry.

1. **`docs/spec/02-domain-model.md`, Ownership — CONFIRMED and made concrete.**
   The MUST "use the provider's own record of user intent where it exists" is
   satisfiable for MacPorts by the registry `requested` flag via the
   `requested` / `unrequested` / `leaves` / `rleaves` pseudo-portnames
   (Finding 3). Suggest the spec (or the adapter's doc comment) name this
   mechanism explicitly, alongside the Homebrew one, and note the
   `leaves`/`rleaves` distinction (Finding 4) so `ORPHAN_DEPENDENCY` has one
   defined meaning rather than two.

2. **`docs/spec/02-domain-model.md`, `inventory()` — needs a shape decision.**
   MacPorts installs are (name, version, revision, variant-set, active?) tuples
   with **multiple simultaneous rows per name** (Finding 5). If
   `InstalledPackage` is currently one-per-name, that is a modelling gap for
   MacPorts, not a formatting detail. This is the one item I would raise for a
   `docs/spec-evolution.md` entry.

3. **`docs/spec/06-cli-and-ux.md`, `--json` — no conflict, one consequence.**
   MacPorts offers no provider-side JSON (Finding 1), so MacPortsBrew's JSON is
   entirely synthesised for that provider. Parser failure is therefore a
   first-class error path for MacPorts in a way it is not for Homebrew, and
   `07-quality-and-testing.md`'s fixture corpus needs MacPorts text fixtures
   covering: inactive ports, epochs, negated variants, multi-version installs,
   and an unknown-port error.

4. **`docs/spec/04-migration-and-safety.md:216` — wording.** "Provider
   invocations set the **environment** that keeps them quiet" is Homebrew-shaped.
   For MacPorts the mechanism is argv (`-q`, which implies `-N`) and the absence
   of `selfupdate`/`sync`, not environment variables (Finding 12). Suggest
   "set the environment **and flags**".

5. **`docs/spec/02-domain-model.md`, `artifactAvailability()` / R-4.** E-16
   confirms the three states the brief insists on distinguishing exist on the
   MacPorts side too: no archive published at all (licence forbids
   redistribution), an archive exists but not for this configuration
   (non-default prefix — "for the default installation prefix"), and a source
   build forced by configuration (`buildfromsource=always`) or by a non-default
   variant. The variant case is the one E-16 does **not** document; R-4 must
   settle it.

## Proposed read-only argv forms for the MacPorts adapter

All forms are read-only. Every one is prefixed `port -q -N` unless the header
line is wanted. **`selfupdate`, `sync`, `livecheck`, `reclaim`, `install`,
`uninstall`, `activate`, `deactivate`, `setrequested`, `unsetrequested`,
`upgrade`, `clean` are never invoked.**

| Contract method | argv | Reads |
|---|---|---|
| `detect()` — version | `port version` | base install |
| `detect()` — prefix | `port prefix` (capability-detected; fall back to `realpath` of the `port` binary, confirm against `macports.conf`) | base install / filesystem |
| `environmentInfo()` | read `${prefix}/etc/macports/macports.conf`, `~/.macports/macports.conf`, `sources.conf`, `variants.conf` (file reads, no subprocess) | config files |
| `capabilities()` | `port help` plus a probe of `port prefix` exit status | base install |
| `inventory()` | `port -q installed` | **registry** |
| `inventory()` + arch/platform | `port -q -v installed` | **registry** |
| `requested()` | `port -q echo requested` | **registry** |
| dependencies-only set | `port -q echo unrequested` | **registry** |
| orphan candidates | `port -q echo leaves` and `port -q echo rleaves` | **registry** |
| active / inactive sets | `port -q echo active` / `port -q echo inactive` (or parse `(active)` from `installed`) | **registry** |
| outdated | `port -q outdated` | **registry + PortIndex** |
| obsolete (gone from tree) | `port -q echo obsolete` | **registry + PortIndex** |
| `dependencies(pkg)` — fast | `port -q deps --index <name>` | **PortIndex** (ignores variants) |
| `dependencies(pkg)` — exact | `port -q deps <name>` (optionally with the machine's variant selection) | **Portfile** in the local ports tree |
| recursive deps | `port -q rdeps --index --no-build <name>` | **PortIndex** |
| `reverseDependencies(pkg)` | `port -q dependents <name>` / `port -q rdependents <name>` | **registry** (relations recorded at install time) |
| reverse deps for a *not-installed* port | `port -q echo depends:<name>` | **PortIndex** (misses non-default-variant deps) |
| `describe(pkg)` — one field | `port -q info --index --<field> <name>` per field | **PortIndex** |
| `describe(pkg)` — variant-aware | `port -q info --<field> <name>` (no `--index`) | **Portfile** |
| available variants | `port -q info --index --variants <name>`; human view `port variants <name>` | **PortIndex** / Portfile |
| installed variants | the `+var`/`-var` suffix of the `port -q installed <name>` line | **registry** |
| replacement port | `port -q info --index --replaced_by <name>` | **PortIndex** |
| `resolution(executable)` | `port -q provides <absolute-path>` | **registry** file map |
| installed files | `port -q contents <name>` | **registry** |
| port directory / archive location | `port dir <name>` / `port -q location <name>` | ports tree / **registry** |
| existence test | `port -q info --index --name <name>`, check exit status — **never `port echo`** | **PortIndex** |

Network: **none of the above is documented to use it**; see Finding 11 and the
matching UNRESOLVED item before relying on that.

## Re-check when

- **Before any MacPorts adapter parser is written.** Every "needs observation"
  item above must become an observation on a machine with MacPorts, including
  at least one inactive port and one multi-version install.
- **On a MacPorts base minor release.** `port prefix` (E-11) is already a
  version-dependent capability; the pseudo-portname list and the `info` field
  vocabulary can grow.
- **If MacPorts ever ships structured output.** Finding 1 is the load-bearing
  assumption behind the whole MacPorts parsing layer; a `--json` on `port info`
  would let most of it be deleted. Re-read `port-info(1)` at each base release.
- The MacPorts Guide is versionless and updated in place. All Guide citations
  here are dated 2026-09-02 and describe a base whose own examples say
  "MacPorts base version 2.12.6".
