<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 12 — read-only improvements after 0.1.0: report

Work order: [`prompts/12-read-only-improvements.md`](../../prompts/12-read-only-improvements.md).
Branch `pass/12-read-only-improvements`. `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`.

## Progress

Log kept for resuming after a compaction: read this and `git log --oneline -15`.

- 0 work order commit: done 917a678
- A pt-BR catalogue: done 3789749; stale docs in CONTRIBUTING.md, `.claude/rules/locales.md` and the add-locale skill are left to F
- B dual-installation doctor check: done (commit follows this line in git log: `feat(adapters,application,domain): doctor reports ... (item B)`)
- C ownership default marker: todo
- E layering test fixes: written and mutation-proved, uncommitted (waits for reviewers)
- D curated mappings: todo
- F record and report: todo

## Notes for the owner (written as they happen)

- The commit gate runs the full suite whenever any file under `.claude/` is dirty
  (`commit-gate.sh` path list), and the suite takes about 16 minutes on a quiet
  machine against a 1380 s deadline. With the machine at load average 15-34
  (unrelated `firebase` node processes and an Xcode toolchain process) two
  commits were refused for time. The first commit of this pass needed three tries.
