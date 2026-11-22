# R-31 — Which startup file does each shell read on macOS, where must a `PATH` block sit to survive `path_helper`, and which names resolve without `PATH`?

- **Status:** partially answered. The startup order of both shells, `ZDOTDIR`, `path_helper`'s effect, both installers' writes and both builtin lists are answered from observation, the shells' own manuals and the providers' own source. Terminal.app's login-shell invocation is UNRESOLVED (cited only as far as Apple's guide goes; not observed). See Cannot determine.
- **Answered on:** 2026-11-22
- **Machine:** macOS 26.7.3 (25G342), x86_64 (E-1). `/bin/zsh` 5.9, `/bin/bash` 3.2.57 (E-1). Homebrew @ `/usr/local` and MacPorts @ `/opt/local` as recorded by R-28/R-29; their versions were **not re-read**, because no `brew` or `port` command was run for this note. No provider command ran, so no auto-update was possible and no quieting environment was needed or used.
- **Blocking:** work order 16 part I, D5 (`SHIM_NAME_IS_SHELL_BUILTIN`) and D6 (the file the managed `PATH` block goes in, `SHELL_FILE_WOULD_SHADOW`, the `path_helper` prediction) — `docs/passes/16-plans/I.md`, "D5" and "D6"; spec `docs/spec/05-environment-and-primary-provider.md`, "Shell configuration".

## Question

Part I writes one managed block that prepends a shim directory to `PATH`, for zsh and bash. It needs to know:

1. Which files zsh reads for login, interactive, and login+interactive shells, in which order, and how `ZDOTDIR` (including one set in `~/.zshenv`) moves the user files.
2. Bash's login rule (first of `~/.bash_profile`, `~/.bash_login`, `~/.profile`), its interactive non-login rule (`~/.bashrc`), what macOS's `/bin/bash` 3.2 and `/etc/profile`, `/etc/bashrc` add.
3. Whether Terminal.app and iTerm2 open login shells, and what `$SHELL` versus the account's login shell says.
4. What `path_helper` does to an existing `PATH`, which system files run it, and so where a user block must sit to be first after it.
5. What each provider's installer writes into these files.
6. This machine's state, read-only.
7. Each shell's builtins and reserved words, as dated data.

## Evidence

All commands were run on 2026-11-22 as the unelevated user, from the Claude Code `Bash` tool (a zsh). **No user startup file was sourced, printed or modified.** Startup order was observed by running the system shells with `env -i` and a `HOME` or `ZDOTDIR` pointing into the session scratchpad, which held trace files written for this note (each file only `echo`es its own name and `PATH`). With `HOME`/`ZDOTDIR` redirected, the shells read Apple's system files (`/etc/zprofile`, `/etc/zshrc`, `/etc/profile`, `/etc/bashrc`) and the trace files, never the user's. User files were tested for existence and with `grep -qs` (yes/no only). In the outputs below the cryptex `PATH` entries are abbreviated `<cryptex>` where marked, and the full middle of `path_helper`'s list as `<system paths>` where marked; nothing else is altered.

### E-1 · Machine, shells, account shell

```
$ sw_vers; uname -m
ProductName:		macOS
ProductVersion:		26.7.3
BuildVersion:		25G342
x86_64
$ echo "SHELL=$SHELL"; echo "ZDOTDIR=${ZDOTDIR-unset}"
SHELL=/bin/zsh
ZDOTDIR=unset
$ dscl . -read /Users/$USER UserShell
UserShell: /bin/zsh
$ /bin/zsh --version
zsh 5.9 (x86_64-apple-darwin25.0)
$ /bin/bash --version | head -1
GNU bash, version 3.2.57(1)-release (x86_64-apple-darwin25)
$ which -a zsh bash
/bin/zsh
/bin/bash
```

### E-2 · User startup files on this machine (existence and yes/no greps only)

```
$ cd "$HOME"; for f in .zshenv .zprofile .zshrc .zlogin .zlogout .bash_profile .bash_login .profile .bashrc .bash_logout; do
    if test -e "$f"; then s=present; test -L "$f" && s=present-symlink; b=no; o=no
      grep -qs 'brew shellenv' "$f" && b=yes; grep -qs '/opt/local/bin' "$f" && o=yes
      echo "~/$f $s brew-shellenv=$b opt-local-bin=$o"; else echo "~/$f absent"; fi; done
~/.zshenv present brew-shellenv=no opt-local-bin=no
~/.zprofile present brew-shellenv=yes opt-local-bin=no
~/.zshrc present brew-shellenv=no opt-local-bin=no
~/.zlogin absent
~/.zlogout absent
~/.bash_profile absent
~/.bash_login absent
~/.profile present brew-shellenv=no opt-local-bin=no
~/.bashrc present brew-shellenv=no opt-local-bin=no
~/.bash_logout absent
```

A second pass over the five present files, same form, for `/opt/local`, the word `MacPorts` and the word `ZDOTDIR`: `no` for every file and every pattern. Yet:

```
$ echo "PATH has /opt/local/bin: $(case :$PATH: in *:/opt/local/bin:*) echo yes;; *) echo no;; esac)"
PATH has /opt/local/bin: yes
```

### E-3 · Apple's system startup files

```
$ for f in /etc/zshenv /etc/zprofile /etc/zshrc /etc/zlogin /etc/zlogout /etc/profile /etc/bashrc /etc/paths /etc/manpaths; do test -e $f && ls -l $f || echo "$f absent"; done
/etc/zshenv absent
-r--r--r--  1 root  wheel  304 Nov  6 02:12 /etc/zprofile
-r--r--r--  1 root  wheel  3191 Nov  6 02:12 /etc/zshrc
/etc/zlogin absent
/etc/zlogout absent
-r--r--r--  1 root  wheel  189 Nov  6 02:12 /etc/profile
-r--r--r--  1 root  wheel  265 Nov  6 02:12 /etc/bashrc
-rw-r--r--  1 root  wheel  75 Nov  6 02:12 /etc/paths
-rw-r--r--  1 root  wheel  36 Nov  6 02:12 /etc/manpaths
```

```
== /etc/zprofile
# System-wide profile for interactive zsh(1) login shells.

# Setup user specific overrides for this in ~/.zprofile. See zshbuiltins(1)
# and zshoptions(1) for more details.

if [ -z "$LANG" ]; then
	export LANG=C.UTF-8
fi

if [ -x /usr/libexec/path_helper ]; then
	eval `/usr/libexec/path_helper -s`
fi
== /etc/profile
# System-wide .profile for sh(1)

if [ -x /usr/libexec/path_helper ]; then
	eval `/usr/libexec/path_helper -s`
fi

if [ "${BASH-no}" != "no" ]; then
	[ -r /etc/bashrc ] && . /etc/bashrc
fi
== /etc/bashrc
# System-wide .bashrc file for interactive bash(1) shells.
if [ -z "$PS1" ]; then
   return
fi

PS1='\h:\W \u\$ '
# Make bash check its window size after a process completes
shopt -s checkwinsize

[ -r "/etc/bashrc_$TERM_PROGRAM" ] && . "/etc/bashrc_$TERM_PROGRAM"
```

`/etc/zshrc`, filtered (`grep -nE 'source|\. |path_helper|ZDOTDIR|zshrc_|TERM_PROGRAM'`): no `path_helper`; it uses `${ZDOTDIR:-$HOME}` for `HISTFILE` and `.zkbd`, and line 74 sources `/etc/zshrc_$TERM_PROGRAM` (`/etc/zshrc_Apple_Terminal` and `/etc/bashrc_Apple_Terminal` exist).

`/etc/paths`: `/usr/local/bin`, `/System/Cryptexes/App/usr/bin`, `/usr/bin`, `/bin`, `/usr/sbin`, `/sbin`. `/etc/paths.d`: `10-cryptex`, `10-pmk-global`, `100-rvictl`, `40-XQuartz`, `dotnet`, `dotnet-cli-tools`; none names `/opt/local` or a Homebrew prefix.

### E-4 · zsh's manual: STARTUP/SHUTDOWN FILES (local, as shipped)

`man zsh` (`/usr/share/man/man1/zsh.1`, zsh 5.9), read 2026-11-22:

> Commands are first read from /etc/zshenv; this cannot be overridden. […]
> Commands are then read from $ZDOTDIR/.zshenv. If the shell is a login shell, commands are read from /etc/zprofile and then $ZDOTDIR/.zprofile. Then, if the shell is interactive, commands are read from /etc/zshrc and then $ZDOTDIR/.zshrc. Finally, if the shell is a login shell, /etc/zlogin and $ZDOTDIR/.zlogin are read.

> If ZDOTDIR is unset, HOME is used instead.

### E-5 · zsh observed: the order per mode, and `path_helper`'s effect

Trace files `.zshenv .zprofile .zshrc .zlogin` in scratch dir `zd1`, each `echo "read zd1/.<name> PATH=$PATH"`:

```
$ for m in "-l -c" "-i -c" "-l -i -c"; do echo "### zsh $m"; env -i HOME=$S/zd1 ZDOTDIR=$S/zd1 TERM=dumb PATH=/SHIM:/usr/bin:/bin /bin/zsh ${=m} 'echo "final PATH=$PATH"' </dev/null; done
### zsh -l -c
read zd1/.zshenv PATH=/SHIM:/usr/bin:/bin
read zd1/.zprofile PATH=/usr/local/bin:/System/Cryptexes/App/usr/bin:/usr/bin:/bin:/usr/sbin:/sbin:<cryptex>/usr/local/bin:<cryptex>/usr/bin:<cryptex>/usr/appleinternal/bin:/pkg/env/global/bin:/opt/X11/bin:/Library/Apple/usr/bin:/usr/local/share/dotnet:~/.dotnet/tools:/SHIM
read zd1/.zlogin PATH=<same as above>
final PATH=<same as above>
### zsh -i -c
read zd1/.zshenv PATH=/SHIM:/usr/bin:/bin
read zd1/.zshrc PATH=/SHIM:/usr/bin:/bin
final PATH=/SHIM:/usr/bin:/bin
### zsh -l -i -c
read zd1/.zshenv PATH=/SHIM:/usr/bin:/bin
read zd1/.zprofile PATH=/usr/local/bin:<system paths>:~/.dotnet/tools:/SHIM
read zd1/.zshrc PATH=/usr/local/bin:<system paths>:~/.dotnet/tools:/SHIM
read zd1/.zlogin PATH=/usr/local/bin:<system paths>:~/.dotnet/tools:/SHIM
final PATH=/usr/local/bin:<system paths>:~/.dotnet/tools:/SHIM
```

`ZDOTDIR` in the environment, `HOME` elsewhere (`h3`, whose own trace files exist):

```
$ env -i HOME=$S/h3 ZDOTDIR=$S/zd1 TERM=dumb PATH=/usr/bin:/bin /bin/zsh -l -i -c 'echo done' </dev/null | sed -E 's/ PATH=.*//'
read zd1/.zshenv
read zd1/.zprofile
read zd1/.zshrc
read zd1/.zlogin
done
```

### E-6 · zsh observed: `ZDOTDIR` set (not exported) in `~/.zshenv`, and D6's guarded block under `path_helper`

`h3/.zshenv` is `echo "read h3/.zshenv"` then `ZDOTDIR=$S/zd2` (no `export`). `h3/.zprofile` and `h3/.zshrc` are trace files. `zd2/.zshenv` and `zd2/.zshrc` are trace files; `zd2/.zprofile` is:

```
echo "read zd2/.zprofile"
case ":$PATH:" in *':/SHIM:'*) ;; *) export PATH='/SHIM'":$PATH" ;; esac
echo "after block PATH=$PATH"
```

(the middle line is D6's block form, `I.md` "D6", with `<dir>` = `/SHIM`).

```
### ZDOTDIR set in HOME/.zshenv, login+interactive, fresh PATH
$ env -i HOME=$S/h3 TERM=dumb PATH=/usr/bin:/bin /bin/zsh -l -i -c 'echo final=$PATH' </dev/null
read h3/.zshenv
read zd2/.zprofile
after block PATH=/SHIM:/usr/local/bin:<system paths>:~/.dotnet/tools
read zd2/.zshrc
final=/SHIM:/usr/local/bin:<system paths>:~/.dotnet/tools

### nested login shell, inherited PATH already has /SHIM first
$ env -i HOME=$S/h3 TERM=dumb PATH=/SHIM:/usr/bin:/bin /bin/zsh -l -i -c 'echo final=$PATH' </dev/null
read h3/.zshenv
read zd2/.zprofile
after block PATH=/usr/local/bin:<system paths>:~/.dotnet/tools:/SHIM
read zd2/.zshrc
final=/usr/local/bin:<system paths>:~/.dotnet/tools:/SHIM
```

```
$ env -i HOME=$S/h3 TERM=dumb PATH=/usr/bin:/bin /bin/zsh -l -i -c 'echo "shell var ZDOTDIR=${ZDOTDIR:+set}"; /usr/bin/env | /usr/bin/grep -c "^ZDOTDIR="' </dev/null | grep -v '^read\|^after'
shell var ZDOTDIR=set
0
```

### E-7 · bash's manual: INVOCATION (local, as shipped)

`man bash` (`/usr/share/man/man1/bash.1`, bash 3.2.57), read 2026-11-22:

> A login shell is one whose first character of argument zero is a -, or one started with the --login option.

> When bash is invoked as an interactive login shell, or as a non-interactive shell with the --login option, it first reads and executes commands from the file /etc/profile, if that file exists. After reading that file, it looks for ~/.bash_profile, ~/.bash_login, and ~/.profile, in that order, and reads and executes commands from the first one that exists and is readable.

> When an interactive shell that is not a login shell is started, bash reads and executes commands from ~/.bashrc, if that file exists.

> If bash is invoked with the name sh, it tries to mimic the startup behavior of historical versions of sh […] /etc/profile and ~/.profile, in that order.

### E-8 · bash observed: the first-of-three rule, `.bashrc`, `/etc/bashrc`

Scratch homes: `b1` = `.profile` + `.bashrc`; `b2` = `.profile` + `.bash_login`; `b3` = all of `.bash_profile`, `.bash_login`, `.profile`, `.bashrc`; `b4` = empty. Each file `echo "read <name>"`.

```
$ for d in b1 b2 b3 b4; do for m in "-l -c" "-l -i -c" "-i -c"; do echo "### $d bash $m"; env -i HOME=$S/$d TERM=dumb PATH=/SHIM:/usr/bin:/bin /bin/bash ${=m} 'echo "PS1=[$PS1] PATH=$PATH"' </dev/null; done; done   (bash: no job control lines removed)
### b1 bash -l -c
read .profile
PS1=[] PATH=/usr/local/bin:<system paths>:~/.dotnet/tools:/SHIM
### b1 bash -l -i -c
read .profile
PS1=[\h:\W \u\$ ] PATH=/usr/local/bin:<system paths>:~/.dotnet/tools:/SHIM
### b1 bash -i -c
read .bashrc
PS1=[\s-\v\$ ] PATH=/SHIM:/usr/bin:/bin
### b2 bash -l -c
read .bash_login
PS1=[] PATH=/usr/local/bin:<system paths>:~/.dotnet/tools:/SHIM
### b2 bash -l -i -c
read .bash_login
PS1=[\h:\W \u\$ ] PATH=/usr/local/bin:<system paths>:~/.dotnet/tools:/SHIM
### b2 bash -i -c
PS1=[\s-\v\$ ] PATH=/SHIM:/usr/bin:/bin
### b3 bash -l -c
read .bash_profile
PS1=[] PATH=...:/SHIM
### b3 bash -l -i -c
read .bash_profile
PS1=[\h:\W \u\$ ] PATH=...:/SHIM
### b3 bash -i -c
read .bashrc
PS1=[\s-\v\$ ] PATH=/SHIM:/usr/bin:/bin
### b4 bash -l -c
PS1=[] PATH=...:/SHIM
### b4 bash -l -i -c
PS1=[\h:\W \u\$ ] PATH=...:/SHIM
### b4 bash -i -c
PS1=[\s-\v\$ ] PATH=/SHIM:/usr/bin:/bin
```

(`\h:\W \u\$ ` is the `PS1` `/etc/bashrc` sets, E-3; `\s-\v\$ ` is bash's compiled-in default.)

### E-9 · `path_helper`: its manual and its observed effect on an existing `PATH`

`man path_helper` (`/usr/share/man/man8/path_helper.8`, dated "March 15, 2007"), read 2026-11-22:

> The path_helper utility reads the contents of the files in the directories /etc/paths.d and /etc/manpaths.d and appends their contents to the PATH and MANPATH environment variables respectively. (The MANPATH environment variable will not be modified unless it is already set in the environment.)

> Prior to reading these directories, default PATH and MANPATH values are obtained from the files /etc/paths and /etc/manpaths respectively.

> The path_helper utility should not be invoked directly. It is intended only for use by the shell profile.

The manual says nothing about what happens to entries already in `PATH`. Observed (it only prints; `-s` emits Bourne syntax):

```
$ env -i PATH=/Users/example/.local/macportsbrew/shims:/usr/bin:/opt/local/bin /usr/libexec/path_helper -s; echo "rc=$?"
PATH="/usr/local/bin:/System/Cryptexes/App/usr/bin:/usr/bin:/bin:/usr/sbin:/sbin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/local/bin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/bin:/var/run/com.apple.security.cryptexd/codex.system/bootstrap/usr/appleinternal/bin:/pkg/env/global/bin:/opt/X11/bin:/Library/Apple/usr/bin:/usr/local/share/dotnet:~/.dotnet/tools:/Users/example/.local/macportsbrew/shims:/opt/local/bin"; export PATH;
rc=0

$ env -i PATH=/a:/usr/local/bin:/b MANPATH=/m /usr/libexec/path_helper -s
PATH="/usr/local/bin:<system paths>:~/.dotnet/tools:/a:/b"; export PATH;
MANPATH="/usr/share/man:/usr/local/share/man:/opt/X11/share/man:/m:"; export MANPATH;

$ env -i PATH= /usr/libexec/path_helper -s        (and with PATH unset: identical)
PATH="/usr/local/bin:<system paths>:~/.dotnet/tools"; export PATH;
```

### E-10 · Terminal emulators

Apple, *Terminal User Guide*, "Change General settings in Terminal on Mac", <https://support.apple.com/guide/terminal/trmlstrtup/mac>, read 2026-11-22:

> **Shells open with:** "Choose the shell that's used when a new window opens."
> **Default login shell:** "The login shell set up for your user account. The default login shell is zsh."
> **Command (complete path):** "To use a different shell, enter the path to the shell."

(<https://support.apple.com/guide/terminal/change-general-settings-trml107/mac> and <https://support.apple.com/guide/terminal/trml113/mac>, read the same day, say nothing further about login-mode invocation.) Apple's guide is Apple documentation but not developer documentation; it states *which* shell, not *whether it is started as a login shell*.

iTerm2, "Profiles > General", <https://iterm2.com/documentation-preferences-profiles-general.html>, read 2026-11-22 (the project's own documentation; a third-party application, so not among this project's listed official sources for providers, but the authoritative source for iTerm2):

> "If _login shell_ is chosen, then `login` is invoked."

and, for a custom shell path, the page says it is run as a login shell (the relay would not quote that sentence verbatim; re-read the page to quote it).

`man login` (`/usr/share/man/man1/login.1`), read 2026-11-22, option `-l`:

> Tells the program executed by login that this is not a login session (by convention, a login session is signalled to the program with a hyphen as the first character of argv[0]; this option disables that), […] The default is to add the hyphen (this is a login session).

Observed on this machine:

```
$ ps -axo args= | awk '{print $1}' | grep -E '^-' | sort | uniq -c
(no output)
$ ps -axo args= | awk '{n=$1; sub(".*/","",n); print n}' | grep -xE 'Terminal|iTerm2|login' | sort | uniq -c
(no output)
$ ps -ax | wc -l
    1282
$ defaults read com.apple.Terminal Shell
The domain/default pair of (com.apple.Terminal, Shell) does not exist
$ defaults read com.googlecode.iterm2 'New Bookmarks' | grep -E '"(Custom Command|Command|Name|Guid)"' | head
        "Custom Command" = No;
$ ls -d /Applications/iTerm.app /System/Applications/Utilities/Terminal.app
/Applications/iTerm.app
/System/Applications/Utilities/Terminal.app
```

Neither emulator was running, so no login shell of theirs could be observed.

### E-11 · What each provider's installer writes

**Homebrew installer**, `install.sh` in `Homebrew/install`, fetched with `gh api repos/Homebrew/install/contents/install.sh` on 2026-11-22, repository head `c9c975046f78e6e6d662deb801bd461ebd6a54ed` (2026-11-20). Lines 1137–1180:

```
ohai "Next steps:"
case "${SHELL}" in
  */bash*)
    shellenv_suffix=" bash"
    if [[ -n "${HOMEBREW_ON_LINUX-}" ]]
    then
      shell_rcfile="${HOME}/.bashrc"
    else
      shell_rcfile="${HOME}/.bash_profile"
    fi
    ;;
  */zsh*)
    shellenv_suffix=" zsh"
    if [[ -n "${HOMEBREW_ON_LINUX-}" ]]
    then
      shell_rcfile="${ZDOTDIR:-"${HOME}"}/.zshrc"
    else
      shell_rcfile="${ZDOTDIR:-"${HOME}"}/.zprofile"
    fi
    ;;
  ...
if grep -qs "eval \"\$(${HOMEBREW_PREFIX}/bin/brew shellenv[^\"]*)\"" "${shell_rcfile}"
...
    echo >> ${shell_rcfile}
    echo 'eval "\$(${HOMEBREW_PREFIX}/bin/brew shellenv${shellenv_suffix})"' >> ${shell_rcfile}
```

These `echo … >>` lines sit inside the printed "Next steps" text: the script prints them for the user to run; it does not run them itself. Lines 599 and 1083–1091: when `HOMEBREW_PREFIX` is `/opt/homebrew` and sudo is available, it installs `/etc/paths.d/homebrew` containing `${HOMEBREW_PREFIX}/bin` (root:wheel, 0644).

**Homebrew `shellenv`**, `Library/Homebrew/cmd/shellenv.sh` in `Homebrew/brew`, fetched with `gh api` on 2026-11-22, repository head `62a9ad2642e0cf2fe1104d955069c78b7d990cd2` (2026-11-21). Lines 9–13 and 66:

```
homebrew-shellenv() {
  if [[ "${HOMEBREW_PATH}:" == "${HOMEBREW_PREFIX}/bin:${HOMEBREW_PREFIX}/sbin:"* ]]
  then
    return
  fi
...
      echo "export PATH=\"${HOMEBREW_PREFIX}/bin:${HOMEBREW_PREFIX}/sbin\${PATH+:\$PATH}\";"
```

**MacPorts installer postflight**, `portmgr/dmg/postflight.in` in `macports/macports-base`, fetched with `gh api` on 2026-11-22, last commit touching the file `49767c8e7aa9eb88f268ab89c86f42d3856a387d` (2026-07-01). Lines 52–53, 112–126, 256–311:

```
OUR_BASESTRING="MacPorts Installer addition"
OUR_STRING="${OUR_BASESTRING} on ${TIMESTAMP}"
...
function write_setting () {
    if [[ -f "${HOME}/.${CONF_FILE}" ]] && ! grep "${OUR_BASESTRING}" "${HOME}/.${CONF_FILE}" > /dev/null; then
        ... /bin/cp -fp "${HOME}/.${CONF_FILE}" "${HOME}/.${CONF_FILE}.${BACKUP_SUFFIX}" ...
    fi
    {
        echo -e "\n# ${OUR_STRING}: adding an appropriate ${1} variable for use with MacPorts."
        echo "${ENV_COMMAND} ${1}${ASSIGN}${2}"
        echo -e "# Finished adapting your ${1} environment variable for use with MacPorts.\n"
    } >> "${HOME}/.${CONF_FILE}"
...
USHELL=$(${DSCL} . -read "/Users/${USER}" UserShell) || {
...
    */bash)
        ...
        if [[ -f "${HOME}/.bash_profile" ]]; then
            CONF_FILE=bash_profile
        elif [[ -f "${HOME}/.bash_login" ]]; then
            CONF_FILE=bash_login
        else
            CONF_FILE=profile
        fi
        ;;
    */zsh)
        ...
        CONF_FILE="zprofile"
        ;;
...
if /usr/bin/su "${USER}" -l -c "/usr/bin/printenv PATH" | tail -n 1 | tr ":" "\n" | grep "^${BINPATH}$" > /dev/null; then
    echo "Your shell already has the right PATH environment variable for use with MacPorts!"
else
    write_setting PATH "\"${BINPATH}:${SBINPATH}:\$PATH\""
fi
```

R-6 E12 (`guide.macports.org/chunked/installing.shell.html`, read 2026-09-02) says the postflight "may modify `.zprofile`, `.profile`, `.bash_login`, `.bash_profile`, `.tcshrc`, or `.cshrc`, depending on which ones exist".

### E-12 · zsh builtins and reserved words

```
$ env -i PATH=/usr/bin:/bin /bin/zsh -f -c 'print -l ${(ko)builtins}' | wc -l
102
$ env -i PATH=/usr/bin:/bin /bin/zsh -f -c 'print -l ${(ko)reswords}' | wc -l
31
$ env -i PATH=/usr/bin:/bin /bin/zsh -f -c 'zmodload'
zsh/main
$ env -i PATH=/usr/bin:/bin /bin/zsh -f -c 'print -l $module_path'
/usr/lib/zsh/5.9
```

The lists are in `data/policy/shell-builtins.toml`, verbatim. In an interactive login zsh with an empty scratch `ZDOTDIR` (Apple's `/etc/zprofile` and `/etc/zshrc` read), `${(k)builtins}` had 101 entries: the same set less `log`. After `zmodload` of each of the 34 loadable modules under `/usr/lib/zsh/5.9` in a throwaway `zsh -f`, `${(k)builtins}` had 147 entries; the 45 added are the file's `module_builtins` (among them `mv`, `rm`, `ln`, `mkdir`, `chmod`, `chown`, `stat`, `sync` from `zsh/files` and `zsh/stat`).

`man zshmisc` (`/usr/share/man/man1/zshmisc.1`), RESERVED WORDS, read 2026-11-22:

> do done esac then elif else fi for case if while function repeat time until select coproc nocorrect foreach end ! [[ { } declare export float integer local readonly typeset

That is the same 31 words as `${(k)reswords}`.

### E-13 · bash builtins and reserved words

```
$ env -i PATH=/usr/bin:/bin /bin/bash --norc --noprofile -c 'compgen -b' | LC_ALL=C sort | wc -l
58
$ env -i PATH=/usr/bin:/bin /bin/bash --norc --noprofile -c 'compgen -k' | LC_ALL=C sort | wc -l
21
```

`man bash`, RESERVED WORDS, read 2026-11-22:

> ! case do done elif else esac fi for function if in select then until while { } time [[ ]]

That is the same 21 words as `compgen -k`.

## Findings

1. **zsh reads every file of its mode, independently, in a fixed order** (E-4, E-5): always `/etc/zshenv` then `$ZDOTDIR/.zshenv`; login adds `/etc/zprofile` then `$ZDOTDIR/.zprofile`; interactive adds `/etc/zshrc` then `$ZDOTDIR/.zshrc`; login adds `/etc/zlogin` then `$ZDOTDIR/.zlogin` last. Login+interactive: zshenv, zprofile, zshrc, zlogin. Login only: zshenv, zprofile, zlogin. Interactive only: zshenv, zshrc. **No zsh file's existence changes whether another zsh file is read**, so creating `~/.zprofile` (or `$ZDOTDIR/.zprofile`) shadows nothing. On this macOS `/etc/zshenv` and `/etc/zlogin` do not exist (E-3).
2. **`ZDOTDIR` replaces `HOME` for all four user files** (E-4, E-5). Set in the environment, `$HOME`'s files are never read. Set in `~/.zshenv`, that one file is read from `$HOME` and every later user file (`.zprofile`, `.zshrc`, `.zlogin`) from the new `ZDOTDIR`; the new `ZDOTDIR`'s `.zshenv` is not read (E-6). **A `ZDOTDIR` set in `~/.zshenv` without `export` does not reach child processes** (E-6), so MacPortsBrew, started from that shell, sees no `ZDOTDIR` and would pick `~/.zprofile`, a file that shell never reads. The environment alone cannot settle the zsh file; see Impact.
3. **bash login reads `/etc/profile`, then the first existing of `~/.bash_profile`, `~/.bash_login`, `~/.profile`, and nothing else from `$HOME`** (E-7, E-8): with `.bash_profile` present, `.bash_login` and `.profile` are silent (b3); with none, no user file (b4). A login shell does **not** read `~/.bashrc` (b3, `-l -i`). Interactive non-login reads `~/.bashrc` only (b1, b3) and no system file: macOS's `/bin/bash` 3.2 does not read `/etc/bashrc` for an interactive non-login shell, since `PS1` stays bash's default `\s-\v\$ ` (E-8). `/etc/bashrc` reaches login shells only because `/etc/profile` sources it (E-3), and it `return`s at once when `PS1` is empty (non-interactive). So **creating `~/.bash_profile` when only `~/.profile` exists silences `~/.profile`**, which is D6's `SHELL_FILE_WOULD_SHADOW` case, confirmed.
4. **`path_helper` runs only in login shells**: `/etc/zprofile` (zsh) and `/etc/profile` (bash, and sh) call it; `/etc/zshrc`, `/etc/bashrc` do not, and `/etc/zshenv` is absent (E-3). Observed in E-5 and E-8: `-i` without `-l` keeps `PATH` unchanged.
5. **What `path_helper` does to an existing `PATH`** (E-9, observed; its manual does not say): it emits `/etc/paths` in order, then each `/etc/paths.d` file's entries (in file-name order as observed), then **every entry of the incoming `PATH` that is not already in that list, in their original order**. An incoming entry that duplicates a system one is dropped (`/usr/local/bin` from `/a:/usr/local/bin:/b`). So **any entry placed before `/etc/zprofile` or `/etc/profile` runs ends up after all system paths**: on Intel that puts it after `/usr/local/bin`, Homebrew's prefix, which is the first line of `/etc/paths` (E-3).
6. **Where the block must sit to come first** (E-5, E-6, E-8): in any user file read *after* the system file that runs `path_helper`.
   - zsh: `$ZDOTDIR/.zprofile` (or `$HOME` without `ZDOTDIR`), `.zshrc` or `.zlogin`. **Not `~/.zshenv`**: it is read before `/etc/zprofile`, and E-5 shows `/SHIM` demoted to last.
   - bash: the login file chosen by finding 3. `~/.bashrc` is never read by a login shell.
   - `.zprofile` is the one zsh file every login shell reads, interactive or not, and it is the file both providers choose (finding 9). `.zshrc` is not read by `zsh -l -c` (E-5).
   - Inside `.zprofile`, the block must come **after** any line that prepends something else, such as the `brew shellenv` line this machine has (E-2). Two prepends leave the later one first, so a block written above that line would sit behind `/usr/local/bin`. Appending at the end of the file achieves this. Whether a line after the block prepends something is the user's file, and the outcome is a prediction (D7).
7. **D6's presence guard fails in a nested login shell** (E-6, second run). A login shell started with an inherited `PATH` that already holds the shim first runs `path_helper`, which moves the shim behind the system paths. The guard `case ":$PATH:" in *':<dir>:'*) ;;` then sees the entry present and does nothing, so the shim stays behind `/usr/local/bin`. That happens with `zsh -l`, an editor's integrated terminal started from a shell, or `tmux`, which by default starts login shells; the tmux default is not cited here. A login shell started from launchd's environment, as a terminal emulator's first window is, does not have the shim in its inherited `PATH`, and the block works (E-6, first run).
   - Homebrew's `shellenv` guards the other way: it returns only when `PATH` *begins with* its prefix, and otherwise prepends again (E-11). That re-establishes its position, at the cost of a duplicate entry.
   - A form that satisfies both INV-16 and spec 05's "Never duplicate a `PATH` entry" has to test "is first", not "is present". When the entry is present but not first, it has to remove the entry and prepend it. That is more shell code than D6's single line, and it is not observed here.
8. **Terminal emulators.**
   - iTerm2's documentation says its "login shell" option invokes `login`. `login` marks a login session with a leading hyphen on argv[0] by default, and bash and zsh treat that as a login shell (E-7, E-10). This machine's iTerm2 profile has `"Custom Command" = No`, which is the login-shell choice (E-10, observed in preferences only).
   - Apple's Terminal guide says only that "Default login shell" is "the login shell set up for your user account". It does not say that the shell is started as a login shell. That is **not established here** (Cannot determine).
   - `$SHELL` and `dscl … UserShell` agree on this machine (`/bin/zsh`, E-1). They are different sources: the Homebrew installer reads `$SHELL`, the MacPorts postflight reads `dscl UserShell` (E-11).
9. **What the providers write** (E-11):
   - **Homebrew's installer writes no startup file.** It *prints* the lines for the user to append the `eval "$(…/brew shellenv zsh)"` line to `${ZDOTDIR:-$HOME}/.zprofile` (zsh, macOS) or `~/.bash_profile` (bash, macOS).
     - For bash it names `.bash_profile` unconditionally, so a user who follows it with only `~/.profile` silences `~/.profile`. That is the hazard D6 refuses.
     - With sudo and the `/opt/homebrew` prefix it also installs `/etc/paths.d/homebrew`. `path_helper` then places `/opt/homebrew/bin` *after* the system paths (finding 5), which is why the `shellenv` line still matters there.
   - **MacPorts' postflight does write.** It appends an `export PATH="<prefix>/bin:<prefix>/sbin:$PATH"` stanza, bracketed by `# MacPorts Installer addition on <timestamp>` comments. The target for zsh is `$HOME/.zprofile` (it ignores `ZDOTDIR`). The target for bash is the first existing of `.bash_profile`, `.bash_login`, or else `.profile` (it follows the first-of-three rule and creates `.profile` only when none exists).
     - It backs up the file first, unless the file already carries its marker.
     - It skips the write when `su $USER -l -c printenv PATH` already contains `<prefix>/bin`.
     - It chooses the shell from `dscl UserShell`.
10. **This machine** (E-2):
    - Present: `~/.zshenv`, `~/.zprofile` (it has a `brew shellenv` line), `~/.zshrc`, `~/.profile`, `~/.bashrc`.
    - Absent: `~/.zlogin`, `~/.bash_profile`, `~/.bash_login`.
    - `ZDOTDIR` is unset in the environment.
    - For bash on this machine, `~/.profile` is the file a login shell reads today, and creating `~/.bash_profile` would shadow it.
    - No user startup file mentions `/opt/local`, `MacPorts` or `ZDOTDIR`, yet `/opt/local/bin` is on this session's `PATH`. How it got there was not determined (Cannot determine).
11. **Offline.** Everything the writer needs is a local read: the environment, file existence, `/etc/*`, and the startup file it will edit. No network is involved.
12. **Builtins and reserved words** (E-12, E-13), written to `data/policy/shell-builtins.toml`:
    - zsh 5.9: **102 builtins** (`zsh -f`, autoloadable ones included), **31 reserved words**, and 45 more names that become builtins only after a user `zmodload`, held separately as `module_builtins`.
    - bash 3.2.57: **58 builtins**, **21 reserved words**.
    - Both reserved-word lists match the shells' manuals exactly.
    - Names in the lists that fit part I's shim name class and could plausibly collide with a real executable include `test`, `echo`, `printf`, `kill`, `pwd`, `true`, `false`, `time`, `which`, `type`, `hash`, `getopts`, `ulimit`, `umask`, `wait`, `read`, `cd`, `command`, and for zsh `where`, `whence`, `r`, `log`, `sched`, `limit`.

## Cannot determine

- **Terminal.app's login-shell invocation — UNRESOLVED.** Apple's guide does not say it starts the shell as a login shell (E-10), and Terminal was not running, so `-zsh` could not be observed. To settle it, open a Terminal window and run `ps -o args= -p $$`. A `-zsh` result settles it by observation, and checking the parent process (`ps -o args= -p $PPID`, expected `login -pf <user>`) would show the mechanism. Alternatively, find Apple developer documentation that states it.
- **iTerm2's login-shell invocation, observed — UNRESOLVED.** Only the documentation and the profile preference were read (E-10); iTerm2 was not running. The same `ps` check in an iTerm2 window settles it.
- **How `/opt/local/bin` reached this session's `PATH`** (E-2): none of the five present user files mentions `/opt/local`, and `/etc/paths.d` does not either. The mechanism (a file sourced from one of them, the Claude application's environment, launchd) was not investigated, because reading file contents was out of scope. It does not affect the findings.
- **`path_helper`'s handling of entries is observed, not documented** (E-9). Its manual describes building from `/etc/paths` and `/etc/paths.d` and is silent on incoming entries, and Apple publishes no source for it that this note read. Re-observe on each major macOS release.
- **Why the interactive zsh's `${builtins}` lacks `log`** (E-12) was not determined. The file keeps `log`, which errs toward refusing.
- **Aliases, functions and user-loaded modules** cannot be seen without sourcing the user's files. That is deferred to the user (D7's report). `module_builtins` lists what a `zmodload` *could* add, not what is loaded.
- **The `tmux` default of starting login shells** is stated in finding 7 from general knowledge, not cited. Cite `man tmux` (`default-command`) before relying on it.

## Impact on the specification

These are for the main agent to judge. Nothing in `docs/spec/` was edited.

- **Confirms** D6's bash rule and `SHELL_FILE_WOULD_SHADOW` (finding 3). For zsh no file shadows another, so creating `$ZDOTDIR/.zprofile` is always shadow-free (finding 1). `invariant_03_creating_the_shell_file_never_shadows_another_startup_file` needs a bash case only, plus a zsh case asserting that creation is admitted.
- **Answers D6's "the file is R-31's answer".** The zsh file is `${ZDOTDIR:-$HOME}/.zprofile`. The bash file is the first existing of `~/.bash_profile`, `~/.bash_login` and `~/.profile`, and `~/.bash_profile` is created only when none exists. In both cases the block is appended **at the end**, after any existing prepend such as `brew shellenv` (findings 6, 9). `~/.zshenv` and `~/.bashrc` are wrong targets, because `path_helper` demotes the former (finding 6) and login bash never reads the latter.
- **Contradicts D6's block as specified** (finding 7). Its presence guard leaves the shim behind the system paths in a nested login shell. That breaks the outcome D6 exists for, and D7's "prediction" would be wrong in a common case. The block needs an "is first, otherwise move to the front" form that also keeps spec 05's no-duplicate MUST and INV-16. That is a Tier 3 change to D6 and needs a `docs/spec-evolution.md` entry if adopted. At the least, D7's prediction must say that nested login shells demote the shim.
- **A gap in D6's `ZDOTDIR` handling** (finding 2). An unexported `ZDOTDIR` set in `~/.zshenv` is invisible to the tool, and the writer would edit a `.zprofile` the shell never reads. Options for the main agent:
  - (a) A bounded textual scan of `~/.zshenv` for the token `ZDOTDIR`, never sourcing it. When found with no `ZDOTDIR` in the environment, refuse (a new refusal, or `MANUAL_REVIEW`).
  - (b) Declare it, as D6 already declares `$SHELL` versus `getpwuid`.
  A `ZDOTDIR` outside `HOME` is already refused by D6.
- **`$SHELL` versus the login shell** (finding 8): MacPorts' own installer uses `dscl UserShell` (a spawn of `/usr/bin/dscl`). That is a third route besides `getpwuid` and a new program; D6's "declared, not claimed" stands.
- **D5's builtin refusal** has its data. The `module_builtins` split is a decision for D5, and it is the main agent's: refusing them would forbid shims named `mv`, `rm` or `ln`.
- **Required with the data file, outside this note's two files:**
  - `data/policy/policy.toml`'s `version` (now `2026.11.04-4`) must become `2026.11.22-1` in the same change. `invariant_20_the_policy_version_names_the_newest_read_date_in_the_data` (`crates/policy/tests/invariants.rs`) fails otherwise, since the new file's `read_on` is the newest in the directory.
  - `.claude/rules/policy-data.md`'s file table should gain a `shell-builtins.toml` row.
  - `docs/spec/09-research-checklist.md` needs the R-31 entry that `I.md` asks for.

## Re-check when

- macOS ships a new `/bin/zsh` or `/bin/bash` version, or changes `/etc/zprofile`, `/etc/profile`, `/etc/bashrc` or `path_helper`. Re-run E-3, E-5, E-8, E-9, E-12 and E-13 on each major macOS release.
- Homebrew's `install.sh` "Next steps" or `cmd/shellenv.sh`, or MacPorts' `portmgr/dmg/postflight.in` changes. The commits read are recorded in E-11.
- Before relying on Terminal.app or iTerm2 opening login shells (Cannot determine).
