# SixMultiVersion

Camada de compatibilidade para rodar um título Xbox Series nativamente no Windows, e o esforço coletivo para descobrir **em quais máquinas ela realmente funciona**.

Este repositório é compartilhado: várias pessoas testam em hardwares diferentes ao mesmo tempo, cada uma na sua pasta, e o que funciona sobe para `main/`.

---

## Comece por aqui

1. **[CONTRIBUTING.md](CONTRIBUTING.md)** — as regras. Leia inteiro antes do primeiro commit; existem por um motivo concreto.
2. **[docs/HARDWARE.md](docs/HARDWARE.md)** — o que já foi testado, em que placa, e até onde chegou. Veja se a sua máquina já tem resultado antes de repetir o trabalho.
3. `contributors/_TEMPLATE/` — copie para `contributors/<seu-nome>/` e preencha o `STATUS.md`.

> **A regra de ouro:** ninguém edita `main/` direto. Você trabalha só dentro da sua pasta em `contributors/`.

---

## O que é, tecnicamente

Não é um emulador. O Xbox Series X é x86-64 AMD, a mesma arquitetura de um PC: o executável do título roda **nativo**, sem emulação de CPU. O que esta camada fornece é o que falta — a API do GDK do Xbox, o D3D12-on-Xbox, e um **tradutor de shaders** que converte o código de máquina GFX10 do console em DXIL que uma GPU de PC entende.

É da mesma família do Wine/Proton, não do Xenia.

```
cargo build --profile diag -p sixm-multiversion --features diagnostics
.\SixM-MultiVersion.cmd pareto-residency
```

O jogo não vem no repositório, não vai vir, e nenhum arquivo dele pode ser commitado aqui. Você usa o seu próprio dump, apontado por `SIXM_GAME_DIR`.

---

## Requisitos declarados

Windows x64, CPU com AVX2, e GPU Direct3D 12 com **Shader Model 6.6 e mesh shaders**.

Esse último item é o que mais barra gente: **35% dos shaders do título são mesh shaders** (9.282 de 26.503). Uma GPU sem mesh shader renderiza a interface e a arte 2D normalmente e **nunca mostra o mundo** — o que é fácil de confundir com bug de software. Confira a sua placa em [docs/HARDWARE.md](docs/HARDWARE.md) antes de abrir uma investigação.

---

## Estado do tradutor

Medido sobre o pacote de shaders completo do título, offline:

| | |
|---|---|
| programas traduzidos | **26.084 de 26.084** |
| DXIL emitido — vertex e pixel | 14.247 de 14.247 |
| DXIL emitido — **mesh** | **8.765 de 8.765** |
| DXIL emitido — compute | 2.677 de 2.682 |
| **total que falha** | **202 (0,8%)** |

O tradutor não é o gargalo. Das 202 falhas, 196 são o mesmo shader de geometria (`GS_VoxelizeGeneric`) e estão descritas em `docs/` com a investigação completa, inclusive as hipóteses já derrubadas.

Reproduza na sua máquina:

```powershell
.\main\target\diag\sixm-multiversion.exe translate --list "<seu dump>\shaders.rpf"
```

Leva uns 4 minutos, não abre o jogo e não depende de GPU — é o jeito rápido de medir.

---

## Origem e licença

Deriva do projeto **VIctory**, de [codec-d/cool_fox](https://github.com/codec-d/cool_fox), sob **GPL-3.0-or-later**. Os nomes `victory*` / `VICTORY_*` foram trocados por `sixm*` / `SIXM_*` neste fork; o código, a licença e a autoria originais continuam sendo de lá.

Tudo que você contribuir aqui entra sob a mesma licença.
