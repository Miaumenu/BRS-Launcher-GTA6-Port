# SixMultiVersion + GTA VI (Scarlett) — o que já se sabe

**Última atualização:** 27/11/2026
**Escopo:** rodar o `game_scarlett_master.exe` (build Series 1.0.2.1) no Windows através do SixMultiVersion, o que funciona, o que não funciona, e o que é limite de hardware.

> **Nota de nomes.** Em 27/11/2026 o projeto foi renomeado de `victory*` / `VICTORY_*`
> para `sixm*` / `SIXM_*`. Este documento usa os nomes novos. As **linhas de log citadas
> aqui são anteriores à renomeação** e foram geradas por binários que ainda imprimiam
> `victory_gpu::...`; elas aparecem com os nomes atuais para não confundir quem lê o código
> de hoje, mas um log antigo no disco vai mostrar a grafia antiga.


Este documento é para ser **continuado por outras pessoas**. Cada achado traz a evidência bruta, o comando que a produz e o arquivo onde ela está, para que ninguém precise refazer a medição para confiar nela. Onde algo é hipótese e não fato medido, está escrito.

---

## 1. Resumo executivo

| | estado |
|---|---|
| O jogo carrega, mapeia e liga os imports | **funciona** (`0 missing`) |
| O jogo renderiza e apresenta frames | **funciona** (2048+ frames, arte 2D pixel-correta) |
| Tradução dos shaders do jogo (26.084 programas) | **99,2% emitem DXIL** |
| Chegar ao mundo / gameplay | **não** — barrado por hardware |
| Mesh shaders (35% dos shaders do jogo) | **impossível nesta máquina** |

A conclusão central: **o tradutor do SixMultiVersion não é o gargalo.** Ele converte quase todo o jogo. O que impede o gameplay na máquina de referência é a GPU não ter mesh shaders nem Shader Model 6.6.

---

## 2. Máquina de referência

Sonda: `_SX-Local/Tools/SixM-diagnostics/d3d12-capabilities.exe`

```
Adapter 0: Radeon RX550/550 Series | dedicated VRAM: 2027 MiB | software: no
  MeshShaderTier: NOT_SUPPORTED (0x00000000)
  HighestShaderModel: 6.5
Adapter 2: Microsoft Basic Render Driver (WARP) | HighestShaderModel: 6.2
```

- GPU: **Radeon RX 550 (Polaris / GCN 4)**, 2 GB. `WaveLaneCountMin = WaveLaneCountMax = 64` (só ondas de 64 lanes).
- CPU: AMD Ryzen 7 3700X. SO: Windows 10.0.19045.
- O SixMultiVersion exige, pelo próprio README: Windows x64, CPU com AVX2, GPU D3D12 com **Shader Model 6.6 e mesh shaders**.

**A máquina de referência não atende os requisitos.** Isso precisa ficar claro em qualquer leitura dos resultados abaixo: muita coisa que parece falha do software é a placa.

O Xbox Series X, por comparação, é RDNA 2: ondas de 32 **e** de 64, mesh shaders, SM 6.6, 10+ GB de memória gráfica.

---

## 3. Como reproduzir

### 3.1 Compilar

```powershell
cargo build --profile diag -p SixMultiVersion --features diagnostics   # target\diag\SixMultiVersion.exe
```

A toolchain Rust portátil fica em `_SX-Local/Tools/rust-toolchain` (`CARGO_HOME`/`RUSTUP_HOME` apontando para `cargo/` e `rustup/`). O projeto fixa Rust 1.98.1 + MSVC.

**Pegadinha:** no Windows 10 o runner carrega o D3D12 Agility SDK e precisa de `D3D12\D3D12Core.dll` **ao lado do exe**. Sem isso ele morre em 9 s com `0x887E0003` ("configuração de versão do SDK D3D12 do host exe é inválida") e nem cria o device. `cargo xtask agility` baixa e põe no lugar.

### 3.2 Rodar o jogo com instrumentação

```powershell
python _SX-Local\Tools\SixM-loop\SIXM_loop.py --label teste --dred --timing
```

`SIXM_loop.py` é uma ferramenta local fora deste repositório. Ela lança o jogo, segue o stderr ao vivo, classifica cada linha, para sozinha no primeiro frame do mundo / erro fatal / timeout, e escreve um relatório de ~40 linhas em vez de 1 MB de log. Ela usa:

| variável | para quê |
|---|---|
| `SIXM_KEYS=40:Enter,55:Enter` | aperta teclas nos segundos dados, **sem precisar de foco** (injeta num pad virtual). É assim que se passa da tela de início. |
| `SIXM_CAPTURE=<dir>` | salva os frames apresentados em PNG (meio tamanho) |
| `SIXM_CAPTURE_WORLD=0,2,8` | marca o **primeiro frame que roda 500 draws** — é o sinal de "o mundo começou a renderizar". Sem essa variável a mensagem não sai. |
| `SIXM_D3D12_DEBUG=1` | camada de debug do D3D12 |
| `SIXM_D3D12_DRED=1` | na falha de GPU, a alocação atingida e os comandos em voo |
| `SIXM_GPU_TIMING=1` | na remoção do device, nomeia a lista e todos os comandos dela |
| `SIXM_SKIP_PIPELINES=<hash>` | pula um pipeline específico, para isolar um culpado |

### 3.3 Medir os shaders sem abrir o jogo (muito mais rápido)

```powershell
.\target\diag\SixMultiVersion.exe shader-info "<dump>\shaders.rpf"
.\target\diag\SixMultiVersion.exe translate --list "<dump>\shaders.rpf"
.\target\diag\SixMultiVersion.exe translate --list --validate "<dump>\shaders.rpf"
.\target\diag\SixMultiVersion.exe translate --cache --pipelines "<dump>"   # aquece o cache que o `run` lê
```

Uma passada completa sobre os 26.503 shaders leva **~255 s** e dá o inventário exato do que o tradutor não aguenta. É o loop de iteração que vale a pena: rodar o jogo custa 4 minutos e depende de GPU; isto custa 4 minutos e não depende de nada.

**Aponte para o `shaders.rpf`, não para a pasta do dump.** Varrer os 223 GB do dump inteiro leva muito mais tempo e não acha mais nada.

---

## 4. Censo do conteúdo de shaders

`shader-info` sobre `shaders.rpf` (411 MB):

```
containers found       26503 (in 1 files)
  with DXIL part       26503
  with LLVM bitcode    0
  PC-standard layout   0
  with native code     26503
    Es                 195      Gs    195
    Vs                2236      Ps  12101
    Ms                9282      Cs   2689
  decoded programs     26698 (44386798 instructions)
```

Dois fatos que decidem estratégia:

1. **`with LLVM bitcode 0` e `PC-standard layout 0`.** O build do Xbox não carrega nenhuma forma portátil de shader — só código de máquina GFX10. Não existe "recompilar"; ou se traduz a ISA, ou se reescreve o shader do zero.
2. **9.282 dos 26.503 são mesh shaders (35%).** A geometria do GTA VI no Series X passa por mesh shaders.

`26698 - 26503 = 195` programas a mais que containers: são os 195 containers que trazem **duas** etapas (Es + Gs).

---

## 5. Achado 1 — wave32 em GPU wave64 (**corrigido**)

### Sintoma

Em cada corrida do jogo, antes da correção:

| mensagem | ocorrências |
|---|---|
| `CreateComputeShader: Shader wants wave lane count of 32 but minimum wave lane count for device is 64` | **509** |
| `host pipeline refused error=Parâmetro incorreto. (0x80070057)` (E_INVALIDARG), 298 compute | **305** |

O número era estável entre corridas: um conjunto fixo de shaders, não ruído.

### Causa

`0x80070057` é `E_INVALIDARG`. O DXIL traduzido declarava `WaveSize(32)`, e o driver de uma GPU GCN recusa, porque a onda mínima do device é 64. Além disso `WaveSize` é recurso de SM 6.6 e a placa para em 6.5.

No código (caminhos do upstream `cool_fox-main`):

- `crates/SixM-gpu/src/host.rs` lia `D3D12_OPTIONS1` e usava **só** `WaveLaneCountMax`. O mínimo entrava apenas na decisão `set_host_rdna(amd && min==32 && max==64)`, nunca na escolha da largura de onda.
- `crates/SixM-shader/src/emit/lower.rs` só tratava o caso oposto — `if wave == 64 && host_max_wave() == 32` rebaixa wave64 do console para wave32 num host NVIDIA. **Não existia o caminho inverso.**
- `crates/SixM-shader/src/emit/shader.rs` emitia o metadado `WaveSize` (tag 11) e o campo do `PSV0` sempre que `wave_size` estava preenchido, sem olhar as capacidades do host.

### Correção

Um host cuja onda mais estreita é de 64 lanes recusa `WaveSize(32)`. Um programa wave32 do console **cujo grupo é uma onda só** roda como está nos primeiros 32 lanes de uma onda de 64: os lanes que estão fora não entram em nenhum ballot, redução ou leitura de lane da onda — que é exatamente o que a onda de 32 do console faz. Só a *declaração* sai; a tradução continua usando os 32 lanes do console.

Um grupo com mais de 32 threads colocaria **duas** ondas do console dentro de uma onda do host, onde o código cross-lane alcançaria os lanes da outra. Esse caso **mantém** a declaração e o host recusa o pipeline — melhor recusar do que rodar errado em silêncio.

Peças:

1. `lower.rs`: `HOST_MIN_WAVE` + `set_host_min_wave()` + `host_min_wave()`, e `host_min_wave()` entra em `host_settings()`, que é **chave do cache de shaders** (senão um cache de outra máquina seria servido).
2. `shader.rs`: campo `declare_wave: bool` (padrão `true`) no `Shader`, consultado nos **dois** lugares que publicam a largura de onda — os metadados de entrada do DXIL e o `PSV0`. Esquecer o `PSV0` não adianta: o driver valida os dois.
3. `lower.rs`, na decisão: `if wave == 32 && host_min_wave() > 32 && group <= 32 { s.declare_wave = false; }`
4. `host.rs` (SixM-gpu): `set_host_min_wave(o1.WaveLaneCountMin)`.
5. Teste em `emit/wave64_tests.rs`: `a_wave32_group_of_one_wave_drops_its_declaration_on_64_lane_waves` — grupo de 32 com host de onda 32 declara `Some(32)`; com host de onda mínima 64 não declara; grupo de 64 com onda mínima 64 volta a declarar.

### Resultado medido

| | antes | depois |
|---|---|---|
| `wave lane count of 32` | 509 | **3** |
| pipelines recusados com `E_INVALIDARG` | 298 | **2** |
| `host wave lanes` detectado | — | `min=64 max=64` |

Suíte do `SixM-shader`: **246 passaram** com a correção contra **245** sem ela (o +1 é o teste novo), e **as mesmas 36 falhas** nos dois casos. As 36 são todas `no SM 6.6 device available` — ambientais, não regressão. Verificado revertendo a correção e rodando a suíte de novo.

Vale para **qualquer GPU GCN** (Polaris, Vega, e as placas AMD anteriores ao RDNA).

---

## 6. Achado 2 — o teto de hardware (**não corrigível por software**)

| limite | consequência |
|---|---|
| `MeshShaderTier: NOT_SUPPORTED` | **9.282 mesh shaders (35% do jogo) não têm onde rodar** |
| `HighestShaderModel: 6.5` | `WaveSize` e outros recursos de 6.6 indisponíveis |
| Sem DXR | o jogo carrega `core_bvh4.rpf`, `story000_bvh4.rpf`, `shared_minigame000_bvh4.rpf` e usa `XboxBvhBuilder.dll` |
| 2 GB de VRAM | um título de Series X espera 10+ GB |

O tradutor **converte os 8.765 programas de mesh perfeitamente** (ver §7) — e o host não executa nenhum. É por isso que a arte 2D sai pixel-correta e o mundo nunca aparece.

Nenhuma conversão de asset cria unidade de mesh shader ou de raytracing. Nem um port nativo perfeito do GTA VI rodaria numa RX 550 de 2 GB.

---

## 7. Achado 3 — tradução offline do pacote inteiro

`translate --list` sobre `shaders.rpf`, no upstream `cool_fox-main` com a correção wave32 aplicada:

```
containers   26503 (614 duplicates skipped)
programs     26084 translated of 26084
structured   26084 of 26084
resources    25889 of 26084 tracked
DXIL         14247 emitted of 14247 vertex and pixel programs
              2677 emitted of  2682 compute programs
              8765 emitted of  8765 mesh programs
```

**99,2% do jogo converte, e os mesh shaders convertem 100%.**

As 202 falhas restantes:

| motivo | qtd | estágio |
|---|---|---|
| `resources: an address in an SGPR that is no root parameter` | **196** | Gs |
| `emit: a descriptor read only as data` | 6 | Cs |

### Cuidado: a árvore importa

A árvore `_SX-Local/Tools/SixM-DXGI67-Test` (um worktree mais antigo, com correções locais de DXGI/Agility) dá **227** falhas no mesmo pacote. O upstream já resolveu 25 delas:

| falha | DXGI67 | upstream |
|---|---|---|
| `Gs: an address in an SGPR that is no root parameter` | 196 | 196 |
| `Cs: a lane-mask join of an untracked mask` | 13 | **0** |
| `Cs: v_cvt_f32_f64` | 5 | **0** |
| `Cs: a lane mask read as data` | 4 | **0** |
| `Ps: barycentrics read directly` | 1 | **0** |
| `Cs: v_permlanex16_b32` | 1 | **0** |
| `Cs: a descriptor read only as data` | 1 | 6 |
| **total** | **227** | **202** |

Antes de caçar um shader que não traduz, confira em qual árvore a medição foi feita.

---

## 8. Achado 4 — os 196 geometry shaders (**em aberto**)

196 falhas, **uma só causa**. Todas são `GS_VoxelizeGeneric` (196 permutações do mesmo shader), todas no **mesmo offset 0x44**, e são **todos os geometry shaders do jogo**.

### Cadeia de evidência

Um diagnóstico temporário em `resource.rs::why_no_base` imprimindo o registrador e o layout deu, nas 195 ocorrências, sempre a mesma linha:

```
WHY-SGPR stage=Gs reg=S(8)
```

Outro diagnóstico, imprimindo os `user_data` declarados por etapa de cada container:

```
151  stage=Gs  user_data=[0, 8, 9, 10, 11, 13, 14]
151  stage=Es  user_data=[0, 8, 9, 10, 11, 13, 14]
7687 stage=Ps  user_data=[0, 8, 9, 10, 11, 13, 15, 16]
```

A conversão de user-data para registrador está em `native.rs::HwStage::user_sgpr`:

```rust
HwStage::Ls | HwStage::Hs | HwStage::Es | HwStage::Gs | HwStage::Ms => index + 8,
HwStage::Vs | HwStage::Ps | HwStage::Cs => index,
```

| estágio | `user_data` da tabela | mapeia para | o código lê |
|---|---|---|---|
| Ps | 8 | **s8** | s8 ✔ |
| Gs | 8 | **s16** | **s8** ✘ |

Layout idêntico, mapeamento diferente. `param_of(S(8))` não casa com nada e o endereço é recusado.

(O `0` que aparece em todos os conjuntos é ruído: são entradas `ImmediateSampler` com `sgprs: 0`, largura zero, com as quais `param_of` nunca casa.)

### Teoria descartada

Supus que fosse um par ES-GS fundido com os user SGPRs divididos (ES com os dwords 0..7, GS com 8..14). **Os dados derrubaram:** Es e Gs declaram layouts idênticos, par a par. Não há divisão. Fica registrado para ninguém repetir o caminho.

### Experimento feito — e revertido

Tirar o `+8` só do `Gs` (`HwStage::Gs => index`) e retraduzir:

- as 196 **saíram** de `an address in an SGPR that is no root parameter`
- e **entraram** em `resources: an address with an untracked high half`

Parecia progresso. **Não era, e o experimento foi revertido.** Duas medições seguintes derrubaram a hipótese:

**1. A metade alta não vem de registrador nenhum.** Diagnóstico `SIXM_WHY_HIGH=1`:

```
190  WHY-HIGH stage=Gs base_reg=S(8) root=Table user_data=8 hi=Value(Value(38)) hi_input=None
  5  WHY-HIGH stage=Gs base_reg=S(8) root=Table user_data=8 hi=Value(Value(36)) hi_input=None
```

`hi_input=None` quer dizer que a metade alta é **valor calculado por instrução**, não um user SGPR. Cai por terra também a ideia de que o ponteiro seria de 64 bits repartido em s8:s9 (duas tabelas declaradas em `user_data` 8 e 9).

**2. O `+8` está certo para os outros estágios fundidos.** Diagnóstico `SIXM_WHY_BASE=1`, contando em que registrador cada base resolvida caiu:

| estágio | user_data → registrador | resoluções |
|---|---|---|
| Ms | ud8 → **s16**, ud9 → s17, ud11 → s19, ud16 → s24 | 753.825 |
| Es | ud9 → **s17**, ud11 → s19 | 1.199 |
| Ps | ud8 → **s8** | 905.844 |
| Cs | ud4 → **s4**, ud5 → s5, ud10 → s10 | 1.458.060 |

`Ms` e `Es` resolvem centenas de milhares de endereços **com o `+8`** — para eles o deslocamento está certo. E `Es` é a outra metade do mesmo programa fundido do `Gs`: não há razão de hardware para as duas metades usarem bases diferentes de user SGPR.

Conclusão: tirar o `+8` do `Gs` fazia `ud8` casar com s8 **por coincidência**, provavelmente ligando o endereço ao parâmetro errado — e o erro seguinte, na metade alta, é consistente com um casamento errado. O experimento foi desfeito e a árvore está como o upstream.

### O que sobra de firme

- s8, para um `Gs`, corresponde ao **user-data dword 0**, que o layout declara com **largura zero** (só entradas `ImmediateSampler`, `sgprs: 0`). O shader lê um registrador ao qual o próprio layout não dá conteúdo.
- A metade alta do endereço é calculada por instrução, não lida de registrador.

### Onde continuar

O caminho não é mais mexer em `user_sgpr`. O próximo passo é **desmontar o `GS_VoxelizeGeneric` no offset 0x44** e ler a sequência de instruções que monta o endereço — o ferramental atual não expõe disassembly, então isso precisa ser construído (ou usar `SIXM_SHADER_SAVE` e um disassembler GFX10 externo). Sem ver as instruções, qualquer correção aqui é chute.

**Pergunta respondida:** `Ms` funciona porque o `+8` é genuinamente correto para ele — 341.082 resoluções em s16 para `user_data 8`. A anomalia é só do `Gs`.

---

## 9. Achado 5 — validação DXIL (**não conclusivo**)

`translate --list --validate` (validador `dxil.dll` do Windows SDK 10.0.26100.0):

| | qtd |
|---|---|
| `validated` | 8.021 |
| `validator: error: Container part 'Pipeline State Validation' does not match expected for module.` | 17.550 |
| `signature element ... has interpolation mode that differs from another element packed into the same row` | ~40 |

Todos os **2.677 programas de compute validaram**. Os erros de `PSV0` atingem os estágios gráficos.

**Isto ainda não foi comparado com uma linha de base sem as nossas mudanças.** Não se sabe se é pré-existente ou introduzido. Quem for mexer nisso: rode `--validate` numa árvore limpa primeiro.

---

## 10. Comportamento em execução (na máquina de referência)

Corrida de 250 s com a correção wave32, DRED e GPU timing ligados:

- **sem remoção de device**, 2048+ frames apresentados
- `frame-1024.png`: arte de carregamento do GTA VI, **cor cheia e correta**
- `frame-2048.png`: preto — 205 black frames no log
- 223 `slow host submission`
- **nunca** emitiu `the world's first frame` — nenhum frame chegou a 500 draws, logo não houve gameplay
- 1 pipeline `[mesh, pixel]` descartado por falta de mesh shaders

Numa corrida anterior **sem** DRED/timing, o device foi removido aos ~55 s (`0x887A0005` → `0x887A0020`, erro interno do driver). Com DRED e timing (que roda cada lista separadamente) isso não se repetiu. A remoção do device **não** foi investigada a fundo e pode voltar.

Cuidado ao ler logs: um filtro que casa a palavra `DRED` pega o nome da pasta de captura e produz falso positivo de "device removed". Já aconteceu aqui.

---

## 11. Fila de trabalho

Por ordem de retorno:

1. **Testar numa GPU que atende o requisito** — RDNA 2+ ou Turing+, SM 6.6, mesh shaders, ≥8 GB. É o experimento decisivo: com 99,2% dos shaders convertendo, é alta a chance de chegar ao gameplay. Enquanto isso não acontece, toda medição aqui é o teto da RX 550, não o do software.
2. **Fechar os 196 geometry shaders** (§8). Não depende de GPU nova. 97% do que falta na tradução.
3. **Os 6 compute de `a descriptor read only as data`.**
4. **Baseline de `--validate`** (§9) para saber se os erros de `PSV0` são nossos.
5. **A remoção de device aos 55 s** (§10), com DRED, se voltar a aparecer.

---

## 12. Guia por hardware

O que esperar em outra máquina:

| hardware | wave32 | mesh | SM 6.6 | expectativa |
|---|---|---|---|---|
| **AMD GCN** (Polaris, Vega, RX 400/500/Vega) | onda mínima 64 — **precisa da correção §5** | não | não | não passa da tela de carregamento |
| **AMD RDNA 1** (RX 5000) | ondas 32 e 64 | não | parcial | mesh shaders barram |
| **AMD RDNA 2+** (RX 6000+) | ondas 32 e 64 | sim | sim | **alvo** — é o que o SixMultiVersion pede |
| **NVIDIA Turing+** (RTX 20xx+) | onda 32 — exercita o caminho wave64→wave32, que já existia | sim | sim | alvo alternativo |
| **Intel Arc** | onda variável | sim | sim | não testado |
| **WARP** (software) | — | não | **SM 6.2** | não serve nem para teste de shader |

A correção de §5 só importa onde a onda mínima é 64, ou seja em GCN. Em RDNA e NVIDIA ela não muda nada (a condição `host_min_wave() > 32` fica falsa).

---

## 13. Arquivos

| o quê | onde |
|---|---|
| Loop de execução instrumentado (ferramenta local, fora do repositório) | `_SX-Local/Tools/SixM-loop/SIXM_loop.py` |
| Relatórios e frames por corrida | `_SX-Local/Tools/SixM-loop/runs/<data>-<label>/` |
| Tradução offline do pacote | `_SX-Local/Tools/SixM-loop/runs/offline/` |
| Sonda de capacidades D3D12 | `_SX-Local/Tools/SixM-diagnostics/d3d12-capabilities.exe` |
| Logs brutos das corridas de 26/11 | `_SX-Local/6M-Development/Evidence/SixM-run-logs-2026-11-26/` |
| Fonte upstream do SixMultiVersion | `cool_fox-main/` (sem git; é o tarball mais novo) |
| Árvore com correções locais DXGI/Agility | `_SX-Local/Tools/SixM-DXGI67-Test/` (worktree de `SixM-Agility-Test`) |
| Tentativa anterior de execução | `_SX-Local/6M-Development/Evidence/SixM-Run-Attempt-2026-11-26.md` |

### Diagnósticos temporários no código

Duas impressões foram adicionadas ao `cool_fox-main` para este trabalho e **devem sair** antes de qualquer envio upstream:

- `resource.rs::why_no_base` — `SIXM_WHY_SGPR=1` imprime o registrador de entrada e o layout
- `native.rs` (parse do container) — `SIXM_WHY_LAYOUT=1` imprime os `user_data` por etapa

### Nota de contexto

O SixMultiVersion **não é um emulador** e não é o launcher do 6M. O Xbox Series X é x86-64 AMD, a mesma arquitetura do host: o runner carrega o PE de verdade e liga os imports (`91 hle, 424 host, 72 title, 0 missing`). É uma camada de compatibilidade no estilo Wine/Proton. Não há CPU sendo emulada e, portanto, não há código de CPU para "converter".
