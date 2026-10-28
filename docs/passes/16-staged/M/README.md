<!-- SPDX-License-Identifier: Apache-2.0 -->

# Staged for the owner: hardened Claude workflows (work order 16, part M)

**Ruling:** DP-039 (b), confirmed 2026-10-27 (SE-109). **Patch:** `M.patch`, one
change to four guarded files. It applies to `main` at `0564fc3`.

## Why

`claude.yml` and `claude-code-review.yml` hold an API key and load Claude Code's
settings and hooks from the checked-out tree. A pull request that changes them,
run by the workflow, would run its own hooks with the key (DP-039). Both are
manual-only today (SE-099) and no key is set. This change hardens them before
any key is set or any automatic trigger returns, on the ruling's five points:

1. **Same repository only.** The gate job, which holds no secret, refuses a pull
   request whose head repository is not this one, and refuses every event except
   `workflow_dispatch` (on a `pull_request` event the workflow file itself would
   come from the pull request).
2. **Configuration only from the base.** The workspace root is the base commit
   from the default branch, the head goes into `work/`, and everything in `work/`
   that Claude Code could read as configuration (`.claude`, `CLAUDE.md`,
   `.mcp.json`, symbolic links, ...) is deleted before the action runs.
   `--setting-sources user` and a settings file derived from the base's
   `.claude/settings.json` (deny rules and hooks kept, allow rules dropped).
3. **`OWNER` or `MEMBER` only:** the dispatcher, the re-runner and the pull
   request's author.
4. **No `Write`/`Edit`; `Bash` by allowlist:** `git status`, `git log`,
   `git show` only, with `git diff` and every git form that reads outside the
   repository or the network denied; subprocess environment scrubbing with the
   action's own process isolation; `contents: read` as the hard bound.
5. **The key in an environment with a required reviewer:** `environment: claude`
   on the one job that runs Claude, after the gate.

`scripts/check-release-workflow.sh` gains rule 11 (the `claude` environment and
the key appear only there) and case-insensitive, plain-spelling hardening of
rules 4 and 6. `scripts/test-gates.sh` gains a 126-check case for the two
workflows and 20 `wf_case` cases for the new rules.

Reviewed three times by `security-reviewer` (final verdict: approve, on this
patch's text). The workflow files passed `actionlint` and the workflow case
passed 126/126. **The two script hunks were written as patch text and have not
been executed**: the session may not edit even a scratch copy of a guarded
script, so running them is the first step below.

## What the owner does

From a clean checkout of `main`, on a topic branch:

```bash
git apply docs/passes/16-staged/M/M.patch
```

```bash
scripts/test-gates.sh
```

```bash
scripts/check-release-workflow.sh
```

```bash
scripts/validate.sh
```

If all pass, commit and open the pull request as usual. If one fails, do not
edit around it: revert with `git apply --reverse docs/passes/16-staged/M/M.patch`
and tell a session what failed.

To have a session apply it instead, send it a message containing exactly:

> MPB_ALLOW_GUARD_EDIT=1 Apply docs/passes/16-staged/M/M.patch on a topic
> branch, run the checks its README lists, and open a pull request.

**Repository settings, which only the owner can make** (the patch cannot check
them):

1. Create the `claude` environment with yourself as required reviewer; leave
   "Prevent self-review" off while you are the only reviewer. Required reviewers
   need a public repository on the Free, Pro and Team plans.
2. Its deployment branches: **selected branches, `main` only**, no tag pattern.
3. Store `ANTHROPIC_API_KEY` as an **environment** secret only, and confirm
   `gh api repos/pabloguia/MacPortsBrew/actions/secrets` does not list it.
4. On the first dispatched run you approve, have Claude run
   `cat /proc/1/environ` and confirm it is refused or empty: that is the only
   way to see that the process isolation engaged.

## Behaviour that changes

- The review writes to the run log and step summary, not inline comments (under
  `workflow_dispatch` the inline-comment tool never existed, so nothing is lost).
- The unpinned code-review plugin and its marketplace are removed.
- `claude.yml` answers; it never pushes a branch or a commit.

## Counted as applied

When `git apply --reverse --check docs/passes/16-staged/M/M.patch` succeeds on
`origin/main`. The ledger records the owner's commit.
