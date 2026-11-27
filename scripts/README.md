# Scripts de desenvolvimento

`check-dev-env.ps1` verifica pré-requisitos sem instalar ferramentas nem alterar o computador.

`cache-shaders.ps1` chama o subcomando `shaders` do CLI já existente para pré-compilar os shaders encontrados em caminhos fornecidos pelo desenvolvedor. O cache é escrito no diretório local configurado, fora do repositório.

O benchmark físico de GPU permanece planejado: o workspace ainda não possui um harness capaz de produzir métricas comparáveis de frametime e VRAM. Não trate `cargo test` como benchmark de GPU.

## run-loop.py

Uma iteração completa do laço: compila, roda o jogo, segue o stderr ao vivo,
classifica cada linha, para sozinho no primeiro frame do mundo / erro fatal /
tempo limite, e escreve um relatório curto em vez de um megabyte de log.

```powershell
$env:SIXM_GAME_DIR = "D:\seu\dump"
python scripts\run-loop.py --label teste --build --dred --timing
```

Usa `SIXM_KEYS` (aperta teclas sem precisar de foco — é como se passa da tela
de início), `SIXM_CAPTURE` (frames em PNG) e `SIXM_CAPTURE_WORLD` (marca o
primeiro frame com 500 draws, o sinal de que o mundo começou a renderizar;
**sem essa variável a mensagem não sai**).

Nada do jogo é copiado: o dump fica onde está, apontado por `SIXM_GAME_DIR`.
