# R-17 — Where does each provider hold a lock while it mutates, and can a filesystem read alone tell "a provider is mutating now"?

- **Status:** answered (residuals, see "Cannot determine")
- **Answered on:** 2026-10-05
- **Machine:** macOS 26.7.1 (`sw_vers`), Darwin 25.6.0 (`uname -sr`), x86_64;
  Homebrew 7.0.4 at `/usr/local` (repository `/usr/local/Homebrew`, `HEAD`
  `edb70f031e`); MacPorts 2.12.6 at `/opt/local`; user `pirado`, not elevated.
- **Environment used to keep the providers quiet:** `HOMEBREW_NO_AUTO_UPDATE=1
  HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_INSTALL_FROM_API=1` for the three `brew`
  calls (`--version`, `--prefix`, `--repository`); `port version` is the only
  `port` call (argv-quieted per R-16 finding 6, but `version` is the one read
  without `-q`; it printed the stale-index warning). No other provider command
  ran.
- **Sources read** (all local copies, read 2026-10-05; line numbers refer to
  these exact files):
  - `/usr/local/Homebrew/Library/Homebrew/lock_file.rb` sha256
    `b5f51acdaa37930a4921125c83b1fb2d50316d248e165c500c1a46914c64a7bc`
  - `.../lock_file/formula_lock.rb` `f2cb0c663046a398e84ae40e5c7b9ca566a919b7604b39641e4e4efb18adc2c6`;
    `cask_lock.rb` `f7861afa845689a4f490ec4eb3c45033ea562550688e49afdfd92b43c96916cd`;
    `download_lock.rb` `8ea2ce13bb4c61cc4f89ff9fe4a02f7d181962bfc3c2abb1b8ea322016f571a3`
  - `.../utils/lock.sh` `be554cf5e6abac3fa3fde53c9e1f60bed1ece29992355e8ac3c70168a31dbc57`
  - `.../formula.rb` `5f09f104376dfe08235c261ed721da391ae47be1cba98a63923e4c7802dbf436`;
    `keg.rb` `891acd501baa7f21c1a08ca116ea0fcc555649b95822a5dde2fa43d9a5be279a`;
    `cleanup.rb` `e7abdfcdcac7b2c5f7b252f9fcf815f5733fa83508588b8f55f928cead351026`;
    `cmd/update.sh` `7fadbd6a0f062ddbb44a1cafd5b1664ef9d4d001bf1b6c0323f69590772b37eb`
  - `/opt/local/libexec/macports/lib/registry2.0/registry.tcl`
    `267720c882b6e7d1f00e63e529fc09fbc057e053da7d705b5a27cef182afee81`;
    `port1.0/portutil.tcl` `dea1e466b773a0440d8663e24c0cc3b6f68da6b68b8e277228226a5d2fe435ce`;
    `macports1.0/macports.tcl` `aa1d25941f3eec0e35a4584471ea7a94471bcdd9a7550978e694fa9e528b2c32`;
    `/opt/local/bin/port` `e04d53dc06ec84cd718cb40f6ba8867b2fa22c20ef9dbfa9c8203a5d6a915e49`
  - `man 2 fcntl`, `man 2 flock` as installed with macOS 26.7.1.
- **Blocking:** whether `analyze`/`doctor` may say anything about a provider
  being mid-mutation, and the refusal logic of a future mutating pass (INV-01,
  INV-02 neighbours). Sibling of R-18 (`R-18-advisory-file-locks-on-the-pinned-toolchain.md`,
  which covers Rust's own `flock` surface, not the providers').

**About the identifier.** Filed as R-17 in `docs/spec/09-research-checklist.md`
(between R-16 and R-18; R-18 was filed first).

## Question

Where does each provider hold a lock while it mutates? Is it a held advisory
lock (so the file existing proves nothing) or a presence file? Can MacPortsBrew,
as an ordinary user, with no process spawn (no `ps`, `pgrep`, `lsof`, no
provider command), tell "a provider is mutating right now", and a stale lock
from a live one? Does probing disturb the lock or create/alter the file?

## Evidence

### E1 — Homebrew `LockFile` (`lock_file.rb`, Homebrew 7.0.4)

- Path (line 26): `HOMEBREW_LOCKS/"#{lock_name}.#{type}.lock"`, `lock_name` the
  basename of the locked path. `HOMEBREW_LOCKS = HOMEBREW_PREFIX/"var/homebrew/locks"`
  (`startup/config.rb` line 55).
- Acquire (lines 32-70): `File.open(path, RDWR | CREAT)`, `FD_CLOEXEC`, then
  `flock(LOCK_EX | LOCK_NB)`; on failure `OperationInProgressError` (the command
  aborts, it does not wait). After locking it compares the open file's inode to
  the path's and retries if they differ (lines 47-56 comment).
- Release (lines 72-82): `@path.unlink if unlink` (default `false`), then
  `flock(LOCK_UN)` and close. `with_lock` calls `unlock` with no argument
  (lines 84-89): **the file is left behind after a normal release.**
- `FormulaLock` -> `HOMEBREW_CELLAR/<rack>`, type `formula`; `CaskLock` ->
  `HOMEBREW_PREFIX/Caskroom/<token>`, type `cask`; `DownloadLock` -> the
  temporary download path, type `download`, with `lock_or_wait` (waits up to
  180 s, `download_lock.rb`, `MAX_WAIT_SECONDS`).

Who takes which lock (grep of `Library/Homebrew`, excluding `test/`, `sorbet/`,
`vendor/`):

- `formula.rb` 2007-2024 `Formula#lock` -> `FormulaLock` for the formula and for
  each old name; `formula_installer.rb` 565 (`install` calls `lock`) and
  1949-1960 (locks the formula **and all its recursive dependencies**, held while
  `@hold_locks`). `upgrade` and `reinstall` reach this through the installer
  (not separately traced).
- `keg.rb` 399 `Keg#lock`, used by `uninstall.rb` 56 (`keg.lock do`).
- `migrator.rb` 528-533: both name locks.
- `cleanup.rb` 567 and 584, 809-823 `cleanup_lockfiles`: **does not hold a lock
  during cleanup**; for each lock file it opens `RDWR`, tries `flock(EX|NB)`,
  and **unlinks the file if it got the lock**. `cleanup.rb` 714
  takes `DownloadLock` per incomplete download and skips on
  `OperationInProgressError`.
- `download_strategy/curl_download_strategy.rb` 42: `DownloadLock` around a fetch.
- **`CaskLock` is referenced only in `cleanup.rb` 584; no cask install/uninstall
  path acquires it** in this checkout (grep above).
- `brew update`: `cmd/update.sh` 680 `lock update`; `cmd/vendor-install.sh` 382
  `lock "vendor-install <name>"`. `utils/lock.sh`: file
  `<prefix>/var/homebrew/locks/<command-with-dashes>` opened `exec 200>` (this
  **truncates**, and the fd is not close-on-exec, "shared to subprocess",
  comment), then `lockf -t 0` / `flock -n` / ruby / python `flock(LOCK_EX|LOCK_NB)`.
  Released when the shell exits. `update.sh` 742-744: "Git's fsmonitor prevents
  the release of our locks" (a child can keep an inherited lock alive).

### E2 — Homebrew lock files on this machine (observed 2026-10-05)

```
$ ls -ld /usr/local/var/homebrew /usr/local/var/homebrew/locks
drwxrwxr-x@ 5 pirado admin ... /usr/local/var/homebrew        (ls -la of the parent, abbreviated)
drwxr-xr-x@ 210 pirado admin 6720 Sep  6 21:30 locks
$ ls -la /usr/local/var/homebrew/locks | head -4
-rw-r--r--@ 1 pirado admin 0 Jul 31 13:00 ada-url.formula.lock
-rw-r--r--@ 1 pirado admin 0 May  1 14:55 aklomp-base64.formula.lock
-rw-r--r--@ 1 pirado admin 0 May 25 19:36 aom.formula.lock
$ ls /usr/local/var/homebrew/locks | grep -v 'formula.lock$'
update
vendor-install-ruby
```

208 files, all zero bytes, 206 `*.formula.lock` plus `update` and
`vendor-install-ruby` (both dated Sep 20 00:00), **no `*.cask.lock` and no
`*.download.lock`**, although `/usr/local/Caskroom` holds casks. The directory
and files are owned by the user, so a non-root process can open them. Files
with mtimes back to May 1 are plainly left behind after release (E1). The
`formula.lock` of `pcaudiolib` (Sep 6 21:30) matches the directory mtime.

All 208 were probed with `fcntl(F_GETLK)` (E6, which acquires nothing):

```
brew lock files probed: 208 held: 0
```

### E3 — MacPorts registry lock (`registry2.0/registry.tcl`, MacPorts 2.12.6)

Lines 389-416, `registry::exclusive_lock`:

```
set lockpath [::file join ${registry.path} registry .registry.lock]
if {![info exists lockfd]} {
    if {![::file writable [::file dirname $lockpath]]} {
        # skip locking, registry can't be modified anyway
        return
    }
    set lockfd [::open $lockpath w]
}
if {[catch {adv-flock $lockfd -exclusive -noblock} result]} {
    if {$result eq "EAGAIN"} {
        ui_msg "Waiting for lock on $lockpath"
        adv-flock $lockfd -exclusive
    } elseif {$result eq "EOPNOTSUPP"} { ... "not locking registry"
```

`exclusive_unlock` (lines 419-431) only does `adv-flock $lockfd -unlock`; it
never closes or removes the file, and `nlocked` counts nesting. Observations:

- Path: `${registry.path}/registry/.registry.lock`, i.e.
  `/opt/local/var/macports/registry/.registry.lock` here (`portdbpath` from
  `macports.conf`, R-16 E7; `registry.path` is `portdbpath`, R-16 E3).
- It is **a held `flock`, not a presence file**; `open w` truncates. It is
  **never removed** and is **skipped entirely when the registry directory is not
  writable by the caller**, so a non-root `port` takes no lock.
- Callers: `/opt/local/bin/port` `lock_reg_if_needed` (lines 4489-4502) locks
  for the actions `activate`, `deactivate`, `setrequested`, `unsetrequested`,
  `setunrequested`, `upgrade`, `uninstall`, `install`, and unlocks after the
  action (lines 4636-4640). `macports.tcl` 2973 / 3052 also locks around
  dependency resolution inside a build. **`sync`, `selfupdate`, `clean`,
  `fetch`, `build` and read actions are not in that list** (the action list was
  read; those actions' own locking beyond the statefile was not traced).
- Second lock, `port1.0/portutil.tcl` 1779-1885 `open_statefile`: opens
  `${workpath}/.macports.${subport}.state` `a+` and `adv-flock -exclusive
  -noblock` (waits with "Waiting for lock on $statefile"); called per target
  (`portutil.tcl` 1374, 2191). It lives in the port's build directory, and is
  not an install/uninstall lock.
- Observed on this machine (2026-10-05):

```
$ ls -la /opt/local/var/macports/registry     (trimmed)
drwxr-xr-x   9 root  wheel      288 Sep  7 14:33 .
-rw-r--r--@  1 root  wheel        0 Sep  7 14:29 .registry.lock
-rw-r--r--@  1 root  wheel  2322432 Sep  7 15:58 registry.db
-rw-r--r--@  1 root  wheel    32768 Sep  7 14:29 registry.db-shm
-rw-r--r--@  1 root  wheel        0 Sep  7 15:58 registry.db-wal
$ ls -la /opt/local/var/macports/build
-rw-r--r--   1 root  wheel    0 Aug 25 11:56 .turd_MacPorts      (no port build directory present)
```

  The lock file is `root:wheel 0644` in a `root:wheel 0755` directory: **readable
  by a non-root user, not writable**.

### E4 — Probe experiments (observed 2026-10-05, `python3` `fcntl`/`os`, scratch directory only)

Scripts in the session scratchpad (`r17/probe.py`, `r17/getlk.py`), not part of
the repository. A holder process opened a 0444 scratch file `O_RDONLY` and took
`flock` EX (or SH); the probe ran in another process, as the same non-root user.

```
A  probe = open(O_RDONLY) + flock(LOCK_SH|LOCK_NB), holder holds EX
   -> EWOULDBLOCK ; inode, mtime, size of the file unchanged
A2 same probe after the holder exited
   -> acquired (not held) ; file unchanged
E  open(O_RDONLY) of a path that does not exist
   -> ENOENT ; path still absent afterwards (nothing created)
B  probe holding LOCK_SH, a second process then tries LOCK_EX|LOCK_NB
   -> refused (errno 35)         <- the probe DISTURBS a non-blocking exclusive taker
C  F_GETLK on an O_RDONLY fd (struct flock, l_type probe = F_WRLCK):
   none held                     -> l_type=F_UNLCK, l_pid=0
   holder flock EX               -> l_type=F_WRLCK, l_pid=-1   (probe F_RDLCK: F_WRLCK too)
   holder flock SH               -> l_type=F_RDLCK, l_pid=-1   (probe F_RDLCK: F_UNLCK)
   after each F_GETLK the holder's lock was intact (a third EX|NB was refused)
```

Real files, the only two probes on a provider lock (2026-10-05, as `pirado`):

```
$ stat -f '%i %m %z' /opt/local/var/macports/registry/.registry.lock   (before and after)
275047081 1788802176 0
flock(SH|NB) on an O_RDONLY fd   -> not held (acquired SH, released)
fcntl(F_GETLK, F_WRLCK)          -> l_type=F_UNLCK, l_pid=0
$ python3 -c "os.access('/opt/local/var/macports/registry', os.W_OK)"  -> False
```

The MacPorts probe was a momentary `SH` hold with nothing running; had
`port install` started in that instant it would have printed "Waiting for lock"
and blocked (E3), not failed. **No `flock` probe was run on a real Homebrew lock
file**, because a brew `EX|NB` taker would abort (B); the 208 Homebrew files
were probed with `F_GETLK` only.

### E5 — The documentation

- `man 2 flock` (macOS 26.7.1, read 2026-10-05): a lock is a property of the open
  file; `dup`/`fork` copies share one lock ("If a process holding a lock on a file
  forks and the child explicitly unlocks the file, the parent will lose its lock").
  It says nothing about `fcntl(F_GETLK)` seeing `flock` locks.
- `man 2 fcntl`: `F_GETLK` "Get the first lock that blocks the lock description
  ... If no lock is found ... the lock type is set to `F_UNLCK`"; `l_pid` "is
  only used with F_GETLK to return the process ID of the process holding a
  blocking lock". It does not say that `flock` locks are reported or that the
  pid is -1 for them; **that is observation C only.**
- `docs.brew.sh`: **not read.** A local grep of `/usr/local/Homebrew/docs` for
  `var/homebrew/locks`, `flock`, `LockFile` returned no file. No official page
  describing either provider's locks was found.

## Findings

Evidence first (E1-E5); these are the conclusions, kept separate.

1. **Homebrew locks are held advisory `flock`s, not presence files, and the
   file survives release.** (E1.) A file in `var/homebrew/locks` proves nothing
   (E2: 208 files, 0 held). Which operations take one: install and upgrade
   (formula and its recursive dependencies), uninstall, migration, curl
   download, and, via the shell helper, `brew update` (`update`) and
   `vendor-install <name>`. **Cleanup holds none while it works, and cask
   operations hold none** (E1), so "a Homebrew cask install is running" is
   invisible to any lock.
2. **MacPorts holds a held `flock` on `<portdbpath>/registry/.registry.lock`**
   for `install`, `upgrade`, `uninstall`, `activate`, `deactivate` and the three
   requested-flag actions; the file is created by the first locking run and
   never removed (E3). **Only an elevated (registry-writable) `port` locks at
   all.** `sync`/`selfupdate`/`clean` take no registry lock.
3. **A `flock` has no stale state.** The kernel drops it when the last holder of
   the open file description exits (observed with SIGKILL in R-18; the same
   mechanism). A file with no lock is simply unlocked; a file whose lock is held
   has a live holder. So "stale lock vs live" reduces to "held vs not held",
   and presence of the file is irrelevant to both providers.
   The one way a held lock can outlive the operation you care about: an
   inherited descriptor in a child (`utils/lock.sh` fd 200, `update.sh` 742-744
   comment, `man 2 flock` fork text).
4. **A filesystem read alone can answer "is this lock file flock-held right
   now?"**, with no process spawn, by `fcntl(F_GETLK)` on an `O_RDONLY` fd
   (observation C). It acquires nothing, creates nothing (E: `ENOENT` with no
   `O_CREAT`), alters nothing (inode/mtime/size unchanged), and did not disturb
   the holder. It works on a `0444`/`root:wheel 0644` file, so the MacPorts lock
   is probeable by a non-root user. It returns no holder identity (`l_pid=-1`).
5. **The `flock(LOCK_SH|LOCK_NB)` probe (the `File::try_lock_shared` shape of
   R-18) is not safe for Homebrew.** While the probe holds `SH`, a Homebrew
   `EX|NB` taker is refused (B) and Homebrew reacts by aborting with
   `OperationInProgressError`. The window is microseconds, but the effect would
   be a spurious failure of the user's `brew install`. For MacPorts the same
   collision costs a "Waiting for lock" message and a blocking retry (E3).
6. **Probing never creates the file** if opened without create (`O_RDONLY`, no
   `O_CREAT`; E). Opening with `O_CREAT` or `O_WRONLY|O_TRUNC` would: Homebrew
   itself creates (`RDWR|CREAT`), MacPorts and `lock.sh` truncate on open.

### False positives and false negatives of a filesystem-only detector

Detector D: enumerate `<prefix>/var/homebrew/locks/*` and
`<portdbpath>/registry/.registry.lock`; report "held" when `F_GETLK` returns
`F_RDLCK`/`F_WRLCK`.

- **False positives (D says mutating, nothing is):** (a) a lock inherited by an
  orphaned child that outlived its parent (E1, `update.sh` fsmonitor comment);
  (b) a held lock that is not a mutation: a `DownloadLock` wait, or a MacPorts
  `registry::exclusive_lock` taken for an `upgrade --dry-run`-class action or
  during dependency resolution (not traced which actions then mutate nothing);
  (c) another tool, or MacPortsBrew's own earlier run, holding the same file;
  (d) a `SH` held by a probe elsewhere. **Never** a stale file: presence alone
  is not used.
- **False negatives (D says quiet, a provider is mutating):** (a) MacPorts
  `sync`, `selfupdate`, `clean`, and any non-root `port` run (no registry lock,
  E3); (b) Homebrew cask install/uninstall (no `CaskLock`, E1) and any `brew`
  step that does not lock; (c) Homebrew work between locks (the lock is taken
  per formula; a plan with several formulae has instants with none held);
  (d) **a provider whose lock directory/file does not exist yet** (never ran
  as that user): `ENOENT`, which says "no lock observed", not "idle";
  (e) the prefix or `portdbpath` is non-default (the MacPorts path is
  configuration, R-16 E3; a user `portdbpath` moves it); (f) MacPorts' registry
  is also protected by SQLite's own locking on `registry.db` (`-wal`/`-shm`
  present, E3), which this note did not investigate; (g) any operation by an
  unrelated tool (`pip`, `npm`, Finder) touching a provider's files.
- **Not distinguishable:** which provider command holds a lock; whose; and
  whether it will mutate or has finished mutating.

## Recommendation

A detector **can** be built from filesystem reads alone, but only as a
*positive-evidence* detector. The honest claims, in the vocabulary of
`CapabilityState` (never `Unknown` reported as `Available`, INV adapters rule):

| Observation | May be claimed | Must not be claimed |
|---|---|---|
| `F_GETLK` reports `F_WRLCK`/`F_RDLCK` on a Homebrew lock file or `.registry.lock` | "a lock on `<path>` is held by some process at this instant" (provider, file name, which operations take it) | that a mutation is in progress, or by what |
| `F_GETLK` reports `F_UNLCK` on every file that exists | "no held lock observed at this instant" | "idle", "safe to proceed" |
| the lock file or its directory is absent | "no lock file exists" | anything about activity |
| the file exists but `open` fails (`EACCES`, `ENOENT` race) or `F_GETLK` errors (`EOPNOTSUPP` on an unusual volume, R-18 left it open) | **unknown**, the fallback | any boolean |
| a file exists, unlocked | nothing (leftover) | "stale lock", "stuck", "crashed" |

Rules that follow: (1) use `F_GETLK`, not `flock(SH|NB)`, so a Homebrew taker is
never refused by the probe (finding 5); (2) open `O_RDONLY`, no create, no
truncate; (3) never unlink a lock file (R-18, and E1's inode race); (4) treat
"not held" as one sample, not a guarantee, so a mutating pass must still take
its own protection: **the executor's real guard is to attempt the provider's own
lock-taking command and report its refusal**, not to pre-check; (5) do not offer
a "stale lock" notion at all, it does not exist for `flock` (finding 3).
`analyze` works offline: this probe reads only local files, no network, no
spawn. Whether `F_GETLK` is reachable from the workspace's dependency set
(std exposes `File::try_lock*` but, to my knowledge, no `F_GETLK` wrapper; **not
verified here**) is a design question for the adapter's dependency policy.

## Cannot determine

- **`docs.brew.sh` / `guide.macports.org` statements on locks:** not read; no
  official page was located. **UNRESOLVED**; a future attempt needs a search of
  both sites and the `macports-base` repository history for `.registry.lock`.
- **That `F_GETLK` reports `flock` locks on other macOS versions or on
  non-APFS volumes** (NFS, SMB, exFAT): observed only on 26.7.1 / Darwin 25.6.0,
  x86_64, on the volume holding the scratch directory and `/opt/local`; the
  `man` pages are silent (E5). The `l_pid=-1` value is observed, undocumented.
- **A held lock from a real provider operation was never observed.** The
  Homebrew and MacPorts held-lock behaviour is established from source (E1, E3)
  and from scratch-file experiments (E4); starting `brew install` or
  `port install` is a mutation and was not done. Whether Homebrew's Ruby
  `File#flock` and `lockf`/`flock(1)` locks both appear to `F_GETLK` as
  observed for Python's `flock` is therefore **UNRESOLVED**; a future attempt
  needs a recorded run of a mutating provider on a throwaway prefix.
- **Which MacPorts actions hold the registry lock for what fraction of their
  run, and the exact build-directory path of the statefile** (`workpath`
  derivation was not read). Whether a long `install` holds `.registry.lock`
  across the whole build or releases it around the build was not traced
  (`macports.tcl` 2973-3052 and the `lock_reg_if_needed` unlock at 4636-4640
  both exist; their interaction was not followed).
- **SQLite locking on `registry.db`** (the `-wal`/`-shm` files): not examined.
- **`brew upgrade` / `reinstall` paths** were taken to reach `FormulaInstaller#lock`
  (E1) without being traced individually.
- **Whether Homebrew on Apple Silicon (`/opt/homebrew`) or a non-default
  `HOMEBREW_PREFIX`/`HOMEBREW_LOCKS` differs**: only the Intel layout was read.

## Impact on the specification

- Nothing in `docs/spec/` was edited. No statement found that contradicts this
  note; a search of the specification for lock-based detection was not done
  beyond the checklist entry.
- **Bears on R-18's reading:** R-18 establishes `File::try_lock_shared` as
  available. This note finds that the shared-lock probe form it enables is
  *unsafe against Homebrew* (finding 5); the safe form is `F_GETLK`. The main
  agent should decide whether that belongs in `docs/spec-evolution.md`.
- **Bears on any "provider busy" capability:** only the positive claim in the
  Recommendation table is supportable; "idle" and "stale lock" are not.
- **Bears on the MacPorts adapter's read assumptions:** a non-root `port` read
  takes no registry lock (E3), so a read can run concurrently with an elevated
  mutation; a snapshot taken during one may be inconsistent. The snapshot
  records no such fact today (not checked in code).

## Re-check when

- Homebrew moves past 7.0.4 (`lock_file.rb`, `utils/lock.sh`, `CaskLock`
  becoming used) or changes `HOMEBREW_LOCKS`.
- MacPorts moves past 2.12.6 (`registry.tcl` `exclusive_lock`, the
  `lock_reg_if_needed` action list, a new lock file, `sync`/`selfupdate` locking).
- macOS changes the `flock`/`fcntl` interplay (re-run the C observation), or the
  project targets a volume class other than the one observed.
