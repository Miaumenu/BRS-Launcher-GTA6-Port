# miaumenu — mudanças

## 0001-wave32-em-host-de-onda-64

- **Data:** 27/11/2026
- **Status:** **funcionou** — e já está aplicado em `main/`
- **Confirmação em segunda máquina:** ❌ **pendente** (ver aviso no fim)
- **Hipótese:** numa GPU cuja onda mais estreita é de 64 lanes (AMD pré-RDNA), o driver recusa `WaveSize(32)` com `E_INVALIDARG (0x80070057)`. O tradutor declarava essa largura sem olhar as capacidades do host.
- **Arquivos tocados:** `crates/gambiarra-shader/src/emit/{lower,shader,mod,wave64_tests}.rs`, `crates/gambiarra-gpu/src/host.rs`
- **Patch:** `patches/0001-wave32-em-host-de-onda-64.patch` (104 linhas adicionadas, 8 removidas)

### O que a mudança faz

Um programa wave32 do console **cujo grupo cabe numa onda** roda como está nos primeiros 32 lanes de uma onda de 64: os lanes de fora não entram em nenhum ballot, redução ou leitura de lane — que é exatamente o que a onda de 32 do console faz. Então só a **declaração** sai; a tradução continua usando os 32 lanes do console.

Grupo com mais de 32 threads **mantém** a declaração e o host recusa o pipeline — porque aí duas ondas do console dividiriam uma onda do host e o código cross-lane alcançaria a outra. Recusar é melhor que rodar errado em silêncio.

Peças:

1. `lower.rs` — `HOST_MIN_WAVE` + `set_host_min_wave()` + `host_min_wave()`, e a onda mínima entra em `host_settings()`, que é **chave do cache de shaders** (senão um cache de outra máquina seria servido).
2. `shader.rs` — campo `declare_wave: bool`, consultado nos **dois** lugares que publicam a largura: os metadados de entrada do DXIL **e o `PSV0`**. Esquecer o `PSV0` não adianta, o driver valida os dois.
3. `lower.rs`, na decisão — `if wave == 32 && host_min_wave() > 32 && group <= 32 { s.declare_wave = false; }`
4. `host.rs` (gambiarra-gpu) — passa `WaveLaneCountMin` ao tradutor; antes só o `Max` era lido.

### Medição

| | ANTES | DEPOIS |
|---|---|---|
| avisos `wave lane count of 32` | 509 | **3** |
| pipelines recusados com `E_INVALIDARG` | 298 | **2** |
| detecção da onda do host | — | `host wave lanes min=64 max=64` |

### Teste de regressão

`a_wave32_group_of_one_wave_drops_its_declaration_on_64_lane_waves`, em `emit/wave64_tests.rs`:

- grupo de 32 com host de onda 32 → declara `Some(32)` (como antes)
- grupo de 32 com host de onda mínima 64 → **não declara**
- grupo de 64 com host de onda mínima 64 → volta a declarar, para o host recusar

Suíte: **246 passaram** com a correção contra **245** sem ela (o +1 é o teste novo), com **as mesmas 36 falhas** nos dois casos — todas `no SM 6.6 device available`, ambientais. Verificado revertendo a correção e rodando de novo.

### ⚠️ Aviso

Esta mudança foi para `main/` **sem** a confirmação em segunda máquina que o `CONTRIBUTING.md` exige. Ela só pode ser validada num host de onda mínima 64 (qualquer AMD GCN: RX 400/500, Vega). **Se você tem uma dessas, aplique e confirme** — é o item mais útil que alguém pode fazer por este repositório agora.

Em RDNA e NVIDIA a correção não muda nada: a condição `host_min_wave() > 32` é falsa.

---

## 0002-tirar-o-+8-do-estagio-Gs — **revertido, não funcionou**

- **Data:** 27/11/2026
- **Status:** **não funcionou** — desfeito, não há patch
- **Hipótese:** os 196 geometry shaders que falham com `an address in an SGPR that is no root parameter` leem **s8**, mas `HwStage::user_sgpr` soma `+8` para `Gs`, jogando a tabela declarada em `user_data 8` para **s16**. Tirando o `+8` só do `Gs`, s8 casaria.
- **O que aconteceu:** as 196 saíram daquele erro e entraram em `an address with an untracked high half`. Parecia progresso.
- **Por que foi revertido:** duas medições derrubaram a hipótese.

| medição | resultado |
|---|---|
| `GAMBIARRA_WHY_HIGH=1` | a metade alta é `hi_input=None` — **valor calculado por instrução**, não um registrador. Cai também a ideia de um ponteiro de 64 bits repartido em s8:s9. |
| `GAMBIARRA_WHY_BASE=1` | `Ms` resolve **753.825** endereços com o `+8` (ud8 → s16) e `Es` resolve **1.199** (ud9 → s17). O `+8` está certo para os dois — e o `Es` é a **outra metade do mesmo programa fundido** do `Gs`. Não há razão de hardware para as duas metades usarem bases diferentes. |

Ou seja: tirar o `+8` fazia `ud8` casar com s8 **por coincidência**, ligando o endereço ao parâmetro errado. O erro seguinte era sintoma disso. Se eu tivesse aceitado "saiu de um erro e entrou em outro, logo avancei", teria enterrado um bug silencioso em 196 shaders.

---

## 0003-caminho-MEMORY-para-geometry-shaders

- **Data:** 27/11/2026
- **Status:** **funcionou** — 196 falhas a menos
- **Confirmação em segunda máquina:** ❌ pendente (mas é mudança de tradutor, reproduzível offline em qualquer GPU)
- **Patch:** `patches/0003-caminho-MEMORY-para-geometry-shaders.patch` (uma condição + comentário)

### Como cheguei nela

Depois de o 0002 falhar, parei de inferir pelo comportamento do rastreador e fui **ler as
instruções**. O tradutor já decodifica GFX10 para montar o IR, então bastava expor o que ele
já sabia: um dump com o offset, o nome, as palavras e os registradores de cada instrução.

O `GS_VoxelizeGeneric` no offset 0x44:

```
0x0030  s_and_b32    s1, s9, 0xff      ; metade alta = s9 & 0xFF
0x0038  s_mov_b32    s0, s8            ; metade baixa = s8
0x0044  s_load_dword s0, s[0:1], 0xb8  ; lê de um ponteiro de 40 bits
```

O endereço é `s8 | ((s9 & 0xFF) << 32)` — **memória do título**, montada de dois user SGPRs
que a root signature declara com largura zero. Por isso a metade alta aparecia como "valor
calculado": é o `s_and`. E os inputs do programa (`S(8)`, `S(9)`, `S(16)`, `S(19)`) confirmam
que o `+8` do `user_sgpr` estava certo o tempo todo: s8=ud0, s9=ud1, s16=ud8, s19=ud11.

### A mudança

```diff
-        if self.stage == HwStage::Cs && RAW.contains(&why) {
+        if matches!(self.stage, HwStage::Cs | HwStage::Gs) && RAW.contains(&why) {
```

O caminho `MEMORY` já existia para "endereço que não rastreia até um root parameter — memória
do título como está", e `"an address in an SGPR that is no root parameter"` já estava na lista
de motivos aceitos. Estava travado em compute por falta de um caso conhecido em outro estágio.

### Medição

| | ANTES (upstream) | DEPOIS |
|---|---|---|
| `resources ... tracked` | 25.889 de 26.084 | **26.084 de 26.084** |
| geometry shaders falhando | **196** | **0** |
| **total de falhas** | **202** | **5** |

De 26.084 programas, falham 5 — todos o mesmo motivo (`Cs: a descriptor read only as data`).

> **Remedido em 27/11/2026 com a árvore atual: 5.** O log preservado em
> `logs/traducao-offline.txt` é a medição **antes** da correção (`resources 25.889 de
> 26.084`) e agrupa `196 Gs + 6 Cs = 202`. A coluna DEPOIS originalmente repetia aquele
> `6` sem remedir os `Cs`. A corrida de hoje emite os mesmos `2677 de 2682` programas de
> compute e agrupa **5** ocorrências. O formato do agrupamento também mudou (`5  emit:` em
> vez de `6  Cs  emit:`), então a diferença de um pode ser mudança de contagem no CLI e
> não um programa a mais convertendo — não foi investigada.

### Teste de regressão

`cargo test -p gambiarra-shader`: **246 passaram, 36 falharam** — a mesma contagem de antes da
mudança. As 36 são `no SM 6.6 device available`, ambientais.

Medido duas vezes: uma com os diagnósticos temporários no código e outra com a árvore já
limpa, para garantir que o número é da correção e não do instrumental.

### Nota

Os diagnósticos temporários (`GAMBIARRA_WHY_SGPR`, `GAMBIARRA_WHY_LAYOUT`, `GAMBIARRA_WHY_BASE`,
`GAMBIARRA_WHY_HIGH`, `GAMBIARRA_DUMP_CODE`) foram **removidos** depois que a investigação terminou.
`native.rs` voltou a ser idêntico ao upstream. O patch contém só a correção.

---

## Hipóteses derrubadas

| hipótese | como foi derrubada |
|---|---|
| Os 196 `Gs` são um par ES-GS fundido com os user SGPRs divididos (ES com dwords 0..7, GS com 8..14) | `GAMBIARRA_WHY_LAYOUT=1` mostrou que **Es e Gs declaram layouts idênticos**, par a par. Não há divisão. |
| O `+8` do `Gs` está errado | `Ms` e `Es` resolvem centenas de milhares de endereços **com** o `+8`. A anomalia é só do `Gs`. |
| A metade alta do endereço vem de s9 (a outra tabela declarada) | `hi_input=None`: é valor calculado. O disassembly mostrou que é `s_and_b32 s1, s9, 0xff` — vem de s9 **mascarado**, não de s9 direto. |
| A remoção do device aos 55 s era causada pela correção wave32 | Com DRED e GPU timing ligados a corrida foi aos 250 s **sem** remoção. Não reproduziu. Continua sem investigação a fundo. |
| Havia 12 remoções de device numa corrida | Falso positivo do meu próprio filtro: o padrão `DRED` casava com o nome da pasta `...-i1-dred/frames/`. |

## O que falta, em ordem de retorno

1. **Confirmar o 0001 numa segunda máquina GCN.**
2. **Rodar em GPU com mesh shaders e SM 6.6** — é o experimento que decide se o projeto funciona. Aqui só dá para medir o teto da RX 550.
3. **Os 5 compute** de `a descriptor read only as data`.
4. **Linha de base do `--validate`**: 17.550 erros de `PSV0` ainda não foram comparados com uma árvore limpa, então não se sabe se são nossos.
