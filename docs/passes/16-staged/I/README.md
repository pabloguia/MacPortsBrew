<!-- SPDX-License-Identifier: Apache-2.0 -->

# Staged for the owner: the shim's symlink token (work order 16, part I)

One patch, against `main` at `a393cc6`. It changes the `Stop` gate and its
tests, which the session may not change (the work order
`prompts/16-complete-mutating-product.md`, §3 items 4 and 5). Part I's plan
(`docs/passes/16-plans/I.md`, D14) is the design, and SE-115 records it; slice
I-6 is the first that needs it applied.

| Patch | Files | Needed by | Executed by the session? |
|---|---|---|---|
| `01-shim-symlink-token.patch` | `.claude/hooks/stop-gate.sh`, `.claude/hooks/test-hooks.sh`, and their `.codex/` mirrors | I-6 (the link writer, `crates/adapters/src/shim/link.rs`) | Written as patch text from the current files; the patched copies were checked with `bash -n`; the patched `test-hooks.sh` was run beside the patched `stop-gate.sh` in a scratch directory outside the repository, 299/299 (243 today, plus 56 new cases), and the patched `stop-gate.sh` passes on `a393cc6` plus this staging. After the first apply (2026-11-24), `validate.sh`'s `shellcheck -x -P SCRIPTDIR` was run on the patched copies too, and passes (see "First apply") |

## Why it is needed

A layer-4 shim is a symlink by spec (05 `:63`), and the `Stop` gate refuses the
`symlink` token in every non-comment line under `src/`. A wrapper script was
rejected (D14): it deviates from the spec's "symlinks" and choosing it to avoid
a guard is the rewording `CLAUDE.md` forbids. The writer serves the shim
offers, which stay refused (`LAYER4_SHIM_OFFER_ADMITTED = false`) until you
rule on DP-058; applying this patch does not decide DP-058.

## What it changes

- **One hit admitted, by file, count and shape.** In
  `crates/adapters/src/shim/link.rs` only, and only while that file has exactly
  one hit of the token scan in non-comment code (`shim_link_pinned=1`), whose
  line starts with `std::os::unix::fs::symlink(` and carries no second token of
  the scan or the elevation program's name. A second hit, an import, a call of
  another shape, or another token on the line turns the exemption off and every
  hit in the file is reported. The rule can only remove one hit; it never
  widens the scan's refusals elsewhere.
- **No macro around the call.** `link.rs` must name neither `macro_rules` nor
  `include` as a word, so the one line cannot be a macro body expanded
  elsewhere, whatever whitespace or line break sits before the `!`.
- **Code is compiled where it lives.** Every code `include!` under `src/`
  fails, and so does every `#[path]` but a bare sibling `<name>_tests.rs`
  literal on one line (the two this tree has), spaced, split-across-lines and
  `cfg_attr` forms included. `include_str!` and `include_bytes!` embed text and
  stay allowed.
- **Still refused, `link.rs` included:** `symlinkat`, `setuid`, `launchctl`,
  `osascript`, `doas`, and, new in this patch, **`soft_link`** (the standard
  library's deprecated alias for the same call), the rest of the uid and gid
  family (`seteuid`, `setreuid`, `setresuid`, `setgid`, `setegid`, `setregid`,
  `setresgid`), and the `SYS_` names of those calls and of `symlink` and
  `symlinkat`, which the word match does not catch.
- **What the gate cannot see** (D14, SE-115): reach, since a function wrapping
  the call can be called from anywhere in `adapters` (the I-9 tripwire
  `the_switch_needs_consented_writes` and review hold it); a syscall named by
  its bare number; and destination, since the gate pins the call's shape, not
  where it writes (`invariant_07_a_shim_link_is_never_written_inside_a_provider_prefix`
  holds it, landed by I-6 in the same pull request as `link.rs`).
- **Two older holes closed, found by the reviews of this patch.** The comment
  filters dropped any line starting with `#`; in Rust that is an attribute,
  and `#[allow(..)] <call>;` on one line passed every scan (so did a file name
  containing `:N:#`). Only `//` lines are dropped now. And macOS's recursive
  grep does not follow a symlink, so a link under `src/` hid its target
  locally while CI's GNU grep scanned it; a symlink under `src/`, or a
  `build.rs` that is one, now fails the gate. No line under `crates/*/src`
  starts with `#` and carries a token or a mutating literal today, no symlink
  is there, and no `include!` or other `#[path]`, so no change moves this
  tree's verdict.
- **Fifty-six `test-hooks.sh` cases**, one change each from a passing fixture.
  Pass: the pinned call, with LF and with CRLF; a sibling
  `#[path = "<name>_tests.rs"]`; an `include_str!`. Block: a second call, an
  import, another shape, a second call behind an attribute; the call as a
  macro body, plain, spaced before `!` and with `!` on the next line; an
  `include!` in `link.rs`; each of twelve other tokens beside the call and on
  its line; the call in another `shim/` file or another crate's
  `shim/link.rs`; a `#[path]` naming `shim/`, to a file outside `src/`,
  spaced, split, under `cfg_attr`, with an escape, or same-directory inside
  `shim/`; a code `include!` anywhere, plain or spaced; the alias anywhere; a
  token, a mutating literal or the elevation program behind an attribute; a
  symlink under `src/`; a `build.rs` that is a symlink. And the new `awk`
  filter, the symlink walk and the read of `link.rs` each failing blocks
  rather than dropping hits.
- The token scan's failure text no longer says nothing here creates
  symlinks; it names the one admitted call.
- The `.codex/hooks/` hunks are the same bodies under the generated header.

D14 also named `scripts/test-gates.sh`. It tests the `scripts/` gates' parsers,
not the `Stop` gate, which `test-hooks.sh` alone exercises, so no case is
added there. `.claude/rules/adapters.md` is not a guarded path; its sentence is
amended in the same pull request as this staging, not in the patch.

## What the owner does

After the pull request that stages this directory merges, from the repository
root, on a topic branch cut from `origin/main`:

```bash
git fetch origin
```

```bash
git switch -c wo16/I-stage-apply origin/main
```

The patch applies to any later `main` whose two hooks are unchanged since
`a393cc6`; check that first:

```bash
git apply --check docs/passes/16-staged/I/01-shim-symlink-token.patch
```

```bash
git apply docs/passes/16-staged/I/01-shim-symlink-token.patch
```

```bash
.claude/hooks/test-hooks.sh
```

A pass ends with `hook tests: 299/299 passed`, the count in the table above,
unless a later change added cases to the unpatched file.

```bash
scripts/generate-codex-layer.sh --check
```

(If that reports drift, run `scripts/generate-codex-layer.sh` to regenerate the
mirror rather than hand-editing `.codex/`.)

```bash
scripts/validate.sh
```

If all pass, commit and open the pull request as usual; it can merge before
I-6, since the exemption admits nothing until `link.rs` exists. If one fails,
do not edit around it: revert with `git apply --reverse
docs/passes/16-staged/I/01-shim-symlink-token.patch` and tell a session what
failed.

To have a session apply it instead, send it a message containing exactly:

> MPB_ALLOW_GUARD_EDIT=1 Apply the patch in docs/passes/16-staged/I/ on a
> topic branch, run the checks its README lists, and open a pull request.

## First apply

On 2026-11-24 a session applied the patch, armed by the owner, on
`wo16/I-stage-apply`. `test-hooks.sh` gave 299/299 and the codex-layer check
passed, but `validate.sh` failed at its `shellcheck` stage: SC2016 on the three
macro-body cases, whose `printf` writes `$a:expr, $b:expr` in single quotes on
purpose (Rust macro metavariables, not shell expansions). The patch was
reverted as this README says. The fix adds a `# shellcheck disable=SC2016`
line before each of the three, with one comment saying why, in both
`test-hooks.sh` copies, and moves the hunk headers by four lines. No case or
verdict changes.

## Reviews

`security-reviewer`, 2026-11-22: the exemption sound as written, resting on two
older holes (`#`-led lines dropped as comments; a symlink under `src/` unread
by macOS grep) and a reach gap (a macro body, a `#[path]` import); all folded
in, with the `SYS_` names refused. `spec-guardian`, the same day: the macro ban
and the `#[path]` check beaten by whitespace and split or `cfg_attr` forms,
SE-115 cited but absent, the uid and gid family unrefused, the destination
unstated; all folded in (above), 25 cases added, 16 of which fail against the
first draft. `docs-auditor`, the same day: this README's apply steps, this
section and the next. Residuals, recorded in SE-115: reach through a wrapper
function, a syscall by bare number, and destination, each with what holds it.

## Counted as applied

When `git apply --reverse --check
docs/passes/16-staged/I/01-shim-symlink-token.patch` succeeds on
`origin/main`. The ledger records the owner's commit.
