# Hardware testado

Antes de investigar qualquer coisa, ache a sua placa aqui. Metade dos "bugs" relatados são a GPU não ter o recurso que o título exige.

Para somar a sua máquina: preencha o `STATUS.md` na sua pasta e acrescente uma linha nesta tabela no mesmo PR.

---

## O que decide

| recurso | por que importa |
|---|---|
| **Mesh shaders** | **35% dos shaders do título (9.282 de 26.503) são mesh.** Sem isso a interface e a arte 2D aparecem e o mundo nunca renderiza. |
| **Shader Model 6.6** | exigido pelo runtime; `WaveSize` é recurso de 6.6 |
| **Onda mínima de 32 lanes** | numa GPU que só tem ondas de 64 (AMD pré-RDNA), os shaders wave32 do console são recusados pelo driver, a menos que o tradutor não declare a largura — ver §Correções |
| **VRAM** | o título é de Series X; 2 GB não chega perto |
| **DXR** | o título carrega pacotes de BVH (`core_bvh4.rpf`, `story000_bvh4.rpf`) |

---

## Resultados

| GPU | Arq. | VRAM | Mesh | SM | Onda mín/máx | Até onde chegou | Quem |
|---|---|---|---|---|---|---|---|
| Radeon RX 550 | GCN 4 (Polaris) | 2 GB | **NÃO** | 6.5 | 64 / 64 | arte 2D renderiza (2048+ frames); mundo nunca aparece | [amd/miaumenu](../contributors/amd/miaumenu/STATUS.md), 27/11/2026 |
| WARP (software) | — | — | NÃO | 6.2 | — | não serve nem para teste de shader | equipe, 27/11/2026 |
| RX 7900 XTX | RDNA 3 | 24 GB | sim | 6.6 | 32 / 64 | relatado pelo upstream, não verificado por nós | upstream |

### O que esperar por arquitetura

| hardware | onda 32 | mesh | SM 6.6 | expectativa |
|---|---|---|---|---|
| AMD GCN (RX 400/500, Vega) | não — precisa da correção abaixo | não | não | não passa do carregamento |
| AMD RDNA 1 (RX 5000) | sim | não | parcial | mesh shaders barram |
| **AMD RDNA 2+ (RX 6000+)** | sim | sim | sim | **alvo** |
| **NVIDIA Turing+ (RTX 20xx+)** | sim | sim | sim | alvo alternativo |
| Intel Arc | sim | sim | sim | não testado |

---

## Como medir a sua

```powershell
.\main\target\diag\gambiarra-cli.exe translate --list "<seu dump>\shaders.rpf"
```

Dá o inventário do que o tradutor consegue converter — 4 minutos, sem abrir o jogo, sem depender da GPU.

Para as capacidades da placa, qualquer ferramenta que leia `D3D12_FEATURE_D3D12_OPTIONS7` (`MeshShaderTier`), `D3D12_FEATURE_SHADER_MODEL` e `D3D12_FEATURE_D3D12_OPTIONS1` (`WaveLaneCountMin`/`Max`) serve. O runtime também registra no log:

```
INFO gambiarra_gpu::host: host wave lanes min=64 max=64
```

---

## Correções que dependem de hardware

### Onda de 32 lanes num host que só tem 64 (AMD GCN)

Numa GPU GCN o driver recusa `WaveSize(32)` com `E_INVALIDARG (0x80070057)`, porque a onda mínima do device é 64 — e `WaveSize` ainda é recurso de SM 6.6, que GCN não tem.

A correção declara a largura só quando o host consegue honrá-la: um programa wave32 cujo grupo cabe numa onda roda nos primeiros 32 lanes de uma onda de 64 (os lanes de fora não entram em ballot, redução nem leitura de lane — que é o que a onda de 32 do console faz). Grupo maior que 32 mantém a declaração e o host recusa, porque aí duas ondas do console dividiriam uma onda do host e o código cross-lane alcançaria a outra.

Medido numa RX 550:

| | antes | depois |
|---|---|---|
| avisos `wave lane count of 32` | 509 | **3** |
| pipelines recusados com `E_INVALIDARG` | 298 | **2** |

Em RDNA e NVIDIA a correção não muda nada — a condição (`onda mínima > 32`) é falsa.
