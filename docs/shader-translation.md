# Tradução de shaders

O crate `gambiarra-shader` traduz programas do formato do console para DXIL usado pelo caminho D3D12 do host. O inventário e a investigação detalhada estão em [Investigação de tradução](shader-translation-investigation.md).

Ao relatar uma falha, inclua a versão do tradutor, o identificador/hash do programa, o estágio, a mensagem e um caso sintético reproduzível quando possível. Não envie shaders ou arquivos extraídos do jogo.

A contagem de programas traduzidos é específica ao pacote e à versão medidos; ela não prova que uma GPU consegue executar todos os pipelines. A capacidade do host e a tradução são verificações distintas.
