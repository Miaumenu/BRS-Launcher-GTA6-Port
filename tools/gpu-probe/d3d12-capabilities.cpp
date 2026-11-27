#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <dxgi1_6.h>
#include <d3d12.h>
#include <wrl/client.h>
#include <cstdio>
using Microsoft::WRL::ComPtr;
int main() {
  ComPtr<IDXGIFactory6> factory;
  HRESULT hr = CreateDXGIFactory2(0, IID_PPV_ARGS(&factory));
  if (FAILED(hr)) { std::printf("CreateDXGIFactory2 failed: 0x%08X\n", (unsigned)hr); return 1; }
  for (UINT i = 0;; ++i) {
    ComPtr<IDXGIAdapter1> adapter;
    if (factory->EnumAdapters1(i, &adapter) == DXGI_ERROR_NOT_FOUND) break;
    DXGI_ADAPTER_DESC1 desc{}; adapter->GetDesc1(&desc);
    char name[256]{}; WideCharToMultiByte(CP_UTF8,0,desc.Description,-1,name,sizeof(name),nullptr,nullptr);
    std::printf("Adapter %u: %s | dedicated VRAM: %llu MiB | software: %s\n", i, name,
      (unsigned long long)(desc.DedicatedVideoMemory / (1024*1024)),
      (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE) ? "yes" : "no");
    ComPtr<ID3D12Device> device;
    hr = D3D12CreateDevice(adapter.Get(), D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&device));
    if (FAILED(hr)) { std::printf("  D3D12CreateDevice: 0x%08X\n", (unsigned)hr); continue; }
    D3D12_FEATURE_DATA_D3D12_OPTIONS7 opt7{};
    hr = device->CheckFeatureSupport(D3D12_FEATURE_D3D12_OPTIONS7, &opt7, sizeof(opt7));
    std::printf("  MeshShaderTier: %s (0x%08X)\n", SUCCEEDED(hr) ? (opt7.MeshShaderTier == D3D12_MESH_SHADER_TIER_NOT_SUPPORTED ? "NOT_SUPPORTED" : "SUPPORTED") : "query failed", (unsigned)hr);
    D3D12_FEATURE_DATA_SHADER_MODEL sm{};
    const D3D_SHADER_MODEL candidates[] = {D3D_SHADER_MODEL_6_7,D3D_SHADER_MODEL_6_6,D3D_SHADER_MODEL_6_5,D3D_SHADER_MODEL_6_0,D3D_SHADER_MODEL_5_1};
    bool found=false;
    for (auto candidate : candidates) { sm.HighestShaderModel=candidate; hr=device->CheckFeatureSupport(D3D12_FEATURE_SHADER_MODEL,&sm,sizeof(sm)); if(SUCCEEDED(hr)){std::printf("  HighestShaderModel: %u.%u\n", ((unsigned)sm.HighestShaderModel>>4)&0xF, (unsigned)sm.HighestShaderModel&0xF);found=true;break;} }
    if(!found) std::printf("  ShaderModel query failed: 0x%08X\n",(unsigned)hr);
  }
  return 0;
}
