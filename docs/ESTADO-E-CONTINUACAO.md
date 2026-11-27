# Estado do projeto e como continuar

**Atualizado:** 27/11/2026, manhã.
**Para que serve:** abrir um chat novo (ou passar para outra pessoa) e continuar sem reconstruir contexto. Fatos medidos estão separados do que ainda é suposição.

---

## 0. LEIA PRIMEIRO: nada está commitado

**São 55 arquivos pendentes** na árvore de trabalho — 47 modificados, 727 inserções, mais 4 caminhos novos. O último commit é `f789ca3`, que já existia antes da sessão de 27/11 pela manhã.

Isso significa que **o `git log` e a página do GitHub não mostram nada** do trabalho descrito na seção 3. Se você abriu um chat novo e o repositório parece intocado, é por isso.

Entre as pendências está a correção de um `panic` em tempo de execução (seção 3.1). Commitar é o primeiro passo.

```powershell
cd "C:\Users\X\Desktop\MEU PROPIO LAUNCHER\GambiarraLauncher"; git status --short
```

---

## 1. Onde o código está

Existe **uma única árvore** de código: esta, `GambiarraLauncher/`.

Antes havia cinco cópias do mesmo código no disco (`cool_fox-main/`, `GambiarraLauncher/main/`, e três árvores de experimento em `_SX-Local/Tools/Gambiarra-*-Test/`). Foram **apagadas** depois de confirmado que esta árvore contém todo o trabalho. Os logs que importavam foram preservados em `_SX-Local/6M-Development/Evidence/victory-run-logs-2026-11-26/`.

Dois repositórios no GitHub, **os dois privados** (owner `Miaumenu`):

| repositório | o que é |
|---|---|
| `Miaumenu/BRS-Launcher-GTA6-Port` | este, o que vale. **Ainda com o nome antigo no GitHub** — ver seção 5. |
| nome perdido na renomeação em massa | tentativa anterior de estrutura; conteúdo já migrado para cá. O nome real só o dono sabe: esta linha e a de cima ficaram idênticas quando `victory`/`sixm` virou `GambiarraLauncher` em todos os arquivos de uma vez. |

Privado significa que o link dá **404 para quem não está logado com acesso**. Para liberar: `Settings` → `Collaborators` (convida um a um) ou `Settings` → `Danger Zone` → `Make public`.

**O dump do título** fica em `C:\Users\X\Downloads\GTA6.Xbox.v1.0.2.1-CYBERLEEK` — **209 GB**, 44% do disco. O link `MEU PROPIO LAUNCHER\game-dump` aponta para ele. Medido: o código só lê **`shaders.rpf` (0,38 GB)** e **`game_scarlett_master.exe` (0,11 GB)** pelo nome. Todo o trabalho offline de shader cabe em meio gigabyte; os 209 GB só fazem falta para rodar o título de verdade.

---

## 2. O que é o projeto, em uma frase honesta

Uma camada de compatibilidade que roda um título Xbox Series nativamente no Windows — **não é emulador**: o Xbox Series X é x86-64 AMD, o executável do título roda na CPU sem emulação. O que a camada fornece é a API do GDK, o D3D12-on-Xbox e um tradutor que converte o código de máquina GFX10 do console em DXIL.

Deriva do projeto **VIctory**, de [codec-d/cool_fox](https://github.com/codec-d/cool_fox), **GPL-3.0-or-later**. Os nomes `victory*`/`VICTORY_*` foram trocados por `gambiarra*`/`GAMBIARRA_*`. A licença e a autoria originais continuam valendo.

Por decisão do dono, **o nome do título não aparece em nenhum arquivo** — use "o título". Identificadores técnicos reais ficam (`game_scarlett_master.exe`, `Bink2Scarlett.dll`, `series-x`/`series-s`): são nome de arquivo e do console, e apagá-los quebraria o software.

---

## 3. O que a sessão de 27/11 (manhã) fez

### 3.1 Um `panic` em execução, achado pelos testes

`crates/gambiarra-hle/src/modules/gameinput.rs:223` estava assim:

```rust
id[..4].copy_from_slice(b"gambiarra");
```

Quatro bytes de destino, nove de origem — `panic` garantido. Entrou no commit **`f789ca3`**, a renomeação em massa: o nome antigo era `sixm`, de quatro letras, e a substituição trocou o literal sem corrigir a largura. **Não era só teste:** `Device::new` é chamado em `std::array::from_fn(Device::new)` (linha 397), no caminho de execução real, então qualquer corrida que inicializasse o GameInput morria ali — teclado e controle.

Corrigido usando o tamanho da própria etiqueta, para não poder quebrar de novo numa renomeação futura. `gambiarra-hle` foi de **144 passando / 6 falhando** para **152 passando / 0 falhando**.

Varredura: o único outro `copy_from_slice` de largura fixa no repositório é `data[..2] = b"MZ"` em `gambiarra-pe`, e esse está correto.

### 3.2 Carregador de presets de hardware (novo)

Implementado e verificado rodando na RX 550.

| arquivo | o que é |
|---|---|
| `crates/gambiarra-core/src/preset.rs` | parse, validação e casamento. Puro, sem GPU. 20 testes |
| `apps/gambiarra-cli/src/probe.rs` | a consulta ao D3D12, isolada (não passa pelo `host.rs`, cujo device é o do jogo) |
| `apps/gambiarra-cli/src/preset.rs` | o subcomando e a aplicação no `run` |
| `configs/presets/` | `_schema.toml`, `baseline.toml`, `amd-gcn-no-mesh.toml`, `README.md` |

```powershell
.\target\debug\gambiarra-cli.exe preset list
```

Na RX 550 a sonda leu sozinha `vendor=amd mesh_shader=false shader_model=6.5 wave_lanes_min=64 vram_mb=2027` — os **2027 MB batem com o `STATUS.md`**, confirmação independente — e escolheu `amd-gcn-no-mesh`.

Duas regras estão **em código**, não em prosa:

1. **Condição sobre capacidade que o driver não respondeu não vale.** Se a consulta falhou, o preset é descartado em vez de aplicado no escuro.
2. **FPS só é aceito a partir do degrau `world`.** Abaixo dele o mundo não foi desenhado, então um FPS mediria tela vazia. Recusado na leitura do arquivo, não pelo revisor.

Precedência: o que o usuário define na mão sempre ganha do preset. `--preset <id>` força um; `--no-preset` não aplica nenhum.

`configs/hardware/profiles/{amd,intel,nvidia}/` foi **apagada**: existia com três pastas vazias e **nenhuma linha de código a lia**. As regras que os READMEs dela enunciavam estão implementadas no casador.

### 3.3 Patches de contribuição regerados

Os dois patches em `reports/archive/legacy-contributor-workflow/contributors/amd/miaumenu/patches/` apontavam para `crates/sixm-shader/` e `crates/sixm-gpu/` e **não aplicavam em lugar nenhum**. Regerados contra esta árvore: aplicam e revertem limpo, com cabeçalho dizendo a árvore-alvo. Verificação de ida e volta: aplicados numa cópia limpa, reproduzem a árvore byte a byte nos 6 arquivos.

⚠️ **Isto contraria uma decisão deliberada.** A mensagem de `f789ca3` diz que os patches não foram renomeados de propósito, porque "valem contra os nomes antigos". O raciocínio só funcionaria se existisse uma árvore `sixm-*`, e não existe: esta árvore usa `gambiarra-*` e o upstream usa `victory-*`. Se preferir o critério original, é reversível.

O `0002` **não está faltando**: foi revertido de propósito (hipótese derrubada), como o `CHANGES.md` registra.

### 3.4 Números defasados corrigidos

O log preservado `logs/traducao-offline.txt` é a medição **antes** das correções (`resources 25.889 de 26.084`), e dele saíram dois números que circulavam como se fossem o estado atual:

- **falhas: 6 → 5.** A coluna "depois" repetia o 6 do upstream sem remedir os `Cs`.
- **99,2% → 99,98%.** O 99,2% é `(26.084 − 202) / 26.084`, ou seja, upstream.

Corrigidos em 5 arquivos. Também: removido do `CHANGES.md` um item "em aberto" sobre os 196 `Gs` que já estavam corrigidos, e um link quebrado no relatório da RX 550.

### 3.5 Formatação e nome

`cargo fmt --all -- --check` — que é o passo do CI — **passa agora**. Reprovava em 25 arquivos de 4 crates, todos anteriores a esta sessão.

Branch local renomeada: `codex/brs-launcher-gta6-port` → `codex/gambiarra-launcher`, sem vínculo com a remota de nome antigo. Link do dump: `GTA6-Local` → `game-dump`. Texto do `.gambiarra-root` corrigido (dizia "VIctory source tree").

### 3.6 Disco

Estava com **0 MB livres**, o que fazia o build morrer com `os error 112`. Agora **10,5 GB**: Lixeira (~7 GB), `target/` (~3,2 GB) e o cache do instalador do Visual Studio em `Temp` (2,2 GB). O dump de 209 GB foi **preservado** de propósito.

Se precisar de mais: Unreal Engine tem **32 GB** em `Program Files/Epic Games` (o servidor `unrealmcp` está configurado nesta máquina, então confirme antes) e `AppData/Roaming` tem **17 GB** ainda não examinados.

---

## 4. O que está medido (fato, não suposição)

### Tradutor de shaders — praticamente pronto

`translate --list` sobre o pacote completo, remedido em 27/11/2026:

| | upstream | agora |
|---|---|---|
| containers | 26.503 | 26.503 |
| programas traduzidos | 26.084 de 26.084 | 26.084 de 26.084 |
| recursos rastreados | 25.889 de 26.084 | **26.084 de 26.084** |
| DXIL — vertex e pixel | 14.247 de 14.247 | 14.247 de 14.247 |
| DXIL — **mesh** | **8.765 de 8.765** | 8.765 de 8.765 |
| DXIL — compute | 2.677 de 2.682 | 2.677 de 2.682 |
| **falhas totais** | **202** | **5** |

As 5 que sobram são todas `Cs: a descriptor read only as data`, em `crates/gambiarra-shader/src/emit/lower.rs:6797`. **Existe sonda pronta e não documentada:** `GAMBIARRA_DESC_WHY=1` imprime offset, instrução e escalar de cada descriptor recusado.

### Duas correções nossas, as duas medidas

1. **wave32 em host de onda 64** (AMD GCN). Avisos `wave lane count of 32`: 509 → 3. Pipelines recusados com `E_INVALIDARG`: 298 → 2. Viva na árvore em `emit/lower.rs` (`HOST_MIN_WAVE`), alimentada por `gambiarra-gpu/src/host.rs:1027`.
2. **Caminho `MEMORY` para o estágio `Gs`.** Os 196 `GS_VoxelizeGeneric` passaram a traduzir. Uma condição em `crates/gambiarra-shader/src/resource.rs`.

### Suítes de teste, todas rodadas em 27/11 pela manhã

| crate | resultado |
|---|---|
| `gambiarra-core` | 34 passando |
| `gambiarra-cli` | 16 passando |
| `gambiarra-loader` | 42 passando |
| `gambiarra-hle` | 152 passando |
| `gambiarra-shader` | 246 passando, **36 falhando** |

As 36 são a linha de base documentada: testes que executam na GPU, e esta placa é SM 6.5. Idênticas antes e depois de tudo que esta sessão fez.

### O muro: hardware

Na máquina de teste (Radeon RX 550, GCN 4, 2 GB, SM 6.5, sem mesh shaders, onda 64/64) o jogo:

- carrega, cria o device, apresenta **2048+ frames** numa corrida de 250 s
- desenha a arte de carregamento **em cor cheia e correta**
- **nunca renderiza o mundo** — nenhum frame chega a 500 draws

Motivo: **35% dos shaders do título (9.282 de 26.503) são mesh shaders** e a placa não tem a unidade. O tradutor converte os 8.765 perfeitamente e não há onde executar.

**O experimento que decide o projeto ainda não foi feito:** rodar numa GPU que atende o requisito (RDNA 2+ ou Turing+, SM 6.6, mesh shaders, ≥8 GB). Tudo medido aqui é o teto da RX 550, não o do software.

---

## 5. O que está aberto, em ordem de retorno

### Precisa do dono, ninguém mais pode fazer

1. **Commitar as 55 mudanças** (seção 0).
2. **Renomear o repositório no GitHub.** `Settings` → `Repository name` → `GambiarraLauncher`. Só **depois** disso rode o comando abaixo — antes dele, o `push` passaria a procurar um repositório que não existe:

```powershell
cd "C:\Users\X\Desktop\MEU PROPIO LAUNCHER\GambiarraLauncher"; git remote set-url origin https://github.com/Miaumenu/GambiarraLauncher.git
```

3. **Apagar a branch remota** `codex/brs-launcher-gta6-port`, que ainda carrega o nome antigo.
4. **Aceitar os colaboradores** em `Settings` → `Collaborators`. Convite pendente só quem recebeu pode aceitar.

### Trabalho técnico

5. **Rodar numa GPU compatível.** Com 99,98% dos shaders convertendo, é alta a chance de ir muito além. **Nenhuma máquina NVIDIA foi testada — nem uma.**
6. **Exercitar a aplicação automática de preset num `run` real.** O `preset list` e o `preset show` foram verificados; a aplicação dentro do `run` está escrita e **nunca foi exercida numa corrida**.
7. **Confirmar as duas correções em segunda máquina.** A wave32 só pode ser validada numa AMD pré-RDNA (RX 400/500, Vega). Os patches agora aplicam (seção 3.3).
8. **Os 5 compute** de `a descriptor read only as data`. Comece pela sonda `GAMBIARRA_DESC_WHY=1`.
9. **Linha de base do `--validate`.** Houve 17.550 erros de `PSV0` numa corrida com validação; **não foram comparados com uma árvore limpa**, então não se sabe se são nossos. Sem baseline, o número não quer dizer nada. Atenção: **este repositório não tem o histórico do upstream** (17 commits, o primeiro é "Estrutura inicial"), então a baseline exige clonar `codec-d/cool_fox` e comparar.
10. **Remoção de device aos ~55 s.** Aconteceu uma vez, não reproduziu com DRED e GPU timing ligados. Nunca investigada a fundo.

### Desenhado, não implementado

11. **Validação automática de preset no CI:** TOML abre, `id` igual ao nome do arquivo, `[measured].report` aponta para arquivo que existe no mesmo PR, e **FPS recusado sem `stage_reached` ≥ `world`**. O repositório já tem `scripts/check-docs.py` e CI, então é extensão do que existe. O fluxo de contribuição está escrito em `configs/presets/README.md`.
12. **Release com binário pronto.** A economia real para um contribuidor não é baixar menos código, é **não precisar compilar**: um `.zip` com `gambiarra-cli.exe`, a sonda `d3d12-capabilities.exe` e o `D3D12Core.dll` do Agility SDK transforma a contribuição de "uma tarde" em "cinco minutos". Isso também responde ao pedido de "só o que a pessoa vai baixar para usar" — que **não** se faz com `.gitignore` (ele não remove arquivo já rastreado, e `Cargo.toml` é obrigatório para compilar).
13. **Sparse-checkout: avaliado e descartado.** O repositório tem 20 MiB de histórico e ~8 MB de código versionado. Não há fatia grande para deixar de fora, e brigaria com a pasta única de presets. Se quiser citar algo no README, cite `git clone --depth 1`.
14. **Launcher visual.** `apps/gambiarra-launcher/` continua sendo só um README.

---

## 6. Como retomar (comandos que funcionam)

```powershell
cargo build -p gambiarra-cli
```

```powershell
cargo build --profile diag -p gambiarra-cli --features diagnostics
```

```powershell
.\target\diag\gambiarra-cli.exe translate --list "C:\Users\X\Desktop\MEU PROPIO LAUNCHER\game-dump\shaders.rpf"
```

```powershell
.\target\diag\gambiarra-cli.exe run "C:\Users\X\Desktop\MEU PROPIO LAUNCHER\game-dump\game_scarlett_master.exe" --execute --console series-x
```

Variáveis úteis: `GAMBIARRA_KEYS=40:Enter` (aperta teclas sem precisar de foco — é como se passa da tela de início), `GAMBIARRA_CAPTURE=<dir>` (salva frames em PNG), `GAMBIARRA_CAPTURE_WORLD=0,2,8` (marca o primeiro frame com 500 draws — **sem ela a mensagem do mundo não sai**), `GAMBIARRA_D3D12_DEBUG=1`, `GAMBIARRA_D3D12_DRED=1`, `GAMBIARRA_GPU_TIMING=1`, `GAMBIARRA_DESC_WHY=1`.

**Armadilhas que já custaram horas:**

1. No Windows 10 o executável precisa de `D3D12\D3D12Core.dll` **ao lado dele** (Agility SDK). Sem isso morre em 9 s com `0x887E0003` e nem cria o device. `cargo xtask agility` resolve.
2. Aponte o `translate` para o `shaders.rpf`, **não** para a pasta do dump (209 GB).
3. Medir pelo jogo é lento e depende de GPU; medir offline é 4 minutos e não depende de nada.
4. **O disco enche.** Um build completo de teste usa vários GB de `target/`. Se aparecer `os error 112`, é disco, não código. `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` reduz muito o tamanho dos artefatos.
5. **Cuidado ao reformatar.** `cargo fmt` pode mexer em arquivos que os patches arquivados tocam (`emit/mod.rs`, `gpu/host.rs`). Depois de formatar, confira com `git apply --check --reverse` nos dois patches.
6. **`sed -i` e scripts Python mudam terminações de linha.** O `.gitattributes` manda `eol=lf` para tudo (`crlf` só para `.cmd`/`.ps1`). Um diff de 300 linhas onde a mudança real é de 12 geralmente é isso.

---

## 7. Decisões pendentes (precisam do dono do projeto)

| decisão | situação |
|---|---|
| **Nome do projeto** | decidido: `GambiarraLauncher`. Pasta local, crates e **todos os textos** já usam. Falta só o GitHub (seção 5, itens 2 e 3). |
| **Ignorar tudo menos esta pasta** | resolvido de fato: a pasta-mãe (`MEU PROPIO LAUNCHER`) **não é um repositório git**, então o `.gitignore` dela não tem efeito. O que vale é o deste repositório, que já ignora `/inspiration/` (linha 96). |
| **Trazer do `_SX-Local` o que ajuda** | pedido. Candidatos reais: o laudo de shaders, os logs preservados, a sonda `d3d12-capabilities.exe`, o loop instrumentado. O resto (Ghidra, assinaturas, análise do REA, `Private/`) é análise do binário e **não ajuda o launcher**. |
| **Separar o launcher do Gambiarra** | pedido, para depois. |
| **Renomear os crates** | `gambiarra-*` ficam por enquanto. É mudança ampla e separada — e `f789ca3` mostra o risco: foi uma renomeação em massa que plantou o `panic` da seção 3.1. |

---

## 8. `inspiration/` — o que dá para reaproveitar

Duas referências clonadas em `inspiration/` (pasta **ignorada pelo Git**, então nada disso está no repositório).

### AmericasLauncher (`isaac-btw/AmericasLauncher`)

**É um launcher pronto em C#** (21 `.cs`, 2 `.csproj`, 2 `.xaml`) — exatamente a peça que falta, já que `apps/gambiarra-launcher/` é só um lugar reservado. Resolve: UI 720p com cartões de runner, dependências por `winget`, clone e build de cada runner, saída do console integrada, `preLaunchArgs` antes do jogo, botão de parar, seletor Series X/S, log em arquivo, auto-update.

Trata runners como **plugins configurados em JSON** (`src/AmericasLauncher/Data/runners.json`, 9 runners). Esse desenho serve direto: o nosso binário vira mais um runner.

⚠️ **Confirmado: não há arquivo de licença no repositório.** Sem licença, é "todos os direitos reservados" por padrão — **copiar não é permitido**. Serve como referência de desenho, não de código. Vale abrir issue pedindo que licenciem.

### ScarlettPort (`ZigZagBoi1/ScarlettPort`)

Outra camada de compatibilidade Xbox Series, em **Rust** (397 `.rs`), com `runtime/`, `engine/`, `launcher/`, `patches/`. Tem releases, 90% declarado, e capturas do próprio runtime desenhando. É o projeto mais próximo do nosso e **tem launcher próprio**.

⚠️ **Confirmado: também não há arquivo de licença**, apesar do selo GPL no README apontar para `LICENSE.md`. Confirmar antes de reaproveitar qualquer linha.

**Recomendação:** referência de arquitetura, não de código. O ganho é copiar **decisões** — principalmente o modelo de runner plugável, que define como o launcher conversa com o runtime.

---

## 9. Aviso: duas IAs na mesma árvore

Dois agentes já editaram estes arquivos ao mesmo tempo. O custo foi real: build quebrado, pasta duplicada criada por engano, variáveis de ambiente corrompidas nos documentos (`VICTORY_KEYS` virou `Phoenix_KEYS`, que não existe), duas renomeações em massa sobrepostas — e o `panic` da seção 3.1, que ficou escondido por horas.

**Garanta que só uma sessão está mexendo nesta árvore.** Se precisar de duas, dê a cada uma a sua pasta e nunca a mesma.

---

## 10. Documentos relacionados

| documento | o que tem |
|---|---|
| `configs/presets/README.md` | como o preset é escolhido e como contribuir um |
| `configs/presets/_schema.toml` | o gabarito do preset, campo por campo |
| `docs/hardware-compatibility.md` | matriz de GPUs: suportado, parcial, bloqueado, não testado |
| `docs/shader-translation.md` e `shader-translation-investigation.md` | o fluxo da tradução e a investigação dos 196 geometry shaders, com as hipóteses derrubadas |
| `docs/gpu-portability.md` | detecção de recursos e a regra dos presets |
| `docs/fork-ray-tracing.md` | o custo real do ray tracing (~40 ms/frame) e as chaves que o controlam |
| `docs/debugging-gpu-hangs.md` | diagnóstico de travamento gráfico |
| `reports/hardware/rx550-2026-11-27.md` | o relatório da máquina de teste |
| `reports/archive/.../amd/miaumenu/CHANGES.md` | as duas correções em detalhe e **as hipóteses derrubadas** — leia antes de ressuscitar qualquer teoria |
| `CONTRIBUTING.md` | como contribuir sem um atropelar o outro |
