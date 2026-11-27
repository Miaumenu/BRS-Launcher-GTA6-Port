# Scripts de desenvolvimento

`check-dev-env.ps1` verifica pré-requisitos sem instalar ferramentas nem alterar o computador.

`cache-shaders.ps1` chama o subcomando `shaders` do CLI já existente para pré-compilar os shaders encontrados em caminhos fornecidos pelo desenvolvedor. O cache é escrito no diretório local configurado, fora do repositório.

O benchmark físico de GPU permanece planejado: o workspace ainda não possui um harness capaz de produzir métricas comparáveis de frametime e VRAM. Não trate `cargo test` como benchmark de GPU.
