# Como trabalhar neste repositório

Este repositório existe para **várias pessoas tentarem fazer o SixMultiVersion rodar em hardwares diferentes ao mesmo tempo**, sem que o trabalho de uma apague o da outra.

Leia esta página inteira antes do primeiro commit. As regras são poucas e existem por um motivo concreto: já aconteceu aqui de dois agentes mexerem na mesma árvore ao mesmo tempo e o resultado foi build quebrado, pasta duplicada e um diagnóstico errado que custou horas.

---

## A regra de ouro

> **`main/` ninguém edita direto. Você trabalha só dentro de `contributors/<fabricante>/<seu-nome>/`.**

`main/` só muda quando uma mudança já foi testada por alguém e **promovida** (ver §5). Quem promove abre um PR; não se escreve em `main/` por commit direto.

---

## 1. A estrutura

```
SixMultiVersion/
├─ README.md                 o que é o projeto
├─ CONTRIBUTING.md           esta página
├─ LICENSE                   GPL-3.0-or-later (herdada do upstream)
├─ main/                     a versão canônica, que todo mundo parte dela
├─ docs/
│   └─ HARDWARE.md           tabela do que já foi testado e em que placa
└─ contributors/
    ├─ _TEMPLATE/            copie esta pasta para começar
    ├─ amd/                  quem testa em GPU AMD
    │   └─ <seu-nome>/
    └─ nvidia/               quem testa em GPU NVIDIA
        └─ <seu-nome>/
            ├─ CHANGES.md    o que você mudou e por quê
            ├─ STATUS.md     sua máquina e até onde chegou
            ├─ patches/      seus .patch contra main/
            └─ logs/         as evidências (logs recortados, não brutos)

A pasta é **por fabricante, e dentro dela por pessoa**, porque o que decide o
resultado aqui é a GPU: a mudança que salva uma AMD GCN não faz diferença nenhuma
numa NVIDIA. Agrupar assim deixa óbvio o que já foi coberto e o que falta. Outro
fabricante (Intel, por exemplo) ganha a sua pasta quando aparecer a primeira
pessoa com a placa — veja o `README.md` dentro de `amd/` e `nvidia/`.
```

### Por que patches e não uma cópia inteira do código

Seria mais fácil cada um jogar uma cópia completa de `main/` na sua pasta. **Não faça isso.** Com cinco pessoas o repositório vira cinco códigos diferentes que ninguém consegue comparar, e a pergunta "o que essa pessoa mudou?" passa a exigir um diff manual de 8 MB.

Um `.patch` responde essa pergunta em um arquivo de texto, aplica em `main/` com um comando, e deixa óbvio quando duas pessoas mexeram na mesma linha.

Sim, o Git já faz isso sozinho com branches. A pasta por pessoa existe porque aqui as tentativas são **experimentos paralelos de hardware**, muitos deles becos sem saída que mesmo assim precisam ficar registrados — e um beco sem saída documentado vale mais numa pasta visível do que numa branch que ninguém vai abrir.

---

## 2. Começando

```powershell
git clone https://github.com/Miaumenu/SixMultiVersion.git
cd SixMultiVersion
Copy-Item -Recurse contributors\_TEMPLATE contributors\<amd ou nvidia>\<seu-nome>
```

Preencha `STATUS.md` **antes** de mexer em código. Sem a sua máquina anotada, o seu resultado não significa nada para os outros.

---

## 3. Fazendo uma mudança

1. Trabalhe numa cópia de `main/` fora do repositório (ou numa branch sua).
2. Quando funcionar, gere o patch:

   ```powershell
   git diff > ..\SixMultiVersion\contributors\<fabricante>\<seu-nome>\patches\0001-descricao-curta.patch
   ```

3. Numere os patches em ordem (`0001-`, `0002-`…). Um patch = uma mudança com um motivo.
4. Escreva no `CHANGES.md` o que aquele patch faz, **com a medição antes e depois**.

Para aplicar o patch de outra pessoa:

```powershell
git apply --check contributors\<fabricante>\<nome>\patches\0001-xxx.patch   # confere antes
git apply contributors\<fabricante>\<nome>\patches\0001-xxx.patch
```

---

## 4. O que cada arquivo tem que conter

### `STATUS.md`

| campo | exemplo |
|---|---|
| GPU, VRAM | Radeon RX 550, 2 GB |
| Arquitetura | GCN 4 (Polaris) |
| `MeshShaderTier` | NOT_SUPPORTED |
| `HighestShaderModel` | 6.5 |
| `WaveLaneCountMin` / `Max` | 64 / 64 |
| CPU, RAM | Ryzen 7 3700X, 24 GB |
| Windows, driver | 10.0.19045, AMD 31.0.12029.10015 |
| Até onde chegou | arte 2D renderiza; mundo nunca aparece |

Tire os quatro primeiros com a sonda:

```powershell
.\main\target\diag\sixm-multiversion.exe inspect --help
```

### `CHANGES.md`

Uma entrada por patch, neste formato:

```text
## 0001-nome-curto
Data:
Hipótese:
Arquivos tocados:
Medição ANTES:
Medição DEPOIS:
Teste de regressão:
Status: funcionou / não funcionou / inconclusivo
```

**"Não funcionou" é uma entrada tão valiosa quanto "funcionou"** — evita que a próxima pessoa gaste o mesmo dia. Registre também as hipóteses que você derrubou.

### `logs/`

Só o **recorte** que prova o que você afirma, com o comando que o gerou. Log bruto de 1 MB não entra no repositório.

---

## 5. Promovendo uma mudança para `main/`

Uma mudança só sobe quando:

1. Tem patch, `CHANGES.md` com medição antes/depois e um teste que falha antes e passa depois.
2. **Outra pessoa, em outra máquina**, aplicou o patch e confirmou o resultado.
3. O build passa: `cargo build --profile diag -p sixm-multiversion --features diagnostics`.
4. A suíte passa: `cargo test -p sixm-shader` — com a mesma contagem de falhas de antes, ou menos. Falha ambiental (`no SM 6.6 device available`) não conta, mas **tem que ser a mesma quantidade**.

Aí sim: PR com o patch já aplicado em `main/`, apontando a pasta do contribuinte de origem.

---

## 6. O que nunca entra no repositório

- **Nenhum arquivo do jogo.** Nem executável, nem `.rpf`, nem textura, nem áudio, nem dump, nem parte de dump. O repositório é só código e documento.
- `target/`, caches de shader, `%LOCALAPPDATA%` de ninguém.
- Logs brutos, capturas de frame, binários compilados.
- Qualquer coisa que contorne autenticação, DRM ou licença.
- Chaves, tokens, caminhos com o seu nome de usuário dentro.

O `.gitignore` já barra a maior parte, mas ele não substitui conferir o `git status` antes de commitar.

---

## 7. Evitando pisar no pé um do outro

- **Uma pessoa por área por vez.** Antes de mexer num crate, veja se já tem patch de outra pessoa nele. Se tiver, fale com ela.
- **Nunca rode duas ferramentas automáticas na mesma árvore.** Se você usa um agente de IA, ele trabalha na *sua* pasta, nunca em `main/` nem na pasta de outro. Renomeação em massa, formatação automática e "limpeza" são as que mais quebram as outras pessoas.
- **Puxe antes de começar** (`git pull`) e **abra o PR no mesmo dia** em que terminar. Patch parado uma semana não aplica mais.
- Se o seu patch deixou de aplicar porque `main/` andou, **regenere o patch**; não force.

---

## 8. Origem e licença

O código em `main/` deriva do projeto **VIctory**, de [codec-d/cool_fox](https://github.com/codec-d/cool_fox), sob **GPL-3.0-or-later**. Os nomes `victory*` / `VICTORY_*` foram trocados por `sixm*` / `SIXM_*` neste fork; o código, a licença e a autoria originais continuam sendo de lá, e o `LICENSE` não se altera.

Qualquer coisa que você publique aqui entra sob a mesma licença.
