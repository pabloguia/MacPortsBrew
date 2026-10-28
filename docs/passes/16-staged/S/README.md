<!-- SPDX-License-Identifier: Apache-2.0 -->

# Staged for the owner: the read that shows a package active (work order 16, part S)

One patch, `01-verify-active-cells.patch`, against `main` at the time of
staging. It changes **only** `data/policy/provider-commands.toml`, which work
order 16 §3 item 3 freezes: a change to it must be the owner's. The plan is
`docs/passes/16-plans/S.md`; the facts are R-20
(`docs/research/R-20-what-a-read-shows-as-active.md`).

**Apply it only after part S-1 is on `origin/main`.** Before S-1 the loader
does not know the two new success strings and refuses the whole table, so
every command that reads it fails.

## Why

04's verification contract names "the destination provider reports the package
as installed and active" first, and no build can check it: neither provider's
current verify read shows "active" (R-20 findings 14 and 19). Two reads that are
**already on the process allowlist** do show it — capture runs both today:

| Provider | Read | What answers "active" |
|---|---|---|
| Homebrew | `brew info --json=v2 --installed` | the one `formulae[]` entry of that name whose `linked_keg` equals one of its own `installed[].version` (R-20 E7, E8) |
| MacPorts | `port -q installed` | exactly one row of the port ending ` (active)` (R-20 E3–E5) |

The planner and the execution gate take every verify command from this table
and nowhere else, so the reads cannot reach a plan without it. Nothing in
`crates/adapters/src/process/` changes: no program, subcommand or option is
added to what the process executor permits.

## What changes

In each of the two entries:

- `verbs.verify_active` — the argv above, naming no package;
- `success.verify_active` — `json_reports_package_linked` (Homebrew) or
  `stdout_marks_package_active` (MacPorts);
- `elevation.verify_active = "none"` and `elevation.verify_active_gui_app`
  (Homebrew `none`, MacPorts `unknown`, as for `verify_absent_gui_app`, so a
  MacPorts GUI app is still refused at the gate);
- a `provenance` cell for each new success and elevation cell, citing R-20 with
  `read_on = "2026-10-18"`;
- the entry's own `read_on` moved from `2026-09-19` to `2026-10-18`, because the
  loader refuses a cell newer than its entry.

That makes `2026-10-18` the newest date in `data/policy/`, so
`invariant_20_the_policy_version_names_the_newest_read_date_in_the_data` fails
until `policy.toml`'s version moves to `2026.10.18-1`. The session makes that
bump, and regenerates every pinned copy of the version, in the **next** commit
on the same branch. Your commit is red on its own; that is expected, and the
pull request's head is what CI judges.

## What the owner does

After a session has pushed `wo16/S-2-active`, cut from `origin/main` after S-1
merged:

```bash
git fetch origin && git switch wo16/S-2-active
```

```bash
git apply --check docs/passes/16-staged/S/01-verify-active-cells.patch
```

```bash
git apply docs/passes/16-staged/S/01-verify-active-cells.patch
```

```bash
cargo test -p macportsbrew-application provider_commands
```

The loader's tests must pass; `invariant_20_the_policy_version_*` failing is
expected until the session's bump. Then commit only that file and push:

```bash
git commit -m "feat(policy-data): verify_active cells for both providers (part S)" -- data/policy/provider-commands.toml
```

```bash
git push origin wo16/S-2-active
```

Tell a session it is done. If the check or the loader's tests fail, do not edit
around it: revert with
`git apply --reverse docs/passes/16-staged/S/01-verify-active-cells.patch`
and tell a session what failed.

**If you prefer a session to apply it**, send it a message containing exactly:

> Apply docs/passes/16-staged/S/01-verify-active-cells.patch on wo16/S-2-active
> with git apply, check it byte for byte as the part S plan says, and commit it
> together with the policy version bump.

The session then writes that file only through `git apply`, checks before
committing that `git apply --check` passed on the parent and that
`git diff <parent> HEAD -- data/policy/provider-commands.toml` equals the
patch's diff for that file, and records both in the ledger with your message
and the commit; `security-reviewer` confirms before the part continues.

## Behaviour that changes

None until part S-2's code reads the cells. Then every `VERIFY_DESTINATION` in a
plan carries a second command, and `PROVIDER_REPORTS_ACTIVE` becomes evaluable.
A cask, and a Homebrew formula that is not linked, fail it (fail closed); part H
reports such a package unchecked. `THIS_BUILD_DISPATCHES_MUTATIONS` stays
`false`.

## Counted as applied

When `git apply --reverse --check docs/passes/16-staged/S/01-verify-active-cells.patch`
succeeds on `origin/main`. The ledger records the commit that applied it and the
result of the byte-equality check on it.
