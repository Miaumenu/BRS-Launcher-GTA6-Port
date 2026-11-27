# Estado do projeto e como continuar

**Atualizado:** 27/11/2026, fim da madrugada.
**Para que serve:** abrir um chat novo (ou passar para outra pessoa) e continuar sem reconstruir contexto. Fatos medidos estão separados do que ainda é suposição.

---

## 1. Onde o código está

Existe **uma única árvore** de código: esta, `BRS-Launcher-GTA6-Port/`.

Até esta madrugada havia cinco cópias do mesmo código no disco (`cool_fox-main/`, `SixMultiVersion/main/`, e três árvores de experimento em `_SX-Local/Tools/SixM-*-Test/`). Foram **apagadas** depois de confirmado que esta árvore contém todo o trabalho — liberou 1,6 GB. Os logs que importavam foram preservados em `_SX-Local/6M-Development/Evidence/victory-run-logs-2026-11-26/`.

Dois repositórios no GitHub, **os dois privados** (owner `Miaumenu`):

| repositório | o que é |
|---|---|
| `Miaumenu/BRS-Launcher-GTA6-Port` | este, o que vale |
| `Miaumenu/SixMultiVersion` | tentativa anterior de estrutura; conteúdo já migrado para cá |

Privado significa que o link dá **404 para quem não está logado com acesso** — foi o que aconteceu quando o link foi aberto em outra janela. Para liberar: `Settings` → `Collaborators` (convida um a um) ou `Settings` → `Danger Zone` → `Make public`.

---

## 2. O que é o projeto, em uma frase honesta

Uma camada de compatibilidade que roda um título Xbox Series nativamente no Windows — **não é emulador**: o Xbox Series X é x86-64 AMD, o executável do título roda na CPU sem emulação. O que a camada fornece é a API do GDK, o D3D12-on-Xbox e um tradutor que converte o código de máquina GFX10 do console em DXIL.

Deriva do projeto **VIctory**, de [codec-d/cool_fox](https://github.com/codec-d/cool_fox), **GPL-3.0-or-later**. Os nomes `victory*`/`VICTORY_*` foram trocados por `sixm*`/`SIXM_*`. A licença e a autoria originais continuam valendo.

---

## 3. O que está medido (fato, não suposição)

### Tradutor de shaders — praticamente pronto

`translate --list` sobre o pacote completo do título (26.503 containers, 44,4 M de instruções):

| | upstream | agora |
|---|---|---|
| programas traduzidos | 26.084 de 26.084 | 26.084 de 26.084 |
| recursos rastreados | 25.889 de 26.084 | **26.084 de 26.084** |
| DXIL — vertex e pixel | 14.247 de 14.247 | 14.247 de 14.247 |
| DXIL — **mesh** | **8.765 de 8.765** | 8.765 de 8.765 |
| DXIL — compute | 2.677 de 2.682 | 2.677 de 2.682 |
| **falhas totais** | **202** | **6** |

As 6 que sobram são todas `Cs: a descriptor read only as data`.

### Duas correções nossas, as duas medidas

1. **wave32 em host de onda 64** (AMD GCN). Avisos `wave lane count of 32`: 509 → 3. Pipelines recusados com `E_INVALIDARG`: 298 → 2.
2. **Caminho `MEMORY` para o estágio `Gs`**. Os 196 `GS_VoxelizeGeneric` passaram a traduzir. Uma condição em `crates/sixm-shader/src/resource.rs`.

Testes: `cargo test -p sixm-shader` → **246 passam, 36 falham**, e as 36 são `no SM 6.6 device available` (ambientais, idênticas antes e depois das duas correções).

### O muro: hardware

Na máquina de teste (Radeon RX 550, GCN 4, 2 GB, SM 6.5, sem mesh shaders, onda 64/64) o jogo:

- carrega, cria o device, apresenta **2048+ frames** numa corrida de 250 s
- desenha a arte de carregamento **em cor cheia e correta**
- **nunca renderiza o mundo** — nenhum frame chega a 500 draws

Motivo: **35% dos shaders do título (9.282 de 26.503) são mesh shaders** e a placa não tem a unidade. O tradutor converte os 8.765 perfeitamente e não há onde executar.

**O experimento que decide o projeto ainda não foi feito:** rodar numa GPU que atende o requisito (RDNA 2+ ou Turing+, SM 6.6, mesh shaders, ≥8 GB). Tudo que medimos aqui é o teto da RX 550, não o do software.

---

## 4. O que está aberto, em ordem de retorno

1. **Rodar numa GPU compatível.** Com 99,98% dos shaders convertendo, é alta a chance de ir muito além. Nenhuma máquina NVIDIA foi testada — nem uma.
2. **Confirmar as duas correções em segunda máquina.** A wave32 só pode ser validada numa AMD pré-RDNA (RX 400/500, Vega). Os patches estão em `reports/archive/legacy-contributor-workflow/contributors/amd/miaumenu/patches/`.
3. **Os 6 compute** de `a descriptor read only as data`.
4. **Linha de base do `--validate`.** Houve 17.550 erros de `PSV0` numa corrida com validação; **não foram comparados com uma árvore limpa**, então não se sabe se são nossos. Sem baseline, esse número não quer dizer nada.
5. **Remoção de device aos ~55 s.** Aconteceu uma vez, não reproduziu com DRED e GPU timing ligados. Nunca investigada a fundo.

---

## 5. Como retomar (comandos que funcionam)

```powershell
# compilar (a toolchain portátil está em _SX-Local\Tools\rust-toolchain)
cargo build --profile diag -p sixm-multiversion --features diagnostics

# medir os shaders: ~4 min, sem abrir o jogo, sem depender de GPU
.\target\diag\sixm-multiversion.exe translate --list "<dump>\shaders.rpf"

# rodar o jogo com instrumentação
.\target\diag\sixm-multiversion.exe run "<dump>\game_scarlett_master.exe" --execute --console series-x
```

Variáveis úteis: `SIXM_KEYS=40:Enter` (aperta teclas sem precisar de foco — é como se passa da tela de início), `SIXM_CAPTURE=<dir>` (salva frames em PNG), `SIXM_CAPTURE_WORLD=0,2,8` (marca o primeiro frame com 500 draws — **sem ela a mensagem do mundo não sai**), `SIXM_D3D12_DEBUG=1`, `SIXM_D3D12_DRED=1`, `SIXM_GPU_TIMING=1`.

**Três armadilhas que já custaram horas:**

1. No Windows 10 o executável precisa de `D3D12\D3D12Core.dll` **ao lado dele** (Agility SDK). Sem isso morre em 9 s com `0x887E0003` e nem cria o device. `cargo xtask agility` resolve.
2. Aponte o `translate` para o `shaders.rpf`, **não** para a pasta do dump (223 GB).
3. Medir pelo jogo é lento e depende de GPU; medir offline é 4 minutos e não depende de nada.

---

## 6. Decisões pendentes (precisam do dono do projeto)

| decisão | situação |
|---|---|
| **Renomear para `GambiarraLauncher`** | pedido, **não executado**. Já houve duas renomeações em massa nesta madrugada (`victory*`→`phoenix*`→`sixm*`, ~2.900 ocorrências). Uma terceira mexe em ~2.000 pontos, exige repositório novo no GitHub e refaz a CI que acabou de ser estabilizada. Vale decidir o nome **definitivo** antes de mexer. |
| **Ignorar tudo menos esta pasta** | pedido. Falta definir se o `.gitignore` é o da raiz do workspace (`MEU PROPIO LAUNCHER`) ou o deste repositório. |
| **Trazer do `_SX-Local` o que ajuda** | pedido. Candidatos reais: o laudo de shaders, os logs de corrida preservados, a sonda `d3d12-capabilities.exe`, o loop de execução instrumentado. O resto (Ghidra, assinaturas, análise do REA, `Private/`) é análise do binário e **não ajuda o launcher**. |
| **Separar o launcher do SixM** | pedido, para depois. |

---

## 7. `inspiration/` — o que dá para reaproveitar

Duas referências foram clonadas em `inspiration/`:

### AmericasLauncher (`isaac-btw/AmericasLauncher`)

**É um launcher pronto em C#** (21 `.cs`, 2 `.csproj`, 2 `.xaml`) — exatamente a peça que falta aqui, já que `apps/gta6-pc-launcher/` é só um lugar reservado. O que ele já resolve: UI 720p com cartões de runner, instalação de dependências por `winget`, clone e build de cada runner, saída do console integrada, execução de `preLaunchArgs` (tradução de pipeline) antes do jogo, botão de parar que mata o runner, seletor Series X/S, log em arquivo, auto-update.

Ele trata runners como **plugins configurados em JSON** (`src/AmericasLauncher/Data/runners.json`, 9 runners). Esse desenho serve direto para nós: o nosso binário vira mais um runner.

⚠️ **Não achei arquivo de licença no repositório.** Sem licença, o código é "todos os direitos reservados" por padrão — **copiar não é permitido**. Dá para usar como referência de desenho (o formato `runners.json`, o fluxo de pré-lançamento), não para copiar código. Vale abrir issue pedindo que licenciem.

### ScarlettPort (`ZigZagBoi1/ScarlettPort`)

Outra camada de compatibilidade Xbox Series, em **Rust** (397 arquivos `.rs`), com `runtime/`, `engine/`, `launcher/`, `patches/`. Tem releases, 90% de progresso declarado, e capturas do próprio runtime desenhando. É o projeto mais próximo do nosso — e **tem launcher próprio**, então também serve de referência de integração.

⚠️ O README exibe selo GPL e aponta para `LICENSE.md`, mas **não encontrei esse arquivo na raiz**. Confirmar antes de reaproveitar qualquer linha.

**Recomendação:** tratar os dois como referência de arquitetura por enquanto. O ganho imediato não é copiar código, é copiar **decisões** — principalmente o modelo de runner plugável do AmericasLauncher, que define como o nosso launcher conversa com o runtime.

---

## 8. Aviso importante: duas IAs na mesma árvore

Durante esta madrugada, dois agentes editaram estes mesmos arquivos ao mesmo tempo. O custo foi real e documentado: build quebrado, uma pasta duplicada criada por engano (diagnóstico de "arquivo perdido" que na verdade era uma renomeação do outro agente), nomes de variáveis de ambiente corrompidos nos documentos (`VICTORY_KEYS` virou `Phoenix_KEYS`, que não existe), e duas renomeações em massa sobrepostas.

**Antes de abrir o próximo chat, garanta que só uma sessão está mexendo nesta árvore.** Se precisar de duas, dê a cada uma a sua pasta e nunca a mesma.

---

## 9. Documentos relacionados

| documento | o que tem |
|---|---|
| `docs/hardware-compatibility.md` | matriz de GPUs: suportado, parcial, bloqueado, não testado |
| `docs/shader-translation.md` e `shader-translation-investigation.md` | o fluxo da tradução e a investigação dos 196 geometry shaders, com as hipóteses derrubadas |
| `docs/gpu-portability.md` | detecção de recursos e exceções por fabricante |
| `docs/debugging-gpu-hangs.md` | diagnóstico de travamento gráfico |
| `reports/hardware/rx550-2026-11-27.md` | o relatório completo da máquina de teste |
| `CONTRIBUTING.md` | como contribuir sem um atropelar o outro |
