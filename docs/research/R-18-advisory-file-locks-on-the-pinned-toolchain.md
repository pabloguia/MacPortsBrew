# R-18 — Are `File::try_lock` and friends available on the pinned toolchain, and how do they behave on macOS?

- **Status:** answered (residual: network volumes, see "Cannot determine")
- **Answered on:** 2026-10-05
- **Machine:** macOS 26.7.1 (25G241), x86_64. Every mounted volume is APFS.
  No provider command was run, nothing was installed, no dependency was added.
- **Toolchain:** `rust-toolchain.toml` pins `channel = "1.98.0"` (read with the
  Read tool, 2026-10-05). Observed: `rustc --version` gives
  `rustc 1.98.0 (88d9e12ae 2026-08-18)`; `cargo --version` gives
  `cargo 1.98.0 (797e8a9bc 2026-08-05)`.
- **Sources read:**
  - <https://doc.rust-lang.org/1.98.0/std/fs/struct.File.html>, read 2026-10-05
    (stability lines and "Platform-specific behavior" of the five methods).
  - <https://doc.rust-lang.org/1.98.0/std/fs/enum.TryLockError.html>, read 2026-10-05.
  - <https://raw.githubusercontent.com/rust-lang/rust/1.98.0/library/std/src/sys/fs/unix.rs>
    (the `1.98.0` tag of the Rust repository), read 2026-10-05.
  - `man flock` on this machine, read 2026-10-05 (`man -w flock` is the Xcode
    SDK copy, `.../MacOSX.sdk/usr/share/man/man2/flock.2`, BSD 4.2, December 1993).
- **Blocking:** any design that serialises mutating runs, or the history store,
  with an advisory lock file, and the "released when the process dies"
  property such a design would rely on.
- **Scratch:** all programs and files lived in the session scratchpad
  `.../scratchpad/r18/`, outside the repository.

## Question

Is `std::fs::File::lock`/`lock_shared`/`try_lock`/`try_lock_shared`/`unlock`
available on the pinned toolchain, what is the shape of `TryLockError`, and
what do they do on macOS: contention, release on process death, inheritance,
network volumes, creation mode and umask, the unlink race, and probing?

## Evidence

### E1 — Stabilisation and error type (std documentation, 1.98.0)

On the `File` page every one of `lock`, `lock_shared`, `try_lock`,
`try_lock_shared`, `unlock` carries the stability line **1.89.0**. The
`TryLockError` page, also **1.89.0**:

```
pub enum TryLockError {
    Error(Error),
    WouldBlock,
}
```
`Error(Error)`: "The lock could not be acquired due to an I/O error on the
file. The standard library will not return an `ErrorKind::WouldBlock` error
inside `TryLockError::Error`". `WouldBlock`: "The lock could not be acquired
at this time because it is held by another handle/process."

Platform text for `try_lock`: "This function currently corresponds to the
`flock` function on Unix with the `LOCK_EX` and `LOCK_NB` flags ... Note that
this may change in the future." The docs also state that locks are released
when the file (and any duplicated descriptors) is closed, and that calling a
lock method while the handle or a clone already holds one may deadlock
(unspecified).

The 1.89.0 release-notes entry was **not** read: the fetch of `RELEASES.md`
was truncated before the stabilised-APIs list (it did report the release date
as 2025-08-07). The stability lines above are the citation.

### E2 — The mapping on Apple targets (`library/std/src/sys/fs/unix.rs`, tag `1.98.0`)

Each method is one `cfg_select!` whose first arm lists
`any(target_os = "freebsd", ..., "android", target_vendor = "apple")`, and
calls, respectively:

```
cvt(unsafe { libc::flock(self.as_raw_fd(), libc::LOCK_EX) })               // lock
cvt(unsafe { libc::flock(self.as_raw_fd(), libc::LOCK_SH) })               // lock_shared
libc::flock(self.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB)               // try_lock
libc::flock(self.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB)               // try_lock_shared
cvt(unsafe { libc::flock(self.as_raw_fd(), libc::LOCK_UN) })               // unlock
```
`try_lock`/`try_lock_shared`: an error whose `kind()` is `WouldBlock` becomes
`TryLockError::WouldBlock`; any other error becomes `TryLockError::Error(err)`.
The fallback arm returns `ErrorKind::Unsupported` (so `try_lock` yields
`TryLockError::Error(Unsupported)` on a target outside the list). The fetch
tool returned the code without line numbers; none are cited.

### E3 — `man flock` on this machine (read 2026-10-05)

> Locks are on files, not file descriptors. That is, file descriptors
> duplicated through dup(2) or fork(2) do not result in multiple instances of a
> lock, but rather multiple references to a single lock. If a process holding a
> lock on a file forks and the child explicitly unlocks the file, the parent
> will lose its lock.

> Advisory locks allow cooperating processes to perform consistent operations
> on files, but do not guarantee consistency ...

> A shared lock may be upgraded to an exclusive lock, and vice versa, simply by
> specifying the appropriate lock type; this results in the previous lock being
> released and the new lock applied (possibly after other processes have gained
> and released the lock).

ERRORS: `EWOULDBLOCK` (locked, `LOCK_NB`), `EBADF`, `EINVAL` (not a file),
`ENOTSUP` ("not of the correct type"). **The page has no BUGS section and says
nothing about NFS, SMB, exFAT or any network or non-APFS file system.**

### E4 — Observed: one program, same process and two processes (2026-10-05)

Throwaway `t.rs` in the scratchpad, `rustc --edition 2021 t.rs -o t` (rustc
1.98.0, no warnings, so `File::try_lock`, `try_lock_shared`, `unlock` and
`TryLockError::{WouldBlock, Error}` resolve on the pinned version). Files are
opened with `OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600)`.
Run as `./t $PWD/lockfile` with umask 022:

```
mode 600
f1 try_lock: Ok
f2 try_lock (f1 holds): Err(WouldBlock)
f2 try_lock_shared (f1 holds excl): Err(WouldBlock)
f2 try_lock after drop(f1): Ok
f2 unlocked
h try_lock: Ok
probe try_lock_shared while excl held: Err(WouldBlock)
h try_lock_shared: Ok
probe try_lock_shared while shared held: Ok
excl while h holds shared: Err(WouldBlock)
excl after h unlock: Ok
parent try_lock: Ok
child while parent holds: child try_lock: Err(WouldBlock)
second child while first child holds: child try_lock: Err(WouldBlock)
after SIGKILL of holder: child try_lock: Ok
u1 lock: Ok
u2 lock after unlink+recreate: Ok
inode u1=298617111 u2=298617117
```

`f1` and `f2` are two separate `open` calls on one path in one process, so two
open file descriptions: the second is refused while the first is alive and
succeeds once the first is dropped. "First child" is a spawned `Command`
holding the lock and sleeping; it was ended with `Child::kill()` then `wait()`
(this is SIGKILL on Unix by std's documented behaviour; the signal number was
not independently observed, and the Rust docs for it were not read in this
note). The next process acquired the lock immediately: the lock did not outlive
its holder, and no unlock or file removal was needed.

### E5 — Observed: creation mode and umask (2026-10-05)

Same program, `mode(0o600)`, first line of output and `ls -l`:

| umask | resulting mode of the new file |
|---|---|
| 022 | `-rw-------` (600) |
| 077 | `-rw-------` (600) |
| 000 | `-rw-------` (600) |
| 0277 | `-r--------` (400); the program's second `open(read+write)` then failed `Os { code: 13, kind: PermissionDenied }` |

### E6 — Volumes on this machine (observed 2026-10-05)

```
$ mount | awk '{print $1, $3, $4}' | grep -v "^/dev/disk.*/System" | head -20
/dev/disk1s4s1 / (apfs,
devfs /dev (devfs,
map on /System/Volumes/Data/home
/dev/disk3s1 /Volumes/<volume> (apfs,
...  /dev/disk3s2 /Volumes/<volume> (apfs,
```
`mount | grep -iE "nfs|smb|exfat|afp|webdav|fuse"` printed nothing. No network
or non-APFS volume was available, so no lock was attempted on one.

## Findings

1. **Available, since 1.89.0.** The pinned 1.98.0 has all five methods, and
   `TryLockError` is a two-variant enum, `Error(io::Error)` and `WouldBlock`
   (E1, compiled in E4). A match must name both. Std guarantees that
   `Error` never carries a `WouldBlock` kind, so "contention" and "failure to
   lock" are distinguishable by variant alone.
2. **On macOS this is `flock(2)`** (E2), not `fcntl` record locking. The
   documentation says "may change in the future" (E1); the mapping is
   therefore a property of the 1.98.0 tag, to be re-read on a toolchain bump.
3. **Contention is per open file description, not per process** (E4): two
   `open`s of one path in one process conflict. That is the property that makes
   a lock test possible inside a single test process, and it differs from
   `fcntl` (POSIX) locks, which are per process (see Cannot determine; this
   contrast is not sourced from Apple's documentation).
4. **Released on process death** (E4, SIGKILL): the lock belongs to the open
   file description and the kernel closes it with the process. A crashed run
   leaves no stale lock to clean up, unlike a pidfile.
5. **Inheritance** (E3, cited): a descriptor duplicated by `dup` or `fork` is a
   reference to the same lock. The lock persists until the **last** reference
   is closed, and a child's explicit `unlock` releases the parent's lock.
   Consequence for a lock held across a spawned subprocess: a child that
   inherits the descriptor keeps the lock alive after the parent exits. Rust's
   `File`/`OpenOptions` open with close-on-exec on std's Unix targets, which
   was **not** verified in this note; it is stated here as a hypothesis, not a
   finding (see Cannot determine).
6. **Network volumes: UNRESOLVED.** `man flock` is silent (E3) and this
   machine has no such volume (E6). Nothing here says whether `flock` on an
   NFS/SMB/exFAT-backed directory errors (`ENOTSUP` is the only documented
   candidate), silently succeeds locally, or works.
7. **Creation mode.** `mode(0o600)` is subject to umask as `open(2)` always
   is: a restrictive umask can only remove bits (E5). Umasks 000-077 leave 600;
   one that clears the owner bits (0277) produces a file the same user can no
   longer open for writing, so the second open fails with `PermissionDenied`.
   A robust opener treats `PermissionDenied` as an error distinct from
   `WouldBlock`; setting permissions after creation is a second step whose
   necessity was not tested.
8. **Never unlink the lock file.** Observed (E4): a holder `u1` on inode
   298617111; the path is removed; a second opener recreates the path and
   locks it at once because it is a **different inode** (298617117). Two
   processes each believe they hold "the" lock. The race: A opens and locks
   the file; B opens the same path; A unlinks and exits; C creates a new file at
   the path and locks it; B (which opened the old inode) also acquires, since
   A's lock died with A. The sequence that produced it here was single
   threaded; the interleaving above is the general case and was not run with
   three processes. A lock file should be created once and persist.
9. **Probing without disturbing the holder.** `try_lock_shared` against an
   exclusive holder returns `WouldBlock` and changes nothing (E4). Against a
   shared holder it succeeds. Two caveats from E4/E3: (a) a successful probe
   holds a shared lock until `unlock` or drop, during which an exclusive
   `try_lock` by anyone else gets `WouldBlock` ("excl while h holds shared"),
   so a probe can cause a false "busy" for a concurrent acquirer; (b) a
   shared probe cannot distinguish "free" from "held shared". The
   `unlock` after a successful probe was exercised (`pr.unlock()` then the
   exclusive acquisition succeeded after the other holder released). Prefer
   probing with `try_lock` only when the caller would take the lock anyway.
   Changing a lock's kind on one description is release-then-apply, not
   atomic (E3), so "upgrade" through `lock` after `lock_shared` is not an
   atomic upgrade.

## Cannot determine

- **Behaviour on NFS, SMB, exFAT, WebDAV or any non-APFS volume** (finding 6).
  A future attempt needs a mounted network share and the same `t.rs`; if no
  share is available, the safe design is runtime detection: attempt
  `try_lock`, treat `TryLockError::Error` as "locking unavailable" (not as
  "free"), and never fall back to an unlocked run silently.
- **Close-on-exec of descriptors opened by `OpenOptions`** (finding 5). Needs
  the std source for `File::open` at the 1.98.0 tag (`O_CLOEXEC`) or a test
  that spawns a grandchild and kills the parent.
- **`fcntl` vs `flock` differences** were not cited from Apple documentation;
  `man 2 fcntl` on this machine was grepped but not read for lock semantics.
- **The 1.89.0 release-notes entry** for these APIs (E1).
- Whether a file system returns `ENOTSUP`/`EINVAL` for `flock` on a directory
  that is not a regular-file volume was not exercised.

## Impact on the specification

- **Nothing in `docs/spec/` was found to contradict** these findings; none of the
  specification was read for this note beyond the checklist file.
- A design that depends on a lock file may state: advisory `flock` through
  `std::fs::File::try_lock` is available on the pinned 1.98.0; contention
  is `TryLockError::WouldBlock`; the lock dies with its holder; the lock file is
  created `0600` and never removed. It must also state that network-volume
  behaviour is UNRESOLVED and handled by runtime detection (a
  `TryLockError::Error` is "locking unavailable", never "free").

## Re-check when

- `rust-toolchain.toml` moves past 1.98.0: re-read `sys/fs/unix.rs` at the new
  tag, since std documents the `flock` mapping as changeable.
- The lock file's directory is chosen: if it can be a network volume, finding 6
  must be answered first.
- A lock is held across a spawned provider process (finding 5).
