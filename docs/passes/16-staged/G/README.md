<!-- SPDX-License-Identifier: Apache-2.0 -->

# Staged for the owner — work order 16, part G

The staging procedure of `prompts/16-complete-mutating-product.md` §3 item 5.
Plan: `docs/passes/16-plans/G.md`.

## `01-check-licences-unsafe-comment.patch`

**What.** A comment-only change to `scripts/check-licences.sh` (lines 10–11):
the header says `unsafe_code = "forbid"` covers the workspace's own code. Part
G-1 lowers the workspace lint to `deny` and puts `#![forbid(unsafe_code)]` at
every crate root but `adapters`, whose one reviewed `unsafe` call is ADR-0007's.
The patch states that.

**Why.** ADR-0007 (Consequences, "G's obligations") asks for this one-line
correction, staged because the script is a guarded file. Nothing the script
checks changes; the comment is wrong, the check is not. **Part G does not wait
on it.**

**Owner action.** On a topic branch from `main`, after G-1 has merged:

```bash
git apply docs/passes/16-staged/G/01-check-licences-unsafe-comment.patch
```

then `scripts/validate.sh --quick`, and commit. The guard refuses a session's
edit of `scripts/`; the arming sentence, if a session is to apply it for you,
is:

> MPB_ALLOW_GUARD_EDIT=1 Apply docs/passes/16-staged/G/01-check-licences-unsafe-comment.patch to scripts/check-licences.sh.

**Applied when** `git apply --reverse --check
docs/passes/16-staged/G/01-check-licences-unsafe-comment.patch` succeeds on
`origin/main`.
