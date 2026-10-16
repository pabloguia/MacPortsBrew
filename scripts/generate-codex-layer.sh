#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Generates `.codex/` from `.claude/`, and checks that it is current.
#
# **Why this exists.** `.codex/` is a second, live agent-harness layer: its
# `hooks.json` wires five hooks; `lib.sh` is sourced and `test-hooks.sh` is run
# by hand, so seven files generate but five are wired. It had drifted into an older,
# weaker copy of `.claude/` — its path guard protected the machine and no
# repository path at all, its stop gate still carried two fail-opens that
# `.claude/` had fixed, and one of the four subagents was simply missing. Nobody
# weakened it; it was written once and `.claude/` moved on. That is F-42, and
# `AGENTS.md` names the shape: "weakening one is invisible until something it
# would have caught ships."
#
# **The ruling (DP-018) is that `.claude/` is the source and `.codex/` is
# generated from it** — not that both point at one shared set of scripts, which
# is what this file first proposed. The owner's reason is better than the
# proposal: this repository is open source and the developer chooses their
# harness, so somebody who picks Codex should get a complete, self-contained
# layer rather than a manifest reaching into a directory named for a different
# tool.
#
# **Why a copy is enough.** The hook scripts contain nothing Codex-specific —
# not one occurrence of the string, checked. Both harnesses feed a hook one JSON
# object on stdin and read one JSON object back, and `mpb_deny` and
# `mpb_context` are byte-identical in the two trees today. So the transform for
# a hook is: take the source, add a banner saying it is generated. If that ever
# stops being true, this script is where the divergence gets a name and a
# reason, instead of happening silently over four passes.
#
# Usage:
#   scripts/generate-codex-layer.sh            regenerate `.codex/` in place
#   scripts/generate-codex-layer.sh --check    fail if it is not current
#
# `--check` is what `validate.sh` runs. It regenerates into a temporary tree and
# compares, so the answer is "would generating change anything", not a hash that
# can drift from the thing it claims to describe.

set -euo pipefail

# From this file, like every other script here. Honouring CLAUDE_PROJECT_DIR
# would let one env var point two of validate.sh's stages at a different
# tree from the other fourteen.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="generate"
[ "${1:-}" = "--check" ] && MODE="check"

# shellcheck disable=SC2016  # the backticks are prose in a generated comment
banner_for() {
  printf '# GENERATED FILE - do not edit.\n#\n'
  printf '# Generated from .claude/hooks/%s by scripts/generate-codex-layer.sh.\n' "$1"
  printf '# `.claude/` is the source of truth (DP-018); edit there and regenerate.\n'
  printf '# A `validate.sh` stage fails when this file and its source disagree.\n'
}

# One hook: shebang, SPDX, the generated banner, then the source verbatim from
# its third line on. The source's own shebang and SPDX are re-emitted rather
# than copied so the banner lands above the file's prose and a reader meets it
# before anything else.
emit_hook() {
  local src="$1" out="$2" name
  name="$(basename "$src")"
  {
    printf '#!/usr/bin/env bash\n'
    printf '# SPDX-License-Identifier: Apache-2.0\n'
    printf '#\n'
    banner_for "$name"
    tail -n +3 "$src"
  } > "$out"
  # Executable only for what `hooks.json` actually spawns. `lib.sh` is sourced,
  # never exec'd, and is 644 in `.claude/` - a blanket `chmod +x` made the
  # generated copy differ from its source in a way `diff -rq` cannot see,
  # because it compares content and not mode.
  case "$name" in
    lib.sh) chmod 644 "$out" ;;
    *)      chmod 755 "$out" ;;
  esac
}

# One subagent: YAML frontmatter to TOML.
#
# Only `name`, `description` and the body are carried, because only those three
# are present in the `.codex/agents/*.toml` this repository already had working.
# `.claude/`'s `tools:` and `disallowedTools:` - which are what make
# `security-reviewer` and the other three read-only - have **no verified
# equivalent here**, and are therefore dropped in translation. That is a real
# gap, recorded as part of F-42 rather than papered over: Codex agents do appear
# to take a `sandbox` setting, but the only source found for it was secondary,
# and this project does not write a security-relevant key from an unverified
# reading (AGENTS.md, "verify, do not assume").
emit_agent() {
  local src="$1" out="$2"
  python3 - "$src" "$out" <<'PY'
import io, json, sys
src, out = sys.argv[1], sys.argv[2]
text = io.open(src, encoding="utf-8").read()
if not text.startswith("---\n"):
    sys.exit("%s: no YAML frontmatter" % src)
_, front, body = text.split("---\n", 2)

field = {}
key = None
for line in front.splitlines():
    if line[:1].isalpha() and ":" in line:
        key, _, value = line.partition(":")
        field[key.strip()] = value.strip()
    elif key and line.startswith(("  ", "\t")):
        field[key] = (field[key] + " " + line.strip()).strip()

for required in ("name", "description"):
    if required not in field:
        sys.exit("%s: frontmatter has no %s" % (src, required))

body = body.strip("\n")
# A *literal* multi-line string, so a backslash in an agent's prose - a regex,
# most likely - stays a backslash instead of becoming an escape sequence TOML
# either rejects or silently rewrites. `--check` compares generation against
# generation, so it could never have caught that on its own.
if "'''" in body:
    sys.exit("%s: body contains a TOML literal multi-line delimiter" % src)

with io.open(out, "w", encoding="utf-8") as f:
    f.write("# SPDX-License-Identifier: Apache-2.0\n")
    f.write("#\n")
    f.write("# GENERATED FILE - do not edit.\n")
    f.write("#\n")
    f.write("# Generated from .claude/agents/%s by scripts/generate-codex-layer.sh.\n"
            % src.rsplit("/", 1)[-1])
    f.write("# `.claude/` is the source of truth (DP-018); edit there and regenerate.\n")
    f.write("#\n")
    f.write("# The source's `tools` and `disallowedTools` are NOT carried: they are what\n")
    f.write("# make this reviewer read-only under Claude Code, and no equivalent key is\n")
    f.write("# verified for Codex. Treat this agent as unrestricted until that is settled\n")
    f.write("# (F-42).\n")
    f.write("\n")
    # json.dumps rather than a hand-rolled replace: JSON string syntax is a
    # subset of TOML's basic-string syntax, and it escapes backslashes, which
    # the hand-rolled version did not - a description ending in one produced an
    # unterminated string that swallowed the next line.
    f.write("name = %s\n" % json.dumps(field["name"]))
    f.write("description = %s\n" % json.dumps(field["description"]))
    f.write("developer_instructions = '''\n%s'''\n" % (body + "\n"))

# Round-trip it. The generator is the only writer of these files, so a malformed
# one is exactly what `--check` would call correct.
try:
    import tomllib
except ImportError:
    tomllib = None
if tomllib is not None:
    with open(out, "rb") as f:
        parsed = tomllib.load(f)
    for required in ("name", "description", "developer_instructions"):
        if not parsed.get(required):
            sys.exit("%s: emitted TOML lost %s" % (out, required))
PY
}

# One skill: the body with its paths retargeted, and nothing else touched.
#
# The existing `.agents/skills/` was made by substituting the product name
# throughout, and the result is why this does not: it produced `.Codex/` with a
# capital C - a directory that does not exist, and that only appears to work
# because macOS is case-insensitive by default - and turned a documentation URL
# into `https://code.Codex.com/`, a domain that does not resolve. A blind
# rename of a proper noun cannot tell a path from a prose word from a hostname.
#
# So only paths move, and `settings.json` is named explicitly because the two
# harnesses do not spell their configuration the same way.
# shellcheck disable=SC2016  # the backticks are prose in a generated comment
emit_skill() {
  local src="$1" out="$2"
  mkdir -p "$(dirname "$out")"
  {
    printf '<!-- GENERATED FILE - do not edit. Generated from %s\n' "$src"
    printf '     by scripts/generate-codex-layer.sh; `.claude/` is the source (DP-018). -->\n'
    sed -e 's|\.claude/settings\.json|.codex/hooks.json|g' -e 's|\.claude/|.codex/|g' "$src"
  } > "$out"
}

# `tooling-recheck` is deliberately not generated. Its subject is Claude Code's
# own field names, hook contracts and rule syntax, verified against Claude's
# documentation - the one skill whose *content* is about the harness rather than
# about this repository. A Codex equivalent has to be written against Codex's
# documentation; deriving it is how the current `.agents/` copy ended up telling
# an agent to read `https://code.Codex.com/docs/`.
SKILLS_NOT_GENERATED="tooling-recheck"

generate_into() {
  local dest="$1" skills="$2"
  mkdir -p "$dest/hooks" "$dest/agents"
  local src name
  for src in .claude/hooks/*.sh; do
    emit_hook "$src" "$dest/hooks/$(basename "$src")"
  done
  for src in .claude/agents/*.md; do
    emit_agent "$src" "$dest/agents/$(basename "$src" .md).toml"
  done
  rm -rf "$skills"
  for src in .claude/skills/*/SKILL.md; do
    name="$(basename "$(dirname "$src")")"
    case " $SKILLS_NOT_GENERATED " in *" $name "*) continue ;; esac
    emit_skill "$src" "$skills/$name/SKILL.md"
  done
}

if [ "$MODE" = "generate" ]; then
  generate_into .codex .agents/skills
  printf 'codex layer: regenerated from .claude/ (%d hooks, %d agents, %d skills; %s not generated)\n' \
    "$(find .claude/hooks -name '*.sh' | wc -l | tr -d ' ')" \
    "$(find .claude/agents -name '*.md' | wc -l | tr -d ' ')" \
    "$(find .agents/skills -name 'SKILL.md' | wc -l | tr -d ' ')" \
    "$SKILLS_NOT_GENERATED"
  exit 0
fi

# --check. Generated into a temporary tree and compared, so this answers
# "would regenerating change anything" rather than trusting a stored digest.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
generate_into "$tmp" "$tmp/skills"

status=0
# A skill in SKILLS_NOT_GENERATED is not this check's business, present or not:
# it is deliberately not derived from .claude/, so a hand-written Codex copy of it
# (which some Codex sessions create) is neither drift nor something to delete.
skip=(); for n in $SKILLS_NOT_GENERATED; do skip+=(-x "$n"); done
if ! diff -rq "$tmp/hooks" .codex/hooks >/dev/null 2>&1 \
   || ! diff -rq "$tmp/agents" .codex/agents >/dev/null 2>&1 \
   || ! diff -rq "${skip[@]}" "$tmp/skills" .agents/skills >/dev/null 2>&1; then
  echo "codex layer: .codex/ is not what .claude/ generates." >&2
  diff -rq "$tmp/hooks" .codex/hooks 2>&1 | sed 's/^/  /' >&2 || true
  diff -rq "$tmp/agents" .codex/agents 2>&1 | sed 's/^/  /' >&2 || true
  diff -rq "${skip[@]}" "$tmp/skills" .agents/skills 2>&1 | sed 's/^/  /' >&2 || true
  echo "  Run scripts/generate-codex-layer.sh. If the difference is deliberate," >&2
  echo "  it belongs in .claude/ or in this generator, not in .codex/ (DP-018)." >&2
  status=1
fi

# The manifest is hand-written - it maps Codex's own event names onto these
# scripts - so it is not generated. Two things about it are checked.
#
# **That it names no absolute path.** The first committed version wired every
# hook as `/Users/<name>/projects/MacPortsBrew/.codex/hooks/...`, which put a
# username into a repository DP-007 sends to public GitHub *and* meant every
# clone got five hooks pointing at a directory that does not exist - a Codex
# session with no PreToolUse guard and no Stop gate, announcing nothing. That is
# the failure this whole layer was generated to end, reintroduced by the one
# file the generator does not write. Codex sets no `CLAUDE_PROJECT_DIR`
# equivalent (developers.openai.com/codex/hooks, read 2026-09-09); its own
# guidance is to resolve from the git root, which is what the manifest does now.
#
# **That every script it names exists.** A reference to a hook that was renamed
# is how a guard silently stops running.
if [ -f .codex/hooks.json ]; then
  missing=""
  while IFS= read -r ref; do
    [ -n "$ref" ] || continue
    [ -f "$ref" ] || missing="$missing $ref"
  done < <(grep -oE '\.codex/hooks/[A-Za-z0-9_-]+\.sh' .codex/hooks.json | sort -u)
  if [ -n "$missing" ]; then
    echo "codex layer: hooks.json references a script that does not exist:$missing" >&2
    status=1
  fi
  # Checked on the whole line, not on a relative substring: the first version of
  # this check grepped out `.codex/hooks/x.sh` from inside an absolute path and
  # then tested *that* against the repository, so it passed while the path Codex
  # would actually spawn was somebody else's home directory.
  if grep -qE '"command"[^"]*"[^"]*(/Users/|/home/|^/)' .codex/hooks.json; then
    echo "codex layer: hooks.json wires a hook by absolute path." >&2
    grep -nE '"command"' .codex/hooks.json | sed 's/^/    /' >&2
    echo "  A committed absolute path carries a username into a public repository" >&2
    echo "  and points every other clone at a directory that does not exist, so the" >&2
    echo "  guards silently do not run. Resolve from the git root instead." >&2
    status=1
  fi
fi

[ "$status" -eq 0 ] && echo "codex layer: ok"
exit "$status"
