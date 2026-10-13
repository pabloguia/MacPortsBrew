<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 13 — direction-specific cross-provider advice, as data: report

Work order: [`prompts/13-cross-provider-advice.md`](../../prompts/13-cross-provider-advice.md).
Branch `pass/13-cross-provider-advice`, from `ae503c2`. `THIS_BUILD_DISPATCHES_MUTATIONS`
stays `false`. Nothing is pushed, tagged or published.

## Resumo (pt-BR)

- **Commits:** A `48185dc` (pesquisa F-2 relida), B `15579e9` (dados e carregador), C `6ec5d51` (regra neutra do `doctor`, catálogo en/pt-BR), D (registros, docs, amostras).
- **Decisões tomadas:** o vocabulário passou de `always_on_filter` para `default_clean_path`, porque o filtro `-I`/`-L` do Homebrew só roda com "refurbishing" ligado (desligado por padrão no macOS); a defesa que vale para todo build é o `PATH` reconstruído. MacPorts continua opt-in (`porttrace` = `no`). Política `2026.10.12-4`. Registrado em SE-098.
- **Bloqueado/devido:** verificação de `/usr/local` não vazio, `doctor --deep`, subprocessos não-build do `brew` com `/opt/local` no `PATH`, revisão nativa do pt-BR (DP-040), DP-042.
- **Precisa de você:** nada novo. O diretório `.agents/skills/tooling-recheck/` foi apagado com sua autorização.

## Progress

- A re-read the F-2 facts, dated: done 48185dc (docs-auditor found the -I/-L filter is not always on; vocabulary changed, see Deviations)
- B data file and loader: done 15579e9 (spec-guardian reviewed)
- C neutral rule, catalogue, neutrality proof: done 6ec5d51
- D record and report: done (SE-098, spec 04, DP-041, doctor.v1 samples, CHANGELOG, wiki; spec-guardian and docs-auditor reviewed, findings fixed; policy bumped to 2026.10.12-4 after the post-review data edits)

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
- CHANGELOG now says `2026.10.12-4` and carries a Pass 13 subsection (done in D).
