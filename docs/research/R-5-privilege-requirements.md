# R-5 — Which operations of each provider require administrator elevation, and does anything the Foundation Pass does?

- **Status:** partially answered — Homebrew observed and cited; MacPorts
  citation-only (not installed on this machine); three cells UNRESOLVED and
  deferred to runtime capability detection
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (build 25G83), x86_64 (Intel), Homebrew 6.0.21 @
  `/usr/local` (`Homebrew/brew` HEAD `560147012b9678b42ef5e83b690f0895552d1366`),
  MacPorts not installed
- **Blocking:** `01-invariants.md` INV-06 (least privilege);
  `04-migration-and-safety.md` (Privilege: "the design assumes MacPorts
  operations generally require elevation and Homebrew's normally do not; R-5 in
  the research checklist verifies that per operation");
  `10-first-pass-work-order.md` (the Foundation Pass binary must contain no
  elevating code path)

## Question

Exactly which operations of each provider require elevation, on each
architecture and prefix, and confirmation that everything the Foundation Pass
performs — read-only inventory, metadata, dependency queries, prefix and
configuration queries — requires none.

Two sub-questions the specification depends on and that this note keeps
separate:

- Elevation is a property of *the prefix's ownership and of what the operation
  writes*, not of the provider's name. A note that says "Homebrew never needs
  root" is wrong in a way that matters.
- Whether a *source build* changes the answer. It does not change elevation for
  either provider; it changes duration and toolchain. Recorded here explicitly
  because "forced source build" is elsewhere a state with its own
  recommendation, and this is the one axis on which it is not.

## Evidence

### E0 — Quiet environment used for every provider invocation below

Set by `.claude/settings.json` (`env` block) and confirmed present in the
process environment. Observed 2026-09-02:

```
$ env | grep -E '^HOMEBREW_' | sort
HOMEBREW_NO_ANALYTICS=1
HOMEBREW_NO_AUTO_UPDATE=1
HOMEBREW_NO_ENV_HINTS=1
```

`brew config` (E3) independently echoes all three as `set`. `HOMEBREW_NO_INSTALL_FROM_API`
is unset, i.e. the default API mode. No MacPorts equivalent was needed, because
no `port` command was run (MacPorts is not installed).

### E1 — Identity and platform (observed 2026-09-02)

```
$ id -un
pirado

$ id
uid=501(pirado) gid=20(staff) groups=20(staff),101(access_bpf),12(everyone),61(localaccounts),79(_appserverusr),80(admin),81(_appserveradm),98(_lpadmin),33(_appstore),100(_lpoperator),204(_developer),250(_analyticsusers),395(com.apple.access_ftp),398(com.apple.access_screensharing),399(com.apple.access_ssh),400(com.apple.access_remote_ae),701(com.apple.sharepoint.group.1)

$ sw_vers
ProductName:		macOS
ProductVersion:		26.6.2
BuildVersion:		25G83

$ uname -m
x86_64
```

Every command in this note ran as uid 501, a member of `admin` but not
elevated. No command in this note elevated.

### E2 — Homebrew prefix ownership and permissions (observed 2026-09-02)

```
$ ls -ld /usr/local /usr/local/Cellar /usr/local/bin /usr/local/Homebrew /usr/local/opt /usr/local/lib /usr/local/share /usr/local/etc /usr/local/var /usr/local/Caskroom
drwxr-xr-x    16 root    wheel    512 Aug 21 03:05 /usr/local
drwxrwxr-x@   11 pirado  admin    352 Aug 24 16:42 /usr/local/Caskroom
drwxrwxr-x@  183 pirado  admin   5856 Sep  2 09:20 /usr/local/Cellar
drwxr-xr-x@   30 pirado  admin    960 Sep  1 19:18 /usr/local/Homebrew
drwxrwxr-x@ 1088 pirado  admin  34816 Sep  2 09:20 /usr/local/bin
drwxrwxr-x@   27 pirado  admin    864 Sep  1 20:19 /usr/local/etc
drwxrwxr-x@  661 pirado  admin  21152 Sep  2 09:20 /usr/local/lib
drwxrwxr-x@  226 pirado  admin   7232 Sep  2 09:20 /usr/local/opt
drwxrwxr-x@   63 pirado  admin   2016 Sep  2 09:20 /usr/local/share
drwxrwxr-x@    9 pirado  admin    288 Sep  1 20:03 /usr/local/var
```

```
$ for p in /usr/local /usr/local/Cellar /usr/local/bin /usr/local/Homebrew /usr/local/opt /usr/local/lib /usr/local/share /usr/local/var /usr/local/Caskroom /usr/local/Homebrew/bin/brew; do stat -f '%Su %Sg %Sp %N' "$p"; done
root wheel drwxr-xr-x /usr/local
pirado admin drwxrwxr-x /usr/local/Cellar
pirado admin drwxrwxr-x /usr/local/bin
pirado admin drwxr-xr-x /usr/local/Homebrew
pirado admin drwxrwxr-x /usr/local/opt
pirado admin drwxrwxr-x /usr/local/lib
pirado admin drwxrwxr-x /usr/local/share
pirado admin drwxrwxr-x /usr/local/var
pirado admin drwxrwxr-x /usr/local/Caskroom
pirado admin -rwxr-xr-x /usr/local/Homebrew/bin/brew
```

Writability, tested read-only with `test -w`:

```
$ for p in /usr/local /usr/local/bin /usr/local/Cellar /usr/local/opt /usr/local/Homebrew; do if [ -w "$p" ]; then echo "writable-by-$(id -un): $p"; else echo "NOT-writable-by-$(id -un): $p"; fi; done
NOT-writable-by-pirado: /usr/local
writable-by-pirado: /usr/local/bin
writable-by-pirado: /usr/local/Cellar
writable-by-pirado: /usr/local/opt
writable-by-pirado: /usr/local/Homebrew
```

The full top level of the prefix, showing that `/usr/local` is a *shared*
root-owned directory that Homebrew does not own outright:

```
$ ls -l /usr/local
total 0
drwxrwxr-x@   11 pirado  admin    352 Aug 24 16:42 Caskroom
drwxrwxr-x@  183 pirado  admin   5856 Sep  2 09:20 Cellar
drwxrwxr-x@    4 pirado  admin    128 Oct 13  2024 Frameworks
drwxr-xr-x@   30 pirado  admin    960 Sep  1 19:18 Homebrew
drwxr-xr-x    18 root    wheel    576 Jun 26 20:30 aws-cli
drwxrwxr-x@ 1088 pirado  admin  34816 Sep  2 09:20 bin
drwxrwxr-x@   27 pirado  admin    864 Sep  1 20:19 etc
drwxrwxr-x@  227 pirado  admin   7264 Sep  2 09:20 include
drwxrwxr-x@  661 pirado  admin  21152 Sep  2 09:20 lib
drwxr-xr-x     3 root    wheel     96 Mar 11 23:08 microsoft
drwxrwxr-x@  226 pirado  admin   7232 Sep  2 09:20 opt
drwxrwxr-x@   12 pirado  admin    384 Aug  5 15:35 sbin
drwxrwxr-x@   63 pirado  admin   2016 Sep  2 09:20 share
drwxrwxr-x@    9 pirado  admin    288 Sep  1 20:03 var
```

```
$ command -v brew; ls -l "$(command -v brew)"
/usr/local/bin/brew
lrwxr-xr-x@ 1 pirado  admin  20 Feb 14  2024 /usr/local/bin/brew -> ../Homebrew/bin/brew
```

### E3 — Homebrew prefix and configuration queries, unprivileged (observed 2026-09-02)

```
$ brew --prefix
/usr/local

$ brew --repository
/usr/local/Homebrew

$ brew --version
Homebrew 6.0.21
Homebrew/homebrew-core (git revision e045d7385fd; last commit 2026-09-02)

$ brew config
HOMEBREW_VERSION: 6.0.21
ORIGIN: https://github.com/Homebrew/brew
HEAD: 560147012b9678b42ef5e83b690f0895552d1366
Last commit: 25 hours ago
Branch: stable
Core tap HEAD: e045d7385fd7ad8907621300224f8a860bdd7bbb
Core tap last commit: 3 hours ago
Core cask tap: N/A
HOMEBREW_PREFIX: /usr/local
HOMEBREW_NO_ANALYTICS: set
HOMEBREW_NO_AUTO_UPDATE: set
HOMEBREW_NO_ENV_HINTS: set
Homebrew Ruby: 4.0.6 => /usr/local/Homebrew/Library/Homebrew/vendor/portable-ruby/4.0.6/bin/ruby
CPU: dodeca-core 64-bit kabylake
Clang: 21.0.0 build 2100
Git: 2.55.0 => /usr/local/bin/git
Curl: 8.7.1 => /usr/bin/curl
macOS: 26.6.2-x86_64
CLT: 26.6.0.0.1781586589
Xcode: 26.5
```

### E4 — Homebrew inventory, intent and dependency queries, unprivileged (observed 2026-09-02)

```
$ brew list --versions | head -3
actionlint 1.7.12
ada-url 4.0.0
aom 3.14.1

$ brew leaves | head -3
actionlint
bvi
cask

$ brew deps --installed | head -2
actionlint: gmp shellcheck
ada-url: fmt

$ brew leaves --installed-on-request | head -3
actionlint
bvi
cask
```

The provider's own record of user intent, read directly from the receipt:

```
$ head -c 200 /usr/local/Cellar/actionlint/*/INSTALL_RECEIPT.json
{
  "homebrew_version": "5.1.12-36-g6c8be83",
  "used_options": [],
  "unused_options": [],
  "built_as_bottle": true,
  "poured_from_bottle": true,
  "loaded_from_api": true,
  "loaded_from_internal_

$ grep -o '"installed_on_request":[^,]*' /usr/local/Cellar/actionlint/*/INSTALL_RECEIPT.json | head -3
"installed_on_request": true

$ stat -f '%Su %Sg %Sp %N' /usr/local/Cellar/actionlint/*/INSTALL_RECEIPT.json
pirado admin -rw-r--r-- /usr/local/Cellar/actionlint/1.7.12/INSTALL_RECEIPT.json
```

### E5 — Homebrew's refusal to run as root, in the shipped source (observed 2026-09-02)

`/usr/local/Homebrew/Library/Homebrew/brew.sh`, lines 164–198, at `Homebrew/brew`
HEAD `560147012b9678b42ef5e83b690f0895552d1366` (E3):

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

### E6 — docs.brew.sh FAQ (read 2026-09-02, <https://docs.brew.sh/FAQ>)

Verbatim, and identical to the local checkout at
`/usr/local/Homebrew/docs/FAQ.md` lines 112–160:

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
> system).

> **Why should I install Homebrew in the default location?**
>
> Homebrew's pre-built binary packages (known as bottles) of many formulae can
> only be used if you install in the default installation prefix, otherwise
> they have to be built from source. […] The default prefix is:
>
> * `/opt/homebrew` for macOS on Apple Silicon,
> * `/usr/local` for macOS on Intel, and
> * `/home/linuxbrew/.linuxbrew` for Linux.

> **Why is the default installation prefix `/opt/homebrew` on Apple Silicon?**
>
> The prefix `/opt/homebrew` was chosen to allow installations in
> `/opt/homebrew` for Apple Silicon and `/usr/local` for Rosetta 2 to coexist
> and use bottles.

> **What are the default ownership and permissions used by Homebrew?**
>
> Ownership on macOS, all subdirectories and files use a forced default of
> `admin` user group (instead of lower default user group `staff`) and the
> current user that executed the installation.
>
> By default, permissions for Homebrew-managed directories and files are
> `0755 (u=rwx,g=rx,o=rx)` on both macOS and Linux. This means that only the
> owning user (typically the installing user) can modify or replace files
> within the Homebrew prefix, while all users are allowed to read and execute
> installed binaries.

Note the doc says nothing about *why* the prefix is user-owned on Intel beyond
"pre-built binary packages … can only be used if you install in the default
installation prefix". The ownership rationale is in the sudo FAQ above, not in
a prefix-specific statement.

### E7 — docs.brew.sh Installation (read 2026-09-02, <https://docs.brew.sh/Installation>)

Verbatim, identical to `/usr/local/Homebrew/docs/Installation.md`
(`last_review_date: "2026-08-27"`):

> The script installs Homebrew to its default prefix (`/opt/homebrew` for Apple
> Silicon, `/usr/local` for macOS Intel and `/home/linuxbrew/.linuxbrew` for
> Linux) so that [you don't need *sudo* after Homebrew's initial installation]
> when you `brew install`.

The only other occurrences of `sudo` / `root` on that page are in the
`HOMEBREW_PKG_USER` override recipe for the `.pkg` installer:

> ```
> sudo defaults write /var/tmp/.homebrew_pkg_user HOMEBREW_PKG_USER penny
> sudo chown root:wheel /var/tmp/.homebrew_pkg_user.plist
> sudo chmod 600 /var/tmp/.homebrew_pkg_user.plist
> sudo chmod -N /var/tmp/.homebrew_pkg_user.plist
> ```
>
> The file must be a regular non-symlink file owned by `root`, have mode `0600`
> and have no access control list.

Asked explicitly whether the page still carries an "Untar anywhere" or
"Multiple installations" section (the historical home of the unsupported
no-root install): **it does not** — the only "Alternative installs" subsection
is "Linux or Windows 10 Subsystem for Linux".

### E8 — docs.brew.sh Manpage, `services` (read 2026-09-02, <https://docs.brew.sh/Manpage>)

Verbatim, identical to `/usr/local/Homebrew/docs/Manpage.md` lines 2010–2029:

> ### `services` [*subcommand*]
>
> Manage background services with macOS' `launchctl`(1) daemon manager or
> Linux's `systemctl`(1) service manager.
>
> If `sudo` is passed, operate on `/Library/LaunchDaemons` or
> `/usr/lib/systemd/system` (started at boot). Otherwise, operate on
> `~/Library/LaunchAgents` or `~/.config/systemd/user` (started at login).
>
> `--sudo-service-user`
> : When run as root on macOS, run the service(s) as this user.

Usage lines, verbatim (the bracketed `sudo` is optional in every one):

> ```
> [sudo] brew services stop [--keep] [--no-wait|--max-wait=] (formula|--all)
> [sudo] brew services start (formula|--all) [--file=]
> [sudo] brew services run (formula|--all) [--file=]
> [sudo] brew services restart (formula|--all) [--file=]
> [sudo] brew services [list] [--json] [--debug]
> [sudo] brew services kill (formula|--all)
> [sudo] brew services info (formula|--all) [--json]
> [sudo] brew services cleanup
> ```

Corresponding launchd directory ownership on this machine (observed 2026-09-02):

```
$ ls -ld ~/Library/LaunchAgents /Library/LaunchAgents /Library/LaunchDaemons
drwxr-xr-x  21 root    wheel  672 Aug 21 03:05 /Library/LaunchAgents
drwxr-xr-x  25 root    wheel  800 Sep  2 07:09 /Library/LaunchDaemons
drwx------@ 10 pirado  staff  320 Sep  1 14:28 /Users/pirado/Library/LaunchAgents
```

### E9 — Homebrew casks can require elevation (observed 2026-09-02)

`/usr/local/Homebrew/docs/Cask-Cookbook.md` at the same HEAD (also published at
<https://docs.brew.sh/Cask-Cookbook>):

- line 190: "`installer script:` is not sandboxed. Many installer scripts are
  vendor installers that require broad filesystem writes, macOS services or
  `sudo`; macOS sandboxing does not work for root processes […]"
- line 739 and line 1118, in the `installer script:` and `uninstall script:`
  tables: "| `sudo:` | set to `true` if the script needs *sudo* |"
- line 963: "[`script:`] (string or hash) - relative path to an uninstall
  script; use a hash to provide arguments or request `sudo: true`"
- line 656: "`set_ownership`: recursively change existing path ownership with
  `sudo chown`; example: `set_ownership "Shared/payload", user: "root", group:
  "wheel"`."

### E10 — MacPorts: installation requires an administrator account (read 2026-09-02, <https://www.macports.org/install.php>)

Verbatim (typo in the original preserved):

> These steps need to be perfomed from an administrator account, for which
> "sudo" will ask the password upon installation.

and

> your chosen installation prefix (the value passed to configure's `--prefix`
> flag, defaulting to `/opt/local`)

### E11 — MacPorts guide, chapter 3 "Using MacPorts" (read 2026-09-02, <https://guide.macports.org/chunked/using.html>)

Commands the guide prints **with** `sudo`:

> `sudo port install apache2 -preforkmpm +workermpm`
> `sudo port uninstall libcomerr`
> `sudo port upgrade outdated`
> `sudo port load prometheus`   → `---> Loading startupitem 'prometheus' for prometheus`
> `sudo port unload prometheus` → `---> Unloading startupitem 'prometheus' for prometheus`
> `sudo port clean apache2`
> `sudo port rev-upgrade`

Commands the guide prints **without** `sudo`:

> `port search php`
> `port info yubico-pam`
> `port deps apache2`
> `port variants apache2`
> `port installed`
> `port outdated`
> `port contents xorg-renderproto`
> `port dependents openssl`
> `port notes xinit`
> `port livecheck rb19-sass`

Asked explicitly whether section 3.1 ("The port Command") contains a general
sentence stating that some actions require root or administrator privileges:
**it does not**. Its introduction reads:

> **port** is the main utility used to interact with MacPorts. It is used to
> update `Portfile`s and the MacPorts infrastructure, and install and manage
> ports. Note that not all actions are listed here; see port(1) for the full
> list.

`select`, `activate` and `deactivate` are **not covered** in chapter 3.

### E12 — MacPorts guide, "Common Tasks" (read 2026-09-02, <https://guide.macports.org/chunked/using.common-tasks.html>)

With `sudo`:

> `$ sudo port selfupdate`
> `$ sudo port upgrade outdated`
> `$ sudo port upgrade makedepend`
> `$ sudo port uninstall inactive`
> `$ sudo port uninstall HexFiend @2.1.2_1`
> `$ sudo port uninstall leaves`
> `$ sudo port uninstall rleaves`
> `$ sudo port_cutleaves`
> `$ sudo port setrequested pkgconfig`
> `$ sudo port unsetrequested git-flow`

Without `sudo`:

> `$ port outdated`
> `$ port installed inactive`
> `$ port depends_on libksba`
> `$ port dependents libksba`
> `$ port rdependents libksba`
> `$ port echo leaves`
> `$ port echo requested`

### E13 — MacPorts `port(1)` (read 2026-09-02, <https://man.macports.org/port.1.html>)

Asked for every passage containing root / sudo / privilege / administrator. The
answer: **two**, both usage examples, no general statement.

> "To update you would normally do: `sudo port -d sync`"
> "To update you would typically do: `sudo port selfupdate`"

Action descriptions, verbatim:

> **install**: "Install and activate *portname*."
> **uninstall**: "Deactivate and uninstall portname."
> **activate**: "Activate the installed *portname*."
> **deactivate**: "Deactivate the installed *portname*."
> **select**: "For a given group, selects a version to be the default by creating appropriate symbolic links."
> **load**: "Provides a shortcut to using launchctl to load a port's daemon (as installed in /Library/LaunchDaemons)."
> **unload**: "A shortcut to launchctl, like load, but unloads the daemon."
> **setrequested**: "Mark portname as requested."
> **unsetrequested**: "Mark portname as unrequested."
> **installed**: "Show the installed version, variants and activation status for each *portname*."
> **echo**: "Writes to stdout the arguments passed to *port*."
> **info**: "Displays meta-information available for *portname*."
> **deps**: "Lists the other ports that are required to build and run portname."
> **dependents**: "Lists the installed ports that depend on the port *portname*."
> **contents**: "Lists the files installed by portname."
> **location**: "Print the install location of a given port."

Pseudo-portname:

> *requested*: "installed ports that were explicitly asked for"

### E14 — MacPorts `port-select(1)` (read 2026-09-02, <https://man.macports.org/port-select.1.html>)

The page contains no explicit privilege statement; its examples are:

> `$> sudo port select --set mysql $option`
> `$> sudo port select --set mysql none`

### E15 — MacPorts `macportsuser` (read 2026-09-02, <https://raw.githubusercontent.com/macports/macports-base/master/doc/macports.conf.in>)

Verbatim, from the shipped configuration template in the project's own
repository:

```
# Directory under which MacPorts should install ports. This must be
# where MacPorts itself is installed.
prefix              	@prefix_expanded@
```

```
# User to run operations as when MacPorts drops privileges.
#macportsuser        	@RUNUSR@
```

`macports.conf(5)` (<https://man.macports.org/macports.conf.5.html>, read
2026-09-02) documents `prefix` — "Sets the directory where ports are installed.
Any path may be used but those with spaces and/or non ascii characters should
be avoided, as this can break some ports. This key is often referred to as
*${prefix}*.", default `/opt/local` — but **does not document `macportsuser`**.

### E16 — MacPorts StartupItems (read 2026-09-02, <https://guide.macports.org/chunked/reference.startupitems.html>)

> "a port with a StartupItem places a link to a .plist file for the port's
> daemon within `/Library/LaunchDaemons/`"
>
> "LaunchDaemons must be loaded as root, and only one instance will run for the
> whole system. LaunchAgents are loaded as a normal user, and one instance per
> user can run."
>
> "MacPorts installs .plist files tagged as 'disabled' for the sake of
> security."

### E17 — MacPorts uninstallation (read 2026-09-02, <https://guide.macports.org/chunked/installing.macports.uninstalling.html>)

> `$ sudo port -f uninstall installed`

The page's procedure also covers deleting "the macports user and group created
during installation" and removing the files at `/opt/local`, both written as
administrative commands. (The individual removal commands were summarised
rather than returned verbatim by the fetch; the `port -f uninstall installed`
line above is the one quoted exactly.)

### E18 — Sources that could not be read

`https://trac.macports.org/wiki/FAQ` and `.../FAQ?format=txt` both returned
**HTTP 403** behind the Anubis anti-scraping challenge (attempted 2026-09-02,
error id `9e4edb5b6b850c41`). The MacPorts FAQ is therefore *not* cited here.
Anything that would have come only from trac is marked UNRESOLVED below.

## Findings

1. **Homebrew refuses to run as root at all**, with a small enumerated set of
   exemptions: CI/container environments, `brew as-console-user`,
   `brew setup-sandbox`, `brew services`, and `brew --prefix`. Everything else
   aborts with "Running Homebrew as root is extremely dangerous and no longer
   supported." (E5, confirmed as policy by E6.) For MacPortsBrew this is not a
   "should not" but a hard provider-side error: an executor that ever wraps a
   `brew` invocation in elevation will fail, not merely be impolite.

2. **On this machine the Homebrew prefix root `/usr/local` is `root:wheel` and
   not writable by the invoking user; every Homebrew-managed subdirectory under
   it is `pirado:admin` and writable** (E2). This is the state that makes
   Homebrew's normal operations unprivileged, and it is *not* the same claim as
   "the prefix is user-owned". `/usr/local` on Intel is a shared system
   directory — `aws-cli` and `microsoft` under it are `root:wheel` and were not
   put there by Homebrew (E2). The user-owned set is the specific list Homebrew
   creates.

3. **The consequence of finding 2 that the specification does not yet state:**
   a Homebrew operation that needs to create a *new top-level directory* under
   `/usr/local` cannot do so unprivileged on this machine. All of Homebrew's
   standard ones already exist (`bin sbin lib include share etc var opt
   Cellar Caskroom Frameworks Homebrew`, E2), so this is an edge case, not the
   normal path — but it is the reason "Homebrew never needs root on Intel" is
   the wrong sentence to write. On Apple Silicon the equivalent question is
   about `/opt/homebrew`, which is a Homebrew-only directory (E6) and therefore
   not shared in this way; **not observed here — no Apple Silicon machine.**

4. **Homebrew formula install/uninstall needs no elevation after installation**
   — cited (E7: "you don't need *sudo* after Homebrew's initial installation
   when you `brew install`") and consistent with the observed ownership (E2).
   This holds equally for a **forced source build**: the sandbox and the
   user-owned Cellar are the same either way, and E6 in fact gives the
   source-build case as the *reason* not to elevate. Source-vs-bottle changes
   duration and toolchain requirements, not privilege.

5. **Homebrew casks are the exception.** A cask whose artifact is a `pkg`, or
   whose `installer script:` / `uninstall script:` carries `sudo: true`, will
   prompt for an administrator password; Homebrew documents this as a
   per-artifact property, not a global one (E9). Elevation for a cask is
   therefore **not answerable statically from "provider = Homebrew"**; it is a
   property of the individual cask definition and belongs in runtime capability
   detection / plan-time inspection of the cask.

6. **Homebrew's `brew services` is the documented user-agent vs system-daemon
   split**: without `sudo` it operates on `~/Library/LaunchAgents` (started at
   login); with `sudo` on `/Library/LaunchDaemons` (started at boot) (E8). The
   observed directory ownership matches exactly — `~/Library/LaunchAgents` is
   `pirado:staff drwx------`, `/Library/LaunchDaemons` is `root:wheel` (E8).
   Note that this cuts both ways for *reading*: `brew services list` is shown
   with an optional `sudo` prefix too, so a complete inventory of system-level
   daemons is not obtainable unprivileged. Enumerating *user-level* agents is.

7. **Installing Homebrew itself requires elevation.** Not stated as such in one
   sentence by the docs, but it follows from two cited/observed facts together:
   `/usr/local` is `root:wheel` and not user-writable (E2), and the documented
   promise is only that you do not need sudo *after* the initial installation
   (E7). The `.pkg` installer path likewise runs as an installer package (E7).
   This is the strongest inference in the note and is labelled as such.

8. **MacPorts installs to a root-owned `/opt/local` and its installation
   requires an administrator account** — cited directly: "These steps need to be
   perfomed from an administrator account, for which "sudo" will ask the
   password upon installation" (E10), default prefix `/opt/local` (E10, E15).

9. **MacPorts' mutating actions are documented with `sudo` throughout**:
   `install`, `uninstall`, `upgrade`, `clean`, `rev-upgrade`, `selfupdate`,
   `sync`, `load`, `unload`, `setrequested`, `unsetrequested`, `select --set`
   (E11, E12, E13, E14, E17). **No MacPorts document found states a general
   rule** — the evidence is uniformly by example, and `port(1)` contains only
   two incidental `sudo` mentions (E13). That is weaker evidence than it looks
   and is why several cells below are marked *inferred*.

10. **MacPorts' read-only actions are documented without `sudo`**: `search`,
    `info`, `deps`, `depends_on`, `dependents`, `rdependents`, `contents`,
    `variants`, `installed` (including `installed inactive`), `outdated`,
    `notes`, `livecheck`, `echo leaves`, `echo requested` (E11, E12). Every
    query class the Foundation Pass needs is in this set.

11. **MacPorts does not "run unprivileged"; it runs as root and drops
    privileges.** The `macportsuser` key is described as "User to run operations
    as when MacPorts drops privileges" (E15) — the direction is root →
    unprivileged user for fetching and building, the opposite of Homebrew's
    refuse-to-be-root model. `macportsuser` is shipped **commented out** with a
    configure-time default (`@RUNUSR@`), and is **not documented in
    `macports.conf(5)`** (E15). The uninstall procedure confirms the installer
    creates that user and group (E17).

12. **MacPorts service load/unload needs root because of what it writes, and
    the guide says so about launchd rather than about `port`:** "LaunchDaemons
    must be loaded as root, and only one instance will run for the whole system.
    LaunchAgents are loaded as a normal user" (E16), with StartupItems linked
    into `/Library/LaunchDaemons/` (E16) and `sudo port load|unload` examples
    (E11).

13. **User intent vs inferred top-level position — both providers record intent,
    and reading it needs no elevation.** Homebrew's record of intent is
    `installed_on_request` in the per-keg `INSTALL_RECEIPT.json`, surfaced by
    `brew leaves --installed-on-request`; plain `brew leaves` is the *inferred*
    dependency-graph answer (E4). The receipt is mode `-rw-r--r--` and readable
    by the invoking user (E4). MacPorts' record of intent is the `requested`
    flag, surfaced by the `requested` pseudo-portname — "installed ports that
    were explicitly asked for" (E13) — read with `port echo requested`, no sudo
    (E12), and *set* with `sudo port setrequested` (E12). The privilege
    conclusion: **reading intent is unprivileged on both sides; only changing it
    elevates, and only on MacPorts.**

14. **Nothing the Foundation Pass performs requires elevation on either
    provider**, subject to the MacPorts caveats in "Cannot determine". Every
    Homebrew operation in the Foundation Pass set was executed here as uid 501
    with no prompt (E3, E4). Every MacPorts equivalent is documented without
    `sudo` (E10 finding, E11, E12, E13).

### The table

Elevation required to perform the operation. **Obs** = observed on this machine;
**Cit** = cited official documentation; **Inf** = inferred from cited/observed
facts, flagged as such; **RT** = must be answered by runtime capability
detection.

| Operation class | Homebrew | Evidence | MacPorts | Evidence |
|---|---|---|---|---|
| Discovery (prefix, version, config) | **No** | Obs E3 (`brew --prefix`, `brew --version`, `brew config` as uid 501) | **No** | Cit E13 (`port location`, `installed`, `info` documented without sudo); reading `${prefix}/etc/macports/macports.conf` **RT** (not observed — see Cannot determine) |
| Inventory (installed set, versions, intent) | **No** | Obs E4 (`brew list --versions`, `brew leaves`, `brew leaves --installed-on-request`, receipt mode `-rw-r--r--`) | **No** | Cit E11/E12 (`port installed`, `port echo requested` printed without sudo) |
| Mapping (equivalence, no provider write) | **No** | Obs E4 — read queries only | **No** | Cit E11/E13 — read queries only |
| Analysis (metadata, artifact availability, deprecation) | **No** | Obs E3/E4; Cit E7 | **No** | Cit E11 (`port info`, `port variants`, `port notes`, `port livecheck` without sudo) |
| Dependency / reverse-dependency queries | **No** | Obs E4 (`brew deps --installed`) | **No** | Cit E11/E12 (`port deps`, `dependents`, `rdependents`, `depends_on` without sudo) |
| Planning | **No** | No provider invocation beyond the above | **No** | No provider invocation beyond the above |
| Dry-run | **No** | Spec MUST; nothing above elevates | **No** | Spec MUST; nothing above elevates |
| Install — formula / port, default prefix | **No** | Cit E7 ("you don't need *sudo* after Homebrew's initial installation when you `brew install`"); Obs E2 (Cellar user-writable) | **Yes** | Cit E11 (`sudo port install …`), E12 (`sudo port upgrade …`) |
| Install — forced source build | **No** (unchanged) | Cit E6 (the source-build case is the *argument against* root); Obs E2 | **Yes** (unchanged) | Cit E11 — same command, same privilege |
| Install — cask / `pkg` artifact | **Depends on the cask** | Cit E9 (`sudo: true` on `installer script:`; unsandboxed vendor installers) — **RT** per cask | n/a | MacPorts has no cask analogue |
| Install — new top-level dir under prefix | **Yes** (edge case) | **Inf** from Obs E2 (`/usr/local` `root:wheel`, not user-writable). Not applicable to `/opt/homebrew` — **not observed** | **Yes** | Cit E10 |
| Uninstall — formula / port | **No** | Cit E7; Obs E2 | **Yes** | Cit E11/E12/E17 (`sudo port uninstall …`) |
| Uninstall — cask with `uninstall script:` | **Depends on the cask** | Cit E9 (`script:` … "request `sudo: true`") — **RT** | n/a | — |
| Activate / deactivate | n/a (no such state) — nearest analogue `brew link`/`unlink` writes into `/usr/local/bin`, `/usr/local/opt`: **No** | Obs E2 (both user-writable) | **Yes** — **Inf** | Cit E13 gives the semantics ("Activate the installed *portname*") but **no document found states the privilege**; inferred from root-owned `${prefix}` (E10) and from `install` being defined as "Install and activate" under `sudo` (E11, E13). See UNRESOLVED-2 |
| Primary-provider selection | **No** — Homebrew has no `select`; the mechanism is symlinking into the user-owned prefix | Obs E2. Mechanism comparison is R-6, not this note | **Yes** | Cit E14 (`sudo port select --set …`). Reading a selection (`--list`, `--show`) **not separately cited** — see UNRESOLVED-3 |
| Service — load/unload, user level | **No** | Cit E8 ("Otherwise, operate on `~/Library/LaunchAgents`"); Obs E8 (`~/Library/LaunchAgents` is `pirado:staff`) | **Not offered** — MacPorts StartupItems link into `/Library/LaunchDaemons/` | Cit E16 |
| Service — load/unload, system level | **Yes** | Cit E8 ("If `sudo` is passed, operate on `/Library/LaunchDaemons`"); Obs E8 (`root:wheel`) | **Yes** | Cit E16 ("LaunchDaemons must be loaded as root"), E11 (`sudo port load` / `sudo port unload`) |
| Service — *listing* running services | User-level: **No**. System-level: **Yes** | Cit E8 (`[sudo] brew services [list]`) | **RT** | Not cited; `port` has no documented list-services action — see UNRESOLVED-4 |
| Provider installation (bootstrapping the provider itself) | **Yes** — **Inf** | Obs E2 + Cit E7 ("*after* Homebrew's initial installation") | **Yes** | Cit E10 ("These steps need to be perfomed from an administrator account, for which "sudo" will ask the password upon installation") |
| Running the provider *as* root | **Refused** | Obs E5 ("Running Homebrew as root is extremely dangerous and no longer supported"), except `services`, `--prefix`, `as-console-user`, `setup-sandbox`, CI/containers | **Expected** — MacPorts runs as root and drops to `macportsuser` | Cit E15 ("User to run operations as when MacPorts drops privileges") |

## Cannot determine

- **UNRESOLVED-1 — every MacPorts row is citation-only.** MacPorts is not
  installed on this machine, so no MacPorts claim here is observed. In
  particular, whether `${prefix}/var/macports/registry/registry.db` and
  `${prefix}/etc/macports/macports.conf` are readable by a non-root user is
  **not established**. `port installed` being documented without `sudo` (E11)
  implies the registry is readable through `port`, but says nothing about
  MacPortsBrew reading the SQLite file directly. *What would settle it:*
  `ls -ld` / `stat -f '%Su %Sg %Sp'` on `/opt/local`,
  `/opt/local/var/macports/registry/registry.db` and
  `/opt/local/etc/macports/macports.conf`, plus `port installed` as an
  unprivileged user, on a machine with MacPorts installed. Until then the
  adapter must treat "can I read the registry unprivileged" as runtime
  capability detection and degrade to `port` invocations, not assume either.

- **UNRESOLVED-2 — no official statement that `port activate` / `port deactivate`
  require root.** Neither `port(1)` (E13) nor the guide's chapter 3 (E11)
  documents the privilege, and the guide does not cover those actions at all.
  The table's "Yes" is an inference from prefix ownership. *What would settle
  it:* a statement in a MacPorts document, or an observation of
  `port deactivate` failing as a non-root user on a real install. The trac FAQ,
  which might contain the general rule, was unreachable (E18).

- **UNRESOLVED-3 — reading a MacPorts `select` group unprivileged.**
  `port select --list` / `--show` privilege is not documented; only `--set` is
  shown with `sudo` (E14). Relevant because primary-provider *detection* must
  be an unprivileged read. *What would settle it:* running
  `port select --list <group>` unprivileged on a real install, or a doc
  statement.

- **UNRESOLVED-4 — enumerating MacPorts services unprivileged.** MacPorts has no
  documented "list loaded startup items" action; the state lives in
  `/Library/LaunchDaemons` and in launchd. Whether `launchctl print`-class reads
  of system daemons work unprivileged on macOS 26 was not tested here. Feeds R-8
  more than R-5. *What would settle it:* an Apple-documented statement on
  `launchctl` read privileges plus an observation.

- **Apple Silicon / `/opt/homebrew` not observed.** Findings 2, 3 and 7 are
  Intel/`/usr/local` observations. The `/opt/homebrew` ownership claim rests on
  E6 (docs) only. *What would settle it:* the same `ls -ld` / `stat` battery on
  an Apple Silicon machine.

- **The MacPorts trac FAQ is not cited** (E18, HTTP 403 behind Anubis).
  Anything the FAQ says about non-root MacPorts is therefore absent from this
  note. *What would settle it:* a fetch that clears the challenge, or the
  equivalent statement located in `guide.macports.org` or `macports-base`.

- **Whether the MacPorts guide documents a non-root MacPorts installation:**
  **no such documentation was found.** `installing.macports.html` documents
  `--prefix` only for the case of multiple MacPorts instances ("Only one copy of
  MacPorts may use the default prefix `/opt/local`, so for additional
  installations use the option `--prefix`") and does not describe a user-owned
  or non-root install. Recorded as a negative result of a targeted search, not
  as proof that none exists.

- **Deep source verification of which `port` actions call `elevateToRoot` was
  attempted and abandoned.** Fetches of `macports-base` `src/port/port.tcl` and
  `src/macports1.0/macports.tcl` returned truncated content, and a claim built
  on a truncated read would be indistinguishable from a guess. No source-level
  MacPorts claim is made in this note. *What would settle it:* a local clone of
  `macports-base` and a grep for `elevateToRoot`.

## Impact on the specification

- **`01-invariants.md` INV-06 — confirmed.** "Discovery, inventory, mapping,
  analysis, planning and dry-run MUST work with no elevation at all" is
  satisfied by both providers: observed for Homebrew (E3, E4), cited for
  MacPorts (E11, E12, E13). No change needed.

- **`04-migration-and-safety.md`, "Privilege" — confirmed in substance, but the
  framing understates two cases.** "The design assumes MacPorts operations
  generally require elevation and Homebrew's normally do not" holds. Two
  refinements the spec does not currently carry, offered for
  `docs/spec-evolution.md` to accept or reject:

  1. **Homebrew casks break the "Homebrew never elevates" heuristic** (finding
     5, E9). An adapter that reports `elevation: none` for every Homebrew
     operation will under-report a cask `pkg` install, and the plan is required
     to list "exactly which steps elevate and why". Elevation for a Homebrew
     cask is a per-cask property and must be asked of the adapter per
     operation — which is already what the spec's "it asks the adapter" clause
     requires, but the clause reads as though the answer varies by *provider*,
     not by *package*.

  2. **`brew services` at system level elevates, and so does *reading* it**
     (finding 6, E8). A services inventory that runs unprivileged is
     necessarily incomplete for system daemons. That is an
     `analyze`-must-work-offline-and-unprivileged constraint with a visible
     consequence for the user, and INV-19-adjacent (refusing to migrate service
     packages) is where it will surface.

- **`04-migration-and-safety.md`, "Process execution"** — the quiet-environment
  requirement is exercised here: `HOMEBREW_NO_AUTO_UPDATE`,
  `HOMEBREW_NO_ANALYTICS`, `HOMEBREW_NO_ENV_HINTS` (E0), confirmed set in the
  process environment and echoed back by `brew config`. No MacPorts equivalent
  was determined (no install); note that `port sync` and `port selfupdate` are
  the explicit-update actions (E13) and neither is implied by a read action in
  any documentation found — but that is a *negative* finding, not a verified
  one, and belongs to R-1's MacPorts half.

- **`10-first-pass-work-order.md`** — nothing in the Foundation Pass set needs
  an elevating code path (finding 14). The stated check ("`grep` finds no
  install/uninstall/sudo/shell-write code path in the binary") is compatible
  with everything observed.

- **No contradiction of any Tier 1 or Tier 2 statement was found.**

## Re-check when

- **Homebrew changes its root-refusal exemption list.** E5 is source at a
  specific revision (`560147012b9678b42ef5e83b690f0895552d1366`); the exemptions
  for `as-console-user`, `setup-sandbox` and `services` are recent additions and
  the list is the kind of thing that grows. Re-read `brew.sh`
  `check-run-command-as-root` on any Homebrew major-version bump.
- **MacPorts is installed on a test machine.** That converts UNRESOLVED-1, -2
  and -3 from citation-and-inference to observation, and it is the single
  highest-value follow-up in this note.
- **An Apple Silicon machine becomes available.** `/opt/homebrew` ownership and
  the "new top-level directory" edge case (finding 3) are unobserved.
- **`brew services` moves out of the exemption list, or its sudo semantics
  change.** E8 is the load-bearing citation for the agent/daemon split.
- **macOS changes `/usr/local` handling or SIP coverage.** Finding 2 is an
  observation of one machine's history as much as of policy: `/usr/local` was
  `root:wheel` with Homebrew-created user-owned subdirectories. A different
  machine, or a future installer, may differ — which is exactly why the adapter
  should test writability at runtime rather than hard-code this note's result.
- **Time-sensitivity:** none of the facts here has a published expiry date. The
  Homebrew documentation pages carry `last_review_date: "2026-08-27"` (E7); the
  MacPorts guide carries no review date, which makes its citations the ones more
  likely to have gone stale silently.
