# `.claude/` — agent configuration and operator runbook

Versioned Claude Code configuration for this repository, checked in because it
is part of the project's safety story rather than a personal preference.
`docs/spec/11-agentic-sdlc.md` specifies what belongs here and why; this file
says what each piece is and how an operator sets up and runs a session. If the
two disagree, the specification wins and this file is the bug.

**Tooling facts here were verified against Anthropic's Claude Code
documentation on 2026-09-02, and re-verified at the start of the Foundation
Pass the same day** — the pages `/tooling-recheck` names; the notes
are in `docs/research/tooling/`. They are dated third-party facts and change;
that skill re-verifies them at the start of every pass.

## What is here

| Path | What it is |
|---|---|
| `settings.json` | Permissions, provider-quieting environment, hook wiring, auto-memory off. Checked in; applies to everyone. |
| `settings.local.json` | Personal overrides. Git-ignored. Never put a project rule here. |
| `current-pass` | The pass id the session-start hook announces. One line. |
| `hooks/` | Mechanical enforcement. See below. |
| `agents/` | Subagents: `provider-researcher` plus the three read-only reviewers `spec-guardian`, `security-reviewer`, `docs-auditor`. |
| `skills/` | Repeatable procedures, invoked as `/name`: `tooling-recheck`, `spec-evolution`, `adr`, `research-note`, `decision-pending`, `preflight`, `pass-report`. |

At the repository root, `CLAUDE.md` is the thin Claude Code layer that imports
`@AGENTS.md`; `CLAUDE.local.md`, if you create one for personal notes, is
git-ignored.

## Hooks

| File | Event | What it does |
|---|---|---|
| `lib.sh` | — | JSON field extraction. Fails closed when neither `jq` nor `python3` is present. |
| `session-start.sh` | `SessionStart` | Injects the active pass, the hard boundary and the open decisions. |
| `guard-provider-mutation.sh` | `PreToolUse(Bash)` | Denies elevation and every mutating `brew`/`port` subcommand, plus toolchain switching, shell-file writes and launch machinery — including absolute-path and wrapped forms. |
| `guard-protected-paths.sh` | `PreToolUse(Edit\|Write\|NotebookEdit)` | Denies writes into either provider's prefix, shell configuration and system paths. |
| `post-edit-checks.sh` | `PostToolUse(Edit\|Write)` | Per-file: shell format and lint, JSON validity, Markdown link resolution, SPDX header. Never blocks. |
| `stop-gate.sh` | `Stop` | The definition-of-done gate. Blocks the turn ending on objective failures: links, configuration validity, the no-mutation grep over `crates/*/src`, and `scripts/validate.sh` (`--quick` on every turn; the test suite too when code, tests, fixtures, data or catalogues changed; `MPB_GATE_FULL=1` forces it). The gate decides how much to run; the checks themselves live in `scripts/validate.sh`, which CI and contributors run the same way. |
| `test-hooks.sh` | — | Feeds every hook fixture inputs and asserts its decision. Run after any change here; CI is configured to run it, and has never run. |

Two layers, on purpose: the deny rules in `settings.json` are what Claude Code
enforces; the hooks catch the forms rule syntax does not. Neither is a sandbox
— `docs/spec/11-agentic-sdlc.md` ("What this is not") says exactly what they
are and are not, and why the honest statement matters here.

## Prerequisites

- **Claude Code** — 2.1.258 is what this configuration was verified with on
  2026-09-02 (`claude --version`).
- **`jq` or `python3`.** The guard hooks parse their JSON input with one of
  them and *fail closed* when neither is present, so without one every Bash
  tool call is denied. `python3` from the Command Line Tools is enough.
- **`shellcheck` and `shfmt`** are optional; the post-edit hook uses them when
  present and skips silently when not.
- **The Rust toolchain via `rustup`**, once the Foundation Pass creates
  `rust-toolchain.toml`: `rustup` itself, the pinned version, and both targets
  (`rustup target add x86_64-apple-darwin aarch64-apple-darwin`).
  `rust-toolchain.toml` is a `rustup` mechanism, so a `rustc` installed by a
  package manager and not managed by `rustup` cannot honour it — and the pin
  is what holds the macOS 10.12 / 11.0 floor (ADR-0002). The guards deny `rustup default`,
  `rustup toolchain` and `cargo install`, so install developer tools yourself,
  outside a session.

You do not configure permissions by hand — they are checked in. Verify a
session with `/context` (the agents appear under Custom Agents and the
`@AGENTS.md` import under Memory files), `/permissions`, and by asking Claude
to run `brew install cowsay` — it must be refused.

## Official plugins worth installing

Personal choice, not committed, so each is a `/plugin install` you run
yourself. Turning any of these into a project commitment means adding
`enabledPlugins` and `extraKnownMarketplaces` to `settings.json`, which is what
every collaborator's Claude Code would then try to load — that belongs in an
ADR, not a quiet edit. Names checked 2026-09-02 against the official
marketplace `claude-plugins-official`, which Claude Code registers on first
interactive start (`/plugin marketplace add anthropics/claude-plugins-official`
if it is missing).

```
/plugin install security-guidance@claude-plugins-official
```

Reviews each change for common vulnerabilities as it is written. The threat
model in `docs/spec/04-migration-and-safety.md` is unusually load-bearing, and
a reviewer that fires during writing catches what one that fires afterwards
argues about.

```
/plugin install rust-analyzer-lsp@claude-plugins-official
```

Diagnostics after every edit and real code navigation, once there is Rust
code. Requires the `rust-analyzer` binary on `PATH`; the plugin does not
install it and `cargo install` is denied, so that one is yours (`rustup
component add rust-analyzer`, outside a session).

```
/plugin install commit-commands@claude-plugins-official
```

Git commit workflows, useful from the Foundation Pass onward — the working
protocol asks for incremental conventional commits.

**Deliberately not recommended:** `github`, `linear`, `notion`, `slack` and
other integration plugins bundle MCP servers and cost context every turn. This
project has no external tracker and DP-007 leaves it local. Install one when
there is something to integrate with.

## GitHub Actions

Both Claude workflows in `.github/workflows/` are committed and **dormant**:
the repository has no remote (DP-007) and no secret. To activate, once the
remote exists: run `claude` in the repository and `/install-github-app`, which
installs the GitHub App, stores the secret and opens the workflow pull request;
or install <https://github.com/apps/claude> by hand and add
`ANTHROPIC_API_KEY` or `CLAUDE_CODE_OAUTH_TOKEN` as a repository secret. With
a subscription token, replace the `anthropic_api_key:` line in both workflows
with `claude_code_oauth_token: ${{ secrets.CLAUDE_CODE_OAUTH_TOKEN }}`.

`repository-validation.yml` needs nothing and runs the same gate as the `Stop`
hook.

The boundary that does not move: per `docs/spec/11-agentic-sdlc.md`, no
unattended run merges, releases, pushes to the default branch, or resolves an
item in `docs/decisions-pending.md`. The `@claude` workflow needs
`contents: write` to push to pull-request branches; what keeps it off the
default branch is branch protection, which is a repository setting and is
therefore a sentence here, not a mechanism in this repository — enable it in
the same step that activates the workflows, and pin both actions to commit
SHAs at that point (the tags `@v1`/`@v6` are floating). The workflow does not
build or test the pull-request head: a fork's `build.rs` or test would run
with the job's secrets; `repository-validation.yml` builds without any.

## Running a pass

```bash
echo 01-FOUNDATION-PASS > .claude/current-pass
claude --permission-mode plan
```

Then: read the work order in `prompts/`, `/tooling-recheck`, produce a plan,
have it reviewed, leave plan mode, work incrementally, `/preflight`,
`/pass-report`. The project does not set `permissions.defaultMode`, so the
mode is the operator's choice per session; plan mode is the recommended start
for non-trivial work, and auto mode is what the owner used for the
Repository Readiness Pass.

## The sandbox: evaluated, not enabled

`docs/spec/11-agentic-sdlc.md` asks the Foundation Pass to evaluate Claude
Code's OS-level sandbox against the real build. Evaluated 2026-09-02 from
the sandboxing documentation (notes in `docs/research/tooling/`): with
`sandbox.enabled: true`, the existing `Edit(//opt/local/**)`-style deny
rules and the `WebFetch(domain:...)` allows are merged into the sandbox
boundary, which would give the hooks' no-write rule for the provider
prefixes an enforcement the hooks cannot provide. The build's needs are now
known: writes under the repository, `~/.cargo` (registry cache) and
`~/.rustup` (the pinned toolchain), and network to `crates.io`,
`static.crates.io`, `index.crates.io` and `static.rust-lang.org`. The block
that expresses that is:

```json
"sandbox": {
  "enabled": true,
  "filesystem": { "allowWrite": ["~/.cargo", "~/.rustup", "~/Library/Application Support/macportsbrew"] },
  "network": { "allowedDomains": ["crates.io", "static.crates.io", "index.crates.io", "static.rust-lang.org", "docs.brew.sh", "trac.macports.org", "guide.macports.org", "ports.macports.org", "code.claude.com"] }
}
```

It is **not** enabled in `settings.json`: the setting takes effect at session
start, so the pass that wrote it could not exercise it against the build,
and enabling it blind is the claim the specification tells us not to make.
Enabling it is the first thing to try in the next session, with
`scripts/validate.sh --full` as the test; if it passes, commit the block and
record it in SE-025.

## Testing the binary against a fixture

`macportsbrew --replay fixtures/recordings/<name>.json <command>` runs the
real binary over a recorded fixture instead of this machine, and
`scripts/run-fixture-matrix.sh` does that for every fixture and compares
`analyze --json` byte-for-byte with `fixtures/golden/`. That is how the
slice is shown to run against the whole matrix without a MacPorts
installation here.

## When a hook gets in the way

`hooks/stop-gate.sh` blocks a turn ending while an objective check fails. If it
is wrong, that is a bug in the gate worth fixing — but the escape hatch exists
so a wrong gate does not get deleted wholesale:

```bash
MPB_SKIP_STOP_GATE=1 claude
```

Using it, or Claude Code's own `disableAllHooks` setting, obliges the
end-of-pass report to say so. Claude Code also stops honouring a `Stop` hook
after eight consecutive blocks and ends the turn anyway; a report written
under that ceiling is a report written with the gate failing, and must say so.

The `PreToolUse` guards have no escape hatch on purpose. If one blocks
something legitimate, the fix is a narrower rule discussed with the owner —
not a reworded command.

## Adding to this directory

Anything added here is a project rule and needs the same justification as a
line in the specification: it maps to something in `docs/spec/`, or it does not
belong. Personal preference goes in `settings.local.json`, `CLAUDE.local.md`
or your own `~/.claude/`.
