# GambiarraLauncher

Projeto de pesquisa sobre compatibilidade do runtime e suporte gráfico no Windows. O código atual é um protótipo técnico; este repositório ainda não entrega um port jogável do título nem um launcher visual completo.

## Um repositório para todos os hardwares

O objetivo é simples: fazer um título Xbox Series rodar no Windows em **qualquer** placa capaz, e deixar registrado, de forma verificável, o que cada placa consegue de verdade.

Não é emulador. O Xbox Series X é x86-64 AMD e o executável do título roda direto na CPU. O que este projeto fornece é a API do GDK, o D3D12-on-Xbox e um tradutor que converte o código de máquina da GPU do console em DXIL.

### Como está em cada hardware

Cada linha é uma máquina **medida**, com relatório e logs no repositório. Nada aqui é estimativa: o que não foi testado está marcado como não testado.

Os degraus, em ordem: `compila` → `device` → `frame` → `arte de carregamento` → `menu` → `mundo` → `jogável`. FPS só aparece a partir de `mundo`, porque abaixo dele o mundo não foi desenhado e um número mediria tela vazia.

| GPU | Classe | Mesh shader | Até onde chegou | FPS | Preset | Relatório |
|---|---|---|---|---|---|---|
| Radeon RX 550 (2 GB) | GCN 4 / Polaris | não | arte de carregamento | — | [`amd-gcn-no-mesh`](configs/presets/amd-gcn-no-mesh.toml) | [RX 550](reports/hardware/rx550-2026-11-27.md) |
| RDNA 2+ (RX 6000 ou mais novo) | — | sim | **não testado** | — | — | — |
| Turing+ (RTX 20 ou mais novo) | — | sim | **não testado** | — | — | — |

**O que a RX 550 ensinou:** o tradutor converte 99,98% dos shaders do título, inclusive 8.765 de 8.765 mesh shaders. O que trava é a placa — 35% dos shaders do título são mesh shaders e esta classe de GPU não tem a unidade. Ou seja: o resultado acima é o teto desta placa, não o do software.

**O experimento que decide o projeto ainda não foi feito:** rodar numa GPU que atende o requisito (mesh shaders, Shader Model 6.6, 8 GB ou mais). Se você tem uma, a sua medição é a contribuição mais valiosa que existe hoje.

### Qual configuração a sua placa recebe

O launcher consulta o adaptador e escolhe sozinho o preset que casa com ele:

    gambiarra-cli.exe preset list
    gambiarra-cli.exe preset show

Um preset é só dado — um TOML em `configs/presets/`, sem código. Ele nunca liga um recurso que a placa não tem, e uma condição sobre capacidade que o driver não respondeu simplesmente não vale. O que você define na mão sempre ganha do preset.

### Contribuir uma medição

Dois comandos, nenhum deles abre o jogo:

    d3d12-capabilities.exe > minha-placa.txt
    gambiarra-cli.exe translate --list "<seu dump>\shaders.rpf" > minha-traducao.txt

Depois, um pull request com dois arquivos: o seu preset em `configs/presets/` e o seu relatório em `reports/hardware/`. O resto a automação confere — inclusive que ninguém declare FPS sem ter renderizado o mundo. Detalhes em [configs/presets/README.md](configs/presets/README.md).


## Estado atual

- O workspace Rust contém o CLI gambiarra-cli e os crates gambiarra-*.
- O relatório local da RX 550 alcança a arte de carregamento, mas não renderiza o mundo: a placa reporta Shader Model 6.5 e ausência de mesh shaders.
- A matriz distingue resultados medidos localmente, resultados externos e hardware ainda não testado.
- Os presets por classe de capacidade estão implementados (`configs/presets/`, comando `preset`). O launcher visual e o benchmark automático ainda não.

O nome do projeto e do repositório no GitHub é **GambiarraLauncher**. A pasta local ainda se chama `BRS-Launcher-GTA6-Port`; os nomes `gambiarra-*` também são usados nos pacotes Rust. Não há arquivos do jogo neste repositório. Não adicione executáveis, pacotes, dumps, texturas, áudio, shaders extraídos, chaves ou dados pessoais.

## Comece aqui

- CONTRIBUTING.md explica o fluxo de branches e pull requests para a equipe e para contribuições externas.
- docs/architecture.md descreve os crates atuais.
- docs/hardware-compatibility.md registra capacidades e resultados de teste.
- docs/gpu-portability.md explica a detecção por recursos, e configs/presets/README.md explica os presets.
- docs/debugging-gpu-hangs.md orienta a coleta de diagnóstico sem anexar arquivos do jogo.
- reports/hardware/_TEMPLATE.md é o formato para enviar um resultado de hardware.

## Compilar e verificar

Requisitos: Windows x64, Rust da versão fixada em rust-toolchain.toml e ferramentas de compilação MSVC/Windows SDK. A GPU precisa ser avaliada pelo relatório de capacidades; Direct3D 12 por si só não garante os recursos usados pelo protótipo.

Na raiz do repositório:

    cargo fmt --all -- --check
    cargo clippy --workspace --locked
    cargo test --workspace --locked
    cargo build --release -p gambiarra-cli

Clippy currently reports inherited warnings in the GPU prototype; CI shows them without making the existing baseline fatal. Compilation errors and failed tests remain blocking.

Para configurar caminhos locais, copie configs/gambiarra-launcher.example.toml para gambiarra-cli.toml e preencha apenas caminhos para arquivos que você possui. O arquivo local é ignorado pelo Git.

## Portabilidade de GPU

AMD, NVIDIA e Intel usam o mesmo caminho D3D12 no host. O runtime consulta recursos reais do adaptador e do driver. Os presets são por **classe de capacidade**, não por fabricante, porque hardware não é uma árvore: duas placas de fabricantes diferentes podem ser equivalentes aqui, e duas do mesmo fabricante podem ser muito diferentes. Um preset só ajusta parâmetros medidos e nunca habilita um recurso que a consulta informou como ausente. Consulte docs/gpu-portability.md e configs/presets/README.md.

## Contribuições

Mudanças de código entram por pull request. A equipe trabalha em branches; contribuidores externos usam forks. Relatórios de hardware podem ser submetidos por formulário ou PR usando o modelo comum; não há uma pasta ativa por pessoa. Veja CONTRIBUTING.md.

## Código aberto e origem

O código mantém a licença e os avisos de origem presentes em LICENSE. Consulte também docs/architecture.md para a proveniência técnica do runtime.
