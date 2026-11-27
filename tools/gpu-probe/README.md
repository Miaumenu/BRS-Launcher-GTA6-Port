# GPU probe — planejado

Ferramenta futura para exibir as capacidades D3D12 consultadas no adaptador escolhido. Ainda não há executável nesta pasta; não use nomes de GPU como substituto de consultas de recursos.

## d3d12-capabilities.cpp

Sonda mínima: lista cada adaptador D3D12 com VRAM dedicada, `MeshShaderTier` e
`HighestShaderModel`. É o primeiro comando a rodar numa máquina nova — responde
em segundos se a placa atende os requisitos, antes de perder horas achando que
um limite de hardware é bug de software.

```powershell
cl /EHsc /O2 d3d12-capabilities.cpp /link d3d12.lib dxgi.lib
.\d3d12-capabilities.exe
```

Saída numa RX 550 (GCN 4), que **não** atende:

```
Adapter 0: Radeon RX550/550 Series | dedicated VRAM: 2027 MiB | software: no
  MeshShaderTier: NOT_SUPPORTED (0x00000000)
  HighestShaderModel: 6.5
```

Falta aqui a leitura de `D3D12_FEATURE_D3D12_OPTIONS1`
(`WaveLaneCountMin`/`Max`), que decide se a correção de wave32 importa na
máquina. O runtime registra isso no log (`host wave lanes min=.. max=..`).
