# Investigação de renderização na RX 550 — 27/11/2026

## Objetivo e escopo

Investigar por que a execução atual não desenha os quadros esperados, compilar o launcher, executá-lo com logs e preparar a continuação do diagnóstico de desempenho. O trabalho foi feito apenas neste repositório. Nenhum arquivo do projeto SixM na pasta-mãe foi alterado, nenhum código dos launchers de referência foi copiado e nada foi commitado ou enviado ao GitHub.

## Resultado resumido

**Não foi possível medir FPS de renderização nesta máquina.** Em todas as execuções, os quadros apresentados registraram `drawn=0` e `dispatched=0`. Isso significa que o programa apresentava quadros sem executar desenhos ou despachos de shaders do host. Um número de FPS nessa etapa não representa o desempenho de renderização.

O launcher requer DXIL com Shader Model 6.6. A pilha D3D12 usada nos testes informou suporte somente até Shader Model 6.5, e o debug layer recusou shaders `vs_6_6`, `ps_6_6` e `cs_6_6`. Isso ainda não prova que o limite seja físico da GPU: o Windows 10 pode estar usando o runtime D3D12 integrado, que não reconhece as flags de indexação de heap necessárias. A RX 550 também informa ausência de mesh shaders, necessários para renderizar o mundo. Portanto, os testes desta etapa não validam a cena de jogo nem localizam uma perda de FPS no mundo.

## Hardware e software observados

| Item | Observação |
|---|---|
| GPU | Radeon RX 550 / 550 Series, aproximadamente 2 GB |
| Driver instalado | AMD `31.0.12029.10015`, datado de 29/11/2022 |
| Shader Model exposto à consulta D3D12 usada pelo preset | 6.5 (não separar ainda limite do runtime de limite físico da GPU) |
| Mesh shader tier | Não suportado |
| Wave lanes | 64/64 |
| Sistema registrado no relatório de hardware | Windows 10.0.19045 |
| Preset correspondente | `amd-gcn-no-mesh` |

A página oficial da AMD consultada em 27/11/2026 lista o Adrenalin 26.11.2 WHQL, lançado em 25/11/2026, para RX 550 em Windows 10/11. O driver instalado está bastante defasado. Isso justifica uma revalidação depois de atualizar o driver; não prova que uma atualização habilitará Shader Model 6.6 ou mesh shaders.

Fonte: [drivers oficiais da Radeon RX 550](https://www.amd.com/en/support/downloads/drivers.html/graphics/radeon-600-500-400/radeon-rx-500-series/radeon-rx-550.html).

## Execuções realizadas

As saídas ficaram fora do checkout, conforme a política de assets do projeto, em:

```text
%LOCALAPPDATA%\GambiarraLauncher\shader-studies\
```

Pastas de execução:

- `baseline` — primeira execução na RX 550 com logs informativos.
- `root-signature-diagnostic` — execução com debug layer e DRED.
- `root-signature-no-index` — experimento diagnóstico retirando temporariamente as flags de indexação direta das root signatures.
- `warp-baseline` — execução no adaptador de software WARP.
- `compatibility-guard` — execução depois da checagem antecipada de Shader Model 6.6.
- `first-attempt-blocked` — tentativa inicial bloqueada pela política de cache do repositório; logs preservados nessa pasta, sem apagar caches do usuário.

As execuções físicas registraram `drawn=0`; a execução WARP também não produziu um caminho válido para medir renderização. A RX 550 não exibiu uma janela de jogo estável que pudesse ser inspecionada e usada interativamente. A variável documentada `GAMBIARRA_KEYS=40:Enter` foi incluída para enviar Enter durante a execução, mas a execução terminou com falha antes de uma transição jogável confirmada.

## Diagnóstico técnico

1. Com a configuração padrão, algumas criações de root signature falharam com `E_INVALIDARG` (`0x80070057`). O log de serialização identificou `Unsupported bit-flag set (root signature flags c00)`: o runtime ativo não aceita as flags de indexação direta exigidas pelas traduções.
2. No experimento sem indexação direta, o debug layer mostrou que o runtime ativo aceita até `cs_6_5`, `vs_6_5` e `ps_6_5`, mas recebeu shaders `*_6_6`. A root signature sem as flags foi aceita, porém o pipeline e os shaders continuaram sem executar.
3. Com o build de diagnóstico anterior às alterações locais de Agility SDK descritas abaixo, o código consultou Shader Model 6.6 antes de criar pipelines. O log confirmou `supported=false` e reportou a incompatibilidade antes de insistir na criação.
4. O launcher ainda encerrou com exceção `0xc0000005` dentro da imagem do título. Essa falha permanece uma investigação separada; não foi demonstrado que ela seja a causa do limite de Shader Model ou um problema de FPS.
5. Nenhuma dessas execuções atingiu a etapa `world` registrada no sistema de presets/relatórios. Não há base para comparar FPS, p50 ou p95.

## Alterações feitas no código

- Em `crates/gambiarra-gpu/src/host.rs`, foi centralizada a consulta D3D12 de Shader Model 6.6 e seu resultado é registrado uma vez.
- Em `crates/gambiarra-gpu/src/pipeline/host.rs`, a criação de pipelines é recusada com uma mensagem explícita quando o adaptador não alcança Shader Model 6.6.
- O caminho existente de verificação de mesh shaders agora reutiliza a mesma consulta de Shader Model.
- Falhas de `CreateRootSignature` agora registram a quantidade de parâmetros e samplers, além do HRESULT.

Essas mudanças não reduzem o requisito do runtime e não alteram o nível de Shader Model emitido. Elas tornam a incompatibilidade observável e evitam tentar criar pipelines que o driver já informou não suportar.

### Referência recebida: crash com FSR 4 na RTX 4060

Fonte analisada: PDF enviado `Crash do VIctory com FSR4 na RTX 4060.pdf`, 2 páginas, datado de 27/11/2026. Ele relata, em outra máquina, `DeserializeComputePipelineStateEx FAILED hr=0x80004001` (`E_NOTIMPL`), fallback para `NewBE` e depois uma access violation por ponteiro nulo numa thread do título. O próprio documento trata a relação entre fallback e crash como hipótese, não como prova. Ele também recomenda comparar as linhas imediatamente anteriores ao fallback e os módulos carregados.

Há pontos concretos para comparar com este checkout:

- Os logs locais mostram `amd_fidelityfx_loader_xbox.dll` carregada e, numa execução, `amd_fidelityfx_upscaler_xbox.dll` carregada com 7 exports HLE, 79 pass-through e nenhum import ausente.
- Há também um aviso `a host DLL outside the host policy: refused dll="amd_fidelityfx_upscaler_xbox.dll"` no fim do log baseline, depois das mensagens do crash. É preciso confirmar se foi uma tentativa tardia/separada de carregamento; por ordem observada, não é evidência de que tenha causado o crash.
- O [stub em `device.rs`](../../crates/gambiarra-gpu/src/device.rs) declara os métodos D3D12 de dispositivo nos slots `0x1c8` e `0x218` como métodos de criação de pipeline não implementados. Ele devolve `E_NOTIMPL`, define `out=null` e registra as descrições; a documentação do próprio stub diz que a estrutura dessas descrições ainda é desconhecida.
- Isso torna plausível que uma chamada de pipeline do caminho FidelityFX encontre um limite semelhante ao PDF. Ainda não foi provado que qualquer desses slots seja `DeserializeComputePipelineStateEx`; o método precisa ser mapeado antes de ligar uma coisa à outra.
- O crash local observado é `0xc0000005` em `gambiarra-image-...dll+0xb5a5f8`, leitura no endereço `0x68`. O PDF descreve escrita em endereço nulo dentro de outra imagem. São crashes da mesma classe geral, mas com endereços/operações e máquinas diferentes; não são evidência de uma causa comum.
- Não encontramos nos logs locais as strings `DeserializeComputePipelineStateEx`, `NewBE` ou uma mensagem explícita de erro do FSR 4. A semelhança observada até agora é a presença do upscaler, os stubs `E_NOTIMPL` e o crash nulo.

Próxima investigação que o PDF sugere: identificar o significado dos slots `0x1c8`/`0x218` e correlacioná-los aos pontos de chamada anotados no código (`title+0x2477578` e `title+0x24800d5`), instrumentar HRESULT e resultado da saída por chamada e conferir a ordem com os logs do FidelityFX. Não retornar sucesso com um PSO falso ou uma saída nula: o stub comenta que isso entregaria um objeto inválido ao título.

As sugestões do PDF para o usuário da RTX 4060 (trocar o upscaler por DLSS/FSR 3/XeSS/TAA, remover mods, validar arquivos do jogo, limpar caches e atualizar o driver) são recomendações para aquela instalação e **não foram executadas**. Para este projeto, o dado transferível é a falha `E_NOTIMPL` seguida de fallback e crash, que deve ser comparada com os stubs locais e com a sequência completa de logs.

### Alterações locais de Agility SDK observadas depois da execução

Ao reabrir o checkout para esta atualização, a árvore já incluía `crates/gambiarra-gpu/src/agility.rs` não rastreado e alterações em `crates/gambiarra-gpu/src/host.rs`/`lib.rs` para tentar carregar o Agility SDK. O módulo descreve exatamente o problema de versão do runtime integrado e pode mudar o resultado da consulta D3D12. **O build bem-sucedido e os logs citados neste relatório são anteriores a essa implementação; ela ainda precisa ser auditada, compilada e executada antes de reutilizar a conclusão `supported=false`.** Preserve essas alterações locais.

## Compilação e integridade

Build realizado com sucesso:

```powershell
cargo build --locked --profile diag -p gambiarra-cli --features diagnostics
```

O build terminou com avisos preexistentes de código não usado, sem erros. `git diff --check` não apontou problemas de whitespace. Nenhum teste automatizado foi acrescentado ou executado nesta etapa.

Estado deixado de propósito: alterações locais não commitadas. Conferir `git status` antes de qualquer commit ou operação remota; não fazer push automaticamente.

## Trabalho que falta

1. **Preservar e auditar as alterações locais de Agility SDK** (`agility.rs`, `lib.rs`, `host.rs`), compilar o estado atual e executar com um Agility SDK válido. Verificar no log se ele realmente foi ativado, qual versão/diretório foi carregado e qual Shader Model a consulta D3D12 retorna; conferir se a serialização aceita as flags `0xc00`.
2. **Reexaminar as capacidades reais depois do Agility SDK** e, se necessário, atualizar o driver da RX 550 pelo pacote oficial AMD e reiniciar. A RX 550 já informa ausência de mesh shaders; se isso persistir, ela não poderá validar renderização do mundo mesmo que o runtime passe a aceitar Shader Model 6.6.
3. **Mapear o caminho FidelityFX**: relacionar os métodos de dispositivo `0x1c8`/`0x218` a `DeserializeComputePipelineStateEx` (ou descartar essa correspondência), identificar o chamador e registrar HRESULT/saída e mensagens imediatamente antes do crash.
4. **Reproduzir em GPU compatível** com Shader Model 6.6 e mesh shaders e confirmar pipeline válido, `drawn > 0` e alcance da etapa `world`.
5. **Só então medir FPS.** Fixar cena, preset, resolução e cache aquecido; registrar tempo de CPU de submissão, tempo/ocupação da GPU e distribuição de frame time, incluindo mediana e p95. Rodar `GAMBIARRA_GPU_TIMING=1` em uma execução separada, pois o timing por comando perturba a execução.
6. Com medições válidas, retomar os testes controlados das opções de shader. O mask padrão do otimizador é `63`; `GAMBIARRA_SHADER_OPT=0` desliga as otimizações e não deve ser presumido como correção de FPS. Registrar uma variável por execução e manter cópias dos logs em `%LOCALAPPDATA%`.

Comandos úteis depois de atualizar o driver ou trocar de GPU:

```powershell
cargo build --locked --profile diag -p gambiarra-cli --features diagnostics
.\target\diag\gambiarra-cli.exe preset show
$env:GAMBIARRA_KEYS = '40:Enter'
$env:GAMBIARRA_LOG = 'info'
$env:GAMBIARRA_D3D12_DEBUG = '1'
.\target\diag\gambiarra-cli.exe run 'C:\caminho\para\game_scarlett_master.exe' --console series-s --execute
```

Defina `GAMBIARRA_CACHE_DIR` para uma pasta de cache fora do checkout antes de executar. Não guardar executáveis, dumps ou arquivos derivados do jogo no repositório.

## Referências arquiteturais consultadas

Não houve cópia de código. Os pontos aplicáveis ao método de diagnóstico foram:

- [VIctory-7900XTX](https://github.com/avatarmanz/VIctory-7900XTX): separar custo da thread de submissão do tempo/ocupação da GPU, manter a cena fixa e testar imagem correta antes de comparar desempenho.
- [ScarlettPort](https://github.com/ZigZagBoi1/ScarlettPort) e [releases](https://github.com/ZigZagBoi1/ScarlettPort/releases): cache e pré-compilação de pipelines ajudam no custo de preparação, mas não garantem FPS alto; medir também frame times de cauda, como p95.

## Instrução para a próxima IA

Leia este relatório e `docs/ESTADO-E-CONTINUACAO.md`. Preserve as alterações locais, especialmente o trabalho de Agility SDK que ainda não foi incluído no build descrito acima. Investigue a correspondência dos slots D3D12 não implementados com o incidente de FSR 4 antes de concluir que os crashes são iguais. Não trate frames com `drawn=0` como FPS do jogo e não altere o projeto SixM na pasta-mãe.
