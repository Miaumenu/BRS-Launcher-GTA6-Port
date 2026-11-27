# Presets de hardware

Um preset diz quais ajustes uma **classe de placa de vídeo** recebe. É só dado:
um arquivo TOML, sem código, sem lógica. O launcher consulta o D3D12, vê o que
a sua placa realmente é, e escolhe o preset que casa.

```powershell
.\gambiarra-cli.exe preset list
```

```powershell
.\gambiarra-cli.exe preset show
```

O `list` marca com `->` o preset que esta máquina receberia. O `show` abre ele
inteiro: de onde veio, o que liga, em que máquina foi medido e até onde chegou.

## As duas regras que não se negociam

**1. Um preset nunca cria capacidade que a placa não tem.** Ele só escolhe
entre caminhos que o driver já permitiu. E uma condição sobre capacidade que o
driver **não respondeu** não vale: o preset é descartado em vez de aplicado no
escuro. Não se chuta hardware.

**2. FPS só existe a partir do degrau `world`.** Abaixo dele o mundo não foi
desenhado, então um número de FPS estaria medindo tela vazia. A automação
recusa o arquivo — não é o revisor que precisa lembrar.

## Por que não há uma pasta por fabricante

Porque hardware não é uma árvore, é um conjunto de capacidades. Uma RTX 3060 e
uma RX 6600 são de fabricantes diferentes e, para este runtime, quase iguais.
Uma RX 550 e uma RX 6600 são do mesmo fabricante e são mundos diferentes.

Se a pasta fosse o fabricante, a mesma configuração acabaria copiada em `amd/`
e `nvidia/`, e no dia em que ela mudasse uma das duas ficaria para trás. Por
isso a pasta é plana e o nome do arquivo descreve a **classe de capacidade**
(`amd-gcn-no-mesh`), não o modelo da placa: um arquivo serve a todas as placas
parecidas.

Este repositório já tentou o outro caminho — havia `configs/hardware/profiles/`
com `amd/`, `intel/` e `nvidia/`, três pastas vazias que nenhuma linha de
código lia. Foi removida.

## Como o preset é escolhido

1. O launcher consulta o adaptador: fabricante, mesh shader, shader model,
   onda mínima e memória de vídeo.
2. Descarta todo preset cujo `[match]` não valha — inclusive os que dependem
   de uma capacidade que a consulta não devolveu.
3. Entre os que sobram, fica o que declara **mais** condições. Empate é
   desfeito pelo `id`, então toda máquina escolhe igual.
4. Nada casou: cai no `baseline.toml`.

O que **você** definiu na mão sempre ganha do preset. Uma variável de ambiente
já definida é respeitada, e o `--preset <id>` força um preset específico.
`--no-preset` não aplica nenhum.

## Contribuir um preset

A medição mais valiosa que existe hoje é de uma GPU com mesh shaders e Shader
Model 6.6 — nenhuma foi testada ainda. Dois comandos, e nenhum deles abre o
jogo:

```powershell
.\d3d12-capabilities.exe > minha-placa.txt
```

```powershell
.\gambiarra-cli.exe translate --list "<seu dump>\shaders.rpf" > minha-traducao.txt
```

Depois, um pull request com **dois arquivos**:

1. `configs/presets/<id>.toml` — copie o `_schema.toml`, que explica cada
   campo. O nome do arquivo tem de ser igual ao `id`.
2. `reports/hardware/<placa>-<data>.md` — o relatório, com os dois logs
   acima colados.

O preset aponta para o relatório no campo `[measured].report`, e o relatório
tem de existir no mesmo pull request. **Preset sem evidência não entra** — é o
que separa medição de chute.

A escada de progresso, que é o campo `stage_reached`:

| degrau | o que significa |
|---|---|
| `compiles` | compila, e nada além disso foi tentado |
| `device` | cria o device D3D12 |
| `frame` | apresenta ao menos um frame |
| `loading-art` | desenha a arte de carregamento |
| `menu` | chega ao menu |
| `world` | renderiza o mundo (primeiro frame com 500 draws ou mais) |
| `playable` | jogável |

A maioria das máquinas não vai ter FPS para reportar — vai ter um degrau. Isso
é informação completa, não resposta incompleta.

## Arquivos

| arquivo | o que é |
|---|---|
| `_schema.toml` | o gabarito, comentado campo por campo. Comece por ele. Arquivos que começam com `_` não são carregados como preset. |
| `baseline.toml` | o que vale quando nada casa. Sem ajuste nenhum, de propósito. |
| `amd-gcn-no-mesh.toml` | AMD pré-RDNA, onda 64, sem mesh shader. A única classe medida até hoje. |

Ver também [Portabilidade de GPU](../../docs/gpu-portability.md) e
[Compatibilidade de hardware](../../docs/hardware-compatibility.md).
