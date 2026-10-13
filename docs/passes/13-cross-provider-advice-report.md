<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 13 — direction-specific cross-provider advice, as data: report

Work order: [`prompts/13-cross-provider-advice.md`](../../prompts/13-cross-provider-advice.md).
Branch `pass/13-cross-provider-advice`, from `ae503c2`. `THIS_BUILD_DISPATCHES_MUTATIONS`
stays `false`. Nothing is pushed, tagged or published.

## Progress

- A re-read the F-2 facts, dated: done 48185dc (docs-auditor found the -I/-L filter is not always on; vocabulary changed, see Deviations)
- B data file and loader: in progress (loader, data, tests, mutants, parity test; spec-guardian reviewed)
- C neutral rule, catalogue, neutrality proof: todo
- D record and report: todo

## Deviations from the work order (D owes the records)

- **`always_on_filter` became `default_clean_path`.** The work order defined the
  Homebrew defence as the compiler shim dropping `-I`/`-L` flags on every build.
  Re-reading the source (F-2 addendum, E-8/E-9 and "Corrections") showed that
  filter only runs when argument refurbishing is on, which macOS builds do not
  enable by default; what holds for every build is that the build `PATH` is
  rebuilt. D owes: an SE entry for the change, the amendment of the sentence in
  `docs/spec/04-migration-and-safety.md` ("Cross-provider contamination", which
  still says the shim drops flags "on every source build"), and the catalogue
  wording must claim only what the addendum's "May claim" column allows.
- The accessor is `PolicyData::build_defence_of(&ProviderId)`, not
  `cross_provider_advice_for`; behaviour as specified (`None`, never a default).
- The MacPorts row's `index_as_of` is the ports-tree index date, stated in the
  row's `source`.
- CHANGELOG still says policy `2026.10.12-2`; D updates it with the pass 13
  subsection.
