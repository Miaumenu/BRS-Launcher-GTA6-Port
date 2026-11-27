# miaumenu — máquina e estado

| campo | valor |
|---|---|
| GPU / VRAM | Radeon RX 550 / 550 Series — **2 GB** (2027 MiB) |
| Arquitetura | **GCN 4 (Polaris)** |
| `MeshShaderTier` | **NOT_SUPPORTED** |
| `HighestShaderModel` | **6.5** |
| `WaveLaneCountMin` / `Max` | **64 / 64** (sem onda de 32) |
| CPU / RAM | AMD Ryzen 7 3700X (AVX2 ok) / 24 GB |
| Windows / driver | 10.0.19045 / AMD 31.0.12029.10015 |
| Adaptador de software | WARP — SM 6.2, sem mesh |
| Data do teste | 26–27/11/2026 |

## Até onde chegou

- [x] compila
- [x] abre e cria o device D3D12
- [x] apresenta frames — **2048+** numa corrida de 250 s, sem remoção de device
- [x] renderiza a arte de carregamento (cor cheia, pixel-correta)
- [ ] chega ao menu
- [ ] renderiza o mundo (`the world's first frame` nunca é emitido)
- [ ] jogável

## Onde parou

Depois da arte de carregamento a tela fica preta — 205 *black frames* no log. **Nenhum frame chegou a 500 draws**, então o mundo nunca começou a renderizar.

A causa é a placa, não o software: **9.282 dos 26.503 shaders do título são mesh shaders (35%)**, e esta GPU reporta `MeshShaderTier: NOT_SUPPORTED`. O tradutor converte os 8.765 programas de mesh perfeitamente — e não há hardware para executá-los. Um pipeline `[mesh, pixel]` é descartado com:

```
no host pipeline: the host GPU has no mesh shaders (or no shader model 6.6)
```

## Veredito

**Reprovado para RX 550, por falta de recurso de hardware.** Não é ajuste de configuração nem bug de tradutor. Precisa de GPU com mesh shaders e SM 6.6 — RDNA 2+ ou Turing+.

Isto **não** diz que o SixMultiVersion não funciona: diz que esta placa não serve para responder essa pergunta. Com 99,2% dos shaders convertendo, quem tiver hardware compatível tem boa chance de ir muito mais longe.

## Armadilhas que custaram tempo aqui

1. **Agility SDK.** No Windows 10 o executável precisa de `D3D12\D3D12Core.dll` ao lado dele. Sem isso morre em 9 s com `0x887E0003` e nem cria o device — fácil de confundir com incompatibilidade. `cargo xtask agility` resolve.
2. **Cache de shaders.** A chave do cache inclui o hash das fontes do tradutor, então mexer no tradutor invalida tudo; mas se você medir sem mexer, os números vêm congelados do cache (`shaders_translated=0 shaders_cached=1316`).
3. **Medir pelo jogo é lento.** Use `translate --list "<dump>\shaders.rpf"`: 4 minutos, sem GPU, sem abrir o jogo. Aponte para o `shaders.rpf`, não para a pasta do dump (223 GB).
4. **Filtro de log.** Um grep por `DRED` casa com o nome da pasta de captura e inventa "device removed" que não houve.

## Evidência

- `logs/capacidades-d3d12.txt` — a sonda da placa
- `logs/traducao-offline.txt` — o resultado da tradução do pacote inteiro
- `../../../docs/INVESTIGACAO-SHADERS.md` — a investigação completa
