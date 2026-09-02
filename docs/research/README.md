# Research notes

Answers to `../spec/09-research-checklist.md`. One file per item, named
`<item-id>-<slug>.md`, using the id as written in the checklist — for example `R-1-homebrew-read-interfaces.md`.

These notes are evidence, not opinion. The specification is allowed to change
because of what is written here; nothing here may be written because of what
the specification says.

## The rule

Every factual claim about Homebrew or MacPorts is exactly one of:

1. **Observed** on a real machine with a read-only command — command recorded
   verbatim, output trimmed but not paraphrased, date recorded, machine
   described.
2. **Cited** from official documentation — URL plus the date you read it.
   Official means `docs.brew.sh`, `trac.macports.org`, `guide.macports.org`,
   `ports.macports.org`, the projects' own repositories, or Apple's developer
   documentation. A blog post is a lead, not a source.
3. **Deferred to runtime capability detection**, because the answer varies by
   machine, provider version or configuration.

There is no fourth option. A note that cannot reach one of the three marks the
question `UNRESOLVED` and says what a future attempt would need.

## Template

```markdown
# <item-id> — <question in one line>

- **Status:** answered | partially answered | unresolved
- **Answered on:** YYYY-MM-DD
- **Machine:** macOS <version>, <arch>, Homebrew <version> @ <prefix>, MacPorts <version> @ <prefix>
- **Blocking:** what in the specification depends on this

## Question
What was actually asked, and why it blocks something.

## Evidence
Commands and their output, and sources with read dates. Evidence only —
no conclusions in this section.

## Findings
The conclusions, numbered, each pointing at the evidence above.

## Cannot determine
What remains open, and what would settle it.

## Impact on the specification
Which files and statements this confirms, contradicts or should change.
Do not edit `../spec/` from here — report it, and let the change go through
`../spec-evolution.md`.

## Re-check when
The condition that makes this note stale. Time-sensitive facts — support
tiers, deadlines — say so explicitly.
```

## Provenance is the point

A note without its date and its machine cannot be trusted once a provider
changes its output format, which both providers do. If you are tempted to omit
provenance because the answer is "obvious", that is precisely the note that will
mislead someone in two years.
