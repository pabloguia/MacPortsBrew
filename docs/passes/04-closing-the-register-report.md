# 04-CLOSING-THE-REGISTER — pass report

One night, three commits, and a pattern that did not vary: **every claim this
repository made about itself that no gate was checking turned out to be wrong.**
Not most. Every one that was checked for the first time.

The work order (`prompts/04-closing-the-register.md`) was written *after* the
pass began, which is backwards and is recorded as such — `spec-guardian` found
three `docs/spec-evolution.md` entries citing a pass identifier `prompts/` did
not define.

---

## What now works

**The policy version names the data that produced it.** `policy.toml`'s header
has defined `version` as the newest `read_on` across the directory since the
foundation pass, and said why: a recommendation must be traceable to the facts
behind it. On 2026-09-08 a commit moved a read date and left the manifest
behind, so for eleven days two data sets shared one version string.
`invariant_20_the_policy_version_names_the_newest_read_date_in_the_data` now
asserts the date half on every run, textually over the directory — because a
check written against the parsed type would have excluded the two files the
policy crate does not load, and it was one of those that drifted.

**The sample-drift guard reaches the document an executor is told to build
against.** `documented_samples.rs` scanned `README.md` and `wiki/*.md`;
`docs/schemas/` had never been compared with the binary. The marker convention
gained `at=<JSON Pointer>` for a block that is part of an output and `exit=<n>`
for one that documents a refusal — `plan.v1.md`'s whole-plan sample exits 4, so
the old `status.success()` assertion could not have checked it at all.

**A removal can be verified.** `VERIFY_REMOVAL` carried `PACKAGE_ABSENT` and no
command since the planner was written — the only step in a plan asserting
something an executor had no way to test.

**Detection beats stored knowledge for a read.** INV-20's second sentence had
four artefacts asserting it and no implementation. `Capability::UnprivilegedRead`
is detected from a read each adapter already performs.

**And a refusal names its cause.** A provider under `$HOME` is redacted by
capture and then refused by the planner for having no absolute program — told,
in effect, that the user's input was at fault.

## What the gates found the first time they ran

Each of these had been true for some time and was invisible:

- `policy.toml` eleven days stale, and `CHANGELOG.md` with it.
- `doctor.v1.md` naming `path_duplicate`, **a finding this build does not
  emit**, with a fabricated `params.entry`; `"tier": "Degraded"` where the
  binary writes `DEGRADED`, in the document that *is* that enum's contract; two
  stale read dates; a policy version two bumps behind.
- Three more stale policy versions in `wiki/FAQ.md`, `wiki/Getting-Started.md`
  and `analysis.v1.md` — one of them in the same file whose other copy this pass
  had already corrected.
- `AGENTS.md` naming `crates/policy/src/engine.rs` as where the engine's
  invariant tests live. That file has never held one.
- Three `_gui_app` counts stale about two different quantities.
- F-43's preamble reading as open when all three items were fixed; F-50's row
  denying its own first sentence.

## What this pass got wrong, and what caught each one

Six times. Listing them is the point of the report.

1. **The `prefix_redacted` term in the new refusal was untested.**
   `spec-guardian` deleted it and every test still passed: a redacted *absolute*
   prefix never reaches that guard. The pairing `doctor` needs is not the
   pairing that site needs. Resolved by removing the branch and pinning its
   premise — `is_safe_captured_path` admits only absolute or `~`-rooted — in the
   crate that owns it.
2. **The marker convention turned markdown into unconstrained argv.**
   `security-reviewer`: two verbs on this binary write files, so one line in any
   scanned document made `cargo test` replace a file at any path the test user
   can write. Older than the change that found it, and widened by it.
3. **`at=`/`exit=` were stripped wherever they appeared**, so one could be the
   value of a preceding flag and the argv run would differ from the argv shown —
   the drift F-27 exists to stop, arriving through the instrument built to catch
   it.
4. **The absence semantic's safety argument was factually wrong.** It said a
   truncated read "produces an empty stdout". It does not — `read_bounded`
   returns a *prefix* and sets a flag. A `brew list --versions` cut off at line
   40 of 201 is non-empty, omits the name, and would have certified a removal it
   never saw, past `REMOVE_SOURCE` where the package is already gone.
5. **The invariant test for that semantic passed under three mutations.**
   Swapping the source provider for the destination certified a removal from one
   provider by reading the *other's* inventory, where the name is guaranteed
   missing. Narrowing the verb to `--formula` answered non-empty while
   structurally omitting what it does not list. One assertion —
   `!argv.is_empty()` — was unfalsifiable on every reachable path: the F-19 class
   inside the test written to close an F-19-class hole.
6. **The F-35 override was keyed on the operation and the stored value on the
   package kind**, so a detected capability would have overridden a
   `verify_gui_app` cell it never consulted — turning MacPorts' deliberate
   `unknown` into a confident `none` for a package kind it does not have.

The shape repeats: **an argument written down was taken for a guarantee.** What
caught every one was someone re-deriving the behaviour from the code or running
a mutation, never re-reading the prose.

## What was found on the way

**F-51 — "the name appears in stdout" was a substring rule, on the step INV-01
rests on.** Measured on this machine: of 201 installed formulae, **18 are a
substring of another's name** — `gh` in `ghidra`, `highway` and `libnghttp2`;
`apr` in `apr-util`. `STDOUT_NAMES_PACKAGE` guards `VERIFY_DESTINATION`, which
INV-01 puts in front of `REMOVE_SOURCE`, so a *failed* install of `gh` was
certified by `ghidra`'s line. Both semantics now require the name to equal the
**first whitespace-separated token** of a line, literally. A prefix test does not
close it — `ghidra` begins with `gh` — and a regex cannot be used, because `.`
and `+` are legal name characters.

**F-40 inventoried.** "Move the in-`src/` test modules" is a three-way split, not
a two-way one: 62 modules, 346 tests, of which **35 modules and 60% of the
tests cannot move without changing production visibility**. One cannot move at
any price — `process/mod.rs` uses `#[cfg(test)]` to widen the *spawn allowlist*,
so moving its tests would silently change what they assert rather than fail to
compile. The achievable piece is 27 modules, which is its own pass.

## What is deliberately not done

- **F-47's two residual holes** and **F-40's denominator** need
  `scripts/check-links.sh` and `scripts/check-coverage.sh`. Both are
  guard-protected and `MPB_ALLOW_GUARD_EDIT` was not set. `CLAUDE.md` makes that
  a question for the human, not an obstacle to route around.
- **F-23's routing** needs a pre-pass over argv before `Cli::parse()` and
  touches a guard-protected exemption comment.
- **F-04's residue** is deferred by design (SE-056) to the executor pass and
  pinned by a test that says so.
- **`analysis.v1.md` and `explain.v1.md`** stay unchecked because they are
  abridged by design. Each now says so in its own text, so the widened scan does
  not read as complete coverage.
- **F-35's per-cell provenance half.** Two cells carry observations the
  entry-level `source` can only describe in prose. That is a schema change.

## Decisions filed for the owner

**DP-021** — a provider installed under `$HOME` can be analysed and never
planned. Completing it trades away an audience (users installing under `$HOME`
for want of administrator rights, which overlaps the Intel users the charter is
written for) or spends the Tier 3 privacy MUST. `AGENTS.md` makes that the
owner's, and it had been carried in the register as "a design question" for
three days without being written where such questions live. The reversible
default is taken and complete; the entry separates what has landed from what
awaits a ruling, and records the identifier question (`InputUnusable` still
names the wrong party, and `CapabilityMissing` is exit **5**, not 8, so the
swap proposed in review is a contract change rather than a wording fix).

**DP-019** is unchanged and still open.

## Tests, and which of them discriminate

Every gate added or touched was run against a constructed failure before being
believed. Eighteen mutations in all; each is named beside the assertion it
proves. The ones worth knowing:

- Reverting the manifest fails the policy-version test, naming the file whose
  read date it missed.
- A changed `success` inside an `at=` fragment, an `exit=0` on the plan that
  exits 4, a pointer resolving to nothing, and a marker naming `--report` each
  fail with the message a reader needs.
- Reading the destination's inventory to certify a removal from the source, and
  narrowing that inventory to `--formula` or `requested`, both fail.
- Deleting the F-35 preference fails the override test; dropping its read-only
  narrowing fails the mutation test; keying it on the operation instead of the
  package kind fails the `_gui_app` test.
- `READ_OPERATIONS` no longer grades its own test: the verbs are named
  literally and the constant is asserted to equal them, because the first
  version drew both the forced cells and the asserted operations from it, so
  shrinking it shrank the test with it.

## Next in this area

1. **The executor.** Two of `plan.v1.md`'s five inherited obligations are now
   discharged as data. Item 3's remaining half is four runtime conditions and a
   privacy rule, all stated.
2. **F-40's achievable piece** — the 27 movable modules — and, with it, the
   denominator and the floors in one edit, which DP-015 already asks for.
3. **F-35's per-cell provenance**, while the two cells that need it are fresh.
4. **F-47 and F-23**, when the guard is open.
