# Portabilidade entre GPUs

## Caminho atual

O runtime tem um caminho de host D3D12 comum. AMD, NVIDIA e Intel não precisam de cópias inteiras do renderer. O código consulta recursos expostos pelo D3D12 e escolhe somente caminhos que o adaptador e o driver confirmam suportar.

Use `ID3D12Device::CheckFeatureSupport` para consultar recursos e níveis, incluindo Shader Model, mesh shaders, wave operations, ray tracing e suporte a formatos. O nível de Direct3D, isoladamente, não garante cada recurso opcional. [Microsoft: consultas de recursos D3D12](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-id3d12device-checkfeaturesupport) · [Microsoft: Shader Model](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/ne-d3d12-d3d_shader_model) · [Microsoft: mesh shader tier](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/ne-d3d12-d3d12_mesh_shader_tier)

## Código comum, perfis e exceções

- O crate `sixm-gpu` mantém o caminho comum D3D12.
- `configs/hardware/profiles/` pode receber ajustes medidos para um grupo de GPUs; um perfil não pode declarar como disponível um recurso ausente.
- Código em `sixm-gpu` específico de fabricante só entra para uma falha reproduzida, identificada por adaptador/driver e coberta por regressão.
- Outro backend gráfico, como Vulkan, seria uma implementação separada de API, caso seja de fato desenvolvido e validado.

A documentação da AMD também recomenda consultar a API gráfica para confirmar recursos: suporte pode depender de hardware, sistema e driver, e não apenas da geração da GPU. [AMD AGS](https://gpuopen.com/amd-gpu-services-ags-library/)

## Perfis

Os diretórios AMD, NVIDIA e Intel começam sem arquivos de ajuste. Um perfil só deve ser adicionado depois de existir um relatório reproduzível para a GPU, versão de driver e cenário. Os perfis registram parâmetros de configuração; a matriz de compatibilidade registra o resultado.

## Quando criar uma pasta de fabricante no código

Não crie uma cópia do backend por fabricante. Se um defeito específico for demonstrado, adicione uma exceção pequena dentro do crate de GPU, com condição de ativação, motivo, teste de regressão e critério para removê-la. A estrutura interna atual de `sixm-gpu/src` permanece intacta até que uma refatoração de módulos seja necessária.
