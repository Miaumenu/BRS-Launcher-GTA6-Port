# BRS-Launcher-GTA6-Port

Projeto público de pesquisa sobre compatibilidade do runtime e suporte gráfico no Windows. O código atual é um protótipo técnico; este repositório ainda não entrega um port jogável de GTA 6 nem um launcher visual completo.

## Estado atual

- O workspace Rust contém o CLI sixm-multiversion e os crates sixm-*.
- O relatório local da RX 550 alcança a arte de carregamento, mas não renderiza o mundo: a placa reporta Shader Model 6.5 e ausência de mesh shaders.
- A matriz distingue resultados medidos localmente, resultados externos e hardware ainda não testado.
- O launcher visual, perfis de GPU por fabricante e benchmark automático ainda não estão implementados.

O nome do projeto é **BRS-Launcher-GTA6-Port**; o estado acima descreve o código que existe hoje. Não há arquivos do jogo neste repositório. Não adicione executáveis, pacotes, dumps, texturas, áudio, shaders extraídos, chaves ou dados pessoais.

## Comece aqui

- CONTRIBUTING.md explica o fluxo de branches e pull requests para a equipe e para contribuições externas.
- docs/architecture.md descreve os crates atuais.
- docs/hardware-compatibility.md registra capacidades e resultados de teste.
- docs/gpu-portability.md explica a detecção por recursos e os perfis de ajuste.
- docs/debugging-gpu-hangs.md orienta a coleta de diagnóstico sem anexar arquivos do jogo.
- reports/hardware/_TEMPLATE.md é o formato para enviar um resultado de hardware.

## Compilar e verificar

Requisitos: Windows x64, Rust da versão fixada em rust-toolchain.toml e ferramentas de compilação MSVC/Windows SDK. A GPU precisa ser avaliada pelo relatório de capacidades; Direct3D 12 por si só não garante os recursos usados pelo protótipo.

Na raiz do repositório:

    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked
    cargo test --workspace --locked
    cargo build --release -p sixm-multiversion

Clippy currently reports inherited warnings in the GPU prototype; CI shows them without making the existing baseline fatal. Compilation errors and failed tests remain blocking.

Para configurar caminhos locais, copie configs/brs-launcher-gta6-port.example.toml para sixm-multiversion.toml e preencha apenas caminhos para arquivos que você possui. O arquivo local é ignorado pelo Git.

## Portabilidade de GPU

AMD, NVIDIA e Intel usam o mesmo caminho D3D12 no host. O runtime consulta recursos reais do adaptador e do driver. Perfis por fabricante só podem ajustar parâmetros medidos; não podem habilitar um recurso que a consulta informou como ausente. Consulte docs/gpu-portability.md.

## Contribuições

Mudanças de código entram por pull request. A equipe trabalha em branches; contribuidores externos usam forks. Relatórios de hardware podem ser submetidos por formulário ou PR usando o modelo comum; não há uma pasta ativa por pessoa. Veja CONTRIBUTING.md.

## Código aberto e origem

O código mantém a licença e os avisos de origem presentes em LICENSE. Consulte também docs/architecture.md para a proveniência técnica do runtime.
