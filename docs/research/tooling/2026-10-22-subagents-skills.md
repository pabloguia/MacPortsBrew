<!-- SPDX-License-Identifier: Apache-2.0 -->

# Tooling re-verification — subagents and skills (2026-10-22)

Group: `sub-agents` and `skills`, read on 2026-10-22 from
`https://code.claude.com/docs/en/sub-agents.md` and `/skills.md` (both fetched
in full). Returned by a read-only subagent; the runner wrote this file. Verdict:
**no contract break**; the findings are mechanisms not adopted and two wordings
that over-state what a field enforces.

## Still documented and valid

Agent frontmatter `name`, `description`, `tools`, `disallowedTools`,
`model: inherit`, `permissionMode: plan`, `color`. Skill frontmatter `name`,
`description`, `argument-hint`, `allowed-tools` (comma string), `license`;
`$ARGUMENTS`. The longest skill body is 88 lines (guidance: under 500).

## Not verifiable from these two pages

`${CLAUDE_PROJECT_DIR}` inside `allowed-tools` `Bash(...)`; the `:*` form beside
the space-glob form; `*` with no preceding space; the path semantics of
`Edit(docs/research/tooling/**)`. All four belong to the permissions page.
`TodoWrite` as a subagent tool name was not re-confirmed.

## Guidance the files over-state

- `allowed-tools` **pre-approves** tools; it does not restrict them ("every tool
  remains callable"). A skill that leaves `Write` or `Edit` out of the list has
  not forbidden them, and `tooling-recheck`'s `Edit(docs/research/tooling/**)`
  is not a path jail. `disallowed-tools` (skill) or a deny rule restricts.
- `permissionMode: plan` on the three reviewers is not independent: if the
  parent session runs `bypassPermissions`, `acceptEdits` or `auto`, the subagent
  runs in the parent's mode and the field is ignored. `disallowedTools` is the
  firm guarantee. `disableBypassPermissionsMode` (settings.json) narrows the
  bypass case, not the other two.

## Documented mechanisms not adopted (recommendations)

- `context: fork` + `agent: provider-researcher` on `research-note`, and on
  `tooling-recheck` (the latter would replace the `git status` after-check).
- A frontmatter `PreToolUse` hook on the three reviewers' `Bash` to refuse write
  commands (`disallowedTools` does not stop `echo >` or `sed -i`); or drop `Bash`.
- A path-validating `PreToolUse` hook confining `provider-researcher`'s
  `Write`/`Edit` to `docs/research/`.
- `maxTurns` on the four agents; `disable-model-invocation: true` on the skills
  that write files (a design call, since their descriptions say "use whenever").

Not adopted this pass: each changes `.claude/` behaviour that the owner has not
asked to change; they are listed in the pass report.
