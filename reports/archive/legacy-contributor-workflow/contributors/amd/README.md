# AMD

Pastas de quem testa em GPU AMD. Uma pasta por pessoa: `contributors/amd/<seu-nome>/`.

| geração | exemplos | onda 32 | mesh shaders | SM 6.6 | o que esperar |
|---|---|---|---|---|---|
| GCN 1–5 | RX 400/500, Vega | **não** (onda só de 64) | não | não | não passa do carregamento; precisa do patch `0001` só para os pipelines de compute serem aceitos |
| RDNA 1 | RX 5000 | sim | **não** | parcial | os mesh shaders barram |
| RDNA 2 | RX 6000 | sim | sim | sim | **alvo** |
| RDNA 3+ | RX 7000, 9000 | sim | sim | sim | **alvo** |

Se a sua é GCN, o teste mais útil que você pode fazer agora é aplicar
`amd/miaumenu/patches/0001-wave32-em-host-de-onda-64.patch` e confirmar se os
números batem — é a confirmação em segunda máquina que falta.
