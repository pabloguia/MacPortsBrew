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
  installed → note that Homebrew's compiler shim already strips `-I/opt/local`
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
