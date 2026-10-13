#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Defence in depth for "this tool spawns six programs and none is a shell".
#
# **The control is not here.** It is `is_permitted_program` in
# `crates/adapters/src/process/mod.rs`, checked by the runner on the `PathBuf`
# about to reach the kernel. That placement is deliberate and was arrived at the
# hard way: a review defeated every version of this that tried to be the control.
#
#   - A private constructor did not stop `spec.program = <runtime value>`,
#     because the fields were public. They are not any more.
#   - `&'static str` did not mean "written in this source": `Box::leak` does it,
#     and so does a module-level `OnceLock` with no `leak` and nothing to grep.
#   - A *denylist* of interpreters was missing `tclsh` - which `port` itself is
#     written in - along with `xcrun`, `arch`, `awk`, `expect` and as many more
#     as anyone cares to name. A denylist of interpreters cannot be completed.
#   - The literal could be split with `concat!`, escaped as `"/bin/\x73h"`, or
#     broken across lines past a line-oriented check like this one.
#
# What this script is for is the thing a run-time check cannot do: fail at
# commit time, in review, where a person sees it. So it asks the same question
# the runner asks, textually, and it asks it as an allowlist for the same reason
# the runner does.
set -euo pipefail
cd "$(dirname "$0")/.."

status=0

# The six, read out of the source that defines them rather than restated here.
# `every_program_an_adapter_names_is_on_the_allowlist` holds that list to what
# the adapters name; this holds the script to that list.
allowlist="$(sed -n '/^pub(crate) const PERMITTED_PROGRAMS/,/^];/p' \
  crates/adapters/src/process/mod.rs \
  | grep -oE '^    "[a-z0-9_-]+"' | tr -d ' "' | sort)"
if [ "$(printf '%s\n' "$allowlist" | grep -c .)" -lt 4 ]; then
  echo "no-shell: could not read PERMITTED_PROGRAMS from process/mod.rs;" >&2
  echo "  the constant moved and this script is now checking nothing." >&2
  exit 1
fi

# Where test code begins.
#
# The exemption is "the trailing `mod tests` block", and the review found the
# earlier version asserting something weaker - at most one `#[cfg(test)]` - and
# relying on something stronger. In the ordinary two-file layout
# (`#[cfg(test)] mod tests;` at the top, tests in a sibling file) that turned the
# scan off for the whole file, silently, for exactly the two 1000-line adapters
# this exists to cover. So: the attribute must be the last one in the file *and*
# be followed immediately by an inline `mod tests {`. Anything else is scanned
# whole.
# A `#[cfg(test)]` on a const or a `use` is ordinary code and stays scanned;
# only the one that opens the inline test module is a boundary, and there must
# be exactly one of those, running to the end of the file.
test_boundary() {
  local file="$1" opens
  opens="$(grep -n -B1 '^mod tests {' "$file" \
    | grep -c '#\[cfg(test)\]' || true)"
  [ "$opens" = "1" ] || { echo 0; return; }
  grep -n '^mod tests {' "$file" | head -1 | cut -d: -f1
}

while IFS= read -r file; do
  boundary="$(test_boundary "$file")"
  end="${boundary}"
  [ "$end" = "0" ] && end="$(( $(wc -l < "$file") + 1 ))"

  # Every absolute path to a *program* named in production source. A program is
  # a leaf under one of the four system binary directories, or under a `bin`
  # directory - which is the shape the runner permits. `/usr/bin` itself, and
  # every provider prefix, has no leaf and is not matched, so the four entries
  # of `SYSTEM_PATH` and the registry's prefixes do not trip it.
  #
  # A backslash is in the leaf class on purpose: `"/bin/\x73h"` is `/bin/sh`
  # to the compiler, and without it the literal simply did not match and was
  # never checked at all - measured, and the one probe of fourteen that got
  # past the first rewrite. It is also the clearest illustration of why the
  # control is the runner's and this is defence in depth: the next escape is
  # somebody else's afternoon.
  while IFS= read -r hit; do
    [ -z "$hit" ] && continue
    leaf="${hit##*/}"
    printf '%s\n' "$allowlist" | grep -qxF "$leaf" && continue
    echo "no-shell: ${file} names the program \"${hit}\" in production source," >&2
    echo "  which is not one of: $(printf '%s ' "$allowlist" | tr '\n' ' ')" >&2
    status=1
  done < <(head -n "$(( end - 1 ))" "$file" \
    | grep -vE '^[[:space:]]*(//|///|//!)' \
    | grep -oE '"(/(usr/)?s?bin|/[A-Za-z0-9_./-]*/bin)/[A-Za-z0-9_.+\\-]+"' \
    | tr -d '"' || true)
done < <(find crates -name '*.rs' -path '*/src/*' | sort)

# Nothing manufactures a `&'static str` at runtime.
#
# `Box::leak` is *not* the only way and this list is not a proof of anything -
# `OnceLock`/`LazyLock` initialised from runtime data reaches the same place with
# none of these words in it. That is precisely why the control is the runner's
# check on the finished path and not this. These stay because they are the forms
# that read as ordinary code, and the bare-name variants are here because
# `let f = Box::leak;` was measured to walk past a version that required `leak(`.
hits="$(grep -RIn --include='*.rs' -E '(Box|String|Vec)::leak\b|\.leak\(\)' crates \
  | grep -vE '^crates/[^/]+/tests/' \
  | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|///|//!)' || true)"
if [ -n "$hits" ]; then
  echo "no-shell: a value is leaked to 'static, which is one way to hand the" >&2
  echo "  program-position constructors something built at run time:" >&2
  printf '  %s\n' "$hits" >&2
  status=1
fi

# The private constructor stays private. `pub(in crate::process)` is the real
# guard and the compiler enforces it; this catches the edit that widens it,
# which would otherwise be invisible until something used it.
hits="$(grep -RIn --include='*.rs' 'CommandSpec::new' crates \
  | grep -v '^crates/adapters/src/process/' || true)"
if [ -n "$hits" ]; then
  echo "no-shell: CommandSpec::new used outside the executor module:" >&2
  printf '  %s\n' "$hits" >&2
  echo "  Use CommandSpec::provider or CommandSpec::system." >&2
  status=1
fi

# And the fields stay private, which is what makes those constructors mean
# anything. A review rewrote `spec.program` after construction and every gate
# stayed green.
# Scoped to CommandSpec: `Invocation`'s fields are a *record* of what ran and
# are read outside this crate, so they are public and should stay so.
if sed -n '/^pub struct CommandSpec {/,/^}/p' crates/adapters/src/process/mod.rs \
    | grep -qE '^[[:space:]]*pub [a-z_]+:'; then
  echo "no-shell: a CommandSpec field is public again; a spec can then be" >&2
  echo "  rewritten after construction, program and environment included." >&2
  status=1
fi

# A test never spawns a program named by a string literal.
#
# The string-literal scan above deliberately exempts `crates/*/tests/`, so a
# fixture may name a mutating subcommand in its recorded data. That exemption
# was about *data*; it silently covered *spawning* too, and the one directory
# permitted to contain the word "install" is the one whose code `cargo test`
# executes against the developer's real machine - with `cargo test*` on the
# allow list, so no guard sees anything but the eleven characters `cargo test`.
# 07-quality-and-testing.md: "Tests never touch the developer's real Homebrew
# or MacPorts installation. Any test that could is a build failure."
#
# The rule is shaped by what legitimate tests actually do: every end-to-end
# test spawns the crate's own binary through `env!("CARGO_BIN_EXE_...")`, bound
# to a constant or passed as a parameter - never a literal. So a literal in
# program position is the signal, and it costs no false positives today.
hits="$(grep -RIn --include='*.rs' -E '(process::)?Command::new\([[:space:]]*"' crates/*/tests/ 2>/dev/null \
  | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|///|//!)' || true)"
if [ -n "$hits" ]; then
  echo "no-shell: a test spawns a program named by a string literal:" >&2
  printf '  %s\n' "$hits" >&2
  echo "  A test spawns only this crate's own binary, through" >&2
  echo "  env!(\"CARGO_BIN_EXE_<name>\"). Anything else risks reaching the" >&2
  echo "  developer's real Homebrew or MacPorts install, which" >&2
  echo "  07-quality-and-testing.md makes a build failure. Use a fixture." >&2
  status=1
fi

exit "$status"
