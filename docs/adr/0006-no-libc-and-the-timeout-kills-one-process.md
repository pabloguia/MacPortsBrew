# ADR-0006 — No `libc`, and the timeout kills one process rather than a group

- **Status:** accepted
- **Date:** 2026-09-04
- **Pass:** 02-ARCHITECTURE-REVIEW

## Context

The executor gives every provider invocation its own process group, so a Ctrl-C
aimed at this process's foreground group does not also reach a provider read
mid-flight. When the timeout fires it kills the child — one process, not the
group.

A child that has spawned its own children therefore leaves them running, and
they hold the pipe this process is reading. `READER_DRAIN_TIMEOUT` bounds that:
the reads stop after six seconds each rather than waiting for a writer that may
never close (SE-057).

Killing the group instead means signalling `-pid`, and `std` exposes no way to
do it. That needs `libc`, which ADR-0004's dependency gate governs — every
dependency is weighed against licence, deployment floor and build scripts, and
`libc` is a build-script crate.

The alternative to a dependency is spawning a kill utility, which would be a
second subprocess path with no `CommandSpec` behind it: no sanitised environment
(INV-10), no timeout, no recording, and outside the allowlist the one spawn site
enforces. `04-migration-and-safety.md` ("Process execution") asks for one
executor precisely so those are decided in one place, and `check-layering.sh`
would not catch it, because the executor module is the one place its grep
exempts.

This was previously recorded only as a comment in `process/system.rs`. Finding
F-20 is that a dependency decision does not live in a code comment, which is
right: a comment is read by whoever is already in the file, and a dependency
choice needs to be visible to whoever is deciding the next one.

## Options considered

**Add `libc` and kill the group.** Correct signal semantics: a grandchild
holding the pipe dies with its parent, and the drain timeout becomes a
belt-and-braces rather than the mechanism. Costs a dependency with a build
script, which ADR-0004 treats as the category needing most scrutiny, and it is
the first `unsafe`-adjacent surface in a workspace that sets
`unsafe_code = "forbid"`. Makes signal handling generally available, which is
what a future `migrate` command will want when a user interrupts a run.

**Spawn a kill utility.** No dependency. Forecloses the single-spawn-site
property that three separate controls now rest on — the allowlist, the sanitised
environment and the recording — for the sake of avoiding one crate. Rejected on
those grounds rather than on cost.

**Bound the reads and kill one process.** No dependency, no second spawn path.
A grandchild survives its parent and is reaped by the system when the terminal
session ends; the tool stops waiting on it. The cost is that a runaway
grandchild of a provider read keeps running until then, and this build performs
only reads, so nothing it spawns is expected to fork.

## Decision

No `libc`. The timeout kills the child; `READER_DRAIN_TIMEOUT` bounds the reads
so a surviving grandchild cannot hang the runner.

## Consequences

The workspace stays free of build-script dependencies and of `unsafe`. The
executor keeps exactly one spawn site, which is what makes the program allowlist
and the sanitised environment enforceable rather than conventional.

Against that: a provider read that forks leaves the fork running. Six seconds
per stream, two streams, is the worst case this tool waits — measured, and
asserted by `a_grandchild_holding_the_pipe_does_not_hang_the_runner`.

What becomes expensive later: an executor that runs *mutating* commands has a
stronger claim on group kill, because an interrupted `port install` leaving
children behind is a different proposition from an interrupted `port installed`.
Reversing this decision then is cheap in code and re-opens ADR-0004's gate,
which is the point of recording it now rather than discovering the reasoning in
a comment.

## Compatibility impact

None. No dependency added, no deployment floor moved, no provider behaviour
assumed.

## Revisit when

The executor pass lands a mutating command path — that is the change of
circumstance, not a change of opinion. Also if `std` ever exposes process-group
signalling, which would make this a decision about nothing.
