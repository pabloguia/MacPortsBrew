# F-2 — Does `/usr/local` interfere with MacPorts builds, and what is the equivalent hazard in the other direction?

- **Status:** partially answered — the MacPorts direction is confirmed from a
  *different* official source than the pre-seeded one, because the pre-seeded
  source could not be read today (`UNRESOLVED`, finding 5). The Homebrew
  direction is answered, and it **contradicts the shape the checklist assumed**.
- **Answered on:** 2026-09-02
- **Machine:** macOS 26.6.2 (build 25G83), x86_64, Homebrew 6.0.21 @
  `/usr/local`, **MacPorts not installed** (no `/opt/local`). Every MacPorts
  claim is cited; the Homebrew claims are observed or cited from the Homebrew
  repository.
- **Blocking:** INV-10 (sanitised environment for provider invocations);
  `06-cli-and-ux.md` / the `doctor` environment checks; the cross-provider
  contamination story in `04-migration-and-safety.md`.

## Question

The pre-seeded finding F-2 in `09-research-checklist.md` says, sourced to the
MacPorts FAQ read 2026-09-02:

> `/usr/local` can interfere with MacPorts even though MacPorts installs to
> `/opt/local`: the GNU autotools and gcc probe `/usr/local` for headers,
> libraries and binaries. On Intel Macs, `/usr/local` is exactly where Homebrew
> lives.

Two tasks: (a) re-fetch the FAQ today and record the current guidance text with
a date; (b) find the equivalent hazard in the **other** direction — MacPorts
affecting Homebrew — from official Homebrew sources.

## Evidence

### E-1 · Machine and provider presence (observed 2026-09-02)

```
$ date "+%Y-%m-%d %H:%M:%S %Z"
2026-09-02 09:39:08 -03

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

$ command -v brew
/usr/local/bin/brew

$ brew --prefix
/usr/local
```

**Quieting environment used, and recorded as required:** every Homebrew
invocation below ran with `HOMEBREW_NO_AUTO_UPDATE=1`, `HOMEBREW_NO_ANALYTICS=1`,
`HOMEBREW_NO_ENV_HINTS=1` — present in the session environment and also passed
explicitly on the command line (E-6). No `brew update`, `brew tap` or any
mutating subcommand was run.

### E-2 · The MacPorts FAQ could not be read today

Attempted with the fetch tool on **2026-09-02**:

| URL | Result |
|---|---|
| `https://trac.macports.org/wiki/FAQ` | "Access Denied" page served by **Anubis**, error code `9e4edb5b6b850c41`; no wiki content |
| `https://trac.macports.org/wiki/FAQ?format=txt` | same Anubis interstitial |
| `https://trac.macports.org/wiki/FAQ#Whyisusrlocalaproblem` | same |
| `https://trac.macports.org/wiki/FAQ?action=view` | same |
| `https://trac.macports.org/wiki/ProblemHotlist` | same |

`curl` is denied by this repository's permission policy
(`Bash(curl *)` → "Blocked by security policy"), so no alternative client was
available. **The 2026-09-02 text of the MacPorts FAQ is therefore not recorded
in this note.** The quoted F-2 wording above remains attributed to whoever read
it during the pre-implementation review; this attempt could neither confirm nor
refute it.

### E-3 · The MacPorts Guide *does* state the hazard, and names the mitigation

Source: MacPorts Guide, §3.1.12 "port install",
<https://guide.macports.org/chunked/using.html#using.port.install>, read
2026-09-02. Cross-checked against the guide's own source
(`macports/macports-guide`, `guide/xml/using.xml`, lines 566–580, section
`xml:id="using.port.install"`, read 2026-09-02 via
`gh api repos/macports/macports-guide/contents/guide/xml/using.xml`).

Verbatim, from the rendered page:

> If the installation of a port fails, you should always clean and try again,
> i.e., run
>
> ```
> $ sudo port clean portname
> ```
>
> and re-execute the command you ran before.
>
> You might also want to try enabling trace mode, which can prevent conflicts
> caused by files installed by other ports or in common system locations, such
> as `/usr/local`. To do that, re-run the installation with the `-t` flag, i.e.,
>
> ```
> $ sudo port -t install portname
> ```

And `port(1)` on what `-t` actually does (`macports-base`, `doc/port.1.txt`,
read 2026-09-02):

```
-t::
    Enable trace mode debug facilities on platforms that support it, currently
    only macOS.
    +
    This feature is two-folded. It consists in automatically detecting and
    reporting undeclared dependencies based on what files the port reads or what
    programs the port executes. In verbose mode, it will also report unused
    dependencies for each stage of the port installation. It also consists in
    forbidding and reporting file creation and file writes outside allowed
    directories (temporary directories and ${workpath}).
```

### E-4 · MacPorts sanitises the build environment by default

`macports-base`, `doc/macports.conf.in`, read 2026-09-02:

```
# Extra environment variables to keep. MacPorts sanitizes its
# environment while processing ports, keeping:
# - DISPLAY
# - DYLD_FALLBACK_FRAMEWORK_PATH, DYLD_FALLBACK_LIBRARY_PATH,
#   DYLD_FRAMEWORK_PATH, DYLD_INSERT_LIBRARIES, DYLD_LIBRARY_PATH
# - JAVA_HOME
# - ARCHIVE_SITE_LOCAL, MASTER_SITE_LOCAL, PATCH_SITE_LOCAL
# - PORTSRC
# - ALL_PROXY, FTP_PROXY, http_proxy, HTTPS_PROXY, NO_PROXY, RSYNC_PROXY
# - GROUP, USER
# - COLUMNS, LINES
# Variables listed in extra_env are added to this list. This has no
```

### E-5 · The other direction — Homebrew's *build* environment explicitly ignores `/opt/local`

Source: Homebrew repository, `Library/Homebrew/shims/super/cc` (the superenv
compiler shim), read 2026-09-02 at `Homebrew/brew` HEAD
`fd1873821d788bf802f3a241490ee484aac357b6` (committed 2026-09-02T10:59:34Z),
via `gh api repos/Homebrew/brew/contents/Library/Homebrew/shims/super/cc`.
Verbatim, lines 262–300:

```ruby
      path = canonical_path(val)
      args << "-I#{val}" if keep?(path) && @iset.add?(path)
    when /^-L(.+)?/
      val  = chuzzle(Regexp.last_match(1)) || enum.next
      path = canonical_path(val)
      args << "-L#{val}" if keep?(path) && @lset.add?(path)
    else
      args << arg
    end

    args
  end

  def keep?(path)
    # Allow references to self
    if @self_dir && path.start_with?(@self_dir)
      true
    # first two paths: reject references to Cellar or opt paths
    # for unspecified dependencies
    elsif path.start_with?(*@keg_dirs)
      dep = path[keg_regex, 2]
      dep && @deps.include?(dep)
    elsif path.start_with?(*@prefix_dirs)
      true
    elsif path.start_with?("/opt/local", "/opt/boxen/homebrew", "/opt/X11", "/sw", "/usr/X11")
      # ignore MacPorts, Boxen's Homebrew, X11, fink
      false
    elsif prefix != "/usr/local" && path.start_with?("/usr/local")
      # ignore things in /usr/local if Homebrew is in another prefix;
      # on macOS, this probably means that the user is on ARM and has an Intel
      # Homebrew in /usr/local
      false
    elsif mac?
      true
    else
      false
    end
  end
```

### E-6 · `brew doctor` at HEAD has no MacPorts, Fink or `/opt/local` check

`Library/Homebrew/diagnostic.rb` at HEAD (1607 lines) and
`Library/Homebrew/extend/os/mac/diagnostic.rb`, fetched 2026-09-02 and grepped
locally:

```
$ grep -n -i "opt/local\|macports\|fink\|/sw" Library/Homebrew/diagnostic.rb
(no matches)
```

The only `MacPorts` occurrences in the macOS diagnostic file point users *to*
MacPorts, not away from it (`extend/os/mac/diagnostic.rb`, lines 139–163):

```ruby
          macports_url = Formatter.url("https://www.macports.org")
          what = if OS::Mac.version.outdated_release?
            tier = 3
            who << " (and Apple)"
            remediation = <<~EOS
              You will have better luck with MacPorts which still supports older versions of macOS:
                #{macports_url}
            EOS
```

Historical check: `Library/Homebrew/diagnostic.rb` at tags `1.9.0`, `2.7.0` and
`3.6.0` contains **zero** case-insensitive matches for `macports`
(`gh api …/contents/Library/Homebrew/diagnostic.rb?ref=<tag>` → `grep -c -i macports` → `0` for all three).
Whatever `brew doctor` may once have checked about other package managers, it
has not been in `diagnostic.rb` since at least Homebrew 1.9.0.

The two prose pages checked, read 2026-09-02, contain nothing about MacPorts,
Fink, `/opt/local` or `/sw`:
<https://docs.brew.sh/Common-Issues> and <https://docs.brew.sh/FAQ>.

`brew doctor` run on this machine, 2026-09-02 (diagnostic, read-only,
completed well inside a 120 s timeout, exit status 0):

```
$ HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1 timeout 120 brew doctor
Please note that these warnings are just used to help the Homebrew maintainers
with debugging if you file an issue. …

Warning: Some installed formulae are deprecated or disabled.
You should find replacements for the following formulae:
    gemini-cli

Warning: You have unlinked kegs in your Cellar.
…
Run `brew link` on these:
  pydantic

Warning: You have an unnecessary local Core tap!
This can cause problems installing up-to-date formulae.

Please remove it by running:
 brew untap homebrew/core

Warning: You are using macOS on Intel x86_64.
We do not provide support for this platform (as-of September 2026, announced August 2025).

Apple have dropped Intel x86_64 support in macOS Golden Gate (27).
GitHub Actions are dropping macOS Intel x86_64 runners in 2027.
Homebrew is a non-profit project run entirely by volunteers, not employees.
If the biggest companies in the world cannot support macOS Intel x86_64
any longer, sadly neither can we.

You will have better luck with MacPorts which still supports macOS Intel x86_64:
  https://www.macports.org
This is a Tier 3 configuration:
  https://docs.brew.sh/Support-Tiers#tier-3
You can report issues with Tier 3 configurations to Homebrew/* repositories!
```

**No `/opt/local` output of any kind.** MacPorts is absent from this machine, so
this observation cannot distinguish "no such check exists" from "the check did
not fire"; E-6's source grep is what establishes the former.

### E-7 · Unrelated but recorded: `brew doctor` confirms the R-1 side effect

The `Warning: You have an unnecessary local Core tap!` above is the residue of
the 1.4 GB `homebrew/core` clone documented in
`docs/research/R-1-homebrew-read-interfaces.md`. It was not created by this
session.

## Findings

1. **The MacPorts direction is confirmed, from the Guide rather than the FAQ.**
   MacPorts' own documentation states that "conflicts caused by files installed
   by other ports or in common system locations, such as `/usr/local`" are a
   real failure mode for `port install`, and prescribes trace mode (`port -t`)
   as the remedy (E-3). Trace mode works by *detecting* what files a build reads
   and *forbidding* writes outside allowed directories (E-3, `port(1)`), which
   is precisely a mitigation for a build picking up headers, libraries or
   binaries from `/usr/local`.

2. **The mechanism named in the pre-seeded F-2 text is not itself quoted in the
   Guide.** The Guide says "common system locations, such as `/usr/local`"; it
   does **not** say "the GNU autotools and gcc probe `/usr/local`". That
   specific mechanism is currently uncited in this repository. Treat it as
   plausible and *unverified* until the FAQ is readable again.

3. **MacPorts already sanitises its build environment** (E-4) — it keeps only a
   named allowlist of environment variables. `/usr/local` contamination is
   therefore not primarily an *environment-variable* problem; it is a
   *filesystem search path* problem, which is why the remedy is trace mode
   (a filesystem sandbox) rather than a variable. **This nuances INV-10:** a
   sanitised environment is necessary but is not what protects MacPorts builds
   from `/usr/local`.

4. **The equivalent hazard in the other direction exists, and Homebrew mitigates
   it in the compiler, not in `doctor`.** Homebrew's superenv compiler shim
   drops any `-I` or `-L` flag whose canonical path starts with `/opt/local`,
   `/opt/boxen/homebrew`, `/opt/X11`, `/sw` or `/usr/X11`, with the comment
   "ignore MacPorts, Boxen's Homebrew, X11, fink" (E-5). This is a *build-time*
   defence at the exact point where a MacPorts install would otherwise leak into
   a Homebrew source build. It is symmetric with MacPorts' trace mode in intent
   and stronger in default posture: MacPorts' `-t` is opt-in, Homebrew's shim is
   always on for source builds.
   The same `keep?` method also rejects `/usr/local` paths when Homebrew's own
   prefix is *not* `/usr/local` — i.e. Homebrew defends an Apple Silicon
   `/opt/homebrew` install against a leftover Intel `/usr/local` install (E-5).

5. **UNRESOLVED — the MacPorts FAQ text of 2026-09-02 was not obtained.**
   `trac.macports.org` serves an Anubis bot-mitigation interstitial to this
   agent's fetch tool for every URL form tried, and `curl` is denied by policy
   (E-2). A future attempt needs either a fetch path that solves the Anubis
   challenge, or a human to paste the page. Until then, cite
   `guide.macports.org` §3.1.12 (E-3) instead of the FAQ for this fact.

6. **A hypothesis I held is disconfirmed: `brew doctor` has no MacPorts/Fink
   check.** There is no `check_for_other_package_managers` (or any
   `/opt/local` / Fink / `/sw` reference) in `diagnostic.rb` at HEAD, nor at
   tags 1.9.0, 2.7.0 or 3.6.0 (E-6), and neither `docs.brew.sh/Common-Issues`
   nor `docs.brew.sh/FAQ` mentions the topic. Anyone writing "brew doctor warns
   about MacPorts on PATH" is recalling something that is not in current
   Homebrew. **MacPortsBrew's `doctor` cannot delegate this check to
   `brew doctor`; it must implement it.**

7. **What `brew doctor` *does* say on this machine is the product's own
   thesis, in Homebrew's words** (E-6): on Intel x86_64 it declares Tier 3, and
   it recommends MacPorts by name and URL. This is the same policy fact as F-1,
   now observed in the tool's output rather than only read on the website — and
   it now carries a harder date than the website did: "as-of September 2026,
   announced August 2025", plus "Apple have dropped Intel x86_64 support in
   macOS Golden Gate (27)" and "GitHub Actions are dropping macOS Intel x86_64
   runners in 2027".

## Cannot determine

- The current MacPorts FAQ wording (finding 5) and therefore whether the
  autotools/gcc mechanism in F-2's pre-seeded text is still, or was ever, the
  project's own wording.
- Whether Homebrew's *runtime* (as opposed to build-time) behaviour is affected
  by `/opt/local` on `PATH`. The shim covers compiler flags; PATH-order effects
  on `brew`'s own subprocesses were not investigated.
- Whether MacPorts trace mode is enabled by default in any MacPorts
  configuration (`macports.conf` was read; no trace-mode default key was found,
  but the file was grepped for `archive`/`buildfromsource`, not exhaustively
  read).
- The reciprocal question of whether Homebrew's shim covers `-isystem`,
  `-F`/`-iframework` and `rpath` flags as well as `-I`/`-L`. Only the `-I`/`-L`
  arms were read.

## Impact on the specification

- **Confirms INV-10's motivation, but relocates it.** The `/usr/local` ↔
  `/opt/local` hazard is a *filesystem search path* problem that both providers
  already mitigate in their build machinery (MacPorts: opt-in trace mode;
  Homebrew: always-on compiler shim). A sanitised environment for MacPortsBrew's
  own provider invocations remains right, but the specification should not imply
  that environment sanitisation is what prevents cross-provider contamination
  *inside a build*. Recommend a wording review of the INV-10 rationale text
  (not the invariant itself, which is Tier 1 and not mine to change).
- **Contradicts an assumption in the work order for this item** — namely that
  `brew doctor` has a check about `/opt/local` on PATH (finding 6). It does not,
  and has not for many major versions. Anything in `docs/spec/` that plans to
  surface or defer to such a check needs correcting. I did not grep the whole
  spec for this; the main agent should.
- **`06-cli-and-ux.md` / `doctor`:** the environment check for cross-provider
  contamination is MacPortsBrew's own responsibility in *both* directions. The
  concrete, citable checks it can implement are: (a) `/usr/local` present and
  non-empty while MacPorts is installed → warn, recommend `port -t` for failing
  builds, citing Guide §3.1.12; (b) `/opt/local` present while Homebrew is
  installed → note that Homebrew's compiler shim already strips `-I/opt/local` (corrected 2026-10-12: only when refurbishing is on; see the addendum)
  and `-L/opt/local`, so the residual risk is in `PATH`-resolved *tools*, not in
  headers and libraries. Direction (b) is materially *lower* risk than direction
  (a) and the UX should say so rather than presenting them as symmetric.
- **F-1 corroboration.** Finding 7 gives an on-machine observation of the Intel
  Tier 3 policy with dates, which the F-1 entry currently sources only to
  `docs.brew.sh/Support-Tiers`. Worth adding as a second, harder-dated source.
- **No contradiction found** with `02-domain-model.md`.

## Re-check when

- **Time-sensitive.** Homebrew's superenv shim (E-5) is a source file with no
  stability guarantee; re-read it whenever the `doctor` contamination check is
  touched, and record the commit sha as this note does.
- `trac.macports.org` becomes machine-readable again — then finish finding 5 and
  either confirm or retire the autotools/gcc wording in the checklist's F-2.
- Homebrew adds a diagnostic about other package managers (which would make
  finding 6 stale and might let `doctor` delegate).
- MacPorts changes trace mode's default, or the Guide's §3.1.12 wording changes.
- The Intel dates in E-6 move in either direction (they are policy data under
  INV-20).

## Addendum, re-read 2026-10-12 (pass 13, DP-041)

This note's "Re-check when" asked for a re-read whenever the `doctor`
contamination check is touched; pass 13 touches it. Read-only, on this machine,
2026-10-12: macOS 26.7.1 (`sw_vers`, build 25G241) x86_64, **Homebrew 7.0.4** at
`/usr/local/Homebrew` (its repository at `edb70f031e4170c780799633a1226ff73e1077f4`,
committed 2026-09-17T16:04:39Z, tag 7.0.4), **MacPorts 2.12.6** at `/opt/local`
(`port` warned its definitions were more than two weeks old; the ports index
`PortIndex` was modified 2026-09-06 local time). Both are local copies; their
dates are the **index dates** the policy rows carry as `index_as_of`.

**Corrections to earlier findings, from this re-read** (a docs-auditor review of
the first draft of this addendum found them; each was re-checked against the
source):

- **Finding 4 and E-5 overstated the shim.** The `-I`/`-L` filtering is **not**
  always on. `refurbished_args` is reached only when `refurbish_args?` is true
  (`shims/super/cc` lines 129-133), and that is `config.include?("O")` (lines
  435-437). On macOS `determine_cccfg` never adds `O`
  (`extend/os/mac/extend/ENV/super.rb` lines 78-88); it is added only by
  `ENV.refurbish_args`, whose callers are the Python `setup.py`/`build.py` and
  setuptools-shim path, virtualenv creation and `bundle exec`. The comment in
  `extend/ENV/super.rb` lines 142-143 ("Only active under the make/bsdmake
  wrappers currently") is stale. The same gate existed at E-5's commit.
- **Finding 7 is out of date for 7.0.4 on Intel.** `macos_bottle_remediation`
  (`diagnostic.rb` lines 112-130, since commit `686e8a54d5`, 2026-09-05) recommends
  **nothing** on Intel macOS Tahoe or later ("Existing bottles may still work,
  but updated formulae may build from source", because MacPorts lacks a full set
  of Intel Tahoe binaries); it names MacPorts only on other outdated releases.
  The Tier 3 text in `extend/os/mac/diagnostic.rb` lines 136-167 does not mention
  MacPorts.
- E-5's verbatim shim block no longer matches the file: `d7b311f` (2026-09-05)
  keeps explicit `-L` for declared dependencies and `9801e04` (2026-09-15) adds the
  `@build_dir` arm. The `/opt/local` arm itself is unchanged.

### E-8 · Homebrew's compiler shim, `Library/Homebrew/shims/super/cc` at that commit

`keep?(path)` (lines 284-309) returns `false` for a path starting with
`/opt/local`, `/opt/boxen/homebrew`, `/opt/X11`, `/sw` or `/usr/X11` ("ignore
MacPorts, Boxen's Homebrew, X11, fink"), and for `/usr/local` when Homebrew's own
prefix is not `/usr/local`. It is called only from the `-I` and `-L` arms of
`refurbish_arg` (lines 264-272), which is reached only through
`refurbished_args`, and **that is skipped unless argument refurbishing is on** (see
the corrections above), as it is for the linker and for autoconf `configure` runs.
So in an ordinary macOS formula build the shim does not filter `-I` or `-L`
flags. Other arguments (`-isystem`, `-F`/`-iframework`) reach the compiler through
the `else args << arg` arm (line 274) and `-Wl,` options through the
`-W[alp],` arm (lines 233-234); none is filtered. The search paths Homebrew adds
itself (`HOMEBREW_ISYSTEM_PATHS`, `_INCLUDE_PATHS`, `_LIBRARY_PATHS`,
`_RPATH_PATHS`) come from its own environment, not from the user's.

### E-9 · What reaches a formula build from the user's `PATH`

`Library/Homebrew/extend/ENV/super.rb`, `determine_path` (lines 189-207; the gcc
formula's `opt_bin` is appended at lines 197-204): in a superenv build the build's
`PATH` is **rebuilt** from the shims directory, the formula's dependencies'
`opt_bin`, Homebrew's extra paths and `/usr/bin:/bin:/usr/sbin:/sbin`. The user's
`PATH` is not an input, so `/opt/local/bin` does not reach a build in that
environment. This holds for every superenv build, and it is the defence
`default_clean_path` records.

Where the user's `PATH` **does** reach Homebrew-run code: a formula that declares
`env :std` (`extend/ENV/std.rb` line 29 prepends `ORIGINAL_PATHS`, taken from the
user's `PATH`, `global.rb` line 144); and `post_install`, which
`formula.rb` lines 1729-1738 runs with `PATH.new(ORIGINAL_PATHS)`, including after
a bottle is poured. The install command's `--env=` option is described as
"Disabled other than for internal Homebrew use" (`cmd/install.rb` lines 58-61, 185:
`odisabled` pointing at `env :std` in formula files); the manual text about
`--env=std` belongs to the build-environment shell command (`docs/Manpage.md` at
that commit, under `### sh`).

### E-10 · MacPorts, re-read

MacPorts Guide, 3.1.12 "port install" (<https://guide.macports.org/chunked/using.html>,
read 2026-10-12), unchanged from E-3: "You might also want to try enabling trace
mode, which can prevent conflicts caused by files installed by other ports or in
common system locations, such as `/usr/local`. To do that, re-run the
installation with the `-t` flag". `port(1)` (<https://man.macports.org/port.1.html>,
read 2026-10-12): `-t`, "Enable trace mode debug facilities on platforms that
support it" (the fetched text cut off after "support i"; E-3 holds the full text).
In `/opt/local/libexec/macports/lib/macports1.0/macports.tcl` (2.12.6): `porttrace`
is listed among the config file options (lines 52-55 and 81), defaults to `no`
(lines 1414-1415), and the `-t` flag sets it through the `ports_trace` global
option (line 221). The installed `/opt/local/etc/macports/macports.conf` is
byte-identical to `macports.conf.default` and does not mention it. So trace mode
is **off unless asked for**: opt-in, whether by flag or by a configuration line.

### E-11 · `brew doctor` has no check for another package manager's prefix

`Library/Homebrew/diagnostic.rb` (last changed 2026-09-13 in that checkout) and
`extend/os/mac/diagnostic.rb` contain no `/opt/local`, MacPorts-on-`PATH`, Fink or
`/sw` check. (`check_for_nix_homebrew` detects Nix managing Homebrew, a different
thing.) Finding 6 stands for 7.0.4: `doctor` must implement the check itself.

### What changed in "Cannot determine"

| F-2 item | Now |
|---|---|
| The MacPorts FAQ text | Not re-attempted; whether it is still blocked is unknown. The Guide is the cited source. |
| Whether `/opt/local` on `PATH` affects Homebrew at runtime | **Builds: answered** (E-9: the build `PATH` is rebuilt, except under `env :std`). Code Homebrew runs with the user's `PATH` is known for `post_install` only; `brew`'s other non-build subprocesses: **not investigated**, still unanswered. |
| Whether the shim covers `-isystem`, `-F`, `rpath` | **Answered: it does not**, and it does not filter `-I`/`-L` either in an ordinary build (E-8). |
| Trace mode default | **Answered for 2.12.6:** `porttrace` defaults to `no` (E-10). |

### What each advice sentence may claim (the input to pass 13, step C)

| Sentence (`BuildDefence`) | May claim | May not claim |
|---|---|---|
| `opt_in_trace` (the affected provider is MacPorts) | Its own Guide says files in common system locations such as `/usr/local` can conflict with a port install and suggests trace mode (`port -t`) as a way to try to avoid that; trace mode is not on by default (E-10). | The autotools/gcc probing mechanism (uncited since F-2). That `-t` always works: it is for "platforms that support it". Anything about what `/usr/local` holds on this machine (nothing looked). |
| `default_clean_path` (the affected provider is Homebrew) | In a formula build Homebrew rebuilds the build's `PATH` from its own directories, so a directory on the user's `PATH`, `/opt/local/bin` included, is not an input to that build (E-9). | That include or library flags are filtered (E-8: not in an ordinary build). That every build is protected (`env :std`, `post_install` and anything run outside Homebrew's build environment are not). Anything about `brew`'s other non-build subprocesses. |
| Both | Which copy of a tool you get when you type its name still follows your `PATH` order, and that order is yours to arrange. The finding reports only; nothing was changed. | A remedy the tool performs. |

### Re-check when (added)

Homebrew's `shims/super/cc`, `ENV/super.rb` or the macOS `determine_cccfg` change
(record the commit as above); a MacPorts release changes trace mode, the
`porttrace` default or the Guide's 3.1.12 wording; `brew doctor` gains a check for
another package manager.
