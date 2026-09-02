# ADR-0001 — Licence: Apache License 2.0

- **Status:** accepted
- **Date:** 2026-09-02
- **Pass:** pre-implementation (owner decision)
- **Supersedes:** DP-003 in `docs/decisions-pending.md`

## Context

MacPortsBrew is a public, permissively licensed open-source project. The
realistic candidates were MIT and Apache-2.0. The choice needed to be made
before a `LICENSE` file was committed, because relicensing after external
contributions arrive requires every contributor's agreement.

Relevant forces:

- The project drives two third-party package managers and will be forked,
  packaged and redistributed by others (potentially as a Homebrew formula and a
  MacPorts port of itself).
- It is expected to attract corporate as well as hobbyist users, some of whom
  have legal review processes that treat an explicit patent grant as a
  requirement.
- The project will accept contributions from strangers, including coding agents
  operating on their behalf, so the inbound contribution terms matter as much as
  the outbound licence.

## Decision

**Apache License 2.0.** Chosen by the project owner.

## Rationale

- **Explicit patent grant (§3).** Every contributor grants a patent licence for
  their contribution, and that grant terminates for anyone who initiates patent
  litigation over the work. MIT is silent on patents, which leaves adopters
  relying on an implied licence. This is the substantive difference between the
  two options and the reason to prefer Apache-2.0 here.
- **Inbound equals outbound by default (§5).** Contributions are licensed under
  the same terms unless explicitly stated otherwise. This removes the need for a
  contributor licence agreement, which is friction for a project that wants
  drive-by fixes to mappings and policy rules.
- **Explicit trademark reservation (§6).** Useful for a project whose name
  combines two third-party project names — it makes clear that the licence
  grants no rights in anyone's marks.
- **Clear redistribution terms (§4).** Downstream packagers know exactly what to
  ship. The cost is a small amount of ceremony versus MIT's brevity.

MIT remains a perfectly good licence and is more common in this ecosystem; it
was not chosen because it lacks the patent grant.

## Consequences

**Positive**

- Corporate adoption and redistribution are unambiguous.
- No CLA is needed; `CONTRIBUTING.md` states the inbound=outbound rule instead.
- Compatible with GPLv3 for downstream combination.

**Negative / constraining**

- **Apache-2.0 is incompatible with GPLv2-only.** Any dependency licensed
  GPLv2-without-the-"or-later" clause cannot be linked into MacPortsBrew. This
  constrains library selection — notably TUI and terminal libraries — and must
  be checked mechanically rather than by inspection. See
  `docs/spec/07-quality-and-testing.md` and research item R-7.
- Slightly heavier boilerplate than MIT.

**Deliberate omission: no `NOTICE` file.**
Apache-2.0 §4(d) only obliges redistributors to reproduce a `NOTICE` file *if
the work contains one*. MacPortsBrew currently has nothing to attribute, so
adding a `NOTICE` would create a redistribution obligation for downstream
packagers in exchange for no benefit. A `NOTICE` MUST be added the moment the
project vendors third-party code that requires attribution — and not before.

**Copyright holder.** The `LICENSE` appendix is boilerplate and is left
unmodified, which is correct and conventional. The copyright *owner* for
README, release metadata and any future `NOTICE` is still undecided — tracked as
DP-006. Source files carry an SPDX identifier only, with no per-file copyright
line, so this decision does not block implementation.

## Compatibility impact

None on macOS versions, architectures or provider versions. Material impact on
dependency selection, as described above.

## Revisit when

The project vendors third-party code (add a `NOTICE`); or a dependency the
project genuinely needs is GPLv2-only (re-open the analysis rather than quietly
violating the licence).
