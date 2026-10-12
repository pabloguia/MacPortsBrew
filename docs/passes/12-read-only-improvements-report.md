<!-- SPDX-License-Identifier: Apache-2.0 -->

# Pass 12 — read-only improvements after 0.1.0: report

Work order: [`prompts/12-read-only-improvements.md`](../../prompts/12-read-only-improvements.md).
Branch `pass/12-read-only-improvements`, from `0b9b04e`. `THIS_BUILD_DISPATCHES_MUTATIONS`
stays `false`. Nothing was pushed, tagged or published.

## Resumo para o dono (em português)

**Commits feitos** (seis commits de trabalho, cada um aceito pelo gate de commit; o sétimo, que contém esta documentação final, é o último da branch: `git log -1`):

| Commit | O quê |
|---|---|
| `917a678` | ordem de trabalho do passo 12 |
| `3789749` | **A** — catálogo `pt-BR` ligado a `--language`, `MACPORTSBREW_LANG`, `LC_ALL`, `LANG` |
| `f233e72` | **B** — `doctor` avisa quando um gerenciador está instalado em dois lugares |
| `56e1dbe` | **E** — o teste de camadas checa o que diz checar; teste de rede renomeado |
| `e0c6e63` | **C** — `explain` diz quando "dependência" foi um valor padrão |
| `976d488` | **D** — 19 mapeamentos curados novos, política `2026.10.12-1` |
| o último da branch (`docs: …`) | **F** — CHANGELOG, README/wiki, correções da revisão final, este relatório |

**Decisões tomadas por mim** (todas reversíveis, todas em `docs/spec-evolution.md`):

- **B**: o campo novo mora em `EnvironmentSnapshot.installations`, e não em `ProviderSnapshot`
  nem em `Snapshot`, porque um campo novo ali quebraria literais de struct em
  `planner.rs` e `executor/tests.rs` (§3 proíbe tocar). Desconhecido nunca vira "um":
  replay, snapshot antigo e "nenhum candidato" dizem "não sei" (`OK`); dois ou mais é `WARNING`.
- **C**: o marcador está em `PackageReport.ownershipSplitDefaulted`, ao lado de `rows`, e não
  dentro de `OwnershipEvidence` (mesma razão: literais em `planner.rs`). Fica **devido**
  movê-lo para dentro da evidência quando o planner puder ser aberto.
- **D**: removi a linha `qt` → `qt6` depois da revisão (nome sem versão para porta numerada
  é o padrão que a linha `node@22` diz estar errado) e reescrevi seis justificativas que
  diziam que a heurística não enxerga a homepage (ela enxerga, só não chega a `EXACT`).
- **A**: o catálogo foi redigido por um modelo e revisado por outros dois (docs-auditor e
  spec-guardian, em `opus`); **nenhum falante nativo revisou**.
- Uma regra que eu tinha criado (recusar chave de instalações de provedor ausente do
  snapshot) foi **removida** porque o gate pegou que ela transformava a recusa do plano
  ("provedor removido") em "snapshot inutilizável".

**Decisões que são suas** (em `docs/decisions-pending.md`, com a opção reversível aplicada):

- **DP-040** — quem revisa o `pt-BR` e se ele sai assim (recomendo (a): sai, com revisão nativa
  das chaves `error.*` e `doctor.*`).
- **DP-041** — o texto "MacPorts no PATH" que a seção 04 pede, dado o INV-17 (recomendo
  (b) quando quiser: dado datado, nunca ramo por nome).
- **DP-042** — buscar no catálogo inteiro do outro provedor (§7): precisa de leituras que a
  allowlist de processos não permite; recomendo (b), uma consulta por pacote, sob demanda,
  depois da trilha que muda a máquina; nada foi adicionado.
- **DP-043** — um "as-of" separado para linhas lidas de índice local: as linhas novas
  empurraram `policyReadOn` para 2026-10-12 e adiaram ~5 semanas o aviso de dados antigos
  dos fatos de suporte do Homebrew Intel (lidos em 2026-09-02, não relidos). Recomendo (b).
- **DP-044** — resolvido por você em 2026-10-12: as mudanças entram no 0.1.0; o CHANGELOG já
  foi fundido em `[0.1.0]`. Também preparei o número de build de CI (ver abaixo).
- **DP-040**: o padrão que apliquei é o menos conservador (a skill diz para não traduzir
  por máquina as frases de remediação e seguir); a opção (b) custa um arquivo e está
  descrita ali.

**O que ficou bloqueado, devido ou incompleto:**

- **Nenhum golden carrega os campos novos** (§3 pede "visível em um golden verificado"):
  nenhuma gravação capturada responde diferente (todas respondem dependências reversas) e
  fixture não se inventa. A cobertura de ponta a ponta de **C** vem de um teste que adultera
  um snapshot capturado (`ownership_default.rs`); a de **B** vem do exemplo verificado em
  `doctor.v1.md`. Fica devida uma gravação capturada de um provedor que não responde
  dependências reversas, e um exemplo de `snapshot` verificado.
- **Skill `add-locale` e a cópia gerada em `.agents/skills/`** continuam dizendo que só o
  pseudolocale é ligado e que `pt-BR` é alvo a fazer. Corrigi o `CONTRIBUTING.md` e
  `.claude/rules/locales.md`; o skill não, porque o espelho gerado fica num caminho guardado
  (§3). Precisa de escrita armada sua e de `scripts/generate-codex-layer.sh`.
- `doctor --deep` e o texto "MacPorts no PATH" (DP-041), a "contagem de propriedade
  desconhecida" que inclui linhas só-mapeadas (agora maior; 10→12 num golden), `qt` fora da
  tabela, e a **gravação de macOS 10.12–11** para a arquitetura (§7) — todos continuam devidos.
- O campo `authoritative` de uma dependência padrão continua `true` (a gravação de intenção
  respondeu); só o marcador diz que a divisão não foi respondida.
- As linhas novas da tabela citam **índices locais** (homebrew-core de 2026-09-02, árvore de
  portas do MacPorts de 2026-09-06), dizem isso em cada `source`, e **empurraram `policyReadOn`
  para 2026-10-12**: o aviso de "dados antigos" foi adiado ~5 semanas também para fatos de
  suporte mais velhos. Está na SE-097; um "as-of" separado para linhas de índice é decisão sua.

**O que preciso de você:** (1) DP-040, DP-041; (2) 0.1.0 com ou sem estas mudanças;
(3) escrita armada para o skill `add-locale` + `generate-codex-layer.sh`; (4) olhar o DP-038
(dados pessoais no histórico) antes de publicar — não mexi; (5) o gate de commit.

**Um deslize de fronteira, desfeito:** um script meu, escrito para corrigir literais de
struct, editou `crates/application/src/executor/tests.rs` (caminho protegido do §3); eu
reverti com `git checkout` e coloquei uma trava no script — foi o que me fez colocar o campo
onde está. **Número de build (DP-044):** o binário aceita `MPB_BUILD_NUMBER` e `--version` imprime
`build <n>` (valor inválido quebra a compilação). **Falta, e é seu:** o script e o workflow de
release (caminhos guardados) precisam passá-lo, p. ex. `${{ github.run_number }}`.

Também toquei `.github/ISSUE_TEMPLATE/*.yml` (só o exemplo de versão da política),
que não está na lista guardada. O diff líquido de `0b9b04e` é vazio em todos os caminhos do §3.

**O gate de commit (importante para a próxima sessão):** `commit-gate.sh` roda o modo padrão
(a suíte inteira) sempre que há mudança em `crates src tests fixtures locales data scripts
.claude Cargo.*` — no primeiro commit foi o `.claude/current-pass`, nos demais, `crates/`. A
suíte leva ~16–35 min nesta máquina. **Medi ~27 s por binário de teste recém-compilado,
mesmo para testes de <1 s (~50 binários); minha hipótese, não provada, é a varredura do
XProtect do macOS (o processo tinha 15 min de CPU)**, contra o prazo de 1380 s do hook. Duas tentativas de commit foram recusadas por tempo e a sessão
caiu duas vezes durante o hook (a chamada fica presa até o fim). O que funcionou: rodar `scripts/validate.sh` à mão em
background antes (aquece os binários) e só então `git commit`. Não usei
`MPB_SKIP_COMMIT_GATE`. Havia também carga alta (load average 15–34) por processos
`firebase`/Xcode estranhos ao projeto numa parte da noite; não os toquei.

## Progress

- 0 work order commit: done 917a678
- A pt-BR catalogue: done 3789749
- B dual-installation doctor check: done f233e72
- E layering test fixes: done 56e1dbe
- C ownership default marker: done e0c6e63
- D curated mappings: done 976d488 (19 rows, policy 2026.10.12-1)
- F record and report: done (the last commit of the branch)

## What changed

**A.** `locales/pt-BR.toml` (320 keys, same keys as `en.toml`), embedded and laid over English by
`Catalogue::portuguese_brazil`; `Messages::for_locale` selects it. `crates/cli/tests/locale_pt_br.rs`
proves the four selectors, their order, `--json` unchanged, and that nearly every key is translated;
the overlay fallback is a unit test; key parity is `scripts/check-i18n-parity.sh`. Mutant killed:
`pt-BR` compared as another tag. Reviewed by `docs-auditor` and `spec-guardian` (opus): nine and five
findings, fixed or recorded (SE-011 addendum, DP-040).

**B.** `crates/adapters/src/installations.rs` searches every candidate prefix of
`provider-prefixes.toml`; `Installations` (`READ` / `UNKNOWABLE` / absent) in the domain, recorded in
`environment.installations`; `ProviderRead::installations` with a default; `doctor` ids
`provider_installed_once` / `_more_than_once` / `provider_installations_unknown`, naming no provider
and no path, and naming the copy this tool runs. Mutants killed (nine, listed in SE-095), including the
`is_this_machine` guard under the INV-15 test. The other 04 "Cross-provider contamination" MUSTs: all
have a check and a test; `build_variable_names_prefix` had **no test** and has one now; the
MacPorts-on-PATH wording is owed (DP-041); `doctor --deep` is a SHOULD and owed. Reviewed by
`spec-guardian` and `security-reviewer` (opus): no High or Medium; Lows fixed (home redaction at
capture, duplicate prefixes refused, running copy named, INV-15 test extended).

**C.** See SE-096. `PackageReport.ownershipSplitDefaulted`, `explain` line (en, pt-BR), schema row,
`ownership_default.rs`. Three mutants killed. Reviewed by `spec-guardian` (opus).

**D.** See SE-097. Nineteen rows; policy `2026.10.12-1`; goldens regenerated on a fresh binary and
compared semantically (only version/date, summary counts, and the units the new rows map changed);
samples in README/wiki/schemas regenerated; one ambiguity test moved from `python@3.13` to `3.14`/`3.15`.
Reviewed by `spec-guardian` and `docs-auditor` (opus): every row re-verified; violations fixed.

**E.** `layering.rs`: only `#[cfg(test)] mod` blocks are skipped (lexer-aware), `adapters::<provider>`
(nested groups, globs refused) is refused, provider names come from the data; the network test is
`invariant_14_no_network_crate_is_locked_and_no_socket_type_is_named_in_the_source` and says it is a
proxy. Six mutants killed. Reviewed by `spec-guardian` (opus); six findings fixed.

**F.** CHANGELOG `[Unreleased]`; README and wiki (languages, `doctor` finding, ownership table);
`CONTRIBUTING.md`, `.claude/rules/locales.md`, `docs/spec/06-cli-and-ux.md` and `AGENTS.md` where they
said translations were absent; SE-095, SE-096, SE-097 and the SE-011 / SE-094 addenda; DP-040..044.
`CONTRIBUTING.md` and the `add-locale` skill now disagree on step 5 until the skill and its
generated copy are updated (owed; the "same source" claim in `AGENTS.md` is false until then).

## Boundary (§3) and gates

- `git diff --stat 0b9b04e..HEAD` is empty on `crates/cli/src/executor/`,
  `crates/application/src/executor*`, `execution_gate.rs`, `crates/domain/src/verification.rs`,
  `crates/application/src/planner.rs`, `crates/adapters/src/process/`, `Cargo.lock`, `.claude/hooks/`,
  `.claude/settings*.json`, `scripts/`, `.github/workflows/`, `.codex/`, `.agents/`.
  (I did touch `.github/ISSUE_TEMPLATE/*.yml` placeholders and `.claude/rules/locales.md`; neither is on
  the list.) One slip was caught and undone in the same minute: a script I wrote to fix struct literals
  edited `crates/application/src/executor/tests.rs`; I reverted it with `git checkout` and added a
  guard to the script, which is why the new field lives where it does.
- **§3 "visible in a checked golden" is not met** for `installations` and
  `ownershipSplitDefaulted` (no captured recording exercises them; recorded in SE-095/096).
- Coverage floors were **not measured** in this pass (no stage of `validate.sh` measures them; CI does).
- No new dependency, no `unsafe`, no network, no process-allowlist change.
- `scripts/validate.sh` passed for every commit (the commit gate runs it). No `MPB_SKIP_*` escape used.

## Not in this pass (§7)

Unchanged and still the owner's or the mutating track's: searching the other provider's whole catalogue,
telling a refused spawn from a missing program (`NotStarted`), pass 08 slices 4–6 and pass 11, and a
macOS 10.12–11 recording for architecture detection.
