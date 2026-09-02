# Pass reports

One report per pass, written with the `/pass-report` skill and named
`<pass-id>-report.md`. A pass that ends without completing its work order still
gets its report — it says so in its first paragraph and names what was left
undone, because an unwritten report is indistinguishable from a pass that never
ran.

A report is the handover: to the next agent, and to a maintainer reading the
repository in a year. It is not a session summary.

Two rules make these worth keeping:

- **Every material deviation a report names already exists in
  `../spec-evolution.md`.** The report indexes; the evolution file records. If a
  deviation is explained here for the first time, the entry is missing.
- **Rejections are the most valuable section.** What a pass considered and
  decided against is what stops the next pass reopening a settled question, and
  it is the part most often left out.
