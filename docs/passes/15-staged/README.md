<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 15 — staged files

Nothing here is executed by a gate or a workflow. `.github/workflows/` and
`scripts/` are the only places those run from.

| Staged | Applies to | How |
|---|---|---|
| [`dp034-security-stdin.yml`](dp034-security-stdin.yml) | `.github/workflows/dp034-security-stdin.yml`, **on a throwaway branch only** | SHA-256 `adb1d736a4fe31b1d9562f32e7921990e8b9cbd6d23db090d5e44d4e133aec92` |

## DP-034 research: does `security -i` keep the passwords out of argv?

Ruled 2026-10-23 (SE-104): research (b) first; if it does not hold, (a). This
workflow is that research. It handles **no secret**: it makes a self-signed
certificate, three random passwords (masked before they are assigned) and a
throwaway keychain inside `$RUNNER_TEMP` on a hosted VM, with `permissions: {}`,
no checkout, no environment and no third-party action.

### Status of the review

- `actionlint` is clean; the staged `check-release-workflow.sh` accepts it beside
  the real workflows; it has no `uses:`, so `check-action-pins.sh` has nothing to say.
- The script's mechanics were run on a developer machine against a **fake**
  `security` that touches no keychain: the control that the process sampler can
  see a secret in argv reads 1, the FIFO hold keeps a reader alive and sees it
  exit at EOF, the heredoc and the four quoting cases parse, and no output file
  holds a secret. Those values are the fake's; they say nothing about the real tool.
- **An independent `security-reviewer` read was requested and the permission
  classifier declined the call; it has not happened.** Do it before arming (ask it to read
  the workflow for: leaks and reach, writes outside \`\$RUNNER_TEMP\`, whether Q1–Q6
  measure what the header claims, gaps, and conflicts with the repository's gates),
  or accept the self-review below.
- Self-review: (1) `create-keychain` adds each keychain to the user's search
  list; they are deleted at the end and the VM is destroyed, so nothing persists.
  (2) Q6 (`create-keychain` with no `-p`) may raise a GUI prompt that nobody
  answers; `perl alarm` ends it after 25 s and the runner is destroyed.
  (3) **Not tested:** the passwords as environment variables (`WRAP_PW`, `SRC_PW`
  are exported to `openssl` exactly as in the real step; `ps eww` as the same
  user would show them), a password beginning with `-`, CRLF, very long lines.
  (4) A same-repository pull request can read repository secrets in general; this
  workflow names none and has a token with no scope.

### To run it (the owner's two acts)

1. **Arm.** Send one message starting with `MPB_ALLOW_GUARD_EDIT=1` asking for this
   file to be written to `.github/workflows/dp034-security-stdin.yml` on a new branch
   `research/dp034-security-stdin` cut from `main`, with nothing else on it.
2. **Allow the run.** The pass-15 work order forbids running workflows; say so in
   words in that message or the next. The branch is pushed and a pull request is
   opened (never merged); the workflow's `paths` filter makes it run once, on four
   runners, in a few minutes.
3. Afterwards: the lines are read from the job logs (`gh run view <id> --log`),
   pasted into `docs/research/R-19-release-toolchain.md` as a dated addendum, the
   pull request is **closed unmerged**, and the branch is deleted (by you: deleting a
   remote branch is yours, SE-099).

### How to read the result

Each runner prints `key=value` lines (and a table in the job summary). (b) holds
for a runner when **all** of these are true on it:

- `Q3.method_sees_a_secret_in_argv_when_present` is 1 or more (the sampler works);
- `C.identity_listed=1` and `C.codesign_rc=0`, as for the control `B.*` (the stdin
  flow produced a keychain that signs);
- `C.password_is_the_intended_one_rc=0` and the Q4 lines the recipe needs (the
  password the keychain has is the one meant; use hex, which needs no quoting);
- `C.argv_secret_seen_by_sampler=no`, `Q3.security_i_alive_argv_has_a_secret=0`
  and `any_process_argv_has_a_secret=0`;
- `C.stdout_contains_a_secret=no`, `Q2.of_those_containing_a_secret=0`,
  `final.any_secret_in_results_or_outputs=no`.

`Q5` decides how the signing step must be written, not whether (b) holds: if
`security -i` exits 0 when a command fails (the fake does, to keep the script's
shape; the real answer is the point of Q5), the step **cannot** rely on `set -e` and
must check the keychain's state after the pipe (`find-identity`, then a test sign).
If (b) fails on any of the four runners, the answer is (a), with the deviation
written into ADR-0005.

### R-19 addendum template

> **DP-034 observation, <date>** — runners <list>, macOS <versions>. `security -i`
> <accepts / does not accept> the five subcommands on stdin; echo <none / …>; history
> files <none / …>; the password was <absent / present> in argv samples while
> <the process lived / …>; exit status on a failing command <…>. Source: workflow run
> <id>, pull request <number> (closed unmerged). Result: DP-034 <closes as (b) / falls
> back to (a)>.
