# Compatibilidade de hardware

Esta matriz separa medições deste checkout de relatos externos e de hipóteses. Uma geração de GPU indicada como alvo não é considerada compatível até ser testada com a versão e o driver anotados.

## Requisitos que o runtime consulta

O projeto usa D3D12 no host. A inicialização deve consultar o adaptador e o driver para obter o Shader Model, mesh shader tier, wave operations, ray tracing e suporte aos formatos realmente usados. O nome AMD, NVIDIA ou Intel não substitui essas consultas.

| Resultado | Significado |
|---|---|
| Compatível testado | O cenário definido foi concluído na combinação registrada de GPU, driver e sistema. |
| Parcial | Algumas fases funcionaram, mas há uma limitação reproduzida. |
| Bloqueado por capacidade | Uma capacidade obrigatória consultada não existe no adaptador. |
| Não testado | Não há resultado reproduzível para essa combinação. |

## Resultados conhecidos

| Adaptador | Capacidades observadas | Resultado | Evidência |
|---|---|---|---|
| Radeon RX 550, GCN 4, 2 GB | Mesh shader ausente; Shader Model 6.5; wave lanes 64/64 | Parcial: apresenta arte de carregamento, não inicia renderização do mundo | [Relatório RX 550](../reports/hardware/rx550-2026-11-27.md) |
| WARP | Adaptador de software; capacidades abaixo dos requisitos do mundo | Não é uma validação de GPU física nem de desempenho | [Relatório RX 550](../reports/hardware/rx550-2026-11-27.md) |
| Radeon RX 7900 XTX | Relato externo de execução; sem reprodução neste checkout | Não verificado pela equipe | Referenciado no histórico do README técnico |

## Antes de adicionar um resultado

1. Registre modelo e ID do adaptador, driver, Windows, commit e capacidades retornadas pela API.
2. Informe o comando, a etapa alcançada, o resultado esperado e o observado.
3. Anexe apenas trechos de log sanitizados; não anexe executáveis, arquivos do jogo, dumps ou caminhos pessoais.
4. Envie um relatório em `reports/hardware/` usando o modelo comum.

Consulte [Portabilidade de GPU](gpu-portability.md) e [Diagnóstico de falhas de GPU](debugging-gpu-hangs.md).
