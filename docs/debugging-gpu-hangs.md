# Diagnóstico de falhas de GPU

Este guia organiza evidências para que um relato possa ser reproduzido sem publicar arquivos do jogo.

## Registre

- commit ou versão do programa e comando usado;
- Windows, modelo da GPU, versão do driver e adaptador selecionado;
- capacidades D3D12 reportadas, especialmente Shader Model, mesh shader tier e wave lanes;
- etapa exata, duração, código de erro e mensagem de device removal, se houver;
- trecho de log sanitizado, sem nome de usuário, caminhos privados, tokens ou conteúdo do jogo.

## Separe os tipos de falha

1. Falha antes da criação do device: verifique runtime D3D12, driver e compatibilidade do adaptador.
2. Falha ao criar pipeline: registre recursos consultados e o estágio/shader que a mensagem identifica.
3. Device removed ou TDR: preserve o código retornado e as evidências DRED disponíveis; informe se a tela/driver se recuperou.
4. Tela preta sem remoção do device: registre contagem de frames e etapa atingida; isso não prova sozinho um crash de GPU.

WARP é um adaptador de software útil para alguns testes de fluxo, mas seus resultados não substituem validação em GPU física nem demonstram compatibilidade com recursos ausentes no hardware.

Use o formulário `gpu-driver-fault.yml` e o [modelo de relatório de hardware](../reports/hardware/_TEMPLATE.md). Não envie executáveis, dumps, capturas com dados pessoais ou arquivos proprietários do jogo.
