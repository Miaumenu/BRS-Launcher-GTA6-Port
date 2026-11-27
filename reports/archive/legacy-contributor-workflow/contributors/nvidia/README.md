# NVIDIA

Pastas de quem testa em GPU NVIDIA. Uma pasta por pessoa: `contributors/nvidia/<seu-nome>/`.

| geração | exemplos | onda | mesh shaders | SM 6.6 | o que esperar |
|---|---|---|---|---|---|
| Pascal e anteriores | GTX 10xx | 32 | não | não | não passa do carregamento |
| Turing | RTX 20xx, GTX 16xx | 32 | sim | sim | **alvo** |
| Ampere | RTX 30xx | 32 | sim | sim | **alvo** |
| Ada / Blackwell | RTX 40xx, 50xx | 32 | sim | sim | **alvo** |

Nenhuma máquina NVIDIA foi testada ainda. **É o resultado que mais falta.**

Numa GPU NVIDIA a onda é de 32 lanes, então o caminho exercitado é o inverso do
da AMD GCN: o tradutor rebaixa os programas wave64 do console para ondas de 32
(`host_max_wave() == 32`), código que já existia e nunca foi medido por nós.
Se algo quebrar aí, é território novo — registre com cuidado.
