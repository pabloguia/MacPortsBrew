# R-5a — Does the `verify` command each provider uses need administrator rights, for a package and for a GUI application?

- **Status:** partially answered — Homebrew observed and cited (both cells);
  MacPorts citation-only and **UNRESOLVED**, recommended `unknown`
- **Answered on:** 2026-09-04
- **Machine:** macOS 26.6.2 (build 25G83), x86_64 (Intel), Homebrew 6.0.21 @
  `/usr/local` (`Homebrew/brew` HEAD `560147012b9678b42ef5e83b690f0895552d1366`,
  committed 2026-09-01), MacPorts **not installed**
- **Blocking:** the four missing cells in `data/policy/provider-commands.toml`
  `[entry.elevation]` — `verify` and `verify_gui_app`, per provider. Today both
  providers' `verify*` keys are absent, and
  `crates/application/src/planner.rs:598-606` maps a missing key to
  `Elevation::Unknown`, so every `VERIFY_DESTINATION` step currently renders
  with unknown elevation by omission rather than by decision.

## Question

For the exact verbs already recorded in `data/policy/provider-commands.toml`
`[entry.verbs]`:

- Homebrew: `brew list --versions <package>`
- MacPorts: `port -q installed <package>`

state `none`, `required` or `unknown` for each of four keys — `verify` and
`verify_gui_app`, per provider — as a cited fact or an observation, never as an
inference from "a read does not write".

Three sub-questions the specification depends on:

1. MacPorts cannot be observed here. Where does the registry `port -q installed`
   reads live under `/opt/local`, and is it documented as readable by an
   unprivileged user?
2. Does `docs.brew.sh/FAQ` — the source already cited at
   `data/policy/provider-commands.toml:20` — cover `brew list`, or does a
   different page?
3. For a Homebrew cask, does verifying differ from verifying a formula in a way
   that changes elevation, and is `brew list --versions` even the right verify
   command for a cask?

## Evidence

### E0 — Quiet environment and identity used for every command below (observed 2026-09-04)

```
$ env | grep -E '^HOMEBREW_' | sort
HOMEBREW_NO_ANALYTICS=1
HOMEBREW_NO_AUTO_UPDATE=1
HOMEBREW_NO_ENV_HINTS=1
```

These three come from the `env` block of `.claude/settings.json` and are the
variables that keep Homebrew from auto-updating, phoning home or printing
hints. `HOMEBREW_NO_INSTALL_FROM_API` was unset, i.e. the default API mode.
No MacPorts quieting was needed: no `port` command was run, because MacPorts is
not installed. (MacPorts is quieted by argv — `-q` — not by environment, and
`-q` is already part of the verb under test.)

```
$ id -un
pirado
$ id -u
501
$ sw_vers
ProductName:		macOS
ProductVersion:		26.6.2
BuildVersion:		25G83
$ uname -m
x86_64
```

Every Homebrew command in this note ran as uid 501, unelevated. No command in
this note elevated, and none mutated anything.

```
$ brew --version
Homebrew 6.0.21
Homebrew/homebrew-core (git revision e045d7385fd; last commit 2026-09-02)
$ brew --prefix
/usr/local
$ git -C /usr/local/Homebrew rev-parse HEAD
560147012b9678b42ef5e83b690f0895552d1366
$ git -C /usr/local/Homebrew log -1 --format='%H %ci'
560147012b9678b42ef5e83b690f0895552d1366 2026-09-01 11:39:08 +0000
```

### E1 — The Homebrew verify verb, on a formula, unprivileged (observed 2026-09-04)

Run with stdin closed, so that any password prompt would have failed rather
than silently succeeded from a cached credential:

```
$ brew list --versions actionlint < /dev/null
actionlint 1.7.12
exit=0
```

No prompt, no stderr, exit status 0.

### E2 — The Homebrew verify verb, on a cask, unprivileged (observed 2026-09-04)

Installed casks on this machine:

```
$ ls /usr/local/Caskroom
adoptopenjdk
copilot-cli
gcloud-cli
gimp
meld
oracle-jdk
visualdiffer
wailbrew
wireshark-chmodbpf
```

The verb exactly as `provider-commands.toml` records it, applied to a cask:

```
$ out=$(brew list --versions gimp </dev/null 2>/dev/null); rc=$?; \
  err=$(brew list --versions gimp </dev/null 2>&1 >/dev/null); \
  echo "rc=$rc"; echo "stdout=[$out]"; echo "stderr=[$err]"
rc=1
stdout=[]
stderr=[]
```

```
$ brew list --versions wireshark-chmodbpf < /dev/null
exit=1
```
(no output on either stream)

The same verb with `--cask` inserted:

```
$ brew list --cask --versions gimp < /dev/null
gimp 3.2.4
exit=0

$ brew list --cask --versions wireshark-chmodbpf < /dev/null
wireshark-chmodbpf 4.6.8
exit=0
```

The reverse, for completeness — `--cask` applied to a formula:

```
$ brew list --cask --versions actionlint < /dev/null
exit=1
```
(no output on either stream)

With **no** named argument, casks *are* included:

```
$ brew list --versions | grep -E '^(gimp|meld|wailbrew|visualdiffer|actionlint) '
actionlint 1.7.12
gimp 3.2.4
meld 3.22.3+105
visualdiffer 2.1.21
wailbrew 0.10.4

$ brew list --cask --versions
adoptopenjdk 16.0.1,9
copilot-cli 1.0.70
gcloud-cli 581.0.0
gimp 3.2.4
meld 3.22.3+105
oracle-jdk 26.0.2.1
visualdiffer 2.1.21
wailbrew 0.10.4
wireshark-chmodbpf 4.6.8
```

The reported version matches the Caskroom directory name, i.e. the answer is
local metadata, not a network lookup:

```
$ stat -f '%Su %Sg %Sp %N' /usr/local/Caskroom /usr/local/Caskroom/gimp /usr/local/Caskroom/gimp/3.2.4
pirado admin drwxrwxr-x /usr/local/Caskroom
pirado admin drwxr-xr-x /usr/local/Caskroom/gimp
pirado admin drwxr-xr-x /usr/local/Caskroom/gimp/3.2.4
```

### E3 — `docs.brew.sh/Manpage`, the `list` entry (read 2026-09-04, <https://docs.brew.sh/Manpage>)

Verbatim, and byte-identical to the local checkout at
`/usr/local/Homebrew/docs/Manpage.md` lines 1551 onwards at the HEAD in E0:

> ### `list`, `ls` \[*`options`*\] \[*`installed_formula`*\|*`installed_cask`* ...\]
>
> List all installed formulae and casks. If *`formula`* is provided, summarise the
> paths within its current keg. If *`cask`* is provided, list its artifacts.
>
> `--formula`
>
> : List only formulae, or treat all named arguments as formulae.
>
> `--cask`
>
> : List only casks, or treat all named arguments as casks.
>
> `--versions`
>
> : Show the version number for installed formulae, or only the specified formulae
>   if *`formula`* are provided.

No option on that page is documented as requiring, accepting or implying
elevation.

### E4 — `docs.brew.sh/FAQ` does not mention `brew list` (read 2026-09-04, <https://docs.brew.sh/FAQ>)

The page (`last_review_date: "2026-08-25"` in the local copy at
`/usr/local/Homebrew/docs/FAQ.md`) says, verbatim:

> **Why does Homebrew say sudo is bad?**
>
> __tl;dr__ Sudo is dangerous, and you installed TextMate.app without sudo anyway.
>
> Homebrew refuses to work using sudo.
>
> Use `sudo` only with software that you trust. Even when you trust Homebrew
> itself, a source build can run large upstream build scripts that have not
> received a security review from Homebrew. Running those scripts as `root`
> would allow them to modify or upload files anywhere permitted by the
> operating system. Some build scripts have attempted to modify `/usr` even
> when configured with another installation prefix.
>
> We use the macOS sandbox to stop this but this doesn't work when run as the
> `root` user (which also has read and write access to almost everything on the
> system). The sandbox is part of Homebrew's wider Software Supply Chain
> Security measures.
>
> Did you `chown root /Applications/TextMate.app`? Probably not. So is it that
> important to `chown root wget`?
>
> Note: Homebrew is primarily designed for single-user use and does not work
> well in multi-user configurations.

Asked explicitly whether the page mentions `brew list`, or distinguishes
read-only commands from mutating ones, or states which commands may be run as
root: **it does none of those three.** The page speaks about Homebrew as a
whole and about builds; it names no subcommand.

### E5 — The command-level root refusal, in the shipped source (observed 2026-09-04)

`/usr/local/Homebrew/Library/Homebrew/brew.sh` lines 164–198, at HEAD
`560147012b9678b42ef5e83b690f0895552d1366` (E0). Verbatim, unchanged from the
copy quoted in R-5 E5 on 2026-09-02:

```sh
check-run-command-as-root() {
  [[ "${EUID}" == 0 || "${UID}" == 0 ]] || return

  # Allow Azure Pipelines/GitHub Actions/Docker/Podman/Concourse/Kubernetes to do everything as root (as it's normal there)
  [[ -f /.dockerenv ]] && return
  [[ -f /run/.containerenv ]] && return
  [[ -f /proc/1/cgroup ]] && grep -E "azpl_job|actions_job|docker|garden|kubepods" -q /proc/1/cgroup && return

  # `brew as-console-user` is run by root-owned MDM/Munki/Jamf workflows so it
  # can immediately dispatch the requested Homebrew command as the console user.
  [[ "${HOMEBREW_COMMAND}" == "as-console-user" ]] && return

  # `brew setup-sandbox` is intended to be run with `sudo` to prepare the
  # Homebrew sandbox.
  [[ "${HOMEBREW_COMMAND}" == "setup-sandbox" ]] && return

  # `brew services` may need `sudo` for system-wide daemons.
  if [[ "${HOMEBREW_COMMAND}" == "services" ]]
  then
    # Need to disable Bootsnap when running as root to avoid permission errors:
    # https://github.com/Homebrew/brew/issues/19904
    export HOMEBREW_NO_BOOTSNAP="1"

    return
  fi

  # It's fine to run this as root as it's not changing anything.
  [[ "${HOMEBREW_COMMAND}" == "--prefix" ]] && return

  odie <<EOS
Running Homebrew as root is extremely dangerous and no longer supported.
As Homebrew does not drop privileges on installation you would be giving all
build scripts full access to your system.
EOS
}
```

`list` is not in the exemption list.

### E6 — MacPorts `port(1)`, the `installed` action and `-q` (read 2026-09-04, <https://man.macports.org/port.1.html>)

Verbatim:

> **installed**: "Show the installed version, variants and activation status
> for each portname. If no arguments are given, all installed ports are
> displayed."

> **-q**: "Quiet mode, suppress informational messages to a minimum, implies -N"

Asked for every passage mentioning root, sudo, privileges, administrator or the
registry: **two**, both usage examples (`sudo port -d sync`,
`sudo port selfupdate`), and **no general statement of which actions require
root privileges**. Same negative result as R-5 E13 on 2026-09-02.

### E7 — MacPorts guide: where the registry lives (read 2026-09-04)

`macports.conf(5)` (<https://man.macports.org/macports.conf.5.html>), verbatim:

> **prefix** — "Sets the directory where ports are installed. Any path may be
> used but those with spaces and/or non ascii characters should be avoided, as
> this can break some ports. This key is often referred to as *${prefix}*."
> Default: `/opt/local`
>
> **portdbpath** — "Directory where MacPorts keeps working data such as
> downloaded sources, installed ports' receipts and the main registry. The same
> path restrictions apply as for *${prefix}*." Default: `${prefix}/var/macports`

The page documents no file permissions, no ownership and no `macportsuser`.

MacPorts guide §6.5.x "Registry Files"
(<https://guide.macports.org/chunked/internals.registry.html>), verbatim from
the guide's own DocBook source, `macports/macports-guide`
`guide/xml/internals.xml` lines 316–326, repository HEAD
`757b3f5a5280ea0a3a699dcad7d1fde60a2f5ff3` (2026-08-26), read 2026-09-04:

> The SQLite registry used by default is located at `${portdbpath}/registry`,
> which by default would be `${prefix}/var/macports/registry`. All data is
> stored in a single file named `registry.db`, although the additional
> directory `portfiles` is used temporarily for extracting stored Portfiles
> from the registry. Furthermore, access to the registry may be locked using
> `.registry.lock` with the `registry::exclusive_lock` and
> `registry::exclusive_unlock` APIs.

MacPorts guide, file hierarchy (`guide/xml/internals-hier.xml` lines 245–249,
published as <https://guide.macports.org/chunked/porthier.html>), verbatim:

> `registry/` — "Contains the registry database in sqlite format."

**Neither page states any permission, mode or ownership for `registry.db`, and
neither says whether a non-root user can read it.** That was the specific
question asked of each, and the specific answer is: not addressed.

### E8 — MacPorts guide: what is printed with `sudo` and what without (read 2026-09-04)

From the guide's own source, `macports/macports-guide`
`guide/xml/using.xml`, HEAD `757b3f5a5280ea0a3a699dcad7d1fde60a2f5ff3`.

The one general remark the guide makes about elevation, line 1239, opening
§3.2 "Common Tasks":

> Mind the <quote>sudo</quote> for some of the subsequent examples, which is
> necessary if you have a default MacPorts installation.

§3.1.x "port installed", lines 852–860, verbatim including the prompt:

> **port installed**
>
> The `installed` action displays the installed versions and variants of the
> specified ports, or if no ports are specified, all installed ports. It also
> displays whether a port is "active", i.e., whether the files belonging to
> this port are currently present on disk or inactive, i.e., stashed away in a
> compressed tarball.
>
> `$ port installed`

Lines 940–944, the closest thing in the guide to a side-by-side contrast — a
read of one named port immediately followed by a write to it, in a single note:

> `$ port installed `*`portname`*
>
> to determine the version number of the old version you want to re-activate,
> and run
>
> `$ sudo port activate `*`portname`*` @`*`old-version`*

Other unprefixed `port installed` examples in the same file: line 1329
(`$ port installed inactive`), line 1409 (`$ port installed requested and
rdependentof:portname`), line 1591 (`$ port installed requested`).

Asked whether §3 contains a sentence stating that some actions require root or
administrator privileges: the only one is the line-1239 remark above. A
targeted grep of `installing.xml`, `intro.xml`, `internals.xml`,
`internals-hier.xml` and `macports.conf.xml` for "privileg", "administrator",
"as root", "non-root", "without root", "readable" and "permissions" returned
exactly one hit, in `installing.xml` lines 455–456, and it is about the
uninstall procedure:

> privilege separation. If you want to remove them, you can use these commands
> from an account that has admin privileges:

### E9 — MacPorts base: which actions check that the prefix is writable (read 2026-09-04)

`macports/macports-base` HEAD `c9797f69df0c00fd5fdbe0da51c86276e0698730`
(2026-08-31), fetched through the GitHub API.

`src/port/port.tcl` lines 1381–1390, verbatim including its comment:

```tcl
# check if the install prefix is writable
# should be called by actions that will modify it
proc prefix_unwritable {} {
    global macports::portdbpath
    if {[file writable $portdbpath]} {
        return 0
    } else {
        ui_error "Insufficient privileges to write to MacPorts install prefix."
        return 1
    }
}
```

Its call sites, all of them:

```
$ grep -n "prefix_unwritable" port.tcl
1382:proc prefix_unwritable {} {
2196, 2226, 2416, 2497, 2531, 2549, 2560, 2567, 2594, 2625, 3016, 3928
```

`action_installed` begins at line 3074 and is **not** among them:

```tcl
proc action_installed { action portlist opts } {
    global private_options
    set status 0
    set restrictedList 0
    set ilist [list]

    if { [llength $portlist] || (![info exists private_options(ports_no_args)] || $private_options(ports_no_args) eq "no")} {
        set restrictedList 1
        foreachport $portlist {
            set composite_version [composite_version $portversion $variations]
            if {[catch {lappend ilist {*}[registry_installed $portname $composite_version all no]} result]} {
```

`src/macports1.0/macports.tcl` lines 2014–2016 — the registry is opened during
`mportinit`, i.e. for *every* action including read-only ones:

```tcl
    # init registry
    set db_path [file join ${registry.path} registry registry.db]
    set db_exists [file exists $db_path]
    registry::open $db_path
```

`src/cregistry/registry.c` opens that path with plain `sqlite3_open` and
contains no `chmod`, no `umask` and no explicit mode:

```
$ grep -n "sqlite3_open\|chmod\|umask\|0644\|SQLITE_OPEN" registry.c
130:    if (sqlite3_open(NULL, &reg->db) == SQLITE_OK) {
518:    if (sqlite3_open(db_path, &db) == SQLITE_OK) {
```

### E10 — MacPorts base: the mode of the registry *directory* (read 2026-09-04)

`macports/macports-base` `doc/base.mtree.in`, in full, at the HEAD in E9:

```
# MacPorts filesystem hierarchy, for internal use only. Changes to this file will not stick across installations.
#

/set type=dir uname=root gname=@MTREE_GNAME@ mode=@DSTMODE@
.
    man type=link link=share/man
    ..
    var
        macports
            build
            ..
            distfiles
            ..
            home
                Library
                    Preferences
                    ..
                ..
            ..
            logs
            ..
            registry
            ..
            sip-workaround mode=01777
            ..
            software
            ..
        ..
    ..
..
```

`aclocal.m4` lines 669–686, the definition of `@DSTMODE@`:

```m4
# MP_DIRECTORY_MODE
#-------------------------------------------------
AC_DEFUN([MP_DIRECTORY_MODE],[
	dnl if the user specifies --with-directory-mode,
	dnl use the specified permissions for ${prefix} directories,
	dnl otherwise use 0755
	AC_REQUIRE([MP_PATH_MPCONFIGDIR])

	AC_ARG_WITH(directory-mode, [AS_HELP_STRING([--with-directory-mode=MODE],[specify directory mode of installed directories])], [ DSTMODE=$withval ] )

	AC_MSG_CHECKING([what permissions to use for installation directories])
	if test "x$DSTMODE" = "x" ; then
		DSTMODE=0755
	fi

	AC_MSG_RESULT([$DSTMODE])
	AC_SUBST(DSTMODE)
])
```

`configure.ac` lines 240–250, the definition of `@MTREE_GNAME@`:

```
# Don't set real uname or gname in .mtree files unless root
# https://trac.macports.org/ticket/49501
if test "$DSTUSR" = "root" ; then
    MTREE_GNAME="$DSTGRP"
else
    # This has to be set to something or mtree won't create any directories.
    # It doesn't matter what it's set to for non-root though, as it can't
    # change the ownership anyway.
    MTREE_GNAME="admin"
fi
AC_SUBST(MTREE_GNAME)
```

The guide corroborates the resulting prefix mode in a transcript of
`selfupdate` output (`guide/xml/using.xml` line 67, published as
<https://guide.macports.org/chunked/using.html>):

> `Installing new MacPorts release in /opt/local as root:admin; permissions 755`

Note the scope: this is the mode of *directories*, including
`${prefix}/var/macports/registry`. The mode of the `registry.db` *file* is not
set by any of it.

### E11 — How the planner consults these cells (read 2026-09-04, this repository)

`crates/application/src/planner.rs` lines 590–606:

```rust
    let mut argv: Vec<String> = entry
        .and_then(|e| e.verbs.get(operation).cloned())
        .unwrap_or_else(|| vec![format!("<{operation}>")]);
    argv.push(package.to_string());
    let elevation_key = match kind {
        PackageKind::GuiApp => format!("{operation}_gui_app"),
        PackageKind::Package => operation.to_string(),
    };
    let elevation = match entry.and_then(|e| e.elevation.get(&elevation_key).map(String::as_str)) {
        Some("none") => Elevation::None,
        Some("required") => Elevation::Required { ... },
        _ => Elevation::Unknown,
    };
```

Lines 199–202, where `kind` comes from:

```rust
    let kind = source_row
        .installed
        .first()
        .map_or(PackageKind::Package, |r| r.kind);
```

The `VERIFY_DESTINATION` step is built at lines 386–410 with
`command(&commands, dest_snapshot, &request.to, "verify", &dest_name, kind)`.

`crates/adapters/src/macports/mod.rs:497` is the only place the MacPorts adapter
sets a kind, and it sets `PackageKind::Package`;
`crates/adapters/src/homebrew/mod.rs:489` and `:597` set `Package` and `GuiApp`
respectively. `crates/domain/src/package.rs:12-21`:

```rust
/// What kind of thing a provider calls a package. `GuiApp` is the Homebrew
/// cask concept; the other provider has no equivalent (R-1, SE-012).
```

## Findings

1. **`brew list --versions <formula>` needs no elevation: `none`, observed and
   cited.** Observed: it ran as uid 501 with stdin closed and returned
   `actionlint 1.7.12`, exit 0, no prompt (E1). Cited, and stronger than
   "needs none": Homebrew *refuses* to run `list` as root — `list` is absent
   from the exemption list in `check-run-command-as-root` (E5), so
   `sudo brew list` aborts with "Running Homebrew as root is extremely
   dangerous and no longer supported." Elevation is therefore not merely
   unnecessary for this verb, it is forbidden by the provider. The general
   policy statement behind that code is E4 ("Homebrew refuses to work using
   sudo").

2. **The existing citation at `data/policy/provider-commands.toml:20` does not
   cover `brew list`.** `docs.brew.sh/FAQ` names no subcommand, and does not
   distinguish read-only from mutating commands (E4). It supports the
   `elevation_reason` sentence as written, but it is not the source for the
   `verify` cell. The source for the `verify` cell is E5 (the shipped
   `brew.sh` at a named revision) plus the observation E1, with
   `docs.brew.sh/Manpage` (E3) for the verb's own semantics.

3. **`brew list --cask --versions <cask>` needs no elevation either: `none`,
   observed and cited.** Observed on two casks, one of which
   (`wireshark-chmodbpf`) is a `pkg`-installing cask: both returned a version
   and exit 0 as uid 501 with stdin closed, no prompt (E2). Cited: `list` is
   not exempt from the root refusal (E5), so this verb cannot be elevated at
   all. The reason a cask's *install* may elevate — a `sudo: true` stanza on an
   `installer script:` (R-5 E9) — does not reach `list`, because `list` runs no
   cask script; it reports the Caskroom, whose directory names match the
   reported versions exactly and whose permissions are `pirado:admin` (E2).
   This is not the "reads do not write" inference: it rests on the provider's
   own refusal to be root for this command and on running it.

4. **But `brew list --versions <cask>` is the wrong verify command, and it
   fails silently.** Observed: exit status 1, empty stdout, empty stderr, for
   both casks tried (E2). Documented: `--versions` is defined as "Show the
   version number for installed formulae, or only the specified formulae if
   *formula* are provided" — formulae only, once a name is given — and `--cask`
   is what makes a named argument be treated as a cask (E3). Note the trap:
   with *no* named argument `brew list --versions` does list casks (E2), so a
   test that checks the bare command would not catch this. Because
   `crates/application/src/planner.rs` takes `argv` from `verbs[operation]`
   and only the *elevation key* from the kind (E11), a `VERIFY_DESTINATION`
   step for a cask today would emit `brew list --versions <cask>` — a command
   that reports failure for a correctly installed cask. That is a plan-content
   defect, separate from and more serious than the elevation question this note
   was asked. The correct verb is `brew list --cask --versions <cask>`, and
   `provider-commands.toml` has no way to express a kind-dependent verb: the
   verbs table has `add`, `remove`, `verify` and no `*_gui_app` counterparts.

5. **MacPorts: the registry `port -q installed` reads is at
   `${portdbpath}/registry/registry.db`, i.e.
   `/opt/local/var/macports/registry/registry.db` by default — cited (E7).**
   The containing directory is created `uname=root`, group `admin` (or
   `$DSTGRP`), mode `0755` by default, per the project's own mtree and
   `configure` machinery (E10), corroborated by the guide's own selfupdate
   transcript "in /opt/local as root:admin; permissions 755" (E10). **The mode
   of `registry.db` itself is documented nowhere.** `sqlite3_open` creates it
   with no explicit mode and no `chmod` in MacPorts' code (E9), which makes the
   file's mode a function of the creating process's umask — a machine
   property, not a documented one.

6. **No MacPorts document states whether `port installed` requires
   privileges.** `port(1)` has no general privilege statement and mentions
   `sudo` only in two unrelated examples (E6). The guide's only general remark
   is "Mind the 'sudo' for some of the subsequent examples, which is necessary
   if you have a default MacPorts installation" (E8) — which tells you the
   examples carry the information but does not itself say which actions need
   it. What the guide *does* do is print `port installed portname` without
   `sudo` two lines above `sudo port activate portname @old-version` in one
   note (E8), and print `port installed` unprefixed in four more places (E8).
   And MacPorts' own source shows `action_installed` is not one of the twelve
   call sites of `prefix_unwritable`, the procedure documented in-source as
   "should be called by actions that will modify it" (E9).

7. **That body of evidence points at `none` but does not reach it as a cited
   fact, so the recommendation is `unknown`.** Everything in finding 6 is
   evidence-by-example plus a source-level negative; none of it is a statement
   that the action needs no privileges, and none of it was observed. The one
   thing that could make it false — the mode of `registry.db` (finding 5) — is
   undocumented and umask-dependent, which is precisely the kind of variance
   the project answers with runtime capability detection rather than a static
   cell. Recording `none` here would be reasoning to a conclusion the sources
   do not carry, and it would understate an elevation prompt that a user on an
   unusually-configured prefix would actually see.

8. **MacPorts' `verify_gui_app` cell is reachable, and `unknown` is right for
   it — but not only for the reason the file currently gives.** The file's
   existing comment on `add_gui_app`/`remove_gui_app` says MacPorts has no
   GUI-application package kind, and that is confirmed: the MacPorts adapter
   sets `PackageKind::Package` and nothing else, and the domain says outright
   that `GuiApp` "is the Homebrew cask concept; the other provider has no
   equivalent" (E11). But the planner takes `kind` from the **source** row and
   uses that same `kind` for the **destination** lookup (E11). So migrating a
   Homebrew cask towards MacPorts consults MacPorts'
   `verify_gui_app`/`add_gui_app`/`remove_gui_app` cells even though MacPorts
   will never produce a `GuiApp` itself. The cells are load-bearing, not merely
   defensive, and `unknown` is the honest value for "this provider has no such
   concept, so no elevation claim can be made about it". It is also the value
   that keeps the whole GUI-app path aligned with the `MANUAL_REVIEW` that
   `PackageClass::GuiApp` already forces (`crates/policy/src/rules/manual_review_class.rs`).

### The four cells

| Key | Provider | Recommended | Kind of claim | Evidence |
|---|---|---|---|---|
| `verify` | homebrew | **`none`** | **cited + observed** | E5 (`list` not exempt from the root refusal, `Homebrew/brew` @ `560147012b9678b42ef5e83b690f0895552d1366`), E1 (`brew list --versions actionlint` as uid 501, exit 0), E3 (Manpage), E4 (FAQ, for the general policy only) |
| `verify_gui_app` | homebrew | **`none`** | **cited + observed** | E5, E2 (`brew list --cask --versions gimp` / `wireshark-chmodbpf` as uid 501, exit 0), E3. **Flagged:** the *verb* is wrong for a cask — finding 4 |
| `verify` | macports | **`unknown`** | **UNRESOLVED** — documentation does not settle it | E6, E7, E8, E9, E10. Points at `none`; not stated by any source; not observed; depends on an undocumented file mode |
| `verify_gui_app` | macports | **`unknown`** | cited *as a non-applicability*, `unknown` as the value | E11 (adapter and domain: MacPorts has no `GuiApp`), and the cell is reachable through a cask *source* — finding 8 |

Suggested `source` / `read_on` for whoever edits the file:

- Homebrew, both cells — `source = "https://docs.brew.sh/Manpage (list, --cask, --versions); Homebrew/brew Library/Homebrew/brew.sh check-run-command-as-root @ 560147012b9678b42ef5e83b690f0895552d1366; docs/research/R-5a-verify-command-elevation.md"`, `read_on = "2026-09-04"`.
- MacPorts, both cells — `source = "https://man.macports.org/port.1.html (installed, -q); https://guide.macports.org/chunked/internals.registry.html; docs/research/R-5a-verify-command-elevation.md"`, `read_on = "2026-09-04"`.

Editing `data/policy/provider-commands.toml` also means bumping `version` in
`data/policy/policy.toml` per `.claude/rules/policy-data.md`; this note does not
do either.

## Cannot determine

- **UNRESOLVED-1 — whether `port -q installed <portname>` needs elevation.**
  No MacPorts document states it (E6, E8), and MacPorts is not installed here.
  *What would settle it:* on a machine with a default MacPorts install, as a
  non-root user — `stat -f '%Su %Sg %Sp %N' /opt/local/var/macports/registry
  /opt/local/var/macports/registry/registry.db`, then
  `port -q installed <someport>` and its exit status. Alternatively an official
  sentence saying which actions require root. Until then the adapter should
  answer this by runtime capability detection (attempt the read, report
  `CapabilityState::Unknown` rather than `Available` if it cannot), and the
  data file should say `unknown`.

- **UNRESOLVED-2 — the mode of `registry.db`.** Not documented (E7), not set by
  MacPorts' code (E9), therefore umask-dependent. Even a single observation on
  one machine would not generalise; this is a per-machine fact and belongs to
  detection. *What would settle the general question:* an upstream change that
  fixes the mode explicitly, or documentation of it.

- **Apple Silicon not observed.** Findings 1–4 are `/usr/local` on Intel. The
  root refusal (E5) is architecture-independent source, so finding 1 and 3
  should hold on `/opt/homebrew`, but the Caskroom ownership in E2 was not
  observed there.

- **Only two of nine installed casks were tried** (E2). Both behaved
  identically, and the mechanism (reading Caskroom directory names) does not
  vary by cask, but the observation is two, not nine.

- **Whether `brew list --cask --versions` is fully offline was not proven.** It
  returned versions matching Caskroom directory names (E2), which is consistent
  with a purely local read, and `HOMEBREW_NO_AUTO_UPDATE=1` was set (E0). No
  network-isolated run was performed. *What would settle it:* the same command
  under a network-denied sandbox, or a recording captured with
  `scripts/capture-fixture.sh` and replayed. This matters because `analyze`
  must work offline.

- **The MacPorts trac FAQ is still unreadable.** <https://trac.macports.org/wiki/FAQ>
  returned an access-denied page behind the Anubis challenge again on
  2026-09-04 (error id `9e4edb5b6b850c41`, the same id R-5 E18 recorded on
  2026-09-02). Anything it may say about non-root MacPorts is absent here.

## Impact on the specification

Nothing here contradicts a Tier 1 or Tier 2 statement. Three things for the
main agent to weigh, none of which this note acts on:

1. **`data/policy/provider-commands.toml` cannot express a kind-dependent
   verb, and needs to.** Finding 4 is a live defect, not a documentation gap:
   a cask `VERIFY_DESTINATION` step would emit `brew list --versions <cask>`,
   which exits 1 with no output on a correctly installed cask (E2). The
   elevation table is already kind-aware (`add_gui_app`, `remove_gui_app`); the
   verbs table is not. Either the verbs table gains `verify_gui_app`
   (`["list", "--cask", "--versions"]`) and the planner looks up the verb by
   the same key it already uses for elevation, or the reason a cask never
   reaches a plan needs to be stated and tested. `PackageClass::GuiApp` forces
   `MANUAL_REVIEW` (`crates/policy/src/rules/manual_review_class.rs`), which
   may make this unreachable in practice — but "unreachable in practice" is not
   what `crates/application/src/planner.rs:590-606` says, and the four
   `*_gui_app` elevation cells exist precisely because the path is considered
   reachable. It cannot be both.

2. **`04-migration-and-safety.md`, "Privilege" — the refinement R-5 already
   offered gets sharper.** For Homebrew, elevation is not a provider-level
   property *nor* purely a package-level one: it is a property of the
   (command, package-kind) pair. `brew install <cask>` may elevate; `brew list
   --cask --versions <cask>` provably cannot, because Homebrew refuses to run
   `list` as root at all (E5). A plan that marks every cask operation
   `unknown` for safety would over-warn on the verify step. This is an argument
   for filling `verify_gui_app = "none"` rather than leaving it absent.

3. **`data/policy/provider-commands.toml:20`'s citation is over-broad for what
   the file will use it for.** `docs.brew.sh/FAQ` supports the
   `elevation_reason` sentence, but it names no subcommand (E4, finding 2). If
   the `verify` cells are added citing the entry-level `source` line, the file
   would be attributing a command-level fact to a page that does not carry one.
   The entry-level `source` should gain the `brew.sh` revision, or the note
   reference should carry it.

## Re-check when

- **Homebrew's `check-run-command-as-root` exemption list changes.** E5 is
  source at revision `560147012b9678b42ef5e83b690f0895552d1366`. If `list` ever
  joins the exemptions, finding 1's strongest leg goes away and only the
  observation remains. Re-read on any Homebrew major-version bump.
- **`brew list`'s treatment of named cask arguments changes.** Finding 4 is the
  behaviour of Homebrew 6.0.21; if `--versions` learns to resolve a named cask
  without `--cask`, the defect disappears and the verb table needs revisiting
  in the other direction. The Manpage wording in E3 is the thing to re-read.
- **MacPorts is installed on a test machine.** That converts UNRESOLVED-1 from
  `unknown` to an observation and is the single highest-value follow-up here,
  as it was in R-5.
- **MacPorts documents which actions require root.** The absence in E6 and E8
  is a negative result of targeted searches, not proof; a future guide revision
  could state it plainly and make the MacPorts cell a cited `none`.
- **Time-sensitivity:** none of these facts has a published expiry. The
  Homebrew FAQ carries `last_review_date: "2026-08-25"`; the MacPorts guide
  carries no review date, so its citations are the ones that can go stale
  silently.
